//! Chat: write-failure surfacing and the create-channel flow.

use super::harness::*;
use crate::screens::chat::{self, ChatMessageEvent, ChatState, PostPolicy};
use crate::screens::user::{Command, Message};
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

// B1: the reported "create is broken" — an empty workspace landed the user on a
// CTA with no input or button (the only create path was the rail's 20px `+`).
// The empty-state center must now host the full create form.
#[test]
fn empty_state_center_offers_a_create_form() {
    let state = ChatState {
        data: Resource::Empty,
        channel_draft: "release".into(),
        ..Default::default()
    };
    let p = *theme::palette(theme::Mode::Light);
    let mut ui = sim(chat::view(&state, &[], p));
    assert!(
        has(&mut ui, Role::Button, "Create channel"),
        "the empty-state center must render a Create channel button"
    );
    ui.click(by::role(Role::Button, "Create channel"))
        .expect("the center create button is clickable");
    assert!(emitted(ui, &Message::Chat(ChatMessageEvent::CreateChannel)));
}

// M2 + P2: creating a channel selects it up front (so the follow-up reload
// enters the new channel, not the previously-active one) and resets the draft
// policy back to Open for the next create.
#[test]
fn create_channel_selects_new_channel_and_resets_policy() {
    let mut state = ChatState {
        channel_draft: "Release Planning".into(),
        channel_policy: PostPolicy::MembersOnly,
        active_channel: Some("general".into()),
        ..Default::default()
    };
    let command = chat::update(&mut state, ChatMessageEvent::CreateChannel);
    assert_eq!(
        state.active_channel.as_deref(),
        Some("release-planning"),
        "the new channel is selected, replacing the previously-active one"
    );
    assert!(
        matches!(
            command,
            Some(Command::CreateChannel {
                policy: PostPolicy::MembersOnly,
                ..
            })
        ),
        "the emitted command carries the chosen Members policy"
    );
    assert_eq!(
        state.channel_policy,
        PostPolicy::Open,
        "the draft policy resets to Open for the next create"
    );
}

// M2: a name with no ASCII alphanumerics slugs to "" — refuse the create rather
// than mint an unaddressable channel (matches the original channelIdOf bail).
#[test]
fn create_channel_refuses_an_unsluggable_name() {
    let mut state = ChatState {
        channel_draft: "한글".into(),
        ..Default::default()
    };
    assert!(chat::update(&mut state, ChatMessageEvent::CreateChannel).is_none());
    assert!(state.active_channel.is_none());
}
