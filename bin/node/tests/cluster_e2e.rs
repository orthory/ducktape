//! real-socket cluster e2e: REAL `ducktape-node` OS processes over localhost
//! TCP, driven end to end through the json-lines rpc with TYPED payloads from
//! the `*-interface` crates (the drift that silently rotted the old bash demo
//! — a module rename plus a payload reshape — now fails to compile instead).
//!
//! `cluster_lifecycle` is the port of demo-2node.sh's assertion spec:
//!   1. genesis app-hashes agree            -> genesis determinism
//!   2. all validators converge             -> payload relay + live BFT
//!   3. converged hashes agree              -> no cross-process fork
//!   4. converged != genesis                -> ops actually applied
//!   5. chat posted via node 0 reads on 1   -> rpc -> consensus -> cross-apply
//!   6. governance admits a 4th key         -> member gating, votes, tally
//!   7. all validators cut over to epoch 1  -> live engine teardown + respawn
//!   8. post-cutover post reads on node 2   -> the epoch-1 engines finalize
//!   9. status app-hashes agree             -> the boundary a joiner rebuilds
//!  10. sync-only joiner hash parity        -> network statesync, full rebuild
//!
//! `quorum_tolerates_one_fault` covers what the demo never could: with 4
//! validators (quorum(4) = 3) the mesh keeps finalizing after one crash-kill —
//! 3-validator nets have zero slack, so this is the smallest real
//! fault-tolerance claim the stack can make.

mod common;

use std::time::Duration;

use chat_interface::{AuthorRef, Block, ChatMsg, ChatQuery, ChatReply, PostPolicy};
use common::{Cluster, poll_until, serial};
use directory_interface::{DirMsg, DirQuery, DirReply};
use governance_interface::{GovAction, GovMsg, GovQuery, GovReply, ProposalStatus};

/// convergence budget: mesh formation + leader rotation are real-time on a
/// possibly-loaded CI core; polls exit early, so generosity is free.
const CONVERGE: Duration = Duration::from_secs(180);
/// budget for one submitted op to finalize and become readable elsewhere.
const FINALIZE: Duration = Duration::from_secs(60);

fn chat_post(channel: &str, message_id: &str, text: &str) -> Vec<u8> {
    chat_interface::encode_msg(&ChatMsg::PostMessage {
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
        &chat_interface::encode_query(&ChatQuery::MessagesLatest {
            channel_id: channel.into(),
            limit: 64,
        }),
    )?;
    let ChatReply::Messages(views) = chat_interface::decode_reply(&reply).ok()? else {
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
        &governance_interface::encode_query(&GovQuery::Proposal {
            proposal_id: id.into(),
        }),
    )?;
    match governance_interface::decode_reply(&reply) {
        Ok(GovReply::Proposal(Some(view))) => Some((view.status, view.votes.len())),
        _ => None,
    }
}

fn dir_value(cluster: &Cluster, idx: usize, key: &str) -> Option<String> {
    let reply = cluster.query(
        idx,
        "directory",
        &directory_interface::encode_query(&DirQuery::Get { key: key.into() }),
    )?;
    match directory_interface::decode_reply(&reply) {
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

    // 2-4. convergence: each validator's startup op crossed the wire, every
    // process applied all three in agreed order, no fork, state advanced.
    let converged: Vec<String> = (0..3)
        .map(|i| cluster.wait_marker(i, "converged app_hash=", CONVERGE))
        .collect();
    assert_eq!(
        converged[0], converged[1],
        "cross-process fork at convergence"
    );
    assert_eq!(
        converged[0], converged[2],
        "cross-process fork at convergence"
    );
    assert_ne!(converged[0], genesis[0], "converged but nothing applied");

    // 5. the rpc product loop: post chat via node 0, read it on node 1 —
    // rpc ingress -> ordered lane -> finalization -> cross-node apply -> query.
    cluster.submit(
        0,
        "chat",
        &chat_interface::encode_msg(&ChatMsg::CreateChannel {
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
        &governance_interface::encode_msg(&GovMsg::Propose {
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
    let vote = governance_interface::encode_msg(&GovMsg::Vote {
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
        &governance_interface::encode_msg(&GovMsg::Execute {
            proposal_id: "admit-node3".into(),
        }),
    );
    poll_until("proposal to settle as Passed", FINALIZE, || {
        proposal_status(&cluster, 0, "admit-node3").filter(|(s, _)| *s == ProposalStatus::Passed)
    });

    // 7. LIVE EPOCH CUTOVER: the valset change schedules a cutover at
    // observed_view + CUTOVER_DELAY; finalized views only advance with ops,
    // so push fillers until every validator respawns onto the 4-member set.
    // fillers go through the raw rpc and tolerate rejection — an op caught
    // mid-teardown dies with its epoch's content store by design.
    let mut filler = 0u32;
    let mut last_filler = std::time::Instant::now() - Duration::from_secs(1);
    poll_until("all validators to cut over to epoch 1", CONVERGE, || {
        if last_filler.elapsed() >= Duration::from_secs(1) {
            last_filler = std::time::Instant::now();
            filler += 1;
            let payload = directory_interface::encode_msg(&DirMsg::Set {
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
            "payload": { "Set": { "key": "via-app-surface", "value": "held" } },
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
    let boundary = status0["app_hash"]
        .as_str()
        .expect("status carries app_hash");

    // 10. the sync-only joiner rebuilds EVERY module over the statesync
    // channel from node 0 and must compose the identical app-hash.
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
        &directory_interface::encode_msg(&DirMsg::Set {
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
