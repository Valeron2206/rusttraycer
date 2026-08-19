//! In-memory live PTY table. Scrollback dies with the process. No sqlite.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::pty::SpawnedPty;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PtyKind {
    Agent,
    Shell,
}

#[derive(Debug, Clone)]
pub struct PtySession {
    pub pty_id: String,
    pub kind: PtyKind,
    pub entity_id: String,
    pub pid: u32,
    pub cols: u16,
    pub rows: u16,
    pub task_id: String,
    pub cwd: String,
}

struct LivePty {
    session: PtySession,
    handle: SpawnedPty,
}

#[derive(Default)]
struct Inner {
    by_id: HashMap<String, LivePty>,
    by_entity: HashMap<(PtyKind, String), String>,
}

pub struct Mux {
    inner: Mutex<Inner>,
}

impl Mux {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
        }
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Inner>, String> {
        self.inner
            .lock()
            .map_err(|_| "mux lock poisoned".to_string())
    }

    /// Live session for this entity, if the child is still running.
    pub fn live_for_entity(
        &self,
        kind: PtyKind,
        entity_id: &str,
    ) -> Result<Option<PtySession>, String> {
        let mut g = self.lock()?;
        let key = (kind, entity_id.to_string());
        let Some(pty_id) = g.by_entity.get(&key).cloned() else {
            return Ok(None);
        };
        let alive = g
            .by_id
            .get(&pty_id)
            .map(|l| l.handle.is_alive())
            .unwrap_or(false);
        if !alive {
            g.by_id.remove(&pty_id);
            g.by_entity.remove(&key);
            return Ok(None);
        }
        Ok(g.by_id.get(&pty_id).map(|l| l.session.clone()))
    }

    pub fn get(&self, pty_id: &str) -> Result<Option<PtySession>, String> {
        let mut g = self.lock()?;
        let alive = g
            .by_id
            .get(pty_id)
            .map(|l| l.handle.is_alive())
            .unwrap_or(false);
        if !alive {
            if let Some(live) = g.by_id.remove(pty_id) {
                g.by_entity
                    .remove(&(live.session.kind, live.session.entity_id));
            }
            return Ok(None);
        }
        Ok(g.by_id.get(pty_id).map(|l| l.session.clone()))
    }

    pub fn insert(&self, session: PtySession, handle: SpawnedPty) -> Result<PtySession, String> {
        let mut g = self.lock()?;
        let key = (session.kind, session.entity_id.clone());
        g.by_entity.insert(key, session.pty_id.clone());
        let out = session.clone();
        g.by_id
            .insert(session.pty_id.clone(), LivePty { session, handle });
        Ok(out)
    }

    pub fn write(&self, pty_id: &str, data: &[u8]) -> Result<(), MuxIoError> {
        let g = self.lock().map_err(MuxIoError::Internal)?;
        let live = g.by_id.get(pty_id).ok_or(MuxIoError::Dead)?;
        if !live.handle.is_alive() {
            return Err(MuxIoError::Dead);
        }
        live.handle.write_bytes(data).map_err(MuxIoError::Internal)
    }

    pub fn resize(&self, pty_id: &str, cols: u16, rows: u16) -> Result<(), MuxIoError> {
        let mut g = self.lock().map_err(MuxIoError::Internal)?;
        let live = g.by_id.get_mut(pty_id).ok_or(MuxIoError::Dead)?;
        if !live.handle.is_alive() {
            return Err(MuxIoError::Dead);
        }
        live.handle
            .resize(cols, rows)
            .map_err(MuxIoError::Internal)?;
        live.session.cols = cols;
        live.session.rows = rows;
        Ok(())
    }

    pub fn kill(&self, pty_id: &str) -> Result<Option<PtySession>, String> {
        let mut g = self.lock()?;
        let Some(live) = g.by_id.remove(pty_id) else {
            return Ok(None);
        };
        g.by_entity
            .remove(&(live.session.kind, live.session.entity_id.clone()));
        if let Err(e) = live.handle.kill() {
            tracing::warn!(pty_id, error = %e, "pty kill failed");
        }
        Ok(Some(live.session))
    }

    pub fn kill_entity(
        &self,
        kind: PtyKind,
        entity_id: &str,
    ) -> Result<Option<PtySession>, String> {
        let pty_id = {
            let g = self.lock()?;
            g.by_entity.get(&(kind, entity_id.to_string())).cloned()
        };
        match pty_id {
            Some(id) => self.kill(&id),
            None => Ok(None),
        }
    }

    pub fn remove_if_present(&self, pty_id: &str) -> Result<Option<PtySession>, String> {
        let mut g = self.lock()?;
        let Some(live) = g.by_id.remove(pty_id) else {
            return Ok(None);
        };
        g.by_entity
            .remove(&(live.session.kind, live.session.entity_id.clone()));
        Ok(Some(live.session))
    }

    pub fn list_shells(&self, task_id: &str) -> Result<Vec<PtySession>, String> {
        let mut g = self.lock()?;
        let mut dead = Vec::new();
        let mut out = Vec::new();
        for (id, live) in g.by_id.iter() {
            if live.session.kind != PtyKind::Shell {
                continue;
            }
            if !live.handle.is_alive() {
                dead.push(id.clone());
                continue;
            }
            if live.session.task_id == task_id {
                out.push(live.session.clone());
            }
        }
        for id in dead {
            if let Some(live) = g.by_id.remove(&id) {
                g.by_entity
                    .remove(&(live.session.kind, live.session.entity_id));
            }
        }
        out.sort_by(|a, b| a.pty_id.cmp(&b.pty_id));
        Ok(out)
    }
}

#[derive(Debug)]
pub enum MuxIoError {
    Dead,
    Internal(String),
}

impl Default for Mux {
    fn default() -> Self {
        Self::new()
    }
}
