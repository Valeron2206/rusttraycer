//! Automated README cycle against real `cli.generic` and `cli.codex` adapters.
//! EchoBackend in `integration.rs` does not satisfy this STAR.

use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use rt_runtime::{AgentBackend, CliCodex, CliGeneric};
use serde_json::{json, Value};

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

fn script_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/scripts")
        .join(name)
}

fn ensure_executable(path: &Path) {
    let mut perms = std::fs::metadata(path)
        .unwrap_or_else(|e| panic!("stat {}: {e}", path.display()))
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms)
        .unwrap_or_else(|e| panic!("chmod {}: {e}", path.display()));
}

async fn handshake(client: &reqwest::Client, base: &str) -> String {
    let hs = rpc(
        client,
        base,
        None,
        "handshake",
        json!({
            "client": "cli",
            "clientVersion": "0.1.0",
            "methods": all_methods()
        }),
    )
    .await;
    hs["ok"]["sessionToken"]
        .as_str()
        .unwrap_or_else(|| panic!("handshake={hs}"))
        .to_string()
}

async fn wait_idle(client: &reqwest::Client, base: &str, token: &str, agent_id: &str) {
    let mut last = Value::Null;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        last = rpc(
            client,
            base,
            Some(token),
            "agent.get",
            json!({ "id": agent_id }),
        )
        .await;
        if last["ok"]["status"] == "idle" {
            return;
        }
    }
    panic!("agent did not return to idle, last={last}");
}

fn rpc_id(resp: &Value, field: &str) -> String {
    resp["ok"][field]
        .as_str()
        .unwrap_or_else(|| panic!("{field} missing: {resp}"))
        .to_string()
}

async fn add_proj_and_task(
    client: &reqwest::Client,
    base: &str,
    token: &str,
    proj: &Path,
    title: &str,
) -> String {
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
        json!({ "title": title, "workspaceId": ws_id }),
    )
    .await;
    rpc_id(&task, "id")
}

fn assert_sqlite_transcript(data_dir: &Path, agent_id: &str, user: &str, assistant_needle: &str) {
    let store = rt_storage::Store::open(data_dir.join("host.db")).unwrap();
    let rows = store.message_list(agent_id).unwrap();
    assert!(
        rows.iter()
            .any(|m| m.role.as_str() == "user" && m.content == user),
        "sqlite missing user {user:?}: {rows:?}"
    );
    assert!(
        rows.iter()
            .any(|m| m.role.as_str() == "assistant" && m.content.contains(assistant_needle)),
        "sqlite missing assistant {assistant_needle:?}: {rows:?}"
    );
}

#[tokio::test]
async fn e2e_readme_cycle_cli_generic() {
    let script = script_path("fake_generic.py");
    ensure_executable(&script);

    let dir = tempfile::tempdir().unwrap();
    let generic = CliGeneric::new(script.to_string_lossy().into_owned());
    let mut backends = HashMap::new();
    backends.insert(
        generic.id().to_string(),
        Arc::new(generic) as Arc<dyn AgentBackend>,
    );

    let (addr, shutdown, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let token = handshake(&client, &base).await;

    let task_id = add_proj_and_task(
        &client,
        &base,
        &token,
        &dir.path().join("proj"),
        "generic cycle",
    )
    .await;

    let agent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.create",
        json!({ "taskId": task_id }),
    )
    .await;
    let agent_id = rpc_id(&agent, "id");
    assert_eq!(agent["ok"]["provider"], "cli.generic");

    let sent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.send",
        json!({ "agentId": agent_id, "content": "ping-generic" }),
    )
    .await;
    assert_eq!(sent["ok"]["userMessage"]["content"], "ping-generic");

    wait_idle(&client, &base, &token, &agent_id).await;

    let ctx = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get_context",
        json!({ "agentId": agent_id }),
    )
    .await;
    let msgs = ctx["ok"]["messages"]
        .as_array()
        .unwrap_or_else(|| panic!("ctx={ctx}"));
    assert!(
        msgs.iter()
            .any(|m| m["role"] == "user" && m["content"] == "ping-generic"),
        "ctx={ctx}"
    );
    assert!(
        msgs.iter().any(|m| m["role"] == "assistant"
            && m["content"].as_str().unwrap_or("").contains("ping-generic")),
        "ctx={ctx}"
    );

    let _ = shutdown.send(());
    let _ = join.await;

    assert_sqlite_transcript(dir.path(), &agent_id, "ping-generic", "ping-generic");
}

#[tokio::test]
async fn e2e_readme_cycle_cli_codex() {
    let script = script_path("fake_codex.py");
    ensure_executable(&script);

    let dir = tempfile::tempdir().unwrap();
    let codex = CliCodex::new(script.to_string_lossy().into_owned());
    let mut backends = HashMap::new();
    backends.insert(
        codex.id().to_string(),
        Arc::new(codex) as Arc<dyn AgentBackend>,
    );

    let (addr, shutdown, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let token = handshake(&client, &base).await;

    let task_id = add_proj_and_task(
        &client,
        &base,
        &token,
        &dir.path().join("proj"),
        "codex cycle",
    )
    .await;

    let agent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.create",
        json!({ "taskId": task_id, "provider": "cli.codex" }),
    )
    .await;
    let agent_id = rpc_id(&agent, "id");
    assert_eq!(agent["ok"]["provider"], "cli.codex");

    let sent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.send",
        json!({ "agentId": agent_id, "content": "ping-codex" }),
    )
    .await;
    assert_eq!(sent["ok"]["userMessage"]["content"], "ping-codex");

    wait_idle(&client, &base, &token, &agent_id).await;

    let ctx = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get_context",
        json!({ "agentId": agent_id }),
    )
    .await;
    let msgs = ctx["ok"]["messages"]
        .as_array()
        .unwrap_or_else(|| panic!("ctx={ctx}"));
    assert!(
        msgs.iter()
            .any(|m| m["role"] == "user" && m["content"] == "ping-codex"),
        "ctx={ctx}"
    );
    assert!(
        msgs.iter().any(|m| m["role"] == "assistant"
            && m["content"].as_str().unwrap_or("").contains("codex-ok")),
        "ctx={ctx}"
    );

    let _ = shutdown.send(());
    let _ = join.await;

    assert_sqlite_transcript(dir.path(), &agent_id, "ping-codex", "codex-ok");
}
