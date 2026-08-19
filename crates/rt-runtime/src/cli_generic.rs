use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Stdio;
use std::time::Duration;

use futures::{Stream, StreamExt};
use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

use crate::{AgentBackend, Availability, HarnessCaps, TurnEvent, TurnRequest, WireMessage};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Clone)]
pub struct CliGeneric {
    command: Option<String>,
    args: Vec<String>,
    args_error: Option<String>,
    timeout: Duration,
}

impl Default for CliGeneric {
    fn default() -> Self {
        Self::from_env()
    }
}

impl CliGeneric {
    pub fn from_env() -> Self {
        let command = std::env::var("RUSTTRAYCER_GENERIC_CMD")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let mut this = Self {
            command,
            args: Vec::new(),
            args_error: None,
            timeout: DEFAULT_TIMEOUT,
        };
        match std::env::var("RUSTTRAYCER_GENERIC_ARGS") {
            Ok(raw) if !raw.trim().is_empty() => this.apply_args_json(&raw),
            _ => {}
        }
        this
    }

    pub fn new(path: impl Into<String>) -> Self {
        let path = path.into();
        Self {
            command: if path.trim().is_empty() {
                None
            } else {
                Some(path)
            },
            args: Vec::new(),
            args_error: None,
            timeout: DEFAULT_TIMEOUT,
        }
    }

    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self.args_error = None;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    fn apply_args_json(&mut self, raw: &str) {
        match parse_generic_args(raw) {
            Ok(args) => {
                self.args = args;
                self.args_error = None;
            }
            Err(e) => {
                self.args.clear();
                self.args_error = Some(e);
            }
        }
    }
}

fn parse_generic_args(raw: &str) -> Result<Vec<String>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let value: serde_json::Value = serde_json::from_str(trimmed)
        .map_err(|e| format!("RUSTTRAYCER_GENERIC_ARGS invalid JSON: {e}"))?;
    let arr = value
        .as_array()
        .ok_or_else(|| "RUSTTRAYCER_GENERIC_ARGS must be a JSON array of strings".to_string())?;
    let mut out = Vec::with_capacity(arr.len());
    for (i, v) in arr.iter().enumerate() {
        match v.as_str() {
            Some(s) => out.push(s.to_string()),
            None => {
                return Err(format!("RUSTTRAYCER_GENERIC_ARGS[{i}] is not a string"));
            }
        }
    }
    Ok(out)
}

fn resolve_binary(prog: &str) -> Option<PathBuf> {
    if prog.contains('/') || prog.contains('\\') {
        let p = Path::new(prog);
        if p.is_file() {
            return p.canonicalize().ok();
        }
        return None;
    }
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(prog);
        if candidate.is_file() {
            return candidate.canonicalize().ok();
        }
    }
    None
}

impl AgentBackend for CliGeneric {
    fn id(&self) -> &'static str {
        "cli.generic"
    }

    fn available(&self) -> Availability {
        match &self.command {
            None => Availability {
                available: false,
                detail: "RUSTTRAYCER_GENERIC_CMD unset".into(),
            },
            Some(cmd) => match resolve_binary(cmd) {
                Some(p) => Availability {
                    available: true,
                    detail: format!("RUSTTRAYCER_GENERIC_CMD={}", p.display()),
                },
                None => Availability {
                    available: false,
                    detail: format!("RUSTTRAYCER_GENERIC_CMD={cmd} (not found)"),
                },
            },
        }
    }

    fn caps(&self) -> HarnessCaps {
        HarnessCaps::CLI_GENERIC
    }

    fn start_turn(&self, req: TurnRequest) -> Pin<Box<dyn Stream<Item = TurnEvent> + Send>> {
        let command = self.command.clone();
        let args = self.args.clone();
        let args_error = self.args_error.clone();
        let timeout = self.timeout;
        Box::pin(async_stream::stream! {
            let Some(command) = command else {
                yield TurnEvent::Failed { message: "RUSTTRAYCER_GENERIC_CMD unset".into() };
                return;
            };
            if let Some(message) = args_error {
                yield TurnEvent::Failed { message };
                return;
            }
            let mut events = run_cli_turn(command, args, req, timeout);
            while let Some(ev) = events.next().await {
                yield ev;
            }
        })
    }
}

#[derive(Serialize)]
struct CliStdin<'a> {
    messages: &'a [WireMessage],
}

fn run_cli_turn(
    command: String,
    args: Vec<String>,
    req: TurnRequest,
    timeout: Duration,
) -> Pin<Box<dyn Stream<Item = TurnEvent> + Send>> {
    Box::pin(async_stream::stream! {
        if !req.workspace_path.is_dir() {
            yield TurnEvent::Failed {
                message: format!("workspace path is not a directory: {}", req.workspace_path.display()),
            };
            return;
        }
        let payload = match serde_json::to_string(&CliStdin {
            messages: &req.messages,
        }) {
            Ok(s) => s,
            Err(e) => {
                yield TurnEvent::Failed { message: format!("serialize stdin: {e}") };
                return;
            }
        };

        let mut cmd = Command::new(&command);
        cmd.args(&args)
            .current_dir(&req.workspace_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .env("RUSTTRAYCER_AGENT_ID", &req.agent_id)
            .env("RUSTTRAYCER_TASK_ID", &req.task_id);
        for (k, v) in &req.extra_env {
            cmd.env(k, v);
        }
        #[cfg(unix)]
        {
            cmd.process_group(0);
        }

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                yield TurnEvent::Failed { message: format!("spawn {command}: {e}") };
                return;
            }
        };

        if let Some(mut stdin) = child.stdin.take() {
            let body = format!("{payload}\n");
            if let Err(e) = stdin.write_all(body.as_bytes()).await {
                // B1: /bin/echo and /bin/false close stdin; EPIPE is not a turn failure.
                if e.kind() != ErrorKind::BrokenPipe && e.kind() != ErrorKind::ConnectionReset {
                    yield TurnEvent::Failed { message: format!("write stdin: {e}") };
                    let _ = child.start_kill();
                    return;
                }
            }
            drop(stdin);
        }

        let mut stdout = match child.stdout.take() {
            Some(s) => s,
            None => {
                yield TurnEvent::Failed { message: "child stdout missing".into() };
                let _ = child.start_kill();
                return;
            }
        };
        let mut stderr = child.stderr.take();
        let stderr_task = tokio::spawn(async move {
            if let Some(mut err) = stderr.take() {
                let mut buf = [0u8; 4096];
                loop {
                    match err.read(&mut buf).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            let text = String::from_utf8_lossy(&buf[..n]);
                            tracing::warn!(target: "cli.generic", "stderr: {}", text.trim_end());
                        }
                    }
                }
            }
        });

        let mut leftover = Vec::new();
        let mut buf = [0u8; 4096];
        let deadline = tokio::time::Instant::now() + timeout;
        let mut finished = false;

        loop {
            let remain = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remain.is_zero() {
                let _ = child.start_kill();
                yield TurnEvent::Failed { message: "timeout".into() };
                finished = true;
                break;
            }
            match tokio::time::timeout(remain, stdout.read(&mut buf)).await {
                Err(_) => {
                    let _ = child.start_kill();
                    yield TurnEvent::Failed { message: "timeout".into() };
                    finished = true;
                    break;
                }
                Ok(Ok(0)) => break,
                Ok(Err(e)) => {
                    yield TurnEvent::Failed { message: format!("read stdout: {e}") };
                    let _ = child.start_kill();
                    finished = true;
                    break;
                }
                Ok(Ok(n)) => {
                    leftover.extend_from_slice(&buf[..n]);
                    let valid = match std::str::from_utf8(&leftover) {
                        Ok(_) => leftover.len(),
                        Err(e) => e.valid_up_to(),
                    };
                    if valid > 0 {
                        let text = String::from_utf8_lossy(&leftover[..valid]).into_owned();
                        leftover.drain(..valid);
                        if !text.is_empty() {
                            yield TurnEvent::Token { text };
                        }
                    }
                }
            }
        }

        if !leftover.is_empty() && !finished {
            let text = String::from_utf8_lossy(&leftover).into_owned();
            if !text.is_empty() {
                yield TurnEvent::Token { text };
            }
        }

        if !finished {
            match tokio::time::timeout(
                deadline.saturating_duration_since(tokio::time::Instant::now()),
                child.wait(),
            )
            .await
            {
                Ok(Ok(status)) => {
                    let code = status.code().unwrap_or(-1);
                    if status.success() {
                        yield TurnEvent::Finished { exit_code: code };
                    } else {
                        yield TurnEvent::Failed { message: format!("exit {code}") };
                    }
                }
                Ok(Err(e)) => yield TurnEvent::Failed { message: format!("wait: {e}") },
                Err(_) => {
                    let _ = child.start_kill();
                    yield TurnEvent::Failed { message: "timeout".into() };
                }
            }
        }

        let _ = stderr_task.await;
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TurnRequest, WireMessage, WireRole};
    use futures::StreamExt;
    use std::collections::BTreeMap;
    use std::os::unix::fs::PermissionsExt;

    fn echo_req() -> TurnRequest {
        TurnRequest {
            agent_id: "a1".into(),
            task_id: "t1".into(),
            workspace_path: std::env::temp_dir(),
            messages: vec![WireMessage {
                role: WireRole::User,
                content: "hi".into(),
            }],
            extra_env: BTreeMap::new(),
        }
    }

    async fn collect(backend: &CliGeneric) -> Vec<TurnEvent> {
        let mut stream = backend.start_turn(echo_req());
        let mut events = Vec::new();
        while let Some(ev) = stream.next().await {
            events.push(ev);
        }
        events
    }

    fn tokens_of(events: &[TurnEvent]) -> String {
        events
            .iter()
            .filter_map(|e| match e {
                TurnEvent::Token { text } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    fn write_exec(path: &Path, body: &str) {
        use std::io::Write;
        let tmp = path.with_extension("writing");
        {
            let mut f = std::fs::File::create(&tmp).unwrap();
            f.write_all(body.as_bytes()).unwrap();
            f.sync_all().unwrap();
            let mut perms = f.metadata().unwrap().permissions();
            perms.set_mode(0o755);
            f.set_permissions(perms).unwrap();
        }
        std::fs::rename(&tmp, path).unwrap();
    }

    async fn collect_exec(backend: &CliGeneric) -> Vec<TurnEvent> {
        // Overlay/tmp can return ETXTBSY if we exec a just-written script.
        for _ in 0..20 {
            let events = collect(backend).await;
            let busy = events.iter().any(|e| {
                matches!(
                    e,
                    TurnEvent::Failed { message } if message.contains("Text file busy")
                )
            });
            if !busy {
                return events;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        collect(backend).await
    }

    #[tokio::test]
    async fn unset_or_empty_cmd_unavailable_and_start_fails() {
        let empty = CliGeneric::new("");
        assert!(!empty.available().available);
        assert_eq!(empty.available().detail, "RUSTTRAYCER_GENERIC_CMD unset");
        let events = collect(&empty).await;
        assert!(matches!(
            events.as_slice(),
            [TurnEvent::Failed { message }] if message == "RUSTTRAYCER_GENERIC_CMD unset"
        ));

        let unset = CliGeneric {
            command: None,
            args: Vec::new(),
            args_error: None,
            timeout: DEFAULT_TIMEOUT,
        };
        assert!(!unset.available().available);
        assert_eq!(unset.available().detail, "RUSTTRAYCER_GENERIC_CMD unset");
        let events = collect(&unset).await;
        assert!(matches!(events.last(), Some(TurnEvent::Failed { .. })));
    }

    #[tokio::test]
    async fn mock_binary_messages_only_stdin() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("echo_user.py");
        write_exec(
            &path,
            r#"#!/usr/bin/env python3
import json, sys
d = json.load(sys.stdin)
assert "version" not in d
assert "agentId" not in d
assert "taskId" not in d
assert "workspacePath" not in d
msgs = d["messages"]
print(next(m["content"] for m in reversed(msgs) if m["role"] == "user"), end="")
sys.stdout.flush()
"#,
        );
        let backend = CliGeneric::new(path.to_string_lossy().into_owned());
        assert!(
            backend.available().available,
            "{}",
            backend.available().detail
        );
        let events = collect_exec(&backend).await;
        let tokens = tokens_of(&events);
        assert_eq!(tokens, "hi", "events={events:?}");
        assert!(matches!(
            events.last(),
            Some(TurnEvent::Finished { exit_code: 0 })
        ));
    }

    #[tokio::test]
    async fn non_zero_exit_is_failed() {
        let backend = CliGeneric::new("/bin/false");
        let events = collect(&backend).await;
        match events.last() {
            Some(TurnEvent::Failed { message }) => assert_eq!(message, "exit 1"),
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn timeout_fails() {
        let backend = CliGeneric::new("/bin/sleep")
            .with_args(vec!["30".into()])
            .with_timeout(Duration::from_millis(250));
        let events = collect(&backend).await;
        match events.last() {
            Some(TurnEvent::Failed { message }) => assert_eq!(message, "timeout"),
            other => panic!("expected timeout, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn args_are_passed() {
        let backend = CliGeneric::new("/bin/echo").with_args(vec!["hello".into()]);
        assert!(backend.available().available);
        let events = collect(&backend).await;
        let tokens = tokens_of(&events);
        assert!(tokens.contains("hello"), "tokens={tokens:?}");
        assert!(matches!(
            events.last(),
            Some(TurnEvent::Finished { exit_code: 0 })
        ));
    }

    #[tokio::test]
    async fn path_with_space_is_not_split() {
        let dir = tempfile::tempdir().unwrap();
        let spaced = dir.path().join("dir with space");
        std::fs::create_dir(&spaced).unwrap();
        let script = spaced.join("run.sh");
        std::fs::copy("/bin/echo", &script).unwrap();
        let backend = CliGeneric::new(script.to_string_lossy().into_owned())
            .with_args(vec!["space-ok".into()]);
        assert!(
            backend.available().available,
            "available detail={}",
            backend.available().detail
        );
        let events = collect(&backend).await;
        let tokens = tokens_of(&events);
        assert!(
            tokens.contains("space-ok"),
            "tokens={tokens:?} events={events:?}"
        );
        assert!(matches!(
            events.last(),
            Some(TurnEvent::Finished { exit_code: 0 })
        ));
    }

    #[test]
    fn caps_are_cli_generic() {
        let backend = CliGeneric::new("/bin/true");
        assert_eq!(backend.caps(), HarnessCaps::CLI_GENERIC);
        assert_eq!(backend.id(), "cli.generic");
    }

    #[tokio::test]
    async fn invalid_generic_args_fails_start() {
        let mut backend = CliGeneric::new("/bin/true");
        backend.apply_args_json("not-json");
        assert!(
            backend.available().available,
            "CMD still resolves when ARGS is invalid"
        );
        let events = collect(&backend).await;
        match events.as_slice() {
            [TurnEvent::Failed { message }] => {
                assert!(
                    message.contains("RUSTTRAYCER_GENERIC_ARGS")
                        && (message.contains("invalid") || message.contains("JSON")),
                    "message={message}"
                );
            }
            other => panic!("expected single Failed, got {other:?}"),
        }
    }

    struct EnvRestore {
        key: &'static str,
        prev: Option<String>,
    }

    impl EnvRestore {
        fn set(key: &'static str, val: &str) -> Self {
            let prev = std::env::var(key).ok();
            std::env::set_var(key, val);
            Self { key, prev }
        }

        fn unset(key: &'static str) -> Self {
            let prev = std::env::var(key).ok();
            std::env::remove_var(key);
            Self { key, prev }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            match &self.prev {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }

    #[test]
    fn from_env_unset_vs_set_and_default() {
        let _g = crate::TEST_ENV_LOCK.lock().unwrap();
        let _cmd = EnvRestore::unset("RUSTTRAYCER_GENERIC_CMD");
        let _args = EnvRestore::unset("RUSTTRAYCER_GENERIC_ARGS");
        let unset = CliGeneric::from_env();
        assert!(!unset.available().available);
        assert_eq!(unset.available().detail, "RUSTTRAYCER_GENERIC_CMD unset");

        drop(_cmd);
        let _cmd = EnvRestore::set("RUSTTRAYCER_GENERIC_CMD", "/bin/true");
        let set = CliGeneric::from_env();
        assert!(set.available().available, "{}", set.available().detail);
        let defaulted = CliGeneric::default();
        assert!(defaulted.available().available);
        assert_eq!(defaulted.id(), "cli.generic");
    }

    #[test]
    fn from_env_applies_valid_args_json() {
        let _g = crate::TEST_ENV_LOCK.lock().unwrap();
        let _cmd = EnvRestore::set("RUSTTRAYCER_GENERIC_CMD", "/bin/echo");
        let _args = EnvRestore::set("RUSTTRAYCER_GENERIC_ARGS", r#"["from-env"]"#);
        let backend = CliGeneric::from_env();
        assert_eq!(backend.args, vec!["from-env".to_string()]);
        assert!(backend.args_error.is_none());
    }

    #[test]
    fn from_env_empty_args_json_is_ignored() {
        let _g = crate::TEST_ENV_LOCK.lock().unwrap();
        let _cmd = EnvRestore::set("RUSTTRAYCER_GENERIC_CMD", "/bin/true");
        let _args = EnvRestore::set("RUSTTRAYCER_GENERIC_ARGS", "   ");
        let backend = CliGeneric::from_env();
        assert!(backend.args.is_empty());
        assert!(backend.args_error.is_none());
    }

    #[test]
    fn parse_generic_args_edges() {
        assert_eq!(parse_generic_args("").unwrap(), Vec::<String>::new());
        assert_eq!(parse_generic_args("   ").unwrap(), Vec::<String>::new());
        assert_eq!(parse_generic_args("[]").unwrap(), Vec::<String>::new());
        assert_eq!(
            parse_generic_args(r#"["a","b"]"#).unwrap(),
            vec!["a".to_string(), "b".to_string()]
        );
        let not_array = parse_generic_args("123").unwrap_err();
        assert!(not_array.contains("JSON array"), "err={not_array}");
        let not_string = parse_generic_args("[1]").unwrap_err();
        assert!(not_string.contains("not a string"), "err={not_string}");
        let bad_json = parse_generic_args("{").unwrap_err();
        assert!(bad_json.contains("invalid JSON"), "err={bad_json}");
    }

    #[test]
    fn apply_args_json_ok_clears_error() {
        let mut backend = CliGeneric::new("/bin/true");
        backend.apply_args_json("not-json");
        assert!(backend.args_error.is_some());
        backend.apply_args_json(r#"["ok"]"#);
        assert!(backend.args_error.is_none());
        assert_eq!(backend.args, vec!["ok".to_string()]);
    }

    #[test]
    fn available_path_name_not_found_vs_found() {
        let missing = CliGeneric::new("definitely-not-a-rt-generic-bin");
        assert!(!missing.available().available);
        assert!(
            missing.available().detail.contains("not found"),
            "{}",
            missing.available().detail
        );

        let found = CliGeneric::new("true");
        assert!(found.available().available, "{}", found.available().detail);

        let slash_missing = CliGeneric::new("/no/such/rt-generic-cmd");
        assert!(!slash_missing.available().available);
        assert!(slash_missing.available().detail.contains("not found"));
    }

    #[test]
    fn with_timeout_with_args_and_default_builder() {
        let backend = CliGeneric::new("/bin/echo")
            .with_args(vec!["x".into()])
            .with_timeout(Duration::from_secs(3));
        assert_eq!(backend.args, vec!["x".to_string()]);
        assert_eq!(backend.timeout, Duration::from_secs(3));
        assert!(backend.args_error.is_none());
    }

    #[test]
    fn cancel_turn_default_is_ok() {
        let backend = CliGeneric::new("/bin/true");
        assert!(backend.cancel_turn("no-such-agent").is_ok());
        let as_trait: &dyn AgentBackend = &backend;
        assert!(as_trait.cancel_turn("still-missing").is_ok());
    }

    #[tokio::test]
    async fn echo_epipe_is_not_write_stdin_failed() {
        let backend = CliGeneric::new("/bin/echo");
        let events = collect(&backend).await;
        assert!(
            events.iter().all(|e| !matches!(
                e,
                TurnEvent::Failed { message } if message.contains("write stdin")
            )),
            "events={events:?}"
        );
        assert!(matches!(
            events.last(),
            Some(TurnEvent::Finished { exit_code: 0 })
        ));
    }

    #[tokio::test]
    async fn true_epipe_finishes_without_stdin_failure() {
        let backend = CliGeneric::new("/bin/true");
        let events = collect(&backend).await;
        assert!(
            events.iter().all(|e| !matches!(
                e,
                TurnEvent::Failed { message } if message.contains("write stdin")
            )),
            "events={events:?}"
        );
        assert!(matches!(
            events.last(),
            Some(TurnEvent::Finished { exit_code: 0 })
        ));
    }

    #[tokio::test]
    async fn extra_env_is_applied() {
        // Generic applies extra_env after the ID env vars (unlike Claude/Codex).
        // Do not change prod: assert the existing order.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("print_env.py");
        write_exec(
            &path,
            r#"#!/usr/bin/env python3
import os, sys
sys.stdin.read()
print(os.environ.get("MY_EXTRA","") + " " + os.environ.get("RUSTTRAYCER_AGENT_ID",""), end="")
"#,
        );
        let mut extra = BTreeMap::new();
        extra.insert("MY_EXTRA".into(), "yes".into());
        extra.insert("RUSTTRAYCER_AGENT_ID".into(), "hijack".into());
        let mut req = echo_req();
        req.extra_env = extra;
        let backend = CliGeneric::new(path.to_string_lossy().into_owned());
        let mut stream = backend.start_turn(req);
        let mut events = Vec::new();
        while let Some(ev) = stream.next().await {
            events.push(ev);
        }
        let tokens = tokens_of(&events);
        assert!(
            tokens.contains("yes"),
            "tokens={tokens:?} events={events:?}"
        );
        assert!(
            tokens.contains("hijack"),
            "extra_env overwrites IDs in generic; tokens={tokens:?}"
        );
    }

    #[tokio::test]
    async fn workspace_path_not_a_dir_fails() {
        let mut req = echo_req();
        req.workspace_path = std::env::temp_dir().join("rt-generic-not-a-dir-file");
        std::fs::write(&req.workspace_path, b"x").unwrap();
        let backend = CliGeneric::new("/bin/true");
        let mut stream = backend.start_turn(req.clone());
        let mut events = Vec::new();
        while let Some(ev) = stream.next().await {
            events.push(ev);
        }
        let _ = std::fs::remove_file(&req.workspace_path);
        match events.as_slice() {
            [TurnEvent::Failed { message }] => {
                assert!(message.contains("not a directory"), "message={message}");
            }
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn stderr_is_not_tokens() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stderr_and_out.sh");
        write_exec(
            &path,
            "#!/bin/sh
echo out-ok
echo err-only >&2
",
        );
        let backend = CliGeneric::new(path.to_string_lossy().into_owned());
        let events = collect_exec(&backend).await;
        let tokens = tokens_of(&events);
        assert!(
            tokens.contains("out-ok"),
            "tokens={tokens:?} events={events:?}"
        );
        assert!(
            !tokens.contains("err-only"),
            "stderr leaked into tokens={tokens:?}"
        );
        assert!(matches!(
            events.last(),
            Some(TurnEvent::Finished { exit_code: 0 })
        ));
    }

    #[tokio::test]
    async fn spawn_failure_for_non_executable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("not_exec.txt");
        std::fs::write(&path, "not a program").unwrap();
        let backend = CliGeneric::new(path.to_string_lossy().into_owned());
        assert!(
            backend.available().available,
            "regular file is resolvable: {}",
            backend.available().detail
        );
        let events = collect(&backend).await;
        match events.last() {
            Some(TurnEvent::Failed { message }) => {
                assert!(message.contains("spawn"), "message={message}");
            }
            other => panic!("expected spawn Failed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn leftover_invalid_utf8_is_still_token() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad_utf8.py");
        write_exec(
            &path,
            r#"#!/usr/bin/env python3
import sys
sys.stdin.read()
sys.stdout.buffer.write(b"ok" + bytes([0xFF]))
"#,
        );
        let backend = CliGeneric::new(path.to_string_lossy().into_owned());
        let events = collect_exec(&backend).await;
        let tokens = tokens_of(&events);
        assert!(tokens.contains("ok"), "tokens={tokens:?} events={events:?}");
        assert!(matches!(
            events.last(),
            Some(TurnEvent::Finished { exit_code: 0 })
        ));
    }
}
