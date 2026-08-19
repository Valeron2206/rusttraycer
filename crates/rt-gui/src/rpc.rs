//! HTTP client for host RPC. No spawn. Agent/files over RPC; WS is in `ws`.

use std::collections::BTreeMap;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::state::PidInfo;

const TIMEOUT: Duration = Duration::from_secs(2);
const WRITE_TIMEOUT: Duration = Duration::from_secs(120);
const CLIENT_VERSION: &str = rt_protocol::CRATE_VERSION;

pub const METHOD_POLICY_GET: &str = "policy.get";
pub const METHOD_POLICY_SET: &str = "policy.set";
pub const METHOD_APPROVAL_RESPOND: &str = "approval.respond";
pub const METHOD_FILES_WRITE: &str = "files.write";
pub const METHOD_FILES_PATCH: &str = "files.patch";
pub const METHOD_FILES_OPEN: &str = "files.open";
pub const METHOD_GIT_STAGE: &str = "git.stage";
pub const METHOD_GIT_UNSTAGE: &str = "git.unstage";
pub const METHOD_GIT_RESTORE: &str = "git.restore";
pub const METHOD_GIT_COMMIT: &str = "git.commit";
pub const METHOD_GIT_PUSH: &str = "git.push";

pub const WRITE_METHODS: &[&str] = &[
    METHOD_FILES_WRITE,
    METHOD_FILES_PATCH,
    METHOD_FILES_OPEN,
    METHOD_GIT_STAGE,
    METHOD_GIT_UNSTAGE,
    METHOD_GIT_RESTORE,
    METHOD_GIT_COMMIT,
    METHOD_GIT_PUSH,
];

pub const METHOD_SHELL_CREATE: &str = "shell.create";
pub const METHOD_SHELL_LIST: &str = "shell.list";
pub const METHOD_SHELL_CLOSE: &str = "shell.close";
pub const METHOD_PTY_OPEN: &str = "pty.open";
pub const METHOD_PTY_WRITE: &str = "pty.write";
pub const METHOD_PTY_RESIZE: &str = "pty.resize";
pub const METHOD_PTY_CLOSE: &str = "pty.close";

pub const PTY_METHODS: &[&str] = &[
    METHOD_SHELL_CREATE,
    METHOD_SHELL_LIST,
    METHOD_SHELL_CLOSE,
    METHOD_PTY_OPEN,
    METHOD_PTY_WRITE,
    METHOD_PTY_RESIZE,
    METHOD_PTY_CLOSE,
];

pub const METHOD_ARTIFACT_CREATE: &str = "artifact.create";
pub const METHOD_ARTIFACT_GET: &str = "artifact.get";
pub const METHOD_ARTIFACT_LIST: &str = "artifact.list";
pub const METHOD_ARTIFACT_UPDATE: &str = "artifact.update";
pub const METHOD_ARTIFACT_DELETE: &str = "artifact.delete";
pub const METHOD_ARTIFACT_EXPORT: &str = "artifact.export";
pub const METHOD_COMMENT_CREATE: &str = "comment.create";
pub const METHOD_COMMENT_LIST: &str = "comment.list";
pub const METHOD_COMMENT_RESOLVE: &str = "comment.resolve";
pub const METHOD_CLEAR_TRANSCRIPT: &str = "agent.clear_transcript";

pub const ARTIFACT_METHODS: &[&str] = &[
    METHOD_ARTIFACT_CREATE,
    METHOD_ARTIFACT_GET,
    METHOD_ARTIFACT_LIST,
    METHOD_ARTIFACT_UPDATE,
    METHOD_ARTIFACT_DELETE,
    METHOD_ARTIFACT_EXPORT,
    METHOD_COMMENT_CREATE,
    METHOD_COMMENT_LIST,
    METHOD_COMMENT_RESOLVE,
    METHOD_CLEAR_TRANSCRIPT,
];

pub const METHOD_A2A_TRANSCRIPT: &str = "a2a.transcript";
pub const METHOD_A2A_DELIVER: &str = "a2a.deliver";
pub const METHOD_LOOP_START: &str = "loop.start";
pub const METHOD_LOOP_GET: &str = "loop.get";
pub const METHOD_LOOP_STOP: &str = "loop.stop";

pub const A2A_METHODS: &[&str] = &[
    METHOD_A2A_TRANSCRIPT,
    METHOD_A2A_DELIVER,
    METHOD_LOOP_START,
    METHOD_LOOP_GET,
    METHOD_LOOP_STOP,
];

pub const METHOD_AGENT_SWITCH: &str = "agent.switch";
pub const METHOD_PROFILE_CREATE: &str = "profile.create";
pub const METHOD_PROFILE_LIST: &str = "profile.list";
pub const METHOD_PROFILE_GET: &str = "profile.get";
pub const METHOD_PROFILE_UPDATE: &str = "profile.update";
pub const METHOD_PROFILE_DELETE: &str = "profile.delete";
pub const METHOD_PREFS_GET: &str = "prefs.get";

pub const MODEL_METHODS: &[&str] = &[
    METHOD_AGENT_SWITCH,
    METHOD_PROFILE_CREATE,
    METHOD_PROFILE_LIST,
    METHOD_PROFILE_GET,
    METHOD_PROFILE_UPDATE,
    METHOD_PROFILE_DELETE,
    METHOD_PREFS_GET,
];

pub const METHOD_WORKSPACE_GUIDES_GET: &str = "workspace.guides.get";
pub const METHOD_SETTINGS_GUIDE_GET: &str = "settings.guide.get";
pub const METHOD_SETTINGS_GUIDE_SET: &str = "settings.guide.set";
pub const METHOD_PRESET_LIST: &str = "preset.list";
pub const METHOD_PRESET_CREATE: &str = "preset.create";
pub const METHOD_PRESET_UPDATE: &str = "preset.update";
pub const METHOD_PRESET_DELETE: &str = "preset.delete";
pub const METHOD_AGENT_UPDATE: &str = "agent.update";

pub const PRESET_CRUD_METHODS: &[&str] = &[
    METHOD_PRESET_CREATE,
    METHOD_PRESET_UPDATE,
    METHOD_PRESET_DELETE,
];

pub const WORKSPACE_METHODS: &[&str] = &[
    METHOD_WORKSPACE_GUIDES_GET,
    METHOD_SETTINGS_GUIDE_GET,
    METHOD_SETTINGS_GUIDE_SET,
    METHOD_PRESET_LIST,
    METHOD_AGENT_UPDATE,
];

pub const METHOD_SYNC_EXPORT: &str = "sync.export";
pub const METHOD_SYNC_IMPORT: &str = "sync.import";
pub const METHOD_SYNC_PUSH: &str = "sync.push";
pub const METHOD_SYNC_PULL: &str = "sync.pull";

pub const SYNC_METHODS: &[&str] = &[METHOD_SYNC_EXPORT, METHOD_SYNC_IMPORT];
pub const SYNC_PEER_METHODS: &[&str] = &[METHOD_SYNC_PUSH, METHOD_SYNC_PULL];

pub const METHOD_SEARCH_QUERY: &str = "search.query";
pub const METHOD_WORKTREE_GC: &str = "worktree.gc";

pub const SEARCH_GC_METHODS: &[&str] = &[METHOD_SEARCH_QUERY, METHOD_WORKTREE_GC];

pub const METHOD_ACCOUNT_LIST: &str = "account.list";
pub const METHOD_AGENT_STEER: &str = "agent.steer";

pub const ACCOUNT_STEER_METHODS: &[&str] = &[METHOD_ACCOUNT_LIST, METHOD_AGENT_STEER];

pub const METHOD_PR_GET: &str = "pr.get";

pub const PR_METHODS: &[&str] = &[METHOD_PR_GET];

pub const METHOD_STASH_LIST: &str = "stash.list";
pub const METHOD_STASH_ADD: &str = "stash.add";
pub const METHOD_STASH_DELETE: &str = "stash.delete";

pub const STASH_METHODS: &[&str] = &[METHOD_STASH_LIST, METHOD_STASH_ADD, METHOD_STASH_DELETE];

#[derive(Debug, Clone)]
pub struct Session {
    pub host_id: String,
    pub host_version: String,
    pub session_token: String,
    pub rpc_url: String,
    pub ws_url: Option<String>,
    pub accepted: BTreeMap<String, rt_protocol::MethodVersion>,
    pub rejected: BTreeMap<String, String>,
}

#[derive(Debug)]
pub enum ConnectError {
    Health(String),
    Handshake(String),
    Ping(String),
    HostIdMismatch { pid: String, rpc: String },
    Rpc { code: String, message: String },
    Transport(String),
}

impl ConnectError {
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::Rpc { code, .. } if code == "not_found")
    }

    pub fn is_agent_busy(&self) -> bool {
        matches!(self, Self::Rpc { code, .. } if code == rt_protocol::error_codes::AGENT_BUSY)
    }

    pub fn is_invalid_params(&self) -> bool {
        matches!(
            self,
            Self::Rpc { code, .. } if code == rt_protocol::error_codes::INVALID_PARAMS
        )
    }

    pub fn is_unsupported_method(&self) -> bool {
        matches!(
            self,
            Self::Rpc { code, .. } if code == rt_protocol::error_codes::UNSUPPORTED_METHOD
        )
    }

    pub fn is_version_mismatch(&self) -> bool {
        matches!(
            self,
            Self::Rpc { code, .. } if code == rt_protocol::error_codes::VERSION_MISMATCH
        )
    }

    pub fn is_write_unsupported(&self) -> bool {
        self.is_unsupported_method() || self.is_version_mismatch()
    }

    pub fn is_pty_unsupported(&self) -> bool {
        self.is_unsupported_method() || self.is_version_mismatch()
    }

    pub fn is_artifacts_unsupported(&self) -> bool {
        self.is_unsupported_method() || self.is_version_mismatch()
    }

    pub fn is_a2a_unsupported(&self) -> bool {
        self.is_unsupported_method() || self.is_version_mismatch()
    }

    pub fn is_model_ux_unsupported(&self) -> bool {
        self.is_unsupported_method() || self.is_version_mismatch()
    }

    pub fn is_workspace_unsupported(&self) -> bool {
        self.is_unsupported_method() || self.is_version_mismatch()
    }

    pub fn is_sync_unsupported(&self) -> bool {
        self.is_unsupported_method() || self.is_version_mismatch()
    }

    pub fn is_search_unsupported(&self) -> bool {
        self.is_unsupported_method() || self.is_version_mismatch()
    }

    pub fn is_worktree_gc_unsupported(&self) -> bool {
        self.is_unsupported_method() || self.is_version_mismatch()
    }

    pub fn is_accounts_unsupported(&self) -> bool {
        self.is_unsupported_method() || self.is_version_mismatch()
    }

    pub fn is_steer_unsupported(&self) -> bool {
        self.is_unsupported_method() || self.is_version_mismatch()
    }

    pub fn is_pr_unsupported(&self) -> bool {
        self.is_unsupported_method() || self.is_version_mismatch()
    }

    pub fn is_stash_unsupported(&self) -> bool {
        self.is_unsupported_method() || self.is_version_mismatch()
    }

    pub fn is_sync_peer_unsupported(&self) -> bool {
        self.is_unsupported_method() || self.is_version_mismatch()
    }

    pub fn is_preset_crud_unsupported(&self) -> bool {
        self.is_unsupported_method() || self.is_version_mismatch()
    }

    pub fn is_pty_dead(&self) -> bool {
        matches!(self, Self::Rpc { code, .. } if code == "pty_dead")
    }

    pub fn is_not_pty(&self) -> bool {
        matches!(self, Self::Rpc { code, .. } if code == "not_pty")
    }

    pub fn as_label(&self) -> String {
        match self {
            Self::Health(msg) => format!("health: {msg}"),
            Self::Handshake(msg) => format!("handshake: {msg}"),
            Self::Ping(msg) => format!("ping: {msg}"),
            Self::HostIdMismatch { pid, rpc } => {
                format!("другой hostId (pid {pid}, rpc {rpc})")
            }
            Self::Rpc { code, message } => {
                if message.is_empty() {
                    code.clone()
                } else {
                    format!("{code}: {message}")
                }
            }
            Self::Transport(msg) => format!("host не отвечает: {msg}"),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HealthBody {
    ok: bool,
    host_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerHello {
    host_id: String,
    host_version: String,
    session_token: String,
    #[serde(default)]
    accepted: BTreeMap<String, rt_protocol::MethodVersion>,
    #[serde(default)]
    rejected: BTreeMap<String, rt_protocol::RejectedMethod>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PingOk {
    host_id: String,
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout(TIMEOUT).build()
}

fn write_agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout(WRITE_TIMEOUT).build()
}

fn git_root_params(workspace_id: &str, worktree_id: Option<&str>) -> Value {
    let mut params = json!({ "workspaceId": workspace_id });
    if let Some(id) = worktree_id {
        params["worktreeId"] = json!(id);
    }
    params
}

fn rpc_origin(rpc_url: &str) -> String {
    rpc_url.trim().trim_end_matches('/').to_string()
}

fn req_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

fn hello_methods() -> Value {
    let mut map = serde_json::Map::new();
    for name in rt_protocol::TRADABLE_METHODS {
        map.insert(name.to_string(), json!({ "major": 1, "minor": 0 }));
    }
    for name in [
        "worktree.ensure",
        "worktree.get",
        "worktree.list",
        "git.status",
        "git.diff",
    ] {
        map.insert(name.to_string(), json!({ "major": 1, "minor": 0 }));
    }
    for name in [
        METHOD_POLICY_GET,
        METHOD_POLICY_SET,
        METHOD_APPROVAL_RESPOND,
    ] {
        map.insert(name.to_string(), json!({ "major": 1, "minor": 1 }));
    }
    for name in WRITE_METHODS {
        map.insert(name.to_string(), json!({ "major": 1, "minor": 2 }));
    }
    for name in PTY_METHODS {
        map.insert(name.to_string(), json!({ "major": 1, "minor": 3 }));
    }
    for name in ARTIFACT_METHODS {
        map.insert(name.to_string(), json!({ "major": 1, "minor": 4 }));
    }
    for name in A2A_METHODS {
        map.insert(name.to_string(), json!({ "major": 1, "minor": 5 }));
    }
    for name in MODEL_METHODS {
        map.insert(name.to_string(), json!({ "major": 1, "minor": 6 }));
    }
    for name in WORKSPACE_METHODS {
        map.insert(name.to_string(), json!({ "major": 1, "minor": 7 }));
    }
    for name in SYNC_METHODS {
        map.insert(name.to_string(), json!({ "major": 1, "minor": 8 }));
    }
    for name in SEARCH_GC_METHODS {
        map.insert(name.to_string(), json!({ "major": 1, "minor": 9 }));
    }
    for name in ACCOUNT_STEER_METHODS {
        map.insert(name.to_string(), json!({ "major": 1, "minor": 9 }));
    }
    for name in PR_METHODS {
        map.insert(name.to_string(), json!({ "major": 1, "minor": 9 }));
    }
    for name in STASH_METHODS {
        map.insert(name.to_string(), json!({ "major": 1, "minor": 9 }));
    }
    for name in SYNC_PEER_METHODS {
        map.insert(name.to_string(), json!({ "major": 1, "minor": 9 }));
    }
    for name in PRESET_CRUD_METHODS {
        map.insert(name.to_string(), json!({ "major": 1, "minor": 9 }));
    }
    Value::Object(map)
}

fn post_rpc(
    http: &ureq::Agent,
    origin: &str,
    method: &str,
    params: Value,
    token: Option<&str>,
) -> Result<Value, ConnectError> {
    let url = format!("{origin}/rpc");
    let body = json!({
        "id": req_id(),
        "method": method,
        "params": params,
    });
    let mut req = http.post(&url).set("Content-Type", "application/json");
    if let Some(token) = token {
        req = req.set(rt_protocol::SESSION_HEADER, token);
    }
    let resp = req
        .send_json(body)
        .map_err(|err| ConnectError::Transport(err.to_string()))?;
    let value: Value = resp
        .into_json()
        .map_err(|err| ConnectError::Transport(err.to_string()))?;
    if value.get("error").is_some() {
        let code = value["error"]["code"]
            .as_str()
            .unwrap_or("error")
            .to_string();
        let message = value["error"]["message"].as_str().unwrap_or("").to_string();
        return Err(ConnectError::Rpc { code, message });
    }
    if value.get("ok").is_none() {
        return Err(ConnectError::Transport("ответ без ok".into()));
    }
    Ok(value)
}

pub fn health(http: &ureq::Agent, origin: &str) -> Result<String, ConnectError> {
    let url = format!("{origin}/health");
    let resp = http
        .get(&url)
        .call()
        .map_err(|err| ConnectError::Health(err.to_string()))?;
    let body: HealthBody = resp
        .into_json()
        .map_err(|err| ConnectError::Health(err.to_string()))?;
    if !body.ok {
        return Err(ConnectError::Health("ok=false".into()));
    }
    if body.host_id.trim().is_empty() {
        return Err(ConnectError::Health("пустой hostId".into()));
    }
    Ok(body.host_id)
}

fn handshake(http: &ureq::Agent, origin: &str) -> Result<ServerHello, ConnectError> {
    let value = post_rpc(
        http,
        origin,
        rt_protocol::METHOD_HANDSHAKE,
        json!({
            "client": "gui",
            "clientVersion": CLIENT_VERSION,
            "methods": hello_methods(),
        }),
        None,
    )
    .map_err(|err| match err {
        ConnectError::Transport(msg) => ConnectError::Handshake(msg),
        other => other,
    })?;
    serde_json::from_value(value["ok"].clone())
        .map_err(|err| ConnectError::Handshake(err.to_string()))
}

pub fn ping(http: &ureq::Agent, origin: &str, token: Option<&str>) -> Result<String, ConnectError> {
    let value = post_rpc(
        http,
        origin,
        rt_protocol::METHOD_HOST_PING,
        json!({}),
        token,
    )
    .map_err(|err| match err {
        ConnectError::Transport(msg) => ConnectError::Ping(msg),
        other => other,
    })?;
    let body: PingOk = serde_json::from_value(value["ok"].clone())
        .map_err(|err| ConnectError::Ping(err.to_string()))?;
    Ok(body.host_id)
}

/// pid.json → GET /health → handshake → host.ping.
/// Online only if every RPC step succeeds. File present is not enough.
pub fn connect(info: &PidInfo) -> Result<Session, ConnectError> {
    let origin = rpc_origin(&info.rpc_url);
    if origin.is_empty() {
        return Err(ConnectError::Transport("пустой rpcUrl".into()));
    }
    let http = agent();

    let health_id = health(&http, &origin)?;
    if health_id != info.host_id {
        return Err(ConnectError::HostIdMismatch {
            pid: info.host_id.clone(),
            rpc: health_id,
        });
    }

    let hello = handshake(&http, &origin)?;
    if hello.host_id != info.host_id {
        return Err(ConnectError::HostIdMismatch {
            pid: info.host_id.clone(),
            rpc: hello.host_id,
        });
    }

    let ping_id = ping(&http, &origin, Some(&hello.session_token))?;
    if ping_id != info.host_id {
        return Err(ConnectError::HostIdMismatch {
            pid: info.host_id.clone(),
            rpc: ping_id,
        });
    }

    let rejected = hello
        .rejected
        .into_iter()
        .map(|(k, v)| (k, v.reason))
        .collect();
    Ok(Session {
        host_id: hello.host_id,
        host_version: hello.host_version,
        session_token: hello.session_token,
        rpc_url: origin,
        ws_url: info.ws_url.clone(),
        accepted: hello.accepted,
        rejected,
    })
}

#[derive(Debug, Clone)]
pub struct TasksCatalog {
    pub workspaces: Vec<rt_protocol::Workspace>,
    pub tasks: Vec<rt_protocol::Task>,
    pub task_presets: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct ItemList<T> {
    items: Vec<T>,
}

fn parse_ok<T: DeserializeOwned>(ok: Value) -> Result<T, ConnectError> {
    serde_json::from_value(ok).map_err(|err| ConnectError::Transport(err.to_string()))
}

fn parse_items<T: DeserializeOwned>(ok: Value) -> Result<Vec<T>, ConnectError> {
    Ok(parse_ok::<ItemList<T>>(ok)?.items)
}

fn extract_opt_str(ok: &Value, key: &str) -> Option<String> {
    ok.get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn put_account_id(params: &mut Value, account_id: Option<&str>) {
    crate::account_ux::put_account_id(params, account_id);
}

fn parse_tasks_with_presets(
    ok: Value,
) -> Result<(Vec<rt_protocol::Task>, BTreeMap<String, String>), ConnectError> {
    let items = ok
        .get("items")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut tasks = Vec::new();
    let mut presets = BTreeMap::new();
    for item in items {
        let preset = extract_opt_str(&item, "preset");
        let task: rt_protocol::Task = parse_ok(item)?;
        if let Some(preset) = preset {
            presets.insert(task.id.clone(), preset);
        }
        tasks.push(task);
    }
    Ok((tasks, presets))
}

fn parse_agents_with_roles(
    ok: Value,
) -> Result<Vec<(rt_protocol::Agent, Option<String>)>, ConnectError> {
    let items = ok
        .get("items")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::new();
    for item in items {
        let role = extract_opt_str(&item, "role");
        let agent: rt_protocol::Agent = parse_ok(item)?;
        out.push((agent, role));
    }
    Ok(out)
}

/// Make `path` absolute without walking the tree or requiring it to exist.
pub fn to_absolute_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    match std::path::absolute(trimmed) {
        Ok(p) => p.to_string_lossy().into_owned(),
        Err(_) => trimmed.to_string(),
    }
}

impl Session {
    fn call(&self, method: &str, params: Value) -> Result<Value, ConnectError> {
        let http = agent();
        let value = post_rpc(
            &http,
            &self.rpc_url,
            method,
            params,
            Some(&self.session_token),
        )?;
        Ok(value["ok"].clone())
    }

    pub fn workspace_list(&self) -> Result<Vec<rt_protocol::Workspace>, ConnectError> {
        parse_items(self.call(rt_protocol::METHOD_WORKSPACE_LIST, json!({}))?)
    }

    pub fn workspace_add(&self, path: &str) -> Result<rt_protocol::Workspace, ConnectError> {
        parse_ok(self.call(rt_protocol::METHOD_WORKSPACE_ADD, json!({ "path": path }))?)
    }

    pub fn task_list(&self, status: &str) -> Result<Vec<rt_protocol::Task>, ConnectError> {
        parse_items(self.call(rt_protocol::METHOD_TASK_LIST, json!({ "status": status }))?)
    }

    pub fn task_create(
        &self,
        title: &str,
        workspace_id: &str,
    ) -> Result<rt_protocol::Task, ConnectError> {
        self.task_create_with_preset(title, workspace_id, None)
    }

    pub fn task_create_with_preset(
        &self,
        title: &str,
        workspace_id: &str,
        preset: Option<&str>,
    ) -> Result<rt_protocol::Task, ConnectError> {
        let mut params = json!({ "title": title, "workspaceId": workspace_id });
        if let Some(preset) = preset {
            params["preset"] = json!(preset);
        }
        parse_ok(self.call(rt_protocol::METHOD_TASK_CREATE, params)?)
    }

    pub fn task_rename(&self, id: &str, title: &str) -> Result<rt_protocol::Task, ConnectError> {
        parse_ok(self.call(
            rt_protocol::METHOD_TASK_RENAME,
            json!({ "id": id, "title": title }),
        )?)
    }

    pub fn task_archive(&self, id: &str) -> Result<rt_protocol::Task, ConnectError> {
        parse_ok(self.call(rt_protocol::METHOD_TASK_ARCHIVE, json!({ "id": id }))?)
    }

    /// `workspace.list`, then `task.list` with `status` if a workspace exists.
    pub fn refresh_tasks_catalog(&self, status: &str) -> Result<TasksCatalog, ConnectError> {
        let workspaces = self.workspace_list()?;
        if workspaces.is_empty() {
            return Ok(TasksCatalog {
                workspaces,
                tasks: Vec::new(),
                task_presets: BTreeMap::new(),
            });
        }
        let ok = self.call(rt_protocol::METHOD_TASK_LIST, json!({ "status": status }))?;
        let (tasks, task_presets) = parse_tasks_with_presets(ok)?;
        Ok(TasksCatalog {
            workspaces,
            tasks,
            task_presets,
        })
    }

    pub fn agent_list(&self, task_id: &str) -> Result<Vec<rt_protocol::Agent>, ConnectError> {
        parse_items(self.call(rt_protocol::METHOD_AGENT_LIST, json!({ "taskId": task_id }))?)
    }

    pub fn agent_create(
        &self,
        task_id: &str,
        provider: &str,
        account_id: Option<&str>,
    ) -> Result<rt_protocol::Agent, ConnectError> {
        let mut params = json!({ "taskId": task_id, "provider": provider });
        put_account_id(&mut params, account_id);
        parse_ok(self.call(rt_protocol::METHOD_AGENT_CREATE, params)?)
    }

    pub fn agent_get(&self, id: &str) -> Result<rt_protocol::Agent, ConnectError> {
        parse_ok(self.call(rt_protocol::METHOD_AGENT_GET, json!({ "id": id }))?)
    }

    pub fn agent_cancel(&self, agent_id: &str) -> Result<CancelOk, ConnectError> {
        parse_ok(self.call(
            rt_protocol::METHOD_AGENT_CANCEL,
            json!({ "agentId": agent_id }),
        )?)
    }

    pub fn agent_send(
        &self,
        agent_id: &str,
        content: &str,
    ) -> Result<rt_protocol::Message, ConnectError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct SendOk {
            user_message: rt_protocol::Message,
        }
        Ok(parse_ok::<SendOk>(self.call(
            rt_protocol::METHOD_AGENT_SEND,
            json!({ "agentId": agent_id, "content": content }),
        )?)?
        .user_message)
    }

    pub fn agent_get_context(
        &self,
        agent_id: &str,
    ) -> Result<Vec<rt_protocol::Message>, ConnectError> {
        #[derive(Deserialize)]
        struct ContextOk {
            messages: Vec<rt_protocol::Message>,
        }
        Ok(parse_ok::<ContextOk>(self.call(
            rt_protocol::METHOD_AGENT_GET_CONTEXT,
            json!({ "agentId": agent_id }),
        )?)?
        .messages)
    }

    pub fn files_tree(
        &self,
        workspace_id: &str,
        path: &str,
    ) -> Result<rt_protocol::FileTreeOk, ConnectError> {
        self.files_tree_for(workspace_id, path, None)
    }

    pub fn files_read(
        &self,
        workspace_id: &str,
        path: &str,
    ) -> Result<rt_protocol::FileReadOk, ConnectError> {
        self.files_read_for(workspace_id, path, None)
    }

    pub fn files_tree_for(
        &self,
        workspace_id: &str,
        path: &str,
        worktree_id: Option<&str>,
    ) -> Result<rt_protocol::FileTreeOk, ConnectError> {
        let mut params = json!({ "workspaceId": workspace_id, "path": path, "depth": 1 });
        if let Some(id) = worktree_id {
            params["worktreeId"] = json!(id);
        }
        parse_ok(self.call(rt_protocol::METHOD_FILES_TREE, params)?)
    }

    pub fn files_read_for(
        &self,
        workspace_id: &str,
        path: &str,
        worktree_id: Option<&str>,
    ) -> Result<rt_protocol::FileReadOk, ConnectError> {
        let mut params = json!({ "workspaceId": workspace_id, "path": path });
        if let Some(id) = worktree_id {
            params["worktreeId"] = json!(id);
        }
        parse_ok(self.call(rt_protocol::METHOD_FILES_READ, params)?)
    }

    pub fn worktree_ensure(&self, agent_id: &str) -> Result<Worktree, ConnectError> {
        parse_ok(self.call("worktree.ensure", json!({ "agentId": agent_id }))?)
    }

    pub fn worktree_get(&self, agent_id: &str) -> Result<Option<Worktree>, ConnectError> {
        match self.call("worktree.get", json!({ "agentId": agent_id })) {
            Ok(ok) => parse_ok(ok).map(Some),
            Err(err) if err.is_not_found() => Ok(None),
            Err(err) => Err(err),
        }
    }

    pub fn git_status(
        &self,
        workspace_id: &str,
        worktree_id: Option<&str>,
    ) -> Result<GitStatusOk, ConnectError> {
        let mut params = json!({ "workspaceId": workspace_id });
        if let Some(id) = worktree_id {
            params["worktreeId"] = json!(id);
        }
        parse_ok(self.call("git.status", params)?)
    }

    pub fn git_diff(
        &self,
        workspace_id: &str,
        worktree_id: Option<&str>,
        path: Option<&str>,
    ) -> Result<GitDiffOk, ConnectError> {
        let mut params = json!({ "workspaceId": workspace_id });
        if let Some(id) = worktree_id {
            params["worktreeId"] = json!(id);
        }
        if let Some(path) = path {
            params["path"] = json!(path);
        }
        parse_ok(self.call("git.diff", params)?)
    }

    pub fn host_doctor(&self) -> Result<DoctorOk, ConnectError> {
        parse_ok(self.call(rt_protocol::METHOD_HOST_DOCTOR, json!({}))?)
    }

    pub fn ladder_accepted(&self) -> bool {
        fn ok(map: &BTreeMap<String, rt_protocol::MethodVersion>, name: &str) -> bool {
            map.get(name)
                .map(|v| v.major == 1 && v.minor >= 1)
                .unwrap_or(false)
        }
        ok(&self.accepted, METHOD_POLICY_GET)
            && ok(&self.accepted, METHOD_POLICY_SET)
            && ok(&self.accepted, METHOD_APPROVAL_RESPOND)
    }

    pub fn ladder_rejected(&self) -> bool {
        self.rejected.contains_key(METHOD_POLICY_GET)
            || self.rejected.contains_key(METHOD_POLICY_SET)
            || self.rejected.contains_key(METHOD_APPROVAL_RESPOND)
    }

    pub fn policy_get(&self, agent_id: &str) -> Result<PolicyOk, ConnectError> {
        parse_ok(self.call(METHOD_POLICY_GET, json!({ "agentId": agent_id }))?)
    }

    pub fn policy_set(
        &self,
        agent_id: &str,
        mode: &str,
        scope: &str,
        yolo: bool,
    ) -> Result<PolicyOk, ConnectError> {
        parse_ok(self.call(
            METHOD_POLICY_SET,
            json!({
                "agentId": agent_id,
                "mode": mode,
                "scope": scope,
                "yolo": yolo,
            }),
        )?)
    }

    pub fn approval_respond(
        &self,
        approval_id: &str,
        decision: &str,
    ) -> Result<ApprovalRespondOk, ConnectError> {
        parse_ok(self.call(
            METHOD_APPROVAL_RESPOND,
            json!({
                "approvalId": approval_id,
                "decision": decision,
            }),
        )?)
    }

    pub fn write_accepted(&self) -> bool {
        fn ok(map: &BTreeMap<String, rt_protocol::MethodVersion>, name: &str) -> bool {
            map.get(name)
                .map(|v| v.major == 1 && v.minor >= 2)
                .unwrap_or(false)
        }
        WRITE_METHODS.iter().all(|name| ok(&self.accepted, name))
    }

    pub fn write_rejected(&self) -> bool {
        WRITE_METHODS
            .iter()
            .any(|name| self.rejected.contains_key(*name))
    }

    fn call_write(&self, method: &str, params: Value) -> Result<Value, ConnectError> {
        let http = write_agent();
        let value = post_rpc(
            &http,
            &self.rpc_url,
            method,
            params,
            Some(&self.session_token),
        )?;
        Ok(value["ok"].clone())
    }

    pub fn files_open(
        &self,
        workspace_id: &str,
        worktree_id: Option<&str>,
        path: &str,
    ) -> Result<FilesOpenOk, ConnectError> {
        let mut params = git_root_params(workspace_id, worktree_id);
        params["path"] = json!(path);
        parse_ok(self.call_write(METHOD_FILES_OPEN, params)?)
    }

    pub fn git_stage(
        &self,
        workspace_id: &str,
        worktree_id: Option<&str>,
        paths: &[&str],
    ) -> Result<GitStatusOk, ConnectError> {
        let mut params = git_root_params(workspace_id, worktree_id);
        params["paths"] = json!(paths);
        parse_ok(self.call_write(METHOD_GIT_STAGE, params)?)
    }

    pub fn git_unstage(
        &self,
        workspace_id: &str,
        worktree_id: Option<&str>,
        paths: &[&str],
    ) -> Result<GitStatusOk, ConnectError> {
        let mut params = git_root_params(workspace_id, worktree_id);
        params["paths"] = json!(paths);
        parse_ok(self.call_write(METHOD_GIT_UNSTAGE, params)?)
    }

    pub fn git_restore(
        &self,
        workspace_id: &str,
        worktree_id: Option<&str>,
        paths: &[&str],
        staged: bool,
    ) -> Result<GitStatusOk, ConnectError> {
        let mut params = git_root_params(workspace_id, worktree_id);
        params["paths"] = json!(paths);
        params["staged"] = json!(staged);
        parse_ok(self.call_write(METHOD_GIT_RESTORE, params)?)
    }

    pub fn git_commit(
        &self,
        workspace_id: &str,
        worktree_id: Option<&str>,
        message: &str,
    ) -> Result<GitCommitOk, ConnectError> {
        let mut params = git_root_params(workspace_id, worktree_id);
        params["message"] = json!(message);
        parse_ok(self.call_write(METHOD_GIT_COMMIT, params)?)
    }

    pub fn git_push(
        &self,
        workspace_id: &str,
        worktree_id: Option<&str>,
        remote: Option<&str>,
        git_ref: Option<&str>,
    ) -> Result<GitPushOk, ConnectError> {
        let mut params = git_root_params(workspace_id, worktree_id);
        if let Some(remote) = remote {
            params["remote"] = json!(remote);
        }
        if let Some(git_ref) = git_ref {
            params["ref"] = json!(git_ref);
        }
        parse_ok(self.call_write(METHOD_GIT_PUSH, params)?)
    }

    pub fn terminal_accepted(&self) -> bool {
        fn ok(map: &BTreeMap<String, rt_protocol::MethodVersion>, name: &str) -> bool {
            map.get(name)
                .map(|v| v.major == 1 && v.minor >= 3)
                .unwrap_or(false)
        }
        PTY_METHODS.iter().all(|name| ok(&self.accepted, name))
    }

    pub fn terminal_rejected(&self) -> bool {
        PTY_METHODS
            .iter()
            .any(|name| self.rejected.contains_key(*name))
    }

    pub fn agent_create_with_interface(
        &self,
        task_id: &str,
        provider: &str,
        interface: &str,
        account_id: Option<&str>,
    ) -> Result<rt_protocol::Agent, ConnectError> {
        let mut params = json!({
            "taskId": task_id,
            "provider": provider,
            "interface": interface,
        });
        put_account_id(&mut params, account_id);
        parse_ok(self.call(rt_protocol::METHOD_AGENT_CREATE, params)?)
    }

    pub fn shell_create(
        &self,
        task_id: Option<&str>,
        workspace_id: &str,
        worktree_id: Option<&str>,
        cols: u16,
        rows: u16,
    ) -> Result<ShellCreateOk, ConnectError> {
        let mut params = json!({
            "workspaceId": workspace_id,
            "worktreeId": worktree_id,
            "cols": cols,
            "rows": rows,
        });
        if let Some(task_id) = task_id {
            params["taskId"] = json!(task_id);
        }
        parse_ok(self.call(METHOD_SHELL_CREATE, params)?)
    }

    pub fn shell_list(&self, task_id: &str) -> Result<Vec<ShellInfo>, ConnectError> {
        parse_items(self.call(METHOD_SHELL_LIST, json!({ "taskId": task_id }))?)
    }

    pub fn shell_close(&self, shell_id: &str) -> Result<Value, ConnectError> {
        self.call(METHOD_SHELL_CLOSE, json!({ "shellId": shell_id }))
    }

    pub fn pty_open_agent(
        &self,
        agent_id: &str,
        cols: u16,
        rows: u16,
    ) -> Result<PtyOpenOk, ConnectError> {
        parse_ok(self.call(
            METHOD_PTY_OPEN,
            json!({ "agentId": agent_id, "cols": cols, "rows": rows }),
        )?)
    }

    pub fn pty_open_shell(
        &self,
        shell_id: &str,
        cols: u16,
        rows: u16,
    ) -> Result<PtyOpenOk, ConnectError> {
        parse_ok(self.call(
            METHOD_PTY_OPEN,
            json!({ "shellId": shell_id, "cols": cols, "rows": rows }),
        )?)
    }

    pub fn pty_write(&self, pty_id: &str, data: &[u8]) -> Result<Value, ConnectError> {
        let encoded = crate::terminal::encode_b64(crate::terminal::clamp_write(data));
        self.call(
            METHOD_PTY_WRITE,
            json!({ "ptyId": pty_id, "data": encoded }),
        )
    }

    pub fn pty_resize(&self, pty_id: &str, cols: u16, rows: u16) -> Result<Value, ConnectError> {
        self.call(
            METHOD_PTY_RESIZE,
            json!({ "ptyId": pty_id, "cols": cols, "rows": rows }),
        )
    }

    pub fn pty_close(&self, pty_id: &str) -> Result<Value, ConnectError> {
        self.call(METHOD_PTY_CLOSE, json!({ "ptyId": pty_id }))
    }

    pub fn artifacts_accepted(&self) -> bool {
        fn ok(map: &BTreeMap<String, rt_protocol::MethodVersion>, name: &str) -> bool {
            map.get(name)
                .map(|v| v.major == 1 && v.minor >= 4)
                .unwrap_or(false)
        }
        ARTIFACT_METHODS.iter().all(|name| ok(&self.accepted, name))
    }

    pub fn artifacts_rejected(&self) -> bool {
        ARTIFACT_METHODS
            .iter()
            .any(|name| self.rejected.contains_key(*name))
    }

    pub fn artifact_list(
        &self,
        task_id: &str,
        kind: Option<&str>,
    ) -> Result<ArtifactListOk, ConnectError> {
        let mut params = json!({ "taskId": task_id });
        if let Some(kind) = kind {
            params["kind"] = json!(kind);
        }
        parse_ok(self.call(METHOD_ARTIFACT_LIST, params)?)
    }

    pub fn artifact_get(&self, artifact_id: &str) -> Result<ArtifactOk, ConnectError> {
        parse_ok(self.call(METHOD_ARTIFACT_GET, json!({ "artifactId": artifact_id }))?)
    }

    pub fn artifact_create(
        &self,
        task_id: &str,
        kind: &str,
        title: &str,
        body: &str,
        parent_id: Option<&str>,
        assignee: Option<&str>,
    ) -> Result<ArtifactOk, ConnectError> {
        parse_ok(self.call(
            METHOD_ARTIFACT_CREATE,
            json!({
                "taskId": task_id,
                "parentId": parent_id,
                "kind": kind,
                "title": title,
                "body": body,
                "assignee": assignee,
                "sourceMessageId": Value::Null,
            }),
        )?)
    }

    pub fn artifact_update(
        &self,
        artifact_id: &str,
        title: Option<&str>,
        body: Option<&str>,
        status: Option<&str>,
        assignee: Option<&str>,
        parent_id: Option<Option<&str>>,
    ) -> Result<ArtifactOk, ConnectError> {
        let mut params = json!({ "artifactId": artifact_id });
        if let Some(title) = title {
            params["title"] = json!(title);
        }
        if let Some(body) = body {
            params["body"] = json!(body);
        }
        if let Some(status) = status {
            params["status"] = json!(status);
        }
        if let Some(assignee) = assignee {
            params["assignee"] = json!(assignee);
        }
        if let Some(parent) = parent_id {
            params["parentId"] = json!(parent);
        }
        parse_ok(self.call(METHOD_ARTIFACT_UPDATE, params)?)
    }

    pub fn artifact_delete(&self, artifact_id: &str) -> Result<ArtifactDeleteOk, ConnectError> {
        parse_ok(self.call(METHOD_ARTIFACT_DELETE, json!({ "artifactId": artifact_id }))?)
    }

    pub fn artifact_export(
        &self,
        artifact_id: &str,
        format: &str,
    ) -> Result<ArtifactExportOk, ConnectError> {
        parse_ok(self.call(
            METHOD_ARTIFACT_EXPORT,
            json!({ "artifactId": artifact_id, "format": format }),
        )?)
    }

    pub fn comment_list(&self, artifact_id: &str) -> Result<CommentListOk, ConnectError> {
        parse_ok(self.call(METHOD_COMMENT_LIST, json!({ "artifactId": artifact_id }))?)
    }

    pub fn comment_create(
        &self,
        artifact_id: &str,
        thread_id: Option<&str>,
        anchor_start: Option<i64>,
        anchor_end: Option<i64>,
        body: &str,
    ) -> Result<CommentThreadOk, ConnectError> {
        let mut params = json!({
            "artifactId": artifact_id,
            "threadId": thread_id,
            "body": body,
        });
        if thread_id.is_none() {
            params["anchorStart"] = json!(anchor_start.unwrap_or(0));
            params["anchorEnd"] = json!(anchor_end.unwrap_or(0));
        }
        parse_ok(self.call(METHOD_COMMENT_CREATE, params)?)
    }

    pub fn comment_resolve(&self, thread_id: &str) -> Result<CommentThreadOk, ConnectError> {
        parse_ok(self.call(METHOD_COMMENT_RESOLVE, json!({ "threadId": thread_id }))?)
    }

    pub fn agent_clear_transcript(
        &self,
        agent_id: &str,
    ) -> Result<ClearTranscriptOk, ConnectError> {
        parse_ok(self.call(METHOD_CLEAR_TRANSCRIPT, json!({ "agentId": agent_id }))?)
    }

    pub fn a2a_accepted(&self) -> bool {
        fn ok(map: &BTreeMap<String, rt_protocol::MethodVersion>, name: &str) -> bool {
            map.get(name)
                .map(|v| v.major == 1 && v.minor >= 5)
                .unwrap_or(false)
        }
        A2A_METHODS.iter().all(|name| ok(&self.accepted, name))
    }

    pub fn a2a_rejected(&self) -> bool {
        A2A_METHODS
            .iter()
            .any(|name| self.rejected.contains_key(*name))
    }

    pub fn agent_create_child(
        &self,
        task_id: &str,
        provider: &str,
        interface: &str,
        parent_id: &str,
    ) -> Result<rt_protocol::Agent, ConnectError> {
        let mut params = json!({
            "taskId": task_id,
            "provider": provider,
            "parentId": parent_id,
        });
        if interface != "chat" {
            params["interface"] = json!(interface);
        }
        parse_ok(self.call(rt_protocol::METHOD_AGENT_CREATE, params)?)
    }

    pub fn a2a_deliver(
        &self,
        from_agent_id: &str,
        to_agent_id: &str,
        content: &str,
    ) -> Result<DeliverOk, ConnectError> {
        parse_ok(self.call(
            METHOD_A2A_DELIVER,
            json!({
                "fromAgentId": from_agent_id,
                "toAgentId": to_agent_id,
                "content": content,
            }),
        )?)
    }

    pub fn loop_start(
        &self,
        task_id: &str,
        agent_a: &str,
        agent_b: &str,
        max_iterations: u32,
        prompt: &str,
    ) -> Result<LoopOk, ConnectError> {
        let max = crate::a2a::clamp_max_iterations(i64::from(max_iterations));
        parse_ok(self.call(
            METHOD_LOOP_START,
            json!({
                "taskId": task_id,
                "agentIds": [agent_a, agent_b],
                "maxIterations": max,
                "prompt": prompt,
            }),
        )?)
    }

    pub fn loop_stop(&self, loop_id: &str) -> Result<LoopOk, ConnectError> {
        parse_ok(self.call(METHOD_LOOP_STOP, json!({ "loopId": loop_id }))?)
    }

    pub fn model_ux_accepted(&self) -> bool {
        fn ok(map: &BTreeMap<String, rt_protocol::MethodVersion>, name: &str) -> bool {
            map.get(name)
                .map(|v| v.major == 1 && v.minor >= 6)
                .unwrap_or(false)
        }
        MODEL_METHODS.iter().all(|name| ok(&self.accepted, name))
    }

    pub fn model_ux_rejected(&self) -> bool {
        MODEL_METHODS
            .iter()
            .any(|name| self.rejected.contains_key(*name))
    }

    pub fn agent_create_with_model(
        &self,
        task_id: &str,
        provider: &str,
        interface: &str,
        model: &crate::model_ux::ModelParams,
        account_id: Option<&str>,
    ) -> Result<rt_protocol::Agent, ConnectError> {
        let mut params = json!({
            "taskId": task_id,
            "provider": provider,
        });
        if interface != "chat" {
            params["interface"] = json!(interface);
        }
        if let Some(name) = model.model.as_deref() {
            params["model"] = json!(name);
        }
        if let Some(effort) = model.effort.as_deref() {
            params["effort"] = json!(effort);
        }
        if model.fast {
            params["fast"] = json!(true);
        }
        put_account_id(&mut params, account_id);
        parse_ok(self.call(rt_protocol::METHOD_AGENT_CREATE, params)?)
    }

    pub fn agent_switch(
        &self,
        agent_id: &str,
        switch: AgentSwitchParams<'_>,
    ) -> Result<AgentModelView, ConnectError> {
        let mut params = json!({ "agentId": agent_id });
        if let Some(provider) = switch.provider {
            params["provider"] = json!(provider);
        }
        if let Some(model) = switch.model {
            params["model"] = json!(model);
        }
        if let Some(effort) = switch.effort {
            params["effort"] = json!(effort);
        }
        if let Some(fast) = switch.fast {
            params["fast"] = json!(fast);
        }
        if let Some(profile_id) = switch.profile_id {
            params["profileId"] = json!(profile_id);
        }
        put_account_id(&mut params, switch.account_id);
        parse_agent_model(self.call(METHOD_AGENT_SWITCH, params)?)
    }

    pub fn profile_create(
        &self,
        name: &str,
        provider: &str,
        model: Option<&str>,
        effort: Option<&str>,
        fast: Option<bool>,
    ) -> Result<ProfileOk, ConnectError> {
        let mut params = json!({
            "name": name,
            "provider": provider,
        });
        if let Some(model) = model {
            params["model"] = json!(model);
        }
        if let Some(effort) = effort {
            params["effort"] = json!(effort);
        }
        if let Some(fast) = fast {
            params["fast"] = json!(fast);
        }
        parse_ok(self.call(METHOD_PROFILE_CREATE, params)?)
    }

    pub fn profile_list(&self) -> Result<Vec<ProfileOk>, ConnectError> {
        parse_items(self.call(METHOD_PROFILE_LIST, json!({}))?)
    }

    pub fn profile_get(&self, profile_id: &str) -> Result<ProfileOk, ConnectError> {
        parse_ok(self.call(METHOD_PROFILE_GET, json!({ "profileId": profile_id }))?)
    }

    pub fn prefs_get(&self) -> Result<Vec<PrefsItem>, ConnectError> {
        parse_items(self.call(METHOD_PREFS_GET, json!({}))?)
    }

    pub fn workspace_accepted(&self) -> bool {
        fn ok(map: &BTreeMap<String, rt_protocol::MethodVersion>, name: &str) -> bool {
            map.get(name)
                .map(|v| v.major == 1 && v.minor >= 7)
                .unwrap_or(false)
        }
        WORKSPACE_METHODS
            .iter()
            .all(|name| ok(&self.accepted, name))
    }

    pub fn workspace_rejected(&self) -> bool {
        WORKSPACE_METHODS
            .iter()
            .any(|name| self.rejected.contains_key(*name))
    }

    pub fn workspace_guides_get(
        &self,
        workspace_id: &str,
    ) -> Result<WorkspaceGuides, ConnectError> {
        parse_ok(self.call(
            METHOD_WORKSPACE_GUIDES_GET,
            json!({ "workspaceId": workspace_id }),
        )?)
    }

    pub fn settings_guide_get(&self) -> Result<SettingsGuide, ConnectError> {
        parse_ok(self.call(METHOD_SETTINGS_GUIDE_GET, json!({}))?)
    }

    pub fn settings_guide_set(&self, content: &str) -> Result<SettingsGuide, ConnectError> {
        parse_ok(self.call(METHOD_SETTINGS_GUIDE_SET, json!({ "content": content }))?)
    }

    pub fn preset_list(&self) -> Result<Vec<PresetItem>, ConnectError> {
        parse_items(self.call(METHOD_PRESET_LIST, json!({}))?)
    }

    pub fn preset_crud_accepted(&self) -> bool {
        PRESET_CRUD_METHODS.iter().all(|name| {
            self.accepted
                .get(*name)
                .map(|v| v.major == 1 && v.minor >= 9)
                .unwrap_or(false)
        })
    }

    pub fn preset_crud_rejected(&self) -> bool {
        PRESET_CRUD_METHODS
            .iter()
            .any(|name| self.rejected.contains_key(*name))
    }

    pub fn preset_create(
        &self,
        name: &str,
        default_role: &str,
        title_hint: &str,
        prompt: &str,
    ) -> Result<PresetItem, ConnectError> {
        let Some(params) =
            crate::workspace_ux::preset_create_params(name, default_role, title_hint, prompt)
        else {
            return Ok(PresetItem::default());
        };
        let ok = self.call(METHOD_PRESET_CREATE, params)?;
        Ok(crate::workspace_ux::parse_preset_item(&ok))
    }

    pub fn preset_update(
        &self,
        id: &str,
        name: &str,
        default_role: &str,
        title_hint: &str,
        prompt: &str,
    ) -> Result<PresetItem, ConnectError> {
        let Some(params) =
            crate::workspace_ux::preset_update_params(id, name, default_role, title_hint, prompt)
        else {
            return Ok(PresetItem::default());
        };
        let ok = self.call(METHOD_PRESET_UPDATE, params)?;
        Ok(crate::workspace_ux::parse_preset_item(&ok))
    }

    pub fn preset_delete(&self, id: &str) -> Result<(), ConnectError> {
        let Some(params) = crate::workspace_ux::preset_delete_params(id) else {
            return Ok(());
        };
        let _ = self.call(METHOD_PRESET_DELETE, params)?;
        Ok(())
    }

    pub fn agent_update_role(
        &self,
        agent_id: &str,
        role: &str,
    ) -> Result<(rt_protocol::Agent, Option<String>), ConnectError> {
        let ok = self.call(
            METHOD_AGENT_UPDATE,
            json!({ "agentId": agent_id, "role": role }),
        )?;
        let role = extract_opt_str(&ok, "role").or_else(|| Some(role.to_string()));
        let agent = parse_ok::<rt_protocol::Agent>(ok)?;
        Ok((agent, role))
    }

    pub fn agent_list_with_roles(
        &self,
        task_id: &str,
    ) -> Result<Vec<(rt_protocol::Agent, Option<String>)>, ConnectError> {
        parse_agents_with_roles(
            self.call(rt_protocol::METHOD_AGENT_LIST, json!({ "taskId": task_id }))?,
        )
    }

    pub fn agent_create_with_role(
        &self,
        task_id: &str,
        provider: &str,
        interface: &str,
        params: &crate::model_ux::ModelParams,
        role: Option<&str>,
        account_id: Option<&str>,
    ) -> Result<(rt_protocol::Agent, Option<String>), ConnectError> {
        let mut body = json!({
            "taskId": task_id,
            "provider": provider,
        });
        if interface != "chat" {
            body["interface"] = json!(interface);
        }
        if let Some(model) = params.model.as_deref() {
            body["model"] = json!(model);
        }
        if let Some(effort) = params.effort.as_deref() {
            body["effort"] = json!(effort);
        }
        if params.fast {
            body["fast"] = json!(true);
        }
        if let Some(role) = role {
            body["role"] = json!(role);
        }
        put_account_id(&mut body, account_id);
        let ok = self.call(rt_protocol::METHOD_AGENT_CREATE, body)?;
        let role = extract_opt_str(&ok, "role");
        let agent = parse_ok::<rt_protocol::Agent>(ok)?;
        Ok((agent, role))
    }

    pub fn sync_accepted(&self) -> bool {
        fn ok(map: &BTreeMap<String, rt_protocol::MethodVersion>, name: &str) -> bool {
            map.get(name)
                .map(|v| v.major == 1 && v.minor >= 8)
                .unwrap_or(false)
        }
        SYNC_METHODS.iter().all(|name| ok(&self.accepted, name))
    }

    pub fn sync_rejected(&self) -> bool {
        SYNC_METHODS
            .iter()
            .any(|name| self.rejected.contains_key(*name))
    }

    pub fn sync_export(&self, task_ids: &[String]) -> Result<SyncExportOk, ConnectError> {
        parse_sync_export(self.call(METHOD_SYNC_EXPORT, json!({ "taskIds": task_ids }))?)
    }

    pub fn sync_import(&self, workspace_id: &str, archive: Value) -> Result<Value, ConnectError> {
        self.call(
            METHOD_SYNC_IMPORT,
            json!({ "workspaceId": workspace_id, "archive": archive }),
        )
    }

    pub fn sync_peer_accepted(&self) -> bool {
        SYNC_PEER_METHODS.iter().all(|name| {
            self.accepted
                .get(*name)
                .map(|v| v.major == 1 && v.minor >= 9)
                .unwrap_or(false)
        })
    }

    pub fn sync_peer_rejected(&self) -> bool {
        SYNC_PEER_METHODS
            .iter()
            .any(|name| self.rejected.contains_key(*name))
    }

    pub fn sync_push(&self, peer_url: &str) -> Result<Value, ConnectError> {
        let Some(params) = crate::sync_ux::push_params(peer_url) else {
            return Ok(json!({}));
        };
        self.call(METHOD_SYNC_PUSH, params)
    }

    pub fn sync_pull(&self, peer_url: &str, workspace_id: &str) -> Result<Value, ConnectError> {
        let Some(params) = crate::sync_ux::pull_params(peer_url, workspace_id) else {
            return Ok(json!({}));
        };
        self.call(METHOD_SYNC_PULL, params)
    }

    pub fn search_accepted(&self) -> bool {
        self.accepted
            .get(METHOD_SEARCH_QUERY)
            .map(|v| v.major == 1 && v.minor >= 9)
            .unwrap_or(false)
    }

    pub fn search_rejected(&self) -> bool {
        self.rejected.contains_key(METHOD_SEARCH_QUERY)
    }

    pub fn worktree_gc_accepted(&self) -> bool {
        self.accepted
            .get(METHOD_WORKTREE_GC)
            .map(|v| v.major == 1 && v.minor >= 9)
            .unwrap_or(false)
    }

    pub fn worktree_gc_rejected(&self) -> bool {
        self.rejected.contains_key(METHOD_WORKTREE_GC)
    }

    pub fn search_query(
        &self,
        q: &str,
        kinds: Option<&[&str]>,
    ) -> Result<Vec<SearchItemOk>, ConnectError> {
        let Some(params) = crate::search_ux::search_params(q, kinds) else {
            return Ok(Vec::new());
        };
        parse_items(self.call(METHOD_SEARCH_QUERY, params)?)
    }

    pub fn worktree_gc(&self, dry_run: bool) -> Result<WorktreeGcOk, ConnectError> {
        parse_ok(self.call(METHOD_WORKTREE_GC, crate::search_ux::gc_params(dry_run))?)
    }

    pub fn accounts_accepted(&self) -> bool {
        self.accepted
            .get(METHOD_ACCOUNT_LIST)
            .map(|v| v.major == 1 && v.minor >= 9)
            .unwrap_or(false)
    }

    pub fn accounts_rejected(&self) -> bool {
        self.rejected.contains_key(METHOD_ACCOUNT_LIST)
    }

    pub fn steer_accepted(&self) -> bool {
        self.accepted
            .get(METHOD_AGENT_STEER)
            .map(|v| v.major == 1 && v.minor >= 9)
            .unwrap_or(false)
    }

    pub fn steer_rejected(&self) -> bool {
        self.rejected.contains_key(METHOD_AGENT_STEER)
    }

    pub fn account_list(&self) -> Result<Vec<crate::account_ux::AccountItem>, ConnectError> {
        let ok = self.call(
            METHOD_ACCOUNT_LIST,
            crate::account_ux::account_list_params(),
        )?;
        Ok(crate::account_ux::parse_account_list(&ok))
    }

    pub fn agent_steer(&self, agent_id: &str, content: &str) -> Result<Value, ConnectError> {
        let Some(params) = crate::account_ux::steer_params(agent_id, content) else {
            return Ok(json!({}));
        };
        self.call(METHOD_AGENT_STEER, params)
    }

    pub fn pr_accepted(&self) -> bool {
        self.accepted
            .get(METHOD_PR_GET)
            .map(|v| v.major == 1 && v.minor >= 9)
            .unwrap_or(false)
    }

    pub fn pr_rejected(&self) -> bool {
        self.rejected.contains_key(METHOD_PR_GET)
    }

    pub fn pr_get(
        &self,
        workspace_id: &str,
        number: &str,
        url: &str,
    ) -> Result<crate::pr_ux::PrView, ConnectError> {
        let Some(params) = crate::pr_ux::pr_get_params(workspace_id, number, url) else {
            return Ok(crate::pr_ux::PrView::default());
        };
        let ok = self.call(METHOD_PR_GET, params)?;
        Ok(crate::pr_ux::parse_pr_get(&ok))
    }

    pub fn stash_accepted(&self) -> bool {
        STASH_METHODS.iter().all(|name| {
            self.accepted
                .get(*name)
                .map(|v| v.major == 1 && v.minor >= 9)
                .unwrap_or(false)
        })
    }

    pub fn stash_rejected(&self) -> bool {
        STASH_METHODS
            .iter()
            .any(|name| self.rejected.contains_key(*name))
    }

    pub fn stash_list(&self) -> Result<Vec<crate::stash::StashItem>, ConnectError> {
        let ok = self.call(METHOD_STASH_LIST, crate::stash::stash_list_params())?;
        Ok(crate::stash::parse_stash_list(&ok))
    }

    pub fn stash_add(
        &self,
        body: &str,
        image_path: Option<&str>,
    ) -> Result<crate::stash::StashItem, ConnectError> {
        let Some(params) = crate::stash::stash_add_params(body, image_path) else {
            return Ok(crate::stash::StashItem::default());
        };
        let ok = self.call(METHOD_STASH_ADD, params)?;
        let mut item = crate::stash::parse_stash_item(&ok);
        if item.body.is_empty() {
            item.body = body.trim().to_string();
        }
        Ok(item)
    }

    pub fn stash_delete(&self, id: &str) -> Result<(), ConnectError> {
        let Some(params) = crate::stash::stash_delete_params(id) else {
            return Ok(());
        };
        let _ = self.call(METHOD_STASH_DELETE, params)?;
        Ok(())
    }

    pub fn fetch_metrics(&self) -> Result<crate::metrics::MetricsChip, ConnectError> {
        fetch_metrics(&self.rpc_url)
    }
}

/// `GET /metrics` on the same origin as health/RPC. Offline callers must not invoke this.
pub fn fetch_metrics(origin: &str) -> Result<crate::metrics::MetricsChip, ConnectError> {
    let origin = rpc_origin(origin);
    if origin.is_empty() {
        return Err(ConnectError::Transport("пустой rpcUrl".into()));
    }
    let url = format!("{}{}", origin, crate::metrics::METRICS_PATH);
    let http = agent();
    let resp = http
        .get(&url)
        .call()
        .map_err(|err| ConnectError::Transport(err.to_string()))?;
    let body = resp
        .into_string()
        .map_err(|err| ConnectError::Transport(err.to_string()))?;
    Ok(crate::metrics::parse_metrics(&body))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncExportOk {
    pub archive: Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchItemOk {
    pub kind: String,
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub hint: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeGcOk {
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub deleted: Vec<Value>,
    #[serde(default)]
    pub items: Vec<Value>,
}

fn parse_sync_export(ok: Value) -> Result<SyncExportOk, ConnectError> {
    if ok.get("archive").is_some() {
        return parse_ok(ok);
    }
    if ok
        .get("kind")
        .and_then(Value::as_str)
        .is_some_and(|k| k == "rusttraycer.export")
    {
        return Ok(SyncExportOk { archive: ok });
    }
    parse_ok(ok)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelOk {
    pub agent_id: String,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuideFile {
    pub path: String,
    pub content: String,
    #[serde(default)]
    pub truncated: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGuides {
    pub agents_md: Option<GuideFile>,
    pub workspace_guide: Option<GuideFile>,
    pub global_guide: Option<GuideFile>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsGuide {
    pub path: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub truncated: bool,
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PresetItem {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub default_role: String,
    #[serde(default)]
    pub title_hint: String,
    #[serde(default)]
    pub prompt: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Worktree {
    pub id: String,
    pub workspace_id: String,
    pub agent_id: String,
    pub path: String,
    pub branch: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatusOk {
    pub branch: String,
    pub dirty: bool,
    #[serde(default)]
    pub entries: Vec<GitStatusEntry>,
    #[serde(default)]
    pub truncated: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatusEntry {
    pub path: String,
    pub status: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitDiffOk {
    pub files: Vec<GitDiffFile>,
    #[serde(default)]
    pub truncated: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitDiffFile {
    pub path: String,
    pub patch: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HarnessCapsView {
    #[serde(default)]
    pub one_shot: bool,
    #[serde(default)]
    pub long_lived: bool,
    #[serde(default)]
    pub stream_tokens: bool,
    #[serde(default)]
    pub tools: bool,
    #[serde(default)]
    pub session_resume: bool,
    #[serde(default)]
    pub a2a_inbox: bool,
    #[serde(default)]
    pub pty: bool,
    #[serde(default)]
    pub needs_api_key: bool,
    #[serde(default)]
    pub api_key_env: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorProvider {
    pub id: String,
    #[serde(default)]
    pub available: bool,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub caps: Option<HarnessCapsView>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DoctorOk {
    #[serde(default)]
    pub host_id: String,
    #[serde(default)]
    pub pid: u64,
    #[serde(default)]
    pub rpc_url: String,
    #[serde(default)]
    pub db_ok: bool,
    #[serde(default)]
    pub data_dir: String,
    #[serde(default)]
    pub db_path: String,
    #[serde(default)]
    pub log_path: String,
    #[serde(default)]
    pub providers: Vec<DoctorProvider>,
    #[serde(default)]
    pub workspace_count: i64,
    #[serde(default)]
    pub task_count: i64,
    #[serde(default)]
    pub agent_count: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyOk {
    pub mode: String,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub yolo: bool,
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalRespondOk {
    #[serde(default)]
    pub applied: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilesOpenOk {
    #[serde(default)]
    pub opened: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitOk {
    pub commit: String,
    pub branch: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPushOk {
    pub remote: String,
    #[serde(rename = "ref")]
    pub git_ref: String,
    pub ok: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellCreateOk {
    pub shell_id: String,
    pub pty_id: String,
    #[serde(default)]
    pub cwd: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellInfo {
    pub shell_id: String,
    #[serde(default)]
    pub pty_id: Option<String>,
    #[serde(default)]
    pub cwd: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PtyOpenOk {
    pub pty_id: String,
    #[serde(default)]
    pub resumed: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactOk {
    pub id: String,
    pub task_id: String,
    #[serde(default)]
    pub parent_id: Option<String>,
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub assignee: Option<String>,
    #[serde(default)]
    pub source_message_id: Option<String>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactListOk {
    #[serde(default)]
    pub items: Vec<ArtifactOk>,
    #[serde(default)]
    pub truncated: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactExportOk {
    #[serde(default)]
    pub format: String,
    #[serde(default)]
    pub markdown: String,
    #[serde(default)]
    pub filename: String,
    /// Raw `%PDF` or base64 PDF when format=pdf (protocol 1.9). Empty for md.
    #[serde(default)]
    pub bytes: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactDeleteOk {
    #[serde(default)]
    pub deleted: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentOk {
    pub id: String,
    pub body: String,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentThreadOk {
    pub id: String,
    #[serde(default)]
    pub artifact_id: String,
    #[serde(default)]
    pub anchor_start: i64,
    #[serde(default)]
    pub anchor_end: i64,
    #[serde(default, deserialize_with = "bool_from_wire")]
    pub resolved: bool,
    #[serde(default)]
    pub comments: Vec<CommentOk>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentListOk {
    #[serde(default)]
    pub threads: Vec<CommentThreadOk>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearTranscriptOk {
    #[serde(default)]
    pub cleared: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliverOk {
    pub message_id: String,
    pub to_agent_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopOk {
    pub loop_id: String,
    #[serde(default)]
    pub iteration: u32,
    #[serde(default)]
    pub turns: u32,
    #[serde(default)]
    pub max_iterations: u32,
    #[serde(default)]
    pub budget_turns: u32,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
}

impl LoopOk {
    pub fn display_max(&self, fallback: u32) -> u32 {
        let _budget = self.budget_turns;
        if self.max_iterations == 0 {
            fallback
        } else {
            self.max_iterations
        }
    }
}

#[derive(Debug, Clone)]
pub struct AgentModelView {
    pub agent: rt_protocol::Agent,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub fast: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct AgentSwitchParams<'a> {
    pub provider: Option<&'a str>,
    pub model: Option<&'a str>,
    pub effort: Option<&'a str>,
    pub fast: Option<bool>,
    pub profile_id: Option<&'a str>,
    pub account_id: Option<&'a str>,
}

fn opt_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn parse_agent_model(ok: Value) -> Result<AgentModelView, ConnectError> {
    let model = opt_string(&ok, "model");
    let effort = opt_string(&ok, "effort");
    let fast = ok.get("fast").and_then(|v| v.as_bool()).unwrap_or(false);
    let agent = parse_ok(ok)?;
    Ok(AgentModelView {
        agent,
        model,
        effort,
        fast,
    })
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileOk {
    pub id: String,
    pub name: String,
    pub provider: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
    #[serde(default)]
    pub fast: bool,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PrefsItem {
    pub provider: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
    #[serde(default)]
    pub fast: bool,
}

fn bool_from_wire<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Bool(b) => Ok(b),
        Value::Number(n) => Ok(n.as_i64().unwrap_or(0) != 0),
        Value::String(s) => Ok(s == "true" || s == "1"),
        _ => Ok(false),
    }
}

pub fn keepalive(session: &Session) -> Result<(), ConnectError> {
    let http = agent();
    let ping_id = ping(&http, &session.rpc_url, Some(&session.session_token))?;
    if ping_id != session.host_id {
        return Err(ConnectError::HostIdMismatch {
            pid: session.host_id.clone(),
            rpc: ping_id,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::thread;

    struct Mock {
        origin: String,
        hits: Arc<Mutex<Vec<String>>>,
    }

    fn start_mock(host_id: &str, token: &str) -> Mock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let hits_t = hits.clone();
        let host_id = host_id.to_string();
        let token = token.to_string();
        thread::spawn(move || {
            for stream in listener.incoming().take(8) {
                let Ok(mut stream) = stream else { break };
                let mut raw = Vec::new();
                let mut tmp = [0u8; 2048];
                loop {
                    match stream.read(&mut tmp) {
                        Ok(0) => break,
                        Ok(n) => raw.extend_from_slice(&tmp[..n]),
                        Err(_) => break,
                    }
                    if let Some(pos) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&raw[..pos]);
                        let cl = headers
                            .lines()
                            .find_map(|line| {
                                let lower = line.to_ascii_lowercase();
                                lower
                                    .strip_prefix("content-length:")
                                    .and_then(|s| s.trim().parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        if raw.len() >= pos + 4 + cl {
                            break;
                        }
                    }
                }
                let req = String::from_utf8_lossy(&raw);
                let path = if req.starts_with("GET /health") {
                    "GET /health"
                } else if req.contains("\"method\":\"handshake\"")
                    || req.contains("\"method\": \"handshake\"")
                {
                    "POST handshake"
                } else if req.contains("host.ping") {
                    "POST host.ping"
                } else {
                    "other"
                };
                hits_t.lock().unwrap().push(path.to_string());
                let body = match path {
                    "GET /health" => json!({"ok": true, "hostId": host_id}).to_string(),
                    "POST handshake" => json!({
                        "id": "echo",
                        "ok": {
                            "hostId": host_id,
                            "hostVersion": "0.1.0",
                            "sessionToken": token,
                            "accepted": { "host.ping": {"major": 1, "minor": 0} },
                            "rejected": {}
                        }
                    })
                    .to_string(),
                    "POST host.ping" => json!({
                        "id": "echo",
                        "ok": { "hostId": host_id, "now": "2026-08-17T12:00:00Z" }
                    })
                    .to_string(),
                    _ => json!({"id":"echo","error":{"code":"unsupported_method","message":"no"}})
                        .to_string(),
                };
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(resp.as_bytes());
            }
        });
        Mock {
            origin: format!("http://{addr}"),
            hits,
        }
    }

    fn pid(host_id: &str, origin: &str) -> PidInfo {
        PidInfo {
            host_id: host_id.into(),
            pid: 1,
            rpc_url: origin.into(),
            ws_url: None,
            started_at: None,
        }
    }

    #[test]
    fn connect_requires_health_handshake_ping() {
        let mock = start_mock("host-a", "tok-1");
        let mut info = pid("host-a", &mock.origin);
        info.ws_url = Some("ws://127.0.0.1:9/ws".into());
        let session = connect(&info).expect("online");
        assert_eq!(session.host_id, "host-a");
        assert_eq!(session.host_version, "0.1.0");
        assert_eq!(session.session_token, "tok-1");
        assert_eq!(session.ws_url.as_deref(), Some("ws://127.0.0.1:9/ws"));
        let hits = mock.hits.lock().unwrap().clone();
        assert_eq!(
            hits,
            vec![
                "GET /health".to_string(),
                "POST handshake".to_string(),
                "POST host.ping".to_string()
            ]
        );
    }

    #[test]
    fn pid_file_alone_is_not_online() {
        let err = connect(&pid("host-a", "http://127.0.0.1:1")).unwrap_err();
        match err {
            ConnectError::Health(_) | ConnectError::Transport(_) => {}
            other => panic!("expected transport/health, got {other:?}"),
        }
    }

    #[test]
    fn live_host_connects_when_env_set() {
        if std::env::var("RT_GUI_LIVE").ok().as_deref() != Some("1") {
            return;
        }
        let info = crate::discovery::read_pid_json().expect("pid.json");
        let session = connect(&info).expect("live online");
        assert_eq!(session.host_id, info.host_id);
        assert!(!session.session_token.is_empty());
    }

    #[test]
    fn host_id_mismatch_is_offline() {
        let mock = start_mock("host-b", "tok-1");
        let err = connect(&pid("host-a", &mock.origin)).unwrap_err();
        match err {
            ConnectError::HostIdMismatch { pid, rpc } => {
                assert_eq!(pid, "host-a");
                assert_eq!(rpc, "host-b");
            }
            other => panic!("expected mismatch, got {other:?}"),
        }
    }

    #[derive(Clone, Debug)]
    struct RpcHit {
        method: String,
        params: Value,
        has_session: bool,
    }

    struct CatalogMock {
        origin: String,
        hits: Arc<Mutex<Vec<RpcHit>>>,
    }

    fn read_http_request(stream: &mut std::net::TcpStream) -> (String, Vec<u8>) {
        use std::io::Read;
        let mut raw = Vec::new();
        let mut tmp = [0u8; 2048];
        loop {
            match stream.read(&mut tmp) {
                Ok(0) => break,
                Ok(n) => raw.extend_from_slice(&tmp[..n]),
                Err(_) => break,
            }
            if let Some(pos) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&raw[..pos]);
                let cl = headers
                    .lines()
                    .find_map(|line| {
                        let lower = line.to_ascii_lowercase();
                        lower
                            .strip_prefix("content-length:")
                            .and_then(|s| s.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                if raw.len() >= pos + 4 + cl {
                    let body = raw[pos + 4..pos + 4 + cl].to_vec();
                    return (headers.into_owned(), body);
                }
            }
        }
        (String::from_utf8_lossy(&raw).into_owned(), Vec::new())
    }

    fn write_http_json(stream: &mut std::net::TcpStream, body: &str) {
        use std::io::Write;
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(resp.as_bytes());
    }

    fn sample_workspace(host_id: &str, path: &str) -> Value {
        json!({
            "id": "ws-1",
            "hostId": host_id,
            "path": path,
            "name": "proj",
            "createdAt": "2026-08-17T12:00:00Z"
        })
    }

    fn sample_task(id: &str, title: &str, status: &str, workspace_id: &str) -> Value {
        json!({
            "id": id,
            "title": title,
            "status": status,
            "createdAt": "2026-08-17T12:00:00Z",
            "updatedAt": "2026-08-17T12:01:00Z",
            "workspaceIds": [workspace_id]
        })
    }

    fn start_catalog_mock(host_id: &str, token: &str) -> CatalogMock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let hits_t = hits.clone();
        let host_id = host_id.to_string();
        let token = token.to_string();
        thread::spawn(move || {
            for stream in listener.incoming().take(32) {
                let Ok(mut stream) = stream else { break };
                let (headers, body) = read_http_request(&mut stream);
                let has_session = headers.to_ascii_lowercase().contains("x-rt-session:");
                let (method, params) = if headers.starts_with("GET /health") {
                    ("GET /health".to_string(), json!({}))
                } else {
                    let parsed: Value = serde_json::from_slice(&body).unwrap_or(json!({}));
                    (
                        parsed
                            .get("method")
                            .and_then(|v| v.as_str())
                            .unwrap_or("other")
                            .to_string(),
                        parsed.get("params").cloned().unwrap_or(json!({})),
                    )
                };
                hits_t.lock().unwrap().push(RpcHit {
                    method: method.clone(),
                    params: params.clone(),
                    has_session,
                });
                let body = match method.as_str() {
                    "GET /health" => json!({"ok": true, "hostId": host_id}).to_string(),
                    "handshake" => json!({
                        "id": "echo",
                        "ok": {
                            "hostId": host_id,
                            "hostVersion": "0.1.0",
                            "sessionToken": token,
                            "accepted": { "host.ping": {"major": 1, "minor": 0} },
                            "rejected": {}
                        }
                    })
                    .to_string(),
                    "host.ping" => json!({
                        "id": "echo",
                        "ok": { "hostId": host_id, "now": "2026-08-17T12:00:00Z" }
                    })
                    .to_string(),
                    "workspace.list" => json!({
                        "id": "echo",
                        "ok": { "items": [sample_workspace(&host_id, "/tmp/proj")] }
                    })
                    .to_string(),
                    "workspace.add" => {
                        let path = params.get("path").and_then(|v| v.as_str()).unwrap_or("");
                        json!({
                            "id": "echo",
                            "ok": sample_workspace(&host_id, path)
                        })
                        .to_string()
                    }
                    "task.list" => json!({
                        "id": "echo",
                        "ok": { "items": [sample_task("task-1", "Demo", "open", "ws-1")] }
                    })
                    .to_string(),
                    "task.create" => {
                        let title = params.get("title").and_then(|v| v.as_str()).unwrap_or("");
                        let ws = params
                            .get("workspaceId")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        json!({
                            "id": "echo",
                            "ok": sample_task("task-new", title, "open", ws)
                        })
                        .to_string()
                    }
                    "task.rename" => {
                        let id = params.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        let title = params.get("title").and_then(|v| v.as_str()).unwrap_or("");
                        json!({
                            "id": "echo",
                            "ok": sample_task(id, title, "open", "ws-1")
                        })
                        .to_string()
                    }
                    "task.archive" => {
                        let id = params.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        json!({
                            "id": "echo",
                            "ok": sample_task(id, "Demo", "archived", "ws-1")
                        })
                        .to_string()
                    }
                    _ => json!({
                        "id": "echo",
                        "error": { "code": "unsupported_method", "message": "no" }
                    })
                    .to_string(),
                };
                write_http_json(&mut stream, &body);
            }
        });
        CatalogMock {
            origin: format!("http://{addr}"),
            hits,
        }
    }

    fn methods_of(mock: &CatalogMock) -> Vec<String> {
        mock.hits
            .lock()
            .unwrap()
            .iter()
            .map(|h| h.method.clone())
            .collect()
    }

    #[test]
    fn catalog_after_connect_calls_workspace_and_task_list() {
        let mock = start_catalog_mock("host-a", "tok-1");
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        let catalog = session.refresh_tasks_catalog("open").expect("catalog");
        assert_eq!(catalog.workspaces.len(), 1);
        assert_eq!(catalog.workspaces[0].id, "ws-1");
        assert_eq!(catalog.tasks.len(), 1);
        assert_eq!(catalog.tasks[0].title, "Demo");
        let methods = methods_of(&mock);
        assert_eq!(
            &methods[..3],
            &[
                "GET /health".to_string(),
                "handshake".to_string(),
                "host.ping".to_string()
            ]
        );
        assert!(methods.contains(&"workspace.list".to_string()));
        assert!(methods.contains(&"task.list".to_string()));
        let list = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "task.list")
            .cloned()
            .expect("task.list");
        assert_eq!(list.params["status"], "open");
        assert!(list.has_session);
    }

    #[test]
    fn workspace_add_sends_absolute_path() {
        let mock = start_catalog_mock("host-a", "tok-1");
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        let abs = to_absolute_path("/tmp/rt-gui-abs-ws");
        assert!(abs.starts_with('/'), "expected absolute, got {abs}");
        let ws = session.workspace_add(&abs).expect("add");
        assert_eq!(ws.path, abs);
        let hit = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "workspace.add")
            .cloned()
            .expect("workspace.add");
        assert_eq!(hit.params["path"], abs);
        assert!(hit.has_session);
    }

    #[test]
    fn task_create_rename_archive_send_right_methods() {
        let mock = start_catalog_mock("host-a", "tok-1");
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        let created = session.task_create("Hello", "ws-1").expect("create");
        assert_eq!(created.title, "Hello");
        assert_eq!(created.workspace_ids, vec!["ws-1".to_string()]);
        let renamed = session.task_rename("task-1", "Renamed").expect("rename");
        assert_eq!(renamed.title, "Renamed");
        let archived = session.task_archive("task-1").expect("archive");
        assert_eq!(archived.status, "archived");

        let hits = mock.hits.lock().unwrap().clone();
        let create = hits.iter().find(|h| h.method == "task.create").unwrap();
        assert_eq!(create.params["title"], "Hello");
        assert_eq!(create.params["workspaceId"], "ws-1");
        assert!(create.has_session);
        let rename = hits.iter().find(|h| h.method == "task.rename").unwrap();
        assert_eq!(rename.params["id"], "task-1");
        assert_eq!(rename.params["title"], "Renamed");
        let archive = hits.iter().find(|h| h.method == "task.archive").unwrap();
        assert_eq!(archive.params["id"], "task-1");
    }

    #[test]
    fn workspace_path_invalid_is_rpc_error() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            for stream in listener.incoming().take(8) {
                let Ok(mut stream) = stream else { break };
                let (headers, body) = read_http_request(&mut stream);
                let parsed: Value = serde_json::from_slice(&body).unwrap_or(json!({}));
                let method = if headers.starts_with("GET /health") {
                    "GET /health"
                } else {
                    parsed.get("method").and_then(|v| v.as_str()).unwrap_or("")
                };
                let body = match method {
                    "GET /health" => json!({"ok": true, "hostId": "host-a"}).to_string(),
                    "handshake" => json!({
                        "id": "echo",
                        "ok": {
                            "hostId": "host-a",
                            "hostVersion": "0.1.0",
                            "sessionToken": "tok-1",
                            "accepted": {},
                            "rejected": {}
                        }
                    })
                    .to_string(),
                    "host.ping" => json!({
                        "id": "echo",
                        "ok": { "hostId": "host-a", "now": "2026-08-17T12:00:00Z" }
                    })
                    .to_string(),
                    "workspace.add" => json!({
                        "id": "echo",
                        "error": {
                            "code": "workspace_path_invalid",
                            "message": "path must exist and be a directory"
                        }
                    })
                    .to_string(),
                    _ => json!({"id":"echo","error":{"code":"unsupported_method","message":"no"}})
                        .to_string(),
                };
                write_http_json(&mut stream, &body);
            }
        });
        let session = connect(&pid("host-a", &format!("http://{addr}"))).expect("online");
        let err = session.workspace_add("/no/such/dir").unwrap_err();
        let label = err.as_label();
        match err {
            ConnectError::Rpc { code, message } => {
                assert_eq!(code, "workspace_path_invalid");
                assert!(message.contains("directory"), "{message}");
                assert!(label.contains("workspace_path_invalid"), "{label}");
            }
            other => panic!("expected Rpc, got {other:?}"),
        }
    }

    fn sample_agent(id: &str, task_id: &str, status: &str) -> Value {
        json!({
            "id": id,
            "taskId": task_id,
            "hostId": "host-a",
            "parentId": null,
            "interface": "chat",
            "provider": "cli.generic",
            "status": status,
            "runLocation": "local",
            "createdAt": "2026-08-17T12:00:00Z"
        })
    }

    fn sample_message(id: &str, agent_id: &str, role: &str, content: &str) -> Value {
        json!({
            "id": id,
            "agentId": agent_id,
            "role": role,
            "content": content,
            "createdAt": "2026-08-17T12:00:00Z"
        })
    }

    fn start_agent_files_mock(host_id: &str, token: &str) -> CatalogMock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let hits_t = hits.clone();
        let host_id = host_id.to_string();
        let token = token.to_string();
        thread::spawn(move || {
            for stream in listener.incoming().take(40) {
                let Ok(mut stream) = stream else { break };
                let (headers, body) = read_http_request(&mut stream);
                let has_session = headers.to_ascii_lowercase().contains("x-rt-session:");
                let (method, params) = if headers.starts_with("GET /health") {
                    ("GET /health".to_string(), json!({}))
                } else {
                    let parsed: Value = serde_json::from_slice(&body).unwrap_or(json!({}));
                    (
                        parsed
                            .get("method")
                            .and_then(|v| v.as_str())
                            .unwrap_or("other")
                            .to_string(),
                        parsed.get("params").cloned().unwrap_or(json!({})),
                    )
                };
                hits_t.lock().unwrap().push(RpcHit {
                    method: method.clone(),
                    params: params.clone(),
                    has_session,
                });
                let body = match method.as_str() {
                    "GET /health" => json!({"ok": true, "hostId": host_id}).to_string(),
                    "handshake" => json!({
                        "id": "echo",
                        "ok": {
                            "hostId": host_id,
                            "hostVersion": "0.1.0",
                            "sessionToken": token,
                            "accepted": {},
                            "rejected": {}
                        }
                    }).to_string(),
                    "host.ping" => json!({
                        "id": "echo",
                        "ok": { "hostId": host_id, "now": "2026-08-17T12:00:00Z" }
                    }).to_string(),
                    "agent.list" => json!({
                        "id": "echo",
                        "ok": { "items": [sample_agent("ag-1", "task-1", "idle")] }
                    }).to_string(),
                    "agent.create" => {
                        let task_id = params.get("taskId").and_then(|v| v.as_str()).unwrap_or("");
                        json!({
                            "id": "echo",
                            "ok": sample_agent("ag-new", task_id, "idle")
                        }).to_string()
                    }
                    "agent.get" => {
                        let id = params.get("id").and_then(|v| v.as_str()).unwrap_or("ag-1");
                        let mut agent = sample_agent(id, "task-1", "idle");
                        agent["lastMessageAt"] = json!("2026-08-17T12:01:00Z");
                        json!({ "id": "echo", "ok": agent }).to_string()
                    }
                    "agent.get_context" => json!({
                        "id": "echo",
                        "ok": { "messages": [sample_message("m1", "ag-1", "user", "hi")] }
                    }).to_string(),
                    "agent.send" => {
                        let agent_id = params.get("agentId").and_then(|v| v.as_str()).unwrap_or("");
                        let content = params.get("content").and_then(|v| v.as_str()).unwrap_or("");
                        json!({
                            "id": "echo",
                            "ok": { "userMessage": sample_message("m-new", agent_id, "user", content) }
                        }).to_string()
                    }
                    "agent.cancel" => {
                        let agent_id = params.get("agentId").and_then(|v| v.as_str()).unwrap_or("");
                        if agent_id == "missing" {
                            json!({
                                "id": "echo",
                                "error": { "code": "not_found", "message": "no agent" }
                            }).to_string()
                        } else {
                            let cancelled = agent_id == "ag-running";
                            json!({
                                "id": "echo",
                                "ok": { "agentId": agent_id, "cancelled": cancelled }
                            }).to_string()
                        }
                    }
                    "worktree.ensure" => {
                        let agent_id = params.get("agentId").and_then(|v| v.as_str()).unwrap_or("ag-1");
                        json!({
                            "id": "echo",
                            "ok": {
                                "id": "wt-1",
                                "workspaceId": "ws-1",
                                "agentId": agent_id,
                                "path": "/tmp/wt",
                                "branch": "agent/ag",
                                "createdAt": "2026-08-17T12:00:00Z"
                            }
                        }).to_string()
                    }
                    "worktree.get" => json!({
                        "id": "echo",
                        "error": { "code": "not_found", "message": "no worktree" }
                    }).to_string(),
                    "git.status" => json!({
                        "id": "echo",
                        "ok": {
                            "branch": "main",
                            "dirty": true,
                            "truncated": false,
                            "entries": [{ "path": "src/lib.rs", "status": "modified" }]
                        }
                    }).to_string(),
                    "git.diff" => json!({
                        "id": "echo",
                        "ok": {
                            "truncated": false,
                            "files": [{
                                "path": params.get("path").and_then(|v| v.as_str()).unwrap_or("src/lib.rs"),
                                "patch": "@@ -1 +1 @@\\n-a\\n+b\\n"
                            }]
                        }
                    }).to_string(),
                    "files.tree" => json!({
                        "id": "echo",
                        "ok": {
                            "items": [{
                                "name": "README.md",
                                "path": "README.md",
                                "kind": "file",
                                "size": 12,
                                "modifiedAt": "2026-08-17T12:00:00Z"
                            }],
                            "truncated": false
                        }
                    }).to_string(),

                    "files.read" => {
                        let path = params.get("path").and_then(|v| v.as_str()).unwrap_or("");
                        if path == "bin.dat" {
                            json!({
                                "id": "echo",
                                "error": { "code": "file_binary", "message": "binary" }
                            }).to_string()
                        } else if path == "huge.txt" {
                            json!({
                                "id": "echo",
                                "error": { "code": "file_too_large", "message": "too large" }
                            }).to_string()
                        } else {
                            json!({
                                "id": "echo",
                                "ok": {
                                    "path": path,
                                    "content": "# hi\n",
                                    "truncated": false,
                                    "encoding": "utf8"
                                }
                            }).to_string()
                        }
                    }
                    _ => json!({
                        "id": "echo",
                        "error": { "code": "unsupported_method", "message": "no" }
                    }).to_string(),
                };
                write_http_json(&mut stream, &body);
            }
        });
        CatalogMock {
            origin: format!("http://{addr}"),
            hits,
        }
    }

    #[test]
    fn agent_rpcs_send_right_methods() {
        let mock = start_agent_files_mock("host-a", "tok-1");
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        let listed = session.agent_list("task-1").expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "ag-1");
        let created = session
            .agent_create("task-1", "cli.generic", None)
            .expect("create");
        assert_eq!(created.id, "ag-new");
        assert_eq!(created.task_id, "task-1");
        assert_eq!(created.provider, "cli.generic");
        let got = session.agent_get("ag-1").expect("get");
        assert_eq!(got.id, "ag-1");
        let ctx = session.agent_get_context("ag-1").expect("ctx");
        assert_eq!(ctx.len(), 1);
        assert_eq!(ctx[0].content, "hi");
        let sent = session.agent_send("ag-1", "hello").expect("send");
        assert_eq!(sent.role, "user");
        assert_eq!(sent.content, "hello");
        assert_eq!(sent.agent_id, "ag-1");

        let hits = mock.hits.lock().unwrap().clone();
        let list = hits.iter().find(|h| h.method == "agent.list").unwrap();
        assert_eq!(list.params["taskId"], "task-1");
        assert!(list.has_session);
        let create = hits.iter().find(|h| h.method == "agent.create").unwrap();
        assert_eq!(create.params["taskId"], "task-1");
        assert_eq!(create.params["provider"], "cli.generic");
        let get = hits.iter().find(|h| h.method == "agent.get").unwrap();
        assert_eq!(get.params["id"], "ag-1");
        let ctx_hit = hits
            .iter()
            .find(|h| h.method == "agent.get_context")
            .unwrap();
        assert_eq!(ctx_hit.params["agentId"], "ag-1");
        let send = hits.iter().find(|h| h.method == "agent.send").unwrap();
        assert_eq!(send.params["agentId"], "ag-1");
        assert_eq!(send.params["content"], "hello");
    }

    #[test]
    fn agent_cancel_sends_method_and_parses_cancelled() {
        let mock = start_agent_files_mock("host-a", "tok-1");
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        let yes = session.agent_cancel("ag-running").expect("cancel true");
        assert_eq!(yes.agent_id, "ag-running");
        assert!(yes.cancelled);
        let no = session.agent_cancel("ag-idle").expect("cancel false");
        assert_eq!(no.agent_id, "ag-idle");
        assert!(!no.cancelled);
        let err = session.agent_cancel("missing").unwrap_err();
        match err {
            ConnectError::Rpc { code, .. } => assert_eq!(code, "not_found"),
            other => panic!("expected not_found, got {other:?}"),
        }

        let hits = mock.hits.lock().unwrap().clone();
        let cancels: Vec<_> = hits.iter().filter(|h| h.method == "agent.cancel").collect();
        assert_eq!(cancels.len(), 3);
        assert_eq!(cancels[0].params["agentId"], "ag-running");
        assert!(cancels[0].has_session);
        assert_eq!(cancels[1].params["agentId"], "ag-idle");
        assert!(cancels[1].has_session);
        assert_eq!(cancels[2].params["agentId"], "missing");
        assert!(cancels[2].has_session);
    }

    #[test]
    fn files_tree_read_send_workspace_and_path() {
        let mock = start_agent_files_mock("host-a", "tok-1");
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        let tree = session.files_tree("ws-1", "").expect("tree root");
        assert_eq!(tree.items.len(), 1);
        assert_eq!(tree.items[0].path, "README.md");
        let tree2 = session.files_tree("ws-1", "src").expect("tree src");
        assert!(!tree2.truncated);
        let read = session.files_read("ws-1", "README.md").expect("read");
        assert_eq!(read.encoding, "utf8");
        assert_eq!(read.content, "# hi\n");

        let err = session.files_read("ws-1", "bin.dat").unwrap_err();
        match err {
            ConnectError::Rpc { code, .. } => assert_eq!(code, "file_binary"),
            other => panic!("expected file_binary, got {other:?}"),
        }
        let err = session.files_read("ws-1", "huge.txt").unwrap_err();
        match err {
            ConnectError::Rpc { code, .. } => assert_eq!(code, "file_too_large"),
            other => panic!("expected file_too_large, got {other:?}"),
        }

        let hits = mock.hits.lock().unwrap().clone();
        let trees: Vec<_> = hits.iter().filter(|h| h.method == "files.tree").collect();
        assert_eq!(trees.len(), 2);
        assert_eq!(trees[0].params["workspaceId"], "ws-1");
        assert_eq!(trees[0].params["path"], "");
        assert_eq!(trees[1].params["workspaceId"], "ws-1");
        assert_eq!(trees[1].params["path"], "src");
        assert!(trees[0].has_session);
        let read_hit = hits
            .iter()
            .find(|h| h.method == "files.read" && h.params["path"] == "README.md")
            .unwrap();
        assert_eq!(read_hit.params["workspaceId"], "ws-1");
        assert_eq!(read_hit.params["path"], "README.md");
    }

    fn rt_host_bin() -> Option<std::path::PathBuf> {
        let candidates = [
            std::path::PathBuf::from("/workspace/rusttraycer/target/debug/rt-host"),
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/rt-host"),
        ];
        candidates.into_iter().find(|p| p.is_file())
    }

    struct LiveHost {
        child: std::process::Child,
        home: std::path::PathBuf,
    }

    impl Drop for LiveHost {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            let _ = std::fs::remove_dir_all(&self.home);
        }
    }

    fn spawn_live_host() -> Option<LiveHost> {
        spawn_live_host_env(&[])
    }

    fn spawn_live_host_env(extra: &[(&str, &std::ffi::OsStr)]) -> Option<LiveHost> {
        let bin = match rt_host_bin() {
            Some(p) => p,
            None if std::env::var("RT_GUI_LIVE").ok().as_deref() == Some("1") => {
                panic!("RT_GUI_LIVE=1 but rt-host binary is missing");
            }
            None => return None,
        };
        let home = std::env::temp_dir().join(format!(
            "rt-gui-live-{}-{}",
            std::process::id(),
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&home).unwrap();
        let mut cmd = std::process::Command::new(&bin);
        cmd.env("RUSTTRAYCER_HOME", &home)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped());
        for (k, v) in extra {
            cmd.env(k, v);
        }
        let child = cmd.spawn().expect("spawn rt-host");
        Some(LiveHost { child, home })
    }

    fn wait_live_pid(home: &std::path::Path) -> PidInfo {
        let path = home.join("host").join("pid.json");
        let start = std::time::Instant::now();
        loop {
            if path.is_file() {
                if let Ok(raw) = std::fs::read_to_string(&path) {
                    if let Ok(v) = serde_json::from_str::<Value>(&raw) {
                        if let (Some(host_id), Some(rpc_url)) = (
                            v.get("hostId").and_then(|x| x.as_str()),
                            v.get("rpcUrl").and_then(|x| x.as_str()),
                        ) {
                            return PidInfo {
                                host_id: host_id.to_string(),
                                pid: v.get("pid").and_then(|x| x.as_u64()).unwrap_or(0),
                                rpc_url: rpc_url.to_string(),
                                ws_url: v
                                    .get("wsUrl")
                                    .and_then(|x| x.as_str())
                                    .map(|s| s.to_string()),
                                started_at: v
                                    .get("startedAt")
                                    .and_then(|x| x.as_str())
                                    .map(|s| s.to_string()),
                            };
                        }
                    }
                }
            }
            if start.elapsed() > std::time::Duration::from_secs(8) {
                panic!("rt-host did not write {} in time", path.display());
            }
            thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    #[test]
    fn live_host_workspace_and_tasks_roundtrip() {
        let Some(mut live) = spawn_live_host() else {
            return;
        };
        let info = wait_live_pid(&live.home);
        let session = {
            let start = std::time::Instant::now();
            loop {
                match connect(&info) {
                    Ok(s) => break s,
                    Err(err) if start.elapsed() < std::time::Duration::from_secs(5) => {
                        let _ = err;
                        thread::sleep(std::time::Duration::from_millis(50));
                    }
                    Err(err) => panic!("live connect failed: {}", err.as_label()),
                }
            }
        };
        assert_eq!(session.host_id, info.host_id);

        let ws_dir = live.home.join("proj");
        std::fs::create_dir_all(&ws_dir).unwrap();
        let abs = to_absolute_path(&ws_dir.to_string_lossy());
        eprintln!(
            "live host online host_id={} rpc={} home={}",
            session.host_id,
            session.rpc_url,
            live.home.display()
        );
        let added = session.workspace_add(&abs).expect("workspace.add");
        eprintln!("workspace.add id={} path={}", added.id, added.path);
        assert!(!added.id.is_empty());
        let canon = std::fs::canonicalize(&ws_dir).unwrap();
        assert_eq!(added.path, canon.to_string_lossy().as_ref());

        let catalog = session.refresh_tasks_catalog("open").expect("catalog");
        assert_eq!(catalog.workspaces.len(), 1);
        assert!(catalog.tasks.is_empty());

        let created = session
            .task_create("Slice 3", &added.id)
            .expect("task.create");
        eprintln!("task.create id={} title={}", created.id, created.title);
        assert_eq!(created.title, "Slice 3");
        assert_eq!(created.status, "open");

        let open = session.task_list("open").expect("task.list open");
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].id, created.id);
        assert_eq!(open[0].title, "Slice 3");

        let renamed = session
            .task_rename(&created.id, "Slice 3 renamed")
            .expect("task.rename");
        assert_eq!(renamed.title, "Slice 3 renamed");
        let open = session.task_list("open").expect("list after rename");
        assert_eq!(open[0].title, "Slice 3 renamed");

        let archived = session.task_archive(&created.id).expect("task.archive");
        assert_eq!(archived.status, "archived");
        let open = session.task_list("open").expect("list after archive");
        assert!(open.is_empty(), "archived task must leave open filter");
        let archived_list = session.task_list("archived").expect("archived list");
        assert_eq!(archived_list.len(), 1);
        assert_eq!(archived_list[0].id, created.id);
        eprintln!(
            "live roundtrip ok: open after archive={}, archived={}",
            open.len(),
            archived_list.len()
        );

        let bad = session.workspace_add("/no/such/rt-gui-dir").unwrap_err();
        match bad {
            ConnectError::Rpc { code, message } => {
                assert_eq!(code, "workspace_path_invalid");
                assert!(!message.is_empty());
            }
            other => panic!("expected workspace_path_invalid, got {other:?}"),
        }

        let _ = live.child.kill();
    }

    fn wait_connect(info: &PidInfo) -> Session {
        let start = std::time::Instant::now();
        loop {
            match connect(info) {
                Ok(s) => return s,
                Err(err) if start.elapsed() < std::time::Duration::from_secs(5) => {
                    let _ = err;
                    thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(err) => panic!("live connect failed: {}", err.as_label()),
            }
        }
    }

    fn write_generic_script(dir: &std::path::Path) -> std::path::PathBuf {
        let path = dir.join("generic_agent.py");
        let body = concat!(
            "#!/usr/bin/env python3\n",
            "import json, sys\n",
            "try:\n",
            "    json.load(sys.stdin)\n",
            "except Exception:\n",
            "    pass\n",
            "sys.stdout.write('hello-chunk-1\\n')\n",
            "sys.stdout.write('hello-chunk-2\\n')\n",
            "sys.stdout.flush()\n",
        );
        std::fs::write(&path, body).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).unwrap();
        }
        path
    }

    #[test]
    fn live_host_agent_ws_and_restart_context() {
        let script_home = std::env::temp_dir().join(format!(
            "rt-gui-agent-script-{}-{}",
            std::process::id(),
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&script_home).unwrap();
        let script = write_generic_script(&script_home);
        let Some(mut live) =
            spawn_live_host_env(&[("RUSTTRAYCER_GENERIC_CMD", script.as_os_str())])
        else {
            let _ = std::fs::remove_dir_all(&script_home);
            return;
        };
        let info = wait_live_pid(&live.home);
        let session = wait_connect(&info);
        assert_eq!(session.host_id, info.host_id);
        assert!(
            session.ws_url.as_deref().unwrap_or("").starts_with("ws://"),
            "ws_url={:?}",
            session.ws_url
        );

        let ws_dir = live.home.join("proj");
        std::fs::create_dir_all(&ws_dir).unwrap();
        std::fs::write(ws_dir.join("README.md"), "# proj\n").unwrap();
        let abs = to_absolute_path(&ws_dir.to_string_lossy());
        let added = session.workspace_add(&abs).expect("workspace.add");
        let created = session
            .task_create("Slice 4", &added.id)
            .expect("task.create");
        let agent = session
            .agent_create(&created.id, "cli.generic", None)
            .expect("agent.create");
        assert_eq!(agent.provider, "cli.generic");
        let listed = session.agent_list(&created.id).expect("agent.list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, agent.id);
        let got = session.agent_get(&agent.id).expect("agent.get");
        assert_eq!(got.id, agent.id);

        let ws_url = session
            .ws_url
            .clone()
            .or_else(|| info.ws_url.clone())
            .expect("ws url");
        let mut socket = match crate::ws::connect_ws(&ws_url, &session.session_token) {
            Ok(s) => s,
            Err(err) => {
                eprintln!("WS connect failed ({err}); falling back to RPC-only loop");
                let sent = session.agent_send(&agent.id, "ping from gui").ok();
                let ctx = session.agent_get_context(&agent.id).expect("get_context");
                if let Some(msg) = sent {
                    assert!(ctx.iter().any(|m| m.id == msg.id));
                }
                let session2 = wait_connect(&info);
                let ctx2 = session2.agent_get_context(&agent.id).expect("restart ctx");
                assert!(
                    ctx2.iter().any(|m| m.role == "user"),
                    "restart context missing user: {ctx2:?}"
                );
                let _ = live.child.kill();
                let _ = std::fs::remove_dir_all(&script_home);
                return;
            }
        };
        let sub = serde_json::json!({ "type": "subscribe", "taskId": created.id }).to_string();
        socket
            .send(tungstenite::Message::text(sub))
            .expect("subscribe");
        thread::sleep(std::time::Duration::from_millis(80));

        let sent = session
            .agent_send(&agent.id, "ping from gui")
            .expect("agent.send");
        assert_eq!(sent.role, "user");
        assert_eq!(sent.content, "ping from gui");

        let mut saw_assistant = false;
        let mut saw_idle = false;
        let mut saw_user_ws = false;
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_secs(8) {
            match socket.read() {
                Ok(tungstenite::Message::Text(text)) => {
                    let text = text.to_string();
                    if let Ok(ev) = crate::ws::parse_event(&text) {
                        match ev {
                            crate::ws::WsEvent::AgentMessage { message, .. } => {
                                if message.role == "user" && message.id == sent.id {
                                    saw_user_ws = true;
                                }
                                if message.role == "assistant" {
                                    saw_assistant = true;
                                }
                            }
                            crate::ws::WsEvent::AgentStatus { status, .. } if status == "idle" => {
                                saw_idle = true;
                            }
                            _ => {}
                        }
                    }
                }
                Ok(_) => {}
                Err(tungstenite::Error::Io(err))
                    if err.kind() == std::io::ErrorKind::WouldBlock
                        || err.kind() == std::io::ErrorKind::TimedOut => {}
                Err(err) => {
                    eprintln!("WS read ended: {err}");
                    break;
                }
            }
            if saw_assistant && saw_idle {
                break;
            }
        }
        let _ = socket.close(None);

        if !saw_assistant {
            eprintln!(
                "WS assistant chunks were not observed (user_ws={saw_user_ws} idle={saw_idle}); RPC path still asserted"
            );
        } else {
            eprintln!(
                "live WS loop: assistant={saw_assistant} idle={saw_idle} user_ws={saw_user_ws}"
            );
        }

        // Simulate GUI restart: new handshake + get_context, no merge.
        let session2 = wait_connect(&info);
        let ctx = session2
            .agent_get_context(&agent.id)
            .expect("restart get_context");
        assert!(
            ctx.iter()
                .any(|m| m.role == "user" && m.content == "ping from gui"),
            "restart context missing user: {ctx:?}"
        );
        if saw_assistant {
            assert!(
                ctx.iter().any(|m| m.role == "assistant"),
                "restart context missing assistant: {ctx:?}"
            );
        }
        let _ = live.child.kill();
        let _ = std::fs::remove_dir_all(&script_home);
    }
    #[test]
    fn handshake_advertises_git_and_worktree() {
        let mock = start_catalog_mock("host-a", "tok-1");
        let _session = connect(&pid("host-a", &mock.origin)).expect("online");
        let hs = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "handshake")
            .cloned()
            .expect("handshake");
        for name in [
            "worktree.ensure",
            "worktree.get",
            "worktree.list",
            "git.status",
            "git.diff",
        ] {
            assert_eq!(hs.params["methods"][name]["major"], 1, "{name}");
            assert_eq!(hs.params["methods"][name]["minor"], 0, "{name}");
        }
    }

    #[test]
    fn handshake_advertises_agent_cancel() {
        let mock = start_catalog_mock("host-a", "tok-1");
        let _session = connect(&pid("host-a", &mock.origin)).expect("online");
        let hs = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "handshake")
            .cloned()
            .expect("handshake");
        assert_eq!(hs.params["methods"]["agent.cancel"]["major"], 1);
        assert_eq!(hs.params["methods"]["agent.cancel"]["minor"], 0);
        assert_eq!(hs.params["client"], "gui");
    }

    #[test]
    fn worktree_ensure_then_tree_sends_worktree_id() {
        let mock = start_agent_files_mock("host-a", "tok-1");
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        let wt = session.worktree_ensure("ag-1").expect("ensure");
        assert_eq!(wt.id, "wt-1");
        assert_eq!(wt.agent_id, "ag-1");
        let _tree = session
            .files_tree_for("ws-1", "", Some(&wt.id))
            .expect("tree");
        let hits = mock.hits.lock().unwrap().clone();
        let ensure = hits.iter().find(|h| h.method == "worktree.ensure").unwrap();
        assert_eq!(ensure.params["agentId"], "ag-1");
        let tree = hits
            .iter()
            .rev()
            .find(|h| h.method == "files.tree")
            .unwrap();
        assert_eq!(tree.params["workspaceId"], "ws-1");
        assert_eq!(tree.params["worktreeId"], "wt-1");
    }

    #[test]
    fn git_status_and_diff_send_workspace_and_path() {
        let mock = start_agent_files_mock("host-a", "tok-1");
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        let status = session.git_status("ws-1", Some("wt-1")).expect("status");
        assert_eq!(status.branch, "main");
        assert!(status.dirty);
        let diff = session
            .git_diff("ws-1", Some("wt-1"), Some("src/lib.rs"))
            .expect("diff");
        assert_eq!(diff.files[0].path, "src/lib.rs");
        let hits = mock.hits.lock().unwrap().clone();
        let st = hits.iter().find(|h| h.method == "git.status").unwrap();
        assert_eq!(st.params["workspaceId"], "ws-1");
        assert_eq!(st.params["worktreeId"], "wt-1");
        let df = hits.iter().find(|h| h.method == "git.diff").unwrap();
        assert_eq!(df.params["path"], "src/lib.rs");
        assert_eq!(df.params["worktreeId"], "wt-1");
    }

    #[test]
    fn isolate_selected_agent_sends_ensure_and_tree() {
        use crate::state::{AgentStatus, AgentStub, AppState, HostStatus};
        let mock = start_agent_files_mock("host-a", "tok-1");
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        let mut state = AppState::new();
        state.pending_discover = false;
        state.demo = false;
        state.host_status = HostStatus::Online;
        state.session = Some(session);
        state.workspace_id = Some("ws-1".into());
        state.selected_task_id = Some("task-1".into());
        state.selected_agent_id = Some("ag-1".into());
        state.agents.push(AgentStub {
            id: "ag-1".into(),
            task_id: "task-1".into(),
            parent_id: None,
            provider: "cli.generic".into(),
            status: AgentStatus::Idle,
            interface: "chat".into(),
        });
        state.isolate_selected_agent();
        assert_eq!(state.worktree.as_ref().map(|w| w.id.as_str()), Some("wt-1"));
        let hits = mock.hits.lock().unwrap().clone();
        let ensure = hits
            .iter()
            .find(|h| h.method == "worktree.ensure")
            .expect("ensure");
        assert_eq!(ensure.params["agentId"], "ag-1");
        assert!(
            hits.iter()
                .any(|h| h.method == "files.tree" && h.params["worktreeId"] == "wt-1"),
            "{hits:?}"
        );
    }

    #[test]
    fn handshake_advertises_policy_methods_1_1() {
        let mock = start_catalog_mock("host-a", "tok-1");
        let _session = connect(&pid("host-a", &mock.origin)).expect("online");
        let hs = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "handshake")
            .cloned()
            .expect("handshake");
        for name in [
            METHOD_POLICY_GET,
            METHOD_POLICY_SET,
            METHOD_APPROVAL_RESPOND,
        ] {
            assert_eq!(hs.params["methods"][name]["major"], 1, "{name}");
            assert_eq!(hs.params["methods"][name]["minor"], 1, "{name}");
        }
    }

    fn start_doctor_policy_mock(mode: &'static str) -> CatalogMock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let hits_t = hits.clone();
        thread::spawn(move || {
            for stream in listener.incoming().take(24) {
                let Ok(mut stream) = stream else { break };
                let (headers, body) = read_http_request(&mut stream);
                let has_session = headers.to_ascii_lowercase().contains("x-rt-session:");
                let (method, params) = if headers.starts_with("GET /health") {
                    ("GET /health".to_string(), json!({}))
                } else {
                    let parsed: Value = serde_json::from_slice(&body).unwrap_or(json!({}));
                    (
                        parsed
                            .get("method")
                            .and_then(|v| v.as_str())
                            .unwrap_or("other")
                            .to_string(),
                        parsed.get("params").cloned().unwrap_or(json!({})),
                    )
                };
                hits_t.lock().unwrap().push(RpcHit {
                    method: method.clone(),
                    params: params.clone(),
                    has_session,
                });
                let body = match (mode, method.as_str()) {
                    (_, "GET /health") => json!({"ok": true, "hostId": "host-a"}).to_string(),
                    (_, "handshake") => json!({
                        "id": "echo",
                        "ok": {
                            "hostId": "host-a",
                            "hostVersion": "0.1.0",
                            "sessionToken": "tok-1",
                            "accepted": {
                                "policy.get": {"major": 1, "minor": 1},
                                "policy.set": {"major": 1, "minor": 1},
                                "approval.respond": {"major": 1, "minor": 1}
                            },
                            "rejected": {}
                        }
                    })
                    .to_string(),
                    (_, "host.ping") => json!({
                        "id": "echo",
                        "ok": { "hostId": "host-a", "now": "2026-08-17T12:00:00Z" }
                    })
                    .to_string(),
                    ("ok", "host.doctor") => json!({
                        "id": "echo",
                        "ok": {
                            "hostId": "host-a",
                            "providers": [
                                {
                                    "id": "byoa.foo",
                                    "available": true,
                                    "detail": "/bin/foo",
                                    "caps": {
                                        "oneShot": true,
                                        "longLived": false,
                                        "streamTokens": true,
                                        "tools": false,
                                        "sessionResume": false,
                                        "a2aInbox": false,
                                        "pty": false,
                                        "needsApiKey": false,
                                        "apiKeyEnv": null
                                    }
                                },
                                {
                                    "id": "cli.claude",
                                    "available": false,
                                    "detail": "missing"
                                }
                            ]
                        }
                    })
                    .to_string(),
                    ("ok", "policy.get") => json!({
                        "id": "echo",
                        "ok": {
                            "mode": "ask",
                            "scope": "agent",
                            "yolo": false,
                            "source": "default"
                        }
                    })
                    .to_string(),
                    ("ok", "policy.set") => {
                        let yolo = params
                            .get("yolo")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let mode = params.get("mode").and_then(|v| v.as_str()).unwrap_or("ask");
                        json!({
                            "id": "echo",
                            "ok": {
                                "mode": mode,
                                "scope": "agent",
                                "yolo": yolo,
                                "source": "agent"
                            }
                        })
                        .to_string()
                    }
                    ("ok", "approval.respond") => json!({
                        "id": "echo",
                        "ok": { "applied": true }
                    })
                    .to_string(),
                    ("old", "host.doctor")
                    | ("old", "policy.get")
                    | ("old", "policy.set")
                    | ("old", "approval.respond") => json!({
                        "id": "echo",
                        "error": { "code": "unsupported_method", "message": "no 1.1" }
                    })
                    .to_string(),
                    _ => json!({
                        "id": "echo",
                        "error": { "code": "unsupported_method", "message": "no" }
                    })
                    .to_string(),
                };
                write_http_json(&mut stream, &body);
            }
        });
        CatalogMock {
            origin: format!("http://{addr}"),
            hits,
        }
    }

    #[test]
    fn host_doctor_parses_providers_and_optional_caps() {
        let mock = start_doctor_policy_mock("ok");
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        let doctor = session.host_doctor().expect("doctor");
        assert_eq!(doctor.providers.len(), 2);
        assert_eq!(doctor.providers[0].id, "byoa.foo");
        assert!(doctor.providers[0].available);
        let caps = doctor.providers[0].caps.as_ref().expect("caps");
        assert!(caps.one_shot);
        assert!(!caps.pty);
        assert_eq!(doctor.providers[1].id, "cli.claude");
        assert!(doctor.providers[1].caps.is_none());
        let hit = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "host.doctor")
            .cloned()
            .expect("host.doctor");
        assert!(hit.has_session);
    }

    #[test]
    fn policy_and_approval_roundtrip() {
        let mock = start_doctor_policy_mock("ok");
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        assert!(session.ladder_accepted());
        let got = session.policy_get("ag-1").expect("get");
        assert_eq!(got.mode, "ask");
        assert!(!got.yolo);
        assert_eq!(got.source, "default");
        let set = session
            .policy_set("ag-1", "allow-always", "agent", true)
            .expect("set");
        assert_eq!(set.mode, "allow-always");
        assert!(set.yolo);
        let resp = session
            .approval_respond("ap-1", "allow-once")
            .expect("respond");
        assert!(resp.applied);
        let hits = mock.hits.lock().unwrap().clone();
        let get = hits.iter().find(|h| h.method == "policy.get").unwrap();
        assert_eq!(get.params["agentId"], "ag-1");
        let set_hit = hits.iter().find(|h| h.method == "policy.set").unwrap();
        assert_eq!(set_hit.params["agentId"], "ag-1");
        assert_eq!(set_hit.params["mode"], "allow-always");
        assert_eq!(set_hit.params["yolo"], true);
        let ap = hits
            .iter()
            .find(|h| h.method == "approval.respond")
            .unwrap();
        assert_eq!(ap.params["approvalId"], "ap-1");
        assert_eq!(ap.params["decision"], "allow-once");
    }

    #[test]
    fn policy_get_missing_method_is_error_not_panic() {
        let mock = start_doctor_policy_mock("old");
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        let err = session.policy_get("ag-1").unwrap_err();
        assert!(err.is_unsupported_method(), "{err:?}");
        let label = err.as_label();
        assert!(label.contains("unsupported_method"), "{label}");
        let err = session.host_doctor().unwrap_err();
        assert!(err.is_unsupported_method());
        let _ = mock;
    }

    fn write_accepted_map() -> Value {
        let mut accepted = serde_json::Map::new();
        for name in WRITE_METHODS {
            accepted.insert(name.to_string(), json!({"major": 1, "minor": 2}));
        }
        Value::Object(accepted)
    }

    fn git_status_ok() -> Value {
        json!({
            "branch": "main",
            "dirty": true,
            "truncated": false,
            "entries": [{ "path": "src/lib.rs", "status": "modified" }]
        })
    }

    fn start_write_mock(mode: &'static str) -> CatalogMock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let hits_t = hits.clone();
        thread::spawn(move || {
            for stream in listener.incoming().take(40) {
                let Ok(mut stream) = stream else { break };
                let (headers, body) = read_http_request(&mut stream);
                let has_session = headers.to_ascii_lowercase().contains("x-rt-session:");
                let (method, params) = if headers.starts_with("GET /health") {
                    ("GET /health".to_string(), json!({}))
                } else {
                    let parsed: Value = serde_json::from_slice(&body).unwrap_or(json!({}));
                    (
                        parsed
                            .get("method")
                            .and_then(|v| v.as_str())
                            .unwrap_or("other")
                            .to_string(),
                        parsed.get("params").cloned().unwrap_or(json!({})),
                    )
                };
                hits_t.lock().unwrap().push(RpcHit {
                    method: method.clone(),
                    params: params.clone(),
                    has_session,
                });
                let body = match (mode, method.as_str()) {
                    (_, "GET /health") => json!({"ok": true, "hostId": "host-a"}).to_string(),
                    ("ok" | "require_task", "handshake") => json!({
                        "id": "echo",
                        "ok": {
                            "hostId": "host-a",
                            "hostVersion": "0.1.0",
                            "sessionToken": "tok-1",
                            "accepted": write_accepted_map(),
                            "rejected": {}
                        }
                    })
                    .to_string(),
                    ("old", "handshake") => json!({
                        "id": "echo",
                        "ok": {
                            "hostId": "host-a",
                            "hostVersion": "0.1.0",
                            "sessionToken": "tok-1",
                            "accepted": {},
                            "rejected": {
                                "git.stage": {"reason": "unsupported"},
                                "git.unstage": {"reason": "unsupported"},
                                "git.commit": {"reason": "unsupported"},
                                "git.push": {"reason": "unsupported"},
                                "git.restore": {"reason": "unsupported"},
                                "files.open": {"reason": "unsupported"},
                                "files.write": {"reason": "unsupported"},
                                "files.patch": {"reason": "unsupported"}
                            }
                        }
                    })
                    .to_string(),
                    (_, "host.ping") => json!({
                        "id": "echo",
                        "ok": { "hostId": "host-a", "now": "2026-08-17T12:00:00Z" }
                    })
                    .to_string(),
                    ("ok", "git.stage" | "git.unstage" | "git.restore") => json!({
                        "id": "echo",
                        "ok": git_status_ok()
                    })
                    .to_string(),
                    ("ok", "git.commit") => json!({
                        "id": "echo",
                        "ok": { "commit": "abc1234", "branch": "main" }
                    })
                    .to_string(),
                    ("ok", "git.push") => json!({
                        "id": "echo",
                        "ok": { "remote": "origin", "ref": "main", "ok": true }
                    })
                    .to_string(),
                    ("ok", "files.open") => json!({
                        "id": "echo",
                        "ok": { "opened": true }
                    })
                    .to_string(),
                    _ => json!({
                        "id": "echo",
                        "error": { "code": "unsupported_method", "message": "no 1.2" }
                    })
                    .to_string(),
                };
                write_http_json(&mut stream, &body);
            }
        });
        CatalogMock {
            origin: format!("http://{addr}"),
            hits,
        }
    }

    #[test]
    fn handshake_advertises_write_methods_1_2() {
        let mock = start_catalog_mock("host-a", "tok-1");
        let _session = connect(&pid("host-a", &mock.origin)).expect("online");
        let hs = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "handshake")
            .cloned()
            .expect("handshake");
        for name in WRITE_METHODS {
            assert_eq!(hs.params["methods"][name]["major"], 1, "{name}");
            assert_eq!(hs.params["methods"][name]["minor"], 2, "{name}");
        }
    }

    #[test]
    fn write_rpcs_send_right_methods_and_params() {
        let mock = start_write_mock("ok");
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        assert!(session.write_accepted());
        let staged = session
            .git_stage("ws-1", Some("wt-1"), &["src/lib.rs"])
            .expect("stage");
        assert_eq!(staged.branch, "main");
        let unstaged = session
            .git_unstage("ws-1", Some("wt-1"), &["src/lib.rs"])
            .expect("unstage");
        assert!(unstaged.dirty);
        let restored = session
            .git_restore("ws-1", Some("wt-1"), &["src/lib.rs"], false)
            .expect("restore");
        assert_eq!(restored.entries[0].path, "src/lib.rs");
        let commit = session
            .git_commit("ws-1", Some("wt-1"), "feat: demo")
            .expect("commit");
        assert_eq!(commit.commit, "abc1234");
        assert_eq!(commit.branch, "main");
        let push = session
            .git_push("ws-1", Some("wt-1"), None, None)
            .expect("push");
        assert!(push.ok);
        assert_eq!(push.remote, "origin");
        assert_eq!(push.git_ref, "main");
        let opened = session
            .files_open("ws-1", Some("wt-1"), "src/lib.rs")
            .expect("open");
        assert!(opened.opened);

        let hits = mock.hits.lock().unwrap().clone();
        let stage = hits.iter().find(|h| h.method == "git.stage").unwrap();
        assert_eq!(stage.params["workspaceId"], "ws-1");
        assert_eq!(stage.params["worktreeId"], "wt-1");
        assert_eq!(stage.params["paths"][0], "src/lib.rs");
        assert!(stage.has_session);
        let unstage = hits.iter().find(|h| h.method == "git.unstage").unwrap();
        assert_eq!(unstage.params["paths"][0], "src/lib.rs");
        let restore = hits.iter().find(|h| h.method == "git.restore").unwrap();
        assert_eq!(restore.params["paths"][0], "src/lib.rs");
        assert_eq!(restore.params["staged"], false);
        let commit_hit = hits.iter().find(|h| h.method == "git.commit").unwrap();
        assert_eq!(commit_hit.params["message"], "feat: demo");
        let push_hit = hits.iter().find(|h| h.method == "git.push").unwrap();
        assert_eq!(push_hit.params["workspaceId"], "ws-1");
        assert!(push_hit.params.get("remote").is_none());
        let open = hits.iter().find(|h| h.method == "files.open").unwrap();
        assert_eq!(open.params["path"], "src/lib.rs");
        assert_eq!(open.params["workspaceId"], "ws-1");
        assert_eq!(open.params["worktreeId"], "wt-1");
    }

    #[test]
    fn old_host_write_methods_error_not_panic() {
        let mock = start_write_mock("old");
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        assert!(!session.write_accepted());
        assert!(session.write_rejected());
        let err = session.git_push("ws-1", None, None, None).unwrap_err();
        assert!(err.is_write_unsupported(), "{err:?}");
        let label = err.as_label();
        assert!(label.contains("unsupported_method"), "{label}");
        let err = session.files_open("ws-1", None, "src/lib.rs").unwrap_err();
        assert!(err.is_unsupported_method());
        let _ = mock;
    }

    fn pty_accepted_map() -> Value {
        let mut accepted = serde_json::Map::new();
        for name in PTY_METHODS {
            accepted.insert(name.to_string(), json!({"major": 1, "minor": 3}));
        }
        Value::Object(accepted)
    }

    fn start_pty_mock(mode: &'static str) -> CatalogMock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let hits_t = hits.clone();
        thread::spawn(move || {
            for stream in listener.incoming().take(40) {
                let Ok(mut stream) = stream else { break };
                let (headers, body) = read_http_request(&mut stream);
                let has_session = headers.to_ascii_lowercase().contains("x-rt-session:");
                let (method, params) = if headers.starts_with("GET /health") {
                    ("GET /health".to_string(), json!({}))
                } else {
                    let parsed: Value = serde_json::from_slice(&body).unwrap_or(json!({}));
                    (
                        parsed
                            .get("method")
                            .and_then(|v| v.as_str())
                            .unwrap_or("other")
                            .to_string(),
                        parsed.get("params").cloned().unwrap_or(json!({})),
                    )
                };
                hits_t.lock().unwrap().push(RpcHit {
                    method: method.clone(),
                    params: params.clone(),
                    has_session,
                });
                let body = match (mode, method.as_str()) {
                    (_, "GET /health") => json!({"ok": true, "hostId": "host-a"}).to_string(),
                    ("ok" | "require_task", "handshake") => json!({
                        "id": "echo",
                        "ok": {
                            "hostId": "host-a",
                            "hostVersion": "0.1.0",
                            "sessionToken": "tok-1",
                            "accepted": pty_accepted_map(),
                            "rejected": {}
                        }
                    })
                    .to_string(),
                    ("old", "handshake") => json!({
                        "id": "echo",
                        "ok": {
                            "hostId": "host-a",
                            "hostVersion": "0.1.0",
                            "sessionToken": "tok-1",
                            "accepted": {},
                            "rejected": {
                                "shell.create": {"reason": "unsupported"},
                                "shell.list": {"reason": "unsupported"},
                                "shell.close": {"reason": "unsupported"},
                                "pty.open": {"reason": "unsupported"},
                                "pty.write": {"reason": "unsupported"},
                                "pty.resize": {"reason": "unsupported"},
                                "pty.close": {"reason": "unsupported"}
                            }
                        }
                    })
                    .to_string(),
                    (_, "host.ping") => json!({
                        "id": "echo",
                        "ok": { "hostId": "host-a", "now": "2026-08-17T12:00:00Z" }
                    })
                    .to_string(),
                    ("ok", "shell.create") => json!({
                        "id": "echo",
                        "ok": {
                            "shellId": "sh-1",
                            "ptyId": "pty-shell-1",
                            "cwd": "/tmp/proj"
                        }
                    })
                    .to_string(),
                    ("require_task", "shell.create") => {
                        if params.get("taskId").and_then(|v| v.as_str()).is_none() {
                            json!({
                                "id": "echo",
                                "error": {
                                    "code": "invalid_params",
                                    "message": "taskId is required"
                                }
                            })
                            .to_string()
                        } else {
                            json!({
                                "id": "echo",
                                "ok": {
                                    "shellId": "sh-1",
                                    "ptyId": "pty-shell-1",
                                    "cwd": "/tmp/proj"
                                }
                            })
                            .to_string()
                        }
                    }
                    ("ok", "shell.list") => json!({
                        "id": "echo",
                        "ok": {
                            "items": [{
                                "shellId": "sh-1",
                                "ptyId": "pty-shell-1",
                                "cwd": "/tmp/proj"
                            }]
                        }
                    })
                    .to_string(),
                    ("ok", "shell.close") => json!({ "id": "echo", "ok": {} }).to_string(),
                    ("ok", "pty.open") => json!({
                        "id": "echo",
                        "ok": { "ptyId": "pty-ag-1", "resumed": false }
                    })
                    .to_string(),
                    ("ok", "pty.write" | "pty.resize" | "pty.close") => {
                        json!({ "id": "echo", "ok": {} }).to_string()
                    }
                    ("ok", "agent.create") => {
                        let task_id = params.get("taskId").and_then(|v| v.as_str()).unwrap_or("");
                        let interface = params
                            .get("interface")
                            .and_then(|v| v.as_str())
                            .unwrap_or("chat");
                        json!({
                            "id": "echo",
                            "ok": {
                                "id": "ag-term",
                                "taskId": task_id,
                                "hostId": "host-a",
                                "parentId": null,
                                "interface": interface,
                                "provider": params.get("provider").and_then(|v| v.as_str()).unwrap_or("cli.claude"),
                                "status": "idle",
                                "runLocation": "local",
                                "createdAt": "2026-08-17T12:00:00Z"
                            }
                        })
                        .to_string()
                    }
                    _ => json!({
                        "id": "echo",
                        "error": { "code": "unsupported_method", "message": "no 1.3" }
                    })
                    .to_string(),
                };
                write_http_json(&mut stream, &body);
            }
        });
        CatalogMock {
            origin: format!("http://{addr}"),
            hits,
        }
    }

    #[test]
    fn handshake_advertises_pty_methods_1_3() {
        let mock = start_catalog_mock("host-a", "tok-1");
        let _session = connect(&pid("host-a", &mock.origin)).expect("online");
        let hs = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "handshake")
            .cloned()
            .expect("handshake");
        for name in PTY_METHODS {
            assert_eq!(hs.params["methods"][name]["major"], 1, "{name}");
            assert_eq!(hs.params["methods"][name]["minor"], 3, "{name}");
        }
    }

    #[test]
    fn handshake_advertises_artifact_methods_1_4() {
        let mock = start_catalog_mock("host-a", "tok-1");
        let _session = connect(&pid("host-a", &mock.origin)).expect("online");
        let hs = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "handshake")
            .cloned()
            .expect("handshake");
        for name in ARTIFACT_METHODS {
            assert_eq!(hs.params["methods"][name]["major"], 1, "{name}");
            assert_eq!(hs.params["methods"][name]["minor"], 4, "{name}");
        }
        assert_eq!(hs.params["methods"]["artifact.create"]["minor"], 4);
        assert_eq!(hs.params["methods"]["artifact.export"]["minor"], 4);
        assert_eq!(hs.params["methods"]["agent.clear_transcript"]["minor"], 4);
        assert_eq!(hs.params["methods"]["comment.create"]["minor"], 4);
    }

    #[test]
    fn handshake_advertises_a2a_methods_1_5() {
        let mock = start_catalog_mock("host-a", "tok-1");
        let _session = connect(&pid("host-a", &mock.origin)).expect("online");
        let hs = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "handshake")
            .cloned()
            .expect("handshake");
        for name in A2A_METHODS {
            assert_eq!(hs.params["methods"][name]["major"], 1, "{name}");
            assert_eq!(hs.params["methods"][name]["minor"], 5, "{name}");
        }
        assert_eq!(hs.params["methods"]["a2a.deliver"]["minor"], 5);
        assert_eq!(hs.params["methods"]["loop.start"]["minor"], 5);
        assert_eq!(hs.params["methods"]["loop.stop"]["minor"], 5);
        assert_eq!(hs.params["methods"]["agent.create"]["minor"], 0);
    }

    #[test]
    fn handshake_advertises_model_methods_1_6() {
        let mock = start_catalog_mock("host-a", "tok-1");
        let _session = connect(&pid("host-a", &mock.origin)).expect("online");
        let hs = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "handshake")
            .cloned()
            .expect("handshake");
        for name in MODEL_METHODS {
            assert_eq!(hs.params["methods"][name]["major"], 1, "{name}");
            assert_eq!(hs.params["methods"][name]["minor"], 6, "{name}");
        }
        assert_eq!(hs.params["methods"]["agent.switch"]["minor"], 6);
        assert_eq!(hs.params["methods"]["profile.create"]["minor"], 6);
        assert_eq!(hs.params["methods"]["profile.list"]["minor"], 6);
        assert_eq!(hs.params["methods"]["prefs.get"]["minor"], 6);
        assert_eq!(hs.params["methods"]["a2a.deliver"]["minor"], 5);
        assert_eq!(hs.params["methods"]["agent.create"]["minor"], 0);
    }

    #[test]
    fn handshake_advertises_workspace_methods_1_7() {
        let mock = start_catalog_mock("host-a", "tok-1");
        let _session = connect(&pid("host-a", &mock.origin)).expect("online");
        let hs = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "handshake")
            .cloned()
            .expect("handshake");
        for name in WORKSPACE_METHODS {
            assert_eq!(hs.params["methods"][name]["major"], 1, "{name}");
            assert_eq!(hs.params["methods"][name]["minor"], 7, "{name}");
        }
        assert_eq!(hs.params["methods"]["workspace.guides.get"]["minor"], 7);
        assert_eq!(hs.params["methods"]["settings.guide.get"]["minor"], 7);
        assert_eq!(hs.params["methods"]["settings.guide.set"]["minor"], 7);
        assert_eq!(hs.params["methods"]["preset.list"]["minor"], 7);
        assert_eq!(hs.params["methods"]["agent.update"]["minor"], 7);
        assert_eq!(hs.params["methods"]["agent.switch"]["minor"], 6);
        assert_eq!(hs.params["methods"]["agent.create"]["minor"], 0);
    }

    #[test]
    fn handshake_advertises_sync_methods_1_8() {
        let mock = start_catalog_mock("host-a", "tok-1");
        let _session = connect(&pid("host-a", &mock.origin)).expect("online");
        let hs = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "handshake")
            .cloned()
            .expect("handshake");
        for name in SYNC_METHODS {
            assert_eq!(hs.params["methods"][name]["major"], 1, "{name}");
            assert_eq!(hs.params["methods"][name]["minor"], 8, "{name}");
        }
        assert_eq!(hs.params["methods"]["sync.export"]["minor"], 8);
        assert_eq!(hs.params["methods"]["sync.import"]["minor"], 8);
        assert_eq!(hs.params["methods"]["workspace.guides.get"]["minor"], 7);
        assert_eq!(hs.params["methods"]["agent.switch"]["minor"], 6);
        assert_eq!(hs.params["methods"]["agent.create"]["minor"], 0);
    }

    fn start_artifacts_rpc_mock() -> CatalogMock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let hits_t = hits.clone();
        thread::spawn(move || {
            for stream in listener.incoming().take(40) {
                let Ok(mut stream) = stream else { break };
                let (headers, body) = read_http_request(&mut stream);
                let has_session = headers.to_ascii_lowercase().contains("x-rt-session:");
                let (method, params) = if headers.starts_with("GET /health") {
                    ("GET /health".to_string(), json!({}))
                } else {
                    let parsed: Value = serde_json::from_slice(&body).unwrap_or(json!({}));
                    (
                        parsed
                            .get("method")
                            .and_then(|v| v.as_str())
                            .unwrap_or("other")
                            .to_string(),
                        parsed.get("params").cloned().unwrap_or(json!({})),
                    )
                };
                hits_t.lock().unwrap().push(RpcHit {
                    method: method.clone(),
                    params: params.clone(),
                    has_session,
                });
                let kind = params
                    .get("kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("spec")
                    .to_string();
                let status = if kind == "ticket" || kind == "story" {
                    json!("todo")
                } else {
                    Value::Null
                };
                let sample = json!({
                    "id": "art-1",
                    "taskId": "task-1",
                    "parentId": null,
                    "kind": kind,
                    "title": params.get("title").cloned().unwrap_or(json!("Auth")),
                    "body": params.get("body").cloned().unwrap_or(json!("# Auth")),
                    "status": status,
                    "assignee": null,
                    "sourceMessageId": null,
                    "createdAt": "t",
                    "updatedAt": "t"
                });
                let thread = json!({
                    "id": "th-1",
                    "artifactId": "art-1",
                    "anchorStart": 0,
                    "anchorEnd": 12,
                    "resolved": method == "comment.resolve",
                    "comments": [{ "id": "c-1", "body": "nit", "createdAt": "t" }],
                    "createdAt": "t",
                    "updatedAt": "t"
                });
                let body = match method.as_str() {
                    "GET /health" => json!({"ok": true, "hostId": "host-a"}).to_string(),
                    "handshake" => {
                        let mut accepted = serde_json::Map::new();
                        for name in ARTIFACT_METHODS {
                            accepted.insert(name.to_string(), json!({"major": 1, "minor": 4}));
                        }
                        json!({
                            "id": "echo",
                            "ok": {
                                "hostId": "host-a",
                                "hostVersion": "0.1.0",
                                "sessionToken": "tok-1",
                                "accepted": accepted,
                                "rejected": {}
                            }
                        })
                        .to_string()
                    }
                    "host.ping" => json!({
                        "id": "echo",
                        "ok": { "hostId": "host-a", "now": "t" }
                    })
                    .to_string(),
                    "artifact.create" | "artifact.get" | "artifact.update" => {
                        json!({ "id": "echo", "ok": sample }).to_string()
                    }
                    "artifact.list" => json!({
                        "id": "echo",
                        "ok": { "items": [sample], "truncated": false }
                    })
                    .to_string(),
                    "artifact.export" => {
                        let format = params
                            .get("format")
                            .and_then(|v| v.as_str())
                            .unwrap_or("md");
                        if format == "pdf" {
                            json!({
                                "id": "echo",
                                "ok": {
                                    "format": "pdf",
                                    "markdown": "",
                                    "filename": "art-1.pdf",
                                    "bytes": crate::terminal::encode_b64(b"%PDF-1.4 test")
                                }
                            })
                            .to_string()
                        } else {
                            json!({
                                "id": "echo",
                                "ok": {
                                    "format": format,
                                    "markdown": "# Auth",
                                    "filename": "art-1.md"
                                }
                            })
                            .to_string()
                        }
                    }
                    "artifact.delete" => json!({
                        "id": "echo",
                        "ok": { "deleted": ["art-1"] }
                    })
                    .to_string(),
                    "comment.create" | "comment.resolve" => {
                        json!({ "id": "echo", "ok": thread }).to_string()
                    }
                    "comment.list" => json!({
                        "id": "echo",
                        "ok": { "threads": [thread] }
                    })
                    .to_string(),
                    "agent.clear_transcript" => json!({
                        "id": "echo",
                        "ok": { "cleared": 2 }
                    })
                    .to_string(),
                    _ => json!({
                        "id": "echo",
                        "error": { "code": "unsupported_method", "message": "no" }
                    })
                    .to_string(),
                };
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(resp.as_bytes());
            }
        });
        CatalogMock {
            origin: format!("http://{addr}"),
            hits,
        }
    }

    #[test]
    fn artifact_rpc_methods_and_export_format() {
        let mock = start_artifacts_rpc_mock();
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        assert!(session.artifacts_accepted());
        let created = session
            .artifact_create("task-1", "spec", "Auth", "# Auth\n", None, None)
            .expect("create");
        assert_eq!(created.kind, "spec");
        session
            .artifact_update("art-1", None, Some("body"), None, None, None)
            .expect("update");
        let exported = session
            .artifact_export("art-1", crate::artifacts::EXPORT_FORMAT)
            .expect("export");
        assert_eq!(exported.format, "md");
        let pdf = session
            .artifact_export("art-1", crate::artifacts::EXPORT_FORMAT_PDF)
            .expect("pdf");
        assert_eq!(pdf.format, "pdf");
        assert_eq!(pdf.filename, "art-1.pdf");
        assert_eq!(
            crate::terminal::decode_b64(&pdf.bytes).expect("b64"),
            b"%PDF-1.4 test"
        );
        session
            .comment_create("art-1", None, Some(0), Some(12), "nit")
            .expect("thread");
        session
            .comment_create("art-1", Some("th-1"), None, None, "reply")
            .expect("reply");
        session.comment_resolve("th-1").expect("resolve");
        session.agent_clear_transcript("ag-1").expect("clear");
        let hits = mock.hits.lock().unwrap().clone();
        let create = hits.iter().find(|h| h.method == "artifact.create").unwrap();
        assert_eq!(create.params["kind"], "spec");
        let update = hits.iter().find(|h| h.method == "artifact.update").unwrap();
        assert_eq!(update.params["artifactId"], "art-1");
        assert_eq!(update.params["body"], "body");
        assert!(update.params.get("path").is_none());
        let exports: Vec<_> = hits
            .iter()
            .filter(|h| h.method == "artifact.export")
            .collect();
        assert_eq!(exports.len(), 2);
        assert_eq!(exports[0].params["format"], "md");
        assert_eq!(exports[1].params["format"], "pdf");
        let thread = hits
            .iter()
            .find(|h| h.method == "comment.create" && h.params["threadId"].is_null())
            .unwrap();
        assert_eq!(thread.params["anchorStart"], 0);
        assert_eq!(thread.params["anchorEnd"], 12);
        let reply = hits
            .iter()
            .find(|h| h.method == "comment.create" && h.params["threadId"] == "th-1")
            .unwrap();
        assert_eq!(reply.params["body"], "reply");
        assert!(hits.iter().any(|h| h.method == "comment.resolve"));
        let clear = hits
            .iter()
            .find(|h| h.method == "agent.clear_transcript")
            .unwrap();
        assert_eq!(clear.params["agentId"], "ag-1");
        assert!(!hits.iter().any(|h| h.method == "files.write"));
    }

    #[test]
    fn pty_write_resize_send_correct_method_and_params() {
        let mock = start_pty_mock("ok");
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        assert!(session.terminal_accepted());
        session.pty_write("pty-1", b"ls\n").expect("write");
        session.pty_resize("pty-1", 100, 30).expect("resize");
        let created = session
            .shell_create(Some("task-1"), "ws-1", None, 80, 24)
            .expect("shell");
        assert_eq!(created.shell_id, "sh-1");
        assert_eq!(created.pty_id, "pty-shell-1");
        let listed = session.shell_list("task-1").expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].shell_id, "sh-1");
        session.pty_open_agent("ag-1", 80, 24).expect("open");

        let hits = mock.hits.lock().unwrap().clone();
        let write = hits.iter().find(|h| h.method == "pty.write").unwrap();
        assert_eq!(write.params["ptyId"], "pty-1");
        assert_eq!(write.params["data"], crate::terminal::encode_b64(b"ls\n"));
        assert!(write.has_session);
        let resize = hits.iter().find(|h| h.method == "pty.resize").unwrap();
        assert_eq!(resize.params["ptyId"], "pty-1");
        assert_eq!(resize.params["cols"], 100);
        assert_eq!(resize.params["rows"], 30);
        let create = hits.iter().find(|h| h.method == "shell.create").unwrap();
        assert_eq!(create.params["taskId"], "task-1");
        assert_eq!(create.params["workspaceId"], "ws-1");
        assert_eq!(create.params["cols"], 80);
        assert_eq!(create.params["rows"], 24);
        let open = hits.iter().find(|h| h.method == "pty.open").unwrap();
        assert_eq!(open.params["agentId"], "ag-1");
        assert!(open.params.get("shellId").is_none());
    }

    #[test]
    fn shell_create_without_task_omits_task_id() {
        let mock = start_pty_mock("ok");
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        session
            .shell_create(None, "ws-1", None, 80, 24)
            .expect("shell");
        let hits = mock.hits.lock().unwrap().clone();
        let create = hits.iter().find(|h| h.method == "shell.create").unwrap();
        assert!(
            create.params.get("taskId").is_none(),
            "taskId must be omitted, not null: {}",
            create.params
        );
        assert_eq!(create.params["workspaceId"], "ws-1");
        assert_eq!(create.params["cols"], 80);
        assert_eq!(create.params["rows"], 24);
    }

    #[test]
    fn shell_create_with_task_still_sends_task_id() {
        let mock = start_pty_mock("ok");
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        session
            .shell_create(Some("task-1"), "ws-1", None, 80, 24)
            .expect("shell");
        let hits = mock.hits.lock().unwrap().clone();
        let create = hits.iter().find(|h| h.method == "shell.create").unwrap();
        assert_eq!(create.params["taskId"], "task-1");
        assert_eq!(create.params["workspaceId"], "ws-1");
    }

    #[test]
    fn shell_create_without_task_invalid_params_is_error() {
        let mock = start_pty_mock("require_task");
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        assert!(session.terminal_accepted());
        let err = session
            .shell_create(None, "ws-1", None, 80, 24)
            .unwrap_err();
        assert!(err.is_invalid_params(), "{err:?}");
        let label = err.as_label();
        assert!(label.contains("invalid_params"), "{label}");
        assert!(label.contains("taskId is required"), "{label}");
    }

    #[test]
    fn old_host_pty_methods_error_not_panic() {
        let mock = start_pty_mock("old");
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        assert!(!session.terminal_accepted());
        assert!(session.terminal_rejected());
        let err = session.pty_write("pty-1", b"x").unwrap_err();
        assert!(err.is_pty_unsupported(), "{err:?}");
        let label = err.as_label();
        assert!(label.contains("unsupported_method"), "{label}");
        let err = session
            .shell_create(Some("task-1"), "ws-1", None, 80, 24)
            .unwrap_err();
        assert!(err.is_unsupported_method());
        let _ = mock;
    }

    #[test]
    fn agent_create_with_interface_sends_terminal() {
        let mock = start_pty_mock("ok");
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        let agent = session
            .agent_create_with_interface("task-1", "cli.claude", "terminal", None)
            .expect("create");
        assert_eq!(agent.interface, "terminal");
        assert_eq!(agent.id, "ag-term");
        let hit = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "agent.create")
            .cloned()
            .expect("agent.create");
        assert_eq!(hit.params["interface"], "terminal");
        assert_eq!(hit.params["taskId"], "task-1");
        assert_eq!(hit.params["provider"], "cli.claude");
    }

    fn model_accepted_map() -> serde_json::Map<String, Value> {
        let mut accepted = serde_json::Map::new();
        for name in MODEL_METHODS {
            accepted.insert(name.to_string(), json!({"major": 1, "minor": 6}));
        }
        accepted
    }

    fn sample_switched_agent(id: &str, provider: &str) -> Value {
        json!({
            "id": id,
            "taskId": "task-1",
            "hostId": "host-a",
            "parentId": null,
            "interface": "chat",
            "provider": provider,
            "status": "idle",
            "runLocation": "local",
            "createdAt": "2026-08-19T10:00:00Z",
            "model": "o3",
            "effort": "high",
            "fast": true
        })
    }

    fn start_model_rpc_mock() -> CatalogMock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let hits_t = hits.clone();
        thread::spawn(move || {
            let mut profile_n = 0u32;
            for stream in listener.incoming().take(24) {
                let Ok(mut stream) = stream else { break };
                let (headers, body) = read_http_request(&mut stream);
                let has_session = headers.to_ascii_lowercase().contains("x-rt-session:");
                let parsed: Value = serde_json::from_slice(&body).unwrap_or(json!({}));
                let method = if headers.starts_with("GET /health") {
                    "GET /health".to_string()
                } else {
                    parsed
                        .get("method")
                        .and_then(|v| v.as_str())
                        .unwrap_or("other")
                        .to_string()
                };
                let params = parsed.get("params").cloned().unwrap_or(json!({}));
                hits_t.lock().unwrap().push(RpcHit {
                    method: method.clone(),
                    params: params.clone(),
                    has_session,
                });
                let body = match method.as_str() {
                    "GET /health" => json!({"ok": true, "hostId": "host-a"}).to_string(),
                    "handshake" => json!({
                        "id": "echo",
                        "ok": {
                            "hostId": "host-a",
                            "hostVersion": "0.1.0",
                            "sessionToken": "tok-1",
                            "accepted": model_accepted_map(),
                            "rejected": {}
                        }
                    })
                    .to_string(),
                    "host.ping" => json!({
                        "id": "echo",
                        "ok": { "hostId": "host-a", "now": "2026-08-19T10:00:00Z" }
                    })
                    .to_string(),
                    "agent.switch" => {
                        let provider = params
                            .get("provider")
                            .and_then(|v| v.as_str())
                            .unwrap_or("cli.codex");
                        json!({
                            "id": "echo",
                            "ok": sample_switched_agent("ag-1", provider)
                        })
                        .to_string()
                    }
                    "profile.create" => {
                        profile_n += 1;
                        json!({
                            "id": "echo",
                            "ok": {
                                "id": format!("prof-{profile_n}"),
                                "name": params.get("name").cloned().unwrap_or(json!("p")),
                                "provider": params.get("provider").cloned().unwrap_or(json!("cli.codex")),
                                "model": params.get("model").cloned().unwrap_or(Value::Null),
                                "effort": params.get("effort").cloned().unwrap_or(Value::Null),
                                "fast": params.get("fast").cloned().unwrap_or(json!(false)),
                                "createdAt": "2026-08-19T10:00:00Z",
                                "updatedAt": "2026-08-19T10:00:00Z"
                            }
                        })
                        .to_string()
                    }
                    "profile.list" => json!({
                        "id": "echo",
                        "ok": {
                            "items": [{
                                "id": "prof-1",
                                "name": "codex high",
                                "provider": "cli.codex",
                                "model": "o3",
                                "effort": "high",
                                "fast": false
                            }]
                        }
                    })
                    .to_string(),
                    "profile.get" => json!({
                        "id": "echo",
                        "ok": {
                            "id": params.get("profileId").cloned().unwrap_or(json!("prof-1")),
                            "name": "codex high",
                            "provider": "cli.codex",
                            "model": "o3",
                            "effort": "high",
                            "fast": false
                        }
                    })
                    .to_string(),
                    "prefs.get" => json!({
                        "id": "echo",
                        "ok": {
                            "items": [{
                                "provider": "cli.codex",
                                "model": "o3",
                                "effort": "high",
                                "fast": true
                            }]
                        }
                    })
                    .to_string(),
                    _ => json!({
                        "id": "echo",
                        "error": { "code": "unsupported_method", "message": "no" }
                    })
                    .to_string(),
                };
                write_http_json(&mut stream, &body);
            }
        });
        CatalogMock {
            origin: format!("http://{addr}"),
            hits,
        }
    }

    #[test]
    fn agent_switch_sends_method_and_params() {
        let mock = start_model_rpc_mock();
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        assert!(session.model_ux_accepted());
        let view = session
            .agent_switch(
                "ag-1",
                AgentSwitchParams {
                    provider: Some("cli.codex"),
                    model: Some("o3"),
                    effort: Some("high"),
                    fast: Some(true),
                    ..AgentSwitchParams::default()
                },
            )
            .expect("switch");
        assert_eq!(view.agent.id, "ag-1");
        assert_eq!(view.agent.provider, "cli.codex");
        assert_eq!(view.model.as_deref(), Some("o3"));
        assert_eq!(view.effort.as_deref(), Some("high"));
        assert!(view.fast);
        let hit = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "agent.switch")
            .cloned()
            .expect("agent.switch");
        assert_eq!(hit.params["agentId"], "ag-1");
        assert_eq!(hit.params["provider"], "cli.codex");
        assert_eq!(hit.params["model"], "o3");
        assert_eq!(hit.params["effort"], "high");
        assert_eq!(hit.params["fast"], true);
        assert!(hit.params.get("profileId").is_none());
    }

    #[test]
    fn profile_create_list_get_and_prefs_rpc() {
        let mock = start_model_rpc_mock();
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        let created = session
            .profile_create(
                "codex high",
                "cli.codex",
                Some("o3"),
                Some("high"),
                Some(false),
            )
            .expect("create");
        assert_eq!(created.name, "codex high");
        assert_eq!(created.provider, "cli.codex");
        let items = session.profile_list().expect("list");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "prof-1");
        let got = session.profile_get("prof-1").expect("get");
        assert_eq!(got.model.as_deref(), Some("o3"));
        let prefs = session.prefs_get().expect("prefs");
        assert_eq!(prefs[0].provider, "cli.codex");
        assert_eq!(prefs[0].model.as_deref(), Some("o3"));
        assert!(prefs[0].fast);
        let hits = mock.hits.lock().unwrap().clone();
        let create = hits
            .iter()
            .find(|h| h.method == "profile.create")
            .expect("profile.create");
        assert_eq!(create.params["name"], "codex high");
        assert_eq!(create.params["provider"], "cli.codex");
        assert_eq!(create.params["model"], "o3");
        assert!(hits.iter().any(|h| h.method == "profile.list"));
        assert!(hits.iter().any(|h| h.method == "profile.get"));
        assert!(hits.iter().any(|h| h.method == "prefs.get"));
    }

    fn workspace_accepted_map() -> serde_json::Map<String, Value> {
        let mut accepted = serde_json::Map::new();
        for name in WORKSPACE_METHODS {
            accepted.insert(name.to_string(), json!({"major": 1, "minor": 7}));
        }
        accepted
    }

    fn sample_agent_role(id: &str, role: &str) -> Value {
        json!({
            "id": id,
            "taskId": "task-1",
            "hostId": "host-a",
            "parentId": null,
            "interface": "chat",
            "provider": "cli.generic",
            "status": "idle",
            "runLocation": "local",
            "createdAt": "2026-08-19T11:00:00Z",
            "role": role
        })
    }

    fn start_workspace_rpc_mock() -> CatalogMock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let hits_t = hits.clone();
        thread::spawn(move || {
            for stream in listener.incoming().take(24) {
                let Ok(mut stream) = stream else { break };
                let (headers, body) = read_http_request(&mut stream);
                let has_session = headers.to_ascii_lowercase().contains("x-rt-session:");
                let parsed: Value = serde_json::from_slice(&body).unwrap_or(json!({}));
                let method = if headers.starts_with("GET /health") {
                    "GET /health".to_string()
                } else {
                    parsed
                        .get("method")
                        .and_then(|v| v.as_str())
                        .unwrap_or("other")
                        .to_string()
                };
                let params = parsed.get("params").cloned().unwrap_or(json!({}));
                hits_t.lock().unwrap().push(RpcHit {
                    method: method.clone(),
                    params: params.clone(),
                    has_session,
                });
                let body = match method.as_str() {
                    "GET /health" => json!({"ok": true, "hostId": "host-a"}).to_string(),
                    "handshake" => json!({
                        "id": "echo",
                        "ok": {
                            "hostId": "host-a",
                            "hostVersion": "0.1.0",
                            "sessionToken": "tok-1",
                            "accepted": workspace_accepted_map(),
                            "rejected": {}
                        }
                    })
                    .to_string(),
                    "host.ping" => json!({
                        "id": "echo",
                        "ok": { "hostId": "host-a", "now": "2026-08-19T11:00:00Z" }
                    })
                    .to_string(),
                    "workspace.guides.get" => json!({
                        "id": "echo",
                        "ok": {
                            "agentsMd": {
                                "path": "/ws/AGENTS.md",
                                "content": "use the planner",
                                "truncated": false
                            },
                            "workspaceGuide": null,
                            "globalGuide": {
                                "path": "/data/agent-selection-guide.md",
                                "content": "prefer cli.codex",
                                "truncated": false
                            }
                        }
                    })
                    .to_string(),
                    "settings.guide.get" => json!({
                        "id": "echo",
                        "ok": {
                            "path": "/data/agent-selection-guide.md",
                            "content": "prefer cli.codex",
                            "truncated": false
                        }
                    })
                    .to_string(),
                    "settings.guide.set" => json!({
                        "id": "echo",
                        "ok": {
                            "path": "/data/agent-selection-guide.md",
                            "content": params.get("content").cloned().unwrap_or(json!("")),
                            "truncated": false
                        }
                    })
                    .to_string(),
                    "preset.list" => json!({
                        "id": "echo",
                        "ok": {
                            "items": [
                                { "id": "planning", "title": "Planning", "defaultRole": "planner" },
                                { "id": "review", "title": "Review", "defaultRole": "reviewer" },
                                { "id": "debug", "title": "Debug", "defaultRole": "debugger" },
                                { "id": "document", "title": "Document", "defaultRole": "documenter" }
                            ]
                        }
                    })
                    .to_string(),
                    "agent.update" => {
                        let role = params.get("role").and_then(|v| v.as_str()).unwrap_or("coder");
                        json!({
                            "id": "echo",
                            "ok": sample_agent_role("ag-1", role)
                        })
                        .to_string()
                    }
                    "agent.create" => {
                        let role = params.get("role").and_then(|v| v.as_str()).unwrap_or("coder");
                        json!({
                            "id": "echo",
                            "ok": sample_agent_role("ag-new", role)
                        })
                        .to_string()
                    }
                    "task.create" => {
                        let title = params.get("title").and_then(|v| v.as_str()).unwrap_or("");
                        let ws = params
                            .get("workspaceId")
                            .and_then(|v| v.as_str())
                            .unwrap_or("ws-1");
                        let mut task = sample_task("task-new", title, "open", ws);
                        if let Some(preset) = params.get("preset") {
                            task["preset"] = preset.clone();
                        }
                        json!({ "id": "echo", "ok": task }).to_string()
                    }
                    _ => json!({
                        "id": "echo",
                        "error": { "code": "unsupported_method", "message": "no" }
                    })
                    .to_string(),
                };
                write_http_json(&mut stream, &body);
            }
        });
        CatalogMock {
            origin: format!("http://{addr}"),
            hits,
        }
    }

    #[test]
    fn workspace_guides_get_uses_rpc_not_filesystem_path() {
        let mock = start_workspace_rpc_mock();
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        assert!(session.workspace_accepted());
        let guides = session.workspace_guides_get("ws-1").expect("guides");
        assert_eq!(
            guides.agents_md.as_ref().map(|f| f.path.as_str()),
            Some("/ws/AGENTS.md")
        );
        assert_eq!(
            guides.agents_md.as_ref().map(|f| f.content.as_str()),
            Some("use the planner")
        );
        assert!(guides.workspace_guide.is_none());
        let hits = mock.hits.lock().unwrap().clone();
        let get = hits
            .iter()
            .find(|h| h.method == "workspace.guides.get")
            .cloned()
            .expect("workspace.guides.get");
        assert_eq!(get.params["workspaceId"], "ws-1");
        assert!(get.params.get("path").is_none());
        assert!(hits.iter().all(|h| h.method != "files.tree"));
        assert!(hits.iter().all(|h| h.method != "files.read"));
        assert!(!hits.iter().any(|h| {
            h.params
                .get("path")
                .and_then(|v| v.as_str())
                .is_some_and(|p| p.contains("AGENTS.md"))
        }));
    }

    #[test]
    fn agent_update_sends_role_method() {
        let mock = start_workspace_rpc_mock();
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        let (agent, role) = session
            .agent_update_role("ag-1", "reviewer")
            .expect("update");
        assert_eq!(agent.id, "ag-1");
        assert_eq!(role.as_deref(), Some("reviewer"));
        let hit = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "agent.update")
            .cloned()
            .expect("agent.update");
        assert_eq!(hit.params["agentId"], "ag-1");
        assert_eq!(hit.params["role"], "reviewer");
        assert!(hit.params.get("token").is_none());
        assert!(hit.params.get("apiKey").is_none());
    }

    #[test]
    fn task_create_preset_sends_one_of_four_names() {
        let mock = start_workspace_rpc_mock();
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        for name in ["planning", "review", "debug", "document"] {
            let created = session
                .task_create_with_preset("Plan", "ws-1", Some(name))
                .expect(name);
            assert_eq!(created.title, "Plan");
        }
        let hits = mock.hits.lock().unwrap().clone();
        let presets: Vec<String> = hits
            .iter()
            .filter(|h| h.method == "task.create")
            .map(|h| h.params["preset"].as_str().expect("preset").to_string())
            .collect();
        assert_eq!(presets, vec!["planning", "review", "debug", "document"]);
        for name in &presets {
            assert!(["planning", "review", "debug", "document"].contains(&name.as_str()));
        }
    }

    fn sync_accepted_map() -> serde_json::Map<String, Value> {
        let mut accepted = serde_json::Map::new();
        for name in SYNC_METHODS {
            accepted.insert(name.to_string(), json!({"major": 1, "minor": 8}));
        }
        accepted
    }

    fn sample_archive() -> Value {
        json!({
            "kind": "rusttraycer.export",
            "exportVersion": 1,
            "sourceHostId": "host-a",
            "exportedAt": "2026-08-19T12:00:00Z",
            "tasks": [],
            "agents": [],
            "messages": [],
            "artifacts": [],
            "commentThreads": [],
            "comments": [],
            "modelProfiles": []
        })
    }

    fn start_sync_rpc_mock() -> CatalogMock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let hits_t = hits.clone();
        thread::spawn(move || {
            for stream in listener.incoming().take(24) {
                let Ok(mut stream) = stream else { break };
                let (headers, body) = read_http_request(&mut stream);
                let has_session = headers.to_ascii_lowercase().contains("x-rt-session:");
                let parsed: Value = serde_json::from_slice(&body).unwrap_or(json!({}));
                let method = if headers.starts_with("GET /health") {
                    "GET /health".to_string()
                } else {
                    parsed
                        .get("method")
                        .and_then(|v| v.as_str())
                        .unwrap_or("other")
                        .to_string()
                };
                let params = parsed.get("params").cloned().unwrap_or(json!({}));
                hits_t.lock().unwrap().push(RpcHit {
                    method: method.clone(),
                    params: params.clone(),
                    has_session,
                });
                let body = match method.as_str() {
                    "GET /health" => json!({"ok": true, "hostId": "host-a"}).to_string(),
                    "handshake" => json!({
                        "id": "echo",
                        "ok": {
                            "hostId": "host-a",
                            "hostVersion": "0.1.0",
                            "sessionToken": "tok-1",
                            "accepted": sync_accepted_map(),
                            "rejected": {}
                        }
                    })
                    .to_string(),
                    "host.ping" => json!({
                        "id": "echo",
                        "ok": { "hostId": "host-a", "now": "2026-08-19T12:00:00Z" }
                    })
                    .to_string(),
                    "sync.export" => json!({
                        "id": "echo",
                        "ok": { "archive": sample_archive() }
                    })
                    .to_string(),
                    "sync.import" => json!({
                        "id": "echo",
                        "ok": {
                            "tasks": 1,
                            "agents": 2,
                            "messages": 10,
                            "artifacts": 1,
                            "profilesImported": 0,
                            "profilesSkipped": 1
                        }
                    })
                    .to_string(),
                    _ => json!({
                        "id": "echo",
                        "error": { "code": "unsupported_method", "message": "no" }
                    })
                    .to_string(),
                };
                write_http_json(&mut stream, &body);
            }
        });
        CatalogMock {
            origin: format!("http://{addr}"),
            hits,
        }
    }

    #[test]
    fn sync_export_sends_task_ids() {
        let mock = start_sync_rpc_mock();
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        assert!(session.sync_accepted());
        let ok = session.sync_export(&["task-1".into()]).expect("export");
        assert_eq!(ok.archive["kind"], "rusttraycer.export");
        assert_eq!(ok.archive["sourceHostId"], "host-a");
        assert!(ok.archive.get("token").is_none());
        let hit = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "sync.export")
            .cloned()
            .expect("sync.export");
        assert_eq!(hit.params["taskIds"], json!(["task-1"]));
        assert!(hit.params.get("token").is_none());
        assert!(hit.params.get("apiKey").is_none());
    }

    #[test]
    fn sync_import_sends_workspace_and_archive() {
        let mock = start_sync_rpc_mock();
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        let archive = sample_archive();
        let ok = session
            .sync_import("ws-1", archive.clone())
            .expect("import");
        assert_eq!(ok["tasks"], 1);
        assert_eq!(ok["agents"], 2);
        let hit = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "sync.import")
            .cloned()
            .expect("sync.import");
        assert_eq!(hit.params["workspaceId"], "ws-1");
        assert_eq!(hit.params["archive"]["kind"], "rusttraycer.export");
        assert!(hit.params.get("token").is_none());
    }

    fn search_gc_accepted_map() -> serde_json::Map<String, Value> {
        let mut accepted = serde_json::Map::new();
        for name in SEARCH_GC_METHODS {
            accepted.insert(name.to_string(), json!({"major": 1, "minor": 9}));
        }
        accepted
    }

    fn start_search_gc_rpc_mock() -> CatalogMock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let hits_t = hits.clone();
        thread::spawn(move || {
            for stream in listener.incoming().take(24) {
                let Ok(mut stream) = stream else { break };
                let (headers, body) = read_http_request(&mut stream);
                let has_session = headers.to_ascii_lowercase().contains("x-rt-session:");
                let parsed: Value = serde_json::from_slice(&body).unwrap_or(json!({}));
                let method = if headers.starts_with("GET /health") {
                    "GET /health".to_string()
                } else {
                    parsed
                        .get("method")
                        .and_then(|v| v.as_str())
                        .unwrap_or("other")
                        .to_string()
                };
                let params = parsed.get("params").cloned().unwrap_or(json!({}));
                hits_t.lock().unwrap().push(RpcHit {
                    method: method.clone(),
                    params: params.clone(),
                    has_session,
                });
                let body = match method.as_str() {
                    "GET /health" => json!({"ok": true, "hostId": "host-a"}).to_string(),
                    "handshake" => json!({
                        "id": "echo",
                        "ok": {
                            "hostId": "host-a",
                            "hostVersion": "0.1.0",
                            "sessionToken": "tok-1",
                            "accepted": search_gc_accepted_map(),
                            "rejected": {}
                        }
                    })
                    .to_string(),
                    "host.ping" => json!({
                        "id": "echo",
                        "ok": { "hostId": "host-a", "now": "2026-08-19T12:00:00Z" }
                    })
                    .to_string(),
                    "search.query" => json!({
                        "id": "echo",
                        "ok": {
                            "items": [{
                                "kind": "task",
                                "id": "task-1",
                                "title": "Auth",
                                "hint": "open"
                            }]
                        }
                    })
                    .to_string(),
                    "worktree.gc" => json!({
                        "id": "echo",
                        "ok": { "dryRun": params.get("dryRun").cloned().unwrap_or(json!(false)), "deleted": [] }
                    })
                    .to_string(),
                    _ => json!({
                        "id": "echo",
                        "error": { "code": "unsupported_method", "message": "no" }
                    })
                    .to_string(),
                };
                write_http_json(&mut stream, &body);
            }
        });
        CatalogMock {
            origin: format!("http://{addr}"),
            hits,
        }
    }

    #[test]
    fn handshake_advertises_search_gc_1_9_and_keeps_1_8() {
        let mock = start_catalog_mock("host-a", "tok-1");
        let _session = connect(&pid("host-a", &mock.origin)).expect("online");
        let hs = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "handshake")
            .cloned()
            .expect("handshake");
        for name in SEARCH_GC_METHODS {
            assert_eq!(hs.params["methods"][name]["major"], 1, "{name}");
            assert_eq!(hs.params["methods"][name]["minor"], 9, "{name}");
        }
        assert_eq!(hs.params["methods"]["search.query"]["minor"], 9);
        assert_eq!(hs.params["methods"]["worktree.gc"]["minor"], 9);
        assert_eq!(hs.params["methods"]["sync.export"]["minor"], 8);
        assert_eq!(hs.params["methods"]["sync.import"]["minor"], 8);
        assert_eq!(hs.params["client"], "gui");
    }

    #[test]
    fn search_query_sends_q_and_optional_kinds() {
        let mock = start_search_gc_rpc_mock();
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        assert!(session.search_accepted());
        let items = session.search_query("auth", None).expect("search");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, "task");
        assert_eq!(items[0].id, "task-1");
        assert_eq!(items[0].title, "Auth");
        assert_eq!(items[0].hint, "open");
        let items = session
            .search_query("auth", Some(&["task", "artifact"]))
            .expect("kinds");
        assert_eq!(items.len(), 1);
        let empty = session.search_query("   ", None).expect("empty");
        assert!(empty.is_empty());
        let hits = mock.hits.lock().unwrap().clone();
        let queries: Vec<Value> = hits
            .iter()
            .filter(|h| h.method == "search.query")
            .map(|h| h.params.clone())
            .collect();
        assert_eq!(queries.len(), 2);
        assert_eq!(queries[0]["q"], "auth");
        assert!(queries[0].get("kinds").is_none());
        assert_eq!(queries[1]["q"], "auth");
        assert_eq!(queries[1]["kinds"], json!(["task", "artifact"]));
        assert!(queries.iter().all(|p| p.get("prefix").is_none()));
    }

    #[test]
    fn worktree_gc_sends_dry_run_without_prefix() {
        let mock = start_search_gc_rpc_mock();
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        assert!(session.worktree_gc_accepted());
        let ok = session.worktree_gc(false).expect("gc");
        assert!(!ok.dry_run);
        let hit = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "worktree.gc")
            .cloned()
            .expect("worktree.gc");
        assert_eq!(hit.params, json!({ "dryRun": false }));
        assert!(hit.params.get("prefix").is_none());
        assert!(hit.params.get("branchPrefix").is_none());
        assert!(hit.params.get("branch_prefix").is_none());
    }

    fn account_steer_accepted_map() -> serde_json::Map<String, Value> {
        let mut accepted = serde_json::Map::new();
        for name in ACCOUNT_STEER_METHODS {
            accepted.insert(name.to_string(), json!({"major": 1, "minor": 9}));
        }
        for name in MODEL_METHODS {
            accepted.insert(name.to_string(), json!({"major": 1, "minor": 6}));
        }
        accepted
    }

    fn start_account_steer_rpc_mock() -> CatalogMock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let hits_t = hits.clone();
        thread::spawn(move || {
            for stream in listener.incoming().take(32) {
                let Ok(mut stream) = stream else { break };
                let (headers, body) = read_http_request(&mut stream);
                let has_session = headers.to_ascii_lowercase().contains("x-rt-session:");
                let parsed: Value = serde_json::from_slice(&body).unwrap_or(json!({}));
                let method = if headers.starts_with("GET /health") {
                    "GET /health".to_string()
                } else {
                    parsed
                        .get("method")
                        .and_then(|v| v.as_str())
                        .unwrap_or("other")
                        .to_string()
                };
                let params = parsed.get("params").cloned().unwrap_or(json!({}));
                hits_t.lock().unwrap().push(RpcHit {
                    method: method.clone(),
                    params: params.clone(),
                    has_session,
                });
                let body = match method.as_str() {
                    "GET /health" => json!({"ok": true, "hostId": "host-a"}).to_string(),
                    "handshake" => json!({
                        "id": "echo",
                        "ok": {
                            "hostId": "host-a",
                            "hostVersion": "0.1.0",
                            "sessionToken": "tok-1",
                            "accepted": account_steer_accepted_map(),
                            "rejected": {}
                        }
                    })
                    .to_string(),
                    "host.ping" => json!({
                        "id": "echo",
                        "ok": { "hostId": "host-a", "now": "2026-08-19T12:00:00Z" }
                    })
                    .to_string(),
                    "account.list" => json!({
                        "id": "echo",
                        "ok": {
                            "items": [
                                {
                                    "id": "acc-1",
                                    "label": "work",
                                    "provider": "cli.claude",
                                    "token": "SECRET"
                                },
                                { "accountId": "acc-2", "label": "home" }
                            ]
                        }
                    })
                    .to_string(),
                    "agent.create" => {
                        let provider = params
                            .get("provider")
                            .and_then(|v| v.as_str())
                            .unwrap_or("cli.claude");
                        json!({
                            "id": "echo",
                            "ok": {
                                "id": "ag-new",
                                "taskId": params.get("taskId").cloned().unwrap_or(json!("task-1")),
                                "hostId": "host-a",
                                "parentId": null,
                                "interface": "chat",
                                "provider": provider,
                                "status": "idle",
                                "runLocation": "local",
                                "createdAt": "2026-08-19T12:00:00Z"
                            }
                        })
                        .to_string()
                    }
                    "agent.switch" => json!({
                        "id": "echo",
                        "ok": sample_switched_agent("ag-1", "cli.claude")
                    })
                    .to_string(),
                    "agent.steer" => json!({
                        "id": "echo",
                        "ok": { "accepted": true }
                    })
                    .to_string(),
                    _ => json!({
                        "id": "echo",
                        "error": { "code": "unsupported_method", "message": "no" }
                    })
                    .to_string(),
                };
                write_http_json(&mut stream, &body);
            }
        });
        CatalogMock {
            origin: format!("http://{addr}"),
            hits,
        }
    }

    #[test]
    fn handshake_advertises_account_steer_1_9_and_keeps_1_8() {
        let mock = start_catalog_mock("host-a", "tok-1");
        let _session = connect(&pid("host-a", &mock.origin)).expect("online");
        let hs = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "handshake")
            .cloned()
            .expect("handshake");
        for name in ACCOUNT_STEER_METHODS {
            assert_eq!(hs.params["methods"][name]["major"], 1, "{name}");
            assert_eq!(hs.params["methods"][name]["minor"], 9, "{name}");
        }
        assert_eq!(hs.params["methods"]["account.list"]["minor"], 9);
        assert_eq!(hs.params["methods"]["agent.steer"]["minor"], 9);
        assert_eq!(hs.params["methods"]["search.query"]["minor"], 9);
        assert_eq!(hs.params["methods"]["sync.export"]["minor"], 8);
        assert_eq!(hs.params["methods"]["sync.import"]["minor"], 8);
        assert_eq!(hs.params["methods"]["agent.switch"]["minor"], 6);
        assert_eq!(hs.params["methods"]["agent.create"]["minor"], 0);
        assert_eq!(hs.params["client"], "gui");
    }

    #[test]
    fn account_list_returns_labels_without_secrets() {
        let mock = start_account_steer_rpc_mock();
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        assert!(session.accounts_accepted());
        let items = session.account_list().expect("list");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, "acc-1");
        assert_eq!(items[0].label, "work");
        assert_eq!(items[0].provider.as_deref(), Some("cli.claude"));
        assert_eq!(items[1].id, "acc-2");
        let hit = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "account.list")
            .cloned()
            .expect("account.list");
        assert!(hit.has_session);
        assert!(hit.params.get("token").is_none());
    }

    #[test]
    fn agent_create_and_switch_include_account_id_only_when_set() {
        let mock = start_account_steer_rpc_mock();
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        session
            .agent_create("task-1", "cli.claude", None)
            .expect("create none");
        session
            .agent_create("task-1", "cli.claude", Some("acc-1"))
            .expect("create acc");
        session
            .agent_switch(
                "ag-1",
                AgentSwitchParams {
                    provider: Some("cli.claude"),
                    ..AgentSwitchParams::default()
                },
            )
            .expect("switch none");
        session
            .agent_switch(
                "ag-1",
                AgentSwitchParams {
                    provider: Some("cli.claude"),
                    account_id: Some("acc-1"),
                    ..AgentSwitchParams::default()
                },
            )
            .expect("switch acc");
        let hits = mock.hits.lock().unwrap().clone();
        let creates: Vec<_> = hits
            .iter()
            .filter(|h| h.method == "agent.create")
            .map(|h| h.params.clone())
            .collect();
        assert_eq!(creates.len(), 2);
        assert!(creates[0].get("accountId").is_none());
        assert_eq!(creates[1]["accountId"], "acc-1");
        let switches: Vec<_> = hits
            .iter()
            .filter(|h| h.method == "agent.switch")
            .map(|h| h.params.clone())
            .collect();
        assert_eq!(switches.len(), 2);
        assert!(switches[0].get("accountId").is_none());
        assert_eq!(switches[1]["accountId"], "acc-1");
    }

    #[test]
    fn agent_steer_sends_agent_id_and_content() {
        let mock = start_account_steer_rpc_mock();
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        assert!(session.steer_accepted());
        session.agent_steer("ag-1", "  nudge  ").expect("steer");
        let empty = session.agent_steer("ag-1", "   ").expect("empty");
        assert_eq!(empty, json!({}));
        let hits = mock.hits.lock().unwrap().clone();
        let steers: Vec<_> = hits
            .iter()
            .filter(|h| h.method == "agent.steer")
            .map(|h| h.params.clone())
            .collect();
        assert_eq!(steers.len(), 1);
        assert_eq!(steers[0]["agentId"], "ag-1");
        assert_eq!(steers[0]["content"], "nudge");
        assert!(hits.iter().all(|h| h.method != "agent.send"));
    }

    fn pr_accepted_map() -> serde_json::Map<String, Value> {
        let mut accepted = serde_json::Map::new();
        for name in PR_METHODS {
            accepted.insert(name.to_string(), json!({"major": 1, "minor": 9}));
        }
        accepted
    }

    fn start_pr_rpc_mock() -> CatalogMock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let hits_t = hits.clone();
        thread::spawn(move || {
            for stream in listener.incoming().take(24) {
                let Ok(mut stream) = stream else { break };
                let (headers, body) = read_http_request(&mut stream);
                let has_session = headers.to_ascii_lowercase().contains("x-rt-session:");
                let parsed: Value = serde_json::from_slice(&body).unwrap_or(json!({}));
                let method = if headers.starts_with("GET /health") {
                    "GET /health".to_string()
                } else {
                    parsed
                        .get("method")
                        .and_then(|v| v.as_str())
                        .unwrap_or("other")
                        .to_string()
                };
                let params = parsed.get("params").cloned().unwrap_or(json!({}));
                hits_t.lock().unwrap().push(RpcHit {
                    method: method.clone(),
                    params: params.clone(),
                    has_session,
                });
                let body = match method.as_str() {
                    "GET /health" => json!({"ok": true, "hostId": "host-a"}).to_string(),
                    "handshake" => json!({
                        "id": "echo",
                        "ok": {
                            "hostId": "host-a",
                            "hostVersion": "0.1.0",
                            "sessionToken": "tok-1",
                            "accepted": pr_accepted_map(),
                            "rejected": {}
                        }
                    })
                    .to_string(),
                    "host.ping" => json!({
                        "id": "echo",
                        "ok": { "hostId": "host-a", "now": "2026-08-19T12:00:00Z" }
                    })
                    .to_string(),
                    "pr.get" => json!({
                        "id": "echo",
                        "ok": {
                            "title": "Panel",
                            "number": params.get("number").cloned().unwrap_or(json!(0)),
                            "checks": [{ "name": "ci", "status": "success" }],
                            "commits": [{ "sha": "abc", "message": "feat" }],
                            "files": [{ "path": "a.rs" }],
                            "localDiff": "diff --git"
                        }
                    })
                    .to_string(),
                    _ => json!({
                        "id": "echo",
                        "error": { "code": "unsupported_method", "message": "no" }
                    })
                    .to_string(),
                };
                write_http_json(&mut stream, &body);
            }
        });
        CatalogMock {
            origin: format!("http://{addr}"),
            hits,
        }
    }

    #[test]
    fn handshake_advertises_pr_get_1_9_and_keeps_1_8() {
        let mock = start_catalog_mock("host-a", "tok-1");
        let _session = connect(&pid("host-a", &mock.origin)).expect("online");
        let hs = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "handshake")
            .cloned()
            .expect("handshake");
        assert_eq!(hs.params["methods"]["pr.get"]["major"], 1);
        assert_eq!(hs.params["methods"]["pr.get"]["minor"], 9);
        assert_eq!(hs.params["methods"]["search.query"]["minor"], 9);
        assert_eq!(hs.params["methods"]["agent.steer"]["minor"], 9);
        assert_eq!(hs.params["methods"]["sync.export"]["minor"], 8);
        assert_eq!(hs.params["methods"]["sync.import"]["minor"], 8);
        assert_eq!(hs.params["methods"]["agent.switch"]["minor"], 6);
        assert_eq!(hs.params["client"], "gui");
    }

    #[test]
    fn pr_get_sends_workspace_and_number_or_url() {
        let mock = start_pr_rpc_mock();
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        assert!(session.pr_accepted());
        let view = session.pr_get("ws-1", "91", "").expect("number");
        assert_eq!(view.title.as_deref(), Some("Panel"));
        assert_eq!(view.checks, vec!["ci · success"]);
        let _ = session
            .pr_get("ws-1", "", "https://example.com/pr/91")
            .expect("url");
        let empty = session.pr_get("ws-1", "  ", "  ").expect("empty");
        assert!(empty.checks.is_empty());
        let none_ws = session.pr_get("  ", "91", "").expect("no ws");
        assert!(none_ws.checks.is_empty());
        let hits = mock.hits.lock().unwrap().clone();
        let gets: Vec<Value> = hits
            .iter()
            .filter(|h| h.method == "pr.get")
            .map(|h| h.params.clone())
            .collect();
        assert_eq!(gets.len(), 2);
        assert_eq!(gets[0]["workspaceId"], "ws-1");
        assert_eq!(gets[0]["number"], 91);
        assert!(gets[0].get("url").is_none());
        assert_eq!(gets[1]["workspaceId"], "ws-1");
        assert_eq!(gets[1]["url"], "https://example.com/pr/91");
        assert!(gets[1].get("number").is_none());
        assert!(gets.iter().all(|p| p.get("token").is_none()));
        assert!(hits
            .iter()
            .filter(|h| h.method == "pr.get")
            .all(|h| h.has_session));
    }

    fn start_metrics_http_mock(body: &str, status: u16) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let hits_t = hits.clone();
        let body = body.to_string();
        thread::spawn(move || {
            for stream in listener.incoming().take(8) {
                let Ok(mut stream) = stream else { break };
                let (headers, _) = read_http_request(&mut stream);
                let first = headers.lines().next().unwrap_or("").to_string();
                hits_t.lock().unwrap().push(first);
                let reason = if status == 200 { "OK" } else { "ERR" };
                let resp = format!(
                    "HTTP/1.1 {status} {reason}\r\nContent-Type: text/plain; version=0.0.4\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = std::io::Write::write_all(&mut stream, resp.as_bytes());
            }
        });
        (format!("http://{addr}"), hits)
    }

    fn start_stash_rpc_mock() -> CatalogMock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let hits_t = hits.clone();
        thread::spawn(move || {
            for stream in listener.incoming().take(32) {
                let Ok(mut stream) = stream else { break };
                let (headers, body) = read_http_request(&mut stream);
                let has_session = headers.to_ascii_lowercase().contains("x-rt-session:");
                let (method, params) = if headers.starts_with("GET /health") {
                    ("GET /health".to_string(), json!({}))
                } else {
                    let parsed: Value = serde_json::from_slice(&body).unwrap_or(json!({}));
                    (
                        parsed
                            .get("method")
                            .and_then(|v| v.as_str())
                            .unwrap_or("other")
                            .to_string(),
                        parsed.get("params").cloned().unwrap_or(json!({})),
                    )
                };
                hits_t.lock().unwrap().push(RpcHit {
                    method: method.clone(),
                    params: params.clone(),
                    has_session,
                });
                let body = match method.as_str() {
                    "GET /health" => json!({"ok": true, "hostId": "host-a"}).to_string(),
                    "handshake" => {
                        let mut accepted = serde_json::Map::new();
                        for name in STASH_METHODS {
                            accepted.insert(name.to_string(), json!({"major": 1, "minor": 9}));
                        }
                        json!({
                            "id": "echo",
                            "ok": {
                                "hostId": "host-a",
                                "hostVersion": "0.1.0",
                                "sessionToken": "tok-1",
                                "accepted": accepted,
                                "rejected": {}
                            }
                        })
                        .to_string()
                    }
                    "host.ping" => json!({
                        "id": "echo",
                        "ok": { "hostId": "host-a", "now": "2026-08-19T12:00:00Z" }
                    })
                    .to_string(),
                    "stash.list" => json!({
                        "id": "echo",
                        "ok": { "items": [{ "id": "s1", "body": "draft" }] }
                    })
                    .to_string(),
                    "stash.add" => json!({
                        "id": "echo",
                        "ok": {
                            "id": "s-new",
                            "body": params.get("body").cloned().unwrap_or(json!(""))
                        }
                    })
                    .to_string(),
                    "stash.delete" => json!({
                        "id": "echo",
                        "ok": { "id": params.get("id").cloned().unwrap_or(json!("")) }
                    })
                    .to_string(),
                    _ => json!({
                        "id": "echo",
                        "error": { "code": "unsupported_method", "message": "no" }
                    })
                    .to_string(),
                };
                write_http_json(&mut stream, &body);
            }
        });
        CatalogMock {
            origin: format!("http://{addr}"),
            hits,
        }
    }

    #[test]
    fn handshake_advertises_stash_1_9_and_keeps_1_8() {
        let mock = start_catalog_mock("host-a", "tok-1");
        let _session = connect(&pid("host-a", &mock.origin)).expect("online");
        let hs = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "handshake")
            .cloned()
            .expect("handshake");
        for name in STASH_METHODS {
            assert_eq!(hs.params["methods"][name]["major"], 1, "{name}");
            assert_eq!(hs.params["methods"][name]["minor"], 9, "{name}");
        }
        assert_eq!(hs.params["methods"]["stash.list"]["minor"], 9);
        assert_eq!(hs.params["methods"]["stash.add"]["minor"], 9);
        assert_eq!(hs.params["methods"]["stash.delete"]["minor"], 9);
        assert_eq!(hs.params["methods"]["sync.export"]["minor"], 8);
        assert_eq!(hs.params["methods"]["sync.import"]["minor"], 8);
        assert_eq!(hs.params["methods"]["search.query"]["minor"], 9);
        assert_eq!(hs.params["methods"]["pr.get"]["minor"], 9);
        assert!(hs.params["methods"].get("metrics").is_none());
        assert_eq!(hs.params["client"], "gui");
    }

    #[test]
    fn stash_add_list_delete_shapes() {
        let mock = start_stash_rpc_mock();
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        assert!(session.stash_accepted());
        let items = session.stash_list().expect("list");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "s1");
        assert_eq!(items[0].body, "draft");
        let added = session.stash_add("hello", None).expect("add");
        assert_eq!(added.id, "s-new");
        assert_eq!(added.body, "hello");
        session.stash_delete("s1").expect("delete");
        let empty = session.stash_add("   ", None).expect("empty");
        assert!(empty.id.is_empty());
        let hits = mock.hits.lock().unwrap().clone();
        let adds: Vec<Value> = hits
            .iter()
            .filter(|h| h.method == "stash.add")
            .map(|h| h.params.clone())
            .collect();
        assert_eq!(adds.len(), 1);
        assert_eq!(adds[0]["body"], "hello");
        assert!(adds[0].get("imagePath").is_none());
        let deletes: Vec<Value> = hits
            .iter()
            .filter(|h| h.method == "stash.delete")
            .map(|h| h.params.clone())
            .collect();
        assert_eq!(deletes, vec![json!({ "id": "s1" })]);
        assert!(hits
            .iter()
            .filter(|h| h.method.starts_with("stash."))
            .all(|h| h.has_session));
    }

    fn start_sync_peer_rpc_mock() -> CatalogMock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let hits_t = hits.clone();
        thread::spawn(move || {
            for stream in listener.incoming().take(32) {
                let Ok(mut stream) = stream else { break };
                let (headers, body) = read_http_request(&mut stream);
                let has_session = headers.to_ascii_lowercase().contains("x-rt-session:");
                let (method, params) = if headers.starts_with("GET /health") {
                    ("GET /health".to_string(), json!({}))
                } else {
                    let parsed: Value = serde_json::from_slice(&body).unwrap_or(json!({}));
                    (
                        parsed
                            .get("method")
                            .and_then(|v| v.as_str())
                            .unwrap_or("other")
                            .to_string(),
                        parsed.get("params").cloned().unwrap_or(json!({})),
                    )
                };
                hits_t.lock().unwrap().push(RpcHit {
                    method: method.clone(),
                    params: params.clone(),
                    has_session,
                });
                let body = match method.as_str() {
                    "GET /health" => json!({"ok": true, "hostId": "host-a"}).to_string(),
                    "handshake" => {
                        let mut accepted = serde_json::Map::new();
                        for name in SYNC_METHODS {
                            accepted.insert(name.to_string(), json!({"major": 1, "minor": 8}));
                        }
                        for name in SYNC_PEER_METHODS {
                            accepted.insert(name.to_string(), json!({"major": 1, "minor": 9}));
                        }
                        json!({
                            "id": "echo",
                            "ok": {
                                "hostId": "host-a",
                                "hostVersion": "0.1.0",
                                "sessionToken": "tok-1",
                                "accepted": accepted,
                                "rejected": {}
                            }
                        })
                        .to_string()
                    }
                    "host.ping" => json!({
                        "id": "echo",
                        "ok": { "hostId": "host-a", "now": "2026-08-19T12:00:00Z" }
                    })
                    .to_string(),
                    "sync.push" => json!({
                        "id": "echo",
                        "ok": { "ok": true }
                    })
                    .to_string(),
                    "sync.pull" => json!({
                        "id": "echo",
                        "ok": { "tasks": 1, "agents": 0 }
                    })
                    .to_string(),
                    _ => json!({
                        "id": "echo",
                        "error": { "code": "unsupported_method", "message": "no" }
                    })
                    .to_string(),
                };
                write_http_json(&mut stream, &body);
            }
        });
        CatalogMock {
            origin: format!("http://{addr}"),
            hits,
        }
    }

    fn start_preset_crud_rpc_mock() -> CatalogMock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let hits_t = hits.clone();
        thread::spawn(move || {
            for stream in listener.incoming().take(32) {
                let Ok(mut stream) = stream else { break };
                let (headers, body) = read_http_request(&mut stream);
                let has_session = headers.to_ascii_lowercase().contains("x-rt-session:");
                let (method, params) = if headers.starts_with("GET /health") {
                    ("GET /health".to_string(), json!({}))
                } else {
                    let parsed: Value = serde_json::from_slice(&body).unwrap_or(json!({}));
                    (
                        parsed
                            .get("method")
                            .and_then(|v| v.as_str())
                            .unwrap_or("other")
                            .to_string(),
                        parsed.get("params").cloned().unwrap_or(json!({})),
                    )
                };
                hits_t.lock().unwrap().push(RpcHit {
                    method: method.clone(),
                    params: params.clone(),
                    has_session,
                });
                let body = match method.as_str() {
                    "GET /health" => json!({"ok": true, "hostId": "host-a"}).to_string(),
                    "handshake" => {
                        let mut accepted = serde_json::Map::new();
                        for name in WORKSPACE_METHODS {
                            accepted.insert(name.to_string(), json!({"major": 1, "minor": 7}));
                        }
                        for name in PRESET_CRUD_METHODS {
                            accepted.insert(name.to_string(), json!({"major": 1, "minor": 9}));
                        }
                        json!({
                            "id": "echo",
                            "ok": {
                                "hostId": "host-a",
                                "hostVersion": "0.1.0",
                                "sessionToken": "tok-1",
                                "accepted": accepted,
                                "rejected": {}
                            }
                        })
                        .to_string()
                    }
                    "host.ping" => json!({
                        "id": "echo",
                        "ok": { "hostId": "host-a", "now": "2026-08-19T12:00:00Z" }
                    })
                    .to_string(),
                    "preset.list" => json!({
                        "id": "echo",
                        "ok": {
                            "items": [
                                { "id": "planning", "title": "Planning", "defaultRole": "planner" },
                                {
                                    "id": "p-1",
                                    "name": "Mine",
                                    "defaultRole": "coder",
                                    "titleHint": "hint",
                                    "prompt": "do"
                                }
                            ]
                        }
                    })
                    .to_string(),
                    "preset.create" => json!({
                        "id": "echo",
                        "ok": {
                            "id": "p-new",
                            "name": params.get("name").cloned().unwrap_or(json!("")),
                            "defaultRole": params.get("defaultRole").cloned().unwrap_or(json!("coder")),
                            "titleHint": params.get("titleHint").cloned().unwrap_or(json!("")),
                            "prompt": params.get("prompt").cloned().unwrap_or(json!(""))
                        }
                    })
                    .to_string(),
                    "preset.update" => json!({
                        "id": "echo",
                        "ok": {
                            "id": params.get("id").cloned().unwrap_or(json!("")),
                            "name": params.get("name").cloned().unwrap_or(json!("")),
                            "defaultRole": params.get("defaultRole").cloned().unwrap_or(json!("coder"))
                        }
                    })
                    .to_string(),
                    "preset.delete" => json!({
                        "id": "echo",
                        "ok": { "id": params.get("id").cloned().unwrap_or(json!("")) }
                    })
                    .to_string(),
                    _ => json!({
                        "id": "echo",
                        "error": { "code": "unsupported_method", "message": "no" }
                    })
                    .to_string(),
                };
                write_http_json(&mut stream, &body);
            }
        });
        CatalogMock {
            origin: format!("http://{addr}"),
            hits,
        }
    }

    #[test]
    fn handshake_advertises_sync_peer_1_9_and_keeps_1_8() {
        let mock = start_catalog_mock("host-a", "tok-1");
        let _session = connect(&pid("host-a", &mock.origin)).expect("online");
        let hs = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "handshake")
            .cloned()
            .expect("handshake");
        for name in SYNC_METHODS {
            assert_eq!(hs.params["methods"][name]["major"], 1, "{name}");
            assert_eq!(hs.params["methods"][name]["minor"], 8, "{name}");
        }
        for name in SYNC_PEER_METHODS {
            assert_eq!(hs.params["methods"][name]["major"], 1, "{name}");
            assert_eq!(hs.params["methods"][name]["minor"], 9, "{name}");
        }
        assert_eq!(hs.params["methods"]["sync.export"]["minor"], 8);
        assert_eq!(hs.params["methods"]["sync.import"]["minor"], 8);
        assert_eq!(hs.params["methods"]["sync.push"]["minor"], 9);
        assert_eq!(hs.params["methods"]["sync.pull"]["minor"], 9);
        assert_eq!(hs.params["client"], "gui");
    }

    #[test]
    fn handshake_advertises_preset_crud_1_9_and_keeps_1_7_list() {
        let mock = start_catalog_mock("host-a", "tok-1");
        let _session = connect(&pid("host-a", &mock.origin)).expect("online");
        let hs = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "handshake")
            .cloned()
            .expect("handshake");
        assert_eq!(hs.params["methods"]["preset.list"]["minor"], 7);
        for name in PRESET_CRUD_METHODS {
            assert_eq!(hs.params["methods"][name]["major"], 1, "{name}");
            assert_eq!(hs.params["methods"][name]["minor"], 9, "{name}");
        }
        assert_eq!(hs.params["methods"]["preset.create"]["minor"], 9);
        assert_eq!(hs.params["methods"]["preset.update"]["minor"], 9);
        assert_eq!(hs.params["methods"]["preset.delete"]["minor"], 9);
        assert_eq!(hs.params["methods"]["workspace.guides.get"]["minor"], 7);
        assert_eq!(hs.params["client"], "gui");
    }

    #[test]
    fn sync_push_sends_peer_url_only() {
        let mock = start_sync_peer_rpc_mock();
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        assert!(session.sync_accepted());
        assert!(session.sync_peer_accepted());
        session
            .sync_push("  http://127.0.0.1:7420  ")
            .expect("push");
        session.sync_push("   ").expect("empty");
        let hits = mock.hits.lock().unwrap().clone();
        let pushes: Vec<Value> = hits
            .iter()
            .filter(|h| h.method == "sync.push")
            .map(|h| h.params.clone())
            .collect();
        assert_eq!(pushes.len(), 1);
        assert_eq!(pushes[0], json!({ "peerUrl": "http://127.0.0.1:7420" }));
        assert!(pushes[0].get("secret").is_none());
        assert!(pushes[0].get("token").is_none());
        assert!(hits
            .iter()
            .filter(|h| h.method == "sync.push")
            .all(|h| h.has_session));
    }

    #[test]
    fn sync_pull_sends_peer_url_and_workspace() {
        let mock = start_sync_peer_rpc_mock();
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        session
            .sync_pull("http://127.0.0.1:9", "ws-1")
            .expect("pull");
        session.sync_pull("", "ws-1").expect("empty url");
        session
            .sync_pull("http://127.0.0.1:9", "")
            .expect("empty ws");
        let hits = mock.hits.lock().unwrap().clone();
        let pulls: Vec<Value> = hits
            .iter()
            .filter(|h| h.method == "sync.pull")
            .map(|h| h.params.clone())
            .collect();
        assert_eq!(pulls.len(), 1);
        assert_eq!(
            pulls[0],
            json!({ "peerUrl": "http://127.0.0.1:9", "workspaceId": "ws-1" })
        );
        assert!(pulls[0].get("secret").is_none());
    }

    #[test]
    fn preset_create_update_delete_shapes_and_list_1_7() {
        let mock = start_preset_crud_rpc_mock();
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        assert!(session.workspace_accepted());
        assert!(session.preset_crud_accepted());
        let listed = session.preset_list().expect("list");
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].id, "planning");
        assert_eq!(listed[1].id, "p-1");
        assert_eq!(listed[1].name, "Mine");
        let created = session
            .preset_create("Mine", "planner", "hint", "do")
            .expect("create");
        assert_eq!(created.id, "p-new");
        assert_eq!(created.name, "Mine");
        session
            .preset_update("p-1", "Mine2", "reviewer", "", "")
            .expect("update");
        session.preset_delete("p-1").expect("delete");
        session.preset_delete("planning").expect("builtin skipped");
        session.preset_create("  ", "coder", "", "").expect("empty");
        let hits = mock.hits.lock().unwrap().clone();
        let creates: Vec<Value> = hits
            .iter()
            .filter(|h| h.method == "preset.create")
            .map(|h| h.params.clone())
            .collect();
        assert_eq!(creates.len(), 1);
        assert_eq!(creates[0]["name"], "Mine");
        assert_eq!(creates[0]["defaultRole"], "planner");
        assert_eq!(creates[0]["titleHint"], "hint");
        assert_eq!(creates[0]["prompt"], "do");
        assert!(creates[0].get("secret").is_none());
        let updates: Vec<Value> = hits
            .iter()
            .filter(|h| h.method == "preset.update")
            .map(|h| h.params.clone())
            .collect();
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0]["id"], "p-1");
        assert_eq!(updates[0]["name"], "Mine2");
        let deletes: Vec<Value> = hits
            .iter()
            .filter(|h| h.method == "preset.delete")
            .map(|h| h.params.clone())
            .collect();
        assert_eq!(deletes, vec![json!({ "id": "p-1" })]);
        assert!(hits.iter().any(|h| h.method == "preset.list"));
    }

    #[test]
    fn metrics_get_uses_metrics_path_and_failure_does_not_panic() {
        let prom = "# TYPE rusttraycer_agents gauge\nrusttraycer_agents{status=\"idle\"} 2\n";
        let (origin, hits) = start_metrics_http_mock(prom, 200);
        let chip = fetch_metrics(&origin).expect("metrics");
        assert_eq!(chip.agents, Some(2));
        let first = hits.lock().unwrap()[0].clone();
        assert!(
            first.starts_with("GET /metrics"),
            "expected GET /metrics, got {first}"
        );
        let err = fetch_metrics("http://127.0.0.1:1");
        assert!(err.is_err());
        match err {
            Err(ConnectError::Transport(_)) | Err(ConnectError::Health(_)) => {}
            other => panic!("expected transport, got {other:?}"),
        }
    }
}
