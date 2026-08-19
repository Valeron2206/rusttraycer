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

#[cfg(test)]
mod tests {
    use super::*;
    use futures::Stream;
    use rt_runtime::{AgentBackend, Availability, TurnEvent, TurnRequest};
    use rt_storage::{new_id, AgentStatus, Store};
    use std::pin::Pin;
    use std::sync::Arc;
    use std::time::Duration;
    use tempfile::tempdir;

    struct SeqBackend(Vec<TurnEvent>);

    impl AgentBackend for SeqBackend {
        fn id(&self) -> &'static str {
            "cli.generic"
        }
        fn available(&self) -> Availability {
            Availability {
                available: true,
                detail: "seq".into(),
            }
        }
        fn start_turn(&self, _req: TurnRequest) -> Pin<Box<dyn Stream<Item = TurnEvent> + Send>> {
            Box::pin(futures::stream::iter(self.0.clone()))
        }
    }

    fn empty_req() -> TurnRequest {
        TurnRequest {
            agent_id: "a".into(),
            task_id: "t".into(),
            workspace_path: ".".into(),
            messages: vec![],
            extra_env: Default::default(),
        }
    }

    fn seeded_store() -> (tempfile::TempDir, Store, String, String) {
        let dir = tempdir().unwrap();
        let store = Store::open(dir.path().join("host.db")).unwrap();
        let host_id = new_id();
        store.host_insert_if_absent(&host_id, "h").unwrap();
        let ws = store.workspace_add("/p", "p").unwrap();
        let task = store.task_create("t", &ws.id).unwrap();
        let agent = store
            .agent_create(&task.id, &host_id, "cli.generic")
            .unwrap();
        (dir, store, task.id, agent.id)
    }

    #[tokio::test]
    async fn inflight_reserve_busy_owns_take_remove_if() {
        let inf = Inflight::new();
        let gen = inf.reserve("a1").unwrap();
        assert!(inf.contains("a1").unwrap());
        assert!(inf.owns("a1", gen));
        assert!(!inf.owns("a1", gen + 1));
        assert_eq!(inf.reserve("a1").unwrap_err().code(), "agent_busy");
        inf.remove_if("a1", gen + 1);
        assert!(inf.contains("a1").unwrap());
        inf.remove_if("a1", gen);
        assert!(!inf.contains("a1").unwrap());
        let gen2 = inf.reserve("a1").unwrap();
        let mismatch = tokio::spawn(async {});
        inf.attach("a1", gen2 + 1, mismatch);
        let ok = tokio::spawn(async {});
        inf.attach("a1", gen2, ok);
        assert!(inf.take("a1").unwrap().is_some());
        assert!(inf.take("missing").unwrap().is_none());
        inf.abort_all();
    }

    #[tokio::test]
    async fn inflight_shutdown_empty_and_with_handle() {
        let inf = Inflight::new();
        inf.shutdown(Duration::from_millis(10)).await;
        let gen = inf.reserve("a1").unwrap();
        let handle = tokio::spawn(async {
            tokio::time::sleep(Duration::from_millis(5)).await;
        });
        inf.attach("a1", gen, handle);
        inf.shutdown(Duration::from_millis(200)).await;
    }

    #[tokio::test]
    async fn run_turn_failed_none_tool_and_cancelled() {
        let (_d, store, task_id, agent_id) = seeded_store();
        store
            .agent_set_status(&agent_id, AgentStatus::Running)
            .unwrap();
        let (tx, _rx) = tokio::sync::broadcast::channel(8);
        let inf = Inflight::new();
        let gen = inf.reserve(&agent_id).unwrap();
        run_turn(
            store.clone(),
            Arc::new(SeqBackend(vec![
                TurnEvent::Tool {
                    name: "x".into(),
                    payload: serde_json::json!({}),
                },
                TurnEvent::Token {
                    text: "partial".into(),
                },
                TurnEvent::Failed {
                    message: "boom".into(),
                },
            ])),
            empty_req(),
            agent_id.clone(),
            task_id.clone(),
            tx.clone(),
            (inf.clone(), gen),
        )
        .await;
        assert_eq!(
            store.agent_get(&agent_id).unwrap().unwrap().status,
            AgentStatus::Error
        );

        let (_d2, store2, task2, agent2) = seeded_store();
        store2
            .agent_set_status(&agent2, AgentStatus::Running)
            .unwrap();
        let inf2 = Inflight::new();
        let gen2 = inf2.reserve(&agent2).unwrap();
        run_turn(
            store2.clone(),
            Arc::new(SeqBackend(vec![TurnEvent::Failed {
                message: "cancelled".into(),
            }])),
            empty_req(),
            agent2.clone(),
            task2,
            tx.clone(),
            (inf2, gen2),
        )
        .await;
        assert_eq!(
            store2.agent_get(&agent2).unwrap().unwrap().status,
            AgentStatus::Running
        );

        let (_d3, store3, task3, agent3) = seeded_store();
        store3
            .agent_set_status(&agent3, AgentStatus::Running)
            .unwrap();
        let inf3 = Inflight::new();
        let gen3 = inf3.reserve(&agent3).unwrap();
        run_turn(
            store3.clone(),
            Arc::new(SeqBackend(vec![])),
            empty_req(),
            agent3.clone(),
            task3,
            tx,
            (inf3, gen3),
        )
        .await;
        assert_eq!(
            store3.agent_get(&agent3).unwrap().unwrap().status,
            AgentStatus::Error
        );
    }
}
