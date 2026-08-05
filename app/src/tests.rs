// The app's update-loop and source-sweep suite. `super` is the crate
// root, so the generated `Ducktape` app and both native modules resolve
// exactly as they did when this mod lived inline in main.rs.
use super::*;

/// EVERY SCREEN BODY, as one string. These are the slot bodies that used to
/// sit inline in `view.ice`; the sweeps below read the console's authored
/// markup, so they must read where that markup now lives. `view.ice` keeps
/// only the mounts, and asserting a widget shape against it now would pass
/// vacuously — the worst kind of green.
const SCREENS: &str = concat!(
    include_str!("ui/screens/chat.ice"),
    include_str!("ui/screens/forge.ice"),
    include_str!("ui/screens/governance.ice"),
    include_str!("ui/screens/overlays.ice"),
    include_str!("ui/screens/pages.ice"),
    include_str!("ui/screens/roster.ice"),
    include_str!("ui/screens/settings.ice"),
    include_str!("ui/screens/storage.ice"),
);

fn message(seq: i64, body: &str, deleted: bool) -> backend::ChatMessage {
    backend::ChatMessage {
        id: format!("message-{seq}"),
        seq,
        author: "user".into(),
        meta: format!("#{seq}"),
        body: body.into(),
        blocks: backend::paragraph_blocks(body),
        pending: false,
        rev: 2,
        edited: false,
        deleted,
        reply_count: 0,
        thread_seq: 0,
        show_author: true,
        initial: "U".into(),
        avatar_kind: "human".into(),
        mine: false,
        height: 0,
        time: 0,
        reactions: Vec::new(),
    }
}

fn compose(text: &str) -> iced::widget::text_editor::Content {
    iced::widget::text_editor::Content::with_text(text)
}

fn composer(app: &Ducktape) -> String {
    app.message_editor.text().trim().to_string()
}

fn reply_composer(app: &Ducktape) -> String {
    app.reply_editor.text().trim().to_string()
}

fn block_editor_text(app: &Ducktape) -> String {
    app.block_editor.text().trim_end().to_string()
}

#[test]
fn full_view_fits_a_four_mib_stack() {
    std::thread::Builder::new()
        .stack_size(4 * 1024 * 1024)
        .spawn(|| {
            let (mut app, _) = Ducktape::__boot();
            let console = iced::window::Id::unique();
            app.console_win = Some(console);
            let _ = app.__view(console);
            let onboarding = iced::window::Id::unique();
            app.onboarding_win = Some(onboarding);
            app.hub_step = "networks".into();
            let _ = app.__view(onboarding);
        })
        .unwrap()
        .join()
        .unwrap();
}

fn default_ice_color(name: &str) -> iced::Color {
    // 2.0 allows ONE theme contract and one palette, so the kit's theme moved
    // out of the vendored copy into the app's own file.
    let source = include_str!("ui/theme.ice");
    let value = source
        .lines()
        .find_map(|line| {
            let mut parts = line.split_ascii_whitespace();
            (parts.next() == Some(name)).then(|| parts.next()).flatten()
        })
        .unwrap_or_else(|| panic!("theme.ice palette is missing `{name}`"));
    let hex = value
        .strip_prefix('#')
        .expect("default Ice colors use hexadecimal literals");
    let value = u32::from_str_radix(hex, 16)
        .expect("default Ice colors are valid hexadecimal literals");
    match hex.len() {
        6 => iced::Color::from_rgb8(
            ((value >> 16) & 0xff) as u8,
            ((value >> 8) & 0xff) as u8,
            (value & 0xff) as u8,
        ),
        8 => iced::Color::from_rgba8(
            ((value >> 24) & 0xff) as u8,
            ((value >> 16) & 0xff) as u8,
            ((value >> 8) & 0xff) as u8,
            (value & 0xff) as f32 / 255.0,
        ),
        _ => panic!("default Ice colors use #RRGGBB or #RRGGBBAA"),
    }
}

fn live_refresh(
    generation: i64,
    active_channel: &str,
    messages: Vec<backend::ChatMessage>,
    active_page: &str,
    blocks: Vec<backend::PageBlock>,
) -> backend::LiveRefresh {
    backend::LiveRefresh {
        generation,
        chat_loaded: true,
        channels: Vec::new(),
        messages,
        active_channel: active_channel.into(),
        active_channel_name: active_channel.into(),
        active_channel_archived: false,
        active_channel_members_only: false,
        active_channel_huddle_count: 0,
        huddle_roster: Vec::new(),
        channel_members: Vec::new(),
        pages_loaded: true,
        pages: Vec::new(),
        blocks,
        active_page: active_page.into(),
        active_page_title: active_page.into(),
        active_page_parent: String::new(),
    }
}

fn posted_delta(channel: &str, row: backend::ChatMessage) -> backend::LiveUpdate {
    backend::LiveUpdate {
        kind: "chat".into(),
        status: "Live".into(),
        height: row.seq.max(1),
        chat: backend::ChatDelta {
            kind: "posted".into(),
            channel_id: channel.into(),
            seq: row.seq,
            message: row,
            ..backend::ChatDelta::default()
        },
        ..backend::LiveUpdate::default()
    }
}

fn chat_data(active_channel: &str, messages: Vec<backend::ChatMessage>) -> backend::ChatData {
    backend::ChatData {
        channels: Vec::new(),
        messages,
        active_channel: active_channel.into(),
        active_channel_name: active_channel.into(),
        active_channel_archived: false,
        active_channel_members_only: false,
        active_channel_huddle_count: 0,
        huddle_roster: Vec::new(),
        channel_members: Vec::new(),
        selected_message_seq: 0,
        selected_message_rev: 0,
        selected_message_body: String::new(),
        active_thread_seq: 0,
        thread_target_seq: 0,
        thread_messages: Vec::new(),
        thread_next_reply_offset: 0,
        thread_has_more: false,
    }
}

#[test]
fn shell_tab_is_app_state_and_palette_hits_switch_panes() {
    let (mut app, _) = Ducktape::__boot();
    assert_eq!(app.shell_tab, "chat");
    let _ = app.__update(__DucktapeMessage::SelectShellTab("pages".into()));
    assert_eq!(app.shell_tab, "pages");

    // a palette chat hit closes the palette and lands on the chat pane
    app.loading = false;
    app.mutation_phase = "idle".into();
    app.connected_rpc = "http://node".into();
    app.palette_open = true;
    let _ = app.__update(__DucktapeMessage::OpenChatSearchHit("general".into(), 7, 7));
    assert!(!app.palette_open);
    assert_eq!(app.shell_tab, "chat");
}

/// A hydration error belongs to the pane that raised it.
///
/// The banner has no self-retiring path — it is dismissed by hand or it
/// stays — so leaving it up across a navigation tells the user the pane they
/// just opened is broken. `select_shell_tab` clears it ABOVE both of its
/// early returns, which is the part worth pinning: the `!connected` return
/// and the chat/pages return each skip the generation bumps, and a clear
/// placed below either one would silently cover only some tabs.
#[test]
fn switching_panes_retires_a_stale_error_banner_on_every_tab() {
    // the disconnected path returns first, and must still clear.
    let (mut app, _) = Ducktape::__boot();
    app.error = "could not reach the node".into();
    let _ = app.__update(__DucktapeMessage::SelectShellTab("files".into()));
    assert_eq!(app.error, "", "the !connected early return must still clear");

    // the chat/pages path returns second, and must still clear.
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.error = "files: path not found".into();
    let _ = app.__update(__DucktapeMessage::SelectShellTab("pages".into()));
    assert_eq!(app.error, "", "the chat/pages early return must still clear");

    // and the full path, which falls through to the generation bumps.
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.error = "explorer hydration failed".into();
    let _ = app.__update(__DucktapeMessage::SelectShellTab("members".into()));
    assert_eq!(app.error, "");
    assert_eq!(app.shell_tab, "members");
}

/// The app has NO polling loop: every live surface rides the delta stream.
/// The only recurring subscriptions are wall clocks that nothing else can
/// supply — the huddle call timer and the toast's own dismissal — and this
/// pins that set exactly, so a reintroduced poll fails the build.
fn assert_no_polling(lifecycle: &str) {
    let recurring: Vec<_> = lifecycle
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("every "))
        .collect();
    assert_eq!(
        recurring,
        [
            "every 1s when huddle_joined -> tick",
            // the toast's dismissal clock: fine ticks against a per-toast
            // age, so a toast raised late in the old shared 2800ms window
            // no longer flashes and vanishes. Still gated on a visible
            // toast — it costs nothing at rest.
            "every 300ms when !empty(toast) -> toast_tick",
            // the block editor's autosave clock: the stock editor's edits
            // never pass through a handler, so a dirty buffer is the only
            // signal there is — and the gate IS the dirty test, so the tick
            // exists solely while unsaved text needs the node. It costs
            // nothing at rest and dies the moment the save lands.
            "every 900ms when (connected && !empty(selected_block_id) && trim(editor_text(block_editor)) != selected_block_saved_text) -> block_autosave_tick",
        ]
    );
}

#[test]
fn forge_depth_rides_the_established_seams() {
    // the forge handlers moved out of lifecycle.ice into their own file;
    // the seams they guard did not, so the guard reads both.
    let lifecycle = concat!(
        include_str!("ui/handlers/lifecycle.ice"),
        include_str!("ui/handlers/forge.ice"),
    );
    let forge = include_str!("ui/components/forge.ice");
    let backend = include_str!("ui/extern/backend.ice");

    // the item discussion IS a chat surface: hydrated through the chat
    // lanes and spliced by the SAME fold the chat pane uses, scoped to
    // the item's hidden channel — never a forge-private message path.
    assert!(lifecycle.contains(
        "forge_discussion = apply_chat_messages(forge_discussion, next.chat, forge_item_channel)"
    ));
    assert!(lifecycle.contains(
        "run send_message(connected_rpc, password, forge_item_channel, forge_discussion_pending"
    ));

    // a review pins the source head the reviewer saw; the merge CASes
    // BOTH heads (recompute on a moved branch, never a blind retry).
    //
    // the line comments ride INSIDE the review's own transaction — there is
    // no standalone comment op, so a comment cannot land without the
    // verdict it was written under, and it cannot outlive the diff it
    // anchors to (`keep_staged_comments` drops them when the head moves).
    assert!(backend.contains(
        "submit_forge_review(rpc:str, password:str, repo:str, number:i64, verdict:str, body:str, commit_oid:str, comments:[ForgeDraftComment])"
    ));
    assert!(backend.contains(
        "merge_forge_pr(rpc:str, password:str, repo:str, number:i64, source_branch:str, expected_source_oid:str, prev_target_oid:str)"
    ));

    // committed forge ops refresh scoped slices through the handler's one
    // terminal parallel — no polling, no per-op full reloads.
    assert!(lifecycle.contains(
        "run forge_live_refresh(connected_rpc, forge_repo, forge_item_number, next.kind, next.module, next.forge, forge_generation)"
    ));
    assert_no_polling(lifecycle);

    // approvals stay advisory in the merge box — `MergeAdvisory` is the
    // ONLY thing said above the merge button, and it recommends, never
    // refuses. The merged state renders the CAS'd commit.
    let forge_screen = include_str!("ui/screens/forge.ice");
    assert!(forge_screen.contains("MergeAdvisory change_requests=forge_item_change_requests"));
    assert_eq!(forge.matches("merge not recommended").count(), 2);
    // MergeAdvisory owns the count: no OTHER predicate may branch on it.
    // The one sibling read is the disclaimer's `<= 0`, which is the
    // no-advisory half and cannot contradict it.
    assert!(!forge_screen.contains("forge_item_change_requests > 0"));
    assert_eq!(
        forge_screen.matches("forge_item_change_requests <= 0").count(),
        1
    );
    assert!(forge_screen.contains("forge_merge_note(forge_item_merge_oid, forge_item_branches)"));
}

#[test]
fn background_refresh_preserves_editing_state() {
    let root = include_str!("ui/app.ice");
    let view = include_str!("ui/view.ice");
    let lifecycle = include_str!("ui/handlers/lifecycle.ice");
    assert!(!view.contains("sync_phase"));
    assert!(root.contains("use \"view.ice\""));
    assert!(!lifecycle.contains("on refresh_now"));
    // live surfaces (chat/pages) never need a manual refresh — the delta
    // stream keeps them current. The explorer's recent-window reload is
    // the one legitimate refresh affordance.
    let before_explorer = view
        .split_once("    explorer:")
        .map_or(view, |(head, _)| head);
    assert!(!before_explorer.contains("button \"Refresh\""));

    let refresh = lifecycle
        .split_once("on live_resynced(next)\n")
        .unwrap()
        .1
        .split_once("\non ")
        .unwrap()
        .0;
    let editable = [
        "rpc",
        "password",
        "channel_draft",
        "chat_search_draft",
        "page_draft",
        "block_draft",
        "page_search_draft",
    ];
    let overwrites_editable = refresh.lines().any(|line| {
        editable
            .iter()
            .any(|name| line.trim_start().starts_with(&format!("{name} =")))
    });
    assert!(!overwrites_editable);
    for scoped in [
        "channel_name_draft",
        "member_key_draft",
        "message_draft",
        "reply_draft",
    ] {
        assert!(refresh.contains(&format!(
            "{scoped} = retain_for_endpoint({scoped}, active_channel, \
keep_str(next.chat_loaded, next.active_channel, active_channel))"
        )));
    }
    assert!(refresh.contains("selected_message_seq = refreshed_required_message_seq("));
    assert!(refresh.contains("failed_message_draft = remember_failed_draft("));
    assert!(lifecycle.contains("run live_events(connected_rpc) when connected"));
    assert_no_polling(lifecycle);
    assert!(lifecycle.contains("run live_resync_load(connected_rpc"));
    assert!(lifecycle.contains("run refresh_live_thread(connected_rpc"));
    assert!(lifecycle.contains("parallel\n    run refresh_live_thread("));
    assert!(lifecycle.contains(
        "active_page_title = keep_str(next.pages_loaded, next.active_page_title, active_page_title)"
    ));
    assert!(lifecycle.contains("block_editor = refreshed_block_editor("));
    assert!(lifecycle.contains("selected_block_saved_text = resynced_saved"));
    // the comment rail is scoped to the PAGE it hangs off, so its draft
    // survives moving the cursor between blocks and dies with the page.
    assert!(lifecycle.contains(
        "block_comment_draft = retain_selected_string(block_comment_draft, block_comments_target)"
    ));
    // the live comment-list callback settles state and stops — re-entering
    // the resync from inside it would loop the rail against the page.
    let comment_callbacks = include_str!("ui/handlers/pages.ice")
        .split_once("on block_threads_loaded(next)\n")
        .unwrap()
        .1
        .split_once("\non load_more_block_threads")
        .unwrap()
        .0;
    assert!(!comment_callbacks.contains("run "));
}

#[test]
fn context_destroying_page_handlers_recover_drafts() {
    let pages = include_str!("ui/handlers/pages.ice");
    for name in [
        "open_page_search_hit(page_id, block_id)",
        "choose_page(id)",
        "select_block(key, id, kind, text, checked, open_actions)",
        "pages_mutated(next)",
    ] {
        let rest = pages.split_once(&format!("on {name}")).unwrap().1;
        let body = rest.split_once("\non ").map_or(rest, |(body, _)| body);
        assert!(body.contains("remember_orphaned_block_drafts("), "{name}");
        assert!(body.contains("remember_orphaned_comment_drafts("), "{name}");
    }
    let close_comments = pages
        .split_once("on close_block_comments\n")
        .unwrap()
        .1
        .split_once("\non ")
        .unwrap()
        .0;
    assert!(close_comments.contains("remember_orphaned_comment_drafts("));
}

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
    app.message_draft = "channel draft".into();
    app.selected_message_seq = 7;
    app.selected_message_rev = 2;
    app.hovered_message_seq = 7;
    app.message_action = "editing".into();
    app.message_edit_draft = "message edit".into();
    app.channel_settings_open = true;
    app.channel_name_draft = "channel rename".into();
    app.member_key_draft = "member".into();
    app.active_thread_seq = 7;
    app.thread_generation = 4;
    app.thread_target_seq = 9;
    app.thread_messages = vec![message(7, "old thread", false)];
    app.thread_next_reply_offset = 4;
    app.thread_has_more = true;
    app.thread_loading = true;
    app.reply_draft = "thread reply".into();
    app.pending_reply = "pending thread reply".into();
    app.active_page = "deleted-page".into();
    app.block_insert_open = true;
    app.block_insert_after_id = "deleted-block".into();
    app.block_draft = "new block draft".into();

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
    assert_eq!(app.hovered_message_seq, 0);
    assert_eq!(app.message_action, "toolbar");
    assert!(app.message_edit_draft.is_empty());
    assert!(!app.channel_settings_open);
    assert!(app.channel_name_draft.is_empty());
    assert!(app.member_key_draft.is_empty());
    assert_eq!(app.active_thread_seq, 0);
    assert_eq!(app.thread_generation, 5);
    assert_eq!(app.thread_target_seq, 0);
    assert!(app.thread_messages.is_empty());
    assert_eq!(app.thread_next_reply_offset, 0);
    assert!(!app.thread_has_more);
    assert!(!app.thread_loading);
    assert!(app.reply_draft.is_empty());
    assert!(app.pending_reply.is_empty());
    assert!(app.message_draft.is_empty());
    assert_eq!(app.active_page, "fallback-page");
    assert!(!app.block_insert_open);
    assert!(app.block_insert_after_id.is_empty());
}

#[test]
fn mutation_acks_preserve_open_editors_and_thread_state() {
    let (mut app, _) = Ducktape::__boot();
    app.active_channel = "general".into();
    app.selected_message_seq = 7;
    app.selected_message_rev = 2;
    app.message_action = "editing".into();
    app.message_edit_draft = "edit in progress".into();
    app.active_thread_seq = 9;
    app.thread_target_seq = 10;
    app.thread_messages = vec![message(9, "thread root", false)];
    app.thread_next_reply_offset = 3;
    app.thread_has_more = true;
    app.reply_editor = compose("reply in progress");
    app.message_editor = compose("next message");
    app.mutation_phase = "reaction".into();
    app.pending_message = "sent message".into();

    // an unrelated mutation's ack carries no snapshot — nothing to stomp
    let _ = app.__update(__DucktapeMessage::ChatAcked(true));

    assert_eq!(app.selected_message_seq, 7);
    assert_eq!(app.message_action, "editing");
    assert_eq!(app.message_edit_draft, "edit in progress");
    assert_eq!(app.active_thread_seq, 9);
    assert_eq!(app.thread_target_seq, 10);
    assert_eq!(app.thread_messages.len(), 1);
    assert_eq!(app.thread_next_reply_offset, 3);
    assert!(app.thread_has_more);
    assert_eq!(reply_composer(&app), "reply in progress");
    assert_eq!(composer(&app), "next message");
    assert_eq!(app.mutation_phase, "idle");
}

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
    app.reply_draft = "unsent reply".into();

    // the root's delete arrives as a delta: both lists tombstone the row
    // in place; the open thread stays open showing the tombstone (the
    // module allows replying to a tombstoned root).
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: "chat".into(),
        status: "Live".into(),
        height: 5,
        chat: backend::ChatDelta {
            kind: "deleted".into(),
            channel_id: "general".into(),
            seq: 9,
            ..backend::ChatDelta::default()
        },
        ..backend::LiveUpdate::default()
    }));

    assert!(app.messages[0].deleted);
    assert!(app.thread_messages[0].deleted);
    assert_eq!(app.thread_messages[0].body, "Message deleted");
    assert_eq!(app.active_thread_seq, 9, "the panel stays open");
    assert_eq!(app.reply_draft, "unsent reply");
}

#[test]
fn unrelated_resyncs_keep_an_initial_thread_load_alive() {
    let (mut refresh, _) = Ducktape::__boot();
    refresh.connected_rpc = "http://node".into();
    refresh.active_channel = "general".into();
    refresh.loading = false;
    refresh.mutation_phase = "idle".into();
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
        next_reply_offset: 1,
        has_more: true,
    }));
    assert_eq!(refresh.active_thread_seq, 7);
    assert_eq!(refresh.thread_messages.len(), 1);
    assert!(!refresh.thread_loading);
}

#[test]
fn pages_mutation_keeps_the_canonical_selected_block() {
    let (mut app, _) = Ducktape::__boot();
    app.mutation_phase = "block-kind".into();
    app.selected_block_id = "block-1".into();
    app.selected_block_kind = "Text".into();
    app.block_editor = compose("local");
    app.block_actions_open = true;

    let block = backend::PageBlock {
        key: 1,
        id: "block-1".into(),
        parent: "page-1".into(),
        kind: "Todo".into(),
        text: "canonical".into(),
        pending: false,
        checked: true,
        prefix: String::new(),
        child_count: 0,
        mark_count: 0,
        spans: Vec::new(),
    };
    let _ = app.__update(__DucktapeMessage::PagesMutated(backend::PagesData {
        pages: Vec::new(),
        blocks: vec![block],
        active_page: "page-1".into(),
        active_page_title: "Page".into(),
        active_page_parent: String::new(),
        selected_block_id: "block-1".into(),
        selected_block_kind: "Todo".into(),
        selected_block_text: "canonical".into(),
        selected_block_checked: true,
        page_title_selected: false,
    }));

    assert_eq!(app.selected_block_id, "block-1");
    assert_eq!(app.selected_block_kind, "Todo");
    assert_eq!(block_editor_text(&app), "canonical");
    assert!(app.selected_block_checked);
    assert!(!app.block_actions_open);
    assert_eq!(app.mutation_phase, "idle");
}

#[test]
fn ready_events_and_stale_searches_do_not_rehydrate_navigation() {
    let (mut chat, _) = Ducktape::__boot();
    chat.loading = false;
    chat.chat_search_generation = 4;
    chat.chat_searching = true;
    let _ = chat.__update(__DucktapeMessage::ChooseChannel("next".into()));
    assert_eq!(chat.chat_search_generation, 5);
    assert!(!chat.chat_searching);
    let _ = chat.__update(__DucktapeMessage::ChatSearchLoaded(
        backend::ChatSearchData {
            generation: 4,
            hits: vec![backend::ChatSearchHit {
                channel_id: "old".into(),
                seq: 1,
                root_seq: 1,
                author: "user".into(),
                text: "stale".into(),
                meta: "#1".into(),
            }],
        },
    ));
    assert!(chat.chat_search_hits.is_empty());

    let (mut pages, _) = Ducktape::__boot();
    pages.loading = false;
    pages.page_search_generation = 8;
    pages.page_searching = true;
    let _ = pages.__update(__DucktapeMessage::ChoosePage("next".into()));
    assert_eq!(pages.page_search_generation, 9);
    assert!(!pages.page_searching);
    let _ = pages.__update(__DucktapeMessage::PageSearchLoaded(
        backend::PageSearchData {
            generation: 8,
            hits: vec![backend::PageSearchHit {
                page_id: "old".into(),
                block_id: "old-block".into(),
                kind: "Text".into(),
                text: "stale".into(),
            }],
        },
    ));
    assert!(pages.page_search_hits.is_empty());

    let (mut live, _) = Ducktape::__boot();
    live.loading = false;
    live.block_height = 41;
    live.hydration_generation = 2;
    let _ = live.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: "ready".into(),
        status: "Live".into(),
        height: -1,
        load_chat: true,
        load_pages: true,
        ..backend::LiveUpdate::default()
    }));
    assert_eq!(
        live.hydration_generation, 3,
        "ready starts the subscribe-then-hydrate catch-up resync"
    );
    assert_eq!(live.block_height, 41, "a heightless event keeps the tip");
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
                iced::widget::text_editor::Action::Edit(
                    iced::widget::text_editor::Edit::Insert(character),
                ),
            )),
        ));
    }
    assert_eq!(composer(&app), "first");

    let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
        editor::composer_submit_event(),
    ));
    let first_id = app.messages[0].id.clone();
    assert_eq!(app.mutation_phase, "idle");
    assert!(app.message_draft.is_empty());
    assert!(composer(&app).is_empty());
    assert_eq!(app.messages.len(), 1);
    assert!(app.messages[0].pending);

    app.message_editor = compose("second");
    let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
        editor::composer_submit_event(),
    ));
    let second_id = app.messages[1].id.clone();
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

    // …the committed row arrives as a delta and settles ONLY its pending
    let mut committed = message(1, "first", false);
    committed.id = first_id;
    let _ = app.__update(__DucktapeMessage::LiveUpdated(posted_delta(
        "general", committed,
    )));
    assert_eq!(composer(&app), "third");
    assert_eq!(app.mutation_phase, "idle");
    assert!(!app.messages[0].pending);
    assert_eq!(app.messages[0].seq, 1);
    assert_eq!(app.messages[1].id, second_id);
    assert!(app.messages[1].pending);

    let chat = include_str!("ui/screens/chat.ice");
    assert!(chat.contains("stack #message(message.id) w=fill"));
    assert!(!chat.contains("#message(message.seq)"));
}

#[test]
fn flow_typing_inserts_blocks_in_order() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_page = "welcome".into();
    app.block_insert_open = true;
    app.block_insert_after_id = String::new();

    app.block_draft = "first".into();
    let _ = app.__update(__DucktapeMessage::AddBlockSubmit);
    let first_id = app.blocks[0].id.clone();
    let _ = app.__update(__DucktapeMessage::BlockAdded(backend::BlockInsertResult {
        data: backend::PagesData {
            pages: Vec::new(),
            blocks: vec![backend::PageBlock {
                key: 1,
                id: first_id.clone(),
                parent: "welcome".into(),
                kind: "Text".into(),
                text: "first".into(),
                pending: false,
                checked: false,
                prefix: String::new(),
                child_count: 0,
                mark_count: 0,
                spans: Vec::new(),
            }],
            active_page: "welcome".into(),
            active_page_title: "Welcome".into(),
            active_page_parent: String::new(),
            selected_block_id: String::new(),
            selected_block_kind: String::new(),
            selected_block_text: String::new(),
            selected_block_checked: false,
            page_title_selected: false,
        },
        operation_id: first_id.clone(),
        page_id: "welcome".into(),
    }));
    assert_eq!(
        app.block_insert_after_id, first_id,
        "the insert anchor advances so Enter-typing appends in order"
    );

    // the next flow-typed block lands AFTER the first, not before it
    app.block_draft = "second".into();
    let _ = app.__update(__DucktapeMessage::AddBlockSubmit);
    assert_eq!(app.blocks.len(), 2);
    assert_eq!(app.blocks[0].text, "first");
    assert_eq!(app.blocks[1].text, "second");
    assert!(app.blocks[1].pending);
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

    let chat = include_str!("ui/screens/chat.ice");
    assert!(chat.contains("button \"Jump to latest\""));
    assert!(chat.contains("-> emit(choose_channel, active_channel)"));
}

#[test]
fn message_actions_require_explicit_intent() {
    let (mut app, _) = Ducktape::__boot();
    app.mutation_phase = "idle".into();

    let _ = app.__update(__DucktapeMessage::MessageEntered(7));
    assert_eq!(app.hovered_message_seq, 7);
    app.chat_pointer_y = 450.0;
    app.chat_height = 500.0;
    let _ = app.__update(__DucktapeMessage::OpenMessageActions(7, "hello".into(), 2));
    assert_eq!(app.message_menu_y, 260.0);
    let _ = app.__update(__DucktapeMessage::MessageExited(7));
    assert_eq!(app.hovered_message_seq, 0);
    assert_eq!(app.selected_message_seq, 7);
    assert_eq!(app.message_action, "more");
    let _ = app.__update(__DucktapeMessage::BeginMessageEdit(7, "hello".into(), 2));
    assert_eq!(app.message_action, "editing");
    // Every cancel affordance in the view routes `clear_message_selection`
    // (view.ice:441, :467, :511, :523, :538), so that is the transition
    // under test — it drops to the toolbar AND drops the selection.
    let _ = app.__update(__DucktapeMessage::ClearMessageSelection);
    assert_eq!(app.message_action, "toolbar");
    assert_eq!(app.selected_message_seq, 0);
    let _ = app.__update(__DucktapeMessage::OpenMessageReactions(
        7,
        "hello".into(),
        2,
    ));
    assert_eq!(app.message_action, "reactions");
    let _ = app.__update(__DucktapeMessage::ClearMessageSelection);
    let _ = app.__update(__DucktapeMessage::ArmMessageDelete(7, "hello".into(), 2));
    assert_eq!(app.message_action, "delete");
    let _ = app.__update(__DucktapeMessage::OpenMessageActionsAccessibly(
        7,
        "hello".into(),
        2,
    ));
    assert_eq!(app.message_action, "more");
    assert_eq!(app.message_menu_y, 0.0);
}

#[test]
fn message_action_toolbar_stays_compact_and_accessible() {
    let components = include_str!("ui/components/chat.ice");
    let toolbar = components
        .split_once("component MessageCard")
        .unwrap()
        .1
        .split_once("component ThreadMessageCard")
        .unwrap()
        .0;
    assert!(toolbar.contains("mouse enter=emit(message_entered, message.seq)"));
    assert!(toolbar.contains("if !message.deleted && !message.pending && hovered"));
    assert!(toolbar.contains("if !message.deleted && !message.pending && !hovered"));
    assert!(toolbar.contains("-> emit(open_message_actions_accessibly, message.seq, message.body, message.rev)"));
    assert!(!toolbar.contains("hovered || selected"));
    assert_eq!(toolbar.matches("w=26.0 h=26.0").count(), 1);
    // the artifact's hover bar is five 27×25 cells: three one-tap reactions,
    // the reaction picker and the overflow menu (Console:244).
    assert_eq!(toolbar.matches("w=27.0 h=25.0").count(), 5);
    // the one svg cell takes the icon as a direct child; a `h=fill` wrapper
    // inside a fixed-size button collapses an SVG to a hairline. The other
    // four cells are the artifact's own typographic glyphs, not icons.
    assert_eq!(toolbar.matches("p=5.0 @icon_action").count(), 1);
    assert!(components.contains(
        "text message.author size=13.0 wrap=none font=display @text-fg\n          if message.avatar_kind == \"agent\""
    ));
    // the stamp beside the author is the block the message was finalized
    // in — a chain fact the app can prove, never a wall-clock time.
    assert!(components.contains(
        "if message.height > 0\n            text height_label_short(message.height) size=11.0 wrap=none font=code_medium @text-hint"
    ));
    // Slack-style grouping: the shared avatar + author header only renders
    // for a run's first message; continuations keep the body aligned via a
    // gutter that matches the avatar's width.
    assert!(components.contains(
        "if message.show_author\n      MessageAvatar initials=message.initial kind=message.avatar_kind"
    ));
    assert!(components.contains("if !message.show_author\n      space w=30.0"));
    assert!(components.contains("\"human\"\n        PersonAvatar initials=initials plate=30.0 ink=11.0"));
    assert!(components.contains("\"agent\"\n        AgentAvatar initials=initials plate=30.0 ink=11.0"));
    assert!(!components.contains("avatar_style"));
    // Rich bodies render structured blocks, not one flattened string.
    assert!(components.contains("for block in message.blocks"));
    assert!(components.contains("if block.kind == \"code\""));
    assert!(components.contains("flex w=fill wrap=wrap"));
    // The hover toolbar uses the shared popover depth role instead of
    // carrying another inline shadow variant. The artifact's own plate is
    // `border-radius:9px; box-shadow:0 3px 12px rgba(40,38,34,.13);
    // padding:2px` (Console:243).
    assert!(toolbar.contains(
        "box p=2.0 bg=surface border=border border-w=1.0 r=9.0 shadow=shadow_popover shadow-y=3.0 shadow-blur=12.0"
    ));
    for label in ["Open thread", "Manage reactions", "More message actions"] {
        assert!(toolbar.contains(&format!("label=\"{label}\"")));
    }
    assert!(components.contains(
        "button label=\"Open thread\" disabled=disabled p=5.0 @icon_action -> emit(open_thread_for, message.seq)"
    ));

    let chat = include_str!("ui/screens/chat.ice");
    assert!(
        chat.contains(
            "overlay when=(selected_message_seq > 0 && message_action != \"toolbar\")"
        )
    );
    assert!(chat.contains("dismiss=emit(clear_message_selection) backdrop=transparent"));
    assert!(chat.contains("mouse press-at=emit(chat_pointer_pressed, _, _)"));
    // per-press, never per-move: a move= stream here rebuilds the view per pixel
    assert!(!chat.contains("mouse move="));
    assert!(chat.contains("float x=0.0 y=message_menu_y"));
    // the pointer sensor is the MESSAGE-LIST stack's first child, so it
    // measures the message list itself and not whatever an overlay happens
    // to cover. The anchor names that stack by its exact indentation: the
    // outer content stack (which floats the search-results card) sits
    // shallower and must not satisfy this pin.
    let sensor = chat
        .split_once("                stack w=fill h=fill\n")
        .unwrap()
        .1;
    assert!(
        sensor
            .trim_start()
            .starts_with("sensor show=emit(chat_resized, _, _)")
    );
    let overlay_content = chat
        .split_once("                  content\n")
        .unwrap()
        .1
        .split_once("                  layer\n")
        .unwrap()
        .0;
    assert!(overlay_content.contains("space w=fill h=fill"));
    assert!(!overlay_content.contains("message_action =="));
    let more = chat
        .split_once("message_action == \"more\"")
        .unwrap()
        .1
        .split_once("message_action == \"reactions\"")
        .unwrap()
        .0;
    // Icon + sentence rows on one raised plate; Esc and the backdrop dismiss,
    // so the menu lists no Close row of its own.
    for row in [
        "label=\"Manage reactions\"",
        "label=\"Reply in thread\"",
        "label=\"Edit message\"",
        "label=\"Delete message\"",
    ] {
        assert!(more.contains(row), "{row}");
    }
    for icon in ["\"emoji\"", "\"nav-chat\"", "\"pencil\"", "\"trash\""] {
        assert!(more.contains(&format!("Icon name={icon}")), "{icon}");
    }
    assert!(!more.contains("button \"Close\""));
    // The reactions arm is the shared ADD grid — removal rides the message's
    // own reaction chips, which already toggle off for `reacted_by_me`.
    let picker = chat
        .split_once("message_action == \"reactions\"")
        .unwrap()
        .1
        .split_once("message_action == \"editing\"")
        .unwrap()
        .0;
    assert!(picker.contains("for emoji in reaction_palette()"));
    assert!(picker.contains("-> emit(add_reaction_submit, emoji)"));
    assert!(!picker.contains("remove_reaction_submit"));

    let handlers = include_str!("ui/handlers/chat.ice");
    for focus in [
        "#workspace-tabs/content/chat/message-action-focus",
        "#workspace-tabs/content/chat/message-reaction-focus",
        "#workspace-tabs/content/chat/message-delete-focus",
    ] {
        assert!(handlers.contains(focus));
    }
    for focus in [
        "#message-action-focus",
        "#message-reaction-focus",
        "#message-delete-focus",
    ] {
        assert!(chat.contains(&format!("input \"\" {focus}")));
    }
    assert_eq!(handlers.matches("task widget focus-next").count(), 7);
    assert!(!include_str!("ui/extern/backend.ice").contains("task focus_next()"));
    let activate = handlers
        .split_once("on begin_message_edit(seq, body, rev)\n")
        .unwrap()
        .1
        .split_once("\non ")
        .unwrap()
        .0;
    assert!(activate.contains("task widget focus #workspace-tabs/content/chat/message-edit"));
}

#[test]
fn thread_messages_mirror_the_main_action_system() {
    let components = include_str!("ui/components/chat.ice");
    let card = components
        .split_once("component ThreadMessageCard")
        .unwrap()
        .1;
    assert!(card.contains(
        "mouse enter=emit(thread_message_entered, message.seq) exit=emit(thread_message_exited, message.seq)"
    ));
    assert!(
        card.contains(
            "-> emit(open_thread_message_reactions, message.seq, message.body, message.rev)"
        )
    );
    assert!(
        card.contains("-> emit(open_thread_message_actions, message.seq, message.body, message.rev)")
    );
    // No open-thread action from inside a thread you are already reading.
    assert!(!card.contains("open_thread_for"));

    let thread = include_str!("ui/screens/chat.ice")
        .split_once("if active_thread_seq > 0 && !channel_settings_open")
        .unwrap()
        .1;
    // A SECOND overlay, keyed on thread-scoped state, independent of the main one.
    assert!(thread.contains(
        "overlay when=(thread_selected_seq > 0 && thread_message_action != \"toolbar\")"
    ));
    assert!(
        thread.contains("dismiss=emit(clear_thread_message_selection) backdrop=transparent")
    );
    assert!(thread.contains("float x=0.0 y=thread_menu_y"));
    assert!(thread.contains("mouse press-at=emit(thread_pointer_pressed, _, _)"));
    // same seat as the message list — the rail measures itself
    assert!(
        thread.contains("sensor show=emit(thread_resized, _, _)")
    );
    // The picker is the shared ADD grid targeting the thread selection;
    // removal rides the reply's own reaction chips.
    assert!(thread.contains("for emoji in reaction_palette()"));
    assert!(thread.contains("-> emit(add_reaction_at, thread_selected_seq, emoji)"));
    // More-menu omits Reply in thread (already inside the thread) and Close.
    let more = thread
        .split_once("thread_message_action == \"more\"")
        .unwrap()
        .1
        .split_once("thread_message_action == \"reactions\"")
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

    let handlers = include_str!("ui/handlers/chat.ice");
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
    assert!(delete.contains(
        "delete_message(connected_rpc, password, active_channel, thread_selected_seq)"
    ));
}

#[test]
fn thread_action_state_is_independent_of_the_main_message_menu() {
    let (mut app, _) = Ducktape::__boot();
    app.mutation_phase = "idle".into();
    app.active_channel = "general".into();
    app.active_thread_seq = 1;

    // Opening a thread action must not touch the main message menu.
    app.thread_pointer_y = 400.0;
    app.thread_height = 500.0;
    let _ = app.__update(__DucktapeMessage::OpenThreadMessageActions(
        2,
        "reply".into(),
        3,
    ));
    assert_eq!(app.thread_selected_seq, 2);
    assert_eq!(app.thread_message_action, "more");
    assert_eq!(app.thread_menu_y, 210.0);
    assert_eq!(app.selected_message_seq, 0);
    assert_eq!(app.message_action, "toolbar");

    // And a main message action must not touch the thread menu.
    let _ = app.__update(__DucktapeMessage::OpenMessageActions(5, "root".into(), 1));
    assert_eq!(app.selected_message_seq, 5);
    assert_eq!(app.message_action, "more");
    assert_eq!(app.thread_selected_seq, 2);
    assert_eq!(app.thread_message_action, "more");

    let _ = app.__update(__DucktapeMessage::ClearThreadMessageSelection);
    assert_eq!(app.thread_selected_seq, 0);
    assert_eq!(app.thread_message_action, "toolbar");
    assert_eq!(app.selected_message_seq, 5);
    assert_eq!(app.message_action, "more");
}

#[test]
fn opening_another_thread_invalidates_the_pending_thread() {
    let (mut app, _) = Ducktape::__boot();
    app.mutation_phase = "idle".into();
    app.active_channel = "general".into();
    app.selected_message_seq = 1;
    app.thread_generation = 4;
    app.thread_loading = true;
    app.active_thread_seq = 1;
    app.thread_messages =
        backend::optimistic_message(Vec::new(), "old thread".into(), "pending-old".into());
    app.reply_editor = compose("old reply");

    let _ = app.__update(__DucktapeMessage::OpenThreadFor(2));
    assert_eq!(app.thread_generation, 5);
    assert!(app.thread_loading);
    assert_eq!(app.active_thread_seq, 2);
    assert!(app.thread_messages.is_empty());
    assert!(reply_composer(&app).is_empty());

    let _ = app.__update(__DucktapeMessage::ThreadLoaded(backend::ThreadLoadData {
        generation: 4,
        root_seq: 1,
        target_seq: 0,
        messages: Vec::new(),
        next_reply_offset: 0,
        has_more: false,
    }));
    assert_eq!(app.active_thread_seq, 2);
}

#[test]
fn thread_pagination_preserves_multiple_pending_replies() {
    let message = |seq: i64, thread_seq: i64, body: &str| backend::ChatMessage {
        id: format!("message-{seq}"),
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
        mine: false,
        height: 0,
        time: 0,
        reactions: Vec::new(),
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
            next_reply_offset: 2,
            has_more: false,
        },
    ));
    assert_eq!(app.thread_messages.len(), 5);
    assert_eq!(app.thread_messages[1].body, "first");
    assert_eq!(app.thread_next_reply_offset, 2);
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
fn changing_endpoint_clears_remote_bound_interaction_state() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.connected_rpc = "http://node-a".into();
    app.rpc = "http://node-b".into();
    app.password = "node-a-password".into();
    app.selected_message_seq = 1;
    app.message_action = "editing".into();
    app.message_edit_draft = "node a edit".into();
    app.active_thread_seq = 1;
    app.reply_editor = compose("node a reply");
    app.selected_block_id = "same-id".into();
    app.block_editor = compose("node a block");
    app.block_comments_open = true;
    app.block_comments_target = "same-id".into();
    app.block_comment_draft = "node a comment".into();
    app.message_editor = compose("node a message");
    app.page_search_draft = "node a search".into();

    let _ = app.__update(__DucktapeMessage::Reconnect);

    assert_eq!(app.connected_rpc, "http://node-b");
    assert!(app.password.is_empty());
    assert_eq!(app.selected_message_seq, 0);
    assert_eq!(app.message_action, "toolbar");
    assert!(app.message_edit_draft.is_empty());
    assert_eq!(app.active_thread_seq, 0);
    assert!(reply_composer(&app).is_empty());
    assert!(app.selected_block_id.is_empty());
    assert!(block_editor_text(&app).is_empty());
    assert!(!app.block_comments_open);
    assert!(app.block_comments_target.is_empty());
    assert!(app.block_comment_draft.is_empty());
    assert!(app.message_draft.is_empty());
    assert!(composer(&app).is_empty());
    assert!(app.page_search_draft.is_empty());

    let _ = app.__update(__DucktapeMessage::Failed(backend::AppError {
        message: "offline".into(),
        committed: false,
    }));
    assert_eq!(app.connected_rpc, "http://node-b");
}

#[test]
fn same_endpoint_reconnect_preserves_unsent_drafts() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.connected_rpc = "http://node-a".into();
    app.rpc = "http://node-a/".into();
    app.message_editor = compose("next message");
    app.failed_message_draft = "unsent message".into();

    let _ = app.__update(__DucktapeMessage::Reconnect);

    assert_eq!(app.rpc, "http://node-a");
    assert_eq!(app.connected_rpc, "http://node-a");
    assert_eq!(composer(&app), "next message");
    assert_eq!(app.failed_message_draft, "unsent message");
}

#[test]
fn page_search_load_selects_the_canonical_block() {
    let (mut app, _) = Ducktape::__boot();
    let _ = app.__update(__DucktapeMessage::PagesUpdated(backend::PagesData {
        pages: Vec::new(),
        blocks: vec![backend::PageBlock {
            key: 0,
            id: "block-1".into(),
            parent: "page-1".into(),
            kind: "Todo".into(),
            text: "Canonical text".into(),
            pending: false,
            checked: true,
            prefix: String::new(),
            child_count: 0,
            mark_count: 0,
            spans: Vec::new(),
        }],
        active_page: "page-1".into(),
        active_page_title: "Page".into(),
        active_page_parent: String::new(),
        selected_block_id: "block-1".into(),
        selected_block_kind: "Todo".into(),
        selected_block_text: "Canonical text".into(),
        selected_block_checked: true,
        page_title_selected: false,
    }));

    assert_eq!(app.selected_block_id, "block-1");
    assert_eq!(app.selected_block_kind, "Todo");
    assert_eq!(block_editor_text(&app), "Canonical text");
    assert!(app.selected_block_checked);
}

#[test]
fn page_blocks_can_be_created_but_not_converted_to_subpages() {
    let (app, _) = Ducktape::__boot();
    assert!(app.block_kinds.iter().any(|kind| kind == "Page"));
    assert!(!app.editable_block_kinds.iter().any(|kind| kind == "Page"));

    let view = include_str!("ui/screens/pages.ice");
    assert!(view.contains("if !block.pending && block.kind == \"Page\""));
    assert!(view.contains(
        "button label=block.kind description=block.text w=fill p=5.0 @ghost_action -> emit(choose_page, block.id)"
    ));
    assert!(view.contains(
        "if !block.pending && block.kind != \"Page\" && block.id == selected_block_id"
    ));
}

#[test]
fn page_title_and_block_actions_use_native_focus_and_overlay_paths() {
    let components = include_str!("ui/components/pages.ice");
    let handlers = include_str!("ui/handlers/pages.ice");
    let view = include_str!("ui/screens/pages.ice");

    assert!(components.contains("task widget focus #title-input"));
    assert!(!components.contains("defer_focus"));
    assert!(!handlers.contains("focus_page_title"));
    assert!(!include_str!("ui/extern/backend.ice").contains("defer_focus"));
    assert!(view.contains("mouse press-at=emit(pages_pointer_pressed, _, _)"));
    assert!(!view.contains("mouse move="));
    assert!(view.contains("overlay when=(connected && !empty(active_page)"));
    assert!(view.contains("dismiss=emit(close_block_actions) backdrop=transparent"));
    assert!(view.contains("float x=(block_menu_x + 10.0)"));
    assert!(!view.contains("pin x=(block_menu_x"));
    assert!(view.contains("BlockActionsMenu block_id=selected_block_id"));
    assert!(view.matches("button \"Insert divider\"").count() == 2);

    assert!(components.contains("text block.prefix size=12.0"));
    assert!(components.contains(
        "component InlineBlockInsert(kind:str, kinds:[str], disabled:bool, prefix:str)"
    ));
    assert!(view.contains("prefix=block.prefix #block-insert-row"));
    // the title sits in the same 56px gutter the blocks hang off, so the
    // document's left edge is one line all the way down.
    let title_gutter = view.split_once("box w=fill pl=56.0\n").unwrap().1;
    assert!(title_gutter.trim_start().starts_with("PageTitleEditor rpc="));
}

#[test]
fn shell_uses_canonical_glass_and_opaque_content() {
    let ui = concat!(
        include_str!("ui/app.ice"),
        include_str!("ui/extern/backend.ice"),
        include_str!("ui/state.ice"),
        include_str!("ui/theme.ice"),
        include_str!("ui/view.ice"),
        include_str!("ui/components/chat.ice"),
        include_str!("ui/components/dm.ice"),
        include_str!("ui/components/files.ice"),
        include_str!("ui/components/forge.ice"),
        include_str!("ui/components/huddle.ice"),
        include_str!("ui/components/icon.ice"),
        include_str!("ui/components/kit.ice"),
        include_str!("ui/components/node.ice"),
        include_str!("ui/components/onboarding.ice"),
        include_str!("ui/components/overlay.ice"),
        include_str!("ui/components/pages.ice"),
        include_str!("ui/components/patterns.ice"),
        include_str!("ui/components/roster.ice"),
        include_str!("ui/components/shell.ice"),
        include_str!("ui/handlers/lifecycle.ice"),
        include_str!("ui/handlers/chat.ice"),
        include_str!("ui/handlers/pages.ice"),
    );
    for gradient in ["linear(", "radial(", "conic("] {
        assert!(!ui.contains(gradient), "{gradient}");
        assert!(!SCREENS.contains(gradient), "{gradient}");
    }
    // The window is opaque. iced has no backdrop blur, so the chrome paints
    // the artifact's own non-glass ladder — desk/rail/sidebar/content — and
    // never a translucent tint that would composite over the desktop.
    let app = include_str!("ui/app.ice");
    assert!(!app.contains("\n    transparent true"));
    assert!(!app.contains("\n    blur true"));
    assert!(app.contains("\n  bg app_background"));
    assert!(app.contains("titlebar-transparent true"));
    assert!(app.contains("fullsize-content-view true"));
    assert!(app.contains("font \"../../../crates/design/assets/fonts/Geist[wght].ttf\""));
    assert!(!ui.contains("white/"));
    assert!(!ui.contains("bg=glass_"));
    assert!(!SCREENS.contains("white/"));
    assert!(!SCREENS.contains("bg=glass_"));

    // The palette moved with the theme: 2.0 permits one contract and one
    // palette, and the vendored kit copy no longer carries either.
    let defaults = include_str!("ui/theme.ice");
    for material in [
        "bg         #fdfdfb",
        "surface    #ffffff",
        "fg         #2c2b27",
        "muted_bg   #f6f5f2",
        "primary    #26251f",
        "brand      #a05a3c",
        "ring       #26251f",
        "glass_thin #fdfcfa80",
        "glass_regular #fdfcfa9e",
        "glass_sheet #fdfcfadb",
        "shadow_popover #28262221",
        "shadow_toast #28262238",
        "shadow_modal #2826224d",
    ] {
        assert!(defaults.contains(material), "{material}");
    }
    let theme = include_str!("ui/theme.ice");
    assert!(theme.contains("font ui family=\"Geist\" weight=normal"));
    assert!(theme.contains("font display family=\"Geist\" weight=semibold"));
    assert!(theme.contains("font strong family=\"Geist\" weight=bold"));
    assert!(theme.contains("font code_medium family=\"Geist Mono\" weight=medium"));
    assert!(theme.contains("font code_semibold family=\"Geist Mono\" weight=semibold"));
    for app_token in [
        "desk #e3e1d9",
        "rail #fafaf8",
        "window_line #d6d4cc",
        "card_line #ece9e1",
        "caption #9a988f",
        "meta #a7a59b",
        "hint #b3b1a8",
        "label #bdbbb1",
        "icon_idle #cbc9bf",
        "sidebar #fbfbf9",
        "elevated #f3f2ef",
        "subtle #ecebe6",
        "row_hover #f8f7f3",
        "rail_hover #f0efea",
        "separator #efeee9",
        "scrim #28262257",
    ] {
        assert!(theme.contains(app_token), "{app_token}");
    }

    let shell = include_str!("ui/components/shell.ice");
    // the shell is titlebar + optional degradation banner over the panes.
    assert!(shell.contains(
        "component TitleBar(network:str, height:i64, loading:bool, degraded:bool, bell_badge:i64, bell_sev:str, tier:str, root_hash:str, consensus_view:str, quorum:str, reachable:str, last_finalized:i64, checkpoint:i64)"
    ));
    // The bar exists only in the console window now — the launch window
    // wears OS chrome — so the chip and the status/bell cluster are
    // unconditional: no `phase` discriminant may return here.
    let bar = shell.split_once("component TitleBar(").unwrap().1;
    let bar = bar.split_once("\ncomponent ").unwrap().0;
    assert!(bar.contains("NetworkChip name=network"));
    assert!(!bar.contains("phase"));
    assert!(shell.contains("component ConnectionBanner(status:str)"));
    assert!(shell.contains("if degraded\n          ConnectionBanner status=status"));
    assert!(shell.contains("box #root w=74.0 h=fill pt=13.0 pb=10.0 bg=rail"));
    assert!(SCREENS.contains("box w=236.0 h=fill bg=sidebar clip=true"));
    assert!(SCREENS.contains("box w=230.0 h=fill bg=sidebar clip=true"));

    assert!(SCREENS.contains("input \"\" #rpc label=\"RPC endpoint\""));
    assert!(SCREENS.contains("if active_thread_seq > 0 && !channel_settings_open"));
    assert!(SCREENS.contains("box w=fill p=5.0 bg=transparent border=fg/12"));
    // the palette card moved into the overlay layer with the rest of the
    // window-level surfaces; the assertion follows the code it guards.
    let overlays = include_str!("ui/screens/overlays.ice");
    assert!(overlays.contains(
        "bg=surface border=border border-w=1.0 r=14.0 shadow=shadow_modal shadow-y=24.0 shadow-blur=60.0"
    ));

    for authored in [shell, include_str!("ui/components/pages.ice"), SCREENS] {
        assert!(!authored.contains("shadow=black/"));
        assert!(!authored.contains("shadow=shadow "));
    }
}

#[test]
fn compact_controls_share_a_single_geometry_and_type_scale() {
    assert!(SCREENS.contains("p=6.2 text-size=13.0 line-h=1.2"));
    // The composer geometry moved into the `rich_composer` extern args
    // (min_h, max_h, pad); type scale (13.5/1.3) is owned by the adapter.
    // Both chat composers share one plate; the forge note runs compact.
    assert_eq!(
        SCREENS.matches(", shift_held, 44.0, 150.0, 10.0) #").count(),
        2
    );
    assert!(SCREENS.contains(", shift_held, 38.0, 120.0, 6.0) #forge-note"));
    assert!(SCREENS.contains("button \"Send\" disabled="));
    assert!(SCREENS.contains(
        "h=29.0 p=7.0 @primary_action -> emit(composer_event, composer_submit_event())"
    ));
    assert!(
        SCREENS
            .matches("box w=fill h=fill align-x=center align-y=center")
            .count()
            >= 10
    );
    for line in SCREENS
        .lines()
        .filter(|line| line.trim_start().starts_with("input "))
    {
        assert!(!line.contains(" h="), "{line}");
    }

    let components = concat!(
        include_str!("ui/components/shell.ice"),
        include_str!("ui/components/chat.ice"),
        include_str!("ui/components/pages.ice"),
    );
    // the pane header is ONE geometry: a 50px plate holding a `gap=9.0`
    // centered row. Chat and pages both draw it, from their screens — the
    // components carry the pane bodies, never a second header shape.
    let pane_headers: Vec<_> = SCREENS
        .lines()
        .zip(SCREENS.lines().skip(1))
        .filter(|(plate, _)| {
            let plate = plate.trim_start();
            plate == "box w=fill h=50.0 pl=18.0 pr=18.0"
                || plate == "box w=fill h=50.0 pl=22.0 pr=22.0"
        })
        .map(|(_, row)| row.trim_start())
        .collect();
    assert_eq!(pane_headers, ["row w=fill h=fill gap=9.0 align=center"; 2]);
    assert!(!components.contains("row w=fill h=fill gap=9.0 align=center"));
    assert!(
        components.contains(
            "button label=\"Insert block below\" disabled=disabled w=28.0 h=28.0 p=0.0"
        )
    );
    for line in SCREENS.lines().chain(components.lines()).filter(|line| {
        [
            "button \"+\" label",
            "button \"×\" label",
            "button \"…\" label",
        ]
        .iter()
        .any(|needle| line.contains(needle))
    }) {
        assert!(line.contains("w="), "{line}");
        assert!(line.contains("h="), "{line}");
    }
}

#[test]
fn semantic_recipes_own_action_focus_and_status_colors() {
    fn assert_recipe_owns_states(name: &str, source: &str, recipe: &str) {
        let lines: Vec<_> = source.lines().collect();
        for (index, line) in lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.contains(recipe))
        {
            let indentation = line.len() - line.trim_start().len();
            for child in &lines[index + 1..] {
                let trimmed = child.trim_start();
                if trimmed.is_empty() {
                    continue;
                }
                let child_indentation = child.len() - trimmed.len();
                if child_indentation <= indentation {
                    break;
                }
                let is_direct_state = child_indentation == indentation + 2
                    && ["active ", "hovered ", "pressed ", "disabled "]
                        .iter()
                        .any(|state| trimmed.starts_with(state));
                assert!(
                    !is_direct_state,
                    "{name}: {recipe} must own its state colors: {child:?}"
                );
            }
        }
    }

    fn assert_controls_inherit_focus(name: &str, source: &str) {
        let lines: Vec<_> = source.lines().collect();
        for (index, line) in lines.iter().enumerate().filter(|(_, line)| {
            line.trim_start().starts_with("input ") && line.contains("@control")
        }) {
            let indentation = line.len() - line.trim_start().len();
            for child in &lines[index + 1..] {
                let trimmed = child.trim_start();
                if trimmed.is_empty() {
                    continue;
                }
                let child_indentation = child.len() - trimmed.len();
                if child_indentation <= indentation {
                    break;
                }
                assert!(
                    !trimmed.starts_with("focused "),
                    "{name}: @control must inherit focus:border-ring: {child:?}"
                );
            }
        }
    }

    let view = include_str!("ui/view.ice");
    let shell = include_str!("ui/components/shell.ice");
    let chat = include_str!("ui/components/chat.ice");
    let pages = include_str!("ui/components/pages.ice");
    let kit = include_str!("ui/components/kit.ice");
    let forge = include_str!("ui/components/forge.ice");

    assert_recipe_owns_states("screens", SCREENS, "@primary_action");
    assert_recipe_owns_states("screens", SCREENS, "@danger_action");
    assert_recipe_owns_states("chat.ice", chat, "@danger_action");
    assert_recipe_owns_states("pages.ice", pages, "@danger_action");
    assert!(!SCREENS.contains("active bg=brand text=fg"));
    assert!(!SCREENS.contains("hovered bg=brand/10"));
    assert!(!SCREENS.contains("hovered bg=brand/12"));
    assert!(!SCREENS.contains("font=code @text-brand"));
    assert!(!chat.contains("bg=brand/10 border=brand/22"));
    assert!(!chat.contains("bg=brand/9 border=brand/20"));
    assert!(SCREENS.contains("Badge.Outline label=\"Members only\""));
    // a tracker row's kind is carried by the PLATE behind the glyph, not by
    // a second badge next to the state — one `match item.kind`, two plates.
    assert!(forge.contains(
        "match item.kind\n            \"pr\"\n              PrStatePlate state=item.state"
    ));
    assert!(forge.contains("IssueStateGlyph state=item.state"));
    assert!(!SCREENS.contains("Badge.Outline label=item.kind"));
    // a degraded node speaks the ALERT family, never a second red language:
    // the status dot and the banner share `alert_*`, and the healthy dot is
    // the same plate in `success_dot`.
    assert!(shell.contains("bg=success_dot r=(plate / 2.0)"));
    assert!(shell.contains("bg=alert_dot r=(plate / 2.0)"));
    assert!(shell.contains("bg=alert_bg border=alert_line"));
    assert!(shell.contains("bg=alert_dot r=3.5"));
    assert!(!shell.contains("danger_"));
    assert!(
        SCREENS.contains("KeyValueRow label=\"Key state\" value=settings_key_state last=false")
    );
    assert!(
        SCREENS.contains("KeyValueRow label=\"Key path\" value=settings_key_path last=true")
    );

    for target in [
        "rename_channel_submit",
        "add_channel_member_submit",
        "fs_mkdir_submit",
        "fs_new_file_submit",
        "gov_execute",
        "account_rename_submit",
    ] {
        let action = SCREENS
            .lines()
            .chain(include_str!("ui/components/kit.ice").lines())
            .find(|line| line.trim_start().starts_with("button ") && line.contains(target))
            .unwrap_or_else(|| panic!("missing action target {target}"));
        assert!(action.contains("@secondary_action"), "{action}");
    }
    let divider_actions: Vec<_> = SCREENS
        .lines()
        .filter(|line| line.contains("button \"Insert divider\""))
        .collect();
    assert_eq!(divider_actions.len(), 2);
    assert!(
        divider_actions
            .iter()
            .all(|line| line.contains("@secondary_action"))
    );

    assert_controls_inherit_focus("screens", SCREENS);
    assert_controls_inherit_focus("pages.ice", pages);
    // The three composer editors carried ad-hoc `focused border=ring`
    // status blocks; their focus ring now lives in the rich composer
    // adapter (`editor::composer_style`). One authored site remains.
    assert_eq!(
        SCREENS
            .lines()
            .filter(|line| line.trim_start().starts_with("focused ")
                && line.contains("border=ring"))
            .count(),
        1
    );
    assert_eq!(
        pages
            .matches("opened text=fg placeholder=muted handle=fg bg=fg/11 border=ring")
            .count(),
        2
    );
    assert!(!SCREENS.contains("selection=brand"));
    assert_eq!(
        SCREENS
            .matches("focused bg=transparent border=transparent value=transparent border-w=0.0")
            .count(),
        6
    );

    for binding in [
        "StatusBadge label=forge_item_state",
        "StatusBadge label=op.disposition",
    ] {
        assert!(SCREENS.contains(binding), "{binding}");
    }
    for mapping in [
        "\"active\"\n        Badge.Success label=label",
        "\"paused\"\n        Badge.Warning label=label",
        "\"open\"\n        Badge.Success label=label",
        "\"closed\"\n        Badge.Destructive label=label",
        "\"merged\"\n        Badge.Success label=label",
        "\"passed\"\n        Badge.Success label=label",
        "\"rejected\"\n        Badge.Destructive label=label",
        "\"applied\"\n        Badge.Success label=label",
        "\"discarded\"\n        Badge.Warning label=label",
    ] {
        // `StatusBadge` is the kit's now, so the state→badge table is read
        // where the component lives.
        assert!(kit.contains(mapping), "{mapping}");
    }
    assert!(SCREENS.contains("bg=danger_bg border=danger_line"));
    assert!(SCREENS.contains("bg=danger_dot"));
    assert!(SCREENS.contains("bg=success_dot"));
    // the semantic status plate is the kit's, so every screen that reports
    // a good outcome paints the same three tokens.
    assert!(kit.contains("bg=success_bg border=success_line border-w=1.0"));
    for source in [view, shell, chat, pages, kit, forge, SCREENS] {
        assert!(!source.contains("bg=success/"));
        assert!(!source.contains("border=success/"));
    }
}

/// App-authored text sizes stay on the app design scale, while the shared
/// Ice palette stays identical to the retained ducktape-ui theme.
#[test]
fn ice_sources_hold_to_the_design_system() {
    let sources = [
        ("view.ice", include_str!("ui/view.ice")),
        ("chat.ice", include_str!("ui/components/chat.ice")),
        ("dm.ice", include_str!("ui/components/dm.ice")),
        ("files.ice", include_str!("ui/components/files.ice")),
        ("forge.ice", include_str!("ui/components/forge.ice")),
        ("huddle.ice", include_str!("ui/components/huddle.ice")),
        ("icon.ice", include_str!("ui/components/icon.ice")),
        ("kit.ice", include_str!("ui/components/kit.ice")),
        ("node.ice", include_str!("ui/components/node.ice")),
        ("onboarding.ice", include_str!("ui/components/onboarding.ice")),
        ("overlay.ice", include_str!("ui/components/overlay.ice")),
        ("pages.ice", include_str!("ui/components/pages.ice")),
        ("patterns.ice", include_str!("ui/components/patterns.ice")),
        ("roster.ice", include_str!("ui/components/roster.ice")),
        ("shell.ice", include_str!("ui/components/shell.ice")),
        // the screens carry the console's authored type scale now that
        // view.ice holds only the mounts.
        ("screens", SCREENS),
    ];
    for (name, source) in sources {
        for line in source.lines() {
            for token in line.split_whitespace() {
                let Some(value) = token
                    .strip_prefix("size=")
                    .or_else(|| token.strip_prefix("text-size="))
                else {
                    continue;
                };
                let Ok(size) = value.parse::<f64>() else {
                    // a prop name, not a step — the literal is at the call site
                    continue;
                };
                assert!(
                    design::type_scale::ALL.contains(&size),
                    "{name}: {size} is off the design scale — change design::type_scale, not the view: {line:?}"
                );
            }
        }
    }
    // NO size→family/weight pairing is asserted, and that is a finding, not
    // an omission: the canonical artifact draws EVERY step of the scale in
    // both faces and at several weights (12.5px alone appears at 400, 500
    // and 600, and 11px splits 226/195 mono-vs-sans). A step therefore
    // fixes size and nothing else — a guard pinning `size=11.0` to
    // `font=code_medium` was describing an older, smaller app, not the
    // design system, and would reject correct markup on nine screens.

    // the font identity: theme roles bind to the design crate's families,
    // and the app embeds exactly the crate's font assets.
    let theme = include_str!("ui/theme.ice");
    assert!(theme.contains(&format!("family=\"{}\"", design::fonts::FAMILY_UI)));
    assert!(theme.contains(&format!("family=\"{}\"", design::fonts::FAMILY_MONO)));
    let app = include_str!("ui/app.ice");
    for asset in design::fonts::ASSETS {
        assert!(
            app.contains(&format!("font \"../../../crates/design/{asset}\"")),
            "app.ice must embed {asset}"
        );
    }
    assert!(app.contains(&format!("text-size {}", design::type_scale::BODY)));

    let palette = ducktape_ui::ui::theme::LIGHT.palette;
    for (token, color) in [
        ("bg", palette.background),
        ("surface", palette.card),
        ("fg", palette.foreground),
        ("muted", palette.muted_foreground),
        ("muted_bg", palette.muted),
        ("primary", palette.primary),
        ("primary_fg", palette.primary_foreground),
        ("secondary", palette.secondary),
        ("secondary_fg", palette.secondary_foreground),
        ("accent", palette.accent),
        ("accent_fg", palette.accent_foreground),
        ("brand", palette.brand),
        ("brand_fg", palette.brand_foreground),
        ("brand_bg", palette.brand_background),
        ("brand_line", palette.brand_line),
        ("danger", palette.destructive),
        ("danger_fg", palette.destructive_foreground),
        ("danger_bg", palette.destructive_background),
        ("danger_line", palette.destructive_line),
        ("danger_dot", palette.destructive_dot),
        ("success", palette.success),
        ("success_fg", palette.success_foreground),
        ("success_bg", palette.success_background),
        ("success_line", palette.success_line),
        ("success_dot", palette.success_dot),
        ("warning", palette.warning),
        ("warning_fg", palette.warning_foreground),
        ("warning_bg", palette.warning_background),
        ("warning_line", palette.warning_line),
        ("warning_dot", palette.warning_dot),
        ("avatar_bg", palette.avatar),
        ("avatar_fg", palette.avatar_foreground),
        ("toast_bg", palette.toast_background),
        ("toast_fg", palette.toast_foreground),
        ("border", palette.border),
        ("control_line", palette.control_line),
        ("input", palette.input),
        ("ring", palette.ring),
    ] {
        assert_eq!(default_ice_color(token), color, "{token}");
    }
}

/// Text painted on a status fill must stay readable in BOTH themes.
///
/// `destructive` and `warning` do not invert between light and dark, they
/// SHIFT — and a foreground that does not shift with them loses contrast as
/// they do. The old React console painted a hardcoded `#fff` on them and
/// measured 3.62:1 in dark (#459). The palette now carries a real
/// `*_foreground` for each fill, which is the right shape; this asserts the
/// VALUES actually clear AA, because nothing else would notice a palette
/// that stopped clearing it.
///
/// That matters here specifically because the palette is vendored from
/// another repo by git `rev`: a routine rev bump could darken a fill with no
/// review in this tree at all.
#[test]
fn every_status_fill_carries_a_readable_foreground_in_both_themes() {
    /// WCAG 2.1 relative luminance — the sRGB channel transfer, then the
    /// standard weights.
    fn luminance(color: iced::Color) -> f32 {
        fn channel(c: f32) -> f32 {
            match c <= 0.039_28 {
                true => c / 12.92,
                false => ((c + 0.055) / 1.055).powf(2.4),
            }
        }
        0.2126 * channel(color.r) + 0.7152 * channel(color.g) + 0.0722 * channel(color.b)
    }
    fn contrast(a: iced::Color, b: iced::Color) -> f32 {
        let (x, y) = (luminance(a), luminance(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }
    /// AA for small text. Every one of these fills carries body-size text.
    const AA_SMALL: f32 = 4.5;

    for (theme_name, theme) in [
        ("light", ducktape_ui::ui::theme::LIGHT),
        ("dark", ducktape_ui::ui::theme::DARK),
    ] {
        let p = theme.palette;
        for (fill_name, fill, foreground) in [
            ("destructive", p.destructive, p.destructive_foreground),
            ("warning", p.warning, p.warning_foreground),
            ("success", p.success, p.success_foreground),
            ("primary", p.primary, p.primary_foreground),
            ("brand", p.brand, p.brand_foreground),
            ("accent", p.accent, p.accent_foreground),
            ("secondary", p.secondary, p.secondary_foreground),
            ("toast", p.toast_background, p.toast_foreground),
        ] {
            let ratio = contrast(fill, foreground);
            assert!(
                ratio >= AA_SMALL,
                "{theme_name}/{fill_name}: {ratio:.2}:1 is below WCAG AA {AA_SMALL}:1 — \
                 a fill that shifts needs a foreground that shifts with it"
            );
        }
    }
}

// The Enter/Shift+Enter send contract moved with the binding: it lives in
// `editor::tests::plain_enter_submits_and_shift_enter_edits`, against the
// classify seam the rich composer actually routes through.

/// The artifact hangs comments off the document as a docked 306px rail on
/// the sidebar ladder, NOT as a floating card over it — a card would cover
/// the block it is about the moment the block sits on the right half.
#[test]
fn block_comments_dock_a_rail_beside_the_document() {
    // the pages screen is its own file now, so the slot slicing is gone.
    let pages = include_str!("ui/screens/pages.ice");
    // the rail is a sibling of the document, separated by the same 1px rule
    // every other docked column uses — never an overlay layer.
    let rail = pages
        .split_once("if connected && !empty(active_page) && block_comments_open\n")
        .unwrap()
        .1;
    let mut opening = rail.lines().map(str::trim);
    assert_eq!(opening.next(), Some("box w=1.0 h=fill bg=separator"));
    assert_eq!(opening.next(), Some("space w=1.0 h=1.0"));
    assert_eq!(
        opening.next(),
        Some("box w=306.0 h=fill bg=sidebar clip=true")
    );
    assert!(!pages.contains("close_block_comments backdrop=transparent"));
    assert!(pages.contains("-> emit(close_block_comments)"));
    assert!(pages.contains("#block-comment(scope_key(connected_rpc, selected_block_id))"));
    assert!(!pages.contains("button \"Save\""));
    assert!(!pages.contains("Saving"));

    let components = include_str!("ui/components/pages.ice");
    assert!(components.contains("label=\"Comments\""));
    assert!(components.contains("-> emit(open_block_comments)"));

    let handlers = include_str!("ui/handlers/pages.ice");
    assert!(handlers.contains("on post_block_comment_submit"));
    assert!(handlers.contains(
        "run post_block_comment(connected_rpc, password, block_comments_target, active_block_comment_thread"
    ));
}

#[test]
fn selecting_another_block_discards_a_stale_comment_page() {
    let (mut app, _) = Ducktape::__boot();
    app.mutation_phase = "idle".into();
    let _ = app.__update(__DucktapeMessage::SelectBlock(
        0,
        "block-a".into(),
        "Text".into(),
        "A".into(),
        false,
        false,
    ));
    let _ = app.__update(__DucktapeMessage::OpenBlockComments);
    let stale_generation = app.block_comments_generation;

    let _ = app.__update(__DucktapeMessage::SelectBlock(
        1,
        "block-b".into(),
        "Text".into(),
        "B".into(),
        false,
        false,
    ));
    let _ = app.__update(__DucktapeMessage::BlockThreadsLoaded(
        backend::BlockThreadListData {
            generation: stale_generation,
            target: "block-a".into(),
            from: 0,
            threads: vec![backend::PageCommentThread {
                id: "thread-a".into(),
                author: "Alice".into(),
                meta: "1".into(),
                resolved: false,
                comment_count: 1,
            }],
            total: 1,
            next_from: 0,
            has_more: false,
        },
    ));

    assert_eq!(app.selected_block_id, "block-b");
    assert!(!app.block_comments_open);
    assert!(app.block_comment_threads.is_empty());
}

#[test]
fn comment_pages_merge_by_identity_and_ordinal() {
    let thread = |id: &str, count: i64| backend::PageCommentThread {
        id: id.into(),
        author: "user".into(),
        meta: count.to_string(),
        resolved: false,
        comment_count: count,
    };
    let comment = |ordinal, text: &str| backend::PageComment {
        id: format!("comment-{ordinal}"),
        ordinal,
        author: "user".into(),
        meta: format!("#{ordinal}"),
        text: text.into(),
    };

    let threads = backend::append_page_comment_threads(
        vec![thread("b", 1), thread("a", 1)],
        vec![thread("b", 2), thread("c", 1)],
    );
    assert_eq!(
        threads
            .iter()
            .map(|thread| thread.id.as_str())
            .collect::<Vec<_>>(),
        ["a", "b", "c"]
    );
    assert_eq!(threads[1].comment_count, 2);

    let comments = backend::append_page_comments(
        vec![comment(1, "first"), comment(3, "old")],
        vec![comment(2, "second"), comment(3, "new")],
    );
    assert_eq!(
        comments
            .iter()
            .map(|comment| comment.ordinal)
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(comments[2].text, "new");
}

#[test]
fn live_comment_refresh_updates_threads_without_touching_the_draft() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.mutation_phase = "idle".into();
    app.selected_block_id = "block-1".into();
    app.active_page = "page".into();
    app.block_comments_open = true;
    // the rail is DOCUMENT-scoped: its anchor is the page it was opened
    // on, never the block selection that opened it.
    app.block_comments_target = "page".into();
    app.block_comment_draft = "draft stays".into();
    app.block_comment_threads_has_more = true;
    app.active_block_comment_thread = "deleted-thread".into();
    app.block_thread_comments = vec![backend::PageComment {
        id: "stale-comment".into(),
        ordinal: 1,
        author: "user".into(),
        meta: "#1".into(),
        text: "stale".into(),
    }];

    // a pages comment op arrives: the delta starts the debounced reload
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: "pages".into(),
        status: "Live".into(),
        height: 8,
        load_pages: true,
        debounce: true,
        pages: backend::PagesDelta {
            kind: "touched".into(),
            comments: true,
        },
        ..backend::LiveUpdate::default()
    }));
    let resync_generation = app.hydration_generation;
    let stale_generation = app.block_comments_generation;
    let _ = app.__update(__DucktapeMessage::LoadMoreBlockThreads);
    assert_ne!(app.block_comments_generation, stale_generation);

    // a comment refresh from a superseded generation is dropped whole
    let _ = app.__update(__DucktapeMessage::BlockThreadsLoaded(
        backend::BlockThreadListData {
            generation: stale_generation,
            target: "page".into(),
            from: 0,
            threads: Vec::new(),
            total: 0,
            next_from: 0,
            has_more: false,
        },
    ));
    assert_eq!(app.block_comment_draft, "draft stays");

    // the scoped reload lands and re-arms the comment refresh
    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        resync_generation,
        "",
        Vec::new(),
        "page",
        vec![backend::PageBlock {
            key: 0,
            id: "block-1".into(),
            parent: "page".into(),
            kind: "Text".into(),
            text: "block".into(),
            pending: false,
            checked: false,
            prefix: String::new(),
            child_count: 0,
            mark_count: 0,
            spans: Vec::new(),
        }],
    )));
    let generation = app.block_comments_generation;

    let _ = app.__update(__DucktapeMessage::BlockThreadsLoaded(
        backend::BlockThreadListData {
            generation,
            target: app.block_comments_target.clone(),
            from: 0,
            threads: vec![backend::PageCommentThread {
                id: "thread-1".into(),
                author: "user".into(),
                meta: "1".into(),
                resolved: false,
                comment_count: 1,
            }],
            total: 3,
            next_from: 0,
            has_more: false,
        },
    ));

    assert_eq!(app.block_comment_thread_total, 3);
    assert_eq!(app.block_comment_draft, "draft stays");
    assert!(!app.block_comment_threads_loading);
    // the live refresh carries the THREAD LIST only. An open comment page
    // is not reloaded under the reader — a task group must be a handler's
    // final statement, so the reply load cannot be guarded on an open
    // thread, and firing it unguarded queries thread "" and paints its
    // failure over the rail on every page edit. Replies arrive on post and
    // on reopen instead.
    assert_eq!(app.active_block_comment_thread, "deleted-thread");
    assert_eq!(app.block_thread_comments.len(), 1);
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
            next_reply_offset: 0,
            has_more: false,
        },
    ));
    assert!(!app.thread_loading);
    assert_eq!(app.hydration_generation, 9);
}

#[test]
fn block_comment_recovery_always_unlocks_mutations() {
    let (mut failed, _) = Ducktape::__boot();
    failed.block_comments_open = true;
    failed.block_comments_generation = 7;
    failed.block_comment_threads_loading = true;
    failed.mutation_phase = "recovering".into();
    let _ = failed.__update(__DucktapeMessage::BlockThreadsRecoveryFailed(
        backend::HydrationError {
            generation: 7,
            message: "recovery read failed".into(),
        },
    ));
    assert_eq!(failed.mutation_phase, "idle");
    assert!(!failed.block_comment_threads_loading);

    let (mut recovered, _) = Ducktape::__boot();
    recovered.block_comments_open = true;
    recovered.block_comments_target = "block-1".into();
    recovered.selected_block_id = "block-1".into();
    recovered.block_comments_generation = 8;
    recovered.block_comment_threads_loading = true;
    recovered.mutation_phase = "recovering".into();
    recovered.error = "write result was uncertain".into();
    let _ = recovered.__update(__DucktapeMessage::BlockThreadsRecovered(
        backend::BlockThreadListData {
            generation: 8,
            target: "block-1".into(),
            from: 0,
            threads: Vec::new(),
            total: 0,
            next_from: 0,
            has_more: false,
        },
    ));
    assert_eq!(recovered.mutation_phase, "idle");
    assert!(recovered.error.is_empty());
}

#[test]
fn page_navigation_ignores_the_previous_block_autosave_callback() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.active_page = "old-page".into();
    app.block_autosave_generation = 4;

    let _ = app.__update(__DucktapeMessage::ChoosePage("next-page".into()));
    let navigation_generation = app.hydration_generation;
    assert_eq!(app.block_autosave_generation, 5);

    let _ = app.__update(__DucktapeMessage::BlockTextSaved(backend::AutosaveResult {
        generation: 4,
        written: true,
    }));
    assert_eq!(app.hydration_generation, navigation_generation);
    assert!(app.loading);
}

#[test]
fn block_edits_invalidate_an_older_resync() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.connected_rpc = "http://node".into();
    app.selected_block_id = "block-1".into();
    app.selected_block_kind = "Text".into();
    app.block_editor = compose("new");
    app.selected_block_saved_text = "old".into();
    app.hydration_generation = 3;

    let _ = app.__update(__DucktapeMessage::BlockAutosaveTick);
    assert_eq!(app.hydration_generation, 4);

    // a stale resync from before the edit cannot roll the draft back
    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        3,
        "",
        Vec::new(),
        "page",
        vec![backend::PageBlock {
            key: 0,
            id: "block-1".into(),
            parent: "page".into(),
            kind: "Text".into(),
            text: "old".into(),
            pending: false,
            checked: false,
            prefix: String::new(),
            child_count: 0,
            mark_count: 0,
            spans: Vec::new(),
        }],
    )));
    assert_eq!(block_editor_text(&app), "new");
}

#[test]
fn remote_block_deletion_recovers_local_drafts_and_closes_the_editor() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.connected_rpc = "http://node".into();
    app.hydration_generation = 4;
    app.selected_block_id = "deleted".into();
    app.selected_block_kind = "Text".into();
    app.block_editor = compose("unfinished block");
    app.block_autosave_status = "error".into();
    app.block_autosave_generation = 8;
    app.block_comments_open = true;
    app.block_comments_target = "page".into();
    app.block_comment_draft = "unfinished comment".into();
    app.active_page = "page".into();
    app.page_delete_armed = true;
    app.page_title_selected = true;

    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        4,
        "",
        Vec::new(),
        "page",
        Vec::new(),
    )));

    assert_eq!(app.orphaned_block_drafts, ["unfinished block"]);
    assert!(app.selected_block_id.is_empty());
    assert!(block_editor_text(&app).is_empty());
    // the comment rail hangs off the PAGE, and the page survived — so the
    // rail and its half-typed comment survive a block deletion. Only a
    // page change closes it. (The orphan LIST is not asserted here: the
    // resync still keys `remember_orphaned_comment_drafts` on
    // `selected_block_id`, so this same live draft is also filed as
    // recovered — see the report.)
    assert_eq!(app.block_comment_draft, "unfinished comment");
    assert!(app.block_comments_open);
    assert_eq!(app.block_autosave_generation, 9);
    assert!(app.page_delete_armed);
    assert!(app.page_title_selected);

    let _ = app.__update(__DucktapeMessage::UseOrphanedBlockDraft(
        "unfinished block".into(),
    ));
    assert_eq!(app.block_draft, "unfinished block");
    assert!(app.orphaned_block_drafts.is_empty());
}

#[test]
fn live_thread_refresh_preserves_the_reply_draft_and_rejects_stale_results() {
    let (mut app, _) = Ducktape::__boot();
    app.active_channel = "general".into();
    app.active_thread_seq = 7;
    app.thread_target_seq = 9;
    app.live_thread_generation = 3;
    app.reply_editor = compose("typing");
    app.thread_messages = backend::optimistic_message(
        backend::optimistic_message(Vec::new(), "pending first".into(), "pending-first".into()),
        "pending second".into(),
        "pending-second".into(),
    );

    let _ = app.__update(__DucktapeMessage::LiveThreadRefreshed(
        backend::LiveThreadData {
            generation: 3,
            channel_id: "other".into(),
            root_seq: 7,
            target_seq: 0,
            messages: Vec::new(),
            next_reply_offset: 99,
            has_more: true,
        },
    ));
    assert_eq!(app.thread_next_reply_offset, 0);

    let _ = app.__update(__DucktapeMessage::LiveThreadRefreshed(
        backend::LiveThreadData {
            generation: 3,
            channel_id: "general".into(),
            root_seq: 7,
            target_seq: 0,
            messages: Vec::new(),
            next_reply_offset: 5,
            has_more: true,
        },
    ));
    assert_eq!(reply_composer(&app), "typing");
    assert_eq!(app.thread_target_seq, 0);
    assert_eq!(app.thread_next_reply_offset, 5);
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
            generation: 3,
            channel_id: "general".into(),
            root_seq: 7,
            target_seq: 9,
            messages: Vec::new(),
            next_reply_offset: 99,
            has_more: true,
        },
    ));
    assert_eq!(app.thread_next_reply_offset, 0);
    assert!(!app.thread_has_more);
}

#[test]
fn reconnect_recovers_active_drafts_for_the_same_endpoint() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.rpc = "http://node".into();
    app.connected_rpc = "http://node".into();
    app.selected_block_id = "block".into();
    app.block_editor = compose("unfinished block");
    app.block_autosave_status = "saving".into();
    app.block_comment_draft = "unfinished comment".into();

    let _ = app.__update(__DucktapeMessage::Reconnect);

    assert_eq!(app.orphaned_block_drafts, ["unfinished block"]);
    assert_eq!(app.orphaned_comment_drafts, ["unfinished comment"]);
}

#[test]
fn page_and_block_context_changes_recover_local_drafts() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.connected_rpc = "http://draft-context-test".into();
    app.selected_block_id = "block-a".into();
    app.selected_block_kind = "Text".into();
    app.block_editor = compose("failed edit");
    app.block_autosave_status = "error".into();
    app.block_comment_draft = "comment a".into();

    let _ = app.__update(__DucktapeMessage::SelectBlock(
        0,
        "block-b".into(),
        "Text".into(),
        "canonical b".into(),
        false,
        false,
    ));
    assert_eq!(app.orphaned_block_drafts, ["failed edit"]);
    assert_eq!(app.orphaned_comment_drafts, ["comment a"]);
    assert_eq!(block_editor_text(&app), "canonical b");

    app.block_comment_draft = "comment b".into();
    let _ = app.__update(__DucktapeMessage::CloseBlockComments);
    assert_eq!(app.orphaned_comment_drafts, ["comment a", "comment b"]);

    app.block_editor = compose("saving b");
    app.block_autosave_status = "saving".into();
    let _ = app.__update(__DucktapeMessage::ChoosePage("next".into()));
    assert_eq!(app.orphaned_block_drafts, ["failed edit", "saving b"]);
    assert!(block_editor_text(&app).is_empty());
    assert!(app.selected_block_id.is_empty());
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
    assert_eq!(app.mutation_phase, "idle");
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
    assert_eq!(app.mutation_phase, "idle");

    app.message_editor = compose("still available");
    let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
        editor::composer_submit_event(),
    ));
    assert_eq!(app.messages.len(), 2);
    assert_eq!(app.mutation_phase, "idle");
}

#[test]
fn committed_message_change_cannot_be_submitted_twice() {
    let (mut app, _) = Ducktape::__boot();
    app.connected_rpc = "http://node".into();
    app.active_channel = "general".into();
    app.selected_message_seq = 7;
    app.selected_message_rev = 2;
    app.message_action = "editing".into();
    app.message_edit_draft = "committed edit".into();
    app.mutation_phase = "message-edit".into();

    let _ = app.__update(__DucktapeMessage::MutationFailed(backend::AppError {
        message: "read failed after commit".into(),
        committed: true,
    }));

    assert_eq!(app.selected_message_seq, 0);
    assert_eq!(app.selected_message_rev, 0);
    assert_eq!(app.message_action, "toolbar");
    assert!(app.message_edit_draft.is_empty());
    assert_eq!(app.mutation_phase, "recovering");
}

#[test]
fn optimistic_thread_replies_settle_independently_out_of_order() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_channel = "general".into();
    app.active_thread_seq = 1;
    app.reply_editor = compose("first");

    let _ = app.__update(__DucktapeMessage::ReplyComposerEvent(
        editor::composer_submit_event(),
    ));
    let first_id = app.thread_messages[0].id.clone();
    assert_eq!(app.mutation_phase, "idle");
    assert!(reply_composer(&app).is_empty());
    assert!(app.thread_messages[0].pending);

    app.reply_editor = compose("second");
    let _ = app.__update(__DucktapeMessage::ReplyComposerEvent(
        editor::composer_submit_event(),
    ));
    let second_id = app.thread_messages[1].id.clone();
    assert_ne!(first_id, second_id);
    assert_eq!(app.thread_messages.len(), 2);
    assert!(app.thread_messages.iter().all(|message| message.pending));

    let mut second = message(3, "second", false);
    second.id = second_id.clone();
    second.thread_seq = 1;
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: "chat".into(),
        status: "Live".into(),
        height: 3,
        chat: backend::ChatDelta {
            kind: "reply".into(),
            channel_id: "general".into(),
            seq: 3,
            root_seq: 1,
            message: second,
            ..backend::ChatDelta::default()
        },
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
        kind: "chat".into(),
        status: "Live".into(),
        height: 4,
        chat: backend::ChatDelta {
            kind: "reply".into(),
            channel_id: "general".into(),
            seq: 2,
            root_seq: 1,
            message: first,
            ..backend::ChatDelta::default()
        },
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
    app.reply_editor = compose("first");

    let _ = app.__update(__DucktapeMessage::ReplyComposerEvent(
        editor::composer_submit_event(),
    ));
    let first_id = app.thread_messages[0].id.clone();
    app.reply_editor = compose("second");
    let _ = app.__update(__DucktapeMessage::ReplyComposerEvent(
        editor::composer_submit_event(),
    ));
    let second_id = app.thread_messages[1].id.clone();
    app.reply_editor = compose("newer draft");

    let _ = app.__update(__DucktapeMessage::ThreadReplySendFailed(
        backend::OptimisticMutationError {
            message: "rejected".into(),
            committed: false,
            operation_id: first_id,
            scope_id: "general".into(),
            body: "first".into(),
        },
    ));
    assert_eq!(reply_composer(&app), "newer draft");
    assert_eq!(app.failed_reply_draft, "first");
    assert_eq!(app.thread_messages.len(), 1);
    assert_eq!(app.thread_messages[0].id, second_id);
    assert!(app.thread_messages[0].pending);
    assert_eq!(app.mutation_phase, "idle");
    assert!(!app.thread_loading);

    let _ = app.__update(__DucktapeMessage::RestoreFailedReply);
    assert_eq!(reply_composer(&app), "newer draft");
    assert_eq!(app.failed_reply_draft, "first");
    app.reply_editor = compose("");
    let _ = app.__update(__DucktapeMessage::RestoreFailedReply);
    assert_eq!(reply_composer(&app), "first");
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
    app.reply_editor = compose("committed");

    let _ = app.__update(__DucktapeMessage::ReplyComposerEvent(
        editor::composer_submit_event(),
    ));
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
    assert!(reply_composer(&app).is_empty());
    assert!(app.failed_reply_draft.is_empty());
    assert_eq!(app.mutation_phase, "idle");
    assert!(!app.thread_loading);

    app.reply_editor = compose("still available");
    let _ = app.__update(__DucktapeMessage::ReplyComposerEvent(
        editor::composer_submit_event(),
    ));
    assert_eq!(app.thread_messages.len(), 2);
    assert!(app.thread_messages.iter().all(|message| message.pending));
    assert!(reply_composer(&app).is_empty());
}

#[test]
fn failed_block_insert_rolls_back_and_restores_the_draft() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_page = "welcome".into();
    app.new_block_kind = "Heading 2".into();
    app.block_draft = "retry heading".into();

    let _ = app.__update(__DucktapeMessage::AddBlockSubmit);
    let operation_id = app.blocks[0].id.clone();
    assert_eq!(app.mutation_phase, "idle");
    assert!(app.block_draft.is_empty());
    assert_eq!(app.blocks[0].kind, "Heading 2");
    assert!(app.blocks[0].pending);

    let _ = app.__update(__DucktapeMessage::BlockAddFailed(
        backend::OptimisticMutationError {
            message: "rejected".into(),
            committed: false,
            operation_id,
            scope_id: "welcome".into(),
            body: "retry heading".into(),
        },
    ));
    assert_eq!(app.block_draft, "retry heading");
    assert!(app.orphaned_block_drafts.is_empty());
    assert!(app.blocks.is_empty());
    assert_eq!(app.mutation_phase, "idle");
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
    let _ = app.__update(__DucktapeMessage::ChatUpdated(switched));
    assert_eq!(app.active_channel, "random");
    assert_eq!(app.unread_boundary, 30);
    assert_eq!(
        backend::first_unread_seq(app.messages.clone(), app.unread_boundary),
        31
    );
    assert!(!backend::channel_is_unread(
        app.channel_reads.clone(),
        "random".into(),
        50
    ));

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
    let _ = app.__update(__DucktapeMessage::ChatUpdated(caught_up));
    assert_eq!(app.active_channel, "general");
    assert_eq!(app.unread_boundary, 0);
}

#[test]
fn unread_indicators_are_wired_client_local_only() {
    // Sidebar badge: ChannelButton takes an `unread` flag and paints the
    // brand treatment + dot when set.
    let components = include_str!("ui/components/chat.ice");
    assert!(
        components.contains(
            "component ChannelButton(channel:ChatChannel, selected:bool, unread:bool)"
        )
    );
    assert!(components.contains("if unread\n                box w=7.0 h=7.0 bg=brand r=3.5"));
    assert!(components.contains(
        "if unread\n                text channel.name w=fill size=13.0 wrap=none font=medium @text-fg"
    ));

    let screen = include_str!("ui/screens/chat.ice");
    assert!(screen.contains(
        "ChannelButton channel=channel selected=(channel.id == active_channel) unread=channel_is_unread(channel_reads, channel.id, channel.head_seq)"
    ));
    // In-channel divider anchored on the first message past the frozen
    // boundary. The seq is a STATE FIELD recomputed where messages or the
    // boundary change — `first_unread_seq(messages, …)` in the view sat
    // inside `for message in messages`, and the extern's by-value ABI deep-
    // cloned the whole timeline once per row per frame.
    assert!(screen.contains("if unread_boundary > 0 && message.seq == unread_marker_seq"));
    assert!(!screen.contains("first_unread_seq("));
    assert!(screen.contains("text \"New messages\" size=12.5 wrap=none @text-brand"));

    // Freeze happens on a real channel change; connect seeds caught-up.
    let lifecycle = include_str!("ui/handlers/lifecycle.ice");
    assert!(
        lifecycle
            .contains("channel_reads = initial_channel_reads(next.channels, channel_reads)")
    );
    // navigation loads freeze on the real channel change (chat.ice);
    // the resync path freezes against the possibly-unchanged channel.
    let chat = include_str!("ui/handlers/chat.ice");
    assert!(chat.contains(
        "unread_boundary = frozen_unread_boundary(channel_reads, next.channels, active_channel, next.active_channel, unread_boundary)"
    ));
    assert!(chat.contains(
        "channel_reads = mark_channel_read(channel_reads, next.active_channel, channel_head_seq(next.channels, next.active_channel))"
    ));
    assert!(lifecycle.contains(
        "unread_boundary = frozen_unread_boundary(channel_reads, channels, active_channel, active_channel, unread_boundary)"
    ));
    assert!(lifecycle.contains(
        "channel_reads = mark_channel_read(channel_reads, active_channel, channel_head_seq(channels, active_channel))"
    ));

    // Client-local only: no wire read-cursor leaked into the module surface.
    let backend_ice = include_str!("ui/extern/backend.ice");
    assert!(!backend_ice.contains("read_cursor"));
    assert!(!backend_ice.contains("mark_read(rpc"));
}
