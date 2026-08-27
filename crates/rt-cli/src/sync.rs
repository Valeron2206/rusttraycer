//! Self-hosted rt-sync (C58): thin CLI client for `sync.push` / `sync.pull`.
//!
//! Talks to an already-running loopback host over `/rpc` (same pattern as
//! `doctor`). Does not exec a sibling `rt-sync` binary, does not open host.db,
//! and never writes `RUSTTRAYCER_SYNC_SECRET` to pid.json, configs, or logs.

use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Value};

use crate::{live_pid_file, resolve_data_dir, CliError, PidFile, PROTOCOL_CRATE};

pub const SYNC_SECRET_ENV: &str = "RUSTTRAYCER_SYNC_SECRET";
pub const METHOD_SYNC_PUSH: &str = "sync.push";
pub const METHOD_SYNC_PULL: &str = "sync.pull";
pub const SYNC_SECRET_HEADER: &str = "X-Rt-Sync-Secret";
const SESSION_HEADER: &str = "X-Rt-Session";
const SYNC_METHOD_MAJOR: u32 = 1;
const SYNC_METHOD_MINOR: u32 = 9;

#[derive(Debug, Clone)]
pub enum SyncOp {
    Push {
        peer_url: String,
    },
    Pull {
        peer_url: String,
        workspace_id: String,
    },
}

/// Wire-facing plan for a push/pull. Secret is intentionally absent.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncInvocation {
    pub method: &'static str,
    pub params: Value,
}

pub fn read_sync_secret() -> Result<String, CliError> {
    match std::env::var(SYNC_SECRET_ENV) {
        Ok(s) if !s.trim().is_empty() => Ok(s),
        Ok(_) | Err(_) => Err(CliError::SyncSecretMissing),
    }
}

/// Fail-fast assembly: require env secret, reject cloud/invalid peer URLs.
/// Does not contact the host or the peer.
pub fn prepare_sync(op: SyncOp) -> Result<SyncInvocation, CliError> {
    let _secret = read_sync_secret()?;
    match op {
        SyncOp::Push { peer_url } => {
            validate_peer_url(&peer_url)?;
            Ok(SyncInvocation {
                method: METHOD_SYNC_PUSH,
                params: json!({ "peerUrl": peer_url }),
            })
        }
        SyncOp::Pull {
            peer_url,
            workspace_id,
        } => {
            if workspace_id.trim().is_empty() {
                return Err(CliError::SyncWorkspaceRequired);
            }
            validate_peer_url(&peer_url)?;
            Ok(SyncInvocation {
                method: METHOD_SYNC_PULL,
                params: json!({
                    "peerUrl": peer_url,
                    "workspaceId": workspace_id,
                }),
            })
        }
    }
}

/// POST `inv` to the running loopback host `/rpc`. Re-reads the secret from
/// env at call time and sends it only as [`SYNC_SECRET_HEADER`], never in JSON.
pub fn sync_execute(inv: &SyncInvocation) -> Result<Value, CliError> {
    let secret = read_sync_secret()?;
    let data_dir = resolve_data_dir();
    let info = match live_pid_file(&data_dir)? {
        Some(info) => info,
        None => return Err(CliError::HostNotRunning),
    };
    if !crate::is_loopback_rpc(&info.rpc_url) {
        return Err(CliError::HostNotLoopback);
    }
    call_sync_rpc(&info, inv, &secret)
}

pub fn validate_peer_url(url: &str) -> Result<(), CliError> {
    let url = url.trim();
    if url.is_empty() {
        return Err(CliError::InvalidPeerUrl);
    }
    let host = match peer_host(url) {
        Some(h) => h,
        None => return Err(CliError::InvalidPeerUrl),
    };
    if is_cloud_peer_host(&host) {
        return Err(CliError::ForbiddenPeerUrl);
    }
    Ok(())
}

fn peer_host(url: &str) -> Option<String> {
    let u = url.trim();
    let rest = if let Some(r) = u.strip_prefix("https://") {
        r
    } else if let Some(r) = u.strip_prefix("http://") {
        r
    } else {
        return None;
    };
    if rest.is_empty() {
        return None;
    }
    // Credentials in the URL are forbidden (ADR-0005 / C74).
    if rest.contains('@') {
        return None;
    }
    let hostport = match rest.split('/').next() {
        Some(h) if !h.is_empty() => h,
        _ => return None,
    };
    let hostport = match hostport.split(['?', '#']).next() {
        Some(h) if !h.is_empty() => h,
        _ => return None,
    };
    let host = if hostport.starts_with('[') {
        let end = hostport.find(']')?;
        hostport.get(1..end)?.to_string()
    } else {
        hostport
            .rsplit_once(':')
            .map(|(h, _)| h)
            .unwrap_or(hostport)
            .to_string()
    };
    if host.is_empty() {
        None
    } else {
        Some(host)
    }
}

fn is_cloud_peer_host(host: &str) -> bool {
    let h = host.trim().trim_end_matches('.').to_ascii_lowercase();
    h == "traycer.ai"
        || h.ends_with(".traycer.ai")
        || h == "traycer.com"
        || h.ends_with(".traycer.com")
}

fn call_sync_rpc(info: &PidFile, inv: &SyncInvocation, secret: &str) -> Result<Value, CliError> {
    let rpc = format!("{}/rpc", info.rpc_url.trim_end_matches('/'));
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(1))
        .timeout(Duration::from_secs(60))
        .build();

    let mut methods = serde_json::Map::new();
    methods.insert(
        inv.method.to_string(),
        json!({ "major": SYNC_METHOD_MAJOR, "minor": SYNC_METHOD_MINOR }),
    );
    let hs_body = json!({
        "id": "cli-sync-hs",
        "method": "handshake",
        "params": {
            "client": "cli",
            "clientVersion": PROTOCOL_CRATE,
            "methods": methods
        }
    });
    let hs = rpc_post_json(&agent, &rpc, None, None, &hs_body, "handshake")?;
    let ok = match hs.get("ok") {
        Some(v) => v,
        None => {
            return Err(rpc_error_from_body("handshake", &hs));
        }
    };
    if ok.get("accepted").and_then(|a| a.get(inv.method)).is_none() {
        return Err(CliError::RpcFailed {
            detail: format!("host did not accept {}", inv.method),
        });
    }
    let token = match ok.get("sessionToken").and_then(|t| t.as_str()) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => {
            return Err(CliError::RpcFailed {
                detail: "handshake missing sessionToken".into(),
            });
        }
    };

    let call_body = json!({
        "id": "cli-sync",
        "method": inv.method,
        "params": inv.params,
    });
    let resp = rpc_post_json(
        &agent,
        &rpc,
        Some(token.as_str()),
        Some(secret),
        &call_body,
        inv.method,
    )?;
    match resp.get("ok") {
        Some(v) => Ok(v.clone()),
        None => Err(rpc_error_from_body(inv.method, &resp)),
    }
}

fn rpc_error_from_body(method: &str, body: &Value) -> CliError {
    let code = body
        .get("error")
        .and_then(|e| e.get("code"))
        .and_then(|c| c.as_str())
        .unwrap_or("rpc_failed");
    CliError::RpcFailed {
        detail: format!("{method} {code}"),
    }
}

fn rpc_post_json(
    agent: &ureq::Agent,
    url: &str,
    token: Option<&str>,
    secret: Option<&str>,
    body: &Value,
    method: &str,
) -> Result<Value, CliError> {
    let payload = serde_json::to_string(body)?;
    let mut req = agent
        .post(url)
        .set("Content-Type", "application/json")
        .set("Accept", "application/json");
    if let Some(t) = token {
        req = req.set(SESSION_HEADER, t);
    }
    if let Some(s) = secret {
        req = req.set(SYNC_SECRET_HEADER, s);
    }
    let resp = match req.send_string(&payload) {
        Ok(r) => r,
        Err(ureq::Error::Status(code, _)) => {
            return Err(CliError::RpcFailed {
                detail: format!("{method} http {code}"),
            });
        }
        Err(_) => {
            return Err(CliError::RpcFailed {
                detail: format!("{method} transport error"),
            });
        }
    };
    let text = match resp.into_string() {
        Ok(t) => t,
        Err(_) => {
            return Err(CliError::RpcFailed {
                detail: format!("{method} invalid response"),
            });
        }
    };
    serde_json::from_str(&text).map_err(|_| CliError::RpcFailed {
        detail: format!("{method} invalid json"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pid_path;
    use serde_json::json;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    use super::super::tests_support::{lock_env, EnvGuard};

    fn write_pid_at(dir: &std::path::Path, pid: u32, rpc_url: &str) {
        std::fs::create_dir_all(dir).unwrap();
        let json = json!({
            "hostId": "test-host",
            "pid": pid,
            "rpcUrl": rpc_url,
            "wsUrl": format!("{}/ws", rpc_url.replace("http://", "ws://")),
            "startedAt": "2026-08-19T00:00:00Z",
            "protocol": { "crate": "2.2.0" },
        });
        std::fs::write(pid_path(dir), serde_json::to_vec_pretty(&json).unwrap()).unwrap();
    }

    #[test]
    fn missing_secret_is_typed_error() {
        let _lock = lock_env();
        let _unset = EnvGuard::remove(SYNC_SECRET_ENV);
        let err = prepare_sync(SyncOp::Push {
            peer_url: "http://127.0.0.1:9".into(),
        })
        .unwrap_err();
        assert_eq!(err.code(), "sync_secret_missing");
        let msg = err.to_string();
        assert!(msg.contains(SYNC_SECRET_ENV));
        assert!(!msg.to_ascii_lowercase().contains("password"));
    }

    #[test]
    fn empty_secret_is_typed_error() {
        let _lock = lock_env();
        let _s = EnvGuard::set(SYNC_SECRET_ENV, "   ");
        let err = prepare_sync(SyncOp::Pull {
            peer_url: "http://127.0.0.1:9".into(),
            workspace_id: "ws1".into(),
        })
        .unwrap_err();
        assert_eq!(err.code(), "sync_secret_missing");
    }

    #[test]
    fn secret_not_in_serialized_invocation() {
        let _lock = lock_env();
        let secret = "super-secret-value-xyz-0095";
        let _s = EnvGuard::set(SYNC_SECRET_ENV, secret);
        let push = prepare_sync(SyncOp::Push {
            peer_url: "http://127.0.0.1:47800".into(),
        })
        .unwrap();
        let ser = serde_json::to_string(&push).unwrap();
        assert!(!ser.contains(secret));
        assert!(!ser.contains(SYNC_SECRET_ENV));
        assert!(!ser.contains(SYNC_SECRET_HEADER));
        assert_eq!(push.method, METHOD_SYNC_PUSH);
        assert_eq!(push.params["peerUrl"], "http://127.0.0.1:47800");
        assert!(push.params.get("secret").is_none());
        assert!(push.params.get("token").is_none());

        let pull = prepare_sync(SyncOp::Pull {
            peer_url: "http://127.0.0.1:47800".into(),
            workspace_id: "ws-1".into(),
        })
        .unwrap();
        let ser = serde_json::to_string(&pull).unwrap();
        assert!(!ser.contains(secret));
        assert_eq!(pull.method, METHOD_SYNC_PULL);
        assert_eq!(pull.params["workspaceId"], "ws-1");
    }

    #[test]
    fn cloud_peer_urls_are_rejected_before_network() {
        let _lock = lock_env();
        let secret = "super-secret-value-xyz-0095";
        let _s = EnvGuard::set(SYNC_SECRET_ENV, secret);
        for url in [
            "https://api.traycer.ai/sync",
            "https://sync.traycer.ai/",
            "http://traycer.com/v1",
            "https://app.traycer.com/sync",
        ] {
            let err = prepare_sync(SyncOp::Push {
                peer_url: url.into(),
            })
            .unwrap_err();
            assert_eq!(err.code(), "forbidden_peer_url", "url={url}");
            let msg = err.to_string();
            assert!(!msg.contains(secret));
        }
    }

    #[test]
    fn invalid_peer_urls_rejected() {
        let _lock = lock_env();
        let _s = EnvGuard::set(SYNC_SECRET_ENV, "sekrit");
        for url in [
            "",
            "ftp://127.0.0.1/x",
            "not-a-url",
            "http://user:pass@127.0.0.1:9",
            "https://alice:tok@sync.example/x",
        ] {
            let err = prepare_sync(SyncOp::Push {
                peer_url: url.into(),
            })
            .unwrap_err();
            assert_eq!(err.code(), "invalid_peer_url", "url={url:?}");
            assert!(!err.to_string().contains("sekrit"));
            assert!(!err.to_string().contains("pass"));
            assert!(!err.to_string().contains("tok"));
        }
    }

    #[test]
    fn empty_workspace_id_is_typed_error() {
        let _lock = lock_env();
        let _s = EnvGuard::set(SYNC_SECRET_ENV, "sekrit");
        let err = prepare_sync(SyncOp::Pull {
            peer_url: "http://127.0.0.1:9".into(),
            workspace_id: "  ".into(),
        })
        .unwrap_err();
        assert_eq!(err.code(), "sync_workspace_required");
    }

    #[test]
    fn user_owned_loopback_and_lan_peers_are_ok() {
        let _lock = lock_env();
        let _s = EnvGuard::set(SYNC_SECRET_ENV, "sekrit");
        for url in [
            "http://127.0.0.1:47800",
            "http://localhost:9",
            "https://192.168.1.10:8443/rpc",
            "http://10.0.0.5:1",
        ] {
            prepare_sync(SyncOp::Push {
                peer_url: url.into(),
            })
            .unwrap_or_else(|e| panic!("{url}: {e}"));
        }
    }

    #[test]
    fn execute_without_host_is_host_not_running() {
        let _lock = lock_env();
        let tmp = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("RUSTTRAYCER_HOME", tmp.path());
        let _s = EnvGuard::set(SYNC_SECRET_ENV, "sekrit");
        let inv = prepare_sync(SyncOp::Push {
            peer_url: "http://127.0.0.1:9".into(),
        })
        .unwrap();
        let err = sync_execute(&inv).unwrap_err();
        assert_eq!(err.code(), "host_not_running");
        assert!(!err.to_string().contains("sekrit"));
    }

    #[derive(Clone, Debug)]
    struct Captured {
        path: String,
        headers: Vec<(String, String)>,
        body: Value,
    }

    fn spawn_fake_host() -> (String, Arc<Mutex<Vec<Captured>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(false).unwrap();
        let addr = listener.local_addr().unwrap();
        let captured = Arc::new(Mutex::new(Vec::new()));
        let cap_t = Arc::clone(&captured);
        thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = match stream {
                    Ok(s) => s,
                    Err(_) => break,
                };
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let mut buf = Vec::new();
                let mut tmp = [0u8; 2048];
                let mut headers_end = None;
                let mut content_len = 0usize;
                loop {
                    let n = match stream.read(&mut tmp) {
                        Ok(0) => break,
                        Ok(n) => n,
                        Err(_) => break,
                    };
                    buf.extend_from_slice(&tmp[..n]);
                    if headers_end.is_none() {
                        if let Some(i) = find_double_crlf(&buf) {
                            headers_end = Some(i);
                            let head = String::from_utf8_lossy(&buf[..i]);
                            content_len = head
                                .lines()
                                .find_map(|l| {
                                    let (k, v) = l.split_once(':')?;
                                    if k.eq_ignore_ascii_case("content-length") {
                                        v.trim().parse().ok()
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or(0);
                        }
                    }
                    if let Some(i) = headers_end {
                        if buf.len() >= i + content_len {
                            break;
                        }
                    }
                }
                let Some(i) = headers_end else { continue };
                let head = String::from_utf8_lossy(&buf[..i]);
                let mut lines = head.split("\r\n");
                let reqline = lines.next().unwrap_or("");
                let path = reqline.split_whitespace().nth(1).unwrap_or("").to_string();
                let mut headers = Vec::new();
                for line in lines {
                    if let Some((k, v)) = line.split_once(':') {
                        headers.push((k.trim().to_string(), v.trim().to_string()));
                    }
                }
                let body_bytes = buf.get(i..i + content_len).unwrap_or(&[]);
                let body: Value = serde_json::from_slice(body_bytes).unwrap_or(Value::Null);
                let method = body.get("method").and_then(|m| m.as_str()).unwrap_or("");
                {
                    cap_t.lock().unwrap().push(Captured {
                        path: path.clone(),
                        headers: headers.clone(),
                        body: body.clone(),
                    });
                }
                let reply = if method == "handshake" {
                    let asked = body
                        .get("params")
                        .and_then(|p| p.get("methods"))
                        .cloned()
                        .unwrap_or(json!({}));
                    json!({
                        "id": body.get("id").cloned().unwrap_or(json!("")),
                        "ok": {
                            "accepted": asked,
                            "sessionToken": "tok-0095"
                        }
                    })
                } else {
                    json!({
                        "id": body.get("id").cloned().unwrap_or(json!("")),
                        "ok": { "tasks": 1, "agents": 0 }
                    })
                };
                let payload = serde_json::to_vec(&reply).unwrap();
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    payload.len()
                );
                let _ = stream.write_all(resp.as_bytes());
                let _ = stream.write_all(&payload);
                let _ = stream.flush();
            }
        });
        (format!("http://127.0.0.1:{}", addr.port()), captured)
    }

    fn find_double_crlf(buf: &[u8]) -> Option<usize> {
        buf.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4)
    }

    #[test]
    fn push_posts_to_loopback_host_not_peer_or_cloud() {
        let _lock = lock_env();
        let tmp = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("RUSTTRAYCER_HOME", tmp.path());
        let secret = "super-secret-value-xyz-0095";
        let _s = EnvGuard::set(SYNC_SECRET_ENV, secret);
        let (rpc, captured) = spawn_fake_host();
        write_pid_at(&tmp.path().join("host"), std::process::id(), &rpc);

        let peer = "http://10.0.0.8:8443";
        let inv = prepare_sync(SyncOp::Push {
            peer_url: peer.into(),
        })
        .unwrap();
        let ok = sync_execute(&inv).unwrap();
        assert_eq!(ok["tasks"], 1);

        let hits = captured.lock().unwrap().clone();
        assert_eq!(hits.len(), 2, "handshake + sync.push");
        for h in &hits {
            assert_eq!(h.path, "/rpc");
            let host = h
                .headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("host"))
                .map(|(_, v)| v.as_str())
                .unwrap_or("");
            assert!(
                host.starts_with("127.0.0.1:"),
                "must hit loopback host, got host={host}"
            );
            let body = serde_json::to_string(&h.body).unwrap();
            assert!(!body.contains(secret));
            assert!(!body.contains("traycer.ai"));
        }
        assert_eq!(hits[0].body["method"], "handshake");
        assert_eq!(hits[1].body["method"], METHOD_SYNC_PUSH);
        assert_eq!(hits[1].body["params"]["peerUrl"], peer);
        let secret_header = hits[1]
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(SYNC_SECRET_HEADER))
            .map(|(_, v)| v.as_str());
        assert_eq!(secret_header, Some(secret));
        assert!(hits[0]
            .headers
            .iter()
            .all(|(k, _)| !k.eq_ignore_ascii_case(SYNC_SECRET_HEADER)));
    }

    #[test]
    fn pull_posts_workspace_id_and_keeps_secret_off_wire_json() {
        let _lock = lock_env();
        let tmp = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("RUSTTRAYCER_HOME", tmp.path());
        let secret = "another-secret-0095";
        let _s = EnvGuard::set(SYNC_SECRET_ENV, secret);
        let (rpc, captured) = spawn_fake_host();
        write_pid_at(&tmp.path().join("host"), std::process::id(), &rpc);

        let inv = prepare_sync(SyncOp::Pull {
            peer_url: "http://192.168.0.4:9".into(),
            workspace_id: "ws-abc".into(),
        })
        .unwrap();
        sync_execute(&inv).unwrap();
        let hits = captured.lock().unwrap().clone();
        assert_eq!(hits[1].body["method"], METHOD_SYNC_PULL);
        assert_eq!(hits[1].body["params"]["workspaceId"], "ws-abc");
        let body = serde_json::to_string(&hits[1].body).unwrap();
        assert!(!body.contains(secret));
    }

    #[test]
    fn cloud_url_does_not_contact_host() {
        let _lock = lock_env();
        let tmp = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("RUSTTRAYCER_HOME", tmp.path());
        let _s = EnvGuard::set(SYNC_SECRET_ENV, "sekrit");
        let (rpc, captured) = spawn_fake_host();
        write_pid_at(&tmp.path().join("host"), std::process::id(), &rpc);
        let err = prepare_sync(SyncOp::Push {
            peer_url: "https://api.traycer.ai/sync".into(),
        })
        .unwrap_err();
        assert_eq!(err.code(), "forbidden_peer_url");
        assert!(captured.lock().unwrap().is_empty());
    }
}
