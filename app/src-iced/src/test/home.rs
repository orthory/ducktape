//! Home: account/network render variants and the emissions its controls fire.
//! `home::view` is crate-private, so render through the `user` aggregate view.

use super::harness::*;
use crate::screens::user::{
    self, Custody, CustodyStatus, HomeData, HomeMessage, Message, Screen, Standing, WorkspaceRow,
};
use crate::theme;
use crate::view_api::Resource;

fn home_data() -> HomeData {
    HomeData {
        profile: None,
        workspaces: vec![],
        devices: vec![],
        device_networks: vec![],
        member_keys: vec![],
        custody: None,
        touch_id_available: false,
        touch_id_enrolled: false,
        disconnected: false,
    }
}

fn ready(data: HomeData) -> user::State {
    let mut state = user::State::default();
    state.home.data = Resource::Ready(data);
    state
}

fn with_custody(status: CustodyStatus) -> user::State {
    let mut data = home_data();
    data.custody = Some(Custody {
        public_key: "ab".repeat(16),
        status,
    });
    ready(data)
}

#[test]
fn loading_shows_progress() {
    // HomeState defaults to Resource::Loading.
    let state = user::State::default();
    let mut ui = sim(user::view(&state, Screen::Home, theme::Mode::Light));
    assert!(
        ui.find("Loading Home…").is_ok(),
        "the loading variant names what it is doing"
    );
}

#[test]
fn empty_offers_add_network() {
    let mut state = user::State::default();
    state.home.data = Resource::Empty;
    let mut ui = sim(user::view(&state, Screen::Home, theme::Mode::Light));
    assert!(has(&mut ui, Role::Button, "+ Add network"));
    ui.click(by::role(Role::Button, "+ Add network"))
        .expect("the empty state hosts the create affordance");
    assert!(emitted(ui, &Message::Home(HomeMessage::AddNetwork)));
}

#[test]
fn error_variant_retries() {
    let mut state = user::State::default();
    state.home.data = Resource::Error("node offline".into());
    let mut ui = sim(user::view(&state, Screen::Home, theme::Mode::Light));
    assert!(ui.find("Couldn't load Home").is_ok(), "the error variant renders");
    ui.click(by::role(Role::Button, "Retry"))
        .expect("the error variant offers a retry");
    assert!(emitted(ui, &Message::Load(Screen::Home)));
}

#[test]
fn network_rows_enter_only_when_inactive() {
    let mut data = home_data();
    data.workspaces = vec![
        WorkspaceRow {
            id: "alpha".into(),
            name: "Alpha".into(),
            network_id: "net#a".into(),
            standing: Standing::Validator,
            active: true,
        },
        WorkspaceRow {
            id: "beta".into(),
            name: "Beta".into(),
            network_id: "net#b".into(),
            standing: Standing::NoSeat,
            active: false,
        },
    ];
    let state = ready(data);
    let mut ui = sim(user::view(&state, Screen::Home, theme::Mode::Light));
    assert!(
        ui.find("ACTIVE").is_ok(),
        "the active row carries a non-interactive chip, not an Enter button"
    );
    assert!(
        has(&mut ui, Role::Button, "Enter"),
        "only the inactive row offers Enter"
    );
    ui.click(by::role(Role::Button, "Enter"))
        .expect("the inactive row is enterable");
    assert!(emitted(
        ui,
        &Message::Home(HomeMessage::SwitchWorkspace("beta".into()))
    ));
}

#[test]
fn plaintext_custody_offers_set_password_and_reveal() {
    let state = with_custody(CustodyStatus::Plaintext);
    let mut ui = sim(user::view(&state, Screen::Home, theme::Mode::Light));
    ui.click(by::role(Role::Button, "Set password"))
        .expect("an unprotected account offers to set a password");
    assert!(emitted(ui, &Message::Home(HomeMessage::SecureAccount)));

    let state = with_custody(CustodyStatus::Plaintext);
    let mut ui = sim(user::view(&state, Screen::Home, theme::Mode::Light));
    ui.click(by::role(Role::Button, "Reveal recovery phrase"))
        .expect("the recovery phrase is always revealable");
    assert!(emitted(ui, &Message::Home(HomeMessage::RevealRecovery)));
}

#[test]
fn locked_custody_offers_unlock() {
    let state = with_custody(CustodyStatus::Locked);
    let mut ui = sim(user::view(&state, Screen::Home, theme::Mode::Light));
    ui.click(by::role(Role::Button, "Unlock"))
        .expect("a locked account offers unlock");
    assert!(emitted(ui, &Message::Home(HomeMessage::UnlockAccount)));
}
