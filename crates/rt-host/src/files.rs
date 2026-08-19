//! Read-only `files.tree` / `files.read` (protocol-v0 1.0). No file tables.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use serde::Serialize;
use serde_json::{json, Value};

use rt_storage::Store;

use crate::HostError;

pub const MAX_FILE_BYTES: u64 = 262_144;
pub const BINARY_SCAN_BYTES: usize = 8 * 1024;
pub const DEFAULT_DEPTH: u64 = 2;
pub const DEFAULT_MAX_ENTRIES: u64 = 500;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub size: Option<u64>,
    pub modified_at: Option<String>,
}

fn modified_at(meta: &fs::Metadata) -> Option<String> {
    let st = meta.modified().ok()?;
    let dt = chrono::DateTime::<Utc>::from(st);
    Some(dt.to_rfc3339_opts(SecondsFormat::Secs, true))
}

fn is_inside(root: &Path, path: &Path) -> bool {
    path == root || path.starts_with(root)
}

/// Join + canonicalize must stay inside the workspace.
/// `..`, symlink-out, absolute path → invalid_params.
/// Missing path → not_found.
pub fn resolve_inside(root: &Path, rel: &str) -> Result<PathBuf, HostError> {
    if rel.starts_with('/') || Path::new(rel).is_absolute() {
        return Err(HostError::InvalidParams(
            "path must be relative to the workspace".into(),
        ));
    }
    if rel.split('/').any(|c| c == "..") {
        return Err(HostError::InvalidParams(
            "path must not contain '..'".into(),
        ));
    }
    let root_canon = root
        .canonicalize()
        .map_err(|_| HostError::NotFound("workspace path is missing".into()))?;

    let mut joined = root_canon.clone();
    if !rel.is_empty() && rel != "." {
        for component in rel.split('/') {
            if component.is_empty() || component == "." {
                continue;
            }
            if component.contains('\0') {
                return Err(HostError::InvalidParams("invalid path".into()));
            }
            joined.push(component);
        }
    }

    match joined.canonicalize() {
        Ok(canon) => {
            if !is_inside(&root_canon, &canon) {
                return Err(HostError::InvalidParams(
                    "path escapes the workspace".into(),
                ));
            }
            Ok(canon)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Err(HostError::NotFound(format!("path not found: {rel}")))
        }
        Err(e) => Err(HostError::InvalidParams(format!(
            "cannot resolve path: {e}"
        ))),
    }
}

fn rel_of(root: &Path, canon: &Path) -> String {
    if canon == root {
        return String::new();
    }
    canon
        .strip_prefix(root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default()
}

fn entry_for(root: &Path, canon: &Path) -> Result<FileEntry, HostError> {
    let meta = fs::metadata(canon).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            HostError::NotFound(rel_of(root, canon))
        } else {
            HostError::Internal(e.to_string())
        }
    })?;
    let rel = rel_of(root, canon);
    let name = if rel.is_empty() {
        String::new()
    } else {
        canon
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| rel.clone())
    };
    if meta.is_dir() {
        Ok(FileEntry {
            name,
            path: rel,
            kind: "dir".into(),
            size: None,
            modified_at: modified_at(&meta),
        })
    } else {
        Ok(FileEntry {
            name,
            path: rel,
            kind: "file".into(),
            size: Some(meta.len()),
            modified_at: modified_at(&meta),
        })
    }
}

fn read_children_sorted(
    root: &Path,
    dir: &Path,
    parent_rel: &str,
) -> Vec<(FileEntry, PathBuf, bool)> {
    let mut kids = Vec::new();
    let rd = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return kids,
    };
    for ent in rd.flatten() {
        let name = ent.file_name();
        let name = name.to_string_lossy();
        let child_rel = if parent_rel.is_empty() {
            name.to_string()
        } else {
            format!("{parent_rel}/{name}")
        };
        let path = ent.path();
        let canon = match path.canonicalize() {
            Ok(c) => c,
            Err(_) => continue,
        };
        if !is_inside(root, &canon) {
            continue;
        }
        let meta = match fs::metadata(&canon) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let is_dir = meta.is_dir();
        let entry = FileEntry {
            name: name.into_owned(),
            path: child_rel,
            kind: if is_dir { "dir".into() } else { "file".into() },
            size: if is_dir { None } else { Some(meta.len()) },
            modified_at: modified_at(&meta),
        };
        kids.push((entry, canon, is_dir));
    }
    kids.sort_by(|a, b| match (a.2, b.2) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.0.name.as_bytes().cmp(b.0.name.as_bytes()),
    });
    kids
}

fn walk(
    root: &Path,
    dir: &Path,
    rel: &str,
    depth: u64,
    max_entries: u64,
    items: &mut Vec<FileEntry>,
    truncated: &mut bool,
) {
    if *truncated || depth < 1 {
        return;
    }
    for (entry, canon, is_dir) in read_children_sorted(root, dir, rel) {
        if items.len() as u64 >= max_entries {
            *truncated = true;
            return;
        }
        let child_rel = entry.path.clone();
        items.push(entry);
        if is_dir && depth > 1 {
            walk(
                root,
                &canon,
                &child_rel,
                depth - 1,
                max_entries,
                items,
                truncated,
            );
            if *truncated {
                return;
            }
        }
    }
}

fn parse_positive(v: Option<&Value>, default: u64, name: &str) -> Result<u64, HostError> {
    match v {
        None => Ok(default),
        Some(Value::Number(n)) => {
            if let Some(i) = n.as_u64() {
                if i >= 1 {
                    return Ok(i);
                }
            }
            if let Some(i) = n.as_i64() {
                if i >= 1 {
                    return Ok(i as u64);
                }
            }
            Err(HostError::InvalidParams(format!(
                "{name} must be an integer >= 1"
            )))
        }
        Some(_) => Err(HostError::InvalidParams(format!(
            "{name} must be an integer >= 1"
        ))),
    }
}

fn require_workspace(
    store: &Store,
    workspace_id: &str,
) -> Result<rt_storage::Workspace, HostError> {
    if uuid::Uuid::parse_str(workspace_id).is_err() {
        return Err(HostError::InvalidParams("invalid workspaceId".into()));
    }
    store
        .workspace_get(workspace_id)?
        .ok_or_else(|| HostError::NotFound(format!("workspace {workspace_id}")))
}

fn optional_worktree_id(params: &Value) -> Result<Option<&str>, HostError> {
    match params.get("worktreeId") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.as_str())),
        Some(_) => Err(HostError::InvalidParams(
            "worktreeId must be a string".into(),
        )),
    }
}

fn walk_root(
    store: &Store,
    ws: &rt_storage::Workspace,
    worktree_id: Option<&str>,
) -> Result<PathBuf, HostError> {
    match worktree_id {
        None => Ok(PathBuf::from(&ws.path)),
        Some(id) => {
            let wt = store
                .worktree_get(id)?
                .ok_or_else(|| HostError::NotFound(format!("worktree {id}")))?;
            if wt.workspace_id != ws.id {
                return Err(HostError::NotFound(format!("worktree {id}")));
            }
            Ok(PathBuf::from(wt.path))
        }
    }
}

pub fn files_tree(store: &Store, params: &Value) -> Result<Value, HostError> {
    if !params.is_object() {
        return Err(HostError::InvalidParams("params must be an object".into()));
    }
    let workspace_id = params
        .get("workspaceId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| HostError::InvalidParams("workspaceId is required".into()))?;
    let ws = require_workspace(store, workspace_id)?;
    let rel = match params.get("path") {
        None => "",
        Some(Value::Null) => "",
        Some(Value::String(s)) => s.as_str(),
        Some(_) => return Err(HostError::InvalidParams("path must be a string".into())),
    };
    let depth = parse_positive(params.get("depth"), DEFAULT_DEPTH, "depth")?;
    let max_entries = parse_positive(params.get("maxEntries"), DEFAULT_MAX_ENTRIES, "maxEntries")?;
    let wt_id = optional_worktree_id(params)?;

    let root = walk_root(store, &ws, wt_id)?;
    let target = resolve_inside(&root, rel)?;
    let root_canon = root
        .canonicalize()
        .map_err(|_| HostError::NotFound("workspace path is missing".into()))?;

    if target.is_file() {
        let item = entry_for(&root_canon, &target)?;
        return Ok(json!({ "items": [item], "truncated": false }));
    }
    if !target.is_dir() {
        return Err(HostError::NotFound(rel.to_string()));
    }

    let start_rel = rel_of(&root_canon, &target);
    let mut items = Vec::new();
    let mut truncated = false;
    walk(
        &root_canon,
        &target,
        &start_rel,
        depth,
        max_entries,
        &mut items,
        &mut truncated,
    );
    Ok(json!({ "items": items, "truncated": truncated }))
}

pub fn files_read(store: &Store, params: &Value) -> Result<Value, HostError> {
    if !params.is_object() {
        return Err(HostError::InvalidParams("params must be an object".into()));
    }
    let workspace_id = params
        .get("workspaceId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| HostError::InvalidParams("workspaceId is required".into()))?;
    let rel = params
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| HostError::InvalidParams("path is required".into()))?;
    if rel.is_empty() {
        return Err(HostError::InvalidParams("path must be nonempty".into()));
    }
    let ws = require_workspace(store, workspace_id)?;
    let wt_id = optional_worktree_id(params)?;
    let root = walk_root(store, &ws, wt_id)?;
    let target = resolve_inside(&root, rel)?;
    let root_canon = root
        .canonicalize()
        .map_err(|_| HostError::NotFound("workspace path is missing".into()))?;

    if target.is_dir() {
        return Err(HostError::InvalidParams("path is a directory".into()));
    }
    if !target.is_file() {
        return Err(HostError::NotFound(rel.to_string()));
    }

    let meta = fs::metadata(&target).map_err(|_| HostError::NotFound(rel.to_string()))?;
    if meta.len() > MAX_FILE_BYTES {
        return Err(HostError::FileTooLarge("file exceeds 256 KiB".into()));
    }
    let data = fs::read(&target).map_err(|e| HostError::Internal(e.to_string()))?;
    if data.len() as u64 > MAX_FILE_BYTES {
        return Err(HostError::FileTooLarge("file exceeds 256 KiB".into()));
    }
    let scan = data.len().min(BINARY_SCAN_BYTES);
    if data[..scan].contains(&0) {
        return Err(HostError::FileBinary("NUL in first 8 KiB".into()));
    }
    let content =
        String::from_utf8(data).map_err(|_| HostError::FileBinary("invalid UTF-8".into()))?;
    let path = rel_of(&root_canon, &target);
    Ok(json!({
        "path": path,
        "content": content,
        "truncated": false,
        "encoding": "utf8",
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rt_storage::{new_id, Store};
    use tempfile::tempdir;

    fn seeded() -> (tempfile::TempDir, Store, String, PathBuf) {
        let tmp = tempdir().unwrap();
        let store = Store::open(tmp.path().join("host.db")).unwrap();
        store.migrate().unwrap();
        store.host_insert_if_absent(&new_id(), "h").unwrap();
        let root = tmp.path().join("proj");
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(root.join("src")).unwrap();
        std::fs::write(root.join("README.md"), b"# hi\n").unwrap();
        std::fs::write(root.join("src").join("main.rs"), b"fn main() {}\n").unwrap();
        std::fs::write(root.join("src").join("lib.rs"), b"// lib\n").unwrap();
        std::fs::write(root.join("a_file"), b"a").unwrap();
        std::fs::create_dir(root.join("b_dir")).unwrap();
        std::fs::write(root.join("b_dir").join("z.txt"), b"z").unwrap();
        std::fs::write(root.join("b_dir").join("a.txt"), b"a").unwrap();
        let ws = store
            .workspace_add(root.to_str().expect("utf8 path"), "proj")
            .unwrap();
        (tmp, store, ws.id, root)
    }

    #[test]
    fn tree_from_root_dirs_then_files_dfs() {
        let (_t, store, id, _) = seeded();
        let v = files_tree(
            &store,
            &json!({ "workspaceId": id, "depth": 2, "maxEntries": 50 }),
        )
        .unwrap();
        let names: Vec<String> = v["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["path"].as_str().unwrap().to_string())
            .collect();
        // dirs first (b_dir, src), then files (README.md, a_file); DFS into dirs
        assert_eq!(names[0], "b_dir");
        assert_eq!(names[1], "b_dir/a.txt");
        assert_eq!(names[2], "b_dir/z.txt");
        assert_eq!(names[3], "src");
        assert!(names.contains(&"src/lib.rs".into()));
        assert!(names.contains(&"src/main.rs".into()));
        assert!(names.contains(&"README.md".into()));
        assert!(names.contains(&"a_file".into()));
        assert_eq!(v["truncated"], false);
    }

    #[test]
    fn tree_depth_one_skips_grandchildren() {
        let (_t, store, id, _) = seeded();
        let v = files_tree(&store, &json!({ "workspaceId": id, "depth": 1 })).unwrap();
        let names: Vec<_> = v["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["path"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"src"));
        assert!(names.contains(&"b_dir"));
        assert!(!names.iter().any(|p| p.contains('/')));
    }

    #[test]
    fn tree_file_path_is_single_entry() {
        let (_t, store, id, _) = seeded();
        let v = files_tree(
            &store,
            &json!({ "workspaceId": id, "path": "README.md", "depth": 9 }),
        )
        .unwrap();
        let items = v["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["name"], "README.md");
        assert_eq!(items[0]["kind"], "file");
        assert_eq!(items[0]["path"], "README.md");
    }

    #[test]
    fn tree_truncated_at_max_entries() {
        let (_t, store, id, _) = seeded();
        let v = files_tree(&store, &json!({ "workspaceId": id, "maxEntries": 2 })).unwrap();
        assert_eq!(v["items"].as_array().unwrap().len(), 2);
        assert_eq!(v["truncated"], true);
    }

    #[test]
    fn tree_escape_and_missing() {
        let (_t, store, id, _) = seeded();
        let err = files_tree(&store, &json!({ "workspaceId": id, "path": ".." })).unwrap_err();
        assert_eq!(err.code(), "invalid_params");
        let err = files_tree(&store, &json!({ "workspaceId": id, "path": "/etc" })).unwrap_err();
        assert_eq!(err.code(), "invalid_params");
        let err = files_tree(&store, &json!({ "workspaceId": id, "path": "no-such" })).unwrap_err();
        assert_eq!(err.code(), "not_found");
        let err = files_tree(
            &store,
            &json!({ "workspaceId": "0191f0c6-7c2a-7c11-8000-6f0c1a2b3c4d" }),
        )
        .unwrap_err();
        assert_eq!(err.code(), "not_found");
    }

    #[test]
    fn read_ok_and_errors() {
        let (_t, store, id, root) = seeded();
        let v = files_read(&store, &json!({ "workspaceId": id, "path": "README.md" })).unwrap();
        assert_eq!(v["path"], "README.md");
        assert_eq!(v["content"], "# hi\n");
        assert_eq!(v["truncated"], false);
        assert_eq!(v["encoding"], "utf8");

        let err = files_read(&store, &json!({ "workspaceId": id, "path": "src" })).unwrap_err();
        assert_eq!(err.code(), "invalid_params");

        let err =
            files_read(&store, &json!({ "workspaceId": id, "path": "missing.txt" })).unwrap_err();
        assert_eq!(err.code(), "not_found");

        let err = files_read(&store, &json!({ "workspaceId": id, "path": "" })).unwrap_err();
        assert_eq!(err.code(), "invalid_params");

        std::fs::write(root.join("nul.bin"), b"hello\0world").unwrap();
        let err = files_read(&store, &json!({ "workspaceId": id, "path": "nul.bin" })).unwrap_err();
        assert_eq!(err.code(), "file_binary");

        std::fs::write(root.join("bad.txt"), [0xff, 0xfe, b'x']).unwrap();
        let err = files_read(&store, &json!({ "workspaceId": id, "path": "bad.txt" })).unwrap_err();
        assert_eq!(err.code(), "file_binary");

        let big = vec![b'a'; (MAX_FILE_BYTES as usize) + 1];
        std::fs::write(root.join("big.txt"), &big).unwrap();
        let err = files_read(&store, &json!({ "workspaceId": id, "path": "big.txt" })).unwrap_err();
        assert_eq!(err.code(), "file_too_large");
    }

    #[test]
    fn files_read_one_mib_is_file_too_large() {
        let (_t, store, id, root) = seeded();
        let big = vec![b'x'; 1024 * 1024 + 1];
        std::fs::write(root.join("huge.txt"), &big).unwrap();
        let err =
            files_read(&store, &json!({ "workspaceId": id, "path": "huge.txt" })).unwrap_err();
        assert_eq!(err.code(), "file_too_large");
    }

    #[test]
    fn symlink_out_is_invalid_params() {
        let (_t, store, id, root) = seeded();
        let outside = _t.path().join("outside");
        std::fs::create_dir(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("leak")).unwrap();
        let err = files_tree(&store, &json!({ "workspaceId": id, "path": "leak" })).unwrap_err();
        assert_eq!(err.code(), "invalid_params");
        let err = files_read(&store, &json!({ "workspaceId": id, "path": "leak" })).unwrap_err();
        assert_eq!(err.code(), "invalid_params");
    }
}
