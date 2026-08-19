//! V5 host slice: C64 pr.get via system gh/git (protocol 1.9). No PAT in db.

use std::path::Path;
use std::pin::Pin;
use std::process::Command;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use futures::Stream;
use rt_protocol::{PrCheck, PrCommit, PrFile, PrGetOk, PrGetParams};
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
        map.insert("sync.export".into(), json!({ "major": 1, "minor": 8 }));
        map.insert("sync.import".into(), json!({ "major": 1, "minor": 8 }));
    }
    m
}

fn client_1_9_methods() -> Value {
    let mut m = client_1_8_methods();
    if let Value::Object(map) = &mut m {
        map.insert("agent.create".into(), json!({ "major": 1, "minor": 9 }));
        map.insert("pr.get".into(), json!({ "major": 1, "minor": 9 }));
    }
    m
}

fn backends() -> std::collections::HashMap<String, ArcDyn> {
    let mut m = std::collections::HashMap::new();
    m.insert(
        "cli.generic".into(),
        std::sync::Arc::new(InstantGeneric) as ArcDyn,
    );
    m
}

type ArcDyn = std::sync::Arc<dyn AgentBackend>;

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
        .expect("git");
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

fn write_exec(path: &Path, body: &str) {
    std::fs::write(path, body).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perm = std::fs::metadata(path).unwrap().permissions();
        perm.set_mode(0o755);
        std::fs::set_permissions(path, perm).unwrap();
    }
}

static PATH_LOCK: Mutex<()> = Mutex::new(());

struct PathGuard {
    prev: Option<String>,
    _lock: MutexGuard<'static, ()>,
}

impl PathGuard {
    fn prepend(dir: &Path) -> Self {
        let lock = PATH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev = std::env::var("PATH").ok();
        let extra = dir.to_str().expect("bin dir utf-8").to_string();
        let next = match &prev {
            Some(p) => format!("{extra}:{p}"),
            None => extra,
        };
        std::env::set_var("PATH", next);
        Self { prev, _lock: lock }
    }
}

impl Drop for PathGuard {
    fn drop(&mut self) {
        match &self.prev {
            Some(p) => std::env::set_var("PATH", p),
            None => std::env::remove_var("PATH"),
        }
    }
}

fn install_fake_gh(bin: &Path, body: &str) -> PathGuard {
    std::fs::create_dir_all(bin).unwrap();
    write_exec(&bin.join("gh"), body);
    PathGuard::prepend(bin)
}

fn gh_ok_script() -> String {
    r#"#!/bin/sh
if [ "$1" = "auth" ] && [ "$2" = "status" ]; then
  exit 0
fi
if [ "$1" = "pr" ] && [ "$2" = "view" ]; then
  cat <<'JSON'
{"number":90,"url":"https://github.com/acme/repo/pull/90","title":"feat: pr.get","state":"OPEN","commits":[{"oid":"abc123def456","messageHeadline":"feat: pr.get","authors":[{"name":"Valeriy","login":"valeron"}]}],"files":[{"path":"crates/rt-host/src/worktree.rs","additions":12,"deletions":1,"changeType":"MODIFIED"}],"statusCheckRollup":[{"name":"ci","status":"COMPLETED","conclusion":"SUCCESS"}]}
JSON
  exit 0
fi
if [ "$1" = "pr" ] && [ "$2" = "list" ]; then
  echo '[]'
  exit 0
fi
echo "unexpected: $*" >&2
exit 1
"#
    .into()
}

fn gh_auth_fail_script() -> String {
    r#"#!/bin/sh
if [ "$1" = "auth" ]; then
  echo "You are not logged into any GitHub hosts. To log in, run: gh auth login" >&2
  exit 1
fi
if [ "$1" = "pr" ] && [ "$2" = "view" ]; then
  echo "HTTP 401: Requires authentication (gh auth login)" >&2
  exit 1
fi
echo "unexpected: $*" >&2
exit 1
"#
    .into()
}

fn assert_no_secret_keys(v: &Value, path: &str) {
    match v {
        Value::Object(m) => {
            for (k, val) in m {
                let low = k.to_ascii_lowercase();
                assert!(
                    low != "token" && low != "pat",
                    "secret-like field {k} at {path}"
                );
                assert_no_secret_keys(val, &format!("{path}.{k}"));
            }
        }
        Value::Array(a) => {
            for (i, val) in a.iter().enumerate() {
                assert_no_secret_keys(val, &format!("{path}[{i}]"));
            }
        }
        _ => {}
    }
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
                    && !low.contains("secret")
                    && !low.contains("api_key"),
                "secret-like column {c} on {t}"
            );
        }
    }
}

#[tokio::test]
async fn pr_get_ok_with_fake_gh_checks_commits_files() {
    let dir = tempfile::tempdir().unwrap();
    let _gh = install_fake_gh(&dir.path().join("bin"), &gh_ok_script());
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, hs) = handshake(&client, &base, client_1_9_methods()).await;
    assert_eq!(hs["ok"]["accepted"]["pr.get"]["minor"], 9, "{hs}");

    let proj = dir.path().join("repo");
    init_git_repo(&proj);
    git(&proj, &["checkout", "-b", "feature"]);
    std::fs::write(proj.join("README.md"), "hello\npr\n").unwrap();
    git(&proj, &["add", "README.md"]);
    git(&proj, &["commit", "-m", "feat"]);

    let ws = rpc(
        &client,
        &base,
        Some(&token),
        "workspace.add",
        json!({ "path": proj.to_str().unwrap() }),
    )
    .await;
    let ws_id = rpc_id(&ws, "id");

    let got = rpc(
        &client,
        &base,
        Some(&token),
        "pr.get",
        json!({ "workspaceId": ws_id, "number": 90 }),
    )
    .await;
    assert!(got.get("error").is_none(), "{got}");
    assert_eq!(got["ok"]["number"], 90, "{got}");
    assert_eq!(got["ok"]["url"], "https://github.com/acme/repo/pull/90");
    assert_eq!(got["ok"]["title"], "feat: pr.get");
    assert_eq!(got["ok"]["state"], "OPEN");
    assert_eq!(got["ok"]["checks"][0]["name"], "ci");
    assert_eq!(got["ok"]["checks"][0]["status"], "COMPLETED");
    assert_eq!(got["ok"]["checks"][0]["conclusion"], "SUCCESS");
    assert_eq!(got["ok"]["commits"][0]["sha"], "abc123def456");
    assert_eq!(got["ok"]["commits"][0]["title"], "feat: pr.get");
    assert_eq!(got["ok"]["commits"][0]["author"], "Valeriy");
    assert_eq!(
        got["ok"]["files"][0]["path"],
        "crates/rt-host/src/worktree.rs"
    );
    assert_eq!(got["ok"]["files"][0]["additions"], 12);
    assert_eq!(got["ok"]["files"][0]["deletions"], 1);
    assert_eq!(got["ok"]["files"][0]["status"], "MODIFIED");
    let diff = got["ok"]["diff"].as_str().expect("diff string");
    if !diff.is_empty() {
        assert!(
            diff.contains("diff") || diff.contains("README"),
            "unexpected local diff: {diff}"
        );
    }
    assert_no_secret_keys(&got["ok"], "ok");

    let via_url = rpc(
        &client,
        &base,
        Some(&token),
        "pr.get",
        json!({
            "workspaceId": ws_id,
            "url": "https://github.com/acme/repo/pull/90"
        }),
    )
    .await;
    assert!(via_url.get("error").is_none(), "{via_url}");
    assert_eq!(via_url["ok"]["number"], 90, "{via_url}");

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn pr_get_auth_required_when_gh_auth_fails_sqlite_has_no_token() {
    let dir = tempfile::tempdir().unwrap();
    let _gh = install_fake_gh(&dir.path().join("bin"), &gh_auth_fail_script());
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, client_1_9_methods()).await;

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

    let got = rpc(
        &client,
        &base,
        Some(&token),
        "pr.get",
        json!({ "workspaceId": ws_id, "number": 90 }),
    )
    .await;
    assert_eq!(got["error"]["code"], "auth_required", "{got}");
    let msg = got["error"]["message"].as_str().unwrap_or_default();
    assert!(
        !msg.contains("ghp_")
            && !msg.contains("github_pat_")
            && !msg.to_ascii_lowercase().contains("bearer "),
        "error leaked a secret: {msg}"
    );
    assert_no_secret_columns(&dir.path().join("host.db"));

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn pr_get_missing_workspace_not_found_missing_selector_invalid_params() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, _) = handshake(&client, &base, client_1_9_methods()).await;

    let missing = rpc(
        &client,
        &base,
        Some(&token),
        "pr.get",
        json!({ "workspaceId": "ws-missing", "number": 1 }),
    )
    .await;
    assert_eq!(missing["error"]["code"], "not_found", "{missing}");

    let proj = dir.path().join("ws");
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

    let no_sel = rpc(
        &client,
        &base,
        Some(&token),
        "pr.get",
        json!({ "workspaceId": ws_id }),
    )
    .await;
    assert_eq!(no_sel["error"]["code"], "invalid_params", "{no_sel}");

    let conflict = rpc(
        &client,
        &base,
        Some(&token),
        "pr.get",
        json!({
            "workspaceId": ws_id,
            "number": 1,
            "url": "https://github.com/acme/repo/pull/2"
        }),
    )
    .await;
    assert_eq!(conflict["error"]["code"], "invalid_params", "{conflict}");

    let bad_url = rpc(
        &client,
        &base,
        Some(&token),
        "pr.get",
        json!({ "workspaceId": ws_id, "url": "https://example.com/not-a-pr" }),
    )
    .await;
    assert_eq!(bad_url["error"]["code"], "invalid_params", "{bad_url}");

    let _ = tx.send(());
    let _ = join.await;
}

#[tokio::test]
async fn client_1_8_send_lives_pr_get_version_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let (addr, tx, join, _) = rt_host::spawn_test_host(dir.path(), Some(backends()))
        .await
        .unwrap();
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();
    let (token, hs) = handshake(&client, &base, client_1_8_methods()).await;
    assert!(hs["ok"]["accepted"].get("agent.send").is_some(), "{hs}");
    assert!(hs["ok"]["accepted"].get("pr.get").is_none(), "{hs}");

    let proj = dir.path().join("ws");
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

    let pr = rpc(
        &client,
        &base,
        Some(&token),
        "pr.get",
        json!({ "workspaceId": ws_id, "number": 90 }),
    )
    .await;
    assert_eq!(pr["error"]["code"], "version_mismatch", "{pr}");

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

#[test]
fn pr_get_request_response_types_have_no_token_pat_fields() {
    let params = PrGetParams {
        workspace_id: "w1".into(),
        number: Some(90),
        url: Some("https://github.com/acme/repo/pull/90".into()),
    };
    let ok = PrGetOk {
        number: 90,
        url: "https://github.com/acme/repo/pull/90".into(),
        title: "feat: pr.get".into(),
        state: "OPEN".into(),
        checks: vec![PrCheck {
            name: "ci".into(),
            status: "COMPLETED".into(),
            conclusion: Some("SUCCESS".into()),
        }],
        commits: vec![PrCommit {
            sha: "abc".into(),
            title: "feat: pr.get".into(),
            author: Some("Valeriy".into()),
        }],
        files: vec![PrFile {
            path: "src/lib.rs".into(),
            additions: 1,
            deletions: 0,
            status: "MODIFIED".into(),
        }],
        diff: String::new(),
    };
    let pv = serde_json::to_value(&params).unwrap();
    let ov = serde_json::to_value(&ok).unwrap();
    assert_no_secret_keys(&pv, "params");
    assert_no_secret_keys(&ov, "ok");
}
