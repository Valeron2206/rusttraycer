//! E7 model UX: same-agent switch, profiles, prefs (protocol 1.6, storage 0007).

use std::path::Path;
use std::pin::Pin;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use futures::Stream;
use rt_runtime::{AgentBackend, Availability, HarnessCaps, TurnEvent, TurnRequest};
use serde_json::{json, Value};

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

fn f6_run_methods() -> Value {
    let mut m = f6_methods();
    if let Value::Object(map) = &mut m {
        map.remove("policy.get");
        map.remove("policy.set");
        map.remove("approval.respond");
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
        Arc::new(InstantClaude) as Arc<dyn AgentBackend>,
    );
    m.insert(
        "cli.codex".into(),
        Arc::new(InstantCodex) as Arc<dyn AgentBackend>,
    );
    m
}

fn slow_backends() -> std::collections::HashMap<String, Arc<dyn AgentBackend>> {
    let mut m = std::collections::HashMap::new();
    m.insert(
        "cli.generic".into(),
        Arc::new(SlowGeneric) as Arc<dyn AgentBackend>,
    );
    m.insert(
        "cli.claude".into(),
        Arc::new(InstantClaude) as Arc<dyn AgentBackend>,
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
        json!({ "title": "e7", "workspaceId": ws_id }),
    )
    .await;
    (ws_id, rpc_id(&task, "id"))
}

async fn create_agent(
    client: &reqwest::Client,
    base: &str,
    token: &str,
    task_id: &str,
    extra: Value,
) -> Value {
    let mut p = json!({ "taskId": task_id, "provider": "cli.generic" });
    if let Value::Object(extra) = extra {
        if let Value::Object(map) = &mut p {
            for (k, v) in extra {
                map.insert(k, v);
            }
        }
    }
    let resp = rpc(client, base, Some(token), "agent.create", p).await;
    assert!(resp.get("error").is_none(), "{resp}");
    resp
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
    let schema: String = conn
        .query_row(
            "SELECT value FROM schema_meta WHERE key = 'schema'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(schema, "7", "schema={schema}");
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
            let secret = low == "token"
                || low == "pat"
                || low == "account"
                || low == "password"
                || low.contains("secret")
                || low.contains("api_key")
                || (low == "key" && t != "schema_meta");
            assert!(!secret, "secret-like column {c} on {t}");
        }
    }
}

#[tokio::test]
async fn handshake_new_methods_1_6_older_keep_minors() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (_token, hs) = handshake(&client, &base, f6_methods()).await;
    assert_eq!(hs["ok"]["accepted"]["agent.switch"]["minor"], 6);
    assert_eq!(hs["ok"]["accepted"]["profile.create"]["minor"], 6);
    assert_eq!(hs["ok"]["accepted"]["prefs.get"]["minor"], 6);
    assert_eq!(hs["ok"]["accepted"]["agent.create"]["minor"], 5);
    assert_eq!(hs["ok"]["accepted"]["a2a.deliver"]["minor"], 5);
    assert_eq!(hs["ok"]["accepted"]["files.write"]["minor"], 2);
    assert_eq!(hs["ok"]["accepted"]["host.ping"]["minor"], 0);
    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn switch_keeps_agent_id_and_messages() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f6_run_methods()).await;
    let proj = dir.path().join("proj");
    let (_ws, task_id) = seed_task(&client, &base, &token, &proj).await;
    let created = create_agent(
        &client,
        &base,
        &token,
        &task_id,
        json!({ "provider": "cli.generic" }),
    )
    .await;
    let agent_id = rpc_id(&created, "id");

    for content in ["turn-one", "turn-two"] {
        let sent = rpc(
            &client,
            &base,
            Some(&token),
            "agent.send",
            json!({ "agentId": agent_id, "content": content }),
        )
        .await;
        assert!(sent.get("error").is_none(), "{sent}");
        wait_status(&client, &base, &token, &agent_id, "idle").await;
    }
    let ctx = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get_context",
        json!({ "agentId": agent_id }),
    )
    .await;
    let before = ctx["ok"]["messages"].as_array().unwrap().clone();
    assert!(before.len() >= 2, "{ctx}");

    let sw = rpc(
        &client,
        &base,
        Some(&token),
        "agent.switch",
        json!({ "agentId": agent_id, "provider": "cli.claude" }),
    )
    .await;
    assert!(sw.get("error").is_none(), "{sw}");
    assert_eq!(sw["ok"]["id"], agent_id);
    assert_eq!(sw["ok"]["provider"], "cli.claude");

    let got = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get",
        json!({ "id": agent_id }),
    )
    .await;
    assert_eq!(got["ok"]["id"], agent_id);
    assert_eq!(got["ok"]["provider"], "cli.claude");

    let ctx2 = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get_context",
        json!({ "agentId": agent_id }),
    )
    .await;
    assert_eq!(ctx2["ok"]["messages"].as_array().unwrap(), &before);
    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn switch_while_running_is_agent_busy() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(slow_backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f6_run_methods()).await;
    let proj = dir.path().join("proj");
    let (_ws, task_id) = seed_task(&client, &base, &token, &proj).await;
    let created = create_agent(
        &client,
        &base,
        &token,
        &task_id,
        json!({ "provider": "cli.generic" }),
    )
    .await;
    let agent_id = rpc_id(&created, "id");
    let sent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.send",
        json!({ "agentId": agent_id, "content": "long" }),
    )
    .await;
    assert!(sent.get("error").is_none(), "{sent}");
    wait_status(&client, &base, &token, &agent_id, "running").await;

    let sw = rpc(
        &client,
        &base,
        Some(&token),
        "agent.switch",
        json!({ "agentId": agent_id, "provider": "cli.claude" }),
    )
    .await;
    assert_eq!(sw["error"]["code"], "agent_busy", "{sw}");

    let got = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get",
        json!({ "id": agent_id }),
    )
    .await;
    assert_eq!(got["ok"]["provider"], "cli.generic");
    let ctx = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get_context",
        json!({ "agentId": agent_id }),
    )
    .await;
    let msgs = ctx["ok"]["messages"].as_array().unwrap();
    assert!(!msgs.is_empty(), "{ctx}");
    assert_eq!(msgs[0]["content"], "long");
    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn profile_apply_keeps_agent_id() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f6_methods()).await;
    let proj = dir.path().join("proj");
    let (_ws, task_id) = seed_task(&client, &base, &token, &proj).await;
    let a = create_agent(
        &client,
        &base,
        &token,
        &task_id,
        json!({ "provider": "cli.generic" }),
    )
    .await;
    let b = create_agent(
        &client,
        &base,
        &token,
        &task_id,
        json!({ "provider": "cli.generic" }),
    )
    .await;
    let b_id = rpc_id(&b, "id");
    assert_ne!(rpc_id(&a, "id"), b_id);

    let prof = rpc(
        &client,
        &base,
        Some(&token),
        "profile.create",
        json!({
            "name": "fast-opus",
            "provider": "cli.claude",
            "model": "opus",
            "effort": "high",
            "fast": true
        }),
    )
    .await;
    assert!(prof.get("error").is_none(), "{prof}");
    let profile_id = rpc_id(&prof, "id");

    let sw = rpc(
        &client,
        &base,
        Some(&token),
        "agent.switch",
        json!({ "agentId": b_id, "profileId": profile_id }),
    )
    .await;
    assert!(sw.get("error").is_none(), "{sw}");
    assert_eq!(sw["ok"]["id"], b_id);
    assert_eq!(sw["ok"]["provider"], "cli.claude");
    assert_eq!(sw["ok"]["model"], "opus");
    assert_eq!(sw["ok"]["effort"], "high");
    assert_eq!(sw["ok"]["fast"], true);

    let del = rpc(
        &client,
        &base,
        Some(&token),
        "profile.delete",
        json!({ "profileId": profile_id }),
    )
    .await;
    assert!(del.get("error").is_none(), "{del}");
    let got = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get",
        json!({ "id": b_id }),
    )
    .await;
    assert_eq!(got["ok"]["model"], "opus");
    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn create_without_params_fills_prefs_no_secret_columns() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f6_methods()).await;
    let proj = dir.path().join("proj");
    let (_ws, task_id) = seed_task(&client, &base, &token, &proj).await;
    let first = create_agent(
        &client,
        &base,
        &token,
        &task_id,
        json!({
            "provider": "cli.claude",
            "model": "sonnet",
            "effort": "medium",
            "fast": true
        }),
    )
    .await;
    let first_id = rpc_id(&first, "id");
    let sw = rpc(
        &client,
        &base,
        Some(&token),
        "agent.switch",
        json!({
            "agentId": first_id,
            "provider": "cli.claude",
            "model": "opus",
            "effort": "high",
            "fast": false
        }),
    )
    .await;
    assert!(sw.get("error").is_none(), "{sw}");

    let prefs = rpc(&client, &base, Some(&token), "prefs.get", json!({})).await;
    assert!(prefs.get("error").is_none(), "{prefs}");
    let items = prefs["ok"]["items"].as_array().unwrap();
    let claude = items
        .iter()
        .find(|i| i["provider"] == "cli.claude")
        .expect("claude prefs");
    assert_eq!(claude["model"], "opus");
    assert_eq!(claude["effort"], "high");
    assert_eq!(claude["fast"], false);

    let second = create_agent(
        &client,
        &base,
        &token,
        &task_id,
        json!({ "provider": "cli.claude" }),
    )
    .await;
    assert_eq!(second["ok"]["model"], "opus");
    assert_eq!(second["ok"]["effort"], "high");
    assert_eq!(second["ok"]["fast"], false);
    assert_ne!(rpc_id(&second, "id"), first_id);

    assert_no_secret_columns(&dir.path().join("host.db"));
    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn terminal_switch_to_generic_is_not_pty() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f6_methods()).await;
    let proj = dir.path().join("proj");
    let (_ws, task_id) = seed_task(&client, &base, &token, &proj).await;
    let created = create_agent(
        &client,
        &base,
        &token,
        &task_id,
        json!({ "provider": "cli.claude", "interface": "terminal" }),
    )
    .await;
    let agent_id = rpc_id(&created, "id");
    assert_eq!(created["ok"]["interface"], "terminal");

    let sw = rpc(
        &client,
        &base,
        Some(&token),
        "agent.switch",
        json!({ "agentId": agent_id, "provider": "cli.generic" }),
    )
    .await;
    assert_eq!(sw["error"]["code"], "not_pty", "{sw}");
    let got = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get",
        json!({ "id": agent_id }),
    )
    .await;
    assert_eq!(got["ok"]["provider"], "cli.claude");
    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn native_create_and_switch_invalid_params() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f6_methods()).await;
    let proj = dir.path().join("proj");
    let (_ws, task_id) = seed_task(&client, &base, &token, &proj).await;
    let created = rpc(
        &client,
        &base,
        Some(&token),
        "agent.create",
        json!({ "taskId": task_id, "provider": "native" }),
    )
    .await;
    assert_eq!(created["error"]["code"], "invalid_params", "{created}");

    let ok = create_agent(
        &client,
        &base,
        &token,
        &task_id,
        json!({ "provider": "cli.generic" }),
    )
    .await;
    let agent_id = rpc_id(&ok, "id");
    let sw = rpc(
        &client,
        &base,
        Some(&token),
        "agent.switch",
        json!({ "agentId": agent_id, "provider": "native" }),
    )
    .await;
    assert_eq!(sw["error"]["code"], "invalid_params", "{sw}");
    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn client_without_1_6_send_artifact_a2a_live_switch_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, hs) = handshake(&client, &base, f5_methods()).await;
    assert!(hs["ok"]["accepted"].get("agent.switch").is_none());
    assert!(hs["ok"]["accepted"].get("agent.send").is_some());
    assert!(hs["ok"]["accepted"].get("artifact.create").is_some());
    assert!(hs["ok"]["accepted"].get("a2a.deliver").is_some());

    let proj = dir.path().join("proj");
    let (_ws, task_id) = seed_task(&client, &base, &token, &proj).await;
    let from = rpc_id(
        &create_agent(
            &client,
            &base,
            &token,
            &task_id,
            json!({ "provider": "cli.generic" }),
        )
        .await,
        "id",
    );
    let to = rpc_id(
        &create_agent(
            &client,
            &base,
            &token,
            &task_id,
            json!({ "provider": "cli.claude" }),
        )
        .await,
        "id",
    );
    let sent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.send",
        json!({ "agentId": from, "content": "hi" }),
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

    let del = rpc(
        &client,
        &base,
        Some(&token),
        "a2a.deliver",
        json!({
            "fromAgentId": from,
            "toAgentId": to,
            "content": "review"
        }),
    )
    .await;
    assert!(del.get("error").is_none(), "{del}");

    let sw = rpc(
        &client,
        &base,
        Some(&token),
        "agent.switch",
        json!({ "agentId": from, "provider": "cli.claude" }),
    )
    .await;
    assert_eq!(sw["error"]["code"], "version_mismatch", "{sw}");
    let _ = tx.send(());
    let _ = join.await;
}

#[test]
fn migrations_0001_to_0006_byte_identical_to_ba44d6d() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../rt-storage/migrations");
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for name in [
        "0001_init.sql",
        "0002_worktrees.sql",
        "0003_policies.sql",
        "0004_terminal.sql",
        "0005_artifacts.sql",
        "0006_loops.sql",
    ] {
        let disk = std::fs::read(root.join(name)).unwrap();
        let git = Command::new("git")
            .args([
                "show",
                &format!("ba44d6d:crates/rt-storage/migrations/{name}"),
            ])
            .current_dir(&repo)
            .output()
            .expect("git show");
        assert!(
            git.status.success(),
            "git show ba44d6d:{name} failed: {}",
            String::from_utf8_lossy(&git.stderr)
        );
        assert_eq!(disk, git.stdout, "{name} drifted from ba44d6d");
    }
}
