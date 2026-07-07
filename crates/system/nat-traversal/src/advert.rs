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
    /// Wall-clock seconds of the last ACCEPTED advert (an `observe` or a
    /// superseding `readvertise`). A stale-nonce replay never refreshes this —
    /// only fresh proof of life extends a mapping.
    pub last_seen: u64,
}

/// How long a registration stays resolvable after its last accepted advert.
/// A NAT's UDP pinhole dies in ~30 s of silence, so a mapping the node has
/// not refreshed for two minutes (≈5 missed 25 s keepalives —
/// `reachability::RENDEZVOUS_KEEPALIVE`) points at a dead hole: answering
/// lookups with it, or fanning `PunchSync` at it, is worse than an honest
/// `None`.
pub const REGISTRATION_TTL_SECS: u64 = 120;

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

pub struct AdvertBook {
    latest: HashMap<NodeKey, ReflexiveAdvert>,
    ttl: u64,
}

impl Default for AdvertBook {
    fn default() -> Self {
        Self {
            latest: HashMap::new(),
            ttl: REGISTRATION_TTL_SECS,
        }
    }
}

impl AdvertBook {
    /// A book with an explicit TTL (seconds). Tests and short-lived rigs
    /// shrink it; production uses [`REGISTRATION_TTL_SECS`] via `Default`.
    pub fn with_ttl(ttl: u64) -> Self {
        Self {
            latest: HashMap::new(),
            ttl,
        }
    }

    fn expired(&self, advert: &ReflexiveAdvert, now: u64) -> bool {
        now.saturating_sub(advert.last_seen) > self.ttl
    }

    /// Boot/live registration at the nonce-0 baseline. The coordinator-observed
    /// `src` is authoritative. This establishes the baseline for a first-seen key
    /// and refreshes it while still at the baseline, but it is NOT unconditional:
    /// once a rebind re-advertisement has advanced the stored nonce above 0, a
    /// later (necessarily nonce-0) `Register` — which under UDP may be a
    /// duplicated, reordered, or replayed datagram from the STALE mapping — must
    /// not roll the fresh mapping back while that mapping is ALIVE. An expired
    /// mapping is dead weight: its pinhole is gone, so the anti-rollback guard
    /// yields and the fresh register takes the slot back (the reboot case).
    pub fn observe(&mut self, key: NodeKey, src: SocketAddr, now: u64) {
        match self.latest.get(&key) {
            // Already superseded past the boot baseline AND still alive: a
            // nonce-0 Register is stale by construction and cannot roll it back.
            Some(prev) if prev.nonce > 0 && !self.expired(prev, now) => {}
            _ => self.insert_fresh(key, src, 0, now),
        }
    }

    /// The one accepted-advert write path (`observe` and `readvertise` both
    /// end here): reclaim space if needed, then store the advert with its
    /// life restarted at `now`.
    fn insert_fresh(&mut self, key: NodeKey, src: SocketAddr, nonce: u64, now: u64) {
        self.evict_if_full(&key, now);
        self.latest.insert(
            key,
            ReflexiveAdvert {
                reflexive: src,
                nonce,
                last_seen: now,
            },
        );
    }

    /// Before inserting a NEW key at the cap, reclaim an expired corpse if one
    /// exists, else drop the lowest-nonce entry — so an attacker spraying fresh
    /// random keys cannot grow the map without bound OR evict a live member
    /// while dead entries sit around. A no-op when the key already exists (an
    /// update, not growth) or there is room.
    fn evict_if_full(&mut self, incoming: &NodeKey, now: u64) {
        if self.latest.contains_key(incoming) || self.latest.len() < MAX_ADVERTS {
            return;
        }
        let victim = self
            .latest
            .iter()
            .find(|(_, a)| self.expired(a, now))
            .map(|(k, _)| *k)
            .or_else(|| {
                self.latest
                    .iter()
                    .min_by_key(|(_, a)| a.nonce)
                    .map(|(k, _)| *k)
            });
        if let Some(victim) = victim {
            self.latest.remove(&victim);
        }
    }

    /// Rebind re-advertisement. A strictly-higher `nonce` supersedes the stored
    /// mapping (store `src`, return `Superseded`); an equal-or-lower nonce
    /// against a LIVE mapping is stale and leaves the stored advert untouched
    /// (`Stale`) — and deliberately does not refresh `last_seen`, so a replayed
    /// datagram cannot keep a mapping alive. No prior entry, or an EXPIRED one,
    /// -> accepted as a first advert (the nonce guard protects live mappings,
    /// not corpses — a rebooted node restarts its nonce sequence).
    pub fn readvertise(
        &mut self,
        key: NodeKey,
        src: SocketAddr,
        nonce: u64,
        now: u64,
    ) -> AdvertOutcome {
        match self.latest.get(&key) {
            Some(prev) if nonce <= prev.nonce && !self.expired(prev, now) => AdvertOutcome::Stale,
            _ => {
                self.insert_fresh(key, src, nonce, now);
                AdvertOutcome::Superseded
            }
        }
    }

    /// The key's live reflexive, if its registration has not expired. An
    /// expired mapping resolves to `None` — the honest answer, since its NAT
    /// pinhole died with the silence.
    pub fn current(&self, key: NodeKey, now: u64) -> Option<SocketAddr> {
        self.latest
            .get(&key)
            .filter(|a| !self.expired(a, now))
            .map(|a| a.reflexive)
    }

    /// Reverse-map an observed source back to the key that advertised it. Used
    /// by the coordinator to bind a caller's datagram source to its identity.
    /// Expired mappings do not resolve.
    pub fn key_for_src(&self, src: SocketAddr, now: u64) -> Option<NodeKey> {
        self.latest
            .iter()
            .filter(|(_, a)| !self.expired(a, now))
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
        book.observe(key, addr(1, 4000), 0); // boot: nonce 0
        assert_eq!(book.current(key, 0), Some(addr(1, 4000)));

        // A rebind advertises the NEW reflexive under a strictly-higher nonce:
        // it supersedes the stale mapping.
        assert_eq!(
            book.readvertise(key, addr(2, 5000), 1, 0),
            AdvertOutcome::Superseded
        );
        assert_eq!(book.current(key, 0), Some(addr(2, 5000)));
        assert_eq!(book.key_for_src(addr(2, 5000), 0), Some(key));
        assert_eq!(
            book.key_for_src(addr(1, 4000), 0),
            None,
            "stale mapping is gone"
        );
    }

    #[test]
    fn equal_or_lower_nonce_is_stale_and_does_not_change_mapping() {
        let key = NodeKey([0xbb; 32]);
        let mut book = AdvertBook::default();
        book.observe(key, addr(1, 4000), 0); // nonce 0
        assert_eq!(
            book.readvertise(key, addr(2, 5000), 2, 0),
            AdvertOutcome::Superseded
        );

        // A replayed / equal-nonce advert must not clobber the fresher mapping
        // (mirrors StaleDuplicateAdvertisement: nonce <= prev).
        assert_eq!(
            book.readvertise(key, addr(9, 9999), 2, 0),
            AdvertOutcome::Stale
        );
        assert_eq!(
            book.readvertise(key, addr(9, 9999), 1, 0),
            AdvertOutcome::Stale
        );
        assert_eq!(
            book.current(key, 0),
            Some(addr(2, 5000)),
            "stale adverts leave state untouched"
        );
    }

    #[test]
    fn observe_does_not_roll_back_a_superseded_higher_nonce_mapping() {
        let key = NodeKey([0xdd; 32]);
        let mut book = AdvertBook::default();
        book.observe(key, addr(1, 4000), 0); // boot: nonce 0
        assert_eq!(
            book.readvertise(key, addr(2, 5000), 1, 0),
            AdvertOutcome::Superseded
        );

        // A replayed/reordered boot Register (observe) from the STALE source must
        // NOT roll the fresh nonce-1 mapping back to the old one.
        book.observe(key, addr(1, 4000), 0);
        assert_eq!(
            book.current(key, 0),
            Some(addr(2, 5000)),
            "a stale nonce-0 register cannot clobber a rebind re-advertisement"
        );
        assert_eq!(
            book.key_for_src(addr(1, 4000), 0),
            None,
            "stale mapping stays gone"
        );
    }

    #[test]
    fn observe_still_refreshes_while_at_the_boot_baseline() {
        // Before any rebind (still nonce 0), a re-register updates the mapping —
        // the fix only blocks rollback of an already-superseded mapping.
        let key = NodeKey([0xee; 32]);
        let mut book = AdvertBook::default();
        book.observe(key, addr(1, 4000), 0);
        book.observe(key, addr(3, 7000), 0);
        assert_eq!(
            book.current(key, 0),
            Some(addr(3, 7000)),
            "a nonce-0 re-register refreshes the baseline mapping"
        );
    }

    #[test]
    fn unknown_key_has_no_current() {
        let book = AdvertBook::default();
        assert_eq!(book.current(NodeKey([0xcc; 32]), 0), None);
    }

    #[test]
    fn the_book_is_capped_against_key_spray() {
        // an attacker registering random keys must not grow the map without
        // bound: at the cap a new key evicts the lowest-nonce (baseline) entry.
        let mut book = AdvertBook::default();
        for i in 0..(MAX_ADVERTS as u64) {
            let mut k = [0u8; 32];
            k[..8].copy_from_slice(&i.to_le_bytes());
            book.observe(NodeKey(k), addr(1, 4000), 0);
        }
        assert_eq!(book.latest.len(), MAX_ADVERTS);
        // promote one entry above the baseline so it survives eviction.
        let mut kept = [0u8; 32];
        kept[..8].copy_from_slice(&7u64.to_le_bytes());
        assert_eq!(
            book.readvertise(NodeKey(kept), addr(2, 5000), 9, 0),
            AdvertOutcome::Superseded
        );
        // one more spray key stays at the cap and does not evict the promoted one.
        book.observe(NodeKey([0xff; 32]), addr(3, 6000), 0);
        assert_eq!(book.latest.len(), MAX_ADVERTS);
        assert_eq!(book.current(NodeKey(kept), 0), Some(addr(2, 5000)));
    }

    #[test]
    fn registration_expires_after_ttl() {
        let key = NodeKey([0x01; 32]);
        let mut book = AdvertBook::with_ttl(120);
        book.observe(key, addr(1, 4000), 1_000);
        assert_eq!(book.current(key, 1_000), Some(addr(1, 4000)));
        assert_eq!(
            book.current(key, 1_120),
            Some(addr(1, 4000)),
            "alive at exactly ttl"
        );
        assert_eq!(book.current(key, 1_121), None, "expired past ttl");
        assert_eq!(
            book.key_for_src(addr(1, 4000), 1_121),
            None,
            "reverse map expires too"
        );
    }

    #[test]
    fn readvertise_refreshes_last_seen() {
        let key = NodeKey([0x02; 32]);
        let mut book = AdvertBook::with_ttl(120);
        book.observe(key, addr(1, 4000), 1_000);
        // keepalive at t=1_100 extends life to 1_220.
        assert_eq!(
            book.readvertise(key, addr(1, 4000), 1, 1_100),
            AdvertOutcome::Superseded
        );
        assert_eq!(book.current(key, 1_200), Some(addr(1, 4000)));
        assert_eq!(book.current(key, 1_221), None);
    }

    #[test]
    fn stale_nonce_does_not_extend_life() {
        // A replayed lower-nonce datagram must not keep a mapping alive: only a
        // fresh (strictly-higher-nonce) readvertise or a baseline observe counts.
        let key = NodeKey([0x03; 32]);
        let mut book = AdvertBook::with_ttl(120);
        book.observe(key, addr(1, 4000), 1_000);
        assert_eq!(
            book.readvertise(key, addr(1, 4000), 5, 1_010),
            AdvertOutcome::Superseded
        );
        assert_eq!(
            book.readvertise(key, addr(9, 9999), 5, 1_100),
            AdvertOutcome::Stale
        );
        assert_eq!(
            book.current(key, 1_131),
            None,
            "life still ends 120s after the LAST accepted advert"
        );
    }

    #[test]
    fn expired_entry_is_replaceable_regardless_of_nonce() {
        // The anti-rollback guard (nonce > 0 blocks a nonce-0 observe) only makes
        // sense for a LIVE mapping. Once expired, the entry is dead — a rebooted
        // node re-registering at the baseline must take the slot back.
        let key = NodeKey([0x04; 32]);
        let mut book = AdvertBook::with_ttl(120);
        book.observe(key, addr(1, 4000), 1_000);
        assert_eq!(
            book.readvertise(key, addr(1, 4000), 999_999, 1_010),
            AdvertOutcome::Superseded
        );
        // Within TTL the high-nonce guard still holds:
        book.observe(key, addr(2, 5000), 1_050);
        assert_eq!(book.current(key, 1_050), Some(addr(1, 4000)));
        // After expiry the fresh register wins:
        book.observe(key, addr(2, 5000), 2_000);
        assert_eq!(book.current(key, 2_000), Some(addr(2, 5000)));
        // ...and a fresh low-nonce readvertise also wins over an expired corpse:
        assert_eq!(
            book.readvertise(key, addr(3, 6000), 1, 3_000),
            AdvertOutcome::Superseded
        );
        assert_eq!(book.current(key, 3_000), Some(addr(3, 6000)));
    }

    #[test]
    fn eviction_prefers_expired_entries() {
        let mut book = AdvertBook::with_ttl(120);
        for i in 0..(MAX_ADVERTS as u64) {
            let mut k = [0u8; 32];
            k[..8].copy_from_slice(&i.to_le_bytes());
            // Promote everyone above the baseline so lowest-nonce eviction alone
            // cannot pick a deterministic victim...
            book.readvertise(NodeKey(k), addr(1, 4000), 10, 1_000);
        }
        // ...except one entry that is EXPIRED (its last accepted advert is far in
        // the past relative to the eviction moment).
        let mut dead = [0u8; 32];
        dead[..8].copy_from_slice(&3u64.to_le_bytes());
        assert_eq!(
            book.readvertise(NodeKey(dead), addr(1, 4000), 11, 500),
            AdvertOutcome::Superseded
        );
        // A fresh key at the cap must evict the EXPIRED entry, not a live one.
        book.observe(NodeKey([0xEE; 32]), addr(2, 5000), 1_000);
        assert_eq!(
            book.current(NodeKey([0xEE; 32]), 1_000),
            Some(addr(2, 5000))
        );
        assert_eq!(book.current(NodeKey(dead), 1_000), None);
        assert_eq!(book.latest.len(), MAX_ADVERTS);
    }
}
