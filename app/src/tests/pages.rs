use super::*;

#[test]
fn background_refresh_preserves_editing_state() {
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
    for scoped in ["channel_name_draft", "member_key_draft"] {
        assert!(refresh.contains(&format!(
            "{scoped} = retain_for_endpoint({scoped}, active_channel, \
keep_str(next.chat_loaded, next.active_channel, active_channel))"
        )));
    }
    assert!(refresh.contains("selected_message_seq = refreshed_required_message_seq("));
    // The evicted inline edit is rescued to the composer of the room it was
    // in, not to an app-wide plate (ducktape-ui#698). The DESTINATION is the
    // half worth pinning: `active_channel` still names the room being LEFT
    // here — its own assignment is further down the handler — so the rescue
    // cannot surface over the room the resync is carrying her to.
    assert!(refresh.contains(
        "slice ChatComposer.unsent(keep_str(message_action == MessageAction.editing, \
message_edit_draft, \"\"), selected_message_seq > 0 || message_action != \
MessageAction.editing) at composer_scope(connected_rpc, active_channel)"
    ));
    assert!(lifecycle.contains("run live_events(connected_rpc) when connected"));
    assert_no_polling(&lifecycle);
    assert!(lifecycle.contains("run replace lane=live_resync live_resync_load(connected_rpc"));
    assert!(lifecycle.contains("run replace lane=live_thread refresh_live_thread(connected_rpc"));
    assert!(lifecycle.contains("parallel\n    run replace lane=live_thread refresh_live_thread("));
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
    assert!(lifecycle.contains("page_editor = refreshed_page_editor("));
    assert!(lifecycle.contains("page_saved_text = resynced_saved"));
    // the comment rail is scoped to the PAGE it hangs off, so its draft
    // survives moving the cursor between blocks and dies with the page.
    assert!(lifecycle.contains(
        "block_comment_draft = retain_selected_string(block_comment_draft, block_comments_target)"
    ));
    // the live comment-list callback settles state and stops — re-entering
    // the resync from inside it would loop the rail against the page.
    let pages_handlers = inlined(include_str!("../ui/handlers/pages.ice"));
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
/// `reply_composer_event` refused on `loading` — a term neither the reply
/// editor, its marks row nor its Send button wears — so in the one state that
/// can raise it under an open rail the reader saw a fully lit Send, pressed it,
/// and got nothing: no post, no error, no banner. Every chat-plane writer of
/// `loading = true` zeroes `active_thread_seq` in the same handler, so the term
/// never fired for a chat load at all; the state it caught was a PAGES load
/// still in flight behind a cross-tab bounce.
#[test]
fn a_pages_load_in_flight_does_not_deaden_the_lit_reply_send() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.active_channel = "general".into();
    app.active_thread_seq = 7;
    // What `open_page_search_hit` leaves behind: the stream's flag up, the rail
    // untouched, and `select_shell_tab` back to Chat clears neither.
    app.loading = true;
    submit(&mut app, ComposerKind::Reply, "on it");
    assert_eq!(
        app.thread_messages.len(),
        1,
        "a Send the surface draws as live must actually send"
    );
    assert!(app.thread_messages[0].pending);
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
    app.block_autosave_status = AutosaveStatus::Saving;

    let _ = app.__update(__DucktapeMessage::PageAutosaveTick);
    assert_eq!(
        app.block_autosave_status,
        AutosaveStatus::Saving,
        "inflight guard"
    );

    app.block_autosave_status = AutosaveStatus::Idle;
    app.page_editor = compose("Title\n```\nstill typing");
    let _ = app.__update(__DucktapeMessage::PageAutosaveTick);
    assert_eq!(
        app.block_autosave_status,
        AutosaveStatus::Idle,
        "fence guard"
    );

    app.page_editor = compose("Title\n```\ndone\n```");
    let _ = app.__update(__DucktapeMessage::PageAutosaveTick);
    assert_eq!(app.block_autosave_status, AutosaveStatus::Saving);
}

#[test]
fn page_autosave_freshness_is_compiler_owned_without_aborting_writes() {
    let pages = inlined(include_str!("../ui/handlers/pages.ice"));
    assert!(pages.contains(
        "run latest lane=page_autosave save_page_document(connected_rpc, password, active_page, text, page_saved_text) -> page_document_saved _ | page_document_save_failed _"
    ));
    assert!(!pages.contains("run replace lane=page_autosave"));
    assert_eq!(pages.matches("invalidate lane=page_autosave").count(), 5);

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
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.active_page = "page".into();
    app.buffer_page = "page".into();
    // she is mid-sentence; her line 0 is the OLD name and she never touched it.
    app.page_editor = compose("Old Name\nbody mid-sentence");
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
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.active_page = "page".into();
    app.buffer_page = "page".into();
    app.page_editor = compose("Notes\nhello");
    app.page_saved_text = "Notes\nhello".into();

    // the tick submits what it can see.
    app.page_inflight_text = "Notes \nhello".into();

    // SHE FINISHES THE WORD while the save is in flight.
    app.page_editor = compose("Notes A\nhello");

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
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.active_page = "page".into();
    app.buffer_page = "page".into();
    app.page_editor = compose("Old Name\nbody typed on");
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

/// The artifact hangs comments off the document as a docked 306px rail on
/// the sidebar ladder, NOT as a floating card over it — a card would cover
/// the block it is about the moment the block sits on the right half.
#[test]
fn block_comments_dock_a_rail_beside_the_document() {
    // the pages screen is its own file now, so the slot slicing is gone.
    let pages = inlined(include_str!("../ui/screens/pages.ice"));
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
    let components = inlined(include_str!("../ui/components/pages.ice"));
    assert!(!components.contains("component BlockActionsMenu"));

    let handlers = inlined(include_str!("../ui/handlers/pages.ice"));
    assert!(handlers.contains("on post_block_comment_submit"));
    // A NEW comment anchors on the CARET's block (the thread's own target on
    // a reply) — never blindly on the page.
    assert!(handlers.contains(
        "run every post_block_comment(connected_rpc, password, active_thread_target, active_block_comment_thread"
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
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_page = "page".into();
    app.buffer_page = "page".into();
    app.active_page_title = "Old Name".into();
    app.pages = vec![page_item("page", "Old Name"), page_item("other", "Other")];
    app.blocks = vec![page_block("b1", "page", "body")];
    // A CLEAN buffer: baseline and buffer agree, so the rebuild is allowed.
    app.page_editor = compose("Old Name\nbody");
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
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_page = "page".into();
    app.buffer_page = "page".into();
    app.active_page_title = "Old Name".into();
    app.pages = vec![page_item("page", "Old Name")];
    app.blocks = vec![page_block("b1", "page", "body")];
    // DIRTY: she has typed since the last save.
    app.page_editor = compose("Old Name\nbody mid-sentence");
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
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_page = "page".into();
    app.buffer_page = "page".into();
    app.active_page_title = "Old Name".into();
    app.pages = vec![page_item("page", "Old Name"), page_item("other", "Other")];
    app.blocks = vec![page_block("b1", "page", "body")];
    app.page_editor = compose("Old Name\nbody");
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
        ..live_refresh(
            resync_generation,
            "",
            Vec::new(),
            "page",
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
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_page = "page".into();
    app.buffer_page = "page".into();
    app.active_page_title = "Old Name".into();
    app.pages = vec![page_item("page", "Old Name")];
    app.blocks = vec![page_block("b1", "page", "body")];
    app.page_editor = compose("Old Name\nbody");
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
        ..live_refresh(
            resync_generation,
            "",
            Vec::new(),
            "page",
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
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_page = "page".into();
    app.buffer_page = "page".into();
    app.active_page_title = "Doc".into();
    app.pages = vec![page_item("page", "Doc")];
    app.blocks = vec![page_block("b1", "page", "body")];
    app.page_editor = compose("Doc\nbody");
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
        ..live_refresh(
            resync_generation,
            "",
            Vec::new(),
            "page",
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
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_page = "page".into();
    app.buffer_page = "page".into();
    app.active_page_title = "Old Name".into();
    app.pages = vec![page_item("page", "Old Name")];
    app.blocks = vec![page_block("b1", "page", "body")];
    app.page_editor = compose("Old Name\nbody");
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
        ..live_refresh(
            app.hydration_generation,
            "",
            Vec::new(),
            "page",
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
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.mutation_phase = MutationPhase::Idle;
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
fn block_comment_recovery_always_unlocks_mutations() {
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
            from: 0,
            threads: Vec::new(),
            total: 0,
            next_from: 0,
            has_more: false,
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
            from: 0,
            threads: Vec::new(),
            total: 0,
            next_from: 0,
            has_more: false,
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
    crate::pages::history::reset();
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.shell_tab = ShellTab::Pages;
    app.active_page = "alpha".into();
    app.page_editor = compose("one");
    crate::pages::history::record(|| ("".to_owned(), app.page_editor.cursor()));
    app.page_delete_armed = true;

    let _ = app.__update(__DucktapeMessage::GlobalKeyPressed(command_chord(
        iced::keyboard::key::Code::KeyZ,
    )));
    assert_eq!(
        app.page_editor.text(),
        "one",
        "the scrim seals the document behind it"
    );

    let _ = app.__update(__DucktapeMessage::GlobalKeyPressed(escape_press()));
    assert!(!app.page_delete_armed, "and Escape is the way out of it");

    // With the confirm down the chord reaches the buffer again.
    let _ = app.__update(__DucktapeMessage::GlobalKeyPressed(command_chord(
        iced::keyboard::key::Code::KeyZ,
    )));
    assert_eq!(app.page_editor.text(), "");
    crate::pages::history::reset();
}
