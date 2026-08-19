//! E6 A2A delivery and bounded loops (protocol 1.5, storage 0006).

use std::path::Path;
use std::pin::Pin;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use futures::{SinkExt, Stream, StreamExt};
use rt_runtime::{AgentBackend, Availability, HarnessCaps, TurnEvent, TurnRequest};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::HeaderValue;
use tokio_tungstenite::tungstenite::Message as WsMsg;

struct InstantGeneric;

impl AgentBackend for InstantGeneric {
    fn id(&self) -> &'static str {
        "cli.generic"
    }
    fn available(&self) -> Availability {
        Availability {
            available: true,
            detail: "generic mock".into(),
        }
    }
    fn caps(&self) -> HarnessCaps {
        HarnessCaps::CLI_GENERIC
    }
    fn start_turn(&self, _req: TurnRequest) -> Pin<Box<dyn Stream<Item = TurnEvent> + Send>> {
        Box::pin(futures::stream::iter([
            TurnEvent::Token {
                text: "ok\n".into(),
            },
            TurnEvent::Finished { exit_code: 0 },
        ]))
    }
}

struct InstantClaudeInbox;

impl AgentBackend for InstantClaudeInbox {
    fn id(&self) -> &'static str {
        "cli.claude"
    }
    fn available(&self) -> Availability {
        Availability {
            available: true,
            detail: "claude inbox mock".into(),
        }
    }
    fn caps(&self) -> HarnessCaps {
        let mut caps = HarnessCaps::CLI_CLAUDE;
        caps.a2a_inbox = true;
        caps
    }
    fn start_turn(&self, _req: TurnRequest) -> Pin<Box<dyn Stream<Item = TurnEvent> + Send>> {
        Box::pin(futures::stream::iter([
            TurnEvent::Token {
                text: "ok\n".into(),
            },
            TurnEvent::Finished { exit_code: 0 },
        ]))
    }
}

struct InstantCodex;

impl AgentBackend for InstantCodex {
    fn id(&self) -> &'static str {
        "cli.codex"
    }
    fn available(&self) -> Availability {
        Availability {
            available: true,
            detail: "codex mock".into(),
        }
    }
    fn caps(&self) -> HarnessCaps {
        HarnessCaps::CLI_CODEX
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

fn f4_methods() -> Value {
    let mut m = f3_methods();
    if let Value::Object(map) = &mut m {
        for n in [
            "artifact.create",
            "artifact.get",
            "artifact.list",
            "artifact.update",
            "artifact.delete",
            "artifact.export",
            "comment.create",
            "comment.list",
            "comment.resolve",
            "agent.clear_transcript",
        ] {
            map.insert(n.into(), json!({ "major": 1, "minor": 4 }));
        }
    }
    m
}

fn f5_methods() -> Value {
    let mut m = f4_methods();
    if let Value::Object(map) = &mut m {
        map.insert("agent.create".into(), json!({ "major": 1, "minor": 5 }));
        for n in [
            "a2a.transcript",
            "a2a.deliver",
            "loop.start",
            "loop.get",
            "loop.stop",
        ] {
            map.insert(n.into(), json!({ "major": 1, "minor": 5 }));
        }
    }
    m
}

fn backends() -> std::collections::HashMap<String, Arc<dyn AgentBackend>> {
    let mut m = std::collections::HashMap::new();
    m.insert(
        "cli.generic".into(),
        Arc::new(InstantGeneric) as Arc<dyn AgentBackend>,
    );
    m.insert(
        "cli.claude".into(),
        Arc::new(InstantClaudeInbox) as Arc<dyn AgentBackend>,
    );
    m.insert(
        "cli.codex".into(),
        Arc::new(InstantCodex) as Arc<dyn AgentBackend>,
    );
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

fn git(cwd: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t.test")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t.test")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .expect("spawn git");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn init_git_repo(path: &Path) {
    std::fs::create_dir_all(path).unwrap();
    git(path, &["init", "-b", "main"]);
    git(path, &["config", "user.email", "t@t.test"]);
    git(path, &["config", "user.name", "t"]);
    git(path, &["config", "commit.gpgsign", "false"]);
    std::fs::write(path.join("README.md"), "hello\n").unwrap();
    git(path, &["add", "README.md"]);
    git(path, &["commit", "-m", "init"]);
}

async fn seed_task(
    client: &reqwest::Client,
    base: &str,
    token: &str,
    proj: &Path,
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
        json!({ "title": "e6", "workspaceId": ws_id }),
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
    let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
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

async fn create_agent(
    client: &reqwest::Client,
    base: &str,
    token: &str,
    task_id: &str,
    provider: &str,
    parent_id: Option<&str>,
) -> String {
    let mut p = json!({ "taskId": task_id, "provider": provider });
    if let Some(pid) = parent_id {
        p["parentId"] = json!(pid);
    }
    let resp = rpc(client, base, Some(token), "agent.create", p).await;
    assert!(resp.get("error").is_none(), "{resp}");
    rpc_id(&resp, "id")
}

#[tokio::test]
async fn handshake_agent_create_1_5_write_stays_1_2() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (_token, hs) = handshake(&client, &base, f5_methods()).await;
    assert_eq!(hs["ok"]["accepted"]["agent.create"]["minor"], 5);
    assert_eq!(hs["ok"]["accepted"]["a2a.deliver"]["minor"], 5);
    assert_eq!(hs["ok"]["accepted"]["loop.start"]["minor"], 5);
    assert_eq!(hs["ok"]["accepted"]["files.write"]["minor"], 2);
    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn deliver_to_claude_inbox_generic_codex_no_inbox() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f5_methods()).await;
    let proj = dir.path().join("proj");
    let (_ws_id, task_id) = seed_task(&client, &base, &token, &proj).await;
    let from = create_agent(&client, &base, &token, &task_id, "cli.generic", None).await;
    let claude = create_agent(&client, &base, &token, &task_id, "cli.claude", None).await;
    let generic = create_agent(&client, &base, &token, &task_id, "cli.generic", None).await;
    let codex = create_agent(&client, &base, &token, &task_id, "cli.codex", None).await;

    let ok = rpc(
        &client,
        &base,
        Some(&token),
        "a2a.deliver",
        json!({
            "fromAgentId": from,
            "toAgentId": claude,
            "content": "review this"
        }),
    )
    .await;
    assert!(ok.get("error").is_none(), "{ok}");
    let mid = rpc_id(&ok, "messageId");
    assert_eq!(ok["ok"]["toAgentId"], claude);

    let ctx = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get_context",
        json!({ "agentId": claude }),
    )
    .await;
    let msgs = ctx["ok"]["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0]["id"], mid);
    assert_eq!(msgs[0]["role"], "system");
    let content = msgs[0]["content"].as_str().unwrap();
    assert!(content.starts_with(&format!("a2a:{from}\n")), "{content}");
    assert!(content.contains("review this"));

    for to in [&generic, &codex] {
        let err = rpc(
            &client,
            &base,
            Some(&token),
            "a2a.deliver",
            json!({
                "fromAgentId": from,
                "toAgentId": to,
                "content": "nope"
            }),
        )
        .await;
        assert_eq!(err["error"]["code"], "no_inbox", "{err}");
    }

    let term = rpc(
        &client,
        &base,
        Some(&token),
        "agent.create",
        json!({ "taskId": task_id, "provider": "cli.claude", "interface": "terminal" }),
    )
    .await;
    let term_id = rpc_id(&term, "id");
    let tr = rpc(
        &client,
        &base,
        Some(&token),
        "a2a.transcript",
        json!({ "agentId": term_id }),
    )
    .await;
    assert_eq!(tr["error"]["code"], "internal", "{tr}");
    assert!(
        tr["error"]["message"]
            .as_str()
            .unwrap()
            .contains("vendor session unavailable"),
        "{tr}"
    );

    let missing = rpc(
        &client,
        &base,
        Some(&token),
        "a2a.transcript",
        json!({ "agentId": "00000000-0000-0000-0000-000000000000" }),
    )
    .await;
    assert_eq!(missing["error"]["code"], "not_found", "{missing}");

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn child_create_parent_id_own_messages_same_task() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f5_methods()).await;
    let proj = dir.path().join("proj");
    let (_ws, task_id) = seed_task(&client, &base, &token, &proj).await;
    let other_proj = dir.path().join("other");
    let (_ws2, other_task) = seed_task(&client, &base, &token, &other_proj).await;

    let parent = create_agent(&client, &base, &token, &task_id, "cli.claude", None).await;
    let _ = rpc(
        &client,
        &base,
        Some(&token),
        "agent.send",
        json!({ "agentId": parent, "content": "parent only" }),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(80)).await;

    let child = create_agent(
        &client,
        &base,
        &token,
        &task_id,
        "cli.claude",
        Some(&parent),
    )
    .await;
    let listed = rpc(
        &client,
        &base,
        Some(&token),
        "agent.list",
        json!({ "taskId": task_id }),
    )
    .await;
    let items = listed["ok"]["items"].as_array().unwrap();
    let child_row = items.iter().find(|a| a["id"] == child).unwrap();
    assert_eq!(child_row["parentId"], parent);

    let pctx = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get_context",
        json!({ "agentId": parent }),
    )
    .await;
    let cctx = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get_context",
        json!({ "agentId": child }),
    )
    .await;
    assert!(!pctx["ok"]["messages"].as_array().unwrap().is_empty());
    assert!(cctx["ok"]["messages"].as_array().unwrap().is_empty());

    let bad = rpc(
        &client,
        &base,
        Some(&token),
        "agent.create",
        json!({
            "taskId": other_task,
            "provider": "cli.claude",
            "parentId": parent
        }),
    )
    .await;
    assert_eq!(bad["error"]["code"], "invalid_params", "{bad}");

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn cycle_parent_id_invalid_params() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f5_methods()).await;
    let proj = dir.path().join("proj");
    let (_ws, task_id) = seed_task(&client, &base, &token, &proj).await;
    let a = create_agent(&client, &base, &token, &task_id, "cli.generic", None).await;
    let b = create_agent(&client, &base, &token, &task_id, "cli.generic", Some(&a)).await;

    let store = rt_storage::Store::open(dir.path().join("host.db")).unwrap();
    let err = store.agent_set_parent(&a, Some(&a)).unwrap_err();
    assert_eq!(err.code(), "invalid_params");
    let err = store.agent_set_parent(&a, Some(&b)).unwrap_err();
    assert_eq!(err.code(), "invalid_params");

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn e2e_artifact_child_deliver_context() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f5_methods()).await;
    let proj = dir.path().join("proj");
    let (_ws, task_id) = seed_task(&client, &base, &token, &proj).await;

    let art = rpc(
        &client,
        &base,
        Some(&token),
        "artifact.create",
        json!({
            "taskId": task_id,
            "kind": "spec",
            "title": "Auth",
            "body": "# Auth\n"
        }),
    )
    .await;
    assert!(art.get("error").is_none(), "{art}");
    let art_id = rpc_id(&art, "id");
    let got = rpc(
        &client,
        &base,
        Some(&token),
        "artifact.get",
        json!({ "artifactId": art_id }),
    )
    .await;
    assert_eq!(got["ok"]["title"], "Auth");

    let parent = create_agent(&client, &base, &token, &task_id, "cli.claude", None).await;
    let child = create_agent(
        &client,
        &base,
        &token,
        &task_id,
        "cli.claude",
        Some(&parent),
    )
    .await;
    let del = rpc(
        &client,
        &base,
        Some(&token),
        "a2a.deliver",
        json!({
            "fromAgentId": parent,
            "toAgentId": child,
            "content": "look at spec"
        }),
    )
    .await;
    assert!(del.get("error").is_none(), "{del}");
    let ctx = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get_context",
        json!({ "agentId": child }),
    )
    .await;
    let msgs = ctx["ok"]["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0]["role"], "system");
    assert!(
        msgs[0]["content"]
            .as_str()
            .unwrap()
            .starts_with(&format!("a2a:{parent}\n")),
        "{ctx}"
    );

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn loop_start_without_max_iterations_invalid_params() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f5_methods()).await;
    let proj = dir.path().join("proj");
    let (_ws, task_id) = seed_task(&client, &base, &token, &proj).await;
    let a = create_agent(&client, &base, &token, &task_id, "cli.generic", None).await;
    let b = create_agent(&client, &base, &token, &task_id, "cli.generic", None).await;
    let err = rpc(
        &client,
        &base,
        Some(&token),
        "loop.start",
        json!({
            "taskId": task_id,
            "agentIds": [a, b],
            "prompt": "talk"
        }),
    )
    .await;
    assert_eq!(err["error"]["code"], "invalid_params", "{err}");
    let err = rpc(
        &client,
        &base,
        Some(&token),
        "loop.start",
        json!({
            "taskId": task_id,
            "agentIds": [a, b],
            "maxIterations": 0,
            "prompt": "talk"
        }),
    )
    .await;
    assert_eq!(err["error"]["code"], "invalid_params", "{err}");

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn loop_start_max_2_stops_no_further_send() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f5_methods()).await;
    let proj = dir.path().join("proj");
    let (_ws, task_id) = seed_task(&client, &base, &token, &proj).await;
    let a = create_agent(&client, &base, &token, &task_id, "cli.generic", None).await;
    let b = create_agent(&client, &base, &token, &task_id, "cli.generic", None).await;
    let mut ws = connect_ws(addr, &token, &task_id).await;

    let started = rpc(
        &client,
        &base,
        Some(&token),
        "loop.start",
        json!({
            "taskId": task_id,
            "agentIds": [a, b],
            "maxIterations": 2,
            "prompt": "hello pair"
        }),
    )
    .await;
    assert!(started.get("error").is_none(), "{started}");
    let loop_id = rpc_id(&started, "loopId");
    assert_eq!(started["ok"]["iteration"], 0);
    assert_eq!(started["ok"]["turns"], 0);
    assert_eq!(started["ok"]["maxIterations"], 2);
    assert_eq!(started["ok"]["budgetTurns"], 4);

    let ev = wait_event(&mut ws, "loop.stopped").await;
    assert_eq!(ev["event"], "loop.stopped");
    assert_eq!(ev["loopId"], loop_id);
    assert_eq!(ev["reason"], "max_iterations");
    assert!(ev.get("type").is_none());
    assert!(ev.get("messages").is_none());

    let got = rpc(
        &client,
        &base,
        Some(&token),
        "loop.get",
        json!({ "loopId": loop_id }),
    )
    .await;
    assert_eq!(got["ok"]["status"], "stopped", "{got}");
    assert_eq!(got["ok"]["reason"], "max_iterations");
    assert_eq!(got["ok"]["iteration"], 2);

    let stop = rpc(
        &client,
        &base,
        Some(&token),
        "loop.stop",
        json!({ "loopId": loop_id }),
    )
    .await;
    assert_eq!(stop["ok"]["status"], "stopped");
    assert_eq!(stop["ok"]["reason"], "max_iterations");

    tokio::time::sleep(Duration::from_millis(200)).await;
    let ca = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get_context",
        json!({ "agentId": a }),
    )
    .await;
    let cb = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get_context",
        json!({ "agentId": b }),
    )
    .await;
    let users_a = ca["ok"]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == "user")
        .count();
    let users_b = cb["ok"]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == "user")
        .count();
    assert_eq!(
        users_a + users_b,
        2,
        "exactly two loop sends: a={ca} b={cb}"
    );

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn client_without_1_5_artifact_write_send_live_a2a_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let mut methods = f4_methods();
    if let Value::Object(map) = &mut methods {
        map.remove("policy.get");
        map.remove("policy.set");
        map.remove("approval.respond");
    }
    let (token, hs) = handshake(&client, &base, methods).await;
    assert!(hs["ok"]["accepted"].get("a2a.deliver").is_none());
    assert!(hs["ok"]["accepted"].get("artifact.create").is_some());
    assert!(hs["ok"]["accepted"].get("files.write").is_some());
    assert!(hs["ok"]["accepted"].get("agent.send").is_some());

    let proj = dir.path().join("proj");
    init_git_repo(&proj);
    let (ws_id, task_id) = seed_task(&client, &base, &token, &proj).await;
    let agent = create_agent(&client, &base, &token, &task_id, "cli.generic", None).await;
    let sent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.send",
        json!({ "agentId": agent, "content": "hi" }),
    )
    .await;
    assert!(sent.get("error").is_none(), "{sent}");

    let art = rpc(
        &client,
        &base,
        Some(&token),
        "artifact.create",
        json!({
            "taskId": task_id,
            "kind": "spec",
            "title": "Live",
            "body": ""
        }),
    )
    .await;
    assert!(art.get("error").is_none(), "{art}");

    let write = rpc(
        &client,
        &base,
        Some(&token),
        "files.write",
        json!({
            "workspaceId": ws_id,
            "agentId": agent,
            "path": "note.txt",
            "content": "x\n"
        }),
    )
    .await;
    assert!(write.get("error").is_none(), "{write}");

    let del = rpc(
        &client,
        &base,
        Some(&token),
        "a2a.deliver",
        json!({
            "fromAgentId": agent,
            "toAgentId": agent,
            "content": "x"
        }),
    )
    .await;
    assert_eq!(del["error"]["code"], "version_mismatch", "{del}");

    let _ = tx.send(());
    let _ = join.await;
}

#[test]
fn migration_0006_only_loops_0001_0005_untouched() {
    let sql6 = include_str!("../../rt-storage/migrations/0006_loops.sql");
    assert!(sql6.contains("CREATE TABLE loops"));
    assert!(!sql6.contains("CREATE TABLE artifacts"));
    assert!(!sql6.contains("ON DELETE CASCADE"));
    assert!(!sql6.to_ascii_lowercase().contains("insert into artifacts"));
    let creates = sql6.matches("CREATE TABLE").count();
    assert_eq!(creates, 1, "0006 must only create loops");

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../rt-storage/migrations");
    for name in [
        "0001_init.sql",
        "0002_worktrees.sql",
        "0003_policies.sql",
        "0004_terminal.sql",
        "0005_artifacts.sql",
    ] {
        let disk = std::fs::read(root.join(name)).unwrap();
        let git = Command::new("git")
            .args([
                "show",
                &format!("ae3bb96:crates/rt-storage/migrations/{name}"),
            ])
            .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .output()
            .expect("git show");
        assert!(
            git.status.success(),
            "git show {name}: {}",
            String::from_utf8_lossy(&git.stderr)
        );
        assert_eq!(disk, git.stdout, "{name} changed vs ae3bb96");
        let text = String::from_utf8_lossy(&disk);
        assert!(
            !text.contains("CREATE TABLE loops"),
            "{name} must not define loops"
        );
    }
}

#[tokio::test]
async fn deliver_cross_host_no_extra_message() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f5_methods()).await;
    let proj = dir.path().join("proj");
    let (_ws, task_id) = seed_task(&client, &base, &token, &proj).await;
    let from = create_agent(&client, &base, &token, &task_id, "cli.claude", None).await;
    let to = create_agent(&client, &base, &token, &task_id, "cli.claude", None).await;

    let conn = rusqlite::Connection::open(dir.path().join("host.db")).unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    conn.execute(
        "INSERT INTO host (id, name, created_at) VALUES ('other-host', 'other', '2026-08-19T00:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "UPDATE agents SET host_id = 'other-host' WHERE id = ?1",
        [&to],
    )
    .unwrap();

    let before = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get_context",
        json!({ "agentId": to }),
    )
    .await;
    let n_before = before["ok"]["messages"].as_array().unwrap().len();

    let err = rpc(
        &client,
        &base,
        Some(&token),
        "a2a.deliver",
        json!({
            "fromAgentId": from,
            "toAgentId": to,
            "content": "cross"
        }),
    )
    .await;
    assert_eq!(err["error"]["code"], "cross_host", "{err}");

    let after = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get_context",
        json!({ "agentId": to }),
    )
    .await;
    assert_eq!(
        after["ok"]["messages"].as_array().unwrap().len(),
        n_before,
        "cross_host must not queue a message: {after}"
    );

    let _ = tx.send(());
    let _ = join.await;
}
