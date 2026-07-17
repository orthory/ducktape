//! Shared types + crypto for the trustless credential gateway PoC.
//!
//! Nothing here does IO or async — it is the pure core both the `tcg-host`
//! and `tcg-client` binaries build on. See the design spec:
//! `docs/superpowers/specs/2026-07-18-trustless-credential-gateway-poc-design.md`.

pub mod attest;
pub mod handshake;
pub mod seal;
pub mod token;
pub mod wire;
