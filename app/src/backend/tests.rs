use ::chat;
use ::forge;
use ::node;

use commonware_cryptography::{Signer as _, ed25519};
use iced::futures::StreamExt as _;

use super::*;

#[test]
fn the_rail_seats_exactly_the_eight_module_screens() {
    let nav = shell_nav("chat".into(), 3, true);
    let ids: Vec<&str> = nav.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "chat",
            "pages",
            "forge",
            "agents",
            "files",
            "explorer",
            "members",
            "governance"
        ]
    );
    let forge = nav.iter().find(|item| item.id == "forge").unwrap();
    assert!(forge.live, "an engaged agent pulses the forge seat");
    assert!(!nav.iter().any(|item| item.id == "node"));
    assert_eq!(
        nav.iter()
            .find(|item| item.id == "governance")
            .unwrap()
            .badge,
        3
    );
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
    assert_eq!(members_summary(1, 0), "1 validator · 0 residents");
    assert_eq!(members_summary(3, 2), "3 validators · 2 residents");
    assert_eq!(tally_note(1, 4), "1 approval · 3 more for quorum");
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
    }];
    let listing = vec![
        channel(&dm_channel_id(a.clone(), b.clone())),
        channel("general"),
    ];
    let rooms = rooms_only(listing.clone(), peers.clone(), a.clone());
    assert_eq!(rooms.len(), 1);
    assert_eq!(rooms[0].id, "general");
    assert_eq!(rooms_only(listing, peers, String::new()).len(), 2);

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
        .map(|kind| FsEntry {
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
    assert_eq!(member_tier(Vec::new()), "guest");
    assert_eq!(filter_members(rows.clone(), "agents".into()).len(), 1);
    assert_eq!(filter_members(rows.clone(), "humans".into()).len(), 2);
    assert_eq!(filter_members(rows.clone(), "validators".into()).len(), 1);
    assert_eq!(filter_members(rows, "all".into()).len(), 3);
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
    assert_eq!(relative_time(UNMEASURED), "—");

    // and a real reading of zero is still a real reading: height 0 is the
    // genesis block, not an absence.
    assert_eq!(height_label(0), "h 0");
    // a record with no stamp keeps printing nothing — an em dash on every
    // unstamped row would be noise, and that is a different fact.
    assert_eq!(relative_time(0), "");
}

/// A record stamp is a BLOCK HEIGHT on this chain, so every record-time
/// string counts blocks. Only `/v1/status` supplies unix seconds.
#[test]
fn record_stamps_count_blocks_and_status_stamps_count_seconds() {
    assert_eq!(height_ago(84_500, 84_912), "412 blocks ago");
    assert_eq!(height_ago(84_911, 84_912), "1 block ago");
    assert_eq!(height_ago(84_912, 84_912), "this block");
    // a follower behind the record it is rendering still reads as now.
    assert_eq!(height_ago(84_913, 84_912), "this block");
    assert_eq!(height_ago(0, 84_912), "");
    assert_eq!(expires_in_blocks(85_324, 84_912), "expires in 412 blocks");
    assert_eq!(expires_in_blocks(84_913, 84_912), "expires in 1 block");
    assert_eq!(expires_in_blocks(84_912, 84_912), "expired");
    let now = now_seconds();
    assert_eq!(relative_time(now - 30), "just now");
    assert_eq!(relative_time(now - 40 * 60), "40m ago");
    assert_eq!(relative_time(now - 2 * 60 * 60), "2h ago");
    assert_eq!(relative_time(0), "");
}

/// The OTHER lane: a single-writer noded stamps `consensus_time` in unix
/// MILLIS, so the very same fields arrive thirteen digits wide. Rendering
/// them as heights printed `h 1,753,622,400,000` on every record.
#[test]
fn a_unix_millis_stamp_is_a_clock_not_a_thirteen_digit_height() {
    let two_hours_ago = (now_seconds() - 2 * 60 * 60) * 1_000;
    assert_eq!(height_label(two_hours_ago), "2h ago");
    assert_eq!(height_label_short(two_hours_ago), "2h ago");
    assert_eq!(height_ago(two_hours_ago, 84_912), "2h ago");
    assert_eq!(
        expires_in_blocks((now_seconds() + 3 * 60 * 60) * 1_000, 84_912),
        "expires in 3h"
    );
    assert_eq!(
        expires_in_blocks((now_seconds() - 60) * 1_000, 84_912),
        "expired"
    );
    // a real height is nowhere near the floor and still reads as one.
    assert_eq!(height_label(84_912), "h 84,912");
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
    let tokens = ducktape_ui::ui::theme::LIGHT;
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

#[test]
fn escape_ladder_names_the_topmost_transient_layer_only() {
    use iced::keyboard::{Key, key::Named};

    let escape = Key::Named(Named::Escape);
    let target = |palette: bool,
                  bell: bool,
                  create: bool,
                  thread_action: &str,
                  action: &str,
                  repo_menu: bool| {
        escape_target(
            escape.clone(),
            palette,
            bell,
            create,
            thread_action.into(),
            action.into(),
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
        ),
        ""
    );
    // An open palette swallows Escape — palette_key_action owns it.
    assert_eq!(target(true, true, true, "more", "more", true), "");
    // The ladder order is the z-order: bell over the create modal, menus
    // after both, thread menu over the stream's, popovers last.
    assert_eq!(target(false, true, true, "more", "more", true), "bell");
    assert_eq!(
        target(false, false, true, "more", "more", true),
        "channel_create"
    );
    assert_eq!(
        target(false, false, false, "more", "more", true),
        "thread_menu"
    );
    assert_eq!(
        target(false, false, false, "toolbar", "editing", true),
        "message_menu"
    );
    assert_eq!(
        target(false, false, false, "toolbar", "toolbar", true),
        "repo_menu"
    );
    // Nothing transient open → Escape is a no-op. The pages rungs are gone
    // with the menus they dismissed: the document has no transient layer.
    assert_eq!(target(false, false, false, "toolbar", "toolbar", false), "");
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
    assert_eq!(signing_input("secret", "00").unwrap(), b"secret\n00\n");
    assert!(signing_input("", "00").is_err());
    assert!(signing_input("bad\nsecret", "00").is_err());

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
    assert!(reply_settled_by(
        pending.clone(),
        settle.clone(),
        "general".into()
    ));
    assert_eq!(
        settled_reply_id(
            pending.clone(),
            settle.clone(),
            "general".into(),
            String::new()
        ),
        "reply-a"
    );
    // Each lane fires only on its own delta kind: a `reply` is the rail's
    // edge and never the stream's, and a `posted` is the reverse.
    assert!(!send_settled_by(
        pending.clone(),
        settle.clone(),
        "general".into()
    ));
    let mut as_post = settle.clone();
    as_post.kind = "posted".into();
    assert!(!reply_settled_by(
        pending.clone(),
        as_post,
        "general".into()
    ));
    assert!(!reply_settled_by(pending, settle, "other".into()));
}

#[test]
fn leaving_chat_prunes_scrollback_to_one_load() {
    let mut messages = Vec::new();
    for seq in 0..(CHAT_TIMELINE_ROOT_LIMIT + 50) {
        messages = optimistic_message(messages, format!("m{seq}"), format!("id-{seq}"));
    }
    // Staying on chat keeps the paged-in scrollback…
    let kept = trim_timeline_on_leave("chat".into(), messages.clone());
    assert_eq!(kept.len(), CHAT_TIMELINE_ROOT_LIMIT + 50);
    // …and leaving prunes to one load's worth, newest rows surviving.
    let trimmed = trim_timeline_on_leave("pages".into(), messages);
    assert_eq!(trimmed.len(), CHAT_TIMELINE_ROOT_LIMIT);
    assert_eq!(
        trimmed.last().unwrap().body,
        format!("m{}", CHAT_TIMELINE_ROOT_LIMIT + 49)
    );
    assert_eq!(trimmed.first().unwrap().body, "m50");
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

    assert!(send_settled_by(
        pending.clone(),
        settle.clone(),
        "general".into()
    ));
    assert_eq!(
        settled_send_id(pending.clone(), settle.clone(), "general".into(), "".into()),
        "message-a"
    );
    // Wrong channel, someone else's post, and a non-post delta all keep the
    // current anchor instead of flashing.
    assert!(!send_settled_by(
        pending.clone(),
        settle.clone(),
        "other".into()
    ));
    let mut foreign = settle.clone();
    foreign.message.id = "someone-else".into();
    assert!(!send_settled_by(pending.clone(), foreign, "general".into()));
    let mut reaction = settle;
    reaction.kind = "reaction".into();
    assert_eq!(
        settled_send_id(pending, reaction, "general".into(), "kept".into()),
        "kept"
    );
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
        mine: true,
        height: 0,
        time: 0,
        reactions: Vec::new(),
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
        mine: false,
        height: 0,
        time: 0,
        reactions: Vec::new(),
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
        mine: false,
        height: 0,
        time: 0,
        reactions: Vec::new(),
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
    let workspace = connect(origin.clone()).await.unwrap();
    let mut live = live_events(origin.clone());
    let ready = live.next().await.unwrap();
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
    let changed = live.next().await.unwrap();
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
    let hit = load_chat_hit(origin.clone(), "general".into(), 1, 3)
        .await
        .unwrap();
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
    let comments = refresh_block_comments(origin.clone(), "intro".into(), "thread-live".into(), 1)
        .await
        .unwrap();
    assert_eq!(comments.thread_id, "thread-live");
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
    let comments = refresh_block_comments(origin.clone(), "intro".into(), "thread-live".into(), 2)
        .await
        .unwrap();
    assert!(comments.thread_id.is_empty());
    assert!(comments.comments.is_empty());

    let refreshed = live_resync_load(
        origin,
        "general".into(),
        "welcome".into(),
        "both".into(),
        false,
        7,
        0,
    )
    .await
    .unwrap();
    assert_eq!(refreshed.generation, 7);
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
        mine: false,
        height: 0,
        time: 0,
        reactions: Vec::new(),
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

    // channel_is_unread: head past the seen cursor.
    assert!(channel_is_unread(reads.clone(), "random".into(), 50));
    assert!(!channel_is_unread(reads.clone(), "random".into(), 30));
    assert!(!channel_is_unread(reads.clone(), "general".into(), 100));

    // initial_channel_reads: seed absent channels to head, preserve existing.
    let seeded = initial_channel_reads(channels.clone(), vec![read("random", 30)]);
    assert_eq!(channel_last_read(seeded.clone(), "random".into()), 30);
    assert_eq!(channel_last_read(seeded.clone(), "general".into()), 100);
    assert!(!channel_is_unread(seeded, "general".into(), 100));

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

/// A bare mirror carrying one `main` commit, in the shape the browse
/// readers take it: a born branch they can resolve by default. A path with
/// one slash lands in a real subtree, which `mirror_commit`'s flat
/// treebuilder cannot express.
fn browsable_mirror(dir: &tempfile::TempDir, files: &[(&str, &str)]) -> git2::Repository {
    let mirror = git2::Repository::init_bare(dir.path()).unwrap();
    let mut root_files: Vec<(String, git2::Oid)> = Vec::new();
    let mut subtrees: BTreeMap<String, Vec<(String, git2::Oid)>> = BTreeMap::new();
    for (path, contents) in files {
        let blob = mirror.blob(contents.as_bytes()).unwrap();
        match path.split_once('/') {
            Some((directory, name)) => subtrees
                .entry(directory.to_string())
                .or_default()
                .push((name.to_string(), blob)),
            None => root_files.push(((*path).to_string(), blob)),
        }
    }
    // Every git2 handle below borrows `mirror`, so they live in a block:
    // the TreeBuilder is still alive at the end of the expression otherwise,
    // and the repository cannot be moved out to the caller.
    {
        let mut root = mirror.treebuilder(None).unwrap();
        for (name, blob) in root_files {
            root.insert(&name, blob, 0o100644).unwrap();
        }
        for (directory, entries) in subtrees {
            let mut sub = mirror.treebuilder(None).unwrap();
            for (name, blob) in entries {
                sub.insert(&name, blob, 0o100644).unwrap();
            }
            let oid = sub.write().unwrap();
            root.insert(&directory, oid, 0o040000).unwrap();
        }
        let tree = mirror.find_tree(root.write().unwrap()).unwrap();
        let signature = git2::Signature::now("mule", "mule@localhost").unwrap();
        let head = mirror
            .commit(None, &signature, &signature, "seed", &tree, &[])
            .unwrap();
        let commit = mirror.find_commit(head).unwrap();
        mirror.branch("main", &commit, true).unwrap();
    }
    mirror
}

#[test]
fn tree_listing_puts_directories_first_and_sizes_the_files() {
    let dir = tempfile::tempdir().unwrap();
    let mirror = browsable_mirror(
        &dir,
        &[
            ("zebra.rs", "fn main() {}\n"),
            ("src/lib.rs", "pub fn one() {}\n"),
            ("alpha.md", "# title\n"),
        ],
    );

    let root = read_tree(&mirror, "main", "").unwrap();
    let names: Vec<&str> = root.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(names, vec!["src", "alpha.md", "zebra.rs"]);
    assert_eq!(root[0].kind, "dir");
    assert_eq!(root[0].size, 0);
    assert_eq!(root[1].size, "# title\n".len() as i64);

    // an empty rev resolves to the default branch, and a nested path lists
    // that subtree with full paths.
    let nested = read_tree(&mirror, "", "src").unwrap();
    assert_eq!(nested.len(), 1);
    assert_eq!(nested[0].path, "src/lib.rs");
    assert_eq!(nested[0].kind, "file");
}

#[test]
fn blob_read_counts_lines_and_names_binary_content() {
    let dir = tempfile::tempdir().unwrap();
    let mirror = browsable_mirror(&dir, &[("a.txt", "one\ntwo\n"), ("bin.dat", "head\0tail")]);

    let text = read_blob(&mirror, "repo".into(), "main".into(), "a.txt".into(), 7).unwrap();
    assert_eq!(text.generation, 7);
    assert_eq!(text.lines, 2);
    assert!(!text.binary && !text.truncated);

    let binary = read_blob(&mirror, "repo".into(), "main".into(), "bin.dat".into(), 7).unwrap();
    assert!(binary.binary, "a NUL byte marks the blob binary");
    assert_eq!(binary.lines, 0);

    let missing = read_blob(&mirror, "repo".into(), "main".into(), "nope".into(), 7);
    assert!(
        missing.is_err(),
        "a path that is not there must not read empty"
    );
}

#[test]
fn about_skips_headings_and_badges_and_names_the_language() {
    let dir = tempfile::tempdir().unwrap();
    let mirror = browsable_mirror(
        &dir,
        &[
            (
                "README.md",
                "# ducktape\n\n[![badge](x)](y)\n\nThe consensus core.\nMore prose.\n",
            ),
            ("a.rs", "fn a() {}\n"),
            ("b.rs", "fn b() {}\n"),
            ("c.rs", "fn c() {}\n"),
        ],
    );
    let commit = mirror_commit_at(&mirror, "").unwrap();

    assert_eq!(readme_about(&mirror, &commit), "The consensus core.");
    assert_eq!(dominant_language(&commit), "Rust");
}

// An unborn repo has no head oid to resolve, so the card gets nothing —
// never a fabricated about line, language or stamp. The guard fires before
// any mirror is opened, so the unreachable endpoint below is never dialled.
#[test]
fn an_unborn_head_derives_no_card_facts() {
    assert_eq!(
        repo_card_facts("http://127.0.0.1:1", "core", "(unborn)"),
        (String::new(), String::new(), 0)
    );
}

#[test]
fn about_is_empty_without_a_readme_rather_than_invented() {
    let dir = tempfile::tempdir().unwrap();
    let mirror = browsable_mirror(&dir, &[("a.rs", "fn a() {}\n")]);
    let commit = mirror_commit_at(&mirror, "").unwrap();

    assert!(readme_about(&mirror, &commit).is_empty());
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
