use std::collections::{HashMap, HashSet};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::{Stream, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

use crate::{
    AgentBackend, Availability, CancelErr, HarnessCaps, TurnEvent, TurnRequest, WireMessage,
};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// Native one-shot: `codex exec --json -` (prompt on stdin).
const CODEX_ARGS: &[&str] = &["exec", "--json", "-"];

#[derive(Debug, Clone, Default)]
struct TurnBook {
    inflight: Arc<Mutex<HashMap<String, u32>>>,
    cancelled: Arc<Mutex<HashSet<String>>>,
}

#[derive(Debug, Clone)]
pub struct CliCodex {
    command: Option<String>,
    timeout: Duration,
    book: TurnBook,
}

impl Default for CliCodex {
    fn default() -> Self {
        Self::from_env()
    }
}

impl CliCodex {
    pub fn from_env() -> Self {
        let command = std::env::var("RUSTTRAYCER_CODEX_CMD")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| Some("codex".into()));
        Self {
            command,
            timeout: DEFAULT_TIMEOUT,
            book: TurnBook::default(),
        }
    }

    pub fn new(path: impl Into<String>) -> Self {
        let path = path.into();
        Self {
            command: if path.trim().is_empty() {
                None
            } else {
                Some(path)
            },
            timeout: DEFAULT_TIMEOUT,
            book: TurnBook::default(),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
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

fn kill_pid_group(pid: u32) {
    #[cfg(unix)]
    {
        let _ = unsafe { libc::kill(-(pid as i32), libc::SIGKILL) };
        let _ = unsafe { libc::kill(pid as i32, libc::SIGKILL) };
    }
}

fn kill_process_group(child: &mut tokio::process::Child) {
    if let Some(pid) = child.id() {
        kill_pid_group(pid);
    }
    let _ = child.start_kill();
}

fn flatten_prompt(messages: &[WireMessage]) -> String {
    let mut out = String::new();
    for m in messages {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(m.role.as_str());
        out.push_str(": ");
        out.push_str(&m.content);
    }
    out
}

#[derive(Debug)]
enum LineEffect {
    Tokens(Vec<String>),
    ResultError(String),
    Ignore,
}

fn parse_exec_json_line(line: &str) -> LineEffect {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return LineEffect::Ignore;
    }
    let v: serde_json::Value = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(_) => return LineEffect::Ignore,
    };
    let typ = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
    match typ {
        "item.completed" => {
            let item = match v.get("item") {
                Some(i) => i,
                None => return LineEffect::Ignore,
            };
            let item_type = item.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if item_type == "agent_message" {
                if let Some(t) = item.get("text").and_then(|t| t.as_str()) {
                    if !t.is_empty() {
                        return LineEffect::Tokens(vec![t.to_string()]);
                    }
                }
            }
            LineEffect::Ignore
        }
        "turn.failed" => {
            let msg = v
                .pointer("/error/message")
                .and_then(|m| m.as_str())
                .unwrap_or("codex turn failed");
            LineEffect::ResultError(msg.to_string())
        }
        "error" => {
            let msg = v
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("codex error");
            if msg.starts_with("Reconnecting") {
                LineEffect::Ignore
            } else {
                LineEffect::ResultError(msg.to_string())
            }
        }
        _ => LineEffect::Ignore,
    }
}

impl AgentBackend for CliCodex {
    fn id(&self) -> &'static str {
        "cli.codex"
    }

    fn available(&self) -> Availability {
        match &self.command {
            None => Availability {
                available: false,
                detail: "RUSTTRAYCER_CODEX_CMD unset".into(),
            },
            Some(cmd) => match resolve_binary(cmd) {
                Some(p) => Availability {
                    available: true,
                    detail: format!("codex={}", p.display()),
                },
                None => Availability {
                    available: false,
                    detail: format!("{cmd} (not found)"),
                },
            },
        }
    }

    fn caps(&self) -> HarnessCaps {
        HarnessCaps::CLI_CODEX
    }

    fn start_turn(&self, req: TurnRequest) -> Pin<Box<dyn Stream<Item = TurnEvent> + Send>> {
        let command = self.command.clone();
        let timeout = self.timeout;
        let book = self.book.clone();
        Box::pin(async_stream::stream! {
            let Some(command) = command else {
                yield TurnEvent::Failed { message: "RUSTTRAYCER_CODEX_CMD unset".into() };
                return;
            };
            let mut events = run_codex_turn(command, req, timeout, book);
            while let Some(ev) = events.next().await {
                yield ev;
            }
        })
    }

    fn cancel_turn(&self, agent_id: &str) -> Result<(), CancelErr> {
        let pid = self
            .book
            .inflight
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(agent_id);
        if let Some(pid) = pid {
            self.book
                .cancelled
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(agent_id.to_string());
            kill_pid_group(pid);
        }
        Ok(())
    }
}

fn run_codex_turn(
    command: String,
    req: TurnRequest,
    timeout: Duration,
    book: TurnBook,
) -> Pin<Box<dyn Stream<Item = TurnEvent> + Send>> {
    Box::pin(async_stream::stream! {
        if !req.workspace_path.is_dir() {
            yield TurnEvent::Failed {
                message: format!("workspace path is not a directory: {}", req.workspace_path.display()),
            };
            return;
        }
        let prompt = flatten_prompt(&req.messages);

        let mut cmd = Command::new(&command);
        cmd.args(CODEX_ARGS)
            .current_dir(&req.workspace_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        for (k, v) in &req.extra_env {
            cmd.env(k, v);
        }
        cmd.env("RUSTTRAYCER_AGENT_ID", &req.agent_id)
            .env("RUSTTRAYCER_TASK_ID", &req.task_id);
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
        let agent_id = req.agent_id.clone();
        if let Some(pid) = child.id() {
            book.inflight
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(agent_id.clone(), pid);
        }

        let stdin = child.stdin.take();
        let stdin_task = tokio::spawn(async move {
            if let Some(mut stdin) = stdin {
                stdin.write_all(prompt.as_bytes()).await?;
                stdin.shutdown().await?;
            }
            Ok::<(), std::io::Error>(())
        });

        let mut stdout = match child.stdout.take() {
            Some(s) => s,
            None => {
                yield TurnEvent::Failed { message: "child stdout missing".into() };
                kill_process_group(&mut child);
                stdin_task.abort();
                book.inflight
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&agent_id);
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
                            tracing::warn!(target: "cli.codex", "stderr: {}", text.trim_end());
                        }
                    }
                }
            }
        });

        let mut leftover = Vec::new();
        let mut buf = [0u8; 4096];
        let deadline = tokio::time::Instant::now() + timeout;
        let mut finished = false;
        let mut had_terminal = false;
        let mut result_error: Option<String> = None;

        loop {
            let remain = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remain.is_zero() {
                kill_process_group(&mut child);
                yield TurnEvent::Failed { message: "timeout".into() };
                finished = true;
                had_terminal = true;
                break;
            }
            match tokio::time::timeout(remain, stdout.read(&mut buf)).await {
                Err(_) => {
                    kill_process_group(&mut child);
                    yield TurnEvent::Failed { message: "timeout".into() };
                    finished = true;
                    had_terminal = true;
                    break;
                }
                Ok(Ok(0)) => break,
                Ok(Err(e)) => {
                    yield TurnEvent::Failed { message: format!("read stdout: {e}") };
                    kill_process_group(&mut child);
                    finished = true;
                    had_terminal = true;
                    break;
                }
                Ok(Ok(n)) => {
                    leftover.extend_from_slice(&buf[..n]);
                    while let Some(pos) = leftover.iter().position(|&b| b == b'\n') {
                        let line = leftover.drain(..=pos).collect::<Vec<_>>();
                        let line = String::from_utf8_lossy(&line);
                        match parse_exec_json_line(&line) {
                            LineEffect::Tokens(tokens) => {
                                for text in tokens {
                                    yield TurnEvent::Token { text };
                                }
                            }
                            LineEffect::ResultError(msg) => {
                                result_error = Some(msg);
                            }
                            LineEffect::Ignore => {}
                        }
                    }
                }
            }
        }

        if !leftover.is_empty() && !finished {
            let line = String::from_utf8_lossy(&leftover);
            match parse_exec_json_line(&line) {
                LineEffect::Tokens(tokens) => {
                    for text in tokens {
                        yield TurnEvent::Token { text };
                    }
                }
                LineEffect::ResultError(msg) => {
                    result_error = Some(msg);
                }
                LineEffect::Ignore => {}
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
                    let cancelled = book
                        .cancelled
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .remove(&agent_id);
                    if cancelled {
                        yield TurnEvent::Failed { message: "cancelled".into() };
                    } else if let Some(msg) = result_error {
                        yield TurnEvent::Failed { message: msg };
                    } else {
                        let code = status.code().unwrap_or(-1);
                        if status.success() {
                            yield TurnEvent::Finished { exit_code: code };
                        } else {
                            yield TurnEvent::Failed { message: format!("exit {code}") };
                        }
                    }
                    had_terminal = true;
                }
                Ok(Err(e)) => {
                    yield TurnEvent::Failed { message: format!("wait: {e}") };
                    had_terminal = true;
                }
                Err(_) => {
                    kill_process_group(&mut child);
                    yield TurnEvent::Failed { message: "timeout".into() };
                    had_terminal = true;
                }
            }
        }

        match stdin_task.await {
            Ok(Err(e))
                if !had_terminal
                    && e.kind() != ErrorKind::BrokenPipe
                    && e.kind() != ErrorKind::ConnectionReset =>
            {
                yield TurnEvent::Failed { message: format!("write stdin: {e}") };
            }
            _ => {}
        }

        book.inflight
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&agent_id);
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

    async fn collect(backend: &CliCodex, req: TurnRequest) -> Vec<TurnEvent> {
        let mut stream = backend.start_turn(req);
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

    async fn collect_exec(backend: &CliCodex, req: TurnRequest) -> Vec<TurnEvent> {
        for _ in 0..20 {
            let events = collect(backend, req.clone()).await;
            let busy = events.iter().any(|e| matches!(
                e,
                TurnEvent::Failed { message } if message.contains("Text file busy")
            ));
            if !busy {
                return events;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        collect(backend, req).await
    }

    #[test]
    fn parse_agent_message_and_ignore_lifecycle() {
        match parse_exec_json_line(r#"{"type":"thread.started","thread_id":"t"}"#) {
            LineEffect::Ignore => {}
            other => panic!("expected ignore, got {other:?}"),
        }
        match parse_exec_json_line(
            r#"{"type":"item.completed","item":{"id":"item_3","type":"agent_message","text":"hello"}}"#,
        ) {
            LineEffect::Tokens(t) => assert_eq!(t, vec!["hello".to_string()]),
            _ => panic!("expected tokens"),
        }
        match parse_exec_json_line(
            r#"{"type":"item.completed","item":{"id":"item_1","type":"command_execution","command":"ls"}}"#,
        ) {
            LineEffect::Ignore => {}
            other => panic!("command_execution must not be Token, got {other:?}"),
        }
        match parse_exec_json_line(
            r#"{"type":"turn.failed","error":{"message":"boom"}}"#,
        ) {
            LineEffect::ResultError(m) => assert_eq!(m, "boom"),
            _ => panic!("expected turn.failed"),
        }
        match parse_exec_json_line(r#"{"type":"error","message":"Reconnecting... 1/5"}"#) {
            LineEffect::Ignore => {}
            other => panic!("reconnect must be ignored, got {other:?}"),
        }
        match parse_exec_json_line("not-json") {
            LineEffect::Ignore => {}
            _ => panic!("non-json must be ignored"),
        }
    }

    #[test]
    fn caps_are_cli_codex() {
        let backend = CliCodex::new("/bin/true");
        assert_eq!(backend.caps(), HarnessCaps::CLI_CODEX);
        assert_eq!(backend.id(), "cli.codex");
    }

    #[tokio::test]
    async fn unset_cmd_unavailable_and_start_fails() {
        let empty = CliCodex::new("");
        assert!(!empty.available().available);
        let events = collect(&empty, echo_req()).await;
        assert!(matches!(
            events.as_slice(),
            [TurnEvent::Failed { message }] if message == "RUSTTRAYCER_CODEX_CMD unset"
        ));
    }

    #[tokio::test]
    async fn exec_json_stdout_becomes_tokens() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fake_codex.py");
        write_exec(
            &path,
            r#"#!/usr/bin/env python3
import json, sys
assert "exec" in sys.argv
assert "--json" in sys.argv
assert "-" in sys.argv
prompt = sys.stdin.read()
print(json.dumps({"type":"thread.started","thread_id":"t"}))
print(json.dumps({"type":"turn.started"}))
print(json.dumps({"type":"item.completed","item":{"id":"item_3","type":"agent_message","text":"pong"}}))
print(json.dumps({"type":"turn.completed","usage":{"input_tokens":1}}))
sys.stderr.write("trace-only\n")
assert "user: hi" in prompt
"#,
        );
        let backend = CliCodex::new(path.to_string_lossy().into_owned());
        assert!(backend.available().available, "{}", backend.available().detail);
        let events = collect_exec(&backend, echo_req()).await;
        assert_eq!(tokens_of(&events), "pong", "events={events:?}");
        assert!(events.iter().all(|e| !matches!(e, TurnEvent::Token { text } if text.contains("trace"))));
        assert!(matches!(
            events.last(),
            Some(TurnEvent::Finished { exit_code: 0 })
        ));
    }

    #[tokio::test]
    async fn turn_failed_is_failed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("err_codex.py");
        write_exec(
            &path,
            r#"#!/usr/bin/env python3
import json, sys
sys.stdin.read()
print(json.dumps({"type":"turn.failed","error":{"message":"quota"}}))
"#,
        );
        let backend = CliCodex::new(path.to_string_lossy().into_owned());
        let events = collect_exec(&backend, echo_req()).await;
        match events.last() {
            Some(TurnEvent::Failed { message }) => assert_eq!(message, "quota"),
            other => panic!("expected Failed quota, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn timeout_fails() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sleep.sh");
        write_exec(&path, "#!/bin/sh\nsleep 30\n");
        let backend = CliCodex::new(path.to_string_lossy().into_owned())
            .with_timeout(Duration::from_millis(250));
        let events = collect_exec(&backend, echo_req()).await;
        match events.last() {
            Some(TurnEvent::Failed { message }) => assert_eq!(message, "timeout"),
            other => panic!("expected timeout, got {other:?}"),
        }
    }

    #[test]
    fn cancel_without_child_is_ok() {
        let backend = CliCodex::new("/bin/true");
        assert!(backend.cancel_turn("no-such-agent").is_ok());
    }

    #[tokio::test]
    async fn cancel_turn_kills_inflight() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sleep.sh");
        write_exec(&path, "#!/bin/sh\nsleep 30\n");
        let backend = CliCodex::new(path.to_string_lossy().into_owned());
        let mut stream = backend.start_turn(echo_req());
        let collect = tokio::spawn(async move {
            let mut last = None;
            while let Some(ev) = stream.next().await {
                last = Some(ev);
            }
            last
        });
        tokio::time::sleep(Duration::from_millis(120)).await;
        assert!(backend.cancel_turn("a1").is_ok());
        let last = tokio::time::timeout(Duration::from_secs(2), collect)
            .await
            .expect("collect timed out")
            .expect("task");
        match last {
            Some(TurnEvent::Failed { message }) => assert_eq!(message, "cancelled"),
            other => panic!("expected cancelled, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn ids_win_over_extra_env() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("print_ids.py");
        write_exec(
            &path,
            r#"#!/usr/bin/env python3
import json, os, sys
sys.stdin.read()
text = os.environ.get("RUSTTRAYCER_AGENT_ID","") + " " + os.environ.get("RUSTTRAYCER_TASK_ID","")
print(json.dumps({"type":"item.completed","item":{"id":"item_3","type":"agent_message","text": text}}))
"#,
        );
        let mut extra = BTreeMap::new();
        extra.insert("RUSTTRAYCER_AGENT_ID".into(), "hijack".into());
        extra.insert("RUSTTRAYCER_TASK_ID".into(), "hijack".into());
        let mut req = echo_req();
        req.extra_env = extra;
        let backend = CliCodex::new(path.to_string_lossy().into_owned());
        let events = collect_exec(&backend, req).await;
        assert_eq!(tokens_of(&events).trim(), "a1 t1", "events={events:?}");
    }
}
