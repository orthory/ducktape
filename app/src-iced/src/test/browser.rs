//! Browser chrome: the icon buttons stay wired after the glyph-centering
//! rebuild.

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
