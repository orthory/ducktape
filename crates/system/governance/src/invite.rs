//! the invite capability — the token a member mints when inviting, and the
//! verification every consumer of that token shares.
//!
//! minting IS the admission decision: a token is the issuer's ed25519
//! signature over `binding ‖ nonce`, where `binding` is the network's genesis
//! namespace (chain-id + genesis fingerprint) and `nonce` is per-invite
//! randomness. the joiner proves possession of its own announced key by
//! signing `binding ‖ nonce ‖ joiner` — so a blob holder cannot redeem under
//! a key that never asked to join. redemption is SINGLE-USE, enforced in
//! consensus state by this crate's `GovMsg::Redeem` handler (the nonce is the
//! exactly-once key).
//!
//! this module is pure crypto + types: minting (which needs OS randomness)
//! lives with the CLI in `bin/node`; the node's lobby path and the in-module
//! `Redeem` verification both call the functions here, so a token means the
//! same thing at every trust point.

use commonware_cryptography::{Verifier as _, ed25519};

/// ed25519 signing namespace for the grant an issuer mints:
/// `sign(INVITE_GRANT_NAMESPACE, binding ‖ nonce)`.
pub const INVITE_GRANT_NAMESPACE: &[u8] = b"ducktape-invite-grant-v1";
/// ed25519 signing namespace for the joiner's proof-of-possession:
/// `sign(INVITE_JOIN_NAMESPACE, binding ‖ nonce ‖ joiner)`.
pub const INVITE_JOIN_NAMESPACE: &[u8] = b"ducktape-invite-join-v1";
/// invite token nonce width in bytes.
pub const INVITE_NONCE_LEN: usize = 16;

#[derive(Clone, Debug, PartialEq)]
pub struct InviteToken {
    /// the minting member — checked against CURRENT membership on redemption.
    pub issuer: ed25519::PublicKey,
    /// per-invite randomness: distinguishes tokens and is the single-use key.
    pub nonce: [u8; INVITE_NONCE_LEN],
    /// issuer's signature over `binding ‖ nonce` in the invite-grant namespace.
    pub sig: ed25519::Signature,
}

/// verify a token: issuer signature over `binding ‖ nonce`.
pub fn verify_invite_token(token: &InviteToken, binding: &[u8]) -> bool {
    let msg = [binding, token.nonce.as_slice()].concat();
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

    fn mint(issuer: &ed25519::PrivateKey, binding: &[u8]) -> InviteToken {
        let nonce = [7u8; INVITE_NONCE_LEN];
        let msg = [binding, &nonce].concat();
        InviteToken {
            issuer: issuer.public_key(),
            nonce,
            sig: issuer.sign(INVITE_GRANT_NAMESPACE, &msg),
        }
    }

    #[test]
    fn token_and_proof_verify_and_bind_to_their_network() {
        let issuer = ed25519::PrivateKey::from_seed(1);
        let joiner = ed25519::PrivateKey::from_seed(2);
        let token = mint(&issuer, BINDING);
        assert!(verify_invite_token(&token, BINDING));
        assert!(!verify_invite_token(&token, b"other-net"));

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
}
