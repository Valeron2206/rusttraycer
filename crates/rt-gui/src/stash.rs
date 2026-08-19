//! C62 stash palette. `stash.list|add|delete` at protocol 1.9.
//! Host without 1.9 degrades; chat/write/pty/artifacts/search/steer/PR stay.

use serde_json::{json, Value};

pub const STASH_UNAVAILABLE: &str = "stash недоступен: host без 1.9";
pub const STASH_TITLE: &str = "Черновики";
pub const STASH_HEADING: &str = "Stash";
pub const STASH_ADD: &str = "В stash";
pub const STASH_DELETE: &str = "Удалить";
pub const STASH_EMPTY: &str = "нет черновиков";
pub const STASH_HINT: &str = "текст черновика";
pub const STASH_OPEN: &str = "Черновики";

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct StashItem {
    pub id: String,
    pub body: String,
}

/// `None` when body is empty after trim — no RPC.
pub fn stash_add_params(body: &str, image_path: Option<&str>) -> Option<Value> {
    let body = body.trim();
    if body.is_empty() {
        return None;
    }
    let mut params = json!({ "body": body });
    if let Some(path) = image_path.map(str::trim).filter(|s| !s.is_empty()) {
        params["imagePath"] = json!(path);
    }
    Some(params)
}

/// `None` when id is empty after trim — no RPC.
pub fn stash_delete_params(id: &str) -> Option<Value> {
    let id = id.trim();
    if id.is_empty() {
        return None;
    }
    Some(json!({ "id": id }))
}

pub fn stash_list_params() -> Value {
    json!({})
}

pub fn parse_stash_list(ok: &Value) -> Vec<StashItem> {
    let Some(arr) = first_array(ok, &["items", "stash", "drafts"]) else {
        let item = parse_item(ok);
        if item.id.is_empty() && item.body.is_empty() {
            return Vec::new();
        }
        return vec![item];
    };
    arr.iter()
        .filter_map(|v| {
            let item = parse_item(v);
            if item.id.is_empty() && item.body.is_empty() {
                None
            } else {
                Some(item)
            }
        })
        .collect()
}

pub fn parse_stash_item(ok: &Value) -> StashItem {
    if let Some(nested) = ok.get("item").filter(|v| v.is_object()) {
        return parse_item(nested);
    }
    if let Some(nested) = ok.get("stash").filter(|v| v.is_object()) {
        return parse_item(nested);
    }
    parse_item(ok)
}

fn parse_item(v: &Value) -> StashItem {
    StashItem {
        id: first_str(v, &["id", "stashId", "stash_id"]).unwrap_or_default(),
        body: first_str(v, &["body", "prompt", "text", "content"]).unwrap_or_default(),
    }
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

fn first_str(v: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        let Some(item) = v.get(*key) else {
            continue;
        };
        if let Some(s) = as_nonempty_str(item) {
            return Some(s);
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
        assert_eq!(STASH_UNAVAILABLE, "stash недоступен: host без 1.9");
        assert_eq!(STASH_TITLE, "Черновики");
        assert_eq!(STASH_HEADING, "Stash");
        assert_eq!(STASH_ADD, "В stash");
        assert_eq!(STASH_DELETE, "Удалить");
        assert_eq!(STASH_EMPTY, "нет черновиков");
        assert_eq!(STASH_HINT, "текст черновика");
        assert_eq!(STASH_OPEN, "Черновики");
        assert_eq!(crate::rpc::METHOD_STASH_LIST, "stash.list");
        assert_eq!(crate::rpc::METHOD_STASH_ADD, "stash.add");
        assert_eq!(crate::rpc::METHOD_STASH_DELETE, "stash.delete");
        assert_eq!(crate::rpc::STASH_METHODS.len(), 3);
    }

    #[test]
    fn add_params_body_only_omits_image() {
        assert_eq!(
            stash_add_params("  hello  ", None),
            Some(json!({ "body": "hello" }))
        );
        let with_img = stash_add_params("hi", Some(" /tmp/a.png ")).expect("img");
        assert_eq!(with_img["body"], "hi");
        assert_eq!(with_img["imagePath"], "/tmp/a.png");
        let no_img = stash_add_params("hi", Some("  ")).expect("blank img");
        assert!(no_img.get("imagePath").is_none());
        assert_eq!(no_img.as_object().map(|m| m.len()), Some(1));
        assert!(stash_add_params("   ", None).is_none());
        assert!(stash_add_params("", Some("/x")).is_none());
    }

    #[test]
    fn delete_params_id_only() {
        assert_eq!(stash_delete_params("  s-1  "), Some(json!({ "id": "s-1" })));
        assert!(stash_delete_params("").is_none());
        assert!(stash_delete_params("   ").is_none());
        assert_eq!(stash_list_params(), json!({}));
    }

    #[test]
    fn parse_accepts_camel_and_aliases() {
        let camel = json!({
            "items": [{ "id": "s1", "body": "draft" }]
        });
        assert_eq!(
            parse_stash_list(&camel),
            vec![StashItem {
                id: "s1".into(),
                body: "draft".into()
            }]
        );
        let aliases = json!({
            "stash": { "items": [{ "stashId": "s2", "prompt": "p" }] }
        });
        assert_eq!(
            parse_stash_list(&aliases),
            vec![StashItem {
                id: "s2".into(),
                body: "p".into()
            }]
        );
        let item = parse_stash_item(&json!({ "item": { "id": "s3", "text": "t" } }));
        assert_eq!(item.id, "s3");
        assert_eq!(item.body, "t");
        assert!(parse_stash_list(&json!({})).is_empty());
    }
}
