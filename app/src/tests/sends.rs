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
             message_action, channel_settings_open, page_delete_armed, forge_repo_menu)) -> \
             global_key_pressed _",
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
    // The first draft arrives as typed composer events into the ROOM'S OWN
    // instance — the same route a real keystroke takes through the rich
    // composer — so this pins the instance's whole cycle: it collects the
    // keystrokes, clears itself on submit, and only then hands the body up.
    let composer = composer_scope(&mut app);
    type_into(&mut app, &composer, ComposerKind::Message, "first");
    assert_eq!(composer_text(&app, &composer), "first");

    submit_composer(&mut app, &composer, ComposerKind::Message, false);
    let first_id = app.messages[0].id.clone();
    let first_view_key = app.messages[0].view_key;
    assert_eq!(app.mutation_phase, MutationPhase::Idle);
    assert!(
        composer_text(&app, &composer).is_empty(),
        "the instance clears itself before it emits"
    );
    assert_eq!(app.messages.len(), 1);
    assert!(app.messages[0].pending);

    submit(&mut app, ComposerKind::Message, "second");
    let second_id = app.messages[1].id.clone();
    let second_view_key = app.messages[1].view_key;
    assert_ne!(first_id, second_id);
    assert_eq!(app.messages.len(), 2);
    assert!(app.messages.iter().all(|message| message.pending));

    type_into(&mut app, &composer, ComposerKind::Message, "third");
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
    assert_eq!(composer_text(&app, &composer), "third");
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
/// never showed, or the click lands in a silent `return`.
///
/// THE DESCENT MADE HALF OF THIS STRUCTURAL (ducktape-ui#697): the instance's
/// own refusal is not a re-derivation at all, it is the very expression the
/// frame drew, handed down the route as `blocked` — button and guard cannot
/// disagree because they are one value. What is left to police is the APP's
/// re-read at delivery, which runs a frame later against fresh state: it may
/// re-read the mount's terms, and nothing else. A term the mount never showed
/// would refuse a send the reader was invited to make — and since the instance
/// has already cleared itself by then, the words would only survive because
/// the arm stashes them.
#[test]
fn the_delivery_re_read_refuses_only_on_what_the_mount_showed() {
    const HANDLERS: &str = include_str!("../ui/handlers/chat.ice");
    const SCREEN: &str = include_str!("../ui/screens/chat.ice");

    // The rail's plate is drawn under `if active_thread_seq > 0` and
    // `open_thread_for` refuses an empty channel, so its re-read may name
    // both without the mount wearing them — the STRUCTURE shows them.
    const ANSWERED_BY_THE_MOUNT: [&str; 2] = ["active_thread_seq <= 0", "empty(active_channel)"];

    // Arguments split at top-level commas, so `post_gate(a, b)` stays whole.
    fn split_top(source: &str) -> Vec<&str> {
        let mut parts = Vec::new();
        let mut depth = 0usize;
        let mut start = 0usize;
        for (index, character) in source.char_indices() {
            match character {
                '(' => depth += 1,
                ')' => depth = depth.saturating_sub(1),
                ',' if depth == 0 => {
                    parts.push(source[start..index].trim());
                    start = index + 1;
                }
                _ => {}
            }
        }
        parts.push(source[start..].trim());
        parts
    }

    // A term is an `||` operand with its own balanced parens; only wrapping
    // parens come off, so `empty(active_channel)` survives whole.
    let terms = |expression: &str| -> Vec<String> {
        expression
            .split("||")
            .map(|term| {
                let mut term = term.trim();
                while term.starts_with('(')
                    && term.ends_with(')')
                    && term[1..term.len() - 1].matches('(').count()
                        == term[1..term.len() - 1].matches(')').count()
                {
                    term = term[1..term.len() - 1].trim();
                }
                term.to_owned()
            })
            .collect()
    };
    // The mount's `blocked=` and the arm's `let refused =`, in mount order.
    let shown: Vec<Vec<String>> = SCREEN
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix("blocked=("))
        .filter_map(|line| line.strip_suffix(')'))
        .map(terms)
        .collect();
    // The re-read is a VERDICT now, computed once from the same four inputs
    // the mount's gate wears — so the lint reads its arguments rather than a
    // hand-written `||` chain.
    let refused: Vec<Vec<String>> = HANDLERS
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix("match submit_verdict("))
        .filter_map(|line| line.strip_suffix(')'))
        .map(|arguments| {
            split_top(arguments)
                .into_iter()
                .enumerate()
                .filter_map(|(index, argument)| match index {
                    // busy, connected, channel, refusal, seated — spelled as
                    // the terms a mount's `blocked=` would carry.
                    0 => Some(argument.to_owned()),
                    1 => Some(format!("!{argument}")),
                    2 => Some(format!("empty({argument})")),
                    3 => Some(format!("!empty({argument})")),
                    // `seated` is the mount's own structural term, spelled
                    // positively here and negatively on the gate.
                    4 if argument != "true" => Some(argument.replace(" > 0", " <= 0")),
                    _ => None,
                })
                .collect()
        })
        .collect();
    assert_eq!(
        shown.len(),
        2,
        "two composers are mounted, each with its own gate"
    );
    assert_eq!(
        refused.len(),
        shown.len(),
        "every mounted composer's submit is re-read at delivery"
    );

    for (arm, (shown, refused)) in shown.iter().zip(&refused).enumerate() {
        for term in refused {
            assert!(
                shown.contains(term) || ANSWERED_BY_THE_MOUNT.contains(&term.as_str()),
                "the delivery re-read of composer {arm} refuses on `{term}`, which                  its mount's `blocked=` does not wear — put it on the mount or                  take it out of the re-read"
            );
        }
    }
}

// THE SAME GUARD THE CHAT COMPOSER NEVER HAD. `live_resynced` rebuilt the
// composer from `message_draft` — the SETTLED stash, which reads "" the whole
// time somebody is typing — so any resync emptied a half-written message: a
// `files` write in another window, a teammate joining the huddle, any plane op
// on the chain at all. The composer is its room's own instance now, so no
// handler can write it at all; this drives the promise anyway, because a
// resync landing on the room is exactly when a future refactor would reach.
#[test]
fn a_resync_never_eats_the_message_being_typed() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.shell_tab = ShellTab::Chat;
    app.active_channel = "general".into();
    app.hydration_generation = 4;
    let composer = composer_scope(&mut app);
    type_into(
        &mut app,
        &composer,
        ComposerKind::Message,
        "half a paragraph, mid-word",
    );

    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        4,
        "general",
        vec![message(7, "somebody else posted", false)],
        "",
        Vec::new(),
    )));

    assert_eq!(
        composer_text(&app, &composer),
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
// changes — so typed drafts survive it untouched, and so does the plate
// standing over them: both are the one instance's own state (ducktape-ui#698),
// and the reconnect never reaches inside a composer to blank either.
#[test]
fn same_endpoint_reconnect_preserves_unsent_drafts() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.connected_rpc = "http://node-a".into();
    let composer = composer_scope(&mut app);
    type_into(&mut app, &composer, ComposerKind::Message, "next message");
    // The stash is seeded the only way anything writes it now: the `unsent`
    // message the app's failure slices publish, addressed to this instance.
    let _ = app.__update(Ducktape::__ice_test_message_chat_composer_unsent(
        composer.clone(),
        "unsent message".into(),
        false,
    ));

    let _ = app.__update(__DucktapeMessage::Reconnect);

    assert_eq!(app.connected_rpc, "http://node-a");
    assert_eq!(composer_text(&app, &composer), "next message");
    assert_eq!(composer_stash(&app, &composer), "unsent message");
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
    let ops = composer_scope(&mut app);
    let ops_rail = reply_composer_scope(&mut app);
    type_into(
        &mut app,
        &ops,
        ComposerKind::Message,
        "the incident started at",
    );
    type_into(&mut app, &ops_rail, ComposerKind::Reply, "half a reply");

    let _ = app.__update(__DucktapeMessage::Reconnect);

    let mut landed = workspace("general");
    landed.generation = app.connect_generation;
    landed.channels = vec![room("private-ops", 10), room("general", 20)];
    let _ = app.__update(__DucktapeMessage::WorkspaceConnected(landed));

    assert_eq!(
        app.active_channel, "general",
        "the connect picks the landing"
    );
    let general = composer_scope(&mut app);
    assert_ne!(general, ops, "a different room is a different instance");
    assert!(
        composer_text(&app, &general).is_empty(),
        "#general's composer is #general's — the note she was writing next door \
         is not armed to send here"
    );

    app.mutation_phase = MutationPhase::Idle;
    let _ = app.__update(__DucktapeMessage::ChooseChannel("private-ops".into()));
    assert_eq!(
        composer_text(&app, &ops),
        "the incident started at",
        "it is waiting in the room she was writing it in"
    );

    let _ = app.__update(__DucktapeMessage::OpenThreadFor(3));
    assert_eq!(
        composer_text(&app, &ops_rail),
        "half a reply",
        "and the rail the reconnect closed kept its reply too"
    );
}

/// AND THE ROOM SHE LEFT DOES NOT HAUNT THE NEXT ONE AS AN "UNSENT MESSAGE".
///
/// `reconnect`'s editor harvest predated the park and left `message_draft` —
/// the settled stash — holding the LEFT room's text after the landing. Its one
/// consumer was `live_resynced`'s failed-draft rescue, which fires when a
/// chat-carrying resync lands on a different room, so opening a DM after
/// reconnecting out of a room raised the failed-draft plate offering to
/// restore the old room's words into the DM composer. NEITHER half of that is
/// left: the instance owns the trip, so there is no harvest to leak, and the
/// rescue is a slice keyed to the room being left (ducktape-ui#698), so the
/// room she lands in is not addressable by it at all.
#[test]
fn a_reconnect_does_not_leak_the_left_rooms_draft_into_the_failed_plate() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.connected_rpc = "http://node".into();
    app.active_channel = "private-ops".into();
    let ops = composer_scope(&mut app);
    type_into(
        &mut app,
        &ops,
        ComposerKind::Message,
        "the incident started at",
    );

    let _ = app.__update(__DucktapeMessage::Reconnect);
    let mut landed = workspace("general");
    landed.generation = app.connect_generation;
    landed.channels = vec![room("private-ops", 10), room("general", 20)];
    let _ = app.__update(__DucktapeMessage::WorkspaceConnected(landed));

    // A chat-carrying resync lands on another room — the exact trip that used
    // to stash the harvest onto the app-wide plate. The rescue it still runs
    // is a slice addressed to `#general`, the room it is carrying her OUT of,
    // and it carries the inline edit's text: nothing is being edited here, so
    // there is no body for it to hand anyone.
    let general = composer_scope(&mut app);
    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        app.hydration_generation,
        "dm-with-alice",
        Vec::new(),
        "",
        Vec::new(),
    )));

    assert_eq!(app.active_channel, "dm-with-alice");
    let dm = composer_scope(&mut app);
    assert!(
        composer_text(&app, &dm).is_empty() && composer_stash(&app, &dm).is_empty(),
        "the room she left keeps its own words in its own instance, and is \
         never offered to the room she is in — neither in its box nor on its plate"
    );
    assert!(
        composer_stash(&app, &general).is_empty(),
        "and the room the resync carried her out of was handed nothing either"
    );
    assert_eq!(
        composer_text(&app, &ops),
        "the incident started at",
        "they are still waiting in #private-ops, where she typed them"
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

/// BOTH COMPOSERS ARE RE-ASKED AT DELIVERY, AND BOTH ARE PINNED HERE. A
/// composer's `disabled=` was decided a frame ago, so a channel that went
/// archived — or a members-only roster that dropped her — between the keystroke
/// and the Enter would otherwise let the send through and surface as a server
/// rejection she cannot act on. The optimistic row is the tell: it is written
/// BEFORE the request, so a refused send that still appends one has skipped the
/// gate.
///
/// AND A REFUSAL IS NOT A DISCARD — which is why the arm stashes. The instance
/// clears itself the moment it emits (ducktape-ui#697), so by the time the app
/// re-reads the gate the box is already empty: silence here would lose her
/// words outright. The failed-send plate is where they land, one click from
/// being back in the box — and the arm slices them straight back to the
/// composer that let them go (ducktape-ui#698), so they land on THAT room's
/// plate rather than on whichever room the reader happens to be in.
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

        let stream = composer_scope(&mut app);
        submit_refused(&mut app, &stream, ComposerKind::Message, "into the void");
        assert!(
            app.messages.is_empty(),
            "the main composer must refuse a {reason} channel at apply time"
        );
        // The words are still hers — #general's own plate holds what
        // #general's box let go of.
        assert_eq!(composer_stash(&app, &stream), "into the void");

        app.active_thread_seq = 7;
        let rail = reply_composer_scope(&mut app);
        submit_refused(&mut app, &rail, ComposerKind::Reply, "into the void");
        assert!(
            app.thread_messages.is_empty(),
            "the reply composer must refuse a {reason} channel at apply time"
        );
        assert_eq!(composer_stash(&app, &rail), "into the void");
        // AND ONLY THAT THREAD'S. The rail's refusal is sliced to the root it
        // was written under, so it cannot reach the room's stream composer —
        // a plate that took both would read `"into the void\ninto the void"`,
        // which is what `remember_failed_draft` does with a second stash.
        assert_eq!(
            composer_stash(&app, &stream),
            "into the void",
            "the stream plate holds only what the stream itself refused"
        );
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

    submit(&mut app, ComposerKind::Message, "hello");
    assert_eq!(app.messages.len(), 1, "a seated member still posts");

    app.active_thread_seq = 7;
    submit(&mut app, ComposerKind::Reply, "hello back");
    assert_eq!(
        app.thread_messages.len(),
        1,
        "a seated member still replies"
    );
}

/// A FORMATTING CHORD LANDS IN THE COMPOSER THAT HAS THE CARET, and nothing
/// in the app has to know which one that is.
///
/// This used to be a whole regime: `composer_focus` stood in for widget focus
/// the app cannot read, every handler that moved the caret owed it a retire,
/// three mechanical rules plus a pinned set policed the set of retirees, and
/// the chord's own `active_thread_seq > 0` term covered the one rail close no
/// retire could reach. All of it existed because the chord arrived on the
/// app's ONE keyboard subscription, which sees no focus.
///
/// The chord does not arrive there any more. `RichTextEditor::on_chord`
/// (ducktape-ui#711) is offered exactly the presses the bubble contract
/// releases, so the composer that HAS the caret claims its own Cmd/Ctrl+B and
/// marks its own content (ducktape-ui#697) — a discriminant that could be
/// stale, and a read side that had to be honest about it, both stopped
/// existing. `mark_chords_follow_slacks_table_at_the_widget` in `editor.rs`
/// drives the table itself; `the_composers_are_out_of_reach_of_every_handler`
/// in `rooms.rs` pins that no handler can reach a composer to mark it.
#[test]
fn the_keyboard_subscription_no_longer_marks_a_composer() {
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
             it — add it to HANDLERS, and decide there whether it marks a composer"
        );
    }

    for (name, source) in HANDLERS {
        for line in source.lines().map(str::trim) {
            if line.starts_with("//") {
                continue;
            }
            assert!(
                !line.contains("composer_toggle_mark"),
                "handlers/{name}.ice marks a composer — the mark belongs to the \
                 instance that has the caret, which is the only place that knows"
            );
        }
    }

    // AND THE BEHAVIOUR: a chord on the app's subscription marks nothing. The
    // rail is open and both composers hold words, which is the state the old
    // regime's every failure mode needed.
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.shell_tab = ShellTab::Chat;
    app.active_channel = "general".into();
    app.active_thread_seq = 7;
    let stream = composer_scope(&mut app);
    let rail = reply_composer_scope(&mut app);
    type_into(&mut app, &stream, ComposerKind::Message, "channel draft");
    type_into(&mut app, &rail, ComposerKind::Reply, "reply draft");

    let _ = app.__update(__DucktapeMessage::GlobalKeyPressed(command_chord(
        iced::keyboard::key::Code::KeyB,
    )));

    assert_eq!(
        composer_text(&app, &stream),
        "channel draft",
        "the subscription cannot reach the channel composer"
    );
    assert_eq!(
        composer_text(&app, &rail),
        "reply draft",
        "nor the rail's — the chord is claimed at the widget or not at all"
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
    // The composer marks left this handler with the descent — the widget
    // claims its own chord (ducktape-ui#711) — so the page buffer's undo/redo
    // is the one take the subscription still performs.
    let take = "page_editor = page_history_key(";
    let at = body.find(take).expect(take);
    assert!(
        guard < at,
        "`{take}…` takes the editor, so it must sit BELOW the inert-press guard"
    );

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
    let composer = composer_scope(&mut app);
    type_into(&mut app, &composer, ComposerKind::Message, "draft");

    // Inert: a bare letter opens nothing.
    let _ = app.__update(__DucktapeMessage::GlobalKeyPressed(plain(
        iced::keyboard::key::Code::KeyB,
        iced::keyboard::Key::Character("b".into()),
    )));
    assert!(!app.palette_open);

    // A formatting chord is the widget's now, so the subscription leaves the
    // draft alone — and the classes the guard DOES let through still land.
    let _ = app.__update(__DucktapeMessage::GlobalKeyPressed(command_chord(
        iced::keyboard::key::Code::KeyB,
    )));
    assert_eq!(
        composer_text(&app, &composer),
        "draft",
        "the subscription no longer marks a composer"
    );

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

/// A FAILED SEND HANDS THE WORDS BACK THROUGH THE PLATE, not silently into
/// the box. The composer cleared itself when it emitted (ducktape-ui#697), so
/// the app cannot refill it — and refilling it would be wrong anyway: the
/// failure can arrive while she is in another room, typing something else.
/// The stash is the offer, and Restore is her taking it. The offer is a slice
/// addressed to the room `cause.scope_id` names (ducktape-ui#698), so the
/// handler's task has to be pumped for the composer to hear it.
#[test]
fn failed_optimistic_send_rolls_back_and_stashes_the_draft() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_channel = "general".into();
    // THROUGH THE INSTANCE, which is the real path and the reason the stash
    // can reach it: a composer that has never been typed into holds no state
    // yet, and a slice delivers to instances that do.
    let composer = composer_scope(&mut app);
    type_into(&mut app, &composer, ComposerKind::Message, "retry me");
    submit_composer(&mut app, &composer, ComposerKind::Message, false);
    let operation_id = app.messages[0].id.clone();
    let task = app.__update(__DucktapeMessage::MessageSendFailed(
        backend::OptimisticMutationError {
            message: "rejected".into(),
            committed: false,
            operation_id,
            scope_id: "general".into(),
            thread_seq: 0,
            body: "retry me".into(),
        },
    ));
    pump(&mut app, task);

    assert_eq!(composer_stash(&app, &composer), "retry me");
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
    submit(&mut app, ComposerKind::Message, "first");
    let operation_id = app.messages[0].id.clone();
    let composer = composer_scope(&mut app);
    type_into(&mut app, &composer, ComposerKind::Message, "second");
    let task = app.__update(__DucktapeMessage::MessageSendFailed(
        backend::OptimisticMutationError {
            message: "rejected".into(),
            committed: false,
            operation_id,
            scope_id: "general".into(),
            thread_seq: 0,
            body: "first".into(),
        },
    ));
    pump(&mut app, task);

    assert_eq!(
        composer_text(&app, &composer),
        "second",
        "the words she is writing now are untouched"
    );
    assert_eq!(composer_stash(&app, &composer), "first");

    // AND RESTORE REFUSES OVER A NON-EMPTY BOX — the instance's own guard,
    // which is why the plate's Restore is disabled while she is typing. The
    // stash is not passed in any more: the instance restores from the words it
    // is already holding, so there is no route by which another room's plate
    // could be armed to post here.
    restore_composer(&mut app, &composer, false);
    assert_eq!(
        composer_text(&app, &composer),
        "second",
        "restoring over a draft in progress would overwrite it"
    );
    assert_eq!(
        composer_stash(&app, &composer),
        "first",
        "so the stash still holds"
    );

    seed_composer(&mut app, &composer, ComposerKind::Message, "");
    restore_composer(&mut app, &composer, false);
    assert_eq!(composer_text(&app, &composer), "first");
    assert!(
        composer_stash(&app, &composer).is_empty(),
        "and the instance clears its own plate the moment the words are back"
    );
}

/// A FAILURE THAT ARRIVES AFTER SHE LEFT THE ROOM IS STILL HER TEXT.
///
/// The whole handler used to return on the room check, so a send refused while
/// she was reading another channel left no error, no unsent stash, and no row —
/// and the last thing she saw was the message sitting in the timeline. The room
/// check now scopes the timeline surgery only: the banner is written above it,
/// and the stash rides a slice to the room `cause.scope_id` names.
///
/// WHICH IS THE HALF THIS TEST GREW. One app-wide stash meant the plate went up
/// wherever she was standing, so #general's refused deploy note raised "An
/// earlier message wasn't sent" over #random — and its Restore was armed to
/// drop those words into #random's box, one click from posting them in the
/// wrong room. The words go home to #general's own instance now
/// (ducktape-ui#698), and #random's composer never hears about it.
#[test]
fn a_send_that_fails_after_she_moved_rooms_still_reaches_her() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_channel = "general".into();
    // Through #general's own box, so the room she leaves has an instance to
    // hand the words back to.
    let general = composer_scope(&mut app);
    type_into(
        &mut app,
        &general,
        ComposerKind::Message,
        "the deploy is at 4pm",
    );
    submit_composer(&mut app, &general, ComposerKind::Message, false);
    let operation_id = app.messages[0].id.clone();

    // She switches rooms while the write is in flight, and starts a new message
    // there. `choose_channel` blanks the timeline; the pending row is gone.
    let _ = app.__update(__DucktapeMessage::ChooseChannel("random".into()));
    let random = composer_scope(&mut app);
    type_into(
        &mut app,
        &random,
        ComposerKind::Message,
        "different thought",
    );

    let task = app.__update(__DucktapeMessage::MessageSendFailed(
        backend::OptimisticMutationError {
            message: "rejected".into(),
            committed: false,
            operation_id,
            scope_id: "general".into(),
            thread_seq: 0,
            body: "the deploy is at 4pm".into(),
        },
    ));
    pump(&mut app, task);

    assert_eq!(app.error, "rejected", "the refusal must be said out loud");
    assert_eq!(
        composer_stash(&app, &general),
        "the deploy is at 4pm",
        "and the body she typed must be recoverable in the room it was for"
    );
    assert!(
        composer_stash(&app, &random).is_empty(),
        "#random raises no plate about #general's send — the offer would have \
         armed those words to post in a room she never wrote them for"
    );
    assert_eq!(
        composer_text(&app, &random),
        "different thought",
        "and the box she is typing in is untouched either way"
    );

    // THE SAME HOLE ON THE REPLY PATH, and wider: `close_thread` empties
    // `thread_messages`, so merely closing the rail under an in-flight reply
    // made the pending check fail and dropped the failure whole. `thread_seq`
    // is what carries the reply home once the rail has moved on: the room
    // alone cannot name which of its threads let the words go.
    let (mut rail, _) = Ducktape::__boot();
    rail.connected = true;
    rail.loading = false;
    rail.active_channel = "general".into();
    rail.active_thread_seq = 7;
    // Through the rail's own box, for the reason #general's half is: a sighted
    // instance holds no state until a message reaches it, and a slice delivers
    // only to instances that do.
    let rail_composer = reply_composer_scope(&mut rail);
    type_into(&mut rail, &rail_composer, ComposerKind::Reply, "on it");
    submit_composer(&mut rail, &rail_composer, ComposerKind::Reply, false);
    let reply_id = rail.thread_messages[0].id.clone();
    let _ = rail.__update(__DucktapeMessage::CloseThread);
    let task = rail.__update(__DucktapeMessage::ThreadReplySendFailed(
        backend::OptimisticMutationError {
            message: "reply rejected".into(),
            committed: false,
            operation_id: reply_id,
            scope_id: "general".into(),
            thread_seq: 7,
            body: "on it".into(),
        },
    ));
    pump(&mut rail, task);

    assert_eq!(rail.error, "reply rejected");
    assert_eq!(
        composer_stash(&rail, &rail_composer),
        "on it",
        "a closed rail is not a reason to throw the reply away — it is waiting \
         in thread 7, which is the only place it can be posted from"
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
    submit(&mut app, ComposerKind::Message, "still sending");
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
    submit(&mut app, ComposerKind::Message, "committed once");
    let operation_id = app.messages[0].id.clone();
    let composer = composer_scope(&mut app);
    let _ = app.__update(__DucktapeMessage::MessageSendFailed(
        backend::OptimisticMutationError {
            message: "read failed after commit".into(),
            committed: true,
            operation_id,
            scope_id: "general".into(),
            thread_seq: 0,
            body: "committed once".into(),
        },
    ));

    // The offer the handler slices out, delivered by hand — the committed arm
    // goes on to launch the recovery resync, so pumping the whole task would
    // put a real request on a node this test does not have. A committed body
    // is not unsent, and the INSTANCE is what says so: `unsent` refuses it.
    let _ = app.__update(Ducktape::__ice_test_message_chat_composer_unsent(
        composer.clone(),
        "committed once".into(),
        true,
    ));
    assert!(
        composer_stash(&app, &composer).is_empty(),
        "a COMMITTED body is not unsent — the plate must not offer it back"
    );
    assert_eq!(app.messages.len(), 1);
    assert!(app.messages[0].pending);
    assert_eq!(app.mutation_phase, MutationPhase::Idle);

    submit(&mut app, ComposerKind::Message, "still available");
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

/// AND THE COMPOSER IS PER-ROOM — which is now what it IS, not what a
/// handler remembers to do.
///
/// `choose_channel` used to reset a dozen fields and leave `message_editor`
/// exactly as it found it: half a sentence typed in #private-ops followed the
/// reader into whatever room she clicked next, sat there above a live Send,
/// and was prepended to the next thing she typed and posted THERE. A chain
/// post is permanent in history even after a tombstone delete, and the leaked
/// text is by construction from the room she just left. The park/restore pair
/// that fixed it then had to be repeated by every mover, in the right order,
/// with a lint to police it.
///
/// The composer is a retained instance keyed by its room now
/// (ducktape-ui#697), so this test drives a property of the KEY: a switch
/// cannot carry a draft because a switch does not touch one.
#[test]
fn the_composer_belongs_to_the_room_she_is_in_and_waits_in_the_one_she_left() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.mutation_phase = MutationPhase::Idle;
    app.active_channel = "private-ops".into();
    app.channels = vec![room("private-ops", 10), room("general", 20)];
    let ops = composer_scope(&mut app);
    type_into(
        &mut app,
        &ops,
        ComposerKind::Message,
        "the incident started at",
    );

    let _ = app.__update(__DucktapeMessage::ChooseChannel("general".into()));
    let general = composer_scope(&mut app);
    assert_ne!(general, ops, "a different room is a different instance");
    assert!(
        composer_text(&app, &general).is_empty(),
        "#general's composer is #general's — nothing from next door is armed to \
         send here"
    );

    type_into(&mut app, &general, ComposerKind::Message, "ok");
    let _ = app.__update(__DucktapeMessage::ChooseChannel("private-ops".into()));
    assert_eq!(
        composer_text(&app, &ops),
        "the incident started at",
        "and the sentence she was writing is waiting where she left it"
    );

    let _ = app.__update(__DucktapeMessage::ChooseChannel("general".into()));
    assert_eq!(
        composer_text(&app, &general),
        "ok",
        "both rooms keep their own"
    );

    // A SENT DRAFT DOES NOT COME BACK: the instance clears itself when it
    // emits, and an empty instance is what a return to the room shows.
    // (#general has never been read here, so the switch left `loading` up.)
    app.loading = false;
    submit_composer(&mut app, &general, ComposerKind::Message, false);
    assert!(
        composer_text(&app, &general).is_empty(),
        "the send emptied the box"
    );
    let _ = app.__update(__DucktapeMessage::ChooseChannel("private-ops".into()));
    let _ = app.__update(__DucktapeMessage::ChooseChannel("general".into()));
    assert!(
        composer_text(&app, &general).is_empty(),
        "a message she already sent must not be handed back as a draft"
    );
}

/// AND CREATING A CHANNEL IS A ROOM SWITCH, so it gets the same answer.
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
    let ops = composer_scope(&mut app);
    type_into(
        &mut app,
        &ops,
        ComposerKind::Message,
        "the incident started at",
    );

    let mut created = chat_data("new-channel", Vec::new());
    created.generation = app.chat_generation;
    created.channels = vec![room("private-ops", 10), room("new-channel", 0)];
    let _ = app.__update(__DucktapeMessage::ChannelCreated(created));

    assert_eq!(
        app.active_channel, "new-channel",
        "the create lands her in it"
    );
    let created_room = composer_scope(&mut app);
    assert!(
        composer_text(&app, &created_room).is_empty(),
        "and the new channel's composer is the new channel's — nothing from the \
         room she left is armed to send here"
    );

    let _ = app.__update(__DucktapeMessage::ChooseChannel("private-ops".into()));
    assert_eq!(
        composer_text(&app, &ops),
        "the incident started at",
        "the sentence is waiting in the room she was writing it in"
    );
}
