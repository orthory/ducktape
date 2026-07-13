# Coordinator-Managed Short Invites Implementation Plan (PR2)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `🦆://<chain-name>/<id>` short invites: the inviter's node publishes the full signed blob to the coordinator by content hash; a joiner fetches it back, verifies it, and joins through the unchanged path.

**Architecture:** A bounded in-memory `InviteStore` on the coordinator (mirroring `AdvertBook`), four new `Msg` variants on the existing tag-dispatch wire, two `CoordClient` one-shot methods, short-URL codec + `--short` on the node CLI, and a short-link-first invite UI. The blob stays self-authenticating (issuer envelope signature + content-hash id), so the coordinator remains untrusted storage; a coordinator restart drops links by design (re-reveal republishes).

**Tech Stack:** Rust (`crates/system/nat-traversal`, `bin/node`, `bin/coordinator` untouched — policy plumbing already generic), Tauri commands, React console.

**Spec:** `docs/superpowers/specs/2026-07-13-coordinator-invites-thin-client-design.md` (Design 2).

## Global Constraints

- Branch from `origin/dev`, worktree `<primary-checkout>/.worktree/coordinator-short-invites`, PR against `dev`. Land after PR1 (both touch the invite plane; rebase over it).
- Gates per touched crate: `ops/build-with.sh cargo clippy -p nat-traversal --tests --no-deps`, same for `ducktape-node`; no `cargo fmt --all`.
- NO backward compatibility (user mandate): no version negotiation. New tags are new; old coordinators answer them with silence (BadTag drop) and the client falls back loudly.
- QoS is required (user): anti-amplification padding, per-owner put quota, per-IP get rate limit, global caps. All caps live in `invite_store.rs` as consts.
- Wire numerology (single source of truth, define once in `wire.rs`):
  - `INVITE_ID_LEN = 16` (first 16 bytes of sha256(blob bytes))
  - `INVITE_BLOB_MAX = 8192` (raw blob bytes)
  - `INVITE_CHUNK_BYTES = 1000` (per InviteChunk payload)
  - `INVITE_GET_PAD = 1024` (minimum zero-pad on InviteGet → reply ≤ request, reflection amplification ≤ 1×)
  - Tags: `TAG_INVITE_PUT = 12`, `TAG_INVITE_PUT_ACK = 13`, `TAG_INVITE_GET = 14`, `TAG_INVITE_CHUNK = 15` (8/9 stay reserved).
- sha256 via the dep each crate already has: `commonware_cryptography::{Hasher as _, Sha256}` (the `genesis_namespace` idiom, `bin/node/src/config/mod.rs:137`); nat-traversal already depends on commonware-cryptography.
- Private-policy coordinators deny `InviteGet` from cap-less joiners — documented limitation; the full blob remains the universal fallback everywhere.

---

### Task 1: Wire — four invite variants on the `Msg` enum

**Files:**
- Modify: `crates/system/nat-traversal/src/wire.rs` (enum :9-43, tags :61-72, `MAX_ENCODED_LEN` :165, `write` :178, `subject_key` :228, `read` :256, tests)

**Interfaces:**
- Consumes: existing `Reader`, `put*` helpers, `ArrayVec` encoding.
- Produces (later tasks depend on these exact shapes):

```rust
Msg::InvitePut  { key: NodeKey, id: [u8; INVITE_ID_LEN], expires_unix_secs: u64, blob: Vec<u8> }
Msg::InvitePutAck { id: [u8; INVITE_ID_LEN], ok: bool }
Msg::InviteGet  { key: NodeKey, id: [u8; INVITE_ID_LEN], chunk: u16, pad: u16 }
Msg::InviteChunk { id: [u8; INVITE_ID_LEN], chunk: u16, total: u16, bytes: Vec<u8> }
```

- [ ] **Step 1: Write failing roundtrip tests** (extend `every_variant_roundtrips`, plus bounds tests)

```rust
// inside the cases vec in every_variant_roundtrips():
Msg::InvitePut {
    key: NodeKey([11u8; 32]),
    id: [0xcd; INVITE_ID_LEN],
    expires_unix_secs: 1_800_000_000,
    blob: vec![0xee; 1500],
},
Msg::InvitePutAck { id: [0xcd; INVITE_ID_LEN], ok: true },
Msg::InviteGet { key: NodeKey([12u8; 32]), id: [0xcd; INVITE_ID_LEN], chunk: 3, pad: INVITE_GET_PAD },
Msg::InviteChunk { id: [0xcd; INVITE_ID_LEN], chunk: 3, total: 9, bytes: vec![0xaa; INVITE_CHUNK_BYTES] },
```

```rust
#[test]
fn invite_put_rejects_an_oversized_blob_and_get_pads_the_datagram() {
    // an InvitePut blob above INVITE_BLOB_MAX must not decode (buffer bound).
    let mut big = Msg::InvitePut {
        key: NodeKey([1u8; 32]),
        id: [0; INVITE_ID_LEN],
        expires_unix_secs: 0,
        blob: vec![0; INVITE_BLOB_MAX],
    }
    .encode();
    // grow the declared length past the cap by hand: tag(1)+key(32)+id(16)+expires(8) then len u16
    let len_at = 1 + 32 + INVITE_ID_LEN + 8;
    big[len_at..len_at + 2].copy_from_slice(&((INVITE_BLOB_MAX as u16) + 1).to_be_bytes());
    big.push(0);
    assert!(Msg::decode(&big).is_err());

    // an InviteGet's encoded datagram is AT LEAST pad bytes long — the
    // anti-amplification property is structural, not caller discipline.
    let get = Msg::InviteGet { key: NodeKey([2u8; 32]), id: [7; INVITE_ID_LEN], chunk: 0, pad: INVITE_GET_PAD };
    assert!(get.encode().len() >= INVITE_GET_PAD as usize);
    assert_eq!(Msg::decode(&get.encode()).unwrap(), get);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `ops/build-with.sh cargo test -p nat-traversal wire`
Expected: FAIL — variants don't exist (compile error counts).

- [ ] **Step 3: Implement the variants**

Consts + enum fields as in Interfaces. Encoding layouts (big-endian lengths, matching the file's style):

```
InvitePut:    tag(1) ‖ key(32) ‖ id(16) ‖ expires(8) ‖ blob_len(u16, ≤ INVITE_BLOB_MAX) ‖ blob
InvitePutAck: tag(1) ‖ id(16) ‖ ok(1: 0|1)
InviteGet:    tag(1) ‖ key(32) ‖ id(16) ‖ chunk(2) ‖ pad(2) ‖ <pad zero bytes>
InviteChunk:  tag(1) ‖ id(16) ‖ chunk(2) ‖ total(2) ‖ len(u16, ≤ INVITE_CHUNK_BYTES) ‖ bytes
```

`write` arms: for `InviteGet`, after writing `pad`, extend with `pad` zero bytes (`out.try_extend_from_slice` via a fixed zero slice loop — pad is capped by the type at u16 but reject encode >4096 with the same expect the file uses). `read` arms: bounds-check `blob_len <= INVITE_BLOB_MAX` and `len <= INVITE_CHUNK_BYTES` (else `WireError::Short`-class error — add `WireError::TooLarge` if clearer); for `InviteGet` read `pad` then `take(pad)` and DISCARD (zeros not enforced — only size matters).

`subject_key`: `Msg::InvitePut { key, .. } | Msg::InviteGet { key, .. } => Some(*key)` — both become authenticable requests; the existing self-op rule (`coordinator.rs:75-76`) then enforces `key == caller` for free. `InvitePutAck`/`InviteChunk` return `None` (node-directed responses).

`MAX_ENCODED_LEN`: the new max is InvitePut: `1 + 32 + INVITE_ID_LEN + 8 + 2 + INVITE_BLOB_MAX`. Update the doc comment: the "stack replies" story still holds — `InviteChunk` (1023 B) is the largest reply. `AuthRequest::MAX_ENCODED_LEN` needs no formula change (it already adds envelope overhead to `Msg::MAX_ENCODED_LEN`).

- [ ] **Step 4: Run wire tests**

Run: `ops/build-with.sh cargo test -p nat-traversal wire`
Expected: PASS, including `auth_request_roundtrips_for_every_request_shape` extended with an `InvitePut`/`InviteGet` inner (add both to its `inners` vec — they are request shapes now).

- [ ] **Step 5: Commit**

```bash
git add crates/system/nat-traversal/src/wire.rs
git commit -m "feat(nat-traversal): invite put/get wire variants with structural anti-amplification pad"
```

---### Task 2: `InviteStore` — bounded, TTL'd, quota'd blob store

**Files:**
- Create: `crates/system/nat-traversal/src/invite_store.rs`
- Modify: `crates/system/nat-traversal/src/lib.rs` (add `pub mod invite_store;` + re-export `InviteStore`)

**Interfaces:**
- Consumes: `NodeKey`, wire consts from Task 1.
- Produces:

```rust
pub struct InviteStore { /* entries, get_limiter, ttl-capped */ }
pub enum PutOutcome { Stored, Replaced, QuotaExceeded, TooLarge, BadId, BadExpiry }
impl InviteStore {
    pub fn put(&mut self, owner: NodeKey, id: [u8; 16], blob: Vec<u8>, expires_unix_secs: u64, now: u64) -> PutOutcome;
    /// None = unknown/expired id. Some((bytes_of_chunk, total_chunks)); out-of-range chunk -> empty bytes with the real total.
    pub fn chunk(&mut self, id: [u8; 16], chunk: u16, now: u64) -> Option<(Vec<u8>, u16)>;
    /// per-source-IP token bucket for gets: false = rate-limited, drop silently.
    pub fn allow_get(&mut self, src_ip: std::net::IpAddr, now: u64) -> bool;
}
```

- [ ] **Step 1: Write failing tests** (same file, `#[cfg(test)]`)

```rust
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
    for i in 0..total { whole.extend(s.chunk(id, i, 0).unwrap().0); }
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
    for _ in 0..GET_BURST { assert!(s.allow_get(ip, 0)); }
    assert!(!s.allow_get(ip, 0), "burst exhausted");
    assert!(s.allow_get(ip, 10), "tokens refill with time");
    assert!(s.allow_get("203.0.113.10".parse().unwrap(), 0), "another ip has its own bucket");
}
```

- [ ] **Step 2: Run to verify failure** — `ops/build-with.sh cargo test -p nat-traversal invite_store` → FAIL (module missing).

- [ ] **Step 3: Implement**

```rust
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

struct Entry { blob: Vec<u8>, expires: u64, owner: NodeKey }

#[derive(Default)]
pub struct InviteStore {
    entries: HashMap<[u8; INVITE_ID_LEN], Entry>,
    gets: HashMap<IpAddr, (f64, u64)>, // (tokens, last_refill_secs)
}
```

`put`: reject `blob.len() > INVITE_BLOB_MAX` (TooLarge), `invite_id(&blob) != id` (BadId), `expires <= now || expires - now > MAX_INVITE_TTL_SECS` (BadExpiry). Purge expired entries (retain). Existing id → overwrite, `Replaced`. Else quota: `entries.values().filter(|e| e.owner == owner).count() >= MAX_INVITES_PER_OWNER` → `QuotaExceeded` (linear scan: puts are rare, map ≤ 4096 — ponytail: index per owner only if profiles ever care). Global cap: at `MAX_INVITES`, evict the soonest-expiry entry. Insert → `Stored`.

`chunk`: expired/missing → None. `total = blob.len().div_ceil(INVITE_CHUNK_BYTES) as u16` (min 1 for an empty blob is unreachable — blobs are non-empty). Out-of-range chunk → `Some((vec![], total))`. Else the byte slice.

`allow_get`: bucket = entry or (GET_BURST as f64, now); refill `tokens = (tokens + (now-last)*GET_RATE_PER_SEC as f64).min(GET_BURST as f64)`; `tokens >= 1.0` → consume, true; else false. Insert-at-cap: evict the entry with the smallest `last_refill`.

Deliberate simplification vs the spec's "token-bucket rate limit per key on InvitePut": the live-count quota (32/owner) already bounds storage, and every put pays PoP signature verification BEFORE dispatch — the same per-datagram CPU floor as all authenticated coordinator ops. A put-specific rate bucket adds state for no new bound. `// ponytail: put quota is count-based; add a per-key put bucket only if a PoP-flood profile ever shows dispatch cost mattering.`

- [ ] **Step 4: Run** — `ops/build-with.sh cargo test -p nat-traversal invite_store` → PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/system/nat-traversal/src/invite_store.rs crates/system/nat-traversal/src/lib.rs
git commit -m "feat(nat-traversal): bounded content-addressed InviteStore with put quota and get rate limit"
```

---

### Task 3: Coordinator dispatch + buffer bump

**Files:**
- Modify: `crates/system/nat-traversal/src/coordinator.rs` (struct :110-124, `handle_with_caller_replies` :256-330, tests)
- Modify: `crates/system/nat-traversal/src/client.rs` (worker-loop recv buffer `let mut buf = [0u8; 512];` :745, and the same buffer in `run_coordinator_with`'s inline loop — find with `grep -n "0u8; 512" client.rs`)

**Interfaces:**
- Consumes: Task 1 variants, Task 2 store.
- Produces: coordinator answers `InvitePut` (authenticated, self-op) with `InvitePutAck`; answers `InviteGet` with one `InviteChunk`; `Coordinator` gains a private `invites: InviteStore` field.

- [ ] **Step 1: Write failing coordinator tests**

```rust
#[test]
fn authenticated_put_then_gets_reassemble_the_blob() {
    use crate::auth::{AuthPolicy, now_secs, sign_authenticator};
    use crate::invite_store::invite_id;
    use commonware_cryptography::{Signer as _, ed25519};

    let node = ed25519::PrivateKey::from_seed(1);
    let mut k = [0u8; 32];
    k.copy_from_slice(node.public_key().as_ref());
    let caller = NodeKey(k);
    let now = now_secs();
    let mut c = Coordinator::with_policy(AuthPolicy::Open { require_pop: true });
    let src = addr(1, 1111);

    let blob = vec![0xAB; 1500];
    let id = invite_id(&blob);
    let put = Msg::InvitePut { key: caller, id, expires_unix_secs: now + 3600, blob: blob.clone() };
    let auth = sign_authenticator(&node, &put.encode(), now, None);
    let out = c.handle_auth(src, AuthRequest { caller, inner: put, auth }, now);
    assert_eq!(out, vec![(src, Msg::InvitePutAck { id, ok: true })]);

    // the joiner (a DIFFERENT key) fetches both chunks, PoP-authenticated.
    let joiner = ed25519::PrivateKey::from_seed(2);
    let mut jk = [0u8; 32];
    jk.copy_from_slice(joiner.public_key().as_ref());
    let jcaller = NodeKey(jk);
    let mut whole = Vec::new();
    for chunk in 0..2u16 {
        let get = Msg::InviteGet { key: jcaller, id, chunk, pad: crate::wire::INVITE_GET_PAD };
        let auth = sign_authenticator(&joiner, &get.encode(), now, None);
        let out = c.handle_auth(addr(2, 2222), AuthRequest { caller: jcaller, inner: get, auth }, now);
        let [(_, Msg::InviteChunk { total: 2, bytes, .. })] = out.as_slice() else {
            panic!("expected one InviteChunk, got {out:?}");
        };
        whole.extend_from_slice(bytes);
    }
    assert_eq!(whole, blob);
}

#[test]
fn an_underpadded_get_and_an_unknown_id_answer_safely() {
    use crate::auth::{AuthPolicy, now_secs, sign_authenticator};
    use crate::invite_store::{GET_BURST, invite_id};
    use commonware_cryptography::{Signer as _, ed25519};

    let joiner = ed25519::PrivateKey::from_seed(9);
    let mut jk = [0u8; 32];
    jk.copy_from_slice(joiner.public_key().as_ref());
    let caller = NodeKey(jk);
    let now = now_secs();
    let mut c = Coordinator::with_policy(AuthPolicy::Open { require_pop: true });
    let src = addr(3, 3333);
    let mut send_get = |c: &mut Coordinator, id: [u8; 16], chunk: u16, pad: u16| {
        let get = Msg::InviteGet { key: caller, id, chunk, pad };
        let auth = sign_authenticator(&joiner, &get.encode(), now, None);
        c.handle_auth(src, AuthRequest { caller, inner: get, auth }, now)
    };

    // pad below the floor → DROPPED, no reply (no reflection amplification).
    let id = invite_id(&[1, 2, 3]);
    assert!(send_get(&mut c, id, 0, crate::wire::INVITE_GET_PAD - 1).is_empty());

    // unknown id, properly padded → total 0: the honest "link is dead" signal.
    let out = send_get(&mut c, id, 0, crate::wire::INVITE_GET_PAD);
    assert_eq!(out, vec![(src, Msg::InviteChunk { id, chunk: 0, total: 0, bytes: vec![] })]);

    // rate limit: exhausting the burst from one ip drops the next get.
    for _ in 0..(GET_BURST - 1) {
        assert!(!send_get(&mut c, id, 0, crate::wire::INVITE_GET_PAD).is_empty());
    }
    assert!(send_get(&mut c, id, 0, crate::wire::INVITE_GET_PAD).is_empty());
}
```

- [ ] **Step 2: Run to verify failure** — `ops/build-with.sh cargo test -p nat-traversal coordinator` → FAIL.

- [ ] **Step 3: Implement dispatch**

`Coordinator` gains `invites: InviteStore` (all three constructors — it's `Default`). New arms in `handle_with_caller_replies`:

```rust
Msg::InvitePut { key, id, expires_unix_secs, blob } => {
    // authenticated self-op (subject_key == caller enforced upstream); on
    // the legacy path `caller` is None — allowed only under fully-open,
    // same trust bar as a bare Register.
    let owner = caller.unwrap_or(key);
    let ok = matches!(
        self.invites.put(owner, id, blob, expires_unix_secs, now),
        PutOutcome::Stored | PutOutcome::Replaced
    );
    CoordinatorReplies::from_iter([(from, Msg::InvitePutAck { id, ok })])
}
Msg::InviteGet { id, chunk, pad, .. } => {
    // structural anti-amplification: the request datagram must be at least
    // as big as the biggest reply we would send back to a spoofed source.
    if (pad as usize) < crate::wire::INVITE_GET_PAD as usize
        || !self.invites.allow_get(from.ip(), now)
    {
        return CoordinatorReplies::new();
    }
    let (bytes, total) = self.invites.chunk(id, chunk, now).unwrap_or((Vec::new(), 0));
    CoordinatorReplies::from_iter([(from, Msg::InviteChunk { id, chunk, total, bytes })])
}
Msg::InvitePutAck { .. } | Msg::InviteChunk { .. } => CoordinatorReplies::new(), // node-directed
```

(Adjust the `pad` comparison to the actual const types; `INVITE_GET_PAD` is `u16`.)

Buffer bump in `client.rs`: both recv loops change `[0u8; 512]` → `[0u8; AuthRequest::MAX_ENCODED_LEN]`. The reply send path already uses `encode_inline()` and is unaffected.

- [ ] **Step 4: Run all nat-traversal tests** — `ops/build-with.sh cargo test -p nat-traversal` → PASS (existing rendezvous tests must be untouched).

- [ ] **Step 5: Commit**

```bash
git add crates/system/nat-traversal/src/coordinator.rs crates/system/nat-traversal/src/client.rs
git commit -m "feat(nat-traversal): coordinator shelves invites — authed put, padded rate-limited get"
```

---

### Task 4: `CoordClient` one-shot publish/fetch

**Files:**
- Modify: `crates/system/nat-traversal/src/client.rs` (mirror `lookup` :335 and `register` :305 — the request/await-reply and auth-signing plumbing already exists there; `bind_multi_auth` :181 shows how a signed client is built)

**Interfaces:**
- Consumes: Tasks 1–3.
- Produces:

```rust
impl CoordClient {
    /// publish a blob under its content id; Ok(true) = shelved.
    pub async fn invite_put(&self, id: [u8; 16], expires_unix_secs: u64, blob: Vec<u8>) -> std::io::Result<bool>;
    /// fetch and reassemble a shelved blob; Ok(None) = coordinator answered "unknown id".
    pub async fn invite_fetch(&self, id: [u8; 16]) -> std::io::Result<Option<Vec<u8>>>;
}
```

- [ ] **Step 1: Implement following the `lookup` pattern**

`invite_put`: encode `Msg::InvitePut { key: self.key, id, expires_unix_secs, blob }`, send through the same signed path `register`/`lookup` use, await the matching `InvitePutAck { id: got, ok }` (`got == id`) with the module's existing per-request timeout/retry idiom; return `ok`.

`invite_fetch`: request `chunk 0` (with `pad: INVITE_GET_PAD`); on `total == 0` → `Ok(None)`; else loop `1..total` collecting `bytes` in order (re-request a chunk on timeout, 3 attempts each, mirroring the retry constants `lookup` uses); verify at the end `invite_store::invite_id(&whole) == id` — mismatch is an io::Error ("coordinator returned tampered bytes") since content addressing makes this impossible against an honest shelf.

- [ ] **Step 2: Wire-level integration test**

In `client.rs`'s test module (find the existing register/lookup socket-loop tests with `grep -n "run_coordinator" client.rs` and copy their socket + spawn scaffolding exactly — policy, ephemeral bind, task spawn):

```rust
#[tokio::test]
async fn invite_put_then_fetch_roundtrips_over_the_socket() {
    // scaffolding: bind an ephemeral UDP socket, spawn run_coordinator on it
    // with AuthPolicy::Open { require_pop: true } — copied verbatim from the
    // existing lookup loop test in this module.
    let coord_addr = /* the spawned coordinator's local_addr, per that test */;

    let inviter = /* CoordClient with a signer, as the auth loop tests build one */;
    let blob = (0..2500u32).map(|i| i as u8).collect::<Vec<u8>>();
    let id = crate::invite_store::invite_id(&blob);
    assert!(inviter.invite_put(id, crate::auth::now_secs() + 3600, blob.clone()).await.unwrap());

    let joiner = /* a SECOND signed CoordClient, different seed */;
    assert_eq!(joiner.invite_fetch(id).await.unwrap(), Some(blob));
    assert_eq!(joiner.invite_fetch([0xEE; 16]).await.unwrap(), None);
}
```

The three `/* … */` slots are the module's existing scaffolding idioms — copy them from the neighboring test, do not invent new plumbing.

Run: `ops/build-with.sh cargo test -p nat-traversal client` → PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/system/nat-traversal/src/client.rs
git commit -m "feat(nat-traversal): CoordClient invite_put/invite_fetch with content verification"
```

---

### Task 5: Short-URL codec + node CLI (`invite --short`, `join 🦆://…`)

**Files:**
- Modify: `bin/node/src/config/invite.rs` (codec + tests), `bin/node/src/cli.rs` (`cmd_invite` :198-379, `cmd_join` :1174-1179)

**Interfaces:**
- Consumes: `CoordClient` (Task 4), `config::primary_coordinator_or_default` (`bin/node/src/config/mod.rs` — the same resolution `cli.rs:115` uses), `INVITE_B64`, `decode_invite`.
- Produces:

```rust
// config/invite.rs
pub const SHORT_INVITE_SCHEME: &str = "🦆://";
/// 🦆://<name>/<base64url(id)> — name is the chain_id's human half ("<name>#<salt>").
pub fn short_invite_url(chain_id: &str, id: &[u8; 16]) -> String;
/// parse a short url -> (name, id). None when `s` is not the short scheme.
pub fn parse_short_invite(s: &str) -> Option<(String, [u8; 16])>;
/// the content id of an encoded 🦆<base64> blob (sha256 of its decoded bytes).
pub fn invite_blob_id(blob: &str) -> Result<[u8; 16], String>;
```

- [ ] **Step 1: Write failing codec tests** (in `config/invite.rs` tests)

```rust
#[test]
fn short_invite_url_roundtrips_and_rejects_junk() {
    let id = [0x5a; 16];
    let url = short_invite_url("ducktape#a1b2c3d4", &id);
    assert!(url.starts_with("🦆://ducktape/"), "{url}");
    assert_eq!(parse_short_invite(&url), Some(("ducktape".into(), id)));
    // a full blob is NOT a short url; junk ids and junk schemes are None.
    assert_eq!(parse_short_invite("🦆AbCdEf"), None);
    assert_eq!(parse_short_invite("🦆://name/not-base64!!!"), None);
    assert_eq!(parse_short_invite("duck://name/AAAA"), None);
}

#[test]
fn invite_blob_id_is_the_content_hash_of_the_decoded_bytes() {
    let issuer = ed25519::PrivateKey::from_seed(7);
    let d = front_test_descriptor(&issuer);
    let token = mint_invite_token(&issuer, d.genesis_namespace().as_bytes());
    let blob = encode_invite(&d, &token, None, &[], u64::MAX, &issuer).expect("encode");
    let id = invite_blob_id(&blob).expect("id");
    // stable under re-parse, changes when the blob changes.
    assert_eq!(id, invite_blob_id(&blob).unwrap());
    let other = encode_invite(&d, &mint_invite_token(&issuer, d.genesis_namespace().as_bytes()), None, &[], u64::MAX, &issuer).unwrap();
    assert_ne!(id, invite_blob_id(&other).unwrap());
}
```

- [ ] **Step 2: Run to verify failure**, then implement: `short_invite_url` = `format!("{SHORT_INVITE_SCHEME}{name}/{}", INVITE_B64.encode(id))` with `name = chain_id.split('#').next().unwrap_or(chain_id)`; `parse_short_invite` strips the scheme, splits on the single `/`, base64-decodes exactly 16 bytes; `invite_blob_id` strips `INVITE_PREFIX`, base64-decodes, sha256-prefixes (the `commonware_cryptography::Sha256` idiom from `config/mod.rs:137`).

Run: `ops/build-with.sh cargo test -p ducktape-node invite` → PASS.

- [ ] **Step 3: `cmd_invite --short`**

After the existing `println!` of the blob (`cli.rs:369-379`), add:

Add one more codec helper next to `invite_blob_id` so the CLI never re-implements the decode:

```rust
/// the decoded (raw) bytes of an encoded 🦆<base64> blob — what the
/// coordinator shelves and what `invite_blob_id` hashes.
pub fn invite_blob_bytes(blob: &str) -> Result<Vec<u8>, String> {
    use base64::Engine as _;
    let body = blob
        .trim()
        .strip_prefix(INVITE_PREFIX)
        .ok_or("not a ducktape invite blob")?;
    INVITE_B64.decode(body).map_err(|e| format!("invite is not valid base64url: {e}"))
}
// invite_blob_id = sha256(invite_blob_bytes(blob))[..16]
```

Then in `cmd_invite`, bind the encoded blob to a variable instead of printing inline (`let blob_string = config::encode_invite(&invite_descriptor, &token, wireguard.as_ref(), &fronts, expires, &key)?; println!("{blob_string}");`), and append:

```rust
if flags.contains_key("short") {
    // the SAME coordinator resolution the network descriptor uses at network
    // creation (cli.rs:115) — config value or the shipped default.
    let coord = config::primary_coordinator_or_default(raw.primary_coordinator.as_deref())
        .ok_or("--short needs a primary coordinator (config or default)")?;
    let coord: std::net::SocketAddr = /* resolve host:port — mirror how the
        reachability plane parses the same string; grep -rn "primary_coordinator"
        bin/node/src/replica for the live parse */;
    let raw_bytes = config::invite_blob_bytes(&blob_string)?;
    let id = config::invite_blob_id(&blob_string)?;
    let published = tokio::runtime::Runtime::new()?.block_on(async {
        let client = nat_traversal::CoordClient::bind_multi_auth(
            nat_traversal::NodeKey(own), // `own` = this member's raw key, cli.rs:318
            /* signer arg: the identity `key` loaded at cli.rs:218 — match
               bind_multi_auth's exact parameter list at client.rs:181 */
            vec![coord],
        )
        .await?;
        client.invite_put(id, expires, raw_bytes).await
    })?;
    if !published {
        return Err("coordinator refused the short invite (quota or size) — share the full blob".into());
    }
    println!("{}", config::short_invite_url(&descriptor.chain_id, &id));
}
```

The two remaining `/* … */` slots are existing idioms to copy, not designs to invent: the coordinator-string parse (the reachability plane already dials this exact config value) and `bind_multi_auth`'s parameter list (`client.rs:181`, plus its live call site — `grep -rn "bind_multi_auth" bin/node/src`). If `primary_coordinator_or_default`'s real signature differs (check `cli.rs:115-118`), follow the call site there verbatim. Output contract: **the short URL is the LAST line** when `--short` is passed (the Tauri layer takes `last_line`).

- [ ] **Step 4: `cmd_join` accepts the short form**

At the top of `cmd_join` (`cli.rs:1176-1179`), before `decode_invite`:

```rust
let blob = match config::parse_short_invite(blob) {
    Some((name, id)) => {
        let coord = /* --primary-coordinator flag or config::DEFAULT_PRIMARY_COORDINATOR */;
        let raw = tokio::runtime::Runtime::new()?
            .block_on(async {
                let client = nat_traversal::CoordClient::bind(/* fresh throwaway key */, coord).await?;
                client.invite_fetch(id).await
            })?
            .ok_or("this short invite is not on the coordinator (expired, evicted, or a coordinator restart) — ask the inviter to reveal it again or paste the full blob")?;
        let fetched = format!("🦆{}", config::INVITE_B64_PUBLIC.encode(&raw)); // re-wrap for the normal path
        let invite = config::decode_invite(&fetched)?; // envelope + token + expiry verify
        let got_name = invite.descriptor.chain_id.split('#').next().unwrap_or("");
        if got_name != name {
            return Err(format!("short invite names network {name:?} but the blob is for {got_name:?} — refusing").into());
        }
        fetched
    }
    None => blob.clone(),
};
```

Details: expose the base64 engine (`INVITE_B64` is private — add `pub(crate)` or a `pub fn wrap_invite_bytes(raw: &[u8]) -> String` helper in config/invite.rs, the lazier move). The fetch client authenticates with a throwaway key under the public-PoP policy (the workspace identity isn't created until later in `cmd_join` — a fresh key is correct and sufficient; `CoordClient::bind` vs `bind_multi_auth`: use whichever the crate requires to sign PoP — check `bind`'s signature: if it takes only `NodeKey` it rides unauthenticated and would be dropped under `require_pop`; in that case generate an ephemeral `ed25519::PrivateKey` via OS rng and use `bind_multi_auth`).

- [ ] **Step 5: e2e — mint short, join via short**

Extend `bin/node/tests/coordinated_invite_cli.rs` (it already spins a real coordinator for CLI flows — follow its socket/policy setup): founder `invite --short` → assert last line parses with `parse_short_invite`; `join <short-url> --dir …` → workspace materializes with the right `chain_id`; then kill the coordinator and `join` a second workspace with the same URL → loud "not on the coordinator" error, full blob still joins.

Run: `ops/build-with.sh cargo test -p ducktape-node --test coordinated_invite_cli -- --nocapture`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add bin/node/src/config/invite.rs bin/node/src/cli.rs bin/node/tests/coordinated_invite_cli.rs
git commit -m "feat(node): 🦆://<chain>/<id> short invites — publish on mint, fetch on join"
```

---

### Task 6: App — short link first in the invite UI

**Files:**
- Modify: `app/src-tauri/src/workspaces/mod.rs` (`workspace_invite_blob_blocking` :500-505)
- Modify: `app/src-tauri/build.rs` + `app/src-tauri/capabilities/trusted.toml` — ONLY if a new command is added; we extend the existing one instead, so likely untouched (verify: changing a command's signature does not need re-registration, new commands do)
- Modify: `app/src/console/store/actions.ts` (`revealInvite` :2792-2797), `app/src/console/views/MembersView.tsx` (invite section :846-887)

**Interfaces:**
- Consumes: `invite --config <cfg> --short` (Task 5; short URL = last line, full blob printed above it).
- Produces: `workspace_invite_blob` returns `{ short: string | null, blob: string }` (JSON object instead of the bare string — in-place change, update the one caller).

- [ ] **Step 1: Tauri command returns both forms**

```rust
#[derive(serde::Serialize, Clone)]
pub struct InviteForms { pub short: Option<String>, pub blob: String }

fn workspace_invite_blob_blocking(app: crate::rt::AppHandle, id: String) -> Result<InviteForms, String> {
    let reg = load_registry(&app)?;
    let ws = find(&reg, &id)?;
    let cfg = node_toml(&workspaces_dir(&app)?.join(&ws.id));
    let cfg_s = cfg.to_string_lossy();
    match run_verb(&["invite", "--config", &cfg_s, "--short"]) {
        Ok(out) => {
            // --short prints the blob line then the short url as the LAST line.
            let short = last_line(&out);
            let blob = out
                .lines()
                .rev()
                .find(|l| l.trim_start().starts_with("🦆") && !l.contains("://"))
                .unwrap_or_default()
                .trim()
                .to_string();
            Ok(InviteForms { short: Some(short), blob })
        }
        // coordinator unreachable/refusing: the full blob must still work.
        Err(_) => run_verb(&["invite", "--config", &cfg_s])
            .map(|out| InviteForms { short: None, blob: last_line(&out) }),
    }
}
```

(Keep the async `workspace_invite_blob` wrapper; only its return type changes to `InviteForms`.)

- [ ] **Step 2: Console UI**

`actions.ts` `revealInvite`: store both fields (`state.inviteShort`, `state.inviteBlob`) from the invoke result. `MembersView.tsx` invite section: the short URL becomes the primary read-only field + copy button when present, with helper copy "One person, expires in 7 days. Link dies if the coordinator restarts — reveal again to refresh."; the full blob moves into a collapsed "Full invite (works without the coordinator)" `<details>` block with its own copy button. Join input (`onboarding` join form) needs no change — the join verb accepts both strings; only relax any client-side "must start with 🦆 and be long" validation to also accept `🦆://` URLs if such a check exists (`grep -rn "🦆" app/src` to find validators).

- [ ] **Step 3: Verify in the running app**

Fleet/tauri-debug QA (skills `qa`/`tauri-debug`): reveal invite in workspace A → copy short URL → join from a fresh instance via the short URL → JoinProgress reaches admitted. Screenshot the Members invite panel for the PR.

- [ ] **Step 4: Commit**

```bash
git add app/src-tauri/src/workspaces/mod.rs app/src/console/store/actions.ts app/src/console/views/MembersView.tsx
git commit -m "feat(app): short invite link first in Members, full blob as fallback"
```

---

### Task 7: Gates, PR

- [ ] **Step 1: Gates**

```bash
ops/build-with.sh cargo clippy -p nat-traversal --tests --no-deps
ops/build-with.sh cargo clippy -p ducktape-node --tests --no-deps
ops/build-with.sh cargo test -p nat-traversal
ops/build-with.sh cargo test -p ducktape-node --test coordinated_invite_cli --test invite_e2e
cd app && npm run typecheck   # or the repo's frontend check target — see app/package.json scripts
```
Expected: all green.

- [ ] **Step 2: PR against dev**

Title: `feat: coordinator-managed short invites (🦆://<chain>/<id>) with QoS`. Body must state: in-memory shelf (restart drops links, by design), the QoS numbers (pad 1024, 32/owner, 4096 global, 5/s+20 burst per IP, 8 KiB blob cap, 30 d TTL cap), the private-coordinator limitation, and that old coordinators simply drop the new tags (client falls back loudly to the full blob).
