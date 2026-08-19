//! Git worktree + `git.status` / `git.diff` via the `git` CLI. No git2.

use std::path::{Path, PathBuf};
use std::process::Command;

use rt_protocol::{GitDiff, GitDiffFile, GitStatus, GitStatusEntry, Worktree};
use serde_json::Value;

use crate::service::HostService;
use crate::{HostError, Result};

const STATUS_CAP: usize = 500;
const DIFF_MAX_BYTES: usize = 256 * 1024;

impl HostService {
    pub fn worktree_ensure(&self, agent_id: &str) -> Result<Worktree> {
        let agent = self
            .store
            .agent_get(agent_id)?
            .ok_or_else(|| HostError::NotFound(format!("agent {agent_id}")))?;
        if let Some(existing) = self.store.worktree_get_by_agent(agent_id)? {
            return Ok(to_proto(existing));
        }

        let task = self
            .store
            .task_get(&agent.task_id)?
            .ok_or_else(|| HostError::NotFound(format!("task {}", agent.task_id)))?;
        let ws_id = task
            .workspace_ids
            .first()
            .ok_or_else(|| HostError::Internal("task has no workspace".into()))?;
        let workspace = self
            .store
            .workspace_get(ws_id)?
            .ok_or_else(|| HostError::NotFound(format!("workspace {ws_id}")))?;
        let ws_path = PathBuf::from(&workspace.path);
        require_git(&ws_path)?;

        let dest = self.data_dir.join("worktrees").join(agent_id);
        let branch = format!("rt/{}", short_agent_id(agent_id));
        git_worktree_add(&ws_path, &dest, &branch)?;
        let canon = dest
            .canonicalize()
            .map_err(|e| HostError::Internal(format!("worktree path: {e}")))?;
        let path = canon
            .to_str()
            .ok_or_else(|| HostError::Internal("worktree path is not utf-8".into()))?
            .to_string();

        let row = self
            .store
            .worktree_insert(&workspace.id, agent_id, &path, &branch)?;
        self.store.agent_set_run_location(agent_id, "worktree")?;
        Ok(to_proto(row))
    }

    pub fn worktree_get(&self, agent_id: &str) -> Result<Worktree> {
        if self.store.agent_get(agent_id)?.is_none() {
            return Err(HostError::NotFound(format!("agent {agent_id}")));
        }
        self.store
            .worktree_get_by_agent(agent_id)?
            .map(to_proto)
            .ok_or_else(|| HostError::NotFound(format!("worktree for agent {agent_id}")))
    }

    pub fn worktree_list(&self, workspace_id: &str) -> Result<Vec<Worktree>> {
        if self.store.workspace_get(workspace_id)?.is_none() {
            return Err(HostError::NotFound(format!("workspace {workspace_id}")));
        }
        Ok(self
            .store
            .worktree_list(workspace_id)?
            .into_iter()
            .map(to_proto)
            .collect())
    }

    pub fn git_status(&self, params: &Value) -> Result<GitStatus> {
        let root = git_root(&self.store, params)?;
        require_git(&root)?;
        run_git_status(&root)
    }

    pub fn git_diff(&self, params: &Value) -> Result<GitDiff> {
        let root = git_root(&self.store, params)?;
        require_git(&root)?;
        let path = optional_rel_path(params)?;
        run_git_diff(&root, path)
    }
}

fn to_proto(w: rt_storage::Worktree) -> Worktree {
    Worktree {
        id: w.id,
        workspace_id: w.workspace_id,
        agent_id: w.agent_id,
        path: w.path,
        branch: w.branch,
        created_at: w.created_at,
    }
}

fn short_agent_id(id: &str) -> String {
    id.chars()
        .filter(|c| c.is_ascii_hexdigit())
        .take(8)
        .collect()
}

fn require_str<'a>(params: &'a Value, field: &str) -> Result<&'a str> {
    match params.get(field) {
        None | Some(Value::Null) => Err(HostError::InvalidParams(format!("{field} is required"))),
        Some(Value::String(s)) => {
            if s.is_empty() {
                return Err(HostError::InvalidParams(format!("{field} is required")));
            }
            Ok(s.as_str())
        }
        Some(_) => Err(HostError::InvalidParams(format!(
            "{field} must be a string"
        ))),
    }
}

fn optional_worktree_id(params: &Value) -> Result<Option<&str>> {
    match params.get("worktreeId") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => {
            if s.is_empty() {
                return Err(HostError::InvalidParams("worktreeId is required".into()));
            }
            Ok(Some(s.as_str()))
        }
        Some(_) => Err(HostError::InvalidParams(
            "worktreeId must be a string".into(),
        )),
    }
}

fn optional_rel_path(params: &Value) -> Result<Option<&str>> {
    match params.get("path") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => {
            if s.split(['/', '\\']).any(|c| c == "..") || Path::new(s).is_absolute() {
                return Err(HostError::InvalidParams(
                    "path must not contain '..' or be absolute".into(),
                ));
            }
            Ok(Some(s.as_str()))
        }
        Some(_) => Err(HostError::InvalidParams("path must be a string".into())),
    }
}

fn git_root(store: &rt_storage::Store, params: &Value) -> Result<PathBuf> {
    let workspace_id = require_str(params, "workspaceId")?;
    let ws = store
        .workspace_get(workspace_id)?
        .ok_or_else(|| HostError::NotFound(format!("workspace {workspace_id}")))?;
    match optional_worktree_id(params)? {
        None => Ok(PathBuf::from(ws.path)),
        Some(wt_id) => {
            let wt = store
                .worktree_get(wt_id)?
                .ok_or_else(|| HostError::NotFound(format!("worktree {wt_id}")))?;
            if wt.workspace_id != ws.id {
                return Err(HostError::NotFound(format!("worktree {wt_id}")));
            }
            Ok(PathBuf::from(wt.path))
        }
    }
}

fn git_command(root: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("--no-pager");
    cmd.current_dir(root);
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    cmd.env("GIT_OPTIONAL_LOCKS", "0");
    cmd
}

fn git_output(root: &Path, args: &[&str]) -> Result<std::process::Output> {
    git_command(root)
        .args(args)
        .output()
        .map_err(|e| HostError::Internal(format!("git: {e}")))
}

fn git_stdout(root: &Path, args: &[&str]) -> Result<String> {
    let out = git_output(root, args)?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(HostError::Internal(format!(
            "git {args:?} failed: {stderr}"
        )));
    }
    String::from_utf8(out.stdout).map_err(|_| HostError::Internal("git output is not utf-8".into()))
}

fn is_git_repo(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    match git_output(path, &["rev-parse", "--is-inside-work-tree"]) {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim() == "true",
        _ => false,
    }
}

fn require_git(path: &Path) -> Result<()> {
    if !is_git_repo(path) {
        return Err(HostError::InvalidParams(format!(
            "not_git: {} is not a git repository",
            path.display()
        )));
    }
    Ok(())
}

fn git_worktree_add(workspace: &Path, dest: &Path, branch: &str) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| HostError::Internal(e.to_string()))?;
    }
    let dest_str = dest
        .to_str()
        .ok_or_else(|| HostError::Internal("worktree path is not utf-8".into()))?;
    let out = git_output(workspace, &["worktree", "add", "-b", branch, dest_str])?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(HostError::Internal(format!(
            "git worktree add failed: {stderr}"
        )));
    }
    Ok(())
}

fn classify_status(x: char, y: char) -> &'static str {
    if x == '?' || y == '?' {
        "untracked"
    } else if x == 'R' || y == 'R' {
        "renamed"
    } else if x == 'A' || y == 'A' {
        "added"
    } else if x == 'D' || y == 'D' {
        "deleted"
    } else {
        "modified"
    }
}

fn parse_branch_line(rest: &str) -> String {
    let rest = rest.trim();
    if let Some(name) = rest.strip_prefix("No commits yet on ") {
        return name
            .split("...")
            .next()
            .unwrap_or(name)
            .split_whitespace()
            .next()
            .unwrap_or(name)
            .to_string();
    }
    let name = rest.split("...").next().unwrap_or(rest);
    name.split_whitespace().next().unwrap_or(name).to_string()
}

fn parse_porcelain(stdout: &str) -> (String, Vec<GitStatusEntry>) {
    let mut branch = String::new();
    let mut entries = Vec::new();
    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix("## ") {
            branch = parse_branch_line(rest);
            continue;
        }
        if line.len() < 3 {
            continue;
        }
        let x = line.as_bytes()[0] as char;
        let y = line.as_bytes()[1] as char;
        let path_part = &line[3..];
        let status = classify_status(x, y);
        let path = if status == "renamed" {
            path_part
                .rsplit_once(" -> ")
                .map(|(_, new)| new)
                .unwrap_or(path_part)
                .to_string()
        } else {
            path_part.to_string()
        };
        entries.push(GitStatusEntry {
            path,
            status: status.to_string(),
        });
    }
    (branch, entries)
}

fn run_git_status(root: &Path) -> Result<GitStatus> {
    let stdout = git_stdout(root, &["status", "--porcelain=v1", "-b"])?;
    let (branch, mut entries) = parse_porcelain(&stdout);
    let truncated = entries.len() > STATUS_CAP;
    if truncated {
        entries.truncate(STATUS_CAP);
    }
    let dirty = !entries.is_empty();
    Ok(GitStatus {
        branch,
        dirty,
        entries,
        truncated,
    })
}

fn extract_b_path(header: &str) -> String {
    let header = header.trim();
    if let Some(idx) = header.rfind(" b/") {
        return header[idx + 3..].to_string();
    }
    if let Some(rest) = header.strip_prefix("a/") {
        if let Some((a, _)) = rest.split_once(" b/") {
            return a.to_string();
        }
    }
    header
        .split_whitespace()
        .next_back()
        .unwrap_or(header)
        .trim_start_matches("b/")
        .to_string()
}

fn parse_diff(output: &str) -> Vec<GitDiffFile> {
    let mut files = Vec::new();
    if output.is_empty() {
        return files;
    }
    let body = output.strip_prefix("diff --git ").unwrap_or(output);
    for part in body.split("diff --git ") {
        if part.trim().is_empty() {
            continue;
        }
        let full = format!("diff --git {part}");
        let first_line = part.lines().next().unwrap_or("");
        let path = extract_b_path(first_line);
        let is_binary = full.contains("Binary files ") || full.contains("GIT binary patch");
        let patch = if is_binary { None } else { Some(full) };
        files.push(GitDiffFile { path, patch });
    }
    files
}

fn untracked_file(root: &Path, rel: &str) -> GitDiffFile {
    let abs = root.join(rel);
    let data = match std::fs::read(&abs) {
        Ok(d) => d,
        Err(_) => {
            return GitDiffFile {
                path: rel.to_string(),
                patch: None,
            };
        }
    };
    let scan = data.len().min(8 * 1024);
    if data[..scan].contains(&0) || std::str::from_utf8(&data).is_err() {
        return GitDiffFile {
            path: rel.to_string(),
            patch: None,
        };
    }
    let text = match std::str::from_utf8(&data) {
        Ok(s) => s,
        Err(_) => {
            return GitDiffFile {
                path: rel.to_string(),
                patch: None,
            };
        }
    };
    let line_count = text.lines().count().max(1);
    let mut patch = format!(
        "diff --git a/{rel} b/{rel}\nnew file mode 100644\n--- /dev/null\n+++ b/{rel}\n@@ -0,0 +1,{line_count} @@\n"
    );
    for line in text.lines() {
        patch.push('+');
        patch.push_str(line);
        patch.push('\n');
    }
    GitDiffFile {
        path: rel.to_string(),
        patch: Some(patch),
    }
}

fn list_untracked(root: &Path) -> Result<Vec<String>> {
    let stdout = git_stdout(root, &["ls-files", "--others", "--exclude-standard"])?;
    Ok(stdout
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect())
}

fn apply_diff_cap(mut files: Vec<GitDiffFile>) -> GitDiff {
    let mut total = 0usize;
    let mut truncated = false;
    let mut kept = Vec::with_capacity(files.len());
    for mut file in files.drain(..) {
        if truncated {
            break;
        }
        if let Some(patch) = file.patch.take() {
            if total >= DIFF_MAX_BYTES {
                truncated = true;
                break;
            }
            if total + patch.len() > DIFF_MAX_BYTES {
                let remain = DIFF_MAX_BYTES - total;
                let cut = match patch.get(..remain) {
                    Some(s) => s.to_string(),
                    None => patch
                        .char_indices()
                        .take_while(|(i, _)| *i < remain)
                        .last()
                        .map(|(i, c)| i + c.len_utf8())
                        .map(|end| patch[..end].to_string())
                        .unwrap_or_default(),
                };
                file.patch = Some(cut);
                kept.push(file);
                truncated = true;
                break;
            }
            total += patch.len();
            file.patch = Some(patch);
        }
        kept.push(file);
    }
    GitDiff {
        files: kept,
        truncated,
    }
}

fn run_git_diff(root: &Path, path: Option<&str>) -> Result<GitDiff> {
    let mut args = vec!["diff"];
    if let Some(p) = path {
        args.push("--");
        args.push(p);
    }
    let stdout = git_stdout(root, &args)?;
    let mut files = parse_diff(&stdout);

    let untracked = list_untracked(root)?;
    for rel in untracked {
        if let Some(want) = path {
            if rel != want {
                continue;
            }
        }
        if files.iter().any(|f| f.path == rel) {
            continue;
        }
        files.push(untracked_file(root, &rel));
    }

    Ok(apply_diff_cap(files))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::HostService;
    use rt_storage::{new_id, Store};
    use serde_json::json;
    use std::collections::HashMap;
    use std::process::Command;
    use tempfile::tempdir;

    fn setup() -> (tempfile::TempDir, HostService) {
        let dir = tempdir().unwrap();
        let store = Store::open(dir.path().join("host.db")).unwrap();
        store.migrate().unwrap();
        let host_id = new_id();
        store.host_insert_if_absent(&host_id, "test").unwrap();
        let svc = HostService::new(
            store,
            HashMap::new(),
            host_id,
            dir.path().to_path_buf(),
            "http://127.0.0.1:0".into(),
            std::process::id(),
        );
        (dir, svc)
    }

    fn git(cwd: &Path, args: &[&str]) {
        let out = Command::new("git")
            .args(args)
            .current_dir(cwd)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t.test")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t.test")
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .expect("spawn git");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn init_git_repo(path: &Path) {
        std::fs::create_dir_all(path).unwrap();
        git(path, &["init", "-b", "main"]);
        git(path, &["config", "user.email", "t@t.test"]);
        git(path, &["config", "user.name", "t"]);
        git(path, &["config", "commit.gpgsign", "false"]);
        std::fs::write(path.join("README.md"), "hello\n").unwrap();
        git(path, &["add", "README.md"]);
        git(path, &["commit", "-m", "init"]);
    }

    fn seed_agent(svc: &HostService, ws_dir: &Path) -> (String, String) {
        let ws = svc.workspace_add(ws_dir.to_str().unwrap()).unwrap();
        let task = svc.task_create("t", &ws.id).unwrap();
        let agent = svc.agent_create(&task.id, Some("cli.generic")).unwrap();
        (ws.id, agent.id)
    }

    #[test]
    fn worktree_ensure_not_git() {
        let (dir, svc) = setup();
        let ws_dir = dir.path().join("plain");
        std::fs::create_dir(&ws_dir).unwrap();
        let (_ws_id, agent_id) = seed_agent(&svc, &ws_dir);
        let err = svc.worktree_ensure(&agent_id).unwrap_err();
        assert_eq!(err.code(), "invalid_params");
        assert!(
            err.to_string().contains("not_git"),
            "message must contain not_git: {err}"
        );
    }

    #[test]
    fn worktree_ensure_idempotent() {
        let (dir, svc) = setup();
        let ws_dir = dir.path().join("repo");
        init_git_repo(&ws_dir);
        let (ws_id, agent_id) = seed_agent(&svc, &ws_dir);
        let first = svc.worktree_ensure(&agent_id).unwrap();
        let second = svc.worktree_ensure(&agent_id).unwrap();
        assert_eq!(first.id, second.id);
        assert_eq!(first.path, second.path);
        assert_eq!(first.branch, second.branch);
        assert_eq!(first.agent_id, agent_id);
        assert_eq!(first.workspace_id, ws_id);
        assert!(first.branch.starts_with("rt/"));
        assert!(first.path.contains("worktrees"));
        let items = svc.worktree_list(&ws_id).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, first.id);
        let agent = svc.agent_get(&agent_id).unwrap();
        assert_eq!(agent.run_location, "worktree");
    }

    #[test]
    fn git_status_and_diff_dirty_repo() {
        let (dir, svc) = setup();
        let ws_dir = dir.path().join("repo");
        init_git_repo(&ws_dir);
        let (ws_id, _agent_id) = seed_agent(&svc, &ws_dir);
        std::fs::write(ws_dir.join("README.md"), "hello\nworld\n").unwrap();
        std::fs::write(ws_dir.join("new.txt"), "fresh\n").unwrap();

        let status = svc.git_status(&json!({ "workspaceId": ws_id })).unwrap();
        assert!(status.dirty, "{status:?}");
        assert!(
            status
                .entries
                .iter()
                .any(|e| e.path == "README.md" && e.status == "modified"),
            "{:?}",
            status.entries
        );
        assert!(
            status
                .entries
                .iter()
                .any(|e| e.path == "new.txt" && e.status == "untracked"),
            "{:?}",
            status.entries
        );
        assert!(!status.truncated);

        let diff = svc.git_diff(&json!({ "workspaceId": ws_id })).unwrap();
        assert!(
            diff.files.iter().any(|f| {
                f.path == "README.md" && f.patch.as_ref().is_some_and(|p| p.contains("world"))
            }),
            "{:?}",
            diff.files
        );
        assert!(!diff.truncated);
    }

    #[test]
    fn worktree_get_missing_is_not_found() {
        let (dir, svc) = setup();
        let ws_dir = dir.path().join("plain");
        std::fs::create_dir(&ws_dir).unwrap();
        let (_ws_id, agent_id) = seed_agent(&svc, &ws_dir);
        let err = svc.worktree_get(&agent_id).unwrap_err();
        assert_eq!(err.code(), "not_found");
        let err = svc
            .worktree_get("0191f0c6-7c2a-7c11-8000-6f0c1a2b3c4d")
            .unwrap_err();
        assert_eq!(err.code(), "not_found");
    }

    #[test]
    fn git_status_not_git_is_invalid_params() {
        let (dir, svc) = setup();
        let ws_dir = dir.path().join("plain");
        std::fs::create_dir(&ws_dir).unwrap();
        let (ws_id, _agent_id) = seed_agent(&svc, &ws_dir);
        let err = svc
            .git_status(&json!({ "workspaceId": ws_id }))
            .unwrap_err();
        assert_eq!(err.code(), "invalid_params");
        assert!(
            err.to_string().contains("not_git"),
            "message must contain not_git: {err}"
        );
    }

    #[test]
    fn worktree_list_missing_workspace_is_not_found() {
        let (_dir, svc) = setup();
        let err = svc
            .worktree_list("0191f0c6-7c2a-7c11-8000-6f0c1a2b3c4d")
            .unwrap_err();
        assert_eq!(err.code(), "not_found");
    }

    #[test]
    fn worktree_ensure_missing_agent_is_not_found() {
        let (_dir, svc) = setup();
        let err = svc
            .worktree_ensure("0191f0c6-7c2a-7c11-8000-6f0c1a2b3c4d")
            .unwrap_err();
        assert_eq!(err.code(), "not_found");
    }
}
