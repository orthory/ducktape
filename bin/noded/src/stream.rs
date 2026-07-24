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
use tokio::sync::{broadcast, watch};
use tracing_subscriber::fmt::MakeWriter;

use crate::NodeHandle;

/// the TIMER beat: the liveness floor while no blocks commit, and (×2.5) the
/// client watchdog's timeout basis. a heartbeat frame also rides every block
/// wake, so on a moving chain the tip reaches clients per block, not per tick.
pub const HEARTBEAT_INTERVAL_MS: u64 = 3_000;
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ClientMsg {
    Subscribe {
        topics: Vec<String>,
        #[serde(default)]
        resume: BTreeMap<String, String>,
    },
    Unsubscribe {
        topics: Vec<String>,
    },
    /// keystrokes for an interactive terminal session (see `crate::term`).
    /// `data` is base64 of the raw bytes to write to the session's pty. An
    /// unknown/unentitled session id is dropped with a `warn`, never a panic.
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
}

// Serialize-only: the node SENDS frames and never parses its own, so there is
// no `Deserialize` to conflict when [`Self::TermChunk`] shares the `event` tag
// with [`Self::Event`] (a derived deserializer's tag match would be ambiguous).
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerFrame {
    Subscribed {
        topics: BTreeMap<String, Option<String>>,
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

#[derive(Clone)]
pub struct StreamHub {
    /// block wakeups: subscribers re-scan on any commit and push the fresh
    /// tip (height/root-hash) as a heartbeat frame — `publish_block` primes
    /// `tip` before broadcasting, so the wake always reads its own block.
    blocks: broadcast::Sender<()>,
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

    pub fn publish_block(&self, height: u64, root_hash: impl Into<String>) {
        self.prime(height, root_hash);
        let _ = self.blocks.send(());
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

    pub(crate) fn subscribe_blocks(&self) -> broadcast::Receiver<()> {
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
    pub fn output_sink(&self) -> capability_host::OutputSink {
        let registry = self.clone();
        Arc::new(move |ctx, line| {
            let Some(run_key) = ctx.run_key.as_deref() else {
                return;
            };
            let stream = match line.stream {
                capability_host::OutputStream::Stdout => RunStream::Stdout,
                capability_host::OutputStream::Stderr => RunStream::Stderr,
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
    let mut topics = BTreeMap::new();

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
                match note {
                    Ok(()) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => return,
                }
                // the tip rides every block wake — nop fillers included, which
                // feed no topic — so a console's height ticks per block instead
                // of waiting out the timer beat below (the idle/stall floor).
                if !send_frame(&mut socket, heartbeat_frame(&hub)).await {
                    return;
                }
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
        }
    }
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
        } => subscribe_topics(handle, topics, requested, &resume),
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
        | ClientMsg::TermCommand { .. } => Vec::new(),
    }
}

/// a ws connection may drive a terminal session only if it has SUBSCRIBED to
/// that session's `term:<id>` output topic. Subscribing is the connection's
/// proof it legitimately holds the id: create is HTTP-gated (`origin_guard`),
/// and this gate stops a trusted-local client that merely knows or guesses an id
/// from driving another member's session. The app subscribes to the topic
/// before it ever sends input (see `TerminalView`, and the ws frames are ordered
/// on one socket), so this never breaks its flow.
fn term_entitled(topics: &BTreeMap<String, TopicState>, session: &str) -> bool {
    topics.contains_key(&crate::term::topic(session))
}

/// write base64-decoded keystrokes to a session's pty. Refused (no-op + `warn`)
/// when the connection isn't subscribed to the session (`unentitled_session`),
/// the terminal plane is absent, the session is unknown, or the base64 is bad —
/// never a panic. Never logs the bytes; the unentitled refusal logs no id (an
/// id the caller isn't entitled to is not the node's to echo into the
/// webview-streamed log ring).
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

async fn handle_term_input(
    handle: &NodeHandle,
    topics: &BTreeMap<String, TopicState>,
    session: &str,
    data_b64: &str,
) {
    if !term_entitled(topics, session) {
        tracing::warn!(target: "ducktape::term", reason = "unentitled_session", "term input dropped");
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
    let Some(live) = terminals.session(session) else {
        tracing::warn!(target: "ducktape::term", session = %session, reason = "unknown_session", "term input dropped");
        return;
    };
    // raw keystrokes are the SINGLE-session path only. A shared session refuses
    // them so nothing bypasses its ordered command lane (drive it with TermCommand).
    if terminals.mode(session) != Some(crate::term::SessionMode::Single) {
        tracing::warn!(target: "ducktape::term", session = %session, reason = "raw_input_on_shared", "term input dropped");
        return;
    }
    let Ok(bytes) = STANDARD.decode(data_b64) else {
        tracing::warn!(target: "ducktape::term", session = %session, reason = "bad_base64", "term input dropped");
        return;
    };
    if let Err(err) = live.write_all(&bytes).await {
        tracing::warn!(target: "ducktape::term", session = %session, reason = "write_failed", error = %err, "term input dropped");
    }
}

/// resize a session's pty. Same entitlement gate + no-op-on-unknown discipline
/// as input.
async fn handle_term_resize(
    handle: &NodeHandle,
    topics: &BTreeMap<String, TopicState>,
    session: &str,
    cols: u16,
    rows: u16,
) {
    if !term_entitled(topics, session) {
        tracing::warn!(target: "ducktape::term", reason = "unentitled_session", "term resize dropped");
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
    let Some(live) = terminals.session(session) else {
        tracing::warn!(target: "ducktape::term", session = %session, reason = "unknown_session", "term resize dropped");
        return;
    };
    if let Err(err) = live.resize(cols, rows) {
        tracing::warn!(target: "ducktape::term", session = %session, reason = "resize_failed", error = %err, "term resize dropped");
    }
}

/// enqueue a submitted COMMAND onto a session's ordered command lane (the
/// `CommandSource` seam). Gated exactly like [`handle_term_input`]: the
/// connection must be subscribed to the session's `term:<id>` output topic
/// (`term_entitled`, M6). Refused (no-op + `warn`) when unentitled or the
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
    if !term_entitled(topics, session) {
        tracing::warn!(target: "ducktape::term", reason = "unentitled_session", "term command dropped");
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
) -> Vec<ServerFrame> {
    let store = handle.stream_index();
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
        match prepare_topic(&topic, resume.get(&topic), store.as_ref()) {
            Ok((state, lagged)) => {
                accepted.insert(topic.clone(), Some(state.cursor()));
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

#[allow(clippy::result_large_err)]
fn prepare_topic(
    topic: &str,
    resume: Option<&String>,
    store: Option<&Arc<indexer::IndexStore>>,
) -> Result<(TopicState, Option<ServerFrame>), ServerFrame> {
    if let Some(module) = topic.strip_prefix("module:") {
        let store = store.ok_or_else(|| unavailable(topic, "no index store configured"))?;
        if !store.module_ids().any(|id| id == module) {
            return Err(unknown_topic(topic));
        }
        let (cursor, lagged) = module_start_cursor(topic, module, resume, store)?;
        return Ok((
            TopicState::Module {
                module: module.to_string(),
                cursor,
            },
            lagged,
        ));
    }
    if topic == "files:watch" {
        let store = store.ok_or_else(|| unavailable(topic, "no index store configured"))?;
        if !store.module_ids().any(|id| id == "files") {
            return Err(unknown_topic(topic));
        }
        let (cursor, lagged) = module_start_cursor(topic, "files", resume, store)?;
        return Ok((TopicState::FilesWatch { cursor }, lagged));
    }
    if topic == "logs" {
        let seq = match resume {
            Some(cursor) => parse_seq_cursor(topic, cursor)?,
            None => 0,
        };
        return Ok((TopicState::Logs { seq }, None));
    }
    if let Some(id) = topic.strip_prefix("run-output:") {
        let seq = match resume {
            Some(cursor) => parse_seq_cursor(topic, cursor)?,
            None => 0,
        };
        return Ok((
            TopicState::RunOutput {
                id: id.to_string(),
                seq,
            },
            None,
        ));
    }
    if let Some(session) = topic.strip_prefix("term-cmd:") {
        // the ordered command log — like `term:`, any session id subscribes
        // (unknown/evicted → empty catch-up, never an error). Checked before
        // `term:` (non-colliding prefixes, but clearer this way).
        let seq = match resume {
            Some(cursor) => parse_seq_cursor(topic, cursor)?,
            None => 0,
        };
        return Ok((
            TopicState::TermCommand {
                session: session.to_string(),
                seq,
            },
            None,
        ));
    }
    if let Some(session) = topic.strip_prefix("term:") {
        // like run-output, any session id subscribes (unknown/evicted → empty
        // catch-up, never an error); the manager gates who may CREATE one.
        let seq = match resume {
            Some(cursor) => parse_seq_cursor(topic, cursor)?,
            None => 0,
        };
        return Ok((
            TopicState::Term {
                session: session.to_string(),
                seq,
            },
            None,
        ));
    }
    if topic == "metrics" {
        // a resume cursor is accepted but meaningless for a snapshot topic:
        // every (re)subscribe starts from a fresh sample, never a replay.
        return Ok((TopicState::Metrics { sampled_ms: 0 }, None));
    }
    Err(unknown_topic(topic))
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
            let row = match serde_json::from_slice::<StreamOpRow>(&value) {
                Ok(row) => row,
                Err(_) => {
                    frames.push(unavailable(
                        topic,
                        "stored op row was not json — rebuild the index",
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

fn unknown_topic(topic: &str) -> ServerFrame {
    ServerFrame::Error {
        topic: topic.to_string(),
        code: StreamErrorCode::UnknownTopic,
        detail: "unknown stream topic".into(),
    }
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
        let (state, lagged) = prepare_topic("module:chat", None, Some(&store)).expect("topic");
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
            prepare_topic("module:chat", None, None),
            Err(ServerFrame::Error {
                code: StreamErrorCode::Unavailable,
                ..
            })
        ));
        let (_dir, store) = temp_store(&["chat"]);
        assert!(matches!(
            prepare_topic("module:nope", None, Some(&store)),
            Err(ServerFrame::Error {
                code: StreamErrorCode::UnknownTopic,
                ..
            })
        ));
        assert!(matches!(
            prepare_topic("logs", Some(&"not-a-seq".to_string()), Some(&store)),
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
        let (state, lagged) = prepare_topic("term:abc", None, None).expect("term topic subscribes");
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
            prepare_topic("term-cmd:abc", None, None).expect("term-cmd topic subscribes");
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
    fn term_input_requires_a_subscription_to_the_session_topic() {
        let mut topics: BTreeMap<String, TopicState> = BTreeMap::new();
        // a connection that never subscribed is not entitled to drive a session.
        assert!(!term_entitled(&topics, "sess1"));
        // subscribing to a session's OWN output topic entitles input to it — and
        // only it: holding one session's topic doesn't entitle another's.
        topics.insert(
            crate::term::topic("sess1"),
            TopicState::Term {
                session: "sess1".into(),
                seq: 0,
            },
        );
        assert!(term_entitled(&topics, "sess1"));
        assert!(!term_entitled(&topics, "sess2"));
        // a non-terminal subscription never entitles terminal input.
        topics.insert("logs".into(), TopicState::Logs { seq: 0 });
        assert!(!term_entitled(&topics, "logs"));
    }

    #[test]
    fn subscription_cap_refuses_new_topics_but_allows_recursoring() {
        let (handle, _rx, _hub) = crate::NodeHandle::channel();
        let mut states = BTreeMap::new();
        let requested: Vec<String> = (0..MAX_TOPICS_PER_CONNECTION + 1)
            .map(|i| format!("run-output:r{i}"))
            .collect();
        let frames = subscribe_topics(&handle, &mut states, requested, &BTreeMap::new());
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
            prepare_topic("metrics", Some(&"1752000000000".to_string()), None).expect("topic");
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
        let (mut state, _) = prepare_topic("metrics", None, None).expect("topic");
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

    #[tokio::test]
    async fn metrics_catch_up_drops_the_topic_when_no_exposition_is_wired() {
        // no exposition source (an embedder that registers no metrics) — the
        // topic drops with the same `unavailable` shape the http 503 carries.
        let (handle, _cmds, _hub) = crate::NodeHandle::channel();
        let (mut state, _) = prepare_topic("metrics", None, None).expect("topic");
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
