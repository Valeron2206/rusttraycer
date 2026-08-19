//! Session/UI state. Live host: health + handshake + ping, then workspace/task catalog.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::discovery::{self, DiscoverError};
use crate::rpc::{GitDiffOk, GitStatusOk, Worktree};
use crate::ws::{self, ApplyOutcome, WsBridge, WsIncoming};

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
}

impl AppState {
    pub fn new() -> Self {
        let demo = std::env::var("RT_GUI_DEMO")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

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

    pub fn wants_repaint(&self) -> bool {
        self.ws.is_some() && self.screen == Screen::Canvas
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
                        let same_host = self
                            .session_host_id
                            .as_deref()
                            .map(|id| id == session.host_id)
                            .unwrap_or(false);
                        self.session_token = Some(session.session_token.clone());
                        self.session_host_id = Some(session.host_id.clone());
                        self.start_ws(&session);
                        self.session = Some(session);
                        self.discover_error = None;
                        self.host_status = HostStatus::Online;
                        self.last_rpc = Some(Instant::now());
                        self.ws_banner = None;
                        self.refresh_tasks_catalog();
                        if same_host && self.screen == Screen::Canvas {
                            if let Some(id) = self.selected_task_id.clone() {
                                self.reload_canvas(&id);
                                self.ws_subscribe(&id);
                            }
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
            WsIncoming::Disconnected { .. } => {
                if self.is_online() {
                    self.ws_banner = Some(
                        "Соединение с host потеряно, переподключение…".into(),
                    );
                }
            }
            WsIncoming::Reconnected => {
                self.ws_banner = None;
                if self.is_online() {
                    if let Some(id) = self.selected_task_id.clone() {
                        self.reload_canvas(&id);
                        self.ws_subscribe(&id);
                    }
                }
            }
        }
    }

    fn apply_ws_event(&mut self, event: ws::WsEvent) {
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
            ApplyOutcome::Appended | ApplyOutcome::Deduped | ApplyOutcome::Ignored => {}
        }
    }

    fn ws_subscribe(&self, task_id: &str) {
        if let Some(ws) = &self.ws {
            ws.subscribe(task_id.to_string());
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
            && self.agents_for_selected_task().is_empty()
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
        let Some(workspace_id) = self.workspace_id.clone() else {
            self.git_status = None;
            self.git_diff = None;
            return;
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
            Ok(status) => self.git_status = Some(status),
            Err(err) => {
                self.git_status = None;
                self.git_note = Some(err.as_label());
            }
        }
        self.load_git_diff();
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
                self.git_note = Some(err.as_label());
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
        self.selected_task_id = Some(id.clone());
        self.screen = Screen::Canvas;
        self.selected_file = None;
        self.file_preview = None;
        if self.demo {
            return;
        }
        self.reload_canvas(&id);
        self.ws_subscribe(&id);
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
                self.agents
                    .extend(items.into_iter().map(AgentStub::from));
                let still_valid = self
                    .selected_agent_id
                    .as_ref()
                    .map(|id| self.agents.iter().any(|a| &a.id == id && a.task_id == task_id))
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
                self.messages = messages.into_iter().map(ChatMessage::from).collect();
            }
            Err(err) => {
                self.toast = Some(err.as_label());
            }
        }
        self.load_git_panel();
    }

    fn load_file_tree_root(&mut self) {
        let Some(workspace_id) = self.workspace_id.clone() else {
            return;
        };
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.files_tree_for(&workspace_id, "", self.worktree_id()) {
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
            return;
        }
        let Some(task_id) = self.selected_task_id.clone() else {
            return;
        };
        let Some(session) = self.session.clone() else {
            return;
        };
        match session.agent_create(&task_id, "cli.generic") {
            Ok(agent) => {
                let stub = AgentStub::from(agent);
                self.selected_agent_id = Some(stub.id.clone());
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
        self.load_selected_agent();
    }

    pub fn send_composer(&mut self) {
        if !self.composer_enabled() {
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
                self.toast = Some(err.as_label());
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
            match session.files_tree_for(&workspace_id, &path, self.worktree_id()) {
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
        match session.files_read_for(&workspace_id, &path, self.worktree_id()) {
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
}
