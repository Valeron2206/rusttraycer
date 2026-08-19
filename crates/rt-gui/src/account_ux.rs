//! C51 account picker + C53 steer helpers. Labels only — never token/secret fields.
//! Host without 1.9 degrades; chat/write/pty/artifacts/a2a/switch/guides/sync/search stay.

use serde_json::{json, Value};

pub const ACCOUNTS_UNAVAILABLE: &str = "аккаунты недоступны: host без 1.9";
pub const STEER_UNAVAILABLE: &str = "steer недоступен: host без 1.9";
pub const ACCOUNT_LABEL: &str = "Аккаунт";
pub const ACCOUNT_HINT: &str = "по умолчанию";
pub const STEER_HINT: &str = "⌘Enter / Ctrl+Enter — подсказка";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountItem {
    pub id: String,
    pub label: String,
    pub provider: Option<String>,
}

impl AccountItem {
    pub fn display_label(&self) -> &str {
        if self.label.is_empty() {
            self.id.as_str()
        } else {
            self.label.as_str()
        }
    }
}

pub fn account_list_params() -> Value {
    json!({})
}

/// `None` when agent id or content is empty after trim — caller must not send RPC.
pub fn steer_params(agent_id: &str, content: &str) -> Option<Value> {
    let agent_id = agent_id.trim();
    let content = content.trim();
    if agent_id.is_empty() || content.is_empty() {
        return None;
    }
    Some(json!({ "agentId": agent_id, "content": content }))
}

pub fn put_account_id(params: &mut Value, account_id: Option<&str>) {
    if let Some(id) = account_id.map(str::trim).filter(|s| !s.is_empty()) {
        params["accountId"] = json!(id);
    }
}

pub fn accounts_for_provider<'a>(
    accounts: &'a [AccountItem],
    provider: Option<&str>,
) -> Vec<&'a AccountItem> {
    let Some(provider) = provider.map(str::trim).filter(|s| !s.is_empty()) else {
        return accounts.iter().collect();
    };
    accounts
        .iter()
        .filter(|a| match a.provider.as_deref() {
            Some(p) => p == provider,
            None => true,
        })
        .collect()
}

/// Parse `account.list` ok. Accepts `items` or `accounts`. id from `id`/`accountId`.
/// Drops token/secret fields — they never enter `AccountItem`.
pub fn parse_account_list(ok: &Value) -> Vec<AccountItem> {
    let arr = ok
        .get("items")
        .and_then(Value::as_array)
        .or_else(|| ok.get("accounts").and_then(Value::as_array));
    let Some(arr) = arr else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in arr {
        if !item.is_object() {
            continue;
        }
        let id = item
            .get("id")
            .and_then(Value::as_str)
            .or_else(|| item.get("accountId").and_then(Value::as_str))
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let Some(id) = id else {
            continue;
        };
        let label = item
            .get("label")
            .and_then(Value::as_str)
            .or_else(|| item.get("name").and_then(Value::as_str))
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(id)
            .to_string();
        let provider = item
            .get("provider")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        out.push(AccountItem {
            id: id.to_string(),
            label,
            provider,
        });
    }
    out
}

pub fn command_or_ctrl(command: bool, ctrl: bool) -> bool {
    command || ctrl
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_copy_is_locked() {
        assert_eq!(ACCOUNTS_UNAVAILABLE, "аккаунты недоступны: host без 1.9");
        assert_eq!(STEER_UNAVAILABLE, "steer недоступен: host без 1.9");
        assert_eq!(ACCOUNT_LABEL, "Аккаунт");
        assert_eq!(ACCOUNT_HINT, "по умолчанию");
        assert_eq!(STEER_HINT, "⌘Enter / Ctrl+Enter — подсказка");
        assert_eq!(crate::rpc::METHOD_ACCOUNT_LIST, "account.list");
        assert_eq!(crate::rpc::METHOD_AGENT_STEER, "agent.steer");
        assert_eq!(crate::rpc::ACCOUNT_STEER_METHODS.len(), 2);
        assert!(command_or_ctrl(true, false));
        assert!(command_or_ctrl(false, true));
        assert!(!command_or_ctrl(false, false));
    }

    #[test]
    fn parse_items_labels_only_no_secrets() {
        let ok = json!({
            "items": [
                {
                    "id": "acc-1",
                    "label": "work",
                    "provider": "cli.claude",
                    "token": "SECRET",
                    "apiKey": "nope"
                },
                { "accountId": "acc-2", "name": "home" },
                { "label": "orphan" }
            ]
        });
        let items = parse_account_list(&ok);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, "acc-1");
        assert_eq!(items[0].label, "work");
        assert_eq!(items[0].provider.as_deref(), Some("cli.claude"));
        assert_eq!(items[0].display_label(), "work");
        assert_eq!(items[1].id, "acc-2");
        assert_eq!(items[1].label, "home");
        assert!(items[1].provider.is_none());
        let raw = serde_json::to_string(&items[0].id).expect("id");
        assert!(!raw.contains("SECRET"));
        assert!(!raw.contains("nope"));
    }

    #[test]
    fn parse_accounts_key_and_empty() {
        let ok = json!({
            "accounts": [{ "id": "a", "label": "one", "provider": "cli.codex" }]
        });
        let items = parse_account_list(&ok);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "a");
        assert!(parse_account_list(&json!({})).is_empty());
        assert!(parse_account_list(&json!({"items": []})).is_empty());
    }

    #[test]
    fn filter_by_provider_keeps_unscoped() {
        let items = vec![
            AccountItem {
                id: "c".into(),
                label: "claude".into(),
                provider: Some("cli.claude".into()),
            },
            AccountItem {
                id: "x".into(),
                label: "codex".into(),
                provider: Some("cli.codex".into()),
            },
            AccountItem {
                id: "any".into(),
                label: "any".into(),
                provider: None,
            },
        ];
        let claude = accounts_for_provider(&items, Some("cli.claude"));
        assert_eq!(
            claude.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(),
            vec!["c", "any"]
        );
        assert_eq!(accounts_for_provider(&items, None).len(), 3);
        assert_eq!(accounts_for_provider(&items, Some("  ")).len(), 3);
    }

    #[test]
    fn steer_params_trim_and_skip_empty() {
        assert_eq!(
            steer_params(" ag-1 ", "  nudge  "),
            Some(json!({ "agentId": "ag-1", "content": "nudge" }))
        );
        assert!(steer_params("", "nudge").is_none());
        assert!(steer_params("ag-1", "   ").is_none());
        assert!(steer_params("  ", "x").is_none());
        let params = steer_params("ag-1", "go").expect("params");
        assert!(params.get("token").is_none());
        assert_eq!(params.as_object().map(|m| m.len()), Some(2));
    }

    #[test]
    fn put_account_id_omits_empty() {
        let mut params = json!({ "taskId": "t", "provider": "cli.claude" });
        put_account_id(&mut params, None);
        assert!(params.get("accountId").is_none());
        put_account_id(&mut params, Some("  "));
        assert!(params.get("accountId").is_none());
        put_account_id(&mut params, Some(" acc-1 "));
        assert_eq!(params["accountId"], "acc-1");
    }
}
