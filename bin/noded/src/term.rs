//! node-local, off-chain interactive terminal sessions.
//!
//! A terminal session is an ephemeral, node-local process — NOT a consensus run
//! and nothing here commits on-chain. It lives entirely in the daemon exactly
//! like the stream hub does: a member creates one over an authenticated local
//! RPC, then drives a `codex`/`claude` CLI's native TUI over the websocket
//! stream. The isolation lives one layer down in `capability_host`: the broker
//! holds the credential and the Podman backend fences the filesystem, so the
//! member typing into the container never reaches the operator's secrets.
//!
//! Three moving parts:
//! - [`TerminalSessions`] — the manager: a bounded map `sessionId ->
//!   Arc<InteractiveSession>`, a per-session output ring, and a pump task per
//!   session that copies pty output into the ring and broadcasts it on the ws
//!   `term:<id>` topic (modelled on the `run-output:<id>` plane).
//! - [`TermRing`] — the per-session scrollback ring, a focused twin of
//!   [`crate::stream::RunOutputRegistry`]: bounded bytes, monotonic seq,
//!   catch-up on (re)subscribe, LRU across sessions. Owned by the
//!   [`crate::stream::StreamHub`] so the ws catch-up path reaches it the same
//!   way it reaches the run-output ring.
//! - the HTTP routes ([`create_session`]/[`close_session`]) + the ws
//!   `TermInput`/`TermResize` handlers (in `stream.rs`).
//!
//! **Podman only.** Interactive spawn refuses the `Direct` backend
//! (`capability_host::CliProvider::spawn_interactive_session`), so this plane is
//! available only when the operator configured a Podman sandbox image
//! (`DUCKTAPE_SANDBOX_IMAGE`). With no image, [`create_session`] returns a clear
//! error and NEVER falls back to a Direct spawn.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use capability_host::{AgentDirs, InteractiveSession, ProviderSet, RunContext, SandboxBackend};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot, watch};

use crate::NodeHandle;

/// the per-node concurrent-session cap. a terminal is arbitrary code execution
/// on the operator's node burning the operator's subscription, so the ceiling
/// is deliberately small; over it, create refuses rather than spawning.
pub const MAX_TERM_SESSIONS: usize = 4;
/// the hard wall-clock ceiling on any single session. A session is a human
/// driving a CLI TUI, so 4h is a generous single working session; past it the
/// session is force-closed no matter what. This is the backstop that makes a
/// session non-immortal: the primary teardown is still explicit close on
/// unmount, but if the app tab is killed (its `close` never runs) or an idle TUI
/// is just left open, this timer guarantees the container + its slot are
/// reclaimed instead of pinned forever. There is deliberately NO idle-timeout —
/// a terminal is legitimately idle while a human reads (see the design's
/// "no idle-timeout kill"); silence is not death, the wall clock is the bound.
const MAX_SESSION_LIFETIME: Duration = Duration::from_secs(4 * 60 * 60);
/// how many bytes of (base64) scrollback each session keeps for catch-up.
/// `ponytail:` fixed per-session cap; make it a config knob only if a real TUI
/// redraw pattern proves it too small.
const TERM_RING_MAX_BYTES: usize = 256 * 1024;
/// how many session rings the shared ring retains before evicting the
/// least-recently-touched — closed sessions age out here. Shared by the output
/// ring and the command-log ring.
const TERM_RING_MAX_SESSIONS: usize = 16;
/// how many commands each session's ordered command log retains for catch-up.
/// A command is a submitted line (a prompt), so the count — not a byte cap — is
/// the natural bound; past it the oldest ages out and a resuming reader lags.
/// `ponytail:` fixed per-session cap; make it a config knob only if a real
/// long-running session proves it too small.
const TERM_CMD_RING_MAX_COMMANDS: usize = 1024;
/// one pty read chunk. human typing + TUI redraws are modest; a chunk this size
/// coalesces a redraw burst into few frames without a large per-session buffer.
const TERM_READ_BUF: usize = 32 * 1024;
/// the environment knob carrying the sandbox image interactive sessions run in
/// (a container image for Podman, a VM image for Tart). mirrors `bin/node`'s
/// `sandbox_image` (node.toml) but as a plain env var, since the daemon parses
/// no toml — same `DUCKTAPE_*` precedent as `DUCKTAPE_AGENT_WORKSPACES` /
/// `DUCKTAPE_PROVIDER_TIMEOUT_SECS`.
pub const SANDBOX_IMAGE_ENV: &str = "DUCKTAPE_SANDBOX_IMAGE";
/// which sandbox backend hosts interactive sessions: `"podman"` (default, Linux)
/// or `"tart"` (macOS guest VM). mirrors `bin/node`'s `sandbox` selector.
pub const SANDBOX_BACKEND_ENV: &str = "DUCKTAPE_SANDBOX_BACKEND";

/// the ws topic a session's output rides.
pub fn topic(session_id: &str) -> String {
    format!("term:{session_id}")
}

/// the ws topic a session's ordered, attributed command log rides — the
/// shared-conversation-object view, distinct from the raw-output `term:<id>`.
pub fn command_topic(session_id: &str) -> String {
    format!("term-cmd:{session_id}")
}

// ---------------------------------------------------------------------------
// the per-session output ring (a focused twin of RunOutputRegistry)
// ---------------------------------------------------------------------------

/// per-session terminal scrollback with catch-up on (re)subscribe. Bounded by
/// bytes per session and by session count overall (LRU); a monotonic `seq` per
/// session is the resume cursor, exactly like the run-output ring. Cheap and
/// always present on the [`crate::stream::StreamHub`]; the manager appends to
/// it, the ws catch-up path reads from it.
#[derive(Clone)]
pub struct TermRing {
    inner: Arc<Mutex<TermRingInner>>,
    watch: watch::Sender<u64>,
}

#[derive(Default)]
struct TermRingInner {
    version: u64,
    touch: u64,
    sessions: BTreeMap<String, SessionRing>,
}

#[derive(Default)]
struct SessionRing {
    next_seq: u64,
    floor_seq: u64,
    bytes: usize,
    touched: u64,
    chunks: VecDeque<(u64, String)>,
}

impl Default for TermRing {
    fn default() -> Self {
        let (watch, _) = watch::channel(0);
        Self {
            inner: Arc::new(Mutex::new(TermRingInner::default())),
            watch,
        }
    }
}

impl TermRing {
    /// append one base64 output chunk to a session's ring and wake subscribers.
    pub fn append(&self, session: &str, chunk_b64: String) {
        let mut inner = self.inner.lock().expect("term ring lock poisoned");
        inner.version += 1;
        inner.touch += 1;
        let version = inner.version;
        let touch = inner.touch;
        let ring = inner.sessions.entry(session.to_string()).or_default();
        ring.touched = touch;
        ring.next_seq += 1;
        let seq = ring.next_seq;
        ring.bytes += chunk_b64.len();
        ring.chunks.push_back((seq, chunk_b64));
        // evict oldest chunks until under the byte cap, always keeping the last.
        while ring.bytes > TERM_RING_MAX_BYTES && ring.chunks.len() > 1 {
            if let Some((evicted, chunk)) = ring.chunks.pop_front() {
                ring.bytes -= chunk.len();
                ring.floor_seq = evicted;
            }
        }
        // evict least-recently-touched WHOLE sessions (closed ones age out).
        while inner.sessions.len() > TERM_RING_MAX_SESSIONS {
            let Some(victim) = inner
                .sessions
                .iter()
                .filter(|(id, _)| *id != session)
                .min_by_key(|(_, ring)| ring.touched)
                .map(|(id, _)| id.clone())
            else {
                break;
            };
            inner.sessions.remove(&victim);
        }
        drop(inner);
        let _ = self.watch.send(version);
    }

    /// chunks with `seq > after`, up to `budget`, plus the ring's floor seq (so
    /// a reader that fell behind an eviction learns it lagged).
    pub fn read_after(&self, session: &str, after: u64, budget: usize) -> (Vec<(u64, String)>, u64) {
        let mut inner = self.inner.lock().expect("term ring lock poisoned");
        inner.touch += 1;
        let touch = inner.touch;
        let Some(ring) = inner.sessions.get_mut(session) else {
            return (Vec::new(), 0);
        };
        ring.touched = touch;
        let rows = ring
            .chunks
            .iter()
            .filter(|(seq, _)| *seq > after)
            .take(budget)
            .cloned()
            .collect();
        (rows, ring.floor_seq)
    }

    /// wake on any append (the version counter), like the run-output watch.
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.watch.subscribe()
    }
}

// ---------------------------------------------------------------------------
// the per-session ordered command log ring (a focused twin of TermRing)
// ---------------------------------------------------------------------------

/// the per-session ordered, origin-attributed command log with catch-up on
/// (re)subscribe — the shared-conversation-object seed. A focused twin of
/// [`TermRing`]: the same monotonic-`seq` cursor, per-session count bound,
/// LRU-across-sessions eviction, and `watch` wakeups; it stores `(seq, origin,
/// text)` command grains instead of base64 output chunks. Owned by the
/// [`crate::stream::StreamHub`] so the ws `term-cmd:<session>` catch-up reads
/// the same ring the session's serial command consumer appends to.
#[derive(Clone)]
pub struct TermCommandRing {
    inner: Arc<Mutex<TermCommandRingInner>>,
    watch: watch::Sender<u64>,
}

#[derive(Default)]
struct TermCommandRingInner {
    version: u64,
    touch: u64,
    sessions: BTreeMap<String, CommandRing>,
}

#[derive(Default)]
struct CommandRing {
    next_seq: u64,
    floor_seq: u64,
    touched: u64,
    commands: VecDeque<(u64, String, String)>,
}

impl Default for TermCommandRing {
    fn default() -> Self {
        let (watch, _) = watch::channel(0);
        Self {
            inner: Arc::new(Mutex::new(TermCommandRingInner::default())),
            watch,
        }
    }
}

impl TermCommandRing {
    /// append one accepted command to a session's ordered log and wake
    /// subscribers. Returns the assigned monotonic `seq` — the total order the
    /// serial consumer stamps each command with (starting at 1). The single
    /// per-session consumer is the only appender, so this ring's `next_seq` IS
    /// that order.
    pub fn append(&self, session: &str, origin: &str, text: &str) -> u64 {
        let mut inner = self.inner.lock().expect("term command ring lock poisoned");
        inner.version += 1;
        inner.touch += 1;
        let version = inner.version;
        let touch = inner.touch;
        let ring = inner.sessions.entry(session.to_string()).or_default();
        ring.touched = touch;
        ring.next_seq += 1;
        let seq = ring.next_seq;
        ring.commands
            .push_back((seq, origin.to_string(), text.to_string()));
        // evict oldest commands until under the count cap, always keeping the last.
        while ring.commands.len() > TERM_CMD_RING_MAX_COMMANDS && ring.commands.len() > 1 {
            if let Some((evicted, _, _)) = ring.commands.pop_front() {
                ring.floor_seq = evicted;
            }
        }
        // evict least-recently-touched WHOLE sessions (closed ones age out).
        while inner.sessions.len() > TERM_RING_MAX_SESSIONS {
            let Some(victim) = inner
                .sessions
                .iter()
                .filter(|(id, _)| *id != session)
                .min_by_key(|(_, ring)| ring.touched)
                .map(|(id, _)| id.clone())
            else {
                break;
            };
            inner.sessions.remove(&victim);
        }
        drop(inner);
        let _ = self.watch.send(version);
        seq
    }

    /// commands with `seq > after`, up to `budget`, plus the ring's floor seq
    /// (so a reader that fell behind an eviction learns it lagged). Empty for an
    /// unknown/evicted session, never a panic — exactly like [`TermRing`].
    pub fn read_after(
        &self,
        session: &str,
        after: u64,
        budget: usize,
    ) -> (Vec<(u64, String, String)>, u64) {
        let mut inner = self.inner.lock().expect("term command ring lock poisoned");
        inner.touch += 1;
        let touch = inner.touch;
        let Some(ring) = inner.sessions.get_mut(session) else {
            return (Vec::new(), 0);
        };
        ring.touched = touch;
        let rows = ring
            .commands
            .iter()
            .filter(|(seq, _, _)| *seq > after)
            .take(budget)
            .cloned()
            .collect();
        (rows, ring.floor_seq)
    }

    /// wake on any append (the version counter), like the output ring's watch.
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.watch.subscribe()
    }
}

// ---------------------------------------------------------------------------
// the session manager
// ---------------------------------------------------------------------------

/// the node-local terminal-session manager. Arc-backed so a clone can ride into
/// each session's pump task; injected onto the [`NodeHandle`] as an `Option`
/// (absent on the test/embedded handle → the routes 503).
#[derive(Clone)]
pub struct TerminalSessions(Arc<Inner>);

/// one ordered command bound for a session's pty: the "command grain" — a
/// submitted line (a prompt), attributed to `origin`. The ws (`TermCommand`)
/// enqueues these today; consensus (PR 2) will, with `origin` becoming the
/// signed member. The per-session serial consumer drains them FIFO.
pub struct Command {
    pub origin: String,
    pub text: String,
}

/// a live session plus the ordered-command lane feeding it and the drop-guard
/// that cancels its wall-clock reaper. When the entry leaves the map (`finish`),
/// dropping `_reaper_cancel` resolves the reaper's cancel receiver, so its timer
/// exits WITHOUT firing — an early end (pump EOF or explicit close) can never
/// leave a stale timer around to reap a later session that reused this id.
/// Dropping the entry also drops `cmd_tx`, the lane's only long-lived sender, so
/// the serial consumer's `recv()` returns `None` and it exits — the same
/// drop-driven teardown the pump takes on EOF, no separate cancel needed.
/// how a session is driven — chosen at create, enforced for its whole life.
///
/// `Single` (the default): ONE member, RAW keystrokes straight to the pty — the
/// solo terminal. `Shared`: ordered, attributed `TermCommand`s through the lane
/// (the consensus-ready path). The two are MUTUALLY EXCLUSIVE per session: a
/// shared session refuses raw input (else a keystroke would bypass the total
/// order), and a single session refuses commands (it has no lane/consumer).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionMode {
    #[default]
    Single,
    Shared,
}

struct Live {
    session: Arc<InteractiveSession>,
    mode: SessionMode,
    cmd_tx: mpsc::UnboundedSender<Command>,
    _reaper_cancel: oneshot::Sender<()>,
}

struct Inner {
    /// the Podman-backed provider set. `None` when no sandbox image is
    /// configured — create then refuses with a clear error, never a Direct
    /// spawn (which the interactive path rejects anyway).
    providers: Option<ProviderSet>,
    /// this node's canonical execution id — Podman lifecycle cleanup scopes
    /// container reaping to it, so a shared rootless user can't cross nodes.
    executing_node: String,
    /// per-session workdirs are created under here (the provider mounts one
    /// rw into the container; the fresh mount namespace fences the rest off).
    workdir_root: PathBuf,
    /// the shared scrollback ring (owned by the StreamHub, cloned in here so
    /// the pump can append to the same ring the ws catch-up reads).
    ring: TermRing,
    /// the shared ordered command-log ring (owned by the StreamHub, cloned in
    /// here so each session's serial consumer appends to the same ring the ws
    /// `term-cmd:<session>` catch-up reads).
    cmd_ring: TermCommandRing,
    /// live sessions. `std::sync::Mutex`: every critical section clones an
    /// `Arc` out and drops the guard before any `.await`, so it never crosses
    /// an await point.
    sessions: Mutex<HashMap<String, Live>>,
    /// reserved-or-live session count, the atomic backing the concurrency cap.
    /// reserved at create (before the spawn await), released exactly once when
    /// the session leaves the map (close or pump EOF).
    active: AtomicUsize,
}

/// the create-session reply — the fixed wire shape the app client consumes.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedSession {
    pub session_id: String,
    pub topic: String,
}

/// why a create refused. Each maps to a distinct status.
pub enum TermError {
    /// no Podman sandbox image is configured — interactive is unavailable.
    NoSandbox,
    /// the per-node concurrent-session cap is reached.
    AtCapacity,
    /// no provider serves the requested agent tag.
    Resolve(String),
    /// the interactive spawn itself failed (podman missing, image absent, …).
    Spawn(String),
}

impl TermError {
    fn response(self) -> Response {
        let (status, msg) = match self {
            TermError::NoSandbox => (
                StatusCode::SERVICE_UNAVAILABLE,
                "interactive sessions require a configured podman sandbox image".to_string(),
            ),
            TermError::AtCapacity => (
                StatusCode::TOO_MANY_REQUESTS,
                format!("terminal session cap ({MAX_TERM_SESSIONS}) reached"),
            ),
            TermError::Resolve(detail) => (StatusCode::BAD_REQUEST, detail),
            TermError::Spawn(detail) => (StatusCode::INTERNAL_SERVER_ERROR, detail),
        };
        (status, Json(serde_json::json!({ "error": msg }))).into_response()
    }
}

impl TerminalSessions {
    /// build a manager. `providers` is `None` when interactive is disabled (no
    /// sandbox image); `ring` is the StreamHub's shared [`TermRing`] and
    /// `cmd_ring` its shared [`TermCommandRing`].
    pub fn new(
        providers: Option<ProviderSet>,
        executing_node: String,
        workdir_root: PathBuf,
        ring: TermRing,
        cmd_ring: TermCommandRing,
    ) -> Self {
        Self(Arc::new(Inner {
            providers,
            executing_node,
            workdir_root,
            ring,
            cmd_ring,
            sessions: Mutex::new(HashMap::new()),
            active: AtomicUsize::new(0),
        }))
    }

    /// create a session for `agent`, spawning its interactive TUI on a pty.
    /// Reserves a slot against the cap BEFORE the spawn await so concurrent
    /// creates can't both slip past a stale count.
    pub async fn create(
        &self,
        agent: &str,
        mode: SessionMode,
    ) -> Result<CreatedSession, TermError> {
        let inner = &self.0;
        let Some(providers) = inner.providers.as_ref() else {
            tracing::warn!(target: "ducktape::term", reason = "no_sandbox", "session create refused");
            return Err(TermError::NoSandbox);
        };
        // reserve; release on any early return below.
        if inner.active.fetch_add(1, Ordering::SeqCst) + 1 > MAX_TERM_SESSIONS {
            inner.active.fetch_sub(1, Ordering::SeqCst);
            tracing::warn!(target: "ducktape::term", reason = "at_capacity", cap = MAX_TERM_SESSIONS, "session create refused");
            return Err(TermError::AtCapacity);
        }
        match self.spawn(providers, agent, mode).await {
            Ok(created) => Ok(created),
            Err(err) => {
                inner.active.fetch_sub(1, Ordering::SeqCst);
                Err(err)
            }
        }
    }

    /// resolve the provider, build the run context, spawn the pty session, and
    /// register it + its pump. The reservation is held by the caller.
    async fn spawn(
        &self,
        providers: &ProviderSet,
        agent: &str,
        mode: SessionMode,
    ) -> Result<CreatedSession, TermError> {
        let provider = providers.resolve(agent).map_err(|detail| {
            tracing::warn!(target: "ducktape::term", reason = "unknown_agent", agent, "session create refused");
            TermError::Resolve(detail)
        })?;
        let id = format!("{:016x}", rand::random::<u64>());
        let ctx = RunContext {
            agent_id: Some(agent.to_string()),
            // Podman requires the executing-node id for lifecycle scoping.
            executing_node: Some(self.0.executing_node.clone()),
            // a fresh per-session workdir; the provider create_dir_all's it and
            // mounts it rw into the container.
            workdir_override: Some(self.0.workdir_root.join(&id)),
            // a native pty session is a host-local optimization, never portable
            // state to resume/capture.
            portable: true,
            ..Default::default()
        };
        // a Shared session runs the restricted (read-only, non-prompting) argv;
        // a Single session runs the full solo TUI.
        let restricted = mode == SessionMode::Shared;
        let session = provider
            .spawn_interactive(&ctx, restricted)
            .await
            .map_err(|detail| {
                tracing::warn!(target: "ducktape::term", reason = "spawn_failed", agent, mode = ?mode, "interactive spawn failed");
                TermError::Spawn(detail)
            })?;
        let session = Arc::new(session);
        // dropping `cancel_tx` (when the entry leaves the map) cancels the
        // reaper; holding it in the map keeps the ceiling armed for the session.
        let (cancel_tx, cancel_rx) = oneshot::channel();
        // the ordered command lane. `cmd_tx` lives in the map entry (its only
        // long-lived sender), so `finish` dropping the entry ends the consumer.
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        self.0
            .sessions
            .lock()
            .expect("term sessions lock poisoned")
            .insert(
                id.clone(),
                Live {
                    session: session.clone(),
                    mode,
                    cmd_tx,
                    _reaper_cancel: cancel_tx,
                },
            );
        self.spawn_pump(id.clone(), session.clone());
        // the ordered command lane exists only for a Shared session; a Single
        // session drives the pty with raw keystrokes (no lane, no consumer).
        if mode == SessionMode::Shared {
            self.spawn_command_consumer(id.clone(), session, cmd_rx);
        }
        self.spawn_reaper(id.clone(), cancel_rx);
        tracing::info!(target: "ducktape::term", session = %id, agent, mode = ?mode, "session_created");
        Ok(CreatedSession {
            topic: topic(&id),
            session_id: id,
        })
    }

    /// the pump: copy pty output into the ring + broadcast until EOF, then
    /// clean the session up. One task per session.
    fn spawn_pump(&self, id: String, session: Arc<InteractiveSession>) {
        let manager = self.clone();
        let ring = self.0.ring.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; TERM_READ_BUF];
            loop {
                match session.read(&mut buf).await {
                    // EOF: the child (and its container) is gone.
                    Ok(0) => break,
                    Ok(n) => {
                        // never log the bytes — only their count.
                        ring.append(&id, STANDARD.encode(&buf[..n]));
                        tracing::trace!(target: "ducktape::term", session = %id, bytes = n, "term_output");
                    }
                    Err(err) => {
                        tracing::warn!(target: "ducktape::term", session = %id, reason = "read_failed", error = %err, "term pump stopped");
                        break;
                    }
                }
            }
            // whoever removes the entry owns teardown + the lifecycle log, so an
            // explicit close() racing the EOF here never double-terminates.
            if let Some(session) = manager.finish(&id) {
                session.close().await;
                tracing::info!(target: "ducktape::term", session = %id, "session_ended");
            }
        });
    }

    /// the serial command consumer: the total order. One task per session,
    /// spawned at create alongside the pump. It drains the session's ordered
    /// command lane FIFO, stamps each command with a monotonic per-session `seq`
    /// (via the shared command-log ring, starting at 1), records `(seq, origin,
    /// text)` to that ring — which wakes `term-cmd:<id>` subscribers — THEN
    /// feeds the command grain (`text` + `\r`, a submitted line) to the pty.
    /// Serial processing IS the total order; one host feeds the pty. Exits the
    /// moment every sender drops (the `Live` entry left the map via `finish`),
    /// the same drop-driven teardown the pump takes on EOF — so it can never
    /// outlive a reused id and never leaks.
    fn spawn_command_consumer(
        &self,
        id: String,
        session: Arc<InteractiveSession>,
        mut rx: mpsc::UnboundedReceiver<Command>,
    ) {
        let cmd_ring = self.0.cmd_ring.clone();
        tokio::spawn(async move {
            while let Some(Command { origin, text }) = rx.recv().await {
                // record + wake subscribers BEFORE the pty write: the ordered,
                // attributed log is the shared object; the pty write is its
                // effect. Never log the command text — it can carry secrets.
                let seq = cmd_ring.append(&id, &origin, &text);
                tracing::debug!(target: "ducktape::term", session = %id, seq, %origin, "term_command");
                if let Err(err) = session.write_all(text.as_bytes()).await {
                    tracing::warn!(target: "ducktape::term", session = %id, reason = "write_failed", error = %err, "term command dropped");
                    continue;
                }
                if let Err(err) = session.write_all(b"\r").await {
                    tracing::warn!(target: "ducktape::term", session = %id, reason = "write_failed", error = %err, "term command dropped");
                }
            }
        });
    }

    /// arm the hard wall-clock ceiling: after [`MAX_SESSION_LIFETIME`] the
    /// session is force-closed through `finish()` — the SAME teardown path pump
    /// EOF and explicit close take, so the slot still releases exactly once and
    /// this can't double-release or race those paths. Cancelled cleanly the
    /// moment the session ends earlier: `finish()` drops the entry (and with it
    /// `cancel`'s sender), the select below takes the cancel arm, and the timer
    /// never fires — so it can't reap a later session that reused this id.
    fn spawn_reaper(&self, id: String, cancel: oneshot::Receiver<()>) {
        let manager = self.clone();
        tokio::spawn(async move {
            if !reaper_fires(MAX_SESSION_LIFETIME, cancel).await {
                return; // the session ended before the ceiling — nothing to reap.
            }
            if let Some(session) = manager.finish(&id) {
                session.close().await;
                tracing::info!(target: "ducktape::term", session = %id, reason = "lifetime_ceiling", "session_ended");
            }
        });
    }

    /// close a session (idempotent): terminate the child + drop it from the
    /// manager. Unknown / already-closed id → a no-op.
    pub async fn close(&self, id: &str) {
        if let Some(session) = self.finish(id) {
            session.close().await;
            tracing::info!(target: "ducktape::term", session = %id, "session_ended");
        }
    }

    /// the `CommandSource` entry point: enqueue an ordered, origin-attributed
    /// command for `session`. The ws calls this now (from `TermCommand`);
    /// consensus (PR 2) will. The per-session serial consumer assigns the total
    /// order and feeds the pty — this only appends to the FIFO lane. An unknown
    /// session id is a no-op + `warn` (`unknown_session`), exactly like the
    /// input/resize handlers; never a panic. Never logs the command text.
    pub fn enqueue_command(&self, session: &str, origin: String, text: String) {
        // read the mode + sender under the lock, drop the guard before sending
        // (send is sync and non-blocking, but this keeps the manager's
        // no-work-under-lock discipline uniform).
        let found = self
            .0
            .sessions
            .lock()
            .expect("term sessions lock poisoned")
            .get(session)
            .map(|live| (live.mode, live.cmd_tx.clone()));
        let Some((mode, tx)) = found else {
            tracing::warn!(target: "ducktape::term", session = %session, reason = "unknown_session", "term command dropped");
            return;
        };
        // commands are the SHARED-session path; a Single session has no lane.
        if mode != SessionMode::Shared {
            tracing::warn!(target: "ducktape::term", session = %session, reason = "command_on_single", "term command dropped");
            return;
        }
        // a send failure means the consumer already exited (a teardown race with
        // finish()); the session is ending, so the drop is benign.
        let _ = tx.send(Command { origin, text });
    }

    /// the mode a session was created with (for the ws input gates), if live.
    pub fn mode(&self, id: &str) -> Option<SessionMode> {
        self.0
            .sessions
            .lock()
            .expect("term sessions lock poisoned")
            .get(id)
            .map(|live| live.mode)
    }

    /// the live session for `id`, if any (for `TermInput`/`TermResize`).
    pub fn session(&self, id: &str) -> Option<Arc<InteractiveSession>> {
        self.0
            .sessions
            .lock()
            .expect("term sessions lock poisoned")
            .get(id)
            .map(|live| live.session.clone())
    }

    /// remove a session from the map, releasing its cap slot exactly once.
    /// Returns the removed handle so the caller can tear it down. Dropping the
    /// removed [`Live`] also drops its reaper-cancel sender, so an early end
    /// disarms the wall-clock timer here (after the lock is released).
    fn finish(&self, id: &str) -> Option<Arc<InteractiveSession>> {
        let removed = self
            .0
            .sessions
            .lock()
            .expect("term sessions lock poisoned")
            .remove(id);
        if removed.is_some() {
            self.0.active.fetch_sub(1, Ordering::SeqCst);
        }
        removed.map(|live| live.session)
    }
}

/// resolve to `true` iff `lifetime` elapses before the session is cancelled
/// (its reaper-cancel sender dropped). Split out of `spawn_reaper` so the
/// ceiling-vs-cancel race is unit-testable under paused time without a live pty
/// session.
async fn reaper_fires(lifetime: Duration, cancel: oneshot::Receiver<()>) -> bool {
    tokio::select! {
        _ = tokio::time::sleep(lifetime) => true,
        _ = cancel => false,
    }
}

/// build the sandbox-backed interactive provider set for `backend`. `None` for
/// `Direct` (interactive requires Podman/Tart — a Direct node simply has no
/// terminal plane, no fallback); a discovery error is logged and also disables
/// it. The caller owns backend selection: `bin/node` passes its resolved
/// `node.toml` backend, `bin/noded` passes [`backend_from_env`].
pub fn discover_interactive(
    node_identity: &[u8],
    dirs: AgentDirs,
    backend: SandboxBackend,
) -> Option<ProviderSet> {
    if matches!(backend, SandboxBackend::Direct) {
        return None;
    }
    // force_private_net = TRUE: terminal containers host adversarial members, so
    // they must not share the host netns (see capability_host::discover).
    match capability_host::discover(node_identity, dirs, None, backend, true) {
        Ok(set) => Some(set),
        Err(err) => {
            tracing::error!(target: "ducktape::term", error = %err, "interactive_discovery_failed");
            None
        }
    }
}

/// derive the interactive sandbox backend from the daemon's env vars
/// (`DUCKTAPE_SANDBOX_IMAGE` / `DUCKTAPE_SANDBOX_BACKEND`). `Direct` (no
/// terminal plane) when no image is configured, or the backend name is unknown.
/// `bin/noded` uses this because it parses no toml; `bin/node` resolves its
/// backend from `node.toml` instead and passes it to [`discover_interactive`].
pub fn backend_from_env() -> SandboxBackend {
    let Ok(image) = std::env::var(SANDBOX_IMAGE_ENV) else {
        return SandboxBackend::Direct;
    };
    let image = image.trim().to_string();
    if image.is_empty() {
        return SandboxBackend::Direct;
    }
    match std::env::var(SANDBOX_BACKEND_ENV)
        .ok()
        .as_deref()
        .map(str::trim)
    {
        None | Some("") | Some("podman") => SandboxBackend::Podman { image },
        Some("tart") => SandboxBackend::Tart { image },
        Some(other) => {
            tracing::error!(target: "ducktape::term", backend = other, "unknown sandbox backend; interactive disabled");
            SandboxBackend::Direct
        }
    }
}

// ---------------------------------------------------------------------------
// HTTP routes
// ---------------------------------------------------------------------------
//
// AUTH: these two mutating routes are registered on the daemon's `public`
// router, so they inherit the SAME gate as `/v1/submit`, `/v1/fs/workspaces` and
// every other mutating `/v1/` RPC: `origin_guard::guard` + its CORS allowlist.
// That surface is trusted-local by design (see `origin_guard`) — a non-browser
// local client (the app, the CLI) sends no `Origin` and is allowed; a browser
// must present an allowlisted origin. There is no bearer token because a local
// process can already read the node's key off disk, so a token would guard no
// boundary this daemon can hold. Session creation is therefore gated exactly the
// way the run/dispatch path (`/v1/submit`) is, per the design (authorization
// rides the existing trusted-local surface, no new ACL).

/// the `POST /v1/term/sessions` body.
#[derive(Deserialize)]
pub struct CreateSessionBody {
    pub agent: String,
    /// `"single"` (default, back-compat) = raw-keystroke solo terminal;
    /// `"shared"` = ordered `TermCommand` lane.
    #[serde(default)]
    pub mode: SessionMode,
}

/// POST /v1/term/sessions — create an interactive session and return its id +
/// ws topic. Over the concurrency cap, missing sandbox, or an unknown agent
/// each return a clear error (never a panic, never a Direct spawn).
pub async fn create_session(
    State(handle): State<NodeHandle>,
    Json(body): Json<CreateSessionBody>,
) -> Response {
    let Some(terminals) = handle.terminals() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": "terminal sessions are not enabled on this node" })),
        )
            .into_response();
    };
    match terminals.create(&body.agent, body.mode).await {
        Ok(created) => (StatusCode::OK, Json(created)).into_response(),
        Err(err) => err.response(),
    }
}

/// POST /v1/term/sessions/{id}/close — end a session. Idempotent: a closed or
/// unknown id is a 204 no-op.
pub async fn close_session(State(handle): State<NodeHandle>, Path(id): Path<String>) -> Response {
    if let Some(terminals) = handle.terminals() {
        terminals.close(&id).await;
    }
    StatusCode::NO_CONTENT.into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_catch_up_replays_then_advances_and_lags_on_eviction() {
        let ring = TermRing::default();
        ring.append("s", STANDARD.encode(b"hello"));
        ring.append("s", STANDARD.encode(b"world"));
        // fresh subscriber (cursor 0) replays both chunks in order.
        let (rows, floor) = ring.read_after("s", 0, 64);
        assert_eq!(floor, 0);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0, 1);
        assert_eq!(STANDARD.decode(&rows[1].1).unwrap(), b"world");
        // a caught-up reader sees nothing new.
        let (none, _) = ring.read_after("s", rows[1].0, 64);
        assert!(none.is_empty());
        // an unknown session is empty, never a panic.
        assert!(ring.read_after("nope", 0, 64).0.is_empty());
    }

    #[test]
    fn ring_evicts_oldest_bytes_and_reports_floor() {
        let ring = TermRing::default();
        let chunk = "x".repeat(TERM_RING_MAX_BYTES); // one chunk already at the cap
        ring.append("s", chunk.clone());
        ring.append("s", chunk); // forces eviction of the first
        let (rows, floor) = ring.read_after("s", 0, 64);
        assert_eq!(rows.len(), 1, "the byte cap evicted the oldest chunk");
        assert_eq!(floor, 1, "the evicted chunk's seq is the reported floor");
        assert_eq!(rows[0].0, 2);
    }

    #[test]
    fn ring_evicts_least_recently_touched_sessions() {
        let ring = TermRing::default();
        for i in 0..=TERM_RING_MAX_SESSIONS {
            ring.append(&format!("s{i}"), STANDARD.encode(b"x"));
        }
        // the first-touched session aged out; the newest survives.
        assert!(ring.read_after("s0", 0, 8).0.is_empty());
        assert!(!ring
            .read_after(&format!("s{TERM_RING_MAX_SESSIONS}"), 0, 8)
            .0
            .is_empty());
    }

    // ----- the ordered command-log ring (a focused twin of TermRing) -----

    #[test]
    fn command_ring_assigns_monotonic_seq_and_catches_up() {
        let ring = TermCommandRing::default();
        // the consumer's seq is the ring's next_seq — monotonic, starting at 1.
        assert_eq!(ring.append("s", "alice", "hello"), 1);
        assert_eq!(ring.append("s", "bob", "world"), 2);
        // a fresh subscriber (cursor 0) replays both commands in order.
        let (rows, floor) = ring.read_after("s", 0, 64);
        assert_eq!(floor, 0);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], (1, "alice".to_string(), "hello".to_string()));
        assert_eq!(rows[1], (2, "bob".to_string(), "world".to_string()));
        // a caught-up reader sees nothing new.
        assert!(ring.read_after("s", 2, 64).0.is_empty());
        // an unknown session is empty, never a panic.
        assert!(ring.read_after("nope", 0, 64).0.is_empty());
    }

    #[test]
    fn command_ring_records_origin_and_text_in_order_then_lags_on_eviction() {
        let ring = TermCommandRing::default();
        // the ordered, attributed log preserves each (seq, origin, text) grain.
        for i in 0..TERM_CMD_RING_MAX_COMMANDS {
            assert_eq!(
                ring.append("s", &format!("m{i}"), &format!("cmd-{i}")),
                (i + 1) as u64
            );
        }
        ring.append("s", "over", "cmd-overflow"); // forces eviction of the first
        let (rows, floor) = ring.read_after("s", 0, TERM_CMD_RING_MAX_COMMANDS + 8);
        assert_eq!(floor, 1, "the evicted command's seq is the reported floor");
        assert_eq!(rows.len(), TERM_CMD_RING_MAX_COMMANDS);
        assert_eq!(
            rows[0],
            (2, "m1".to_string(), "cmd-1".to_string()),
            "the oldest survivor after eviction, attribution intact"
        );
        let last = rows.last().unwrap();
        assert_eq!(last.0, (TERM_CMD_RING_MAX_COMMANDS + 1) as u64);
        assert_eq!(last.1, "over");
        assert_eq!(last.2, "cmd-overflow");
    }

    #[test]
    fn command_ring_evicts_least_recently_touched_sessions() {
        let ring = TermCommandRing::default();
        for i in 0..=TERM_RING_MAX_SESSIONS {
            ring.append(&format!("s{i}"), "m", "x");
        }
        // the first-touched session aged out; the newest survives.
        assert!(ring.read_after("s0", 0, 8).0.is_empty());
        assert!(!ring
            .read_after(&format!("s{TERM_RING_MAX_SESSIONS}"), 0, 8)
            .0
            .is_empty());
    }

    #[test]
    fn enqueue_command_on_an_unknown_session_is_a_no_op() {
        // no sandbox providers → no live session can exist; enqueue must warn +
        // no-op, never panic (mirrors the input/resize unknown-session
        // discipline) and record nothing. The live enqueue→consumer→pty path
        // needs a real InteractiveSession (private ctor, real pty) and is
        // exercised by the parent's live podman check.
        let terminals = TerminalSessions::new(
            None,
            "node".into(),
            PathBuf::from("term-sessions"),
            TermRing::default(),
            TermCommandRing::default(),
        );
        terminals.enqueue_command("nope", "alice".into(), "ls".into());
        assert!(terminals.0.cmd_ring.read_after("nope", 0, 8).0.is_empty());
    }

    // The reaper drives `finish()` (the exactly-once slot release) once its
    // ceiling elapses; a live `InteractiveSession` can't be built in a unit test
    // (private ctor, real pty), so the two tests below pin the ceiling-vs-cancel
    // decision that gates that call. The reap-exactly-once itself is structural:
    // pump EOF, explicit close, and the reaper all funnel through the single
    // `remove().is_some()`-guarded `finish()`, covered live by fleet QA.

    #[tokio::test(start_paused = true)]
    async fn reaper_fires_once_past_the_lifetime_ceiling() {
        // the sender is held (session still live), so only the sleep can win.
        let (_cancel_tx, cancel_rx) = oneshot::channel();
        let reaper = tokio::spawn(reaper_fires(Duration::from_secs(10), cancel_rx));
        tokio::time::advance(Duration::from_secs(11)).await;
        assert!(
            reaper.await.unwrap(),
            "past the ceiling with no earlier end, the reaper fires"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn reaper_is_cancelled_when_the_session_ends_early() {
        let (cancel_tx, cancel_rx) = oneshot::channel();
        let reaper = tokio::spawn(reaper_fires(Duration::from_secs(10), cancel_rx));
        // the session ended (finish() dropped the entry, and with it the sender)
        // before the ceiling — the timer must NOT fire, so a reused id is safe.
        drop(cancel_tx);
        assert!(
            !reaper.await.unwrap(),
            "an early end cancels the timer; it never fires on a reused id"
        );
    }
}
