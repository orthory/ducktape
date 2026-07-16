//! Chat: write-failure surfacing.

use super::harness::*;
use crate::screens::chat::{self, ChatMessageEvent, ChatState};
use crate::screens::user::Message;
use crate::theme;
use crate::view_api::Resource;

#[test]
fn write_failure_shows_a_dismissible_error() {
    let state = ChatState {
        data: Resource::Empty,
        error: Some("send failed: node offline".into()),
        ..Default::default()
    };
    let p = *theme::palette(theme::Mode::Light);
    let mut ui = sim(chat::view(&state, &[], p));
    assert!(
        has(&mut ui, Role::Button, "Dismiss"),
        "a chat write failure must render an inline error the user can dismiss"
    );
    ui.click(by::role(Role::Button, "Dismiss"))
        .expect("dismiss is clickable");
    assert!(emitted(ui, &Message::Chat(ChatMessageEvent::DismissError)));
}
