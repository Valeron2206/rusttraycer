//! V1 host slice: C42 PDF export + nested AGENTS.md walk (protocol 1.9, no 0009).

use std::path::Path;
use std::pin::Pin;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::Stream;
use rt_runtime::{AgentBackend, Availability, TurnEvent, TurnRequest};
use serde_json::{json, Value};

#[derive(Clone)]
struct CaptureGeneric {
    last: Arc<Mutex<Option<TurnRequest>>>,
}

impl AgentBackend for CaptureGeneric {
    fn id(&self) -> &'static str {
        "cli.generic"
    }
    fn available(&self) -> Availability {
        Availability {
            available: true,
            detail: "capture generic".into(),
        }
    }
    fn start_turn(&self, req: TurnRequest) -> Pin<Box<dyn Stream<Item = TurnEvent> + Send>> {
        if let Ok(mut g) = self.last.lock() {
            *g = Some(req);
        }
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
    }
    m
}

fn backends(
    last: Arc<Mutex<Option<TurnRequest>>>,
) -> std::collections::HashMap<String, Arc<dyn AgentBackend>> {
    let mut m = std::collections::HashMap::new();
    m.insert(
        "cli.generic".into(),
        Arc::new(CaptureGeneric { last }) as Arc<dyn AgentBackend>,
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

fn pdf_payload(ok: &Value) -> Vec<u8> {
    for key in ["bytes", "content", "body", "markdown"] {
        if let Some(s) = ok.get(key).and_then(|v| v.as_str()) {
            if s.starts_with("%PDF") {
                return s.as_bytes().to_vec();
            }
            if let Ok(d) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, s) {
                if d.starts_with(b"%PDF") {
                    return d;
                }
            }
        }
    }
    Vec::new()
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
        json!({ "title": title, "workspaceId": ws_id }),
    )
    .await;
    assert!(task.get("error").is_none(), "{task}");
    (ws_id, rpc_id(&task, "id"))
}

fn last_contents(last: &Arc<Mutex<Option<TurnRequest>>>) -> Vec<String> {
    let g = last.lock().unwrap();
    let req = g.as_ref().unwrap_or_else(|| panic!("no turn captured"));
    req.messages.iter().map(|m| m.content.clone()).collect()
}

#[tokio::test]
async fn artifact_export_format_pdf_returns_200_and_pdf_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let last = Arc::new(Mutex::new(None));
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends(last)))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, hs) = handshake(&client, &base, f9_methods()).await;
    assert_eq!(hs["ok"]["accepted"]["artifact.export"]["minor"], 9, "{hs}");

    let proj = dir.path().join("proj");
    let (_ws, task_id) = seed_ws_task(&client, &base, &token, &proj, "pdf").await;
    let art = rpc(
        &client,
        &base,
        Some(&token),
        "artifact.create",
        json!({
            "taskId": task_id,
            "kind": "spec",
            "title": "Auth",
            "body": "# Auth\n"
        }),
    )
    .await;
    let art_id = rpc_id(&art, "id");

    let resp = client
        .post(format!("{base}/rpc"))
        .header("X-Rt-Session", &token)
        .json(&json!({
            "id": "pdf",
            "method": "artifact.export",
            "params": { "artifactId": art_id, "format": "pdf" }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let pdf: Value = resp.json().await.unwrap();
    assert!(pdf.get("error").is_none(), "{pdf}");
    assert_eq!(pdf["ok"]["format"], "pdf");
    let raw = pdf_payload(&pdf["ok"]);
    assert!(
        raw.starts_with(b"%PDF"),
        "body/bytes must start with %PDF: {:?}",
        raw.get(..8)
    );

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
    assert_eq!(md["ok"]["markdown"], "Auth\n\n# Auth\n");

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn nested_agents_md_pkg_before_root_not_in_get_context() {
    let dir = tempfile::tempdir().unwrap();
    let last = Arc::new(Mutex::new(None));
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends(last.clone())))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, f8_methods()).await;

    let ws = dir.path().join("ws");
    let pkg = ws.join("pkg");
    std::fs::create_dir_all(&pkg).unwrap();
    std::fs::write(ws.join("AGENTS.md"), "ROOT_AGENTS_MD_MARKER").unwrap();
    std::fs::write(pkg.join("AGENTS.md"), "PKG_AGENTS_MD_MARKER").unwrap();
    std::fs::write(pkg.join("lib.rs"), "fn main() {}\n").unwrap();

    let (_ws_id, task_id) = seed_ws_task(&client, &base, &token, &ws, "nested").await;
    let created = rpc(
        &client,
        &base,
        Some(&token),
        "agent.create",
        json!({ "taskId": task_id, "provider": "cli.generic" }),
    )
    .await;
    let agent_id = rpc_id(&created, "id");
    let sent = rpc(
        &client,
        &base,
        Some(&token),
        "agent.send",
        json!({
            "agentId": agent_id,
            "content": "hello",
            "path": pkg.join("lib.rs").to_string_lossy(),
            "cwd": pkg.to_string_lossy(),
        }),
    )
    .await;
    assert!(sent.get("error").is_none(), "{sent}");
    wait_status(&client, &base, &token, &agent_id, "idle").await;

    let contents = last_contents(&last);
    let joined = contents.join("\n---\n");
    let pkg_at = joined
        .find("PKG_AGENTS_MD_MARKER")
        .unwrap_or_else(|| panic!("pkg text missing: {joined}"));
    let root_at = joined
        .find("ROOT_AGENTS_MD_MARKER")
        .unwrap_or_else(|| panic!("root text missing: {joined}"));
    assert!(
        pkg_at < root_at,
        "pkg must be injected before root: {joined}"
    );

    let ctx = rpc(
        &client,
        &base,
        Some(&token),
        "agent.get_context",
        json!({ "agentId": agent_id }),
    )
    .await;
    let msgs = ctx["ok"]["messages"].as_array().unwrap();
    for m in msgs {
        let text = m["content"].as_str().unwrap_or("");
        assert!(
            !text.contains("PKG_AGENTS_MD_MARKER") && !text.contains("ROOT_AGENTS_MD_MARKER"),
            "guide leaked into get_context: {ctx}"
        );
    }

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn client_1_8_create_send_export_md_live() {
    let dir = tempfile::tempdir().unwrap();
    let last = Arc::new(Mutex::new(None));
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends(last)))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, hs) = handshake(&client, &base, f8_methods()).await;
    assert!(hs["ok"]["accepted"].get("agent.create").is_some(), "{hs}");
    assert!(hs["ok"]["accepted"].get("agent.send").is_some(), "{hs}");
    assert_eq!(
        hs["ok"]["accepted"]["artifact.export"]["minor"], 9,
        "host offers 1.9 even when client 1.8 offers 1.4: {hs}"
    );
    assert!(
        hs["ok"]["rejected"].get("artifact.export").is_none(),
        "{hs}"
    );

    let proj = dir.path().join("proj");
    let (_ws, task_id) = seed_ws_task(&client, &base, &token, &proj, "c18").await;
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
    assert_eq!(md["ok"]["markdown"], "Note\n\nbody");

    let _ = tx.send(());
    let _ = join.await;
}

#[test]
fn migrations_0001_to_0008_byte_identical_vs_0b8bb7c() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
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
        let out = Command::new("git")
            .args([
                "show",
                &format!("0b8bb7c:crates/rt-storage/migrations/{name}"),
            ])
            .current_dir(&root)
            .output()
            .expect("git show");
        assert!(
            out.status.success(),
            "git show 0b8bb7c:{name} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            current, out.stdout,
            "{name} must be byte-identical to 0b8bb7c"
        );
    }
    let nine = root.join("crates/rt-storage/migrations/0009_v21.sql");
    assert!(
        nine.exists(),
        "0009 is opened by the V2 host slice: {nine:?}"
    );
}
