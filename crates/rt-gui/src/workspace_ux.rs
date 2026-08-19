//! E8 workspace GUI: AGENTS.md / selection guides, agent roles, task presets.
//! Guides come from host RPC only. No workspace walk. Host without 1.7 degrades.

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
        },
        PresetItem {
            id: PRESET_REVIEW.into(),
            title: "Review".into(),
            default_role: ROLE_REVIEWER.into(),
        },
        PresetItem {
            id: PRESET_DEBUG.into(),
            title: "Debug".into(),
            default_role: ROLE_DEBUGGER.into(),
        },
        PresetItem {
            id: PRESET_DOCUMENT.into(),
            title: "Document".into(),
            default_role: ROLE_DOCUMENTER.into(),
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
        assert_eq!(crate::rpc::METHOD_PRESET_LIST, "preset.list");
        assert_eq!(crate::rpc::METHOD_AGENT_UPDATE, "agent.update");
        assert_eq!(crate::rpc::WORKSPACE_METHODS.len(), 5);
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
    }
}
