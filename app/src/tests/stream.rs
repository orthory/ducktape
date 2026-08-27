use super::*;

#[test]
fn stale_resyncs_are_ignored_and_deltas_fold_without_reloads() {
    let (mut app, _) = Ducktape::__boot();
    app.status = "current".into();
    app.hydration_generation = 3;
    app.loading = false;

    // a channel switch invalidates any in-flight resync
    let _ = app.__update(__DucktapeMessage::ChooseChannel("next".into()));
    assert_eq!(app.hydration_generation, 4);

    // a chat delta folds straight into state — no reload cycle
    app.loading = false;
    app.active_channel = "next".into();
    let _ = app.__update(__DucktapeMessage::LiveUpdated(posted_delta(
        "next",
        message(1, "hello from the feed", false),
    )));
    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].body, "hello from the feed");

    // a resync from a superseded generation is dropped whole
    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        3,
        "stale",
        vec![message(9, "stale", false)],
        "stale-page",
        Vec::new(),
    )));
    assert_eq!(app.active_channel, "next");
    assert_eq!(app.messages[0].body, "hello from the feed");

    // the current generation applies
    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        4,
        "next",
        vec![message(1, "hello from the feed", false)],
        "page",
        Vec::new(),
    )));
    assert_eq!(app.active_page_title, "page");
}

#[test]
fn consecutive_deltas_fold_in_place_and_keep_the_freshest_status() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.hydration_generation = 10;
    app.active_channel = "general".into();

    let _ = app.__update(__DucktapeMessage::LiveUpdated(posted_delta(
        "general",
        message(1, "first", false),
    )));
    let _ = app.__update(__DucktapeMessage::LiveUpdated(posted_delta(
        "general",
        message(2, "second", false),
    )));
    assert_eq!(app.messages.len(), 2);
    assert_eq!(app.messages[1].body, "second");
    assert_eq!(
        app.hydration_generation, 10,
        "chat deltas never start a reload cycle"
    );
}

#[test]
fn resyncs_cannot_retarget_drafts_to_fallback_contexts() {
    let (mut app, _) = Ducktape::__boot();
    app.connected_rpc = "http://node".into();
    app.loading = false;
    app.hydration_generation = 7;
    app.active_channel = "deleted-channel".into();
    // The rail's instance has to materialize BEFORE the drawer goes up: the
    // screen draws the rail under `!channel_settings_open`, so a capture with
    // the drawer open would find nothing to name.
    app.active_thread_seq = 7;
    let rail = reply_composer_scope(&mut app);
    type_into(&mut app, &rail, ComposerKind::Reply, "thread reply");
    app.selected_message_seq = 7;
    app.selected_message_rev = 2;
    app.message_action = MessageAction::Editing;
    app.message_edit_draft = "message edit".into();
    app.channel_settings_open = true;
    app.channel_name_draft = "channel rename".into();
    app.member_key_draft = "member".into();
    app.thread_generation = 4;
    app.thread_target_seq = 9;
    app.thread_messages = vec![message(7, "old thread", false)];
    app.thread_next_reply_seq = 4;
    app.thread_has_more = true;
    app.thread_loading = true;
    app.active_page = "deleted-page".into();

    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        7,
        "fallback-channel",
        vec![message(7, "same sequence, other channel", false)],
        "fallback-page",
        Vec::new(),
    )));

    assert_eq!(app.active_channel, "fallback-channel");
    assert_eq!(app.selected_message_seq, 0);
    assert_eq!(app.selected_message_rev, 0);
    assert_eq!(app.message_action, MessageAction::Toolbar);
    assert!(app.message_edit_draft.is_empty());
    assert!(!app.channel_settings_open);
    assert!(app.channel_name_draft.is_empty());
    assert!(app.member_key_draft.is_empty());
    assert_eq!(app.active_thread_seq, 0);
    assert_eq!(app.thread_generation, 5);
    assert_eq!(app.thread_target_seq, 0);
    assert!(app.thread_messages.is_empty());
    assert_eq!(app.thread_next_reply_seq, 0);
    assert!(!app.thread_has_more);
    assert!(!app.thread_loading);
    // The rail closed under her, and her words did NOT go with it: the reply
    // composer is that thread's own retained instance (ducktape-ui#697), so
    // it waits under its key for the rail to reopen there.
    assert_eq!(composer_text(&app, &rail), "thread reply");
    assert_eq!(app.active_page, "fallback-page");
}

#[test]
fn mutation_acks_preserve_open_editors_and_thread_state() {
    let (mut app, _) = Ducktape::__boot();
    app.active_channel = "general".into();
    app.selected_message_seq = 7;
    app.selected_message_rev = 2;
    app.message_action = MessageAction::Editing;
    app.message_edit_draft = "edit in progress".into();
    app.active_thread_seq = 9;
    app.thread_target_seq = 10;
    app.thread_messages = vec![message(9, "thread root", false)];
    app.thread_next_reply_seq = 3;
    app.thread_has_more = true;
    let rail = reply_composer_scope(&mut app);
    let stream = composer_scope(&mut app);
    type_into(&mut app, &rail, ComposerKind::Reply, "reply in progress");
    type_into(&mut app, &stream, ComposerKind::Message, "next message");
    app.mutation_phase = MutationPhase::Channel;

    // an unrelated mutation's ack carries no snapshot — nothing to stomp
    // (reactions no longer route through ChatAcked at all; a channel op is
    // the surviving non-message phase)
    let _ = app.__update(__DucktapeMessage::ChatAcked(true));

    assert_eq!(app.selected_message_seq, 7);
    assert_eq!(app.message_action, MessageAction::Editing);
    assert_eq!(app.message_edit_draft, "edit in progress");
    assert_eq!(app.active_thread_seq, 9);
    assert_eq!(app.thread_target_seq, 10);
    assert_eq!(app.thread_messages.len(), 1);
    assert_eq!(app.thread_next_reply_seq, 3);
    assert!(app.thread_has_more);
    assert_eq!(composer_text(&app, &rail), "reply in progress");
    assert_eq!(composer_text(&app, &stream), "next message");
    assert_eq!(app.mutation_phase, MutationPhase::Idle);
}

/// A HISTORY PAGE BELONGS TO THE CHANNEL THAT ASKED FOR IT. The compiler drops
/// a superseded `history` run, while `HistoryPageData` carries the channel for
/// room movement that starts no replacement history request.
///
/// The flag is released ABOVE that check: a page dropped for landing in the
/// wrong room must still free "Load older", which `load_more_history` refuses
/// while `history_loading` stands.
#[test]
fn a_history_page_prepends_only_into_the_channel_that_asked_for_it() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.mutation_phase = MutationPhase::Idle;
    app.active_channel = "a".into();
    app.messages = vec![message(10, "a-ten", false)];
    let _ = app.__update(__DucktapeMessage::LoadMoreHistory);
    assert!(app.history_loading);

    // The reader is on #b by the time the page lands. `active_channel` moves
    // under an open request on the resync, the search-hit and the create routes
    // too, without necessarily starting another history request.
    app.active_channel = "b".into();
    app.messages = vec![message(10, "b-ten", false)];
    let _ = app.__update(__DucktapeMessage::HistoryLoaded(backend::HistoryPageData {
        channel_id: "a".into(),
        messages: vec![message(1, "a-one", false)],
        has_more: false,
    }));
    assert_eq!(
        app.messages.len(),
        1,
        "a page for #a must not prepend into #b's timeline"
    );
    assert_eq!(app.messages[0].body, "b-ten");
    assert!(
        !app.history_loading,
        "the dropped page still frees `Load older` in the room she is in"
    );

    // The same page stamped for #b IS #b's history, and prepends.
    let _ = app.__update(__DucktapeMessage::LoadMoreHistory);
    let _ = app.__update(__DucktapeMessage::HistoryLoaded(backend::HistoryPageData {
        channel_id: "b".into(),
        messages: vec![message(1, "b-one", false)],
        has_more: false,
    }));
    assert_eq!(app.messages.len(), 2);
    assert_eq!(app.messages[0].body, "b-one");
    assert!(!app.history_loading);

    // AND THE FLAG DOES NOT SURVIVE ANY ROUTE THAT ABANDONS ITS REQUEST.
    // `load_more_history` returns early on it, so until the abandoned page lands
    // — forever if it hangs — "Load older" is dead in the room she lands in.
    // Every LAUNCH that starts a room transition is here, not just the two
    // channel pickers: the search hit and the create both land in a different
    // room, and the reconnect and the console open drop the socket the page was
    // requested on, so those two may never answer at all.
    for abandoning in [
        __DucktapeMessage::ChooseChannel("b".into()),
        __DucktapeMessage::ChooseDm("peer".into()),
        __DucktapeMessage::OpenChatSearchHit("b".into(), 7, 7),
        __DucktapeMessage::CreateChannelSubmit,
        __DucktapeMessage::Reconnect,
        __DucktapeMessage::ConsoleOpened(iced::window::Id::unique()),
    ] {
        let (mut app, _) = Ducktape::__boot();
        app.loading = false;
        app.mutation_phase = MutationPhase::Idle;
        app.active_channel = "a".into();
        app.messages = vec![message(10, "a-ten", false)];
        // `create_channel_submit` refuses an empty draft; the rest ignore it.
        app.channel_draft = "new-room".into();
        let _ = app.__update(__DucktapeMessage::LoadMoreHistory);
        assert!(
            app.history_loading,
            "the route must start with a live request"
        );
        let route = format!("{abandoning:?}");
        let _ = app.__update(abandoning);
        assert!(
            !app.history_loading,
            "{route} abandons the request, so it must release the flag"
        );
    }
}

/// A RESYNC IS THE ONE DROPPER THAT MUST ASK. Every other route that abandons a
/// history request is a launch the reader drove, so it clears the flag flatly.
/// `live_resynced` is server-driven and moves `active_channel` on its own, so a
/// flat clear would strand a page that is still legitimately coming: the reducer
/// refuses any page arriving with the flag already down (`|| !history_loading`),
/// which would drop it silently and leave the timeline short.
#[test]
fn a_resync_releases_load_older_only_when_it_moves_the_room() {
    for (landing, expected) in [("a", true), ("b", false)] {
        let (mut app, _) = Ducktape::__boot();
        app.loading = false;
        app.mutation_phase = MutationPhase::Idle;
        app.active_channel = "a".into();
        app.messages = vec![message(10, "a-ten", false)];
        let _ = app.__update(__DucktapeMessage::LoadMoreHistory);
        assert!(app.history_loading);

        let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
            app.hydration_generation,
            landing,
            vec![message(10, "ten", false)],
            "",
            Vec::new(),
        )));
        assert_eq!(
            app.history_loading,
            expected,
            "a resync landing on #{landing} from #a must {} the flag",
            if expected { "keep" } else { "release" }
        );
    }
}

/// A MIRRORED VIEW READING IS ONLY AS GOOD AS ITS WRITERS, SO THE WRITERS ARE
/// PINNED. These fields exist purely so the view stops paying for them —
/// sidebar rows, page-comment anchors, huddle tile mute readings,
/// `post_refusal`, and `active_dm` — because a
/// `sync` extern takes every list BY VALUE and a call in a view expression is
/// therefore a deep clone per frame (the room projection also ran a SHA-256 per DM
/// peer, twice a frame). The trade is real: a mirror that a writer forgets is a
/// sidebar listing DMs under CHANNELS, an unread dot that never lights, a
/// composer refused in a room she may post in, or a stranger's face over the
/// header — none of which any type checker can see.
///
/// So the rule is mechanical and checked here: a handler that assigns any of a
/// mirror's SOURCES assigns the mirror too. That is what makes mirroring
/// cheaper than the per-frame call instead of six chances to drift, and it is
/// the same shape as the caret-retire and room-mover lints above.
#[test]
fn every_writer_of_a_mirrored_view_reading_refreshes_its_mirror() {
    // (mirror, the sources whose movement invalidates it). THIS ACCOUNT'S
    // NUMBER decides which channels are its own DMs (both ends of a DM hash the
    // same pair of numbers); THIS DEVICE'S KEY decides whether it is seated in
    // a members-only room.
    const MIRRORS: [(&str, &[&str]); 8] = [
        (
            "rooms",
            &["channels", "dm_peers", "account_number", "channel_reads"],
        ),
        ("dm_rows", &["channels", "dm_peers", "channel_reads"]),
        (
            "block_comment_rows",
            &["blocks", "block_comment_threads", "active_page"],
        ),
        (
            "active_thread_anchor",
            &["blocks", "active_thread_target", "active_page"],
        ),
        (
            "huddle_rows",
            &["huddle_roster", "call_peers", "call_muted"],
        ),
        ("fs_preview_entry", &["fs_entries", "fs_preview_path"]),
        (
            "post_refusal",
            &[
                "channel_members",
                "active_channel_archived",
                "active_channel_members_only",
                "settings_user_key",
            ],
        ),
        ("active_dm", &["active_dm_peer", "dm_peers"]),
    ];

    // Every handler file, because a mirror's source can move in any of them.
    macro_rules! handler_sources {
        ($($path:literal),* $(,)?) => { [$(($path, include_str!(concat!("../", $path)))),*] };
    }
    let files = handler_sources![
        "ui/handlers/chat.ice",
        "ui/handlers/files.ice",
        "ui/handlers/forge.ice",
        "ui/handlers/huddle.ice",
        "ui/handlers/lifecycle.ice",
        "ui/handlers/node.ice",
        "ui/handlers/onboarding.ice",
        "ui/handlers/overlays.ice",
        "ui/handlers/pages.ice",
        "ui/handlers/roster.ice",
    ];

    // An ASSIGNMENT opens a statement line — prose naming a field, and a call
    // that merely READS one, are not writes.
    let assigns = |body: &str, field: &str| {
        let statement = format!("{field} = ");
        body.lines()
            .any(|line| line.trim_start().starts_with(&statement))
    };

    let mut checked = 0usize;
    for (path, source) in files {
        for block in source
            .split(
                "
on ",
            )
            .skip(1)
        {
            let handler = block.split('(').next().unwrap_or(block).trim();
            let handler = handler.lines().next().unwrap_or(handler).trim();
            for (mirror, sources) in MIRRORS {
                let Some(moved) = sources.iter().find(|field| assigns(block, field)) else {
                    continue;
                };
                checked += 1;
                assert!(
                    assigns(block, mirror),
                    "{path}: `on {handler}` assigns `{moved}`, so it must also                      assign `{mirror}` — the view reads the mirror and never                      recomputes it (see state/chat.ice)"
                );
            }
        }
    }
    // The sweep must actually have found writers: a rename that silently
    // stopped matching would otherwise pass with nothing checked at all.
    assert!(
        checked >= 20,
        "the mirror sweep matched only {checked} writers — it has stopped seeing them"
    );
}

#[test]
fn history_windows_offer_a_jump_back_to_latest() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.active_channel = "general".into();

    // landing on a search hit enters history mode…
    let _ = app.__update(__DucktapeMessage::ChatHitLoaded(chat_data(
        "general",
        vec![message(7, "an old message", false)],
    )));
    assert!(app.history_view);

    // …and a plain channel load (the Jump-to-latest path) leaves it
    let _ = app.__update(__DucktapeMessage::ChatUpdated(chat_data(
        "general",
        vec![message(50, "the latest", false)],
    )));
    assert!(!app.history_view);

    let chat = inlined(include_str!("../ui/screens/chat.ice"));
    assert!(chat.contains("button \"Jump to latest\""));
    assert!(chat.contains("-> emit(choose_channel, active_channel)"));
}

/// THE BANNER DESCRIBES THE ROWS IN HAND, SO EVERY WRITER OF THEM ANSWERS IT.
///
/// `history_view` was raised by the search hit and lowered by a channel load,
/// and by nothing else — so a resync (a `files` write in another window, a
/// teammate joining a huddle, any plane op at all) replaced the window with
/// `load_chat_data`'s LATEST page and left the amber "Viewing history" banner
/// up over the live tail, with a "Jump to latest" that reloads the channel the
/// reader is already at the end of. Same after a create.
#[test]
fn a_resync_that_lands_the_live_tail_lowers_the_history_banner() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.active_channel = "general".into();
    let _ = app.__update(__DucktapeMessage::ChatHitLoaded(chat_data(
        "general",
        vec![message(7, "six months ago", false)],
    )));
    assert!(app.history_view);

    // a resync carrying no chat news leaves the window — and its banner — alone
    let _ = app.__update(__DucktapeMessage::LiveResynced(backend::LiveRefresh {
        chat_loaded: false,
        ..live_refresh(
            app.hydration_generation,
            "general",
            Vec::new(),
            "",
            Vec::new(),
        )
    }));
    assert!(
        app.history_view,
        "a pages-only resync did not touch the timeline, so the window stands"
    );

    // one that carries chat replaced it with the latest page
    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        app.hydration_generation,
        "general",
        vec![message(50, "the latest", false)],
        "",
        Vec::new(),
    )));
    assert!(
        !app.history_view,
        "the rows on screen are the tail now — the banner is a lie about them"
    );

    // and a create lands you in a brand-new room, which has no history at all
    let _ = app.__update(__DucktapeMessage::ChatHitLoaded(chat_data(
        "general",
        vec![message(7, "six months ago", false)],
    )));
    assert!(app.history_view);
    let _ = app.__update(__DucktapeMessage::ChannelCreated(chat_data(
        "brand-new",
        Vec::new(),
    )));
    assert!(!app.history_view);
}

/// A RESYNC ANSWERS WITH THE TAIL, SO IT FOLDS ONTO THE WINDOW — IT DOES NOT
/// REPLACE IT.
///
/// `load_chat_data` walks a bounded number of roots back from HEAD however far
/// the reader has paged, and the triggers are ordinary: a huddle join or leave
/// in the room on screen, a websocket reconnect, any chat op the delta path
/// cannot fold, the three chat failure resyncs. Assigning that page back threw
/// away every "Load older" page she had loaded — and, the scrollable staying
/// mounted at `anchor-y=end`, clamped her offset onto the top of the suddenly
/// short window, hundreds of rows forward from where she was reading, with no
/// banner and nothing to click to get back.
#[test]
fn a_chat_resync_keeps_the_pages_the_reader_paged_in() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.loading = false;
    app.active_channel = "general".into();
    app.channels = vec![room("general", 51)];

    // the live tail, then two "Load older" pages behind it
    let _ = app.__update(__DucktapeMessage::ChatUpdated(chat_data(
        "general",
        vec![
            message(50, "the tail", false),
            message(51, "and its next", false),
        ],
    )));
    let _ = app.__update(__DucktapeMessage::LoadMoreHistory);
    let _ = app.__update(__DucktapeMessage::HistoryLoaded(backend::HistoryPageData {
        channel_id: "general".into(),
        messages: vec![message(20, "older", false)],
        has_more: true,
    }));
    let _ = app.__update(__DucktapeMessage::LoadMoreHistory);
    let _ = app.__update(__DucktapeMessage::HistoryLoaded(backend::HistoryPageData {
        channel_id: "general".into(),
        messages: vec![message(2, "older still", false)],
        has_more: true,
    }));
    assert_eq!(backend::oldest_message_seq(app.messages.clone()), 2);
    assert!(
        app.history_view,
        "back-paging enters the bounded history window until Jump to latest"
    );

    // someone joins the huddle in this room, and the resync it forces answers
    // with the latest page alone
    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        app.hydration_generation,
        "general",
        vec![
            message(51, "and its next", false),
            message(52, "arrived meanwhile", false),
        ],
        "",
        Vec::new(),
    )));

    assert_eq!(
        app.messages.iter().map(|row| row.seq).collect::<Vec<_>>(),
        vec![2, 20, 50, 51, 52],
        "the pages she loaded survive, with the fresh tail spliced onto them"
    );
    assert!(
        app.has_older_history,
        "and 'Load older' still points past the oldest row she holds"
    );

    // A HISTORY WINDOW IS STILL REPLACED: it is not contiguous with the tail,
    // so merging the two would leave a hole in the middle that nothing pages in.
    // The real search-hit launch clears the old presentation window before
    // this response can land; mirror that launch boundary in the direct
    // reducer fixture so post-launch live rows, not pre-launch history, are
    // the only rows the landing merge may preserve.
    app.messages.clear();
    let _ = app.__update(__DucktapeMessage::ChatHitLoaded(chat_data(
        "general",
        vec![message(7, "six months ago", false)],
    )));
    assert!(app.history_view);
    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        app.hydration_generation,
        "general",
        vec![message(52, "arrived meanwhile", false)],
        "",
        Vec::new(),
    )));
    assert_eq!(
        app.messages.iter().map(|row| row.seq).collect::<Vec<_>>(),
        vec![52],
        "the window is dropped whole, and the banner goes with it"
    );
    assert!(!app.history_view);
}

/// AND A SPLICE THAT DOES NOT TOUCH IS NOT A SPLICE — it is a HOLE, and the
/// hole is permanent.
///
/// `ModuleEvent::Lagged` says the client fell so far behind that the missed ops
/// are gone; the resync it forces answers with the last N roots, which can start
/// PAST the newest row on screen. Merging those two windows draws today's
/// messages directly under a stretch that is simply missing, and nothing can
/// ever fill it: "Load older" pages back from `oldest_message_seq`, now the
/// far-back end, so every click walks further AWAY from the gap. `history_view`
/// is not the only non-contiguous landing, so the test is the rows themselves.
#[test]
fn a_resync_the_window_cannot_reach_replaces_it_rather_than_leaving_a_hole() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.loading = false;
    app.active_channel = "general".into();
    app.channels = vec![room("general", 900)];
    app.messages = vec![
        message(2, "she paged back this far", false),
        message(20, "and read up to here", false),
    ];
    // an in-flight send of her own is still on screen, seq -1 and no page
    app.messages.push(backend::ChatMessage {
        view_key: -1,
        seq: -1,
        pending: true,
        ..message(0, "mine, still sending", false)
    });

    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        app.hydration_generation,
        "general",
        vec![
            message(880, "the tail after the lag", false),
            message(900, "and today", false),
        ],
        "",
        Vec::new(),
    )));

    assert_eq!(
        app.messages
            .iter()
            .filter(|row| !row.pending)
            .map(|row| row.seq)
            .collect::<Vec<_>>(),
        vec![880, 900],
        "the unreachable window is dropped whole — no 20-then-880 seam"
    );
    assert_eq!(
        backend::oldest_message_seq(app.messages.clone()),
        880,
        "so 'Load older' walks back from the tail, into the gap and not past it"
    );
    assert!(
        app.messages.iter().any(|row| row.pending),
        "and her own in-flight send is not collateral"
    );
}

/// A PLANE OP IS NOT "JUMP TO LATEST".
///
/// The resync that lands on every files write, valset change, identity, agent
/// or governance op carries no chat — the search window and its amber banner are
/// still exactly what is on screen — and it used to mark the room read to a head
/// the reader has demonstrably not reached — and `mark_channel_read` only moves
/// forward, so the badge `chat_sidebar_rooms` paints off that cursor never comes
/// back. `chat_hit_loaded` refuses that write; this is the handler that was
/// undoing it one save later.
#[test]
fn a_plane_resync_leaves_a_search_window_and_its_badge_alone() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_channel = "general".into();
    // she read up to 10 when she connected; thirty messages have landed since
    app.channels = vec![room("general", 10)];
    app.channel_reads = backend::initial_channel_reads(app.channels.clone(), Vec::new());
    app.channels = vec![room("general", 40)];

    let _ = app.__update(__DucktapeMessage::ChatHitLoaded(chat_data(
        "general",
        vec![message(7, "six months ago", false)],
    )));
    assert!(app.history_view);
    assert!(
        app.rooms
            .iter()
            .any(|row| row.channel.id == "general" && row.unread)
    );

    let plane_only = backend::LiveRefresh {
        chat_loaded: false,
        pages_loaded: false,
        ..live_refresh(
            app.hydration_generation,
            "general",
            Vec::new(),
            "",
            Vec::new(),
        )
    };
    let _ = app.__update(__DucktapeMessage::LiveResynced(plane_only));

    assert!(
        app.history_view,
        "the banner is still the only way back to the tail"
    );
    assert_eq!(
        app.messages.iter().map(|row| row.seq).collect::<Vec<_>>(),
        vec![7],
        "and the window around the hit is still what she is reading"
    );
    assert!(
        app.rooms
            .iter()
            .any(|row| row.channel.id == "general" && row.unread),
        "so the room is no more read than it was before she saved a file"
    );
}

/// A HISTORY WINDOW IS A SNAPSHOT, NOT A LIVE TAIL.
///
/// The rows in hand are a window around one old message, so a post from today
/// has a seq past every one of them and `insert_committed_root` appends it to
/// the END of the window — today's message drawn directly under one from six
/// months ago, and (authors matching) folded into the same run, with no gap
/// marker anywhere. Marking the channel read off that fold is the same lie in
/// the sidebar: the reader is not caught up on a room she is reading backwards.
#[test]
fn a_live_post_does_not_splice_itself_into_a_history_window() {
    // the room as the sidebar knows it. re-seated by hand between steps: a
    // landing FOLDS its refreshed row into this list rather than installing
    // one (`upsert_channel_rows`), so the fixture has to put the row back.
    let room = || {
        vec![backend::ChatChannel {
            id: "general".into(),
            name: "general".into(),
            archived: false,
            members_only: false,
            huddle_count: 0,
            head_seq: 7,
        }]
    };
    let cursor = |app: &Ducktape| {
        app.channel_reads
            .iter()
            .find(|read| read.channel == "general")
            .map(|read| read.seq)
    };

    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.connected = true;
    app.active_channel = "general".into();
    // THE SIDEBAR ALREADY KNOWS THE ROOM'S HEAD when the hit lands, which is the
    // whole point: search is workspace-wide, so the hit routinely opens a room
    // with unread waiting, and `MessageWindow::Around` is not the tail.
    app.channels = room();
    let _ = app.__update(__DucktapeMessage::ChatHitLoaded(chat_data(
        "general",
        vec![message(7, "six months ago", false)],
    )));
    assert!(app.history_view);
    assert_eq!(
        cursor(&app),
        None,
        "opening a search hit is not catching up on the room it landed in"
    );
    assert!(
        app.rooms
            .iter()
            .any(|row| row.channel.id == "general" && row.unread),
        "so the badge she has not cleared is still lit"
    );
    app.channels = room();

    let _ = app.__update(__DucktapeMessage::LiveUpdated(posted_delta(
        "general",
        message(500, "posted just now", false),
    )));
    assert_eq!(
        app.messages.len(),
        1,
        "a message 493 seqs newer is not the next row of this window"
    );
    assert_eq!(
        cursor(&app),
        None,
        "and reading old scrollback is not being caught up on today's post"
    );

    // HER OWN SEND IS THE EXCEPTION. The composer posts from a window too and
    // splices the optimistic row in unconditionally, so a refused settle would
    // strand it `pending` forever.
    submit(&mut app, ComposerKind::Message, "mine, from the window");
    assert!(app.messages[1].pending);
    let mut settled = message(501, "mine, from the window", false);
    settled.id = app.messages[1].id.clone();
    app.channels = room();
    let _ = app.__update(__DucktapeMessage::LiveUpdated(posted_delta(
        "general", settled,
    )));
    assert_eq!(app.messages.len(), 2, "her row settled in place, not twice");
    assert!(!app.messages[1].pending, "and the row becomes canonical");
    assert_eq!(
        cursor(&app),
        None,
        "settling her own send is still not catching up on the room"
    );

    // Jump to latest, and the tail is live again.
    // `choose_channel` clears the history window before starting this load.
    // This fixture delivers the response directly, so apply the launch-side
    // clear explicitly; a same-room committed row after this point would be a
    // live RTT arrival and must be retained by the landing merge.
    app.messages.clear();
    let _ = app.__update(__DucktapeMessage::ChatUpdated(chat_data(
        "general",
        vec![message(499, "the latest", false)],
    )));
    app.channels = room();
    // 502, not 500: her own send settled at 501 two steps up, so a post that
    // arrives after the jump is newer than that.
    let _ = app.__update(__DucktapeMessage::LiveUpdated(posted_delta(
        "general",
        message(502, "posted just now", false),
    )));
    assert_eq!(app.messages.len(), 2);
    assert_eq!(app.messages[1].body, "posted just now");
    assert_eq!(
        cursor(&app),
        Some(502),
        "the tail marks the room read as it arrives"
    );
}

/// Live delivery does not own the server's page cursor. A reply may arrive on
/// the stream while the page that contains it is in flight; the page replaces
/// the cursor and the row merge deduplicates their overlap.
#[test]
fn live_reply_overlapping_a_page_keeps_one_row_and_the_server_cursor() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_channel = "general".into();
    app.active_thread_seq = 1;
    let mut first_reply = message(2, "first reply", false);
    first_reply.thread_seq = 1;
    app.thread_messages = vec![message(1, "the root", false), first_reply];
    app.thread_next_reply_seq = 2;
    app.thread_has_more = true;
    app.thread_generation = 7;
    app.thread_loading = true;

    let mut streamed = message(3, "streamed reply", false);
    streamed.thread_seq = 1;
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: LiveKind::Chat,
        status: "Live".into(),
        height: 3,
        chat: vec![backend::ChatDelta::Reply {
            channel_id: "general".into(),
            seq: 3,
            root_seq: 1,
            message: streamed,
        }],
        ..backend::LiveUpdate::default()
    }));
    assert_eq!(app.thread_next_reply_seq, 2);
    assert!(app.thread_has_more);

    let mut overlap = message(3, "streamed reply", false);
    overlap.thread_seq = 1;
    let mut next = message(4, "next reply", false);
    next.thread_seq = 1;
    let _ = app.__update(__DucktapeMessage::ThreadPageLoaded(
        backend::ThreadPageData {
            generation: 7,
            messages: vec![overlap, next],
            next_reply_seq: 4,
            has_more: true,
        },
    ));

    assert_eq!(
        app.thread_messages
            .iter()
            .filter(|message| message.seq == 3)
            .count(),
        1
    );
    assert!(app.thread_messages.iter().any(|message| message.seq == 4));
    assert_eq!(app.thread_next_reply_seq, 4);
    assert!(app.thread_has_more);
    assert_eq!(app.thread_messages.len(), 4, "root plus three replies");
}

/// A CHAT-ONLY RESYNC MUST NOT CLAIM THE PAGE IT CARRIES NO NEWS ABOUT. The
/// click blanks the pane and moves `active_page`; a resync that arrives with
/// `pages_loaded == false` keeps the empty `blocks` and canonicalises
/// `title + []` into a document the node never sent. Stamping `buffer_page`
/// for that fabrication hands `page_autosave_tick` a blank document it is
/// willing to write over the real page.
#[test]
fn a_chat_only_resync_does_not_claim_the_page_it_never_loaded() {
    let mut app = reading_alpha();
    let _ = app.__update(__DucktapeMessage::ChoosePage("beta".into()));
    assert!(app.buffer_page.is_empty(), "the click released the buffer");

    let mut chat_only = live_refresh(app.hydration_generation, "", Vec::new(), "", Vec::new());
    chat_only.pages_loaded = false;
    chat_only.active_page = String::new();
    let _ = app.__update(__DucktapeMessage::LiveResynced(chat_only));

    assert!(
        app.buffer_page.is_empty(),
        "a resync carrying no page news must not claim the page as the buffer's"
    );

    // And the tick still refuses, which is the consequence that matters.
    let _ = app.__update(__DucktapeMessage::Failed(backend::AppError {
        message: "node blip".into(),
        committed: false,
    }));
    app.page_editor = compose("h");
    let _ = app.__update(__DucktapeMessage::PageAutosaveTick);
    assert_eq!(
        app.block_autosave_status,
        AutosaveStatus::Idle,
        "a fabricated buffer must never be saved into a real page"
    );
}

/// A PLANE'S OP REFETCHES THAT PLANE AND NO OTHER.
///
/// These five modules feed surfaces that were correct only at connect and at
/// tab-switch time: a validator joining, a proposal being voted, a device being
/// renamed, an agent registering, a file being committed — none of it reached a
/// console already looking at the page that shows it.
///
/// The generation counters ARE the assertion: each is the refetch's own guard,
/// so one moving means exactly that plane was asked for, and the others holding
/// means nothing else was.
#[test]
fn a_plane_op_refetches_only_the_plane_it_names() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;

    let plane = |app: &mut Ducktape, module: &str| {
        let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
            kind: LiveKind::Plane,
            status: "Live".into(),
            height: 12,
            module: module.into(),
            ..backend::LiveUpdate::default()
        }));
    };

    let (members, gov, agents, account, dm, fs) = (
        app.members_generation,
        app.gov_generation,
        app.agents_generation,
        app.account_generation,
        app.dm_peers_generation,
        app.fs_generation,
    );

    plane(&mut app, "valset");
    assert_eq!(app.members_generation, members + 1, "valset feeds members");
    assert_eq!(app.gov_generation, gov, "and nothing else");
    assert_eq!(app.fs_generation, fs);

    plane(&mut app, "governance");
    assert_eq!(app.gov_generation, gov + 1);
    assert_eq!(
        app.members_generation,
        members + 1,
        "unchanged by governance"
    );

    // identity feeds TWO surfaces: the account card and the DM directory.
    plane(&mut app, "identity");
    assert_eq!(app.account_generation, account + 1);
    assert_eq!(app.dm_peers_generation, dm + 1);

    plane(&mut app, "agent");
    assert_eq!(app.agents_generation, agents + 1);

    // AND `runs` FEEDS THE SAME PROJECTION. `AgentRow.live` — the Forge seat's
    // dot — is read from the runs module's pending register, so a run
    // starting or ending changes a row while `agent` commits nothing. Its op
    // is the dot's ONLY off-tab signal.
    plane(&mut app, "runs");
    assert_eq!(app.agents_generation, agents + 2, "runs feeds agents too");
    assert_eq!(app.account_generation, account + 1, "and nothing else");

    plane(&mut app, "files");
    assert_eq!(app.fs_generation, fs + 1);

    // A module with no plane of its own moves nothing.
    let before = app.members_generation;
    plane(&mut app, "tagging");
    assert_eq!(
        app.members_generation, before,
        "an unrouted module is inert"
    );
}

#[test]
fn a_bounded_window_never_leaves_actions_targeting_an_evicted_row() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.mutation_phase = MutationPhase::Idle;
    app.active_channel = "general".into();
    app.messages = (1..=backend::CHAT_HOT_WINDOW_LIMIT as i64)
        .map(|seq| message(seq, &format!("message {seq}"), false))
        .collect();
    app.selected_message_seq = 1;
    app.selected_message_rev = 7;
    app.message_action = MessageAction::Delete;
    app.message_edit_draft = "stale edit".into();
    submit(&mut app, ComposerKind::Message, "new tail");
    assert_eq!(app.messages.first().map(|row| row.seq), Some(2));
    assert_eq!(app.selected_message_seq, 0);
    assert_eq!(app.selected_message_rev, 0);
    assert_eq!(app.message_action, MessageAction::Toolbar);
    assert!(app.message_edit_draft.is_empty());

    app.messages = (100..356)
        .map(|seq| message(seq, &format!("message {seq}"), false))
        .collect();
    app.selected_message_seq = 355;
    app.selected_message_rev = 9;
    app.message_action = MessageAction::Editing;
    app.message_edit_draft = "also stale".into();
    app.has_older_history = true;
    let _ = app.__update(__DucktapeMessage::LoadMoreHistory);
    let _ = app.__update(__DucktapeMessage::HistoryLoaded(backend::HistoryPageData {
        channel_id: "general".into(),
        messages: (1..100)
            .map(|seq| message(seq, &format!("older {seq}"), false))
            .collect(),
        has_more: false,
    }));
    assert!(!app.messages.iter().any(|row| row.seq == 355));
    assert_eq!(app.selected_message_seq, 0);
    assert_eq!(app.selected_message_rev, 0);
    assert_eq!(app.message_action, MessageAction::Toolbar);
    assert!(app.message_edit_draft.is_empty());

    let root = message(1, "thread root", false);
    let mut replies: Vec<_> = (2..=backend::THREAD_HOT_WINDOW_LIMIT as i64)
        .map(|seq| {
            let mut reply = message(seq, &format!("reply {seq}"), false);
            reply.thread_seq = 1;
            reply
        })
        .collect();
    app.thread_messages = std::iter::once(root).chain(replies.drain(..)).collect();
    app.active_thread_seq = 1;
    app.thread_selected_seq = 2;
    app.thread_selected_rev = 3;
    app.thread_message_action = MessageAction::Delete;
    app.thread_edit_draft = "evicted reply".into();
    submit(&mut app, ComposerKind::Reply, "new reply at the tail");
    assert!(!app.thread_messages.iter().any(|row| row.seq == 2));
    assert_eq!(app.thread_selected_seq, 0);
    assert_eq!(app.thread_selected_rev, 0);
    assert_eq!(app.thread_message_action, MessageAction::Toolbar);
    assert!(app.thread_edit_draft.is_empty());
}

/// SCROLLING NEAR THE TOP STARTS THE PAGE. The offset arrives relative to the
/// scrollable's anchor, and the stream is bottom-anchored, so 1.0 is the top.
#[test]
fn approaching_the_top_of_the_scrollback_prefetches_the_older_page() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.active_channel = "a".into();
    app.messages = vec![message(40, "oldest loaded", false)];
    app.has_older_history = true;

    // Mid-scrollback: nothing happens, and no message means no view pass spent.
    let _ = app.__update(__DucktapeMessage::ChatScrolled(0.0, 120.0, 0.0, 0.4));
    assert!(!app.history_loading);

    // Content that FITS reports 0/0. Nothing scrolls, so nothing is approached
    // — and the explicit button is already on screen.
    let _ = app.__update(__DucktapeMessage::ChatScrolled(0.0, 0.0, 0.0, f64::NAN));
    assert!(!app.history_loading);

    let _ = app.__update(__DucktapeMessage::ChatScrolled(0.0, 900.0, 0.0, 0.95));
    assert!(app.history_loading, "the older page is already on its way");
    // And it does not fan out: the in-flight page holds the next steps off.
    let _ = app.__update(__DucktapeMessage::ChatScrolled(0.0, 950.0, 0.0, 0.98));
    assert!(app.history_loading);
}
