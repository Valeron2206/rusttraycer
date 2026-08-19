//! Session/UI state. Live host: health + handshake + ping, then workspace/task catalog.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Instant;

use crate::discovery::{self, DiscoverError};
use crate::ladder::{
    self, AgentPolicy, PaneKind, PendingApproval, PolicyMode, SplitLayout, PICKER_EMPTY,
    PICKER_HINT,
};
use crate::rpc::{
    CancelOk, ConnectError, DoctorOk, DoctorProvider, GitDiffOk, GitStatusOk, Worktree,
};
use crate::ws::{self, ApplyOutcome, WsBridge, WsIncoming};

enum RpcIncoming {
    Cancel {
        agent_id: String,
        result: Result<CancelOk, ConnectError>,
    },
}

/// Toast when host returns `agent_busy` (one inflight turn).
pub const TOAST_AGENT_BUSY: &str = "агент занят";

/// Git panel empty/error when `git.status` / `git.diff` returns `invalid_params`.
pub const GIT_NOTE_INVALID_PARAMS: &str = "нет git-статуса (invalid_params)";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostStatus {
    Connecting,
    Online,
    Offline,
}

impl HostStatus {
    pub fn label_ru(self) -> &'static str {
        match self {
            Self::Connecting => "подключение",
            Self::Online => "онлайн",
            Self::Offline => "офлайн",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Tasks,
    Canvas,
    Host,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskFilter {
    Open,
    Archived,
}

impl TaskFilter {
    pub fn as_rpc(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Archived => "archived",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskStatus {
    Open,
    Archived,
}

impl TaskStatus {
    pub fn label_ru(self) -> &'static str {
        match self {
            Self::Open => "открыта",
            Self::Archived => "в архиве",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentStatus {
    Idle,
    Running,
    Error,
}

impl AgentStatus {
    pub fn label_ru(self) -> &'static str {
        match self {
            Self::Idle => "ожидание",
            Self::Running => "работает",
            Self::Error => "ошибка",
        }
    }

    pub fn from_wire(s: &str) -> Self {
        match s {
            "running" => Self::Running,
            "error" => Self::Error,
            _ => Self::Idle,
        }
    }

    pub fn as_wire(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    File,
    Dir,
}

#[derive(Clone, Debug)]
pub struct PidInfo {
    pub host_id: String,
    pub pid: u64,
    pub rpc_url: String,
    pub ws_url: Option<String>,
    pub started_at: Option<String>,
}

#[derive(Clone, Debug)]
pub struct TaskStub {
    pub id: String,
    pub title: String,
    pub status: TaskStatus,
    pub updated_at: String,
}

impl From<rt_protocol::Task> for TaskStub {
    fn from(task: rt_protocol::Task) -> Self {
        Self {
            id: task.id,
            title: task.title,
            status: match task.status.as_str() {
                "archived" => TaskStatus::Archived,
                _ => TaskStatus::Open,
            },
            updated_at: task.updated_at,
        }
    }
}

#[derive(Clone, Debug)]
pub struct AgentStub {
    pub id: String,
    pub task_id: String,
    pub provider: String,
    pub status: AgentStatus,
}

impl From<rt_protocol::Agent> for AgentStub {
    fn from(agent: rt_protocol::Agent) -> Self {
        Self {
            id: agent.id,
            task_id: agent.task_id,
            provider: agent.provider,
            status: AgentStatus::from_wire(&agent.status),
        }
    }
}

#[derive(Clone, Debug)]
pub struct FileNode {
    pub name: String,
    pub path: String,
    pub kind: FileKind,
}

impl From<rt_protocol::FileEntry> for FileNode {
    fn from(entry: rt_protocol::FileEntry) -> Self {
        Self {
            name: entry.name,
            path: entry.path,
            kind: if entry.kind == "dir" {
                FileKind::Dir
            } else {
                FileKind::File
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub id: String,
    pub role: String,
    pub content: String,
}

impl From<rt_protocol::Message> for ChatMessage {
    fn from(message: rt_protocol::Message) -> Self {
        Self {
            id: message.id,
            role: message.role,
            content: message.content,
        }
    }
}

#[derive(Clone, Debug)]
pub enum FilePreview {
    Text {
        path: String,
        content: String,
        truncated: bool,
    },
    Message {
        path: String,
        text: String,
    },
}

pub struct AppState {
    pub host_status: HostStatus,
    pub pid_info: Option<PidInfo>,
    pub discover_error: Option<String>,
    pub pending_discover: bool,
    pub screen: Screen,
    pub selected_task_id: Option<String>,
    pub selected_agent_id: Option<String>,
    pub workspace_path: Option<String>,
    pub workspace_path_draft: String,
    pub workspaces: Vec<rt_protocol::Workspace>,
    pub workspace_id: Option<String>,
    pub tasks: Vec<TaskStub>,
    pub agents: Vec<AgentStub>,
    pub file_tree: Vec<FileNode>,
    pub file_children: HashMap<String, Vec<FileNode>>,
    pub file_expanded: HashSet<String>,
    pub file_tree_truncated: bool,
    pub messages: Vec<ChatMessage>,
    pub task_filter: TaskFilter,
    pub new_task_title: String,
    pub show_new_task_dialog: bool,
    pub show_rename_dialog: bool,
    pub rename_task_id: Option<String>,
    pub rename_task_title: String,
    pub composer_text: String,
    pub demo: bool,
    pub toast: Option<String>,
    pub selected_file: Option<String>,
    pub file_preview: Option<FilePreview>,
    pub copied_flash: Option<String>,
    pub session_token: Option<String>,
    pub session_host_id: Option<String>,
    pub session: Option<crate::rpc::Session>,
    pub last_rpc: Option<Instant>,
    pub ws: Option<WsBridge>,
    pub ws_banner: Option<String>,
    pub canvas_loaded_for: Option<String>,
    pub worktree: Option<Worktree>,
    pub git_status: Option<GitStatusOk>,
    pub git_diff: Option<GitDiffOk>,
    pub git_selected_path: Option<String>,
    pub git_note: Option<String>,
    pub providers: Vec<DoctorProvider>,
    pub doctor: Option<DoctorOk>,
    pub picker_provider: Option<String>,
    pub open_task_ids: Vec<String>,
    pub selected_agent_by_task: HashMap<String, String>,
    pub split: SplitLayout,
    pub policies: HashMap<String, AgentPolicy>,
    pub pending_approvals: HashMap<String, PendingApproval>,
    pub show_yolo_confirm: bool,
    pub ladder_status: Option<String>,
    pending_cancel: Option<String>,
    rpc_tx: Sender<RpcIncoming>,
    rpc_rx: Receiver<RpcIncoming>,
}

impl AppState {
    pub fn new() -> Self {
        let demo = std::env::var("RT_GUI_DEMO")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let (rpc_tx, rpc_rx) = mpsc::channel();
        let mut state = Self {
            host_status: HostStatus::Connecting,
            pid_info: None,
            discover_error: None,
            pending_discover: true,
            screen: Screen::Tasks,
            selected_task_id: None,
            selected_agent_id: None,
            workspace_path: None,
            workspace_path_draft: String::new(),
            workspaces: Vec::new(),
            workspace_id: None,
            tasks: Vec::new(),
            agents: Vec::new(),
            file_tree: Vec::new(),
            file_children: HashMap::new(),
            file_expanded: HashSet::new(),
            file_tree_truncated: false,
            messages: Vec::new(),
            task_filter: TaskFilter::Open,
            new_task_title: String::new(),
            show_new_task_dialog: false,
            show_rename_dialog: false,
            rename_task_id: None,
            rename_task_title: String::new(),
            composer_text: String::new(),
            demo,
            toast: None,
            selected_file: None,
            file_preview: None,
            copied_flash: None,
            session_token: None,
            session_host_id: None,
            session: None,
            last_rpc: None,
            ws: None,
            ws_banner: None,
            canvas_loaded_for: None,
            worktree: None,
            git_status: None,
            git_diff: None,
            git_selected_path: None,
            git_note: None,
            providers: Vec::new(),
            doctor: None,
            picker_provider: None,
            open_task_ids: Vec::new(),
            selected_agent_by_task: HashMap::new(),
            split: ladder::load_split_layout(),
            policies: HashMap::new(),
            pending_approvals: HashMap::new(),
            show_yolo_confirm: false,
            ladder_status: None,
            pending_cancel: None,
            rpc_tx,
            rpc_rx,
        };

        if demo {
            state.seed_demo();
        }
        state
    }

    fn seed_demo(&mut self) {
        // Demo chrome only. Do not inject fake tasks — list data comes from host.
        self.selected_task_id = Some("demo-task-1".into());
        self.selected_agent_id = Some("demo-agent-1".into());
        self.agents.push(AgentStub {
            id: "demo-agent-1".into(),
            task_id: "demo-task-1".into(),
            provider: "cli.generic".into(),
            status: AgentStatus::Idle,
        });
        self.file_tree = vec![
            FileNode {
                name: "src".into(),
                path: "src".into(),
                kind: FileKind::Dir,
            },
            FileNode {
                name: "main.rs".into(),
                path: "src/main.rs".into(),
                kind: FileKind::File,
            },
            FileNode {
                name: "README.md".into(),
                path: "README.md".into(),
                kind: FileKind::File,
            },
        ];
        self.messages.push(ChatMessage {
            id: "demo-msg-1".into(),
            role: "system".into(),
            content: "Демо-режим: host offline, composer выключен. Это заглушка для скриншотов."
                .into(),
        });
    }

    pub fn tick_discovery(&mut self) {
        if self.pending_discover {
            self.pending_discover = false;
            self.apply_discovery();
            return;
        }
        if self.host_status == HostStatus::Online {
            let due = self
                .last_rpc
                .map(|t| t.elapsed() >= std::time::Duration::from_secs(5))
                .unwrap_or(true);
            if due {
                if let Some(session) = self.session.clone() {
                    match crate::rpc::keepalive(&session) {
                        Ok(()) => self.last_rpc = Some(Instant::now()),
                        Err(err) => {
                            self.go_offline(Some(err.as_label()));
                        }
                    }
                }
            }
        }
    }

    pub fn tick_ws(&mut self) {
        let mut incoming = Vec::new();
        if let Some(ws) = &self.ws {
            while let Some(ev) = ws.try_recv() {
                incoming.push(ev);
            }
        }
        for ev in incoming {
            self.apply_ws_incoming(ev);
        }
    }

    pub fn tick_rpc(&mut self) {
        let mut incoming = Vec::new();
        while let Ok(ev) = self.rpc_rx.try_recv() {
            incoming.push(ev);
        }
        for ev in incoming {
            self.apply_rpc_incoming(ev);
        }
    }

    fn apply_rpc_incoming(&mut self, incoming: RpcIncoming) {
        match incoming {
            RpcIncoming::Cancel { agent_id, result } => {
                self.apply_cancel_result(&agent_id, result);
            }
        }
    }

    fn apply_cancel_result(&mut self, agent_id: &str, result: Result<CancelOk, ConnectError>) {
        match result {
            Ok(ok) => {
                if self.pending_cancel.as_deref() == Some(agent_id)
                    || self.pending_cancel.as_deref() == Some(ok.agent_id.as_str())
                {
                    self.pending_cancel = None;
                }
                if ok.cancelled {
                    self.pending_cancel = None;
                }
                // cancelled true or false is both ok — hide Stop, enable composer.
                if let Some(agent) = self
                    .agents
                    .iter_mut()
                    .find(|a| a.id == ok.agent_id || a.id == agent_id)
                {
                    agent.status = AgentStatus::Idle;
                }
            }
            Err(err) => {
                if self.pending_cancel.as_deref() == Some(agent_id) {
                    self.pending_cancel = None;
                }
                self.toast = Some(err.as_label());
            }
        }
    }

    pub fn wants_repaint(&self) -> bool {
        self.pending_cancel.is_some() || (self.ws.is_some() && self.screen == Screen::Canvas)
    }

    pub fn request_retry(&mut self) {
        self.host_status = HostStatus::Connecting;
        self.pending_discover = true;
        self.toast = None;
        self.ws_banner = None;
    }

    fn apply_discovery(&mut self) {
        self.host_status = HostStatus::Connecting;
        self.session_token = None;
        match discovery::read_pid_json() {
            Ok(info) => {
                self.pid_info = Some(info.clone());
                match crate::rpc::connect(&info) {
                    Ok(session) => {
                        self.session_token = Some(session.session_token.clone());
                        self.session_host_id = Some(session.host_id.clone());
                        self.start_ws(&session);
                        self.session = Some(session);
                        self.discover_error = None;
                        self.host_status = HostStatus::Online;
                        self.last_rpc = Some(Instant::now());
                        self.ws_banner = None;
                        self.refresh_doctor();
                        self.refresh_tasks_catalog();
                        if self.screen == Screen::Canvas {
                            self.refresh_canvas_after_reconnect();
                        }
                    }
                    Err(err) => {
                        self.go_offline(Some(err.as_label()));
                    }
                }
            }
            Err(DiscoverError::Missing) => {
                self.pid_info = None;
                self.go_offline(Some(DiscoverError::Missing.as_label()));
            }
            Err(err) => {
                self.pid_info = None;
                self.go_offline(Some(err.as_label()));
            }
        }
    }

    fn start_ws(&mut self, session: &crate::rpc::Session) {
        self.ws = None;
        if let Some(url) = session.ws_url.clone().filter(|u| !u.trim().is_empty()) {
            self.ws = Some(WsBridge::start(url, session.session_token.clone()));
        }
    }

    fn stop_ws(&mut self) {
        self.ws = None;
    }

    fn go_offline(&mut self, error: Option<String>) {
        self.stop_ws();
        self.session = None;
        self.session_token = None;
        if error.is_some() {
            self.discover_error = error;
        }
        self.host_status = HostStatus::Offline;
        self.ws_banner = None;
        self.clear_host_catalog();
    }

    fn apply_ws_incoming(&mut self, incoming: WsIncoming) {
        match incoming {
            WsIncoming::Event(event) => self.apply_ws_event(event),
            WsIncoming::Disconnected { reason } => {
                if self.is_online() {
                    self.ws_banner = Some(if reason.is_empty() {
                        "Соединение с host потеряно, переподключение…".into()
                    } else {
                        format!("Соединение с host потеряно, переподключение… ({reason})")
                    });
                }
            }
            WsIncoming::Reconnected => {
                self.ws_banner = None;
                if self.is_online() {
                    self.refresh_canvas_after_reconnect();
                }
            }
        }
    }

    fn apply_ws_event(&mut self, event: ws::WsEvent) {
        if let ws::WsEvent::AgentApproval {
            approval_id,
            agent_id,
            task_id,
            kind,
            summary,
        } = &event
        {
            let on_open = self.open_task_ids.iter().any(|id| id == task_id)
                || self.selected_task_id.as_deref() == Some(task_id.as_str());
            if on_open && !approval_id.is_empty() {
                self.pending_approvals.insert(
                    agent_id.clone(),
                    PendingApproval {
                        approval_id: approval_id.clone(),
                        agent_id: agent_id.clone(),
                        task_id: task_id.clone(),
                        kind: kind.clone(),
                        summary: summary.clone(),
                    },
                );
            }
            return;
        }
        let task_filter = self.selected_task_id.clone();
        let agent_filter = self.selected_agent_id.clone();
        let outcome = ws::apply_event(
            &mut self.messages,
            &event,
            task_filter.as_deref(),
            agent_filter.as_deref(),
        );
        match outcome {
            ApplyOutcome::StatusChanged(status) => {
                if let ws::WsEvent::AgentStatus { agent_id, .. } = &event {
                    if let Some(agent) = self.agents.iter_mut().find(|a| &a.id == agent_id) {
                        agent.status = AgentStatus::from_wire(&status);
                    }
                }
            }
            ApplyOutcome::GoingAway => {
                // Offline banner; do not close the window.
                self.stop_ws();
                self.session = None;
                self.session_token = None;
                self.discover_error = Some("host.going_away".into());
                self.host_status = HostStatus::Offline;
                self.ws_banner = None;
            }
            ApplyOutcome::TaskUpdated => {
                if self.is_online() {
                    self.reload_task_list();
                }
            }
            ApplyOutcome::Appended
            | ApplyOutcome::Deduped
            | ApplyOutcome::Ignored
            | ApplyOutcome::Approval => {}
        }
    }

    fn ws_subscribe(&self, task_id: &str) {
        if let Some(ws) = &self.ws {
            ws.subscribe(task_id.to_string());
        }
    }

    /// Host restart / WS reconnect: refetch and REPLACE canvas data. Never append.
    fn refresh_canvas_after_reconnect(&mut self) {
        if let Some(id) = self.selected_task_id.clone() {
            self.reload_canvas(&id);
            self.ws_subscribe(&id);
        }
    }

    pub fn is_online(&self) -> bool {
        self.host_status == HostStatus::Online
    }

    pub fn is_offline(&self) -> bool {
        self.host_status == HostStatus::Offline
    }

    pub fn has_workspace(&self) -> bool {
        let Some(id) = self.workspace_id.as_ref() else {
            return false;
        };
        self.workspaces.iter().any(|w| &w.id == id)
    }

    pub fn can_rpc(&self) -> bool {
        self.is_online()
    }

    pub fn can_create_task(&self) -> bool {
        self.can_rpc() && self.has_workspace()
    }

    pub fn can_create_agent(&self) -> bool {
        self.can_rpc()
            && self.selected_task_id.is_some()
            && self.picker_provider.is_some()
            && !self.providers.is_empty()
    }

    pub fn worktree_id(&self) -> Option<&str> {
        self.worktree.as_ref().map(|w| w.id.as_str())
    }

    pub fn can_isolate_agent(&self) -> bool {
        self.can_rpc() && self.selected_agent().is_some()
    }

    pub fn isolate_selected_agent(&mut self) {
        if !self.can_isolate_agent() {
            return;
        }
        let Some(agent_id) = self.selected_agent().map(|a| a.id.clone()) else {
            return;
        };
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.worktree_ensure(&agent_id) {
            Ok(wt) => {
                self.worktree = Some(wt);
                self.git_note = None;
                self.load_file_tree_root();
                self.load_git_panel();
            }
            Err(err) => {
                self.toast = Some(err.as_label());
                self.git_note = Some(err.as_label());
            }
        }
    }

    pub fn select_git_path(&mut self, path: String) {
        self.git_selected_path = Some(path);
        self.load_git_diff();
    }

    fn load_git_panel(&mut self) {
        self.git_note = None;
        let workspace_id = match self.workspace_id.clone() {
            Some(id) => id,
            None => match self.worktree.as_ref() {
                Some(wt) if !wt.workspace_id.is_empty() => wt.workspace_id.clone(),
                _ => {
                    self.git_status = None;
                    self.git_diff = None;
                    return;
                }
            },
        };
        let Some(session) = self.session.clone() else {
            return;
        };
        if let Some(agent_id) = self.selected_agent().map(|a| a.id.clone()) {
            let same = self
                .worktree
                .as_ref()
                .map(|w| w.agent_id == agent_id)
                .unwrap_or(false);
            if !same {
                match session.worktree_get(&agent_id) {
                    Ok(wt) => self.worktree = wt,
                    Err(err) => {
                        self.toast = Some(err.as_label());
                    }
                }
            }
        } else {
            self.worktree = None;
        }
        let wt = self.worktree_id().map(|s| s.to_string());
        match session.git_status(&workspace_id, wt.as_deref()) {
            Ok(status) => {
                self.git_status = Some(status);
                self.load_git_diff();
            }
            Err(err) => {
                self.git_status = None;
                self.git_diff = None;
                self.git_note = Some(git_error_note(&err));
            }
        }
    }

    fn load_git_diff(&mut self) {
        let Some(workspace_id) = self.workspace_id.clone() else {
            self.git_diff = None;
            return;
        };
        let Some(session) = self.session.clone() else {
            return;
        };
        let wt = self.worktree_id().map(|s| s.to_string());
        let path = self.git_selected_path.clone();
        match session.git_diff(&workspace_id, wt.as_deref(), path.as_deref()) {
            Ok(diff) => self.git_diff = Some(diff),
            Err(err) => {
                self.git_diff = None;
                self.git_note = Some(git_error_note(&err));
            }
        }
    }

    pub fn selected_task(&self) -> Option<&TaskStub> {
        let id = self.selected_task_id.as_ref()?;
        self.tasks.iter().find(|t| &t.id == id)
    }

    pub fn selected_task_title(&self) -> Option<&str> {
        self.selected_task().map(|t| t.title.as_str())
    }

    pub fn agents_for_selected_task(&self) -> Vec<&AgentStub> {
        let Some(id) = self.selected_task_id.as_ref() else {
            return Vec::new();
        };
        self.agents.iter().filter(|a| &a.task_id == id).collect()
    }

    pub fn selected_agent(&self) -> Option<&AgentStub> {
        let agents = self.agents_for_selected_task();
        if let Some(id) = self.selected_agent_id.as_ref() {
            if let Some(agent) = agents.iter().copied().find(|a| &a.id == id) {
                return Some(agent);
            }
        }
        agents.into_iter().next()
    }

    pub fn composer_enabled(&self) -> bool {
        ws::composer_allowed(
            self.can_rpc(),
            self.selected_agent().map(|a| a.status.as_wire()),
        )
    }

    pub fn selected_agent_is_running(&self) -> bool {
        self.selected_agent()
            .map(|a| a.status == AgentStatus::Running)
            .unwrap_or(false)
    }

    /// «Стоп» only while the selected agent is running. Never for idle/error.
    pub fn show_stop_button(&self) -> bool {
        self.selected_agent_is_running()
    }

    pub fn composer_disabled_reason(&self) -> Option<&'static str> {
        if self.composer_enabled() {
            return None;
        }
        if !self.can_rpc() {
            return Some("недоступно: host offline");
        }
        if self.selected_agent().is_none() {
            return Some("сначала создайте агента");
        }
        Some("агент работает")
    }

    pub fn filtered_tasks(&self) -> Vec<&TaskStub> {
        self.tasks
            .iter()
            .filter(|t| match self.task_filter {
                TaskFilter::Open => t.status == TaskStatus::Open,
                TaskFilter::Archived => t.status == TaskStatus::Archived,
            })
            .collect()
    }

    pub fn open_task(&mut self, id: String) {
        self.remember_selected_agent();
        if !self.open_task_ids.iter().any(|open| open == &id) {
            self.open_task_ids.push(id.clone());
        }
        self.selected_task_id = Some(id.clone());
        self.screen = Screen::Canvas;
        self.selected_file = None;
        self.file_preview = None;
        self.selected_agent_id = self.selected_agent_by_task.get(&id).cloned();
        if self.demo {
            return;
        }
        self.reload_canvas(&id);
        self.ws_subscribe(&id);
    }

    pub fn switch_task_tab(&mut self, id: String) {
        if self.selected_task_id.as_deref() == Some(id.as_str()) {
            return;
        }
        self.open_task(id);
    }

    pub fn close_task_tab(&mut self, id: &str) {
        self.open_task_ids.retain(|open| open != id);
        self.selected_agent_by_task.remove(id);
        self.pending_approvals.retain(|_, ap| ap.task_id != id);
        if self.selected_task_id.as_deref() != Some(id) {
            return;
        }
        if let Some(next) = self.open_task_ids.last().cloned() {
            self.selected_task_id = None;
            self.open_task(next);
        } else {
            self.selected_task_id = None;
            self.selected_agent_id = None;
            self.screen = Screen::Tasks;
        }
    }

    fn remember_selected_agent(&mut self) {
        if let (Some(task_id), Some(agent_id)) = (&self.selected_task_id, &self.selected_agent_id) {
            self.selected_agent_by_task
                .insert(task_id.clone(), agent_id.clone());
        }
    }

    fn reload_canvas(&mut self, task_id: &str) {
        self.file_tree.clear();
        self.file_children.clear();
        self.file_expanded.clear();
        self.file_tree_truncated = false;
        self.reload_agents(task_id);
        self.load_selected_agent();
        self.load_file_tree_root();
        self.canvas_loaded_for = Some(task_id.to_string());
    }

    fn reload_agents(&mut self, task_id: &str) {
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.agent_list(task_id) {
            Ok(items) => {
                self.agents.retain(|a| a.task_id != task_id);
                self.agents.extend(items.into_iter().map(AgentStub::from));
                let still_valid = self
                    .selected_agent_id
                    .as_ref()
                    .map(|id| {
                        self.agents
                            .iter()
                            .any(|a| &a.id == id && a.task_id == task_id)
                    })
                    .unwrap_or(false);
                if !still_valid {
                    self.selected_agent_id = self
                        .agents
                        .iter()
                        .find(|a| a.task_id == task_id)
                        .map(|a| a.id.clone());
                }
            }
            Err(err) => {
                self.toast = Some(err.as_label());
            }
        }
    }

    fn load_selected_agent(&mut self) {
        let Some(agent_id) = self.selected_agent().map(|a| a.id.clone()) else {
            self.messages.clear();
            return;
        };
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.agent_get(&agent_id) {
            Ok(agent) => {
                if let Some(stub) = self.agents.iter_mut().find(|a| a.id == agent_id) {
                    *stub = AgentStub::from(agent);
                }
            }
            Err(err) => {
                self.toast = Some(err.as_label());
            }
        }
        match session.agent_get_context(&agent_id) {
            Ok(messages) => {
                apply_get_context_replace(
                    &mut self.messages,
                    messages.into_iter().map(ChatMessage::from).collect(),
                );
            }
            Err(err) => {
                self.toast = Some(err.as_label());
            }
        }
        self.load_policy_for(&agent_id);
        self.load_git_panel();
    }

    fn load_file_tree_root(&mut self) {
        let Some(workspace_id) = self.workspace_id.clone() else {
            return;
        };
        let Some(session) = self.session.clone() else {
            return;
        };
        let tree = match self.worktree_id() {
            Some(id) => session.files_tree_for(&workspace_id, "", Some(id)),
            None => session.files_tree(&workspace_id, ""),
        };
        match tree {
            Ok(tree) => {
                self.file_tree = tree.items.into_iter().map(FileNode::from).collect();
                self.file_tree_truncated = tree.truncated;
            }
            Err(err) => {
                self.toast = Some(err.as_label());
            }
        }
    }

    pub fn create_agent(&mut self) {
        if !self.can_create_agent() {
            if self.providers.is_empty() {
                self.toast = Some(PICKER_EMPTY.into());
            } else if self.picker_provider.is_none() {
                self.toast = Some(PICKER_HINT.into());
            }
            return;
        }
        let Some(task_id) = self.selected_task_id.clone() else {
            return;
        };
        let Some(provider) = self.picker_provider.clone() else {
            self.toast = Some(PICKER_HINT.into());
            return;
        };
        if !self.providers.iter().any(|p| p.id == provider) {
            self.toast = Some(PICKER_EMPTY.into());
            return;
        }
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.agent_create(&task_id, &provider) {
            Ok(agent) => {
                let stub = AgentStub::from(agent);
                self.selected_agent_id = Some(stub.id.clone());
                self.remember_selected_agent();
                self.agents.push(stub);
                self.load_selected_agent();
            }
            Err(err) => {
                self.toast = Some(err.as_label());
            }
        }
    }

    pub fn select_agent(&mut self, id: String) {
        if self.selected_agent_id.as_deref() == Some(id.as_str()) {
            return;
        }
        self.selected_agent_id = Some(id);
        self.remember_selected_agent();
        self.load_selected_agent();
    }

    pub fn set_picker_provider(&mut self, id: String) {
        if self.providers.iter().any(|p| p.id == id) {
            self.picker_provider = Some(id);
        }
    }

    pub fn cancel_running_agent(&mut self) {
        if self.pending_cancel.is_some() {
            return;
        }
        let Some(agent) = self.selected_agent() else {
            return;
        };
        if agent.status != AgentStatus::Running {
            return;
        }
        let Some(session) = self.session.clone() else {
            return;
        };
        let agent_id = agent.id.clone();
        self.pending_cancel = Some(agent_id.clone());
        let tx = self.rpc_tx.clone();
        let _ = thread::Builder::new()
            .name("rt-gui-rpc-cancel".into())
            .spawn(move || {
                let result = session.agent_cancel(&agent_id);
                let _ = tx.send(RpcIncoming::Cancel { agent_id, result });
            });
    }

    pub fn send_composer(&mut self) {
        if !self.composer_enabled() {
            return;
        }
        if self.pending_cancel.is_some() {
            return;
        }
        let content = self.composer_text.trim().to_string();
        if content.is_empty() {
            return;
        }
        let Some(agent_id) = self.selected_agent().map(|a| a.id.clone()) else {
            return;
        };
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.agent_send(&agent_id, &content) {
            Ok(message) => {
                self.composer_text.clear();
                let chat = ChatMessage::from(message);
                if !self.messages.iter().any(|m| m.id == chat.id) {
                    self.messages.push(chat);
                }
            }
            Err(err) => {
                // agent_busy / not_found → toast, do not mutate the list.
                self.toast = Some(if err.is_agent_busy() {
                    TOAST_AGENT_BUSY.to_string()
                } else {
                    err.as_label()
                });
            }
        }
    }

    pub fn toggle_dir(&mut self, path: String) {
        if self.file_expanded.contains(&path) {
            self.file_expanded.remove(&path);
            return;
        }
        if !self.file_children.contains_key(&path) {
            let Some(workspace_id) = self.workspace_id.clone() else {
                return;
            };
            let Some(session) = self.session.clone() else {
                return;
            };
            let tree = match self.worktree_id() {
                Some(id) => session.files_tree_for(&workspace_id, &path, Some(id)),
                None => session.files_tree(&workspace_id, &path),
            };
            match tree {
                Ok(tree) => {
                    self.file_children.insert(
                        path.clone(),
                        tree.items.into_iter().map(FileNode::from).collect(),
                    );
                }
                Err(err) => {
                    self.toast = Some(err.as_label());
                    return;
                }
            }
        }
        self.file_expanded.insert(path);
    }

    pub fn open_file(&mut self, path: String) {
        self.selected_file = Some(path.clone());
        if !self.can_rpc() {
            self.file_preview = Some(FilePreview::Message {
                path,
                text: "нет данных (host offline)".into(),
            });
            return;
        }
        let Some(workspace_id) = self.workspace_id.clone() else {
            self.file_preview = Some(FilePreview::Message {
                path,
                text: "нет workspace".into(),
            });
            return;
        };
        let Some(session) = self.session.clone() else {
            self.file_preview = Some(FilePreview::Message {
                path,
                text: "нет данных (host offline)".into(),
            });
            return;
        };
        let read = match self.worktree_id() {
            Some(id) => session.files_read_for(&workspace_id, &path, Some(id)),
            None => session.files_read(&workspace_id, &path),
        };
        match read {
            Ok(read) => {
                self.file_preview = Some(FilePreview::Text {
                    path: read.path,
                    content: read.content,
                    truncated: read.truncated,
                });
            }
            Err(err) => {
                self.file_preview = Some(FilePreview::Message {
                    path,
                    text: err.as_label(),
                });
            }
        }
    }

    pub fn set_task_filter(&mut self, filter: TaskFilter) {
        if self.task_filter == filter {
            return;
        }
        self.task_filter = filter;
        if self.is_online() {
            self.reload_task_list();
        }
    }

    pub fn refresh_tasks_catalog(&mut self) {
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.refresh_tasks_catalog(self.task_filter.as_rpc()) {
            Ok(catalog) => self.apply_catalog(catalog),
            Err(err) => {
                // Catalog failure must not flip Online back to Offline.
                self.toast = Some(err.as_label());
            }
        }
    }

    fn reload_task_list(&mut self) {
        if !self.has_workspace() {
            self.tasks.clear();
            return;
        }
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.task_list(self.task_filter.as_rpc()) {
            Ok(tasks) => {
                self.tasks = tasks.into_iter().map(TaskStub::from).collect();
            }
            Err(err) => {
                self.toast = Some(err.as_label());
            }
        }
    }

    fn apply_catalog(&mut self, catalog: crate::rpc::TasksCatalog) {
        self.workspaces = catalog.workspaces;
        if let Some(ws) = self.workspaces.first().cloned() {
            self.workspace_id = Some(ws.id);
            self.workspace_path = Some(ws.path.clone());
            self.workspace_path_draft = ws.path;
            self.tasks = catalog.tasks.into_iter().map(TaskStub::from).collect();
        } else {
            self.workspace_id = None;
            self.workspace_path = None;
            self.tasks.clear();
        }
    }

    fn clear_host_catalog(&mut self) {
        self.workspaces.clear();
        self.workspace_id = None;
        self.workspace_path = None;
        self.tasks.clear();
    }

    pub fn set_workspace_path(&mut self, path: String) {
        let trimmed = path.trim().to_string();
        if trimmed.is_empty() {
            return;
        }
        if !self.is_online() {
            self.toast = Some("недоступно: host offline".into());
            return;
        }
        let Some(session) = self.session.clone() else {
            self.toast = Some("недоступно: host offline".into());
            return;
        };
        let abs = crate::rpc::to_absolute_path(&trimmed);
        match session.workspace_add(&abs) {
            Ok(_) => {
                self.refresh_tasks_catalog();
            }
            Err(err) => {
                self.toast = Some(err.as_label());
            }
        }
    }

    pub fn pick_workspace_folder(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Выберите папку workspace")
            .pick_folder()
        {
            self.set_workspace_path(path.to_string_lossy().into_owned());
        }
    }

    pub fn create_task(&mut self) {
        if !self.can_create_task() {
            return;
        }
        let title = self.new_task_title.trim().to_string();
        if title.is_empty() {
            return;
        }
        let Some(workspace_id) = self.workspace_id.clone() else {
            return;
        };
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.task_create(&title, &workspace_id) {
            Ok(task) => {
                self.new_task_title.clear();
                self.show_new_task_dialog = false;
                self.refresh_tasks_catalog();
                self.open_task(task.id);
            }
            Err(err) => {
                self.toast = Some(err.as_label());
            }
        }
    }

    pub fn begin_rename(&mut self, id: String, title: String) {
        self.rename_task_id = Some(id);
        self.rename_task_title = title;
        self.show_rename_dialog = true;
    }

    pub fn commit_rename(&mut self) {
        let Some(id) = self.rename_task_id.clone() else {
            return;
        };
        let title = self.rename_task_title.trim().to_string();
        if title.is_empty() {
            return;
        }
        self.rename_task(&id, &title);
    }

    pub fn rename_task(&mut self, id: &str, title: &str) {
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.task_rename(id, title) {
            Ok(_) => {
                self.show_rename_dialog = false;
                self.rename_task_id = None;
                self.refresh_tasks_catalog();
            }
            Err(err) => {
                self.toast = Some(err.as_label());
            }
        }
    }

    pub fn archive_task(&mut self, id: &str) {
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.task_archive(id) {
            Ok(_) => {
                self.refresh_tasks_catalog();
            }
            Err(err) => {
                self.toast = Some(err.as_label());
            }
        }
    }

    pub fn host_id_prefix(&self) -> String {
        let id = self
            .session_host_id
            .as_deref()
            .or_else(|| self.pid_info.as_ref().map(|p| p.host_id.as_str()));
        match id {
            Some(host_id) => host_id.chars().take(8).collect(),
            None => "—".into(),
        }
    }

    pub fn copy_host_id(&self, ctx: &egui::Context) -> bool {
        if let Some(info) = &self.pid_info {
            ctx.copy_text(info.host_id.clone());
            true
        } else {
            false
        }
    }

    pub fn refresh_doctor(&mut self) {
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.host_doctor() {
            Ok(doctor) => self.apply_doctor(doctor),
            Err(err) => {
                self.toast = Some(err.as_label());
            }
        }
    }

    fn apply_doctor(&mut self, doctor: DoctorOk) {
        self.providers = doctor.providers.clone();
        let keep = self
            .picker_provider
            .as_ref()
            .map(|id| self.providers.iter().any(|p| &p.id == id))
            .unwrap_or(false);
        if !keep {
            self.picker_provider = None;
        }
        self.doctor = Some(doctor);
    }

    pub fn selected_provider(&self) -> Option<&DoctorProvider> {
        let id = self.picker_provider.as_ref()?;
        self.providers.iter().find(|p| &p.id == id)
    }

    pub fn persist_split(&mut self) {
        ladder::save_split_layout(&self.split);
    }

    pub fn set_split_pane(&mut self, side: &str, kind: PaneKind) {
        match side {
            "left" => self.split.left = kind,
            "right" => self.split.right = kind,
            _ => return,
        }
        self.persist_split();
    }

    pub fn selected_policy(&self) -> AgentPolicy {
        match self.selected_agent() {
            Some(agent) => self.policies.get(&agent.id).cloned().unwrap_or_default(),
            None => AgentPolicy::default(),
        }
    }

    pub fn yolo_on(&self) -> bool {
        self.selected_policy().yolo
    }

    pub fn selected_approval(&self) -> Option<&PendingApproval> {
        let agent_id = self.selected_agent()?.id.as_str();
        self.pending_approvals.get(agent_id)
    }

    pub fn request_yolo_on(&mut self) {
        if self.yolo_on() {
            return;
        }
        self.show_yolo_confirm = true;
    }

    pub fn cancel_yolo_confirm(&mut self) {
        self.show_yolo_confirm = false;
    }

    pub fn confirm_yolo(&mut self) {
        self.show_yolo_confirm = false;
        self.set_yolo(true);
    }

    pub fn set_yolo_off(&mut self) {
        self.set_yolo(false);
    }

    fn set_yolo(&mut self, yolo: bool) {
        let policy = self.selected_policy();
        self.write_policy(policy.mode, &policy.scope, yolo);
    }

    pub fn set_policy_mode(&mut self, mode: PolicyMode) {
        let policy = self.selected_policy();
        if policy.mode == mode {
            return;
        }
        self.write_policy(mode, &policy.scope, policy.yolo);
    }

    fn write_policy(&mut self, mode: PolicyMode, scope: &str, yolo: bool) {
        let Some(agent_id) = self.selected_agent().map(|a| a.id.clone()) else {
            return;
        };
        let Some(session) = self.session.clone() else {
            return;
        };
        if session.ladder_rejected() && !session.ladder_accepted() {
            self.ladder_status = Some(crate::ladder::LADDER_UNAVAILABLE.into());
            return;
        }
        match session.policy_set(&agent_id, mode.as_wire(), scope, yolo) {
            Ok(ok) => {
                self.policies.insert(agent_id, AgentPolicy::from(ok));
                self.ladder_status = None;
            }
            Err(err) => {
                self.surface_ladder_error(err);
            }
        }
    }

    fn load_policy_for(&mut self, agent_id: &str) {
        let Some(session) = self.session.clone() else {
            return;
        };
        if session.ladder_rejected() && !session.ladder_accepted() {
            self.ladder_status = Some(crate::ladder::LADDER_UNAVAILABLE.into());
            return;
        }
        match session.policy_get(agent_id) {
            Ok(ok) => {
                self.policies
                    .insert(agent_id.to_string(), AgentPolicy::from(ok));
                self.ladder_status = None;
            }
            Err(err) => {
                self.surface_ladder_error(err);
            }
        }
    }

    fn surface_ladder_error(&mut self, err: ConnectError) {
        let label = err.as_label();
        if err.is_unsupported_method() {
            self.ladder_status = Some(crate::ladder::LADDER_UNAVAILABLE.into());
        } else {
            self.ladder_status = Some(label.clone());
        }
        self.toast = Some(label);
    }

    /// Title-bar X: same path as «Отказать» — `approval.respond` with deny.
    pub fn close_approval_card(&mut self) {
        self.respond_approval("deny");
    }

    pub fn respond_approval(&mut self, decision: &str) {
        let Some(approval) = self.selected_approval().cloned() else {
            return;
        };
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.approval_respond(&approval.approval_id, decision) {
            Ok(ok) => {
                self.pending_approvals.remove(&approval.agent_id);
                if !ok.applied {
                    self.toast = Some("решение уже не применяется".into());
                }
                if decision == "allow-always" {
                    if let Some(policy) = self.policies.get_mut(&approval.agent_id) {
                        policy.mode = PolicyMode::AllowAlways;
                    }
                }
            }
            Err(err) => {
                self.surface_ladder_error(err);
            }
        }
    }
}

/// Retry/reconnect: replace transcript from `agent.get_context`. Never merge.
pub fn apply_get_context_replace(messages: &mut Vec<ChatMessage>, context: Vec<ChatMessage>) {
    *messages = context;
}

fn git_error_note(err: &crate::rpc::ConnectError) -> String {
    if err.is_invalid_params() {
        GIT_NOTE_INVALID_PARAMS.to_string()
    } else {
        err.as_label()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc::connect;
    use serde_json::{json, Value};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Instant;

    #[derive(Clone, Debug)]
    struct RpcHit {
        method: String,
        params: Value,
    }

    struct SliceMock {
        origin: String,
        hits: Arc<Mutex<Vec<RpcHit>>>,
    }

    fn read_http_request(stream: &mut std::net::TcpStream) -> (String, Vec<u8>) {
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
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(resp.as_bytes());
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

    #[derive(Clone, Copy)]
    enum SendMode {
        Ok,
        Busy,
        Rpc {
            code: &'static str,
            message: &'static str,
        },
    }

    #[derive(Clone, Copy)]
    enum GitMode {
        Ok,
        InvalidParams,
    }

    fn start_slice_mock(send: SendMode, git: GitMode) -> SliceMock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let hits_t = hits.clone();
        thread::spawn(move || {
            for stream in listener.incoming().take(48) {
                let Ok(mut stream) = stream else { break };
                let (headers, body) = read_http_request(&mut stream);
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
                });
                let body = match method.as_str() {
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
                    "agent.list" => json!({
                        "id": "echo",
                        "ok": { "items": [sample_agent("ag-1", "task-1", "idle")] }
                    })
                    .to_string(),
                    "agent.get" => {
                        let id = params.get("id").and_then(|v| v.as_str()).unwrap_or("ag-1");
                        json!({ "id": "echo", "ok": sample_agent(id, "task-1", "idle") })
                            .to_string()
                    }
                    "agent.get_context" => json!({
                        "id": "echo",
                        "ok": {
                            "messages": [
                                sample_message("ctx-1", "ag-1", "user", "hi"),
                                sample_message("ctx-2", "ag-1", "assistant", "ok")
                            ]
                        }
                    })
                    .to_string(),
                    "agent.send" => match send {
                        SendMode::Ok => {
                            let agent_id =
                                params.get("agentId").and_then(|v| v.as_str()).unwrap_or("");
                            let content =
                                params.get("content").and_then(|v| v.as_str()).unwrap_or("");
                            json!({
                                "id": "echo",
                                "ok": {
                                    "userMessage": sample_message("m-new", agent_id, "user", content)
                                }
                            })
                            .to_string()
                        }
                        SendMode::Busy => json!({
                            "id": "echo",
                            "error": {
                                "code": "agent_busy",
                                "message": "agent has an in-flight turn"
                            }
                        })
                        .to_string(),
                        SendMode::Rpc { code, message } => json!({
                            "id": "echo",
                            "error": { "code": code, "message": message }
                        })
                        .to_string(),
                    },
                    "worktree.get" => json!({
                        "id": "echo",
                        "error": { "code": "not_found", "message": "no worktree" }
                    })
                    .to_string(),
                    "git.status" => match git {
                        GitMode::Ok => json!({
                            "id": "echo",
                            "ok": {
                                "branch": "main",
                                "dirty": false,
                                "truncated": false,
                                "entries": []
                            }
                        })
                        .to_string(),
                        GitMode::InvalidParams => json!({
                            "id": "echo",
                            "error": {
                                "code": "invalid_params",
                                "message": "not_git"
                            }
                        })
                        .to_string(),
                    },
                    "git.diff" => match git {
                        GitMode::Ok => json!({
                            "id": "echo",
                            "ok": { "truncated": false, "files": [] }
                        })
                        .to_string(),
                        GitMode::InvalidParams => json!({
                            "id": "echo",
                            "error": {
                                "code": "invalid_params",
                                "message": "not_git"
                            }
                        })
                        .to_string(),
                    },
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
        SliceMock {
            origin: format!("http://{addr}"),
            hits,
        }
    }

    fn pid(origin: &str) -> PidInfo {
        PidInfo {
            host_id: "host-a".into(),
            pid: 1,
            rpc_url: origin.into(),
            ws_url: None,
            started_at: None,
        }
    }

    fn online_state(session: crate::rpc::Session) -> AppState {
        let mut state = AppState::new();
        state.pending_discover = false;
        state.demo = false;
        state.host_status = HostStatus::Online;
        state.session_host_id = Some(session.host_id.clone());
        state.session_token = Some(session.session_token.clone());
        state.session = Some(session);
        state.workspace_id = Some("ws-1".into());
        state.selected_task_id = Some("task-1".into());
        state.selected_agent_id = Some("ag-1".into());
        state.agents.push(AgentStub {
            id: "ag-1".into(),
            task_id: "task-1".into(),
            provider: "cli.generic".into(),
            status: AgentStatus::Idle,
        });
        state
    }

    #[test]
    fn send_composer_agent_busy_sets_toast() {
        let mock = start_slice_mock(SendMode::Busy, GitMode::Ok);
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.messages.push(ChatMessage {
            id: "keep-me".into(),
            role: "user".into(),
            content: "already there".into(),
        });
        state.composer_text = "second turn".into();
        assert!(state.composer_enabled());
        state.send_composer();
        assert_eq!(state.toast.as_deref(), Some(TOAST_AGENT_BUSY));
        assert_eq!(TOAST_AGENT_BUSY, "агент занят");
        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[0].id, "keep-me");
        assert_eq!(state.composer_text, "second turn");
        let send = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "agent.send")
            .cloned()
            .expect("agent.send");
        assert_eq!(send.params["agentId"], "ag-1");
        assert_eq!(send.params["content"], "second turn");
    }

    #[test]
    fn send_composer_rpc_error_sets_toast() {
        let mock = start_slice_mock(
            SendMode::Rpc {
                code: "not_found",
                message: "agent missing",
            },
            GitMode::Ok,
        );
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.composer_text = "hello".into();
        state.send_composer();
        let toast = state.toast.clone().expect("toast");
        assert!(toast.contains("not_found"), "{toast}");
        assert!(toast.contains("agent missing"), "{toast}");
        assert!(state.messages.is_empty());
    }

    #[test]
    fn apply_get_context_replace_does_not_merge() {
        let mut messages = vec![
            ChatMessage {
                id: "stale".into(),
                role: "user".into(),
                content: "old".into(),
            },
            ChatMessage {
                id: "ctx-1".into(),
                role: "user".into(),
                content: "overlap-old".into(),
            },
        ];
        apply_get_context_replace(
            &mut messages,
            vec![
                ChatMessage {
                    id: "ctx-1".into(),
                    role: "user".into(),
                    content: "overlap-new".into(),
                },
                ChatMessage {
                    id: "ctx-2".into(),
                    role: "assistant".into(),
                    content: "ok".into(),
                },
            ],
        );
        assert_eq!(messages.len(), 2);
        assert_eq!(
            messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            vec!["ctx-1", "ctx-2"]
        );
        assert_eq!(messages[0].content, "overlap-new");
        assert!(!messages.iter().any(|m| m.id == "stale"));
    }

    #[test]
    fn reconnect_replaces_messages_via_get_context() {
        let mock = start_slice_mock(SendMode::Ok, GitMode::Ok);
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.screen = Screen::Canvas;
        state.messages = vec![
            ChatMessage {
                id: "stale-1".into(),
                role: "user".into(),
                content: "old".into(),
            },
            ChatMessage {
                id: "ctx-1".into(),
                role: "user".into(),
                content: "overlap-local".into(),
            },
        ];
        state.apply_ws_incoming(WsIncoming::Reconnected);
        let ids: Vec<_> = state.messages.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, vec!["ctx-1", "ctx-2"]);
        assert_eq!(state.messages[0].content, "hi");
        assert!(!state.messages.iter().any(|m| m.id == "stale-1"));
        let methods: Vec<_> = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .map(|h| h.method.clone())
            .collect();
        assert!(
            methods.contains(&"agent.get_context".to_string()),
            "{methods:?}"
        );
        assert!(methods.contains(&"agent.list".to_string()), "{methods:?}");
    }

    #[test]
    fn git_status_invalid_params_sets_empty_note() {
        let mock = start_slice_mock(SendMode::Ok, GitMode::InvalidParams);
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.load_git_panel();
        assert!(state.git_status.is_none());
        assert!(state.git_diff.is_none());
        assert_eq!(state.git_note.as_deref(), Some(GIT_NOTE_INVALID_PARAMS));
        assert_eq!(GIT_NOTE_INVALID_PARAMS, "нет git-статуса (invalid_params)");
        assert!(state.toast.is_none());
        let _ = mock;
    }

    fn start_cancel_mock(cancel: Result<bool, (&'static str, &'static str)>) -> SliceMock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let hits_t = hits.clone();
        thread::spawn(move || {
            for stream in listener.incoming().take(16) {
                let Ok(mut stream) = stream else { break };
                let (headers, body) = read_http_request(&mut stream);
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
                });
                let body = match method.as_str() {
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
                    "agent.cancel" => match cancel {
                        Ok(flag) => {
                            let agent_id =
                                params.get("agentId").and_then(|v| v.as_str()).unwrap_or("");
                            json!({
                                "id": "echo",
                                "ok": { "agentId": agent_id, "cancelled": flag }
                            })
                            .to_string()
                        }
                        Err((code, message)) => json!({
                            "id": "echo",
                            "error": { "code": code, "message": message }
                        })
                        .to_string(),
                    },
                    _ => json!({
                        "id": "echo",
                        "error": { "code": "unsupported_method", "message": "no" }
                    })
                    .to_string(),
                };
                write_http_json(&mut stream, &body);
            }
        });
        SliceMock {
            origin: format!("http://{addr}"),
            hits,
        }
    }

    fn running_agent_state(session: crate::rpc::Session) -> AppState {
        let mut state = online_state(session);
        if let Some(agent) = state.agents.iter_mut().find(|a| a.id == "ag-1") {
            agent.status = AgentStatus::Running;
        }
        state
    }

    fn wait_cancel(state: &mut AppState) {
        let start = Instant::now();
        while state.pending_cancel.is_some() && start.elapsed() < std::time::Duration::from_secs(2)
        {
            state.tick_rpc();
            if state.pending_cancel.is_none() {
                return;
            }
            thread::sleep(std::time::Duration::from_millis(5));
        }
        state.tick_rpc();
        assert!(
            state.pending_cancel.is_none(),
            "cancel worker did not finish"
        );
    }

    #[test]
    fn stop_button_only_when_running() {
        let mut state = AppState::new();
        state.pending_discover = false;
        state.demo = false;
        state.host_status = HostStatus::Online;
        state.selected_task_id = Some("task-1".into());
        state.selected_agent_id = Some("ag-1".into());
        state.agents.push(AgentStub {
            id: "ag-1".into(),
            task_id: "task-1".into(),
            provider: "cli.generic".into(),
            status: AgentStatus::Idle,
        });
        assert!(!state.show_stop_button());
        state.agents[0].status = AgentStatus::Error;
        assert!(!state.show_stop_button());
        state.agents[0].status = AgentStatus::Running;
        assert!(state.show_stop_button());
    }

    #[test]
    fn cancel_ok_true_sets_idle_and_enables_composer() {
        let mock = start_cancel_mock(Ok(true));
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = running_agent_state(session);
        assert!(!state.composer_enabled());
        assert!(state.show_stop_button());
        state.cancel_running_agent();
        assert_eq!(state.selected_agent().unwrap().status, AgentStatus::Running);
        assert!(!state.composer_enabled());
        wait_cancel(&mut state);
        assert_eq!(state.selected_agent().unwrap().status, AgentStatus::Idle);
        assert!(state.composer_enabled());
        assert!(!state.show_stop_button());
        assert!(state.toast.is_none());
        let hit = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "agent.cancel")
            .cloned()
            .expect("agent.cancel");
        assert_eq!(hit.params["agentId"], "ag-1");
    }

    #[test]
    fn cancel_ok_false_sets_idle_without_toast() {
        let mock = start_cancel_mock(Ok(false));
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = running_agent_state(session);
        state.cancel_running_agent();
        wait_cancel(&mut state);
        assert_eq!(state.selected_agent().unwrap().status, AgentStatus::Idle);
        assert!(state.composer_enabled());
        assert!(!state.show_stop_button());
        assert!(state.toast.is_none());
        let _ = mock;
    }

    #[test]
    fn cancel_not_found_toasts_and_keeps_running() {
        let mock = start_cancel_mock(Err(("not_found", "agent missing")));
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = running_agent_state(session);
        state.cancel_running_agent();
        wait_cancel(&mut state);
        assert_eq!(state.selected_agent().unwrap().status, AgentStatus::Running);
        assert!(!state.composer_enabled());
        assert!(state.show_stop_button());
        let toast = state.toast.clone().expect("toast");
        assert!(toast.contains("not_found"), "{toast}");
        let _ = mock;
    }

    #[test]
    fn cancel_ignored_when_idle_or_error() {
        let mock = start_cancel_mock(Ok(true));
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = running_agent_state(session);
        state.agents[0].status = AgentStatus::Idle;
        state.cancel_running_agent();
        assert!(state.pending_cancel.is_none());
        state.agents[0].status = AgentStatus::Error;
        state.cancel_running_agent();
        assert!(state.pending_cancel.is_none());
        let methods: Vec<_> = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .map(|h| h.method.clone())
            .collect();
        assert!(
            !methods.contains(&"agent.cancel".to_string()),
            "{methods:?}"
        );
    }

    fn start_e1_mock() -> SliceMock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let hits_t = hits.clone();
        thread::spawn(move || {
            for stream in listener.incoming().take(64) {
                let Ok(mut stream) = stream else { break };
                let (headers, body) = read_http_request(&mut stream);
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
                });
                let body = match method.as_str() {
                    "GET /health" => json!({"ok": true, "hostId": "host-a"}).to_string(),
                    "handshake" => json!({
                        "id": "echo",
                        "ok": {
                            "hostId": "host-a",
                            "hostVersion": "0.1.0",
                            "sessionToken": "tok-1",
                            "accepted": {},
                            "rejected": {
                                "policy.get": {"reason": "unsupported"},
                                "policy.set": {"reason": "unsupported"},
                                "approval.respond": {"reason": "unsupported"}
                            }
                        }
                    })
                    .to_string(),
                    "host.ping" => json!({
                        "id": "echo",
                        "ok": { "hostId": "host-a", "now": "2026-08-17T12:00:00Z" }
                    })
                    .to_string(),
                    "host.doctor" => json!({
                        "id": "echo",
                        "ok": {
                            "hostId": "host-a",
                            "providers": [
                                {"id": "byoa.foo", "available": true, "detail": "/bin/foo"},
                                {"id": "cli.claude", "available": false, "detail": "missing"}
                            ]
                        }
                    })
                    .to_string(),
                    "agent.list" => json!({
                        "id": "echo",
                        "ok": {
                            "items": [
                                sample_agent("ag-1", "task-1", "idle"),
                                {
                                    "id": "ag-2",
                                    "taskId": "task-1",
                                    "hostId": "host-a",
                                    "parentId": null,
                                    "interface": "chat",
                                    "provider": "byoa.foo",
                                    "status": "idle",
                                    "runLocation": "local",
                                    "createdAt": "2026-08-17T12:00:00Z"
                                }
                            ]
                        }
                    })
                    .to_string(),
                    "agent.get" => {
                        let id = params.get("id").and_then(|v| v.as_str()).unwrap_or("ag-1");
                        let provider = match id {
                            "ag-2" => "byoa.foo",
                            "ag-new" => "cli.claude",
                            _ => "cli.generic",
                        };
                        json!({
                            "id": "echo",
                            "ok": {
                                "id": id,
                                "taskId": "task-1",
                                "hostId": "host-a",
                                "parentId": null,
                                "interface": "chat",
                                "provider": provider,
                                "status": "idle",
                                "runLocation": "local",
                                "createdAt": "2026-08-17T12:00:00Z"
                            }
                        })
                        .to_string()
                    }
                    "agent.get_context" => {
                        let id = params
                            .get("agentId")
                            .and_then(|v| v.as_str())
                            .unwrap_or("ag-1");
                        let content = if id == "ag-2" {
                            "from-ag-2"
                        } else {
                            "from-ag-1"
                        };
                        json!({
                            "id": "echo",
                            "ok": {
                                "messages": [sample_message(&format!("ctx-{id}"), id, "assistant", content)]
                            }
                        })
                        .to_string()
                    }
                    "agent.create" => {
                        let task_id = params
                            .get("taskId")
                            .and_then(|v| v.as_str())
                            .unwrap_or("task-1");
                        let provider = params
                            .get("provider")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        json!({
                            "id": "echo",
                            "ok": {
                                "id": "ag-new",
                                "taskId": task_id,
                                "hostId": "host-a",
                                "parentId": null,
                                "interface": "chat",
                                "provider": provider,
                                "status": "idle",
                                "runLocation": "local",
                                "createdAt": "2026-08-17T12:00:00Z"
                            }
                        })
                        .to_string()
                    }
                    "policy.get" | "policy.set" | "approval.respond" => json!({
                        "id": "echo",
                        "error": { "code": "unsupported_method", "message": "no 1.1" }
                    })
                    .to_string(),
                    "worktree.get" => json!({
                        "id": "echo",
                        "error": { "code": "not_found", "message": "no worktree" }
                    })
                    .to_string(),
                    "git.status" => json!({
                        "id": "echo",
                        "ok": {
                            "branch": "main",
                            "dirty": false,
                            "truncated": false,
                            "entries": []
                        }
                    })
                    .to_string(),
                    "git.diff" => json!({
                        "id": "echo",
                        "ok": { "truncated": false, "files": [] }
                    })
                    .to_string(),
                    "files.tree" => json!({
                        "id": "echo",
                        "ok": { "items": [], "truncated": false }
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
        SliceMock {
            origin: format!("http://{addr}"),
            hits,
        }
    }

    fn start_ladder_ok_mock() -> SliceMock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let hits_t = hits.clone();
        thread::spawn(move || {
            for stream in listener.incoming().take(32) {
                let Ok(mut stream) = stream else { break };
                let (headers, body) = read_http_request(&mut stream);
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
                });
                let body = match method.as_str() {
                    "GET /health" => json!({"ok": true, "hostId": "host-a"}).to_string(),
                    "handshake" => json!({
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
                    "host.ping" => json!({
                        "id": "echo",
                        "ok": { "hostId": "host-a", "now": "2026-08-17T12:00:00Z" }
                    })
                    .to_string(),
                    "policy.get" => json!({
                        "id": "echo",
                        "ok": {
                            "mode": "ask",
                            "scope": "agent",
                            "yolo": false,
                            "source": "default"
                        }
                    })
                    .to_string(),
                    "policy.set" => {
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
                    "approval.respond" => json!({
                        "id": "echo",
                        "ok": { "applied": true }
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
        SliceMock {
            origin: format!("http://{addr}"),
            hits,
        }
    }

    #[test]
    fn picker_create_sends_doctor_selected_provider() {
        let mock = start_e1_mock();
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.agents.clear();
        state.selected_agent_id = None;
        state.refresh_doctor();
        assert_eq!(
            state
                .providers
                .iter()
                .map(|p| p.id.as_str())
                .collect::<Vec<_>>(),
            vec!["byoa.foo", "cli.claude"]
        );
        state.set_picker_provider("cli.claude".into());
        assert_eq!(state.picker_provider.as_deref(), Some("cli.claude"));
        assert!(state.can_create_agent());
        state.create_agent();
        assert_eq!(
            state.selected_agent().map(|a| a.provider.as_str()),
            Some("cli.claude")
        );
        let create = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "agent.create")
            .cloned()
            .expect("agent.create");
        assert_eq!(create.params["provider"], "cli.claude");
        assert_eq!(create.params["taskId"], "task-1");
    }

    #[test]
    fn apply_doctor_does_not_auto_pick_first() {
        let mock = start_e1_mock();
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.agents.clear();
        state.selected_agent_id = None;
        state.refresh_doctor();
        assert_eq!(
            state
                .providers
                .iter()
                .map(|p| p.id.as_str())
                .collect::<Vec<_>>(),
            vec!["byoa.foo", "cli.claude"]
        );
        assert!(state.picker_provider.is_none());
        assert!(!state.can_create_agent());
        state.create_agent();
        assert_eq!(state.toast.as_deref(), Some(PICKER_HINT));
        let methods: Vec<_> = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .map(|h| h.method.clone())
            .collect();
        assert!(
            !methods.contains(&"agent.create".to_string()),
            "create without pick must not send a provider: {methods:?}"
        );
    }

    #[test]
    fn apply_doctor_keeps_user_pick_and_clears_stale() {
        let mut state = AppState::new();
        state.pending_discover = false;
        state.demo = false;
        state.host_status = HostStatus::Online;
        state.selected_task_id = Some("task-1".into());
        let listed = DoctorOk {
            providers: vec![
                DoctorProvider {
                    id: "byoa.foo".into(),
                    available: true,
                    detail: "/bin/foo".into(),
                    caps: None,
                },
                DoctorProvider {
                    id: "cli.claude".into(),
                    available: false,
                    detail: "missing".into(),
                    caps: None,
                },
            ],
            ..Default::default()
        };
        state.apply_doctor(listed.clone());
        assert!(state.picker_provider.is_none());
        state.set_picker_provider("cli.claude".into());
        assert_eq!(state.picker_provider.as_deref(), Some("cli.claude"));
        state.apply_doctor(listed);
        assert_eq!(state.picker_provider.as_deref(), Some("cli.claude"));
        state.apply_doctor(DoctorOk {
            providers: vec![DoctorProvider {
                id: "byoa.foo".into(),
                available: true,
                detail: "/bin/foo".into(),
                caps: None,
            }],
            ..Default::default()
        });
        assert!(
            state.picker_provider.is_none(),
            "stale pick must clear, not fall back to first"
        );
        assert_eq!(
            state
                .providers
                .iter()
                .map(|p| p.id.as_str())
                .collect::<Vec<_>>(),
            vec!["byoa.foo"]
        );
    }

    #[test]
    fn select_agent_swaps_context() {
        let mock = start_e1_mock();
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.reload_canvas("task-1");
        assert_eq!(state.messages[0].content, "from-ag-1");
        state.select_agent("ag-2".into());
        assert_eq!(state.selected_agent_id.as_deref(), Some("ag-2"));
        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[0].content, "from-ag-2");
        assert_eq!(
            state
                .selected_agent_by_task
                .get("task-1")
                .map(String::as_str),
            Some("ag-2")
        );
        let _ = mock;
    }

    #[test]
    fn yolo_requires_explicit_confirm() {
        let mut state = AppState::new();
        state.pending_discover = false;
        state.demo = false;
        state.selected_task_id = Some("task-1".into());
        state.selected_agent_id = Some("ag-1".into());
        state.agents.push(AgentStub {
            id: "ag-1".into(),
            task_id: "task-1".into(),
            provider: "byoa.foo".into(),
            status: AgentStatus::Idle,
        });
        assert!(!state.yolo_on());
        state.request_yolo_on();
        assert!(state.show_yolo_confirm);
        assert!(!state.yolo_on());
        state.cancel_yolo_confirm();
        assert!(!state.show_yolo_confirm);
        assert!(!state.yolo_on());
        assert_eq!(crate::ladder::YOLO_CONFIRM_TITLE, "Включить Yolo?");
    }

    #[test]
    fn yolo_confirm_and_approval_card_talk_to_host() {
        let mock = start_ladder_ok_mock();
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.load_policy_for("ag-1");
        assert_eq!(state.selected_policy().mode, crate::ladder::PolicyMode::Ask);
        assert!(!state.yolo_on());
        state.request_yolo_on();
        state.confirm_yolo();
        assert!(state.yolo_on());
        assert!(!state.show_yolo_confirm);
        state.pending_approvals.insert(
            "ag-1".into(),
            crate::ladder::PendingApproval {
                approval_id: "ap-9".into(),
                agent_id: "ag-1".into(),
                task_id: "task-1".into(),
                kind: "exec".into(),
                summary: "spawn byoa.foo".into(),
            },
        );
        state.respond_approval("allow-once");
        assert!(state.selected_approval().is_none());
        let hits = mock.hits.lock().unwrap().clone();
        let set = hits.iter().find(|h| h.method == "policy.set").expect("set");
        assert_eq!(set.params["yolo"], true);
        let resp = hits
            .iter()
            .find(|h| h.method == "approval.respond")
            .expect("respond");
        assert_eq!(resp.params["approvalId"], "ap-9");
        assert_eq!(resp.params["decision"], "allow-once");
    }

    #[test]
    fn approval_card_close_sends_deny() {
        let mock = start_ladder_ok_mock();
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.pending_approvals.insert(
            "ag-1".into(),
            crate::ladder::PendingApproval {
                approval_id: "ap-9".into(),
                agent_id: "ag-1".into(),
                task_id: "task-1".into(),
                kind: "exec".into(),
                summary: "spawn byoa.foo".into(),
            },
        );
        state.close_approval_card();
        assert!(state.selected_approval().is_none());
        assert!(state.toast.is_none());
        let hits = mock.hits.lock().unwrap().clone();
        let resp = hits
            .iter()
            .find(|h| h.method == "approval.respond")
            .expect("approval.respond");
        assert_eq!(resp.params["approvalId"], "ap-9");
        assert_eq!(resp.params["decision"], "deny");
    }

    #[test]
    fn approval_card_close_deny_fail_toasts_and_keeps_card() {
        let mock = start_e1_mock();
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.pending_approvals.insert(
            "ag-1".into(),
            crate::ladder::PendingApproval {
                approval_id: "ap-9".into(),
                agent_id: "ag-1".into(),
                task_id: "task-1".into(),
                kind: "exec".into(),
                summary: "spawn byoa.foo".into(),
            },
        );
        state.close_approval_card();
        assert!(
            state.selected_approval().is_some(),
            "card must stay until host accepts deny"
        );
        let toast = state.toast.clone().expect("toast via Сообщение");
        assert!(toast.contains("unsupported_method"), "{toast}");
        let hits = mock.hits.lock().unwrap().clone();
        let resp = hits
            .iter()
            .find(|h| h.method == "approval.respond")
            .expect("approval.respond still fired");
        assert_eq!(resp.params["approvalId"], "ap-9");
        assert_eq!(resp.params["decision"], "deny");
    }

    #[test]
    fn old_host_policy_does_not_panic() {
        let mock = start_e1_mock();
        let session = connect(&pid(&mock.origin)).expect("online");
        assert!(session.ladder_rejected());
        let mut state = online_state(session);
        state.load_policy_for("ag-1");
        assert_eq!(
            state.ladder_status.as_deref(),
            Some(crate::ladder::LADDER_UNAVAILABLE)
        );
        assert!(state.toast.is_none());
        state.set_policy_mode(crate::ladder::PolicyMode::Deny);
        assert!(state.toast.is_none());
        let methods: Vec<_> = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .map(|h| h.method.clone())
            .collect();
        assert!(!methods.contains(&"policy.get".to_string()), "{methods:?}");
        assert!(!methods.contains(&"policy.set".to_string()), "{methods:?}");
    }

    #[test]
    fn task_tabs_remember_selected_agent() {
        let mut state = AppState::new();
        state.pending_discover = false;
        state.demo = true;
        state.selected_task_id = Some("task-1".into());
        state.selected_agent_id = Some("ag-1".into());
        state.tasks.push(TaskStub {
            id: "task-1".into(),
            title: "One".into(),
            status: TaskStatus::Open,
            updated_at: "t".into(),
        });
        state.tasks.push(TaskStub {
            id: "task-2".into(),
            title: "Two".into(),
            status: TaskStatus::Open,
            updated_at: "t".into(),
        });
        state.agents.push(AgentStub {
            id: "ag-1".into(),
            task_id: "task-1".into(),
            provider: "byoa.foo".into(),
            status: AgentStatus::Idle,
        });
        state.agents.push(AgentStub {
            id: "ag-9".into(),
            task_id: "task-2".into(),
            provider: "cli.claude".into(),
            status: AgentStatus::Idle,
        });
        state.open_task("task-1".into());
        state.select_agent("ag-1".into());
        state.open_task("task-2".into());
        assert_eq!(
            state.open_task_ids,
            vec!["task-1".to_string(), "task-2".to_string()]
        );
        state.selected_agent_id = Some("ag-9".into());
        state.remember_selected_agent();
        state.switch_task_tab("task-1".into());
        assert_eq!(state.selected_task_id.as_deref(), Some("task-1"));
        assert_eq!(state.selected_agent_id.as_deref(), Some("ag-1"));
    }
}
