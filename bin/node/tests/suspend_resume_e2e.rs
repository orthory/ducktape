//! a suspended follower resumes following, end to end: freeze the resident
//! (SIGSTOP) for longer than the mesh read/write deadline, thaw it, and
//! require a post finalized AFTER the thaw to reach its local reads within a
//! bounded window.
//!
//! this is the laptop-sleep regression net for the desktop node. a slept
//! machine leaves exactly this shape behind: the frozen node goes silent, so
//! the peer's next read runs into `MESH_IO_TIMEOUT` (15s) and tears the
//! connection down; on wake the follower holds half-open sockets and must
//! heal through teardown → redial → catch-up. the 25s freeze is deliberately
//! ABOVE the 15s deadline so the founder-side teardown really happens — under
//! the old 60s default the founder would still be holding the dead-quiet
//! connection when the resident thaws. (linux cannot fake the one macOS-only
//! aggravation — `CLOCK_UPTIME_RAW` pausing across sleep, which makes the
//! wakened node itself burn its full residual deadline — so the bound on that
//! side is the constant itself, asserted where it is defined.)
//!
//! run alone (cluster e2es flake under parallel load):
//!   cargo test -p node-bin --test suspend_resume_e2e -- --nocapture --test-threads=1

mod common;

use std::time::{Duration, Instant};

use chat::{
    Block, ChatMsg, ChatQuery, ChatReply, PostPolicy, decode_reply, encode_msg, encode_query,
};
use common::{NetworkShapeCluster, poll_until, serial};

/// generous like the sibling legs: standing → follow-arm sync → first
/// pre-synced boundary is several blocks of slack.
const CONVERGE: Duration = Duration::from_secs(180);

/// freeze long enough that the founder's 15s read deadline fires mid-freeze
/// and tears the resident's connections down — the slept-laptop shape.
const FREEZE: Duration = Duration::from_secs(25);

/// the thaw-to-caught-up bound. healing is deadline-driven: the resident's
/// own dead reads fail fast (its sockets got FIN/RST while frozen), the
/// dialer re-dials at 500ms cadence, and the follow loop backfills. 45s is
/// several times the worst honest path (one 15s residual deadline + redial +
/// backfill) while staying far from the minutes-long stall this test exists
/// to catch.
const RECOVER: Duration = Duration::from_secs(45);

#[test]
fn a_suspended_resident_resumes_following_within_the_deadline_budget() {
    let _serial = serial();
    let mut cluster = NetworkShapeCluster::new();

    let chain_id = cluster.init_founder("suspend-resume");
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
    let (ok, out) = cluster.run_membership_verb("invite-accept", &friend_key);
    assert!(ok, "invite-accept failed:\n{out}");
    cluster.wait_admitted(1, CONVERGE);
    cluster.wait_marker(1, "resident: pre-synced boundary", CONVERGE);

    // prove the follow loop is live before the freeze.
    cluster.submit(0, "chat", &encode_msg(&post("m-live", "pre-freeze post")));
    resident_sees(&cluster, "m-live", "the pre-freeze adoption", CONVERGE);

    // ---- sleep the laptop: freeze the resident while the chain advances.
    cluster.signal(1, "STOP");
    std::thread::sleep(FREEZE);
    cluster.signal(1, "CONT");

    // ---- the point: a post finalized only after the thaw must land in the
    // resident's local reads inside the deadline-driven healing budget.
    let thawed = Instant::now();
    cluster.submit(0, "chat", &encode_msg(&post("m-thaw", "post-thaw post")));
    resident_sees(&cluster, "m-thaw", "the post-thaw adoption", RECOVER);
    println!(
        "resident healed and adopted the post-thaw boundary in {:?}",
        thawed.elapsed()
    );
    // NOTE: this used to also insist the heal went through the
    // fresh-boundary re-sync — the manifest-proxy retention floor refused a
    // ~25-frame gap even though the journal still held it. the floor is
    // honest now (the journal's own first retained block), so a freeze this
    // short heals by DIRECT frame catch-up and no re-sync is needed. the
    // RangePruned branch keeps its pins where the gap is real: the recovery
    // contract test (range_read_refuses_below_the_retained_floor) and
    // busy_chain_ascension_e2e's restart against a genuinely-outrun window.
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
            &encode_query(&ChatQuery::MessagesLatest {
                channel_id: "general".into(),
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
