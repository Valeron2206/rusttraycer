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
    let value: serde_json::Value = serde_json::from_str(trimmed).map_err(|e| {
        format!("RUSTTRAYCER_GENERIC_ARGS invalid JSON: {e}")
    })?;
    let arr = value.as_array().ok_or_else(|| {
        "RUSTTRAYCER_GENERIC_ARGS must be a JSON array of strings".to_string()
    })?;
    let mut out = Vec::with_capacity(arr.len());
    for (i, v) in arr.iter().enumerate() {
        match v.as_str() {
            Some(s) => out.push(s.to_string()),
            None => {
                return Err(format!(
                    "RUSTTRAYCER_GENERIC_ARGS[{i}] is not a string"
                ));
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
            let busy = events.iter().any(|e| matches!(
                e,
                TurnEvent::Failed { message } if message.contains("Text file busy")
            ));
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
        assert!(backend.available().available, "{}", backend.available().detail);
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
        assert!(tokens.contains("space-ok"), "tokens={tokens:?} events={events:?}");
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
}
