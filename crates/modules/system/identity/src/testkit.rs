//! Test-only consent builders, beside [`IDENTITY_ADD_KEY_NS`] where they
//! belong. Every node/sim suite that founds an account or admits a key mints
//! its messages here, so "what a signer produces" is written once.
//!
//! Gated behind the `testkit` feature — consumers enable it as a dev-dependency
//! feature only, so it never compiles into a shipping build.

use commonware_cryptography::{Signer as _, ed25519};

use crate::{
    AccountNumber, Authorizer, IDENTITY_ADD_KEY_NS, IdentityMsg, KeyScheme, add_key_preimage,
};

/// an existing ed25519 member's consent to admit `new_key` (of `scheme`) into
/// `account` at generation `gen` on chain `chain_id`, live until `expires_at`.
pub fn ed_authorizer(
    member: &ed25519::PrivateKey,
    chain_id: &str,
    scheme: KeyScheme,
    new_key: &[u8],
    generation: u64,
    account: AccountNumber,
    expires_at: u64,
) -> Authorizer {
    let preimage = add_key_preimage(chain_id, scheme, new_key, generation, account, expires_at);
    Authorizer {
        key: member.public_key().as_ref().to_vec(),
        account,
        expires_at,
        proof: keyscheme::testkit::ed25519_proof(member, IDENTITY_ADD_KEY_NS, &preimage),
    }
}

/// the founding op for an ed25519 origin.
pub fn create(name: &str) -> IdentityMsg {
    IdentityMsg::Create {
        name: name.into(),
        scheme: KeyScheme::Ed25519,
    }
}

/// the admission op for an ed25519 origin, consented to by `member` (a key of
/// `account`) until `expires_at`.
pub fn add_ed25519_key(
    member: &ed25519::PrivateKey,
    chain_id: &str,
    new_key: &[u8],
    generation: u64,
    label: Option<&str>,
    account: AccountNumber,
    expires_at: u64,
) -> IdentityMsg {
    IdentityMsg::AddKey {
        scheme: KeyScheme::Ed25519,
        label: label.map(str::to_string),
        authorizer: ed_authorizer(
            member,
            chain_id,
            KeyScheme::Ed25519,
            new_key,
            generation,
            account,
            expires_at,
        ),
    }
}
