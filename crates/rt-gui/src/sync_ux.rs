//! E9 sync GUI: durable export / import via host RPC. No sqlite. No C58 daemon.
//! Host without 1.8 degrades; chat/write/pty/artifacts/a2a/switch/guides stay.

use serde_json::Value;

pub const SYNC_UNAVAILABLE: &str = "синк недоступен: host без 1.8";
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
pub const EXPORT_MAX_TASKS: usize = 32;

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
        assert_eq!(crate::rpc::METHOD_SYNC_EXPORT, "sync.export");
        assert_eq!(crate::rpc::METHOD_SYNC_IMPORT, "sync.import");
        assert_eq!(crate::rpc::SYNC_METHODS.len(), 2);
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
}
