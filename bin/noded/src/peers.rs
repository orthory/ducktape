//! the direct-peer projection behind `GET /v1/peers` (and the local rpc's
//! `peers` cmd): one view over every peer this node is currently meshed with
//! or recently served, assembled by parsing the runtime's own metrics
//! exposition — the same mechanism the operations projection and
//! `reachable_validators` already read connectivity from, so there is exactly
//! ONE source of per-peer truth and no second bookkeeping path to drift.
//!
//! what each field is read from:
//! - `network_tracker_directory_connected{peer}` — commonware's tracker gauge:
//!   the peer is HELD OPEN by the mesh right now; the value is the unix-ms
//!   timestamp the connection became active.
//! - `network_spawner_messages_{sent,received}_total{peer,message}` — per-peer
//!   mesh message counters, summed across channels. counts, not bytes: the
//!   transport exports no per-peer byte series today, so rates derived from
//!   these are messages/second.
//! - `ducktape_statesync_serve_*{peer}` — the statesync serve lane: cumulative
//!   wire bytes to the peer plus its replay progression (`servedHeight` /
//!   `boundaryHeight` are the closest thing to a peer-REPORTED height this
//!   node observes; a peer that never syncs from us reports none).
//!
//! a peer present only in the serve series was served within the monitor's
//! expiry window but is not currently connection-tracked (`connected: false`).
//! cumulative fields never carry rates — a consumer (the `node peers` CLI)
//! derives rates by diffing two samples' counters over their `sampledAtMs`.

use std::collections::BTreeMap;

/// the `/v1/peers` (and rpc `peers`) reply: one sample of the peer set.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PeersView {
    /// when this sample was taken (unix ms) — the denominator anchor for
    /// rate-deriving consumers.
    pub sampled_at_ms: u64,
    /// every known direct peer, sorted by key hex for a stable order.
    pub peers: Vec<PeerView>,
}

/// one direct peer's sample.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PeerView {
    /// the peer's mesh key, hex.
    pub peer: String,
    /// the mesh tracker holds this connection open right now.
    pub connected: bool,
    /// unix ms the current connection became active (`None` when not
    /// currently connected).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connected_since_ms: Option<u64>,
    /// the peer's standing in the valset — `validator` / `resident` — when
    /// the answering lane knows it; absent otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// cumulative mesh messages sent to the peer, all channels.
    pub msgs_sent: u64,
    /// cumulative mesh messages received from the peer, all channels.
    pub msgs_received: u64,
    /// the statesync serve lane's per-peer accounting, when this node served
    /// the peer recently.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statesync: Option<StatesyncServeView>,
}

/// the statesync serve lane's view of one peer (mirrors the
/// `ducktape_statesync_serve_*` series).
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StatesyncServeView {
    /// cumulative wire bytes served to the peer.
    pub bytes_tx: u64,
    /// cumulative finalized frames (blocks) served.
    pub frames_served: u64,
    /// the snapshot boundary height last served — the peer's restore base.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boundary_height: Option<u64>,
    /// the highest frame height served — the peer's replay reach.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub served_height: Option<u64>,
    /// seconds since the peer's last answered request.
    pub idle_seconds: u64,
    /// seconds since this serve conversation began.
    pub age_seconds: u64,
    /// the peer's most recent request kind (`tip_coords` here with heights
    /// set means a synced, parked resident — not a replay in flight).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_request_kind: Option<String>,
}

impl PeersView {
    /// stamp valset standing onto the sampled peers: a peer whose key hex is
    /// in `validators` is a `validator`, in `residents` a `resident`; anyone
    /// else keeps `None`. lanes that cannot read the valset skip this.
    pub fn with_roles(
        mut self,
        validators: &std::collections::BTreeSet<String>,
        residents: &std::collections::BTreeSet<String>,
    ) -> Self {
        for peer in &mut self.peers {
            let standing = if validators.contains(&peer.peer) {
                Some("validator")
            } else if residents.contains(&peer.peer) {
                Some("resident")
            } else {
                None
            };
            peer.role = standing.map(str::to_string);
        }
        self
    }
}

/// one exposition sample line, split: family name, raw label body, value.
/// label values in the families read here are hex keys / kind tokens — never
/// escaped quotes — so a plain quote scan is exact.
fn parse_sample(line: &str) -> Option<(&str, &str, f64)> {
    if line.starts_with('#') {
        return None;
    }
    let (family, rest) = line.split_once('{')?;
    let (labels, value) = rest.rsplit_once("} ")?;
    let value = value.split_whitespace().next()?.parse::<f64>().ok()?;
    Some((family, labels, value))
}

/// a label's value out of a raw `k="v",k2="v2"` body.
fn label<'a>(labels: &'a str, name: &str) -> Option<&'a str> {
    let start = labels.find(&format!("{name}=\""))? + name.len() + 2;
    let end = labels[start..].find('"')?;
    Some(&labels[start..start + end])
}

/// cumulative counters and heights are integral and far below 2^53, so the
/// exposition's f64 round-trips them exactly.
fn integral(value: f64) -> u64 {
    value as u64
}

#[derive(Default)]
struct PeerAccum {
    connected_since_ms: Option<u64>,
    msgs_sent: u64,
    msgs_received: u64,
    sync_bytes: Option<u64>,
    sync_frames: u64,
    sync_boundary: Option<u64>,
    sync_served: Option<u64>,
    sync_idle: u64,
    sync_age: u64,
    sync_last_kind: Option<String>,
}

impl PeerAccum {
    fn into_view(self, peer: String) -> PeerView {
        let statesync = self.sync_bytes.map(|bytes_tx| StatesyncServeView {
            bytes_tx,
            frames_served: self.sync_frames,
            boundary_height: self.sync_boundary,
            served_height: self.sync_served,
            idle_seconds: self.sync_idle,
            age_seconds: self.sync_age,
            last_request_kind: self.sync_last_kind,
        });
        PeerView {
            peer,
            connected: self.connected_since_ms.is_some(),
            connected_since_ms: self.connected_since_ms,
            role: None,
            msgs_sent: self.msgs_sent,
            msgs_received: self.msgs_received,
            statesync,
        }
    }
}

/// assemble the peer sample from one metrics exposition. pure: the caller
/// supplies the sample time (the lane's runtime clock). only the families
/// this projection owns mint a peer entry — other peer-labeled series (the
/// consensus engine's vote gauges include SELF under `peer=`) never do.
pub fn peers_from_exposition(exposition: &str, sampled_at_ms: u64) -> PeersView {
    let mut accums: BTreeMap<String, PeerAccum> = BTreeMap::new();
    for line in exposition.lines() {
        let Some((family, labels, value)) = parse_sample(line) else {
            continue;
        };
        let Some(peer) = label(labels, "peer") else {
            continue;
        };
        /// the line's peer entry — named once, used by every arm.
        macro_rules! entry {
            () => {
                accums.entry(peer.to_string()).or_default()
            };
        }
        match family {
            "network_tracker_directory_connected" => {
                entry!().connected_since_ms = Some(integral(value));
            }
            "network_spawner_messages_sent_total" => entry!().msgs_sent += integral(value),
            "network_spawner_messages_received_total" => {
                entry!().msgs_received += integral(value);
            }
            "ducktape_statesync_serve_bytes" => entry!().sync_bytes = Some(integral(value)),
            "ducktape_statesync_serve_frames" => entry!().sync_frames = integral(value),
            "ducktape_statesync_serve_boundary_height" => {
                entry!().sync_boundary = Some(integral(value));
            }
            "ducktape_statesync_serve_frame_height" => {
                entry!().sync_served = Some(integral(value));
            }
            "ducktape_statesync_serve_idle_seconds" => entry!().sync_idle = integral(value),
            "ducktape_statesync_serve_age_seconds" => entry!().sync_age = integral(value),
            "ducktape_statesync_serve_last_request" => {
                entry!().sync_last_kind = label(labels, "kind").map(str::to_string);
            }
            _ => {}
        }
    }
    PeersView {
        sampled_at_ms,
        peers: accums
            .into_iter()
            .map(|(peer, accum)| accum.into_view(peer))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// a trimmed capture of a live node's exposition: one peer connection-
    /// tracked with mesh counters and a full statesync serve conversation,
    /// plus the consensus vote gauge that must NOT mint a peer (it labels
    /// SELF under `peer=` too).
    const EXPOSITION: &str = r#"# HELP network_tracker_directory_connected Unix timestamp in milliseconds when each connected peer became active.
# TYPE network_tracker_directory_connected gauge
network_tracker_directory_connected{peer="e653"} 1784737525068
consensus_e1_engine_batcher_latest_vote{peer="7c35"} 0
network_spawner_messages_sent_total{peer="e653",message="data_15"} 5090
network_spawner_messages_sent_total{peer="e653",message="greeting"} 1
network_spawner_messages_received_total{peer="e653",message="bit_vec"} 258
ducktape_statesync_serve_bytes{peer="e653"} 38597
ducktape_statesync_serve_frames{peer="e653"} 2
ducktape_statesync_serve_boundary_height{peer="e653"} 230
ducktape_statesync_serve_frame_height{peer="e653"} 230
ducktape_statesync_serve_idle_seconds{peer="e653"} 416
ducktape_statesync_serve_age_seconds{peer="e653"} 500
ducktape_statesync_serve_last_request{peer="e653",kind="tip_coords"} 1
"#;

    #[test]
    fn live_capture_projects_one_connected_peer() {
        let view = peers_from_exposition(EXPOSITION, 7);
        assert_eq!(view.sampled_at_ms, 7);
        assert_eq!(
            view.peers,
            vec![PeerView {
                peer: "e653".into(),
                connected: true,
                connected_since_ms: Some(1784737525068),
                role: None,
                msgs_sent: 5091,
                msgs_received: 258,
                statesync: Some(StatesyncServeView {
                    bytes_tx: 38597,
                    frames_served: 2,
                    boundary_height: Some(230),
                    served_height: Some(230),
                    idle_seconds: 416,
                    age_seconds: 500,
                    last_request_kind: Some("tip_coords".into()),
                }),
            }]
        );
    }

    /// a peer only the serve lane remembers (its connection already closed)
    /// stays listed, marked not-connected, with no progression invented.
    #[test]
    fn serve_only_peer_is_listed_disconnected() {
        let exposition = "ducktape_statesync_serve_bytes{peer=\"aa\"} 128\n";
        let view = peers_from_exposition(exposition, 0);
        assert_eq!(view.peers.len(), 1);
        let peer = &view.peers[0];
        assert!(!peer.connected);
        assert_eq!(peer.connected_since_ms, None);
        let sync = peer.statesync.as_ref().expect("serve view");
        assert_eq!(sync.bytes_tx, 128);
        assert_eq!(sync.boundary_height, None);
        assert_eq!(sync.served_height, None);
    }

    /// families outside the projection never mint a peer entry, and a
    /// mesh-only peer carries no statesync view.
    #[test]
    fn unrelated_peer_labels_and_missing_lanes_stay_absent() {
        let exposition = "consensus_e1_engine_batcher_latest_vote{peer=\"self\"} 3\n\
                          network_tracker_directory_connected{peer=\"bb\"} 1000\n";
        let view = peers_from_exposition(exposition, 0);
        assert_eq!(view.peers.len(), 1);
        assert_eq!(view.peers[0].peer, "bb");
        assert_eq!(view.peers[0].statesync, None);
    }

    /// the json shape is camelCase with absent options omitted — the wire
    /// contract the CLI and any dashboard key on.
    #[test]
    fn serializes_camel_case_without_absent_options() {
        let view = peers_from_exposition(
            "network_tracker_directory_connected{peer=\"bb\"} 1000\n",
            42,
        );
        let json = serde_json::to_string(&view).expect("serializes");
        assert_eq!(
            json,
            r#"{"sampledAtMs":42,"peers":[{"peer":"bb","connected":true,"connectedSinceMs":1000,"msgsSent":0,"msgsReceived":0}]}"#
        );
    }
}
