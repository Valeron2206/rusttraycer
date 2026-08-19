//! Sync WebSocket client. Background thread only; UI drains an mpsc each frame.

use std::net::TcpStream;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::Duration;

use serde::Deserialize;
use tungstenite::http::Request;
use tungstenite::{Message, WebSocket};

use crate::state::ChatMessage;

const READ_TIMEOUT: Duration = Duration::from_millis(200);
const RECONNECT_WAIT: Duration = Duration::from_millis(400);

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "event")]
pub enum WsEvent {
    #[serde(rename = "agent.message")]
    AgentMessage {
        #[serde(rename = "taskId")]
        task_id: String,
        #[serde(rename = "agentId")]
        agent_id: String,
        message: rt_protocol::Message,
    },
    #[serde(rename = "agent.status")]
    AgentStatus {
        #[serde(rename = "taskId")]
        task_id: String,
        #[serde(rename = "agentId")]
        agent_id: String,
        status: String,
    },
    #[serde(rename = "task.updated")]
    TaskUpdated {
        #[serde(rename = "taskId")]
        task_id: String,
    },
    #[serde(rename = "host.going_away")]
    HostGoingAway {
        #[serde(rename = "hostId")]
        host_id: String,
    },
    #[serde(rename = "agent.approval")]
    AgentApproval {
        #[serde(rename = "approvalId")]
        approval_id: String,
        #[serde(rename = "agentId")]
        agent_id: String,
        #[serde(rename = "taskId")]
        task_id: String,
        kind: String,
        summary: String,
    },
    #[serde(rename = "pty.data")]
    PtyData {
        #[serde(rename = "ptyId")]
        pty_id: String,
        data: String,
    },
    #[serde(rename = "pty.exit")]
    PtyExit {
        #[serde(rename = "ptyId")]
        pty_id: String,
        #[serde(default)]
        code: i32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyOutcome {
    Deduped,
    Appended,
    StatusChanged(String),
    GoingAway,
    TaskUpdated,
    Approval,
    PtyData,
    PtyExit,
    Ignored,
}

/// Composer is enabled only when the host is online and the agent is idle or error.
pub fn composer_allowed(online: bool, status: Option<&str>) -> bool {
    online && matches!(status, Some("idle" | "error"))
}

pub fn parse_event(json: &str) -> Result<WsEvent, String> {
    match serde_json::from_str(json) {
        Ok(event) => Ok(event),
        Err(err) => {
            let mut value: serde_json::Value =
                serde_json::from_str(json).map_err(|e| e.to_string())?;
            if value.get("event").is_none() {
                if let Some(kind) = value.get("type").cloned() {
                    value["event"] = kind;
                    return serde_json::from_value(value).map_err(|e| e.to_string());
                }
            }
            Err(err.to_string())
        }
    }
}

/// Apply a parsed WS event to a transcript. Dedup by `message.id`. No merge.
pub fn apply_event(
    messages: &mut Vec<ChatMessage>,
    event: &WsEvent,
    task_id_filter: Option<&str>,
    agent_id_filter: Option<&str>,
) -> ApplyOutcome {
    match event {
        WsEvent::AgentMessage {
            task_id,
            agent_id,
            message,
        } => {
            if let Some(want) = task_id_filter {
                if want != task_id {
                    return ApplyOutcome::Ignored;
                }
            }
            if let Some(want) = agent_id_filter {
                if want != agent_id {
                    return ApplyOutcome::Ignored;
                }
            }
            if messages.iter().any(|m| m.id == message.id) {
                return ApplyOutcome::Deduped;
            }
            messages.push(ChatMessage::from(message.clone()));
            ApplyOutcome::Appended
        }
        WsEvent::AgentStatus {
            task_id,
            agent_id,
            status,
        } => {
            if let Some(want) = task_id_filter {
                if want != task_id {
                    return ApplyOutcome::Ignored;
                }
            }
            if let Some(want) = agent_id_filter {
                if want != agent_id {
                    return ApplyOutcome::Ignored;
                }
            }
            ApplyOutcome::StatusChanged(status.clone())
        }
        WsEvent::HostGoingAway { host_id } => {
            if host_id.is_empty() {
                ApplyOutcome::Ignored
            } else {
                ApplyOutcome::GoingAway
            }
        }
        WsEvent::TaskUpdated { task_id } => {
            if task_id.is_empty() {
                ApplyOutcome::Ignored
            } else {
                ApplyOutcome::TaskUpdated
            }
        }
        WsEvent::AgentApproval {
            task_id,
            agent_id,
            approval_id,
            ..
        } => {
            if approval_id.is_empty() {
                return ApplyOutcome::Ignored;
            }
            if let Some(want) = task_id_filter {
                if want != task_id {
                    return ApplyOutcome::Ignored;
                }
            }
            if let Some(want) = agent_id_filter {
                if want != agent_id {
                    return ApplyOutcome::Ignored;
                }
            }
            ApplyOutcome::Approval
        }
        WsEvent::PtyData { .. } => ApplyOutcome::PtyData,
        WsEvent::PtyExit { .. } => ApplyOutcome::PtyExit,
    }
}

#[derive(Debug)]
pub enum WsCmd {
    Subscribe(String),
    Shutdown,
}

#[derive(Debug)]
pub enum WsIncoming {
    Event(WsEvent),
    Disconnected { reason: String },
    Reconnected,
}

pub struct WsBridge {
    cmd_tx: Sender<WsCmd>,
    event_rx: Receiver<WsIncoming>,
}

impl WsBridge {
    pub fn start(ws_url: String, token: String) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let _ = thread::Builder::new()
            .name("rt-gui-ws".into())
            .spawn(move || ws_loop(ws_url, token, cmd_rx, event_tx));
        Self { cmd_tx, event_rx }
    }

    pub fn subscribe(&self, task_id: String) {
        let _ = self.cmd_tx.send(WsCmd::Subscribe(task_id));
    }

    pub fn try_recv(&self) -> Option<WsIncoming> {
        match self.event_rx.try_recv() {
            Ok(ev) => Some(ev),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => None,
        }
    }
}

impl Drop for WsBridge {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(WsCmd::Shutdown);
    }
}

fn parse_ws_url(ws_url: &str) -> Result<(String, u16, String), String> {
    let url = ws_url.trim();
    let rest = url
        .strip_prefix("ws://")
        .ok_or_else(|| format!("unsupported ws url: {url}"))?;
    let (addr, path) = match rest.split_once('/') {
        Some((addr, path)) => (addr, format!("/{path}")),
        None => (rest, "/ws".to_string()),
    };
    let (host, port) = match addr.rsplit_once(':') {
        Some((host, port)) => {
            let port: u16 = port.parse().map_err(|_| format!("bad ws port: {port}"))?;
            (host.to_string(), port)
        }
        None => (addr.to_string(), 80u16),
    };
    if host.is_empty() {
        return Err("empty ws host".into());
    }
    Ok((host, port, path))
}

/// Open a WS connection with `X-Rt-Session`. Used by the GUI thread helper and tests.
pub fn connect_ws(ws_url: &str, token: &str) -> Result<WebSocket<TcpStream>, String> {
    let (host, port, path) = parse_ws_url(ws_url)?;
    let stream = TcpStream::connect((host.as_str(), port)).map_err(|e| e.to_string())?;
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    let uri = format!("ws://{host}:{port}{path}");
    let req = Request::builder()
        .uri(&uri)
        .header("Host", format!("{host}:{port}"))
        .header("Upgrade", "websocket")
        .header("Connection", "Upgrade")
        .header("Sec-WebSocket-Version", "13")
        .header(
            "Sec-WebSocket-Key",
            tungstenite::handshake::client::generate_key(),
        )
        .header(rt_protocol::SESSION_HEADER, token)
        .body(())
        .map_err(|e| e.to_string())?;
    let (ws, _resp) = tungstenite::client::client(req, stream).map_err(|e| e.to_string())?;
    Ok(ws)
}

fn send_subscribe(socket: &mut WebSocket<TcpStream>, task_id: &str) -> Result<(), String> {
    let body = serde_json::json!({ "type": "subscribe", "taskId": task_id }).to_string();
    socket.send(Message::text(body)).map_err(|e| e.to_string())
}

fn ws_loop(ws_url: String, token: String, cmd_rx: Receiver<WsCmd>, event_tx: Sender<WsIncoming>) {
    let mut task_id: Option<String> = None;
    let mut ever_connected = false;
    loop {
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                WsCmd::Shutdown => return,
                WsCmd::Subscribe(id) => task_id = Some(id),
            }
        }

        let mut socket = match connect_ws(&ws_url, &token) {
            Ok(s) => s,
            Err(reason) => {
                let _ = event_tx.send(WsIncoming::Disconnected { reason });
                match cmd_rx.recv_timeout(RECONNECT_WAIT) {
                    Ok(WsCmd::Shutdown) => return,
                    Ok(WsCmd::Subscribe(id)) => task_id = Some(id),
                    Err(_) => {}
                }
                continue;
            }
        };

        if ever_connected && event_tx.send(WsIncoming::Reconnected).is_err() {
            return;
        }
        ever_connected = true;

        if let Some(id) = task_id.as_deref() {
            if send_subscribe(&mut socket, id).is_err() {
                let _ = event_tx.send(WsIncoming::Disconnected {
                    reason: "subscribe failed".into(),
                });
                continue;
            }
        }

        let mut running = true;
        while running {
            match cmd_rx.try_recv() {
                Ok(WsCmd::Shutdown) => {
                    let _ = socket.close(None);
                    return;
                }
                Ok(WsCmd::Subscribe(id)) => {
                    if send_subscribe(&mut socket, &id).is_err() {
                        running = false;
                    }
                    task_id = Some(id);
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => return,
            }
            if !running {
                break;
            }

            match socket.read() {
                Ok(Message::Text(text)) => {
                    let text = text.to_string();
                    if let Ok(event) = parse_event(&text) {
                        if event_tx.send(WsIncoming::Event(event)).is_err() {
                            return;
                        }
                    }
                }
                Ok(Message::Ping(p)) => {
                    let _ = socket.send(Message::Pong(p));
                }
                Ok(Message::Close(_)) => {
                    running = false;
                }
                Ok(_) => {}
                Err(tungstenite::Error::Io(err))
                    if err.kind() == std::io::ErrorKind::WouldBlock
                        || err.kind() == std::io::ErrorKind::TimedOut => {}
                Err(_) => {
                    running = false;
                }
            }
        }

        let _ = event_tx.send(WsIncoming::Disconnected {
            reason: "socket closed".into(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user_event(id: &str) -> String {
        serde_json::json!({
            "event": "agent.message",
            "taskId": "task-1",
            "agentId": "ag-1",
            "message": {
                "id": id,
                "agentId": "ag-1",
                "role": "user",
                "content": "hello",
                "createdAt": "2026-08-17T12:00:00Z"
            }
        })
        .to_string()
    }

    fn assistant_event(id: &str, content: &str) -> String {
        serde_json::json!({
            "event": "agent.message",
            "taskId": "task-1",
            "agentId": "ag-1",
            "message": {
                "id": id,
                "agentId": "ag-1",
                "role": "assistant",
                "content": content,
                "createdAt": "2026-08-17T12:00:01Z"
            }
        })
        .to_string()
    }

    fn status_event(status: &str) -> String {
        serde_json::json!({
            "event": "agent.status",
            "taskId": "task-1",
            "agentId": "ag-1",
            "status": status
        })
        .to_string()
    }

    #[test]
    fn apply_event_dedups_user_and_appends_assistant() {
        let mut messages = Vec::new();
        let user = parse_event(&user_event("msg-user")).unwrap();
        assert_eq!(
            apply_event(&mut messages, &user, Some("task-1"), Some("ag-1")),
            ApplyOutcome::Appended
        );
        assert_eq!(messages.len(), 1);
        assert_eq!(
            apply_event(&mut messages, &user, Some("task-1"), Some("ag-1")),
            ApplyOutcome::Deduped
        );
        assert_eq!(messages.len(), 1);

        let assistant = parse_event(&assistant_event("msg-as-1", "chunk")).unwrap();
        assert_eq!(
            apply_event(&mut messages, &assistant, Some("task-1"), Some("ag-1")),
            ApplyOutcome::Appended
        );
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].content, "chunk");
    }

    #[test]
    fn apply_status_running_disables_composer() {
        let mut messages = Vec::new();
        let running = parse_event(&status_event("running")).unwrap();
        match apply_event(&mut messages, &running, Some("task-1"), Some("ag-1")) {
            ApplyOutcome::StatusChanged(status) => {
                assert_eq!(status, "running");
                assert!(!composer_allowed(true, Some(status.as_str())));
            }
            other => panic!("expected status, got {other:?}"),
        }
        assert!(composer_allowed(true, Some("idle")));
        assert!(composer_allowed(true, Some("error")));
        assert!(!composer_allowed(false, Some("idle")));
        assert!(!composer_allowed(true, None));
    }

    #[test]
    fn apply_going_away_and_ignore_other_task() {
        let mut messages = Vec::new();
        let ev = parse_event(&user_event("msg-x")).unwrap();
        assert_eq!(
            apply_event(&mut messages, &ev, Some("other-task"), None),
            ApplyOutcome::Ignored
        );
        assert!(messages.is_empty());

        let away = parse_event(r#"{"event":"host.going_away","hostId":"host-a"}"#).unwrap();
        assert_eq!(
            apply_event(&mut messages, &away, None, None),
            ApplyOutcome::GoingAway
        );
    }

    #[test]
    fn parse_approval_event_type_or_event_field() {
        let via_event = parse_event(
            r#"{"event":"agent.approval","approvalId":"ap-1","agentId":"ag-1","taskId":"task-1","kind":"exec","summary":"spawn cli.claude"}"#,
        )
        .unwrap();
        let via_type = parse_event(
            r#"{"type":"agent.approval","approvalId":"ap-1","agentId":"ag-1","taskId":"task-1","kind":"exec","summary":"spawn cli.claude"}"#,
        )
        .unwrap();
        for ev in [via_event, via_type] {
            match ev {
                WsEvent::AgentApproval {
                    approval_id,
                    agent_id,
                    summary,
                    ..
                } => {
                    assert_eq!(approval_id, "ap-1");
                    assert_eq!(agent_id, "ag-1");
                    assert_eq!(summary, "spawn cli.claude");
                }
                other => panic!("expected approval, got {other:?}"),
            }
        }
        let mut messages = Vec::new();
        let ev = parse_event(
            r#"{"event":"agent.approval","approvalId":"ap-1","agentId":"ag-1","taskId":"task-1","kind":"exec","summary":"spawn"}"#,
        )
        .unwrap();
        assert_eq!(
            apply_event(&mut messages, &ev, Some("task-1"), Some("ag-1")),
            ApplyOutcome::Approval
        );
        assert!(messages.is_empty());
    }

    #[test]
    fn pty_events_do_not_touch_chat_messages() {
        let mut messages = Vec::new();
        let data = parse_event(r#"{"type":"pty.data","ptyId":"pty-9","data":"aGVsbG8="}"#).unwrap();
        match &data {
            WsEvent::PtyData { pty_id, data } => {
                assert_eq!(pty_id, "pty-9");
                assert_eq!(data, "aGVsbG8=");
            }
            other => panic!("expected pty.data, got {other:?}"),
        }
        assert_eq!(
            apply_event(&mut messages, &data, Some("task-1"), Some("ag-1")),
            ApplyOutcome::PtyData
        );
        assert!(messages.is_empty());
        let exit = parse_event(r#"{"event":"pty.exit","ptyId":"pty-9","code":1}"#).unwrap();
        assert_eq!(
            apply_event(&mut messages, &exit, None, None),
            ApplyOutcome::PtyExit
        );
        assert!(messages.is_empty());
    }
}
