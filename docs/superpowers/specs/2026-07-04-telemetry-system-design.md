# Telemetry System — Design

Status: **SUPERSEDED (2026-07-06).** The frame plane (`TelemetryFrame` /
`TelemetryRing` / `GET /v1/telemetry` / `WsFrame::Telemetry` / the console
Telemetry view) was removed: its dispatch trace duplicated the explorer
(`GET /v1/blocks`, the durable block index), its `events` were always empty,
and `consensus_time == height` on the validator lane. What survives is the
Prometheus plane — the shared `noded::NodeMetrics` (`ducktape_*` series behind
`GET /metrics`), now recorded by BOTH the embedded daemon (at submit) and the
consensus validator (at drain), carrying the per-block apply latency this spec
introduced. Phase 2 (the on-consensus telemetry module) remains unbuilt and
unaffected.
Date: 2026-07-04

## Summary

Ducktape gets a telemetry system with **two planes that share one correlation
key space** — `(height, source)`, where `source` is the emitting module id:

- **Observability plane (node-local, non-deterministic).** Per-block operational
  signal from a running `ducktape-node`: which modules were dispatched, their
  causal fan-out, and this node's wall-clock apply latency. Differs per node, so
  it can never live in consensus. **This PR.**
- **On-consensus telemetry module (deterministic, agreed).** A replicated
  product module that records an agreed, verifiable event log into its own
  32-byte root — a bounded hot feed plus a rolling per-source digest of full
  history. Every node agrees. **Deferred to Phase 2.**

Approach chosen: **A — unified key space, observability-first.** The two planes
are built against the same `(height, source)` vocabulary so "what happened"
(consensus) and "how it performed here" (local) join at block+module
granularity. Observability ships first because it is self-contained, carries
zero app-hash risk, and immediately serves fleet/debug work.

## Why this fits the architecture

The determinism boundary is already encoded in the `sdk` module contract:

| SDK primitive | Plane | Behavior |
| --- | --- | --- |
| `Ctx::emit_event(Event)` | Observability | "Leaves the state machine, handed to the effectful node layer, never re-enters as a follow-up." |
| `Ctx::emit_msg(Msg)` | Deterministic | Re-dispatched as a follow-up op → becomes committed module state. |
| `Env { height, consensus_time, origin, me }` | Both | A deterministic correlation-key space every dispatch already sees. |

The host already collects `events: Vec<Event>` per block and hands them back on
`BlockOutcome`; `noded` dropped them. The ws frame type `WsFrame` was already
tagged and documented as extensible. So this is mostly connecting seams left as
stubs, not building from scratch.

## Phase 1 — Observability plane (this PR)

### Kernel: a deterministic dispatch trace

`host::BlockOutcome` gains `dispatches: Vec<DispatchRecord>`, populated in the
drain loop in dispatch (causal) order. Each `DispatchRecord` carries the
`module`, the trigger `origin`, and the intent fan-out (`emitted_msgs`,
`emitted_events`). It is **pure deterministic data** — no wall-clock — so it is
identical on every honest validator and safe to assert in `demo`/e2e. Only
committed blocks yield a `BlockOutcome`, so an aborted block discards its trace
with the block.

`sdk::Origin` gains `PartialEq, Eq` (additive) so the record can be compared.

**Wall-clock never enters the kernel.** The one non-deterministic signal —
per-block apply latency — is measured in `noded` (the effectful layer, where
`unix_millis()` already lives) around `host.submit_at`.

### Daemon: ring buffer, ws frame, pull endpoint

`bin/noded` assembles one `TelemetryFrame` per committed block:
`{ height, consensusTime, latencyUs, dispatches[], events[] }`, where
`dispatches`/`events` come from `BlockOutcome` and `latencyUs` is the local
`Instant` measurement.

- **Buffer:** a bounded `TelemetryRing` (`Arc<Mutex<VecDeque>>`, cap 256,
  drop-oldest) — node-local, like the files blob store; never crosses the actor
  command lane.
- **Live:** the broadcast channel switches from `BlockSummary` to `WsFrame`;
  each block fans out `WsFrame::Block` then `WsFrame::Telemetry`. Old clients
  ignore unknown frame kinds.
- **Pull:** `GET /v1/telemetry?limit=N` returns `{ frames: [...] }` oldest-first
  from the ring — the backfill a client pulls on connect before following ws.

### Console app: a Telemetry view

The React console (`app/`) consumes the frame natively:

- `transport.ts` gains the `TelemetryFrame` types, a `telemetry(limit?)` pull
  (defensive: a node without the surface reads as empty), and an `onTelemetry`
  subscription over the one shared socket.
- `DucktapeProvider` backfills from the ring on connect, then follows the live
  stream into a bounded (200) `state.telemetry`, deduped on strictly-increasing
  height.
- A new registered **Telemetry** view renders recent blocks newest-first: height,
  latency, consensus clock, the dispatch chain (module + origin + fan-out), and
  any emitted events.

### Determinism safety (load-bearing invariant)

Wall-clock lives only in the `noded` frame assembly and is never read back into
module state. The kernel change is pure structural data and cannot affect the
app-hash — asserted by the existing schedule-independence / cross-run e2e, which
are unchanged.

### Out of scope for Phase 1

- **Fleet dashboard tile.** `ops/fleet-console/` polls a static `fleet.json`
  from `ops/fleet.sh` and does not talk to the daemon. Surfacing telemetry there
  is a separate bash/JSON plumb (fetch each instance's `/v1/telemetry`, stamp
  onto `FleetNode`, render a `TelemetryFeed`). Deferred — each app is already
  visible live via VNC, and this keeps the PR a clean daemon↔console wire.

## Phase 1.5 — Prometheus `/metrics` (shipped)

A pull-based scrape surface for operators (Grafana/alerting), complementary to
the live in-app view. commonware's runtime already keeps a `prometheus-client`
registry and its `Metrics` trait exposes `register(...)` + `encode()`, so the
daemon registers its own series **into that registry** and one `context.encode()`
serves everything:

- `GET /metrics` on `noded` (root path, scrape convention) → OpenMetrics text via
  a `NodeCommand::Metrics` round-trip (the actor owns the commonware context that
  holds the registry).
- Ducktape series, folded per committed block in `submit_one`:
  `ducktape_block_height` (gauge), `ducktape_blocks_total` (counter),
  `ducktape_block_apply_latency_seconds` (histogram),
  `ducktape_dispatch_total{module,origin}` (counter — `origin` is the
  low-cardinality kind: external/module/system).
- The same endpoint also serves commonware's runtime metrics (one shared
  registry). `bin/node` (the validator) serves its runtime registry too, but the
  block-derived `ducktape_*` series are the local daemon's surface only.
- No new direct dependency — all via `commonware_runtime::telemetry::metrics`.

## Phase 2 — On-consensus telemetry module (deferred)

New crates `crates/apps/telemetry` + `telemetry-interface`, modeled on `inbox`.

- **Hot feed:** per-source bounded `BTreeMap<seq, Record>`, drop-oldest over cap;
  `next_seq` never rewinds.
- **Digest:** per-source `{ count, rolling_hash }` where
  `rolling_hash = H(prev || record_bytes)` folded in for **every** event
  including aged-out ones — constant size, so the root commits a verifiable
  digest of full history while storage stays bounded.
- **Record:** `{ height, source, seq, origin, kind(≤64B), payload(≤cap) }`; caps
  enforced at `execute` with rejection so oversized bytes never enter the root
  preimage.
- **Write path:** a module `emit_msg`s `TelemetryMsg::Record{kind,payload}` →
  follow-up op → commits atomically in the same block as the cause (platform
  promise P2). The module fills `height/source/seq` from `Env`.
- **Lifecycle:** inbox's staged-overlay → `commit_block`/`abort_block` → `root()`
  from committed state only → `snapshot`/`install` byte-identical to the root
  preimage, so a joiner state-syncs and verifies against the committed root.
- **Query:** `TelemetryQuery::{ Feed{source,limit}, Digest{source} }` over
  `/v1/query`.
- **Wiring:** register in genesis; first real emitter = agent runs.

Correlation with Phase 1 is at `(height, source)` — exact per-record seq join
across planes is explicitly out of scope (YAGNI). The authoritative seq lives
only in the consensus module.

## Testing

- **Kernel:** `dispatch_trace_records_every_dispatch_in_causal_order` asserts the
  trace's order, origins, and fan-out over the relay→kv fixture. Existing
  determinism e2e (schedule-independence, cross-run) unchanged.
- **Daemon:** `TelemetryRing` unit test (cap eviction + `recent` slicing); the
  `daemon_e2e` full-surface test now classifies the interleaved
  `Block`/`Telemetry` ws frames, asserts the dispatch trace + latency, and
  backfills over `GET /v1/telemetry`.
- **Console:** transport tests for the pull endpoint (with `?limit=`), the shared
  telemetry stream, unknown-frame tolerance, and shared-socket lifecycle. All
  `NodeTransport` mocks updated for the two new methods.
- **Gate:** `make test` (workspace Rust incl. cluster/daemon e2e, then app suites
  against a freshly built daemon).

## Explicitly not built (either phase)

OTLP push to an OpenTelemetry collector, distributed tracing spans, cross-node
aggregation, governance-tuned retention, per-event cross-plane seq join.
(Prometheus `/metrics` pull shipped in Phase 1.5.)
