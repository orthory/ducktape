//! The device keystore: named encrypted user keys and the active-wallet
//! pointer, plus the seal/open primitives underneath them.
//!
//! A LIBRARY rather than part of the node binary, because two very different
//! programs need the same keystore and only one of them is a CLI. The desktop
//! app used to reach this code by SPAWNING `ducktape` and talking to it over
//! pipes — which meant the app had to FIND that binary on disk, and a person
//! whose helper was missing (a fresh checkout, a build still linking, a bundle
//! without its sibling) was told to check their install while trying to open
//! their own key. Opening your own key is not an errand to send a subprocess
//! on; it is the app's own business, and this crate is what lets it be.
//!
//! Nothing here knows about consensus, the node, or the network: the whole
//! dependency set is argon2 + XChaCha20-Poly1305 + ed25519 + encoding. That is
//! what makes it linkable from a GUI without dragging a validator in behind it.

pub mod userkey;
pub mod wallet;
