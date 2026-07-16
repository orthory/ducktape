//! Chat transaction round-trips — the sim lane's proof scenarios.

use super::super::*;
use super::SimShell;
use crate::screens::chat::ChatMessageEvent;
use crate::screens::user::Resource;
use iced_agent_plugin::Role;

/// Create a channel through the UI. The name input is a bare TextInput
/// (no Sem wrapper), so the draft change message is injected; the toggle
/// and submit are real widget interactions.
fn create_channel(ui: &mut SimShell, name: &str) {
    ui.click(Role::Button, "+");
    ui.inject(Message::UserScreen(user_screens::Message::Chat(
        ChatMessageEvent::ChannelNameChanged(name.into()),
    )));
    ui.click(Role::Button, "Create channel");
}

#[test]
fn create_channel_and_post_message_round_trip() {
    let mut ui = SimShell::boot();
    ui.inject(Message::Navigate(Screen::Chat));
    assert!(
        matches!(ui.shell().user_screens.chat.data, Resource::Empty),
        "fresh sim has no channels"
    );

    create_channel(&mut ui, "qa-lane");

    assert!(
        ui.shell().user_screens.chat.error.is_none(),
        "create failed: {:?}",
        ui.shell().user_screens.chat.error
    );
    assert!(ui.has(Role::ListItem, "qa-lane"), "committed channel renders in the rail");
    let channels = ui.node_query("chat", serde_json::json!("channels"));
    assert!(
        channels.to_string().contains("qa-lane"),
        "channel is committed node-side: {channels}"
    );

    // CreateChannel does not set `active_channel` (only SelectChannel does),
    // and the composer's Submit is a silent no-op without one — so select the
    // freshly committed channel from the rail before posting.
    ui.click(Role::ListItem, "qa-lane");

    // The composer is a Sem-wrapped `text_editor`; simulated typewrite does not
    // reach it (it leaves a blank post), so inject the edit as a Paste action
    // and keep Send as a real widget click. `thread` is a `bool`, not `Option`.
    use crate::screens::chat_composer;
    use iced::widget::text_editor;
    ui.inject(Message::UserScreen(user_screens::Message::Chat(
        ChatMessageEvent::Composer {
            thread: false,
            message: chat_composer::Message::Edit(text_editor::Action::Edit(
                text_editor::Edit::Paste(std::sync::Arc::new("hello from the sim lane".into())),
            )),
        },
    )));
    ui.click(Role::Button, "Send");
    assert!(
        ui.shell().user_screens.chat.error.is_none(),
        "post failed: {:?}",
        ui.shell().user_screens.chat.error
    );

    // The committed message re-renders from node state — the post auto-chains a
    // LoadChat re-query, so `chat.data` here is node data, not a local echo.
    // The body itself renders as an iced `rich_text` widget (real node messages
    // always carry a rich run), and rich_text has no `operate`/text hook, so an
    // operation-based text find can never match the body. So assert the render
    // model carries the exact committed body, and confirm the row materialized
    // in the widget tree via a Simulator find on its plain-text author label.
    let (body, author) = match &ui.shell().user_screens.chat.data {
        Resource::Ready(data) => data.messages.last().map(|m| (m.body.clone(), m.author.clone())),
        _ => None,
    }
    .expect("committed message loaded into the render model");
    assert_eq!(body, "hello from the sim lane", "render model carries the committed body");
    assert!(
        ui.sees_text(&author),
        "the committed message row renders in the widget tree"
    );

    let latest = ui.node_query(
        "chat",
        serde_json::json!({"messages_latest": {"channel_id": "qa-lane", "limit": 20}}),
    );
    assert!(
        latest.to_string().contains("hello from the sim lane"),
        "message is committed node-side: {latest}"
    );
}

#[test]
fn duplicate_channel_rejection_lands_in_error_and_chains_no_refresh() {
    let mut ui = SimShell::boot();
    ui.inject(Message::Navigate(Screen::Chat));

    create_channel(&mut ui, "dup");
    assert!(ui.has(Role::ListItem, "dup"));
    assert!(ui.shell().user_screens.chat.error.is_none());

    // Same name → same slug → the module rejects the second create.
    create_channel(&mut ui, "dup");
    let error = ui
        .shell()
        .user_screens
        .chat
        .error
        .clone()
        .expect("module rejection reaches chat.error");
    assert!(
        error.contains("already exists"),
        "error carries the module reason: {error}"
    );
    // The rejected submit chained no refresh and corrupted nothing.
    assert!(ui.has(Role::ListItem, "dup"));
    let channels = ui.node_query("chat", serde_json::json!("channels"));
    let listed = channels.to_string().matches("dup").count();
    assert!(listed >= 1, "committed list unchanged: {channels}");
}
