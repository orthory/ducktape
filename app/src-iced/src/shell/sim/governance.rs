//! Governance transaction round-trips against the embedded sim node.

use super::super::*;
use super::SimShell;
use crate::screens::governance::{self, Action, Resource};
use iced_agent_plugin::Role;

#[test]
fn default_boot_missing_governance_is_visible() {
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

#[test]
fn valset_boot_signal_proposal_round_trips() {
    const SIGNAL: &str = "ship the sim coverage";

    let mut ui = SimShell::boot_with_valset();
    ui.inject(Message::Navigate(Screen::Governance));
    let Resource::Ready(data) = &ui.shell().governance.data else {
        panic!("governance did not load: {:?}", ui.shell().governance.data)
    };
    assert!(
        data.legacy_can_vote,
        "fixture signer must be eligible to propose"
    );

    ui.inject(Message::Governance(governance::Message::SignalChanged(
        SIGNAL.into(),
    )));
    ui.click(Role::Button, "Propose");

    assert!(
        ui.shell().governance.error.is_none(),
        "proposal failed: {:?}",
        ui.shell().governance.error
    );
    let proposal = match &ui.shell().governance.data {
        Resource::Ready(data) => data
            .proposals
            .iter()
            .find(|proposal| proposal.action == Action::Signal(SIGNAL.into())),
        _ => None,
    }
    .expect("committed signal loaded into the render model");
    assert!(
        ui.sees_text(SIGNAL),
        "committed signal renders in the proposal list"
    );

    let proposals = ui.node_query("governance", serde_json::json!("proposals"));
    assert!(
        proposals["proposals"]
            .as_array()
            .is_some_and(|rows| rows.iter().any(|row| {
                row["proposal_id"].as_str() == Some(proposal.id.as_str())
                    && row["action"]["signal"]["text"].as_str() == Some(SIGNAL)
            })),
        "rendered proposal is committed node-side: {proposals}"
    );
}
