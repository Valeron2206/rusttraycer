//! E9 durable export/import (protocol 1.8). No new migration.

use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures::Stream;
use rt_runtime::{AgentBackend, Availability, TurnEvent, TurnRequest};
use serde_json::{json, Value};

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

fn f8_methods() -> Value {
    let mut m = f7_methods();
    if let Value::Object(map) = &mut m {
        map.insert("sync.export".into(), json!({ "major": 1, "minor": 8 }));
        map.insert("sync.import".into(), json!({ "major": 1, "minor": 8 }));
    }
    m
}

fn f8_run_methods() -> Value {
    let mut m = f8_methods();
    if let Value::Object(map) = &mut m {
        map.remove("policy.get");
        map.remove("policy.set");
        map.remove("approval.respond");
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

fn collect_keys(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            for (k, val) in m {
                out.push(k.clone());
                collect_keys(val, out);
            }
        }
        Value::Array(a) => {
            for val in a {
                collect_keys(val, out);
            }
        }
        _ => {}
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[tokio::test]
async fn handshake_new_methods_1_8() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (_token, hs) = handshake(&client, &base, f8_methods()).await;
    assert_eq!(hs["ok"]["accepted"]["sync.export"]["minor"], 8);
    assert_eq!(hs["ok"]["accepted"]["sync.import"]["minor"], 8);
    assert_eq!(hs["ok"]["accepted"]["workspace.guides.get"]["minor"], 7);
    assert_eq!(hs["ok"]["accepted"]["agent.switch"]["minor"], 6);
    assert_eq!(hs["ok"]["accepted"]["agent.create"]["minor"], 9);
    assert_eq!(hs["ok"]["accepted"]["host.ping"]["minor"], 0);
    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn clone_export_import_preserves_ids_rewrites_host() {
    let dir_a = tempfile::tempdir().unwrap();
    let dir_b = tempfile::tempdir().unwrap();
    let (addr_a, tx_a, join_a, host_a) = rt_host::spawn_test_host(dir_a.path(), Some(backends()))
        .await
        .unwrap();
    let (addr_b, tx_b, join_b, host_b) = rt_host::spawn_test_host(dir_b.path(), Some(backends()))
        .await
        .unwrap();
    assert_ne!(host_a, host_b);
    let base_a = format!("http://{addr_a}");
    let base_b = format!("http://{addr_b}");
    let client = reqwest::Client::new();
    let (tok_a, _) = handshake(&client, &base_a, f8_run_methods()).await;
    let (tok_b, _) = handshake(&client, &base_b, f8_run_methods()).await;

    let proj_a = dir_a.path().join("proj-a");
    let (ws_a, task_id) =
        seed_ws_task(&client, &base_a, &tok_a, &proj_a, "e9", Some("planning")).await;

    let parent = rpc(
        &client,
        &base_a,
        Some(&tok_a),
        "agent.create",
        json!({ "taskId": task_id, "provider": "cli.generic", "role": "planner" }),
    )
    .await;
    let parent_id = rpc_id(&parent, "id");
    let child = rpc(
        &client,
        &base_a,
        Some(&tok_a),
        "agent.create",
        json!({
            "taskId": task_id,
            "provider": "cli.generic",
            "parentId": parent_id,
            "role": "coder"
        }),
    )
    .await;
    let child_id = rpc_id(&child, "id");

    let sent = rpc(
        &client,
        &base_a,
        Some(&tok_a),
        "agent.send",
        json!({ "agentId": parent_id, "content": "hello from A" }),
    )
    .await;
    assert!(sent.get("error").is_none(), "{sent}");
    wait_status(&client, &base_a, &tok_a, &parent_id, "idle").await;
    let ctx = rpc(
        &client,
        &base_a,
        Some(&tok_a),
        "agent.get_context",
        json!({ "agentId": parent_id }),
    )
    .await;
    let msg_id = ctx["ok"]["messages"][0]["id"]
        .as_str()
        .expect("message id")
        .to_string();

    let art = rpc(
        &client,
        &base_a,
        Some(&tok_a),
        "artifact.create",
        json!({
            "taskId": task_id,
            "kind": "spec",
            "title": "Auth",
            "body": "hello world",
            "sourceMessageId": msg_id
        }),
    )
    .await;
    assert!(art.get("error").is_none(), "{art}");
    let art_id = rpc_id(&art, "id");
    let comment = rpc(
        &client,
        &base_a,
        Some(&tok_a),
        "comment.create",
        json!({
            "artifactId": art_id,
            "anchorStart": 0,
            "anchorEnd": 5,
            "body": "nit"
        }),
    )
    .await;
    assert!(comment.get("error").is_none(), "{comment}");
    let thread_id = rpc_id(&comment, "id");

    let profile = rpc(
        &client,
        &base_a,
        Some(&tok_a),
        "profile.create",
        json!({
            "name": "e9-fast",
            "provider": "cli.generic",
            "model": "gpt",
            "fast": true
        }),
    )
    .await;
    assert!(profile.get("error").is_none(), "{profile}");
    let profile_id = rpc_id(&profile, "id");

    let exported = rpc(
        &client,
        &base_a,
        Some(&tok_a),
        "sync.export",
        json!({ "taskIds": [task_id] }),
    )
    .await;
    assert!(exported.get("error").is_none(), "{exported}");
    let archive = exported["ok"]["archive"].clone();
    assert_eq!(archive["kind"], "rusttraycer.export");
    assert_eq!(archive["exportVersion"], 1);
    assert_eq!(archive["sourceHostId"], host_a);
    assert_eq!(archive["tasks"][0]["id"], task_id);
    assert_eq!(archive["tasks"][0]["preset"], "planning");
    assert_eq!(archive["agents"].as_array().unwrap().len(), 2);
    assert!(archive.get("host").is_none());
    assert!(archive.get("workspaces").is_none());
    assert!(archive.get("worktrees").is_none());

    let json_files: Vec<_> = std::fs::read_dir(dir_a.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
        .collect();
    assert!(
        json_files.iter().all(|e| e.file_name() == "pid.json"),
        "host must not write the archive file: {json_files:?}"
    );

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
    let ws_b_id = rpc_id(&ws_b, "id");
    assert_ne!(ws_b_id, ws_a);

    let imported = rpc(
        &client,
        &base_b,
        Some(&tok_b),
        "sync.import",
        json!({ "workspaceId": ws_b_id, "archive": archive }),
    )
    .await;
    assert!(imported.get("error").is_none(), "{imported}");
    assert_eq!(imported["ok"]["tasks"], 1);
    assert_eq!(imported["ok"]["agents"], 2);
    assert!(imported["ok"]["messages"].as_u64().unwrap() >= 1);
    assert_eq!(imported["ok"]["artifacts"], 1);
    assert_eq!(imported["ok"]["profilesImported"], 1);
    assert_eq!(imported["ok"]["profilesSkipped"], 0);

    let got_task = rpc(
        &client,
        &base_b,
        Some(&tok_b),
        "task.get",
        json!({ "id": task_id }),
    )
    .await;
    assert_eq!(got_task["ok"]["id"], task_id);
    assert_eq!(got_task["ok"]["preset"], "planning");
    assert_eq!(got_task["ok"]["workspaceIds"][0], ws_b_id);

    let agents_b = rpc(
        &client,
        &base_b,
        Some(&tok_b),
        "agent.list",
        json!({ "taskId": task_id }),
    )
    .await;
    let items = agents_b["ok"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    for a in items {
        assert_eq!(a["hostId"], host_b, "{a}");
        assert_eq!(a["status"], "idle");
        assert_eq!(a["runLocation"], "local");
        assert!(a["providerSessionId"].is_null() || a.get("providerSessionId").is_none());
    }
    let parent_b = items.iter().find(|a| a["id"] == parent_id).unwrap();
    let child_b = items.iter().find(|a| a["id"] == child_id).unwrap();
    assert_eq!(parent_b["role"], "planner");
    assert_eq!(child_b["parentId"], parent_id);

    let ping_a = rpc(&client, &base_a, Some(&tok_a), "host.ping", json!({})).await;
    assert_eq!(ping_a["ok"]["hostId"], host_a);
    let ping_b = rpc(&client, &base_b, Some(&tok_b), "host.ping", json!({})).await;
    assert_eq!(ping_b["ok"]["hostId"], host_b);

    let ctx_b = rpc(
        &client,
        &base_b,
        Some(&tok_b),
        "agent.get_context",
        json!({ "agentId": parent_id }),
    )
    .await;
    assert_eq!(ctx_b["ok"]["messages"][0]["id"], msg_id);
    assert_eq!(ctx_b["ok"]["messages"][0]["content"], "hello from A");

    let art_b = rpc(
        &client,
        &base_b,
        Some(&tok_b),
        "artifact.get",
        json!({ "artifactId": art_id }),
    )
    .await;
    assert_eq!(art_b["ok"]["id"], art_id);
    assert_eq!(art_b["ok"]["sourceMessageId"], msg_id);

    let comments_b = rpc(
        &client,
        &base_b,
        Some(&tok_b),
        "comment.list",
        json!({ "artifactId": art_id }),
    )
    .await;
    assert_eq!(comments_b["ok"]["threads"][0]["id"], thread_id);
    assert_eq!(comments_b["ok"]["threads"][0]["comments"][0]["body"], "nit");

    let prof_b = rpc(
        &client,
        &base_b,
        Some(&tok_b),
        "profile.get",
        json!({ "profileId": profile_id }),
    )
    .await;
    assert_eq!(prof_b["ok"]["id"], profile_id);
    assert_eq!(prof_b["ok"]["name"], "e9-fast");

    let _ = tx_a.send(());
    let _ = tx_b.send(());
    let _ = join_a.await;
    let _ = join_b.await;
}

#[tokio::test]
async fn archive_omits_worktree_session_host_and_secrets() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, host_id) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f8_run_methods()).await;
    let proj = dir.path().join("proj");
    let (_ws, task_id) = seed_ws_task(&client, &base, &token, &proj, "e9-omit", None).await;
    let created = rpc(
        &client,
        &base,
        Some(&token),
        "agent.create",
        json!({ "taskId": task_id, "provider": "cli.generic" }),
    )
    .await;
    let agent_id = rpc_id(&created, "id");

    let db = dir.path().join("host.db");
    {
        let store = rt_storage::Store::open(&db).unwrap();
        store
            .agent_set_status(&agent_id, rt_storage::AgentStatus::Running)
            .unwrap();
    }

    let exported = rpc(
        &client,
        &base,
        Some(&token),
        "sync.export",
        json!({ "taskIds": [task_id] }),
    )
    .await;
    assert!(exported.get("error").is_none(), "{exported}");
    let archive = &exported["ok"]["archive"];
    assert_eq!(archive["sourceHostId"], host_id);
    assert!(archive.get("host").is_none());
    assert!(archive.get("workspaces").is_none());
    assert!(archive.get("worktrees").is_none());
    assert!(archive.get("loops").is_none());
    assert!(archive.get("policies").is_none());
    let agent = &archive["agents"][0];
    assert_eq!(agent["status"], "idle");
    assert_eq!(agent["runLocation"], "local");
    assert!(agent.get("providerSessionId").is_none());
    assert!(agent.get("hostId").is_none());
    assert!(archive["tasks"][0].get("workspaceIds").is_none());

    let mut keys = Vec::new();
    collect_keys(archive, &mut keys);
    for k in &keys {
        let low = k.to_ascii_lowercase();
        assert_ne!(low, "token", "{keys:?}");
        assert_ne!(low, "pat", "{keys:?}");
        assert_ne!(low, "password", "{keys:?}");
        assert_ne!(low, "keyring", "{keys:?}");
        assert_ne!(low, "providersessionid", "{keys:?}");
        assert_ne!(low, "worktrees", "{keys:?}");
    }
    let raw = serde_json::to_string(archive).unwrap();
    let raw_l = raw.to_ascii_lowercase();
    for needle in ["token", "password", "keyring"] {
        assert!(
            !raw_l.contains(needle),
            "archive JSON must not contain {needle}: {raw}"
        );
    }

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn reimport_same_archive_is_conflict() {
    let dir_a = tempfile::tempdir().unwrap();
    let dir_b = tempfile::tempdir().unwrap();
    let (addr_a, tx_a, join_a, _) = rt_host::spawn_test_host(dir_a.path(), Some(backends()))
        .await
        .unwrap();
    let (addr_b, tx_b, join_b, _) = rt_host::spawn_test_host(dir_b.path(), Some(backends()))
        .await
        .unwrap();
    let base_a = format!("http://{addr_a}");
    let base_b = format!("http://{addr_b}");
    let client = reqwest::Client::new();
    let (tok_a, _) = handshake(&client, &base_a, f8_run_methods()).await;
    let (tok_b, _) = handshake(&client, &base_b, f8_run_methods()).await;
    let (_ws_a, task_id) = seed_ws_task(
        &client,
        &base_a,
        &tok_a,
        &dir_a.path().join("proj-a"),
        "e9-re",
        None,
    )
    .await;
    let _ = rpc(
        &client,
        &base_a,
        Some(&tok_a),
        "agent.create",
        json!({ "taskId": task_id, "provider": "cli.generic" }),
    )
    .await;
    let exported = rpc(
        &client,
        &base_a,
        Some(&tok_a),
        "sync.export",
        json!({ "taskIds": [task_id] }),
    )
    .await;
    let archive = exported["ok"]["archive"].clone();

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
    let ws_b_id = rpc_id(&ws_b, "id");
    let first = rpc(
        &client,
        &base_b,
        Some(&tok_b),
        "sync.import",
        json!({ "workspaceId": ws_b_id, "archive": archive }),
    )
    .await;
    assert!(first.get("error").is_none(), "{first}");

    let tasks_before = rpc(
        &client,
        &base_b,
        Some(&tok_b),
        "task.list",
        json!({ "status": "all" }),
    )
    .await;
    let agents_before = rpc(
        &client,
        &base_b,
        Some(&tok_b),
        "agent.list",
        json!({ "taskId": task_id }),
    )
    .await;
    let n_tasks = tasks_before["ok"]["items"].as_array().unwrap().len();
    let n_agents = agents_before["ok"]["items"].as_array().unwrap().len();

    let second = rpc(
        &client,
        &base_b,
        Some(&tok_b),
        "sync.import",
        json!({ "workspaceId": ws_b_id, "archive": archive }),
    )
    .await;
    assert_eq!(second["error"]["code"], "conflict", "{second}");

    let tasks_after = rpc(
        &client,
        &base_b,
        Some(&tok_b),
        "task.list",
        json!({ "status": "all" }),
    )
    .await;
    let agents_after = rpc(
        &client,
        &base_b,
        Some(&tok_b),
        "agent.list",
        json!({ "taskId": task_id }),
    )
    .await;
    assert_eq!(
        tasks_after["ok"]["items"].as_array().unwrap().len(),
        n_tasks
    );
    assert_eq!(
        agents_after["ok"]["items"].as_array().unwrap().len(),
        n_agents
    );

    let same_host = rpc(
        &client,
        &base_a,
        Some(&tok_a),
        "sync.import",
        json!({ "workspaceId": archive["tasks"][0]["id"], "archive": archive }),
    )
    .await;
    // dest workspace must exist; using a task id as workspace is not_found.
    // Re-import on A with A's real workspace is conflict (same ids).
    let list_ws = rpc(&client, &base_a, Some(&tok_a), "workspace.list", json!({})).await;
    let ws_a_id = list_ws["ok"]["items"][0]["id"].as_str().unwrap();
    let same = rpc(
        &client,
        &base_a,
        Some(&tok_a),
        "sync.import",
        json!({ "workspaceId": ws_a_id, "archive": archive }),
    )
    .await;
    assert_eq!(same["error"]["code"], "conflict", "{same}");
    let _ = same_host;

    let _ = tx_a.send(());
    let _ = tx_b.send(());
    let _ = join_a.await;
    let _ = join_b.await;
}

#[tokio::test]
async fn import_missing_or_unknown_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f8_run_methods()).await;
    let (_ws, task_id) = seed_ws_task(
        &client,
        &base,
        &token,
        &dir.path().join("proj"),
        "e9-ws",
        None,
    )
    .await;
    let exported = rpc(
        &client,
        &base,
        Some(&token),
        "sync.export",
        json!({ "taskIds": [task_id] }),
    )
    .await;
    let archive = exported["ok"]["archive"].clone();

    let missing = rpc(
        &client,
        &base,
        Some(&token),
        "sync.import",
        json!({ "archive": archive }),
    )
    .await;
    assert_eq!(missing["error"]["code"], "invalid_params", "{missing}");

    let empty = rpc(
        &client,
        &base,
        Some(&token),
        "sync.import",
        json!({ "workspaceId": "", "archive": archive }),
    )
    .await;
    assert_eq!(empty["error"]["code"], "invalid_params", "{empty}");

    let unknown = rpc(
        &client,
        &base,
        Some(&token),
        "sync.import",
        json!({ "workspaceId": "00000000-0000-0000-0000-000000000000", "archive": archive }),
    )
    .await;
    assert_eq!(unknown["error"]["code"], "not_found", "{unknown}");

    let empty_ids = rpc(
        &client,
        &base,
        Some(&token),
        "sync.export",
        json!({ "taskIds": [] }),
    )
    .await;
    assert_eq!(empty_ids["error"]["code"], "invalid_params", "{empty_ids}");

    let dups = rpc(
        &client,
        &base,
        Some(&token),
        "sync.export",
        json!({ "taskIds": [task_id, task_id] }),
    )
    .await;
    assert_eq!(dups["error"]["code"], "invalid_params", "{dups}");

    let missing_task = rpc(
        &client,
        &base,
        Some(&token),
        "sync.export",
        json!({ "taskIds": ["00000000-0000-0000-0000-000000000000"] }),
    )
    .await;
    assert_eq!(missing_task["error"]["code"], "not_found", "{missing_task}");

    let too_many: Vec<String> = (0..33).map(|i| format!("t{i}")).collect();
    let over = rpc(
        &client,
        &base,
        Some(&token),
        "sync.export",
        json!({ "taskIds": too_many }),
    )
    .await;
    assert_eq!(over["error"]["code"], "invalid_params", "{over}");

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn client_without_1_8_keeps_guides_sync_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, hs) = handshake(&client, &base, f7_run_methods()).await;
    assert!(hs["ok"]["accepted"].get("sync.export").is_none());
    assert!(hs["ok"]["accepted"].get("workspace.guides.get").is_some());
    assert!(hs["ok"]["accepted"].get("agent.create").is_some());
    assert!(hs["ok"]["accepted"].get("agent.send").is_some());

    let (ws_id, task_id) = seed_ws_task(
        &client,
        &base,
        &token,
        &dir.path().join("proj"),
        "e9-old",
        None,
    )
    .await;
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
    let guides = rpc(
        &client,
        &base,
        Some(&token),
        "workspace.guides.get",
        json!({ "workspaceId": ws_id }),
    )
    .await;
    assert!(guides.get("error").is_none(), "{guides}");

    let exp = rpc(
        &client,
        &base,
        Some(&token),
        "sync.export",
        json!({ "taskIds": [task_id] }),
    )
    .await;
    assert_eq!(exp["error"]["code"], "version_mismatch", "{exp}");

    let _ = tx.send(());
    let _ = join.await;
}

#[test]
fn migrations_0001_to_0008_byte_identical_to_freeze() {
    let root = repo_root();
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
}
