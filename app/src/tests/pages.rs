use super::*;

#[test]
fn background_refresh_preserves_editing_state() {
    let _turn = crate::module_view::tests::blocking_connection_turn();
    let root = inlined(include_str!("../ui/app.ice"));
    let view = inlined(include_str!("../ui/view.ice"));
    let lifecycle = inlined(include_str!("../ui/handlers/lifecycle.ice"));
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
    // THE INLINE EDIT IS THE CHAT VIEW'S NOW, and so is the row it was armed
    // on: a resync carries the room, and the view re-reads it. What the app
    // still holds of an edit in flight is the row it named, which a resync
    // that moved the room cannot keep pointing at.
    assert!(!refresh.contains("message_edit_draft"));
    assert!(lifecycle.contains("run live_events(connected_rpc) when connected"));
    assert_no_polling(&lifecycle);
    assert!(lifecycle.contains("run replace lane=live_resync live_resync_load(connected_rpc"));
    // Page-scoped state waits for a reply that answers for the page in hand —
    // a resync issued before a mutation moved the selection speaks for a
    // document nobody is on. And the fold-owned fields (#1041) additionally
    // wait for a reply no text fold outran: the title and the row titles keep
    // the fold's value when the serial moved, while the structural half still
    // lands from the reply.
    assert!(lifecycle.contains(
        "active_page_title = keep_str(pages_answer_is_current && !pages_fold_outran_reply, next.active_page_title, active_page_title)"
    ));
    assert!(lifecycle.contains(
        "let pages_answer_is_current = next.pages_loaded && pages_reply_answers_current(next.pages, next.active_page, active_page)"
    ));
    assert!(
        lifecycle.contains("let pages_fold_outran_reply = next.fold_serial != pages_fold_serial")
    );
    // The fold site is the ONE writer of the serial: a text fold bumps it, and
    // every resync request snapshots it, or the token guards nothing.
    assert!(lifecycle.contains(
        "pages_fold_serial = keep_i64(pages_delta_folds(next.pages), pages_fold_serial + 1, pages_fold_serial)"
    ));
    // The page LIST's structure is never stale — it is the whole index either
    // way — but shared rows keep their folded titles.
    assert!(lifecycle.contains(
        "pages = keep_pages(next.pages_loaded, keep_folded_page_titles(pages_fold_outran_reply, next.pages, pages), pages)"
    ));
    assert!(lifecycle.contains(
        "blocks = keep_blocks(pages_answer_is_current, merge_pending_blocks(keep_folded_block_texts(pages_fold_outran_reply, next.blocks, blocks), blocks, buffer_page, next.active_page, \"\"), blocks)"
    ));
    // A live resync must never install remote text over a buffer the user is
    // still typing in; the buffer and its dirty baseline move on ONE decision.
    assert!(lifecycle.contains("page_text = refreshed_page_buffer("));
    assert!(lifecycle.contains("page_saved_text = resynced_saved"));
    // the comment rail is scoped to the PAGE it hangs off, so its draft
    // survives moving the cursor between blocks and dies with the page.
    assert!(lifecycle.contains(
        "block_comment_draft = retain_selected_string(block_comment_draft, block_comments_target)"
    ));
    // The list callback installs what landed and asks for nothing more: the
    // one query already carried every thread WITH its comments.
    let pages_handlers = inlined(include_str!("../ui/handlers/pages.ice"));
    let comment_callbacks = pages_handlers
        .split_once("on block_threads_loaded(next)\n")
        .unwrap()
        .1
        .split_once("\non block_threads_failed")
        .unwrap()
        .0;
    assert!(!comment_callbacks.contains("run "));
    assert!(comment_callbacks.contains("commented_block_hits = commented_targets_of("));
}

#[test]
fn context_destroying_page_handlers_recover_drafts() {
    let _turn = crate::module_view::tests::blocking_connection_turn();
    let pages = inlined(include_str!("../ui/handlers/pages.ice"));
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

/// THE STREAM'S LOAD FLAG IS NOT THE RAIL'S, AND THE RAIL'S SEND SAID SO.
///
/// The reply submit refused on `loading` — a term neither the reply editor,
/// its marks row nor its Send button wears — so in the one state that can
/// raise it under an open rail the reader saw a fully lit Send, pressed it,
/// and got nothing: no post, no error, no banner. Every chat-plane writer of
/// `loading = true` closes the room the rail belongs to, so the term never
/// fired for a chat load at all; the state it caught was a PAGES load still in
/// flight behind a cross-tab bounce.
#[test]
fn a_pages_load_in_flight_does_not_deaden_the_lit_reply_send() {
    let _turn = crate::module_view::tests::blocking_connection_turn();
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.active_channel = "general".into();
    // What `open_page_search_hit` leaves behind: the stream's flag up, the rail
    // untouched, and `select_shell_tab` back to Chat clears neither.
    app.loading = true;
    submit(&mut app, ComposerKind::Reply, "on it");
    assert_eq!(
        app.chat_pending_sends.len(),
        1,
        "a Send the surface draws as live must actually send"
    );
    assert_eq!(app.chat_pending_sends[0].thread_seq, RAIL_THREAD_SEQ);
}

// The dirty gate makes the tick FIRE; these two guards make it WAIT. An
// in-flight op chain must finish before the next starts (the awaited loop is
// the ordering rule), and an open ``` must be closed before the buffer is
// parsed — otherwise everything under it reads as one code block and the plan
// removes the "vanished" lines.
#[test]
fn the_save_tick_waits_for_inflight_saves_and_open_fences() {
    let _turn = crate::module_view::tests::blocking_connection_turn();
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.connected = true;
    app.active_page = "page".into();
    // The buffer is this page's — the tick refuses one that is not.
    app.buffer_page = "page".into();
    app.page_text = ("Title\nfresh body").to_string();
    app.page_saved_text = "Title\nstale".into();
    app.block_autosave_status = AutosaveStatus::Saving;

    let _ = app.__update(__DucktapeMessage::PageAutosaveTick);
    assert_eq!(
        app.block_autosave_status,
        AutosaveStatus::Saving,
        "inflight guard"
    );

    app.block_autosave_status = AutosaveStatus::Idle;
    app.page_text = ("Title\n```\nstill typing").to_string();
    let _ = app.__update(__DucktapeMessage::PageAutosaveTick);
    assert_eq!(
        app.block_autosave_status,
        AutosaveStatus::Idle,
        "fence guard"
    );

    app.page_text = ("Title\n```\ndone\n```").to_string();
    let _ = app.__update(__DucktapeMessage::PageAutosaveTick);
    assert_eq!(app.block_autosave_status, AutosaveStatus::Saving);
}

#[test]
fn page_autosave_freshness_is_compiler_owned_without_aborting_writes() {
    let _turn = crate::module_view::tests::blocking_connection_turn();
    let pages = inlined(include_str!("../ui/handlers/pages.ice"));
    assert!(pages.contains(
        "run latest lane=page_autosave save_page_document(connected_rpc, password, active_page, text, page_saved_text) -> page_document_saved _ | page_document_save_failed _"
    ));
    assert!(!pages.contains("run replace lane=page_autosave"));
    assert_eq!(pages.matches("invalidate lane=page_autosave").count(), 4);

    let lifecycle = inlined(include_str!("../ui/handlers/lifecycle.ice"));
    assert_eq!(
        lifecycle.matches("invalidate lane=page_autosave").count(),
        2
    );
    let onboarding = inlined(include_str!("../ui/handlers/onboarding.ice"));
    assert_eq!(
        onboarding.matches("invalidate lane=page_autosave").count(),
        3
    );
}

/// TICK TWO MUST NOT REVERT WHAT TICK ONE CORRECTLY LEFT ALONE.
///
/// The predicate alone survives exactly one tick. A save that writes body ops
/// comes back with the node's canonical text — carrying a rename someone else
/// made — and the handler adopts it as the baseline while deliberately leaving
/// the dirty buffer's stale line 0 in place. That manufactures authorship out
/// of nothing, and the NEXT tick writes the old name back on chain.
///
/// Driven through the handler, two ticks, on the fixture #1032 uses for the
/// same collision: a reader mid-sentence whose page is renamed under her.
#[test]
fn a_save_that_lands_body_ops_does_not_manufacture_a_rename_next_tick() {
    let _turn = crate::module_view::tests::blocking_connection_turn();
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.active_page = "page".into();
    app.buffer_page = "page".into();
    // she is mid-sentence; her line 0 is the OLD name and she never touched it.
    app.page_text = ("Old Name\nbody mid-sentence").to_string();
    app.page_saved_text = "Old Name\nbody".into();
    // WHAT THE TICK ACTUALLY SUBMITTED. The correction reads this, not the live
    // buffer, so leaving it at its default empty string would hand the baseline
    // an empty title and prove nothing about the case under test.
    app.page_inflight_text = "Old Name\nbody mid-sentence".into();

    // the save landed her body edit. The node's canonical text carries the
    // other person's rename, which her buffer has never shown.
    let _ = app.__update(__DucktapeMessage::PageDocumentSaved(
        backend::DocumentSaveResult {
            written: true,
            refusal: String::new(),
            document: "New Name\nbody mid-sentence".into(),
            data: backend::PagesData {
                pages: vec![page_item("page", "New Name")],
                blocks: vec![page_block("b1", "page", "body mid-sentence")],
                active_page: "page".into(),
                active_page_title: "New Name".into(),
                active_page_parent: String::new(),
                comment_thread_total: 0,
                commented_block_hits: Vec::new(),
            },
        },
    ));

    // the label follows the chain — that half is right and stays right.
    assert_eq!(app.active_page_title, "New Name");
    // THE BASELINE KEEPS HER LINE 0. Adopting "New Name" here is what made the
    // next tick believe she had retitled the page.
    assert_eq!(
        app.page_saved_text, "Old Name\nbody mid-sentence",
        "the baseline may not claim a title the buffer never showed"
    );
    // and with buffer and baseline agreeing at line 0, the document is clean:
    // the tick does not even fire, so no rename can be planned from it.
    assert_eq!(
        page_document_text(&app),
        app.page_saved_text,
        "no manufactured dirt at line 0"
    );
    let _ = app.__update(__DucktapeMessage::PageAutosaveTick);
    assert_eq!(
        app.block_autosave_status,
        AutosaveStatus::Saved,
        "tick two must plan nothing — there is nothing of hers left unsaved"
    );
}

/// A RENAME TYPED DURING THE ROUND TRIP MUST STILL REACH THE CHAIN.
///
/// The correction has to use the text the tick actually reconciled against the
/// node, never the live buffer — she keeps typing while the save is in flight.
/// Feeding the live buffer adopts characters she has not saved into the
/// baseline, which makes the document read CLEAN, retires the tick that owed
/// the node her rename, and lets the next live fold rebuild the buffer and
/// erase what she typed. Worse than the bug this file exists to fix.
#[test]
fn a_title_typed_during_the_round_trip_is_not_swallowed_by_the_baseline() {
    let _turn = crate::module_view::tests::blocking_connection_turn();
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.active_page = "page".into();
    app.buffer_page = "page".into();
    app.page_text = ("Notes\nhello").to_string();
    app.page_saved_text = "Notes\nhello".into();

    // the tick submits what it can see.
    app.page_inflight_text = "Notes \nhello".into();

    // SHE FINISHES THE WORD while the save is in flight.
    app.page_text = ("Notes A\nhello").to_string();

    // the save was a no-op — the trimmed title still matched the node.
    let _ = app.__update(__DucktapeMessage::PageDocumentSaved(
        backend::DocumentSaveResult {
            written: false,
            refusal: String::new(),
            document: "Notes\nhello".into(),
            data: backend::PagesData {
                pages: vec![page_item("page", "Notes")],
                blocks: vec![page_block("b1", "page", "hello")],
                active_page: "page".into(),
                active_page_title: "Notes".into(),
                active_page_parent: String::new(),
                comment_thread_total: 0,
                commented_block_hits: Vec::new(),
            },
        },
    ));

    assert_ne!(
        page_document_text(&app),
        app.page_saved_text,
        "her unsaved rename must leave the document DIRTY — a clean one retires \
         the tick that owes the node that rename, and it is never written"
    );
    let _ = app.__update(__DucktapeMessage::PageAutosaveTick);
    assert_eq!(
        app.block_autosave_status,
        AutosaveStatus::Saving,
        "the next tick must plan her rename"
    );
}

/// The refusal path takes the same correction, and nothing pinned it: deleting
/// that call site left every test in this change green while a refused write
/// plus a remote rename reverted the rename on the next tick.
#[test]
fn a_refused_write_does_not_hand_the_baseline_someone_elses_title() {
    let _turn = crate::module_view::tests::blocking_connection_turn();
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.active_page = "page".into();
    app.buffer_page = "page".into();
    app.page_text = ("Old Name\nbody typed on").to_string();
    app.page_saved_text = "Old Name\nbody".into();
    app.page_inflight_text = "Old Name\nbody typed".into();

    // the node refused the body op, and its text carries someone else's rename.
    let _ = app.__update(__DucktapeMessage::PageDocumentSaved(
        backend::DocumentSaveResult {
            written: false,
            refusal: "that edit would destroy comments".into(),
            document: "New Name\nbody".into(),
            data: backend::PagesData {
                pages: vec![page_item("page", "New Name")],
                blocks: vec![page_block("b1", "page", "body")],
                active_page: "page".into(),
                active_page_title: "New Name".into(),
                active_page_parent: String::new(),
                comment_thread_total: 0,
                commented_block_hits: Vec::new(),
            },
        },
    ));

    assert_eq!(
        app.page_saved_text, "Old Name\nbody",
        "the baseline keeps the title she submitted — adopting the node's makes \
         the next tick believe she renamed the page and revert the other rename"
    );
}

/// A FAILED LOAD MUST NOT LET THE BLANK PANE EAT THE PAGE IT NEVER OPENED.
/// The optimistic switch moves `active_page` and blanks the buffer before the
/// round trip. If the load then FAILS, `on failed` clears `loading` without
/// clearing `connected` or putting `active_page` back — so the reader is left
/// looking at an empty, fully typable document under the new page's title.
///
/// One keystroke there used to reach the 900ms save tick, which wrote
/// `editor_text(page_editor)` into `active_page`. Saving an empty document
/// against a real page is a `RemoveBlock` for every line it had: the page would
/// be destroyed by the act of failing to open it, and the reader would never
/// have seen a line of it.
#[test]
fn a_failed_page_load_cannot_save_the_blank_pane_over_the_page() {
    let _turn = crate::module_view::tests::blocking_connection_turn();
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

    app.page_text = ("h").to_string();
    let _ = app.__update(__DucktapeMessage::PageAutosaveTick);

    // The tick must refuse: the buffer is not Beta's.
    assert_eq!(
        app.block_autosave_status,
        AutosaveStatus::Idle,
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
    let _turn = crate::module_view::tests::blocking_connection_turn();
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

// `buffer_page`, not `active_page`, is what the install decision compares. A
// live resync moves `active_page` while a DIRTY buffer stays on the page it
// came from — read against `active_page` the landing document is a same-page
// refresh, the dirty buffer refuses it, and Beta opens showing Alpha's text.
#[test]
fn the_landing_document_installs_when_the_page_actually_moved() {
    let _turn = crate::module_view::tests::blocking_connection_turn();
    let mut app = reading_alpha();
    app.page_text = ("Alpha\nalpha body, still typing").to_string();
    app.active_page = "beta".into();
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
    let _turn = crate::module_view::tests::blocking_connection_turn();
    let mut app = reading_alpha();
    app.page_text = ("Alpha\nalpha body, still typing").to_string();

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

/// Opening comments preserves the document layout and leaves input outside
/// the floating card available.
#[test]
fn block_comments_float_a_card_over_the_document() {
    let _turn = crate::module_view::tests::blocking_connection_turn();
    // the pages screen is the `pages` view's now (crates/views/pages).
    let pages = inlined(include_str!("../../../crates/views/pages/src/ui/pages.ice"));
    // THE LAYER IS THE CARD. A stack lays every layer out at its own top-left,
    // so the card's natural place is the pane's corner and one native float
    // carries it from there — to the right edge, or onto the text column. The
    // sensor around it measures the card the inline gap is sized from, and
    // there is still no modal backdrop.
    let card = pages
        .split_once("if connected && !empty(active_page) && block_comments_open\n")
        .unwrap()
        .1;
    let mut opening = card.lines().map(str::trim);
    assert_eq!(
        opening.next(),
        Some(
            "sensor show=emit(measure_comments_card, _, _) resize=emit(measure_comments_card, _, _)"
        )
    );
    assert_eq!(
        opening.next(),
        Some("float x=((viewport_x + viewport_width - original_x - original_width) * comments_right_anchor + comments_left_inset) y=comments_offset")
    );
    assert_eq!(
        opening.next(),
        Some(
            "box #comments-card w=comments_card_width(pane_width) h=shrink max-h=comments_height bg=elevated r=12.0 shadow=shadow_popover shadow-y=8.0 shadow-blur=24.0 border=separator border-w=1.0 clip=true"
        )
    );
    assert!(!pages.contains("w=306.0"));
    // The pane the placement is read off is measured, not assumed: the page
    // list is the reader's to size and the card answers what it leaves.
    assert!(pages.contains("sensor #pane-measure show=emit(resize_pane, _, _) resize=emit(resize_pane, _, _)"));
    assert!(pages.contains("max-w=document_width(pane_width, block_comments_open)"));
    // A STACK LAYER DECLARED AFTER THE DOCUMENT ARM, or it would paint under
    // the text it covers and the editor would own the pointer through it. The
    // delete confirm still outranks it: that one IS an overlay with a scrim.
    let layer = pages.find("if connected && !empty(active_page) && block_comments_open");
    assert!(pages.find("slot document") < layer);
    assert!(layer < pages.find("overlay when=page_delete_armed"));
    assert!(!pages.contains("close_block_comments backdrop=transparent"));
    assert!(pages.contains("-> emit(close_block_comments)"));
    assert!(pages.contains("#page-comment(active_page)"));
    assert!(!pages.contains("button \"Save\""));
    assert!(!pages.contains("Saving"));

    // The control is a DOCUMENT ACTION in the header now, not a row buried in
    // a per-block menu — the card was always page-scoped.
    assert!(pages.contains("button label=\"Comments\""));
    assert!(pages.contains("-> emit(toggle_block_comments)"));
    let components = inlined(include_str!("../../../crates/views/pages/src/ui/rows.ice"));
    assert!(!components.contains("component BlockActionsMenu"));

    // ONE SCOPE, NO DRILL-DOWN. The card lists that scope's threads expanded,
    // each with its own Resolve and its own reply box; there is no row that
    // opens a thread and no second load behind one.
    assert!(!pages.contains("open_block_comment_thread"));
    assert!(!pages.contains("load_more_block_comments"));
    assert!(components.contains("component PageCommentThreadCard("));
    assert!(components.contains("-> emit(resolve_thread_submit, thread.id, true)"));
    assert!(components.contains("-> emit(post_thread_reply, thread.id)"));
    assert!(components.contains("#thread-reply(thread.id)"));
    // The page's own group header is text; a block's is the way into its scope.
    assert!(pages.contains("-> emit(narrow_comment_scope, comment_group.target)"));
    assert!(pages.contains("-> emit(widen_comment_scope)"));

    let handlers = inlined(include_str!("../ui/handlers/pages.ice"));
    assert!(handlers.contains("on post_block_comment_submit(thread_id)"));
    // A REPLY inherits its thread's anchor; the composer takes the card's
    // scope. One resolver decides, and refuses a thread the list has lost.
    assert!(handlers.contains(
        "let post_target = comment_post_target(block_comment_threads, thread_id, scope_target)"
    ));
    assert!(handlers.contains("return if empty(post_target)"));
    assert!(handlers.contains(
        "run every post_block_comment(connected_rpc, password, post_target, thread_id, pending_block_comment)"
    ));
    // Narrowing and widening re-slice threads already in hand: no round trip.
    for scope_handler in [
        "on narrow_block_comments(target)",
        "on widen_block_comments",
    ] {
        let rest = handlers.split_once(scope_handler).unwrap().1;
        let body = rest.split_once("\non ").map_or(rest, |(body, _)| body);
        assert!(!body.contains("run "), "{scope_handler} must not reload");
        assert!(body.contains("block_comment_rows = "), "{scope_handler}");
    }
    // The guest editor shares the screen's document slot and keeps comment
    // counts in its declarative presentation, under the comments card.
    let guest_source = include_str!("../../../crates/views/pages/src/ui/app.ice");
    assert!(guest_source.contains("editor #document <-> document -> document_committed _"));
    let guest = inlined(guest_source);
    assert!(guest.contains("document_marks = next.comment_marks"));
    let view = inlined(include_str!("../ui/view.ice"));
    assert!(view.contains(", blocks, commented_block_hits, orphaned_comment_drafts,"));
    assert!(view.contains(", inline_comment_target, block_comments_pinned,"));
}

/// A REPLY INHERITS ITS THREAD'S ANCHOR; the composer takes the card's scope.
///
/// The node validates the `(thread_id, target)` pair, so a reply to a
/// block-anchored thread posted with the page id is refused outright — and a
/// thread id the list no longer carries must post NOWHERE rather than land on
/// the scope as if it were a new thread, which would silently split a
/// conversation the reader thought she was answering.
#[test]
fn a_reply_takes_its_threads_anchor_and_a_new_thread_takes_the_scope() {
    let _turn = crate::module_view::tests::blocking_connection_turn();
    let thread = |id: &str, target: &str| backend::PageCommentThread {
        id: id.into(),
        target: target.into(),
        author: "user".into(),
        meta: "1 comment".into(),
        resolved: false,
        comment_count: 1,
        comments: Vec::new(),
    };
    let threads = vec![thread("on-seven", "block-7"), thread("on-page", "page")];
    assert_eq!(
        backend::comment_post_target(threads.clone(), "on-seven".into(), "page".into()),
        "block-7"
    );
    assert_eq!(
        backend::comment_post_target(threads.clone(), "on-page".into(), "block-7".into()),
        "page"
    );
    // No thread id: the card's scope, whatever it is showing.
    assert_eq!(
        backend::comment_post_target(threads.clone(), String::new(), "block-7".into()),
        "block-7"
    );
    assert_eq!(
        backend::comment_post_target(threads.clone(), String::new(), "page".into()),
        "page"
    );
    // A thread that left the list between the render and the press.
    assert_eq!(
        backend::comment_post_target(threads, "gone".into(), "page".into()),
        ""
    );
}

/// A REMOTE RENAME REACHES LINE 0, WHICH IS WHERE THE SAVE READS THE TITLE.
///
/// `UpdateText` on the page's own block is the rename op, and it classifies as
/// `text` like any body edit — so it folds, and nothing reloads. Before the
/// title fold it landed nowhere at all: `apply_page_text` cannot see the page
/// head (the block list drops it), so the reader kept the old name on screen
/// AND in buffer line 0. Their next keystroke then ran `save_page_document`,
/// which reads the node fresh, found line 0 disagreeing with the node's new
/// title, and wrote the OLD one back — reverting someone else's rename on
/// chain, with nothing on screen.
///
/// Asserting the buffer, not just the label, is the point: line 0 is the only
/// copy of the title the save ever reads.
#[test]
fn a_folded_rename_moves_the_title_the_page_row_and_line_zero() {
    let _turn = crate::module_view::tests::blocking_connection_turn();
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_page = "page".into();
    app.buffer_page = "page".into();
    app.active_page_title = "Old Name".into();
    app.pages = vec![page_item("page", "Old Name"), page_item("other", "Other")];
    app.blocks = vec![page_block("b1", "page", "body")];
    // A CLEAN buffer: baseline and buffer agree, so the rebuild is allowed.
    app.page_text = ("Old Name\nbody").to_string();
    app.page_saved_text = "Old Name\nbody".into();
    let before = app.hydration_generation;

    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: LiveKind::Pages,
        status: "Live".into(),
        height: 11,
        module: "pages".into(),
        pages: backend::PagesDelta {
            kind: "text".into(),
            block_id: "page".into(),
            text: "New Name".into(),
        },
        ..backend::LiveUpdate::default()
    }));

    assert_eq!(app.active_page_title, "New Name", "the open page's title");
    assert_eq!(app.pages[0].title, "New Name", "and its row in the list");
    assert_eq!(app.pages[1].title, "Other", "and only its row");
    assert_eq!(
        page_document_text(&app),
        "New Name\nbody",
        "line 0 is the title the save reads — a stale one writes it back over the rename"
    );
    assert_eq!(
        app.page_saved_text, "New Name\nbody",
        "the baseline moves with the buffer, or the next save plans a title change nobody made"
    );
    assert_eq!(
        app.hydration_generation, before,
        "a rename still folds — it must not buy back the reload this PR removed"
    );
}

/// The dirty-buffer rule is UNCHANGED by the title fold: a reader mid-sentence
/// keeps their words and their caret. The label and the list still move (they
/// are not the reader's text), but the buffer does not.
#[test]
fn a_folded_rename_never_overwrites_a_dirty_buffer() {
    let _turn = crate::module_view::tests::blocking_connection_turn();
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_page = "page".into();
    app.buffer_page = "page".into();
    app.active_page_title = "Old Name".into();
    app.pages = vec![page_item("page", "Old Name")];
    app.blocks = vec![page_block("b1", "page", "body")];
    // DIRTY: she has typed since the last save.
    app.page_text = ("Old Name\nbody mid-sentence").to_string();
    app.page_saved_text = "Old Name\nbody".into();

    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: LiveKind::Pages,
        status: "Live".into(),
        height: 12,
        module: "pages".into(),
        pages: backend::PagesDelta {
            kind: "text".into(),
            block_id: "page".into(),
            text: "New Name".into(),
        },
        ..backend::LiveUpdate::default()
    }));

    assert_eq!(
        page_document_text(&app),
        "Old Name\nbody mid-sentence",
        "her buffer is hers until she saves"
    );
    assert_eq!(
        app.active_page_title, "New Name",
        "the title itself still moved — it is not part of her unsaved text"
    );
}

/// A COMMITTED EDIT LANDS WITHOUT RE-READING THE DOCUMENT IT LANDED IN.
///
/// The page autosave commits one `UpdateText` per tick while a reader types,
/// and every one used to set `load_pages` — buying a `live_resync_load` and its
/// three sequential queries, against a read path that is checkpoint-gated. Your
/// own keystrokes came back on your own stream and made you re-read the page
/// you were typing into.
///
/// `hydration_generation` is the reload's own counter, so an unchanged one IS
/// the assertion that nothing was fetched.
#[test]
fn a_folded_text_edit_updates_the_block_and_fetches_nothing() {
    let _turn = crate::module_view::tests::blocking_connection_turn();
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_page = "page".into();
    app.buffer_page = "page".into();
    // THE TITLE IS HERE TO PROVE A BODY EDIT CANNOT MOVE IT. `apply_page_title`
    // rests entirely on `delta.block_id == active_page`; drop that term and
    // every body edit renames the open page — on chain, via line 0 — which is
    // the bug this fold exists to fix, pointed the other way. Nothing else in
    // the suite constrains it.
    app.active_page_title = "Doc".into();
    app.blocks = vec![
        page_block("b1", "page", "old"),
        page_block("b2", "page", "untouched"),
    ];
    let before = app.hydration_generation;

    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: LiveKind::Pages,
        status: "Live".into(),
        height: 9,
        module: "pages".into(),
        pages: backend::PagesDelta {
            kind: "text".into(),
            block_id: "b1".into(),
            text: "typed".into(),
        },
        ..backend::LiveUpdate::default()
    }));

    assert_eq!(
        app.blocks[0].text, "typed",
        "the edit folded into its block"
    );
    assert_eq!(app.blocks[1].text, "untouched", "and only into its block");
    assert_eq!(
        app.active_page_title, "Doc",
        "a body edit must never move the page's title"
    );
    assert_eq!(
        app.hydration_generation, before,
        "a folded edit must not start a reload — that is the whole point"
    );

    // A block this document does not hold belongs to another page. Fold
    // nothing, fetch nothing.
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: LiveKind::Pages,
        status: "Live".into(),
        height: 10,
        module: "pages".into(),
        pages: backend::PagesDelta {
            kind: "text".into(),
            block_id: "elsewhere".into(),
            text: "another page".into(),
        },
        ..backend::LiveUpdate::default()
    }));
    assert_eq!(app.blocks[0].text, "typed");
    assert_eq!(app.hydration_generation, before);
}

/// THE RACE #1041 RECORDS: a fold is not reverted by a reload that was
/// already in flight when it landed.
///
/// A fold does not bump `hydration_generation` — folding instead of reloading
/// is its whole point — so a `live_resync_load` issued BEFORE the fold still
/// passes `live_resynced`'s generation guard when it answers AFTER it,
/// carrying a pre-fold snapshot: the sidebar row, the header title and line 0
/// all reverted, and stayed reverted until the next structural op on the page
/// happened to buy a fresh read. The fold serial is the ordering token the
/// reply must clear — and it gates ONLY the fold-owned fields, so the reply
/// still delivers the structural change it was issued for. Neither staleness
/// is traded for the other.
#[test]
fn a_fold_landing_during_a_resync_flight_is_not_reverted_by_the_reply() {
    let _turn = crate::module_view::tests::blocking_connection_turn();
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_page = "page".into();
    app.buffer_page = "page".into();
    app.active_page_title = "Old Name".into();
    app.pages = vec![page_item("page", "Old Name"), page_item("other", "Other")];
    app.blocks = vec![page_block("b1", "page", "body")];
    app.page_text = ("Old Name\nbody").to_string();
    app.page_saved_text = "Old Name\nbody".into();

    // Someone inserts a block: the structural delta buys the debounced resync.
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: LiveKind::Pages,
        status: "Live".into(),
        height: 20,
        module: "pages".into(),
        load_pages: true,
        debounce: true,
        pages: backend::PagesDelta {
            kind: "touched".into(),
            ..backend::PagesDelta::default()
        },
        ..backend::LiveUpdate::default()
    }));
    let resync_generation = app.hydration_generation;
    let request_fold_serial = app.pages_fold_serial;

    // A rename folds while the resync's three reads are still executing.
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: LiveKind::Pages,
        status: "Live".into(),
        height: 21,
        module: "pages".into(),
        pages: backend::PagesDelta {
            kind: "text".into(),
            block_id: "page".into(),
            text: "New Name".into(),
        },
        ..backend::LiveUpdate::default()
    }));
    assert_eq!(app.active_page_title, "New Name");
    assert_eq!(
        app.hydration_generation, resync_generation,
        "a fold buys no reload — which is exactly why the in-flight reply stays current"
    );
    assert_ne!(
        app.pages_fold_serial, request_fold_serial,
        "the fold moved the serial the in-flight request snapshotted"
    );

    // The reply lands afterwards, built from the PRE-fold snapshot — but
    // carrying the inserted block, the very thing it was issued to fetch.
    let _ = app.__update(__DucktapeMessage::LiveResynced(backend::LiveRefresh {
        pages: vec![page_item("page", "Old Name"), page_item("other", "Other")],
        active_page_title: "Old Name".into(),
        fold_serial: request_fold_serial,
        ..live_refresh(resync_generation, "", "page",
            vec![
                page_block("b1", "page", "body"),
                page_block("b2", "page", "inserted"),
            ],
        )
    }));

    assert_eq!(
        app.active_page_title, "New Name",
        "the fold owns the header — the pre-fold reply must not revert it"
    );
    assert_eq!(app.pages[0].title, "New Name", "and the sidebar row");
    assert_eq!(
        app.pages[1].title, "Other",
        "and only the folded row's title"
    );
    assert_eq!(
        app.blocks
            .iter()
            .map(|block| block.text.as_str())
            .collect::<Vec<_>>(),
        vec!["body", "inserted"],
        "while the reply still delivers the structural half it was issued for"
    );
    assert_eq!(
        page_document_text(&app),
        "New Name\nbody\ninserted",
        "line 0 is rebuilt from the KEPT title, so header, row and editor agree"
    );
    assert_eq!(app.page_saved_text, "New Name\nbody\ninserted");
}

/// THE HALF BOTH OF #1041's REJECTED DESIGNS LOST: the reply is NOT discarded
/// wholesale. A generation bump on the fold — or a serial gating the whole
/// pages half — would throw away the structural data the read was issued for,
/// trading one staleness for another. Only the fold-owned fields (titles,
/// block texts) are kept; every reply-owned field still lands.
#[test]
fn a_fold_in_the_window_does_not_discard_the_replys_pages_half() {
    let _turn = crate::module_view::tests::blocking_connection_turn();
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_page = "page".into();
    app.buffer_page = "page".into();
    app.active_page_title = "Old Name".into();
    app.pages = vec![page_item("page", "Old Name")];
    app.blocks = vec![page_block("b1", "page", "body")];
    app.page_text = ("Old Name\nbody").to_string();
    app.page_saved_text = "Old Name\nbody".into();

    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: LiveKind::Pages,
        status: "Live".into(),
        height: 30,
        module: "pages".into(),
        load_pages: true,
        debounce: true,
        pages: backend::PagesDelta {
            kind: "touched".into(),
            ..backend::PagesDelta::default()
        },
        ..backend::LiveUpdate::default()
    }));
    let resync_generation = app.hydration_generation;
    let request_fold_serial = app.pages_fold_serial;

    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: LiveKind::Pages,
        status: "Live".into(),
        height: 31,
        module: "pages".into(),
        pages: backend::PagesDelta {
            kind: "text".into(),
            block_id: "page".into(),
            text: "New Name".into(),
        },
        ..backend::LiveUpdate::default()
    }));

    // The pre-fold reply carries page-list structure (a row state has never
    // seen), a fresh comment census and a parent — all reply-owned.
    let _ = app.__update(__DucktapeMessage::LiveResynced(backend::LiveRefresh {
        pages: vec![
            page_item("page", "Old Name"),
            page_item("brand-new", "Brand New"),
        ],
        active_page_title: "Old Name".into(),
        active_page_parent: "parent-page".into(),
        comment_thread_total: 4,
        commented_block_hits: vec!["b1".into()],
        fold_serial: request_fold_serial,
        ..live_refresh(resync_generation, "", "page",
            vec![page_block("b1", "page", "body")],
        )
    }));

    assert_eq!(app.active_page_title, "New Name", "the folded title holds");
    assert_eq!(
        app.pages
            .iter()
            .map(|page| (page.id.as_str(), page.title.as_str()))
            .collect::<Vec<_>>(),
        vec![("page", "New Name"), ("brand-new", "Brand New")],
        "the list takes the reply's structure and the fold's title"
    );
    assert_eq!(
        app.active_page_parent, "parent-page",
        "no fold writes a parent, so the reply's lands"
    );
    assert_eq!(app.block_comment_thread_total, 4);
    assert_eq!(app.commented_block_hits, vec!["b1".to_string()]);
}

/// THE OWNERSHIP CALL #1041 LEFT OPEN, PINNED: block STRUCTURE is the
/// reply's, block TEXT is the fold's.
///
/// `apply_page_text` folds body edits exactly as the rename folds the title
/// (#1027), so a body edit landing in the resync window is clobbered the same
/// way — and not merely on screen: a clean buffer rebuilt from the reply's
/// pre-fold text makes the reader's next keystroke plan the OLD text back
/// onto the chain (`document_plan` is a two-way diff, and body lines have no
/// authorship guard the way the title has `title_write_owed`). The LIST is
/// still the reply's: keeping current blocks wholesale would discard the
/// inserted block the read was issued for.
#[test]
fn a_body_text_fold_keeps_its_text_and_takes_the_replys_structure() {
    let _turn = crate::module_view::tests::blocking_connection_turn();
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_page = "page".into();
    app.buffer_page = "page".into();
    app.active_page_title = "Doc".into();
    app.pages = vec![page_item("page", "Doc")];
    app.blocks = vec![page_block("b1", "page", "body")];
    app.page_text = ("Doc\nbody").to_string();
    app.page_saved_text = "Doc\nbody".into();

    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: LiveKind::Pages,
        status: "Live".into(),
        height: 40,
        module: "pages".into(),
        load_pages: true,
        debounce: true,
        pages: backend::PagesDelta {
            kind: "touched".into(),
            ..backend::PagesDelta::default()
        },
        ..backend::LiveUpdate::default()
    }));
    let resync_generation = app.hydration_generation;
    let request_fold_serial = app.pages_fold_serial;

    // A peer's body edit folds into b1 while the reads are executing.
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: LiveKind::Pages,
        status: "Live".into(),
        height: 41,
        module: "pages".into(),
        pages: backend::PagesDelta {
            kind: "text".into(),
            block_id: "b1".into(),
            text: "peer edit".into(),
        },
        ..backend::LiveUpdate::default()
    }));
    assert_eq!(app.blocks[0].text, "peer edit");

    let _ = app.__update(__DucktapeMessage::LiveResynced(backend::LiveRefresh {
        pages: vec![page_item("page", "Doc")],
        active_page_title: "Doc".into(),
        fold_serial: request_fold_serial,
        ..live_refresh(resync_generation, "", "page",
            vec![
                page_block("b1", "page", "body"),
                page_block("b2", "page", "inserted"),
            ],
        )
    }));

    assert_eq!(
        app.blocks
            .iter()
            .map(|block| (block.id.as_str(), block.text.as_str()))
            .collect::<Vec<_>>(),
        vec![("b1", "peer edit"), ("b2", "inserted")],
        "the fold owns b1's text, the reply owns the list — including b2"
    );
    assert_eq!(
        page_document_text(&app),
        "Doc\npeer edit\ninserted",
        "a buffer rebuilt from the reply's pre-fold text would write it back \
         on the next keystroke — body lines have no title_write_owed"
    );
    assert_eq!(app.page_saved_text, "Doc\npeer edit\ninserted");
}

/// The gate RELEASES: a request issued after the fold snapshots the moved
/// serial, so its reply — which carries the fold's own values — lands
/// wholesale. The keep is scoped to replies the fold actually outran, not a
/// permanent title freeze.
#[test]
fn a_request_issued_after_the_fold_lands_its_title_normally() {
    let _turn = crate::module_view::tests::blocking_connection_turn();
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_page = "page".into();
    app.buffer_page = "page".into();
    app.active_page_title = "Old Name".into();
    app.pages = vec![page_item("page", "Old Name")];
    app.blocks = vec![page_block("b1", "page", "body")];
    app.page_text = ("Old Name\nbody").to_string();
    app.page_saved_text = "Old Name\nbody".into();

    // The rename folds FIRST, then a structural delta buys the resync: the
    // request snapshots the post-fold serial.
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: LiveKind::Pages,
        status: "Live".into(),
        height: 50,
        module: "pages".into(),
        pages: backend::PagesDelta {
            kind: "text".into(),
            block_id: "page".into(),
            text: "New Name".into(),
        },
        ..backend::LiveUpdate::default()
    }));
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: LiveKind::Pages,
        status: "Live".into(),
        height: 51,
        module: "pages".into(),
        load_pages: true,
        debounce: true,
        pages: backend::PagesDelta {
            kind: "touched".into(),
            ..backend::PagesDelta::default()
        },
        ..backend::LiveUpdate::default()
    }));

    // Its reply reads post-fold state — including a SECOND rename the stream
    // has not delivered yet. Serials match, so the reply's title lands.
    let _ = app.__update(__DucktapeMessage::LiveResynced(backend::LiveRefresh {
        pages: vec![page_item("page", "Renamed Again")],
        active_page_title: "Renamed Again".into(),
        fold_serial: app.pages_fold_serial,
        ..live_refresh(app.hydration_generation, "", "page",
            vec![page_block("b1", "page", "body")],
        )
    }));

    assert_eq!(
        app.active_page_title, "Renamed Again",
        "no fold outran this reply — its title is the freshest reading"
    );
    assert_eq!(app.pages[0].title, "Renamed Again");
}

#[test]
fn live_comment_refresh_updates_threads_without_touching_the_draft() {
    let _turn = crate::module_view::tests::blocking_connection_turn();
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.mutation_phase = MutationPhase::Idle;
    app.active_page = "page".into();
    app.block_comments_open = true;
    // the card is DOCUMENT-scoped: its anchor is the page it was opened
    // on, never the block selection that opened it.
    app.block_comments_target = "page".into();
    app.block_comment_draft = "draft stays".into();

    // a pages comment op arrives: the delta starts the debounced reload
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: LiveKind::Pages,
        status: "Live".into(),
        height: 8,
        load_pages: true,
        debounce: true,
        pages: backend::PagesDelta {
            kind: "touched".into(),
            ..backend::PagesDelta::default()
        },
        ..backend::LiveUpdate::default()
    }));
    let resync_generation = app.hydration_generation;
    let stale_generation = app.block_comments_generation;
    // a write bumps the generation, and every earlier reply is dropped whole
    let _ = app.__update(__DucktapeMessage::ThreadResolved(true));
    assert_ne!(app.block_comments_generation, stale_generation);

    let _ = app.__update(__DucktapeMessage::BlockThreadsLoaded(
        backend::BlockThreadListData {
            generation: stale_generation,
            target: "page".into(),
            threads: Vec::new(),
            total: 0,
        },
    ));
    assert_eq!(app.block_comment_draft, "draft stays");

    // the scoped reload lands and re-arms the comment refresh
    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(resync_generation, "", "page",
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
            threads: vec![backend::PageCommentThread {
                id: "thread-1".into(),
                target: "page".into(),
                author: "user".into(),
                meta: "2 comments".into(),
                resolved: false,
                comment_count: 2,
                comments: vec![
                    backend::PageComment {
                        id: "c1".into(),
                        ordinal: 1,
                        author: "user".into(),
                        meta: "#1".into(),
                        text: "opened".into(),
                    },
                    backend::PageComment {
                        id: "c2".into(),
                        ordinal: 2,
                        author: "other".into(),
                        meta: "#2".into(),
                        text: "answered".into(),
                    },
                ],
            }],
            total: 3,
        },
    ));

    assert_eq!(app.block_comment_thread_total, 3);
    assert_eq!(app.block_comment_draft, "draft stays");
    assert!(!app.block_comment_threads_loading);
    // ONE QUERY CARRIES THE WHOLE CONVERSATION, so a live refresh brings
    // every thread's replies with it — there is no second, per-thread load
    // left to go stale under the reader.
    assert_eq!(app.block_comment_threads[0].comments.len(), 2);
    assert_eq!(app.block_comment_rows.len(), 1);
    assert_eq!(
        app.block_comment_rows[0].thread.comments[1].text,
        "answered"
    );
}

#[test]
fn block_comment_recovery_always_unlocks_mutations() {
    let _turn = crate::module_view::tests::blocking_connection_turn();
    let (mut failed, _) = Ducktape::__boot();
    failed.block_comments_open = true;
    failed.block_comments_generation = 7;
    failed.block_comment_threads_loading = true;
    failed.mutation_phase = MutationPhase::Recovering;
    let _ = failed.__update(__DucktapeMessage::BlockThreadsRecoveryFailed(
        backend::HydrationError {
            generation: 7,
            message: "recovery read failed".into(),
        },
    ));
    assert_eq!(failed.mutation_phase, MutationPhase::Idle);
    assert!(!failed.block_comment_threads_loading);

    let (mut recovered, _) = Ducktape::__boot();
    recovered.block_comments_open = true;
    recovered.block_comments_target = "block-1".into();
    recovered.block_comments_generation = 8;
    recovered.block_comment_threads_loading = true;
    recovered.mutation_phase = MutationPhase::Recovering;
    recovered.error = "write result was uncertain".into();
    let _ = recovered.__update(__DucktapeMessage::BlockThreadsRecovered(
        backend::BlockThreadListData {
            generation: 8,
            target: "block-1".into(),
            threads: Vec::new(),
            total: 0,
        },
    ));
    assert_eq!(recovered.mutation_phase, MutationPhase::Idle);
    assert!(recovered.error.is_empty());

    // AND IT UNLOCKS ONLY WHAT IT LOCKED. "recovering" has a second terminal —
    // `live_resynced` ends the one `mutation_failed` parks — and it cannot tell
    // whose recovery it landed on, so this pair can arrive to find the lock
    // already released and a FRESH mutation holding it. Writing "idle" flatly
    // there re-enables a button whose write is still in flight, which is a
    // double submit one click away.
    let (mut overtaken, _) = Ducktape::__boot();
    overtaken.block_comments_open = true;
    overtaken.block_comments_target = "block-1".into();
    overtaken.block_comments_generation = 8;
    overtaken.mutation_phase = MutationPhase::Channel;
    let _ = overtaken.__update(__DucktapeMessage::BlockThreadsRecovered(
        backend::BlockThreadListData {
            generation: 8,
            target: "block-1".into(),
            threads: Vec::new(),
            total: 0,
        },
    ));
    assert_eq!(
        overtaken.mutation_phase,
        MutationPhase::Channel,
        "a stale recovery does not unlock the mutation that came after it"
    );

    // BOTH ARMS, because both took the term. A failed recovery is no more
    // entitled to a lock it no longer holds, and its arm would revert to a flat
    // "idle" with everything above still green.
    let (mut overtaken_failure, _) = Ducktape::__boot();
    overtaken_failure.block_comments_open = true;
    overtaken_failure.block_comments_generation = 8;
    overtaken_failure.block_comment_threads_loading = true;
    overtaken_failure.mutation_phase = MutationPhase::Channel;
    let _ = overtaken_failure.__update(__DucktapeMessage::BlockThreadsRecoveryFailed(
        backend::HydrationError {
            generation: 8,
            message: "recovery read failed".into(),
        },
    ));
    assert_eq!(
        overtaken_failure.mutation_phase,
        MutationPhase::Channel,
        "and neither does the failure arm"
    );
    assert!(!overtaken_failure.block_comment_threads_loading);
}

/// AN ARMED DELETE IS A LAYER, AND A LAYER SEALS WHAT IS UNDER IT.
///
/// `page_delete_armed` paints a scrim and a confirm over the canvas. It had no
/// Escape rung — the mouse was the only way out, while every other overlay in
/// the console answered the key — and `pages_ready` did not name it either, so
/// Cmd/Ctrl+Z walked through the scrim and mutated the very document the
/// reader is being asked to confirm the deletion of, autosave following the
/// buffer down.
#[test]
fn an_armed_page_delete_answers_escape_and_seals_the_document() {
    let _turn = crate::module_view::tests::blocking_connection_turn();
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.shell_tab = ShellTab::Pages;
    app.active_page = "alpha".into();
    app.page_text = ("one").to_string();
    app.page_delete_armed = true;

    let _ = app.__update(__DucktapeMessage::GlobalKeyPressed(command_chord(
        iced::keyboard::key::Code::KeyZ,
    )));
    assert_eq!(
        app.page_text.clone(),
        "one",
        "the scrim seals the document behind it"
    );

    let _ = app.__update(__DucktapeMessage::GlobalKeyPressed(escape_press()));
    assert!(!app.page_delete_armed, "and Escape is the way out of it");

    // With the confirm down the chord reaches the buffer again.
    let _ = app.__update(__DucktapeMessage::GlobalKeyPressed(command_chord(
        iced::keyboard::key::Code::KeyZ,
    )));
    assert_eq!(
        app.page_text.clone(),
        "one",
        "Undo belongs to the guest binding"
    );
}

/// A BADGE OPENS ITS BLOCK'S CONVERSATION, WHOLE — every thread on it, and
/// only that block's. The card is then the block's: the widen affordance is
/// withheld, because the page was never what the badge pointed at.
#[test]
fn a_document_comment_badge_scopes_the_card_to_its_block_and_pins_it_there() {
    let _turn = crate::module_view::tests::blocking_connection_turn();
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.mutation_phase = MutationPhase::Idle;
    app.active_page = "page".into();
    app.block_comments_open = true;
    app.block_comments_target = "page".into();
    app.inline_comment_target = "block-b".into();
    app.block_comments_pinned = true;
    app.block_comments_generation = 10;
    app.block_comment_threads_loading = true;
    app.blocks = vec![
        page_block("block-a", "page", "Paragraph A"),
        page_block("block-b", "page", "Paragraph B"),
    ];
    let thread = |id: &str, target: &str, resolved| backend::PageCommentThread {
        id: id.into(),
        target: target.into(),
        resolved,
        author: "Reader".into(),
        meta: "1 comment".into(),
        comment_count: 1,
        comments: vec![backend::PageComment {
            id: format!("{id}-1"),
            ordinal: 1,
            author: "Reader".into(),
            meta: "#1".into(),
            text: format!("said on {id}"),
        }],
    };
    let threads = vec![
        thread("other", "block-a", false),
        thread("resolved", "block-b", true),
        thread("wanted", "block-b", false),
    ];
    let _ = app.__update(__DucktapeMessage::BlockThreadsLoaded(
        backend::BlockThreadListData {
            generation: 10,
            target: "page".into(),
            threads,
            total: 2,
        },
    ));
    assert!(!app.block_comment_threads_loading);
    // The scope's threads, resolved one included — the card files that one
    // behind its own toggle rather than dropping it.
    assert_eq!(app.block_comment_rows.len(), 2);
    assert!(
        app.block_comment_rows
            .iter()
            .all(|row| row.thread.target == "block-b")
    );
    assert_eq!(app.block_comment_rows[0].anchor, "“Paragraph B”");
    // The comments came with the threads: nothing is loading behind the card.
    assert_eq!(
        app.block_comment_rows[1].thread.comments[0].text,
        "said on wanted"
    );
    // A resolved thread marks no line, so only the open one lights a badge.
    assert_eq!(app.commented_block_hits, ["block-a", "block-b"]);

    // WIDENING IS REFUSED while the card is pinned to its badge's block …
    let _ = app.__update(__DucktapeMessage::WidenBlockComments);
    assert_eq!(app.inline_comment_target, "block-b");
    // … and a chip-opened card widens back to every thread on the page.
    app.block_comments_pinned = false;
    let _ = app.__update(__DucktapeMessage::WidenBlockComments);
    assert!(app.inline_comment_target.is_empty());
    assert_eq!(
        app.block_comment_rows.len(),
        3,
        "no round trip, just a re-slice"
    );
    // Narrowing to a group header re-slices the same threads back down.
    let _ = app.__update(__DucktapeMessage::NarrowBlockComments("block-a".into()));
    assert_eq!(app.inline_comment_target, "block-a");
    assert_eq!(app.block_comment_rows.len(), 1);
    assert_eq!(app.block_comment_rows[0].thread.id, "other");
}

/// THE COMPOSER TAKES THE CARD'S SCOPE AND A REPLY TAKES ITS THREAD'S ANCHOR,
/// and a thread id the list has lost posts nowhere at all.
#[test]
fn a_post_anchors_on_its_thread_or_on_the_cards_scope() {
    let _turn = crate::module_view::tests::blocking_connection_turn();
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.connected = true;
    app.mutation_phase = MutationPhase::Idle;
    app.active_page = "page".into();
    app.block_comments_open = true;
    app.block_comments_target = "page".into();
    app.inline_comment_target = "block-b".into();
    app.block_comment_threads = vec![backend::PageCommentThread {
        id: "on-a".into(),
        target: "block-a".into(),
        author: "Reader".into(),
        meta: "1 comment".into(),
        resolved: false,
        comment_count: 1,
        comments: Vec::new(),
    }];

    // A thread nobody is carrying any more: refused, and the draft is kept.
    app.block_comment_draft = "into the void".into();
    let _ = app.__update(__DucktapeMessage::PostBlockCommentSubmit("gone".into()));
    assert_eq!(app.mutation_phase, MutationPhase::Idle);
    assert_eq!(app.block_comment_draft, "into the void");

    // A reply rides its OWN thread's anchor, not the card's scope.
    let _ = app.__update(__DucktapeMessage::PostBlockCommentSubmit("on-a".into()));
    assert_eq!(app.mutation_phase, MutationPhase::BlockComment);
    assert_eq!(app.pending_block_comment, "into the void");
    assert!(app.block_comment_draft.is_empty());
}
