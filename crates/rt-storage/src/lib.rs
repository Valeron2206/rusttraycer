//! Durable SQLite store for the RustTraycer host. One writer per process.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

const MIGRATION_0001: &str = include_str!("../migrations/0001_init.sql");
const MIGRATION_0002: &str = include_str!("../migrations/0002_worktrees.sql");
const MIGRATION_0003: &str = include_str!("../migrations/0003_policies.sql");

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
                Ok(())
            }
            Some("2") => {
                conn.execute_batch(MIGRATION_0003)?;
                Ok(())
            }
            Some("3") => Ok(()),
            Some(other) => Err(StorageError::UnsupportedSchema(other.to_string())),
            None => {
                conn.execute_batch(MIGRATION_0001)?;
                conn.execute_batch(MIGRATION_0002)?;
                conn.execute_batch(MIGRATION_0003)?;
                Ok(())
            }
        }
    }

    /// After migrate, before listen: running agents become error.
    pub fn recover(&self) -> Result<usize> {
        self.set_running_agents_to_error()
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
            "SELECT id, task_id, host_id, parent_id, interface, provider, status, run_location, created_at \
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
        let provider = provider.into();
        if self.task_get(task_id)?.is_none() {
            return Err(StorageError::NotFound);
        }
        let id = new_id();
        let created_at = now_rfc3339();
        {
            let conn = self.lock()?;
            conn.execute(
                "INSERT INTO agents (id, task_id, host_id, parent_id, interface, provider, status, run_location, created_at) \
                 VALUES (?1, ?2, ?3, NULL, 'chat', ?4, 'idle', 'local', ?5)",
                params![id, task_id, host_id, provider.as_str(), created_at],
            )?;
        }
        Ok(Agent {
            id,
            task_id: task_id.to_string(),
            host_id: host_id.to_string(),
            parent_id: None,
            interface: "chat".into(),
            provider,
            status: AgentStatus::Idle,
            run_location: "local".into(),
            created_at,
        })
    }

    pub fn agent_get(&self, id: &str) -> Result<Option<Agent>> {
        let conn = self.lock()?;
        let row = conn
            .query_row(
                "SELECT id, task_id, host_id, parent_id, interface, provider, status, run_location, created_at \
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
);

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
    ))
}

fn agent_from_tuple(t: AgentTuple) -> Result<Agent> {
    let (id, task_id, host_id, parent_id, interface, provider, status, run_location, created_at) =
        t;
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
                "UPDATE schema_meta SET value = '4' WHERE key = 'schema'",
                [],
            )
            .unwrap();
        }
        drop(store);
        let err = Store::open(&db).unwrap_err();
        assert_eq!(err.code(), "internal");
        match &err {
            StorageError::UnsupportedSchema(v) => assert_eq!(v, "4"),
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
    fn fresh_db_is_schema_three() {
        let (_tmp, store) = open_store();
        let conn = rusqlite::Connection::open(store.path()).unwrap();
        let schema: String = conn
            .query_row(
                "SELECT value FROM schema_meta WHERE key = 'schema'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(schema, "3");
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
        assert_eq!(schema, "3");
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
        assert_eq!(schema, "3");
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
}
