//! Wire contract for client ↔ host. camelCase on the wire. No storage, no HTTP.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CRATE_VERSION: &str = "1.0.0";
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
];

pub fn host_method_version() -> MethodVersion {
    MethodVersion {
        major: HOST_METHOD_MAJOR,
        minor: HOST_METHOD_MINOR,
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
        assert_eq!(CRATE_VERSION, "1.0.0");
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
        assert_eq!(TRADABLE_METHODS.len(), 22);
        assert!(!TRADABLE_METHODS.contains(&METHOD_HANDSHAKE));
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
            "artifact.create".into(),
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
        assert_eq!(v["rejected"]["artifact.create"]["reason"], "unsupported");
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
        };
        let v = serde_json::to_value(&agent).unwrap();
        assert_eq!(v["taskId"], "t1");
        assert_eq!(v["hostId"], "h");
        assert_eq!(v["parentId"], serde_json::Value::Null);
        assert_eq!(v["runLocation"], "local");
        assert_eq!(v["createdAt"], "c");
        let agent2: Agent = serde_json::from_value(v).unwrap();
        assert_eq!(agent2.provider, "cli.generic");

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
}
