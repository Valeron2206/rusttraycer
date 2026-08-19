//! E8 workspace GUI: AGENTS.md / selection guides, agent roles, task presets.
//! Guides come from host RPC only. No workspace walk. Host without 1.7 degrades.

use serde_json::{json, Value};

use crate::rpc::{GuideFile, PresetItem, WorkspaceGuides};

pub const WORKSPACE_UNAVAILABLE: &str = "воркспейс-гайд недоступен: host без 1.7";
pub const ROLE_LABEL: &str = "Роль";
pub const PRESET_LABEL: &str = "Пресет";
pub const PRESET_NONE: &str = "без пресета";
pub const GUIDE_PANE: &str = "Гайды";
pub const AGENTS_MD_PRESENT: &str = "AGENTS.md есть";
pub const AGENTS_MD_MISSING: &str = "AGENTS.md нет";
pub const WS_GUIDE_PRESENT: &str = "гайд есть";
pub const WS_GUIDE_MISSING: &str = "гайда нет";
pub const GLOBAL_GUIDE_LABEL: &str = "Глобальный гайд выбора агента";
pub const GLOBAL_GUIDE_SAVE: &str = "Сохранить гайд";
pub const GLOBAL_GUIDE_HINT: &str = "как выбирать агента";
pub const GUIDE_TOO_LONG: &str = "гайд: не больше 65536 байт";
pub const PRESETS_UNAVAILABLE: &str = "пресеты недоступны: host без 1.9";
pub const PRESET_CREATE: &str = "Создать";
pub const PRESET_SAVE: &str = "Сохранить";
pub const PRESET_DELETE: &str = "Удалить";
pub const PRESET_NAME_LABEL: &str = "Имя";
pub const PRESET_NAME_HINT: &str = "имя пресета";
pub const PRESET_TITLE_HINT_LABEL: &str = "Подсказка названия";
pub const PRESET_PROMPT_LABEL: &str = "Промпт";
pub const PRESET_DELETE_TITLE: &str = "Удалить пресет?";
pub const PRESET_DELETE_BODY: &str = "Пользовательский пресет будет удалён.";
pub const PRESET_DELETE_OK: &str = "Удалить";

pub const ROLE_CODER: &str = "coder";
pub const ROLE_PLANNER: &str = "planner";
pub const ROLE_REVIEWER: &str = "reviewer";
pub const ROLE_DEBUGGER: &str = "debugger";
pub const ROLE_DOCUMENTER: &str = "documenter";

pub const ROLE_CHOICES: &[&str] = &[
    ROLE_CODER,
    ROLE_PLANNER,
    ROLE_REVIEWER,
    ROLE_DEBUGGER,
    ROLE_DOCUMENTER,
];

pub const PRESET_PLANNING: &str = "planning";
pub const PRESET_REVIEW: &str = "review";
pub const PRESET_DEBUG: &str = "debug";
pub const PRESET_DOCUMENT: &str = "document";

pub const PRESET_CHOICES: &[&str] = &[
    PRESET_PLANNING,
    PRESET_REVIEW,
    PRESET_DEBUG,
    PRESET_DOCUMENT,
];

pub const GUIDE_CONTENT_MAX: usize = 65_536;

pub fn valid_role(role: &str) -> bool {
    ROLE_CHOICES.contains(&role)
}

pub fn valid_preset(preset: &str) -> bool {
    PRESET_CHOICES.contains(&preset)
}

pub fn is_builtin_preset(id: &str) -> bool {
    valid_preset(id)
}

pub fn listed_preset(id: &str, listed: &[PresetItem]) -> bool {
    valid_preset(id) || listed.iter().any(|item| item.id == id)
}

pub fn preset_display_name(item: &PresetItem) -> &str {
    if !item.name.is_empty() {
        &item.name
    } else if !item.title.is_empty() {
        &item.title
    } else {
        &item.id
    }
}

pub fn preset_combo_label(item: &PresetItem) -> String {
    let name = preset_display_name(item);
    format!(
        "{} · {} → {}",
        preset_label_ru(&item.id),
        name,
        role_label_ru(&item.default_role)
    )
}

/// `None` when name/role empty or invalid — no RPC.
pub fn preset_create_params(
    name: &str,
    default_role: &str,
    title_hint: &str,
    prompt: &str,
) -> Option<Value> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let default_role = default_role.trim();
    if !valid_role(default_role) {
        return None;
    }
    let mut params = json!({
        "name": name,
        "defaultRole": default_role,
    });
    let hint = title_hint.trim();
    if !hint.is_empty() {
        params["titleHint"] = json!(hint);
    }
    let prompt = prompt.trim();
    if !prompt.is_empty() {
        params["prompt"] = json!(prompt);
    }
    Some(params)
}

/// `None` when id is empty or a built-in — no RPC.
pub fn preset_update_params(
    id: &str,
    name: &str,
    default_role: &str,
    title_hint: &str,
    prompt: &str,
) -> Option<Value> {
    let id = id.trim();
    if id.is_empty() || is_builtin_preset(id) {
        return None;
    }
    let mut params = preset_create_params(name, default_role, title_hint, prompt)?;
    params["id"] = json!(id);
    Some(params)
}

/// `None` when id is empty or a built-in — no RPC.
pub fn preset_delete_params(id: &str) -> Option<Value> {
    let id = id.trim();
    if id.is_empty() || is_builtin_preset(id) {
        return None;
    }
    Some(json!({ "id": id }))
}

pub fn parse_preset_item(ok: &Value) -> PresetItem {
    let src = if let Some(nested) = ok.get("item").filter(|v| v.is_object()) {
        nested
    } else if let Some(nested) = ok.get("preset").filter(|v| v.is_object()) {
        nested
    } else {
        ok
    };
    PresetItem {
        id: first_str(src, &["id", "presetId", "preset_id"]).unwrap_or_default(),
        title: first_str(src, &["title"]).unwrap_or_default(),
        name: first_str(src, &["name"]).unwrap_or_default(),
        default_role: first_str(src, &["defaultRole", "default_role", "role"]).unwrap_or_default(),
        title_hint: first_str(src, &["titleHint", "title_hint"]).unwrap_or_default(),
        prompt: first_str(src, &["prompt", "body"]).unwrap_or_default(),
    }
}

fn first_str(v: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        let Some(item) = v.get(*key) else {
            continue;
        };
        if let Some(s) = item
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
        {
            return Some(s);
        }
    }
    None
}

pub fn role_label_ru(role: &str) -> &str {
    match role {
        ROLE_CODER => "Кодер",
        ROLE_PLANNER => "Планировщик",
        ROLE_REVIEWER => "Ревьюер",
        ROLE_DEBUGGER => "Отладчик",
        ROLE_DOCUMENTER => "Документатор",
        _ => role,
    }
}

pub fn preset_label_ru(preset: &str) -> &str {
    match preset {
        PRESET_PLANNING => "Планирование",
        PRESET_REVIEW => "Ревью",
        PRESET_DEBUG => "Отладка",
        PRESET_DOCUMENT => "Документ",
        _ => preset,
    }
}

pub fn default_role_for_preset(preset: &str) -> &'static str {
    match preset {
        PRESET_PLANNING => ROLE_PLANNER,
        PRESET_REVIEW => ROLE_REVIEWER,
        PRESET_DEBUG => ROLE_DEBUGGER,
        PRESET_DOCUMENT => ROLE_DOCUMENTER,
        _ => ROLE_CODER,
    }
}

pub fn builtin_presets() -> Vec<PresetItem> {
    vec![
        PresetItem {
            id: PRESET_PLANNING.into(),
            title: "Planning".into(),
            default_role: ROLE_PLANNER.into(),
            ..Default::default()
        },
        PresetItem {
            id: PRESET_REVIEW.into(),
            title: "Review".into(),
            default_role: ROLE_REVIEWER.into(),
            ..Default::default()
        },
        PresetItem {
            id: PRESET_DEBUG.into(),
            title: "Debug".into(),
            default_role: ROLE_DEBUGGER.into(),
            ..Default::default()
        },
        PresetItem {
            id: PRESET_DOCUMENT.into(),
            title: "Document".into(),
            default_role: ROLE_DOCUMENTER.into(),
            ..Default::default()
        },
    ]
}

pub fn agents_md_chip(guides: Option<&WorkspaceGuides>) -> &'static str {
    match guides.and_then(|g| g.agents_md.as_ref()) {
        Some(_) => AGENTS_MD_PRESENT,
        None => AGENTS_MD_MISSING,
    }
}

pub fn workspace_guide_chip(guides: Option<&WorkspaceGuides>) -> &'static str {
    match guides.and_then(|g| g.workspace_guide.as_ref()) {
        Some(_) => WS_GUIDE_PRESENT,
        None => WS_GUIDE_MISSING,
    }
}

pub fn guide_preview(file: Option<&GuideFile>) -> Option<&str> {
    file.map(|f| f.content.as_str())
}

pub fn guide_content_fits(content: &str) -> bool {
    content.len() <= GUIDE_CONTENT_MAX
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_copy_is_locked() {
        assert_eq!(
            WORKSPACE_UNAVAILABLE,
            "воркспейс-гайд недоступен: host без 1.7"
        );
        assert_eq!(ROLE_LABEL, "Роль");
        assert_eq!(PRESET_LABEL, "Пресет");
        assert_eq!(PRESET_NONE, "без пресета");
        assert_eq!(GUIDE_PANE, "Гайды");
        assert_eq!(AGENTS_MD_PRESENT, "AGENTS.md есть");
        assert_eq!(AGENTS_MD_MISSING, "AGENTS.md нет");
        assert_eq!(WS_GUIDE_PRESENT, "гайд есть");
        assert_eq!(WS_GUIDE_MISSING, "гайда нет");
        assert_eq!(GLOBAL_GUIDE_LABEL, "Глобальный гайд выбора агента");
        assert_eq!(GLOBAL_GUIDE_SAVE, "Сохранить гайд");
        assert_eq!(GLOBAL_GUIDE_HINT, "как выбирать агента");
        assert_eq!(GUIDE_TOO_LONG, "гайд: не больше 65536 байт");
        assert_eq!(
            crate::rpc::METHOD_WORKSPACE_GUIDES_GET,
            "workspace.guides.get"
        );
        assert_eq!(crate::rpc::METHOD_SETTINGS_GUIDE_GET, "settings.guide.get");
        assert_eq!(crate::rpc::METHOD_SETTINGS_GUIDE_SET, "settings.guide.set");
        assert_eq!(PRESETS_UNAVAILABLE, "пресеты недоступны: host без 1.9");
        assert_eq!(PRESET_CREATE, "Создать");
        assert_eq!(PRESET_SAVE, "Сохранить");
        assert_eq!(PRESET_DELETE, "Удалить");
        assert_eq!(PRESET_NAME_LABEL, "Имя");
        assert_eq!(PRESET_NAME_HINT, "имя пресета");
        assert_eq!(PRESET_TITLE_HINT_LABEL, "Подсказка названия");
        assert_eq!(PRESET_PROMPT_LABEL, "Промпт");
        assert_eq!(PRESET_DELETE_TITLE, "Удалить пресет?");
        assert_eq!(PRESET_DELETE_BODY, "Пользовательский пресет будет удалён.");
        assert_eq!(PRESET_DELETE_OK, "Удалить");
        assert_eq!(crate::rpc::METHOD_PRESET_LIST, "preset.list");
        assert_eq!(crate::rpc::METHOD_PRESET_CREATE, "preset.create");
        assert_eq!(crate::rpc::METHOD_PRESET_UPDATE, "preset.update");
        assert_eq!(crate::rpc::METHOD_PRESET_DELETE, "preset.delete");
        assert_eq!(crate::rpc::METHOD_AGENT_UPDATE, "agent.update");
        assert_eq!(crate::rpc::WORKSPACE_METHODS.len(), 5);
        assert_eq!(crate::rpc::PRESET_CRUD_METHODS.len(), 3);
        assert!(!crate::rpc::WORKSPACE_METHODS.contains(&crate::rpc::METHOD_PRESET_CREATE));
        assert_eq!(
            ROLE_CHOICES,
            ["coder", "planner", "reviewer", "debugger", "documenter"]
        );
        assert_eq!(PRESET_CHOICES, ["planning", "review", "debug", "document"]);
        assert!(valid_role(ROLE_PLANNER));
        assert!(!valid_role("owner"));
        assert!(valid_preset(PRESET_PLANNING));
        assert!(!valid_preset("epic"));
        assert_eq!(default_role_for_preset(PRESET_PLANNING), ROLE_PLANNER);
        assert_eq!(default_role_for_preset(PRESET_REVIEW), ROLE_REVIEWER);
        assert_eq!(default_role_for_preset(PRESET_DEBUG), ROLE_DEBUGGER);
        assert_eq!(default_role_for_preset(PRESET_DOCUMENT), ROLE_DOCUMENTER);
        let presets = builtin_presets();
        assert_eq!(presets.len(), 4);
        assert_eq!(presets[0].id, PRESET_PLANNING);
        assert_eq!(presets[1].id, PRESET_REVIEW);
        assert_eq!(presets[2].id, PRESET_DEBUG);
        assert_eq!(presets[3].id, PRESET_DOCUMENT);
        assert_eq!(agents_md_chip(None), AGENTS_MD_MISSING);
        assert_eq!(workspace_guide_chip(None), WS_GUIDE_MISSING);
        assert!(guide_content_fits("ok"));
        assert!(!guide_content_fits(&"x".repeat(GUIDE_CONTENT_MAX + 1)));
    }

    #[test]
    fn guide_load_method_is_rpc_not_fs_walk() {
        assert_eq!(
            crate::rpc::METHOD_WORKSPACE_GUIDES_GET,
            "workspace.guides.get"
        );
        assert_ne!(crate::rpc::METHOD_WORKSPACE_GUIDES_GET, "files.tree");
        assert_ne!(crate::rpc::METHOD_WORKSPACE_GUIDES_GET, "files.read");
        assert!(!crate::rpc::WORKSPACE_METHODS.contains(&"files.tree"));
        assert!(!ROLE_CHOICES.iter().any(|r| *r == "search"));
        assert!(!PRESET_CHOICES.iter().any(|p| *p == "custom"));
        assert!(is_builtin_preset(PRESET_PLANNING));
        assert!(!is_builtin_preset("mine"));
    }

    #[test]
    fn preset_crud_shapes_omit_empty_and_protect_builtin() {
        let created =
            preset_create_params(" Mine ", "planner", " hint ", " do it ").expect("create");
        assert_eq!(created["name"], "Mine");
        assert_eq!(created["defaultRole"], "planner");
        assert_eq!(created["titleHint"], "hint");
        assert_eq!(created["prompt"], "do it");
        assert!(created.get("secret").is_none());
        assert!(created.get("token").is_none());
        let slim = preset_create_params("Mine", "coder", "  ", "").expect("slim");
        assert_eq!(slim, json!({ "name": "Mine", "defaultRole": "coder" }));
        assert!(preset_create_params("  ", "coder", "", "").is_none());
        assert!(preset_create_params("Mine", "owner", "", "").is_none());

        let updated = preset_update_params("p-1", "Mine", "reviewer", "", "x").expect("update");
        assert_eq!(updated["id"], "p-1");
        assert_eq!(updated["name"], "Mine");
        assert_eq!(updated["defaultRole"], "reviewer");
        assert_eq!(updated["prompt"], "x");
        assert!(updated.get("titleHint").is_none());
        assert!(preset_update_params("planning", "Mine", "coder", "", "").is_none());
        assert!(preset_update_params("", "Mine", "coder", "", "").is_none());

        assert_eq!(
            preset_delete_params("  p-1  "),
            Some(json!({ "id": "p-1" }))
        );
        assert!(preset_delete_params("planning").is_none());
        assert!(preset_delete_params("review").is_none());
        assert!(preset_delete_params("debug").is_none());
        assert!(preset_delete_params("document").is_none());
        assert!(preset_delete_params("").is_none());

        let item = parse_preset_item(&json!({
            "item": {
                "id": "p-2",
                "name": "Mine",
                "defaultRole": "debugger",
                "titleHint": "fix",
                "prompt": "debug it"
            }
        }));
        assert_eq!(item.id, "p-2");
        assert_eq!(item.name, "Mine");
        assert_eq!(item.default_role, "debugger");
        assert_eq!(item.title_hint, "fix");
        assert_eq!(item.prompt, "debug it");
        assert!(!is_builtin_preset(&item.id));
    }
}
