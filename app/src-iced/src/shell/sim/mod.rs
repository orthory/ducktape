//! The sim lane: transaction round-trips against a deterministic simnode.
//!
//! The in-process recipe lane (`shell/qa.rs`) proves what `update()` +
//! `view()` can prove but never runs a Task; the fleet lane has a real node
//! but nothing deterministic. This lane closes the gap — the iced twin of the
//! TS `app/src/test/sim/` suites: boot the real shell state, point
//! `node_client` at a spawned `ducktape-simnode`, execute `update()` Tasks on
//! a private tokio runtime, and assert committed state renders back through
//! the real view. Design: docs/superpowers/specs/2026-07-16-iced-sim-lane-design.md.

mod node;
