//! real-socket cluster e2e: REAL `ducktape` OS processes over localhost
//! TCP, driven end to end through the json-lines rpc with TYPED payloads from
//! the modules' crate-root wire types (the drift that silently rotted the old bash demo
//! — a module rename plus a payload reshape — now fails to compile instead).
//!
//! `cluster_lifecycle` is the port of demo-2node.sh's assertion spec:
//!   1. genesis app-hashes agree            -> genesis determinism
//!   2. all validators converge             -> payload relay + live BFT
//!   3. converged hashes agree              -> no cross-process fork
//!   4. converged != genesis                -> ops actually applied
//!   5. chat posted via node 0 reads on 1   -> rpc -> consensus -> cross-apply
//!   6. governance admits a 4th key         -> member gating, votes, tally
//!   7. admission cutover                   -> the passed proposal seats the
//!      key at one boundary; node 3 syncs the frozen boundary and promotes
//!   8. post-cutover post reads on node 2   -> the respawned engines finalize
//!   9. status app-hashes agree             -> the boundary a joiner rebuilds
//!  10. sync-only joiner hash parity        -> network statesync, full rebuild
//!
//! `quorum_tolerates_one_fault` covers what the demo never could: with 4
//! validators (quorum(4) = 3) the mesh keeps finalizing after one crash-kill —
//! 3-validator nets have zero slack, so this is the smallest real
//! fault-tolerance claim the stack can make.

mod common;

use std::time::Duration;

use chat::{AuthorRef, Block, ChatMsg, ChatQuery, ChatReply, PostPolicy};
use common::{Cluster, poll_until, serial};
use directory::{DirMsg, DirQuery, DirReply};
use governance::{GovAction, GovMsg, GovQuery, GovReply, ProposalStatus};

/// convergence budget: mesh formation + leader rotation are real-time on a
/// possibly-loaded CI core; polls exit early, so generosity is free.
const CONVERGE: Duration = Duration::from_secs(180);
/// budget for one submitted op to finalize and become readable elsewhere.
const FINALIZE: Duration = Duration::from_secs(60);

fn chat_post(channel: &str, message_id: &str, text: &str) -> Vec<u8> {
    chat::encode_msg(&ChatMsg::PostMessage {
        channel_id: channel.into(),
        message_id: message_id.into(),
        blocks: vec![Block::paragraph(text)],
        thread: None,
        as_agent: None,
    })
}

/// query `channel`'s latest messages on `idx` and return the plain text of
/// the message whose id is `message_id`, with its author.
fn read_message(
    cluster: &Cluster,
    idx: usize,
    channel: &str,
    message_id: &str,
) -> Option<(String, AuthorRef)> {
    let reply = cluster.query(
        idx,
        "chat",
        &chat::encode_query(&ChatQuery::MessagesLatest {
            channel_id: channel.into(),
            limit: 64,
        }),
    )?;
    let ChatReply::Messages(views) = chat::decode_reply(&reply).ok()? else {
        return None;
    };
    views.into_iter().find_map(|v| {
        (v.head.message_id == message_id).then(|| {
            let text = v
                .head
                .blocks
                .iter()
                .map(|b| match b {
                    Block::Paragraph(spans) | Block::Quote(spans) => {
                        spans.iter().map(|s| s.text.as_str()).collect::<String>()
                    }
                    Block::Code { text, .. } => text.clone(),
                    Block::Divider => String::new(),
                })
                .collect::<String>();
            (text, v.head.author)
        })
    })
}

fn proposal_status(cluster: &Cluster, idx: usize, id: &str) -> Option<(ProposalStatus, usize)> {
    let reply = cluster.query(
        idx,
        "governance",
        &governance::encode_query(&GovQuery::Proposal {
            proposal_id: id.into(),
        }),
    )?;
    match governance::decode_reply(&reply) {
        Ok(GovReply::Proposal(Some(view))) => Some((view.status, view.votes.len())),
        _ => None,
    }
}

fn dir_value(cluster: &Cluster, idx: usize, key: &str) -> Option<String> {
    let reply = cluster.query(
        idx,
        "directory",
        &directory::encode_query(&DirQuery::Get { key: key.into() }),
    )?;
    match directory::decode_reply(&reply) {
        Ok(DirReply::Value(v)) => v,
        Err(_) => None,
    }
}

#[test]
fn cluster_lifecycle() {
    let _serial = serial();
    // mesh of 4 (node 3 is the future joiner), consensus subset of 3.
    let mut cluster = Cluster::new(&[0, 1, 2, 3], &[0, 1, 2]);

    // bootstrapper first — everyone else dials it.
    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", Duration::from_secs(60));
    cluster.spawn(1);
    cluster.spawn(2);

    // 1. genesis determinism: identical module registry -> identical app-hash.
    let genesis: Vec<String> = (0..3)
        .map(|i| cluster.wait_marker(i, "genesis app_hash=", Duration::from_secs(60)))
        .collect();
    assert_eq!(genesis[0], genesis[1], "genesis fork between nodes 0 and 1");
    assert_eq!(genesis[0], genesis[2], "genesis fork between nodes 0 and 2");

    // 2. convergence (LIVENESS): each validator's startup op crossed the
    // wire and every process applied at least all three. the marker samples
    // its hash at a node-local drain-batch boundary, so with concurrent
    // startup traffic (capability announces on a host with an executor CLI
    // installed) the three lines can legitimately sample different heights —
    // per the harness contract it proves liveness, never hash equality.
    let converged: Vec<String> = (0..3)
        .map(|i| cluster.wait_marker(i, "converged app_hash=", CONVERGE))
        .collect();

    // 3. no cross-process fork: the state assertion goes through the rpc
    // (the harness-documented pattern) — poll until every validator reports
    // the same status app-hash. a real fork never reconciles, so it fails
    // this poll's budget; sampling skew settles within a block or two.
    poll_until("status app-hashes to agree across validators", FINALIZE, || {
        let hashes: Vec<serde_json::Value> = (0..3)
            .map(|i| cluster.status(i)["app_hash"].clone())
            .collect();
        (!hashes[0].is_null() && hashes[0] == hashes[1] && hashes[0] == hashes[2]).then_some(())
    });

    // 4. ops actually applied: every node's own sampled hash moved off its
    // (agreed) genesis hash.
    for (i, c) in converged.iter().enumerate() {
        assert_ne!(*c, genesis[i], "node {i} converged but nothing applied");
    }

    // 5. the rpc product loop: post chat via node 0, read it on node 1 —
    // rpc ingress -> ordered lane -> finalization -> cross-node apply -> query.
    cluster.submit(
        0,
        "chat",
        &chat::encode_msg(&ChatMsg::CreateChannel {
            channel_id: "general".into(),
            name: "General".into(),
            post_policy: PostPolicy::Open,
        }),
    );
    cluster.submit(0, "chat", &chat_post("general", "m1", "hello ducktape"));
    let (text, author) = poll_until("chat post to finalize on node 1", FINALIZE, || {
        read_message(&cluster, 1, "general", "m1")
    });
    assert_eq!(text, "hello ducktape");
    // authorship is derived from the VERIFIED frame origin — node 0 signed it.
    assert_eq!(author, AuthorRef::User(Cluster::identity(0)));

    // 6. governance: node 0 proposes admitting node 3's key, nodes 0+1 vote
    // yes (2 of 3 = strict majority), node 1 executes; the passing proposal
    // emits the valset Join follow-up governance alone is authorized to make.
    cluster.submit(
        0,
        "governance",
        &governance::encode_msg(&GovMsg::Propose {
            proposal_id: "admit-node3".into(),
            action: GovAction::AddValidator {
                key: Cluster::identity(3),
            },
            voting_period: 600_000, // consensus-time ms; far past test end
        }),
    );
    poll_until("proposal to open on node 1", FINALIZE, || {
        proposal_status(&cluster, 1, "admit-node3").filter(|(s, _)| *s == ProposalStatus::Open)
    });
    let vote = governance::encode_msg(&GovMsg::Vote {
        proposal_id: "admit-node3".into(),
        approve: true,
    });
    cluster.submit(0, "governance", &vote);
    cluster.submit(1, "governance", &vote);
    poll_until("both ballots to land", FINALIZE, || {
        proposal_status(&cluster, 1, "admit-node3").filter(|(_, votes)| *votes == 2)
    });
    cluster.submit(
        1,
        "governance",
        &governance::encode_msg(&GovMsg::Execute {
            proposal_id: "admit-node3".into(),
        }),
    );
    poll_until("proposal to settle as Passed", FINALIZE, || {
        proposal_status(&cluster, 0, "admit-node3").filter(|(s, _)| *s == ProposalStatus::Passed)
    });

    // the passed proposal seats node 3 directly: cutover #1 (epoch 1)
    // widens the quorum to 4. node 3 boots as a parked joiner, sees itself
    // in the participant set at the boundary, syncs, and promotes.
    cluster.spawn(3);
    cluster.wait_marker(3, "joiner mode:", Duration::from_secs(60));

    // 7. ADMISSION CUTOVER. finalized views only advance with ops, so push
    // fillers through the boundary. fillers go through the raw rpc and
    // tolerate rejection — an op caught mid-teardown dies with its epoch's
    // content store by design.
    let mut filler = 0u32;
    let mut last_filler = std::time::Instant::now() - Duration::from_secs(1);
    poll_until("the admission cutover to cross", CONVERGE, || {
        if last_filler.elapsed() >= Duration::from_secs(1) {
            last_filler = std::time::Instant::now();
            filler += 1;
            let payload = directory::encode_msg(&DirMsg::Set {
                key: format!("cutover-filler-{filler}"),
                value: "x".into(),
            });
            let _ = cluster.rpc(
                0,
                serde_json::json!({
                    "cmd": "submit",
                    "target": "directory",
                    "payload_hex": common::hex(&payload),
                }),
            );
        }
        (0..3)
            .all(|i| cluster.marker(i, "cutover complete: epoch 1").is_some())
            .then_some(())
    });
    // the admitted key sees itself in the epoch-1 participant set and
    // promotes through the normal restore path.
    cluster.wait_marker(3, "promoted: validator at epoch 1", CONVERGE);

    // 8. the epoch-1 engines must still finalize: post through the respawned
    // net via node 0, read on node 2.
    cluster.submit(0, "chat", &chat_post("general", "m2", "epoch one lives"));
    let (text, _) = poll_until("post-cutover chat post on node 2", FINALIZE, || {
        read_message(&cluster, 2, "general", "m2")
    });
    assert_eq!(text, "epoch one lives");

    // 8b. app integration on the NETWORKED node: the validator serves the
    // noded /v1 wire itself, and a submit reply is HELD until the op's frame
    // drains at a finalized boundary — so the block summary that comes back is
    // already-applied consensus state, readable from any other validator.
    let (code, block) = cluster.http(
        0,
        "POST",
        "/v1/submit",
        Some(&serde_json::json!({
            "target": "directory",
            "payload": { "set": { "key": "via-app-surface", "value": "held" } },
        })),
    );
    assert_eq!(code, 200, "app-surface submit failed: {block}");
    assert!(
        block["height"].as_u64().is_some_and(|h| h > 0),
        "held submit must reply with the finalized block: {block}"
    );
    for reader in [1, 2] {
        let value = poll_until("app-surface op readable via rpc", FINALIZE, || {
            dir_value(&cluster, reader, "via-app-surface")
        });
        assert_eq!(value, "held", "node {reader} read a wrong value");
    }

    // 8c. the explorer surface: the held submit finalized a NON-EMPTY block,
    // so /v1/blocks must carry it — frame hash, per-block commit app-hash,
    // the submitting validator's VERIFIED key as proposer, and the dispatch
    // trace — while the heartbeat nops that tick the idle chain never appear.
    let (code, body) = cluster.http(0, "GET", "/v1/blocks", None);
    assert_eq!(code, 200, "explorer blocks fetch failed: {body}");
    let records = body["blocks"].as_array().expect("blocks is an array");
    assert!(
        records
            .iter()
            .all(|b| b["ops"][0]["target"] != "consensus.nop"),
        "heartbeat nops must never reach the explorer: {body}"
    );
    let submitted = records
        .iter()
        .find(|b| b["height"] == block["height"])
        .unwrap_or_else(|| panic!("held submit's block missing from the explorer: {body}"));
    // the held submit is its block's member op; a block carries its ops under
    // `ops[]` now.
    let op = &submitted["ops"][0];
    assert_eq!(op["target"], "directory");
    assert_eq!(op["disposition"], "applied");
    assert_eq!(
        submitted["commitHash"], block["appHash"],
        "explorer commit hash must equal the held reply's app-hash"
    );
    assert_eq!(
        op["proposer"].as_str().unwrap_or_default(),
        common::hex(&Cluster::identity(0)),
        "proposer is node 0's verified signer key"
    );
    assert_eq!(
        submitted["hash"].as_str().map(str::len),
        Some(64),
        "the frame content hash is 64 hex chars"
    );
    assert!(
        op["operations"]
            .as_array()
            .is_some_and(|ops| !ops.is_empty()),
        "an applied op carries its dispatch trace: {submitted}"
    );
    // 8c-bis. the metrics plane: the drain that applied the held submit folded
    // it into the validator's `ducktape_*` Prometheus series BEFORE the held
    // reply returned, so /metrics must already report a recorded block apply —
    // count, apply-latency histogram samples, and the dispatch counter labeled
    // with the submitted module. (this endpoint served only commonware runtime
    // series until the validator learned to record its own blocks.)
    let (code, exposition) = cluster.http_text(0, "/metrics");
    assert_eq!(code, 200, "metrics scrape failed:\n{exposition}");
    let blocks_total = exposition
        .lines()
        .find(|l| l.starts_with("ducktape_blocks_total"))
        .and_then(|l| l.split_whitespace().last())
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or_else(|| panic!("no ducktape_blocks_total sample:\n{exposition}"));
    assert!(
        blocks_total >= 1.0,
        "the applied block was recorded: ducktape_blocks_total={blocks_total}"
    );
    assert!(
        exposition
            .lines()
            .any(|l| l.starts_with("ducktape_block_apply_latency_seconds_count")
                && l.split_whitespace().last() != Some("0")),
        "the apply-latency histogram observed the block:\n{exposition}"
    );
    assert!(
        exposition.contains("ducktape_dispatch_total")
            && exposition.contains("module=\"directory\""),
        "the dispatch counter carries the submitted module label:\n{exposition}"
    );
    for sample in [
        r#"ducktape_node_phase{role="validator",phase="validating"} 1"#,
        "ducktape_consensus_validators ",
        "ducktape_consensus_quorum ",
        "ducktape_consensus_reachable_validators ",
        "ducktape_consensus_pending_ops ",
        "ducktape_last_finalized_timestamp_seconds ",
        r#"ducktape_ops_outcome_total{outcome="applied"}"#,
        r#"ducktape_index_height{module="directory"}"#,
    ] {
        assert!(
            exposition.contains(sample),
            "stable operational sample {sample:?} missing:\n{exposition}"
        );
    }
    let metric_value = |name: &str| {
        exposition
            .lines()
            .find(|line| line.starts_with(name))
            .and_then(|line| line.split_whitespace().last())
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or_else(|| panic!("missing numeric {name}:\n{exposition}"))
    };
    assert!(
        metric_value("ducktape_consensus_reachable_validators")
            >= metric_value("ducktape_consensus_quorum"),
        "a finalizing validator reports quorum reachability:\n{exposition}"
    );
    // 8d. the record's op hash is a real content address: staging at the
    // drain keys the committed payload bytes by sha256, so the blob lane
    // must serve the exact submitted payload back under that digest.
    let op_hash = submitted["ops"][0]["opHash"].as_str().unwrap_or_default();
    assert_eq!(
        op_hash.len(),
        64,
        "explorer record carries the op's content address: {submitted}"
    );
    let (code, blob) = cluster.http(0, "GET", &format!("/v1/files/blob/{op_hash}"), None);
    assert_eq!(code, 200, "op hash must dereference on the blob lane: {blob}");
    assert_eq!(
        blob,
        serde_json::json!({ "set": { "key": "via-app-surface", "value": "held" } }),
        "blob lane serves the committed payload bytes back"
    );

    // 9. quiesce, then the boundary every joiner must rebuild: identical
    // status app-hashes across validators — and the app surface reports the
    // same hash as the rpc (one state, two wires). both node-2 reads happen
    // AFTER the quiesce with nothing left in flight, so a mismatch means the
    // two wires project different host state, not a straggling block.
    std::thread::sleep(Duration::from_secs(2));
    let status0 = cluster.status(0);
    let status1 = cluster.status(1);
    assert_eq!(
        status0["app_hash"], status1["app_hash"],
        "post-rpc status app-hashes disagree"
    );
    let (code, http_status) = cluster.http(2, "GET", "/v1/status", None);
    assert_eq!(code, 200, "app-surface status failed");
    let http_hash = http_status["appHash"].as_str().unwrap_or_default();
    assert!(
        !http_hash.is_empty(),
        "app-surface status carries appHash: {http_status}"
    );
    assert_eq!(
        cluster.status(2)["app_hash"].as_str().unwrap_or_default(),
        http_hash,
        "the app surface and the rpc disagree on node 2's app-hash"
    );
    assert_eq!(http_status["operations"]["role"], "validator");
    assert_eq!(http_status["operations"]["phase"], "validating");
    assert!(
        http_status["operations"]["consensus"]["validators"]
            .as_u64()
            .is_some_and(|validators| validators >= 3),
        "status carries the current consensus set: {http_status}"
    );
    assert!(
        http_status["operations"]["consensus"]["reachableValidators"]
            .as_u64()
            .zip(http_status["operations"]["consensus"]["quorum"].as_u64())
            .is_some_and(|(reachable, quorum)| reachable >= quorum),
        "status reports quorum reachability: {http_status}"
    );
    assert!(
        http_status["operations"]["storage"]["indexes"]
            .as_array()
            .is_some_and(|indexes| indexes.iter().any(|index| index["module"] == "directory")),
        "status carries bounded index watermarks: {http_status}"
    );
    let boundary = status0["app_hash"]
        .as_str()
        .expect("status carries app_hash");

    // 10. the sync-only joiner rebuilds EVERY module over the statesync
    // channel from node 0 and must compose the identical app-hash. node 3's
    // slot is reused as a FRESH resident: kill the promoted validator
    // (quorum(4) = 3 keeps the network live — nops move heights, not state,
    // so the step-9 boundary hash stands) and wipe its state.
    cluster.kill(3);
    cluster.wipe_storage(3);
    let (ok, log) = cluster.run_sync_only(3, Duration::from_secs(120));
    assert!(ok, "sync-only joiner failed:\n{log}");
    let synced = log
        .lines()
        .find_map(|l| l.split("synced app_hash=").nth(1))
        .expect("joiner printed a synced app-hash")
        .trim();
    assert_eq!(synced, boundary, "joiner rebuilt a DIFFERENT app-hash");
}

#[test]
fn quorum_tolerates_one_fault() {
    let _serial = serial();
    // 4 running validators: quorum(4) = 3, so ONE crash keeps liveness.
    let mut cluster = Cluster::new(&[0, 1, 2, 3], &[0, 1, 2, 3]);
    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", Duration::from_secs(60));
    for i in 1..4 {
        cluster.spawn(i);
    }
    for i in 0..4 {
        cluster.wait_marker(i, "converged app_hash=", CONVERGE);
    }

    // crash-kill one validator; the other three still form a quorum.
    cluster.kill(3);
    cluster.submit(
        0,
        "directory",
        &directory::encode_msg(&DirMsg::Set {
            key: "after-fault".into(),
            value: "alive".into(),
        }),
    );
    for reader in [1, 2] {
        let value = poll_until("post-fault op to finalize", CONVERGE, || {
            dir_value(&cluster, reader, "after-fault")
        });
        assert_eq!(value, "alive", "node {reader} read a wrong value");
    }
}

/// the staged reachability plane must converge a mesh on a FRESH boot: both
/// nodes fire their boot `Retarget` (and the initial `EndpointRecord` send it
/// triggers) before the p2p actors have any live connection, so the plane's
/// liveness may not depend on that first datagram surviving. fake effect —
/// the protocol (records -> adverts -> verified mesh -> handshakes -> apply)
/// is identical to the real path right up to the interface call, and two
/// same-host nodes with the real effect would fight over one dt-* name.
#[test]
fn reachability_plane_converges_mesh_on_boot() {
    let _serial = serial();
    let mut cluster = Cluster::new(&[0, 1], &[0, 1]);
    cluster.wireguard = true;
    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", Duration::from_secs(60));
    cluster.spawn(1);
    for i in 0..2 {
        cluster.wait_marker(i, "mesh verified", Duration::from_secs(60));
        cluster.wait_marker(i, "tunnels applied (config accepted", Duration::from_secs(60));
    }
}
