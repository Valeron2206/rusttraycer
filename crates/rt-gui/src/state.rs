//! Session/UI state. Live host: health + handshake + ping, then workspace/task catalog.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Instant;

use serde_json::Value;

use crate::a2a::{self, InboxItem, LoopView, A2A_UNAVAILABLE, DEFAULT_ITERATIONS};
use crate::artifacts::{
    self, ArtifactKind, ArtifactStub, CommentThread, ARTIFACTS_UNAVAILABLE, CREATE_KINDS,
    EXPORT_FORMAT, EXPORT_FORMAT_PDF, FILTER_ALL,
};
use crate::discovery::{self, DiscoverError};
use crate::ladder::{
    self, AgentPolicy, PaneKind, PendingApproval, PolicyMode, SplitLayout, PICKER_EMPTY,
    PICKER_HINT,
};
use crate::model_ux::{self, ModelParams, ModelPrefs, MODEL_UNAVAILABLE, PROFILE_NAME_BAD};
use crate::rpc::{
    AgentModelView, CancelOk, ConnectError, DoctorOk, DoctorProvider, GitDiffOk, GitStatusOk,
    PrefsItem, PresetItem, ProfileOk, SettingsGuide, WorkspaceGuides, Worktree,
};
use crate::search_ux::{self, SearchItem, GC_UNAVAILABLE, SEARCH_DEBOUNCE_MS, SEARCH_UNAVAILABLE};
use crate::sync_ux::{
    self, EXPORT_BUTTON, EXPORT_SAVED, IMPORT_BUTTON, NEED_TASK as SYNC_NEED_TASK,
    NEED_WORKSPACE as SYNC_NEED_WORKSPACE, SYNC_UNAVAILABLE,
};
use crate::terminal::{
    self, AgentInterface, AgentView, ShellStub, DEFAULT_COLS, DEFAULT_ROWS, NEED_TASK,
    TERMINAL_UNAVAILABLE,
};
use crate::workspace_ux::{self, GUIDE_TOO_LONG, ROLE_CODER, WORKSPACE_UNAVAILABLE};
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
    pub parent_id: Option<String>,
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
            parent_id: agent.parent_id,
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
    pub artifacts: Vec<ArtifactStub>,
    pub selected_artifact_id: Option<String>,
    pub artifact_kind_filter: String,
    pub artifact_status_filter: String,
    pub artifact_create_kind: ArtifactKind,
    pub artifact_create_title: String,
    pub artifact_create_as_child: bool,
    pub artifact_title_draft: String,
    pub artifact_body_draft: String,
    pub artifact_status_draft: String,
    pub artifact_assignee_draft: String,
    pub artifact_editing: bool,
    pub artifact_comment_draft: String,
    pub artifact_reply_drafts: HashMap<String, String>,
    pub artifact_threads: Vec<CommentThread>,
    pub artifact_selection: Option<(usize, usize)>,
    pub artifacts_status: Option<String>,
    pub show_clear_transcript_confirm: bool,
    pub inbox: Vec<InboxItem>,
    pub deliver_target: Option<String>,
    pub deliver_text: String,
    pub loop_agent_a: Option<String>,
    pub loop_agent_b: Option<String>,
    pub loop_max_draft: String,
    pub loop_prompt: String,
    pub loop_state: Option<LoopView>,
    pub a2a_status: Option<String>,
    pub picker_model: String,
    pub picker_effort: String,
    pub picker_fast: bool,
    pub model_prefs: ModelPrefs,
    pub agent_params: HashMap<String, ModelParams>,
    pub profiles: Vec<ProfileOk>,
    pub selected_profile_id: Option<String>,
    pub profile_name_draft: String,
    pub host_prefs: Vec<PrefsItem>,
    pub model_status: Option<String>,
    pub new_task_preset: Option<String>,
    pub picker_role: String,
    pub agent_roles: HashMap<String, String>,
    pub task_presets: HashMap<String, String>,
    pub workspace_guides: Option<WorkspaceGuides>,
    pub settings_guide_draft: String,
    pub settings_guide_path: String,
    pub settings_guide_truncated: bool,
    pub settings_guide_loaded: bool,
    pub presets: Vec<PresetItem>,
    pub workspace_status: Option<String>,
    pub sync_status: Option<String>,
    pub show_sync_import_confirm: bool,
    pub search_q: String,
    pub search_items: Vec<SearchItem>,
    pub search_status: Option<String>,
    pub search_ran: bool,
    pub show_worktree_gc_confirm: bool,
    search_edited_at: Option<Instant>,
    last_search_q: Option<String>,
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
            artifacts: Vec::new(),
            selected_artifact_id: None,
            artifact_kind_filter: FILTER_ALL.into(),
            artifact_status_filter: FILTER_ALL.into(),
            artifact_create_kind: ArtifactKind::Spec,
            artifact_create_title: String::new(),
            artifact_create_as_child: false,
            artifact_title_draft: String::new(),
            artifact_body_draft: String::new(),
            artifact_status_draft: String::new(),
            artifact_assignee_draft: String::new(),
            artifact_editing: false,
            artifact_comment_draft: String::new(),
            artifact_reply_drafts: HashMap::new(),
            artifact_threads: Vec::new(),
            artifact_selection: None,
            artifacts_status: None,
            show_clear_transcript_confirm: false,
            inbox: Vec::new(),
            deliver_target: None,
            deliver_text: String::new(),
            loop_agent_a: None,
            loop_agent_b: None,
            loop_max_draft: DEFAULT_ITERATIONS.to_string(),
            loop_prompt: String::new(),
            loop_state: None,
            a2a_status: None,
            picker_model: String::new(),
            picker_effort: String::new(),
            picker_fast: false,
            model_prefs: ModelPrefs::default(),
            agent_params: HashMap::new(),
            profiles: Vec::new(),
            selected_profile_id: None,
            profile_name_draft: String::new(),
            host_prefs: Vec::new(),
            model_status: None,
            new_task_preset: None,
            picker_role: ROLE_CODER.into(),
            agent_roles: HashMap::new(),
            task_presets: HashMap::new(),
            workspace_guides: None,
            settings_guide_draft: String::new(),
            settings_guide_path: String::new(),
            settings_guide_truncated: false,
            settings_guide_loaded: false,
            presets: workspace_ux::builtin_presets(),
            workspace_status: None,
            sync_status: None,
            show_sync_import_confirm: false,
            search_q: String::new(),
            search_items: Vec::new(),
            search_status: None,
            search_ran: false,
            show_worktree_gc_confirm: false,
            search_edited_at: None,
            last_search_q: None,
            pending_cancel: None,
            rpc_tx,
            rpc_rx,
        };

        state.apply_local_model_prefs(model_ux::load_model_prefs());
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
            parent_id: None,
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
        self.tick_search_debounce();
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
        self.pending_cancel.is_some()
            || self.search_edited_at.is_some()
            || (self.ws.is_some() && self.screen == Screen::Canvas)
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
                        self.refresh_artifacts_capability();
                        self.refresh_a2a_capability();
                        self.refresh_model_capability();
                        self.refresh_workspace_capability();
                        self.refresh_sync_capability();
                        self.refresh_search_gc_capability();
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
        if let ws::WsEvent::ArtifactUpdated {
            artifact_id,
            task_id,
        } = &event
        {
            if self.selected_task_id.as_deref() == Some(task_id.as_str()) {
                self.load_artifacts();
                if self.selected_artifact_id.as_deref() == Some(artifact_id.as_str()) {
                    self.load_artifact_detail(artifact_id);
                }
            }
            return;
        }
        if let ws::WsEvent::ArtifactDeleted {
            artifact_id,
            task_id,
        } = &event
        {
            if self.selected_task_id.as_deref() == Some(task_id.as_str()) {
                if self.selected_artifact_id.as_deref() == Some(artifact_id.as_str()) {
                    self.selected_artifact_id = None;
                    self.artifact_threads.clear();
                }
                self.load_artifacts();
            }
            return;
        }
        if let ws::WsEvent::A2aDelivered {
            from_agent_id,
            to_agent_id,
            message_id,
        } = &event
        {
            self.push_inbox_item(InboxItem {
                from_agent_id: from_agent_id.clone(),
                to_agent_id: to_agent_id.clone(),
                message_id: message_id.clone(),
                content: String::new(),
            });
            return;
        }
        if let ws::WsEvent::LoopStopped { loop_id, reason } = &event {
            if let Some(loop_state) = self.loop_state.as_mut() {
                if loop_state.id == *loop_id {
                    loop_state.status = "stopped".into();
                    loop_state.reason = if reason.is_empty() {
                        None
                    } else {
                        Some(reason.clone())
                    };
                }
            }
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
            ApplyOutcome::ArtifactChanged => {
                if self.is_online() {
                    self.load_artifacts();
                }
            }
            ApplyOutcome::Appended
            | ApplyOutcome::Deduped
            | ApplyOutcome::Ignored
            | ApplyOutcome::Approval
            | ApplyOutcome::PtyData
            | ApplyOutcome::PtyExit
            | ApplyOutcome::A2aDelivered
            | ApplyOutcome::LoopStopped => {}
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
        self.load_artifacts();
        self.load_workspace_guides();
        self.canvas_loaded_for = Some(task_id.to_string());
    }

    fn reload_agents(&mut self, task_id: &str) {
        let Some(session) = self.session.clone() else {
            return;
        };
        let listed = if self.workspace_host_ok() {
            session.agent_list_with_roles(task_id).map(|items| {
                items
                    .into_iter()
                    .map(|(agent, role)| {
                        let stub = AgentStub::from(agent);
                        if let Some(role) = role {
                            self.agent_roles.insert(stub.id.clone(), role);
                        }
                        stub
                    })
                    .collect::<Vec<_>>()
            })
        } else {
            session
                .agent_list(task_id)
                .map(|items| items.into_iter().map(AgentStub::from).collect())
        };
        match listed {
            Ok(items) => {
                self.agents.retain(|a| a.task_id != task_id);
                self.agents.extend(items);
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
                self.ingest_inbox_from_messages(&agent_id);
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
        let params = self.picker_params();
        let role = if self.workspace_host_ok() {
            Some(self.picker_role.clone())
        } else {
            None
        };
        let created = if self.workspace_host_ok() {
            session.agent_create_with_role(&task_id, &provider, interface, &params, role.as_deref())
        } else if self.model_ux_host_ok() {
            session
                .agent_create_with_model(
                    &task_id,
                    &provider,
                    interface,
                    params.model.as_deref(),
                    params.effort.as_deref(),
                    Some(params.fast),
                )
                .map(|agent| (agent, None))
        } else if interface == "chat" {
            session
                .agent_create(&task_id, &provider)
                .map(|agent| (agent, None))
        } else {
            session
                .agent_create_with_interface(&task_id, &provider, interface)
                .map(|agent| (agent, None))
        };
        match created {
            Ok((agent, got_role)) => {
                let stub = AgentStub::from(agent);
                if let Some(role) = got_role.or(role) {
                    self.agent_roles.insert(stub.id.clone(), role);
                }
                if interface == "terminal" && !stub.is_terminal() {
                    self.surface_terminal_error_label(TERMINAL_UNAVAILABLE);
                }
                self.agent_params.insert(stub.id.clone(), params);
                self.remember_model_choice();
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
        self.sync_picker_role_from_selected();
        self.remember_selected_agent();
        self.load_selected_agent();
    }

    pub fn set_picker_provider(&mut self, id: String) {
        if self.providers.iter().any(|p| p.id == id) {
            self.picker_provider = Some(id.clone());
            if !self.picker_allows_terminal() && self.picker_interface == AgentInterface::Terminal {
                self.picker_interface = AgentInterface::Chat;
            }
            self.apply_remembered_for_provider(&id);
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
            self.task_presets = catalog.task_presets.into_iter().collect();
            self.load_workspace_guides();
            self.load_presets();
        } else {
            self.workspace_id = None;
            self.workspace_path = None;
            self.tasks.clear();
            self.task_presets.clear();
            self.workspace_guides = None;
        }
    }

    fn clear_host_catalog(&mut self) {
        self.workspaces.clear();
        self.workspace_id = None;
        self.workspace_path = None;
        self.tasks.clear();
        self.clear_live_pty();
        self.artifacts.clear();
        self.artifact_threads.clear();
        self.selected_artifact_id = None;
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
        let preset = self
            .new_task_preset
            .clone()
            .filter(|p| workspace_ux::valid_preset(p));
        if preset.is_some() && !self.workspace_host_ok() {
            self.surface_workspace_unavailable();
        }
        let send_preset = if self.workspace_host_ok() {
            preset.as_deref()
        } else {
            None
        };
        let created = if let Some(preset) = send_preset {
            session.task_create_with_preset(&title, &workspace_id, Some(preset))
        } else {
            session.task_create(&title, &workspace_id)
        };
        match created {
            Ok(task) => {
                if let Some(preset) = send_preset {
                    self.task_presets
                        .insert(task.id.clone(), preset.to_string());
                }
                self.new_task_title.clear();
                self.new_task_preset = None;
                self.show_new_task_dialog = false;
                self.refresh_tasks_catalog();
                self.open_task(task.id);
            }
            Err(err) => {
                if err.is_workspace_unsupported() {
                    self.surface_workspace_error(err);
                } else {
                    self.toast = Some(err.as_label());
                }
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

    pub fn artifacts_host_ok(&self) -> bool {
        self.session
            .as_ref()
            .map(|s| s.artifacts_accepted() && !s.artifacts_rejected())
            .unwrap_or(false)
    }

    pub fn comments_visible(&self) -> bool {
        self.selected_artifact_id.is_some()
    }

    pub fn can_create_artifact(&self) -> bool {
        self.can_rpc() && self.selected_task_id.is_some() && self.artifacts_host_ok()
    }

    pub fn can_clear_transcript(&self) -> bool {
        self.can_rpc()
            && self.selected_task_id.is_some()
            && self.selected_agent().is_some()
            && self.artifacts_host_ok()
    }

    pub fn selected_artifact(&self) -> Option<&ArtifactStub> {
        let id = self.selected_artifact_id.as_ref()?;
        self.artifacts.iter().find(|a| &a.id == id)
    }

    fn refresh_artifacts_capability(&mut self) {
        match &self.session {
            Some(s) if s.artifacts_accepted() && !s.artifacts_rejected() => {
                if self.artifacts_status.as_deref() == Some(ARTIFACTS_UNAVAILABLE) {
                    self.artifacts_status = None;
                }
            }
            Some(_) | None => {
                if self.can_rpc() {
                    self.artifacts_status = Some(ARTIFACTS_UNAVAILABLE.into());
                }
            }
        }
    }

    fn surface_artifacts_error(&mut self, err: ConnectError) {
        if err.is_artifacts_unsupported() {
            self.artifacts_status = Some(ARTIFACTS_UNAVAILABLE.into());
            self.toast = Some(ARTIFACTS_UNAVAILABLE.into());
        } else {
            let label = err.as_label();
            self.artifacts_status = Some(label.clone());
            self.toast = Some(label);
        }
    }

    fn load_artifacts(&mut self) {
        let Some(task_id) = self.selected_task_id.clone() else {
            self.artifacts.clear();
            self.artifact_threads.clear();
            self.selected_artifact_id = None;
            return;
        };
        let Some(session) = self.session.clone() else {
            return;
        };
        if !session.artifacts_accepted() || session.artifacts_rejected() {
            self.artifacts_status = Some(ARTIFACTS_UNAVAILABLE.into());
            return;
        }
        match session.artifact_list(&task_id, None) {
            Ok(list) => {
                if list.truncated && self.artifacts_status.is_none() {
                    self.artifacts_status = Some("список артефактов усечён".into());
                }
                self.artifacts = list.items.into_iter().map(ArtifactStub::from).collect();
                if self
                    .selected_artifact_id
                    .as_ref()
                    .map(|id| !self.artifacts.iter().any(|a| &a.id == id))
                    .unwrap_or(false)
                {
                    self.selected_artifact_id = None;
                    self.artifact_threads.clear();
                }
                if let Some(id) = self.selected_artifact_id.clone() {
                    self.load_artifact_detail(&id);
                }
                if self.artifacts_status.as_deref() == Some(ARTIFACTS_UNAVAILABLE) {
                    self.artifacts_status = None;
                }
            }
            Err(err) => self.surface_artifacts_error(err),
        }
    }

    fn load_artifact_detail(&mut self, artifact_id: &str) {
        let Some(session) = self.session.clone() else {
            return;
        };
        if !session.artifacts_accepted() {
            return;
        }
        match session.artifact_get(artifact_id) {
            Ok(ok) => {
                let stub = ArtifactStub::from(ok);
                if let Some(existing) = self.artifacts.iter_mut().find(|a| a.id == stub.id) {
                    *existing = stub.clone();
                }
                self.artifact_title_draft = stub.title.clone();
                self.artifact_body_draft = stub.body.clone();
                self.artifact_status_draft = stub.status.clone().unwrap_or_default();
                self.artifact_assignee_draft = stub.assignee.clone().unwrap_or_default();
            }
            Err(err) => self.surface_artifacts_error(err),
        }
        match session.comment_list(artifact_id) {
            Ok(list) => {
                self.artifact_threads = list.threads.into_iter().map(CommentThread::from).collect();
            }
            Err(err) => self.surface_artifacts_error(err),
        }
    }

    pub fn select_artifact(&mut self, id: String) {
        self.selected_artifact_id = Some(id.clone());
        self.artifact_editing = false;
        self.artifact_selection = None;
        self.load_artifact_detail(&id);
    }

    pub fn create_artifact(&mut self) {
        if self.selected_task_id.is_none() {
            self.toast = Some(NEED_TASK.into());
            return;
        }
        if !self.can_create_artifact() {
            if self.can_rpc() && !self.artifacts_host_ok() {
                self.toast = Some(ARTIFACTS_UNAVAILABLE.into());
                self.artifacts_status = Some(ARTIFACTS_UNAVAILABLE.into());
            }
            return;
        }
        let Some(task_id) = self.selected_task_id.clone() else {
            return;
        };
        let title = self.artifact_create_title.trim().to_string();
        if title.is_empty() {
            self.toast = Some(artifacts::CREATE_TITLE_HINT.into());
            return;
        }
        let kind = self.artifact_create_kind.as_wire();
        if !CREATE_KINDS.contains(&kind) {
            return;
        }
        let parent = if self.artifact_create_as_child {
            self.selected_artifact_id.clone()
        } else {
            None
        };
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.artifact_create(&task_id, kind, &title, "", parent.as_deref(), None) {
            Ok(ok) => {
                self.artifact_create_title.clear();
                let stub = ArtifactStub::from(ok);
                let id = stub.id.clone();
                self.artifacts.push(stub);
                self.select_artifact(id);
            }
            Err(err) => self.surface_artifacts_error(err),
        }
    }

    pub fn save_artifact_body(&mut self) {
        let Some(id) = self.selected_artifact_id.clone() else {
            return;
        };
        if !self.artifacts_host_ok() {
            self.toast = Some(ARTIFACTS_UNAVAILABLE.into());
            return;
        }
        let Some(session) = self.session.clone() else {
            return;
        };
        let title = self.artifact_title_draft.trim().to_string();
        let body = self.artifact_body_draft.clone();
        let allows_status = self
            .selected_artifact()
            .map(|a| a.allows_status())
            .unwrap_or(false);
        let status = if allows_status && !self.artifact_status_draft.trim().is_empty() {
            Some(self.artifact_status_draft.trim())
        } else {
            None
        };
        let assignee = if allows_status && !self.artifact_assignee_draft.trim().is_empty() {
            Some(self.artifact_assignee_draft.trim())
        } else {
            None
        };
        match session.artifact_update(&id, Some(&title), Some(&body), status, assignee, None) {
            Ok(ok) => {
                let stub = ArtifactStub::from(ok);
                if let Some(existing) = self.artifacts.iter_mut().find(|a| a.id == stub.id) {
                    *existing = stub;
                }
                self.artifact_editing = false;
            }
            Err(err) => self.surface_artifacts_error(err),
        }
    }

    pub fn set_artifact_status(&mut self, status: String) {
        self.artifact_status_draft = status;
        self.save_artifact_body();
    }

    pub fn delete_selected_artifact(&mut self) {
        let Some(id) = self.selected_artifact_id.clone() else {
            return;
        };
        if !self.artifacts_host_ok() {
            self.toast = Some(ARTIFACTS_UNAVAILABLE.into());
            return;
        }
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.artifact_delete(&id) {
            Ok(ok) => {
                self.artifacts
                    .retain(|a| !ok.deleted.iter().any(|d| d == &a.id));
                self.selected_artifact_id = None;
                self.artifact_threads.clear();
            }
            Err(err) => self.surface_artifacts_error(err),
        }
    }

    pub fn export_selected_markdown(&mut self) -> Option<(String, String)> {
        let id = self.selected_artifact_id.clone()?;
        if !self.artifacts_host_ok() {
            self.toast = Some(ARTIFACTS_UNAVAILABLE.into());
            return None;
        }
        let session = self.session.clone()?;
        match session.artifact_export(&id, EXPORT_FORMAT) {
            Ok(ok) => {
                let filename = artifacts::export_suggested_filename(&id, &ok.filename, "md");
                // Nit 0081: host may echo format=pdf on the MD path. Save markdown anyway.
                let _echoed_format = ok.format.as_str();
                Some((filename, ok.markdown))
            }
            Err(err) => {
                self.surface_artifacts_error(err);
                None
            }
        }
    }

    pub fn save_exported_markdown(&mut self, filename: &str, markdown: &str) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title(artifacts::EXPORT_MARKDOWN)
            .set_file_name(filename)
            .add_filter("Markdown", &["md"])
            .save_file()
        {
            match std::fs::write(&path, markdown) {
                Ok(()) => self.toast = Some(artifacts::EXPORT_SAVED.into()),
                Err(err) => self.toast = Some(err.to_string()),
            }
        }
    }

    pub fn export_selected_pdf(&mut self) -> Option<(String, Vec<u8>)> {
        let id = self.selected_artifact_id.clone()?;
        if !self.artifacts_host_ok() {
            self.toast = Some(ARTIFACTS_UNAVAILABLE.into());
            return None;
        }
        let session = self.session.clone()?;
        match session.artifact_export(&id, EXPORT_FORMAT_PDF) {
            Ok(ok) => {
                let filename = artifacts::export_suggested_filename(&id, &ok.filename, "pdf");
                match artifacts::decode_export_pdf(&ok.bytes, &ok.markdown) {
                    Ok(bytes) => Some((filename, bytes)),
                    Err(msg) => {
                        self.toast = Some(msg);
                        None
                    }
                }
            }
            Err(err) => {
                // 1.8 host: invalid_params / unsupported. Toast only — do not hide MD.
                self.toast = Some(err.as_label());
                None
            }
        }
    }

    pub fn save_exported_pdf(&mut self, filename: &str, bytes: &[u8]) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title(artifacts::EXPORT_PDF)
            .set_file_name(filename)
            .add_filter("PDF", &["pdf"])
            .save_file()
        {
            match std::fs::write(&path, bytes) {
                Ok(()) => self.toast = Some(artifacts::EXPORT_PDF_SAVED.into()),
                Err(err) => self.toast = Some(err.to_string()),
            }
        }
    }

    pub fn request_clear_transcript(&mut self) {
        if self.selected_task_id.is_none() {
            self.toast = Some(NEED_TASK.into());
            return;
        }
        if self.selected_agent().is_none() {
            return;
        }
        if !self.artifacts_host_ok() {
            self.toast = Some(ARTIFACTS_UNAVAILABLE.into());
            self.artifacts_status = Some(ARTIFACTS_UNAVAILABLE.into());
            return;
        }
        self.show_clear_transcript_confirm = true;
    }

    pub fn cancel_clear_transcript(&mut self) {
        self.show_clear_transcript_confirm = false;
    }

    pub fn confirm_clear_transcript(&mut self) {
        self.show_clear_transcript_confirm = false;
        if !self.can_clear_transcript() {
            if self.selected_task_id.is_none() {
                self.toast = Some(NEED_TASK.into());
            } else if self.can_rpc() && !self.artifacts_host_ok() {
                self.toast = Some(ARTIFACTS_UNAVAILABLE.into());
            }
            return;
        }
        let Some(agent_id) = self.selected_agent().map(|a| a.id.clone()) else {
            return;
        };
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.agent_clear_transcript(&agent_id) {
            Ok(ok) => {
                let _ = ok.cleared;
                self.messages.clear();
            }
            Err(err) => self.surface_artifacts_error(err),
        }
    }

    pub fn open_comment_thread(&mut self) {
        let Some(artifact_id) = self.selected_artifact_id.clone() else {
            return;
        };
        let Some((start, end)) = self
            .artifact_selection
            .and_then(|(s, e)| artifacts::utf8_range(s, e))
        else {
            self.toast = Some(artifacts::NO_SELECTION.into());
            return;
        };
        let body = self.artifact_comment_draft.trim().to_string();
        if body.is_empty() {
            self.toast = Some(artifacts::COMMENT_HINT.into());
            return;
        }
        if !self.artifacts_host_ok() {
            self.toast = Some(ARTIFACTS_UNAVAILABLE.into());
            return;
        }
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.comment_create(
            &artifact_id,
            None,
            Some(start as i64),
            Some(end as i64),
            &body,
        ) {
            Ok(thread) => {
                self.artifact_comment_draft.clear();
                self.artifact_threads.push(CommentThread::from(thread));
            }
            Err(err) => self.surface_artifacts_error(err),
        }
    }

    pub fn reply_comment(&mut self, thread_id: String) {
        let Some(artifact_id) = self.selected_artifact_id.clone() else {
            return;
        };
        let body = self
            .artifact_reply_drafts
            .get(&thread_id)
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        if body.is_empty() {
            return;
        }
        if !self.artifacts_host_ok() {
            self.toast = Some(ARTIFACTS_UNAVAILABLE.into());
            return;
        }
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.comment_create(&artifact_id, Some(&thread_id), None, None, &body) {
            Ok(thread) => {
                self.artifact_reply_drafts.remove(&thread_id);
                let updated = CommentThread::from(thread);
                if let Some(existing) = self
                    .artifact_threads
                    .iter_mut()
                    .find(|t| t.id == updated.id)
                {
                    *existing = updated;
                } else {
                    self.artifact_threads.push(updated);
                }
            }
            Err(err) => self.surface_artifacts_error(err),
        }
    }

    pub fn resolve_comment(&mut self, thread_id: String) {
        if !self.artifacts_host_ok() {
            self.toast = Some(ARTIFACTS_UNAVAILABLE.into());
            return;
        }
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.comment_resolve(&thread_id) {
            Ok(thread) => {
                let updated = CommentThread::from(thread);
                if let Some(existing) = self
                    .artifact_threads
                    .iter_mut()
                    .find(|t| t.id == updated.id)
                {
                    *existing = updated;
                }
            }
            Err(err) => self.surface_artifacts_error(err),
        }
    }

    pub fn a2a_host_ok(&self) -> bool {
        self.session
            .as_ref()
            .map(|s| s.a2a_accepted() && !s.a2a_rejected())
            .unwrap_or(false)
    }

    fn refresh_a2a_capability(&mut self) {
        match &self.session {
            Some(s) if s.a2a_accepted() && !s.a2a_rejected() => {
                if self.a2a_status.as_deref() == Some(A2A_UNAVAILABLE) {
                    self.a2a_status = None;
                }
            }
            Some(_) | None => {
                if self.can_rpc() {
                    self.a2a_status = Some(A2A_UNAVAILABLE.into());
                }
            }
        }
    }

    fn surface_a2a_unavailable(&mut self) {
        self.a2a_status = Some(A2A_UNAVAILABLE.into());
        self.toast = Some(A2A_UNAVAILABLE.into());
    }

    fn surface_a2a_error(&mut self, err: ConnectError) {
        if err.is_a2a_unsupported() {
            self.surface_a2a_unavailable();
        } else {
            let label = err.as_label();
            self.a2a_status = Some(label.clone());
            self.toast = Some(label);
        }
    }

    pub fn agent_has_inbox(&self, agent: &AgentStub) -> bool {
        self.providers
            .iter()
            .find(|p| p.id == agent.provider)
            .and_then(|p| p.caps.as_ref())
            .map(|c| c.a2a_inbox)
            .unwrap_or(false)
    }

    pub fn selected_inbox_live(&self) -> bool {
        self.selected_agent()
            .map(|a| self.agent_has_inbox(a))
            .unwrap_or(false)
    }

    pub fn inbox_for_selected(&self) -> Vec<&InboxItem> {
        let Some(id) = self.selected_agent().map(|a| a.id.as_str()) else {
            return Vec::new();
        };
        self.inbox
            .iter()
            .filter(|item| item.to_agent_id == id)
            .collect()
    }

    fn push_inbox_item(&mut self, item: InboxItem) {
        a2a::merge_inbox_item(&mut self.inbox, item);
    }

    fn ingest_inbox_from_messages(&mut self, agent_id: &str) {
        for msg in &self.messages {
            if let Some(item) =
                a2a::inbox_item_from_message(agent_id, &msg.id, &msg.role, &msg.content)
            {
                a2a::merge_inbox_item(&mut self.inbox, item);
            }
        }
    }

    pub fn can_create_child(&self) -> bool {
        self.can_create_agent() && self.selected_agent().is_some()
    }

    pub fn create_child_conversation(&mut self) {
        if !self.can_create_child() {
            if self.selected_agent().is_none() {
                self.toast = Some("сначала создайте агента".into());
            } else if self.providers.is_empty() {
                self.toast = Some(PICKER_EMPTY.into());
            } else if self.picker_provider.is_none() {
                self.toast = Some(PICKER_HINT.into());
            }
            return;
        }
        if !self.a2a_host_ok() {
            self.surface_a2a_unavailable();
            return;
        }
        let Some(task_id) = self.selected_task_id.clone() else {
            return;
        };
        let Some(parent_id) = self.selected_agent().map(|a| a.id.clone()) else {
            return;
        };
        let Some(provider) = self.picker_provider.clone() else {
            self.toast = Some(PICKER_HINT.into());
            return;
        };
        if self.picker_interface == AgentInterface::Terminal && !self.picker_allows_terminal() {
            self.toast = Some(terminal::TERMINAL_DISABLED_CAPS.into());
            return;
        }
        let Some(session) = self.session.clone() else {
            return;
        };
        let interface = self.picker_interface.as_wire();
        match session.agent_create_child(&task_id, &provider, interface, &parent_id) {
            Ok(agent) => {
                let stub = AgentStub::from(agent);
                self.selected_agent_id = Some(stub.id.clone());
                self.remember_selected_agent();
                self.agents.push(stub);
                self.load_selected_agent();
            }
            Err(err) => self.surface_a2a_error(err),
        }
    }

    /// Drop one agent. Surviving children stay on the same Task (C46).
    pub fn remove_agent(&mut self, id: &str) {
        self.agents.retain(|a| a.id != id);
        for agent in &mut self.agents {
            if agent.parent_id.as_deref() == Some(id) {
                agent.parent_id = None;
            }
        }
        self.inbox
            .retain(|item| item.to_agent_id != id && item.from_agent_id != id);
        if self.selected_agent_id.as_deref() == Some(id) {
            self.selected_agent_id = self.selected_task_id.as_ref().and_then(|task| {
                self.agents
                    .iter()
                    .find(|a| &a.task_id == task)
                    .map(|a| a.id.clone())
            });
            if self.selected_agent_id.is_some() {
                self.remember_selected_agent();
                self.load_selected_agent();
            } else {
                self.messages.clear();
            }
        }
    }

    pub fn mention_targets(&self) -> Vec<&AgentStub> {
        self.agents_for_selected_task()
    }

    pub fn can_deliver_to(&self, to_id: &str) -> bool {
        self.a2a_host_ok()
            && self
                .agents
                .iter()
                .find(|a| a.id == to_id)
                .is_some_and(|a| self.agent_has_inbox(a))
    }

    pub fn deliver_to_selected_target(&mut self) {
        if !self.a2a_host_ok() {
            self.surface_a2a_unavailable();
            return;
        }
        let Some(from) = self.selected_agent().map(|a| a.id.clone()) else {
            self.toast = Some("сначала создайте агента".into());
            return;
        };
        let Some(to) = self.deliver_target.clone() else {
            return;
        };
        if from == to {
            return;
        }
        if !self.can_deliver_to(&to) {
            self.toast = Some(a2a::INBOX_OFF.into());
            return;
        }
        let content = self.deliver_text.trim().to_string();
        if content.is_empty() {
            return;
        }
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.a2a_deliver(&from, &to, &content) {
            Ok(ok) => {
                self.push_inbox_item(InboxItem {
                    from_agent_id: from,
                    to_agent_id: ok.to_agent_id,
                    message_id: ok.message_id,
                    content,
                });
                self.deliver_text.clear();
            }
            Err(err) => self.surface_a2a_error(err),
        }
    }

    pub fn loop_max_value(&self) -> u32 {
        a2a::parse_max_iterations(&self.loop_max_draft)
    }

    pub fn can_start_loop(&self) -> bool {
        self.can_rpc()
            && self.selected_task_id.is_some()
            && self.loop_agent_a.is_some()
            && self.loop_agent_b.is_some()
            && self.loop_agent_a != self.loop_agent_b
    }

    pub fn start_loop(&mut self) {
        if !self.a2a_host_ok() {
            self.surface_a2a_unavailable();
            return;
        }
        if !self.can_start_loop() {
            return;
        }
        let Some(task_id) = self.selected_task_id.clone() else {
            return;
        };
        let Some(a) = self.loop_agent_a.clone() else {
            return;
        };
        let Some(b) = self.loop_agent_b.clone() else {
            return;
        };
        if a2a::allows_infinite_loop() {
            return;
        }
        let max = self.loop_max_value();
        let prompt = self.loop_prompt.clone();
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.loop_start(&task_id, &a, &b, max, &prompt) {
            Ok(ok) => {
                let max_iterations = ok.display_max(max);
                self.loop_state = Some(LoopView {
                    id: ok.loop_id,
                    iteration: ok.iteration,
                    max_iterations,
                    turns: ok.turns,
                    status: ok.status.unwrap_or_else(|| "running".into()),
                    reason: ok.reason,
                });
            }
            Err(err) => self.surface_a2a_error(err),
        }
    }

    pub fn stop_loop(&mut self) {
        if !self.a2a_host_ok() {
            self.surface_a2a_unavailable();
            return;
        }
        let Some(loop_id) = self.loop_state.as_ref().map(|l| l.id.clone()) else {
            return;
        };
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.loop_stop(&loop_id) {
            Ok(ok) => {
                let fallback = self
                    .loop_state
                    .as_ref()
                    .map(|l| l.max_iterations)
                    .unwrap_or(self.loop_max_value());
                let max_iterations = ok.display_max(fallback);
                self.loop_state = Some(LoopView {
                    id: ok.loop_id,
                    iteration: ok.iteration,
                    max_iterations,
                    turns: ok.turns,
                    status: ok.status.unwrap_or_else(|| "stopped".into()),
                    reason: ok.reason.or(Some("stop".into())),
                });
            }
            Err(err) => self.surface_a2a_error(err),
        }
    }

    #[cfg(test)]
    pub fn test_apply_ws_event(&mut self, event: crate::ws::WsEvent) {
        self.apply_ws_event(event);
    }

    pub fn model_ux_host_ok(&self) -> bool {
        self.session
            .as_ref()
            .map(|s| s.model_ux_accepted() && !s.model_ux_rejected())
            .unwrap_or(false)
    }

    fn refresh_model_capability(&mut self) {
        match &self.session {
            Some(s) if s.model_ux_accepted() && !s.model_ux_rejected() => {
                if self.model_status.as_deref() == Some(MODEL_UNAVAILABLE) {
                    self.model_status = None;
                }
                self.refresh_profiles();
                self.refresh_host_prefs();
            }
            Some(_) | None => {
                if self.can_rpc() {
                    self.model_status = Some(MODEL_UNAVAILABLE.into());
                }
            }
        }
    }

    fn surface_model_unavailable(&mut self) {
        self.model_status = Some(MODEL_UNAVAILABLE.into());
        self.toast = Some(MODEL_UNAVAILABLE.into());
    }

    fn surface_model_error(&mut self, err: ConnectError) {
        if err.is_model_ux_unsupported() {
            self.surface_model_unavailable();
            return;
        }
        let label = if err.is_agent_busy() {
            TOAST_AGENT_BUSY.to_string()
        } else {
            err.as_label()
        };
        self.model_status = Some(label.clone());
        self.toast = Some(label);
    }

    pub fn picker_params(&self) -> ModelParams {
        ModelParams::from_drafts(&self.picker_model, &self.picker_effort, self.picker_fast)
    }

    pub fn apply_local_model_prefs(&mut self, prefs: ModelPrefs) {
        self.model_prefs = prefs;
        self.picker_model = self.model_prefs.last.model_draft();
        self.picker_effort = self.model_prefs.last.effort_draft();
        self.picker_fast = self.model_prefs.last.fast;
    }

    fn apply_remembered_for_provider(&mut self, provider: &str) {
        let params = model_ux::params_for_provider(&self.model_prefs, provider).clone();
        if let Some(item) = self.host_prefs.iter().find(|p| p.provider == provider) {
            self.picker_model = item.model.clone().unwrap_or_else(|| params.model_draft());
            self.picker_effort = item.effort.clone().unwrap_or_else(|| params.effort_draft());
            self.picker_fast = item.fast;
            return;
        }
        self.picker_model = params.model_draft();
        self.picker_effort = params.effort_draft();
        self.picker_fast = params.fast;
    }

    pub fn remember_model_choice(&mut self) {
        let params = self.picker_params();
        model_ux::remember_params(
            &mut self.model_prefs,
            self.picker_provider.as_deref(),
            params,
        );
        model_ux::save_model_prefs(&self.model_prefs);
    }

    pub fn selected_agent_params(&self) -> Option<&ModelParams> {
        let id = self.selected_agent()?.id.as_str();
        self.agent_params.get(id)
    }

    pub fn can_switch_agent(&self) -> bool {
        self.can_rpc()
            && self.model_ux_host_ok()
            && self.selected_agent().is_some()
            && self.picker_provider.is_some()
            && !self.selected_agent_is_running()
    }

    pub fn switch_selected_agent(&mut self) {
        if !self.model_ux_host_ok() {
            self.surface_model_unavailable();
            return;
        }
        if self.selected_agent_is_running() {
            self.toast = Some(TOAST_AGENT_BUSY.into());
            return;
        }
        let Some(agent_id) = self.selected_agent().map(|a| a.id.clone()) else {
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
        let params = self.picker_params();
        match session.agent_switch(
            &agent_id,
            Some(&provider),
            params.model.as_deref(),
            params.effort.as_deref(),
            Some(params.fast),
            None,
        ) {
            Ok(view) => self.apply_switched_agent(&agent_id, view, params),
            Err(err) => self.surface_model_error(err),
        }
    }

    pub fn can_create_profile(&self) -> bool {
        self.can_rpc()
            && self.model_ux_host_ok()
            && self.picker_provider.is_some()
            && model_ux::valid_profile_name(&self.profile_name_draft)
    }

    pub fn create_profile_from_picker(&mut self) {
        if !self.model_ux_host_ok() {
            self.surface_model_unavailable();
            return;
        }
        if !model_ux::valid_profile_name(&self.profile_name_draft) {
            self.toast = Some(PROFILE_NAME_BAD.into());
            return;
        }
        let Some(provider) = self.picker_provider.clone() else {
            self.toast = Some(PICKER_HINT.into());
            return;
        };
        let Some(session) = self.session.clone() else {
            return;
        };
        let name = self.profile_name_draft.trim().to_string();
        let params = self.picker_params();
        match session.profile_create(
            &name,
            &provider,
            params.model.as_deref(),
            params.effort.as_deref(),
            Some(params.fast),
        ) {
            Ok(profile) => {
                self.selected_profile_id = Some(profile.id.clone());
                if !self.profiles.iter().any(|p| p.id == profile.id) {
                    self.profiles.push(profile);
                }
                self.remember_model_choice();
                self.refresh_profiles();
            }
            Err(err) => self.surface_model_error(err),
        }
    }

    fn apply_profile_to_picker(&mut self, profile: &ProfileOk) {
        self.picker_model = profile.model.clone().unwrap_or_default();
        self.picker_effort = profile.effort.clone().unwrap_or_default();
        self.picker_fast = profile.fast;
        if self.providers.iter().any(|p| p.id == profile.provider) {
            self.picker_provider = Some(profile.provider.clone());
        }
    }

    pub fn select_profile(&mut self, id: Option<String>) {
        if let Some(ref pid) = id {
            if let Some(profile) = self.profiles.iter().find(|p| &p.id == pid).cloned() {
                self.apply_profile_to_picker(&profile);
            } else if self.model_ux_host_ok() {
                if let Some(session) = self.session.clone() {
                    match session.profile_get(pid) {
                        Ok(profile) => {
                            if !self.profiles.iter().any(|p| p.id == profile.id) {
                                self.profiles.push(profile.clone());
                            }
                            self.apply_profile_to_picker(&profile);
                        }
                        Err(err) => self.surface_model_error(err),
                    }
                }
            }
        }
        self.selected_profile_id = id;
    }

    pub fn can_apply_profile(&self) -> bool {
        self.can_rpc()
            && self.model_ux_host_ok()
            && self.selected_agent().is_some()
            && self.selected_profile_id.is_some()
            && !self.selected_agent_is_running()
    }

    pub fn apply_selected_profile(&mut self) {
        if !self.model_ux_host_ok() {
            self.surface_model_unavailable();
            return;
        }
        if self.selected_agent_is_running() {
            self.toast = Some(TOAST_AGENT_BUSY.into());
            return;
        }
        let Some(agent_id) = self.selected_agent().map(|a| a.id.clone()) else {
            return;
        };
        let Some(profile_id) = self.selected_profile_id.clone() else {
            self.toast = Some(model_ux::PROFILE_HINT.into());
            return;
        };
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.agent_switch(&agent_id, None, None, None, None, Some(&profile_id)) {
            Ok(view) => {
                let params = self
                    .profiles
                    .iter()
                    .find(|p| p.id == profile_id)
                    .map(|p| ModelParams {
                        model: p.model.clone(),
                        effort: p.effort.clone(),
                        fast: p.fast,
                    })
                    .unwrap_or_else(|| ModelParams {
                        model: view.model.clone(),
                        effort: view.effort.clone(),
                        fast: view.fast,
                    });
                if self.providers.iter().any(|p| p.id == view.agent.provider) {
                    self.picker_provider = Some(view.agent.provider.clone());
                }
                self.picker_model = params.model_draft();
                self.picker_effort = params.effort_draft();
                self.picker_fast = params.fast;
                self.apply_switched_agent(&agent_id, view, params);
            }
            Err(err) => self.surface_model_error(err),
        }
    }

    pub fn refresh_profiles(&mut self) {
        if !self.model_ux_host_ok() {
            return;
        }
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.profile_list() {
            Ok(items) => {
                self.profiles = items;
                if let Some(id) = self.selected_profile_id.clone() {
                    if !self.profiles.iter().any(|p| p.id == id) {
                        self.selected_profile_id = None;
                    }
                }
            }
            Err(err) => self.surface_model_error(err),
        }
    }

    fn refresh_host_prefs(&mut self) {
        if !self.model_ux_host_ok() {
            return;
        }
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.prefs_get() {
            Ok(items) => {
                self.host_prefs = items;
                if let Some(provider) = self.picker_provider.clone() {
                    self.apply_remembered_for_provider(&provider);
                } else if self.picker_model.is_empty() {
                    if let Some(item) = self.host_prefs.iter().find(|p| p.model.is_some()) {
                        self.picker_model = item.model.clone().unwrap_or_default();
                        self.picker_effort = item.effort.clone().unwrap_or_default();
                        self.picker_fast = item.fast;
                    }
                }
            }
            Err(err) => {
                if err.is_model_ux_unsupported() {
                    self.surface_model_unavailable();
                }
            }
        }
    }

    fn apply_switched_agent(&mut self, agent_id: &str, view: AgentModelView, params: ModelParams) {
        let keep_id = self.selected_agent_id.clone();
        if let Some(stub) = self.agents.iter_mut().find(|a| a.id == agent_id) {
            *stub = AgentStub::from(view.agent.clone());
        }
        let stored = ModelParams {
            model: view.model.clone().or(params.model),
            effort: view.effort.clone().or(params.effort),
            fast: view.fast || params.fast,
        };
        self.agent_params.insert(agent_id.to_string(), stored);
        self.remember_model_choice();
        self.refresh_after_switch(agent_id);
        // Same agent: never drop the selection just because host echoed fields.
        if self.selected_agent_id.is_none() {
            self.selected_agent_id = keep_id;
        }
    }

    fn refresh_after_switch(&mut self, agent_id: &str) {
        let Some(session) = self.session.clone() else {
            return;
        };
        if let Ok(agent) = session.agent_get(agent_id) {
            if let Some(stub) = self.agents.iter_mut().find(|a| a.id == agent_id) {
                *stub = AgentStub::from(agent);
            }
        }
        match session.agent_get_context(agent_id) {
            Ok(messages) => {
                apply_get_context_replace(
                    &mut self.messages,
                    messages.into_iter().map(ChatMessage::from).collect(),
                );
                self.ingest_inbox_from_messages(agent_id);
            }
            Err(_) => {
                // Keep the local transcript. Host owns it; GUI must not wipe.
            }
        }
    }

    pub fn workspace_host_ok(&self) -> bool {
        self.session
            .as_ref()
            .map(|s| s.workspace_accepted() && !s.workspace_rejected())
            .unwrap_or(false)
    }

    fn refresh_workspace_capability(&mut self) {
        match &self.session {
            Some(s) if s.workspace_accepted() && !s.workspace_rejected() => {
                if self.workspace_status.as_deref() == Some(WORKSPACE_UNAVAILABLE) {
                    self.workspace_status = None;
                }
            }
            Some(_) | None => {
                if self.can_rpc() {
                    self.workspace_status = Some(WORKSPACE_UNAVAILABLE.into());
                }
            }
        }
    }

    fn surface_workspace_unavailable(&mut self) {
        self.workspace_status = Some(WORKSPACE_UNAVAILABLE.into());
        self.toast = Some(WORKSPACE_UNAVAILABLE.into());
    }

    fn surface_workspace_error(&mut self, err: ConnectError) {
        if err.is_workspace_unsupported() {
            self.surface_workspace_unavailable();
        } else {
            let label = err.as_label();
            self.workspace_status = Some(label.clone());
            self.toast = Some(label);
        }
    }

    pub fn sync_host_ok(&self) -> bool {
        self.session
            .as_ref()
            .map(|s| s.sync_accepted() && !s.sync_rejected())
            .unwrap_or(false)
    }

    fn refresh_sync_capability(&mut self) {
        match &self.session {
            Some(s) if s.sync_accepted() && !s.sync_rejected() => {
                if self.sync_status.as_deref() == Some(SYNC_UNAVAILABLE) {
                    self.sync_status = None;
                }
            }
            Some(_) | None => {
                if self.can_rpc() {
                    self.sync_status = Some(SYNC_UNAVAILABLE.into());
                }
            }
        }
    }

    fn surface_sync_unavailable(&mut self) {
        self.sync_status = Some(SYNC_UNAVAILABLE.into());
        self.toast = Some(SYNC_UNAVAILABLE.into());
        self.show_sync_import_confirm = false;
    }

    fn surface_sync_error(&mut self, err: ConnectError) {
        if err.is_sync_unsupported() {
            self.surface_sync_unavailable();
        } else {
            let label = err.as_label();
            self.sync_status = Some(label.clone());
            self.toast = Some(label);
        }
    }

    fn export_task_ids(&self) -> Vec<String> {
        let selected = self.selected_task_id.as_deref();
        let loaded: Vec<String> = self.tasks.iter().map(|t| t.id.clone()).collect();
        sync_ux::export_task_ids(selected, &loaded)
    }

    pub fn export_sync(&mut self) -> Option<(String, String)> {
        if !self.sync_host_ok() {
            self.surface_sync_unavailable();
            return None;
        }
        let session = self.session.clone()?;
        let task_ids = self.export_task_ids();
        if task_ids.is_empty() {
            self.toast = Some(SYNC_NEED_TASK.into());
            return None;
        }
        match session.sync_export(&task_ids) {
            Ok(ok) => {
                let archive = sync_ux::strip_secrets(ok.archive);
                let filename = sync_ux::export_filename(&archive);
                match serde_json::to_string_pretty(&archive) {
                    Ok(payload) => Some((filename, payload)),
                    Err(err) => {
                        self.toast = Some(err.to_string());
                        None
                    }
                }
            }
            Err(err) => {
                self.surface_sync_error(err);
                None
            }
        }
    }

    pub fn save_exported_sync(&mut self, filename: &str, payload: &str) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title(EXPORT_BUTTON)
            .set_file_name(filename)
            .add_filter("JSON", &["json"])
            .save_file()
        {
            match std::fs::write(&path, payload) {
                Ok(()) => self.toast = Some(EXPORT_SAVED.into()),
                Err(err) => self.toast = Some(err.to_string()),
            }
        }
    }

    pub fn request_sync_import(&mut self) {
        if !self.sync_host_ok() {
            self.surface_sync_unavailable();
            return;
        }
        if self
            .workspace_id
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
        {
            self.toast = Some(SYNC_NEED_WORKSPACE.into());
            return;
        }
        self.show_sync_import_confirm = true;
    }

    pub fn cancel_sync_import(&mut self) {
        self.show_sync_import_confirm = false;
    }

    pub fn confirm_sync_import(&mut self) {
        if !self.show_sync_import_confirm {
            return;
        }
        self.show_sync_import_confirm = false;
        if let Some(path) = rfd::FileDialog::new()
            .set_title(IMPORT_BUTTON)
            .add_filter("JSON", &["json"])
            .pick_file()
        {
            match std::fs::read_to_string(&path) {
                Ok(text) => self.import_sync_archive_text(&text),
                Err(err) => self.toast = Some(err.to_string()),
            }
        }
    }

    #[cfg(test)]
    pub fn confirm_sync_import_payload(&mut self, archive: Value) {
        if !self.show_sync_import_confirm {
            return;
        }
        self.show_sync_import_confirm = false;
        self.import_sync_archive(archive);
    }

    fn import_sync_archive_text(&mut self, text: &str) {
        match serde_json::from_str::<Value>(text) {
            Ok(value) => self.import_sync_archive(sync_ux::unwrap_archive(value)),
            Err(err) => self.toast = Some(err.to_string()),
        }
    }

    fn import_sync_archive(&mut self, archive: Value) {
        if !self.sync_host_ok() {
            self.surface_sync_unavailable();
            return;
        }
        let Some(workspace_id) = self
            .workspace_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
        else {
            self.toast = Some(SYNC_NEED_WORKSPACE.into());
            return;
        };
        let Some(session) = self.session.clone() else {
            return;
        };
        let archive = sync_ux::strip_secrets(archive);
        match session.sync_import(&workspace_id, archive) {
            Ok(ok) => {
                let summary = sync_ux::format_import_result(&ok, &session.host_id);
                if self.sync_status.as_deref() == Some(SYNC_UNAVAILABLE) {
                    self.sync_status = None;
                }
                self.refresh_tasks_catalog();
                if let Some(task_id) = self.selected_task_id.clone() {
                    self.reload_agents(&task_id);
                    if self.selected_agent_id.is_some() {
                        self.load_selected_agent();
                    }
                }
                self.toast = Some(summary);
            }
            Err(err) => self.surface_sync_error(err),
        }
    }

    pub fn search_host_ok(&self) -> bool {
        self.session
            .as_ref()
            .map(|s| s.search_accepted() && !s.search_rejected())
            .unwrap_or(false)
    }

    pub fn worktree_gc_host_ok(&self) -> bool {
        self.session
            .as_ref()
            .map(|s| s.worktree_gc_accepted() && !s.worktree_gc_rejected())
            .unwrap_or(false)
    }

    fn refresh_search_gc_capability(&mut self) {
        match &self.session {
            Some(s) if s.search_accepted() && !s.search_rejected() => {
                if self.search_status.as_deref() == Some(SEARCH_UNAVAILABLE) {
                    self.search_status = None;
                }
            }
            Some(s) if s.search_rejected() => {
                if self.can_rpc() {
                    self.search_status = Some(SEARCH_UNAVAILABLE.into());
                }
            }
            _ => {}
        }
    }

    fn surface_search_unavailable(&mut self) {
        self.search_status = Some(SEARCH_UNAVAILABLE.into());
        self.toast = Some(SEARCH_UNAVAILABLE.into());
    }

    fn surface_search_error(&mut self, err: ConnectError) {
        if err.is_search_unsupported() {
            self.surface_search_unavailable();
        } else {
            let label = err.as_label();
            self.search_status = Some(label.clone());
            self.toast = Some(label);
        }
    }

    fn surface_gc_unavailable(&mut self) {
        self.toast = Some(GC_UNAVAILABLE.into());
        self.show_worktree_gc_confirm = false;
    }

    fn surface_gc_error(&mut self, err: ConnectError) {
        if err.is_worktree_gc_unsupported() {
            self.surface_gc_unavailable();
        } else {
            self.toast = Some(err.as_label());
        }
    }

    pub fn mark_search_edited(&mut self) {
        self.search_edited_at = Some(Instant::now());
    }

    fn tick_search_debounce(&mut self) {
        let Some(at) = self.search_edited_at else {
            return;
        };
        if at.elapsed() < std::time::Duration::from_millis(SEARCH_DEBOUNCE_MS) {
            return;
        }
        self.submit_search();
    }

    pub fn submit_search(&mut self) {
        self.search_edited_at = None;
        let q = self.search_q.trim().to_string();
        if q.is_empty() {
            self.search_items.clear();
            self.search_ran = false;
            self.last_search_q = None;
            return;
        }
        if self.last_search_q.as_deref() == Some(q.as_str()) && self.search_ran {
            return;
        }
        let Some(session) = self.session.clone() else {
            return;
        };
        if !self.search_host_ok() && session.search_rejected() {
            self.surface_search_unavailable();
            return;
        }
        match session.search_query(&q, None) {
            Ok(items) => {
                self.search_items = items
                    .into_iter()
                    .map(|ok| SearchItem {
                        kind: ok.kind,
                        id: ok.id,
                        title: ok.title,
                        hint: ok.hint,
                    })
                    .collect();
                self.search_ran = true;
                self.last_search_q = Some(q);
                if self.search_status.as_deref() == Some(SEARCH_UNAVAILABLE) {
                    self.search_status = None;
                }
            }
            Err(err) => {
                self.search_items.clear();
                self.search_ran = true;
                self.last_search_q = Some(q);
                self.surface_search_error(err);
            }
        }
    }

    pub fn activate_search_result(&mut self, index: usize) {
        let Some(item) = self.search_items.get(index).cloned() else {
            return;
        };
        match item.kind.as_str() {
            search_ux::KIND_TASK => {
                self.open_task(item.id);
            }
            search_ux::KIND_ARTIFACT => {
                if let Some(art) = self.artifacts.iter().find(|a| a.id == item.id) {
                    let task_id = art.task_id.clone();
                    if self.selected_task_id.as_deref() != Some(task_id.as_str())
                        && self.tasks.iter().any(|t| t.id == task_id)
                    {
                        self.open_task(task_id);
                    }
                }
                self.select_artifact(item.id);
                if self.split.right != crate::ladder::PaneKind::Artifacts {
                    self.set_split_pane("right", crate::ladder::PaneKind::Artifacts);
                }
            }
            search_ux::KIND_WORKSPACE => {
                if let Some(ws) = self.workspaces.iter().find(|w| w.id == item.id) {
                    self.workspace_id = Some(ws.id.clone());
                    self.workspace_path = Some(ws.path.clone());
                } else {
                    self.workspace_id = Some(item.id);
                }
            }
            _ => {}
        }
    }

    pub fn request_worktree_gc(&mut self) {
        if !self.can_rpc() {
            return;
        }
        if !self.worktree_gc_host_ok()
            && self
                .session
                .as_ref()
                .is_some_and(|s| s.worktree_gc_rejected())
        {
            self.surface_gc_unavailable();
            return;
        }
        self.show_worktree_gc_confirm = true;
    }

    pub fn cancel_worktree_gc(&mut self) {
        self.show_worktree_gc_confirm = false;
    }

    pub fn confirm_worktree_gc(&mut self) {
        if !self.show_worktree_gc_confirm {
            return;
        }
        self.show_worktree_gc_confirm = false;
        let Some(session) = self.session.clone() else {
            return;
        };
        if !self.worktree_gc_host_ok() && session.worktree_gc_rejected() {
            self.surface_gc_unavailable();
            return;
        }
        match session.worktree_gc(false) {
            Ok(ok) => {
                let value = serde_json::json!({
                    "dryRun": ok.dry_run,
                    "deleted": ok.deleted,
                    "items": ok.items,
                });
                self.toast = Some(search_ux::format_gc_result(&value));
                self.load_git_panel();
            }
            Err(err) => self.surface_gc_error(err),
        }
    }

    pub fn selected_task_preset(&self) -> Option<&str> {
        let id = self.selected_task_id.as_ref()?;
        self.task_presets.get(id).map(String::as_str)
    }

    pub fn selected_agent_role(&self) -> &str {
        self.selected_agent_id
            .as_ref()
            .and_then(|id| self.agent_roles.get(id))
            .map(String::as_str)
            .unwrap_or(self.picker_role.as_str())
    }

    fn sync_picker_role_from_selected(&mut self) {
        if let Some(id) = self.selected_agent_id.clone() {
            if let Some(role) = self.agent_roles.get(&id).cloned() {
                self.picker_role = role;
                return;
            }
        }
        if let Some(preset) = self.selected_task_preset() {
            self.picker_role = workspace_ux::default_role_for_preset(preset).to_string();
        }
    }

    pub fn set_new_task_preset(&mut self, preset: Option<String>) {
        match preset {
            Some(name) if workspace_ux::valid_preset(&name) => {
                let role = self
                    .presets
                    .iter()
                    .find(|item| item.id == name)
                    .map(|item| item.default_role.clone())
                    .filter(|role| workspace_ux::valid_role(role))
                    .unwrap_or_else(|| workspace_ux::default_role_for_preset(&name).to_string());
                self.new_task_preset = Some(name);
                self.picker_role = role;
            }
            _ => self.new_task_preset = None,
        }
    }

    pub fn set_picker_role(&mut self, role: String) {
        if !workspace_ux::valid_role(&role) {
            return;
        }
        if self.picker_role == role {
            return;
        }
        self.picker_role = role;
        if self.selected_agent_id.is_some() {
            self.update_selected_agent_role();
        }
    }

    pub fn update_selected_agent_role(&mut self) {
        if !self.workspace_host_ok() {
            self.surface_workspace_unavailable();
            return;
        }
        let Some(agent_id) = self.selected_agent_id.clone() else {
            return;
        };
        let role = self.picker_role.clone();
        if !workspace_ux::valid_role(&role) {
            return;
        }
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.agent_update_role(&agent_id, &role) {
            Ok((agent, got_role)) => {
                if let Some(stub) = self.agents.iter_mut().find(|a| a.id == agent_id) {
                    *stub = AgentStub::from(agent);
                }
                let stored = got_role.unwrap_or(role);
                self.agent_roles.insert(agent_id, stored);
            }
            Err(err) => self.surface_workspace_error(err),
        }
    }

    pub fn load_workspace_guides(&mut self) {
        if !self.workspace_host_ok() {
            if self.can_rpc() {
                self.workspace_status = Some(WORKSPACE_UNAVAILABLE.into());
            }
            return;
        }
        let Some(workspace_id) = self.workspace_id.clone() else {
            self.workspace_guides = None;
            return;
        };
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.workspace_guides_get(&workspace_id) {
            Ok(guides) => {
                if self.workspace_status.as_deref() == Some(WORKSPACE_UNAVAILABLE) {
                    self.workspace_status = None;
                }
                if !self.settings_guide_loaded {
                    if let Some(global) = guides.global_guide.as_ref() {
                        self.settings_guide_path = global.path.clone();
                        self.settings_guide_draft = global.content.clone();
                        self.settings_guide_truncated = global.truncated;
                    }
                }
                self.workspace_guides = Some(guides);
            }
            Err(err) => self.surface_workspace_error(err),
        }
    }

    pub fn load_presets(&mut self) {
        if !self.workspace_host_ok() {
            self.presets = workspace_ux::builtin_presets();
            return;
        }
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.preset_list() {
            Ok(items) if !items.is_empty() => self.presets = items,
            Ok(_) => self.presets = workspace_ux::builtin_presets(),
            Err(err) => {
                self.presets = workspace_ux::builtin_presets();
                if err.is_workspace_unsupported() {
                    self.surface_workspace_error(err);
                }
            }
        }
    }

    pub fn ensure_settings_guide(&mut self) {
        if self.settings_guide_loaded {
            return;
        }
        self.load_settings_guide();
    }

    pub fn load_settings_guide(&mut self) {
        if !self.workspace_host_ok() {
            if self.can_rpc() {
                self.workspace_status = Some(WORKSPACE_UNAVAILABLE.into());
            }
            return;
        }
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.settings_guide_get() {
            Ok(guide) => self.apply_settings_guide(guide),
            Err(err) => self.surface_workspace_error(err),
        }
    }

    fn apply_settings_guide(&mut self, guide: SettingsGuide) {
        self.settings_guide_path = guide.path;
        self.settings_guide_draft = guide.content;
        self.settings_guide_truncated = guide.truncated;
        self.settings_guide_loaded = true;
    }

    pub fn save_settings_guide(&mut self) {
        if !self.workspace_host_ok() {
            self.surface_workspace_unavailable();
            return;
        }
        if !workspace_ux::guide_content_fits(&self.settings_guide_draft) {
            self.toast = Some(GUIDE_TOO_LONG.into());
            return;
        }
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.settings_guide_set(&self.settings_guide_draft) {
            Ok(guide) => self.apply_settings_guide(guide),
            Err(err) => self.surface_workspace_error(err),
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
            parent_id: None,
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
            parent_id: None,
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
            parent_id: None,
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
            parent_id: None,
            provider: "byoa.foo".into(),
            status: AgentStatus::Idle,
            interface: "chat".into(),
        });
        state.agents.push(AgentStub {
            id: "ag-9".into(),
            task_id: "task-2".into(),
            parent_id: None,
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

    fn session_without_1_4() -> crate::rpc::Session {
        use std::collections::BTreeMap;
        let mut rejected = BTreeMap::new();
        for name in crate::rpc::ARTIFACT_METHODS {
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

    fn artifact_export_mock_ok(params: &Value) -> String {
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

    fn start_artifacts_pdf_reject_mock() -> SliceMock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let hits_t = hits.clone();
        thread::spawn(move || {
            for stream in listener.incoming().take(24) {
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
                    "handshake" => {
                        let mut accepted = serde_json::Map::new();
                        for name in crate::rpc::ARTIFACT_METHODS {
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
                    "artifact.export" => {
                        let format = params
                            .get("format")
                            .and_then(|v| v.as_str())
                            .unwrap_or("md");
                        if format == "pdf" {
                            json!({
                                "id": "echo",
                                "error": {
                                    "code": "invalid_params",
                                    "message": "pdf export is not implemented"
                                }
                            })
                            .to_string()
                        } else {
                            artifact_export_mock_ok(&params)
                        }
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

    fn start_artifacts_state_mock() -> SliceMock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let hits_t = hits.clone();
        thread::spawn(move || {
            let store = Arc::new(Mutex::new(Vec::<Value>::new()));
            let messages_left = Arc::new(Mutex::new(2i64));
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
                    "id": params.get("artifactId").cloned().unwrap_or(json!("art-1")),
                    "taskId": params.get("taskId").cloned().unwrap_or(json!("task-1")),
                    "parentId": params.get("parentId").cloned().unwrap_or(Value::Null),
                    "kind": kind,
                    "title": params.get("title").cloned().unwrap_or(json!("Auth")),
                    "body": params.get("body").cloned().unwrap_or(json!("body")),
                    "status": status,
                    "assignee": null,
                    "sourceMessageId": null,
                    "createdAt": "t",
                    "updatedAt": "t"
                });
                let thread = json!({
                    "id": params.get("threadId").cloned().unwrap_or(json!("th-1")),
                    "artifactId": "art-1",
                    "anchorStart": params.get("anchorStart").cloned().unwrap_or(json!(0)),
                    "anchorEnd": params.get("anchorEnd").cloned().unwrap_or(json!(12)),
                    "resolved": method == "comment.resolve",
                    "comments": [{ "id": "c-1", "body": params.get("body").cloned().unwrap_or(json!("nit")), "createdAt": "t" }],
                    "createdAt": "t",
                    "updatedAt": "t"
                });
                if method == "artifact.create" {
                    store.lock().unwrap().push(sample.clone());
                }
                if method == "artifact.update" {
                    let mut items = store.lock().unwrap();
                    if let Some(existing) = items.iter_mut().find(|a| a["id"] == sample["id"]) {
                        if let Some(body) = params.get("body") {
                            existing["body"] = body.clone();
                        }
                        if let Some(title) = params.get("title") {
                            existing["title"] = title.clone();
                        }
                    } else {
                        items.push(sample.clone());
                    }
                }
                let listed = store.lock().unwrap().clone();
                let body = match method.as_str() {
                    "GET /health" => json!({"ok": true, "hostId": "host-a"}).to_string(),
                    "handshake" => {
                        let mut accepted = serde_json::Map::new();
                        for name in crate::rpc::ARTIFACT_METHODS {
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
                        "ok": { "items": listed, "truncated": false }
                    })
                    .to_string(),
                    "artifact.export" => artifact_export_mock_ok(&params),
                    "comment.create" | "comment.resolve" => {
                        json!({ "id": "echo", "ok": thread }).to_string()
                    }
                    "comment.list" => json!({
                        "id": "echo",
                        "ok": { "threads": [] }
                    })
                    .to_string(),
                    "agent.clear_transcript" => {
                        *messages_left.lock().unwrap() = 0;
                        json!({ "id": "echo", "ok": { "cleared": 2 } }).to_string()
                    }
                    "agent.get_context" => json!({
                        "id": "echo",
                        "ok": { "messages": [] }
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
    fn create_sends_kind_spec_ticket_story_review() {
        let mock = start_artifacts_state_mock();
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        for kind in ArtifactKind::ALL {
            state.artifact_create_kind = kind;
            state.artifact_create_title = format!("item-{}", kind.as_wire());
            state.create_artifact();
        }
        let hits = mock.hits.lock().unwrap().clone();
        let kinds: Vec<String> = hits
            .iter()
            .filter(|h| h.method == "artifact.create")
            .map(|h| h.params["kind"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(kinds, vec!["spec", "ticket", "story", "review"]);
        assert!(!hits.iter().any(|h| h.method == "files.write"));
    }

    #[test]
    fn update_body_uses_artifact_update_not_files_write() {
        let mock = start_artifacts_state_mock();
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.artifact_create_kind = ArtifactKind::Spec;
        state.artifact_create_title = "Auth".into();
        state.create_artifact();
        state.artifact_body_draft = "new body".into();
        state.save_artifact_body();
        let hits = mock.hits.lock().unwrap().clone();
        let update = hits
            .iter()
            .find(|h| h.method == "artifact.update")
            .expect("artifact.update");
        assert_eq!(update.params["body"], "new body");
        assert!(update.params.get("path").is_none());
        assert!(!hits.iter().any(|h| h.method == "files.write"));
    }

    #[test]
    fn export_markdown_sends_method_and_format() {
        let mock = start_artifacts_state_mock();
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.selected_artifact_id = Some("art-1".into());
        let exported = state.export_selected_markdown().expect("export");
        assert_eq!(exported.0, "art-1.md");
        let hits = mock.hits.lock().unwrap().clone();
        let export = hits
            .iter()
            .find(|h| h.method == "artifact.export")
            .expect("export");
        assert_eq!(export.params["format"], "md");
        assert_eq!(export.params["artifactId"], "art-1");
        assert!(!hits.iter().any(|h| h.method == "files.write"));
    }

    #[test]
    fn export_pdf_sends_method_and_format() {
        let mock = start_artifacts_state_mock();
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.selected_artifact_id = Some("art-1".into());
        let exported = state.export_selected_pdf().expect("pdf");
        assert_eq!(exported.0, "art-1.pdf");
        assert_eq!(exported.1, b"%PDF-1.4 test");
        let hits = mock.hits.lock().unwrap().clone();
        let export = hits
            .iter()
            .find(|h| h.method == "artifact.export")
            .expect("export");
        assert_eq!(export.params["format"], "pdf");
        assert_eq!(export.params["artifactId"], "art-1");
        assert!(!hits.iter().any(|h| h.method == "files.write"));
    }

    #[test]
    fn export_pdf_without_selection_does_not_call_or_toast() {
        let mock = start_artifacts_state_mock();
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.selected_artifact_id = None;
        assert!(state.export_selected_pdf().is_none());
        assert!(state.toast.is_none());
        let hits = mock.hits.lock().unwrap().clone();
        assert!(!hits.iter().any(|h| h.method == "artifact.export"));
    }

    #[test]
    fn export_pdf_unsupported_host_toasts_md_still_works() {
        let mock = start_artifacts_pdf_reject_mock();
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.selected_artifact_id = Some("art-1".into());
        assert!(state.export_selected_pdf().is_none());
        let toast = state.toast.clone().expect("toast");
        assert!(toast.contains("invalid_params"), "{toast}");
        assert!(!toast.contains("паник"));
        let md = state.export_selected_markdown().expect("md still works");
        assert_eq!(md.0, "art-1.md");
        assert_eq!(md.1, "# Auth");
        let hits = mock.hits.lock().unwrap().clone();
        let formats: Vec<String> = hits
            .iter()
            .filter(|h| h.method == "artifact.export")
            .map(|h| h.params["format"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(formats, vec!["pdf", "md"]);
    }

    #[test]
    fn clear_transcript_requires_confirm_then_keeps_artifacts() {
        let mock = start_artifacts_state_mock();
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.messages.push(ChatMessage {
            id: "m1".into(),
            role: "user".into(),
            content: "keep then clear".into(),
        });
        state.artifact_create_kind = ArtifactKind::Spec;
        state.artifact_create_title = "Spec".into();
        state.create_artifact();
        assert!(!state.artifacts.is_empty());
        state.request_clear_transcript();
        assert!(state.show_clear_transcript_confirm);
        assert_eq!(state.messages.len(), 1);
        let hits_before = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .filter(|h| h.method == "agent.clear_transcript")
            .count();
        assert_eq!(hits_before, 0);
        state.cancel_clear_transcript();
        assert!(!state.show_clear_transcript_confirm);
        assert_eq!(state.messages.len(), 1);
        state.request_clear_transcript();
        state.confirm_clear_transcript();
        assert!(!state.show_clear_transcript_confirm);
        assert!(state.messages.is_empty());
        assert!(!state.artifacts.is_empty());
        let hits = mock.hits.lock().unwrap().clone();
        assert!(hits.iter().any(|h| h.method == "agent.clear_transcript"));
    }

    #[test]
    fn no_task_cannot_create_or_clear() {
        let mut state = AppState::new();
        state.pending_discover = false;
        state.demo = false;
        state.host_status = HostStatus::Online;
        state.session = Some(session_without_1_4());
        assert!(state.selected_task_id.is_none());
        state.artifact_create_title = "x".into();
        state.create_artifact();
        assert_eq!(state.toast.as_deref(), Some(NEED_TASK));
        assert!(state.artifacts.is_empty());
        state.request_clear_transcript();
        assert!(!state.show_clear_transcript_confirm);
        assert_eq!(state.toast.as_deref(), Some(NEED_TASK));
    }

    #[test]
    fn old_host_artifacts_toast_and_does_not_panic() {
        let mut state = AppState::new();
        state.pending_discover = false;
        state.demo = false;
        state.host_status = HostStatus::Online;
        state.session = Some(session_without_1_4());
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
        state.artifact_create_title = "x".into();
        state.create_artifact();
        assert_eq!(state.toast.as_deref(), Some(ARTIFACTS_UNAVAILABLE));
        assert_eq!(
            state.artifacts_status.as_deref(),
            Some(ARTIFACTS_UNAVAILABLE)
        );
        assert!(state.artifacts.is_empty());
        state.composer_text = "hello".into();
        // chat / write / pty stay addressable — no panic on degrade
        let _ = state.composer_enabled();
        let _ = state.write_ready();
        let _ = state.terminal_host_ok();
        state.selected_artifact_id = Some("art-1".into());
        state.export_selected_markdown();
        assert_eq!(state.toast.as_deref(), Some(ARTIFACTS_UNAVAILABLE));
        state.export_selected_pdf();
        assert_eq!(state.toast.as_deref(), Some(ARTIFACTS_UNAVAILABLE));
    }

    #[test]
    fn comments_open_reply_resolve_rpc() {
        let mock = start_artifacts_state_mock();
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.selected_artifact_id = Some("art-1".into());
        state.artifact_selection = Some((0, 12));
        state.artifact_comment_draft = "nit".into();
        state.open_comment_thread();
        state
            .artifact_reply_drafts
            .insert("th-1".into(), "reply".into());
        state.reply_comment("th-1".into());
        state.resolve_comment("th-1".into());
        let hits = mock.hits.lock().unwrap().clone();
        let open = hits
            .iter()
            .find(|h| h.method == "comment.create" && h.params["threadId"].is_null())
            .expect("open");
        assert_eq!(open.params["anchorStart"], 0);
        assert_eq!(open.params["anchorEnd"], 12);
        assert_eq!(open.params["body"], "nit");
        let reply = hits
            .iter()
            .find(|h| h.method == "comment.create" && h.params["threadId"] == "th-1")
            .expect("reply");
        assert_eq!(reply.params["body"], "reply");
        let resolve = hits
            .iter()
            .find(|h| h.method == "comment.resolve")
            .expect("resolve");
        assert_eq!(resolve.params["threadId"], "th-1");
    }

    #[test]
    fn comments_hidden_without_selected_artifact() {
        let mut state = AppState::new();
        assert!(!state.comments_visible());
        state.selected_artifact_id = Some("art-1".into());
        assert!(state.comments_visible());
    }

    fn a2a_accepted_map() -> serde_json::Map<String, Value> {
        let mut accepted = serde_json::Map::new();
        for name in crate::rpc::A2A_METHODS {
            accepted.insert(name.to_string(), json!({"major": 1, "minor": 5}));
        }
        accepted
    }

    fn start_a2a_state_mock() -> SliceMock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let hits_t = hits.clone();
        thread::spawn(move || {
            let mut child_n = 0u32;
            for stream in listener.incoming().take(40) {
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
                            "accepted": a2a_accepted_map(),
                            "rejected": {}
                        }
                    })
                    .to_string(),
                    "host.ping" => json!({
                        "id": "echo",
                        "ok": { "hostId": "host-a", "now": "2026-08-17T12:00:00Z" }
                    })
                    .to_string(),
                    "agent.create" => {
                        child_n += 1;
                        let parent = params.get("parentId").cloned().unwrap_or(Value::Null);
                        json!({
                            "id": "echo",
                            "ok": {
                                "id": format!("child-{child_n}"),
                                "taskId": params.get("taskId").cloned().unwrap_or(json!("task-1")),
                                "hostId": "host-a",
                                "parentId": parent,
                                "interface": params.get("interface").cloned().unwrap_or(json!("chat")),
                                "provider": params.get("provider").cloned().unwrap_or(json!("cli.claude")),
                                "status": "idle",
                                "runLocation": "local",
                                "createdAt": "2026-08-19T10:00:00Z"
                            }
                        })
                        .to_string()
                    }
                    "agent.get" => {
                        let id = params
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("ag-1")
                            .to_string();
                        let parent = if id.starts_with("child-") {
                            json!("ag-1")
                        } else {
                            Value::Null
                        };
                        json!({
                            "id": "echo",
                            "ok": {
                                "id": id,
                                "taskId": "task-1",
                                "hostId": "host-a",
                                "parentId": parent,
                                "interface": "chat",
                                "provider": "cli.claude",
                                "status": "idle",
                                "runLocation": "local",
                                "createdAt": "2026-08-19T10:00:00Z"
                            }
                        })
                        .to_string()
                    }
                    "agent.get_context" => json!({
                        "id": "echo",
                        "ok": { "messages": [] }
                    })
                    .to_string(),
                    "a2a.deliver" => json!({
                        "id": "echo",
                        "ok": {
                            "messageId": "msg-a2a-1",
                            "toAgentId": params.get("toAgentId").cloned().unwrap_or(json!("ag-2"))
                        }
                    })
                    .to_string(),
                    "a2a.transcript" => json!({
                        "id": "echo",
                        "ok": {
                            "agentId": params.get("agentId").cloned().unwrap_or(json!("ag-1")),
                            "interface": "chat",
                            "messages": []
                        }
                    })
                    .to_string(),
                    "loop.start" => json!({
                        "id": "echo",
                        "ok": {
                            "loopId": "lp-1",
                            "iteration": 0,
                            "turns": 0,
                            "maxIterations": params.get("maxIterations").cloned().unwrap_or(json!(2)),
                            "budgetTurns": 4,
                            "status": "running"
                        }
                    })
                    .to_string(),
                    "loop.get" | "loop.stop" => json!({
                        "id": "echo",
                        "ok": {
                            "loopId": params.get("loopId").cloned().unwrap_or(json!("lp-1")),
                            "iteration": 1,
                            "turns": 2,
                            "maxIterations": 2,
                            "budgetTurns": 4,
                            "status": "stopped",
                            "reason": "stop"
                        }
                    })
                    .to_string(),
                    "host.doctor" | "policy.get" | "agent.list" | "artifact.list" => json!({
                        "id": "echo",
                        "ok": { "items": [], "providers": [] }
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

    fn session_without_1_5() -> crate::rpc::Session {
        use std::collections::BTreeMap;
        let mut rejected = BTreeMap::new();
        for name in crate::rpc::A2A_METHODS {
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

    fn claude_inbox_caps() -> crate::rpc::HarnessCapsView {
        crate::rpc::HarnessCapsView {
            a2a_inbox: true,
            ..Default::default()
        }
    }

    fn generic_no_inbox_caps() -> crate::rpc::HarnessCapsView {
        crate::rpc::HarnessCapsView {
            a2a_inbox: false,
            ..Default::default()
        }
    }

    #[test]
    fn new_conversation_sends_parent_id_and_same_task() {
        let mock = start_a2a_state_mock();
        let session = connect(&pid(&mock.origin)).expect("online");
        assert!(session.a2a_accepted());
        let mut state = online_state(session);
        state.agents.clear();
        state.agents.push(AgentStub {
            id: "ag-1".into(),
            task_id: "task-1".into(),
            parent_id: None,
            provider: "cli.claude".into(),
            status: AgentStatus::Idle,
            interface: "chat".into(),
        });
        state.selected_agent_id = Some("ag-1".into());
        state.providers.push(DoctorProvider {
            id: "cli.claude".into(),
            available: true,
            detail: String::new(),
            caps: Some(claude_inbox_caps()),
        });
        state.set_picker_provider("cli.claude".into());
        assert!(state.can_create_child());
        state.create_child_conversation();
        assert_eq!(state.selected_agent_id.as_deref(), Some("child-1"));
        let child = state
            .agents
            .iter()
            .find(|a| a.id == "child-1")
            .expect("child");
        assert_eq!(child.task_id, "task-1");
        assert_eq!(child.parent_id.as_deref(), Some("ag-1"));
        let hits = mock.hits.lock().unwrap().clone();
        let create = hits
            .iter()
            .find(|h| h.method == "agent.create")
            .expect("agent.create");
        assert_eq!(create.params["parentId"], "ag-1");
        assert_eq!(create.params["taskId"], "task-1");
        assert_eq!(create.params["provider"], "cli.claude");
        assert!(create.params.get("maxIterations").is_none());
    }

    #[test]
    fn parent_delete_does_not_drop_children() {
        let mut state = AppState::new();
        state.pending_discover = false;
        state.demo = false;
        state.selected_task_id = Some("task-1".into());
        state.agents.push(AgentStub {
            id: "parent".into(),
            task_id: "task-1".into(),
            parent_id: None,
            provider: "cli.claude".into(),
            status: AgentStatus::Idle,
            interface: "chat".into(),
        });
        state.agents.push(AgentStub {
            id: "child".into(),
            task_id: "task-1".into(),
            parent_id: Some("parent".into()),
            provider: "cli.claude".into(),
            status: AgentStatus::Idle,
            interface: "chat".into(),
        });
        state.selected_agent_id = Some("parent".into());
        let before = a2a::build_agent_tree(&state.agents);
        assert_eq!(before[0].children[0].id, "child");
        state.remove_agent("parent");
        assert!(state.agents.iter().any(|a| a.id == "child"));
        assert!(!state.agents.iter().any(|a| a.id == "parent"));
        let child = state.agents.iter().find(|a| a.id == "child").unwrap();
        assert_eq!(child.parent_id, None);
        assert_eq!(child.task_id, "task-1");
        let listed: Vec<&str> = state
            .agents_for_selected_task()
            .into_iter()
            .map(|a| a.id.as_str())
            .collect();
        assert_eq!(listed, vec!["child"]);
        let tree = a2a::build_agent_tree(&state.agents);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].id, "child");
        assert!(tree[0].children.is_empty());
    }

    #[test]
    fn inbox_live_only_with_cap_and_deliver_is_not_artifact() {
        let mock = start_a2a_state_mock();
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.agents.clear();
        state.agents.push(AgentStub {
            id: "ag-1".into(),
            task_id: "task-1".into(),
            parent_id: None,
            provider: "cli.generic".into(),
            status: AgentStatus::Idle,
            interface: "chat".into(),
        });
        state.agents.push(AgentStub {
            id: "ag-2".into(),
            task_id: "task-1".into(),
            parent_id: None,
            provider: "cli.claude".into(),
            status: AgentStatus::Idle,
            interface: "chat".into(),
        });
        state.selected_agent_id = Some("ag-1".into());
        state.providers = vec![
            DoctorProvider {
                id: "cli.generic".into(),
                available: true,
                detail: String::new(),
                caps: Some(generic_no_inbox_caps()),
            },
            DoctorProvider {
                id: "cli.claude".into(),
                available: true,
                detail: String::new(),
                caps: Some(claude_inbox_caps()),
            },
        ];
        assert!(!state.selected_inbox_live());
        assert!(!state.can_deliver_to("ag-1"));
        assert!(state.can_deliver_to("ag-2"));
        state.deliver_target = Some("ag-2".into());
        state.deliver_text = "review this".into();
        let arts_before = state.artifacts.len();
        state.deliver_to_selected_target();
        assert_eq!(state.inbox.len(), 1);
        assert_eq!(state.inbox[0].from_agent_id, "ag-1");
        assert_eq!(state.inbox[0].to_agent_id, "ag-2");
        assert_eq!(state.inbox[0].content, "review this");
        assert_eq!(state.artifacts.len(), arts_before);
        assert!(!state.inbox[0].content.contains("artifact"));
        let hits = mock.hits.lock().unwrap().clone();
        assert!(hits.iter().any(|h| h.method == "a2a.deliver"));
        assert!(!hits.iter().any(|h| h.method.starts_with("artifact.")));
        state.test_apply_ws_event(
            crate::ws::parse_event(
                r#"{"event":"a2a.delivered","fromAgentId":"ag-2","toAgentId":"ag-1","messageId":"msg-ws"}"#,
            )
            .expect("parse"),
        );
        assert!(state.inbox.iter().any(|i| i.message_id == "msg-ws"));
        assert_eq!(state.artifacts.len(), arts_before);
    }

    #[test]
    fn loop_start_sends_max_iterations_never_infinite() {
        let mock = start_a2a_state_mock();
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.agents.push(AgentStub {
            id: "ag-2".into(),
            task_id: "task-1".into(),
            parent_id: None,
            provider: "cli.claude".into(),
            status: AgentStatus::Idle,
            interface: "chat".into(),
        });
        state.loop_agent_a = Some("ag-1".into());
        state.loop_agent_b = Some("ag-2".into());
        state.loop_max_draft = "0".into();
        state.loop_prompt = "ping".into();
        assert!(!a2a::allows_infinite_loop());
        state.start_loop();
        let hits = mock.hits.lock().unwrap().clone();
        let start = hits
            .iter()
            .find(|h| h.method == "loop.start")
            .expect("loop.start");
        assert_eq!(start.params["taskId"], "task-1");
        assert_eq!(start.params["agentIds"], json!(["ag-1", "ag-2"]));
        assert_eq!(start.params["maxIterations"], 1);
        assert!(start.params.get("infinite").is_none());
        assert_ne!(start.params["maxIterations"], Value::Null);
        assert_eq!(
            state.loop_state.as_ref().map(|l| l.id.as_str()),
            Some("lp-1")
        );
        assert_eq!(
            state.loop_state.as_ref().map(|l| l.counter_label()),
            Some("0 / 1".into())
        );
    }

    #[test]
    fn loop_stop_sends_rpc() {
        let mock = start_a2a_state_mock();
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.agents.push(AgentStub {
            id: "ag-2".into(),
            task_id: "task-1".into(),
            parent_id: None,
            provider: "cli.claude".into(),
            status: AgentStatus::Idle,
            interface: "chat".into(),
        });
        state.loop_agent_a = Some("ag-1".into());
        state.loop_agent_b = Some("ag-2".into());
        state.loop_max_draft = "2".into();
        state.start_loop();
        state.stop_loop();
        let hits = mock.hits.lock().unwrap().clone();
        let stop = hits
            .iter()
            .find(|h| h.method == "loop.stop")
            .expect("loop.stop");
        assert_eq!(stop.params["loopId"], "lp-1");
        assert_eq!(
            state.loop_state.as_ref().map(|l| l.status.as_str()),
            Some("stopped")
        );
    }

    #[test]
    fn old_host_a2a_toasts_and_does_not_panic() {
        let mut state = AppState::new();
        state.pending_discover = false;
        state.demo = false;
        state.host_status = HostStatus::Online;
        state.session = Some(session_without_1_5());
        state.workspace_id = Some("ws-1".into());
        state.selected_task_id = Some("task-1".into());
        state.selected_agent_id = Some("ag-1".into());
        state.agents.push(AgentStub {
            id: "ag-1".into(),
            task_id: "task-1".into(),
            parent_id: None,
            provider: "cli.claude".into(),
            status: AgentStatus::Idle,
            interface: "chat".into(),
        });
        state.providers.push(DoctorProvider {
            id: "cli.claude".into(),
            available: true,
            detail: String::new(),
            caps: Some(claude_inbox_caps()),
        });
        state.set_picker_provider("cli.claude".into());
        state.create_child_conversation();
        assert_eq!(state.toast.as_deref(), Some(A2A_UNAVAILABLE));
        assert_eq!(state.a2a_status.as_deref(), Some(A2A_UNAVAILABLE));
        assert_eq!(A2A_UNAVAILABLE, "a2a недоступен: host без 1.5");
        state.deliver_target = Some("ag-1".into());
        state.deliver_text = "x".into();
        state.deliver_to_selected_target();
        assert_eq!(state.toast.as_deref(), Some(A2A_UNAVAILABLE));
        state.loop_agent_a = Some("ag-1".into());
        state.loop_agent_b = Some("ag-1".into());
        state.start_loop();
        assert_eq!(state.toast.as_deref(), Some(A2A_UNAVAILABLE));
        state.stop_loop();
        assert_eq!(state.toast.as_deref(), Some(A2A_UNAVAILABLE));
        let _ = state.composer_enabled();
        let _ = state.write_ready();
        let _ = state.terminal_host_ok();
        let _ = state.artifacts_host_ok();
        assert!(state.artifacts.is_empty());
        assert_eq!(state.agents.len(), 1);
    }

    fn model_accepted_map() -> serde_json::Map<String, Value> {
        let mut accepted = serde_json::Map::new();
        for name in crate::rpc::MODEL_METHODS {
            accepted.insert(name.to_string(), json!({"major": 1, "minor": 6}));
        }
        for name in crate::rpc::A2A_METHODS {
            accepted.insert(name.to_string(), json!({"major": 1, "minor": 5}));
        }
        accepted
    }

    fn session_without_1_6() -> crate::rpc::Session {
        use std::collections::BTreeMap;
        let mut rejected = BTreeMap::new();
        for name in crate::rpc::MODEL_METHODS {
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

    fn start_model_state_mock() -> SliceMock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let hits_t = hits.clone();
        thread::spawn(move || {
            let mut profile_n = 0u32;
            let mut last_provider = "cli.generic".to_string();
            for stream in listener.incoming().take(48) {
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
                    params: params.clone(),
                });
                if let Some(p) = params.get("provider").and_then(|v| v.as_str()) {
                    last_provider = p.to_string();
                }
                if method == "agent.switch" {
                    if let Some(pid) = params.get("profileId").and_then(|v| v.as_str()) {
                        if pid == "prof-1" {
                            last_provider = "cli.codex".into();
                        }
                    }
                }
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
                    "host.doctor" => json!({
                        "id": "echo",
                        "ok": {
                            "providers": [
                                {"id": "cli.generic", "available": true, "detail": ""},
                                {"id": "cli.claude", "available": true, "detail": ""},
                                {"id": "cli.codex", "available": true, "detail": ""}
                            ]
                        }
                    })
                    .to_string(),
                    "agent.switch" | "agent.get" => {
                        let id = params
                            .get("agentId")
                            .or_else(|| params.get("id"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("ag-1")
                            .to_string();
                        json!({
                            "id": "echo",
                            "ok": {
                                "id": id,
                                "taskId": "task-1",
                                "hostId": "host-a",
                                "parentId": null,
                                "interface": "chat",
                                "provider": last_provider,
                                "status": "idle",
                                "runLocation": "local",
                                "createdAt": "2026-08-19T10:00:00Z",
                                "model": "o3",
                                "effort": "high",
                                "fast": true
                            }
                        })
                        .to_string()
                    }
                    "agent.get_context" => json!({
                        "id": "echo",
                        "ok": {
                            "messages": [
                                sample_message("keep-1", "ag-1", "user", "turn one"),
                                sample_message("keep-2", "ag-1", "assistant", "turn two")
                            ]
                        }
                    })
                    .to_string(),
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
                                "fast": params.get("fast").cloned().unwrap_or(json!(false))
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
                                "fast": true
                            }]
                        }
                    })
                    .to_string(),
                    "profile.get" => json!({
                        "id": "echo",
                        "ok": {
                            "id": "prof-1",
                            "name": "codex high",
                            "provider": "cli.codex",
                            "model": "o3",
                            "effort": "high",
                            "fast": true
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
                        "ok": { "items": [] }
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
    fn switch_on_existing_agent_sends_method_and_keeps_messages() {
        let mock = start_model_state_mock();
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.providers = vec![
            DoctorProvider {
                id: "cli.generic".into(),
                available: true,
                detail: String::new(),
                caps: None,
            },
            DoctorProvider {
                id: "cli.codex".into(),
                available: true,
                detail: String::new(),
                caps: None,
            },
        ];
        state.messages = vec![
            ChatMessage {
                id: "keep-1".into(),
                role: "user".into(),
                content: "turn one".into(),
            },
            ChatMessage {
                id: "keep-2".into(),
                role: "assistant".into(),
                content: "turn two".into(),
            },
        ];
        state.set_picker_provider("cli.codex".into());
        state.picker_model = "o3".into();
        state.picker_effort = "high".into();
        state.picker_fast = true;
        assert!(state.can_switch_agent());
        state.switch_selected_agent();
        assert_eq!(state.selected_agent_id.as_deref(), Some("ag-1"));
        assert_eq!(
            state.selected_agent().map(|a| a.provider.as_str()),
            Some("cli.codex")
        );
        assert_eq!(state.messages.len(), 2);
        assert_eq!(state.messages[0].id, "keep-1");
        assert_eq!(state.messages[1].id, "keep-2");
        let hits = mock.hits.lock().unwrap().clone();
        let switch = hits
            .iter()
            .find(|h| h.method == "agent.switch")
            .cloned()
            .expect("agent.switch");
        assert_eq!(switch.params["agentId"], "ag-1");
        assert_eq!(switch.params["provider"], "cli.codex");
        assert_eq!(switch.params["model"], "o3");
        assert_eq!(switch.params["effort"], "high");
        assert_eq!(switch.params["fast"], true);
        assert!(hits.iter().any(|h| h.method == "agent.get"));
        assert!(hits.iter().any(|h| h.method == "agent.get_context"));
        assert!(!hits.iter().any(|h| h.method == "agent.create"));
        assert!(!hits.iter().any(|h| h.method == "agent.clear_transcript"));
    }

    #[test]
    fn profile_create_select_apply_rpc() {
        let mock = start_model_state_mock();
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.providers = vec![DoctorProvider {
            id: "cli.codex".into(),
            available: true,
            detail: String::new(),
            caps: None,
        }];
        state.messages = vec![ChatMessage {
            id: "keep-1".into(),
            role: "user".into(),
            content: "turn one".into(),
        }];
        state.set_picker_provider("cli.codex".into());
        state.picker_model = "o3".into();
        state.picker_effort = "high".into();
        state.picker_fast = true;
        state.profile_name_draft = "codex high".into();
        assert!(state.can_create_profile());
        state.create_profile_from_picker();
        assert_eq!(state.selected_profile_id.as_deref(), Some("prof-1"));
        state.select_profile(Some("prof-1".into()));
        assert_eq!(state.picker_model, "o3");
        assert!(state.can_apply_profile());
        state.apply_selected_profile();
        assert_eq!(state.selected_agent_id.as_deref(), Some("ag-1"));
        assert_eq!(
            state.selected_agent().map(|a| a.provider.as_str()),
            Some("cli.codex")
        );
        assert!(!state.messages.is_empty());
        let hits = mock.hits.lock().unwrap().clone();
        let create = hits
            .iter()
            .find(|h| h.method == "profile.create")
            .expect("profile.create");
        assert_eq!(create.params["name"], "codex high");
        assert_eq!(create.params["provider"], "cli.codex");
        assert_eq!(create.params["model"], "o3");
        assert!(hits.iter().any(|h| h.method == "profile.list"));
        let apply = hits
            .iter()
            .find(|h| h.method == "agent.switch" && h.params.get("profileId").is_some())
            .expect("apply switch");
        assert_eq!(apply.params["agentId"], "ag-1");
        assert_eq!(apply.params["profileId"], "prof-1");
    }

    #[test]
    fn last_model_effort_fast_remembered() {
        let prefs = ModelPrefs {
            last: ModelParams {
                model: Some("o3".into()),
                effort: Some("high".into()),
                fast: true,
            },
            ..Default::default()
        };
        let mut state = AppState::new();
        state.apply_local_model_prefs(prefs);
        assert_eq!(state.picker_model, "o3");
        assert_eq!(state.picker_effort, "high");
        assert!(state.picker_fast);
        state.picker_model = "sonnet".into();
        state.picker_effort = "medium".into();
        state.picker_fast = false;
        state.picker_provider = Some("cli.claude".into());
        state.remember_model_choice();
        assert_eq!(state.model_prefs.last.model.as_deref(), Some("sonnet"));
        assert_eq!(state.model_prefs.last.effort.as_deref(), Some("medium"));
        assert!(!state.model_prefs.last.fast);
        let for_claude = model_ux::params_for_provider(&state.model_prefs, "cli.claude");
        assert_eq!(for_claude.model.as_deref(), Some("sonnet"));
        let mut next = AppState::new();
        next.apply_local_model_prefs(state.model_prefs.clone());
        assert_eq!(next.picker_model, "sonnet");
        assert_eq!(next.picker_effort, "medium");
        assert!(!next.picker_fast);
    }

    #[test]
    fn old_host_model_ux_toasts_and_does_not_panic() {
        let mut state = AppState::new();
        state.pending_discover = false;
        state.demo = false;
        state.host_status = HostStatus::Online;
        state.session = Some(session_without_1_6());
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
        state.providers.push(DoctorProvider {
            id: "cli.codex".into(),
            available: true,
            detail: String::new(),
            caps: None,
        });
        state.messages.push(ChatMessage {
            id: "keep-1".into(),
            role: "user".into(),
            content: "stay".into(),
        });
        state.set_picker_provider("cli.codex".into());
        state.switch_selected_agent();
        assert_eq!(state.toast.as_deref(), Some(MODEL_UNAVAILABLE));
        assert_eq!(state.model_status.as_deref(), Some(MODEL_UNAVAILABLE));
        assert_eq!(MODEL_UNAVAILABLE, "модели недоступны: host без 1.6");
        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[0].id, "keep-1");
        state.profile_name_draft = "x".into();
        state.create_profile_from_picker();
        assert_eq!(state.toast.as_deref(), Some(MODEL_UNAVAILABLE));
        state.selected_profile_id = Some("prof-1".into());
        state.apply_selected_profile();
        assert_eq!(state.toast.as_deref(), Some(MODEL_UNAVAILABLE));
        let _ = state.composer_enabled();
        let _ = state.write_ready();
        let _ = state.terminal_host_ok();
        let _ = state.artifacts_host_ok();
        let _ = state.a2a_host_ok();
        assert_eq!(state.agents.len(), 1);
        assert_eq!(state.selected_agent_id.as_deref(), Some("ag-1"));
    }

    fn workspace_accepted_map() -> serde_json::Map<String, Value> {
        let mut accepted = serde_json::Map::new();
        for name in crate::rpc::WORKSPACE_METHODS {
            accepted.insert(name.to_string(), json!({"major": 1, "minor": 7}));
        }
        accepted
    }

    fn session_without_1_7() -> crate::rpc::Session {
        use std::collections::BTreeMap;
        let mut rejected = BTreeMap::new();
        for name in crate::rpc::WORKSPACE_METHODS {
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

    fn start_workspace_state_mock() -> SliceMock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let hits_t = hits.clone();
        thread::spawn(move || {
            for stream in listener.incoming().take(48) {
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
                                "content": "from rpc",
                                "truncated": false
                            },
                            "workspaceGuide": null,
                            "globalGuide": null
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
                    "agent.update" => json!({
                        "id": "echo",
                        "ok": {
                            "id": params.get("agentId").cloned().unwrap_or(json!("ag-1")),
                            "taskId": "task-1",
                            "hostId": "host-a",
                            "parentId": null,
                            "interface": "chat",
                            "provider": "cli.generic",
                            "status": "idle",
                            "runLocation": "local",
                            "createdAt": "2026-08-19T11:00:00Z",
                            "role": params.get("role").cloned().unwrap_or(json!("coder"))
                        }
                    })
                    .to_string(),
                    "task.create" => {
                        let title = params.get("title").and_then(|v| v.as_str()).unwrap_or("");
                        json!({
                            "id": "echo",
                            "ok": {
                                "id": "task-new",
                                "title": title,
                                "status": "open",
                                "createdAt": "2026-08-19T11:00:00Z",
                                "updatedAt": "2026-08-19T11:00:00Z",
                                "workspaceIds": ["ws-1"],
                                "preset": params.get("preset").cloned().unwrap_or(Value::Null)
                            }
                        })
                        .to_string()
                    }
                    "workspace.list" => json!({
                        "id": "echo",
                        "ok": { "items": [] }
                    })
                    .to_string(),
                    "task.list" => json!({
                        "id": "echo",
                        "ok": { "items": [] }
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
        SliceMock {
            origin: format!("http://{addr}"),
            hits,
        }
    }

    #[test]
    fn guide_load_uses_guides_get_not_fs_walk() {
        let mock = start_workspace_state_mock();
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.load_workspace_guides();
        assert!(state.workspace_guides.is_some());
        assert_eq!(
            workspace_ux::agents_md_chip(state.workspace_guides.as_ref()),
            workspace_ux::AGENTS_MD_PRESENT
        );
        let hits = mock.hits.lock().unwrap().clone();
        assert!(hits.iter().any(|h| h.method == "workspace.guides.get"));
        assert!(hits.iter().all(|h| h.method != "files.tree"));
        assert!(hits.iter().all(|h| h.method != "files.read"));
        let get = hits
            .iter()
            .find(|h| h.method == "workspace.guides.get")
            .expect("guides");
        assert_eq!(get.params["workspaceId"], "ws-1");
        assert!(get.params.get("path").is_none());
    }

    #[test]
    fn role_set_sends_agent_update() {
        let mock = start_workspace_state_mock();
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.selected_agent_id = Some("ag-1".into());
        state.agents.push(AgentStub {
            id: "ag-1".into(),
            task_id: "task-1".into(),
            parent_id: None,
            provider: "cli.generic".into(),
            status: AgentStatus::Idle,
            interface: "chat".into(),
        });
        state.set_picker_role("reviewer".into());
        assert_eq!(
            state.agent_roles.get("ag-1").map(String::as_str),
            Some("reviewer")
        );
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
    }

    #[test]
    fn preset_set_sends_one_of_four_names() {
        let mock = start_workspace_state_mock();
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.workspaces.push(rt_protocol::Workspace {
            id: "ws-1".into(),
            host_id: "host-a".into(),
            path: "/tmp/proj".into(),
            name: "proj".into(),
            created_at: "2026-08-19T11:00:00Z".into(),
        });
        state.new_task_title = "Plan login".into();
        state.set_new_task_preset(Some("planning".into()));
        assert!(state.can_create_task());
        state.create_task();
        let hit = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "task.create")
            .cloned()
            .expect("task.create");
        let preset = hit.params["preset"].as_str().expect("preset");
        assert!(
            ["planning", "review", "debug", "document"].contains(&preset),
            "{preset}"
        );
        assert_eq!(preset, "planning");
    }

    #[test]
    fn old_host_workspace_toasts_and_does_not_panic() {
        let mut state = AppState::new();
        state.pending_discover = false;
        state.demo = false;
        state.host_status = HostStatus::Online;
        state.session = Some(session_without_1_7());
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
        state.messages.push(ChatMessage {
            id: "keep-1".into(),
            role: "user".into(),
            content: "stay".into(),
        });
        state.load_workspace_guides();
        assert_eq!(
            state.workspace_status.as_deref(),
            Some(WORKSPACE_UNAVAILABLE)
        );
        state.set_picker_role("planner".into());
        assert_eq!(state.toast.as_deref(), Some(WORKSPACE_UNAVAILABLE));
        assert_eq!(
            WORKSPACE_UNAVAILABLE,
            "воркспейс-гайд недоступен: host без 1.7"
        );
        state.save_settings_guide();
        assert_eq!(state.toast.as_deref(), Some(WORKSPACE_UNAVAILABLE));
        let _ = state.composer_enabled();
        let _ = state.write_ready();
        let _ = state.terminal_host_ok();
        let _ = state.artifacts_host_ok();
        let _ = state.a2a_host_ok();
        let _ = state.model_ux_host_ok();
        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[0].id, "keep-1");
        assert_eq!(state.agents.len(), 1);
        assert_eq!(state.selected_agent_id.as_deref(), Some("ag-1"));
    }

    fn sync_accepted_map() -> serde_json::Map<String, Value> {
        let mut accepted = serde_json::Map::new();
        for name in crate::rpc::SYNC_METHODS {
            accepted.insert(name.to_string(), json!({"major": 1, "minor": 8}));
        }
        accepted
    }

    fn session_without_1_8() -> crate::rpc::Session {
        use std::collections::BTreeMap;
        let mut rejected = BTreeMap::new();
        for name in crate::rpc::SYNC_METHODS {
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

    fn sample_sync_archive() -> Value {
        json!({
            "kind": "rusttraycer.export",
            "exportVersion": 1,
            "sourceHostId": "host-a",
            "exportedAt": "2026-08-19T12:00:00Z",
            "tasks": [{"id": "task-1"}],
            "agents": [{"id": "ag-1", "hostId": "host-a"}],
            "messages": [],
            "artifacts": [],
            "commentThreads": [],
            "comments": [],
            "modelProfiles": [],
            "token": "should-strip",
            "sessionToken": "nope"
        })
    }

    fn start_sync_state_mock() -> SliceMock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let hits_t = hits.clone();
        thread::spawn(move || {
            for stream in listener.incoming().take(64) {
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
                        "ok": { "archive": sample_sync_archive() }
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
                    "workspace.list" => json!({
                        "id": "echo",
                        "ok": {
                            "items": [{
                                "id": "ws-1",
                                "hostId": "host-a",
                                "path": "/tmp/proj",
                                "name": "proj",
                                "createdAt": "2026-08-19T12:00:00Z"
                            }]
                        }
                    })
                    .to_string(),
                    "task.list" => json!({
                        "id": "echo",
                        "ok": {
                            "items": [{
                                "id": "task-1",
                                "title": "Login",
                                "status": "open",
                                "createdAt": "2026-08-19T12:00:00Z",
                                "updatedAt": "2026-08-19T12:00:00Z",
                                "workspaceIds": ["ws-1"]
                            }]
                        }
                    })
                    .to_string(),
                    "agent.list" => json!({
                        "id": "echo",
                        "ok": { "items": [sample_agent("ag-1", "task-1", "idle")] }
                    })
                    .to_string(),
                    "agent.get" => json!({
                        "id": "echo",
                        "ok": sample_agent("ag-1", "task-1", "idle")
                    })
                    .to_string(),
                    "agent.get_context" => json!({
                        "id": "echo",
                        "ok": { "messages": [] }
                    })
                    .to_string(),
                    "policy.get" => json!({
                        "id": "echo",
                        "ok": { "mode": "ask", "scope": "agent", "yolo": false, "source": "default" }
                    })
                    .to_string(),
                    "git.status" => json!({
                        "id": "echo",
                        "ok": { "branch": "main", "dirty": false, "entries": [], "truncated": false }
                    })
                    .to_string(),
                    "git.diff" => json!({
                        "id": "echo",
                        "ok": { "files": [], "truncated": false }
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
    fn export_sends_sync_export_save_is_client_side() {
        let mock = start_sync_state_mock();
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        let exported = state.export_sync().expect("export");
        assert!(exported.0.ends_with(".json"));
        assert!(exported.1.contains("rusttraycer.export"));
        assert!(!exported.1.contains("should-strip"));
        assert!(!exported.1.contains("sessionToken"));
        let hits = mock.hits.lock().unwrap().clone();
        let export = hits
            .iter()
            .find(|h| h.method == "sync.export")
            .expect("sync.export");
        assert_eq!(export.params["taskIds"], json!(["task-1"]));
        assert!(export.params.get("token").is_none());
        assert!(export.params.get("path").is_none());
        assert!(!hits.iter().any(|h| h.method == "files.write"));
    }

    #[test]
    fn import_requires_confirm_then_sends_sync_import() {
        let mock = start_sync_state_mock();
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        let archive = sample_sync_archive();
        state.confirm_sync_import_payload(archive.clone());
        let before = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .filter(|h| h.method == "sync.import")
            .count();
        assert_eq!(before, 0);
        state.request_sync_import();
        assert!(state.show_sync_import_confirm);
        let mid = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .filter(|h| h.method == "sync.import")
            .count();
        assert_eq!(mid, 0);
        state.cancel_sync_import();
        assert!(!state.show_sync_import_confirm);
        state.request_sync_import();
        state.confirm_sync_import_payload(archive);
        assert!(!state.show_sync_import_confirm);
        let hits = mock.hits.lock().unwrap().clone();
        let import = hits
            .iter()
            .find(|h| h.method == "sync.import")
            .expect("sync.import");
        assert_eq!(import.params["workspaceId"], "ws-1");
        assert_eq!(import.params["archive"]["kind"], "rusttraycer.export");
        assert!(import.params["archive"].get("token").is_none());
        assert!(import.params.get("token").is_none());
        assert_eq!(
            state.toast.as_deref(),
            Some("импорт: tasks=1 agents=2 messages=10 artifacts=1 profilesImported=0 profilesSkipped=1 hostId=host-a")
        );
    }

    #[test]
    fn old_host_sync_toasts_and_does_not_panic() {
        let mut state = AppState::new();
        state.pending_discover = false;
        state.demo = false;
        state.host_status = HostStatus::Online;
        state.session = Some(session_without_1_8());
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
        state.messages.push(ChatMessage {
            id: "keep-1".into(),
            role: "user".into(),
            content: "stay".into(),
        });
        assert!(state.export_sync().is_none());
        assert_eq!(state.toast.as_deref(), Some(SYNC_UNAVAILABLE));
        assert_eq!(SYNC_UNAVAILABLE, "синк недоступен: host без 1.8");
        state.request_sync_import();
        assert!(!state.show_sync_import_confirm);
        assert_eq!(state.toast.as_deref(), Some(SYNC_UNAVAILABLE));
        let _ = state.composer_enabled();
        let _ = state.write_ready();
        let _ = state.terminal_host_ok();
        let _ = state.artifacts_host_ok();
        let _ = state.a2a_host_ok();
        let _ = state.model_ux_host_ok();
        let _ = state.workspace_host_ok();
        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[0].id, "keep-1");
        assert_eq!(state.agents.len(), 1);
        assert_eq!(state.selected_agent_id.as_deref(), Some("ag-1"));
    }

    fn search_gc_accepted_map() -> serde_json::Map<String, Value> {
        let mut accepted = serde_json::Map::new();
        for name in crate::rpc::SEARCH_GC_METHODS {
            accepted.insert(name.to_string(), json!({"major": 1, "minor": 9}));
        }
        accepted
    }

    fn session_without_1_9() -> crate::rpc::Session {
        use std::collections::BTreeMap;
        let mut rejected = BTreeMap::new();
        for name in crate::rpc::SEARCH_GC_METHODS {
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

    fn start_search_gc_state_mock() -> SliceMock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let hits_t = hits.clone();
        thread::spawn(move || {
            for stream in listener.incoming().take(64) {
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
                        "ok": { "dryRun": false, "deleted": [] }
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

    fn start_missing_method_search_mock() -> SliceMock {
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
                        "ok": { "hostId": "host-a", "now": "2026-08-19T12:00:00Z" }
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
    fn search_query_sends_q_and_skips_empty() {
        let mock = start_search_gc_state_mock();
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.search_q = "   ".into();
        state.submit_search();
        assert!(state.search_items.is_empty());
        assert!(!state.search_ran);
        let empty_hits = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .filter(|h| h.method == "search.query")
            .count();
        assert_eq!(empty_hits, 0);
        state.search_q = "auth".into();
        state.submit_search();
        assert_eq!(state.search_items.len(), 1);
        assert_eq!(state.search_items[0].kind, "task");
        assert_eq!(state.search_items[0].title, "Auth");
        let hit = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "search.query")
            .cloned()
            .expect("search.query");
        assert_eq!(hit.params["q"], "auth");
        assert!(hit.params.get("kinds").is_none());
        assert!(hit.params.get("prefix").is_none());
    }

    #[test]
    fn old_host_search_and_gc_toast_and_do_not_panic() {
        let mut state = AppState::new();
        state.pending_discover = false;
        state.demo = false;
        state.host_status = HostStatus::Online;
        state.session = Some(session_without_1_9());
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
        state.messages.push(ChatMessage {
            id: "keep-1".into(),
            role: "user".into(),
            content: "stay".into(),
        });
        state.search_q = "auth".into();
        assert!(!state.search_host_ok());
        assert!(!state.worktree_gc_host_ok());
        state.submit_search();
        assert_eq!(state.toast.as_deref(), Some(SEARCH_UNAVAILABLE));
        assert_eq!(SEARCH_UNAVAILABLE, "поиск недоступен: host без 1.9");
        state.request_worktree_gc();
        assert!(!state.show_worktree_gc_confirm);
        assert_eq!(state.toast.as_deref(), Some(GC_UNAVAILABLE));
        assert_eq!(GC_UNAVAILABLE, "очистка worktree недоступна: host без 1.9");
        let _ = state.composer_enabled();
        let _ = state.write_ready();
        let _ = state.terminal_host_ok();
        let _ = state.artifacts_host_ok();
        let _ = state.a2a_host_ok();
        let _ = state.model_ux_host_ok();
        let _ = state.workspace_host_ok();
        let _ = state.sync_host_ok();
        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[0].id, "keep-1");
        assert_eq!(state.agents.len(), 1);
    }

    #[test]
    fn missing_method_search_toasts_without_panic() {
        let mock = start_missing_method_search_mock();
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.search_q = "auth".into();
        state.submit_search();
        assert_eq!(state.toast.as_deref(), Some(SEARCH_UNAVAILABLE));
        let _ = state.composer_enabled();
        let _ = state.write_ready();
        state.request_worktree_gc();
        assert!(state.show_worktree_gc_confirm);
        state.confirm_worktree_gc();
        assert_eq!(state.toast.as_deref(), Some(GC_UNAVAILABLE));
    }

    #[test]
    fn worktree_gc_sends_after_confirm_not_before_or_cancel() {
        let mock = start_search_gc_state_mock();
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.request_worktree_gc();
        assert!(state.show_worktree_gc_confirm);
        let before = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .filter(|h| h.method == "worktree.gc")
            .count();
        assert_eq!(before, 0);
        state.cancel_worktree_gc();
        assert!(!state.show_worktree_gc_confirm);
        let mid = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .filter(|h| h.method == "worktree.gc")
            .count();
        assert_eq!(mid, 0);
        state.request_worktree_gc();
        state.confirm_worktree_gc();
        assert!(!state.show_worktree_gc_confirm);
        let hits = mock.hits.lock().unwrap().clone();
        let gc = hits
            .iter()
            .find(|h| h.method == "worktree.gc")
            .expect("worktree.gc");
        assert_eq!(gc.params, json!({ "dryRun": false }));
        assert!(gc.params.get("prefix").is_none());
        assert!(gc.params.get("branchPrefix").is_none());
        assert_eq!(state.toast.as_deref(), Some("очистка: deleted=0"));
    }

    #[test]
    fn md_export_does_not_toast_stale_pdf_leftover() {
        let mock = start_artifacts_state_mock();
        let session = connect(&pid(&mock.origin)).expect("online");
        let mut state = online_state(session);
        state.selected_artifact_id = Some("art-1".into());
        let exported = state.export_selected_markdown().expect("md");
        assert_eq!(exported.0, "art-1.md");
        assert_eq!(exported.1, "# Auth");
        assert_ne!(state.toast.as_deref(), Some("PDF не поддерживается"));
        state.selected_artifact_id = None;
        assert!(state.export_selected_markdown().is_none());
        assert_ne!(state.toast.as_deref(), Some("PDF не поддерживается"));
    }
}
