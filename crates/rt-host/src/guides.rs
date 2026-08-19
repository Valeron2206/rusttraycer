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

/// True when `path` is `root` or a descendant (component-wise, no string prefix trap).
pub fn path_is_within(root: &Path, path: &Path) -> bool {
    path == root || path.starts_with(root)
}

fn canon_or_owned(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// Start directory for the AGENTS.md walk.
///
/// Attached file → parent; attached directory (cwd) → itself.
/// Else worktree if it sits inside the workspace.
/// Else workspace root. Never walks above the workspace root.
pub fn walk_start(
    attached: Option<&Path>,
    worktree: Option<&Path>,
    workspace_root: &Path,
) -> PathBuf {
    let root = canon_or_owned(workspace_root);
    if let Some(p) = attached {
        let p = canon_or_owned(p);
        let start = if p.is_dir() {
            p
        } else {
            p.parent().map(Path::to_path_buf).unwrap_or(p)
        };
        let start = canon_or_owned(&start);
        if path_is_within(&root, &start) {
            return start;
        }
        return root;
    }
    if let Some(wt) = worktree {
        let wt = canon_or_owned(wt);
        if path_is_within(&root, &wt) {
            return if wt.is_dir() {
                wt
            } else {
                wt.parent().map(Path::to_path_buf).unwrap_or(wt)
            };
        }
    }
    root
}

/// Nearest-first AGENTS.md files from `start` up to (and including) workspace root.
/// Does not recurse into children. Does not walk above root.
pub fn collect_nested_agents_md(start: &Path, workspace_root: &Path) -> Vec<GuideFile> {
    let root = canon_or_owned(workspace_root);
    let mut cur = canon_or_owned(start);
    if !path_is_within(&root, &cur) {
        cur = root.clone();
    }
    let mut out = Vec::new();
    loop {
        if let Some(file) = read_guide_file(&cur.join(AGENTS_MD_NAME)) {
            out.push(file);
        }
        if cur == root {
            break;
        }
        match cur.parent() {
            Some(parent) if path_is_within(&root, parent) => {
                cur = parent.to_path_buf();
            }
            _ => break,
        }
    }
    out
}

pub fn inject_preamble(
    data_dir: &Path,
    workspace_path: &Path,
    start: &Path,
    role: &str,
    transcript: Vec<WireMessage>,
) -> Vec<WireMessage> {
    let mut out = Vec::new();
    for file in collect_nested_agents_md(start, workspace_path) {
        if file.content.is_empty() {
            continue;
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn walk_collects_nearest_first_and_stops_at_root() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path().join("ws");
        let pkg = ws.join("pkg");
        let src = pkg.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(ws.join("AGENTS.md"), "ROOT_TEXT").unwrap();
        fs::write(pkg.join("AGENTS.md"), "PKG_TEXT").unwrap();
        fs::write(src.join("lib.rs"), "fn x() {}").unwrap();

        let start = walk_start(Some(&src.join("lib.rs")), None, &ws);
        assert_eq!(start, src);
        let files = collect_nested_agents_md(&start, &ws);
        let bodies: Vec<_> = files.iter().map(|f| f.content.as_str()).collect();
        assert_eq!(bodies, ["PKG_TEXT", "ROOT_TEXT"]);

        let from_cwd = walk_start(Some(&pkg), None, &ws);
        assert_eq!(from_cwd, pkg);
        let files = collect_nested_agents_md(&from_cwd, &ws);
        let bodies: Vec<_> = files.iter().map(|f| f.content.as_str()).collect();
        assert_eq!(bodies, ["PKG_TEXT", "ROOT_TEXT"]);

        let root_only = walk_start(None, None, &ws);
        assert_eq!(root_only, ws.canonicalize().unwrap());
        let files = collect_nested_agents_md(&root_only, &ws);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].content, "ROOT_TEXT");
    }

    #[test]
    fn walk_does_not_leave_workspace() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path().join("ws");
        fs::create_dir_all(&ws).unwrap();
        fs::write(ws.join("AGENTS.md"), "ROOT_TEXT").unwrap();
        fs::write(tmp.path().join("AGENTS.md"), "OUTSIDE").unwrap();
        let start = walk_start(Some(tmp.path()), None, &ws);
        assert_eq!(start, ws.canonicalize().unwrap());
        let files = collect_nested_agents_md(&start, &ws);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].content, "ROOT_TEXT");
    }

    #[test]
    fn worktree_inside_workspace_is_start() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path().join("ws");
        let pkg = ws.join("pkg");
        fs::create_dir_all(&pkg).unwrap();
        fs::write(ws.join("AGENTS.md"), "ROOT_TEXT").unwrap();
        fs::write(pkg.join("AGENTS.md"), "PKG_TEXT").unwrap();
        let start = walk_start(None, Some(&pkg), &ws);
        assert_eq!(start, pkg.canonicalize().unwrap());
        let files = collect_nested_agents_md(&start, &ws);
        assert_eq!(files[0].content, "PKG_TEXT");
        assert_eq!(files[1].content, "ROOT_TEXT");
    }
}
