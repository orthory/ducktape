use super::*;

/// THE ROUTE LIST IS THE INVARIANT, SO THE ROUTE LIST IS PINNED. A ninth handler
/// that moves the reader between rooms has to decide whether it abandons a
/// history request, and nothing about writing one would prompt that thought —
/// which is exactly how the five uncovered routes above got written. This fails
/// the build on a new mover so the decision is forced, rather than trusting the
/// next author to remember an invariant spread across three files.
///
/// LAUNCHES clear the flag themselves. LANDINGS do not, and must not be added
/// here without checking that every launch reaching them already cleared it:
/// `chat_updated` answers the two pickers, `chat_hit_loaded` answers the search
/// hit, `channel_created` answers the create, `workspace_connected` answers the
/// reconnect. `live_resynced` is a landing with NO launch behind it, which is
/// why it is the one that asks.
#[test]
fn every_handler_that_moves_the_reader_between_rooms_is_accounted_for() {
    const HANDLERS: &str = concat!(
        include_str!("../ui/handlers/chat.ice"),
        include_str!("../ui/handlers/lifecycle.ice"),
        include_str!("../ui/handlers/onboarding.ice"),
    );

    let mut handler = "";
    let mut movers: Vec<&str> = Vec::new();
    for line in HANDLERS.lines() {
        if let Some(rest) = line.strip_prefix("on ") {
            handler = rest.split('(').next().unwrap_or(rest).trim();
        }
        if line.trim_start().starts_with("active_channel = ") {
            movers.push(handler);
        }
    }
    movers.sort_unstable();
    movers.dedup();

    assert_eq!(
        movers,
        [
            "channel_created",
            "chat_hit_loaded",
            "chat_updated",
            "choose_channel",
            "choose_dm",
            "console_opened",
            "live_resynced",
            "open_chat_search_hit",
            "reconnect",
            "workspace_connected",
        ],
        "a handler started or stopped moving `active_channel`: decide whether it \
         abandons an in-flight history page, then update this list"
    );

    // And the launches genuinely carry the clear — a mover list alone would pass
    // with every clear deleted.
    for launch in [
        "choose_channel",
        "choose_dm",
        "open_chat_search_hit",
        "create_channel_submit",
        "reconnect",
        "console_opened",
    ] {
        let body = HANDLERS
            .split(&format!("\non {launch}"))
            .nth(1)
            .unwrap_or_else(|| panic!("{launch} is a handler"))
            .split("\non ")
            .next()
            .expect("handler body");
        assert!(
            body.contains("history_loading = false"),
            "{launch} abandons a history request and must release the flag"
        );
    }

    // THE COMPOSER IS PER-ROOM, AND ITS TWO LINES ARE ORDER-DEPENDENT. The park
    // must read `active_channel` while it still names the room being LEFT and
    // the restore must read it once it names the room being ENTERED, so the
    // rule is not "both lines are present" but "park, move, restore" — a
    // restore above the move hands the old room its own draft back and the new
    // one whatever it was already holding.
    //
    // `channel_created` IS ON THIS LIST, and was the route that proved it has
    // to be: creating a channel lands the reader IN it (`create_channel_submit`
    // abandons the old room's load for exactly that reason), so a composer left
    // alone there followed her into the new room armed to send — and the next
    // switch parked those words under the NEW room's id, silently reattributing
    // them. The three landings that also write `active_channel`
    // (`chat_updated`, `chat_hit_loaded`, `live_resynced`) are NOT switches:
    // they re-affirm or correct the id of the room already on screen, so a park
    // there would file the live composer under a room she never left.
    //
    // AND ONE SWITCH IS SPREAD OVER TWO HANDLERS, which is how it escaped: the
    // pair is `reconnect` (blanks the room, carrying the live composer across)
    // and `workspace_connected` (lands on `landing_channel(channels)` — the
    // first room with traffic, which is rarely the room she left). Both halves
    // are named here so the ordering rule reaches across the round trip.
    let body_of = |handler: &str| {
        HANDLERS
            .split(&format!("\non {handler}"))
            .nth(1)
            .unwrap_or_else(|| panic!("{handler} is a handler"))
            .split("\non ")
            .next()
            .expect("handler body")
    };
    let at = |handler: &str, body: &str, token: &str| {
        body.lines()
            .position(|line| line.trim_start().starts_with(token))
            .unwrap_or_else(|| {
                panic!(
                    "{handler} moves the reader between contexts and must carry \
                     `{token}` — a composer that follows her is armed to send \
                     what she wrote next door into the room she clicked"
                )
            })
    };
    for (leaves, lands) in [
        ("choose_channel", "choose_channel"),
        ("choose_dm", "choose_dm"),
        ("open_chat_search_hit", "open_chat_search_hit"),
        ("channel_created", "channel_created"),
        ("reconnect", "workspace_connected"),
    ] {
        let out = body_of(leaves);
        let landing = body_of(lands);
        let park = at(leaves, out, "message_drafts = park_message_draft(");
        let rail_park = at(leaves, out, "reply_drafts = park_reply_draft(");
        let left = at(leaves, out, "active_channel = ");
        let arrived = at(lands, landing, "active_channel = ");
        let restore = at(
            lands,
            landing,
            "message_editor = editor(parked_message_draft(",
        );
        assert!(
            park < left && rail_park < left && arrived < restore,
            "{leaves} must park BOTH composers BEFORE it moves `active_channel` \
             and {lands} must restore AFTER (park {park}, rail park {rail_park}, \
             move {left}, landing move {arrived}, restore {restore})"
        );
    }

    // AND THE RAIL'S OWN SWITCH OBEYS THE SAME RULE ONE LEVEL DOWN, on
    // `active_thread_seq` instead of `active_channel`. `open_thread_for` is
    // what every "N replies" row in the timeline emits, so this is the ordinary
    // click that used to destroy a half-typed reply.
    //
    // THE PARK MUST SIT ABOVE THE WRITE, not merely inside the handler:
    // `park_reply_draft` refuses `thread_seq <= 0` outright, so a park read
    // below a line that can zero the seq — `live_resynced`'s deleted-root and
    // channel-move arms — is a guaranteed no-op that files nothing.
    for parker in ["open_thread_for", "live_resynced"] {
        let body = body_of(parker);
        let park = at(parker, body, "reply_drafts = park_reply_draft(");
        let moved = at(parker, body, "active_thread_seq = ");
        assert!(
            park < moved,
            "{parker} must park the reply BEFORE it moves `active_thread_seq` — \
             a park below the move reads a seq that no longer names the thread \
             (park {park}, move {moved})"
        );
    }

    // AND EVERY LANDING THAT SEATS A THREAD RESTORES BESIDE THE WRITE. Arriving
    // in a thread by any other route left an empty box over parked words, and
    // the first character typed into it parked OVER them under the same key —
    // silent overwrite, not just loss of a live buffer. `chat_hit_loaded` is the
    // reachable one (`load_chat_hit` answers with `root.seq` for a reply hit);
    // `chat_updated` and `channel_created` answer 0 today and ride the same
    // rule so a payload that starts seating a thread cannot forget it.
    //
    // THE SEATERS ARE DERIVED, NOT LISTED, exactly as the movers above are. A
    // literal `active_thread_seq = 0` RETIRES the rail rather than seating one,
    // so the zero writers stay off this list; every other writer lands on it,
    // and a new one fails the pin below until its restore decision is made.
    let mut handler = "";
    let mut seaters: Vec<&str> = Vec::new();
    for line in HANDLERS.lines() {
        if let Some(rest) = line.strip_prefix("on ") {
            handler = rest.split('(').next().unwrap_or(rest).trim();
        }
        let writes = line.trim_start().starts_with("active_thread_seq = ");
        let retires = line.trim() == "active_thread_seq = 0";
        if writes && !retires {
            seaters.push(handler);
        }
    }
    seaters.sort_unstable();
    seaters.dedup();

    assert_eq!(
        seaters,
        [
            "channel_created",
            "chat_hit_loaded",
            "chat_updated",
            "live_resynced",
            "open_thread_for",
            "thread_loaded",
        ],
        "a handler started or stopped seating `active_thread_seq`: decide \
         whether it restores the parked reply beside the write, then update \
         this list"
    );

    for landing in &seaters {
        // Two seaters land under a rail whose LIVE buffer is the truth, so
        // they must NOT restore — each is pinned to that refusal below.
        let live_buffer_is_the_truth = matches!(*landing, "live_resynced" | "thread_loaded");
        if live_buffer_is_the_truth {
            continue;
        }
        let body = body_of(landing);
        let moved = at(landing, body, "active_thread_seq = ");
        let restore = at(landing, body, "reply_editor = editor(parked_reply_draft(");
        assert!(
            moved < restore,
            "{landing} seats `active_thread_seq` and must restore the parked \
             reply AFTER it (move {moved}, restore {restore})"
        );
    }
    assert!(
        !body_of("thread_loaded").contains("parked_reply_draft("),
        "thread_loaded lands under a rail that is already open and typeable — a \
         restore there overwrites the keystrokes the round trip collected"
    );
    assert!(
        !body_of("live_resynced").contains("parked_reply_draft("),
        "live_resynced either leaves the rail on the thread it was already on — \
         where the live buffer is the truth — or closes it, and a closed rail \
         has no composer to fill"
    );

    // TWO READINGS OF THE ROOM RIDE WITH IT. `active_dm_peer` decides whether
    // the header names a peer instead of the channel (suppressing the `#` and
    // the channel name with it), and `history_view` decides whether the amber
    // banner claims these rows are old scrollback. Both used to be written by
    // one handler each — the DM picker and the search hit — so every OTHER
    // route that moved the room left them describing a pane that was gone.
    // A mover answers both or the build fails here.
    for mover in movers {
        let body = HANDLERS
            .split(&format!("\non {mover}"))
            .nth(1)
            .unwrap_or_else(|| panic!("{mover} is a handler"))
            .split("\non ")
            .next()
            .expect("handler body");
        // An ASSIGNMENT, not a mention: the token opens a statement line, so
        // prose naming the field where the write used to be fails here.
        for reading in ["active_dm_peer = ", "history_view = "] {
            assert!(
                body.lines()
                    .any(|line| line.trim_start().starts_with(reading)),
                "{mover} moves the reader between rooms and must answer \
                 `{reading}` — a reading of the room cannot outlive it"
            );
        }
    }
}

/// THE DM HEADER NAMES A PEER, AND THE ROOM IT NAMES HIM FOR IS `active_channel`.
///
/// A non-empty `active_dm_peer` draws `DmHeader` AND suppresses both the `#`
/// glyph and `active_channel_name`, so a peer that outlived the room he named
/// put Alice's face over #general's timeline with the room the composer posts
/// into never named — and left two sidebar rows reading as selected. It is a
/// derivation of the room now, so no landing can disagree with the pane.
#[test]
fn a_landing_in_another_room_retires_the_dm_header() {
    let me = "aa";
    let peer = "bb";
    let dm = backend::dm_channel_id(me.into(), peer.into());

    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.settings_user_key = me.into();
    app.active_dm_peer = peer.into();
    app.active_channel = dm.clone();

    // a search hit jumps to an ordinary room…
    let _ = app.__update(__DucktapeMessage::ChatHitLoaded(chat_data(
        "general",
        vec![message(7, "an old message", false)],
    )));
    assert!(
        app.active_dm_peer.is_empty(),
        "the peer does not follow the reader into #general"
    );

    // …and a landing inside the DM itself keeps him
    app.active_dm_peer = peer.into();
    let _ = app.__update(__DucktapeMessage::ChatUpdated(chat_data(
        &dm,
        vec![message(1, "hey", false)],
    )));
    assert_eq!(app.active_dm_peer, peer, "this room IS his DM");

    // the resync is the landing with no launch behind it — it moves the room
    // on its own, which is how the peer used to survive every other route
    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        app.hydration_generation,
        "general",
        vec![message(9, "in the room she was moved to", false)],
        "",
        Vec::new(),
    )));
    assert!(app.active_dm_peer.is_empty());

    // BUT A RESYNC THAT MOVED NO ROOM DERIVES NOTHING. `choose_dm` names the
    // peer optimistically and leaves `active_channel` on the room being left
    // for the several blocks `open_dm` takes to answer; a pages-only resync
    // landing in that window would otherwise derive the peer against the OLD
    // room and blank him, and `chat_updated` then derives "" from "" — the DM
    // opens under a `#` for good.
    app.active_dm_peer = peer.into();
    app.active_channel = "general".into();
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
    assert_eq!(
        app.active_dm_peer, peer,
        "the room did not move, so nothing about it was re-read"
    );

    // NOR DOES A CHAT-CARRYING ONE INSIDE THAT SAME WINDOW. `live_resync_load`
    // is launched with today's `active_channel`, so a `ready`/`Lagged{chat}`
    // resync lands `chat_loaded` on the room being LEFT — deriving against it
    // blanks the peer just as permanently as the pages-only case above.
    app.active_dm_peer = peer.into();
    app.active_channel = "general".into();
    app.loading = true;
    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        app.hydration_generation,
        "general",
        Vec::new(),
        "",
        Vec::new(),
    )));
    assert_eq!(
        app.active_dm_peer, peer,
        "a landing is in flight — it answers for the peer, this resync does not"
    );
    app.loading = false;

    // a device with no user key derives no DM id, so it holds no DM — the same
    // answer `chat_sidebar_rooms` gives when `me` is empty
    app.settings_user_key = String::new();
    app.active_dm_peer = peer.into();
    let _ = app.__update(__DucktapeMessage::ChatUpdated(chat_data(
        &dm,
        vec![message(1, "hey", false)],
    )));
    assert!(app.active_dm_peer.is_empty());
}

/// THE DM HEADER IS THE ROW'S FLEXIBLE CHILD, exactly as the channel title is.
///
/// `align=center` is CROSS-axis only and iced's Row has no main-axis
/// justification, so the header's right-hand cluster — the huddle control and
/// the ⋯ that is the only mouse route to Channel details — sits at the right
/// edge only while some child takes the row's slack. The channel arm has a
/// `box w=fill clip=true` around its title for exactly this; the DM arm mounted
/// `DmHeader` bare, so ⋯ packed against the peer's name and moved with its
/// length, and a long name pushed the huddle control and ⋯ past the pane's clip.
///
/// It also branches on the resolved NAME, not the key: `dm_peer_named` answers
/// a roster miss with the blank peer while the key stays set, so branching on
/// the key drew an empty plate with no name — never the fall-through to the
/// derived two-party title that three comments promise. ONE discriminant for
/// the whole surface: the thread rail draws the same room's breadcrumb, and a
/// rail still reading the KEY would print that room without its `#` while the
/// header above it printed one — two readings of one room, on screen together.
#[test]
fn the_dm_header_takes_the_slack_the_channel_title_would() {
    let screen = inlined(include_str!("../ui/screens/chat.ice"));
    assert!(screen.contains(
        "if !empty(active_dm.name)\n                    box w=fill clip=true\n                      DmHeader peer=active_dm"
    ));
    // The header's two fall-through arms (`#` glyph, channel title) and the
    // thread rail's breadcrumb, all reading the one derivation.
    assert_eq!(screen.matches("if empty(active_dm.name)").count(), 3);
    // No arm anywhere on this screen decides a title from the KEY, which
    // survives the roster miss the resolved row does not.
    assert!(!screen.contains("if empty(active_dm_peer)"));
    assert!(!screen.contains("if !empty(active_dm_peer)"));
}

// Opening a (possibly different) network through the console handoff clears
// every reading and draft of the previous one — and the in-flight huddle —
// while the KEY password survives: it unlocks this device's user.key, not an
// endpoint.
#[test]
fn opening_a_network_clears_the_previous_networks_state() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.connected_rpc = "http://node-a".into();
    app.rpc = "http://node-b".into();
    app.password = "device-key-password".into();
    app.selected_message_seq = 1;
    app.message_action = MessageAction::Editing;
    app.message_edit_draft = "node a edit".into();
    app.active_thread_seq = 1;
    app.reply_editor = compose("node a reply");
    app.page_editor = compose("node a page body");
    app.page_saved_text = "node a page body".into();
    app.block_comments_open = true;
    app.block_comments_target = "same-id".into();
    app.block_comment_draft = "node a comment".into();
    app.message_editor = compose("node a message");
    app.page_search_draft = "node a search".into();
    app.forge_list_phase = ForgePhase::Ready;
    app.forge_repo = "same-repo".into();
    app.forge_repo_phase = ForgePhase::Ready;
    app.forge_item_number = 1;
    app.forge_item_phase = ForgePhase::Ready;
    app.forge_review_draft = "node a review".into();
    app.huddle_joined = true;
    app.huddle_channel = "chan-a".into();
    // AND THE PARKS, which the by-name clears around them would otherwise miss.
    // A channel id is a user-chosen string, so both networks can hold a
    // `#general` and a park keyed on it would hand node A's sentence to node B.
    app.message_drafts =
        backend::park_message_draft(Vec::new(), "general".into(), "node a draft".into());
    app.reply_drafts =
        backend::park_reply_draft(Vec::new(), "general".into(), 1, "node a reply draft".into());

    let _ = app.__update(__DucktapeMessage::ConsoleOpened(iced::window::Id::unique()));

    assert_eq!(app.connected_rpc, "http://node-b");
    assert_eq!(app.password, "device-key-password");
    assert_eq!(app.selected_message_seq, 0);
    assert_eq!(app.message_action, MessageAction::Toolbar);
    assert!(app.message_edit_draft.is_empty());
    assert_eq!(app.active_thread_seq, 0);
    assert!(reply_composer(&app).is_empty());
    assert!(page_document_text(&app).is_empty());
    assert!(app.page_saved_text.is_empty());
    assert!(!app.block_comments_open);
    assert!(app.block_comments_target.is_empty());
    assert!(app.block_comment_draft.is_empty());
    assert!(app.message_draft.is_empty());
    assert!(composer(&app).is_empty());
    assert!(
        app.message_drafts.is_empty() && app.reply_drafts.is_empty(),
        "a draft parked on node A is not node B's to hand back"
    );
    assert!(app.page_search_draft.is_empty());
    assert_eq!(app.forge_list_phase, ForgePhase::Idle);
    assert!(app.forge_repo.is_empty());
    assert_eq!(app.forge_repo_phase, ForgePhase::Idle);
    assert_eq!(app.forge_item_number, 0);
    assert_eq!(app.forge_item_phase, ForgePhase::Idle);
    assert!(app.forge_review_draft.is_empty());
    // The tree lives in ForgeCodeBrowser component state now: a network
    // switch closes the console content and mounted-lifetime pruning drops
    // the instance — there is no app field left to reset.
    assert!(!app.huddle_joined);
    assert!(app.huddle_channel.is_empty());

    let _ = app.__update(__DucktapeMessage::Failed(backend::AppError {
        message: "offline".into(),
        committed: false,
    }));
    assert_eq!(app.connected_rpc, "http://node-b");
}

/// A CLAIM ON THE CARET HAS TO DIE WHEN THE CARET LEAVES. `composer_focus`
/// stands in for widget focus the app cannot read: the rich editor drops its
/// own focus on any press landing outside it (`rich_text_editor.rs` sets
/// `state.focus = None` in the else-arm of its press handler) and publishes
/// NOTHING when it does. So a discriminant stamped on entry is honest only for
/// as long as the set of handlers that retire it is complete — and #1005
/// shipped the claim with no retire at all, leaving Cmd+B marking the reply
/// draft while the caret sat in an inline edit box, on another tab, or in a
/// channel two switches away.
///
/// The enforcement is three MECHANICAL rules, not a remembered list, because
/// the hole was never in a route that existed — it was in the one nobody
/// thought to write. A handler carrying a `task widget focus` moves the caret
/// by hand; a handler writing `shell_tab` unmounts the composer under it; a
/// handler writing a literal `active_thread_seq = 0` tears the rail, and the
/// reply composer, out from under it. Any of the three must RETIRE — `"none"`
/// and nothing else, since a mover by definition took the caret somewhere that
/// is not a chat composer. The pinned set then catches the two the rules cannot
/// name, `open_thread_for`'s reset included: deleting that line used to fail
/// nothing.
///
/// Every rule here records the VALUE and not merely the assignment. A retire
/// flipped to `"message"` is a claim on a composer the caret is not in — the
/// exact defect — and a lint that only counted assignments called that green.
///
/// The rules cannot reach every rail close, and the last arm here is the one
/// they miss on purpose — which is what the chord's own `active_thread_seq > 0`
/// term is for. Both halves are driven, so neither the guard nor the retires
/// can be deleted quietly.
/// OPENING THE CHANNEL DRAWER IS NOT A REQUEST TO CLOSE THE THREAD.
/// `toggle_channel_settings` cleared `active_thread_seq`, the thread's messages
/// and `reply_editor` on the way in, so a part-typed reply was gone and closing
/// the drawer gave back an empty one. The main composer's draft survives the
/// same trip, which is the app's own standard — `reconnect` parks it
/// deliberately rather than letting a transition eat it.
///
/// A NOTE ON HOW THIS WAS FOUND, because the first account of it was wrong.
/// The live drive that "reproduced" it had clicked (1408, 164) — which with the
/// rail open is the RAIL's own `×`, not the channel header's `⋯` (that moves to
/// 1077 when the rail narrows the column). `close_thread` discarding a reply is
/// by design. The defect is real on the drawer's path and this test is what
/// proves it: restoring the teardown fails the first assertion below. The fix
/// was then driven correctly — drawer opened at 1077, Escape, reply intact.
///
/// The teardown was never what hid the rail: the screen draws it under
/// `if active_thread_seq > 0 && !channel_settings_open`. `close_thread` remains
/// the one route that discards a reply, because that one is a request to.
#[test]
fn the_channel_drawer_does_not_eat_a_reply_you_are_typing() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.shell_tab = ShellTab::Chat;
    app.active_channel = "general".into();
    app.active_thread_seq = 7;
    app.thread_messages = vec![message(7, "the root", false)];
    app.reply_editor = compose("half a reply");

    let _ = app.__update(__DucktapeMessage::ToggleChannelSettings);
    assert!(app.channel_settings_open, "the drawer opened");
    assert_eq!(
        reply_composer(&app),
        "half a reply",
        "the drawer does not discard a reply in progress"
    );
    assert_eq!(app.active_thread_seq, 7, "and it does not close the thread");
    assert_eq!(
        app.thread_messages.len(),
        1,
        "nor throw away the thread it was reading"
    );

    // Closing it gives the rail back exactly as it was.
    let _ = app.__update(__DucktapeMessage::ToggleChannelSettings);
    assert!(!app.channel_settings_open);
    assert_eq!(reply_composer(&app), "half a reply");
    assert_eq!(app.active_thread_seq, 7);

    // The screen is what hides the rail while the drawer is up — the handler
    // never needed to.
    assert!(
        SCREENS.contains("if active_thread_seq > 0 && !channel_settings_open"),
        "the rail is drawn under the drawer's own gate"
    );
    // And the claim on the caret still retires, because the drawer lays its own
    // inputs over a composer that stays mounted.
    let chat = inlined(include_str!("../ui/handlers/chat.ice"));
    let arm = chat
        .split_once("on toggle_channel_settings")
        .expect("the handler")
        .1
        .split_once("\non ")
        .expect("it ends")
        .0;
    // Statements, not prose: the comment above this handler NAMES the
    // teardown it no longer does, and a substring check over the arm would
    // read that as the teardown itself. Third time tonight.
    let statements: Vec<&str> = arm
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with("//"))
        .collect();
    assert!(statements.contains(&"composer_focus = ComposerFocus.unfocused"));
    assert!(
        !statements.contains(&"active_thread_seq = 0"),
        "the drawer must not tear the rail down"
    );
}

#[test]
fn a_channel_switch_freezes_the_unread_divider_while_a_same_channel_refresh_does_not() {
    let channel = |id: &str, head: i64| backend::ChatChannel {
        id: id.into(),
        name: id.into(),
        archived: false,
        members_only: false,
        huddle_count: 0,
        head_seq: head,
    };

    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.active_channel = "general".into();
    app.channels = vec![channel("general", 100), channel("random", 50)];
    // I last read #random at seq 30; it has since grown to head 50.
    app.channel_reads = vec![backend::ChannelRead {
        channel: "random".into(),
        seq: 30,
    }];

    // Switching INTO #random freezes the divider above the first unread
    // (>30) and marks #random read up to head so its sidebar badge clears.
    // The freeze must survive the REAL click path: `choose_channel` takes the
    // header and highlight optimistically, so by `chat_updated` current ==
    // next and the load-time freeze self-defers to the click-time one.
    let _ = app.__update(__DucktapeMessage::ChooseChannel("random".into()));
    assert_eq!(app.active_channel, "random");
    assert_eq!(app.unread_boundary, 30);
    assert!(app.messages.is_empty());
    app.loading = false;
    let mut switched = chat_data(
        "random",
        vec![
            message(31, "a", false),
            message(40, "b", false),
            message(50, "c", false),
        ],
    );
    switched.channels = vec![channel("general", 100), channel("random", 50)];
    switched.generation = app.chat_generation;
    let _ = app.__update(__DucktapeMessage::ChatUpdated(switched));
    assert_eq!(app.active_channel, "random");
    assert_eq!(app.unread_boundary, 30);
    assert_eq!(
        backend::first_unread_seq(app.messages.clone(), app.unread_boundary),
        31
    );
    assert!(
        !app.rooms
            .iter()
            .any(|row| row.channel.id == "random" && row.unread)
    );

    // A same-channel live delta that brings a NEW message must NOT move
    // the frozen boundary — the divider would jump as you read.
    let _ = app.__update(__DucktapeMessage::LiveUpdated(posted_delta(
        "random",
        message(60, "d", false),
    )));
    assert_eq!(app.active_channel, "random");
    assert_eq!(app.unread_boundary, 30);
    assert_eq!(app.messages.len(), 4);

    // Arriving at a caught-up channel shows no divider (boundary 0).
    app.channel_reads =
        backend::mark_channel_read(app.channel_reads.clone(), "general".into(), 100);
    let mut caught_up = chat_data("general", vec![message(100, "x", false)]);
    caught_up.channels = vec![channel("general", 100), channel("random", 60)];
    caught_up.generation = app.chat_generation;
    let _ = app.__update(__DucktapeMessage::ChatUpdated(caught_up));
    assert_eq!(app.active_channel, "general");
    assert_eq!(app.unread_boundary, 0);
}

/// THE LAST CLICK WINS. `choose_channel` used to open `return if loading`, and
/// `loading` covers the whole switch it starts — so the second and third clicks
/// of a fast A→B→C were discarded on the way out, with nothing on screen
/// admitting it. The clicks are taken now and the SUPERSEDED REPLY is dropped:
/// B answering after C must not drag the reader back into B.
#[test]
fn a_burst_of_channel_clicks_lands_on_the_last_one_and_drops_the_replies_it_passed() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.active_channel = "a".into();
    app.channels = vec![room("a", 10), room("b", 20), room("c", 30)];

    let _ = app.__update(__DucktapeMessage::ChooseChannel("b".into()));
    let for_b = app.chat_generation;
    // The click DURING the load is what used to vanish.
    assert!(app.loading);
    let _ = app.__update(__DucktapeMessage::ChooseChannel("c".into()));
    assert_eq!(app.active_channel, "c", "the second click moved the reader");
    assert_ne!(app.chat_generation, for_b);

    // One refreshed row is the whole channel list a window loader answers with.
    let mut late_b = chat_data("b", vec![message(20, "from b", false)]);
    late_b.channels = vec![room("b", 20)];
    late_b.generation = for_b;
    let _ = app.__update(__DucktapeMessage::ChatUpdated(late_b));
    assert_eq!(app.active_channel, "c", "b's reply must not take the pane");
    assert!(app.messages.is_empty());
    assert!(app.loading, "c is still in flight — the plate stays up");

    let mut for_c = chat_data("c", vec![message(30, "from c", false)]);
    for_c.channels = vec![room("c", 30)];
    for_c.generation = app.chat_generation;
    let _ = app.__update(__DucktapeMessage::ChatUpdated(for_c));
    assert_eq!(app.active_channel, "c");
    assert_eq!(app.messages.len(), 1);
    assert!(!app.loading);
}

/// A SWITCH REPLY FOLDS INTO THE SIDEBAR, IT DOES NOT REPLACE IT.
///
/// The window loader is handed the list the reader is already looking at and
/// answers with the one row it refreshed, so everything the live stream landed
/// DURING the round trip has to survive the reply: a peer's post in a THIRD
/// room and the unread badge it lit, and a channel someone created while she
/// waited. Nothing re-pages the list afterwards — `load_chat` is raised only
/// for `kind == LiveKind::Ready`, i.e. a websocket reconnect — so a revert here is not
/// a frame of staleness, it is permanent.
#[test]
fn a_switch_reply_keeps_what_the_live_stream_folded_while_it_was_in_flight() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.active_channel = "general".into();
    app.channels = vec![room("general", 10), room("random", 20), room("eng", 40)];
    app.channel_reads = backend::initial_channel_reads(app.channels.clone(), Vec::new());

    let _ = app.__update(__DucktapeMessage::ChooseChannel("random".into()));
    let switch = app.chat_generation;

    // Mid-RTT: a peer posts into a third room, and another creates a channel.
    let _ = app.__update(__DucktapeMessage::LiveUpdated(posted_delta(
        "eng",
        message(41, "from a peer", false),
    )));
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: LiveKind::Chat,
        status: "Live".into(),
        height: 1,
        chat: vec![backend::ChatDelta::ChannelCreated {
            channel: room("brand-new", 0),
        }],
        ..backend::LiveUpdate::default()
    }));
    assert!(
        app.rooms
            .iter()
            .any(|row| row.channel.id == "eng" && row.unread)
    );

    let mut landed = chat_data("random", vec![message(20, "from random", false)]);
    landed.channels = vec![room("random", 20)];
    landed.generation = switch;
    let _ = app.__update(__DucktapeMessage::ChatUpdated(landed));

    assert_eq!(
        app.channels
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        vec!["general", "random", "eng", "brand-new"],
        "the room created mid-switch is still in the sidebar"
    );
    assert_eq!(
        backend::channel_head_seq(app.channels.clone(), "eng".into()),
        41,
        "and the third room's head did not walk back to the pre-click snapshot"
    );
    assert!(
        app.rooms
            .iter()
            .any(|row| row.channel.id == "eng" && row.unread),
        "so its badge survives the switch it had nothing to do with"
    );
}

/// AND NEITHER DOES A RESYNC'S REPLY — same rule, same seam, wider blast.
///
/// `live_resync_load` is a checkpoint-gated multi-query read whose latency the
/// repo measures in seconds, so every delta the live stream folds inside its
/// round trip is the NEWER fact. A flat assignment walked a third room's
/// `head_seq` back to the snapshot — while `channel_reads` was NOT reverted with
/// it — so `head_seq > last_read` went false and the badge the reader never saw
/// blinked out, dark until that room got another message.
#[test]
fn a_resync_keeps_the_badge_the_live_stream_lit_while_it_was_in_flight() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.loading = false;
    app.active_channel = "general".into();
    app.channels = vec![room("general", 10), room("eng", 40)];
    app.channel_reads = backend::initial_channel_reads(app.channels.clone(), Vec::new());

    // mid-RTT: a peer posts into a third room, and another creates a channel
    let _ = app.__update(__DucktapeMessage::LiveUpdated(posted_delta(
        "eng",
        message(41, "from a peer", false),
    )));
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: LiveKind::Chat,
        status: "Live".into(),
        height: 1,
        chat: vec![backend::ChatDelta::ChannelCreated {
            channel: room("brand-new", 0),
        }],
        ..backend::LiveUpdate::default()
    }));
    assert!(
        app.rooms
            .iter()
            .any(|row| row.channel.id == "eng" && row.unread)
    );

    // the resync answers off a snapshot taken before either of them
    let mut landed = live_refresh(
        app.hydration_generation,
        "general",
        Vec::new(),
        "",
        Vec::new(),
    );
    landed.channels = vec![room("general", 10), room("eng", 40)];
    let _ = app.__update(__DucktapeMessage::LiveResynced(landed));

    assert_eq!(
        backend::channel_head_seq(app.channels.clone(), "eng".into()),
        41,
        "the third room's head does not walk back to the snapshot"
    );
    assert!(
        app.rooms
            .iter()
            .any(|row| row.channel.id == "eng" && row.unread),
        "so the badge it lit survives a resync it had nothing to do with"
    );
    assert!(
        app.channels.iter().any(|row| row.id == "brand-new"),
        "and the room created mid-resync is still in the sidebar"
    );
}

/// NOBODY READS A PANE THAT IS NOT MOUNTED.
///
/// The live feed is subscribed on `connected`, not on the tab, so an arrival in
/// the open room while the reader was in Settings or Files marked it read on the
/// spot: she came back to no divider and no way to tell the new rows from the
/// ones she had already read, and every OTHER room badged normally while that
/// one stayed dark. The rows still fold in — only the cursor waits for her.
#[test]
fn messages_that_arrive_off_tab_wait_for_the_reader_to_come_back() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_channel = "general".into();
    app.channels = vec![room("general", 10), room("eng", 40)];
    app.channel_reads = backend::initial_channel_reads(app.channels.clone(), Vec::new());
    app.messages = vec![message(10, "the last thing she read", false)];

    let _ = app.__update(__DucktapeMessage::SelectShellTab(ShellTab::Settings));
    let _ = app.__update(__DucktapeMessage::LiveUpdated(posted_delta(
        "general",
        message(11, "while she was away", false),
    )));

    assert_eq!(
        app.messages.len(),
        2,
        "the row folds in either way — it is on screen when she returns"
    );
    assert!(
        app.rooms
            .iter()
            .any(|row| row.channel.id == "general" && row.unread),
        "but the room she left open is unread like any other room"
    );

    // AND THEN SHE SAVES A FILE. A plane op resyncs the client — files, valset,
    // identity, agent and governance all land in `live_resynced`, carrying no
    // chat at all — and the read cursor used to move to the head on the way
    // past, retiring the badge and the divider for a room she has not looked at
    // since. It is traffic she generates herself, so the off-tab gate above
    // survived roughly one keystroke without this.
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
    assert_eq!(
        app.messages.len(),
        2,
        "a resync that carried no chat leaves the window alone"
    );
    assert!(
        app.rooms
            .iter()
            .any(|row| row.channel.id == "general" && row.unread),
        "and it does not catch her up on a room she is not on the tab for"
    );

    let _ = app.__update(__DucktapeMessage::SelectShellTab(ShellTab::Chat));
    assert_eq!(
        app.unread_marker_seq, 11,
        "coming back freezes the divider on what arrived while she was gone"
    );
    assert!(
        !app.rooms
            .iter()
            .any(|row| row.channel.id == "general" && row.unread),
        "and only then is she caught up"
    );

    // a tab round trip with nothing new must not throw the divider away
    let _ = app.__update(__DucktapeMessage::SelectShellTab(ShellTab::Files));
    let _ = app.__update(__DucktapeMessage::SelectShellTab(ShellTab::Chat));
    assert_eq!(app.unread_marker_seq, 11);
}

/// A SUPERSEDED SWITCH'S FAILURE STAYS WITH IT. Nothing serializes the room
/// pickers any more, so B's error can arrive after the reader has clicked on to
/// C — and ungated it would clear `loading` under C (swapping C's plate for "No
/// messages yet") and put B's message in the banner until C lands.
#[test]
fn a_failed_switch_the_reader_clicked_past_does_not_land_on_the_room_she_is_in() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.active_channel = "a".into();
    app.channels = vec![room("a", 10), room("b", 20), room("c", 30)];

    let _ = app.__update(__DucktapeMessage::ChooseChannel("b".into()));
    let for_b = app.chat_generation;
    let _ = app.__update(__DucktapeMessage::ChooseChannel("c".into()));

    let _ = app.__update(__DucktapeMessage::ChatLoadFailed(backend::HydrationError {
        generation: for_b,
        message: "b is unreachable".into(),
    }));
    assert!(app.loading, "c is still in flight — the plate stays up");
    assert!(app.error.is_empty(), "and b's failure is not c's");

    let for_c = app.chat_generation;
    let _ = app.__update(__DucktapeMessage::ChatLoadFailed(backend::HydrationError {
        generation: for_c,
        message: "c is unreachable too".into(),
    }));
    assert!(!app.loading);
    assert_eq!(app.error, "c is unreachable too");
}

/// A ROOM SWITCH DROPS THE OLD WINDOW BEFORE IT STARTS THE SELECTED ROOM'S
/// ROOT-WINDOW READ. Keeping several rich windows in state made the synchronous
/// click cost proportional to every retained row through the by-value UI ABI.
#[test]
fn switching_channels_paints_an_empty_loading_state_until_the_root_window_lands() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.settings_user_key = "me".into();
    app.active_channel = "a".into();
    app.channels = vec![room("a", 10), room("b", 20)];
    app.messages = vec![message(9, "older", false), message(10, "newest", false)];
    app.channel_members = vec![backend::ChatMember {
        key: "me".into(),
        label: "me".into(),
    }];

    let _ = app.__update(__DucktapeMessage::ChooseChannel("b".into()));
    assert_eq!(app.active_channel, "b");
    assert!(
        app.messages.is_empty(),
        "the old room's rows leave immediately"
    );
    assert!(app.channel_members.is_empty(), "so does its member roll");
    assert!(
        !app.has_older_history,
        "there is no page cursor before the load"
    );
    assert!(app.loading, "the selected room is fetching one root window");
    assert!(app.post_refusal.is_empty());
}

/// A DM CLICK LANDS THE WHOLE ROOM, NOT JUST THE FACE.
///
/// `choose_dm` used to move `active_dm_peer` and nothing else about the room,
/// so for the several blocks `open_dm` takes — a channel create plus two
/// membership seats on a first open — the peer's name sat beside the ARCHIVED
/// badge, the "· N added" count and the composer refusal of the room she left.
/// The id is derivable here (`dm_channel_id` is the same deterministic hash
/// `open_dm` resolves), so the empty loading state can still take the right room
/// identity on the click.
#[test]
fn a_dm_click_takes_the_room_with_it_instead_of_wearing_the_last_ones_badges() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.settings_user_key = "me".into();
    app.active_channel = "locked".into();
    app.active_channel_name = "locked".into();
    app.active_channel_archived = true;
    app.active_channel_members_only = true;
    app.channel_members = vec![backend::ChatMember {
        key: "someone-else".into(),
        label: "Someone else".into(),
    }];
    app.post_refusal = "channel_archived".into();
    app.messages = vec![message(10, "in the room she leaves", false)];
    app.dm_peers = vec![backend::DmPeer {
        key: "peer".into(),
        name: "Peer".into(),
        initials: "P".into(),
        is_agent: false,
        channel_id: backend::dm_channel_id("me".into(), "peer".into()),
    }];

    let _ = app.__update(__DucktapeMessage::ChooseDm("peer".into()));
    let dm = backend::dm_channel_id("me".into(), "peer".into());
    assert_eq!(app.active_channel, dm, "the DM's own room, on the click");
    assert_eq!(app.active_dm_peer, "peer");
    assert!(!app.active_channel_archived, "not the left room's badge");
    assert!(!app.active_channel_members_only);
    assert!(app.channel_members.is_empty(), "nor its member count");
    assert!(app.post_refusal.is_empty(), "nor its composer refusal");
    assert!(!app.history_view, "a DM open is a live tail");
    assert!(app.loading, "this peer has never been read");

    // A re-open follows the same no-window-cache path as every channel switch.
    let mut landed = chat_data(&dm, vec![message(30, "from the peer", false)]);
    landed.generation = app.chat_generation;
    let _ = app.__update(__DucktapeMessage::ChatUpdated(landed));
    let _ = app.__update(__DucktapeMessage::ChooseChannel("locked".into()));
    let _ = app.__update(__DucktapeMessage::ChooseDm("peer".into()));
    assert!(
        app.messages.is_empty(),
        "the stale DM window is not restored"
    );
    assert!(app.loading, "the DM's root window is fetched again");
}

/// A SEARCH HIT PAINTS THE ROOM IT IS JUMPING TO, NOT THE ROOM IT LEFT.
///
/// Every landing field used to move only in `chat_hit_loaded`, so a hit that
/// lives in another room kept that room's header, rows and sidebar highlight
/// for the whole walk — the one navigation whose entire purpose is to jump
/// somewhere else, and the only one still showing the "did my click land?" void
/// #1059 removed from the pickers. A hit is a history window, and an empty
/// timeline under the skeleton is honest until that window arrives.
#[test]
fn opening_a_search_hit_moves_the_room_on_the_click() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.settings_user_key = "me".into();
    app.active_channel = "general".into();
    app.active_channel_name = "general".into();
    app.active_channel_archived = true;
    app.channels = vec![room("general", 10), room("design", 40)];
    app.messages = vec![message(10, "in general", false)];
    app.channel_members = vec![backend::ChatMember {
        key: "me".into(),
        label: "me".into(),
    }];

    let _ = app.__update(__DucktapeMessage::OpenChatSearchHit("design".into(), 7, 7));
    assert_eq!(
        app.active_channel, "design",
        "the sidebar moves on the click"
    );
    assert_eq!(app.active_channel_name, "design", "and so does the header");
    assert!(!app.active_channel_archived, "not general's badge");
    assert!(app.channel_members.is_empty(), "nor general's roll");
    assert!(app.post_refusal.is_empty());
    assert!(app.messages.is_empty(), "general's rows leave with general");
    assert!(
        app.loading,
        "so the skeleton draws for the room being entered"
    );
    assert!(app.history_view, "a hit is a window around one old message");
}

#[test]
fn unread_indicators_are_wired_client_local_only() {
    // Sidebar badge: ChannelButton takes an `unread` flag and paints the
    // brand treatment + dot when set.
    let components = inlined(include_str!("../ui/components/chat.ice"));
    assert!(components.contains(
        "component ChannelButton(channel:ChatChannel, selected:bool, unread:bool, disabled:bool)"
    ));
    assert!(components.contains("if unread\n                box w=7.0 h=7.0 bg=brand r=3.5"));
    // The name rides a `box w=fill clip=true`: `wrap=none` text lays out at its
    // INTRINSIC width whatever box it is given, so an unclipped long channel
    // name inflated the whole row past the 236px pane and the pane's own clip
    // sliced the row plate square through its rounded corner.
    // Unread is WEIGHT, not just ink — the same signal `ChannelButton` gives
    // an unread row over a read one (`font=medium` there, `font=display`
    // here), the conventional stronger signal.
    assert!(components.contains(
        "if unread\n                box w=fill clip=true\n                  text channel.name size=13.0 wrap=none font=display @text-fg"
    ));

    let screen = inlined(include_str!("../ui/screens/chat.ice"));
    // The prepared row owns the scalar. No list-taking extern runs in either
    // sidebar loop.
    assert!(screen.contains(
        "ChannelButton channel=room.channel selected=(room.channel.id == active_channel) unread=room.unread"
    ));
    // In-channel divider anchored on the first message past the frozen
    // boundary. The seq is a STATE FIELD recomputed where messages or the
    // boundary change — `first_unread_seq(messages, …)` in the view sat
    // inside `for message in messages`, and the extern's by-value ABI deep-
    // cloned the whole timeline once per row per frame.
    assert!(screen.contains("if unread_boundary > 0 && message.seq == unread_marker_seq"));
    assert!(!screen.contains("first_unread_seq("));
    // The eyebrow spelling: FIELD_LABEL scale (10.0, mono semibold caps) —
    // every other structural label in the console reads this way, and a
    // 12.5px sentence-case run inside the message column read as a MESSAGE
    // at first glance.
    assert!(screen.contains("text \"NEW\" size=10.0 wrap=none font=code_semibold @text-brand"));

    // Freeze happens on a real channel change; connect seeds caught-up.
    let lifecycle = inlined(include_str!("../ui/handlers/lifecycle.ice"));
    assert!(
        lifecycle.contains("channel_reads = initial_channel_reads(next.channels, channel_reads)")
    );
    // navigation loads freeze on the real channel change (chat.ice);
    // the resync path freezes against the possibly-unchanged channel.
    let chat = inlined(include_str!("../ui/handlers/chat.ice"));
    assert!(chat.contains(
        "unread_boundary = frozen_unread_boundary(channel_reads, channels, active_channel, next.active_channel, unread_boundary)"
    ));
    assert!(chat.contains(
        "channel_reads = mark_channel_read(channel_reads, next.active_channel, channel_head_seq(channels, next.active_channel))"
    ));
    assert!(lifecycle.contains(
        "unread_boundary = frozen_unread_boundary(channel_reads, channels, active_channel, active_channel, unread_boundary)"
    ));
    // AND NEITHER LANDING MARKS A ROOM READ UNNAMED. Every read-cursor write
    // outside a deliberate channel entry goes through a gated channel name, so
    // that the gate cannot be dropped without this failing: a plane-only resync
    // (every files/agent/identity op) and an off-tab arrival both reach these
    // lines, and `mark_channel_read` refuses an empty channel.
    for gated in [
        "channel_reads = mark_channel_read(channel_reads, resync_tail_channel, channel_head_seq(channels, resync_tail_channel))",
        "channel_reads = mark_channel_read(channel_reads, chat_tab_channel, channel_head_seq(channels, chat_tab_channel))",
    ] {
        assert!(lifecycle.contains(gated), "{gated}");
    }
    for gate in [
        "let resync_tail_channel = keep_str(!history_view && shell_tab == ShellTab.chat, active_channel, \"\")",
        "let chat_tab_channel = keep_str(shell_tab == ShellTab.chat && !history_view, active_channel, \"\")",
    ] {
        assert!(lifecycle.contains(gate), "{gate}");
    }
    let live = inlined(include_str!("../backend/live.rs"));
    assert!(lifecycle.contains("history_view, shell_tab == ShellTab.chat, unread_boundary"));
    assert!(live.contains("let reads_live_tail = !history_view && chat_visible"));
    assert!(live.contains("if reads_live_tail"));

    // Client-local only: no wire read-cursor leaked into the module surface.
    let backend_ice = inlined(include_str!("../ui/extern/backend.ice"));
    assert!(!backend_ice.contains("read_cursor"));
    assert!(!backend_ice.contains("mark_read(rpc"));
}
