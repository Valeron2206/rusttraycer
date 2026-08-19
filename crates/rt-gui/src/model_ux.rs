//! E7 model UX: switch harness on the same agent, named profiles, last prefs.
//! No host spawn. Host without 1.6 degrades; chat/write/pty/artifacts/a2a stay.

use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const MODEL_UNAVAILABLE: &str = "модели недоступны: host без 1.6";
pub const MODEL_LABEL: &str = "Модель";
pub const MODEL_HINT: &str = "модель";
pub const EFFORT_LABEL: &str = "Effort";
pub const EFFORT_HINT: &str = "effort";
pub const FAST_LABEL: &str = "Fast";
pub const SWITCH_BUTTON: &str = "Сменить";
pub const PROFILES_LABEL: &str = "Профили";
pub const PROFILE_HINT: &str = "выберите профиль";
pub const PROFILE_EMPTY: &str = "нет профилей";
pub const PROFILE_NAME_HINT: &str = "имя профиля";
pub const PROFILE_SAVE: &str = "Сохранить профиль";
pub const PROFILE_APPLY: &str = "Применить профиль";
pub const PROFILE_NAME_BAD: &str = "имя профиля: 1…80 символов";

pub const EFFORT_LOW: &str = "low";
pub const EFFORT_MEDIUM: &str = "medium";
pub const EFFORT_HIGH: &str = "high";
pub const PROFILE_NAME_MAX: usize = 80;

pub const EFFORT_CHOICES: &[&str] = &["", EFFORT_LOW, EFFORT_MEDIUM, EFFORT_HIGH];

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelParams {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
    #[serde(default)]
    pub fast: bool,
}

impl ModelParams {
    pub fn from_drafts(model: &str, effort: &str, fast: bool) -> Self {
        Self {
            model: trim_opt(model),
            effort: trim_opt(effort),
            fast,
        }
    }

    pub fn model_draft(&self) -> String {
        self.model.clone().unwrap_or_default()
    }

    pub fn effort_draft(&self) -> String {
        self.effort.clone().unwrap_or_default()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelPrefs {
    #[serde(flatten)]
    pub last: ModelParams,
    #[serde(default)]
    pub by_provider: BTreeMap<String, ModelParams>,
}

pub fn trim_opt(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

pub fn valid_profile_name(name: &str) -> bool {
    let n = name.chars().count();
    (1..=PROFILE_NAME_MAX).contains(&n)
}

pub fn model_prefs_path() -> PathBuf {
    crate::discovery::rusttraycer_home()
        .join("gui")
        .join("model.json")
}

pub fn load_model_prefs() -> ModelPrefs {
    load_model_prefs_from(&model_prefs_path())
}

pub fn load_model_prefs_from(path: &Path) -> ModelPrefs {
    match std::fs::read_to_string(path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => ModelPrefs::default(),
    }
}

pub fn save_model_prefs(prefs: &ModelPrefs) {
    save_model_prefs_to(&model_prefs_path(), prefs);
}

pub fn save_model_prefs_to(path: &Path, prefs: &ModelPrefs) {
    if let Some(parent) = path.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
    }
    let Ok(raw) = serde_json::to_string(prefs) else {
        return;
    };
    let _ = std::fs::write(path, raw);
}

pub fn remember_params(prefs: &mut ModelPrefs, provider: Option<&str>, params: ModelParams) {
    prefs.last = params.clone();
    if let Some(id) = provider.map(str::trim).filter(|s| !s.is_empty()) {
        prefs.by_provider.insert(id.to_string(), params);
    }
}

pub fn params_for_provider<'a>(prefs: &'a ModelPrefs, provider: &str) -> &'a ModelParams {
    prefs.by_provider.get(provider).unwrap_or(&prefs.last)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_copy_is_locked() {
        assert_eq!(MODEL_UNAVAILABLE, "модели недоступны: host без 1.6");
        assert_eq!(MODEL_LABEL, "Модель");
        assert_eq!(EFFORT_LABEL, "Effort");
        assert_eq!(FAST_LABEL, "Fast");
        assert_eq!(SWITCH_BUTTON, "Сменить");
        assert_eq!(PROFILES_LABEL, "Профили");
        assert_eq!(PROFILE_HINT, "выберите профиль");
        assert_eq!(PROFILE_EMPTY, "нет профилей");
        assert_eq!(PROFILE_NAME_HINT, "имя профиля");
        assert_eq!(PROFILE_SAVE, "Сохранить профиль");
        assert_eq!(PROFILE_APPLY, "Применить профиль");
        assert_eq!(PROFILE_NAME_BAD, "имя профиля: 1…80 символов");
        assert_eq!(crate::rpc::METHOD_AGENT_SWITCH, "agent.switch");
        assert_eq!(crate::rpc::METHOD_PROFILE_CREATE, "profile.create");
        assert_eq!(crate::rpc::METHOD_PROFILE_LIST, "profile.list");
        assert_eq!(crate::rpc::METHOD_PROFILE_GET, "profile.get");
        assert_eq!(crate::rpc::METHOD_PROFILE_UPDATE, "profile.update");
        assert_eq!(crate::rpc::METHOD_PROFILE_DELETE, "profile.delete");
        assert_eq!(crate::rpc::METHOD_PREFS_GET, "prefs.get");
        assert_eq!(crate::rpc::MODEL_METHODS.len(), 7);
        assert!(valid_profile_name("fast"));
        assert!(!valid_profile_name(""));
        assert!(!valid_profile_name(&"x".repeat(81)));
    }

    #[test]
    fn last_model_effort_fast_roundtrip() {
        let dir = std::env::temp_dir().join(format!(
            "rt-gui-e7-model-{}",
            uuid::Uuid::now_v7().as_simple()
        ));
        let path = dir.join("gui").join("model.json");
        let mut prefs = ModelPrefs::default();
        remember_params(
            &mut prefs,
            Some("cli.codex"),
            ModelParams {
                model: Some("o3".into()),
                effort: Some("high".into()),
                fast: true,
            },
        );
        save_model_prefs_to(&path, &prefs);
        let loaded = load_model_prefs_from(&path);
        assert_eq!(loaded.last.model.as_deref(), Some("o3"));
        assert_eq!(loaded.last.effort.as_deref(), Some("high"));
        assert!(loaded.last.fast);
        let for_codex = params_for_provider(&loaded, "cli.codex");
        assert_eq!(for_codex.model.as_deref(), Some("o3"));
        assert_eq!(for_codex.effort.as_deref(), Some("high"));
        assert!(for_codex.fast);
        let _ = std::fs::remove_dir_all(dir);
    }
}
