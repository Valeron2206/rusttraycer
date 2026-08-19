//! pid.json, loopback listen, process lock, data-dir resolution.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::HostError;

pub const PROTOCOL_CRATE: &str = "2.1.0";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PidFile {
    pub host_id: String,
    pub pid: u32,
    pub rpc_url: String,
    pub ws_url: String,
    pub started_at: String,
    pub protocol: ProtocolInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolInfo {
    pub crate_version: String,
}

impl PidFile {
    pub fn new(host_id: String, pid: u32, port: u16) -> Self {
        Self {
            host_id,
            pid,
            rpc_url: format!("http://127.0.0.1:{port}"),
            ws_url: format!("ws://127.0.0.1:{port}/ws"),
            started_at: rt_storage::now_rfc3339(),
            protocol: ProtocolInfo {
                crate_version: PROTOCOL_CRATE.to_string(),
            },
        }
    }
}

/// Product home: `$RUSTTRAYCER_HOME` or `~/.rusttraycer`.
/// Host data (pid.json, host.db, host.log) lives in `<product_home>/host/`.
pub fn resolve_product_home() -> PathBuf {
    if let Ok(home) = std::env::var("RUSTTRAYCER_HOME") {
        let home = home.trim();
        if !home.is_empty() {
            return PathBuf::from(home);
        }
    }
    let base = directories::BaseDirs::new()
        .map(|b| b.home_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    base.join(".rusttraycer")
}

pub fn resolve_data_dir() -> PathBuf {
    resolve_product_home().join("host")
}

pub fn pid_path(data_dir: &Path) -> PathBuf {
    data_dir.join("pid.json")
}

pub fn db_path(data_dir: &Path) -> PathBuf {
    data_dir.join("host.db")
}

pub fn log_path(data_dir: &Path) -> PathBuf {
    data_dir.join("host.log")
}

pub fn is_pid_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    Path::new(&format!("/proc/{pid}")).exists()
}

pub fn read_pid_file(data_dir: &Path) -> Result<Option<PidFile>, HostError> {
    let path = pid_path(data_dir);
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path)?;
    let v: serde_json::Value = serde_json::from_str(&text)?;
    Ok(Some(pid_from_value(&v)?))
}

fn pid_from_value(v: &serde_json::Value) -> Result<PidFile, HostError> {
    let host_id = v
        .get("hostId")
        .and_then(|x| x.as_str())
        .ok_or_else(|| HostError::Internal("pid.json missing hostId".into()))?
        .to_string();
    let pid = v
        .get("pid")
        .and_then(|x| x.as_u64())
        .ok_or_else(|| HostError::Internal("pid.json missing pid".into()))? as u32;
    let rpc_url = v
        .get("rpcUrl")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let ws_url = v
        .get("wsUrl")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let started_at = v
        .get("startedAt")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let crate_version = v
        .get("protocol")
        .and_then(|p| p.get("crate"))
        .and_then(|x| x.as_str())
        .unwrap_or(PROTOCOL_CRATE)
        .to_string();
    Ok(PidFile {
        host_id,
        pid,
        rpc_url,
        ws_url,
        started_at,
        protocol: ProtocolInfo { crate_version },
    })
}

/// If pid.json names a live pid that is not `our_pid`, the host is already running.
pub fn check_not_already_running(data_dir: &Path, our_pid: u32) -> Result<(), HostError> {
    match read_pid_file(data_dir)? {
        Some(info) if info.pid != our_pid && is_pid_alive(info.pid) => {
            Err(HostError::AlreadyRunning {
                pid: info.pid,
                rpc_url: info.rpc_url,
            })
        }
        _ => Ok(()),
    }
}

pub fn write_pid_file(data_dir: &Path, info: &PidFile) -> Result<(), HostError> {
    fs::create_dir_all(data_dir)?;
    let dest = pid_path(data_dir);
    let tmp = data_dir.join(format!("pid.json.{}.tmp", info.pid));
    let json = serde_json::json!({
        "hostId": info.host_id,
        "pid": info.pid,
        "rpcUrl": info.rpc_url,
        "wsUrl": info.ws_url,
        "startedAt": info.started_at,
        "protocol": { "crate": info.protocol.crate_version },
    });
    fs::write(&tmp, serde_json::to_vec_pretty(&json)?)?;
    fs::rename(&tmp, dest)?;
    Ok(())
}

/// Remove pid.json only if it still names `our_pid`.
pub fn remove_pid_file_if_ours(data_dir: &Path, our_pid: u32) -> Result<(), HostError> {
    let path = pid_path(data_dir);
    if !path.exists() {
        return Ok(());
    }
    if let Ok(Some(info)) = read_pid_file(data_dir) {
        if info.pid != our_pid {
            return Ok(());
        }
    }
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn pid_lock_rejects_live_foreign_pid() {
        let dir = tempdir().unwrap();
        let our_pid = std::process::id();
        let info = PidFile::new("host-1".into(), our_pid, 9);
        write_pid_file(dir.path(), &info).unwrap();

        // A different "starting" pid must see us as already running.
        let err = check_not_already_running(dir.path(), our_pid.wrapping_add(1)).unwrap_err();
        assert_eq!(err.code(), "already_running");
    }

    #[test]
    fn pid_lock_allows_stale_or_self() {
        let dir = tempdir().unwrap();
        let our_pid = std::process::id();
        // stale dead pid
        let info = PidFile::new("host-1".into(), 999_999_999, 9);
        write_pid_file(dir.path(), &info).unwrap();
        assert!(!is_pid_alive(999_999_999));
        check_not_already_running(dir.path(), our_pid).unwrap();

        // self is allowed
        let info = PidFile::new("host-1".into(), our_pid, 9);
        write_pid_file(dir.path(), &info).unwrap();
        check_not_already_running(dir.path(), our_pid).unwrap();
    }

    #[test]
    fn rusttraycer_home_is_product_home() {
        assert_eq!(
            PathBuf::from("/opt/rt").join("host"),
            PathBuf::from("/opt/rt/host")
        );
        // resolve_product_home + /host is the data dir
        let data = resolve_data_dir();
        assert!(data.ends_with("host"), "{data:?}");
    }

    #[test]
    fn pid_json_wire_uses_crate_key() {
        let dir = tempdir().unwrap();
        let info = PidFile::new("abc".into(), 1, 1234);
        write_pid_file(dir.path(), &info).unwrap();
        let text = fs::read_to_string(pid_path(dir.path())).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["protocol"]["crate"], "2.1.0");
        assert_eq!(v["rpcUrl"], "http://127.0.0.1:1234");
        assert_eq!(v["wsUrl"], "ws://127.0.0.1:1234/ws");
    }
    #[test]
    fn resolve_product_home_respects_env() {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _g = LOCK.lock().unwrap();
        let prev = std::env::var("RUSTTRAYCER_HOME").ok();
        std::env::set_var("RUSTTRAYCER_HOME", "/tmp/rt-home-0024");
        assert_eq!(resolve_product_home(), PathBuf::from("/tmp/rt-home-0024"));
        assert_eq!(resolve_data_dir(), PathBuf::from("/tmp/rt-home-0024/host"));
        std::env::set_var("RUSTTRAYCER_HOME", "   ");
        let fallback = resolve_product_home();
        assert!(fallback.ends_with(".rusttraycer"), "{fallback:?}");
        match prev {
            Some(v) => std::env::set_var("RUSTTRAYCER_HOME", v),
            None => std::env::remove_var("RUSTTRAYCER_HOME"),
        }
    }

    #[test]
    fn pid_zero_is_not_alive() {
        assert!(!is_pid_alive(0));
        assert!(is_pid_alive(std::process::id()));
        assert_eq!(db_path(Path::new("/d")), PathBuf::from("/d/host.db"));
        assert_eq!(log_path(Path::new("/d")), PathBuf::from("/d/host.log"));
    }

    #[test]
    fn remove_pid_file_missing_and_foreign() {
        let dir = tempdir().unwrap();
        remove_pid_file_if_ours(dir.path(), 1).unwrap();
        let info = PidFile::new("h".into(), 1, 9);
        write_pid_file(dir.path(), &info).unwrap();
        remove_pid_file_if_ours(dir.path(), 99).unwrap();
        assert!(pid_path(dir.path()).exists());
        remove_pid_file_if_ours(dir.path(), 1).unwrap();
        assert!(!pid_path(dir.path()).exists());
    }

    #[test]
    fn read_pid_file_missing_and_malformed() {
        let dir = tempdir().unwrap();
        assert!(read_pid_file(dir.path()).unwrap().is_none());
        std::fs::write(pid_path(dir.path()), b"{\"pid\":1}").unwrap();
        let err = read_pid_file(dir.path()).unwrap_err();
        assert_eq!(err.code(), "internal");
    }
}
