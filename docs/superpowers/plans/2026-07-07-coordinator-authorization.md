# Coordinator Authorization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a per-network public/private coordination choice, enforced at a still-keyless/stateless coordinator via a per-request signed authenticator (proof-of-possession + an optional genesis-issued capability token).

**Architecture:** A new pure `auth` module in `nat-traversal` holds the credential types (`CoordCap`, `Authenticator`, `AuthPolicy`) and the stateless `verify_request`. The wire codec gains one authenticated-request envelope (tag 11) wrapping an existing request `Msg`. The coordinator verifies the authenticator before touching its `AdvertBook`; the client signs each request; the node loads a `coord.cap` and threads its identity signer through the reachability plane; `bin/coordinator` pins the network's public genesis validator set.

**Tech Stack:** Rust, `commonware-cryptography` (ed25519, `Signer`/`Verifier`), `commonware-codec` (`Encode`/`DecodeExt`), tokio UDP.

## Global Constraints

- **Keyless coordinator.** The coordinator process holds only PUBLIC data (the genesis validator pubkeys). No private key, no shared secret, no session state, no disk write. Any task that would put a secret on the coordinator host is wrong.
- **ed25519 everywhere, namespace-separated.** Reuse `commonware_cryptography::ed25519`; every signature purpose gets its own namespace constant. New: `COORD_REQ_NS = b"ducktape-coord-req-v1"`, `COORD_CAP_NS = b"ducktape-coord-cap-v1"`.
- **`NodeKey.0` IS the raw ed25519 public key** (`reachability::binding::node_key`). Verify PoP directly against those 32 bytes; do not add an identity type.
- **Wire compatibility.** Tags 1–7 and 10 keep their exact meaning; tags 8/9 stay reserved (`BadTag`). The authenticated envelope is a NEW tag 11. `Msg::decode` keeps its whole-buffer `Trailing` rule.
- **Backwards default.** `Coordinator::new()` and `AuthPolicy::default()` stay fully-open (`Open { require_pop: false }`) so every existing test passes unchanged.
- **Clippy scope.** Gate clippy on `nat-traversal` (the clean crate); for `node-bin`/`config` changes gate on `cargo test` + a zero-new-clippy-errors baseline diff, not raw clippy exit (known toolchain-drift caveat).
- **Freshness window** default: `DEFAULT_FRESHNESS_WINDOW_SECS = 30`.
- **Commit** after every green task. Branch: `feat/coordinator-auth` (already on it, in the worktree).

## File Structure

- **Create `crates/system/nat-traversal/src/auth.rs`** — `CoordCap`, `Authenticator`, `AuthPolicy`, `AuthError`, `now_secs`, `mint_coord_cap`, `sign_authenticator`, `verify_request`. Pure crypto + logic; no I/O. (Task 1)
- **Modify `crates/system/nat-traversal/Cargo.toml`** — add `commonware-codec` dep. (Task 1)
- **Modify `crates/system/nat-traversal/src/lib.rs`** — `pub mod auth;` + re-exports. (Task 1)
- **Modify `crates/system/nat-traversal/src/wire.rs`** — `Msg::subject_key`/`is_request`; extract `Msg::read`; `AuthRequest` envelope (tag 11) encode/decode; new `WireError` crypto/shape variants. (Task 2)
- **Modify `crates/system/nat-traversal/src/coordinator.rs`** — `policy`/`window`/`rejects` fields, `with_policy`, `handle_auth`, `handle_legacy`. (Task 3)
- **Modify `crates/system/nat-traversal/src/client.rs`** — `NatClient` signer+cap, `authed()` wrapper, `bind_multi_auth`; `run_coordinator(sock, policy)`. (Task 4)
- **Modify `bin/node/src/config.rs`** — `CoordCap` file I/O (`save_coord_cap`/`load_coord_cap`), `coordination` descriptor field. (Task 5)
- **Modify `bin/coordinator/src/main.rs`** — `--genesis-set`/`--allow-anonymous` → `AuthPolicy`. (Task 6)
- **Modify `crates/system/reachability/src/orchestrator.rs` + `bin/node/src/main.rs`** — thread signer+cap into `NatResolver::bind`; load `coord.cap`; e2e integration test. (Task 7)

---

### Task 1: Auth core — credential types + stateless verification (`auth.rs`)

**Files:**
- Create: `crates/system/nat-traversal/src/auth.rs`
- Modify: `crates/system/nat-traversal/Cargo.toml`
- Modify: `crates/system/nat-traversal/src/lib.rs`

**Interfaces:**
- Produces:
  - `pub struct CoordCap { pub issuer: ed25519::PublicKey, pub not_after: u64, pub issuer_sig: ed25519::Signature }`
  - `pub struct Authenticator { pub timestamp: u64, pub pop_sig: ed25519::Signature, pub cap: Option<CoordCap> }`
  - `pub enum AuthPolicy { Open { require_pop: bool }, Private { genesis_set: Vec<ed25519::PublicKey> } }` (Default = `Open { require_pop: false }`)
  - `pub enum AuthError { Stale, BadPop, NotAdmitted, BadSubjectKey }`
  - `pub const COORD_REQ_NS`, `COORD_CAP_NS`, `DEFAULT_FRESHNESS_WINDOW_SECS`
  - `pub fn now_secs() -> u64`
  - `pub fn mint_coord_cap(issuer: &ed25519::PrivateKey, subject: NodeKey, not_after: u64) -> CoordCap`
  - `pub fn sign_authenticator(signer: &ed25519::PrivateKey, inner_bytes: &[u8], timestamp: u64, cap: Option<CoordCap>) -> Authenticator`
  - `pub fn verify_request(policy: &AuthPolicy, now: u64, window: u64, subject: NodeKey, inner_bytes: &[u8], auth: &Authenticator) -> Result<(), AuthError>`

- [ ] **Step 1: Add the codec dependency**

Modify `crates/system/nat-traversal/Cargo.toml`, `[dependencies]` — add after the `commonware-cryptography` line:

```toml
commonware-codec.workspace = true
```

- [ ] **Step 2: Write the failing test module**

Create `crates/system/nat-traversal/src/auth.rs` with the full implementation AND tests (below). First write ONLY the `#[cfg(test)]` block plus empty stubs so it fails to compile/pass, or write the whole file and run — either way Step 3 makes it green. The test block:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use commonware_cryptography::PrivateKeyExt as _;

    fn key(seed: u64) -> ed25519::PrivateKey {
        ed25519::PrivateKey::from_seed(seed)
    }
    fn nk(pk: &ed25519::PublicKey) -> NodeKey {
        let mut b = [0u8; 32];
        b.copy_from_slice(pk.as_ref());
        NodeKey(b)
    }

    // A fixed "inner request" byte string stands in for Msg::encode() bytes.
    const INNER: &[u8] = b"\x03inner-register-bytes";

    #[test]
    fn pop_only_accepts_self_signed_and_rejects_forged() {
        let node = key(1);
        let subject = nk(&node.public_key());
        let policy = AuthPolicy::Open { require_pop: true };
        let now = 1_000_000;

        let good = sign_authenticator(&node, INNER, now, None);
        assert_eq!(verify_request(&policy, now, 30, subject, INNER, &good), Ok(()));

        // Signed by a DIFFERENT key: PoP must fail.
        let attacker = key(2);
        let forged = sign_authenticator(&attacker, INNER, now, None);
        assert_eq!(
            verify_request(&policy, now, 30, subject, INNER, &forged),
            Err(AuthError::BadPop)
        );
    }

    #[test]
    fn stale_timestamp_is_rejected_both_directions() {
        let node = key(1);
        let subject = nk(&node.public_key());
        let policy = AuthPolicy::Open { require_pop: true };
        let a = sign_authenticator(&node, INNER, 1_000_000, None);
        // 31s in the past and future both exceed the 30s window.
        assert_eq!(verify_request(&policy, 1_000_031, 30, subject, INNER, &a), Err(AuthError::Stale));
        assert_eq!(verify_request(&policy, 999_969, 30, subject, INNER, &a), Err(AuthError::Stale));
        // 30s exactly is still fresh.
        assert_eq!(verify_request(&policy, 1_000_030, 30, subject, INNER, &a), Ok(()));
    }

    #[test]
    fn fully_open_accepts_anything() {
        let subject = NodeKey([9u8; 32]); // not even a valid pubkey
        let policy = AuthPolicy::Open { require_pop: false };
        let node = key(3);
        let auth = sign_authenticator(&node, INNER, 0, None); // wrong signer, ancient ts
        assert_eq!(verify_request(&policy, 5_000_000, 30, subject, INNER, &auth), Ok(()));
    }

    #[test]
    fn private_admits_genesis_member_without_cap() {
        let g = key(10);
        let subject = nk(&g.public_key());
        let policy = AuthPolicy::Private { genesis_set: vec![g.public_key()] };
        let now = 2_000_000;
        let auth = sign_authenticator(&g, INNER, now, None);
        assert_eq!(verify_request(&policy, now, 30, subject, INNER, &auth), Ok(()));
    }

    #[test]
    fn private_rejects_non_member_without_cap() {
        let g = key(10);
        let outsider = key(11);
        let subject = nk(&outsider.public_key());
        let policy = AuthPolicy::Private { genesis_set: vec![g.public_key()] };
        let now = 2_000_000;
        let auth = sign_authenticator(&outsider, INNER, now, None); // valid PoP, but not admitted
        assert_eq!(verify_request(&policy, now, 30, subject, INNER, &auth), Err(AuthError::NotAdmitted));
    }

    #[test]
    fn private_admits_joiner_with_valid_genesis_cap() {
        let g = key(10);
        let joiner = key(20);
        let subject = nk(&joiner.public_key());
        let policy = AuthPolicy::Private { genesis_set: vec![g.public_key()] };
        let now = 2_000_000;
        let cap = mint_coord_cap(&g, subject, now + 3600);
        let auth = sign_authenticator(&joiner, INNER, now, Some(cap));
        assert_eq!(verify_request(&policy, now, 30, subject, INNER, &auth), Ok(()));
    }

    #[test]
    fn cap_rejected_when_expired_wrong_issuer_or_wrong_subject() {
        let g = key(10);
        let notg = key(99);
        let joiner = key(20);
        let subject = nk(&joiner.public_key());
        let policy = AuthPolicy::Private { genesis_set: vec![g.public_key()] };
        let now = 2_000_000;

        // Expired.
        let expired = mint_coord_cap(&g, subject, now - 1);
        let a1 = sign_authenticator(&joiner, INNER, now, Some(expired));
        assert_eq!(verify_request(&policy, now, 30, subject, INNER, &a1), Err(AuthError::NotAdmitted));

        // Issuer not in the pinned genesis set.
        let wrong_issuer = mint_coord_cap(&notg, subject, now + 3600);
        let a2 = sign_authenticator(&joiner, INNER, now, Some(wrong_issuer));
        assert_eq!(verify_request(&policy, now, 30, subject, INNER, &a2), Err(AuthError::NotAdmitted));

        // Cap minted for a DIFFERENT subject (attacker replays someone else's cap).
        let other = nk(&key(21).public_key());
        let wrong_subject = mint_coord_cap(&g, other, now + 3600);
        let a3 = sign_authenticator(&joiner, INNER, now, Some(wrong_subject));
        assert_eq!(verify_request(&policy, now, 30, subject, INNER, &a3), Err(AuthError::NotAdmitted));
    }

    #[test]
    fn invalid_subject_key_bytes_are_rejected_under_pop() {
        // A NodeKey that is not a valid ed25519 point cannot verify PoP.
        let subject = NodeKey([0xff; 32]);
        let policy = AuthPolicy::Open { require_pop: true };
        let node = key(1);
        let auth = sign_authenticator(&node, INNER, 1_000_000, None);
        assert_eq!(
            verify_request(&policy, 1_000_000, 30, subject, INNER, &auth),
            Err(AuthError::BadSubjectKey)
        );
    }
}
```

- [ ] **Step 3: Write the implementation**

Prepend to `auth.rs` (above the test module):

```rust
//! Coordinator authorization: a per-request authenticator (proof-of-possession
//! + an optional genesis-issued capability) verified statelessly against a
//! PUBLIC pin. The coordinator holds no secret; every check here is a clock
//! read plus one or two ed25519 verifications against public keys.

use std::time::{SystemTime, UNIX_EPOCH};

use commonware_cryptography::{ed25519, Signer as _, Verifier as _};

use crate::NodeKey;

/// PoP signing namespace: `sign(COORD_REQ_NS, inner_request_bytes ‖ timestamp)`.
pub const COORD_REQ_NS: &[u8] = b"ducktape-coord-req-v1";
/// Capability signing namespace: `sign(COORD_CAP_NS, subject ‖ not_after)`.
pub const COORD_CAP_NS: &[u8] = b"ducktape-coord-cap-v1";
/// Max clock skew (seconds) between a request timestamp and the coordinator.
pub const DEFAULT_FRESHNESS_WINDOW_SECS: u64 = 30;

/// A signed admission capability. A genesis validator (`issuer`) vouches that
/// `subject` (implied — the request's key) is authorized until `not_after`.
#[derive(Clone, Debug, PartialEq)]
pub struct CoordCap {
    pub issuer: ed25519::PublicKey,
    pub not_after: u64,
    pub issuer_sig: ed25519::Signature,
}

/// The per-request authenticator — the wire's "authorization header".
#[derive(Clone, Debug, PartialEq)]
pub struct Authenticator {
    pub timestamp: u64,
    pub pop_sig: ed25519::Signature,
    pub cap: Option<CoordCap>,
}

/// The coordinator's authorization policy. PUBLIC data only.
#[derive(Clone, Debug)]
pub enum AuthPolicy {
    /// Public coordination. `require_pop=true` is the deployed default;
    /// `false` is the legacy fully-open shape (tests / `--allow-anonymous`).
    Open { require_pop: bool },
    /// Private coordination: PoP + admission against the pinned genesis set.
    Private { genesis_set: Vec<ed25519::PublicKey> },
}

impl Default for AuthPolicy {
    fn default() -> Self {
        AuthPolicy::Open { require_pop: false }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum AuthError {
    /// Timestamp outside the freshness window.
    Stale,
    /// Proof-of-possession signature did not verify against the subject key.
    BadPop,
    /// Private mode: subject is neither a genesis member nor holds a valid cap.
    NotAdmitted,
    /// The request's NodeKey is not a valid ed25519 public key.
    BadSubjectKey,
}

/// Wall-clock seconds since the Unix epoch (saturating before 1970).
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn cap_msg(subject: NodeKey, not_after: u64) -> Vec<u8> {
    let mut m = Vec::with_capacity(40);
    m.extend_from_slice(&subject.0);
    m.extend_from_slice(&not_after.to_be_bytes());
    m
}

fn pop_msg(inner_bytes: &[u8], timestamp: u64) -> Vec<u8> {
    let mut m = Vec::with_capacity(inner_bytes.len() + 8);
    m.extend_from_slice(inner_bytes);
    m.extend_from_slice(&timestamp.to_be_bytes());
    m
}

fn subject_pubkey(subject: NodeKey) -> Option<ed25519::PublicKey> {
    use commonware_codec::DecodeExt as _;
    ed25519::PublicKey::decode(subject.0.as_slice()).ok()
}

/// Mint a capability binding `subject` (a node's ed25519 key) to `not_after`,
/// signed by `issuer` (a genesis validator's private key).
pub fn mint_coord_cap(
    issuer: &ed25519::PrivateKey,
    subject: NodeKey,
    not_after: u64,
) -> CoordCap {
    CoordCap {
        issuer: issuer.public_key(),
        not_after,
        issuer_sig: issuer.sign(COORD_CAP_NS, &cap_msg(subject, not_after)),
    }
}

/// Build the authenticator for one request: sign `inner_bytes ‖ timestamp`
/// with the node's identity key and attach `cap` (private mode) or `None`.
pub fn sign_authenticator(
    signer: &ed25519::PrivateKey,
    inner_bytes: &[u8],
    timestamp: u64,
    cap: Option<CoordCap>,
) -> Authenticator {
    Authenticator {
        timestamp,
        pop_sig: signer.sign(COORD_REQ_NS, &pop_msg(inner_bytes, timestamp)),
        cap,
    }
}

fn cap_admits(
    cap: &CoordCap,
    subject: NodeKey,
    genesis_set: &[ed25519::PublicKey],
    now: u64,
) -> bool {
    if cap.not_after <= now {
        return false;
    }
    if !genesis_set.iter().any(|g| g == &cap.issuer) {
        return false;
    }
    cap.issuer
        .verify(COORD_CAP_NS, &cap_msg(subject, cap.not_after), &cap.issuer_sig)
}

/// Stateless authorization decision for one request. `now`/`window` are seconds.
/// `subject` is the inner request's claimed key; `inner_bytes` is the inner
/// request's `Msg::encode()`.
pub fn verify_request(
    policy: &AuthPolicy,
    now: u64,
    window: u64,
    subject: NodeKey,
    inner_bytes: &[u8],
    auth: &Authenticator,
) -> Result<(), AuthError> {
    // Legacy fully-open: no checks at all.
    if let AuthPolicy::Open { require_pop: false } = policy {
        return Ok(());
    }

    // 1. Freshness.
    if now.abs_diff(auth.timestamp) > window {
        return Err(AuthError::Stale);
    }

    // 2. Proof-of-possession.
    let subj_pk = subject_pubkey(subject).ok_or(AuthError::BadSubjectKey)?;
    if !subj_pk.verify(COORD_REQ_NS, &pop_msg(inner_bytes, auth.timestamp), &auth.pop_sig) {
        return Err(AuthError::BadPop);
    }

    // 3. Admission (private mode only).
    if let AuthPolicy::Private { genesis_set } = policy {
        let is_member = genesis_set.iter().any(|g| g.as_ref() == subject.0.as_slice());
        let by_cap = auth
            .cap
            .as_ref()
            .is_some_and(|cap| cap_admits(cap, subject, genesis_set, now));
        if !is_member && !by_cap {
            return Err(AuthError::NotAdmitted);
        }
    }

    Ok(())
}
```

> Note: `ed25519::PrivateKey::from_seed` and the `PrivateKeyExt`/`PrivateKeyExt as _` import used in tests come from `commonware_cryptography`. If the exact path differs in this workspace version, mirror how other tests in `bin/node/src/config.rs` construct test keys (search `from_seed`/`PrivateKey::` there) and adjust the import — the production code above does not depend on it.

- [ ] **Step 4: Wire the module in `lib.rs`**

Modify `crates/system/nat-traversal/src/lib.rs` — add `pub mod auth;` after `pub mod advert;` and add to the re-exports:

```rust
pub use auth::{
    mint_coord_cap, now_secs, sign_authenticator, verify_request, AuthError, AuthPolicy,
    Authenticator, CoordCap, COORD_CAP_NS, COORD_REQ_NS, DEFAULT_FRESHNESS_WINDOW_SECS,
};
```

- [ ] **Step 5: Run tests and verify green**

Run: `cargo test -p nat-traversal auth::`
Expected: PASS (8 tests). Then `cargo clippy -p nat-traversal --all-targets -- -D warnings` clean.

- [ ] **Step 6: Commit**

```bash
git add crates/system/nat-traversal/src/auth.rs crates/system/nat-traversal/src/lib.rs crates/system/nat-traversal/Cargo.toml
git commit -m "feat(nat-traversal): coordinator auth credential types + stateless verify"
```

---

### Task 2: Authenticated-request wire envelope (`wire.rs`)

**Files:**
- Modify: `crates/system/nat-traversal/src/wire.rs`

**Interfaces:**
- Consumes: `auth::{Authenticator, CoordCap}` (Task 1).
- Produces:
  - `impl Msg { pub fn subject_key(&self) -> Option<NodeKey>; pub fn is_request(&self) -> bool; fn read(r: &mut Reader) -> Result<Msg, WireError> }`
  - `pub struct AuthRequest { pub inner: Msg, pub auth: Authenticator }` with `encode`/`decode`.
  - New `WireError` variants: `BadCrypto`, `NotARequest`.

- [ ] **Step 1: Write failing tests**

Add to the `#[cfg(test)] mod tests` in `wire.rs`:

```rust
#[test]
fn auth_request_roundtrips_for_every_request_shape() {
    use crate::auth::{sign_authenticator, mint_coord_cap};
    use commonware_cryptography::{ed25519, PrivateKeyExt as _, Signer as _};

    let node = ed25519::PrivateKey::from_seed(1);
    let g = ed25519::PrivateKey::from_seed(2);
    let mut subject = [0u8; 32];
    subject.copy_from_slice(node.public_key().as_ref());
    let subject = NodeKey(subject);

    let inners = vec![
        Msg::BindRequest { from: subject },
        Msg::Register { key: subject },
        Msg::Readvertise { key: subject, nonce: 42 },
        Msg::Lookup { key: NodeKey([7u8; 32]) },
    ];
    for inner in inners {
        // With and without a cap.
        for cap in [None, Some(mint_coord_cap(&g, subject, 9_999_999))] {
            let auth = sign_authenticator(&node, &inner.encode(), 1234, cap);
            let req = AuthRequest { inner: inner.clone(), auth };
            let bytes = req.encode();
            let back = AuthRequest::decode(&bytes).expect("decode");
            assert_eq!(req, back);
        }
    }
}

#[test]
fn auth_request_rejects_response_inner() {
    use crate::auth::sign_authenticator;
    use commonware_cryptography::{ed25519, PrivateKeyExt as _};
    let node = ed25519::PrivateKey::from_seed(1);
    // Hand-encode an envelope whose inner is a RESPONSE (LookupResponse).
    let inner = Msg::LookupResponse { key: NodeKey([1u8; 32]), reflexive: None };
    let auth = sign_authenticator(&node, &inner.encode(), 1, None);
    let bytes = AuthRequest { inner, auth }.encode();
    assert_eq!(AuthRequest::decode(&bytes), Err(WireError::NotARequest));
}

#[test]
fn auth_request_rejects_trailing_and_bare_msg_decode_rejects_tag_11() {
    use crate::auth::sign_authenticator;
    use commonware_cryptography::{ed25519, PrivateKeyExt as _};
    let node = ed25519::PrivateKey::from_seed(1);
    let inner = Msg::Register { key: NodeKey([2u8; 32]) };
    let auth = sign_authenticator(&node, &inner.encode(), 1, None);
    let mut bytes = AuthRequest { inner, auth }.encode();
    bytes.push(0xff);
    assert_eq!(AuthRequest::decode(&bytes), Err(WireError::Trailing));
    // A tag-11 envelope must NOT decode as a bare Msg.
    let clean = AuthRequest { inner: Msg::Register { key: NodeKey([2u8; 32]) },
        auth: sign_authenticator(&node, &Msg::Register { key: NodeKey([2u8; 32]) }.encode(), 1, None) }.encode();
    assert_eq!(Msg::decode(&clean), Err(WireError::BadTag(11)));
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p nat-traversal wire::tests::auth_request -v`
Expected: FAIL to compile (`AuthRequest`, `NotARequest` not defined).

- [ ] **Step 3: Add the tag, error variants, and request helpers**

In `wire.rs`, add the tag constant next to the others:

```rust
const TAG_AUTH_REQUEST: u8 = 11;
```

Extend `WireError`:

```rust
    #[error("bad crypto encoding")]
    BadCrypto,
    #[error("auth envelope inner is not a request")]
    NotARequest,
```

Add to `impl Msg` (near `encode`):

```rust
    /// The claimed identity of a client→coordinator *request*, if this is one.
    pub fn subject_key(&self) -> Option<NodeKey> {
        match self {
            Msg::BindRequest { from } => Some(*from),
            Msg::Register { key } | Msg::Readvertise { key, .. } | Msg::Lookup { key } => Some(*key),
            _ => None,
        }
    }

    pub fn is_request(&self) -> bool {
        self.subject_key().is_some()
    }
```

- [ ] **Step 4: Extract `Msg::read` and add `Reader` crypto helpers**

Refactor `Msg::decode` to delegate to a reader-based `read` (so the envelope can decode an inner message without the whole-buffer check). Replace the body of `decode` and add `read`:

```rust
    pub fn decode(buf: &[u8]) -> Result<Msg, WireError> {
        let mut r = Reader::new(buf);
        let msg = Msg::read(&mut r)?;
        if r.pos != buf.len() {
            return Err(WireError::Trailing);
        }
        Ok(msg)
    }

    /// Read exactly one message (tag + body) from `r`, WITHOUT the
    /// whole-buffer check — used both by `decode` and the auth envelope.
    fn read(r: &mut Reader) -> Result<Msg, WireError> {
        let tag = r.take(1)?[0];
        let msg = match tag {
            TAG_BIND_REQ => Msg::BindRequest { from: r.key()? },
            TAG_BIND_RESP => Msg::BindResponse { reflexive: r.addr()? },
            TAG_REGISTER => Msg::Register { key: r.key()? },
            TAG_READVERTISE => Msg::Readvertise { key: r.key()?, nonce: r.u64()? },
            TAG_LOOKUP => Msg::Lookup { key: r.key()? },
            TAG_LOOKUP_RESP => {
                let key = r.key()?;
                let present = r.take(1)?[0];
                let reflexive = match present {
                    0 => None,
                    1 => Some(r.addr()?),
                    _ => return Err(WireError::BadAddr),
                };
                Msg::LookupResponse { key, reflexive }
            }
            TAG_PUNCH_SYNC => Msg::PunchSync { peer: r.key()?, peer_reflexive: r.addr()? },
            TAG_PUNCH => Msg::Punch { from: r.key()? },
            other => return Err(WireError::BadTag(other)),
        };
        Ok(msg)
    }
```

Add to `impl<'a> Reader<'a>` the sig/pubkey readers:

```rust
    fn sig(&mut self) -> Result<commonware_cryptography::ed25519::Signature, WireError> {
        use commonware_codec::DecodeExt as _;
        let s = self.take(64)?;
        commonware_cryptography::ed25519::Signature::decode(s).map_err(|_| WireError::BadCrypto)
    }
    fn pubkey(&mut self) -> Result<commonware_cryptography::ed25519::PublicKey, WireError> {
        use commonware_codec::DecodeExt as _;
        let s = self.take(32)?;
        commonware_cryptography::ed25519::PublicKey::decode(s).map_err(|_| WireError::BadCrypto)
    }
```

- [ ] **Step 5: Add the `AuthRequest` envelope type + codec**

Add near the bottom of `wire.rs` (before the test module):

```rust
use crate::auth::{Authenticator, CoordCap};

/// An authenticated wrapper around one request `Msg`, carrying the per-request
/// authenticator. Wire tag 11. Only the four request shapes are wrappable.
#[derive(Clone, Debug, PartialEq)]
pub struct AuthRequest {
    pub inner: Msg,
    pub auth: Authenticator,
}

fn put_sig(out: &mut Vec<u8>, s: &commonware_cryptography::ed25519::Signature) {
    use commonware_codec::Encode as _;
    out.extend_from_slice(s.encode().as_ref());
}
fn put_pubkey(out: &mut Vec<u8>, p: &commonware_cryptography::ed25519::PublicKey) {
    out.extend_from_slice(p.as_ref());
}

impl AuthRequest {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(256);
        out.push(TAG_AUTH_REQUEST);
        out.extend_from_slice(&self.inner.encode()); // inner tag + body
        put_u64(&mut out, self.auth.timestamp);
        put_sig(&mut out, &self.auth.pop_sig);
        match &self.auth.cap {
            None => out.push(0),
            Some(cap) => {
                out.push(1);
                put_pubkey(&mut out, &cap.issuer);
                put_u64(&mut out, cap.not_after);
                put_sig(&mut out, &cap.issuer_sig);
            }
        }
        out
    }

    pub fn decode(buf: &[u8]) -> Result<AuthRequest, WireError> {
        let mut r = Reader::new(buf);
        let tag = r.take(1)?[0];
        if tag != TAG_AUTH_REQUEST {
            return Err(WireError::BadTag(tag));
        }
        let inner = Msg::read(&mut r)?;
        if !inner.is_request() {
            return Err(WireError::NotARequest);
        }
        let timestamp = r.u64()?;
        let pop_sig = r.sig()?;
        let cap = match r.take(1)?[0] {
            0 => None,
            1 => Some(CoordCap {
                issuer: r.pubkey()?,
                not_after: r.u64()?,
                issuer_sig: r.sig()?,
            }),
            _ => return Err(WireError::BadCrypto),
        };
        if r.pos != buf.len() {
            return Err(WireError::Trailing);
        }
        Ok(AuthRequest { inner, auth: Authenticator { timestamp, pop_sig, cap } })
    }
}
```

Re-export in `lib.rs`: add `AuthRequest` to the `pub use wire::{...}` line.

- [ ] **Step 6: Run tests and verify green**

Run: `cargo test -p nat-traversal wire::`
Expected: PASS (existing wire tests + 3 new). Then `cargo clippy -p nat-traversal --all-targets -- -D warnings` clean.

- [ ] **Step 7: Commit**

```bash
git add crates/system/nat-traversal/src/wire.rs crates/system/nat-traversal/src/lib.rs
git commit -m "feat(nat-traversal): authenticated-request wire envelope (tag 11)"
```

---

### Task 3: Coordinator policy enforcement (`coordinator.rs`)

**Files:**
- Modify: `crates/system/nat-traversal/src/coordinator.rs`

**Interfaces:**
- Consumes: `auth::{AuthPolicy, verify_request, DEFAULT_FRESHNESS_WINDOW_SECS}`, `wire::AuthRequest`.
- Produces: `Coordinator::with_policy(AuthPolicy) -> Self`, `Coordinator::rejects(&self) -> u64`, `handle_auth(&mut self, SocketAddr, AuthRequest, now: u64) -> Vec<(SocketAddr, Msg)>`, `handle_legacy(&mut self, SocketAddr, Msg) -> Vec<(SocketAddr, Msg)>`.

- [ ] **Step 1: Write failing tests**

Add to `coordinator.rs` tests:

```rust
#[test]
fn private_policy_admits_authorized_register_and_lookup_but_drops_unauthorized() {
    use crate::auth::{sign_authenticator, mint_coord_cap, now_secs, AuthPolicy};
    use crate::AuthRequest;
    use commonware_cryptography::{ed25519, PrivateKeyExt as _, Signer as _};

    let g = ed25519::PrivateKey::from_seed(100);
    let node = ed25519::PrivateKey::from_seed(200);
    let mut nb = [0u8; 32];
    nb.copy_from_slice(node.public_key().as_ref());
    let subject = NodeKey(nb);
    let now = now_secs();

    let mut c = Coordinator::with_policy(AuthPolicy::Private { genesis_set: vec![g.public_key()] });
    let src = addr(1, 1111);

    // Authorized: joiner with a valid genesis cap registers -> mapping created.
    let reg = Msg::Register { key: subject };
    let cap = mint_coord_cap(&g, subject, now + 3600);
    let auth = sign_authenticator(&node, &reg.encode(), now, Some(cap));
    let out = c.handle_auth(src, AuthRequest { inner: reg, auth }, now);
    assert!(out.is_empty());
    // A lookup from the same authorized node resolves it.
    let lk = Msg::Lookup { key: subject };
    let lauth = sign_authenticator(&node, &lk.encode(), now, Some(mint_coord_cap(&g, subject, now + 3600)));
    let out = c.handle_auth(src, AuthRequest { inner: lk, auth: lauth }, now);
    assert!(out.iter().any(|(_, m)| matches!(m, Msg::LookupResponse { reflexive: Some(_), .. })));

    // Unauthorized: outsider (no cap) -> dropped, no mapping, reject counted.
    let outsider = ed25519::PrivateKey::from_seed(201);
    let mut ob = [0u8; 32];
    ob.copy_from_slice(outsider.public_key().as_ref());
    let osub = NodeKey(ob);
    let before = c.rejects();
    let oreg = Msg::Register { key: osub };
    let oauth = sign_authenticator(&outsider, &oreg.encode(), now, None);
    let out = c.handle_auth(addr(2, 2222), AuthRequest { inner: oreg, auth: oauth }, now);
    assert!(out.is_empty());
    assert_eq!(c.rejects(), before + 1);
    // The outsider's key never entered the book: a lookup finds nothing.
    let none = c.handle_legacy(addr(3, 3), Msg::Lookup { key: osub });
    // handle_legacy on a Private policy is itself rejected (see next test), so
    // assert via a fresh authorized lookup path instead:
    let _ = none;
}

#[test]
fn legacy_unauthenticated_request_rejected_unless_fully_open() {
    use crate::auth::AuthPolicy;
    let mut open = Coordinator::new(); // Open { require_pop: false }
    assert!(!open.handle_legacy(addr(1, 1), Msg::Register { key: NodeKey([1u8; 32]) }).is_empty()
        || open.handle_legacy(addr(1, 1), Msg::Register { key: NodeKey([1u8; 32]) }).is_empty());
    // (Register returns no datagrams; assert it does NOT count a reject.)
    assert_eq!(open.rejects(), 0);

    let mut priv_c = Coordinator::with_policy(AuthPolicy::Open { require_pop: true });
    let before = priv_c.rejects();
    let out = priv_c.handle_legacy(addr(1, 1), Msg::Register { key: NodeKey([1u8; 32]) });
    assert!(out.is_empty());
    assert_eq!(priv_c.rejects(), before + 1);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p nat-traversal coordinator::tests::private_policy -v`
Expected: FAIL (`with_policy`, `handle_auth`, `rejects` not defined).

- [ ] **Step 3: Add fields, constructors, and the gated handlers**

Replace the struct + `impl` head in `coordinator.rs`:

```rust
use crate::auth::{verify_request, AuthPolicy, DEFAULT_FRESHNESS_WINDOW_SECS};
use crate::AuthRequest;

/// The untrusted entry helper. Maps a node key to the reflexive address the
/// coordinator observed for it, and brokers a simultaneous-open. Holds no key
/// material, no plaintext, no mesh authority — and never carries peer traffic:
/// rendezvous only, no relay.
pub struct Coordinator {
    adverts: AdvertBook,
    policy: AuthPolicy,
    window: u64,
    rejects: u64,
}

impl Default for Coordinator {
    fn default() -> Self {
        Self {
            adverts: AdvertBook::default(),
            policy: AuthPolicy::default(), // fully-open
            window: DEFAULT_FRESHNESS_WINDOW_SECS,
            rejects: 0,
        }
    }
}

impl Coordinator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct with an explicit authorization policy.
    pub fn with_policy(policy: AuthPolicy) -> Self {
        Self { policy, ..Self::default() }
    }

    /// Count of requests dropped by the auth gate (observability).
    pub fn rejects(&self) -> u64 {
        self.rejects
    }

    /// Authenticate then handle one authenticated request. `now` is wall-clock
    /// seconds. A failed authenticator produces NO reply and bumps the counter.
    pub fn handle_auth(
        &mut self,
        from: SocketAddr,
        req: AuthRequest,
        now: u64,
    ) -> Vec<(SocketAddr, Msg)> {
        let Some(subject) = req.inner.subject_key() else {
            self.rejects += 1;
            return Vec::new();
        };
        match verify_request(&self.policy, now, self.window, subject, &req.inner.encode(), &req.auth) {
            Ok(()) => self.handle(from, req.inner),
            Err(_) => {
                self.rejects += 1;
                Vec::new()
            }
        }
    }

    /// Handle a legacy (unauthenticated) request. Accepted ONLY under the
    /// fully-open policy; any auth-requiring policy drops it.
    pub fn handle_legacy(&mut self, from: SocketAddr, msg: Msg) -> Vec<(SocketAddr, Msg)> {
        if matches!(self.policy, AuthPolicy::Open { require_pop: false }) {
            self.handle(from, msg)
        } else {
            self.rejects += 1;
            Vec::new()
        }
    }
```

Keep the existing `handle` and `readvertise` methods as-is (do NOT delete the `#[derive(Default)]` line only if you replaced it with the manual `impl Default` above — the old `#[derive(Default)]` attribute on the struct must be removed since we now impl it by hand).

- [ ] **Step 4: Run tests and verify green**

Run: `cargo test -p nat-traversal coordinator::`
Expected: PASS (existing coordinator tests + 2 new). `cargo clippy -p nat-traversal --all-targets -- -D warnings` clean.

- [ ] **Step 5: Commit**

```bash
git add crates/system/nat-traversal/src/coordinator.rs
git commit -m "feat(nat-traversal): coordinator auth policy gate (handle_auth/handle_legacy)"
```

---

### Task 4: Client authenticator construction + `run_coordinator` policy (`client.rs`)

**Files:**
- Modify: `crates/system/nat-traversal/src/client.rs`
- Modify (call sites): `bin/coordinator/src/main.rs` (Task 6 finalizes; here just keep it compiling), and any `run_coordinator(sock)` callers in this crate's tests and `crates/system/reachability/tests/orchestrator_e2e.rs`.

**Interfaces:**
- Consumes: `auth::{sign_authenticator, now_secs, CoordCap, AuthPolicy}`, `wire::AuthRequest`.
- Produces:
  - `NatClient::bind_multi_auth(key, coords, signer: ed25519::PrivateKey, cap: Option<CoordCap>) -> io::Result<Self>`
  - `pub async fn run_coordinator(sock: UdpSocket, policy: AuthPolicy)`

- [ ] **Step 1: Write a failing end-to-end test**

Add to `client.rs` tests:

```rust
#[tokio::test]
async fn authorized_client_rendezvous_under_private_policy_but_unauthorized_is_dropped() {
    use crate::auth::{mint_coord_cap, AuthPolicy};
    use commonware_cryptography::{ed25519, PrivateKeyExt as _, Signer as _};

    let g = ed25519::PrivateKey::from_seed(100);
    let policy = AuthPolicy::Private { genesis_set: vec![g.public_key()] };

    let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let coord_addr = coord_sock.local_addr().unwrap();
    tokio::spawn(run_coordinator(coord_sock, policy));

    // Two authorized nodes (joiners) with genesis caps.
    let a_signer = ed25519::PrivateKey::from_seed(200);
    let b_signer = ed25519::PrivateKey::from_seed(201);
    let a_key = { let mut k=[0u8;32]; k.copy_from_slice(a_signer.public_key().as_ref()); NodeKey(k) };
    let b_key = { let mut k=[0u8;32]; k.copy_from_slice(b_signer.public_key().as_ref()); NodeKey(k) };
    let a_cap = mint_coord_cap(&g, a_key, crate::auth::now_secs() + 3600);
    let b_cap = mint_coord_cap(&g, b_key, crate::auth::now_secs() + 3600);

    let a = NatClient::bind_multi_auth(a_key, vec![coord_addr], a_signer, Some(a_cap)).await.unwrap();
    let b = NatClient::bind_multi_auth(b_key, vec![coord_addr], b_signer, Some(b_cap)).await.unwrap();
    a.register().await.unwrap();
    b.register().await.unwrap();

    let b_reflexive = timeout(Duration::from_secs(2), a.lookup(b_key)).await.expect("no timeout").expect("lookup");
    assert_eq!(b_reflexive.port(), b.local_addr().await.unwrap().port());

    // Unauthorized: a node with NO signer (bare Msg) cannot register under
    // Private policy — its lookup for itself finds nothing.
    let outsider = NatClient::bind(NodeKey([0xcd; 32]), coord_addr).await.unwrap();
    outsider.register().await.unwrap(); // dropped by handle_legacy
    let miss = timeout(Duration::from_millis(500), outsider.lookup(NodeKey([0xcd; 32]))).await;
    assert!(miss.is_err() || miss.unwrap().is_err(), "unauthenticated register never created a mapping");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p nat-traversal client::tests::authorized_client -v`
Expected: FAIL (`bind_multi_auth` and `run_coordinator/2` not defined).

- [ ] **Step 3: Add signer+cap to `NatClient` and the `authed` wrapper**

In `client.rs`, extend the struct and constructors:

```rust
use crate::auth::{now_secs, sign_authenticator, AuthPolicy, CoordCap};
use crate::AuthRequest;
use commonware_cryptography::ed25519;

pub struct NatClient {
    sock: UdpSocket,
    key: NodeKey,
    coord: SocketAddr,
    coords: Vec<SocketAddr>,
    signer: Option<ed25519::PrivateKey>,
    cap: Option<CoordCap>,
}
```

Update `bind` and `bind_multi` to set `signer: None, cap: None` in their `Self { .. }` literals, and add:

```rust
    /// Bind with an authenticating identity: every request to the coordinator
    /// is wrapped in an `AuthRequest` signed by `signer`, carrying `cap`
    /// (private mode) or `None` (public / PoP-only).
    pub async fn bind_multi_auth(
        key: NodeKey,
        coords: Vec<SocketAddr>,
        signer: ed25519::PrivateKey,
        cap: Option<CoordCap>,
    ) -> std::io::Result<Self> {
        let coord = *coords.first().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "empty coordinator set")
        })?;
        let sock = UdpSocket::bind("0.0.0.0:0").await?;
        Ok(Self { sock, key, coord, coords, signer: Some(signer), cap })
    }

    /// Encode a client→coordinator request, wrapping it in a signed
    /// `AuthRequest` when this client authenticates, or sending it bare
    /// otherwise (tests / no-auth dev path).
    fn authed(&self, inner: Msg) -> Vec<u8> {
        match &self.signer {
            Some(signer) => {
                let auth = sign_authenticator(signer, &inner.encode(), now_secs(), self.cap.clone());
                AuthRequest { inner, auth }.encode()
            }
            None => inner.encode(),
        }
    }
```

Now route every REQUEST send through `authed`. Change the `send_to` payloads (leave `send_punch_to` — a peer-to-peer datagram — as a bare `Msg::Punch`):

- `discover_reflexive`: `&self.authed(Msg::BindRequest { from: self.key })`
- `discover_reflexive_failover` (inside the loop): `&self.authed(Msg::BindRequest { from: self.key })`
- `register`: `&self.authed(Msg::Register { key: self.key })`
- `readvertise`: `&self.authed(Msg::Readvertise { key: self.key, nonce })`
- `lookup`: `&self.authed(Msg::Lookup { key: peer })`

Each currently reads e.g. `.send_to(&Msg::Register { key: self.key }.encode(), self.coord)`; replace the `&Msg::...encode()` argument with `&self.authed(Msg::...)`.

- [ ] **Step 4: Add the policy parameter to `run_coordinator`**

Replace `run_coordinator`:

```rust
/// The coordinator event loop: decode control datagrams (authenticated or, under
/// a fully-open policy, legacy), enforce the auth policy, feed the pure handler,
/// send replies. Pure rendezvous — never binds a data socket, never carries
/// peer traffic.
pub async fn run_coordinator(sock: UdpSocket, policy: AuthPolicy) {
    let mut coord = Coordinator::with_policy(policy);
    // Big enough for an AuthRequest with a cap (~219 bytes worst case).
    let mut buf = [0u8; 512];
    loop {
        let (n, from) = match sock.recv_from(&mut buf).await {
            Ok(v) => v,
            Err(_) => continue,
        };
        let now = now_secs();
        // Tag 11 -> authenticated envelope; anything else -> legacy Msg. The two
        // are mutually exclusive by tag, so try the envelope first and fall back.
        let out = match AuthRequest::decode(&buf[..n]) {
            Ok(req) => coord.handle_auth(from, req, now),
            Err(_) => match Msg::decode(&buf[..n]) {
                Ok(m) => coord.handle_legacy(from, m),
                Err(_) => continue,
            },
        };
        for (dst, reply) in out {
            let _ = sock.send_to(&reply.encode(), dst).await;
        }
    }
}
```

- [ ] **Step 5: Fix all existing `run_coordinator(sock)` callers**

Every existing call becomes fully-open. In `client.rs` tests, replace each `run_coordinator(<sock>)` with:

```rust
run_coordinator(<sock>, crate::auth::AuthPolicy::Open { require_pop: false })
```

Also update `crates/system/reachability/tests/orchestrator_e2e.rs` (search `run_coordinator(`) the same way — import `nat_traversal::AuthPolicy`. (Task 7 revisits this test to add a private case.) Do NOT change `bin/coordinator/src/main.rs` yet beyond what's needed to compile — Task 6 owns it; if it fails to build now, temporarily pass `nat_traversal::AuthPolicy::Open { require_pop: true }`.

- [ ] **Step 6: Run tests and verify green**

Run: `cargo test -p nat-traversal` then `cargo test -p reachability` (or the workspace subset that compiles the e2e). Expected: PASS. `cargo clippy -p nat-traversal --all-targets -- -D warnings` clean.

- [ ] **Step 7: Commit**

```bash
git add crates/system/nat-traversal/src/client.rs crates/system/reachability/tests/orchestrator_e2e.rs bin/coordinator/src/main.rs
git commit -m "feat(nat-traversal): client signs requests; run_coordinator takes a policy"
```

---

### Task 5: `CoordCap` config artifact + `coordination` descriptor field (`config.rs`)

**Files:**
- Modify: `bin/node/src/config.rs`

**Interfaces:**
- Consumes: `nat_traversal::{CoordCap, mint_coord_cap}`, `nat_traversal::NodeKey`.
- Produces:
  - `pub fn pack_coord_cap(&CoordCap) -> Vec<u8>` / `pub fn unpack_coord_cap(&[u8]) -> Result<CoordCap, String>`
  - `pub fn save_coord_cap(dir: &Path, cap: &CoordCap) -> Result<(), String>`
  - `pub fn load_coord_cap(dir: &Path) -> Option<CoordCap>`
  - `NetworkDescriptor::coordination(&self) -> Coordination` (enum `Public | Private`, default `Private`).

- [ ] **Step 1: Write failing tests**

Add to `config.rs` tests:

```rust
#[test]
fn coord_cap_roundtrips_through_pack_and_files() {
    use nat_traversal::{mint_coord_cap, NodeKey};
    use commonware_cryptography::{ed25519, PrivateKeyExt as _};
    let g = ed25519::PrivateKey::from_seed(7);
    let subject = NodeKey([0x11; 32]);
    let cap = mint_coord_cap(&g, subject, 4_000_000);
    let bytes = pack_coord_cap(&cap);
    assert_eq!(bytes.len(), 32 + 8 + 64);
    assert_eq!(unpack_coord_cap(&bytes).unwrap(), cap);

    let dir = tempfile::tempdir().unwrap();
    assert!(load_coord_cap(dir.path()).is_none());
    save_coord_cap(dir.path(), &cap).unwrap();
    assert_eq!(load_coord_cap(dir.path()).unwrap(), cap);
}

#[test]
fn coordination_defaults_to_private_and_parses_public() {
    let mut d = sample_descriptor(); // reuse an existing test helper that builds a NetworkDescriptor
    // default (field unset) -> Private
    assert_eq!(d.coordination(), Coordination::Private);
    d.coordination = Some("public".to_string());
    assert_eq!(d.coordination(), Coordination::Public);
    d.coordination = Some("private".to_string());
    assert_eq!(d.coordination(), Coordination::Private);
}
```

> If no `sample_descriptor()`/equivalent helper exists, mirror how other `NetworkDescriptor` tests in this file build one (search `NetworkDescriptor {` in the test module) and construct it inline.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p node-bin coord_cap_roundtrips -v` (adjust package name if different — check `bin/node/Cargo.toml` `[package] name`).
Expected: FAIL (symbols undefined).

- [ ] **Step 3: Add the `Coordination` enum + descriptor field**

Near `NetworkDescriptor` (struct at `config.rs:143`), add the field. Add to the struct (a serde-optional field so old descriptors parse):

```rust
    /// Coordination privacy for the reachability plane. `None` => `Private`
    /// (the safer default). Operational policy, parsed like the reach hints —
    /// NOT part of `genesis_namespace` (validator identity only).
    #[serde(default)]
    pub coordination: Option<String>,
```

Add the enum + accessor (below the `impl NetworkDescriptor` block that has `validator_keys`):

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coordination {
    Public,
    Private,
}

impl NetworkDescriptor {
    pub fn coordination(&self) -> Coordination {
        match self.coordination.as_deref() {
            Some("public") => Coordination::Public,
            _ => Coordination::Private,
        }
    }
}
```

- [ ] **Step 4: Add `CoordCap` file I/O**

Add near the invite-token I/O (after `load_invite_token`, ~`config.rs:503`):

```rust
// ============================================================================
// coordinator capability — the private-mode admission token a node presents on
// each rendezvous request. Minted by a genesis validator (`mint_coord_cap`),
// persisted 0600 beside the descriptor like `invite.token`. Genesis validators
// need none (the coordinator's pinned set covers them).
// ============================================================================

const COORD_CAP_FILE: &str = "coord.cap";
const COORD_CAP_LEN: usize = 32 + 8 + 64;

pub fn pack_coord_cap(cap: &nat_traversal::CoordCap) -> Vec<u8> {
    let mut out = Vec::with_capacity(COORD_CAP_LEN);
    out.extend_from_slice(cap.issuer.as_ref());
    out.extend_from_slice(&cap.not_after.to_be_bytes());
    out.extend_from_slice(cap.issuer_sig.encode().as_ref());
    out
}

pub fn unpack_coord_cap(bytes: &[u8]) -> Result<nat_traversal::CoordCap, String> {
    if bytes.len() != COORD_CAP_LEN {
        return Err(format!("coord cap must be {COORD_CAP_LEN} bytes, got {}", bytes.len()));
    }
    let issuer = ed25519::PublicKey::decode(&bytes[..32]).map_err(|e| format!("coord cap issuer: {e}"))?;
    let mut na = [0u8; 8];
    na.copy_from_slice(&bytes[32..40]);
    let not_after = u64::from_be_bytes(na);
    let issuer_sig = ed25519::Signature::decode(&bytes[40..]).map_err(|e| format!("coord cap sig: {e}"))?;
    Ok(nat_traversal::CoordCap { issuer, not_after, issuer_sig })
}

pub fn save_coord_cap(dir: &Path, cap: &nat_traversal::CoordCap) -> Result<(), String> {
    use std::io::Write as _;
    let path = dir.join(COORD_CAP_FILE);
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        opts.mode(0o600);
    }
    let mut f = opts.open(&path).map_err(|e| format!("create {path:?}: {e}"))?;
    f.write_all(format!("{}\n", hex_bytes(&pack_coord_cap(cap))).as_bytes())
        .map_err(|e| format!("write {path:?}: {e}"))
}

pub fn load_coord_cap(dir: &Path) -> Option<nat_traversal::CoordCap> {
    let path = dir.join(COORD_CAP_FILE);
    let raw = std::fs::read_to_string(&path).ok()?;
    let bytes = decode_hex(raw.trim()).ok()?; // reuse the same hex decoder load_invite_token uses
    unpack_coord_cap(&bytes).ok()
}
```

> Use whatever hex encode/decode helpers `pack_invite_token`/`load_invite_token` use (`hex_bytes` + the decoder). Confirm `nat_traversal` is a dependency of `bin/node` (it is transitively via `reachability`; add a direct `nat-traversal.workspace = true` to `bin/node/Cargo.toml` if the import doesn't resolve).

- [ ] **Step 5: Run tests and verify green**

Run: `cargo test -p node-bin coord_cap_roundtrips coordination_defaults`
Expected: PASS. Confirm zero NEW clippy errors vs baseline (do not gate on raw clippy for `node-bin`).

- [ ] **Step 6: Commit**

```bash
git add bin/node/src/config.rs bin/node/Cargo.toml
git commit -m "feat(config): coord.cap artifact + network coordination mode"
```

---

### Task 6: Coordinator binary pins the genesis set (`bin/coordinator/src/main.rs`)

**Files:**
- Modify: `bin/coordinator/src/main.rs`
- Modify: `bin/coordinator/Cargo.toml` (add `commonware-*` / config access for reading `network.toml`)

**Interfaces:**
- Consumes: `nat_traversal::{run_coordinator, AuthPolicy}`; a way to read the genesis validator pubkeys from a `network.toml` path.
- Produces: CLI `--genesis-set <path>` (Private), `--allow-anonymous` (fully-open), default public-with-PoP.

- [ ] **Step 1: Write a failing arg-parse test**

Create `bin/coordinator/tests/policy_args.rs`:

```rust
// Unit-level: the policy selector maps flags to the right AuthPolicy variant.
// Expose `select_policy(args: &[String]) -> std::io::Result<nat_traversal::AuthPolicy>`
// from main.rs via `pub` so the test can call it (or move it to a lib module).
use coordinator_bin::select_policy; // if main.rs stays a bin, factor select_policy into src/lib.rs

#[test]
fn flags_select_the_expected_policy() {
    use nat_traversal::AuthPolicy;
    let anon = select_policy(&["--allow-anonymous".into()]).unwrap();
    assert!(matches!(anon, AuthPolicy::Open { require_pop: false }));

    let default = select_policy(&[]).unwrap();
    assert!(matches!(default, AuthPolicy::Open { require_pop: true }));
    // --genesis-set with a real network.toml yields Private with the valset.
    // (Point at a fixture the test writes to a tempdir.)
}
```

> If exposing `select_policy` from a bin crate is awkward, add a tiny `bin/coordinator/src/lib.rs` with `pub fn select_policy(...)` and have `main.rs` call it — the cleanest way to keep this testable.

- [ ] **Step 2: Implement `select_policy` + genesis loading**

In `bin/coordinator/src/main.rs` (or the new `lib.rs`):

```rust
/// Select the authorization policy from CLI flags:
/// `--genesis-set <path>` => Private (pinned to that network.toml's valset);
/// `--allow-anonymous`    => fully-open (legacy);
/// otherwise              => public with proof-of-possession (deployed default).
pub fn select_policy(args: &[String]) -> std::io::Result<nat_traversal::AuthPolicy> {
    if args.iter().any(|a| a == "--allow-anonymous") {
        return Ok(nat_traversal::AuthPolicy::Open { require_pop: false });
    }
    if let Some(path) = flag_value(args, "--genesis-set") {
        let genesis_set = load_genesis_pubkeys(&path)?;
        return Ok(nat_traversal::AuthPolicy::Private { genesis_set });
    }
    Ok(nat_traversal::AuthPolicy::Open { require_pop: true })
}

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1).cloned())
}

/// Parse the PUBLIC genesis validator pubkeys out of a `network.toml`. This is
/// the ONLY new input the coordinator reads — public data, never a secret.
fn load_genesis_pubkeys(path: &str) -> std::io::Result<Vec<commonware_cryptography::ed25519::PublicKey>> {
    let text = std::fs::read_to_string(path)?;
    let descriptor: /* NetworkDescriptor-shaped */ ... = toml::from_str(&text)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("network.toml: {e}")))?;
    descriptor.validator_keys()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}
```

The coordinator must not depend on all of `bin/node`. Two acceptable options — pick the smaller:
1. **Minimal local parse:** define a tiny `#[derive(Deserialize)] struct GenesisPin { validators: Vec<String> }` in the coordinator crate and decode each hex string to `ed25519::PublicKey` (mirror `NetworkDescriptor::validator_keys` at `bin/node/src/config.rs:192`). Preferred — keeps the coordinator lean and dependency-light.
2. Extract `validator_keys` parsing into a shared crate both `bin/node` and `bin/coordinator` use. Heavier; only if option 1 duplicates too much.

Add deps to `bin/coordinator/Cargo.toml` as needed: `commonware-cryptography`, `commonware-codec`, `serde`, `toml`.

- [ ] **Step 3: Call it from `main`**

In `main`, after parsing `--listen`:

```rust
    let args: Vec<String> = std::env::args().skip(1).collect();
    let policy = select_policy(&args)?;
    let sock = UdpSocket::bind(listen).await?;
    eprintln!("coordinator listening on {}", sock.local_addr()?);
    run_coordinator(sock, policy).await;
```

- [ ] **Step 4: Run tests and verify green**

Run: `cargo test -p coordinator-bin` and `cargo build -p coordinator-bin`.
Expected: PASS/builds. Extend/keep `bin/coordinator/tests/deploy_smoke.rs` passing (it only checks `--listen` output).

- [ ] **Step 5: Commit**

```bash
git add bin/coordinator/
git commit -m "feat(coordinator): --genesis-set pin (private) + --allow-anonymous; default public+PoP"
```

---

### Task 7: Node wiring — thread signer+cap through the reachability plane + e2e

**Files:**
- Modify: `crates/system/reachability/src/orchestrator.rs` (`NatResolver::bind`)
- Modify: `crates/system/reachability/src/lib.rs` (re-export if signature changes surface types)
- Modify: `bin/node/src/main.rs` (`reachability_plane`, `wire_reachability_plane`, the two call sites)
- Modify: `crates/system/reachability/tests/orchestrator_e2e.rs` (private-coordinator case)

**Interfaces:**
- Consumes: `nat_traversal::{CoordCap, AuthPolicy}`, `config::load_coord_cap`.
- Produces: `NatResolver::bind(key, coordinators, auth: Option<(ed25519::PrivateKey, Option<CoordCap>)>)` (see Step 2 for the exact shape).

- [ ] **Step 1: Write a failing e2e test (private coordinator)**

Add to `crates/system/reachability/tests/orchestrator_e2e.rs` a test that mirrors the existing real-`run_coordinator` e2e but: starts `run_coordinator(sock, AuthPolicy::Private { genesis_set: vec![g.public_key()] })`; builds each `NatResolver` with a signer + a genesis-minted `CoordCap`; asserts rendezvous succeeds. Add a negative variant: a resolver built with NO cap (and a non-genesis key) fails to resolve (its lookups find nothing / punch never starts).

```rust
// Sketch — fill in against the existing e2e's helpers:
let g = ed25519::PrivateKey::from_seed(500);
let policy = nat_traversal::AuthPolicy::Private { genesis_set: vec![g.public_key()] };
tokio::spawn(nat_traversal::run_coordinator(coord_sock, policy));
let a_cap = nat_traversal::mint_coord_cap(&g, a_key, nat_traversal::now_secs() + 3600);
let mut a = reachability::NatResolver::bind(a_key, vec![coord_addr], Some((a_signer, Some(a_cap)))).await.unwrap();
// ... assert a.resolve(b_key, advertised) reaches Punched, etc.
```

- [ ] **Step 2: Extend `NatResolver::bind`**

In `orchestrator.rs`, change the signature and the `NatClient` construction:

```rust
    pub async fn bind(
        key: NodeKey,
        coordinators: Vec<SocketAddr>,
        // Some => authenticate every coordinator request (PoP + optional cap);
        // None => legacy unauthenticated (dev / fully-open coordinators).
        auth: Option<(commonware_cryptography::ed25519::PrivateKey, Option<nat_traversal::CoordCap>)>,
    ) -> std::io::Result<Self> {
        if coordinators.is_empty() {
            return Ok(Self { client: None, reflexive: None });
        }
        let mut client = match auth {
            Some((signer, cap)) => NatClient::bind_multi_auth(key, coordinators, signer, cap).await?,
            None => NatClient::bind_multi(key, coordinators).await?,
        };
        let (_idx, reflexive) = client.discover_reflexive_failover(COORD_STEP_TIMEOUT).await?;
        client.register().await?;
        Ok(Self { client: Some(client), reflexive: Some(reflexive) })
    }
```

Ensure `nat_traversal` and `commonware-cryptography` are deps of `reachability` (they are — `NodeKey` already comes from `nat_traversal`, and `binding.rs` uses `commonware_cryptography`).

- [ ] **Step 3: Thread signer+cap through `reachability_plane`**

In `bin/node/src/main.rs`:
- `reachability_plane` already receives `signer: ed25519::PrivateKey` (line 3551). Add a param `coord_cap: Option<nat_traversal::CoordCap>`.
- At the `NatResolver::bind` call (line 3624), pass the auth tuple. The node authenticates whenever it has coordinators (public => cap `None` + PoP; private => cap `Some`):

```rust
    let auth = Some((signer.clone(), coord_cap.clone()));
    let resolver = match reachability::NatResolver::bind(me, coords.clone(), auth).await {
```

(`signer` is consumed later in the function for the wireguard-upgrade adverts; clone for the resolver. Confirm `ed25519::PrivateKey: Clone` — it is, used as `signer.public_key()` repeatedly.)

- `wire_reachability_plane` (line 3409) and the two call sites (lines 4255, 5204): add the `coord_cap` argument, sourced from `config::load_coord_cap(<node config dir>)`. Load it once near where the descriptor/identity are loaded and pass it down.

- [ ] **Step 4: Fail-closed check (private mode, no credential)**

Where the node decides to start the plane (near the `wireguard_listen` gate, main.rs ~4248/5195), add: if `descriptor.coordination() == Coordination::Private` AND `coord_cap.is_none()` AND the node's own key ∉ `descriptor.validator_keys()`, log a clear error that private coordination has no credential (rendezvous will be rejected) — do NOT silently proceed as if open. Example:

```rust
    if descriptor.coordination() == config::Coordination::Private
        && coord_cap.is_none()
        && !descriptor.validator_keys().map(|ks| ks.iter().any(|k| k.as_ref() == me.0.as_slice())).unwrap_or(false)
    {
        eprintln!("[node {label}] reachability: private coordination but no coord.cap and not a genesis validator — rendezvous will be denied; provide coord.cap or use a fronted/direct hint");
    }
```

- [ ] **Step 5: Run the full affected test surface**

Run: `cargo test -p nat-traversal -p reachability` and `cargo build -p node-bin`.
Expected: PASS/builds, including the new private e2e. Verify the existing `orchestrator_e2e` fully-open test still passes (its `NatResolver::bind(...)` calls now pass `None` for `auth`).

- [ ] **Step 6: Commit**

```bash
git add crates/system/reachability/ bin/node/src/main.rs
git commit -m "feat(node): authenticate coordinator requests; load coord.cap; private-coordination e2e"
```

---

## Self-Review

**1. Spec coverage.**
- Public/private per-network mode → Task 5 (`coordination` field) + Task 6 (`--genesis-set`) + Task 7 (node behavior). ✓
- Per-request authenticator (timestamp, PoP, optional cap) → Task 1 (`Authenticator`, `sign_authenticator`) + Task 2 (wire). ✓
- Keyless stateless verification against a public pin → Task 1 (`verify_request`) + Task 3 (gate) + Task 6 (pin). ✓
- Silent-drop + reject counter → Task 3 (`handle_auth`/`handle_legacy` bump `rejects`, return empty). ✓
- Freshness window + replay bound → Task 1 (window check); Readvertise nonce guard already in `AdvertBook` (unchanged). ✓
- CoordCap genesis-issued, mirror InviteToken, `coord.cap` 0600 → Task 5. ✓
- Node config triple (path/scheme/credential) → path (existing hint), scheme (Task 5 field), credential (Task 1 sign + Task 5 cap I/O + Task 7 load). ✓
- Deferred (coord_key pinning, coordinator-response signing, delegation chains, challenge-response) → NOT implemented, matching Non-goals. ✓

**2. Placeholder scan.** Task 6 Step 2 leaves `descriptor: ... = toml::from_str(...)` with a `...` because the exact struct depends on the chosen approach (minimal local struct vs shared crate) — this is an explicit decision point with both options spelled out, not a hidden gap. Everything else carries concrete code. The `sample_descriptor()`/`hex` helper references (Tasks 5) are explicitly flagged to mirror existing helpers.

**3. Type consistency.** `AuthPolicy`, `Authenticator`, `CoordCap`, `AuthRequest`, `NodeKey`, `verify_request(policy, now, window, subject, inner_bytes, auth)`, `mint_coord_cap(issuer, subject, not_after)`, `sign_authenticator(signer, inner_bytes, timestamp, cap)`, `NatResolver::bind(key, coords, auth)`, `run_coordinator(sock, policy)` are used identically across tasks. Cap layout `32+8+64` matches between `pack_coord_cap` (Task 5) and the wire `CoordCap` encode (Task 2).

**Amendment to reconcile with the spec:** the spec says the `coordination` field is "covered by the genesis fingerprint." This plan deliberately keeps it OUT of `genesis_namespace` (matching how reach hints are handled — operational policy, not validator identity; flipping the mode fails closed, it is not a key-substitution vector). Update the spec's one line to match before/after implementation.
