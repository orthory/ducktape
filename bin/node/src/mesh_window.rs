//! the ONE mesh peer-set tracking discipline: every node tracks the
//! IDENTICAL generation-keyed window derived from replicated valset state.
//!
//! index = membership GENERATION (the valset counter), not the engine epoch:
//! a grant advances the generation at its commit block, so the widened mesh
//! is trackable immediately — the epoch cutover (which lags by
//! `CUTOVER_DELAY` views) stays an engine/channel concern. the discovery
//! directory accepts only strictly-increasing indices (a duplicate is
//! warn-dropped in commonware), and generation indices are monotone by
//! construction; index GAPS are fine — a statesync'd joiner skips the
//! generations it never saw, and both sides ignore bit-vectors for an index
//! they don't track.
//!
//! ## primary vs secondary — the load-bearing split
//!
//! the PRIMARY set at every index is exactly the replicated generation
//! snapshot (`validators ∪ residents`), identical on every node by
//! construction — it is the set discovery builds its shared bit-vector over,
//! where a composition mismatch at a shared index kills the link. the
//! node-local descriptor dial set rides as SECONDARY on every tracked index:
//! connection-eligible and dialable, but never in the bit-vector, so per-node
//! differences (an invite-stripped descriptor, dev peer_seeds extras, a
//! demoted member kept reachable) can never kill a link. connection
//! authorization is the union of every tracked set on each side.
//!
//! index 0 is GENESIS: the descriptor's fingerprinted validator list,
//! byte-equal to valset's generation-0 snapshot — the one index a node can
//! track before it has synced any state.

use std::collections::BTreeSet;

use commonware_codec::DecodeExt as _;
use commonware_cryptography::ed25519;
use commonware_p2p::{Manager as _, TrackedPeers, authenticated::discovery};
use commonware_utils::ordered::Set;

/// the genesis window index — generation 0.
const GENESIS_INDEX: u64 = 0;

/// the tracker's write seam onto the mesh oracle — exactly the one call it
/// makes, injected so the parity tests observe `(index, primary, secondary)`
/// without standing up a discovery `Network`.
pub(crate) trait TrackSink {
    fn track_set(&mut self, index: u64, peers: TrackedPeers<ed25519::PublicKey>);
}

impl TrackSink for discovery::Oracle<ed25519::PublicKey> {
    fn track_set(&mut self, index: u64, peers: TrackedPeers<ed25519::PublicKey>) {
        // Feedback is deliberately discarded — tracking is fire-and-forget
        // at every existing site, and a dropped mailbox send surfaces as the
        // network actor's own failure, not ours.
        let _ = self.track(index, peers);
    }
}

/// the per-node mesh window tracker: monotonic bookkeeping over the oracle's
/// generation indices. ONE instance follows a node through its roles (parked
/// → promoted), because oracle clones share one directory — `last_tracked`
/// must travel with it.
pub(crate) struct MeshWindowTracker {
    /// the highest generation index handed to the oracle — mirrors the
    /// directory's own strictly-increasing contract, so repeated window
    /// syncs are naturally no-ops.
    last_tracked: Option<u64>,
    /// the node-local descriptor dial set, SECONDARY on every tracked index
    /// (see the module doc).
    extras: Set<ed25519::PublicKey>,
    /// this node's log label.
    label: String,
}

impl MeshWindowTracker {
    pub(crate) fn new(descriptor_mesh: &[ed25519::PublicKey], label: impl Into<String>) -> Self {
        let extras: BTreeSet<ed25519::PublicKey> = descriptor_mesh.iter().cloned().collect();
        Self {
            last_tracked: None,
            extras: ordered_set(extras),
            label: label.into(),
        }
    }

    /// track index 0 — the genesis snapshot: primary = the descriptor's
    /// fingerprinted VALIDATORS (byte-equal to valset's generation-0 record),
    /// secondary = the descriptor dial set. the one track a node performs
    /// before it can read state; a no-op if any index is already tracked.
    pub(crate) fn track_genesis(
        &mut self,
        sink: &mut impl TrackSink,
        descriptor_validators: &[ed25519::PublicKey],
    ) {
        if self.last_tracked.is_some() {
            return;
        }
        let primary: BTreeSet<ed25519::PublicKey> = descriptor_validators.iter().cloned().collect();
        sink.track_set(
            GENESIS_INDEX,
            TrackedPeers::new(ordered_set(primary), self.extras.clone()),
        );
        self.last_tracked = Some(GENESIS_INDEX);
        tracing::debug!(
            target: "ducktape::node",
            node = %self.label,
            validators = descriptor_validators.len(),
            "mesh genesis window tracked"
        );
    }

    /// track every window generation above `last_tracked`, ascending, each
    /// with primary = the snapshot's `validators ∪ residents` (undecodable
    /// keys dropped — dead serving hints) and secondary = the descriptor
    /// extras. returns the LATEST snapshot when anything advanced, so
    /// callers can feed the plane books; `None` on a no-advance call, which
    /// stays silent (this runs once per drain pass).
    pub(crate) fn track_new<'w>(
        &mut self,
        sink: &mut impl TrackSink,
        window: &'w [valset::GenerationSet],
    ) -> Option<&'w valset::GenerationSet> {
        let from = self.last_tracked;
        let mut advanced: Option<&valset::GenerationSet> = None;
        for entry in window {
            let already_tracked = self
                .last_tracked
                .is_some_and(|last| entry.generation <= last);
            if already_tracked {
                continue;
            }
            let primary = snapshot_primary(entry);
            sink.track_set(
                entry.generation,
                TrackedPeers::new(primary, self.extras.clone()),
            );
            self.last_tracked = Some(entry.generation);
            advanced = Some(entry);
        }
        let latest = advanced?;
        tracing::info!(
            target: "ducktape::node",
            event = "mesh_window_tracked",
            node = %self.label,
            generation = latest.generation,
            from = from.map(|g| g as i64).unwrap_or(-1),
            members = latest.validators.len(),
            residents = latest.residents.len(),
            "mesh window advanced"
        );
        Some(latest)
    }

    #[cfg(test)]
    pub(crate) fn last_tracked(&self) -> Option<u64> {
        self.last_tracked
    }
}

/// the sync-wire window in tracker shape — the parked joiner's tip poll
/// carries [`statesync::MeshWindowEntry`]s (statesync stays valset-agnostic);
/// this is the one conversion back.
pub(crate) fn window_from_sync(entries: &[statesync::MeshWindowEntry]) -> Vec<valset::GenerationSet> {
    entries
        .iter()
        .map(|e| valset::GenerationSet {
            generation: e.generation,
            validators: e.validators.clone(),
            residents: e.residents.clone(),
        })
        .collect()
}

/// one snapshot's PRIMARY set: `validators ∪ residents`, decoded, sorted,
/// deduped. identical on every node because the snapshot is replicated state.
fn snapshot_primary(entry: &valset::GenerationSet) -> Set<ed25519::PublicKey> {
    let union: BTreeSet<ed25519::PublicKey> = entry
        .validators
        .iter()
        .chain(entry.residents.iter())
        .filter_map(|k| ed25519::PublicKey::decode(k.as_slice()).ok())
        .collect();
    ordered_set(union)
}

fn ordered_set(keys: BTreeSet<ed25519::PublicKey>) -> Set<ed25519::PublicKey> {
    Set::try_from(keys.into_iter().collect::<Vec<_>>())
        .expect("a btree-set union has no duplicates")
}

#[cfg(test)]
mod tests {
    use super::*;
    use commonware_cryptography::Signer as _;
    use commonware_cryptography::ed25519::PrivateKey;

    /// records every (index, primary, secondary) handed to the oracle.
    #[derive(Default)]
    struct Recorder {
        tracked: Vec<(u64, Vec<ed25519::PublicKey>, Vec<ed25519::PublicKey>)>,
    }
    impl TrackSink for Recorder {
        fn track_set(&mut self, index: u64, peers: TrackedPeers<ed25519::PublicKey>) {
            self.tracked.push((
                index,
                peers.primary.iter().cloned().collect(),
                peers.secondary.iter().cloned().collect(),
            ));
        }
    }

    fn key(seed: u8) -> ed25519::PublicKey {
        PrivateKey::decode(&[seed; 32][..])
            .expect("any 32 bytes is a valid seed")
            .public_key()
    }
    fn key_bytes(seed: u8) -> Vec<u8> {
        key(seed).as_ref().to_vec()
    }
    fn snapshot(generation: u64, validators: &[u8], residents: &[u8]) -> valset::GenerationSet {
        valset::GenerationSet {
            generation,
            validators: validators.iter().map(|s| key_bytes(*s)).collect(),
            residents: residents.iter().map(|s| key_bytes(*s)).collect(),
        }
    }
    /// keys sorted by their DERIVED public-key bytes (the Set order), not by
    /// seed — expectations stay honest about what the wire orders on.
    fn sorted_keys(seeds: &[u8]) -> Vec<ed25519::PublicKey> {
        let set: BTreeSet<ed25519::PublicKey> = seeds.iter().map(|s| key(*s)).collect();
        set.into_iter().collect()
    }

    /// the depth pin: the module-side retained window and the transport's
    /// tracked-set window must be the SAME depth — the two moving apart
    /// would let one side track sets the other has already pruned.
    #[test]
    fn retained_depth_matches_discovery_tracked_peer_sets() {
        let signer = PrivateKey::decode(&[1u8; 32][..]).unwrap();
        let cfg = discovery::Config::local(
            signer,
            b"depth-pin",
            "127.0.0.1:0".parse().unwrap(),
            "127.0.0.1:0".parse::<std::net::SocketAddr>().unwrap(),
            Vec::new(),
            1 << 20,
        );
        assert_eq!(
            cfg.tracked_peer_sets.get() as u64,
            valset::RETAINED_GENERATIONS,
            "valset::RETAINED_GENERATIONS must equal discovery's tracked_peer_sets"
        );
    }

    /// the parity property: two nodes with DIFFERENT extras produce the
    /// identical (index, primary) sequence from the same window — extras
    /// never leak into the shared bit-vector set.
    #[test]
    fn identical_primary_sequence_regardless_of_extras() {
        let window = [snapshot(3, &[1, 2], &[3]), snapshot(4, &[1, 2, 3], &[])];
        let mut a = MeshWindowTracker::new(&[key(1), key(9)], "a");
        let mut b = MeshWindowTracker::new(&[], "b");
        let (mut sink_a, mut sink_b) = (Recorder::default(), Recorder::default());
        a.track_new(&mut sink_a, &window);
        b.track_new(&mut sink_b, &window);

        let primaries =
            |r: &Recorder| -> Vec<(u64, Vec<ed25519::PublicKey>)> {
                r.tracked.iter().map(|(i, p, _)| (*i, p.clone())).collect()
            };
        assert_eq!(primaries(&sink_a), primaries(&sink_b));
        assert_eq!(
            sink_a.tracked[0].2,
            sorted_keys(&[1, 9]),
            "extras ride as secondary"
        );
        assert!(sink_b.tracked[0].2.is_empty());
    }

    #[test]
    fn genesis_primary_is_the_descriptor_validators_and_only_tracks_once() {
        let mut t = MeshWindowTracker::new(&[key(1), key(7)], "n");
        let mut sink = Recorder::default();
        t.track_genesis(&mut sink, &[key(2), key(1)]);
        t.track_genesis(&mut sink, &[key(2), key(1)]);
        assert_eq!(sink.tracked.len(), 1, "genesis tracks exactly once");
        let (index, primary, secondary) = &sink.tracked[0];
        assert_eq!(*index, 0);
        assert_eq!(*primary, sorted_keys(&[2, 1]), "sorted descriptor validators");
        assert_eq!(*secondary, sorted_keys(&[1, 7]));
    }

    #[test]
    fn window_skips_at_or_below_last_tracked_and_advances_ascending() {
        let mut t = MeshWindowTracker::new(&[], "n");
        let mut sink = Recorder::default();
        t.track_genesis(&mut sink, &[key(1)]);

        // a statesync'd window that SKIPS generations (1..4 never seen).
        let window = [snapshot(5, &[1], &[2]), snapshot(6, &[1, 2], &[])];
        let latest = t.track_new(&mut sink, &window).expect("advanced");
        assert_eq!(latest.generation, 6);
        assert_eq!(t.last_tracked(), Some(6));
        let indices: Vec<u64> = sink.tracked.iter().map(|(i, _, _)| *i).collect();
        assert_eq!(indices, vec![0, 5, 6], "gap-tolerant, strictly ascending");

        // a re-sync of the same window is a silent no-op.
        assert!(t.track_new(&mut sink, &window).is_none());
        assert_eq!(sink.tracked.len(), 3);
    }

    #[test]
    fn snapshot_primary_unions_tiers_and_drops_undecodable_keys() {
        let mut entry = snapshot(2, &[2], &[1]);
        entry.validators.push(vec![0u8; 7]); // a dead serving hint
        let primary = snapshot_primary(&entry);
        let keys: Vec<ed25519::PublicKey> = primary.iter().cloned().collect();
        assert_eq!(keys, sorted_keys(&[1, 2]), "union, sorted, junk dropped");
    }
}
