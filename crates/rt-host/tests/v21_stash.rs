//! V6 host slice: C63 prompt stash list/add/delete (protocol 1.9).
//! imagePath is encoded inside prompt_stash.body; 0009 stays untouched.

use std::path::Path;
use std::pin::Pin;
use std::process::Command;
use std::time::Duration;

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

fn client_1_8_methods() -> Value {
    let mut m = v1_methods();
    if let Value::Object(map) = &mut m {
        map.insert("agent.create".into(), json!({ "major": 1, "minor": 5 }));
        map.insert("sync.export".into(), json!({ "major": 1, "minor": 8 }));
        map.insert("sync.import".into(), json!({ "major": 1, "minor": 8 }));
    }
    m
}

fn client_1_9_methods() -> Value {
    let mut m = client_1_8_methods();
    if let Value::Object(map) = &mut m {
        map.insert("agent.create".into(), json!({ "major": 1, "minor": 9 }));
        map.insert("stash.list".into(), json!({ "major": 1, "minor": 9 }));
        map.insert("stash.add".into(), json!({ "major": 1, "minor": 9 }));
        map.insert("stash.delete".into(), json!({ "major": 1, "minor": 9 }));
    }
    m
}

fn backends() -> std::collections::HashMap<String, std::sync::Arc<dyn AgentBackend>> {
    let mut m = std::collections::HashMap::new();
    m.insert(
        "cli.generic".into(),
        std::sync::Arc::new(InstantGeneric) as std::sync::Arc<dyn AgentBackend>,
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

fn assert_no_secret_columns(db: &Path) {
    let conn = rusqlite::Connection::open(db).unwrap();
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
        .unwrap();
    let tables: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(
        !tables
            .iter()
            .any(|t| t.to_ascii_lowercase().contains("hook")),
        "hook table present: {tables:?}"
    );
    for t in tables {
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
                low != "token"
                    && low != "pat"
                    && low != "password"
                    && low != "dsn"
                    && low != "hook_secret"
                    && low != "hook-secret"
                    && !low.contains("secret")
                    && !low.contains("api_key"),
                "secret-like column {c} on {t}"
            );
        }
    }
}

#[tokio::test]
async fn stash_add_list_survives_host_restart() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, hs) = handshake(&client, &base, client_1_9_methods()).await;
    assert_eq!(hs["ok"]["accepted"]["stash.list"]["minor"], 9, "{hs}");
    assert_eq!(hs["ok"]["accepted"]["stash.add"]["minor"], 9, "{hs}");
    assert_eq!(hs["ok"]["accepted"]["stash.delete"]["minor"], 9, "{hs}");
    assert!(hs["ok"]["accepted"].get("hooks.list").is_none(), "{hs}");

    let empty = rpc(
        &client,
        &base,
        Some(&token),
        "stash.add",
        json!({ "body": "" }),
    )
    .await;
    assert_eq!(empty["error"]["code"], "invalid_params", "{empty}");

    let added = rpc(
        &client,
        &base,
        Some(&token),
        "stash.add",
        json!({ "body": "remember this prompt" }),
    )
    .await;
    assert!(added.get("error").is_none(), "{added}");
    assert_eq!(added["ok"]["body"], "remember this prompt");
    assert!(added["ok"].get("imagePath").is_none(), "{added}");
    let stash_id = rpc_id(&added, "id");
    let created_at = added["ok"]["createdAt"]
        .as_str()
        .expect("createdAt")
        .to_string();

    let listed = rpc(&client, &base, Some(&token), "stash.list", json!({})).await;
    assert!(listed.get("error").is_none(), "{listed}");
    let items = listed["ok"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{listed}");
    assert_eq!(items[0]["id"], stash_id);
    assert_eq!(items[0]["body"], "remember this prompt");
    assert_eq!(items[0]["createdAt"], created_at);

    let _ = tx.send(());
    let _ = join.await;

    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let (token, _) = handshake(&client, &base, client_1_9_methods()).await;
    let listed = rpc(&client, &base, Some(&token), "stash.list", json!({})).await;
    assert!(listed.get("error").is_none(), "{listed}");
    let items = listed["ok"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{listed}");
    assert_eq!(items[0]["id"], stash_id);
    assert_eq!(items[0]["body"], "remember this prompt");
    assert_eq!(items[0]["createdAt"], created_at);

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn stash_add_with_image_path_strips_prefix_on_list() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, client_1_9_methods()).await;

    let cloud = rpc(
        &client,
        &base,
        Some(&token),
        "stash.add",
        json!({ "body": "x", "imagePath": "https://cdn.example/a.png" }),
    )
    .await;
    assert_eq!(cloud["error"]["code"], "invalid_params", "{cloud}");

    let added = rpc(
        &client,
        &base,
        Some(&token),
        "stash.add",
        json!({
            "body": "prompt with shot",
            "imagePath": "/tmp/local-shot.png"
        }),
    )
    .await;
    assert!(added.get("error").is_none(), "{added}");
    assert_eq!(added["ok"]["body"], "prompt with shot");
    assert_eq!(added["ok"]["imagePath"], "/tmp/local-shot.png");
    assert!(
        !added["ok"]["body"]
            .as_str()
            .unwrap_or_default()
            .contains("rt-image-path:"),
        "prefix leaked in add body: {added}"
    );
    let stash_id = rpc_id(&added, "id");

    let listed = rpc(&client, &base, Some(&token), "stash.list", json!({})).await;
    assert!(listed.get("error").is_none(), "{listed}");
    let items = listed["ok"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{listed}");
    assert_eq!(items[0]["id"], stash_id);
    assert_eq!(items[0]["body"], "prompt with shot");
    assert_eq!(items[0]["imagePath"], "/tmp/local-shot.png");
    assert!(
        !items[0]["body"]
            .as_str()
            .unwrap_or_default()
            .contains("rt-image-path:"),
        "prefix leaked in list body: {listed}"
    );

    let conn = rusqlite::Connection::open(dir.path().join("host.db")).unwrap();
    let raw: String = conn
        .query_row(
            "SELECT body FROM prompt_stash WHERE id = ?1",
            [stash_id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        raw.starts_with("rt-image-path:/tmp/local-shot.png"),
        "expected durable prefix in sqlite body: {raw}"
    );
    assert!(raw.contains("prompt with shot"), "{raw}");

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn stash_delete_empties_list_missing_is_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, client_1_9_methods()).await;

    let added = rpc(
        &client,
        &base,
        Some(&token),
        "stash.add",
        json!({ "body": "to delete" }),
    )
    .await;
    let stash_id = rpc_id(&added, "id");

    let deleted = rpc(
        &client,
        &base,
        Some(&token),
        "stash.delete",
        json!({ "stashId": stash_id }),
    )
    .await;
    assert!(deleted.get("error").is_none(), "{deleted}");

    let listed = rpc(&client, &base, Some(&token), "stash.list", json!({})).await;
    assert!(listed.get("error").is_none(), "{listed}");
    let items = listed["ok"]["items"].as_array().unwrap();
    assert!(items.is_empty(), "{listed}");

    let missing = rpc(
        &client,
        &base,
        Some(&token),
        "stash.delete",
        json!({ "stashId": stash_id }),
    )
    .await;
    assert_eq!(missing["error"]["code"], "not_found", "{missing}");

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn sqlite_has_no_token_pat_hook_secret_columns() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, client_1_9_methods()).await;
    let _ = rpc(
        &client,
        &base,
        Some(&token),
        "stash.add",
        json!({ "body": "x", "imagePath": "/tmp/a.png" }),
    )
    .await;
    assert_no_secret_columns(&dir.path().join("host.db"));
    let conn = rusqlite::Connection::open(dir.path().join("host.db")).unwrap();
    let mut info = conn.prepare("PRAGMA table_info('prompt_stash')").unwrap();
    let cols: Vec<String> = info
        .query_map([], |r| r.get::<_, String>(1))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(cols, ["id", "body", "created_at"]);
    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn client_1_8_send_lives_stash_add_version_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, hs) = handshake(&client, &base, client_1_8_methods()).await;
    assert!(hs["ok"]["accepted"].get("agent.send").is_some(), "{hs}");
    assert!(hs["ok"]["accepted"].get("agent.create").is_some(), "{hs}");
    assert!(hs["ok"]["accepted"].get("stash.add").is_none(), "{hs}");
    assert!(hs["ok"]["accepted"].get("stash.list").is_none(), "{hs}");

    let proj = dir.path().join("ws");
    std::fs::create_dir_all(&proj).unwrap();
    let ws = rpc(
        &client,
        &base,
        Some(&token),
        "workspace.add",
        json!({ "path": proj.to_str().unwrap() }),
    )
    .await;
    let ws_id = rpc_id(&ws, "id");
    let task = rpc(
        &client,
        &base,
        Some(&token),
        "task.create",
        json!({ "title": "c18", "workspaceId": ws_id }),
    )
    .await;
    let task_id = rpc_id(&task, "id");
    let created = rpc(
        &client,
        &base,
        Some(&token),
        "agent.create",
        json!({ "taskId": task_id, "provider": "cli.generic" }),
    )
    .await;
    assert!(created.get("error").is_none(), "{created}");
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

    let stash = rpc(
        &client,
        &base,
        Some(&token),
        "stash.add",
        json!({ "body": "should not land" }),
    )
    .await;
    assert_eq!(stash["error"]["code"], "version_mismatch", "{stash}");

    let _ = tx.send(());
    let _ = join.await;
}

#[test]
fn migrations_0001_to_0010_byte_identical_to_673b549() {
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
        "0010_c37.sql",
    ];
    for name in names {
        let path = root.join("crates/rt-storage/migrations").join(name);
        let current = std::fs::read(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
        let out = Command::new("git")
            .args([
                "show",
                &format!("673b549:crates/rt-storage/migrations/{name}"),
            ])
            .current_dir(&root)
            .output()
            .expect("git show");
        assert!(
            out.status.success(),
            "git show 673b549:{name} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            current, out.stdout,
            "{name} must be byte-identical to 673b549"
        );
    }
}
