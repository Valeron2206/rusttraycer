//! In-process PTY via `portable-pty`. GUI is a byte client; host owns the child.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};

type ChildHandle = Box<dyn portable_pty::Child + Send + Sync>;

pub struct SpawnedPty {
    pub pid: u32,
    writer: Mutex<Box<dyn Write + Send>>,
    child: Arc<Mutex<ChildHandle>>,
    master: Mutex<Box<dyn MasterPty + Send>>,
}

#[derive(Debug, Clone)]
pub struct SpawnSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: std::path::PathBuf,
    pub cols: u16,
    pub rows: u16,
    pub env: Vec<(String, String)>,
}

impl SpawnedPty {
    pub fn write_bytes(&self, data: &[u8]) -> Result<(), String> {
        let mut w = self
            .writer
            .lock()
            .map_err(|_| "pty writer lock poisoned".to_string())?;
        w.write_all(data).map_err(|e| e.to_string())?;
        w.flush().map_err(|e| e.to_string())
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), String> {
        let master = self
            .master
            .lock()
            .map_err(|_| "pty master lock poisoned".to_string())?;
        master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| e.to_string())
    }

    pub fn kill(&self) -> Result<(), String> {
        let mut child = self
            .child
            .lock()
            .map_err(|_| "pty child lock poisoned".to_string())?;
        child.kill().map_err(|e| e.to_string())
    }

    pub fn try_wait(&self) -> Result<Option<u32>, String> {
        let mut child = self
            .child
            .lock()
            .map_err(|_| "pty child lock poisoned".to_string())?;
        match child.try_wait() {
            Ok(Some(st)) => Ok(Some(st.exit_code())),
            Ok(None) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn is_alive(&self) -> bool {
        matches!(self.try_wait(), Ok(None))
    }
}

/// Command for a terminal-agent PTY. `$RUSTTRAYCER_PTY_CMD` overrides for tests.
pub fn agent_pty_command(launch_args: &[String]) -> (String, Vec<String>) {
    match std::env::var("RUSTTRAYCER_PTY_CMD") {
        Ok(raw) if !raw.trim().is_empty() => split_cmd(raw.trim(), launch_args),
        _ => ("/bin/bash".to_string(), launch_args.to_vec()),
    }
}

/// Command for a user shell. Test override: `$RUSTTRAYCER_PTY_CMD`, else `$SHELL`, else bash.
pub fn shell_pty_command() -> (String, Vec<String>) {
    match std::env::var("RUSTTRAYCER_PTY_CMD") {
        Ok(raw) if !raw.trim().is_empty() => split_cmd(raw.trim(), &[]),
        _ => match std::env::var("SHELL") {
            Ok(s) if !s.trim().is_empty() => (s, Vec::new()),
            _ => ("/bin/bash".to_string(), Vec::new()),
        },
    }
}

fn split_cmd(raw: &str, extra: &[String]) -> (String, Vec<String>) {
    let mut parts = raw.split_whitespace();
    match parts.next() {
        Some(prog) => {
            let mut args: Vec<String> = parts.map(str::to_string).collect();
            args.extend(extra.iter().cloned());
            (prog.to_string(), args)
        }
        None => ("/bin/bash".to_string(), extra.to_vec()),
    }
}

/// Spawn `program` in `cwd`. Reader thread invokes `on_data` / `on_exit`.
///
/// After the first successful spawn the host may store a generated
/// `provider_session_id`. Integration later replaces how that id is obtained
/// from the harness; F3 only persists a non-empty string so resume can be tested.
pub fn spawn(
    spec: &SpawnSpec,
    on_data: impl Fn(Vec<u8>) + Send + 'static,
    on_exit: impl Fn(u32) + Send + 'static,
) -> Result<SpawnedPty, String> {
    if !Path::new(&spec.cwd).is_dir() {
        return Err(format!("cwd is not a directory: {}", spec.cwd.display()));
    }
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: spec.rows,
            cols: spec.cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())?;

    let mut cmd = CommandBuilder::new(&spec.program);
    for a in &spec.args {
        cmd.arg(a);
    }
    cmd.cwd(&spec.cwd);
    for (k, v) in &spec.env {
        cmd.env(k, v);
    }

    let child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
    let pid = child.process_id().unwrap_or(0);
    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
    let child = Arc::new(Mutex::new(child));
    let child_wait = Arc::clone(&child);
    thread::Builder::new()
        .name("rt-pty-reader".into())
        .spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => on_data(buf[..n].to_vec()),
                    Err(_) => break,
                }
            }
            let code = match child_wait.lock() {
                Ok(mut c) => match c.wait() {
                    Ok(st) => st.exit_code(),
                    Err(_) => 1,
                },
                Err(_) => 1,
            };
            on_exit(code);
        })
        .map_err(|e| e.to_string())?;

    Ok(SpawnedPty {
        pid,
        writer: Mutex::new(writer),
        child,
        master: Mutex::new(pair.master),
    })
}
