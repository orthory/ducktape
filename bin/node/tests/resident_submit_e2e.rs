//! resident submit relay, end to end on the network-shape cluster: a parked
//! joiner cannot write; once granted RESIDENT standing it posts to chat
//! through its OWN surface — the frame relays to the founder, finalizes, and
//! the recorded author is the RESIDENT's key (authorship rides the frame
//! signature, not the injecting validator). a member-gated module op from the
//! same resident finalizes Rejected — the relay grants no authority.
//!
//! run alone (cluster e2es flake under parallel load):
//!   cargo test -p node-bin --test resident_submit_e2e -- --nocapture --test-threads=1

mod common;

use std::time::Duration;

use chat::{
    AuthorRef, Block, ChatMsg, ChatQuery, ChatReply, PostPolicy, decode_reply, encode_msg,
    encode_query,
};
use common::{NetworkShapeCluster, poll_until, serial};

/// generous like the sibling live-admission legs: standing → follow-arm sync →
/// first pre-synced boundary is several blocks of slack.
const CONVERGE: Duration = Duration::from_secs(180);

#[test]
fn resident_posts_to_chat_with_its_own_authorship() {
    let _serial = serial();
    let mut cluster = NetworkShapeCluster::new();

    let chain_id = cluster.init_founder("resident-submit");
    assert!(!chain_id.is_empty(), "init should print the founded chain id");
    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", Duration::from_secs(60));

    // the founder opens the room BEFORE the friend even exists — policy Open,
    // so posting needs no chat membership, only authenticated authorship.
    cluster.submit(
        0,
        "chat",
        &encode_msg(&ChatMsg::CreateChannel {
            channel_id: "general".into(),
            name: "general".into(),
            post_policy: PostPolicy::Open,
        }),
    );
    // the create is only ACCEPTED above; wait for it to FINALIZE so the later
    // relayed post can never race an un-created channel (a missing channel
    // would come back Rejected and mask the authorship assertion).
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

    // invite + join a fresh identity, spawn it; it parks with NO standing.
    let invite = cluster.invite();
    let friend_key = cluster.join_friend_manual(&invite);
    assert_eq!(friend_key.len(), 64, "join prints the friend's pubkey hex");
    cluster.spawn(1);
    cluster.wait_marker(1, "joining:", Duration::from_secs(60));

    // (1) WHILE JOINING (no standing): a write is refused, and the refusal
    //     names the no-standing contract — refused for the RIGHT reason.
    let refused = cluster.rpc(
        1,
        serde_json::json!({
            "cmd": "submit",
            "target": "chat",
            "payload_hex": common::hex(&encode_msg(&post("m-parked", "too early"))),
        }),
    );
    assert_eq!(
        refused["ok"], false,
        "a joining node (no standing) must refuse writes: {refused}"
    );
    assert!(
        refused["error"]
            .as_str()
            .unwrap_or_default()
            .contains("joining"),
        "the refusal names the joining/no-standing contract: {refused}"
    );

    // grant RESIDENT standing (invite-accept = AddResident), then wait for the
    // follow arm to grant standing AND pre-sync a boundary — the write gate
    // needs both (serving is Some only after the first boundary).
    let (ok, out) = cluster.run_membership_verb("invite-accept", &friend_key);
    assert!(ok, "invite-accept failed:\n{out}");
    cluster.wait_marker(1, "resident: standing granted", CONVERGE);
    cluster.wait_marker(1, "resident: pre-synced boundary", CONVERGE);

    // (2) THE POINT: the resident posts through its OWN surface and the reply is
    //     the relayed op's consensus fate (ok == Applied — relay → validator →
    //     finalize).
    let posted = cluster.rpc(
        1,
        serde_json::json!({
            "cmd": "submit",
            "target": "chat",
            "payload_hex": common::hex(&encode_msg(&post("m-resident", "hi from the cheap seats"))),
        }),
    );
    assert_eq!(
        posted["ok"], true,
        "the resident submit should relay + finalize (ok == Applied): {posted}"
    );

    // (3) the founder's view of the message carries the RESIDENT's authorship —
    //     authorship rides the frame signature, not the injecting validator.
    let author = poll_until(
        "the relayed post to finalize into the founder's channel",
        CONVERGE,
        || {
            let raw = cluster.query(
                0,
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
                .find(|v| v.head.message_id == "m-resident")
                .map(|v| v.head.author)
        },
    );
    assert_eq!(
        author,
        AuthorRef::User(common::unhex(&friend_key)),
        "authorship is the resident's key, not the injecting validator's"
    );

    // (4) NO AUTHORITY ESCALATION: a member-gated governance op from the
    //     resident finalizes Rejected (deterministic no-op), and the relay
    //     reply says so — the relay grants no membership authority.
    let gov = cluster.rpc(
        1,
        serde_json::json!({
            "cmd": "submit",
            "target": "governance",
            // a syntactically-valid proposal from a NON-MEMBER origin: the
            // governance module rejects it deterministically at execute time
            // (the proposer must be a current validator-set member).
            "payload_hex": common::hex(&governance_probe()),
        }),
    );
    assert_eq!(
        gov["ok"], false,
        "a member-gated op from a non-member resident must not apply: {gov}"
    );
    assert!(
        gov["error"]
            .as_str()
            .unwrap_or_default()
            .contains("rejected"),
        "the op finalized and was deterministically Rejected (not refused at the door): {gov}"
    );

    cluster.kill(1);
    cluster.kill(0);
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

/// a well-formed but member-gated governance op: `Propose { AddResident }`.
/// the key is a valid 32-byte length (so it clears the door), and the proposer
/// (the resident's origin) is not a validator-set member — so it finalizes
/// Rejected.
fn governance_probe() -> Vec<u8> {
    use governance::{GovAction, GovMsg, encode_msg};
    encode_msg(&GovMsg::Propose {
        proposal_id: "resident-escalation-probe:0".into(),
        action: GovAction::AddResident {
            key: vec![0xAA; 32],
        },
        voting_period: 1_000,
    })
}
