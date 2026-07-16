//! Onboarding: stage rendering and the messages its controls emit.

use super::harness::*;
use crate::onboarding::{self, EntryMode, Message, Stage, State};
use crate::theme;

fn at(stage: Stage) -> State {
    // State implements Drop (zeroize), so no struct-update syntax. The
    // default state models the boot probe (stage Loading, busy) — a stage
    // fixture is an interactive screen, so clear the busy flag.
    let mut state = State::default();
    state.stage = stage;
    state.busy = false;
    state
}

#[test]
fn create_stage_renders_the_form() {
    let state = at(Stage::Create);
    let mut ui = sim(onboarding::view(&state, theme::Mode::Light));
    assert!(ui.find("Create your account").is_ok(), "headline renders");
    assert!(
        has(&mut ui, Role::Button, "Create account"),
        "primary action renders"
    );
}

#[test]
fn create_submit_emits() {
    let mut state = at(Stage::Create);
    // The primary button enables once credentials are plausible.
    state.password = "correct horse battery".into();
    state.confirm_password = state.password.clone();
    let mut ui = sim(onboarding::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::Button, "Create account"))
        .expect("create is clickable once passwords are set");
    assert!(emitted(ui, &Message::Submit));
}

#[test]
fn reveal_legacy_requires_a_password() {
    let state = at(Stage::RevealLegacy);
    let mut ui = sim(onboarding::view(&state, theme::Mode::Light));
    assert!(
        has(&mut ui, Role::Button, "View recovery phrase"),
        "the reveal form renders its submit"
    );

    // Submitting with no password is refused with an error, not sent to the
    // backend as an empty secret that can never succeed.
    let mut blank = at(Stage::RevealLegacy);
    assert!(onboarding::update(&mut blank, Message::Submit).is_none());
    assert!(blank.error.is_some(), "empty password is flagged");

    // A typed password drives the reveal.
    let mut ready = at(Stage::RevealLegacy);
    ready.password = "correct horse battery".into();
    assert!(
        onboarding::update(&mut ready, Message::Submit).is_some(),
        "a password lets the reveal proceed"
    );
}

#[test]
fn mode_switch_is_inert_while_busy() {
    // ON-G1: a mode tab clicked mid-custody-op must not wipe the in-flight
    // secret or drop to another stage — otherwise the late ServiceEvent lands
    // in the wrong stage.
    let mut state = at(Stage::Create);
    state.busy = true;
    state.password = "in flight".into();
    assert!(
        onboarding::update(&mut state, Message::SelectMode(EntryMode::Restore)).is_none(),
        "SelectMode is a no-op while busy"
    );
    assert_eq!(state.stage, Stage::Create, "stage is unchanged");
    assert_eq!(state.mode, EntryMode::Create, "mode is unchanged");
    assert!(state.busy, "the in-flight flag survives");
    assert!(!state.password.is_empty(), "the in-flight secret is not wiped");
}

#[test]
fn busy_tabs_do_not_switch_mode() {
    // ON-G6: the tab is still rendered while busy (so the block is visible) but
    // carries no on_press, so clicking it emits nothing.
    let mut state = at(Stage::Create);
    state.busy = true;
    let mut ui = sim(onboarding::view(&state, theme::Mode::Light));
    let _ = ui.click(by::role(Role::Tab, "Restore"));
    assert!(!emitted(ui, &Message::SelectMode(EntryMode::Restore)));
}

#[test]
fn password_field_is_addressable_by_the_bridge() {
    // ON-3a: every onboarding input carries a Sem TextInput node so the agent
    // bridge can focus + type. Secrets omit their value from the tree but stay
    // drivable by name.
    let state = at(Stage::Create);
    let mut ui = sim(onboarding::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::TextInput, "Password"))
        .expect("the password field is addressable by name");
    ui.typewrite("s");
    assert!(
        emitted(ui, &Message::PasswordChanged("s".into())),
        "typing into the tagged field drives the reducer"
    );
}

#[test]
fn busy_create_is_disabled() {
    let mut state = at(Stage::Create);
    state.password = "correct horse battery".into();
    state.confirm_password = state.password.clone();
    state.busy = true;
    let mut ui = sim(onboarding::view(&state, theme::Mode::Light));
    assert!(
        !has(&mut ui, Role::Button, "Create account"),
        "busy stage swaps the label"
    );
    assert!(
        has(&mut ui, Role::Button, "Creating…"),
        "busy label renders instead"
    );
}
