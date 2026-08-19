//! V2 host slice: C21 search.query + C65 worktree.gc (protocol 1.9, storage 0009).

use std::path::Path;
use std::pin::Pin;
use std::process::Command;
use std::sync::Arc;
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
        map.insert("artifact.export".into(), json!({ "major": 1, "minor": 9 }));
        map.insert("search.query".into(), json!({ "major": 1, "minor": 9 }));
        map.insert("worktree.gc".into(), json!({ "major": 1, "minor": 9 }));
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

#[tokio::test]
async fn search_finds_task_workspace_artifact_by_title_body_kinds_filter() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, hs) = handshake(&client, &base, f9_methods()).await;
    assert_eq!(hs["ok"]["accepted"]["search.query"]["minor"], 9, "{hs}");

    let proj = dir.path().join("needle-ws-dir");
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
        json!({ "title": "needle-task-title", "workspaceId": ws_id }),
    )
    .await;
    let task_id = rpc_id(&task, "id");
    let art = rpc(
        &client,
        &base,
        Some(&token),
        "artifact.create",
        json!({
            "taskId": task_id,
            "kind": "spec",
            "title": "needle-art-title",
            "body": "unique-body-token lives here"
        }),
    )
    .await;
    let art_id = rpc_id(&art, "id");

    let all = rpc(
        &client,
        &base,
        Some(&token),
        "search.query",
        json!({ "q": "needle" }),
    )
    .await;
    assert!(all.get("error").is_none(), "{all}");
    let items = all["ok"]["items"].as_array().unwrap();
    let kinds: Vec<&str> = items.iter().filter_map(|i| i["kind"].as_str()).collect();
    assert!(kinds.contains(&"task"), "{all}");
    assert!(kinds.contains(&"workspace"), "{all}");
    assert!(kinds.contains(&"artifact"), "{all}");
    assert!(items.iter().any(|i| i["id"] == task_id), "{all}");
    assert!(items.iter().any(|i| i["id"] == ws_id), "{all}");
    assert!(items.iter().any(|i| i["id"] == art_id), "{all}");

    let body = rpc(
        &client,
        &base,
        Some(&token),
        "search.query",
        json!({ "q": "unique-body-token", "kinds": ["artifact"] }),
    )
    .await;
    assert!(body.get("error").is_none(), "{body}");
    let items = body["ok"]["items"].as_array().unwrap();
    assert!(items.iter().all(|i| i["kind"] == "artifact"), "{body}");
    assert!(items.iter().any(|i| i["id"] == art_id), "{body}");

    let only_task = rpc(
        &client,
        &base,
        Some(&token),
        "search.query",
        json!({ "q": "needle", "kinds": ["task"] }),
    )
    .await;
    let items = only_task["ok"]["items"].as_array().unwrap();
    assert!(items.iter().all(|i| i["kind"] == "task"), "{only_task}");
    assert!(items.iter().any(|i| i["id"] == task_id), "{only_task}");
    assert!(
        !items.iter().any(|i| i["kind"] == "workspace"),
        "{only_task}"
    );

    let empty = rpc(
        &client,
        &base,
        Some(&token),
        "search.query",
        json!({ "q": "" }),
    )
    .await;
    assert!(empty.get("error").is_none(), "{empty}");
    assert!(
        empty["ok"]["items"].as_array().unwrap().is_empty(),
        "{empty}"
    );

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn worktree_gc_dry_run_lists_stale_and_does_not_delete() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, hs) = handshake(&client, &base, f9_methods()).await;
    assert_eq!(hs["ok"]["accepted"]["worktree.gc"]["minor"], 9, "{hs}");

    let proj = dir.path().join("repo");
    init_git_repo(&proj);
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
        json!({ "title": "gc", "workspaceId": ws_id }),
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
    let agent_id = rpc_id(&created, "id");
    let wt = rpc(
        &client,
        &base,
        Some(&token),
        "worktree.ensure",
        json!({ "agentId": agent_id }),
    )
    .await;
    assert!(wt.get("error").is_none(), "{wt}");
    let wt_id = rpc_id(&wt, "id");
    let wt_path = wt["ok"]["path"].as_str().unwrap().to_string();
    assert!(Path::new(&wt_path).exists(), "{wt_path}");

    let gc = rpc(
        &client,
        &base,
        Some(&token),
        "worktree.gc",
        json!({ "dryRun": true }),
    )
    .await;
    assert!(gc.get("error").is_none(), "{gc}");
    assert_eq!(gc["ok"]["dryRun"], true, "{gc}");
    let items = gc["ok"]["items"].as_array().unwrap();
    assert!(
        items
            .iter()
            .any(|i| i["worktreeId"] == wt_id && i["reason"] == "stale"),
        "{gc}"
    );
    assert!(
        Path::new(&wt_path).exists(),
        "dryRun must not delete {wt_path}"
    );
    let listed = rpc(
        &client,
        &base,
        Some(&token),
        "worktree.list",
        json!({ "workspaceId": ws_id }),
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

#[tokio::test]
async fn worktree_gc_does_not_remove_running_agent_worktree() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f9_methods()).await;

    let proj = dir.path().join("repo");
    init_git_repo(&proj);
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
        json!({ "title": "gc-run", "workspaceId": ws_id }),
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
    let agent_id = rpc_id(&created, "id");
    let wt = rpc(
        &client,
        &base,
        Some(&token),
        "worktree.ensure",
        json!({ "agentId": agent_id }),
    )
    .await;
    let wt_id = rpc_id(&wt, "id");
    let wt_path = wt["ok"]["path"].as_str().unwrap().to_string();

    let db = dir.path().join("host.db");
    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute(
            "UPDATE agents SET status = 'running' WHERE id = ?1",
            [&agent_id],
        )
        .unwrap();
    }

    let dry = rpc(
        &client,
        &base,
        Some(&token),
        "worktree.gc",
        json!({ "dryRun": true }),
    )
    .await;
    assert!(dry.get("error").is_none(), "{dry}");
    let items = dry["ok"]["items"].as_array().unwrap();
    assert!(
        !items.iter().any(|i| i["worktreeId"] == wt_id),
        "running worktree listed: {dry}"
    );

    let real = rpc(
        &client,
        &base,
        Some(&token),
        "worktree.gc",
        json!({ "dryRun": false }),
    )
    .await;
    assert!(real.get("error").is_none(), "{real}");
    let items = real["ok"]["items"].as_array().unwrap();
    assert!(
        !items.iter().any(|i| i["worktreeId"] == wt_id),
        "running worktree removed: {real}"
    );
    assert!(Path::new(&wt_path).exists(), "running worktree dir gone");
    let listed = rpc(
        &client,
        &base,
        Some(&token),
        "worktree.list",
        json!({ "workspaceId": ws_id }),
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
fn schema_0009_is_nine_no_secret_columns_0001_to_0008_byte_identical_to_freeze() {
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
    let dir = tempfile::tempdir().unwrap();
    let store = rt_storage::Store::open(dir.path().join("host.db")).unwrap();
    store.migrate().unwrap();
    let conn = rusqlite::Connection::open(store.path()).unwrap();
    let schema: String = conn
        .query_row(
            "SELECT value FROM schema_meta WHERE key = 'schema'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(schema, "10");
    for name in [
        "provider_accounts",
        "user_presets",
        "prompt_stash",
        "worktree_settings",
    ] {
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [name],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1, "missing {name}");
    }
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
                || low == "password"
                || low.contains("secret")
                || low.contains("api_key")
                || (low == "key" && t != "schema_meta");
            assert!(!secret, "secret-like column {c} on {t}");
        }
    }
}
#[tokio::test]
async fn client_without_1_9_send_export_live_search_query_version_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, hs) = handshake(&client, &base, f8_methods()).await;
    assert!(hs["ok"]["accepted"].get("agent.send").is_some(), "{hs}");
    assert!(
        hs["ok"]["accepted"].get("artifact.export").is_some(),
        "{hs}"
    );
    assert!(hs["ok"]["accepted"].get("search.query").is_none(), "{hs}");
    assert!(hs["ok"]["accepted"].get("worktree.gc").is_none(), "{hs}");

    let proj = dir.path().join("proj");
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

    let art = rpc(
        &client,
        &base,
        Some(&token),
        "artifact.create",
        json!({
            "taskId": task_id,
            "kind": "spec",
            "title": "Note",
            "body": "body"
        }),
    )
    .await;
    assert!(art.get("error").is_none(), "{art}");
    let art_id = rpc_id(&art, "id");
    let md = rpc(
        &client,
        &base,
        Some(&token),
        "artifact.export",
        json!({ "artifactId": art_id, "format": "md" }),
    )
    .await;
    assert!(md.get("error").is_none(), "{md}");
    assert_eq!(md["ok"]["format"], "md");

    let search = rpc(
        &client,
        &base,
        Some(&token),
        "search.query",
        json!({ "q": "Note" }),
    )
    .await;
    assert_eq!(search["error"]["code"], "version_mismatch", "{search}");

    let _ = tx.send(());
    let _ = join.await;
}
