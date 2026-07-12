//! `ducktape_statesync_serve_*`: the per-peer Prometheus series behind GET
//! /metrics for the statesync SERVE lane — who is pulling state from this
//! node, how far along they are, and how much has been shipped.
//!
//! statesync rides the multiplexed mesh channel (never a data plane — the
//! `Service::StateSync` binding is unwired), so the `ducktape_dataplane_*`
//! family can never show it; this family observes the serve loop instead.
//! Collection is scrape-fresh like `plane_metrics`: each series reads the
//! shared [`ServeMonitor`] at `context.encode()` time, no sampler task. All
//! values are gauges; `bytes`, `frames`, and `requests` are cumulative for a
//! peer's conversation, so readers derive rates from deltas. A peer that
//! stops requesting ages out of the snapshot ([`statesync::monitor::SERVE_EXPIRE`])
//! and its series vanish — presence IS recent utilization.
//!
//! Progression is served-side truth: `boundary_height` is the snapshot base
//! the peer restores from, `frame_height` the highest finalized frame this
//! node has handed it. The goal to measure either against is the node's own
//! `ducktape_block_height` from the same scrape.
//!
//! The heights are conversation history and FREEZE once a joiner finishes
//! and parks (its tip polls keep the entry alive); `last_request{kind}` — an
//! info gauge, constant 1, the kind label is the value — carries the recency
//! discriminant readers phase a peer by instead.

use commonware_runtime::telemetry::metrics::{EncodeMetric, MetricEncoder, MetricType, Registered};
use statesync::monitor::{PeerServeReport, ServeMonitor};

/// Extra label pairs beyond the shared `{peer}` identity, built per report
/// (the `requests` family fans out by kind).
type ExtraLabels = Vec<(&'static str, &'static str)>;

/// One exported gauge family over the recently-served peer set: `project`
/// maps each peer's report to this family's `(extra labels, value)` samples —
/// an empty vec omits the peer (e.g. no manifest served yet).
struct ServeSeries {
    monitor: ServeMonitor,
    project: fn(&PeerServeReport) -> Vec<(ExtraLabels, i64)>,
}

impl std::fmt::Debug for ServeSeries {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServeSeries").finish_non_exhaustive()
    }
}

impl EncodeMetric for ServeSeries {
    fn encode(&self, mut encoder: MetricEncoder) -> Result<(), std::fmt::Error> {
        for report in self.monitor.snapshot() {
            for (extra, value) in (self.project)(&report) {
                let mut labels: Vec<(&str, &str)> = vec![("peer", report.peer.as_str())];
                labels.extend(extra.iter().map(|(k, v)| (*k, *v)));
                encoder.encode_family(&labels)?.encode_gauge(&value)?;
            }
        }
        Ok(())
    }

    fn metric_type(&self) -> MetricType {
        MetricType::Gauge
    }
}

/// Cumulative counters exceed `i64` only past 9.2 EB / 9.2e18 events —
/// unreachable for a serve conversation; the cast is total in practice.
fn gauge(value: u64) -> i64 {
    value as i64
}

/// The registered `ducktape_statesync_serve_*` handles. Dropping a
/// [`Registered`] unregisters its series, so hold this for the life of
/// the process (alongside the node's other metrics).
pub(crate) struct SyncServeMetrics {
    _series: Vec<Registered<ServeSeries>>,
}

impl SyncServeMetrics {
    /// Register the per-peer serve series on the runtime context (root
    /// context, so names carry no child prefix), reading `monitor` at
    /// scrape time.
    pub(crate) fn register<C: commonware_runtime::Metrics>(
        context: &C,
        monitor: &ServeMonitor,
    ) -> Self {
        let series =
            |name: &str, help: &str, project: fn(&PeerServeReport) -> Vec<(ExtraLabels, i64)>| {
                context.register(
                    name,
                    help,
                    ServeSeries {
                        monitor: monitor.clone(),
                        project,
                    },
                )
            };
        Self {
            _series: vec![
                series(
                    "ducktape_statesync_serve_age_seconds",
                    "seconds since this node first served the peer's current sync conversation",
                    |r| vec![(vec![], gauge(r.age.as_secs()))],
                ),
                series(
                    "ducktape_statesync_serve_idle_seconds",
                    "seconds since the peer's last answered statesync request",
                    |r| vec![(vec![], gauge(r.idle.as_secs()))],
                ),
                series(
                    "ducktape_statesync_serve_bytes",
                    "cumulative wire bytes served to the peer over the statesync lane",
                    |r| vec![(vec![], gauge(r.bytes_tx))],
                ),
                series(
                    "ducktape_statesync_serve_frames",
                    "cumulative finalized frames (blocks) served to the peer",
                    |r| vec![(vec![], gauge(r.frames_served))],
                ),
                series(
                    "ducktape_statesync_serve_requests",
                    "cumulative answered statesync requests from the peer, by request kind",
                    |r| {
                        r.requests
                            .iter()
                            .map(|(kind, count)| (vec![("kind", *kind)], gauge(*count)))
                            .collect()
                    },
                ),
                series(
                    "ducktape_statesync_serve_last_request",
                    "the peer's most recent answered request kind (info gauge: constant 1)",
                    |r| vec![(vec![("kind", r.last_kind)], 1)],
                ),
                series(
                    "ducktape_statesync_serve_boundary_height",
                    "the snapshot boundary height last served to the peer (its restore base)",
                    |r| match r.boundary_height {
                        Some(h) => vec![(vec![], gauge(h))],
                        None => vec![],
                    },
                ),
                series(
                    "ducktape_statesync_serve_frame_height",
                    "the highest finalized frame height served to the peer (its replay reach)",
                    |r| match r.served_height {
                        Some(h) => vec![(vec![], gauge(h))],
                        None => vec![],
                    },
                ),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use statesync::{SyncRequest, SyncResponse};

    /// The full exposition path: a monitor with one served peer, registered
    /// on a real runtime registry, must encode `{peer}`-labeled
    /// `ducktape_statesync_serve_*` samples — with the progression heights
    /// absent until the responses that establish them are served.
    #[test]
    fn scrape_encodes_served_peers() {
        use commonware_runtime::{Metrics as _, Runner as _};

        let executor = commonware_runtime::deterministic::Runner::default();
        executor.start(|context| async move {
            let monitor = ServeMonitor::default();
            monitor.record(
                "abcd1234",
                SyncRequest::TipCoords.kind_name(),
                &SyncResponse::Error("not ready".into()),
                256,
            );
            let _metrics = SyncServeMetrics::register(&context, &monitor);

            let scrape = context.encode();
            assert!(
                scrape.contains(r#"ducktape_statesync_serve_bytes{peer="abcd1234"} 256"#),
                "bytes series missing from scrape:\n{scrape}"
            );
            assert!(
                scrape.contains(
                    r#"ducktape_statesync_serve_requests{peer="abcd1234",kind="tip_coords"} 1"#
                ),
                "requests series missing from scrape:\n{scrape}"
            );
            assert!(
                scrape.contains(
                    r#"ducktape_statesync_serve_last_request{peer="abcd1234",kind="tip_coords"} 1"#
                ),
                "last-request info gauge missing from scrape:\n{scrape}"
            );
            // no manifest and no frames served yet: neither height encodes.
            assert!(
                !scrape.contains("ducktape_statesync_serve_boundary_height{"),
                "boundary height encoded without a served manifest:\n{scrape}"
            );
            assert!(
                !scrape.contains("ducktape_statesync_serve_frame_height{"),
                "frame height encoded without served frames:\n{scrape}"
            );

            // frames land: the replay-reach series appears.
            monitor.record(
                "abcd1234",
                "frames",
                &SyncResponse::Frames {
                    frames: vec![statesync::FinalizedFrame {
                        height: 7,
                        frame: vec![],
                        disposition: statesync::FrameDisposition::Applied,
                        roots: vec![],
                        app_hash: sdk::StateRoot([0u8; 32]),
                    }],
                },
                1024,
            );
            let scrape = context.encode();
            assert!(
                scrape.contains(r#"ducktape_statesync_serve_frame_height{peer="abcd1234"} 7"#),
                "frame height missing after served frames:\n{scrape}"
            );
            assert!(
                scrape.contains(r#"ducktape_statesync_serve_frames{peer="abcd1234"} 1"#),
                "frames counter missing:\n{scrape}"
            );
            // the recency discriminant followed the latest request kind.
            assert!(
                scrape.contains(
                    r#"ducktape_statesync_serve_last_request{peer="abcd1234",kind="frames"} 1"#
                ),
                "last-request info gauge did not follow the latest kind:\n{scrape}"
            );
            assert!(
                !scrape.contains(
                    r#"ducktape_statesync_serve_last_request{peer="abcd1234",kind="tip_coords"}"#
                ),
                "stale last-request sample lingered:\n{scrape}"
            );
        });
    }
}
