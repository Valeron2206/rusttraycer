//! HTTP JSON-RPC (`POST /rpc`), `GET /health`, `GET /metrics`, `GET /ws`.

use std::sync::Arc;

use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::extract::{FromRequest, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use rt_storage::{AgentStatus, Message};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::handshake::HandshakeParams;
use crate::service::HostService;
use crate::HostError;

#[derive(Clone)]
pub struct AppState {
    pub service: HostService,
}

pub fn router(service: HostService) -> Router {
    Router::new()
        .route("/rpc", post(rpc_handler))
        .route("/health", get(health_handler))
        .route("/metrics", get(metrics_handler))
        .route("/ws", get(ws_upgrade))
        .with_state(AppState { service })
}

#[derive(Debug, Deserialize)]
struct RpcRequest {
    id: String,
    method: String,
    #[serde(default)]
    params: Value,
}

fn rpc_ok(id: String, ok: Value) -> Json<Value> {
    Json(json!({ "id": id, "ok": ok }))
}

fn rpc_err(id: String, err: &HostError) -> Json<Value> {
    Json(json!({
        "id": id,
        "error": { "code": err.code(), "message": err.to_string() }
    }))
}

async fn health_handler(State(state): State<AppState>) -> Json<Value> {
    Json(json!({ "ok": true, "hostId": state.service.host_id() }))
}

const PROMETHEUS_CONTENT_TYPE: &str = "text/plain; version=0.0.4";

async fn metrics_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.service.prometheus_metrics() {
        Ok(body) => {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static(PROMETHEUS_CONTENT_TYPE),
            );
            (headers, body).into_response()
        }
        Err(e) => {
            tracing::error!(code = e.code(), error = %e, "metrics");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

fn session_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get("X-Rt-Session")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

fn require_session(state: &AppState, headers: &HeaderMap, method: &str) -> Result<(), HostError> {
    if method == "handshake" || method == "host.ping" {
        return Ok(());
    }
    let tok = session_token(headers).ok_or(HostError::Unauthorized)?;
    match state.service.session_accepts(&tok, method)? {
        Some(true) => Ok(()),
        Some(false) => {
            if crate::handshake::host_knows(method) {
                Err(HostError::VersionMismatch(method.to_string()))
            } else {
                Err(HostError::UnsupportedMethod(method.to_string()))
            }
        }
        None => Err(HostError::Unauthorized),
    }
}

async fn rpc_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<RpcRequest>,
) -> impl IntoResponse {
    tracing::debug!(method = %req.method, "rpc enter");
    if let Err(e) = require_session(&state, &headers, &req.method) {
        tracing::info!(method = %req.method, code = e.code(), "rpc error");
        return (StatusCode::OK, rpc_err(req.id, &e));
    }
    let token = session_token(&headers);
    match dispatch_method(&state.service, &req.method, req.params, token.as_deref()).await {
        Ok(ok) => {
            tracing::info!(method = %req.method, "rpc ok");
            (StatusCode::OK, rpc_ok(req.id, ok))
        }
        Err(e) => {
            tracing::info!(method = %req.method, code = e.code(), "rpc error");
            (StatusCode::OK, rpc_err(req.id, &e))
        }
    }
}

/// Test helper: dispatch with the ladder off (no session token).
#[cfg(test)]
async fn dispatch(svc: &HostService, method: &str, params: Value) -> Result<Value, HostError> {
    dispatch_method(svc, method, params, None).await
}

async fn dispatch_method(
    svc: &HostService,
    method: &str,
    params: Value,
    session_token: Option<&str>,
) -> Result<Value, HostError> {
    tracing::debug!(method, "dispatch");
    if !params.is_object() {
        return Err(HostError::InvalidParams("params must be an object".into()));
    }
    let result = match method {
        "handshake" => {
            let p: HandshakeParams = serde_json::from_value(params)?;
            Ok(serde_json::to_value(svc.handshake(p)?)?)
        }
        "host.ping" => Ok(serde_json::to_value(svc.ping())?),
        "host.doctor" => Ok(serde_json::to_value(svc.doctor()?)?),
        "workspace.list" => {
            let items = svc.workspace_list()?;
            Ok(json!({ "items": items }))
        }
        "workspace.add" => {
            let path = params
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| HostError::InvalidParams("path is required".into()))?;
            Ok(serde_json::to_value(svc.workspace_add(path)?)?)
        }
        "task.list" => {
            let status = params
                .get("status")
                .and_then(|v| v.as_str())
                .ok_or_else(|| HostError::InvalidParams("status is required".into()))?;
            let items = svc.task_list(status)?;
            Ok(json!({ "items": items }))
        }
        "task.create" => {
            let title = params
                .get("title")
                .and_then(|v| v.as_str())
                .ok_or_else(|| HostError::InvalidParams("title is required".into()))?;
            let workspace_id = params
                .get("workspaceId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| HostError::InvalidParams("workspaceId is required".into()))?;
            let preset = match params.get("preset") {
                None | Some(Value::Null) => None,
                Some(Value::String(s)) => Some(s.as_str()),
                Some(_) => {
                    return Err(HostError::InvalidParams("preset must be a string".into()));
                }
            };
            Ok(serde_json::to_value(svc.task_create_ex(
                title,
                workspace_id,
                preset,
            )?)?)
        }
        "task.get" => {
            let id = require_id(&params)?;
            Ok(serde_json::to_value(svc.task_get(id)?)?)
        }
        "task.rename" => {
            let id = require_id(&params)?;
            let title = params
                .get("title")
                .and_then(|v| v.as_str())
                .ok_or_else(|| HostError::InvalidParams("title is required".into()))?;
            Ok(serde_json::to_value(svc.task_rename(id, title)?)?)
        }
        "task.archive" => {
            let id = require_id(&params)?;
            Ok(serde_json::to_value(svc.task_archive(id)?)?)
        }
        "agent.list" => {
            let task_id = optional_id(&params, "taskId")?;
            let workspace_id = optional_id(&params, "workspaceId")?;
            let items = match (task_id, workspace_id) {
                (Some(task_id), _) => svc.agent_list(task_id)?,
                (None, Some(workspace_id)) => svc.agent_list_for_workspace(workspace_id)?,
                (None, None) => {
                    return Err(HostError::InvalidParams(
                        "workspaceId is required when taskId is omitted".into(),
                    ));
                }
            };
            Ok(json!({ "items": items }))
        }
        "agent.create" => {
            let task_id = optional_id(&params, "taskId")?;
            let workspace_id = optional_id(&params, "workspaceId")?;
            let provider = params.get("provider").and_then(|v| v.as_str());
            let interface = params.get("interface").and_then(|v| v.as_str());
            let launch_args = match params.get("launchArgs") {
                None | Some(Value::Null) => None,
                Some(Value::Array(arr)) => {
                    let mut out = Vec::new();
                    for v in arr {
                        let s = v.as_str().ok_or_else(|| {
                            HostError::InvalidParams("launchArgs must be strings".into())
                        })?;
                        out.push(s.to_string());
                    }
                    Some(out)
                }
                Some(_) => {
                    return Err(HostError::InvalidParams(
                        "launchArgs must be an array of strings".into(),
                    ));
                }
            };
            let parent_id = match params.get("parentId") {
                None | Some(Value::Null) => None,
                Some(Value::String(s)) => Some(s.clone()),
                Some(_) => {
                    return Err(HostError::InvalidParams("parentId must be a string".into()));
                }
            };
            let model = match params.get("model") {
                None | Some(Value::Null) => None,
                Some(Value::String(s)) => Some(s.clone()),
                Some(_) => {
                    return Err(HostError::InvalidParams("model must be a string".into()));
                }
            };
            let effort = match params.get("effort") {
                None | Some(Value::Null) => None,
                Some(Value::String(s)) => Some(s.clone()),
                Some(_) => {
                    return Err(HostError::InvalidParams("effort must be a string".into()));
                }
            };
            let fast = match params.get("fast") {
                None | Some(Value::Null) => None,
                Some(Value::Bool(b)) => Some(*b),
                Some(_) => {
                    return Err(HostError::InvalidParams("fast must be a boolean".into()));
                }
            };
            let role = match params.get("role") {
                None | Some(Value::Null) => None,
                Some(Value::String(s)) => Some(s.clone()),
                Some(_) => {
                    return Err(HostError::InvalidParams("role must be a string".into()));
                }
            };
            let account_id = match params.get("accountId") {
                None | Some(Value::Null) => None,
                Some(Value::String(s)) => Some(s.clone()),
                Some(_) => {
                    return Err(HostError::InvalidParams(
                        "accountId must be a string".into(),
                    ));
                }
            };
            Ok(serde_json::to_value(svc.agent_create_ex(
                crate::service::AgentCreateArgs {
                    task_id,
                    workspace_id,
                    provider,
                    interface,
                    launch_args,
                    parent_id: parent_id.as_deref(),
                    model,
                    effort,
                    fast,
                    role: role.as_deref(),
                    account_id: account_id.as_deref(),
                },
            )?)?)
        }
        "agent.get" => {
            let id = require_id(&params)?;
            Ok(serde_json::to_value(svc.agent_get(id)?)?)
        }
        "agent.send" => {
            let agent_id = params
                .get("agentId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| HostError::InvalidParams("agentId is required".into()))?;
            let content = params
                .get("content")
                .and_then(|v| v.as_str())
                .ok_or_else(|| HostError::InvalidParams("content is required".into()))?;
            let attached = params
                .get("path")
                .or_else(|| params.get("attachedPath"))
                .or_else(|| params.get("cwd"))
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(std::path::PathBuf::from);
            let ladder = match session_token {
                Some(t) => svc.session_accepts(t, rt_protocol::METHOD_POLICY_GET)? == Some(true),
                None => false,
            };
            let sent = svc.send_gated(agent_id, content, ladder, attached.as_deref())?;
            let mut ok = json!({ "userMessage": sent.user });
            if let Some(id) = sent.approval_id {
                ok["approvalId"] = json!(id);
            }
            Ok(ok)
        }
        "policy.get" => {
            let p: rt_protocol::PolicyGetParams = serde_json::from_value(params)
                .map_err(|e| HostError::InvalidParams(e.to_string()))?;
            tracing::info!("policy.get");
            Ok(serde_json::to_value(svc.policy_get(&p)?)?)
        }
        "policy.set" => {
            let p: rt_protocol::PolicySetParams = serde_json::from_value(params)
                .map_err(|e| HostError::InvalidParams(e.to_string()))?;
            tracing::info!(yolo = p.yolo, "policy.set");
            Ok(serde_json::to_value(svc.policy_set(&p)?)?)
        }
        "approval.respond" => {
            let p: rt_protocol::ApprovalRespondParams = serde_json::from_value(params)
                .map_err(|e| HostError::InvalidParams(e.to_string()))?;
            tracing::info!(approval_id = %p.approval_id, "approval.respond");
            Ok(serde_json::to_value(svc.approval_respond(&p)?)?)
        }
        "agent.get_context" => {
            let agent_id = params
                .get("agentId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| HostError::InvalidParams("agentId is required".into()))?;
            let messages = svc.get_context(agent_id)?;
            Ok(json!({ "messages": messages }))
        }
        "agent.cancel" => {
            let agent_id = params
                .get("agentId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| HostError::InvalidParams("agentId is required".into()))?;
            if uuid::Uuid::parse_str(agent_id).is_err() {
                return Err(HostError::InvalidParams("invalid agentId".into()));
            }
            Ok(serde_json::to_value(svc.cancel(agent_id)?)?)
        }
        "files.tree" => {
            let workspace_id = params
                .get("workspaceId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| HostError::InvalidParams("workspaceId is required".into()))?;
            let path = params.get("path").and_then(|v| v.as_str());
            let depth = params
                .get("depth")
                .and_then(|v| v.as_u64())
                .map(|n| n as u32);
            let max_entries = params
                .get("maxEntries")
                .and_then(|v| v.as_u64())
                .map(|n| n as u32);
            let worktree_id = params.get("worktreeId").and_then(|v| v.as_str());
            Ok(serde_json::to_value(svc.files_tree(
                workspace_id,
                path,
                depth,
                max_entries,
                worktree_id,
            )?)?)
        }
        "files.read" => {
            let workspace_id = params
                .get("workspaceId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| HostError::InvalidParams("workspaceId is required".into()))?;
            let path = params
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| HostError::InvalidParams("path is required".into()))?;
            let worktree_id = params.get("worktreeId").and_then(|v| v.as_str());
            Ok(serde_json::to_value(svc.files_read(
                workspace_id,
                path,
                worktree_id,
            )?)?)
        }
        "worktree.ensure" => {
            let agent_id = params
                .get("agentId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| HostError::InvalidParams("agentId is required".into()))?;
            Ok(serde_json::to_value(svc.worktree_ensure(agent_id)?)?)
        }
        "worktree.get" => {
            let agent_id = params
                .get("agentId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| HostError::InvalidParams("agentId is required".into()))?;
            Ok(serde_json::to_value(svc.worktree_get(agent_id)?)?)
        }
        "worktree.list" => {
            let workspace_id = params
                .get("workspaceId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| HostError::InvalidParams("workspaceId is required".into()))?;
            let items = svc.worktree_list(workspace_id)?;
            Ok(json!({ "items": items }))
        }
        "git.status" => Ok(serde_json::to_value(svc.git_status(&params)?)?),
        "git.diff" => Ok(serde_json::to_value(svc.git_diff(&params)?)?),
        "files.write" => {
            let ladder = match session_token {
                Some(t) => svc.session_accepts(t, rt_protocol::METHOD_POLICY_GET)? == Some(true),
                None => false,
            };
            Ok(svc.files_write_gated(&params, ladder)?)
        }
        "files.patch" => {
            let ladder = match session_token {
                Some(t) => svc.session_accepts(t, rt_protocol::METHOD_POLICY_GET)? == Some(true),
                None => false,
            };
            Ok(svc.files_patch_gated(&params, ladder)?)
        }
        "files.open" => Ok(svc.files_open(&params)?),
        "git.stage" => Ok(serde_json::to_value(svc.git_stage(&params)?)?),
        "git.unstage" => Ok(serde_json::to_value(svc.git_unstage(&params)?)?),
        "git.restore" => Ok(serde_json::to_value(svc.git_restore(&params)?)?),
        "git.commit" => Ok(serde_json::to_value(svc.git_commit(&params)?)?),
        "git.push" => Ok(serde_json::to_value(svc.git_push(&params).await?)?),
        "pty.open" => {
            let ladder = match session_token {
                Some(t) => svc.session_accepts(t, rt_protocol::METHOD_POLICY_GET)? == Some(true),
                None => false,
            };
            let agent_id = params.get("agentId").and_then(|v| v.as_str());
            let shell_id = params.get("shellId").and_then(|v| v.as_str());
            let cols = params
                .get("cols")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| HostError::InvalidParams("cols is required".into()))?;
            let rows = params
                .get("rows")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| HostError::InvalidParams("rows is required".into()))?;
            if cols > u16::MAX as u64 || rows > u16::MAX as u64 {
                return Err(HostError::InvalidParams(
                    "cols and rows must be in 1..=500".into(),
                ));
            }
            svc.pty_open(agent_id, shell_id, cols as u16, rows as u16, ladder)
        }
        "pty.write" => {
            let pty_id = params
                .get("ptyId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| HostError::InvalidParams("ptyId is required".into()))?;
            let data = params
                .get("data")
                .and_then(|v| v.as_str())
                .ok_or_else(|| HostError::InvalidParams("data is required".into()))?;
            svc.pty_write(pty_id, data)
        }
        "pty.resize" => {
            let pty_id = params
                .get("ptyId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| HostError::InvalidParams("ptyId is required".into()))?;
            let cols = params
                .get("cols")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| HostError::InvalidParams("cols is required".into()))?;
            let rows = params
                .get("rows")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| HostError::InvalidParams("rows is required".into()))?;
            if cols > u16::MAX as u64 || rows > u16::MAX as u64 {
                return Err(HostError::InvalidParams(
                    "cols and rows must be in 1..=500".into(),
                ));
            }
            svc.pty_resize(pty_id, cols as u16, rows as u16)
        }
        "pty.close" => {
            let pty_id = params
                .get("ptyId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| HostError::InvalidParams("ptyId is required".into()))?;
            svc.pty_close(pty_id)
        }
        "shell.create" => {
            let task_id = optional_id(&params, "taskId")?;
            let workspace_id = optional_id(&params, "workspaceId")?.ok_or_else(|| {
                HostError::InvalidParams("workspaceId is required when taskId is omitted".into())
            })?;
            let worktree_id = params.get("worktreeId").and_then(|v| v.as_str());
            let cols = params
                .get("cols")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| HostError::InvalidParams("cols is required".into()))?;
            let rows = params
                .get("rows")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| HostError::InvalidParams("rows is required".into()))?;
            if cols > u16::MAX as u64 || rows > u16::MAX as u64 {
                return Err(HostError::InvalidParams(
                    "cols and rows must be in 1..=500".into(),
                ));
            }
            svc.shell_create(task_id, workspace_id, worktree_id, cols as u16, rows as u16)
        }
        "shell.list" => {
            let task_id = optional_id(&params, "taskId")?;
            let workspace_id = optional_id(&params, "workspaceId")?;
            svc.shell_list(task_id, workspace_id)
        }
        "shell.close" => {
            let shell_id = params
                .get("shellId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| HostError::InvalidParams("shellId is required".into()))?;
            svc.shell_close(shell_id)
        }
        "artifact.create" => {
            let p: rt_protocol::ArtifactCreateParams = serde_json::from_value(params)
                .map_err(|e| HostError::InvalidParams(e.to_string()))?;
            Ok(serde_json::to_value(svc.artifact_create(&p)?)?)
        }
        "artifact.get" => {
            let p: rt_protocol::ArtifactGetParams = serde_json::from_value(params)
                .map_err(|e| HostError::InvalidParams(e.to_string()))?;
            Ok(serde_json::to_value(svc.artifact_get(&p.artifact_id)?)?)
        }
        "artifact.list" => {
            let p: rt_protocol::ArtifactListParams = serde_json::from_value(params)
                .map_err(|e| HostError::InvalidParams(e.to_string()))?;
            Ok(serde_json::to_value(
                svc.artifact_list(&p.task_id, p.kind.as_deref())?,
            )?)
        }
        "artifact.update" => Ok(serde_json::to_value(svc.artifact_update(&params)?)?),
        "artifact.delete" => {
            let p: rt_protocol::ArtifactDeleteParams = serde_json::from_value(params)
                .map_err(|e| HostError::InvalidParams(e.to_string()))?;
            Ok(serde_json::to_value(svc.artifact_delete(&p.artifact_id)?)?)
        }
        "artifact.export" => {
            let p: rt_protocol::ArtifactExportParams = serde_json::from_value(params)
                .map_err(|e| HostError::InvalidParams(e.to_string()))?;
            Ok(serde_json::to_value(
                svc.artifact_export(&p.artifact_id, &p.format)?,
            )?)
        }
        "comment.create" => {
            let p: rt_protocol::CommentCreateParams = serde_json::from_value(params)
                .map_err(|e| HostError::InvalidParams(e.to_string()))?;
            Ok(serde_json::to_value(svc.comment_create(&p)?)?)
        }
        "comment.list" => {
            let p: rt_protocol::CommentListParams = serde_json::from_value(params)
                .map_err(|e| HostError::InvalidParams(e.to_string()))?;
            Ok(serde_json::to_value(svc.comment_list(&p.artifact_id)?)?)
        }
        "comment.resolve" => {
            let p: rt_protocol::CommentResolveParams = serde_json::from_value(params)
                .map_err(|e| HostError::InvalidParams(e.to_string()))?;
            Ok(serde_json::to_value(svc.comment_resolve(&p.thread_id)?)?)
        }
        "agent.clear_transcript" => {
            let p: rt_protocol::ClearTranscriptParams = serde_json::from_value(params)
                .map_err(|e| HostError::InvalidParams(e.to_string()))?;
            Ok(serde_json::to_value(svc.clear_transcript(&p.agent_id)?)?)
        }
        "a2a.transcript" => {
            let p: rt_protocol::A2aTranscriptParams = serde_json::from_value(params)
                .map_err(|e| HostError::InvalidParams(e.to_string()))?;
            Ok(serde_json::to_value(svc.a2a_transcript(&p.agent_id)?)?)
        }
        "a2a.deliver" => {
            let p: rt_protocol::A2aDeliverParams = serde_json::from_value(params)
                .map_err(|e| HostError::InvalidParams(e.to_string()))?;
            Ok(serde_json::to_value(svc.a2a_deliver(
                &p.from_agent_id,
                &p.to_agent_id,
                &p.content,
            )?)?)
        }
        "loop.start" => {
            let max_iterations = params
                .get("maxIterations")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| HostError::InvalidParams("maxIterations is required".into()))?;
            if max_iterations > u32::MAX as u64 {
                return Err(HostError::InvalidParams(
                    "maxIterations must be 1..32".into(),
                ));
            }
            let budget_turns = match params.get("budgetTurns") {
                None | Some(Value::Null) => None,
                Some(v) => Some(v.as_u64().ok_or_else(|| {
                    HostError::InvalidParams("budgetTurns must be a number".into())
                })?),
            };
            if let Some(b) = budget_turns {
                if b > u32::MAX as u64 {
                    return Err(HostError::InvalidParams("budgetTurns must be 1..64".into()));
                }
            }
            let task_id = params
                .get("taskId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| HostError::InvalidParams("taskId is required".into()))?;
            let agent_ids = params
                .get("agentIds")
                .and_then(|v| v.as_array())
                .ok_or_else(|| HostError::InvalidParams("agentIds is required".into()))?;
            let mut ids = Vec::new();
            for v in agent_ids {
                let s = v
                    .as_str()
                    .ok_or_else(|| HostError::InvalidParams("agentIds must be strings".into()))?;
                ids.push(s.to_string());
            }
            let prompt = params
                .get("prompt")
                .and_then(|v| v.as_str())
                .ok_or_else(|| HostError::InvalidParams("prompt is required".into()))?;
            Ok(serde_json::to_value(svc.loop_start(
                task_id,
                &ids,
                max_iterations as u32,
                budget_turns.map(|b| b as u32),
                prompt,
            )?)?)
        }
        "loop.get" => {
            let p: rt_protocol::LoopGetParams = serde_json::from_value(params)
                .map_err(|e| HostError::InvalidParams(e.to_string()))?;
            Ok(serde_json::to_value(svc.loop_get(&p.loop_id)?)?)
        }
        "loop.stop" => {
            let p: rt_protocol::LoopStopParams = serde_json::from_value(params)
                .map_err(|e| HostError::InvalidParams(e.to_string()))?;
            Ok(serde_json::to_value(svc.loop_stop(&p.loop_id)?)?)
        }
        "agent.switch" => {
            let p: rt_protocol::AgentSwitchParams = serde_json::from_value(params)
                .map_err(|e| HostError::InvalidParams(e.to_string()))?;
            Ok(serde_json::to_value(svc.agent_switch(p)?)?)
        }
        "account.list" => Ok(serde_json::to_value(svc.account_list()?)?),
        "account.create" => {
            let p: rt_protocol::AccountCreateParams = serde_json::from_value(params)
                .map_err(|e| HostError::InvalidParams(e.to_string()))?;
            Ok(serde_json::to_value(
                svc.account_create(&p.provider, &p.label)?,
            )?)
        }
        "agent.steer" => {
            let p: rt_protocol::AgentSteerParams = serde_json::from_value(params)
                .map_err(|e| HostError::InvalidParams(e.to_string()))?;
            Ok(serde_json::to_value(
                svc.agent_steer(&p.agent_id, &p.content)?,
            )?)
        }
        "profile.create" => {
            let p: rt_protocol::ProfileCreateParams = serde_json::from_value(params)
                .map_err(|e| HostError::InvalidParams(e.to_string()))?;
            Ok(serde_json::to_value(svc.profile_create(p)?)?)
        }
        "profile.list" => Ok(serde_json::to_value(svc.profile_list()?)?),
        "profile.get" => {
            let p: rt_protocol::ProfileGetParams = serde_json::from_value(params)
                .map_err(|e| HostError::InvalidParams(e.to_string()))?;
            Ok(serde_json::to_value(svc.profile_get(&p)?)?)
        }
        "profile.update" => {
            let p: rt_protocol::ProfileUpdateParams = serde_json::from_value(params)
                .map_err(|e| HostError::InvalidParams(e.to_string()))?;
            Ok(serde_json::to_value(svc.profile_update(p)?)?)
        }
        "profile.delete" => {
            let p: rt_protocol::ProfileDeleteParams = serde_json::from_value(params)
                .map_err(|e| HostError::InvalidParams(e.to_string()))?;
            svc.profile_delete(&p)?;
            Ok(serde_json::json!({ "deleted": true }))
        }
        "prefs.get" => Ok(serde_json::to_value(svc.prefs_get()?)?),
        "workspace.guides.get" => {
            let p: rt_protocol::WorkspaceGuidesGetParams = serde_json::from_value(params)
                .map_err(|e| HostError::InvalidParams(e.to_string()))?;
            Ok(serde_json::to_value(
                svc.workspace_guides_get(&p.workspace_id)?,
            )?)
        }
        "settings.guide.get" => Ok(serde_json::to_value(svc.settings_guide_get())?),
        "settings.guide.set" => {
            let p: rt_protocol::SettingsGuideSetParams = serde_json::from_value(params)
                .map_err(|e| HostError::InvalidParams(e.to_string()))?;
            Ok(serde_json::to_value(svc.settings_guide_set(&p.content)?)?)
        }
        "preset.list" => Ok(serde_json::to_value(svc.preset_list())?),
        "agent.update" => {
            let p: rt_protocol::AgentUpdateParams = serde_json::from_value(params)
                .map_err(|e| HostError::InvalidParams(e.to_string()))?;
            Ok(serde_json::to_value(
                svc.agent_update(&p.agent_id, &p.role)?,
            )?)
        }
        "sync.export" => {
            let p: rt_protocol::SyncExportParams = serde_json::from_value(params)
                .map_err(|e| HostError::InvalidParams(e.to_string()))?;
            Ok(serde_json::to_value(svc.sync_export(&p.task_ids)?)?)
        }
        "sync.import" => {
            let p: rt_protocol::SyncImportParams = serde_json::from_value(params)
                .map_err(|e| HostError::InvalidParams(e.to_string()))?;
            Ok(serde_json::to_value(svc.sync_import(p)?)?)
        }
        "search.query" => {
            let p: rt_protocol::SearchQueryParams = serde_json::from_value(params)
                .map_err(|e| HostError::InvalidParams(e.to_string()))?;
            Ok(serde_json::to_value(svc.search_query(&p)?)?)
        }
        "worktree.gc" => {
            let p: rt_protocol::WorktreeGcParams = serde_json::from_value(params)
                .map_err(|e| HostError::InvalidParams(e.to_string()))?;
            Ok(serde_json::to_value(svc.worktree_gc(p.dry_run)?)?)
        }
        other => Err(HostError::UnsupportedMethod(other.to_string())),
    };
    match &result {
        Ok(_) => tracing::debug!(method, "rpc ok"),
        Err(e) => tracing::info!(method, code = e.code(), "rpc error"),
    }
    result
}

fn optional_id<'a>(params: &'a Value, key: &str) -> Result<Option<&'a str>, HostError> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.is_empty() => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.as_str())),
        Some(_) => Err(HostError::InvalidParams(format!("{key} must be a string"))),
    }
}

fn require_id(params: &Value) -> Result<&str, HostError> {
    params
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| HostError::InvalidParams("id is required".into()))
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "event")]
pub enum WsEvent {
    #[serde(rename = "agent.message")]
    AgentMessage {
        #[serde(rename = "taskId")]
        task_id: String,
        #[serde(rename = "agentId")]
        agent_id: String,
        message: Message,
    },
    #[serde(rename = "agent.status")]
    AgentStatus {
        #[serde(rename = "taskId")]
        task_id: String,
        #[serde(rename = "agentId")]
        agent_id: String,
        status: AgentStatus,
    },
    #[serde(rename = "task.updated")]
    TaskUpdated {
        #[serde(rename = "taskId")]
        task_id: String,
    },
    #[serde(rename = "host.going_away")]
    HostGoingAway {
        #[serde(rename = "hostId")]
        host_id: String,
    },
    #[serde(rename = "agent.approval")]
    AgentApproval {
        #[serde(rename = "approvalId")]
        approval_id: String,
        #[serde(rename = "agentId")]
        agent_id: String,
        #[serde(rename = "taskId")]
        task_id: String,
        kind: String,
        summary: String,
    },
    #[serde(rename = "pty.data")]
    PtyData {
        #[serde(rename = "taskId")]
        task_id: String,
        #[serde(rename = "ptyId")]
        pty_id: String,
        data: String,
    },
    #[serde(rename = "pty.exit")]
    PtyExit {
        #[serde(rename = "taskId")]
        task_id: String,
        #[serde(rename = "ptyId")]
        pty_id: String,
        code: i32,
    },
    #[serde(rename = "artifact.updated")]
    ArtifactUpdated {
        #[serde(rename = "artifactId")]
        artifact_id: String,
        #[serde(rename = "taskId")]
        task_id: String,
    },
    #[serde(rename = "artifact.deleted")]
    ArtifactDeleted {
        #[serde(rename = "artifactId")]
        artifact_id: String,
        #[serde(rename = "taskId")]
        task_id: String,
    },
    #[serde(rename = "a2a.delivered")]
    A2aDelivered {
        #[serde(rename = "fromAgentId")]
        from_agent_id: String,
        #[serde(rename = "toAgentId")]
        to_agent_id: String,
        #[serde(rename = "messageId")]
        message_id: String,
    },
    #[serde(rename = "loop.stopped")]
    LoopStopped {
        #[serde(rename = "loopId")]
        loop_id: String,
        reason: String,
    },
}

impl WsEvent {
    pub fn agent_message(task_id: &str, agent_id: &str, message: Message) -> Self {
        Self::AgentMessage {
            task_id: task_id.to_string(),
            agent_id: agent_id.to_string(),
            message,
        }
    }

    pub fn agent_status(task_id: &str, agent_id: &str, status: AgentStatus) -> Self {
        Self::AgentStatus {
            task_id: task_id.to_string(),
            agent_id: agent_id.to_string(),
            status,
        }
    }

    pub fn task_updated(task_id: &str) -> Self {
        Self::TaskUpdated {
            task_id: task_id.to_string(),
        }
    }

    pub fn host_going_away(host_id: &str) -> Self {
        Self::HostGoingAway {
            host_id: host_id.to_string(),
        }
    }

    pub fn agent_approval(
        approval_id: &str,
        agent_id: &str,
        task_id: &str,
        kind: &str,
        summary: &str,
    ) -> Self {
        Self::AgentApproval {
            approval_id: approval_id.to_string(),
            agent_id: agent_id.to_string(),
            task_id: task_id.to_string(),
            kind: kind.to_string(),
            summary: summary.to_string(),
        }
    }

    pub fn pty_data(task_id: &str, pty_id: &str, bytes: &[u8]) -> Self {
        use base64::Engine;
        Self::PtyData {
            task_id: task_id.to_string(),
            pty_id: pty_id.to_string(),
            data: base64::engine::general_purpose::STANDARD.encode(bytes),
        }
    }

    pub fn pty_exit(task_id: &str, pty_id: &str, code: i32) -> Self {
        Self::PtyExit {
            task_id: task_id.to_string(),
            pty_id: pty_id.to_string(),
            code,
        }
    }

    pub fn artifact_updated(artifact_id: &str, task_id: &str) -> Self {
        Self::ArtifactUpdated {
            artifact_id: artifact_id.to_string(),
            task_id: task_id.to_string(),
        }
    }

    pub fn artifact_deleted(artifact_id: &str, task_id: &str) -> Self {
        Self::ArtifactDeleted {
            artifact_id: artifact_id.to_string(),
            task_id: task_id.to_string(),
        }
    }

    pub fn a2a_delivered(from_agent_id: &str, to_agent_id: &str, message_id: &str) -> Self {
        Self::A2aDelivered {
            from_agent_id: from_agent_id.to_string(),
            to_agent_id: to_agent_id.to_string(),
            message_id: message_id.to_string(),
        }
    }

    pub fn loop_stopped(loop_id: &str, reason: &str) -> Self {
        Self::LoopStopped {
            loop_id: loop_id.to_string(),
            reason: reason.to_string(),
        }
    }

    pub fn task_id(&self) -> Option<&str> {
        match self {
            Self::AgentMessage { task_id, .. }
            | Self::AgentStatus { task_id, .. }
            | Self::TaskUpdated { task_id }
            | Self::AgentApproval { task_id, .. }
            | Self::PtyData { task_id, .. }
            | Self::PtyExit { task_id, .. }
            | Self::ArtifactUpdated { task_id, .. }
            | Self::ArtifactDeleted { task_id, .. } => Some(task_id.as_str()),
            Self::HostGoingAway { .. } | Self::A2aDelivered { .. } | Self::LoopStopped { .. } => {
                None
            }
        }
    }
}

async fn ws_upgrade(State(state): State<AppState>, request: Request) -> impl IntoResponse {
    // Auth first so a plain GET /ws (no Upgrade) still yields 401, not the
    // WebSocketUpgrade extractor's 400.
    let tok = request
        .headers()
        .get("X-Rt-Session")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let valid = match tok.as_deref() {
        None => false,
        Some(t) => match state.service.session_valid(t) {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": { "code": e.code(), "message": e.to_string() } })),
                )
                    .into_response();
            }
        },
    };
    if !valid {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": { "code": "unauthorized", "message": "unauthorized" } })),
        )
            .into_response();
    }
    match WebSocketUpgrade::from_request(request, &state).await {
        Ok(ws) => ws
            .on_upgrade(move |socket| handle_ws(state, socket))
            .into_response(),
        Err(rej) => rej.into_response(),
    }
}

async fn handle_ws(state: AppState, mut socket: WebSocket) {
    let filter: Arc<tokio::sync::Mutex<Option<Option<String>>>> =
        Arc::new(tokio::sync::Mutex::new(None));
    // None = not yet subscribed; Some(None) = all; Some(Some(id)) = one task
    let mut rx = state.service.subscribe_events();

    loop {
        tokio::select! {
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(WsMessage::Text(text))) => {
                        if let Ok(v) = serde_json::from_str::<Value>(&text) {
                            if v.get("type").and_then(|t| t.as_str()) == Some("subscribe") {
                                let task_id = match v.get("taskId") {
                                    None | Some(Value::Null) => None,
                                    Some(Value::String(s)) => Some(s.clone()),
                                    Some(_) => None,
                                };
                                *filter.lock().await = Some(task_id);
                            }
                        }
                    }
                    Some(Ok(WsMessage::Ping(p))) => {
                        let _ = socket.send(WsMessage::Pong(p)).await;
                    }
                    Some(Ok(WsMessage::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
            ev = rx.recv() => {
                match ev {
                    Ok(event) => {
                        let sub = filter.lock().await.clone();
                        let Some(task_filter) = sub else { continue };
                        if let (Some(want), Some(got)) = (task_filter.as_ref(), event.task_id()) {
                            if want != got {
                                continue;
                            }
                        }
                        if let Ok(text) = serde_json::to_string(&event) {
                            if socket.send(WsMessage::Text(text.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rt_storage::Store;
    use std::collections::HashMap;
    use tempfile::tempdir;

    fn test_service() -> (tempfile::TempDir, HostService) {
        let dir = tempdir().unwrap();
        let store = Store::open(dir.path().join("host.db")).unwrap();
        store.migrate().unwrap();
        let host_id = rt_storage::new_id();
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

    #[tokio::test]
    async fn dispatch_rejects_null_or_non_object_params() {
        let (_dir, svc) = test_service();
        for params in [Value::Null, json!([]), json!(1), json!("x")] {
            let err = dispatch(&svc, "host.ping", params).await.unwrap_err();
            assert_eq!(err.code(), "invalid_params");
            assert!(err.to_string().contains("params must be an object"));
        }
        let ok = dispatch(&svc, "host.ping", json!({})).await.unwrap();
        assert_eq!(ok["hostId"], svc.host_id());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn rpc_logs_method_enter_and_result() {
        use std::io::Write;
        use std::sync::{Arc, Mutex};

        let (_dir, svc) = test_service();
        let buf = Arc::new(Mutex::new(Vec::<u8>::new()));

        #[derive(Clone)]
        struct Capture(Arc<Mutex<Vec<u8>>>);
        impl Write for Capture {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(b);
                Ok(b.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Capture {
            type Writer = Capture;
            fn make_writer(&'a self) -> Self::Writer {
                self.clone()
            }
        }

        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_writer(Capture(buf.clone()))
            .with_ansi(false)
            .finish();
        let _guard = tracing::subscriber::set_default(subscriber);

        let _ = dispatch(&svc, "host.ping", json!({})).await.unwrap();
        let err = dispatch(&svc, "no.such", json!({})).await.unwrap_err();
        assert_eq!(err.code(), "unsupported_method");

        let text = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(text.contains("host.ping"), "enter/ok missing: {text}");
        assert!(text.contains("no.such"), "error method missing: {text}");
        assert!(
            text.contains("rpc ok") || text.contains("dispatch"),
            "result log missing: {text}"
        );
        assert!(
            text.contains("unsupported_method")
                || text.contains("rpc error")
                || text.contains("no.such"),
            "{text}"
        );
    }
    #[tokio::test]
    async fn dispatch_task_workspace_agent_methods() {
        let (_dir, svc) = test_service();
        let ws_dir = _dir.path().join("proj");
        std::fs::create_dir(&ws_dir).unwrap();
        std::fs::write(ws_dir.join("README.md"), b"hi\n").unwrap();

        let listed = dispatch(&svc, "workspace.list", json!({})).await.unwrap();
        assert!(listed["items"].as_array().unwrap().is_empty());

        let added = dispatch(
            &svc,
            "workspace.add",
            json!({ "path": ws_dir.to_str().unwrap() }),
        )
        .await
        .unwrap();
        let ws_id = added["id"].as_str().unwrap().to_string();

        let err = dispatch(&svc, "workspace.add", json!({}))
            .await
            .unwrap_err();
        assert_eq!(err.code(), "invalid_params");

        let err = dispatch(&svc, "task.list", json!({})).await.unwrap_err();
        assert_eq!(err.code(), "invalid_params");
        let tasks = dispatch(&svc, "task.list", json!({ "status": "open" }))
            .await
            .unwrap();
        assert!(tasks["items"].as_array().unwrap().is_empty());

        let err = dispatch(&svc, "task.create", json!({ "title": "t" }))
            .await
            .unwrap_err();
        assert_eq!(err.code(), "invalid_params");
        let created = dispatch(
            &svc,
            "task.create",
            json!({ "title": "t", "workspaceId": ws_id }),
        )
        .await
        .unwrap();
        let task_id = created["id"].as_str().unwrap().to_string();

        let got = dispatch(&svc, "task.get", json!({ "id": task_id }))
            .await
            .unwrap();
        assert_eq!(got["title"], "t");
        let renamed = dispatch(&svc, "task.rename", json!({ "id": task_id, "title": "t2" }))
            .await
            .unwrap();
        assert_eq!(renamed["title"], "t2");
        let err = dispatch(&svc, "task.rename", json!({ "id": task_id }))
            .await
            .unwrap_err();
        assert_eq!(err.code(), "invalid_params");
        let archived = dispatch(&svc, "task.archive", json!({ "id": task_id }))
            .await
            .unwrap();
        assert_eq!(archived["status"], "archived");

        let err = dispatch(&svc, "agent.list", json!({})).await.unwrap_err();
        assert_eq!(err.code(), "invalid_params");
        let agents = dispatch(&svc, "agent.list", json!({ "taskId": task_id }))
            .await
            .unwrap();
        assert!(agents["items"].as_array().unwrap().is_empty());

        let err = dispatch(&svc, "agent.create", json!({})).await.unwrap_err();
        assert_eq!(err.code(), "invalid_params");
        let agent = dispatch(&svc, "agent.create", json!({ "taskId": task_id }))
            .await
            .unwrap();
        let agent_id = agent["id"].as_str().unwrap().to_string();
        let got = dispatch(&svc, "agent.get", json!({ "id": agent_id }))
            .await
            .unwrap();
        assert_eq!(got["id"], agent_id);

        let err = dispatch(&svc, "agent.get_context", json!({}))
            .await
            .unwrap_err();
        assert_eq!(err.code(), "invalid_params");
        let ctx = dispatch(&svc, "agent.get_context", json!({ "agentId": agent_id }))
            .await
            .unwrap();
        assert!(ctx["messages"].as_array().unwrap().is_empty());

        let err = dispatch(&svc, "agent.send", json!({ "agentId": agent_id }))
            .await
            .unwrap_err();
        assert_eq!(err.code(), "invalid_params");
        let err = dispatch(&svc, "agent.send", json!({ "content": "hi" }))
            .await
            .unwrap_err();
        assert_eq!(err.code(), "invalid_params");

        let err = dispatch(&svc, "agent.cancel", json!({})).await.unwrap_err();
        assert_eq!(err.code(), "invalid_params");
        let err = dispatch(&svc, "agent.cancel", json!({ "agentId": "bad" }))
            .await
            .unwrap_err();
        assert_eq!(err.code(), "invalid_params");
        let cancel = dispatch(&svc, "agent.cancel", json!({ "agentId": agent_id }))
            .await
            .unwrap();
        assert_eq!(cancel["cancelled"], false);

        let err = dispatch(&svc, "worktree.ensure", json!({}))
            .await
            .unwrap_err();
        assert_eq!(err.code(), "invalid_params");
        let err = dispatch(&svc, "worktree.get", json!({})).await.unwrap_err();
        assert_eq!(err.code(), "invalid_params");
        let err = dispatch(&svc, "worktree.list", json!({}))
            .await
            .unwrap_err();
        assert_eq!(err.code(), "invalid_params");
        let err = dispatch(&svc, "worktree.get", json!({ "agentId": agent_id }))
            .await
            .unwrap_err();
        assert_eq!(err.code(), "not_found");
        let listed = dispatch(&svc, "worktree.list", json!({ "workspaceId": ws_id }))
            .await
            .unwrap();
        assert!(listed["items"].as_array().unwrap().is_empty());

        let err = dispatch(&svc, "git.status", json!({})).await.unwrap_err();
        assert_eq!(err.code(), "invalid_params");
        let err = dispatch(&svc, "git.diff", json!({ "workspaceId": ws_id }))
            .await
            .unwrap_err();
        assert_eq!(err.code(), "invalid_params");

        let err = dispatch(&svc, "files.tree", json!({})).await.unwrap_err();
        assert_eq!(err.code(), "invalid_params");
        let err = dispatch(&svc, "files.read", json!({ "workspaceId": ws_id }))
            .await
            .unwrap_err();
        assert_eq!(err.code(), "invalid_params");
        let tree = dispatch(
            &svc,
            "files.tree",
            json!({ "workspaceId": ws_id, "path": ".", "depth": 1, "maxEntries": 10 }),
        )
        .await
        .unwrap();
        assert!(tree["items"].is_array());
        let read = dispatch(
            &svc,
            "files.read",
            json!({ "workspaceId": ws_id, "path": "README.md" }),
        )
        .await
        .unwrap();
        assert_eq!(read["content"], "hi\n");

        let err = dispatch(&svc, "no.such", json!({})).await.unwrap_err();
        assert_eq!(err.code(), "unsupported_method");
        let err = dispatch(&svc, "task.get", json!({})).await.unwrap_err();
        assert_eq!(err.code(), "invalid_params");
    }

    #[test]
    fn ws_event_constructors_and_task_id() {
        let msg = Message {
            id: "m".into(),
            agent_id: "a".into(),
            role: rt_storage::MessageRole::User,
            content: "hi".into(),
            created_at: "c".into(),
        };
        let e = WsEvent::agent_message("t", "a", msg);
        assert_eq!(e.task_id(), Some("t"));
        let e = WsEvent::agent_status("t", "a", AgentStatus::Idle);
        assert_eq!(e.task_id(), Some("t"));
        let e = WsEvent::task_updated("t");
        assert_eq!(e.task_id(), Some("t"));
        let e = WsEvent::host_going_away("h");
        assert_eq!(e.task_id(), None);
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["event"], "host.going_away");
        assert_eq!(v["hostId"], "h");
        let e = WsEvent::agent_approval("ap1", "a", "t", "exec", "spawn cli.generic");
        assert_eq!(e.task_id(), Some("t"));
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["event"], "agent.approval");
        assert_eq!(v["approvalId"], "ap1");
        assert_eq!(v["agentId"], "a");
        assert_eq!(v["taskId"], "t");
        assert_eq!(v["kind"], "exec");
        assert_eq!(v["summary"], "spawn cli.generic");
        assert!(v.get("type").is_none());
    }

    #[tokio::test]
    async fn rpc_http_session_and_health() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;

        let (_dir, svc) = test_service();
        let hello = svc
            .handshake(crate::handshake::HandshakeParams {
                client: "cli".into(),
                client_version: "0.1.0".into(),
                methods: {
                    let mut m = std::collections::BTreeMap::new();
                    m.insert(
                        "host.doctor".into(),
                        rt_protocol::MethodVersion { major: 1, minor: 0 },
                    );
                    m
                },
            })
            .unwrap();
        let token = hello.session_token.clone();

        async fn post(
            app: axum::Router,
            token: Option<&str>,
            method: &str,
            params: Value,
        ) -> (StatusCode, Value) {
            let mut builder = Request::builder()
                .method("POST")
                .uri("/rpc")
                .header("content-type", "application/json");
            if let Some(t) = token {
                builder = builder.header("X-Rt-Session", t);
            }
            let req = builder
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "id": "1",
                        "method": method,
                        "params": params
                    }))
                    .unwrap(),
                ))
                .unwrap();
            let res = app.oneshot(req).await.unwrap();
            let status = res.status();
            let bytes = axum::body::to_bytes(res.into_body(), 1 << 20)
                .await
                .unwrap();
            (status, serde_json::from_slice(&bytes).unwrap())
        }

        let app = router(svc.clone());
        let health = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(health.status(), StatusCode::OK);

        let app = router(svc.clone());
        let (st, body) = post(app, None, "host.doctor", json!({})).await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(body["error"]["code"], "unauthorized");

        let app = router(svc.clone());
        let (st, body) = post(app, Some(&token), "workspace.list", json!({})).await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(body["error"]["code"], "version_mismatch");

        let app = router(svc.clone());
        let (st, body) = post(app, Some(&token), "artifact.create", json!({})).await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(body["error"]["code"], "version_mismatch");

        let app = router(svc.clone());
        let (st, body) = post(app, Some(&token), "host.doctor", json!({})).await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(body["ok"]["hostId"], svc.host_id());

        let app = router(svc.clone());
        let ws = app
            .oneshot(Request::builder().uri("/ws").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(ws.status(), StatusCode::UNAUTHORIZED);

        let app = router(svc);
        let ping = post(app, None, "host.ping", json!({})).await;
        assert_eq!(ping.0, StatusCode::OK);
        assert!(ping.1["ok"]["hostId"].is_string());
    }
}
