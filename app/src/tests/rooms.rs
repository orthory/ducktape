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
            "chat_updated",
            "choose_channel",
            "choose_dm",
            "live_resynced",
            "network_entered",
            "open_chat_search_hit",
            "reconnect",
            "workspace_connected",
        ],
        "a handler started or stopped moving `active_channel`: decide whether it \
         abandons an in-flight history page, then update this list"
    );

    // And the launches genuinely carry the landing seq — a mover list alone
    // would pass with every reset deleted, and a stale `chat_land_seq` opens
    // the next room in the middle of its scrollback.
    for launch in [
        "choose_channel",
        "choose_dm",
        "open_chat_search_hit",
        "reconnect",
        "network_entered",
    ] {
        let body = HANDLERS
            .split(&format!("\non {launch}"))
            .nth(1)
            .unwrap_or_else(|| panic!("{launch} is a handler"))
            .split("\non ")
            .next()
            .expect("handler body");
        assert!(
            body.lines()
                .any(|line| line.trim_start().starts_with("chat_land_seq = ")),
            "{launch} moves the reader and must say where the view opens"
        );
    }

    // THE COMPOSER IS PER-ROOM, AND NOTHING HERE CARRIES IT ANY MORE. It used
    // to be app state parked and restored by every handler on the list above —
    // a whole handler class, policed from here by an ordering rule (park while
    // `active_channel` still names the room being LEFT, restore once it names
    // the room being ENTERED). The class is gone: each composer is a retained
    // component instance keyed by its own room (ducktape-ui#697), so a room
    // switch cannot touch a draft, and this lint pins the two facts that
    // replaced it — see `the_composers_are_out_of_reach_of_every_handler`.

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

/// THE COMPOSERS ARE OUT OF REACH, AND THAT IS THE WHOLE SAFETY ARGUMENT
/// (ducktape-ui#697). Two facts replace the retired park/restore class:
///
/// 1. NO handler can name a composer, because the app holds none. The park
///    store, the two `editor` states, the focus discriminant and the two
///    failed-send stashes are gone, so there is nothing for a room switch to
///    carry, drop, or hand to the wrong room — the bug class the ordering lint
///    policed cannot be written. The stash was the last of them to descend
///    (ducktape-ui#698): it is instance state reached by a slice keyed to the
///    room the failure names, so a refusal in #private-ops can no longer raise
///    its plate over whatever room the reader has moved to.
/// 2. EVERY composer key carries the ENDPOINT. A channel id is a user-chosen
///    string: two networks' `#general` are two rooms, and a key without the
///    endpoint would hand one network's words to the other — exactly what the
///    old store had to be emptied by hand on every network switch to avoid.
#[test]
fn the_composers_are_out_of_reach_of_every_handler() {
    const HANDLERS: &str = concat!(
        include_str!("../ui/handlers/chat.ice"),
        include_str!("../ui/handlers/lifecycle.ice"),
        include_str!("../ui/handlers/onboarding.ice"),
        include_str!("../ui/handlers/overlays.ice"),
        include_str!("../ui/handlers/huddle.ice"),
        include_str!("../ui/handlers/pages.ice"),
    );
    const STATE: &str = include_str!("../ui/state/chat.ice");

    for retired in [
        "message_editor",
        "reply_editor",
        "message_drafts",
        "reply_drafts",
        "composer_focus",
        "park_message_draft",
        "park_reply_draft",
        "parked_message_draft",
        "parked_reply_draft",
        "composer_mark_shortcut",
        "failed_message_draft",
        "failed_reply_draft",
    ] {
        // An ASSIGNMENT or a CALL, not a mention: the prose above may name
        // what was retired, and should.
        assert!(
            !HANDLERS.lines().any(|line| {
                let line = line.trim_start();
                !line.starts_with("//") && line.contains(retired)
            }),
            "`{retired}` is back in a handler — the composers are component              instances, and app state that shadows one can only disagree with it"
        );
        assert!(
            !STATE.lines().any(|line| {
                let line = line.trim_start();
                !line.starts_with("//") && line.starts_with(retired)
            }),
            "`{retired}` is back in app state — see above"
        );
    }

    const SCREEN: &str = include_str!("../../../crates/views/chat/src/ui/chat.ice");
    for (mount, key) in [
        ("#composer", "extern chat_composer(composer_scope(endpoint,"),
        (
            "#reply_composer",
            "extern chat_composer(thread_scope(endpoint,",
        ),
    ] {
        let line = SCREEN
            .lines()
            .map(str::trim)
            .find(|line| line.ends_with(mount))
            .unwrap_or_else(|| panic!("`{mount}` is mounted"));
        assert!(
            line.contains(key),
            "`{mount}` must key on `{key}` — a key without the endpoint gives              two networks' rooms of the same name ONE composer, and the words              typed on one node are handed back on another"
        );
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
    app.account_number = me.into();
    // THE DIRECTORY IS WHAT SAYS A ROOM IS A DM — `load_dm_peers` stamps each
    // row's `channel_id` from the account number IT resolved, and all three DM
    // decisions read that one field. A fixture that only sets `account_number`
    // is a console whose account load has not landed, which is exactly the state
    // that used to scatter DMs into the room list.
    app.dm_peers = vec![backend::DmPeer {
        key: peer.into(),
        name: "Peer".into(),
        initials: "P".into(),
        is_agent: false,
        channel_id: dm.clone(),
    }];
    app.active_dm_peer = peer.into();
    app.active_channel = dm.clone();

    // a search hit jumps to an ordinary room…
    let _ = app.__update(__DucktapeMessage::ChatUpdated(chat_data("general")));
    assert!(
        app.active_dm_peer.is_empty(),
        "the peer does not follow the reader into #general"
    );

    // …and a landing inside the DM itself keeps him
    app.active_dm_peer = peer.into();
    let _ = app.__update(__DucktapeMessage::ChatUpdated(chat_data(&dm)));
    assert_eq!(app.active_dm_peer, peer, "this room IS his DM");

    // the resync is the landing with no launch behind it — it moves the room
    // on its own, which is how the peer used to survive every other route
    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        app.hydration_generation,
        "general",
    )));
    assert!(app.active_dm_peer.is_empty());

    // BUT A RESYNC THAT MOVED NO ROOM DERIVES NOTHING. `choose_dm` names the
    // peer optimistically and leaves `active_channel` on the room being left
    // for the several blocks `open_dm` takes to answer; a plane-only resync
    // landing in that window would otherwise derive the peer against the OLD
    // room and blank him, and `chat_updated` then derives "" from "" — the DM
    // opens under a `#` for good.
    app.active_dm_peer = peer.into();
    app.active_channel = "general".into();
    let _ = app.__update(__DucktapeMessage::LiveResynced(backend::LiveRefresh {
        chat_loaded: false,
        ..live_refresh(app.hydration_generation, "general")
    }));
    assert_eq!(
        app.active_dm_peer, peer,
        "the room did not move, so nothing about it was re-read"
    );

    // NOR DOES A CHAT-CARRYING ONE INSIDE THAT SAME WINDOW. `live_resync_load`
    // is launched with today's `active_channel`, so a `ready`/`Lagged{chat}`
    // resync lands `chat_loaded` on the room being LEFT — deriving against it
    // blanks the peer just as permanently as the plane-only case above.
    app.active_dm_peer = peer.into();
    app.active_channel = "general".into();
    app.loading = true;
    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        app.hydration_generation,
        "general",
    )));
    assert_eq!(
        app.active_dm_peer, peer,
        "a landing is in flight — it answers for the peer, this resync does not"
    );
    app.loading = false;

    // A DIRECTORY THAT RESOLVED NO ACCOUNT OF OURS carries no channel id, so it
    // claims no room — the same answer `chat_sidebar_rooms` gives, from the same
    // field, which is the point of there being only one derivation.
    app.dm_peers[0].channel_id = String::new();
    app.active_dm_peer = peer.into();
    let _ = app.__update(__DucktapeMessage::ChatUpdated(chat_data(&dm)));
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
    let screen = inlined(include_str!("../../../crates/views/chat/src/ui/chat.ice"));
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

// Entering a (possibly different) network through the doors' landing clears
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
    app.chat_edit_seq = 1;
    app.chat_edit_rev = 2;
    app.chat_land_seq = 9;
    // The page a `duck://page/…` address asked for is the one pages fact the
    // app still holds — and it named the network being left.
    app.page_route = "node-a-page".into();
    app.forge_link = "duck://forge/core/pull/1".into();
    app.forge_note_pending = "op-a".into();
    app.huddle_joined = true;
    app.huddle_channel = "chan-a".into();
    // AND A COMPOSER WITH WORDS IN IT, typed against node A. A channel id is a
    // user-chosen string, so both networks can hold a `#general` — the key
    // carries the ENDPOINT for exactly that reason (ducktape-ui#697), and the
    // assertions below drive both halves of the promise.
    let node_a_composer = composer_scope(&app);
    type_into(&node_a_composer, "node a draft");
    assert_eq!(composer_text(&node_a_composer), "node a draft");

    let _ = app.__update(__DucktapeMessage::NetworkEntered);

    assert_eq!(app.connected_rpc, "http://node-b");
    assert_eq!(app.password, "device-key-password");
    assert_eq!(app.chat_edit_seq, 0);
    assert_eq!(app.chat_edit_rev, 0);
    assert_eq!(app.chat_land_seq, 0);
    assert!(app.page_route.is_empty());
    // NODE B'S ROOM IS NODE B'S. Same channel id, other endpoint, other
    // instance — and node A's words are still under node A's key, which is
    // the half a `message_drafts = []` clear used to get wrong by throwing
    // them away instead.
    let node_b_composer = composer_scope(&app);
    assert_ne!(
        node_b_composer, node_a_composer,
        "the endpoint is in the key, so #general on node B is not #general on \
         node A"
    );
    assert!(
        composer_text(&node_b_composer).is_empty(),
        "a draft typed on node A is not node B's to hand back"
    );
    assert_eq!(
        composer_text(&node_a_composer),
        "node a draft",
        "and it is still node A's, waiting where it was typed"
    );
    // The forge screen is the Forge VIEW's: what the app clears is the link
    // it last routed there, which named node A, and the note it had in flight.
    assert!(app.forge_link.is_empty());
    assert!(app.forge_note_pending.is_empty());
    assert!(!app.huddle_joined);
    assert!(app.huddle_channel.is_empty());

    let _ = app.__update(__DucktapeMessage::ChatLoadFailed(backend::HydrationError {
        generation: app.chat_generation,
        message: "offline".into(),
    }));
    assert_eq!(app.connected_rpc, "http://node-b");
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
    app.loading = false;
    let mut switched = chat_data("random");
    switched.channels = vec![channel("general", 100), channel("random", 50)];
    switched.generation = app.chat_generation;
    let _ = app.__update(__DucktapeMessage::ChatUpdated(switched));
    assert_eq!(app.active_channel, "random");
    // the boundary is what the view divides on; where the divider lands is
    // its own fold over the rows it read
    assert_eq!(app.unread_boundary, 30);
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

    // Arriving at a caught-up channel shows no divider (boundary 0).
    app.channel_reads =
        backend::mark_channel_read(app.channel_reads.clone(), "general".into(), 100);
    let mut caught_up = chat_data("general");
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
    let mut late_b = chat_data("b");
    late_b.channels = vec![room("b", 20)];
    late_b.generation = for_b;
    let _ = app.__update(__DucktapeMessage::ChatUpdated(late_b));
    assert_eq!(app.active_channel, "c", "b's reply must not take the pane");
    assert!(app.loading, "c is still in flight — the plate stays up");

    let mut for_c = chat_data("c");
    for_c.channels = vec![room("c", 30)];
    for_c.generation = app.chat_generation;
    let _ = app.__update(__DucktapeMessage::ChatUpdated(for_c));
    assert_eq!(app.active_channel, "c");
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

    let mut landed = chat_data("random");
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
    let mut landed = live_refresh(app.hydration_generation, "general");
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

    let _ = app.__update(__DucktapeMessage::SelectShellTab(ShellTab::Settings));
    let _ = app.__update(__DucktapeMessage::LiveUpdated(posted_delta(
        "general",
        message(11, "while she was away", false),
    )));

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
        ..live_refresh(app.hydration_generation, "general")
    };
    let _ = app.__update(__DucktapeMessage::LiveResynced(plane_only));
    assert!(
        app.rooms
            .iter()
            .any(|row| row.channel.id == "general" && row.unread),
        "and it does not catch her up on a room she is not on the tab for"
    );

    let _ = app.__update(__DucktapeMessage::SelectShellTab(ShellTab::Chat));
    assert_eq!(
        app.unread_boundary, 10,
        "coming back freezes the boundary on what she had already read, so the \
         view divides above what arrived while she was gone"
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
    assert_eq!(app.unread_boundary, 10);
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
    app.channel_members = vec![backend::ChatMember {
        key: "me".into(),
        label: "me".into(),
    }];

    let _ = app.__update(__DucktapeMessage::ChooseChannel("b".into()));
    assert_eq!(app.active_channel, "b");
    // The ROWS are the view's — it re-reads them off the index the moment its
    // room key moves. What the app drops on the click is the room facts that
    // would otherwise wear the last room's badges.
    assert!(app.channel_members.is_empty(), "its member roll leaves");
    assert!(app.loading, "the selected room is fetching its record"); 
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
    app.account_number = "me".into();
    app.active_channel = "locked".into();
    app.active_channel_name = "locked".into();
    app.active_channel_archived = true;
    app.active_channel_members_only = true;
    app.channel_members = vec![backend::ChatMember {
        key: "someone-else".into(),
        label: "Someone else".into(),
    }];
    app.post_refusal = "channel_archived".into();
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
    let mut landed = chat_data(&dm);
    landed.generation = app.chat_generation;
    let _ = app.__update(__DucktapeMessage::ChatUpdated(landed));
    let _ = app.__update(__DucktapeMessage::ChooseChannel("locked".into()));
    let _ = app.__update(__DucktapeMessage::ChooseDm("peer".into()));
    assert!(app.loading, "the DM's record is fetched again");
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
    app.channel_members = vec![backend::ChatMember {
        key: "me".into(),
        label: "me".into(),
    }];

    let _ = app.__update(__DucktapeMessage::OpenChatSearchHit("design".into(), 7));
    assert_eq!(
        app.active_channel, "design",
        "the sidebar moves on the click"
    );
    assert_eq!(app.active_channel_name, "design", "and so does the header");
    assert!(!app.active_channel_archived, "not general's badge");
    assert!(app.channel_members.is_empty(), "nor general's roll");
    assert!(app.post_refusal.is_empty());
    assert_eq!(
        app.chat_land_seq, 7,
        "and the seq the hit named is what the view opens its window around"
    );
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
    let components = inlined(include_str!(
        "../../../crates/views/chat/src/ui/components.ice"
    ));
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

    let screen = inlined(include_str!("../../../crates/views/chat/src/ui/chat.ice"));
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
    assert!(
        lifecycle.contains("history_view, shell_tab == ShellTab.chat, active_channel_name")
    );
    assert!(live.contains("let reads_live_tail = !history_view && chat_visible"));
    assert!(live.contains("if reads_live_tail"));

    // Client-local only: no wire read-cursor leaked into the module surface.
    let backend_ice = inlined(include_str!("../ui/extern/backend.ice"));
    assert!(!backend_ice.contains("read_cursor"));
    assert!(!backend_ice.contains("mark_read(rpc"));
}

/// A DM RECORD IS A NETWORK-VISIBLE CHANNEL ROW, so a DM between two OTHER
/// people arrives in this device's channel list like any room — and, being in
/// no peer's `channel_id`, it used to fall through the directory exclusion and
/// draw under CHANNELS with a `#` glyph and that person's name, right beside
/// their real DIRECT row. It reads as "clicking a DM created a channel".
///
/// Mine belong in DIRECT and theirs belong nowhere on my screen, so no derived
/// two-party id is ever a CHANNELS row. A channel a person deliberately NAMED
/// `dm-standup` is not one of those and stays listed — the test pins the shape
/// rule, not a `dm-` prefix.
#[test]
fn another_members_dm_is_not_a_channel_of_mine() {
    let channel = |id: &str| backend::ChatChannel {
        id: id.into(),
        name: id.into(),
        archived: false,
        members_only: false,
        huddle_count: 0,
        head_seq: 4,
    };
    // the live network's shape: me(1) ↔ orthory(2), me(1) ↔ orthory-ops(3),
    // and orthory(2) ↔ orthory-ops(3) — the last one none of my business.
    let mine = backend::dm_channel_id("1".into(), "2".into());
    let also_mine = backend::dm_channel_id("1".into(), "3".into());
    let theirs = backend::dm_channel_id("2".into(), "3".into());

    let rooms = backend::chat_sidebar_rooms(
        vec![
            channel("general"),
            channel("dm-standup"),
            channel(&mine),
            channel(&also_mine),
            channel(&theirs),
        ],
        vec![
            backend::DmPeer {
                key: "2".into(),
                name: "orthory".into(),
                initials: "O".into(),
                is_agent: false,
                channel_id: mine.clone(),
            },
            backend::DmPeer {
                key: "3".into(),
                name: "orthory-ops".into(),
                initials: "O".into(),
                is_agent: false,
                channel_id: also_mine.clone(),
            },
        ],
        Vec::new(),
    );

    let listed: Vec<&str> = rooms.iter().map(|row| row.channel.id.as_str()).collect();
    assert_eq!(
        listed,
        ["general", "dm-standup"],
        "no derived DM id is a CHANNELS row — not mine, and not theirs"
    );
}

/// THE LIVE-RUN READING IS REFUSED, NEVER FOLDED. Its rows are the node's whole
/// pending set, and the reading is stamped with the connection it was taken over
/// — so a reading that crossed with a reconnect has to be DROPPED. Folding it in
/// would assign its emptiness and blank the cards the current connection just
/// installed, until the next two-second poll put them back.
///
/// Pinned as statements, not as a substring: the comment above that handler
/// NAMES the blanking it refuses to do, and a `contains` over the arm would read
/// the prose as the code.
#[test]
fn a_stale_live_run_reading_is_dropped_rather_than_folded() {
    let chat = inlined(include_str!("../ui/handlers/chat.ice"));
    let arm = chat
        .split_once("on live_agents_event(next)")
        .expect("the handler")
        .1
        .split_once("\non ")
        .expect("it ends")
        .0;
    let statements: Vec<&str> = arm
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with("//") && !line.is_empty())
        .collect();
    assert_eq!(
        statements,
        [
            "return if live_agents_stale(next, connected_rpc, network_chain_id, connect_generation, signer_key)",
            "live_agents = next.rows",
        ],
        "the guard RETURNS, and it stands before the only assignment"
    );

    // ALL FOUR IDENTITIES REACH THE LANE, or the guard above cannot ask. The
    // endpoint is the weakest of them: a workspace switch brings the node back
    // on the same loopback port, which is the same trap `live_resynced` names
    // `chain_left_behind`. The seat is the one that does not move with the
    // connection at all — see the seams pinned below.
    let lifecycle = inlined(include_str!("../ui/handlers/lifecycle.ice"));
    let lane: Vec<&str> = lifecycle
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("run chat_live_agents("))
        .collect();
    assert_eq!(
        lane,
        [
            "run chat_live_agents(connected_rpc, network_chain_id, connect_generation, signer_key) when connected -> live_agents_event _"
        ],
        "one lane, for the node, keyed on the whole connection AND the seat"
    );

    // EVERY SEAM THAT MOVES THE SEAT WRITES `signer_key`, or the lane is keyed
    // on a lie. A Settings unlock and lock change what this device may read and
    // bump NO `connect_generation` (`node.ice` SettingsIntent.unlock/.lock), so
    // a missed seam here is a device that never recovers from a lock — or one
    // that keeps the previous key's output after a switch.
    let node = inlined(include_str!("../ui/handlers/node.ice"));
    let onboarding = inlined(include_str!("../ui/handlers/onboarding.ice"));
    for (file, source, seam) in [
        // seated: the handler that fires when a key is opened carries its pubkey
        ("node.ice", &node, "on settings_unlocked(pubkey)"),
        ("onboarding.ice", &onboarding, "on key_unlocked(pubkey)"),
        ("onboarding.ice", &onboarding, "on phrase_confirmed(pubkey)"),
        ("onboarding.ice", &onboarding, "on key_restored(pubkey)"),
    ] {
        let arm = source
            .split_once(seam)
            .unwrap_or_else(|| panic!("{file} must still carry `{seam}`"))
            .1
            .split_once("\non ")
            .expect("the handler ends")
            .0;
        assert!(
            arm.contains("signer_key = pubkey"),
            "{file}: `{seam}` seats a key without naming it — the live agent \
             lane keys on `signer_key`"
        );
        // AND DROPS THE ROWS IN THE SAME ARM. Re-keying the lane only fences
        // what arrives next, and the new lane's first notice waits on a `runs`
        // query — so a seam that moves the seat without clearing leaves the
        // previous key's private output on screen for as long as that query
        // takes, or forever if it never answers.
        assert!(
            arm.contains("live_agents = []"),
            "{file}: `{seam}` moves the seat and leaves the previous key's rows \
             on screen until a fresh notice arrives"
        );
    }
    // and the teardown clears it, in the arm that retires the signer.
    let lock = node
        .split_once("SettingsIntent.lock")
        .expect("the Lock intent")
        .1
        .split_once("    SettingsIntent.")
        .expect("the next intent")
        .0;
    assert!(
        lock.contains("signer_key = \"\"")
            && lock.contains("live_agents = []")
            && lock.contains("lock_signer()"),
        "node.ice: the arm that retires the signer must clear `signer_key` and \
         the rows it could read: {lock}"
    );
    assert!(
        onboarding.contains("signer_key = \"\""),
        "onboarding.ice: leaving a network must clear the seat it was taken in"
    );

    // AND NO ROOM OWES IT ANYTHING. Eight handlers move `active_channel`; the
    // room is chosen in `encode_chat_props`, so none of them may carry a
    // per-room launch or teardown for this lane.
    assert!(
        !chat.contains("lane=live_agents"),
        "a per-room lane is back, and five of the eight movers will forget it"
    );
}
