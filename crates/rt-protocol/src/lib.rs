//! Wire contract for client ↔ host. camelCase on the wire. No storage, no HTTP.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CRATE_VERSION: &str = "0.1.0";
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
pub const METHOD_FILES_TREE: &str = "files.tree";
pub const METHOD_FILES_READ: &str = "files.read";

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
    METHOD_FILES_TREE,
    METHOD_FILES_READ,
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
}
