//! V4 host slice: C51 account labels + C53 mid-turn steer (protocol 1.9).

use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures::Stream;
use rt_runtime::{AgentBackend, Availability, HarnessCaps, TurnEvent, TurnRequest};
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

struct SlowGeneric;

impl AgentBackend for SlowGeneric {
    fn id(&self) -> &'static str {
        "cli.generic"
    }
    fn available(&self) -> Availability {
        Availability {
            available: true,
            detail: "slow generic".into(),
        }
    }
    fn caps(&self) -> HarnessCaps {
        HarnessCaps::CLI_GENERIC
    }
    fn start_turn(&self, _req: TurnRequest) -> Pin<Box<dyn Stream<Item = TurnEvent> + Send>> {
        Box::pin(futures::stream::once(async {
            tokio::time::sleep(Duration::from_secs(30)).await;
            TurnEvent::Finished { exit_code: 0 }
        }))
    }
}

struct SlowClaude;

impl AgentBackend for SlowClaude {
    fn id(&self) -> &'static str {
        "cli.claude"
    }
    fn available(&self) -> Availability {
        Availability {
            available: true,
            detail: "slow claude".into(),
        }
    }
    fn caps(&self) -> HarnessCaps {
        HarnessCaps::CLI_CLAUDE
    }
    fn start_turn(&self, _req: TurnRequest) -> Pin<Box<dyn Stream<Item = TurnEvent> + Send>> {
        Box::pin(futures::stream::once(async {
            tokio::time::sleep(Duration::from_secs(30)).await;
            TurnEvent::Finished { exit_code: 0 }
        }))
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
        map.insert("agent.switch".into(), json!({ "major": 1, "minor": 6 }));
        map.insert("sync.export".into(), json!({ "major": 1, "minor": 8 }));
        map.insert("sync.import".into(), json!({ "major": 1, "minor": 8 }));
    }
    m
}

fn client_1_9_methods() -> Value {
    let mut m = client_1_8_methods();
    if let Value::Object(map) = &mut m {
        map.insert("agent.create".into(), json!({ "major": 1, "minor": 9 }));
        map.insert("account.list".into(), json!({ "major": 1, "minor": 9 }));
        map.insert("account.create".into(), json!({ "major": 1, "minor": 9 }));
        map.insert("agent.steer".into(), json!({ "major": 1, "minor": 9 }));
    }
    m
}

fn backends_instant() -> std::collections::HashMap<String, Arc<dyn AgentBackend>> {
    let mut m = std::collections::HashMap::new();
    m.insert(
        "cli.generic".into(),
        Arc::new(InstantGeneric) as Arc<dyn AgentBackend>,
    );
    m
}

fn backends_slow() -> std::collections::HashMap<String, Arc<dyn AgentBackend>> {
    let mut m = std::collections::HashMap::new();
    m.insert(
        "cli.generic".into(),
        Arc::new(SlowGeneric) as Arc<dyn AgentBackend>,
    );
    m.insert(
        "cli.claude".into(),
        Arc::new(SlowClaude) as Arc<dyn AgentBackend>,
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

async fn seed_task(
    client: &reqwest::Client,
    base: &str,
    token: &str,
    dir: &Path,
) -> (String, String) {
    let proj = dir.join("ws");
    std::fs::create_dir_all(&proj).unwrap();
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
        json!({ "title": "t", "workspaceId": ws_id }),
    )
    .await;
    (ws_id, rpc_id(&task, "id"))
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
                    && !low.contains("secret")
                    && !low.contains("api_key"),
                "secret-like column {c} on {t}"
            );
        }
    }
}

#[tokio::test]
async fn account_create_list_returns_labels_sqlite_has_no_secrets() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends_instant()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, hs) = handshake(&client, &base, client_1_9_methods()).await;
    assert_eq!(hs["ok"]["accepted"]["account.list"]["minor"], 9, "{hs}");
    assert_eq!(hs["ok"]["accepted"]["account.create"]["minor"], 9, "{hs}");

    let created = rpc(
        &client,
        &base,
        Some(&token),
        "account.create",
        json!({ "provider": "cli.claude", "label": "work" }),
    )
    .await;
    assert!(created.get("error").is_none(), "{created}");
    assert_eq!(created["ok"]["provider"], "cli.claude");
    assert_eq!(created["ok"]["label"], "work");
    assert!(created["ok"].get("token").is_none(), "{created}");
    assert!(created["ok"].get("pat").is_none(), "{created}");
    let acc_id = rpc_id(&created, "id");

    let listed = rpc(&client, &base, Some(&token), "account.list", json!({})).await;
    assert!(listed.get("error").is_none(), "{listed}");
    let items = listed["ok"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{listed}");
    assert_eq!(items[0]["id"], acc_id);
    assert_eq!(items[0]["provider"], "cli.claude");
    assert_eq!(items[0]["label"], "work");
    assert!(items[0].get("token").is_none(), "{listed}");

    let dup = rpc(
        &client,
        &base,
        Some(&token),
        "account.create",
        json!({ "provider": "cli.claude", "label": "work" }),
    )
    .await;
    assert_eq!(dup["error"]["code"], "invalid_params", "{dup}");

    let bad = rpc(
        &client,
        &base,
        Some(&token),
        "account.create",
        json!({ "provider": "native", "label": "x" }),
    )
    .await;
    assert_eq!(bad["error"]["code"], "invalid_params", "{bad}");

    assert_no_secret_columns(&dir.path().join("host.db"));

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn agent_create_and_switch_store_account_id() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends_instant()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, client_1_9_methods()).await;
    let (_ws_id, task_id) = seed_task(&client, &base, &token, dir.path()).await;

    let a1 = rpc(
        &client,
        &base,
        Some(&token),
        "account.create",
        json!({ "provider": "cli.generic", "label": "home" }),
    )
    .await;
    let a1_id = rpc_id(&a1, "id");
    let a2 = rpc(
        &client,
        &base,
        Some(&token),
        "account.create",
        json!({ "provider": "cli.generic", "label": "work" }),
    )
    .await;
    let a2_id = rpc_id(&a2, "id");

    let agent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.create",
        json!({
            "taskId": task_id,
            "provider": "cli.generic",
            "accountId": a1_id
        }),
    )
    .await;
    assert!(agent.get("error").is_none(), "{agent}");
    assert_eq!(agent["ok"]["accountId"], a1_id, "{agent}");
    let agent_id = rpc_id(&agent, "id");

    let got = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get",
        json!({ "id": agent_id }),
    )
    .await;
    assert_eq!(got["ok"]["accountId"], a1_id, "{got}");

    let switched = rpc(
        &client,
        &base,
        Some(&token),
        "agent.switch",
        json!({ "agentId": agent_id, "accountId": a2_id }),
    )
    .await;
    assert!(switched.get("error").is_none(), "{switched}");
    assert_eq!(switched["ok"]["accountId"], a2_id, "{switched}");

    let mismatch = rpc(
        &client,
        &base,
        Some(&token),
        "account.create",
        json!({ "provider": "cli.claude", "label": "other" }),
    )
    .await;
    let claude_id = rpc_id(&mismatch, "id");
    let bad = rpc(
        &client,
        &base,
        Some(&token),
        "agent.switch",
        json!({ "agentId": agent_id, "accountId": claude_id }),
    )
    .await;
    assert_eq!(bad["error"]["code"], "invalid_params", "{bad}");

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn steer_on_idle_is_invalid_params() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends_instant()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, client_1_9_methods()).await;
    let (_ws_id, task_id) = seed_task(&client, &base, &token, dir.path()).await;
    let agent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.create",
        json!({ "taskId": task_id, "provider": "cli.claude" }),
    )
    .await;
    let agent_id = rpc_id(&agent, "id");
    let got = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get",
        json!({ "id": agent_id }),
    )
    .await;
    assert_eq!(got["ok"]["status"], "idle", "{got}");

    let steer = rpc(
        &client,
        &base,
        Some(&token),
        "agent.steer",
        json!({ "agentId": agent_id, "content": "nudge" }),
    )
    .await;
    assert_eq!(steer["error"]["code"], "invalid_params", "{steer}");

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn steer_on_running_generic_is_not_supported() {
    let dir = tempfile::tempdir().unwrap();
    let backends = backends_slow();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, client_1_9_methods()).await;
    let (_ws_id, task_id) = seed_task(&client, &base, &token, dir.path()).await;
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
        json!({ "agentId": agent_id, "content": "go" }),
    )
    .await;
    assert!(sent.get("error").is_none(), "{sent}");
    wait_status(&client, &base, &token, &agent_id, "running").await;

    let steer = rpc(
        &client,
        &base,
        Some(&token),
        "agent.steer",
        json!({ "agentId": agent_id, "content": "nudge" }),
    )
    .await;
    assert_eq!(steer["error"]["code"], "not_supported", "{steer}");

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn steer_on_running_claude_mock_is_ok() {
    let dir = tempfile::tempdir().unwrap();
    let backends = backends_slow();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, client_1_9_methods()).await;
    let (_ws_id, task_id) = seed_task(&client, &base, &token, dir.path()).await;
    let agent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.create",
        json!({ "taskId": task_id, "provider": "cli.claude" }),
    )
    .await;
    let agent_id = rpc_id(&agent, "id");
    let sent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.send",
        json!({ "agentId": agent_id, "content": "go" }),
    )
    .await;
    assert!(sent.get("error").is_none(), "{sent}");
    wait_status(&client, &base, &token, &agent_id, "running").await;

    let steer = rpc(
        &client,
        &base,
        Some(&token),
        "agent.steer",
        json!({ "agentId": agent_id, "content": "mid-turn nudge" }),
    )
    .await;
    assert!(steer.get("error").is_none(), "{steer}");
    assert_eq!(steer["ok"]["steered"], true, "{steer}");

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn client_1_8_send_switch_live_new_methods_version_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends_instant()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, hs) = handshake(&client, &base, client_1_8_methods()).await;
    assert!(hs["ok"]["accepted"].get("agent.send").is_some(), "{hs}");
    assert!(hs["ok"]["accepted"].get("agent.switch").is_some(), "{hs}");
    assert!(hs["ok"]["accepted"].get("account.list").is_none(), "{hs}");
    assert!(hs["ok"]["accepted"].get("agent.steer").is_none(), "{hs}");

    let (_ws_id, task_id) = seed_task(&client, &base, &token, dir.path()).await;
    let agent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.create",
        json!({ "taskId": task_id, "provider": "cli.generic" }),
    )
    .await;
    assert!(agent.get("error").is_none(), "{agent}");
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
    wait_status(&client, &base, &token, &agent_id, "idle").await;

    let switched = rpc(
        &client,
        &base,
        Some(&token),
        "agent.switch",
        json!({ "agentId": agent_id, "model": "gpt" }),
    )
    .await;
    assert!(switched.get("error").is_none(), "{switched}");

    let list = rpc(&client, &base, Some(&token), "account.list", json!({})).await;
    assert_eq!(list["error"]["code"], "version_mismatch", "{list}");
    let steer = rpc(
        &client,
        &base,
        Some(&token),
        "agent.steer",
        json!({ "agentId": agent_id, "content": "nudge" }),
    )
    .await;
    assert_eq!(steer["error"]["code"], "version_mismatch", "{steer}");

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

#[tokio::test]
async fn doctor_advertises_steer_caps() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends_slow()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, client_1_9_methods()).await;
    let doc = rpc(&client, &base, Some(&token), "host.doctor", json!({})).await;
    let items = doc["ok"]["providers"].as_array().expect("providers");
    let mut saw_generic = false;
    let mut saw_claude = false;
    for p in items {
        let id = p["id"].as_str().unwrap();
        let steer = p["caps"]["steer"].as_bool().unwrap_or(false);
        if id == "cli.generic" {
            saw_generic = true;
            assert!(!steer, "generic must not advertise steer: {p}");
        }
        if id == "cli.claude" {
            saw_claude = true;
            assert!(steer, "claude must advertise steer: {p}");
        }
    }
    assert!(saw_generic && saw_claude, "doctor={doc}");
    let _ = tx.send(());
    let _ = join.await;
}
