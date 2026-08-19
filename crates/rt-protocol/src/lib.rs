//! Wire contract for client ↔ host. camelCase on the wire. No storage, no HTTP.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CRATE_VERSION: &str = "2.0.0";
pub const HOST_METHOD_MAJOR: u32 = 1;
pub const HOST_METHOD_MINOR: u32 = 0;
pub const MAX_TITLE_SCALARS: usize = 200;
pub const MAX_CONTENT_BYTES: usize = 1_048_576;
pub const MAX_FILE_BYTES: u64 = 256 * 1024;
pub const BINARY_SCAN_BYTES: usize = 8 * 1024;
pub const SESSION_HEADER: &str = "X-Rt-Session";

pub mod error_codes {
    pub const NOT_FOUND: &str = "not_found";
    pub const INVALID_PARAMS: &str = "invalid_params";
    pub const AGENT_BUSY: &str = "agent_busy";
    pub const WORKSPACE_PATH_INVALID: &str = "workspace_path_invalid";
    pub const UNSUPPORTED_METHOD: &str = "unsupported_method";
    pub const VERSION_MISMATCH: &str = "version_mismatch";
    pub const UNAUTHORIZED: &str = "unauthorized";
    pub const INTERNAL: &str = "internal";
    pub const ALREADY_RUNNING: &str = "already_running";
    pub const FILE_TOO_LARGE: &str = "file_too_large";
    pub const FILE_BINARY: &str = "file_binary";
    pub const DENIED: &str = "denied";
    pub const APPROVAL_EXPIRED: &str = "approval_expired";
    pub const GIT_IDENTITY: &str = "git_identity";
    pub const GIT_AUTH: &str = "git_auth";
    pub const GIT_CONFLICT: &str = "git_conflict";
    pub const PATCH_FAILED: &str = "patch_failed";
    pub const NOT_PTY: &str = "not_pty";
    pub const PTY_DEAD: &str = "pty_dead";
    pub const CROSS_HOST: &str = "cross_host";
    pub const NO_INBOX: &str = "no_inbox";
    pub const LOOP_EXHAUSTED: &str = "loop_exhausted";
    pub const CONFLICT: &str = "conflict";
}

pub const METHOD_HANDSHAKE: &str = "handshake";
pub const METHOD_HOST_PING: &str = "host.ping";
pub const METHOD_HOST_DOCTOR: &str = "host.doctor";
pub const METHOD_WORKSPACE_LIST: &str = "workspace.list";
pub const METHOD_WORKSPACE_ADD: &str = "workspace.add";
pub const METHOD_TASK_LIST: &str = "task.list";
pub const METHOD_TASK_CREATE: &str = "task.create";
pub const METHOD_TASK_GET: &str = "task.get";
pub const METHOD_TASK_RENAME: &str = "task.rename";
pub const METHOD_TASK_ARCHIVE: &str = "task.archive";
pub const METHOD_AGENT_LIST: &str = "agent.list";
pub const METHOD_AGENT_CREATE: &str = "agent.create";
pub const METHOD_AGENT_GET: &str = "agent.get";
pub const METHOD_AGENT_SEND: &str = "agent.send";
pub const METHOD_AGENT_GET_CONTEXT: &str = "agent.get_context";
pub const METHOD_AGENT_CANCEL: &str = "agent.cancel";
pub const METHOD_FILES_TREE: &str = "files.tree";
pub const METHOD_FILES_READ: &str = "files.read";
pub const METHOD_WORKTREE_ENSURE: &str = "worktree.ensure";
pub const METHOD_WORKTREE_GET: &str = "worktree.get";
pub const METHOD_WORKTREE_LIST: &str = "worktree.list";
pub const METHOD_GIT_STATUS: &str = "git.status";
pub const METHOD_GIT_DIFF: &str = "git.diff";
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
pub const METHOD_SHELL_CREATE: &str = "shell.create";
pub const METHOD_SHELL_LIST: &str = "shell.list";
pub const METHOD_SHELL_CLOSE: &str = "shell.close";
pub const METHOD_PTY_OPEN: &str = "pty.open";
pub const METHOD_PTY_WRITE: &str = "pty.write";
pub const METHOD_PTY_RESIZE: &str = "pty.resize";
pub const METHOD_PTY_CLOSE: &str = "pty.close";
pub const METHOD_ARTIFACT_CREATE: &str = "artifact.create";
pub const METHOD_ARTIFACT_GET: &str = "artifact.get";
pub const METHOD_ARTIFACT_LIST: &str = "artifact.list";
pub const METHOD_ARTIFACT_UPDATE: &str = "artifact.update";
pub const METHOD_ARTIFACT_DELETE: &str = "artifact.delete";
pub const METHOD_ARTIFACT_EXPORT: &str = "artifact.export";
pub const METHOD_COMMENT_CREATE: &str = "comment.create";
pub const METHOD_COMMENT_LIST: &str = "comment.list";
pub const METHOD_COMMENT_RESOLVE: &str = "comment.resolve";
pub const METHOD_AGENT_CLEAR_TRANSCRIPT: &str = "agent.clear_transcript";
pub const METHOD_A2A_TRANSCRIPT: &str = "a2a.transcript";
pub const METHOD_A2A_DELIVER: &str = "a2a.deliver";
pub const METHOD_LOOP_START: &str = "loop.start";
pub const METHOD_LOOP_GET: &str = "loop.get";
pub const METHOD_LOOP_STOP: &str = "loop.stop";
pub const METHOD_AGENT_SWITCH: &str = "agent.switch";
pub const METHOD_PROFILE_CREATE: &str = "profile.create";
pub const METHOD_PROFILE_LIST: &str = "profile.list";
pub const METHOD_PROFILE_GET: &str = "profile.get";
pub const METHOD_PROFILE_UPDATE: &str = "profile.update";
pub const METHOD_PROFILE_DELETE: &str = "profile.delete";
pub const METHOD_PREFS_GET: &str = "prefs.get";
pub const METHOD_WORKSPACE_GUIDES_GET: &str = "workspace.guides.get";
pub const METHOD_SETTINGS_GUIDE_GET: &str = "settings.guide.get";
pub const METHOD_SETTINGS_GUIDE_SET: &str = "settings.guide.set";
pub const METHOD_PRESET_LIST: &str = "preset.list";
pub const METHOD_AGENT_UPDATE: &str = "agent.update";
pub const METHOD_SYNC_EXPORT: &str = "sync.export";
pub const METHOD_SYNC_IMPORT: &str = "sync.import";
pub const METHOD_SEARCH_QUERY: &str = "search.query";
pub const METHOD_WORKTREE_GC: &str = "worktree.gc";

pub const EXPORT_KIND: &str = "rusttraycer.export";
pub const EXPORT_VERSION: u32 = 1;
pub const MAX_EXPORT_TASKS: usize = 32;

/// Tradable methods (handshake itself is not included).
pub const TRADABLE_METHODS: &[&str] = &[
    METHOD_HOST_PING,
    METHOD_HOST_DOCTOR,
    METHOD_WORKSPACE_LIST,
    METHOD_WORKSPACE_ADD,
    METHOD_TASK_LIST,
    METHOD_TASK_CREATE,
    METHOD_TASK_GET,
    METHOD_TASK_RENAME,
    METHOD_TASK_ARCHIVE,
    METHOD_AGENT_LIST,
    METHOD_AGENT_CREATE,
    METHOD_AGENT_GET,
    METHOD_AGENT_SEND,
    METHOD_AGENT_GET_CONTEXT,
    METHOD_AGENT_CANCEL,
    METHOD_FILES_TREE,
    METHOD_FILES_READ,
    METHOD_WORKTREE_ENSURE,
    METHOD_WORKTREE_GET,
    METHOD_WORKTREE_LIST,
    METHOD_GIT_STATUS,
    METHOD_GIT_DIFF,
    METHOD_POLICY_GET,
    METHOD_POLICY_SET,
    METHOD_APPROVAL_RESPOND,
    METHOD_FILES_WRITE,
    METHOD_FILES_PATCH,
    METHOD_FILES_OPEN,
    METHOD_GIT_STAGE,
    METHOD_GIT_UNSTAGE,
    METHOD_GIT_RESTORE,
    METHOD_GIT_COMMIT,
    METHOD_GIT_PUSH,
    METHOD_SHELL_CREATE,
    METHOD_SHELL_LIST,
    METHOD_SHELL_CLOSE,
    METHOD_PTY_OPEN,
    METHOD_PTY_WRITE,
    METHOD_PTY_RESIZE,
    METHOD_PTY_CLOSE,
    METHOD_ARTIFACT_CREATE,
    METHOD_ARTIFACT_GET,
    METHOD_ARTIFACT_LIST,
    METHOD_ARTIFACT_UPDATE,
    METHOD_ARTIFACT_DELETE,
    METHOD_ARTIFACT_EXPORT,
    METHOD_COMMENT_CREATE,
    METHOD_COMMENT_LIST,
    METHOD_COMMENT_RESOLVE,
    METHOD_AGENT_CLEAR_TRANSCRIPT,
    METHOD_A2A_TRANSCRIPT,
    METHOD_A2A_DELIVER,
    METHOD_LOOP_START,
    METHOD_LOOP_GET,
    METHOD_LOOP_STOP,
    METHOD_AGENT_SWITCH,
    METHOD_PROFILE_CREATE,
    METHOD_PROFILE_LIST,
    METHOD_PROFILE_GET,
    METHOD_PROFILE_UPDATE,
    METHOD_PROFILE_DELETE,
    METHOD_PREFS_GET,
    METHOD_WORKSPACE_GUIDES_GET,
    METHOD_SETTINGS_GUIDE_GET,
    METHOD_SETTINGS_GUIDE_SET,
    METHOD_PRESET_LIST,
    METHOD_AGENT_UPDATE,
    METHOD_SYNC_EXPORT,
    METHOD_SYNC_IMPORT,
    METHOD_SEARCH_QUERY,
    METHOD_WORKTREE_GC,
];

pub fn host_method_version() -> MethodVersion {
    MethodVersion {
        major: HOST_METHOD_MAJOR,
        minor: HOST_METHOD_MINOR,
    }
}

/// Per-method negotiated version. Policy/approval methods are 1.1; write/git
/// mutate methods are 1.2; shell/pty methods are 1.3; artifact/comment/
/// `agent.clear_transcript` methods are 1.4; `agent.create` and A2A/loop
/// methods are 1.5; model-ux methods (`agent.switch`, `profile.*`, `prefs.get`)
/// are 1.6; workspace/guides/preset/`agent.update` methods are 1.7;
/// `sync.export`/`sync.import` are 1.8; `artifact.export`, `search.query`,
/// and `worktree.gc` are 1.9; all other tradable methods stay 1.0
/// (`HOST_METHOD_MINOR` is not bumped).
/// Unknown names return `None`.
pub fn method_version(name: &str) -> Option<MethodVersion> {
    if !TRADABLE_METHODS.iter().any(|m| *m == name) {
        return None;
    }
    match name {
        METHOD_POLICY_GET | METHOD_POLICY_SET | METHOD_APPROVAL_RESPOND => {
            Some(MethodVersion { major: 1, minor: 1 })
        }
        METHOD_FILES_WRITE | METHOD_FILES_PATCH | METHOD_FILES_OPEN | METHOD_GIT_STAGE
        | METHOD_GIT_UNSTAGE | METHOD_GIT_RESTORE | METHOD_GIT_COMMIT | METHOD_GIT_PUSH => {
            Some(MethodVersion { major: 1, minor: 2 })
        }
        METHOD_SHELL_CREATE | METHOD_SHELL_LIST | METHOD_SHELL_CLOSE | METHOD_PTY_OPEN
        | METHOD_PTY_WRITE | METHOD_PTY_RESIZE | METHOD_PTY_CLOSE => {
            Some(MethodVersion { major: 1, minor: 3 })
        }
        METHOD_ARTIFACT_CREATE
        | METHOD_ARTIFACT_GET
        | METHOD_ARTIFACT_LIST
        | METHOD_ARTIFACT_UPDATE
        | METHOD_ARTIFACT_DELETE
        | METHOD_COMMENT_CREATE
        | METHOD_COMMENT_LIST
        | METHOD_COMMENT_RESOLVE
        | METHOD_AGENT_CLEAR_TRANSCRIPT => Some(MethodVersion { major: 1, minor: 4 }),
        METHOD_ARTIFACT_EXPORT | METHOD_SEARCH_QUERY | METHOD_WORKTREE_GC => {
            Some(MethodVersion { major: 1, minor: 9 })
        }
        METHOD_AGENT_CREATE
        | METHOD_A2A_TRANSCRIPT
        | METHOD_A2A_DELIVER
        | METHOD_LOOP_START
        | METHOD_LOOP_GET
        | METHOD_LOOP_STOP => Some(MethodVersion { major: 1, minor: 5 }),
        METHOD_AGENT_SWITCH
        | METHOD_PROFILE_CREATE
        | METHOD_PROFILE_LIST
        | METHOD_PROFILE_GET
        | METHOD_PROFILE_UPDATE
        | METHOD_PROFILE_DELETE
        | METHOD_PREFS_GET => Some(MethodVersion { major: 1, minor: 6 }),
        METHOD_WORKSPACE_GUIDES_GET
        | METHOD_SETTINGS_GUIDE_GET
        | METHOD_SETTINGS_GUIDE_SET
        | METHOD_PRESET_LIST
        | METHOD_AGENT_UPDATE => Some(MethodVersion { major: 1, minor: 7 }),
        METHOD_SYNC_EXPORT | METHOD_SYNC_IMPORT => Some(MethodVersion { major: 1, minor: 8 }),
        _ => Some(host_method_version()),
    }
}

pub fn validate_title(title: &str) -> bool {
    let n = title.chars().count();
    (1..=MAX_TITLE_SCALARS).contains(&n)
}

fn empty_object() -> Value {
    Value::Object(serde_json::Map::new())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    pub id: String,
    pub method: String,
    #[serde(default = "empty_object")]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcOk {
    pub id: String,
    pub ok: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub id: String,
    pub error: ErrorBody,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodVersion {
    pub major: u32,
    pub minor: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectedMethod {
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientHello {
    pub client: String,
    pub client_version: String,
    #[serde(default)]
    pub methods: BTreeMap<String, MethodVersion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerHello {
    pub host_id: String,
    pub host_version: String,
    pub session_token: String,
    pub accepted: BTreeMap<String, MethodVersion>,
    pub rejected: BTreeMap<String, RejectedMethod>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub title: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub workspace_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    pub id: String,
    pub task_id: String,
    pub host_id: String,
    pub parent_id: Option<String>,
    pub interface: String,
    pub provider: String,
    pub status: String,
    pub run_location: String,
    pub created_at: String,
    #[serde(default)]
    pub provider_session_id: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
    #[serde(default)]
    pub fast: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub agent_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelOk {
    pub agent_id: String,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub size: Option<u64>,
    pub modified_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileTreeOk {
    pub items: Vec<FileEntry>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileReadOk {
    pub path: String,
    pub content: String,
    pub truncated: bool,
    pub encoding: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatusEntry {
    pub path: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatus {
    pub branch: String,
    pub dirty: bool,
    pub entries: Vec<GitStatusEntry>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitDiffFile {
    pub path: String,
    pub patch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitDiff {
    pub files: Vec<GitDiffFile>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PolicyMode {
    Ask,
    AllowAlways,
    Deny,
}

impl PolicyMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ask => "ask",
            Self::AllowAlways => "allow-always",
            Self::Deny => "deny",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "ask" => Some(Self::Ask),
            "allow-always" => Some(Self::AllowAlways),
            "deny" => Some(Self::Deny),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PolicyScope {
    Agent,
    Workspace,
}

impl PolicyScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Workspace => "workspace",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "agent" => Some(Self::Agent),
            "workspace" => Some(Self::Workspace),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PolicySource {
    Default,
    Agent,
    Workspace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyView {
    pub mode: PolicyMode,
    pub scope: PolicyScope,
    pub yolo: bool,
    pub source: PolicySource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyGetParams {
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub workspace_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicySetParams {
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub workspace_id: Option<String>,
    pub mode: PolicyMode,
    pub scope: PolicyScope,
    pub yolo: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApprovalDecision {
    AllowOnce,
    AllowAlways,
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalRespondParams {
    pub approval_id: String,
    pub decision: ApprovalDecision,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalRespondOk {
    pub applied: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilesWriteOk {
    pub path: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilesPatchOk {
    pub paths: Vec<String>,
    pub hunks: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilesOpenOk {
    pub opened: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitOk {
    pub commit: String,
    pub branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPushOk {
    pub remote: String,
    #[serde(rename = "ref")]
    pub ref_name: String,
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PtyOpenOk {
    pub pty_id: String,
    pub resumed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellCreateOk {
    pub shell_id: String,
    pub pty_id: String,
    pub cwd: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellListItem {
    pub shell_id: String,
    pub pty_id: String,
    pub cwd: String,
}

/// Wire `caps` object on `host.doctor.providers[]` (e1-canvas-v2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessCaps {
    pub one_shot: bool,
    pub long_lived: bool,
    pub stream_tokens: bool,
    pub tools: bool,
    pub session_resume: bool,
    pub a2a_inbox: bool,
    pub pty: bool,
    pub needs_api_key: bool,
    pub api_key_env: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactListOk {
    pub items: Vec<Artifact>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactDeleteOk {
    pub deleted: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactExportOk {
    pub format: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub markdown: String,
    /// PDF payload (raw bytes as a string when ASCII; starts with `%PDF`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub bytes: String,
    pub filename: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub id: String,
    pub body: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentListOk {
    pub threads: Vec<CommentThread>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearTranscriptOk {
    pub cleared: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactCreateParams {
    pub task_id: String,
    #[serde(default)]
    pub parent_id: Option<String>,
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub assignee: Option<String>,
    #[serde(default)]
    pub source_message_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactGetParams {
    pub artifact_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactListParams {
    pub task_id: String,
    #[serde(default)]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactDeleteParams {
    pub artifact_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactExportParams {
    pub artifact_id: String,
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentCreateParams {
    pub artifact_id: String,
    #[serde(default)]
    pub thread_id: Option<String>,
    #[serde(default)]
    pub anchor_start: Option<i64>,
    #[serde(default)]
    pub anchor_end: Option<i64>,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentListParams {
    pub artifact_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentResolveParams {
    pub thread_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearTranscriptParams {
    pub agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct A2aTranscriptParams {
    pub agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct A2aTranscriptOk {
    pub agent_id: String,
    pub interface: String,
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct A2aDeliverParams {
    pub from_agent_id: String,
    pub to_agent_id: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct A2aDeliverOk {
    pub message_id: String,
    pub to_agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopStartParams {
    pub task_id: String,
    pub agent_ids: Vec<String>,
    pub max_iterations: u32,
    #[serde(default)]
    pub budget_turns: Option<u32>,
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopStartOk {
    pub loop_id: String,
    pub iteration: u32,
    pub turns: u32,
    pub max_iterations: u32,
    pub budget_turns: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopView {
    pub loop_id: String,
    pub iteration: u32,
    pub turns: u32,
    pub max_iterations: u32,
    pub budget_turns: u32,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopGetParams {
    pub loop_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopStopParams {
    pub loop_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub provider: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
    #[serde(default)]
    pub fast: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileListOk {
    pub items: Vec<Profile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileCreateParams {
    pub name: String,
    pub provider: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
    #[serde(default)]
    pub fast: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileGetParams {
    pub profile_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileUpdateParams {
    pub profile_id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
    #[serde(default)]
    pub fast: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileDeleteParams {
    pub profile_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSwitchParams {
    pub agent_id: String,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
    #[serde(default)]
    pub fast: Option<bool>,
    #[serde(default)]
    pub profile_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefsGetOk {
    pub items: Vec<PrefsItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuideFile {
    pub path: String,
    pub content: String,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGuidesOk {
    pub agents_md: Option<GuideFile>,
    pub workspace_guide: Option<GuideFile>,
    pub global_guide: Option<GuideFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsGuide {
    pub path: String,
    pub content: String,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGuidesGetParams {
    pub workspace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsGuideSetParams {
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetItem {
    pub id: String,
    pub title: String,
    pub default_role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetListOk {
    pub items: Vec<PresetItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentUpdateParams {
    pub agent_id: String,
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportTask {
    pub id: String,
    pub title: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportAgent {
    pub id: String,
    pub task_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    pub interface: String,
    pub provider: String,
    pub status: String,
    pub run_location: String,
    pub created_at: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
    #[serde(default)]
    pub fast: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportCommentThread {
    pub id: String,
    pub artifact_id: String,
    pub anchor_start: i64,
    pub anchor_end: i64,
    pub resolved: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportComment {
    pub id: String,
    pub thread_id: String,
    pub body: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportArchive {
    pub kind: String,
    pub export_version: u32,
    pub source_host_id: String,
    pub exported_at: String,
    pub tasks: Vec<ExportTask>,
    pub agents: Vec<ExportAgent>,
    pub messages: Vec<Message>,
    pub artifacts: Vec<Artifact>,
    pub comment_threads: Vec<ExportCommentThread>,
    pub comments: Vec<ExportComment>,
    pub model_profiles: Vec<Profile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncExportParams {
    pub task_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncExportOk {
    pub archive: ExportArchive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncImportParams {
    pub workspace_id: String,
    pub archive: ExportArchive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncImportOk {
    pub tasks: u32,
    pub agents: u32,
    pub messages: u32,
    pub artifacts: u32,
    pub profiles_imported: u32,
    pub profiles_skipped: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SearchKind {
    Task,
    Workspace,
    Artifact,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQueryParams {
    pub q: String,
    #[serde(default)]
    pub kinds: Option<Vec<SearchKind>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchItem {
    pub kind: SearchKind,
    pub id: String,
    pub title: String,
    pub hint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQueryOk {
    pub items: Vec<SearchItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeGcParams {
    pub dry_run: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorktreeGcReason {
    Stale,
    Merged,
    Landed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeGcItem {
    pub worktree_id: String,
    pub path: String,
    pub reason: WorktreeGcReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeGcOk {
    pub dry_run: bool,
    pub items: Vec<WorktreeGcItem>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_camel_case() {
        let req = RpcRequest {
            id: "1".into(),
            method: "host.ping".into(),
            params: empty_object(),
        };
        let v = serde_json::to_value(&req).unwrap();
        assert_eq!(v["id"], "1");
        assert_eq!(v["method"], "host.ping");
        assert!(v["params"].is_object());
    }

    #[test]
    fn title_unicode_scalars() {
        assert!(!validate_title(""));
        assert!(validate_title("я"));
        assert!(validate_title(&"a".repeat(200)));
        assert!(!validate_title(&"a".repeat(201)));
        assert!(validate_title(&"й".repeat(200)));
        assert!(!validate_title(&"й".repeat(201)));
    }

    #[test]
    fn worktree_and_git_types_camel_case() {
        assert!(TRADABLE_METHODS.contains(&METHOD_WORKTREE_ENSURE));
        assert!(TRADABLE_METHODS.contains(&METHOD_WORKTREE_GET));
        assert!(TRADABLE_METHODS.contains(&METHOD_WORKTREE_LIST));
        assert!(TRADABLE_METHODS.contains(&METHOD_GIT_STATUS));
        assert!(TRADABLE_METHODS.contains(&METHOD_GIT_DIFF));
        assert!(TRADABLE_METHODS.contains(&METHOD_AGENT_CANCEL));
        let cancel = CancelOk {
            agent_id: "a1".into(),
            cancelled: true,
        };
        let cv = serde_json::to_value(&cancel).unwrap();
        assert_eq!(cv["agentId"], "a1");
        assert_eq!(cv["cancelled"], true);
        assert!(cv.get("agent_id").is_none());
        let back: CancelOk = serde_json::from_value(cv).unwrap();
        assert_eq!(back.agent_id, "a1");
        assert!(back.cancelled);

        let wt = Worktree {
            id: "w1".into(),
            workspace_id: "ws".into(),
            agent_id: "ag".into(),
            path: "/tmp/wt".into(),
            branch: "rt/abcd1234".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
        };
        let v = serde_json::to_value(&wt).unwrap();
        assert_eq!(v["workspaceId"], "ws");
        assert_eq!(v["agentId"], "ag");
        assert_eq!(v["createdAt"], "2026-01-01T00:00:00Z");
        assert!(v.get("workspace_id").is_none());
        let wt2: Worktree = serde_json::from_value(v).unwrap();
        assert_eq!(wt2.branch, "rt/abcd1234");

        let st = GitStatus {
            branch: "main".into(),
            dirty: true,
            entries: vec![GitStatusEntry {
                path: "src/lib.rs".into(),
                status: "modified".into(),
            }],
            truncated: false,
        };
        let v = serde_json::to_value(&st).unwrap();
        assert_eq!(v["branch"], "main");
        assert_eq!(v["dirty"], true);
        assert_eq!(v["entries"][0]["path"], "src/lib.rs");
        assert_eq!(v["entries"][0]["status"], "modified");
        assert_eq!(v["truncated"], false);

        let diff = GitDiff {
            files: vec![GitDiffFile {
                path: "src/lib.rs".into(),
                patch: Some("@@".into()),
            }],
            truncated: false,
        };
        let v = serde_json::to_value(&diff).unwrap();
        assert_eq!(v["files"][0]["path"], "src/lib.rs");
        assert_eq!(v["files"][0]["patch"], "@@");
        assert_eq!(v["truncated"], false);
        let bin = GitDiffFile {
            path: "a.bin".into(),
            patch: None,
        };
        let v = serde_json::to_value(&bin).unwrap();
        assert_eq!(v["path"], "a.bin");
        assert!(v["patch"].is_null());
    }
    #[test]
    fn host_method_version_is_1_0() {
        let v = host_method_version();
        assert_eq!(v, MethodVersion { major: 1, minor: 0 });
        assert_eq!(v.major, HOST_METHOD_MAJOR);
        assert_eq!(v.minor, HOST_METHOD_MINOR);
        assert_eq!(CRATE_VERSION, "2.0.0");
        assert_eq!(SESSION_HEADER, "X-Rt-Session");
        assert_eq!(MAX_CONTENT_BYTES, 1_048_576);
        assert_eq!(MAX_FILE_BYTES, 256 * 1024);
        assert_eq!(BINARY_SCAN_BYTES, 8 * 1024);
        assert_eq!(error_codes::NOT_FOUND, "not_found");
        assert_eq!(error_codes::INVALID_PARAMS, "invalid_params");
        assert_eq!(error_codes::AGENT_BUSY, "agent_busy");
        assert_eq!(error_codes::UNAUTHORIZED, "unauthorized");
        assert_eq!(error_codes::ALREADY_RUNNING, "already_running");
        assert_eq!(error_codes::FILE_TOO_LARGE, "file_too_large");
        assert_eq!(error_codes::FILE_BINARY, "file_binary");
        assert_eq!(error_codes::VERSION_MISMATCH, "version_mismatch");
        assert_eq!(error_codes::UNSUPPORTED_METHOD, "unsupported_method");
        assert_eq!(
            error_codes::WORKSPACE_PATH_INVALID,
            "workspace_path_invalid"
        );
        assert_eq!(error_codes::INTERNAL, "internal");
        assert_eq!(error_codes::DENIED, "denied");
        assert_eq!(error_codes::APPROVAL_EXPIRED, "approval_expired");
        assert_eq!(error_codes::GIT_IDENTITY, "git_identity");
        assert_eq!(error_codes::GIT_AUTH, "git_auth");
        assert_eq!(error_codes::GIT_CONFLICT, "git_conflict");
        assert_eq!(error_codes::PATCH_FAILED, "patch_failed");
        assert_eq!(error_codes::NOT_PTY, "not_pty");
        assert_eq!(error_codes::PTY_DEAD, "pty_dead");
        assert_eq!(error_codes::CROSS_HOST, "cross_host");
        assert_eq!(error_codes::NO_INBOX, "no_inbox");
        assert_eq!(error_codes::LOOP_EXHAUSTED, "loop_exhausted");
        assert_eq!(error_codes::CONFLICT, "conflict");
        assert_eq!(TRADABLE_METHODS.len(), 71);
        assert!(TRADABLE_METHODS.contains(&METHOD_POLICY_GET));
        assert!(TRADABLE_METHODS.contains(&METHOD_POLICY_SET));
        assert!(TRADABLE_METHODS.contains(&METHOD_APPROVAL_RESPOND));
        assert!(!TRADABLE_METHODS.contains(&METHOD_HANDSHAKE));
        assert_eq!(
            method_version(METHOD_POLICY_GET),
            Some(MethodVersion { major: 1, minor: 1 })
        );
        assert_eq!(
            method_version(METHOD_POLICY_SET),
            Some(MethodVersion { major: 1, minor: 1 })
        );
        assert_eq!(
            method_version(METHOD_APPROVAL_RESPOND),
            Some(MethodVersion { major: 1, minor: 1 })
        );
        assert_eq!(
            method_version(METHOD_HOST_PING),
            Some(MethodVersion { major: 1, minor: 0 })
        );
        assert_eq!(
            method_version(METHOD_ARTIFACT_CREATE),
            Some(MethodVersion { major: 1, minor: 4 })
        );
        assert_eq!(
            method_version(METHOD_ARTIFACT_EXPORT),
            Some(MethodVersion { major: 1, minor: 9 })
        );
        assert_eq!(
            method_version(METHOD_SEARCH_QUERY),
            Some(MethodVersion { major: 1, minor: 9 })
        );
        assert_eq!(
            method_version(METHOD_WORKTREE_GC),
            Some(MethodVersion { major: 1, minor: 9 })
        );
        assert!(TRADABLE_METHODS.contains(&METHOD_SEARCH_QUERY));
        assert!(TRADABLE_METHODS.contains(&METHOD_WORKTREE_GC));
        assert_eq!(
            method_version(METHOD_COMMENT_LIST),
            Some(MethodVersion { major: 1, minor: 4 })
        );
        assert_eq!(
            method_version(METHOD_AGENT_CLEAR_TRANSCRIPT),
            Some(MethodVersion { major: 1, minor: 4 })
        );
        assert_eq!(
            method_version(METHOD_FILES_WRITE),
            Some(MethodVersion { major: 1, minor: 2 })
        );
        assert_eq!(
            method_version(METHOD_FILES_PATCH),
            Some(MethodVersion { major: 1, minor: 2 })
        );
        assert_eq!(
            method_version(METHOD_FILES_OPEN),
            Some(MethodVersion { major: 1, minor: 2 })
        );
        assert_eq!(
            method_version(METHOD_GIT_STAGE),
            Some(MethodVersion { major: 1, minor: 2 })
        );
        assert_eq!(
            method_version(METHOD_GIT_UNSTAGE),
            Some(MethodVersion { major: 1, minor: 2 })
        );
        assert_eq!(
            method_version(METHOD_GIT_RESTORE),
            Some(MethodVersion { major: 1, minor: 2 })
        );
        assert_eq!(
            method_version(METHOD_GIT_COMMIT),
            Some(MethodVersion { major: 1, minor: 2 })
        );
        assert_eq!(
            method_version(METHOD_GIT_PUSH),
            Some(MethodVersion { major: 1, minor: 2 })
        );
        assert_eq!(
            method_version(METHOD_FILES_TREE),
            Some(MethodVersion { major: 1, minor: 0 })
        );
        assert_eq!(
            method_version(METHOD_AGENT_CREATE),
            Some(MethodVersion { major: 1, minor: 5 })
        );
        assert_eq!(
            method_version(METHOD_SHELL_CREATE),
            Some(MethodVersion { major: 1, minor: 3 })
        );
        assert_eq!(
            method_version(METHOD_SHELL_LIST),
            Some(MethodVersion { major: 1, minor: 3 })
        );
        assert_eq!(
            method_version(METHOD_SHELL_CLOSE),
            Some(MethodVersion { major: 1, minor: 3 })
        );
        assert_eq!(
            method_version(METHOD_PTY_OPEN),
            Some(MethodVersion { major: 1, minor: 3 })
        );
        assert_eq!(
            method_version(METHOD_PTY_WRITE),
            Some(MethodVersion { major: 1, minor: 3 })
        );
        assert_eq!(
            method_version(METHOD_PTY_RESIZE),
            Some(MethodVersion { major: 1, minor: 3 })
        );
        assert_eq!(
            method_version(METHOD_PTY_CLOSE),
            Some(MethodVersion { major: 1, minor: 3 })
        );
        assert!(TRADABLE_METHODS.contains(&METHOD_FILES_WRITE));
        assert!(TRADABLE_METHODS.contains(&METHOD_GIT_PUSH));
        assert!(TRADABLE_METHODS.contains(&METHOD_SHELL_CREATE));
        assert!(TRADABLE_METHODS.contains(&METHOD_PTY_OPEN));
        assert_eq!(method_version("handshake"), None);
        assert_eq!(method_version("no.such"), None);
    }

    #[test]
    fn policy_approval_and_caps_camel_case() {
        let view = PolicyView {
            mode: PolicyMode::Ask,
            scope: PolicyScope::Agent,
            yolo: false,
            source: PolicySource::Default,
        };
        let v = serde_json::to_value(&view).unwrap();
        assert_eq!(v["mode"], "ask");
        assert_eq!(v["scope"], "agent");
        assert_eq!(v["yolo"], false);
        assert_eq!(v["source"], "default");
        assert!(v.get("agent_id").is_none());
        let view2: PolicyView = serde_json::from_value(v).unwrap();
        assert_eq!(view2.mode, PolicyMode::Ask);
        assert_eq!(view2.source, PolicySource::Default);

        assert_eq!(
            serde_json::to_value(PolicyMode::AllowAlways).unwrap(),
            "allow-always"
        );
        assert_eq!(serde_json::to_value(PolicyMode::Deny).unwrap(), "deny");
        assert_eq!(PolicyMode::AllowAlways.as_str(), "allow-always");
        assert_eq!(
            PolicyMode::parse("allow-always"),
            Some(PolicyMode::AllowAlways)
        );
        assert_eq!(PolicyMode::parse("nope"), None);
        assert_eq!(PolicyScope::Agent.as_str(), "agent");
        assert_eq!(
            PolicyScope::parse("workspace"),
            Some(PolicyScope::Workspace)
        );
        assert_eq!(PolicyScope::parse("nope"), None);

        let get: PolicyGetParams = serde_json::from_str(r#"{"agentId":"a1"}"#).unwrap();
        assert_eq!(get.agent_id.as_deref(), Some("a1"));
        assert!(get.workspace_id.is_none());
        let get_ws: PolicyGetParams = serde_json::from_str(r#"{"workspaceId":"w1"}"#).unwrap();
        assert_eq!(get_ws.workspace_id.as_deref(), Some("w1"));

        let set: PolicySetParams = serde_json::from_str(
            r#"{"agentId":"a1","mode":"allow-always","scope":"agent","yolo":true}"#,
        )
        .unwrap();
        assert_eq!(set.agent_id.as_deref(), Some("a1"));
        assert_eq!(set.mode, PolicyMode::AllowAlways);
        assert_eq!(set.scope, PolicyScope::Agent);
        assert!(set.yolo);
        let sv = serde_json::to_value(&set).unwrap();
        assert_eq!(sv["agentId"], "a1");
        assert_eq!(sv["mode"], "allow-always");
        assert!(sv.get("agent_id").is_none());

        assert_eq!(
            serde_json::to_value(ApprovalDecision::AllowOnce).unwrap(),
            "allow-once"
        );
        assert_eq!(
            serde_json::to_value(ApprovalDecision::AllowAlways).unwrap(),
            "allow-always"
        );
        assert_eq!(
            serde_json::to_value(ApprovalDecision::Deny).unwrap(),
            "deny"
        );

        let resp: ApprovalRespondParams =
            serde_json::from_str(r#"{"approvalId":"x","decision":"deny"}"#).unwrap();
        assert_eq!(resp.approval_id, "x");
        assert_eq!(resp.decision, ApprovalDecision::Deny);
        let rv = serde_json::to_value(&resp).unwrap();
        assert_eq!(rv["approvalId"], "x");
        assert!(rv.get("approval_id").is_none());

        let ok = ApprovalRespondOk { applied: true };
        let ov = serde_json::to_value(&ok).unwrap();
        assert_eq!(ov["applied"], true);

        let caps = HarnessCaps {
            one_shot: true,
            long_lived: false,
            stream_tokens: true,
            tools: false,
            session_resume: false,
            a2a_inbox: false,
            pty: false,
            needs_api_key: false,
            api_key_env: None,
        };
        let cv = serde_json::to_value(&caps).unwrap();
        assert_eq!(cv["oneShot"], true);
        assert_eq!(cv["longLived"], false);
        assert_eq!(cv["streamTokens"], true);
        assert_eq!(cv["sessionResume"], false);
        assert_eq!(cv["a2aInbox"], false);
        assert_eq!(cv["needsApiKey"], false);
        assert!(cv["apiKeyEnv"].is_null());
        assert!(cv.get("one_shot").is_none());
        let caps2: HarnessCaps = serde_json::from_value(cv).unwrap();
        assert!(caps2.one_shot);
        assert!(caps2.api_key_env.is_none());
    }

    #[test]
    fn request_params_default_empty_object() {
        let req: RpcRequest = serde_json::from_str(r#"{"id":"9","method":"host.ping"}"#).unwrap();
        assert_eq!(req.id, "9");
        assert!(req.params.is_object());
        assert_eq!(req.params, empty_object());
    }

    #[test]
    fn rpc_ok_error_and_hello_camel_case() {
        let ok = RpcOk {
            id: "1".into(),
            ok: serde_json::json!({"pong": true}),
        };
        let v = serde_json::to_value(&ok).unwrap();
        assert_eq!(v["id"], "1");
        assert_eq!(v["ok"]["pong"], true);
        let ok2: RpcOk = serde_json::from_value(v).unwrap();
        assert_eq!(ok2.id, "1");

        let err = RpcError {
            id: "2".into(),
            error: ErrorBody {
                code: error_codes::NOT_FOUND.into(),
                message: "gone".into(),
            },
        };
        let v = serde_json::to_value(&err).unwrap();
        assert_eq!(v["error"]["code"], "not_found");
        assert_eq!(v["error"]["message"], "gone");
        let err2: RpcError = serde_json::from_value(v).unwrap();
        assert_eq!(err2.error.code, "not_found");

        let hello: ClientHello = serde_json::from_str(
            r#"{"client":"gui","clientVersion":"0.1.0","methods":{"host.ping":{"major":1,"minor":0}}}"#,
        )
        .unwrap();
        assert_eq!(hello.client, "gui");
        assert_eq!(hello.client_version, "0.1.0");
        assert_eq!(hello.methods["host.ping"].major, 1);

        let bare: ClientHello =
            serde_json::from_str(r#"{"client":"cli","clientVersion":"1.0.0"}"#).unwrap();
        assert!(bare.methods.is_empty());

        let mut accepted = BTreeMap::new();
        accepted.insert("host.ping".into(), MethodVersion { major: 1, minor: 0 });
        let mut rejected = BTreeMap::new();
        rejected.insert(
            "leftover.foo".into(),
            RejectedMethod {
                reason: "unsupported".into(),
            },
        );
        let sh = ServerHello {
            host_id: "h".into(),
            host_version: "0.1.0".into(),
            session_token: "tok".into(),
            accepted,
            rejected,
        };
        let v = serde_json::to_value(&sh).unwrap();
        assert_eq!(v["hostId"], "h");
        assert_eq!(v["hostVersion"], "0.1.0");
        assert_eq!(v["sessionToken"], "tok");
        assert_eq!(v["accepted"]["host.ping"]["major"], 1);
        assert_eq!(v["rejected"]["leftover.foo"]["reason"], "unsupported");
        let sh2: ServerHello = serde_json::from_value(v).unwrap();
        assert_eq!(sh2.session_token, "tok");
    }

    #[test]
    fn workspace_task_agent_message_file_types_camel_case() {
        let ws = Workspace {
            id: "w".into(),
            host_id: "h".into(),
            path: "/p".into(),
            name: "proj".into(),
            created_at: "t".into(),
        };
        let v = serde_json::to_value(&ws).unwrap();
        assert_eq!(v["hostId"], "h");
        assert_eq!(v["createdAt"], "t");
        let ws2: Workspace = serde_json::from_value(v).unwrap();
        assert_eq!(ws2.name, "proj");

        let task = Task {
            id: "t1".into(),
            title: "T".into(),
            status: "open".into(),
            created_at: "c".into(),
            updated_at: "u".into(),
            workspace_ids: vec!["w".into()],
            preset: None,
        };
        let v = serde_json::to_value(&task).unwrap();
        assert_eq!(v["workspaceIds"][0], "w");
        assert_eq!(v["createdAt"], "c");
        assert_eq!(v["updatedAt"], "u");
        let task2: Task = serde_json::from_value(v).unwrap();
        assert_eq!(task2.status, "open");

        let agent = Agent {
            id: "a".into(),
            task_id: "t1".into(),
            host_id: "h".into(),
            parent_id: None,
            interface: "chat".into(),
            provider: "cli.generic".into(),
            status: "idle".into(),
            run_location: "local".into(),
            created_at: "c".into(),
            provider_session_id: None,
            model: None,
            effort: None,
            fast: false,
            role: None,
        };
        let v = serde_json::to_value(&agent).unwrap();
        assert_eq!(v["taskId"], "t1");
        assert_eq!(v["hostId"], "h");
        assert_eq!(v["parentId"], serde_json::Value::Null);
        assert_eq!(v["runLocation"], "local");
        assert_eq!(v["createdAt"], "c");
        assert!(v["providerSessionId"].is_null());
        assert!(v.get("provider_session_id").is_none());
        let agent2: Agent = serde_json::from_value(v).unwrap();
        assert_eq!(agent2.provider, "cli.generic");
        assert!(agent2.provider_session_id.is_none());

        let msg = Message {
            id: "m".into(),
            agent_id: "a".into(),
            role: "user".into(),
            content: "hi".into(),
            created_at: "c".into(),
        };
        let v = serde_json::to_value(&msg).unwrap();
        assert_eq!(v["agentId"], "a");
        assert_eq!(v["createdAt"], "c");
        let msg2: Message = serde_json::from_value(v).unwrap();
        assert_eq!(msg2.content, "hi");

        let entry = FileEntry {
            name: "README.md".into(),
            path: "README.md".into(),
            kind: "file".into(),
            size: Some(4),
            modified_at: Some("c".into()),
        };
        let v = serde_json::to_value(&entry).unwrap();
        assert_eq!(v["modifiedAt"], "c");
        let tree = FileTreeOk {
            items: vec![entry],
            truncated: true,
        };
        let v = serde_json::to_value(&tree).unwrap();
        assert_eq!(v["truncated"], true);
        assert_eq!(v["items"][0]["name"], "README.md");
        let read = FileReadOk {
            path: "README.md".into(),
            content: "hi".into(),
            truncated: false,
            encoding: "utf8".into(),
        };
        let v = serde_json::to_value(&read).unwrap();
        assert_eq!(v["encoding"], "utf8");
        let read2: FileReadOk = serde_json::from_value(v).unwrap();
        assert_eq!(read2.content, "hi");
    }

    #[test]
    fn e3_write_types_and_codes_camel_case() {
        let w = FilesWriteOk {
            path: "src/lib.rs".into(),
            bytes: 12,
        };
        let v = serde_json::to_value(&w).unwrap();
        assert_eq!(v["path"], "src/lib.rs");
        assert_eq!(v["bytes"], 12);
        let w2: FilesWriteOk = serde_json::from_value(v).unwrap();
        assert_eq!(w2.path, "src/lib.rs");

        let p = FilesPatchOk {
            paths: vec!["a.rs".into(), "b.rs".into()],
            hunks: 3,
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["paths"][0], "a.rs");
        assert_eq!(v["hunks"], 3);
        assert!(v.get("file_count").is_none());

        let o = FilesOpenOk { opened: true };
        let v = serde_json::to_value(&o).unwrap();
        assert_eq!(v["opened"], true);

        let c = GitCommitOk {
            commit: "abc123".into(),
            branch: "main".into(),
        };
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!(v["commit"], "abc123");
        assert_eq!(v["branch"], "main");

        let push = GitPushOk {
            remote: "origin".into(),
            ref_name: "main".into(),
            ok: true,
        };
        let v = serde_json::to_value(&push).unwrap();
        assert_eq!(v["remote"], "origin");
        assert_eq!(v["ref"], "main");
        assert_eq!(v["ok"], true);
        assert!(v.get("ref_name").is_none());
        let push2: GitPushOk = serde_json::from_value(v).unwrap();
        assert_eq!(push2.ref_name, "main");

        assert_eq!(error_codes::GIT_IDENTITY, "git_identity");
        assert_eq!(error_codes::GIT_AUTH, "git_auth");
        assert_eq!(error_codes::GIT_CONFLICT, "git_conflict");
        assert_eq!(error_codes::PATCH_FAILED, "patch_failed");
    }

    #[test]
    fn e4_pty_shell_types_and_codes_camel_case() {
        let open = PtyOpenOk {
            pty_id: "p1".into(),
            resumed: true,
        };
        let v = serde_json::to_value(&open).unwrap();
        assert_eq!(v["ptyId"], "p1");
        assert_eq!(v["resumed"], true);
        assert!(v.get("pty_id").is_none());
        let open2: PtyOpenOk = serde_json::from_value(v).unwrap();
        assert_eq!(open2.pty_id, "p1");
        assert!(open2.resumed);

        let created = ShellCreateOk {
            shell_id: "s1".into(),
            pty_id: "p2".into(),
            cwd: "/ws".into(),
        };
        let v = serde_json::to_value(&created).unwrap();
        assert_eq!(v["shellId"], "s1");
        assert_eq!(v["ptyId"], "p2");
        assert_eq!(v["cwd"], "/ws");
        assert!(v.get("shell_id").is_none());
        let created2: ShellCreateOk = serde_json::from_value(v).unwrap();
        assert_eq!(created2.shell_id, "s1");

        let item = ShellListItem {
            shell_id: "s1".into(),
            pty_id: "p2".into(),
            cwd: "/ws".into(),
        };
        let v = serde_json::to_value(&item).unwrap();
        assert_eq!(v["shellId"], "s1");
        assert_eq!(v["ptyId"], "p2");

        let agent = Agent {
            id: "a".into(),
            task_id: "t1".into(),
            host_id: "h".into(),
            parent_id: None,
            interface: "terminal".into(),
            provider: "cli.claude".into(),
            status: "idle".into(),
            run_location: "local".into(),
            created_at: "c".into(),
            provider_session_id: Some("sess-1".into()),
            model: Some("opus".into()),
            effort: Some("high".into()),
            fast: true,
            role: None,
        };
        let v = serde_json::to_value(&agent).unwrap();
        assert_eq!(v["providerSessionId"], "sess-1");
        assert_eq!(v["interface"], "terminal");
        assert!(v.get("provider_session_id").is_none());

        assert_eq!(error_codes::NOT_PTY, "not_pty");
        assert_eq!(error_codes::PTY_DEAD, "pty_dead");
        assert_eq!(
            method_version(METHOD_AGENT_CREATE),
            Some(MethodVersion { major: 1, minor: 5 })
        );
        assert_eq!(
            method_version(METHOD_HOST_PING),
            Some(MethodVersion { major: 1, minor: 0 })
        );
        assert_eq!(
            method_version(METHOD_FILES_WRITE),
            Some(MethodVersion { major: 1, minor: 2 })
        );
        assert_eq!(
            method_version(METHOD_ARTIFACT_CREATE),
            Some(MethodVersion { major: 1, minor: 4 })
        );
        assert_eq!(
            method_version(METHOD_ARTIFACT_GET),
            Some(MethodVersion { major: 1, minor: 4 })
        );
        assert_eq!(
            method_version(METHOD_COMMENT_CREATE),
            Some(MethodVersion { major: 1, minor: 4 })
        );
        assert_eq!(
            method_version(METHOD_AGENT_CLEAR_TRANSCRIPT),
            Some(MethodVersion { major: 1, minor: 4 })
        );
        assert!(TRADABLE_METHODS.contains(&METHOD_ARTIFACT_CREATE));
        assert!(TRADABLE_METHODS.contains(&METHOD_AGENT_CLEAR_TRANSCRIPT));
    }

    #[test]
    fn e5_artifact_comment_types_camel_case() {
        let art = Artifact {
            id: "a1".into(),
            task_id: "t1".into(),
            parent_id: None,
            kind: "spec".into(),
            title: "Auth".into(),
            body: "# Auth\n".into(),
            status: None,
            assignee: None,
            source_message_id: None,
            created_at: "c".into(),
            updated_at: "u".into(),
        };
        let v = serde_json::to_value(&art).unwrap();
        assert_eq!(v["taskId"], "t1");
        assert_eq!(v["parentId"], serde_json::Value::Null);
        assert_eq!(v["sourceMessageId"], serde_json::Value::Null);
        assert_eq!(v["createdAt"], "c");
        assert_eq!(v["updatedAt"], "u");
        assert!(v.get("task_id").is_none());
        let art2: Artifact = serde_json::from_value(v).unwrap();
        assert_eq!(art2.title, "Auth");

        let list = ArtifactListOk {
            items: vec![art2],
            truncated: false,
        };
        let v = serde_json::to_value(&list).unwrap();
        assert_eq!(v["truncated"], false);
        assert_eq!(v["items"][0]["id"], "a1");

        let del = ArtifactDeleteOk {
            deleted: vec!["a1".into(), "a2".into()],
        };
        let v = serde_json::to_value(&del).unwrap();
        assert_eq!(v["deleted"][0], "a1");

        let exp = ArtifactExportOk {
            format: "md".into(),
            markdown: "Auth\n\n# Auth\n".into(),
            bytes: String::new(),
            filename: "a1.md".into(),
        };
        let v = serde_json::to_value(&exp).unwrap();
        assert_eq!(v["format"], "md");
        assert_eq!(v["filename"], "a1.md");
        assert!(v.get("file_name").is_none());

        let c = Comment {
            id: "c1".into(),
            body: "nit".into(),
            created_at: "c".into(),
        };
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!(v["createdAt"], "c");
        assert!(v.get("created_at").is_none());

        let th = CommentThread {
            id: "th1".into(),
            artifact_id: "a1".into(),
            anchor_start: 0,
            anchor_end: 12,
            resolved: false,
            comments: vec![c],
            created_at: "c".into(),
            updated_at: "u".into(),
        };
        let v = serde_json::to_value(&th).unwrap();
        assert_eq!(v["artifactId"], "a1");
        assert_eq!(v["anchorStart"], 0);
        assert_eq!(v["anchorEnd"], 12);
        assert_eq!(v["resolved"], false);
        assert_eq!(v["comments"][0]["body"], "nit");
        assert!(v.get("artifact_id").is_none());

        let cleared = ClearTranscriptOk { cleared: 12 };
        let v = serde_json::to_value(&cleared).unwrap();
        assert_eq!(v["cleared"], 12);

        let create: ArtifactCreateParams =
            serde_json::from_str(r#"{"taskId":"t1","kind":"ticket","title":"Add login"}"#).unwrap();
        assert_eq!(create.task_id, "t1");
        assert_eq!(create.kind, "ticket");
        assert!(create.body.is_empty());
        assert!(create.parent_id.is_none());
    }

    #[test]
    fn e6_a2a_loop_types_camel_case() {
        assert_eq!(
            method_version(METHOD_AGENT_CREATE),
            Some(MethodVersion { major: 1, minor: 5 })
        );
        assert_eq!(
            method_version(METHOD_A2A_TRANSCRIPT),
            Some(MethodVersion { major: 1, minor: 5 })
        );
        assert_eq!(
            method_version(METHOD_A2A_DELIVER),
            Some(MethodVersion { major: 1, minor: 5 })
        );
        assert_eq!(
            method_version(METHOD_LOOP_START),
            Some(MethodVersion { major: 1, minor: 5 })
        );
        assert_eq!(
            method_version(METHOD_LOOP_GET),
            Some(MethodVersion { major: 1, minor: 5 })
        );
        assert_eq!(
            method_version(METHOD_LOOP_STOP),
            Some(MethodVersion { major: 1, minor: 5 })
        );
        assert_eq!(
            method_version(METHOD_AGENT_SWITCH),
            Some(MethodVersion { major: 1, minor: 6 })
        );
        assert_eq!(
            method_version(METHOD_PROFILE_CREATE),
            Some(MethodVersion { major: 1, minor: 6 })
        );
        assert_eq!(
            method_version(METHOD_PREFS_GET),
            Some(MethodVersion { major: 1, minor: 6 })
        );
        assert_eq!(
            method_version(METHOD_FILES_WRITE),
            Some(MethodVersion { major: 1, minor: 2 })
        );
        assert!(TRADABLE_METHODS.contains(&METHOD_A2A_DELIVER));
        assert!(TRADABLE_METHODS.contains(&METHOD_LOOP_START));

        let tr = A2aTranscriptOk {
            agent_id: "a1".into(),
            interface: "chat".into(),
            messages: vec![],
        };
        let v = serde_json::to_value(&tr).unwrap();
        assert_eq!(v["agentId"], "a1");
        assert!(v.get("agent_id").is_none());

        let del = A2aDeliverOk {
            message_id: "m1".into(),
            to_agent_id: "a2".into(),
        };
        let v = serde_json::to_value(&del).unwrap();
        assert_eq!(v["messageId"], "m1");
        assert_eq!(v["toAgentId"], "a2");

        let start: LoopStartParams = serde_json::from_str(
            r#"{"taskId":"t1","agentIds":["a","b"],"maxIterations":2,"prompt":"hi"}"#,
        )
        .unwrap();
        assert_eq!(start.task_id, "t1");
        assert_eq!(start.agent_ids, ["a", "b"]);
        assert_eq!(start.max_iterations, 2);
        assert!(start.budget_turns.is_none());

        let ok = LoopStartOk {
            loop_id: "l1".into(),
            iteration: 0,
            turns: 0,
            max_iterations: 2,
            budget_turns: 4,
        };
        let v = serde_json::to_value(&ok).unwrap();
        assert_eq!(v["loopId"], "l1");
        assert_eq!(v["maxIterations"], 2);
        assert_eq!(v["budgetTurns"], 4);

        let view = LoopView {
            loop_id: "l1".into(),
            iteration: 1,
            turns: 2,
            max_iterations: 2,
            budget_turns: 4,
            status: "stopped".into(),
            reason: Some("max_iterations".into()),
        };
        let v = serde_json::to_value(&view).unwrap();
        assert_eq!(v["status"], "stopped");
        assert_eq!(v["reason"], "max_iterations");
        assert!(v.get("loop_id").is_none());
    }

    #[test]
    fn e7_model_ux_types_and_versions_camel_case() {
        assert_eq!(
            method_version(METHOD_AGENT_SWITCH),
            Some(MethodVersion { major: 1, minor: 6 })
        );
        assert_eq!(
            method_version(METHOD_PROFILE_LIST),
            Some(MethodVersion { major: 1, minor: 6 })
        );
        assert_eq!(
            method_version(METHOD_PROFILE_GET),
            Some(MethodVersion { major: 1, minor: 6 })
        );
        assert_eq!(
            method_version(METHOD_PROFILE_UPDATE),
            Some(MethodVersion { major: 1, minor: 6 })
        );
        assert_eq!(
            method_version(METHOD_PROFILE_DELETE),
            Some(MethodVersion { major: 1, minor: 6 })
        );
        assert_eq!(
            method_version(METHOD_PREFS_GET),
            Some(MethodVersion { major: 1, minor: 6 })
        );
        assert_eq!(
            method_version(METHOD_AGENT_CREATE),
            Some(MethodVersion { major: 1, minor: 5 })
        );
        assert_eq!(
            method_version(METHOD_HOST_PING),
            Some(MethodVersion { major: 1, minor: 0 })
        );
        assert!(TRADABLE_METHODS.contains(&METHOD_AGENT_SWITCH));
        assert!(TRADABLE_METHODS.contains(&METHOD_PREFS_GET));
        assert_eq!(TRADABLE_METHODS.len(), 71);

        let sw: AgentSwitchParams = serde_json::from_str(
            r#"{"agentId":"a1","provider":"cli.codex","model":"o3","effort":"high","fast":false}"#,
        )
        .unwrap();
        assert_eq!(sw.agent_id, "a1");
        assert_eq!(sw.provider.as_deref(), Some("cli.codex"));
        assert_eq!(sw.model.as_deref(), Some("o3"));
        assert_eq!(sw.effort.as_deref(), Some("high"));
        assert_eq!(sw.fast, Some(false));
        assert!(sw.profile_id.is_none());
        let sv = serde_json::to_value(&sw).unwrap();
        assert_eq!(sv["agentId"], "a1");
        assert!(sv.get("agent_id").is_none());

        let profile = Profile {
            id: "p1".into(),
            name: "fast-opus".into(),
            provider: "cli.claude".into(),
            model: Some("opus".into()),
            effort: Some("high".into()),
            fast: true,
            created_at: "c".into(),
            updated_at: "u".into(),
        };
        let v = serde_json::to_value(&profile).unwrap();
        assert_eq!(v["createdAt"], "c");
        assert_eq!(v["updatedAt"], "u");
        assert_eq!(v["fast"], true);
        assert!(v.get("created_at").is_none());

        let prefs = PrefsGetOk {
            items: vec![PrefsItem {
                provider: "cli.generic".into(),
                model: None,
                effort: None,
                fast: false,
            }],
        };
        let v = serde_json::to_value(&prefs).unwrap();
        assert_eq!(v["items"][0]["provider"], "cli.generic");
        assert!(v["items"][0]["model"].is_null());

        let agent = Agent {
            id: "a".into(),
            task_id: "t1".into(),
            host_id: "h".into(),
            parent_id: None,
            interface: "chat".into(),
            provider: "cli.generic".into(),
            status: "idle".into(),
            run_location: "local".into(),
            created_at: "c".into(),
            provider_session_id: None,
            model: Some("gpt".into()),
            effort: Some("low".into()),
            fast: true,
            role: None,
        };
        let v = serde_json::to_value(&agent).unwrap();
        assert_eq!(v["model"], "gpt");
        assert_eq!(v["effort"], "low");
        assert_eq!(v["fast"], true);
        let agent2: Agent = serde_json::from_value(v).unwrap();
        assert_eq!(agent2.model.as_deref(), Some("gpt"));
        assert!(agent2.fast);
    }

    #[test]
    fn e8_workspace_types_and_versions_camel_case() {
        assert_eq!(
            method_version(METHOD_WORKSPACE_GUIDES_GET),
            Some(MethodVersion { major: 1, minor: 7 })
        );
        assert_eq!(
            method_version(METHOD_SETTINGS_GUIDE_GET),
            Some(MethodVersion { major: 1, minor: 7 })
        );
        assert_eq!(
            method_version(METHOD_SETTINGS_GUIDE_SET),
            Some(MethodVersion { major: 1, minor: 7 })
        );
        assert_eq!(
            method_version(METHOD_PRESET_LIST),
            Some(MethodVersion { major: 1, minor: 7 })
        );
        assert_eq!(
            method_version(METHOD_AGENT_UPDATE),
            Some(MethodVersion { major: 1, minor: 7 })
        );
        assert_eq!(
            method_version(METHOD_AGENT_SWITCH),
            Some(MethodVersion { major: 1, minor: 6 })
        );
        assert_eq!(
            method_version(METHOD_AGENT_CREATE),
            Some(MethodVersion { major: 1, minor: 5 })
        );
        assert_eq!(
            method_version(METHOD_TASK_CREATE),
            Some(MethodVersion { major: 1, minor: 0 })
        );
        assert_eq!(
            method_version(METHOD_HOST_PING),
            Some(MethodVersion { major: 1, minor: 0 })
        );
        assert_eq!(TRADABLE_METHODS.len(), 71);
        assert!(TRADABLE_METHODS.contains(&METHOD_WORKSPACE_GUIDES_GET));
        assert!(TRADABLE_METHODS.contains(&METHOD_AGENT_UPDATE));
        assert!(!TRADABLE_METHODS
            .iter()
            .any(|m| m.to_ascii_lowercase().contains("phase")));
        assert!(!TRADABLE_METHODS
            .iter()
            .any(|m| m.to_ascii_lowercase().contains("epic")));

        let gf = GuideFile {
            path: "/ws/AGENTS.md".into(),
            content: "hi".into(),
            truncated: false,
        };
        let v = serde_json::to_value(&gf).unwrap();
        assert_eq!(v["path"], "/ws/AGENTS.md");
        assert_eq!(v["content"], "hi");
        assert_eq!(v["truncated"], false);

        let ok = WorkspaceGuidesOk {
            agents_md: Some(gf),
            workspace_guide: None,
            global_guide: None,
        };
        let v = serde_json::to_value(&ok).unwrap();
        assert_eq!(v["agentsMd"]["path"], "/ws/AGENTS.md");
        assert!(v["workspaceGuide"].is_null());
        assert!(v["globalGuide"].is_null());
        assert!(v.get("agents_md").is_none());

        let sg = SettingsGuide {
            path: "/data/agent-selection-guide.md".into(),
            content: "".into(),
            truncated: false,
        };
        let v = serde_json::to_value(&sg).unwrap();
        assert_eq!(v["path"], "/data/agent-selection-guide.md");
        assert_eq!(v["content"], "");

        let item = PresetItem {
            id: "planning".into(),
            title: "Planning".into(),
            default_role: "planner".into(),
        };
        let v = serde_json::to_value(&item).unwrap();
        assert_eq!(v["id"], "planning");
        assert_eq!(v["title"], "Planning");
        assert_eq!(v["defaultRole"], "planner");
        assert!(v.get("default_role").is_none());

        let list = PresetListOk { items: vec![item] };
        let v = serde_json::to_value(&list).unwrap();
        assert_eq!(v["items"][0]["defaultRole"], "planner");

        let upd: AgentUpdateParams =
            serde_json::from_str(r#"{"agentId":"a1","role":"reviewer"}"#).unwrap();
        assert_eq!(upd.agent_id, "a1");
        assert_eq!(upd.role, "reviewer");
        let uv = serde_json::to_value(&upd).unwrap();
        assert_eq!(uv["agentId"], "a1");
        assert!(uv.get("agent_id").is_none());

        let get: WorkspaceGuidesGetParams =
            serde_json::from_str(r#"{"workspaceId":"w1"}"#).unwrap();
        assert_eq!(get.workspace_id, "w1");

        let set: SettingsGuideSetParams = serde_json::from_str(r#"{"content":"x"}"#).unwrap();
        assert_eq!(set.content, "x");

        let task = Task {
            id: "t1".into(),
            title: "T".into(),
            status: "open".into(),
            created_at: "c".into(),
            updated_at: "u".into(),
            workspace_ids: vec!["w".into()],
            preset: Some("planning".into()),
        };
        let v = serde_json::to_value(&task).unwrap();
        assert_eq!(v["preset"], "planning");
        assert_eq!(v["workspaceIds"][0], "w");

        let agent = Agent {
            id: "a".into(),
            task_id: "t1".into(),
            host_id: "h".into(),
            parent_id: None,
            interface: "chat".into(),
            provider: "cli.generic".into(),
            status: "idle".into(),
            run_location: "local".into(),
            created_at: "c".into(),
            provider_session_id: None,
            model: None,
            effort: None,
            fast: false,
            role: Some("planner".into()),
        };
        let v = serde_json::to_value(&agent).unwrap();
        assert_eq!(v["role"], "planner");
        assert_eq!(v["taskId"], "t1");
    }

    #[test]
    fn e9_sync_types_and_versions_camel_case() {
        assert_eq!(
            method_version(METHOD_ARTIFACT_EXPORT),
            Some(MethodVersion { major: 1, minor: 9 })
        );
        assert_eq!(
            method_version(METHOD_SYNC_EXPORT),
            Some(MethodVersion { major: 1, minor: 8 })
        );
        assert_eq!(
            method_version(METHOD_SYNC_IMPORT),
            Some(MethodVersion { major: 1, minor: 8 })
        );
        assert_eq!(
            method_version(METHOD_WORKSPACE_GUIDES_GET),
            Some(MethodVersion { major: 1, minor: 7 })
        );
        assert_eq!(
            method_version(METHOD_AGENT_SWITCH),
            Some(MethodVersion { major: 1, minor: 6 })
        );
        assert_eq!(
            method_version(METHOD_AGENT_CREATE),
            Some(MethodVersion { major: 1, minor: 5 })
        );
        assert_eq!(
            method_version(METHOD_HOST_PING),
            Some(MethodVersion { major: 1, minor: 0 })
        );
        assert_eq!(host_method_version(), MethodVersion { major: 1, minor: 0 });
        assert_eq!(TRADABLE_METHODS.len(), 71);
        assert!(TRADABLE_METHODS.contains(&METHOD_SYNC_EXPORT));
        assert!(TRADABLE_METHODS.contains(&METHOD_SYNC_IMPORT));
        assert_eq!(EXPORT_KIND, "rusttraycer.export");
        assert_eq!(EXPORT_VERSION, 1);
        assert_eq!(MAX_EXPORT_TASKS, 32);
        assert_eq!(error_codes::CONFLICT, "conflict");

        let archive = ExportArchive {
            kind: EXPORT_KIND.into(),
            export_version: EXPORT_VERSION,
            source_host_id: "h1".into(),
            exported_at: "2026-08-19T12:00:00Z".into(),
            tasks: vec![ExportTask {
                id: "t1".into(),
                title: "T".into(),
                status: "open".into(),
                created_at: "c".into(),
                updated_at: "u".into(),
                preset: Some("planning".into()),
            }],
            agents: vec![ExportAgent {
                id: "a1".into(),
                task_id: "t1".into(),
                parent_id: None,
                interface: "chat".into(),
                provider: "cli.generic".into(),
                status: "idle".into(),
                run_location: "local".into(),
                created_at: "c".into(),
                model: Some("gpt".into()),
                effort: Some("low".into()),
                fast: true,
                role: Some("coder".into()),
            }],
            messages: vec![],
            artifacts: vec![],
            comment_threads: vec![],
            comments: vec![],
            model_profiles: vec![],
        };
        let v = serde_json::to_value(&archive).unwrap();
        assert_eq!(v["kind"], "rusttraycer.export");
        assert_eq!(v["exportVersion"], 1);
        assert_eq!(v["sourceHostId"], "h1");
        assert_eq!(v["exportedAt"], "2026-08-19T12:00:00Z");
        assert_eq!(v["tasks"][0]["preset"], "planning");
        assert_eq!(v["agents"][0]["taskId"], "t1");
        assert_eq!(v["agents"][0]["runLocation"], "local");
        assert_eq!(v["modelProfiles"].as_array().unwrap().len(), 0);
        assert!(v.get("export_version").is_none());
        assert!(v.get("source_host_id").is_none());
        assert!(v["agents"][0].get("hostId").is_none());
        assert!(v["agents"][0].get("providerSessionId").is_none());
        assert!(v["tasks"][0].get("workspaceIds").is_none());
        assert!(v.get("host").is_none());
        assert!(v.get("workspaces").is_none());
        assert!(v.get("worktrees").is_none());

        let exp: SyncExportParams = serde_json::from_str(r#"{"taskIds":["t1","t2"]}"#).unwrap();
        assert_eq!(exp.task_ids, vec!["t1", "t2"]);
        let ev = serde_json::to_value(&exp).unwrap();
        assert_eq!(ev["taskIds"][0], "t1");
        assert!(ev.get("task_ids").is_none());

        let ok = SyncExportOk {
            archive: archive.clone(),
        };
        let v = serde_json::to_value(&ok).unwrap();
        assert_eq!(v["archive"]["kind"], "rusttraycer.export");

        let imp: SyncImportParams = serde_json::from_value(serde_json::json!({
            "workspaceId": "w1",
            "archive": archive
        }))
        .unwrap();
        assert_eq!(imp.workspace_id, "w1");
        let iv = serde_json::to_value(&imp).unwrap();
        assert_eq!(iv["workspaceId"], "w1");
        assert!(iv.get("workspace_id").is_none());

        let counts = SyncImportOk {
            tasks: 1,
            agents: 2,
            messages: 10,
            artifacts: 1,
            profiles_imported: 0,
            profiles_skipped: 1,
        };
        let v = serde_json::to_value(&counts).unwrap();
        assert_eq!(v["tasks"], 1);
        assert_eq!(v["agents"], 2);
        assert_eq!(v["messages"], 10);
        assert_eq!(v["artifacts"], 1);
        assert_eq!(v["profilesImported"], 0);
        assert_eq!(v["profilesSkipped"], 1);
        assert!(v.get("profiles_imported").is_none());

        let thread = ExportCommentThread {
            id: "th1".into(),
            artifact_id: "art1".into(),
            anchor_start: 0,
            anchor_end: 5,
            resolved: false,
            created_at: "c".into(),
            updated_at: "u".into(),
        };
        let v = serde_json::to_value(&thread).unwrap();
        assert_eq!(v["artifactId"], "art1");
        assert_eq!(v["anchorStart"], 0);
        assert!(v.get("comments").is_none());

        let c = ExportComment {
            id: "c1".into(),
            thread_id: "th1".into(),
            body: "nit".into(),
            created_at: "c".into(),
        };
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!(v["threadId"], "th1");
        assert!(v.get("thread_id").is_none());
    }

    #[test]
    fn search_and_worktree_gc_types_camel_case() {
        let p: SearchQueryParams =
            serde_json::from_str(r#"{"q":"hello","kinds":["task","workspace","artifact"]}"#)
                .unwrap();
        assert_eq!(p.q, "hello");
        assert_eq!(p.kinds.as_ref().map(|k| k.len()), Some(3));
        let item = SearchItem {
            kind: SearchKind::Task,
            id: "t1".into(),
            title: "Title".into(),
            hint: "open".into(),
        };
        let v = serde_json::to_value(&item).unwrap();
        assert_eq!(v["kind"], "task");
        assert_eq!(v["id"], "t1");
        assert_eq!(v["title"], "Title");
        assert_eq!(v["hint"], "open");
        assert!(v.get("worktree_id").is_none());
        let ok = SearchQueryOk { items: vec![item] };
        let v = serde_json::to_value(&ok).unwrap();
        assert_eq!(v["items"][0]["kind"], "task");

        let gcp: WorktreeGcParams = serde_json::from_str(r#"{"dryRun":true}"#).unwrap();
        assert!(gcp.dry_run);
        let item = WorktreeGcItem {
            worktree_id: "w1".into(),
            path: "/wt".into(),
            reason: WorktreeGcReason::Stale,
        };
        let v = serde_json::to_value(&item).unwrap();
        assert_eq!(v["worktreeId"], "w1");
        assert_eq!(v["path"], "/wt");
        assert_eq!(v["reason"], "stale");
        assert!(v.get("worktree_id").is_none());
        let ok = WorktreeGcOk {
            dry_run: true,
            items: vec![item],
        };
        let v = serde_json::to_value(&ok).unwrap();
        assert_eq!(v["dryRun"], true);
        assert_eq!(v["items"][0]["reason"], "stale");
        assert_eq!(
            serde_json::to_value(WorktreeGcReason::Merged).unwrap(),
            "merged"
        );
        assert_eq!(
            serde_json::to_value(WorktreeGcReason::Landed).unwrap(),
            "landed"
        );
    }
}
