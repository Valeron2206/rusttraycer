//! Host discovery via pid.json only. Never spawn a process.

use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

use crate::state::PidInfo;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PidFile {
    host_id: String,
    pid: u64,
    rpc_url: String,
    #[serde(default)]
    ws_url: Option<String>,
    #[serde(default)]
    started_at: Option<String>,
}

/// `$RUSTTRAYCER_HOME` or `~/.rusttraycer`.
pub fn rusttraycer_home() -> PathBuf {
    if let Ok(value) = std::env::var("RUSTTRAYCER_HOME") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("."));
    PathBuf::from(home).join(".rusttraycer")
}

pub fn pid_json_path() -> PathBuf {
    rusttraycer_home().join("host").join("pid.json")
}

#[derive(Debug)]
pub enum DiscoverError {
    Missing,
    Unreadable(String),
}

impl DiscoverError {
    pub fn as_label(&self) -> String {
        match self {
            Self::Missing => "pid.json отсутствует".to_string(),
            Self::Unreadable(msg) => format!("pid.json не прочитан: {msg}"),
        }
    }
}

/// Read `~/.rusttraycer/host/pid.json`. Missing/unreadable is an error, never a panic.
pub fn read_pid_json() -> Result<PidInfo, DiscoverError> {
    let path = pid_json_path();
    let raw = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(DiscoverError::Missing);
        }
        Err(err) => return Err(DiscoverError::Unreadable(err.to_string())),
    };

    let parsed: PidFile = serde_json::from_str(&raw)
        .map_err(|err| DiscoverError::Unreadable(err.to_string()))?;

    Ok(PidInfo {
        host_id: parsed.host_id,
        pid: parsed.pid,
        rpc_url: parsed.rpc_url,
        ws_url: parsed.ws_url,
        started_at: parsed.started_at,
    })
}
