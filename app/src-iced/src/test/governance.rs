//! Governance: per-form error routing (B2) and per-proposal busy scoping (M1).

use super::harness::*;
use crate::screens::governance::{
    self, Action, Ballot, Filter, FormSlot, GovernanceData, Message, OperationPhase, Proposal,
    ProposalStatus, Resource, Shares, State, UpgradeStatus, VoterKind, VotingRule,
};
use crate::theme;

fn open_proposal(id: &str) -> Proposal {
    Proposal {
        id: id.into(),
        action: Action::Signal("Ship it".into()),
        proposer: "aa".repeat(32),
        created_at: 10,
        deadline: 20,
        status: ProposalStatus::Open,
        votes: Vec::new(),
        voter_kind: VoterKind::ValidatorNode,
        electorate: Vec::new(),
        voting_rule: VotingRule::DynamicValidatorMajority,
    }
}

fn ready(proposals: Vec<Proposal>, shares: Shares) -> State {
    State {
        data: Resource::Ready(GovernanceData {
            proposals,
            shares,
            local_nodes: vec!["aa".repeat(32)],
            local_account: Some("11".repeat(32)),
            member_count: 3,
            legacy_can_vote: true,
            known_accounts: vec!["11".repeat(32)],
            current_height: 100,
            upgrade: Resource::Ready(UpgradeStatus {
                current_version: 3,
                pending: None,
                armed: false,
                members: Vec::new(),
            }),
        }),
        ..State::default()
    }
}

fn no_shares() -> Shares {
    Shares {
        active: false,
        allocations: Vec::new(),
        total: 0,
    }
}

#[test]
fn settle_stays_live_while_an_unrelated_write_is_in_flight() {
    // M1: an unrelated write sets the global `busy` flag; Settle must remain
    // clickable on an open proposal that has no in-flight op of its own.
    let mut state = ready(vec![open_proposal("signal:1")], no_shares());
    state.busy = true;
    let mut ui = sim(governance::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::Button, "Settle"))
        .expect("Settle must stay live under an unrelated busy flag");
    // `governance::Message` is not `PartialEq` (it carries a `text_editor::Action`),
    // so match the emitted message by shape rather than via `emitted`.
    assert!(
        ui.into_messages()
            .any(|message| matches!(message, Message::Execute(id) if id == "signal:1")),
        "clicking Settle emits Execute for the open proposal"
    );
    // The live button is only honest if `update` actually acts on the message
    // while `busy` is set — otherwise the control is enabled but inert.
    assert!(
        matches!(
            governance::update(&mut state, Message::Execute("signal:1".into())),
            Some(governance::Command::Execute(id)) if id == "signal:1"
        ),
        "Execute must be honored under an unrelated busy flag, not silently dropped"
    );
}

#[test]
fn an_in_flight_op_on_one_proposal_leaves_the_others_votable() {
    // M1: acting on proposal A must not disable proposal B. A's own vote is
    // blocked by its in-flight op; B, which has no op, stays actionable.
    let mut state = ready(
        vec![open_proposal("prop-a"), open_proposal("prop-b")],
        no_shares(),
    );
    state.operations.insert(
        "prop-a".into(),
        OperationPhase::Receipt {
            height: 200,
            op_hash: None,
        },
    );
    assert_eq!(
        governance::update(
            &mut state,
            Message::Vote {
                proposal_id: "prop-a".into(),
                approve: true,
            }
        ),
        None,
        "the acting proposal is disabled while its own op is in flight"
    );
    assert!(
        matches!(
            governance::update(
                &mut state,
                Message::Vote {
                    proposal_id: "prop-b".into(),
                    approve: true,
                }
            ),
            Some(governance::Command::Vote { .. })
        ),
        "an unrelated proposal stays votable"
    );
}

#[test]
fn an_invalid_share_change_lands_inline_not_in_the_bottom_banner() {
    // B2: a rejected share submit must surface at its own form, so `form_error`
    // (rendered inline) is set and the shared bottom `error` banner is not.
    let mut state = ready(
        vec![open_proposal("signal:1")],
        Shares {
            active: true,
            allocations: vec![governance::ShareAllocation {
                account_id: "11".repeat(32),
                shares: 10,
            }],
            total: 10,
        },
    );
    governance::update(&mut state, Message::ShareAccountChanged("11".repeat(32)));
    governance::update(&mut state, Message::ShareValueChanged("not-a-number".into()));
    assert_eq!(
        governance::update(&mut state, Message::ProposeSetShares),
        None
    );
    assert!(
        matches!(&state.form_error, Some((FormSlot::ShareChange, _))),
        "the reason belongs to the share-change form"
    );
    assert!(
        state.error.is_none(),
        "the off-screen bottom banner must stay empty for a form-owned error"
    );
    // The view still renders with the inline error present.
    let _ = sim(governance::view(&state, theme::Mode::Light));
}

fn settled(id: &str, status: ProposalStatus) -> Proposal {
    Proposal {
        status,
        ..open_proposal(id)
    }
}

#[test]
fn loading_hides_retry() {
    let state = State {
        data: Resource::Loading,
        ..State::default()
    };
    let mut ui = sim(governance::view(&state, theme::Mode::Light));
    assert!(ui.find("Loading governance").is_ok());
    assert!(
        !has(&mut ui, Role::Button, "Retry"),
        "an in-progress load is not retryable"
    );
}

#[test]
fn empty_offers_retry() {
    let state = State {
        data: Resource::Empty,
        ..State::default()
    };
    let mut ui = sim(governance::view(&state, theme::Mode::Light));
    assert!(ui.find("Governance unavailable").is_ok());
    ui.click(by::role(Role::Button, "Retry"))
        .expect("the empty state offers a retry");
    assert!(ui.into_messages().any(|m| matches!(m, Message::Load)));
}

#[test]
fn error_surfaces_reason_and_retry() {
    // The error render variant plus its recovery affordance — the class of gap
    // that let the pages regression hide.
    let state = State {
        data: Resource::Error("node offline".into()),
        ..State::default()
    };
    let mut ui = sim(governance::view(&state, theme::Mode::Light));
    assert!(
        ui.find("node offline").is_ok(),
        "the load-failure reason is shown, not swallowed"
    );
    ui.click(by::role(Role::Button, "Retry"))
        .expect("the error state offers a retry");
    assert!(ui.into_messages().any(|m| matches!(m, Message::Load)));
}

#[test]
fn filter_segment_emits_set_filter() {
    let state = ready(vec![open_proposal("signal:1")], no_shares());
    let mut ui = sim(governance::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::Button, "Open"))
        .expect("a filter segment is clickable");
    assert!(
        ui.into_messages()
            .any(|m| matches!(m, Message::SetFilter(Filter::Open)))
    );
}

#[test]
fn proposal_list_shows_filtered_empty_states() {
    let empty = ready(Vec::new(), no_shares());
    let mut ui = sim(governance::view(&empty, theme::Mode::Light));
    assert!(ui.find("No proposals to show.").is_ok());

    // An open proposal hidden by the Settled filter → the settled-empty copy.
    let mut hidden = ready(vec![open_proposal("signal:1")], no_shares());
    hidden.filter = Filter::Settled;
    let mut ui = sim(governance::view(&hidden, theme::Mode::Light));
    assert!(ui.find("No settled proposals to show.").is_ok());
}

#[test]
fn open_proposal_renders_detail_and_vote_controls() {
    let state = ready(vec![open_proposal("signal:1")], no_shares());
    let mut ui = sim(governance::view(&state, theme::Mode::Light));
    assert!(ui.find("Ship it").is_ok(), "the action detail renders");
    assert!(ui.find("open").is_ok(), "the open status pill renders");
    assert!(has(&mut ui, Role::Button, "✓  Approve"));
    assert!(has(&mut ui, Role::Button, "×  Reject"));
    assert!(has(&mut ui, Role::Button, "Settle"));
}

#[test]
fn settled_proposal_hides_the_vote_row() {
    let state = ready(
        vec![settled("signal:1", ProposalStatus::Passed)],
        no_shares(),
    );
    let mut ui = sim(governance::view(&state, theme::Mode::Light));
    assert!(ui.find("passed").is_ok(), "the settled status pill renders");
    assert!(
        !has(&mut ui, Role::Button, "✓  Approve"),
        "a settled proposal exposes no vote row"
    );
    assert!(!has(&mut ui, Role::Button, "Settle"));
}

#[test]
fn approve_click_emits_a_yes_vote() {
    let state = ready(vec![open_proposal("signal:1")], no_shares());
    let mut ui = sim(governance::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::Button, "✓  Approve"))
        .expect("an eligible voter can approve");
    assert!(
        ui.into_messages().any(
            |m| matches!(m, Message::Vote { proposal_id, approve: true } if proposal_id == "signal:1")
        ),
        "clicking Approve emits a yes vote for the proposal"
    );
}

#[test]
fn a_cast_yes_vote_locks_approve_but_frees_reject() {
    // The Approve button is the toggled/active state once the local node has
    // voted yes: re-clicking it must not re-emit, while Reject stays live to flip
    // the ballot.
    let mut proposal = open_proposal("signal:1");
    proposal.votes = vec![Ballot {
        principal: "aa".repeat(32),
        approve: true,
    }];
    let state = ready(vec![proposal], no_shares());
    let mut ui = sim(governance::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::Button, "✓  Approve"))
        .expect("the toggled Approve button is still present");
    ui.click(by::role(Role::Button, "×  Reject"))
        .expect("reject stays live");
    let messages: Vec<Message> = ui.into_messages().collect();
    assert!(
        !messages
            .iter()
            .any(|m| matches!(m, Message::Vote { approve: true, .. })),
        "re-approving an already-approved proposal emits nothing"
    );
    assert!(
        messages
            .iter()
            .any(|m| matches!(m, Message::Vote { approve: false, .. })),
        "reject remains actionable to change the ballot"
    );
}

#[test]
fn reload_error_surfaces_over_a_populated_view() {
    // A background-refresh failure keeps the last-good proposals but surfaces the
    // reason inline instead of blanking the surface (Ready-view error path).
    let mut state = ready(vec![open_proposal("signal:1")], no_shares());
    state.reload_error = Some("node offline".into());
    let mut ui = sim(governance::view(&state, theme::Mode::Light));
    assert!(
        ui.find("Live refresh unavailable: node offline").is_ok(),
        "the reload error is shown without discarding the proposals"
    );
    assert!(
        ui.find("Ship it").is_ok(),
        "the last-good proposals remain rendered"
    );
}
