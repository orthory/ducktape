//! Settings screen: render variants, interactions, local update transitions.

use super::harness::*;
use crate::screens::settings::{
    self, Command, DangerAction, Message, Resource, ServiceEvent, SettingsData, State,
};

fn ready() -> State {
    State {
        data: Resource::Ready(SettingsData {
            client_mode: false,
            can_control_node: true,
            workspace_name: Some("demo".into()),
            network_id: Some("net-1".into()),
            active_channel: Some("general".into()),
            in_validator_set: true,
            validator_count: 3,
            roster_loaded: true,
            forget_needs_force: false,
        }),
        ..State::default()
    }
}

#[test]
fn loading_hides_controls() {
    let state = State::default();
    let mut ui = sim(settings::view(&state));
    assert!(
        !has(&mut ui, Role::Button, "Open Account"),
        "loading state must not render account controls"
    );
}

#[test]
fn ready_renders_account_controls() {
    let state = ready();
    let mut ui = sim(settings::view(&state));
    assert!(ui.find("Settings").is_ok(), "title renders");
    assert!(has(&mut ui, Role::Button, "Open account"));
}

#[test]
fn open_account_emits() {
    let state = ready();
    let mut ui = sim(settings::view(&state));
    ui.click(by::role(Role::Button, "Open account"))
        .expect("account button is clickable");
    assert!(emitted(ui, &Message::OpenAccount));
}

#[test]
fn section_titles_render() {
    // The redesign groups preferences under clearly-titled sections; lock the
    // group headings so a future refactor can't silently flatten them.
    let state = ready();
    let mut ui = sim(settings::view(&state));
    for title in ["ACCOUNT", "PREFERENCES", "NOTIFICATIONS", "NETWORK"] {
        assert!(ui.find(title).is_ok(), "section heading {title} renders");
    }
}

#[test]
fn toggle_notifications_updates_state() {
    let mut state = ready();
    assert!(state.notifications.enabled);
    let command = settings::update(&mut state, Message::ToggleNotifications);
    assert!(!state.notifications.enabled, "toggle flips the pref");
    assert!(state.saving, "toggling marks the save in flight");
    assert!(command.is_some(), "toggling asks the shell to persist");
}

#[test]
fn danger_action_requires_confirmation() {
    let mut state = ready();
    assert!(state.pending.is_none());
    let _ = settings::update(&mut state, Message::AskDanger(DangerAction::Leave));
    assert_eq!(
        state.pending,
        Some(DangerAction::Leave),
        "danger actions arm a confirmation instead of firing"
    );
}

#[test]
fn empty_state_offers_retry() {
    let state = State {
        data: Resource::Empty,
        ..State::default()
    };
    let mut ui = sim(settings::view(&state));
    assert!(ui.find("No active network").is_ok());
    ui.click(by::role(Role::Button, "Retry"))
        .expect("the empty resolved state offers a reload");
    assert!(emitted(ui, &Message::Load));
}

#[test]
fn error_state_surfaces_detail_and_retries() {
    let state = State {
        data: Resource::Error("node handshake refused".into()),
        ..State::default()
    };
    let mut ui = sim(settings::view(&state));
    assert!(ui.find("Settings unavailable").is_ok());
    assert!(
        ui.find("node handshake refused").is_ok(),
        "the backend error is shown selectable so it can be copied into a report"
    );
    ui.click(by::role(Role::Button, "Retry"))
        .expect("an errored settings load offers retry");
    assert!(emitted(ui, &Message::Load));
}

// A failed preferences write must surface inline — the class of silent-write
// regression the campaign exists to catch.
#[test]
fn save_failure_surfaces_an_inline_error() {
    let mut state = ready();
    state.saving = true;
    let command = settings::update(
        &mut state,
        Message::Service(ServiceEvent::PreferencesSaved(Err(
            "preferences write refused".into(),
        ))),
    );
    assert!(command.is_none());
    assert!(!state.saving, "the failed save clears the in-flight flag");
    let mut ui = sim(settings::view(&state));
    assert!(
        ui.find("preferences write refused").is_ok(),
        "a failed save renders a dismissible error banner"
    );
}

#[test]
fn theme_switch_toggles_the_mode() {
    // Light default → the theme switch reads OFF and is the only OFF switch
    // (every notification pref defaults ON).
    let state = ready();
    let mut ui = sim(settings::view(&state));
    ui.click(by::role(Role::Button, "OFF"))
        .expect("the theme switch is clickable");
    assert!(emitted(ui, &Message::ToggleTheme));
}

#[test]
fn accent_index_is_bounds_checked() {
    let mut state = ready();
    assert!(
        settings::update(&mut state, Message::SetAccent(5)).is_none(),
        "an out-of-range accent index is refused"
    );
    assert_eq!(state.accent, 0, "the refused index leaves the accent untouched");
    assert_eq!(
        settings::update(&mut state, Message::SetAccent(3)),
        Some(Command::SetAccent(3))
    );
    assert_eq!(state.accent, 3);
}
