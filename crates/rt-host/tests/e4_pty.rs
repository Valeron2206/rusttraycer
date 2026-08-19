//! E4 PTY mux and terminal interface (C32–C36, C37). Transcript ≠ scrollback.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures::{SinkExt, Stream, StreamExt};
use rt_runtime::{AgentBackend, Availability, HarnessCaps, TurnEvent, TurnRequest};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::HeaderValue;
use tokio_tungstenite::tungstenite::Message as WsMsg;

struct GenericBackend;

impl AgentBackend for GenericBackend {
    fn id(&self) -> &'static str {
        "cli.generic"
    }
    fn available(&self) -> Availability {
        Availability {
            available: true,
            detail: "generic mock".into(),
        }
    }
    fn start_turn(&self, _req: TurnRequest) -> Pin<Box<dyn Stream<Item = TurnEvent> + Send>> {
        Box::pin(futures::stream::iter([
            TurnEvent::Token {
                text: "chat-ok\n".into(),
            },
            TurnEvent::Finished { exit_code: 0 },
        ]))
    }
}

struct ClaudePtyBackend;

impl AgentBackend for ClaudePtyBackend {
    fn id(&self) -> &'static str {
        "cli.claude"
    }
    fn available(&self) -> Availability {
        Availability {
            available: true,
            detail: "claude pty mock".into(),
        }
    }
    fn caps(&self) -> HarnessCaps {
        HarnessCaps {
            one_shot: true,
            long_lived: false,
            stream_tokens: true,
            tools: false,
            session_resume: true,
            a2a_inbox: false,
            pty: true,
            needs_api_key: false,
            api_key_env: None,
        }
    }
    fn start_turn(&self, _req: TurnRequest) -> Pin<Box<dyn Stream<Item = TurnEvent> + Send>> {
        Box::pin(futures::stream::iter([TurnEvent::Finished {
            exit_code: 0,
        }]))
    }
}

fn v1_methods() -> Value {
    let names = [
        "host.ping",
        "host.doctor",
        "workspace.list",
        "workspace.add",
        "task.list",
        "task.create",
        "task.get",
        "task.rename",
        "task.archive",
        "agent.list",
        "agent.create",
        "agent.get",
        "agent.send",
        "agent.get_context",
        "agent.cancel",
        "files.tree",
        "files.read",
        "worktree.ensure",
        "worktree.get",
        "worktree.list",
        "git.status",
        "git.diff",
    ];
    let mut m = serde_json::Map::new();
    for n in names {
        m.insert(n.to_string(), json!({ "major": 1, "minor": 0 }));
    }
    Value::Object(m)
}

fn f1_methods() -> Value {
    let mut m = v1_methods();
    if let Value::Object(map) = &mut m {
        map.insert("policy.get".into(), json!({ "major": 1, "minor": 1 }));
        map.insert("policy.set".into(), json!({ "major": 1, "minor": 1 }));
        map.insert("approval.respond".into(), json!({ "major": 1, "minor": 1 }));
    }
    m
}

fn f2_methods() -> Value {
    let mut m = f1_methods();
    if let Value::Object(map) = &mut m {
        for n in [
            "files.write",
            "files.patch",
            "files.open",
            "git.stage",
            "git.unstage",
            "git.restore",
            "git.commit",
            "git.push",
        ] {
            map.insert(n.into(), json!({ "major": 1, "minor": 2 }));
        }
    }
    m
}

fn f3_methods() -> Value {
    let mut m = f2_methods();
    if let Value::Object(map) = &mut m {
        map.insert("agent.create".into(), json!({ "major": 1, "minor": 3 }));
        for n in [
            "shell.create",
            "shell.list",
            "shell.close",
            "pty.open",
            "pty.write",
            "pty.resize",
            "pty.close",
        ] {
            map.insert(n.into(), json!({ "major": 1, "minor": 3 }));
        }
    }
    m
}

fn pty_backends() -> HashMap<String, Arc<dyn AgentBackend>> {
    let mut m: HashMap<String, Arc<dyn AgentBackend>> = HashMap::new();
    m.insert("cli.generic".into(), Arc::new(GenericBackend));
    m.insert("cli.claude".into(), Arc::new(ClaudePtyBackend));
    m
}

fn set_pty_cmd() {
    std::env::set_var("RUSTTRAYCER_PTY_CMD", "/bin/cat");
}

async fn rpc(
    client: &reqwest::Client,
    base: &str,
    token: Option<&str>,
    method: &str,
    params: Value,
) -> Value {
    let mut req = client.post(format!("{base}/rpc")).json(&json!({
        "id": "t1",
        "method": method,
        "params": params,
    }));
    if let Some(t) = token {
        req = req.header("X-Rt-Session", t);
    }
    req.send().await.unwrap().json().await.unwrap()
}

async fn handshake(client: &reqwest::Client, base: &str, methods: Value) -> (String, Value) {
    let hs = rpc(
        client,
        base,
        None,
        "handshake",
        json!({
            "client": "cli",
            "clientVersion": "0.1.0",
            "methods": methods
        }),
    )
    .await;
    let token = hs["ok"]["sessionToken"]
        .as_str()
        .unwrap_or_else(|| panic!("handshake={hs}"))
        .to_string();
    (token, hs)
}

fn rpc_id(resp: &Value, field: &str) -> String {
    resp["ok"][field]
        .as_str()
        .unwrap_or_else(|| panic!("{field} missing: {resp}"))
        .to_string()
}

async fn seed_task(
    client: &reqwest::Client,
    base: &str,
    token: &str,
    proj: &std::path::Path,
) -> (String, String) {
    std::fs::create_dir_all(proj).unwrap();
    let ws = rpc(
        client,
        base,
        Some(token),
        "workspace.add",
        json!({ "path": proj.to_str().unwrap() }),
    )
    .await;
    let ws_id = rpc_id(&ws, "id");
    let task = rpc(
        client,
        base,
        Some(token),
        "task.create",
        json!({ "title": "e4", "workspaceId": ws_id }),
    )
    .await;
    (ws_id, rpc_id(&task, "id"))
}

type TestWs =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn connect_ws(addr: std::net::SocketAddr, token: &str, task_id: &str) -> TestWs {
    let url = format!("ws://{addr}/ws");
    let mut req = url.into_client_request().unwrap();
    req.headers_mut()
        .insert("X-Rt-Session", HeaderValue::from_str(token).unwrap());
    let (mut ws, _) = tokio_tungstenite::connect_async(req).await.unwrap();
    ws.send(WsMsg::Text(
        json!({ "type": "subscribe", "taskId": task_id })
            .to_string()
            .into(),
    ))
    .await
    .unwrap();
    ws
}

async fn wait_event(ws: &mut TestWs, event: &str) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        if left.is_zero() {
            panic!("timed out waiting for {event}");
        }
        let msg = tokio::time::timeout(left, ws.next())
            .await
            .unwrap_or_else(|_| panic!("timeout waiting for {event}"))
            .unwrap_or_else(|| panic!("ws closed waiting for {event}"))
            .unwrap();
        let WsMsg::Text(text) = msg else { continue };
        let v: Value = serde_json::from_str(&text).unwrap();
        if v["event"] == event {
            return v;
        }
    }
}

async fn wait_pty_data_containing(ws: &mut TestWs, pty_id: &str, needle: &str) -> Value {
    use base64::Engine;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        if left.is_zero() {
            panic!("timed out waiting for pty.data {needle} on {pty_id}");
        }
        let msg = tokio::time::timeout(left, ws.next())
            .await
            .unwrap_or_else(|_| panic!("timeout pty.data"))
            .unwrap_or_else(|| panic!("ws closed pty.data"))
            .unwrap();
        let WsMsg::Text(text) = msg else { continue };
        let v: Value = serde_json::from_str(&text).unwrap();
        if v["event"] != "pty.data" {
            continue;
        }
        if v["ptyId"] != pty_id {
            continue;
        }
        let data = v["data"].as_str().unwrap_or("");
        let raw = base64::engine::general_purpose::STANDARD
            .decode(data)
            .unwrap_or_default();
        if String::from_utf8_lossy(&raw).contains(needle) {
            return v;
        }
    }
}

fn b64(s: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(s.as_bytes())
}

async fn yolo_agent(client: &reqwest::Client, base: &str, token: &str, agent_id: &str) {
    let r = rpc(
        client,
        base,
        Some(token),
        "policy.set",
        json!({
            "agentId": agent_id,
            "mode": "ask",
            "scope": "agent",
            "yolo": true
        }),
    )
    .await;
    assert!(r.get("error").is_none(), "policy.set={r}");
}

#[tokio::test]
async fn create_terminal_generic_is_not_pty() {
    set_pty_cmd();
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(pty_backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f3_methods()).await;
    let proj = dir.path().join("proj");
    let (_ws, task_id) = seed_task(&client, &base, &token, &proj).await;
    let created = rpc(
        &client,
        &base,
        Some(&token),
        "agent.create",
        json!({
            "taskId": task_id,
            "provider": "cli.generic",
            "interface": "terminal"
        }),
    )
    .await;
    assert_eq!(created["error"]["code"], "not_pty", "{created}");
    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn pty_open_ask_emits_exec_approval_deny_no_child() {
    set_pty_cmd();
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(pty_backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f3_methods()).await;
    let proj = dir.path().join("proj");
    let (_ws, task_id) = seed_task(&client, &base, &token, &proj).await;
    let agent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.create",
        json!({
            "taskId": task_id,
            "provider": "cli.claude",
            "interface": "terminal"
        }),
    )
    .await;
    let agent_id = rpc_id(&agent, "id");
    assert_eq!(agent["ok"]["interface"], "terminal");
    assert!(agent["ok"]["providerSessionId"].is_null());

    let mut ws = connect_ws(addr, &token, &task_id).await;
    let open = rpc(
        &client,
        &base,
        Some(&token),
        "pty.open",
        json!({ "agentId": agent_id, "cols": 80, "rows": 24 }),
    )
    .await;
    assert!(open.get("error").is_none(), "{open}");
    assert!(open["ok"]["ptyId"].is_null(), "{open}");
    let approval_id = rpc_id(&open, "approvalId");

    let ev = wait_event(&mut ws, "agent.approval").await;
    assert_eq!(ev["kind"], "exec");
    assert_eq!(ev["agentId"], agent_id);
    assert!(
        ev["summary"].as_str().unwrap().contains("spawn pty"),
        "{}",
        ev["summary"]
    );

    let deny = rpc(
        &client,
        &base,
        Some(&token),
        "approval.respond",
        json!({ "approvalId": approval_id, "decision": "deny" }),
    )
    .await;
    assert_eq!(deny["ok"]["applied"], true, "{deny}");

    let again = rpc(
        &client,
        &base,
        Some(&token),
        "pty.open",
        json!({ "agentId": agent_id, "cols": 80, "rows": 24 }),
    )
    .await;
    assert!(
        again["ok"]["ptyId"].is_null(),
        "deny must leave no live pty: {again}"
    );
    assert!(again["ok"]["approvalId"].is_string(), "{again}");

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn two_ptys_data_does_not_mix_pty_id() {
    set_pty_cmd();
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(pty_backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f3_methods()).await;
    let proj = dir.path().join("proj");
    let (ws_id, task_id) = seed_task(&client, &base, &token, &proj).await;
    let agent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.create",
        json!({
            "taskId": task_id,
            "provider": "cli.claude",
            "interface": "terminal"
        }),
    )
    .await;
    let agent_id = rpc_id(&agent, "id");
    yolo_agent(&client, &base, &token, &agent_id).await;

    let mut ws = connect_ws(addr, &token, &task_id).await;
    let open = rpc(
        &client,
        &base,
        Some(&token),
        "pty.open",
        json!({ "agentId": agent_id, "cols": 80, "rows": 24 }),
    )
    .await;
    let agent_pty = rpc_id(&open, "ptyId");
    assert_eq!(open["ok"]["resumed"], false, "{open}");

    let sh = rpc(
        &client,
        &base,
        Some(&token),
        "shell.create",
        json!({
            "taskId": task_id,
            "workspaceId": ws_id,
            "cols": 80,
            "rows": 24
        }),
    )
    .await;
    let shell_pty = rpc_id(&sh, "ptyId");
    assert_ne!(agent_pty, shell_pty, "{open} {sh}");

    rpc(
        &client,
        &base,
        Some(&token),
        "pty.write",
        json!({ "ptyId": agent_pty, "data": b64("AGENT-MARK\n") }),
    )
    .await;
    rpc(
        &client,
        &base,
        Some(&token),
        "pty.write",
        json!({ "ptyId": shell_pty, "data": b64("SHELL-MARK\n") }),
    )
    .await;

    let a = wait_pty_data_containing(&mut ws, &agent_pty, "AGENT-MARK").await;
    let b = wait_pty_data_containing(&mut ws, &shell_pty, "SHELL-MARK").await;
    assert_eq!(a["ptyId"], agent_pty);
    assert_eq!(b["ptyId"], shell_pty);

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn pty_bytes_never_insert_into_messages() {
    set_pty_cmd();
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(pty_backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f3_methods()).await;
    let proj = dir.path().join("proj");
    let (_ws, task_id) = seed_task(&client, &base, &token, &proj).await;
    let agent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.create",
        json!({
            "taskId": task_id,
            "provider": "cli.claude",
            "interface": "terminal"
        }),
    )
    .await;
    let agent_id = rpc_id(&agent, "id");
    yolo_agent(&client, &base, &token, &agent_id).await;
    let open = rpc(
        &client,
        &base,
        Some(&token),
        "pty.open",
        json!({ "agentId": agent_id, "cols": 80, "rows": 24 }),
    )
    .await;
    let pty_id = rpc_id(&open, "ptyId");
    let w = rpc(
        &client,
        &base,
        Some(&token),
        "pty.write",
        json!({ "ptyId": pty_id, "data": b64("scrollback-must-not-persist\n") }),
    )
    .await;
    assert!(w.get("error").is_none(), "{w}");
    tokio::time::sleep(Duration::from_millis(150)).await;

    let ctx = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get_context",
        json!({ "agentId": agent_id }),
    )
    .await;
    let msgs = ctx["ok"]["messages"].as_array().unwrap();
    assert!(
        msgs.is_empty(),
        "PTY bytes must not land in messages: {ctx}"
    );

    let db = dir.path().join("host.db");
    let conn = rusqlite::Connection::open(&db).unwrap();
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE agent_id = ?1",
            [&agent_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 0, "sqlite messages must stay empty");

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn restart_chat_messages_live_pty_dead_terminal_resumes() {
    set_pty_cmd();
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().to_path_buf();
    let (chat_id, term_id, old_pty, _ws_id, _task_id) = {
        let (addr, tx, join, _) = rt_host::spawn_test_host(&data, Some(pty_backends()))
            .await
            .unwrap();
        let base = format!("http://{addr}");
        let client = reqwest::Client::new();
        let (token, _) = handshake(&client, &base, f3_methods()).await;
        let proj = data.join("proj");
        let (ws_id, task_id) = seed_task(&client, &base, &token, &proj).await;
        let chat = rpc(
            &client,
            &base,
            Some(&token),
            "agent.create",
            json!({ "taskId": task_id, "provider": "cli.generic" }),
        )
        .await;
        let chat_id = rpc_id(&chat, "id");
        let sent = rpc(
            &client,
            &base,
            Some(&token),
            "agent.send",
            json!({ "agentId": chat_id, "content": "hello-chat" }),
        )
        .await;
        assert!(sent.get("error").is_none(), "{sent}");
        tokio::time::sleep(Duration::from_millis(80)).await;

        let term = rpc(
            &client,
            &base,
            Some(&token),
            "agent.create",
            json!({
                "taskId": task_id,
                "provider": "cli.claude",
                "interface": "terminal"
            }),
        )
        .await;
        let term_id = rpc_id(&term, "id");
        yolo_agent(&client, &base, &token, &term_id).await;
        let open = rpc(
            &client,
            &base,
            Some(&token),
            "pty.open",
            json!({ "agentId": term_id, "cols": 80, "rows": 24 }),
        )
        .await;
        let old_pty = rpc_id(&open, "ptyId");
        let got = rpc(
            &client,
            &base,
            Some(&token),
            "agent.get",
            json!({ "id": term_id }),
        )
        .await;
        assert!(
            !got["ok"]["providerSessionId"].as_str().unwrap().is_empty(),
            "{got}"
        );

        let _ = tx.send(());
        let _ = join.await;
        (chat_id, term_id, old_pty, ws_id, task_id)
    };

    let (addr, tx, join, _) = rt_host::spawn_test_host(&data, Some(pty_backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f3_methods()).await;

    let ctx = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get_context",
        json!({ "agentId": chat_id }),
    )
    .await;
    let msgs = ctx["ok"]["messages"].as_array().unwrap();
    assert!(
        msgs.iter().any(|m| m["content"] == "hello-chat"),
        "chat messages must persist: {ctx}"
    );

    let dead = rpc(
        &client,
        &base,
        Some(&token),
        "pty.write",
        json!({ "ptyId": old_pty, "data": b64("x\n") }),
    )
    .await;
    assert_eq!(dead["error"]["code"], "pty_dead", "{dead}");

    let open = rpc(
        &client,
        &base,
        Some(&token),
        "pty.open",
        json!({ "agentId": term_id, "cols": 80, "rows": 24 }),
    )
    .await;
    assert!(open.get("error").is_none(), "{open}");
    assert_eq!(open["ok"]["resumed"], true, "{open}");
    assert!(open["ok"]["ptyId"].as_str().unwrap() != old_pty);

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn shell_absent_from_list_after_restart_new_create_is_new_pty() {
    set_pty_cmd();
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().to_path_buf();
    let (old_pty, ws_id, task_id) = {
        let (addr, tx, join, _) = rt_host::spawn_test_host(&data, Some(pty_backends()))
            .await
            .unwrap();
        let base = format!("http://{addr}");
        let client = reqwest::Client::new();
        let (token, _) = handshake(&client, &base, f3_methods()).await;
        let proj = data.join("proj");
        let (ws_id, task_id) = seed_task(&client, &base, &token, &proj).await;
        let sh = rpc(
            &client,
            &base,
            Some(&token),
            "shell.create",
            json!({
                "taskId": task_id,
                "workspaceId": ws_id,
                "cols": 80,
                "rows": 24
            }),
        )
        .await;
        let old_pty = rpc_id(&sh, "ptyId");
        let listed = rpc(
            &client,
            &base,
            Some(&token),
            "shell.list",
            json!({ "taskId": task_id }),
        )
        .await;
        assert_eq!(
            listed["ok"]["items"].as_array().unwrap().len(),
            1,
            "{listed}"
        );
        let _ = tx.send(());
        let _ = join.await;
        (old_pty, ws_id, task_id)
    };

    let (addr, tx, join, _) = rt_host::spawn_test_host(&data, Some(pty_backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f3_methods()).await;
    let listed = rpc(
        &client,
        &base,
        Some(&token),
        "shell.list",
        json!({ "taskId": task_id }),
    )
    .await;
    assert_eq!(
        listed["ok"]["items"].as_array().unwrap().len(),
        0,
        "shells are live-only: {listed}"
    );
    let sh = rpc(
        &client,
        &base,
        Some(&token),
        "shell.create",
        json!({
            "taskId": task_id,
            "workspaceId": ws_id,
            "cols": 80,
            "rows": 24
        }),
    )
    .await;
    let new_pty = rpc_id(&sh, "ptyId");
    assert_ne!(new_pty, old_pty);

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn shell_create_without_task_id_is_invalid_params() {
    set_pty_cmd();
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(pty_backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f3_methods()).await;
    let missing = rpc(
        &client,
        &base,
        Some(&token),
        "shell.create",
        json!({ "workspaceId": "w", "cols": 80, "rows": 24 }),
    )
    .await;
    assert_eq!(missing["error"]["code"], "invalid_params", "{missing}");
    let empty = rpc(
        &client,
        &base,
        Some(&token),
        "shell.create",
        json!({ "taskId": "", "workspaceId": "w", "cols": 80, "rows": 24 }),
    )
    .await;
    assert_eq!(empty["error"]["code"], "invalid_params", "{empty}");
    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn client_without_1_3_chat_and_write_live() {
    set_pty_cmd();
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(pty_backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    // 1.2 client without policy.*: write/send live, no edit card, no 1.3 pty.
    let mut methods = v1_methods();
    if let Value::Object(map) = &mut methods {
        for n in [
            "files.write",
            "files.patch",
            "files.open",
            "git.stage",
            "git.unstage",
            "git.restore",
            "git.commit",
            "git.push",
        ] {
            map.insert(n.into(), json!({ "major": 1, "minor": 2 }));
        }
    }
    let (token, hs) = handshake(&client, &base, methods).await;
    assert!(hs["ok"]["accepted"]["agent.send"].is_object(), "{hs}");
    assert!(hs["ok"]["accepted"]["files.write"].is_object(), "{hs}");
    assert!(hs["ok"]["accepted"].get("pty.open").is_none(), "{hs}");
    assert_eq!(
        hs["ok"]["accepted"]["agent.create"]["minor"], 5,
        "offering 1.0 still accepts host 1.5: {hs}"
    );
    assert_eq!(hs["ok"]["accepted"]["files.write"]["minor"], 2);
    assert_eq!(hs["ok"]["accepted"]["host.ping"]["minor"], 0);

    let proj = dir.path().join("proj");
    let (ws_id, task_id) = seed_task(&client, &base, &token, &proj).await;
    let chat = rpc(
        &client,
        &base,
        Some(&token),
        "agent.create",
        json!({ "taskId": task_id, "provider": "cli.generic" }),
    )
    .await;
    let chat_id = rpc_id(&chat, "id");
    let write = rpc(
        &client,
        &base,
        Some(&token),
        "files.write",
        json!({
            "workspaceId": ws_id,
            "agentId": chat_id,
            "path": "a.txt",
            "content": "x"
        }),
    )
    .await;
    assert!(write.get("error").is_none(), "{write}");

    let sent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.send",
        json!({ "agentId": chat_id, "content": "hi" }),
    )
    .await;
    assert!(sent.get("error").is_none(), "{sent}");

    let open = rpc(
        &client,
        &base,
        Some(&token),
        "pty.open",
        json!({ "agentId": chat_id, "cols": 80, "rows": 24 }),
    )
    .await;
    assert_eq!(open["error"]["code"], "version_mismatch", "{open}");

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn agent_send_on_terminal_is_invalid_params() {
    set_pty_cmd();
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(pty_backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f3_methods()).await;
    let proj = dir.path().join("proj");
    let (_ws, task_id) = seed_task(&client, &base, &token, &proj).await;
    let agent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.create",
        json!({
            "taskId": task_id,
            "provider": "cli.claude",
            "interface": "terminal"
        }),
    )
    .await;
    let agent_id = rpc_id(&agent, "id");
    let sent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.send",
        json!({ "agentId": agent_id, "content": "nope" }),
    )
    .await;
    assert_eq!(sent["error"]["code"], "invalid_params", "{sent}");
    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn handshake_agent_create_1_3_write_1_2_ping_1_0() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(pty_backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (_token, hs) = handshake(&client, &base, f3_methods()).await;
    assert_eq!(hs["ok"]["accepted"]["agent.create"]["major"], 1);
    assert_eq!(hs["ok"]["accepted"]["agent.create"]["minor"], 5);
    assert_eq!(hs["ok"]["accepted"]["files.write"]["minor"], 2);
    assert_eq!(hs["ok"]["accepted"]["host.ping"]["minor"], 0);
    assert_eq!(hs["ok"]["accepted"]["pty.open"]["minor"], 3);
    let _ = tx.send(());
    let _ = join.await;
}
