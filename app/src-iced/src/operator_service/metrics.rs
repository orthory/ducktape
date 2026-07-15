use super::*;

static METRICS_PREVIOUS: OnceLock<Mutex<BTreeMap<String, TimedMetrics>>> = OnceLock::new();

#[derive(Debug, Clone, Default)]
struct TimedMetrics {
    time_ms: u64,
    blocks_total: u64,
    planes: BTreeMap<(String, String), (u64, u64)>,
    sync_bytes: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, Default)]
struct ParsedMetrics {
    present: bool,
    block_height: u64,
    blocks_total: u64,
    connected_peers: usize,
    accepted: u64,
    rejected: u64,
    buckets: Vec<(f64, u64)>,
    latency_count: u64,
    planes: BTreeMap<(String, String), ParsedPlane>,
    sync_peers: BTreeMap<String, ParsedSyncPeer>,
}

#[derive(Debug, Clone, Default)]
struct ParsedPlane {
    service: String,
    owner: String,
    age_seconds: f64,
    halted: bool,
    tx_bytes: u64,
    rx_bytes: u64,
    drops: u64,
}

#[derive(Debug, Clone, Default)]
struct ParsedSyncPeer {
    peer: String,
    age_seconds: f64,
    bytes_tx: u64,
    frames: u64,
    boundary_height: Option<u64>,
    served_height: Option<u64>,
    requests: BTreeMap<String, u64>,
    last_kind: Option<String>,
}

pub(super) async fn load(
    node: Option<&NodeClient>,
    workspace: Option<&Workspace>,
) -> Result<Option<operator::MetricsSnapshot>, String> {
    let owned_client = local_client(node, workspace)?;
    let Some(client) = node.or(owned_client.as_ref()) else {
        return Ok(None);
    };
    let status = client.status().await.map_err(|error| error.to_string())?;
    if let Some(workspace) = workspace {
        validate_node_identity(&status, workspace)?;
    }
    let text = client
        .metrics_text()
        .await
        .map_err(|error| error.to_string())?;
    let time_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock is before the Unix epoch".to_string())?
        .as_millis()
        .min(u64::MAX as u128) as u64;
    let parsed = parse_metrics(&text)?;
    if !parsed.present {
        return Err("the connected node does not expose Ducktape metrics".into());
    }
    let key = client.cache_key();
    let mut cache = METRICS_PREVIOUS
        .get_or_init(|| Mutex::new(BTreeMap::new()))
        .lock()
        .map_err(|_| "metrics sample cache is unavailable".to_string())?;
    let previous = cache.get(&key);
    let snapshot = metrics_snapshot(&parsed, time_ms, previous);
    cache.insert(key, timed_metrics(&parsed, time_ms));
    Ok(Some(snapshot))
}

fn parse_metrics(text: &str) -> Result<ParsedMetrics, String> {
    let mut metrics = ParsedMetrics::default();
    for line in text.lines() {
        let Some((name, labels, value)) = parse_metric_line(line) else {
            continue;
        };
        match name {
            "ducktape_block_height" => {
                metrics.present = true;
                metrics.block_height = metric_u64(value);
            }
            "ducktape_blocks_total" => {
                metrics.present = true;
                metrics.blocks_total = metric_u64(value);
            }
            "ducktape_consensus_reachable_validators" => {
                metrics.present = true;
                metrics.connected_peers = metric_u64(value).min(usize::MAX as u64) as usize;
            }
            "ducktape_ops_outcome_total" => match labels.get("outcome").map(String::as_str) {
                Some("applied") => metrics.accepted = metric_u64(value),
                Some("rejected") => metrics.rejected = metric_u64(value),
                _ => {}
            },
            "ducktape_block_apply_latency_seconds_count" => {
                metrics.present = true;
                metrics.latency_count = metric_u64(value);
            }
            "ducktape_block_apply_latency_seconds_bucket" => {
                metrics.present = true;
                let le = match labels.get("le").map(String::as_str) {
                    Some("+Inf") => f64::INFINITY,
                    Some(le) => le.parse().unwrap_or(f64::NAN),
                    None => f64::NAN,
                };
                if le.is_finite() || le == f64::INFINITY {
                    metrics.buckets.push((le, metric_u64(value)));
                }
            }
            name if name.starts_with("ducktape_dataplane_") => {
                let service = labels.get("service").cloned().unwrap_or_else(|| "?".into());
                let owner = labels.get("owner").cloned().unwrap_or_else(|| "?".into());
                let plane = metrics
                    .planes
                    .entry((service.clone(), owner.clone()))
                    .or_insert_with(|| ParsedPlane {
                        service,
                        owner,
                        ..ParsedPlane::default()
                    });
                match name {
                    "ducktape_dataplane_halted" => plane.halted = value > 0.0,
                    "ducktape_dataplane_age_seconds" => plane.age_seconds = value.max(0.0),
                    "ducktape_dataplane_bytes" => match labels.get("dir").map(String::as_str) {
                        Some("tx") => {
                            plane.tx_bytes = plane.tx_bytes.saturating_add(metric_u64(value))
                        }
                        Some("rx") => {
                            plane.rx_bytes = plane.rx_bytes.saturating_add(metric_u64(value))
                        }
                        _ => {}
                    },
                    "ducktape_dataplane_drops" => {
                        plane.drops = plane.drops.saturating_add(metric_u64(value));
                    }
                    _ => {}
                }
            }
            name if name.starts_with("ducktape_statesync_serve_") => {
                let peer = labels.get("peer").cloned().unwrap_or_else(|| "?".into());
                let sync =
                    metrics
                        .sync_peers
                        .entry(peer.clone())
                        .or_insert_with(|| ParsedSyncPeer {
                            peer,
                            ..ParsedSyncPeer::default()
                        });
                match name {
                    "ducktape_statesync_serve_age_seconds" => sync.age_seconds = value.max(0.0),
                    "ducktape_statesync_serve_bytes" => sync.bytes_tx = metric_u64(value),
                    "ducktape_statesync_serve_frames" => sync.frames = metric_u64(value),
                    "ducktape_statesync_serve_boundary_height" => {
                        sync.boundary_height = Some(metric_u64(value))
                    }
                    "ducktape_statesync_serve_frame_height" => {
                        sync.served_height = Some(metric_u64(value))
                    }
                    "ducktape_statesync_serve_requests" => {
                        sync.requests.insert(
                            labels.get("kind").cloned().unwrap_or_else(|| "?".into()),
                            metric_u64(value),
                        );
                    }
                    "ducktape_statesync_serve_last_request" => {
                        sync.last_kind = labels.get("kind").cloned()
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    metrics
        .buckets
        .sort_by(|left, right| left.0.total_cmp(&right.0));
    Ok(metrics)
}

fn parse_metric_line(line: &str) -> Option<(&str, BTreeMap<String, String>, f64)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') || line.len() > 16 * 1024 {
        return None;
    }
    let (head, rest) = if let Some(open) = line.find('{') {
        let close = line[open + 1..].find('}')? + open + 1;
        (&line[..open], (&line[open + 1..close], &line[close + 1..]))
    } else {
        let split = line.find(char::is_whitespace)?;
        (&line[..split], ("", &line[split..]))
    };
    if !head.starts_with("ducktape_") {
        return None;
    }
    let labels = rest
        .0
        .split(',')
        .filter_map(|pair| {
            let (key, value) = pair.trim().split_once('=')?;
            let value = value.strip_prefix('"')?.strip_suffix('"')?;
            (key.len() <= 64 && value.len() <= 256).then(|| (key.into(), value.into()))
        })
        .collect();
    let token = rest.1.split_whitespace().next()?;
    let value = token.parse::<f64>().ok()?;
    value.is_finite().then_some((head, labels, value))
}

fn metric_u64(value: f64) -> u64 {
    if value <= 0.0 {
        0
    } else if value >= u64::MAX as f64 {
        u64::MAX
    } else {
        value as u64
    }
}

fn histogram_quantile(metrics: &ParsedMetrics, q: f64) -> f64 {
    if metrics.latency_count == 0 || metrics.buckets.is_empty() {
        return 0.0;
    }
    let rank = q.clamp(0.0, 1.0) * metrics.latency_count as f64;
    let (mut previous_le, mut previous_count) = (0.0, 0_u64);
    for &(le, cumulative) in &metrics.buckets {
        if cumulative as f64 >= rank {
            if le == f64::INFINITY {
                return previous_le;
            }
            let within = cumulative.saturating_sub(previous_count);
            return if within == 0 {
                previous_le
            } else {
                previous_le + (le - previous_le) * (rank - previous_count as f64) / within as f64
            };
        }
        if le.is_finite() {
            previous_le = le;
        }
        previous_count = cumulative;
    }
    previous_le
}

fn rate(previous: u64, current: u64, elapsed_ms: u64) -> f64 {
    if elapsed_ms == 0 || current < previous {
        0.0
    } else {
        (current - previous) as f64 * 1000.0 / elapsed_ms as f64
    }
}

fn format_age(seconds: f64) -> String {
    let seconds = metric_u64(seconds);
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3_600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else if seconds < 86_400 {
        format!("{}h {}m", seconds / 3_600, seconds % 3_600 / 60)
    } else {
        format!("{}d {}h", seconds / 86_400, seconds % 86_400 / 3_600)
    }
}

fn timed_metrics(metrics: &ParsedMetrics, time_ms: u64) -> TimedMetrics {
    TimedMetrics {
        time_ms,
        blocks_total: metrics.blocks_total,
        planes: metrics
            .planes
            .iter()
            .map(|(key, plane)| (key.clone(), (plane.tx_bytes, plane.rx_bytes)))
            .collect(),
        sync_bytes: metrics
            .sync_peers
            .iter()
            .map(|(peer, sync)| (peer.clone(), sync.bytes_tx))
            .collect(),
    }
}

fn metrics_snapshot(
    metrics: &ParsedMetrics,
    time_ms: u64,
    previous: Option<&TimedMetrics>,
) -> operator::MetricsSnapshot {
    let elapsed = previous.map_or(0, |previous| time_ms.saturating_sub(previous.time_ms));
    let blocks_per_second = previous.map_or(0.0, |previous| {
        rate(previous.blocks_total, metrics.blocks_total, elapsed)
    });
    let data_planes = metrics
        .planes
        .iter()
        .map(|(key, plane)| {
            let prior = previous
                .and_then(|sample| sample.planes.get(key))
                .copied()
                .unwrap_or((plane.tx_bytes, plane.rx_bytes));
            operator::DataPlaneMetric {
                service: plane.service.clone(),
                owner: plane.owner.clone(),
                age: format_age(plane.age_seconds),
                tx_bytes_per_second: rate(prior.0, plane.tx_bytes, elapsed),
                rx_bytes_per_second: rate(prior.1, plane.rx_bytes, elapsed),
                total_bytes: plane.tx_bytes.saturating_add(plane.rx_bytes),
                dropped: plane.drops,
                halted: plane.halted,
            }
        })
        .collect();
    let sync_peers = metrics
        .sync_peers
        .iter()
        .map(|(peer, sync)| {
            let prior = previous
                .and_then(|sample| sample.sync_bytes.get(peer))
                .copied()
                .unwrap_or(sync.bytes_tx);
            let reach = sync.served_height.or(sync.boundary_height);
            let parked = sync.last_kind.as_deref() == Some("tip_coords") && reach.is_some();
            let blocks_left = (!parked)
                .then(|| reach.map(|height| metrics.block_height.saturating_sub(height)))
                .flatten();
            let progress = (!parked && metrics.block_height > 0)
                .then(|| {
                    reach
                        .map(|height| (height as f64 / metrics.block_height as f64).min(1.0) as f32)
                })
                .flatten();
            operator::SyncPeerMetric {
                peer: sync.peer.clone(),
                phase: sync_phase(sync),
                age: format_age(sync.age_seconds),
                progress,
                blocks_left,
                tx_bytes_per_second: rate(prior, sync.bytes_tx, elapsed),
                total_bytes: sync.bytes_tx,
                frames: sync.frames,
            }
        })
        .collect();
    operator::MetricsSnapshot {
        block_height: metrics.block_height,
        connected_peers: metrics.connected_peers,
        blocks_per_second,
        apply_p50_ms: histogram_quantile(metrics, 0.5) * 1000.0,
        apply_p95_ms: histogram_quantile(metrics, 0.95) * 1000.0,
        accepted: metrics.accepted,
        rejected: metrics.rejected,
        data_planes,
        sync_peers,
        sampled_at: format!("{time_ms} ms"),
    }
}

fn sync_phase(peer: &ParsedSyncPeer) -> String {
    match peer.last_kind.as_deref() {
        Some("manifest") => "manifest served",
        Some("chunk" | "module" | "index_chunk" | "index_modules") => "restoring snapshot",
        Some("frames") => "replaying frames",
        Some("tip_coords") if peer.served_height.or(peer.boundary_height).is_some() => "parked",
        Some("tip_coords") => "polling tip",
        Some("blob") => "fetching blobs",
        Some(kind) => kind,
        None if peer.served_height.is_some() => "replaying frames",
        None if ["chunk", "module", "index_chunk", "index_modules"]
            .iter()
            .any(|kind| peer.requests.get(*kind).copied().unwrap_or(0) > 0) =>
        {
            "restoring snapshot"
        }
        None if peer.boundary_height.is_some() => "manifest served",
        None if peer.requests.get("tip_coords").copied().unwrap_or(0) > 0 => "polling tip",
        None => "fetching blobs",
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_parser_derives_histograms_rates_and_planes() {
        let first = parse_metrics(
            r#"
ducktape_block_height 10
ducktape_blocks_total 20
ducktape_consensus_reachable_validators 3
ducktape_ops_outcome_total{outcome="applied"} 8
ducktape_ops_outcome_total{outcome="rejected"} 2
ducktape_block_apply_latency_seconds_count 10
ducktape_block_apply_latency_seconds_bucket{le="0.1"} 5
ducktape_block_apply_latency_seconds_bucket{le="0.5"} 9
ducktape_block_apply_latency_seconds_bucket{le="+Inf"} 10
ducktape_dataplane_open{service="voice",owner="chat"} 1
ducktape_dataplane_age_seconds{service="voice",owner="chat"} 65
ducktape_dataplane_bytes{service="voice",owner="chat",dir="tx",class="stream"} 1000
ducktape_dataplane_bytes{service="voice",owner="chat",dir="rx",class="stream"} 500
ducktape_statesync_serve_age_seconds{peer="abc"} 8
ducktape_statesync_serve_bytes{peer="abc"} 200
ducktape_statesync_serve_frame_height{peer="abc"} 8
ducktape_statesync_serve_last_request{peer="abc",kind="frames"} 1
"#,
        )
        .unwrap();
        let previous = timed_metrics(&first, 1_000);
        let mut second = first.clone();
        second.blocks_total = 24;
        second
            .planes
            .get_mut(&("voice".into(), "chat".into()))
            .unwrap()
            .tx_bytes = 1_500;
        second.sync_peers.get_mut("abc").unwrap().bytes_tx = 500;
        let snapshot = metrics_snapshot(&second, 3_000, Some(&previous));
        assert_eq!(snapshot.block_height, 10);
        assert_eq!(snapshot.connected_peers, 3);
        assert_eq!(snapshot.blocks_per_second, 2.0);
        assert!((snapshot.apply_p50_ms - 100.0).abs() < 0.001);
        assert_eq!(snapshot.data_planes[0].age, "1m 5s");
        assert_eq!(snapshot.data_planes[0].tx_bytes_per_second, 250.0);
        assert_eq!(snapshot.sync_peers[0].phase, "replaying frames");
        assert_eq!(snapshot.sync_peers[0].tx_bytes_per_second, 150.0);
    }
}
