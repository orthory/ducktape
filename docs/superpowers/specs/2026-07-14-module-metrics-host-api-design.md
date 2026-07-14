# Module Metrics Host API — Design of Record

2026-07-14. Status: approved design, pre-implementation.

## Summary

Ducktape modules need to expose feature-specific operational metrics without
putting telemetry state in the world state or teaching the node the semantics
of every module. This design adds a typed, write-only metrics import to the
Wasm module ABI. A module computes semantic observations while it executes;
the host buffers them through replay and block commit; the node operator then
chooses whether to export them through the existing OpenMetrics registry or
discard them.

```text
Wasm/native module
  descriptor catalog + typed observations
                    |
                    v
host replay/transaction buffer
                    |
          successful block commit
                    |
                    v
node-local metric store
                    |
          Prometheus/OpenMetrics | off
```

The load-bearing split is:

- the **module owns meaning**: which operation is a success, what a queue depth
  means, and which bounded dimensions are useful;
- the **host owns correctness**: module identity, replay de-duplication,
  rejection/abort discard, validation, and resource bounds;
- the **node operator owns handling**: whether committed observations are
  retained and exported;
- Prometheus owns time-series history and derived calculations such as rates,
  error ratios, and percentiles.

There is no `observer.wasm`. The runtime module itself emits observations, and
the host performs the generic Counter/Gauge/Histogram aggregation.

## Goals

1. Let native and Wasm modules publish their own bounded operational metrics.
2. Keep metric state outside module roots, snapshots, app hashes, recovery
   manifests, and state sync.
3. Preserve exactly one observation for a successful dispatch despite Wasm
   memoized replay and batch member replay.
4. Publish nothing from rejected operations, aborted blocks, or failed replay
   rounds.
5. Make metric handling invisible to module logic and optional for each node
   operator.
6. Reuse the node's existing Commonware OpenMetrics registry and `/metrics`
   surface.
7. Bound memory, work, and label cardinality at the host boundary.

## Non-goals

- Business analytics, usage reporting, or user-facing totals.
- Exporting user ids, document ids, task ids, peer keys, free-form errors, or
  other unbounded values as labels.
- Storing process-lifetime counters in consensus state merely to survive a
  restart.
- Exact-once metric delivery across a process crash.
- Making telemetry part of consensus validity or app-hash composition.
- Adding OTLP, dashboards, alert rules, or a general observer plug-in runtime
  in the first implementation.
- Replacing structured events, logs, or traces. Metrics answer aggregate
  operational questions; events explain individual occurrences.
- Adding Tauri/UI surfaces or host CPU, memory, and disk metrics. This contract
  belongs to the node layer; machine exporters own host-resource telemetry.

## Existing constraints

The design follows four existing implementation facts:

1. `sdk::Module` is a deterministic state machine and `sdk::Ctx` is its host
   capability surface.
2. `crates/kernel/module-guest/wit/module.wit` deliberately exposes no clock,
   RNG, network, or ambient host state.
3. `WasmModule` creates a fresh component instance per call. A guest therefore
   cannot retain a metric tracker in guest memory across dispatches.
4. A state or sibling read miss pauses a Wasm call and replays it from the
   beginning. `out_msgs` and `out_events` are already recreated per round and
   only the final successful round is published. Metric observations must use
   the same rule.

The node already registers `ducktape_*` series in the Commonware runtime
registry. `bin/node/src/plane_metrics.rs` also proves the dynamic pattern this
feature needs: a custom `EncodeMetric` reads a shared snapshot at scrape time,
and dropping its `Registered` handle removes the series.

## State placement

Metric data lives in exactly three non-consensus places:

```text
per-round/per-block buffers     ephemeral; discarded on replay/abort
node metric store              process-local; resets on restart
Prometheus TSDB                external retention chosen by the operator
```

World state continues to contain only domain state required for deterministic
module behavior. A module must not add a durable count, histogram, or last-seen
field solely to support telemetry.

Process-local Counter and Histogram values may reset at restart; Prometheus
counter reset handling is the continuity mechanism. A Gauge may be restored
from already-existing committed domain state when it is cheap to do so. If a
supposed metric must be durable, queryable, and identical across every node,
it is domain state or an index—not telemetry.

## SDK contract

The SDK adds backend-neutral metric types. They do not mention Prometheus,
Commonware, HTTP, or a node configuration.

```rust
pub struct MetricDescriptor {
    pub name: String,
    pub help: String,
    pub kind: MetricKind,
    pub labels: Vec<String>,
}

pub enum MetricKind {
    Counter,
    Gauge,
    Histogram { buckets: Vec<f64> },
}

pub struct MetricObservation {
    pub metric: u32,
    pub label_values: Vec<String>,
    pub value: MetricValue,
}

pub enum MetricValue {
    CounterAdd(u64),
    GaugeSet(f64),
    HistogramObserve(f64),
}
```

`metric` is the zero-based index in the module's descriptor catalog. Label
values are positional and must exactly match the descriptor's label keys. A
guest never supplies its module id, external metric name, label keys, or metric
type on the hot path; the host supplies or validates all four.

`sdk::Module` gains two default-no-op methods:

```rust
fn metric_descriptors(&self) -> Vec<MetricDescriptor> {
    Vec::new()
}

async fn collect_metrics(&self) -> Result<Vec<MetricObservation>, Error> {
    Ok(Vec::new())
}
```

`collect_metrics` is a read-only, best-effort snapshot hook for Gauge recovery.
It may only emit `GaugeSet`. The returned observations are the complete current
Gauge snapshot for that module: an omitted label set disappears from the next
snapshot. Counter and Histogram observations from this hook are dropped because
repeated collection would double-count them.

`sdk::Ctx` gains a default-no-op write method:

```rust
fn observe_metric(&mut self, observation: MetricObservation) {}
```

The method returns no status and there is no read API. Module execution cannot
branch on whether collection is enabled, whether a series was dropped, or what
its current node-local value is.

## Wasm ABI

Metrics are introduced as `ducktape:module@0.2.0`. The host continues to
support `0.1.0` components as modules with an empty metric catalog while the
committed fixtures and production tenants migrate.

The conceptual WIT addition is:

```wit
interface metrics {
  type metric-id = u32;

  variant metric-kind {
    counter,
    gauge,
    histogram(list<float64>),
  }

  record metric-descriptor {
    name: string,
    help: string,
    kind: metric-kind,
    labels: list<string>,
  }

  variant metric-value {
    counter-add(u64),
    gauge-set(float64),
    histogram-observe(float64),
  }

  record metric-observation {
    metric: metric-id,
    label-values: list<string>,
    value: metric-value,
  }

  observe: func(observation: metric-observation);
}

world module {
  use host.{error};
  use metrics.{metric-descriptor};
  import host;
  import metrics;

  export metric-descriptors: func() -> list<metric-descriptor>;
  export collect-metrics: func() -> result<_, error>;
  export execute: func(payload: list<u8>) -> result<_, error>;
  export query: func(req: list<u8>) -> result<list<u8>, error>;
}
```

`metric-descriptors` is called once when a component is loaded or swapped, not
on every dispatch. `WasmModule` validates and caches the result. A trap or an
invalid descriptor disables that descriptor/catalog only; it must not prevent
the module from executing consensus work.

`collect-metrics` runs in a read-only, fuel-metered instance after recovery,
state sync, component activation, and successful commits that touched the
module. It uses the same bounded state-read replay machinery as `query`. State
writes, messages, and events attempted during collection are discarded. The
collection context is sealed against sibling `module-root`/`query-module`
reads, matching the native hook's access to `&self` only.
Collection failure retains the previous Gauge snapshot and is visible through
host-owned collection-error metrics.

## Descriptor rules

The host validates catalogs before accepting observations:

- at most 128 descriptors per module;
- local names match `[a-z][a-z0-9_]{0,63}`;
- Counter names omit the OpenMetrics `_total` suffix—the exporter adds it;
- help text is non-empty and at most 256 UTF-8 bytes;
- units use the standard local-name suffix (`duration_seconds`,
  `payload_bytes`); there is no second unit field to drift from the name;
- at most eight unique label keys per descriptor;
- label keys match `[a-z][a-z0-9_]{0,31}`;
- Histogram buckets contain at most 32 finite, strictly increasing values;
  the exporter supplies the final `+Inf` bucket;
- metric ids must exist, value variants must match descriptor kinds, label
  counts must match, and floating-point observations must be finite;
- each label value is at most 128 UTF-8 bytes.

Invalid input is dropped without trapping or returning an error to the guest.
The host emits a structured warning once per catalog fault and increments a
bounded host-owned drop counter. Validation protects availability and metric
integrity; it cannot determine whether a label leaks private business data.
Module authors and package review remain responsible for label semantics.

## Naming

The exporter owns the global namespace:

```text
ducktape_module_<escaped-module-id>_<local-name>[_total]
```

Module ids are escaped injectively into the OpenMetrics identifier alphabet:
ASCII alphanumerics remain unchanged, `_` becomes `__`, and every other UTF-8
byte becomes `_hh` using lowercase hex. Thus `dispatch-oracle` becomes
`dispatch_2doracle`, while `dispatch_2doracle` cannot collide with it because
its underscore is doubled.

Examples:

```text
ducktape_module_tasks_open 12
ducktape_module_jobs_executions_total{outcome="failed"} 4
ducktape_module_saga_duration_seconds_bucket{le="0.5"} 31
```

Metric names are semantic contracts. A component upgrade may add or remove a
name. Reusing a name with a different type, label schema, or Histogram bucket
set resets that process-local series and emits an operator warning.

## Transaction and replay semantics

Metric observations follow the successful state transition, never the first
attempt to execute it.

### Wasm replay

Each Wasm replay round starts with an empty `out_metrics`, alongside the
existing empty `out_msgs` and `out_events`. A pending state/sibling read drops
the round's observations. Only a clean final `execute` transfers observations
to `sdk::Ctx`.

This forbids directly updating a registry inside the WIT import: an observation
before a pending read would otherwise be counted once per replay.

### Dispatch and block commit

`HostCtx` stamps every observation with `env.me`; a module cannot attribute a
sample to a sibling. The host adds observations to the same per-member trace
that already carries events and dispatch records.

The batch rules mirror the existing event trace:

1. a rejected member's observations are discarded;
2. when a later rejection forces accepted-member replay, the replayed trace
   replaces the earlier trace rather than appending to it;
3. a failed system injection or block abort discards every observation;
4. only after every touched module's `commit_block` succeeds does the host
   return the authoritative observation batch;
5. the node metric handler applies that batch once.

`BlockOutcome` and `BatchOutcome` therefore gain an aggregate
`metric_observations` field. `DispatchRecord` gains `emitted_metrics`, matching
its existing emitted message/event counts. Observations are not serialized into
the journal, block wire, state-sync frames, or app hash.

If the process dies after state commit but before applying the metric batch,
the metrics may miss that block. That is acceptable: telemetry must never add
a second durability protocol to consensus commit.

### Recovery and state sync

Recovery replay and state-sync catch-up discard Counter/Histogram observations.
Replaying only the journal tail or only frames after a snapshot would otherwise
produce arbitrary partial lifetime totals. Once the node reaches its serving
boundary, it runs `collect_metrics` to restore cheap state-derived Gauges and
starts new process-local Counters and Histograms from zero.

## Host and node data flow

The host maintains a validated catalog generation for each registered module.
An internal observation leaving the host carries `(module_id, generation,
metric_id, label_values, value)`. Generation is node-local and only prevents a
hot-swap observation from being resolved against the wrong descriptor list; it
is not a consensus or exported label.

The host exposes the active catalogs to the node at boot and reports catalog
changes on module registration, removal, and code realization. At a height-gated
swap the node applies the old component's final committed observations first,
then realizes the component swap, then replaces the exporter catalog. An
observation whose generation is unknown or no longer active is dropped rather
than guessed against another schema.

The node layer owns a `ModuleMetricStore`:

- Counter observations add to the current series;
- Gauge observations replace the current value for one label set;
- Histogram observations update the descriptor's fixed buckets, sum, and
  count;
- a Gauge collection replaces that module's full Gauge snapshot atomically;
- catalog replacement retains only descriptors whose complete schema is
  unchanged and drops removed/incompatible series.

The Commonware adapter uses one custom `EncodeMetric` registration per active
descriptor, reading the shared store at `context.encode()` time. It retains the
`Registered` handles while descriptors are active and drops them on removal,
so stale module series disappear without hand-editing OpenMetrics text or
creating a second `/metrics` response path.

Scraping never invokes Wasm, scans world state, or takes the consensus host
lock. It only encodes the latest node-local snapshot.

## Operator policy

The WIT import is always present on a `0.2.0` host. Disabling metrics must not
make a component fail to instantiate or give the guest a distinguishable
return value.

The node handler has two initial modes:

```toml
[module_metrics]
handler = "prometheus" # or "off"
modules = ["*"]        # optional allowlist
max_series_per_module = 1024
```

- Node binaries already serving `/metrics` default to `prometheus`.
- SDK/test embedders with no handler default to no-op.
- `off` discards committed observations and skips Gauge collection while core
  node metrics remain available.
- An allowlist excludes untrusted or unwanted module catalogs without changing
  module behavior.
- The series limit may be lowered by an operator but may not exceed the hard
  host ceiling of 16,384 per module.

The execution host still accepts calls into a bounded per-round buffer so the
guest-visible behavior is identical. Handler selection happens only after the
block result is known. No operator option is read by module code or included in
world state, package activation, governance, recovery, or state sync.

OTLP or another sink may later consume the same committed observation batch.
No generic sink plug-in interface is added until a second real handler exists.

## Resource bounds and self-observation

In addition to descriptor and label limits, the host accepts at most 4,096
observations per dispatch and 16,384 per block. Calls beyond the bound become
no-ops. These are host hard ceilings, not operator-tunable consensus inputs.

The exporter caps active series per module according to operator policy. A new
label combination beyond the cap is dropped; existing combinations continue
to update. Drops never evict an arbitrary existing series.

The node exposes these bounded core series, using only registered module ids
and fixed reason values as labels:

```text
ducktape_module_metrics_dropped_total{module,reason}
ducktape_module_metrics_collection_errors_total{module}
ducktape_module_metrics_last_collection_timestamp_seconds{module}
ducktape_module_metrics_series{module}
```

Drop reasons are a fixed enum such as `invalid_descriptor`, `invalid_sample`,
`observation_budget`, and `series_budget`. Error strings never become labels.

## Quack packages and module upgrades

Quack remains the package boundary and the Wasm component remains the runtime
artifact inside it. The metrics WIT and descriptor export travel with that
component; Quack needs no separate tracker artifact and no metric state file.

`modreg.active_code_hash` keeps its current meaning: SHA-256 of the runtime
component bytes. Instrumentation or descriptor changes alter those bytes and
therefore use the existing component distribution and height-gated swap flow.
No package hash, observer hash, or metric snapshot is added to world state.

A node must support the component's module ABI before activating it. Quack
preflight declares the required ABI/protocol version, and operators roll out a
host that understands `0.2.0` before activating a component that imports it.
Nodes may choose different metric handlers and still compute identical module
roots and block outcomes.

## Failure behavior

- Unsupported module ABI: fail module preflight/activation as today; do not
  pretend the component can execute.
- Descriptor export trap: disable that module's metrics, log once, continue
  running the module.
- Invalid descriptor/sample: drop it, increment a bounded reason counter, never
  trap the module.
- Gauge collection trap/read failure: retain the last good snapshot and record
  a collection error.
- Exporter encoding failure: omit the affected metric family and report a node
  error; never fail consensus or the HTTP server.
- Handler disabled: silently discard committed observations.
- Hot-swap schema conflict: replace/reset the affected node-local series after
  the old component's final committed observations have been handled.

## Verification

The implementation is complete only with these checks:

### SDK and validation

- descriptor name/help/label/bucket boundary tests;
- kind, metric-id, label-count, finite-value, and value-length rejection;
- module-id escaping is injective for `_`, `-`, non-ASCII bytes, and ordinary
  alphanumerics.

### Wasm host

- one observation emitted before a pending sibling read appears exactly once
  after the final replay;
- the same proof for a store-backed state-read replay;
- a guest rejection/trap publishes no observations;
- an invalid observation is a no-op visible only in host drop accounting;
- `collect-metrics` cannot mutate state or emit messages/events;
- `0.1.0` components still execute with an empty catalog;
- hot swap replaces the catalog without changing the module root.

### Host transaction boundary

- a later batch-member rejection does not duplicate observations from replayed
  accepted members;
- rejected members and aborted blocks contribute nothing;
- system-injection observations follow the same commit rule;
- recovery selective commit does not leak observations from aborted modules;
- app hashes are byte-identical with Prometheus handling on and off.

### Node exporter

- Counter, Gauge, and Histogram exposition has the expected namespace, type,
  labels, buckets, and `_total` convention;
- catalog removal removes the family from the next scrape;
- cardinality and observation ceilings drop only new work and expose bounded
  reason counters;
- `handler = "off"` preserves core `ducktape_*` metrics while omitting every
  module-defined family;
- recovery/state-sync does not reconstruct partial Counters, while Gauge
  collection restores the current snapshot;
- one native/Wasm reference module produces equivalent external series.

## Implementation slices

1. Add SDK metric types, validation, default-no-op native seams, and the
   `0.2.0` WIT alongside `0.1.0` compatibility.
2. Add Wasm descriptor loading, replay-local observation buffers, and read-only
   Gauge collection; prove replay and abort behavior before adding an exporter.
3. Carry committed observations through host outcomes and add the dynamic
   Commonware `EncodeMetric` store in `noded`/`node`.
4. Add operator policy, host self-metrics, one native/Wasm parity tenant, and
   the recovery/state-sync gates.

Each slice must leave metric handling unable to affect module results, roots,
or node liveness.
