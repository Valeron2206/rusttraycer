//! Workspace/global selection guides and built-in presets. Files only, not sqlite.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use rt_protocol::{GuideFile, PresetItem, SettingsGuide};
use rt_runtime::{WireMessage, WireRole};

use crate::HostError;

pub const GUIDE_CAP_BYTES: usize = 65_536;
pub const GLOBAL_GUIDE_NAME: &str = "agent-selection-guide.md";
pub const AGENTS_MD_NAME: &str = "AGENTS.md";
pub const WORKSPACE_GUIDE_REL: &str = ".traycer/agent-selection-guide.md";

pub const ROLES: [&str; 5] = ["coder", "planner", "reviewer", "debugger", "documenter"];

#[derive(Clone, Copy)]
pub struct PresetDef {
    pub id: &'static str,
    pub title: &'static str,
    pub default_role: &'static str,
}

pub const PRESETS: [PresetDef; 4] = [
    PresetDef {
        id: "planning",
        title: "Planning",
        default_role: "planner",
    },
    PresetDef {
        id: "review",
        title: "Review",
        default_role: "reviewer",
    },
    PresetDef {
        id: "debug",
        title: "Debug",
        default_role: "debugger",
    },
    PresetDef {
        id: "document",
        title: "Document",
        default_role: "documenter",
    },
];

pub fn parse_role(role: &str) -> Result<&str, HostError> {
    ROLES
        .iter()
        .copied()
        .find(|r| *r == role)
        .ok_or_else(|| HostError::InvalidParams(format!("invalid role: {role}")))
}

pub fn parse_preset(preset: &str) -> Result<PresetDef, HostError> {
    PRESETS
        .iter()
        .copied()
        .find(|p| p.id == preset)
        .ok_or_else(|| HostError::InvalidParams(format!("invalid preset: {preset}")))
}

pub fn preset_items() -> Vec<PresetItem> {
    PRESETS
        .iter()
        .map(|p| PresetItem {
            id: p.id.to_string(),
            title: p.title.to_string(),
            default_role: p.default_role.to_string(),
        })
        .collect()
}

pub fn global_guide_path(data_dir: &Path) -> PathBuf {
    data_dir.join(GLOBAL_GUIDE_NAME)
}

pub fn agents_md_path(workspace_path: &Path) -> PathBuf {
    workspace_path.join(AGENTS_MD_NAME)
}

pub fn workspace_guide_path(workspace_path: &Path) -> PathBuf {
    workspace_path.join(WORKSPACE_GUIDE_REL)
}

fn read_capped(path: &Path) -> Option<(String, bool)> {
    let meta = fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let data = fs::read(path).ok()?;
    let truncated = data.len() > GUIDE_CAP_BYTES;
    let slice = if truncated {
        &data[..GUIDE_CAP_BYTES]
    } else {
        data.as_slice()
    };
    let content = match std::str::from_utf8(slice) {
        Ok(s) => s.to_string(),
        Err(e) => String::from_utf8_lossy(&slice[..e.valid_up_to()]).into_owned(),
    };
    Some((content, truncated))
}

pub fn read_guide_file(path: &Path) -> Option<GuideFile> {
    let (content, truncated) = read_capped(path)?;
    Some(GuideFile {
        path: path.to_string_lossy().into_owned(),
        content,
        truncated,
    })
}

pub fn settings_guide_get(data_dir: &Path) -> SettingsGuide {
    let path = global_guide_path(data_dir);
    match read_capped(&path) {
        Some((content, truncated)) => SettingsGuide {
            path: path.to_string_lossy().into_owned(),
            content,
            truncated,
        },
        None => SettingsGuide {
            path: path.to_string_lossy().into_owned(),
            content: String::new(),
            truncated: false,
        },
    }
}

pub fn settings_guide_set(data_dir: &Path, content: &str) -> Result<SettingsGuide, HostError> {
    if content.len() > GUIDE_CAP_BYTES {
        return Err(HostError::InvalidParams(format!(
            "content must be 0..{GUIDE_CAP_BYTES} bytes"
        )));
    }
    if let Err(e) = fs::create_dir_all(data_dir) {
        return Err(HostError::Internal(e.to_string()));
    }
    let path = global_guide_path(data_dir);
    let tmp = data_dir.join("agent-selection-guide.md.tmp");
    let write = (|| {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(content.as_bytes())?;
        f.sync_all()?;
        fs::rename(&tmp, &path)?;
        Ok::<(), std::io::Error>(())
    })();
    if let Err(e) = write {
        let _ = fs::remove_file(&tmp);
        return Err(HostError::Internal(e.to_string()));
    }
    Ok(SettingsGuide {
        path: path.to_string_lossy().into_owned(),
        content: content.to_string(),
        truncated: false,
    })
}

pub fn role_prefix(role: &str) -> String {
    format!("You are a {role}.")
}

pub fn inject_preamble(
    data_dir: &Path,
    workspace_path: &Path,
    role: &str,
    transcript: Vec<WireMessage>,
) -> Vec<WireMessage> {
    let mut out = Vec::new();
    if let Some(file) = read_guide_file(&agents_md_path(workspace_path)) {
        out.push(WireMessage {
            role: WireRole::System,
            content: file.content,
        });
    }
    if let Some(file) = read_guide_file(&global_guide_path(data_dir)) {
        if !file.content.is_empty() {
            out.push(WireMessage {
                role: WireRole::System,
                content: file.content,
            });
        }
    }
    if let Some(file) = read_guide_file(&workspace_guide_path(workspace_path)) {
        out.push(WireMessage {
            role: WireRole::System,
            content: file.content,
        });
    }
    out.push(WireMessage {
        role: WireRole::System,
        content: role_prefix(role),
    });
    out.extend(transcript);
    out
}
