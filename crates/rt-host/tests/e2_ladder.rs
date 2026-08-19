//! E2 permission ladder acceptance (host/storage/protocol). C26: default is ask.

use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures::{SinkExt, Stream, StreamExt};
use rt_runtime::{AgentBackend, Availability, TurnEvent, TurnRequest};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::HeaderValue;
use tokio_tungstenite::tungstenite::Message as WsMsg;

struct CountingBackend {
    starts: Arc<AtomicUsize>,
}

impl AgentBackend for CountingBackend {
    fn id(&self) -> &'static str {
        "cli.generic"
    }
    fn available(&self) -> Availability {
        Availability {
            available: true,
            detail: "counting".into(),
        }
    }
    fn start_turn(&self, _req: TurnRequest) -> Pin<Box<dyn Stream<Item = TurnEvent> + Send>> {
        self.starts.fetch_add(1, Ordering::SeqCst);
        Box::pin(futures::stream::iter([
            TurnEvent::Token {
                text: "ok\n".into(),
            },
            TurnEvent::Finished { exit_code: 0 },
        ]))
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

async fn seed_agent(
    client: &reqwest::Client,
    base: &str,
    token: &str,
    proj: &std::path::Path,
) -> (String, String, String) {
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
        json!({ "title": "ladder", "workspaceId": ws_id }),
    )
    .await;
    let task_id = rpc_id(&task, "id");
    let agent = rpc(
        client,
        base,
        Some(token),
        "agent.create",
        json!({ "taskId": task_id, "provider": "cli.generic" }),
    )
    .await;
    let agent_id = rpc_id(&agent, "id");
    (ws_id, task_id, agent_id)
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
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
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

async fn no_event(ws: &mut TestWs, event: &str, wait: Duration) {
    let deadline = tokio::time::Instant::now() + wait;
    while tokio::time::Instant::now() < deadline {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(left, ws.next()).await {
            Ok(Some(Ok(WsMsg::Text(text)))) => {
                let v: Value = serde_json::from_str(&text).unwrap();
                assert_ne!(v["event"], event, "unexpected {event}: {v}");
            }
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(e))) => panic!("ws err: {e}"),
            Ok(None) => return,
            Err(_) => return,
        }
    }
}

fn counting() -> (
    Arc<AtomicUsize>,
    std::collections::HashMap<String, Arc<dyn AgentBackend>>,
) {
    let starts = Arc::new(AtomicUsize::new(0));
    let backend: Arc<dyn AgentBackend> = Arc::new(CountingBackend {
        starts: starts.clone(),
    });
    let mut backends = std::collections::HashMap::new();
    backends.insert("cli.generic".into(), backend);
    (starts, backends)
}

async fn wait_idle(client: &reqwest::Client, base: &str, token: &str, agent_id: &str) {
    let mut last = Value::Null;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(40)).await;
        last = rpc(
            client,
            base,
            Some(token),
            "agent.get",
            json!({ "id": agent_id }),
        )
        .await;
        if last["ok"]["status"] == "idle" {
            return;
        }
    }
    panic!("agent not idle: {last}");
}

#[tokio::test]
async fn policy_get_new_agent_is_ask_source_default() {
    let dir = tempfile::tempdir().unwrap();
    let (_starts, backends) = counting();
    let (addr, shutdown, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, hs) = handshake(&client, &base, f1_methods()).await;
    assert_eq!(hs["ok"]["accepted"]["policy.get"]["minor"], 1);

    let (_ws_id, _task_id, agent_id) =
        seed_agent(&client, &base, &token, &dir.path().join("proj")).await;
    let got = rpc(
        &client,
        &base,
        Some(&token),
        "policy.get",
        json!({ "agentId": agent_id }),
    )
    .await;
    assert_eq!(got["ok"]["mode"], "ask", "C26: not full-access: {got}");
    assert_eq!(got["ok"]["source"], "default");
    assert_eq!(got["ok"]["yolo"], false);
    assert_eq!(got["ok"]["scope"], "agent");

    let both = rpc(
        &client,
        &base,
        Some(&token),
        "policy.get",
        json!({ "agentId": agent_id, "workspaceId": "x" }),
    )
    .await;
    assert_eq!(both["error"]["code"], "invalid_params");
    let missing = rpc(
        &client,
        &base,
        Some(&token),
        "policy.get",
        json!({ "agentId": "0191f0c6-7c2a-7c11-8000-6f0c1a2b3c4d" }),
    )
    .await;
    assert_eq!(missing["error"]["code"], "not_found");

    let _ = shutdown.send(());
    let _ = join.await;
}

#[tokio::test]
async fn send_ask_emits_approval_deny_has_no_child() {
    let dir = tempfile::tempdir().unwrap();
    let (starts, backends) = counting();
    let (addr, shutdown, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f1_methods()).await;
    let (_ws_id, task_id, agent_id) =
        seed_agent(&client, &base, &token, &dir.path().join("proj")).await;

    let mut ws = connect_ws(addr, &token, &task_id).await;
    let sent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.send",
        json!({ "agentId": agent_id, "content": "hello" }),
    )
    .await;
    assert_eq!(sent["ok"]["userMessage"]["content"], "hello");
    let approval_id = sent["ok"]["approvalId"]
        .as_str()
        .unwrap_or_else(|| panic!("approvalId missing: {sent}"))
        .to_string();

    let ev = wait_event(&mut ws, "agent.approval").await;
    assert_eq!(ev["approvalId"], approval_id);
    assert_eq!(ev["agentId"], agent_id);
    assert_eq!(ev["taskId"], task_id);
    assert_eq!(ev["kind"], "exec");
    assert!(ev["summary"].as_str().unwrap().starts_with("spawn "));

    let deny = rpc(
        &client,
        &base,
        Some(&token),
        "approval.respond",
        json!({ "approvalId": approval_id, "decision": "deny" }),
    )
    .await;
    assert_eq!(deny["ok"]["applied"], true);
    tokio::time::sleep(Duration::from_millis(80)).await;
    assert_eq!(starts.load(Ordering::SeqCst), 0, "deny must not spawn");
    let got = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get",
        json!({ "id": agent_id }),
    )
    .await;
    assert_eq!(got["ok"]["status"], "idle");

    let again = rpc(
        &client,
        &base,
        Some(&token),
        "approval.respond",
        json!({ "approvalId": approval_id, "decision": "deny" }),
    )
    .await;
    assert_eq!(again["ok"]["applied"], false);

    let _ = shutdown.send(());
    let _ = join.await;
}

#[tokio::test]
async fn send_ask_allow_once_one_turn_mode_stays_ask() {
    let dir = tempfile::tempdir().unwrap();
    let (starts, backends) = counting();
    let (addr, shutdown, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f1_methods()).await;
    let (_ws_id, task_id, agent_id) =
        seed_agent(&client, &base, &token, &dir.path().join("proj")).await;
    let mut ws = connect_ws(addr, &token, &task_id).await;

    let sent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.send",
        json!({ "agentId": agent_id, "content": "once" }),
    )
    .await;
    let approval_id = sent["ok"]["approvalId"].as_str().unwrap().to_string();
    let _ = wait_event(&mut ws, "agent.approval").await;

    let allow = rpc(
        &client,
        &base,
        Some(&token),
        "approval.respond",
        json!({ "approvalId": approval_id, "decision": "allow-once" }),
    )
    .await;
    assert_eq!(allow["ok"]["applied"], true);
    wait_idle(&client, &base, &token, &agent_id).await;
    assert_eq!(starts.load(Ordering::SeqCst), 1);

    let got = rpc(
        &client,
        &base,
        Some(&token),
        "policy.get",
        json!({ "agentId": agent_id }),
    )
    .await;
    assert_eq!(got["ok"]["mode"], "ask");
    assert_eq!(got["ok"]["source"], "default");

    let _ = shutdown.send(());
    let _ = join.await;
}

#[tokio::test]
async fn allow_always_persists_across_host_restart() {
    let dir = tempfile::tempdir().unwrap();
    let (starts, backends) = counting();
    let (addr, shutdown, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f1_methods()).await;
    let (_ws_id, task_id, agent_id) =
        seed_agent(&client, &base, &token, &dir.path().join("proj")).await;

    let set = rpc(
        &client,
        &base,
        Some(&token),
        "policy.set",
        json!({
            "agentId": agent_id,
            "mode": "allow-always",
            "scope": "agent",
            "yolo": false
        }),
    )
    .await;
    assert_eq!(set["ok"]["mode"], "allow-always");
    assert_eq!(set["ok"]["source"], "agent");

    let _ = shutdown.send(());
    let _ = join.await;
    assert!(
        !rt_host::bind::pid_path(dir.path()).exists(),
        "pid.json must be gone"
    );

    let (starts2, backends2) = counting();
    let (addr2, shutdown2, join2, _) = rt_host::spawn_test_host(dir.path(), Some(backends2))
        .await
        .unwrap();
    let base2 = format!("http://{addr2}");
    let (token2, _) = handshake(&client, &base2, f1_methods()).await;
    let mut ws = connect_ws(addr2, &token2, &task_id).await;

    let sent = rpc(
        &client,
        &base2,
        Some(&token2),
        "agent.send",
        json!({ "agentId": agent_id, "content": "after-restart" }),
    )
    .await;
    assert_eq!(sent["ok"]["userMessage"]["content"], "after-restart");
    assert!(sent["ok"].get("approvalId").is_none() || sent["ok"]["approvalId"].is_null());
    no_event(&mut ws, "agent.approval", Duration::from_millis(250)).await;
    wait_idle(&client, &base2, &token2, &agent_id).await;
    assert_eq!(starts2.load(Ordering::SeqCst), 1);
    assert_eq!(starts.load(Ordering::SeqCst), 0);

    let _ = shutdown2.send(());
    let _ = join2.await;
}

#[tokio::test]
async fn yolo_bypasses_ladder_then_ask_after_clear() {
    let dir = tempfile::tempdir().unwrap();
    let (starts, backends) = counting();
    let (addr, shutdown, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f1_methods()).await;
    let (_ws_id, task_id, agent_id) =
        seed_agent(&client, &base, &token, &dir.path().join("proj")).await;
    let mut ws = connect_ws(addr, &token, &task_id).await;

    let set = rpc(
        &client,
        &base,
        Some(&token),
        "policy.set",
        json!({
            "agentId": agent_id,
            "mode": "ask",
            "scope": "agent",
            "yolo": true
        }),
    )
    .await;
    assert_eq!(set["ok"]["yolo"], true);
    let got = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get",
        json!({ "id": agent_id }),
    )
    .await;
    assert_eq!(got["ok"]["yolo"], true);

    let sent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.send",
        json!({ "agentId": agent_id, "content": "yolo-turn" }),
    )
    .await;
    assert_eq!(sent["ok"]["userMessage"]["content"], "yolo-turn");
    assert!(sent["ok"].get("approvalId").is_none() || sent["ok"]["approvalId"].is_null());
    no_event(&mut ws, "agent.approval", Duration::from_millis(250)).await;
    wait_idle(&client, &base, &token, &agent_id).await;
    assert_eq!(starts.load(Ordering::SeqCst), 1);

    let clear = rpc(
        &client,
        &base,
        Some(&token),
        "policy.set",
        json!({
            "agentId": agent_id,
            "mode": "ask",
            "scope": "agent",
            "yolo": false
        }),
    )
    .await;
    assert_eq!(clear["ok"]["yolo"], false);
    assert_eq!(clear["ok"]["mode"], "ask");
    let get = rpc(
        &client,
        &base,
        Some(&token),
        "policy.get",
        json!({ "agentId": agent_id }),
    )
    .await;
    assert_eq!(get["ok"]["mode"], "ask");
    assert_eq!(get["ok"]["yolo"], false);

    let sent2 = rpc(
        &client,
        &base,
        Some(&token),
        "agent.send",
        json!({ "agentId": agent_id, "content": "need-card" }),
    )
    .await;
    assert!(sent2["ok"]["approvalId"].is_string(), "{sent2}");
    let ev = wait_event(&mut ws, "agent.approval").await;
    assert_eq!(ev["event"], "agent.approval");
    assert_eq!(starts.load(Ordering::SeqCst), 1);

    let _ = shutdown.send(());
    let _ = join.await;
}

#[tokio::test]
async fn client_without_1_1_v1_send_passes() {
    let dir = tempfile::tempdir().unwrap();
    let (starts, backends) = counting();
    let (addr, shutdown, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, hs) = handshake(&client, &base, v1_methods()).await;
    assert!(hs["ok"]["accepted"]["policy.get"].is_null());
    let (_ws_id, _task_id, agent_id) =
        seed_agent(&client, &base, &token, &dir.path().join("proj")).await;

    let sent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.send",
        json!({ "agentId": agent_id, "content": "compat" }),
    )
    .await;
    assert_eq!(sent["ok"]["userMessage"]["content"], "compat");
    wait_idle(&client, &base, &token, &agent_id).await;
    assert_eq!(starts.load(Ordering::SeqCst), 1);

    let blocked = rpc(
        &client,
        &base,
        Some(&token),
        "policy.get",
        json!({ "agentId": agent_id }),
    )
    .await;
    assert_eq!(blocked["error"]["code"], "version_mismatch");

    let _ = shutdown.send(());
    let _ = join.await;
}

#[tokio::test]
async fn doctor_providers_include_caps() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, shutdown, join, _) = rt_host::spawn_test_host(dir.path(), None).await.unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, v1_methods()).await;
    let doctor = rpc(&client, &base, Some(&token), "host.doctor", json!({})).await;
    let providers = doctor["ok"]["providers"]
        .as_array()
        .unwrap_or_else(|| panic!("doctor={doctor}"));
    assert_eq!(providers.len(), 3, "{doctor}");
    let mut ids = Vec::new();
    for p in providers {
        ids.push(p["id"].as_str().unwrap().to_string());
        let caps = &p["caps"];
        assert!(caps.is_object(), "caps missing on {}: {p}", p["id"]);
        assert!(caps["oneShot"].is_boolean());
        assert!(caps["longLived"].is_boolean());
        assert!(caps["streamTokens"].is_boolean());
        assert!(caps["tools"].is_boolean());
        assert!(caps["sessionResume"].is_boolean());
        assert!(caps["a2aInbox"].is_boolean());
        assert!(caps["pty"].is_boolean());
        assert!(caps["needsApiKey"].is_boolean());
        assert!(caps.get("apiKeyEnv").is_some());
        assert!(caps.get("one_shot").is_none());
    }
    ids.sort();
    assert_eq!(ids, ["cli.claude", "cli.codex", "cli.generic"]);

    let _ = shutdown.send(());
    let _ = join.await;
}

#[tokio::test]
async fn doctor_exposes_yolo_flag() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, shutdown, join, _) = rt_host::spawn_test_host(dir.path(), None).await.unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f1_methods()).await;

    let doctor = rpc(&client, &base, Some(&token), "host.doctor", json!({})).await;
    assert_eq!(doctor["ok"]["yolo"], false, "{doctor}");
    let providers = doctor["ok"]["providers"]
        .as_array()
        .unwrap_or_else(|| panic!("doctor={doctor}"));
    assert!(!providers.is_empty(), "{doctor}");
    for p in providers {
        assert!(p["caps"].is_object(), "caps missing on {}: {p}", p["id"]);
    }

    let (_ws_id, _task_id, agent_id) =
        seed_agent(&client, &base, &token, &dir.path().join("proj")).await;
    let set = rpc(
        &client,
        &base,
        Some(&token),
        "policy.set",
        json!({
            "agentId": agent_id,
            "mode": "ask",
            "scope": "agent",
            "yolo": true
        }),
    )
    .await;
    assert_eq!(set["ok"]["yolo"], true);

    let doctor = rpc(&client, &base, Some(&token), "host.doctor", json!({})).await;
    assert_eq!(doctor["ok"]["yolo"], true, "{doctor}");
    let providers = doctor["ok"]["providers"]
        .as_array()
        .unwrap_or_else(|| panic!("doctor={doctor}"));
    for p in providers {
        assert!(p["caps"].is_object(), "caps missing on {}: {p}", p["id"]);
    }
    let got = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get",
        json!({ "id": agent_id }),
    )
    .await;
    assert_eq!(got["ok"]["yolo"], true);

    let clear = rpc(
        &client,
        &base,
        Some(&token),
        "policy.set",
        json!({
            "agentId": agent_id,
            "mode": "ask",
            "scope": "agent",
            "yolo": false
        }),
    )
    .await;
    assert_eq!(clear["ok"]["yolo"], false);

    let doctor = rpc(&client, &base, Some(&token), "host.doctor", json!({})).await;
    assert_eq!(doctor["ok"]["yolo"], false, "{doctor}");
    let got = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get",
        json!({ "id": agent_id }),
    )
    .await;
    assert_eq!(got["ok"]["yolo"], false);

    let _ = shutdown.send(());
    let _ = join.await;
}

#[tokio::test]
async fn send_while_approval_pending_is_agent_busy() {
    let dir = tempfile::tempdir().unwrap();
    let (starts, backends) = counting();
    let (addr, shutdown, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f1_methods()).await;
    let (_ws_id, _task_id, agent_id) =
        seed_agent(&client, &base, &token, &dir.path().join("proj")).await;

    let first = rpc(
        &client,
        &base,
        Some(&token),
        "agent.send",
        json!({ "agentId": agent_id, "content": "first" }),
    )
    .await;
    assert!(first["ok"]["approvalId"].is_string(), "{first}");
    let second = rpc(
        &client,
        &base,
        Some(&token),
        "agent.send",
        json!({ "agentId": agent_id, "content": "second" }),
    )
    .await;
    assert_eq!(second["error"]["code"], "agent_busy");
    assert_eq!(starts.load(Ordering::SeqCst), 0);

    let cancel = rpc(
        &client,
        &base,
        Some(&token),
        "agent.cancel",
        json!({ "agentId": agent_id }),
    )
    .await;
    assert_eq!(cancel["ok"]["cancelled"], true);
    let expired = rpc(
        &client,
        &base,
        Some(&token),
        "approval.respond",
        json!({
            "approvalId": first["ok"]["approvalId"],
            "decision": "allow-once"
        }),
    )
    .await;
    assert_eq!(expired["error"]["code"], "approval_expired");

    let _ = shutdown.send(());
    let _ = join.await;
}

#[tokio::test]
async fn policy_deny_send_is_denied() {
    let dir = tempfile::tempdir().unwrap();
    let (starts, backends) = counting();
    let (addr, shutdown, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f1_methods()).await;
    let (_ws_id, task_id, agent_id) =
        seed_agent(&client, &base, &token, &dir.path().join("proj")).await;
    let mut ws = connect_ws(addr, &token, &task_id).await;

    let set = rpc(
        &client,
        &base,
        Some(&token),
        "policy.set",
        json!({
            "agentId": agent_id,
            "mode": "deny",
            "scope": "agent",
            "yolo": false
        }),
    )
    .await;
    assert_eq!(set["ok"]["mode"], "deny");

    let sent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.send",
        json!({ "agentId": agent_id, "content": "nope" }),
    )
    .await;
    assert_eq!(sent["error"]["code"], "denied");
    no_event(&mut ws, "agent.approval", Duration::from_millis(200)).await;
    assert_eq!(starts.load(Ordering::SeqCst), 0);
    let ctx = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get_context",
        json!({ "agentId": agent_id }),
    )
    .await;
    assert!(ctx["ok"]["messages"].as_array().unwrap().is_empty());

    let _ = shutdown.send(());
    let _ = join.await;
}

#[tokio::test]
async fn handshake_policy_1_1_and_unknown_leftover() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, shutdown, join, _) = rt_host::spawn_test_host(dir.path(), None).await.unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let mut methods = f1_methods();
    if let Value::Object(map) = &mut methods {
        map.insert("leftover.foo".into(), json!({ "major": 1, "minor": 0 }));
    }
    let (_token, hs) = handshake(&client, &base, methods).await;
    assert_eq!(hs["ok"]["accepted"]["policy.get"]["major"], 1);
    assert_eq!(hs["ok"]["accepted"]["policy.get"]["minor"], 1);
    assert_eq!(
        hs["ok"]["rejected"]["leftover.foo"]["reason"],
        "unsupported"
    );

    let _ = shutdown.send(());
    let _ = join.await;
}
