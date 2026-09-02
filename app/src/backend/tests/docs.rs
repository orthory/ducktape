use super::*;

#[test]
fn the_stored_tab_list_forgets_pages_that_are_gone() {
    let pages = ["welcome", "runbook"]
        .into_iter()
        .map(|id| PageItem {
            id: id.into(),
            title: String::new(),
            parent: String::new(),
            prefix: String::new(),
            child_count: 0,
        })
        .collect::<Vec<_>>();
    let stored = ["welcome", "deleted-1", "runbook", "deleted-2"]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    // `doc_tab_rows` already hides a dead tab when it draws; this is what keeps
    // the PERSISTED list — and the count Settings reads off it — honest.
    assert_eq!(
        doc_tabs_pruned(stored, pages.clone()),
        ["welcome", "runbook"]
    );
    assert!(doc_tabs_pruned(Vec::new(), pages).is_empty());

    // An unnamed principal gets a bare plate, never a `?` — that glyph in the
    // rail's corner reads as HELP, not as "nobody has named this account".
    assert_eq!(initial_of(""), "");
    assert_eq!(initial_of("   "), "");
    assert_eq!(initial_of("quackbot"), "Q");
}

#[test]
fn deleting_a_page_takes_its_subtree_with_it() {
    let page = |id: &str, parent: &str| PageItem {
        id: id.into(),
        title: String::new(),
        parent: parent.into(),
        prefix: String::new(),
        child_count: 0,
    };
    // root -> child -> grandchild, plus an unrelated sibling tree.
    let pages = vec![
        page("root", ""),
        page("child", "root"),
        page("grandchild", "child"),
        page("other", ""),
        page("other-child", "other"),
    ];
    let doomed = descendants_of(&pages, "root");
    // `RemoveBlock` takes the whole subtree, so the correction has to as well —
    // taking only the named row would leave orphans pointing at a gone parent.
    assert!(doomed.contains("root") && doomed.contains("child") && doomed.contains("grandchild"));
    assert!(!doomed.contains("other") && !doomed.contains("other-child"));
    assert_eq!(doomed.len(), 3);
    // A leaf takes only itself, and an id the index never had takes only itself.
    assert_eq!(descendants_of(&pages, "grandchild").len(), 1);
    assert_eq!(descendants_of(&pages, "gone").len(), 1);
}

#[test]
fn a_stale_pages_reply_does_not_move_the_reader() {
    let listed = |ids: &[&str]| {
        ids.iter()
            .map(|id| PageItem {
                id: (*id).into(),
                title: String::new(),
                parent: String::new(),
                prefix: String::new(),
                child_count: 0,
            })
            .collect::<Vec<_>>()
    };
    // Resolved to the page in hand — current.
    assert!(pages_reply_answers_current(
        listed(&["a", "b"]),
        "a".into(),
        "a".into()
    ));
    // Issued for `a` before the reader moved to `b`, answered after: `b` is
    // right there in the index the reply just read, so the reply is stale.
    assert!(!pages_reply_answers_current(
        listed(&["a", "b"]),
        "a".into(),
        "b".into()
    ));
    // The page the reader is on is GONE — the fallback is the honest answer.
    assert!(pages_reply_answers_current(
        listed(&["a"]),
        "a".into(),
        "b".into()
    ));
    // Nothing selected yet: anything the reply offers is an improvement.
    assert!(pages_reply_answers_current(
        listed(&["a"]),
        "a".into(),
        String::new()
    ));
}

#[test]
fn block_comment_posts_reuse_the_selected_thread() {
    assert_eq!(comment_thread_id("thread-a".into()).unwrap(), "thread-a");
    assert!(
        comment_thread_id(String::new())
            .unwrap()
            .starts_with("thread-")
    );
    assert!(comment_thread_id(" ".into()).is_err());
}

/// THE TITLE WRITE IS AUTHORSHIP, NOT DISAGREEMENT.
///
/// Disagreement with the node was the whole old test, and it is why a reader
/// who had merely not caught up wrote the old name back over someone else's
/// rename. `title_write_owed` takes the title, the baseline the buffer was
/// synced to, and the node's title — a pure decide, so it needs no node.
#[test]
fn a_title_write_is_owed_only_when_this_reader_retitled_the_page() {
    assert!(
        title_write_owed("New", "Old\nbody", "Old"),
        "she retyped line 0, so the node owes a rename"
    );
    assert!(
        !title_write_owed("Old", "Old\nbody", "New"),
        "her line 0 matches the baseline she started from — she renamed nothing, \
         and writing it back would revert the other rename on chain"
    );
    // AUTHORSHIP ALONE IS NOT ENOUGH, and this is the case that proves it: an
    // empty baseline (a buffer that has never synced) makes every title look
    // authored. Only the node's disagreement stops a first save from
    // submitting a rename nobody asked for.
    assert!(
        !title_write_owed("Doc", "", "Doc"),
        "an agreeing title must not submit an op, whatever the baseline says"
    );
    assert!(
        title_write_owed("Doc", "", "Other"),
        "a genuinely new title on a fresh buffer IS owed"
    );
}

/// The baseline may not claim a sync that never happened.
///
/// A save adopts the node's canonical text, and that carries a title someone
/// else may have changed while this reader typed — one the buffer has never
/// shown, because the dirty guard refuses to rebuild it mid-sentence. Swallow
/// that and the NEXT tick reads the difference as this reader retitling the
/// page: the same revert, one tick later.
#[test]
fn the_baseline_keeps_the_title_the_buffer_is_actually_showing() {
    // the ordinary path: titles agree, the canonical text is not reshaped.
    let untouched = baseline_at_submitted_title("Doc\nbody".into(), "Doc\nbody typing".into());
    assert_eq!(untouched, "Doc\nbody");

    // someone else renamed it: the node's body is adopted, her line 0 is kept,
    // so the next tick still reads "she retitled nothing".
    let corrected =
        baseline_at_submitted_title("New Name\nbody".into(), "Old Name\nbody mid-sen".into());
    assert_eq!(corrected, "Old Name\nbody");
    assert!(!title_write_owed(
        &crate::pages::sync::document_title("Old Name\nbody mid-sen"),
        &corrected,
        "New Name"
    ));

    // VERBATIM, not trimmed: the dirty test compares these byte for byte, so a
    // normalized line 0 would leave the buffer permanently dirty and the save
    // tick running forever.
    let spaced = baseline_at_submitted_title("New\nbody".into(), "Old  \nbody".into());
    assert_eq!(spaced, "Old  \nbody");

    // a title-only document keeps its shape — no newline is invented.
    let titleless = baseline_at_submitted_title("New".into(), "Old".into());
    assert_eq!(titleless, "Old");

    // THE EARLY RETURN IS LOAD-BEARING, not an optimization: when the titles
    // agree the canonical text must come back BYTE-IDENTICAL, body and all.
    // Rebuilding it from the submitted line 0 would drop the node's own body
    // edits into the baseline and call the buffer clean.
    let agreeing = baseline_at_submitted_title("Doc\nnode body".into(), "Doc\nher body".into());
    assert_eq!(
        agreeing, "Doc\nnode body",
        "an agreeing title returns the node's text untouched"
    );
}

#[test]
fn block_action_menu_stays_inside_the_page_viewport() {
    assert_eq!(block_action_menu_y(100.0, 500.0), 96.0);
    assert_eq!(block_action_menu_y(450.0, 500.0), 260.0);
    assert_eq!(block_action_menu_y(2.0, 500.0), 0.0);
}

#[test]
fn an_empty_block_is_writable_but_an_empty_page_title_is_not() {
    // A blank line is what Enter-Enter makes; rejecting it put every save
    // after one into a permanent retry loop.
    assert_eq!(
        bounded_new_block_text(BlockKind::Paragraph, String::new()).unwrap(),
        ""
    );
    assert!(bounded_new_block_text(BlockKind::Page, String::new()).is_err());
}

#[test]
fn block_text_is_bounded_by_the_modules_own_caps() {
    // An app-side cap tighter than the module's refuses text the node accepts
    // — and leaves no way to shorten a block another signer already landed.
    let at_cap = "x".repeat(pages::MAX_BLOCK_LEN);
    assert!(bounded_new_block_text(BlockKind::Paragraph, at_cap.clone()).is_ok());
    assert!(bounded_updated_block_text(BlockKind::Paragraph, at_cap).is_ok());
    let over_cap = "x".repeat(pages::MAX_BLOCK_LEN + 1);
    assert!(bounded_new_block_text(BlockKind::Paragraph, over_cap.clone()).is_err());
    assert!(bounded_updated_block_text(BlockKind::Paragraph, over_cap).is_err());

    let title_at_cap = "t".repeat(pages::MAX_PAGE_TITLE_LEN);
    assert!(bounded_new_block_text(BlockKind::Page, title_at_cap.clone()).is_ok());
    assert!(bounded_updated_block_text(BlockKind::Page, title_at_cap).is_ok());
    let title_over_cap = "t".repeat(pages::MAX_PAGE_TITLE_LEN + 1);
    assert!(bounded_new_block_text(BlockKind::Page, title_over_cap.clone()).is_err());
    assert!(bounded_updated_block_text(BlockKind::Page, title_over_cap).is_err());
}

#[test]
fn a_write_adopts_the_nodes_text_and_a_noop_adopts_the_submitted_text() {
    // Written: the canonical baseline keeps a one-step-per-tick depth change
    // ticking until buffer and node agree.
    assert_eq!(
        saved_baseline(true, "canonical".into(), "submitted".into()),
        "canonical"
    );
    // No-op: `* item` and `- item` parse identically — a canonical baseline
    // here would leave the tick firing forever over spelling.
    assert_eq!(
        saved_baseline(false, "canonical".into(), "submitted".into()),
        "submitted"
    );
}

#[test]
fn page_updates_preserve_exact_text() {
    assert_eq!(
        bounded_updated_block_text(BlockKind::Code, "  code\n".into()).unwrap(),
        "  code\n"
    );
    assert_eq!(
        bounded_updated_block_text(BlockKind::Paragraph, String::new()).unwrap(),
        ""
    );
    assert_eq!(
        bounded_exact_text(String::new(), "page title", pages::MAX_PAGE_TITLE_LEN).unwrap(),
        ""
    );
}

#[test]
fn block_moves_follow_visible_sibling_order() {
    let block = |id: &str, parent: Option<&str>, kind, children: &[&str]| pages::Block {
        id: id.into(),
        parent: parent.map(str::to_string),
        page: "page".into(),
        kind,
        text: id.into(),
        marks: Vec::new(),
        checked: false,
        children: children.iter().map(|child| (*child).into()).collect(),
    };
    let blocks = vec![
        block("page", None, BlockKind::Page, &["a", "b"]),
        block("a", Some("page"), BlockKind::Paragraph, &["c"]),
        block("c", Some("a"), BlockKind::Paragraph, &[]),
        block("b", Some("page"), BlockKind::Paragraph, &[]),
    ];

    assert_eq!(
        block_move(&blocks, "b", "up").unwrap(),
        (Some("page".into()), None)
    );
    assert_eq!(
        block_move(&blocks, "a", "down").unwrap(),
        (Some("page".into()), Some("b".into()))
    );
    assert_eq!(
        block_move(&blocks, "b", "indent").unwrap(),
        (Some("a".into()), Some("c".into()))
    );
    assert_eq!(
        block_move(&blocks, "c", "outdent").unwrap(),
        (Some("page".into()), Some("a".into()))
    );

    let page = block("child-page", Some("page"), BlockKind::Page, &[]);
    let parent = block("page", None, BlockKind::Page, &["child-page"]);
    assert_eq!(
        block_move(&[parent, page], "child-page", "outdent").unwrap(),
        (None, None)
    );
}

/// THE CLASSIFIER, not the handler: a text edit must reach the shell with
/// `load_pages` FALSE, which is what stops the reload.
///
/// `a_folded_text_edit_updates_the_block_and_fetches_nothing` (app/src/tests.rs)
/// asserts the handler's half by building the update by hand, so it cannot see
/// this half at all — flipping `load_pages` back to unconditional leaves it
/// green. This is the test that goes red.
#[tokio::test(flavor = "current_thread")]
async fn a_pages_text_op_folds_and_a_structural_one_reloads() {
    let op = |msg: &PageMsg| ducktape_rpc::StreamOp {
        height: 9,
        seq: 0,
        time: 0,
        origin: ducktape_rpc::StreamOrigin {
            kind: ducktape_rpc::StreamOriginKind::External,
            id: None,
        },
        payload: Some(serde_json::from_slice(&pages::encode_msg(msg)).expect("payload json")),
        payload_hex: None,
        assigned: None,
        assigned_hex: None,
    };

    let edit = folded_update(
        "",
        "pages",
        op(&PageMsg::UpdateText {
            block_id: "b1".into(),
            text: "typed".into(),
            marks: None,
        }),
    )
    .await
    .expect("a text op is visible to the shell");
    assert_eq!(edit.pages.kind, "text");
    assert_eq!(edit.pages.block_id, "b1");
    assert_eq!(edit.pages.text, "typed");
    assert!(
        !edit.load_pages,
        "a folded edit must not ask for a reload — that is the whole change"
    );
    assert!(
        !edit.debounce,
        "nothing to coalesce when nothing is fetched"
    );

    let moved = folded_update(
        "",
        "pages",
        op(&PageMsg::MoveBlock {
            block_id: "b1".into(),
            parent: Some("page".into()),
            after: None,
        }),
    )
    .await
    .expect("a structural op is visible to the shell");
    assert_eq!(moved.pages.kind, "touched");
    assert!(
        moved.load_pages,
        "ordering and prefixes are not derivable from the op — reload"
    );
}

/// A PAGE HIT NAMES ITS PAGE, AND SAYS EACH THING ONCE. The index's hit row
/// carries a `page_id` and no title, so nothing downstream could name the page
/// a match came from: the Explorer set BOTH its row title and its snippet to
/// `hit.text` — the same sentence printed twice — and its only metadata was
/// the block kind (`pages · Text`), which is true of nearly every hit. The
/// palette printed that bare kind too, and the pages search panel printed the
/// raw `block_id`.
///
/// The title is now joined in at the producer, so all three surfaces agree —
/// the shape #997 used for the chat hit's room.
#[test]
fn a_page_search_hit_names_the_page_it_came_from() {
    let row = |page_id: &str, text: &str| pages::index::PageBlockRow {
        block_id: format!("block-{text}"),
        page_id: page_id.into(),
        parent: Some(page_id.into()),
        kind: BlockKind::Paragraph,
        text: text.into(),
        marks: Vec::new(),
        checked: false,
        children: Vec::new(),
        height: 1,
        time: 1,
    };
    let page = |id: &str, title: &str| PageRow {
        id: id.into(),
        title: title.into(),
        parent: None,
    };

    let hits = titled_page_hits(
        vec![
            row("page-1", "Tail paragraph after the list"),
            row("page-2", "second mention"),
            // A page the index does not carry, and one with no title at all.
            row("page-gone", "orphan mention"),
            row("page-3", "untitled mention"),
        ],
        vec![
            page("page-1", "Design QA"),
            page("page-2", "Team Runbook"),
            page("page-3", ""),
        ],
    );

    assert_eq!(hits[0].page_title, "Design QA");
    assert_eq!(hits[1].page_title, "Team Runbook");
    // The sidebar calls a titleless page "Untitled"; a hit must not read
    // differently, and a missing page must not read blank.
    assert_eq!(hits[2].page_title, "Untitled");
    assert_eq!(hits[3].page_title, "Untitled");
    // The join must not disturb what the row already carried.
    assert_eq!(hits[0].text, "Tail paragraph after the list");
    assert_eq!(hits[0].page_id, "page-1");
    assert_eq!(hits[0].kind, "Text");

    // THE CALL SITES. A pure join proves nothing about what the surfaces
    // render, and the Explorer's double print lived at ITS call site.
    const SEARCH: &str = include_str!("../search.rs");
    let page_arm = SEARCH
        .split("kind: \"page\".into(),")
        .nth(1)
        .expect("the page hit arm")
        .split("}));")
        .next()
        .expect("arm body");
    assert!(
        page_arm.contains("title: hit.page_title,") && page_arm.contains("snippet: hit.text,"),
        "the Explorer heads a page hit with its page and keeps the block text as the snippet"
    );
    assert!(
        !page_arm.contains("title: hit.text"),
        "titling the row with the block text is what printed the same sentence twice"
    );

    // The palette and the pages search panel render the same hit type; #997's
    // lesson is that a fix at one surface leaves the siblings broken.
    const PALETTE: &str = include_str!("../../ui/screens/overlays.ice");
    const PANEL: &str = include_str!("../../ui/components/pages.ice");
    assert!(
        PALETTE.contains("text hit.page_title"),
        "the palette's page hit names its page"
    );
    assert!(
        PANEL.contains("text hit.page_title") && !PANEL.contains("text hit.block_id"),
        "the pages search panel names the page instead of printing a raw block id"
    );
}

/// A FAILED TITLE LOOKUP DEGRADES THE LABEL, NOT THE RESULTS. #1003 joined the
/// page index onto the hits with `?`, so a `ListPages` failure — a SECOND round
/// trip, made after the node had already answered the search — turned a
/// successful search into an `Err`. Both readers discard that silently: the
/// Explorer's `if let Ok(pages)` (backend/search.rs) drops every page hit from
/// a workspace search, and the palette keeps only whichever leg survived. A
/// decoration must never destroy the payload.
#[tokio::test(flavor = "current_thread")]
async fn a_failed_title_lookup_keeps_the_page_hits_it_could_not_name() {
    let rpc = node_with_a_broken_page_list().await;
    let data = search_pages(rpc, String::new(), "tail".into())
        .await
        .expect("a search the node answered must not fail on its title lookup");

    assert_eq!(data.hits.len(), 1, "the hit the search returned survives");
    assert_eq!(data.hits[0].text, "Tail paragraph after the list");
    assert_eq!(data.hits[0].page_id, "page-1");
    assert_eq!(data.hits[0].block_id, "block-1");
    // Only the LABEL degrades, onto the same fallback an unresolvable page id
    // already takes in `titled_page_hits`.
    assert_eq!(data.hits[0].page_title, "Untitled");
}

/// THE PAGE READ RIDES THE VIEW LANE, AND WHAT STAYS ON THE OTHER ONE IS
/// NAMED. Opening a document is the app's highest-frequency read, and on
/// `/v1/query` every one of them went through the node's dispatch actor —
/// the select-loop/checkpoint tax of issue #1018, paid per page open, per
/// autosave tick that reads the tree back, per comment rail.
///
/// `PagesViewQuery::GetPage` answers the identical `PageBlockPage` off an MVCC
/// snapshot (pages' `tests/index_parity.rs` proves the two replies are the
/// same reply), so what remains is to keep it there. Same source-shape pin as
/// the chat lane's, for the same reason: both lanes answer the same rows
/// against a live node, so only the route tells them apart.
#[test]
fn the_page_read_never_crosses_the_dispatch_query_lane() {
    const LOAD: &str = include_str!("../load.rs");
    let load_page_blocks = backend_fn(LOAD, "pub(crate) async fn load_page_blocks(");
    assert!(
        load_page_blocks.contains("PagesViewQuery::GetPage {"),
        "the page read is the index view arm"
    );
    assert!(
        !load_page_blocks.contains(".query("),
        "a page read on /v1/query pays the node's checkpoint tax"
    );

    // THE WHOLE MODULE'S LANE, not just this function. `PageQuery` is pages'
    // DISPATCH read surface, and exactly one arm of it legitimately remains:
    // `CommentThread`. Two reasons, and both have to stop holding before it
    // moves — the index guest serves grouped `ThreadRow`s through
    // `threads_for_targets`, not the `ThreadView` this reply carries; and its
    // one caller is `add_block_comment`'s read of the comment it JUST posted,
    // where the canonical lane is read-after-write by construction. Anything
    // else appearing here is a pages read crawling back onto the select loop.
    //
    // WALKED, never listed. A hand-written list of files is a rule carrying its
    // own escape hatch: `PageQuery` is imported once (`backend/mod.rs`) and
    // every module here inherits it through `use super::*`, so the next backend
    // module added is exactly the one a list would not sweep — and a page read
    // dropped into it would pass this test silently. Only this file is skipped,
    // and for the reason the chat pin skips its own prose: a sweep over raw
    // source cannot tell the banned symbol from the string that bans it.
    const KEPT: &str = "CommentThread";
    let backend = backend_sources();
    assert!(
        backend.iter().any(|(name, _)| name == "load.rs"),
        "the walk found the backend it is supposed to be sweeping"
    );
    let mut arms: Vec<String> = Vec::new();
    for (name, source) in &backend {
        for rest in source.split("PageQuery::").skip(1) {
            let arm: String = rest
                .chars()
                .take_while(char::is_ascii_alphanumeric)
                .collect();
            assert_eq!(arm, KEPT, "{name} reads pages::{arm} on the dispatch lane");
            arms.push(arm);
        }
    }
    assert_eq!(
        arms,
        [KEPT],
        "the ONE kept dispatch read, exactly once — a second call site is a \
         lane decision, not a copy-paste"
    );
}

/// AND THE READ ITSELF WAITS. The lane pin above proves the page read left
/// `/v1/query`; this proves read-after-write did not stay behind with it. The
/// view lane folds BEHIND the block loop, so both pages reads open with
/// `await_pages_fold` — the reload that ends a save, the plan the next autosave
/// tick diffs against, the pane a live push refreshes.
///
/// Pinned as a source shape because deleting either call fails NOTHING else in
/// this suite: the wait costs zero probes when nothing is outstanding, so a
/// passing test cannot tell it apart from an absent one. It shows up on a live
/// node instead, as a document that lost the line it just took.
#[test]
fn the_pages_reads_wait_for_the_fold_before_they_read() {
    const LOAD: &str = include_str!("../load.rs");
    for (declaration, read) in [
        ("pub(crate) async fn load_pages_data(", "load_page_index("),
        ("pub(crate) async fn load_page_blocks(", ".view("),
    ] {
        let body = backend_fn(LOAD, declaration);
        let wait = body
            .find("await_pages_fold(rpc).await")
            .unwrap_or_else(|| panic!("{declaration} waits for the pages fold"));
        let read = body
            .find(read)
            .unwrap_or_else(|| panic!("{declaration} reads the index"));
        assert!(wait < read, "{declaration} waits BEFORE it reads");
    }
}

/// A WRITE'S OWN RELOAD WAITS FOR THE FOLD THAT CARRIES IT. `submit_frame`
/// answers ACCEPTANCE; the pages read model is folded behind the block loop,
/// so the reload `create_page`/`delete_page` fire immediately afterwards used
/// to read an index that predated the write — the new page absent from the
/// list it was supposed to land on, the deleted one still sitting in the
/// sidebar. The view lane now answers how far the fold has consumed the op
/// feed, and this is the wait that reads it: stale, stale, then caught up.
#[tokio::test(flavor = "current_thread")]
async fn a_post_write_reload_waits_for_the_fold_to_reach_its_block() {
    let (origin, served) =
        node_scripting_its_fold_watermark(vec![Some("6:0"), Some("6:3"), Some("9:0")]).await;
    let rpc = rpc_client(&origin).expect("stub client");

    let arrived = await_fold(&rpc, "pages", &empty_pages_probe(), 9).await;

    assert!(arrived, "the third probe's watermark reaches block 9");
    assert_eq!(
        served.load(std::sync::atomic::Ordering::SeqCst),
        3,
        "it stops at the probe that answered, not at the budget"
    );
}

/// THE WAIT IS BOUNDED, AND ITS FALLBACK IS THE CALLER'S OWN CORRECTION. A
/// fold that never reaches the block — a wedged guest, a module whose feed
/// went quiet below the write, a node that is simply slow — must not hold a
/// page create open. The budget runs out and the answer is "no", which is what
/// puts `create_page`/`delete_page` back on the hand-correction they have
/// always carried.
#[tokio::test(flavor = "current_thread")]
async fn a_fold_that_never_arrives_gives_up_inside_its_budget() {
    let (origin, served) = node_scripting_its_fold_watermark(vec![Some("6:0")]).await;
    let rpc = rpc_client(&origin).expect("stub client");

    let arrived = await_fold(&rpc, "pages", &empty_pages_probe(), 9).await;

    assert!(!arrived, "the fold never reached block 9");
    assert_eq!(
        served.load(std::sync::atomic::Ordering::SeqCst),
        FOLD_WAIT_PROBES as usize,
        "bounded: it probes its budget and stops, never forever"
    );
}

/// ABSENT IS UNKNOWN, NOT "NOT YET". A module with no index guest, a fresh
/// database, or one a boundary stamp wiped reports no watermark at all —
/// forever. Reading that as "still folding" would spend the whole budget on
/// every write against such a module, so it stops on the FIRST reply.
#[tokio::test(flavor = "current_thread")]
async fn an_unstamped_reply_stops_the_wait_instead_of_spending_it() {
    let (origin, served) = node_scripting_its_fold_watermark(vec![None]).await;
    let rpc = rpc_client(&origin).expect("stub client");

    let arrived = await_fold(&rpc, "pages", &empty_pages_probe(), 9).await;

    assert!(!arrived);
    assert_eq!(
        served.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "an unknown watermark is answered once, never waited on"
    );
}

/// A READ WAITS FOR WHAT THIS CLIENT ALREADY KNOWS, AND FOR NOTHING ELSE.
///
/// The height a read owes itself is learned by whoever learned it — the write
/// that was signed, the op the stream delivered — and asked for at the read,
/// because the two are routinely different callers with several layers between
/// them. Three facts, and the middle one is the whole mechanism:
///
/// - a read with nothing outstanding pays NO probe, which is the ordinary page
///   open and the reason this costs the app's highest-frequency read nothing;
/// - a read behind a known block waits for it;
/// - once the fold is SEEN past that block the requirement is retired, because
///   the tip is monotonic and a later read cannot fall behind it again.
#[tokio::test(flavor = "current_thread")]
async fn a_read_waits_out_a_block_this_client_already_knows_about() {
    use std::sync::atomic::Ordering::SeqCst;
    let (origin, served) = node_scripting_its_fold_watermark(vec![Some("6:0"), Some("9:0")]).await;
    let rpc = rpc_client(&origin).expect("stub client");

    assert!(
        await_seen_fold(&rpc, "pages", &empty_pages_probe()).await,
        "nothing outstanding is not a stale read — it is nothing to wait for"
    );
    assert_eq!(served.load(SeqCst), 0, "and it costs no request at all");

    note_module_block(&rpc, "pages", 9);
    assert!(await_seen_fold(&rpc, "pages", &empty_pages_probe()).await);
    assert_eq!(
        served.load(SeqCst),
        2,
        "stale, then caught up: it waited for the fold to carry block 9"
    );

    assert!(await_seen_fold(&rpc, "pages", &empty_pages_probe()).await);
    assert_eq!(
        served.load(SeqCst),
        2,
        "the fold was observed past 9 — every later read is free"
    );
}

/// A WAIT THAT GAVE UP LEAVES THE NEXT READ STILL OWING IT — the autosave's
/// duplicate line, pinned.
///
/// The document tick reads the tree, diffs the buffer against it, writes, and
/// reads back. The NEXT tick reads again, and `document_plan` pairs the
/// disturbed middle POSITIONALLY: a tree still missing the line the previous
/// tick inserted is not merely stale, it makes the plan emit a second
/// `InsertBlock` for a line that is already on chain. So the requirement is
/// retired by an OBSERVED fold and by nothing else — never by the budget
/// running out, which is precisely the case where the read cannot be trusted.
#[tokio::test(flavor = "current_thread")]
async fn a_wait_that_gave_up_leaves_the_next_read_still_owing_it() {
    use std::sync::atomic::Ordering::SeqCst;
    let (origin, served) = node_scripting_its_fold_watermark(vec![Some("6:0")]).await;
    let rpc = rpc_client(&origin).expect("stub client");
    note_module_block(&rpc, "pages", 9);

    assert!(!await_seen_fold(&rpc, "pages", &empty_pages_probe()).await);
    assert_eq!(served.load(SeqCst), FOLD_WAIT_PROBES as usize);

    assert!(!await_seen_fold(&rpc, "pages", &empty_pages_probe()).await);
    assert_eq!(
        served.load(SeqCst),
        2 * FOLD_WAIT_PROBES as usize,
        "the next tick owes the same block: a spent budget is not a fold"
    );
}

/// AN OP THE STREAM DELIVERED IS WAITED OUT BY THE RELOAD BEHIND IT.
///
/// A push reports APPLICATION — the acceptance gap closed, the FOLD gap still
/// open, since the node's block loop writes the op feed and the index folds
/// behind it on its own runner. Every structural pages op asks for a reload,
/// and `LiveRefresh`'s structural half is applied unconditionally, so a reload
/// that read a snapshot predating the push would install the tree as it was
/// BEFORE it — the deleted line still there, the inserted one missing — with
/// no further op coming to correct it.
///
/// The height rides in the push, so it is recorded where the push is decoded
/// rather than threaded down through the update, the handler's debounce and
/// the resync extern's argument list.
#[tokio::test(flavor = "current_thread")]
async fn an_op_the_stream_delivered_is_waited_out_by_the_reload_behind_it() {
    use std::sync::atomic::Ordering::SeqCst;
    let (origin, served) = node_scripting_its_fold_watermark(vec![Some("12:0")]).await;
    let rpc = rpc_client(&origin).expect("stub client");

    let moved = folded_update(
        &origin,
        "pages",
        ducktape_rpc::StreamOp {
            height: 12,
            seq: 0,
            time: 0,
            origin: ducktape_rpc::StreamOrigin {
                kind: ducktape_rpc::StreamOriginKind::External,
                id: None,
            },
            payload: Some(
                serde_json::from_slice(&pages::encode_msg(&PageMsg::MoveBlock {
                    block_id: "b1".into(),
                    parent: Some("page".into()),
                    after: None,
                }))
                .expect("payload json"),
            ),
            payload_hex: None,
            assigned: None,
            assigned_hex: None,
        },
    )
    .await
    .expect("a structural op is visible to the shell");
    assert!(moved.load_pages, "this is the op that buys a reload");
    assert_eq!(
        served.load(SeqCst),
        0,
        "recording the height is bookkeeping, not a request"
    );

    assert!(await_seen_fold(&rpc, "pages", &empty_pages_probe()).await);
    assert_eq!(
        served.load(SeqCst),
        1,
        "the reload waited for the fold to carry the pushed block"
    );
}

/// THE WAIT SITS BETWEEN THE WRITE AND THE RELOAD, AND THE CORRECTION SURVIVES
/// IT. Both facts are source shapes: a wait placed after the reload would read
/// the same stale index it was meant to outlast, and a correction deleted on
/// the strength of the wait would take the fallback with it — the watermark is
/// bounded and can be absent, so it narrows the window rather than closing it.
#[test]
fn the_pages_write_reloads_wait_first_and_keep_their_correction() {
    const CHAT: &str = include_str!("../chat.rs");
    for (name, correction) in [
        ("create_page", "data.active_page = page_id"),
        ("delete_page", "data.pages.retain("),
    ] {
        let body = CHAT
            .split(&format!("pub async fn {name}("))
            .nth(1)
            .unwrap_or_else(|| panic!("{name} is declared"))
            .split("\npub ")
            .next()
            .unwrap_or_else(|| panic!("{name} body"));
        let wait = body
            .find("await_fold(")
            .unwrap_or_else(|| panic!("{name} waits for the fold that carries its write"));
        let reload = body
            .find("load_pages_data(")
            .unwrap_or_else(|| panic!("{name} reloads"));
        assert!(wait < reload, "{name} waits BEFORE it reloads");
        assert!(
            body.contains(correction),
            "{name} keeps its correction — the wait is bounded, not a guarantee"
        );
    }
}
