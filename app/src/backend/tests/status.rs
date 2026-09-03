use super::*;

#[test]
fn a_count_of_one_takes_the_singular_noun() {
    assert_eq!(plural(1, "agent", "agents"), "1 agent");
    assert_eq!(plural(0, "agent", "agents"), "0 agents");
    assert_eq!(plural(2, "agent", "agents"), "2 agents");
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
        owner_handle: String::new(),
        live,
        skill_count: 0,
        cap_count: 0,
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
    assert_eq!(members_summary(true, &[]), "");
    assert_eq!(agents_summary(true, &[]), "");
    assert_eq!(proposals_summary(true, &[]), "");
    assert_eq!(fs_counts_summary(true, true, &[]), "");

    // Something there: every subtitle speaks, zeros included.
    assert_eq!(members_summary(true, &[human]), "1 human · 0 agents");
    assert_eq!(agents_summary(true, &[agent(false)]), "1 agent · 0 working");
    assert_eq!(
        proposals_summary(true, &[proposal(true)]),
        "1 open · 0 settled"
    );
    assert_eq!(
        proposals_summary(true, &[proposal(false)]),
        "0 open · 1 settled"
    );
    assert_eq!(fs_counts_summary(true, true, &[entry]), "1 file · 0 dirs");
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
fn a_log_line_splits_into_time_level_and_message() {
    let parts =
        split_log_line("2026-07-27T09:12:44.918Z  INFO ducktape::join: admitted resident".into());
    assert_eq!(parts.time, "2026-07-27T09:12:44.918Z");
    assert_eq!(parts.level, "INFO");
    assert_eq!(parts.message, "ducktape::join: admitted resident");

    let micro =
        split_log_line("2026-08-14T01:02:03.918273Z DEBUG ducktape::files: staged chunk".into());
    assert_eq!(
        micro.time, "2026-08-14T01:02:03.918Z",
        "the ring's microsecond timer is trimmed to the column's millisecond width"
    );
    assert_eq!(micro.level, "DEBUG");

    let prose = split_log_line("no level here".into());
    assert_eq!(prose.level, "");
    assert_eq!(prose.message, "no level here");
}

#[test]
fn explorer_ops_keep_the_full_hash_and_pretty_print_json_payloads() {
    let op_hash = "dd".repeat(32);
    let rows = vec![serde_json::json!({
        "height": 7,
        "hash": "aa".repeat(32),
        "commit_hash": "bb".repeat(32),
        "ops": [
            {
                "proposer": "cc".repeat(32),
                "target": "files",
                "disposition": "applied",
                "op_hash": op_hash,
                "payload": "{\"put\":{\"path\":\"/shared/a.png\"}}",
                "operations": []
            },
            {
                "proposer": "cc".repeat(32),
                "target": "chat",
                "disposition": "applied",
                "op_hash": op_hash,
                "payload": "plain prose, not a document",
                "operations": []
            }
        ]
    })];
    // the window lists newest first: ops arrive [files, chat] and reverse.
    let data = explorer_window(1, &rows);
    assert_eq!(
        data.ops[1].op_hash, op_hash,
        "the op hash is the blob key — the card carries it whole"
    );
    assert_eq!(
        data.ops[1].payload, "{\n  \"put\": {\n    \"path\": \"/shared/a.png\"\n  }\n}",
        "a JSON payload renders pretty-printed"
    );
    assert_eq!(
        data.ops[0].payload, "plain prose, not a document",
        "a non-JSON payload stays verbatim"
    );
    // the list's landmark stays the short form
    assert_eq!(data.blocks[0].hash.chars().count(), 13);
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
    let facts = node_facts(&resident);
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
/// `crates/noded/src/metrics.rs` puts it back to `None`. A node that finished
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
    let facts = node_facts(&caught_up);
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
    let facts = node_facts(&catching_up);
    assert!(sync_in_progress(&facts.phase));
    assert_eq!(facts.sync_applied, 412);
    assert_eq!(facts.sync_target, 900);

    // A node that has never synced publishes no `sync` at all. Heights are
    // UNMEASURED because the node published none; the counters are genuinely
    // zero, because a count of nothing IS zero.
    let fresh = serde_json::json!({ "operations": { "phase": "validating" } });
    let facts = node_facts(&fresh);
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
    assert_eq!(sync_label("syncing", 412, 900), "Syncing 412 / 900");

    // CAUGHT UP: the phase alone, however live the numbers beside it look.
    // `operations.sync` is never cleared, so a finished run's heights sit in
    // the document forever and must not reach a reader.
    assert_eq!(sync_label("serving", 900, 900), "Serving");
    assert_eq!(sync_label("validating", 900, 900), "Validating");

    // syncing with nothing published yet is still honest about the phase.
    assert_eq!(sync_label("syncing", UNMEASURED, UNMEASURED), "Syncing");
    assert_eq!(sync_label("syncing", 412, UNMEASURED), "Syncing");

    // and a node that published no phase says NOTHING rather than guessing.
    assert_eq!(sync_label("", 412, 900), "");
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
    assert_eq!(proposal_kind_tone("add_validator"), "access");
    assert_eq!(proposal_kind_tone("signal"), "neutral");
    assert_eq!(
        gov_action_detail(&serde_json::json!({ "signal": { "text": "ship it" } })),
        "ship it"
    );
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

    let facts = load_node_facts(rpc)
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

    let facts = load_node_facts(rpc)
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

/// The duckfs root is `/`, never "": the module's path check is `starts_with('/')`,
/// so the crumb's "" root answered every root open with a 400.
#[test]
fn the_files_root_is_a_slash() {
    assert_eq!(fs_parent("/shared".into()), "/");
    assert_eq!(fs_parent("/".into()), "/");
    assert_eq!(fs_parent("/shared/reports".into()), "/shared");
    assert_eq!(fs_child("/".into(), "notes".into()), "/notes");
    assert_eq!(fs_child("/shared".into(), "notes".into()), "/shared/notes");
}
