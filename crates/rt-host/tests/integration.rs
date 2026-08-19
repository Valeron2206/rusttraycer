//! End-to-end: handshake, files.*, session, pid lock.

use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures::Stream;
use rt_runtime::{AgentBackend, Availability, TurnEvent, TurnRequest};
use serde_json::{json, Value};

struct EchoBackend;

impl AgentBackend for EchoBackend {
    fn id(&self) -> &'static str {
        "cli.generic"
    }
    fn available(&self) -> Availability {
        Availability {
            available: true,
            detail: "mock echo".into(),
        }
    }
    fn start_turn(&self, _req: TurnRequest) -> Pin<Box<dyn Stream<Item = TurnEvent> + Send>> {
        Box::pin(futures::stream::iter([
            TurnEvent::Token {
                text: "hello\n".into(),
            },
            TurnEvent::Finished { exit_code: 0 },
        ]))
    }
}

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
async fn host_loop_echo_files_and_busy() {
    let dir = tempfile::tempdir().unwrap();
    let mut backends = std::collections::HashMap::new();
    let backend: Arc<dyn AgentBackend> = Arc::new(EchoBackend);
    backends.insert("cli.generic".into(), backend);

    let (addr, shutdown, join, host_id) = rt_host::spawn_test_host(dir.path(), Some(backends))
        .await
        .unwrap();
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

    let denied = rpc(&client, &base, None, "workspace.list", json!({})).await;
    assert_eq!(denied["error"]["code"], "unauthorized");

    let ws_status = client
        .get(format!("{base}/ws"))
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
        .send()
        .await
        .unwrap()
        .status();
    assert!(
        matches!(ws_status.as_u16(), 400 | 401),
        "ws status={ws_status}"
    );

    let hs = rpc(
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
    assert!(
        hs["ok"]["accepted"]["task.create"].is_object(),
        "handshake={hs}"
    );
    assert!(hs["ok"]["accepted"]["files.tree"].is_object(), "{hs}");
    let token = hs["ok"]["sessionToken"].as_str().unwrap().to_string();

    assert!(hs["ok"]["accepted"]["agent.cancel"].is_object(), "{hs}");
    let cancel_missing = rpc(&client, &base, Some(&token), "agent.cancel", json!({})).await;
    assert_eq!(cancel_missing["error"]["code"], "invalid_params");
    let cancel_bad = rpc(
        &client,
        &base,
        Some(&token),
        "agent.cancel",
        json!({ "agentId": "not-a-uuid" }),
    )
    .await;
    assert_eq!(cancel_bad["error"]["code"], "invalid_params");
    let cancel_missing_agent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.cancel",
        json!({ "agentId": "0191f0c6-cccc-7000-8000-000000000003" }),
    )
    .await;
    assert_eq!(cancel_missing_agent["error"]["code"], "not_found");

    let empty_hs = rpc(
        &client,
        &base,
        None,
        "handshake",
        json!({
            "client": "gui",
            "clientVersion": "0.1.0",
            "methods": {}
        }),
    )
    .await;
    let empty_tok = empty_hs["ok"]["sessionToken"].as_str().unwrap().to_string();
    let mismatch = rpc(
        &client,
        &base,
        Some(&empty_tok),
        "files.tree",
        json!({"workspaceId":"x"}),
    )
    .await;
    assert_eq!(mismatch["error"]["code"], "version_mismatch");

    let ws_dir = dir.path().join("proj");
    std::fs::create_dir(&ws_dir).unwrap();
    std::fs::write(ws_dir.join("README.md"), b"# proj\n").unwrap();
    std::fs::create_dir(ws_dir.join("src")).unwrap();
    std::fs::write(ws_dir.join("src/main.rs"), b"fn main() {}\n").unwrap();

    let ws = rpc(
        &client,
        &base,
        Some(&token),
        "workspace.add",
        json!({ "path": ws_dir.to_str().unwrap() }),
    )
    .await;
    let ws_id = ws["ok"]["id"]
        .as_str()
        .unwrap_or_else(|| panic!("ws={ws}"))
        .to_string();
    assert_eq!(ws["ok"]["name"], "proj");

    let again = rpc(
        &client,
        &base,
        Some(&token),
        "workspace.add",
        json!({ "path": ws_dir.to_str().unwrap() }),
    )
    .await;
    assert_eq!(again["ok"]["id"], ws_id);

    let tree = rpc(
        &client,
        &base,
        Some(&token),
        "files.tree",
        json!({ "workspaceId": ws_id, "depth": 2 }),
    )
    .await;
    let items = tree["ok"]["items"]
        .as_array()
        .unwrap_or_else(|| panic!("tree={tree}"));
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
    assert_eq!(read["ok"]["content"], "# proj\n");
    assert_eq!(read["ok"]["encoding"], "utf8");
    assert_eq!(read["ok"]["truncated"], false);

    let task = rpc(
        &client,
        &base,
        Some(&token),
        "task.create",
        json!({ "title": "Demo", "workspaceId": ws_id }),
    )
    .await;
    let task_id = task["ok"]["id"].as_str().unwrap().to_string();

    let missing_task = rpc(
        &client,
        &base,
        Some(&token),
        "agent.list",
        json!({ "taskId": "0191f0c6-7c2a-7c11-8000-6f0c1a2b3c4d" }),
    )
    .await;
    assert_eq!(missing_task["error"]["code"], "not_found");

    let agent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.create",
        json!({ "taskId": task_id }),
    )
    .await;
    let agent_id = agent["ok"]["id"]
        .as_str()
        .unwrap_or_else(|| panic!("agent={agent}"))
        .to_string();
    assert_eq!(agent["ok"]["interface"], "chat");
    assert_eq!(agent["ok"]["provider"], "cli.generic");
    assert_eq!(agent["ok"]["runLocation"], "local");
    assert!(agent["ok"]["parentId"].is_null());

    let got = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get",
        json!({ "id": agent_id }),
    )
    .await;
    assert!(got["ok"].get("lastMessageAt").is_some(), "agent.get={got}");
    assert!(got["ok"]["lastMessageAt"].is_null());

    let sent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.send",
        json!({ "agentId": agent_id, "content": "hi there" }),
    )
    .await;
    assert_eq!(sent["ok"]["userMessage"]["role"], "user");
    assert_eq!(sent["ok"]["userMessage"]["content"], "hi there");

    let mut idle = false;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let got = rpc(
            &client,
            &base,
            Some(&token),
            "agent.get",
            json!({ "id": agent_id }),
        )
        .await;
        if got["ok"]["status"] == "idle" {
            idle = true;
            assert!(got["ok"]["lastMessageAt"].is_string());
            break;
        }
    }
    assert!(idle, "agent did not return to idle");

    let idle_cancel = rpc(
        &client,
        &base,
        Some(&token),
        "agent.cancel",
        json!({ "agentId": agent_id }),
    )
    .await;
    assert_eq!(idle_cancel["ok"]["agentId"], agent_id);
    assert_eq!(idle_cancel["ok"]["cancelled"], false);

    let ctx = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get_context",
        json!({ "agentId": agent_id }),
    )
    .await;
    let msgs = ctx["ok"]["messages"].as_array().unwrap();
    assert!(msgs
        .iter()
        .any(|m| m["role"] == "user" && m["content"] == "hi there"));
    assert!(msgs
        .iter()
        .any(|m| m["role"] == "assistant" && m["content"].as_str().unwrap().contains("hello")));

    let doctor = rpc(&client, &base, Some(&token), "host.doctor", json!({})).await;
    assert_eq!(doctor["ok"]["hostId"], host_id);
    assert_eq!(doctor["ok"]["workspaceCount"], 1);
    assert_eq!(doctor["ok"]["taskCount"], 1);
    assert_eq!(doctor["ok"]["agentCount"], 1);
    assert_eq!(doctor["ok"]["dbOk"], true);
    assert_eq!(doctor["ok"]["providers"][0]["id"], "cli.generic");

    let _ = shutdown.send(());
    let _ = join.await;
}

#[tokio::test]
async fn handshake_version_reject_via_rpc() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, shutdown, join, _) = rt_host::spawn_test_host(dir.path(), None).await.unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let hs = rpc(
        &client,
        &base,
        None,
        "handshake",
        json!({
            "client": "gui",
            "clientVersion": "0.1.0",
            "methods": {
                "task.create": {"major": 2, "minor": 0},
                "host.ping": {"major": 1, "minor": 9}
            }
        }),
    )
    .await;
    assert_eq!(
        hs["ok"]["rejected"]["task.create"]["reason"],
        "version_mismatch"
    );
    assert_eq!(
        hs["ok"]["rejected"]["host.ping"]["reason"],
        "version_mismatch"
    );
    let _ = shutdown.send(());
    let _ = join.await;
}

#[tokio::test]
async fn second_prepare_fails_already_running() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, shutdown, join, _) = rt_host::spawn_test_host(dir.path(), None).await.unwrap();
    assert!(addr.ip().is_loopback());

    // Same-process prepare sees our own pid and is allowed; simulate a foreign live host.
    let mut info = rt_host::bind::read_pid_file(dir.path()).unwrap().unwrap();
    info.pid = 1; // init, always alive
    rt_host::bind::write_pid_file(dir.path(), &info).unwrap();

    let err = match rt_host::prepare(rt_host::HostConfig {
        data_dir: dir.path().to_path_buf(),
        init_tracing: false,
        backends: None,
    })
    .await
    {
        Err(e) => e,
        Ok(_) => panic!("expected already_running"),
    };
    assert_eq!(err.code(), "already_running");
    assert_eq!(err.exit_code(), 2);
    let _ = shutdown.send(());
    let _ = join.await;
}
