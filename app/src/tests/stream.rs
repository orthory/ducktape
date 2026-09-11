use super::*;

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
    // (mirror, the sources whose movement invalidates it). THE DM DIRECTORY
    // decides which channels are DMs — `load_dm_peers` stamps each row's
    // `channel_id` from the account number it resolved itself, and `account_number`
    // is Settings' reading alone; THIS DEVICE'S KEY decides whether it is seated
    // in a members-only room.
    const MIRRORS: [(&str, &[&str]); 6] = [
        ("rooms", &["channels", "dm_peers", "channel_reads"]),
        ("dm_rows", &["channels", "dm_peers", "channel_reads"]),
        // The card's rows are its SCOPE's threads with their anchors resolved,
        // so narrowing and widening invalidate the mirror exactly as a new
        // thread list or a moved page does.
        (
            "block_comment_rows",
            &[
                "blocks",
                "block_comment_threads",
                "active_page",
                "inline_comment_target",
            ],
        ),
        (
            "huddle_rows",
            &["huddle_roster", "call_peers", "call_muted"],
        ),
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
    let _ = app.__update(__DucktapeMessage::OpenChatSearchHit("general".into(), 7));
    assert!(app.history_view);
    assert_eq!(app.chat_land_seq, 7);

    // …and the Jump-to-latest press — which the view emits as `choose_channel`
    // on the room it is already in — leaves it
    let _ = app.__update(__DucktapeMessage::ChooseChannel("general".into()));
    assert!(!app.history_view);
    assert_eq!(app.chat_land_seq, 0, "and the view opens back on the tail");

    // The way back is a float over the timeline's bottom edge now, not a
    // button inside an amber band at the top of the column — and it is shown
    // for a reader who simply scrolled up, not only for a history window.
    let chat = inlined(include_str!("../../../crates/views/chat/src/ui/chat.ice"));
    assert!(chat.contains("if !empty(messages) && (history_view || !at_live_tail)"));
    assert!(chat.contains("button \"↓  Jump to latest\""));
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
    let _ = app.__update(__DucktapeMessage::OpenChatSearchHit("general".into(), 7));
    assert!(app.history_view);

    // a resync carrying no chat news leaves the window — and its banner — alone
    let _ = app.__update(__DucktapeMessage::LiveResynced(backend::LiveRefresh {
        chat_loaded: false,
        ..live_refresh(app.hydration_generation, "general", "",
            Vec::new(),
        )
    }));
    assert!(
        app.history_view,
        "a pages-only resync did not touch the timeline, so the window stands"
    );

    // one that carries chat replaced it with the latest page
    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(app.hydration_generation, "general", "",
        Vec::new(),
    )));
    assert!(
        !app.history_view,
        "the rows on screen are the tail now — the banner is a lie about them"
    );

    // and a create lands you in a brand-new room, which has no history at all
    let _ = app.__update(__DucktapeMessage::OpenChatSearchHit("general".into(), 7));
    assert!(app.history_view);
    let mut created = chat_data("brand-new");
    created.generation = app.chat_generation;
    let _ = app.__update(__DucktapeMessage::ChannelCreated(created));
    assert!(!app.history_view);
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

    let mut chat_only = live_refresh(app.hydration_generation, "", "", Vec::new());
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
    app.page_text = ("h").to_string();
    let _ = app.__update(__DucktapeMessage::PageAutosaveTick);
    assert_eq!(
        app.block_autosave_status,
        AutosaveStatus::Idle,
        "a fabricated buffer must never be saved into a real page"
    );
}

/// A PLANE'S OP REFETCHES THAT PLANE AND NO OTHER.
///
/// These modules feed surfaces that were correct only at connect and at
/// tab-switch time: a validator joining, a device being renamed, a file being
/// committed — none of it reached a console already looking at the page that
/// shows it.
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

    let (members, account, dm) = (
        app.members_generation,
        app.account_generation,
        app.dm_peers_generation,
    );

    plane(&mut app, "valset");
    assert_eq!(app.members_generation, members + 1, "valset feeds members");
    assert_eq!(app.account_generation, account, "and nothing else");

    // the governance and files planes are their VIEWS' to re-read, through
    // the kernel's `rpc.live`; no app reading moves for either
    plane(&mut app, "governance");
    assert_eq!(
        app.members_generation,
        members + 1,
        "unchanged by governance"
    );

    // identity feeds TWO surfaces: the account card and the DM directory.
    plane(&mut app, "identity");
    assert_eq!(app.account_generation, account + 1);
    assert_eq!(app.dm_peers_generation, dm + 1);

    // the agents pair — `agent` for the register, `runs` for the liveness —
    // is the agents VIEW's to re-read, through the kernel's `rpc.live`; no
    // app reading moves for either
    plane(&mut app, "agent");
    plane(&mut app, "runs");
    assert_eq!(app.account_generation, account + 1, "and nothing else");

    plane(&mut app, "files");
    assert_eq!(app.members_generation, members + 1, "unchanged by files");

    // A module with no plane of its own moves nothing.
    let before = app.members_generation;
    plane(&mut app, "attribution");
    assert_eq!(
        app.members_generation, before,
        "an unrouted module is inert"
    );
}

/// THE PREVIOUS NETWORK'S ROOMS DO NOT SURVIVE INTO THIS ONE.
///
/// A workspace switch does not change the endpoint — the node comes back on the
/// same loopback port — so the console can live right through one: the websocket
/// drops, reconnects, and resyncs, and no `connect` ever re-runs to install the
/// new network's channel list outright. Every fold in the resync only ever ADDS
/// rows, so the sidebar kept every room the reader had ever seen: she joined a
/// network with one DM in it and went on seeing the `#general` of the workspace
/// she had just forgotten, clickable, with nothing behind it.
///
/// The signal costs nothing: the node pushes its own status document, which
/// names the chain, and `chat_chain_id` records the chain the rows on screen
/// were learned from.
#[test]
fn a_resync_across_a_chain_drops_the_previous_networks_rooms() {
    let room = |id: &str, head_seq: i64| backend::ChatChannel {
        id: id.into(),
        name: id.into(),
        archived: false,
        members_only: false,
        huddle_count: 0,
        head_seq,
    };
    let resync = |app: &Ducktape, channels: Vec<backend::ChatChannel>| {
        let mut refresh =
            live_refresh(app.hydration_generation, "dm-1", "", Vec::new());
        refresh.channels = channels;
        __DucktapeMessage::LiveResynced(refresh)
    };

    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    // The console is holding the network she left, and the node is now serving
    // the one she joined.
    app.chat_chain_id = "ducktape-industries#c7cf82df".into();
    app.network_chain_id = "ducktape-industries#549d70e8".into();
    app.channels = vec![room("general", 40), room("random", 3)];

    let _ = app.__update(resync(&app, vec![room("dm-1", 6)]));

    let held: Vec<&str> = app.channels.iter().map(|row| row.id.as_str()).collect();
    assert_eq!(
        held,
        vec!["dm-1"],
        "a room the network she left had is gone, not folded forward"
    );
    assert_eq!(
        app.chat_chain_id, app.network_chain_id,
        "and the list on screen now belongs to the chain that answered for it"
    );

    // ON THE SAME CHAIN THE FOLD IS BACK, and it is load-bearing: this read left
    // the node several queries ago, so a room created while it was in flight
    // must survive it and a head a delta moved must not walk back.
    app.channels.push(room("brand-new", 1));
    app.channels[0].head_seq = 9;
    let _ = app.__update(resync(&app, vec![room("dm-1", 6)]));
    assert!(
        app.channels.iter().any(|row| row.id == "brand-new"),
        "the room created mid-resync is still in the sidebar"
    );
    assert_eq!(
        backend::channel_head_seq(app.channels.clone(), "dm-1".into()),
        9,
        "and the head the delta moved does not walk back to the snapshot"
    );
}
