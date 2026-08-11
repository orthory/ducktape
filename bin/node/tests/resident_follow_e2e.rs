//! resident head-follow, end to end on the network-shape cluster: once a
//! resident serves a boundary, it adopts the NEXT one wake-driven — the
//! cert-lane nudge — never by waiting out the fallback poll.
//!
//! the discriminator is phase math, not luck: completing a boundary adoption
//! re-arms the serve window's fallback tick, so at the moment an adoption is
//! OBSERVED the next fallback-driven manifest fetch is a full
//! `RESIDENT_FALLBACK_POLL` (12s) away — minus only the probe loop's small
//! observation lag. a post finalized right after that observation can
//! therefore only appear in the resident's LOCAL reads within the 8s window
//! if the cert-lane wake fired. two consecutive legs, each re-anchored by the
//! previous adoption, make a lucky phase impossible rather than unlikely.
//!
//! run alone (cluster e2es flake under parallel load):
//!   cargo test -p node-bin --test resident_follow_e2e -- --nocapture --test-threads=1

mod common;

use std::time::Duration;

use chat::{
    Block, ChatMsg, ChatQuery, ChatReply, PostPolicy, decode_reply, encode_msg, encode_query,
};
use common::{NetworkShapeCluster, poll_until, serial};

/// generous like the sibling legs: standing → follow-arm sync → first
/// pre-synced boundary is several blocks of slack.
const CONVERGE: Duration = Duration::from_secs(180);

/// the wake proof window: must hold finalize (~one 2s block) + the resident's
/// delta re-sync, and must sit clearly under the 12s fallback the adoption
/// just re-armed. 8s leaves ~2x headroom on both sides.
const WAKE_WINDOW: Duration = Duration::from_secs(8);

#[test]
fn resident_adopts_boundaries_on_the_cert_wake_not_the_fallback_poll() {
    let _serial = serial();
    let mut cluster = NetworkShapeCluster::new();

    let chain_id = cluster.init_founder("resident-follow");
    assert!(!chain_id.is_empty(), "init should print the founded chain id");
    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", Duration::from_secs(60));

    // an Open room so the founder's later posts need no chat membership.
    cluster.submit(
        0,
        "chat",
        &encode_msg(&ChatMsg::CreateChannel {
            channel_id: "general".into(),
            name: "general".into(),
            post_policy: PostPolicy::Open,
        }),
    );
    poll_until("the channel to finalize on the founder", CONVERGE, || {
        let raw = cluster.query(
            0,
            "chat",
            &encode_query(&ChatQuery::Channel {
                channel_id: "general".into(),
            }),
        )?;
        matches!(decode_reply(&raw).ok()?, ChatReply::Channel(Some(_))).then_some(())
    });

    // invite + join a fresh identity; grant it RESIDENT standing and wait for
    // the first pre-synced boundary — the follow loop is live from here on.
    let invite = cluster.invite();
    let friend_key = cluster.join_friend_manual(&invite);
    assert_eq!(friend_key.len(), 64, "join prints the friend's pubkey hex");
    cluster.spawn(1);
    cluster.wait_marker(1, "joining:", Duration::from_secs(60));
    let (ok, out) = cluster.run_membership_verb("resident accept", &friend_key);
    assert!(ok, "resident accept failed:\n{out}");
    cluster.wait_admitted(1, CONVERGE);
    cluster.wait_marker(1, "resident: pre-synced boundary", CONVERGE);

    // ---- arm the phase: one post under the GENEROUS deadline. observing it
    // in the resident's local reads means an adoption just completed, so the
    // fallback tick just re-armed — the wake is now the only sub-12s path.
    cluster.submit(0, "chat", &encode_msg(&post("m-arm", "arm the phase")));
    resident_sees(&cluster, "m-arm", "the arming post", CONVERGE);

    // ---- the point, twice: each leg's deadline opens at the moment the
    // previous adoption was observed, when the fallback is a known 12s away.
    cluster.submit(0, "chat", &encode_msg(&post("m-follow-1", "first wake leg")));
    resident_sees(&cluster, "m-follow-1", "the first wake-driven adoption", WAKE_WINDOW);

    cluster.submit(0, "chat", &encode_msg(&post("m-follow-2", "second wake leg")));
    resident_sees(&cluster, "m-follow-2", "the second wake-driven adoption", WAKE_WINDOW);

    cluster.kill(1);
    cluster.kill(0);
}

/// poll the RESIDENT's own read surface (node 1 — reads serve from its
/// pre-synced host, never a validator's) until `message_id` is visible.
fn resident_sees(cluster: &NetworkShapeCluster, message_id: &str, what: &str, deadline: Duration) {
    poll_until(what, deadline, || {
        let raw = cluster.query(
            1,
            "chat",
            &encode_query(&ChatQuery::MessagesRange {
                channel_id: "general".into(),
                from_seq: 1,
                limit: 10,
            }),
        )?;
        let ChatReply::Messages(views) = decode_reply(&raw).ok()? else {
            return None;
        };
        views
            .into_iter()
            .any(|v| v.head.message_id == message_id)
            .then_some(())
    });
}

/// an Open-channel post to `general` with a caller-chosen message id.
fn post(id: &str, text: &str) -> ChatMsg {
    ChatMsg::PostMessage {
        channel_id: "general".into(),
        message_id: id.into(),
        blocks: vec![Block::paragraph(text)],
        thread: None,
        as_agent: None,
    }
}
