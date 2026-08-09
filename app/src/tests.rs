// The app's update-loop and source-sweep suite. `super` is the crate
// root, so the generated `Ducktape` app and both native modules resolve
// exactly as they did when this mod lived inline in main.rs.
use super::*;

/// EVERY SCREEN BODY, as one string. These are the slot bodies that used to
/// sit inline in `view.ice`; the sweeps below read the console's authored
/// markup, so they must read where that markup now lives. `view.ice` keeps
/// only the mounts, and asserting a widget shape against it now would pass
/// vacuously — the worst kind of green.
static SCREENS: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    inlined(concat!(
        include_str!("ui/screens/chat.ice"),
        include_str!("ui/screens/forge.ice"),
        include_str!("ui/screens/governance.ice"),
        include_str!("ui/screens/overlays.ice"),
        include_str!("ui/screens/pages.ice"),
        include_str!("ui/screens/roster.ice"),
        include_str!("ui/screens/settings.ice"),
        include_str!("ui/screens/storage.ice"),
    ))
});

/// Fold `with` blocks back onto their node line, so the source sweeps keep
/// pinning a node and its props as ONE readable line no matter how
/// `cargo ice fmt` wrapped it — and so `!contains` sweeps stay falsifiable
/// instead of passing vacuously against wrapped text. Props keep source
/// order; a trailing `-> route` stays last.
fn inlined(source: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut lines = source.lines().peekable();
    while let Some(line) = lines.next() {
        let indent = line.len() - line.trim_start().len();
        if line.trim() == "with" && !out.is_empty() {
            let mut props = Vec::new();
            while let Some(next) = lines.peek() {
                let deeper = next.len() - next.trim_start().len() > indent;
                if next.trim().is_empty() || !deeper {
                    break;
                }
                props.push(next.trim().to_owned());
                lines.next();
            }
            let node = out.pop().expect("with follows its node line");
            let props = props.join(" ");
            out.push(match node.split_once(" -> ") {
                Some((head, route)) => format!("{head} {props} -> {route}"),
                None => format!("{node} {props}"),
            });
            continue;
        }
        out.push(line.to_owned());
    }
    out.join("\n")
}

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

/// The page document's text, the way the save tick reads it.
fn page_document_text(app: &Ducktape) -> String {
    crate::pages::page_text(app.page_editor.clone())
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
            let huddle = iced::window::Id::unique();
            app.huddle_win = Some(huddle);
            let _ = app.__view(huddle);
        })
        .unwrap()
        .join()
        .unwrap();
}

fn default_ice_color(name: &str) -> iced::Color {
    // 2.0 allows ONE theme contract and one palette, so the kit's theme moved
    // out of the vendored copy into the app's own file.
    let source = inlined(include_str!("ui/theme.ice"));
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
    let value =
        u32::from_str_radix(hex, 16).expect("default Ice colors are valid hexadecimal literals");
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
        comment_thread_total: 0,
        commented_block_hits: Vec::new(),
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
    assert_eq!(
        app.error, "",
        "the !connected early return must still clear"
    );

    // the chat/pages path returns second, and must still clear.
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.error = "files: path not found".into();
    let _ = app.__update(__DucktapeMessage::SelectShellTab("pages".into()));
    assert_eq!(
        app.error, "",
        "the chat/pages early return must still clear"
    );

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
            // NO video clock here, on purpose: the tile strip is a
            // self-redrawing widget that repaints only its own window at
            // the capture cadence. A reintroduced video tick would rebuild
            // EVERY window's view tree per beat — fail the build instead.
            "every 1s when huddle_joined -> tick",
            // the toast's dismissal clock: fine ticks against a per-toast
            // age, so a toast raised late in the old shared 2800ms window
            // no longer flashes and vanishes. Still gated on a visible
            // toast — it costs nothing at rest.
            "every 300ms when !empty(toast) -> toast_tick",
            // the settle-✓'s dismissal clock: two beats — hold + fade, then
            // unmount. Gated on the flash anchor, so it exists only for the
            // seconds after one of OUR sends settles.
            "every 1200ms when (!empty(send_flash_id)) || (!empty(thread_send_flash_id)) -> send_flash_tick",
            // the block editor's autosave clock: the stock editor's edits
            // never pass through a handler, so a dirty buffer is the only
            // signal there is — and the gate IS the dirty test, so the tick
            // exists solely while unsaved text needs the node. It costs
            // nothing at rest and dies the moment the save lands.
            // the page document's write gate: dirty IS the condition, so the
            // tick exists only while the buffer has drifted from the node's
            // text — not a poll, an edit-driven flush.
            "every 900ms when (connected && !empty(active_page) && page_text(page_editor) != page_saved_text) -> page_autosave_tick",
        ]
    );
}

#[test]
fn forge_depth_rides_the_established_seams() {
    // the forge handlers moved out of lifecycle.ice into their own file;
    // the seams they guard did not, so the guard reads both.
    let lifecycle = inlined(concat!(
        include_str!("ui/handlers/lifecycle.ice"),
        include_str!("ui/handlers/forge.ice"),
    ));
    let forge = inlined(include_str!("ui/components/forge.ice"));
    let backend = inlined(include_str!("ui/extern/backend.ice"));

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
    assert_no_polling(&lifecycle);

    // approvals stay advisory in the merge box — `MergeAdvisory` is the
    // ONLY thing said above the merge button, and it recommends, never
    // refuses. The merged state renders the CAS'd commit.
    let forge_screen = inlined(include_str!("ui/screens/forge.ice"));
    assert!(forge_screen.contains("MergeAdvisory change_requests=forge_item_change_requests"));
    assert_eq!(forge.matches("merge not recommended").count(), 2);
    // MergeAdvisory owns the count: no OTHER predicate may branch on it.
    // The one sibling read is the disclaimer's `<= 0`, which is the
    // no-advisory half and cannot contradict it.
    assert!(!forge_screen.contains("forge_item_change_requests > 0"));
    assert_eq!(
        forge_screen
            .matches("forge_item_change_requests <= 0")
            .count(),
        1
    );
    assert!(forge_screen.contains("forge_merge_note(forge_item_merge_oid, forge_item_branches)"));
}

#[test]
fn background_refresh_preserves_editing_state() {
    let root = inlined(include_str!("ui/app.ice"));
    let view = inlined(include_str!("ui/view.ice"));
    let lifecycle = inlined(include_str!("ui/handlers/lifecycle.ice"));
    assert!(!view.contains("sync_phase"));
    assert!(root.contains("use \"view.ice\""));
    assert!(!lifecycle.contains("on refresh_now"));
    // live surfaces (chat/pages) never need a manual refresh — the delta
    // stream keeps them current. The explorer's recent-window reload is
    // the one legitimate refresh affordance.
    let before_explorer = view
        .split_once("    explorer:")
        .map_or(view.as_str(), |(head, _)| head);
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
    assert_no_polling(&lifecycle);
    assert!(lifecycle.contains("run live_resync_load(connected_rpc"));
    assert!(lifecycle.contains("run refresh_live_thread(connected_rpc"));
    assert!(lifecycle.contains("parallel\n    run refresh_live_thread("));
    // Page-scoped state waits for a reply that answers for the page in hand —
    // a resync issued before a mutation moved the selection speaks for a
    // document nobody is on.
    assert!(lifecycle.contains(
        "active_page_title = keep_str(pages_answer_is_current, next.active_page_title, active_page_title)"
    ));
    assert!(lifecycle.contains(
        "pages_answer_is_current = next.pages_loaded && pages_reply_answers_current(next.pages, next.active_page, active_page)"
    ));
    // The page LIST is never stale — it is the whole index either way.
    assert!(lifecycle.contains("pages = keep_pages(next.pages_loaded, next.pages, pages)"));
    // A live resync must never install remote text over a buffer the user is
    // still typing in; the buffer and its dirty baseline move on ONE decision.
    assert!(lifecycle.contains("page_editor = refreshed_page_editor("));
    assert!(lifecycle.contains("page_saved_text = resynced_saved"));
    // the comment rail is scoped to the PAGE it hangs off, so its draft
    // survives moving the cursor between blocks and dies with the page.
    assert!(lifecycle.contains(
        "block_comment_draft = retain_selected_string(block_comment_draft, block_comments_target)"
    ));
    // the live comment-list callback settles state and stops — re-entering
    // the resync from inside it would loop the rail against the page.
    let pages_handlers = inlined(include_str!("ui/handlers/pages.ice"));
    let comment_callbacks = pages_handlers
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
    let pages = inlined(include_str!("ui/handlers/pages.ice"));
    // The page BODY is no longer among the drafts to rescue: it is one buffer
    // that flushes to the node on its own tick and is reinstalled from the
    // node's text on the next load. A half-typed COMMENT still has nowhere
    // else to live, so every context-destroying handler still guards it.
    for name in [
        "open_page_search_hit(page_id, _block_id)",
        "choose_page(id)",
        "toggle_block_comments",
        "pages_mutated(next)",
    ] {
        let rest = pages.split_once(&format!("on {name}")).unwrap().1;
        let body = rest.split_once("\non ").map_or(rest, |(body, _)| body);
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
    app.mutation_phase = "channel".into();
    app.pending_message = "sent message".into();

    // an unrelated mutation's ack carries no snapshot — nothing to stomp
    // (reactions no longer route through ChatAcked at all; a channel op is
    // the surviving non-message phase)
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

/// The picker-dismissal bug in one contract: an in-flight reaction must not
/// take the global mutation lock. A locked picker's disabled cells capture
/// no press, so the SECOND click of a picking session fell through to the
/// backdrop and dismissed the overlay; the hover bar's one-tap reactions
/// silently no-op'd through the same window.
#[test]
fn reactions_run_outside_the_mutation_lock() {
    let (mut app, _) = Ducktape::__boot();
    app.connected_rpc = "http://node".into();
    app.loading = false;
    app.active_channel = "general".into();
    app.messages = vec![message(7, "root", false)];
    app.selected_message_seq = 7;
    app.selected_message_rev = 1;
    app.message_action = "reactions".into();

    let _ = app.__update(__DucktapeMessage::AddReactionSubmit("👍".into()));

    assert_eq!(app.mutation_phase, "idle", "reactions never take the lock");
    assert_eq!(app.selected_message_seq, 7);
    assert_eq!(app.message_action, "reactions", "the picker stays open");

    // the ack leaves the picker exactly where it was — multi-pick works
    let _ = app.__update(__DucktapeMessage::ReactionAcked(true));
    assert_eq!(app.selected_message_seq, 7);
    assert_eq!(app.message_action, "reactions");
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

/// A HISTORY PAGE BELONGS TO THE CHANNEL THAT ASKED FOR IT. Only
/// `load_more_history` bumps `history_generation` — nothing that moves
/// `active_channel` does — so the generation guard ALONE let a page still in
/// flight for #a prepend into #b's timeline, merging one room's scrollback into
/// another's with no error and no way to tell. `HistoryPageData` carries the
/// channel it was requested for and the reducer checks it.
///
/// The flag is released ABOVE that check: a page dropped for landing in the
/// wrong room must still free "Load older", which `load_more_history` refuses
/// while `history_loading` stands.
#[test]
fn a_history_page_prepends_only_into_the_channel_that_asked_for_it() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.mutation_phase = "idle".into();
    app.active_channel = "a".into();
    app.messages = vec![message(10, "a-ten", false)];
    let _ = app.__update(__DucktapeMessage::LoadMoreHistory);
    assert!(app.history_loading);
    let in_flight = app.history_generation;

    // The reader is on #b by the time the page lands. `active_channel` moves
    // under an open request on the resync, the search-hit and the create routes
    // too, and NONE of them bump `history_generation`.
    app.active_channel = "b".into();
    app.messages = vec![message(10, "b-ten", false)];
    let _ = app.__update(__DucktapeMessage::HistoryLoaded(backend::HistoryPageData {
        generation: in_flight,
        channel_id: "a".into(),
        messages: vec![message(1, "a-one", false)],
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
    let in_flight = app.history_generation;
    let _ = app.__update(__DucktapeMessage::HistoryLoaded(backend::HistoryPageData {
        generation: in_flight,
        channel_id: "b".into(),
        messages: vec![message(1, "b-one", false)],
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
        app.mutation_phase = "idle".into();
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
        app.mutation_phase = "idle".into();
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
        include_str!("ui/handlers/chat.ice"),
        include_str!("ui/handlers/lifecycle.ice"),
        include_str!("ui/handlers/onboarding.ice"),
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
            "console_opened",
            "live_resynced",
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
}

/// CLOSING RETIRES THE LOAD THAT WOULD REOPEN WHAT YOU JUST LEFT. Every forge
/// loader guards on generation equality alone, so a close that did not bump the
/// generation answered its own in-flight read: `forge_repo_loaded` re-assigns
/// `forge_repo` and `forge_item_loaded` re-assigns `forge_item_number`, dropping
/// the user straight back into the repo or item they had just backed out of.
///
/// `forge_merged` is the one reply NOT keyed on the generation — it is guarded on
/// repo + number, and `forge_close_item` zeroes the number. It therefore releases
/// `forge_merge_busy` ABOVE that check: this reply is the only thing that lowers
/// the flag and `forge_merge_submit` returns early on it, so closing an item
/// mid-merge left the Merge button disabled for the rest of the session.
#[test]
fn closing_a_repo_or_an_item_retires_the_load_that_would_reopen_it() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();

    let _ = app.__update(__DucktapeMessage::ForgeOpenRepo("core".into()));
    let in_flight = app.forge_generation;
    let _ = app.__update(__DucktapeMessage::ForgeCloseRepo);
    assert!(app.forge_repo.is_empty());
    let _ = app.__update(__DucktapeMessage::ForgeRepoLoaded(backend::ForgeRepoData {
        generation: in_flight,
        repo: "core".into(),
        branches: vec!["main".into()],
        items: Vec::new(),
    }));
    assert!(
        app.forge_repo.is_empty(),
        "a closed repo must not be reopened by the load it left in flight"
    );
    assert!(app.forge_branches.is_empty());

    // The same retirement one level in.
    let _ = app.__update(__DucktapeMessage::ForgeOpenRepo("core".into()));
    let _ = app.__update(__DucktapeMessage::ForgeOpenItem(7));
    let in_flight = app.forge_generation;
    let _ = app.__update(__DucktapeMessage::ForgeCloseItem);
    assert_eq!(app.forge_item_number, 0);
    let _ = app.__update(__DucktapeMessage::ForgeItemLoaded(backend::ForgeItemData {
        generation: in_flight,
        repo: "core".into(),
        number: 7,
        title: "a pull request".into(),
        ..backend::ForgeItemData::default()
    }));
    assert_eq!(
        app.forge_item_number, 0,
        "a closed item must not be reopened by the load it left in flight"
    );
    assert!(app.forge_item_title.is_empty());

    // AND THE MERGE FLAG COMES DOWN EVEN THOUGH ITS ITEM IS GONE.
    let (mut merging, _) = Ducktape::__boot();
    merging.connected = true;
    merging.connected_rpc = "http://node".into();
    merging.forge_repo = "core".into();
    merging.forge_item_number = 7;
    let _ = merging.__update(__DucktapeMessage::ForgeMergeSubmit);
    assert!(merging.forge_merge_busy);
    let _ = merging.__update(__DucktapeMessage::ForgeCloseItem);
    let _ = merging.__update(__DucktapeMessage::ForgeMerged(backend::ForgeMergeOutcome {
        repo: "core".into(),
        number: 7,
        merged: false,
        merge_oid: String::new(),
        conflicts: vec!["app/src/main.rs".into()],
    }));
    assert!(
        !merging.forge_merge_busy,
        "closing an item mid-merge must not disable Merge for the rest of the session"
    );
    // The identity check still guards the BODY: that outcome describes an item
    // nobody has open, so nothing of it is rendered.
    assert!(merging.forge_merge_conflicts.is_empty());
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
                page_title: "Old page".into(),
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

    let chat = inlined(include_str!("ui/screens/chat.ice"));
    assert!(chat.contains("stack #message(message.id) w=fill"));
    assert!(!chat.contains("#message(message.seq)"));
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

    let chat = inlined(include_str!("ui/screens/chat.ice"));
    assert!(chat.contains("button \"Jump to latest\""));
    assert!(chat.contains("-> emit(choose_channel, active_channel)"));
}

#[test]
fn message_actions_require_explicit_intent() {
    let (mut app, _) = Ducktape::__boot();
    app.mutation_phase = "idle".into();

    app.chat_pointer_y = 450.0;
    app.chat_height = 500.0;
    let _ = app.__update(__DucktapeMessage::OpenMessageActions(7, "hello".into(), 2));
    assert_eq!(app.message_menu_y, 260.0);
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
}

#[test]
fn message_action_toolbar_stays_compact_and_accessible() {
    let components = inlined(include_str!("ui/components/chat.ice"));
    let toolbar = components
        .split_once("component MessageCard")
        .unwrap()
        .1
        .split_once("component ThreadMessageCard")
        .unwrap()
        .0;
    // Hover is DRAW-TIME: the `hover` widget reveals the toolbar under the
    // cursor with no enter/exit routes and no hovered state — a cached lazy
    // row keeps native-latency hover. `open=menu_open` is the one exception,
    // and it is the ANCHOR CONTRACT: while the ♡/⋯ card this row opened is
    // up, the toolbar it hangs off stays, however far the pointer went.
    assert!(toolbar.contains("hover tint=row_hover r=9.0 open=menu_open"));
    let stream = inlined(include_str!("ui/screens/chat.ice"));
    assert!(stream.contains("MessageCard message selected=true menu_open=true"));
    assert!(toolbar.contains("if !message.deleted && !message.pending"));
    assert!(!toolbar.contains("&& hovered"));
    assert!(!toolbar.contains("mouse enter="));
    // the artifact's hover bar is five 27×25 cells: three one-tap reactions,
    // the reaction picker and the overflow menu (Console:244).
    assert_eq!(toolbar.matches("w=27.0 h=25.0").count(), 5);
    // the one svg cell takes the icon as a direct child; a `h=fill` wrapper
    // inside a fixed-size button collapses an SVG to a hairline. The other
    // four cells are the artifact's own typographic glyphs, not icons.
    assert_eq!(toolbar.matches("p=5.0 @icon_action").count(), 1);
    assert!(components.contains(
        "text message.author size=13.0 wrap=none font=display @text-fg\n            if message.avatar_kind == \"agent\""
    ));
    // the stamp beside the author is the block the message was finalized
    // in — a chain fact the app can prove, never a wall-clock time.
    assert!(components.contains(
        "if message.height > 0\n              text height_label_short(message.height) size=11.0 wrap=none font=code_medium @text-hint"
    ));
    // Slack-style grouping: the shared avatar + author header only renders
    // for a run's first message; continuations keep the body aligned via a
    // gutter that matches the avatar's width.
    assert!(components.contains(
        "if message.show_author\n        MessageAvatar initials=message.initial kind=message.avatar_kind"
    ));
    assert!(components.contains("if !message.show_author\n        space w=30.0"));
    assert!(components.contains("\"human\"\n        PersonAvatar initials plate=30.0 ink=11.0"));
    assert!(components.contains("\"agent\"\n        AgentAvatar initials plate=30.0 ink=11.0"));
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

    let chat = inlined(include_str!("ui/screens/chat.ice"));
    assert!(
        chat.contains("overlay when=(selected_message_seq > 0 && message_action != \"toolbar\")")
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
    // Cells must stay pressable while a reaction is in flight: a disabled
    // button captures no press, and an uncaptured press inside the overlay
    // dismisses it (see `reactions_run_outside_the_mutation_lock`).
    assert!(!picker.contains("mutation_phase"));

    let handlers = inlined(include_str!("ui/handlers/chat.ice"));
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
    assert_eq!(handlers.matches("task widget focus-next").count(), 6);
    assert!(!inlined(include_str!("ui/extern/backend.ice")).contains("task focus_next()"));
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
    let components = inlined(include_str!("ui/components/chat.ice"));
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
    assert!(card.contains("MessageContents message=message flash=flash"));
    // The rail's settle ✓ is real now: the card takes the fade as a prop and
    // `thread_send_flash_id` anchors it from the screen's flash arm. (`card`
    // starts right after the component name, so the signature is its head.)
    assert!(card.starts_with(
        "(message:ChatMessage, selected:bool, menu_open:bool, disabled:bool, flash:f64)"
    ));
    // `menu_open` cannot be `selected` here: in the rail `selected` marks the
    // deep-link TARGET reply, not the row whose action card is open.
    let chat_screen_rail = inlined(include_str!("ui/screens/chat.ice"));
    assert!(chat_screen_rail.contains("menu_open=(thread_message.seq == thread_selected_seq)"));
    // No open-thread action from inside a thread you are already reading. The
    // shared contents still declare the event (their reply pill emits it) so
    // the card forwards it, but the rail's toolbar has no seat for it — and a
    // reply carries no replies, so the pill never renders here.
    assert!(!card.contains("label=\"Open thread\""));

    let chat_screen = inlined(include_str!("ui/screens/chat.ice"));
    let thread = chat_screen
        .split_once("if active_thread_seq > 0 && !channel_settings_open")
        .unwrap()
        .1;
    // A SECOND overlay, keyed on thread-scoped state, independent of the main one.
    assert!(thread.contains(
        "overlay when=(thread_selected_seq > 0 && thread_message_action != \"toolbar\")"
    ));
    assert!(thread.contains("dismiss=emit(clear_thread_message_selection) backdrop=transparent"));
    assert!(thread.contains("float x=0.0 y=thread_menu_y"));
    assert!(thread.contains("mouse press-at=emit(thread_pointer_pressed, _, _)"));
    // same seat as the message list — the rail measures itself
    assert!(thread.contains("sensor show=emit(thread_resized, _, _)"));
    // The picker is the shared ADD grid targeting the thread selection;
    // removal rides the reply's own reaction chips.
    assert!(thread.contains("for emoji in reaction_palette()"));
    assert!(thread.contains("-> emit(add_reaction_at, thread_selected_seq, emoji)"));
    // Same pressable-while-in-flight contract as the stream picker.
    let thread_picker = thread
        .split_once("thread_message_action == \"reactions\"")
        .unwrap()
        .1
        .split_once("thread_message_action == \"editing\"")
        .unwrap()
        .0;
    assert!(!thread_picker.contains("mutation_phase"));
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

    let handlers = inlined(include_str!("ui/handlers/chat.ice"));
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
    app.message_action = "editing".into();
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
    app.huddle_joined = true;
    app.huddle_channel = "chan-a".into();

    let _ = app.__update(__DucktapeMessage::ConsoleOpened(iced::window::Id::unique()));

    assert_eq!(app.connected_rpc, "http://node-b");
    assert_eq!(app.password, "device-key-password");
    assert_eq!(app.selected_message_seq, 0);
    assert_eq!(app.message_action, "toolbar");
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
    assert!(app.page_search_draft.is_empty());
    assert!(!app.huddle_joined);
    assert!(app.huddle_channel.is_empty());

    let _ = app.__update(__DucktapeMessage::Failed(backend::AppError {
        message: "offline".into(),
        committed: false,
    }));
    assert_eq!(app.connected_rpc, "http://node-b");
}

// The dirty gate makes the tick FIRE; these two guards make it WAIT. An
// in-flight op chain must finish before the next starts (the awaited loop is
// the ordering rule), and an open ``` must be closed before the buffer is
// parsed — otherwise everything under it reads as one code block and the plan
// removes the "vanished" lines.
#[test]
fn the_save_tick_waits_for_inflight_saves_and_open_fences() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.connected = true;
    app.active_page = "page".into();
    // The buffer is this page's — the tick refuses one that is not.
    app.buffer_page = "page".into();
    app.page_editor = compose("Title\nfresh body");
    app.page_saved_text = "Title\nstale".into();
    app.block_autosave_status = "saving".into();
    let generation = app.block_autosave_generation;

    let _ = app.__update(__DucktapeMessage::PageAutosaveTick);
    assert_eq!(app.block_autosave_generation, generation, "inflight guard");

    app.block_autosave_status = "idle".into();
    app.page_editor = compose("Title\n```\nstill typing");
    let _ = app.__update(__DucktapeMessage::PageAutosaveTick);
    assert_eq!(app.block_autosave_generation, generation, "fence guard");

    app.page_editor = compose("Title\n```\ndone\n```");
    let _ = app.__update(__DucktapeMessage::PageAutosaveTick);
    assert_eq!(
        app.block_autosave_generation,
        generation + 1,
        "a closed fence saves"
    );
    assert_eq!(app.block_autosave_status, "saving");
}

fn page_item(id: &str, title: &str) -> backend::PageItem {
    backend::PageItem {
        id: id.into(),
        title: title.into(),
        parent: String::new(),
        prefix: String::new(),
        child_count: 0,
    }
}

fn page_block(id: &str, page: &str, text: &str) -> backend::PageBlock {
    backend::PageBlock {
        key: 0,
        id: id.into(),
        parent: page.into(),
        kind: "Text".into(),
        text: text.into(),
        pending: false,
        checked: false,
        prefix: String::new(),
        child_count: 0,
    }
}

fn page_load(id: &str, title: &str, body: &str) -> backend::PagesData {
    backend::PagesData {
        pages: vec![page_item("alpha", "Alpha"), page_item("beta", "Beta")],
        blocks: vec![page_block(&format!("{id}-1"), id, body)],
        active_page: id.into(),
        active_page_title: title.into(),
        active_page_parent: String::new(),
        comment_thread_total: 0,
        commented_block_hits: Vec::new(),
    }
}

/// The app on Alpha, its document loaded and its buffer clean.
fn reading_alpha() -> Ducktape {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.pages = vec![page_item("alpha", "Alpha"), page_item("beta", "Beta")];
    app.doc_tabs = vec!["alpha".into(), "beta".into()];
    app.active_page = "alpha".into();
    app.active_page_title = "Alpha".into();
    app.active_page_parent = "Root".into();
    app.blocks = vec![page_block("alpha-1", "alpha", "alpha body")];
    app.page_editor = compose("Alpha\nalpha body");
    app.page_saved_text = "Alpha\nalpha body".into();
    app.buffer_page = "alpha".into();
    app
}

/// AN ERROR MUST NOT ASSERT A DIAGNOSIS IT HAS NOT MADE. `connect` discarded
/// the real cause with `map_err(|_| …)` and said "Could not connect. Check the
/// endpoint and node." — the one thing the reader can act on, and wrong
/// whenever the node is answering fine and the failure is a timeout, an
/// unreadable reply or a broken signer. Measured while debugging this screen:
/// the node served `/v1/status` in under a millisecond and the app still said
/// to go check it.
///
/// Pinned as a source shape because the failure is an async RPC round trip with
/// no seam to fake here; `user_error` itself is covered by its own tests.
#[test]
fn connect_reports_the_cause_instead_of_guessing_at_it() {
    const LIVE: &str = include_str!("backend/live.rs");
    let connect = LIVE
        .split("pub async fn connect(")
        .nth(1)
        .expect("connect is declared")
        .split("\npub ")
        .next()
        .expect("connect body");

    assert!(
        connect.contains("user_error(cause.to_string())"),
        "connect must route its cause through the translator the rest of the app uses"
    );
    assert!(
        !connect.contains("map_err(|_|"),
        "throwing the cause away is what made this error a guess"
    );
    // NOT asserted: that the old sentence is absent from the function. The
    // comment above the fix quotes it to explain what was wrong, and a sweep
    // over source text cannot tell a message from the prose about it — the
    // check would fail on its own documentation.
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
        app.block_autosave_status, "idle",
        "a fabricated buffer must never be saved into a real page"
    );
}

/// A FAILED LOAD MUST NOT LET THE BLANK PANE EAT THE PAGE IT NEVER OPENED.
/// The optimistic switch moves `active_page` and blanks the buffer before the
/// round trip. If the load then FAILS, `on failed` clears `loading` without
/// clearing `connected` or putting `active_page` back — so the reader is left
/// looking at an empty, fully typable document under the new page's title.
///
/// One keystroke there used to reach the 900ms save tick, which wrote
/// `page_text(page_editor)` into `active_page`. Saving an empty document
/// against a real page is a `RemoveBlock` for every line it had: the page would
/// be destroyed by the act of failing to open it, and the reader would never
/// have seen a line of it.
#[test]
fn a_failed_page_load_cannot_save_the_blank_pane_over_the_page() {
    let mut app = reading_alpha();

    let _ = app.__update(__DucktapeMessage::ChoosePage("beta".into()));
    let _ = app.__update(__DucktapeMessage::Failed(backend::AppError {
        message: "node blip".into(),
        committed: false,
    }));

    // The pane is live and typable: this is the state the guard must survive,
    // not one it can assume away.
    assert!(!app.loading, "the failure released the load");
    assert!(app.connected, "the failure did not disconnect");
    assert_eq!(app.active_page, "beta");
    assert!(
        app.buffer_page.is_empty(),
        "no load landed, so the buffer belongs to no page"
    );

    app.page_editor = compose("h");
    let _ = app.__update(__DucktapeMessage::PageAutosaveTick);

    // The tick must refuse: the buffer is not Beta's.
    assert_eq!(
        app.block_autosave_status, "idle",
        "a buffer that belongs to no page must never be saved into one"
    );
    assert!(app.pending_page.is_empty());
}

// A CLICK MUST REPAINT ON THE CLICK. The page load is several round trips; the
// sidebar highlight, the header title and the document cannot wait for it, or
// the app reads as dead for seconds. Everything asserted here is the state of
// the very next frame — nothing has landed yet.
#[test]
fn a_page_click_repaints_before_the_load_lands() {
    let mut app = reading_alpha();

    let _ = app.__update(__DucktapeMessage::ChoosePage("beta".into()));

    assert_eq!(app.active_page, "beta", "the sidebar highlight moves now");
    assert_eq!(
        app.active_page_title, "Beta",
        "the header title comes from the page list already in hand"
    );
    assert!(
        app.active_page_parent.is_empty(),
        "the breadcrumb of the page she left must not hang over the new one"
    );
    assert!(
        app.blocks.is_empty(),
        "the previous document's blocks must leave the pane"
    );
    assert!(
        page_document_text(&app).is_empty(),
        "the previous document's text must leave the pane"
    );
    assert!(app.loading, "the load is still in flight");
    // The buffer is honest about holding nothing: `buffer_page` is what the
    // install decision reads, and the baseline moves with the buffer so an
    // empty pane never reads as dirty to the save tick.
    assert!(app.buffer_page.is_empty());
    assert!(app.page_saved_text.is_empty());
}

// `buffer_page`, not `active_page`, is what the install decision compares.
// Closing the front tab moves the selection while the buffer is still the old
// page's and still DIRTY — read against `active_page` the landing document is
// a same-page refresh, the dirty buffer refuses it, and Beta opens showing
// Alpha's text.
#[test]
fn the_landing_document_installs_when_the_page_actually_moved() {
    let mut app = reading_alpha();
    app.page_editor = compose("Alpha\nalpha body, still typing");

    let _ = app.__update(__DucktapeMessage::CloseDocTab("alpha".into()));
    assert_eq!(app.active_page, "beta", "the tab close moved the selection");
    assert_eq!(app.buffer_page, "alpha", "the buffer is still Alpha's");

    let _ = app.__update(__DucktapeMessage::PagesUpdated(page_load(
        "beta",
        "Beta",
        "beta body",
    )));

    assert_eq!(page_document_text(&app), "Beta\nbeta body");
    assert_eq!(app.page_saved_text, "Beta\nbeta body");
    assert_eq!(app.buffer_page, "beta");
    assert_eq!(app.blocks.len(), 1);
    assert_eq!(app.blocks[0].id, "beta-1");
}

// THE KEYSTROKE-EATING GUARD, which the split must not cost us: a reload of
// the page the user is typing in leaves her words alone — even when the text
// it carries is genuinely newer than the baseline (somebody else edited the
// page). A same-page refresh whose text merely equals the baseline would
// install nothing anyway, and would prove nothing here.
#[test]
fn a_refresh_never_overwrites_a_dirty_buffer_on_the_same_page() {
    let mut app = reading_alpha();
    app.page_editor = compose("Alpha\nalpha body, still typing");

    let _ = app.__update(__DucktapeMessage::PagesUpdated(page_load(
        "alpha",
        "Alpha",
        "alpha body, edited by somebody else",
    )));

    assert_eq!(
        page_document_text(&app),
        "Alpha\nalpha body, still typing",
        "a reload must never eat keystrokes"
    );
    assert_eq!(
        app.page_saved_text, "Alpha\nalpha body",
        "the baseline stays with the buffer — the drift is what makes the next tick save"
    );
    assert_eq!(app.buffer_page, "alpha");
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

#[test]
fn the_page_surface_is_one_editor_with_no_click_to_edit_left() {
    let components = inlined(include_str!("ui/components/pages.ice"));
    let handlers = inlined(include_str!("ui/handlers/pages.ice"));
    let view = inlined(include_str!("ui/screens/pages.ice"));

    // THE TITLE IS LINE 0 OF THE BUFFER, not a control. The click-to-edit
    // title editor is gone the same way the click-to-edit blocks are; these
    // stay as refusals so neither creeps back.
    assert!(!components.contains("PageTitleEditor"));
    assert!(!components.contains("task widget focus #title-input"));
    assert!(!components.contains("defer_focus"));
    assert!(!handlers.contains("focus_page_title"));
    assert!(!inlined(include_str!("ui/extern/backend.ice")).contains("defer_focus"));
    // THE CANVAS HAS NO MENUS LEFT TO PLACE. The block-actions popover, its
    // pointer tracking and the insert row's type dropdown are gone with the
    // click-to-edit model — a page is one editor, and `# ` is the block-type
    // menu.
    assert!(!view.contains("pages_pointer"));
    assert!(!view.contains("mouse move="));
    assert!(!view.contains(
        "scroll dir=vertical w=fill h=fill bar=hidden\n              box w=fill max-w=720.0"
    ));
    assert!(!view.contains("BlockActionsMenu"));
    assert!(!view.contains("block_menu_x"));
    assert!(!view.contains("InlineBlockInsert"));
    assert!(!view.contains("slash_kind_matches"));
    assert!(!components.contains("component DocumentBlock"));
    // The one overlay the surface still raises is the page-delete confirm.
    assert!(view.contains("overlay when=page_delete_armed"));
    // The document column opens directly on the one editor.
    assert!(view.contains("extern page_document(page_editor, dark,"));
}

#[test]
fn shell_uses_canonical_glass_and_opaque_content() {
    let ui = inlined(concat!(
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
    ));
    for gradient in ["linear(", "radial(", "conic("] {
        assert!(!ui.contains(gradient), "{gradient}");
        assert!(!SCREENS.contains(gradient), "{gradient}");
    }
    // The window is opaque. iced has no backdrop blur, so the chrome paints
    // the artifact's own non-glass ladder — desk/rail/sidebar/content — and
    // never a translucent tint that would composite over the desktop.
    let app = inlined(include_str!("ui/app.ice"));
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
    let defaults = inlined(include_str!("ui/theme.ice"));
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
    let theme = inlined(include_str!("ui/theme.ice"));
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

    let shell = inlined(include_str!("ui/components/shell.ice"));
    // the shell is titlebar + optional degradation banner over the panes.
    assert!(shell.contains(
        "component TitleBar(network:str, height:i64, loading:bool, degraded:bool, bell_badge:i64, bell_sev:str, tier:str, answered:bool, root_hash:str, consensus_view:str, quorum:str, reachable:str, last_finalized:i64, checkpoint:i64)"
    ));
    // The bar exists only in the console window now — the launch window
    // wears OS chrome — so the chip and the status/bell cluster are
    // unconditional: no `phase` discriminant may return here.
    let bar = shell.split_once("component TitleBar(").unwrap().1;
    let bar = bar.split_once("\ncomponent ").unwrap().0;
    assert!(bar.contains("NetworkChip name=network"));
    assert!(!bar.contains("phase"));
    assert!(shell.contains("component ConnectionBanner(status:str)"));
    // The degradation band rides in the CONTENT column, below the rail row —
    // mounted above it, it pushed the whole nav rail down by its own height
    // whenever the connection wobbled, so a click at a remembered rail position
    // landed one item off. Pin the order, not the indent.
    let tabs = shell.split_once("component WorkspaceTabs(").unwrap().1;
    let rail_at = tabs.find("NavRail #rail").expect("the rail mounts here");
    let banner_at = tabs
        .find("ConnectionBanner status=status")
        .expect("the band mounts here");
    assert!(banner_at > rail_at, "the band must not sit above the rail");
    assert!(shell.contains("box #root w=74.0 h=fill pt=13.0 pb=10.0 bg=rail"));
    // The status tooltip ALWAYS overflows the window's right edge, and iced
    // snaps an overflowing tip hard against it. The paper therefore belongs to
    // StatusCard and the tooltip frame stays transparent, so the `pr` gutter
    // can hold the card off the wall on the bell card's line.
    assert!(bar.contains("tooltip position=bottom gap=13.5 p=0.0 delay=90 style=transparent"));
    // Per-frame extern bans. These two walk the disk (workspace tomls) or
    // deep-clone the whole timeline through the extern ABI, so the view reads
    // their STATE MIRRORS (`network_name`, `has_older_history`) instead. If
    // either name returns to a view or screen file, the per-frame tax is back.
    assert!(!SCREENS.contains("network_label("));
    assert!(!inlined(include_str!("ui/view.ice")).contains("network_label("));
    assert!(!SCREENS.contains("history_has_older("));
    assert!(!inlined(include_str!("ui/view.ice")).contains("history_has_older("));
    assert!(bar.contains("box pr=13.0\n              StatusCard "));
    assert!(shell.contains(
        "box #root w=284.0 pl=14.0 pr=14.0 pt=13.0 pb=13.0 bg=surface border=border border-w=1.0 r=13.0 shadow=shadow_modal shadow-y=16.0 shadow-blur=40.0"
    ));
    assert!(SCREENS.contains("box w=236.0 h=fill bg=sidebar clip=true"));
    assert!(SCREENS.contains("box w=230.0 h=fill bg=sidebar clip=true"));

    // The endpoint field is GONE from Settings — the launch window's picker
    // owns which network; Settings keeps only Reconnect / Switch network.
    assert!(!SCREENS.contains("#rpc"));
    assert!(SCREENS.contains("emit(switch_network)"));
    assert!(SCREENS.contains("input \"\" #key-password <-> key_pw label=\"Key password\""));
    assert!(SCREENS.contains("if active_thread_seq > 0 && !channel_settings_open"));
    // Both chat composers wear the SAME plate now — the rail dropped its old
    // transparent fg/12 frame for the stream's surface/control_line/r12 chrome.
    assert_eq!(
        SCREENS
            .matches("box w=fill bg=surface border=control_line border-w=1.0 r=12.0 clip=true")
            .count(),
        2
    );
    // the palette card moved into the overlay layer with the rest of the
    // window-level surfaces; the assertion follows the code it guards.
    let overlays = inlined(include_str!("ui/screens/overlays.ice"));
    assert!(overlays.contains(
        "bg=surface border=border border-w=1.0 r=14.0 shadow=shadow_modal shadow-y=24.0 shadow-blur=60.0"
    ));

    let authored_pages = inlined(include_str!("ui/components/pages.ice"));
    for authored in [&shell, &authored_pages, &*SCREENS] {
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
        SCREENS
            .matches(", shift_held, 44.0, 150.0, 10.0) #")
            .count(),
        2
    );
    assert!(SCREENS.contains(", shift_held, 38.0, 120.0, 6.0) #forge-note"));
    assert!(SCREENS.contains("button \"Send\" disabled="));
    assert!(SCREENS.contains(
        "h=29.0 @primary_action @px-12px @py-7px -> emit(composer_event, composer_submit_event())"
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

    let components = inlined(concat!(
        include_str!("ui/components/shell.ice"),
        include_str!("ui/components/chat.ice"),
        include_str!("ui/components/pages.ice"),
    ));
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
    // The `+`/`⋮⋮` gutter cluster went with the block canvas.
    assert!(!components.contains("Insert block below"));
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

    let view = inlined(include_str!("ui/view.ice"));
    let shell = inlined(include_str!("ui/components/shell.ice"));
    let chat = inlined(include_str!("ui/components/chat.ice"));
    let pages = inlined(include_str!("ui/components/pages.ice"));
    let kit = inlined(include_str!("ui/components/kit.ice"));
    let forge = inlined(include_str!("ui/components/forge.ice"));

    assert_recipe_owns_states("screens", &SCREENS, "@primary_action");
    assert_recipe_owns_states("screens", &SCREENS, "@danger_action");
    assert_recipe_owns_states("chat.ice", &chat, "@danger_action");
    assert_recipe_owns_states("pages.ice", &pages, "@danger_action");
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
    assert!(SCREENS.contains("KeyValueRow label=\"Key path\" value=settings_key_path last=false"));

    for target in [
        "rename_channel_submit",
        "add_channel_member_submit",
        "fs_mkdir_submit",
        "fs_new_file_submit",
        "gov_execute",
        "account_rename_submit",
    ] {
        let kit_components = inlined(include_str!("ui/components/kit.ice"));
        let action = SCREENS
            .lines()
            .chain(kit_components.lines())
            .find(|line| line.trim_start().starts_with("button ") && line.contains(target))
            .unwrap_or_else(|| panic!("missing action target {target}"));
        assert!(action.contains("@secondary_action"), "{action}");
    }
    // A divider is `---` typed into the document now, not a button.
    assert!(!SCREENS.contains("Insert divider"));

    assert_controls_inherit_focus("screens", &SCREENS);
    assert_controls_inherit_focus("pages.ice", &pages);
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
    // ZERO, and it must stay zero: those two `opened` blocks styled the
    // block-type dropdowns — the one parked at the right of every insert row
    // and the one inside the `⋮⋮` menu. Pages has no block-type picker at all
    // now; the markdown prefix is the picker.
    assert_eq!(
        pages
            .matches("opened text=fg placeholder=muted handle=fg bg=fg/11 border=ring")
            .count(),
        0
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
    for source in [&view, &shell, &chat, &pages, &kit, &forge, &*SCREENS] {
        assert!(!source.contains("bg=success/"));
        assert!(!source.contains("border=success/"));
    }
}

/// App-authored text sizes stay on the app design scale, while the shared
/// Ice palette stays identical to the retained ducktape-ui theme.
#[test]
fn ice_sources_hold_to_the_design_system() {
    let sources = [
        ("view.ice", inlined(include_str!("ui/view.ice"))),
        ("chat.ice", inlined(include_str!("ui/components/chat.ice"))),
        ("dm.ice", inlined(include_str!("ui/components/dm.ice"))),
        (
            "files.ice",
            inlined(include_str!("ui/components/files.ice")),
        ),
        (
            "forge.ice",
            inlined(include_str!("ui/components/forge.ice")),
        ),
        (
            "huddle.ice",
            inlined(include_str!("ui/components/huddle.ice")),
        ),
        ("icon.ice", inlined(include_str!("ui/components/icon.ice"))),
        ("kit.ice", inlined(include_str!("ui/components/kit.ice"))),
        ("node.ice", inlined(include_str!("ui/components/node.ice"))),
        (
            "onboarding.ice",
            inlined(include_str!("ui/components/onboarding.ice")),
        ),
        (
            "overlay.ice",
            inlined(include_str!("ui/components/overlay.ice")),
        ),
        (
            "pages.ice",
            inlined(include_str!("ui/components/pages.ice")),
        ),
        (
            "patterns.ice",
            inlined(include_str!("ui/components/patterns.ice")),
        ),
        (
            "roster.ice",
            inlined(include_str!("ui/components/roster.ice")),
        ),
        (
            "shell.ice",
            inlined(include_str!("ui/components/shell.ice")),
        ),
        // the screens carry the console's authored type scale now that
        // view.ice holds only the mounts.
        ("screens", SCREENS.clone()),
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
    let theme = inlined(include_str!("ui/theme.ice"));
    assert!(theme.contains(&format!("family=\"{}\"", design::fonts::FAMILY_UI)));
    assert!(theme.contains(&format!("family=\"{}\"", design::fonts::FAMILY_MONO)));
    let app = inlined(include_str!("ui/app.ice"));
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
    let pages = inlined(include_str!("ui/screens/pages.ice"));
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
    assert!(pages.contains("#page-comment(scope_key(connected_rpc, active_page))"));
    assert!(!pages.contains("button \"Save\""));
    assert!(!pages.contains("Saving"));

    // The control is a DOCUMENT ACTION in the header now, not a row buried in
    // a per-block menu — the rail was always page-scoped.
    assert!(pages.contains("button label=\"Comments\""));
    assert!(pages.contains("-> emit(toggle_block_comments)"));
    let components = inlined(include_str!("ui/components/pages.ice"));
    assert!(!components.contains("component BlockActionsMenu"));

    let handlers = inlined(include_str!("ui/handlers/pages.ice"));
    assert!(handlers.contains("on post_block_comment_submit"));
    // A NEW comment anchors on the CARET's block (the thread's own target on
    // a reply) — never blindly on the page.
    assert!(handlers.contains(
        "run post_block_comment(connected_rpc, password, active_thread_target, active_block_comment_thread"
    ));
    assert!(handlers.contains(
        "let fresh_target = keep_str(!empty(caret_comment_target), caret_comment_target, active_page)"
    ));
    // Opening a thread rides the thread's OWN anchor — a block-anchored
    // thread opened with the page id is refused by the node.
    assert!(handlers.contains("on open_block_comment_thread(id, target)"));
    // The document wears its comment story: washes from the load, resolve
    // available from the open thread. The editor is handed the BLOCKS and the
    // raw hit list rather than a precomputed line set, because the chip in the
    // margin spells how many threads sit on the line and the count is the
    // repetition in `commented_block_hits` — a precomputed `[i64]` of lines
    // has already thrown it away.
    assert!(pages.contains(
        "page_document(page_editor, dark, (loading || !connected), blocks, commented_block_hits)"
    ));
    assert!(pages.contains("-> emit(resolve_thread_submit, true)"));
}

#[test]
fn comment_pages_merge_by_identity_and_ordinal() {
    let thread = |id: &str, count: i64| backend::PageCommentThread {
        id: id.into(),
        target: "page".into(),
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
                target: "page".into(),
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
    app.active_page = "page".into();
    app.block_comment_draft = "unfinished comment".into();

    let _ = app.__update(__DucktapeMessage::Reconnect);

    // A half-typed COMMENT still survives a reconnect. The page body does not
    // need the same rescue: it is one buffer whose every keystroke is already
    // heading for the node on the save tick, and it is reinstalled from the
    // node's own text on the next load.
    assert_eq!(app.orphaned_comment_drafts, ["unfinished comment"]);
}

/// "NOTHING OPEN" IS TWO DIFFERENT FACTS AND THE PLATE MUST NOT CONFLATE THEM.
/// One message served both, so a workspace that had never held a vote read
/// `0 open · 0 settled` in its header and "every decision on this network is
/// finalized" in its body — asserting a history of decisions nobody ever made.
/// Driven on the running app: the demo network shows exactly that.
///
/// Pinned as COMPLEMENTARY CONDITIONS, not as copy. Asserting the sentences
/// alone would stay green if both arms fired at once, or if the new arm were
/// unreachable.
#[test]
fn approvals_tells_a_first_run_apart_from_a_finished_one() {
    let source = inlined(include_str!("ui/screens/governance.ice"));
    let arms: Vec<&str> = source
        .lines()
        .filter(|line| line.trim_start().starts_with("if ") && line.contains("answered"))
        .map(|line| line.trim())
        .collect();

    assert!(
        arms.contains(&"if connected && empty(rows) && answered"),
        "a network with no proposals at all needs its own arm, found {arms:?}"
    );
    assert!(
        arms.contains(&"if connected && open_proposals(rows) <= 0 && !empty(rows) && answered"),
        "the settled arm must exclude the empty case, found {arms:?}"
    );

    // The two plates must never be able to fire together: the first requires an
    // empty list, the second an inhabited one.
    let first_run = source
        .split("if connected && empty(rows) && answered")
        .nth(1)
        .expect("first-run arm");
    assert!(
        first_run.contains("No proposals yet"),
        "the first-run plate says nothing has happened yet"
    );
    assert!(
        !first_run
            .split("EmptyPlate")
            .nth(1)
            .unwrap_or("")
            .contains("finalized"),
        "the first-run plate must not claim decisions were finalized"
    );
}

/// ACCOUNT FACTS ONLY WHEN THERE IS AN ACCOUNT. With no account bound,
/// `load_account` returns zeros for every field, and the identity card printed
/// `0 keys 0 nodes` one line under "· validator keypair on this device" — a
/// count of the ACCOUNT's keys reading as a count of THIS DEVICE's, and the two
/// contradicting each other inside one card.
///
/// `account_bound` is the fact that tells an empty account from no account. It
/// was already in state, already gating the Rename submit, and simply was not
/// given to the screen.
#[test]
fn the_identity_card_counts_only_a_bound_account() {
    let settings = inlined(include_str!("ui/screens/settings.ice"));
    assert!(
        settings.contains("account_bound:bool"),
        "the screen has to be handed the fact before it can use it"
    );
    let card = settings
        .split("if account_bound")
        .nth(1)
        .expect("the counts sit under the bound gate")
        .split("\n        col ")
        .next()
        .expect("card region");
    for reading in ["account_members", "account_nodes", "Copy key"] {
        assert!(
            card.contains(reading),
            "{reading} is an account reading and belongs under the gate"
        );
    }

    // And view.ice actually passes it, or the screen renders a default.
    let view = inlined(include_str!("ui/view.ice"));
    assert!(view.contains("account_bound"), "the mount has to supply it");

    // THE SEPARATOR BELONGS TO THE KEY. #998 stopped the card counting an
    // account that does not exist; the dot that had joined those counts stayed
    // ungated, and an unbound account has no `account_id` either — so the line
    // led with it: `· validator keypair on this device`. Every other separator
    // in the console is gated by the run it introduces (forge.ice's repo-count
    // dot carries the reasoning in its own comment).
    let key_line = settings
        .split_once("text account_id")
        .expect("the key line")
        .1
        .split_once(r#"text "keypair on this device""#)
        .expect("the custody clause closes it")
        .0;
    let dot = key_line.find(r#"text "·""#).expect("the separator");
    let guard = key_line
        .find("if !empty(account_id)")
        .expect("the separator's guard");
    assert!(guard < dot, "the dot is gated by the key it introduces");
}

/// A ZERO IS A CLAIM, AND THIS APP SAYS IT WITH BLANK. `count_label` returns
/// "" below one and every count in the app routes through it — except two,
/// which printed the digit right beside a plate that had just said the same
/// thing in words:
///
///   - the bell read `Alerts 0 unread` directly above "Nothing yet — mentions
///     and deliveries land here", while its own `Mark all read` was already
///     gated on `bell_unread <= 0`;
///   - Channel details read `MEMBERS 0` above "No members added."
#[test]
fn the_two_counts_that_printed_a_zero_now_say_nothing() {
    assert_eq!(backend::count_label(0), "", "the convention");
    assert_eq!(backend::count_label(3), "3");

    // The GATE is the load-bearing half — with it, the digit never reaches the
    // screen at zero whatever formats it. Both the number and the word "unread"
    // must carry it, or the panel reads a bare "unread".
    let view = inlined(include_str!("ui/view.ice"));
    let alerts = view
        .split("text \"Alerts\"")
        .nth(1)
        .expect("the bell's Alerts header")
        .split("space w=fill")
        .next()
        .expect("header region");
    assert_eq!(
        alerts.matches("if bell_unread > 0").count(),
        2,
        "the count and the word both stand down at zero"
    );
    assert!(
        alerts.contains("count_label(bell_unread)"),
        "and the digit routes through the app's own blank-at-zero label"
    );

    // Scoped to the drawer's MEMBERS eyebrow. The OTHER member count in this
    // file — the `· N added` run in the channel header — is already correct: it
    // sits under `if !empty(channel_members)` and its comment says why
    // ("`· 0 added` on every normal channel is noise"). A file-wide negative
    // would flag that one too, which is how this assertion first failed.
    let chat = inlined(include_str!("ui/screens/chat.ice"));
    let eyebrow = chat
        .split("Eyebrow label=\"MEMBERS\"")
        .nth(1)
        .expect("the drawer's MEMBERS eyebrow")
        .split("Eyebrow ")
        .next()
        .expect("eyebrow region");
    assert!(
        eyebrow.contains("count_label(len(channel_members))"),
        "the member eyebrow blanks at zero like every other count"
    );
    assert!(
        !eyebrow.contains("text len(channel_members)"),
        "the raw len is what printed `0`"
    );
}

/// AN EMPTY STATE MAY ONLY NAME A MECHANISM THAT EXISTS. Forge's two tracker
/// plates each promised a route into the list, and one of them was wrong while
/// the other named nothing at all:
///
///   - "a PR **pushed to** this repo appears here" — a push does not open a PR.
///     The only production emitter of `ForgeMsg::OpenPr` is the runs sink
///     (`crates/modules/apps/runs/src/sink.rs`), reached from `response.rs`
///     when a run with a PR sink DELIVERS. A push can update an already-open
///     PR; it cannot open one.
///   - "an issue opened against this repo appears here" — passive, naming
///     neither who opens it nor from where. `OpenIssue` has NO production
///     sender at all: only the tests and the demo seeder emit it, there is no
///     CLI verb, and the Code tab on the same screen says the app is view only.
///
/// The third plate on this screen has always done it right — the repo overview
/// says forge IS a git remote and prints the push command — so this is the
/// house style, not a new one.
#[test]
fn forge_empty_states_name_only_routes_that_exist() {
    let forge = inlined(include_str!("ui/screens/forge.ice"));

    assert!(
        forge.contains("No pull requests — an agent run opens one when it delivers its work."),
        "a PR comes from a delivering run, not from a push"
    );
    assert!(
        !forge.contains("a PR pushed to this repo"),
        "the push route was never real"
    );
    assert!(
        forge.contains("No issues — this app reads the tracker but cannot open one yet."),
        "nothing in the shipped surface opens an issue; say so rather than implying a route"
    );

    // AND THE CODE PANE MUST NOT SEND THE READER TO AN EMPTY TREE. The sidebar
    // says "Nothing committed on this branch yet" while the pane beside it said
    // "Pick a file from the tree to read it" — two claims about the same tree,
    // in the same frame.
    assert!(
        forge.contains("if empty(file_path) && empty(tree_entries)"),
        "the no-file pane splits on whether there is a tree to pick from"
    );
    assert!(
        forge.contains("Nothing is committed on this branch yet, so there is no file to read."),
        "an unborn repo says why there is nothing to read"
    );
}

/// THE EXPLORER NAMES WHAT IT SHOWS. Several defects on one screen, all the
/// same shape — the pixels were right and the words were absent or false.
///
/// The header claimed the list holds "the blocks this node verified for
/// itself". It does not. `GET /v1/blocks` serves the derived-index rows, and
/// `bin/noded`'s projection writes NO row for a block whose members are all the
/// `consensus.nop` heartbeat ("a pure nop/idle block — the explorer hides it").
/// Measured against the demo node: `/v1/status` height 419718, while the
/// hundred rows `/v1/blocks` served spanned heights 102907-366045 — a
/// hundred-row "recent" window covering 263k heights, which no lag explains. A
/// reader comparing the top row against the height in the titlebar concludes
/// the node is fifty thousand blocks behind.
///
/// Naming the set truthfully then obliges the list to HOLD that set in every
/// role, which it did not: a node following from a checkpoint writes one
/// op-less boundary row, and that row printed `0 ops` under the new sentence.
///
/// The op detail then printed two values nobody named: `chat(+0m/+0e)`, whose
/// units are spelled out only in `crates/kernel/host`, and a hash that differs
/// from the row's hash on the left because one is the frame id and the other
/// the op payload digest.
///
/// This pins the DERIVATION, the SET, and the LABEL SITES rather than the
/// sentences: the copy may be rewritten, but a count must carry its noun, a
/// value must carry its name, one set must not have two names on one screen,
/// and no row may contradict the name the screen prints over it.
#[test]
fn the_explorer_names_what_it_shows() {
    // The dispatch trace, fed the `operations` shape `bin/noded`'s projection
    // serves. Both units appear once singular and once plural, so a hand-rolled
    // `{n} msgs` that skips the `plural` seam fails here.
    let hops = vec![
        serde_json::json!({
            "module": "chat", "origin": "external",
            "emitted_msgs": 1, "emitted_events": 0,
        }),
        serde_json::json!({
            "module": "tagging", "origin": "module:chat",
            "emitted_msgs": 0, "emitted_events": 2,
        }),
    ];
    assert_eq!(
        backend::explorer_trace(Some(&hops)),
        "chat · 1 msg · 0 events → tagging · 0 msgs · 2 events",
        "every count in the trace names what it counts"
    );

    let explorer = SCREENS
        .split_once("component ExplorerScreen(")
        .expect("the Explorer screen exists")
        .1;
    let explorer = explorer
        .split_once("\ncomponent ")
        .map_or(explorer, |(body, _)| body);

    // ONE SET, ONE NAME. The subtitle and the "No blocks yet" plate describe
    // the same list and had drifted — only the plate knew the list is filtered.
    // The clause lives here once so a rewrite has to move both sites together.
    const SET: &str = "locks that carried operations";
    for (site, opener) in [
        ("subtitle", "ScreenTitle title=\"Explorer\" detail=\""),
        (
            "empty plate",
            "EmptyState title=\"No blocks yet\" description=\"",
        ),
    ] {
        let sentence = explorer
            .split_once(opener)
            .unwrap_or_else(|| panic!("the Explorer {site} is where it was"))
            .1
            .split_once('"')
            .expect("a quoted string closes")
            .0;
        assert!(
            sentence.contains(SET),
            "the Explorer {site} describes the block list as `{sentence}` \
             instead of naming the set the node actually serves"
        );
    }

    // AND NO ROW MAY CONTRADICT THAT SENTENCE — which is a claim about the
    // DATA, not about the copy, because `/v1/blocks` is not uniformly filtered.
    // Three of its four row writers drop an op-less block; the fourth,
    // `boundary_block_row` (`bin/node/src/explorer.rs`, applied in
    // `replica/park.rs`), writes the follower's ascension tip with `hash: ""`
    // and no ops. `bin/node/src/main.rs` routes every key that is neither a
    // validator nor seated by the checkpoint into `replica::run` — every joined
    // member until promotion — so that row drew a blank hash and `0 ops`
    // directly under the subtitle asserted above, and opened to an empty pane.
    let served = [
        serde_json::json!({
            "height": 41, "hash": "", "commit_hash": "aa11bb22cc33dd44", "ops": [],
        }),
        serde_json::json!({
            "height": 42, "hash": "ee55ff66aa77bb88", "commit_hash": "cc99dd00ee11ff22",
            "ops": [{
                "proposer": "abc123def456789a", "disposition": "applied", "target": "chat",
                "op_hash": "0f1e2d3c4b5a6978", "payload": "hi", "operations": hops,
            }],
        }),
    ];
    let window = backend::explorer_window(0, &served);
    assert_eq!(
        window
            .blocks
            .iter()
            .map(|block| (block.height, block.op_count))
            .collect::<Vec<_>>(),
        vec![(42, 1)],
        "the Explorer listed a block carrying no operations under a subtitle \
         that says every row carried some"
    );
    assert!(
        window.ops.iter().all(|op| op.height == 42),
        "an op was attributed to a block the list does not hold"
    );

    // AND EVERY VALUE IN THE OP DETAIL CARRIES ITS NAME. `by` was already
    // right and is pinned with the two that were not, so the rule reads as a
    // rule instead of an exception for these two fields. `hash` and not `op`:
    // the list's third column already spends `op` as a count noun (`1 op`).
    let detail = explorer
        .split_once("for op in explorer_ops_at(ops, selected)")
        .expect("the op detail pane iterates the selected block's ops")
        .1;
    for (label, value) in [
        ("hash", "text op.op_hash"),
        ("by", "text op.proposer"),
        ("dispatch", "text op.trace"),
    ] {
        let named = format!("text \"{label}\"");
        let label_at = detail
            .find(&named)
            .unwrap_or_else(|| panic!("`{value}` is drawn with no `{named}` beside it"));
        let value_at = detail
            .find(value)
            .unwrap_or_else(|| panic!("the op detail no longer draws `{value}`"));
        assert!(
            label_at < value_at,
            "`{named}` must precede `{value}`, or it labels something else"
        );
    }
}

/// A LIST THAT DRIVES A DETAIL PANE MUST SAY WHICH ROW IT IS SHOWING. The
/// Explorer's block rows carried `active bg=transparent` on BOTH states, so
/// clicking a row filled the pane on the right and left the list identical to
/// the pixel — measured on the running app at 36.78/255 before and after the
/// click, pointer parked away. The only visible response was `hovered`, which
/// follows the pointer rather than the selection. In a list whose every row is a
/// height and a truncated hash there is no landmark to re-find your place by.
///
/// Pinned as a DIFFERENCE, not as a colour: asserting `selected_row` alone
/// would stay green if the unselected arm adopted it too, which is the same
/// both-arms-identical bug in a new coat.
#[test]
fn the_explorer_marks_the_block_row_whose_detail_is_open() {
    let source = inlined(include_str!("ui/screens/storage.ice"));
    let row = source
        .split("component ExplorerBlockRow")
        .nth(1)
        .expect("ExplorerBlockRow is declared")
        .split("\ncomponent ")
        .next()
        .expect("component body");

    let selected = row.split("if !selected").next().expect("selected arm");
    let unselected = row.split("if !selected").nth(1).expect("unselected arm");

    assert!(
        selected.contains("active bg=selected_row"),
        "the open row wears a plate"
    );
    assert!(
        unselected.contains("active bg=transparent"),
        "every other row stays flat"
    );
    // The whole point: the two arms must not paint the same thing.
    let plate_of = |arm: &str| {
        arm.lines()
            .find_map(|line| {
                line.trim()
                    .strip_prefix("active bg=")
                    .map(|rest| rest.split_whitespace().next().unwrap_or("").to_owned())
            })
            .expect("each arm sets active bg=")
    };
    assert_ne!(
        plate_of(selected),
        plate_of(unselected),
        "a selected row that paints the unselected plate marks nothing"
    );
}

/// ONE TOKEN MEANS "THIS IS THE ROW YOU ARE ON". The Forge file tree marked its
/// selected row with `tree_selected`; the Files tree marked its own with
/// `subtle` — #35322a against #31302b, 2.3/255 apart in dark. Nobody saw the
/// difference, which IS the defect: two tokens for one meaning drift apart the
/// moment either is retuned. `subtle` could never have been the selection plate
/// anyway, because it is also the PRESSED plate on those very components —
/// pressing an unselected channel painted it the colour of the selected one.
/// Thirteen surfaces mark a current row; ten of them were split across `subtle`
/// and `elevated`. All thirteen now read one token, named for the meaning it
/// carries rather than for the first tree that needed it: `selected_row`.
///
/// Pinned as the CONVENTION in two halves, because neither holds alone:
///   - NEGATIVE, self-extending: no arm that opens on "this is the current one"
///     may rest on `subtle` or `elevated`. This is the half that fails on the
///     NEXT surface someone writes, which a pin on any one call site cannot.
///   - COVERAGE: the set of sources carrying the token is fixed, so a surface
///     cannot quietly slide off the convention onto some third plate.
///
/// Marks that are not a plate at all rest on their own tokens ON PURPOSE and
/// are commented where they live: the tab underline and the filter chip invert
/// to ink, the matrix cell takes the faintest wash because its column HEAD
/// already wears the mark, and the huddle camera toggle is an engaged control
/// rather than a row you navigated to.
#[test]
fn every_current_row_marker_rests_on_one_selection_token() {
    // Every arm that opens on "this is the current one", paired with each
    // plate it RESTS on: the `bg=` of the arm's own first node, and any
    // button's `active bg=` inside it. `hovered`/`pressed` are not resting
    // plates, and a descendant's dot or badge is not the row's plate.
    fn current_row_plates(source: &str) -> Vec<(String, String)> {
        let lines: Vec<&str> = source.lines().collect();
        let mut out = Vec::new();
        for (at, line) in lines.iter().enumerate() {
            let Some(cond) = line.trim().strip_prefix("if ") else {
                continue;
            };
            // A negation is the OTHER arm; a comparison is a count guard
            // (`if selected > 0`), not a selection.
            let names_the_current_one = cond
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .any(|word| word == "selected" || word == "active");
            let opens_a_marker = names_the_current_one && !cond.contains(['!', '<', '>']);
            if !opens_a_marker {
                continue;
            }
            let indent = line.len() - line.trim_start().len();
            let arm: Vec<&&str> = lines[at + 1..]
                .iter()
                .take_while(|body| {
                    body.trim().is_empty() || body.len() - body.trim_start().len() > indent
                })
                .collect();
            let plate_of = |text: &str| {
                text.split_whitespace()
                    .find_map(|token| token.strip_prefix("bg="))
                    .map(str::to_owned)
            };
            let own_plate = arm
                .iter()
                .find(|body| !body.trim().is_empty())
                .and_then(|body| plate_of(body));
            let button_plates = arm
                .iter()
                .filter(|body| body.trim().starts_with("active "))
                .filter_map(|body| plate_of(body));
            for plate in own_plate.into_iter().chain(button_plates) {
                out.push((cond.to_owned(), plate));
            }
        }
        out
    }

    // EVERY authored ice source, each paired with its own path so a failure
    // names the file. The scan must see all of them: a source left out is a
    // surface the convention stops covering.
    macro_rules! ice_sources {
        ($($path:literal),* $(,)?) => { [$(($path, include_str!($path))),*] };
    }
    let sources = ice_sources![
        "ui/components/chat.ice",
        "ui/components/dm.ice",
        "ui/components/files.ice",
        "ui/components/forge.ice",
        "ui/components/huddle.ice",
        "ui/components/icon.ice",
        "ui/components/kit.ice",
        "ui/components/node.ice",
        "ui/components/onboarding.ice",
        "ui/components/overlay.ice",
        "ui/components/pages.ice",
        "ui/components/patterns.ice",
        "ui/components/roster.ice",
        "ui/components/shell.ice",
        "ui/screens/chat.ice",
        "ui/screens/forge.ice",
        "ui/screens/governance.ice",
        "ui/screens/overlays.ice",
        "ui/screens/pages.ice",
        "ui/screens/roster.ice",
        "ui/screens/settings.ice",
        "ui/screens/storage.ice",
        "ui/view.ice",
    ];

    let mut carriers: Vec<&str> = Vec::new();
    for (name, raw) in sources {
        // `with` blocks fold back onto their node line, so a plate and the
        // node it paints stay ONE line however the source was wrapped.
        let source = inlined(raw);
        if source.contains("bg=selected_row") {
            carriers.push(name);
        }
        for (cond, plate) in current_row_plates(&source) {
            assert!(
                plate != "subtle" && plate != "elevated",
                "{name}: `if {cond}` marks the current row with `{plate}` — \
                 that is the track grey / the raised surface, not `selected_row`"
            );
        }
    }

    // The surfaces that mark a current row, all of them, on one token: the
    // channel and the DM you are reading, the page you are editing, the tree
    // directory and the open object, the tree file and the repo switcher, the
    // matrix column head, the network you picked, the nav rail tab and
    // Settings, the member whose card is open, the Explorer block you
    // inspected. A source that drops off this list has either lost its mark or
    // invented a second token for it; both are the bug this test exists for.
    assert_eq!(
        carriers,
        [
            "ui/components/chat.ice",
            "ui/components/dm.ice",
            "ui/components/files.ice",
            "ui/components/forge.ice",
            "ui/components/node.ice",
            "ui/components/onboarding.ice",
            "ui/components/pages.ice",
            "ui/components/shell.ice",
            "ui/screens/roster.ice",
            "ui/screens/storage.ice",
        ],
        "every surface that marks a current row reads `selected_row`"
    );
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
    for (reason, archived, members_only) in
        [("channel_archived", true, false), ("members_only", false, true)]
    {
        let (mut app, _) = Ducktape::__boot();
        app.connected = true;
        app.loading = false;
        app.active_channel = "general".into();
        app.active_channel_archived = archived;
        app.active_channel_members_only = members_only;
        // Empty roster: she is not seated, which is what `members_only` refuses.
        app.channel_members = Vec::new();
        app.settings_user_key = "me".into();

        app.message_editor = compose("into the void");
        let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
            editor::composer_submit_event(),
        ));
        assert!(
            app.messages.is_empty(),
            "the main composer must refuse a {reason} channel at apply time"
        );
        assert!(app.pending_message.is_empty());
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
        assert!(app.pending_reply.is_empty());
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

/// A REPLY IS FORMATTABLE, through both of the doors the stream's composer has.
///
/// The rail's composer had NEITHER: no toolbar (the seat row was hint + Send),
/// and the Cmd/Ctrl chord — supposedly the keyboard half of the same table —
/// was hard-wired to `message_editor` in `handlers/overlays.ice`. The chord
/// rides the app's ONE global key subscription, which sees no widget focus, so
/// Cmd+B pressed with the caret in a thread reply wrapped the CHANNEL draft
/// instead: a silent write into a composer the user was not looking at.
///
/// Both halves are pinned because they are separate mechanisms with the same
/// failure. The toolbar half needs its MOUNTS asserted too: `ComposerMarks` is
/// one component seated twice, and the two seats must route to DIFFERENT
/// editors — a same-name `forward` would have collapsed them back onto one,
/// which is exactly the defect in miniature.
#[test]
fn a_thread_reply_takes_marks_from_its_own_toolbar_and_the_chord() {
    // THE MOUNTS. Two seats of one component, two routes.
    assert_eq!(SCREENS.matches("ComposerMarks disabled=(").count(), 2);
    assert!(SCREENS.contains("mark -> emit(composer_mark, _)"));
    assert!(SCREENS.contains("mark -> emit(reply_composer_mark, _)"));

    // THE TOOLBAR half: the rail's Bold wraps the REPLY and nothing else.
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.shell_tab = "chat".into();
    app.active_channel = "general".into();
    app.active_thread_seq = 7;
    app.message_editor = compose("channel draft");
    app.reply_editor = compose("reply draft");
    app.reply_editor
        .perform(iced::widget::text_editor::Action::SelectAll);
    let _ = app.__update(__DucktapeMessage::ReplyComposerMark("bold".into()));
    assert_eq!(reply_composer(&app), "**reply draft**");
    assert_eq!(
        composer(&app),
        "channel draft",
        "the stream draft is not the reply's"
    );

    // THE CHORD half, caret in the reply. A click into an editor arrives as a
    // composer event — that event is what stamps the focus the subscription
    // cannot read, so drive it rather than poking the field.
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.shell_tab = "chat".into();
    app.active_channel = "general".into();
    app.active_thread_seq = 7;
    let _ = app.__update(__DucktapeMessage::ReplyComposerEvent(
        editor::ComposerEvent::Apply(editor::RichAction::Edit(
            iced::widget::text_editor::Action::Move(iced::widget::text_editor::Motion::DocumentEnd),
        )),
    ));
    app.message_editor = compose("channel draft");
    app.reply_editor = compose("reply draft");
    app.reply_editor
        .perform(iced::widget::text_editor::Action::SelectAll);
    let _ = app.__update(__DucktapeMessage::GlobalKeyPressed(command_chord(
        iced::keyboard::key::Code::KeyB,
    )));
    assert_eq!(reply_composer(&app), "**reply draft**");
    assert_eq!(
        composer(&app),
        "channel draft",
        "Cmd+B in a reply is not a channel edit"
    );

    // AND IT IS NOT A BLANKET REDIRECT: the same chord with the caret back in
    // the stream's composer still marks the stream's draft, rail open or not.
    // Without this arm the asserts above would pass against a chord that had
    // simply been rewired to the reply editor.
    let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
        editor::ComposerEvent::Apply(editor::RichAction::Edit(
            iced::widget::text_editor::Action::Move(iced::widget::text_editor::Motion::DocumentEnd),
        )),
    ));
    app.message_editor = compose("channel draft");
    app.message_editor
        .perform(iced::widget::text_editor::Action::SelectAll);
    let reply_before = reply_composer(&app);
    let _ = app.__update(__DucktapeMessage::GlobalKeyPressed(command_chord(
        iced::keyboard::key::Code::KeyB,
    )));
    assert_eq!(composer(&app), "**channel draft**");
    assert_eq!(reply_composer(&app), reply_before);
}

/// One Cmd/Ctrl chord, shaped the way the keyboard subscription delivers it.
fn command_chord(code: iced::keyboard::key::Code) -> __IceKeyPress {
    __IceKeyPress {
        key: iced::keyboard::Key::Unidentified,
        modified_key: iced::keyboard::Key::Unidentified,
        physical_key: iced::keyboard::key::Physical::Code(code),
        location: iced::keyboard::Location::Standard,
        modifiers: iced::keyboard::Modifiers::COMMAND,
        text: None,
        repeat: false,
    }
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
    let components = inlined(include_str!("ui/components/chat.ice"));
    assert!(
        components
            .contains("component ChannelButton(channel:ChatChannel, selected:bool, unread:bool)")
    );
    assert!(components.contains("if unread\n                box w=7.0 h=7.0 bg=brand r=3.5"));
    // The name rides a `box w=fill clip=true`: `wrap=none` text lays out at its
    // INTRINSIC width whatever box it is given, so an unclipped long channel
    // name inflated the whole row past the 236px pane and the pane's own clip
    // sliced the row plate square through its rounded corner.
    assert!(components.contains(
        "if unread\n                box w=fill clip=true\n                  text channel.name size=13.0 wrap=none font=medium @text-fg"
    ));

    let screen = inlined(include_str!("ui/screens/chat.ice"));
    assert!(screen.contains(
        "ChannelButton channel selected=(channel.id == active_channel) unread=channel_is_unread(channel_reads, channel.id, channel.head_seq)"
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
    let lifecycle = inlined(include_str!("ui/handlers/lifecycle.ice"));
    assert!(
        lifecycle.contains("channel_reads = initial_channel_reads(next.channels, channel_reads)")
    );
    // navigation loads freeze on the real channel change (chat.ice);
    // the resync path freezes against the possibly-unchanged channel.
    let chat = inlined(include_str!("ui/handlers/chat.ice"));
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
    let backend_ice = inlined(include_str!("ui/extern/backend.ice"));
    assert!(!backend_ice.contains("read_cursor"));
    assert!(!backend_ice.contains("mark_read(rpc"));
}

/// THE "Not connected" WORDING, ONCE. Every data screen swaps its empty-state
/// claim for this exact plate, so the console reads as one app rather than eight
/// dialects of "I don't know".
const NOT_CONNECTED_PLATE: &str = concat!(
    "EmptyState title=\"Not connected\" ",
    "description=\"Click the network name in the titlebar to pick or reconnect a network.\""
);

/// A SCREEN THAT CANNOT REACH THE NODE MUST NOT REPORT ON ITS CONTENTS. With the
/// node down the status bar already says "Connection degraded · Offline", and the
/// bodies used to answer underneath it with `0 files · 0 dirs` and "Empty
/// directory — nothing is committed under this path." — a claim about CONTENT
/// made from a request that never went out. Chat and Pages already got this
/// right; the other six asserted an emptiness nobody measured.
///
/// THE SCREEN LIST IS THE INVARIANT, SO THE SCREEN LIST IS PINNED. A ninth data
/// screen has to decide what it says with the node down, and nothing about
/// writing one would prompt that thought — which is exactly how six of them got
/// written. This fails the build on a new screen so the decision is forced.
/// Exemptions are named with their reason, never left implicit.
#[test]
fn every_data_screen_answers_a_dead_node_with_not_connected() {
    /// The one screen that is NOT a reading of the network: Settings is the
    /// endpoint, the key, the appearance and the node's own diagnostics, and it
    /// is where a reader goes to fix the connection. Blanking it would hide the
    /// repair from the person doing the repairing.
    const EXEMPT: [&str; 1] = ["SettingsScreen"];

    let mut screens: Vec<&str> = SCREENS
        .lines()
        .filter_map(|line| line.strip_prefix("component "))
        .map(|rest| rest.split('(').next().unwrap_or(rest).trim())
        .filter(|name| name.ends_with("Screen"))
        .collect();
    screens.sort_unstable();

    assert_eq!(
        screens,
        [
            "AgentsScreen",
            "ChatScreen",
            "ExplorerScreen",
            "FilesScreen",
            "ForgeScreen",
            "GovernanceScreen",
            "MembersScreen",
            "PagesScreen",
            "SettingsScreen",
        ],
        "a screen appeared or vanished: decide what it says with the node down, \
         then add it here or to EXEMPT with a reason"
    );

    // Scoped to each component's OWN body — a sweep over the whole file would
    // pass on six screens off Chat's single arm.
    for screen in screens.iter().filter(|name| !EXEMPT.contains(name)) {
        let body = SCREENS
            .split(&format!("\ncomponent {screen}("))
            .nth(1)
            .unwrap_or_else(|| panic!("{screen} is a component"))
            .split("\ncomponent ")
            .next()
            .expect("component body");
        assert!(
            body.contains("if !connected\n"),
            "{screen} draws readings of a network it may not be able to reach, \
             so it needs an `if !connected` arm in place of its empty-state claim"
        );
        assert!(
            body.contains(NOT_CONNECTED_PLATE),
            "{screen} must use the shared \"Not connected\" wording verbatim"
        );
    }
}

/// AND THE PLATE IS ONLY HALF OF IT — the arms BESIDE it must stand down too.
/// The first cut gated each screen's empty-state claim and left its POPULATED
/// arms open, so a screen rendered "Not connected" and the stale register
/// underneath it at the same time: Approvals showed the plate above three vote
/// cards with live Approve buttons, Members showed it beside a detail panel
/// carrying Promote and Remove, and Explorer showed it under a strip of search
/// hits nobody had run. No register is ever cleared on disconnect — they are
/// only overwritten by a successful load — so `connected == false` is routinely
/// reached with a full set of stale rows in hand.
///
/// The invariant, stated mechanically: a screen may touch a LIST-TYPED register
/// prop only under a `connected` gate, or on a line that carries `connected`
/// itself (a header subtitle folding rows through `members_summary(connected,
/// rows)` is already honest).
#[test]
fn a_disconnected_screen_stands_its_registers_down_too() {
    const EXEMPT: [&str; 1] = ["SettingsScreen"];

    for source in [
        include_str!("ui/screens/governance.ice"),
        include_str!("ui/screens/roster.ice"),
        include_str!("ui/screens/storage.ice"),
        include_str!("ui/screens/forge.ice"),
    ] {
        for chunk in source.split("\ncomponent ").skip(1) {
            let name = chunk.split('(').next().unwrap_or("").trim();
            if !name.ends_with("Screen") || EXEMPT.contains(&name) {
                continue;
            }
            // The registers are the list-typed params of the screen's own
            // signature — the readings only a live node can deliver.
            // AFTER the opening paren: splitting the whole head on `,` leaves
            // the first param as `GovernanceScreen(rows`, whose name never
            // matches a word in the body — which silently disarmed the check
            // for every screen whose only register is its first param.
            let signature = chunk
                .split(')')
                .next()
                .unwrap_or("")
                .split_once('(')
                .map(|(_, params)| params)
                .unwrap_or("");
            let registers: Vec<&str> = signature
                .split(',')
                .filter(|param| param.contains(":["))
                .filter_map(|param| param.split(':').next())
                .map(|name| name.trim().trim_start_matches("bind "))
                .filter(|name| !name.is_empty())
                .collect();
            assert!(!registers.is_empty(), "{name} has no register to guard");

            // Walk the body tracking which `if` arms are open above each line;
            // a line is covered when it, or any arm enclosing it, says
            // `connected`.
            let mut open: Vec<(usize, bool)> = Vec::new();
            for line in chunk.lines() {
                let trimmed = line.trim_start();
                if trimmed.is_empty() || trimmed.starts_with("//") {
                    continue;
                }
                let indent = line.len() - trimmed.len();
                open.retain(|(at, _)| *at < indent);
                let says_connected = line.contains("connected");
                if trimmed.starts_with("if ") {
                    open.push((indent, says_connected));
                }
                let covered = says_connected || open.iter().any(|(_, gated)| *gated);
                if covered {
                    continue;
                }
                // Prose is not a register read: the Explorer's own subtitle
                // says "read the blocks this node verified", and a screen may
                // describe what it shows while showing nothing.
                let code: String = trimmed.split('"').step_by(2).collect::<Vec<_>>().join(" ");
                if let Some(register) = registers.iter().find(|reg| {
                    code.split(|c: char| !c.is_alphanumeric() && c != '_')
                        .any(|word| word == **reg)
                }) {
                    panic!(
                        "{name} touches `{register}` with no `connected` gate above it:\n  {trimmed}\n\
                         a register nobody could read must not be drawn beside the \"Not connected\" plate"
                    );
                }
            }
        }
    }
}

/// A DIRECTORY YOU HAVE NOT LISTED HAS NO CONTENTS TO REPORT. Measured live:
/// clicking `reports` inside `/shared` moved the crumb to
/// `duckfs /shared/reports` while the rows below it, and the `0 files · 1 dir`
/// beside the crumb, still described `/shared`. Both were `/shared`'s reading,
/// printed under `/shared/reports`'s name — and `/shared/reports` is in fact
/// empty, so every word of it was wrong.
///
/// `fs_path` is the path asked for (the crumb moves on the click, deliberately
/// — a click that repaints nothing reads as a dead app). `fs_listed_path` is
/// the path the rows describe. Same split as `active_page`/`buffer_page`.
#[test]
fn the_files_pane_reports_only_a_directory_it_has_listed() {
    let entry = |path: &str, kind: &str| backend::FsEntry {
        path: path.into(),
        name: path.rsplit('/').next().unwrap_or(path).into(),
        kind: kind.into(),
        size: 0,
        object: String::new(),
    };
    let listing = |generation: i64, path: &str, entries: Vec<backend::FsEntry>| {
        __DucktapeMessage::FsListed(backend::FsListing {
            generation,
            path: path.into(),
            entries,
        })
    };

    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    let _ = app.__update(listing(
        app.fs_generation,
        "/shared",
        vec![entry("/shared/reports", "dir")],
    ));
    assert_eq!(
        app.fs_listed_path, app.fs_path,
        "the listing answered for it"
    );
    assert_eq!(
        backend::fs_counts_summary(app.connected, true, app.fs_entries.clone()),
        "0 files · 1 dir"
    );

    // Navigate. The crumb moves at once; the rows have not.
    let _ = app.__update(__DucktapeMessage::FsOpenDir("/shared/reports".into()));
    assert_eq!(
        app.fs_path, "/shared/reports",
        "the crumb moves on the click"
    );
    assert_eq!(
        app.fs_listed_path, "/shared",
        "the rows still describe where you came from"
    );
    assert_ne!(
        app.fs_listed_path, app.fs_path,
        "`listed` is false for the whole of the navigation, and every reading \
         of `entries` on the screen is gated on it"
    );
    assert_eq!(
        backend::fs_counts_summary(app.connected, false, app.fs_entries.clone()),
        "",
        "no tally for a directory nobody has answered for"
    );

    // The answer lands and the two agree again. The directory is empty, so the
    // tally stays silent — the pane's own plate says "Empty directory" in
    // words, and a subtitle of nothing but zeros repeats it in digits.
    let _ = app.__update(listing(app.fs_generation, "/shared/reports", Vec::new()));
    assert_eq!(app.fs_listed_path, app.fs_path);
    assert_eq!(
        backend::fs_counts_summary(app.connected, true, app.fs_entries.clone()),
        ""
    );

    // A same-path refresh — what a write kicks off — must NOT blank the pane:
    // the rows on hand still describe the path in the crumb.
    let _ = app.__update(listing(
        app.fs_generation,
        "/shared/reports",
        vec![entry("/shared/reports/q3.md", "file")],
    ));
    assert_eq!(app.fs_listed_path, app.fs_path, "a refresh never disagrees");
    assert_eq!(
        backend::fs_counts_summary(app.connected, true, app.fs_entries.clone()),
        "1 file · 0 dirs",
        "and it speaks again as soon as there is something to count"
    );

    // And the screen actually gates on it, at every reading of the rows.
    let storage = inlined(include_str!("ui/screens/storage.ice"));
    let files = storage
        .split_once("component FilesScreen(")
        .expect("the screen")
        .1
        .split_once("\ncomponent ")
        .expect("the screen ends")
        .0;
    assert!(
        files.contains("listed:bool"),
        "the screen is handed the fact"
    );
    for gate in [
        "meta=fs_counts_summary(connected, listed, entries)",
        "if connected && listed && fs_dir_count(entries) <= 0",
        "if connected && listed",
        "if listed && empty(entries)",
        "if listed && !empty(entries)",
    ] {
        assert!(files.contains(gate), "ungated reading of the rows: {gate}");
    }
    let view = inlined(include_str!("ui/view.ice"));
    assert!(
        view.contains("listed=(fs_listed_path == fs_path)"),
        "the mount has to compute it"
    );
}

/// AND THE HEADER SUBTITLES ARE CLAIMS TOO — the subtler half. `Agents 0 agents ·
/// 0 working` is a measured zero about a register nobody read, and it sits ABOVE
/// the body arm above, so it survives it. Every subtitle fold takes `connected`
/// as its first argument and returns "" without it; this pins that no call site
/// can quietly drop the guard.
#[test]
fn every_header_subtitle_is_gated_on_the_connection() {
    let sites: Vec<&str> = SCREENS
        .match_indices("_summary(")
        .map(|(at, _)| {
            let head = SCREENS[..at]
                .rfind(|c: char| !c.is_alphanumeric() && c != '_')
                .map_or(0, |before| before + 1);
            let close = at + SCREENS[at..].find(')').expect("a call site closes");
            &SCREENS[head..=close]
        })
        .collect();

    assert_eq!(
        sites,
        [
            "proposals_summary(connected, rows)",
            "members_summary(connected, rows)",
            "agents_summary(connected, rows)",
            "members_summary(connected, members_rows)",
            "fs_counts_summary(connected, listed, entries)",
        ],
        "a header subtitle folds rows only a live node delivers: pass `connected` \
         first so it says nothing rather than a confident zero"
    );
}

/// THE CONSOLE HEALS ITSELF FROM A CONNECT FAILURE. The steady-state path has
/// always retried forever (`live_resync_failed`), so the app recovered from
/// every interruption except the one that gets it running: `on failed` set
/// Offline and stopped, leaving the console dead against a node answering
/// `/v1/status` in under a millisecond, with no way back but the network
/// picker. Issue #1018 makes that failure ordinary — a `/v1/query` can block
/// until the node writes its next checkpoint, outlasting the client's 30s
/// timeout.
#[test]
fn a_failed_connect_retries_instead_of_giving_up() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    let before = app.connect_generation;

    let fail = |generation: i64| {
        __DucktapeMessage::ConnectFailed(backend::HydrationError {
            generation,
            message: "error sending request".into(),
        })
    };
    let _ = app.__update(fail(app.connect_generation));
    assert_eq!(
        app.hydration_retry_attempt, 1,
        "the first failure is attempt 1"
    );
    assert_eq!(app.status, "Offline", "and it is offline while it retries");
    assert!(
        app.connect_generation > before,
        "each attempt owns a generation, so an abandoned one cannot answer"
    );

    // The counter CLIMBS — that is what feeds the backoff. A reset here would
    // retry at 1s forever against a genuinely dead endpoint.
    let _ = app.__update(fail(app.connect_generation));
    let _ = app.__update(fail(app.connect_generation));
    assert_eq!(app.hydration_retry_attempt, 3);

    // A CONNECT IS NOT GUARDED ON `hydration_generation`, AND THIS IS WHY.
    // Thirty-seven handlers bump that counter for reasons of their own —
    // `choose_channel` is one of them — and a connect is in flight for seconds,
    // or for up to 30s while the node sits in issue #1018's checkpoint stall.
    // Guarded on the shared counter, one click on a channel mid-connect drops
    // the successful reply; because it SUCCEEDED no failure arm fires and
    // nothing retries, so the console sits Offline forever. Strictly worse than
    // the defect this PR fixes.
    let (mut wired, _) = Ducktape::__boot();
    wired.connected_rpc = "http://127.0.0.1:38259".into();
    let connect_gen = wired.connect_generation;
    let shared_before = wired.hydration_generation;
    let _ = wired.__update(__DucktapeMessage::ChooseChannel("general".into()));
    assert!(
        wired.hydration_generation > shared_before,
        "an ordinary channel click bumps the SHARED counter"
    );
    assert_eq!(
        wired.connect_generation, connect_gen,
        "and leaves the connect's own alone — only the three routes that start a connect may touch it"
    );

    // AND A FAILURE FROM AN ABANDONED CHAIN IS DROPPED UNREAD. Without this,
    // two chains retry forever and each one's generation bump can reject the
    // other's success — measured live as two interleaved retry series 5.2s and
    // 10.8s apart, summing to one 16s cap.
    let stale = app.connect_generation - 1;
    let attempts = app.hydration_retry_attempt;
    let _ = app.__update(fail(stale));
    assert_eq!(
        app.hydration_retry_attempt, attempts,
        "an abandoned chain must not start a second retry loop"
    );

    // The handler re-runs the connect, and the reply is generation-guarded.
    let lifecycle = inlined(include_str!("ui/handlers/lifecycle.ice"));
    let arm = lifecycle
        .split_once("on connect_failed(cause)")
        .expect("connect owns its failure arm, not the shared one")
        .1
        .split_once("\non ")
        .expect("the arm ends")
        .0;
    assert!(
        arm.contains("hydration_retry_attempt = hydration_retry_attempt + 1"),
        "the attempt climbs"
    );
    assert!(
        arm.contains(
            "run connect(connected_rpc, hydration_retry_attempt, connect_generation) -> workspace_connected _ | connect_failed _"
        ),
        "and it goes round again, carrying the attempt into the backoff"
    );
    // Scoped to the ARM, not to the rest of the file: `live_resynced` further
    // down guards on `hydration_generation` and is right to — that one really
    // is the live plane's counter.
    let connected_rest = lifecycle
        .split_once("on workspace_connected(next)")
        .expect("the success arm")
        .1;
    let connected = connected_rest
        .split_once("\non ")
        .map_or(connected_rest, |(arm, _)| arm);
    // NO `||` HERE. An alternative that is trivially true short-circuits the
    // half that matters — the first version of this assertion accepted the
    // presence of a COMMENT and stayed green with the guard pointed back at the
    // shared counter, which is the wedge this test exists to prevent.
    assert!(
        connected.contains("return if next.generation != connect_generation"),
        "the connect is guarded on its OWN generation, never the shared one"
    );
    assert!(
        !connected.contains("return if next.generation != hydration_generation"),
        "the shared counter is bumped by 37 handlers; guarding on it drops a \
         successful connect and nothing retries"
    );

    // The SHARED `failed` arm still belongs to the six page/chat loaders that
    // route to it, and must NOT have grown a connect retry.
    // `on failed` is the LAST handler in the file, so there may be no `\non `
    // after it to cut at — take the remainder when there is not.
    let shared_rest = lifecycle
        .split_once("on failed(cause)")
        .expect("the shared arm")
        .1;
    let shared = shared_rest
        .split_once("\non ")
        .map_or(shared_rest, |(arm, _)| arm);
    assert!(
        !shared.contains("run connect("),
        "a failed page load must not restart the workspace connect"
    );
}

/// THE BEHAVIOUR THE SWEEPS ABOVE ONLY SPELL. Boot the console, drop the
/// connection, and the four subtitles go silent instead of reporting the zeros
/// an unfetched listing folds to.
#[test]
fn a_disconnected_console_reports_no_counts_at_all() {
    let (mut app, _) = Ducktape::__boot();
    app.members_rows = vec![backend::MemberRow {
        key: "aa".into(),
        label: "aa".into(),
        role: "validator".into(),
        is_this_node: true,
        is_agent: false,
        model: String::new(),
        live: true,
    }];
    app.fs_entries = vec![backend::FsEntry {
        path: "/shared/notes".into(),
        name: "notes".into(),
        kind: "file".into(),
        size: 0,
        object: String::new(),
    }];

    app.connected = true;
    assert_eq!(
        backend::members_summary(app.connected, app.members_rows.clone()),
        "1 human · 0 agents"
    );
    assert_eq!(
        backend::fs_counts_summary(app.connected, true, app.fs_entries.clone()),
        "1 file · 0 dirs"
    );
    // The two registers this boot leaves EMPTY are silent while connected too —
    // an all-zero subtitle repeats, in digits, the plate that already said
    // "No agents registered" / "No proposals yet". `a_subtitle_that_is_all_zeros_
    // says_nothing_at_all` (backend/tests.rs) is where the speaking case is
    // proved with real rows; here they are empty on purpose.
    assert_eq!(
        backend::agents_summary(app.connected, app.agents_rows.clone()),
        ""
    );
    assert_eq!(
        backend::proposals_summary(app.connected, app.gov_rows.clone()),
        ""
    );

    // The node goes down. Everything above was a reading; none of it is one now.
    app.connected = false;
    for (screen, meta) in [
        (
            "Members",
            backend::members_summary(app.connected, app.members_rows.clone()),
        ),
        (
            "Agents",
            backend::agents_summary(app.connected, app.agents_rows.clone()),
        ),
        (
            "Approvals",
            backend::proposals_summary(app.connected, app.gov_rows.clone()),
        ),
        (
            "Files",
            backend::fs_counts_summary(app.connected, true, app.fs_entries.clone()),
        ),
    ] {
        assert_eq!(
            meta, "",
            "{screen} printed `{meta}` off a node that answered nothing"
        );
    }
}
