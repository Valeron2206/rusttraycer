//! Local `$RUSTTRAYCER_HOME/hooks.json` editor. Not RPC, not sqlite.
//! Values are command or URL only — no token/secret fields.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::discovery;

pub const HOOKS_LABEL: &str = "hooks.json";
pub const HOOKS_SAVE: &str = "Сохранить";
pub const HOOKS_HINT: &str = "command или URL";
pub const HOOKS_INVALID: &str = "hooks.json: невалидный JSON";
pub const HOOKS_FORBIDDEN: &str = "hooks.json: секретные поля запрещены";
pub const HOOKS_SAVED: &str = "hooks.json сохранён";

const FORBIDDEN_KEYS: &[&str] = &[
    "secret",
    "token",
    "password",
    "pat",
    "hook-secret",
    "hook_secret",
    "hooksecret",
];

/// `$RUSTTRAYCER_HOME/hooks.json` if set, else `~/.rusttraycer/hooks.json`.
pub fn hooks_json_path() -> PathBuf {
    hooks_json_path_from(
        std::env::var("RUSTTRAYCER_HOME").ok().as_deref(),
        std::env::var("HOME").ok().as_deref(),
    )
}

pub fn hooks_json_path_from(rusttraycer_home: Option<&str>, user_home: Option<&str>) -> PathBuf {
    if let Some(home) = rusttraycer_home.map(str::trim).filter(|s| !s.is_empty()) {
        return PathBuf::from(home).join("hooks.json");
    }
    let home = user_home
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(".");
    PathBuf::from(home).join(".rusttraycer").join("hooks.json")
}

pub fn default_hooks_path_display() -> String {
    discovery::rusttraycer_home()
        .join("hooks.json")
        .display()
        .to_string()
}

pub fn validate_hooks_json(text: &str) -> Result<(), String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    let value: Value = serde_json::from_str(trimmed).map_err(|_| HOOKS_INVALID.to_string())?;
    check_value(&value)
}

fn check_value(value: &Value) -> Result<(), String> {
    match value {
        Value::Null | Value::String(_) => Ok(()),
        Value::Array(items) => {
            for item in items {
                check_value(item)?;
            }
            Ok(())
        }
        Value::Object(map) => {
            for (key, item) in map {
                if is_forbidden_key(key) {
                    return Err(HOOKS_FORBIDDEN.to_string());
                }
                match item {
                    Value::String(_) | Value::Null => {}
                    Value::Object(_) | Value::Array(_) => check_value(item)?,
                    _ => return Err(HOOKS_INVALID.to_string()),
                }
            }
            Ok(())
        }
        _ => Err(HOOKS_INVALID.to_string()),
    }
}

fn is_forbidden_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    FORBIDDEN_KEYS.iter().any(|k| lower == *k)
}

pub fn load_hooks_at(path: &Path) -> Result<String, String> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(err) => Err(format!("hooks.json: {err}")),
    }
}

/// Validate then write. Invalid JSON or secret fields → no write.
pub fn save_hooks_at(path: &Path, text: &str) -> Result<(), String> {
    validate_hooks_json(text)?;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|err| format!("hooks.json: {err}"))?;
        }
    }
    let payload = if text.trim().is_empty() { "{}\n" } else { text };
    fs::write(path, payload).map_err(|err| format!("hooks.json: {err}"))
}

pub fn load_hooks() -> Result<String, String> {
    load_hooks_at(&hooks_json_path())
}

pub fn save_hooks(text: &str) -> Result<(), String> {
    save_hooks_at(&hooks_json_path(), text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn ui_copy_is_locked() {
        assert_eq!(HOOKS_LABEL, "hooks.json");
        assert_eq!(HOOKS_SAVE, "Сохранить");
        assert_eq!(HOOKS_HINT, "command или URL");
        assert_eq!(HOOKS_INVALID, "hooks.json: невалидный JSON");
        assert_eq!(HOOKS_FORBIDDEN, "hooks.json: секретные поля запрещены");
        assert_eq!(HOOKS_SAVED, "hooks.json сохранён");
    }

    #[test]
    fn path_uses_rusttraycer_home_then_dot_rusttraycer() {
        let via_env = hooks_json_path_from(Some("/tmp/rt-home"), Some("/home/u"));
        assert_eq!(via_env, PathBuf::from("/tmp/rt-home/hooks.json"));
        let via_home = hooks_json_path_from(None, Some("/home/u"));
        assert_eq!(via_home, PathBuf::from("/home/u/.rusttraycer/hooks.json"));
        let via_blank = hooks_json_path_from(Some("  "), Some("/home/u"));
        assert_eq!(via_blank, PathBuf::from("/home/u/.rusttraycer/hooks.json"));
        assert!(via_env.file_name().is_some_and(|n| n == "hooks.json"));
        assert!(!via_env.to_string_lossy().contains("sqlite"));
        assert!(!via_env.to_string_lossy().contains("host.db"));
    }

    #[test]
    fn validate_rejects_invalid_json_and_secrets() {
        assert!(validate_hooks_json("").is_ok());
        assert!(validate_hooks_json("{}").is_ok());
        assert!(validate_hooks_json(r#"{"push":"https://ex"}"#).is_ok());
        assert!(validate_hooks_json(r#"{"push":{"command":"echo"}}"#).is_ok());
        assert_eq!(validate_hooks_json("{"), Err(HOOKS_INVALID.to_string()));
        assert_eq!(
            validate_hooks_json(r#"{"token":"x"}"#),
            Err(HOOKS_FORBIDDEN.to_string())
        );
        assert_eq!(
            validate_hooks_json(r#"{"hook-secret":"x"}"#),
            Err(HOOKS_FORBIDDEN.to_string())
        );
        assert_eq!(
            validate_hooks_json(r#"{"push":1}"#),
            Err(HOOKS_INVALID.to_string())
        );
    }

    #[test]
    fn save_load_roundtrip_and_invalid_does_not_write() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("rt-gui-hooks-{nanos}"));
        let path = dir.join("hooks.json");
        let body = "{\n  \"onEvent\": \"https://example.local/hook\"\n}\n";
        save_hooks_at(&path, body).expect("save");
        assert_eq!(load_hooks_at(&path).expect("load"), body);
        let before = fs::read_to_string(&path).expect("read");
        assert!(save_hooks_at(&path, "{").is_err());
        assert_eq!(fs::read_to_string(&path).expect("still"), before);
        assert!(save_hooks_at(&path, r#"{"secret":"nope"}"#).is_err());
        assert_eq!(fs::read_to_string(&path).expect("still"), before);
        assert!(!path.to_string_lossy().contains(".db"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_file_is_empty_not_panic() {
        let path = PathBuf::from("/tmp/rt-gui-hooks-missing-surely-not-there.json");
        assert_eq!(load_hooks_at(&path).expect("missing"), "");
    }
}
