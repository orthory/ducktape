//! Terminal: the idle overlay must offer a way to start the session.

use super::harness::*;
use crate::screens::terminal::{self, Message, State};
use crate::theme;

#[test]
fn idle_offers_a_start_action() {
    // Default state models an idle (not-yet-running) session.
    let state = State::default();
    let mut ui = sim(terminal::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::Button, "Start"))
        .expect("the idle terminal overlay offers a Start button");
    assert!(emitted(ui, &Message::Start));
}
