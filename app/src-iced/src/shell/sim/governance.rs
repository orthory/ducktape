//! Governance load coverage against the embedded sim node.
//!
//! `SimShell::boot()` uses the default `SimOpts`, whose module set excludes
//! governance. Write coverage therefore needs an embedder-level fixture with
//! `SimOpts::valset_keys`; see `bin/simnode/tests/embed.rs`.

use super::super::*;
use super::SimShell;
use crate::screens::governance::Resource;

#[test]
fn load_error_is_visible() {
    let mut ui = SimShell::boot();
    ui.inject(Message::Navigate(Screen::Governance));

    let error = match &ui.shell().governance.data {
        Resource::Error(error) => error,
        other => panic!("default sim unexpectedly loaded governance: {other:?}"),
    };
    assert_eq!(error, "UnknownModule(governance)");
    assert!(
        ui.sees_text(error),
        "governance load failure must be visible: {error}"
    );
}
