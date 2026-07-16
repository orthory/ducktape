//! The sim lane: transaction round-trips against an EMBEDDED deterministic
//! simnode (`simnode::boot`) — the iced twin of the TS `app/src/test/sim/`
//! suites, foundry-style: plain `cargo test` needs no external binaries.
//! Design: docs/superpowers/specs/2026-07-16-iced-sim-lane-design.md (v2).
