//! node metrics: the `ducktape_*` Prometheus series and the GET /metrics
//! scrape handler.

use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use commonware_runtime::telemetry::metrics::{EncodeLabelSet, MetricsExt as _, Registered, raw};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    ConsensusOperationalStatus, IndexOperationalStatus, NodeHandle, NodePhase, NodeRole,
    OperationalStatus, SyncOperationalStatus,
};

// ---------------------------------------------------------------------------
// node metrics: the `ducktape_*` Prometheus series behind GET /metrics.
// shared by every binary serving this surface — the embedded daemon folds a
// block in at submit, the consensus validator at drain — so one Grafana board
// reads them all.
// ---------------------------------------------------------------------------

/// histogram buckets for block apply latency, in SECONDS (Prometheus
/// convention). ~100µs to ~1s — the range one local block apply falls in.
const LATENCY_BUCKETS: [f64; 13] = [
    0.0001, 0.00025, 0.0005, 0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0,
];

/// labels for the per-dispatch counter. kept LOW-CARDINALITY: `module` is the
/// bounded registered set; `origin` is the trigger KIND only — never the
/// specific submitter name or emitter id.
#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct DispatchLabels {
    module: String,
    origin: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct PhaseLabels {
    role: String,
    phase: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct OutcomeLabels {
    outcome: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct ModuleLabels {
    module: String,
}

/// the low-cardinality trigger KIND of a dispatch origin — the metrics label.
fn origin_kind(origin: &sdk::Origin) -> &'static str {
    match origin {
        sdk::Origin::External(_) => "external",
        sdk::Origin::Module(_) => "module",
        sdk::Origin::System => "system",
    }
}

/// the node's own Prometheus series, registered INTO commonware's runtime
/// registry so one `context.encode()` (GET /metrics) serves runtime + app
/// metrics together. each `Registered` handle is retained for the process life;
/// updates go through its `Deref` to the underlying metric.
#[derive(Clone)]
pub struct NodeMetrics {
    block_height: Registered<raw::Gauge>,
    blocks_total: Registered<raw::Counter>,
    ops_total: Registered<raw::Counter>,
    apply_latency: Registered<raw::Histogram>,
    dispatch_total: Registered<raw::Family<DispatchLabels, raw::Counter>>,
    node_phase: Registered<raw::Family<PhaseLabels, raw::Gauge>>,
    op_outcomes: Registered<raw::Family<OutcomeLabels, raw::Counter>>,
    consensus_epoch: Registered<raw::Gauge>,
    consensus_view: Registered<raw::Gauge>,
    consensus_validators: Registered<raw::Gauge>,
    consensus_quorum: Registered<raw::Gauge>,
    consensus_reachable: Registered<raw::Gauge>,
    consensus_pending: Registered<raw::Gauge>,
    last_finalized_at: Registered<raw::Gauge>,
    sync_target_height: Registered<raw::Gauge>,
    sync_applied_height: Registered<raw::Gauge>,
    sync_retries: Registered<raw::Counter>,
    sync_failures: Registered<raw::Counter>,
    sync_last_progress_at: Registered<raw::Gauge>,
    checkpoint_height: Registered<raw::Gauge>,
    index_poisoned: Registered<raw::Gauge>,
    index_height: Registered<raw::Family<ModuleLabels, raw::Gauge>>,
    operations: Arc<RwLock<OperationalStatus>>,
}

impl NodeMetrics {
    /// register the `ducktape_*` series on the runtime context (root context, so
    /// names carry no child prefix).
    pub fn register<C: commonware_runtime::Metrics>(context: &C) -> Self {
        let metrics = Self {
            block_height: context.gauge(
                "ducktape_block_height",
                "latest committed local block height",
            ),
            // NB: the registry appends the OpenMetrics `_total` suffix to a
            // counter, so the exposed names are `ducktape_blocks_total` and
            // `ducktape_dispatch_total{…}` — DON'T put `_total` in the name here
            // or it doubles.
            blocks_total: context.counter(
                "ducktape_blocks",
                "committed local blocks since daemon start",
            ),
            // one BLOCK now aggregates N member ops; `ducktape_blocks_total`
            // counts blocks, `ducktape_ops_total` counts the aggregated ops, so
            // ops/blocks is the average batch size.
            ops_total: context.counter(
                "ducktape_ops",
                "member ops aggregated into committed local blocks since daemon start",
            ),
            apply_latency: context.histogram(
                "ducktape_block_apply_latency_seconds",
                "node-local wall-clock cost of applying one block",
                LATENCY_BUCKETS,
            ),
            dispatch_total: context.family(
                "ducktape_dispatch",
                "module dispatches, by module and trigger-origin kind",
            ),
            node_phase: context.family(
                "ducktape_node_phase",
                "whether this node is currently in a bounded role and lifecycle phase",
            ),
            op_outcomes: context.family(
                "ducktape_ops_outcome",
                "finalized member operations by applied or rejected outcome",
            ),
            consensus_epoch: context.gauge("ducktape_consensus_epoch", "current consensus epoch"),
            consensus_view: context.gauge(
                "ducktape_consensus_view",
                "latest locally finalized view in the current epoch",
            ),
            consensus_validators: context.gauge(
                "ducktape_consensus_validators",
                "validators in the current epoch",
            ),
            consensus_quorum: context.gauge(
                "ducktape_consensus_quorum",
                "validators required to finalize in the current epoch",
            ),
            consensus_reachable: context.gauge(
                "ducktape_consensus_reachable_validators",
                "current validators reachable by this node, including itself when a member",
            ),
            consensus_pending: context.gauge(
                "ducktape_consensus_pending_ops",
                "operations staged locally or waiting in the consensus orderer",
            ),
            last_finalized_at: context.gauge(
                "ducktape_last_finalized_timestamp_seconds",
                "unix timestamp of this node's latest finalized block",
            ),
            sync_target_height: context.gauge(
                "ducktape_statesync_target_height",
                "target boundary height of the local state-sync attempt",
            ),
            sync_applied_height: context.gauge(
                "ducktape_statesync_applied_height",
                "latest height installed by the local state-sync attempt",
            ),
            sync_retries: context.counter(
                "ducktape_statesync_retries",
                "local state-sync retries since process start",
            ),
            sync_failures: context.counter(
                "ducktape_statesync_failures",
                "failed local state-sync attempts since process start",
            ),
            sync_last_progress_at: context.gauge(
                "ducktape_statesync_last_progress_timestamp_seconds",
                "unix timestamp of the latest local state-sync progress",
            ),
            checkpoint_height: context.gauge(
                "ducktape_checkpoint_height",
                "height of the latest durable recovery checkpoint",
            ),
            index_poisoned: context.gauge(
                "ducktape_index_poisoned",
                "whether the derived index has stopped accepting writes after a failure",
            ),
            index_height: context.family(
                "ducktape_index_height",
                "latest fully indexed height by bounded module id",
            ),
            operations: Arc::new(RwLock::new(OperationalStatus {
                phase_since: unix_seconds(),
                ..OperationalStatus::default()
            })),
        };
        metrics.set_role_phase(NodeRole::Unknown, NodePhase::Starting);
        metrics
    }

    /// fold one applied block into the series: height, count, this node's
    /// wall-clock apply latency, and the per-module dispatch counters.
    pub fn record_block(&self, height: u64, latency_us: u64, dispatches: &[host::DispatchRecord]) {
        self.block_height.set(height as i64);
        self.record_finalized_now();
        self.blocks_total.inc();
        // microseconds → seconds for the Prometheus convention.
        self.apply_latency.observe(latency_us as f64 / 1_000_000.0);
        for d in dispatches {
            self.dispatch_total
                .get_or_create(&DispatchLabels {
                    module: d.module.clone(),
                    origin: origin_kind(&d.origin).to_string(),
                })
                .inc();
        }
    }

    /// count the member ops an applied block aggregated (`ducktape_ops_total`).
    /// called once per applied block alongside [`record_block`](Self::record_block).
    pub fn record_ops(&self, ops: usize) {
        self.ops_total.inc_by(ops as u64);
    }

    /// Record deterministic finalized outcomes while retaining the older
    /// aggregate `ducktape_ops_total` compatibility series.
    pub fn record_op_outcomes(&self, applied: usize, rejected: usize) {
        self.record_ops(applied + rejected);
        for (outcome, count) in [("applied", applied), ("rejected", rejected)] {
            self.op_outcomes
                .get_or_create(&OutcomeLabels {
                    outcome: outcome.to_string(),
                })
                .inc_by(count as u64);
        }
    }

    /// follow the committed height WITHOUT recording a block apply — the
    /// validator lane calls this for rejected frames (a deterministic no-op
    /// advances the height but is not a sample worth the block series; the
    /// idle heartbeat nop lands here, so it never pollutes the histogram).
    pub fn record_height(&self, height: u64) {
        self.block_height.set(height as i64);
        self.record_finalized_now();
    }

    /// Change the bounded lifecycle coordinates and update the status snapshot
    /// and phase metric together. Old coordinates remain present at zero so a
    /// dashboard does not retain a stale `1` after a transition.
    pub fn set_role_phase(&self, role: NodeRole, phase: NodePhase) {
        let mut status = self.operations.write().expect("operations lock poisoned");
        let old = PhaseLabels {
            role: status.role.as_str().to_string(),
            phase: status.phase.as_str().to_string(),
        };
        self.node_phase.get_or_create(&old).set(0);
        if status.role != role || status.phase != phase {
            status.phase_since = unix_seconds();
        }
        status.role = role;
        status.phase = phase;
        self.node_phase
            .get_or_create(&PhaseLabels {
                role: role.as_str().to_string(),
                phase: phase.as_str().to_string(),
            })
            .set(1);
    }

    pub fn operational_status(&self) -> OperationalStatus {
        self.operations
            .read()
            .expect("operations lock poisoned")
            .clone()
    }

    /// the shared operations projection itself, for the status cell's live
    /// overlay ([`crate::StatusCell::wire_metrics`]).
    pub(crate) fn operations_handle(&self) -> Arc<RwLock<OperationalStatus>> {
        Arc::clone(&self.operations)
    }

    pub fn update_consensus(
        &self,
        epoch: u64,
        view: u64,
        validators: u64,
        reachable_validators: u64,
        pending_ops: u64,
    ) {
        let quorum = quorum(validators);
        self.consensus_epoch.set(epoch as i64);
        self.consensus_view.set(view as i64);
        self.consensus_validators.set(validators as i64);
        self.consensus_quorum.set(quorum as i64);
        self.consensus_reachable.set(reachable_validators as i64);
        self.consensus_pending.set(pending_ops as i64);
        let mut status = self.operations.write().expect("operations lock poisoned");
        status.consensus = Some(ConsensusOperationalStatus {
            epoch,
            view,
            validators,
            quorum,
            reachable_validators,
            pending_ops,
        });
    }

    pub fn begin_sync(&self, source: Option<String>, target_height: u64) {
        self.sync_target_height.set(target_height as i64);
        let mut status = self.operations.write().expect("operations lock poisoned");
        let prior = status.sync.take().unwrap_or_default();
        status.sync = Some(SyncOperationalStatus {
            source,
            target_height,
            ..prior
        });
    }

    pub fn record_sync_progress(&self, applied_height: u64) {
        let now = unix_seconds();
        self.sync_applied_height.set(applied_height as i64);
        self.sync_last_progress_at.set(now as i64);
        let mut status = self.operations.write().expect("operations lock poisoned");
        let sync = status.sync.get_or_insert_with(Default::default);
        sync.applied_height = applied_height;
        sync.last_progress_at = Some(now);
        sync.last_error = None;
    }

    pub fn record_sync_retry(&self, error: impl Into<String>) {
        self.sync_retries.inc();
        let mut status = self.operations.write().expect("operations lock poisoned");
        let sync = status.sync.get_or_insert_with(Default::default);
        sync.retries += 1;
        sync.last_error = Some(error.into());
    }

    pub fn record_sync_failure(&self, error: impl Into<String>) {
        self.sync_failures.inc();
        let mut status = self.operations.write().expect("operations lock poisoned");
        let sync = status.sync.get_or_insert_with(Default::default);
        sync.failures += 1;
        sync.last_error = Some(error.into());
    }

    pub fn update_storage<I, S>(&self, checkpoint_height: u64, index_poisoned: bool, indexes: I)
    where
        I: IntoIterator<Item = (S, u64)>,
        S: Into<String>,
    {
        self.checkpoint_height.set(checkpoint_height as i64);
        self.index_poisoned.set(i64::from(index_poisoned));
        let indexes: Vec<_> = indexes
            .into_iter()
            .map(|(module, applied_height)| IndexOperationalStatus {
                module: module.into(),
                applied_height,
            })
            .collect();
        for index in &indexes {
            self.index_height
                .get_or_create(&ModuleLabels {
                    module: index.module.clone(),
                })
                .set(index.applied_height as i64);
        }
        let mut status = self.operations.write().expect("operations lock poisoned");
        status.storage.checkpoint_height = checkpoint_height;
        status.storage.index_poisoned = index_poisoned;
        status.storage.indexes = indexes;
    }

    fn record_finalized_now(&self) {
        let now = unix_seconds();
        self.last_finalized_at.set(now as i64);
        let mut status = self.operations.write().expect("operations lock poisoned");
        status.last_finalized_at = Some(now);
    }
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn quorum(validators: u64) -> u64 {
    validators.saturating_mul(2) / 3 + u64::from(validators > 0)
}

/// the OpenMetrics content type a Prometheus scraper negotiates for `/metrics`.
const OPENMETRICS_CONTENT_TYPE: &str = "application/openmetrics-text; version=1.0.0; charset=utf-8";

/// GET /metrics — the Prometheus scrape surface. encodes the runtime
/// registry (which the daemon's `ducktape_*` series are registered into)
/// through the handle's wired exposition source — the registry is shared
/// state, so a scrape never crosses the command lane and stays live while a
/// sync/catch-up stage has the pump busy.
pub(crate) async fn metrics(State(handle): State<NodeHandle>) -> Response {
    match handle.status_cell().exposition() {
        Some(body) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, OPENMETRICS_CONTENT_TYPE)],
            body,
        )
            .into_response(),
        None => crate::error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "no metrics exposition is wired on this daemon",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operational_snapshot_and_scrape_follow_the_same_updates() {
        use commonware_runtime::{Metrics as _, Runner as _};

        commonware_runtime::deterministic::Runner::default().start(|context| async move {
            let metrics = NodeMetrics::register(&context);
            metrics.set_role_phase(NodeRole::Validator, NodePhase::Validating);
            metrics.update_consensus(3, 9, 4, 3, 2);
            metrics.record_op_outcomes(5, 1);
            metrics.begin_sync(Some("peer-a".into()), 42);
            metrics.record_sync_retry("manifest unavailable");
            metrics.record_sync_progress(40);
            metrics.update_storage(36, true, [("chat", 39), ("files", 40)]);

            let status = metrics.operational_status();
            assert_eq!(status.role, NodeRole::Validator);
            assert_eq!(status.phase, NodePhase::Validating);
            assert_eq!(status.consensus.as_ref().unwrap().quorum, 3);
            assert_eq!(status.sync.as_ref().unwrap().applied_height, 40);
            assert_eq!(status.sync.as_ref().unwrap().retries, 1);
            assert!(status.storage.index_poisoned);

            let scrape = context.encode();
            for sample in [
                r#"ducktape_node_phase{role="validator",phase="validating"} 1"#,
                "ducktape_consensus_quorum 3",
                "ducktape_consensus_reachable_validators 3",
                r#"ducktape_ops_outcome_total{outcome="applied"} 5"#,
                r#"ducktape_ops_outcome_total{outcome="rejected"} 1"#,
                "ducktape_statesync_target_height 42",
                "ducktape_statesync_applied_height 40",
                "ducktape_statesync_retries_total 1",
                "ducktape_checkpoint_height 36",
                r#"ducktape_index_height{module="files"} 40"#,
                "ducktape_index_poisoned 1",
            ] {
                assert!(scrape.contains(sample), "missing {sample:?}:\n{scrape}");
            }
            assert!(
                !scrape.contains("peer-a"),
                "sync source leaked into an unbounded metric label:\n{scrape}"
            );
        });
    }
}
