//! Screen-level unit tests — the `test.tsx` layer of the QA pyramid.
//!
//! One file per surface, short names, mirroring the React twin's
//! `app/src/test/sim/<surface>.test.tsx` convention. Tests live inside the
//! crate because screen `State`/`Message` types are crate-private on purpose.
//!
//! What belongs here: a single screen's render variants (loading/empty/
//! error/ready), interactions → the `Message` values they emit (the Elm
//! version of asserting a callback fired — no mocks needed), and
//! screen-local `update` transitions.
//!
//! What does NOT belong here: cross-screen navigation and app-level flows
//! (`qa/recipes/*.json`, `lane: both`), and anything only a living process
//! shows — subscriptions, multi-window, node/CEF (`lane: fleet` recipes,
//! `ops/iced-fleet`). See `skills/qa/SKILL.md` for the full lane doctrine.
//!
//! Transaction round-trips against a deterministic embedded node belong in
//! the sim lane (`src/shell/sim/`, `cargo test -p ducktape-iced shell::sim`).

mod browser;
mod chat;
mod files;
mod harness;
mod onboarding;
mod settings;
mod terminal;
mod workspace;
