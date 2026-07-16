//! Members: invite disclosure (M4) and per-row working state (M1).

use super::harness::*;
use crate::screens::members::{
    self, BoundAccount, Command, Member, MemberAction, MembersData, Message, Provider, Resource,
    State, Tier,
};
use crate::theme;

const LOCAL: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
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
