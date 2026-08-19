use serde_json::{json, Value};
use tempfile::tempdir;

fn all_methods() -> Value {
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

#[tokio::test]
async fn health_handshake_doctor_files() {
    let dir = tempdir().unwrap();
    let (addr, tx, join, host_id) = rt_host::spawn_test_host(dir.path(), None).await.unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();

    let health: Value = client
        .get(format!("{base}/health"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(health["ok"], true);
    assert_eq!(health["hostId"], host_id);

    let ping = rpc(&client, &base, None, "host.ping", json!({})).await;
    assert_eq!(ping["ok"]["hostId"], host_id);

    let bad = rpc(
        &client,
        &base,
        None,
        "handshake",
        json!({
            "client": "cli",
            "clientVersion": "0.1.0",
            "methods": { "task.create": { "major": 2, "minor": 0 } }
        }),
    )
    .await;
    assert_eq!(
        bad["ok"]["rejected"]["task.create"]["reason"],
        "version_mismatch"
    );
    assert!(bad["ok"]["accepted"].as_object().unwrap().is_empty());

    let hello = rpc(
        &client,
        &base,
        None,
        "handshake",
        json!({
            "client": "cli",
            "clientVersion": "0.1.0",
            "methods": all_methods()
        }),
    )
    .await;
    let token = hello["ok"]["sessionToken"].as_str().unwrap().to_string();
    assert!(hello["ok"]["accepted"].get("files.tree").is_some());
    assert!(hello["ok"]["accepted"].get("host.doctor").is_some());
    assert!(hello["ok"]["accepted"].get("worktree.ensure").is_some());
    assert!(hello["ok"]["accepted"].get("worktree.get").is_some());
    assert!(hello["ok"]["accepted"].get("worktree.list").is_some());
    assert!(hello["ok"]["accepted"].get("git.status").is_some());
    assert!(hello["ok"]["accepted"].get("git.diff").is_some());
    assert!(hello["ok"]["accepted"].get("agent.cancel").is_some());

    let doctor = rpc(&client, &base, Some(&token), "host.doctor", json!({})).await;
    let providers = doctor["ok"]["providers"].as_array().unwrap();
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0]["id"], "cli.generic");
    let env_set = std::env::var("RUSTTRAYCER_GENERIC_CMD")
        .ok()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    assert_eq!(providers[0]["available"], env_set);

    let ws_dir = dir.path().join("proj");
    std::fs::create_dir(&ws_dir).unwrap();
    std::fs::write(ws_dir.join("README.md"), b"# hi\n").unwrap();
    std::fs::create_dir(ws_dir.join("src")).unwrap();
    std::fs::write(ws_dir.join("src/main.rs"), b"fn main() {}\n").unwrap();

    let added = rpc(
        &client,
        &base,
        Some(&token),
        "workspace.add",
        json!({ "path": ws_dir.to_str().unwrap() }),
    )
    .await;
    let ws_id = added["ok"]["id"].as_str().unwrap();

    let tree = rpc(
        &client,
        &base,
        Some(&token),
        "files.tree",
        json!({ "workspaceId": ws_id, "depth": 2 }),
    )
    .await;
    let items = tree["ok"]["items"].as_array().unwrap();
    assert!(items.iter().any(|e| e["path"] == "README.md"));
    assert!(items.iter().any(|e| e["path"] == "src/main.rs"));

    let read = rpc(
        &client,
        &base,
        Some(&token),
        "files.read",
        json!({ "workspaceId": ws_id, "path": "README.md" }),
    )
    .await;
    assert_eq!(read["ok"]["content"], "# hi\n");
    assert_eq!(read["ok"]["encoding"], "utf8");

    let _ = tx.send(());
    let _ = join.await;
}
