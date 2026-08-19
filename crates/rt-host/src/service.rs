//! HostService: domain operations + agent.send orchestration.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rt_protocol::{
    AccountListOk, AgentSteerOk, AgentSwitchParams, ApprovalDecision, ApprovalRespondOk,
    ApprovalRespondParams, CancelOk, HarnessCaps as HarnessCapsWire, PolicyGetParams, PolicyMode,
    PolicyScope, PolicySetParams, PolicySource, PolicyView, PrefsGetOk, PrefsItem,
    PresetCreateParams, PresetDeleteOk, PresetItem, PresetListOk, PresetUpdateParams, Profile,
    ProfileCreateParams, ProfileDeleteParams, ProfileGetParams, ProfileListOk, ProfileUpdateParams,
    ProviderAccount, SearchItem, SearchKind, SearchQueryOk, SearchQueryParams, SettingsGuide,
    StashDeleteOk, StashItem, StashListOk, WorkspaceGuidesOk,
};
use rt_runtime::{AgentBackend, TurnRequest, WireMessage, WireRole};
use rt_storage::{
    Agent, AgentModelSpec, AgentStatus, HarnessId, Message, MessageRole, ModelProfile, Store, Task,
    TaskFilter, TaskStatus, Workspace,
};
use serde::Serialize;

use crate::bind;
use crate::files;
use crate::guides;
use crate::handshake::{self, HandshakeParams, HandshakeResult};
use crate::mux::{Mux, MuxIoError, PtyKind};
use crate::pty::{self, SpawnSpec};
use crate::rpc::WsEvent;
use crate::supervisor::{self, Inflight};
use crate::{HostError, Result};

const MAX_CONTENT: usize = 1024 * 1024;
const MAX_TITLE_CHARS: usize = 200;

fn stash_item_from_row(row: rt_storage::PromptStash) -> StashItem {
    StashItem {
        id: row.id,
        body: row.body,
        image_path: row.image_path,
        created_at: row.created_at,
    }
}

fn preset_item_from_user(row: rt_storage::UserPreset) -> PresetItem {
    PresetItem {
        id: row.id,
        title: row.name.clone(),
        default_role: row.default_role,
        name: Some(row.name),
        title_hint: row.title_hint,
        prompt: row.prompt,
    }
}

fn is_builtin_preset_id(id: &str) -> bool {
    guides::PRESETS.iter().any(|p| p.id == id)
}

fn builtin_preset_conflict(name: &str) -> bool {
    guides::PRESETS
        .iter()
        .any(|p| p.id == name || p.title == name)
}

#[derive(Clone)]
struct Session {
    accepted: HashSet<String>,
}

#[derive(Clone, Debug)]
enum EditOp {
    Write,
    Patch,
}

#[derive(Clone, Debug)]
enum PendingKind {
    Exec,
    EditWrite {
        workspace_id: String,
        worktree_id: Option<String>,
        path: String,
        content: String,
    },
    EditPatch {
        workspace_id: String,
        worktree_id: Option<String>,
        patch: String,
    },
    PtyOpen {
        cols: u16,
        rows: u16,
    },
}

#[derive(Clone, Debug)]
struct PendingApproval {
    approval_id: String,
    agent_id: String,
    task_id: String,
    kind: String,
    summary: String,
    payload: PendingKind,
}

#[derive(Default)]
struct LadderState {
    pending_by_id: HashMap<String, PendingApproval>,
    pending_by_agent: HashMap<String, String>,
    applied: HashSet<String>,
}

struct ReservedTurn {
    agent_id: String,
    task_id: String,
    workspace_path: std::path::PathBuf,
    backend: Arc<dyn AgentBackend>,
    gen: u64,
    user: Option<Message>,
    attached: Option<std::path::PathBuf>,
}

pub struct SendOutcome {
    pub user: rt_storage::Message,
    pub approval_id: Option<String>,
}

#[derive(Clone)]
pub struct HostService {
    pub store: Store,
    backends: HashMap<String, Arc<dyn AgentBackend>>,
    inflight: Inflight,
    turn_gate: Arc<Mutex<()>>,
    events: tokio::sync::broadcast::Sender<WsEvent>,
    sessions: Arc<Mutex<HashMap<String, Session>>>,
    ladder: Arc<Mutex<LadderState>>,
    host_id: String,
    pub(crate) data_dir: std::path::PathBuf,
    db_path: std::path::PathBuf,
    log_path: std::path::PathBuf,
    rpc_url: String,
    pid: u32,
    turn_timeout: Duration,
    mux: std::sync::Arc<Mux>,
    launch_args: Arc<Mutex<HashMap<String, Vec<String>>>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PingResult {
    pub host_id: String,
    pub now: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    pub id: String,
    pub available: bool,
    pub detail: String,
    pub caps: HarnessCapsWire,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorResult {
    pub host_id: String,
    pub pid: u32,
    pub rpc_url: String,
    pub db_ok: bool,
    pub data_dir: String,
    pub db_path: String,
    pub log_path: String,
    pub providers: Vec<ProviderInfo>,
    pub workspace_count: i64,
    pub task_count: i64,
    pub agent_count: i64,
    pub yolo: bool,
}

pub struct AgentCreateArgs<'a> {
    pub task_id: Option<&'a str>,
    pub workspace_id: Option<&'a str>,
    pub provider: Option<&'a str>,
    pub interface: Option<&'a str>,
    pub launch_args: Option<Vec<String>>,
    pub parent_id: Option<&'a str>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub fast: Option<bool>,
    pub role: Option<&'a str>,
    pub account_id: Option<&'a str>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentView {
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub task_id: String,
    pub host_id: String,
    pub parent_id: Option<String>,
    pub interface: String,
    pub provider: HarnessId,
    pub status: AgentStatus,
    pub run_location: String,
    pub created_at: String,
    pub last_message_at: Option<String>,
    pub yolo: bool,
    pub provider_session_id: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub fast: bool,
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
}

impl From<Agent> for AgentView {
    fn from(a: Agent) -> Self {
        Self {
            id: a.id,
            task_id: a.task_id,
            host_id: a.host_id,
            parent_id: a.parent_id,
            interface: a.interface,
            provider: a.provider,
            status: a.status,
            run_location: a.run_location,
            created_at: a.created_at,
            last_message_at: None,
            yolo: false,
            provider_session_id: a.provider_session_id,
            model: a.model,
            effort: a.effort,
            fast: a.fast,
            role: a.role,
            workspace_id: a.workspace_id,
            account_id: a.account_id,
        }
    }
}

fn provider_supports_steer(provider: &str) -> bool {
    matches!(provider, "cli.claude" | "cli.codex")
}

fn harness_caps_wire(provider: &str, caps: rt_runtime::HarnessCaps) -> HarnessCapsWire {
    HarnessCapsWire {
        one_shot: caps.one_shot,
        long_lived: caps.long_lived,
        stream_tokens: caps.stream_tokens,
        tools: caps.tools,
        session_resume: caps.session_resume,
        a2a_inbox: caps.a2a_inbox,
        pty: caps.pty,
        steer: provider_supports_steer(provider),
        needs_api_key: caps.needs_api_key,
        api_key_env: caps.api_key_env.map(str::to_string),
    }
}

fn policy_view_from_row(row: &rt_storage::PolicyRow, source: PolicySource) -> Result<PolicyView> {
    let mode = PolicyMode::parse(&row.mode).ok_or_else(|| {
        HostError::Internal(format!("stored policy mode is invalid: {}", row.mode))
    })?;
    let scope = PolicyScope::parse(&row.scope).ok_or_else(|| {
        HostError::Internal(format!("stored policy scope is invalid: {}", row.scope))
    })?;
    Ok(PolicyView {
        mode,
        scope,
        yolo: row.yolo,
        source,
    })
}

fn check_title(title: &str) -> Result<()> {
    let n = title.chars().count();
    if !(1..=MAX_TITLE_CHARS).contains(&n) {
        return Err(HostError::InvalidParams(
            "title must be 1..200 characters".into(),
        ));
    }
    Ok(())
}

const ALLOWED_HARNESSES: [&str; 3] = ["cli.generic", "cli.claude", "cli.codex"];

fn allowlisted_provider(provider: &str) -> bool {
    ALLOWED_HARNESSES.contains(&provider)
}

fn reject_provider(provider: &str) -> Result<()> {
    if provider == "native" || !allowlisted_provider(provider) {
        return Err(HostError::InvalidParams(format!(
            "provider must be cli.generic|cli.claude|cli.codex, got {provider}"
        )));
    }
    Ok(())
}

fn account_secret_env_name(provider: &str, label: &str) -> String {
    let mut out = String::from("RUSTTRAYCER_");
    for c in provider.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_uppercase());
        } else {
            out.push('_');
        }
    }
    out.push('_');
    for c in label.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_uppercase());
        } else {
            out.push('_');
        }
    }
    out
}

fn account_secret_from_env(provider: &str, label: &str) -> Option<(String, String)> {
    let key = account_secret_env_name(provider, label);
    match std::env::var(&key) {
        Ok(v) if !v.is_empty() => Some((key, v)),
        _ => None,
    }
}

fn check_profile_name(name: &str) -> Result<()> {
    let n = name.chars().count();
    if !(1..=80).contains(&n) {
        return Err(HostError::InvalidParams(
            "profile name must be 1..80 characters".into(),
        ));
    }
    Ok(())
}

fn profile_from_row(row: ModelProfile) -> Profile {
    Profile {
        id: row.id,
        name: row.name,
        provider: row.provider,
        model: row.model,
        effort: row.effort,
        fast: row.fast,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

impl HostService {
    pub fn new(
        store: Store,
        backends: HashMap<String, Arc<dyn AgentBackend>>,
        host_id: String,
        data_dir: std::path::PathBuf,
        rpc_url: String,
        pid: u32,
    ) -> Self {
        let (events, _) = tokio::sync::broadcast::channel(256);
        let db_path = store.path().to_path_buf();
        let log_path = bind::log_path(&data_dir);
        Self {
            store,
            backends,
            inflight: Inflight::new(),
            turn_gate: Arc::new(Mutex::new(())),
            events,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            ladder: Arc::new(Mutex::new(LadderState::default())),
            host_id,
            data_dir,
            db_path,
            log_path,
            rpc_url,
            pid,
            turn_timeout: supervisor::TURN_TIMEOUT,
            mux: std::sync::Arc::new(Mux::new()),
            launch_args: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    #[cfg(test)]
    pub(crate) fn set_turn_timeout(&mut self, timeout: Duration) {
        self.turn_timeout = timeout;
    }

    pub fn host_id(&self) -> &str {
        &self.host_id
    }

    pub fn subscribe_events(&self) -> tokio::sync::broadcast::Receiver<WsEvent> {
        self.events.subscribe()
    }

    pub fn inflight(&self) -> Inflight {
        self.inflight.clone()
    }

    pub fn session_valid(&self, token: &str) -> Result<bool> {
        let g = self
            .sessions
            .lock()
            .map_err(|_| HostError::Internal("session lock poisoned".into()))?;
        Ok(g.contains_key(token))
    }

    /// `Ok(None)` = no session. `Ok(Some(false))` = session exists but method not accepted.
    pub fn session_accepts(&self, token: &str, method: &str) -> Result<Option<bool>> {
        let g = self
            .sessions
            .lock()
            .map_err(|_| HostError::Internal("session lock poisoned".into()))?;
        Ok(g.get(token).map(|s| s.accepted.contains(method)))
    }

    pub fn handshake(&self, params: HandshakeParams) -> Result<HandshakeResult> {
        if params.client != "gui" && params.client != "cli" {
            return Err(HostError::InvalidParams(format!(
                "client must be gui or cli, got {}",
                params.client
            )));
        }
        let (accepted, rejected) = handshake::negotiate(&params.methods);
        let token = uuid::Uuid::now_v7().to_string();
        let accepted_names: HashSet<String> = accepted.keys().cloned().collect();
        self.sessions
            .lock()
            .map_err(|_| HostError::Internal("session lock poisoned".into()))?
            .insert(
                token.clone(),
                Session {
                    accepted: accepted_names,
                },
            );
        Ok(HandshakeResult {
            host_id: self.host_id.clone(),
            host_version: handshake::HOST_VERSION.to_string(),
            session_token: token,
            accepted,
            rejected,
        })
    }

    pub fn ping(&self) -> PingResult {
        PingResult {
            host_id: self.host_id.clone(),
            now: rt_storage::now_rfc3339(),
        }
    }

    pub fn doctor(&self) -> Result<DoctorResult> {
        let counts = self.store.counts()?;
        let db_ok = self.store.integrity_ok().unwrap_or(false);
        let yolo = self.store.policy_any_yolo()?;
        let mut providers: Vec<ProviderInfo> = self
            .backends
            .values()
            .map(|backend| {
                let avail = backend.available();
                ProviderInfo {
                    id: backend.id().to_string(),
                    available: avail.available,
                    detail: avail.detail,
                    caps: harness_caps_wire(backend.id(), backend.caps()),
                }
            })
            .collect();
        providers.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(DoctorResult {
            host_id: self.host_id.clone(),
            pid: self.pid,
            rpc_url: self.rpc_url.clone(),
            db_ok,
            data_dir: self.data_dir.to_string_lossy().into_owned(),
            db_path: self.db_path.to_string_lossy().into_owned(),
            log_path: self.log_path.to_string_lossy().into_owned(),
            providers,
            workspace_count: counts.workspace_count,
            task_count: counts.task_count,
            agent_count: counts.agent_count,
            yolo,
        })
    }

    /// Prometheus text 0.0.4. No hostId, paths, transcript, or secrets.
    pub fn prometheus_metrics(&self) -> Result<String> {
        let mut open = 0usize;
        let mut archived = 0usize;
        let mut idle = 0usize;
        let mut running = 0usize;
        let mut error = 0usize;
        for task in self.store.task_list(TaskFilter::All)? {
            match task.status {
                TaskStatus::Open => open += 1,
                TaskStatus::Archived => archived += 1,
            }
            for agent in self.store.agent_list(&task.id)? {
                match agent.status {
                    AgentStatus::Idle => idle += 1,
                    AgentStatus::Running => running += 1,
                    AgentStatus::Error => error += 1,
                }
            }
        }
        Ok(format!(
            r#"# TYPE rusttraycer_up gauge
rusttraycer_up 1
# TYPE rusttraycer_agents gauge
rusttraycer_agents{{status="idle"}} {idle}
rusttraycer_agents{{status="running"}} {running}
rusttraycer_agents{{status="error"}} {error}
# TYPE rusttraycer_tasks gauge
rusttraycer_tasks{{status="open"}} {open}
rusttraycer_tasks{{status="archived"}} {archived}
"#
        ))
    }

    pub fn workspace_list(&self) -> Result<Vec<Workspace>> {
        Ok(self.store.workspace_list()?)
    }

    pub fn workspace_add(&self, path: &str) -> Result<Workspace> {
        if path.is_empty() {
            return Err(HostError::InvalidParams("path is required".into()));
        }
        let p = Path::new(path);
        if !p.exists() || !p.is_dir() {
            return Err(HostError::WorkspacePathInvalid(format!(
                "path must exist and be a directory: {path}"
            )));
        }
        let canon = p.canonicalize().map_err(|e| {
            HostError::WorkspacePathInvalid(format!("cannot canonicalize {path}: {e}"))
        })?;
        let name = canon
            .file_name()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("workspace")
            .to_string();
        Ok(self
            .store
            .workspace_add(canon.to_string_lossy().as_ref(), &name)?)
    }

    pub fn task_list(&self, status: &str) -> Result<Vec<Task>> {
        let filter = match status {
            "open" => TaskFilter::Open,
            "archived" => TaskFilter::Archived,
            "all" => TaskFilter::All,
            other => {
                return Err(HostError::InvalidParams(format!(
                    "status must be open|archived|all, got {other}"
                )))
            }
        };
        Ok(self.store.task_list(filter)?)
    }

    pub fn task_create(&self, title: &str, workspace_id: &str) -> Result<Task> {
        self.task_create_ex(title, workspace_id, None)
    }

    pub fn task_create_ex(
        &self,
        title: &str,
        workspace_id: &str,
        preset: Option<&str>,
    ) -> Result<Task> {
        check_title(title)?;
        if workspace_id.is_empty() {
            return Err(HostError::InvalidParams("workspaceId is required".into()));
        }
        let preset = match preset {
            None => None,
            Some(p) => Some(guides::parse_preset(p)?.id),
        };
        let task = self.store.task_create_ex(title, workspace_id, preset)?;
        let _ = self.events.send(WsEvent::task_updated(&task.id));
        Ok(task)
    }

    pub fn task_get(&self, id: &str) -> Result<Task> {
        self.store
            .task_get(id)?
            .ok_or_else(|| HostError::NotFound(format!("task {id}")))
    }

    pub fn task_rename(&self, id: &str, title: &str) -> Result<Task> {
        check_title(title)?;
        self.store.task_rename(id, title)?;
        let _ = self.events.send(WsEvent::task_updated(id));
        self.task_get(id)
    }

    pub fn task_archive(&self, id: &str) -> Result<Task> {
        let before = self.task_get(id)?;
        self.store.task_archive(id)?;
        if before.status != TaskStatus::Archived {
            let _ = self.events.send(WsEvent::task_updated(id));
        }
        self.task_get(id)
    }

    pub fn agent_list(&self, task_id: &str) -> Result<Vec<Agent>> {
        if self.store.task_get(task_id)?.is_none() {
            return Err(HostError::NotFound(format!("task {task_id}")));
        }
        Ok(self.store.agent_list(task_id)?)
    }

    pub fn agent_list_for_workspace(&self, workspace_id: &str) -> Result<Vec<Agent>> {
        if workspace_id.is_empty() {
            return Err(HostError::InvalidParams("workspaceId is required".into()));
        }
        if self.store.workspace_get(workspace_id)?.is_none() {
            return Err(HostError::NotFound(format!("workspace {workspace_id}")));
        }
        Ok(self.store.agent_list_for_workspace(workspace_id)?)
    }

    pub fn agent_create(&self, task_id: &str, provider: Option<&str>) -> Result<Agent> {
        self.agent_create_ex(AgentCreateArgs {
            task_id: Some(task_id),
            workspace_id: None,
            provider,
            interface: None,
            launch_args: None,
            parent_id: None,
            model: None,
            effort: None,
            fast: None,
            role: None,
            account_id: None,
        })
    }

    pub fn agent_create_ex(&self, args: AgentCreateArgs<'_>) -> Result<Agent> {
        let AgentCreateArgs {
            task_id,
            workspace_id,
            provider,
            interface,
            launch_args,
            parent_id,
            model,
            effort,
            fast,
            role,
            account_id,
        } = args;
        let provider = provider.unwrap_or("cli.generic");
        reject_provider(provider)?;
        let interface = interface.unwrap_or("chat");
        if interface != "chat" && interface != "terminal" {
            return Err(HostError::InvalidParams(format!(
                "interface must be chat|terminal, got {interface}"
            )));
        }
        let launch_args = launch_args.unwrap_or_default();
        if !launch_args.is_empty() && interface != "terminal" {
            return Err(HostError::InvalidParams(
                "launchArgs is only valid when interface=terminal".into(),
            ));
        }
        if launch_args.len() > 32 {
            return Err(HostError::InvalidParams(
                "launchArgs must have at most 32 strings".into(),
            ));
        }
        let task_id = task_id.filter(|s| !s.is_empty());
        let workspace_id = workspace_id.filter(|s| !s.is_empty());
        if interface == "terminal" {
            let pty = self
                .backends
                .get(provider)
                .map(|b| b.caps().pty)
                .unwrap_or(false);
            if !pty {
                return Err(HostError::NotPty);
            }
        }
        let mut spec = self.resolve_model_spec(provider, model, effort, fast)?;
        let agent = if let Some(task_id) = task_id {
            let task = self
                .store
                .task_get(task_id)?
                .ok_or_else(|| HostError::NotFound(format!("task {task_id}")))?;
            spec.role = self.resolve_role(&task, role)?;
            self.store.agent_create_model(
                task_id,
                &self.host_id,
                provider,
                interface,
                parent_id,
                spec.clone(),
            )?
        } else {
            let workspace_id = workspace_id.ok_or_else(|| {
                HostError::InvalidParams("workspaceId is required when taskId is omitted".into())
            })?;
            if self.store.workspace_get(workspace_id)?.is_none() {
                return Err(HostError::NotFound(format!("workspace {workspace_id}")));
            }
            if parent_id.is_some() {
                return Err(HostError::InvalidParams("parentId requires taskId".into()));
            }
            spec.role = match role {
                Some(r) => guides::parse_role(r)?.to_string(),
                None => "coder".into(),
            };
            self.store.agent_create_for_workspace(
                workspace_id,
                &self.host_id,
                provider,
                interface,
                spec.clone(),
            )?
        };
        self.store.harness_pref_upsert(
            provider,
            spec.model.as_deref(),
            spec.effort.as_deref(),
            spec.fast,
        )?;
        if interface == "terminal" && !launch_args.is_empty() {
            let mut g = self
                .launch_args
                .lock()
                .map_err(|_| HostError::Internal("launch_args lock poisoned".into()))?;
            g.insert(agent.id.clone(), launch_args);
        }
        if let Some(account_id) = account_id {
            self.bind_account(&agent.id, agent.provider.as_str(), Some(account_id))?;
            return self
                .store
                .agent_get(&agent.id)?
                .ok_or_else(|| HostError::NotFound(format!("agent {}", agent.id)));
        }
        Ok(agent)
    }

    fn bind_account(&self, agent_id: &str, provider: &str, account_id: Option<&str>) -> Result<()> {
        let Some(account_id) = account_id.filter(|s| !s.is_empty()) else {
            self.store.agent_set_account_id(agent_id, None)?;
            return Ok(());
        };
        let acc = self
            .store
            .account_get(account_id)?
            .ok_or_else(|| HostError::InvalidParams(format!("unknown accountId {account_id}")))?;
        if acc.provider != provider {
            return Err(HostError::InvalidParams(
                "accountId provider does not match agent provider".into(),
            ));
        }
        self.store
            .agent_set_account_id(agent_id, Some(account_id))?;
        Ok(())
    }

    pub fn account_create(&self, provider: &str, label: &str) -> Result<ProviderAccount> {
        reject_provider(provider)?;
        let row = self.store.account_create(provider, label)?;
        Ok(ProviderAccount {
            id: row.id,
            provider: row.provider,
            label: row.label,
        })
    }

    pub fn account_list(&self) -> Result<AccountListOk> {
        let items = self
            .store
            .account_list()?
            .into_iter()
            .map(|row| ProviderAccount {
                id: row.id,
                provider: row.provider,
                label: row.label,
            })
            .collect();
        Ok(AccountListOk { items })
    }

    pub fn stash_list(&self) -> Result<StashListOk> {
        let items = self
            .store
            .stash_list()?
            .into_iter()
            .map(stash_item_from_row)
            .collect();
        Ok(StashListOk { items })
    }

    pub fn stash_add(&self, body: &str, image_path: Option<&str>) -> Result<StashItem> {
        Ok(stash_item_from_row(self.store.stash_add(body, image_path)?))
    }

    pub fn stash_delete(&self, stash_id: &str) -> Result<StashDeleteOk> {
        self.store.stash_delete(stash_id)?;
        Ok(StashDeleteOk { deleted: true })
    }

    pub fn agent_steer(&self, agent_id: &str, content: &str) -> Result<AgentSteerOk> {
        if content.is_empty() || content.len() > MAX_CONTENT {
            return Err(HostError::InvalidParams(
                "content must be 1..=1048576 bytes".into(),
            ));
        }
        let agent = self
            .store
            .agent_get(agent_id)?
            .ok_or_else(|| HostError::NotFound(format!("agent {agent_id}")))?;
        if agent.status != AgentStatus::Running {
            return Err(HostError::InvalidParams(
                "agent.steer requires status=running".into(),
            ));
        }
        match agent.provider.as_str() {
            "cli.generic" => {
                return Err(HostError::NotSupported(
                    "cli.generic does not support steer".into(),
                ));
            }
            "cli.claude" | "cli.codex" => {}
            other => {
                return Err(HostError::InvalidParams(format!(
                    "provider must be cli.generic|cli.claude|cli.codex, got {other}"
                )));
            }
        }
        if let Ok(Some(live)) = self.mux.live_for_entity(PtyKind::Agent, agent_id) {
            if let Err(e) = self.mux.write(&live.pty_id, content.as_bytes()) {
                tracing::warn!(agent_id, error = ?e, "steer pty write");
            }
        }
        let _injected = self.inflight.push_steer(agent_id, content)?;
        Ok(AgentSteerOk { steered: true })
    }

    fn resolve_role(&self, task: &Task, role: Option<&str>) -> Result<String> {
        if let Some(r) = role {
            return Ok(guides::parse_role(r)?.to_string());
        }
        if let Some(preset) = task.preset.as_deref() {
            if let Ok(def) = guides::parse_preset(preset) {
                return Ok(def.default_role.to_string());
            }
        }
        Ok("coder".into())
    }

    pub fn agent_update(&self, agent_id: &str, role: &str) -> Result<AgentView> {
        let role = guides::parse_role(role)?;
        if self.store.agent_get(agent_id)?.is_none() {
            return Err(HostError::NotFound(format!("agent {agent_id}")));
        }
        self.store.agent_set_role(agent_id, role)?;
        self.agent_get(agent_id)
    }

    pub fn workspace_guides_get(&self, workspace_id: &str) -> Result<WorkspaceGuidesOk> {
        let ws = self
            .store
            .workspace_get(workspace_id)?
            .ok_or_else(|| HostError::NotFound(format!("workspace {workspace_id}")))?;
        let root = Path::new(&ws.path);
        Ok(WorkspaceGuidesOk {
            agents_md: guides::read_guide_file(&guides::agents_md_path(root)),
            workspace_guide: guides::read_guide_file(&guides::workspace_guide_path(root)),
            global_guide: guides::read_guide_file(&guides::global_guide_path(&self.data_dir)),
        })
    }

    pub fn settings_guide_get(&self) -> SettingsGuide {
        guides::settings_guide_get(&self.data_dir)
    }

    pub fn settings_guide_set(&self, content: &str) -> Result<SettingsGuide> {
        guides::settings_guide_set(&self.data_dir, content)
    }

    pub fn preset_list(&self) -> Result<PresetListOk> {
        let mut items = guides::preset_items();
        for row in self.store.user_preset_list()? {
            items.push(preset_item_from_user(row));
        }
        Ok(PresetListOk { items })
    }

    pub fn preset_create(&self, params: &PresetCreateParams) -> Result<PresetItem> {
        if builtin_preset_conflict(&params.name) {
            return Err(HostError::InvalidParams(
                "name must not match a built-in preset id or title".into(),
            ));
        }
        let role = guides::parse_role(&params.default_role)?;
        let row = self.store.user_preset_create(
            &params.name,
            role,
            params.title_hint.as_deref(),
            params.prompt.as_deref(),
        )?;
        Ok(preset_item_from_user(row))
    }

    pub fn preset_update(&self, params: &PresetUpdateParams) -> Result<PresetItem> {
        if is_builtin_preset_id(&params.id) {
            return Err(HostError::InvalidParams(
                "cannot update a built-in preset".into(),
            ));
        }
        if let Some(name) = params.name.as_deref() {
            if builtin_preset_conflict(name) {
                return Err(HostError::InvalidParams(
                    "name must not match a built-in preset id or title".into(),
                ));
            }
        }
        let role = match params.default_role.as_deref() {
            Some(r) => Some(guides::parse_role(r)?),
            None => None,
        };
        let row = self.store.user_preset_update(
            &params.id,
            params.name.as_deref(),
            role,
            params.title_hint.as_deref(),
            params.prompt.as_deref(),
        )?;
        Ok(preset_item_from_user(row))
    }

    pub fn preset_delete(&self, id: &str) -> Result<PresetDeleteOk> {
        if is_builtin_preset_id(id) {
            return Err(HostError::InvalidParams(
                "cannot delete a built-in preset".into(),
            ));
        }
        self.store.user_preset_delete(id)?;
        Ok(PresetDeleteOk { deleted: true })
    }

    pub fn search_query(&self, params: &SearchQueryParams) -> Result<SearchQueryOk> {
        if params.q.is_empty() {
            return Ok(SearchQueryOk { items: Vec::new() });
        }
        let kinds: Vec<&str> = match params.kinds.as_deref() {
            None | Some([]) => vec!["task", "workspace", "artifact"],
            Some(ks) => ks
                .iter()
                .map(|k| match k {
                    SearchKind::Task => "task",
                    SearchKind::Workspace => "workspace",
                    SearchKind::Artifact => "artifact",
                })
                .collect(),
        };
        let hits = self.store.search_query(&params.q, &kinds)?;
        let items = hits
            .into_iter()
            .map(|h| {
                let kind = match h.kind.as_str() {
                    "workspace" => SearchKind::Workspace,
                    "artifact" => SearchKind::Artifact,
                    _ => SearchKind::Task,
                };
                SearchItem {
                    kind,
                    id: h.id,
                    title: h.title,
                    hint: h.hint,
                }
            })
            .collect();
        Ok(SearchQueryOk { items })
    }

    fn resolve_model_spec(
        &self,
        provider: &str,
        model: Option<String>,
        effort: Option<String>,
        fast: Option<bool>,
    ) -> Result<AgentModelSpec> {
        let prefs = self.store.harness_pref_get(provider)?;
        Ok(AgentModelSpec {
            model: model.or_else(|| prefs.as_ref().and_then(|p| p.model.clone())),
            effort: effort.or_else(|| prefs.as_ref().and_then(|p| p.effort.clone())),
            fast: fast.unwrap_or_else(|| prefs.as_ref().map(|p| p.fast).unwrap_or(false)),
            role: "coder".into(),
        })
    }

    pub fn agent_switch(&self, p: AgentSwitchParams) -> Result<AgentView> {
        let agent_id = p.agent_id.as_str();
        let agent = self
            .store
            .agent_get(agent_id)?
            .ok_or_else(|| HostError::NotFound(format!("agent {agent_id}")))?;
        if agent.status == AgentStatus::Running {
            return Err(HostError::AgentBusy);
        }
        let (provider, spec) = if let Some(pid) = p.profile_id.as_deref() {
            let profile = self
                .store
                .profile_get(pid)?
                .ok_or_else(|| HostError::NotFound(format!("profile {pid}")))?;
            let provider = p
                .provider
                .as_deref()
                .unwrap_or(profile.provider.as_str())
                .to_string();
            reject_provider(&provider)?;
            let spec = AgentModelSpec {
                model: p.model.or(profile.model),
                effort: p.effort.or(profile.effort),
                fast: p.fast.unwrap_or(profile.fast),
                role: agent.role.clone(),
            };
            (provider, spec)
        } else {
            let provider = p
                .provider
                .as_deref()
                .unwrap_or_else(|| agent.provider.as_str())
                .to_string();
            reject_provider(&provider)?;
            let spec = self.resolve_model_spec(&provider, p.model, p.effort, p.fast)?;
            (provider, spec)
        };
        if agent.interface == "terminal" {
            let pty = self
                .backends
                .get(provider.as_str())
                .map(|b| b.caps().pty)
                .unwrap_or(false);
            if !pty {
                return Err(HostError::NotPty);
            }
        }
        self.store
            .agent_switch(agent_id, provider.as_str(), spec.clone())?;
        if p.account_id.is_some() {
            self.bind_account(agent_id, provider.as_str(), p.account_id.as_deref())?;
        } else if let Some(existing) = agent.account_id.as_deref() {
            if let Some(acc) = self.store.account_get(existing)? {
                if acc.provider != provider {
                    self.store.agent_set_account_id(agent_id, None)?;
                }
            }
        }
        self.store.harness_pref_upsert(
            provider.as_str(),
            spec.model.as_deref(),
            spec.effort.as_deref(),
            spec.fast,
        )?;
        let updated = self
            .store
            .agent_get(agent_id)?
            .ok_or_else(|| HostError::NotFound(format!("agent {agent_id}")))?;
        let mut view = AgentView::from(updated);
        view.last_message_at = self.store.last_message_at(agent_id)?;
        view.yolo = self.resolve_policy_for_agent(agent_id)?.yolo;
        Ok(view)
    }

    pub fn profile_create(&self, p: ProfileCreateParams) -> Result<Profile> {
        check_profile_name(&p.name)?;
        reject_provider(&p.provider)?;
        let row = self.store.profile_create(
            &p.name,
            &p.provider,
            p.model.as_deref(),
            p.effort.as_deref(),
            p.fast.unwrap_or(false),
        )?;
        Ok(profile_from_row(row))
    }

    pub fn profile_list(&self) -> Result<ProfileListOk> {
        let items = self
            .store
            .profile_list()?
            .into_iter()
            .map(profile_from_row)
            .collect();
        Ok(ProfileListOk { items })
    }

    pub fn profile_get(&self, p: &ProfileGetParams) -> Result<Profile> {
        let row = self
            .store
            .profile_get(&p.profile_id)?
            .ok_or_else(|| HostError::NotFound(format!("profile {}", p.profile_id)))?;
        Ok(profile_from_row(row))
    }

    pub fn profile_update(&self, p: ProfileUpdateParams) -> Result<Profile> {
        let current = self
            .store
            .profile_get(&p.profile_id)?
            .ok_or_else(|| HostError::NotFound(format!("profile {}", p.profile_id)))?;
        let name = p.name.unwrap_or(current.name);
        check_profile_name(&name)?;
        let provider = p.provider.unwrap_or(current.provider);
        reject_provider(&provider)?;
        let model = p.model.or(current.model);
        let effort = p.effort.or(current.effort);
        let fast = p.fast.unwrap_or(current.fast);
        let row = self.store.profile_update(
            &p.profile_id,
            &name,
            &provider,
            model.as_deref(),
            effort.as_deref(),
            fast,
        )?;
        Ok(profile_from_row(row))
    }

    pub fn profile_delete(&self, p: &ProfileDeleteParams) -> Result<()> {
        self.store.profile_delete(&p.profile_id)?;
        Ok(())
    }

    pub fn prefs_get(&self) -> Result<PrefsGetOk> {
        let mut items = Vec::with_capacity(ALLOWED_HARNESSES.len());
        for provider in ALLOWED_HARNESSES {
            match self.store.harness_pref_get(provider)? {
                Some(row) => items.push(PrefsItem {
                    provider: row.provider,
                    model: row.model,
                    effort: row.effort,
                    fast: row.fast,
                }),
                None => items.push(PrefsItem {
                    provider: (*provider).to_string(),
                    model: None,
                    effort: None,
                    fast: false,
                }),
            }
        }
        Ok(PrefsGetOk { items })
    }

    pub fn agent_get(&self, id: &str) -> Result<AgentView> {
        let agent = self
            .store
            .agent_get(id)?
            .ok_or_else(|| HostError::NotFound(format!("agent {id}")))?;
        let mut view = AgentView::from(agent);
        view.last_message_at = self.store.last_message_at(id)?;
        view.yolo = self.resolve_policy_for_agent(id)?.yolo;
        Ok(view)
    }

    pub fn get_context(&self, agent_id: &str) -> Result<Vec<Message>> {
        if self.store.agent_get(agent_id)?.is_none() {
            return Err(HostError::NotFound(format!("agent {agent_id}")));
        }
        Ok(self.store.message_list(agent_id)?)
    }

    pub fn files_tree(
        &self,
        workspace_id: &str,
        path: Option<&str>,
        depth: Option<u32>,
        max_entries: Option<u32>,
        worktree_id: Option<&str>,
    ) -> Result<serde_json::Value> {
        let mut p = serde_json::json!({ "workspaceId": workspace_id });
        if let Some(path) = path {
            p["path"] = serde_json::json!(path);
        }
        if let Some(d) = depth {
            p["depth"] = serde_json::json!(d);
        }
        if let Some(m) = max_entries {
            p["maxEntries"] = serde_json::json!(m);
        }
        if let Some(wt) = worktree_id {
            p["worktreeId"] = serde_json::json!(wt);
        }
        files::files_tree(&self.store, &p)
    }

    pub fn files_read(
        &self,
        workspace_id: &str,
        path: &str,
        worktree_id: Option<&str>,
    ) -> Result<serde_json::Value> {
        let mut p = serde_json::json!({ "workspaceId": workspace_id, "path": path });
        if let Some(wt) = worktree_id {
            p["worktreeId"] = serde_json::json!(wt);
        }
        files::files_read(&self.store, &p)
    }

    /// Durable user message + spawn turn. Ladder off — existing callers stay green.
    pub fn send(&self, agent_id: &str, content: &str) -> Result<Message> {
        Ok(self.send_gated(agent_id, content, false, None)?.user)
    }

    pub fn send_gated(
        &self,
        agent_id: &str,
        content: &str,
        ladder: bool,
        attached: Option<&std::path::Path>,
    ) -> Result<SendOutcome> {
        if content.len() > MAX_CONTENT {
            return Err(HostError::InvalidParams("content exceeds 1 MiB".into()));
        }
        if content.is_empty() {
            return Err(HostError::InvalidParams("content is required".into()));
        }

        let gate = self
            .turn_gate
            .lock()
            .map_err(|_| HostError::Internal("turn_gate poisoned".into()))?;

        let agent = self
            .store
            .agent_get(agent_id)?
            .ok_or_else(|| HostError::NotFound(format!("agent {agent_id}")))?;

        if agent.interface == "terminal" {
            return Err(HostError::InvalidParams(
                "agent.send is not valid on interface=terminal; use pty.write".into(),
            ));
        }

        if agent.status == AgentStatus::Running
            || self.inflight.contains(agent_id)?
            || self.has_pending_locked(agent_id)?
        {
            return Err(HostError::AgentBusy);
        }

        let task = self
            .store
            .task_get(&agent.task_id)?
            .ok_or_else(|| HostError::NotFound(format!("task {}", agent.task_id)))?;
        let ws_id = task
            .workspace_ids
            .first()
            .ok_or_else(|| HostError::Internal("task has no workspace".into()))?;
        let workspace = self
            .store
            .workspace_get(ws_id)?
            .ok_or_else(|| HostError::NotFound(format!("workspace {ws_id}")))?;

        if ladder {
            let view = self.resolve_policy_for_agent(agent_id)?;
            if !view.yolo && view.mode == PolicyMode::Deny {
                tracing::info!(agent_id, "policy deny");
                return Err(HostError::Denied);
            }
            if !view.yolo && view.mode == PolicyMode::Ask {
                let backend = self.lookup_backend(&agent)?;
                let avail = backend.available();
                if !avail.available {
                    return Err(HostError::Internal(avail.detail));
                }
                let user = self
                    .store
                    .message_append(agent_id, MessageRole::User, content)?;
                let _ = self
                    .events
                    .send(WsEvent::agent_message(&task.id, agent_id, user.clone()));
                let _ = self.events.send(WsEvent::task_updated(&task.id));
                let approval_id = rt_storage::new_id();
                let summary = format!("spawn {}", agent.provider);
                self.store_pending(PendingApproval {
                    approval_id: approval_id.clone(),
                    agent_id: agent_id.to_string(),
                    task_id: task.id.clone(),
                    kind: "exec".into(),
                    summary: summary.clone(),
                    payload: PendingKind::Exec,
                })?;
                let _ = self.events.send(WsEvent::agent_approval(
                    &approval_id,
                    agent_id,
                    &task.id,
                    "exec",
                    &summary,
                ));
                tracing::info!(agent_id, approval_id = %approval_id, "approval pending");
                return Ok(SendOutcome {
                    user,
                    approval_id: Some(approval_id),
                });
            }
        }

        let backend = self.lookup_backend(&agent)?;
        let avail = backend.available();
        if !avail.available {
            return Err(HostError::Internal(avail.detail));
        }

        let user = self.append_user_and_spawn(
            agent_id,
            content,
            &task,
            workspace.path.into(),
            backend,
            attached.map(std::path::Path::to_path_buf),
        )?;
        drop(gate);
        Ok(SendOutcome {
            user,
            approval_id: None,
        })
    }

    /// Cancel an inflight turn. Idempotent: idle/error/finished -> cancelled false.
    pub fn cancel(&self, agent_id: &str) -> Result<CancelOk> {
        if uuid::Uuid::parse_str(agent_id).is_err() {
            return Err(HostError::InvalidParams("invalid agentId".into()));
        }

        let _gate = self
            .turn_gate
            .lock()
            .map_err(|_| HostError::Internal("turn_gate poisoned".into()))?;

        let agent = self
            .store
            .agent_get(agent_id)?
            .ok_or_else(|| HostError::NotFound(format!("agent {agent_id}")))?;

        let closed_pty = match self.mux.kill_entity(PtyKind::Agent, agent_id) {
            Ok(Some(_)) => {
                if let Err(e) = self.store.agent_set_status(agent_id, AgentStatus::Idle) {
                    tracing::warn!(agent_id, error = %e, "pty close status");
                }
                true
            }
            Ok(None) => false,
            Err(e) => return Err(HostError::Internal(e)),
        };

        if self.clear_pending_for_agent(agent_id)? {
            tracing::info!(agent_id, "cancel pending approval");
            return Ok(CancelOk {
                agent_id: agent_id.to_string(),
                cancelled: true,
            });
        }

        let has_inflight = self.inflight.contains(agent_id)?;
        if !has_inflight && agent.status != AgentStatus::Running && !closed_pty {
            return Ok(CancelOk {
                agent_id: agent_id.to_string(),
                cancelled: false,
            });
        }

        if let Some(backend) = self.backends.get(agent.provider.as_str()) {
            if let Err(e) = backend.cancel_turn(agent_id) {
                return Err(HostError::Internal(e.message));
            }
        }

        // Let run_turn flush any unflushed assistant buffer on stream end.
        // Do not delete existing Messages.
        self.store.agent_set_status(agent_id, AgentStatus::Idle)?;
        let _ = self.inflight.take(agent_id)?;
        let _ = self.events.send(WsEvent::agent_status(
            &agent.task_id,
            agent_id,
            AgentStatus::Idle,
        ));

        Ok(CancelOk {
            agent_id: agent_id.to_string(),
            cancelled: true,
        })
    }

    pub fn going_away(&self) {
        let _ = self.events.send(WsEvent::host_going_away(&self.host_id));
    }

    pub fn policy_get(&self, params: &PolicyGetParams) -> Result<PolicyView> {
        match (params.agent_id.as_deref(), params.workspace_id.as_deref()) {
            (Some(agent_id), None) => {
                if self.store.agent_get(agent_id)?.is_none() {
                    return Err(HostError::NotFound(format!("agent {agent_id}")));
                }
                self.resolve_policy_for_agent(agent_id)
            }
            (None, Some(workspace_id)) => {
                if self.store.workspace_get(workspace_id)?.is_none() {
                    return Err(HostError::NotFound(format!("workspace {workspace_id}")));
                }
                self.resolve_policy_for_workspace(workspace_id)
            }
            _ => Err(HostError::InvalidParams(
                "exactly one of agentId / workspaceId is required".into(),
            )),
        }
    }

    pub fn policy_set(&self, params: &PolicySetParams) -> Result<PolicyView> {
        match params.scope {
            PolicyScope::Agent => {
                let agent_id = params.agent_id.as_deref().ok_or_else(|| {
                    HostError::InvalidParams("scope=agent requires agentId".into())
                })?;
                if params.workspace_id.is_some() {
                    return Err(HostError::InvalidParams(
                        "exactly one of agentId / workspaceId is required".into(),
                    ));
                }
                if self.store.agent_get(agent_id)?.is_none() {
                    return Err(HostError::NotFound(format!("agent {agent_id}")));
                }
                let row = self.store.policy_upsert(
                    Some(agent_id),
                    None,
                    params.mode.as_str(),
                    "agent",
                    params.yolo,
                )?;
                tracing::info!(agent_id, mode = %row.mode, yolo = row.yolo, "policy.set agent");
                policy_view_from_row(&row, PolicySource::Agent)
            }
            PolicyScope::Workspace => {
                let workspace_id = params.workspace_id.as_deref().ok_or_else(|| {
                    HostError::InvalidParams("scope=workspace requires workspaceId".into())
                })?;
                if params.agent_id.is_some() {
                    return Err(HostError::InvalidParams(
                        "exactly one of agentId / workspaceId is required".into(),
                    ));
                }
                if self.store.workspace_get(workspace_id)?.is_none() {
                    return Err(HostError::NotFound(format!("workspace {workspace_id}")));
                }
                let row = self.store.policy_upsert(
                    None,
                    Some(workspace_id),
                    params.mode.as_str(),
                    "workspace",
                    params.yolo,
                )?;
                tracing::info!(
                    workspace_id,
                    mode = %row.mode,
                    yolo = row.yolo,
                    "policy.set workspace"
                );
                policy_view_from_row(&row, PolicySource::Workspace)
            }
        }
    }

    pub fn approval_respond(&self, params: &ApprovalRespondParams) -> Result<ApprovalRespondOk> {
        let _gate = self
            .turn_gate
            .lock()
            .map_err(|_| HostError::Internal("turn_gate poisoned".into()))?;

        {
            let g = self
                .ladder
                .lock()
                .map_err(|_| HostError::Internal("ladder lock poisoned".into()))?;
            if g.applied.contains(&params.approval_id) {
                return Ok(ApprovalRespondOk { applied: false });
            }
        }

        let pending = self.take_pending(&params.approval_id)?;
        let Some(pending) = pending else {
            return Err(HostError::ApprovalExpired);
        };

        let result = match params.decision {
            ApprovalDecision::Deny => {
                tracing::info!(approval_id = %params.approval_id, "approval deny");
                Ok(())
            }
            ApprovalDecision::AllowOnce => {
                tracing::info!(
                    approval_id = %params.approval_id,
                    kind = %pending.kind,
                    summary = %pending.summary,
                    task_id = %pending.task_id,
                    "approval allow-once"
                );
                self.apply_pending(&pending)
            }
            ApprovalDecision::AllowAlways => {
                tracing::info!(
                    approval_id = %params.approval_id,
                    kind = %pending.kind,
                    summary = %pending.summary,
                    task_id = %pending.task_id,
                    "approval allow-always"
                );
                let existing = self.store.policy_get_for_agent(&pending.agent_id)?;
                let yolo = existing.map(|r| r.yolo).unwrap_or(false);
                self.store.policy_upsert(
                    Some(&pending.agent_id),
                    None,
                    PolicyMode::AllowAlways.as_str(),
                    "agent",
                    yolo,
                )?;
                self.apply_pending(&pending)
            }
        };
        if let Err(e) = result {
            self.store_pending(pending)?;
            return Err(e);
        }

        let mut g = self
            .ladder
            .lock()
            .map_err(|_| HostError::Internal("ladder lock poisoned".into()))?;
        g.applied.insert(params.approval_id.clone());
        Ok(ApprovalRespondOk { applied: true })
    }

    fn resolve_policy_for_agent(&self, agent_id: &str) -> Result<PolicyView> {
        if let Some(row) = self.store.policy_get_for_agent(agent_id)? {
            return policy_view_from_row(&row, PolicySource::Agent);
        }
        let agent = self
            .store
            .agent_get(agent_id)?
            .ok_or_else(|| HostError::NotFound(format!("agent {agent_id}")))?;
        if !agent.task_id.is_empty() {
            let task = self
                .store
                .task_get(&agent.task_id)?
                .ok_or_else(|| HostError::NotFound(format!("task {}", agent.task_id)))?;
            if let Some(ws_id) = task.workspace_ids.first() {
                if let Some(row) = self.store.policy_get_for_workspace(ws_id)? {
                    return policy_view_from_row(&row, PolicySource::Workspace);
                }
            }
        } else if let Some(ws_id) = agent.workspace_id.as_deref() {
            if let Some(row) = self.store.policy_get_for_workspace(ws_id)? {
                return policy_view_from_row(&row, PolicySource::Workspace);
            }
        }
        Ok(PolicyView {
            mode: PolicyMode::Ask,
            scope: PolicyScope::Agent,
            yolo: false,
            source: PolicySource::Default,
        })
    }

    fn resolve_policy_for_workspace(&self, workspace_id: &str) -> Result<PolicyView> {
        if let Some(row) = self.store.policy_get_for_workspace(workspace_id)? {
            return policy_view_from_row(&row, PolicySource::Workspace);
        }
        Ok(PolicyView {
            mode: PolicyMode::Ask,
            scope: PolicyScope::Workspace,
            yolo: false,
            source: PolicySource::Default,
        })
    }

    fn lookup_backend(&self, agent: &Agent) -> Result<Arc<dyn AgentBackend>> {
        self.backends
            .get(agent.provider.as_str())
            .cloned()
            .ok_or_else(|| {
                HostError::Internal(format!("no backend registered for {}", agent.provider))
            })
    }

    fn has_pending_locked(&self, agent_id: &str) -> Result<bool> {
        let g = self
            .ladder
            .lock()
            .map_err(|_| HostError::Internal("ladder lock poisoned".into()))?;
        Ok(g.pending_by_agent.contains_key(agent_id))
    }

    fn store_pending(&self, pending: PendingApproval) -> Result<()> {
        let mut g = self
            .ladder
            .lock()
            .map_err(|_| HostError::Internal("ladder lock poisoned".into()))?;
        g.pending_by_agent
            .insert(pending.agent_id.clone(), pending.approval_id.clone());
        g.pending_by_id.insert(pending.approval_id.clone(), pending);
        Ok(())
    }

    fn take_pending(&self, approval_id: &str) -> Result<Option<PendingApproval>> {
        let mut g = self
            .ladder
            .lock()
            .map_err(|_| HostError::Internal("ladder lock poisoned".into()))?;
        let pending = g.pending_by_id.remove(approval_id);
        if let Some(ref p) = pending {
            g.pending_by_agent.remove(&p.agent_id);
        }
        Ok(pending)
    }

    fn clear_pending_for_agent(&self, agent_id: &str) -> Result<bool> {
        let mut g = self
            .ladder
            .lock()
            .map_err(|_| HostError::Internal("ladder lock poisoned".into()))?;
        if let Some(id) = g.pending_by_agent.remove(agent_id) {
            g.pending_by_id.remove(&id);
            return Ok(true);
        }
        Ok(false)
    }

    fn append_user_and_spawn(
        &self,
        agent_id: &str,
        content: &str,
        task: &Task,
        workspace_path: std::path::PathBuf,
        backend: Arc<dyn AgentBackend>,
        attached: Option<std::path::PathBuf>,
    ) -> Result<Message> {
        let gen = self.inflight.reserve(agent_id)?;
        let user = match self
            .store
            .message_append(agent_id, MessageRole::User, content)
        {
            Ok(user) => user,
            Err(e) => {
                self.inflight.remove_if(agent_id, gen);
                return Err(e.into());
            }
        };
        if let Err(e) = self.spawn_reserved_turn(ReservedTurn {
            agent_id: agent_id.to_string(),
            task_id: task.id.clone(),
            workspace_path,
            backend,
            gen,
            user: Some(user.clone()),
            attached,
        }) {
            self.inflight.remove_if(agent_id, gen);
            return Err(e);
        }
        Ok(user)
    }

    fn apply_pending(&self, pending: &PendingApproval) -> Result<()> {
        match &pending.payload {
            PendingKind::Exec => self.start_saved_turn(pending),
            PendingKind::EditWrite {
                workspace_id,
                worktree_id,
                path,
                content,
            } => {
                files::apply_write(
                    &self.store,
                    workspace_id,
                    worktree_id.as_deref(),
                    path,
                    content,
                )?;
                Ok(())
            }
            PendingKind::EditPatch {
                workspace_id,
                worktree_id,
                patch,
            } => {
                self.apply_patch_at(workspace_id, worktree_id.as_deref(), patch)?;
                Ok(())
            }
            PendingKind::PtyOpen { cols, rows } => {
                self.spawn_agent_pty(&pending.agent_id, *cols, *rows)?;
                Ok(())
            }
        }
    }

    pub fn files_write_gated(
        &self,
        params: &serde_json::Value,
        ladder: bool,
    ) -> Result<serde_json::Value> {
        self.edit_gated(params, ladder, EditOp::Write)
    }

    pub fn files_patch_gated(
        &self,
        params: &serde_json::Value,
        ladder: bool,
    ) -> Result<serde_json::Value> {
        self.edit_gated(params, ladder, EditOp::Patch)
    }

    pub fn files_open(&self, params: &serde_json::Value) -> Result<serde_json::Value> {
        files::files_open(&self.store, params)
    }

    fn edit_gated(
        &self,
        params: &serde_json::Value,
        ladder: bool,
        op: EditOp,
    ) -> Result<serde_json::Value> {
        if !params.is_object() {
            return Err(HostError::InvalidParams("params must be an object".into()));
        }
        let workspace_id = params
            .get("workspaceId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| HostError::InvalidParams("workspaceId is required".into()))?;
        let agent_id = params
            .get("agentId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| HostError::InvalidParams("agentId is required".into()))?;
        if uuid::Uuid::parse_str(agent_id).is_err() {
            return Err(HostError::InvalidParams("invalid agentId".into()));
        }
        let worktree_id = match params.get("worktreeId") {
            None | Some(serde_json::Value::Null) => None,
            Some(serde_json::Value::String(s)) => Some(s.as_str()),
            Some(_) => {
                return Err(HostError::InvalidParams(
                    "worktreeId must be a string".into(),
                ));
            }
        };

        let (summary, payload) = match op {
            EditOp::Write => {
                let path = params
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| HostError::InvalidParams("path is required".into()))?;
                let content = params
                    .get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| HostError::InvalidParams("content is required".into()))?;
                files::check_text_content(content)?;
                let ws = files::require_workspace(&self.store, workspace_id)?;
                let root = files::walk_root(&self.store, &ws, worktree_id)?;
                files::resolve_for_create(&root, path)?;
                (
                    format!("write {path}"),
                    PendingKind::EditWrite {
                        workspace_id: workspace_id.to_string(),
                        worktree_id: worktree_id.map(str::to_string),
                        path: path.to_string(),
                        content: content.to_string(),
                    },
                )
            }
            EditOp::Patch => {
                let patch = params
                    .get("patch")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| HostError::InvalidParams("patch is required".into()))?;
                if (patch.len() as u64) > files::MAX_FILE_BYTES {
                    return Err(HostError::FileTooLarge("patch exceeds 256 KiB".into()));
                }
                let scan = patch.len().min(files::BINARY_SCAN_BYTES);
                if patch.as_bytes()[..scan].contains(&0) {
                    return Err(HostError::FileBinary("NUL in first 8 KiB".into()));
                }
                let (paths, _) = crate::worktree::parse_patch_stats(patch);
                let summary = if paths.is_empty() {
                    "patch".to_string()
                } else {
                    format!("patch {} files", paths.len())
                };
                (
                    summary,
                    PendingKind::EditPatch {
                        workspace_id: workspace_id.to_string(),
                        worktree_id: worktree_id.map(str::to_string),
                        patch: patch.to_string(),
                    },
                )
            }
        };

        let gate = self
            .turn_gate
            .lock()
            .map_err(|_| HostError::Internal("turn_gate poisoned".into()))?;

        let agent = self
            .store
            .agent_get(agent_id)?
            .ok_or_else(|| HostError::NotFound(format!("agent {agent_id}")))?;

        if agent.status == AgentStatus::Running
            || self.inflight.contains(agent_id)?
            || self.has_pending_locked(agent_id)?
        {
            return Err(HostError::AgentBusy);
        }

        if ladder {
            let view = self.resolve_policy_for_agent(agent_id)?;
            if !view.yolo && view.mode == PolicyMode::Deny {
                tracing::info!(agent_id, "policy deny edit");
                return Err(HostError::Denied);
            }
            if !view.yolo && view.mode == PolicyMode::Ask {
                let approval_id = rt_storage::new_id();
                self.store_pending(PendingApproval {
                    approval_id: approval_id.clone(),
                    agent_id: agent_id.to_string(),
                    task_id: agent.task_id.clone(),
                    kind: "edit".into(),
                    summary: summary.clone(),
                    payload,
                })?;
                let _ = self.events.send(WsEvent::agent_approval(
                    &approval_id,
                    agent_id,
                    &agent.task_id,
                    "edit",
                    &summary,
                ));
                tracing::info!(agent_id, approval_id = %approval_id, "edit approval pending");
                drop(gate);
                let mut ok = serde_json::json!({ "approvalId": approval_id });
                if let Some(path) = params.get("path") {
                    ok["path"] = path.clone();
                }
                return Ok(ok);
            }
        }

        drop(gate);
        match payload {
            PendingKind::EditWrite {
                workspace_id,
                worktree_id,
                path,
                content,
            } => files::apply_write(
                &self.store,
                &workspace_id,
                worktree_id.as_deref(),
                &path,
                &content,
            ),
            PendingKind::EditPatch {
                workspace_id,
                worktree_id,
                patch,
            } => self.apply_patch_at(&workspace_id, worktree_id.as_deref(), &patch),
            PendingKind::Exec => Err(HostError::Internal("edit payload missing".into())),
            PendingKind::PtyOpen { .. } => Err(HostError::Internal("edit payload missing".into())),
        }
    }

    fn start_saved_turn(&self, pending: &PendingApproval) -> Result<()> {
        let agent = self
            .store
            .agent_get(&pending.agent_id)?
            .ok_or_else(|| HostError::NotFound(format!("agent {}", pending.agent_id)))?;
        if agent.status == AgentStatus::Running || self.inflight.contains(&pending.agent_id)? {
            return Err(HostError::AgentBusy);
        }
        let backend = self.lookup_backend(&agent)?;
        let avail = backend.available();
        if !avail.available {
            return Err(HostError::Internal(avail.detail));
        }
        let task = self
            .store
            .task_get(&agent.task_id)?
            .ok_or_else(|| HostError::NotFound(format!("task {}", agent.task_id)))?;
        let ws_id = task
            .workspace_ids
            .first()
            .ok_or_else(|| HostError::Internal("task has no workspace".into()))?;
        let workspace = self
            .store
            .workspace_get(ws_id)?
            .ok_or_else(|| HostError::NotFound(format!("workspace {ws_id}")))?;
        let gen = self.inflight.reserve(&pending.agent_id)?;
        if let Err(e) = self.spawn_reserved_turn(ReservedTurn {
            agent_id: pending.agent_id.clone(),
            task_id: task.id.clone(),
            workspace_path: workspace.path.into(),
            backend,
            gen,
            user: None,
            attached: None,
        }) {
            self.inflight.remove_if(&pending.agent_id, gen);
            return Err(e);
        }
        Ok(())
    }

    fn spawn_reserved_turn(&self, args: ReservedTurn) -> Result<()> {
        let ReservedTurn {
            agent_id,
            task_id,
            workspace_path,
            backend,
            gen,
            user,
            attached,
        } = args;
        self.store
            .agent_set_status(&agent_id, AgentStatus::Running)?;
        if let Err(e) = self.store.task_touch(&task_id) {
            tracing::warn!(task_id, error = %e, "task_touch failed");
        }
        let _ = self.events.send(WsEvent::agent_status(
            &task_id,
            &agent_id,
            AgentStatus::Running,
        ));
        if let Some(user) = user {
            let _ = self
                .events
                .send(WsEvent::agent_message(&task_id, &agent_id, user));
        }
        let _ = self.events.send(WsEvent::task_updated(&task_id));

        let transcript = self
            .store
            .message_list(&agent_id)?
            .into_iter()
            .map(|m| WireMessage {
                role: match m.role {
                    MessageRole::User => WireRole::User,
                    MessageRole::Assistant => WireRole::Assistant,
                    MessageRole::System => WireRole::System,
                    MessageRole::Tool => WireRole::Tool,
                },
                content: m.content,
            })
            .collect();
        let role = match self.store.agent_get(&agent_id)? {
            Some(a) => a.role,
            None => "coder".to_string(),
        };
        let worktree = self
            .store
            .worktree_get_by_agent(&agent_id)
            .ok()
            .flatten()
            .map(|w| std::path::PathBuf::from(w.path));
        let start = guides::walk_start(attached.as_deref(), worktree.as_deref(), &workspace_path);
        let messages =
            guides::inject_preamble(&self.data_dir, &workspace_path, &start, &role, transcript);

        let req = TurnRequest {
            agent_id: agent_id.clone(),
            task_id: task_id.clone(),
            workspace_path,
            messages,
            extra_env: {
                let mut env = BTreeMap::new();
                env.insert("RUSTTRAYCER_AGENT_ID".into(), agent_id.clone());
                env.insert("RUSTTRAYCER_TASK_ID".into(), task_id.clone());
                if let Some(agent) = self.store.agent_get(&agent_id)? {
                    if let Some(acc_id) = agent.account_id.as_deref() {
                        if let Some(acc) = self.store.account_get(acc_id)? {
                            if let Some((k, v)) = account_secret_from_env(&acc.provider, &acc.label)
                            {
                                env.insert(k, v);
                            }
                        }
                    }
                }
                env
            },
        };

        let handle = supervisor::spawn_turn(supervisor::SpawnTurn {
            store: self.store.clone(),
            backend,
            req,
            agent_id: agent_id.clone(),
            task_id: task_id.clone(),
            events: self.events.clone(),
            inflight: self.inflight.clone(),
            gen,
            timeout: self.turn_timeout,
        });
        self.inflight.attach(&agent_id, gen, handle);
        Ok(())
    }

    pub fn pty_open(
        &self,
        agent_id: Option<&str>,
        shell_id: Option<&str>,
        cols: u16,
        rows: u16,
        ladder: bool,
    ) -> Result<serde_json::Value> {
        Self::check_pty_size(cols, rows)?;
        match (agent_id, shell_id) {
            (Some(aid), None) => self.pty_open_agent(aid, cols, rows, ladder),
            (None, Some(sid)) => self.pty_open_shell(sid, cols, rows),
            _ => Err(HostError::InvalidParams(
                "exactly one of agentId or shellId is required".into(),
            )),
        }
    }

    fn check_pty_size(cols: u16, rows: u16) -> Result<()> {
        if !(1..=500).contains(&cols) || !(1..=500).contains(&rows) {
            return Err(HostError::InvalidParams(
                "cols and rows must be in 1..=500".into(),
            ));
        }
        Ok(())
    }

    fn launch_args_for(&self, agent_id: &str) -> Result<Vec<String>> {
        let g = self
            .launch_args
            .lock()
            .map_err(|_| HostError::Internal("launch_args lock poisoned".into()))?;
        Ok(g.get(agent_id).cloned().unwrap_or_default())
    }

    fn agent_cwd(&self, agent: &Agent) -> Result<std::path::PathBuf> {
        if let Some(wt) = self.store.worktree_get_by_agent(&agent.id)? {
            return Ok(std::path::PathBuf::from(wt.path));
        }
        if !agent.task_id.is_empty() {
            let task = self.task_get(&agent.task_id)?;
            let ws_id = task
                .workspace_ids
                .first()
                .ok_or_else(|| HostError::Internal("task has no workspace".into()))?;
            let ws = self
                .store
                .workspace_get(ws_id)?
                .ok_or_else(|| HostError::NotFound(format!("workspace {ws_id}")))?;
            return Ok(std::path::PathBuf::from(ws.path));
        }
        let ws_id = agent
            .workspace_id
            .as_deref()
            .ok_or_else(|| HostError::InvalidParams("agent is not bound to a workspace".into()))?;
        let ws = self
            .store
            .workspace_get(ws_id)?
            .ok_or_else(|| HostError::NotFound(format!("workspace {ws_id}")))?;
        Ok(std::path::PathBuf::from(ws.path))
    }

    fn shell_cwd(
        &self,
        task_id: Option<&str>,
        workspace_id: &str,
        worktree_id: Option<&str>,
    ) -> Result<std::path::PathBuf> {
        if let Some(task_id) = task_id {
            let task = self.task_get(task_id)?;
            if !task.workspace_ids.iter().any(|id| id == workspace_id) {
                return Err(HostError::InvalidParams(
                    "workspaceId is not attached to this task".into(),
                ));
            }
        } else if self.store.workspace_get(workspace_id)?.is_none() {
            return Err(HostError::NotFound(format!("workspace {workspace_id}")));
        }
        if let Some(wt_id) = worktree_id {
            let wt = self
                .store
                .worktree_get(wt_id)?
                .ok_or_else(|| HostError::NotFound(format!("worktree {wt_id}")))?;
            if wt.workspace_id != workspace_id {
                return Err(HostError::InvalidParams(
                    "worktreeId does not belong to workspace".into(),
                ));
            }
            return Ok(std::path::PathBuf::from(wt.path));
        }
        let ws = self
            .store
            .workspace_get(workspace_id)?
            .ok_or_else(|| HostError::NotFound(format!("workspace {workspace_id}")))?;
        Ok(std::path::PathBuf::from(ws.path))
    }

    fn pty_open_ok(session: &crate::mux::PtySession, resumed: bool) -> Result<serde_json::Value> {
        Ok(serde_json::to_value(rt_protocol::PtyOpenOk {
            pty_id: session.pty_id.clone(),
            resumed,
        })?)
    }

    fn pty_open_shell(&self, shell_id: &str, _cols: u16, _rows: u16) -> Result<serde_json::Value> {
        match self.mux.live_for_entity(PtyKind::Shell, shell_id) {
            Ok(Some(s)) => Self::pty_open_ok(&s, false),
            Ok(None) => Err(HostError::NotFound(format!("shell {shell_id}"))),
            Err(e) => Err(HostError::Internal(e)),
        }
    }

    fn pty_open_agent(
        &self,
        agent_id: &str,
        cols: u16,
        rows: u16,
        ladder: bool,
    ) -> Result<serde_json::Value> {
        let _gate = self
            .turn_gate
            .lock()
            .map_err(|_| HostError::Internal("turn_gate poisoned".into()))?;
        let agent = self
            .store
            .agent_get(agent_id)?
            .ok_or_else(|| HostError::NotFound(format!("agent {agent_id}")))?;
        if agent.interface != "terminal" {
            return Err(HostError::InvalidParams(
                "pty.open agentId requires interface=terminal".into(),
            ));
        }
        if let Some(live) = self
            .mux
            .live_for_entity(PtyKind::Agent, agent_id)
            .map_err(HostError::Internal)?
        {
            return Self::pty_open_ok(&live, false);
        }
        if self.has_pending_locked(agent_id)? {
            return Err(HostError::AgentBusy);
        }
        if ladder {
            let view = self.resolve_policy_for_agent(agent_id)?;
            if !view.yolo && view.mode == PolicyMode::Deny {
                return Err(HostError::Denied);
            }
            if !view.yolo && view.mode == PolicyMode::Ask {
                let approval_id = rt_storage::new_id();
                let summary = format!("spawn pty {}", agent.provider);
                self.store_pending(PendingApproval {
                    approval_id: approval_id.clone(),
                    agent_id: agent_id.to_string(),
                    task_id: agent.task_id.clone(),
                    kind: "exec".into(),
                    summary: summary.clone(),
                    payload: PendingKind::PtyOpen { cols, rows },
                })?;
                let _ = self.events.send(WsEvent::agent_approval(
                    &approval_id,
                    agent_id,
                    &agent.task_id,
                    "exec",
                    &summary,
                ));
                return Ok(serde_json::json!({ "approvalId": approval_id }));
            }
        }
        let session = self.spawn_agent_pty(agent_id, cols, rows)?;
        Self::pty_open_ok(&session, session_resumed_flag(&agent, &self.backends))
    }

    fn spawn_agent_pty(
        &self,
        agent_id: &str,
        cols: u16,
        rows: u16,
    ) -> Result<crate::mux::PtySession> {
        if let Some(live) = self
            .mux
            .live_for_entity(PtyKind::Agent, agent_id)
            .map_err(HostError::Internal)?
        {
            return Ok(live);
        }
        let agent = self
            .store
            .agent_get(agent_id)?
            .ok_or_else(|| HostError::NotFound(format!("agent {agent_id}")))?;
        if agent.interface != "terminal" {
            return Err(HostError::InvalidParams(
                "pty.open agentId requires interface=terminal".into(),
            ));
        }
        let backend = self.backends.get(agent.provider.as_str());
        let caps = backend.map(|b| b.caps());
        if !caps.as_ref().map(|c| c.pty).unwrap_or(false) {
            return Err(HostError::NotPty);
        }
        let cwd = self.agent_cwd(&agent)?;
        let launch_args = self.launch_args_for(agent_id)?;
        let (program, args) = pty::agent_pty_command(&launch_args);
        let mut env = Vec::new();
        let resumed = agent.provider_session_id.is_some()
            && caps.as_ref().map(|c| c.session_resume).unwrap_or(false);
        if resumed {
            if let Some(sid) = &agent.provider_session_id {
                env.push(("RUSTTRAYCER_PROVIDER_SESSION_ID".into(), sid.clone()));
            }
        }
        if let Some(acc_id) = agent.account_id.as_deref() {
            if let Some(acc) = self.store.account_get(acc_id)? {
                if let Some((k, v)) = account_secret_from_env(&acc.provider, &acc.label) {
                    env.push((k, v));
                }
            }
        }
        let session = self.spawn_into_mux(&SpawnInto {
            kind: PtyKind::Agent,
            entity_id: agent_id,
            task_id: &agent.task_id,
            workspace_id: agent.workspace_id.as_deref().unwrap_or(""),
            cwd: cwd.to_string_lossy().as_ref(),
            program,
            args,
            cols,
            rows,
            env,
        })?;
        if let Err(e) = self.store.agent_set_status(agent_id, AgentStatus::Running) {
            tracing::warn!(agent_id, error = %e, "pty spawn status");
        }
        let _ = self.events.send(WsEvent::agent_status(
            &agent.task_id,
            agent_id,
            AgentStatus::Running,
        ));
        if agent.provider_session_id.is_none() {
            let sid = rt_storage::new_id();
            self.store.agent_set_provider_session_id(agent_id, &sid)?;
        }
        Ok(session)
    }

    fn spawn_into_mux(&self, req: &SpawnInto<'_>) -> Result<crate::mux::PtySession> {
        let pty_id = rt_storage::new_id();
        let events = self.events.clone();
        let events_exit = self.events.clone();
        let mux = std::sync::Arc::clone(&self.mux);
        let store = self.store.clone();
        let task_id_data = req.task_id.to_string();
        let task_id_exit = req.task_id.to_string();
        let entity_s = req.entity_id.to_string();
        let kind_copy = req.kind;
        let pty_id_data = pty_id.clone();
        let pty_id_exit = pty_id.clone();
        let spec = SpawnSpec {
            program: req.program.clone(),
            args: req.args.clone(),
            cwd: std::path::PathBuf::from(req.cwd),
            cols: req.cols,
            rows: req.rows,
            env: req.env.clone(),
        };
        let handle = pty::spawn(
            &spec,
            move |bytes| {
                let _ = events.send(WsEvent::pty_data(&task_id_data, &pty_id_data, &bytes));
            },
            move |code| {
                if let Err(e) = mux.remove_if_present(&pty_id_exit) {
                    tracing::warn!(error = %e, "mux remove on exit");
                }
                if kind_copy == PtyKind::Agent {
                    if let Err(e) = store.agent_set_status(&entity_s, AgentStatus::Idle) {
                        tracing::warn!(error = %e, "agent idle on pty.exit");
                    }
                    let _ = events_exit.send(WsEvent::agent_status(
                        &task_id_exit,
                        &entity_s,
                        AgentStatus::Idle,
                    ));
                }
                let _ =
                    events_exit.send(WsEvent::pty_exit(&task_id_exit, &pty_id_exit, code as i32));
            },
        )
        .map_err(HostError::Internal)?;
        let session = crate::mux::PtySession {
            pty_id,
            kind: req.kind,
            entity_id: req.entity_id.to_string(),
            pid: handle.pid,
            cols: req.cols,
            rows: req.rows,
            task_id: req.task_id.to_string(),
            workspace_id: req.workspace_id.to_string(),
            cwd: req.cwd.to_string(),
        };
        self.mux
            .insert(session, handle)
            .map_err(HostError::Internal)
    }

    pub fn pty_write(&self, pty_id: &str, data_b64: &str) -> Result<serde_json::Value> {
        use base64::Engine;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(data_b64)
            .map_err(|e| HostError::InvalidParams(format!("data must be base64: {e}")))?;
        if decoded.len() > 64 * 1024 {
            return Err(HostError::InvalidParams(
                "pty.write data exceeds 64 KiB decoded".into(),
            ));
        }
        match self.mux.write(pty_id, &decoded) {
            Ok(()) => Ok(serde_json::json!({})),
            Err(MuxIoError::Dead) => Err(HostError::PtyDead),
            Err(MuxIoError::Internal(e)) => Err(HostError::Internal(e)),
        }
    }

    pub fn pty_resize(&self, pty_id: &str, cols: u16, rows: u16) -> Result<serde_json::Value> {
        Self::check_pty_size(cols, rows)?;
        match self.mux.resize(pty_id, cols, rows) {
            Ok(()) => Ok(serde_json::json!({})),
            Err(MuxIoError::Dead) => Err(HostError::PtyDead),
            Err(MuxIoError::Internal(e)) => Err(HostError::Internal(e)),
        }
    }

    pub fn pty_close(&self, pty_id: &str) -> Result<serde_json::Value> {
        let session = self.mux.kill(pty_id).map_err(HostError::Internal)?;
        let Some(session) = session else {
            return Err(HostError::PtyDead);
        };
        if session.kind == PtyKind::Agent {
            if let Err(e) = self
                .store
                .agent_set_status(&session.entity_id, AgentStatus::Idle)
            {
                tracing::warn!(error = %e, "pty.close status");
            }
            let _ = self.events.send(WsEvent::agent_status(
                &session.task_id,
                &session.entity_id,
                AgentStatus::Idle,
            ));
        }
        Ok(serde_json::json!({}))
    }

    pub fn shell_create(
        &self,
        task_id: Option<&str>,
        workspace_id: &str,
        worktree_id: Option<&str>,
        cols: u16,
        rows: u16,
    ) -> Result<serde_json::Value> {
        let task_id = task_id.filter(|s| !s.is_empty());
        if workspace_id.is_empty() {
            return Err(HostError::InvalidParams(
                "workspaceId is required when taskId is omitted".into(),
            ));
        }
        if task_id.is_none() && self.store.workspace_get(workspace_id)?.is_none() {
            return Err(HostError::NotFound(format!("workspace {workspace_id}")));
        }
        Self::check_pty_size(cols, rows)?;
        let cwd = self.shell_cwd(task_id, workspace_id, worktree_id)?;
        let (program, args) = pty::shell_pty_command();
        let shell_id = rt_storage::new_id();
        let bound_task = task_id.unwrap_or("");
        let session = self.spawn_into_mux(&SpawnInto {
            kind: PtyKind::Shell,
            entity_id: &shell_id,
            task_id: bound_task,
            workspace_id,
            cwd: cwd.to_string_lossy().as_ref(),
            program,
            args,
            cols,
            rows,
            env: Vec::new(),
        })?;
        Ok(serde_json::to_value(rt_protocol::ShellCreateOk {
            shell_id,
            pty_id: session.pty_id,
            cwd: session.cwd,
        })?)
    }

    pub fn shell_list(
        &self,
        task_id: Option<&str>,
        workspace_id: Option<&str>,
    ) -> Result<serde_json::Value> {
        let task_id = task_id.filter(|s| !s.is_empty());
        let workspace_id = workspace_id.filter(|s| !s.is_empty());
        let items = match (task_id, workspace_id) {
            (Some(task_id), _) => {
                if self.store.task_get(task_id)?.is_none() {
                    return Err(HostError::NotFound(format!("task {task_id}")));
                }
                self.mux.list_shells(task_id).map_err(HostError::Internal)?
            }
            (None, Some(workspace_id)) => {
                if self.store.workspace_get(workspace_id)?.is_none() {
                    return Err(HostError::NotFound(format!("workspace {workspace_id}")));
                }
                self.mux
                    .list_shells_for_workspace(workspace_id)
                    .map_err(HostError::Internal)?
            }
            (None, None) => {
                return Err(HostError::InvalidParams(
                    "workspaceId is required when taskId is omitted".into(),
                ));
            }
        };
        let items: Vec<rt_protocol::ShellListItem> = items
            .into_iter()
            .map(|s| rt_protocol::ShellListItem {
                shell_id: s.entity_id,
                pty_id: s.pty_id,
                cwd: s.cwd,
            })
            .collect();
        Ok(serde_json::json!({ "items": items }))
    }

    pub fn shell_close(&self, shell_id: &str) -> Result<serde_json::Value> {
        let session = self
            .mux
            .kill_entity(PtyKind::Shell, shell_id)
            .map_err(HostError::Internal)?;
        if session.is_none() {
            return Err(HostError::NotFound(format!("shell {shell_id}")));
        }
        Ok(serde_json::json!({}))
    }

    pub fn artifact_create(
        &self,
        params: &rt_protocol::ArtifactCreateParams,
    ) -> Result<rt_storage::Artifact> {
        let assignee = params
            .assignee
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let parent = params
            .parent_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let source = params
            .source_message_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let art = self
            .store
            .artifact_create(rt_storage::ArtifactCreateInput {
                task_id: &params.task_id,
                parent_id: parent,
                kind: &params.kind,
                title: &params.title,
                body: &params.body,
                assignee,
                source_message_id: source,
            })?;
        let _ = self
            .events
            .send(WsEvent::artifact_updated(&art.id, &art.task_id));
        Ok(art)
    }

    pub fn artifact_get(&self, artifact_id: &str) -> Result<rt_storage::Artifact> {
        self.store
            .artifact_get(artifact_id)?
            .ok_or_else(|| HostError::NotFound(format!("artifact {artifact_id}")))
    }

    pub fn artifact_list(
        &self,
        task_id: &str,
        kind: Option<&str>,
    ) -> Result<rt_protocol::ArtifactListOk> {
        if self.store.task_get(task_id)?.is_none() {
            return Err(HostError::NotFound(format!("task {task_id}")));
        }
        let (items, truncated) = self.store.artifact_list(task_id, kind)?;
        let items = items.into_iter().map(storage_artifact_to_wire).collect();
        Ok(rt_protocol::ArtifactListOk { items, truncated })
    }

    pub fn artifact_update(&self, params: &serde_json::Value) -> Result<rt_storage::Artifact> {
        let artifact_id = params
            .get("artifactId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| HostError::InvalidParams("artifactId is required".into()))?;
        let title = match params.get("title") {
            None => None,
            Some(v) => Some(
                v.as_str()
                    .ok_or_else(|| HostError::InvalidParams("title must be a string".into()))?,
            ),
        };
        let body = match params.get("body") {
            None => None,
            Some(v) => Some(
                v.as_str()
                    .ok_or_else(|| HostError::InvalidParams("body must be a string".into()))?,
            ),
        };
        let status = match params.get("status") {
            None => None,
            Some(serde_json::Value::Null) => {
                return Err(HostError::InvalidParams("status cannot be null".into()));
            }
            Some(v) => Some(
                v.as_str()
                    .ok_or_else(|| HostError::InvalidParams("status must be a string".into()))?,
            ),
        };
        let assignee = match params.get("assignee") {
            None => None,
            Some(serde_json::Value::Null) => Some(None),
            Some(v) => {
                let s = v
                    .as_str()
                    .ok_or_else(|| HostError::InvalidParams("assignee must be a string".into()))?;
                Some(if s.is_empty() { None } else { Some(s) })
            }
        };
        let parent_id = match params.get("parentId") {
            None => None,
            Some(serde_json::Value::Null) => Some(None),
            Some(v) => {
                let s = v
                    .as_str()
                    .ok_or_else(|| HostError::InvalidParams("parentId must be a string".into()))?;
                Some(Some(s))
            }
        };
        let art =
            self.store
                .artifact_update(artifact_id, title, body, status, assignee, parent_id)?;
        let _ = self
            .events
            .send(WsEvent::artifact_updated(&art.id, &art.task_id));
        Ok(art)
    }

    pub fn artifact_delete(&self, artifact_id: &str) -> Result<rt_protocol::ArtifactDeleteOk> {
        let art = self.artifact_get(artifact_id)?;
        let deleted = self.store.artifact_delete_tree(artifact_id)?;
        for id in &deleted {
            let _ = self
                .events
                .send(WsEvent::artifact_deleted(id, &art.task_id));
        }
        Ok(rt_protocol::ArtifactDeleteOk { deleted })
    }

    pub fn artifact_export(
        &self,
        artifact_id: &str,
        format: &str,
    ) -> Result<rt_protocol::ArtifactExportOk> {
        match format {
            "md" | "pdf" => {}
            other => {
                return Err(HostError::InvalidParams(format!(
                    "format must be md|pdf, got {other}"
                )));
            }
        }
        let art = self.artifact_get(artifact_id)?;
        let markdown = format!("{}\n\n{}", art.title, art.body);
        if format == "pdf" {
            let pdf = crate::pdf::render_markdown_pdf(&art.title, &art.body)
                .map_err(HostError::Internal)?;
            let bytes = String::from_utf8(pdf)
                .map_err(|e| HostError::Internal(format!("pdf is not utf-8: {e}")))?;
            return Ok(rt_protocol::ArtifactExportOk {
                format: "pdf".into(),
                markdown: String::new(),
                bytes,
                filename: format!("{}.pdf", art.id),
            });
        }
        Ok(rt_protocol::ArtifactExportOk {
            format: "md".into(),
            markdown,
            bytes: String::new(),
            filename: format!("{}.md", art.id),
        })
    }

    pub fn comment_create(
        &self,
        params: &rt_protocol::CommentCreateParams,
    ) -> Result<rt_storage::CommentThread> {
        let thread = if let Some(tid) = params
            .thread_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            self.store.comment_add(tid, &params.body)?
        } else {
            let start = params.anchor_start.ok_or_else(|| {
                HostError::InvalidParams("anchorStart is required for a new thread".into())
            })?;
            let end = params.anchor_end.ok_or_else(|| {
                HostError::InvalidParams("anchorEnd is required for a new thread".into())
            })?;
            self.store
                .comment_thread_create(&params.artifact_id, start, end, &params.body)?
        };
        if let Some(art) = self.store.artifact_get(&thread.artifact_id)? {
            let _ = self
                .events
                .send(WsEvent::artifact_updated(&art.id, &art.task_id));
        }
        Ok(thread)
    }

    pub fn comment_list(&self, artifact_id: &str) -> Result<rt_protocol::CommentListOk> {
        let threads = self.store.comment_list(artifact_id)?;
        let threads = threads.into_iter().map(storage_thread_to_wire).collect();
        Ok(rt_protocol::CommentListOk { threads })
    }

    pub fn comment_resolve(&self, thread_id: &str) -> Result<rt_storage::CommentThread> {
        let thread = self.store.comment_resolve(thread_id)?;
        if let Some(art) = self.store.artifact_get(&thread.artifact_id)? {
            let _ = self
                .events
                .send(WsEvent::artifact_updated(&art.id, &art.task_id));
        }
        Ok(thread)
    }

    pub fn clear_transcript(&self, agent_id: &str) -> Result<rt_protocol::ClearTranscriptOk> {
        let cleared = self.store.clear_transcript(agent_id)?;
        Ok(rt_protocol::ClearTranscriptOk { cleared })
    }

    pub fn a2a_transcript(&self, agent_id: &str) -> Result<rt_protocol::A2aTranscriptOk> {
        let agent = self
            .store
            .agent_get(agent_id)?
            .ok_or_else(|| HostError::NotFound(format!("agent {agent_id}")))?;
        if agent.host_id != self.host_id {
            return Err(HostError::CrossHost);
        }
        if agent.interface == "terminal" {
            return Err(HostError::Internal("vendor session unavailable".into()));
        }
        let messages = self
            .store
            .message_list(agent_id)?
            .into_iter()
            .map(storage_message_to_wire)
            .collect();
        Ok(rt_protocol::A2aTranscriptOk {
            agent_id: agent.id,
            interface: agent.interface,
            messages,
        })
    }

    pub fn a2a_deliver(
        &self,
        from_agent_id: &str,
        to_agent_id: &str,
        content: &str,
    ) -> Result<rt_protocol::A2aDeliverOk> {
        if content.is_empty() || content.len() > MAX_CONTENT {
            return Err(HostError::InvalidParams("content must be 1..=1 MiB".into()));
        }
        let from = self
            .store
            .agent_get(from_agent_id)?
            .ok_or_else(|| HostError::NotFound(format!("agent {from_agent_id}")))?;
        let to = self
            .store
            .agent_get(to_agent_id)?
            .ok_or_else(|| HostError::NotFound(format!("agent {to_agent_id}")))?;
        if from.task_id != to.task_id {
            return Err(HostError::InvalidParams(
                "from and to must belong to the same task".into(),
            ));
        }
        if from.host_id != to.host_id || to.host_id != self.host_id {
            return Err(HostError::CrossHost);
        }
        let backend = self.lookup_backend(&to)?;
        if !backend.caps().a2a_inbox {
            return Err(HostError::NoInbox);
        }
        let prefixed = format!("a2a:{from_agent_id}\n{content}");
        let msg = self
            .store
            .message_append(to_agent_id, MessageRole::System, &prefixed)?;
        let _ = self
            .events
            .send(WsEvent::a2a_delivered(from_agent_id, to_agent_id, &msg.id));
        Ok(rt_protocol::A2aDeliverOk {
            message_id: msg.id,
            to_agent_id: to.id,
        })
    }

    pub fn loop_start(
        &self,
        task_id: &str,
        agent_ids: &[String],
        max_iterations: u32,
        budget_turns: Option<u32>,
        prompt: &str,
    ) -> Result<rt_protocol::LoopStartOk> {
        if agent_ids.len() != 2 {
            return Err(HostError::InvalidParams(
                "agentIds must contain exactly 2 ids".into(),
            ));
        }
        if agent_ids[0] == agent_ids[1] {
            return Err(HostError::InvalidParams(
                "agentIds must be two different agents".into(),
            ));
        }
        if !(1..=32).contains(&max_iterations) {
            return Err(HostError::InvalidParams(
                "maxIterations must be 1..32".into(),
            ));
        }
        let budget = match budget_turns {
            None => (max_iterations.saturating_mul(2)).min(64),
            Some(b) if (1..=64).contains(&b) => b,
            Some(_) => return Err(HostError::InvalidParams("budgetTurns must be 1..64".into())),
        };
        if prompt.is_empty() {
            return Err(HostError::InvalidParams("prompt is required".into()));
        }
        if self.store.task_get(task_id)?.is_none() {
            return Err(HostError::NotFound(format!("task {task_id}")));
        }
        let a = self
            .store
            .agent_get(&agent_ids[0])?
            .ok_or_else(|| HostError::NotFound(format!("agent {}", agent_ids[0])))?;
        let b = self
            .store
            .agent_get(&agent_ids[1])?
            .ok_or_else(|| HostError::NotFound(format!("agent {}", agent_ids[1])))?;
        if a.task_id != task_id || b.task_id != task_id {
            return Err(HostError::InvalidParams(
                "both agents must belong to the task".into(),
            ));
        }
        let row = self.store.loop_insert(
            task_id,
            &a.id,
            &b.id,
            i64::from(max_iterations),
            i64::from(budget),
            prompt,
        )?;
        self.spawn_loop_runner(row.id.clone());
        Ok(rt_protocol::LoopStartOk {
            loop_id: row.id,
            iteration: 0,
            turns: 0,
            max_iterations,
            budget_turns: budget,
        })
    }

    pub fn loop_get(&self, loop_id: &str) -> Result<rt_protocol::LoopView> {
        let row = self
            .store
            .loop_get(loop_id)?
            .ok_or_else(|| HostError::NotFound(format!("loop {loop_id}")))?;
        Ok(loop_row_to_view(&row))
    }

    pub fn loop_stop(&self, loop_id: &str) -> Result<rt_protocol::LoopView> {
        let before = self
            .store
            .loop_get(loop_id)?
            .ok_or_else(|| HostError::NotFound(format!("loop {loop_id}")))?;
        let row = self.store.loop_stop(loop_id, "stop")?;
        if before.status == "running" {
            let _ = self.events.send(WsEvent::loop_stopped(loop_id, "stop"));
        }
        Ok(loop_row_to_view(&row))
    }

    fn spawn_loop_runner(&self, loop_id: String) {
        let svc = self.clone();
        tokio::spawn(async move {
            if let Err(e) = svc.run_loop(&loop_id).await {
                tracing::warn!(loop_id, error = %e, "loop runner failed");
                if let Err(stop_err) = svc.finish_loop(&loop_id, "error") {
                    tracing::warn!(loop_id, error = %stop_err, "loop stop after error");
                }
            }
        });
    }

    async fn run_loop(&self, loop_id: &str) -> Result<()> {
        let row = self
            .store
            .loop_get(loop_id)?
            .ok_or_else(|| HostError::NotFound(format!("loop {loop_id}")))?;
        let agents = [row.agent_a.clone(), row.agent_b.clone()];
        let mut idx = 0usize;
        let mut first = true;
        let mut last_text = row.prompt.clone();
        loop {
            let row = match self.store.loop_get(loop_id)? {
                Some(r) => r,
                None => return Ok(()),
            };
            if row.status != "running" {
                return Ok(());
            }
            if row.iteration >= row.max_iterations {
                return self.finish_loop(loop_id, "max_iterations");
            }
            if row.turns >= row.budget_turns {
                return self.finish_loop(loop_id, "budget");
            }
            let agent_id = &agents[idx];
            let content = if first {
                row.prompt.clone()
            } else {
                last_text.clone()
            };
            first = false;
            self.wait_agent_idle(agent_id).await?;
            let row = match self.store.loop_get(loop_id)? {
                Some(r) => r,
                None => return Ok(()),
            };
            if row.status != "running" {
                return Ok(());
            }
            match self.send(agent_id, &content) {
                Ok(_) => {}
                Err(HostError::Denied) => return self.finish_loop(loop_id, "denied"),
                Err(HostError::LoopExhausted) => {
                    return self.finish_loop(loop_id, "max_iterations")
                }
                Err(e) => {
                    tracing::info!(loop_id, error = %e, "loop send failed");
                    return self.finish_loop(loop_id, "error");
                }
            }
            self.wait_agent_idle(agent_id).await?;
            last_text = self
                .last_assistant_text(agent_id)?
                .unwrap_or_else(|| content.clone());
            let new_iter = row.iteration + 1;
            let new_turns = row.turns + 1;
            self.store
                .loop_update_progress(loop_id, new_iter, new_turns)?;
            if new_iter >= row.max_iterations {
                return self.finish_loop(loop_id, "max_iterations");
            }
            if new_turns >= row.budget_turns {
                return self.finish_loop(loop_id, "budget");
            }
            idx = 1 - idx;
        }
    }

    fn finish_loop(&self, loop_id: &str, reason: &str) -> Result<()> {
        let before = self.store.loop_get(loop_id)?;
        let row = self.store.loop_stop(loop_id, reason)?;
        if before
            .as_ref()
            .map(|r| r.status == "running")
            .unwrap_or(false)
        {
            let _ = self.events.send(WsEvent::loop_stopped(
                loop_id,
                &row.reason.clone().unwrap_or_else(|| reason.to_string()),
            ));
        }
        Ok(())
    }

    async fn wait_agent_idle(&self, agent_id: &str) -> Result<()> {
        for _ in 0..200 {
            let agent = self
                .store
                .agent_get(agent_id)?
                .ok_or_else(|| HostError::NotFound(format!("agent {agent_id}")))?;
            let busy = agent.status == AgentStatus::Running || self.inflight.contains(agent_id)?;
            if !busy {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        Err(HostError::Internal(format!(
            "loop timed out waiting for agent {agent_id}"
        )))
    }

    fn last_assistant_text(&self, agent_id: &str) -> Result<Option<String>> {
        let msgs = self.store.message_list(agent_id)?;
        Ok(msgs
            .into_iter()
            .rev()
            .find(|m| m.role == MessageRole::Assistant)
            .map(|m| m.content))
    }
}

fn storage_message_to_wire(m: Message) -> rt_protocol::Message {
    rt_protocol::Message {
        id: m.id,
        agent_id: m.agent_id,
        role: m.role.as_str().to_string(),
        content: m.content,
        created_at: m.created_at,
    }
}

fn loop_row_to_view(row: &rt_storage::LoopRow) -> rt_protocol::LoopView {
    rt_protocol::LoopView {
        loop_id: row.id.clone(),
        iteration: u32::try_from(row.iteration).unwrap_or(0),
        turns: u32::try_from(row.turns).unwrap_or(0),
        max_iterations: u32::try_from(row.max_iterations).unwrap_or(0),
        budget_turns: u32::try_from(row.budget_turns).unwrap_or(0),
        status: row.status.clone(),
        reason: row.reason.clone(),
    }
}

fn storage_artifact_to_wire(a: rt_storage::Artifact) -> rt_protocol::Artifact {
    rt_protocol::Artifact {
        id: a.id,
        task_id: a.task_id,
        parent_id: a.parent_id,
        kind: a.kind,
        title: a.title,
        body: a.body,
        status: a.status,
        assignee: a.assignee,
        source_message_id: a.source_message_id,
        created_at: a.created_at,
        updated_at: a.updated_at,
    }
}

fn storage_thread_to_wire(t: rt_storage::CommentThread) -> rt_protocol::CommentThread {
    rt_protocol::CommentThread {
        id: t.id,
        artifact_id: t.artifact_id,
        anchor_start: t.anchor_start,
        anchor_end: t.anchor_end,
        resolved: t.resolved,
        comments: t
            .comments
            .into_iter()
            .map(|c| rt_protocol::Comment {
                id: c.id,
                body: c.body,
                created_at: c.created_at,
            })
            .collect(),
        created_at: t.created_at,
        updated_at: t.updated_at,
    }
}

fn session_resumed_flag(agent: &Agent, backends: &HashMap<String, Arc<dyn AgentBackend>>) -> bool {
    agent.provider_session_id.is_some()
        && backends
            .get(agent.provider.as_str())
            .map(|b| b.caps().session_resume)
            .unwrap_or(false)
}

struct SpawnInto<'a> {
    kind: PtyKind,
    entity_id: &'a str,
    task_id: &'a str,
    workspace_id: &'a str,
    cwd: &'a str,
    program: String,
    args: Vec<String>,
    cols: u16,
    rows: u16,
    env: Vec<(String, String)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::Stream;
    use rt_runtime::{Availability, TurnEvent};
    use std::pin::Pin;
    use std::time::Duration;
    use tempfile::tempdir;

    struct SlowBackend;

    impl AgentBackend for SlowBackend {
        fn id(&self) -> &'static str {
            "cli.generic"
        }
        fn available(&self) -> Availability {
            Availability {
                available: true,
                detail: "slow".into(),
            }
        }
        fn start_turn(&self, _req: TurnRequest) -> Pin<Box<dyn Stream<Item = TurnEvent> + Send>> {
            Box::pin(futures::stream::once(async {
                tokio::time::sleep(Duration::from_secs(30)).await;
                TurnEvent::Finished { exit_code: 0 }
            }))
        }
    }

    struct DownBackend;

    impl AgentBackend for DownBackend {
        fn id(&self) -> &'static str {
            "cli.generic"
        }
        fn available(&self) -> Availability {
            Availability {
                available: false,
                detail: "RUSTTRAYCER_GENERIC_CMD unset".into(),
            }
        }
        fn start_turn(&self, _req: TurnRequest) -> Pin<Box<dyn Stream<Item = TurnEvent> + Send>> {
            Box::pin(futures::stream::once(async {
                TurnEvent::Failed {
                    message: "should not run".into(),
                }
            }))
        }
    }

    fn setup_with(backend: Arc<dyn AgentBackend>) -> (tempfile::TempDir, HostService) {
        let dir = tempdir().unwrap();
        let store = Store::open(dir.path().join("host.db")).unwrap();
        store.migrate().unwrap();
        let host_id = rt_storage::new_id();
        store.host_insert_if_absent(&host_id, "test").unwrap();
        let mut backends: HashMap<String, Arc<dyn AgentBackend>> = HashMap::new();
        backends.insert(backend.id().to_string(), backend);
        let svc = HostService::new(
            store,
            backends,
            host_id,
            dir.path().to_path_buf(),
            "http://127.0.0.1:0".into(),
            std::process::id(),
        );
        (dir, svc)
    }

    struct InstantBackend;

    impl AgentBackend for InstantBackend {
        fn id(&self) -> &'static str {
            "cli.generic"
        }
        fn available(&self) -> Availability {
            Availability {
                available: true,
                detail: "instant".into(),
            }
        }
        fn start_turn(&self, _req: TurnRequest) -> Pin<Box<dyn Stream<Item = TurnEvent> + Send>> {
            Box::pin(futures::stream::once(async {
                TurnEvent::Finished { exit_code: 0 }
            }))
        }
    }

    /// First turn finishes after a short delay (survives cancel because
    /// `cancel_turn` is the default no-op). Later turns stay Running.
    struct CountingBackend {
        turns: std::sync::atomic::AtomicUsize,
    }

    impl AgentBackend for CountingBackend {
        fn id(&self) -> &'static str {
            "cli.generic"
        }
        fn available(&self) -> Availability {
            Availability {
                available: true,
                detail: "counting".into(),
            }
        }
        fn start_turn(&self, _req: TurnRequest) -> Pin<Box<dyn Stream<Item = TurnEvent> + Send>> {
            let n = self.turns.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if n == 0 {
                Box::pin(futures::stream::once(async {
                    tokio::time::sleep(Duration::from_millis(180)).await;
                    TurnEvent::Finished { exit_code: 0 }
                }))
            } else {
                Box::pin(futures::stream::once(async {
                    tokio::time::sleep(Duration::from_secs(30)).await;
                    TurnEvent::Finished { exit_code: 0 }
                }))
            }
        }
    }

    fn seed_agent(svc: &HostService, dir: &tempfile::TempDir) -> String {
        let ws_dir = dir.path().join("ws");
        std::fs::create_dir(&ws_dir).unwrap();
        let ws = svc.workspace_add(ws_dir.to_str().unwrap()).unwrap();
        let task = svc.task_create("t", &ws.id).unwrap();
        let agent = svc.agent_create(&task.id, Some("cli.generic")).unwrap();
        agent.id
    }

    #[tokio::test]
    async fn send_while_running_is_agent_busy() {
        let (dir, svc) = setup_with(Arc::new(SlowBackend));
        let agent_id = seed_agent(&svc, &dir);
        let first = svc.send(&agent_id, "hello").unwrap();
        assert_eq!(first.role, MessageRole::User);
        assert_eq!(first.content, "hello");
        let err = svc.send(&agent_id, "again").unwrap_err();
        assert_eq!(err.code(), "agent_busy");
        let agent = svc.agent_get(&agent_id).unwrap();
        assert_eq!(agent.status, AgentStatus::Running);
        svc.inflight.abort_all();
    }

    #[tokio::test]
    async fn send_after_instant_finish_is_not_stuck_busy() {
        let (dir, svc) = setup_with(Arc::new(InstantBackend));
        let agent_id = seed_agent(&svc, &dir);
        let first = svc.send(&agent_id, "hello").unwrap();
        assert_eq!(first.role, MessageRole::User);
        assert_eq!(first.content, "hello");
        tokio::time::sleep(Duration::from_millis(50)).await;
        let second = svc
            .send(&agent_id, "again")
            .expect("second send after instant finish");
        assert_eq!(second.role, MessageRole::User);
        assert_eq!(second.content, "again");
        svc.inflight.abort_all();
    }

    #[tokio::test]
    async fn concurrent_send_one_ok_one_busy() {
        let (dir, svc) = setup_with(Arc::new(SlowBackend));
        let agent_id = seed_agent(&svc, &dir);
        let svc_a = svc.clone();
        let svc_b = svc.clone();
        let id_a = agent_id.clone();
        let id_b = agent_id.clone();
        let a = tokio::task::spawn_blocking(move || svc_a.send(&id_a, "one"));
        let b = tokio::task::spawn_blocking(move || svc_b.send(&id_b, "two"));
        let (ra, rb) = tokio::join!(a, b);
        let ra = ra.expect("join a");
        let rb = rb.expect("join b");
        let oks = [&ra, &rb].iter().filter(|r| r.is_ok()).count();
        let busy = [&ra, &rb]
            .iter()
            .filter(|r| matches!(r, Err(e) if e.code() == "agent_busy"))
            .count();
        assert_eq!(oks, 1, "exactly one send should succeed: {ra:?} {rb:?}");
        assert_eq!(
            busy, 1,
            "exactly one send should be agent_busy: {ra:?} {rb:?}"
        );
        let users: Vec<_> = svc
            .get_context(&agent_id)
            .unwrap()
            .into_iter()
            .filter(|m| m.role == MessageRole::User)
            .collect();
        assert_eq!(users.len(), 1, "exactly one user message in transcript");
        svc.inflight.abort_all();
    }

    #[test]
    fn create_allowed_when_unavailable_send_fails_internal() {
        let (dir, svc) = setup_with(Arc::new(DownBackend));
        let agent_id = seed_agent(&svc, &dir);
        let err = svc.send(&agent_id, "hello").unwrap_err();
        assert_eq!(err.code(), "internal");
        assert!(err.to_string().contains("RUSTTRAYCER_GENERIC_CMD unset"));
        // user message must not have been written
        assert!(svc.get_context(&agent_id).unwrap().is_empty());
    }

    #[test]
    fn restart_recovery_sets_running_to_error() {
        let (dir, svc) = setup_with(Arc::new(SlowBackend));
        let agent_id = seed_agent(&svc, &dir);
        svc.store
            .agent_set_status(&agent_id, AgentStatus::Running)
            .unwrap();
        let n = svc.store.set_running_agents_to_error().unwrap();
        assert_eq!(n, 1);
        assert_eq!(
            svc.store.agent_get(&agent_id).unwrap().unwrap().status,
            AgentStatus::Error
        );
    }

    #[test]
    fn provider_is_harness_not_interface_or_shell() {
        let (dir, svc) = setup_with(Arc::new(SlowBackend));
        let ws_dir = dir.path().join("ws2");
        std::fs::create_dir(&ws_dir).unwrap();
        let ws = svc.workspace_add(ws_dir.to_str().unwrap()).unwrap();
        let task = svc.task_create("t", &ws.id).unwrap();
        assert_eq!(
            svc.agent_create(&task.id, Some("chat")).unwrap_err().code(),
            "invalid_params"
        );
        assert_eq!(
            svc.agent_create(&task.id, Some("pty")).unwrap_err().code(),
            "invalid_params"
        );
        let agent = svc.agent_create(&task.id, Some("cli.generic")).unwrap();
        assert_eq!(agent.provider, HarnessId::cli_generic());
        assert_eq!(agent.interface, "chat");
        assert_eq!(agent.status, AgentStatus::Idle);
        let v = serde_json::to_value(&agent).unwrap();
        assert_eq!(v["provider"], "cli.generic");
        assert_eq!(v["interface"], "chat");
    }

    #[test]
    fn create_cli_claude_is_known_harness() {
        let (dir, svc) = setup_with(Arc::new(SlowBackend));
        let ws_dir = dir.path().join("ws-claude");
        std::fs::create_dir(&ws_dir).unwrap();
        let ws = svc.workspace_add(ws_dir.to_str().unwrap()).unwrap();
        let task = svc.task_create("t", &ws.id).unwrap();
        let agent = svc.agent_create(&task.id, Some("cli.claude")).unwrap();
        assert_eq!(agent.provider.as_str(), "cli.claude");
        assert_eq!(agent.interface, "chat");
        assert_eq!(agent.status, AgentStatus::Idle);
        assert_eq!(
            svc.agent_create(&task.id, Some("unknown"))
                .unwrap_err()
                .code(),
            "invalid_params"
        );
    }

    #[test]
    fn create_cli_codex_is_known_harness() {
        let (dir, svc) = setup_with(Arc::new(SlowBackend));
        let ws_dir = dir.path().join("ws-codex");
        std::fs::create_dir(&ws_dir).unwrap();
        let ws = svc.workspace_add(ws_dir.to_str().unwrap()).unwrap();
        let task = svc.task_create("t", &ws.id).unwrap();
        let agent = svc.agent_create(&task.id, Some("cli.codex")).unwrap();
        assert_eq!(agent.provider.as_str(), "cli.codex");
        assert_eq!(agent.interface, "chat");
        assert_eq!(agent.status, AgentStatus::Idle);
        assert_eq!(
            svc.agent_create(&task.id, Some("unknown"))
                .unwrap_err()
                .code(),
            "invalid_params"
        );
    }

    #[test]
    fn send_rejects_content_over_1_mib() {
        let (dir, svc) = setup_with(Arc::new(InstantBackend));
        let agent_id = seed_agent(&svc, &dir);
        let content = "x".repeat(MAX_CONTENT + 1);
        let err = svc.send(&agent_id, &content).unwrap_err();
        assert_eq!(err.code(), "invalid_params");
        assert!(err.to_string().contains("1 MiB"));
    }

    #[tokio::test]
    async fn turn_timeout_marks_agent_error() {
        let (dir, mut svc) = setup_with(Arc::new(SlowBackend));
        svc.set_turn_timeout(Duration::from_millis(30));
        let agent_id = seed_agent(&svc, &dir);
        svc.send(&agent_id, "hello").unwrap();
        let mut timed_out = false;
        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(20)).await;
            if svc.agent_get(&agent_id).unwrap().status == AgentStatus::Error {
                timed_out = true;
                break;
            }
        }
        assert!(
            timed_out,
            "agent did not enter Error after short turn timeout"
        );
        svc.inflight.abort_all();
    }

    #[tokio::test]
    async fn cancel_inflight_sets_idle_keeps_user_message() {
        let (dir, svc) = setup_with(Arc::new(SlowBackend));
        let agent_id = seed_agent(&svc, &dir);
        let user = svc.send(&agent_id, "hello").unwrap();
        assert_eq!(user.role, MessageRole::User);
        let result = svc.cancel(&agent_id).unwrap();
        assert_eq!(result.agent_id, agent_id);
        assert!(result.cancelled);
        let v = serde_json::to_value(&result).unwrap();
        assert_eq!(v["agentId"], agent_id);
        assert_eq!(v["cancelled"], true);
        let agent = svc.agent_get(&agent_id).unwrap();
        assert_eq!(agent.status, AgentStatus::Idle);
        let msgs = svc.get_context(&agent_id).unwrap();
        assert!(msgs
            .iter()
            .any(|m| m.role == MessageRole::User && m.content == "hello"));
        svc.inflight.abort_all();
    }

    #[tokio::test]
    async fn cancel_idle_is_false_no_status_change() {
        let (dir, svc) = setup_with(Arc::new(SlowBackend));
        let agent_id = seed_agent(&svc, &dir);
        let before = svc.agent_get(&agent_id).unwrap();
        assert_eq!(before.status, AgentStatus::Idle);
        let result = svc.cancel(&agent_id).unwrap();
        assert!(!result.cancelled);
        assert_eq!(result.agent_id, agent_id);
        let after = svc.agent_get(&agent_id).unwrap();
        assert_eq!(after.status, AgentStatus::Idle);
        assert!(svc.get_context(&agent_id).unwrap().is_empty());
        svc.inflight.abort_all();
    }

    #[tokio::test]
    async fn send_after_cancel_is_allowed() {
        let (dir, svc) = setup_with(Arc::new(SlowBackend));
        let agent_id = seed_agent(&svc, &dir);
        svc.send(&agent_id, "hello").unwrap();
        let first = svc.cancel(&agent_id).unwrap();
        assert!(first.cancelled);
        let second = svc
            .send(&agent_id, "again")
            .expect("send after cancel should be allowed");
        assert_eq!(second.role, MessageRole::User);
        assert_eq!(second.content, "again");
        svc.inflight.abort_all();
    }

    #[tokio::test]
    async fn cancel_then_send_old_stream_does_not_clobber_new_turn() {
        let backend = CountingBackend {
            turns: std::sync::atomic::AtomicUsize::new(0),
        };
        let (dir, svc) = setup_with(Arc::new(backend));
        let agent_id = seed_agent(&svc, &dir);

        svc.send(&agent_id, "hello").unwrap();
        assert_eq!(
            svc.agent_get(&agent_id).unwrap().status,
            AgentStatus::Running
        );

        let result = svc.cancel(&agent_id).unwrap();
        assert!(result.cancelled);
        assert_eq!(svc.agent_get(&agent_id).unwrap().status, AgentStatus::Idle);

        svc.send(&agent_id, "again").unwrap();
        assert_eq!(
            svc.agent_get(&agent_id).unwrap().status,
            AgentStatus::Running
        );

        // Old stream still completes (~180ms) because cancel_turn is a no-op.
        tokio::time::sleep(Duration::from_millis(400)).await;

        assert_eq!(
            svc.agent_get(&agent_id).unwrap().status,
            AgentStatus::Running,
            "old stream must not clobber the new turn"
        );
        assert!(
            svc.inflight.contains(&agent_id).unwrap(),
            "new turn must still be inflight"
        );
        svc.inflight.abort_all();
    }
    #[test]
    fn handshake_rejects_unknown_client() {
        let (dir, svc) = setup_with(Arc::new(InstantBackend));
        let err = svc
            .handshake(HandshakeParams {
                client: "web".into(),
                client_version: "0.1.0".into(),
                methods: BTreeMap::new(),
            })
            .unwrap_err();
        assert_eq!(err.code(), "invalid_params");
        let hello = svc
            .handshake(HandshakeParams {
                client: "gui".into(),
                client_version: "0.1.0".into(),
                methods: BTreeMap::new(),
            })
            .unwrap();
        assert!(!hello.session_token.is_empty());
        assert!(svc.session_valid(&hello.session_token).unwrap());
        assert!(!svc.session_valid("nope").unwrap());
        assert_eq!(
            svc.session_accepts(&hello.session_token, "host.ping")
                .unwrap(),
            Some(false)
        );
        assert_eq!(svc.session_accepts("nope", "host.ping").unwrap(), None);
        let _ = dir;
    }

    #[test]
    fn workspace_and_task_error_edges() {
        let (dir, svc) = setup_with(Arc::new(InstantBackend));
        assert!(svc.workspace_list().unwrap().is_empty());
        assert_eq!(svc.workspace_add("").unwrap_err().code(), "invalid_params");
        assert_eq!(
            svc.workspace_add("/no/such/dir-0024").unwrap_err().code(),
            "workspace_path_invalid"
        );
        let file = dir.path().join("not-dir");
        std::fs::write(&file, b"x").unwrap();
        assert_eq!(
            svc.workspace_add(file.to_str().unwrap())
                .unwrap_err()
                .code(),
            "workspace_path_invalid"
        );

        let ws_dir = dir.path().join("ws-edges");
        std::fs::create_dir(&ws_dir).unwrap();
        let ws = svc.workspace_add(ws_dir.to_str().unwrap()).unwrap();
        assert_eq!(svc.workspace_list().unwrap().len(), 1);

        assert_eq!(
            svc.task_create("", &ws.id).unwrap_err().code(),
            "invalid_params"
        );
        assert_eq!(
            svc.task_create(&"a".repeat(201), &ws.id)
                .unwrap_err()
                .code(),
            "invalid_params"
        );
        assert_eq!(
            svc.task_create("ok", "").unwrap_err().code(),
            "invalid_params"
        );
        let task = svc.task_create("t", &ws.id).unwrap();
        assert_eq!(svc.task_list("open").unwrap().len(), 1);
        assert!(svc.task_list("archived").unwrap().is_empty());
        assert_eq!(svc.task_list("all").unwrap().len(), 1);
        assert_eq!(
            svc.task_list("closed").unwrap_err().code(),
            "invalid_params"
        );
        assert_eq!(svc.task_get("missing").unwrap_err().code(), "not_found");
        let renamed = svc.task_rename(&task.id, "renamed").unwrap();
        assert_eq!(renamed.title, "renamed");
        let archived = svc.task_archive(&task.id).unwrap();
        assert_eq!(archived.status, TaskStatus::Archived);
        let again = svc.task_archive(&task.id).unwrap();
        assert_eq!(again.status, TaskStatus::Archived);
        assert_eq!(svc.agent_list("missing").unwrap_err().code(), "not_found");
        assert!(svc.agent_list(&task.id).unwrap().is_empty());
        assert_eq!(
            svc.agent_create("missing", Some("cli.generic"))
                .unwrap_err()
                .code(),
            "not_found"
        );
        assert_eq!(svc.agent_get("missing").unwrap_err().code(), "not_found");
        assert_eq!(svc.get_context("missing").unwrap_err().code(), "not_found");
        let ping = svc.ping();
        assert_eq!(ping.host_id, svc.host_id());
        let doc = svc.doctor().unwrap();
        assert_eq!(doc.host_id, svc.host_id());
        assert!(doc.db_ok);
        assert_eq!(doc.workspace_count, 1);
        svc.going_away();
        let _ = svc.subscribe_events();
    }

    #[test]
    fn send_empty_content_and_missing_agent() {
        let (dir, svc) = setup_with(Arc::new(InstantBackend));
        let agent_id = seed_agent(&svc, &dir);
        assert_eq!(
            svc.send(&agent_id, "").unwrap_err().code(),
            "invalid_params"
        );
        assert_eq!(
            svc.send("0191f0c6-7c2a-7c11-8000-6f0c1a2b3c4d", "hi")
                .unwrap_err()
                .code(),
            "not_found"
        );
        assert_eq!(
            svc.cancel("not-a-uuid").unwrap_err().code(),
            "invalid_params"
        );
        assert_eq!(
            svc.cancel("0191f0c6-7c2a-7c11-8000-6f0c1a2b3c4d")
                .unwrap_err()
                .code(),
            "not_found"
        );
    }
}
