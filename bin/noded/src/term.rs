//! node-local, off-chain interactive terminal sessions.
//!
//! A terminal session is an ephemeral, node-local process — NOT a consensus run
//! and nothing here commits on-chain. It lives entirely in the daemon exactly
//! like the stream hub does: a member creates one over an authenticated local
//! RPC, then drives a `codex`/`claude` CLI's native TUI over the websocket
//! stream. The isolation lives one layer down in `provider_host`: the broker
//! holds the credential and the microVM fences the filesystem, so the member
//! typing into the guest never reaches the operator's secrets.
//!
//! Three moving parts:
//! - [`TerminalSessions`] — the node's half of the plane: per-session metadata,
//!   the ordered command lane, and the link to the agent daemon that owns the
//!   actual ptys.
//! - [`TermRing`] — the per-session scrollback ring, a focused twin of
//!   [`crate::stream::RunOutputRegistry`]: bounded bytes, monotonic seq,
//!   catch-up on (re)subscribe, LRU across sessions. Owned by the
//!   [`crate::stream::StreamHub`] so the ws catch-up path reaches it the same
//!   way it reaches the run-output ring.
//! - the HTTP routes ([`create_session`]/[`close_session`]) + the ws
//!   `TermInput`/`TermResize` handlers (in `stream.rs`).
//!
//! ## the process boundary
//!
//! **This node spawns no pty.** The interactive plane's execution half is
//! `agent-service`, running as a separate process (`ducktape service run
//! agent`) that dials this node's `/v1/ws` and drives ptys on the other side of
//! [`agent_service::wire`]. This file is everything ABOVE the pty: the rings the
//! ws serves, the mode/creator metadata the input gates read, the ordered
//! command lane, and the mesh-facing create/close entry points.
//!
//! The split is what it is because everything here is inherently the node's: a
//! pty client attaches to THIS node's ws, cross-node sessions ride the mesh term
//! plane (authenticated by mesh `PeerId`, admitted from committed state), and
//! neither is expressible in a process that holds no keypair.
//!
//! With no daemon attached there is no interactive plane: create refuses with
//! [`TermError::NoSandbox`] — the same 503 rung the unsandboxed node used to
//! return, kept distinct from the "terminal sessions are not enabled on this
//! node" 503 that means the plane was never wired at all.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_service::wire;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc, oneshot, watch};

use crate::NodeHandle;

/// how many commands may be in flight to the agent daemon before a sender
/// waits. Deep enough that a burst of keystrokes never blocks the ws reader;
/// bounded so a wedged daemon back-pressures instead of growing without limit.
const COMMAND_LANE: usize = 1024;
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
/// the per-ring peer-forwarder broadcast buffer (the feed `bin/node`'s
/// `term_plane` tails and fans out to peer nodes). A lagged subscriber — a slow
/// or stalled peer stream — drops the overflow and continues: terminal output
/// is observational, never consensus. Mirrors the run-output feed's buffer.
const TERM_APPEND_BUFFER: usize = 2048;

/// the ws topic a session's output rides.
pub fn topic(session_id: &str) -> String {
    format!("term:{session_id}")
}

/// the ws topic a session's ordered, attributed command log rides — the
/// shared-conversation-object view, distinct from the raw-output `term:<id>`.
pub fn command_topic(session_id: &str) -> String {
    format!("term-cmd:{session_id}")
}

/// one raw output chunk of a session, as it entered this node's local ring —
/// the wire grain `bin/node`'s `term_plane` forwards to peer nodes. A twin of
/// [`crate::stream::RunOutputEvent`]: a peer ingests it via
/// [`TermRing::append_remote`], which appends WITHOUT re-broadcasting (breaks
/// the fan-out loop). `chunk_b64` is base64 of the pty bytes, never logged. No
/// `seq`: the raw byte stream is opaque, so each node stamps its own local
/// ring cursor — the reliable per-peer stream preserves arrival order.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TermChunkEvent {
    pub session: String,
    pub chunk_b64: String,
}

/// one grain of a forwarded session's output feed — what `term_plane` writes on
/// its peer stream, and the only thing a peer node learns about a session it
/// does not host.
///
/// [`Self::Ended`] is why this is an enum rather than the bare chunk it used to
/// be. A peer-attached session's `agent pty` client subscribes on ITS OWN node
/// and blocks until that node's ring reports the session over; the host marking
/// its own ring ended told the guest nothing, so a cross-node pty stayed
/// attached to a dead session forever — the exact wedge the node-local
/// `TermEnded` frame exists to prevent, reachable again one hop away. Riding the
/// SAME ordered stream as the chunks is what keeps the end LAST: the terminal
/// grain can never overtake output the client has not seen.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "grain", rename_all = "snake_case")]
pub enum TermFeedEvent {
    Chunk(TermChunkEvent),
    Ended { session: String },
}

impl TermFeedEvent {
    /// the session every grain names — the id both the fan-out gate and the
    /// receiving ring key on.
    pub fn session(&self) -> &str {
        match self {
            Self::Chunk(chunk) => &chunk.session,
            Self::Ended { session } => session,
        }
    }
}

/// one entry of a session's ordered, attributed command log, as its serial
/// consumer stamped it — the wire grain `term_plane` forwards to peer nodes.
/// Unlike [`TermChunkEvent`] it carries `seq`: that is the AUTHORITATIVE total
/// order the origin node's single consumer assigned, and a peer replays it
/// verbatim via [`TermCommandRing::append_remote`] (which never re-stamps),
/// so every node shows the same order. `text` can carry secrets — never logged.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TermCommandEvent {
    pub session: String,
    pub seq: u64,
    pub origin: String,
    pub text: String,
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
    /// the peer-forwarder feed: a LOCAL append (the pump) publishes here so
    /// `bin/node`'s `term_plane` forwards it to peers; a `append_remote`
    /// (a peer's chunk arriving) does NOT, which breaks the fan-out loop. Twin
    /// of [`crate::stream::RunOutputRegistry`]'s `appends`. Carries the
    /// session's END as well as its bytes — see [`TermFeedEvent`].
    appends: broadcast::Sender<TermFeedEvent>,
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
    /// the child (and its container) has exited — the pump reached EOF. A
    /// `term:<id>` ws subscriber that has drained the ring learns the session
    /// is over from this and closes, rather than blocking forever on a topic
    /// that will never append again (the wedge that stranded `agent pty`).
    ended: bool,
}

impl Default for TermRing {
    fn default() -> Self {
        let (watch, _) = watch::channel(0);
        let (appends, _) = broadcast::channel(TERM_APPEND_BUFFER);
        Self {
            inner: Arc::new(Mutex::new(TermRingInner::default())),
            watch,
            appends,
        }
    }
}

impl TermRing {
    /// append one base64 output chunk from THIS node's pump: rings it, wakes the
    /// node-local ws subscribers, AND publishes it on the forwarder feed so
    /// `bin/node`'s `term_plane` fans it out to peer nodes.
    pub fn append(&self, session: &str, chunk_b64: String) {
        self.push(session, chunk_b64, true);
    }

    /// append one chunk received FROM a peer node without re-broadcasting it —
    /// the ring-only path that breaks the fan-out loop. Only bumps the ring's
    /// `watch` version, which wakes this node's `term:<session>` ws subscribers;
    /// the raw byte stream is opaque, so the local ring stamps its own cursor.
    pub fn append_remote(&self, event: TermChunkEvent) {
        self.push(&event.session, event.chunk_b64, false);
    }

    /// append a chunk from THIS node's pump WITHOUT forwarding it to peers — the
    /// SINGLE-session path. A single session is the solo, node-local terminal:
    /// its output rings + wakes local ws subscribers exactly like `append`, but
    /// is never fanned out, so a private terminal's bytes never leave the host.
    /// Only a Shared session (the huddle-style path) forwards.
    pub fn append_local_only(&self, session: &str, chunk_b64: String) {
        self.push(session, chunk_b64, false);
    }

    /// end a FORWARDED session: flag the ring, wake this node's subscribers, AND
    /// publish the terminal grain so `term_plane` carries it to the peers that
    /// have been streaming this session. The peer half of the wedge fix — a
    /// guest node's `agent pty` client learns the session is over from nothing
    /// else. Twin of [`Self::append`].
    pub fn mark_ended(&self, session: &str) {
        self.end(session, true);
    }

    /// end a session whose output never left this node — a solo local terminal,
    /// or a peer's session this node is only mirroring (its end arrived FROM the
    /// host, so re-publishing it would fan the grain back out). Twin of
    /// [`Self::append_local_only`] and [`Self::append_remote`].
    pub fn mark_ended_local_only(&self, session: &str) {
        self.end(session, false);
    }

    /// flag a session's ring as ended (the pump reached EOF) and wake its ws
    /// subscribers so the catch-up path can emit the terminal frame. CREATES the
    /// ring entry if absent: a session that dies before printing a single byte
    /// (a fast crash, or a kill before the child renders) has no ring entry yet,
    /// and its end MUST still reach the `agent pty` client waiting on the topic —
    /// otherwise that no-output session is exactly the one that wedges. Bumps
    /// `version` so `term:<id>` `watch` fires exactly as an append would.
    fn end(&self, session: &str, publish: bool) {
        let mut inner = self.inner.lock().expect("term ring lock poisoned");
        let touch = inner.touch + 1;
        let version = inner.version + 1;
        {
            let ring = inner.sessions.entry(session.to_string()).or_default();
            if ring.ended {
                return; // already ended — the double-close race is a no-op
            }
            ring.ended = true;
            ring.touched = touch; // not the instant LRU victim before the signal lands
        }
        inner.touch = touch;
        inner.version = version;
        drop(inner);
        let _ = self.watch.send(version);
        // AFTER the local flag and the watch: a peer learning of the end before
        // this node's own subscribers would invert the order for no reason.
        if publish {
            let _ = self.appends.send(TermFeedEvent::Ended {
                session: session.to_string(),
            });
        }
    }

    /// whether the session's pump has reached EOF. `false` for an unknown or
    /// evicted session — a caller with no ring to drain has nothing to close on.
    pub fn is_ended(&self, session: &str) -> bool {
        let inner = self.inner.lock().expect("term ring lock poisoned");
        inner.sessions.get(session).is_some_and(|ring| ring.ended)
    }

    fn push(&self, session: &str, chunk_b64: String, publish: bool) {
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
        // keep a copy for the forwarder feed only when we will publish.
        let forwarded = publish.then(|| chunk_b64.clone());
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
        if let Some(chunk_b64) = forwarded {
            let _ = self.appends.send(TermFeedEvent::Chunk(TermChunkEvent {
                session: session.to_string(),
                chunk_b64,
            }));
        }
    }

    /// subscribe the peer-forwarder feed: every LOCAL append (never a remote
    /// one) arrives here, so `bin/node`'s `term_plane` can forward it to peers.
    pub fn subscribe_appends(&self) -> broadcast::Receiver<TermFeedEvent> {
        self.appends.subscribe()
    }

    /// chunks with `seq > after`, up to `budget`, plus the ring's floor seq (so
    /// a reader that fell behind an eviction learns it lagged).
    pub fn read_after(
        &self,
        session: &str,
        after: u64,
        budget: usize,
    ) -> (Vec<(u64, String)>, u64) {
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
    /// the peer-forwarder feed — twin of [`TermRing`]'s. A LOCAL append (the
    /// serial consumer stamping a command) publishes here; a `append_remote`
    /// (a peer's command arriving) does NOT, breaking the fan-out loop.
    appends: broadcast::Sender<TermCommandEvent>,
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
        let (appends, _) = broadcast::channel(TERM_APPEND_BUFFER);
        Self {
            inner: Arc::new(Mutex::new(TermCommandRingInner::default())),
            watch,
            appends,
        }
    }
}

impl TermCommandRing {
    /// append one accepted command from THIS node's serial consumer: rings it,
    /// wakes the node-local ws subscribers, AND publishes it on the forwarder
    /// feed so `term_plane` fans it out. Returns the assigned monotonic `seq` —
    /// the total order (starting at 1). The single per-session consumer is the
    /// only local appender, so this ring's `next_seq` IS that order.
    pub fn append(&self, session: &str, origin: &str, text: &str) -> u64 {
        self.push(session, None, origin, text, true)
    }

    /// append one command received FROM a peer node without re-broadcasting it —
    /// the ring-only path that breaks the fan-out loop. Preserves the origin
    /// node's `seq` VERBATIM (never re-stamps): that node's serial consumer owns
    /// the authoritative total order, so a peer replaying it must show the same
    /// order. Off-consensus and observational, so this stays honest to the
    /// origin's numbering rather than inventing a local one.
    pub fn append_remote(&self, event: TermCommandEvent) {
        self.push(
            &event.session,
            Some(event.seq),
            &event.origin,
            &event.text,
            false,
        );
    }

    fn push(
        &self,
        session: &str,
        seq_override: Option<u64>,
        origin: &str,
        text: &str,
        publish: bool,
    ) -> u64 {
        let mut inner = self.inner.lock().expect("term command ring lock poisoned");
        inner.version += 1;
        inner.touch += 1;
        let version = inner.version;
        let touch = inner.touch;
        let ring = inner.sessions.entry(session.to_string()).or_default();
        ring.touched = touch;
        // a local append stamps the next serial seq; a remote one carries the
        // origin's seq verbatim (bump `next_seq` past it only to keep the local
        // cursor monotonic — a peer never appends locally, so it is bookkeeping).
        let seq = match seq_override {
            Some(seq) => {
                ring.next_seq = ring.next_seq.max(seq);
                seq
            }
            None => {
                ring.next_seq += 1;
                ring.next_seq
            }
        };
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
        if publish {
            let _ = self.appends.send(TermCommandEvent {
                session: session.to_string(),
                seq,
                origin: origin.to_string(),
                text: text.to_string(),
            });
        }
        seq
    }

    /// subscribe the peer-forwarder feed: every LOCAL append (never a remote
    /// one) arrives here for `term_plane` to forward to peers.
    pub fn subscribe_appends(&self) -> broadcast::Receiver<TermCommandEvent> {
        self.appends.subscribe()
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
// the session bridge — the node's half of the interactive plane
// ---------------------------------------------------------------------------

/// the node's half of the terminal plane: what a session IS to this node
/// (its mode, its creator, its ordered command lane) plus the link to the agent
/// daemon that owns the pty. Arc-backed so a clone rides into each session's
/// command consumer; injected onto the [`NodeHandle`] as an `Option` (absent on
/// a sync-only node → the routes 503 "not enabled").
///
/// Nothing here spawns a process. Every effect on a pty is a
/// [`wire::Command`] down the link, and every fact about one arrives as a
/// [`wire::Event`] through [`TerminalSessions::on_event`].
#[derive(Clone)]
pub struct TerminalSessions(Arc<Bridge>);

/// one ordered command bound for a session's pty: the "command grain" — a
/// submitted line (a prompt), attributed to `origin`. The ws (`TermCommand`)
/// enqueues these today; consensus (PR 2) will, with `origin` becoming the
/// signed member. The per-session serial consumer drains them FIFO.
pub struct Command {
    pub origin: String,
    pub text: String,
}

/// how a session is driven — chosen at create, enforced for its whole life.
///
/// `Single`: one member, raw keystrokes straight to the pty — the
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

/// what this node knows about a live session. The pty itself lives in the
/// daemon; none of these facts do, because none of them is the daemon's
/// business: `mode` and `creator_node` are admission decisions this node made,
/// and the command lane is the total order it owns.
struct Live {
    mode: SessionMode,
    /// the node that created this session, for the host-side input gate. `None`
    /// for a local (non peer-attached) session — a forwarded input frame naming
    /// a local session is refused, since no peer owns it.
    creator_node: Option<[u8; 32]>,
    /// whether output fans out to peer nodes (a Shared local session, or a
    /// peer-attached one). Decided at create and stored, so an output frame
    /// racing the create reply already knows where it goes.
    forward: bool,
    cmd_tx: mpsc::UnboundedSender<Command>,
    /// the pending create's answer, taken by whichever of `TermCreated` /
    /// `TermRefused` / a detach arrives first. `None` once the session is live.
    reply: Option<oneshot::Sender<Result<(), TermError>>>,
}

struct Bridge {
    /// the shared scrollback ring (owned by the StreamHub, cloned in here so
    /// daemon output lands in the same ring the ws catch-up reads).
    ring: TermRing,
    /// the shared ordered command-log ring (owned by the StreamHub, cloned in
    /// here so each session's serial consumer appends to the same ring the ws
    /// `term-cmd:<session>` catch-up reads).
    cmd_ring: TermCommandRing,
    /// live sessions. `std::sync::Mutex`: every critical section clones what it
    /// needs out and drops the guard before any `.await`, so it never crosses an
    /// await point.
    sessions: Mutex<HashMap<String, Live>>,
    /// the attached agent daemon's command lane. `None` — no daemon signaling —
    /// IS the `no_sandbox` state: this node has no interactive plane to offer.
    link: Mutex<Option<mpsc::Sender<wire::Command>>>,
    /// the secret a daemon must present to take the link. `None` — a node with
    /// no workspace to hold one — refuses every attach: holding the link means
    /// becoming this node's interactive plane, which is not a capability to hand
    /// out on the strength of dialing loopback.
    link_token: Option<String>,
}

/// everything the host needs to spawn a session on behalf of a mesh peer: the
/// creator node (the input gate), the guest's consensus-resolved credential
/// (which the daemon turns back into the broker's self-host airlock config), and
/// the cpu/mem limits.
pub struct PeerAttach {
    pub creator_node: [u8; 32],
    pub credential: wire::Credential,
    pub limits: BTreeMap<String, u64>,
}

/// the create-session reply — the fixed wire shape the app client consumes.
#[derive(Clone, Serialize)]
pub struct CreatedSession {
    pub session_id: String,
    pub topic: String,
}

/// why a create refused. Each maps to a distinct status.
#[derive(Debug)]
pub enum TermError {
    /// no agent service is attached — this node has no interactive plane.
    NoSandbox,
    /// the daemon's concurrent-session cap is reached.
    AtCapacity,
    /// no provider serves the requested agent tag.
    Resolve(String),
    /// the interactive spawn itself failed (guest artifacts absent, no /dev/kvm, …).
    Spawn(String),
}

impl TermError {
    /// the daemon's refusal, in this node's vocabulary. `NoSandbox` has no
    /// counterpart on purpose: a daemon with no sandbox never starts, so only
    /// this node can be in that state and it answers without asking.
    fn from_refusal(reason: wire::Refusal, detail: String) -> Self {
        match reason {
            wire::Refusal::AtCapacity => TermError::AtCapacity,
            wire::Refusal::UnknownProvider => TermError::Resolve(detail),
            wire::Refusal::SpawnFailed => TermError::Spawn(detail),
        }
    }

    fn response(self) -> Response {
        let (status, msg) = match self {
            TermError::NoSandbox => (
                StatusCode::SERVICE_UNAVAILABLE,
                "interactive sessions require an agent service on this node — \
                 run `ducktape service run agent`"
                    .to_string(),
            ),
            TermError::AtCapacity => (
                StatusCode::TOO_MANY_REQUESTS,
                format!("terminal session cap ({}) reached", agent_service::MAX_TERM_SESSIONS),
            ),
            TermError::Resolve(detail) => (StatusCode::BAD_REQUEST, detail),
            TermError::Spawn(detail) => (StatusCode::INTERNAL_SERVER_ERROR, detail),
        };
        (status, Json(serde_json::json!({ "error": msg }))).into_response()
    }
}

/// what a create needs decided before anything is sent. Built by
/// [`TerminalSessions::create`] (local) or [`TerminalSessions::create_for_peer`]
/// (mesh), executed by [`TerminalSessions::start`] — so the two entry points
/// stay pure decisions and exactly one place performs the effects.
/// what the operator asked this session's sandbox to be sized at — the CLI's
/// `--cpu`/`--mem`, straight off the create body.
///
/// A struct rather than two adjacent `Option<u64>` parameters: swapping cores
/// and gigabytes at a call site would build a plausible VM of the wrong shape,
/// and nothing downstream could tell.
#[derive(Debug, Clone, Copy, Default)]
pub struct SessionSize {
    pub cpu: Option<u64>,
    pub mem_gb: Option<u64>,
}

impl SessionSize {
    /// the limit keys the sandbox backend enforces. Absent dimensions are left
    /// out here and filled with the defaults at the single create seam, so one
    /// place decides what "unsized" means.
    fn limits(self) -> BTreeMap<String, u64> {
        let mut limits = BTreeMap::new();
        if let Some(cores) = self.cpu {
            limits.insert("cores".to_string(), cores);
        }
        if let Some(mem_gb) = self.mem_gb {
            limits.insert("mem_gb".to_string(), mem_gb);
        }
        limits
    }
}

/// the size a session's sandbox is built at when the operator named none.
///
/// Matches the delegated-child profile the runs module already uses
/// (`DELEGATED_CHILD_CORES` / `_MEM_GB`): enough for one interactive CLI, and
/// small enough that a node hosting several is not surprised.
const DEFAULT_SESSION_CORES: u64 = 2;
const DEFAULT_SESSION_MEM_GB: u64 = 4;

struct Spawn {
    provider: String,
    mode: SessionMode,
    creator_node: Option<[u8; 32]>,
    /// output fans out to peer nodes.
    forward: bool,
    /// run the restricted (read-only, non-prompting) argv rather than the solo TUI.
    restricted: bool,
    limits: BTreeMap<String, u64>,
    credential: Option<wire::Credential>,
}

/// holds a daemon's attachment open. Dropping it — the ws connection closed,
/// the daemon exited, the node is shutting down — detaches the link AND ends
/// every session it was serving.
///
/// Ending them is not tidiness. The pty lives in the other process; once the
/// link is gone this node can neither feed a session's topic nor close it, so a
/// client attached to `term:<id>` would block forever on a stream that will
/// never produce another byte. That is exactly the wedge the `term_ended` signal
/// exists to prevent, and a daemon dying is just another way for a session to
/// end.
pub struct AttachGuard(TerminalSessions);

impl Drop for AttachGuard {
    fn drop(&mut self) {
        self.0.detach();
    }
}

impl TerminalSessions {
    /// build the bridge over the StreamHub's shared [`TermRing`] and
    /// [`TermCommandRing`]. No daemon is attached yet — one arrives (or does
    /// not) over the ws.
    pub fn new(ring: TermRing, cmd_ring: TermCommandRing, link_token: Option<String>) -> Self {
        Self(Arc::new(Bridge {
            ring,
            cmd_ring,
            sessions: Mutex::new(HashMap::new()),
            link: Mutex::new(None),
            link_token,
        }))
    }

    // ---- the daemon's attachment ------------------------------------------

    /// Take the interactive plane for this connection.
    ///
    /// `None` when a daemon is already attached: one agent service per node, and
    /// FIRST ATTACH WINS. That is a boundary, not a nicety — a second attacher
    /// could otherwise displace the live daemon and receive the create commands
    /// (including lent-credential records) meant for it.
    pub fn attach(&self, token: &str) -> Option<(AttachGuard, mpsc::Receiver<wire::Command>)> {
        // two reasons, not one: "this node minted no secret" is an operator's
        // node to fix and "you presented the wrong one" is the daemon's, and
        // collapsing them sends whoever reads the log to the wrong machine.
        if self.0.link_token.is_none() {
            tracing::warn!(target: "ducktape::service", reason = "no_link_token", "agent service link refused");
            return None;
        }
        if !self.link_token_matches(token) {
            tracing::warn!(target: "ducktape::service", reason = "bad_link_token", "agent service link refused");
            return None;
        }
        let mut link = self.0.link.lock().expect("term link lock poisoned");
        if link.is_some() {
            return None;
        }
        let (tx, rx) = mpsc::channel(COMMAND_LANE);
        *link = Some(tx);
        tracing::info!(target: "ducktape::term", "agent service attached");
        Some((AttachGuard(self.clone()), rx))
    }

    /// Does `presented` match this node's 0600 workspace link secret?
    ///
    /// The node's ONE proof that a caller can read its own workspace, so it
    /// answers two questions: may you take the interactive plane ([`Self::attach`]),
    /// and may you hold a workspace-gated ws topic
    /// (`crate::stream::Admission::Workspace`, reached through
    /// [`crate::NodeHandle::workspace_secret_matches`]). Holding the secret
    /// already grants the first, which strictly contains the second, so serving
    /// both from one file adds no authority — a second secret would only be a
    /// second thing to leak.
    ///
    /// `None` — a node with no workspace to hold one — matches NOTHING, which
    /// fails closed.
    ///
    /// A pure predicate: it logs nothing, because its two callers must not log
    /// alike. An attach is a once-per-daemon-session event worth a `warn`; a
    /// subscribe is per-request and locally drivable in a loop, so a `warn`
    /// there would evict the 4096-line ring. Each caller names its own level.
    pub(crate) fn link_token_matches(&self, presented: &str) -> bool {
        self.0
            .link_token
            .as_deref()
            .is_some_and(|expected| crate::services::token_matches(presented, expected))
    }

    /// drop the link and end every session it was serving. See [`AttachGuard`].
    fn detach(&self) {
        *self.0.link.lock().expect("term link lock poisoned") = None;
        let live = std::mem::take(
            &mut *self
                .0
                .sessions
                .lock()
                .expect("term sessions lock poisoned"),
        );
        if !live.is_empty() {
            tracing::warn!(
                target: "ducktape::term",
                sessions = live.len(),
                reason = "agent_service_gone",
                "ending every session: the agent service detached"
            );
        }
        for (id, session) in live {
            // the terminator every `term:<id>` subscriber is blocked on. The
            // ring creates the entry if the session never produced a byte, so a
            // session that died before its first output still unblocks. A
            // peer-attached session's subscriber is on ANOTHER node, so the end
            // takes the same route its output did — `session.forward` is the
            // same stored discriminant the output path branches on.
            if session.forward {
                self.0.ring.mark_ended(&id);
            } else {
                self.0.ring.mark_ended_local_only(&id);
            }
            answer(session.reply, TermError::NoSandbox);
        }
        tracing::info!(target: "ducktape::term", "agent service detached");
    }

    /// whether an agent service is attached — the host-side admission's
    /// `no_sandbox` gate, and what `has capacity` means to a mesh peer.
    pub fn has_sandbox(&self) -> bool {
        self.link().is_some()
    }

    fn link(&self) -> Option<mpsc::Sender<wire::Command>> {
        self.0.link.lock().expect("term link lock poisoned").clone()
    }

    // ---- events from the daemon -------------------------------------------

    /// THE dispatch for everything the daemon reports. One arm per variant, each
    /// a single delegation — a new event fails the build until it is routed.
    pub fn on_event(&self, event: wire::Event) {
        match event {
            wire::Event::TermCreated { session } => self.created(&session),
            wire::Event::TermRefused {
                session,
                reason,
                detail,
            } => self.refused(&session, reason, detail),
            wire::Event::TermOutput {
                session,
                chunk_b64,
            } => self.output(&session, chunk_b64),
            wire::Event::TermEnded { session } => self.ended(&session),
        }
    }

    /// the pty is live: release the create that is waiting on it.
    ///
    /// If nobody is still waiting, END IT. A dropped receiver means the caller
    /// gave up — axum drops the handler future when the client disconnects, and
    /// a Ctrl-C during a cold image pull is ordinary behaviour, not an edge
    /// case. The id was never returned to anyone, so no close can ever arrive
    /// for this session: left alone it would burn a container running the agent
    /// CLI on the operator's credential, plus one of the daemon's few cap slots,
    /// until the wall-clock ceiling fires hours later.
    fn created(&self, id: &str) {
        let Some(reply) = self.take_reply(id) else {
            return;
        };
        if reply.send(Ok(())).is_ok() {
            return;
        }
        tracing::warn!(
            target: "ducktape::term",
            session = %id,
            reason = "create_abandoned",
            "ending a session whose caller went away"
        );
        // `close` only hands a frame to the link, but that is an await, and this
        // runs on the ws read loop. The teardown proper happens when the
        // daemon's `TermEnded` comes back through `ended`.
        let bridge = self.clone();
        let id = id.to_string();
        tokio::spawn(async move { bridge.close(&id).await });
    }

    /// the create failed: release it with the daemon's reason, and forget the
    /// session — there is no pty to end, so no `TermEnded` will follow.
    fn refused(&self, id: &str, reason: wire::Refusal, detail: String) {
        let removed = self
            .0
            .sessions
            .lock()
            .expect("term sessions lock poisoned")
            .remove(id);
        let Some(live) = removed else {
            return;
        };
        answer(live.reply, TermError::from_refusal(reason, detail));
    }

    /// one chunk of pty output, into the ring the ws catch-up reads. A session
    /// that forwards also publishes to the peer-forwarder feed; a solo one stays
    /// node-local. An unknown id is a stale frame from a session this node
    /// already ended — dropped without a line, since the daemon's own teardown
    /// already logged the end.
    fn output(&self, id: &str, chunk_b64: String) {
        let Some(forward) = self.with_session(id, |live| live.forward) else {
            return;
        };
        if forward {
            self.0.ring.append(id, chunk_b64);
        } else {
            self.0.ring.append_local_only(id, chunk_b64);
        }
    }

    /// the session is over. Mark the ring ended FIRST — unconditionally, before
    /// the entry is removed — so the terminal frame reaches every `term:<id>`
    /// subscriber even for a session that never produced a byte (`mark_ended`
    /// creates the ring entry). An `agent pty` client blocks on this topic and
    /// only unblocks when it sees the session is over.
    ///
    /// Read `forward` BEFORE the removal below, for the same reason [`Self::output`]
    /// reads it: a peer-attached session's client waits on ITS OWN node, so the
    /// end has to be published on the peer feed, and the record that says so is
    /// about to be dropped. An unknown session (already removed) is treated as
    /// forwarded: a redundant terminal grain on the peer stream is idempotent
    /// (`mark_ended_local_only` no-ops on an ended ring), while a missed one
    /// strands a client forever.
    fn ended(&self, id: &str) {
        let node_local = self.with_session(id, |live| !live.forward).unwrap_or(false);
        if node_local {
            self.0.ring.mark_ended_local_only(id);
        } else {
            self.0.ring.mark_ended(id);
        }
        let removed = self
            .0
            .sessions
            .lock()
            .expect("term sessions lock poisoned")
            .remove(id);
        // a session that ends before its create was answered (the child exited
        // instantly) still owes that create a reply.
        if let Some(live) = removed {
            answer(live.reply, TermError::Spawn("the session ended immediately".into()));
        }
    }

    // ---- creates ----------------------------------------------------------

    /// create a session for `agent` on this node's agent service, at `size`.
    pub async fn create(
        &self,
        agent: &str,
        mode: SessionMode,
        size: SessionSize,
    ) -> Result<CreatedSession, TermError> {
        // a Shared session runs the restricted argv and fans out to peers; a
        // Single session is the solo TUI and stays node-local.
        let shared = mode == SessionMode::Shared;
        self.start(Spawn {
            provider: agent.to_string(),
            mode,
            creator_node: None,
            forward: shared,
            restricted: shared,
            limits: size.limits(),
            credential: None,
        })
        .await
    }

    /// create a session on behalf of a mesh PEER (the host side of a directed
    /// `ducktape agent pty --node <this>`): the FULL solo TUI (raw keystrokes)
    /// but with output FORWARDING on — so `term_plane` fans the pty out to the
    /// guest node — and the creator node recorded for the host-side input gate.
    /// The credential rides to the daemon, which pins it as the broker's
    /// self-host airlock upstream; `attach.limits` becomes the container's
    /// cpu/mem ceilings.
    pub async fn create_for_peer(
        &self,
        provider: &str,
        attach: PeerAttach,
    ) -> Result<CreatedSession, TermError> {
        self.start(Spawn {
            provider: provider.to_string(),
            // a peer-attached session is driven by raw keystrokes, never the
            // ordered command lane.
            mode: SessionMode::Single,
            creator_node: Some(attach.creator_node),
            // ALWAYS forwards — that is how the guest node streams it (the
            // security-critical INPUT direction is creator-gated host-side).
            forward: true,
            restricted: false,
            limits: attach.limits,
            credential: Some(attach.credential),
        })
        .await
    }

    /// the one place a create is performed: mint the id, record what this node
    /// knows, send the command, and wait for the daemon's answer.
    ///
    /// The metadata goes in BEFORE the command goes out, so an output frame that
    /// races the reply always finds its session. There is deliberately no
    /// timeout: a cold image pull legitimately takes minutes, and the failure
    /// mode a timeout would cover — the daemon vanishing — is already covered by
    /// [`AttachGuard`], which answers every pending create.
    async fn start(&self, mut spawn: Spawn) -> Result<CreatedSession, TermError> {
        let Some(link) = self.link() else {
            tracing::warn!(target: "ducktape::term", reason = "no_agent_service", "session create refused");
            return Err(TermError::NoSandbox);
        };
        // A sandbox is BUILT at a size. Under the container backend an absent
        // dimension meant "unlimited"; a microVM has no such state, and the
        // provider refuses the run outright ("a microVM run needs an explicit
        // `cores` limit"). So the ONE place a create is performed fills in
        // whatever the operator did not name — for the local path and the peer
        // path alike, because a `--host-node` create with no `--cpu` reaches
        // here just as empty.
        spawn
            .limits
            .entry("cores".to_string())
            .or_insert(DEFAULT_SESSION_CORES);
        spawn
            .limits
            .entry("mem_gb".to_string())
            .or_insert(DEFAULT_SESSION_MEM_GB);
        let id = format!("{:016x}", rand::random::<u64>());
        let (reply_tx, reply_rx) = oneshot::channel();
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        self.0
            .sessions
            .lock()
            .expect("term sessions lock poisoned")
            .insert(
                id.clone(),
                Live {
                    mode: spawn.mode,
                    creator_node: spawn.creator_node,
                    forward: spawn.forward,
                    cmd_tx,
                    reply: Some(reply_tx),
                },
            );
        let command = wire::Command::TermCreate(wire::Create {
            session: id.clone(),
            provider: spawn.provider,
            restricted: spawn.restricted,
            limits: spawn.limits,
            credential: spawn.credential,
        });
        if link.send(command).await.is_err() {
            // the daemon detached between the link clone and the send; its guard
            // has already cleaned the entry up.
            tracing::warn!(target: "ducktape::term", reason = "no_agent_service", "session create refused");
            return Err(TermError::NoSandbox);
        }
        // `Err` = the sender was dropped without answering, which only the
        // detach path does — and it logs its own reason.
        match reply_rx.await {
            Ok(Ok(())) => {}
            Ok(Err(refusal)) => return Err(refusal),
            Err(_) => return Err(TermError::NoSandbox),
        }
        // the ordered command lane exists only for a Shared session; a Single
        // session drives the pty with raw keystrokes (no lane, no consumer).
        if spawn.mode == SessionMode::Shared {
            self.spawn_command_consumer(id.clone(), cmd_rx);
        }
        tracing::info!(target: "ducktape::term", session = %id, mode = ?spawn.mode, "session_created");
        Ok(CreatedSession {
            topic: topic(&id),
            session_id: id,
        })
    }

    // ---- driving a live session -------------------------------------------

    /// close a session (idempotent). The daemon owns the teardown, so this only
    /// asks: the session leaves this node's map when its `TermEnded` arrives,
    /// which is the same path a child exiting on its own takes.
    pub async fn close(&self, id: &str) {
        self.send(wire::Command::TermClose {
            session: id.to_string(),
        })
        .await;
    }

    /// write raw keystrokes to a session's pty.
    pub async fn input(&self, id: &str, data_b64: &str) {
        self.send(wire::Command::TermInput {
            session: id.to_string(),
            data_b64: data_b64.to_string(),
        })
        .await;
    }

    /// change a session's window size.
    pub async fn resize(&self, id: &str, cols: u16, rows: u16) {
        self.send(wire::Command::TermResize {
            session: id.to_string(),
            cols,
            rows,
        })
        .await;
    }

    /// the single writer to the daemon. A missing or closed link drops the
    /// command with a named reason — never a panic; the session is ending
    /// anyway, and [`AttachGuard`] is what tells the client so.
    async fn send(&self, command: wire::Command) {
        let Some(link) = self.link() else {
            tracing::warn!(target: "ducktape::term", reason = "no_agent_service", "term command dropped");
            return;
        };
        if link.send(command).await.is_err() {
            tracing::warn!(target: "ducktape::term", reason = "agent_service_gone", "term command dropped");
        }
    }

    /// the `CommandSource` entry point: enqueue an ordered, origin-attributed
    /// command for `session`. The per-session serial consumer assigns the total
    /// order and feeds the pty — this only appends to the FIFO lane. An unknown
    /// session id is a no-op + `warn` (`unknown_session`), exactly like the
    /// input/resize handlers; never a panic. Never logs the command text.
    pub fn enqueue_command(&self, session: &str, origin: String, text: String) {
        let found = self.with_session(session, |live| (live.mode, live.cmd_tx.clone()));
        let Some((mode, tx)) = found else {
            tracing::warn!(target: "ducktape::term", session = %session, reason = "unknown_session", "term command dropped");
            return;
        };
        // commands are the SHARED-session path; a Single session has no lane.
        if mode != SessionMode::Shared {
            tracing::warn!(target: "ducktape::term", session = %session, reason = "command_on_single", "term command dropped");
            return;
        }
        // a send failure means the consumer already exited (a teardown race);
        // the session is ending, so the drop is benign.
        let _ = tx.send(Command { origin, text });
    }

    /// the serial command consumer: the total order. One task per Shared
    /// session. It drains the session's ordered command lane FIFO, stamps each
    /// command with a monotonic per-session `seq` (via the shared command-log
    /// ring, starting at 1), records `(seq, origin, text)` to that ring — which
    /// wakes `term-cmd:<id>` subscribers — THEN feeds the command grain to the
    /// pty as a submitted line (the text with a trailing carriage return).
    /// Serial processing IS the total order; one node stamps it. Exits the
    /// moment every sender drops (the `Live` entry left the map), so it can
    /// never outlive a reused id.
    ///
    /// The order is assigned HERE, not in the daemon: the ordered attributed log
    /// is the shared object this node publishes, and the pty write is merely its
    /// effect.
    fn spawn_command_consumer(&self, id: String, mut rx: mpsc::UnboundedReceiver<Command>) {
        let bridge = self.clone();
        tokio::spawn(async move {
            while let Some(Command { origin, text }) = rx.recv().await {
                // record + wake subscribers BEFORE the pty write. Never log the
                // command text — it can carry secrets.
                let seq = bridge.0.cmd_ring.append(&id, &origin, &text);
                tracing::debug!(target: "ducktape::term", session = %id, seq, %origin, "term_command");
                // one write, not two: a submitted line is `text` plus the
                // carriage return that submits it.
                let mut line = text.into_bytes();
                line.push(b'\r');
                bridge.input(&id, &STANDARD.encode(&line)).await;
            }
        });
    }

    // ---- what this node knows about a session ------------------------------

    /// the mode a session was created with (for the ws input gates), if live.
    pub fn mode(&self, id: &str) -> Option<SessionMode> {
        self.with_session(id, |live| live.mode)
    }

    /// the node that created a peer-attached session — the host-side input gate.
    /// `None` for a local session (no creator peer) or an unknown id, so a
    /// forwarded input frame for either is refused.
    pub fn creator_node(&self, id: &str) -> Option<[u8; 32]> {
        self.with_session(id, |live| live.creator_node).flatten()
    }

    /// read one fact off a live session under the lock, never holding it across
    /// anything that can await.
    fn with_session<T>(&self, id: &str, pick: impl FnOnce(&Live) -> T) -> Option<T> {
        self.0
            .sessions
            .lock()
            .expect("term sessions lock poisoned")
            .get(id)
            .map(pick)
    }

    /// take a pending create's answer channel, if it has not been answered yet.
    fn take_reply(&self, id: &str) -> Option<oneshot::Sender<Result<(), TermError>>> {
        self.0
            .sessions
            .lock()
            .expect("term sessions lock poisoned")
            .get_mut(id)
            .and_then(|live| live.reply.take())
    }
}

/// answer a pending create that ended without a pty — a refusal, or a detach.
///
/// A dropped receiver is benign HERE and only here: there is no session to leak,
/// because none was ever created. The success path deliberately does NOT use
/// this ([`TerminalSessions::created`] must act on the dropped receiver).
fn answer(reply: Option<oneshot::Sender<Result<(), TermError>>>, refusal: TermError) {
    if let Some(reply) = reply {
        let _ = reply.send(Err(refusal));
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

/// the `POST /v1/term/sessions` body. The `{agent, mode}` local path is
/// unchanged when the cross-node fields are absent; a `node` (or a bare `cred`
/// on this node) routes the create over the mesh to a host peer.
#[derive(Deserialize)]
pub struct CreateSessionBody {
    /// provider tag (`claude`|`codex`|a test provider); required.
    pub agent: String,
    /// `"single"` (default) = raw-keystroke solo terminal; `"shared"` = ordered
    /// `TermCommand` lane.
    #[serde(default)]
    pub mode: SessionMode,
    /// hex host node key; `None` = this node (the local path).
    #[serde(default)]
    pub node: Option<String>,
    /// credential name; required when `node` is set.
    #[serde(default)]
    pub cred: Option<String>,
    #[serde(default)]
    pub cpu: Option<u64>,
    #[serde(default)]
    pub mem_gb: Option<u64>,
}

/// where a create is served: on this node (today's local path) or directed to a
/// host peer over the mesh (the guest lane, loopback-short-circuited when the
/// host is this node itself).
#[derive(Debug, PartialEq, Eq)]
pub enum CreateRoute {
    Local,
    Remote { host: [u8; 32] },
}

/// decode a 64-hex node key to its 32 bytes.
fn decode_node_key(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (byte, pair) in out.iter_mut().zip(hex.as_bytes().chunks_exact(2)) {
        let text = std::str::from_utf8(pair).ok()?;
        *byte = u8::from_str_radix(text, 16).ok()?;
    }
    Some(out)
}

/// the pure create-routing decision. `own_node` is this node's key (for the bare-
/// cred own-node attach). A cross-node create requires a credential; a set `node`
/// must be a 32-byte hex key.
fn create_route(
    node: Option<&str>,
    cred: Option<&str>,
    own_node: Option<[u8; 32]>,
) -> Result<CreateRoute, (StatusCode, &'static str)> {
    match node {
        // no node: a bare `cred` runs on THIS node with that credential (the
        // own-node attach, looped back through the session lane); no cred is the
        // unchanged local path.
        None => match cred {
            None => Ok(CreateRoute::Local),
            Some(_) => match own_node {
                Some(host) => Ok(CreateRoute::Remote { host }),
                None => Err((
                    StatusCode::SERVICE_UNAVAILABLE,
                    "terminal sessions are not enabled on this node",
                )),
            },
        },
        // a named host: a credential is mandatory, and the key must be 32-byte hex.
        Some(node) => {
            if cred.is_none() {
                return Err((StatusCode::BAD_REQUEST, "a cross-node session requires --cred"));
            }
            let host = decode_node_key(node)
                .ok_or((StatusCode::BAD_REQUEST, "node must be a 32-byte hex node key"))?;
            Ok(CreateRoute::Remote { host })
        }
    }
}

/// POST /v1/term/sessions — create an interactive session and return its id +
/// ws topic. A local create spawns here; a directed (`node`/`cred`) create rides
/// the guest lane to the host. Over the cap, missing sandbox, unknown agent, or a
/// host refusal each return a clear error (never a panic, never a Direct spawn).
pub async fn create_session(
    State(handle): State<NodeHandle>,
    Json(body): Json<CreateSessionBody>,
) -> Response {
    let own_node = handle
        .admin
        .node_key
        .as_deref()
        .and_then(|key| <[u8; 32]>::try_from(key).ok());
    let route = match create_route(body.node.as_deref(), body.cred.as_deref(), own_node) {
        Ok(route) => route,
        Err((status, msg)) => {
            return (status, Json(serde_json::json!({ "error": msg }))).into_response();
        }
    };
    match route {
        CreateRoute::Local => create_local(&handle, body).await,
        CreateRoute::Remote { host } => create_remote(&handle, host, body).await,
    }
}

/// the unchanged local create path (today's `{agent, mode}`).
async fn create_local(handle: &NodeHandle, body: CreateSessionBody) -> Response {
    let Some(terminals) = handle.terminals() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": "terminal sessions are not enabled on this node" })),
        )
            .into_response();
    };
    // the size the CLI sent rides through: it used to be decoded into `body`
    // and then silently dropped here, so `--cpu`/`--mem` did nothing on the
    // local path and every microVM session was refused for having no `cores`.
    let size = SessionSize {
        cpu: body.cpu,
        mem_gb: body.mem_gb,
    };
    let created = match terminals.create(&body.agent, body.mode, size).await {
        Ok(created) => created,
        Err(err) => return err.response(),
    };
    // PR2 consensus command source: a Shared session's ordered command lane is a
    // dedicated chat channel — consensus signs + orders + persists each command
    // for free (a Single session has no lane, so nothing to wire). Ensure the
    // channel exists BEFORE returning, so a member cannot post a command before
    // its carrier does; then spawn the off-loop projector that drives committed
    // posts into this node's pty. A channel-create failure degrades to PR1's
    // node-local ws `TermCommand` path — the session still works single-node,
    // just without the consensus lane — so it warns and continues.
    if body.mode == SessionMode::Shared {
        let channel = crate::term_consensus::session_channel(&created.session_id);
        match crate::term_consensus::ensure_channel(handle, &channel).await {
            Ok(()) => {
                crate::term_consensus::spawn_projector(handle.clone(), created.session_id.clone());
            }
            Err(reason) => {
                tracing::warn!(target: "ducktape::term", session = %created.session_id, reason = %reason, "term consensus channel not created");
            }
        }
    }
    (StatusCode::OK, Json(created)).into_response()
}

/// the directed create path: hand a `SessionJob::Create` to the guest lane and
/// await the host's reply. On success remember the session→host binding so the
/// ws input/resize handlers forward to that host.
async fn create_remote(handle: &NodeHandle, host: [u8; 32], body: CreateSessionBody) -> Response {
    let Some(lane) = handle.session_lane() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": "terminal sessions are not enabled on this node" })),
        )
            .into_response();
    };
    let cred = body.cred.expect("create_route guarantees a cred on the remote path");
    let (reply, rx) = oneshot::channel();
    let job = crate::term_remote::SessionJob::Create {
        host,
        provider: body.agent,
        cred,
        cpu: body.cpu,
        mem_gb: body.mem_gb,
        reply,
    };
    if lane.send(job).await.is_err() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": "terminal sessions are not enabled on this node" })),
        )
            .into_response();
    }
    let created = match tokio::time::timeout(Duration::from_secs(30), rx).await {
        Ok(Ok(Ok(created))) => created,
        Ok(Ok(Err(msg))) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": format!("host refused: {msg}") })),
            )
                .into_response();
        }
        Ok(Err(_)) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": "the session lane dropped the request" })),
            )
                .into_response();
        }
        Err(_) => {
            return (
                StatusCode::GATEWAY_TIMEOUT,
                Json(serde_json::json!({ "error": "the host did not respond" })),
            )
                .into_response();
        }
    };
    handle
        .remote_sessions()
        .remember(created.session_id.clone(), host);
    (StatusCode::OK, Json(created)).into_response()
}

/// POST /v1/term/sessions/{id}/close — end a session. Idempotent: a closed or
/// unknown id is a 204 no-op. A remote session's close is forwarded to its host.
pub async fn close_session(State(handle): State<NodeHandle>, Path(id): Path<String>) -> Response {
    if let Some(host) = handle.remote_sessions().host_of(&id) {
        if let Some(lane) = handle.session_lane() {
            let _ = lane
                .send(crate::term_remote::SessionJob::Close {
                    host,
                    session: id.clone(),
                })
                .await;
        }
        handle.remote_sessions().forget(&id);
        return StatusCode::NO_CONTENT.into_response();
    }
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
    fn ring_marks_ended_idempotently_even_without_prior_output() {
        let ring = TermRing::default();
        ring.append("s", STANDARD.encode(b"hi"));
        assert!(!ring.is_ended("s"), "a live session is not ended");
        ring.mark_ended("s");
        assert!(ring.is_ended("s"), "the pump's EOF marks the ring ended");
        ring.mark_ended("s"); // idempotent — the double-close race is a no-op
        assert!(ring.is_ended("s"));

        // THE WEDGE CASE: a session that produced NO output has no ring entry
        // yet — mark_ended must still create it and flag it ended, so the
        // `agent pty` client waiting on the topic learns the session is over.
        assert!(!ring.is_ended("silent"), "no entry yet, so not ended");
        ring.mark_ended("silent");
        assert!(ring.is_ended("silent"), "a no-output session still signals end");
    }

    /// THE CROSS-NODE WEDGE: a peer-attached session's `agent pty` client waits
    /// on ITS OWN node's `term:<id>` topic, which only ends when the guest node's
    /// ring is flagged — and the guest node learns nothing except what
    /// `term_plane` forwards. So the END has to ride the peer feed, exactly like
    /// the bytes, or the client stays attached to a dead session forever.
    #[test]
    fn a_forwarded_session_publishes_its_end_on_the_peer_feed() {
        let ring = TermRing::default();
        let mut feed = ring.subscribe_appends();

        ring.append("00000000deadbeef", STANDARD.encode(b"hi"));
        let chunk = feed.try_recv().expect("the chunk reaches the peer feed");
        assert!(matches!(chunk, TermFeedEvent::Chunk(_)));

        ring.mark_ended("00000000deadbeef");
        assert_eq!(
            feed.try_recv().expect("the END reaches the peer feed"),
            TermFeedEvent::Ended {
                session: "00000000deadbeef".into()
            },
            "the terminal grain must follow the bytes to the guest node"
        );
    }

    /// The other half: a solo local terminal, and a session this node is only
    /// MIRRORING, must not put anything on the peer feed — the first has no peer
    /// audience, and the second would fan a grain that arrived from a peer right
    /// back out.
    #[test]
    fn a_local_only_end_stays_off_the_peer_feed() {
        let ring = TermRing::default();
        let mut feed = ring.subscribe_appends();

        ring.append_local_only("00000000deadbeef", STANDARD.encode(b"hi"));
        ring.mark_ended_local_only("00000000deadbeef");
        assert!(ring.is_ended("00000000deadbeef"), "the local ring still ends");
        assert!(
            feed.try_recv().is_err(),
            "a node-local session owes its peers nothing"
        );
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
        assert!(
            !ring
            .read_after(&format!("s{TERM_RING_MAX_SESSIONS}"), 0, 8)
            .0
                .is_empty()
        );
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

    // ----- the peer-forwarder feed: publish (local) vs append_remote -----

    #[test]
    fn output_ring_publishes_local_appends_but_stays_silent_on_remote() {
        let ring = TermRing::default();
        let mut appends = ring.subscribe_appends();
        // a LOCAL append (the pump) publishes to the forwarder feed.
        ring.append("00000000deadbeef", STANDARD.encode(b"hi"));
        let event = appends.try_recv().expect("a local append publishes");
        assert_eq!(event.session(), "00000000deadbeef");
        let TermFeedEvent::Chunk(chunk) = event else {
            panic!("a local append publishes bytes, not a terminal grain");
        };
        assert_eq!(STANDARD.decode(&chunk.chunk_b64).unwrap(), b"hi");
        // a peer's chunk enters the ring WITHOUT re-publishing — breaks the loop.
        ring.append_remote(TermChunkEvent {
            session: "00000000deadbeef".into(),
            chunk_b64: STANDARD.encode(b"yo"),
        });
        assert!(
            matches!(
                appends.try_recv(),
                Err(broadcast::error::TryRecvError::Empty)
            ),
            "append_remote must not re-broadcast"
        );
        // both chunks are readable locally (delivery to this node's ws is free).
        let (rows, _) = ring.read_after("00000000deadbeef", 0, 8);
        assert_eq!(rows.len(), 2);
        assert_eq!(STANDARD.decode(&rows[1].1).unwrap(), b"yo");
        // a SINGLE session's pump append rings + wakes local ws but does NOT
        // publish — a solo terminal's bytes never leave the host node.
        ring.append_local_only("00000000deadbeef", STANDARD.encode(b"solo"));
        assert!(
            matches!(
                appends.try_recv(),
                Err(broadcast::error::TryRecvError::Empty)
            ),
            "append_local_only must not publish to peers"
        );
        let (rows, _) = ring.read_after("00000000deadbeef", 0, 8);
        assert_eq!(rows.len(), 3, "the solo chunk still rings locally");
    }

    #[test]
    fn command_ring_publishes_local_and_replays_remote_seq_verbatim() {
        let ring = TermCommandRing::default();
        let mut appends = ring.subscribe_appends();
        // a LOCAL append stamps seq 1 and publishes the grain.
        assert_eq!(ring.append("00000000deadbeef", "alice", "ls"), 1);
        let event = appends.try_recv().expect("a local append publishes");
        assert_eq!(
            (event.seq, event.origin.as_str(), event.text.as_str()),
            (1, "alice", "ls")
        );
        // a peer's command carries the ORIGIN's seq; append_remote preserves it
        // verbatim (never renumbers to 2) and does not re-broadcast.
        ring.append_remote(TermCommandEvent {
            session: "00000000deadbeef".into(),
            seq: 42,
            origin: "bob".into(),
            text: "pwd".into(),
        });
        assert!(matches!(
            appends.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        ));
        let (rows, _) = ring.read_after("00000000deadbeef", 0, 8);
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[1],
            (42, "bob".to_string(), "pwd".to_string()),
            "the origin's seq is replayed verbatim, not re-stamped"
        );
    }

    #[test]
    fn command_ring_evicts_least_recently_touched_sessions() {
        let ring = TermCommandRing::default();
        for i in 0..=TERM_RING_MAX_SESSIONS {
            ring.append(&format!("s{i}"), "m", "x");
        }
        // the first-touched session aged out; the newest survives.
        assert!(ring.read_after("s0", 0, 8).0.is_empty());
        assert!(
            !ring
            .read_after(&format!("s{TERM_RING_MAX_SESSIONS}"), 0, 8)
            .0
                .is_empty()
        );
    }

    #[test]
    fn create_body_routes_local_when_node_absent_and_remote_when_present() {
        let host_hex = "aa".repeat(32);
        let host = [0xaau8; 32];
        let me = [0x11u8; 32];
        // no node, no cred → local.
        assert_eq!(
            create_route(None, None, Some(me)).unwrap(),
            CreateRoute::Local
        );
        // a named host + cred → remote to that host.
        assert_eq!(
            create_route(Some(&host_hex), Some("c"), Some(me)).unwrap(),
            CreateRoute::Remote { host }
        );
        // no node but a bare cred → own-node attach (looped back).
        assert_eq!(
            create_route(None, Some("c"), Some(me)).unwrap(),
            CreateRoute::Remote { host: me }
        );
        // a named host without a cred → 400 requires --cred.
        let (status, msg) = create_route(Some(&host_hex), None, Some(me)).unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(msg, "a cross-node session requires --cred");
        // a malformed node key → 400 32-byte hex.
        let (status, msg) = create_route(Some("zz"), Some("c"), Some(me)).unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(msg, "node must be a 32-byte hex node key");
    }

    const TEST_TOKEN: &str = "0123456789abcdef0123456789abcdef";

    /// a bridge with a fake daemon on the other end: every `TermCreate` is
    /// answered `TermCreated`, which is all a create needs to complete. The
    /// caller drives everything else (`on_event`) by hand, so each test names
    /// exactly the daemon behaviour it is about.
    fn with_daemon() -> (TerminalSessions, AttachGuard) {
        let terminals = TerminalSessions::new(TermRing::default(), TermCommandRing::default(), Some(TEST_TOKEN.into()));
        let (guard, mut rx) = terminals.attach(TEST_TOKEN).expect("the first attach wins");
        let daemon = terminals.clone();
        tokio::spawn(async move {
            while let Some(command) = rx.recv().await {
                if let wire::Command::TermCreate(create) = command {
                    daemon.on_event(wire::Event::TermCreated {
                        session: create.session,
                    });
                }
            }
        });
        (terminals, guard)
    }

    /// like [`with_daemon`], but hands back every create the daemon received —
    /// the only place a session's sandbox size is observable.
    fn with_daemon_watching_creates()
    -> (TerminalSessions, AttachGuard, mpsc::Receiver<wire::Create>) {
        let terminals = TerminalSessions::new(
            TermRing::default(),
            TermCommandRing::default(),
            Some(TEST_TOKEN.into()),
        );
        let (guard, mut rx) = terminals.attach(TEST_TOKEN).expect("the first attach wins");
        let (seen_tx, seen_rx) = mpsc::channel::<wire::Create>(8);
        let daemon = terminals.clone();
        tokio::spawn(async move {
            while let Some(command) = rx.recv().await {
                if let wire::Command::TermCreate(create) = command {
                    daemon.on_event(wire::Event::TermCreated {
                        session: create.session.clone(),
                    });
                    let _ = seen_tx.send(create).await;
                }
            }
        });
        (terminals, guard, seen_rx)
    }

    #[tokio::test]
    async fn a_session_the_operator_did_not_size_still_gets_one() {
        // THE regression: under the microVM backend an absent `cores` is not an
        // unlimited run, it is a refused one — `ducktape agent pty claude` died
        // with "a microVM run needs an explicit `cores` limit" on every node,
        // with or without `--cpu`, because nothing filled the size in.
        let (terminals, _guard, mut creates) = with_daemon_watching_creates();
        terminals
            .create("claude", SessionMode::Single, SessionSize::default())
            .await
            .expect("the daemon answered");
        let create = creates.recv().await.expect("the daemon saw a create");
        assert_eq!(create.limits.get("cores"), Some(&DEFAULT_SESSION_CORES));
        assert_eq!(create.limits.get("mem_gb"), Some(&DEFAULT_SESSION_MEM_GB));
    }

    #[tokio::test]
    async fn the_size_the_operator_named_reaches_the_daemon() {
        // the other half: `--cpu`/`--mem` were decoded into the create body and
        // then dropped on the floor, so the flags did nothing at all.
        let (terminals, _guard, mut creates) = with_daemon_watching_creates();
        terminals
            .create(
                "claude",
                SessionMode::Single,
                SessionSize {
                    cpu: Some(6),
                    mem_gb: Some(12),
                },
            )
            .await
            .expect("the daemon answered");
        let create = creates.recv().await.expect("the daemon saw a create");
        assert_eq!(create.limits.get("cores"), Some(&6));
        assert_eq!(create.limits.get("mem_gb"), Some(&12));
    }

    #[test]
    fn creator_node_is_absent_for_a_local_or_unknown_session() {
        // the input-gate accessor is pure: an unknown id (and, by construction,
        // any local session) has no creator node — so a forwarded input frame
        // naming it is refused.
        let terminals = TerminalSessions::new(TermRing::default(), TermCommandRing::default(), Some(TEST_TOKEN.into()));
        assert!(terminals.creator_node("nope").is_none());
    }

    #[test]
    fn enqueue_command_on_an_unknown_session_is_a_no_op() {
        // enqueue must warn + no-op, never panic (mirrors the input/resize
        // unknown-session discipline) and record nothing.
        let terminals = TerminalSessions::new(TermRing::default(), TermCommandRing::default(), Some(TEST_TOKEN.into()));
        terminals.enqueue_command("nope", "alice".into(), "ls".into());
        assert!(terminals.0.cmd_ring.read_after("nope", 0, 8).0.is_empty());
    }

    #[tokio::test]
    async fn a_create_refuses_when_no_agent_service_is_attached() {
        // the `no_sandbox` rung, in its new meaning: the plane is wired (the
        // manager exists, so the route does NOT return "not enabled") but no
        // daemon owns a pty for it.
        let terminals = TerminalSessions::new(TermRing::default(), TermCommandRing::default(), Some(TEST_TOKEN.into()));
        assert!(!terminals.has_sandbox());
        let refused = terminals.create("claude", SessionMode::Single, SessionSize::default()).await;
        assert!(matches!(refused, Err(TermError::NoSandbox)));
    }

    #[test]
    fn a_second_attach_cannot_displace_a_live_daemon() {
        // FIRST ATTACH WINS is a boundary: a local impersonator that could take
        // the link would receive the create commands — lent-credential records
        // included — meant for the real daemon.
        let terminals = TerminalSessions::new(TermRing::default(), TermCommandRing::default(), Some(TEST_TOKEN.into()));
        let first = terminals.attach(TEST_TOKEN);
        assert!(first.is_some());
        assert!(
            terminals.attach(TEST_TOKEN).is_none(),
            "the link is already held"
        );
        // and it is reclaimable once the holder goes.
        drop(first);
        assert!(terminals.attach(TEST_TOKEN).is_some());
    }

    #[tokio::test]
    async fn a_create_completes_when_the_daemon_answers() {
        let (terminals, _guard) = with_daemon();
        let created = terminals
            .create("claude", SessionMode::Single, SessionSize::default())
            .await
            .expect("the daemon answered");
        assert_eq!(created.topic, topic(&created.session_id));
        assert_eq!(terminals.mode(&created.session_id), Some(SessionMode::Single));
    }

    #[tokio::test]
    async fn a_create_whose_caller_went_away_is_closed_not_leaked() {
        // THE cancelled-create regression. axum drops the handler future when
        // the client disconnects, and a Ctrl-C during a cold image pull is
        // ordinary. The pty still comes up on the daemon — but nobody ever
        // received its id, so no close can ever arrive for it. Unless this node
        // sends one, it burns a container on the operator's credential and one
        // of four cap slots for four hours.
        let terminals = TerminalSessions::new(
            TermRing::default(),
            TermCommandRing::default(),
            Some(TEST_TOKEN.into()),
        );
        let (_guard, mut rx) = terminals.attach(TEST_TOKEN).expect("the first attach wins");

        // no auto-answering daemon here: hold the create unanswered, exactly as
        // a slow pull would, and take the command off the link by hand.
        let mut pending = Box::pin(terminals.create("claude", SessionMode::Single, SessionSize::default()));
        let command = tokio::select! {
            _ = &mut pending => panic!("a create cannot complete while nothing answers it"),
            command = rx.recv() => command.expect("the create reached the link"),
        };
        let wire::Command::TermCreate(create) = command else {
            panic!("the first command must be the create");
        };
        // the caller gives up.
        drop(pending);
        // the daemon finishes starting the pty anyway.
        terminals.on_event(wire::Event::TermCreated {
            session: create.session.clone(),
        });
        assert_eq!(
            rx.recv().await.expect("a close must follow an abandoned create"),
            wire::Command::TermClose {
                session: create.session
            },
        );
    }

    #[test]
    fn the_link_needs_this_nodes_token() {
        // holding the link means BECOMING the interactive plane and receiving
        // every lent-credential record with it. Dialing loopback is not enough.
        let terminals = TerminalSessions::new(
            TermRing::default(),
            TermCommandRing::default(),
            Some(TEST_TOKEN.into()),
        );
        assert!(terminals.attach("").is_none(), "an empty token is not a token");
        assert!(
            terminals.attach("0123456789abcdef0123456789abcdee").is_none(),
            "a near miss is still a miss"
        );
        assert!(terminals.attach(TEST_TOKEN).is_some());
    }

    #[test]
    fn a_node_that_could_not_mint_a_token_refuses_every_attach() {
        // fail CLOSED: a node that cannot write its 0600 token has no way to
        // tell a daemon from any other local process, so it has no interactive
        // plane rather than an unguarded one.
        let terminals =
            TerminalSessions::new(TermRing::default(), TermCommandRing::default(), None);
        assert!(terminals.attach("").is_none());
        assert!(terminals.attach(TEST_TOKEN).is_none());
        assert!(!terminals.has_sandbox());
    }

    #[tokio::test]
    async fn a_session_whose_child_exits_ends_without_wedging() {
        // THE wedge regression. An attached `agent pty` client blocks on
        // `term:<id>` until the ring says the session ended; if the child's exit
        // did not mark it, the client would hang forever.
        let (terminals, _guard) = with_daemon();
        let created = terminals
            .create("claude", SessionMode::Single, SessionSize::default())
            .await
            .expect("the daemon answered");
        terminals.on_event(wire::Event::TermOutput {
            session: created.session_id.clone(),
            chunk_b64: STANDARD.encode(b"hi"),
        });
        terminals.on_event(wire::Event::TermEnded {
            session: created.session_id.clone(),
        });
        assert!(terminals.0.ring.is_ended(&created.session_id));
        assert!(
            terminals.mode(&created.session_id).is_none(),
            "an ended session leaves the map"
        );
    }

    #[tokio::test]
    async fn a_session_that_never_printed_a_byte_still_marks_ended() {
        // the no-output path, historically the one that wedged: the ring has no
        // entry for a session that produced nothing, so `mark_ended` must CREATE
        // one rather than silently do nothing.
        let (terminals, _guard) = with_daemon();
        let created = terminals
            .create("claude", SessionMode::Single, SessionSize::default())
            .await
            .expect("the daemon answered");
        terminals.on_event(wire::Event::TermEnded {
            session: created.session_id.clone(),
        });
        assert!(terminals.0.ring.is_ended(&created.session_id));
    }

    #[tokio::test]
    async fn a_detaching_daemon_ends_every_live_session() {
        // the failure mode the process split introduces: the daemon dies while
        // sessions are live. This node can no longer feed or close them, so it
        // must terminate them rather than leave every attached client blocked.
        let (terminals, guard) = with_daemon();
        let created = terminals
            .create("claude", SessionMode::Single, SessionSize::default())
            .await
            .expect("the daemon answered");
        drop(guard);
        assert!(terminals.0.ring.is_ended(&created.session_id));
        assert!(!terminals.has_sandbox());
        assert!(matches!(
            terminals.create("claude", SessionMode::Single, SessionSize::default()).await,
            Err(TermError::NoSandbox)
        ));
    }

    #[tokio::test]
    async fn a_solo_session_never_reaches_the_peer_forwarder_feed() {
        // `forward` is decided at create and stored, so an output frame that
        // races the create reply still routes correctly. A Single local session
        // stays node-local; publishing it would fan a private terminal out to
        // every peer.
        let (terminals, _guard) = with_daemon();
        let created = terminals
            .create("claude", SessionMode::Single, SessionSize::default())
            .await
            .expect("the daemon answered");
        let mut appends = terminals.0.ring.subscribe_appends();
        terminals.on_event(wire::Event::TermOutput {
            session: created.session_id.clone(),
            chunk_b64: STANDARD.encode(b"secret"),
        });
        assert!(
            appends.try_recv().is_err(),
            "a solo session must not publish to the peer feed"
        );
        // but it IS in the local ring the ws catch-up serves.
        assert!(!terminals.0.ring.read_after(&created.session_id, 0, 8).0.is_empty());
    }

    #[tokio::test]
    async fn a_shared_session_does_reach_the_peer_forwarder_feed() {
        let (terminals, _guard) = with_daemon();
        let created = terminals
            .create("claude", SessionMode::Shared, SessionSize::default())
            .await
            .expect("the daemon answered");
        let mut appends = terminals.0.ring.subscribe_appends();
        terminals.on_event(wire::Event::TermOutput {
            session: created.session_id.clone(),
            chunk_b64: STANDARD.encode(b"shared"),
        });
        assert!(appends.try_recv().is_ok(), "a shared session fans out");
    }

    #[tokio::test]
    async fn a_daemon_refusal_keeps_the_diagnosis_ladder_intact() {
        // the rungs an operator reads: 503 = nothing can serve this, 429 = it
        // could but is full, 400 = you asked for a provider nobody has, 500 =
        // the spawn itself failed. Each must survive the process boundary.
        let ladder = [
            (TermError::NoSandbox, StatusCode::SERVICE_UNAVAILABLE),
            (
                TermError::from_refusal(wire::Refusal::AtCapacity, String::new()),
                StatusCode::TOO_MANY_REQUESTS,
            ),
            (
                TermError::from_refusal(wire::Refusal::UnknownProvider, "no such tag".into()),
                StatusCode::BAD_REQUEST,
            ),
            (
                TermError::from_refusal(wire::Refusal::SpawnFailed, "image absent".into()),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        ];
        for (error, expected) in ladder {
            assert_eq!(error.response().status(), expected);
        }
    }

    #[tokio::test]
    async fn a_refused_create_returns_the_daemon_reason_and_forgets_the_session() {
        let terminals = TerminalSessions::new(TermRing::default(), TermCommandRing::default(), Some(TEST_TOKEN.into()));
        let (_guard, mut rx) = terminals.attach(TEST_TOKEN).expect("the first attach wins");
        let daemon = terminals.clone();
        tokio::spawn(async move {
            while let Some(command) = rx.recv().await {
                if let wire::Command::TermCreate(create) = command {
                    daemon.on_event(wire::Event::TermRefused {
                        session: create.session,
                        reason: wire::Refusal::SpawnFailed,
                        detail: "image absent".into(),
                    });
                }
            }
        });
        let refused = terminals.create("claude", SessionMode::Single, SessionSize::default()).await;
        let Err(TermError::Spawn(detail)) = refused else {
            panic!("a spawn failure must surface as Spawn, not another rung");
        };
        assert_eq!(detail, "image absent");
    }
}
