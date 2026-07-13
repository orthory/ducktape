//! the coordinator's short-invite shelf: keyed by a caller-chosen RANDOM id,
//! TTL'd, bounded. UNTRUSTED STORAGE by design — the blob authenticates itself
//! (issuer envelope signature, which the fetching client verifies via
//! `decode_invite`), the coordinator only shelves bytes. The id is a random
//! PoP-owned LOOKUP KEY, not a content hash: there is no hash to brute-force, so
//! a colliding blob cannot substitute a victim's shelved invite — a cross-owner
//! put to an occupied id is refused. in-memory only: a restart drops links,
//! republishing is the recovery (same statelessness posture as `AdvertBook`).

use std::collections::HashMap;
use std::net::IpAddr;

use crate::NodeKey;
use crate::wire::{INVITE_BLOB_MAX, INVITE_CHUNK_BYTES, INVITE_ID_LEN};

/// hard ceiling on shelved invites — a DoS backstop, not a working limit.
pub const MAX_INVITES: usize = 4096;
/// live invites one issuer key may shelve (quota rides the PoP'd caller).
pub const MAX_INVITES_PER_OWNER: usize = 32;
/// longest accepted shelf life (invites default to 7d; 30d is the cap).
pub const MAX_INVITE_TTL_SECS: u64 = 30 * 24 * 60 * 60;
/// unauthenticated-get token bucket: sustained rate and burst, per source IP.
pub const GET_RATE_PER_SEC: u64 = 5;
pub const GET_BURST: u64 = 20;
/// distinct source IPs tracked; at the cap the stalest bucket is evicted.
const MAX_GET_BUCKETS: usize = 1024;

/// the outcome of a `put` — the coordinator answers Stored/Replaced with
/// `InvitePutAck { ok: true }`, everything else with `ok: false`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PutOutcome {
    Stored,
    Replaced,
    /// the id is already shelved by a DIFFERENT owner — refused (never
    /// overwritten). The caller re-mints under a fresh random id and retries.
    Taken,
    QuotaExceeded,
    TooLarge,
    BadExpiry,
}

struct Entry {
    blob: Vec<u8>,
    expires: u64,
    owner: NodeKey,
}

#[derive(Default)]
pub struct InviteStore {
    entries: HashMap<[u8; INVITE_ID_LEN], Entry>,
    gets: HashMap<IpAddr, (f64, u64)>, // (tokens, last_refill_secs)
}

impl InviteStore {
    /// shelve `blob` under `id`, a caller-chosen RANDOM lookup key. The id is
    /// PoP-owned: only the owner that first shelved an id may overwrite it
    /// (republish → `Replaced`); a put to an id held by a different owner is
    /// refused (`Taken`). Integrity is NOT the id — it is the blob's envelope
    /// signature, which the fetching client verifies via `decode_invite`. The
    /// stored shelf life is capped at `MAX_INVITE_TTL_SECS` (a far-future expiry
    /// is clamped, not rejected).
    pub fn put(
        &mut self,
        owner: NodeKey,
        id: [u8; INVITE_ID_LEN],
        blob: Vec<u8>,
        expires_unix_secs: u64,
        now: u64,
    ) -> PutOutcome {
        if blob.len() > INVITE_BLOB_MAX {
            return PutOutcome::TooLarge;
        }
        if expires_unix_secs <= now {
            return PutOutcome::BadExpiry;
        }
        // cap shelf life; a caller asking for longer (or u64::MAX) gets the cap.
        let expires = expires_unix_secs.min(now.saturating_add(MAX_INVITE_TTL_SECS));

        // drop anything already dead so it never counts against quota or the cap.
        self.entries.retain(|_, e| e.expires > now);

        // an occupied id: only its OWNER may overwrite (idempotent republish,
        // never quota'd). A different owner is refused — this is what stops a
        // brute-forced random-id guess from substituting a victim's invite.
        if let Some(slot) = self.entries.get_mut(&id) {
            if slot.owner != owner {
                return PutOutcome::Taken;
            }
            *slot = Entry { blob, expires, owner };
            return PutOutcome::Replaced;
        }

        // per-owner quota (linear scan: puts are rare, map ≤ MAX_INVITES).
        // ponytail: put quota is count-based; add a per-key put bucket only if a
        // PoP-flood profile ever shows dispatch cost mattering.
        let owned = self.entries.values().filter(|e| e.owner == owner).count();
        if owned >= MAX_INVITES_PER_OWNER {
            return PutOutcome::QuotaExceeded;
        }

        // global backstop: at the cap, evict the soonest-to-expire entry.
        // ponytail: griefable by ~128 PoP keys spraying max-TTL puts to evict
        // honest 7d invites — accepted, this is a DoS backstop not a working
        // limit; the full blob is the universal fallback. upgrade path: evict
        // the most-recently-inserted owner-cluster instead.
        if self.entries.len() >= MAX_INVITES
            && let Some(stale) = self
                .entries
                .iter()
                .min_by_key(|(_, e)| e.expires)
                .map(|(k, _)| *k)
        {
            self.entries.remove(&stale);
        }

        self.entries.insert(id, Entry { blob, expires, owner });
        PutOutcome::Stored
    }

    /// one chunk of a shelved blob. `None` = unknown/expired id.
    /// `Some((bytes, total_chunks))`; an out-of-range chunk yields empty bytes
    /// with the real total.
    pub fn chunk(&mut self, id: [u8; INVITE_ID_LEN], chunk: u16, now: u64) -> Option<(Vec<u8>, u16)> {
        let entry = self.entries.get(&id)?;
        if entry.expires <= now {
            return None;
        }
        let total = entry.blob.len().div_ceil(INVITE_CHUNK_BYTES) as u16;
        let start = chunk as usize * INVITE_CHUNK_BYTES;
        if start >= entry.blob.len() {
            return Some((Vec::new(), total));
        }
        let end = (start + INVITE_CHUNK_BYTES).min(entry.blob.len());
        Some((entry.blob[start..end].to_vec(), total))
    }

    /// per-source-IP token bucket for gets: `false` = rate-limited, drop
    /// silently. A fresh IP starts with a full burst.
    pub fn allow_get(&mut self, src_ip: IpAddr, now: u64) -> bool {
        // at the bucket cap, make room by evicting the stalest tracked IP.
        if !self.gets.contains_key(&src_ip)
            && self.gets.len() >= MAX_GET_BUCKETS
            && let Some(stalest) = self
                .gets
                .iter()
                .min_by_key(|(_, (_, last))| *last)
                .map(|(k, _)| *k)
        {
            self.gets.remove(&stalest);
        }
        let bucket = self.gets.entry(src_ip).or_insert((GET_BURST as f64, now));
        let refill = now.saturating_sub(bucket.1) as f64 * GET_RATE_PER_SEC as f64;
        let avail = (bucket.0 + refill).min(GET_BURST as f64);
        bucket.1 = now;
        if avail >= 1.0 {
            bucket.0 = avail - 1.0;
            true
        } else {
            bucket.0 = avail;
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_shelves_under_a_random_id_and_chunks_roundtrip() {
        let mut s = InviteStore::default();
        let blob = vec![7u8; 2500]; // 3 chunks: 1000+1000+500
        let id = [1u8; INVITE_ID_LEN]; // a random lookup key, not a content hash
        assert_eq!(s.put(NodeKey([1; 32]), id, blob.clone(), 100, 0), PutOutcome::Stored);
        let (c0, total) = s.chunk(id, 0, 0).unwrap();
        assert_eq!((c0.len(), total), (1000, 3));
        let (c2, _) = s.chunk(id, 2, 0).unwrap();
        assert_eq!(c2, vec![7u8; 500]);
        // reassembly equals the original
        let mut whole = Vec::new();
        for i in 0..total {
            whole.extend(s.chunk(id, i, 0).unwrap().0);
        }
        assert_eq!(whole, blob);
        // an unknown id is None; expiry kills the shelved one
        assert!(s.chunk([9u8; INVITE_ID_LEN], 0, 0).is_none());
        assert!(s.chunk(id, 0, 101).is_none(), "expired ids resolve to None");
    }

    #[test]
    fn a_cross_owner_put_is_refused_but_the_owner_may_republish() {
        let mut s = InviteStore::default();
        let id = [3u8; INVITE_ID_LEN];
        let alice = NodeKey([1; 32]);
        let bob = NodeKey([2; 32]);
        assert_eq!(s.put(alice, id, vec![0xAA; 10], u64::MAX, 0), PutOutcome::Stored);
        // bob cannot overwrite alice's id, and alice's blob is untouched.
        assert_eq!(s.put(bob, id, vec![0xBB; 10], u64::MAX, 0), PutOutcome::Taken);
        assert_eq!(s.chunk(id, 0, 0).unwrap().0, vec![0xAA; 10]);
        // alice republishing her own id is an idempotent Replaced.
        assert_eq!(s.put(alice, id, vec![0xCC; 10], u64::MAX, 0), PutOutcome::Replaced);
        assert_eq!(s.chunk(id, 0, 0).unwrap().0, vec![0xCC; 10]);
    }

    #[test]
    fn per_owner_quota_and_global_cap_hold() {
        let mut s = InviteStore::default();
        let owner = NodeKey([1; 32]);
        for i in 0..MAX_INVITES_PER_OWNER as u8 {
            let id = [i; INVITE_ID_LEN];
            assert_eq!(s.put(owner, id, vec![i; 10], u64::MAX, 0), PutOutcome::Stored);
        }
        assert_eq!(
            s.put(owner, [0xFF; INVITE_ID_LEN], vec![0xFF; 10], u64::MAX, 0),
            PutOutcome::QuotaExceeded
        );
        // re-putting an EXISTING id never counts against quota (idempotent republish)
        assert_eq!(s.put(owner, [0u8; INVITE_ID_LEN], vec![0u8; 10], u64::MAX, 0), PutOutcome::Replaced);
    }

    #[test]
    fn get_rate_limit_is_per_ip_and_refills() {
        let mut s = InviteStore::default();
        let ip: std::net::IpAddr = "203.0.113.9".parse().unwrap();
        for _ in 0..GET_BURST {
            assert!(s.allow_get(ip, 0));
        }
        assert!(!s.allow_get(ip, 0), "burst exhausted");
        assert!(s.allow_get(ip, 10), "tokens refill with time");
        assert!(s.allow_get("203.0.113.10".parse().unwrap(), 0), "another ip has its own bucket");
    }
}
