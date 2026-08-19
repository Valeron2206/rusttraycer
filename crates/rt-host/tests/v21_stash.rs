//! V6 host slice: C63 prompt stash list/add/delete (protocol 1.9).
//! imagePath is encoded inside prompt_stash.body; 0009 stays untouched.

use std::path::Path;
use std::pin::Pin;
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
fn migrations_0001_to_0010_byte_identical_to_freeze() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let frozen = [
        (
            "0001_init.sql",
            "1a331f2fd958ca9ed19261cfd696c1aad2c8d309aeb2d908953446b316bcbc7c",
        ),
        (
            "0002_worktrees.sql",
            "7a56889d97b25d9cba5effec10564b7a7f06acbaa836278e5518cb29bf1b68e3",
        ),
        (
            "0003_policies.sql",
            "b7a3a099705e845771312c6e56ef44fb286277c2d911763cf54c60bea9a7f398",
        ),
        (
            "0004_terminal.sql",
            "b7a6863b5fd47347c6cb87255e2eebb6a0913a2c20abef438d8e26a7593c756a",
        ),
        (
            "0005_artifacts.sql",
            "628be3a01bb1527fe06ca6a3a985abb0269080f6b5854d2cadfb85275381fd81",
        ),
        (
            "0006_loops.sql",
            "1021badfe2cb976217aa7accb7110f1ccceee7b5bd82ac49957f0a7e5fedf5b8",
        ),
        (
            "0007_model_ux.sql",
            "ce87343bdacd507400a7ffa9160483542914e5205f47df7047be0c35da0aac39",
        ),
        (
            "0008_workspace.sql",
            "dc194b66f7b26bd7ddefd833cd251ca55e0eaec0b89d30167e59d337ce87cbe3",
        ),
        (
            "0009_v21.sql",
            "a49cd2ab7c5a566ed45b3306aca330fd37dc16eff7d062309e632b2340962ddd",
        ),
        (
            "0010_c37.sql",
            "3f3f6ad375561d98f962c03a0af036a66d8d0573db87494badf264e81f1a26d4",
        ),
    ];
    for (name, expected) in frozen {
        let path = root.join("crates/rt-storage/migrations").join(name);
        let _current = std::fs::read(&path).unwrap_or_else(|_| panic!("{name}"));
        let out = std::process::Command::new("sha256sum")
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let got = String::from_utf8(out.stdout).unwrap();
        let got = got.split_whitespace().next().unwrap();
        assert_eq!(got, expected, "{name}");
    }
}
