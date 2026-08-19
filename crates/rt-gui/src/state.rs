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
use crate::terminal::{
    self, AgentInterface, AgentView, ShellStub, DEFAULT_COLS, DEFAULT_ROWS, NEED_TASK,
    TERMINAL_UNAVAILABLE,
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
    pub interface: String,
}

impl AgentStub {
    pub fn is_terminal(&self) -> bool {
        AgentInterface::from_wire(&self.interface) == AgentInterface::Terminal
    }

    pub fn interface_kind(&self) -> AgentInterface {
        AgentInterface::from_wire(&self.interface)
    }
}

impl From<rt_protocol::Agent> for AgentStub {
    fn from(agent: rt_protocol::Agent) -> Self {
        Self {
            id: agent.id,
            task_id: agent.task_id,
            provider: agent.provider,
            status: AgentStatus::from_wire(&agent.status),
            interface: if agent.interface.is_empty() {
                "chat".into()
            } else {
                agent.interface
            },
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
    pub git_commit_message: String,
    pub git_staged: HashSet<String>,
    pub show_push_confirm: bool,
    pub write_status: Option<String>,
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
    pub picker_interface: AgentInterface,
    pub agent_view: AgentView,
    pub shells: Vec<ShellStub>,
    pub selected_shell_id: Option<String>,
    pub pty_buffers: HashMap<String, String>,
    pub agent_pty: HashMap<String, String>,
    pub shell_pty: HashMap<String, String>,
    pub pty_size: HashMap<String, (u16, u16)>,
    pub pty_alive: HashSet<String>,
    pub pty_input: String,
    pub terminal_status: Option<String>,
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
            git_commit_message: String::new(),
            git_staged: HashSet::new(),
            show_push_confirm: false,
            write_status: None,
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
            picker_interface: AgentInterface::Chat,
            agent_view: AgentView::Chat,
            shells: Vec::new(),
            selected_shell_id: None,
            pty_buffers: HashMap::new(),
            agent_pty: HashMap::new(),
            shell_pty: HashMap::new(),
            pty_size: HashMap::new(),
            pty_alive: HashSet::new(),
            pty_input: String::new(),
            terminal_status: None,
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
            interface: "chat".into(),
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
                        self.refresh_terminal_capability();
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
        if let ws::WsEvent::PtyData { pty_id, data } = &event {
            self.append_pty_output(pty_id, data);
            return;
        }
        if let ws::WsEvent::PtyExit { pty_id, code } = &event {
            self.pty_alive.remove(pty_id);
            self.terminal_status = Some(format!("PTY завершился ({code})"));
            return;
        }
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
            | ApplyOutcome::Approval
            | ApplyOutcome::PtyData
            | ApplyOutcome::PtyExit => {}
        }
    }

    fn ws_subscribe(&self, task_id: &str) {
        if let Some(ws) = &self.ws {
            ws.subscribe(task_id.to_string());
        }
    }

    /// Host restart / WS reconnect: refetch and REPLACE canvas data. Never append.
    fn refresh_canvas_after_reconnect(&mut self) {
        self.clear_live_pty();
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
                self.git_staged = staged_paths_from_status(&status);
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
        if self
            .selected_agent()
            .is_some_and(|a| a.interface_kind() == AgentInterface::Terminal)
        {
            return false;
        }
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
        if self.selected_agent().is_some_and(|a| a.is_terminal()) {
            return Some(terminal::TERMINAL_AGENT_COMPOSER);
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
        self.refresh_shells();
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
        if self
            .agents
            .iter()
            .find(|a| a.id == agent_id)
            .is_some_and(|a| a.is_terminal())
        {
            self.agent_view = AgentView::Terminal;
            self.ensure_agent_pty();
        } else {
            self.agent_view = AgentView::Chat;
        }
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
        if self.picker_interface == AgentInterface::Terminal && !self.picker_allows_terminal() {
            self.toast = Some(terminal::TERMINAL_DISABLED_CAPS.into());
            return;
        }
        let Some(session) = self.session.clone() else {
            return;
        };
        if self.picker_interface == AgentInterface::Terminal && !session.terminal_accepted() {
            self.surface_terminal_error_label(TERMINAL_UNAVAILABLE);
            return;
        }
        let interface = self.picker_interface.as_wire();
        let created = if interface == "chat" {
            session.agent_create(&task_id, &provider)
        } else {
            session.agent_create_with_interface(&task_id, &provider, interface)
        };
        match created {
            Ok(agent) => {
                let stub = AgentStub::from(agent);
                if interface == "terminal" && !stub.is_terminal() {
                    self.surface_terminal_error_label(TERMINAL_UNAVAILABLE);
                }
                self.selected_agent_id = Some(stub.id.clone());
                self.remember_selected_agent();
                self.agents.push(stub);
                self.load_selected_agent();
            }
            Err(err) => {
                self.surface_terminal_or_rpc(err);
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
            if !self.picker_allows_terminal() && self.picker_interface == AgentInterface::Terminal {
                self.picker_interface = AgentInterface::Chat;
            }
        }
    }

    pub fn set_picker_interface(&mut self, interface: AgentInterface) {
        if interface == AgentInterface::Terminal && !self.picker_allows_terminal() {
            return;
        }
        self.picker_interface = interface;
    }

    pub fn picker_allows_terminal(&self) -> bool {
        self.selected_provider()
            .and_then(|p| p.caps.as_ref())
            .map(|c| c.pty)
            .unwrap_or(false)
    }

    pub fn set_agent_view(&mut self, view: AgentView) {
        self.agent_view = view;
        if view == AgentView::Terminal {
            self.ensure_agent_pty();
        }
    }

    pub fn terminal_host_ok(&self) -> bool {
        self.session
            .as_ref()
            .map(|s| s.terminal_accepted())
            .unwrap_or(false)
    }

    pub fn can_create_shell(&self) -> bool {
        self.can_rpc() && self.selected_task_id.is_some() && self.has_workspace()
    }

    pub fn create_shell(&mut self) {
        if self.selected_task_id.is_none() {
            self.toast = Some(NEED_TASK.into());
            self.terminal_status = Some(NEED_TASK.into());
            return;
        }
        if !self.has_workspace() {
            self.toast = Some("нет workspace".into());
            return;
        }
        if !self.can_rpc() {
            return;
        }
        let Some(session) = self.session.clone() else {
            return;
        };
        if !session.terminal_accepted() {
            self.surface_terminal_error_label(TERMINAL_UNAVAILABLE);
            return;
        }
        let Some(task_id) = self.selected_task_id.clone() else {
            self.toast = Some(NEED_TASK.into());
            return;
        };
        let Some(workspace_id) = self.workspace_id.clone() else {
            return;
        };
        let wt = self.worktree_id().map(|s| s.to_string());
        let agents_before = self.agents.len();
        match session.shell_create(
            &task_id,
            &workspace_id,
            wt.as_deref(),
            DEFAULT_COLS,
            DEFAULT_ROWS,
        ) {
            Ok(ok) => {
                if self.agents.len() != agents_before {
                    self.agents.truncate(agents_before);
                }
                let stub = ShellStub {
                    id: ok.shell_id.clone(),
                    pty_id: Some(ok.pty_id.clone()),
                    cwd: ok.cwd,
                };
                self.shell_pty
                    .insert(ok.shell_id.clone(), ok.pty_id.clone());
                self.pty_alive.insert(ok.pty_id.clone());
                self.pty_size
                    .insert(ok.pty_id.clone(), (DEFAULT_COLS, DEFAULT_ROWS));
                self.pty_buffers.entry(ok.pty_id).or_default();
                self.selected_shell_id = Some(ok.shell_id.clone());
                if !self.shells.iter().any(|s| s.id == stub.id) {
                    self.shells.push(stub);
                }
                self.terminal_status = None;
            }
            Err(err) => self.surface_terminal_or_rpc(err),
        }
    }

    pub fn close_selected_shell(&mut self) {
        let Some(shell_id) = self.selected_shell_id.clone() else {
            return;
        };
        let Some(session) = self.session.clone() else {
            return;
        };
        if !session.terminal_accepted() {
            self.surface_terminal_error_label(TERMINAL_UNAVAILABLE);
            return;
        }
        if let Some(pty_id) = self.shell_pty.remove(&shell_id) {
            let _ = session.pty_close(&pty_id);
            self.pty_alive.remove(&pty_id);
            self.pty_buffers.remove(&pty_id);
            self.pty_size.remove(&pty_id);
        }
        match session.shell_close(&shell_id) {
            Ok(_) => {
                self.shells.retain(|s| s.id != shell_id);
                self.selected_shell_id = self.shells.last().map(|s| s.id.clone());
            }
            Err(err) => self.surface_terminal_or_rpc(err),
        }
    }

    pub fn select_shell(&mut self, id: String) {
        self.selected_shell_id = Some(id);
    }

    pub fn selected_shell(&self) -> Option<&ShellStub> {
        let id = self.selected_shell_id.as_ref()?;
        self.shells.iter().find(|s| &s.id == id)
    }

    pub fn ensure_shell_pty(&mut self) {
        let (shell_id, existing) = match self.selected_shell() {
            Some(s) => (s.id.clone(), s.pty_id.clone()),
            None => return,
        };
        if self.shell_pty.contains_key(&shell_id) || existing.is_some() {
            if let Some(pty) = existing {
                self.shell_pty.entry(shell_id).or_insert(pty);
            }
            return;
        }
        let Some(session) = self.session.clone() else {
            return;
        };
        if !session.terminal_accepted() {
            self.terminal_status = Some(TERMINAL_UNAVAILABLE.into());
            return;
        }
        match session.pty_open_shell(&shell_id, DEFAULT_COLS, DEFAULT_ROWS) {
            Ok(ok) => {
                self.shell_pty.insert(shell_id, ok.pty_id.clone());
                self.pty_alive.insert(ok.pty_id.clone());
                self.pty_size
                    .insert(ok.pty_id.clone(), (DEFAULT_COLS, DEFAULT_ROWS));
                self.pty_buffers.entry(ok.pty_id).or_default();
                let _ = ok.resumed;
            }
            Err(err) => self.surface_terminal_or_rpc(err),
        }
    }

    pub fn selected_agent_pty_id(&self) -> Option<&str> {
        let agent_id = self.selected_agent()?.id.as_str();
        self.agent_pty.get(agent_id).map(String::as_str)
    }

    pub fn selected_shell_pty_id(&self) -> Option<&str> {
        let shell_id = self.selected_shell_id.as_ref()?;
        self.shell_pty
            .get(shell_id)
            .map(String::as_str)
            .or_else(|| {
                self.shells
                    .iter()
                    .find(|s| &s.id == shell_id)
                    .and_then(|s| s.pty_id.as_deref())
            })
    }

    pub fn pty_scrollback(&self, pty_id: &str) -> &str {
        self.pty_buffers
            .get(pty_id)
            .map(String::as_str)
            .unwrap_or("")
    }

    pub fn ensure_agent_pty(&mut self) {
        let Some(agent) = self.selected_agent() else {
            return;
        };
        if !agent.is_terminal() {
            return;
        }
        let agent_id = agent.id.clone();
        if self.agent_pty.contains_key(&agent_id) {
            return;
        }
        let Some(session) = self.session.clone() else {
            return;
        };
        if !session.terminal_accepted() {
            self.terminal_status = Some(TERMINAL_UNAVAILABLE.into());
            return;
        }
        match session.pty_open_agent(&agent_id, DEFAULT_COLS, DEFAULT_ROWS) {
            Ok(ok) => {
                self.agent_pty.insert(agent_id, ok.pty_id.clone());
                self.pty_alive.insert(ok.pty_id.clone());
                self.pty_size
                    .insert(ok.pty_id.clone(), (DEFAULT_COLS, DEFAULT_ROWS));
                self.pty_buffers.entry(ok.pty_id.clone()).or_default();
                self.terminal_status = if ok.resumed {
                    Some("PTY resume (provider session)".into())
                } else {
                    None
                };
            }
            Err(err) => self.surface_terminal_or_rpc(err),
        }
    }

    pub fn submit_pty_input(&mut self, pty_id: &str) {
        let raw = std::mem::take(&mut self.pty_input);
        if raw.is_empty() {
            return;
        }
        let mut bytes = raw.into_bytes();
        if !bytes.ends_with(b"\n") {
            bytes.push(b'\n');
        }
        self.write_pty_bytes(pty_id, &bytes);
    }

    pub fn write_pty_bytes(&mut self, pty_id: &str, data: &[u8]) {
        let Some(session) = self.session.clone() else {
            return;
        };
        if !session.terminal_accepted() {
            self.surface_terminal_error_label(TERMINAL_UNAVAILABLE);
            return;
        }
        match session.pty_write(pty_id, data) {
            Ok(_) => {
                self.terminal_status = None;
            }
            Err(err) => self.surface_terminal_or_rpc(err),
        }
    }

    pub fn maybe_resize_pty(&mut self, pty_id: &str, cols: u16, rows: u16) {
        let next = (cols, rows);
        if self.pty_size.get(pty_id) == Some(&next) {
            return;
        }
        let Some(session) = self.session.clone() else {
            return;
        };
        if !session.terminal_accepted() {
            return;
        }
        match session.pty_resize(pty_id, cols, rows) {
            Ok(_) => {
                self.pty_size.insert(pty_id.to_string(), next);
            }
            Err(err) => self.surface_terminal_or_rpc(err),
        }
    }

    fn refresh_shells(&mut self) {
        let Some(task_id) = self.selected_task_id.clone() else {
            self.shells.clear();
            self.selected_shell_id = None;
            return;
        };
        let Some(session) = self.session.clone() else {
            return;
        };
        if !session.terminal_accepted() {
            return;
        }
        match session.shell_list(&task_id) {
            Ok(items) => {
                let mut shells = Vec::new();
                for s in items {
                    if let Some(pty) = s.pty_id.clone() {
                        self.shell_pty.insert(s.shell_id.clone(), pty.clone());
                        self.pty_alive.insert(pty);
                    }
                    shells.push(ShellStub {
                        id: s.shell_id,
                        pty_id: s.pty_id,
                        cwd: s.cwd,
                    });
                }
                self.shells = shells;
                if let Some(id) = &self.selected_shell_id {
                    if !self.shells.iter().any(|s| &s.id == id) {
                        self.selected_shell_id = None;
                    }
                }
                if self.selected_shell_id.is_none() {
                    self.selected_shell_id = self.shells.first().map(|s| s.id.clone());
                }
            }
            Err(err) => self.surface_terminal_or_rpc(err),
        }
    }

    fn refresh_terminal_capability(&mut self) {
        match &self.session {
            Some(s) if s.terminal_accepted() => {
                if self.terminal_status.as_deref() == Some(TERMINAL_UNAVAILABLE) {
                    self.terminal_status = None;
                }
            }
            Some(s) if s.terminal_rejected() => {
                self.terminal_status = Some(TERMINAL_UNAVAILABLE.into());
            }
            Some(_) => {
                self.terminal_status = Some(TERMINAL_UNAVAILABLE.into());
            }
            None => {}
        }
    }

    fn clear_live_pty(&mut self) {
        self.pty_buffers.clear();
        self.agent_pty.clear();
        self.shell_pty.clear();
        self.pty_size.clear();
        self.pty_alive.clear();
        self.shells.clear();
        self.selected_shell_id = None;
    }

    fn append_pty_output(&mut self, pty_id: &str, data_b64: &str) {
        let chunk = terminal::decode_pty_data(data_b64);
        if chunk.is_empty() && data_b64.is_empty() {
            return;
        }
        let buf = self.pty_buffers.entry(pty_id.to_string()).or_default();
        terminal::append_scrollback(buf, &chunk);
        self.pty_alive.insert(pty_id.to_string());
    }

    fn surface_terminal_error_label(&mut self, label: &str) {
        self.terminal_status = Some(label.to_string());
        self.toast = Some(label.to_string());
    }

    fn surface_terminal_or_rpc(&mut self, err: ConnectError) {
        if err.is_pty_unsupported() {
            self.surface_terminal_error_label(TERMINAL_UNAVAILABLE);
            return;
        }
        if err.is_not_pty() {
            self.surface_terminal_error_label(terminal::TERMINAL_DISABLED_CAPS);
            return;
        }
        if err.is_pty_dead() {
            self.surface_terminal_error_label(&err.as_label());
            return;
        }
        let label = err.as_label();
        self.terminal_status = Some(label.clone());
        self.toast = Some(label);
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
        self.clear_live_pty();
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

    pub fn write_ready(&self) -> bool {
        self.can_rpc()
            && self
                .session
                .as_ref()
                .map(|s| s.write_accepted() && !s.write_rejected())
                .unwrap_or(false)
    }

    pub fn request_push(&mut self) {
        if !self.can_rpc() {
            self.toast = Some("недоступно: host offline".into());
            return;
        }
        if !self.write_ready() {
            self.surface_write_unavailable();
            return;
        }
        self.show_push_confirm = true;
    }

    pub fn cancel_push_confirm(&mut self) {
        self.show_push_confirm = false;
    }

    pub fn confirm_push(&mut self) {
        self.show_push_confirm = false;
        self.run_git_write(|session, workspace_id, wt| {
            let ok = session.git_push(&workspace_id, wt, None, None)?;
            if !ok.ok || ok.remote.is_empty() || ok.git_ref.is_empty() {
                return Err(ConnectError::Rpc {
                    code: "git_conflict".into(),
                    message: format!("{} {}", ok.remote, ok.git_ref),
                });
            }
            Ok(())
        });
    }

    pub fn stage_paths(&mut self, paths: Vec<String>) {
        self.mutate_git_status(paths, |session, workspace_id, wt, refs| {
            session.git_stage(&workspace_id, wt, refs)
        });
    }

    pub fn unstage_paths(&mut self, paths: Vec<String>) {
        self.mutate_git_status(paths, |session, workspace_id, wt, refs| {
            session.git_unstage(&workspace_id, wt, refs)
        });
    }

    pub fn restore_paths(&mut self, paths: Vec<String>, staged: bool) {
        self.mutate_git_status(paths, |session, workspace_id, wt, refs| {
            session.git_restore(&workspace_id, wt, refs, staged)
        });
    }

    pub fn restore_selected(&mut self) {
        if let Some(path) = self.git_selected_path.clone() {
            self.restore_paths(vec![path], false);
        }
    }

    pub fn commit_git(&mut self) {
        let message = self.git_commit_message.trim().to_string();
        if message.is_empty() {
            self.toast = Some("сообщение коммита пусто".into());
            return;
        }
        if message.len() > 4 * 1024 {
            self.toast = Some("сообщение коммита слишком длинное".into());
            return;
        }
        let ok = self.run_git_write(|session, workspace_id, wt| {
            let committed = session.git_commit(&workspace_id, wt, &message)?;
            if committed.commit.is_empty() || committed.branch.is_empty() {
                return Err(ConnectError::Transport("commit без sha или ветки".into()));
            }
            Ok(())
        });
        if ok {
            self.git_commit_message.clear();
        }
    }

    pub fn open_in_editor(&mut self, path: String) {
        if path.trim().is_empty() {
            return;
        }
        if !self.can_rpc() {
            self.toast = Some("недоступно: host offline".into());
            return;
        }
        if !self.write_ready() {
            self.surface_write_unavailable();
            return;
        }
        let Some(workspace_id) = self.workspace_id.clone() else {
            self.toast = Some("нет workspace".into());
            return;
        };
        let Some(session) = self.session.clone() else {
            self.toast = Some("недоступно: host offline".into());
            return;
        };
        let wt = self.worktree_id().map(|s| s.to_string());
        match session.files_open(&workspace_id, wt.as_deref(), &path) {
            Ok(ok) if ok.opened => {
                self.write_status = None;
            }
            Ok(_) => {
                self.toast = Some("не открыто".into());
            }
            Err(err) => self.surface_write_error(err),
        }
    }

    fn mutate_git_status<F>(&mut self, paths: Vec<String>, f: F)
    where
        F: FnOnce(
            &crate::rpc::Session,
            String,
            Option<&str>,
            &[&str],
        ) -> Result<crate::rpc::GitStatusOk, ConnectError>,
    {
        let cleaned: Vec<String> = paths
            .into_iter()
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect();
        if cleaned.is_empty() {
            return;
        }
        if !self.can_rpc() {
            self.toast = Some("недоступно: host offline".into());
            return;
        }
        if !self.write_ready() {
            self.surface_write_unavailable();
            return;
        }
        let Some(workspace_id) = self.workspace_id.clone() else {
            self.toast = Some("нет workspace".into());
            return;
        };
        let Some(session) = self.session.clone() else {
            self.toast = Some("недоступно: host offline".into());
            return;
        };
        let wt = self.worktree_id().map(|s| s.to_string());
        let refs: Vec<&str> = cleaned.iter().map(String::as_str).collect();
        match f(&session, workspace_id, wt.as_deref(), &refs) {
            Ok(status) => {
                self.write_status = None;
                self.git_staged = staged_paths_from_status(&status);
                self.git_status = Some(status);
                self.load_git_diff();
                self.load_file_tree_root();
            }
            Err(err) => self.surface_write_error(err),
        }
    }

    fn run_git_write<F>(&mut self, f: F) -> bool
    where
        F: FnOnce(&crate::rpc::Session, String, Option<&str>) -> Result<(), ConnectError>,
    {
        if !self.can_rpc() {
            self.toast = Some("недоступно: host offline".into());
            return false;
        }
        if !self.write_ready() {
            self.surface_write_unavailable();
            return false;
        }
        let Some(workspace_id) = self.workspace_id.clone() else {
            self.toast = Some("нет workspace".into());
            return false;
        };
        let Some(session) = self.session.clone() else {
            self.toast = Some("недоступно: host offline".into());
            return false;
        };
        let wt = self.worktree_id().map(|s| s.to_string());
        match f(&session, workspace_id, wt.as_deref()) {
            Ok(()) => {
                self.write_status = None;
                self.load_git_panel();
                self.load_file_tree_root();
                true
            }
            Err(err) => {
                self.surface_write_error(err);
                false
            }
        }
    }

    fn surface_write_unavailable(&mut self) {
        let label = crate::ladder::WRITE_UNAVAILABLE.to_string();
        self.write_status = Some(label.clone());
        self.toast = Some(label);
    }

    fn surface_write_error(&mut self, err: ConnectError) {
        let label = write_error_label(&err);
        if err.is_write_unsupported() {
            self.write_status = Some(crate::ladder::WRITE_UNAVAILABLE.into());
        } else {
            self.write_status = Some(label.clone());
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

    #[cfg(test)]
    pub fn test_apply_ws_event(&mut self, event: crate::ws::WsEvent) {
        self.apply_ws_event(event);
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

fn write_error_label(err: &crate::rpc::ConnectError) -> String {
    if err.is_write_unsupported() {
        return crate::ladder::WRITE_UNAVAILABLE.to_string();
    }
    match err {
        crate::rpc::ConnectError::Rpc { code, .. } if code == "git_identity" => {
            crate::ladder::GIT_IDENTITY_HINT.to_string()
        }
        crate::rpc::ConnectError::Rpc { code, .. } if code == "git_auth" => {
            crate::ladder::GIT_AUTH_HINT.to_string()
        }
        _ => err.as_label(),
    }
}

fn entry_looks_staged(status: &str) -> bool {
    let s = status.to_ascii_lowercase();
    s == "added" || s.contains("staged")
}

fn staged_paths_from_status(status: &crate::rpc::GitStatusOk) -> HashSet<String> {
    status
        .entries
        .iter()
        .filter(|e| entry_looks_staged(&e.status))
        .map(|e| e.path.clone())
        .collect()
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
            interface: "chat".into(),
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
            interface: "chat".into(),
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
            interface: "chat".into(),
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
            interface: "chat".into(),
        });
        state.agents.push(AgentStub {
            id: "ag-9".into(),
            task_id: "task-2".into(),
            provider: "cli.claude".into(),
            status: AgentStatus::Idle,
            interface: "chat".into(),
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

    fn write_accepted_hello() -> Value {
        let mut accepted = serde_json::Map::new();
        for name in crate::rpc::WRITE_METHODS {
            accepted.insert(name.to_string(), json!({"major": 1, "minor": 2}));
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
    }

    fn start_write_state_mock(mode: &'static str) -> SliceMock {
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
                let body = match (mode, method.as_str()) {
                    (_, "GET /health") => json!({"ok": true, "hostId": "host-a"}).to_string(),
                    ("ok", "handshake") => write_accepted_hello().to_string(),
                    ("old", "handshake") => json!({
                        "id": "echo",
                        "ok": {
                            "hostId": "host-a",
                            "hostVersion": "0.1.0",
                            "sessionToken": "tok-1",
                            "accepted": {},
                            "rejected": {
                                "git.stage": {"reason": "unsupported"},
                                "git.push": {"reason": "unsupported"},
                                "files.open": {"reason": "unsupported"}
                            }
                        }
                    })
                    .to_string(),
                    (_, "host.ping") => json!({
                        "id": "echo",
                        "ok": { "hostId": "host-a", "now": "2026-08-17T12:00:00Z" }
                    })
                    .to_string(),
                    ("ok", "worktree.get") => json!({
                        "id": "echo",
                        "error": { "code": "not_found", "message": "no worktree" }
                    })
                    .to_string(),
                    ("ok", "git.status" | "git.stage" | "git.unstage" | "git.restore") => json!({
                        "id": "echo",
                        "ok": {
                            "branch": "main",
                            "dirty": true,
                            "truncated": false,
                            "entries": [{ "path": "src/lib.rs", "status": "modified" }]
                        }
                    })
                    .to_string(),
                    ("ok", "git.diff") => json!({
                        "id": "echo",
                        "ok": { "truncated": false, "files": [] }
                    })
                    .to_string(),
                    ("ok", "files.tree") => json!({
                        "id": "echo",
                        "ok": { "items": [], "truncated": false }
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
        SliceMock {
            origin: format!("http://{addr}"),
            hits,
        }
    }

    fn methods_of(mock: &SliceMock) -> Vec<String> {
        mock.hits
            .lock()
            .unwrap()
            .iter()
            .map(|h| h.method.clone())
            .collect()
    }

    #[test]
    fn stage_unstage_commit_restore_open_send_right_rpc() {
        let mock = start_write_state_mock("ok");
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        assert!(state.write_ready());
        state.stage_paths(vec!["src/lib.rs".into()]);
        state.unstage_paths(vec!["src/lib.rs".into()]);
        state.git_commit_message = "feat: demo".into();
        state.commit_git();
        assert!(state.git_commit_message.is_empty());
        state.git_selected_path = Some("src/lib.rs".into());
        state.restore_selected();
        state.open_in_editor("src/lib.rs".into());
        assert!(state.toast.is_none());
        let hits = mock.hits.lock().unwrap().clone();
        let stage = hits
            .iter()
            .find(|h| h.method == "git.stage")
            .expect("stage");
        assert_eq!(stage.params["workspaceId"], "ws-1");
        assert_eq!(stage.params["paths"][0], "src/lib.rs");
        let unstage = hits
            .iter()
            .find(|h| h.method == "git.unstage")
            .expect("unstage");
        assert_eq!(unstage.params["paths"][0], "src/lib.rs");
        let commit = hits
            .iter()
            .find(|h| h.method == "git.commit")
            .expect("commit");
        assert_eq!(commit.params["message"], "feat: demo");
        let restore = hits
            .iter()
            .find(|h| h.method == "git.restore")
            .expect("restore");
        assert_eq!(restore.params["paths"][0], "src/lib.rs");
        assert_eq!(restore.params["staged"], false);
        let open = hits
            .iter()
            .find(|h| h.method == "files.open")
            .expect("files.open");
        assert_eq!(open.params["path"], "src/lib.rs");
        assert_eq!(open.params["workspaceId"], "ws-1");
    }

    #[test]
    fn push_without_confirm_does_not_fire_rpc() {
        let mock = start_write_state_mock("ok");
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.request_push();
        assert!(state.show_push_confirm);
        assert!(!methods_of(&mock).contains(&"git.push".to_string()));
        state.cancel_push_confirm();
        assert!(!state.show_push_confirm);
        assert!(!methods_of(&mock).contains(&"git.push".to_string()));
        assert_eq!(crate::ladder::PUSH_CONFIRM_TITLE, "Отправить в remote?");
    }

    #[test]
    fn confirm_then_push_fires_git_push() {
        let mock = start_write_state_mock("ok");
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.request_push();
        assert!(state.show_push_confirm);
        state.confirm_push();
        assert!(!state.show_push_confirm);
        assert!(state.toast.is_none());
        let hits = mock.hits.lock().unwrap().clone();
        let push = hits.iter().find(|h| h.method == "git.push").expect("push");
        assert_eq!(push.params["workspaceId"], "ws-1");
        assert!(push.params.get("remote").is_none());
        assert!(push.params.get("ref").is_none());
    }

    #[test]
    fn revert_fires_git_restore() {
        let mock = start_write_state_mock("ok");
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.restore_paths(vec!["README.md".into()], false);
        let hits = mock.hits.lock().unwrap().clone();
        let restore = hits
            .iter()
            .find(|h| h.method == "git.restore")
            .expect("git.restore");
        assert_eq!(restore.params["paths"][0], "README.md");
        assert_eq!(restore.params["staged"], false);
    }

    #[test]
    fn open_in_editor_fires_files_open() {
        let mock = start_write_state_mock("ok");
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.open_in_editor("Cargo.toml".into());
        let hits = mock.hits.lock().unwrap().clone();
        let open = hits
            .iter()
            .find(|h| h.method == "files.open")
            .expect("files.open");
        assert_eq!(open.params["path"], "Cargo.toml");
        assert!(
            !hits.iter().any(|h| h.method == "files.write"),
            "open must not write"
        );
    }

    #[test]
    fn old_host_write_toasts_and_does_not_panic() {
        let mock = start_write_state_mock("old");
        let session = connect(&pid(&mock.origin)).expect("online");
        assert!(!session.write_accepted());
        let mut state = online_state(session);
        state.stage_paths(vec!["src/lib.rs".into()]);
        assert_eq!(
            state.write_status.as_deref(),
            Some(crate::ladder::WRITE_UNAVAILABLE)
        );
        assert_eq!(
            state.toast.as_deref(),
            Some(crate::ladder::WRITE_UNAVAILABLE)
        );
        state.request_push();
        assert!(!state.show_push_confirm);
        state.confirm_push();
        state.open_in_editor("src/lib.rs".into());
        state.restore_paths(vec!["src/lib.rs".into()], false);
        state.git_commit_message = "x".into();
        state.commit_git();
        let methods = methods_of(&mock);
        assert!(!methods.contains(&"git.stage".to_string()), "{methods:?}");
        assert!(!methods.contains(&"git.push".to_string()), "{methods:?}");
        assert!(!methods.contains(&"git.commit".to_string()), "{methods:?}");
        assert!(!methods.contains(&"git.restore".to_string()), "{methods:?}");
        assert!(!methods.contains(&"files.open".to_string()), "{methods:?}");
        assert_eq!(
            crate::ladder::WRITE_UNAVAILABLE,
            "запись недоступна: host без 1.2"
        );
    }

    #[test]
    fn write_error_label_maps_identity_and_auth() {
        let identity = write_error_label(&ConnectError::Rpc {
            code: "git_identity".into(),
            message: "no user".into(),
        });
        assert_eq!(identity, crate::ladder::GIT_IDENTITY_HINT);
        let auth = write_error_label(&ConnectError::Rpc {
            code: "git_auth".into(),
            message: "auth".into(),
        });
        assert_eq!(auth, crate::ladder::GIT_AUTH_HINT);
        let unsupported = write_error_label(&ConnectError::Rpc {
            code: "unsupported_method".into(),
            message: "no 1.2".into(),
        });
        assert_eq!(unsupported, crate::ladder::WRITE_UNAVAILABLE);
    }

    fn offline_session_without_1_3() -> crate::rpc::Session {
        use std::collections::BTreeMap;
        let mut rejected = BTreeMap::new();
        for name in crate::rpc::PTY_METHODS {
            rejected.insert((*name).to_string(), "unsupported".into());
        }
        crate::rpc::Session {
            host_id: "host-a".into(),
            host_version: "0.1.0".into(),
            session_token: "tok-1".into(),
            rpc_url: "http://127.0.0.1:1".into(),
            ws_url: None,
            accepted: BTreeMap::new(),
            rejected,
        }
    }

    fn start_pty_state_mock() -> SliceMock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let hits_t = hits.clone();
        thread::spawn(move || {
            for stream in listener.incoming().take(24) {
                let Ok(mut stream) = stream else { break };
                let (headers, body) = read_http_request(&mut stream);
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
                    params,
                });
                let mut accepted = serde_json::Map::new();
                for name in crate::rpc::PTY_METHODS {
                    accepted.insert(name.to_string(), json!({"major": 1, "minor": 3}));
                }
                let body = match method.as_str() {
                    "GET /health" => json!({"ok": true, "hostId": "host-a"}).to_string(),
                    "handshake" => json!({
                        "id": "echo",
                        "ok": {
                            "hostId": "host-a",
                            "hostVersion": "0.1.0",
                            "sessionToken": "tok-1",
                            "accepted": accepted,
                            "rejected": {}
                        }
                    })
                    .to_string(),
                    "host.ping" => json!({
                        "id": "echo",
                        "ok": { "hostId": "host-a", "now": "2026-08-17T12:00:00Z" }
                    })
                    .to_string(),
                    "shell.create" => json!({
                        "id": "echo",
                        "ok": {
                            "shellId": "sh-1",
                            "ptyId": "pty-shell-1",
                            "cwd": "/tmp/proj"
                        }
                    })
                    .to_string(),
                    "shell.list" => json!({
                        "id": "echo",
                        "ok": { "items": [] }
                    })
                    .to_string(),
                    "pty.write" | "pty.resize" | "pty.open" | "pty.close" | "shell.close" => {
                        json!({ "id": "echo", "ok": { "ptyId": "pty-shell-1", "resumed": false } })
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
        SliceMock {
            origin: format!("http://{addr}"),
            hits,
        }
    }

    #[test]
    fn no_task_cannot_start_shell() {
        let mut state = AppState::new();
        state.pending_discover = false;
        state.demo = false;
        state.host_status = HostStatus::Online;
        state.session = Some(offline_session_without_1_3());
        state.workspace_id = Some("ws-1".into());
        state.workspaces.push(rt_protocol::Workspace {
            id: "ws-1".into(),
            host_id: "host-a".into(),
            path: "/tmp/proj".into(),
            name: "proj".into(),
            created_at: "t".into(),
        });
        assert!(state.selected_task_id.is_none());
        state.create_shell();
        assert_eq!(state.toast.as_deref(), Some(NEED_TASK));
        assert!(state.shells.is_empty());
        assert!(state.agents.is_empty());
    }

    #[test]
    fn old_host_terminal_toasts_and_does_not_panic() {
        let mut state = AppState::new();
        state.pending_discover = false;
        state.demo = false;
        state.host_status = HostStatus::Online;
        state.session = Some(offline_session_without_1_3());
        state.workspace_id = Some("ws-1".into());
        state.workspaces.push(rt_protocol::Workspace {
            id: "ws-1".into(),
            host_id: "host-a".into(),
            path: "/tmp/proj".into(),
            name: "proj".into(),
            created_at: "t".into(),
        });
        state.selected_task_id = Some("task-1".into());
        state.create_shell();
        assert_eq!(state.toast.as_deref(), Some(TERMINAL_UNAVAILABLE));
        assert_eq!(state.terminal_status.as_deref(), Some(TERMINAL_UNAVAILABLE));
        assert!(state.shells.is_empty());
        state.write_pty_bytes("pty-1", b"x");
        assert_eq!(state.toast.as_deref(), Some(TERMINAL_UNAVAILABLE));
    }

    #[test]
    fn new_terminal_creates_shell_not_agent() {
        let mock = start_pty_state_mock();
        let session = connect(&pid(&mock.origin)).expect("online");
        assert!(session.terminal_accepted());
        let mut state = online_state(session);
        state.workspaces.push(rt_protocol::Workspace {
            id: "ws-1".into(),
            host_id: "host-a".into(),
            path: "/tmp/proj".into(),
            name: "proj".into(),
            created_at: "t".into(),
        });
        let agents_before = state.agents.len();
        state.create_shell();
        assert_eq!(state.shells.len(), 1);
        assert_eq!(state.shells[0].id, "sh-1");
        assert_eq!(state.shells[0].pty_id.as_deref(), Some("pty-shell-1"));
        assert_eq!(state.selected_shell_id.as_deref(), Some("sh-1"));
        assert_eq!(state.agents.len(), agents_before);
        assert!(state.agents.iter().all(|a| a.id != "sh-1"));
        let hits = mock.hits.lock().unwrap().clone();
        assert!(hits.iter().any(|h| h.method == "shell.create"));
        assert!(!hits.iter().any(|h| h.method == "agent.create"));
    }

    #[test]
    fn pty_output_stays_out_of_chat_messages() {
        let mut state = AppState::new();
        state.pending_discover = false;
        state.demo = false;
        state.messages.push(ChatMessage {
            id: "chat-1".into(),
            role: "user".into(),
            content: "hi".into(),
        });
        let ev = crate::ws::parse_event(r#"{"type":"pty.data","ptyId":"pty-1","data":"bHMK"}"#)
            .expect("parse");
        state.test_apply_ws_event(ev);
        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[0].content, "hi");
        assert_eq!(state.pty_scrollback("pty-1"), "ls\n");
        assert!(!state.messages.iter().any(|m| m.content.contains("ls")));
    }
}
