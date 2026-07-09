# Live-Stream Protocol — Typed Multiplexed `/v1/ws` — Design of Record

Status: Design of record, approved. Implementation lands as one flag-day PR
against `dev` (node + app move together).
Date: 2026-07-10

## Summary

Today `/v1/ws` pushes a payload-free `{type:"block", height, appHash}` frame,
so the app store refires ~18 parallel queries on **every** block tick and runs
a separate 3s `/v1/status` liveness poll. This spec replaces `/v1/ws` with a
**typed multiplexed topic-subscription WebSocket**: one socket carries
per-module finalized-op feeds (fed from the derived index), feature streams
(node log tail, duckfs change watch, live executor stdout), heartbeats, and
per-topic resume cursors. Frame types are Rust structs exported to TypeScript
via ts-rs — the app consumes generated types, not hand mirrors, on this
surface.

gRPC was considered and rejected for this boundary: the webview is a browser
(gRPC requires a gRPC-Web layer, has no client-streaming, and per-topic HTTP
streams collide with the browser's ~6-connection limit once run-output and log
tails join). A WebSocket is browser-native, works identically in the web build,
and reuses the app's existing reconnect machinery. The topic/cursor contract is
transport-agnostic; if Rust↔Rust component RPC is revisited later, a tonic
service could serve the same shapes.

This is a flag-day change per house style: `WsFrame::Block` and the app's
`onBlock` are deleted, every consumer moves in the same PR. The voice socket
`/v1/call/ws` is untouched.

## Wire contract

Types live in a new `bin/noded/src/stream.rs` (noded is already the shared
app-surface crate for `noded`/`node`/`simnode`). ts-rs derives are test-gated
(`#[cfg_attr(test, derive(ts_rs::TS), ts(export))]`); a `#[test]
export_ts_bindings` writes one committed `app/src/domain/stream.gen.ts`.

Client → server (tag `"op"`, camelCase):

- `subscribe { topics: string[], resume?: {topic → cursor} }` — idempotent;
  re-subscribe re-cursors a topic.
- `unsubscribe { topics: string[] }`.

Server → client (tag `"type"`, camelCase):

- `subscribed { topics: {topic → cursor|null} }` — ack with post-replay
  cursors; the client compares against its resume cursor to know replay depth.
- `event { topic, cursor, op }` — one finalized op record, **verbatim
  `indexer::OpRow`**; `cursor` is its `op/{height:016x}/{seq:04x}` index key.
  A stream event is definitionally an incremental `/v1/index/{m}/ops` read:
  the same record shape, and the cursor doubles as that endpoint's `after`
  parameter.
- `tail { topic, cursor, item }` — one feature-stream item (log line, files
  change summary, stdout chunk).
- `lagged { topic, cursor }` — the catch-up budget was exceeded or the resume
  cursor fell below the retained floor; the stream has jumped to `cursor`; the
  client adopts it and snapshot-refetches the gap over HTTP.
- `heartbeat { height, appHash, timeMs, intervalMs }` — periodic (3s), always
  on, no subscription needed; `intervalMs` lets the client watchdog self-tune.
- `error { topic, code: unknownTopic|unavailable|badCursor, detail }` —
  per-topic refusal; the socket stays open. Malformed client frames get one
  `error` and are otherwise ignored; the server never closes on bad input.

Topics: `module:<id>` (validated against the index store's module catalog),
`logs`, `files:watch`, `run-output:<id>`.

## Node architecture

**StreamHub** replaces the raw `broadcast::Sender<WsFrame>` on `NodeHandle`:
an internal `BlockNote { height, app_hash }` broadcast, a cached tip for
heartbeats (`hub.prime(...)` at boot), the log ring, and the run-output
registry. The `NodeCommand` actor seam is untouched — the hub is read-side
only.

**Catch-up-from-cursor is the core mechanism.** Each connection runs a
`stream_session` select loop (socket recv / block notes / heartbeat interval /
feature-stream wakeups) whose only per-topic state is a cursor. On a
`BlockNote` — or on a harmless internal-broadcast lag — the session scans the
derived index `after=cursor` with a budget (~256 rows/topic/note) and emits
`event`s. Because the hub reads rows *from the index* instead of forwarding
producer data, an event can never precede its index materialization, and a
slow client costs O(cursors), not O(queued frames). Budget exceeded or cursor
below the module's `backfill_height` → `lagged` + cursor jump.

**Producer ordering fix.** `noded`'s `submit_one` and `simnode`'s `commit`
today broadcast the block event *before* `index.apply_block` — a real race
under the old protocol, fatal under this one. The publish moves after apply at
both sites; `bin/node`'s three send sites are already post-apply and swap
mechanically to `hub.publish_block`.

**`logs`** — an in-process `LogRing` (~4096 lines, seq cursors, watch wakeup)
fed by a second tracing fmt layer configured `with_ansi(false)`: frames are
SGR-clean by construction, nobody strips. The node cannot tail `daemon.log`
(that path is the spawner's stdio redirect). Accepted v1 gap: `println!`/
`eprintln!` call sites bypass tracing; migrating them is a follow-up.

**`files:watch`** — a server-side filtered view of `module:files` catch-up
scans: rows decoding as `FilesMsg::Commit` emit `tail` items with
`{height, time, message, baseSnapshot, paths[]}`. Inherits cursor/resume/
lagged semantics for free. Path-prefix filtering stays client-side in v1.

**`run-output:<id>` — live executor stdout (v1 scope, user decision).**
capability-host switches from `wait_with_output()` to piped per-line reads on
stdout+stderr, feeding (a) the accumulated final output (existing
`OracleResult` behavior preserved) and (b) an optional line sink in the
provider config. The node installs a sink writing into a `RunOutputRegistry`
(per-run ring, seq cursors, ~256KB/run cap, retained briefly after exit, LRU
across runs) keyed by the same dispatch/run id the app sees. The topic serves
live `{stream:"stdout"|"stderr", line}` items only on the node hosting the
executor; elsewhere it answers `error{unavailable}`. Run status/results ride
`module:runs`/`module:saga` subscriptions — one topic, one source, no merged
cursors.

**Heartbeat** — per-connection 3000ms interval from the cached tip. Liveness
is time-driven and independent of block flow: an idle chain still beats.

## App architecture

**Transport** (`app/src/domain/transport.ts`): `onBlock` is deleted; the
single shared reconnecting socket gains

```ts
subscribe(topics, handlers: {onEvent?, onTail?, onLagged?}, resume?) => unsubscribe
onStream(listener: (s: {kind:"heartbeat"|"up"|"down", ...}) => void) => unsubscribe
```

with refcounted topic subscriptions (first/last handler sends incremental
subscribe/unsubscribe frames), a cursor map updated from every frame, one
union-subscribe with cursors on socket open (reconnect-with-resume for free),
the existing backoff loop kept verbatim, and a heartbeat dead-man watchdog
(~2.5× `intervalMs`) that closes the socket into the normal backoff path.
A thin hand-written `app/src/domain/stream.ts` holds topic constructors and
frame guards; only it and `transport.ts` import `stream.gen.ts`.

**Store stage 1 — events as change hints.** `refresh()` decomposes into
per-slice fetchers (`store/refresh.ts`) with a module→fetcher map; `event`
frames mark modules dirty and a ~100ms debounce flushes only the dirty slices.
The 3s poll is deleted; heartbeats patch the tip, `down` raises the banner,
and the `up` edge runs one-shot recovery (`status()` → impostor check
preserved verbatim → full refresh). Hints-only is naturally lag-proof: a
`lagged` just refetches that slice. Stage 2 (narrow-apply of op records for
chat/pages) is an explicit follow-up.

**Feature consumers**: Logs tab streams `logs` (remote nodes gain live logs;
the 1.5s daemon.log poll goes away for the log body), FilesView reloads on
`files:watch` while on the live head, and the runs timeline gains an
expandable per-run output pane that subscribes `run-output:<id>` on expand and
unsubscribes on collapse.

**Codegen freshness**: `make stream-types` regenerates `stream.gen.ts` via the
noded export test; `make test` fails on drift (`git diff --exit-code` on the
generated file). ts-rs is a dev-dependency only.

## Error handling

- Unknown/unavailable topics refuse per-topic, never close the socket; the
  affected app slice silently stays fetch-based.
- A resume the server can no longer honor is a `lagged`, not an error — the
  client's contract is "adopt cursor, snapshot-refetch".
- Old app against new node (or vice versa) has no compatible stream; the
  watchdog surfaces it as a persistent connection-down banner rather than a
  silent hang. Acceptable flag-day behavior; release-note it.

## Testing

- Hub unit tests over a real temp `IndexStore`: cursor catch-up, budget
  overflow → `lagged`, resume replay + ack cursors, refusal frames, log-ring
  wrap, heartbeat under paused time, ts-rs export.
- `daemon_e2e`: masked client-frame writer; heartbeat → subscribe →
  submit ops → events byte-match `/v1/index/chat/ops` and cursors page it;
  reconnect-with-resume replays exactly the missed row.
- capability-host: sink receives lines while the child still runs.
- App: transport state-machine suite (routing, refcounts, resume, lagged,
  watchdog), provider hint-refetch suite, simnode scenario + live-daemon e2e
  asserting real frames.
- Gates: per-crate clippy `--tests --no-deps`, `cargo check -p files
  --no-default-features`, `make install`, `make test` (includes the codegen
  drift check).

## Non-goals

- gRPC/tonic anywhere on this surface (revisitable for Rust↔Rust later).
- Stage 2 narrow-apply; `println!`→`tracing` migration; server-side
  `files:watch:<prefix>` filtering; cross-node run-output relay.
- Any change to `/v1` request/response routes, the voice socket, the
  `NodeCommand` seam, or the indexer's dependency rules.
