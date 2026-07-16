//! Members transaction round-trips against the embedded sim node.
//!
//! `SimShell` boots the default sim composition, which has no `valset` module,
//! so this surface cannot reach `Resource::Ready`. A ready-roster embedder test
//! needs `SimOpts::valset_keys`; invite coverage also needs a matching
//! `invite_binding` and managed workspace. See `bin/simnode/tests/embed.rs`.

use super::super::*;
use super::SimShell;
use crate::screens::members::Resource;
use iced_agent_plugin::Role;

#[test]
fn missing_valset_is_visible() {
    let mut ui = SimShell::boot();
    ui.inject(Message::Navigate(Screen::Members));

    assert!(ui.shell().members.error.is_none());
    let error = match &ui.shell().members.data {
        Resource::Error(error) => error,
        other => panic!("members load did not surface its failure: {other:?}"),
    };
    assert_eq!(error, "UnknownModule(valset)");
    assert!(ui.sees_text("Members unavailable"));
    assert!(ui.sees_text(error));

    ui.click(Role::Button, "Retry");
    assert_eq!(
        ui.shell().members.data,
        Resource::Error("UnknownModule(valset)".into())
    );
}
