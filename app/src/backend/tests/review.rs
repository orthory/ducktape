use super::*;

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
