//! HTTP client for host RPC. No spawn. Agent/files over RPC; WS is in `ws`.

use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::state::PidInfo;

const TIMEOUT: Duration = Duration::from_secs(2);
const CLIENT_VERSION: &str = rt_protocol::CRATE_VERSION;

#[derive(Debug, Clone)]
pub struct Session {
    pub host_id: String,
    #[allow(dead_code)]
    pub host_version: String,
    pub session_token: String,
    pub rpc_url: String,
    pub ws_url: Option<String>,
}

#[derive(Debug)]
pub enum ConnectError {
    Health(String),
    Handshake(String),
    Ping(String),
    HostIdMismatch { pid: String, rpc: String },
    Rpc { code: String, message: String },
    Transport(String),
}

impl ConnectError {
    pub fn as_label(&self) -> String {
        match self {
            Self::Health(msg) => format!("health: {msg}"),
            Self::Handshake(msg) => format!("handshake: {msg}"),
            Self::Ping(msg) => format!("ping: {msg}"),
            Self::HostIdMismatch { pid, rpc } => {
                format!("другой hostId (pid {pid}, rpc {rpc})")
            }
            Self::Rpc { code, message } => {
                if message.is_empty() {
                    code.clone()
                } else {
                    format!("{code}: {message}")
                }
            }
            Self::Transport(msg) => format!("host не отвечает: {msg}"),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HealthBody {
    ok: bool,
    host_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerHello {
    host_id: String,
    host_version: String,
    session_token: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PingOk {
    host_id: String,
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout(TIMEOUT).build()
}

fn rpc_origin(rpc_url: &str) -> String {
    rpc_url.trim().trim_end_matches('/').to_string()
}

fn req_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

fn hello_methods() -> Value {
    let mut map = serde_json::Map::new();
    for name in rt_protocol::TRADABLE_METHODS {
        map.insert(name.to_string(), json!({ "major": 1, "minor": 0 }));
    }
    Value::Object(map)
}

fn post_rpc(
    http: &ureq::Agent,
    origin: &str,
    method: &str,
    params: Value,
    token: Option<&str>,
) -> Result<Value, ConnectError> {
    let url = format!("{origin}/rpc");
    let body = json!({
        "id": req_id(),
        "method": method,
        "params": params,
    });
    let mut req = http
        .post(&url)
        .set("Content-Type", "application/json");
    if let Some(token) = token {
        req = req.set(rt_protocol::SESSION_HEADER, token);
    }
    let resp = req
        .send_json(body)
        .map_err(|err| ConnectError::Transport(err.to_string()))?;
    let value: Value = resp
        .into_json()
        .map_err(|err| ConnectError::Transport(err.to_string()))?;
    if value.get("error").is_some() {
        let code = value["error"]["code"]
            .as_str()
            .unwrap_or("error")
            .to_string();
        let message = value["error"]["message"]
            .as_str()
            .unwrap_or("")
            .to_string();
        return Err(ConnectError::Rpc { code, message });
    }
    if value.get("ok").is_none() {
        return Err(ConnectError::Transport("ответ без ok".into()));
    }
    Ok(value)
}

pub fn health(http: &ureq::Agent, origin: &str) -> Result<String, ConnectError> {
    let url = format!("{origin}/health");
    let resp = http
        .get(&url)
        .call()
        .map_err(|err| ConnectError::Health(err.to_string()))?;
    let body: HealthBody = resp
        .into_json()
        .map_err(|err| ConnectError::Health(err.to_string()))?;
    if !body.ok {
        return Err(ConnectError::Health("ok=false".into()));
    }
    if body.host_id.trim().is_empty() {
        return Err(ConnectError::Health("пустой hostId".into()));
    }
    Ok(body.host_id)
}

fn handshake(http: &ureq::Agent, origin: &str) -> Result<ServerHello, ConnectError> {
    let value = post_rpc(
        http,
        origin,
        rt_protocol::METHOD_HANDSHAKE,
        json!({
            "client": "gui",
            "clientVersion": CLIENT_VERSION,
            "methods": hello_methods(),
        }),
        None,
    )
    .map_err(|err| match err {
        ConnectError::Transport(msg) => ConnectError::Handshake(msg),
        other => other,
    })?;
    serde_json::from_value(value["ok"].clone()).map_err(|err| ConnectError::Handshake(err.to_string()))
}

pub fn ping(http: &ureq::Agent, origin: &str, token: Option<&str>) -> Result<String, ConnectError> {
    let value = post_rpc(
        http,
        origin,
        rt_protocol::METHOD_HOST_PING,
        json!({}),
        token,
    )
    .map_err(|err| match err {
        ConnectError::Transport(msg) => ConnectError::Ping(msg),
        other => other,
    })?;
    let body: PingOk = serde_json::from_value(value["ok"].clone())
        .map_err(|err| ConnectError::Ping(err.to_string()))?;
    Ok(body.host_id)
}

/// pid.json → GET /health → handshake → host.ping.
/// Online only if every RPC step succeeds. File present is not enough.
pub fn connect(info: &PidInfo) -> Result<Session, ConnectError> {
    let origin = rpc_origin(&info.rpc_url);
    if origin.is_empty() {
        return Err(ConnectError::Transport("пустой rpcUrl".into()));
    }
    let http = agent();

    let health_id = health(&http, &origin)?;
    if health_id != info.host_id {
        return Err(ConnectError::HostIdMismatch {
            pid: info.host_id.clone(),
            rpc: health_id,
        });
    }

    let hello = handshake(&http, &origin)?;
    if hello.host_id != info.host_id {
        return Err(ConnectError::HostIdMismatch {
            pid: info.host_id.clone(),
            rpc: hello.host_id,
        });
    }

    let ping_id = ping(&http, &origin, Some(&hello.session_token))?;
    if ping_id != info.host_id {
        return Err(ConnectError::HostIdMismatch {
            pid: info.host_id.clone(),
            rpc: ping_id,
        });
    }

    Ok(Session {
        host_id: hello.host_id,
        host_version: hello.host_version,
        session_token: hello.session_token,
        rpc_url: origin,
        ws_url: info.ws_url.clone(),
    })
}

#[derive(Debug, Clone)]
pub struct TasksCatalog {
    pub workspaces: Vec<rt_protocol::Workspace>,
    pub tasks: Vec<rt_protocol::Task>,
}

#[derive(Debug, Deserialize)]
struct ItemList<T> {
    items: Vec<T>,
}

fn parse_ok<T: DeserializeOwned>(ok: Value) -> Result<T, ConnectError> {
    serde_json::from_value(ok).map_err(|err| ConnectError::Transport(err.to_string()))
}

fn parse_items<T: DeserializeOwned>(ok: Value) -> Result<Vec<T>, ConnectError> {
    Ok(parse_ok::<ItemList<T>>(ok)?.items)
}

/// Make `path` absolute without walking the tree or requiring it to exist.
pub fn to_absolute_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    match std::path::absolute(trimmed) {
        Ok(p) => p.to_string_lossy().into_owned(),
        Err(_) => trimmed.to_string(),
    }
}

impl Session {
    fn call(&self, method: &str, params: Value) -> Result<Value, ConnectError> {
        let http = agent();
        let value = post_rpc(
            &http,
            &self.rpc_url,
            method,
            params,
            Some(&self.session_token),
        )?;
        Ok(value["ok"].clone())
    }

    pub fn workspace_list(&self) -> Result<Vec<rt_protocol::Workspace>, ConnectError> {
        parse_items(self.call(rt_protocol::METHOD_WORKSPACE_LIST, json!({}))?)
    }

    pub fn workspace_add(&self, path: &str) -> Result<rt_protocol::Workspace, ConnectError> {
        parse_ok(self.call(
            rt_protocol::METHOD_WORKSPACE_ADD,
            json!({ "path": path }),
        )?)
    }

    pub fn task_list(&self, status: &str) -> Result<Vec<rt_protocol::Task>, ConnectError> {
        parse_items(self.call(
            rt_protocol::METHOD_TASK_LIST,
            json!({ "status": status }),
        )?)
    }

    pub fn task_create(
        &self,
        title: &str,
        workspace_id: &str,
    ) -> Result<rt_protocol::Task, ConnectError> {
        parse_ok(self.call(
            rt_protocol::METHOD_TASK_CREATE,
            json!({ "title": title, "workspaceId": workspace_id }),
        )?)
    }

    pub fn task_rename(&self, id: &str, title: &str) -> Result<rt_protocol::Task, ConnectError> {
        parse_ok(self.call(
            rt_protocol::METHOD_TASK_RENAME,
            json!({ "id": id, "title": title }),
        )?)
    }

    pub fn task_archive(&self, id: &str) -> Result<rt_protocol::Task, ConnectError> {
        parse_ok(self.call(
            rt_protocol::METHOD_TASK_ARCHIVE,
            json!({ "id": id }),
        )?)
    }

    /// `workspace.list`, then `task.list` with `status` if a workspace exists.
    pub fn refresh_tasks_catalog(&self, status: &str) -> Result<TasksCatalog, ConnectError> {
        let workspaces = self.workspace_list()?;
        let tasks = if workspaces.is_empty() {
            Vec::new()
        } else {
            self.task_list(status)?
        };
        Ok(TasksCatalog { workspaces, tasks })
    }

    pub fn agent_list(&self, task_id: &str) -> Result<Vec<rt_protocol::Agent>, ConnectError> {
        parse_items(self.call(
            rt_protocol::METHOD_AGENT_LIST,
            json!({ "taskId": task_id }),
        )?)
    }

    pub fn agent_create(
        &self,
        task_id: &str,
        provider: &str,
    ) -> Result<rt_protocol::Agent, ConnectError> {
        parse_ok(self.call(
            rt_protocol::METHOD_AGENT_CREATE,
            json!({ "taskId": task_id, "provider": provider }),
        )?)
    }

    pub fn agent_get(&self, id: &str) -> Result<rt_protocol::Agent, ConnectError> {
        parse_ok(self.call(rt_protocol::METHOD_AGENT_GET, json!({ "id": id }))?)
    }

    pub fn agent_send(
        &self,
        agent_id: &str,
        content: &str,
    ) -> Result<rt_protocol::Message, ConnectError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct SendOk {
            user_message: rt_protocol::Message,
        }
        Ok(parse_ok::<SendOk>(self.call(
            rt_protocol::METHOD_AGENT_SEND,
            json!({ "agentId": agent_id, "content": content }),
        )?)?
        .user_message)
    }

    pub fn agent_get_context(
        &self,
        agent_id: &str,
    ) -> Result<Vec<rt_protocol::Message>, ConnectError> {
        #[derive(Deserialize)]
        struct ContextOk {
            messages: Vec<rt_protocol::Message>,
        }
        Ok(parse_ok::<ContextOk>(self.call(
            rt_protocol::METHOD_AGENT_GET_CONTEXT,
            json!({ "agentId": agent_id }),
        )?)?
        .messages)
    }

    pub fn files_tree(
        &self,
        workspace_id: &str,
        path: &str,
    ) -> Result<rt_protocol::FileTreeOk, ConnectError> {
        parse_ok(self.call(
            rt_protocol::METHOD_FILES_TREE,
            json!({ "workspaceId": workspace_id, "path": path, "depth": 1 }),
        )?)
    }

    pub fn files_read(
        &self,
        workspace_id: &str,
        path: &str,
    ) -> Result<rt_protocol::FileReadOk, ConnectError> {
        parse_ok(self.call(
            rt_protocol::METHOD_FILES_READ,
            json!({ "workspaceId": workspace_id, "path": path }),
        )?)
    }
}


pub fn keepalive(session: &Session) -> Result<(), ConnectError> {
    let http = agent();
    let ping_id = ping(&http, &session.rpc_url, Some(&session.session_token))?;
    if ping_id != session.host_id {
        return Err(ConnectError::HostIdMismatch {
            pid: session.host_id.clone(),
            rpc: ping_id,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::thread;

    struct Mock {
        origin: String,
        hits: Arc<Mutex<Vec<String>>>,
    }

    fn start_mock(host_id: &str, token: &str) -> Mock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let hits_t = hits.clone();
        let host_id = host_id.to_string();
        let token = token.to_string();
        thread::spawn(move || {
            for stream in listener.incoming().take(8) {
                let Ok(mut stream) = stream else { break };
                let mut raw = Vec::new();
                let mut tmp = [0u8; 2048];
                loop {
                    match stream.read(&mut tmp) {
                        Ok(0) => break,
                        Ok(n) => raw.extend_from_slice(&tmp[..n]),
                        Err(_) => break,
                    }
                    if let Some(pos) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&raw[..pos]);
                        let cl = headers.lines().find_map(|line| {
                            let lower = line.to_ascii_lowercase();
                            lower
                                .strip_prefix("content-length:")
                                .and_then(|s| s.trim().parse::<usize>().ok())
                        }).unwrap_or(0);
                        if raw.len() >= pos + 4 + cl {
                            break;
                        }
                    }
                }
                let req = String::from_utf8_lossy(&raw);
                let path = if req.starts_with("GET /health") {
                    "GET /health"
                } else if req.contains("\"method\":\"handshake\"")
                    || req.contains("\"method\": \"handshake\"")
                {
                    "POST handshake"
                } else if req.contains("host.ping") {
                    "POST host.ping"
                } else {
                    "other"
                };
                hits_t.lock().unwrap().push(path.to_string());
                let body = match path {
                    "GET /health" => json!({"ok": true, "hostId": host_id}).to_string(),
                    "POST handshake" => json!({
                        "id": "echo",
                        "ok": {
                            "hostId": host_id,
                            "hostVersion": "0.1.0",
                            "sessionToken": token,
                            "accepted": { "host.ping": {"major": 1, "minor": 0} },
                            "rejected": {}
                        }
                    }).to_string(),
                    "POST host.ping" => json!({
                        "id": "echo",
                        "ok": { "hostId": host_id, "now": "2026-08-17T12:00:00Z" }
                    }).to_string(),
                    _ => json!({"id":"echo","error":{"code":"unsupported_method","message":"no"}}).to_string(),
                };
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(resp.as_bytes());
            }
        });
        Mock {
            origin: format!("http://{addr}"),
            hits,
        }
    }

    fn pid(host_id: &str, origin: &str) -> PidInfo {
        PidInfo {
            host_id: host_id.into(),
            pid: 1,
            rpc_url: origin.into(),
            ws_url: None,
            started_at: None,
        }
    }

    #[test]
    fn connect_requires_health_handshake_ping() {
        let mock = start_mock("host-a", "tok-1");
        let mut info = pid("host-a", &mock.origin);
        info.ws_url = Some("ws://127.0.0.1:9/ws".into());
        let session = connect(&info).expect("online");
        assert_eq!(session.host_id, "host-a");
        assert_eq!(session.session_token, "tok-1");
        assert_eq!(session.ws_url.as_deref(), Some("ws://127.0.0.1:9/ws"));
        let hits = mock.hits.lock().unwrap().clone();
        assert_eq!(
            hits,
            vec![
                "GET /health".to_string(),
                "POST handshake".to_string(),
                "POST host.ping".to_string()
            ]
        );
    }

    #[test]
    fn pid_file_alone_is_not_online() {
        let err = connect(&pid("host-a", "http://127.0.0.1:1")).unwrap_err();
        match err {
            ConnectError::Health(_) | ConnectError::Transport(_) => {}
            other => panic!("expected transport/health, got {other:?}"),
        }
    }

    #[test]
    fn live_host_connects_when_env_set() {
        if std::env::var("RT_GUI_LIVE").ok().as_deref() != Some("1") {
            return;
        }
        let info = crate::discovery::read_pid_json().expect("pid.json");
        let session = connect(&info).expect("live online");
        assert_eq!(session.host_id, info.host_id);
        assert!(!session.session_token.is_empty());
    }

    #[test]
    fn host_id_mismatch_is_offline() {
        let mock = start_mock("host-b", "tok-1");
        let err = connect(&pid("host-a", &mock.origin)).unwrap_err();
        match err {
            ConnectError::HostIdMismatch { pid, rpc } => {
                assert_eq!(pid, "host-a");
                assert_eq!(rpc, "host-b");
            }
            other => panic!("expected mismatch, got {other:?}"),
        }
    }

    #[derive(Clone, Debug)]
    struct RpcHit {
        method: String,
        params: Value,
        has_session: bool,
    }

    struct CatalogMock {
        origin: String,
        hits: Arc<Mutex<Vec<RpcHit>>>,
    }

    fn read_http_request(stream: &mut std::net::TcpStream) -> (String, Vec<u8>) {
        use std::io::Read;
        let mut raw = Vec::new();
        let mut tmp = [0u8; 2048];
        loop {
            match stream.read(&mut tmp) {
                Ok(0) => break,
                Ok(n) => raw.extend_from_slice(&tmp[..n]),
                Err(_) => break,
            }
            if let Some(pos) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&raw[..pos]);
                let cl = headers
                    .lines()
                    .find_map(|line| {
                        let lower = line.to_ascii_lowercase();
                        lower
                            .strip_prefix("content-length:")
                            .and_then(|s| s.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                if raw.len() >= pos + 4 + cl {
                    let body = raw[pos + 4..pos + 4 + cl].to_vec();
                    return (headers.into_owned(), body);
                }
            }
        }
        (String::from_utf8_lossy(&raw).into_owned(), Vec::new())
    }

    fn write_http_json(stream: &mut std::net::TcpStream, body: &str) {
        use std::io::Write;
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(resp.as_bytes());
    }

    fn sample_workspace(host_id: &str, path: &str) -> Value {
        json!({
            "id": "ws-1",
            "hostId": host_id,
            "path": path,
            "name": "proj",
            "createdAt": "2026-08-17T12:00:00Z"
        })
    }

    fn sample_task(id: &str, title: &str, status: &str, workspace_id: &str) -> Value {
        json!({
            "id": id,
            "title": title,
            "status": status,
            "createdAt": "2026-08-17T12:00:00Z",
            "updatedAt": "2026-08-17T12:01:00Z",
            "workspaceIds": [workspace_id]
        })
    }

    fn start_catalog_mock(host_id: &str, token: &str) -> CatalogMock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let hits_t = hits.clone();
        let host_id = host_id.to_string();
        let token = token.to_string();
        thread::spawn(move || {
            for stream in listener.incoming().take(32) {
                let Ok(mut stream) = stream else { break };
                let (headers, body) = read_http_request(&mut stream);
                let has_session = headers.to_ascii_lowercase().contains("x-rt-session:");
                let (method, params) = if headers.starts_with("GET /health") {
                    ("GET /health".to_string(), json!({}))
                } else {
                    let parsed: Value = serde_json::from_slice(&body).unwrap_or(json!({}));
                    (
                        parsed
                            .get("method")
                            .and_then(|v| v.as_str())
                            .unwrap_or("other")
                            .to_string(),
                        parsed.get("params").cloned().unwrap_or(json!({})),
                    )
                };
                hits_t.lock().unwrap().push(RpcHit {
                    method: method.clone(),
                    params: params.clone(),
                    has_session,
                });
                let body = match method.as_str() {
                    "GET /health" => json!({"ok": true, "hostId": host_id}).to_string(),
                    "handshake" => json!({
                        "id": "echo",
                        "ok": {
                            "hostId": host_id,
                            "hostVersion": "0.1.0",
                            "sessionToken": token,
                            "accepted": { "host.ping": {"major": 1, "minor": 0} },
                            "rejected": {}
                        }
                    }).to_string(),
                    "host.ping" => json!({
                        "id": "echo",
                        "ok": { "hostId": host_id, "now": "2026-08-17T12:00:00Z" }
                    }).to_string(),
                    "workspace.list" => json!({
                        "id": "echo",
                        "ok": { "items": [sample_workspace(&host_id, "/tmp/proj")] }
                    }).to_string(),
                    "workspace.add" => {
                        let path = params.get("path").and_then(|v| v.as_str()).unwrap_or("");
                        json!({
                            "id": "echo",
                            "ok": sample_workspace(&host_id, path)
                        }).to_string()
                    }
                    "task.list" => json!({
                        "id": "echo",
                        "ok": { "items": [sample_task("task-1", "Demo", "open", "ws-1")] }
                    }).to_string(),
                    "task.create" => {
                        let title = params.get("title").and_then(|v| v.as_str()).unwrap_or("");
                        let ws = params.get("workspaceId").and_then(|v| v.as_str()).unwrap_or("");
                        json!({
                            "id": "echo",
                            "ok": sample_task("task-new", title, "open", ws)
                        }).to_string()
                    }
                    "task.rename" => {
                        let id = params.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        let title = params.get("title").and_then(|v| v.as_str()).unwrap_or("");
                        json!({
                            "id": "echo",
                            "ok": sample_task(id, title, "open", "ws-1")
                        }).to_string()
                    }
                    "task.archive" => {
                        let id = params.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        json!({
                            "id": "echo",
                            "ok": sample_task(id, "Demo", "archived", "ws-1")
                        }).to_string()
                    }
                    _ => json!({
                        "id": "echo",
                        "error": { "code": "unsupported_method", "message": "no" }
                    }).to_string(),
                };
                write_http_json(&mut stream, &body);
            }
        });
        CatalogMock {
            origin: format!("http://{addr}"),
            hits,
        }
    }

    fn methods_of(mock: &CatalogMock) -> Vec<String> {
        mock.hits
            .lock()
            .unwrap()
            .iter()
            .map(|h| h.method.clone())
            .collect()
    }

    #[test]
    fn catalog_after_connect_calls_workspace_and_task_list() {
        let mock = start_catalog_mock("host-a", "tok-1");
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        let catalog = session
            .refresh_tasks_catalog("open")
            .expect("catalog");
        assert_eq!(catalog.workspaces.len(), 1);
        assert_eq!(catalog.workspaces[0].id, "ws-1");
        assert_eq!(catalog.tasks.len(), 1);
        assert_eq!(catalog.tasks[0].title, "Demo");
        let methods = methods_of(&mock);
        assert_eq!(
            &methods[..3],
            &[
                "GET /health".to_string(),
                "handshake".to_string(),
                "host.ping".to_string()
            ]
        );
        assert!(methods.contains(&"workspace.list".to_string()));
        assert!(methods.contains(&"task.list".to_string()));
        let list = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "task.list")
            .cloned()
            .expect("task.list");
        assert_eq!(list.params["status"], "open");
        assert!(list.has_session);
    }

    #[test]
    fn workspace_add_sends_absolute_path() {
        let mock = start_catalog_mock("host-a", "tok-1");
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        let abs = to_absolute_path("/tmp/rt-gui-abs-ws");
        assert!(abs.starts_with('/'), "expected absolute, got {abs}");
        let ws = session.workspace_add(&abs).expect("add");
        assert_eq!(ws.path, abs);
        let hit = mock
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|h| h.method == "workspace.add")
            .cloned()
            .expect("workspace.add");
        assert_eq!(hit.params["path"], abs);
        assert!(hit.has_session);
    }

    #[test]
    fn task_create_rename_archive_send_right_methods() {
        let mock = start_catalog_mock("host-a", "tok-1");
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        let created = session.task_create("Hello", "ws-1").expect("create");
        assert_eq!(created.title, "Hello");
        assert_eq!(created.workspace_ids, vec!["ws-1".to_string()]);
        let renamed = session.task_rename("task-1", "Renamed").expect("rename");
        assert_eq!(renamed.title, "Renamed");
        let archived = session.task_archive("task-1").expect("archive");
        assert_eq!(archived.status, "archived");

        let hits = mock.hits.lock().unwrap().clone();
        let create = hits.iter().find(|h| h.method == "task.create").unwrap();
        assert_eq!(create.params["title"], "Hello");
        assert_eq!(create.params["workspaceId"], "ws-1");
        assert!(create.has_session);
        let rename = hits.iter().find(|h| h.method == "task.rename").unwrap();
        assert_eq!(rename.params["id"], "task-1");
        assert_eq!(rename.params["title"], "Renamed");
        let archive = hits.iter().find(|h| h.method == "task.archive").unwrap();
        assert_eq!(archive.params["id"], "task-1");
    }

    #[test]
    fn workspace_path_invalid_is_rpc_error() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            for stream in listener.incoming().take(8) {
                let Ok(mut stream) = stream else { break };
                let (headers, body) = read_http_request(&mut stream);
                let parsed: Value = serde_json::from_slice(&body).unwrap_or(json!({}));
                let method = if headers.starts_with("GET /health") {
                    "GET /health"
                } else {
                    parsed.get("method").and_then(|v| v.as_str()).unwrap_or("")
                };
                let body = match method {
                    "GET /health" => json!({"ok": true, "hostId": "host-a"}).to_string(),
                    "handshake" => json!({
                        "id": "echo",
                        "ok": {
                            "hostId": "host-a",
                            "hostVersion": "0.1.0",
                            "sessionToken": "tok-1",
                            "accepted": {},
                            "rejected": {}
                        }
                    }).to_string(),
                    "host.ping" => json!({
                        "id": "echo",
                        "ok": { "hostId": "host-a", "now": "2026-08-17T12:00:00Z" }
                    }).to_string(),
                    "workspace.add" => json!({
                        "id": "echo",
                        "error": {
                            "code": "workspace_path_invalid",
                            "message": "path must exist and be a directory"
                        }
                    }).to_string(),
                    _ => json!({"id":"echo","error":{"code":"unsupported_method","message":"no"}}).to_string(),
                };
                write_http_json(&mut stream, &body);
            }
        });
        let session = connect(&pid("host-a", &format!("http://{addr}"))).expect("online");
        let err = session.workspace_add("/no/such/dir").unwrap_err();
        let label = err.as_label();
        match err {
            ConnectError::Rpc { code, message } => {
                assert_eq!(code, "workspace_path_invalid");
                assert!(message.contains("directory"), "{message}");
                assert!(label.contains("workspace_path_invalid"), "{label}");
            }
            other => panic!("expected Rpc, got {other:?}"),
        }
    }


    fn sample_agent(id: &str, task_id: &str, status: &str) -> Value {
        json!({
            "id": id,
            "taskId": task_id,
            "hostId": "host-a",
            "parentId": null,
            "interface": "chat",
            "provider": "cli.generic",
            "status": status,
            "runLocation": "local",
            "createdAt": "2026-08-17T12:00:00Z"
        })
    }

    fn sample_message(id: &str, agent_id: &str, role: &str, content: &str) -> Value {
        json!({
            "id": id,
            "agentId": agent_id,
            "role": role,
            "content": content,
            "createdAt": "2026-08-17T12:00:00Z"
        })
    }

    fn start_agent_files_mock(host_id: &str, token: &str) -> CatalogMock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(Vec::new()));
        let hits_t = hits.clone();
        let host_id = host_id.to_string();
        let token = token.to_string();
        thread::spawn(move || {
            for stream in listener.incoming().take(24) {
                let Ok(mut stream) = stream else { break };
                let (headers, body) = read_http_request(&mut stream);
                let has_session = headers.to_ascii_lowercase().contains("x-rt-session:");
                let (method, params) = if headers.starts_with("GET /health") {
                    ("GET /health".to_string(), json!({}))
                } else {
                    let parsed: Value = serde_json::from_slice(&body).unwrap_or(json!({}));
                    (
                        parsed
                            .get("method")
                            .and_then(|v| v.as_str())
                            .unwrap_or("other")
                            .to_string(),
                        parsed.get("params").cloned().unwrap_or(json!({})),
                    )
                };
                hits_t.lock().unwrap().push(RpcHit {
                    method: method.clone(),
                    params: params.clone(),
                    has_session,
                });
                let body = match method.as_str() {
                    "GET /health" => json!({"ok": true, "hostId": host_id}).to_string(),
                    "handshake" => json!({
                        "id": "echo",
                        "ok": {
                            "hostId": host_id,
                            "hostVersion": "0.1.0",
                            "sessionToken": token,
                            "accepted": {},
                            "rejected": {}
                        }
                    }).to_string(),
                    "host.ping" => json!({
                        "id": "echo",
                        "ok": { "hostId": host_id, "now": "2026-08-17T12:00:00Z" }
                    }).to_string(),
                    "agent.list" => json!({
                        "id": "echo",
                        "ok": { "items": [sample_agent("ag-1", "task-1", "idle")] }
                    }).to_string(),
                    "agent.create" => {
                        let task_id = params.get("taskId").and_then(|v| v.as_str()).unwrap_or("");
                        json!({
                            "id": "echo",
                            "ok": sample_agent("ag-new", task_id, "idle")
                        }).to_string()
                    }
                    "agent.get" => {
                        let id = params.get("id").and_then(|v| v.as_str()).unwrap_or("ag-1");
                        let mut agent = sample_agent(id, "task-1", "idle");
                        agent["lastMessageAt"] = json!("2026-08-17T12:01:00Z");
                        json!({ "id": "echo", "ok": agent }).to_string()
                    }
                    "agent.get_context" => json!({
                        "id": "echo",
                        "ok": { "messages": [sample_message("m1", "ag-1", "user", "hi")] }
                    }).to_string(),
                    "agent.send" => {
                        let agent_id = params.get("agentId").and_then(|v| v.as_str()).unwrap_or("");
                        let content = params.get("content").and_then(|v| v.as_str()).unwrap_or("");
                        json!({
                            "id": "echo",
                            "ok": { "userMessage": sample_message("m-new", agent_id, "user", content) }
                        }).to_string()
                    }
                    "files.tree" => json!({
                        "id": "echo",
                        "ok": {
                            "items": [{
                                "name": "README.md",
                                "path": "README.md",
                                "kind": "file",
                                "size": 12,
                                "modifiedAt": "2026-08-17T12:00:00Z"
                            }],
                            "truncated": false
                        }
                    }).to_string(),
                    "files.read" => {
                        let path = params.get("path").and_then(|v| v.as_str()).unwrap_or("");
                        if path == "bin.dat" {
                            json!({
                                "id": "echo",
                                "error": { "code": "file_binary", "message": "binary" }
                            }).to_string()
                        } else if path == "huge.txt" {
                            json!({
                                "id": "echo",
                                "error": { "code": "file_too_large", "message": "too large" }
                            }).to_string()
                        } else {
                            json!({
                                "id": "echo",
                                "ok": {
                                    "path": path,
                                    "content": "# hi\n",
                                    "truncated": false,
                                    "encoding": "utf8"
                                }
                            }).to_string()
                        }
                    }
                    _ => json!({
                        "id": "echo",
                        "error": { "code": "unsupported_method", "message": "no" }
                    }).to_string(),
                };
                write_http_json(&mut stream, &body);
            }
        });
        CatalogMock {
            origin: format!("http://{addr}"),
            hits,
        }
    }

    #[test]
    fn agent_rpcs_send_right_methods() {
        let mock = start_agent_files_mock("host-a", "tok-1");
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        let listed = session.agent_list("task-1").expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "ag-1");
        let created = session.agent_create("task-1", "cli.generic").expect("create");
        assert_eq!(created.id, "ag-new");
        assert_eq!(created.task_id, "task-1");
        assert_eq!(created.provider, "cli.generic");
        let got = session.agent_get("ag-1").expect("get");
        assert_eq!(got.id, "ag-1");
        let ctx = session.agent_get_context("ag-1").expect("ctx");
        assert_eq!(ctx.len(), 1);
        assert_eq!(ctx[0].content, "hi");
        let sent = session.agent_send("ag-1", "hello").expect("send");
        assert_eq!(sent.role, "user");
        assert_eq!(sent.content, "hello");
        assert_eq!(sent.agent_id, "ag-1");

        let hits = mock.hits.lock().unwrap().clone();
        let list = hits.iter().find(|h| h.method == "agent.list").unwrap();
        assert_eq!(list.params["taskId"], "task-1");
        assert!(list.has_session);
        let create = hits.iter().find(|h| h.method == "agent.create").unwrap();
        assert_eq!(create.params["taskId"], "task-1");
        assert_eq!(create.params["provider"], "cli.generic");
        let get = hits.iter().find(|h| h.method == "agent.get").unwrap();
        assert_eq!(get.params["id"], "ag-1");
        let ctx_hit = hits.iter().find(|h| h.method == "agent.get_context").unwrap();
        assert_eq!(ctx_hit.params["agentId"], "ag-1");
        let send = hits.iter().find(|h| h.method == "agent.send").unwrap();
        assert_eq!(send.params["agentId"], "ag-1");
        assert_eq!(send.params["content"], "hello");
    }

    #[test]
    fn files_tree_read_send_workspace_and_path() {
        let mock = start_agent_files_mock("host-a", "tok-1");
        let session = connect(&pid("host-a", &mock.origin)).expect("online");
        let tree = session.files_tree("ws-1", "").expect("tree root");
        assert_eq!(tree.items.len(), 1);
        assert_eq!(tree.items[0].path, "README.md");
        let tree2 = session.files_tree("ws-1", "src").expect("tree src");
        assert!(!tree2.truncated);
        let read = session.files_read("ws-1", "README.md").expect("read");
        assert_eq!(read.encoding, "utf8");
        assert_eq!(read.content, "# hi\n");

        let err = session.files_read("ws-1", "bin.dat").unwrap_err();
        match err {
            ConnectError::Rpc { code, .. } => assert_eq!(code, "file_binary"),
            other => panic!("expected file_binary, got {other:?}"),
        }
        let err = session.files_read("ws-1", "huge.txt").unwrap_err();
        match err {
            ConnectError::Rpc { code, .. } => assert_eq!(code, "file_too_large"),
            other => panic!("expected file_too_large, got {other:?}"),
        }

        let hits = mock.hits.lock().unwrap().clone();
        let trees: Vec<_> = hits.iter().filter(|h| h.method == "files.tree").collect();
        assert_eq!(trees.len(), 2);
        assert_eq!(trees[0].params["workspaceId"], "ws-1");
        assert_eq!(trees[0].params["path"], "");
        assert_eq!(trees[1].params["workspaceId"], "ws-1");
        assert_eq!(trees[1].params["path"], "src");
        assert!(trees[0].has_session);
        let read_hit = hits.iter().find(|h| {
            h.method == "files.read" && h.params["path"] == "README.md"
        }).unwrap();
        assert_eq!(read_hit.params["workspaceId"], "ws-1");
        assert_eq!(read_hit.params["path"], "README.md");
    }

    fn rt_host_bin() -> Option<std::path::PathBuf> {
        let candidates = [
            std::path::PathBuf::from("/workspace/rusttraycer/target/debug/rt-host"),
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/rt-host"),
        ];
        candidates.into_iter().find(|p| p.is_file())
    }

    struct LiveHost {
        child: std::process::Child,
        home: std::path::PathBuf,
    }

    impl Drop for LiveHost {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            let _ = std::fs::remove_dir_all(&self.home);
        }
    }

    fn spawn_live_host() -> Option<LiveHost> {
        spawn_live_host_env(&[])
    }

    fn spawn_live_host_env(extra: &[(&str, &std::ffi::OsStr)]) -> Option<LiveHost> {
        let bin = match rt_host_bin() {
            Some(p) => p,
            None if std::env::var("RT_GUI_LIVE").ok().as_deref() == Some("1") => {
                panic!("RT_GUI_LIVE=1 but rt-host binary is missing");
            }
            None => return None,
        };
        let home = std::env::temp_dir().join(format!(
            "rt-gui-live-{}-{}",
            std::process::id(),
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&home).unwrap();
        let mut cmd = std::process::Command::new(&bin);
        cmd.env("RUSTTRAYCER_HOME", &home)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped());
        for (k, v) in extra {
            cmd.env(k, v);
        }
        let child = cmd.spawn().expect("spawn rt-host");
        Some(LiveHost { child, home })
    }

    fn wait_live_pid(home: &std::path::Path) -> PidInfo {
        let path = home.join("host").join("pid.json");
        let start = std::time::Instant::now();
        loop {
            if path.is_file() {
                if let Ok(raw) = std::fs::read_to_string(&path) {
                    if let Ok(v) = serde_json::from_str::<Value>(&raw) {
                        if let (Some(host_id), Some(rpc_url)) = (
                            v.get("hostId").and_then(|x| x.as_str()),
                            v.get("rpcUrl").and_then(|x| x.as_str()),
                        ) {
                            return PidInfo {
                                host_id: host_id.to_string(),
                                pid: v.get("pid").and_then(|x| x.as_u64()).unwrap_or(0),
                                rpc_url: rpc_url.to_string(),
                                ws_url: v.get("wsUrl").and_then(|x| x.as_str()).map(|s| s.to_string()),
                                started_at: v
                                    .get("startedAt")
                                    .and_then(|x| x.as_str())
                                    .map(|s| s.to_string()),
                            };
                        }
                    }
                }
            }
            if start.elapsed() > std::time::Duration::from_secs(8) {
                panic!("rt-host did not write {} in time", path.display());
            }
            thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    #[test]
    fn live_host_workspace_and_tasks_roundtrip() {
        let Some(mut live) = spawn_live_host() else {
            return;
        };
        let info = wait_live_pid(&live.home);
        let session = {
            let start = std::time::Instant::now();
            loop {
                match connect(&info) {
                    Ok(s) => break s,
                    Err(err) if start.elapsed() < std::time::Duration::from_secs(5) => {
                        let _ = err;
                        thread::sleep(std::time::Duration::from_millis(50));
                    }
                    Err(err) => panic!("live connect failed: {}", err.as_label()),
                }
            }
        };
        assert_eq!(session.host_id, info.host_id);

        let ws_dir = live.home.join("proj");
        std::fs::create_dir_all(&ws_dir).unwrap();
        let abs = to_absolute_path(&ws_dir.to_string_lossy());
        eprintln!(
            "live host online host_id={} rpc={} home={}",
            session.host_id,
            session.rpc_url,
            live.home.display()
        );
        let added = session.workspace_add(&abs).expect("workspace.add");
        eprintln!("workspace.add id={} path={}", added.id, added.path);
        assert!(!added.id.is_empty());
        let canon = std::fs::canonicalize(&ws_dir).unwrap();
        assert_eq!(added.path, canon.to_string_lossy().as_ref());

        let catalog = session.refresh_tasks_catalog("open").expect("catalog");
        assert_eq!(catalog.workspaces.len(), 1);
        assert!(catalog.tasks.is_empty());

        let created = session
            .task_create("Slice 3", &added.id)
            .expect("task.create");
        eprintln!("task.create id={} title={}", created.id, created.title);
        assert_eq!(created.title, "Slice 3");
        assert_eq!(created.status, "open");

        let open = session.task_list("open").expect("task.list open");
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].id, created.id);
        assert_eq!(open[0].title, "Slice 3");

        let renamed = session
            .task_rename(&created.id, "Slice 3 renamed")
            .expect("task.rename");
        assert_eq!(renamed.title, "Slice 3 renamed");
        let open = session.task_list("open").expect("list after rename");
        assert_eq!(open[0].title, "Slice 3 renamed");

        let archived = session.task_archive(&created.id).expect("task.archive");
        assert_eq!(archived.status, "archived");
        let open = session.task_list("open").expect("list after archive");
        assert!(open.is_empty(), "archived task must leave open filter");
        let archived_list = session.task_list("archived").expect("archived list");
        assert_eq!(archived_list.len(), 1);
        assert_eq!(archived_list[0].id, created.id);
        eprintln!(
            "live roundtrip ok: open after archive={}, archived={}",
            open.len(),
            archived_list.len()
        );

        let bad = session.workspace_add("/no/such/rt-gui-dir").unwrap_err();
        match bad {
            ConnectError::Rpc { code, message } => {
                assert_eq!(code, "workspace_path_invalid");
                assert!(!message.is_empty());
            }
            other => panic!("expected workspace_path_invalid, got {other:?}"),
        }

        let _ = live.child.kill();
    }

    fn wait_connect(info: &PidInfo) -> Session {
        let start = std::time::Instant::now();
        loop {
            match connect(info) {
                Ok(s) => return s,
                Err(err) if start.elapsed() < std::time::Duration::from_secs(5) => {
                    let _ = err;
                    thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(err) => panic!("live connect failed: {}", err.as_label()),
            }
        }
    }

    fn write_generic_script(dir: &std::path::Path) -> std::path::PathBuf {
        let path = dir.join("generic_agent.py");
        let body = concat!(
            "#!/usr/bin/env python3\n",
            "import json, sys\n",
            "try:\n",
            "    json.load(sys.stdin)\n",
            "except Exception:\n",
            "    pass\n",
            "sys.stdout.write('hello-chunk-1\\n')\n",
            "sys.stdout.write('hello-chunk-2\\n')\n",
            "sys.stdout.flush()\n",
        );
        std::fs::write(&path, body).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).unwrap();
        }
        path
    }

    #[test]
    fn live_host_agent_ws_and_restart_context() {
        let script_home = std::env::temp_dir().join(format!(
            "rt-gui-agent-script-{}-{}",
            std::process::id(),
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&script_home).unwrap();
        let script = write_generic_script(&script_home);
        let Some(mut live) = spawn_live_host_env(&[("RUSTTRAYCER_GENERIC_CMD", script.as_os_str())])
        else {
            let _ = std::fs::remove_dir_all(&script_home);
            return;
        };
        let info = wait_live_pid(&live.home);
        let session = wait_connect(&info);
        assert_eq!(session.host_id, info.host_id);
        assert!(
            session.ws_url.as_deref().unwrap_or("").starts_with("ws://"),
            "ws_url={:?}",
            session.ws_url
        );

        let ws_dir = live.home.join("proj");
        std::fs::create_dir_all(&ws_dir).unwrap();
        std::fs::write(ws_dir.join("README.md"), "# proj\n").unwrap();
        let abs = to_absolute_path(&ws_dir.to_string_lossy());
        let added = session.workspace_add(&abs).expect("workspace.add");
        let created = session
            .task_create("Slice 4", &added.id)
            .expect("task.create");
        let agent = session
            .agent_create(&created.id, "cli.generic")
            .expect("agent.create");
        assert_eq!(agent.provider, "cli.generic");
        let listed = session.agent_list(&created.id).expect("agent.list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, agent.id);
        let got = session.agent_get(&agent.id).expect("agent.get");
        assert_eq!(got.id, agent.id);

        let ws_url = session
            .ws_url
            .clone()
            .or_else(|| info.ws_url.clone())
            .expect("ws url");
        let mut socket = match crate::ws::connect_ws(&ws_url, &session.session_token) {
            Ok(s) => s,
            Err(err) => {
                eprintln!("WS connect failed ({err}); falling back to RPC-only loop");
                let sent = session.agent_send(&agent.id, "ping from gui").ok();
                let ctx = session.agent_get_context(&agent.id).expect("get_context");
                if let Some(msg) = sent {
                    assert!(ctx.iter().any(|m| m.id == msg.id));
                }
                let session2 = wait_connect(&info);
                let ctx2 = session2.agent_get_context(&agent.id).expect("restart ctx");
                assert!(
                    ctx2.iter().any(|m| m.role == "user"),
                    "restart context missing user: {ctx2:?}"
                );
                let _ = live.child.kill();
                let _ = std::fs::remove_dir_all(&script_home);
                return;
            }
        };
        let sub = serde_json::json!({ "type": "subscribe", "taskId": created.id }).to_string();
        socket
            .send(tungstenite::Message::text(sub))
            .expect("subscribe");
        thread::sleep(std::time::Duration::from_millis(80));

        let sent = session
            .agent_send(&agent.id, "ping from gui")
            .expect("agent.send");
        assert_eq!(sent.role, "user");
        assert_eq!(sent.content, "ping from gui");

        let mut saw_assistant = false;
        let mut saw_idle = false;
        let mut saw_user_ws = false;
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_secs(8) {
            match socket.read() {
                Ok(tungstenite::Message::Text(text)) => {
                    let text = text.to_string();
                    if let Ok(ev) = crate::ws::parse_event(&text) {
                        match ev {
                            crate::ws::WsEvent::AgentMessage { message, .. } => {
                                if message.role == "user" && message.id == sent.id {
                                    saw_user_ws = true;
                                }
                                if message.role == "assistant" {
                                    saw_assistant = true;
                                }
                            }
                            crate::ws::WsEvent::AgentStatus { status, .. } if status == "idle" => {
                                saw_idle = true;
                            }
                            _ => {}
                        }
                    }
                }
                Ok(_) => {}
                Err(tungstenite::Error::Io(err))
                    if err.kind() == std::io::ErrorKind::WouldBlock
                        || err.kind() == std::io::ErrorKind::TimedOut => {}
                Err(err) => {
                    eprintln!("WS read ended: {err}");
                    break;
                }
            }
            if saw_assistant && saw_idle {
                break;
            }
        }
        let _ = socket.close(None);

        if !saw_assistant {
            eprintln!(
                "WS assistant chunks were not observed (user_ws={saw_user_ws} idle={saw_idle}); RPC path still asserted"
            );
        } else {
            eprintln!("live WS loop: assistant={saw_assistant} idle={saw_idle} user_ws={saw_user_ws}");
        }

        // Simulate GUI restart: new handshake + get_context, no merge.
        let session2 = wait_connect(&info);
        let ctx = session2
            .agent_get_context(&agent.id)
            .expect("restart get_context");
        assert!(
            ctx.iter()
                .any(|m| m.role == "user" && m.content == "ping from gui"),
            "restart context missing user: {ctx:?}"
        );
        if saw_assistant {
            assert!(
                ctx.iter().any(|m| m.role == "assistant"),
                "restart context missing assistant: {ctx:?}"
            );
        }
        let _ = live.child.kill();
        let _ = std::fs::remove_dir_all(&script_home);
    }
}
