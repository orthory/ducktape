//! Onboarding: stage rendering and the messages its controls emit.

use super::harness::*;
use crate::onboarding::{self, Message, Stage, State};
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
