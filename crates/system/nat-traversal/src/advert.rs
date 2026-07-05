use std::collections::HashMap;
use std::net::SocketAddr;

use crate::NodeKey;

/// One node's latest reflexive advertisement: the reflexive `SocketAddr` a node
/// published and the monotonic `nonce` that orders it. The nonce is an ordering
/// token only — the address is always the coordinator-observed source, never a
/// self-reported one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReflexiveAdvert {
    pub reflexive: SocketAddr,
    pub nonce: u64,
}

/// Result of applying a re-advertisement: it either superseded the stored
/// mapping or was rejected as stale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdvertOutcome {
    Superseded,
    Stale,
}

/// The reachability-plane reflexive registry: for each node key, the latest
/// accepted `ReflexiveAdvert`. `observe` is the unconditional boot/live
/// registration (the observed source is authoritative); `readvertise` is the
/// nonce-gated rebind path.
///
/// The nonce rule deliberately MIRRORS `wireguard_upgrade::MeshView::verify`'s
/// duplicate-advertisement rule (`nonce <= prev => StaleDuplicateAdvertisement`)
/// so a NAT-rebound node re-advertises under a strictly-higher nonce to
/// supersede its stale mapping — WITHOUT this crate depending on
/// `wireguard-upgrade` or any validator-identity type (the Slice 2 invariant).
/// Cap on distinct advertised keys the coordinator will hold. The `NodeKey` in
/// a `Register` is fully attacker-chosen, so an UNBOUNDED map is a trivial
/// memory-exhaustion vector: a stranger sprays millions of random keys and the
/// resident set grows without limit. At the cap, a first-seen key evicts the
/// key with the LOWEST nonce (a still-at-baseline entry the sender can always
/// re-advertise) rather than accepting unbounded growth. Generous for any real
/// validator mesh; a DoS backstop, not a working limit.
const MAX_ADVERTS: usize = 4096;

#[derive(Default)]
pub struct AdvertBook {
    latest: HashMap<NodeKey, ReflexiveAdvert>,
}

impl AdvertBook {
    /// Boot/live registration at the nonce-0 baseline. The coordinator-observed
    /// `src` is authoritative. This establishes the baseline for a first-seen key
    /// and refreshes it while still at the baseline, but it is NOT unconditional:
    /// once a rebind re-advertisement has advanced the stored nonce above 0, a
    /// later (necessarily nonce-0) `Register` — which under UDP may be a
    /// duplicated, reordered, or replayed datagram from the STALE mapping — must
    /// not roll the fresh mapping back. Only `readvertise` moves a superseded
    /// mapping, and only under a strictly-higher nonce.
    pub fn observe(&mut self, key: NodeKey, src: SocketAddr) {
        match self.latest.get(&key) {
            // Already superseded past the boot baseline: a nonce-0 Register is
            // stale by construction and cannot roll it back.
            Some(prev) if prev.nonce > 0 => {}
            _ => {
                self.evict_if_full(&key);
                self.latest.insert(key, ReflexiveAdvert { reflexive: src, nonce: 0 });
            }
        }
    }

    /// Before inserting a NEW key at the cap, drop the lowest-nonce entry so an
    /// attacker spraying fresh random keys cannot grow the map without bound.
    /// A no-op when the key already exists (an update, not growth) or there is
    /// room.
    fn evict_if_full(&mut self, incoming: &NodeKey) {
        if self.latest.contains_key(incoming) || self.latest.len() < MAX_ADVERTS {
            return;
        }
        if let Some(victim) = self
            .latest
            .iter()
            .min_by_key(|(_, a)| a.nonce)
            .map(|(k, _)| *k)
        {
            self.latest.remove(&victim);
        }
    }

    /// Rebind re-advertisement. A strictly-higher `nonce` supersedes the stored
    /// mapping (store `src`, return `Superseded`); an equal-or-lower nonce is
    /// stale and leaves the stored advert untouched (`Stale`). No prior entry ->
    /// accepted as a first advert.
    pub fn readvertise(&mut self, key: NodeKey, src: SocketAddr, nonce: u64) -> AdvertOutcome {
        match self.latest.get(&key) {
            Some(prev) if nonce <= prev.nonce => AdvertOutcome::Stale,
            _ => {
                self.evict_if_full(&key);
                self.latest.insert(key, ReflexiveAdvert { reflexive: src, nonce });
                AdvertOutcome::Superseded
            }
        }
    }

    pub fn current(&self, key: NodeKey) -> Option<SocketAddr> {
        self.latest.get(&key).map(|a| a.reflexive)
    }

    /// Reverse-map an observed source back to the key that advertised it. Used
    /// by the coordinator to bind a caller's datagram source to its identity.
    pub fn key_for_src(&self, src: SocketAddr) -> Option<NodeKey> {
        self.latest
            .iter()
            .find(|(_, a)| a.reflexive == src)
            .map(|(k, _)| *k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    fn addr(o: u8, p: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, o)), p)
    }

    #[test]
    fn first_observe_then_higher_nonce_supersedes() {
        let key = NodeKey([0xaa; 32]);
        let mut book = AdvertBook::default();
        book.observe(key, addr(1, 4000)); // boot: nonce 0
        assert_eq!(book.current(key), Some(addr(1, 4000)));

        // A rebind advertises the NEW reflexive under a strictly-higher nonce:
        // it supersedes the stale mapping.
        assert_eq!(book.readvertise(key, addr(2, 5000), 1), AdvertOutcome::Superseded);
        assert_eq!(book.current(key), Some(addr(2, 5000)));
        assert_eq!(book.key_for_src(addr(2, 5000)), Some(key));
        assert_eq!(book.key_for_src(addr(1, 4000)), None, "stale mapping is gone");
    }

    #[test]
    fn equal_or_lower_nonce_is_stale_and_does_not_change_mapping() {
        let key = NodeKey([0xbb; 32]);
        let mut book = AdvertBook::default();
        book.observe(key, addr(1, 4000)); // nonce 0
        assert_eq!(book.readvertise(key, addr(2, 5000), 2), AdvertOutcome::Superseded);

        // A replayed / equal-nonce advert must not clobber the fresher mapping
        // (mirrors StaleDuplicateAdvertisement: nonce <= prev).
        assert_eq!(book.readvertise(key, addr(9, 9999), 2), AdvertOutcome::Stale);
        assert_eq!(book.readvertise(key, addr(9, 9999), 1), AdvertOutcome::Stale);
        assert_eq!(book.current(key), Some(addr(2, 5000)), "stale adverts leave state untouched");
    }

    #[test]
    fn observe_does_not_roll_back_a_superseded_higher_nonce_mapping() {
        let key = NodeKey([0xdd; 32]);
        let mut book = AdvertBook::default();
        book.observe(key, addr(1, 4000)); // boot: nonce 0
        assert_eq!(book.readvertise(key, addr(2, 5000), 1), AdvertOutcome::Superseded);

        // A replayed/reordered boot Register (observe) from the STALE source must
        // NOT roll the fresh nonce-1 mapping back to the old one.
        book.observe(key, addr(1, 4000));
        assert_eq!(
            book.current(key),
            Some(addr(2, 5000)),
            "a stale nonce-0 register cannot clobber a rebind re-advertisement"
        );
        assert_eq!(book.key_for_src(addr(1, 4000)), None, "stale mapping stays gone");
    }

    #[test]
    fn observe_still_refreshes_while_at_the_boot_baseline() {
        // Before any rebind (still nonce 0), a re-register updates the mapping —
        // the fix only blocks rollback of an already-superseded mapping.
        let key = NodeKey([0xee; 32]);
        let mut book = AdvertBook::default();
        book.observe(key, addr(1, 4000));
        book.observe(key, addr(3, 7000));
        assert_eq!(
            book.current(key),
            Some(addr(3, 7000)),
            "a nonce-0 re-register refreshes the baseline mapping"
        );
    }

    #[test]
    fn unknown_key_has_no_current() {
        let book = AdvertBook::default();
        assert_eq!(book.current(NodeKey([0xcc; 32])), None);
    }

    #[test]
    fn the_book_is_capped_against_key_spray() {
        // an attacker registering random keys must not grow the map without
        // bound: at the cap a new key evicts the lowest-nonce (baseline) entry.
        let mut book = AdvertBook::default();
        for i in 0..(MAX_ADVERTS as u64) {
            let mut k = [0u8; 32];
            k[..8].copy_from_slice(&i.to_le_bytes());
            book.observe(NodeKey(k), addr(1, 4000));
        }
        assert_eq!(book.latest.len(), MAX_ADVERTS);
        // promote one entry above the baseline so it survives eviction.
        let mut kept = [0u8; 32];
        kept[..8].copy_from_slice(&7u64.to_le_bytes());
        assert_eq!(
            book.readvertise(NodeKey(kept), addr(2, 5000), 9),
            AdvertOutcome::Superseded
        );
        // one more spray key stays at the cap and does not evict the promoted one.
        book.observe(NodeKey([0xff; 32]), addr(3, 6000));
        assert_eq!(book.latest.len(), MAX_ADVERTS);
        assert_eq!(book.current(NodeKey(kept)), Some(addr(2, 5000)));
    }
}
