//! In-flight turn supervision: one turn per agent, chunked transcript, panic-safe.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::{FutureExt, StreamExt};
use rt_runtime::{AgentBackend, TurnEvent, TurnRequest};
use rt_storage::{AgentStatus, MessageRole, Store};
use tokio::task::JoinHandle;
use tokio::time::interval;

use crate::rpc::WsEvent;
use crate::HostError;

const FLUSH_EVERY: Duration = Duration::from_millis(100);

/// Production turn deadline. Tests pass a shorter `Duration` via [`SpawnTurn::timeout`].
pub(crate) const TURN_TIMEOUT: Duration = Duration::from_secs(600);

struct Slot {
    gen: u64,
    handle: Option<JoinHandle<()>>,
}

#[derive(Clone)]
pub struct Inflight {
    inner: Arc<Mutex<HashMap<String, Slot>>>,
    next_gen: Arc<AtomicU64>,
}

impl Default for Inflight {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            next_gen: Arc::new(AtomicU64::new(1)),
        }
    }
}

impl Inflight {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock_map(&self) -> Result<std::sync::MutexGuard<'_, HashMap<String, Slot>>, HostError> {
        self.inner
            .lock()
            .map_err(|_| HostError::Internal("inflight lock poisoned".into()))
    }

    /// Reserve a generation slot. Fails with `AgentBusy` if one is already present.
    pub fn reserve(&self, agent_id: &str) -> Result<u64, HostError> {
        let mut g = self.lock_map()?;
        if g.contains_key(agent_id) {
            return Err(HostError::AgentBusy);
        }
        let gen = self.next_gen.fetch_add(1, Ordering::Relaxed);
        g.insert(agent_id.to_string(), Slot { gen, handle: None });
        Ok(gen)
    }

    /// Attach a join handle only if `gen` still owns the slot.
    pub fn attach(&self, agent_id: &str, gen: u64, handle: JoinHandle<()>) {
        if let Ok(mut g) = self.inner.lock() {
            if let Some(slot) = g.get_mut(agent_id) {
                if slot.gen == gen {
                    slot.handle = Some(handle);
                    return;
                }
            }
        }
        // Slot gone (cancel already took it), generation mismatch, or poison:
        // drop the handle so the turn task can flush leftover tokens.
        drop(handle);
    }

    /// Drop the slot only if it still belongs to `gen`.
    pub fn remove_if(&self, agent_id: &str, gen: u64) {
        if let Ok(mut g) = self.inner.lock() {
            if g.get(agent_id).map(|s| s.gen == gen).unwrap_or(false) {
                g.remove(agent_id);
            }
        }
    }

    pub fn contains(&self, agent_id: &str) -> Result<bool, HostError> {
        Ok(self.lock_map()?.contains_key(agent_id))
    }

    /// True if `gen` still owns the slot for `agent_id`.
    pub fn owns(&self, agent_id: &str, gen: u64) -> bool {
        match self.inner.lock() {
            Ok(g) => g.get(agent_id).is_some_and(|s| s.gen == gen),
            Err(_) => false,
        }
    }

    /// Remove the slot and return the join handle without aborting, so the
    /// old task can flush leftover tokens and exit. `None` if there was no slot.
    pub fn take(&self, agent_id: &str) -> Result<Option<JoinHandle<()>>, HostError> {
        Ok(self
            .lock_map()?
            .remove(agent_id)
            .and_then(|slot| slot.handle))
    }

    /// Wait up to `grace` for turns to finish, then abort the rest (kills children).
    pub async fn shutdown(&self, grace: Duration) {
        let handles: Vec<JoinHandle<()>> = {
            let mut g = match self.inner.lock() {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
            g.drain().filter_map(|(_, s)| s.handle).collect()
        };
        if handles.is_empty() {
            return;
        }
        let aborts: Vec<_> = handles.iter().map(|h| h.abort_handle()).collect();
        if tokio::time::timeout(grace, futures::future::join_all(handles))
            .await
            .is_err()
        {
            for a in aborts {
                a.abort();
            }
        }
        let mut g = match self.inner.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        for (_, s) in g.drain() {
            if let Some(h) = s.handle {
                h.abort();
            }
        }
    }

    pub fn abort_all(&self) {
        let mut g = match self.inner.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        for (_, s) in g.drain() {
            if let Some(h) = s.handle {
                h.abort();
            }
        }
    }
}

pub async fn run_turn(
    store: Store,
    backend: std::sync::Arc<dyn AgentBackend>,
    req: TurnRequest,
    agent_id: String,
    task_id: String,
    events: tokio::sync::broadcast::Sender<WsEvent>,
    ownership: (Inflight, u64),
) {
    tracing::info!(agent_id = %agent_id, task_id = %task_id, "turn start");
    let (inflight, gen) = ownership;
    let mut stream = backend.start_turn(req);
    let mut buf = String::new();
    let mut tick = interval(FLUSH_EVERY);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    // skip the immediate first tick
    tick.tick().await;

    let flush = |buf: &mut String,
                 store: &Store,
                 events: &tokio::sync::broadcast::Sender<WsEvent>,
                 agent_id: &str,
                 task_id: &str| {
        if buf.is_empty() {
            return;
        }
        let chunk = std::mem::take(buf);
        match store.message_append(agent_id, MessageRole::Assistant, &chunk) {
            Ok(msg) => {
                let _ = events.send(WsEvent::agent_message(task_id, agent_id, msg));
            }
            Err(e) => {
                tracing::error!("flush assistant chunk: {e}");
            }
        }
    };

    let set_status = |status: AgentStatus| {
        // Only the generation that still owns the slot may write a terminal
        // status. A cancelled or superseded turn must not clobber a newer Running.
        if !inflight.owns(&agent_id, gen) {
            return;
        }
        if let Err(e) = store.agent_set_status(&agent_id, status) {
            tracing::error!("set agent status: {e}");
        }
        let _ = events.send(WsEvent::agent_status(&task_id, &agent_id, status));
    };

    loop {
        tokio::select! {
            ev = stream.next() => {
                match ev {
                    Some(TurnEvent::Token { text }) => {
                        buf.push_str(&text);
                        if buf.contains('\n') {
                            flush(&mut buf, &store, &events, &agent_id, &task_id);
                        }
                    }
                    Some(TurnEvent::Tool { name, .. }) => {
                        tracing::debug!(target: "supervisor", "ignoring tool event {name}");
                    }
                    Some(TurnEvent::Finished { exit_code }) => {
                        tracing::info!(agent_id, exit_code, "turn finished");
                        flush(&mut buf, &store, &events, &agent_id, &task_id);
                        set_status(AgentStatus::Idle);
                        return;
                    }
                    Some(TurnEvent::Failed { message }) => {
                        tracing::warn!(agent_id, %message, "turn failed");
                        flush(&mut buf, &store, &events, &agent_id, &task_id);
                        // Cancel may emit Failed { cancelled } before take().
                        // That is not a failure even if this generation still owns.
                        if message != "cancelled" {
                            set_status(AgentStatus::Error);
                        }
                        return;
                    }
                    None => {
                        flush(&mut buf, &store, &events, &agent_id, &task_id);
                        set_status(AgentStatus::Error);
                        return;
                    }
                }
            }
            _ = tick.tick() => {
                if !buf.is_empty() {
                    flush(&mut buf, &store, &events, &agent_id, &task_id);
                }
            }
        }
    }
}

/// Arguments for [`spawn_turn`]. Bundled so the helper stays under clippy's
/// `too_many_arguments` limit.
pub(crate) struct SpawnTurn {
    pub store: Store,
    pub backend: std::sync::Arc<dyn AgentBackend>,
    pub req: TurnRequest,
    pub agent_id: String,
    pub task_id: String,
    pub events: tokio::sync::broadcast::Sender<WsEvent>,
    pub inflight: Inflight,
    pub gen: u64,
    pub timeout: Duration,
}

/// Spawn a panic-safe turn task. Caller must have reserved `agent_id` in `inflight`.
pub(crate) fn spawn_turn(args: SpawnTurn) -> JoinHandle<()> {
    let SpawnTurn {
        store,
        backend,
        req,
        agent_id,
        task_id,
        events,
        inflight,
        gen,
        timeout,
    } = args;
    let agent_for_err = agent_id.clone();
    let task_for_err = task_id.clone();
    let store_for_err = store.clone();
    let events_for_err = events.clone();
    let inflight_done = inflight.clone();
    let agent_done = agent_id.clone();

    tokio::spawn(async move {
        let fut = run_turn(
            store,
            backend,
            req,
            agent_id,
            task_id,
            events,
            (inflight_done.clone(), gen),
        );
        let timed = tokio::time::timeout(timeout, fut);
        let outcome = std::panic::AssertUnwindSafe(timed).catch_unwind().await;
        match outcome {
            Ok(Ok(())) => {}
            Ok(Err(_elapsed)) => {
                tracing::warn!(agent_id = %agent_for_err, "turn timeout");
                if inflight_done.owns(&agent_done, gen) {
                    if let Err(e) =
                        store_for_err.agent_set_status(&agent_for_err, AgentStatus::Error)
                    {
                        tracing::error!(error = %e, "set agent status after timeout");
                    }
                    if events_for_err
                        .send(WsEvent::agent_status(
                            &task_for_err,
                            &agent_for_err,
                            AgentStatus::Error,
                        ))
                        .is_err()
                    {
                        tracing::debug!("no ws subscribers for timeout status");
                    }
                }
            }
            Err(_) => {
                tracing::error!(agent_id = %agent_for_err, "turn task panicked");
                if inflight_done.owns(&agent_done, gen) {
                    if let Err(e) =
                        store_for_err.agent_set_status(&agent_for_err, AgentStatus::Error)
                    {
                        tracing::error!(error = %e, "set agent status after panic");
                    }
                    if events_for_err
                        .send(WsEvent::agent_status(
                            &task_for_err,
                            &agent_for_err,
                            AgentStatus::Error,
                        ))
                        .is_err()
                    {
                        tracing::debug!("no ws subscribers for panic status");
                    }
                }
            }
        }
        inflight_done.remove_if(&agent_done, gen);
    })
}
