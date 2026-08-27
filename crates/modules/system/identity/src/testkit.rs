//! Test-only ed25519 member-auth builders, beside [`IDENTITY_BIND_NS`] where
//! they belong. The [`MemberAuth`] these produce is what a bound/governed
//! scenario submits as an `authorizer`; it was re-rolled in ~8 node/sim suites
//! before landing here.
//!
//! Gated behind the `testkit` feature — consumers enable it as a dev-dependency
//! feature only, so it never compiles into a shipping build.

use commonware_cryptography::ed25519;

use crate::{IDENTITY_BIND_NS, KeyScheme, MemberAuth};

/// a [`MemberAuth`] whose ed25519 `key` signs `preimage` in the `ns` domain —
/// the general builder (bind, unbind, add/remove-member differ only in `ns` and
/// the preimage bytes the caller passes).
pub fn ed_auth(key: &ed25519::PrivateKey, ns: &[u8], preimage: &[u8]) -> MemberAuth {
    use commonware_cryptography::Signer as _;
    MemberAuth {
        key: key.public_key().as_ref().to_vec(),
        scheme: KeyScheme::Ed25519,
        proof: keyscheme::testkit::ed25519_proof(key, ns, preimage),
    }
}

/// [`ed_auth`] fixed to the node-bind namespace ([`IDENTITY_BIND_NS`]) — the
/// common case (a member consents to binding a node over some `bind_preimage`).
pub fn ed_bind_auth(key: &ed25519::PrivateKey, preimage: &[u8]) -> MemberAuth {
    ed_auth(key, IDENTITY_BIND_NS, preimage)
}
