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

    // Selection-wise this click is redundant — ChatLoaded auto-selects the
    // first non-archived channel — but it drives the real rail path
    // (SelectChannel → LoadChannel), which is coverage the lane wants.
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

/// Post `body` through the composer + Send button, asserting the submit
/// committed (chat.error stays clear). Same path as the single-post block in
/// `create_channel_and_post_message_round_trip`: `typewrite` never reaches the
/// `text_editor`, so the edit is injected as a Paste; Send is a real click. The
/// composer clears on Submit, so consecutive calls don't concatenate.
fn post_message(ui: &mut SimShell, body: &str) {
    use crate::screens::chat_composer;
    use iced::widget::text_editor;
    ui.inject(Message::UserScreen(user_screens::Message::Chat(
        ChatMessageEvent::Composer {
            thread: false,
            message: chat_composer::Message::Edit(text_editor::Action::Edit(
                text_editor::Edit::Paste(std::sync::Arc::new(body.into())),
            )),
        },
    )));
    ui.click(Role::Button, "Send");
    assert!(
        ui.shell().user_screens.chat.error.is_none(),
        "post {body:?} failed: {:?}",
        ui.shell().user_screens.chat.error
    );
}

#[test]
fn two_messages_render_in_posting_order() {
    let mut ui = SimShell::boot();
    ui.inject(Message::Navigate(Screen::Chat));

    create_channel(&mut ui, "order-lane");
    assert!(
        ui.shell().user_screens.chat.error.is_none(),
        "create failed: {:?}",
        ui.shell().user_screens.chat.error
    );
    // Drive the real rail path (SelectChannel → LoadChannel), same as the
    // single-post exemplar.
    ui.click(Role::ListItem, "order-lane");

    post_message(&mut ui, "first message");
    post_message(&mut ui, "second message");

    // UI side: both posts re-render from node state (each Send auto-chains a
    // LoadChat re-query, so `chat.data` is node data, not a local echo). The
    // render model maps `messages_latest` 1:1, which is ascending by `seq`, so
    // the two entries are the two posts in posting order. Bodies render as
    // `rich_text` (no text finder hook), so assert the render model directly.
    let rendered: Vec<(u64, String)> = match &ui.shell().user_screens.chat.data {
        Resource::Ready(data) => {
            data.messages.iter().map(|m| (m.sequence, m.body.clone())).collect()
        }
        other => panic!("channel data not loaded into the render model: {other:?}"),
    };
    assert_eq!(
        rendered.iter().map(|(_, body)| body.as_str()).collect::<Vec<_>>(),
        vec!["first message", "second message"],
        "both posts render, in posting order: {rendered:?}"
    );
    assert!(
        rendered[0].0 < rendered[1].0,
        "the second post carries the higher seq: {rendered:?}"
    );
    // Confirm both rows materialized in the widget tree via their plain-text
    // author labels (the body's rich_text can't be found by text).
    let author = match &ui.shell().user_screens.chat.data {
        Resource::Ready(data) => data.messages[0].author.clone(),
        _ => unreachable!(),
    };
    assert!(ui.sees_text(&author), "the committed message rows render in the widget tree");

    // Node side: `messages_latest` is ascending by seq, so the committed bodies
    // appear in posting order in the serialized reply.
    let latest = ui.node_query(
        "chat",
        serde_json::json!({"messages_latest": {"channel_id": "order-lane", "limit": 20}}),
    );
    let text = latest.to_string();
    let first_at = text.find("first message").expect("first post committed node-side");
    let second_at = text.find("second message").expect("second post committed node-side");
    assert!(
        first_at < second_at,
        "node-side order is posting order: {latest}"
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
    let listed = channels["channels"].as_array().map_or(0, Vec::len);
    assert_eq!(listed, 1, "committed list unchanged — exactly one channel: {channels}");
}
