//! Members load coverage against the embedded sim node.

use super::super::*;
use super::{SimShell, fixture_pubkey_bytes, signing};
use crate::screens::members::{Resource, Tier};
use iced_agent_plugin::Role;

#[test]
fn default_boot_missing_valset_is_visible() {
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

#[test]
fn valset_boot_renders_the_fixture_validator() {
    let mut ui = SimShell::boot_with_valset();
    ui.inject(Message::Navigate(Screen::Members));

    assert!(
        ui.shell().members.error.is_none(),
        "members load failed: {:?}",
        ui.shell().members.error
    );
    let data = match &ui.shell().members.data {
        Resource::Ready(data) => data,
        other => panic!("members roster did not load: {other:?}"),
    };
    let pubkey = signing::author_pubkey_hex();
    let validator = data
        .members
        .iter()
        .find(|member| member.key == pubkey)
        .expect("fixture validator is present in the render model");
    assert_eq!(validator.tier, Tier::Validator);
    assert!(
        ui.has(Role::ListItem, &validator.display_name),
        "fixture validator row renders"
    );

    let validators = ui.node_query("valset", serde_json::json!("validators"));
    assert_eq!(
        validators["validators"],
        serde_json::json!([fixture_pubkey_bytes()]),
        "rendered fixture validator matches committed valset state"
    );
}
