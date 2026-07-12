//! node metrics: the `ducktape_*` Prometheus series and the GET /metrics
//! scrape handler.

use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use commonware_runtime::telemetry::metrics::{EncodeLabelSet, MetricsExt as _, Registered, raw};
use futures::channel::oneshot;

use crate::{NodeCommand, NodeHandle, actor_gone};

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
pub struct NodeMetrics {
    block_height: Registered<raw::Gauge>,
    blocks_total: Registered<raw::Counter>,
    ops_total: Registered<raw::Counter>,
    apply_latency: Registered<raw::Histogram>,
    dispatch_total: Registered<raw::Family<DispatchLabels, raw::Counter>>,
}

impl NodeMetrics {
    /// register the `ducktape_*` series on the runtime context (root context, so
    /// names carry no child prefix).
    pub fn register<C: commonware_runtime::Metrics>(context: &C) -> Self {
        Self {
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
        }
    }

    /// fold one applied block into the series: height, count, this node's
    /// wall-clock apply latency, and the per-module dispatch counters.
    pub fn record_block(&self, height: u64, latency_us: u64, dispatches: &[host::DispatchRecord]) {
        self.block_height.set(height as i64);
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

    /// follow the committed height WITHOUT recording a block apply — the
    /// validator lane calls this for rejected frames (a deterministic no-op
    /// advances the height but is not a sample worth the block series; the
    /// idle heartbeat nop lands here, so it never pollutes the histogram).
    pub fn record_height(&self, height: u64) {
        self.block_height.set(height as i64);
    }
}

/// the OpenMetrics content type a Prometheus scraper negotiates for `/metrics`.
const OPENMETRICS_CONTENT_TYPE: &str = "application/openmetrics-text; version=1.0.0; charset=utf-8";

/// GET /metrics — the Prometheus scrape surface. the actor encodes the
/// commonware runtime registry (which the daemon's `ducktape_*` series are
/// registered into) to OpenMetrics text and hands it back over the command lane.
pub(crate) async fn metrics(State(handle): State<NodeHandle>) -> Response {
    let (reply, rx) = oneshot::channel();
    if let Err(resp) = handle.send(NodeCommand::Metrics { reply }).await {
        return resp;
    }
    match rx.await {
        Ok(body) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, OPENMETRICS_CONTENT_TYPE)],
            body,
        )
            .into_response(),
        Err(_) => actor_gone(),
    }
}
