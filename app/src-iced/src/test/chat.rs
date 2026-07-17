//! Chat: write-failure surfacing and the create-channel flow.

use super::harness::*;
use crate::screens::chat::{
    self, Channel, ChatData, ChatLink, ChatMessage, ChatMessageEvent, ChatState, HuddleMember,
    PostPolicy,
};
use crate::screens::chat_composer;
use crate::screens::user::{Command, Message, Screen};
use crate::theme;
use crate::view_api::Resource;

fn light() -> theme::Palette {
    *theme::palette(theme::Mode::Light)
}

fn channel(id: &str, name: &str) -> Channel {
    Channel {
        id: id.into(),
        name: name.into(),
        archived: false,
        policy: PostPolicy::Open,
        owner: None,
        huddle: Vec::new(),
    }
}

fn ready(channels: Vec<Channel>, active: Option<&str>, self_key: Option<&str>) -> ChatState {
    ChatState {
        data: Resource::Ready(ChatData {
            channels,
            messages: Vec::new(),
            thread: None,
            members: Vec::new(),
            tags: Vec::new(),
            hits: Vec::new(),
            history_window: None,
            self_key: self_key.map(Into::into),
        }),
        active_channel: active.map(Into::into),
        ..Default::default()
    }
}

fn msg(sequence: u64) -> ChatMessage {
    ChatMessage {
        sequence,
        message_id: format!("m{sequence}"),
        revision: 0,
        author: "Ada".into(),
        body: "hello".into(),
        time: "09:00".into(),
        day: None,
        replies: 0,
        reactions: Vec::new(),
        author_key: None,
        edited: false,
        rich: Vec::new(),
    }
}

fn push_message(state: &mut ChatState, message: ChatMessage) {
    if let Resource::Ready(data) = &mut state.data {
        data.messages.push(message);
    }
}

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

// --- render variants -------------------------------------------------------

#[test]
fn loading_renders_a_placeholder() {
    let state = ChatState {
        data: Resource::Loading,
        ..Default::default()
    };
    let mut ui = sim(chat::view(&state, &[], light()));
    assert!(ui.find("Loading chat…").is_ok(), "the loading arm renders");
}

#[test]
fn load_error_offers_retry() {
    let state = ChatState {
        data: Resource::Error("statesync stalled".into()),
        ..Default::default()
    };
    let mut ui = sim(chat::view(&state, &[], light()));
    assert!(ui.find("Couldn't load Chat").is_ok());
    assert!(
        has(&mut ui, Role::Button, "Retry"),
        "a load failure offers a Retry, not just a dead end"
    );
    ui.click(by::role(Role::Button, "Retry"))
        .expect("retry is clickable");
    assert!(emitted(ui, &Message::Load(Screen::Chat)));
}

#[test]
fn channel_list_selects_on_click() {
    let state = ready(
        vec![channel("general", "general"), channel("random", "random")],
        None,
        None,
    );
    let mut ui = sim(chat::view(&state, &[], light()));
    assert!(has(&mut ui, Role::ListItem, "general"));
    ui.click(by::role(Role::ListItem, "random"))
        .expect("a channel row is clickable");
    assert!(emitted(
        ui,
        &Message::Chat(ChatMessageEvent::SelectChannel("random".into()))
    ));
}

#[test]
fn archived_channels_render_in_their_own_section() {
    let mut old = channel("old", "old");
    old.archived = true;
    let state = ready(vec![channel("general", "general"), old], None, None);
    let mut ui = sim(chat::view(&state, &[], light()));
    assert!(
        ui.find("ARCHIVED · 1").is_ok(),
        "archived channels get a labeled section, not silently dropped"
    );
    assert!(has(&mut ui, Role::ListItem, "old"));
}

#[test]
fn archived_active_channel_swaps_composer_for_unarchive() {
    let mut old = channel("old", "old");
    old.archived = true;
    let state = ready(vec![old], Some("old"), None);
    let mut ui = sim(chat::view(&state, &[], light()));
    assert!(
        !has(&mut ui, Role::Button, "Send"),
        "an archived channel disables posting"
    );
    ui.click(by::role(Role::Button, "Unarchive"))
        .expect("the archived bar offers Unarchive");
    assert!(emitted(
        ui,
        &Message::Chat(ChatMessageEvent::SetArchived(false))
    ));
}

// --- create form & policy toggle ------------------------------------------

#[test]
fn policy_toggle_emits_members_only() {
    let state = ChatState {
        data: Resource::Empty,
        channel_draft: "eng".into(),
        ..Default::default()
    };
    let mut ui = sim(chat::view(&state, &[], light()));
    assert!(has(&mut ui, Role::Button, "Members"));
    ui.click(by::role(Role::Button, "Members"))
        .expect("the Members policy is clickable");
    assert!(emitted(
        ui,
        &Message::Chat(ChatMessageEvent::SetPolicy(PostPolicy::MembersOnly))
    ));
}

#[test]
fn rail_plus_toggles_the_create_form() {
    let state = ChatState {
        data: Resource::Empty,
        ..Default::default()
    };
    let mut ui = sim(chat::view(&state, &[], light()));
    ui.click(by::role(Role::Button, "+"))
        .expect("the rail's new-channel + is clickable");
    assert!(emitted(
        ui,
        &Message::Chat(ChatMessageEvent::ToggleNewChannel)
    ));
}

// --- huddle affordances ----------------------------------------------------

#[test]
fn empty_huddle_offers_start() {
    let state = ready(vec![channel("general", "general")], Some("general"), None);
    let mut ui = sim(chat::view(&state, &[], light()));
    ui.click(by::role(Role::Button, "Start huddle"))
        .expect("an empty huddle offers Start");
    assert!(emitted(ui, &Message::Chat(ChatMessageEvent::JoinHuddle)));
}

#[test]
fn self_in_huddle_offers_leave_and_pop_out() {
    let mut general = channel("general", "general");
    general.huddle = vec![HuddleMember {
        user: "alice".into(),
        node: "n1".into(),
    }];
    // self_key matches the huddle member case-insensitively.
    let state = ready(vec![general], Some("general"), Some("ALICE"));
    let mut ui = sim(chat::view(&state, &[], light()));
    assert!(
        has(&mut ui, Role::Button, "Pop out"),
        "a member already in the huddle can pop it out"
    );
    ui.click(by::role(Role::Button, "Leave huddle · 1"))
        .expect("a joined member can leave");
    assert!(emitted(ui, &Message::Chat(ChatMessageEvent::LeaveHuddle)));
}

// --- channel header & details panel ---------------------------------------

#[test]
fn channel_header_opens_tags_and_details() {
    let state = ready(vec![channel("general", "general")], Some("general"), None);
    let mut ui = sim(chat::view(&state, &[], light()));
    ui.click(by::role(Role::Button, "# Tags"))
        .expect("tags is clickable");
    ui.click(by::role(Role::Button, "…"))
        .expect("details is clickable");
    let sent: Vec<_> = ui.into_messages().collect();
    assert!(sent.contains(&Message::Chat(ChatMessageEvent::LoadTags)));
    assert!(sent.contains(&Message::Chat(ChatMessageEvent::ToggleDetails)));
}

#[test]
fn details_panel_exposes_rename_and_archive() {
    let mut state = ready(vec![channel("general", "general")], Some("general"), None);
    state.show_channel_details = true;
    let mut ui = sim(chat::view(&state, &[], light()));
    assert!(has(&mut ui, Role::Button, "Rename"));
    ui.click(by::role(Role::Button, "Archive"))
        .expect("the details panel offers Archive");
    assert!(emitted(
        ui,
        &Message::Chat(ChatMessageEvent::SetArchived(true))
    ));
}

// --- message row affordances ----------------------------------------------

#[test]
fn message_offers_reply_and_reaction() {
    let mut state = ready(vec![channel("general", "general")], Some("general"), None);
    push_message(&mut state, msg(5));
    let mut ui = sim(chat::view(&state, &[], light()));
    assert!(
        has(&mut ui, Role::Button, "+ 👍"),
        "an open message offers a quick reaction"
    );
    ui.click(by::role(Role::Button, "Reply"))
        .expect("reply opens a thread");
    assert!(emitted(ui, &Message::Chat(ChatMessageEvent::OpenThread(5))));
}

#[test]
fn own_message_offers_edit_and_delete() {
    let mut mine = msg(5);
    mine.author_key = Some("ADA-KEY".into());
    // self_key matches author_key case-insensitively.
    let mut state = ready(vec![channel("general", "general")], Some("general"), Some("ada-key"));
    push_message(&mut state, mine);
    let mut ui = sim(chat::view(&state, &[], light()));
    assert!(
        has(&mut ui, Role::Button, "Delete"),
        "authors can delete their own message"
    );
    ui.click(by::role(Role::Button, "Edit"))
        .expect("authors can edit their own message");
    assert!(emitted(
        ui,
        &Message::Chat(ChatMessageEvent::BeginEdit(5, 0, "hello".into()))
    ));
}

// --- composer transitions & guards ----------------------------------------

#[test]
fn composer_send_button_emits_submit() {
    let mut state = ready(vec![channel("general", "general")], Some("general"), None);
    state.draft = "hi ducks".into();
    let mut ui = sim(chat::view(&state, &[], light()));
    ui.click(by::role(Role::Button, "Send"))
        .expect("a non-blank draft can be sent");
    assert!(emitted(
        ui,
        &Message::Chat(ChatMessageEvent::Composer {
            thread: false,
            message: chat_composer::Message::Submit,
        })
    ));
}

#[test]
fn composer_submit_without_channel_is_refused() {
    let mut state = ChatState {
        active_channel: None,
        ..Default::default()
    };
    state.draft = "orphan".into();
    let command = chat::update(
        &mut state,
        ChatMessageEvent::Composer {
            thread: false,
            message: chat_composer::Message::Submit,
        },
    );
    assert!(command.is_none(), "no active channel ⇒ nowhere to send");
    assert_eq!(state.draft, "orphan", "the unsendable draft is not lost");
}

#[test]
fn thread_submit_without_a_loaded_thread_is_refused() {
    let mut state = ready(vec![channel("general", "general")], Some("general"), None);
    state.reply_draft = "reply".into();
    let command = chat::update(
        &mut state,
        ChatMessageEvent::Composer {
            thread: true,
            message: chat_composer::Message::Submit,
        },
    );
    assert!(command.is_none());
    assert_eq!(state.reply_draft, "reply", "the reply text survives a no-op submit");
}

#[test]
fn attachment_choose_is_ignored_while_busy() {
    let state = &mut ChatState {
        attachment_busy: true,
        ..Default::default()
    };
    let command = chat::update(
        state,
        ChatMessageEvent::Composer {
            thread: false,
            message: chat_composer::Message::ChooseAttachment,
        },
    );
    assert!(
        command.is_none(),
        "a second attach while one is uploading is dropped"
    );
}

// --- selection & link routing (data-protection transitions) ---------------

#[test]
fn selecting_a_channel_drops_a_pending_delete() {
    let mut state = ready(
        vec![channel("general", "general"), channel("random", "random")],
        Some("general"),
        None,
    );
    state.pending_delete = Some(7);
    state.reply_draft = "half-typed".into();
    let command = chat::update(&mut state, ChatMessageEvent::SelectChannel("random".into()));
    assert_eq!(command, Some(Command::LoadChannel("random".into())));
    assert_eq!(state.active_channel.as_deref(), Some("random"));
    assert!(
        state.pending_delete.is_none(),
        "a delete confirm can't survive a channel switch — it would delete in the wrong channel"
    );
    assert!(state.reply_draft.is_empty());
    assert_eq!(
        state.rename_draft, "random",
        "the rename field pre-fills with the newly-selected channel's name"
    );
}

#[test]
fn open_link_routes_by_kind() {
    let mut state = ready(vec![channel("general", "general")], Some("general"), None);
    assert_eq!(
        chat::update(
            &mut state,
            ChatMessageEvent::OpenLink(ChatLink::File {
                path: "duck://file/notes.txt".into(),
                name: "notes.txt".into(),
            })
        ),
        Some(Command::DownloadChatAttachment("duck://file/notes.txt".into()))
    );
    assert_eq!(
        chat::update(
            &mut state,
            ChatMessageEvent::OpenLink(ChatLink::Channel {
                id: "random".into(),
                sequence: Some(9),
            })
        ),
        Some(Command::LoadMessageWindow {
            channel: "random".into(),
            sequence: 9,
        })
    );
    assert_eq!(
        state.active_channel.as_deref(),
        Some("random"),
        "a channel link switches the active channel"
    );
    assert!(
        chat::update(&mut state, ChatMessageEvent::OpenLink(ChatLink::User("u".into())))
            .is_none(),
        "an unrouted link kind is a no-op"
    );
}
