//! V3 host slice: C37 terminal/shell without a Task (protocol 1.9, storage 0010).

use std::path::Path;
use std::pin::Pin;
use std::process::Command;
use std::sync::Arc;

use futures::Stream;
use rt_runtime::{AgentBackend, Availability, TurnEvent, TurnRequest};
use serde_json::{json, Value};

#[derive(Clone)]
struct InstantGeneric;

impl AgentBackend for InstantGeneric {
    fn id(&self) -> &'static str {
        "cli.generic"
    }
    fn available(&self) -> Availability {
        Availability {
            available: true,
            detail: "instant generic".into(),
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

fn f8_methods() -> Value {
    let mut m = v1_methods();
    if let Value::Object(map) = &mut m {
        map.insert("agent.create".into(), json!({ "major": 1, "minor": 5 }));
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
            "workspace.guides.get",
            "settings.guide.get",
            "settings.guide.set",
            "preset.list",
            "agent.update",
        ] {
            map.insert(n.into(), json!({ "major": 1, "minor": 7 }));
        }
        map.insert("sync.export".into(), json!({ "major": 1, "minor": 8 }));
        map.insert("sync.import".into(), json!({ "major": 1, "minor": 8 }));
    }
    m
}

fn f9_methods() -> Value {
    let mut m = f8_methods();
    if let Value::Object(map) = &mut m {
        map.insert("agent.create".into(), json!({ "major": 1, "minor": 9 }));
        map.insert("shell.create".into(), json!({ "major": 1, "minor": 9 }));
        map.insert("shell.list".into(), json!({ "major": 1, "minor": 3 }));
        map.insert("shell.close".into(), json!({ "major": 1, "minor": 3 }));
        map.insert("pty.open".into(), json!({ "major": 1, "minor": 3 }));
        map.insert("pty.write".into(), json!({ "major": 1, "minor": 3 }));
        map.insert("pty.resize".into(), json!({ "major": 1, "minor": 3 }));
        map.insert("pty.close".into(), json!({ "major": 1, "minor": 3 }));
    }
    m
}

fn backends() -> std::collections::HashMap<String, Arc<dyn AgentBackend>> {
    let mut m = std::collections::HashMap::new();
    m.insert(
        "cli.generic".into(),
        Arc::new(InstantGeneric) as Arc<dyn AgentBackend>,
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

fn set_pty_cmd() {
    std::env::set_var("RUSTTRAYCER_PTY_CMD", "/bin/cat");
}

async fn add_workspace(
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
    let path = ws["ok"]["path"]
        .as_str()
        .unwrap_or_else(|| panic!("path missing: {ws}"))
        .to_string();
    (ws_id, path)
}

#[tokio::test]
async fn agent_create_without_task_binds_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, hs) = handshake(&client, &base, f9_methods()).await;
    assert_eq!(hs["ok"]["accepted"]["agent.create"]["minor"], 9, "{hs}");

    let proj = dir.path().join("ws");
    let (ws_id, _) = add_workspace(&client, &base, &token, &proj).await;

    let created = rpc(
        &client,
        &base,
        Some(&token),
        "agent.create",
        json!({ "workspaceId": ws_id }),
    )
    .await;
    assert!(created.get("error").is_none(), "{created}");
    let agent_id = rpc_id(&created, "id");
    assert!(
        created["ok"].get("taskId").is_none() || created["ok"]["taskId"].is_null(),
        "taskId should be null/absent: {created}"
    );
    assert_eq!(created["ok"]["workspaceId"], ws_id, "{created}");

    let got = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get",
        json!({ "id": agent_id }),
    )
    .await;
    assert!(got.get("error").is_none(), "{got}");
    assert!(
        got["ok"].get("taskId").is_none() || got["ok"]["taskId"].is_null(),
        "taskId should be null/absent: {got}"
    );
    assert_eq!(got["ok"]["workspaceId"], ws_id, "{got}");

    let listed = rpc(
        &client,
        &base,
        Some(&token),
        "agent.list",
        json!({ "workspaceId": ws_id }),
    )
    .await;
    assert!(listed.get("error").is_none(), "{listed}");
    let items = listed["ok"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{listed}");
    assert_eq!(items[0]["id"], agent_id);
    assert!(
        items[0].get("taskId").is_none() || items[0]["taskId"].is_null(),
        "{listed}"
    );
    assert_eq!(items[0]["workspaceId"], ws_id);

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn shell_create_without_task_binds_workspace_and_lists() {
    set_pty_cmd();
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, hs) = handshake(&client, &base, f9_methods()).await;
    assert_eq!(hs["ok"]["accepted"]["shell.create"]["minor"], 9, "{hs}");

    let proj = dir.path().join("sh");
    let (ws_id, ws_path) = add_workspace(&client, &base, &token, &proj).await;

    let created = rpc(
        &client,
        &base,
        Some(&token),
        "shell.create",
        json!({
            "workspaceId": ws_id,
            "cols": 80,
            "rows": 24
        }),
    )
    .await;
    assert!(created.get("error").is_none(), "{created}");
    assert!(created["ok"]["shellId"].as_str().is_some(), "{created}");
    assert!(created["ok"]["ptyId"].as_str().is_some(), "{created}");
    let cwd = created["ok"]["cwd"].as_str().unwrap();
    assert_eq!(Path::new(cwd), Path::new(&ws_path));

    let listed = rpc(
        &client,
        &base,
        Some(&token),
        "shell.list",
        json!({ "workspaceId": ws_id }),
    )
    .await;
    assert!(listed.get("error").is_none(), "{listed}");
    let items = listed["ok"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{listed}");
    assert_eq!(items[0]["shellId"], created["ok"]["shellId"]);
    assert_eq!(items[0]["ptyId"], created["ok"]["ptyId"]);

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn create_without_workspace_or_task_is_invalid_params() {
    set_pty_cmd();
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f9_methods()).await;

    let agent = rpc(&client, &base, Some(&token), "agent.create", json!({})).await;
    assert_eq!(agent["error"]["code"], "invalid_params", "{agent}");

    let agent_empty = rpc(
        &client,
        &base,
        Some(&token),
        "agent.create",
        json!({ "workspaceId": "" }),
    )
    .await;
    assert_eq!(
        agent_empty["error"]["code"], "invalid_params",
        "{agent_empty}"
    );

    let sh = rpc(
        &client,
        &base,
        Some(&token),
        "shell.create",
        json!({ "cols": 80, "rows": 24 }),
    )
    .await;
    assert_eq!(sh["error"]["code"], "invalid_params", "{sh}");

    let sh_empty = rpc(
        &client,
        &base,
        Some(&token),
        "shell.create",
        json!({ "workspaceId": "", "cols": 80, "rows": 24 }),
    )
    .await;
    assert_eq!(sh_empty["error"]["code"], "invalid_params", "{sh_empty}");

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn client_1_8_agent_create_with_task_still_lives() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, hs) = handshake(&client, &base, f8_methods()).await;
    assert_eq!(hs["ok"]["accepted"]["agent.create"]["minor"], 9, "{hs}");
    assert!(
        hs["ok"]["rejected"]
            .as_object()
            .map(|m| !m.contains_key("agent.create"))
            .unwrap_or(true),
        "{hs}"
    );

    let proj = dir.path().join("old");
    let (ws_id, _) = add_workspace(&client, &base, &token, &proj).await;
    let task = rpc(
        &client,
        &base,
        Some(&token),
        "task.create",
        json!({ "title": "c37-1.8", "workspaceId": ws_id }),
    )
    .await;
    let task_id = rpc_id(&task, "id");

    let created = rpc(
        &client,
        &base,
        Some(&token),
        "agent.create",
        json!({ "taskId": task_id }),
    )
    .await;
    assert!(created.get("error").is_none(), "{created}");
    assert_eq!(created["ok"]["taskId"], task_id, "{created}");
    assert!(
        created["ok"].get("workspaceId").is_none() || created["ok"]["workspaceId"].is_null(),
        "{created}"
    );

    let listed = rpc(
        &client,
        &base,
        Some(&token),
        "agent.list",
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
}

#[test]
fn migrations_0001_to_0009_byte_identical_vs_head_and_0010_present() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let names = [
        "0001_init.sql",
        "0002_worktrees.sql",
        "0003_policies.sql",
        "0004_terminal.sql",
        "0005_artifacts.sql",
        "0006_loops.sql",
        "0007_model_ux.sql",
        "0008_workspace.sql",
        "0009_v21.sql",
    ];
    for name in names {
        let path = root.join("crates/rt-storage/migrations").join(name);
        let current = std::fs::read(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
        let out = Command::new("git")
            .args(["show", &format!("HEAD:crates/rt-storage/migrations/{name}")])
            .current_dir(&root)
            .output()
            .expect("git show");
        assert!(
            out.status.success(),
            "git show HEAD:{name} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(current, out.stdout, "{name} must be byte-identical to HEAD");
    }
    let new_path = root.join("crates/rt-storage/migrations/0010_c37.sql");
    let body = std::fs::read_to_string(&new_path).unwrap_or_else(|e| panic!("read 0010: {e}"));
    assert!(body.contains("schema', '10'"), "{body}");
    assert!(body.contains("workspace_id"), "{body}");
    assert!(!body.to_ascii_lowercase().contains("cascade"), "{body}");
    assert!(!body.to_ascii_lowercase().contains("token"), "{body}");
    assert!(!body.to_ascii_lowercase().contains("secret"), "{body}");
}
