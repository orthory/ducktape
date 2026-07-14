use std::collections::{BTreeMap, VecDeque};
use std::io::{Result as IoResult, Write};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::extract::ws::{Message, WebSocket};
use duckfs_core::{Change, FilesMsg};
use futures::StreamExt as _;
use futures::channel::oneshot;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, watch};
use tracing_subscriber::fmt::MakeWriter;

use crate::{NodeCommand, NodeHandle};

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
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ClientMsg {
    Subscribe {
        topics: Vec<String>,
        #[serde(default)]
        resume: BTreeMap<String, String>,
    },
    Unsubscribe {
        topics: Vec<String>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ServerFrame {
    Subscribed {
        topics: BTreeMap<String, Option<String>>,
    },
    Event {
        topic: String,
        cursor: String,
        op: StreamOpRow,
    },
    Tail {
        topic: String,
        cursor: String,
        item: TailItem,
    },
    Lagged {
        topic: String,
        cursor: String,
    },
    Heartbeat {
        #[cfg_attr(test, ts(type = "number"))]
        height: u64,
        app_hash: String,
        #[cfg_attr(test, ts(type = "number"))]
        time_ms: u64,
        #[cfg_attr(test, ts(type = "number"))]
        interval_ms: u64,
    },
    Error {
        topic: String,
        code: StreamErrorCode,
        detail: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum StreamErrorCode {
    UnknownTopic,
    Unavailable,
    BadCursor,
    BadFrame,
}

/// Owned mirror of indexer::OpRow's exact serde output.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct StreamOpRow {
    #[cfg_attr(test, ts(type = "number"))]
    pub height: u64,
    #[cfg_attr(test, ts(type = "number"))]
    pub seq: u32,
    #[cfg_attr(test, ts(type = "number"))]
    pub time: u64,
    pub origin: StreamOrigin,
    // skip_serializing_if omits the field on the wire, so the TS side must
    // read `payload?: …` (absent), not `payload: … | null`.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub payload: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub payload_hex: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct StreamOrigin {
    pub kind: StreamOriginKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(optional))]
    pub id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum StreamOriginKind {
    External,
    Module,
    System,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(untagged, rename_all_fields = "camelCase")]
pub enum TailItem {
    Log {
        line: String,
    },
    FileChange {
        #[cfg_attr(test, ts(type = "number"))]
        height: u64,
        #[cfg_attr(test, ts(type = "number"))]
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
        #[cfg_attr(test, ts(type = "number"))]
        time_ms: u64,
        text: String,
    },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum RunStream {
    Stdout,
    Stderr,
}

#[derive(Clone)]
pub struct StreamHub {
    /// block wakeups: subscribers re-scan on any commit and push the fresh
    /// tip (height/app-hash) as a heartbeat frame — `publish_block` primes
    /// `tip` before broadcasting, so the wake always reads its own block.
    blocks: broadcast::Sender<()>,
    tip: Arc<RwLock<Option<(u64, String)>>>,
    logs: LogRing,
    run_output: RunOutputRegistry,
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
        }
    }

    pub fn publish_block(&self, height: u64, app_hash: impl Into<String>) {
        self.prime(height, app_hash);
        let _ = self.blocks.send(());
    }

    pub fn prime(&self, height: u64, app_hash: impl Into<String>) {
        *self.tip.write().expect("stream tip lock poisoned") = Some((height, app_hash.into()));
    }

    pub fn log_ring(&self) -> LogRing {
        self.logs.clone()
    }

    pub fn run_output(&self) -> RunOutputRegistry {
        self.run_output.clone()
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
#[serde(rename_all = "camelCase")]
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
            Self::Logs { seq } | Self::RunOutput { seq, .. } => seq.to_string(),
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
            Wake::Tick => matches!(self, Self::Metrics { .. }),
        }
    }
}

pub async fn stream_session(mut socket: WebSocket, handle: NodeHandle) {
    let hub = handle.stream_hub();
    let mut block_rx = hub.subscribe_blocks();
    let mut log_rx = hub.log_ring().subscribe();
    let mut run_rx = hub.run_output().subscribe();
    let mut heartbeat = tokio::time::interval(Duration::from_millis(HEARTBEAT_INTERVAL_MS));
    let mut topics = BTreeMap::new();

    loop {
        tokio::select! {
            frame = socket.next() => {
                let Some(frame) = frame else { return };
                match frame {
                    Ok(Message::Text(text)) => {
                        match serde_json::from_str::<ClientMsg>(text.as_str()) {
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
    }
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
            let op = match serde_json::from_slice::<StreamOpRow>(&value) {
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

/// re-sample the node's OpenMetrics exposition through the SAME actor command
/// GET /metrics answers (`NodeCommand::Metrics` — every embedder already
/// handles it), so the stream needs no second registry encoder. one Tail
/// frame per wakeup carrying the whole scrape text; a gone actor drops the
/// topic with the same `unavailable` shape the http lane's 503 carries.
async fn catch_up_metrics(
    topic: &str,
    state: &mut TopicState,
    handle: &NodeHandle,
) -> CatchUpResult {
    let TopicState::Metrics { sampled_ms } = state else {
        return CatchUpResult::keep(Vec::new());
    };
    let (reply, rx) = oneshot::channel();
    if handle.send(NodeCommand::Metrics { reply }).await.is_err() {
        return CatchUpResult::drop(vec![unavailable(topic, "node actor is gone")]);
    }
    let Ok(text) = rx.await else {
        return CatchUpResult::drop(vec![unavailable(topic, "node actor dropped the reply")]);
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
    let (height, app_hash) = hub.tip().unwrap_or_else(|| (0, String::new()));
    ServerFrame::Heartbeat {
        height,
        app_hash,
        time_ms: unix_millis(),
        interval_ms: HEARTBEAT_INTERVAL_MS,
    }
}

fn unix_millis() -> u64 {
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
    use std::fs;

    use indexer::{AppliedOp, BlockOps, OriginTag, RebuildMeta};
    use serde_json::json;
    use ts_rs::TS;

    use super::*;

    fn temp_store(modules: &[&str]) -> (tempfile::TempDir, Arc<indexer::IndexStore>) {
        let dir = tempfile::TempDir::new().expect("temp index dir");
        let store = indexer::IndexStore::open(dir.path(), modules).expect("open index");
        (dir, Arc::new(store))
    }

    fn apply_chat(store: &indexer::IndexStore, height: u64, payloads: Vec<serde_json::Value>) {
        let ops = payloads
            .into_iter()
            .map(|payload| AppliedOp {
                module: "chat".into(),
                origin: OriginTag::external("tester"),
                payload: serde_json::to_vec(&payload).expect("payload json"),
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
        store
            .mark_backfilled(
                "chat",
                RebuildMeta {
                    height: 10,
                    time: 0,
                },
            )
            .expect("mark backfilled");
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
                app_hash,
                interval_ms,
                ..
            } => {
                assert_eq!(height, 7);
                assert_eq!(app_hash, "abc");
                assert_eq!(interval_ms, HEARTBEAT_INTERVAL_MS);
            }
            other => panic!("expected heartbeat, got {other:?}"),
        }
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
        assert!(module.wakes_on(Wake::Block) && files.wakes_on(Wake::Block));
        assert!(!logs.wakes_on(Wake::Block) && !run.wakes_on(Wake::Block));
        assert!(logs.wakes_on(Wake::Logs) && !module.wakes_on(Wake::Logs));
        assert!(run.wakes_on(Wake::RunOutput) && !files.wakes_on(Wake::RunOutput));
        // metrics is time-driven ONLY: a block/log/run wakeup never re-samples
        // it, and no other topic class re-scans on the heartbeat tick.
        assert!(metrics.wakes_on(Wake::Tick) && !metrics.wakes_on(Wake::Block));
        assert!(!metrics.wakes_on(Wake::Logs) && !metrics.wakes_on(Wake::RunOutput));
        for state in [&module, &files, &logs, &run] {
            assert!(state.wakes_on(Wake::All));
            assert!(!state.wakes_on(Wake::Tick));
        }
        assert!(metrics.wakes_on(Wake::All));
    }

    #[test]
    fn metrics_topic_subscribes_without_a_store_and_ignores_resume() {
        // metrics rides the actor lane, not the index — a daemon with no index
        // store still serves it, and a reconnect's stored cursor is harmless.
        let (state, lagged) =
            prepare_topic("metrics", Some(&"1752000000000".to_string()), None).expect("topic");
        assert!(lagged.is_none());
        assert_eq!(state.cursor(), "0", "a fresh subscribe never resumes");
    }

    #[tokio::test]
    async fn metrics_catch_up_samples_through_the_actor_lane() {
        let (handle, mut cmds, _hub) = crate::NodeHandle::channel();
        tokio::spawn(async move {
            while let Some(cmd) = cmds.next().await {
                if let NodeCommand::Metrics { reply } = cmd {
                    let _ = reply.send("ducktape_blocks_total 5\n".to_string());
                }
            }
        });
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
    async fn metrics_catch_up_drops_the_topic_when_the_actor_is_gone() {
        let (handle, cmds, _hub) = crate::NodeHandle::channel();
        drop(cmds); // the actor never ran (or exited) — the command lane is closed
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

    #[test]
    fn export_ts_bindings() {
        let header = "// GENERATED by `make stream-types` (bin/noded/src/stream.rs, \
            bin/noded/src/lib.rs call-control types) — do not edit.\n\n";
        let cfg = ts_rs::Config::default();
        let decls = [
            serde_json::Value::decl(&cfg),
            ClientMsg::decl(&cfg),
            ServerFrame::decl(&cfg),
            StreamErrorCode::decl(&cfg),
            StreamOpRow::decl(&cfg),
            StreamOrigin::decl(&cfg),
            StreamOriginKind::decl(&cfg),
            TailItem::decl(&cfg),
            RunStream::decl(&cfg),
            crate::CallClientControl::decl(&cfg),
            crate::CallServerControl::decl(&cfg),
        ];
        let mut out = header.to_string();
        for decl in decls {
            out.push_str("export ");
            out.push_str(&decl);
            if !decl.ends_with('\n') {
                out.push('\n');
            }
            out.push('\n');
        }
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../app/src/domain/stream.gen.ts"
        );
        if fs::read_to_string(path).ok().as_deref() != Some(out.as_str()) {
            fs::write(path, out).expect("write stream.gen.ts");
        }
    }
}
