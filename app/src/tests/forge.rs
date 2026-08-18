use super::*;

#[test]
fn forge_depth_rides_the_established_seams() {
    // the forge handlers moved out of lifecycle.ice into their own file;
    // the seams they guard did not, so the guard reads both.
    let lifecycle = inlined(concat!(
        include_str!("../ui/handlers/lifecycle.ice"),
        include_str!("../ui/handlers/forge.ice"),
    ));
    let forge = inlined(include_str!("../ui/components/forge.ice"));
    let backend = inlined(include_str!("../ui/extern/backend.ice"));
    let forge_state = inlined(include_str!("../ui/state/forge.ice"));
    let onboarding = inlined(include_str!("../ui/handlers/onboarding.ice"));

    // the item discussion IS a chat surface: hydrated through the chat
    // lanes and spliced by the SAME fold the chat pane uses, scoped to
    // the item's hidden channel — never a forge-private message path.
    assert!(lifecycle.contains("forge_discussion = folded_chat.forge_discussion"));
    assert!(lifecycle.contains("fold_live_chat(next.chat"));
    assert!(lifecycle.contains(
        "run every send_message(connected_rpc, password, forge_item_channel, forge_discussion_pending"
    ));

    // Replace lanes own request freshness. Their payloads keep only the
    // semantic scope needed when the selected repo/item moves without a new
    // request: channel for discussion, and repo/revision/path for code.
    assert!(lifecycle.contains(
        "run replace lane=forge_discussion load_forge_discussion(connected_rpc, forge_item_channel)"
    ));
    assert!(lifecycle.contains("return if next.channel_id != forge_item_channel"));
    let screen = include_str!("../ui/screens/forge.ice");
    assert!(screen.contains("return if next.path != tree_path"));
    assert!(screen.contains("return if !empty(tree_rev) && next.rev != tree_rev"));
    assert!(backend.contains(
        "load_forge_discussion(rpc:str, channel_id:str) -> ForgeDiscussionData ! AppError"
    ));
    assert!(
        backend.contains(
            "forge_tree(rpc:str, repo:str, rev:str, path:str) -> ForgeTreeData ! AppError"
        )
    );
    assert!(
        backend.contains("forge_blob(rpc:str, repo:str, rev:str, path:str) -> BlobView ! AppError")
    );
    for deleted in [
        concat!("forge_", "discussion_generation"),
        concat!("forge_", "code_generation"),
    ] {
        for (path, source) in [
            ("state/forge.ice", forge_state.as_str()),
            ("handlers/forge.ice", lifecycle.as_str()),
            ("handlers/onboarding.ice", onboarding.as_str()),
            ("extern/backend.ice", backend.as_str()),
        ] {
            assert!(
                !source.contains(deleted),
                "{path} still carries deleted request token `{deleted}`"
            );
        }
    }

    // a review pins the source head the reviewer saw; the merge CASes
    // BOTH heads (recompute on a moved branch, never a blind retry).
    //
    // the line comments ride INSIDE the review's own transaction — there is
    // no standalone comment op, so a comment cannot land without the
    // verdict it was written under, and it cannot outlive the diff it
    // anchors to (`keep_staged_comments` drops them when the head moves).
    assert!(backend.contains(
        "submit_forge_review(rpc:str, password:str, repo:str, number:i64, verdict:ForgeReviewVerdict, body:str, commit_oid:str, comments:[ForgeDraftComment])"
    ));
    assert!(backend.contains(
        "merge_forge_pr(rpc:str, password:str, repo:str, number:i64, source_branch:str, expected_source_oid:str, prev_target_oid:str)"
    ));

    // committed forge ops refresh scoped slices through the handler's one
    // terminal parallel — no polling, no per-op full reloads. The repo LIST
    // is the one slice with no open-scope of its own, so it carries the forge
    // surface's own gate: a chain op must not query a list that is not on
    // screen.
    assert!(lifecycle.contains(
        "run replace lane=forge_live forge_live_refresh(connected_rpc, forge_repo, forge_item_number, next.kind, next.module, next.forge, (shell_tab == ShellTab.forge), forge_generation)"
    ));
    assert_no_polling(&lifecycle);

    // approvals stay advisory in the merge box — `MergeAdvisory` is the
    // ONLY thing said above the merge button, and it recommends, never
    // refuses. The merged state renders the CAS'd commit.
    let forge_screen = inlined(include_str!("../ui/screens/forge.ice"));
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

/// THE COMMITTED LIST IS THE WHOLE CARD ANSWER. A former follow-up lane launched
/// from `forge_loaded`, fetched every repo mirror and walked README/tree facts.
/// That made the tab wait on work no card needs. Keep the landing handler pure
/// state installation and keep mirror work behind the explicit merge act.
#[test]
fn forge_repo_list_never_launches_mirror_details_work() {
    let backend = inlined(include_str!("../backend/forge.rs"));
    let list_loader = backend
        .split_once("pub async fn load_forge(")
        .expect("forge list loader")
        .1
        .split_once("pub async fn load_forge_repo(")
        .expect("repo loader boundary")
        .0;
    assert!(list_loader.contains("list_forge_repos"));
    for mirror_work in [
        "load_forge_details",
        "repo_card_facts",
        "sync_forge_mirror",
        "spawn_blocking",
    ] {
        assert!(
            !list_loader.contains(mirror_work),
            "the repo list must not start {mirror_work}"
        );
    }
    assert!(!backend.contains("pub async fn load_forge_details("));
    assert!(!backend.contains("fn repo_card_facts("));
    assert!(
        backend.contains("fn sync_forge_mirror("),
        "merge preflight still owns its client-computed commit mirror"
    );

    let handlers = inlined(include_str!("../ui/handlers/forge.ice"));
    let loaded = handlers
        .split_once("on forge_loaded(next)")
        .expect("forge loaded handler")
        .1
        .split_once("\non ")
        .expect("forge loaded arm")
        .0;
    assert!(loaded.contains("forge_repos = next.repos"));
    assert!(!loaded.contains("run "));
    assert!(!handlers.contains("forge_details"));

    let externs = inlined(include_str!("../ui/extern/backend.ice"));
    assert!(externs.contains("ForgeRepo(name:str, head:str)"));
    assert!(!externs.contains("load_forge_details"));

    let components = inlined(include_str!("../ui/components/forge.ice"));
    let header = components
        .split_once("component ForgeOrgHeader(")
        .expect("forge org header")
        .1
        .split_once("\ncomponent ")
        .expect("forge org header boundary")
        .0;
    assert!(header.contains("answered:bool"));
    assert!(header.contains("if answered"));
    assert!(!header.contains("if connected"));

    let card = components
        .split_once("component RepoCard(")
        .expect("repo card")
        .1
        .split_once("\ncomponent ")
        .expect("repo card boundary")
        .0;
    assert!(card.contains("repo.name"));
    assert!(card.contains("repo.head"));
    for removed in [
        "repo.about",
        "repo.language",
        "repo.updated_at",
        "relative_time",
    ] {
        assert!(!card.contains(removed), "repo card must not read {removed}");
    }
}

/// CODE BROWSING IS AN API READ, NOT A COLD CLONE. The root tree query resolves
/// an empty revision to one exact commit; every directory and blob click then
/// sends that commit back. Neither loader may touch the merge-only mirror or a
/// blocking git task.
#[test]
fn forge_code_loaders_query_only_the_requested_tree_or_blob() {
    let backend = include_str!("../backend/forge.rs");
    let tree = backend
        .split_once("pub async fn forge_tree(")
        .expect("tree loader")
        .1
        .split_once("pub async fn forge_blob(")
        .expect("blob loader boundary")
        .0;
    let blob = backend
        .split_once("pub async fn forge_blob(")
        .expect("blob loader")
        .1
        .split_once("pub fn forge_live_hit(")
        .expect("blob loader boundary")
        .0;
    for (loader, query) in [(tree, "\"tree\""), (blob, "\"blob\"")] {
        assert!(loader.contains(query));
        for field in ["\"repo\": &repo", "\"rev\": &rev", "\"path\": &path"] {
            assert!(loader.contains(field));
        }
        assert!(loader.contains("client.query(\"forge\", &query).await?"));
        for full_repo_work in [
            "sync_forge_mirror",
            "mirror_holding_revision",
            "spawn_blocking",
        ] {
            assert!(
                !loader.contains(full_repo_work),
                "Code loader must not start {full_repo_work}"
            );
        }
    }

    let handlers = include_str!("../ui/handlers/forge.ice");
    assert!(
        !handlers.contains("forge_tree(") && !handlers.contains("forge_blob("),
        "the browse launches from ForgeCodeBrowser, not the app plane"
    );
    let screen = include_str!("../ui/screens/forge.ice");
    assert!(screen.contains(
        "run replace lane=tree forge_tree(connected_rpc, repo, \"\", \"\")"
    ));
    assert!(screen.contains("run replace lane=tree forge_tree(rpc, repo_now, tree_rev, path)"));

    let screen = include_str!("../ui/screens/forge.ice");
    assert!(
        screen.contains("run replace lane=blob forge_blob(rpc, repo_now, rev, path)"),
        "the blob read launches from ForgeCodeBrowser's local handler"
    );
}

/// Forge's repo chrome used to stack three independent rows — crumb, every
/// branch, then tabs — before a reader reached any code or tracker content.
/// Keep branch context in the tab row and keep detail navigation in the
/// persistent repo bar, so neither can quietly grow another empty band.
#[test]
fn forge_layout_keeps_repo_navigation_compact() {
    let screen = inlined(include_str!("../ui/screens/forge.ice"));

    let repo_body = screen
        .split_once("if forge_item_number <= 0")
        .expect("repo body")
        .1
        .split_once("match tab")
        .expect("repo navigation boundary")
        .0;
    let tabs_end = repo_body
        .find("emit(select_forge_tab, ForgeTab.issues)")
        .expect("issues tab");
    let branches = repo_body
        .find("for branch in branches")
        .expect("branch strip");
    assert!(
        tabs_end < branches,
        "branch context follows the tabs in their shared navigation row"
    );
    assert_eq!(repo_body.matches("for branch in branches").count(), 1);

    let item_body = screen
        .split_once("if forge_item_number > 0 && item_phase == ForgePhase.ready")
        .expect("detail back control")
        .1;
    assert!(item_body.starts_with("\n                BackToList"));
    assert_eq!(screen.matches("BackToList kind=forge_item_kind").count(), 1);
}

/// Source and patch rows are one code-reading surface. The source rows render
/// in the backend extern `forge_code` now (syntax colour needs per-span inks
/// Ice cannot carry), but their metrics and the diff's must not drift apart:
/// this pins the Rust constants to the Ice diff row's values, and the embed
/// to the screen.
#[test]
fn forge_source_and_diff_rows_share_a_compact_code_style() {
    let source = include_str!("../backend/forge.rs");
    assert!(source.contains("pub const CODE_SIZE: f32 = 11.5;"));
    assert!(source.contains("pub const CODE_ROW_HEIGHT: f32 = 20.0;"));
    assert!(source.contains("pub const CODE_GUTTER_WIDTH: f32 = 44.0;"));
    let screen = inlined(include_str!("../ui/screens/forge.ice"));
    assert!(
        screen.contains("extern forge_code(cached_source, file_path, dark) #forge-code"),
        "the code pane mounts the highlighted reader"
    );
    assert!(
        screen.contains("lazy file_text by file_text, file_path, dark as cached_source"),
        "the reader's memo boundary is the Ice mount's lazy — the app keeps \
         zero raw iced Lazy uses"
    );

    let components = inlined(include_str!("../ui/components/forge.ice"));
    let diff = components
        .split_once("component DiffRow(")
        .expect("diff row")
        .1
        .split_once("\ncomponent ")
        .expect("diff row boundary")
        .0;
    assert!(diff.contains("font=code_semibold @text-diff_add_fg"));
    assert!(diff.contains("font=code_semibold @text-diff_del_fg"));
    assert!(!diff.contains("text=gutter_ink"));
    assert_eq!(diff.matches("text=forge_gutter_ink").count(), 3);
    assert_eq!(
        diff.matches("font=code @text-strong_ink").count(),
        3,
        "added, deleted and context code use the same neutral ink"
    );
    assert!(diff.contains("font=code_semibold @text-merged"));
}

/// CLOSING RETIRES THE LOAD THAT WOULD REOPEN WHAT YOU JUST LEFT. Named request
/// lanes abort the work immediately, and the generation bump rejects a
/// completion already queued for delivery: `forge_repo_loaded` re-assigns
/// `forge_repo` and `forge_item_loaded` re-assigns `forge_item_number`, dropping
/// the user straight back into the repo or item they had just backed out of.
///
/// Review and merge launches snapshot repo + number into their completion
/// routes. That identity follows the request without requiring the backend to
/// echo UI routing state, while the busy flag still comes down before a stale
/// completion is rejected.
#[test]
fn closing_a_repo_or_an_item_retires_the_load_that_would_reopen_it() {
    let handlers = inlined(include_str!("../ui/handlers/forge.ice"));
    let close_repo = handlers
        .split_once("on forge_close_repo")
        .expect("repo close handler")
        .1
        .split_once("\non ")
        .expect("repo close arm")
        .0;
    // `forge_code` is gone from this list on purpose: the browse's lanes are
    // instance-owned now, and closing the repo unmounts the keyed instance,
    // which prunes its state and aborts its lanes.
    for lane in ["forge_repo", "forge_item", "forge_discussion"] {
        assert!(close_repo.contains(&format!("invalidate lane={lane}")));
    }
    let close_item = handlers
        .split_once("on forge_close_item")
        .expect("item close handler")
        .1
        .split_once("\non ")
        .expect("item close arm")
        .0;
    for lane in ["forge_item", "forge_discussion"] {
        assert!(close_item.contains(&format!("invalidate lane={lane}")));
    }

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
    let _ = merging.__update(__DucktapeMessage::ForgeMerged(
        "http://node".into(),
        "core".into(),
        7,
        backend::ForgeMergeOutcome {
            merged: false,
            merge_oid: String::new(),
            conflicts: vec!["app/src/main.rs".into()],
        },
    ));
    assert!(
        !merging.forge_merge_busy,
        "closing an item mid-merge must not disable Merge for the rest of the session"
    );
    // The identity check still guards the BODY: that outcome describes an item
    // nobody has open, so nothing of it is rendered.
    assert!(merging.forge_merge_conflicts.is_empty());
}

#[test]
fn forge_code_reads_are_compiler_replaced_without_ui_generations() {
    let handlers = inlined(include_str!("../ui/handlers/forge.ice"));
    assert!(!handlers.contains("forge_code_generation"));
    let component = inlined(include_str!("../ui/screens/forge.ice"));
    for launch in [
        "forge_tree(connected_rpc, repo, \"\", \"\")",
        "forge_tree(rpc, repo_now, tree_rev, path)",
    ] {
        assert!(
            component.contains(&format!("run replace lane=tree {launch}")),
            "{launch} must supersede the previous code read"
        );
    }
    let screen = inlined(include_str!("../ui/screens/forge.ice"));
    assert!(
        screen.contains("run replace lane=blob forge_blob(rpc, repo_now, rev, path)"),
        "the blob read supersedes on the component's own lane"
    );

    let backend = include_str!("../backend/forge.rs");
    assert!(backend.contains("item: item_slice.unwrap_or(noop.item)"));
}

fn materialized_code_browser(app: &mut Ducktape) -> String {
    let window = iced::window::Id::unique();
    app.console_win = Some(window);
    app.shell_tab = ShellTab::Forge;
    let _ = app.__view(window);
    let boots: Vec<__DucktapeMessage> = app.__ice_boot_queue.borrow_mut().drain(..).collect();
    for message in boots {
        let _ = app.__update(message);
    }
    app.__ice_test_scopes_forge_code_browser()
        .pop()
        .expect("the code browser materialized")
}

#[test]
fn forge_scoped_reads_do_not_call_loading_or_failure_empty() {
    let (mut failed_list, _) = Ducktape::__boot();
    failed_list.forge_generation = 3;
    let _ = failed_list.__update(__DucktapeMessage::ForgeListFailed(
        backend::HydrationError {
            generation: 3,
            message: "forge unavailable".into(),
        },
    ));
    assert_eq!(failed_list.forge_list_phase, ForgePhase::Failed);
    assert_eq!(failed_list.error, "forge unavailable");

    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();

    let _ = app.__update(__DucktapeMessage::ForgeOpenRepo("core".into()));
    assert_eq!(app.forge_repo_phase, ForgePhase::Loading);
    let generation = app.forge_generation;

    let _ = app.__update(__DucktapeMessage::ForgeRepoLoaded(backend::ForgeRepoData {
        generation,
        repo: "core".into(),
        branches: Vec::new(),
        items: Vec::new(),
    }));
    assert_eq!(app.forge_repo_phase, ForgePhase::Ready);

    // The browse guards live in ForgeCodeBrowser now: another repository is
    // another keyed instance, so the seam exercises the path and revision
    // guards the completion still carries.
    let scope = materialized_code_browser(&mut app);
    let tree = |rev: &str, path: &str, truncated: bool| {
        Ducktape::__ice_test_message_forge_code_browser_tree_loaded(
            scope.clone(),
            backend::ForgeTreeData {
                repo: "core".into(),
                rev: rev.into(),
                path: path.into(),
                born: true,
                entries: Vec::new(),
                truncated,
            },
        )
    };
    let _ = app.__update(tree("2222222222222222222222222222222222222222", "src", false));
    let state = app.__ice_test_state_forge_code_browser(&scope).expect("instance");
    assert!(
        !state.tree_born,
        "a listing for a path the browse never asked for must not paint"
    );

    let _ = app.__update(tree("1111111111111111111111111111111111111111", "", true));
    let state = app.__ice_test_state_forge_code_browser(&scope).expect("instance");
    assert!(state.tree_born);
    assert!(state.tree_truncated);
    assert_eq!(
        state.tree_rev, "1111111111111111111111111111111111111111",
        "nested tree and file reads stay pinned to the tree's commit"
    );

    let _ = app.__update(Ducktape::__ice_test_message_forge_code_browser_open_dir(
        scope.clone(),
        "http://node".into(),
        true,
        "core".into(),
        "src".into(),
    ));
    let _ = app.__update(tree("2222222222222222222222222222222222222222", "src", false));
    let state = app.__ice_test_state_forge_code_browser(&scope).expect("instance");
    assert!(
        state.tree_entries.is_empty() && !state.tree_truncated,
        "a tree from another revision must not paint"
    );

    app.forge_item_channel = "forge:core:7".into();
    let _ = app.__update(__DucktapeMessage::ForgeDiscussionLoaded(
        backend::ForgeDiscussionData {
            channel_id: "forge:core:8".into(),
            messages: vec![message(1, "wrong item", false)],
            members: Vec::new(),
        },
    ));
    assert!(
        app.forge_discussion.is_empty(),
        "another item's discussion must not paint"
    );
    let _ = app.__update(__DucktapeMessage::ForgeDiscussionLoaded(
        backend::ForgeDiscussionData {
            channel_id: "forge:core:7".into(),
            messages: vec![message(1, "right item", false)],
            members: Vec::new(),
        },
    ));
    assert_eq!(app.forge_discussion[0].body, "right item");

    let _ = app.__update(__DucktapeMessage::ForgeOpenItem(7));
    assert_eq!(app.forge_item_phase, ForgePhase::Loading);
    let generation = app.forge_generation;
    let _ = app.__update(__DucktapeMessage::ForgeItemFailed(
        backend::HydrationError {
            generation,
            message: "tracker unavailable".into(),
        },
    ));
    assert_eq!(app.forge_item_phase, ForgePhase::Failed);
    assert_eq!(app.error, "tracker unavailable");
}

#[test]
fn forge_directory_navigation_retires_the_previous_file_preview() {
    // The preview is `ForgeCodeBrowser` component state now, retired by its
    // gate rather than by a handler clear: `forge_file_header` names the file
    // only while the browse stands where it was opened — same repository,
    // same directory, same commit. The app half of a navigation still only
    // reloads the tree.
    let moved = |dir: &str, rev: &str| {
        backend::forge_file_header(
            "src".into(),
            "1111".into(),
            dir.into(),
            rev.into(),
            "src/lib.rs".into(),
        )
    };
    assert_eq!(moved("src", "1111"), "src/lib.rs");
    assert_eq!(moved("", "1111"), "", "leaving the directory retires it");
    assert_eq!(moved("src", "2222"), "", "a newer commit retires it");
    // Another repository is another instance: the call site keys the
    // component on the repo, so cross-repo staleness cannot arise at all.

    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.forge_repo = "core".into();
    let scope = materialized_code_browser(&mut app);
    let _ = app.__update(Ducktape::__ice_test_message_forge_code_browser_open_dir(
        scope.clone(),
        "http://node".into(),
        true,
        "core".into(),
        "src".into(),
    ));
    let state = app.__ice_test_state_forge_code_browser(&scope).expect("instance");
    assert_eq!(state.tree_path, "src");
    assert!(state.tree_entries.is_empty(), "navigation clears the listing it left");
}

#[test]
fn the_file_reader_owns_its_cycle_inside_the_component() {
    // The whole browse lives in `ForgeCodeBrowser` local state. The seam
    // tests above exercise the behavior; this lint pins the SHAPE — boot
    // reads the root with the props the instance was mounted with, both
    // reads run on the component's own lanes, the completions guard what
    // they answer for, and every preview surface gates on the
    // place-and-revision header.
    let screen = include_str!("../ui/screens/forge.ice");
    let handlers = include_str!("../ui/handlers/forge.ice");

    let (_, browser) = screen
        .split_once("component ForgeCodeBrowser(")
        .expect("the code browser component exists");
    let (head, _) = browser.split_once("\ncomponent ").unwrap_or((browser, ""));
    assert!(head.contains("lifetime mounted"));
    assert!(head.contains("  boot\n"));
    assert!(head.contains("run replace lane=tree forge_tree(connected_rpc, repo, \"\", \"\")"));
    assert!(head.contains("run replace lane=tree forge_tree(rpc, repo_now, tree_rev, path)"));
    assert!(head.contains("run replace lane=blob forge_blob("));
    assert!(head.contains("return if next.path != tree_path"));
    assert!(head.contains("return if next.path != file_path"));
    let gates = head.matches("forge_file_header(").count();
    assert!(gates >= 8, "every preview arm gates on the header, found {gates}");
    assert!(
        !handlers.contains("forge_blob(") && !handlers.contains("forge_tree("),
        "the app half of the browse is gone"
    );
}

#[test]
fn forge_review_completion_cannot_clear_a_new_items_draft() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.forge_repo = "core".into();
    app.forge_item_number = 7;
    app.forge_review_draft = "review seven".into();

    let _ = app.__update(__DucktapeMessage::ForgeReviewSubmit);
    assert!(app.forge_review_busy);

    app.forge_item_number = 8;
    app.forge_review_draft = "review eight".into();
    let _ = app.__update(__DucktapeMessage::ForgeReviewSubmitted(
        "http://node".into(),
        "core".into(),
        7,
        true,
    ));

    assert!(!app.forge_review_busy);
    assert_eq!(app.forge_review_draft, "review eight");
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
    let forge = inlined(include_str!("../ui/screens/forge.ice"));

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

    // AND THE CODE PANE MUST NOT CALL A MIRROR FETCH "UNBORN". The first fetch
    // can take seconds for a real repository; only the loader's born bit may
    // decide that no branch exists, and an empty born commit is distinct too.
    assert!(
        forge.contains("if tree_phase == ForgeTreePhase.loading"),
        "the in-flight tree has its own visible state"
    );
    assert!(
        forge.contains("empty(tree_entries) && !tree_born"),
        "unborn is driven by branch presence, not an empty listing"
    );
    assert!(
        forge.contains("empty(tree_entries) && tree_born"),
        "a born empty commit does not get called unborn"
    );
}
