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
    AgentBackend, Availability, CancelErr, HarnessCaps, TurnEvent, TurnRequest,
    VendorTranscriptErr, VendorTranscriptRequest, WireMessage, WireRole,
};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const CLAUDE_ARGS: &[&str] = &["-p", "--output-format", "stream-json", "--verbose"];

#[derive(Debug, Clone, Default)]
struct TurnBook {
    inflight: Arc<Mutex<HashMap<String, u32>>>,
    cancelled: Arc<Mutex<HashSet<String>>>,
}

#[derive(Debug, Clone)]
pub struct CliClaude {
    command: Option<String>,
    timeout: Duration,
    book: TurnBook,
}

impl Default for CliClaude {
    fn default() -> Self {
        Self::from_env()
    }
}

impl CliClaude {
    pub fn from_env() -> Self {
        let command = std::env::var("RUSTTRAYCER_CLAUDE_CMD")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| Some("claude".into()));
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

fn parse_stream_json_line(line: &str) -> LineEffect {
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
        "assistant" => {
            let mut tokens = Vec::new();
            if let Some(content) = v.pointer("/message/content").or_else(|| v.get("content")) {
                if let Some(s) = content.as_str() {
                    if !s.is_empty() {
                        tokens.push(s.to_string());
                    }
                } else if let Some(arr) = content.as_array() {
                    for block in arr {
                        let btype = block.get("type").and_then(|t| t.as_str()).unwrap_or("text");
                        if btype == "text" {
                            if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                                if !t.is_empty() {
                                    tokens.push(t.to_string());
                                }
                            }
                        }
                    }
                }
            }
            if tokens.is_empty() {
                LineEffect::Ignore
            } else {
                LineEffect::Tokens(tokens)
            }
        }
        "text" => match v.get("text").and_then(|t| t.as_str()) {
            Some(t) if !t.is_empty() => LineEffect::Tokens(vec![t.to_string()]),
            _ => LineEffect::Ignore,
        },
        "stream_event" => {
            let ev = v.get("event").unwrap_or(&v);
            let ev_type = ev.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if ev_type == "content_block_delta" {
                if let Some(t) = ev.pointer("/delta/text").and_then(|t| t.as_str()) {
                    if !t.is_empty() {
                        return LineEffect::Tokens(vec![t.to_string()]);
                    }
                }
            }
            LineEffect::Ignore
        }
        "result" => {
            let is_err = v.get("is_error").and_then(|b| b.as_bool()).unwrap_or(false)
                || v.get("subtype").and_then(|s| s.as_str()) == Some("error");
            if is_err {
                let msg = v
                    .get("result")
                    .and_then(|r| r.as_str())
                    .or_else(|| v.get("errors").and_then(|r| r.as_str()))
                    .unwrap_or("claude result error");
                LineEffect::ResultError(msg.to_string())
            } else {
                LineEffect::Ignore
            }
        }
        _ => LineEffect::Ignore,
    }
}

fn encode_claude_project_dir(workspace: &Path) -> String {
    let abs = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    abs.to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

fn claude_config_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR") {
        let p = PathBuf::from(dir);
        if !p.as_os_str().is_empty() {
            return p;
        }
    }
    match std::env::var_os("HOME") {
        Some(home) => PathBuf::from(home).join(".claude"),
        None => PathBuf::from(".claude"),
    }
}

fn session_id_ok(id: &str) -> bool {
    !id.is_empty()
        && !id.contains('/')
        && !id.contains('\\')
        && !id.contains("..")
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn extract_history_text(v: &serde_json::Value) -> Option<String> {
    let content = v.pointer("/message/content").or_else(|| v.get("content"))?;
    if let Some(s) = content.as_str() {
        if s.is_empty() {
            return None;
        }
        return Some(s.to_string());
    }
    let arr = content.as_array()?;
    let mut parts = Vec::new();
    for block in arr {
        let btype = block.get("type").and_then(|t| t.as_str()).unwrap_or("text");
        if btype == "text" {
            if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                if !t.is_empty() {
                    parts.push(t.to_string());
                }
            }
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(""))
    }
}

fn parse_vendor_history_line(line: &str) -> Option<WireMessage> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(trimmed).ok()?;
    let typ = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
    let role = match typ {
        "user" | "human" => WireRole::User,
        "assistant" => WireRole::Assistant,
        _ => return None,
    };
    let content = extract_history_text(&v)?;
    Some(WireMessage { role, content })
}

fn vendor_session_path(workspace: &Path, session_id: &str) -> Result<PathBuf, VendorTranscriptErr> {
    let encoded = encode_claude_project_dir(workspace);
    let root = claude_config_dir().join("projects").join(encoded);
    let flat = root.join(format!("{session_id}.jsonl"));
    if flat.is_file() {
        return Ok(flat);
    }
    let nested = root.join("sessions").join(format!("{session_id}.jsonl"));
    if nested.is_file() {
        return Ok(nested);
    }
    Err(VendorTranscriptErr {
        message: "no vendor session".into(),
    })
}

fn read_claude_vendor_transcript(
    req: &VendorTranscriptRequest,
) -> Result<Vec<WireMessage>, VendorTranscriptErr> {
    let sid = req.session_id.trim();
    if sid.is_empty() {
        return Err(VendorTranscriptErr {
            message: "session id is empty".into(),
        });
    }
    if !session_id_ok(sid) {
        return Err(VendorTranscriptErr {
            message: "session id is invalid".into(),
        });
    }
    let path = vendor_session_path(&req.workspace_path, sid)?;
    let raw = std::fs::read_to_string(&path).map_err(|e| VendorTranscriptErr {
        message: format!("vendor session read: {e}"),
    })?;
    let mut messages = Vec::new();
    for line in raw.lines() {
        if let Some(m) = parse_vendor_history_line(line) {
            messages.push(m);
        }
    }
    if messages.is_empty() {
        return Err(VendorTranscriptErr {
            message: "provider silent".into(),
        });
    }
    Ok(messages)
}

impl AgentBackend for CliClaude {
    fn id(&self) -> &'static str {
        "cli.claude"
    }

    fn available(&self) -> Availability {
        match &self.command {
            None => Availability {
                available: false,
                detail: "RUSTTRAYCER_CLAUDE_CMD unset".into(),
            },
            Some(cmd) => match resolve_binary(cmd) {
                Some(p) => Availability {
                    available: true,
                    detail: format!("claude={}", p.display()),
                },
                None => Availability {
                    available: false,
                    detail: format!("{cmd} (not found)"),
                },
            },
        }
    }

    fn caps(&self) -> HarnessCaps {
        HarnessCaps::CLI_CLAUDE
    }

    fn start_turn(&self, req: TurnRequest) -> Pin<Box<dyn Stream<Item = TurnEvent> + Send>> {
        let command = self.command.clone();
        let timeout = self.timeout;
        let book = self.book.clone();
        Box::pin(async_stream::stream! {
            let Some(command) = command else {
                yield TurnEvent::Failed { message: "RUSTTRAYCER_CLAUDE_CMD unset".into() };
                return;
            };
            let mut events = run_claude_turn(command, req, timeout, book);
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

    fn read_vendor_transcript(
        &self,
        req: VendorTranscriptRequest,
    ) -> Result<Vec<WireMessage>, VendorTranscriptErr> {
        read_claude_vendor_transcript(&req)
    }
}

fn run_claude_turn(
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
        cmd.args(CLAUDE_ARGS);
        if let Some(sid) = crate::provider_session_id(&req.extra_env) {
            cmd.args(["--resume", sid]);
        }
        cmd.current_dir(&req.workspace_path)
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
                            tracing::warn!(target: "cli.claude", "stderr: {}", text.trim_end());
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
                        match parse_stream_json_line(&line) {
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
            match parse_stream_json_line(&line) {
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

    async fn collect(backend: &CliClaude, req: TurnRequest) -> Vec<TurnEvent> {
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

    async fn collect_exec(backend: &CliClaude, req: TurnRequest) -> Vec<TurnEvent> {
        for _ in 0..20 {
            let events = collect(backend, req.clone()).await;
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
        collect(backend, req).await
    }

    #[test]
    fn parse_assistant_text_and_ignore_system() {
        match parse_stream_json_line(r#"{"type":"system","subtype":"init"}"#) {
            LineEffect::Ignore => {}
            other => panic!("expected ignore, got system parsed as {other:?}"),
        }
        match parse_stream_json_line(
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hello"}]}}"#,
        ) {
            LineEffect::Tokens(t) => assert_eq!(t, vec!["hello".to_string()]),
            _ => panic!("expected tokens"),
        }
        match parse_stream_json_line(r#"{"type":"result","is_error":true,"result":"boom"}"#) {
            LineEffect::ResultError(m) => assert_eq!(m, "boom"),
            _ => panic!("expected result error"),
        }
        match parse_stream_json_line("not-json") {
            LineEffect::Ignore => {}
            _ => panic!("non-json must be ignored"),
        }
    }

    #[test]
    fn caps_are_cli_claude() {
        let backend = CliClaude::new("/bin/true");
        assert_eq!(backend.caps(), HarnessCaps::CLI_CLAUDE);
        assert!(backend.caps().pty);
        assert!(backend.caps().session_resume);
        assert!(backend.caps().a2a_inbox);
        assert_eq!(backend.id(), "cli.claude");
    }

    #[tokio::test]
    async fn unset_cmd_unavailable_and_start_fails() {
        let empty = CliClaude::new("");
        assert!(!empty.available().available);
        let events = collect(&empty, echo_req()).await;
        assert!(matches!(
            events.as_slice(),
            [TurnEvent::Failed { message }] if message == "RUSTTRAYCER_CLAUDE_CMD unset"
        ));
    }

    #[tokio::test]
    async fn stream_json_stdout_becomes_tokens() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fake_claude.py");
        write_exec(
            &path,
            r#"#!/usr/bin/env python3
import json, sys
assert "-p" in sys.argv
assert "--output-format" in sys.argv
assert "stream-json" in sys.argv
assert "--verbose" in sys.argv
prompt = sys.stdin.read()
print(json.dumps({"type":"system","subtype":"init"}))
print(json.dumps({"type":"assistant","message":{"content":[{"type":"text","text":"pong"}]}}))
print(json.dumps({"type":"result","subtype":"success","is_error":False}))
sys.stderr.write("trace-only\n")
assert "user: hi" in prompt
"#,
        );
        let backend = CliClaude::new(path.to_string_lossy().into_owned());
        assert!(
            backend.available().available,
            "{}",
            backend.available().detail
        );
        let events = collect_exec(&backend, echo_req()).await;
        assert_eq!(tokens_of(&events), "pong", "events={events:?}");
        assert!(events
            .iter()
            .all(|e| !matches!(e, TurnEvent::Token { text } if text.contains("trace"))));
        assert!(matches!(
            events.last(),
            Some(TurnEvent::Finished { exit_code: 0 })
        ));
    }

    #[tokio::test]
    async fn result_error_is_failed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("err_claude.py");
        write_exec(
            &path,
            r#"#!/usr/bin/env python3
import json, sys
sys.stdin.read()
print(json.dumps({"type":"result","is_error":True,"result":"quota"}))
"#,
        );
        let backend = CliClaude::new(path.to_string_lossy().into_owned());
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
        let backend = CliClaude::new(path.to_string_lossy().into_owned())
            .with_timeout(Duration::from_millis(250));
        let events = collect_exec(&backend, echo_req()).await;
        match events.last() {
            Some(TurnEvent::Failed { message }) => assert_eq!(message, "timeout"),
            other => panic!("expected timeout, got {other:?}"),
        }
    }

    #[test]
    fn cancel_without_child_is_ok() {
        let backend = CliClaude::new("/bin/true");
        assert!(backend.cancel_turn("no-such-agent").is_ok());
    }

    #[tokio::test]
    async fn cancel_turn_kills_inflight() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sleep.sh");
        write_exec(&path, "#!/bin/sh\nsleep 30\n");
        let backend = CliClaude::new(path.to_string_lossy().into_owned());
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
print(json.dumps({"type":"assistant","message":{"content":[{"type":"text","text": text}]}}))
"#,
        );
        let mut extra = BTreeMap::new();
        extra.insert("RUSTTRAYCER_AGENT_ID".into(), "hijack".into());
        extra.insert("RUSTTRAYCER_TASK_ID".into(), "hijack".into());
        let mut req = echo_req();
        req.extra_env = extra;
        let backend = CliClaude::new(path.to_string_lossy().into_owned());
        let events = collect_exec(&backend, req).await;
        assert_eq!(tokens_of(&events).trim(), "a1 t1", "events={events:?}");
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
    fn from_env_defaults_to_claude_and_honors_override() {
        let _g = crate::TEST_ENV_LOCK.lock().unwrap();
        let _cmd = EnvRestore::unset("RUSTTRAYCER_CLAUDE_CMD");
        let unset = CliClaude::from_env();
        assert_eq!(unset.command.as_deref(), Some("claude"));
        let defaulted = CliClaude::default();
        assert_eq!(defaulted.command.as_deref(), Some("claude"));

        drop(_cmd);
        let _cmd = EnvRestore::set("RUSTTRAYCER_CLAUDE_CMD", "/bin/true");
        let set = CliClaude::from_env();
        assert!(set.available().available, "{}", set.available().detail);
    }

    #[test]
    fn available_path_name_not_found() {
        let missing = CliClaude::new("definitely-not-a-rt-claude-bin");
        assert!(!missing.available().available);
        assert!(
            missing.available().detail.contains("not found"),
            "{}",
            missing.available().detail
        );
        let slash_missing = CliClaude::new("/no/such/rt-claude-cmd");
        assert!(!slash_missing.available().available);
        let found = CliClaude::new("true");
        assert!(found.available().available, "{}", found.available().detail);
    }

    #[test]
    fn parse_more_line_types() {
        match parse_stream_json_line("") {
            LineEffect::Ignore => {}
            other => panic!("empty must ignore, got {other:?}"),
        }
        match parse_stream_json_line("   ") {
            LineEffect::Ignore => {}
            other => panic!("whitespace must ignore, got {other:?}"),
        }
        match parse_stream_json_line(r#"{"type":"assistant","content":"plain"}"#) {
            LineEffect::Tokens(t) => assert_eq!(t, vec!["plain".to_string()]),
            other => panic!("plain content, got {other:?}"),
        }
        match parse_stream_json_line(r#"{"type":"assistant","message":{"content":""}}"#) {
            LineEffect::Ignore => {}
            other => panic!("empty string content, got {other:?}"),
        }
        match parse_stream_json_line(r#"{"type":"assistant","message":{"content":[]}}"#) {
            LineEffect::Ignore => {}
            other => panic!("empty array, got {other:?}"),
        }
        match parse_stream_json_line(
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use"}]}}"#,
        ) {
            LineEffect::Ignore => {}
            other => panic!("non-text block, got {other:?}"),
        }
        match parse_stream_json_line(r#"{"type":"text","text":"delta"}"#) {
            LineEffect::Tokens(t) => assert_eq!(t, vec!["delta".to_string()]),
            other => panic!("text type, got {other:?}"),
        }
        match parse_stream_json_line(r#"{"type":"text","text":""}"#) {
            LineEffect::Ignore => {}
            other => panic!("empty text, got {other:?}"),
        }
        match parse_stream_json_line(r#"{"type":"text"}"#) {
            LineEffect::Ignore => {}
            other => panic!("text missing, got {other:?}"),
        }
        match parse_stream_json_line(
            r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"text":"x"}}}"#,
        ) {
            LineEffect::Tokens(t) => assert_eq!(t, vec!["x".to_string()]),
            other => panic!("stream_event delta, got {other:?}"),
        }
        match parse_stream_json_line(
            r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"text":""}}}"#,
        ) {
            LineEffect::Ignore => {}
            other => panic!("empty delta, got {other:?}"),
        }
        match parse_stream_json_line(r#"{"type":"stream_event","event":{"type":"other"}}"#) {
            LineEffect::Ignore => {}
            other => panic!("other stream_event, got {other:?}"),
        }
        match parse_stream_json_line(r#"{"type":"result","subtype":"error"}"#) {
            LineEffect::ResultError(m) => assert_eq!(m, "claude result error"),
            other => panic!("subtype error default, got {other:?}"),
        }
        match parse_stream_json_line(r#"{"type":"result","is_error":true,"errors":"nope"}"#) {
            LineEffect::ResultError(m) => assert_eq!(m, "nope"),
            other => panic!("errors field, got {other:?}"),
        }
        match parse_stream_json_line(r#"{"type":"result","is_error":false}"#) {
            LineEffect::Ignore => {}
            other => panic!("success result, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn workspace_path_not_a_dir_fails() {
        let mut req = echo_req();
        req.workspace_path = std::env::temp_dir().join("rt-claude-not-a-dir-file");
        std::fs::write(&req.workspace_path, b"x").unwrap();
        let backend = CliClaude::new("/bin/true");
        let events = collect(&backend, req.clone()).await;
        let _ = std::fs::remove_file(&req.workspace_path);
        match events.as_slice() {
            [TurnEvent::Failed { message }] => {
                assert!(message.contains("not a directory"), "message={message}");
            }
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn flatten_prompt_joins_multiple_roles() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("echo_prompt.py");
        write_exec(
            &path,
            r#"#!/usr/bin/env python3
import json, sys
prompt = sys.stdin.read()
print(json.dumps({"type":"assistant","message":{"content":[{"type":"text","text": prompt}]}}), end="")
"#,
        );
        let req = TurnRequest {
            agent_id: "a1".into(),
            task_id: "t1".into(),
            workspace_path: std::env::temp_dir(),
            messages: vec![
                WireMessage {
                    role: WireRole::User,
                    content: "one".into(),
                },
                WireMessage {
                    role: WireRole::Assistant,
                    content: "two".into(),
                },
                WireMessage {
                    role: WireRole::System,
                    content: "sys".into(),
                },
            ],
            extra_env: BTreeMap::new(),
        };
        let backend = CliClaude::new(path.to_string_lossy().into_owned());
        let events = collect_exec(&backend, req).await;
        let tokens = tokens_of(&events);
        assert!(tokens.contains("user: one"), "tokens={tokens:?}");
        assert!(tokens.contains("assistant: two"), "tokens={tokens:?}");
        assert!(tokens.contains("system: sys"), "tokens={tokens:?}");
    }

    #[tokio::test]
    async fn leftover_line_without_newline_is_parsed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("no_nl.py");
        write_exec(
            &path,
            r#"#!/usr/bin/env python3
import json, sys
sys.stdin.read()
sys.stdout.write(json.dumps({"type":"assistant","message":{"content":[{"type":"text","text":"tail"}]}}))
"#,
        );
        let backend = CliClaude::new(path.to_string_lossy().into_owned());
        let events = collect_exec(&backend, echo_req()).await;
        assert_eq!(tokens_of(&events), "tail", "events={events:?}");
    }

    #[tokio::test]
    async fn spawn_failure_for_non_executable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("not_exec.txt");
        std::fs::write(&path, "not a program").unwrap();
        let backend = CliClaude::new(path.to_string_lossy().into_owned());
        let events = collect(&backend, echo_req()).await;
        match events.last() {
            Some(TurnEvent::Failed { message }) => {
                assert!(message.contains("spawn"), "message={message}");
            }
            other => panic!("expected spawn Failed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn resume_session_id_adds_vendor_argv() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dump_argv.py");
        write_exec(
            &path,
            r#"#!/usr/bin/env python3
import json, sys
sys.stdin.read()
text = " ".join(sys.argv[1:])
print(json.dumps({"type":"assistant","message":{"content":[{"type":"text","text": text}]}}))
"#,
        );
        let mut req = echo_req();
        req.extra_env
            .insert(crate::PROVIDER_SESSION_ENV.into(), "sess-abc".into());
        let backend = CliClaude::new(path.to_string_lossy().into_owned());
        let events = collect_exec(&backend, req).await;
        let tokens = tokens_of(&events);
        assert!(
            tokens.contains("-p")
                && tokens.contains("--output-format")
                && tokens.contains("--resume")
                && tokens.contains("sess-abc"),
            "tokens={tokens:?}"
        );
    }

    #[tokio::test]
    async fn empty_session_id_does_not_add_resume() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dump_argv.py");
        write_exec(
            &path,
            r#"#!/usr/bin/env python3
import json, sys
sys.stdin.read()
text = " ".join(sys.argv[1:])
print(json.dumps({"type":"assistant","message":{"content":[{"type":"text","text": text}]}}))
"#,
        );
        let mut req = echo_req();
        req.extra_env
            .insert(crate::PROVIDER_SESSION_ENV.into(), "   ".into());
        let backend = CliClaude::new(path.to_string_lossy().into_owned());
        let events = collect_exec(&backend, req).await;
        let tokens = tokens_of(&events);
        assert!(tokens.contains("-p"), "tokens={tokens:?}");
        assert!(
            !tokens.contains("--resume"),
            "empty session must not resume, tokens={tokens:?}"
        );
    }

    fn write_vendor_jsonl(
        config: &Path,
        workspace: &Path,
        session_id: &str,
        body: &str,
    ) -> PathBuf {
        let encoded = encode_claude_project_dir(workspace);
        let dir = config.join("projects").join(encoded);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{session_id}.jsonl"));
        std::fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn parse_vendor_history_user_and_assistant() {
        let user = parse_vendor_history_line(
            r#"{"type":"user","message":{"role":"user","content":"hello"}}"#,
        )
        .expect("user");
        assert_eq!(user.role, WireRole::User);
        assert_eq!(user.content, "hello");
        let asst = parse_vendor_history_line(
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hi"}]}}"#,
        )
        .expect("assistant");
        assert_eq!(asst.role, WireRole::Assistant);
        assert_eq!(asst.content, "hi");
        assert!(parse_vendor_history_line(r#"{"type":"system"}"#).is_none());
        assert!(parse_vendor_history_line("not-json").is_none());
    }

    #[test]
    fn vendor_transcript_reads_session_not_scrollback() {
        let _g = crate::TEST_ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        std::fs::create_dir_all(&workspace).unwrap();
        let config = dir.path().join("claude");
        let _cfg = EnvRestore::set("CLAUDE_CONFIG_DIR", &config.to_string_lossy());
        write_vendor_jsonl(
            &config,
            &workspace,
            "sess-1",
            concat!(
                r#"{"type":"user","message":{"content":"from vendor"}}"#,
                "
",
                r#"{"type":"assistant","message":{"content":[{"type":"text","text":"ok"}]}}"#,
                "
",
            ),
        );
        let encoded = encode_claude_project_dir(&workspace);
        std::fs::write(
            config.join("projects").join(encoded).join("scrollback"),
            "PTY BYTES MUST NOT BE READ",
        )
        .unwrap();
        let backend = CliClaude::new("/bin/true");
        let messages = backend
            .read_vendor_transcript(VendorTranscriptRequest {
                session_id: "sess-1".into(),
                workspace_path: workspace,
            })
            .expect("vendor history");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content, "from vendor");
        assert_eq!(messages[1].content, "ok");
        assert!(messages.iter().all(|m| !m.content.contains("PTY")));
    }

    #[test]
    fn vendor_transcript_missing_session_is_error() {
        let _g = crate::TEST_ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        std::fs::create_dir_all(&workspace).unwrap();
        let config = dir.path().join("claude");
        std::fs::create_dir_all(&config).unwrap();
        let _cfg = EnvRestore::set("CLAUDE_CONFIG_DIR", &config.to_string_lossy());
        let backend = CliClaude::new("/bin/true");
        let err = backend
            .read_vendor_transcript(VendorTranscriptRequest {
                session_id: "missing".into(),
                workspace_path: workspace,
            })
            .expect_err("missing");
        assert!(err.message.contains("no vendor session"), "{}", err.message);
    }

    #[test]
    fn vendor_transcript_silent_provider_is_error() {
        let _g = crate::TEST_ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        std::fs::create_dir_all(&workspace).unwrap();
        let config = dir.path().join("claude");
        let _cfg = EnvRestore::set("CLAUDE_CONFIG_DIR", &config.to_string_lossy());
        write_vendor_jsonl(&config, &workspace, "quiet", r#"{"type":"system"}"#);
        let backend = CliClaude::new("/bin/true");
        let err = backend
            .read_vendor_transcript(VendorTranscriptRequest {
                session_id: "quiet".into(),
                workspace_path: workspace,
            })
            .expect_err("silent");
        assert!(err.message.contains("provider silent"), "{}", err.message);
    }

    #[test]
    fn vendor_transcript_rejects_empty_and_path_session_id() {
        let backend = CliClaude::new("/bin/true");
        let ws = std::env::temp_dir();
        let empty = backend
            .read_vendor_transcript(VendorTranscriptRequest {
                session_id: "  ".into(),
                workspace_path: ws.clone(),
            })
            .expect_err("empty");
        assert!(empty.message.contains("empty"), "{}", empty.message);
        let slash = backend
            .read_vendor_transcript(VendorTranscriptRequest {
                session_id: "../etc".into(),
                workspace_path: ws,
            })
            .expect_err("slash");
        assert!(slash.message.contains("invalid"), "{}", slash.message);
    }
}
