//! E9 durable export/import (protocol 1.8). Host does not write the archive file.

use std::collections::HashSet;

use rt_protocol::{
    ExportAgent, ExportArchive, ExportComment, ExportCommentThread, ExportTask, Profile,
    SyncExportOk, SyncImportOk, SyncImportParams, EXPORT_KIND, EXPORT_VERSION, MAX_EXPORT_TASKS,
};
use rt_storage::{
    ImportAgent, ImportArtifact, ImportBundle, ImportComment, ImportCommentThread, ImportMessage,
    ImportTask, ModelProfile,
};

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
