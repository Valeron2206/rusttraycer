//! E8 workspace guides, roles, and presets (protocol 1.7, storage 0008).

use std::path::Path;
use std::pin::Pin;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::Stream;
use rt_runtime::{AgentBackend, Availability, HarnessCaps, TurnEvent, TurnRequest};
use serde_json::{json, Value};

#[derive(Clone)]
struct CaptureGeneric {
    last: Arc<Mutex<Option<TurnRequest>>>,
}

impl AgentBackend for CaptureGeneric {
    fn id(&self) -> &'static str {
        "cli.generic"
    }
    fn available(&self) -> Availability {
        Availability {
            available: true,
            detail: "capture generic".into(),
        }
    }
    fn caps(&self) -> HarnessCaps {
        HarnessCaps::CLI_GENERIC
    }
    fn start_turn(&self, req: TurnRequest) -> Pin<Box<dyn Stream<Item = TurnEvent> + Send>> {
        if let Ok(mut g) = self.last.lock() {
            *g = Some(req);
        }
        Box::pin(futures::stream::iter([
            TurnEvent::Token {
                text: "ok\n".into(),
            },
            TurnEvent::Finished { exit_code: 0 },
        ]))
    }
}

struct InstantClaude;

impl AgentBackend for InstantClaude {
    fn id(&self) -> &'static str {
        "cli.claude"
    }
    fn available(&self) -> Availability {
        Availability {
            available: true,
            detail: "claude mock".into(),
        }
    }
    fn caps(&self) -> HarnessCaps {
        HarnessCaps::CLI_CLAUDE
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

fn f5_methods() -> Value {
    let mut m = v1_methods();
    if let Value::Object(map) = &mut m {
        map.insert("agent.create".into(), json!({ "major": 1, "minor": 5 }));
        map.insert("policy.get".into(), json!({ "major": 1, "minor": 1 }));
        map.insert("policy.set".into(), json!({ "major": 1, "minor": 1 }));
        map.insert("approval.respond".into(), json!({ "major": 1, "minor": 1 }));
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

fn f6_methods() -> Value {
    let mut m = f5_methods();
    if let Value::Object(map) = &mut m {
        for n in [
            "agent.switch",
            "profile.create",
            "profile.list",
            "profile.get",
            "profile.update",
            "profile.delete",
            "prefs.get",
        ] {
            map.insert(n.into(), json!({ "major": 1, "minor": 6 }));
        }
    }
    m
}

fn f7_methods() -> Value {
    let mut m = f6_methods();
    if let Value::Object(map) = &mut m {
        for n in [
            "workspace.guides.get",
            "settings.guide.get",
            "settings.guide.set",
            "preset.list",
            "agent.update",
        ] {
            map.insert(n.into(), json!({ "major": 1, "minor": 7 }));
        }
    }
    m
}

fn f7_run_methods() -> Value {
    let mut m = f7_methods();
    if let Value::Object(map) = &mut m {
        map.remove("policy.get");
        map.remove("policy.set");
        map.remove("approval.respond");
    }
    m
}

fn f6_run_methods() -> Value {
    let mut m = f6_methods();
    if let Value::Object(map) = &mut m {
        map.remove("policy.get");
        map.remove("policy.set");
        map.remove("approval.respond");
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

async fn wait_status(
    client: &reqwest::Client,
    base: &str,
    token: &str,
    agent_id: &str,
    want: &str,
) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
    loop {
        let got = rpc(
            client,
            base,
            Some(token),
            "agent.get",
            json!({ "id": agent_id }),
        )
        .await;
        if got["ok"]["status"] == want {
            return;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("status {want} not reached: {got}");
        }
        tokio::time::sleep(Duration::from_millis(40)).await;
    }
}

fn backends(
    last: Arc<Mutex<Option<TurnRequest>>>,
) -> std::collections::HashMap<String, Arc<dyn AgentBackend>> {
    let mut m = std::collections::HashMap::new();
    m.insert(
        "cli.generic".into(),
        Arc::new(CaptureGeneric { last }) as Arc<dyn AgentBackend>,
    );
    m.insert(
        "cli.claude".into(),
        Arc::new(InstantClaude) as Arc<dyn AgentBackend>,
    );
    m
}

async fn seed_ws_task(
    client: &reqwest::Client,
    base: &str,
    token: &str,
    proj: &Path,
    title: &str,
    preset: Option<&str>,
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
    let mut p = json!({ "title": title, "workspaceId": ws_id });
    if let Some(preset) = preset {
        p["preset"] = json!(preset);
    }
    let task = rpc(client, base, Some(token), "task.create", p).await;
    assert!(task.get("error").is_none(), "{task}");
    (ws_id, rpc_id(&task, "id"))
}

fn last_contents(last: &Arc<Mutex<Option<TurnRequest>>>) -> Vec<String> {
    let g = last.lock().unwrap();
    let req = g.as_ref().unwrap_or_else(|| panic!("no turn captured"));
    req.messages.iter().map(|m| m.content.clone()).collect()
}

#[tokio::test]
async fn handshake_new_methods_1_7() {
    let dir = tempfile::tempdir().unwrap();
    let last = Arc::new(Mutex::new(None));
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends(last)))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (_token, hs) = handshake(&client, &base, f7_methods()).await;
    assert_eq!(hs["ok"]["accepted"]["workspace.guides.get"]["minor"], 7);
    assert_eq!(hs["ok"]["accepted"]["settings.guide.get"]["minor"], 7);
    assert_eq!(hs["ok"]["accepted"]["settings.guide.set"]["minor"], 7);
    assert_eq!(hs["ok"]["accepted"]["preset.list"]["minor"], 7);
    assert_eq!(hs["ok"]["accepted"]["agent.update"]["minor"], 7);
    assert_eq!(hs["ok"]["accepted"]["agent.switch"]["minor"], 6);
    assert_eq!(hs["ok"]["accepted"]["agent.create"]["minor"], 5);
    assert_eq!(hs["ok"]["accepted"]["task.create"]["minor"], 0);
    assert_eq!(hs["ok"]["accepted"]["host.ping"]["minor"], 0);
    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn agents_md_in_turn_not_in_get_context() {
    let dir = tempfile::tempdir().unwrap();
    let last = Arc::new(Mutex::new(None));
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends(last.clone())))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f7_run_methods()).await;
    let proj = dir.path().join("proj");
    std::fs::write(proj.join("AGENTS.md"), "ROOT_AGENTS_MD_MARKER").ok();
    std::fs::create_dir_all(&proj).unwrap();
    std::fs::write(proj.join("AGENTS.md"), "ROOT_AGENTS_MD_MARKER").unwrap();
    let (_ws, task_id) = seed_ws_task(&client, &base, &token, &proj, "e8", None).await;
    let created = rpc(
        &client,
        &base,
        Some(&token),
        "agent.create",
        json!({ "taskId": task_id, "provider": "cli.generic" }),
    )
    .await;
    let agent_id = rpc_id(&created, "id");
    let sent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.send",
        json!({ "agentId": agent_id, "content": "hello" }),
    )
    .await;
    assert!(sent.get("error").is_none(), "{sent}");
    wait_status(&client, &base, &token, &agent_id, "idle").await;
    let contents = last_contents(&last);
    assert!(
        contents.iter().any(|c| c.contains("ROOT_AGENTS_MD_MARKER")),
        "preamble missing: {contents:?}"
    );
    let ctx = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get_context",
        json!({ "agentId": agent_id }),
    )
    .await;
    let msgs = ctx["ok"]["messages"].as_array().unwrap();
    for m in msgs {
        let text = m["content"].as_str().unwrap_or("");
        assert!(
            !text.contains("ROOT_AGENTS_MD_MARKER"),
            "guide leaked into get_context: {ctx}"
        );
    }
    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn missing_agents_md_send_lives_guides_null() {
    let dir = tempfile::tempdir().unwrap();
    let last = Arc::new(Mutex::new(None));
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends(last)))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f7_run_methods()).await;
    let proj = dir.path().join("proj");
    let (ws_id, task_id) = seed_ws_task(&client, &base, &token, &proj, "e8", None).await;
    let created = rpc(
        &client,
        &base,
        Some(&token),
        "agent.create",
        json!({ "taskId": task_id, "provider": "cli.generic" }),
    )
    .await;
    let agent_id = rpc_id(&created, "id");
    let sent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.send",
        json!({ "agentId": agent_id, "content": "hello" }),
    )
    .await;
    assert!(sent.get("error").is_none(), "{sent}");
    wait_status(&client, &base, &token, &agent_id, "idle").await;
    let guides = rpc(
        &client,
        &base,
        Some(&token),
        "workspace.guides.get",
        json!({ "workspaceId": ws_id }),
    )
    .await;
    assert!(guides.get("error").is_none(), "{guides}");
    assert!(guides["ok"]["agentsMd"].is_null(), "{guides}");
    let missing = rpc(
        &client,
        &base,
        Some(&token),
        "workspace.guides.get",
        json!({ "workspaceId": "no-such-ws" }),
    )
    .await;
    assert_eq!(missing["error"]["code"], "not_found", "{missing}");
    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn settings_guide_set_writes_file_not_sqlite() {
    let dir = tempfile::tempdir().unwrap();
    let last = Arc::new(Mutex::new(None));
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends(last)))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f7_run_methods()).await;
    let marker = "E8_GLOBAL_GUIDE_UNIQUE_MARKER";
    let set = rpc(
        &client,
        &base,
        Some(&token),
        "settings.guide.set",
        json!({ "content": marker }),
    )
    .await;
    assert!(set.get("error").is_none(), "{set}");
    let path = dir.path().join("agent-selection-guide.md");
    assert!(path.exists(), "global guide missing next to host.db");
    let on_disk = std::fs::read_to_string(&path).unwrap();
    assert_eq!(on_disk, marker);
    let db = dir.path().join("host.db");
    let conn = rusqlite::Connection::open(&db).unwrap();
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = ?1")
        .unwrap();
    let tables: Vec<String> = stmt
        .query_map(["table"], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    for t in &tables {
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
            assert!(!low.contains("guide"), "guide column {c} on {t}");
            assert_ne!(low, "agents_md");
            assert_ne!(low, "phase");
            assert_ne!(low, "epic");
        }
    }
    let blob: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE sql LIKE ?1",
            [format!("%{marker}%")],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(blob, 0, "guide marker leaked into sqlite schema");
    let get = rpc(
        &client,
        &base,
        Some(&token),
        "settings.guide.get",
        json!({}),
    )
    .await;
    assert_eq!(get["ok"]["content"], marker);
    let empty = rpc(
        &client,
        &base,
        Some(&token),
        "settings.guide.set",
        json!({ "content": "" }),
    )
    .await;
    assert!(empty.get("error").is_none(), "{empty}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "");
    assert!(path.exists());
    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn both_guides_inject_workspace_after_global() {
    let dir = tempfile::tempdir().unwrap();
    let last = Arc::new(Mutex::new(None));
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends(last.clone())))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f7_run_methods()).await;
    std::fs::write(
        dir.path().join("agent-selection-guide.md"),
        "GLOBAL_GUIDE_MARKER",
    )
    .unwrap();
    let proj = dir.path().join("proj");
    std::fs::create_dir_all(proj.join(".traycer")).unwrap();
    std::fs::write(proj.join("AGENTS.md"), "ROOT_AGENTS_MD_MARKER").unwrap();
    std::fs::write(
        proj.join(".traycer/agent-selection-guide.md"),
        "WS_GUIDE_MARKER",
    )
    .unwrap();
    let (_ws, task_id) = seed_ws_task(&client, &base, &token, &proj, "e8", None).await;
    let created = rpc(
        &client,
        &base,
        Some(&token),
        "agent.create",
        json!({ "taskId": task_id, "provider": "cli.generic" }),
    )
    .await;
    let agent_id = rpc_id(&created, "id");
    let sent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.send",
        json!({ "agentId": agent_id, "content": "hello" }),
    )
    .await;
    assert!(sent.get("error").is_none(), "{sent}");
    wait_status(&client, &base, &token, &agent_id, "idle").await;
    let contents = last_contents(&last);
    let joined = contents.join("\n---\n");
    let agents_at = joined.find("ROOT_AGENTS_MD_MARKER").expect(&joined);
    let global_at = joined.find("GLOBAL_GUIDE_MARKER").expect(&joined);
    let ws_at = joined.find("WS_GUIDE_MARKER").expect(&joined);
    assert!(agents_at < global_at, "{joined}");
    assert!(global_at < ws_at, "{joined}");
    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn task_create_preset_planning() {
    let dir = tempfile::tempdir().unwrap();
    let last = Arc::new(Mutex::new(None));
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends(last)))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f7_run_methods()).await;
    let proj = dir.path().join("proj");
    let (_ws, task_id) =
        seed_ws_task(&client, &base, &token, &proj, "plan", Some("planning")).await;
    let got = rpc(
        &client,
        &base,
        Some(&token),
        "task.get",
        json!({ "id": task_id }),
    )
    .await;
    assert_eq!(got["ok"]["preset"], "planning", "{got}");
    let bad = rpc(
        &client,
        &base,
        Some(&token),
        "task.create",
        json!({
            "title": "x",
            "workspaceId": _ws,
            "preset": "kanban"
        }),
    )
    .await;
    assert_eq!(bad["error"]["code"], "invalid_params", "{bad}");
    let presets = rpc(&client, &base, Some(&token), "preset.list", json!({})).await;
    let ids: Vec<&str> = presets["ok"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["planning", "review", "debug", "document"]);
    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn agent_create_inherits_planner_update_keeps_id() {
    let dir = tempfile::tempdir().unwrap();
    let last = Arc::new(Mutex::new(None));
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends(last)))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f7_run_methods()).await;
    let proj = dir.path().join("proj");
    let (_ws, task_id) =
        seed_ws_task(&client, &base, &token, &proj, "plan", Some("planning")).await;
    let created = rpc(
        &client,
        &base,
        Some(&token),
        "agent.create",
        json!({ "taskId": task_id, "provider": "cli.generic" }),
    )
    .await;
    assert_eq!(created["ok"]["role"], "planner", "{created}");
    let agent_id = rpc_id(&created, "id");
    let sent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.send",
        json!({ "agentId": agent_id, "content": "hello" }),
    )
    .await;
    assert!(sent.get("error").is_none(), "{sent}");
    wait_status(&client, &base, &token, &agent_id, "idle").await;
    let before = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get_context",
        json!({ "agentId": agent_id }),
    )
    .await;
    let n_before = before["ok"]["messages"].as_array().unwrap().len();
    assert!(n_before >= 1, "{before}");
    let upd = rpc(
        &client,
        &base,
        Some(&token),
        "agent.update",
        json!({ "agentId": agent_id, "role": "reviewer" }),
    )
    .await;
    assert!(upd.get("error").is_none(), "{upd}");
    assert_eq!(upd["ok"]["id"], agent_id);
    assert_eq!(upd["ok"]["role"], "reviewer");
    assert_eq!(upd["ok"]["provider"], "cli.generic");
    let after = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get_context",
        json!({ "agentId": agent_id }),
    )
    .await;
    assert_eq!(
        after["ok"]["messages"].as_array().unwrap().len(),
        n_before,
        "{after}"
    );
    let bad = rpc(
        &client,
        &base,
        Some(&token),
        "agent.create",
        json!({ "taskId": task_id, "role": "wizard" }),
    )
    .await;
    assert_eq!(bad["error"]["code"], "invalid_params", "{bad}");
    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn client_without_1_7_create_send_context_switch_live() {
    let dir = tempfile::tempdir().unwrap();
    let last = Arc::new(Mutex::new(None));
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends(last)))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, hs) = handshake(&client, &base, f6_run_methods()).await;
    assert!(hs["ok"]["accepted"].get("workspace.guides.get").is_none());
    assert!(hs["ok"]["accepted"].get("agent.send").is_some());
    assert!(hs["ok"]["accepted"].get("agent.switch").is_some());
    let proj = dir.path().join("proj");
    let (_ws, task_id) = seed_ws_task(&client, &base, &token, &proj, "e8", None).await;
    let created = rpc(
        &client,
        &base,
        Some(&token),
        "agent.create",
        json!({ "taskId": task_id, "provider": "cli.generic" }),
    )
    .await;
    let agent_id = rpc_id(&created, "id");
    let sent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.send",
        json!({ "agentId": agent_id, "content": "hi" }),
    )
    .await;
    assert!(sent.get("error").is_none(), "{sent}");
    wait_status(&client, &base, &token, &agent_id, "idle").await;
    let ctx = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get_context",
        json!({ "agentId": agent_id }),
    )
    .await;
    assert!(ctx.get("error").is_none(), "{ctx}");
    let sw = rpc(
        &client,
        &base,
        Some(&token),
        "agent.switch",
        json!({ "agentId": agent_id, "provider": "cli.claude" }),
    )
    .await;
    assert!(sw.get("error").is_none(), "{sw}");
    let guides = rpc(
        &client,
        &base,
        Some(&token),
        "workspace.guides.get",
        json!({ "workspaceId": _ws }),
    )
    .await;
    assert_eq!(guides["error"]["code"], "version_mismatch", "{guides}");
    let _ = tx.send(());
    let _ = join.await;
}

#[test]
fn migrations_0001_to_0007_byte_identical_to_ceb841b() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../rt-storage/migrations");
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for name in [
        "0001_init.sql",
        "0002_worktrees.sql",
        "0003_policies.sql",
        "0004_terminal.sql",
        "0005_artifacts.sql",
        "0006_loops.sql",
        "0007_model_ux.sql",
    ] {
        let disk = std::fs::read(root.join(name)).unwrap();
        let git = Command::new("git")
            .args([
                "show",
                &format!("ceb841b:crates/rt-storage/migrations/{name}"),
            ])
            .current_dir(&repo)
            .output()
            .expect("git show");
        assert!(
            git.status.success(),
            "git show ceb841b:{name} failed: {}",
            String::from_utf8_lossy(&git.stderr)
        );
        assert_eq!(disk, git.stdout, "{name} drifted from ceb841b");
    }
    assert!(!rt_protocol::TRADABLE_METHODS
        .iter()
        .any(|m| m.to_ascii_lowercase().contains("phase")));
    assert!(!rt_protocol::TRADABLE_METHODS
        .iter()
        .any(|m| m.to_ascii_lowercase().contains("epic")));
}
