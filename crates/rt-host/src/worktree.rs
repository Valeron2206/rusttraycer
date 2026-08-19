//! Git worktree + `git.status` / `git.diff` / mutate via the `git` CLI. No git2.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use rt_protocol::{
    GitCommitOk, GitDiff, GitDiffFile, GitPushOk, GitStatus, GitStatusEntry, PrCheck, PrCommit,
    PrFile, PrGetOk, PrGetParams, Worktree, WorktreeGcItem, WorktreeGcOk, WorktreeGcReason,
};
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
        let prefix = self.store.worktree_branch_prefix(&workspace.id)?;
        let branch = format!("{prefix}{}", short_agent_id(agent_id));
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

    pub fn worktree_gc(&self, dry_run: bool) -> Result<WorktreeGcOk> {
        let rows = self.store.worktree_list_all()?;
        let mut items = Vec::new();
        for row in rows {
            let prefix = self.store.worktree_branch_prefix(&row.workspace_id)?;
            if !row.branch.starts_with(&prefix) {
                continue;
            }
            let agent = self.store.agent_get(&row.agent_id)?;
            if agent
                .as_ref()
                .is_some_and(|a| a.status == rt_storage::AgentStatus::Running)
            {
                continue;
            }
            let ws = self.store.workspace_get(&row.workspace_id)?;
            let ws_path = ws.as_ref().map(|w| PathBuf::from(&w.path));
            let wt_path = PathBuf::from(&row.path);
            let dir_missing = !wt_path.exists();
            let agent_gone = agent.is_none();
            let reason =
                classify_gc_reason(ws_path.as_deref(), &row.branch, dir_missing, agent_gone);
            if !dry_run {
                let removed = remove_worktree_if_safe(ws_path.as_deref(), &wt_path)?;
                if !removed && !dir_missing {
                    continue;
                }
                self.store.worktree_delete(&row.id)?;
                if agent.is_some() {
                    if let Err(e) = self.store.agent_set_run_location(&row.agent_id, "local") {
                        tracing::warn!(error = %e, agent_id = %row.agent_id, "reset run_location after gc");
                    }
                }
            }
            items.push(WorktreeGcItem {
                worktree_id: row.id,
                path: row.path,
                reason,
            });
        }
        Ok(WorktreeGcOk { dry_run, items })
    }

    pub fn pr_get(&self, params: &PrGetParams) -> Result<PrGetOk> {
        let selector = resolve_pr_selector(params)?;
        if params.workspace_id.is_empty() {
            return Err(HostError::InvalidParams("workspaceId is required".into()));
        }
        let workspace = self
            .store
            .workspace_get(&params.workspace_id)?
            .ok_or_else(|| HostError::NotFound(format!("workspace {}", params.workspace_id)))?;
        let cwd = resolve_git_cwd(Path::new(&workspace.path));
        require_gh_auth(&cwd)?;
        let mut ok = gh_pr_view(&cwd, &selector)?;
        ok.diff = local_pr_diff(&cwd);
        Ok(ok)
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

    pub fn git_stage(&self, params: &Value) -> Result<GitStatus> {
        let root = git_root(&self.store, params)?;
        require_git(&root)?;
        let paths = require_paths(params)?;
        let mut args = vec!["add".to_string(), "--".to_string()];
        args.extend(paths);
        git_stdout(&root, &args.iter().map(String::as_str).collect::<Vec<_>>())?;
        run_git_status(&root)
    }

    pub fn git_unstage(&self, params: &Value) -> Result<GitStatus> {
        let root = git_root(&self.store, params)?;
        require_git(&root)?;
        let paths = require_paths(params)?;
        let mut args = vec![
            "restore".to_string(),
            "--staged".to_string(),
            "--".to_string(),
        ];
        args.extend(paths);
        git_stdout(&root, &args.iter().map(String::as_str).collect::<Vec<_>>())?;
        run_git_status(&root)
    }

    pub fn git_restore(&self, params: &Value) -> Result<GitStatus> {
        let root = git_root(&self.store, params)?;
        require_git(&root)?;
        let paths = require_paths(params)?;
        let staged = optional_bool(params, "staged", false)?;
        let mut tracked = Vec::new();
        let mut untracked = Vec::new();
        for path in paths {
            if is_tracked(&root, &path)? {
                tracked.push(path);
            } else {
                untracked.push(path);
            }
        }
        if !tracked.is_empty() {
            let mut args = vec!["restore".to_string(), "--worktree".to_string()];
            if staged {
                args.push("--staged".to_string());
            }
            args.push("--".to_string());
            args.extend(tracked);
            git_stdout(&root, &args.iter().map(String::as_str).collect::<Vec<_>>())?;
        }
        for path in untracked {
            let target = crate::files::resolve_inside(&root, &path)?;
            if target.is_dir() {
                return Err(HostError::InvalidParams("path is a directory".into()));
            }
            if target.is_file() {
                std::fs::remove_file(&target).map_err(|e| HostError::Internal(e.to_string()))?;
            }
        }
        run_git_status(&root)
    }

    pub fn git_commit(&self, params: &Value) -> Result<GitCommitOk> {
        let root = git_root(&self.store, params)?;
        require_git(&root)?;
        let message = require_str(params, "message")?;
        if message.trim().is_empty() {
            return Err(HostError::InvalidParams("message is required".into()));
        }
        if message.len() > 4096 {
            return Err(HostError::InvalidParams("message exceeds 4 KiB".into()));
        }
        require_git_identity(&root)?;
        let out = git_output(&root, &["commit", "-m", message])?;
        if !out.status.success() {
            let stderr = redact_secrets(&String::from_utf8_lossy(&out.stderr));
            return Err(classify_commit_fail(&stderr));
        }
        let commit = git_stdout(&root, &["rev-parse", "HEAD"])?
            .trim()
            .to_string();
        let branch = git_stdout(&root, &["rev-parse", "--abbrev-ref", "HEAD"])?
            .trim()
            .to_string();
        Ok(GitCommitOk { commit, branch })
    }

    pub async fn git_push(&self, params: &Value) -> Result<GitPushOk> {
        let root = git_root(&self.store, params)?;
        require_git(&root)?;
        let remote = match params.get("remote") {
            None | Some(Value::Null) => "origin".to_string(),
            Some(Value::String(s)) if !s.is_empty() => s.clone(),
            Some(Value::String(_)) => {
                return Err(HostError::InvalidParams("remote is required".into()));
            }
            Some(_) => return Err(HostError::InvalidParams("remote must be a string".into())),
        };
        let ref_name = match params.get("ref") {
            None | Some(Value::Null) => git_stdout(&root, &["rev-parse", "--abbrev-ref", "HEAD"])?
                .trim()
                .to_string(),
            Some(Value::String(s)) if !s.is_empty() => s.clone(),
            Some(Value::String(_)) => {
                return Err(HostError::InvalidParams("ref is required".into()));
            }
            Some(_) => return Err(HostError::InvalidParams("ref must be a string".into())),
        };
        if ref_name == "HEAD" {
            return Err(HostError::InvalidParams(
                "ref must be a branch name, not detached HEAD".into(),
            ));
        }
        let args = push_args(&remote, &ref_name);
        let out = git_output_timeout(&root, &args, std::time::Duration::from_secs(120)).await?;
        if !out.status.success() {
            let stderr = redact_secrets(&String::from_utf8_lossy(&out.stderr));
            return Err(classify_push_stderr(&stderr));
        }
        Ok(GitPushOk {
            remote,
            ref_name,
            ok: true,
        })
    }

    pub fn files_patch(&self, params: &Value) -> Result<Value> {
        if !params.is_object() {
            return Err(HostError::InvalidParams("params must be an object".into()));
        }
        let workspace_id = require_str(params, "workspaceId")?;
        let patch = require_str(params, "patch")?;
        let wt = optional_worktree_id(params)?;
        apply_unified_diff(&self.store, workspace_id, wt, patch)
    }

    pub fn apply_patch_at(
        &self,
        workspace_id: &str,
        worktree_id: Option<&str>,
        patch: &str,
    ) -> Result<Value> {
        apply_unified_diff(&self.store, workspace_id, worktree_id, patch)
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
    // Tail of a UUID v7 is random; the prefix is a millisecond timestamp and
    // collides for two agents created in the same ~65s window.
    let hex: String = id.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    let start = hex.len().saturating_sub(8);
    hex[start..].to_string()
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
        let stderr = redact_secrets(&String::from_utf8_lossy(&out.stderr));
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

fn resolve_git_cwd(path: &Path) -> PathBuf {
    if is_git_repo(path) {
        return path.to_path_buf();
    }
    match git_output(path, &["rev-parse", "--show-toplevel"]) {
        Ok(out) if out.status.success() => {
            let top = String::from_utf8_lossy(&out.stdout);
            let top = top.trim();
            if !top.is_empty() {
                return PathBuf::from(top);
            }
        }
        _ => {}
    }
    path.to_path_buf()
}

fn parse_github_pr_number(url: &str) -> Option<u64> {
    let lower = url.to_ascii_lowercase();
    let host = lower.find("github.com")?;
    let after_host = &url[host + "github.com".len()..];
    let after_host_l = after_host.to_ascii_lowercase();
    let pull = after_host_l.find("/pull/")?;
    let rest = &after_host[pull + "/pull/".len()..];
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
}

#[derive(Debug)]
enum PrSelector {
    Number(u64),
    Url(String),
}

impl PrSelector {
    fn as_arg(&self) -> String {
        match self {
            Self::Number(n) => n.to_string(),
            Self::Url(u) => u.clone(),
        }
    }
}

fn resolve_pr_selector(params: &PrGetParams) -> Result<PrSelector> {
    let number = params.number;
    let url = params
        .url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    match (number, url) {
        (None, None) => Err(HostError::InvalidParams("number or url is required".into())),
        (Some(n), None) => Ok(PrSelector::Number(n)),
        (None, Some(u)) => match parse_github_pr_number(u) {
            Some(_) => Ok(PrSelector::Url(u.to_string())),
            None => Err(HostError::InvalidParams(
                "url must be github.com/.../pull/N".into(),
            )),
        },
        (Some(n), Some(u)) => match parse_github_pr_number(u) {
            None => Err(HostError::InvalidParams(
                "url must be github.com/.../pull/N".into(),
            )),
            Some(un) if un != n => Err(HostError::InvalidParams("number and url conflict".into())),
            Some(_) => Ok(PrSelector::Number(n)),
        },
    }
}

fn gh_command(cwd: &Path) -> Command {
    let mut cmd = Command::new("gh");
    cmd.current_dir(cwd);
    cmd.env("GH_PROMPT_DISABLED", "1");
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    cmd
}

fn gh_spawn_error(err: io::Error) -> HostError {
    if err.kind() == io::ErrorKind::NotFound {
        HostError::AuthRequired("gh is not installed".into())
    } else {
        HostError::AuthRequired(format!("gh: {err}"))
    }
}

fn is_gh_auth_error(text: &str) -> bool {
    let low = text.to_ascii_lowercase();
    low.contains("http 401")
        || low.contains("401 unauthorized")
        || low.contains("not logged into")
        || low.contains("not logged in")
        || low.contains("authentication required")
        || low.contains("to log in, run")
        || low.contains("gh auth login")
        || low.contains("not authenticated")
}

fn is_gh_not_found(text: &str) -> bool {
    let low = text.to_ascii_lowercase();
    low.contains("could not find")
        || low.contains("could not resolve")
        || low.contains("no pull requests")
        || low.contains("does not exist")
        || low.contains("not found")
        || low.contains("http 404")
}

fn require_gh_auth(cwd: &Path) -> Result<()> {
    let out = gh_command(cwd)
        .args(["auth", "status"])
        .output()
        .map_err(gh_spawn_error)?;
    if out.status.success() {
        return Ok(());
    }
    let stderr = redact_secrets(&String::from_utf8_lossy(&out.stderr));
    let stdout = redact_secrets(&String::from_utf8_lossy(&out.stdout));
    let detail = if !stderr.trim().is_empty() {
        stderr
    } else if !stdout.trim().is_empty() {
        stdout
    } else {
        "gh auth status failed".into()
    };
    Err(HostError::AuthRequired(detail))
}

#[derive(Debug, serde::Deserialize)]
struct GhPrView {
    number: u64,
    url: String,
    title: String,
    state: String,
    #[serde(default)]
    commits: Vec<GhCommit>,
    #[serde(default)]
    files: Vec<GhFile>,
    #[serde(default, rename = "statusCheckRollup")]
    status_check_rollup: Vec<GhCheck>,
}

#[derive(Debug, serde::Deserialize)]
struct GhCommit {
    #[serde(default)]
    oid: String,
    #[serde(default, rename = "messageHeadline")]
    message_headline: String,
    #[serde(default)]
    authors: Vec<GhAuthor>,
}

#[derive(Debug, serde::Deserialize)]
struct GhAuthor {
    name: Option<String>,
    login: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct GhFile {
    path: String,
    #[serde(default)]
    additions: u64,
    #[serde(default)]
    deletions: u64,
    #[serde(default, rename = "changeType")]
    change_type: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct GhCheck {
    name: Option<String>,
    context: Option<String>,
    status: Option<String>,
    state: Option<String>,
    conclusion: Option<String>,
}

fn map_gh_pr_view(view: GhPrView) -> PrGetOk {
    let checks = view
        .status_check_rollup
        .into_iter()
        .filter_map(|c| {
            let name = c
                .name
                .filter(|s| !s.is_empty())
                .or_else(|| c.context.filter(|s| !s.is_empty()))?;
            let status = c
                .status
                .filter(|s| !s.is_empty())
                .or_else(|| c.state.clone())
                .unwrap_or_default();
            let conclusion = c.conclusion.filter(|s| !s.is_empty()).or_else(|| {
                c.state
                    .filter(|s| !s.is_empty() && !status.eq_ignore_ascii_case("pending"))
            });
            Some(PrCheck {
                name,
                status,
                conclusion,
            })
        })
        .collect();
    let commits = view
        .commits
        .into_iter()
        .map(|c| {
            let author = c.authors.into_iter().find_map(|a| {
                a.name
                    .filter(|s| !s.is_empty())
                    .or_else(|| a.login.filter(|s| !s.is_empty()))
            });
            PrCommit {
                sha: c.oid,
                title: c.message_headline,
                author,
            }
        })
        .collect();
    let files = view
        .files
        .into_iter()
        .map(|f| {
            let status = f
                .change_type
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "MODIFIED".into());
            PrFile {
                path: f.path,
                additions: f.additions,
                deletions: f.deletions,
                status,
            }
        })
        .collect();
    PrGetOk {
        number: view.number,
        url: view.url,
        title: view.title,
        state: view.state,
        checks,
        commits,
        files,
        diff: String::new(),
    }
}

fn classify_gh_pr_error(stdout: &str, stderr: &str) -> HostError {
    let combined = redact_secrets(&format!("{stdout}\n{stderr}"));
    if is_gh_auth_error(&combined) {
        return HostError::AuthRequired(combined);
    }
    if is_gh_not_found(&combined) {
        return HostError::NotFound(format!("pull request: {combined}"));
    }
    let stderr = redact_secrets(stderr);
    if stderr.trim().is_empty() {
        HostError::Internal("gh pr view failed".into())
    } else {
        HostError::Internal(format!("gh pr view failed: {stderr}"))
    }
}

fn gh_pr_view(cwd: &Path, selector: &PrSelector) -> Result<PrGetOk> {
    let arg = selector.as_arg();
    let out = gh_command(cwd)
        .args([
            "pr",
            "view",
            &arg,
            "--json",
            "number,url,title,state,commits,files,statusCheckRollup",
        ])
        .output()
        .map_err(gh_spawn_error)?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    if !out.status.success() {
        return Err(classify_gh_pr_error(&stdout, &stderr));
    }
    if is_gh_auth_error(&stdout) || is_gh_auth_error(&stderr) {
        return Err(classify_gh_pr_error(&stdout, &stderr));
    }
    let view: GhPrView = serde_json::from_str(stdout.trim()).map_err(|e| {
        HostError::Internal(format!(
            "gh pr view json: {e}: {}",
            redact_secrets(stdout.trim())
        ))
    })?;
    Ok(map_gh_pr_view(view))
}

fn local_pr_diff(root: &Path) -> String {
    if !is_git_repo(root) {
        return String::new();
    }
    let Some(base) = default_branch(root) else {
        return String::new();
    };
    let mb = match git_stdout(root, &["merge-base", "HEAD", &base]) {
        Ok(s) => s.trim().to_string(),
        Err(_) => return String::new(),
    };
    if mb.is_empty() {
        return String::new();
    }
    git_stdout(root, &["diff", &mb]).unwrap_or_default()
}

fn classify_gc_reason(
    workspace: Option<&Path>,
    branch: &str,
    _dir_missing: bool,
    _agent_gone: bool,
) -> WorktreeGcReason {
    if let Some(root) = workspace {
        if is_git_repo(root) {
            if gh_branch_merged(root, branch) {
                return WorktreeGcReason::Merged;
            }
            if branch_landed(root, branch) {
                return WorktreeGcReason::Landed;
            }
        }
    }
    WorktreeGcReason::Stale
}

fn gh_branch_merged(root: &Path, branch: &str) -> bool {
    let out = Command::new("gh")
        .args([
            "pr", "list", "--head", branch, "--state", "merged", "--json", "number", "--limit", "1",
        ])
        .current_dir(root)
        .env("GH_PROMPT_DISABLED", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output();
    let out = match out {
        Ok(o) if o.status.success() => o,
        _ => return false,
    };
    match serde_json::from_slice::<Value>(&out.stdout) {
        Ok(Value::Array(arr)) => !arr.is_empty(),
        _ => false,
    }
}

fn default_branch(root: &Path) -> Option<String> {
    if let Ok(out) = git_output(
        root,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    ) {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout);
            let s = s.trim();
            if let Some(name) = s.strip_prefix("origin/") {
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }
    }
    for cand in ["main", "master"] {
        if let Ok(out) = git_output(root, &["rev-parse", "--verify", cand]) {
            if out.status.success() {
                return Some(cand.to_string());
            }
        }
    }
    None
}

fn branch_landed(root: &Path, branch: &str) -> bool {
    let default = match default_branch(root) {
        Some(d) => d,
        None => return false,
    };
    let ancestor = match git_output(root, &["merge-base", "--is-ancestor", branch, &default]) {
        Ok(o) => o.status.success(),
        Err(_) => false,
    };
    if !ancestor {
        return false;
    }
    let bsha = git_stdout(root, &["rev-parse", branch]).ok();
    let dsha = git_stdout(root, &["rev-parse", &default]).ok();
    match (bsha, dsha) {
        (Some(b), Some(d)) => b.trim() != d.trim(),
        _ => false,
    }
}

fn remove_worktree_if_safe(workspace: Option<&Path>, wt_path: &Path) -> Result<bool> {
    let Some(root) = workspace else {
        return Ok(!wt_path.exists());
    };
    if !is_git_repo(root) {
        return Ok(!wt_path.exists());
    }
    let Some(path_str) = wt_path.to_str() else {
        return Err(HostError::Internal("worktree path is not utf-8".into()));
    };
    let out = git_output(root, &["worktree", "remove", "--force", path_str])?;
    if out.status.success() {
        return Ok(true);
    }
    let _ = git_output(root, &["worktree", "prune"]);
    Ok(!wt_path.exists())
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
        let stderr = redact_secrets(&String::from_utf8_lossy(&out.stderr));
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

fn require_paths(params: &Value) -> Result<Vec<String>> {
    let arr = match params.get("paths") {
        None | Some(Value::Null) => {
            return Err(HostError::InvalidParams("paths is required".into()));
        }
        Some(Value::Array(a)) => a,
        Some(_) => {
            return Err(HostError::InvalidParams("paths must be an array".into()));
        }
    };
    if arr.is_empty() || arr.len() > 500 {
        return Err(HostError::InvalidParams(
            "paths must have 1..=500 entries".into(),
        ));
    }
    let mut out = Vec::with_capacity(arr.len());
    for v in arr {
        let s = v
            .as_str()
            .ok_or_else(|| HostError::InvalidParams("paths entries must be strings".into()))?;
        if s.is_empty() {
            return Err(HostError::InvalidParams("path must be nonempty".into()));
        }
        if s.split(['/', '\\']).any(|c| c == "..") || Path::new(s).is_absolute() {
            return Err(HostError::InvalidParams(
                "path must not contain '..' or be absolute".into(),
            ));
        }
        out.push(s.to_string());
    }
    Ok(out)
}

fn optional_bool(params: &Value, field: &str, default: bool) -> Result<bool> {
    match params.get(field) {
        None | Some(Value::Null) => Ok(default),
        Some(Value::Bool(b)) => Ok(*b),
        Some(_) => Err(HostError::InvalidParams(format!(
            "{field} must be a boolean"
        ))),
    }
}

fn is_tracked(root: &Path, rel: &str) -> Result<bool> {
    let out = git_output(root, &["ls-files", "--error-unmatch", "--", rel])?;
    Ok(out.status.success())
}

fn git_config_get(root: &Path, key: &str) -> Result<Option<String>> {
    let out = git_output(root, &["config", "--get", key])?;
    if !out.status.success() {
        return Ok(None);
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        Ok(None)
    } else {
        Ok(Some(s))
    }
}

fn require_git_identity(root: &Path) -> Result<()> {
    let name = git_config_get(root, "user.name")?;
    let email = git_config_get(root, "user.email")?;
    match (name, email) {
        (Some(n), Some(e)) if !n.is_empty() && !e.is_empty() => Ok(()),
        _ => Err(HostError::GitIdentity(
            "set git config user.email (and user.name) before committing".into(),
        )),
    }
}

fn classify_commit_fail(stderr: &str) -> HostError {
    let low = stderr.to_ascii_lowercase();
    if low.contains("author identity")
        || low.contains("tell me who you are")
        || (low.contains("user.email") && low.contains("unable"))
    {
        HostError::GitIdentity("set git config user.email (and user.name) before committing".into())
    } else {
        HostError::Internal(format!("git commit failed: {stderr}"))
    }
}

/// `git push <remote> <ref>` — never force, tags, or mirror.
pub fn push_args(remote: &str, ref_name: &str) -> Vec<String> {
    vec!["push".to_string(), remote.to_string(), ref_name.to_string()]
}

pub fn classify_push_stderr(stderr: &str) -> HostError {
    let stderr = redact_secrets(stderr);
    let low = stderr.to_ascii_lowercase();
    const AUTH: &[&str] = &[
        "authentication",
        "403",
        "401",
        "could not read username",
        "permission denied (publickey)",
        "fatal: could not read",
        "unable to access",
        "failed to connect",
        "connection refused",
        "could not resolve",
        "could not read from remote",
        "terminal prompts disabled",
        "could not read password",
    ];
    const CONFLICT: &[&str] = &[
        "non-fast-forward",
        "failed to push some refs",
        "[rejected]",
        "updates were rejected",
        "fetch first",
    ];
    if AUTH.iter().any(|p| low.contains(p)) {
        return HostError::GitAuth(stderr.to_string());
    }
    if CONFLICT.iter().any(|p| low.contains(p)) || low.contains("rejected") {
        return HostError::GitConflict(stderr.to_string());
    }
    HostError::Internal(format!("git push failed: {stderr}"))
}

pub fn redact_secrets(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(scheme) = rest.find("://") {
        out.push_str(&rest[..scheme]);
        let after = &rest[scheme + 3..];
        if let Some(at) = after.find('@') {
            let creds = &after[..at];
            if creds.contains(':') && !creds.contains('/') {
                out.push_str("://***:***@");
                rest = &after[at + 1..];
                continue;
            }
        }
        out.push_str("://");
        rest = after;
    }
    out.push_str(rest);
    redact_token_like(&out)
}

fn redact_token_like(s: &str) -> String {
    const PREFIXES: &[&str] = &[
        "ghp_",
        "gho_",
        "ghs_",
        "ghu_",
        "ghr_",
        "github_pat_",
        "glpat-",
        "xoxb-",
        "xoxp-",
        "sk-live-",
        "sk-test-",
    ];
    let mut out = s.to_string();
    for prefix in PREFIXES {
        let mut start = 0;
        while let Some(idx) = out[start..].find(prefix) {
            let abs = start + idx;
            let rest = &out[abs + prefix.len()..];
            let n = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                .count();
            if n >= 8 {
                let end =
                    abs + prefix.len() + rest.chars().take(n).map(|c| c.len_utf8()).sum::<usize>();
                out.replace_range(abs..end, &format!("{prefix}***"));
                start = abs + prefix.len() + 3;
            } else {
                start = abs + prefix.len();
            }
        }
    }
    out
}

pub fn parse_patch_stats(patch: &str) -> (Vec<String>, u32) {
    let mut paths = Vec::new();
    let mut hunks = 0u32;
    for line in patch.lines() {
        if let Some(rest) = line.strip_prefix("+++ b/") {
            let p = rest.trim().to_string();
            if !p.is_empty() && !paths.iter().any(|x| x == &p) {
                paths.push(p);
            }
        } else if let Some(rest) = line.strip_prefix("diff --git ") {
            if let Some(idx) = rest.rfind(" b/") {
                let p = rest[idx + 3..].trim().to_string();
                if !p.is_empty() && !paths.iter().any(|x| x == &p) {
                    paths.push(p);
                }
            }
        } else if line.starts_with("@@") {
            hunks += 1;
        }
    }
    (paths, hunks)
}

fn apply_unified_diff(
    store: &rt_storage::Store,
    workspace_id: &str,
    worktree_id: Option<&str>,
    patch: &str,
) -> Result<Value> {
    if patch.len() as u64 > crate::files::MAX_FILE_BYTES {
        return Err(HostError::FileTooLarge("patch exceeds 256 KiB".into()));
    }
    let scan = patch.len().min(crate::files::BINARY_SCAN_BYTES);
    if patch.as_bytes()[..scan].contains(&0) {
        return Err(HostError::FileBinary("NUL in first 8 KiB".into()));
    }
    let mut params = serde_json::json!({ "workspaceId": workspace_id });
    if let Some(wt) = worktree_id {
        params["worktreeId"] = serde_json::json!(wt);
    }
    let root = git_root(store, &params)?;
    require_git(&root)?;
    let check = git_apply(&root, patch, true)?;
    if !check.status.success() {
        let stderr = redact_secrets(&String::from_utf8_lossy(&check.stderr));
        return Err(HostError::PatchFailed(stderr));
    }
    let applied = git_apply(&root, patch, false)?;
    if !applied.status.success() {
        let stderr = redact_secrets(&String::from_utf8_lossy(&applied.stderr));
        return Err(HostError::PatchFailed(stderr));
    }
    let (paths, hunks) = parse_patch_stats(patch);
    Ok(serde_json::json!({ "paths": paths, "hunks": hunks }))
}

fn git_apply(root: &Path, patch: &str, check: bool) -> Result<std::process::Output> {
    use std::io::Write;
    use std::process::Stdio;
    let mut cmd = git_command(root);
    cmd.arg("apply");
    if check {
        cmd.arg("--check");
    }
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| HostError::Internal(format!("git apply: {e}")))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(patch.as_bytes())
            .map_err(|e| HostError::Internal(format!("git apply stdin: {e}")))?;
    }
    child
        .wait_with_output()
        .map_err(|e| HostError::Internal(format!("git apply: {e}")))
}

async fn git_output_timeout(
    root: &Path,
    args: &[String],
    timeout: std::time::Duration,
) -> Result<std::process::Output> {
    let mut cmd = tokio::process::Command::new("git");
    cmd.arg("--no-pager");
    cmd.current_dir(root);
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    cmd.env("GIT_OPTIONAL_LOCKS", "0");
    cmd.args(args);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    cmd.kill_on_drop(true);
    let child = cmd
        .spawn()
        .map_err(|e| HostError::Internal(format!("git: {e}")))?;
    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(out)) => Ok(out),
        Ok(Err(e)) => Err(HostError::Internal(format!("git: {e}"))),
        Err(_) => Err(HostError::Internal("git push timed out".into())),
    }
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
    fn worktree_files_do_not_leak_between_worktrees() {
        let (dir, svc) = setup();
        let ws_dir = dir.path().join("repo");
        init_git_repo(&ws_dir);
        let ws = svc.workspace_add(ws_dir.to_str().unwrap()).unwrap();
        let task = svc.task_create("t", &ws.id).unwrap();
        let agent_a = svc.agent_create(&task.id, Some("cli.generic")).unwrap();
        let agent_b = svc.agent_create(&task.id, Some("cli.generic")).unwrap();
        let wt_a = svc.worktree_ensure(&agent_a.id).unwrap();
        let wt_b = svc.worktree_ensure(&agent_b.id).unwrap();
        assert_ne!(wt_a.path, wt_b.path);
        assert_ne!(wt_a.id, wt_b.id);

        let isolated = "only_in_a.txt";
        std::fs::write(Path::new(&wt_a.path).join(isolated), "secret-a\n").unwrap();

        let tree_a = svc
            .files_tree(&ws.id, None, Some(2), None, Some(&wt_a.id))
            .unwrap();
        let names_a: Vec<&str> = tree_a["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|e| e["path"].as_str())
            .collect();
        assert!(
            names_a.contains(&isolated),
            "worktree A must see {isolated}: {names_a:?}"
        );

        let tree_b = svc
            .files_tree(&ws.id, None, Some(2), None, Some(&wt_b.id))
            .unwrap();
        let names_b: Vec<&str> = tree_b["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|e| e["path"].as_str())
            .collect();
        assert!(
            !names_b.contains(&isolated),
            "worktree B must not see {isolated}: {names_b:?}"
        );

        let read_a = svc.files_read(&ws.id, isolated, Some(&wt_a.id)).unwrap();
        assert_eq!(read_a["content"], "secret-a\n");
        let err = svc
            .files_read(&ws.id, isolated, Some(&wt_b.id))
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
    #[test]
    fn porcelain_and_diff_parsers() {
        assert_eq!(classify_status('?', '?'), "untracked");
        assert_eq!(classify_status('R', ' '), "renamed");
        assert_eq!(classify_status(' ', 'R'), "renamed");
        assert_eq!(classify_status('A', ' '), "added");
        assert_eq!(classify_status(' ', 'A'), "added");
        assert_eq!(classify_status('D', ' '), "deleted");
        assert_eq!(classify_status(' ', 'D'), "deleted");
        assert_eq!(classify_status('M', ' '), "modified");
        assert_eq!(parse_branch_line("main...origin/main"), "main");
        assert_eq!(parse_branch_line("No commits yet on topic"), "topic");
        let (branch, entries) = parse_porcelain(
            "## main\n M src/lib.rs\n?? new.txt\nR  old.rs -> new.rs\nA  added.rs\n D gone.rs\nx\n",
        );
        assert_eq!(branch, "main");
        assert_eq!(entries[0].status, "modified");
        assert_eq!(entries[1].status, "untracked");
        assert_eq!(entries[2].path, "new.rs");
        assert_eq!(entries[2].status, "renamed");
        assert_eq!(entries[3].status, "added");
        assert_eq!(entries[4].status, "deleted");

        assert_eq!(extract_b_path("a/src/lib.rs b/src/lib.rs"), "src/lib.rs");
        assert!(parse_diff("").is_empty());
        let files = parse_diff(
            "diff --git a/a.rs b/a.rs\nindex 1..2\n--- a/a.rs\n+++ b/a.rs\n@@\n+hi\ndiff --git a/b.bin b/b.bin\nBinary files a/b.bin and b/b.bin differ\n",
        );
        assert_eq!(files.len(), 2);
        assert!(files[0].patch.is_some());
        assert!(files[1].patch.is_none());

        let big = "x".repeat(DIFF_MAX_BYTES + 50);
        let cap = apply_diff_cap(vec![
            GitDiffFile {
                path: "a".into(),
                patch: Some(big),
            },
            GitDiffFile {
                path: "b".into(),
                patch: Some("more".into()),
            },
        ]);
        assert!(cap.truncated);
        assert_eq!(cap.files.len(), 1);
        assert!(cap.files[0].patch.as_ref().unwrap().len() <= DIFF_MAX_BYTES);

        assert_eq!(
            short_agent_id("0191f0c6-7c2a-7c11-8000-6f0c1a2b3c4d").len(),
            8
        );
    }

    #[test]
    fn optional_path_and_worktree_id_validation() {
        assert!(optional_rel_path(&json!({})).unwrap().is_none());
        assert_eq!(
            optional_rel_path(&json!({ "path": "src/a.rs" })).unwrap(),
            Some("src/a.rs")
        );
        assert_eq!(
            optional_rel_path(&json!({ "path": "../x" }))
                .unwrap_err()
                .code(),
            "invalid_params"
        );
        assert_eq!(
            optional_rel_path(&json!({ "path": "/etc" }))
                .unwrap_err()
                .code(),
            "invalid_params"
        );
        assert_eq!(
            optional_rel_path(&json!({ "path": 1 })).unwrap_err().code(),
            "invalid_params"
        );
        assert!(optional_worktree_id(&json!({})).unwrap().is_none());
        assert_eq!(
            optional_worktree_id(&json!({ "worktreeId": "" }))
                .unwrap_err()
                .code(),
            "invalid_params"
        );
        assert_eq!(
            optional_worktree_id(&json!({ "worktreeId": 1 }))
                .unwrap_err()
                .code(),
            "invalid_params"
        );
        assert_eq!(
            require_str(&json!({}), "workspaceId").unwrap_err().code(),
            "invalid_params"
        );
        assert_eq!(
            require_str(&json!({ "workspaceId": "" }), "workspaceId")
                .unwrap_err()
                .code(),
            "invalid_params"
        );
        assert_eq!(
            require_str(&json!({ "workspaceId": 1 }), "workspaceId")
                .unwrap_err()
                .code(),
            "invalid_params"
        );
    }

    #[test]
    fn git_status_added_deleted_and_untracked_diff() {
        let (dir, svc) = setup();
        let ws_dir = dir.path().join("repo2");
        init_git_repo(&ws_dir);
        let (ws_id, _agent_id) = seed_agent(&svc, &ws_dir);
        std::fs::write(ws_dir.join("added.rs"), "fn a() {}\n").unwrap();
        git(&ws_dir, &["add", "added.rs"]);
        std::fs::remove_file(ws_dir.join("README.md")).unwrap();
        std::fs::write(ws_dir.join("fresh.txt"), "hello\n").unwrap();
        std::fs::write(ws_dir.join("nul.bin"), b"a\0b").unwrap();

        let status = svc.git_status(&json!({ "workspaceId": ws_id })).unwrap();
        assert!(status.dirty);
        assert!(status
            .entries
            .iter()
            .any(|e| e.path == "added.rs" && e.status == "added"));
        assert!(status
            .entries
            .iter()
            .any(|e| e.path == "README.md" && e.status == "deleted"));
        assert!(status
            .entries
            .iter()
            .any(|e| e.path == "fresh.txt" && e.status == "untracked"));

        let diff = svc.git_diff(&json!({ "workspaceId": ws_id })).unwrap();
        assert!(diff.files.iter().any(
            |f| f.path == "fresh.txt" && f.patch.as_ref().is_some_and(|p| p.contains("+hello"))
        ));
        assert!(diff
            .files
            .iter()
            .any(|f| f.path == "nul.bin" && f.patch.is_none()));

        let one = svc
            .git_diff(&json!({ "workspaceId": ws_id, "path": "fresh.txt" }))
            .unwrap();
        assert!(one.files.iter().all(|f| f.path == "fresh.txt"));
        let err = svc
            .git_diff(&json!({ "workspaceId": ws_id, "path": "../x" }))
            .unwrap_err();
        assert_eq!(err.code(), "invalid_params");
        let err = svc
            .git_status(&json!({ "workspaceId": ws_id, "worktreeId": "0191f0c6-7c2a-7c11-8000-6f0c1a2b3c4d" }))
            .unwrap_err();
        assert_eq!(err.code(), "not_found");
    }

    #[test]
    fn push_args_has_no_force() {
        let args = push_args("origin", "main");
        assert_eq!(args, vec!["push", "origin", "main"]);
        assert!(!args.iter().any(|a| a.contains("force")));
        assert!(!args.iter().any(|a| a == "--mirror" || a == "--tags"));
    }

    #[test]
    fn classify_push_stderr_auth_and_conflict() {
        let err = classify_push_stderr(
            "fatal: Authentication failed for 'https://user:hunter2@example.com/repo.git'",
        );
        assert_eq!(err.code(), "git_auth");
        let msg = err.to_string();
        assert!(!msg.contains("hunter2"), "{msg}");
        assert!(!msg.contains("user:hunter2"));

        let err = classify_push_stderr(
            "unable to access 'https://127.0.0.1:1/denied.git/': Failed to connect",
        );
        assert_eq!(err.code(), "git_auth");

        let err = classify_push_stderr("! [rejected] main -> main (non-fast-forward)");
        assert_eq!(err.code(), "git_conflict");

        let red =
            redact_secrets("clone https://octocat:ghp_abcdefghijklmnopqrstuvwxyz@github.com/x/y");
        assert!(!red.contains("ghp_abcdefgh"), "{red}");
        assert!(!red.contains("octocat:"), "{red}");
        assert!(red.contains("://***:***@"), "{red}");
    }

    #[test]
    fn parse_patch_stats_counts_files_and_hunks() {
        let patch = "diff --git a/src/a.rs b/src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1,1 +1,2 @@\n+x\n@@ -4,1 +5,1 @@\n+y\n";
        let (paths, hunks) = parse_patch_stats(&patch.replace("\\n", "\n"));
        assert_eq!(paths, vec!["src/a.rs"]);
        assert_eq!(hunks, 2);
    }

    #[test]
    fn parse_github_pr_number_accepts_canonical_urls() {
        assert_eq!(
            parse_github_pr_number("https://github.com/acme/repo/pull/90"),
            Some(90)
        );
        assert_eq!(
            parse_github_pr_number("github.com/acme/repo/pull/7/files"),
            Some(7)
        );
        assert_eq!(parse_github_pr_number("https://example.com/pull/1"), None);
        assert_eq!(parse_github_pr_number("https://github.com/acme/repo"), None);
        assert_eq!(parse_github_pr_number(""), None);
    }

    #[test]
    fn resolve_pr_selector_requires_number_or_url() {
        let err = resolve_pr_selector(&rt_protocol::PrGetParams {
            workspace_id: "w".into(),
            number: None,
            url: None,
        })
        .unwrap_err();
        assert_eq!(err.code(), "invalid_params");
        let err = resolve_pr_selector(&rt_protocol::PrGetParams {
            workspace_id: "w".into(),
            number: Some(1),
            url: Some("https://github.com/acme/repo/pull/2".into()),
        })
        .unwrap_err();
        assert_eq!(err.code(), "invalid_params");
        let ok = resolve_pr_selector(&rt_protocol::PrGetParams {
            workspace_id: "w".into(),
            number: Some(90),
            url: Some("https://github.com/acme/repo/pull/90".into()),
        })
        .unwrap();
        assert_eq!(ok.as_arg(), "90");
    }

    #[test]
    fn map_gh_pr_view_maps_checks_commits_files() {
        let raw = r#"{
            "number": 90,
            "url": "https://github.com/acme/repo/pull/90",
            "title": "feat: pr.get",
            "state": "OPEN",
            "commits": [{"oid":"abc123","messageHeadline":"feat: pr.get","authors":[{"name":"Valeriy","login":"v"}]}],
            "files": [{"path":"src/lib.rs","additions":3,"deletions":1,"changeType":"MODIFIED"}],
            "statusCheckRollup": [{"name":"ci","status":"COMPLETED","conclusion":"SUCCESS"}]
        }"#;
        let view: GhPrView = serde_json::from_str(raw).unwrap();
        let ok = map_gh_pr_view(view);
        assert_eq!(ok.number, 90);
        assert_eq!(ok.checks[0].name, "ci");
        assert_eq!(ok.checks[0].conclusion.as_deref(), Some("SUCCESS"));
        assert_eq!(ok.commits[0].sha, "abc123");
        assert_eq!(ok.commits[0].author.as_deref(), Some("Valeriy"));
        assert_eq!(ok.files[0].status, "MODIFIED");
        assert!(ok.diff.is_empty());
    }
}
