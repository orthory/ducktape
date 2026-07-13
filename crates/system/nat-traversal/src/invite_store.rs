//! the coordinator's short-invite shelf: content-addressed, TTL'd, bounded.
//! UNTRUSTED STORAGE by design — the blob authenticates itself (issuer
//! envelope signature; the id is its content hash), the coordinator only
//! shelves bytes. in-memory only: a restart drops links, republishing is the
//! recovery (same statelessness posture as `AdvertBook`).

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

/// the id IS the first 16 bytes of sha256(blob) — content addressing makes
/// the shelf tamper-evident without trusting the coordinator.
pub fn invite_id(blob: &[u8]) -> [u8; INVITE_ID_LEN] {
    use commonware_cryptography::{Hasher as _, Sha256};
    let mut h = Sha256::default();
    h.update(blob);
    let digest = h.finalize();
    let mut id = [0u8; INVITE_ID_LEN];
    id.copy_from_slice(&digest.as_ref()[..INVITE_ID_LEN]);
    id
}

/// the outcome of a `put` — the coordinator answers Stored/Replaced with
/// `InvitePutAck { ok: true }`, everything else with `ok: false`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PutOutcome {
    Stored,
    Replaced,
    QuotaExceeded,
    TooLarge,
    BadId,
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
    /// shelve `blob` under its content `id`. Self-authenticating: the id must
    /// be the blob's content hash. The stored shelf life is capped at
    /// `MAX_INVITE_TTL_SECS` (a far-future expiry is clamped, not rejected).
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
        if invite_id(&blob) != id {
            return PutOutcome::BadId;
        }
        if expires_unix_secs <= now {
            return PutOutcome::BadExpiry;
        }
        // cap shelf life; a caller asking for longer (or u64::MAX) gets the cap.
        let expires = expires_unix_secs.min(now.saturating_add(MAX_INVITE_TTL_SECS));

        // drop anything already dead so it never counts against quota or the cap.
        self.entries.retain(|_, e| e.expires > now);

        // re-putting an existing id is idempotent republish — never quota'd.
        if let Some(slot) = self.entries.get_mut(&id) {
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
    fn put_verifies_the_content_hash_and_chunks_roundtrip() {
        let mut s = InviteStore::default();
        let blob = vec![7u8; 2500]; // 3 chunks: 1000+1000+500
        let id = invite_id(&blob);
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
        // wrong id refused; unknown id is None; expiry kills it
        assert_eq!(s.put(NodeKey([1; 32]), [9; 16], vec![1, 2, 3], 100, 0), PutOutcome::BadId);
        assert!(s.chunk([9; 16], 0, 0).is_none());
        assert!(s.chunk(id, 0, 101).is_none(), "expired ids resolve to None");
    }

    #[test]
    fn per_owner_quota_and_global_cap_hold() {
        let mut s = InviteStore::default();
        let owner = NodeKey([1; 32]);
        for i in 0..MAX_INVITES_PER_OWNER as u8 {
            let blob = vec![i; 10];
            assert_eq!(s.put(owner, invite_id(&blob), blob, u64::MAX, 0), PutOutcome::Stored);
        }
        let over = vec![0xFF; 10];
        assert_eq!(s.put(owner, invite_id(&over), over, u64::MAX, 0), PutOutcome::QuotaExceeded);
        // re-putting an EXISTING id never counts against quota (idempotent republish)
        let again = vec![0u8; 10];
        assert_eq!(s.put(owner, invite_id(&again), again, u64::MAX, 0), PutOutcome::Replaced);
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
