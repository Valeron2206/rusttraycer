//! E9 durable export/import (protocol 1.8) and C58 host-to-host push/pull.
//! Host does not write the archive file. Sync secret is env-only.

use std::collections::HashSet;

use rt_protocol::{
    ExportAgent, ExportArchive, ExportComment, ExportCommentThread, ExportTask, Profile,
    SyncExportOk, SyncImportOk, SyncImportParams, SyncPullParams, SyncPushParams, EXPORT_KIND,
    EXPORT_VERSION, MAX_EXPORT_TASKS,
};
use rt_storage::{
    ImportAgent, ImportArtifact, ImportBundle, ImportComment, ImportCommentThread, ImportMessage,
    ImportTask, ModelProfile, TaskFilter,
};
use serde::Deserialize;
use serde_json::Value;

use crate::service::HostService;
use crate::{HostError, Result};

impl HostService {
    pub fn sync_export(&self, task_ids: &[String]) -> Result<SyncExportOk> {
        validate_task_ids(task_ids)?;
        let mut tasks = Vec::with_capacity(task_ids.len());
        for id in task_ids {
            let task = self
                .store
                .task_get(id)?
                .ok_or_else(|| HostError::NotFound(format!("task {id}")))?;
            tasks.push(ExportTask {
                id: task.id,
                title: task.title,
                status: task.status.as_str().to_string(),
                created_at: task.created_at,
                updated_at: task.updated_at,
                preset: task.preset,
            });
        }

        let mut agents = Vec::new();
        let mut messages = Vec::new();
        let mut artifacts = Vec::new();
        let mut comment_threads = Vec::new();
        let mut comments = Vec::new();

        for task in &tasks {
            for agent in self.store.agent_list(&task.id)? {
                agents.push(ExportAgent {
                    id: agent.id.clone(),
                    task_id: agent.task_id,
                    parent_id: agent.parent_id,
                    interface: agent.interface,
                    provider: agent.provider.as_str().to_string(),
                    status: "idle".into(),
                    run_location: "local".into(),
                    created_at: agent.created_at,
                    model: agent.model,
                    effort: agent.effort,
                    fast: agent.fast,
                    role: if agent.role.is_empty() {
                        None
                    } else {
                        Some(agent.role)
                    },
                });
                for msg in self.store.message_list(&agent.id)? {
                    messages.push(rt_protocol::Message {
                        id: msg.id,
                        agent_id: msg.agent_id,
                        role: msg.role.as_str().to_string(),
                        content: msg.content,
                        created_at: msg.created_at,
                    });
                }
            }
            let (arts, _) = self.store.artifact_list(&task.id, None)?;
            artifacts.extend(arts.into_iter().map(storage_artifact_to_wire));
        }

        let message_ids: HashSet<&str> = messages.iter().map(|m| m.id.as_str()).collect();
        for art in &mut artifacts {
            if let Some(mid) = art.source_message_id.as_deref() {
                if !message_ids.contains(mid) {
                    art.source_message_id = None;
                }
            }
        }

        for art in &artifacts {
            for th in self.store.comment_list(&art.id)? {
                comment_threads.push(ExportCommentThread {
                    id: th.id.clone(),
                    artifact_id: th.artifact_id,
                    anchor_start: th.anchor_start,
                    anchor_end: th.anchor_end,
                    resolved: th.resolved,
                    created_at: th.created_at,
                    updated_at: th.updated_at,
                });
                for c in th.comments {
                    comments.push(ExportComment {
                        id: c.id,
                        thread_id: th.id.clone(),
                        body: c.body,
                        created_at: c.created_at,
                    });
                }
            }
        }

        let model_profiles = self
            .store
            .profile_list()?
            .into_iter()
            .map(profile_from_row)
            .collect();

        Ok(SyncExportOk {
            archive: ExportArchive {
                kind: EXPORT_KIND.to_string(),
                export_version: EXPORT_VERSION,
                source_host_id: self.host_id().to_string(),
                exported_at: rt_storage::now_rfc3339(),
                tasks,
                agents,
                messages,
                artifacts,
                comment_threads,
                comments,
                model_profiles,
            },
        })
    }

    pub fn sync_import(&self, params: SyncImportParams) -> Result<SyncImportOk> {
        if params.workspace_id.is_empty() {
            return Err(HostError::InvalidParams("workspaceId is required".into()));
        }
        if params.archive.kind != EXPORT_KIND || params.archive.export_version != EXPORT_VERSION {
            return Err(HostError::InvalidParams(
                "archive kind/exportVersion is invalid".into(),
            ));
        }
        match self.store.workspace_get(&params.workspace_id)? {
            Some(_) => {}
            None => {
                return Err(HostError::NotFound(format!(
                    "workspace {}",
                    params.workspace_id
                )))
            }
        }

        let bundle = archive_to_bundle(self.host_id(), &params.workspace_id, params.archive);
        let result = self.store.import_bundle(&bundle)?;
        Ok(SyncImportOk {
            tasks: u32_from_usize(result.tasks)?,
            agents: u32_from_usize(result.agents)?,
            messages: u32_from_usize(result.messages)?,
            artifacts: u32_from_usize(result.artifacts)?,
            profiles_imported: u32_from_usize(result.profiles_imported)?,
            profiles_skipped: u32_from_usize(result.profiles_skipped)?,
        })
    }

    pub fn resolve_export_task_ids(&self, task_ids: Option<&[String]>) -> Result<Vec<String>> {
        match task_ids {
            Some(ids) if !ids.is_empty() => Ok(ids.to_vec()),
            _ => {
                let all = self.store.task_list(TaskFilter::All)?;
                Ok(all.into_iter().map(|t| t.id).collect())
            }
        }
    }

    pub fn resolve_import_workspace(&self, workspace_id: Option<&str>) -> Result<String> {
        if let Some(id) = workspace_id {
            if !id.is_empty() {
                return Ok(id.to_string());
            }
        }
        let list = self.store.workspace_list()?;
        if list.len() == 1 {
            return Ok(list[0].id.clone());
        }
        Err(HostError::InvalidParams(
            "workspaceId is required when the host does not have exactly one workspace".into(),
        ))
    }

    pub async fn sync_push(&self, params: SyncPushParams) -> Result<SyncImportOk> {
        let peer = validate_peer_url(&params.peer_url)?;
        let task_ids = self.resolve_export_task_ids(params.task_ids.as_deref())?;
        let exported = self.sync_export(&task_ids)?;
        let mut body = serde_json::Map::new();
        if let Some(ws) = params.workspace_id.as_deref() {
            if !ws.is_empty() {
                body.insert("workspaceId".into(), Value::String(ws.to_string()));
            }
        }
        body.insert("archive".into(), serde_json::to_value(&exported.archive)?);
        let url = format!("{peer}/sync/v1/import");
        let resp = peer_post(&url, Value::Object(body)).await?;
        serde_json::from_value(resp).map_err(|e| HostError::Internal(e.to_string()))
    }

    pub async fn sync_pull(&self, params: SyncPullParams) -> Result<SyncImportOk> {
        let peer = validate_peer_url(&params.peer_url)?;
        if params.workspace_id.is_empty() {
            return Err(HostError::InvalidParams("workspaceId is required".into()));
        }
        let mut body = serde_json::Map::new();
        if let Some(ids) = params.task_ids.as_ref() {
            if !ids.is_empty() {
                body.insert("taskIds".into(), serde_json::to_value(ids)?);
            }
        }
        let url = format!("{peer}/sync/v1/export");
        let resp = peer_post(&url, Value::Object(body)).await?;
        let exported: PeerExportOk = serde_json::from_value(resp)
            .map_err(|e| HostError::InvalidParams(format!("peer export: {e}")))?;
        self.sync_import(SyncImportParams {
            workspace_id: params.workspace_id,
            archive: exported.archive,
        })
    }
}

fn validate_task_ids(task_ids: &[String]) -> Result<()> {
    if task_ids.is_empty() || task_ids.len() > MAX_EXPORT_TASKS {
        return Err(HostError::InvalidParams(format!(
            "taskIds must be 1..={MAX_EXPORT_TASKS}"
        )));
    }
    let mut seen = HashSet::new();
    for id in task_ids {
        if id.is_empty() {
            return Err(HostError::InvalidParams("taskIds must be non-empty".into()));
        }
        if !seen.insert(id.as_str()) {
            return Err(HostError::InvalidParams("taskIds must be unique".into()));
        }
    }
    Ok(())
}

fn storage_artifact_to_wire(a: rt_storage::Artifact) -> rt_protocol::Artifact {
    rt_protocol::Artifact {
        id: a.id,
        task_id: a.task_id,
        parent_id: a.parent_id,
        kind: a.kind,
        title: a.title,
        body: a.body,
        status: a.status,
        assignee: a.assignee,
        source_message_id: a.source_message_id,
        created_at: a.created_at,
        updated_at: a.updated_at,
    }
}

fn profile_from_row(row: ModelProfile) -> Profile {
    Profile {
        id: row.id,
        name: row.name,
        provider: row.provider,
        model: row.model,
        effort: row.effort,
        fast: row.fast,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn archive_to_bundle(
    dest_host_id: &str,
    dest_workspace_id: &str,
    archive: ExportArchive,
) -> ImportBundle {
    let agent_ids: HashSet<String> = archive.agents.iter().map(|a| a.id.clone()).collect();
    ImportBundle {
        dest_host_id: dest_host_id.to_string(),
        dest_workspace_id: dest_workspace_id.to_string(),
        tasks: archive
            .tasks
            .into_iter()
            .map(|t| ImportTask {
                id: t.id,
                title: t.title,
                status: t.status,
                created_at: t.created_at,
                updated_at: t.updated_at,
                preset: t.preset,
            })
            .collect(),
        agents: archive
            .agents
            .into_iter()
            .map(|a| {
                let parent_id = a.parent_id.filter(|pid| agent_ids.contains(pid));
                ImportAgent {
                    id: a.id,
                    task_id: a.task_id,
                    parent_id,
                    interface: a.interface,
                    provider: a.provider,
                    created_at: a.created_at,
                    model: a.model,
                    effort: a.effort,
                    fast: a.fast,
                    role: a.role.unwrap_or_else(|| "coder".into()),
                }
            })
            .collect(),
        messages: archive
            .messages
            .into_iter()
            .map(|m| ImportMessage {
                id: m.id,
                agent_id: m.agent_id,
                role: m.role,
                content: m.content,
                created_at: m.created_at,
            })
            .collect(),
        artifacts: archive
            .artifacts
            .into_iter()
            .map(|a| ImportArtifact {
                id: a.id,
                task_id: a.task_id,
                parent_id: a.parent_id,
                kind: a.kind,
                title: a.title,
                body: a.body,
                status: a.status,
                assignee: a.assignee,
                source_message_id: a.source_message_id,
                created_at: a.created_at,
                updated_at: a.updated_at,
            })
            .collect(),
        comment_threads: archive
            .comment_threads
            .into_iter()
            .map(|t| ImportCommentThread {
                id: t.id,
                artifact_id: t.artifact_id,
                anchor_start: t.anchor_start,
                anchor_end: t.anchor_end,
                resolved: t.resolved,
                created_at: t.created_at,
                updated_at: t.updated_at,
            })
            .collect(),
        comments: archive
            .comments
            .into_iter()
            .map(|c| ImportComment {
                id: c.id,
                thread_id: c.thread_id,
                body: c.body,
                created_at: c.created_at,
            })
            .collect(),
        profiles: archive
            .model_profiles
            .into_iter()
            .map(|p| ModelProfile {
                id: p.id,
                name: p.name,
                provider: p.provider,
                model: p.model,
                effort: p.effort,
                fast: p.fast,
                created_at: p.created_at,
                updated_at: p.updated_at,
            })
            .collect(),
    }
}

fn u32_from_usize(n: usize) -> Result<u32> {
    u32::try_from(n).map_err(|_| HostError::Internal("import count overflow".into()))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PeerExportOk {
    archive: ExportArchive,
}

#[derive(Debug, Deserialize)]
struct PeerErrorBody {
    error: rt_protocol::ErrorBody,
}

pub(crate) fn sync_secret_from_env() -> Option<String> {
    match std::env::var("RUSTTRAYCER_SYNC_SECRET") {
        Ok(s) if !s.is_empty() => Some(s),
        _ => None,
    }
}

pub(crate) fn secrets_equal(expected: &str, provided: &str) -> bool {
    let a = expected.as_bytes();
    let b = provided.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut acc = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        acc |= x ^ y;
    }
    acc == 0
}

fn validate_peer_url(raw: &str) -> Result<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(HostError::InvalidParams("peerUrl is required".into()));
    }
    let parsed = reqwest::Url::parse(raw).map_err(|_| {
        HostError::InvalidParams("peerUrl must be an absolute http or https URL".into())
    })?;
    match parsed.scheme() {
        "http" | "https" => {}
        _ => {
            return Err(HostError::InvalidParams(
                "peerUrl must be http or https".into(),
            ));
        }
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| HostError::InvalidParams("peerUrl must include a host".into()))?;
    if host.is_empty() {
        return Err(HostError::InvalidParams(
            "peerUrl must include a host".into(),
        ));
    }
    let host_l = host.to_ascii_lowercase();
    if host_l.contains("traycer.ai")
        || host_l.contains("traycer.com")
        || host_l.contains("sync.traycer")
    {
        return Err(HostError::InvalidParams(
            "managed cloud sync is not supported".into(),
        ));
    }
    let mut s = parsed.as_str().to_string();
    if s.ends_with('/') {
        s.pop();
    }
    Ok(s)
}

fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| HostError::Internal(format!("http client: {e}")))
}

async fn peer_post(url: &str, body: Value) -> Result<Value> {
    let client = http_client()?;
    let mut req = client.post(url).json(&body);
    if let Some(secret) = sync_secret_from_env() {
        req = req.header("X-Rt-Sync-Secret", secret);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| HostError::Internal(format!("peer request failed: {e}")))?;
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| HostError::Internal(format!("peer body: {e}")))?;
    if status == reqwest::StatusCode::UNAUTHORIZED {
        if let Ok(parsed) = serde_json::from_str::<PeerErrorBody>(&text) {
            return Err(HostError::AuthRequired(parsed.error.message));
        }
        return Err(HostError::AuthRequired("peer rejected sync secret".into()));
    }
    if let Ok(parsed) = serde_json::from_str::<PeerErrorBody>(&text) {
        return Err(map_peer_code(&parsed.error.code, parsed.error.message));
    }
    if !status.is_success() {
        return Err(HostError::Internal(format!(
            "peer HTTP {}: {text}",
            status.as_u16()
        )));
    }
    serde_json::from_str(&text).map_err(|e| HostError::Internal(format!("peer JSON: {e}")))
}

fn map_peer_code(code: &str, message: String) -> HostError {
    match code {
        "auth_required" => HostError::AuthRequired(message),
        "invalid_params" => HostError::InvalidParams(message),
        "not_found" => HostError::NotFound(message),
        "conflict" => HostError::Conflict(message),
        "unauthorized" => HostError::Unauthorized,
        _ => HostError::Internal(format!("peer {code}: {message}")),
    }
}
