//! RustTraycer host daemon library: HTTP/WS, domain, agent supervisor.

pub mod bind;
pub mod files;
pub mod handshake;
pub mod mux;
pub mod pty;
pub mod rpc;
pub mod service;
pub mod supervisor;
pub mod worktree;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use rt_runtime::AgentBackend;
use rt_storage::Store;

pub use rt_storage::HarnessId;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use crate::bind::PidFile;
use crate::service::HostService;

pub type Result<T> = std::result::Result<T, HostError>;

#[derive(Debug, thiserror::Error)]
pub enum HostError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid params: {0}")]
    InvalidParams(String),
    #[error("agent is busy")]
    AgentBusy,
    #[error("denied")]
    Denied,
    #[error("approval expired")]
    ApprovalExpired,
    #[error("workspace path invalid: {0}")]
    WorkspacePathInvalid(String),
    #[error("unsupported method: {0}")]
    UnsupportedMethod(String),
    #[error("version mismatch: {0}")]
    VersionMismatch(String),
    #[error("unauthorized")]
    Unauthorized,
    #[error("file too large: {0}")]
    FileTooLarge(String),
    #[error("file is binary: {0}")]
    FileBinary(String),
    #[error("git identity: {0}")]
    GitIdentity(String),
    #[error("git auth: {0}")]
    GitAuth(String),
    #[error("git conflict: {0}")]
    GitConflict(String),
    #[error("patch failed: {0}")]
    PatchFailed(String),
    #[error("not a pty harness")]
    NotPty,
    #[error("pty is dead")]
    PtyDead,
    #[error("{0}")]
    Internal(String),
    #[error("already running (pid {pid})")]
    AlreadyRunning { pid: u32, rpc_url: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl HostError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound(_) => "not_found",
            Self::InvalidParams(_) => "invalid_params",
            Self::AgentBusy => "agent_busy",
            Self::Denied => "denied",
            Self::ApprovalExpired => "approval_expired",
            Self::WorkspacePathInvalid(_) => "workspace_path_invalid",
            Self::UnsupportedMethod(_) => "unsupported_method",
            Self::VersionMismatch(_) => "version_mismatch",
            Self::Unauthorized => "unauthorized",
            Self::FileTooLarge(_) => "file_too_large",
            Self::FileBinary(_) => "file_binary",
            Self::GitIdentity(_) => "git_identity",
            Self::GitAuth(_) => "git_auth",
            Self::GitConflict(_) => "git_conflict",
            Self::PatchFailed(_) => "patch_failed",
            Self::NotPty => "not_pty",
            Self::PtyDead => "pty_dead",
            Self::Internal(_) | Self::Io(_) | Self::Json(_) => "internal",
            Self::AlreadyRunning { .. } => "already_running",
        }
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            Self::AlreadyRunning { .. } => 2,
            _ => 1,
        }
    }
}

impl From<rt_storage::StorageError> for HostError {
    fn from(e: rt_storage::StorageError) -> Self {
        match e {
            rt_storage::StorageError::NotFound => Self::NotFound("not found".into()),
            rt_storage::StorageError::WorkspacePathInvalid(s) => Self::WorkspacePathInvalid(s),
            rt_storage::StorageError::InvalidParams(s) => Self::InvalidParams(s),
            rt_storage::StorageError::UnsupportedSchema(s) => {
                Self::Internal(format!("unsupported schema: {s}"))
            }
            other => Self::Internal(other.to_string()),
        }
    }
}

/// Doctor probe: env set? Does not spawn a child.
pub fn generic_cmd_probe() -> rt_runtime::Availability {
    match std::env::var("RUSTTRAYCER_GENERIC_CMD") {
        Ok(s) if !s.trim().is_empty() => rt_runtime::Availability {
            available: true,
            detail: format!("RUSTTRAYCER_GENERIC_CMD={}", s.trim()),
        },
        _ => rt_runtime::Availability {
            available: false,
            detail: "RUSTTRAYCER_GENERIC_CMD unset".into(),
        },
    }
}

fn init_tracing(log_path: &Path) -> Result<()> {
    use std::fs::OpenOptions;
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::{fmt, EnvFilter};

    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    let file = Arc::new(std::sync::Mutex::new(file));
    let make = FileMake(file);
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_ansi(false).with_writer(make))
        .with(fmt::layer().with_writer(std::io::stderr))
        .try_init();
    Ok(())
}

#[derive(Clone)]
struct FileMake(Arc<std::sync::Mutex<std::fs::File>>);

impl std::io::Write for FileMake {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| std::io::Error::other("poison"))?
            .write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0
            .lock()
            .map_err(|_| std::io::Error::other("poison"))?
            .flush()
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for FileMake {
    type Writer = FileMake;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

pub struct HostConfig {
    pub data_dir: PathBuf,
    pub init_tracing: bool,
    pub backends: Option<HashMap<String, Arc<dyn AgentBackend>>>,
}

impl HostConfig {
    pub fn from_env() -> Self {
        Self {
            data_dir: bind::resolve_data_dir(),
            init_tracing: true,
            backends: None,
        }
    }
}

pub struct RunningHost {
    pub service: HostService,
    pub listener: TcpListener,
    pub data_dir: PathBuf,
    pub pid: u32,
    pub addr: std::net::SocketAddr,
}

/// Open DB, lock pid, bind loopback, write pid.json. Does not serve yet.
pub async fn prepare(config: HostConfig) -> Result<RunningHost> {
    let data_dir = config.data_dir;
    std::fs::create_dir_all(&data_dir)?;
    if config.init_tracing {
        if let Err(e) = init_tracing(&bind::log_path(&data_dir)) {
            tracing::error!(error = %e, "init_tracing failed");
        }
    }
    tracing::info!(data_dir = %data_dir.display(), "prepare");

    let db = bind::db_path(&data_dir);
    let store = Store::open(&db)?;
    store.migrate()?;

    let host_id = match store.host_get() {
        Ok(h) => h.id,
        Err(rt_storage::StorageError::NotFound) => {
            let id = rt_storage::new_id();
            store.host_insert_if_absent(&id, "rusttraycer")?;
            id
        }
        Err(e) => return Err(e.into()),
    };
    store.host_insert_if_absent(&host_id, "rusttraycer")?;
    let host_id = store.host_get()?.id;

    let pid = std::process::id();
    bind::check_not_already_running(&data_dir, pid)?;

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    if !addr.ip().is_loopback() {
        return Err(HostError::Internal("refusing non-loopback bind".into()));
    }

    let info = PidFile::new(host_id.clone(), pid, addr.port());
    bind::write_pid_file(&data_dir, &info)?;

    if let Err(e) = store.set_running_agents_to_error() {
        tracing::warn!("restart recovery: {e}");
    }

    let backends = config.backends.unwrap_or_else(|| {
        let mut m: HashMap<String, Arc<dyn AgentBackend>> = HashMap::new();
        let generic = rt_runtime::CliGeneric::from_env();
        m.insert(generic.id().to_string(), Arc::new(generic));
        let claude = rt_runtime::CliClaude::from_env();
        m.insert(claude.id().to_string(), Arc::new(claude));
        let codex = rt_runtime::CliCodex::from_env();
        m.insert(codex.id().to_string(), Arc::new(codex));
        m
    });
    let service = HostService::new(
        store,
        backends,
        host_id,
        data_dir.clone(),
        info.rpc_url,
        pid,
    );

    Ok(RunningHost {
        service,
        listener,
        data_dir,
        pid,
        addr,
    })
}

pub async fn serve(
    host: RunningHost,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) -> Result<()> {
    let data_dir = host.data_dir.clone();
    let pid = host.pid;
    let service = host.service.clone();
    let app = rpc::router(host.service);

    let result = axum::serve(host.listener, app)
        .with_graceful_shutdown(async move {
            shutdown.await;
            tracing::info!("shutdown start");
            service.going_away();
            service.inflight().shutdown(Duration::from_secs(2)).await;
            if let Err(e) = service.store.checkpoint() {
                tracing::error!(error = %e, "wal checkpoint failed");
            }
            tracing::info!("shutdown done");
        })
        .await;

    bind::remove_pid_file_if_ours(&data_dir, pid)?;
    result.map_err(|e| HostError::Internal(e.to_string()))
}

/// Process entry: prepare + serve until SIGINT/SIGTERM.
pub fn run() -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| HostError::Internal(e.to_string()))?;
    rt.block_on(async {
        let host = prepare(HostConfig::from_env()).await?;
        tracing::info!(
            host_id = %host.service.host_id(),
            addr = %host.addr,
            "rt-host listening"
        );
        serve(host, shutdown_signal()).await
    })
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {}
        _ = term => {}
    }
}

/// Test helper: start host on a temp data dir, return addr + shutdown.
pub async fn spawn_test_host(
    data_dir: &Path,
    backends: Option<HashMap<String, Arc<dyn AgentBackend>>>,
) -> Result<(
    std::net::SocketAddr,
    oneshot::Sender<()>,
    tokio::task::JoinHandle<Result<()>>,
    String,
)> {
    let host = prepare(HostConfig {
        data_dir: data_dir.to_path_buf(),
        init_tracing: false,
        backends,
    })
    .await?;
    let addr = host.addr;
    let host_id = host.service.host_id().to_string();
    let (tx, rx) = oneshot::channel::<()>();
    let join = tokio::spawn(async move {
        serve(host, async move {
            let _ = rx.await;
        })
        .await
    });
    Ok((addr, tx, join, host_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn serve_shutdown_removes_pid_file() {
        let dir = tempfile::tempdir().unwrap();
        let host = prepare(HostConfig {
            data_dir: dir.path().to_path_buf(),
            init_tracing: false,
            backends: None,
        })
        .await
        .unwrap();
        let store = host.service.store.clone();
        let pid_path = bind::pid_path(dir.path());
        assert!(pid_path.exists(), "prepare writes pid.json");

        serve(host, std::future::ready(())).await.unwrap();

        assert!(
            !pid_path.exists(),
            "pid.json must be gone after serve returns"
        );
        store
            .checkpoint()
            .expect("TRUNCATE checkpoint succeeds on a real store");
    }

    #[tokio::test]
    async fn prepare_second_host_is_already_running() {
        let dir = tempfile::tempdir().unwrap();
        let _host = prepare(HostConfig {
            data_dir: dir.path().to_path_buf(),
            init_tracing: false,
            backends: None,
        })
        .await
        .unwrap();

        // First prepare wrote live pid = this process. Same-process prepare
        // treats self as allowed, so rewrite as a live foreign pid.
        let mut info = bind::read_pid_file(dir.path()).unwrap().unwrap();
        assert_eq!(info.pid, std::process::id());
        info.pid = 1; // init, always alive
        bind::write_pid_file(dir.path(), &info).unwrap();

        let err = match prepare(HostConfig {
            data_dir: dir.path().to_path_buf(),
            init_tracing: false,
            backends: None,
        })
        .await
        {
            Err(e) => e,
            Ok(_) => panic!("expected already_running"),
        };
        assert_eq!(err.code(), "already_running");
        assert_eq!(err.exit_code(), 2);
    }
    #[test]
    fn host_error_codes_exit_and_from_storage() {
        assert_eq!(HostError::NotFound("x".into()).code(), "not_found");
        assert_eq!(
            HostError::InvalidParams("x".into()).code(),
            "invalid_params"
        );
        assert_eq!(HostError::AgentBusy.code(), "agent_busy");
        assert_eq!(HostError::Denied.code(), "denied");
        assert_eq!(HostError::ApprovalExpired.code(), "approval_expired");
        assert_eq!(
            HostError::WorkspacePathInvalid("x".into()).code(),
            "workspace_path_invalid"
        );
        assert_eq!(
            HostError::UnsupportedMethod("x".into()).code(),
            "unsupported_method"
        );
        assert_eq!(
            HostError::VersionMismatch("x".into()).code(),
            "version_mismatch"
        );
        assert_eq!(HostError::Unauthorized.code(), "unauthorized");
        assert_eq!(HostError::FileTooLarge("x".into()).code(), "file_too_large");
        assert_eq!(HostError::FileBinary("x".into()).code(), "file_binary");
        assert_eq!(HostError::GitIdentity("x".into()).code(), "git_identity");
        assert_eq!(HostError::GitAuth("x".into()).code(), "git_auth");
        assert_eq!(HostError::GitConflict("x".into()).code(), "git_conflict");
        assert_eq!(HostError::PatchFailed("x".into()).code(), "patch_failed");
        assert_eq!(HostError::NotPty.code(), "not_pty");
        assert_eq!(HostError::PtyDead.code(), "pty_dead");
        assert_eq!(HostError::Internal("x".into()).code(), "internal");
        assert_eq!(
            HostError::AlreadyRunning {
                pid: 7,
                rpc_url: "http://127.0.0.1:1".into()
            }
            .code(),
            "already_running"
        );
        assert_eq!(
            HostError::AlreadyRunning {
                pid: 7,
                rpc_url: "u".into()
            }
            .exit_code(),
            2
        );
        assert_eq!(HostError::NotFound("x".into()).exit_code(), 1);
        assert_eq!(
            HostError::from(rt_storage::StorageError::NotFound).code(),
            "not_found"
        );
        assert_eq!(
            HostError::from(rt_storage::StorageError::WorkspacePathInvalid("p".into())).code(),
            "workspace_path_invalid"
        );
        assert_eq!(
            HostError::from(rt_storage::StorageError::InvalidParams("p".into())).code(),
            "invalid_params"
        );
        assert_eq!(
            HostError::from(rt_storage::StorageError::UnsupportedSchema("9".into())).code(),
            "internal"
        );
        assert_eq!(
            HostError::from(rt_storage::StorageError::Io(std::io::Error::other("e"))).code(),
            "internal"
        );
    }

    #[test]
    fn generic_cmd_probe_set_and_unset() {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _g = LOCK.lock().unwrap();
        let prev = std::env::var("RUSTTRAYCER_GENERIC_CMD").ok();
        std::env::remove_var("RUSTTRAYCER_GENERIC_CMD");
        let down = generic_cmd_probe();
        assert!(!down.available);
        assert!(down.detail.contains("unset"));
        std::env::set_var("RUSTTRAYCER_GENERIC_CMD", "  echo  ");
        let up = generic_cmd_probe();
        assert!(up.available);
        assert!(up.detail.contains("echo"));
        match prev {
            Some(v) => std::env::set_var("RUSTTRAYCER_GENERIC_CMD", v),
            None => std::env::remove_var("RUSTTRAYCER_GENERIC_CMD"),
        }
    }
}
