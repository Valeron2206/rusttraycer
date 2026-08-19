//! V7 host slice: C58 sync.push/pull and user presets (protocol 1.9).
//! RUSTTRAYCER_SYNC_SECRET is optional and env-only. 0001–0010 stay untouched.

use std::path::Path;
use std::pin::Pin;

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
        map.insert("preset.list".into(), json!({ "major": 1, "minor": 7 }));
        map.insert("sync.export".into(), json!({ "major": 1, "minor": 8 }));
        map.insert("sync.import".into(), json!({ "major": 1, "minor": 8 }));
    }
    m
}

fn client_1_9_methods() -> Value {
    let mut m = client_1_8_methods();
    if let Value::Object(map) = &mut m {
        map.insert("agent.create".into(), json!({ "major": 1, "minor": 9 }));
        map.insert("sync.push".into(), json!({ "major": 1, "minor": 9 }));
        map.insert("sync.pull".into(), json!({ "major": 1, "minor": 9 }));
        map.insert("preset.create".into(), json!({ "major": 1, "minor": 9 }));
        map.insert("preset.update".into(), json!({ "major": 1, "minor": 9 }));
        map.insert("preset.delete".into(), json!({ "major": 1, "minor": 9 }));
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

async fn seed_ws_task(
    client: &reqwest::Client,
    base: &str,
    token: &str,
    proj: &Path,
    title: &str,
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
    assert!(ws.get("error").is_none(), "{ws}");
    let ws_id = rpc_id(&ws, "id");
    let task = rpc(
        client,
        base,
        Some(token),
        "task.create",
        json!({ "title": title, "workspaceId": ws_id }),
    )
    .await;
    assert!(task.get("error").is_none(), "{task}");
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
                    && low != "hook_secret"
                    && low != "hook-secret"
                    && !low.contains("secret")
                    && !low.contains("api_key"),
                "secret-like column {c} on {t}"
            );
        }
    }
}

static SYNC_SECRET_ENV: std::sync::LazyLock<tokio::sync::Mutex<()>> =
    std::sync::LazyLock::new(|| tokio::sync::Mutex::new(()));

struct ClearSyncSecret;

impl Drop for ClearSyncSecret {
    fn drop(&mut self) {
        std::env::remove_var("RUSTTRAYCER_SYNC_SECRET");
    }
}

#[tokio::test(flavor = "current_thread")]
async fn push_no_secret_rewrites_host_id_then_conflict() {
    let _env_guard = SYNC_SECRET_ENV.lock().await;
    let _clear = ClearSyncSecret;
    std::env::remove_var("RUSTTRAYCER_SYNC_SECRET");

    let dir_a = tempfile::tempdir().unwrap();
    let dir_b = tempfile::tempdir().unwrap();
    let (addr_a, tx_a, join_a, host_a) = rt_host::spawn_test_host(dir_a.path(), Some(backends()))
        .await
        .unwrap();
    let (addr_b, tx_b, join_b, host_b) = rt_host::spawn_test_host(dir_b.path(), Some(backends()))
        .await
        .unwrap();
    assert_eq!(addr_a.ip().to_string(), "127.0.0.1");
    assert_eq!(addr_b.ip().to_string(), "127.0.0.1");
    let base_a = format!("http://127.0.0.1:{}", addr_a.port());
    let base_b = format!("http://127.0.0.1:{}", addr_b.port());
    let client = reqwest::Client::new();
    let (tok_a, hs_a) = handshake(&client, &base_a, client_1_9_methods()).await;
    let (tok_b, hs_b) = handshake(&client, &base_b, client_1_9_methods()).await;
    assert_eq!(hs_a["ok"]["accepted"]["sync.push"]["minor"], 9, "{hs_a}");
    assert_eq!(hs_a["ok"]["accepted"]["sync.pull"]["minor"], 9, "{hs_a}");
    assert_eq!(hs_b["ok"]["accepted"]["sync.push"]["minor"], 9, "{hs_b}");

    let (_ws_a, task_id) = seed_ws_task(
        &client,
        &base_a,
        &tok_a,
        &dir_a.path().join("proj-a"),
        "c58-push",
    )
    .await;
    let created = rpc(
        &client,
        &base_a,
        Some(&tok_a),
        "agent.create",
        json!({ "taskId": task_id, "provider": "cli.generic" }),
    )
    .await;
    assert!(created.get("error").is_none(), "{created}");
    let agent_id = rpc_id(&created, "id");
    let agents_a = rpc(
        &client,
        &base_a,
        Some(&tok_a),
        "agent.list",
        json!({ "taskId": task_id }),
    )
    .await;
    assert_eq!(agents_a["ok"]["items"][0]["hostId"], host_a);

    let proj_b = dir_b.path().join("proj-b");
    std::fs::create_dir_all(&proj_b).unwrap();
    let ws_b = rpc(
        &client,
        &base_b,
        Some(&tok_b),
        "workspace.add",
        json!({ "path": proj_b.to_str().unwrap() }),
    )
    .await;
    assert!(ws_b.get("error").is_none(), "{ws_b}");

    // HTTP peer route: no X-Rt-Session, env unset, header optional.
    let with_header = client
        .post(format!("{base_a}/sync/v1/export"))
        .header("X-Rt-Sync-Secret", "ignored-when-unset")
        .json(&json!({ "taskIds": [task_id] }))
        .send()
        .await
        .unwrap();
    assert_eq!(with_header.status(), reqwest::StatusCode::OK);
    let without_header = client
        .post(format!("{base_a}/sync/v1/export"))
        .json(&json!({ "taskIds": [task_id] }))
        .send()
        .await
        .unwrap();
    assert_eq!(without_header.status(), reqwest::StatusCode::OK);

    let pushed = rpc(
        &client,
        &base_a,
        Some(&tok_a),
        "sync.push",
        json!({ "peerUrl": base_b, "taskIds": [task_id] }),
    )
    .await;
    assert!(pushed.get("error").is_none(), "{pushed}");
    assert_eq!(pushed["ok"]["tasks"], 1, "{pushed}");
    assert_eq!(pushed["ok"]["agents"], 1, "{pushed}");

    let got = rpc(
        &client,
        &base_b,
        Some(&tok_b),
        "task.get",
        json!({ "id": task_id }),
    )
    .await;
    assert_eq!(got["ok"]["id"], task_id, "{got}");
    let agents_b = rpc(
        &client,
        &base_b,
        Some(&tok_b),
        "agent.list",
        json!({ "taskId": task_id }),
    )
    .await;
    let items = agents_b["ok"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{agents_b}");
    assert_eq!(items[0]["id"], agent_id);
    assert_eq!(items[0]["hostId"], host_b, "{agents_b}");

    let ping_a = rpc(&client, &base_a, Some(&tok_a), "host.ping", json!({})).await;
    assert_eq!(ping_a["ok"]["hostId"], host_a);
    let agents_a_after = rpc(
        &client,
        &base_a,
        Some(&tok_a),
        "agent.list",
        json!({ "taskId": task_id }),
    )
    .await;
    assert_eq!(agents_a_after["ok"]["items"][0]["hostId"], host_a);

    let again = rpc(
        &client,
        &base_a,
        Some(&tok_a),
        "sync.push",
        json!({ "peerUrl": base_b, "taskIds": [task_id] }),
    )
    .await;
    assert_eq!(again["error"]["code"], "conflict", "{again}");

    let task_b = rpc(
        &client,
        &base_b,
        Some(&tok_b),
        "task.create",
        json!({
            "title": "from-b",
            "workspaceId": ws_b["ok"]["id"]
        }),
    )
    .await;
    let task_b_id = rpc_id(&task_b, "id");
    let ws_a_list = rpc(&client, &base_a, Some(&tok_a), "workspace.list", json!({})).await;
    let ws_a_id = ws_a_list["ok"]["items"][0]["id"].as_str().unwrap();
    let pulled_ok = rpc(
        &client,
        &base_a,
        Some(&tok_a),
        "sync.pull",
        json!({
            "peerUrl": base_b,
            "workspaceId": ws_a_id,
            "taskIds": [task_b_id]
        }),
    )
    .await;
    assert!(pulled_ok.get("error").is_none(), "{pulled_ok}");
    assert_eq!(pulled_ok["ok"]["tasks"], 1, "{pulled_ok}");

    let pull = rpc(
        &client,
        &base_a,
        Some(&tok_a),
        "sync.pull",
        json!({ "peerUrl": base_b, "workspaceId": ws_a_id }),
    )
    .await;
    assert_eq!(pull["error"]["code"], "conflict", "{pull}");

    let _ = tx_a.send(());
    let _ = tx_b.send(());
    let _ = join_a.await;
    let _ = join_b.await;
}

#[tokio::test]
async fn handshake_1_9_accepts_new_methods_1_8_keeps_export_import() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();

    let (_, hs19) = handshake(&client, &base, client_1_9_methods()).await;
    assert_eq!(hs19["ok"]["accepted"]["sync.push"]["minor"], 9, "{hs19}");
    assert_eq!(hs19["ok"]["accepted"]["sync.pull"]["minor"], 9, "{hs19}");
    assert_eq!(
        hs19["ok"]["accepted"]["preset.create"]["minor"], 9,
        "{hs19}"
    );
    assert_eq!(
        hs19["ok"]["accepted"]["preset.update"]["minor"], 9,
        "{hs19}"
    );
    assert_eq!(
        hs19["ok"]["accepted"]["preset.delete"]["minor"], 9,
        "{hs19}"
    );
    assert_eq!(hs19["ok"]["accepted"]["sync.export"]["minor"], 8, "{hs19}");
    assert_eq!(hs19["ok"]["accepted"]["sync.import"]["minor"], 8, "{hs19}");

    let (token18, hs18) = handshake(&client, &base, client_1_8_methods()).await;
    assert_eq!(hs18["ok"]["accepted"]["sync.export"]["minor"], 8, "{hs18}");
    assert_eq!(hs18["ok"]["accepted"]["sync.import"]["minor"], 8, "{hs18}");
    assert!(hs18["ok"]["accepted"].get("sync.push").is_none(), "{hs18}");
    assert!(hs18["ok"]["accepted"].get("sync.pull").is_none(), "{hs18}");
    assert!(
        hs18["ok"]["accepted"].get("preset.create").is_none(),
        "{hs18}"
    );
    assert!(
        hs18["ok"]["accepted"].get("preset.update").is_none(),
        "{hs18}"
    );
    assert!(
        hs18["ok"]["accepted"].get("preset.delete").is_none(),
        "{hs18}"
    );

    let dir_b = tempfile::tempdir().unwrap();
    let (addr_b, tx_b, join_b, _) = rt_host::spawn_test_host(dir_b.path(), Some(backends()))
        .await
        .unwrap();
    let base_b = format!("http://{addr_b}");
    let (token_b, _) = handshake(&client, &base_b, client_1_8_methods()).await;
    let (ws_a, task_id) =
        seed_ws_task(&client, &base, &token18, &dir.path().join("ws18"), "c18").await;
    let exported = rpc(
        &client,
        &base,
        Some(&token18),
        "sync.export",
        json!({ "taskIds": [task_id] }),
    )
    .await;
    assert!(exported.get("error").is_none(), "{exported}");
    let (ws_b, _) = seed_ws_task(
        &client,
        &base_b,
        &token_b,
        &dir_b.path().join("ws18b"),
        "ph",
    )
    .await;
    let imported = rpc(
        &client,
        &base_b,
        Some(&token_b),
        "sync.import",
        json!({ "workspaceId": ws_b, "archive": exported["ok"]["archive"] }),
    )
    .await;
    assert!(imported.get("error").is_none(), "{imported}");
    let _ = ws_a;

    let denied = rpc(
        &client,
        &base,
        Some(&token18),
        "sync.push",
        json!({ "peerUrl": "http://127.0.0.1:9" }),
    )
    .await;
    assert_eq!(denied["error"]["code"], "version_mismatch", "{denied}");

    let _ = tx.send(());
    let _ = tx_b.send(());
    let _ = join.await;
    let _ = join_b.await;
}

#[tokio::test(flavor = "current_thread")]
async fn sync_secret_mismatch_is_auth_required_match_succeeds() {
    let _env_guard = SYNC_SECRET_ENV.lock().await;
    let _clear = ClearSyncSecret;
    std::env::set_var("RUSTTRAYCER_SYNC_SECRET", "c58-test-secret");

    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, client_1_9_methods()).await;
    let (_ws, task_id) =
        seed_ws_task(&client, &base, &token, &dir.path().join("ws"), "secret").await;

    let missing = client
        .post(format!("{base}/sync/v1/export"))
        .json(&json!({ "taskIds": [task_id] }))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), reqwest::StatusCode::UNAUTHORIZED);
    let missing_j: Value = missing.json().await.unwrap();
    assert_eq!(missing_j["error"]["code"], "auth_required", "{missing_j}");

    let wrong = client
        .post(format!("{base}/sync/v1/export"))
        .header("X-Rt-Sync-Secret", "nope")
        .json(&json!({ "taskIds": [task_id] }))
        .send()
        .await
        .unwrap();
    assert_eq!(wrong.status(), reqwest::StatusCode::UNAUTHORIZED);
    let wrong_j: Value = wrong.json().await.unwrap();
    assert_eq!(wrong_j["error"]["code"], "auth_required", "{wrong_j}");

    let ok = client
        .post(format!("{base}/sync/v1/export"))
        .header("X-Rt-Sync-Secret", "c58-test-secret")
        .json(&json!({ "taskIds": [task_id] }))
        .send()
        .await
        .unwrap();
    assert_eq!(ok.status(), reqwest::StatusCode::OK);
    let ok_j: Value = ok.json().await.unwrap();
    assert_eq!(ok_j["archive"]["kind"], "rusttraycer.export", "{ok_j}");

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn managed_cloud_and_invalid_peer_url_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, client_1_9_methods()).await;
    for url in [
        "https://sync.traycer.ai/v1",
        "https://app.TRAYCER.com/sync",
        "",
        "  ",
        "/relative",
        "file:///tmp",
        "ftp://127.0.0.1",
    ] {
        let got = rpc(
            &client,
            &base,
            Some(&token),
            "sync.push",
            json!({ "peerUrl": url }),
        )
        .await;
        assert_eq!(got["error"]["code"], "invalid_params", "url={url} {got}");
    }
    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn preset_crud_lists_builtins_plus_user_survives_restart() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, client_1_9_methods()).await;

    let created = rpc(
        &client,
        &base,
        Some(&token),
        "preset.create",
        json!({
            "name": "mine",
            "defaultRole": "coder",
            "titleHint": "hint",
            "prompt": "do the thing"
        }),
    )
    .await;
    assert!(created.get("error").is_none(), "{created}");
    assert_eq!(created["ok"]["title"], "mine");
    assert_eq!(created["ok"]["name"], "mine");
    assert_eq!(created["ok"]["defaultRole"], "coder");
    assert_eq!(created["ok"]["titleHint"], "hint");
    let user_id = rpc_id(&created, "id");

    let listed = rpc(&client, &base, Some(&token), "preset.list", json!({})).await;
    let items = listed["ok"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 5, "{listed}");
    let ids: Vec<&str> = items.iter().map(|i| i["id"].as_str().unwrap()).collect();
    assert_eq!(&ids[..4], ["planning", "review", "debug", "document"]);
    assert_eq!(ids[4], user_id);
    assert!(items[0].get("name").is_none(), "{listed}");

    let updated = rpc(
        &client,
        &base,
        Some(&token),
        "preset.update",
        json!({ "id": user_id, "name": "renamed", "defaultRole": "planner" }),
    )
    .await;
    assert!(updated.get("error").is_none(), "{updated}");
    assert_eq!(updated["ok"]["name"], "renamed");
    assert_eq!(updated["ok"]["defaultRole"], "planner");

    let builtin_del = rpc(
        &client,
        &base,
        Some(&token),
        "preset.delete",
        json!({ "id": "planning" }),
    )
    .await;
    assert_eq!(
        builtin_del["error"]["code"], "invalid_params",
        "{builtin_del}"
    );
    let builtin_upd = rpc(
        &client,
        &base,
        Some(&token),
        "preset.update",
        json!({ "id": "planning", "name": "nope" }),
    )
    .await;
    assert_eq!(
        builtin_upd["error"]["code"], "invalid_params",
        "{builtin_upd}"
    );

    let reserved = rpc(
        &client,
        &base,
        Some(&token),
        "preset.create",
        json!({ "name": "Planning", "defaultRole": "planner" }),
    )
    .await;
    assert_eq!(reserved["error"]["code"], "invalid_params", "{reserved}");
    let dup = rpc(
        &client,
        &base,
        Some(&token),
        "preset.create",
        json!({ "name": "renamed", "defaultRole": "coder" }),
    )
    .await;
    assert_eq!(dup["error"]["code"], "invalid_params", "{dup}");

    let _ = tx.send(());
    let _ = join.await;

    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let (token, _) = handshake(&client, &base, client_1_9_methods()).await;
    let listed = rpc(&client, &base, Some(&token), "preset.list", json!({})).await;
    let items = listed["ok"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 5, "{listed}");
    assert_eq!(items[4]["id"], user_id);
    assert_eq!(items[4]["name"], "renamed");

    let deleted = rpc(
        &client,
        &base,
        Some(&token),
        "preset.delete",
        json!({ "id": user_id }),
    )
    .await;
    assert_eq!(deleted["ok"]["deleted"], true);
    let missing = rpc(
        &client,
        &base,
        Some(&token),
        "preset.delete",
        json!({ "id": user_id }),
    )
    .await;
    assert_eq!(missing["error"]["code"], "not_found", "{missing}");
    assert_no_secret_columns(&dir.path().join("host.db"));
    let pid = std::fs::read_to_string(dir.path().join("pid.json")).unwrap();
    assert!(!pid.to_ascii_lowercase().contains("secret"), "{pid}");

    let _ = tx.send(());
    let _ = join.await;
}
