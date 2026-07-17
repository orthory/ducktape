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

fn chrome(state: &State) -> iced_test::Simulator<'_, Message> {
    sim(browser_chrome::view(state, Mode::Light, true))
}

// A fresh tab has no history and hasn't committed a page: Back / Forward /
// Reload render but are inert (the disabled button drops its on_press), so a
// greyed control never fires.
#[test]
fn idle_chrome_gates_history_and_reload() {
    for (label, message) in [
        ("Back", Message::Back),
        ("Forward", Message::Forward),
        ("Reload", Message::Reload),
    ] {
        let state = State::default();
        let mut ui = chrome(&state);
        assert!(has(&mut ui, Role::Button, label), "{label} renders");
        ui.click(by::role(Role::Button, label))
            .expect("the control is present");
        assert!(
            !emitted(ui, &message),
            "{label} must be inert on an idle tab"
        );
    }
}

// Forward is gated at the history head, and only becomes live once Back has
// opened room ahead.
#[test]
fn forward_enables_after_going_back() {
    let mut state = State::default();
    commit(&mut state, "duck://net.duck/docs");
    commit(&mut state, "duck://net.duck/reference");

    let mut ui = chrome(&state);
    ui.click(by::role(Role::Button, "Forward"))
        .expect("Forward is present");
    assert!(!emitted(ui, &Message::Forward), "Forward is inert at head");

    browser_chrome::update(&mut state, Message::Back);
    let mut ui = chrome(&state);
    ui.click(by::role(Role::Button, "Forward"))
        .expect("Forward is live after Back");
    assert!(emitted(ui, &Message::Forward));
}

#[test]
fn close_tab_emits_close() {
    let state = State::default();
    let mut ui = chrome(&state);
    ui.click(by::role(Role::Button, "Close tab"))
        .expect("each tab carries a close control");
    assert!(emitted(ui, &Message::CloseTab(0)));
}

// The first (inactive) tab is a selectable target that switches to its index.
#[test]
fn inactive_tab_is_selectable() {
    let mut state = State::default();
    browser_chrome::update(&mut state, Message::NewTab);
    assert_eq!(state.active_tab, 1, "the new tab is the active one");

    let mut ui = chrome(&state);
    // Both tabs share the default "net.duck" name; find returns the first, tab 0.
    ui.click(by::role(Role::Tab, "net.duck"))
        .expect("the tab strip renders selectable tabs");
    assert!(emitted(ui, &Message::SelectTab(0)));
}

// A refused route surfaces as a readable alert card whose reason is selectable
// for copy, not a silent no-op.
#[test]
fn refused_route_renders_a_readable_alert() {
    let state = State {
        error: Some("Browser accepts only signed .duck routes.".into()),
        ..State::default()
    };
    let mut ui = chrome(&state);
    assert!(ui.find("Route refused").is_ok());
    assert!(
        ui.find("Browser accepts only signed .duck routes.").is_ok(),
        "the refusal reason is shown and selectable"
    );
}

#[test]
fn address_bar_is_a_semantic_input() {
    let state = State::default();
    let mut ui = chrome(&state);
    assert!(has(&mut ui, Role::TextInput, "Address"));
}
