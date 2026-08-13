use ::chat;
use ::forge;
use ::node;

use commonware_cryptography::{Signer as _, ed25519};
use iced::futures::StreamExt as _;

use super::*;

#[test]
fn the_rail_seats_collaboration_and_node_operations_separately() {
    let nav = shell_nav("chat".into(), 3, true);
    let ids: Vec<&str> = nav.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "chat",
            "shell",
            "pages",
            "forge",
            "agents",
            "files",
            "explorer",
            "node",
            "members",
            "governance"
        ]
    );
    let forge = nav.iter().find(|item| item.id == "forge").unwrap();
    assert!(forge.live, "an engaged agent pulses the forge seat");
    assert_eq!(
        nav.iter().find(|item| item.id == "node").unwrap().title,
        "Node"
    );
    assert_eq!(
        nav.iter()
            .find(|item| item.id == "governance")
            .unwrap()
            .badge,
        3
    );
}

#[test]
fn the_forge_hint_is_a_command_that_actually_pushes() {
    // Verified end to end against a live node: a push to a NEW lowercase name
    // creates the repo, and an uppercase one 404s the ref advertisement because
    // `forge::norm_repo` accepts `[a-z0-9._-]` only — so the placeholder has to
    // be a name the reader can paste unchanged.
    let hint = forge_push_command("http://127.0.0.1:38259".into());
    assert_eq!(
        hint,
        "git remote add ducktape http://127.0.0.1:38259/forge/my-repo && git push ducktape main"
    );
    let placeholder = hint
        .split("/forge/")
        .nth(1)
        .and_then(|rest| rest.split(' ').next())
        .expect("the hint names a repo");
    assert!(forge::norm_repo(placeholder).is_ok());
    // A trailing slash on the endpoint must not double up in the URL.
    assert_eq!(forge_push_command("http://127.0.0.1:38259/".into()), hint);
}

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
fn a_count_of_one_takes_the_singular_noun() {
    assert_eq!(plural(1, "agent".into(), "agents".into()), "1 agent");
    assert_eq!(plural(0, "agent".into(), "agents".into()), "0 agents");
    assert_eq!(plural(2, "agent".into(), "agents".into()), "2 agents");
    // the register subtitles that used to read `1 agents` / `1 validators`.
    assert_eq!(tally_note(1, 4), "1 approval · 3 more for quorum");
}

/// A SUBTITLE THAT COUNTS NOTHING, OVER A PLATE THAT ALREADY SAID SO. Approvals
/// read `0 open · 0 settled` directly above "No proposals yet — a membership or
/// configuration change opens the first one." Both halves of the subtitle were
/// zero, so it repeated the plate in digits. #996 settled the rule for the
/// bell's `0 unread` and Channel details' `MEMBERS 0`; these four folds are the
/// sites it did not reach, and each of their screens plates the empty case in
/// words already.
///
/// A zero BESIDE a real reading is a different thing and stays: `1 agent ·
/// 0 working` is the sentence doing its job.
#[test]
fn a_subtitle_that_is_all_zeros_says_nothing_at_all() {
    let agent = |live: bool| AgentRow {
        id: "agent-1".into(),
        name: "Quackbot".into(),
        initials: "QU".into(),
        capability: "mock-llm-1".into(),
        status: "active".into(),
        owner_key: String::new(),
        owner_handle: String::new(),
        created_at: 0,
        is_mine: false,
        live,
        tools: 0,
        secrets: 0,
        subagent_budget: 0,
        allowed_actions: Vec::new(),
        skills: Vec::new(),
        caps: Vec::new(),
    };
    let proposal = |open: bool| ProposalRow {
        id: "proposal-1".into(),
        action: "add_resident".into(),
        detail: String::new(),
        proposer: String::new(),
        status: "open".into(),
        deadline: 0,
        approvals: 0,
        rejections: 0,
        rule: "threshold".into(),
        required_yes: 1,
        electorate: 1,
        open,
        settled_height: 0,
    };
    let entry = FsEntry {
        key: 0,
        path: "/shared/notes".into(),
        name: "notes".into(),
        kind: "file".into(),
        size: 0,
        object: String::new(),
    };

    let human = MemberRow {
        key: "aa".into(),
        label: "aa".into(),
        is_agent: false,
        role: "validator".into(),
        is_this_node: false,
        model: String::new(),
        live: true,
    };

    // Nothing there: the plate on each screen says it in words.
    assert_eq!(members_summary(true, Vec::new()), "");
    assert_eq!(agents_summary(true, Vec::new()), "");
    assert_eq!(proposals_summary(true, Vec::new()), "");
    assert_eq!(fs_counts_summary(true, true, Vec::new()), "");

    // Something there: every subtitle speaks, zeros included.
    assert_eq!(members_summary(true, vec![human]), "1 human · 0 agents");
    assert_eq!(
        agents_summary(true, vec![agent(false)]),
        "1 agent · 0 working"
    );
    assert_eq!(
        proposals_summary(true, vec![proposal(true)]),
        "1 open · 0 settled"
    );
    assert_eq!(
        proposals_summary(true, vec![proposal(false)]),
        "0 open · 1 settled"
    );
    assert_eq!(
        fs_counts_summary(true, true, vec![entry]),
        "1 file · 0 dirs"
    );
}

#[test]
fn quorum_dots_count_the_frozen_rule_not_the_electorate() {
    // three of the four REQUIRED signatures are in, inside a six-node pool.
    let dots = quorum_dots(3, 4);
    assert_eq!(dots.len(), 4);
    assert_eq!(dots.iter().filter(|seat| seat.filled).count(), 3);
    assert_eq!(tally_label(3, 4), "3 / 4");
    assert_eq!(tally_tone(3, 4), "near");
    assert_eq!(tally_tone(1, 4), "far");
    assert_eq!(tally_note(3, 4), "3 approvals · 1 more for quorum");
    assert_eq!(tally_note(4, 4), "quorum met");
    assert_eq!(approve_label(3, 4), "Approve →");
    assert_eq!(approve_label(1, 4), "Approve");
}

#[test]
fn a_diff_paints_gutters_signs_and_kinds() {
    let rows = diff_lines(
        "diff --git a/round.rs b/round.rs\n@@ -138,3 +138,4 @@ impl RoundState {\n ctx\n-gone\n+added\n"
            .into(),
    );
    let kinds: Vec<&str> = rows.iter().map(|row| row.kind.as_str()).collect();
    assert_eq!(kinds, ["file", "hunk", "ctx", "del", "add"]);
    assert_eq!(
        rows.iter()
            .map(|row| row.key)
            .collect::<BTreeSet<_>>()
            .len(),
        rows.len(),
        "every parsed patch row has a unique keyed identity"
    );
    let context = &rows[2];
    assert_eq!(
        (context.old_no.as_str(), context.new_no.as_str()),
        ("138", "138")
    );
    assert_eq!(rows[3].sign, "-");
    assert_eq!(rows[3].old_no, "139");
    assert_eq!(rows[4].sign, "+");
    assert_eq!(rows[4].new_no, "139");
    assert_eq!(rows[4].text, "added");
}

#[test]
fn a_changed_diff_rekeys_rows_instead_of_transferring_focus() {
    let before = diff_lines(
        concat!(
            "+++ b/one.rs\n",
            "@@ -10,1 +10,2 @@\n",
            " kept\n",
            "+target\n",
        )
        .into(),
    );
    let after = diff_lines(
        concat!(
            "+++ b/one.rs\n",
            "@@ -10,1 +10,3 @@\n",
            " kept\n",
            "+unrelated\n",
            "+target\n",
        )
        .into(),
    );
    let after_identical = diff_lines(
        concat!(
            "+++ b/one.rs\n",
            "@@ -10,1 +10,3 @@\n",
            " kept\n",
            "+target\n",
            "+target\n",
        )
        .into(),
    );
    let before_target = before
        .iter()
        .find(|row| row.text == "target")
        .expect("the target row is parsed before the insertion");
    let after_target = after
        .iter()
        .find(|row| row.text == "target")
        .expect("the target row is parsed after the insertion");

    assert_ne!(before_target.new_no, after_target.new_no);
    assert_ne!(before_target.key, after_target.key);
    assert!(
        after_identical
            .iter()
            .filter(|row| row.text == "target")
            .all(|row| row.key != before_target.key),
        "even an indistinguishable inserted line cannot inherit a focused button"
    );
    assert_eq!(
        diff_lines(
            concat!(
                "+++ b/one.rs\n",
                "@@ -10,1 +10,2 @@\n",
                " kept\n",
                "+target\n",
            )
            .into()
        ),
        before,
        "an unchanged patch keeps its keyed tree"
    );
}

/// Every code row knows which FILE it is in, across a multi-file patch.
///
/// `forge_item_diff` is the whole patch as one string, so without this a row
/// cannot say what it belongs to — and `ReviewComment` anchors on
/// `(path, line, side)`, so an unanchored row cannot carry a comment.
#[test]
fn every_code_row_carries_its_file_and_side() {
    let rows = diff_lines(
        concat!(
            "diff --git a/one.rs b/one.rs\n",
            "--- a/one.rs\n",
            "+++ b/one.rs\n",
            "@@ -1,2 +1,2 @@\n",
            " ctx\n",
            "-gone\n",
            "+added\n",
            "diff --git a/two.rs b/two.rs\n",
            "--- a/two.rs\n",
            "+++ b/two.rs\n",
            "@@ -10,1 +10,1 @@\n",
            "+second\n",
        )
        .into(),
    );
    let coded: Vec<(&str, &str, &str)> = rows
        .iter()
        .filter(|row| row.kind != "file" && row.kind != "hunk")
        .map(|row| (row.path.as_str(), row.side.as_str(), row.text.as_str()))
        .collect();
    assert_eq!(
        coded,
        [
            ("one.rs", "new", "ctx"),
            ("one.rs", "old", "gone"),
            ("one.rs", "new", "added"),
            // the second header re-points the anchor; a row after it must
            // not still claim the first file.
            ("two.rs", "new", "second"),
        ]
    );

    // headers and hunks are not commentable positions and say so.
    assert!(
        rows.iter()
            .filter(|row| row.kind == "file" || row.kind == "hunk")
            .all(|row| row.path.is_empty() && row.side.is_empty())
    );
}

/// A pure deletion writes `+++ /dev/null` — no head-side file exists, so
/// its rows are deliberately unanchorable rather than being attributed to
/// whichever file happened to come before them in the patch.
#[test]
fn a_deleted_files_rows_anchor_to_nothing() {
    let rows = diff_lines(
        concat!(
            "diff --git a/kept.rs b/kept.rs\n",
            "+++ b/kept.rs\n",
            "@@ -1,1 +1,1 @@\n",
            "+kept\n",
            "diff --git a/dropped.rs b/dropped.rs\n",
            "+++ /dev/null\n",
            "@@ -1,1 +0,0 @@\n",
            "-dropped\n",
        )
        .into(),
    );
    let dropped = rows
        .iter()
        .find(|row| row.text == "dropped")
        .expect("del row");
    assert_eq!(dropped.path, "", "a /dev/null head side anchors nothing");
    let kept = rows.iter().find(|row| row.text == "kept").expect("add row");
    assert_eq!(kept.path, "kept.rs");
}

/// A hunk BODY is bounded by the counts its header declares, so content is
/// never re-read as a header.
///
/// This is not hypothetical prettiness: a source line reading `++ x` is
/// written `+++ x` in a patch, and adding one used to be taken for the
/// `+++ b/<path>` header — which silently re-anchored every row after it to
/// a path that does not exist, and would have submitted a comment against
/// it. Any file that contains a diff, a changelog, or C++ increments can
/// produce that line.
#[test]
fn a_hunk_body_is_bounded_so_content_is_never_read_as_a_header() {
    let rows = diff_lines(
        concat!(
            "diff --git a/notes.md b/notes.md\n",
            "--- a/notes.md\n",
            "+++ b/notes.md\n",
            "@@ -1,2 +1,4 @@\n",
            " intro\n",
            // the patch's rendering of a source line `++ counter`
            "+++ counter\n",
            // and of a source line `-- dashes`
            "--- dashes\n",
            " outro\n",
        )
        .into(),
    );
    let coded: Vec<(&str, &str, &str)> = rows
        .iter()
        .filter(|row| row.kind != "file" && row.kind != "hunk")
        .map(|row| (row.kind.as_str(), row.path.as_str(), row.text.as_str()))
        .collect();
    assert_eq!(
        coded,
        [
            ("ctx", "notes.md", "intro"),
            ("add", "notes.md", "++ counter"),
            ("del", "notes.md", "-- dashes"),
            ("ctx", "notes.md", "outro"),
        ],
        "a body line was re-read as a file header"
    );
}

/// The `\ No newline at end of file` note holds no position on either side.
/// Counting it would spend the hunk's budget a line early and re-open
/// header detection inside the body.
#[test]
fn the_no_newline_note_consumes_no_line_and_no_budget() {
    let rows = diff_lines(
        concat!(
            "+++ b/tail.rs\n",
            "@@ -1,1 +1,2 @@\n",
            " kept\n",
            "+added\n",
            "\\ No newline at end of file\n",
        )
        .into(),
    );
    let added = rows
        .iter()
        .find(|row| row.text == "added")
        .expect("add row");
    assert_eq!(added.new_no, "2");
    assert_eq!(added.path, "tail.rs");
    let note = rows.last().expect("the note is a row");
    assert_eq!(note.kind, "file", "the note is not a commentable position");
    assert!(note.path.is_empty() && note.new_no.is_empty());
}

/// A range written without a comma covers exactly one line, so the body
/// budget must read `@@ -1 +1 @@` as 1 and 1 rather than 0.
#[test]
fn a_comma_less_hunk_range_covers_one_line() {
    let rows = diff_lines(concat!("+++ b/one.rs\n", "@@ -7 +7 @@\n", "+++ still content\n").into());
    let code: Vec<(&str, &str)> = rows
        .iter()
        .filter(|row| row.kind == "add")
        .map(|row| (row.new_no.as_str(), row.text.as_str()))
        .collect();
    assert_eq!(code, [("7", "++ still content")]);
}

fn stage(staged: Vec<ForgeDraftComment>, line: &str, body: &str) -> Vec<ForgeDraftComment> {
    stage_forge_comment(
        staged,
        "src/main.rs".into(),
        line.into(),
        "new".into(),
        body.into(),
    )
}

/// Clicking one gutter twice REPLACES that line's comment. Two comments on
/// one position is not a thing the author can have meant, and the anchor is
/// the row's identity precisely so this cannot stack.
#[test]
fn restaging_a_line_replaces_the_comment_already_on_it() {
    let staged = stage(Vec::new(), "14", "first");
    let staged = stage(staged, "14", "second");
    assert_eq!(staged.len(), 1);
    assert_eq!(staged[0].body, "second");
    assert_eq!(staged[0].anchor, "src/main.rs:14 (new)");

    // a DIFFERENT line is a different anchor, so it lands beside it.
    let staged = stage(staged, "15", "third");
    assert_eq!(staged.len(), 2);
    // and the same line on the OTHER side is its own position too.
    let staged = stage_forge_comment(
        staged,
        "src/main.rs".into(),
        "14".into(),
        "old".into(),
        "base side".into(),
    );
    assert_eq!(staged.len(), 3);
    assert_eq!(staged[2].anchor, "src/main.rs:14 (old)");
}

/// A draft the wire could not carry never enters the list. Each of these
/// would otherwise reach `submit_forge_review` and be rejected there — or
/// worse, land as a comment anchored to nothing.
#[test]
fn a_draft_the_wire_cannot_anchor_never_stages() {
    let unusable = [
        // a deleted file's rows carry no head-side path
        ("", "14", "new", "body"),
        // gutters are blank on the side a row does not touch
        ("src/main.rs", "", "new", "body"),
        ("src/main.rs", "0", "new", "body"),
        ("src/main.rs", "-3", "new", "body"),
        // only the two diff sides exist
        ("src/main.rs", "14", "both", "body"),
        // the module refuses an empty comment body
        ("src/main.rs", "14", "new", "   "),
    ];
    for (path, line, side, body) in unusable {
        assert!(
            stage_forge_comment(
                Vec::new(),
                path.into(),
                line.into(),
                side.into(),
                body.into()
            )
            .is_empty(),
            "staged an unusable draft: {path:?} {line:?} {side:?} {body:?}"
        );
    }
    assert!(
        !stage(Vec::new(), "14", "real").is_empty(),
        "a usable draft stages"
    );
}

/// The staged list stops at the module's OWN per-review cap, and the gate
/// the composer disables on agrees with it — both read the one constant, so
/// the button greys out on exactly the draft the list would have dropped.
#[test]
fn staging_stops_at_the_modules_own_cap() {
    let mut staged = Vec::new();
    for line in 1..=forge::MAX_REVIEW_COMMENTS {
        staged = stage(staged, &line.to_string(), "body");
    }
    assert_eq!(staged.len(), forge::MAX_REVIEW_COMMENTS);
    assert!(forge_comment_cap_reached(staged.clone()));

    let past_cap = stage(staged.clone(), "9999", "one too many");
    assert_eq!(past_cap.len(), forge::MAX_REVIEW_COMMENTS, "the cap holds");

    // replacing an EXISTING anchor is not a new row, so it still works at
    // the cap — otherwise a full list could never be corrected.
    let edited = stage(staged, "1", "edited at the cap");
    assert_eq!(edited.len(), forge::MAX_REVIEW_COMMENTS);
    assert_eq!(edited[0].body, "edited at the cap");
}

#[test]
fn dropping_one_staged_comment_leaves_the_others() {
    let staged = stage(stage(Vec::new(), "14", "a"), "15", "b");
    let kept = drop_forge_comment(staged.clone(), "src/main.rs:14 (new)".into());
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].body, "b");
    assert_eq!(
        drop_forge_comment(staged.clone(), "nothing/here.rs:1 (new)".into()).len(),
        2,
        "a miss leaves the list alone"
    );
    assert!(drop_forge_comment(staged, "src/main.rs:15 (new)".into())[0].body == "a");
}

/// The composer's visibility IS this string, and it is spelled by the same
/// helper the staged rows wear — so the header over the draft and the chip
/// it becomes can never disagree.
#[test]
fn the_composer_opens_only_on_a_picked_line() {
    assert_eq!(
        forge_comment_target("src/main.rs".into(), "14".into(), "new".into()),
        "src/main.rs:14 (new)"
    );
    assert_eq!(
        forge_comment_target(String::new(), "14".into(), "new".into()),
        "",
        "no line picked, no composer"
    );
    assert_eq!(
        forge_comment_target("src/main.rs".into(), "14".into(), "new".into()),
        stage(Vec::new(), "14", "body")[0].anchor,
        "the composer header and the staged chip are the same anchor"
    );
}

/// A branch that moves under an open composer takes the staged comments
/// with it, and says so. The alternative is a review pinning the NEW head
/// while carrying comments written about the OLD one — which `outdated`
/// would then report as current, because the pin matches.
#[test]
fn a_moved_branch_discards_the_comments_staged_against_the_old_diff() {
    let staged = stage(Vec::new(), "14", "about the old line 14");
    let held = ("aaa".to_string(), "aaa".to_string());
    let moved = ("bbb".to_string(), "aaa".to_string());

    // the same head: nothing is disturbed, and no note is raised.
    assert_eq!(
        keep_staged_comments(true, held.0.clone(), held.1.clone(), staged.clone()).len(),
        1
    );
    assert_eq!(
        keep_comment_text(true, held.0.clone(), held.1.clone(), "draft".into()),
        "draft"
    );
    assert_eq!(
        staged_comment_drop_note(
            true,
            held.0.clone(),
            held.1.clone(),
            staged.clone(),
            String::new()
        ),
        ""
    );

    // a moved head: the drafts go, and the reason is reported.
    assert!(
        keep_staged_comments(true, moved.0.clone(), moved.1.clone(), staged.clone()).is_empty()
    );
    assert_eq!(
        keep_comment_text(true, moved.0.clone(), moved.1.clone(), "draft".into()),
        ""
    );
    assert!(
        staged_comment_drop_note(
            true,
            moved.0.clone(),
            moved.1.clone(),
            staged.clone(),
            String::new()
        )
        .contains("discarded")
    );

    // a refresh that carried no item, or that resolved no head, is not a
    // move — a miss must never be read as a force-push.
    for (loaded, next, current) in [(false, "bbb", "aaa"), (true, "", "aaa"), (true, "bbb", "")] {
        assert_eq!(
            keep_staged_comments(loaded, next.into(), current.into(), staged.clone()).len(),
            1,
            "an unresolved head dropped staged comments: {loaded} {next:?} {current:?}"
        );
    }

    // nothing staged is nothing lost: a real move raises no note.
    assert_eq!(
        staged_comment_drop_note(true, moved.0, moved.1, Vec::new(), "prior banner".into()),
        "prior banner",
        "an empty composer neither loses work nor clears a standing error"
    );
}

/// The staged drafts become the module's own `ReviewComment`s — the string
/// line number parsed back to `u32` and the side back to `DiffSide`.
#[test]
fn staged_drafts_cross_the_wire_as_review_comments() {
    let staged = stage_forge_comment(
        stage(Vec::new(), "14", "on the head"),
        "old.rs".into(),
        "3".into(),
        "old".into(),
        "on the base".into(),
    );
    let wire = review_comments(staged).expect("staged drafts are wire-valid by construction");
    assert_eq!(
        wire,
        [
            forge::ReviewComment {
                path: "src/main.rs".into(),
                line: 14,
                side: forge::DiffSide::New,
                body: "on the head".into(),
            },
            forge::ReviewComment {
                path: "old.rs".into(),
                line: 3,
                side: forge::DiffSide::Old,
                body: "on the base".into(),
            },
        ]
    );
    assert!(
        review_comments(Vec::new())
            .expect("no comments is fine")
            .is_empty()
    );

    // the boundary refuses rather than posting a comment anchored to
    // nothing, even though staging should never produce one.
    let forged = vec![ForgeDraftComment {
        anchor: "src/main.rs:x (new)".into(),
        path: "src/main.rs".into(),
        line: "x".into(),
        side: "new".into(),
        body: "body".into(),
    }];
    assert!(review_comments(forged).is_err());
}

#[test]
fn a_log_line_splits_into_time_level_and_message() {
    let parts =
        split_log_line("2026-07-27T09:12:44.918Z  INFO ducktape::join: admitted resident".into());
    assert_eq!(parts.time, "2026-07-27T09:12:44.918Z");
    assert_eq!(parts.level, "INFO");
    assert_eq!(parts.message, "ducktape::join: admitted resident");

    let prose = split_log_line("no level here".into());
    assert_eq!(prose.level, "");
    assert_eq!(prose.message, "no level here");
}

#[test]
fn a_dm_id_is_pair_derived_and_cannot_be_forged() {
    let a = "aa".repeat(32);
    let b = "bb".repeat(32);
    assert_eq!(
        dm_channel_id(a.clone(), b.clone()),
        dm_channel_id(b.clone(), a.clone()),
        "both sides derive the same channel"
    );
    // the sidebar's own filter: the pair's derived channel drops out of
    // CHANNELS, an ordinary room stays, and an unknown viewer filters
    // nothing rather than guessing.
    let channel = |id: &str| ChatChannel {
        id: id.into(),
        name: id.into(),
        archived: false,
        members_only: false,
        huddle_count: 0,
        head_seq: 0,
    };
    let peers = vec![DmPeer {
        key: b.clone(),
        name: "b".into(),
        initials: "B".into(),
        is_agent: false,
        channel_id: dm_channel_id(a.clone(), b.clone()),
    }];
    let listing = vec![
        channel(&dm_channel_id(a.clone(), b.clone())),
        channel("general"),
    ];
    let rooms = chat_sidebar_rooms(listing.clone(), peers.clone(), a.clone(), Vec::new());
    assert_eq!(rooms.len(), 1);
    assert_eq!(rooms[0].channel.id, "general");
    assert_eq!(
        chat_sidebar_rooms(listing, peers, String::new(), Vec::new()).len(),
        2
    );

    // the id the app mints is the id chat will accept from a USER author:
    // ':' is reserved for module origins and '/' is refused outright, so a
    // minted id carrying either is a DM that can never be created.
    // `chat::client`'s own test runs the id through that rule directly.
    let id = dm_channel_id(a, b);
    assert!(
        !id.contains(':'),
        "a user-authored channel id may not carry ':'"
    );
    assert!(!id.contains('/'), "a channel id may not carry '/'");
    assert!(id.starts_with("dm-") && id.len() == 67);
}

#[test]
fn the_post_gate_names_why_a_viewer_cannot_post() {
    let members = vec![ChatMember {
        key: "beef".into(),
        label: "b".into(),
    }];
    assert_eq!(post_gate(false, false, Vec::new(), "cafe".into()), "");
    assert_eq!(
        post_gate(true, false, members.clone(), "beef".into()),
        "channel_archived"
    );
    assert_eq!(
        post_gate(false, true, members.clone(), "cafe".into()),
        "members_only"
    );
    assert_eq!(post_gate(false, true, members, "beef".into()), "");
}

/// The three folds the mounted surfaces are drawn from — the crumb bar's
/// counts, the blob gutter, and the roster the popped panel keeps.
#[test]
fn the_crumb_counts_split_the_listing_in_two() {
    let entries = ["dir", "file", "file"]
        .into_iter()
        .enumerate()
        .map(|(key, kind)| FsEntry {
            key: key as i64,
            path: format!("/shared/{kind}"),
            name: kind.into(),
            kind: kind.into(),
            size: 0,
            object: String::new(),
        })
        .collect::<Vec<_>>();
    assert_eq!(fs_dir_count(entries.clone()), 1);
    assert_eq!(fs_file_count(entries.clone()), 2);
    assert_eq!(
        fs_dir_count(entries.clone()) + fs_file_count(entries),
        3,
        "every row lands in exactly one bucket"
    );
    assert_eq!(fs_dir_count(Vec::new()), 0);
    assert_eq!(fs_file_count(Vec::new()), 0);
}

#[test]
fn the_selected_fs_entry_resolves_or_blanks() {
    let selected = FsEntry {
        key: 1,
        path: "/shared/notes".into(),
        name: "notes".into(),
        kind: "file".into(),
        size: 7,
        object: "abc".into(),
    };

    assert_eq!(
        fs_entry_named(vec![no_fs_entry(), selected.clone()], selected.path.clone(),),
        selected
    );
    assert_eq!(
        fs_entry_named(Vec::new(), "/shared/missing".into()),
        no_fs_entry()
    );
}

#[test]
fn directory_rows_are_prepared_from_the_listing() {
    let entry = |name: &str, kind: &str| FsEntry {
        key: 0,
        path: format!("/shared/{name}"),
        name: name.into(),
        kind: kind.into(),
        size: 0,
        object: String::new(),
    };

    assert_eq!(
        fs_directories(vec![entry("docs", "dir"), entry("readme", "file")]),
        vec![entry("docs", "dir")]
    );
}

#[test]
fn source_lines_number_from_one_and_an_empty_blob_has_none() {
    let rows = source_lines("alpha\nbeta\n".into());
    assert_eq!(rows.len(), 2, "a trailing newline is not a third line");
    assert_eq!(rows[0].number, "1");
    assert_eq!(rows[0].text, "alpha");
    assert_eq!(rows[1].number, "2");
    assert_eq!(rows[1].text, "beta");
    assert!(source_lines(String::new()).is_empty());
}

#[test]
fn a_roster_survives_only_while_this_device_is_in_the_huddle() {
    let roster = vec![HuddleParticipant {
        key: "aa".into(),
        label: "aa".into(),
        initials: "A".into(),
        is_agent: false,
        is_you: true,
        joined_at: 0,
        node: "aa11".into(),
    }];
    assert_eq!(keep_roster(true, roster.clone()).len(), 1);
    assert!(
        keep_roster(false, roster).is_empty(),
        "another channel's roster never reaches the panel"
    );
}

#[test]
fn the_roster_answers_admin_tier_and_filters() {
    let rows = vec![
        MemberRow {
            key: "aa".into(),
            label: "aa".into(),
            role: "validator".into(),
            is_this_node: true,
            is_agent: false,
            model: String::new(),
            live: true,
        },
        MemberRow {
            key: "bb".into(),
            label: "bb".into(),
            role: "resident".into(),
            is_this_node: false,
            is_agent: false,
            model: String::new(),
            live: false,
        },
        MemberRow {
            key: "triage".into(),
            label: "triage".into(),
            role: "agent".into(),
            is_this_node: false,
            is_agent: true,
            model: "codex".into(),
            live: true,
        },
    ];
    assert!(members_is_admin(rows.clone()));
    assert_eq!(member_tier(rows.clone()), "validator");
    // the two halves of "no row for this node", kept apart: an unanswered
    // roster is unknown, an answered one without this node is a real guest.
    assert_eq!(member_tier(Vec::new()), "");
    let mut answered_without_this_node = rows.clone();
    answered_without_this_node[0].is_this_node = false;
    assert_eq!(member_tier(answered_without_this_node), "guest");
    assert_eq!(filter_members(rows.clone(), "agents".into()).len(), 1);
    assert_eq!(filter_members(rows.clone(), "humans".into()).len(), 2);
    assert_eq!(filter_members(rows.clone(), "validators".into()).len(), 1);
    assert_eq!(filter_members(rows, "all".into()).len(), 3);
}

/// THE HEADER COUNTS THE LIST IT SITS ABOVE. `members_summary` used to fold the
/// two VALSET queries — validators and residents — while the roster under it
/// also draws every registered agent, which holds no valset standing at all. On
/// the demo workspace that printed `1 validator · 0 residents` over two rows:
/// both numbers true, the sentence not, because it measured a different set
/// than the one on screen. The subtitle now splits the rows on `is_agent`, the
/// same predicate the Humans / Agents chips use, so its two counts partition
/// the list and sum to the All chip.
#[test]
fn the_members_subtitle_folds_the_rows_the_screen_lists() {
    let member = |key: &str, role: &str| MemberRow {
        key: key.into(),
        label: key.into(),
        is_agent: role == "agent",
        role: role.into(),
        is_this_node: false,
        model: String::new(),
        live: true,
    };
    let rows = vec![
        member("aa", "validator"),
        member("bb", "resident"),
        member("triage", "agent"),
    ];
    assert_eq!(members_summary(true, rows.clone()), "2 humans · 1 agent");
    // singulars, and the count that used to be the whole subtitle.
    assert_eq!(
        members_summary(true, rows[..1].to_vec()),
        "1 human · 0 agents"
    );

    // The invariant under the wording: every number in the subtitle is a slice
    // of the list, so they add up to the row count. The valset fold never did.
    let counted: usize = members_summary(true, rows.clone())
        .split(" · ")
        .filter_map(|part| part.split(' ').next()?.parse::<usize>().ok())
        .sum();
    assert_eq!(
        counted,
        rows.len(),
        "the Members subtitle must sum to the roster printed under it"
    );
}

#[test]
fn the_tracker_splits_into_open_prs_and_open_issues() {
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
    assert_eq!(filter_forge_items(items.clone(), "pr".into()).len(), 2);
    assert_eq!(forge_open_count(items.clone(), "pr".into()), 1);
    assert_eq!(forge_open_count(items, "issue".into()), 1);
}

#[test]
fn machine_values_read_as_a_person_reads_them() {
    assert_eq!(size_label(421_888), "412 KB");
    assert_eq!(size_label(900), "900 B");
    assert_eq!(size_label(3 * 1024 * 1024), "3.0 MB");
    assert_eq!(mmss(0), "00:00");
    assert_eq!(mmss(4 * 60 + 7), "04:07");
    assert_eq!(initials_of("Kestrel Song"), "KS");
    assert_eq!(initials_of("triage"), "TR");
    assert_eq!(initials_of(""), "?");
    assert_eq!(network_slug("Acme Research!".into()), "acme-research");
    assert_eq!(height_label(84_912), "h 84,912");
    assert_eq!(height_label_short(84_912), "h 84,912");
    assert_eq!(height_label(-1), "h —");
    assert_eq!(optional_number(Some(4)), "4");
    // absent, not zero: a resident's status carries no consensus section.
    assert_eq!(optional_number(None), "—");
}

/// An `operations` reading the node never published prints `—`, not a
/// measured value.
///
/// `operations` is absent on a resident, a joiner and the embedded local
/// daemon — which is exactly why the consensus trio beside these two is
/// `Option`. `last_finalized_at` and `checkpoint_height` are plain `i64`
/// because `0` is a legal height and a legal timestamp, so they carry
/// `UNMEASURED` instead and both renderers turn it into an em dash.
#[test]
fn an_unpublished_operations_reading_renders_as_unknown() {
    assert_eq!(height_label(UNMEASURED), "h —");
    assert_eq!(height_label_short(UNMEASURED), "h —");
    assert_eq!(relative_time(UNMEASURED, 1), "—");

    // and a real reading of zero is still a real reading: height 0 is the
    // genesis block, not an absence.
    assert_eq!(height_label(0), "h 0");
    // a record with no stamp keeps printing nothing — an em dash on every
    // unstamped row would be noise, and that is a different fact.
    assert_eq!(relative_time(0, 1), "");
}

/// THE OTHER HALF OF THE SAME FACT: the reading has to ARRIVE as `UNMEASURED`.
///
/// The test above pins the renderers. This pins the producer, on the one path
/// where a real node publishes nothing: a resident, a joiner or the embedded
/// local daemon omits `operations` entirely (`skip_serializing_if` on the node
/// side), so the `unwrap_or` arms are what run. Every fixture in this file
/// supplies an `operations` object, so those arms had no coverage at all —
/// `.unwrap_or(UNMEASURED)` could be changed to `.unwrap_or(0)` with the whole
/// suite green, which is exactly the defect on exactly the lane this file
/// spends twenty lines forbidding.
#[test]
fn a_status_without_operations_produces_unmeasured_readings() {
    let resident = serde_json::json!({
        "version": "0.1.0",
        "root_hash": "abc",
        "height": 0,
    });
    let facts = node_facts(&resident, 7);

    assert_eq!(facts.generation, 7);
    assert_eq!(
        facts.last_finalized_at, UNMEASURED,
        "an omitted `operations` must not read as a finalization at epoch zero"
    );
    assert_eq!(
        facts.checkpoint_height, UNMEASURED,
        "nor as a checkpoint at genesis"
    );
    assert_eq!(
        facts.height, UNMEASURED,
        "and a wire `0` head is the node's own no-boundary-served sentinel"
    );
    // the trio is `Option` for the same reason and stays absent.
    assert_eq!(facts.view, None);
    assert_eq!(facts.quorum, None);
    assert_eq!(facts.reachable_validators, None);
}

/// SYNC IS READ OFF `phase`, NEVER OFF THE PRESENCE OF `sync`.
///
/// `operations.sync` is set by `begin_sync` and never cleared — no writer in
/// `bin/noded/src/metrics.rs` puts it back to `None`. A node that finished
/// syncing hours ago still carries the last run's heights, so reading presence
/// as "is syncing" paints a progress bar that never goes away.
#[test]
fn a_finished_sync_is_not_a_sync_in_progress() {
    let caught_up = serde_json::json!({
        "operations": {
            "phase": "serving",
            "phase_since": 1_700_000_000,
            "sync": { "target_height": 900, "applied_height": 900, "retries": 3, "failures": 1 },
        },
    });
    let facts = node_facts(&caught_up, 0);
    assert_eq!(facts.phase, "serving");
    assert!(
        !sync_in_progress(&facts.phase),
        "a stale sync block must not read as a sync in progress"
    );
    // the numbers still ride: Settings prints them as CUMULATIVE totals.
    assert_eq!(facts.sync_retries, 3);
    assert_eq!(facts.sync_failures, 1);
    assert_eq!(facts.phase_since, 1_700_000_000);

    let catching_up = serde_json::json!({
        "operations": {
            "phase": "syncing",
            "sync": { "target_height": 900, "applied_height": 412 },
        },
    });
    let facts = node_facts(&catching_up, 0);
    assert!(sync_in_progress(&facts.phase));
    assert_eq!(facts.sync_applied, 412);
    assert_eq!(facts.sync_target, 900);

    // A node that has never synced publishes no `sync` at all. Heights are
    // UNMEASURED because the node published none; the counters are genuinely
    // zero, because a count of nothing IS zero.
    let fresh = serde_json::json!({ "operations": { "phase": "validating" } });
    let facts = node_facts(&fresh, 0);
    assert_eq!(facts.sync_target, UNMEASURED);
    assert_eq!(facts.sync_applied, UNMEASURED);
    assert_eq!(facts.sync_retries, 0);
    assert_eq!(facts.sync_last_error, "");
    assert_eq!(facts.phase_since, UNMEASURED);
}

/// ONE STRING FOR ALL THREE SURFACES, so the titlebar, the explorer header and
/// Settings cannot disagree about what the node is doing.
#[test]
fn the_sync_label_shows_progress_only_while_catching_up() {
    assert_eq!(sync_label("syncing".into(), 412, 900), "Syncing 412 / 900");

    // CAUGHT UP: the phase alone, however live the numbers beside it look.
    // `operations.sync` is never cleared, so a finished run's heights sit in
    // the document forever and must not reach a reader.
    assert_eq!(sync_label("serving".into(), 900, 900), "Serving");
    assert_eq!(sync_label("validating".into(), 900, 900), "Validating");

    // syncing with nothing published yet is still honest about the phase.
    assert_eq!(
        sync_label("syncing".into(), UNMEASURED, UNMEASURED),
        "Syncing"
    );
    assert_eq!(sync_label("syncing".into(), 412, UNMEASURED), "Syncing");

    // and a node that published no phase says NOTHING rather than guessing.
    assert_eq!(sync_label(String::new(), 412, 900), "");
}

/// A record stamp is a BLOCK HEIGHT on this chain, so every record-time
/// string counts blocks. Only `/v1/status` supplies unix seconds.
#[test]
fn record_stamps_count_blocks_and_status_stamps_count_seconds() {
    let now = now_seconds();
    assert_eq!(height_ago(84_500, 84_912, now), "412 blocks ago");
    assert_eq!(height_ago(84_911, 84_912, now), "1 block ago");
    assert_eq!(height_ago(84_912, 84_912, now), "this block");
    // a follower behind the record it is rendering still reads as now.
    assert_eq!(height_ago(84_913, 84_912, now), "this block");
    assert_eq!(height_ago(0, 84_912, now), "");
    assert_eq!(
        expires_in_blocks(85_324, 84_912, now),
        "expires in 412 blocks"
    );
    assert_eq!(expires_in_blocks(84_913, 84_912, now), "expires in 1 block");
    assert_eq!(expires_in_blocks(84_912, 84_912, now), "expired");
    assert_eq!(relative_time(now - 30, now), "just now");
    assert_eq!(relative_time(now - 40 * 60, now), "40m ago");
    assert_eq!(relative_time(now - 2 * 60 * 60, now), "2h ago");
    assert_eq!(relative_time(0, now), "");
}

/// The OTHER lane: a single-writer noded stamps `consensus_time` in unix
/// MILLIS, so renderers for consensus stamps use the shared wall reading.
#[test]
fn a_unix_millis_consensus_stamp_uses_the_wall_clock() {
    let now = now_seconds();
    let two_hours_ago = (now - 2 * 60 * 60) * 1_000;
    assert_eq!(height_ago(two_hours_ago, 84_912, now), "2h ago");
    assert_eq!(
        expires_in_blocks((now + 3 * 60 * 60) * 1_000, 84_912, now),
        "expires in 3h"
    );
    assert_eq!(
        expires_in_blocks((now - 60) * 1_000, 84_912, now),
        "expired"
    );
}

#[test]
fn a_proposal_renders_its_payload_and_its_frozen_bar() {
    let view = serde_json::json!({
        "action": { "add_validator": { "key": [0x8c, 0x4f, 0xa2, 0x11] } },
        "voting_rule": { "threshold": { "required_yes": 4 } }
    });
    assert_eq!(gov_action_detail(&view["action"]), "key 8c4fa211");
    // a threshold's bar does not move with the no votes.
    assert_eq!(yes_needed(&view["voting_rule"], 0), 4);
    assert_eq!(yes_needed(&view["voting_rule"], 2), 4);

    // a participating majority's quorum is TURNOUT, and passing also needs
    // yes > no — reading `quorum` straight into a yes counter says "quorum
    // met" at 3/3 on a vote of 3 yes / 3 no, which does not settle.
    let majority = serde_json::json!({ "participating_majority": { "quorum": 6 } });
    assert_eq!(yes_needed(&majority, 0), 6);
    assert_eq!(
        yes_needed(&majority, 2),
        4,
        "two no votes count toward turnout"
    );
    assert_eq!(yes_needed(&majority, 3), 4, "…but yes must still exceed no");
    assert_eq!(
        tally_note(3, yes_needed(&majority, 3)),
        "3 approvals · 1 more for quorum"
    );

    assert_eq!(tagged_name(&view["action"]), "add_validator");
    assert_eq!(proposal_kind_tone("add_validator".into()), "access");
    assert_eq!(proposal_kind_tone("signal".into()), "neutral");
    assert_eq!(
        gov_action_detail(&serde_json::json!({ "signal": { "text": "ship it" } })),
        "ship it"
    );
}

#[test]
fn the_huddle_roster_marks_the_row_this_device_holds() {
    // The wire truth: `HuddleEntry.user` is the kernel's BARE user id, never
    // `user:{hex}` — the previous fixture invented prefixed entries and
    // asserted a compare no real roster row could satisfy.
    let me = [0xaau8; 32];
    let peer = [0xbbu8; 32];
    let roster = huddle_roster(
        &[
            chat::index::HuddleEntry {
                user: hex_encode(&me),
                node: String::new(),
                joined_at: 10,
            },
            chat::index::HuddleEntry {
                user: hex_encode(&peer),
                node: String::new(),
                joined_at: 20,
            },
        ],
        Some(&me),
    );
    assert_eq!(roster.len(), 2);
    assert!(roster[0].is_you && !roster[0].is_agent);
    assert!(!roster[1].is_you && !roster[1].is_agent);
    assert!(huddle_self(roster.clone()));
    assert!(!huddle_self(vec![roster[1].clone()]));
}

#[test]
fn container_depth_uses_only_shared_design_roles() {
    let tokens = ui_lang_components::ui::theme::LIGHT;
    let theme = iced::Theme::Light;
    let card = card_style(&theme);
    let raised = raised_style(&theme);

    assert_eq!(
        card.background,
        Some(iced::Background::Color(tokens.palette.card))
    );
    assert_eq!(card.border.radius, tokens.radius.card.into());
    assert_eq!(card.shadow, iced::Shadow::default());
    // OPAQUE. iced has no backdrop blur, so a glass role over a menu is just
    // transparency: the sentence behind an item and the item's own label draw
    // through each other.
    assert_eq!(
        raised.background,
        Some(iced::Background::Color(tokens.palette.popover))
    );
    assert_eq!(raised.background.map(alpha_of), Some(1.0));
    assert_eq!(
        raised_style(&iced::Theme::Dark).background.map(alpha_of),
        Some(1.0)
    );
    assert_eq!(raised.border.radius, tokens.radius.card.into());
    assert_eq!(raised.shadow, tokens.elevation.popover);
}

fn alpha_of(background: iced::Background) -> f32 {
    let iced::Background::Color(color) = background else {
        panic!("a depth role paints a flat colour");
    };
    color.a
}

#[test]
fn palette_keys_use_logical_escape_and_physical_shortcut() {
    use iced::keyboard::{
        Key, Modifiers,
        key::{Code, Named, Physical},
    };

    assert_eq!(
        palette_key_action(
            Key::Named(Named::Escape),
            Physical::Code(Code::KeyA),
            Modifiers::default(),
            true,
        ),
        "close"
    );
    assert_eq!(
        palette_key_action(
            Key::Named(Named::Escape),
            Physical::Code(Code::KeyA),
            Modifiers::default(),
            false,
        ),
        "none"
    );
    assert_eq!(
        palette_key_action(
            Key::Character("x".into()),
            Physical::Code(Code::KeyK),
            Modifiers::COMMAND,
            false,
        ),
        "open"
    );
    assert_eq!(
        palette_key_action(
            Key::Character("x".into()),
            Physical::Code(Code::KeyK),
            Modifiers::COMMAND,
            true,
        ),
        "close"
    );
}

/// One stubbed lane of the workspace search: the substring that names it in a
/// raw request, the search LEG it belongs to, and the reply. An empty leg name
/// marks a FOLLOW-UP — a request a leg can only issue after its own first reply
/// (forge's per-repo tracker read, pages' title lookup), so it can never be in
/// flight at first contact and is not counted as overlap.
///
/// FIRST MATCH WINS, SO A FOLLOW-UP GOES ABOVE THE LANE IT SHARES A ROUTE WITH.
/// `list_pages` is `/v1/index/pages/view` too — the same route as the page
/// search — so on the substring alone the title lookup would be served the
/// search's own reply and read as a second `pages` arrival. Matching the query
/// discriminant first is what keeps "a request this stub does not model panics
/// by name" true rather than nearly true.
///
/// The chat lane answers with no hits on purpose: a `MsgRow` is seventeen wire
/// fields and nothing here turns on its contents. The other five carry one
/// matching row each, which is what pins the row order.
const SEARCH_LANES: &[(&str, &str, &str)] = &[
    ("/v1/index/chat/view", "chat", r#"{"hits":[]}"#),
    (
        "list_pages",
        "",
        r#"{"pages":{"pages":[{"id":"page-1","title":"The needle page"}],"has_more":false}}"#,
    ),
    (
        "/v1/index/pages/view",
        "pages",
        r#"{"hits":[{"block_id":"block-1","page_id":"page-1","parent":"page-1","kind":"paragraph","text":"needle in a page","height":1,"time":1}]}"#,
    ),
    (
        "list_repos",
        "forge",
        r#"{"repos":[{"name":"needle-repo","head":"0000000000000000000000000000000000000000"}]}"#,
    ),
    (
        "list_items",
        "",
        r#"{"items":[{"number":7,"kind":"issue","title":"needle issue","state":"open","author":"system","created_at":1,"updated_at":1}]}"#,
    ),
    (
        "/v1/files/grep",
        "files",
        r#"{"hits":[{"path":"src/needle.rs","line":3,"text":"a needle here"}]}"#,
    ),
    (
        "/v1/index/tasks/view",
        "tasks",
        r#"{"tasks":{"tasks":[{"title":"needle task","task_id":"task-1","created_by":"user:aa","updated_height":2}]}}"#,
    ),
    (
        "pending_runs",
        "runs",
        r#"{"pending_runs":[{"run_id":"needle-run","agent_id":"agent-1","created_at":1,"channel_id":"c1"}]}"#,
    ),
    ("recent_runs", "runs", r#"{"recent_runs":[]}"#),
];

/// The lane one raw HTTP request belongs to. `None` = a request this stub does
/// not model, which is a test bug, not a product one.
fn search_lane_of(request: &str) -> Option<&'static (&'static str, &'static str, &'static str)> {
    SEARCH_LANES
        .iter()
        .find(|(mark, ..)| request.contains(mark))
}

/// Which REQUESTS of a workspace search were in flight AT THE SAME MOMENT, each
/// tagged with the leg it belongs to, as seen by the stub node below.
///
/// Requests, not legs — a set of leg names cannot see a leg serializing its own
/// round trips (tasks reads three status pages, runs reads two queries), and
/// "the six legs overlapped" stays true while the work inside them is a chain.
/// Counting arrivals makes the multiplicity part of the answer.
#[derive(Default)]
struct FanOutWatch {
    waiting: Vec<String>,
    /// The arrivals as they stood when the stub let them through — the answer
    /// the test asserts on. Empty until then.
    overlapped: Vec<String>,
    released: bool,
}

impl FanOutWatch {
    /// Record one arrival; true when it is the one that completes the wave. An
    /// arrival AFTER release is not recorded: it is by definition a request that
    /// was waiting on something, which is the failure being measured.
    fn arrive(&mut self, leg: &str, requests: usize) -> bool {
        if self.released {
            return false;
        }
        self.waiting.push(leg.to_string());
        self.waiting.len() >= requests
    }

    /// Let everyone through and freeze the report.
    fn release(&mut self) {
        if self.released {
            return;
        }
        self.released = true;
        self.overlapped = self.waiting.clone();
        self.overlapped.sort();
    }
}

/// A stub node that ANSWERS NOTHING until a workspace search has `requests`
/// round trips in flight at once, then releases them together. That overlap is
/// the only thing observable from outside the process, and it is the whole
/// guarantee: a request that waits on another request's reply — a serial chain,
/// a nested `.await` inside the join, a helper that folds two legs into one
/// future, a leg walking its own pages one at a time — cannot be in flight
/// beside what it waits on, so the wave never completes and the stub reports
/// exactly what did overlap.
///
/// [`FanOutWatch::overlapped`] is filled ONCE, at release, and requests that
/// arrive after it are not counted — the report is what overlapped, not what
/// ever arrived. Recording every arrival instead makes this stub pass the very
/// break it exists to catch: a leg held back behind another leg's reply lands
/// the moment the rest are let go, and a set that keeps filling then reads as a
/// full fan-out.
///
/// A grep of the join's TEXT cannot see any of this, which is why the pin this
/// replaced could be broken while staying green.
///
/// THE ESCAPE FROM A WEDGE IS AN EVENT, NOT A CLOCK. A serialized search never
/// completes the wave, so its held requests run out `RpcClient`'s own 30 s
/// ceiling and reqwest drops the connection — and that FIN is what this stub
/// waits on beside the release. The first hang-up freezes the report at what
/// had genuinely overlapped and lets the rest go, so a broken fan-out FAILS
/// with the truth in the message instead of hanging the suite. The passing path
/// never touches either seam: nine loopback requests overlap in milliseconds
/// and release on the ninth ARRIVAL, so no duration is load-bearing anywhere
/// here.
async fn node_that_answers_only_a_full_fan_out(
    requests: usize,
    refused: &'static [&'static str],
    watch: std::sync::Arc<Mutex<FanOutWatch>>,
) -> String {
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

    /// The request is in hand once the body reaches its declared length.
    fn request_is_complete(request: &[u8]) -> bool {
        let text = String::from_utf8_lossy(request);
        let Some((head, body)) = text.split_once("\r\n\r\n") else {
            return false;
        };
        let declared = head
            .to_lowercase()
            .lines()
            .find_map(|line| {
                line.strip_prefix("content-length:")?
                    .trim()
                    .parse::<usize>()
                    .ok()
            })
            .unwrap_or(0);
        body.len() >= declared
    }

    /// The held request's client gave up and closed the socket — the only
    /// event a stub holding a reply can observe when the wave will never
    /// complete. `Ok(0)` is the FIN; an error is the same fact, harder.
    async fn hung_up(stream: &mut tokio::net::TcpStream) {
        let mut ignored = [0u8; 1];
        while let Ok(read) = stream.read(&mut ignored).await {
            if read == 0 {
                return;
            }
        }
    }

    let (release, _) = tokio::sync::watch::channel(false);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind the stub node");
    let origin = format!("http://{}", listener.local_addr().expect("stub address"));
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let watch = watch.clone();
            let release = release.clone();
            tokio::spawn(async move {
                let mut request = Vec::new();
                let mut chunk = [0u8; 2048];
                while let Ok(read) = stream.read(&mut chunk).await {
                    if read == 0 {
                        break;
                    }
                    request.extend_from_slice(&chunk[..read]);
                    if request_is_complete(&request) {
                        break;
                    }
                }
                let request = String::from_utf8_lossy(&request).to_string();
                let (_, leg, reply) = search_lane_of(&request).unwrap_or_else(|| {
                    panic!(
                        "the workspace search touched a lane this stub does not \
                         model, so it costs a round trip nobody accounted for: \
                         {request}"
                    )
                });
                let counts = !leg.is_empty();
                if counts {
                    let completes_the_wave =
                        watch.lock().expect("stub watch").arrive(leg, requests);
                    if completes_the_wave {
                        watch.lock().expect("stub watch").release();
                        let _ = release.send(true);
                    }
                    let mut open = release.subscribe();
                    let opened = async {
                        while !*open.borrow_and_update() {
                            let _ = open.changed().await;
                        }
                    };
                    tokio::select! {
                        () = opened => {}
                        () = hung_up(&mut stream) => {
                            // Nobody is coming: this request's client already
                            // walked away. Freeze the report at what did
                            // overlap and let the others answer into the void.
                            watch.lock().expect("stub watch").release();
                            let _ = release.send(true);
                        }
                    }
                }
                let refuse = refused.contains(leg);
                let (status, reply) = match refuse {
                    true => ("503 Service Unavailable", "the module is not answering"),
                    false => ("200 OK", *reply),
                };
                let response = format!(
                    "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
                    reply.len()
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.shutdown().await;
            });
        }
    });
    origin
}

/// A SEARCH COSTS ITS SLOWEST SOURCE, NOT THEIR SUM. `search_workspace` awaited
/// six independent sources one after another, and nothing in it reads what
/// another leg produced. Warm that is worth nothing — every leg answers in a
/// few milliseconds. COLD it is the whole cost: a module's first touch measured
/// 10-54 s against this app's 30 s client ceiling, so serial is several
/// ceilings end to end and fanned out is one.
///
/// THE OVERLAP IS THE GUARANTEE, SO THE OVERLAP IS WHAT IS PINNED — observed
/// from outside the process by a node that answers nothing until all six legs
/// are in flight together. The pin this replaced greped the join's text for six
/// names, which folding two legs into one `async { a.await; b.await }` inside
/// the join defeats while staying green.
///
/// The row order is asserted in the same run: a fan-out that silently reordered
/// the results would be a different defect.
///
/// This does NOT contradict the `join_all` ban in backend/document.rs: that one
/// guards the WRITE chain, where an op built on the block before it must land
/// after it.
#[tokio::test(flavor = "current_thread")]
async fn a_workspace_search_reaches_its_six_sources_together() {
    let watch: std::sync::Arc<Mutex<FanOutWatch>> = Default::default();
    let rpc = node_that_answers_only_a_full_fan_out(9, &[], watch.clone()).await;

    let results = search_workspace(rpc, "needle".into(), 4)
        .await
        .expect("the stub answers every lane");

    assert_eq!(
        watch.lock().expect("stub watch").overlapped,
        [
            "chat", "files", "forge", "pages", "runs", "runs", "tasks", "tasks", "tasks"
        ],
        "every round trip a workspace search opens with must be in flight at \
         once — anything missing here waited on another request's reply. The \
         repeats are the legs that read more than one thing: tasks walks three \
         status pages, runs reads pending and recent."
    );
    // Every lane answered, so nothing is held back.
    assert_eq!(results.partial, "");
    // And the rows land in the order the screen shows them. The tasks lane is
    // three status pages behind one source, hence the run-length squash.
    let mut order: Vec<String> = results.hits.iter().map(|hit| hit.kind.clone()).collect();
    order.dedup();
    assert_eq!(order, ["page", "code", "file", "task", "run"]);
    // The page row heads with its PAGE, which is the second wave's whole job —
    // and the reason `list_pages` is a lane of its own rather than a substring
    // collision with the search it follows. Served the search's reply instead,
    // the title lookup fails and every page hit falls back to "Untitled".
    let page = results
        .hits
        .iter()
        .find(|hit| hit.kind == "page")
        .expect("the pages lane answered");
    assert_eq!(page.title, "The needle page");
}

/// A SOURCE THAT DID NOT ANSWER IS NOT A SOURCE WITH NOTHING TO SAY. All six
/// legs failed silently — `if let Ok(..)` on two, `return Vec::new()` on the
/// rest — and the node's per-module cold start runs tens of seconds against a
/// 30 s client ceiling, so a timeout was the ordinary case, not the exotic one.
/// A search that reached the node and lost three of its six sources still
/// rendered a confident count, a full chip strip reading 0 for kinds it never
/// read, and — when the survivors were empty — "Nothing matched that query in
/// this workspace". Three lies off one timeout, in the app that spent the night
/// learning to say nothing rather than something false.
///
/// EVERY LEG, NOT THE ONE I FIXED FIRST. The round-2 version refused the forge
/// lane alone, and reverting the silence report on chat, files or tasks — two
/// of them the `return Vec::new()` swallowers — kept the suite green. The
/// defect was class-wide, so the pin walks the class: each source in turn is
/// the one that does not answer.
#[tokio::test(flavor = "current_thread")]
async fn a_search_that_lost_a_source_says_which_one() {
    /// The six sources, each with the name the screen must call it by and the
    /// hit kind it contributes. One table: a seventh source added to
    /// `search_workspace` with no silence report has to be added here to pass,
    /// and then fails.
    const SOURCES: [(&str, &str, &str); 6] = [
        ("chat", "Messages", "message"),
        ("pages", "Pages", "page"),
        ("forge", "Code", "code"),
        ("files", "Files", "file"),
        ("tasks", "Tasks", "task"),
        ("runs", "Runs", "run"),
    ];

    for (leg, label, silent_kind) in SOURCES {
        let leg_alone: &'static [&'static str] = match leg {
            "chat" => &["chat"],
            "pages" => &["pages"],
            "forge" => &["forge"],
            "files" => &["files"],
            "tasks" => &["tasks"],
            _ => &["runs"],
        };
        let rpc = node_that_answers_only_a_full_fan_out(9, leg_alone, Default::default()).await;

        let results = search_workspace(rpc, "needle".into(), 5)
            .await
            .expect("the other five sources answered");

        assert_eq!(
            results.partial,
            format!("{label} did not answer — these results are incomplete."),
            "the screen must name the source it did not read"
        );
        // The chip strip's contract is "a count of 0 means nothing matched,
        // never no loader", so the source that never ran keeps no chip at all.
        let chips: Vec<&str> = results
            .kinds
            .iter()
            .map(|kind| kind.kind.as_str())
            .collect();
        let answered: Vec<&str> = SOURCES
            .iter()
            .map(|(_, _, kind)| *kind)
            .filter(|kind| *kind != silent_kind)
            .collect();
        assert_eq!(chips, answered, "{label} was refused, so it keeps no chip");
        // And the answer that did arrive is untouched, in screen order —
        // degrading the survivors would be the opposite mistake. The chat lane
        // carries no rows on purpose (see `SEARCH_LANES`); every other source
        // contributes exactly one.
        let mut rows: Vec<&str> = results.hits.iter().map(|hit| hit.kind.as_str()).collect();
        rows.dedup();
        let carried: Vec<&str> = ["page", "code", "file", "task", "run"]
            .into_iter()
            .filter(|kind| *kind != silent_kind)
            .collect();
        assert_eq!(
            rows, carried,
            "with {label} silent the other sources still land, in screen order"
        );
    }

    // CARDINALITY. Every case above refuses exactly ONE source, and a filter
    // keyed on `silent.first()` instead of `silent.contains(..)` passes all six
    // — the reviewer changed that one token and the suite stayed green while a
    // second silent source got a chip reading 0, against the strip's own "a
    // count of 0 means nothing matched, never no loader". Two at once is the
    // case the PR body's own headline scenario describes.
    let rpc =
        node_that_answers_only_a_full_fan_out(9, &["chat", "pages"], Default::default()).await;
    let results = search_workspace(rpc, "needle".into(), 5)
        .await
        .expect("the other four sources answered");
    assert_eq!(
        results.partial, "Messages, Pages did not answer — these results are incomplete.",
        "both silent sources are named, in screen order"
    );
    let chips: Vec<&str> = results
        .kinds
        .iter()
        .map(|kind| kind.kind.as_str())
        .collect();
    assert_eq!(
        chips,
        ["code", "file", "task", "run"],
        "NEITHER refused source keeps a chip — not just the first one"
    );
}

/// A SEARCH HIT SAYS WHICH ROOM IT IS IN, ONCE. The hit's `meta` was
/// `#{seq}` — the message's sequence number, rendered exactly like a channel,
/// because every channel in this app is written `# General`. So a palette row
/// read `#1` and the reader could not tell whether that was a room, a position,
/// or which of four channels the message actually lived in.
///
/// Three surfaces render `hit.meta` — the palette, the chat sidebar and the
/// Explorer — and only the Explorer composed the channel in, at its own call
/// site. The room now lives in `meta` itself, so all three agree and the
/// Explorer stops composing (which would have printed it twice).
#[test]
fn a_search_hit_names_its_room_exactly_once() {
    const CHAT: &str = include_str!("chat.rs");
    let hit = CHAT
        .split("ChatSearchHit {")
        .nth(1)
        .expect("the search hit mapping")
        .split("})")
        .next()
        .expect("mapping body");
    assert!(
        hit.contains(r#"meta: format!("{} · #{}", hit.channel_id, hit.seq)"#),
        "the room comes first, then the sequence"
    );

    const SEARCH: &str = include_str!("search.rs");
    let message_arm = SEARCH
        .split("kind: \"message\".into(),")
        .nth(1)
        .expect("the message hit arm")
        .split("});")
        .next()
        .expect("arm body");
    assert!(
        message_arm.contains("meta: hit.meta,"),
        "the Explorer carries the meta through"
    );
    assert!(
        !message_arm.contains("hit.channel_id, hit.meta"),
        "composing the channel again is what printed it twice"
    );
}

/// AN UNREAD HEIGHT SAYS SO. The Node overview must not print `h 0` before a
/// status document lands — a measured zero for a chain sitting at ~398,000.
///
/// `height_label` already had the vocabulary: a negative height is `h —`. The
/// field simply defaulted to 0, which is a reading rather than the absence of
/// one, so the state default is now the sentinel the label understands.
#[test]
fn an_unread_block_height_is_not_reported_as_zero() {
    assert_eq!(height_label(-1), "h —", "the no-reading sentinel");
    assert_ne!(
        height_label(0),
        "h —",
        "zero is a real height and must keep reading as one"
    );

    // The state default is what Node shows before any node fact lands.
    // This is the RENDERER's contract and it is unchanged: `0` still reads as a
    // real height here. What changed is upstream — `served_height` decides that
    // a `0` on the wire was never a measurement, so no zero reaches this label
    // as a head. See `a_resyncing_replica_has_no_head_to_print_a_checkpoint_against`.
    const STATE: &str = include_str!("../ui/state.ice");
    assert!(
        STATE.contains("node_height:i64 = -1"),
        "an unread height must default to the sentinel, not to a measured zero"
    );
}

/// A node that serves `GET /v1/status` EXACTLY ONCE and answers `500` to every
/// later ask for it. `/v1/peers` answers every time — the pin is on the status
/// document, not on the peer sample.
///
/// This is the whole point of the fixture: a loader that reads the chain twice
/// to fill one card cannot get away with it here, whichever field it takes from
/// whichever read. Counting reads is the only pin that survives a rename —
/// #1017's first round asserted identifier names instead, and a reviewer put
/// the literal second `client.status()` back with every name intact and all
/// 272 tests still green.
async fn node_that_serves_its_status_once(status_body: &'static str) -> String {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind the stub node");
    let origin = format!("http://{}", listener.local_addr().expect("stub address"));
    let status_reads = std::sync::Arc::new(AtomicUsize::new(0));
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let mut request = Vec::new();
            let mut chunk = [0u8; 2048];
            // Both routes are bodyless GETs, so the request is in hand as soon
            // as the head is.
            while let Ok(read) = stream.read(&mut chunk).await {
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..read]);
                if request.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            let asked_for_status = String::from_utf8_lossy(&request).contains("/v1/status");
            let already_served =
                asked_for_status && status_reads.fetch_add(1, Ordering::SeqCst) > 0;
            let (code, body) = match (asked_for_status, already_served) {
                (true, false) => ("200 OK", status_body),
                (true, true) => (
                    "500 Internal Server Error",
                    "this node answers /v1/status once",
                ),
                (false, _) => ("200 OK", r#"{"peers":[]}"#),
            };
            let response = format!(
                "HTTP/1.1 {code}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.shutdown().await;
        }
    });
    origin
}

/// ONE CARD, ONE SAMPLE — AND THE SAMPLE IS THE WHOLE PAIR.
///
/// A checkpoint carries no meaning alone; it only ever says how far the durable
/// snapshot trails the head. So the head printed beside it has to come from the
/// same read, or the pair can render an order no node is ever in. The app did
/// exactly that: `HEIGHT h 422,553` under `CHECKPOINT h 422,563`, from a live
/// register and a facts document sampled seconds apart on a chain moving
/// several blocks a second.
///
/// The stub serves `/v1/status` once and refuses the rest, so this fails on a
/// second read no matter what the fields are called.
#[tokio::test(flavor = "current_thread")]
async fn the_node_tile_prints_a_head_and_a_checkpoint_from_one_status_read() {
    // Shape copied from the live demo node, whose checkpoint trails its head by
    // 118 blocks — the direction a healthy node is always in.
    const SERVING: &str = r#"{"version":"0.1.0","root_hash":"20c53b","height":426099,"modules":[],"public_key":"ab","operations":{"last_finalized_at":426099,"storage":{"checkpoint_height":425981}}}"#;
    let rpc = node_that_serves_its_status_once(SERVING).await;

    let facts = load_node_facts(rpc, 9)
        .await
        .expect("one read fills the whole card; a second one is the defect");

    assert_eq!(facts.height, 426_099);
    assert_eq!(facts.checkpoint_height, 425_981);
    assert_eq!(facts.public_key, "ab");
    assert!(
        facts.checkpoint_height <= facts.height,
        "a durable snapshot cannot be ahead of the head it was taken from"
    );
}

/// A NODE SERVING NO BOUNDARY HAS NO HEAD — AND ITS CHECKPOINT KEEPS CLIMBING.
///
/// One read is necessary and not sufficient: the replica lane publishes a
/// document that is itself two instants. `publish_replica_status` fills `height`
/// from `serving`, which is `None` — and so `0` — through a range-pruned
/// backfill, an unresolvable pruned view and an epoch cutover, while it hands
/// the same call the LIVE `replica_prev_ckpt`, which only ever climbs. Read as
/// a measurement, that one honest document renders `HEIGHT h 0` above
/// `CHECKPOINT h 425,981`: a measured zero AND a starker inversion than the
/// ten-block skew this change set out to remove.
///
/// `0` is the node's own word for "no boundary served" — `NodeStatus::default`,
/// the validator's `unwrap_or(0)` and the replica's `None` arm all write it —
/// so the app reads it as absence, and the pair prints `h —` over the
/// checkpoint that is genuinely on disk.
#[tokio::test(flavor = "current_thread")]
async fn a_resyncing_replica_has_no_head_to_print_a_checkpoint_against() {
    const RESYNCING: &str = r#"{"version":"0.1.0","root_hash":"","height":0,"modules":[],"public_key":"ab","operations":{"last_finalized_at":0,"storage":{"checkpoint_height":425981}}}"#;
    let rpc = node_that_serves_its_status_once(RESYNCING).await;

    let facts = load_node_facts(rpc, 9)
        .await
        .expect("one read fills the whole card; a second one is the defect");

    assert_eq!(
        facts.checkpoint_height, 425_981,
        "the checkpoint on disk is real and stays printed"
    );
    assert_eq!(
        facts.height, UNMEASURED,
        "a node serving no boundary reports height 0; that is absence, not a measurement"
    );
    assert_eq!(
        height_label_short(facts.height),
        "h —",
        "the rendered head says it has no reading"
    );
    assert!(
        facts.height < 0 || facts.checkpoint_height <= facts.height,
        "the pair may say `h —`, but it may never say a checkpoint above its head"
    );
}

/// A DISPLAY NAME MUST NOT BE FORMATTED TWICE. `search_chat` already runs the
/// wire author through `author_display`, so an Explorer hit arrives holding
/// "you", "user 48cedb0d…" or "@quackbot". The Explorer then ran `author_name`
/// over that a SECOND time; none of those strings carries a `user:`/`agent:`
/// prefix to split, so every one fell through to the `_` arm and every message
/// hit in workspace search was attributed to "system".
///
/// Driven: the same message reads `user 48cedb0d…` in the timeline and `system`
/// in Explorer search.
#[test]
fn a_search_hits_author_is_not_reformatted_into_system() {
    // What `search_chat` hands the Explorer, for each kind of author.
    for displayed in ["you", "user 48cedb0d…", "@quackbot", "chat"] {
        assert_eq!(
            author_name(displayed),
            "system",
            "a second pass over a display name loses it — this is why the hit \
             must carry `hit.author` through untouched"
        );
    }

    // And the first pass is the one that is correct.
    assert_eq!(author_display("user:48cedb0d131f", None), "user 48cedb0d…");
    assert_eq!(author_name("agent:demo/quackbot"), "@quackbot");

    // The call site itself, pinned: the message arm must carry the author
    // through, never re-format it. Without this the assertions above hold
    // while the Explorer goes on printing "system".
    const SEARCH: &str = include_str!("search.rs");
    let message_arm = SEARCH
        .split("kind: \"message\".into(),")
        .nth(1)
        .expect("the message hit arm")
        .split("});")
        .next()
        .expect("arm body");
    assert!(
        message_arm.contains("title: hit.author,"),
        "the message hit carries the display name it was handed"
    );
    assert!(
        !message_arm.contains("author_name("),
        "re-formatting it is what produced `system`"
    );
}

#[test]
fn escape_ladder_names_the_topmost_transient_layer_only() {
    use iced::keyboard::{Key, key::Named};

    let escape = Key::Named(Named::Escape);
    let target = |palette: bool,
                  bell: bool,
                  create: bool,
                  thread_action: &str,
                  action: &str,
                  drawer: bool,
                  repo_menu: bool| {
        escape_target(
            escape.clone(),
            palette,
            bell,
            create,
            thread_action.into(),
            action.into(),
            drawer,
            repo_menu,
        )
    };

    // Not Escape → nothing, whatever is open.
    assert_eq!(
        escape_target(
            Key::Character("x".into()),
            false,
            true,
            true,
            "more".into(),
            "more".into(),
            true,
            true,
        ),
        ""
    );
    // An open palette swallows Escape — palette_key_action owns it.
    assert_eq!(target(true, true, true, "more", "more", true, true), "");
    // The ladder order is the z-order: bell over the create modal, menus
    // after both, thread menu over the stream's, popovers last.
    assert_eq!(
        target(false, true, true, "more", "more", true, true),
        "bell"
    );
    assert_eq!(
        target(false, false, true, "more", "more", true, true),
        "channel_create"
    );
    assert_eq!(
        target(false, false, false, "more", "more", true, true),
        "thread_menu"
    );
    assert_eq!(
        target(false, false, false, "toolbar", "editing", true, true),
        "message_menu"
    );
    // THE DRAWER SITS BETWEEN THEM. Both message menus float over Channel
    // details, so they win; the repo menu lives on another tab, so it loses.
    // It had no rung at all — an `×` and no keyboard exit, while every other
    // overlay answered Escape. Measured: Escape over an open drawer changed
    // exactly zero pixels on the running app.
    assert_eq!(
        target(false, false, false, "toolbar", "toolbar", true, true),
        "channel_settings"
    );
    assert_eq!(
        target(false, false, false, "toolbar", "toolbar", false, true),
        "repo_menu"
    );
    // Nothing transient open → Escape is a no-op. The pages rungs are gone
    // with the menus they dismissed: the document has no transient layer.
    assert_eq!(
        target(false, false, false, "toolbar", "toolbar", false, false),
        ""
    );
}

// THE PANE SCROLL'S THREE CONDITIONS, one assertion each, over the router
// itself rather than over one key's pixels. #1006 shipped it with only the
// modifier condition: it claimed the arrows (which a focused single-line
// `text_input` leaves UNCAPTURED — `iced_widget-0.14.2/src/text_input.rs:1245`
// falls Up/Down through to `_ => {}` — so `status=ignored` handed them here
// while a caret sat in the field), and it never asked whether a transient
// layer was over the pane it was about to move.
#[test]
fn the_content_pane_claims_only_the_keys_nothing_else_owns() {
    use iced::keyboard::{Key, Modifiers, key::Named};

    let step = |named: Named, modifiers: Modifiers, overlay: &str| {
        content_scroll_step(Key::Named(named), modifiers, overlay.into())
    };
    let free = Modifiers::empty();

    // 1. THE PANE'S OWN KEYS. Page Up/Down and Home/End: iced's text widgets
    //    capture Home/End when focused, so one only ever reaches here with
    //    nothing focused, and no widget in this console owns a Page key.
    assert!(step(Named::PageDown, free, "") > 0.0);
    assert!(step(Named::PageUp, free, "") < 0.0);
    assert!(step(Named::End, free, "") > 0.0);
    assert!(step(Named::Home, free, "") < 0.0);

    // 2. AN ARROW BELONGS TO WHATEVER HAS FOCUS. Nothing in this stack can
    //    read widget focus, and a single-line input does not capture Up/Down,
    //    so the pane cannot tell a caret's arrow from its own and must not
    //    claim one — at any time, under any layer.
    assert_eq!(step(Named::ArrowDown, free, ""), 0.0);
    assert_eq!(step(Named::ArrowUp, free, ""), 0.0);

    // 3. A TRANSIENT LAYER'S KEY IS NOT THE PANE'S. Every rung `topmost_overlay`
    //    can name stops every scroll key, so no press moves the screen behind
    //    an open palette or bell.
    for overlay in [
        "palette",
        "bell",
        "channel_create",
        "thread_menu",
        "message_menu",
        "channel_settings",
        "repo_menu",
    ] {
        for key in [Named::PageDown, Named::PageUp, Named::End, Named::Home] {
            assert_eq!(step(key, free, overlay), 0.0, "{overlay} is over the pane");
        }
    }

    // 4. A CHORD IS NOT THE PANE'S — it belongs to its own router.
    assert_eq!(step(Named::PageDown, Modifiers::SHIFT, ""), 0.0);
    assert_eq!(step(Named::Home, Modifiers::CTRL, ""), 0.0);
}

#[test]
fn files_base64_round_trips() {
    for sample in [
        b"".as_slice(),
        b"a".as_slice(),
        b"ab".as_slice(),
        b"abc".as_slice(),
        b"hello duckfs \xf0\x9f\xa6\x86".as_slice(),
    ] {
        let encoded = base64_encode(sample);
        assert_eq!(
            base64_decode(&encoded).as_deref(),
            Some(sample),
            "{encoded}"
        );
    }
    assert_eq!(base64_encode(b"abc"), "YWJj");
    assert_eq!(base64_encode(b"ab"), "YWI=");
}

#[test]
fn signer_requires_the_encrypted_v1_key_format() {
    assert_eq!(password_line("secret").unwrap(), b"secret\n");
    assert!(password_line("").is_err());
    assert!(password_line("bad\nsecret").is_err());

    let directory = tempfile::tempdir().unwrap();
    let key = directory.path().join("user.key");
    std::fs::write(&key, format!("{ENCRYPTED_KEY_PREFIX}ciphertext")).unwrap();
    require_encrypted_key(&key).unwrap();
    std::fs::write(&key, "plaintext-key").unwrap();
    assert!(require_encrypted_key(&key).is_err());

    let public_key = "ab".repeat(32);
    assert_eq!(
        parse_user_key_status(&format!("encrypted {public_key}\n")),
        Some(vec![0xab; 32])
    );
    assert!(parse_user_key_status("absent\n").is_none());
    assert!(parse_user_key_status(&format!("plaintext {public_key}\n")).is_none());
}

/// THE session property on the app's side of the pipe: ONE unlock, then a
/// frame per request line, each answering ITS OWN request in order. The child
/// this drives is a stub, but the contract is the real one — a stray or
/// mispaired line here would mean the app submits the frame for another
/// operation, and the argon2id pass it skips is the whole point of the
/// session (a per-op process paid it on every reaction tap).
#[tokio::test(flavor = "current_thread")]
async fn one_unlock_signs_every_request_of_the_session() {
    use std::os::unix::fs::PermissionsExt as _;

    let directory = tempfile::tempdir().unwrap();
    let key = directory.path().join("user.key");
    std::fs::write(&key, format!("{ENCRYPTED_KEY_PREFIX}ciphertext")).unwrap();
    // The stub signer: records its unlock, then echoes each request's payload
    // back as the frame — so the answer names the request it belongs to.
    let unlocks = directory.path().join("unlocks");
    let binary = directory.path().join("stub-signer");
    std::fs::write(
        &binary,
        format!(
            "#!/bin/sh\necho unlock >> {}\nread password\n\
             while read -r target seq payload; do echo \"$payload\"; done\n",
            unlocks.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();

    let mut signer = super::rpc::Signer::unlock(binary, key, Zeroizing::new("secret".into()))
        .await
        .expect("the stub signer starts");
    for op in 0..5u8 {
        let payload = hex_encode(&[op, op, op]);
        let frame = signer
            .sign(&format!("chat {op} {payload}\n"))
            .await
            .expect("a frame per request");
        assert_eq!(frame, vec![op, op, op], "request {op} got another's frame");
    }

    assert_eq!(
        std::fs::read_to_string(&unlocks).unwrap(),
        "unlock\n",
        "five signed writes, one key open"
    );
}

#[test]
fn post_commit_hydration_errors_are_not_retryable() {
    let error = committed_error("read failed".into());
    assert!(error.committed);
    assert_eq!(error.message, "read failed");
}

#[test]
fn optimistic_reaction_survives_the_canonical_replay() {
    let mut message = optimistic_message(Vec::new(), "hello".into(), "message-a".into());
    message[0].pending = false;
    message[0].seq = 7;
    let reactor = "user:aa11".to_string();

    let tapped =
        ::chat::client::optimistic_reaction(message, 7, "👍".into(), true, reactor.clone());
    assert_eq!(tapped[0].reactions.len(), 1);
    assert_eq!(tapped[0].reactions[0].emoji, "👍");
    assert_eq!(tapped[0].reactions[0].count, 1);
    assert!(tapped[0].reactions[0].reacted_by_me);

    // The settled delta replays the SAME reactor handle — set semantics keep
    // the count at 1 instead of doubling the optimistic chip.
    let replay = ChatDelta {
        kind: "reaction".into(),
        channel_id: "general".into(),
        seq: 7,
        emoji: "👍".into(),
        added: true,
        reactor,
        by_me: true,
        ..ChatDelta::default()
    };
    let replayed = apply_chat_messages(tapped, replay, "general".into());
    assert_eq!(replayed[0].reactions.len(), 1);
    assert_eq!(replayed[0].reactions[0].count, 1);
    assert!(replayed[0].reactions[0].reacted_by_me);

    // The optimistic remove folds the chip away entirely.
    let removed =
        ::chat::client::optimistic_reaction(replayed, 7, "👍".into(), false, "user:aa11".into());
    assert!(removed[0].reactions.is_empty());
}

#[test]
fn reply_settle_flash_mirrors_the_stream_for_the_thread_rail() {
    let pending = optimistic_message(Vec::new(), "a reply".into(), "reply-a".into());
    let mut settled_row = pending[0].clone();
    settled_row.pending = false;
    settled_row.seq = 9;
    let settle = ChatDelta {
        kind: "reply".into(),
        channel_id: "general".into(),
        root_seq: 3,
        seq: 9,
        message: settled_row,
        ..ChatDelta::default()
    };
    assert!(reply_settled_by(&pending, &settle, "general"));
    // The fused verdict answers both lanes off one pass: a `reply` settle
    // anchors the rail's ✓ and leaves the stream's anchor alone.
    let verdict = chat_settle(
        Vec::new(),
        pending.clone(),
        settle.clone(),
        "general".into(),
        with_settled_id(String::new(), true, "kept".into()),
        String::new(),
    );
    assert!(verdict.flashed);
    assert!(has_flash_id(verdict.reply_ids, "reply-a".into()));
    assert!(has_flash_id(verdict.send_ids, "kept".into()));
    // Each lane fires only on its own delta kind: a `reply` is the rail's
    // edge and never the stream's, and a `posted` is the reverse.
    assert!(!send_settled_by(&pending, &settle, "general"));
    let mut as_post = settle.clone();
    as_post.kind = "posted".into();
    assert!(!reply_settled_by(&pending, &as_post, "general"));
    assert!(!reply_settled_by(&pending, &settle, "other"));
}

#[test]
fn send_settle_flash_fires_only_for_own_pending_rows() {
    let pending = optimistic_message(Vec::new(), "hello".into(), "message-a".into());
    let mut settled_row = pending[0].clone();
    settled_row.pending = false;
    settled_row.seq = 3;
    let settle = ChatDelta {
        kind: "posted".into(),
        channel_id: "general".into(),
        seq: 3,
        message: settled_row,
        ..ChatDelta::default()
    };

    assert!(send_settled_by(&pending, &settle, "general"));
    let verdict = chat_settle(
        pending.clone(),
        Vec::new(),
        settle.clone(),
        "general".into(),
        String::new(),
        with_settled_id(String::new(), true, "kept".into()),
    );
    assert!(verdict.flashed);
    assert!(has_flash_id(verdict.send_ids, "message-a".into()));
    assert!(has_flash_id(verdict.reply_ids, "kept".into()));
    // Wrong channel, someone else's post, and a non-post delta all keep the
    // current anchor instead of flashing.
    assert!(!send_settled_by(&pending, &settle, "other"));
    let mut foreign = settle.clone();
    foreign.message.id = "someone-else".into();
    assert!(!send_settled_by(&pending, &foreign, "general"));
    let mut reaction = settle;
    reaction.kind = "reaction".into();
    let unrelated = chat_settle(
        pending,
        Vec::new(),
        reaction,
        "general".into(),
        with_settled_id(String::new(), true, "kept".into()),
        with_settled_id(String::new(), true, "kept-reply".into()),
    );
    assert!(!unrelated.flashed);
    assert!(has_flash_id(unrelated.send_ids, "kept".into()));
    assert!(has_flash_id(unrelated.reply_ids, "kept-reply".into()));
}

#[test]
fn concurrent_optimistic_messages_settle_independently() {
    let pending = optimistic_message(
        optimistic_message(Vec::new(), "first".into(), "message-a".into()),
        "second".into(),
        "message-b".into(),
    );
    assert_eq!(
        pending
            .iter()
            .map(|message| message.id.as_str())
            .collect::<Vec<_>>(),
        ["message-a", "message-b"]
    );

    let canonical = |id: &str, seq: i64, body: &str| ChatMessage {
        id: id.into(),
        seq,
        author: "You".into(),
        meta: format!("#{seq}"),
        body: body.into(),
        blocks: paragraph_blocks(body),
        pending: false,
        rev: 0,
        edited: false,
        deleted: false,
        reply_count: 0,
        thread_seq: 0,
        show_author: true,
        initial: "Y".into(),
        avatar_kind: "human".into(),
        height: 0,
        time: 0,
        reactions: Vec::new(),
        render_rev: 0,
    };
    let after_second = merge_message_send_result(
        vec![canonical("message-b", 1, "second")],
        pending,
        "general".into(),
        "general".into(),
        "message-b".into(),
    );
    assert_eq!(after_second.len(), 2);
    assert!(!after_second[0].pending);
    assert_eq!(after_second[1].id, "message-a");
    assert!(after_second[1].pending);

    let settled = merge_message_send_result(
        vec![
            canonical("message-b", 1, "second"),
            canonical("message-a", 2, "first"),
        ],
        after_second,
        "general".into(),
        "general".into(),
        "message-a".into(),
    );
    assert_eq!(
        settled
            .iter()
            .map(|message| message.id.as_str())
            .collect::<Vec<_>>(),
        ["message-b", "message-a"]
    );
    assert!(settled.iter().all(|message| !message.pending));

    let after_stale_response = merge_message_send_result(
        vec![canonical("message-b", 1, "second")],
        settled,
        "general".into(),
        "general".into(),
        "message-b".into(),
    );
    assert_eq!(
        after_stale_response
            .iter()
            .map(|message| message.id.as_str())
            .collect::<Vec<_>>(),
        ["message-b", "message-a"]
    );
}

#[test]
fn message_groups_collapse_consecutive_authors() {
    let msg = |seq: i64, author: &str, deleted: bool| ChatMessage {
        id: format!("m{seq}"),
        seq,
        author: author.into(),
        meta: format!("#{seq}"),
        body: "body".into(),
        blocks: paragraph_blocks("body"),
        pending: false,
        rev: 0,
        edited: false,
        deleted,
        reply_count: 0,
        thread_seq: 0,
        show_author: false,
        initial: "A".into(),
        avatar_kind: "human".into(),
        height: 0,
        time: 0,
        reactions: Vec::new(),
        render_rev: 0,
    };
    let mut messages = vec![
        msg(1, "alice", false),
        msg(2, "alice", false),
        msg(3, "bob", false),
        msg(4, "bob", true),
        msg(5, "bob", false),
    ];
    mark_message_groups(&mut messages);
    let shown: Vec<bool> = messages.iter().map(|message| message.show_author).collect();
    // 1 opens the list; 2 shares alice -> continuation; 3 switches to bob -> header;
    // 4 is deleted -> header; 5 follows a deleted message -> header.
    assert_eq!(shown, vec![true, false, true, true, true]);
}

#[test]
fn history_pagination_prepends_older_and_flags_more() {
    let msg = |seq: i64| ChatMessage {
        id: format!("m{seq}"),
        seq,
        author: "alice".into(),
        meta: format!("#{seq}"),
        body: "body".into(),
        blocks: paragraph_blocks("body"),
        pending: false,
        rev: 0,
        edited: false,
        deleted: false,
        reply_count: 0,
        thread_seq: 0,
        show_author: false,
        initial: "A".into(),
        avatar_kind: "human".into(),
        height: 0,
        time: 0,
        reactions: Vec::new(),
        render_rev: 0,
    };
    // oldest loaded root is seq 3 -> older history exists.
    let loaded = vec![msg(3), msg(4), msg(5)];
    assert!(history_has_older(loaded.clone()));
    assert_eq!(oldest_message_seq(loaded.clone()), 3);
    // prepend an older page whose last item (seq 3) duplicates the current head.
    let merged = prepend_history(loaded, vec![msg(1), msg(2), msg(3)]);
    assert_eq!(
        merged.iter().map(|message| message.seq).collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5]
    );
    // now the oldest loaded root is seq 1 -> no more history to page.
    assert!(!history_has_older(merged));
}

#[test]
fn thread_offsets_advance_only_for_loaded_commits() {
    assert_eq!(thread_offset_after_reply(3, false, true), 4);
    assert_eq!(thread_offset_after_reply(3, false, false), 3);
    assert_eq!(thread_offset_after_reply(256, true, true), 256);
    assert_eq!(thread_offset_after_reply(-1, false, true), -1);
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

/// A DEFAULTED STATUS DOCUMENT MUST NOT READ AS MEASURED.
///
/// Asserting the RENDERED strings, not the integers: `-1` is only correct
/// because the renderers turn it into an em dash, and a future change to
/// either end that broke the pairing would leave an integer assertion green
/// while the screen went back to printing a head no node ever served.
#[test]
fn a_defaulted_node_facts_prints_as_unserved_everywhere() {
    let facts = NodeFacts::default();
    assert_eq!(facts.height, UNMEASURED);
    assert_eq!(facts.checkpoint_height, UNMEASURED);
    assert_eq!(facts.last_finalized_at, UNMEASURED);

    // What these render as is pinned once, by
    // `an_unpublished_operations_reading_renders_as_unknown` above — including
    // that a real `0` still prints `h 0`. Repeating it here would be
    // duplication wearing the costume of defence in depth.
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

#[tokio::test(flavor = "current_thread")]
async fn chat_and_pages_round_trip_over_signed_frames() {
    let storage = tempfile::tempdir().unwrap();
    let sim = simnode::boot(
        storage.path(),
        "127.0.0.1:0".parse().unwrap(),
        simnode::SimOpts {
            auto: true,
            ..Default::default()
        },
    )
    .unwrap();
    let origin = format!("http://{}", sim.addr());
    let rpc = RpcClient::new(&origin).unwrap();
    let signer = ed25519::PrivateKey::from_seed(7);

    submit_test(
        &rpc,
        &signer,
        1,
        "chat",
        chat::encode_msg(&ChatMsg::CreateChannel {
            channel_id: "general".into(),
            name: "General".into(),
            post_policy: PostPolicy::Open,
        }),
    )
    .await;
    submit_test(
        &rpc,
        &signer,
        2,
        "chat",
        chat::encode_msg(&ChatMsg::PostMessage {
            channel_id: "general".into(),
            message_id: "hello-1".into(),
            blocks: vec![chat::Block::paragraph("hello from the app")],
            thread: None,
            as_agent: None,
        }),
    )
    .await;
    submit_test(
        &rpc,
        &signer,
        3,
        "pages",
        pages::encode_msg(&PageMsg::CreatePage {
            page_id: "welcome".into(),
            title: "Welcome".into(),
        }),
    )
    .await;
    submit_test(
        &rpc,
        &signer,
        4,
        "pages",
        pages::encode_msg(&PageMsg::InsertBlock {
            parent: "welcome".into(),
            after: None,
            block: NewBlock {
                id: "intro".into(),
                kind: BlockKind::Paragraph,
                text: "A signed page block".into(),
                marks: Vec::new(),
            },
        }),
    )
    .await;

    let chat = load_chat_data(&rpc, Some("general")).await.unwrap();
    assert_eq!(chat.channels[0].name, "General");
    assert_eq!(chat.messages[0].body, "hello from the app");
    let pages = load_pages_data(&rpc, Some("welcome")).await.unwrap();
    assert_eq!(pages.active_page_title, "Welcome");
    assert_eq!(pages.blocks[0].text, "A signed page block");

    let origin = rpc.origin().to_string();
    let loaded_page = load_page(origin.clone(), "welcome".into()).await.unwrap();
    assert_eq!(loaded_page.active_page, "welcome");
    assert_eq!(loaded_page.blocks[0].text, "A signed page block");

    // A SAVE AGAINST A PAGE THE INDEX DOES NOT HOLD MUST REFUSE, NOT RETARGET.
    // `load_pages_data` answers a missing id with `pages.first()` — here that is
    // `welcome`, a real page full of real blocks. Without the guard in
    // `save_page_document` this call plans one buffer against another page's
    // blocks and emits removes against ITS ids. It refuses before any write, so
    // the password never has to be real.
    //
    // THE TITLE MUST MATCH THE PAGE IT WOULD FALL BACK TO, or this test proves
    // nothing: a differing title makes the title write fire first, and it dies
    // `BlockNotFound` on the id that does not exist. That accident is the only
    // thing standing between today's code and the corruption — and it does not
    // happen when the titles agree, which two untitled pages always do. With
    // the title matched, the body plan is `remove every line`.
    let stray = save_page_document(
        origin.clone(),
        String::new(),
        "no-such-page".into(),
        "Welcome\n".into(),
        "Welcome\n".into(),
        0,
    )
    .await;
    // ASSERT THE REASON, NOT JUST THE FAILURE. An unsigned save fails anyway —
    // at the signer, several steps after the plan was already built against the
    // wrong page's blocks. Only the message separates "refused before planning"
    // from "planned the damage, then could not sign it".
    let refusal = stray.expect_err("a save must not retarget to another page");
    assert_eq!(
        refusal.message, "page was not found",
        "the save must refuse on the page it cannot find, before it plans or signs anything"
    );
    let after = load_pages_data(&rpc, Some("welcome")).await.unwrap();
    assert_eq!(
        after.blocks[0].text, "A signed page block",
        "the refused save must not have touched the page it fell back to"
    );
    let workspace = connect(origin.clone(), 0, 0).await.unwrap();
    let mut live = live_events(origin.clone());
    let ready = next_change(&mut live).await;
    assert_eq!(ready.kind, "ready");
    submit_test(
        &rpc,
        &signer,
        5,
        "chat",
        chat::encode_msg(&ChatMsg::PostMessage {
            channel_id: "general".into(),
            message_id: "hello-2".into(),
            blocks: vec![chat::Block::paragraph("arrived on the next block")],
            thread: None,
            as_agent: None,
        }),
    )
    .await;
    let changed = next_change(&mut live).await;
    assert_eq!(changed.kind, "chat", "a chat op folds into a chat delta");
    assert_eq!(changed.chat.kind, "posted");
    assert_eq!(changed.chat.channel_id, "general");
    assert_eq!(
        changed.chat.seq, 2,
        "the delta carries the module-assigned sequence from the feed stamp"
    );
    assert_eq!(changed.chat.message.body, "arrived on the next block");
    assert!(
        !changed.load_chat && !changed.load_pages,
        "a folded chat delta requires no reload"
    );
    assert!(changed.height > workspace.height);
    let base_height = changed.height;
    submit_test(
        &rpc,
        &signer,
        6,
        "chat",
        chat::encode_msg(&ChatMsg::PostMessage {
            channel_id: "general".into(),
            message_id: "reply-1".into(),
            blocks: vec![chat::Block::paragraph("a threaded reply")],
            thread: Some(1),
            as_agent: None,
        }),
    )
    .await;
    submit_test(
        &rpc,
        &signer,
        7,
        "chat",
        chat::encode_msg(&ChatMsg::EditMessage {
            channel_id: "general".into(),
            seq: 1,
            blocks: vec![chat::Block::paragraph("hello, edited")],
            base_rev: Some(0),
        }),
    )
    .await;
    submit_test(
        &rpc,
        &signer,
        8,
        "chat",
        chat::encode_msg(&ChatMsg::AddReaction {
            channel_id: "general".into(),
            seq: 1,
            emoji: "👍".into(),
        }),
    )
    .await;

    wait_for_block(&mut live, base_height + 3).await;
    let chat = load_chat_data(&rpc, Some("general")).await.unwrap();
    assert_eq!(chat.active_channel_name, "General");
    assert_eq!(chat.messages[0].body, "hello, edited");
    assert!(chat.messages[0].edited);
    assert_eq!(chat.messages[0].reply_count, 1);
    assert_eq!(chat.messages[0].reactions[0].emoji, "👍");
    let thread = load_thread_data(&rpc, "general", 1, 0).await.unwrap();
    assert_eq!(thread.messages.len(), 2);
    assert_eq!(thread.messages[1].body, "a threaded reply");
    let hit = load_chat_hit(
        origin.clone(),
        chat.channels.clone(),
        "general".into(),
        1,
        3,
        7,
    )
    .await
    .unwrap();
    // ONE ROW BACK, NOT THE LIST IT WAS HANDED. The switch loaders take the
    // reader's list only as a `head_seq` hint; carrying it back would have the
    // reducer revert every delta the live stream folded during the round trip
    // (`upsert_channel_rows`).
    assert_eq!(
        hit.channels
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        vec!["general"]
    );
    assert_eq!(hit.generation, 7);
    assert_eq!(hit.selected_message_seq, 1);
    assert_eq!(hit.active_thread_seq, 1);
    assert_eq!(hit.thread_target_seq, 3);
    assert_eq!(hit.thread_messages[1].body, "a threaded reply");
    submit_test(
        &rpc,
        &signer,
        9,
        "pages",
        pages::encode_msg(&PageMsg::InsertBlock {
            parent: "welcome".into(),
            after: Some("intro".into()),
            block: NewBlock {
                id: "heading".into(),
                kind: BlockKind::Heading2,
                text: "Nested work".into(),
                marks: Vec::new(),
            },
        }),
    )
    .await;
    submit_test(
        &rpc,
        &signer,
        10,
        "pages",
        pages::encode_msg(&PageMsg::InsertBlock {
            parent: "heading".into(),
            after: None,
            block: NewBlock {
                id: "todo".into(),
                kind: BlockKind::Todo,
                text: "Ship the editor".into(),
                marks: Vec::new(),
            },
        }),
    )
    .await;
    submit_test(
        &rpc,
        &signer,
        11,
        "pages",
        pages::encode_msg(&PageMsg::SetChecked {
            block_id: "todo".into(),
            checked: true,
        }),
    )
    .await;
    submit_test(
        &rpc,
        &signer,
        12,
        "pages",
        pages::encode_msg(&PageMsg::InsertBlock {
            parent: "welcome".into(),
            after: Some("heading".into()),
            block: NewBlock {
                id: "child".into(),
                kind: BlockKind::Page,
                text: "Child page".into(),
                marks: Vec::new(),
            },
        }),
    )
    .await;

    wait_for_block(&mut live, base_height + 7).await;
    let pages = load_pages_data(&rpc, Some("welcome")).await.unwrap();
    assert_eq!(pages.pages[0].id, "welcome");
    assert_eq!(pages.pages[1].id, "child");
    assert_eq!(pages.pages[1].prefix, "  ");
    assert_eq!(pages.blocks[2].id, "todo");
    assert_eq!(pages.blocks[2].prefix, "  ");
    assert!(pages.blocks[2].checked);

    submit_test(
        &rpc,
        &signer,
        13,
        "pages",
        pages::encode_msg(&PageMsg::AddComment {
            thread_id: "thread-live".into(),
            comment_id: "comment-live".into(),
            target: "intro".into(),
            text: "temporary".into(),
            anchor: None,
            mentions: Vec::new(),
            as_agent: None,
        }),
    )
    .await;
    wait_for_block(&mut live, base_height + 8).await;
    let threads = load_page_threads(origin.clone(), "welcome".into(), 1)
        .await
        .unwrap();
    assert!(
        threads
            .threads
            .iter()
            .any(|thread| thread.id == "thread-live"),
        "the live comment's thread is on the page rail"
    );
    submit_test(
        &rpc,
        &signer,
        14,
        "pages",
        pages::encode_msg(&PageMsg::DeleteComment {
            comment_id: "comment-live".into(),
        }),
    )
    .await;
    wait_for_block(&mut live, base_height + 9).await;
    let threads = load_page_threads(origin.clone(), "welcome".into(), 2)
        .await
        .unwrap();
    assert!(
        !threads
            .threads
            .iter()
            .any(|thread| thread.id == "thread-live"),
        "the deleted comment's thread is gone from the page rail"
    );

    let refreshed = live_resync_load(
        origin,
        "general".into(),
        "welcome".into(),
        "both".into(),
        false,
        7,
        3,
        0,
    )
    .await
    .unwrap();
    assert_eq!(refreshed.generation, 7);
    assert_eq!(
        refreshed.fold_serial, 3,
        "the reply echoes the fold serial the request snapshotted (#1041)"
    );
    assert!(refreshed.chat_loaded && refreshed.pages_loaded);
    assert_eq!(refreshed.messages[1].body, "arrived on the next block");
    assert_eq!(refreshed.active_page, "welcome");
    sim.shutdown();
}

/// The composer's grammar loop, closed over a real node: the SAME parser
/// the rich composer previews (`parse_message_with_members`) builds the
/// committed blocks, and the spans read back off the node still carry the
/// marks. If the preview grammar and the renderer grammar ever drift, one
/// of the two ends of this test moves.
#[tokio::test(flavor = "current_thread")]
async fn composer_markdown_round_trips_rich_spans() {
    let storage = tempfile::tempdir().unwrap();
    let sim = simnode::boot(
        storage.path(),
        "127.0.0.1:0".parse().unwrap(),
        simnode::SimOpts {
            auto: true,
            ..Default::default()
        },
    )
    .unwrap();
    let origin = format!("http://{}", sim.addr());
    let rpc = RpcClient::new(&origin).unwrap();
    let signer = ed25519::PrivateKey::from_seed(11);

    submit_test(
        &rpc,
        &signer,
        1,
        "chat",
        chat::encode_msg(&ChatMsg::CreateChannel {
            channel_id: "general".into(),
            name: "General".into(),
            post_policy: PostPolicy::Open,
        }),
    )
    .await;
    submit_test(
        &rpc,
        &signer,
        2,
        "chat",
        chat::encode_msg(&ChatMsg::PostMessage {
            channel_id: "general".into(),
            message_id: "styled-1".into(),
            blocks: parse_message_with_members("say **hi** to _all_", &[]),
            thread: None,
            as_agent: None,
        }),
    )
    .await;

    let chat = load_chat_data(&rpc, Some("general")).await.unwrap();
    let message = &chat.messages[0];
    let block = &message.blocks[0];
    assert!(block.rich, "marked text lands as a rich paragraph");
    let bold = block
        .spans
        .iter()
        .find(|span| span.text.contains("hi"))
        .expect("the bold run survives the round trip");
    assert!(bold.bold && !bold.italic);
    let italic = block
        .spans
        .iter()
        .find(|span| span.text.contains("all"))
        .expect("the italic run survives the round trip");
    assert!(italic.italic && !italic.bold);
    sim.shutdown();
}

#[tokio::test(flavor = "current_thread")]
async fn timeline_pages_past_thread_only_traffic() {
    let storage = tempfile::tempdir().unwrap();
    let sim = simnode::boot(
        storage.path(),
        "127.0.0.1:0".parse().unwrap(),
        simnode::SimOpts {
            auto: true,
            ..Default::default()
        },
    )
    .unwrap();
    let origin = format!("http://{}", sim.addr());
    let rpc = RpcClient::new(&origin).unwrap();
    let signer = ed25519::PrivateKey::from_seed(8);

    submit_test(
        &rpc,
        &signer,
        1,
        "chat",
        chat::encode_msg(&ChatMsg::CreateChannel {
            channel_id: "general".into(),
            name: "General".into(),
            post_policy: PostPolicy::Open,
        }),
    )
    .await;
    submit_test(
        &rpc,
        &signer,
        2,
        "chat",
        chat::encode_msg(&ChatMsg::PostMessage {
            channel_id: "general".into(),
            message_id: "root".into(),
            blocks: vec![chat::Block::paragraph("root stays visible")],
            thread: None,
            as_agent: None,
        }),
    )
    .await;
    for index in 0_u64..257 {
        submit_test(
            &rpc,
            &signer,
            index + 3,
            "chat",
            chat::encode_msg(&ChatMsg::PostMessage {
                channel_id: "general".into(),
                message_id: format!("reply-{index}"),
                blocks: vec![chat::Block::paragraph(format!("reply {index}"))],
                thread: Some(1),
                as_agent: None,
            }),
        )
        .await;
    }

    let chat = load_chat_data(&rpc, Some("general")).await.unwrap();
    assert_eq!(chat.messages.len(), 1);
    assert_eq!(chat.messages[0].body, "root stays visible");
    // The backward walk is page-bounded now, but the bound may never cut it
    // short of a root: a page that yields none keeps going, so the walk still
    // reaches seq 1 here and "Load older messages" stays correctly hidden
    // rather than becoming a button that returns an empty page forever.
    assert!(!history_has_older(chat.messages.clone()));
    let first = load_thread_data(&rpc, "general", 1, 0).await.unwrap();
    assert_eq!(first.messages.len(), 257);
    assert_eq!(first.next_reply_offset, 256);
    assert!(first.has_more);
    let last = load_thread_page(origin.clone(), "general".into(), 1, 256, 9)
        .await
        .unwrap();
    assert_eq!(last.messages.len(), 1);
    assert_eq!(last.messages[0].body, "reply 256");
    assert_eq!(last.next_reply_offset, 257);
    assert!(!last.has_more);
    let sparse = load_thread(origin, "general".into(), 1, 258, -1, 10)
        .await
        .unwrap();
    assert_eq!(sparse.target_seq, 258);
    assert_eq!(sparse.next_reply_offset, -1);
    assert_eq!(sparse.messages.len(), 2);
    assert_eq!(sparse.messages[1].body, "reply 256");
    sim.shutdown();
}

/// The page that carried the walk past the root quota is kept whole rather
/// than trimmed back to it. Those rows already came over the wire, so
/// discarding them would only mean fetching them again on the first "Load
/// older messages" click — and the timeline that mounts them is virtualized,
/// so holding them costs no layout. The quota bounds round trips, not rows.
#[tokio::test(flavor = "current_thread")]
async fn a_timeline_load_keeps_the_page_that_crossed_the_root_quota() {
    let storage = tempfile::tempdir().unwrap();
    let sim = simnode::boot(
        storage.path(),
        "127.0.0.1:0".parse().unwrap(),
        simnode::SimOpts {
            auto: true,
            ..Default::default()
        },
    )
    .unwrap();
    let origin = format!("http://{}", sim.addr());
    let rpc = RpcClient::new(&origin).unwrap();
    let signer = ed25519::PrivateKey::from_seed(9);

    submit_test(
        &rpc,
        &signer,
        1,
        "chat",
        chat::encode_msg(&ChatMsg::CreateChannel {
            channel_id: "general".into(),
            name: "General".into(),
            post_policy: PostPolicy::Open,
        }),
    )
    .await;

    // Comfortably past the quota, and still inside one 256-row page, so the
    // walk crosses the quota and stops within a single fetch.
    let roots = CHAT_TIMELINE_ROOT_QUOTA + 20;
    for index in 0..roots {
        submit_test(
            &rpc,
            &signer,
            index as u64 + 2,
            "chat",
            chat::encode_msg(&ChatMsg::PostMessage {
                channel_id: "general".into(),
                message_id: format!("root-{index}"),
                blocks: vec![chat::Block::paragraph(format!("root {index}"))],
                thread: None,
                as_agent: None,
            }),
        )
        .await;
    }

    let chat = load_chat_data(&rpc, Some("general")).await.unwrap();
    assert_eq!(
        chat.messages.len(),
        roots,
        "the crossing page is returned whole; trimming it back to the quota \
         would re-fetch these rows on the first Load older click"
    );
    assert_eq!(chat.messages[0].body, "root 0");
    assert_eq!(chat.messages[roots - 1].body, format!("root {}", roots - 1));
    sim.shutdown();
}

#[test]
fn hydration_retry_is_capped() {
    assert_eq!(retry_delay(1), Duration::from_secs(1));
    assert_eq!(retry_delay(3), Duration::from_secs(4));
    assert_eq!(retry_delay(99), Duration::from_secs(16));
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
        bounded_exact_text(String::new(), "page title", 512).unwrap(),
        ""
    );
}

#[test]
fn cancelling_autosaves_bumps_the_generation() {
    assert_eq!(cancel_autosaves("http://old".into(), 4), 5);
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

/// A cold start used to open on `channels.first()` — wire order is by ID, so
/// the demo workspace landed on an empty `channel-1786073…` and the console
/// said "No messages yet" with three populated rooms listed under it.
#[test]
fn a_cold_start_lands_on_a_room_with_something_in_it() {
    let channel = |id: &str, head: i64, archived: bool| ChatChannel {
        id: id.into(),
        name: id.into(),
        archived,
        members_only: false,
        huddle_count: 0,
        head_seq: head,
    };
    let landing = |channels: &[ChatChannel]| {
        landing_channel(channels)
            .map(|channel| channel.id.clone())
            .unwrap_or_default()
    };

    // The demo's own shape: the empty room sorts first by ID.
    let demo = vec![
        channel("channel-1786073", 0, false),
        channel("engineering", 46, false),
        channel("general", 9, false),
    ];
    assert_eq!(landing(&demo), "engineering");

    // An archived room is not a landing even when it is the only one with
    // traffic — you cannot post into it.
    let archived_history = vec![channel("archive", 500, true), channel("general", 0, false)];
    assert_eq!(landing(&archived_history), "general");

    // Every room empty, and every room archived: still land somewhere.
    assert_eq!(
        landing(&[channel("a", 0, false), channel("b", 0, false)]),
        "a"
    );
    assert_eq!(
        landing(&[channel("a", 0, true), channel("b", 5, true)]),
        "a"
    );
    assert_eq!(landing(&[]), "");

    // The chooser is only worth anything if the loader routes through it.
    const LOAD: &str = include_str!("load.rs");
    assert!(
        LOAD.contains(".or_else(|| landing_channel(&channels).map(|channel| channel.id.clone()))"),
        "load_chat_data falls back through the chooser, not through .first()"
    );
}

#[test]
fn client_local_unread_tracking_seeds_marks_and_places_the_divider() {
    let channel = |id: &str, head: i64| ChatChannel {
        id: id.into(),
        name: id.into(),
        archived: false,
        members_only: false,
        huddle_count: 0,
        head_seq: head,
    };
    let read = |channel: &str, seq: i64| ChannelRead {
        channel: channel.into(),
        seq,
    };
    let message = |seq: i64, pending: bool| ChatMessage {
        id: format!("m{seq}"),
        seq: if pending { -1 } else { seq },
        author: "u".into(),
        meta: String::new(),
        body: String::new(),
        blocks: Vec::new(),
        pending,
        rev: 0,
        edited: false,
        deleted: false,
        reply_count: 0,
        thread_seq: 0,
        show_author: true,
        initial: "U".into(),
        avatar_kind: "human".into(),
        height: 0,
        time: 0,
        reactions: Vec::new(),
        render_rev: 0,
    };

    let reads = vec![read("general", 100), read("random", 30)];
    let channels = vec![channel("general", 100), channel("random", 50)];

    // channel_last_read / channel_head_seq: lookup, 0 when absent.
    assert_eq!(channel_last_read(reads.clone(), "random".into()), 30);
    assert_eq!(channel_last_read(reads.clone(), "missing".into()), 0);
    assert_eq!(channel_head_seq(channels.clone(), "random".into()), 50);
    assert_eq!(channel_head_seq(channels.clone(), "missing".into()), 0);

    // mark_channel_read upserts to the max, adds absent, ignores empty id.
    let marked = mark_channel_read(reads.clone(), "random".into(), 50);
    assert_eq!(channel_last_read(marked.clone(), "random".into()), 50);
    let lowered = mark_channel_read(marked, "random".into(), 40);
    assert_eq!(channel_last_read(lowered, "random".into()), 50);
    let added = mark_channel_read(reads.clone(), "new".into(), 7);
    assert_eq!(channel_last_read(added, "new".into()), 7);
    assert_eq!(
        mark_channel_read(reads.clone(), String::new(), 9).len(),
        reads.len()
    );

    // Every prepared row carries its own unread scalar. Both sections resolve
    // it once when source state moves, never from a list-taking view call.
    let rooms = chat_sidebar_rooms(channels.clone(), Vec::new(), String::new(), reads.clone());
    assert!(!rooms[0].unread);
    assert!(rooms[1].unread);
    let dm = DmPeer {
        key: "peer".into(),
        name: "Peer".into(),
        initials: "P".into(),
        is_agent: false,
        channel_id: "random".into(),
    };
    let dms = chat_sidebar_dms(channels.clone(), vec![dm], reads.clone());
    assert!(dms[0].unread);
    assert!(
        !chat_sidebar_rooms(
            vec![channel("random", 30)],
            Vec::new(),
            String::new(),
            reads.clone(),
        )[0]
        .unread
    );

    // initial_channel_reads: seed absent channels to head, preserve existing.
    let seeded = initial_channel_reads(channels.clone(), vec![read("random", 30)]);
    assert_eq!(channel_last_read(seeded.clone(), "random".into()), 30);
    assert_eq!(channel_last_read(seeded.clone(), "general".into()), 100);
    assert!(
        !chat_sidebar_rooms(
            vec![channel("general", 100)],
            Vec::new(),
            String::new(),
            seeded,
        )[0]
        .unread
    );

    // first_unread_seq: first message past the boundary; pending (seq -1)
    // never anchors it; 0 when caught up.
    let messages = vec![
        message(31, false),
        message(40, false),
        message(50, false),
        message(0, true),
    ];
    assert_eq!(first_unread_seq(messages.clone(), 30), 31);
    assert_eq!(first_unread_seq(messages.clone(), 45), 50);
    assert_eq!(first_unread_seq(messages.clone(), 50), 0);
    assert_eq!(first_unread_seq(messages, 0), 0);

    // frozen_unread_boundary: same channel is left untouched; a change
    // re-freezes at the arrived channel's last-read, or 0 when caught up.
    assert_eq!(
        frozen_unread_boundary(
            reads.clone(),
            channels.clone(),
            "random".into(),
            "random".into(),
            30
        ),
        30
    );
    assert_eq!(
        frozen_unread_boundary(
            reads.clone(),
            channels.clone(),
            "general".into(),
            "random".into(),
            999
        ),
        30
    );
    let caught_up = vec![read("general", 100), read("random", 50)];
    assert_eq!(
        frozen_unread_boundary(caught_up, channels, "general".into(), "random".into(), 999),
        0
    );
}

/// The next update that says something happened.
///
/// THE TIP ARRIVES FIRST, EVERY BLOCK. The node sends the heartbeat on the
/// block wake and only THEN catches its topics up (`bin/noded/src/stream.rs`,
/// the `block_rx` arm), so the head for block N is on the wire before N's ops
/// are. A test that asserts on `live.next()` directly is asserting on that
/// heartbeat, not on its own submit.
async fn next_change(
    live: &mut iced::futures::stream::BoxStream<'static, LiveUpdate>,
) -> LiveUpdate {
    loop {
        let update = live.next().await.expect("live stream ended");
        if update.kind != "tip" {
            return update;
        }
        // AND WHILE WE ARE HOLDING A REAL ONE, PIN IT HERE. The unit test below
        // builds its own tip, so it can only speak for `live_update` — it stays
        // green if the stream's arm starts asking for a load. These are the
        // tips the node actually sent, decoded by the real client, so this is
        // the assertion that binds the arm.
        assert!(update.height > 0, "a tip carries the head it was sent with");
        assert!(
            !update.load_chat && !update.load_pages,
            "a tip must not trigger a load — that is a 1 Hz poll on an idle chain"
        );
    }
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

/// A TIP MOVES THE HEAD AND MUST FETCH NOTHING.
///
/// The heartbeat rides every block, and an idle chain nop-fills once per
/// `BLOCK_TIME` (`bin/node/src/constants.rs`) — so anything this update
/// triggers runs at ~1 Hz forever, on a chain where nothing happened. A load
/// hung off it would be a poll wearing a consensus costume, and `/v1/query` is
/// checkpoint-gated (`backend/live.rs`), so that poll would also be the thing
/// that hands a healthy node's console "error sending request".
///
/// `assert_no_polling` cannot see this: it greps `lifecycle.ice` for lines
/// starting with `every ` and a load reached through a live update is invisible
/// to it. So the guard is here, on the value itself.
#[test]
fn a_tip_carries_the_head_and_loads_nothing() {
    let tip = live_update("tip", "Live · block 41", 41);
    assert_eq!(tip.height, 41, "the head is the tip's entire payload");
    assert!(
        !tip.load_chat && !tip.load_pages,
        "a tip must not trigger a load — that is a 1 Hz poll on an idle chain"
    );
    assert!(
        !tip.debounce,
        "there is nothing to coalesce: a tip fetches nothing"
    );
    assert!(
        tip.module.is_empty(),
        "a heartbeat is not a topic, so it names no module"
    );
}

/// drain the live event stream until the index has folded the block at
/// `min_height` — the system's own commit signal, never a timed poll.
async fn wait_for_block(
    live: &mut iced::futures::stream::BoxStream<'static, LiveUpdate>,
    min_height: i64,
) {
    loop {
        let update = live.next().await.expect("live stream ended");
        let folded = update.kind == "chat" || update.kind == "pages";
        if folded && update.height >= min_height {
            return;
        }
    }
}

async fn submit_test(
    rpc: &RpcClient,
    signer: &ed25519::PrivateKey,
    sequence: u64,
    target: &str,
    payload: Vec<u8>,
) {
    let frame = node::encode_frame(
        signer,
        sequence,
        &sdk::Msg {
            target: target.into(),
            payload,
        },
    );
    rpc.submit_frame(frame).await.unwrap();
}

/// One commit in `repo` holding exactly `files`, on top of `parent`.
fn mirror_commit(
    repo: &git2::Repository,
    parent: Option<git2::Oid>,
    files: &[(&str, &str)],
) -> git2::Oid {
    let mut tree = repo.treebuilder(None).unwrap();
    for (path, contents) in files {
        let blob = repo.blob(contents.as_bytes()).unwrap();
        tree.insert(path, blob, 0o100644).unwrap();
    }
    let tree = repo.find_tree(tree.write().unwrap()).unwrap();
    let signature = git2::Signature::now("mule", "mule@localhost").unwrap();
    let parents: Vec<git2::Commit> = parent
        .map(|oid| vec![repo.find_commit(oid).unwrap()])
        .unwrap_or_default();
    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
    repo.commit(None, &signature, &signature, "mule", &tree, &parent_refs)
        .unwrap()
}

#[test]
fn merge_builder_produces_the_cas_commit_and_its_minimal_pack() {
    let dir = tempfile::tempdir().unwrap();
    let mirror = git2::Repository::init_bare(dir.path()).unwrap();
    let base = mirror_commit(&mirror, None, &[("a.txt", "base\n"), ("b.txt", "keep\n")]);
    let ours = mirror_commit(
        &mirror,
        Some(base),
        &[("a.txt", "ours\n"), ("b.txt", "keep\n")],
    );
    let theirs = mirror_commit(
        &mirror,
        Some(base),
        &[("a.txt", "base\n"), ("b.txt", "theirs\n")],
    );

    let build = merge_against_mirror(&mirror, ours, theirs, "Merge pull request #1").unwrap();
    let MergeBuild::Clean { merge_oid, pack } = build else {
        panic!("disjoint edits must merge cleanly");
    };

    // land the pack in the mirror and read the merge commit back out —
    // exactly what a validator does after the blob fan-out.
    let odb = mirror.odb().unwrap();
    let mut writepack = odb.packwriter().unwrap();
    std::io::Write::write_all(&mut writepack, &pack).unwrap();
    writepack.commit().unwrap();
    let merged = mirror
        .find_commit(git2::Oid::from_str(&merge_oid).unwrap())
        .unwrap();
    let parents: Vec<git2::Oid> = merged.parent_ids().collect();
    assert_eq!(parents, vec![ours, theirs], "target first, source second");
    let tree = merged.tree().unwrap();
    let read = |path: &str| {
        let entry = tree.get_path(Path::new(path)).unwrap();
        String::from_utf8(mirror.find_blob(entry.id()).unwrap().content().to_vec()).unwrap()
    };
    assert_eq!(read("a.txt"), "ours\n");
    assert_eq!(read("b.txt"), "theirs\n");
}

#[test]
fn merge_builder_reports_conflicts_and_builds_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mirror = git2::Repository::init_bare(dir.path()).unwrap();
    let base = mirror_commit(&mirror, None, &[("a.txt", "base\n")]);
    let ours = mirror_commit(&mirror, Some(base), &[("a.txt", "ours\n")]);
    let theirs = mirror_commit(&mirror, Some(base), &[("a.txt", "theirs\n")]);

    let build = merge_against_mirror(&mirror, ours, theirs, "Merge pull request #2").unwrap();
    let MergeBuild::Conflicts(paths) = build else {
        panic!("competing edits must conflict");
    };
    assert_eq!(paths, vec!["a.txt".to_string()]);
}

#[test]
fn forge_code_replies_keep_the_server_revision_and_preview_flags() {
    let rev = "1".repeat(40);
    let tree = tree_data(
        serde_json::json!({ "tree": {
            "rev": rev,
            "born": true,
            "entries": [{
                "path": "src/lib.rs",
                "name": "lib.rs",
                "kind": "file"
            }],
            "truncated": true
        }}),
        "core".into(),
        "src".into(),
        7,
    )
    .unwrap();
    assert_eq!(tree.generation, 7);
    assert_eq!(tree.rev, "1".repeat(40));
    assert!(tree.born);
    assert!(tree.truncated);
    assert_eq!(tree.entries[0].path, "src/lib.rs");

    let text = blob_view(
        serde_json::json!({ "blob": {
            "rev": "1".repeat(40),
            "path": "src/lib.rs",
            "text": "one\ntwo\n",
            "size": 8,
            "truncated": true,
            "binary": false
        }}),
        "core".into(),
        8,
    )
    .unwrap();
    assert_eq!(text.lines, 2);
    assert!(text.truncated && !text.binary);

    let binary = blob_view(
        serde_json::json!({ "blob": {
            "rev": "1".repeat(40),
            "path": "asset.bin",
            "text": "",
            "size": 400,
            "truncated": false,
            "binary": true
        }}),
        "core".into(),
        9,
    )
    .unwrap();
    assert!(binary.binary);
    assert_eq!(binary.lines, 0);

    assert_eq!(
        blob_view(
            serde_json::json!({ "blob": null }),
            "core".into(),
            10,
        )
        .unwrap_err(),
        "the requested file was not found"
    );
}

#[test]
fn bell_severity_projects_the_kind_and_defaults_to_info() {
    assert_eq!(bell_severity("run_failed".into()), "danger");
    assert_eq!(bell_severity("review_requested".into()), "warning");
    assert_eq!(bell_severity("mentioned".into()), "info");
    // an unnamed kind is a notice, never an alarm.
    assert_eq!(bell_severity("brand_new_kind".into()), "info");
}

#[test]
fn bell_badge_takes_the_worst_unread_severity() {
    let item = |seq: i64, kind: &str, read: bool| BellItem {
        seq,
        kind: kind.into(),
        body: String::new(),
        source: String::new(),
        height: 0,
        read,
    };

    assert_eq!(
        bell_worst_severity(vec![
            item(1, "mentioned", false),
            item(2, "run_failed", false)
        ]),
        "danger"
    );
    // a READ error does not keep the badge red.
    assert_eq!(
        bell_worst_severity(vec![
            item(1, "run_failed", true),
            item(2, "review_requested", false)
        ]),
        "warning"
    );
    assert_eq!(bell_worst_severity(Vec::new()), "info");
}

#[test]
fn a_refused_key_password_reaches_the_screen_as_a_sentence() {
    // Verbatim what the key tool hands the app on a mistyped unlock password —
    // the mapping keys on the CLI's own `WRONG_PASSWORD_ERR` text.
    let refused =
        user_error("ducktape user key unlock refused: FATAL: corrupt or wrong password".into());
    assert_eq!(
        refused,
        "That password did not open this device's key. Check it and try again."
    );
    // A module's own sentence still flows through untouched.
    assert_eq!(user_error("post is empty".into()), "post is empty");
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
    const SEARCH: &str = include_str!("search.rs");
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
    const PALETTE: &str = include_str!("../ui/screens/overlays.ice");
    const PANEL: &str = include_str!("../ui/components/pages.ice");
    assert!(
        PALETTE.contains("text hit.page_title"),
        "the palette's page hit names its page"
    );
    assert!(
        PANEL.contains("text hit.page_title") && !PANEL.contains("text hit.block_id"),
        "the pages search panel names the page instead of printing a raw block id"
    );
}

/// A node whose page SEARCH answers and whose page LIST refuses — the exact
/// split the title join has to survive. Answers one request per connection and
/// closes, so the two views of a search never share a socket. Returns its
/// origin.
async fn node_with_a_broken_page_list() -> String {
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

    /// The request is in hand once the body reaches its declared length.
    fn request_is_complete(request: &[u8]) -> bool {
        let text = String::from_utf8_lossy(request);
        let Some((head, body)) = text.split_once("\r\n\r\n") else {
            return false;
        };
        let declared = head
            .to_lowercase()
            .lines()
            .find_map(|line| {
                line.strip_prefix("content-length:")?
                    .trim()
                    .parse::<usize>()
                    .ok()
            })
            .unwrap_or(0);
        body.len() >= declared
    }

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind the stub node");
    let origin = format!("http://{}", listener.local_addr().expect("stub address"));
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let mut request = Vec::new();
            let mut chunk = [0u8; 2048];
            while let Ok(read) = stream.read(&mut chunk).await {
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..read]);
                if request_is_complete(&request) {
                    break;
                }
            }
            // Both views POST to `/v1/index/pages/view`; only the body says
            // which one this is. Shapes copied from the live demo node.
            let asked_for_the_index = String::from_utf8_lossy(&request).contains("list_pages");
            let (status, body) = match asked_for_the_index {
                true => ("500 Internal Server Error", "pages index unavailable"),
                false => (
                    "200 OK",
                    r#"{"hits":[{"block_id":"block-1","page_id":"page-1","parent":"page-1","kind":"paragraph","text":"Tail paragraph after the list","height":1,"time":1}]}"#,
                ),
            };
            let response = format!(
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.shutdown().await;
        }
    });
    origin
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
    let data = search_pages(rpc, String::new(), "tail".into(), 7)
        .await
        .expect("a search the node answered must not fail on its title lookup");

    assert_eq!(data.generation, 7);
    assert_eq!(data.hits.len(), 1, "the hit the search returned survives");
    assert_eq!(data.hits[0].text, "Tail paragraph after the list");
    assert_eq!(data.hits[0].page_id, "page-1");
    assert_eq!(data.hits[0].block_id, "block-1");
    // Only the LABEL degrades, onto the same fallback an unresolvable page id
    // already takes in `titled_page_hits`.
    assert_eq!(data.hits[0].page_title, "Untitled");
}

/// EVERY CHAT READ RIDES THE VIEW LANE. `load_channel_row` is awaited INSIDE
/// the live stream's decoder fold, so a `/v1/query` there hands the node's
/// single select loop the fold of every subscriber: a channel row cost up to
/// the node's whole checkpoint write (issue #1018). `ChatViewQuery::Channel`
/// returns the identical `ChannelInfo` off an MVCC snapshot, off-loop.
///
/// Pinned as a source shape for the same reason `connect`'s cause is: the
/// difference between the two lanes is which HTTP route the round trip takes,
/// and both answer the same rows against a live node — a behavioural assertion
/// cannot tell them apart.
#[test]
fn chat_reads_never_cross_the_dispatch_query_lane() {
    // `load_channel_row` is the fold's caller; `load_channel_facts` is the read
    // itself, shared with the channel-switch window loader.
    const LIVE: &str = include_str!("live.rs");
    const LOAD: &str = include_str!("load.rs");
    let load_channel_row = LIVE
        .split("pub(crate) async fn load_channel_row(")
        .nth(1)
        .expect("load_channel_row is declared")
        .split("\npub ")
        .next()
        .expect("load_channel_row body");
    assert!(
        load_channel_row.contains("load_channel_facts("),
        "the channel row goes through the shared index-view read"
    );
    let load_channel_facts = LOAD
        .split("pub(crate) async fn load_channel_facts(")
        .nth(1)
        .expect("load_channel_facts is declared")
        .split("\npub ")
        .next()
        .expect("load_channel_facts body");
    assert!(
        load_channel_facts.contains("ChatViewQuery::Channel {"),
        "the channel row reads the index view arm"
    );
    for body in [load_channel_row, load_channel_facts] {
        assert!(
            !body.contains(".query("),
            "a chat read on /v1/query pays the node's checkpoint tax"
        );
    }

    // The whole crate, not just this function: `ChatQuery`/`ChatReply` are the
    // dispatch-lane types, and `backend/mod.rs` is the one `use` every backend
    // module inherits. An import reappearing IS a chat read crawling back onto
    // the select loop.
    // The `use` LINES, not the file: the comment above them names the banned
    // types to say why they are banned, and a sweep over raw source cannot
    // tell a symbol from the prose about it.
    const MOD: &str = include_str!("mod.rs");
    let imports: Vec<&str> = MOD
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("use "))
        .collect();
    assert!(
        !imports
            .iter()
            .any(|line| line.contains("ChatQuery") || line.contains("ChatReply")),
        "no backend module may reach for chat's dispatch query type: {imports:?}"
    );
    assert!(
        imports.iter().any(|line| line.contains("ChatViewQuery")),
        "the view lane's types are the ones the backend imports"
    );
}

/// THE TAB-SWITCH GATE. Four planes used to refetch on every tab move —
/// members, governance, agents, account — regardless of the destination, so a
/// click into Files paid four `/v1/query` round trips for rows nothing on
/// screen reads.
#[test]
fn a_tab_move_only_refetches_what_its_destination_draws() {
    // EVERY tab, taken from the rail itself plus the footer's Settings, so a
    // new seat lands in this sweep instead of quietly defaulting to "reads
    // nothing" behind a hand-written negative list.
    let mut tabs: Vec<String> = shell_nav("chat".into(), 0, false)
        .into_iter()
        .map(|seat| seat.id)
        .collect();
    tabs.push("settings".into());

    // the roster is drawn by five panes: its own, the admin gate under
    // Approvals, the forge write gate, the Node permissions, and the Settings
    // standing card. The rest are narrow: Settings draws the account card,
    // Forge the org "about", and proposals and agent rows belong to one pane
    // each.
    for (plane, drawn) in [
        (
            "members",
            &["forge", "node", "members", "governance", "settings"][..],
        ),
        ("governance", &["governance"][..]),
        ("agents", &["agents"][..]),
        ("account", &["forge", "settings"][..]),
        // an unknown plane name is nobody's — a typo must not silently reopen
        // the storm by answering true.
        ("explorer", &[][..]),
    ] {
        let readers: Vec<&str> = tabs
            .iter()
            .filter(|tab| tab_reads_plane((*tab).clone(), plane.into()))
            .map(String::as_str)
            .collect();
        assert_eq!(readers, drawn, "exactly these tabs draw {plane}");
    }
}

/// THE GATE IS ONLY WORTH ANYTHING IF THE LOADER HONOURS IT. `keep_i64` sends
/// an off-screen plane generation -1; each loader has to refuse it BEFORE any
/// I/O, which is what turns the gate into a skipped round trip rather than a
/// wasted one. `unreachable` is never contacted: reaching the client at all is
/// the failure this pins.
#[tokio::test(flavor = "current_thread")]
async fn an_off_screen_plane_is_refused_before_it_touches_the_node() {
    let unreachable = "http://127.0.0.1:9".to_string();
    let refusals = [
        load_members(unreachable.clone(), -1).await.err(),
        load_governance(unreachable.clone(), -1).await.err(),
        load_agents(unreachable.clone(), -1).await.err(),
        load_account(unreachable.clone(), -1).await.err(),
        // the DM directory is on the same -1 lane (lifecycle.ice's `identity`
        // live arm) and its `all{from:0,limit:256}` walk is the priciest of
        // the five — an ungated one fires on every chat post.
        load_dm_peers(unreachable.clone(), -1).await.err(),
        // the settings facts ride the -1 lane on the inline gate instead of a
        // plane row: they read the local user key, prefs and workspace dir.
        load_settings_facts(unreachable, -1).await.err(),
    ];
    for refusal in refusals {
        let refusal = refusal.expect("an off-screen load refuses");
        assert_eq!(refusal.generation, -1);
        assert_eq!(
            refusal.message, "skipped_offscreen",
            "the refusal must be the guard's, not a failed round trip's"
        );
    }
}

/// THE FORGE REPO LIST IS THE ONE UNSCOPED SLICE. Every other slice here is
/// keyed on what the forge pane has open. Off the forge tab this reloads
/// nothing, and reaching the (unreachable) node is what a lost gate looks like.
#[tokio::test(flavor = "current_thread")]
async fn a_forge_op_does_not_load_the_repo_list_for_a_closed_pane() {
    let data = forge_live_refresh(
        "http://127.0.0.1:9".into(),
        String::new(),
        0,
        "forge".into(),
        "forge".into(),
        ForgeRefresh::default(),
        false,
        4,
    )
    .await
    .expect("a closed forge pane loads nothing, so nothing can fail");

    assert_eq!(data.generation, 4);
    assert!(
        !data.repos_loaded,
        "an unloaded list must leave the handler's keep alone"
    );
    assert!(data.repos.is_empty());
}

