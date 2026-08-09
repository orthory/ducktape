use std::collections::{BTreeMap, VecDeque};
use std::io::{Result as IoResult, Write};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::extract::ws::{Message, WebSocket};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use duckfs_core::{Change, FilesMsg};
use futures::StreamExt as _;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc, watch};
use tracing_subscriber::fmt::MakeWriter;

use crate::NodeHandle;

/// the TIMER beat: the liveness floor while no blocks commit, and (×2.5) the
/// client watchdog's timeout basis. a heartbeat frame also rides every block
/// wake, so on a moving chain the tip reaches clients per block, not per tick.
pub const HEARTBEAT_INTERVAL_MS: u64 = 3_000;
/// THE ANTI-ENTROPY BACKSTOP. A block wake sweeps the index topics only when
/// the block appended rows ([`BlockWake`]), which makes delivery depend on
/// every index writer announcing. That set is provable today and silently
/// breakable tomorrow — a writer added later that forgets would strand a topic
/// with no error, no `lagged`, and a head that keeps rising. This bounds every
/// such miss, known or not, to one period.
///
/// It REPLACES a sweep that ran once per block — 1 Hz on an idle chain, since
/// the nop filler publishes every `BLOCK_TIME` — so it is 30x less work than
/// what it stands in for, not new work.
pub const INDEX_BACKSTOP_INTERVAL: Duration = Duration::from_secs(30);
pub const STREAM_CATCHUP_BUDGET: usize = 256;
/// per-connection subscription ceiling. the ws surface is unauthenticated
/// (trusted-client convention), so per-connection state must stay bounded:
/// the console needs ~15 module topics + logs + files:watch + metrics + a
/// few run-output panes; far below this. beyond it, subscribes refuse
/// per-topic.
pub const MAX_TOPICS_PER_CONNECTION: usize = 64;
/// rows a files:watch catch-up may SCAN (not just emit) per wakeup — a
/// stage-heavy history is mostly non-commit rows, and an unbounded back-scan
/// would stall the session task; past this the topic lags to live instead.
pub const FILES_SCAN_BUDGET: usize = STREAM_CATCHUP_BUDGET * 4;
pub const LOG_RING_CAPACITY: usize = 4_096;
pub const RUN_OUTPUT_MAX_RUNS: usize = 32;
pub const RUN_OUTPUT_MAX_LINES: usize = 2_048;
/// the exact width of a run-output id: `runs::dispatch_id_for` is a hex
/// sha256, and the agent data plane's `valid_event` enforces the same 64-hex
/// shape before forwarding a line to a peer. This is NOT cosmetic — see
/// [`ClientMsg::RunOutput`].
const RUN_OUTPUT_ID_LEN: usize = 64;
/// the longest run-output line accepted from a ws publisher.
///
/// The agent data plane refuses to write a serialized event over 64 KiB, so
/// this must stay comfortably under that: a line admitted here but refused
/// there would be the same stream teardown, one layer later. 16 KiB is far
/// above any real provider line while leaving room for the peer forwarder's
/// `[node xxxxxxxx] ` prefix and the json envelope.
const MAX_RUN_OUTPUT_LINE: usize = 16 * 1024;

/// how long a command may wait to reach the attached service daemon before the
/// link is declared wedged. Generous — a healthy daemon takes one in microseconds
/// (it only enqueues), so anything near this is a stuck process, not a slow one.
const SERVICE_COMMAND_WRITE_TIMEOUT: Duration = Duration::from_secs(10);

/// what a ws client may say to this node.
///
/// `deny_unknown_fields`: there is no live network and no compat obligation, so
/// a frame carrying a field this build does not know is refused with a
/// `BadFrame` naming it, never decoded into whatever subset happens to match.
/// Silently dropping the rest would make the sender's intent unobservable — and
/// this PR is a live instance of the direction that used to be tolerated, since
/// it deleted `ServiceAttach.build`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClientMsg {
    /// join one or more topics. THIS is where a topic's admission is decided —
    /// see [`Topic::admission`] — so the handles this frame hands back are
    /// themselves the capability, and a family a caller was never admitted to
    /// leaves nothing on the connection to act on.
    Subscribe {
        topics: Vec<String>,
        #[serde(default)]
        resume: BTreeMap<String, String>,
        /// this node's own 0600 workspace secret ([`crate::services::LINK_TOKEN_FILE`]),
        /// for the families that carry a member's terminal or a run's output.
        ///
        /// The SAME secret [`Self::ServiceAttach`] presents, deliberately: the
        /// node has exactly one proof that a caller can read its own workspace,
        /// and a second would be a second thing to leak. Presenting it here
        /// claims no link and displaces no daemon — it only answers the
        /// admission question.
        ///
        /// Absent on a caller that wants only the public families (committed
        /// module events, the log ring, the metrics exposition): those admit
        /// anyone the ws surface admits, because the same bytes already leave
        /// this node over an unauthenticated HTTP route.
        #[serde(default)]
        token: Option<String>,
    },
    Unsubscribe {
        topics: Vec<String>,
    },
    /// keystrokes for an interactive terminal session (see `crate::term`).
    /// `data` is base64 of the raw bytes to write to the session's pty. A
    /// session this connection holds no admitted handle on — and an unknown id
    /// — is dropped with a named reason, never a panic ([`holds_session`]).
    TermInput {
        session: String,
        data: String,
    },
    /// a terminal resize for an interactive session: set the pty window size so
    /// the CLI's TUI reflows.
    TermResize {
        session: String,
        cols: u16,
        rows: u16,
    },
    /// a submitted COMMAND for an interactive session — the ordered "command
    /// grain" (a prompt / line), not raw keystrokes. `origin` is the
    /// caller-supplied attribution (the app passes a member label), stored
    /// verbatim and UNTRUSTED until consensus signs it (PR 2). Gated exactly
    /// like [`Self::TermInput`] (the
    /// connection must be subscribed to the session's `term:<id>` topic); the
    /// session's serial consumer assigns the total order and feeds `text` +
    /// Enter to the pty. This is the `CommandSource` seam consensus (PR 2) will
    /// drive; `TermInput` stays for the solo raw-keystroke case.
    TermCommand {
        session: String,
        text: String,
        origin: String,
    },
    /// one live output line from a run this node's COMPUTE DAEMON is executing.
    ///
    /// The daemon runs out of process, so the in-process `OutputSink` that used
    /// to feed [`RunOutputRegistry`] cannot reach it any more; this is that sink
    /// across the process boundary, on the ws connection the daemon already
    /// holds for work-intake hints. It is a publish, not a subscription, which
    /// is why it is a `ClientMsg` and not a topic.
    ///
    /// Trust: the ws surface is unauthenticated by the trusted-local convention
    /// (a local process can already read the node's key off disk), and a
    /// run-output ring is a DISPLAY buffer no consensus decision reads. There is
    /// deliberately NO subscription check on this publish, unlike the terminal
    /// frames' [`holds_session`]: the publisher — the compute daemon — subscribes
    /// to nothing (`bin/node/src/compute/link.rs`), and a run id names consensus
    /// state a publisher legitimately learns from the chain, so subscription is
    /// not evidence of authorship and gating on it would refuse the daemon its
    /// own runs. Spoofing a line into another run's DISPLAY ring is the accepted
    /// residual risk of the trusted-local surface.
    ///
    /// READING that ring is a different question with a different answer:
    /// `run-output:<id>` is [`Admission::Workspace`], because provider stdout is
    /// the same class of bytes a pty carries. Write-open / read-gated is
    /// deliberate asymmetry, not an oversight.
    ///
    /// What is NOT accepted is an unbounded or malformed one. `id` must be the
    /// 64-hex shape the agent data plane's `valid_event` enforces, because a
    /// line that reaches the ring is broadcast to every overlay peer and a
    /// rejected write there tears the peer's stream down — one bad frame would
    /// otherwise be a remote, repeatable denial of service against every peer's
    /// telemetry. `line` is capped for the same reason. Both are checked at this
    /// boundary and dropped with a stable reason; the ring's own
    /// `RUN_OUTPUT_MAX_RUNS`/`RUN_OUTPUT_MAX_LINES` bound line COUNT, never
    /// bytes, so they are no substitute.
    RunOutput {
        id: String,
        stream: RunStream,
        line: String,
    },
    /// a local service daemon claims this connection as its command link.
    ///
    /// The agent daemon owns the ptys behind this node's interactive plane, and
    /// it is the only side that dials — so it must be able to say "commands for
    /// the terminal plane come to me". Until one connection does this, the node
    /// has no interactive plane and every create refuses.
    ///
    /// The claim carries no build stamp and this node compares none: node and
    /// daemon are separate processes with independent restart timing, so skew
    /// is ordinary, and it is named by `service status` rather than refused
    /// (see [`crate::services::build_identity`]). What IS refused is a second
    /// holder — only one connection may hold the link at a time, which is what
    /// stops a local impersonator from displacing the live daemon and receiving
    /// the create commands (and lent-credential records) meant for it.
    ServiceAttach {
        kind: String,
        /// the node's own 0600 link secret, read from its workspace. Holding the
        /// link means BECOMING this node's interactive plane and receiving every
        /// lent-credential record with it, so dialing loopback is not enough.
        token: String,
    },
    /// one lifecycle fact about a pty from the daemon that owns it. Honored ONLY
    /// on a connection that has attached: without that gate, any local process
    /// could inject output into a session's ring or fake its end.
    AgentEvent {
        event: agent_service::wire::Event,
    },
}

// Serialize-only: the node SENDS frames and never parses its own, so there is
// no `Deserialize` to conflict when [`Self::TermChunk`] shares the `event` tag
// with [`Self::Event`] (a derived deserializer's tag match would be ambiguous).
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerFrame {
    Subscribed {
        /// admitted topic -> its start cursor. A REFUSED topic is not in here;
        /// it got its own `Error` frame, ahead of this one, naming the code.
        /// (This was `Option<String>` with exactly one inhabitant — the only
        /// insert always carried a cursor — which read as "admitted but
        /// cursorless", a state that has never existed.)
        topics: BTreeMap<String, String>,
    },
    Event {
        topic: String,
        cursor: String,
        op: StreamOpRow,
    },
    /// one raw chunk of interactive-terminal output on a `term:<session>`
    /// topic: `item` is base64 of the pty bytes and `cursor` is the ring
    /// sequence used to resume without replaying bytes already rendered. It
    /// rides the SAME `type: "event"` tag the client keys on, but carries no
    /// `op` — the client routes it by the `term:` topic prefix + string `item`,
    /// distinct from the op-carrying
    /// module [`Self::Event`]. ServerFrame is
    /// serialize-only at runtime (the node sends, never parses its own frames),
    /// so sharing the `event` tag is safe.
    #[serde(rename = "event")]
    TermChunk {
        topic: String,
        cursor: String,
        item: String,
    },
    /// one entry of an interactive session's ordered, attributed command log on
    /// a `term-cmd:<session>` topic: the total-order `seq`, the command's
    /// `origin` (attribution), and its `text` (the submitted line). Distinct
    /// from the raw-output [`Self::TermChunk`] — this is the
    /// shared-conversation-object view. Delivered + caught up like a run-output
    /// tail: a `seq` cursor, replayed on (re)subscribe.
    TermCommandLog {
        topic: String,
        seq: u64,
        origin: String,
        text: String,
    },
    Tail {
        topic: String,
        cursor: String,
        item: TailItem,
    },
    /// the interactive session's child (and its container) has exited: the
    /// `term:<session>` topic is complete and will never append again. Sent
    /// ONCE, after the session's final output chunks, then the topic is dropped.
    /// A driving `agent pty` client breaks its attach loop on this (the desktop
    /// app closes the pane); without it a client blocks forever on a dead topic.
    TermEnded {
        topic: String,
    },
    /// one command for the attached agent daemon: spawn a pty, feed it, resize
    /// it, end it. Sent only on the connection that holds the service link, so
    /// it never reaches an ordinary subscriber.
    ServiceCommand {
        command: agent_service::wire::Command,
    },
    Lagged {
        topic: String,
        cursor: String,
    },
    Heartbeat {
        height: u64,
        root_hash: String,
        time_ms: u64,
        interval_ms: u64,
    },
    Error {
        topic: String,
        code: StreamErrorCode,
        detail: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StreamErrorCode {
    UnknownTopic,
    Unavailable,
    BadCursor,
    BadFrame,
    /// the topic exists and this caller may not hold it — see
    /// [`Topic::admission`]. Distinct from `UnknownTopic` on purpose: a client
    /// that mistyped a name and one that omitted the node's secret need
    /// different fixes, and collapsing them would send an operator hunting a
    /// typo that is not there.
    Forbidden,
}

/// The ws projection of one stored (borsh) op row — the same json row shape
/// the /v1/index/*/ops lane serves.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct StreamOpRow {
    pub height: u64,
    pub seq: u32,
    pub time: u64,
    pub origin: StreamOrigin,
    // skip_serializing_if omits the field on the wire, so the TS side must
    // read `payload?: …` (absent), not `payload: … | null`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_hex: Option<String>,
    /// the module-assigned stamp of the dispatch (empty stamps are omitted).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assigned: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assigned_hex: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StreamOrigin {
    pub kind: StreamOriginKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StreamOriginKind {
    External,
    Module,
    System,
}

/// project one stored (borsh) op row onto the ws frame shape — the same
/// json row the /v1/index/*/ops lane serves.
fn stream_op_row(row: indexer::OpRow) -> StreamOpRow {
    let payload: Option<serde_json::Value> = serde_json::from_slice(&row.payload).ok();
    let payload_hex = payload.is_none().then(|| crate::hex_bytes(&row.payload));
    let assigned: Option<serde_json::Value> = (!row.assigned.is_empty())
        .then(|| serde_json::from_slice(&row.assigned).ok())
        .flatten();
    let assigned_hex = (!row.assigned.is_empty() && assigned.is_none())
        .then(|| crate::hex_bytes(&row.assigned));
    StreamOpRow {
        height: row.height,
        seq: row.seq,
        time: row.time,
        origin: StreamOrigin {
            kind: match row.origin.kind {
                indexer::OriginKind::External => StreamOriginKind::External,
                indexer::OriginKind::Module => StreamOriginKind::Module,
                indexer::OriginKind::System => StreamOriginKind::System,
            },
            id: row.origin.id,
        },
        payload,
        payload_hex,
        assigned,
        assigned_hex,
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum TailItem {
    Log {
        line: String,
    },
    FileChange {
        height: u64,
        time: u64,
        message: String,
        base_snapshot: Option<String>,
        paths: Vec<String>,
    },
    RunOutput {
        stream: RunStream,
        line: String,
    },
    /// one OpenMetrics snapshot — the same text GET /metrics serves, pushed
    /// per heartbeat tick while the `metrics` topic is subscribed. `time_ms`
    /// is the server-side sample instant, so a client derives counter rates
    /// from one clock instead of its own frame-arrival jitter.
    Metrics {
        time_ms: u64,
        text: String,
    },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStream {
    Stdout,
    Stderr,
}

/// What a block wake owes the index-tier topics.
///
/// The tip snapshot is owed UNCONDITIONALLY — a console's head moves on nop
/// fillers, which feed no topic at all — so this gates the index SWEEP alone,
/// never the heartbeat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockWake {
    /// the tip moved and nothing else. An idle chain nop-fills once per
    /// `BLOCK_TIME` (`bin/node/src/constants.rs`) and that filler appends no
    /// per-module op row, so every scan it used to trigger returned empty.
    TipOnly,
    /// op rows were appended under the subscribers.
    IndexChanged,
}

impl BlockWake {
    /// Op rows are built one-for-one from dispatches (`index_block_ops`), so an
    /// empty dispatch list appends none and the index tier owes nothing.
    ///
    /// NOT `applied`, and not the explorer `record`: `applied` is false on a
    /// System-only block whose dispatches are not (see `projection.rs`, where
    /// System dispatches merge after the member loop), and `record` lands in
    /// the blocks db, which no ws topic reads.
    pub fn from_dispatches(dispatches: &[host::DispatchRecord]) -> Self {
        match dispatches.is_empty() {
            true => Self::TipOnly,
            false => Self::IndexChanged,
        }
    }
}

/// What one block wake tells a session to do.
///
/// A VALUE, not a branch taken in place: the arm that consumes it is inside a
/// `select!` over a live socket, so this is the only way the decision is
/// reachable from a test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlockAction {
    /// send the tip, then re-scan the index topics.
    SweepIndex,
    /// send the tip and nothing else — the block appended no op row.
    TipOnly,
    /// the hub is gone; the session is over.
    Stop,
}

/// Decide what a block wake owes, from the wake alone. Writes nothing.
fn block_action(note: Result<BlockWake, broadcast::error::RecvError>) -> BlockAction {
    match note {
        Ok(BlockWake::IndexChanged) => BlockAction::SweepIndex,
        Ok(BlockWake::TipOnly) => BlockAction::TipOnly,
        // N WAKES WERE DROPPED AND THEIR DISCRIMINANTS WITH THEM. Any one may
        // have been `IndexChanged` and there is no way to tell which, so sweep:
        // a scan re-reads the store as it stands now, which costs a miss
        // nothing, while skipping one strands the topic until the backstop.
        Err(broadcast::error::RecvError::Lagged(_)) => BlockAction::SweepIndex,
        Err(broadcast::error::RecvError::Closed) => BlockAction::Stop,
    }
}

#[derive(Clone)]
pub struct StreamHub {
    /// block wakeups carrying whether the index tier changed — `publish_block`
    /// primes `tip` before broadcasting, so the wake always reads its own
    /// block. A `TipOnly` wake still moves every subscriber's head; it just
    /// does not send them back to the store for rows that are not there.
    blocks: broadcast::Sender<BlockWake>,
    tip: Arc<RwLock<Option<(u64, String)>>>,
    logs: LogRing,
    run_output: RunOutputRegistry,
    /// per-session interactive-terminal scrollback. Always present (cheap), the
    /// same way `run_output` is: the terminal manager appends to it and the ws
    /// catch-up path replays it for `term:<session>` subscribers.
    terminals: crate::term::TermRing,
    /// per-session ordered command log — a focused twin of `terminals`: the
    /// session's serial command consumer appends `(seq, origin, text)` and the
    /// ws catch-up path replays it for `term-cmd:<session>` subscribers.
    term_commands: crate::term::TermCommandRing,
}

impl StreamHub {
    #[cfg(test)]
    pub fn new(buffer: usize) -> Self {
        Self::with_log_ring(buffer, LogRing::default())
    }

    pub fn with_log_ring(buffer: usize, logs: LogRing) -> Self {
        let (blocks, _) = broadcast::channel(buffer);
        Self {
            blocks,
            tip: Arc::new(RwLock::new(None)),
            logs,
            run_output: RunOutputRegistry::default(),
            terminals: crate::term::TermRing::default(),
            term_commands: crate::term::TermCommandRing::default(),
        }
    }

    pub fn publish_block(&self, height: u64, root_hash: impl Into<String>, wake: BlockWake) {
        self.prime(height, root_hash);
        let _ = self.blocks.send(wake);
    }

    pub fn prime(&self, height: u64, root_hash: impl Into<String>) {
        *self.tip.write().expect("stream tip lock poisoned") = Some((height, root_hash.into()));
    }

    pub fn log_ring(&self) -> LogRing {
        self.logs.clone()
    }

    pub fn run_output(&self) -> RunOutputRegistry {
        self.run_output.clone()
    }

    /// the interactive-terminal scrollback ring. The daemon hands this to the
    /// [`crate::term::TerminalSessions`] manager so its pump appends to the same
    /// ring the ws catch-up replays.
    pub fn terminals(&self) -> crate::term::TermRing {
        self.terminals.clone()
    }

    /// the interactive-terminal ordered command-log ring. The daemon hands this
    /// to the [`crate::term::TerminalSessions`] manager so each session's serial
    /// command consumer appends to the same ring the ws `term-cmd:<session>`
    /// catch-up replays.
    pub fn term_commands(&self) -> crate::term::TermCommandRing {
        self.term_commands.clone()
    }

    pub(crate) fn subscribe_blocks(&self) -> broadcast::Receiver<BlockWake> {
        self.blocks.subscribe()
    }

    fn tip(&self) -> Option<(u64, String)> {
        self.tip.read().expect("stream tip lock poisoned").clone()
    }
}

#[derive(Clone)]
pub struct LogRing {
    inner: Arc<Mutex<LogRingInner>>,
    watch: watch::Sender<u64>,
}

#[derive(Default)]
struct LogRingInner {
    next_seq: u64,
    floor_seq: u64,
    lines: VecDeque<(u64, String)>,
}

impl Default for LogRing {
    fn default() -> Self {
        let (watch, _) = watch::channel(0);
        Self {
            inner: Arc::new(Mutex::new(LogRingInner::default())),
            watch,
        }
    }
}

impl LogRing {
    pub fn push_line(&self, line: impl Into<String>) {
        let mut inner = self.inner.lock().expect("log ring lock poisoned");
        inner.next_seq += 1;
        let seq = inner.next_seq;
        inner.lines.push_back((seq, line.into()));
        while inner.lines.len() > LOG_RING_CAPACITY {
            if let Some((evicted, _)) = inner.lines.pop_front() {
                inner.floor_seq = evicted;
            }
        }
        drop(inner);
        let _ = self.watch.send(seq);
    }

    pub fn read_after(&self, seq: u64, budget: usize) -> (Vec<(u64, String)>, u64) {
        let inner = self.inner.lock().expect("log ring lock poisoned");
        let rows = inner
            .lines
            .iter()
            .filter(|(line_seq, _)| *line_seq > seq)
            .take(budget)
            .cloned()
            .collect();
        (rows, inner.floor_seq)
    }

    pub fn latest_seq(&self) -> u64 {
        *self.watch.borrow()
    }

    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.watch.subscribe()
    }
}

impl<'a> MakeWriter<'a> for LogRing {
    type Writer = LogRingWriter;

    fn make_writer(&'a self) -> Self::Writer {
        LogRingWriter {
            ring: self.clone(),
            buf: Vec::new(),
        }
    }
}

pub struct LogRingWriter {
    ring: LogRing,
    buf: Vec<u8>,
}

impl LogRingWriter {
    fn push_complete_lines(&mut self) {
        while let Some(pos) = self.buf.iter().position(|b| *b == b'\n') {
            let mut line = self.buf.drain(..=pos).collect::<Vec<_>>();
            if line.last() == Some(&b'\n') {
                line.pop();
            }
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            self.ring
                .push_line(String::from_utf8_lossy(&line).into_owned());
        }
    }

    fn flush_partial(&mut self) {
        self.push_complete_lines();
        if !self.buf.is_empty() {
            let line = std::mem::take(&mut self.buf);
            self.ring
                .push_line(String::from_utf8_lossy(&line).into_owned());
        }
    }
}

impl Write for LogRingWriter {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.buf.extend_from_slice(buf);
        self.push_complete_lines();
        Ok(buf.len())
    }

    fn flush(&mut self) -> IoResult<()> {
        self.flush_partial();
        Ok(())
    }
}

impl Drop for LogRingWriter {
    fn drop(&mut self) {
        self.flush_partial();
    }
}

#[derive(Clone)]
pub struct RunOutputRegistry {
    inner: Arc<Mutex<RunOutputInner>>,
    watch: watch::Sender<u64>,
    appends: broadcast::Sender<RunOutputEvent>,
}

/// One provider line as it entered this node's local registry. The node's
/// agent data-plane subscribes to this feed and forwards it to peer nodes;
/// remotely ingested lines deliberately do not re-enter the feed.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunOutputEvent {
    pub id: String,
    pub stream: RunStream,
    pub line: String,
}

#[derive(Default)]
struct RunOutputInner {
    version: u64,
    touch: u64,
    runs: BTreeMap<String, RunRing>,
}

#[derive(Default)]
struct RunRing {
    next_seq: u64,
    floor_seq: u64,
    touched: u64,
    lines: VecDeque<(u64, RunStream, String)>,
}

impl Default for RunOutputRegistry {
    fn default() -> Self {
        let (watch, _) = watch::channel(0);
        let (appends, _) = broadcast::channel(RUN_OUTPUT_MAX_LINES);
        Self {
            inner: Arc::new(Mutex::new(RunOutputInner::default())),
            watch,
            appends,
        }
    }
}

impl RunOutputRegistry {
    pub fn output_sink(&self) -> provider_host::OutputSink {
        let registry = self.clone();
        Arc::new(move |ctx, line| {
            let Some(run_key) = ctx.run_key.as_deref() else {
                return;
            };
            let stream = match line.stream {
                provider_host::OutputStream::Stdout => RunStream::Stdout,
                provider_host::OutputStream::Stderr => RunStream::Stderr,
            };
            registry.append(run_key, stream, line.line);
        })
    }

    pub fn append(&self, id: impl Into<String>, stream: RunStream, line: impl Into<String>) {
        self.push(id.into(), stream, line.into(), true);
    }

    /// Add a line received from another node without broadcasting it again.
    pub fn append_remote(&self, event: RunOutputEvent) {
        self.push(event.id, event.stream, event.line, false);
    }

    fn push(&self, id: String, stream: RunStream, line: String, publish: bool) {
        let mut inner = self.inner.lock().expect("run output lock poisoned");
        inner.version += 1;
        inner.touch += 1;
        let version = inner.version;
        let touch = inner.touch;
        let ring = inner.runs.entry(id.clone()).or_default();
        ring.touched = touch;
        ring.next_seq += 1;
        let seq = ring.next_seq;
        ring.lines.push_back((seq, stream, line.clone()));
        while ring.lines.len() > RUN_OUTPUT_MAX_LINES {
            if let Some((evicted, _, _)) = ring.lines.pop_front() {
                ring.floor_seq = evicted;
            }
        }
        while inner.runs.len() > RUN_OUTPUT_MAX_RUNS {
            let Some(victim) = inner
                .runs
                .iter()
                .filter(|(run_id, _)| *run_id != &id)
                .min_by_key(|(_, ring)| ring.touched)
                .map(|(run_id, _)| run_id.clone())
            else {
                break;
            };
            inner.runs.remove(&victim);
        }
        drop(inner);
        let _ = self.watch.send(version);
        if publish {
            let _ = self.appends.send(RunOutputEvent { id, stream, line });
        }
    }

    pub fn read_after(
        &self,
        id: &str,
        seq: u64,
        budget: usize,
    ) -> (Vec<(u64, RunStream, String)>, u64) {
        let mut inner = self.inner.lock().expect("run output lock poisoned");
        inner.touch += 1;
        let touch = inner.touch;
        let Some(ring) = inner.runs.get_mut(id) else {
            return (Vec::new(), 0);
        };
        ring.touched = touch;
        let rows = ring
            .lines
            .iter()
            .filter(|(line_seq, _, _)| *line_seq > seq)
            .take(budget)
            .cloned()
            .collect();
        (rows, ring.floor_seq)
    }

    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.watch.subscribe()
    }

    pub fn subscribe_appends(&self) -> broadcast::Receiver<RunOutputEvent> {
        self.appends.subscribe()
    }
}

#[derive(Clone, Debug)]
enum TopicState {
    Module {
        module: String,
        cursor: String,
    },
    FilesWatch {
        cursor: String,
    },
    Logs {
        seq: u64,
    },
    RunOutput {
        id: String,
        seq: u64,
    },
    /// an interactive terminal session's output stream (`term:<session>`).
    /// `seq` is the last emitted ring chunk — the same seq-cursor model as
    /// `RunOutput`.
    Term {
        session: String,
        seq: u64,
    },
    /// an interactive session's ordered command log (`term-cmd:<session>`).
    /// `seq` is the last emitted command — the same seq-cursor model as `Term`,
    /// against the command-log ring instead of the output ring.
    TermCommand {
        session: String,
        seq: u64,
    },
    /// a SNAPSHOT topic: each wakeup re-samples the whole exposition, so the
    /// cursor (the last sample's `time_ms`) is bookkeeping, never a resume
    /// point — there is no backlog to replay and the topic never lags.
    Metrics {
        sampled_ms: u64,
    },
}

impl TopicState {
    fn cursor(&self) -> String {
        match self {
            Self::Module { cursor, .. } | Self::FilesWatch { cursor } => cursor.clone(),
            Self::Logs { seq }
            | Self::RunOutput { seq, .. }
            | Self::Term { seq, .. }
            | Self::TermCommand { seq, .. } => seq.to_string(),
            Self::Metrics { sampled_ms } => sampled_ms.to_string(),
        }
    }
}

struct CatchUpResult {
    frames: Vec<ServerFrame>,
    drop_topic: bool,
}

impl CatchUpResult {
    fn keep(frames: Vec<ServerFrame>) -> Self {
        Self {
            frames,
            drop_topic: false,
        }
    }

    fn drop(frames: Vec<ServerFrame>) -> Self {
        Self {
            frames,
            drop_topic: true,
        }
    }
}

/// which wakeup source fired — each catch-up pass only visits the topic
/// classes that source can have fed, so a log-line storm never re-scans
/// module topics and a run-output append never touches the index. `All`
/// covers subscribe replay, where any topic may owe frames. `Tick` is the
/// heartbeat interval — the cadence of snapshot topics (metrics), which
/// re-sample on time rather than on any append.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Wake {
    Block,
    Logs,
    RunOutput,
    Term,
    TermCommand,
    Tick,
    All,
}

impl TopicState {
    fn wakes_on(&self, wake: Wake) -> bool {
        match wake {
            Wake::All => true,
            Wake::Block => matches!(self, Self::Module { .. } | Self::FilesWatch { .. }),
            Wake::Logs => matches!(self, Self::Logs { .. }),
            Wake::RunOutput => matches!(self, Self::RunOutput { .. }),
            Wake::Term => matches!(self, Self::Term { .. }),
            Wake::TermCommand => matches!(self, Self::TermCommand { .. }),
            Wake::Tick => matches!(self, Self::Metrics { .. }),
        }
    }
}

pub async fn stream_session(mut socket: WebSocket, handle: NodeHandle) {
    let hub = handle.stream_hub();
    let mut block_rx = hub.subscribe_blocks();
    let mut log_rx = hub.log_ring().subscribe();
    let mut run_rx = hub.run_output().subscribe();
    let mut term_rx = hub.terminals().subscribe();
    let mut term_cmd_rx = hub.term_commands().subscribe();
    let mut heartbeat = tokio::time::interval(Duration::from_millis(HEARTBEAT_INTERVAL_MS));
    let mut index_backstop = tokio::time::interval(INDEX_BACKSTOP_INTERVAL);
    let mut topics = BTreeMap::new();
    // set once, by a `ServiceAttach` that this node accepts. The guard's Drop —
    // on every `return` below, and on the task being cancelled — releases the
    // link and ends every session the daemon was serving, so a client attached
    // to a dead session's topic is told rather than left blocked.
    let mut attached: Option<crate::term::AttachGuard> = None;
    let mut service_rx: Option<mpsc::Receiver<agent_service::wire::Command>> = None;

    loop {
        tokio::select! {
            frame = socket.next() => {
                let Some(frame) = frame else { return };
                match frame {
                    Ok(Message::Text(text)) => {
                        match serde_json::from_str::<ClientMsg>(text.as_str()) {
                            // terminal input/resize act on the session manager,
                            // not on this connection's topic set — and the write
                            // is async — so they're handled here, before the
                            // sync topic path.
                            Ok(ClientMsg::TermInput { session, data }) => {
                                handle_term_input(&handle, &topics, &session, &data).await;
                            }
                            Ok(ClientMsg::TermResize { session, cols, rows }) => {
                                handle_term_resize(&handle, &topics, &session, cols, rows).await;
                            }
                            Ok(ClientMsg::TermCommand { session, text, origin }) => {
                                handle_term_command(&handle, &topics, &session, origin, text);
                            }
                            // a compute daemon's live run tail: append to the
                            // same ring the in-process sink used to feed, so
                            // `run-output:<id>` subscribers cannot tell which
                            // process produced the line.
                            Ok(ClientMsg::RunOutput { id, stream, line }) => {
                                handle_run_output(&hub, id, stream, line);
                            }
                            // a service daemon claiming this connection as its
                            // command link, and the events it publishes back.
                            Ok(ClientMsg::ServiceAttach { kind, token }) => {
                                match take_service_link(&handle, &kind, &token) {
                                    Ok((guard, rx)) => {
                                        attached = Some(guard);
                                        service_rx = Some(rx);
                                    }
                                    Err(reason) => {
                                        if !send_frame(&mut socket, ServerFrame::Error {
                                            topic: String::new(),
                                            code: StreamErrorCode::Unavailable,
                                            detail: reason.to_string(),
                                        }).await {
                                            return;
                                        }
                                    }
                                }
                            }
                            Ok(ClientMsg::AgentEvent { event }) => {
                                handle_agent_event(&handle, attached.is_some(), event);
                            }
                            Ok(msg) => {
                                let frames = handle_client_msg(&handle, &mut topics, msg);
                                if !send_frames(&mut socket, frames).await {
                                    return;
                                }
                                if !catch_up(&handle, &mut socket, &mut topics, Wake::All).await {
                                    return;
                                }
                            }
                            Err(err) => {
                                if !send_frame(&mut socket, ServerFrame::Error {
                                    topic: String::new(),
                                    code: StreamErrorCode::BadFrame,
                                    detail: err.to_string(),
                                }).await {
                                    return;
                                }
                            }
                        }
                    }
                    Ok(Message::Binary(_)) | Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {}
                    Ok(Message::Close(_)) | Err(_) => return,
                }
            }
            note = block_rx.recv() => {
                let sweep = match block_action(note) {
                    BlockAction::Stop => return,
                    BlockAction::SweepIndex => true,
                    BlockAction::TipOnly => false,
                };
                // the tip rides every block wake — nop fillers included, which
                // feed no topic — so a console's height ticks per block instead
                // of waiting out the timer beat below (the idle/stall floor).
                // NOT gated on `sweep`: gating it here re-freezes the head on an
                // idle chain, which is the bug #1021 fixed.
                if !send_frame(&mut socket, heartbeat_frame(&hub)).await {
                    return;
                }
                // An idle block appended no op row, so every scan it used to
                // trigger read the store and found nothing.
                if sweep && !catch_up(&handle, &mut socket, &mut topics, Wake::Block).await {
                    return;
                }
            }
            _ = index_backstop.tick() => {
                // see `INDEX_BACKSTOP_INTERVAL`: the floor under every writer
                // that appends rows and tells nobody.
                if !catch_up(&handle, &mut socket, &mut topics, Wake::Block).await {
                    return;
                }
            }
            _ = heartbeat.tick() => {
                if !send_frame(&mut socket, heartbeat_frame(&hub)).await {
                    return;
                }
                if !catch_up(&handle, &mut socket, &mut topics, Wake::Tick).await {
                    return;
                }
            }
            changed = log_rx.changed() => {
                if changed.is_err() {
                    return;
                }
                if !catch_up(&handle, &mut socket, &mut topics, Wake::Logs).await {
                    return;
                }
            }
            changed = run_rx.changed() => {
                if changed.is_err() {
                    return;
                }
                if !catch_up(&handle, &mut socket, &mut topics, Wake::RunOutput).await {
                    return;
                }
            }
            changed = term_rx.changed() => {
                if changed.is_err() {
                    return;
                }
                if !catch_up(&handle, &mut socket, &mut topics, Wake::Term).await {
                    return;
                }
            }
            changed = term_cmd_rx.changed() => {
                if changed.is_err() {
                    return;
                }
                if !catch_up(&handle, &mut socket, &mut topics, Wake::TermCommand).await {
                    return;
                }
            }
            // the service link's outbound half. Inert on every connection that
            // is not the attached daemon (see `next_service_command`).
            //
            // BOUNDED, and it is the only write here that is: every other frame
            // on this loop goes to a subscriber, but this one goes to another
            // PROCESS that is simultaneously writing events back. If the daemon
            // ever stopped reading, an unbounded await here would stop this loop
            // reading its events, and the two blocked writes would deadlock with
            // nothing to break them — taking the whole interactive plane with
            // them, permanently. A daemon that cannot accept a command in this
            // long is wedged; dropping the link ends its sessions cleanly
            // (`AttachGuard`) and lets it redial.
            command = next_service_command(&mut service_rx) => {
                let sent = tokio::time::timeout(
                    SERVICE_COMMAND_WRITE_TIMEOUT,
                    send_frame(&mut socket, ServerFrame::ServiceCommand { command }),
                )
                .await;
                let Ok(true) = sent else {
                    tracing::warn!(
                        target: "ducktape::service",
                        reason = "service_link_write_stalled",
                        "dropping the agent service link"
                    );
                    return;
                };
            }
        }
    }
}

/// the attached daemon's next command, or never.
///
/// A connection that holds no service link must not make this arm ready — it
/// would spin the select loop — so it parks forever instead. Same for a link
/// whose sender the bridge has already dropped: the guard tidies up when this
/// socket closes, and until then there is nothing to send.
async fn next_service_command(
    rx: &mut Option<mpsc::Receiver<agent_service::wire::Command>>,
) -> agent_service::wire::Command {
    let Some(rx) = rx else {
        return std::future::pending().await;
    };
    match rx.recv().await {
        Some(command) => command,
        None => std::future::pending().await,
    }
}

/// Admit a service daemon's claim on this connection, or name why not.
///
/// Two refusals, each a stable reason an operator can act on: a kind this node
/// hosts no plane for, and a link another daemon already holds.
///
/// Build equality is NOT one of them. It authenticated nobody (a stamp is
/// compiled into a binary any local process can read), it excluded every
/// separately-compiled daemon by construction, and its `None` case refused
/// every link on any build without `.git`.
///
/// Skew is caught per FRAME instead, and that is a claim with an enforcement
/// site rather than a hope: [`ClientMsg`] and every `agent_service::wire` type
/// carry `deny_unknown_fields` and default nothing, so a field this build does
/// not know is refused by name and a field it does know cannot go missing. A
/// connection-wide version check would refuse frames this node understands
/// perfectly; the per-frame check refuses exactly the ones it does not.
///
/// The strictness is ONE-DIRECTIONAL, and saying otherwise would be the same
/// overclaim the deleted gate's justification made. Daemon→node is the clean
/// half: an undecodable frame earns this connection a `BadFrame` naming the
/// field and the socket stays open. Node→daemon only DROPS — the daemon warns
/// `malformed_command` and the node's create waits on a reply that never
/// arrives. That gap and its fix live at the drop site,
/// `bin/node/src/agent/link.rs`'s `classify`.
fn take_service_link(
    handle: &NodeHandle,
    kind: &str,
    token: &str,
) -> Result<
    (
        crate::term::AttachGuard,
        mpsc::Receiver<agent_service::wire::Command>,
    ),
    &'static str,
> {
    if kind != crate::services::AGENT_KIND {
        return Err("only the agent service has a command link on this node");
    }
    let terminals = handle
        .terminals()
        .ok_or("terminal sessions are not enabled on this node")?;
    terminals
        .attach(token)
        .ok_or("refused: present this node's service-link token, and only one agent service may attach")
}

/// Apply one daemon-published event to the terminal plane, or drop it.
///
/// The `attached` gate is a trust boundary, not tidiness: these events append to
/// scrollback rings and terminate sessions, so an unattached connection
/// publishing one would be injecting into another member's terminal.
fn handle_agent_event(handle: &NodeHandle, attached: bool, event: agent_service::wire::Event) {
    if !attached {
        tracing::warn!(
            target: "ducktape::term",
            reason = "unattached_publisher",
            "agent event dropped"
        );
        return;
    }
    let Some(terminals) = handle.terminals() else {
        tracing::warn!(
            target: "ducktape::term",
            reason = "no_terminal_plane",
            "agent event dropped"
        );
        return;
    };
    terminals.on_event(event);
}

fn handle_client_msg(
    handle: &NodeHandle,
    topics: &mut BTreeMap<String, TopicState>,
    msg: ClientMsg,
) -> Vec<ServerFrame> {
    match msg {
        ClientMsg::Subscribe {
            topics: requested,
            resume,
            token,
        } => subscribe_topics(handle, topics, requested, &resume, token.as_deref()),
        ClientMsg::Unsubscribe { topics: requested } => {
            for topic in requested {
                topics.remove(&topic);
            }
            Vec::new()
        }
        // handled inline in `stream_session` (they act on the session manager,
        // off this connection's topic set), so they never reach here — but the
        // match stays exhaustive.
        ClientMsg::TermInput { .. }
        | ClientMsg::TermResize { .. }
        | ClientMsg::TermCommand { .. }
        | ClientMsg::RunOutput { .. }
        | ClientMsg::ServiceAttach { .. }
        | ClientMsg::AgentEvent { .. } => Vec::new(),
    }
}

/// Admit one published run-output line, or drop it with a named reason.
///
/// The two checks are a trust boundary, not tidiness: see [`ClientMsg::RunOutput`].
/// Dropping is deliberate — a malformed line is not worth closing an otherwise
/// healthy publisher's connection over, and the `warn` carries the counter.
fn handle_run_output(hub: &StreamHub, id: String, stream: RunStream, line: String) {
    let id_well_formed =
        id.len() == RUN_OUTPUT_ID_LEN && id.bytes().all(|byte| byte.is_ascii_hexdigit());
    if !id_well_formed {
        tracing::warn!(
            target: "ducktape::agent",
            reason = "malformed_run_id",
            "run output dropped"
        );
        return;
    }
    if line.len() > MAX_RUN_OUTPUT_LINE {
        tracing::warn!(
            target: "ducktape::agent",
            bytes = line.len(),
            reason = "run_output_line_too_long",
            "run output dropped"
        );
        return;
    }
    hub.run_output().append(id, stream, line);
}

/// Does this connection hold an ADMITTED handle on `session`'s pty?
///
/// The handle IS the capability, and that is no longer circular. Its predecessor
/// (`term_entitled`) asked `topics.contains_key("term:<id>")` while subscribing
/// was unconditional, so any connection self-granted pty write access by
/// subscribing first — the check answered "are you subscribed?" as a proxy for
/// "are you allowed?", and nothing gated the subscribe. Admission now happens at
/// the subscribe ([`Topic::admission`]: `term:<session>` is
/// [`Admission::Workspace`]), so a connection that holds this handle has already
/// proved it can read this node's own workspace — the same proof
/// [`take_service_link`] makes the agent daemon give.
///
/// The state VARIANT is part of the answer, not just the key: nothing but an
/// admitted `term:` subscription may drive a pty, whatever else is on the
/// connection.
fn holds_session(topics: &BTreeMap<String, TopicState>, session: &str) -> bool {
    matches!(
        topics.get(&crate::term::topic(session)),
        Some(TopicState::Term { .. })
    )
}

/// the host node a session's input must be forwarded to, or `None` for a local
/// session (write to this node's pty). Just the guest-side registry lookup.
fn forward_target(handle: &NodeHandle, session: &str) -> Option<[u8; 32]> {
    handle.remote_sessions().host_of(session)
}

/// forward one input event to the session's host over the guest lane. A missing
/// lane or a full channel drops the frame (never a panic); never logs the bytes.
async fn forward_input(handle: &NodeHandle, host: [u8; 32], event: crate::SessionInputWire) {
    let Some(lane) = handle.session_lane() else {
        tracing::warn!(target: "ducktape::term", reason = "no_session_lane", "term input dropped");
        return;
    };
    if lane
        .send(crate::SessionJob::Input { host, event })
        .await
        .is_err()
    {
        tracing::warn!(target: "ducktape::term", reason = "input_forward_failed", "term input dropped");
    }
}

/// write base64-decoded keystrokes to a session's pty. Refused (no-op + a log
/// line) when the connection holds no admitted handle on the session
/// (`unadmitted_session`), the terminal plane is absent, the session is unknown,
/// or the base64 is bad — never a panic. Never logs the bytes; the refusal logs
/// no id (an id the caller was not admitted to is not the node's to echo into
/// the log ring the app streams).
///
/// `unadmitted_session` is `debug`, not `warn`, and for the reason
/// [`refuse_topic`] already gives: it is PER-KEYSTROKE. An unadmitted client
/// held-down key would otherwise mint one `warn` per repeat into the 4096-line
/// ring — evicting the very context an operator opened the Logs tab to read,
/// and doing it through the `logs` topic any ws caller may hold. The other three
/// reasons here stay `warn`: each is once per frame class, not once per byte.
async fn handle_term_input(
    handle: &NodeHandle,
    topics: &BTreeMap<String, TopicState>,
    session: &str,
    data_b64: &str,
) {
    if !holds_session(topics, session) {
        tracing::debug!(target: "ducktape::term", reason = "unadmitted_session", "term input dropped");
        return;
    }
    // a remote session lives on a host peer — forward the keystrokes there rather
    // than writing a (nonexistent) local pty.
    if let Some(host) = forward_target(handle, session) {
        forward_input(
            handle,
            host,
            crate::SessionInputWire::Input {
                session: session.to_string(),
                data_b64: data_b64.to_string(),
            },
        )
        .await;
        return;
    }
    let Some(terminals) = handle.terminals() else {
        tracing::warn!(target: "ducktape::term", reason = "no_terminal_plane", "term input dropped");
        return;
    };
    // a live session has a mode; an unknown or already-ended one has none. Two
    // causes, two countable reasons — collapsing them would hide "the id is
    // stale" behind "you used the wrong lane".
    let Some(mode) = terminals.mode(session) else {
        tracing::warn!(target: "ducktape::term", session = %session, reason = "unknown_session", "term input dropped");
        return;
    };
    // raw keystrokes are the SINGLE-session path only. A shared session refuses
    // them so nothing bypasses its ordered command lane (drive it with
    // TermCommand).
    if mode != crate::term::SessionMode::Single {
        tracing::warn!(target: "ducktape::term", session = %session, reason = "raw_input_on_shared", "term input dropped");
        return;
    }
    // decoded here purely to refuse a malformed frame at this boundary; the
    // daemon takes the base64 as-is, so the bytes never round-trip.
    if STANDARD.decode(data_b64).is_err() {
        tracing::warn!(target: "ducktape::term", session = %session, reason = "bad_base64", "term input dropped");
        return;
    }
    terminals.input(session, data_b64).await;
}

/// resize a session's pty. Same admission gate + no-op-on-unknown discipline
/// as input.
async fn handle_term_resize(
    handle: &NodeHandle,
    topics: &BTreeMap<String, TopicState>,
    session: &str,
    cols: u16,
    rows: u16,
) {
    if !holds_session(topics, session) {
        tracing::debug!(target: "ducktape::term", reason = "unadmitted_session", "term resize dropped");
        return;
    }
    // a remote session's window change forwards to its host.
    if let Some(host) = forward_target(handle, session) {
        forward_input(
            handle,
            host,
            crate::SessionInputWire::Resize {
                session: session.to_string(),
                cols,
                rows,
            },
        )
        .await;
        return;
    }
    let Some(terminals) = handle.terminals() else {
        tracing::warn!(target: "ducktape::term", reason = "no_terminal_plane", "term resize dropped");
        return;
    };
    // the same no-op-on-unknown discipline as input: refuse here rather than
    // spending a link frame on a session that is already gone.
    if terminals.mode(session).is_none() {
        tracing::warn!(target: "ducktape::term", session = %session, reason = "unknown_session", "term resize dropped");
        return;
    }
    terminals.resize(session, cols, rows).await;
}

/// enqueue a submitted COMMAND onto a session's ordered command lane (the
/// `CommandSource` seam). Gated exactly like [`handle_term_input`]: the
/// connection must hold an ADMITTED handle on the session's `term:<id>` output
/// topic ([`holds_session`]). Refused (no-op + `warn`) when it does not, or the
/// terminal plane is absent; an unknown session id is warned inside
/// `enqueue_command`. Never logs the command text (it can carry secrets); the
/// serial consumer assigns the total order and feeds the pty.
fn handle_term_command(
    handle: &NodeHandle,
    topics: &BTreeMap<String, TopicState>,
    session: &str,
    origin: String,
    text: String,
) {
    if !holds_session(topics, session) {
        tracing::debug!(target: "ducktape::term", reason = "unadmitted_session", "term command dropped");
        return;
    }
    let Some(terminals) = handle.terminals() else {
        tracing::warn!(target: "ducktape::term", reason = "no_terminal_plane", "term command dropped");
        return;
    };
    terminals.enqueue_command(session, origin, text);
}

fn subscribe_topics(
    handle: &NodeHandle,
    states: &mut BTreeMap<String, TopicState>,
    requested: Vec<String>,
    resume: &BTreeMap<String, String>,
    token: Option<&str>,
) -> Vec<ServerFrame> {
    let store = handle.stream_index();
    // ONE constant-time compare per frame, not per topic: the secret is
    // connection-wide, so this is both the cheapest place to spend it and the
    // only place the presented bytes are touched at all.
    let holds_workspace_secret = token.is_some_and(|token| handle.workspace_secret_matches(token));
    let mut frames = Vec::new();
    let mut accepted = BTreeMap::new();
    for topic in requested {
        // the cap counts a NEW topic only — re-subscribing (re-cursoring) an
        // existing one is always allowed.
        if !states.contains_key(&topic) && states.len() >= MAX_TOPICS_PER_CONNECTION {
            frames.push(unavailable(
                &topic,
                format!("subscription cap ({MAX_TOPICS_PER_CONNECTION} topics) reached"),
            ));
            continue;
        }
        match prepare_topic(&topic, holds_workspace_secret, resume.get(&topic), store.as_ref()) {
            Ok((state, lagged)) => {
                accepted.insert(topic.clone(), state.cursor());
                states.insert(topic, state);
                if let Some(frame) = lagged {
                    frames.push(frame);
                }
            }
            Err(frame) => frames.push(frame),
        }
    }
    frames.push(ServerFrame::Subscribed { topics: accepted });
    frames
}

/// the `files` module's live-change topic, spelled once.
const FILES_WATCH_TOPIC: &str = "files:watch";
/// the log-ring tail topic.
const LOGS_TOPIC: &str = "logs";
/// the metrics-exposition snapshot topic.
const METRICS_TOPIC: &str = "metrics";
const MODULE_PREFIX: &str = "module:";
const RUN_OUTPUT_PREFIX: &str = "run-output:";
/// checked before [`TERM_PREFIX`] for readability only — the two diverge at the
/// fifth byte (`-` vs `:`), so neither is a prefix of the other.
const TERM_COMMAND_PREFIX: &str = "term-cmd:";
const TERM_PREFIX: &str = "term:";

/// every topic family this node serves, parsed from the wire name exactly once.
///
/// ONE tagged value so admission is ONE `match` with no `_` arm ([`Self::admission`]):
/// a family added later cannot compile until that match names it, which is what
/// makes "deny by default" a build error rather than a habit. The prefix ladder
/// in [`Self::parse`] is not the decision — it is the parse, and a `&str` is not
/// a closed set; every decision downstream of it branches on this enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Topic<'a> {
    /// every committed op of one indexed module, decoded.
    Module(&'a str),
    /// the same, pinned to `files` and projected as path changes.
    FilesWatch,
    /// this node's 4096-line log ring.
    Logs,
    /// one run's stdout/stderr tail.
    RunOutput(&'a str),
    /// one interactive session's ordered, attributed command log.
    TermCommand(&'a str),
    /// one interactive session's raw pty bytes, local or remote-hosted.
    Term(&'a str),
    /// the Prometheus exposition, re-sampled per heartbeat.
    Metrics,
}

/// what a caller must have proved to hold a topic handle.
///
/// Two values and no more: the ws surface has exactly one piece of evidence
/// about a caller — whether it can read this node's workspace — so a richer
/// lattice would be names without a mechanism behind them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Admission {
    /// nothing. The same bytes already leave this node over an HTTP route with
    /// no gate on it, so a check here would refuse an honest client and stop
    /// nobody.
    Public,
    /// this node's own 0600 workspace secret ([`crate::services::LINK_TOKEN_FILE`]).
    Workspace,
}

impl<'a> Topic<'a> {
    /// Parse a wire topic name, or `None` for a name no family owns.
    ///
    /// `None` is a refusal, not a fallthrough: an unparsed name reaches no
    /// `TopicState` and so hands the connection nothing.
    fn parse(name: &'a str) -> Option<Self> {
        if let Some(module) = name.strip_prefix(MODULE_PREFIX) {
            return Some(Self::Module(module));
        }
        if let Some(id) = name.strip_prefix(RUN_OUTPUT_PREFIX) {
            return Some(Self::RunOutput(id));
        }
        if let Some(session) = name.strip_prefix(TERM_COMMAND_PREFIX) {
            return Some(Self::TermCommand(session));
        }
        if let Some(session) = name.strip_prefix(TERM_PREFIX) {
            return Some(Self::Term(session));
        }
        match name {
            FILES_WATCH_TOPIC => Some(Self::FilesWatch),
            LOGS_TOPIC => Some(Self::Logs),
            METRICS_TOPIC => Some(Self::Metrics),
            _ => None,
        }
    }

    /// What this family costs to hold. The whole authorization decision, in one
    /// place, with every family named.
    ///
    /// The public three are public because gating them would be theater: an
    /// `Origin`-less caller already reads the identical bytes over
    /// `POST /v1/query` + `GET /v1/index/{module}/{ops,scan}` (`Module`,
    /// `FilesWatch`) and `GET /metrics` (`Metrics`), neither of which this
    /// change touches. `Logs` is public for a different reason and a weaker one,
    /// named honestly: the ring is the app's Logs tab, the app reaches this node
    /// by URL with no workspace handle to read a secret from, and the logging
    /// doctrine already forbids a token, a URI or key material from ever
    /// entering it. Its admin twin (`GET /v1/admin/logs/tail`) IS gated, so the
    /// asymmetry is real and survives this change deliberately rather than
    /// silently.
    ///
    /// The gated three all carry provider/member bytes with no unauthenticated
    /// HTTP twin at all: a pty's raw output, the command log whose `text`
    /// `crate::term` documents as able to carry secrets, and a run's stdout.
    fn admission(&self) -> Admission {
        match self {
            Self::Module(_) => Admission::Public,
            Self::FilesWatch => Admission::Public,
            Self::Logs => Admission::Public,
            Self::Metrics => Admission::Public,
            Self::RunOutput(_) => Admission::Workspace,
            Self::TermCommand(_) => Admission::Workspace,
            Self::Term(_) => Admission::Workspace,
        }
    }
}

/// Why a subscribe was refused. Typed, mirroring [`crate::services::HelloRefusal`]:
/// the stable snake_case `reason` and the wire code are derived from the
/// variant, so a typo cannot silently downgrade a refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TopicRefusal {
    /// no family owns this name.
    UnknownFamily,
    /// the family is real but names a module this node does not index. A
    /// separate variant from [`Self::UnknownFamily`] because the two send an
    /// operator to different places — a typo in the topic grammar, versus a
    /// module absent from THIS node's genesis set — and one token covering both
    /// would be uncountable.
    UnknownModule,
    /// the family is workspace-gated and no matching secret was presented.
    NotAdmitted,
}

impl TopicRefusal {
    /// the stable snake_case token — greppable, countable, never prose.
    fn reason(self) -> &'static str {
        match self {
            Self::UnknownFamily => "unknown_topic",
            Self::UnknownModule => "unknown_module",
            Self::NotAdmitted => "topic_not_admitted",
        }
    }

    fn code(self) -> StreamErrorCode {
        match self {
            Self::UnknownFamily | Self::UnknownModule => StreamErrorCode::UnknownTopic,
            Self::NotAdmitted => StreamErrorCode::Forbidden,
        }
    }

    /// the caller-facing sentence. It names what the caller must PRESENT and
    /// never what this node EXPECTS: echoing the secret into a refusal body is
    /// a bug this repo has already shipped once.
    ///
    /// `&'static str` is that guarantee, structurally — there is no formatting
    /// site here for a secret to reach.
    fn detail(self) -> &'static str {
        match self {
            Self::UnknownFamily => "unknown stream topic",
            Self::UnknownModule => "this node indexes no such module",
            Self::NotAdmitted => {
                "this topic requires the node's service-link token — read it from \
                 the workspace and send it as `token` on the subscribe"
            }
        }
    }
}

/// Refuse one topic: the wire frame back to the caller, and one `debug` line.
///
/// `debug`, not `warn`, and for the reason `crate::admin`'s `refuse` already
/// documents: a refusal is per-request and any local process can drive one in a
/// loop, so an unconditional `warn!` is a log-ring DoS that evicts the evidence
/// around whatever you were hunting. The topic NAME never reaches the log — it
/// carries a session id — while the frame does, because that is the caller's own
/// input going back to the caller.
fn refuse_topic(topic: &str, refusal: TopicRefusal) -> ServerFrame {
    tracing::debug!(
        target: "ducktape::stream",
        reason = refusal.reason(),
        "topic subscribe refused"
    );
    ServerFrame::Error {
        topic: topic.to_string(),
        code: refusal.code(),
        detail: refusal.detail().into(),
    }
}

/// Decide one requested topic: admit it (with its start cursor) or refuse it.
///
/// A decide-fn as far as STATE goes — it inserts no handle, mutates nothing, and
/// the caller applies the result. It is not effect-free: [`refuse_topic`] emits
/// one `debug` line, deliberately kept beside the decision so a refusal cannot
/// be returned without being counted.
///
/// `holds_workspace_secret` is the connection's ONE proved fact, compared once
/// per subscribe frame by [`subscribe_topics`].
#[allow(clippy::result_large_err)]
fn prepare_topic(
    topic: &str,
    holds_workspace_secret: bool,
    resume: Option<&String>,
    store: Option<&Arc<indexer::IndexStore>>,
) -> Result<(TopicState, Option<ServerFrame>), ServerFrame> {
    let Some(family) = Topic::parse(topic) else {
        return Err(refuse_topic(topic, TopicRefusal::UnknownFamily));
    };
    let admitted = match family.admission() {
        Admission::Public => true,
        Admission::Workspace => holds_workspace_secret,
    };
    if !admitted {
        return Err(refuse_topic(topic, TopicRefusal::NotAdmitted));
    }
    match family {
        Topic::Module(module) => prepare_module(topic, module, resume, store),
        Topic::FilesWatch => prepare_files_watch(topic, resume, store),
        Topic::Logs => prepare_logs(topic, resume),
        Topic::RunOutput(id) => prepare_run_output(topic, id, resume),
        Topic::TermCommand(session) => prepare_term_command(topic, session, resume),
        Topic::Term(session) => prepare_term(topic, session, resume),
        Topic::Metrics => prepare_metrics(),
    }
}

#[allow(clippy::result_large_err)]
fn prepare_module(
    topic: &str,
    module: &str,
    resume: Option<&String>,
    store: Option<&Arc<indexer::IndexStore>>,
) -> Result<(TopicState, Option<ServerFrame>), ServerFrame> {
    let store = store.ok_or_else(|| unavailable(topic, "no index store configured"))?;
    if !store.module_ids().any(|id| id == module) {
        return Err(refuse_topic(topic, TopicRefusal::UnknownModule));
    }
    let (cursor, lagged) = module_start_cursor(topic, module, resume, store)?;
    Ok((
        TopicState::Module {
            module: module.to_string(),
            cursor,
        },
        lagged,
    ))
}

#[allow(clippy::result_large_err)]
fn prepare_files_watch(
    topic: &str,
    resume: Option<&String>,
    store: Option<&Arc<indexer::IndexStore>>,
) -> Result<(TopicState, Option<ServerFrame>), ServerFrame> {
    let store = store.ok_or_else(|| unavailable(topic, "no index store configured"))?;
    if !store.module_ids().any(|id| id == "files") {
        return Err(refuse_topic(topic, TopicRefusal::UnknownModule));
    }
    let (cursor, lagged) = module_start_cursor(topic, "files", resume, store)?;
    Ok((TopicState::FilesWatch { cursor }, lagged))
}

#[allow(clippy::result_large_err)]
fn prepare_logs(
    topic: &str,
    resume: Option<&String>,
) -> Result<(TopicState, Option<ServerFrame>), ServerFrame> {
    Ok((
        TopicState::Logs {
            seq: start_seq(topic, resume)?,
        },
        None,
    ))
}

#[allow(clippy::result_large_err)]
fn prepare_run_output(
    topic: &str,
    id: &str,
    resume: Option<&String>,
) -> Result<(TopicState, Option<ServerFrame>), ServerFrame> {
    Ok((
        TopicState::RunOutput {
            id: id.to_string(),
            seq: start_seq(topic, resume)?,
        },
        None,
    ))
}

/// the ordered command log — like `term:`, any session id subscribes once the
/// caller is admitted (unknown/evicted → empty catch-up, never an error).
#[allow(clippy::result_large_err)]
fn prepare_term_command(
    topic: &str,
    session: &str,
    resume: Option<&String>,
) -> Result<(TopicState, Option<ServerFrame>), ServerFrame> {
    Ok((
        TopicState::TermCommand {
            session: session.to_string(),
            seq: start_seq(topic, resume)?,
        },
        None,
    ))
}

/// like run-output, any session id subscribes once the caller is admitted
/// (unknown/evicted → empty catch-up, never an error); the manager gates who may
/// CREATE one.
#[allow(clippy::result_large_err)]
fn prepare_term(
    topic: &str,
    session: &str,
    resume: Option<&String>,
) -> Result<(TopicState, Option<ServerFrame>), ServerFrame> {
    Ok((
        TopicState::Term {
            session: session.to_string(),
            seq: start_seq(topic, resume)?,
        },
        None,
    ))
}

/// a resume cursor is accepted but meaningless for a snapshot topic: every
/// (re)subscribe starts from a fresh sample, never a replay.
#[allow(clippy::result_large_err)]
fn prepare_metrics() -> Result<(TopicState, Option<ServerFrame>), ServerFrame> {
    Ok((TopicState::Metrics { sampled_ms: 0 }, None))
}

/// the seq a ring-backed topic starts from: the caller's resume cursor, or the
/// bottom of the ring.
#[allow(clippy::result_large_err)]
fn start_seq(topic: &str, resume: Option<&String>) -> Result<u64, ServerFrame> {
    match resume {
        Some(cursor) => parse_seq_cursor(topic, cursor),
        None => Ok(0),
    }
}

#[allow(clippy::result_large_err)]
fn module_start_cursor(
    topic: &str,
    module: &str,
    resume: Option<&String>,
    store: &indexer::IndexStore,
) -> Result<(String, Option<ServerFrame>), ServerFrame> {
    let Some(cursor) = resume else {
        return live_cursor(store, module)
            .map(|cursor| (cursor, None))
            .map_err(|err| unavailable(topic, err.to_string()));
    };
    if !cursor.starts_with(indexer::OP_PREFIX) || cursor_height(cursor).is_none() {
        return Err(ServerFrame::Error {
            topic: topic.to_string(),
            code: StreamErrorCode::BadCursor,
            detail: "cursor must be an op/{height}/{seq} key".into(),
        });
    }
    let height = cursor_height(cursor).expect("checked above");
    match store.backfill_height(module) {
        Ok(Some(floor)) if height < floor => {
            let jump =
                live_cursor(store, module).map_err(|err| unavailable(topic, err.to_string()))?;
            Ok((
                jump.clone(),
                Some(ServerFrame::Lagged {
                    topic: topic.to_string(),
                    cursor: jump,
                }),
            ))
        }
        Ok(_) => Ok((cursor.clone(), None)),
        Err(err) => Err(unavailable(topic, err.to_string())),
    }
}

async fn catch_up(
    handle: &NodeHandle,
    socket: &mut WebSocket,
    topics: &mut BTreeMap<String, TopicState>,
    wake: Wake,
) -> bool {
    let store = handle.stream_index();
    let hub = handle.stream_hub();
    let topic_names = topics
        .iter()
        .filter(|(_, state)| state.wakes_on(wake))
        .map(|(topic, _)| topic.clone())
        .collect::<Vec<_>>();
    for topic in topic_names {
        let Some(state) = topics.get_mut(&topic) else {
            continue;
        };
        // metrics is the one topic whose catch-up crosses the actor command
        // lane (an await); every cursor-scan topic stays on the sync path.
        let result = match state {
            TopicState::Metrics { .. } => catch_up_metrics(&topic, state, handle).await,
            _ => catch_up_topic(&topic, state, store.as_ref(), &hub),
        };
        if !send_frames(socket, result.frames).await {
            return false;
        }
        if result.drop_topic {
            topics.remove(&topic);
        }
    }
    true
}

fn catch_up_topic(
    topic: &str,
    state: &mut TopicState,
    store: Option<&Arc<indexer::IndexStore>>,
    hub: &StreamHub,
) -> CatchUpResult {
    match state {
        TopicState::Module { module, cursor } => {
            let Some(store) = store else {
                return CatchUpResult::drop(vec![unavailable(topic, "no index store configured")]);
            };
            catch_up_module(topic, module, cursor, store)
        }
        TopicState::FilesWatch { cursor } => {
            let Some(store) = store else {
                return CatchUpResult::drop(vec![unavailable(topic, "no index store configured")]);
            };
            catch_up_files(topic, cursor, store)
        }
        TopicState::Logs { seq } => catch_up_logs(topic, seq, &hub.log_ring()),
        TopicState::RunOutput { id, seq } => catch_up_run_output(topic, id, seq, &hub.run_output()),
        TopicState::Term { session, seq } => catch_up_term(topic, session, seq, &hub.terminals()),
        TopicState::TermCommand { session, seq } => {
            catch_up_term_command(topic, session, seq, &hub.term_commands())
        }
        // routed to catch_up_metrics by the caller (it needs the actor lane,
        // an await this sync path cannot make) — nothing owed here.
        TopicState::Metrics { .. } => CatchUpResult::keep(Vec::new()),
    }
}

fn catch_up_module(
    topic: &str,
    module: &str,
    cursor: &mut String,
    store: &indexer::IndexStore,
) -> CatchUpResult {
    if let Some(frame) = lag_if_below_backfill(topic, module, cursor, store) {
        return frame;
    }

    let mut frames = Vec::new();
    let mut emitted = 0usize;
    loop {
        let remaining = STREAM_CATCHUP_BUDGET.saturating_sub(emitted);
        if remaining == 0 {
            return lag_to_live(topic, module, cursor, store, frames);
        }
        let page = match store.scan(
            module,
            indexer::OP_PREFIX.as_bytes(),
            Some(cursor.as_bytes()),
            remaining,
        ) {
            Ok(page) => page,
            Err(err) => {
                frames.push(unavailable(topic, err.to_string()));
                return CatchUpResult::drop(frames);
            }
        };
        let entry_count = page.entries.len();
        for (key, value) in page.entries {
            let key = String::from_utf8_lossy(&key).into_owned();
            let op = match borsh::from_slice::<indexer::OpRow>(&value) {
                Ok(row) => stream_op_row(row),
                Err(_) => {
                    frames.push(unavailable(
                        topic,
                        "stored op row was not a borsh envelope — rebuild the index",
                    ));
                    return CatchUpResult::drop(frames);
                }
            };
            *cursor = key.clone();
            emitted += 1;
            frames.push(ServerFrame::Event {
                topic: topic.to_string(),
                cursor: key,
                op,
            });
        }
        if page.has_more {
            if emitted >= STREAM_CATCHUP_BUDGET {
                return lag_to_live(topic, module, cursor, store, frames);
            }
            if entry_count == 0 {
                break;
            }
            continue;
        }
        break;
    }
    CatchUpResult::keep(frames)
}

fn catch_up_files(topic: &str, cursor: &mut String, store: &indexer::IndexStore) -> CatchUpResult {
    if let Some(frame) = lag_if_below_backfill(topic, "files", cursor, store) {
        return frame;
    }

    let mut frames = Vec::new();
    let mut emitted = 0usize;
    let mut scanned = 0usize;
    loop {
        let page = match store.scan(
            "files",
            indexer::OP_PREFIX.as_bytes(),
            Some(cursor.as_bytes()),
            STREAM_CATCHUP_BUDGET,
        ) {
            Ok(page) => page,
            Err(err) => {
                frames.push(unavailable(topic, err.to_string()));
                return CatchUpResult::drop(frames);
            }
        };
        let entry_count = page.entries.len();
        scanned += entry_count;
        for (key, value) in page.entries {
            let key = String::from_utf8_lossy(&key).into_owned();
            // THE SAME BYTES `catch_up_module` READS, so the same decoder.
            // `IndexStore::apply_block` writes one BORSH `indexer::OpRow` per
            // dispatch; this read them as json, so every files commit answered
            // "rebuild the index" and dropped the topic. The index was fine —
            // `files:watch` has simply never delivered a frame.
            let row = match borsh::from_slice::<indexer::OpRow>(&value) {
                Ok(row) => stream_op_row(row),
                Err(_) => {
                    frames.push(unavailable(
                        topic,
                        "stored op row was not a borsh envelope — rebuild the index",
                    ));
                    return CatchUpResult::drop(frames);
                }
            };
            *cursor = key.clone();
            let Some(payload) = row.payload else {
                continue;
            };
            let Ok(FilesMsg::Commit {
                base_snapshot,
                message,
                changes,
            }) = serde_json::from_value::<FilesMsg>(payload)
            else {
                continue;
            };
            let mut paths = Vec::new();
            for change in &changes {
                append_change_paths(change, &mut paths);
            }
            emitted += 1;
            frames.push(ServerFrame::Tail {
                topic: topic.to_string(),
                cursor: key,
                item: TailItem::FileChange {
                    height: row.height,
                    time: row.time,
                    message,
                    base_snapshot,
                    paths,
                },
            });
            if emitted >= STREAM_CATCHUP_BUDGET && page.has_more {
                return lag_to_live(topic, "files", cursor, store, frames);
            }
        }
        if page.has_more {
            if entry_count == 0 {
                break;
            }
            // a stage-heavy history is mostly non-commit rows that never
            // count against the emit budget — bound the raw scan too, or a
            // far-behind resume stalls the session task in one wakeup.
            if scanned >= FILES_SCAN_BUDGET {
                return lag_to_live(topic, "files", cursor, store, frames);
            }
            continue;
        }
        break;
    }
    CatchUpResult::keep(frames)
}

fn catch_up_logs(topic: &str, seq: &mut u64, logs: &LogRing) -> CatchUpResult {
    let mut frames = Vec::new();
    let (_, floor) = logs.read_after(*seq, STREAM_CATCHUP_BUDGET);
    if *seq < floor {
        // the ring wrapped past this reader: the evidence it came for is GONE.
        // `Lagged` alone re-cursors SILENTLY, so the tab just shows a shorter
        // history and nothing says why — say it in the tail itself, where the
        // human is actually looking. this is also how you learn empirically
        // whether the `info` floor is too chatty, instead of guessing.
        let dropped = floor - *seq;
        *seq = floor;
        frames.push(ServerFrame::Lagged {
            topic: topic.to_string(),
            cursor: floor.to_string(),
        });
        frames.push(ServerFrame::Tail {
            topic: topic.to_string(),
            cursor: floor.to_string(),
            item: TailItem::Log {
                line: format!("--- {dropped} earlier log line(s) dropped (ring full) ---"),
            },
        });
    }
    loop {
        let (rows, _) = logs.read_after(*seq, STREAM_CATCHUP_BUDGET);
        if rows.is_empty() {
            break;
        }
        let row_count = rows.len();
        for (line_seq, line) in rows {
            *seq = line_seq;
            frames.push(ServerFrame::Tail {
                topic: topic.to_string(),
                cursor: line_seq.to_string(),
                item: TailItem::Log { line },
            });
        }
        if row_count < STREAM_CATCHUP_BUDGET {
            break;
        }
    }
    CatchUpResult::keep(frames)
}

fn catch_up_run_output(
    topic: &str,
    id: &str,
    seq: &mut u64,
    runs: &RunOutputRegistry,
) -> CatchUpResult {
    let mut frames = Vec::new();
    let (_, floor) = runs.read_after(id, *seq, STREAM_CATCHUP_BUDGET);
    if *seq < floor {
        *seq = floor;
        frames.push(ServerFrame::Lagged {
            topic: topic.to_string(),
            cursor: floor.to_string(),
        });
    }
    loop {
        let (rows, _) = runs.read_after(id, *seq, STREAM_CATCHUP_BUDGET);
        if rows.is_empty() {
            break;
        }
        let row_count = rows.len();
        for (line_seq, stream, line) in rows {
            *seq = line_seq;
            frames.push(ServerFrame::Tail {
                topic: topic.to_string(),
                cursor: line_seq.to_string(),
                item: TailItem::RunOutput { stream, line },
            });
        }
        if row_count < STREAM_CATCHUP_BUDGET {
            break;
        }
    }
    CatchUpResult::keep(frames)
}

/// replay a terminal session's ring the way `catch_up_run_output` replays a
/// run's: a `Lagged` frame if the reader fell behind an eviction, then every
/// buffered chunk after the cursor as a `Term` tail item. Emits nothing for an
/// unknown/evicted session (the ring read returns empty) — never an error.
fn catch_up_term(
    topic: &str,
    session: &str,
    seq: &mut u64,
    ring: &crate::term::TermRing,
) -> CatchUpResult {
    let mut frames = Vec::new();
    let (_, floor) = ring.read_after(session, *seq, STREAM_CATCHUP_BUDGET);
    if *seq < floor {
        *seq = floor;
        frames.push(ServerFrame::Lagged {
            topic: topic.to_string(),
            cursor: floor.to_string(),
        });
    }
    loop {
        let (rows, _) = ring.read_after(session, *seq, STREAM_CATCHUP_BUDGET);
        if rows.is_empty() {
            break;
        }
        let row_count = rows.len();
        for (chunk_seq, item) in rows {
            *seq = chunk_seq;
            frames.push(ServerFrame::TermChunk {
                topic: topic.to_string(),
                cursor: chunk_seq.to_string(),
                item,
            });
        }
        if row_count < STREAM_CATCHUP_BUDGET {
            break;
        }
    }
    // the pump reached EOF and the ring is fully drained: tell the subscriber the
    // session is over and drop the topic, so a driving client stops waiting on a
    // stream that will never append again. Only after every buffered chunk above
    // has been emitted — the terminal frame is the LAST thing on the topic.
    if ring.is_ended(session) {
        frames.push(ServerFrame::TermEnded {
            topic: topic.to_string(),
        });
        return CatchUpResult::drop(frames);
    }
    CatchUpResult::keep(frames)
}

/// replay a session's ordered command log the way `catch_up_term` replays its
/// output ring: a `Lagged` frame if the reader fell behind an eviction, then
/// every buffered command after the cursor as a `TermCommandLog` frame — the
/// ordered, attributed view of the shared session. Emits nothing for an
/// unknown/evicted session (the ring read returns empty) — never an error.
fn catch_up_term_command(
    topic: &str,
    session: &str,
    seq: &mut u64,
    ring: &crate::term::TermCommandRing,
) -> CatchUpResult {
    let mut frames = Vec::new();
    let (_, floor) = ring.read_after(session, *seq, STREAM_CATCHUP_BUDGET);
    if *seq < floor {
        *seq = floor;
        frames.push(ServerFrame::Lagged {
            topic: topic.to_string(),
            cursor: floor.to_string(),
        });
    }
    loop {
        let (rows, _) = ring.read_after(session, *seq, STREAM_CATCHUP_BUDGET);
        if rows.is_empty() {
            break;
        }
        let row_count = rows.len();
        for (cmd_seq, origin, text) in rows {
            *seq = cmd_seq;
            frames.push(ServerFrame::TermCommandLog {
                topic: topic.to_string(),
                seq: cmd_seq,
                origin,
                text,
            });
        }
        if row_count < STREAM_CATCHUP_BUDGET {
            break;
        }
    }
    CatchUpResult::keep(frames)
}

/// re-sample the node's OpenMetrics exposition through the SAME wired source
/// GET /metrics reads (the handle's status cell), so the stream needs no
/// second registry encoder — and no actor round-trip. one Tail frame per
/// wakeup carrying the whole scrape text; an unwired source drops the topic
/// with the same `unavailable` shape the http lane's 503 carries.
async fn catch_up_metrics(
    topic: &str,
    state: &mut TopicState,
    handle: &NodeHandle,
) -> CatchUpResult {
    let TopicState::Metrics { sampled_ms } = state else {
        return CatchUpResult::keep(Vec::new());
    };
    let Some(text) = handle.status_cell().exposition() else {
        return CatchUpResult::drop(vec![unavailable(
            topic,
            "no metrics exposition is wired on this daemon",
        )]);
    };
    let time_ms = unix_millis();
    *sampled_ms = time_ms;
    CatchUpResult::keep(vec![ServerFrame::Tail {
        topic: topic.to_string(),
        cursor: time_ms.to_string(),
        item: TailItem::Metrics { time_ms, text },
    }])
}

fn lag_if_below_backfill(
    topic: &str,
    module: &str,
    cursor: &mut String,
    store: &indexer::IndexStore,
) -> Option<CatchUpResult> {
    let floor = match store.backfill_height(module) {
        Ok(Some(floor)) => floor,
        Ok(None) => return None,
        Err(err) => {
            return Some(CatchUpResult::drop(vec![unavailable(
                topic,
                err.to_string(),
            )]));
        }
    };
    if cursor_height(cursor).is_some_and(|height| height < floor) {
        return Some(lag_to_live(topic, module, cursor, store, Vec::new()));
    }
    None
}

fn lag_to_live(
    topic: &str,
    module: &str,
    cursor: &mut String,
    store: &indexer::IndexStore,
    mut frames: Vec<ServerFrame>,
) -> CatchUpResult {
    match live_cursor(store, module) {
        Ok(jump) => {
            *cursor = jump.clone();
            frames.push(ServerFrame::Lagged {
                topic: topic.to_string(),
                cursor: jump,
            });
            CatchUpResult::keep(frames)
        }
        Err(err) => {
            frames.push(unavailable(topic, err.to_string()));
            CatchUpResult::drop(frames)
        }
    }
}

async fn send_frames(socket: &mut WebSocket, frames: Vec<ServerFrame>) -> bool {
    for frame in frames {
        if !send_frame(socket, frame).await {
            return false;
        }
    }
    true
}

async fn send_frame(socket: &mut WebSocket, frame: ServerFrame) -> bool {
    let text = serde_json::to_string(&frame).expect("stream frame serializes");
    socket.send(Message::Text(text.into())).await.is_ok()
}

fn heartbeat_frame(hub: &StreamHub) -> ServerFrame {
    let (height, root_hash) = hub.tip().unwrap_or_else(|| (0, String::new()));
    ServerFrame::Heartbeat {
        height,
        root_hash,
        time_ms: unix_millis(),
        interval_ms: HEARTBEAT_INTERVAL_MS,
    }
}

pub(crate) fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is past the epoch")
        .as_millis() as u64
}

fn live_cursor(store: &indexer::IndexStore, module: &str) -> Result<String, indexer::Error> {
    let applied = store.applied_height(module)?;
    Ok(format!("{}{:016x}/ffff", indexer::OP_PREFIX, applied))
}

fn cursor_height(cursor: &str) -> Option<u64> {
    let rest = cursor.strip_prefix(indexer::OP_PREFIX)?;
    let height = rest.get(0..16)?;
    if rest.as_bytes().get(16) != Some(&b'/') {
        return None;
    }
    u64::from_str_radix(height, 16).ok()
}

#[allow(clippy::result_large_err)]
fn parse_seq_cursor(topic: &str, cursor: &str) -> Result<u64, ServerFrame> {
    cursor.parse::<u64>().map_err(|_| ServerFrame::Error {
        topic: topic.to_string(),
        code: StreamErrorCode::BadCursor,
        detail: "cursor must be a numeric sequence".into(),
    })
}

fn unavailable(topic: impl Into<String>, detail: impl Into<String>) -> ServerFrame {
    ServerFrame::Error {
        topic: topic.into(),
        code: StreamErrorCode::Unavailable,
        detail: detail.into(),
    }
}

fn append_change_paths(change: &Change, paths: &mut Vec<String>) {
    match change {
        Change::Put { path, .. }
        | Change::Mkdir { path }
        | Change::Rm { path }
        | Change::Symlink { path, .. } => paths.push(path.clone()),
        Change::Mv { from, to } => {
            paths.push(from.clone());
            paths.push(to.clone());
        }
    }
}

#[cfg(test)]
mod tests {

    use indexer::{AppliedOp, BlockOps, IndexModule, OriginTag};
    use serde_json::json;

    use super::*;

    /// a caller that presented no workspace secret (or the wrong one).
    const NO_SECRET: bool = false;
    /// a caller whose presented secret matched.
    const HOLDS_SECRET: bool = true;
    /// the workspace secret a test node mints.
    const TEST_SECRET: &str = "d3adb33fd3adb33fd3adb33fd3adb33f";

    /// a handle whose terminal plane holds [`TEST_SECRET`] — a node with a
    /// workspace, i.e. the only shape that can admit a gated topic at all. The
    /// actor lane is unused on every subscribe path, so its receiver is dropped
    /// here rather than parked in each caller.
    fn handle_with_secret() -> crate::NodeHandle {
        let (handle, _cmds, _hub) = crate::NodeHandle::channel();
        handle.with_terminals(crate::term::TerminalSessions::new(
            crate::term::TermRing::default(),
            crate::term::TermCommandRing::default(),
            Some(TEST_SECRET.into()),
        ))
    }

    fn temp_store(modules: &[&str]) -> (tempfile::TempDir, Arc<indexer::IndexStore>) {
        let dir = tempfile::TempDir::new().expect("temp index dir");
        let bare: Vec<IndexModule> = modules.iter().map(|id| IndexModule::bare(id)).collect();
        let store = indexer::IndexStore::open(dir.path(), &bare).expect("open index");
        (dir, Arc::new(store))
    }

    fn apply_chat(store: &indexer::IndexStore, height: u64, payloads: Vec<serde_json::Value>) {
        let ops = payloads
            .into_iter()
            .map(|payload| AppliedOp {
                module: "chat".into(),
                origin: OriginTag::external("tester"),
                payload: serde_json::to_vec(&payload).expect("payload json"),
                assigned: Vec::new(),
            })
            .collect();
        store
            .apply_block(&BlockOps {
                height,
                time: height * 10,
                ops,
                record: None,
            })
            .expect("apply block");
    }

    /// FILES:WATCH HAD NEVER DELIVERED A FRAME.
    ///
    /// `IndexStore::apply_block` writes one BORSH `indexer::OpRow` per dispatch
    /// — the same bytes `catch_up_module` reads with `borsh::from_slice`. This
    /// path read them as json, so the first files commit answered "rebuild the
    /// index" and dropped the topic, blaming a store that was correct.
    ///
    /// The block goes in through the REAL `apply_block`, so the encoding under
    /// test is the one the node actually writes — which is the whole reason a
    /// test here catches it and none existed.
    #[test]
    fn files_watch_reads_the_rows_the_index_actually_wrote() {
        let (_dir, store) = temp_store(&["files"]);
        let commit = json!({
            "commit": {
                "base_snapshot": null,
                "message": "first",
                "changes": [{ "mkdir": { "path": "notes" } }],
            }
        });
        store
            .apply_block(&BlockOps {
                height: 1,
                time: 10,
                ops: vec![AppliedOp {
                    module: "files".into(),
                    origin: OriginTag::external("tester"),
                    payload: serde_json::to_vec(&commit).expect("payload json"),
                    assigned: Vec::new(),
                }],
                record: None,
            })
            .expect("apply block");

        let mut cursor = "op/0000000000000000/ffff".to_string();
        let result = catch_up_files("files:watch", &mut cursor, &store);
        assert!(
            !result.drop_topic,
            "a healthy index must not drop the topic: {:?}",
            result.frames
        );
        match result.frames.as_slice() {
            [
                ServerFrame::Tail {
                    item: TailItem::FileChange { paths, message, .. },
                    ..
                },
            ] => {
                assert_eq!(paths, &["notes".to_string()]);
                assert_eq!(message, "first");
            }
            other => panic!("expected one file-change tail, got {other:?}"),
        }
    }

    #[test]
    fn module_catch_up_emits_rows_and_cursors() {
        let (_dir, store) = temp_store(&["chat"]);
        apply_chat(&store, 1, vec![json!({"one": 1}), json!({"two": 2})]);
        let mut cursor = "op/0000000000000000/ffff".to_string();
        let result = catch_up_module("module:chat", "chat", &mut cursor, &store);
        assert!(!result.drop_topic);
        assert_eq!(result.frames.len(), 2);
        match &result.frames[0] {
            ServerFrame::Event { cursor, op, .. } => {
                assert_eq!(cursor, "op/0000000000000001/0000");
                assert_eq!(op.payload, Some(json!({"one": 1})));
            }
            other => panic!("expected event, got {other:?}"),
        }
        assert_eq!(cursor, "op/0000000000000001/0001");
    }

    /// A BLOCK THAT APPENDED NOTHING MUST NOT SEND ANYONE BACK TO THE STORE.
    ///
    /// An idle chain nop-fills once per `BLOCK_TIME`, and that filler dispatches
    /// nothing, so every scan it used to trigger read the index and found
    /// nothing — per subscribed topic, per session, once a second, forever.
    #[test]
    fn an_unfed_block_owes_the_index_nothing() {
        assert_eq!(BlockWake::from_dispatches(&[]), BlockWake::TipOnly);
        assert_eq!(
            block_action(Ok(BlockWake::TipOnly)),
            BlockAction::TipOnly,
            "an unfed block still sends the tip — the head moves on nop blocks"
        );
        assert_eq!(
            block_action(Ok(BlockWake::IndexChanged)),
            BlockAction::SweepIndex
        );
    }

    /// A DROPPED WAKE IS SWEPT, NOT SKIPPED. The discriminants went with the
    /// dropped wakes, so any of them may have fed a module. Sweeping a block
    /// that did not costs one empty scan; skipping one that did strands the
    /// topic until the backstop.
    #[test]
    fn a_lagged_wake_sweeps_and_a_closed_hub_stops() {
        assert_eq!(
            block_action(Err(broadcast::error::RecvError::Lagged(7))),
            BlockAction::SweepIndex
        );
        assert_eq!(
            block_action(Err(broadcast::error::RecvError::Closed)),
            BlockAction::Stop
        );
    }

    #[test]
    fn module_budget_overflow_lagged_jumps_to_watermark() {
        let (_dir, store) = temp_store(&["chat"]);
        let payloads = (0..=STREAM_CATCHUP_BUDGET)
            .map(|i| json!({ "n": i }))
            .collect();
        apply_chat(&store, 1, payloads);
        let mut cursor = "op/0000000000000000/ffff".to_string();
        let result = catch_up_module("module:chat", "chat", &mut cursor, &store);
        assert_eq!(result.frames.len(), STREAM_CATCHUP_BUDGET + 1);
        assert!(
            matches!(result.frames.last(), Some(ServerFrame::Lagged { cursor, .. }) if cursor == "op/0000000000000001/ffff")
        );
        assert_eq!(cursor, "op/0000000000000001/ffff");
    }

    #[test]
    fn fresh_module_subscribe_starts_at_live_tip() {
        let (_dir, store) = temp_store(&["chat"]);
        apply_chat(&store, 1, vec![json!({"one": 1})]);
        let (state, lagged) =
            prepare_topic("module:chat", NO_SECRET, None, Some(&store)).expect("topic");
        assert!(lagged.is_none());
        assert_eq!(state.cursor(), "op/0000000000000001/ffff");
        let mut state = state;
        let result = catch_up_topic(
            "module:chat",
            &mut state,
            Some(&store),
            &StreamHub::new(crate::handle::EVENT_BUFFER),
        );
        assert!(result.frames.is_empty());
    }

    #[test]
    fn resume_below_backfill_floor_lagged_to_live_watermark() {
        let (_dir, store) = temp_store(&["chat"]);
        store.mark_backfilled("chat", 10).expect("mark backfilled");
        let (state, lagged) = prepare_topic(
            "module:chat",
            NO_SECRET,
            Some(&"op/0000000000000005/0000".to_string()),
            Some(&store),
        )
        .expect("topic");
        assert_eq!(state.cursor(), "op/000000000000000a/ffff");
        assert!(
            matches!(lagged, Some(ServerFrame::Lagged { cursor, .. }) if cursor == "op/000000000000000a/ffff")
        );
    }

    #[test]
    fn topic_refusals_are_per_topic() {
        assert!(matches!(
            prepare_topic("module:chat", NO_SECRET, None, None),
            Err(ServerFrame::Error {
                code: StreamErrorCode::Unavailable,
                ..
            })
        ));
        let (_dir, store) = temp_store(&["chat"]);
        assert!(matches!(
            prepare_topic("module:nope", NO_SECRET, None, Some(&store)),
            Err(ServerFrame::Error {
                code: StreamErrorCode::UnknownTopic,
                ..
            })
        ));
        assert!(matches!(
            prepare_topic("logs", NO_SECRET, Some(&"not-a-seq".to_string()), Some(&store)),
            Err(ServerFrame::Error {
                code: StreamErrorCode::BadCursor,
                ..
            })
        ));
    }

    #[test]
    fn log_ring_wrap_reports_lagged_then_replays_from_floor() {
        let logs = LogRing::default();
        for i in 0..=LOG_RING_CAPACITY {
            logs.push_line(format!("line-{i}"));
        }
        let mut seq = 0;
        let result = catch_up_logs("logs", &mut seq, &logs);
        assert!(
            matches!(result.frames.first(), Some(ServerFrame::Lagged { cursor, .. }) if cursor == "1")
        );
        // the eviction is NAMED in the tail, not just silently re-cursored: a
        // reader must never mistake a truncated history for a quiet node.
        assert!(matches!(
            result.frames.get(1),
            Some(ServerFrame::Tail { item: TailItem::Log { line }, .. })
                if line == "--- 1 earlier log line(s) dropped (ring full) ---"
        ));
        assert!(
            matches!(result.frames.get(2), Some(ServerFrame::Tail { cursor, .. }) if cursor == "2")
        );
        assert_eq!(seq, (LOG_RING_CAPACITY + 1) as u64);
    }

    #[test]
    fn run_output_ring_wraps_and_evicts_lru_runs() {
        let runs = RunOutputRegistry::default();
        for i in 0..=RUN_OUTPUT_MAX_LINES {
            runs.append("active", RunStream::Stdout, format!("line-{i}"));
        }
        let mut seq = 0;
        let result = catch_up_run_output("run-output:active", "active", &mut seq, &runs);
        assert!(
            matches!(result.frames.first(), Some(ServerFrame::Lagged { cursor, .. }) if cursor == "1")
        );
        assert_eq!(seq, (RUN_OUTPUT_MAX_LINES + 1) as u64);

        for i in 0..RUN_OUTPUT_MAX_RUNS {
            runs.append(format!("run-{i}"), RunStream::Stderr, "x");
        }
        let (rows, floor) = runs.read_after("active", 0, 1);
        assert!(rows.is_empty());
        assert_eq!(floor, 0);
    }

    #[test]
    fn remote_run_output_does_not_rebroadcast() {
        let runs = RunOutputRegistry::default();
        let mut appends = runs.subscribe_appends();
        runs.append("aa".repeat(32), RunStream::Stdout, "local");
        assert_eq!(appends.try_recv().unwrap().line, "local");

        runs.append_remote(RunOutputEvent {
            id: "aa".repeat(32),
            stream: RunStream::Stderr,
            line: "remote".into(),
        });
        assert!(matches!(
            appends.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        ));
        let (rows, _) = runs.read_after(&"aa".repeat(32), 0, 10);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].2, "remote");
    }

    #[tokio::test(start_paused = true)]
    async fn heartbeat_frame_shape_under_paused_time() {
        let hub = StreamHub::new(crate::handle::EVENT_BUFFER);
        hub.prime(7, "abc");
        let mut interval = tokio::time::interval(Duration::from_millis(HEARTBEAT_INTERVAL_MS));
        interval.tick().await;
        tokio::time::advance(Duration::from_millis(HEARTBEAT_INTERVAL_MS)).await;
        interval.tick().await;
        match heartbeat_frame(&hub) {
            ServerFrame::Heartbeat {
                height,
                root_hash,
                interval_ms,
                ..
            } => {
                assert_eq!(height, 7);
                assert_eq!(root_hash, "abc");
                assert_eq!(interval_ms, HEARTBEAT_INTERVAL_MS);
            }
            other => panic!("expected heartbeat, got {other:?}"),
        }
    }

    #[test]
    fn term_topic_subscribes_and_replays_as_event_tagged_chunks() {
        // any session id subscribes (the manager gates who may CREATE one);
        // a fresh subscribe starts at cursor 0 and needs no index store.
        let (state, lagged) =
            prepare_topic("term:abc", HOLDS_SECRET, None, None).expect("term topic subscribes");
        assert!(lagged.is_none());
        assert_eq!(state.cursor(), "0");

        let ring = crate::term::TermRing::default();
        ring.append("s", "aGk=".to_string()); // base64("hi")
        ring.append("s", "eW8=".to_string()); // base64("yo")
        let mut seq = 0u64;
        let result = catch_up_term("term:s", "s", &mut seq, &ring);
        assert!(!result.drop_topic);
        assert_eq!(result.frames.len(), 2);
        assert_eq!(seq, 2, "the cursor advances past the replayed chunks");
        // the LOAD-BEARING wire shape the app's `isTermChunkFrame` keys on: a
        // `type:"event"` frame with a bare-string `item`, its resume cursor,
        // and no module op.
        let json = serde_json::to_value(&result.frames[0]).expect("frame json");
        assert_eq!(json["type"], "event");
        assert_eq!(json["topic"], "term:s");
        assert_eq!(json["cursor"], "1");
        assert_eq!(json["item"], "aGk=");
        assert!(json.get("op").is_none(), "a term chunk carries no op");
        let mut resumed = 1;
        let resumed_result = catch_up_term("term:s", "s", &mut resumed, &ring);
        assert_eq!(resumed_result.frames.len(), 1);
        assert_eq!(resumed, 2);
        let json = serde_json::to_value(&resumed_result.frames[0]).expect("resumed frame json");
        assert_eq!(json["cursor"], "2");
        assert_eq!(json["item"], "eW8=");
        // a caught-up reader sees nothing new.
        assert!(
            catch_up_term("term:s", "s", &mut seq, &ring)
                .frames
                .is_empty()
        );
    }

    #[test]
    fn term_command_topic_subscribes_and_replays_the_ordered_attributed_log() {
        // any session id subscribes to its command log (like `term:`); a fresh
        // subscribe starts at cursor 0 and needs no index store.
        let (state, lagged) =
            prepare_topic("term-cmd:abc", HOLDS_SECRET, None, None)
                .expect("term-cmd topic subscribes");
        assert!(lagged.is_none());
        assert_eq!(state.cursor(), "0");
        assert!(matches!(state, TopicState::TermCommand { .. }));

        let ring = crate::term::TermCommandRing::default();
        ring.append("s", "alice", "list files");
        ring.append("s", "", "run tests"); // empty origin = "local" (attribution kept verbatim)
        let mut seq = 0u64;
        let result = catch_up_term_command("term-cmd:s", "s", &mut seq, &ring);
        assert!(!result.drop_topic);
        assert_eq!(result.frames.len(), 2);
        assert_eq!(seq, 2, "the cursor advances past the replayed commands");
        // ordered + attributed: seq 1 first, carrying its origin + text.
        let json = serde_json::to_value(&result.frames[0]).expect("frame json");
        assert_eq!(json["type"], "term_command_log");
        assert_eq!(json["topic"], "term-cmd:s");
        assert_eq!(json["seq"], 1);
        assert_eq!(json["origin"], "alice");
        assert_eq!(json["text"], "list files");
        // a caught-up reader sees nothing new.
        assert!(
            catch_up_term_command("term-cmd:s", "s", &mut seq, &ring)
                .frames
                .is_empty()
        );
    }

    #[test]
    fn published_run_output_is_bounded_and_shaped_before_it_reaches_the_ring() {
        // the guard exists because a line that reaches the ring is broadcast to
        // every overlay peer, and the agent data plane REFUSES to write an event
        // whose id is not 64-hex — treating that refusal as fatal to the peer
        // stream. One malformed frame would otherwise tear down and reopen every
        // peer's telemetry, repeatably, from any local ws client.
        let hub = StreamHub::new(16);
        let runs = hub.run_output();
        let good = "a".repeat(RUN_OUTPUT_ID_LEN);

        // every id shape the peer path would reject is dropped here instead.
        for bad in [
            String::from("x"),
            String::new(),
            "a".repeat(RUN_OUTPUT_ID_LEN - 1),
            "a".repeat(RUN_OUTPUT_ID_LEN + 1),
            "g".repeat(RUN_OUTPUT_ID_LEN),
            format!("{}-", "a".repeat(RUN_OUTPUT_ID_LEN - 1)),
        ] {
            handle_run_output(&hub, bad.clone(), RunStream::Stdout, "hi".into());
            let mut seq = 0;
            assert!(
                catch_up_run_output(&format!("run-output:{bad}"), &bad, &mut seq, &runs)
                    .frames
                    .is_empty(),
                "id {bad:?} must never reach the ring"
            );
        }

        // an oversized line is dropped too — the ring's caps bound line COUNT,
        // never bytes.
        handle_run_output(
            &hub,
            good.clone(),
            RunStream::Stdout,
            "x".repeat(MAX_RUN_OUTPUT_LINE + 1),
        );
        let mut seq = 0;
        assert!(
            catch_up_run_output(&format!("run-output:{good}"), &good, &mut seq, &runs)
                .frames
                .is_empty(),
            "an oversized line must never reach the ring"
        );

        // the real shape — a hex sha256 run key, an ordinary line — is admitted,
        // so the guard bounds the surface without refusing the daemon its own
        // runs.
        handle_run_output(&hub, good.clone(), RunStream::Stdout, "real output".into());
        let mut seq = 0;
        let caught = catch_up_run_output(&format!("run-output:{good}"), &good, &mut seq, &runs);
        assert_eq!(caught.frames.len(), 1, "a well-formed line is admitted");
        // and a line exactly AT the cap is still admitted (the bound is not
        // accidentally off by one against real output).
        handle_run_output(
            &hub,
            good.clone(),
            RunStream::Stderr,
            "x".repeat(MAX_RUN_OUTPUT_LINE),
        );
        let caught = catch_up_run_output(&format!("run-output:{good}"), &good, &mut seq, &runs);
        assert_eq!(caught.frames.len(), 1, "a line at the cap is admitted");
    }

    #[test]
    fn only_an_admitted_session_handle_may_drive_a_pty() {
        let mut topics: BTreeMap<String, TopicState> = BTreeMap::new();
        // a connection that holds no handle drives nothing.
        assert!(!holds_session(&topics, "sess1"));
        // an admitted handle on a session drives it — and only it.
        topics.insert(
            crate::term::topic("sess1"),
            TopicState::Term {
                session: "sess1".into(),
                seq: 0,
            },
        );
        assert!(holds_session(&topics, "sess1"));
        assert!(!holds_session(&topics, "sess2"));
        // a non-terminal handle never drives a pty, whatever its name.
        topics.insert(LOGS_TOPIC.into(), TopicState::Logs { seq: 0 });
        assert!(!holds_session(&topics, LOGS_TOPIC));
        // and neither does the COMMAND-log handle for the same session: it is a
        // different key, so it can never stand in for the output handle.
        topics.insert(
            crate::term::command_topic("sess3"),
            TopicState::TermCommand {
                session: "sess3".into(),
                seq: 0,
            },
        );
        assert!(!holds_session(&topics, "sess3"));

        // the VARIANT is load-bearing, not decoration. Every case above differs
        // by KEY too, so a revert to the deleted check's key-only
        // `contains_key` would pass them all; this one cannot be built by
        // `prepare_topic` and exists precisely to fail that revert. A `term:`
        // key whose state is not a `Term` is a map this node never wrote — and
        // "never written" is a claim worth a test rather than a comment.
        topics.insert(crate::term::topic("sess4"), TopicState::Logs { seq: 0 });
        assert!(
            !holds_session(&topics, "sess4"),
            "a term-keyed handle that is not a Term state must never drive a pty"
        );
    }

    /// Every family's admission is DECIDED, and the table is the decision.
    ///
    /// A new family cannot reach this list by accident: `Topic::admission` has
    /// no `_` arm, so adding a variant fails the build until someone writes its
    /// admission, and adding it here is how that choice gets reviewed.
    #[test]
    fn every_topic_family_has_a_decided_admission() {
        let decided = [
            (Topic::Module("chat"), Admission::Public),
            (Topic::FilesWatch, Admission::Public),
            (Topic::Logs, Admission::Public),
            (Topic::Metrics, Admission::Public),
            (Topic::RunOutput("r1"), Admission::Workspace),
            (Topic::TermCommand("s1"), Admission::Workspace),
            (Topic::Term("s1"), Admission::Workspace),
        ];
        for (family, expected) in decided {
            assert_eq!(family.admission(), expected, "{family:?}");
        }

        // the wire names round-trip to the families above ...
        assert_eq!(Topic::parse("module:chat"), Some(Topic::Module("chat")));
        assert_eq!(Topic::parse("files:watch"), Some(Topic::FilesWatch));
        assert_eq!(Topic::parse("logs"), Some(Topic::Logs));
        assert_eq!(Topic::parse("metrics"), Some(Topic::Metrics));
        assert_eq!(Topic::parse("run-output:r1"), Some(Topic::RunOutput("r1")));
        assert_eq!(Topic::parse("term:s1"), Some(Topic::Term("s1")));
        // `term-cmd:` is its own family and never decodes as a `term:` session
        // named "cmd:s1" — the two prefixes diverge before the colon.
        assert_eq!(
            Topic::parse("term-cmd:s1"),
            Some(Topic::TermCommand("s1"))
        );

        // ... and a name no family owns parses to nothing, which is what makes
        // admission deny-by-default rather than a habit.
        for unknown in ["", "term", "logs2", "modules:chat", "files:watch2"] {
            assert_eq!(Topic::parse(unknown), None, "{unknown:?} owns no family");
            assert!(matches!(
                prepare_topic(unknown, HOLDS_SECRET, None, None),
                Err(ServerFrame::Error {
                    code: StreamErrorCode::UnknownTopic,
                    ..
                })
            ));
        }
    }

    /// A workspace-gated family hands back NO handle without the secret.
    #[test]
    fn gated_families_refuse_a_caller_with_no_workspace_secret() {
        for gated in ["term:s1", "term-cmd:s1", "run-output:r1"] {
            let Err(ServerFrame::Error { code, detail, .. }) =
                prepare_topic(gated, NO_SECRET, None, None)
            else {
                panic!("{gated} must refuse a caller with no workspace secret");
            };
            assert_eq!(code, StreamErrorCode::Forbidden);
            // A TRIPWIRE, not the live check: `detail()` is a `&'static str`
            // with no access to any secret, so this cannot fail today — it fails
            // the day someone gives the refusal a formatted body. The real
            // guarantee is structural and is stated where it is enforced, on
            // `TopicRefusal::detail`.
            assert!(
                !detail.contains(TEST_SECRET),
                "a refusal must never carry the secret: {detail}"
            );
            // and it admits the same caller once the secret matches.
            assert!(prepare_topic(gated, HOLDS_SECRET, None, None).is_ok());
        }
        // the public families need nothing, on the same call.
        assert!(prepare_topic("logs", NO_SECRET, None, None).is_ok());
        assert!(prepare_topic("metrics", NO_SECRET, None, None).is_ok());
    }

    /// A wrong secret is exactly as good as no secret — the compare is the gate,
    /// not the presence of the field.
    #[test]
    fn a_wrong_secret_admits_nothing() {
        let handle = handle_with_secret();
        for presented in [None, Some("not-the-secret"), Some("")] {
            let mut states = BTreeMap::new();
            subscribe_topics(
                &handle,
                &mut states,
                vec!["term:s1".into()],
                &BTreeMap::new(),
                presented,
            );
            assert!(states.is_empty(), "presented {presented:?} admitted a pty");
        }
        // a node with NO TERMINAL PLANE admits nobody, whatever they present.
        let (bare, _cmds, _hub) = crate::NodeHandle::channel();
        let mut states = BTreeMap::new();
        subscribe_topics(
            &bare,
            &mut states,
            vec!["term:s1".into()],
            &BTreeMap::new(),
            Some(TEST_SECRET),
        );
        assert!(states.is_empty(), "a node with no plane admits nobody");

        // and NEITHER does a node whose plane minted no secret — the case that
        // actually ships. `bin/noded/src/main.rs` passes `None`, and
        // `bin/node/src/boot/surfaces.rs` does too whenever `mint_link_token`
        // fails. It must reach `link_token_matches` itself rather than
        // short-circuiting in `workspace_secret_matches` one level up, which is
        // where the plane-less case above stops: an `is_none_or` slip inside
        // that function turns "this node minted no secret" into "this node
        // admits EVERYBODY", and only this case can see it.
        let (unminted, _cmds, _hub) = crate::NodeHandle::channel();
        let unminted = unminted.with_terminals(crate::term::TerminalSessions::new(
            crate::term::TermRing::default(),
            crate::term::TermCommandRing::default(),
            None,
        ));
        for presented in ["", TEST_SECRET] {
            assert!(
                !unminted.workspace_secret_matches(presented),
                "a plane that minted no secret must match nothing, got {presented:?}"
            );
            let mut states = BTreeMap::new();
            subscribe_topics(
                &unminted,
                &mut states,
                vec!["term:s1".into()],
                &BTreeMap::new(),
                Some(presented),
            );
            assert!(
                states.is_empty(),
                "a plane with no minted secret admitted {presented:?}"
            );
        }
    }

    /// THE hole, end to end: a connection that never presented the node's
    /// workspace secret cannot write a keystroke into a live pty, and one that
    /// did can.
    ///
    /// The old `term_entitled` asked `topics.contains_key("term:<id>")` while
    /// the subscribe was unconditional, so this test's first half passed only
    /// because the attacker had not bothered to subscribe. It subscribes here.
    ///
    /// Waits on the daemon's own receive, never on a duration: the assertion is
    /// that the FIRST input to reach the link is the admitted one, which is
    /// false the moment the gate leaks.
    /// a live session plus the ORDERED feed of what actually reached the
    /// daemon's link.
    ///
    /// The link is the observation seam every terminal-frame test below waits
    /// on: a frame this node dropped never arrives on it, so an assertion about
    /// what did arrive is an assertion about the gate — with no duration in it.
    /// One channel for all three command kinds, because ORDER is half the claim.
    async fn session_on_the_link(
        mode: crate::term::SessionMode,
    ) -> (
        crate::NodeHandle,
        String,
        crate::term::AttachGuard,
        mpsc::Receiver<String>,
    ) {
        use agent_service::wire;

        let handle = handle_with_secret();
        let terminals = handle.terminals().expect("a wired terminal plane").clone();
        let (link, mut commands) = terminals.attach(TEST_SECRET).expect("the daemon attaches");
        let (seen_tx, seen) = mpsc::channel::<String>(8);
        let daemon = terminals.clone();
        tokio::spawn(async move {
            while let Some(command) = commands.recv().await {
                match command {
                    wire::Command::TermCreate(create) => daemon.on_event(wire::Event::TermCreated {
                        session: create.session,
                    }),
                    wire::Command::TermInput { data_b64, .. } => {
                        let _ = seen_tx.send(format!("input:{data_b64}")).await;
                    }
                    wire::Command::TermResize { cols, rows, .. } => {
                        let _ = seen_tx.send(format!("resize:{cols}x{rows}")).await;
                    }
                    wire::Command::TermClose { .. } => {}
                }
            }
        });
        let created = terminals
            .create("claude", mode)
            .await
            .expect("the daemon answered the create");
        (handle, created.session_id, link, seen)
    }

    /// the two subscription maps a session is driven through: one that
    /// subscribed WITHOUT the node's secret (the self-grant attempt) and one
    /// that presented it.
    fn unadmitted_and_admitted(
        handle: &crate::NodeHandle,
        session: &str,
    ) -> (
        BTreeMap<String, TopicState>,
        BTreeMap<String, TopicState>,
        Vec<ServerFrame>,
    ) {
        let mut unadmitted = BTreeMap::new();
        let refusals = subscribe_topics(
            handle,
            &mut unadmitted,
            vec![crate::term::topic(session)],
            &BTreeMap::new(),
            None,
        );
        let mut admitted = BTreeMap::new();
        subscribe_topics(
            handle,
            &mut admitted,
            vec![crate::term::topic(session)],
            &BTreeMap::new(),
            Some(TEST_SECRET),
        );
        (unadmitted, admitted, refusals)
    }

    #[tokio::test]
    async fn a_connection_without_the_workspace_secret_cannot_drive_a_pty() {
        let (handle, session, _link, mut seen) =
            session_on_the_link(crate::term::SessionMode::Single).await;
        let (unadmitted, admitted, refusals) = unadmitted_and_admitted(&handle, &session);

        // the unadmitted connection SUBSCRIBED FIRST — the exact self-grant the
        // deleted check waved through — and still got nothing to send on.
        assert!(unadmitted.is_empty(), "subscribing self-granted a handle");
        assert!(!holds_session(&unadmitted, &session));
        assert!(refusals.iter().any(|frame| matches!(
            frame,
            ServerFrame::Error {
                code: StreamErrorCode::Forbidden,
                ..
            }
        )));
        assert!(holds_session(&admitted, &session));

        handle_term_input(&handle, &unadmitted, &session, "dW5hZG1pdHRlZA==").await;
        handle_term_input(&handle, &admitted, &session, "YWRtaXR0ZWQ=").await;

        assert_eq!(
            seen.recv().await.as_deref(),
            Some("input:YWRtaXR0ZWQ="),
            "the first keystroke to reach the pty must be the ADMITTED one — an \
             unadmitted write reaching the link at all is the hole"
        );
    }

    /// The other two write frames, which `term_input` alone does not cover.
    ///
    /// Both were unguarded-by-any-test before: deleting `holds_session` from
    /// `handle_term_resize` or `handle_term_command` left every unit green,
    /// because nothing in-tree constructed either frame. `term_command` is the
    /// sharper of the two — its `text` is documented as able to carry secrets
    /// and its caller-chosen `origin` is attribution written into a shared
    /// session's ordered lane.
    ///
    /// Shared mode, because the command lane exists only there (raw input is
    /// refused on it, which is why the test above uses a Single session).
    /// Ordering is deterministic, not raced: the resize is awaited onto the link
    /// channel before the command is enqueued, and the command's serial consumer
    /// writes to that SAME channel, which is FIFO.
    #[tokio::test]
    async fn neither_a_resize_nor_a_command_reaches_an_unadmitted_session() {
        let (handle, session, _link, mut seen) =
            session_on_the_link(crate::term::SessionMode::Shared).await;
        let (unadmitted, admitted, _) = unadmitted_and_admitted(&handle, &session);

        handle_term_resize(&handle, &unadmitted, &session, 1, 1).await;
        handle_term_command(
            &handle,
            &unadmitted,
            &session,
            "attacker".into(),
            "unadmitted".into(),
        );
        handle_term_resize(&handle, &admitted, &session, 120, 40).await;
        handle_term_command(&handle, &admitted, &session, "operator".into(), "ls".into());

        assert_eq!(
            seen.recv().await.as_deref(),
            Some("resize:120x40"),
            "the first resize to reach the pty must be the ADMITTED one"
        );
        assert_eq!(
            seen.recv().await.as_deref(),
            Some(format!("input:{}", STANDARD.encode(b"ls\r")).as_str()),
            "the first command to reach the pty must be the ADMITTED one"
        );
    }

    #[test]
    fn subscription_cap_refuses_new_topics_but_allows_recursoring() {
        let handle = handle_with_secret();
        let mut states = BTreeMap::new();
        let requested: Vec<String> = (0..MAX_TOPICS_PER_CONNECTION + 1)
            .map(|i| format!("run-output:r{i}"))
            .collect();
        let frames = subscribe_topics(
            &handle,
            &mut states,
            requested,
            &BTreeMap::new(),
            Some(TEST_SECRET),
        );
        assert_eq!(states.len(), MAX_TOPICS_PER_CONNECTION);
        let refused = frames
            .iter()
            .filter(|f| {
                matches!(
                    f,
                    ServerFrame::Error {
                        code: StreamErrorCode::Unavailable,
                        ..
                    }
                )
            })
            .count();
        assert_eq!(refused, 1, "exactly the over-cap topic refuses");
        // re-subscribing an EXISTING topic at the cap re-cursors, never refuses.
        let again = subscribe_topics(
            &handle,
            &mut states,
            vec!["run-output:r0".into()],
            &BTreeMap::new(),
            Some(TEST_SECRET),
        );
        assert!(
            again
                .iter()
                .all(|f| !matches!(f, ServerFrame::Error { .. })),
            "re-subscribe at the cap must stay allowed: {again:?}"
        );
        assert_eq!(states.len(), MAX_TOPICS_PER_CONNECTION);
    }

    #[test]
    fn wake_classes_route_to_their_topics() {
        let module = TopicState::Module {
            module: "chat".into(),
            cursor: String::new(),
        };
        let files = TopicState::FilesWatch {
            cursor: String::new(),
        };
        let logs = TopicState::Logs { seq: 0 };
        let run = TopicState::RunOutput {
            id: "r1".into(),
            seq: 0,
        };
        let metrics = TopicState::Metrics { sampled_ms: 0 };
        let term = TopicState::Term {
            session: "s".into(),
            seq: 0,
        };
        let term_cmd = TopicState::TermCommand {
            session: "s".into(),
            seq: 0,
        };
        assert!(module.wakes_on(Wake::Block) && files.wakes_on(Wake::Block));
        assert!(!logs.wakes_on(Wake::Block) && !run.wakes_on(Wake::Block));
        assert!(logs.wakes_on(Wake::Logs) && !module.wakes_on(Wake::Logs));
        assert!(run.wakes_on(Wake::RunOutput) && !files.wakes_on(Wake::RunOutput));
        // the two terminal planes are distinct wake sources: an output append
        // never re-scans the command log and vice versa.
        assert!(term.wakes_on(Wake::Term) && !term.wakes_on(Wake::TermCommand));
        assert!(term_cmd.wakes_on(Wake::TermCommand) && !term_cmd.wakes_on(Wake::Term));
        assert!(!run.wakes_on(Wake::Term) && !run.wakes_on(Wake::TermCommand));
        // metrics is time-driven ONLY: a block/log/run wakeup never re-samples
        // it, and no other topic class re-scans on the heartbeat tick.
        assert!(metrics.wakes_on(Wake::Tick) && !metrics.wakes_on(Wake::Block));
        assert!(!metrics.wakes_on(Wake::Logs) && !metrics.wakes_on(Wake::RunOutput));
        for state in [&module, &files, &logs, &run, &term, &term_cmd] {
            assert!(state.wakes_on(Wake::All));
            assert!(!state.wakes_on(Wake::Tick));
        }
        assert!(metrics.wakes_on(Wake::All));
    }

    #[test]
    fn metrics_topic_subscribes_without_a_store_and_ignores_resume() {
        // metrics rides the exposition source, not the index — a daemon with
        // no index store still serves it, and a reconnect's stored cursor is
        // harmless.
        let (state, lagged) =
            prepare_topic("metrics", NO_SECRET, Some(&"1752000000000".to_string()), None)
                .expect("topic");
        assert!(lagged.is_none());
        assert_eq!(state.cursor(), "0", "a fresh subscribe never resumes");
    }

    #[tokio::test]
    async fn metrics_catch_up_samples_through_the_wired_exposition() {
        // NO actor: the topic samples the handle's wired exposition source
        // directly, so it stays live while the pump is busy (or absent).
        let (handle, _cmds, _hub) = crate::NodeHandle::channel();
        handle
            .status_cell()
            .wire_exposition(|| "ducktape_blocks_total 5\n".to_string());
        let (mut state, _) = prepare_topic("metrics", NO_SECRET, None, None).expect("topic");
        let result = catch_up_metrics("metrics", &mut state, &handle).await;
        assert!(!result.drop_topic);
        match &result.frames[..] {
            [
                ServerFrame::Tail {
                    topic,
                    cursor,
                    item: TailItem::Metrics { time_ms, text },
                },
            ] => {
                assert_eq!(topic, "metrics");
                assert_eq!(text, "ducktape_blocks_total 5\n");
                assert_eq!(cursor, &time_ms.to_string());
                assert_eq!(
                    &state.cursor(),
                    cursor,
                    "the sample instant becomes the topic cursor"
                );
            }
            other => panic!("expected one metrics tail frame, got {other:?}"),
        }
    }

    /// The service link is granted on the TOKEN and nothing else.
    ///
    /// `take_service_link` had no behavioural coverage at all, which left the
    /// deleted build gate's only guard a source lint — and a lint is defeated by
    /// any indirection. This is the direct assertion: present the node's token
    /// and the link is yours, whatever this binary was built from.
    ///
    /// What it CANNOT see: `build_identity()` is `option_env!`, resolved at
    /// compile time, so a test running in a stamped build cannot make the
    /// git-absent case happen. A reintroduced `if build_identity().is_none() {
    /// refuse }` would pass this test and break exactly the checkouts the gate
    /// broke. That specific hole is why the source lint stays — see
    /// `crate::services`'s `no_admission_path_reads_this_node_s_build_stamp`,
    /// which forbids the stamp anywhere in THIS file.
    #[test]
    fn a_service_link_is_granted_on_the_token_alone() {
        const TOKEN: &str = "b0a1c2d3e4f50617";
        let terminals = crate::term::TerminalSessions::new(
            crate::term::TermRing::default(),
            crate::term::TermCommandRing::default(),
            Some(TOKEN.into()),
        );
        let (handle, _cmds, _hub) = crate::NodeHandle::channel();
        let handle = handle.with_terminals(terminals);

        // the whole admission: the right kind, the node's own token.
        let (guard, _rx) = take_service_link(&handle, crate::services::AGENT_KIND, TOKEN)
            .expect("the token alone grants the link");

        // and the two refusals that DO exist, so this test also pins that the
        // grant above is not simply "everything succeeds".
        assert!(take_service_link(&handle, "compute", TOKEN).is_err());
        assert!(take_service_link(&handle, crate::services::AGENT_KIND, "wrong").is_err());
        assert!(
            take_service_link(&handle, crate::services::AGENT_KIND, TOKEN).is_err(),
            "first attach wins while the guard lives"
        );

        // the guard's Drop releases the link — the next daemon may claim it.
        drop(guard);
        take_service_link(&handle, crate::services::AGENT_KIND, TOKEN)
            .expect("a released link is claimable again");
    }

    /// A handle with no terminal plane refuses every link, and that refusal is
    /// about the NODE's wiring, never about a build.
    #[test]
    fn a_node_with_no_terminal_plane_has_no_link_to_give() {
        let (handle, _cmds, _hub) = crate::NodeHandle::channel();
        let Err(refusal) = take_service_link(&handle, crate::services::AGENT_KIND, "any") else {
            panic!("a handle with no terminal plane has no link to give");
        };
        assert!(refusal.contains("terminal sessions are not enabled"), "{refusal}");
    }

    #[tokio::test]
    async fn metrics_catch_up_drops_the_topic_when_no_exposition_is_wired() {
        // no exposition source (an embedder that registers no metrics) — the
        // topic drops with the same `unavailable` shape the http 503 carries.
        let (handle, _cmds, _hub) = crate::NodeHandle::channel();
        let (mut state, _) = prepare_topic("metrics", NO_SECRET, None, None).expect("topic");
        let result = catch_up_metrics("metrics", &mut state, &handle).await;
        assert!(result.drop_topic);
        assert!(matches!(
            result.frames.first(),
            Some(ServerFrame::Error {
                code: StreamErrorCode::Unavailable,
                ..
            })
        ));
    }
}
