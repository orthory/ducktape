//! quack-harness — the Quack package test framework (design D9a).
//!
//! testing is a first-class primitive of the packaged-module system: the ADR
//! requires every package with agents or actions to ship deterministic
//! harness tests a recipient can run before activation. this crate makes that
//! cheap:
//!
//! - [`PackageTestBed`] — an in-process [`host::Host`] with the standard
//!   platform module set (package, memory, pages, chat, tasks, tagging, saga,
//!   capability, dispatch, jobs, agent, runs) plus caller-supplied package
//!   modules; ordered-op submission with real origins; the canned-oracle
//!   worker loop (`collaboration_loop.rs` pattern), so provider output is
//!   scripted data, never a live LLM.
//! - **install driving** — [`install_spec_from_capsule`] maps a verified
//!   [`quack::Capsule`] into the [`package::InstallSpec`] wire shape (the same
//!   mapping the CLI performs), and
//!   [`PackageTestBed::install_capsule`] submits it and returns an
//!   [`InstallReport`] (row status, seeded prompt generations + hashes,
//!   registered agents + owners, action routes).
//! - **the assertion kit** — panicking helpers on the testbed and the report
//!   for the ADR harness checklist (see `InstallReport::assert_*` and
//!   `PackageTestBed::assert_*`).
//! - **golden fixtures** — [`GoldenFixture`]/[`GoldenStep`] parse the
//!   capsule's `harness/golden.json` and [`run_golden`] executes it: the same
//!   script a package author's crate test runs is what the CLI's
//!   `package test` replays against the binary's native module catalog.
//! - [`PackageTestBed::snapshot_roundtrip_all`] — the snapshot/state-sync
//!   sweep over every registered module.
//!
//! a package author writes:
//!
//! ```ignore
//! PackageTestBed::run(vec![Box::new(MyHarness::new(...))], |mut bed| async move {
//!     let report = bed.install_capsule(&capsule, "my-harness", &bindings, alice()).await?;
//!     report.assert_active();
//!     // ... script oracle turns, assert the checklist ...
//! });
//! ```
//!
//! the in-crate [`dummy`] module is the framework's own test double AND the
//! smallest complete template of the harness contract a package author can
//! copy from.

pub mod dummy;
mod error;
mod golden;
mod install;
mod testbed;

pub use error::HarnessError;
pub use golden::{
    GoldenError, GoldenFixture, GoldenRun, GoldenStep, SubmitExpect, diff_json, parse_origin,
    run_golden,
};
pub use install::{
    InstallReport, RegisteredAgent, SeededPrompt, install_spec_from_capsule,
    install_spec_from_capsule_defaulted,
};
pub use testbed::{BlockEvent, ModuleRoundtrip, PackageTestBed, RoundtripKind};

// the shapes a package test asserts against, re-exported so an author's test
// crate needs only `quack-harness` (plus its own harness module) to start.
pub use package::{InstallSpec, PackageStatus};
pub use quack::Capsule;
pub use sdk::{Module, Origin};
