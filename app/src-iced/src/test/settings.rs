//! Settings screen: render variants, interactions, local update transitions.

use super::harness::*;
use crate::screens::settings::{self, DangerAction, Message, Resource, SettingsData, State};

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
    assert!(has(&mut ui, Role::Button, "Open Account"));
}

#[test]
fn open_account_emits() {
    let state = ready();
    let mut ui = sim(settings::view(&state));
    ui.click(by::role(Role::Button, "Open Account"))
        .expect("account button is clickable");
    assert!(emitted(ui, &Message::OpenAccount));
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
