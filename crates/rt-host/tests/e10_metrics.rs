//! C59: loopback `GET /metrics` (Prometheus text 0.0.4). Not RPC.
//! Client 1.8 (`/health`, handshake, TRADABLE_METHODS) stays unchanged.

use std::collections::HashMap;
use std::path::PathBuf;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use rt_protocol::TRADABLE_METHODS;
use rt_storage::{AgentStatus, Store};
use serde_json::{json, Value};
use tempfile::tempdir;
use tower::ServiceExt;

fn test_service() -> (tempfile::TempDir, rt_host::service::HostService) {
    let dir = tempdir().unwrap();
    let store = Store::open(dir.path().join("host.db")).unwrap();
    store.migrate().unwrap();
    let host_id = rt_storage::new_id();
    store.host_insert_if_absent(&host_id, "test").unwrap();
    let svc = rt_host::service::HostService::new(
        store,
        HashMap::new(),
        host_id,
        dir.path().to_path_buf(),
        "http://127.0.0.1:0".into(),
        std::process::id(),
    );
    (dir, svc)
}

async fn oneshot(
    app: axum::Router,
    req: Request<Body>,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let res = app.oneshot(req).await.unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20)
        .await
        .unwrap();
    (status, headers, bytes.to_vec())
}

fn content_type(headers: &axum::http::HeaderMap) -> String {
    headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string()
}

fn series(body: &str, name: &str) -> bool {
    body.lines().any(|l| l == name)
}

#[tokio::test]
async fn metrics_get_without_token_is_prometheus_text() {
    let (_dir, svc) = test_service();
    let app = rt_host::rpc::router(svc);
    let (st, headers, bytes) = oneshot(
        app,
        Request::builder()
            .uri("/metrics")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let ct = content_type(&headers);
    assert!(ct.contains("text/plain"), "content-type={ct}");
    assert!(ct.contains("0.0.4"), "content-type={ct}");
    let body = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(
        body.contains("rusttraycer_up 1"),
        "missing rusttraycer_up 1:\n{body}"
    );
    assert!(series(&body, r#"rusttraycer_agents{status="idle"} 0"#));
    assert!(series(&body, r#"rusttraycer_agents{status="running"} 0"#));
    assert!(series(&body, r#"rusttraycer_agents{status="error"} 0"#));
    assert!(series(&body, r#"rusttraycer_tasks{status="open"} 0"#));
    assert!(series(&body, r#"rusttraycer_tasks{status="archived"} 0"#));
}

#[tokio::test]
async fn metrics_counts_follow_agent_and_task_status() {
    let (dir, svc) = test_service();
    let host_id = svc.host_id().to_string();

    let ws_dir = dir.path().join("proj");
    std::fs::create_dir(&ws_dir).unwrap();
    let ws = svc.workspace_add(ws_dir.to_str().unwrap()).unwrap();
    let open_task = svc.task_create("open-one", &ws.id).unwrap();
    let archived_task = svc.task_create("to-archive", &ws.id).unwrap();
    svc.task_archive(&archived_task.id).unwrap();

    let _idle = svc.agent_create(&open_task.id, None).unwrap();
    let running = svc.agent_create(&open_task.id, None).unwrap();
    let errored = svc.agent_create(&open_task.id, None).unwrap();
    svc.store
        .agent_set_status(&running.id, AgentStatus::Running)
        .unwrap();
    svc.store
        .agent_set_status(&errored.id, AgentStatus::Error)
        .unwrap();

    let app = rt_host::rpc::router(svc.clone());
    let (st, headers, bytes) = oneshot(
        app,
        Request::builder()
            .uri("/metrics")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let ct = content_type(&headers);
    assert!(ct.contains("text/plain") && ct.contains("0.0.4"), "{ct}");
    let body = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(body.contains("rusttraycer_up 1"), "{body}");
    assert!(
        series(&body, r#"rusttraycer_agents{status="idle"} 1"#),
        "{body}"
    );
    assert!(
        series(&body, r#"rusttraycer_agents{status="running"} 1"#),
        "{body}"
    );
    assert!(
        series(&body, r#"rusttraycer_agents{status="error"} 1"#),
        "{body}"
    );
    assert!(
        series(&body, r#"rusttraycer_tasks{status="open"} 1"#),
        "{body}"
    );
    assert!(
        series(&body, r#"rusttraycer_tasks{status="archived"} 1"#),
        "{body}"
    );
    assert!(
        !body.contains(&host_id),
        "hostId must not appear in labels/body: {body}"
    );
    assert!(
        !body.contains(dir.path().to_str().unwrap()),
        "paths must not appear: {body}"
    );
}

#[tokio::test]
async fn metrics_post_is_not_json_rpc() {
    let (_dir, svc) = test_service();
    let app = rt_host::rpc::router(svc);
    let (st, _headers, bytes) = oneshot(
        app,
        Request::builder()
            .method("POST")
            .uri("/metrics")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(&json!({
                    "id": "1",
                    "method": "host.ping",
                    "params": {}
                }))
                .unwrap(),
            ))
            .unwrap(),
    )
    .await;
    assert!(
        st == StatusCode::NOT_FOUND || st == StatusCode::METHOD_NOT_ALLOWED,
        "POST /metrics must be 404/405, got {st}"
    );
    let body = String::from_utf8_lossy(&bytes);
    let parsed: Result<Value, _> = serde_json::from_slice(&bytes);
    if let Ok(v) = parsed {
        assert!(
            v.get("ok").is_none() && v.get("id").is_none(),
            "POST /metrics must not be JSON-RPC: {v}"
        );
    }
    assert!(
        !body.contains("\"method\"") || st != StatusCode::OK,
        "POST /metrics leaked RPC: {body}"
    );
}

#[tokio::test]
async fn client_1_8_health_handshake_unchanged() {
    assert_eq!(TRADABLE_METHODS.len(), 69);
    assert!(!TRADABLE_METHODS.iter().any(|m| m.contains("metrics")));
    assert!(TRADABLE_METHODS.contains(&"sync.export"));
    assert!(TRADABLE_METHODS.contains(&"sync.import"));

    let dir = tempdir().unwrap();
    let (addr, tx, join, host_id) = rt_host::spawn_test_host(dir.path(), None).await.unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();

    let health = client.get(format!("{base}/health")).send().await.unwrap();
    assert_eq!(health.status(), reqwest::StatusCode::OK);
    let health_json: Value = health.json().await.unwrap();
    assert_eq!(health_json["ok"], true);
    assert_eq!(health_json["hostId"], host_id);

    let metrics = client.get(format!("{base}/metrics")).send().await.unwrap();
    assert_eq!(metrics.status(), reqwest::StatusCode::OK);
    let ct = metrics
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(ct.contains("text/plain") && ct.contains("0.0.4"), "{ct}");
    let metrics_body = metrics.text().await.unwrap();
    assert!(metrics_body.contains("rusttraycer_up 1"), "{metrics_body}");

    let hello: Value = client
        .post(format!("{base}/rpc"))
        .json(&json!({
            "id": "hs",
            "method": "handshake",
            "params": {
                "client": "cli",
                "clientVersion": "0.1.0",
                "methods": {
                    "host.ping": { "major": 1, "minor": 0 },
                    "host.doctor": { "major": 1, "minor": 0 },
                    "sync.export": { "major": 1, "minor": 8 },
                    "sync.import": { "major": 1, "minor": 8 }
                }
            }
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(hello["ok"]["sessionToken"].as_str().is_some());
    assert_eq!(hello["ok"]["accepted"]["sync.export"]["minor"], 8);
    assert_eq!(hello["ok"]["accepted"]["sync.import"]["minor"], 8);
    assert!(hello["ok"]["accepted"].get("metrics").is_none());
    assert!(hello["ok"]["rejected"].get("metrics").is_none());

    let token = hello["ok"]["sessionToken"].as_str().unwrap();
    let doctor: Value = client
        .post(format!("{base}/rpc"))
        .header("X-Rt-Session", token)
        .json(&json!({
            "id": "d",
            "method": "host.doctor",
            "params": {}
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let doc = &doctor["ok"];
    assert!(doc.get("rusttraycer_up").is_none());
    assert!(doc.get("rusttraycer_agents").is_none());
    assert!(doc.get("rusttraycer_tasks").is_none());
    assert_eq!(doc["hostId"], host_id);

    drop(tx);
    let _ = join.await;
}

#[tokio::test]
async fn health_get_still_ok() {
    let (_dir, svc) = test_service();
    let host_id = svc.host_id().to_string();
    let app = rt_host::rpc::router(svc);
    let (st, _headers, bytes) = oneshot(
        app,
        Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["hostId"], host_id);
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn migrations_0001_to_0008_byte_identical_vs_6433bfb() {
    let root = repo_root();
    let names = [
        "0001_init.sql",
        "0002_worktrees.sql",
        "0003_policies.sql",
        "0004_terminal.sql",
        "0005_artifacts.sql",
        "0006_loops.sql",
        "0007_model_ux.sql",
        "0008_workspace.sql",
    ];
    for name in names {
        let path = root.join("crates/rt-storage/migrations").join(name);
        let current = std::fs::read(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
        let out = std::process::Command::new("git")
            .args([
                "show",
                &format!("6433bfb:crates/rt-storage/migrations/{name}"),
            ])
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git show 6433bfb:{} failed: {}",
            name,
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            current, out.stdout,
            "{name} must be byte-identical to 6433bfb"
        );
    }
}
