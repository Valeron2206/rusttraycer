//! C21 search + C65 worktree.gc GUI helpers. No sqlite. No hardcoded branch prefix.
//! Host without 1.9 degrades; chat/write/pty/artifacts/a2a/switch/guides/sync stay.

use serde_json::{json, Value};

pub const SEARCH_UNAVAILABLE: &str = "поиск недоступен: host без 1.9";
pub const GC_UNAVAILABLE: &str = "очистка worktree недоступна: host без 1.9";
pub const SEARCH_LABEL: &str = "Поиск";
pub const SEARCH_HINT: &str = "задача, папка, артефакт";
pub const SEARCH_EMPTY: &str = "нет результатов";
pub const GC_BUTTON: &str = "Очистить worktree";
pub const GC_CONFIRM_TITLE: &str = "Очистить worktree?";
pub const GC_CONFIRM_BODY: &str =
    "Удалить устаревшие и влитые worktree. Префикс ветки задаёт host.";
pub const GC_CONFIRM_OK: &str = "Очистить";
pub const GC_DONE: &str = "очистка выполнена";
pub const KIND_TASK: &str = "task";
pub const KIND_WORKSPACE: &str = "workspace";
pub const KIND_ARTIFACT: &str = "artifact";
pub const SEARCH_KINDS: [&str; 3] = [KIND_TASK, KIND_WORKSPACE, KIND_ARTIFACT];
pub const SEARCH_DEBOUNCE_MS: u64 = 350;

/// Enter submits while the search field has focus (not only after lost_focus).
pub fn search_enter_submits(has_focus: bool, lost_focus: bool, enter_pressed: bool) -> bool {
    enter_pressed && (has_focus || lost_focus)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchItem {
    pub kind: String,
    pub id: String,
    pub title: String,
    pub hint: String,
}

impl SearchItem {
    pub fn kind_label_ru(&self) -> &str {
        match self.kind.as_str() {
            KIND_TASK => "задача",
            KIND_WORKSPACE => "папка",
            KIND_ARTIFACT => "артефакт",
            other => other,
        }
    }
}

/// `None` when `q` is empty after trim — caller must not send RPC.
pub fn search_params(q: &str, kinds: Option<&[&str]>) -> Option<Value> {
    let q = q.trim();
    if q.is_empty() {
        return None;
    }
    let mut params = json!({ "q": q });
    if let Some(kinds) = kinds {
        let cleaned: Vec<&str> = kinds
            .iter()
            .copied()
            .filter(|k| SEARCH_KINDS.contains(k))
            .collect();
        if !cleaned.is_empty() {
            params["kinds"] = json!(cleaned);
        }
    }
    Some(params)
}

/// Host owns `worktree_settings.branch_prefix`. GUI sends only `dryRun`.
pub fn gc_params(dry_run: bool) -> Value {
    json!({ "dryRun": dry_run })
}

pub fn format_gc_result(ok: &Value) -> String {
    let deleted = ok.get("deleted").and_then(Value::as_array).map(|a| a.len());
    let items = ok.get("items").and_then(Value::as_array).map(|a| a.len());
    match (deleted, items) {
        (Some(n), _) => format!("очистка: deleted={n}"),
        (None, Some(n)) => format!("очистка: items={n}"),
        _ => GC_DONE.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_copy_is_locked() {
        assert_eq!(SEARCH_UNAVAILABLE, "поиск недоступен: host без 1.9");
        assert_eq!(GC_UNAVAILABLE, "очистка worktree недоступна: host без 1.9");
        assert_eq!(SEARCH_LABEL, "Поиск");
        assert_eq!(SEARCH_HINT, "задача, папка, артефакт");
        assert_eq!(SEARCH_EMPTY, "нет результатов");
        assert_eq!(GC_BUTTON, "Очистить worktree");
        assert_eq!(GC_CONFIRM_TITLE, "Очистить worktree?");
        assert_eq!(
            GC_CONFIRM_BODY,
            "Удалить устаревшие и влитые worktree. Префикс ветки задаёт host."
        );
        assert_eq!(GC_CONFIRM_OK, "Очистить");
        assert_eq!(GC_DONE, "очистка выполнена");
        assert_eq!(crate::rpc::METHOD_SEARCH_QUERY, "search.query");
        assert_eq!(crate::rpc::METHOD_WORKTREE_GC, "worktree.gc");
        assert_eq!(crate::rpc::SEARCH_GC_METHODS.len(), 2);
        assert_eq!(SEARCH_DEBOUNCE_MS, 350);
        let item = SearchItem {
            kind: KIND_TASK.into(),
            id: "t1".into(),
            title: "Auth".into(),
            hint: "open".into(),
        };
        assert_eq!(item.kind_label_ru(), "задача");
        assert_eq!(
            SearchItem {
                kind: KIND_WORKSPACE.into(),
                id: "w".into(),
                title: "ws".into(),
                hint: String::new(),
            }
            .kind_label_ru(),
            "папка"
        );
        assert_eq!(
            SearchItem {
                kind: KIND_ARTIFACT.into(),
                id: "a".into(),
                title: "spec".into(),
                hint: String::new(),
            }
            .kind_label_ru(),
            "артефакт"
        );
    }

    #[test]
    fn search_params_q_only_and_optional_kinds() {
        assert_eq!(
            search_params("  auth  ", None),
            Some(json!({ "q": "auth" }))
        );
        assert_eq!(
            search_params("auth", Some(&["task", "artifact"])),
            Some(json!({ "q": "auth", "kinds": ["task", "artifact"] }))
        );
        assert_eq!(
            search_params("auth", Some(&["task", "nope"])),
            Some(json!({ "q": "auth", "kinds": ["task"] }))
        );
        let params = search_params("auth", Some(&SEARCH_KINDS)).expect("kinds");
        assert_eq!(params["q"], "auth");
        assert_eq!(params["kinds"], json!(["task", "workspace", "artifact"]));
        assert!(params.get("prefix").is_none());
    }

    #[test]
    fn empty_q_is_none_no_payload() {
        assert!(search_params("", None).is_none());
        assert!(search_params("   ", None).is_none());
        assert!(search_params("\t\n", Some(&["task"])).is_none());
    }

    #[test]
    fn gc_params_are_dry_run_only_no_prefix() {
        assert_eq!(gc_params(false), json!({ "dryRun": false }));
        assert_eq!(gc_params(true), json!({ "dryRun": true }));
        let params = gc_params(false);
        assert!(params.get("prefix").is_none());
        assert!(params.get("branchPrefix").is_none());
        assert!(params.get("branch_prefix").is_none());
        assert_eq!(params.as_object().map(|m| m.len()), Some(1));
    }

    #[test]
    fn enter_submits_while_focused_or_lost_focus() {
        assert!(search_enter_submits(true, false, true));
        assert!(search_enter_submits(false, true, true));
        assert!(!search_enter_submits(true, false, false));
        assert!(!search_enter_submits(false, false, true));
    }

    #[test]
    fn format_gc_uses_host_counts() {
        assert_eq!(format_gc_result(&json!({})), GC_DONE);
        assert_eq!(
            format_gc_result(&json!({"deleted": [{"id": "wt-1"}]})),
            "очистка: deleted=1"
        );
        assert_eq!(
            format_gc_result(&json!({"items": [{}, {}]})),
            "очистка: items=2"
        );
    }
}
