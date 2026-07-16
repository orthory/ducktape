//! Browser chrome: the icon buttons stay wired after the glyph-centering
//! rebuild, and the trust chip reflects the committed host.

use super::harness::*;
use crate::browser_chrome::{self, Message, State};
use crate::theme::Mode;

fn commit(state: &mut State, url: &str) {
    browser_chrome::commit_navigation(state, 0, url).expect("a valid duck route commits");
}

#[test]
fn nav_controls_stay_wired_after_centering_rebuild() {
    let mut state = State::default();
    commit(&mut state, "duck://net.duck/docs");
    commit(&mut state, "duck://net.duck/reference");

    let mut ui = sim(browser_chrome::view(&state, Mode::Light, true));
    ui.click(by::role(Role::Button, "Back")).expect("Back button");
    assert!(emitted(ui, &Message::Back));

    let mut ui = sim(browser_chrome::view(&state, Mode::Light, true));
    ui.click(by::role(Role::Button, "Reload")).expect("Reload button");
    assert!(emitted(ui, &Message::Reload));

    let mut ui = sim(browser_chrome::view(&state, Mode::Light, true));
    ui.click(by::role(Role::Button, "New tab")).expect("New tab button");
    assert!(emitted(ui, &Message::NewTab));
}

#[test]
fn trust_chip_marks_network_snapshot_and_account_signed() {
    let mut network = State::default();
    commit(&mut network, "duck://net.duck/docs");
    let mut ui = sim(browser_chrome::view(&network, Mode::Light, true));
    assert!(has(&mut ui, Role::Label, "SNAPSHOT"));
    assert!(!has(&mut ui, Role::Label, "SIGNED"));

    let mut account = State::default();
    commit(&mut account, "duck://app.demo.duck/start");
    let mut ui = sim(browser_chrome::view(&account, Mode::Light, true));
    assert!(has(&mut ui, Role::Label, "SIGNED"));

    // Idle (nothing committed yet) shows no trust chip.
    let idle = State::default();
    let mut ui = sim(browser_chrome::view(&idle, Mode::Light, true));
    assert!(!has(&mut ui, Role::Label, "SNAPSHOT"));
    assert!(!has(&mut ui, Role::Label, "SIGNED"));
}
