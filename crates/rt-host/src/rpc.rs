//! HTTP JSON-RPC (`POST /rpc`), `GET /health`, `GET /ws`.

use std::sync::Arc;

use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::extract::{FromRequest, Request, State};
use axum::http::{HeaderMap, StatusCode};
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
    match dispatch(&state.service, &req.method, req.params).await {
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

async fn dispatch(svc: &HostService, method: &str, params: Value) -> Result<Value, HostError> {
    tracing::debug!(method, "dispatch");
    let result = dispatch_method(svc, method, params).await;
    match &result {
        Ok(_) => tracing::debug!(method, "rpc ok"),
        Err(e) => tracing::info!(method, code = e.code(), "rpc error"),
    }
    result
}

async fn dispatch_method(
    svc: &HostService,
    method: &str,
    params: Value,
) -> Result<Value, HostError> {
    if !params.is_object() {
        return Err(HostError::InvalidParams("params must be an object".into()));
    }
    match method {
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
            Ok(serde_json::to_value(svc.task_create(title, workspace_id)?)?)
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
            let task_id = params
                .get("taskId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| HostError::InvalidParams("taskId is required".into()))?;
            let items = svc.agent_list(task_id)?;
            Ok(json!({ "items": items }))
        }
        "agent.create" => {
            let task_id = params
                .get("taskId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| HostError::InvalidParams("taskId is required".into()))?;
            let provider = params.get("provider").and_then(|v| v.as_str());
            Ok(serde_json::to_value(svc.agent_create(task_id, provider)?)?)
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
            let user = svc.send(agent_id, content)?;
            Ok(json!({ "userMessage": user }))
        }
        "agent.get_context" => {
            let agent_id = params
                .get("agentId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| HostError::InvalidParams("agentId is required".into()))?;
            let messages = svc.get_context(agent_id)?;
            Ok(json!({ "messages": messages }))
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
        other => Err(HostError::UnsupportedMethod(other.to_string())),
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

    pub fn task_id(&self) -> Option<&str> {
        match self {
            Self::AgentMessage { task_id, .. }
            | Self::AgentStatus { task_id, .. }
            | Self::TaskUpdated { task_id } => Some(task_id.as_str()),
            Self::HostGoingAway { .. } => None,
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

    #[tokio::test]
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
            text.contains("unsupported_method") || text.contains("rpc error"),
            "{text}"
        );
    }
}
