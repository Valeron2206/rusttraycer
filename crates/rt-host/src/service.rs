//! HostService: domain operations + agent.send orchestration.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};

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
use crate::{generic_cmd_probe, HostError, Result};

const MAX_CONTENT: usize = 1024 * 1024;
const MAX_TITLE_CHARS: usize = 200;

#[derive(Clone)]
struct Session {
    accepted: HashSet<String>,
}

#[derive(Clone)]
pub struct HostService {
    pub store: Store,
    backends: HashMap<String, Arc<dyn AgentBackend>>,
    inflight: Inflight,
    turn_gate: Arc<Mutex<()>>,
    events: tokio::sync::broadcast::Sender<WsEvent>,
    sessions: Arc<Mutex<HashMap<String, Session>>>,
    host_id: String,
    data_dir: std::path::PathBuf,
    db_path: std::path::PathBuf,
    log_path: std::path::PathBuf,
    rpc_url: String,
    pid: u32,
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
        }
    }
}

fn check_title(title: &str) -> Result<()> {
    let n = title.chars().count();
    if n < 1 || n > MAX_TITLE_CHARS {
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
            host_id,
            data_dir,
            db_path,
            log_path,
            rpc_url,
            pid,
        }
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
            .insert(token.clone(), Session { accepted: accepted_names });
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
        let probe = generic_cmd_probe();
        let providers = vec![ProviderInfo {
            id: "cli.generic".into(),
            available: probe.available,
            detail: probe.detail,
        }];
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
        Ok(self.store.workspace_add(canon.to_string_lossy().as_ref(), &name)?)
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
        if provider != "cli.generic" {
            return Err(HostError::InvalidParams(format!(
                "provider must be cli.generic, got {provider}"
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
        files::files_tree(&self.store, &p)
    }

    pub fn files_read(&self, workspace_id: &str, path: &str) -> Result<serde_json::Value> {
        files::files_read(
            &self.store,
            &serde_json::json!({ "workspaceId": workspace_id, "path": path }),
        )
    }

    /// Durable user message + spawn turn. Returns the user Message.
    pub fn send(&self, agent_id: &str, content: &str) -> Result<Message> {
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

        if agent.status == AgentStatus::Running || self.inflight.contains(agent_id)? {
            return Err(HostError::AgentBusy);
        }

        // HashMap lookup, no match on provider. caps() ignored in MVP.
        let backend = self.backends.get(agent.provider.as_str()).cloned().ok_or_else(|| {
            HostError::Internal(format!("no backend registered for {}", agent.provider))
        })?;
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
        if let Err(e) = self.store.agent_set_status(agent_id, AgentStatus::Running) {
            self.inflight.remove_if(agent_id, gen);
            return Err(e.into());
        }
        let _ = self.store.task_touch(&task.id);
        let _ = self.events.send(WsEvent::agent_status(
            &task.id,
            agent_id,
            AgentStatus::Running,
        ));
        let _ = self
            .events
            .send(WsEvent::agent_message(&task.id, agent_id, user.clone()));
        let _ = self.events.send(WsEvent::task_updated(&task.id));

        let messages = match self.store.message_list(agent_id) {
            Ok(list) => list
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
                .collect(),
            Err(e) => {
                self.inflight.remove_if(agent_id, gen);
                return Err(e.into());
            }
        };

        let req = TurnRequest {
            agent_id: agent_id.to_string(),
            task_id: task.id.clone(),
            workspace_path: workspace.path.into(),
            messages,
            extra_env: {
                let mut env = BTreeMap::new();
                env.insert("RUSTTRAYCER_AGENT_ID".into(), agent_id.to_string());
                env.insert("RUSTTRAYCER_TASK_ID".into(), task.id.clone());
                env
            },
        };

        drop(gate);

        let handle = supervisor::spawn_turn(
            self.store.clone(),
            backend,
            req,
            agent_id.to_string(),
            task.id,
            self.events.clone(),
            self.inflight.clone(),
            gen,
        );
        self.inflight.attach(agent_id, gen, handle);
        Ok(user)
    }

    pub fn going_away(&self) {
        let _ = self.events.send(WsEvent::host_going_away(&self.host_id));
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
                TurnEvent::Failed { message: "should not run".into() }
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
        let second = svc.send(&agent_id, "again").expect("second send after instant finish");
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
        assert_eq!(busy, 1, "exactly one send should be agent_busy: {ra:?} {rb:?}");
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
}
