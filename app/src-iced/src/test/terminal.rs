//! Terminal: the session-lifecycle overlays (idle/starting/reconnecting/failed/
//! live) and their affordances, plus the shared-mode command sender.

use super::harness::*;
use crate::screens::terminal::{self, Message, SessionMode, State};
use crate::theme;

/// Drive to Starting (via Start), returning the live generation so callers can
/// forge the service events (Connected/Failed/Reconnecting) the shell would send.
fn started() -> (State, u64) {
    let mut state = State::default();
    terminal::update(&mut state, Message::Start);
    let generation = state.generation();
    (state, generation)
}

#[test]
fn idle_offers_a_start_action() {
    // Default state models an idle (not-yet-running) session.
    let state = State::default();
    let mut ui = sim(terminal::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::Button, "Start"))
        .expect("the idle terminal overlay offers a Start button");
    assert!(emitted(ui, &Message::Start));
}

#[test]
fn starting_hides_the_actions() {
    let (state, _) = started();
    let mut ui = sim(terminal::view(&state, theme::Mode::Light));
    assert!(ui.find("starting codex session…").is_ok());
    assert!(!has(&mut ui, Role::Button, "Start"));
    assert!(!has(&mut ui, Role::Button, "Retry"));
}

#[test]
fn failed_surfaces_detail_and_retries() {
    let (mut state, generation) = started();
    terminal::update(
        &mut state,
        Message::Failed {
            generation,
            detail: "codex exited: 137".into(),
        },
    );
    let mut ui = sim(terminal::view(&state, theme::Mode::Light));
    assert!(
        ui.find("codex exited: 137").is_ok(),
        "the failure detail is surfaced, not a generic string"
    );
    ui.click(by::role(Role::Button, "Retry"))
        .expect("a failed session offers Retry");
    assert!(emitted(ui, &Message::Retry));
}

#[test]
fn reconnecting_shows_a_notice_without_actions() {
    let (mut state, generation) = started();
    terminal::update(
        &mut state,
        Message::Reconnecting {
            generation,
            detail: "socket closed".into(),
        },
    );
    let mut ui = sim(terminal::view(&state, theme::Mode::Light));
    assert!(ui.find("reconnecting terminal session…").is_ok());
    assert!(!has(&mut ui, Role::Button, "Retry"));
    assert!(!has(&mut ui, Role::Button, "Start"));
}

#[test]
fn live_clears_the_overlay() {
    let (mut state, generation) = started();
    terminal::update(&mut state, Message::Connected { generation });
    let mut ui = sim(terminal::view(&state, theme::Mode::Light));
    assert!(!has(&mut ui, Role::Button, "Start"));
    assert!(!has(&mut ui, Role::Button, "Retry"));
    assert!(
        ui.find("codex session is not running").is_err(),
        "a live session shows only the terminal, no idle overlay"
    );
}

#[test]
fn shared_mode_shows_the_command_panel() {
    let mut state = State::default();
    terminal::update(&mut state, Message::SetMode(SessionMode::Shared));
    let mut ui = sim(terminal::view(&state, theme::Mode::Light));
    assert!(
        has(&mut ui, Role::Button, "Send"),
        "shared mode adds an ordered-command sender"
    );
    assert!(ui.find("No commands yet — the ordered log appears here.").is_ok());
}

#[test]
fn mode_tab_switches_session() {
    let state = State::default(); // Single by default
    let mut ui = sim(terminal::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::Tab, "Shared"))
        .expect("the header offers a Shared mode tab");
    assert!(emitted(ui, &Message::SetMode(SessionMode::Shared)));
}
