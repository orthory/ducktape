# Live-Stream `/v1/ws` — Implementation Plan (work orders)

Spec: `docs/superpowers/specs/2026-07-10-live-stream-ws-design.md`. Flag-day:
`WsFrame::Block` and the app's `onBlock` are deleted; everything moves in one
PR. Voice (`/v1/call/ws`) untouched.

Verified anchors (line numbers at branch base `a6fc6e8a`):
- `bin/noded/src/lib.rs`: `WsFrame` :551, `NodeHandle` :586 (`events` field),
  `channel()` :624, `ws` :1994, `stream_frames` :1999, `index_ops` :1137,
  `EVENT_BUFFER` :57.
- Producers: `bin/noded/src/main.rs` `submit_one` sends at :496 BEFORE
  `index.apply_block` :524 (must move after); `bin/simnode/src/main.rs`
  `commit` sends at :639 BEFORE apply :676 (same fix); `bin/node/src/main.rs`
  :7481, :7869, :9867 already post-apply (mechanical swap).
- `indexer::OpRow` serde shape (camelCase): `{height, seq, time,
  origin: {kind: "external"|"module"|"system", id?}, payload?, payloadHex?}`;
  op keys `op/{height:016x}/{seq:04x}` (lexicographic = numeric); `Page
  {entries, has_more, next_after}`; `store.scan(module, OP_PREFIX, after,
  limit)`; watermarks `applied_height`, floor `backfill_height`.
- `capability-host/src/lib.rs:338`: `tokio::join!(feed,
  child.wait_with_output())` — no live tail today.
- `bin/node` tracing init: `bin/node/src/main.rs:2971`
  (`tracing_subscriber::fmt()`... stderr).

## Work order A — node-side protocol (bin/noded, bin/node, bin/simnode)

### A1. New module `bin/noded/src/stream.rs`

Constants: `HEARTBEAT_INTERVAL_MS = 3_000`, `STREAM_CATCHUP_BUDGET = 256`
(rows per topic per wakeup), `LOG_RING_CAPACITY = 4_096` (lines),
`RUN_OUTPUT_MAX_RUNS = 32`, `RUN_OUTPUT_MAX_LINES = 2_048` (per run).

Wire types, all `#[derive(Clone, Debug, Serialize, Deserialize)]` plus
`#[cfg_attr(test, derive(ts_rs::TS))]`. camelCase fields; enum tags below.
Every `u64`/`u32` field carries `#[cfg_attr(test, ts(type = "number"))]`
(serde_json emits numbers; the TS side must not see bigint).

```rust
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ClientMsg {
    Subscribe { topics: Vec<String>, #[serde(default)] resume: BTreeMap<String, String> },
    Unsubscribe { topics: Vec<String> },
}

#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ServerFrame {
    Subscribed { topics: BTreeMap<String, Option<String>> },
    Event { topic: String, cursor: String, op: StreamOpRow },
    Tail { topic: String, cursor: String, item: TailItem },
    Lagged { topic: String, cursor: String },
    Heartbeat { height: u64, app_hash: String, time_ms: u64, interval_ms: u64 },
    Error { topic: String, code: StreamErrorCode, detail: String },
}

#[serde(rename_all = "camelCase")]
pub enum StreamErrorCode { UnknownTopic, Unavailable, BadCursor, BadFrame }

/// owned mirror of indexer::OpRow's exact serde output.
#[serde(rename_all = "camelCase")]
pub struct StreamOpRow {
    pub height: u64,
    pub seq: u32,
    pub time: u64,
    pub origin: StreamOrigin,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_hex: Option<String>,
}
#[serde(rename_all = "camelCase")]
pub struct StreamOrigin { pub kind: StreamOriginKind, #[serde(skip_serializing_if = "Option::is_none")] pub id: Option<String> }
#[serde(rename_all = "camelCase")]
pub enum StreamOriginKind { External, Module, System }

#[serde(untagged)]  // topic discriminates; TS side narrows by topic
pub enum TailItem {
    Log { line: String },
    FileChange { height: u64, time: u64, message: String, base_snapshot: Option<String>, paths: Vec<String> },
    RunOutput { stream: RunStream, line: String },
}
#[serde(rename_all = "camelCase")]
pub enum RunStream { Stdout, Stderr }
```

Topic grammar + parsing: `module:<id>`, `logs`, `files:watch`,
`run-output:<id>`. Unknown grammar → `Error{unknownTopic}`. `module:<id>`
whose id is not in `store.module_ids()` → `Error{unknownTopic}`. Module topics
and `files:watch` on a handle with `index: None` → `Error{unavailable}`.
`run-output:<id>` accepts ANY id (subscribing before the executor spawns is
legal; frames flow when the ring appears — no unavailable race).

`StreamHub` (cheap Clone) replaces `NodeHandle.events`:
- `blocks: tokio::sync::broadcast::Sender<BlockNote>` where
  `struct BlockNote { height: u64, app_hash: String }` (internal, not wire).
  Buffer = existing `EVENT_BUFFER`.
- cached tip `Arc<std::sync::RwLock<Option<(u64, String)>>>`.
- `logs: LogRing`, `run_output: RunOutputRegistry` (both Arc-backed clones).
- `publish_block(height, app_hash)` — set tip, then send note (send may fail
  with no subscribers; fine). `prime(height, app_hash)` — set tip only.

`LogRing`: `Arc<Mutex<VecDeque<(u64 seq, String line)>>>` capped at
`LOG_RING_CAPACITY` + `tokio::sync::watch::<u64>` latest-seq for wakeups.
`push_line`, `read_after(seq, budget) -> (Vec<(u64, String)>, floor_seq)`,
`latest_seq()`. Also a `MakeWriter` impl (`LogRingWriter`) so a
`tracing_subscriber::fmt` layer with `.with_ansi(false)` feeds it — buffered
per-write, complete lines pushed on write/flush/drop.

`RunOutputRegistry`: `Arc<Mutex<BTreeMap<String, RunRing>>>` (LRU-evict past
`RUN_OUTPUT_MAX_RUNS`; each `RunRing` a seq-numbered `VecDeque` capped at
`RUN_OUTPUT_MAX_LINES`) + one global `watch::<u64>` version bump on any
append. `append(id, stream, line)`, `read_after(id, seq, budget)` (missing id
→ empty, floor 0).

`stream_session(socket, handle)` — per-connection task replacing
`stream_frames`. State: `BTreeMap<String, TopicCursor>` where cursor is a
`String` for module/files topics (op key) and `u64` seq for logs/run-output.
`tokio::select!` over: socket recv; `blocks.subscribe()` recv (treat
`RecvError::Lagged` as a normal wakeup — the scan-from-cursor design makes it
harmless); heartbeat `tokio::time::interval(HEARTBEAT_INTERVAL_MS)`; log-ring
watch changed; run-output watch changed.

Catch-up semantics (the core):
- module topic: loop `store.scan(module, OP_PREFIX, after=cursor, limit)` in
  pages, emitting `Event` per row (row bytes → `StreamOpRow` via
  `serde_json::from_slice`; a row that fails to parse = damaged store → emit
  `Error{unavailable, detail:"stored op row was not json — rebuild the
  index"}` and drop the topic). After `STREAM_CATCHUP_BUDGET` rows in one
  wakeup with `has_more` still true → emit `Lagged{cursor: jump}` and set
  cursor to `jump = format!("op/{:016x}/ffff", applied_height(module))`.
- `files:watch`: same scan over module `files`, independent cursor; emit only
  rows whose payload decodes as a files Commit op, as
  `TailItem::FileChange` (reuse whatever wire type `files_http.rs` already
  imports to decode; extract per-change paths). Non-commit rows advance the
  cursor silently and do NOT count against the budget.
- `logs`: `read_after(seq, budget)` loop; ring wrap (resume seq < floor) →
  `Lagged{cursor: floor.to_string()}` then stream from floor.
- `run-output:<id>`: same as logs against the run's ring.

Subscribe handling: idempotent; re-subscribe re-cursors. Resume rules —
module/files topics: cursor must start with `op/` (else `Error{badCursor}`,
topic not added); a cursor whose height < `backfill_height(module)` floor →
immediate `Lagged` jump to the live watermark. NO resume given → start at the
live watermark (`op/{applied:016x}/ffff`) — no replay (clients snapshot-fetch
anyway). `logs`/`run-output` with no resume → replay the retained ring from
seq 0 (that IS the scrollback the consumers want). After processing a
subscribe, reply `Subscribed{topics: {topic → Some(current cursor)}}` with
post-replay cursors, then run an immediate catch-up pass.

Unsubscribe: drop topics; no ack. Malformed client text frame → one
`Error{topic:"", badFrame}`, never close. Binary client frames ignored.
Outbound send failure → session ends (client hung up).

Heartbeat: every tick, `Heartbeat{height, app_hash, time_ms:
unix-millis, interval_ms: HEARTBEAT_INTERVAL_MS}` from the cached tip;
unprimed tip → `height: 0, app_hash: ""` (clients ignore tip at height 0 but
still treat the frame as liveness).

### A2. `bin/noded/src/lib.rs` rewiring

- Delete `WsFrame` + `stream_frames`; `pub mod stream;` re-export the wire
  types. `NodeHandle.events: broadcast::Sender<WsFrame>` → `hub: StreamHub`;
  `channel()` returns `(Self, mpsc::Receiver<NodeCommand>, StreamHub)`.
- `ws` handler: `upgrade.on_upgrade(move |socket| stream::stream_session(socket, handle))`
  (handle cloned in; the session reads `handle.index` for scans and
  `handle.hub` for notes/rings — expose what stream.rs needs via
  `pub(crate)` accessors rather than making fields public).
- Update the `EVENT_BUFFER` doc comment (it now sizes the internal BlockNote
  broadcast).

### A3. Producers

- `bin/noded/src/main.rs`: `run_node`/`submit_one` take the `StreamHub`
  instead of `broadcast::Sender<WsFrame>`; the publish moves from :496 to
  AFTER the `index.apply_block` block at :524 (publish even when apply
  errored — the note still drives tip/heartbeat; no rows appear because none
  were written). Prime the hub once the recovered boot height is known inside
  `run_node`.
- `bin/simnode/src/main.rs`: same swap — publish after apply (:639 below
  :676); `SimState.events`/`run_sim` thread the hub through.
- `bin/node/src/main.rs`: replace the three `http_events.send(...)` sites
  (:7481, :7869, :9867) with `hub.publish_block(height, hex(...))`; rename the
  binding from `http_events` to `stream_hub`. Prime near the index wiring
  (~:6002) from `index.resume_height()` with an empty app hash.
- `bin/node` tracing: convert the init at :2971 to
  `tracing_subscriber::registry()` with (a) the existing stderr fmt layer
  (same env filter + ansi behavior as today) and (b) a ring fmt layer
  (`with_ansi(false)`, writer = the hub's `LogRing`). The hub must therefore
  exist before that init OR the ring is a process-global handed to the hub —
  prefer constructing `LogRing` first, passing it to both the subscriber init
  and `NodeHandle::channel()`/hub (adjust `channel()` to accept or expose the
  ring; keep it simple: `channel()` builds the ring, `hub.log_ring()` clone is
  handed to the tracing init; init happens after `channel()` in bin/node —
  verify order, `channel()` is at :5983 which is AFTER :2971, so instead give
  `LogRing::default()` a standalone constructor + `NodeHandle::channel_with_log_ring(ring)`.
  noded/simnode use plain `channel()` which builds its own ring).
- `bin/noded/src/main.rs`: add a minimal tracing init (ring layer only) using
  the same pattern so the `logs` topic answers on the embedded daemon
  (accepted gap: `println!` lines bypass it).

### A4. ts-rs export + Makefile

- `bin/noded/Cargo.toml`: `[dev-dependencies] ts-rs = { version = "11",
  features = ["serde-compat", "serde-json-impl"] }` (pick the current major;
  `serde-compat` honors rename_all/tag attributes).
- `#[test] fn export_ts_bindings()` in stream.rs: compose ONE file
  `app/src/domain/stream.gen.ts` (path via
  `concat!(env!("CARGO_MANIFEST_DIR"), "/../../app/src/domain/stream.gen.ts")`)
  from the `TS::decl()` of: ClientMsg, ServerFrame, StreamErrorCode,
  StreamOpRow, StreamOrigin, StreamOriginKind, TailItem, RunStream — each
  prefixed `export `, with a header:
  `// GENERATED by \`make stream-types\` (bin/noded/src/stream.rs) — do not edit.`
  Write only when content differs (idempotent mtime).
- Root `Makefile`: add
  ```make
  ## regenerate app/src/domain/stream.gen.ts from the stream contract
  stream-types:
  	cargo test -p noded export_ts_bindings
  ```
  and, in the `test` target after the cargo tests and before the app suites:
  `$(MAKE) stream-types` + `git diff --exit-code -- app/src/domain/stream.gen.ts`.
  (Match the Makefile's existing variable/tab conventions.)

### A5. Node tests

- Unit (in stream.rs `#[cfg(test)]`, real temp `IndexStore` via `tempfile` —
  check what dev-deps noded already has): catch-up emits exactly the scanned
  rows in order with op-key cursors; budget overflow emits `Lagged` with the
  watermark jump; fresh subscribe starts at live tip (no replay); resume
  below a `mark_backfilled` floor → `Lagged`; unknown topic / no-index-store
  refusals; log ring wrap → `Lagged` then floor replay; run-output ring
  append/read + LRU eviction; heartbeat frame shape under
  `tokio::time::pause()`; `export_ts_bindings`.
- `bin/noded/tests/router.rs`: keep compiling (fake-actor handle now needs
  the hub); add a ws-upgrade-still-works probe if cheap.
- `bin/noded/tests/daemon_e2e.rs`: the raw RFC6455 helpers (`ws_connect`
  :213, `ws_read_text` :240) gain a masked `ws_send_text`; rewrite the block
  assertions (:394-428): connect → first `heartbeat` arrives → subscribe
  `module:chat` (no resume) → `subscribed` ack → submit two chat ops over
  HTTP → two `event` frames arrive whose `op` rows byte-match
  `GET /v1/index/chat/ops` and whose cursors page it (`after=<cursor1>`
  returns exactly row 2) → reconnect with `resume` at cursor1 and assert
  exactly row 2 replays.
- Gates: `cargo clippy -p noded -p node-bin -p simnode --tests --no-deps`;
  `cargo test -p noded`; `cargo check -p files --no-default-features`.

## Work order B — capability-host live stdout (crate-local only)

Scope: `crates/kernel/capability-host/` ONLY (work order A owns bin/*; the
registry wiring joins them afterwards).

- Provider config gains an optional line sink:
  `pub type OutputSink = std::sync::Arc<dyn Fn(OutputLine) + Send + Sync>;`
  `pub struct OutputLine { pub stream: OutputStream, pub line: String }`,
  `pub enum OutputStream { Stdout, Stderr }` — or an equivalent
  `tokio::sync::mpsc::UnboundedSender<OutputLine>`; pick what fits the
  existing config style, keep it optional (None = today's behavior cost-free).
- Replace `child.wait_with_output()` (:338) with piped reads:
  `BufReader::lines()` tasks on stdout and stderr that BOTH accumulate into
  the final output (byte-preserving enough for the existing `OracleResult`
  contract — line-joined with `\n` is acceptable if the current consumers
  only treat output as text; verify how the output is consumed before
  deciding) AND forward each line to the sink as it arrives. `tokio::join!`
  the two readers + `child.wait()`. Preserve every existing behavior:
  timeouts, kill-on-timeout, exit-status handling, the `feed` half.
- Unit test: a provider running a script that prints a line, sleeps, prints
  another — assert the sink receives line 1 BEFORE the child exits (e.g.
  gate the child's second line on a tempfile the test writes after observing
  line 1), and the final accumulated output still matches.
- Gate: `cargo clippy -p capability-host --tests --no-deps`,
  `cargo test -p capability-host`.

## Work order C — wiring run-output (after A+B merge in-branch)

`bin/node/src/oracle_pool.rs` (and any other provider-construction site —
grep `capability_host::` in bin/): install a sink that appends to the hub's
`RunOutputRegistry` keyed by the id the oracle path carries for the run —
find the dispatch/run id available at provider spawn and CONFIRM it equals
the id the app's `pendingRuns` rows expose (runs-client). If they differ,
key by the id the app can see.

## Work order D — app side

As specced (transport subscribe/onStream, store/refresh.ts stage 1,
LogsTab/FilesView/RunOutputPane consumers, transport-stub swap, vitest
suites). Detailed in the spec + the plan file at
`~/.claude/plans/support-grpc-for-components-dazzling-quiche.md`; will be cut
into Codex-sized orders once `stream.gen.ts` exists on the branch.
