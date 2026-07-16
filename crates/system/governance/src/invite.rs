//! the invite capability — the token a member mints when inviting, and the
//! verification every consumer of that token shares.
//!
//! minting IS the admission decision: a token is the issuer's ed25519
//! signature over `binding ‖ nonce ‖ kind[‖ target] ‖ role ‖ expiry`, where
//! `binding` is the network's genesis namespace (chain-id + genesis
//! fingerprint), `nonce` is per-invite randomness, and `kind` says whether a
//! `target` follows: a TARGETED token (kind 1) admits exactly the one key it
//! names, a BEARER token (kind 0) admits whichever key redeems it first.
//! bearer is CLIENT-ROLE-ONLY — no bearer path onto the resident plane
//! exists; redemption and every admission door enforce it. `role` selects
//! the standing plane (`Resident` = full node standing, `Client` = submit
//! authorization only) and `expires` is the unix-seconds redemption
//! deadline. the joiner proves possession of its own announced key by
//! signing `binding ‖ nonce ‖ joiner` — a blob holder cannot redeem under a
//! key that never asked to join, and a non-target key is refused by a
//! targeted token even with a valid self-proof. redemption is SINGLE-USE,
//! enforced in consensus state by this crate's `GovMsg::Redeem` handler (the
//! nonce is the exactly-once key) — for a bearer token that single use IS
//! the whole containment story, alongside its short expiry.
//!
//! this module is pure crypto + types: minting (which needs OS randomness)
//! lives with the CLI in `bin/node`; the node's lobby path and the in-module
//! `Redeem` verification both call the functions here, so a token means the
//! same thing at every trust point.

use commonware_cryptography::{Verifier as _, ed25519};

/// ed25519 signing namespace for the grant an issuer mints:
/// `sign(INVITE_GRANT_NAMESPACE, binding ‖ nonce ‖ kind[‖ target] ‖ role ‖ expiry)`.
pub const INVITE_GRANT_NAMESPACE: &[u8] = b"ducktape-invite-grant-v1";
/// ed25519 signing namespace for the joiner's proof-of-possession:
/// `sign(INVITE_JOIN_NAMESPACE, binding ‖ nonce ‖ joiner)`.
pub const INVITE_JOIN_NAMESPACE: &[u8] = b"ducktape-invite-join-v1";
/// invite token nonce width in bytes.
pub const INVITE_NONCE_LEN: usize = 16;

/// the standing an invite grants. `Resident` = full node standing (mesh +
/// statesync, targeted invites only). `Client` = submit authorization only
/// (the `clients` module — no statesync, no mesh, no quorum seat), and the
/// only role a BEARER token may carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum InviteRole {
    Resident = 0,
    Client = 1,
}

impl InviteRole {
    pub fn from_u8(b: u8) -> Result<Self, String> {
        match b {
            0 => Ok(Self::Resident),
            1 => Ok(Self::Client),
            other => Err(format!("unknown invite role {other}")),
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct InviteToken {
    /// the minting member — checked against CURRENT membership on redemption.
    pub issuer: ed25519::PublicKey,
    /// per-invite randomness: distinguishes tokens and is the single-use key.
    pub nonce: [u8; INVITE_NONCE_LEN],
    /// the ONE key this invite admits, or `None` for a BEARER invite —
    /// bearer is Client-role-only (a bearer token can never grant resident
    /// standing; redemption and every admission door enforce it).
    pub target: Option<ed25519::PublicKey>,
    /// the standing this invite grants (see [`InviteRole`]).
    pub role: InviteRole,
    /// unix seconds; enforced at decode (joiner) and at the gating member's
    /// wall clock (consensus has no wall clock — block-height time).
    pub expires_unix_secs: u64,
    /// issuer's signature over `binding ‖ nonce ‖ kind[‖ target] ‖ role ‖
    /// expiry` in the invite-grant namespace.
    pub sig: ed25519::Signature,
}

/// the signed preimage of an invite grant: `binding ‖ nonce ‖ kind[‖
/// target] ‖ role ‖ expiry`, where the kind byte distinguishes targeted
/// (`0x01 ‖ target`) from bearer (`0x00`, no target bytes) so neither form
/// can be replayed as the other. every covered field is authenticated —
/// tampering any of them breaks the signature.
fn grant_preimage(
    binding: &[u8],
    nonce: &[u8],
    target: Option<&ed25519::PublicKey>,
    role: InviteRole,
    expires: u64,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(binding.len() + nonce.len() + 42);
    out.extend_from_slice(binding);
    out.extend_from_slice(nonce);
    match target {
        Some(t) => {
            out.push(1);
            out.extend_from_slice(t.as_ref());
        }
        None => out.push(0),
    }
    out.push(role.as_u8());
    out.extend_from_slice(&expires.to_le_bytes());
    out
}

/// verify a token: issuer signature over the grant preimage. pure
/// signature math — the bearer-is-client-only invariant is enforced where
/// admission is decided (redeem, the doors), not here.
pub fn verify_invite_token(token: &InviteToken, binding: &[u8]) -> bool {
    let msg = grant_preimage(
        binding,
        token.nonce.as_slice(),
        token.target.as_ref(),
        token.role,
        token.expires_unix_secs,
    );
    token
        .issuer
        .verify(INVITE_GRANT_NAMESPACE, &msg, &token.sig)
}

/// the joiner's proof-of-possession over its own key for `token` — binds the
/// announced pubkey to someone actually holding its secret.
pub fn sign_join_proof(
    joiner: &ed25519::PrivateKey,
    binding: &[u8],
    token: &InviteToken,
) -> ed25519::Signature {
    use commonware_cryptography::Signer as _;
    let msg = [
        binding,
        token.nonce.as_slice(),
        joiner.public_key().as_ref(),
    ]
    .concat();
    joiner.sign(INVITE_JOIN_NAMESPACE, &msg)
}

/// verify a joiner's proof-of-possession against `token`.
pub fn verify_join_proof(
    joiner: &ed25519::PublicKey,
    binding: &[u8],
    token: &InviteToken,
    proof: &ed25519::Signature,
) -> bool {
    let msg = [binding, token.nonce.as_slice(), joiner.as_ref()].concat();
    joiner.verify(INVITE_JOIN_NAMESPACE, &msg, proof)
}

#[cfg(test)]
mod tests {
    use super::*;
    use commonware_cryptography::Signer as _;

    const BINDING: &[u8] = b"net#00000000@feedface";

    fn mint(issuer: &ed25519::PrivateKey, binding: &[u8], target: &ed25519::PublicKey) -> InviteToken {
        let nonce = [7u8; INVITE_NONCE_LEN];
        let (role, expires) = (InviteRole::Resident, 4_102_444_800); // 2100-01-01
        let msg = grant_preimage(binding, &nonce, Some(target), role, expires);
        InviteToken {
            issuer: issuer.public_key(),
            nonce,
            target: Some(target.clone()),
            role,
            expires_unix_secs: expires,
            sig: issuer.sign(INVITE_GRANT_NAMESPACE, &msg),
        }
    }

    #[test]
    fn a_token_binds_network_target_role_and_expiry() {
        let issuer = ed25519::PrivateKey::from_seed(1);
        let target = ed25519::PrivateKey::from_seed(2);
        let token = mint(&issuer, BINDING, &target.public_key());
        assert!(verify_invite_token(&token, BINDING));
        assert!(!verify_invite_token(&token, b"other-net"));

        // tampering ANY covered field kills the signature.
        let mut t = token.clone();
        t.target = Some(ed25519::PrivateKey::from_seed(3).public_key());
        assert!(!verify_invite_token(&t, BINDING));
        let mut t = token.clone();
        t.role = InviteRole::Client;
        assert!(!verify_invite_token(&t, BINDING));
        let mut t = token.clone();
        t.expires_unix_secs += 1;
        assert!(!verify_invite_token(&t, BINDING));
    }

    #[test]
    fn the_join_proof_binds_the_key_not_just_the_token() {
        let issuer = ed25519::PrivateKey::from_seed(1);
        let joiner = ed25519::PrivateKey::from_seed(2);
        let token = mint(&issuer, BINDING, &joiner.public_key());

        let proof = sign_join_proof(&joiner, BINDING, &token);
        assert!(verify_join_proof(
            &joiner.public_key(),
            BINDING,
            &token,
            &proof
        ));
        // the proof binds the KEY, not just the token.
        assert!(!verify_join_proof(
            &ed25519::PrivateKey::from_seed(3).public_key(),
            BINDING,
            &token,
            &proof
        ));
    }

    #[test]
    fn a_client_role_survives_the_signature_and_role_bytes_round_trip() {
        // a Client token verifies cryptographically — the redeem gate, not the
        // signature, is what defers the thin-client plane.
        let issuer = ed25519::PrivateKey::from_seed(1);
        let target = ed25519::PrivateKey::from_seed(2);
        let nonce = [9u8; INVITE_NONCE_LEN];
        let (role, expires) = (InviteRole::Client, u64::MAX);
        let msg = grant_preimage(BINDING, &nonce, Some(&target.public_key()), role, expires);
        let token = InviteToken {
            issuer: issuer.public_key(),
            nonce,
            target: Some(target.public_key()),
            role,
            expires_unix_secs: expires,
            sig: issuer.sign(INVITE_GRANT_NAMESPACE, &msg),
        };
        assert!(verify_invite_token(&token, BINDING));
        assert_eq!(InviteRole::from_u8(0), Ok(InviteRole::Resident));
        assert_eq!(InviteRole::from_u8(1), Ok(InviteRole::Client));
        assert!(InviteRole::from_u8(2).is_err());
    }

    #[test]
    fn a_bearer_token_verifies_and_kinds_are_not_interchangeable() {
        let issuer = ed25519::PrivateKey::from_seed(1);
        let nonce = [8u8; INVITE_NONCE_LEN];
        let (role, expires) = (InviteRole::Client, 4_102_444_800);
        let msg = grant_preimage(BINDING, &nonce, None, role, expires);
        let token = InviteToken {
            issuer: issuer.public_key(),
            nonce,
            target: None,
            role,
            expires_unix_secs: expires,
            sig: issuer.sign(INVITE_GRANT_NAMESPACE, &msg),
        };
        assert!(verify_invite_token(&token, BINDING));
        assert!(!verify_invite_token(&token, b"other-net"));

        // grafting a target onto a bearer sig (or stripping one off a
        // targeted sig) breaks verification: the kind byte is
        // signature-covered, so the forms are not interchangeable.
        let mut t = token.clone();
        t.target = Some(ed25519::PrivateKey::from_seed(3).public_key());
        assert!(!verify_invite_token(&t, BINDING));
        let targeted = mint(
            &issuer,
            BINDING,
            &ed25519::PrivateKey::from_seed(4).public_key(),
        );
        let mut stripped = targeted.clone();
        stripped.target = None;
        assert!(!verify_invite_token(&stripped, BINDING));

        // the join proof works identically for a bearer token: it binds the
        // REDEEMING key (there is no target to bind).
        let redeemer = ed25519::PrivateKey::from_seed(5);
        let proof = sign_join_proof(&redeemer, BINDING, &token);
        assert!(verify_join_proof(&redeemer.public_key(), BINDING, &token, &proof));
        assert!(!verify_join_proof(
            &ed25519::PrivateKey::from_seed(6).public_key(),
            BINDING,
            &token,
            &proof
        ));
    }
}
