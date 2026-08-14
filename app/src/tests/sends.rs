use super::*;

/// A TYPED CHARACTER COSTS ONE VIEW REBUILD, NOT TWO.
///
/// iced 0.14 rebuilds the whole UI once per message batch and has no dirty
/// check. A `keyboard press` with no `status=` fires for keys a focused widget
/// already CONSUMED, and the message it publishes cannot join the batch that
/// widget's own message is in — it leaves through the event-loop proxy and
/// comes back a turn later. So an unfiltered global key subscription charged
/// every character typed into a composer a SECOND full ChatScreen build+layout,
/// which `frame_probe`'s keystroke gate could not see: that gate drives the
/// widget's message alone.
///
/// The arbitration is mechanical, so it is pinned rather than commented. Every
/// `keyboard press` names a `status=`, and the one that takes the CAPTURED half
/// is gated on the escape ladder's OWN reading of whether a transient layer is
/// up — iced's single-line input consumes Escape, and that is the only reason
/// the captured half exists. With no layer open a captured key has nothing to
/// dismiss, which is exactly the state a reader typing into a composer is in.
///
/// Pinned as a SET, for the reason the node streams below are: a `contains` is
/// equally satisfied by a second, unfiltered subscription sitting beside the
/// right one.
#[test]
fn no_keyboard_subscription_charges_a_captured_key_to_a_bare_composer() {
    let lifecycle = inlined(include_str!("../ui/handlers/lifecycle.ice"));
    let presses: Vec<_> = lifecycle
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("keyboard press"))
        .collect();
    assert_eq!(
        presses,
        [
            "keyboard press status=ignored when (connected || palette_open) -> global_key_pressed _",
            "keyboard press key=escape status=captured when !empty(topmost_overlay(shell_tab, \
             palette_open, bell_open, channel_create_open, thread_message_action, \
             message_action, channel_settings_open, forge_repo_menu)) -> global_key_pressed _",
            "keyboard press status=ignored -> content_scroll_key _",
        ],
        "a `keyboard press` without `status=` bills every captured key a whole \
         extra view rebuild; the captured half is Escape-only (ducktape-ui#602) \
         and stays gated on an open layer"
    );
}

#[test]
fn optimistic_sends_are_independent_and_never_erase_the_next_draft() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_channel = "general".into();
    // The first draft arrives as typed composer events — the same route a
    // real keystroke takes through the rich composer — so this also pins
    // the apply half of `chat_composer_event`, not just the submit half.
    for character in "first".chars() {
        let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
            editor::ComposerEvent::Apply(editor::RichAction::Edit(
                iced::widget::text_editor::Action::Edit(iced::widget::text_editor::Edit::Insert(
                    character,
                )),
            )),
        ));
    }
    assert_eq!(composer(&app), "first");

    let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
        editor::composer_submit_event(),
    ));
    let first_id = app.messages[0].id.clone();
    let first_view_key = app.messages[0].view_key;
    assert_eq!(app.mutation_phase, MutationPhase::Idle);
    assert!(app.message_draft.is_empty());
    assert!(composer(&app).is_empty());
    assert_eq!(app.messages.len(), 1);
    assert!(app.messages[0].pending);

    app.message_editor = compose("second");
    let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
        editor::composer_submit_event(),
    ));
    let second_id = app.messages[1].id.clone();
    let second_view_key = app.messages[1].view_key;
    assert_ne!(first_id, second_id);
    assert_eq!(app.messages.len(), 2);
    assert!(app.messages.iter().all(|message| message.pending));

    app.message_editor = compose("third");
    // the submit receipt itself never touches the list…
    let _ = app.__update(__DucktapeMessage::MessageSent(backend::SendReceipt {
        operation_id: first_id.clone(),
        channel_id: "general".into(),
    }));
    assert_eq!(app.messages.len(), 2);
    assert!(app.messages.iter().all(|message| message.pending));

    // The SECOND send commits first. Root confirmation must sort by canonical
    // seq just like the thread rail while keeping that row's virtual identity.
    let mut second = message(1, "second", false);
    second.id = second_id.clone();
    let _ = app.__update(__DucktapeMessage::LiveUpdated(posted_delta(
        "general", second,
    )));
    assert_eq!(composer(&app), "third");
    assert_eq!(app.mutation_phase, MutationPhase::Idle);
    assert!(!app.messages[0].pending);
    assert_eq!(app.messages[0].seq, 1);
    assert_eq!(app.messages[0].id, second_id);
    assert_eq!(app.messages[0].view_key, second_view_key);
    assert_eq!(app.messages[1].id, first_id);
    assert_eq!(app.messages[1].view_key, first_view_key);
    assert!(app.messages[1].pending);

    // Then the first send lands at seq 2. Committed rows stay ordered, and
    // neither virtual key is replaced.
    let mut first = message(2, "first", false);
    first.id = first_id.clone();
    let _ = app.__update(__DucktapeMessage::LiveUpdated(posted_delta(
        "general", first,
    )));
    assert!(app.messages.iter().all(|message| !message.pending));
    assert_eq!(
        app.messages
            .iter()
            .map(|message| (message.id.as_str(), message.seq, message.view_key))
            .collect::<Vec<_>>(),
        [
            (second_id.as_str(), 1, second_view_key),
            (first_id.as_str(), 2, first_view_key),
        ]
    );

    // A canonical reload is allowed to replace rendered content, but not the
    // client-only identity of the same IDs.
    let reloaded = backend::merge_pending_messages(
        vec![message(1, "second", false), message(2, "first", false)]
            .into_iter()
            .zip([second_id.clone(), first_id.clone()])
            .map(|(mut message, id)| {
                message.id = id;
                message
            })
            .collect(),
        app.messages.clone(),
        "general".into(),
        "general".into(),
    );
    assert_eq!(
        reloaded
            .iter()
            .map(|message| message.view_key)
            .collect::<Vec<_>>(),
        [second_view_key, first_view_key]
    );

    let chat = inlined(include_str!("../ui/screens/chat.ice"));
    assert!(chat.contains("keyed message in messages by=message.view_key"));
    assert!(!chat.contains("keyed message in messages by=message.seq"));
    assert!(chat.contains("stack #message(message.id) w=fill"));
    assert!(!chat.contains("#message(message.seq)"));
    let externs = inlined(include_str!("../ui/extern/backend.ice"));
    assert!(externs.contains("sync optimistic_message("));
    assert!(!externs.contains("pure optimistic_message("));
}

/// A TERM IN THE GUARD THAT THE BUTTON DOES NOT WEAR IS A DEAD CONTROL.
///
/// The affordance is decided at render time and the guard runs at apply time,
/// so the guard may re-read a term — but it may not carry a term the button
/// never showed, or the click lands in a silent `return`. The two the rail's
/// MOUNT already answers are the exception: the whole plate is drawn under
/// `if active_thread_seq > 0`, and `open_thread_for` refuses an empty channel.
#[test]
fn the_reply_send_refuses_only_on_what_its_button_shows() {
    const HANDLERS: &str = include_str!("../ui/handlers/chat.ice");
    const SCREEN: &str = include_str!("../ui/screens/chat.ice");
    const ANSWERED_BY_THE_MOUNT: [&str; 2] = ["active_thread_seq <= 0", "empty(active_channel)"];

    let guard = HANDLERS
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("return if ") && line.contains("editor_text(reply_editor)"))
        .and_then(|line| line.strip_prefix("return if "))
        .expect("the reply submit guard");
    let send = SCREEN
        .lines()
        .map(str::trim)
        .find(|line| {
            line.starts_with("disabled=(thread_loading")
                && line.contains("editor_text(reply_editor)")
        })
        .and_then(|line| line.strip_prefix("disabled=("))
        .and_then(|line| line.strip_suffix(')'))
        .expect("the reply Send's disabled expression");

    let terms = |expression: &str| -> Vec<String> {
        expression
            .split("||")
            .map(|term| term.trim().to_owned())
            .collect()
    };
    let shown = terms(send);
    for term in terms(guard) {
        let on_the_button = shown.contains(&term);
        let structural = ANSWERED_BY_THE_MOUNT.contains(&term.as_str());
        assert!(
            on_the_button || structural,
            "`reply_composer_event` refuses on `{term}`, which the rail's Send does \
             not wear — put it on the button or take it out of the guard"
        );
    }
}

// THE SAME GUARD THE CHAT COMPOSER NEVER HAD. `live_resynced` rebuilt
// `message_editor` from `message_draft` — the SETTLED stash, which reads "" the
// whole time somebody is typing — so any resync emptied a half-written message:
// a `files` write in another window, a teammate joining the huddle, any plane
// op on the chain at all. Nothing writes the composer here now; it owns its own
// text and no resync produces a new one.
#[test]
fn a_resync_never_eats_the_message_being_typed() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.shell_tab = ShellTab::Chat;
    app.active_channel = "general".into();
    app.hydration_generation = 4;
    app.message_editor = compose("half a paragraph, mid-word");

    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        4,
        "general",
        vec![message(7, "somebody else posted", false)],
        "",
        Vec::new(),
    )));

    assert_eq!(
        composer(&app),
        "half a paragraph, mid-word",
        "a resync must never eat keystrokes either"
    );
    assert_eq!(
        app.messages.len(),
        1,
        "and it still installs the timeline it answered with"
    );
}

// Reconnect is the same-endpoint retry now — the picker owns endpoint
// changes — so typed drafts survive it untouched.
#[test]
fn same_endpoint_reconnect_preserves_unsent_drafts() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.connected_rpc = "http://node-a".into();
    app.message_editor = compose("next message");
    app.failed_message_draft = "unsent message".into();

    let _ = app.__update(__DucktapeMessage::Reconnect);

    assert_eq!(app.connected_rpc, "http://node-a");
    assert_eq!(composer(&app), "next message");
    assert_eq!(app.failed_message_draft, "unsent message");
}

/// SURVIVING THE RECONNECT IS NOT THE SAME AS SURVIVING IT IN THE RIGHT ROOM.
///
/// The reconnect is one room switch spread over two handlers, and that is how it
/// escaped the park: `reconnect` carries the live composer across and blanks
/// `active_channel`, then `workspace_connected` lands on
/// `landing_channel(channels)` — the first room with traffic, rarely the room
/// she left. So #private-ops' half-typed incident note stood over #general's
/// Send, and the next pick parked those words under #general's id: she found
/// #private-ops empty and her sentence filed in a room she never typed it in.
/// The rail's composer had it worse — the reconnect simply ate it.
#[test]
fn a_reconnect_lands_each_composer_in_the_room_it_was_typed_in() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.connected_rpc = "http://node".into();
    app.active_channel = "private-ops".into();
    app.active_thread_seq = 3;
    app.message_editor = compose("the incident started at");
    app.reply_editor = compose("half a reply");

    let _ = app.__update(__DucktapeMessage::Reconnect);

    let mut landed = workspace("general");
    landed.generation = app.connect_generation;
    landed.channels = vec![room("private-ops", 10), room("general", 20)];
    let _ = app.__update(__DucktapeMessage::WorkspaceConnected(landed));

    assert_eq!(
        app.active_channel, "general",
        "the connect picks the landing"
    );
    assert!(
        composer(&app).is_empty(),
        "#general's composer is #general's — the note she was writing next door \
         is not armed to send here"
    );

    app.mutation_phase = MutationPhase::Idle;
    let _ = app.__update(__DucktapeMessage::ChooseChannel("private-ops".into()));
    assert_eq!(
        composer(&app),
        "the incident started at",
        "it is waiting in the room she was writing it in"
    );

    let _ = app.__update(__DucktapeMessage::OpenThreadFor(3));
    assert_eq!(
        reply_composer(&app),
        "half a reply",
        "and the rail the reconnect closed kept its reply too"
    );
}

/// AND THE ROOM SHE LEFT DOES NOT HAUNT THE NEXT ONE AS AN "UNSENT MESSAGE".
///
/// `reconnect`'s editor harvest predated the park and left `message_draft` —
/// the settled stash — holding the LEFT room's text after the landing. Its one
/// consumer, `live_resynced`'s `remember_failed_draft(…, "channel",
/// message_draft, …)`, fires when a chat-carrying resync lands on a different
/// room, so opening a DM after reconnecting out of a room raised the
/// failed-draft plate offering to restore the old room's words into the DM
/// composer. The park owns the trip now; the stash stays empty across it.
#[test]
fn a_reconnect_does_not_leak_the_left_rooms_draft_into_the_failed_plate() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.connected_rpc = "http://node".into();
    app.active_channel = "private-ops".into();
    app.message_editor = compose("the incident started at");

    let _ = app.__update(__DucktapeMessage::Reconnect);
    let mut landed = workspace("general");
    landed.generation = app.connect_generation;
    landed.channels = vec![room("private-ops", 10), room("general", 20)];
    let _ = app.__update(__DucktapeMessage::WorkspaceConnected(landed));

    // A chat-carrying resync lands on another room — the exact trip that used
    // to stash the harvest into `failed_message_draft`.
    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        app.hydration_generation,
        "dm-with-alice",
        Vec::new(),
        "",
        Vec::new(),
    )));

    assert_eq!(app.active_channel, "dm-with-alice");
    assert!(
        app.failed_message_draft.is_empty(),
        "the room she left is parked under its own id, not offered to the room \
         she is in"
    );
}

#[test]
fn reconnect_recovers_active_drafts_for_the_same_endpoint() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.rpc = "http://node".into();
    app.connected_rpc = "http://node".into();
    app.active_page = "page".into();
    app.block_comment_draft = "unfinished comment".into();

    let _ = app.__update(__DucktapeMessage::Reconnect);

    // A half-typed COMMENT still survives a reconnect. The page body does not
    // need the same rescue: it is one buffer whose every keystroke is already
    // heading for the node on the save tick, and it is reinstalled from the
    // node's own text on the next load.
    assert_eq!(app.orphaned_comment_drafts, ["unfinished comment"]);
}

/// BOTH COMPOSERS RE-ASK THE GATE AT APPLY TIME, AND BOTH ARE PINNED HERE. A
/// composer's `disabled=` was decided a frame ago, so a channel that went
/// archived — or a members-only roster that dropped her — between the keystroke
/// and the Enter would otherwise let the send through and surface as a server
/// rejection she cannot act on. The optimistic row is the tell: it is written
/// BEFORE the request, so a refused send that still appends one has skipped the
/// gate.
#[test]
fn neither_composer_sends_into_a_channel_that_refuses_the_post() {
    // The two reasons `post_gate` names, each driven through both composers.
    for (reason, archived, members_only) in [
        ("channel_archived", true, false),
        ("members_only", false, true),
    ] {
        let (mut app, _) = Ducktape::__boot();
        app.connected = true;
        app.loading = false;
        app.active_channel = "general".into();
        app.active_channel_archived = archived;
        app.active_channel_members_only = members_only;
        // Empty roster: she is not seated, which is what `members_only` refuses.
        app.channel_members = Vec::new();
        app.settings_user_key = "me".into();
        // The gate the composers re-ask is the MIRROR every handler that moves
        // one of those four inputs writes — so the fixture writes it the same
        // way, through `post_gate` itself, and the two reasons are still real.
        app.post_refusal = backend::post_gate(
            archived,
            members_only,
            app.channel_members.clone(),
            app.settings_user_key.clone(),
        );
        assert_eq!(app.post_refusal, reason);

        app.message_editor = compose("into the void");
        let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
            editor::composer_submit_event(),
        ));
        assert!(
            app.messages.is_empty(),
            "the main composer must refuse a {reason} channel at apply time"
        );
        // The words are still hers — a refusal is not a discard.
        assert_eq!(composer(&app), "into the void");

        app.active_thread_seq = 7;
        app.reply_editor = compose("into the void");
        let _ = app.__update(__DucktapeMessage::ReplyComposerEvent(
            editor::composer_submit_event(),
        ));
        assert!(
            app.thread_messages.is_empty(),
            "the reply composer must refuse a {reason} channel at apply time"
        );
        assert_eq!(reply_composer(&app), "into the void");
    }

    // AND THE GATE IS NOT A BLANKET REFUSAL: seated in the same members-only
    // channel, both composers send. Without this the asserts above would pass
    // against a composer that refused everything.
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_channel = "general".into();
    app.active_channel_members_only = true;
    app.channel_members = vec![backend::ChatMember {
        key: "me".into(),
        label: "me".into(),
    }];
    app.settings_user_key = "me".into();
    app.post_refusal = backend::post_gate(
        false,
        true,
        app.channel_members.clone(),
        app.settings_user_key.clone(),
    );
    assert!(
        app.post_refusal.is_empty(),
        "a seated member is not refused"
    );

    app.message_editor = compose("hello");
    let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
        editor::composer_submit_event(),
    ));
    assert_eq!(app.messages.len(), 1, "a seated member still posts");

    app.active_thread_seq = 7;
    app.reply_editor = compose("hello back");
    let _ = app.__update(__DucktapeMessage::ReplyComposerEvent(
        editor::composer_submit_event(),
    ));
    assert_eq!(
        app.thread_messages.len(),
        1,
        "a seated member still replies"
    );
}

#[test]
fn every_handler_that_moves_the_caret_retires_the_composer_focus() {
    // THE BEHAVIOUR, on the route the rules are about: a claim, then a handler
    // that takes the caret with the rail still open — so neither the
    // `active_thread_seq > 0` gate nor the tab gate can save it — then the
    // chord. It must mark NEITHER draft.
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.shell_tab = ShellTab::Chat;
    app.active_channel = "general".into();
    app.active_thread_seq = 7;
    let _ = app.__update(__DucktapeMessage::ReplyComposerEvent(
        editor::ComposerEvent::Apply(editor::RichAction::Edit(
            iced::widget::text_editor::Action::Move(iced::widget::text_editor::Motion::DocumentEnd),
        )),
    ));
    let _ = app.__update(__DucktapeMessage::BeginMessageEdit(7, "hello".into(), 2));
    app.message_editor = compose("channel draft");
    app.reply_editor = compose("reply draft");
    app.reply_editor
        .perform(iced::widget::text_editor::Action::SelectAll);
    let _ = app.__update(__DucktapeMessage::GlobalKeyPressed(command_chord(
        iced::keyboard::key::Code::KeyB,
    )));
    assert_eq!(
        reply_composer(&app),
        "reply draft",
        "the caret is in the inline edit box, so Cmd+B is not a reply edit"
    );
    assert_eq!(
        composer(&app),
        "channel draft",
        "and it is not a channel edit either — a retired claim marks neither"
    );

    // THE SAME BEHAVIOUR ON THE RAIL'S OWN OPEN, which is where the VALUE of a
    // retire is load-bearing. `open_thread_for` inherits whatever the channel
    // composer claimed, and the click that opened the rail landed on a message
    // row — the caret is in NEITHER box. The rail is open, so `"reply"` is as
    // live as `"message"` here: every wrong value this one line could carry
    // marks a draft, which is why the assertion is on both drafts and not on
    // the presence of the line.
    let (mut rail, _) = Ducktape::__boot();
    rail.connected = true;
    rail.loading = false;
    rail.shell_tab = ShellTab::Chat;
    rail.active_channel = "general".into();
    let _ = rail.__update(__DucktapeMessage::ChatComposerEvent(
        editor::ComposerEvent::Apply(editor::RichAction::Edit(
            iced::widget::text_editor::Action::Move(iced::widget::text_editor::Motion::DocumentEnd),
        )),
    ));
    let _ = rail.__update(__DucktapeMessage::OpenThreadFor(7));
    rail.message_editor = compose("channel draft");
    rail.reply_editor = compose("reply draft");
    rail.message_editor
        .perform(iced::widget::text_editor::Action::SelectAll);
    let _ = rail.__update(__DucktapeMessage::GlobalKeyPressed(command_chord(
        iced::keyboard::key::Code::KeyB,
    )));
    assert_eq!(
        composer(&rail),
        "channel draft",
        "opening the rail moved the caret off the channel composer, so Cmd+B \
         is not a channel edit"
    );
    assert_eq!(
        reply_composer(&rail),
        "reply draft",
        "and the rail's own composer never had it either — the click landed on \
         a message row"
    );

    // THE ONE RAIL CLOSE NO RETIRE CAN COVER, which is the whole job of the
    // chord's `active_thread_seq > 0` term. Someone deletes the thread root
    // while you are typing a reply: `live_resynced` answers 0 for a root it
    // finds deleted (`refreshed_known_message_seq`) and the rail — with the
    // reply composer in it — is gone. That handler ALSO runs on every ordinary
    // resync while the rail stays open and you keep typing, so it cannot
    // retire unconditionally the way the user-driven teardowns do. The claim
    // survives on purpose; the READ side is what has to be honest.
    let (mut gone, _) = Ducktape::__boot();
    gone.connected = true;
    gone.loading = false;
    gone.shell_tab = ShellTab::Chat;
    gone.active_channel = "general".into();
    gone.hydration_generation = 4;
    gone.active_thread_seq = 7;
    let _ = gone.__update(__DucktapeMessage::ReplyComposerEvent(
        editor::ComposerEvent::Apply(editor::RichAction::Edit(
            iced::widget::text_editor::Action::Move(iced::widget::text_editor::Motion::DocumentEnd),
        )),
    ));
    let _ = gone.__update(__DucktapeMessage::LiveResynced(live_refresh(
        4,
        "general",
        vec![message(7, "the root", true)],
        "",
        Vec::new(),
    )));
    assert_eq!(
        gone.active_thread_seq, 0,
        "a deleted root closes the rail under the caret"
    );
    assert_eq!(
        gone.composer_focus,
        ComposerFocus::Reply,
        "and nothing retires the claim on that route — if this ever stops \
         holding, the arm below has gone vacuous and this gate needs a new pin"
    );
    // Both drafts are seated after the resync — this arm is about which box the
    // chord lands in, not about what a resync leaves in them.
    gone.message_editor = compose("channel draft");
    gone.reply_editor = compose("reply draft");
    gone.reply_editor
        .perform(iced::widget::text_editor::Action::SelectAll);
    let _ = gone.__update(__DucktapeMessage::GlobalKeyPressed(command_chord(
        iced::keyboard::key::Code::KeyB,
    )));
    assert_eq!(
        reply_composer(&gone),
        "reply draft",
        "a closed rail is never the chord's target, however stale the claim is"
    );
    assert_eq!(
        composer(&gone),
        "channel draft",
        "and a stale \"reply\" does not fall through to the channel draft either"
    );

    // THE RULES. Every handler file, so a focus mover added to a screen nobody
    // is thinking about today still has to answer.
    const HANDLERS: [(&str, &str); 11] = [
        ("chat", include_str!("../ui/handlers/chat.ice")),
        ("files", include_str!("../ui/handlers/files.ice")),
        ("forge", include_str!("../ui/handlers/forge.ice")),
        ("huddle", include_str!("../ui/handlers/huddle.ice")),
        ("lifecycle", include_str!("../ui/handlers/lifecycle.ice")),
        ("node", include_str!("../ui/handlers/node.ice")),
        ("onboarding", include_str!("../ui/handlers/onboarding.ice")),
        ("overlays", include_str!("../ui/handlers/overlays.ice")),
        ("pages", include_str!("../ui/handlers/pages.ice")),
        ("roster", include_str!("../ui/handlers/roster.ice")),
        ("shell", include_str!("../ui/handlers/shell.ice")),
    ];

    // `app.ice` is the real registry; the list above is a hand copy of it, and
    // a twelfth handler file would otherwise ship unscanned.
    for line in include_str!("../ui/app.ice").lines() {
        let Some(rest) = line.trim_start().strip_prefix("use \"handlers/") else {
            continue;
        };
        let Some(file) = rest.strip_suffix(".ice\"") else {
            continue;
        };
        assert!(
            HANDLERS.iter().any(|(scanned, _)| *scanned == file),
            "app.ice registers handlers/{file}.ice and this lint does not read \
             it — add it to HANDLERS, or the next focus mover lands there \
             unchecked"
        );
    }

    let mut moves_the_caret: Vec<String> = Vec::new();
    // Handler AND value: `composer_focus = ComposerFocus.message` in a retire is the defect
    // itself, so recording only the handler name pins nothing worth pinning.
    let mut writes_the_focus: Vec<String> = Vec::new();
    for (file, source) in HANDLERS {
        // Per FILE, not per sweep: carrying the previous file's last handler in
        // here credits it with any statement standing above the first `on `.
        let mut handler = format!("{file}::<above the first handler>");
        for line in source.lines() {
            if let Some(rest) = line.strip_prefix("on ") {
                handler = format!("{file}::{}", rest.split('(').next().unwrap_or(rest).trim());
            }
            let statement = line.trim_start();
            let takes_the_caret = statement.starts_with("task widget focus");
            let unmounts_the_tab = statement.starts_with("shell_tab = ");
            // The LITERAL zero only. A computed write (`= seq`,
            // `= next.active_thread_seq`, `= refreshed_known_message_seq(…)`)
            // may leave the rail open, so it is not a teardown and a retire
            // there would fire mid-typing; the chord's own `> 0` gate covers
            // what those can produce, and the last behaviour arm drives it.
            let closes_the_rail = statement == "active_thread_seq = 0";
            if takes_the_caret || unmounts_the_tab || closes_the_rail {
                moves_the_caret.push(handler.clone());
            }
            if let Some(value) = statement.strip_prefix("composer_focus = ") {
                writes_the_focus.push(format!("{handler} = {}", value.trim()));
            }
        }
    }
    moves_the_caret.sort();
    moves_the_caret.dedup();
    writes_the_focus.sort();
    writes_the_focus.dedup();

    let silent: Vec<&String> = moves_the_caret
        .iter()
        .filter(|mover| !writes_the_focus.contains(&format!("{mover} = ComposerFocus.unfocused")))
        .collect();
    assert!(
        silent.is_empty(),
        "these handlers move the caret (`task widget focus`), unmount the \
         composer under it (`shell_tab = `), or tear the thread rail out from \
         under it (`active_thread_seq = 0`) without RETIRING the claim on it — \
         each needs `composer_focus = ComposerFocus.unfocused`, and `unfocused` is the only \
         honest value: a mover took the caret somewhere that is not a chat \
         composer: {silent:?}"
    );

    assert_eq!(
        writes_the_focus,
        [
            "chat::arm_message_delete = ComposerFocus.unfocused",
            "chat::arm_thread_message_delete = ComposerFocus.unfocused",
            "chat::begin_message_edit = ComposerFocus.unfocused",
            "chat::begin_thread_message_edit = ComposerFocus.unfocused",
            "chat::chat_composer_event = ComposerFocus.message",
            "chat::choose_channel = ComposerFocus.unfocused",
            "chat::choose_dm = ComposerFocus.unfocused",
            "chat::close_thread = ComposerFocus.unfocused",
            "chat::open_chat_search_hit = ComposerFocus.unfocused",
            "chat::open_message_actions = ComposerFocus.unfocused",
            "chat::open_message_reactions = ComposerFocus.unfocused",
            "chat::open_thread_for = ComposerFocus.unfocused",
            "chat::open_thread_message_actions = ComposerFocus.unfocused",
            "chat::open_thread_message_reactions = ComposerFocus.unfocused",
            "chat::reply_composer_event = ComposerFocus.reply",
            "chat::toggle_channel_create = ComposerFocus.unfocused",
            "chat::toggle_channel_settings = ComposerFocus.unfocused",
            "huddle::huddle_go_channel = ComposerFocus.unfocused",
            "lifecycle::reconnect = ComposerFocus.unfocused",
            "lifecycle::select_shell_tab = ComposerFocus.unfocused",
            "onboarding::console_opened = ComposerFocus.unfocused",
            "overlays::global_key_pressed = ComposerFocus.unfocused",
            "pages::open_page_search_hit = ComposerFocus.unfocused",
            "pages::toggle_page_create = ComposerFocus.unfocused",
        ],
        "a handler started, stopped, or CHANGED what it says about the caret: \
         exactly two may CLAIM it (the two composer-event handlers, and only \
         with their own composer's name), everyone else here RETIRES it to \
         `unfocused` — decide which yours is, then update this list"
    );
}

/// One Cmd/Ctrl chord, shaped the way the keyboard subscription delivers it.
/// AN ORDINARY KEYSTROKE IS NOT A CHORD, AND MUST NOT BE CHARGED AS ONE.
/// `global_key_pressed` rides the app's ONE keyboard subscription, so it sees
/// every letter typed into a composer. Its three `editor` self-assignments each
/// lower to `mem::take(&mut self.<editor>)`, which leaves a `Content::default()`
/// behind — a fresh cosmic-text buffer built under a WRITE lock on the
/// process-global font system — so a letter used to pay three of them on the
/// literal typing path, serialized against whatever the renderer was shaping.
/// The handler now resolves all four verdicts up front and returns when the
/// press names none of them.
///
/// The saving is invisible in state (a take hands the same document straight
/// back), so the guard's POSITION is pinned in the source and its only real
/// failure mode — refusing a press that should act — is driven here, one press
/// per class the guard tests.
#[test]
fn an_inert_key_press_leaves_the_handler_before_it_rebuilds_an_editor() {
    let overlays = inlined(include_str!("../ui/handlers/overlays.ice"));
    let body = overlays
        .split_once("\non global_key_pressed(event)")
        .expect("the keyboard handler")
        .1;
    let guard = body
        .find("  return if empty(escape_key)")
        .expect("the inert-press guard");
    for take in [
        "message_editor = composer_toggle_mark(",
        "reply_editor = composer_toggle_mark(",
        "page_editor = page_history_key(",
    ] {
        let at = body.find(take).expect(take);
        assert!(
            guard < at,
            "`{take}…` takes the editor, so it must sit BELOW the inert-press guard"
        );
    }

    fn plain(code: iced::keyboard::key::Code, key: iced::keyboard::Key) -> __IceKeyPress {
        __IceKeyPress {
            key,
            modified_key: iced::keyboard::Key::Unidentified,
            physical_key: iced::keyboard::key::Physical::Code(code),
            location: iced::keyboard::Location::Standard,
            modifiers: iced::keyboard::Modifiers::empty(),
            text: None,
            repeat: false,
        }
    }
    let escape = || {
        plain(
            iced::keyboard::key::Code::Escape,
            iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
        )
    };

    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.shell_tab = ShellTab::Chat;
    app.active_channel = "general".into();
    let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
        editor::ComposerEvent::Apply(editor::RichAction::Edit(
            iced::widget::text_editor::Action::Move(iced::widget::text_editor::Motion::DocumentEnd),
        )),
    ));
    app.message_editor = compose("draft");

    // Inert: a bare letter marks nothing and opens nothing.
    let _ = app.__update(__DucktapeMessage::GlobalKeyPressed(plain(
        iced::keyboard::key::Code::KeyB,
        iced::keyboard::Key::Character("b".into()),
    )));
    assert_eq!(composer(&app), "draft", "a bare letter is not a mark");
    assert!(!app.palette_open);

    // …and every class the guard tests still gets through it.
    let _ = app.__update(__DucktapeMessage::GlobalKeyPressed(command_chord(
        iced::keyboard::key::Code::KeyB,
    )));
    assert_eq!(composer(&app), "****draft", "the chord still marks");

    let _ = app.__update(__DucktapeMessage::GlobalKeyPressed(command_chord(
        iced::keyboard::key::Code::KeyK,
    )));
    assert!(app.palette_open, "Cmd+K still opens the palette");
    let _ = app.__update(__DucktapeMessage::GlobalKeyPressed(escape()));
    assert!(!app.palette_open, "Escape still closes it");

    app.channel_settings_open = true;
    let _ = app.__update(__DucktapeMessage::GlobalKeyPressed(escape()));
    assert!(
        !app.channel_settings_open,
        "and the escape ladder still runs below the guard"
    );

    // The pages chord is the third take, so it is driven too.
    app.shell_tab = ShellTab::Pages;
    app.page_editor = iced::widget::text_editor::Content::with_text("one");
    crate::pages::history::record(|| ("".to_owned(), app.page_editor.cursor()));
    let _ = app.__update(__DucktapeMessage::GlobalKeyPressed(command_chord(
        iced::keyboard::key::Code::KeyZ,
    )));
    assert_eq!(
        app.page_editor.text(),
        "",
        "Cmd+Z on the pages tab still reaches the buffer"
    );
    crate::pages::history::reset();
}

#[test]
fn failed_optimistic_send_rolls_back_and_restores_the_draft() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_channel = "general".into();
    app.message_editor = compose("retry me");

    let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
        editor::composer_submit_event(),
    ));
    let operation_id = app.messages[0].id.clone();
    let _ = app.__update(__DucktapeMessage::MessageSendFailed(
        backend::OptimisticMutationError {
            message: "rejected".into(),
            committed: false,
            operation_id,
            scope_id: "general".into(),
            body: "retry me".into(),
        },
    ));

    assert_eq!(composer(&app), "retry me");
    assert_eq!(app.message_draft, "retry me");
    assert!(app.failed_message_draft.is_empty());
    assert!(app.messages.is_empty());
    assert_eq!(app.error, "rejected");
    assert_eq!(app.mutation_phase, MutationPhase::Idle);
}

#[test]
fn failed_send_preserves_the_next_and_unsent_drafts() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_channel = "general".into();
    app.message_editor = compose("first");

    let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
        editor::composer_submit_event(),
    ));
    let operation_id = app.messages[0].id.clone();
    app.message_editor = compose("second");
    let _ = app.__update(__DucktapeMessage::MessageSendFailed(
        backend::OptimisticMutationError {
            message: "rejected".into(),
            committed: false,
            operation_id,
            scope_id: "general".into(),
            body: "first".into(),
        },
    ));

    assert_eq!(composer(&app), "second");
    assert_eq!(app.failed_message_draft, "first");
    app.message_editor = compose("");
    let _ = app.__update(__DucktapeMessage::RestoreFailedMessage);
    assert_eq!(composer(&app), "first");
    assert_eq!(app.message_draft, "first");
    assert!(app.failed_message_draft.is_empty());
}

/// A FAILURE THAT ARRIVES AFTER SHE LEFT THE ROOM IS STILL HER TEXT.
///
/// The whole handler used to return on the room check, so a send refused while
/// she was reading another channel left no error, no unsent stash, and no row —
/// and the last thing she saw was the message sitting in the timeline. The room
/// check now scopes the timeline surgery only: the stash and the banner are
/// written above it, and the composer she is typing in NOW is not touched.
#[test]
fn a_send_that_fails_after_she_moved_rooms_still_reaches_her() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_channel = "general".into();
    app.message_editor = compose("the deploy is at 4pm");

    let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
        editor::composer_submit_event(),
    ));
    let operation_id = app.messages[0].id.clone();

    // She switches rooms while the write is in flight, and starts a new message
    // there. `choose_channel` blanks the timeline; the pending row is gone.
    let _ = app.__update(__DucktapeMessage::ChooseChannel("random".into()));
    app.message_editor = compose("different thought");

    let _ = app.__update(__DucktapeMessage::MessageSendFailed(
        backend::OptimisticMutationError {
            message: "rejected".into(),
            committed: false,
            operation_id,
            scope_id: "general".into(),
            body: "the deploy is at 4pm".into(),
        },
    ));

    assert_eq!(app.error, "rejected", "the refusal must be said out loud");
    assert_eq!(
        app.failed_message_draft, "the deploy is at 4pm",
        "and the body she typed must be recoverable, not gone"
    );
    assert_eq!(
        composer(&app),
        "different thought",
        "the composer belongs to the room she is in now — a restore here would \
         overwrite the message she is writing"
    );

    // THE SAME HOLE ON THE REPLY PATH, and wider: `close_thread` empties
    // `thread_messages`, so merely closing the rail under an in-flight reply
    // made the pending check fail and dropped the failure whole.
    let (mut rail, _) = Ducktape::__boot();
    rail.connected = true;
    rail.loading = false;
    rail.active_channel = "general".into();
    rail.active_thread_seq = 7;
    rail.reply_editor = compose("on it");
    let _ = rail.__update(__DucktapeMessage::ReplyComposerEvent(
        editor::composer_submit_event(),
    ));
    let reply_id = rail.thread_messages[0].id.clone();
    let _ = rail.__update(__DucktapeMessage::CloseThread);
    let _ = rail.__update(__DucktapeMessage::ThreadReplySendFailed(
        backend::OptimisticMutationError {
            message: "reply rejected".into(),
            committed: false,
            operation_id: reply_id,
            scope_id: "general".into(),
            body: "on it".into(),
        },
    ));

    assert_eq!(rail.error, "reply rejected");
    assert_eq!(
        rail.failed_reply_draft, "on it",
        "a closed rail is not a reason to throw the reply away"
    );
}

/// A PENDING ROW HAS NO SEQ, SO IT CANNOT ANSWER FOR THE TOP OF THE TIMELINE.
///
/// `optimistic_message` mints a descending negative seq, which sorts ahead of
/// every real message. Sorting it numerically into a prepended page put an in-flight send
/// at the top of months-old scrollback, and then `history_has_older` read
/// `-1 > 1` and hid "Load older" outright — the pending send locked the reader
/// out of her own history until it settled.
#[test]
fn a_pending_send_survives_a_history_page_without_poisoning_it() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.mutation_phase = MutationPhase::Idle;
    app.active_channel = "general".into();
    app.messages = vec![message(40, "the oldest loaded root", false)];
    app.has_older_history = true;
    app.message_editor = compose("still sending");

    let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
        editor::composer_submit_event(),
    ));
    assert!(app.messages[1].pending, "the send is in flight at the tail");

    let _ = app.__update(__DucktapeMessage::LoadMoreHistory);
    assert!(
        app.history_loading,
        "an in-flight send must not block paging"
    );
    let _ = app.__update(__DucktapeMessage::HistoryLoaded(backend::HistoryPageData {
        channel_id: "general".into(),
        messages: vec![message(20, "older", false)],
        has_more: true,
    }));

    let ordering: Vec<i64> = app.messages.iter().map(|message| message.seq).collect();
    assert_eq!(
        ordering,
        vec![20, 40, -1],
        "the page prepends, the pending row stays at the tail"
    );
    assert!(
        app.has_older_history,
        "seq 20 is not the channel's first message, so `Load older` stays live"
    );
    assert_eq!(
        backend::oldest_message_seq(app.messages.clone()),
        20,
        "and the next page is asked for from the oldest COMMITTED row"
    );
}

#[test]
fn committed_mutation_keeps_optimistic_state_until_refresh() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.connected_rpc = "http://node".into();
    app.active_channel = "general".into();
    app.message_editor = compose("committed once");

    let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
        editor::composer_submit_event(),
    ));
    let operation_id = app.messages[0].id.clone();
    let _ = app.__update(__DucktapeMessage::MessageSendFailed(
        backend::OptimisticMutationError {
            message: "read failed after commit".into(),
            committed: true,
            operation_id,
            scope_id: "general".into(),
            body: "committed once".into(),
        },
    ));

    assert!(app.message_draft.is_empty());
    assert_eq!(app.messages.len(), 1);
    assert!(app.messages[0].pending);
    assert_eq!(app.mutation_phase, MutationPhase::Idle);

    app.message_editor = compose("still available");
    let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
        editor::composer_submit_event(),
    ));
    assert_eq!(app.messages.len(), 2);
    assert_eq!(app.mutation_phase, MutationPhase::Idle);
}

#[test]
fn committed_message_change_cannot_be_submitted_twice() {
    let (mut app, _) = Ducktape::__boot();
    app.connected_rpc = "http://node".into();
    app.active_channel = "general".into();
    app.selected_message_seq = 7;
    app.selected_message_rev = 2;
    app.message_action = MessageAction::Editing;
    app.message_edit_draft = "committed edit".into();
    app.mutation_phase = MutationPhase::MessageEdit;

    let _ = app.__update(__DucktapeMessage::MutationFailed(backend::AppError {
        message: "read failed after commit".into(),
        committed: true,
    }));

    assert_eq!(app.selected_message_seq, 0);
    assert_eq!(app.selected_message_rev, 0);
    assert_eq!(app.message_action, MessageAction::Toolbar);
    assert!(app.message_edit_draft.is_empty());
    assert_eq!(app.mutation_phase, MutationPhase::Recovering);
}

/// AND "recovering" HAS A TERMINAL. It is the phase a write the node COMMITTED
/// but could not read back parks in — ordinary enough, a `/v1/query` can block
/// past the RPC timeout (#1018) — and the resync `mutation_failed` launches is
/// the recovery. Nothing released it: every other writer of "idle" sits behind
/// a `mutation_phase != MutationPhase.idle` guard it can no longer pass, so the sidebar went
/// dead (no room click, no DM, no search hit, no scrollback, no edit or delete)
/// under a titlebar stuck on "Syncing…", with Settings → Reconnect the only way
/// out and no reason for anyone to guess at it.
#[test]
fn a_committed_mutation_failure_unlocks_when_its_recovery_lands() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.loading = false;
    app.active_channel = "general".into();
    app.mutation_phase = MutationPhase::Channel;

    let _ = app.__update(__DucktapeMessage::MutationFailed(backend::AppError {
        message: "read failed after commit".into(),
        committed: true,
    }));
    assert_eq!(app.mutation_phase, MutationPhase::Recovering);

    // a resync belonging to an abandoned chain answers for nothing
    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        app.hydration_generation - 1,
        "general",
        Vec::new(),
        "",
        Vec::new(),
    )));
    assert_eq!(
        app.mutation_phase,
        MutationPhase::Recovering,
        "a stale answer is not it"
    );

    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        app.hydration_generation,
        "general",
        Vec::new(),
        "",
        Vec::new(),
    )));
    assert_eq!(
        app.mutation_phase,
        MutationPhase::Idle,
        "the state the lock protected is known good now"
    );
    assert!(app.error.is_empty());
}

/// AND THE COMPOSER IS PER-ROOM TOO — the one piece of per-room state no
/// switch handler touched.
///
/// `choose_channel` resets a dozen fields and the rail's editor, and left
/// `message_editor` exactly as it found it: half a sentence typed in
/// #private-ops followed the reader into whatever room she clicked next, sat
/// there above a live Send, and was prepended to the next thing she typed and
/// posted THERE. A chain post is permanent in history even after a tombstone
/// delete, and the leaked text is by construction from the room she just left.
///
/// The rule is the one `chat.ice` already states for a failed send — "the
/// composer belongs to the room she is in now" — finally applied to the live
/// buffer, and drafts survive the switch instead of being thrown away for it.
#[test]
fn the_composer_belongs_to_the_room_she_is_in_and_waits_in_the_one_she_left() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.mutation_phase = MutationPhase::Idle;
    app.active_channel = "private-ops".into();
    app.channels = vec![room("private-ops", 10), room("general", 20)];
    app.message_editor = compose("the incident started at");

    let _ = app.__update(__DucktapeMessage::ChooseChannel("general".into()));
    assert!(
        composer(&app).is_empty(),
        "#general's composer is #general's — nothing from next door is armed to \
         send here"
    );

    app.message_editor = compose("ok");
    let _ = app.__update(__DucktapeMessage::ChooseChannel("private-ops".into()));
    assert_eq!(
        composer(&app),
        "the incident started at",
        "and the sentence she was writing is waiting where she left it"
    );

    let _ = app.__update(__DucktapeMessage::ChooseChannel("general".into()));
    assert_eq!(composer(&app), "ok", "both rooms keep their own");

    // A SENT DRAFT DOES NOT COME BACK. The composer empties on submit, and the
    // park that runs on the way out drops the entry rather than storing "".
    // (#general has never been read here, so the switch left `loading` up.)
    app.loading = false;
    let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
        editor::composer_submit_event(),
    ));
    assert!(composer(&app).is_empty(), "the send emptied the box");
    let _ = app.__update(__DucktapeMessage::ChooseChannel("private-ops".into()));
    let _ = app.__update(__DucktapeMessage::ChooseChannel("general".into()));
    assert!(
        composer(&app).is_empty(),
        "a message she already sent must not be handed back as a draft"
    );
}

/// AND CREATING A CHANNEL IS A ROOM SWITCH, so the composer parks there too.
///
/// `channel_created` writes `active_channel = next.active_channel` — the reader
/// lands IN the room she just made, which is why `create_channel_submit`
/// abandons the old room's window load. With no park the sentence she was
/// half-way through in #private-ops arrived in #new-channel above a live Send,
/// and the NEXT switch parked it under #new-channel's id: silently
/// reattributed, and gone when she went back to #private-ops for it.
#[test]
fn creating_a_channel_leaves_the_old_rooms_draft_in_the_old_room() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.mutation_phase = MutationPhase::Idle;
    app.active_channel = "private-ops".into();
    app.channels = vec![room("private-ops", 10)];
    app.message_editor = compose("the incident started at");

    let mut created = chat_data("new-channel", Vec::new());
    created.generation = app.chat_generation;
    created.channels = vec![room("private-ops", 10), room("new-channel", 0)];
    let _ = app.__update(__DucktapeMessage::ChannelCreated(created));

    assert_eq!(
        app.active_channel, "new-channel",
        "the create lands her in it"
    );
    assert!(
        composer(&app).is_empty(),
        "and the new channel's composer is the new channel's — nothing from the \
         room she left is armed to send here"
    );

    let _ = app.__update(__DucktapeMessage::ChooseChannel("private-ops".into()));
    assert_eq!(
        composer(&app),
        "the incident started at",
        "the sentence is waiting in the room she was writing it in"
    );
}
