//! E5 artifacts, comments, markdown export (protocol 1.4, storage 0005).

use std::path::Path;
use std::pin::Pin;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use futures::{SinkExt, Stream, StreamExt};
use rt_runtime::{AgentBackend, Availability, TurnEvent, TurnRequest};
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

fn backends() -> std::collections::HashMap<String, Arc<dyn AgentBackend>> {
    let mut m = std::collections::HashMap::new();
    m.insert(
        "cli.generic".into(),
        Arc::new(GenericBackend) as Arc<dyn AgentBackend>,
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

fn pdf_bytes(ok: &Value) -> Vec<u8> {
    for key in ["bytes", "content", "body", "markdown"] {
        if let Some(s) = ok.get(key).and_then(|v| v.as_str()) {
            if s.starts_with("%PDF") {
                return s.as_bytes().to_vec();
            }
            if let Ok(d) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, s) {
                if d.starts_with(b"%PDF") {
                    return d;
                }
            }
        }
    }
    Vec::new()
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
        json!({ "title": "e5", "workspaceId": ws_id }),
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

#[tokio::test]
async fn handshake_1_4_accepts_artifact_create() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (_token, hs) = handshake(&client, &base, f4_methods()).await;
    assert_eq!(hs["ok"]["accepted"]["artifact.create"]["minor"], 4);
    assert_eq!(hs["ok"]["accepted"]["comment.create"]["minor"], 4);
    assert_eq!(hs["ok"]["accepted"]["agent.clear_transcript"]["minor"], 4);
    assert!(hs["ok"]["rejected"].get("artifact.create").is_none());
    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn create_spec_child_ticket_list_get_update_delete_parent_agents_live() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f4_methods()).await;
    let proj = dir.path().join("proj");
    let (_ws_id, task_id) = seed_task(&client, &base, &token, &proj).await;
    let agent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.create",
        json!({ "taskId": task_id, "provider": "cli.generic" }),
    )
    .await;
    let agent_id = rpc_id(&agent, "id");

    let spec = rpc(
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
    assert!(spec.get("error").is_none(), "{spec}");
    let spec_id = rpc_id(&spec, "id");
    assert_eq!(spec["ok"]["kind"], "spec");
    assert!(spec["ok"]["status"].is_null());

    let ticket = rpc(
        &client,
        &base,
        Some(&token),
        "artifact.create",
        json!({
            "taskId": task_id,
            "parentId": spec_id,
            "kind": "ticket",
            "title": "Add login",
            "body": ""
        }),
    )
    .await;
    assert!(ticket.get("error").is_none(), "{ticket}");
    let ticket_id = rpc_id(&ticket, "id");
    assert_eq!(ticket["ok"]["status"], "todo");
    assert_eq!(ticket["ok"]["parentId"], spec_id);

    let listed = rpc(
        &client,
        &base,
        Some(&token),
        "artifact.list",
        json!({ "taskId": task_id }),
    )
    .await;
    assert_eq!(listed["ok"]["items"].as_array().unwrap().len(), 2);
    assert_eq!(listed["ok"]["truncated"], false);

    let got = rpc(
        &client,
        &base,
        Some(&token),
        "artifact.get",
        json!({ "artifactId": ticket_id }),
    )
    .await;
    assert_eq!(got["ok"]["title"], "Add login");

    let upd = rpc(
        &client,
        &base,
        Some(&token),
        "artifact.update",
        json!({ "artifactId": ticket_id, "status": "in_progress" }),
    )
    .await;
    assert_eq!(upd["ok"]["status"], "in_progress");

    let del = rpc(
        &client,
        &base,
        Some(&token),
        "artifact.delete",
        json!({ "artifactId": spec_id }),
    )
    .await;
    assert!(del.get("error").is_none(), "{del}");
    let deleted = del["ok"]["deleted"].as_array().unwrap();
    assert!(deleted.iter().any(|v| v.as_str() == Some(spec_id.as_str())));
    assert!(deleted
        .iter()
        .any(|v| v.as_str() == Some(ticket_id.as_str())));

    let child = rpc(
        &client,
        &base,
        Some(&token),
        "artifact.get",
        json!({ "artifactId": ticket_id }),
    )
    .await;
    assert_eq!(child["error"]["code"], "not_found");

    let agents = rpc(
        &client,
        &base,
        Some(&token),
        "agent.list",
        json!({ "taskId": task_id }),
    )
    .await;
    let items = agents["ok"]["items"].as_array().unwrap();
    assert!(items.iter().any(|a| a["id"] == agent_id), "{agents}");

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn comment_anchor_reply_resolve_survives_restart() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().to_path_buf();
    let (addr, tx, join, _) = rt_host::spawn_test_host(&data, Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f4_methods()).await;
    let proj = data.join("proj");
    let (_ws_id, task_id) = seed_task(&client, &base, &token, &proj).await;
    let spec = rpc(
        &client,
        &base,
        Some(&token),
        "artifact.create",
        json!({
            "taskId": task_id,
            "kind": "spec",
            "title": "Auth",
            "body": "hello world"
        }),
    )
    .await;
    let spec_id = rpc_id(&spec, "id");
    let created = rpc(
        &client,
        &base,
        Some(&token),
        "comment.create",
        json!({
            "artifactId": spec_id,
            "threadId": null,
            "anchorStart": 0,
            "anchorEnd": 5,
            "body": "nit"
        }),
    )
    .await;
    assert!(created.get("error").is_none(), "{created}");
    let thread_id = rpc_id(&created, "id");
    assert_eq!(created["ok"]["anchorStart"], 0);
    assert_eq!(created["ok"]["comments"].as_array().unwrap().len(), 1);

    let reply = rpc(
        &client,
        &base,
        Some(&token),
        "comment.create",
        json!({
            "artifactId": spec_id,
            "threadId": thread_id,
            "body": "reply"
        }),
    )
    .await;
    assert_eq!(reply["ok"]["comments"].as_array().unwrap().len(), 2);

    let resolved = rpc(
        &client,
        &base,
        Some(&token),
        "comment.resolve",
        json!({ "threadId": thread_id }),
    )
    .await;
    assert_eq!(resolved["ok"]["resolved"], true);

    let _ = tx.send(());
    let _ = join.await;

    let (addr, tx, join, _) = rt_host::spawn_test_host(&data, Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let (token, _) = handshake(&client, &base, f4_methods()).await;
    let listed = rpc(
        &client,
        &base,
        Some(&token),
        "comment.list",
        json!({ "artifactId": spec_id }),
    )
    .await;
    let threads = listed["ok"]["threads"].as_array().unwrap();
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0]["id"], thread_id);
    assert_eq!(threads[0]["resolved"], true);
    assert_eq!(threads[0]["comments"].as_array().unwrap().len(), 2);
    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn clear_transcript_unlinks_source_message_artifact_survives() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().to_path_buf();
    let (addr, tx, join, _) = rt_host::spawn_test_host(&data, Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f4_methods()).await;
    let proj = data.join("proj");
    let (_ws_id, task_id) = seed_task(&client, &base, &token, &proj).await;
    let agent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.create",
        json!({ "taskId": task_id, "provider": "cli.generic" }),
    )
    .await;
    let agent_id = rpc_id(&agent, "id");
    let sent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.send",
        json!({ "agentId": agent_id, "content": "please write a spec" }),
    )
    .await;
    assert!(sent.get("error").is_none(), "{sent}");
    let mid = sent["ok"]["userMessage"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let art = rpc(
        &client,
        &base,
        Some(&token),
        "artifact.create",
        json!({
            "taskId": task_id,
            "kind": "spec",
            "title": "From chat",
            "body": "# kept\n",
            "sourceMessageId": mid
        }),
    )
    .await;
    let art_id = rpc_id(&art, "id");
    assert_eq!(art["ok"]["sourceMessageId"], mid);

    let cleared = rpc(
        &client,
        &base,
        Some(&token),
        "agent.clear_transcript",
        json!({ "agentId": agent_id }),
    )
    .await;
    assert!(cleared.get("error").is_none(), "{cleared}");
    assert!(cleared["ok"]["cleared"].as_u64().unwrap() > 0);

    let ctx = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get_context",
        json!({ "agentId": agent_id }),
    )
    .await;
    assert_eq!(ctx["ok"]["messages"].as_array().unwrap().len(), 0);

    let got = rpc(
        &client,
        &base,
        Some(&token),
        "artifact.get",
        json!({ "artifactId": art_id }),
    )
    .await;
    assert_eq!(got["ok"]["body"], "# kept\n");
    assert!(got["ok"]["sourceMessageId"].is_null());

    let _ = tx.send(());
    let _ = join.await;

    let (addr, tx, join, _) = rt_host::spawn_test_host(&data, Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let (token, _) = handshake(&client, &base, f4_methods()).await;
    let got = rpc(
        &client,
        &base,
        Some(&token),
        "artifact.get",
        json!({ "artifactId": art_id }),
    )
    .await;
    assert_eq!(got["ok"]["body"], "# kept\n");
    assert_eq!(got["ok"]["title"], "From chat");
    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn export_md_ok_pdf_ok() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f4_methods()).await;
    let proj = dir.path().join("proj");
    let (_ws_id, task_id) = seed_task(&client, &base, &token, &proj).await;
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
    let art_id = rpc_id(&art, "id");

    let md = client
        .post(format!("{base}/rpc"))
        .header("X-Rt-Session", &token)
        .json(&json!({
            "id": "exp",
            "method": "artifact.export",
            "params": { "artifactId": art_id, "format": "md" }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(md.status().as_u16(), 200);
    let md: Value = md.json().await.unwrap();
    assert!(md.get("error").is_none(), "{md}");
    assert_eq!(md["ok"]["format"], "md");
    assert_eq!(md["ok"]["markdown"], "Auth\n\n# Auth\n");
    assert_eq!(md["ok"]["filename"], format!("{art_id}.md"));

    let pdf = client
        .post(format!("{base}/rpc"))
        .header("X-Rt-Session", &token)
        .json(&json!({
            "id": "pdf",
            "method": "artifact.export",
            "params": { "artifactId": art_id, "format": "pdf" }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(pdf.status().as_u16(), 200);
    let pdf: Value = pdf.json().await.unwrap();
    assert!(pdf.get("error").is_none(), "{pdf}");
    assert_eq!(pdf["ok"]["format"], "pdf");
    let raw = pdf_bytes(&pdf["ok"]);
    assert!(
        raw.starts_with(b"%PDF"),
        "pdf bytes must start with %PDF: {:?}",
        raw.get(..8)
    );
    assert_eq!(pdf["ok"]["filename"], format!("{art_id}.pdf"));

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn create_update_do_not_emit_agent_approval() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f4_methods()).await;
    let proj = dir.path().join("proj");
    let (_ws_id, task_id) = seed_task(&client, &base, &token, &proj).await;
    let mut ws = connect_ws(addr, &token, &task_id).await;

    let art = rpc(
        &client,
        &base,
        Some(&token),
        "artifact.create",
        json!({
            "taskId": task_id,
            "kind": "ticket",
            "title": "T",
            "body": "b"
        }),
    )
    .await;
    let art_id = rpc_id(&art, "id");
    let ev = wait_event(&mut ws, "artifact.updated").await;
    assert_eq!(ev["artifactId"], art_id);
    assert_eq!(ev["taskId"], task_id);
    assert!(ev.get("body").is_none());

    let _ = rpc(
        &client,
        &base,
        Some(&token),
        "artifact.update",
        json!({ "artifactId": art_id, "status": "done" }),
    )
    .await;
    let ev = wait_event(&mut ws, "artifact.updated").await;
    assert_eq!(ev["artifactId"], art_id);
    assert!(ev.get("body").is_none());

    no_event(&mut ws, "agent.approval", Duration::from_millis(200)).await;

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn client_without_1_4_send_git_live_artifact_version_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, hs) = handshake(&client, &base, f3_methods()).await;
    assert!(hs["ok"]["accepted"].get("artifact.create").is_none());
    assert!(hs["ok"]["accepted"].get("agent.send").is_some());
    assert!(hs["ok"]["accepted"].get("git.status").is_some());
    assert!(hs["ok"]["accepted"].get("pty.open").is_some());

    let proj = dir.path().join("proj");
    init_git_repo(&proj);
    let (ws_id, task_id) = seed_task(&client, &base, &token, &proj).await;
    let agent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.create",
        json!({ "taskId": task_id, "provider": "cli.generic" }),
    )
    .await;
    let agent_id = rpc_id(&agent, "id");
    let sent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.send",
        json!({ "agentId": agent_id, "content": "hi" }),
    )
    .await;
    assert!(sent.get("error").is_none(), "{sent}");

    let st = rpc(
        &client,
        &base,
        Some(&token),
        "git.status",
        json!({ "workspaceId": ws_id }),
    )
    .await;
    assert!(st.get("error").is_none(), "{st}");
    assert_eq!(st["ok"]["branch"], "main");

    let create = rpc(
        &client,
        &base,
        Some(&token),
        "artifact.create",
        json!({
            "taskId": task_id,
            "kind": "spec",
            "title": "Nope",
            "body": ""
        }),
    )
    .await;
    assert_eq!(create["error"]["code"], "version_mismatch", "{create}");

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn migration_0005_no_cascade_no_md_on_disk() {
    let sql = include_str!("../../rt-storage/migrations/0005_artifacts.sql");
    assert!(sql.contains("CREATE TABLE artifacts"));
    assert!(!sql.contains("ON DELETE CASCADE"));
    assert!(!sql.contains("REFERENCES messages"));

    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f4_methods()).await;
    let proj = dir.path().join("proj");
    let (_ws_id, task_id) = seed_task(&client, &base, &token, &proj).await;
    let art = rpc(
        &client,
        &base,
        Some(&token),
        "artifact.create",
        json!({
            "taskId": task_id,
            "kind": "spec",
            "title": "Auth",
            "body": "only in sqlite"
        }),
    )
    .await;
    let art_id = rpc_id(&art, "id");
    let _ = rpc(
        &client,
        &base,
        Some(&token),
        "artifact.export",
        json!({ "artifactId": art_id, "format": "md" }),
    )
    .await;

    let leaked = format!("{art_id}.md");
    let mut found = Vec::new();
    for entry in walkdir_md(dir.path()) {
        if entry.file_name().map(|n| n.to_string_lossy())
            == Some(std::borrow::Cow::from(leaked.as_str()))
        {
            found.push(entry);
        }
    }
    assert!(
        found.is_empty(),
        "export must not write {leaked}: {found:?}"
    );

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
    let (token, _) = handshake(&client, &base, f4_methods()).await;
    let proj = dir.path().join("proj");
    let (_ws_id, task_id) = seed_task(&client, &base, &token, &proj).await;
    let a = rpc(
        &client,
        &base,
        Some(&token),
        "artifact.create",
        json!({ "taskId": task_id, "kind": "spec", "title": "A", "body": "" }),
    )
    .await;
    let a_id = rpc_id(&a, "id");
    let b = rpc(
        &client,
        &base,
        Some(&token),
        "artifact.create",
        json!({
            "taskId": task_id,
            "parentId": a_id,
            "kind": "ticket",
            "title": "B",
            "body": ""
        }),
    )
    .await;
    let b_id = rpc_id(&b, "id");
    let cycle = rpc(
        &client,
        &base,
        Some(&token),
        "artifact.update",
        json!({ "artifactId": a_id, "parentId": b_id }),
    )
    .await;
    assert_eq!(cycle["error"]["code"], "invalid_params", "{cycle}");
    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn spec_plus_status_invalid_params() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f4_methods()).await;
    let proj = dir.path().join("proj");
    let (_ws_id, task_id) = seed_task(&client, &base, &token, &proj).await;
    let spec = rpc(
        &client,
        &base,
        Some(&token),
        "artifact.create",
        json!({ "taskId": task_id, "kind": "spec", "title": "S", "body": "" }),
    )
    .await;
    let spec_id = rpc_id(&spec, "id");
    let bad = rpc(
        &client,
        &base,
        Some(&token),
        "artifact.update",
        json!({ "artifactId": spec_id, "status": "todo" }),
    )
    .await;
    assert_eq!(bad["error"]["code"], "invalid_params", "{bad}");
    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn four_kinds_on_one_task() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f4_methods()).await;
    let proj = dir.path().join("proj");
    let (_ws_id, task_id) = seed_task(&client, &base, &token, &proj).await;
    for (kind, title) in [
        ("spec", "S"),
        ("ticket", "T"),
        ("story", "Y"),
        ("review", "R"),
    ] {
        let r = rpc(
            &client,
            &base,
            Some(&token),
            "artifact.create",
            json!({ "taskId": task_id, "kind": kind, "title": title, "body": "" }),
        )
        .await;
        assert!(r.get("error").is_none(), "{kind}: {r}");
        assert_eq!(r["ok"]["kind"], kind);
    }
    let listed = rpc(
        &client,
        &base,
        Some(&token),
        "artifact.list",
        json!({ "taskId": task_id }),
    )
    .await;
    assert_eq!(listed["ok"]["items"].as_array().unwrap().len(), 4);
    let _ = tx.send(());
    let _ = join.await;
}

fn walkdir_md(root: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for ent in rd.flatten() {
            let p = ent.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().and_then(|e| e.to_str()) == Some("md") {
                out.push(p);
            }
        }
    }
    out
}
