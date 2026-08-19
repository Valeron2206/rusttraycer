//! C64 PR panel helpers. `pr.get` only — no token/PAT field, no gh-login chrome.
//! Host without 1.9 degrades; chat/write/pty/artifacts/search/gc/accounts/steer stay.

use serde_json::{json, Value};

pub const PR_UNAVAILABLE: &str = "PR недоступен: host без 1.9";
pub const PR_NEED_WORKSPACE: &str = "нет workspace";
pub const PR_HEADING: &str = "PR";
pub const PR_NUMBER_LABEL: &str = "Номер";
pub const PR_NUMBER_HINT: &str = "номер";
pub const PR_URL_HINT: &str = "URL";
pub const PR_OPEN_BUTTON: &str = "Открыть PR";
pub const PR_CHECKS_HEADING: &str = "Проверки";
pub const PR_COMMITS_HEADING: &str = "Коммиты";
pub const PR_FILES_HEADING: &str = "Файлы";
pub const PR_DIFF_HEADING: &str = "Локальный diff";
pub const PR_EMPTY: &str = "нет";

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct PrView {
    pub title: Option<String>,
    pub number: Option<String>,
    pub url: Option<String>,
    pub checks: Vec<String>,
    pub commits: Vec<String>,
    pub files: Vec<String>,
    pub local_diff: Option<String>,
}

/// `None` when workspace is empty or both number and URL are empty — no RPC.
pub fn pr_get_params(workspace_id: &str, number: &str, url: &str) -> Option<Value> {
    let workspace_id = workspace_id.trim();
    if workspace_id.is_empty() {
        return None;
    }
    let number = number.trim().trim_start_matches('#');
    let url = url.trim();
    if number.is_empty() && url.is_empty() {
        return None;
    }
    let mut params = json!({ "workspaceId": workspace_id });
    if !number.is_empty() {
        if let Ok(n) = number.parse::<i64>() {
            params["number"] = json!(n);
        } else {
            params["number"] = json!(number);
        }
    }
    if !url.is_empty() {
        params["url"] = json!(url);
    }
    Some(params)
}

pub fn parse_pr_get(ok: &Value) -> PrView {
    let root = ok.get("pr").filter(|v| v.is_object()).unwrap_or(ok);
    PrView {
        title: first_str(root, &["title", "name"]),
        number: first_str(root, &["number", "prNumber"]),
        url: first_str(root, &["url", "htmlUrl", "html_url"]),
        checks: parse_list(
            root,
            &["checks"],
            &["name", "context", "title"],
            &["status", "state", "conclusion"],
        ),
        commits: parse_list(
            root,
            &["commits"],
            &["sha", "oid", "id"],
            &["message", "title"],
        ),
        files: parse_list(
            root,
            &["files"],
            &["path", "filename", "name"],
            &["status", "changeType"],
        ),
        local_diff: parse_diff(root),
    }
}

fn parse_list(root: &Value, keys: &[&str], primary: &[&str], extra: &[&str]) -> Vec<String> {
    let Some(arr) = first_array(root, keys) else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|item| {
            let line = format_item(item, primary, extra);
            if line.is_empty() {
                None
            } else {
                Some(line)
            }
        })
        .collect()
}

fn parse_diff(root: &Value) -> Option<String> {
    for key in ["localDiff", "local_diff", "diff"] {
        let Some(v) = root.get(key) else {
            continue;
        };
        if let Some(s) = as_nonempty_str(v) {
            return Some(s);
        }
        if let Some(s) = first_str(v, &["patch", "text", "diff", "content"]) {
            return Some(s);
        }
        if let Some(files) = v.get("files").and_then(Value::as_array) {
            let mut out = String::new();
            for file in files {
                if let Some(path) = first_str(file, &["path", "filename"]) {
                    if !out.is_empty() {
                        out.push('\n');
                    }
                    out.push_str(&path);
                }
                if let Some(patch) = first_str(file, &["patch", "diff", "text"]) {
                    if !out.is_empty() {
                        out.push('\n');
                    }
                    out.push_str(&patch);
                }
            }
            if !out.is_empty() {
                return Some(out);
            }
        }
    }
    None
}

fn first_array<'a>(root: &'a Value, keys: &[&str]) -> Option<&'a Vec<Value>> {
    for key in keys {
        let Some(v) = root.get(*key) else {
            continue;
        };
        if let Some(arr) = v.as_array() {
            return Some(arr);
        }
        if let Some(arr) = v.get("items").and_then(Value::as_array) {
            return Some(arr);
        }
    }
    None
}

fn format_item(v: &Value, primary: &[&str], extra: &[&str]) -> String {
    if let Some(s) = as_nonempty_str(v) {
        return s;
    }
    let head = first_str(v, primary).unwrap_or_default();
    let tail: Vec<String> = extra.iter().filter_map(|k| first_str(v, &[k])).collect();
    if head.is_empty() {
        return tail.join(" · ");
    }
    if tail.is_empty() {
        return head;
    }
    format!("{} · {}", head, tail.join(" · "))
}

fn first_str(v: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        let Some(item) = v.get(*key) else {
            continue;
        };
        if let Some(s) = as_nonempty_str(item) {
            return Some(s);
        }
        if let Some(n) = item.as_i64() {
            return Some(n.to_string());
        }
        if let Some(n) = item.as_u64() {
            return Some(n.to_string());
        }
    }
    None
}

fn as_nonempty_str(v: &Value) -> Option<String> {
    v.as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_copy_is_locked() {
        assert_eq!(PR_UNAVAILABLE, "PR недоступен: host без 1.9");
        assert_eq!(PR_NEED_WORKSPACE, "нет workspace");
        assert_eq!(PR_HEADING, "PR");
        assert_eq!(PR_NUMBER_LABEL, "Номер");
        assert_eq!(PR_NUMBER_HINT, "номер");
        assert_eq!(PR_URL_HINT, "URL");
        assert_eq!(PR_OPEN_BUTTON, "Открыть PR");
        assert_eq!(PR_CHECKS_HEADING, "Проверки");
        assert_eq!(PR_COMMITS_HEADING, "Коммиты");
        assert_eq!(PR_FILES_HEADING, "Файлы");
        assert_eq!(PR_DIFF_HEADING, "Локальный diff");
        assert_eq!(PR_EMPTY, "нет");
        assert_eq!(crate::rpc::METHOD_PR_GET, "pr.get");
        assert_eq!(crate::rpc::PR_METHODS.len(), 1);
    }

    #[test]
    fn params_send_workspace_and_number_or_url() {
        assert_eq!(
            pr_get_params(" ws-1 ", " 91 ", ""),
            Some(json!({ "workspaceId": "ws-1", "number": 91 }))
        );
        assert_eq!(
            pr_get_params("ws-1", "#42", ""),
            Some(json!({ "workspaceId": "ws-1", "number": 42 }))
        );
        assert_eq!(
            pr_get_params("ws-1", "", " https://example.com/pr/1 "),
            Some(json!({
                "workspaceId": "ws-1",
                "url": "https://example.com/pr/1"
            }))
        );
        let both = pr_get_params("ws-1", "7", "https://ex/7").expect("both");
        assert_eq!(both["workspaceId"], "ws-1");
        assert_eq!(both["number"], 7);
        assert_eq!(both["url"], "https://ex/7");
        assert!(both.get("token").is_none());
        assert!(both.get("pat").is_none());
        assert!(both.get("secret").is_none());
        assert_eq!(both.as_object().map(|m| m.len()), Some(3));
    }

    #[test]
    fn empty_query_or_workspace_is_none() {
        assert!(pr_get_params("", "91", "").is_none());
        assert!(pr_get_params("   ", "91", "https://x").is_none());
        assert!(pr_get_params("ws-1", "", "").is_none());
        assert!(pr_get_params("ws-1", "  ", "  ").is_none());
        assert!(pr_get_params("ws-1", "#", "").is_none());
    }

    #[test]
    fn parse_accepts_camel_and_aliases() {
        let camel = json!({
            "title": "Panel",
            "number": 91,
            "url": "https://ex/91",
            "checks": [{ "name": "ci", "status": "success" }],
            "commits": [{ "sha": "abc1234", "message": "feat" }],
            "files": [{ "path": "a.rs", "status": "modified" }],
            "localDiff": "diff --git a/a.rs"
        });
        let view = parse_pr_get(&camel);
        assert_eq!(view.title.as_deref(), Some("Panel"));
        assert_eq!(view.number.as_deref(), Some("91"));
        assert_eq!(view.url.as_deref(), Some("https://ex/91"));
        assert_eq!(view.checks, vec!["ci · success"]);
        assert_eq!(view.commits, vec!["abc1234 · feat"]);
        assert_eq!(view.files, vec!["a.rs · modified"]);
        assert_eq!(view.local_diff.as_deref(), Some("diff --git a/a.rs"));

        let aliases = json!({
            "pr": {
                "name": "Nested",
                "prNumber": 2,
                "htmlUrl": "https://ex/2",
                "checks": { "items": ["lint"] },
                "commits": [{ "oid": "def", "title": "fix" }],
                "files": [{ "filename": "b.rs", "changeType": "added" }],
                "diff": { "patch": "+ok" }
            }
        });
        let view = parse_pr_get(&aliases);
        assert_eq!(view.title.as_deref(), Some("Nested"));
        assert_eq!(view.number.as_deref(), Some("2"));
        assert_eq!(view.url.as_deref(), Some("https://ex/2"));
        assert_eq!(view.checks, vec!["lint"]);
        assert_eq!(view.commits, vec!["def · fix"]);
        assert_eq!(view.files, vec!["b.rs · added"]);
        assert_eq!(view.local_diff.as_deref(), Some("+ok"));

        let snake = json!({ "local_diff": "@@ -1 +1 @@" });
        assert_eq!(
            parse_pr_get(&snake).local_diff.as_deref(),
            Some("@@ -1 +1 @@")
        );
        assert!(parse_pr_get(&json!({})).checks.is_empty());
    }
}
