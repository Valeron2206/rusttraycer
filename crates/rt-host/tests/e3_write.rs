//! E3 write path acceptance (protocol 1.2). C27–C31 host; C64 not here.

use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Command;
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
    std::fs::create_dir_all(path.join("src")).unwrap();
    std::fs::write(path.join("src").join("lib.rs"), "fn lib() {}\n").unwrap();
    git(path, &["add", "README.md", "src/lib.rs"]);
    git(path, &["commit", "-m", "init"]);
}

async fn seed_agent(
    client: &reqwest::Client,
    base: &str,
    token: &str,
    proj: &Path,
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
        json!({ "title": "write", "workspaceId": ws_id }),
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

fn host_db(dir: &Path) -> PathBuf {
    dir.join("host.db")
}

fn assert_no_secret_columns(db: &Path) {
    let conn = rusqlite::Connection::open(db).unwrap();
    let schema: String = conn
        .query_row(
            "SELECT value FROM schema_meta WHERE key = 'schema'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        schema, "6",
        "0006 is current; no secret tables, schema={schema}"
    );
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .unwrap();
    let tables: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    let allowed = [
        "agents",
        "host",
        "messages",
        "policies",
        "schema_meta",
        "sqlite_sequence",
        "task_workspaces",
        "tasks",
        "workspaces",
        "worktrees",
        "artifacts",
        "comment_threads",
        "comments",
        "loops",
    ];
    for t in &tables {
        assert!(
            allowed.contains(&t.as_str()),
            "unexpected table {t}: {tables:?}"
        );
        let mut info = conn
            .prepare(&format!("PRAGMA table_info(\"{t}\")"))
            .unwrap();
        let cols: Vec<String> = info
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        for c in cols {
            let low = c.to_ascii_lowercase();
            assert!(
                low != "token" && low != "pat" && low != "password",
                "secret-like column {c} on {t}"
            );
        }
    }
}

#[tokio::test]
async fn write_ask_emits_edit_approval_deny_leaves_file_absent() {
    let dir = tempfile::tempdir().unwrap();
    let (_starts, backends) = counting();
    let (addr, shutdown, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, hs) = handshake(&client, &base, f2_methods()).await;
    assert_eq!(hs["ok"]["accepted"]["files.write"]["minor"], 2);
    assert!(!hs["ok"]["accepted"]
        .as_object()
        .unwrap()
        .contains_key("handshake"));

    let proj = dir.path().join("proj");
    init_git_repo(&proj);
    let (ws_id, task_id, agent_id) = seed_agent(&client, &base, &token, &proj).await;
    let mut ws = connect_ws(addr, &token, &task_id).await;

    let dest = proj.join("src").join("new.rs");
    assert!(!dest.exists());
    let sent = rpc(
        &client,
        &base,
        Some(&token),
        "files.write",
        json!({
            "workspaceId": ws_id,
            "agentId": agent_id,
            "path": "src/new.rs",
            "content": "fn n() {}\\n"
        }),
    )
    .await;
    let approval_id = sent["ok"]["approvalId"]
        .as_str()
        .unwrap_or_else(|| panic!("approvalId missing: {sent}"))
        .to_string();
    assert!(!dest.exists(), "ask must not write yet");

    let ev = wait_event(&mut ws, "agent.approval").await;
    assert_eq!(ev["approvalId"], approval_id);
    assert_eq!(ev["agentId"], agent_id);
    assert_eq!(ev["kind"], "edit");
    assert_eq!(ev["summary"], "write src/new.rs");

    let busy = rpc(
        &client,
        &base,
        Some(&token),
        "files.write",
        json!({
            "workspaceId": ws_id,
            "agentId": agent_id,
            "path": "src/other.rs",
            "content": "x"
        }),
    )
    .await;
    assert_eq!(busy["error"]["code"], "agent_busy");
    let send_busy = rpc(
        &client,
        &base,
        Some(&token),
        "agent.send",
        json!({ "agentId": agent_id, "content": "hi" }),
    )
    .await;
    assert_eq!(send_busy["error"]["code"], "agent_busy");

    let deny = rpc(
        &client,
        &base,
        Some(&token),
        "approval.respond",
        json!({ "approvalId": approval_id, "decision": "deny" }),
    )
    .await;
    assert_eq!(deny["ok"]["applied"], true);
    assert!(!dest.exists(), "deny must leave file absent");

    let _ = shutdown.send(());
    let _ = join.await;
}

#[tokio::test]
async fn write_ask_allow_once_creates_file_mode_stays_ask() {
    let dir = tempfile::tempdir().unwrap();
    let (_starts, backends) = counting();
    let (addr, shutdown, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f2_methods()).await;
    let proj = dir.path().join("proj");
    init_git_repo(&proj);
    let (ws_id, task_id, agent_id) = seed_agent(&client, &base, &token, &proj).await;
    let mut ws = connect_ws(addr, &token, &task_id).await;

    let sent = rpc(
        &client,
        &base,
        Some(&token),
        "files.write",
        json!({
            "workspaceId": ws_id,
            "agentId": agent_id,
            "path": "src/lib.rs",
            "content": "pub fn ok() {}\\n"
        }),
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
    assert_eq!(
        std::fs::read_to_string(proj.join("src").join("lib.rs")).unwrap(),
        "pub fn ok() {}\\n"
    );

    let policy = rpc(
        &client,
        &base,
        Some(&token),
        "policy.get",
        json!({ "agentId": agent_id }),
    )
    .await;
    assert_eq!(policy["ok"]["mode"], "ask");

    let again = rpc(
        &client,
        &base,
        Some(&token),
        "approval.respond",
        json!({ "approvalId": approval_id, "decision": "allow-once" }),
    )
    .await;
    assert_eq!(again["ok"]["applied"], false);

    let _ = shutdown.send(());
    let _ = join.await;
}

#[tokio::test]
async fn write_yolo_or_allow_always_no_card() {
    let dir = tempfile::tempdir().unwrap();
    let (_starts, backends) = counting();
    let (addr, shutdown, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f2_methods()).await;
    let proj = dir.path().join("proj");
    init_git_repo(&proj);
    let (ws_id, task_id, agent_id) = seed_agent(&client, &base, &token, &proj).await;
    let mut ws = connect_ws(addr, &token, &task_id).await;

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

    let wrote = rpc(
        &client,
        &base,
        Some(&token),
        "files.write",
        json!({
            "workspaceId": ws_id,
            "agentId": agent_id,
            "path": "src/lib.rs",
            "content": "always\\n"
        }),
    )
    .await;
    assert_eq!(wrote["ok"]["path"], "src/lib.rs");
    assert!(wrote["ok"]["approvalId"].is_null());
    assert_eq!(
        std::fs::read_to_string(proj.join("src").join("lib.rs")).unwrap(),
        "always\\n"
    );
    no_event(&mut ws, "agent.approval", Duration::from_millis(200)).await;

    let yolo = rpc(
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
    assert_eq!(yolo["ok"]["yolo"], true);
    let wrote = rpc(
        &client,
        &base,
        Some(&token),
        "files.write",
        json!({
            "workspaceId": ws_id,
            "agentId": agent_id,
            "path": "src/yolo.rs",
            "content": "yolo\\n"
        }),
    )
    .await;
    assert_eq!(wrote["ok"]["path"], "src/yolo.rs");
    assert!(proj.join("src").join("yolo.rs").exists());
    no_event(&mut ws, "agent.approval", Duration::from_millis(200)).await;

    let deny = rpc(
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
    assert_eq!(deny["ok"]["mode"], "deny");
    let blocked = rpc(
        &client,
        &base,
        Some(&token),
        "files.write",
        json!({
            "workspaceId": ws_id,
            "agentId": agent_id,
            "path": "src/denied.rs",
            "content": "nope"
        }),
    )
    .await;
    assert_eq!(blocked["error"]["code"], "denied");
    assert!(!proj.join("src").join("denied.rs").exists());

    let _ = shutdown.send(());
    let _ = join.await;
}

#[tokio::test]
async fn client_without_1_2_ro_git_lives_write_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let (_starts, backends) = counting();
    let (addr, shutdown, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, hs) = handshake(&client, &base, f1_methods()).await;
    assert!(hs["ok"]["accepted"]["git.status"].is_object());
    assert!(hs["ok"]["accepted"]
        .as_object()
        .unwrap()
        .get("files.write")
        .is_none());

    let proj = dir.path().join("proj");
    init_git_repo(&proj);
    let (ws_id, _task_id, agent_id) = seed_agent(&client, &base, &token, &proj).await;

    let status = rpc(
        &client,
        &base,
        Some(&token),
        "git.status",
        json!({ "workspaceId": ws_id }),
    )
    .await;
    assert!(status["ok"]["branch"].is_string(), "{status}");

    let write = rpc(
        &client,
        &base,
        Some(&token),
        "files.write",
        json!({
            "workspaceId": ws_id,
            "agentId": agent_id,
            "path": "src/lib.rs",
            "content": "x"
        }),
    )
    .await;
    assert_eq!(write["error"]["code"], "version_mismatch");

    let _ = shutdown.send(());
    let _ = join.await;
}

#[tokio::test]
async fn git_stage_then_commit_local_no_network_no_secret_columns() {
    let dir = tempfile::tempdir().unwrap();
    let (_starts, backends) = counting();
    let (addr, shutdown, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f2_methods()).await;
    let proj = dir.path().join("proj");
    init_git_repo(&proj);
    let (ws_id, _task_id, _agent_id) = seed_agent(&client, &base, &token, &proj).await;

    std::fs::write(proj.join("src").join("lib.rs"), "fn staged() {}\\n").unwrap();
    let staged = rpc(
        &client,
        &base,
        Some(&token),
        "git.stage",
        json!({ "workspaceId": ws_id, "paths": ["src/lib.rs"] }),
    )
    .await;
    assert_eq!(staged["ok"]["dirty"], true, "{staged}");
    assert!(
        staged["ok"]["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["path"] == "src/lib.rs"),
        "{staged}"
    );

    let committed = rpc(
        &client,
        &base,
        Some(&token),
        "git.commit",
        json!({ "workspaceId": ws_id, "message": "stage and commit" }),
    )
    .await;
    assert!(
        committed["ok"]["commit"].as_str().unwrap().len() >= 7,
        "{committed}"
    );
    assert_eq!(committed["ok"]["branch"], "main");

    let status = rpc(
        &client,
        &base,
        Some(&token),
        "git.status",
        json!({ "workspaceId": ws_id }),
    )
    .await;
    assert_eq!(status["ok"]["dirty"], false, "{status}");

    assert_no_secret_columns(&host_db(dir.path()));

    let _ = shutdown.send(());
    let _ = join.await;
}

#[tokio::test]
async fn git_push_system_git_auth_fail_is_git_auth() {
    let dir = tempfile::tempdir().unwrap();
    let (_starts, backends) = counting();
    let (addr, shutdown, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f2_methods()).await;
    let proj = dir.path().join("proj");
    init_git_repo(&proj);
    git(
        &proj,
        &["remote", "add", "origin", "https://127.0.0.1:1/denied.git"],
    );
    let (ws_id, _task_id, _agent_id) = seed_agent(&client, &base, &token, &proj).await;

    let pushed = rpc(
        &client,
        &base,
        Some(&token),
        "git.push",
        json!({ "workspaceId": ws_id }),
    )
    .await;
    let code = pushed["error"]["code"].as_str().unwrap_or("");
    assert!(
        code == "git_auth" || code == "git_conflict",
        "expected git_auth/git_conflict, got {pushed}"
    );
    let msg = pushed["error"]["message"].as_str().unwrap_or("");
    assert!(!msg.contains("://") || !msg.contains(":@"), "{msg}");
    assert!(!msg.contains("ghp_"), "{msg}");

    assert_no_secret_columns(&host_db(dir.path()));

    let _ = shutdown.send(());
    let _ = join.await;
}

#[tokio::test]
async fn git_restore_reverts_tracked_and_deletes_untracked() {
    let dir = tempfile::tempdir().unwrap();
    let (_starts, backends) = counting();
    let (addr, shutdown, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f2_methods()).await;
    let proj = dir.path().join("proj");
    init_git_repo(&proj);
    let (ws_id, _task_id, _agent_id) = seed_agent(&client, &base, &token, &proj).await;

    std::fs::write(proj.join("README.md"), "changed\n").unwrap();
    std::fs::write(proj.join("scratch.txt"), "tmp\n").unwrap();

    let restored = rpc(
        &client,
        &base,
        Some(&token),
        "git.restore",
        json!({ "workspaceId": ws_id, "paths": ["README.md", "scratch.txt"] }),
    )
    .await;
    assert!(restored["ok"]["entries"].is_array(), "{restored}");
    assert_eq!(
        std::fs::read_to_string(proj.join("README.md")).unwrap(),
        "hello\n"
    );
    assert!(!proj.join("scratch.txt").exists());

    let _ = shutdown.send(());
    let _ = join.await;
}

#[test]
fn git_push_argv_has_no_force() {
    let args = rt_host::worktree::push_args("origin", "main");
    assert_eq!(args, ["push", "origin", "main"]);
    for a in &args {
        assert!(!a.contains("force"), "{args:?}");
        assert_ne!(a.as_str(), "--mirror");
        assert_ne!(a.as_str(), "--tags");
        assert_ne!(a.as_str(), "--force-with-lease");
    }
}

#[tokio::test]
async fn files_write_parent_must_exist() {
    let dir = tempfile::tempdir().unwrap();
    let (_starts, backends) = counting();
    let (addr, shutdown, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f2_methods()).await;
    let proj = dir.path().join("proj");
    init_git_repo(&proj);
    let (ws_id, _task_id, agent_id) = seed_agent(&client, &base, &token, &proj).await;
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

    let err = rpc(
        &client,
        &base,
        Some(&token),
        "files.write",
        json!({
            "workspaceId": ws_id,
            "agentId": agent_id,
            "path": "no-such-dir/a.rs",
            "content": "x"
        }),
    )
    .await;
    assert_eq!(err["error"]["code"], "not_found", "{err}");
    assert!(!proj.join("no-such-dir").exists());

    let _ = shutdown.send(());
    let _ = join.await;
}

#[tokio::test]
async fn files_write_rejects_escape_and_binary() {
    let dir = tempfile::tempdir().unwrap();
    let (_starts, backends) = counting();
    let (addr, shutdown, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f2_methods()).await;
    let proj = dir.path().join("proj");
    init_git_repo(&proj);
    let (ws_id, _task_id, agent_id) = seed_agent(&client, &base, &token, &proj).await;
    rpc(
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

    let escape = rpc(
        &client,
        &base,
        Some(&token),
        "files.write",
        json!({
            "workspaceId": ws_id,
            "agentId": agent_id,
            "path": "../escape.rs",
            "content": "x"
        }),
    )
    .await;
    assert_eq!(escape["error"]["code"], "invalid_params", "{escape}");

    let abs = rpc(
        &client,
        &base,
        Some(&token),
        "files.write",
        json!({
            "workspaceId": ws_id,
            "agentId": agent_id,
            "path": "/etc/passwd",
            "content": "x"
        }),
    )
    .await;
    assert_eq!(abs["error"]["code"], "invalid_params", "{abs}");

    let bin = rpc(
        &client,
        &base,
        Some(&token),
        "files.write",
        json!({
            "workspaceId": ws_id,
            "agentId": agent_id,
            "path": "src/nul.rs",
            "content": "hello\u{0000}world"
        }),
    )
    .await;
    assert_eq!(bin["error"]["code"], "file_binary", "{bin}");
    assert!(!proj.join("src").join("nul.rs").exists());

    let _ = shutdown.send(());
    let _ = join.await;
}

#[tokio::test]
async fn files_patch_check_then_apply() {
    let dir = tempfile::tempdir().unwrap();
    let (_starts, backends) = counting();
    let (addr, shutdown, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f2_methods()).await;
    let proj = dir.path().join("proj");
    init_git_repo(&proj);
    let (ws_id, task_id, agent_id) = seed_agent(&client, &base, &token, &proj).await;
    rpc(
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

    let good = "diff --git a/README.md b/README.md\n--- a/README.md\n+++ b/README.md\n@@ -1 +1,2 @@\n hello\n+world\n";
    let ok = rpc(
        &client,
        &base,
        Some(&token),
        "files.patch",
        json!({
            "workspaceId": ws_id,
            "agentId": agent_id,
            "patch": good
        }),
    )
    .await;
    assert!(
        ok["ok"]["paths"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p == "README.md"),
        "{ok}"
    );
    assert!(ok["ok"]["hunks"].as_u64().unwrap() >= 1, "{ok}");
    assert_eq!(
        std::fs::read_to_string(proj.join("README.md")).unwrap(),
        "hello\nworld\n"
    );

    let before = std::fs::read_to_string(proj.join("README.md")).unwrap();
    let bad = "diff --git a/README.md b/README.md\n--- a/README.md\n+++ b/README.md\n@@ -1 +1,2 @@\n goodbye\n+nope\n";
    let fail = rpc(
        &client,
        &base,
        Some(&token),
        "files.patch",
        json!({
            "workspaceId": ws_id,
            "agentId": agent_id,
            "patch": bad
        }),
    )
    .await;
    assert_eq!(fail["error"]["code"], "patch_failed", "{fail}");
    assert_eq!(
        std::fs::read_to_string(proj.join("README.md")).unwrap(),
        before
    );

    let mut ws = connect_ws(addr, &token, &task_id).await;
    rpc(
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
    let pending = rpc(
        &client,
        &base,
        Some(&token),
        "files.patch",
        json!({
            "workspaceId": ws_id,
            "agentId": agent_id,
            "patch": good
        }),
    )
    .await;
    assert!(pending["ok"]["approvalId"].is_string(), "{pending}");
    let ev = wait_event(&mut ws, "agent.approval").await;
    assert_eq!(ev["kind"], "edit");
    assert!(ev["summary"].as_str().unwrap().starts_with("patch"));

    let _ = shutdown.send(());
    let _ = join.await;
}

#[test]
fn git_commit_without_identity_is_git_identity() {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _g = LOCK.lock().unwrap();
    let prev_global = std::env::var("GIT_CONFIG_GLOBAL").ok();
    let prev_system = std::env::var("GIT_CONFIG_SYSTEM").ok();
    let prev_nosys = std::env::var("GIT_CONFIG_NOSYSTEM").ok();
    std::env::set_var("GIT_CONFIG_GLOBAL", "/dev/null");
    std::env::set_var("GIT_CONFIG_SYSTEM", "/dev/null");
    std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");

    let dir = tempfile::tempdir().unwrap();
    let store = rt_storage::Store::open(dir.path().join("host.db")).unwrap();
    store.migrate().unwrap();
    let host_id = rt_storage::new_id();
    store.host_insert_if_absent(&host_id, "test").unwrap();
    let svc = rt_host::service::HostService::new(
        store,
        std::collections::HashMap::new(),
        host_id,
        dir.path().to_path_buf(),
        "http://127.0.0.1:0".into(),
        std::process::id(),
    );
    let proj = dir.path().join("proj");
    std::fs::create_dir_all(&proj).unwrap();
    git(&proj, &["init", "-b", "main"]);
    git(&proj, &["config", "commit.gpgsign", "false"]);
    std::fs::write(proj.join("README.md"), "hello\n").unwrap();
    git(
        &proj,
        &[
            "-c",
            "user.email=t@t.test",
            "-c",
            "user.name=t",
            "add",
            "README.md",
        ],
    );
    git(
        &proj,
        &[
            "-c",
            "user.email=t@t.test",
            "-c",
            "user.name=t",
            "commit",
            "-m",
            "init",
        ],
    );
    std::fs::write(proj.join("README.md"), "changed\n").unwrap();
    git(&proj, &["add", "README.md"]);
    let ws = svc.workspace_add(proj.to_str().unwrap()).unwrap();
    let err = svc
        .git_commit(&json!({ "workspaceId": ws.id, "message": "no identity" }))
        .unwrap_err();
    assert_eq!(err.code(), "git_identity");
    assert!(err.to_string().contains("git config user.email"), "{err}");

    match prev_global {
        Some(v) => std::env::set_var("GIT_CONFIG_GLOBAL", v),
        None => std::env::remove_var("GIT_CONFIG_GLOBAL"),
    }
    match prev_system {
        Some(v) => std::env::set_var("GIT_CONFIG_SYSTEM", v),
        None => std::env::remove_var("GIT_CONFIG_SYSTEM"),
    }
    match prev_nosys {
        Some(v) => std::env::set_var("GIT_CONFIG_NOSYSTEM", v),
        None => std::env::remove_var("GIT_CONFIG_NOSYSTEM"),
    }
}

#[tokio::test]
async fn files_open_not_under_ladder() {
    let dir = tempfile::tempdir().unwrap();
    let (_starts, backends) = counting();
    let (addr, shutdown, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f2_methods()).await;
    let proj = dir.path().join("proj");
    init_git_repo(&proj);
    let (ws_id, task_id, _agent_id) = seed_agent(&client, &base, &token, &proj).await;
    let mut ws = connect_ws(addr, &token, &task_id).await;

    let missing = rpc(
        &client,
        &base,
        Some(&token),
        "files.open",
        json!({ "workspaceId": ws_id, "path": "nope.rs" }),
    )
    .await;
    assert_eq!(missing["error"]["code"], "not_found", "{missing}");
    assert!(missing["ok"]["approvalId"].is_null());

    let escape = rpc(
        &client,
        &base,
        Some(&token),
        "files.open",
        json!({ "workspaceId": ws_id, "path": "../x" }),
    )
    .await;
    assert_eq!(escape["error"]["code"], "invalid_params", "{escape}");

    let dir_path = rpc(
        &client,
        &base,
        Some(&token),
        "files.open",
        json!({ "workspaceId": ws_id, "path": "src" }),
    )
    .await;
    assert_eq!(dir_path["error"]["code"], "invalid_params", "{dir_path}");

    let opened = rpc(
        &client,
        &base,
        Some(&token),
        "files.open",
        json!({ "workspaceId": ws_id, "path": "README.md" }),
    )
    .await;
    assert!(
        opened["ok"]["opened"] == true || opened["error"]["code"] == "internal",
        "{opened}"
    );
    assert!(opened["ok"].get("approvalId").is_none() || opened["ok"]["approvalId"].is_null());
    no_event(&mut ws, "agent.approval", Duration::from_millis(200)).await;

    let _ = shutdown.send(());
    let _ = join.await;
}
