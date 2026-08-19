//! Process lifecycle CLI. Outside view of the host: paths, pid.json, exec/stop.
//!
//! Does not open host.db, does not depend on rt-storage / rusqlite / rt-host.

use std::fs;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{json, Value};

mod sync;

pub use sync::{
    prepare_sync, read_sync_secret, sync_execute, validate_peer_url, SyncInvocation, SyncOp,
    METHOD_SYNC_PULL, METHOD_SYNC_PUSH, SYNC_SECRET_ENV, SYNC_SECRET_HEADER,
};

pub const PROTOCOL_CRATE: &str = "2.0.0";
const SESSION_HEADER: &str = "X-Rt-Session";
const STOP_WAIT: Duration = Duration::from_secs(2);

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("already running (pid {pid})")]
    AlreadyRunning { pid: u32, rpc_url: String },
    #[error("rt-host binary not found")]
    HostBinNotFound,
    #[error("failed to exec {}: {source}", bin.display())]
    ExecFailed {
        bin: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("host pid {pid} did not exit after SIGTERM")]
    StopTimeout { pid: u32 },
    #[error("invalid pid.json: {0}")]
    InvalidPidFile(String),
    #[error("reset-db requires --yes")]
    ResetNeedsYes,
    #[error("host is running; stop first")]
    HostRunning,
    #[error("RUSTTRAYCER_SYNC_SECRET is missing")]
    SyncSecretMissing,
    #[error("workspace-id is required")]
    SyncWorkspaceRequired,
    #[error("peer URL is not a user-owned host (Traycer cloud is forbidden)")]
    ForbiddenPeerUrl,
    #[error("invalid peer URL")]
    InvalidPeerUrl,
    #[error("host is not running; start it first")]
    HostNotRunning,
    #[error("host rpcUrl is not loopback")]
    HostNotLoopback,
    #[error("rpc failed: {detail}")]
    RpcFailed { detail: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl CliError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::AlreadyRunning { .. } => "already_running",
            Self::HostBinNotFound => "host_bin_not_found",
            Self::ExecFailed { .. } => "exec_failed",
            Self::StopTimeout { .. } => "stop_timeout",
            Self::InvalidPidFile(_) => "invalid_pid_file",
            Self::ResetNeedsYes => "reset_needs_yes",
            Self::HostRunning => "host_running",
            Self::SyncSecretMissing => "sync_secret_missing",
            Self::SyncWorkspaceRequired => "sync_workspace_required",
            Self::ForbiddenPeerUrl => "forbidden_peer_url",
            Self::InvalidPeerUrl => "invalid_peer_url",
            Self::HostNotRunning => "host_not_running",
            Self::HostNotLoopback => "host_not_loopback",
            Self::RpcFailed { .. } => "rpc_failed",
            Self::Io(_) | Self::Json(_) => "internal",
        }
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            Self::AlreadyRunning { .. } => 2,
            _ => 1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PidFile {
    pub host_id: String,
    pub pid: u32,
    pub rpc_url: String,
    pub ws_url: String,
    pub started_at: String,
    pub protocol_crate: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopOutcome {
    NotRunning,
    Stopped { pid: u32 },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HarnessStatus {
    pub id: String,
    pub available: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorReport {
    pub running: bool,
    pub pid: Option<u32>,
    pub host_id: Option<String>,
    pub rpc_url: Option<String>,
    pub port: Option<u16>,
    pub version: String,
    pub data_dir: String,
    pub db_path: String,
    pub log_path: String,
    pub pid_path: String,
    pub harnesses: Vec<HarnessStatus>,
    pub host: Option<Value>,
}

/// Product home: `$RUSTTRAYCER_HOME` if set and nonempty, else `~/.rusttraycer`.
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

/// Host data (pid.json, host.db, host.log) lives in `<product_home>/host/`.
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
    // Zombies still have a /proc/<pid> directory; they are not running.
    let stat = match fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(s) => s,
        Err(_) => return false,
    };
    // comm is in parentheses and may contain spaces; state follows the last ')'.
    let state = stat
        .rfind(')')
        .and_then(|i| stat[i + 1..].split_whitespace().next());
    match state {
        Some("Z") | Some("X") | Some("x") => false,
        Some(_) => true,
        None => false,
    }
}

pub fn read_pid_file(data_dir: &Path) -> Result<Option<PidFile>, CliError> {
    let path = pid_path(data_dir);
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path)?;
    let v: Value = serde_json::from_str(&text)?;
    Ok(Some(pid_from_value(&v)?))
}

fn pid_from_value(v: &Value) -> Result<PidFile, CliError> {
    let host_id = v
        .get("hostId")
        .and_then(|x| x.as_str())
        .ok_or_else(|| CliError::InvalidPidFile("missing hostId".into()))?
        .to_string();
    let pid = v
        .get("pid")
        .and_then(|x| x.as_u64())
        .ok_or_else(|| CliError::InvalidPidFile("missing pid".into()))? as u32;
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
    let protocol_crate = v
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
        protocol_crate,
    })
}

/// Live foreign pid named by pid.json, if any.
pub fn live_pid_file(data_dir: &Path) -> Result<Option<PidFile>, CliError> {
    match read_pid_file(data_dir)? {
        Some(info) if is_pid_alive(info.pid) => Ok(Some(info)),
        _ => Ok(None),
    }
}

/// Locate the rt-host binary (does not exec).
pub fn resolve_host_bin() -> Result<PathBuf, CliError> {
    if let Ok(p) = std::env::var("RUSTTRAYCER_HOST_BIN") {
        let p = p.trim();
        if !p.is_empty() {
            return Ok(PathBuf::from(p));
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let sibling = dir.join("rt-host");
            if sibling.is_file() {
                return Ok(sibling);
            }
        }
    }
    if let Some(found) = search_path("rt-host") {
        return Ok(found);
    }
    Err(CliError::HostBinNotFound)
}

fn search_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let cand = dir.join(name);
        if cand.is_file() {
            return Some(cand);
        }
    }
    None
}

/// Already-running check + resolve host binary. Does not exec (safe for tests).
pub fn prepare_start() -> Result<PathBuf, CliError> {
    let data_dir = resolve_data_dir();
    if let Some(info) = live_pid_file(&data_dir)? {
        return Err(CliError::AlreadyRunning {
            pid: info.pid,
            rpc_url: info.rpc_url,
        });
    }
    resolve_host_bin()
}

/// Replace the current process with `bin`. Never returns on success (Unix exec).
#[cfg(unix)]
pub fn exec_host(bin: &Path) -> Result<(), CliError> {
    use std::os::unix::process::CommandExt;
    let err = Command::new(bin).exec();
    Err(CliError::ExecFailed {
        bin: bin.to_path_buf(),
        source: err,
    })
}

#[cfg(not(unix))]
pub fn exec_host(bin: &Path) -> Result<(), CliError> {
    Err(CliError::ExecFailed {
        bin: bin.to_path_buf(),
        source: std::io::Error::new(std::io::ErrorKind::Unsupported, "exec_host is Unix-only"),
    })
}

/// SIGTERM the pid in pid.json. Idempotent if not running. Does not remove pid.json.
pub fn stop() -> Result<StopOutcome, CliError> {
    let data_dir = resolve_data_dir();
    let Some(info) = read_pid_file(&data_dir)? else {
        return Ok(StopOutcome::NotRunning);
    };
    if !is_pid_alive(info.pid) {
        return Ok(StopOutcome::NotRunning);
    }
    send_sigterm(info.pid)?;
    if wait_until_dead(info.pid, STOP_WAIT) {
        Ok(StopOutcome::Stopped { pid: info.pid })
    } else {
        Err(CliError::StopTimeout { pid: info.pid })
    }
}

fn send_sigterm(pid: u32) -> Result<(), CliError> {
    if pid == 0 || pid == 1 {
        return Ok(());
    }
    let rc = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
    if rc == 0 {
        return Ok(());
    }
    let err = std::io::Error::last_os_error();
    if err.raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(err.into())
    }
}

fn wait_until_dead(pid: u32, timeout: Duration) -> bool {
    let start = Instant::now();
    loop {
        if !is_pid_alive(pid) {
            return true;
        }
        if start.elapsed() >= timeout {
            return !is_pid_alive(pid);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Outside view: paths + is pid alive. Never opens sqlite.
/// If the host process is alive, optionally handshake + host.doctor over loopback RPC.
pub fn doctor() -> Result<DoctorReport, CliError> {
    let data_dir = resolve_data_dir();
    let live = live_pid_file(&data_dir)?;
    let running = live.is_some();
    let host = if let Some(info) = live.as_ref() {
        fetch_host_doctor(&info.rpc_url)
    } else {
        None
    };
    let port = live.as_ref().and_then(|i| port_from_rpc_url(&i.rpc_url));
    let version = live
        .as_ref()
        .map(|i| i.protocol_crate.clone())
        .unwrap_or_else(|| PROTOCOL_CRATE.to_string());
    let harnesses = rt_runtime::probe_harnesses()
        .into_iter()
        .map(|p| HarnessStatus {
            id: p.id,
            available: p.available,
            detail: p.detail,
        })
        .collect();
    Ok(DoctorReport {
        running,
        pid: live.as_ref().map(|i| i.pid),
        host_id: live.as_ref().map(|i| i.host_id.clone()),
        rpc_url: live.as_ref().map(|i| i.rpc_url.clone()),
        port,
        version,
        data_dir: data_dir.to_string_lossy().into_owned(),
        db_path: db_path(&data_dir).to_string_lossy().into_owned(),
        log_path: log_path(&data_dir).to_string_lossy().into_owned(),
        pid_path: pid_path(&data_dir).to_string_lossy().into_owned(),
        harnesses,
        host,
    })
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StatusReport {
    pub alive: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rpc_url: Option<String>,
    pub data_dir: String,
}

/// pid.json only. Does not call `/rpc`. Dead host → `alive: false`, exit 0.
pub fn status() -> Result<StatusReport, CliError> {
    let data_dir = resolve_data_dir();
    let live = live_pid_file(&data_dir)?;
    Ok(StatusReport {
        alive: live.is_some(),
        pid: live.as_ref().map(|i| i.pid),
        rpc_url: live.as_ref().map(|i| i.rpc_url.clone()),
        data_dir: data_dir.to_string_lossy().into_owned(),
    })
}

fn last_n_lines(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(n);
    let mut out = lines[start..].join("\n");
    if text.ends_with('\n') && !out.is_empty() {
        out.push('\n');
    }
    out
}

const FOLLOW_POLL: Duration = Duration::from_millis(50);

/// Tail `host.log`. Missing file → empty string. `lines` clamped to 1..=10000.
pub fn logs(lines: u32) -> Result<String, CliError> {
    let n = lines.clamp(1, 10_000) as usize;
    tail_file(&log_path(&resolve_data_dir()), n)
}

fn tail_file(path: &Path, n: usize) -> Result<String, CliError> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(last_n_lines(&text, n)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e.into()),
    }
}

fn read_from_offset(path: &Path, offset: u64) -> std::io::Result<(Vec<u8>, u64)> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    let start = if len < offset { 0 } else { offset };
    file.seek(SeekFrom::Start(start))?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    let new_offset = start + buf.len() as u64;
    Ok((buf, new_offset))
}

/// Print the current tail of `path`, then append new bytes until `stop` is true.
///
/// Missing file: write nothing and wait for it to appear (or stop). File-only;
/// does not call `/rpc` or `/metrics`.
pub fn follow_log(path: &Path, lines: u32, stop: impl FnMut() -> bool) -> Result<(), CliError> {
    follow_log_to(path, lines, &mut std::io::stdout(), stop)
}

/// Same as [`follow_log`] but writes to `out` so tests can assert without hanging.
pub fn follow_log_to<W: Write>(
    path: &Path,
    lines: u32,
    out: &mut W,
    mut stop: impl FnMut() -> bool,
) -> Result<(), CliError> {
    let n = lines.clamp(1, 10_000) as usize;

    while !path.exists() {
        if stop() {
            return Ok(());
        }
        std::thread::sleep(FOLLOW_POLL);
    }

    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.into()),
    };
    let tail = last_n_lines(&text, n);
    out.write_all(tail.as_bytes())?;
    out.flush()?;
    let mut offset = text.len() as u64;

    loop {
        if stop() {
            return Ok(());
        }
        match read_from_offset(path, offset) {
            Ok((buf, new_offset)) => {
                if !buf.is_empty() {
                    out.write_all(&buf)?;
                    out.flush()?;
                }
                offset = new_offset;
                if buf.is_empty() {
                    std::thread::sleep(FOLLOW_POLL);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                offset = 0;
                std::thread::sleep(FOLLOW_POLL);
            }
            Err(e) => return Err(e.into()),
        }
    }
}

/// Tail then follow `<RUSTTRAYCER_HOME>/host/host.log` until SIGINT. Exit 0 on SIGINT.
pub fn logs_follow(lines: u32) -> Result<(), CliError> {
    let path = log_path(&resolve_data_dir());
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    ctrlc::set_handler(move || {
        flag.store(true, Ordering::SeqCst);
    })
    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    follow_log(&path, lines, move || stop.load(Ordering::SeqCst))
}

/// Unlink `host.db` + wal/shm. Requires `--yes`. Refuses if the host pid is alive.
pub fn reset_db(yes: bool) -> Result<(), CliError> {
    if !yes {
        return Err(CliError::ResetNeedsYes);
    }
    let data_dir = resolve_data_dir();
    if live_pid_file(&data_dir)?.is_some() {
        return Err(CliError::HostRunning);
    }
    for name in ["host.db", "host.db-wal", "host.db-shm"] {
        let p = data_dir.join(name);
        match fs::remove_file(&p) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

/// Parse the TCP port from a pid.json `rpcUrl` (`http://127.0.0.1:47800` → 47800).
fn port_from_rpc_url(url: &str) -> Option<u16> {
    let rest = url
        .trim()
        .strip_prefix("http://")
        .or_else(|| url.trim().strip_prefix("https://"))?;
    let hostport = rest.split('/').next().unwrap_or(rest);
    let hostport = hostport
        .rsplit_once('@')
        .map(|(_, h)| h)
        .unwrap_or(hostport);
    let port = if hostport.starts_with('[') {
        let end = hostport.find(']')?;
        hostport.get(end + 1..)?.strip_prefix(':')?
    } else {
        hostport.rsplit_once(':').map(|(_, p)| p)?
    };
    port.parse().ok()
}

pub(crate) fn is_loopback_rpc(url: &str) -> bool {
    let u = url.trim();
    let rest = if let Some(r) = u.strip_prefix("http://") {
        r
    } else {
        return false;
    };
    let host = rest.split('/').next().unwrap_or(rest);
    let host = host.rsplit_once('@').map(|(_, h)| h).unwrap_or(host);
    let hostname = if host.starts_with('[') {
        return false;
    } else {
        host.rsplit_once(':').map(|(h, _)| h).unwrap_or(host)
    };
    hostname == "127.0.0.1" || hostname.eq_ignore_ascii_case("localhost")
}

fn fetch_host_doctor(rpc_url: &str) -> Option<Value> {
    if !is_loopback_rpc(rpc_url) {
        return None;
    }
    let rpc = format!("{}/rpc", rpc_url.trim_end_matches('/'));
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_millis(400))
        .timeout(Duration::from_secs(2))
        .build();

    let hs_body = json!({
        "id": "cli-hs",
        "method": "handshake",
        "params": {
            "client": "cli",
            "clientVersion": PROTOCOL_CRATE,
            "methods": {
                "host.doctor": { "major": 1, "minor": 0 }
            }
        }
    });
    let hs = rpc_post(&agent, &rpc, None, &hs_body)?;
    let ok = hs.get("ok")?;
    ok.get("accepted").and_then(|a| a.get("host.doctor"))?;
    let token = ok.get("sessionToken")?.as_str()?;

    let doc_body = json!({
        "id": "cli-doc",
        "method": "host.doctor",
        "params": {}
    });
    let doc = rpc_post(&agent, &rpc, Some(token), &doc_body)?;
    doc.get("ok").cloned()
}

fn rpc_post(agent: &ureq::Agent, url: &str, token: Option<&str>, body: &Value) -> Option<Value> {
    let payload = serde_json::to_string(body).ok()?;
    let mut req = agent
        .post(url)
        .set("Content-Type", "application/json")
        .set("Accept", "application/json");
    if let Some(t) = token {
        req = req.set(SESSION_HEADER, t);
    }
    let resp = req.send_string(&payload).ok()?;
    let text = resp.into_string().ok()?;
    serde_json::from_str(&text).ok()
}

#[cfg(test)]
pub(crate) mod tests_support {
    use std::sync::{Mutex, MutexGuard};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    pub struct EnvGuard {
        key: &'static str,
        old: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        pub fn set(key: &'static str, val: impl AsRef<std::ffi::OsStr>) -> Self {
            let old = std::env::var_os(key);
            std::env::set_var(key, val);
            Self { key, old }
        }

        pub fn remove(key: &'static str) -> Self {
            let old = std::env::var_os(key);
            std::env::remove_var(key);
            Self { key, old }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.old {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }

    pub fn lock_env() -> MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::tests_support::{lock_env, EnvGuard};
    use super::*;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    fn write_pid(dir: &Path, pid: u32) {
        fs::create_dir_all(dir).unwrap();
        let json = json!({
            "hostId": "test-host",
            "pid": pid,
            "rpcUrl": "http://127.0.0.1:9",
            "wsUrl": "ws://127.0.0.1:9/ws",
            "startedAt": "2026-01-01T00:00:00Z",
            "protocol": { "crate": "1.0.0" },
        });
        fs::write(pid_path(dir), serde_json::to_vec_pretty(&json).unwrap()).unwrap();
    }

    fn dummy_bin(dir: &Path) -> PathBuf {
        let p = dir.join("dummy-rt-host");
        fs::write(&p, "#!/bin/sh\nexit 0\n").unwrap();
        let mut perms = fs::metadata(&p).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&p, perms).unwrap();
        p
    }

    #[test]
    fn data_dir_uses_rusttraycer_home() {
        let _lock = lock_env();
        let tmp = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("RUSTTRAYCER_HOME", tmp.path());
        assert_eq!(resolve_data_dir(), tmp.path().join("host"));
        assert_eq!(resolve_product_home(), tmp.path());
    }

    #[test]
    fn prepare_start_errors_already_running_for_live_pid() {
        let _lock = lock_env();
        let tmp = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("RUSTTRAYCER_HOME", tmp.path());
        let _bin = EnvGuard::set("RUSTTRAYCER_HOST_BIN", dummy_bin(tmp.path()));
        write_pid(&tmp.path().join("host"), std::process::id());

        let err = prepare_start().unwrap_err();
        assert_eq!(err.code(), "already_running");
        assert_eq!(err.exit_code(), 2);
        match err {
            CliError::AlreadyRunning { pid, .. } => assert_eq!(pid, std::process::id()),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn prepare_start_ok_when_pid_missing_or_dead() {
        let _lock = lock_env();
        let tmp = tempfile::tempdir().unwrap();
        let dummy = dummy_bin(tmp.path());
        let _home = EnvGuard::set("RUSTTRAYCER_HOME", tmp.path());
        let _bin = EnvGuard::set("RUSTTRAYCER_HOST_BIN", &dummy);

        let got = prepare_start().unwrap();
        assert_eq!(got, dummy);

        write_pid(&tmp.path().join("host"), 999_999_999);
        assert!(!is_pid_alive(999_999_999));
        let got = prepare_start().unwrap();
        assert_eq!(got, dummy);
    }

    #[test]
    fn stop_is_idempotent_when_not_running() {
        let _lock = lock_env();
        let tmp = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("RUSTTRAYCER_HOME", tmp.path());

        assert_eq!(stop().unwrap(), StopOutcome::NotRunning);

        write_pid(&tmp.path().join("host"), 999_999_999);
        assert_eq!(stop().unwrap(), StopOutcome::NotRunning);
        // CLI must not remove pid.json itself.
        assert!(pid_path(&tmp.path().join("host")).exists());
    }

    #[test]
    fn start_stop_roundtrip_with_mock_host() {
        let _lock = lock_env();
        let tmp = tempfile::tempdir().unwrap();
        let mock = tmp.path().join("mock-rt-host");
        {
            let mut f = fs::File::create(&mock).unwrap();
            writeln!(
                f,
                r#"#!/bin/sh
set -eu
dir="$RUSTTRAYCER_HOME/host"
mkdir -p "$dir"
cat > "$dir/pid.json" <<EOF
{{
  "hostId": "mock-host",
  "pid": $$,
  "rpcUrl": "http://127.0.0.1:9",
  "wsUrl": "ws://127.0.0.1:9/ws",
  "startedAt": "2026-01-01T00:00:00Z",
  "protocol": {{ "crate": "1.0.0" }}
}}
EOF
trap 'exit 0' TERM INT
while true; do
  sleep 0.2
done
"#
            )
            .unwrap();
        }
        let mut perms = fs::metadata(&mock).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&mock, perms).unwrap();

        let _home = EnvGuard::set("RUSTTRAYCER_HOME", tmp.path());
        let _bin = EnvGuard::set("RUSTTRAYCER_HOST_BIN", &mock);

        let bin = prepare_start().unwrap();
        assert_eq!(bin, mock);

        let mut child = Command::new(&bin)
            .env("RUSTTRAYCER_HOME", tmp.path())
            .spawn()
            .unwrap();

        let data = tmp.path().join("host");
        let ppath = pid_path(&data);
        let start = Instant::now();
        while !ppath.exists() && start.elapsed() < Duration::from_secs(2) {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(ppath.exists(), "mock host did not write pid.json");

        let info = read_pid_file(&data).unwrap().expect("pid.json");
        assert!(is_pid_alive(info.pid), "mock pid {} not alive", info.pid);
        assert_eq!(info.host_id, "mock-host");

        let outcome = stop().unwrap();
        assert_eq!(outcome, StopOutcome::Stopped { pid: info.pid });
        let _ = child.wait();
        assert!(!is_pid_alive(info.pid), "mock still alive after stop");
    }

    #[test]
    fn doctor_does_not_open_sqlite() {
        let _lock = lock_env();
        let tmp = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("RUSTTRAYCER_HOME", tmp.path());
        // host.db is missing; doctor must still return paths + running=false.
        assert!(!db_path(&tmp.path().join("host")).exists());

        let report = doctor().unwrap();
        assert!(!report.running);
        assert!(report.pid.is_none());
        assert!(report.host_id.is_none());
        assert!(report.rpc_url.is_none());
        assert!(report.host.is_none());
        assert_eq!(report.data_dir, tmp.path().join("host").to_string_lossy());
        assert_eq!(
            report.db_path,
            tmp.path().join("host").join("host.db").to_string_lossy()
        );
        assert_eq!(
            report.log_path,
            tmp.path().join("host").join("host.log").to_string_lossy()
        );
        assert_eq!(
            report.pid_path,
            tmp.path().join("host").join("pid.json").to_string_lossy()
        );
        assert!(report.port.is_none());
        assert_eq!(report.version, PROTOCOL_CRATE);
        assert_eq!(report.harnesses.len(), 3);
        assert_eq!(report.harnesses[0].id, "cli.generic");
        assert_eq!(report.harnesses[1].id, "cli.claude");
        assert_eq!(report.harnesses[2].id, "cli.codex");
    }

    #[test]
    fn doctor_generic_available_when_cmd_set() {
        let _lock = lock_env();
        let tmp = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("RUSTTRAYCER_HOME", tmp.path());
        let dummy = dummy_bin(tmp.path());
        let _cmd = EnvGuard::set("RUSTTRAYCER_GENERIC_CMD", &dummy);

        let report = doctor().unwrap();
        assert!(!report.running);
        assert!(!db_path(&tmp.path().join("host")).exists());
        let generic = report
            .harnesses
            .iter()
            .find(|h| h.id == "cli.generic")
            .expect("cli.generic");
        assert!(generic.available, "detail={}", generic.detail);
    }

    #[test]
    fn port_from_rpc_url_parses_host_port() {
        assert_eq!(port_from_rpc_url("http://127.0.0.1:47800"), Some(47800));
        assert_eq!(port_from_rpc_url("http://127.0.0.1:9"), Some(9));
        assert_eq!(port_from_rpc_url("http://localhost:1234/rpc"), Some(1234));
        assert_eq!(port_from_rpc_url("http://127.0.0.1"), None);
        assert_eq!(port_from_rpc_url(""), None);
    }

    #[test]
    fn loopback_rpc_guard() {
        assert!(is_loopback_rpc("http://127.0.0.1:1234"));
        assert!(is_loopback_rpc("http://localhost:9"));
        assert!(!is_loopback_rpc("http://10.0.0.1:1234"));
        assert!(!is_loopback_rpc("https://127.0.0.1:1234"));
        assert!(!is_loopback_rpc(""));
    }

    #[test]
    fn status_dead_host_is_alive_false() {
        let _lock = lock_env();
        let tmp = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("RUSTTRAYCER_HOME", tmp.path());
        let report = status().unwrap();
        assert!(!report.alive);
        assert!(report.pid.is_none());
        assert!(report.rpc_url.is_none());
        assert_eq!(report.data_dir, tmp.path().join("host").to_string_lossy());
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["alive"], false);
    }

    #[test]
    fn status_live_pid_does_not_need_rpc() {
        let _lock = lock_env();
        let tmp = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("RUSTTRAYCER_HOME", tmp.path());
        write_pid(&tmp.path().join("host"), std::process::id());
        let report = status().unwrap();
        assert!(report.alive);
        assert_eq!(report.pid, Some(std::process::id()));
        assert_eq!(report.rpc_url.as_deref(), Some("http://127.0.0.1:9"));
    }

    #[test]
    fn logs_missing_file_is_empty() {
        let _lock = lock_env();
        let tmp = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("RUSTTRAYCER_HOME", tmp.path());
        assert_eq!(logs(10).unwrap(), "");
    }

    #[test]
    fn logs_tails_last_lines() {
        let _lock = lock_env();
        let tmp = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("RUSTTRAYCER_HOME", tmp.path());
        let host = tmp.path().join("host");
        fs::create_dir_all(&host).unwrap();
        fs::write(log_path(&host), "a\nb\nc\nd\n").unwrap();
        assert_eq!(logs(2).unwrap(), "c\nd\n");
        assert_eq!(logs(0).unwrap(), "d\n"); // clamp to 1
        fs::write(host.join("scrollback"), "PTY").unwrap();
        assert!(!logs(10).unwrap().contains("PTY"));
    }

    #[test]
    fn reset_db_requires_yes_and_refuses_running() {
        let _lock = lock_env();
        let tmp = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("RUSTTRAYCER_HOME", tmp.path());
        let err = reset_db(false).unwrap_err();
        assert_eq!(err.code(), "reset_needs_yes");
        write_pid(&tmp.path().join("host"), std::process::id());
        let err = reset_db(true).unwrap_err();
        assert_eq!(err.code(), "host_running");
    }

    #[test]
    fn reset_db_removes_sqlite_files_keeps_log() {
        let _lock = lock_env();
        let tmp = tempfile::tempdir().unwrap();
        let _home = EnvGuard::set("RUSTTRAYCER_HOME", tmp.path());
        let host = tmp.path().join("host");
        fs::create_dir_all(&host).unwrap();
        fs::write(host.join("host.db"), b"db").unwrap();
        fs::write(host.join("host.db-wal"), b"wal").unwrap();
        fs::write(host.join("host.db-shm"), b"shm").unwrap();
        fs::write(host.join("host.log"), b"keep").unwrap();
        fs::write(host.join("agent-selection-guide.md"), b"keep").unwrap();
        reset_db(true).unwrap();
        assert!(!host.join("host.db").exists());
        assert!(!host.join("host.db-wal").exists());
        assert!(!host.join("host.db-shm").exists());
        assert_eq!(fs::read(host.join("host.log")).unwrap(), b"keep");
        assert_eq!(
            fs::read(host.join("agent-selection-guide.md")).unwrap(),
            b"keep"
        );
    }

    #[test]
    fn follow_log_missing_file_stop_immediately_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("host.log");
        let mut out = Vec::new();
        follow_log_to(&path, 200, &mut out, || true).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn follow_log_prints_tail_then_new_lines_then_stops() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("host.log");
        fs::write(&path, "a\nb\nc\n").unwrap();
        let mut out = Vec::new();
        let mut step = 0u8;
        follow_log_to(&path, 2, &mut out, || {
            step += 1;
            if step == 1 {
                let mut f = fs::OpenOptions::new().append(true).open(&path).unwrap();
                f.write_all(b"d\ne\n").unwrap();
                false
            } else {
                true
            }
        })
        .unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), "b\nc\nd\ne\n");
    }

    #[test]
    fn follow_log_waits_for_missing_file_then_tails() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("host.log");
        let mut out = Vec::new();
        let mut step = 0u8;
        follow_log_to(&path, 10, &mut out, || {
            step += 1;
            if step == 1 {
                fs::write(&path, "hello\nworld\n").unwrap();
                false
            } else {
                true
            }
        })
        .unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), "hello\nworld\n");
    }

    #[test]
    fn follow_log_clamps_lines_like_logs() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("host.log");
        fs::write(&path, "a\nb\nc\n").unwrap();
        let mut out = Vec::new();
        follow_log_to(&path, 0, &mut out, || true).unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), "c\n");
    }
}
