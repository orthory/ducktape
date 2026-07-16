//! Members: invite disclosure (M4) and per-row working state (M1).

use super::harness::*;
use crate::screens::members::{
    self, BoundAccount, Command, Filter, Member, MemberAction, MembersData, Message, Provider,
    Resource, State, Tier,
};
use crate::theme;

const LOCAL: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const PEER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const RESIDENT: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";

fn member(key: &str, tier: Tier, local: bool) -> Member {
    Member {
        key: key.into(),
        display_name: if local { "Founder Rae" } else { "Joiner" }.into(),
        profile_name: None,
        initials: "FR".into(),
        avatar_bytes: None,
        tier,
        role: if local { "genesis validator" } else { "resident standing" }.into(),
        is_founder: local,
        is_local: local,
        bound_account: local.then(|| BoundAccount {
            id: "11".repeat(32),
            name: Some("Rae".into()),
            device_label: None,
        }),
        providers: if local {
            vec![Provider {
                label: "openai".into(),
                models: vec!["gpt-a".into(), "gpt-b".into()],
            }]
        } else {
            Vec::new()
        },
    }
}

fn ready() -> State {
    State {
        data: Resource::Ready(MembersData {
            members: vec![
                member(LOCAL, Tier::Validator, true),
                member(RESIDENT, Tier::Resident, false),
            ],
            can_admin: true,
            workspace_role: "Genesis".into(),
            invite_blob: Some("duck-blob-".to_string() + &"cd".repeat(60)),
            invite_short: Some("duck://short-link".into()),
            pending_joins: Vec::new(),
        }),
        ..State::default()
    }
}

#[test]
fn full_invite_stays_hidden_until_the_expander_is_opened() {
    // M4: the coordinator-free blob lives behind a disclosure, not on screen.
    let mut state = ready();
    {
        let mut ui = sim(members::view(&state, theme::Mode::Light));
        assert!(
            !has(&mut ui, Role::Button, "Copy full invite"),
            "the full-invite copy must be hidden while collapsed"
        );
    }

    members::update(&mut state, Message::ToggleInviteFull);
    assert!(state.invite_expanded, "the toggle expands the disclosure");

    let mut ui = sim(members::view(&state, theme::Mode::Light));
    assert!(
        has(&mut ui, Role::Button, "Copy full invite"),
        "expanding reveals a selectable blob with its own copy action"
    );
}

#[test]
fn only_the_acting_row_shows_a_working_state() {
    // M1: a write in flight on the resident row shows a working marker there and
    // replaces that row's actions, instead of greying the whole surface.
    let mut state = ready();
    state.busy = true;
    state.in_flight = Some(RESIDENT.to_ascii_lowercase());
    let mut ui = sim(members::view(&state, theme::Mode::Light));
    assert!(
        ui.find("Working…").is_ok(),
        "the acting row shows a working marker"
    );
    assert!(
        !has(&mut ui, Role::Button, "Promote"),
        "the acting row's own actions are replaced while it works"
    );
}

#[test]
fn an_unrelated_write_still_lets_other_rows_open_a_confirm_card() {
    // M1: the row buttons only ASK (open a local confirm card, no write), so an
    // unrelated in-flight write must not turn them into dead controls. The
    // submit is what serializes.
    let mut state = ready();
    state.busy = true;
    state.in_flight = Some("invite".into());
    members::update(&mut state, Message::AskAction(MemberAction::Promote, RESIDENT.into()));
    assert!(
        state.pending.is_some(),
        "Promote on an unrelated row must open the confirm card despite the busy flag"
    );
    // The submit itself stays serialized until the in-flight write settles.
    assert_eq!(members::update(&mut state, Message::ConfirmAction), None);
    state.busy = false;
    state.in_flight = None;
    assert_eq!(
        members::update(&mut state, Message::ConfirmAction),
        Some(Command::PromoteMember(RESIDENT.into()))
    );
}

/// A removable peer: a non-local validator (so Demote is offered) alongside the
/// resident. `invite_blob` stays `None` to keep the admin cards compact so the
/// member rows land on-screen for the simulator's clicks.
fn with_peer() -> State {
    State {
        data: Resource::Ready(MembersData {
            members: vec![
                member(LOCAL, Tier::Validator, true),
                member(PEER, Tier::Validator, false),
                member(RESIDENT, Tier::Resident, false),
            ],
            can_admin: true,
            workspace_role: "Genesis".into(),
            invite_blob: None,
            invite_short: None,
            pending_joins: Vec::new(),
        }),
        ..State::default()
    }
}

#[test]
fn loading_hides_retry() {
    let state = State {
        data: Resource::Loading,
        ..State::default()
    };
    let mut ui = sim(members::view(&state, theme::Mode::Light));
    assert!(ui.find("Loading members").is_ok());
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
    let mut ui = sim(members::view(&state, theme::Mode::Light));
    assert!(ui.find("No validators to show").is_ok());
    ui.click(by::role(Role::Button, "Retry"))
        .expect("the empty state offers a retry");
    assert!(emitted(ui, &Message::Load));
}

#[test]
fn error_surfaces_reason_and_retry() {
    // The class of gap that hid the pages bug: the error render variant plus its
    // recovery affordance.
    let state = State {
        data: Resource::Error("valset offline".into()),
        ..State::default()
    };
    let mut ui = sim(members::view(&state, theme::Mode::Light));
    assert!(
        ui.find("valset offline").is_ok(),
        "the load-failure reason is shown, not swallowed"
    );
    ui.click(by::role(Role::Button, "Retry"))
        .expect("the error state offers a retry");
    assert!(emitted(ui, &Message::Load));
}

#[test]
fn filter_segment_emits_set_filter() {
    let state = ready();
    let mut ui = sim(members::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::Button, "Validators"))
        .expect("a filter segment is clickable");
    assert!(emitted(ui, &Message::SetFilter(Filter::Validators)));
}

#[test]
fn reveal_invite_create_affordance_emits() {
    // With no invite yet revealed, a valid join code enables the create button.
    let mut state = ready();
    if let Resource::Ready(data) = &mut state.data {
        data.invite_blob = None;
        data.invite_short = None;
    }
    state.invitee_code = RESIDENT.into(); // 64-hex → a valid join code
    let mut ui = sim(members::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::Button, "Reveal invite"))
        .expect("a valid join code enables Reveal invite");
    assert!(emitted(ui, &Message::RevealInvite));
}

#[test]
fn copy_invite_link_emits_copy() {
    // ready() carries a coordinator-minted short link; the primary copy yields it.
    let state = ready();
    let mut ui = sim(members::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::Button, "Copy link"))
        .expect("the invite copy action is clickable");
    assert!(emitted(
        ui,
        &Message::Copy {
            id: "invite".into(),
            value: "duck://short-link".into(),
        }
    ));
}

#[test]
fn resident_row_offers_promote_and_revoke() {
    // Role rendering: a resident row exposes Promote + Revoke, and the local
    // validator offers no self-Remove.
    let state = ready();
    let mut ui = sim(members::view(&state, theme::Mode::Light));
    assert!(has(&mut ui, Role::Button, "Promote"));
    assert!(has(&mut ui, Role::Button, "Revoke"));
    assert!(
        !has(&mut ui, Role::Button, "Remove"),
        "the local validator cannot demote itself"
    );
}

#[test]
fn member_row_is_a_selectable_list_item() {
    let mut state = ready();
    if let Resource::Ready(data) = &mut state.data {
        data.can_admin = false; // keep the surface compact
    }
    {
        let mut ui = sim(members::view(&state, theme::Mode::Light));
        assert!(
            has(&mut ui, Role::ListItem, "Joiner"),
            "each member row is a selectable list item"
        );
    }
    // The list-item's own click isn't reachable headless (the `sem()` probe
    // reports zero height inside the scrollable), so drive the Select it carries.
    assert_eq!(
        members::update(&mut state, Message::Select(RESIDENT.into())),
        None
    );
    assert_eq!(state.selected_key.as_deref(), Some(RESIDENT));
}

#[test]
fn detail_pane_renders_and_closes() {
    let mut state = ready();
    if let Resource::Ready(data) = &mut state.data {
        data.can_admin = false;
    }
    state.selected_key = Some(RESIDENT.into());
    let mut ui = sim(members::view(&state, theme::Mode::Light));
    assert!(
        ui.find("RUNS ON").is_ok(),
        "the selected member's detail pane renders"
    );
    ui.click(by::role(Role::Button, "×"))
        .expect("the detail pane closes");
    assert!(emitted(ui, &Message::CloseDetail));
}

#[test]
fn removing_a_validator_asks_then_confirms() {
    // The member-removal confirmation flow, driven through the view: the row
    // button only ASKS, and the confirm card commits.
    let mut state = with_peer();
    let mut ui = sim(members::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::Button, "Remove"))
        .expect("a peer validator offers Remove");
    assert!(emitted(
        ui,
        &Message::AskAction(MemberAction::Demote, PEER.into())
    ));

    members::update(&mut state, Message::AskAction(MemberAction::Demote, PEER.into()));
    assert!(state.pending.is_some(), "the ask opens a confirm card");
    let mut ui = sim(members::view(&state, theme::Mode::Light));
    assert!(has(&mut ui, Role::Button, "Cancel"));
    ui.click(by::role(Role::Button, "Remove from validators"))
        .expect("the confirm card commits the removal");
    assert!(emitted(ui, &Message::ConfirmAction));
}
