use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::{Latch, NodeKey};

/// One node's latest reflexive advertisement: the reflexive `SocketAddr` a node
/// published and the monotonic `nonce` that orders it. The nonce is an ordering
/// token only — the address is always the coordinator-observed source, never a
/// self-reported one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReflexiveAdvert {
    pub reflexive: SocketAddr,
    /// Freshness for THIS key and nothing else: a strictly-higher nonce
    /// supersedes this key's own stored mapping. It is sender-chosen, so it
    /// never ranks one key against another — comparing two keys' nonces would
    /// hand a stranger the ordering.
    pub nonce: u64,
    /// Wall-clock seconds of the last ACCEPTED advert (an `observe` or a
    /// superseding `readvertise`). A stale-nonce replay never refreshes this —
    /// only fresh proof of life extends a mapping.
    pub last_seen: u64,
    /// Admission order, stamped by the COORDINATOR when this key first took a
    /// slot and carried through every later refresh. Nothing on the wire can
    /// influence it, which is exactly why eviction reads it and not the nonce.
    pub admitted: u64,
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
    /// A write that would add a live entry to a source IP already at
    /// [`MAX_ADVERTS_PER_SOURCE_IP`] — a first-seen key, an expired key's
    /// refresh, or a readvertise that migrates a key to a new source. Admission,
    /// not eviction: no existing entry — from this source or any other — was
    /// touched.
    Refused,
    /// `observe` found a mapping already ahead of the baseline (a superseding
    /// nonce, or a live mapping from a different source) and left it
    /// untouched. Not a refusal — nothing was refused, there was simply
    /// nothing to do.
    NoOp,
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
/// resident set grows without limit. At the cap, a first-seen key reclaims an
/// expired corpse if there is one, else evicts the YOUNGEST admission — the
/// newest arrival, which under a spray is another of the sprayer's own keys.
/// Seniority is the coordinator's own stamp, so no wire value decides who
/// stays. Generous for any real validator mesh; a DoS backstop, not a working
/// limit.
///
/// The honest consequence of youngest-eviction: past the cap the book is
/// strict first-come-first-served. While keepalives run nothing expires, so a
/// mesh larger than the cap never rotates and member 4097 holds no slot at
/// all. That is the deliberate trade — a last-seen LRU would rotate, but it
/// hands the choice back to whoever refreshes fastest, which is the attacker
/// (an honest keepalive is 25 s apart; a sprayer picks its own rate).
const MAX_ADVERTS: usize = 4096;

/// Per-source-IP cap on distinct keys the coordinator admits from ONE
/// observed IP. Mirrors `relay.rs`'s `MAX_SESSIONS_PER_IP`: this bounds
/// admission, not eviction, so it does nothing against a botnet spread
/// across many addresses — it only stops one host from claiming an
/// unbounded share of [`MAX_ADVERTS`] by spraying keys from a single
/// source. A handful is generous for any real host (a home gateway, a small
/// rack) running several nodes behind one address.
const MAX_ADVERTS_PER_SOURCE_IP: usize = 8;

pub struct AdvertBook {
    latest: HashMap<NodeKey, ReflexiveAdvert>,
    ttl: u64,
    /// Sequence stamped on the next first-seen key. Monotonic and
    /// coordinator-owned: it is the eviction order, so it must never be
    /// derivable from anything a sender sends.
    next_admission: u64,
}

impl Default for AdvertBook {
    fn default() -> Self {
        Self::with_ttl(REGISTRATION_TTL_SECS)
    }
}

impl AdvertBook {
    /// A book with an explicit TTL (seconds). Tests and short-lived rigs
    /// shrink it; production uses [`REGISTRATION_TTL_SECS`] via `Default`.
    pub fn with_ttl(ttl: u64) -> Self {
        Self {
            latest: HashMap::new(),
            ttl,
            next_admission: 0,
        }
    }

    fn expired(&self, advert: &ReflexiveAdvert, now: u64) -> bool {
        now.saturating_sub(advert.last_seen) > self.ttl
    }

    /// Boot/live registration at the nonce-0 baseline. The coordinator-observed
    /// `src` is authoritative. This establishes the baseline for a first-seen key
    /// and refreshes it while still at the baseline, but it is NOT unconditional:
    /// a nonce-0 `Register` must never REPOINT a still-ALIVE mapping, because the
    /// authenticator is not bound to the datagram source, so a captured `Register`
    /// replayed from a DIFFERENT source within the freshness window would
    /// otherwise hijack the owner's mapping to the attacker's observed address.
    /// Two cases keep a live mapping fixed. When `nonce > 0`, a rebind
    /// re-advertisement already superseded the baseline, so a later (necessarily
    /// nonce-0) `Register` is stale by construction. When `nonce == 0` but the
    /// source DIFFERS, a live baseline mapping is not repointed by a bare
    /// `Register` from elsewhere: a genuine NAT rebind re-advertises under a
    /// strictly-higher nonce (`readvertise`, the keepalive path), never a bare
    /// nonce-0 `Register`, so no legitimate node needs this — while a SAME-source
    /// `Register` still refreshes liveness. An EXPIRED mapping is dead weight (its
    /// pinhole is gone), so both guards yield and the fresh register takes the
    /// slot back — the reboot case.
    pub fn observe(&mut self, key: NodeKey, src: SocketAddr, now: u64) -> AdvertOutcome {
        match self.latest.get(&key) {
            Some(prev) if !self.expired(prev, now) && (prev.nonce > 0 || prev.reflexive != src) => {
                AdvertOutcome::NoOp
            }
            _ => {
                if self.insert_fresh(key, src, 0, now) {
                    AdvertOutcome::Superseded
                } else {
                    AdvertOutcome::Refused
                }
            }
        }
    }

    /// The one accepted-advert write path (`observe` and `readvertise` both
    /// end here): keep the key's existing seniority, or take a fresh admission
    /// slot for a first-seen key, then store the advert with its life restarted
    /// at `now`. Returns `false`, leaving state untouched, when the write
    /// would add a LIVE entry to `src.ip()` — a first-seen key, an expired
    /// key's refresh, or a migration to a new source — and that source is
    /// already at [`MAX_ADVERTS_PER_SOURCE_IP`]. A refresh from the SAME live
    /// source never re-runs the cap check: it isn't adding a live entry
    /// anywhere, just extending the one already counted.
    fn insert_fresh(&mut self, key: NodeKey, src: SocketAddr, nonce: u64, now: u64) -> bool {
        let prev = self.latest.get(&key).copied();
        // Seniority belongs to the KEY and survives every refresh: a member
        // that re-advertises keeps the slot it earned instead of becoming the
        // newest arrival (and so the next victim).
        let admitted = match prev {
            Some(prev) if !self.expired(&prev, now) && prev.reflexive.ip() == src.ip() => {
                prev.admitted
            }
            Some(prev) => {
                if !self.cap_check(src.ip(), now) {
                    return false;
                }
                prev.admitted
            }
            None => match self.admit(src, now) {
                Some(admitted) => admitted,
                None => return false,
            },
        };
        self.latest.insert(
            key,
            ReflexiveAdvert {
                reflexive: src,
                nonce,
                last_seen: now,
                admitted,
            },
        );
        true
    }

    /// Refuse a write to `ip` outright if it already holds
    /// [`MAX_ADVERTS_PER_SOURCE_IP`] live entries — never evicting one of
    /// them, so an at-cap source can never bump another source out. Shared by
    /// a first-seen key's admission and by a refresh that is about to add a
    /// live entry under a DIFFERENT ip than the one it's replacing (an
    /// expired-key reclaim or a migrated key) — both would otherwise grow
    /// that source's live share past the cap the same way a brand-new key
    /// would.
    fn cap_check(&mut self, ip: IpAddr, now: u64) -> bool {
        if self.live_adverts_from(ip, now) < MAX_ADVERTS_PER_SOURCE_IP {
            return true;
        }
        // peer-driven and unauthenticated by definition (any key can spray
        // from one address), so this is latched like every other refusal in
        // this crate — the count is the diagnosis.
        static SOURCE_CAP: Latch = Latch::new();
        if let Some(occurrences) = SOURCE_CAP.hit("advert_source_cap") {
            tracing::warn!(
                target: "ducktape::reachability",
                event = "advert_refused",
                reason = "advert_source_cap",
                source = %ip,
                cap = MAX_ADVERTS_PER_SOURCE_IP,
                occurrences,
                "source IP at its per-source advert cap — new key refused"
            );
        }
        false
    }

    /// Take an admission slot for a FIRST-SEEN key: refuse it via
    /// [`Self::cap_check`] if its source IP is already at
    /// [`MAX_ADVERTS_PER_SOURCE_IP`], otherwise reclaim space if the book
    /// itself is at [`MAX_ADVERTS`], then stamp the next admission sequence.
    fn admit(&mut self, src: SocketAddr, now: u64) -> Option<u64> {
        if !self.cap_check(src.ip(), now) {
            return None;
        }
        self.evict_if_full(now);
        let admitted = self.next_admission;
        self.next_admission += 1;
        Some(admitted)
    }

    /// Count of `ip`'s currently-live entries — what a fresh
    /// [`MAX_ADVERTS_PER_SOURCE_IP`] check weighs. An expired entry no longer
    /// counts: it is dead weight, exactly as `evict_if_full` treats it for
    /// the whole book, so a source's slots free up as its members' keepalives
    /// lapse rather than staying pinned until something else reclaims them.
    fn live_adverts_from(&self, ip: IpAddr, now: u64) -> usize {
        self.latest
            .values()
            .filter(|a| a.reflexive.ip() == ip && !self.expired(a, now))
            .count()
    }

    /// Reclaim one slot for a first-seen key at the cap: an expired corpse if
    /// there is one, else the YOUNGEST admission.
    ///
    /// Eviction reads only what the coordinator itself stamped. It used to drop
    /// the lowest `nonce`, which is a number the sender picks: a stranger
    /// registering 4096 keys at `u64::MAX` evicted every real member (an honest
    /// node seeds its nonce from wall-clock, and a just-booted one sits at 0),
    /// and endpoint-less peers — the default for an invite-joined node — then
    /// resolved to `None` for as long as the spray lasted. Evicting the newest
    /// arrival instead makes an established registration unevictable by a
    /// newcomer: a sprayer's next key can only displace the sprayer's own last
    /// one. It does NOT make a full book admit strangers forever — capacity is
    /// finite by design — but capacity now goes to whoever was here first.
    ///
    /// The case it does not cover: a key registering for the FIRST time during
    /// a sustained spray (a fresh join, or a member whose corpse was already
    /// reclaimed after a >TTL outage) takes the revolving newest slot and the
    /// next spray packet displaces it, so that join retries until the flood
    /// stops. Everyone already in the book rides it out.
    fn evict_if_full(&mut self, now: u64) {
        if self.latest.len() < MAX_ADVERTS {
            return;
        }
        let corpse = self
            .latest
            .iter()
            .find(|(_, a)| self.expired(a, now))
            .map(|(k, _)| *k);
        if let Some(corpse) = corpse {
            self.latest.remove(&corpse);
            return;
        }
        let youngest = self
            .latest
            .iter()
            .max_by_key(|(_, a)| a.admitted)
            .map(|(k, _)| *k);
        let Some(youngest) = youngest else {
            return;
        };
        self.latest.remove(&youngest);
        // a full book of LIVE registrations is either a mesh past the cap or a
        // spray in progress; either way the operator wants to know that a
        // registration is being displaced, and the count says which it is.
        // the evicted KEY is not named: under the spray this line exists to
        // report, the victim is always the sprayer's own previous throwaway,
        // so the number of displacements is the whole diagnosis.
        static BOOK_FULL: Latch = Latch::new();
        if let Some(occurrences) = BOOK_FULL.hit("book_full") {
            tracing::warn!(
                target: "ducktape::reachability",
                event = "advert_evicted",
                reason = "book_full",
                capacity = MAX_ADVERTS,
                occurrences,
                "advert book at capacity — the newest registration lost its slot"
            );
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
        let stale = matches!(
            self.latest.get(&key),
            Some(prev) if nonce <= prev.nonce && !self.expired(prev, now)
        );
        if stale {
            return AdvertOutcome::Stale;
        }
        if self.insert_fresh(key, src, nonce, now) {
            AdvertOutcome::Superseded
        } else {
            AdvertOutcome::Refused
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
}

/// One [`AdvertBook`] shared between the UDP rendezvous state machine (the
/// [`crate::Coordinator`], which owns every write) and the TCP relay lane
/// (`crate::relay`, which only resolves targets). The relay MUST read the same
/// book the rendezvous maintains: a member's reflexive is wherever its live
/// keepalives say it is, and a second book would drift.
///
/// A `std::sync::Mutex`, not tokio's: every lock scope is a single book
/// operation and is NEVER held across an await. Both UDP serving loops are
/// single-threaded, so contention is limited to the (rare) relay resolution.
#[derive(Clone)]
pub struct SharedAdverts(Arc<Mutex<AdvertBook>>);

impl SharedAdverts {
    #[cfg(feature = "runtime")]
    pub(crate) fn wrap(book: AdvertBook) -> Self {
        Self(Arc::new(Mutex::new(book)))
    }

    /// The key's live reflexive (`None` once expired) —
    /// [`AdvertBook::current`] behind the shared lock.
    pub fn current(&self, key: NodeKey, now: u64) -> Option<SocketAddr> {
        self.lock().current(key, now)
    }

    /// A poisoned lock only means another holder panicked mid-operation; the
    /// book is a plain map whose worst partial state is one stale entry, so
    /// keep serving joins rather than wedging every future lock on it.
    pub(crate) fn lock(&self) -> MutexGuard<'_, AdvertBook> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
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
    }

    #[test]
    fn observe_refreshes_the_baseline_from_the_same_source_but_never_repoints_it() {
        // A SAME-source nonce-0 re-register refreshes liveness (a legitimate node
        // re-registering from its own pinhole while still at the baseline)...
        let key = NodeKey([0xee; 32]);
        let mut book = AdvertBook::with_ttl(120);
        book.observe(key, addr(1, 4000), 1_000);
        book.observe(key, addr(1, 4000), 1_050);
        assert_eq!(
            book.current(key, 1_160),
            Some(addr(1, 4000)),
            "life extended to 1_170"
        );
        assert_eq!(
            book.current(key, 1_171),
            None,
            "the same-source refresh moved last_seen"
        );

        // ...but a DIFFERENT-source nonce-0 register does NOT repoint a live
        // baseline mapping. A genuine NAT rebind re-advertises under a higher
        // nonce (readvertise); only a replayed/spoofed bare Register lands here.
        let key = NodeKey([0xef; 32]);
        let mut book = AdvertBook::default();
        book.observe(key, addr(1, 4000), 0);
        book.observe(key, addr(3, 7000), 0);
        assert_eq!(
            book.current(key, 0),
            Some(addr(1, 4000)),
            "a different-source nonce-0 register cannot repoint a live baseline mapping"
        );
    }

    #[test]
    fn replayed_register_from_another_source_cannot_hijack_a_live_mapping() {
        // The H3 register-hijack: an on-path attacker captures a victim's valid
        // Register and replays the identical (still-freshly-PoP'd) datagram from
        // its OWN socket. At the coordinator that lands as observe(victim, attacker_src).
        // The victim is at the nonce-0 baseline (registered, not yet keepalived),
        // and its mapping is live — so the attacker's source must NOT take over.
        let victim = NodeKey([0x77; 32]);
        let victim_src = addr(1, 4000);
        let attacker_src = addr(9, 6666);
        let mut book = AdvertBook::with_ttl(120);
        book.observe(victim, victim_src, 1_000); // victim boots, registers
        book.observe(victim, attacker_src, 1_010); // attacker replays from its own src
        assert_eq!(
            book.current(victim, 1_010),
            Some(victim_src),
            "the replayed register cannot hijack the victim's reflexive mapping"
        );
        // The victim's own keepalive readvertise (strictly-higher nonce) still
        // works normally afterward — a genuine rebind is unaffected.
        assert_eq!(
            book.readvertise(victim, addr(1, 5000), 1_011, 1_020),
            AdvertOutcome::Superseded
        );
        assert_eq!(book.current(victim, 1_020), Some(addr(1, 5000)));
    }

    #[test]
    fn unknown_key_has_no_current() {
        let book = AdvertBook::default();
        assert_eq!(book.current(NodeKey([0xcc; 32]), 0), None);
    }

    /// Spread `i` across enough distinct source IPs that a `MAX_ADVERTS`-size
    /// fill never trips [`MAX_ADVERTS_PER_SOURCE_IP`] — `MAX_ADVERTS_PER_SOURCE_IP`
    /// keys share each synthetic `10.0.x.y` address before moving to the next
    /// one. A fixed address alone caps out at 8, so scattering is what it
    /// takes to keep exercising the GLOBAL cap's seniority-ordered eviction.
    fn scattered_addr(i: u64) -> SocketAddr {
        let group = i / MAX_ADVERTS_PER_SOURCE_IP as u64;
        SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(10, 0, (group >> 8) as u8, group as u8)),
            4000 + (i % MAX_ADVERTS_PER_SOURCE_IP as u64) as u16,
        )
    }

    /// Fill the book with `count` distinct keys registered at `now`.
    fn fill(book: &mut AdvertBook, count: u64, now: u64) {
        for i in 0..count {
            let mut k = [0u8; 32];
            k[..8].copy_from_slice(&i.to_le_bytes());
            book.observe(NodeKey(k), scattered_addr(i), now);
        }
    }

    /// A spray key: distinct from every `fill` key (which occupy the low
    /// bytes) and from the honest keys the tests name.
    fn spray_key(i: u64) -> NodeKey {
        let mut k = [0xA5u8; 32];
        k[..8].copy_from_slice(&i.to_le_bytes());
        NodeKey(k)
    }

    #[test]
    fn the_book_is_capped_against_key_spray() {
        // an attacker registering random keys must not grow the map without
        // bound: at the cap a new key takes an existing slot, never a new one.
        let mut book = AdvertBook::default();
        fill(&mut book, MAX_ADVERTS as u64, 0);
        assert_eq!(book.latest.len(), MAX_ADVERTS);
        let mut kept = [0u8; 32];
        kept[..8].copy_from_slice(&7u64.to_le_bytes());
        assert_eq!(
            book.readvertise(NodeKey(kept), addr(2, 5000), 9, 0),
            AdvertOutcome::Superseded
        );
        // one more spray key stays at the cap and does not evict the established one.
        book.observe(NodeKey([0xff; 32]), addr(3, 6000), 0);
        assert_eq!(book.latest.len(), MAX_ADVERTS);
        assert_eq!(book.current(NodeKey(kept), 0), Some(addr(2, 5000)));
    }

    #[test]
    fn a_spraying_stranger_cannot_evict_an_established_entry() {
        // The shipped public policy admits any self-signed key, so the whole
        // defense is WHICH entry loses its slot. A stranger picks its own
        // nonces (u64::MAX here) while an honest node seeds from wall-clock and
        // a just-booted one sits at the nonce-0 baseline — so eviction must not
        // read the nonce at all.
        let mut book = AdvertBook::with_ttl(120);
        let member = NodeKey([0x42; 32]);
        book.observe(member, addr(1, 4000), 1_000);
        fill(&mut book, MAX_ADVERTS as u64 - 1, 1_000);
        assert_eq!(book.latest.len(), MAX_ADVERTS);

        // A sustained spray: every registration fresh, self-signed, and at the
        // top of the nonce space — the shape that used to clear the book.
        let sprayed = MAX_ADVERTS as u64 / 4;
        for i in 0..sprayed {
            let key = spray_key(i);
            book.observe(key, addr(9, 6666), 1_010);
            book.readvertise(key, addr(9, 6666), u64::MAX, 1_010);
        }
        assert_eq!(book.latest.len(), MAX_ADVERTS, "still bounded");
        assert_eq!(
            book.current(member, 1_010),
            Some(addr(1, 4000)),
            "an established registration outranks every newcomer, whatever nonce it claims"
        );
        // ...and the sprayer holds ONE slot: each new key displaces its own last.
        let held = (0..sprayed)
            .filter(|i| book.current(spray_key(*i), 1_010).is_some())
            .count();
        assert_eq!(held, 1, "a sprayer can only ever displace itself");
    }

    #[test]
    fn a_re_advert_keeps_its_slot_instead_of_becoming_the_newest() {
        // Seniority is per key and survives a refresh: a member that keeps
        // re-advertising (the 25 s keepalive) must not walk itself to the front
        // of the eviction queue.
        let mut book = AdvertBook::with_ttl(120);
        let member = NodeKey([0x42; 32]);
        book.observe(member, addr(1, 4000), 1_000);
        fill(&mut book, MAX_ADVERTS as u64 - 1, 1_000);

        // the member keepalives, then a stranger registers.
        assert_eq!(
            book.readvertise(member, addr(1, 4000), 1_025, 1_025),
            AdvertOutcome::Superseded
        );
        book.observe(spray_key(0), addr(9, 6666), 1_030);
        assert_eq!(
            book.current(member, 1_030),
            Some(addr(1, 4000)),
            "the keepalive refreshed the mapping without forfeiting seniority"
        );
        assert_eq!(book.latest.len(), MAX_ADVERTS);
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
            book.readvertise(NodeKey(k), scattered_addr(i), 10, 1_000);
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

    /// One of `MAX_ADVERTS_PER_SOURCE_IP` distinct keys, all sharing ONE
    /// source IP (`addr(200, ..)`) with a distinct port per key — the shape
    /// of one host legitimately (or a sprayer illegitimately) running
    /// several nodes behind the same address.
    fn source_key(i: u64) -> NodeKey {
        let mut k = [0x5Au8; 32];
        k[..8].copy_from_slice(&i.to_le_bytes());
        NodeKey(k)
    }

    #[test]
    fn a_ninth_key_from_one_source_is_refused_while_another_source_stays() {
        let mut book = AdvertBook::with_ttl(120);
        let other = NodeKey([0x11; 32]);
        book.observe(other, addr(9, 4000), 1_000);

        for i in 0..MAX_ADVERTS_PER_SOURCE_IP as u64 {
            book.observe(source_key(i), addr(200, 4000 + i as u16), 1_000);
        }
        assert_eq!(book.current(source_key(0), 1_000), Some(addr(200, 4000)));

        // A NINTH key from the SAME source IP is refused outright: not
        // admitted, and none of the eight already there is evicted to make
        // room for it.
        let ninth = source_key(MAX_ADVERTS_PER_SOURCE_IP as u64);
        book.observe(ninth, addr(200, 9000), 1_000);
        assert_eq!(
            book.current(ninth, 1_000),
            None,
            "the 9th key from an at-cap source is refused"
        );
        for i in 0..MAX_ADVERTS_PER_SOURCE_IP as u64 {
            assert_eq!(
                book.current(source_key(i), 1_000),
                Some(addr(200, 4000 + i as u16)),
                "an at-cap source's own established keys are untouched"
            );
        }

        // An established member from a DIFFERENT source IP is untouched too
        // — the cap is per source, never global eviction pressure.
        assert_eq!(
            book.current(other, 1_000),
            Some(addr(9, 4000)),
            "an established member from a different source stays"
        );
    }

    #[test]
    fn a_refresh_from_a_capped_source_still_succeeds() {
        let mut book = AdvertBook::with_ttl(120);
        for i in 0..MAX_ADVERTS_PER_SOURCE_IP as u64 {
            book.observe(source_key(i), addr(200, 4000 + i as u16), 1_000);
        }

        // The source IP is now at its cap; an EXISTING key from it still
        // readvertises (a refresh is never an admission, so the cap never
        // applies to it).
        let member = source_key(3);
        assert_eq!(
            book.readvertise(member, addr(200, 4003), 1, 1_050),
            AdvertOutcome::Superseded,
            "a refresh from an already-held key is never refused by the source cap"
        );
        assert_eq!(book.current(member, 1_050), Some(addr(200, 4003)));

        // ...and a plain re-observe (same source, still live) refreshes too.
        book.observe(member, addr(200, 4003), 1_060);
        assert_eq!(book.current(member, 1_060), Some(addr(200, 4003)));
    }

    #[test]
    fn expiry_frees_a_capped_sources_slot() {
        let mut book = AdvertBook::with_ttl(120);
        for i in 0..MAX_ADVERTS_PER_SOURCE_IP as u64 {
            book.observe(source_key(i), addr(200, 4000 + i as u16), 1_000);
        }
        let ninth = source_key(MAX_ADVERTS_PER_SOURCE_IP as u64);

        // Refused while all eight are still alive.
        book.observe(ninth, addr(200, 9000), 1_000);
        assert_eq!(book.current(ninth, 1_000), None);

        // Past the TTL (120s, per `with_ttl` above) every one of the eight
        // is expired — dead weight, not counted against the cap — so the
        // ninth key is now admitted.
        let past_ttl = 1_000 + 120 + 1;
        book.observe(ninth, addr(200, 9000), past_ttl);
        assert_eq!(
            book.current(ninth, past_ttl),
            Some(addr(200, 9000)),
            "expiry of the source's held slots frees room for a new key"
        );
    }

    #[test]
    fn an_expired_keys_refresh_still_obeys_the_source_cap() {
        // A key already IN the book (as a corpse) is not exempt from the cap
        // just because `latest` already has an entry for it: reviving it at
        // an at-cap source must be refused exactly like a first-seen key.
        let mut book = AdvertBook::with_ttl(120);
        for i in 0..MAX_ADVERTS_PER_SOURCE_IP as u64 {
            book.observe(source_key(i), addr(200, 4000 + i as u16), 0);
        }
        let stale_key = NodeKey([0x99; 32]);
        book.observe(stale_key, addr(9, 4000), 0);
        // Keep the capped source's eight entries alive while `stale_key`'s
        // own mapping (last touched at t=0) ages past its ttl.
        for i in 0..MAX_ADVERTS_PER_SOURCE_IP as u64 {
            book.readvertise(source_key(i), addr(200, 4000 + i as u16), 1, 50);
        }
        assert_eq!(book.current(stale_key, 170), None, "stale_key has expired");
        book.observe(stale_key, addr(200, 9999), 170);
        assert_eq!(
            book.current(stale_key, 170),
            None,
            "reviving an expired key at an at-cap source obeys the cap"
        );
    }

    #[test]
    fn a_readvertise_migrating_to_an_at_cap_source_is_refused() {
        // A key LIVE at one source that re-advertises to a DIFFERENT source
        // must still be checked against the destination's cap — moving is
        // just as much an admission at the new IP as a first-seen key.
        let mut book = AdvertBook::with_ttl(120);
        for i in 0..MAX_ADVERTS_PER_SOURCE_IP as u64 {
            book.observe(source_key(i), addr(200, 4000 + i as u16), 1_000);
        }
        let member = NodeKey([0x42; 32]);
        book.observe(member, addr(1, 4000), 1_000);
        assert_eq!(
            book.readvertise(member, addr(200, 9999), 1, 1_010),
            AdvertOutcome::Refused,
            "migrating to an at-cap source is refused"
        );
        assert_eq!(
            book.current(member, 1_010),
            Some(addr(1, 4000)),
            "a refused migration leaves the old live mapping untouched"
        );
    }
}
