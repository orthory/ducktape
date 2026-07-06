//! Coordinator authorization: a per-request authenticator
//! (proof-of-possession plus an optional genesis-issued capability) verified
//! statelessly against a PUBLIC pin. The coordinator holds no secret; every
//! check here is a clock read plus one or two ed25519 verifications against
//! public keys.

use std::time::{SystemTime, UNIX_EPOCH};

use commonware_cryptography::{ed25519, Signer as _, Verifier as _};

use crate::NodeKey;

/// PoP signing namespace: `sign(COORD_REQ_NS, inner_request_bytes ‖ timestamp)`.
pub const COORD_REQ_NS: &[u8] = b"ducktape-coord-req-v1";
/// Capability signing namespace: `sign(COORD_CAP_NS, subject ‖ not_after)`.
pub const COORD_CAP_NS: &[u8] = b"ducktape-coord-cap-v1";
/// Max clock skew (seconds) between a request timestamp and the coordinator.
pub const DEFAULT_FRESHNESS_WINDOW_SECS: u64 = 30;
/// Lifetime of a minted admission capability (`mint_coord_cap`), in seconds:
/// one year. Deliberately long-lived — there is NO cap-rotation flow yet (a
/// joiner receives exactly one cap over its `JoinReply` and never refreshes
/// it), so a short TTL would strand admitted nodes. Rotation (re-minting +
/// re-delivering a fresh cap before expiry) is DEFERRED; when it lands this
/// TTL should shrink to match the rotation cadence.
pub const COORD_CAP_TTL_SECS: u64 = 365 * 24 * 3600;

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

#[cfg(test)]
mod tests {
    // `Signer` (for `from_seed`/`public_key`) and `ed25519` come in via the
    // parent module's imports through this glob.
    use super::*;

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
        // A NodeKey that is not a valid ed25519 point cannot verify PoP. `[2u8;
        // 32]` is not a decompressable curve25519 point in this build's
        // `ed25519::PublicKey::decode`, so admission fails at the key check
        // BEFORE PoP verification. (The plan's `[0xff; 32]` decompresses to a
        // valid point in this commonware version — its non-canonical y reduces
        // mod p — so it would surface as `BadPop`, not `BadSubjectKey`.)
        let subject = NodeKey([2u8; 32]);
        let policy = AuthPolicy::Open { require_pop: true };
        let node = key(1);
        let auth = sign_authenticator(&node, INNER, 1_000_000, None);
        assert_eq!(
            verify_request(&policy, 1_000_000, 30, subject, INNER, &auth),
            Err(AuthError::BadSubjectKey)
        );
    }
}
