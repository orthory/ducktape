use super::*;

#[test]
fn a_tombstoned_thread_root_renders_deleted_in_place() {
    let (mut app, _) = Ducktape::__boot();
    app.connected_rpc = "http://node".into();
    app.loading = false;
    app.active_channel = "general".into();
    app.messages = vec![message(9, "thread root", false)];
    app.active_thread_seq = 9;
    app.thread_target_seq = 10;
    app.thread_messages = vec![message(9, "thread root", false)];
    let rail = reply_composer_scope(&mut app);
    type_into(&mut app, &rail, ComposerKind::Reply, "unsent reply");

    // the root's delete arrives as a delta: both lists tombstone the row
    // in place; the open thread stays open showing the tombstone (the
    // module allows replying to a tombstoned root).
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: LiveKind::Chat,
        status: "Live".into(),
        height: 5,
        chat: vec![backend::ChatDelta::Deleted {
            channel_id: "general".into(),
            seq: 9,
        }],
        ..backend::LiveUpdate::default()
    }));

    assert!(app.messages[0].deleted);
    assert!(app.thread_messages[0].deleted);
    assert_eq!(app.thread_messages[0].body, "Message deleted");
    assert_eq!(app.active_thread_seq, 9, "the panel stays open");
    assert_eq!(composer_text(&app, &rail), "unsent reply");
}

#[test]
fn unrelated_resyncs_keep_an_initial_thread_load_alive() {
    let (mut refresh, _) = Ducktape::__boot();
    refresh.connected_rpc = "http://node".into();
    refresh.active_channel = "general".into();
    refresh.loading = false;
    refresh.mutation_phase = MutationPhase::Idle;
    refresh.thread_generation = 6;
    let _ = refresh.__update(__DucktapeMessage::OpenThreadFor(7));
    assert_eq!(refresh.active_thread_seq, 7);
    assert_eq!(refresh.thread_generation, 7);
    assert!(refresh.thread_loading);
    refresh.hydration_generation = 5;

    // an unrelated resync leaves the in-flight thread load untouched
    let _ = refresh.__update(__DucktapeMessage::LiveResynced(live_refresh(
        5,
        "general",
        vec![message(7, "root", false)],
        "",
        Vec::new(),
    )));
    assert_eq!(refresh.active_thread_seq, 7);
    assert_eq!(refresh.thread_generation, 7);
    assert!(refresh.thread_loading);
    let _ = refresh.__update(__DucktapeMessage::ThreadLoaded(backend::ThreadLoadData {
        generation: 7,
        root_seq: 7,
        target_seq: 7,
        messages: vec![message(7, "root", false)],
        next_reply_seq: 0,
        has_more: false,
    }));
    assert_eq!(refresh.active_thread_seq, 7);
    assert_eq!(refresh.thread_messages.len(), 1);
    assert!(!refresh.thread_loading);
}

/// THE RAIL IS THE SAME LIST WITH THE SAME BILL. A thread pages in at the same
/// 256 replies a channel does, and a plain column culls only `draw` — `update`,
/// `mouse_interaction`, `overlay` and `layout` walk every reply ever loaded, on
/// every event and every frame. Virtualization culls those four; `lazy` stops
/// the rows that ARE visible from rebuilding ~60 nodes of scope strings and
/// a11y keys apiece. The two are not alternatives, and the stream carries both.
#[test]
fn the_thread_rail_virtualizes_and_caches_its_quiet_replies() {
    let chat = inlined(include_str!("../ui/screens/chat.ice"));
    assert!(chat.contains("scroll dir=vertical w=fill h=fill anchor-y=end auto=true"));
    assert!(chat.contains(
        "keyed thread_message in messages by=thread_message.view_key w=fill gap=3.0 virtual-row=44.0"
    ));
    assert!(chat.contains(
        "lazy thread_messages by thread_messages_revision, active_channel, active_thread_seq, thread_target_seq, thread_selected_seq, loading as cached_thread_messages"
    ));
    // A `lazy` subtree reads nothing but its dependency, so the quiet arm can
    // only exist because the rows that read SCREEN state — the search target
    // and the open action menu — were split off into live arms. Confirmation
    // is row state and moves `render_rev`, so it belongs inside the lazy arm.
    // The KEYED form is pinned too:
    // dropping `by (seq, render_rev)` silently reverts every visible reply to
    // a full row clone + hash per frame — the #1058 residue this collects.
    assert!(chat.contains("lazy thread_message as cached_reply"));
    for live in [
        "thread_message.seq == thread_target_seq",
        "thread_message.seq == thread_selected_seq",
    ] {
        assert!(chat.contains(live), "the live arm on {live} is gone");
    }
}

#[test]
fn thread_messages_mirror_the_main_action_system() {
    let components = inlined(include_str!("../ui/components/chat.ice"));
    let card = components
        .split_once("component ThreadMessageCard")
        .unwrap()
        .1;
    // `open=menu_open` is the toolbar's anchor contract: the reveal outlives
    // the pointer for exactly as long as the card it opened is up.
    assert!(card.contains("hover tint=row_hover r=9.0 open=menu_open"));
    assert!(
        card.contains(
            "-> emit(open_thread_message_actions, message.seq, message.body, message.rev)"
        )
    );
    assert!(
        card.contains(
            "-> emit(open_thread_message_actions, message.seq, message.body, message.rev)"
        )
    );
    assert!(card.contains(
        "-> emit(open_thread_message_reactions, message.seq, message.body, message.rev)"
    ));
    // A reply is the SAME message block as a timeline row — the rail mounts
    // the shared contents rather than a second spelling of them, so the
    // message redesign lands in both lanes at once.
    assert!(card.contains("MessageContents message=message"));
    // Confirmation is the pending dot disappearing, so the card needs no
    // timer or animation prop. (`card` starts right after the component name,
    // so the signature is its head.)
    assert!(
        card.starts_with("(message:ChatMessage, selected:bool, menu_open:bool, disabled:bool)")
    );
    // `menu_open` cannot be `selected` here: in the rail `selected` marks the
    // deep-link TARGET reply, not the row whose action card is open.
    let chat_screen_rail = inlined(include_str!("../ui/screens/chat.ice"));
    assert!(chat_screen_rail.contains("menu_open=(thread_message.seq == thread_selected_seq)"));
    // No open-thread action from inside a thread you are already reading. The
    // shared contents still declare the event (their reply pill emits it) so
    // the card forwards it, but the rail's toolbar has no seat for it — and a
    // reply carries no replies, so the pill never renders here.
    assert!(!card.contains("label=\"Open thread\""));

    let chat_screen = inlined(include_str!("../ui/screens/chat.ice"));
    let thread = chat_screen
        .split_once("if active_thread_seq > 0 && !channel_settings_open")
        .unwrap()
        .1;
    // A SECOND overlay, keyed on thread-scoped state, independent of the main one.
    assert!(thread.contains(
        "overlay when=(thread_selected_seq > 0 && thread_message_action != MessageAction.toolbar)"
    ));
    assert!(thread.contains("dismiss=emit(clear_thread_message_selection) backdrop=transparent"));
    assert!(thread.contains(
        "box w=fill h=fill pt=block_action_menu_y(thread_pointer_y, thread_height) align-x=end align-y=start"
    ));
    assert!(thread.contains("mouse press-at=thread_pointer_pressed"));
    // same seat as the message list — the rail measures itself
    assert!(thread.contains("sensor show=thread_resized resize=thread_resized"));
    // The picker is the shared ADD grid targeting the thread selection;
    // removal rides the reply's own reaction chips.
    assert!(thread.contains("for emoji in reaction_palette()"));
    assert!(thread.contains("-> emit(add_reaction_at, thread_selected_seq, emoji)"));
    // Same pressable-while-in-flight contract as the stream picker.
    let thread_picker = thread
        .split_once("thread_message_action == MessageAction.reactions")
        .unwrap()
        .1
        .split_once("thread_message_action == MessageAction.editing")
        .unwrap()
        .0;
    assert!(!thread_picker.contains("mutation_phase"));
    // More-menu omits Reply in thread (already inside the thread) and Close.
    let more = thread
        .split_once("thread_message_action == MessageAction.more")
        .unwrap()
        .1
        .split_once("thread_message_action == MessageAction.reactions")
        .unwrap()
        .0;
    for label in [
        "label=\"Manage reactions\"",
        "label=\"Edit message\"",
        "label=\"Delete message\"",
    ] {
        assert!(more.contains(label), "{label}");
    }
    assert!(!more.contains("Reply in thread"));
    assert!(!more.contains("button \"Close\""));

    let handlers = inlined(include_str!("../ui/handlers/chat.ice"));
    for name in [
        "on open_thread_message_actions(seq, body, rev)",
        "on open_thread_message_reactions(seq, body, rev)",
        "on begin_thread_message_edit(seq, body, rev)",
        "on arm_thread_message_delete(seq, body, rev)",
        "on clear_thread_message_selection",
        "on edit_thread_message_submit",
        "on delete_thread_message_submit",
    ] {
        assert!(handlers.contains(name), "{name}");
    }
    // Thread edit/delete target the thread selection, never the main one.
    let edit = handlers
        .split_once("on edit_thread_message_submit\n")
        .unwrap()
        .1
        .split_once("\non ")
        .unwrap()
        .0;
    assert!(edit.contains(
        "edit_message(connected_rpc, password, active_channel, thread_selected_seq, thread_selected_rev, trim(thread_edit_draft), channel_members)"
    ));
    let delete = handlers
        .split_once("on delete_thread_message_submit\n")
        .unwrap()
        .1
        .split_once("\non ")
        .unwrap()
        .0;
    assert!(
        delete.contains(
            "delete_message(connected_rpc, password, active_channel, thread_selected_seq)"
        )
    );
}

#[test]
fn thread_action_state_is_independent_of_the_main_message_menu() {
    let (mut app, _) = Ducktape::__boot();
    app.mutation_phase = MutationPhase::Idle;
    app.active_channel = "general".into();
    app.active_thread_seq = 1;

    // Opening a thread action must not touch the main message menu.
    let _ = app.__update(__DucktapeMessage::OpenThreadMessageActions(
        2,
        "reply".into(),
        3,
    ));
    assert_eq!(app.thread_selected_seq, 2);
    assert_eq!(app.thread_message_action, MessageAction::More);
    assert_eq!(app.selected_message_seq, 0);
    assert_eq!(app.message_action, MessageAction::Toolbar);

    // And a main message action must not touch the thread menu.
    let _ = app.__update(__DucktapeMessage::OpenMessageActions(5, "root".into(), 1));
    assert_eq!(app.selected_message_seq, 5);
    assert_eq!(app.message_action, MessageAction::More);
    assert_eq!(app.thread_selected_seq, 2);
    assert_eq!(app.thread_message_action, MessageAction::More);

    let _ = app.__update(__DucktapeMessage::ClearThreadMessageSelection);
    assert_eq!(app.thread_selected_seq, 0);
    assert_eq!(app.thread_message_action, MessageAction::Toolbar);
    assert_eq!(app.selected_message_seq, 5);
    assert_eq!(app.message_action, MessageAction::More);
}

#[test]
fn opening_another_thread_invalidates_the_pending_thread() {
    let (mut app, _) = Ducktape::__boot();
    app.mutation_phase = MutationPhase::Idle;
    app.active_channel = "general".into();
    app.selected_message_seq = 1;
    app.thread_generation = 4;
    app.thread_loading = true;
    app.active_thread_seq = 1;
    app.thread_messages =
        backend::optimistic_message(Vec::new(), "old thread".into(), "pending-old".into());
    let thread_one = reply_composer_scope(&mut app);
    type_into(&mut app, &thread_one, ComposerKind::Reply, "old reply");

    let _ = app.__update(__DucktapeMessage::OpenThreadFor(2));
    assert_eq!(app.thread_generation, 5);
    assert!(app.thread_loading);
    assert_eq!(app.active_thread_seq, 2);
    assert!(app.thread_messages.is_empty());
    let thread_two = reply_composer_scope(&mut app);
    assert!(composer_text(&app, &thread_two).is_empty());
    assert_eq!(
        composer_text(&app, &thread_one),
        "old reply",
        "the rail that opened is thread 2's own composer instance; thread 1's \
         words are not thrown away, they wait under thread 1's key"
    );

    let _ = app.__update(__DucktapeMessage::ThreadLoaded(backend::ThreadLoadData {
        generation: 4,
        root_seq: 1,
        target_seq: 0,
        messages: Vec::new(),
        next_reply_seq: 0,
        has_more: false,
    }));
    assert_eq!(app.active_thread_seq, 2);
}

/// CLICKING ANOTHER THREAD IS NOT A REQUEST TO THROW THE REPLY AWAY.
///
/// The rail sits beside a timeline that stays mounted, and every "N replies"
/// row in it emits `open_thread_for`. While both rails shared ONE `reply_editor`
/// on the app, that handler had to blank the LIVE buffer every keystroke lands
/// in — so three sentences into a reply, a click meant to check something next
/// door destroyed them: no banner, no Restore, nothing.
///
/// Each rail owns a retained `ChatComposer` instance now (ducktape-ui#697),
/// keyed by room AND root, so there is no shared buffer left for a rail open to
/// blank: the words stay in the one composer they can be posted from. The key is
/// NOT `failed_reply_draft`, which is only channel-scoped — that plate would
/// have offered thread A's words over every later thread of the room, and its
/// Restore would have armed them to post in B, the same cross-context
/// re-targeting the stream composer's own key exists to end.
///
/// `close_thread` USED to be the one route that discarded; it no longer is, and
/// the last arm pins that. Closing hides the rail, and an accidental Escape
/// stopped being able to eat text — Dismiss on the banner (or sending) is how a
/// reply is actually let go. The drawer never ate one either:
/// `the_channel_drawer_does_not_eat_a_reply_you_are_typing`.
#[test]
fn opening_another_thread_leaves_the_reply_in_the_thread_it_belongs_to() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_channel = "general".into();
    app.active_thread_seq = 1;
    let thread_one = reply_composer_scope(&mut app);
    type_into(
        &mut app,
        &thread_one,
        ComposerKind::Reply,
        "three sentences in and",
    );

    let _ = app.__update(__DucktapeMessage::OpenThreadFor(2));
    assert_eq!(
        app.active_thread_seq, 2,
        "the rail she clicked is the one open"
    );
    let thread_two = reply_composer_scope(&mut app);
    assert!(
        composer_text(&app, &thread_two).is_empty(),
        "a rail that just opened has an untouched composer"
    );
    assert!(
        app.failed_reply_draft.is_empty(),
        "and NOT through the channel-scoped plate, which would offer thread 1's \
         words to every other thread in #general"
    );

    // Back to the thread they belong to, and they are waiting there — the same
    // instance under the same key, never emptied by anything in between.
    let _ = app.__update(__DucktapeMessage::OpenThreadFor(1));
    assert_eq!(reply_composer_scope(&mut app), thread_one);
    assert_eq!(composer_text(&app, &thread_one), "three sentences in and");

    // A ROOM SWITCH CARRIES THE RAIL AWAY AND HANDS IT BACK on the way in. The
    // text is CHANGED first, so this arm reads the instance as the picker left
    // it rather than what the entry `open_thread_for` above found.
    seed_composer(
        &mut app,
        &thread_one,
        ComposerKind::Reply,
        "and then the pager went off",
    );
    app.channels = vec![room("general", 10), room("random", 20)];
    app.mutation_phase = MutationPhase::Idle;
    let _ = app.__update(__DucktapeMessage::ChooseChannel("random".into()));
    assert_eq!(app.active_thread_seq, 0, "the rail closes with the room");
    let _ = app.__update(__DucktapeMessage::ChooseChannel("general".into()));
    let _ = app.__update(__DucktapeMessage::OpenThreadFor(1));
    assert_eq!(
        composer_text(&app, &thread_one),
        "and then the pager went off",
        "the reply belongs to #general's thread 1 and is still there"
    );

    // AND CLOSE IS NOT A DISCARD ANY MORE. The click hides the rail; the
    // instance behind it keeps the words, so the next open on that thread finds
    // them where she left them.
    let _ = app.__update(__DucktapeMessage::CloseThread);
    let _ = app.__update(__DucktapeMessage::OpenThreadFor(1));
    assert_eq!(
        composer_text(&app, &thread_one),
        "and then the pager went off",
        "Close hides the rail, it does not empty the composer behind it"
    );
}

/// A PEER DELETING THE ROOT IS NOT A REQUEST TO THROW THE REPLY AWAY EITHER.
///
/// `live_resynced` closes the rail on its own whenever `refreshed_known_message_seq`
/// finds the root deleted or the room moved. That used to be a data-loss route
/// with an ORDERING trap under it: the app held one `reply_editor`, the close
/// blanked it under the caret, and the park that was supposed to catch the words
/// had to run ABOVE the close — `park_reply_draft` refused `thread_seq <= 0`, so
/// a read taken below it was a guaranteed no-op.
///
/// There is no ordering left to get wrong. The rail's composer is the thread's
/// own retained instance (ducktape-ui#697) and no handler can name it, so a
/// close of any kind — this one included — cannot reach the words. They wait
/// under `general#7`, which is the only place they can be posted anyway.
#[test]
fn a_resync_that_closes_the_rail_leaves_the_reply_it_closes_over() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_channel = "general".into();
    app.hydration_generation = 4;
    app.active_thread_seq = 7;
    let rail = reply_composer_scope(&mut app);
    type_into(
        &mut app,
        &rail,
        ComposerKind::Reply,
        "three sentences in and",
    );

    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        4,
        "general",
        vec![message(7, "the root", true)],
        "",
        Vec::new(),
    )));
    assert_eq!(
        app.active_thread_seq, 0,
        "a deleted root closes the rail under the caret"
    );
    assert_eq!(
        composer_text(&app, &rail),
        "three sentences in and",
        "and the words are still in the composer of the thread they were written in"
    );

    // Which is the only place they can be posted, so that is where they come
    // back — through the ordinary rail open, no banner and no Restore needed.
    // The reopened rail IS the instance she left, not a refilled copy of it.
    let _ = app.__update(__DucktapeMessage::OpenThreadFor(7));
    assert_eq!(reply_composer_scope(&mut app), rail);
    assert_eq!(composer_text(&app, &rail), "three sentences in and");
}

/// AND ARRIVING IN A THREAD BY THE SEARCH ROUTE OPENS THE SAME COMPOSER.
///
/// `load_chat_hit` answers with `root.seq` when the hit is a reply, so a
/// chat-search jump SEATS a thread — and `chat_hit_loaded` wrote
/// `active_thread_seq` with no restore beside it. Against the park store that
/// was a silent overwrite, not just the loss of a live buffer: the rail opened
/// on an empty box over her parked reply, and the first character typed into it
/// parked OVER those words under the same `general#7` key.
///
/// The seat NAMES an instance now (ducktape-ui#697) instead of filling a
/// buffer, so every route into a thread — the rail click, the resync, this one —
/// arrives at the composer that thread already had. There is no refill step
/// left for a route to forget.
#[test]
fn a_search_hit_that_seats_a_thread_opens_that_threads_own_composer() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_channel = "general".into();
    app.active_thread_seq = 7;
    let seven = reply_composer_scope(&mut app);
    type_into(&mut app, &seven, ComposerKind::Reply, "half an answer");

    // Clicking another thread swaps the instance under the rail — the route
    // this one has to arrive back from.
    let _ = app.__update(__DucktapeMessage::OpenThreadFor(9));
    let nine = reply_composer_scope(&mut app);
    assert!(composer_text(&app, &nine).is_empty());

    let mut hit = chat_data("general", vec![message(7, "the root", false)]);
    hit.generation = app.chat_generation;
    hit.active_thread_seq = 7;
    let _ = app.__update(__DucktapeMessage::ChatHitLoaded(hit));

    assert_eq!(app.active_thread_seq, 7, "the hit seated its thread");
    assert_eq!(reply_composer_scope(&mut app), seven);
    assert_eq!(
        composer_text(&app, &seven),
        "half an answer",
        "and the rail it opened is the rail she left words in"
    );
}

#[test]
fn thread_pagination_preserves_multiple_pending_replies() {
    let message = |seq: i64, thread_seq: i64, body: &str| backend::ChatMessage {
        id: format!("message-{seq}"),
        view_key: seq,
        seq,
        author: "user".into(),
        meta: format!("#{seq}"),
        body: body.into(),
        blocks: backend::paragraph_blocks(body),
        pending: false,
        rev: 0,
        edited: false,
        deleted: false,
        reply_count: 0,
        thread_seq,
        show_author: true,
        initial: "U".into(),
        avatar_kind: "human".into(),
        height: 0,
        time: 0,
        reactions: Vec::new(),
        render_rev: 0,
    };
    let (mut app, _) = Ducktape::__boot();
    app.active_thread_seq = 1;
    app.thread_generation = 7;
    app.thread_loading = true;
    app.thread_messages = backend::optimistic_message(
        backend::optimistic_message(
            vec![message(1, 0, "root"), message(2, 1, "first")],
            "pending first".into(),
            "pending-first".into(),
        ),
        "pending second".into(),
        "pending-second".into(),
    );

    let _ = app.__update(__DucktapeMessage::ThreadPageLoaded(
        backend::ThreadPageData {
            generation: 7,
            messages: vec![message(3, 1, "second")],
            next_reply_seq: 0,
            has_more: false,
        },
    ));
    assert_eq!(app.thread_messages.len(), 5);
    assert_eq!(app.thread_messages[1].body, "first");
    assert_eq!(app.thread_next_reply_seq, 0);
    assert!(
        app.thread_messages
            .iter()
            .any(|message| { message.pending && message.id == "pending-first" })
    );
    assert!(
        app.thread_messages
            .iter()
            .any(|message| { message.pending && message.id == "pending-second" })
    );
}

#[test]
fn deltas_fold_during_thread_pagination_without_disturbing_it() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.active_channel = "general".into();
    app.messages = vec![message(7, "root", false)];
    app.active_thread_seq = 7;
    app.thread_generation = 4;
    app.thread_loading = true;
    app.hydration_generation = 9;

    // a delta folds immediately — pagination in flight is not a gate
    let _ = app.__update(__DucktapeMessage::LiveUpdated(posted_delta(
        "general",
        message(8, "landed mid-pagination", false),
    )));
    assert_eq!(app.messages.len(), 2);
    assert_eq!(
        app.hydration_generation, 9,
        "a folded delta starts no reload"
    );
    assert!(app.thread_loading);

    // the pending thread page still lands on its own generation
    let _ = app.__update(__DucktapeMessage::ThreadPageLoaded(
        backend::ThreadPageData {
            generation: 4,
            messages: Vec::new(),
            next_reply_seq: 0,
            has_more: false,
        },
    ));
    assert!(!app.thread_loading);
    assert_eq!(app.hydration_generation, 9);
}

#[test]
fn live_thread_refresh_preserves_the_reply_draft_and_rejects_other_scopes() {
    let (mut app, _) = Ducktape::__boot();
    app.active_channel = "general".into();
    app.active_thread_seq = 7;
    app.thread_target_seq = 9;
    app.thread_next_reply_seq = 5;
    app.thread_has_more = true;
    let rail = reply_composer_scope(&mut app);
    type_into(&mut app, &rail, ComposerKind::Reply, "typing");
    app.thread_messages = backend::optimistic_message(
        backend::optimistic_message(Vec::new(), "pending first".into(), "pending-first".into()),
        "pending second".into(),
        "pending-second".into(),
    );

    let _ = app.__update(__DucktapeMessage::LiveThreadRefreshed(
        backend::LiveThreadData {
            channel_id: "other".into(),
            root_seq: 7,
            messages: Vec::new(),
        },
    ));
    assert_eq!(app.thread_next_reply_seq, 5);

    let _ = app.__update(__DucktapeMessage::LiveThreadRefreshed(
        backend::LiveThreadData {
            channel_id: "general".into(),
            root_seq: 7,
            messages: Vec::new(),
        },
    ));
    assert_eq!(composer_text(&app, &rail), "typing");
    assert_eq!(app.thread_target_seq, 9);
    assert_eq!(app.thread_next_reply_seq, 5);
    assert!(app.thread_has_more);
    assert!(
        app.thread_messages
            .iter()
            .any(|message| { message.pending && message.id == "pending-first" })
    );
    assert!(
        app.thread_messages
            .iter()
            .any(|message| { message.pending && message.id == "pending-second" })
    );

    let _ = app.__update(__DucktapeMessage::CloseThread);
    let _ = app.__update(__DucktapeMessage::LiveThreadRefreshed(
        backend::LiveThreadData {
            channel_id: "general".into(),
            root_seq: 7,
            messages: Vec::new(),
        },
    ));
    assert_eq!(app.thread_next_reply_seq, 0);
    assert!(!app.thread_has_more);
}

/// A REPLY IS FORMATTABLE, through both of the doors the stream's composer has.
///
/// The rail's composer had NEITHER: no toolbar (the seat row was hint + Send),
/// and the Cmd/Ctrl chord — supposedly the keyboard half of the same table —
/// was hard-wired to `message_editor` in `handlers/overlays.ice`. The chord rode
/// the app's ONE global key subscription, which sees no widget focus, so Cmd+B
/// pressed with the caret in a thread reply wrapped the CHANNEL draft instead: a
/// silent write into a composer the user was not looking at.
///
/// BOTH DOORS ARE INSIDE THE COMPOSER NOW (ducktape-ui#697/#711), and that —
/// not a wider app-side discriminant — is what ended the bug class. The marks
/// row is mounted ONCE, in `ChatComposer`, and its `mark` handler can only
/// reach that instance's own `body`; the chord is claimed at the widget that
/// HAS the caret and arrives as a `ComposerEvent::Mark` on the same instance.
/// Two seats can no longer collapse onto one editor because neither route names
/// an editor at all — the retired app state that could is swept by
/// `the_composers_are_out_of_reach_of_every_handler`.
///
/// Both doors stay pinned because they are still separate mechanisms: the
/// toolbar goes through the component's own handler, the chord through
/// `apply_composer_event`.
#[test]
fn a_thread_reply_takes_marks_from_its_own_toolbar_and_the_chord() {
    // THE MOUNT. One marks row, inside the composer, routed to the local
    // handler — no app event in the path to aim at the wrong editor.
    let components = inlined(include_str!("../ui/components/chat.ice"));
    let chat_composer = components.split_once("component ChatComposer").unwrap().1;
    assert!(chat_composer.contains("mark -> mark(_, blocked)"));

    // A caret selection, through the same seam a real drag arrives on.
    let select_all = |app: &mut Ducktape, scope: &str, kind: ComposerKind| {
        let message = Ducktape::__ice_test_message_chat_composer_composer_event(
            scope.to_owned(),
            editor::ComposerEvent::Apply(editor::RichAction::Edit(
                iced::widget::text_editor::Action::SelectAll,
            )),
            false,
            kind,
        );
        let _ = app.__update(message);
    };

    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_channel = "general".into();
    app.active_thread_seq = 7;
    let stream = composer_scope(&mut app);
    let rail = reply_composer_scope(&mut app);
    type_into(&mut app, &stream, ComposerKind::Message, "channel draft");
    type_into(&mut app, &rail, ComposerKind::Reply, "reply draft");

    // THE TOOLBAR half: the rail's Bold wraps the REPLY and nothing else.
    select_all(&mut app, &rail, ComposerKind::Reply);
    let _ = app.__update(Ducktape::__ice_test_message_chat_composer_mark(
        rail.clone(),
        "bold".into(),
        false,
    ));
    assert_eq!(composer_text(&app, &rail), "**reply draft**");
    assert_eq!(
        composer_text(&app, &stream),
        "channel draft",
        "the stream draft is not the reply's"
    );

    // THE CHORD half, caret in the reply: the widget claimed Cmd+B and handed
    // it to its OWN instance as a mark. Nothing consults a focus discriminant,
    // because the press never leaves the composer it was pressed in.
    seed_composer(&mut app, &rail, ComposerKind::Reply, "reply draft");
    select_all(&mut app, &rail, ComposerKind::Reply);
    let _ = app.__update(Ducktape::__ice_test_message_chat_composer_composer_event(
        rail.clone(),
        editor::ComposerEvent::Mark("bold".into()),
        false,
        ComposerKind::Reply,
    ));
    assert_eq!(composer_text(&app, &rail), "**reply draft**");
    assert_eq!(
        composer_text(&app, &stream),
        "channel draft",
        "Cmd+B in a reply is not a channel edit"
    );

    // AND IT IS NOT A BLANKET REDIRECT: the same chord pressed in the stream's
    // composer marks the stream's draft, rail open or not. Without this arm the
    // asserts above would pass against a route hard-wired to the reply.
    let reply_before = composer_text(&app, &rail);
    select_all(&mut app, &stream, ComposerKind::Message);
    let _ = app.__update(Ducktape::__ice_test_message_chat_composer_composer_event(
        stream.clone(),
        editor::ComposerEvent::Mark("bold".into()),
        false,
        ComposerKind::Message,
    ));
    assert_eq!(composer_text(&app, &stream), "**channel draft**");
    assert_eq!(composer_text(&app, &rail), reply_before);
}

#[test]
fn optimistic_thread_replies_settle_independently_out_of_order() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_channel = "general".into();
    app.active_thread_seq = 1;
    submit(&mut app, ComposerKind::Reply, "first");
    let first_id = app.thread_messages[0].id.clone();
    assert_eq!(app.mutation_phase, MutationPhase::Idle);
    assert!(app.thread_messages[0].pending);

    submit(&mut app, ComposerKind::Reply, "second");
    let second_id = app.thread_messages[1].id.clone();
    assert_ne!(first_id, second_id);
    assert_eq!(app.thread_messages.len(), 2);
    assert!(app.thread_messages.iter().all(|message| message.pending));

    let mut second = message(3, "second", false);
    second.id = second_id.clone();
    second.thread_seq = 1;
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: LiveKind::Chat,
        status: "Live".into(),
        height: 3,
        chat: vec![backend::ChatDelta::Reply {
            channel_id: "general".into(),
            seq: 3,
            root_seq: 1,
            message: second,
        }],
        ..backend::LiveUpdate::default()
    }));
    assert_eq!(app.thread_messages.len(), 2);
    assert!(
        app.thread_messages
            .iter()
            .any(|message| message.id == first_id && message.pending)
    );
    assert!(
        app.thread_messages
            .iter()
            .any(|message| message.body == "second" && !message.pending)
    );

    let mut first = message(2, "first", false);
    first.id = first_id.clone();
    first.thread_seq = 1;
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: LiveKind::Chat,
        status: "Live".into(),
        height: 4,
        chat: vec![backend::ChatDelta::Reply {
            channel_id: "general".into(),
            seq: 2,
            root_seq: 1,
            message: first,
        }],
        ..backend::LiveUpdate::default()
    }));
    assert_eq!(app.thread_messages.len(), 2);
    assert!(app.thread_messages.iter().all(|message| !message.pending));
    assert_eq!(app.thread_messages[0].body, "first");
    assert_eq!(app.thread_messages[1].body, "second");
}

#[test]
fn failed_thread_reply_rolls_back_only_itself_and_preserves_the_newer_draft() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_channel = "general".into();
    app.active_thread_seq = 1;
    let rail = reply_composer_scope(&mut app);
    submit(&mut app, ComposerKind::Reply, "first");
    let first_id = app.thread_messages[0].id.clone();
    submit(&mut app, ComposerKind::Reply, "second");
    let second_id = app.thread_messages[1].id.clone();
    type_into(&mut app, &rail, ComposerKind::Reply, "newer draft");

    let _ = app.__update(__DucktapeMessage::ThreadReplySendFailed(
        backend::OptimisticMutationError {
            message: "rejected".into(),
            committed: false,
            operation_id: first_id,
            scope_id: "general".into(),
            body: "first".into(),
        },
    ));
    assert_eq!(composer_text(&app, &rail), "newer draft");
    assert_eq!(app.failed_reply_draft, "first");
    assert_eq!(app.thread_messages.len(), 1);
    assert_eq!(app.thread_messages[0].id, second_id);
    assert!(app.thread_messages[0].pending);
    assert_eq!(app.mutation_phase, MutationPhase::Idle);
    assert!(!app.thread_loading);

    // RESTORE REFUSES OVER A LIVE DRAFT, and the guard is the instance's own
    // (`restore` returns on a non-empty body) — the words she is typing now are
    // never overwritten by a banner about an older send.
    let stashed = app.failed_reply_draft.clone();
    let __restore = app.__update(Ducktape::__ice_test_message_chat_composer_restore(
        rail.clone(),
        stashed.clone(),
        false,
        ComposerKind::Reply,
    ));
    pump(&mut app, __restore);
    assert_eq!(composer_text(&app, &rail), "newer draft");
    assert_eq!(
        app.failed_reply_draft, "first",
        "a refused restore leaves the plate armed"
    );

    // Empty the box and it hands them back. The instance writes the words; the
    // `composer_restored` message it emits only clears the plate.
    seed_composer(&mut app, &rail, ComposerKind::Reply, "");
    let __restore = app.__update(Ducktape::__ice_test_message_chat_composer_restore(
        rail.clone(),
        stashed,
        false,
        ComposerKind::Reply,
    ));
    pump(&mut app, __restore);
    assert_eq!(composer_text(&app, &rail), "first");
    assert!(app.failed_reply_draft.is_empty());
}

#[test]
fn committed_thread_reply_refreshes_without_blocking_the_composer() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.connected_rpc = "http://node".into();
    app.active_channel = "general".into();
    app.active_thread_seq = 1;
    submit(&mut app, ComposerKind::Reply, "committed");
    let operation_id = app.thread_messages[0].id.clone();
    let _ = app.__update(__DucktapeMessage::ThreadReplySendFailed(
        backend::OptimisticMutationError {
            message: "read failed after commit".into(),
            committed: true,
            operation_id,
            scope_id: "general".into(),
            body: "committed".into(),
        },
    ));
    assert_eq!(app.thread_messages.len(), 1);
    assert!(app.thread_messages[0].pending);
    assert!(
        app.failed_reply_draft.is_empty(),
        "a committed body is on the node, so no banner offers it back"
    );
    assert_eq!(app.mutation_phase, MutationPhase::Idle);
    // `thread_loading` IS the rail composer's `blocked` term, so a failure that
    // leaves it clear leaves a composer the reader can still send from — which
    // the second reply below is the proof of.
    assert!(!app.thread_loading);

    submit(&mut app, ComposerKind::Reply, "still available");
    assert_eq!(app.thread_messages.len(), 2);
    assert!(app.thread_messages.iter().all(|message| message.pending));
}

/// THE THREAD RAIL OPENS ON THE MESSAGE IT IS ABOUT.
///
/// `open_thread_for` emptied `thread_messages` and the rail's only body is a
/// loop over it, with both loading arms gated on `thread_has_more` — which the
/// same handler clears. So the click produced a 330px pane of bare background
/// with no root row, no skeleton and a disabled composer for the whole round
/// trip, and a load that FAILED left it that way until Close. The clicked
/// message is already in hand; seeding it costs one filter.
#[test]
fn opening_a_thread_seeds_its_root_row_instead_of_a_blank_rail() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.active_channel = "a".into();
    app.messages = vec![message(7, "the message it is about", false)];

    let _ = app.__update(__DucktapeMessage::OpenThreadFor(7));
    assert_eq!(app.active_thread_seq, 7);
    assert!(app.thread_loading, "the replies are still out");
    assert_eq!(
        app.thread_messages
            .iter()
            .map(|row| row.seq)
            .collect::<Vec<_>>(),
        vec![7],
        "the root draws on the click, before the node answers"
    );

    // A FAILURE LEAVES THE ROOT STANDING. `thread_failed` clears the busy term
    // and routes the text to the app banner; it never touches the rail, so an
    // unseeded rail stayed blank for as long as it was open.
    let _ = app.__update(__DucktapeMessage::ThreadFailed(backend::HydrationError {
        generation: app.thread_generation,
        message: "the node did not answer".into(),
    }));
    assert!(!app.thread_loading);
    assert_eq!(
        app.thread_messages.len(),
        1,
        "the rail still says which thread"
    );

    // AND A RE-ROOT ONTO A REPLY WORKS TOO: that seq lives in the rail's own
    // vec, never in the timeline, because the button is on a reply card.
    let mut loaded = backend::ThreadLoadData {
        generation: 0,
        root_seq: 7,
        target_seq: 0,
        messages: vec![message(7, "root", false), message(9, "a reply", false)],
        next_reply_seq: 0,
        has_more: false,
    };
    let _ = app.__update(__DucktapeMessage::OpenThreadFor(7));
    loaded.generation = app.thread_generation;
    let _ = app.__update(__DucktapeMessage::ThreadLoaded(loaded));
    let _ = app.__update(__DucktapeMessage::OpenThreadFor(9));
    assert_eq!(
        app.thread_messages
            .iter()
            .map(|row| row.seq)
            .collect::<Vec<_>>(),
        vec![9],
        "the reply the reader re-rooted on is the new rail's root"
    );
}
