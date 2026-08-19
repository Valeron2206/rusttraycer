//! HostService: domain operations + agent.send orchestration.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rt_protocol::{
    ApprovalDecision, ApprovalRespondOk, ApprovalRespondParams, CancelOk,
    HarnessCaps as HarnessCapsWire, PolicyGetParams, PolicyMode, PolicyScope, PolicySetParams,
    PolicySource, PolicyView,
};
use rt_runtime::{AgentBackend, TurnRequest, WireMessage, WireRole};
use rt_storage::{
    Agent, AgentStatus, HarnessId, Message, MessageRole, Store, Task, TaskFilter, TaskStatus,
    Workspace,
};
use serde::Serialize;

use crate::bind;
use crate::files;
use crate::handshake::{self, HandshakeParams, HandshakeResult};
use crate::rpc::WsEvent;
use crate::supervisor::{self, Inflight};
use crate::{HostError, Result};

const MAX_CONTENT: usize = 1024 * 1024;
const MAX_TITLE_CHARS: usize = 200;

#[derive(Clone)]
struct Session {
    accepted: HashSet<String>,
}

#[derive(Clone, Debug)]
struct PendingApproval {
    approval_id: String,
    agent_id: String,
    task_id: String,
    kind: String,
    summary: String,
}

#[derive(Default)]
struct LadderState {
    pending_by_id: HashMap<String, PendingApproval>,
    pending_by_agent: HashMap<String, String>,
    applied: HashSet<String>,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentView {
    pub id: String,
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
        }
    }
}

fn harness_caps_wire(caps: rt_runtime::HarnessCaps) -> HarnessCapsWire {
    HarnessCapsWire {
        one_shot: caps.one_shot,
        long_lived: caps.long_lived,
        stream_tokens: caps.stream_tokens,
        tools: caps.tools,
        session_resume: caps.session_resume,
        a2a_inbox: caps.a2a_inbox,
        pty: caps.pty,
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
                    caps: harness_caps_wire(backend.caps()),
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
        check_title(title)?;
        if workspace_id.is_empty() {
            return Err(HostError::InvalidParams("workspaceId is required".into()));
        }
        let task = self.store.task_create(title, workspace_id)?;
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

    pub fn agent_create(&self, task_id: &str, provider: Option<&str>) -> Result<Agent> {
        let provider = provider.unwrap_or("cli.generic");
        if !matches!(provider, "cli.generic" | "cli.claude" | "cli.codex") {
            return Err(HostError::InvalidParams(format!(
                "provider must be cli.generic|cli.claude|cli.codex, got {provider}"
            )));
        }
        if self.store.task_get(task_id)?.is_none() {
            return Err(HostError::NotFound(format!("task {task_id}")));
        }
        // available=false does not block create
        Ok(self.store.agent_create(task_id, &self.host_id, provider)?)
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
        Ok(self.send_gated(agent_id, content, false)?.user)
    }

    pub fn send_gated(&self, agent_id: &str, content: &str, ladder: bool) -> Result<SendOutcome> {
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

        let user =
            self.append_user_and_spawn(agent_id, content, &task, workspace.path.into(), backend)?;
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

        if self.clear_pending_for_agent(agent_id)? {
            tracing::info!(agent_id, "cancel pending approval");
            return Ok(CancelOk {
                agent_id: agent_id.to_string(),
                cancelled: true,
            });
        }

        let has_inflight = self.inflight.contains(agent_id)?;
        if !has_inflight && agent.status != AgentStatus::Running {
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
                self.start_saved_turn(&pending)
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
                self.start_saved_turn(&pending)
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
        let task = self
            .store
            .task_get(&agent.task_id)?
            .ok_or_else(|| HostError::NotFound(format!("task {}", agent.task_id)))?;
        if let Some(ws_id) = task.workspace_ids.first() {
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
        if let Err(e) = self.spawn_reserved_turn(
            agent_id,
            &task.id,
            workspace_path,
            backend,
            gen,
            Some(user.clone()),
        ) {
            self.inflight.remove_if(agent_id, gen);
            return Err(e);
        }
        Ok(user)
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
        if let Err(e) = self.spawn_reserved_turn(
            &pending.agent_id,
            &task.id,
            workspace.path.into(),
            backend,
            gen,
            None,
        ) {
            self.inflight.remove_if(&pending.agent_id, gen);
            return Err(e);
        }
        Ok(())
    }

    fn spawn_reserved_turn(
        &self,
        agent_id: &str,
        task_id: &str,
        workspace_path: std::path::PathBuf,
        backend: Arc<dyn AgentBackend>,
        gen: u64,
        user: Option<Message>,
    ) -> Result<()> {
        self.store
            .agent_set_status(agent_id, AgentStatus::Running)?;
        if let Err(e) = self.store.task_touch(task_id) {
            tracing::warn!(task_id, error = %e, "task_touch failed");
        }
        let _ = self.events.send(WsEvent::agent_status(
            task_id,
            agent_id,
            AgentStatus::Running,
        ));
        if let Some(user) = user {
            let _ = self
                .events
                .send(WsEvent::agent_message(task_id, agent_id, user));
        }
        let _ = self.events.send(WsEvent::task_updated(task_id));

        let messages = self
            .store
            .message_list(agent_id)?
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

        let req = TurnRequest {
            agent_id: agent_id.to_string(),
            task_id: task_id.to_string(),
            workspace_path,
            messages,
            extra_env: {
                let mut env = BTreeMap::new();
                env.insert("RUSTTRAYCER_AGENT_ID".into(), agent_id.to_string());
                env.insert("RUSTTRAYCER_TASK_ID".into(), task_id.to_string());
                env
            },
        };

        let handle = supervisor::spawn_turn(supervisor::SpawnTurn {
            store: self.store.clone(),
            backend,
            req,
            agent_id: agent_id.to_string(),
            task_id: task_id.to_string(),
            events: self.events.clone(),
            inflight: self.inflight.clone(),
            gen,
            timeout: self.turn_timeout,
        });
        self.inflight.attach(agent_id, gen, handle);
        Ok(())
    }
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
