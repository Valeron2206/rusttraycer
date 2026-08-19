//! E9 sync GUI: durable export / import via host RPC. No sqlite. No C58 daemon.
//! Host without 1.8 degrades; chat/write/pty/artifacts/a2a/switch/guides stay.

use serde_json::{json, Value};

pub const SYNC_UNAVAILABLE: &str = "синк недоступен: host без 1.8";
pub const SYNC_PEER_UNAVAILABLE: &str = "синк peer недоступен: host без 1.9";
pub const SYNC_SECTION: &str = "Синк";
pub const EXPORT_BUTTON: &str = "Экспорт";
pub const IMPORT_BUTTON: &str = "Импорт";
pub const IMPORT_CONFIRM_TITLE: &str = "Импортировать архив?";
pub const IMPORT_CONFIRM_BODY: &str =
    "Задачи и агенты будут клонированы в текущий workspace. Совпадения id дадут conflict.";
pub const IMPORT_CONFIRM_OK: &str = "Импортировать";
pub const EXPORT_SAVED: &str = "архив сохранён";
pub const EXPORT_KIND: &str = "rusttraycer.export";
pub const NEED_TASK: &str = "сначала выберите задачу";
pub const NEED_WORKSPACE: &str = "сначала добавьте папку";
pub const PULL_NEED_WORKSPACE: &str = "нет workspace";
pub const EXPORT_MAX_TASKS: usize = 32;
pub const PEER_URL_LABEL: &str = "URL peer";
pub const PEER_URL_HINT: &str = "http://127.0.0.1:…";
pub const PUSH_BUTTON: &str = "Push";
pub const PULL_BUTTON: &str = "Pull";
pub const PUSH_OK: &str = "push выполнен";
pub const PULL_OK: &str = "pull выполнен";
pub const PULL_CONFIRM_TITLE: &str = "Забрать архив с peer?";
pub const PULL_CONFIRM_BODY: &str =
    "Задачи и агенты будут клонированы в текущий workspace. Совпадения id дадут conflict.";
pub const PULL_CONFIRM_OK: &str = "Pull";
pub const SYNC_SECRET_HINT: &str = "host читает RUSTTRAYCER_SYNC_SECRET из env";

const SECRET_KEYS: &[&str] = &[
    "token",
    "sessiontoken",
    "session_token",
    "password",
    "passwd",
    "secret",
    "apikey",
    "api_key",
    "authorization",
    "credential",
    "credentials",
    "pat",
    "keyring",
    "providersessionid",
    "provider_session_id",
    "privatekey",
    "private_key",
];

fn is_secret_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    SECRET_KEYS.iter().any(|name| lower == *name)
}

/// Drop credential-like fields before the client writes a file or shows a toast.
pub fn strip_secrets(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (key, child) in map {
                if is_secret_key(&key) {
                    continue;
                }
                out.insert(key, strip_secrets(child));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(strip_secrets).collect()),
        other => other,
    }
}

pub fn export_filename(archive: &Value) -> String {
    match archive
        .get("sourceHostId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(id) => format!("rusttraycer-{id}.json"),
        None => "rusttraycer-export.json".into(),
    }
}

pub fn unwrap_archive(value: Value) -> Value {
    if value
        .get("kind")
        .and_then(Value::as_str)
        .is_some_and(|k| k == EXPORT_KIND)
    {
        return value;
    }
    match value.get("archive") {
        Some(inner) if !inner.is_null() => inner.clone(),
        _ => value,
    }
}

/// Host counts only — never invent numbers. `hostId` is dest session id.
pub fn format_import_result(ok: &Value, host_id: &str) -> String {
    const KEYS: &[&str] = &[
        "tasks",
        "agents",
        "messages",
        "artifacts",
        "profilesImported",
        "profilesSkipped",
    ];
    let mut parts = Vec::new();
    for key in KEYS {
        if let Some(n) = ok.get(*key).and_then(Value::as_i64) {
            parts.push(format!("{key}={n}"));
        }
    }
    let dest = host_id.trim();
    if !dest.is_empty() {
        parts.push(format!("hostId={dest}"));
    }
    if parts.is_empty() {
        "импорт выполнен".into()
    } else {
        format!("импорт: {}", parts.join(" "))
    }
}

/// `{ peerUrl }` only. Empty URL → no RPC. Never a secret/token field.
pub fn push_params(peer_url: &str) -> Option<Value> {
    let peer_url = peer_url.trim();
    if peer_url.is_empty() {
        return None;
    }
    Some(json!({ "peerUrl": peer_url }))
}

/// `{ peerUrl, workspaceId }`. Empty URL or workspace → no RPC.
pub fn pull_params(peer_url: &str, workspace_id: &str) -> Option<Value> {
    let peer_url = peer_url.trim();
    let workspace_id = workspace_id.trim();
    if peer_url.is_empty() || workspace_id.is_empty() {
        return None;
    }
    Some(json!({ "peerUrl": peer_url, "workspaceId": workspace_id }))
}

pub fn format_push_result(ok: &Value) -> String {
    if ok.as_object().is_some_and(|m| m.is_empty()) || ok.is_null() {
        return PUSH_OK.into();
    }
    let summary = format_import_result(ok, "");
    if summary == "импорт выполнен" {
        PUSH_OK.into()
    } else {
        format!("push: {}", summary.trim_start_matches("импорт: ").trim())
    }
}

pub fn format_pull_result(ok: &Value, host_id: &str) -> String {
    let summary = format_import_result(ok, host_id);
    if summary == "импорт выполнен" {
        PULL_OK.into()
    } else {
        summary.replacen("импорт:", "pull:", 1)
    }
}

pub fn export_task_ids(selected: Option<&str>, tasks: &[String]) -> Vec<String> {
    if let Some(id) = selected.map(str::trim).filter(|s| !s.is_empty()) {
        return vec![id.to_string()];
    }
    tasks.iter().take(EXPORT_MAX_TASKS).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ui_copy_is_locked() {
        assert_eq!(SYNC_UNAVAILABLE, "синк недоступен: host без 1.8");
        assert_eq!(SYNC_SECTION, "Синк");
        assert_eq!(EXPORT_BUTTON, "Экспорт");
        assert_eq!(IMPORT_BUTTON, "Импорт");
        assert_eq!(IMPORT_CONFIRM_TITLE, "Импортировать архив?");
        assert_eq!(
            IMPORT_CONFIRM_BODY,
            "Задачи и агенты будут клонированы в текущий workspace. Совпадения id дадут conflict."
        );
        assert_eq!(IMPORT_CONFIRM_OK, "Импортировать");
        assert_eq!(EXPORT_SAVED, "архив сохранён");
        assert_eq!(NEED_TASK, "сначала выберите задачу");
        assert_eq!(NEED_WORKSPACE, "сначала добавьте папку");
        assert_eq!(SYNC_PEER_UNAVAILABLE, "синк peer недоступен: host без 1.9");
        assert_eq!(PULL_NEED_WORKSPACE, "нет workspace");
        assert_eq!(PEER_URL_LABEL, "URL peer");
        assert_eq!(PEER_URL_HINT, "http://127.0.0.1:…");
        assert_eq!(PUSH_BUTTON, "Push");
        assert_eq!(PULL_BUTTON, "Pull");
        assert_eq!(PUSH_OK, "push выполнен");
        assert_eq!(PULL_OK, "pull выполнен");
        assert_eq!(PULL_CONFIRM_TITLE, "Забрать архив с peer?");
        assert_eq!(PULL_CONFIRM_BODY, IMPORT_CONFIRM_BODY);
        assert_eq!(PULL_CONFIRM_OK, "Pull");
        assert_eq!(
            SYNC_SECRET_HINT,
            "host читает RUSTTRAYCER_SYNC_SECRET из env"
        );
        assert_eq!(crate::rpc::METHOD_SYNC_EXPORT, "sync.export");
        assert_eq!(crate::rpc::METHOD_SYNC_IMPORT, "sync.import");
        assert_eq!(crate::rpc::METHOD_SYNC_PUSH, "sync.push");
        assert_eq!(crate::rpc::METHOD_SYNC_PULL, "sync.pull");
        assert_eq!(crate::rpc::SYNC_METHODS.len(), 2);
        assert_eq!(crate::rpc::SYNC_PEER_METHODS.len(), 2);
        assert!(!crate::rpc::SYNC_METHODS.contains(&crate::rpc::METHOD_SYNC_PUSH));
        assert!(!crate::rpc::SYNC_METHODS.contains(&crate::rpc::METHOD_SYNC_PULL));
        assert_eq!(EXPORT_MAX_TASKS, 32);
        assert_eq!(export_filename(&json!({})), "rusttraycer-export.json");
        assert_eq!(
            export_filename(&json!({"sourceHostId": "host-a"})),
            "rusttraycer-host-a.json"
        );
        assert_eq!(
            export_task_ids(Some("task-1"), &["task-2".into()]),
            vec!["task-1".to_string()]
        );
        assert_eq!(
            export_task_ids(None, &["a".into(), "b".into()]),
            vec!["a".to_string(), "b".to_string()]
        );
    }

    #[test]
    fn strip_secrets_drops_credential_fields() {
        let raw = json!({
            "kind": EXPORT_KIND,
            "sourceHostId": "host-a",
            "token": "leak",
            "sessionToken": "sess",
            "agents": [{
                "id": "ag-1",
                "providerSessionId": "vendor",
                "apiKey": "k",
                "role": "coder"
            }]
        });
        let clean = strip_secrets(raw);
        assert_eq!(clean["kind"], EXPORT_KIND);
        assert_eq!(clean["sourceHostId"], "host-a");
        assert!(clean.get("token").is_none());
        assert!(clean.get("sessionToken").is_none());
        assert_eq!(clean["agents"][0]["id"], "ag-1");
        assert_eq!(clean["agents"][0]["role"], "coder");
        assert!(clean["agents"][0].get("providerSessionId").is_none());
        assert!(clean["agents"][0].get("apiKey").is_none());
    }

    #[test]
    fn import_result_uses_host_counts_only() {
        let ok = json!({"tasks": 1, "agents": 2, "profilesSkipped": 1});
        assert_eq!(
            format_import_result(&ok, "host-b"),
            "импорт: tasks=1 agents=2 profilesSkipped=1 hostId=host-b"
        );
        assert_eq!(format_import_result(&json!({}), ""), "импорт выполнен");
        let wrapped = json!({"archive": {"kind": EXPORT_KIND, "tasks": []}});
        assert_eq!(unwrap_archive(wrapped)["kind"], EXPORT_KIND);
    }

    #[test]
    fn push_params_peer_url_only_no_secret() {
        let params = push_params("  http://127.0.0.1:7420  ").expect("url");
        assert_eq!(params, json!({ "peerUrl": "http://127.0.0.1:7420" }));
        let keys: Vec<_> = params.as_object().expect("obj").keys().cloned().collect();
        assert_eq!(keys, vec!["peerUrl".to_string()]);
        assert!(params.get("secret").is_none());
        assert!(params.get("token").is_none());
        assert!(params.get("password").is_none());
        assert!(params.get("RUSTTRAYCER_SYNC_SECRET").is_none());
        assert!(push_params("").is_none());
        assert!(push_params("   ").is_none());
    }

    #[test]
    fn pull_params_peer_url_and_workspace_only() {
        let params = pull_params(" http://127.0.0.1:9 ", " ws-1 ").expect("ok");
        assert_eq!(
            params,
            json!({ "peerUrl": "http://127.0.0.1:9", "workspaceId": "ws-1" })
        );
        let keys: Vec<_> = params.as_object().expect("obj").keys().cloned().collect();
        assert_eq!(keys.len(), 2);
        assert!(params.get("secret").is_none());
        assert!(params.get("token").is_none());
        assert!(pull_params("", "ws-1").is_none());
        assert!(pull_params("http://127.0.0.1:9", "").is_none());
        assert!(pull_params("  ", "  ").is_none());
    }
}
