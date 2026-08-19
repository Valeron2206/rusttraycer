//! Durable SQLite store for the RustTraycer host. One writer per process.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

const MIGRATION_0001: &str = include_str!("../migrations/0001_init.sql");
const MIGRATION_0002: &str = include_str!("../migrations/0002_worktrees.sql");
const MIGRATION_0003: &str = include_str!("../migrations/0003_policies.sql");
const MIGRATION_0004: &str = include_str!("../migrations/0004_terminal.sql");
const MIGRATION_0005: &str = include_str!("../migrations/0005_artifacts.sql");
const MIGRATION_0006: &str = include_str!("../migrations/0006_loops.sql");

/// RFC3339 UTC timestamp (millis, Z suffix).
pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("not found")]
    NotFound,
    #[error("workspace path invalid: {0}")]
    WorkspacePathInvalid(String),
    #[error("invalid params: {0}")]
    InvalidParams(String),
    #[error("unsupported schema version: {0}")]
    UnsupportedSchema(String),
    #[error("database: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

impl StorageError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::WorkspacePathInvalid(_) => "workspace_path_invalid",
            Self::InvalidParams(_) => "invalid_params",
            Self::UnsupportedSchema(_) | Self::Database(_) | Self::Io(_) => "internal",
        }
    }
}

pub type Result<T> = std::result::Result<T, StorageError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostRow {
    pub id: String,
    pub name: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub host_id: String,
    pub path: String,
    pub name: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Open,
    Archived,
}

impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Archived => "archived",
        }
    }

    fn parse(s: &str) -> Result<Self> {
        match s {
            "open" => Ok(Self::Open),
            "archived" => Ok(Self::Archived),
            other => Err(StorageError::InvalidParams(format!(
                "unknown task status: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskFilter {
    Open,
    Archived,
    All,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub title: String,
    pub status: TaskStatus,
    pub created_at: String,
    pub updated_at: String,
    pub workspace_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Idle,
    Running,
    Error,
}

impl AgentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::Error => "error",
        }
    }

    fn parse(s: &str) -> Result<Self> {
        match s {
            "idle" => Ok(Self::Idle),
            "running" => Ok(Self::Running),
            "error" => Ok(Self::Error),
            other => Err(StorageError::InvalidParams(format!(
                "unknown agent status: {other}"
            ))),
        }
    }
}

/// Wire/domain id of a coding-agent harness. MVP value: `"cli.generic"`.
///
/// This is **not** an agent type, **not** an interface, and **not** a shell.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HarnessId(String);

impl HarnessId {
    pub const CLI_GENERIC: &'static str = "cli.generic";

    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn cli_generic() -> Self {
        Self::new(Self::CLI_GENERIC)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for HarnessId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for HarnessId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl AsRef<str> for HarnessId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for HarnessId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl PartialEq<str> for HarnessId {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for HarnessId {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    pub id: String,
    pub task_id: String,
    pub host_id: String,
    pub parent_id: Option<String>,
    pub interface: String,
    pub provider: HarnessId,
    pub status: AgentStatus,
    pub run_location: String,
    pub created_at: String,
    pub provider_session_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

impl MessageRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::System => "system",
            Self::Tool => "tool",
        }
    }

    fn parse(s: &str) -> Result<Self> {
        match s {
            "user" => Ok(Self::User),
            "assistant" => Ok(Self::Assistant),
            "system" => Ok(Self::System),
            "tool" => Ok(Self::Tool),
            other => Err(StorageError::InvalidParams(format!(
                "unknown message role: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub agent_id: String,
    pub role: MessageRole,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Worktree {
    pub id: String,
    pub workspace_id: String,
    pub agent_id: String,
    pub path: String,
    pub branch: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Default)]
pub struct Counts {
    pub workspace_count: i64,
    pub task_count: i64,
    pub agent_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyRow {
    pub id: String,
    pub workspace_id: Option<String>,
    pub agent_id: Option<String>,
    pub mode: String,
    pub scope: String,
    pub yolo: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub id: String,
    pub task_id: String,
    pub parent_id: Option<String>,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub status: Option<String>,
    pub assignee: Option<String>,
    pub source_message_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub id: String,
    pub body: String,
    pub created_at: String,
}

pub struct ArtifactCreateInput<'a> {
    pub task_id: &'a str,
    pub parent_id: Option<&'a str>,
    pub kind: &'a str,
    pub title: &'a str,
    pub body: &'a str,
    pub assignee: Option<&'a str>,
    pub source_message_id: Option<&'a str>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopRow {
    pub id: String,
    pub task_id: String,
    pub agent_a: String,
    pub agent_b: String,
    pub max_iterations: i64,
    pub budget_turns: i64,
    pub iteration: i64,
    pub turns: i64,
    pub status: String,
    pub reason: Option<String>,
    pub prompt: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentThread {
    pub id: String,
    pub artifact_id: String,
    pub anchor_start: i64,
    pub anchor_end: i64,
    pub resolved: bool,
    pub comments: Vec<Comment>,
    pub created_at: String,
    pub updated_at: String,
}

const ARTIFACT_LIST_CAP: usize = 500;
const MAX_ARTIFACT_TITLE: usize = 200;
const MAX_ARTIFACT_BODY: usize = 1_048_576;

/// Single-writer SQLite store. Cheap to clone (`Arc<Mutex<Connection>>`).
#[derive(Clone)]
pub struct Store {
    conn: Arc<Mutex<Connection>>,
    path: PathBuf,
}

impl Store {
    /// Open (or create) `host.db`, apply pragmas, migrate, then recover.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let conn = Connection::open(&path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(Duration::from_millis(5000))?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
            path,
        };
        store.migrate()?;
        store.recover()?;
        Ok(store)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store").field("path", &self.path).finish()
    }
}

impl Store {
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn.lock().map_err(|_| {
            tracing::error!("store mutex poisoned");
            StorageError::InvalidParams("store mutex poisoned".into())
        })
    }

    pub fn migrate(&self) -> Result<()> {
        self.migrate_inner().inspect_err(|e| {
            tracing::error!(error = %e, "migrate failed");
        })
    }

    fn migrate_inner(&self) -> Result<()> {
        let conn = self.lock()?;
        let current: Option<String> = conn
            .query_row(
                "SELECT value FROM schema_meta WHERE key = 'schema'",
                [],
                |r| r.get(0),
            )
            .optional()
            .or_else(|e| {
                if e.to_string().contains("no such table") {
                    Ok(None)
                } else {
                    Err(e)
                }
            })?;
        match current.as_deref() {
            Some("1") => {
                conn.execute_batch(MIGRATION_0002)?;
                conn.execute_batch(MIGRATION_0003)?;
                conn.execute_batch(MIGRATION_0004)?;
                conn.execute_batch(MIGRATION_0005)?;
                conn.execute_batch(MIGRATION_0006)?;
                Ok(())
            }
            Some("2") => {
                conn.execute_batch(MIGRATION_0003)?;
                conn.execute_batch(MIGRATION_0004)?;
                conn.execute_batch(MIGRATION_0005)?;
                conn.execute_batch(MIGRATION_0006)?;
                Ok(())
            }
            Some("3") => {
                conn.execute_batch(MIGRATION_0004)?;
                conn.execute_batch(MIGRATION_0005)?;
                conn.execute_batch(MIGRATION_0006)?;
                Ok(())
            }
            Some("4") => {
                conn.execute_batch(MIGRATION_0005)?;
                conn.execute_batch(MIGRATION_0006)?;
                Ok(())
            }
            Some("5") => {
                conn.execute_batch(MIGRATION_0006)?;
                Ok(())
            }
            Some("6") => Ok(()),
            Some(other) => Err(StorageError::UnsupportedSchema(other.to_string())),
            None => {
                conn.execute_batch(MIGRATION_0001)?;
                conn.execute_batch(MIGRATION_0002)?;
                conn.execute_batch(MIGRATION_0003)?;
                conn.execute_batch(MIGRATION_0004)?;
                conn.execute_batch(MIGRATION_0005)?;
                conn.execute_batch(MIGRATION_0006)?;
                Ok(())
            }
        }
    }

    /// After migrate, before listen: running agents become error.
    pub fn recover(&self) -> Result<usize> {
        let n = self.set_running_agents_to_error()?;
        self.recover_running_loops_to_stopped()?;
        Ok(n)
    }

    pub fn recover_running(&self) -> Result<usize> {
        self.recover()
    }

    pub fn host_get(&self) -> Result<HostRow> {
        let conn = self.lock()?;
        conn.query_row("SELECT id, name, created_at FROM host LIMIT 1", [], |r| {
            Ok(HostRow {
                id: r.get(0)?,
                name: r.get(1)?,
                created_at: r.get(2)?,
            })
        })
        .optional()?
        .ok_or(StorageError::NotFound)
    }

    /// INSERT if the table is empty. Existing id/name are never rewritten.
    pub fn host_insert_if_absent(&self, id: &str, name: &str) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT OR IGNORE INTO host (id, name, created_at)
             SELECT ?1, ?2, ?3
             WHERE NOT EXISTS (SELECT 1 FROM host)",
            params![id, name, now_rfc3339()],
        )?;
        Ok(())
    }

    pub fn workspace_list(&self) -> Result<Vec<Workspace>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, host_id, path, name, created_at FROM workspaces ORDER BY created_at ASC, id ASC",
        )?;
        let rows = stmt.query_map([], map_workspace)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// `path` is already canonical. Store does not touch the filesystem.
    /// UNIQUE(path) conflict returns the existing row.
    pub fn workspace_add(&self, path: impl AsRef<str>, name: &str) -> Result<Workspace> {
        let path = path.as_ref();
        let host = self.host_get()?;
        let id = new_id();
        let created_at = now_rfc3339();
        let conn = self.lock()?;
        conn.execute(
            "INSERT OR IGNORE INTO workspaces (id, host_id, path, name, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, host.id, path, name, created_at],
        )?;
        conn.query_row(
            "SELECT id, host_id, path, name, created_at FROM workspaces WHERE path = ?1",
            [path],
            map_workspace,
        )
        .map_err(Into::into)
    }

    pub fn workspace_get(&self, id: &str) -> Result<Option<Workspace>> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT id, host_id, path, name, created_at FROM workspaces WHERE id = ?1",
            [id],
            map_workspace,
        )
        .optional()
        .map_err(Into::into)
    }

    fn task_workspace_ids(conn: &Connection, task_id: &str) -> Result<Vec<String>> {
        let mut stmt = conn.prepare(
            "SELECT workspace_id FROM task_workspaces WHERE task_id = ?1 ORDER BY workspace_id",
        )?;
        let rows = stmt.query_map([task_id], |r| r.get(0))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn row_to_task(
        conn: &Connection,
        id: String,
        title: String,
        status: String,
        created_at: String,
        updated_at: String,
    ) -> Result<Task> {
        let workspace_ids = Self::task_workspace_ids(conn, &id)?;
        Ok(Task {
            id,
            title,
            status: TaskStatus::parse(&status)?,
            created_at,
            updated_at,
            workspace_ids,
        })
    }

    pub fn task_list(&self, filter: TaskFilter) -> Result<Vec<Task>> {
        let conn = self.lock()?;
        let sql = match filter {
            TaskFilter::Open => {
                "SELECT id, title, status, created_at, updated_at FROM tasks WHERE status = 'open' ORDER BY updated_at DESC, id DESC"
            }
            TaskFilter::Archived => {
                "SELECT id, title, status, created_at, updated_at FROM tasks WHERE status = 'archived' ORDER BY updated_at DESC, id DESC"
            }
            TaskFilter::All => {
                "SELECT id, title, status, created_at, updated_at FROM tasks ORDER BY updated_at DESC, id DESC"
            }
        };
        let mut stmt = conn.prepare(sql)?;
        let raw: Vec<(String, String, String, String, String)> = stmt
            .query_map([], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut out = Vec::with_capacity(raw.len());
        for (id, title, status, created_at, updated_at) in raw {
            out.push(Self::row_to_task(
                &conn, id, title, status, created_at, updated_at,
            )?);
        }
        Ok(out)
    }

    pub fn task_create(&self, title: &str, workspace_id: &str) -> Result<Task> {
        let id = new_id();
        let now = now_rfc3339();
        {
            let mut conn = self.lock()?;
            let tx = conn.transaction()?;
            let ws_ok: Option<String> = tx
                .query_row(
                    "SELECT id FROM workspaces WHERE id = ?1",
                    [workspace_id],
                    |r| r.get(0),
                )
                .optional()?;
            if ws_ok.is_none() {
                return Err(StorageError::NotFound);
            }
            tx.execute(
                "INSERT INTO tasks (id, title, status, created_at, updated_at) VALUES (?1, ?2, 'open', ?3, ?3)",
                params![id, title, now],
            )?;
            tx.execute(
                "INSERT INTO task_workspaces (task_id, workspace_id) VALUES (?1, ?2)",
                params![id, workspace_id],
            )?;
            tx.commit()?;
        }
        Ok(Task {
            id,
            title: title.to_string(),
            status: TaskStatus::Open,
            created_at: now.clone(),
            updated_at: now,
            workspace_ids: vec![workspace_id.to_string()],
        })
    }

    pub fn task_get(&self, id: &str) -> Result<Option<Task>> {
        let conn = self.lock()?;
        let row: Option<(String, String, String, String, String)> = conn
            .query_row(
                "SELECT id, title, status, created_at, updated_at FROM tasks WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .optional()?;
        match row {
            None => Ok(None),
            Some((id, title, status, created_at, updated_at)) => Ok(Some(Self::row_to_task(
                &conn, id, title, status, created_at, updated_at,
            )?)),
        }
    }

    pub fn task_rename(&self, id: &str, title: &str) -> Result<()> {
        let conn = self.lock()?;
        let n = conn.execute(
            "UPDATE tasks SET title = ?1, updated_at = ?2 WHERE id = ?3",
            params![title, now_rfc3339(), id],
        )?;
        if n == 0 {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }

    /// Already archived → no-op (updated_at left alone).
    pub fn task_archive(&self, id: &str) -> Result<()> {
        let conn = self.lock()?;
        let status: Option<String> = conn
            .query_row("SELECT status FROM tasks WHERE id = ?1", [id], |r| r.get(0))
            .optional()?;
        match status.as_deref() {
            None => Err(StorageError::NotFound),
            Some("archived") => Ok(()),
            Some(_) => {
                conn.execute(
                    "UPDATE tasks SET status = 'archived', updated_at = ?1 WHERE id = ?2",
                    params![now_rfc3339(), id],
                )?;
                Ok(())
            }
        }
    }

    pub fn task_touch(&self, id: &str) -> Result<()> {
        let conn = self.lock()?;
        let n = conn.execute(
            "UPDATE tasks SET updated_at = ?1 WHERE id = ?2",
            params![now_rfc3339(), id],
        )?;
        if n == 0 {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }

    pub fn agent_list(&self, task_id: &str) -> Result<Vec<Agent>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, task_id, host_id, parent_id, interface, provider, status, run_location, created_at, provider_session_id \
             FROM agents WHERE task_id = ?1 ORDER BY created_at ASC, id ASC",
        )?;
        let rows = stmt.query_map([task_id], map_agent_tuple)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(agent_from_tuple(row?)?);
        }
        Ok(out)
    }

    pub fn agent_create(
        &self,
        task_id: &str,
        host_id: &str,
        provider: impl Into<HarnessId>,
    ) -> Result<Agent> {
        self.agent_create_interface(task_id, host_id, provider, "chat", None)
    }

    pub fn agent_create_interface(
        &self,
        task_id: &str,
        host_id: &str,
        provider: impl Into<HarnessId>,
        interface: &str,
        parent_id: Option<&str>,
    ) -> Result<Agent> {
        let provider = provider.into();
        if interface != "chat" && interface != "terminal" {
            return Err(StorageError::InvalidParams(format!(
                "interface must be chat|terminal, got {interface}"
            )));
        }
        if self.task_get(task_id)?.is_none() {
            return Err(StorageError::NotFound);
        }
        let id = new_id();
        if let Some(pid) = parent_id {
            self.assert_agent_parent_ok(task_id, host_id, Some(id.as_str()), pid)?;
        }
        let created_at = now_rfc3339();
        {
            let conn = self.lock()?;
            conn.execute(
                "INSERT INTO agents (id, task_id, host_id, parent_id, interface, provider, status, run_location, created_at, provider_session_id) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'idle', 'local', ?7, NULL)",
                params![
                    id,
                    task_id,
                    host_id,
                    parent_id,
                    interface,
                    provider.as_str(),
                    created_at
                ],
            )?;
        }
        Ok(Agent {
            id,
            task_id: task_id.to_string(),
            host_id: host_id.to_string(),
            parent_id: parent_id.map(str::to_string),
            interface: interface.to_string(),
            provider,
            status: AgentStatus::Idle,
            run_location: "local".into(),
            created_at,
            provider_session_id: None,
        })
    }

    pub fn agent_get(&self, id: &str) -> Result<Option<Agent>> {
        let conn = self.lock()?;
        let row = conn
            .query_row(
                "SELECT id, task_id, host_id, parent_id, interface, provider, status, run_location, created_at, provider_session_id \
                 FROM agents WHERE id = ?1",
                [id],
                map_agent_tuple,
            )
            .optional()?;
        match row {
            None => Ok(None),
            Some(t) => Ok(Some(agent_from_tuple(t)?)),
        }
    }

    pub fn agent_set_status(&self, id: &str, status: AgentStatus) -> Result<()> {
        let conn = self.lock()?;
        let n = conn.execute(
            "UPDATE agents SET status = ?1 WHERE id = ?2",
            params![status.as_str(), id],
        )?;
        if n == 0 {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }

    /// Persist a vendor session id. Allowed only when `interface=terminal`.
    /// Chat agents must keep `provider_session_id` NULL (0004 CHECK).
    pub fn agent_set_provider_session_id(&self, agent_id: &str, session_id: &str) -> Result<()> {
        let agent = self.agent_get(agent_id)?.ok_or(StorageError::NotFound)?;
        if agent.interface != "terminal" {
            return Err(StorageError::InvalidParams(
                "provider_session_id is only allowed when interface=terminal".into(),
            ));
        }
        if session_id.is_empty() {
            return Err(StorageError::InvalidParams(
                "provider_session_id must be non-empty".into(),
            ));
        }
        let conn = self.lock()?;
        let n = conn.execute(
            "UPDATE agents SET provider_session_id = ?1 WHERE id = ?2",
            params![session_id, agent_id],
        )?;
        if n == 0 {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }

    pub fn message_append(
        &self,
        agent_id: &str,
        role: MessageRole,
        content: &str,
    ) -> Result<Message> {
        if self.agent_get(agent_id)?.is_none() {
            return Err(StorageError::NotFound);
        }
        let id = new_id();
        let created_at = now_rfc3339();
        {
            let conn = self.lock()?;
            conn.execute(
                "INSERT INTO messages (id, agent_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, agent_id, role.as_str(), content, created_at],
            )?;
        }
        Ok(Message {
            id,
            agent_id: agent_id.to_string(),
            role,
            content: content.to_string(),
            created_at,
        })
    }

    pub fn message_list(&self, agent_id: &str) -> Result<Vec<Message>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, role, content, created_at FROM messages \
             WHERE agent_id = ?1 ORDER BY created_at ASC, id ASC",
        )?;
        let rows = stmt.query_map([agent_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, agent_id, role, content, created_at) = row?;
            out.push(Message {
                id,
                agent_id,
                role: MessageRole::parse(&role)?,
                content,
                created_at,
            });
        }
        Ok(out)
    }

    pub fn last_message_at(&self, agent_id: &str) -> Result<Option<String>> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT MAX(created_at) FROM messages WHERE agent_id = ?1",
            [agent_id],
            |r| r.get::<_, Option<String>>(0),
        )
        .map_err(Into::into)
    }

    pub fn counts(&self) -> Result<Counts> {
        let conn = self.lock()?;
        let workspace_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM workspaces", [], |r| r.get(0))?;
        let task_count: i64 = conn.query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))?;
        let agent_count: i64 = conn.query_row("SELECT COUNT(*) FROM agents", [], |r| r.get(0))?;
        Ok(Counts {
            workspace_count,
            task_count,
            agent_count,
        })
    }

    pub fn policy_get_for_agent(&self, agent_id: &str) -> Result<Option<PolicyRow>> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT id, workspace_id, agent_id, mode, scope, yolo, updated_at \
             FROM policies WHERE agent_id = ?1",
            [agent_id],
            map_policy,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn policy_get_for_workspace(&self, workspace_id: &str) -> Result<Option<PolicyRow>> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT id, workspace_id, agent_id, mode, scope, yolo, updated_at \
             FROM policies WHERE workspace_id = ?1",
            [workspace_id],
            map_policy,
        )
        .optional()
        .map_err(Into::into)
    }

    /// Insert or replace the unique policy row for the given agent xor workspace.
    pub fn policy_upsert(
        &self,
        agent_id: Option<&str>,
        workspace_id: Option<&str>,
        mode: &str,
        scope: &str,
        yolo: bool,
    ) -> Result<PolicyRow> {
        match (agent_id, workspace_id) {
            (Some(_), None) | (None, Some(_)) => {}
            _ => {
                return Err(StorageError::InvalidParams(
                    "exactly one of agent_id / workspace_id is required".into(),
                ));
            }
        }
        if !matches!(mode, "ask" | "allow-always" | "deny") {
            return Err(StorageError::InvalidParams(format!(
                "mode must be ask|allow-always|deny, got {mode}"
            )));
        }
        if !matches!(scope, "agent" | "workspace") {
            return Err(StorageError::InvalidParams(format!(
                "scope must be agent|workspace, got {scope}"
            )));
        }
        let updated_at = now_rfc3339();
        let yolo_i = i64::from(yolo);
        let existing = if let Some(aid) = agent_id {
            self.policy_get_for_agent(aid)?
        } else if let Some(wid) = workspace_id {
            self.policy_get_for_workspace(wid)?
        } else {
            None
        };
        let id = match existing {
            Some(row) => {
                let conn = self.lock()?;
                conn.execute(
                    "UPDATE policies SET mode = ?1, scope = ?2, yolo = ?3, updated_at = ?4 WHERE id = ?5",
                    params![mode, scope, yolo_i, updated_at, row.id],
                )?;
                row.id
            }
            None => {
                let id = new_id();
                let conn = self.lock()?;
                conn.execute(
                    "INSERT INTO policies (id, workspace_id, agent_id, mode, scope, yolo, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![id, workspace_id, agent_id, mode, scope, yolo_i, updated_at],
                )?;
                id
            }
        };
        Ok(PolicyRow {
            id,
            workspace_id: workspace_id.map(str::to_string),
            agent_id: agent_id.map(str::to_string),
            mode: mode.to_string(),
            scope: scope.to_string(),
            yolo,
            updated_at,
        })
    }

    /// True if any stored policy row has yolo enabled (agent or workspace).
    pub fn policy_any_yolo(&self) -> Result<bool> {
        let conn = self.lock()?;
        let found: Option<i64> = conn
            .query_row("SELECT 1 FROM policies WHERE yolo = 1 LIMIT 1", [], |r| {
                r.get(0)
            })
            .optional()?;
        Ok(found.is_some())
    }

    pub fn worktree_get_by_agent(&self, agent_id: &str) -> Result<Option<Worktree>> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT id, workspace_id, agent_id, path, branch, created_at              FROM worktrees WHERE agent_id = ?1",
            [agent_id],
            map_worktree,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn worktree_get(&self, id: &str) -> Result<Option<Worktree>> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT id, workspace_id, agent_id, path, branch, created_at              FROM worktrees WHERE id = ?1",
            [id],
            map_worktree,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn worktree_list(&self, workspace_id: &str) -> Result<Vec<Worktree>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, workspace_id, agent_id, path, branch, created_at              FROM worktrees WHERE workspace_id = ?1 ORDER BY created_at ASC, id ASC",
        )?;
        let rows = stmt.query_map([workspace_id], map_worktree)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn worktree_insert(
        &self,
        workspace_id: &str,
        agent_id: &str,
        path: &str,
        branch: &str,
    ) -> Result<Worktree> {
        let id = new_id();
        let created_at = now_rfc3339();
        {
            let mut conn = self.lock()?;
            let tx = conn.transaction()?;
            let ws_ok: Option<String> = tx
                .query_row(
                    "SELECT id FROM workspaces WHERE id = ?1",
                    [workspace_id],
                    |r| r.get(0),
                )
                .optional()?;
            if ws_ok.is_none() {
                return Err(StorageError::NotFound);
            }
            let ag_ok: Option<String> = tx
                .query_row("SELECT id FROM agents WHERE id = ?1", [agent_id], |r| {
                    r.get(0)
                })
                .optional()?;
            if ag_ok.is_none() {
                return Err(StorageError::NotFound);
            }
            tx.execute(
                "INSERT INTO worktrees (id, workspace_id, agent_id, path, branch, created_at)                  VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![id, workspace_id, agent_id, path, branch, created_at],
            )?;
            tx.commit()?;
        }
        Ok(Worktree {
            id,
            workspace_id: workspace_id.to_string(),
            agent_id: agent_id.to_string(),
            path: path.to_string(),
            branch: branch.to_string(),
            created_at,
        })
    }

    pub fn agent_set_run_location(&self, id: &str, run_location: &str) -> Result<()> {
        if run_location != "local" && run_location != "worktree" {
            return Err(StorageError::InvalidParams(format!(
                "run_location must be local|worktree, got {run_location}"
            )));
        }
        let conn = self.lock()?;
        let n = conn.execute(
            "UPDATE agents SET run_location = ?1 WHERE id = ?2",
            params![run_location, id],
        )?;
        if n == 0 {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }

    pub fn set_running_agents_to_error(&self) -> Result<usize> {
        let conn = self.lock()?;
        let n = conn.execute(
            "UPDATE agents SET status = 'error' WHERE status = 'running'",
            [],
        )?;
        Ok(n)
    }

    /// Fast doctor check: connection alive + `PRAGMA quick_check`.
    pub fn quick_ok(&self) -> Result<bool> {
        let conn = self.lock()?;
        let s: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
        Ok(s == "ok")
    }

    pub fn integrity_ok(&self) -> Result<bool> {
        self.quick_ok()
    }

    pub fn checkpoint(&self) -> Result<()> {
        self.checkpoint_inner().inspect_err(|e| {
            tracing::error!(error = %e, "checkpoint failed");
        })
    }

    fn checkpoint_inner(&self) -> Result<()> {
        let conn = self.lock()?;
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
        Ok(())
    }

    pub fn artifact_create(&self, input: ArtifactCreateInput<'_>) -> Result<Artifact> {
        validate_title_len(input.title)?;
        validate_body_len(input.body)?;
        let status = match input.kind {
            "spec" | "review" => {
                if input.assignee.is_some() {
                    return Err(StorageError::InvalidParams(
                        "spec/review cannot have assignee".into(),
                    ));
                }
                None
            }
            "ticket" | "story" => Some("todo"),
            other => {
                return Err(StorageError::InvalidParams(format!(
                    "kind must be spec|ticket|story|review, got {other}"
                )));
            }
        };
        validate_kind_fields(input.kind, status, input.assignee)?;
        if self.task_get(input.task_id)?.is_none() {
            return Err(StorageError::NotFound);
        }
        if let Some(pid) = input.parent_id {
            self.assert_parent_ok(input.task_id, None, pid)?;
        }
        if let Some(mid) = input.source_message_id {
            if mid.is_empty() {
                return Err(StorageError::InvalidParams(
                    "sourceMessageId must be non-empty when set".into(),
                ));
            }
        }
        let id = new_id();
        let now = now_rfc3339();
        {
            let conn = self.lock()?;
            conn.execute(
                "INSERT INTO artifacts                  (id, task_id, parent_id, kind, title, body, status, assignee, source_message_id, created_at, updated_at)                  VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    id,
                    input.task_id,
                    input.parent_id,
                    input.kind,
                    input.title,
                    input.body,
                    status,
                    input.assignee,
                    input.source_message_id,
                    now,
                    now
                ],
            )?;
        }
        self.artifact_get(&id)?
            .ok_or_else(|| StorageError::InvalidParams("artifact insert vanished".into()))
    }

    pub fn artifact_get(&self, id: &str) -> Result<Option<Artifact>> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT id, task_id, parent_id, kind, title, body, status, assignee, source_message_id, created_at, updated_at \
             FROM artifacts WHERE id = ?1",
            [id],
            map_artifact,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn artifact_list(
        &self,
        task_id: &str,
        kind: Option<&str>,
    ) -> Result<(Vec<Artifact>, bool)> {
        if let Some(k) = kind {
            if !matches!(k, "spec" | "ticket" | "story" | "review") {
                return Err(StorageError::InvalidParams(format!(
                    "kind must be spec|ticket|story|review, got {k}"
                )));
            }
        }
        let conn = self.lock()?;
        let sql = if kind.is_some() {
            "SELECT id, task_id, parent_id, kind, title, body, status, assignee, source_message_id, created_at, updated_at \
             FROM artifacts WHERE task_id = ?1 AND kind = ?2 \
             ORDER BY created_at ASC, id ASC LIMIT ?3"
        } else {
            "SELECT id, task_id, parent_id, kind, title, body, status, assignee, source_message_id, created_at, updated_at \
             FROM artifacts WHERE task_id = ?1 \
             ORDER BY created_at ASC, id ASC LIMIT ?2"
        };
        let cap = (ARTIFACT_LIST_CAP + 1) as i64;
        let mut stmt = conn.prepare(sql)?;
        let rows = if let Some(k) = kind {
            stmt.query_map(params![task_id, k, cap], map_artifact)?
        } else {
            stmt.query_map(params![task_id, cap], map_artifact)?
        };
        let mut items = rows.collect::<std::result::Result<Vec<_>, _>>()?;
        let truncated = items.len() > ARTIFACT_LIST_CAP;
        if truncated {
            items.truncate(ARTIFACT_LIST_CAP);
        }
        Ok((items, truncated))
    }

    pub fn artifact_update(
        &self,
        artifact_id: &str,
        title: Option<&str>,
        body: Option<&str>,
        status: Option<&str>,
        assignee: Option<Option<&str>>,
        parent_id: Option<Option<&str>>,
    ) -> Result<Artifact> {
        let current = self
            .artifact_get(artifact_id)?
            .ok_or(StorageError::NotFound)?;
        if let Some(t) = title {
            validate_title_len(t)?;
        }
        if let Some(b) = body {
            validate_body_len(b)?;
        }
        let new_title = title.unwrap_or(&current.title);
        let new_body = body.unwrap_or(&current.body);
        let new_status = match status {
            Some(s) => Some(s),
            None => current.status.as_deref(),
        };
        let new_assignee = match assignee {
            Some(a) => a,
            None => current.assignee.as_deref(),
        };
        validate_kind_fields(&current.kind, new_status, new_assignee)?;
        let new_parent = match parent_id {
            Some(p) => p,
            None => current.parent_id.as_deref(),
        };
        if let Some(pid) = new_parent {
            self.assert_parent_ok(&current.task_id, Some(artifact_id), pid)?;
        }
        let now = now_rfc3339();
        {
            let conn = self.lock()?;
            let n = conn.execute(
                "UPDATE artifacts SET title = ?1, body = ?2, status = ?3, assignee = ?4, parent_id = ?5, updated_at = ?6 \
                 WHERE id = ?7",
                params![
                    new_title,
                    new_body,
                    new_status,
                    new_assignee,
                    new_parent,
                    now,
                    artifact_id
                ],
            )?;
            if n == 0 {
                return Err(StorageError::NotFound);
            }
        }
        self.artifact_get(artifact_id)?
            .ok_or(StorageError::NotFound)
    }

    pub fn artifact_delete_tree(&self, artifact_id: &str) -> Result<Vec<String>> {
        if self.artifact_get(artifact_id)?.is_none() {
            return Err(StorageError::NotFound);
        }
        let mut ids = Vec::new();
        self.collect_artifact_descendants(artifact_id, &mut ids)?;
        // children first so parent_id FK stays satisfied
        ids.reverse();
        {
            let conn = self.lock()?;
            for id in &ids {
                conn.execute(
                    "DELETE FROM comments WHERE thread_id IN \
                     (SELECT id FROM comment_threads WHERE artifact_id = ?1)",
                    [id],
                )?;
                conn.execute("DELETE FROM comment_threads WHERE artifact_id = ?1", [id])?;
                conn.execute("DELETE FROM artifacts WHERE id = ?1", [id])?;
            }
        }
        Ok(ids)
    }

    pub fn comment_thread_create(
        &self,
        artifact_id: &str,
        anchor_start: i64,
        anchor_end: i64,
        body: &str,
    ) -> Result<CommentThread> {
        validate_body_len(body)?;
        if body.is_empty() {
            return Err(StorageError::InvalidParams(
                "comment body is required".into(),
            ));
        }
        let art = self
            .artifact_get(artifact_id)?
            .ok_or(StorageError::NotFound)?;
        validate_anchor(&art.body, anchor_start, anchor_end)?;
        let id = new_id();
        let now = now_rfc3339();
        let cid = new_id();
        {
            let conn = self.lock()?;
            conn.execute(
                "INSERT INTO comment_threads \
                 (id, artifact_id, anchor_start, anchor_end, resolved, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6)",
                params![id, artifact_id, anchor_start, anchor_end, now, now],
            )?;
            conn.execute(
                "INSERT INTO comments (id, thread_id, body, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![cid, id, body, now],
            )?;
        }
        self.comment_thread_get(&id)?
            .ok_or_else(|| StorageError::InvalidParams("comment thread insert vanished".into()))
    }

    pub fn comment_add(&self, thread_id: &str, body: &str) -> Result<CommentThread> {
        validate_body_len(body)?;
        if body.is_empty() {
            return Err(StorageError::InvalidParams(
                "comment body is required".into(),
            ));
        }
        if self.comment_thread_get(thread_id)?.is_none() {
            return Err(StorageError::NotFound);
        }
        let cid = new_id();
        let now = now_rfc3339();
        {
            let conn = self.lock()?;
            conn.execute(
                "INSERT INTO comments (id, thread_id, body, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![cid, thread_id, body, now],
            )?;
            conn.execute(
                "UPDATE comment_threads SET updated_at = ?1 WHERE id = ?2",
                params![now, thread_id],
            )?;
        }
        self.comment_thread_get(thread_id)?
            .ok_or(StorageError::NotFound)
    }

    pub fn comment_list(&self, artifact_id: &str) -> Result<Vec<CommentThread>> {
        if self.artifact_get(artifact_id)?.is_none() {
            return Err(StorageError::NotFound);
        }
        let ids: Vec<String> = {
            let conn = self.lock()?;
            let mut stmt = conn.prepare(
                "SELECT id FROM comment_threads WHERE artifact_id = ?1 \
                 ORDER BY created_at ASC, id ASC",
            )?;
            let rows = stmt.query_map([artifact_id], |r| r.get(0))?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        let mut out = Vec::new();
        for id in ids {
            if let Some(th) = self.comment_thread_get(&id)? {
                out.push(th);
            }
        }
        Ok(out)
    }

    pub fn comment_resolve(&self, thread_id: &str) -> Result<CommentThread> {
        if self.comment_thread_get(thread_id)?.is_none() {
            return Err(StorageError::NotFound);
        }
        let now = now_rfc3339();
        {
            let conn = self.lock()?;
            conn.execute(
                "UPDATE comment_threads SET resolved = 1, updated_at = ?1 WHERE id = ?2",
                params![now, thread_id],
            )?;
        }
        self.comment_thread_get(thread_id)?
            .ok_or(StorageError::NotFound)
    }

    pub fn comment_thread_get(&self, thread_id: &str) -> Result<Option<CommentThread>> {
        let conn = self.lock()?;
        let row = conn
            .query_row(
                "SELECT id, artifact_id, anchor_start, anchor_end, resolved, created_at, updated_at \
                 FROM comment_threads WHERE id = ?1",
                [thread_id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, i64>(2)?,
                        r.get::<_, i64>(3)?,
                        r.get::<_, i64>(4)?,
                        r.get::<_, String>(5)?,
                        r.get::<_, String>(6)?,
                    ))
                },
            )
            .optional()?;
        let Some((id, artifact_id, anchor_start, anchor_end, resolved, created_at, updated_at)) =
            row
        else {
            return Ok(None);
        };
        let mut stmt = conn.prepare(
            "SELECT id, body, created_at FROM comments WHERE thread_id = ?1 \
             ORDER BY created_at ASC, id ASC",
        )?;
        let comments = stmt
            .query_map([&id], |r| {
                Ok(Comment {
                    id: r.get(0)?,
                    body: r.get(1)?,
                    created_at: r.get(2)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(Some(CommentThread {
            id,
            artifact_id,
            anchor_start,
            anchor_end,
            resolved: resolved != 0,
            comments,
            created_at,
            updated_at,
        }))
    }

    pub fn clear_transcript(&self, agent_id: &str) -> Result<usize> {
        if self.agent_get(agent_id)?.is_none() {
            return Err(StorageError::NotFound);
        }
        let conn = self.lock()?;
        conn.execute(
            "UPDATE artifacts SET source_message_id = NULL \
             WHERE source_message_id IN (SELECT id FROM messages WHERE agent_id = ?1)",
            [agent_id],
        )?;
        let n = conn.execute("DELETE FROM messages WHERE agent_id = ?1", [agent_id])?;
        Ok(n)
    }

    fn assert_agent_parent_ok(
        &self,
        task_id: &str,
        host_id: &str,
        self_id: Option<&str>,
        parent_id: &str,
    ) -> Result<()> {
        if parent_id.is_empty() {
            return Err(StorageError::InvalidParams("parentId is empty".into()));
        }
        if self_id == Some(parent_id) {
            return Err(StorageError::InvalidParams(
                "parentId cannot be the agent itself".into(),
            ));
        }
        let parent = self
            .agent_get(parent_id)?
            .ok_or_else(|| StorageError::InvalidParams("parentId not found".into()))?;
        if parent.task_id != task_id {
            return Err(StorageError::InvalidParams(
                "parentId must belong to the same task".into(),
            ));
        }
        if parent.host_id != host_id {
            return Err(StorageError::InvalidParams(
                "parentId must belong to the same host".into(),
            ));
        }
        if let Some(sid) = self_id {
            let mut walk = parent.parent_id.clone();
            let mut guard = 0usize;
            while let Some(cur) = walk {
                if cur == sid {
                    return Err(StorageError::InvalidParams(
                        "parentId would create a cycle".into(),
                    ));
                }
                guard += 1;
                if guard > 10_000 {
                    return Err(StorageError::InvalidParams(
                        "parentId ancestor walk exceeded limit".into(),
                    ));
                }
                walk = self.agent_get(&cur)?.and_then(|a| a.parent_id);
            }
        }
        Ok(())
    }

    /// Lift children when a parent agent is detached/deleted: SET parent_id NULL.
    pub fn agent_lift_children(&self, parent_id: &str) -> Result<usize> {
        let conn = self.lock()?;
        let n = conn.execute(
            "UPDATE agents SET parent_id = NULL WHERE parent_id = ?1",
            [parent_id],
        )?;
        Ok(n)
    }

    pub fn agent_set_parent(&self, agent_id: &str, parent_id: Option<&str>) -> Result<()> {
        let agent = self.agent_get(agent_id)?.ok_or(StorageError::NotFound)?;
        if let Some(pid) = parent_id {
            self.assert_agent_parent_ok(&agent.task_id, &agent.host_id, Some(agent_id), pid)?;
        }
        let conn = self.lock()?;
        let n = conn.execute(
            "UPDATE agents SET parent_id = ?1 WHERE id = ?2",
            params![parent_id, agent_id],
        )?;
        if n == 0 {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }

    pub fn loop_insert(
        &self,
        task_id: &str,
        agent_a: &str,
        agent_b: &str,
        max_iterations: i64,
        budget_turns: i64,
        prompt: &str,
    ) -> Result<LoopRow> {
        if !(1..=32).contains(&max_iterations) {
            return Err(StorageError::InvalidParams(
                "maxIterations must be 1..32".into(),
            ));
        }
        if !(1..=64).contains(&budget_turns) {
            return Err(StorageError::InvalidParams(
                "budgetTurns must be 1..64".into(),
            ));
        }
        if self.task_get(task_id)?.is_none() {
            return Err(StorageError::NotFound);
        }
        let id = new_id();
        let now = now_rfc3339();
        {
            let conn = self.lock()?;
            conn.execute(
                "INSERT INTO loops (id, task_id, agent_a, agent_b, max_iterations, budget_turns, iteration, turns, status, reason, prompt, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, 0, 'running', NULL, ?7, ?8, ?8)",
                params![
                    id,
                    task_id,
                    agent_a,
                    agent_b,
                    max_iterations,
                    budget_turns,
                    prompt,
                    now
                ],
            )?;
        }
        self.loop_get(&id)?.ok_or(StorageError::NotFound)
    }

    pub fn loop_get(&self, id: &str) -> Result<Option<LoopRow>> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT id, task_id, agent_a, agent_b, max_iterations, budget_turns, iteration, turns, status, reason, prompt, created_at, updated_at
             FROM loops WHERE id = ?1",
            [id],
            map_loop_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn loop_update_progress(&self, id: &str, iteration: i64, turns: i64) -> Result<()> {
        let conn = self.lock()?;
        let n = conn.execute(
            "UPDATE loops SET iteration = ?1, turns = ?2, updated_at = ?3 WHERE id = ?4",
            params![iteration, turns, now_rfc3339(), id],
        )?;
        if n == 0 {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }

    pub fn loop_stop(&self, id: &str, reason: &str) -> Result<LoopRow> {
        let current = self.loop_get(id)?.ok_or(StorageError::NotFound)?;
        if current.status == "stopped" {
            return Ok(current);
        }
        {
            let conn = self.lock()?;
            let n = conn.execute(
                "UPDATE loops SET status = 'stopped', reason = ?1, updated_at = ?2 WHERE id = ?3 AND status = 'running'",
                params![reason, now_rfc3339(), id],
            )?;
            if n == 0 {
                // raced to stopped
            }
        }
        self.loop_get(id)?.ok_or(StorageError::NotFound)
    }

    pub fn recover_running_loops_to_stopped(&self) -> Result<usize> {
        let conn = self.lock()?;
        let n = conn.execute(
            "UPDATE loops SET status = 'stopped', reason = 'error', updated_at = ?1 WHERE status = 'running'",
            params![now_rfc3339()],
        )?;
        Ok(n)
    }

    fn collect_artifact_descendants(&self, root: &str, out: &mut Vec<String>) -> Result<()> {
        out.push(root.to_string());
        let children: Vec<String> = {
            let conn = self.lock()?;
            let mut stmt = conn.prepare(
                "SELECT id FROM artifacts WHERE parent_id = ?1 ORDER BY created_at ASC, id ASC",
            )?;
            let rows = stmt.query_map([root], |r| r.get(0))?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        for child in children {
            self.collect_artifact_descendants(&child, out)?;
        }
        Ok(())
    }

    fn assert_parent_ok(
        &self,
        task_id: &str,
        self_id: Option<&str>,
        parent_id: &str,
    ) -> Result<()> {
        if parent_id.is_empty() {
            return Err(StorageError::InvalidParams("parentId is empty".into()));
        }
        if self_id == Some(parent_id) {
            return Err(StorageError::InvalidParams(
                "parentId cannot be the artifact itself".into(),
            ));
        }
        let parent = self
            .artifact_get(parent_id)?
            .ok_or_else(|| StorageError::InvalidParams("parentId not found".into()))?;
        if parent.task_id != task_id {
            return Err(StorageError::InvalidParams(
                "parentId must belong to the same task".into(),
            ));
        }
        if let Some(sid) = self_id {
            let mut walk = parent.parent_id.clone();
            let mut guard = 0usize;
            while let Some(cur) = walk {
                if cur == sid {
                    return Err(StorageError::InvalidParams(
                        "parentId would create a cycle".into(),
                    ));
                }
                guard += 1;
                if guard > 10_000 {
                    return Err(StorageError::InvalidParams(
                        "parentId ancestor walk exceeded limit".into(),
                    ));
                }
                walk = self.artifact_get(&cur)?.and_then(|a| a.parent_id);
            }
        }
        Ok(())
    }
}

fn validate_title_len(title: &str) -> Result<()> {
    let n = title.chars().count();
    if !(1..=MAX_ARTIFACT_TITLE).contains(&n) {
        return Err(StorageError::InvalidParams(
            "title must be 1..200 characters".into(),
        ));
    }
    Ok(())
}

fn validate_body_len(body: &str) -> Result<()> {
    if body.len() > MAX_ARTIFACT_BODY {
        return Err(StorageError::InvalidParams(
            "body must be at most 1 MiB".into(),
        ));
    }
    Ok(())
}

fn validate_kind_fields(kind: &str, status: Option<&str>, assignee: Option<&str>) -> Result<()> {
    match kind {
        "spec" | "review" => {
            if status.is_some() || assignee.is_some() {
                return Err(StorageError::InvalidParams(
                    "spec/review cannot have status or assignee".into(),
                ));
            }
        }
        "ticket" | "story" => match status {
            Some("todo") | Some("in_progress") | Some("done") => {}
            Some(other) => {
                return Err(StorageError::InvalidParams(format!(
                    "status must be todo|in_progress|done, got {other}"
                )));
            }
            None => {
                return Err(StorageError::InvalidParams(
                    "ticket/story require status".into(),
                ));
            }
        },
        other => {
            return Err(StorageError::InvalidParams(format!(
                "kind must be spec|ticket|story|review, got {other}"
            )));
        }
    }
    Ok(())
}

fn validate_anchor(body: &str, start: i64, end: i64) -> Result<()> {
    let n = i64::try_from(body.chars().count())
        .map_err(|_| StorageError::InvalidParams("body is too large for anchors".into()))?;
    if start < 0 || end <= start || end > n {
        return Err(StorageError::InvalidParams(
            "anchorStart/anchorEnd must be UTF-8 codepoint offsets with end > start".into(),
        ));
    }
    Ok(())
}

fn map_artifact(r: &rusqlite::Row<'_>) -> rusqlite::Result<Artifact> {
    Ok(Artifact {
        id: r.get(0)?,
        task_id: r.get(1)?,
        parent_id: r.get(2)?,
        kind: r.get(3)?,
        title: r.get(4)?,
        body: r.get(5)?,
        status: r.get(6)?,
        assignee: r.get(7)?,
        source_message_id: r.get(8)?,
        created_at: r.get(9)?,
        updated_at: r.get(10)?,
    })
}

fn map_workspace(r: &rusqlite::Row<'_>) -> rusqlite::Result<Workspace> {
    Ok(Workspace {
        id: r.get(0)?,
        host_id: r.get(1)?,
        path: r.get(2)?,
        name: r.get(3)?,
        created_at: r.get(4)?,
    })
}

fn map_worktree(r: &rusqlite::Row<'_>) -> rusqlite::Result<Worktree> {
    Ok(Worktree {
        id: r.get(0)?,
        workspace_id: r.get(1)?,
        agent_id: r.get(2)?,
        path: r.get(3)?,
        branch: r.get(4)?,
        created_at: r.get(5)?,
    })
}

fn map_policy(r: &rusqlite::Row<'_>) -> rusqlite::Result<PolicyRow> {
    let yolo_i: i64 = r.get(5)?;
    Ok(PolicyRow {
        id: r.get(0)?,
        workspace_id: r.get(1)?,
        agent_id: r.get(2)?,
        mode: r.get(3)?,
        scope: r.get(4)?,
        yolo: yolo_i != 0,
        updated_at: r.get(6)?,
    })
}

type AgentTuple = (
    String,
    String,
    String,
    Option<String>,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
);

fn map_loop_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<LoopRow> {
    Ok(LoopRow {
        id: r.get(0)?,
        task_id: r.get(1)?,
        agent_a: r.get(2)?,
        agent_b: r.get(3)?,
        max_iterations: r.get(4)?,
        budget_turns: r.get(5)?,
        iteration: r.get(6)?,
        turns: r.get(7)?,
        status: r.get(8)?,
        reason: r.get(9)?,
        prompt: r.get(10)?,
        created_at: r.get(11)?,
        updated_at: r.get(12)?,
    })
}

fn map_agent_tuple(r: &rusqlite::Row<'_>) -> rusqlite::Result<AgentTuple> {
    Ok((
        r.get(0)?,
        r.get(1)?,
        r.get(2)?,
        r.get(3)?,
        r.get(4)?,
        r.get(5)?,
        r.get(6)?,
        r.get(7)?,
        r.get(8)?,
        r.get(9)?,
    ))
}

fn agent_from_tuple(t: AgentTuple) -> Result<Agent> {
    let (
        id,
        task_id,
        host_id,
        parent_id,
        interface,
        provider,
        status,
        run_location,
        created_at,
        provider_session_id,
    ) = t;
    Ok(Agent {
        id,
        task_id,
        host_id,
        parent_id,
        interface,
        provider: HarnessId::new(provider),
        status: AgentStatus::parse(&status)?,
        run_location,
        created_at,
        provider_session_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn open_store() -> (tempfile::TempDir, Store) {
        let dir = tempdir().unwrap();
        let store = Store::open(dir.path().join("host.db")).unwrap();
        (dir, store)
    }

    #[test]
    fn checkpoint_truncate_succeeds() {
        let (_tmp, store) = open_store();
        store.checkpoint().unwrap();
    }

    #[test]
    fn migration_sql_matches_contract() {
        assert!(!MIGRATION_0001.contains("IF NOT EXISTS"));
        assert!(MIGRATION_0001.contains("CREATE INDEX idx_workspaces_host"));
        assert!(MIGRATION_0001.contains("CREATE INDEX idx_tasks_status_updated"));
        assert!(MIGRATION_0001.contains("CREATE INDEX idx_messages_agent"));
        assert!(MIGRATION_0001.contains("parent_id    TEXT REFERENCES agents(id)"));
        assert!(MIGRATION_0001.contains("UNIQUE (path)"));
        assert!(
            MIGRATION_0001.contains("INSERT INTO schema_meta(key, value) VALUES ('schema', '1')")
        );
        assert!(!MIGRATION_0001.to_lowercase().contains("create table files"));
        assert!(!MIGRATION_0001.contains("schema_major"));
        assert!(!MIGRATION_0001.contains("ON DELETE CASCADE"));
    }

    #[test]
    fn host_id_is_stable() {
        let (_tmp, store) = open_store();
        let host_id = new_id();
        store.host_insert_if_absent(&host_id, "test-host").unwrap();
        store.host_insert_if_absent(&new_id(), "other").unwrap();
        let host = store.host_get().unwrap();
        assert_eq!(host.id, host_id);
        assert_eq!(host.name, "test-host");
    }

    #[test]
    fn workspace_add_is_idempotent_by_path_without_fs_check() {
        let (_tmp, store) = open_store();
        store.host_insert_if_absent(&new_id(), "h").unwrap();
        // Store must not require the path to exist on disk.
        let a = store.workspace_add("/canon/proj", "proj").unwrap();
        let b = store.workspace_add("/canon/proj", "ignored-name").unwrap();
        assert_eq!(a.id, b.id);
        assert_eq!(a.name, "proj");
        assert_eq!(b.name, "proj");
        assert_eq!(store.workspace_list().unwrap().len(), 1);
    }

    #[test]
    fn task_create_without_workspace_is_not_found() {
        let (_tmp, store) = open_store();
        store.host_insert_if_absent(&new_id(), "h").unwrap();
        let err = store.task_create("t", "no-such-ws").unwrap_err();
        assert_eq!(err.code(), "not_found");
    }

    #[test]
    fn message_list_order_is_created_at_then_id() {
        let (_tmp, store) = open_store();
        let host_id = new_id();
        store.host_insert_if_absent(&host_id, "h").unwrap();
        let ws = store.workspace_add("/p", "p").unwrap();
        let task = store.task_create("t", &ws.id).unwrap();
        let agent = store
            .agent_create(&task.id, &host_id, "cli.generic")
            .unwrap();
        let m1 = store
            .message_append(&agent.id, MessageRole::User, "first")
            .unwrap();
        let m2 = store
            .message_append(&agent.id, MessageRole::Assistant, "second")
            .unwrap();
        let msgs = store.message_list(&agent.id).unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].id, m1.id);
        assert_eq!(msgs[1].id, m2.id);
        assert_eq!(msgs[0].content, "first");
        assert_eq!(msgs[1].content, "second");
    }

    #[test]
    fn recovery_sets_running_to_error() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("host.db");
        let store = Store::open(&db).unwrap();
        let host_id = new_id();
        store.host_insert_if_absent(&host_id, "h").unwrap();
        let ws = store.workspace_add("/p", "p").unwrap();
        let task = store.task_create("t", &ws.id).unwrap();
        let agent = store
            .agent_create(&task.id, &host_id, "cli.generic")
            .unwrap();
        store
            .agent_set_status(&agent.id, AgentStatus::Running)
            .unwrap();
        drop(store);

        let store2 = Store::open(&db).unwrap();
        assert_eq!(
            store2.agent_get(&agent.id).unwrap().unwrap().status,
            AgentStatus::Error
        );
    }

    #[test]
    fn fk_rejects_agent_without_task() {
        let (_tmp, store) = open_store();
        store.host_insert_if_absent(&new_id(), "h").unwrap();
        let host = store.host_get().unwrap();
        let conn = rusqlite::Connection::open(store.path()).unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        let err = conn.execute(
            "INSERT INTO agents (id, task_id, host_id, parent_id, interface, provider, status, run_location, created_at) \
             VALUES ('a1', 'missing-task', ?1, NULL, 'chat', 'cli.generic', 'idle', 'local', '2026-08-17T00:00:00Z')",
            [&host.id],
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.to_lowercase().contains("foreign key") || msg.contains("CONSTRAINT"),
            "expected FK error, got {msg}"
        );
    }

    #[test]
    fn schema_greater_than_current_refuses() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("host.db");
        let store = Store::open(&db).unwrap();
        {
            let conn = store.lock().unwrap();
            conn.execute(
                "UPDATE schema_meta SET value = '7' WHERE key = 'schema'",
                [],
            )
            .unwrap();
        }
        drop(store);
        let err = Store::open(&db).unwrap_err();
        assert_eq!(err.code(), "internal");
        match &err {
            StorageError::UnsupportedSchema(v) => assert_eq!(v, "7"),
            other => panic!("expected UnsupportedSchema, got {other:?}"),
        }
    }

    #[test]
    fn pragmas_and_no_files_table() {
        let (_tmp, store) = open_store();
        let conn = rusqlite::Connection::open(store.path()).unwrap();
        let journal: String = conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(journal.to_lowercase(), "wal");
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('files', 'harnesses')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0);
        assert!(store.quick_ok().unwrap());
    }

    #[test]
    fn provider_is_harness_id_distinct_from_interface_and_status() {
        let (_tmp, store) = open_store();
        let host_id = new_id();
        store.host_insert_if_absent(&host_id, "h").unwrap();
        let ws = store.workspace_add("/proj", "proj").unwrap();
        let task = store.task_create("t", &ws.id).unwrap();
        let agent = store
            .agent_create(&task.id, &host_id, HarnessId::cli_generic())
            .unwrap();
        assert_eq!(agent.provider, HarnessId::cli_generic());
        assert_eq!(agent.interface, "chat");
        assert_eq!(agent.status, AgentStatus::Idle);
        let v = serde_json::to_value(&agent).unwrap();
        assert_eq!(v["provider"], "cli.generic");
        assert_eq!(v["interface"], "chat");
        assert_eq!(v["status"], "idle");
    }

    #[test]
    fn archive_already_archived_is_noop() {
        let (_tmp, store) = open_store();
        store.host_insert_if_absent(&new_id(), "h").unwrap();
        let ws = store.workspace_add("/p", "p").unwrap();
        let task = store.task_create("t", &ws.id).unwrap();
        store.task_archive(&task.id).unwrap();
        let first = store.task_get(&task.id).unwrap().unwrap();
        store.task_archive(&task.id).unwrap();
        let second = store.task_get(&task.id).unwrap().unwrap();
        assert_eq!(first.updated_at, second.updated_at);
        assert_eq!(second.status, TaskStatus::Archived);
    }

    #[test]
    fn migration_0002_matches_contract() {
        assert!(MIGRATION_0002.contains("CREATE TABLE worktrees"));
        assert!(MIGRATION_0002.contains("CHECK (run_location IN ('local', 'worktree'))"));
        assert!(MIGRATION_0002.contains("CREATE INDEX idx_agents_task"));
        assert!(MIGRATION_0002.contains("CREATE INDEX idx_agents_status"));
        assert!(MIGRATION_0002
            .contains("INSERT OR REPLACE INTO schema_meta(key, value) VALUES ('schema', '2')"));
        assert!(!MIGRATION_0001.contains("CREATE TABLE worktrees"));
    }

    #[test]
    fn fresh_db_is_schema_six() {
        let (_tmp, store) = open_store();
        let conn = rusqlite::Connection::open(store.path()).unwrap();
        let schema: String = conn
            .query_row(
                "SELECT value FROM schema_meta WHERE key = 'schema'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(schema, "6");
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'artifacts'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'loops'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'worktrees'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'policies'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn migrate_schema_one_applies_worktrees() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("host.db");
        {
            let conn = rusqlite::Connection::open(&db).unwrap();
            conn.execute_batch(MIGRATION_0001).unwrap();
            let schema: String = conn
                .query_row(
                    "SELECT value FROM schema_meta WHERE key = 'schema'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(schema, "1");
            let n: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'worktrees'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 0);
        }
        let store = Store::open(&db).unwrap();
        let conn = rusqlite::Connection::open(store.path()).unwrap();
        let schema: String = conn
            .query_row(
                "SELECT value FROM schema_meta WHERE key = 'schema'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(schema, "6");
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'worktrees'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
        store.host_insert_if_absent(&new_id(), "h").unwrap();
        let ws = store.workspace_add("/p", "p").unwrap();
        let task = store.task_create("t", &ws.id).unwrap();
        let host = store.host_get().unwrap();
        let agent = store
            .agent_create(&task.id, &host.id, "cli.generic")
            .unwrap();
        store.agent_set_run_location(&agent.id, "worktree").unwrap();
        assert_eq!(
            store.agent_get(&agent.id).unwrap().unwrap().run_location,
            "worktree"
        );
    }

    #[test]
    fn worktree_crud_and_run_location() {
        let (_tmp, store) = open_store();
        let host_id = new_id();
        store.host_insert_if_absent(&host_id, "h").unwrap();
        let ws = store.workspace_add("/proj", "proj").unwrap();
        let task = store.task_create("t", &ws.id).unwrap();
        let agent = store
            .agent_create(&task.id, &host_id, "cli.generic")
            .unwrap();
        assert!(store.worktree_get_by_agent(&agent.id).unwrap().is_none());
        assert!(store.worktree_list(&ws.id).unwrap().is_empty());
        let wt = store
            .worktree_insert(&ws.id, &agent.id, "/wt/a", "rt/abcd1234")
            .unwrap();
        store.agent_set_run_location(&agent.id, "worktree").unwrap();
        assert_eq!(store.worktree_get(&wt.id).unwrap().unwrap().id, wt.id);
        let by_agent = store.worktree_get_by_agent(&agent.id).unwrap().unwrap();
        assert_eq!(by_agent.id, wt.id);
        assert_eq!(by_agent.path, "/wt/a");
        assert_eq!(by_agent.branch, "rt/abcd1234");
        assert_eq!(store.worktree_list(&ws.id).unwrap().len(), 1);
        assert_eq!(
            store.agent_get(&agent.id).unwrap().unwrap().run_location,
            "worktree"
        );
        let v = serde_json::to_value(&wt).unwrap();
        assert_eq!(v["workspaceId"], ws.id);
        assert_eq!(v["agentId"], agent.id);
        assert!(v.get("workspace_id").is_none());
    }
    #[test]
    fn error_codes_and_display() {
        assert_eq!(StorageError::NotFound.code(), "not_found");
        assert_eq!(
            StorageError::WorkspacePathInvalid("x".into()).code(),
            "workspace_path_invalid"
        );
        assert_eq!(
            StorageError::InvalidParams("bad".into()).code(),
            "invalid_params"
        );
        assert_eq!(
            StorageError::UnsupportedSchema("9".into()).code(),
            "internal"
        );
        let db = StorageError::Database(rusqlite::Error::InvalidQuery);
        assert_eq!(db.code(), "internal");
        assert!(db.to_string().contains("database"));
        let io = StorageError::Io(std::io::Error::other("boom"));
        assert_eq!(io.code(), "internal");
        assert!(StorageError::NotFound.to_string().contains("not found"));
    }

    #[test]
    fn status_role_parse_and_as_str() {
        assert_eq!(TaskStatus::Open.as_str(), "open");
        assert_eq!(TaskStatus::Archived.as_str(), "archived");
        assert_eq!(TaskStatus::parse("open").unwrap(), TaskStatus::Open);
        assert_eq!(TaskStatus::parse("archived").unwrap(), TaskStatus::Archived);
        let err = TaskStatus::parse("done").unwrap_err();
        assert_eq!(err.code(), "invalid_params");

        assert_eq!(AgentStatus::Idle.as_str(), "idle");
        assert_eq!(AgentStatus::Running.as_str(), "running");
        assert_eq!(AgentStatus::Error.as_str(), "error");
        assert_eq!(AgentStatus::parse("idle").unwrap(), AgentStatus::Idle);
        assert_eq!(AgentStatus::parse("running").unwrap(), AgentStatus::Running);
        assert_eq!(AgentStatus::parse("error").unwrap(), AgentStatus::Error);
        assert_eq!(
            AgentStatus::parse("zzz").unwrap_err().code(),
            "invalid_params"
        );

        assert_eq!(MessageRole::User.as_str(), "user");
        assert_eq!(MessageRole::Assistant.as_str(), "assistant");
        assert_eq!(MessageRole::System.as_str(), "system");
        assert_eq!(MessageRole::Tool.as_str(), "tool");
        assert_eq!(MessageRole::parse("user").unwrap(), MessageRole::User);
        assert_eq!(
            MessageRole::parse("assistant").unwrap(),
            MessageRole::Assistant
        );
        assert_eq!(MessageRole::parse("system").unwrap(), MessageRole::System);
        assert_eq!(MessageRole::parse("tool").unwrap(), MessageRole::Tool);
        assert_eq!(
            MessageRole::parse("narrator").unwrap_err().code(),
            "invalid_params"
        );
    }

    #[test]
    fn harness_id_traits() {
        let a = HarnessId::cli_generic();
        assert_eq!(a.as_str(), HarnessId::CLI_GENERIC);
        assert_eq!(a.as_ref(), "cli.generic");
        assert_eq!(a.to_string(), "cli.generic");
        assert_eq!(a, "cli.generic");
        assert_eq!(a, HarnessId::CLI_GENERIC);
        assert_eq!(
            HarnessId::from("cli.claude".to_string()).as_str(),
            "cli.claude"
        );
        assert_eq!(HarnessId::from("cli.codex"), HarnessId::new("cli.codex"));
        assert_eq!(format!("{a}"), "cli.generic");
    }

    #[test]
    fn host_get_not_found_and_store_debug() {
        let (_tmp, store) = open_store();
        let err = store.host_get().unwrap_err();
        assert_eq!(err.code(), "not_found");
        let dbg = format!("{store:?}");
        assert!(dbg.contains("Store"));
        assert!(dbg.contains("path"));
        assert!(store.path().ends_with("host.db"));
        assert!(store.integrity_ok().unwrap());
        assert_eq!(store.recover_running().unwrap(), 0);
    }

    #[test]
    fn task_list_filters_rename_touch_and_missing() {
        let (_tmp, store) = open_store();
        store.host_insert_if_absent(&new_id(), "h").unwrap();
        let ws = store.workspace_add("/p", "p").unwrap();
        assert!(store.workspace_get("nope").unwrap().is_none());
        assert_eq!(store.workspace_get(&ws.id).unwrap().unwrap().id, ws.id);

        let t1 = store.task_create("alpha", &ws.id).unwrap();
        let t2 = store.task_create("beta", &ws.id).unwrap();
        store.task_archive(&t2.id).unwrap();

        let open = store.task_list(TaskFilter::Open).unwrap();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].id, t1.id);
        let archived = store.task_list(TaskFilter::Archived).unwrap();
        assert_eq!(archived.len(), 1);
        assert_eq!(archived[0].id, t2.id);
        let all = store.task_list(TaskFilter::All).unwrap();
        assert_eq!(all.len(), 2);

        assert!(store.task_get("missing").unwrap().is_none());
        store.task_rename(&t1.id, "alpha-2").unwrap();
        assert_eq!(store.task_get(&t1.id).unwrap().unwrap().title, "alpha-2");
        assert_eq!(
            store.task_rename("missing", "x").unwrap_err().code(),
            "not_found"
        );
        store.task_touch(&t1.id).unwrap();
        assert_eq!(store.task_touch("missing").unwrap_err().code(), "not_found");
        assert_eq!(
            store.task_archive("missing").unwrap_err().code(),
            "not_found"
        );

        let counts = store.counts().unwrap();
        assert_eq!(counts.workspace_count, 1);
        assert_eq!(counts.task_count, 2);
        assert_eq!(counts.agent_count, 0);
    }

    #[test]
    fn agent_and_message_missing_edges() {
        let (_tmp, store) = open_store();
        let host_id = new_id();
        store.host_insert_if_absent(&host_id, "h").unwrap();
        let ws = store.workspace_add("/p", "p").unwrap();
        let task = store.task_create("t", &ws.id).unwrap();
        assert!(store.agent_list(&task.id).unwrap().is_empty());
        assert_eq!(
            store
                .agent_create("missing-task", &host_id, "cli.generic")
                .unwrap_err()
                .code(),
            "not_found"
        );
        let agent = store
            .agent_create(&task.id, &host_id, "cli.generic")
            .unwrap();
        assert!(store.agent_get("nope").unwrap().is_none());
        assert_eq!(
            store
                .agent_set_status("nope", AgentStatus::Error)
                .unwrap_err()
                .code(),
            "not_found"
        );
        store
            .agent_set_status(&agent.id, AgentStatus::Error)
            .unwrap();
        assert_eq!(
            store.agent_get(&agent.id).unwrap().unwrap().status,
            AgentStatus::Error
        );

        assert_eq!(
            store
                .message_append("nope", MessageRole::User, "x")
                .unwrap_err()
                .code(),
            "not_found"
        );
        assert!(store.last_message_at(&agent.id).unwrap().is_none());
        let sys = store
            .message_append(&agent.id, MessageRole::System, "sys")
            .unwrap();
        let tool = store
            .message_append(&agent.id, MessageRole::Tool, "tool")
            .unwrap();
        let last = store.last_message_at(&agent.id).unwrap().unwrap();
        assert_eq!(last, tool.created_at);
        let msgs = store.message_list(&agent.id).unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, MessageRole::System);
        assert_eq!(msgs[0].id, sys.id);
        assert_eq!(msgs[1].role, MessageRole::Tool);
    }

    #[test]
    fn worktree_insert_missing_and_bad_run_location() {
        let (_tmp, store) = open_store();
        let host_id = new_id();
        store.host_insert_if_absent(&host_id, "h").unwrap();
        let ws = store.workspace_add("/p", "p").unwrap();
        let task = store.task_create("t", &ws.id).unwrap();
        let agent = store
            .agent_create(&task.id, &host_id, "cli.generic")
            .unwrap();
        assert_eq!(
            store
                .worktree_insert("no-ws", &agent.id, "/wt", "rt/x")
                .unwrap_err()
                .code(),
            "not_found"
        );
        assert_eq!(
            store
                .worktree_insert(&ws.id, "no-agent", "/wt", "rt/x")
                .unwrap_err()
                .code(),
            "not_found"
        );
        assert_eq!(
            store
                .agent_set_run_location(&agent.id, "remote")
                .unwrap_err()
                .code(),
            "invalid_params"
        );
        assert_eq!(
            store
                .agent_set_run_location("nope", "local")
                .unwrap_err()
                .code(),
            "not_found"
        );
        store.agent_set_run_location(&agent.id, "local").unwrap();
    }

    #[test]
    fn migration_0003_matches_contract() {
        assert!(MIGRATION_0003.contains("CREATE TABLE policies"));
        assert!(MIGRATION_0003.contains("CHECK (mode IN ('ask', 'allow-always', 'deny'))"));
        assert!(MIGRATION_0003.contains("CHECK (scope IN ('agent', 'workspace'))"));
        assert!(MIGRATION_0003.contains("CHECK (yolo IN (0, 1))"));
        assert!(MIGRATION_0003.contains("WHERE agent_id IS NOT NULL"));
        assert!(MIGRATION_0003.contains("WHERE workspace_id IS NOT NULL"));
        assert!(MIGRATION_0003
            .contains("INSERT OR REPLACE INTO schema_meta(key, value) VALUES ('schema', '3')"));
        assert!(!MIGRATION_0003.contains("ON DELETE CASCADE"));
        assert!(!MIGRATION_0003.to_lowercase().contains("secret"));
        assert!(!MIGRATION_0001.contains("CREATE TABLE policies"));
        assert!(!MIGRATION_0002.contains("CREATE TABLE policies"));
    }

    #[test]
    fn migrate_schema_two_applies_policies() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("host.db");
        {
            let conn = rusqlite::Connection::open(&db).unwrap();
            conn.execute_batch(MIGRATION_0001).unwrap();
            conn.execute_batch(MIGRATION_0002).unwrap();
            let schema: String = conn
                .query_row(
                    "SELECT value FROM schema_meta WHERE key = 'schema'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(schema, "2");
            let n: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'policies'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 0);
        }
        let store = Store::open(&db).unwrap();
        let conn = rusqlite::Connection::open(store.path()).unwrap();
        let schema: String = conn
            .query_row(
                "SELECT value FROM schema_meta WHERE key = 'schema'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(schema, "6");
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'policies'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('shells', 'pty_sessions')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn policy_upsert_get_and_default_empty() {
        let (_tmp, store) = open_store();
        let host_id = new_id();
        store.host_insert_if_absent(&host_id, "h").unwrap();
        let ws = store.workspace_add("/p", "p").unwrap();
        let task = store.task_create("t", &ws.id).unwrap();
        let agent = store
            .agent_create(&task.id, &host_id, "cli.generic")
            .unwrap();

        assert!(store.policy_get_for_agent(&agent.id).unwrap().is_none());
        assert!(store.policy_get_for_workspace(&ws.id).unwrap().is_none());

        let row = store
            .policy_upsert(Some(&agent.id), None, "ask", "agent", false)
            .unwrap();
        assert_eq!(row.agent_id.as_deref(), Some(agent.id.as_str()));
        assert!(row.workspace_id.is_none());
        assert_eq!(row.mode, "ask");
        assert_eq!(row.scope, "agent");
        assert!(!row.yolo);
        let got = store.policy_get_for_agent(&agent.id).unwrap().unwrap();
        assert_eq!(got.id, row.id);
        assert!(!got.yolo);

        let again = store
            .policy_upsert(Some(&agent.id), None, "allow-always", "agent", true)
            .unwrap();
        assert_eq!(again.id, row.id);
        assert_eq!(again.mode, "allow-always");
        assert!(again.yolo);
        let n: i64 = {
            let conn = store.lock().unwrap();
            conn.query_row(
                "SELECT COUNT(*) FROM policies WHERE agent_id = ?1",
                [&agent.id],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(n, 1, "unique per agent");

        let ws_row = store
            .policy_upsert(None, Some(&ws.id), "deny", "workspace", false)
            .unwrap();
        assert_eq!(ws_row.workspace_id.as_deref(), Some(ws.id.as_str()));
        assert_eq!(ws_row.mode, "deny");
        let got_ws = store.policy_get_for_workspace(&ws.id).unwrap().unwrap();
        assert_eq!(got_ws.id, ws_row.id);
    }

    #[test]
    fn policy_any_yolo_empty_then_any_row() {
        let (_tmp, store) = open_store();
        let host_id = new_id();
        store.host_insert_if_absent(&host_id, "h").unwrap();
        let ws = store.workspace_add("/p", "p").unwrap();
        let task = store.task_create("t", &ws.id).unwrap();
        let agent = store
            .agent_create(&task.id, &host_id, "cli.generic")
            .unwrap();

        assert!(!store.policy_any_yolo().unwrap());
        store
            .policy_upsert(Some(&agent.id), None, "ask", "agent", false)
            .unwrap();
        assert!(!store.policy_any_yolo().unwrap());
        store
            .policy_upsert(Some(&agent.id), None, "ask", "agent", true)
            .unwrap();
        assert!(store.policy_any_yolo().unwrap());
        store
            .policy_upsert(None, Some(&ws.id), "ask", "workspace", true)
            .unwrap();
        assert!(store.policy_any_yolo().unwrap());
        store
            .policy_upsert(Some(&agent.id), None, "ask", "agent", false)
            .unwrap();
        assert!(store.policy_any_yolo().unwrap());
        store
            .policy_upsert(None, Some(&ws.id), "ask", "workspace", false)
            .unwrap();
        assert!(!store.policy_any_yolo().unwrap());
    }

    #[test]
    fn policy_upsert_rejects_xor_and_bad_mode() {
        let (_tmp, store) = open_store();
        assert_eq!(
            store
                .policy_upsert(None, None, "ask", "agent", false)
                .unwrap_err()
                .code(),
            "invalid_params"
        );
        assert_eq!(
            store
                .policy_upsert(Some("a"), Some("w"), "ask", "agent", false)
                .unwrap_err()
                .code(),
            "invalid_params"
        );
        assert_eq!(
            store
                .policy_upsert(Some("a"), None, "full-access", "agent", false)
                .unwrap_err()
                .code(),
            "invalid_params"
        );
        assert_eq!(
            store
                .policy_upsert(Some("a"), None, "ask", "global", false)
                .unwrap_err()
                .code(),
            "invalid_params"
        );
    }

    #[test]
    fn policy_check_xor_and_unique_per_agent() {
        let (_tmp, store) = open_store();
        let host_id = new_id();
        store.host_insert_if_absent(&host_id, "h").unwrap();
        let ws = store.workspace_add("/p", "p").unwrap();
        let task = store.task_create("t", &ws.id).unwrap();
        let agent = store
            .agent_create(&task.id, &host_id, "cli.generic")
            .unwrap();
        store
            .policy_upsert(Some(&agent.id), None, "ask", "agent", false)
            .unwrap();

        let conn = rusqlite::Connection::open(store.path()).unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        let both = conn.execute(
            "INSERT INTO policies (id, workspace_id, agent_id, mode, scope, yolo, updated_at)              VALUES ('p-both', ?1, ?2, 'ask', 'agent', 0, '2026-08-19T00:00:00Z')",
            [&ws.id, &agent.id],
        );
        assert!(both.is_err(), "CHECK xor both must fail");
        let neither = conn.execute(
            "INSERT INTO policies (id, workspace_id, agent_id, mode, scope, yolo, updated_at)              VALUES ('p-none', NULL, NULL, 'ask', 'agent', 0, '2026-08-19T00:00:00Z')",
            [],
        );
        assert!(neither.is_err(), "CHECK xor neither must fail");
        let dup = conn.execute(
            "INSERT INTO policies (id, workspace_id, agent_id, mode, scope, yolo, updated_at)              VALUES ('p-dup', NULL, ?1, 'deny', 'agent', 0, '2026-08-19T00:00:00Z')",
            [&agent.id],
        );
        assert!(dup.is_err(), "unique per agent must fail");
    }
    #[test]
    fn migration_0004_matches_contract() {
        assert!(MIGRATION_0004.contains("CHECK (interface IN ('chat', 'terminal'))"));
        assert!(MIGRATION_0004.contains("provider_session_id"));
        assert!(MIGRATION_0004.contains("interface != 'chat' OR provider_session_id IS NULL"));
        assert!(MIGRATION_0004
            .contains("INSERT OR REPLACE INTO schema_meta(key, value) VALUES ('schema', '4')"));
        assert!(!MIGRATION_0004.contains("CREATE TABLE shells"));
        assert!(!MIGRATION_0004.contains("CREATE TABLE pty_sessions"));
        assert!(!MIGRATION_0004.contains("ON DELETE CASCADE"));
        assert!(!MIGRATION_0001.contains("provider_session_id"));
        assert!(!MIGRATION_0002.contains("provider_session_id"));
        assert!(!MIGRATION_0003.contains("provider_session_id"));
    }

    #[test]
    fn migrate_schema_three_applies_terminal() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("host.db");
        {
            let conn = rusqlite::Connection::open(&db).unwrap();
            conn.execute_batch(MIGRATION_0001).unwrap();
            conn.execute_batch(MIGRATION_0002).unwrap();
            conn.execute_batch(MIGRATION_0003).unwrap();
            let schema: String = conn
                .query_row(
                    "SELECT value FROM schema_meta WHERE key = 'schema'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(schema, "3");
        }
        let store = Store::open(&db).unwrap();
        let conn = rusqlite::Connection::open(store.path()).unwrap();
        let schema: String = conn
            .query_row(
                "SELECT value FROM schema_meta WHERE key = 'schema'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(schema, "6");
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('shells', 'pty_sessions')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn chat_cannot_have_provider_session_id_terminal_can() {
        let (_tmp, store) = open_store();
        let host_id = new_id();
        store.host_insert_if_absent(&host_id, "h").unwrap();
        let ws = store.workspace_add("/p", "p").unwrap();
        let task = store.task_create("t", &ws.id).unwrap();
        let chat = store
            .agent_create(&task.id, &host_id, "cli.generic")
            .unwrap();
        assert_eq!(chat.interface, "chat");
        assert!(chat.provider_session_id.is_none());
        assert_eq!(
            store
                .agent_set_provider_session_id(&chat.id, "sess")
                .unwrap_err()
                .code(),
            "invalid_params"
        );

        let term = store
            .agent_create_interface(&task.id, &host_id, "cli.claude", "terminal", None)
            .unwrap();
        assert_eq!(term.interface, "terminal");
        assert!(term.provider_session_id.is_none());
        store
            .agent_set_provider_session_id(&term.id, "sess-1")
            .unwrap();
        let got = store.agent_get(&term.id).unwrap().unwrap();
        assert_eq!(got.provider_session_id.as_deref(), Some("sess-1"));

        let conn = rusqlite::Connection::open(store.path()).unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        let chat_with_sid = conn.execute(
            "INSERT INTO agents (id, task_id, host_id, parent_id, interface, provider, status, run_location, created_at, provider_session_id)              VALUES ('a-chat-sid', ?1, ?2, NULL, 'chat', 'cli.generic', 'idle', 'local', '2026-08-19T00:00:00Z', 'nope')",
            [&task.id, &host_id],
        );
        assert!(chat_with_sid.is_err(), "chat + session id must fail CHECK");
        let term_with_sid = conn.execute(
            "INSERT INTO agents (id, task_id, host_id, parent_id, interface, provider, status, run_location, created_at, provider_session_id)              VALUES ('a-term-sid', ?1, ?2, NULL, 'terminal', 'cli.claude', 'idle', 'local', '2026-08-19T00:00:00Z', 'ok')",
            [&task.id, &host_id],
        );
        assert!(term_with_sid.is_ok(), "terminal + session id must succeed");
        let bad_iface = conn.execute(
            "INSERT INTO agents (id, task_id, host_id, parent_id, interface, provider, status, run_location, created_at, provider_session_id)              VALUES ('a-bad', ?1, ?2, NULL, 'shell', 'cli.generic', 'idle', 'local', '2026-08-19T00:00:00Z', NULL)",
            [&task.id, &host_id],
        );
        assert!(bad_iface.is_err(), "interface=shell must fail CHECK");
    }

    #[test]
    fn migration_0005_matches_contract() {
        assert!(MIGRATION_0005.contains("CREATE TABLE artifacts"));
        assert!(MIGRATION_0005.contains("CREATE TABLE comment_threads"));
        assert!(MIGRATION_0005.contains("CREATE TABLE comments"));
        assert!(MIGRATION_0005.contains("source_message_id TEXT"));
        assert!(!MIGRATION_0005.contains("ON DELETE CASCADE"));
        assert!(!MIGRATION_0005.contains("REFERENCES messages"));
        assert!(MIGRATION_0005
            .contains("INSERT OR REPLACE INTO schema_meta(key, value) VALUES ('schema', '5')"));
        assert!(!MIGRATION_0005.contains("CREATE TABLE shells"));
        assert!(!MIGRATION_0005.contains("a2a."));
    }

    #[test]
    fn migrate_from_four_applies_0005() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("host.db");
        {
            let conn = rusqlite::Connection::open(&db).unwrap();
            conn.execute_batch(MIGRATION_0001).unwrap();
            conn.execute_batch(MIGRATION_0002).unwrap();
            conn.execute_batch(MIGRATION_0003).unwrap();
            conn.execute_batch(MIGRATION_0004).unwrap();
            let schema: String = conn
                .query_row(
                    "SELECT value FROM schema_meta WHERE key = 'schema'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(schema, "4");
        }
        let store = Store::open(&db).unwrap();
        let conn = rusqlite::Connection::open(store.path()).unwrap();
        let schema: String = conn
            .query_row(
                "SELECT value FROM schema_meta WHERE key = 'schema'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(schema, "6");
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'artifacts'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn artifact_kinds_cycle_and_clear_transcript() {
        let (_tmp, store) = open_store();
        let host_id = new_id();
        store.host_insert_if_absent(&host_id, "h").unwrap();
        let ws = store.workspace_add("/p", "p").unwrap();
        let task = store.task_create("t", &ws.id).unwrap();
        let other = store.task_create("o", &ws.id).unwrap();
        let agent = store
            .agent_create(&task.id, &host_id, "cli.generic")
            .unwrap();
        let msg = store
            .message_append(&agent.id, MessageRole::User, "hi")
            .unwrap();

        let spec = store
            .artifact_create(ArtifactCreateInput {
                task_id: &task.id,
                parent_id: None,
                kind: "spec",
                title: "Auth",
                body: "# Auth\n",
                assignee: None,
                source_message_id: None,
            })
            .unwrap();
        assert_eq!(spec.kind, "spec");
        assert!(spec.status.is_none());
        let ticket = store
            .artifact_create(ArtifactCreateInput {
                task_id: &task.id,
                parent_id: Some(&spec.id),
                kind: "ticket",
                title: "Add login",
                body: "",
                assignee: Some("alice"),
                source_message_id: Some(&msg.id),
            })
            .unwrap();
        assert_eq!(ticket.status.as_deref(), Some("todo"));
        assert_eq!(ticket.assignee.as_deref(), Some("alice"));
        assert_eq!(ticket.source_message_id.as_deref(), Some(msg.id.as_str()));

        let story = store
            .artifact_create(ArtifactCreateInput {
                task_id: &task.id,
                parent_id: None,
                kind: "story",
                title: "Story",
                body: "s",
                assignee: None,
                source_message_id: None,
            })
            .unwrap();
        assert_eq!(story.status.as_deref(), Some("todo"));
        let review = store
            .artifact_create(ArtifactCreateInput {
                task_id: &task.id,
                parent_id: None,
                kind: "review",
                title: "Rev",
                body: "r",
                assignee: None,
                source_message_id: None,
            })
            .unwrap();
        assert_eq!(review.kind, "review");

        let (items, truncated) = store.artifact_list(&task.id, None).unwrap();
        assert_eq!(items.len(), 4);
        assert!(!truncated);
        let (tickets, _) = store.artifact_list(&task.id, Some("ticket")).unwrap();
        assert_eq!(tickets.len(), 1);

        assert_eq!(
            store
                .artifact_create(ArtifactCreateInput {
                    task_id: &task.id,
                    parent_id: None,
                    kind: "spec",
                    title: "X",
                    body: "",
                    assignee: Some("bob"),
                    source_message_id: None
                })
                .unwrap_err()
                .code(),
            "invalid_params"
        );
        assert_eq!(
            store
                .artifact_update(&spec.id, None, None, Some("todo"), None, None)
                .unwrap_err()
                .code(),
            "invalid_params"
        );

        let updated = store
            .artifact_update(&ticket.id, None, None, Some("in_progress"), None, None)
            .unwrap();
        assert_eq!(updated.status.as_deref(), Some("in_progress"));

        assert_eq!(
            store
                .artifact_update(&ticket.id, None, None, None, None, Some(Some(&ticket.id)))
                .unwrap_err()
                .code(),
            "invalid_params"
        );
        assert_eq!(
            store
                .artifact_update(&spec.id, None, None, None, None, Some(Some(&ticket.id)))
                .unwrap_err()
                .code(),
            "invalid_params"
        );
        let foreign = store
            .artifact_create(ArtifactCreateInput {
                task_id: &other.id,
                parent_id: None,
                kind: "spec",
                title: "O",
                body: "",
                assignee: None,
                source_message_id: None,
            })
            .unwrap();
        assert_eq!(
            store
                .artifact_update(&ticket.id, None, None, None, None, Some(Some(&foreign.id)))
                .unwrap_err()
                .code(),
            "invalid_params"
        );

        let n = store.clear_transcript(&agent.id).unwrap();
        assert_eq!(n, 1);
        assert!(store.message_list(&agent.id).unwrap().is_empty());
        let after = store.artifact_get(&ticket.id).unwrap().unwrap();
        assert_eq!(after.body, "");
        assert!(after.source_message_id.is_none());
        assert_eq!(store.clear_transcript(&agent.id).unwrap(), 0);

        let deleted = store.artifact_delete_tree(&spec.id).unwrap();
        assert!(deleted.contains(&spec.id));
        assert!(deleted.contains(&ticket.id));
        assert!(store.artifact_get(&ticket.id).unwrap().is_none());
        assert!(store.agent_get(&agent.id).unwrap().is_some());
    }

    #[test]
    fn comment_thread_and_resolve() {
        let (_tmp, store) = open_store();
        store.host_insert_if_absent(&new_id(), "h").unwrap();
        let ws = store.workspace_add("/p", "p").unwrap();
        let task = store.task_create("t", &ws.id).unwrap();
        let art = store
            .artifact_create(ArtifactCreateInput {
                task_id: &task.id,
                parent_id: None,
                kind: "spec",
                title: "Auth",
                body: "hello world",
                assignee: None,
                source_message_id: None,
            })
            .unwrap();
        let th = store.comment_thread_create(&art.id, 0, 5, "nit").unwrap();
        assert_eq!(th.anchor_start, 0);
        assert_eq!(th.anchor_end, 5);
        assert!(!th.resolved);
        assert_eq!(th.comments.len(), 1);
        let th = store.comment_add(&th.id, "reply").unwrap();
        assert_eq!(th.comments.len(), 2);
        let th = store.comment_resolve(&th.id).unwrap();
        assert!(th.resolved);
        let again = store.comment_resolve(&th.id).unwrap();
        assert!(again.resolved);
        let listed = store.comment_list(&art.id).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(
            store
                .comment_thread_create(&art.id, 5, 5, "bad")
                .unwrap_err()
                .code(),
            "invalid_params"
        );
    }

    #[test]
    fn migration_0006_matches_contract() {
        assert!(MIGRATION_0006.contains("CREATE TABLE loops"));
        assert!(MIGRATION_0006.contains("CHECK (max_iterations BETWEEN 1 AND 32)"));
        assert!(MIGRATION_0006.contains("CHECK (budget_turns BETWEEN 1 AND 64)"));
        assert!(MIGRATION_0006.contains("CHECK (status IN ('running', 'stopped'))"));
        assert!(!MIGRATION_0006.contains("ON DELETE CASCADE"));
        assert!(!MIGRATION_0006.contains("CREATE TABLE artifacts"));
        assert!(MIGRATION_0006
            .contains("INSERT OR REPLACE INTO schema_meta(key, value) VALUES ('schema', '6')"));
        assert!(!MIGRATION_0001.contains("CREATE TABLE loops"));
        assert!(!MIGRATION_0005.contains("CREATE TABLE loops"));
    }

    #[test]
    fn migrate_from_five_applies_0006() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("host.db");
        {
            let conn = rusqlite::Connection::open(&db).unwrap();
            conn.execute_batch(MIGRATION_0001).unwrap();
            conn.execute_batch(MIGRATION_0002).unwrap();
            conn.execute_batch(MIGRATION_0003).unwrap();
            conn.execute_batch(MIGRATION_0004).unwrap();
            conn.execute_batch(MIGRATION_0005).unwrap();
            let schema: String = conn
                .query_row(
                    "SELECT value FROM schema_meta WHERE key = 'schema'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(schema, "5");
        }
        let store = Store::open(&db).unwrap();
        let conn = rusqlite::Connection::open(store.path()).unwrap();
        let schema: String = conn
            .query_row(
                "SELECT value FROM schema_meta WHERE key = 'schema'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(schema, "6");
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'loops'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn loop_checks_and_recover_running() {
        let (_tmp, store) = open_store();
        let host_id = new_id();
        store.host_insert_if_absent(&host_id, "h").unwrap();
        let ws = store.workspace_add("/p", "p").unwrap();
        let task = store.task_create("t", &ws.id).unwrap();
        let a = store
            .agent_create(&task.id, &host_id, "cli.generic")
            .unwrap();
        let b = store
            .agent_create(&task.id, &host_id, "cli.generic")
            .unwrap();
        let err = store
            .loop_insert(&task.id, &a.id, &b.id, 0, 2, "hi")
            .unwrap_err();
        assert_eq!(err.code(), "invalid_params");
        let err = store
            .loop_insert(&task.id, &a.id, &b.id, 33, 2, "hi")
            .unwrap_err();
        assert_eq!(err.code(), "invalid_params");
        let err = store
            .loop_insert(&task.id, &a.id, &b.id, 2, 0, "hi")
            .unwrap_err();
        assert_eq!(err.code(), "invalid_params");
        let err = store
            .loop_insert(&task.id, &a.id, &b.id, 2, 65, "hi")
            .unwrap_err();
        assert_eq!(err.code(), "invalid_params");
        let row = store
            .loop_insert(&task.id, &a.id, &b.id, 2, 4, "go")
            .unwrap();
        assert_eq!(row.status, "running");
        assert_eq!(row.iteration, 0);
        assert_eq!(row.turns, 0);
        store.loop_update_progress(&row.id, 1, 2).unwrap();
        let got = store.loop_get(&row.id).unwrap().unwrap();
        assert_eq!(got.iteration, 1);
        assert_eq!(got.turns, 2);
        let n = store.recover_running_loops_to_stopped().unwrap();
        assert_eq!(n, 1);
        let got = store.loop_get(&row.id).unwrap().unwrap();
        assert_eq!(got.status, "stopped");
        assert_eq!(got.reason.as_deref(), Some("error"));
        let again = store.loop_stop(&row.id, "stop").unwrap();
        assert_eq!(again.reason.as_deref(), Some("error"));

        let conn = rusqlite::Connection::open(store.path()).unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        let err = conn.execute(
            "INSERT INTO loops (id, task_id, agent_a, agent_b, max_iterations, budget_turns, iteration, turns, status, reason, prompt, created_at, updated_at)
             VALUES ('l-bad', ?1, ?2, ?3, 0, 2, 0, 0, 'running', NULL, 'x', 't', 't')",
            [&task.id, &a.id, &b.id],
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.to_lowercase().contains("check") || msg.contains("CONSTRAINT"),
            "expected CHECK error, got {msg}"
        );
    }

    #[test]
    fn agent_parent_same_task_and_cycle() {
        let (_tmp, store) = open_store();
        let host_id = new_id();
        store.host_insert_if_absent(&host_id, "h").unwrap();
        let ws = store.workspace_add("/p", "p").unwrap();
        let task = store.task_create("t", &ws.id).unwrap();
        let other = store.task_create("o", &ws.id).unwrap();
        let a = store
            .agent_create(&task.id, &host_id, "cli.generic")
            .unwrap();
        let b = store
            .agent_create_interface(&task.id, &host_id, "cli.generic", "chat", Some(&a.id))
            .unwrap();
        assert_eq!(b.parent_id.as_deref(), Some(a.id.as_str()));
        let err = store
            .agent_create_interface(&other.id, &host_id, "cli.generic", "chat", Some(&a.id))
            .unwrap_err();
        assert_eq!(err.code(), "invalid_params");
        let err = store.agent_set_parent(&a.id, Some(&a.id)).unwrap_err();
        assert_eq!(err.code(), "invalid_params");
        let err = store.agent_set_parent(&a.id, Some(&b.id)).unwrap_err();
        assert_eq!(err.code(), "invalid_params");
        store.agent_set_parent(&b.id, None).unwrap();
        assert!(store.agent_get(&b.id).unwrap().unwrap().parent_id.is_none());
        let c = store
            .agent_create_interface(&task.id, &host_id, "cli.generic", "chat", Some(&a.id))
            .unwrap();
        let n = store.agent_lift_children(&a.id).unwrap();
        assert_eq!(n, 1);
        assert!(store.agent_get(&c.id).unwrap().unwrap().parent_id.is_none());
    }
}
