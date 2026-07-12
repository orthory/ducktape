//! observing the serve side: the process-wide [`ServeMonitor`] a serving
//! node's statesync loop records every answered request into, keyed by the
//! requesting peer.
//!
//! statesync has no per-peer session object to watch (each request is
//! self-contained — the carrier multiplexes every peer over one channel), so
//! unlike the data-plane crate's `PlaneMonitor` pattern there is nothing
//! whose death could prune an entry. presence is therefore RECENCY:
//! a peer that stops asking ages out of the snapshot after [`SERVE_EXPIRE`].
//! every consumer of a snapshot sees only peers this node actually served
//! recently — "the state-sync lane is being utilized" is exactly the
//! non-emptiness of the snapshot.
//!
//! the monitor is transport-blind: peers are the opaque label strings the
//! carrier hands it (the node passes the mesh key's hex), and everything it
//! learns comes from the `(request kind, response)` pairs the serve loop
//! already produces. progression is read off the responses themselves — a
//! served [`SyncResponse::Manifest`] names the boundary the peer restores
//! from, and each [`SyncResponse::Frames`] batch advances the highest block
//! height this node has handed that peer.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::SyncResponse;

/// how long a silent peer stays in the snapshot. well above every routine
/// request cadence on the lane (a syncing joiner asks continuously; a parked
/// resident polls tip coordinates every 2–12 s), so an entry expiring means
/// the peer is genuinely done or gone — not between requests.
pub const SERVE_EXPIRE: Duration = Duration::from_secs(600);

/// one peer's serve-side accounting at snapshot time. cumulative fields
/// cover the peer's presence in the monitor (an expired-and-returned peer
/// starts over — the counters describe the CURRENT sync conversation).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeerServeReport {
    /// the carrier's label for the peer (the node passes the mesh key hex).
    pub peer: String,
    /// time since this node first served the peer (this conversation).
    pub age: Duration,
    /// time since the last answered request.
    pub idle: Duration,
    /// cumulative wire bytes sent to the peer (rpc-framed responses).
    pub bytes_tx: u64,
    /// cumulative finalized frames (blocks) served to the peer.
    pub frames_served: u64,
    /// the boundary height of the last manifest served to the peer — the
    /// height its snapshot restore lands on. `None` until a manifest is
    /// served (tip pollers and blob fetchers never have one).
    pub boundary_height: Option<u64>,
    /// the highest frame (block) height served to the peer — how far its
    /// replay has gotten on the frames this node handed it. `None` until a
    /// frames batch is served.
    pub served_height: Option<u64>,
    /// cumulative answered requests by request kind
    /// ([`crate::SyncRequest::kind_name`]), sorted by kind.
    pub requests: Vec<(&'static str, u64)>,
}

struct PeerEntry {
    first_active: Instant,
    last_active: Instant,
    bytes_tx: u64,
    frames_served: u64,
    boundary_height: Option<u64>,
    served_height: Option<u64>,
    requests: BTreeMap<&'static str, u64>,
}

/// the registry of recently served peers. cloneable handle; the serve loop
/// `record`s, observers `snapshot`. entries expire by idleness — see the
/// module doc.
#[derive(Clone, Default)]
pub struct ServeMonitor {
    peers: Arc<Mutex<HashMap<String, PeerEntry>>>,
}

impl ServeMonitor {
    /// record one answered request: `kind` is the REQUEST's kind (an Error
    /// response still counts against what was asked), `response` is what was
    /// served, `wire_bytes` the framed bytes that went out for it.
    pub fn record(&self, peer: &str, kind: &'static str, response: &SyncResponse, wire_bytes: u64) {
        let now = Instant::now();
        let mut peers = self.peers.lock().expect("serve monitor lock");
        let entry = peers
            .entry(peer.to_string())
            .or_insert_with(|| PeerEntry {
                first_active: now,
                last_active: now,
                bytes_tx: 0,
                frames_served: 0,
                boundary_height: None,
                served_height: None,
                requests: BTreeMap::new(),
            });
        entry.last_active = now;
        entry.bytes_tx += wire_bytes;
        *entry.requests.entry(kind).or_insert(0) += 1;
        match response {
            SyncResponse::Manifest(m) => entry.boundary_height = Some(m.height),
            SyncResponse::Frames { frames } => {
                entry.frames_served += frames.len() as u64;
                let batch_top = frames.iter().map(|f| f.height).max();
                entry.served_height = entry.served_height.max(batch_top);
            }
            _ => {}
        }
    }

    /// report every recently served peer, pruning entries idle past
    /// [`SERVE_EXPIRE`]. sorted by peer for a stable exposition order.
    pub fn snapshot(&self) -> Vec<PeerServeReport> {
        let now = Instant::now();
        let mut peers = self.peers.lock().expect("serve monitor lock");
        peers.retain(|_, entry| now.duration_since(entry.last_active) < SERVE_EXPIRE);
        let mut reports: Vec<PeerServeReport> = peers
            .iter()
            .map(|(peer, entry)| PeerServeReport {
                peer: peer.clone(),
                age: now.duration_since(entry.first_active),
                idle: now.duration_since(entry.last_active),
                bytes_tx: entry.bytes_tx,
                frames_served: entry.frames_served,
                boundary_height: entry.boundary_height,
                served_height: entry.served_height,
                requests: entry.requests.iter().map(|(k, v)| (*k, *v)).collect(),
            })
            .collect();
        reports.sort_by(|a, b| a.peer.cmp(&b.peer));
        reports
    }
}

impl std::fmt::Debug for ServeMonitor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let peers = self.peers.lock().expect("serve monitor lock");
        f.debug_struct("ServeMonitor")
            .field("peers", &peers.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FinalizedFrame, FrameDisposition, Manifest, SyncRequest, TipCoords};
    use sdk::StateRoot;

    fn manifest_at(height: u64) -> Manifest {
        Manifest {
            height,
            app_hash: StateRoot([7u8; 32]),
            epoch: 0,
            view_base: 0,
            participants: vec![],
            residents: vec![],
            floor_cert: None,
            current_version: 1,
            pending_upgrade: None,
            required_min_version: 1,
            state_schema: [0u8; 32],
            entries: vec![],
        }
    }

    fn frame_at(height: u64) -> FinalizedFrame {
        FinalizedFrame {
            height,
            frame: vec![],
            disposition: FrameDisposition::Applied,
            roots: vec![],
            app_hash: StateRoot([9u8; 32]),
        }
    }

    /// a full serve conversation accumulates: manifest sets the boundary,
    /// frames batches advance the served height monotonically, every answer
    /// adds bytes and a kind-labeled request count.
    #[test]
    fn conversation_accumulates_progress() {
        let monitor = ServeMonitor::default();
        let manifest = SyncResponse::Manifest(manifest_at(100));
        monitor.record("aa", SyncRequest::Manifest.kind_name(), &manifest, 512);
        let batch = SyncResponse::Frames {
            frames: vec![frame_at(101), frame_at(102)],
        };
        monitor.record("aa", "frames", &batch, 2048);
        // a re-request of an OLDER range must not regress the high-water mark.
        let stale = SyncResponse::Frames {
            frames: vec![frame_at(101)],
        };
        monitor.record("aa", "frames", &stale, 1024);

        let reports = monitor.snapshot();
        assert_eq!(reports.len(), 1);
        let r = &reports[0];
        assert_eq!(r.peer, "aa");
        assert_eq!(r.bytes_tx, 512 + 2048 + 1024);
        assert_eq!(r.frames_served, 3);
        assert_eq!(r.boundary_height, Some(100));
        assert_eq!(r.served_height, Some(102));
        assert_eq!(r.requests, vec![("frames", 2), ("manifest", 1)]);
    }

    /// a tip poller never gains progression fields — only utilization.
    #[test]
    fn tip_polling_reports_no_heights() {
        let monitor = ServeMonitor::default();
        let coords = SyncResponse::TipCoords(TipCoords {
            height: 42,
            app_hash: StateRoot([1u8; 32]),
            epoch: 0,
            view_base: 0,
            participants: vec![],
            residents: vec![],
            has_floor: true,
        });
        monitor.record("bb", SyncRequest::TipCoords.kind_name(), &coords, 128);
        let reports = monitor.snapshot();
        assert_eq!(reports[0].boundary_height, None);
        assert_eq!(reports[0].served_height, None);
        assert_eq!(reports[0].requests, vec![("tip_coords", 1)]);
    }

    /// peers are independent entries, snapshot-sorted by label.
    #[test]
    fn peers_are_isolated_and_sorted() {
        let monitor = ServeMonitor::default();
        let resp = SyncResponse::Error("not ready".into());
        monitor.record("zz", "manifest", &resp, 64);
        monitor.record("aa", "manifest", &resp, 32);
        let reports = monitor.snapshot();
        assert_eq!(
            reports.iter().map(|r| r.peer.as_str()).collect::<Vec<_>>(),
            vec!["aa", "zz"]
        );
        assert_eq!(reports[0].bytes_tx, 32);
        assert_eq!(reports[1].bytes_tx, 64);
    }
}
