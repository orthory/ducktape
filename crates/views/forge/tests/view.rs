//! The facts the host pushes are what the screen shows; every act leaves as
//! an intent carrying what the reader picked or typed, and a committed op
//! the host reports consumes only the drafts it read.

use forge_view::host::{
    Body, ChatBlock, CommentStage, DiffLine, ForgeBranch, ForgeItem, ForgeProps, ForgeRepo, Name,
    Number, Path, Tab, TreeEntry, branch_names, commit_label, drafts_cleared_by,
    duck_forge_item_link, filter_forge_items, forge_comment_target, forge_open_count,
    forge_push_command, pinned_branch, repo_names,
};
use forge_view::{boot_native, tick_native};
use ui_lang_guest::testing::{has_text, item, pick, press, submit, texts, type_into};
use ui_lang_guest::wire::Frame;

#[test]
fn display_limits_use_a_bounded_footer_only_when_content_is_clipped() {
    use ui_lang_guest::wire::{Length, Node};
    fn notice(node: &Node) -> Option<&Node> {
        let is_notice = node
            .key()
            .is_some_and(|key| key.ends_with("/display-notice"));
        if is_notice {
            return Some(node);
        }
        node.children().iter().find_map(|node| notice(node))
    }
    let (_, frame) = shown(&overview());
    assert!(notice(frame.root.as_ref().unwrap()).is_none());
    assert!(!has_text(&frame, "rows are not shown."));
    for (omitted, shortened) in [(100, false), (0, true), (100, true)] {
        let (_, frame) = shown(&ForgeProps {
            display_omitted: omitted,
            display_shortened: shortened,
            ..overview()
        });
        let Node::Container { height, .. } = notice(frame.root.as_ref().unwrap()).unwrap() else {
            panic!("notice is a bounded footer");
        };
        assert_eq!(*height, Some(Length::Fixed(26.0)));
        assert_eq!(has_text(&frame, "rows omitted"), omitted > 0);
        assert_eq!(has_text(&frame, "Preview shortened"), shortened);
        assert!(has_text(&frame, "core"));
    }
}

fn overview() -> ForgeProps {
    ForgeProps {
        dark: false,
        connected: true,
        org: "duckhouse".into(),
        about: "a pond".into(),
        tier: "validator".into(),
        network_chain_id: "mynet#d0cdf950".into(),
        connected_rpc: "http://127.0.0.1:1".into(),
        repos: vec![ForgeRepo {
            name: "core".into(),
            head: "main".into(),
        }],
        list_phase: "ready".into(),
        repo_phase: "idle".into(),
        tab: "code".into(),
        item_phase: "idle".into(),
        review_verdict: "comment".into(),
        tree_phase: "loading".into(),
        file_phase: "idle".into(),
        ..ForgeProps::default()
    }
}

/// The repo open on its code seat: a listing with one directory and one file.
fn repo_open() -> ForgeProps {
    ForgeProps {
        open_repo: "core".into(),
        repo_phase: "ready".into(),
        branches: vec![
            ForgeBranch {
                name: "main".into(),
                head: "1111".into(),
            },
            ForgeBranch {
                name: "feature".into(),
                head: "2222".into(),
            },
        ],
        tree_branch: "main".into(),
        items: vec![
            ForgeItem {
                number: 7,
                kind: "pr".into(),
                state: "open".into(),
                title: "a pull request".into(),
                author: "user:aa".into(),
                author_name: "aa".into(),
            },
            ForgeItem {
                number: 8,
                kind: "issue".into(),
                state: "closed".into(),
                title: "an issue".into(),
                author: "user:bb".into(),
                author_name: "bb".into(),
            },
        ],
        tree_rev: "1111".into(),
        tree_born: true,
        tree_phase: "ready".into(),
        tree_entries: vec![
            TreeEntry {
                name: "src".into(),
                path: "src".into(),
                kind: "dir".into(),
            },
            TreeEntry {
                name: "README.md".into(),
                path: "README.md".into(),
                kind: "file".into(),
            },
        ],
        ..overview()
    }
}

/// The pull request open, with one painted diff row to comment on.
fn item_open() -> ForgeProps {
    ForgeProps {
        forge_item_number: 7,
        item_phase: "ready".into(),
        forge_item_kind: "pr".into(),
        forge_item_title: "a pull request".into(),
        forge_item_state: "open".into(),
        forge_item_author: "aa".into(),
        forge_item_branches: "feature → main".into(),
        forge_item_body: "please".into(),
        forge_item_blocks: vec![ChatBlock {
            kind: "paragraph".into(),
            text: "please".into(),
            ..ChatBlock::default()
        }],
        forge_item_source_oid: "abc123".into(),
        diff_rows: vec![DiffLine {
            key: 1,
            kind: "add".into(),
            old_no: String::new(),
            new_no: "14".into(),
            sign: "+".into(),
            text: "let answer = 42;".into(),
            path: "src/main.rs".into(),
            side: "new".into(),
        }],
        ..repo_open()
    }
}

fn encoded(props: &ForgeProps) -> Vec<u8> {
    serde_json::to_vec(props).expect("props encode")
}

/// Boot and push the facts; returns the subscription id and the frame.
fn shown(props: &ForgeProps) -> (u64, Frame) {
    boot_native();
    let frame = tick_native(Vec::new());
    assert_eq!(frame.requests[0].kind, "forge.props");
    let subscription = frame.requests[0].id;
    let frame = tick_native(vec![item(subscription, &encoded(props))]);
    (subscription, frame)
}

fn one_intent(frame: &Frame) -> &ui_lang_guest::wire::Request {
    let [intent] = frame.requests.as_slice() else {
        panic!("one intent, got {:?}", frame.requests);
    };
    intent
}

#[test]
fn the_facts_the_host_pushes_are_what_the_screen_shows_and_a_card_opens_its_repo() {
    let (_, frame) = shown(&overview());
    for expected in ["duckhouse", "core", "1 repository"] {
        assert!(
            has_text(&frame, expected),
            "missing {expected:?} in {:?}",
            texts(&frame)
        );
    }
    assert!(frame.requests.is_empty(), "{:?}", frame.requests);
    let frame = tick_native(press(&frame, "Open repo"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "forge.open_repo");
    assert_eq!(
        serde_json::from_slice::<Name>(&intent.payload).expect("decodes"),
        Name {
            name: "core".into()
        }
    );
}

#[test]
fn the_repo_seats_pick_a_tab_and_the_code_browse_asks_the_host_for_a_file() {
    let (subscription, frame) = shown(&repo_open());
    assert!(has_text(&frame, "README.md"), "{:?}", texts(&frame));
    let frame = tick_native(press(&frame, "Open file"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "forge.blob");
    assert_eq!(
        serde_json::from_slice::<Path>(&intent.payload).expect("decodes"),
        Path {
            path: "README.md".into()
        }
    );
    let frame = tick_native(press(&frame, "Show pull requests"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "forge.tab");
    assert_eq!(
        serde_json::from_slice::<Tab>(&intent.payload).expect("decodes"),
        Tab {
            tab: "pulls".into()
        }
    );
    // the seat is the host's: the list shows once the tab comes back as props
    let pulls = ForgeProps {
        tab: "pulls".into(),
        ..repo_open()
    };
    let frame = tick_native(vec![item(subscription, &encoded(&pulls))]);
    assert!(has_text(&frame, "a pull request"), "{:?}", texts(&frame));
    assert!(!has_text(&frame, "an issue"));
    let frame = tick_native(press(&frame, "Open item"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "forge.open_item");
    assert_eq!(
        serde_json::from_slice::<Number>(&intent.payload).expect("decodes"),
        Number { number: 7 }
    );
}

/// The branch selector is the host's own pick list over the repo's born
/// branches, selecting the branch the browse is pinned to; a pick leaves as
/// an intent naming the branch. The host draws and dismisses the menu, so
/// the view carries no open flag for it.
#[test]
fn the_branch_selector_lists_the_branches_and_a_pick_names_the_branch_to_the_host() {
    let (subscription, frame) = shown(&repo_open());
    assert_eq!(branch_names(&repo_open().branches), ["main", "feature"]);
    assert_eq!(pinned_branch("main").as_deref(), Some("main"));
    assert_eq!(pinned_branch(""), None);
    let frame = tick_native(pick(&frame, "ForgeView/forge/branch-pick", "feature"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "forge.branch");
    assert_eq!(
        serde_json::from_slice::<Name>(&intent.payload).expect("decodes"),
        Name {
            name: "feature".into()
        }
    );
    // pinned past every branch, the selector's hint reads the commit itself
    assert_eq!(commit_label("3333333333333333"), "333333333333");
    assert_eq!(commit_label(""), "…");
    let past_every_branch = ForgeProps {
        tree_branch: String::new(),
        tree_rev: "3333333333333333".into(),
        ..repo_open()
    };
    let frame = tick_native(vec![item(subscription, &encoded(&past_every_branch))]);
    assert!(has_text(&frame, "333333333333"), "{:?}", texts(&frame));
}

/// The repository switcher is the same shape over the forge's repositories:
/// a pick leaves as an intent naming the repo to open.
#[test]
fn the_repository_switcher_lists_the_repos_and_a_pick_names_the_repo_to_the_host() {
    let props = ForgeProps {
        repos: vec![
            ForgeRepo {
                name: "core".into(),
                head: "1111".into(),
            },
            ForgeRepo {
                name: "playground".into(),
                head: "2222".into(),
            },
        ],
        ..repo_open()
    };
    assert_eq!(repo_names(&props.repos), ["core", "playground"]);
    let (_, frame) = shown(&props);
    let frame = tick_native(pick(&frame, "ForgeView/forge/repo-pick", "playground"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "forge.open_repo");
    assert_eq!(
        serde_json::from_slice::<Name>(&intent.payload).expect("decodes"),
        Name {
            name: "playground".into()
        }
    );
}

/// The tab bar stays up over an open item, and a tab press there leaves as
/// the same `forge.tab` intent a press over a list does — the host closes
/// the item on it. The bar's other exit, the crumb, names the repo overview.
#[test]
fn the_tabs_stay_over_an_open_item_and_the_crumb_leads_back_to_every_repo() {
    let (_, frame) = shown(&item_open());
    assert!(has_text(&frame, "a pull request"), "{:?}", texts(&frame));
    assert!(has_text(&frame, "Issues"), "{:?}", texts(&frame));
    let frame = tick_native(press(&frame, "Show issues"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "forge.tab");
    assert_eq!(
        serde_json::from_slice::<Tab>(&intent.payload).expect("decodes"),
        Tab {
            tab: "issues".into()
        }
    );
    let (_, frame) = shown(&item_open());
    let frame = tick_native(press(&frame, "All repos"));
    assert_eq!(one_intent(&frame).kind, "forge.close_repo");
}

#[test]
fn a_review_leaves_with_its_body_and_a_landed_one_consumes_the_drafts_it_read() {
    let (subscription, frame) = shown(&item_open());
    assert!(has_text(&frame, "a pull request"), "{:?}", texts(&frame));
    // a picked diff line opens the line composer; staging it leaves as an
    // intent and empties the composer
    let frame = tick_native(press(&frame, "Comment on this line"));
    assert!(frame.requests.is_empty(), "picking a line runs no intent");
    assert!(
        has_text(&frame, "src/main.rs:14 (new)"),
        "{:?}",
        texts(&frame)
    );
    let frame = tick_native(type_into(&frame, "Comment on this line…", "nit"));
    let frame = tick_native(submit(&frame, "Comment on this line…"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "forge.comment_stage");
    assert_eq!(
        serde_json::from_slice::<CommentStage>(&intent.payload).expect("decodes"),
        CommentStage {
            path: "src/main.rs".into(),
            line: "14".into(),
            side: "new".into(),
            body: "nit".into(),
        }
    );
    assert!(
        !has_text(&frame, "src/main.rs:14 (new)"),
        "the pick is spent"
    );
    // the review body leaves on submit …
    let frame = tick_native(type_into(&frame, "Leave a review…", "looks right"));
    assert!(frame.requests.is_empty(), "typing runs no handler");
    let frame = tick_native(press(&frame, "Submit review"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "forge.review_submit");
    assert_eq!(
        serde_json::from_slice::<Body>(&intent.payload).expect("decodes"),
        Body {
            body: "looks right".into()
        }
    );
    // … and stays in the box until the host says the review landed
    let frame = tick_native(vec![item(subscription, &encoded(&item_open()))]);
    let frame = tick_native(press(&frame, "Submit review"));
    assert_eq!(one_intent(&frame).kind, "forge.review_submit");
    let landed = ForgeProps {
        drafts_cleared: 1,
        drafts_scope: "review".into(),
        ..item_open()
    };
    let frame = tick_native(vec![item(subscription, &encoded(&landed))]);
    let review_body = ui_lang_guest::testing::keys(&frame)
        .into_iter()
        .find(|key| key.ends_with("forge-review-body"))
        .expect("the review box");
    let submit_review = ui_lang_guest::testing::find(&frame, &review_body);
    assert!(
        matches!(submit_review, Some(ui_lang_guest::wire::Node::Input { value, .. }) if value.is_empty()),
        "the landed review consumed its body: {submit_review:?}"
    );
}

#[test]
fn the_readings_repeat_the_apps_words() {
    let hint = forge_push_command("http://127.0.0.1:38259/");
    assert_eq!(
        hint,
        "git remote add ducktape http://127.0.0.1:38259/forge/my-repo && git push ducktape main"
    );
    let item = |number: i64, kind: &str, state: &str| ForgeItem {
        number,
        kind: kind.into(),
        state: state.into(),
        title: format!("item {number}"),
        author: "user:aa".into(),
        author_name: "aa".into(),
    };
    let items = vec![
        item(1, "pr", "open"),
        item(2, "pr", "merged"),
        item(3, "issue", "open"),
        item(4, "issue", "closed"),
    ];
    assert_eq!(filter_forge_items(&items, "pulls").len(), 2);
    assert_eq!(filter_forge_items(&items, "issues").len(), 2);
    assert!(filter_forge_items(&items, "code").is_empty());
    assert_eq!(forge_open_count(&items, "pr"), 1);
    assert_eq!(forge_open_count(&items, "issue"), 1);
    assert_eq!(
        duck_forge_item_link("ducktape", 58, "mynet#d0cdf950"),
        "duck://forge/ducktape/58?net=d0cdf950"
    );
    assert_eq!(
        forge_comment_target("src/main.rs", "14", "new"),
        "src/main.rs:14 (new)"
    );
    assert_eq!(forge_comment_target("", "14", "new"), "");
    assert!(drafts_cleared_by("item", "review") && drafts_cleared_by("item", "comment"));
    assert!(drafts_cleared_by("review", "comment") && !drafts_cleared_by("comment", "review"));
}
