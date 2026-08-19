//! E2 ladder GUI types and copy. No host spawn.

use crate::rpc::PolicyOk;

pub const PICKER_LABEL: &str = "Провайдер";
pub const PICKER_EMPTY: &str = "нет провайдеров (doctor пуст)";
pub const PICKER_HINT: &str = "выберите провайдера";
pub const PICKER_UNAVAILABLE: &str = "недоступен — host всё равно примет";
pub const CAPS_LABEL: &str = "Возможности";
pub const POLICY_LABEL: &str = "Разрешения";
pub const LADDER_UNAVAILABLE: &str = "лестница недоступна: host без 1.1";
pub const WRITE_UNAVAILABLE: &str = "запись недоступна: host без 1.2";

pub const PUSH_BUTTON: &str = "Push";
pub const PUSH_CONFIRM_TITLE: &str = "Отправить в remote?";
pub const PUSH_CONFIRM_BODY: &str =
    "Push вызывает system git. Креды — helper или gh на машине. Поля пароля и токена нет.";
pub const PUSH_CONFIRM_OK: &str = "Отправить";

pub const COMMIT_BUTTON: &str = "Закоммитить";
pub const COMMIT_HINT: &str = "сообщение коммита";
pub const STAGE_BUTTON: &str = "В индекс";
pub const UNSTAGE_BUTTON: &str = "Убрать из индекса";
pub const REVERT_BUTTON: &str = "Вернуть";
pub const OPEN_IN_EDITOR: &str = "Открыть в редакторе";
pub const GIT_IDENTITY_HINT: &str = "настрой `git config user.email`";
pub const GIT_AUTH_HINT: &str = "войди в git/gh на машине";

pub const YOLO_CONFIRM_TITLE: &str = "Включить Yolo?";
pub const YOLO_CONFIRM_BODY: &str =
    "Yolo обходит лестницу разрешений. Пока флаг включён, exec и edit идут без карточки.";
pub const YOLO_CONFIRM_OK: &str = "Включить Yolo";
pub const YOLO_BANNER: &str = "Yolo — лестница разрешений не вызывается.";
pub const YOLO_OFF: &str = "Выключить Yolo";
pub const YOLO_ON_BUTTON: &str = "Yolo…";

pub const APPROVAL_TITLE: &str = "Нужно разрешение";
pub const APPROVAL_ONCE: &str = "Разрешить один раз";
pub const APPROVAL_ALWAYS: &str = "Всегда (этот агент)";
pub const APPROVAL_DENY: &str = "Отказать";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyMode {
    Ask,
    AllowAlways,
    Deny,
}

impl PolicyMode {
    pub fn as_wire(self) -> &'static str {
        match self {
            Self::Ask => "ask",
            Self::AllowAlways => "allow-always",
            Self::Deny => "deny",
        }
    }

    pub fn from_wire(s: &str) -> Self {
        match s {
            "allow-always" => Self::AllowAlways,
            "deny" => Self::Deny,
            _ => Self::Ask,
        }
    }

    pub fn label_ru(self) -> &'static str {
        match self {
            Self::Ask => "Спросить",
            Self::AllowAlways => "Всегда",
            Self::Deny => "Запретить",
        }
    }

    pub const ALL: [Self; 3] = [Self::Ask, Self::AllowAlways, Self::Deny];
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentPolicy {
    pub mode: PolicyMode,
    pub scope: String,
    pub yolo: bool,
    pub source: String,
}

impl Default for AgentPolicy {
    fn default() -> Self {
        Self {
            mode: PolicyMode::Ask,
            scope: "agent".into(),
            yolo: false,
            source: "default".into(),
        }
    }
}

impl From<PolicyOk> for AgentPolicy {
    fn from(ok: PolicyOk) -> Self {
        Self {
            mode: PolicyMode::from_wire(&ok.mode),
            scope: if ok.scope.is_empty() {
                "agent".into()
            } else {
                ok.scope
            },
            yolo: ok.yolo,
            source: ok.source,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingApproval {
    pub approval_id: String,
    pub agent_id: String,
    pub task_id: String,
    pub kind: String,
    pub summary: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaneKind {
    Canvas,
    Git,
    Files,
    Host,
}

impl PaneKind {
    pub fn as_id(self) -> &'static str {
        match self {
            Self::Canvas => "canvas",
            Self::Git => "git",
            Self::Files => "files",
            Self::Host => "host",
        }
    }

    pub fn from_id(s: &str) -> Option<Self> {
        match s {
            "canvas" => Some(Self::Canvas),
            "git" => Some(Self::Git),
            "files" => Some(Self::Files),
            "host" => Some(Self::Host),
            _ => None,
        }
    }

    pub fn label_ru(self) -> &'static str {
        match self {
            Self::Canvas => "Канвас",
            Self::Git => "Git",
            Self::Files => "Файлы",
            Self::Host => "Host",
        }
    }

    pub const ALL: [Self; 4] = [Self::Canvas, Self::Git, Self::Files, Self::Host];
}

#[derive(Clone, Debug, PartialEq)]
pub struct SplitLayout {
    pub left: PaneKind,
    pub right: PaneKind,
    pub left_width: f32,
}

impl Default for SplitLayout {
    fn default() -> Self {
        Self {
            left: PaneKind::Canvas,
            right: PaneKind::Git,
            left_width: 520.0,
        }
    }
}

impl SplitLayout {
    pub fn from_json(raw: &str) -> Self {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
            return Self::default();
        };
        let left = value
            .get("left")
            .and_then(|v| v.as_str())
            .and_then(PaneKind::from_id)
            .unwrap_or(PaneKind::Canvas);
        let right = value
            .get("right")
            .and_then(|v| v.as_str())
            .and_then(PaneKind::from_id)
            .unwrap_or(PaneKind::Git);
        let left_width = value
            .get("leftWidth")
            .and_then(|v| v.as_f64())
            .map(|w| w as f32)
            .filter(|w| w.is_finite() && *w >= 180.0)
            .unwrap_or(520.0);
        Self {
            left,
            right,
            left_width,
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::json!({
            "left": self.left.as_id(),
            "right": self.right.as_id(),
            "leftWidth": self.left_width,
        })
        .to_string()
    }
}

pub fn layout_path() -> std::path::PathBuf {
    crate::discovery::rusttraycer_home()
        .join("gui")
        .join("layout.json")
}

pub fn load_split_layout() -> SplitLayout {
    let path = layout_path();
    match std::fs::read_to_string(path) {
        Ok(raw) => SplitLayout::from_json(&raw),
        Err(_) => SplitLayout::default(),
    }
}

pub fn save_split_layout(layout: &SplitLayout) {
    let path = layout_path();
    if let Some(parent) = path.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
    }
    let _ = std::fs::write(path, layout.to_json());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_is_ask_not_full() {
        let p = AgentPolicy::default();
        assert_eq!(p.mode, PolicyMode::Ask);
        assert!(!p.yolo);
        assert_eq!(p.mode.label_ru(), "Спросить");
        assert_eq!(PolicyMode::from_wire("full-access"), PolicyMode::Ask);
        assert_eq!(
            PolicyMode::from_wire("allow-always"),
            PolicyMode::AllowAlways
        );
    }

    #[test]
    fn split_layout_roundtrip_and_unknown_falls_back() {
        let layout = SplitLayout {
            left: PaneKind::Canvas,
            right: PaneKind::Git,
            left_width: 400.0,
        };
        let parsed = SplitLayout::from_json(&layout.to_json());
        assert_eq!(parsed.left, PaneKind::Canvas);
        assert_eq!(parsed.right, PaneKind::Git);
        assert_eq!(parsed.left_width, 400.0);
        let bad = SplitLayout::from_json(r#"{"left":"search","right":"tiles"}"#);
        assert_eq!(bad.left, PaneKind::Canvas);
        assert_eq!(bad.right, PaneKind::Git);
    }

    #[test]
    fn ui_copy_is_locked() {
        assert_eq!(PICKER_LABEL, "Провайдер");
        assert_eq!(PICKER_EMPTY, "нет провайдеров (doctor пуст)");
        assert_eq!(PICKER_HINT, "выберите провайдера");
        assert_eq!(YOLO_CONFIRM_TITLE, "Включить Yolo?");
        assert_eq!(
            YOLO_CONFIRM_BODY,
            "Yolo обходит лестницу разрешений. Пока флаг включён, exec и edit идут без карточки."
        );
        assert_eq!(YOLO_CONFIRM_OK, "Включить Yolo");
        assert_eq!(YOLO_BANNER, "Yolo — лестница разрешений не вызывается.");
        assert_eq!(APPROVAL_TITLE, "Нужно разрешение");
        assert_eq!(APPROVAL_ONCE, "Разрешить один раз");
        assert_eq!(APPROVAL_ALWAYS, "Всегда (этот агент)");
        assert_eq!(APPROVAL_DENY, "Отказать");
        assert_eq!(WRITE_UNAVAILABLE, "запись недоступна: host без 1.2");
        assert_eq!(PUSH_CONFIRM_TITLE, "Отправить в remote?");
        assert_eq!(
            PUSH_CONFIRM_BODY,
            "Push вызывает system git. Креды — helper или gh на машине. Поля пароля и токена нет."
        );
        assert_eq!(PUSH_CONFIRM_OK, "Отправить");
        assert_eq!(PUSH_BUTTON, "Push");
        assert_eq!(REVERT_BUTTON, "Вернуть");
        assert_eq!(OPEN_IN_EDITOR, "Открыть в редакторе");
        assert_eq!(STAGE_BUTTON, "В индекс");
        assert_eq!(UNSTAGE_BUTTON, "Убрать из индекса");
        assert_eq!(COMMIT_BUTTON, "Закоммитить");
        assert_eq!(GIT_IDENTITY_HINT, "настрой `git config user.email`");
        assert_eq!(GIT_AUTH_HINT, "войди в git/gh на машине");
    }
}
