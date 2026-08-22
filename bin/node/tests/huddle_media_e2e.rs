//! The huddle media leg on TWO REAL NODES: real processes, a real overlay,
//! the real `/v1/call/ws`, and a fan-out set read from consensus exactly as
//! the desktop app's roster poll reads it.
//!
//! THE BUG THIS IS THE E2E OF. Two people joined a huddle and neither heard
//! or saw the other. The fan-out set a call session is steered to is BOTH the
//! send list and the receive admission roster, and a session that opened
//! alone was steered to nobody: it sent to no one and refused everything. The
//! only thing that used to re-steer it was a peer's beacon — which that same
//! admission dropped. So the late joiner was a person nobody could hear, and
//! the huddle stayed that way for as long as it lasted.
//!
//! What this test pins, in order:
//!   1. the node key vocabulary is ONE — `/v1/status`.public_key (what the
//!      app stamps into `JoinHuddle`) is the validator signer's key (what the
//!      hub admits by). A mismatch here is the same silence with no bug in
//!      sight.
//!   2. the deadlock is real and observable AT THE NODE: with A steered to an
//!      empty set, B's media is refused, counted as `rogue_datagrams` on A's
//!      voice plane.
//!   3. the re-steer is the whole cure: A sends the roster it now reads from
//!      the chain and nothing else changes — no reconnect, no rejoin, no
//!      beacon — and B's voice, camera and presence all arrive.
//!   4. it crosses the other way too, so "my mic doesn't work" and "I can't
//!      see them" are both answered.

mod common;

use std::time::Duration;

use chat::call_wire::{self, CapturedFrame};
use chat::{Channel, ChatMsg, ChatQuery, ChatReply, PostPolicy};
use common::{Cluster, hex, poll_until, serial, unhex};
use futures::{SinkExt as _, StreamExt as _};
use tokio::net::TcpStream;
use tokio::runtime::Runtime;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

/// mesh formation + overlay handshake on a possibly-loaded box; polls exit
/// early, so generosity is free.
const READY: Duration = Duration::from_secs(180);
/// budget for one submitted op to finalize and read back elsewhere.
const FINALIZE: Duration = Duration::from_secs(60);
/// budget for media to cross once the fan-out set is right: the voice plane's
/// own first-contact handshake plus a few 20 ms frames.
const CROSSES: Duration = Duration::from_secs(45);

const CHANNEL: &str = "eng";

type CallSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;
type CallSink = futures::stream::SplitSink<CallSocket, Message>;
type CallStream = futures::stream::SplitStream<CallSocket>;

/// This node's mesh identity as `/v1/status` publishes it — read off the SAME
/// app surface, by the same route, that `backend::join_huddle` reads to fill
/// `JoinHuddle.node`.
fn status_key(cluster: &Cluster, idx: usize) -> String {
    let (status, body) = cluster.http(idx, "GET", "/v1/status", None);
    assert_eq!(status, 200, "node {idx} status: {body}");
    body["public_key"]
        .as_str()
        .expect("the node publishes its mesh identity")
        .to_string()
}

/// The channel record as consensus holds it — the same row the app's channel
/// view projects the huddle roster from.
fn channel_record(cluster: &Cluster, idx: usize, channel_id: &str) -> Option<Channel> {
    let reply = cluster.query(
        idx,
        "chat",
        &chat::encode_query(&ChatQuery::Channel {
            channel_id: channel_id.into(),
        }),
    )?;
    let ChatReply::Channel(found) = chat::decode_reply(&reply).ok()? else {
        return None;
    };
    found
}

/// Every node key on the huddle roster, in join order.
fn roster_nodes(cluster: &Cluster, idx: usize, channel_id: &str) -> Vec<String> {
    channel_record(cluster, idx, channel_id)
        .map(|channel| channel.huddle.iter().map(|m| hex(&m.node)).collect())
        .unwrap_or_default()
}

/// The fan-out set THE APP WOULD COMPUTE on this node: the roster's node keys
/// minus this device's own (`backend::huddle_recipient_nodes`). Ours in the
/// set would aim this device's media at itself; the peer's missing from it is
/// the silence.
fn fanout(cluster: &Cluster, idx: usize, channel_id: &str, me: &str) -> Vec<String> {
    roster_nodes(cluster, idx, channel_id)
        .into_iter()
        .filter(|node| node != me)
        .collect()
}

/// Datagrams node `idx`'s VOICE plane threw away because the sender was not
/// admitted: `rogue_datagrams` (a live flow, sender not in its roster) and
/// `unregistered_datagrams` (no flow at all) are the same fact from the
/// sender's side — nothing this node will ever hand up. The deadlock with a
/// number on it.
fn refused_voice_datagrams(cluster: &Cluster, idx: usize) -> u64 {
    const REFUSALS: [&str; 2] = [
        r#"kind="rogue_datagrams""#,
        r#"kind="unregistered_datagrams""#,
    ];
    let (status, body) = cluster.http_text(idx, "/metrics");
    assert_eq!(status, 200, "metrics exposition: {body}");
    body.lines()
        .filter(|line| line.starts_with("ducktape_dataplane_drops{"))
        .filter(|line| line.contains(r#"service="voice""#))
        .filter(|line| REFUSALS.iter().any(|kind| line.contains(kind)))
        .filter_map(|line| line.rsplit(' ').next()?.parse::<u64>().ok())
        .sum()
}

async fn open_call(base: &str, channel_id: &str) -> CallSocket {
    let url = format!(
        "{}/v1/call/ws?channel={channel_id}",
        base.replacen("http://", "ws://", 1)
    );
    let (socket, _) = tokio_tungstenite::connect_async(&url)
        .await
        .expect("the call socket upgrades");
    socket
}

fn recipients_frame(peers: &[String]) -> Message {
    Message::Text(serde_json::json!({ "type": "recipients", "peers": peers }).to_string())
}

fn beacon_frame() -> Message {
    Message::Text(
        serde_json::json!({
            "type": "beacon",
            "muted": false,
            "camera_on": true,
            "sharing": false,
        })
        .to_string(),
    )
}

/// A 500 Hz square, not a constant frame: SILK's high-pass strips DC, so a
/// steady level would decode to converged silence at the far end.
fn loud_frame() -> Vec<i16> {
    (0..chat::voice::FRAME_SAMPLES)
        .map(|i| if (i / 48) % 2 == 0 { 8000 } else { -8000 })
        .collect()
}

/// Position-dependent fill, so a reassembly regression breaks exact equality
/// instead of hiding behind uniform bytes.
fn camera_frame(seed: u8) -> Vec<u8> {
    (0..5000)
        .map(|i| ((i * 7 + usize::from(seed)) % 251) as u8)
        .collect()
}

/// What one leg publishes for as long as the test holds the handle: a mic
/// frame every 20 ms, a keyframe every 100 ms, and the 1 Hz presence beacon —
/// the same three things the app's session pumps.
async fn publish(mut out: CallSink, video: Vec<u8>) {
    let mut audio = tokio::time::interval(Duration::from_millis(20));
    let mut camera = tokio::time::interval(Duration::from_millis(100));
    let mut beacon = tokio::time::interval(Duration::from_secs(1));
    let mut ts: u32 = 0;
    loop {
        let frame = tokio::select! {
            _ = audio.tick() => Message::Binary(call_wire::encode_audio(&loud_frame())),
            _ = camera.tick() => {
                ts += 100;
                Message::Binary(call_wire::encode_captured(&CapturedFrame {
                    keyframe: true,
                    ts_ms: ts,
                    data: video.clone(),
                }))
            }
            _ = beacon.tick() => beacon_frame(),
        };
        if out.send(frame).await.is_err() {
            return;
        }
    }
}

/// Everything one participant must observe of another before the huddle is
/// worth the name: their voice in the mix, their camera frame whole, and
/// their presence on the control leg.
async fn hear_and_see(inbound: &mut CallStream, peer: &str, video: &[u8]) {
    let (mut heard, mut seen, mut present) = (false, false, false);
    while let Some(Ok(message)) = inbound.next().await {
        match message {
            Message::Binary(bytes) => {
                if let Some(pcm) = call_wire::decode_audio(&bytes) {
                    heard |= pcm.iter().any(|sample| sample.abs() > 1000);
                } else if let Some(frame) = call_wire::decode_peer(&bytes) {
                    assert_eq!(hex(&frame.peer), peer, "a frame from someone else");
                    assert_eq!(frame.data, video, "the camera frame must cross whole");
                    seen = true;
                }
            }
            Message::Text(text) => {
                let control: serde_json::Value =
                    serde_json::from_str(&text).expect("server control is json");
                // The hub beacons a peer from the state it last heard, so the
                // first one can still carry the pre-beacon default. What is
                // being waited on is the peer's OWN state arriving.
                let named = control["type"] == "peer_beacon" && control["peer"] == peer;
                present |= named && control["camera_on"] == true;
            }
            _ => {}
        }
        if heard && seen && present {
            return;
        }
    }
    panic!(
        "the call socket closed before {peer} was heard ({heard}), seen ({seen}) and present ({present})"
    );
}

#[test]
fn a_late_joiner_is_heard_and_seen_once_the_roster_re_steers_the_fan_out() {
    let _serial = serial();
    let rt = Runtime::new().expect("runtime");
    let mut cluster = Cluster::new(&[0, 1], &[0, 1]);
    // media rides the OVERLAY and nothing else: with no `wireguard_listen`
    // the node refuses to wire a call hub at all.
    cluster.wireguard = true;
    for idx in 0..2 {
        cluster.spawn(idx);
    }
    for idx in 0..2 {
        cluster.wait_marker(idx, "rpc listening on", READY);
        cluster.wait_marker(idx, "converged root_hash=", READY);
        cluster.wait_marker(idx, "peer handshake COMPLETE", READY);
    }

    // ONE KEY VOCABULARY. The app stamps `/v1/status`.public_key into
    // `JoinHuddle`; the hub admits by the validator signer's key. If those
    // ever diverge the roster names nobody the media plane knows, and the
    // symptom is exactly the silence below with no bug in sight.
    let node_a = status_key(&cluster, 0);
    let node_b = status_key(&cluster, 1);
    assert_eq!(node_a, hex(&Cluster::identity(0)));
    assert_eq!(node_b, hex(&Cluster::identity(1)));
    assert_ne!(node_a, node_b);

    cluster.submit(
        0,
        "chat",
        &chat::encode_msg(&ChatMsg::CreateChannel {
            channel_id: CHANNEL.into(),
            name: "Engineering".into(),
            post_policy: PostPolicy::Open,
        }),
    );
    cluster.submit(
        0,
        "chat",
        &chat::encode_msg(&ChatMsg::JoinHuddle {
            channel_id: CHANNEL.into(),
            node: unhex(&node_a),
        }),
    );
    poll_until("A alone in the huddle", FINALIZE, || {
        (roster_nodes(&cluster, 0, CHANNEL) == [node_a.clone()]).then_some(())
    });

    // A's session opens while A is alone, and steers to what the roster says:
    // nobody. This is the state the old app then stayed in forever.
    let mut leg_a = rt.block_on(open_call(&cluster.http_base(0), CHANNEL));
    let alone = fanout(&cluster, 0, CHANNEL, &node_a);
    assert!(alone.is_empty(), "A joined an empty huddle: {alone:?}");
    rt.block_on(leg_a.send(recipients_frame(&alone)))
        .expect("A's session takes its fan-out");

    // B joins the huddle LATER — the whole bug is in that word.
    cluster.submit(
        1,
        "chat",
        &chat::encode_msg(&ChatMsg::JoinHuddle {
            channel_id: CHANNEL.into(),
            node: unhex(&node_b),
        }),
    );
    for idx in 0..2 {
        poll_until("both nodes read a two-person roster", FINALIZE, || {
            let roster = roster_nodes(&cluster, idx, CHANNEL);
            (roster == [node_a.clone(), node_b.clone()]).then_some(())
        });
    }

    let leg_b = rt.block_on(open_call(&cluster.http_base(1), CHANNEL));
    let (mut b_out, mut b_in) = leg_b.split();
    let b_sees = fanout(&cluster, 1, CHANNEL, &node_b);
    assert_eq!(
        b_sees,
        std::slice::from_ref(&node_a),
        "the newcomer's roster names A"
    );
    rt.block_on(b_out.send(recipients_frame(&b_sees)))
        .expect("B's session takes its fan-out");

    // B talks and shows a camera from here to the end of the test.
    let b_video = camera_frame(3);
    let b_pump = rt.spawn(publish(b_out, b_video.clone()));

    // THE DEADLOCK, COUNTED. A's voice plane refuses B's datagrams because
    // admission is gated on the set A was steered to, and B is not in it.
    // This is a refusal the node itself reports — not an absence we waited
    // out.
    poll_until("node A refuses the late joiner's media", CROSSES, || {
        (refused_voice_datagrams(&cluster, 0) > 0).then_some(())
    });

    // THE CURE, AND NOTHING ELSE: A re-reads the roster from consensus and
    // steers to it. No reconnect, no rejoin, no beacon — the one message the
    // session's 1 s poll sends when the roster moves.
    let steered = fanout(&cluster, 0, CHANNEL, &node_a);
    assert_eq!(
        steered,
        std::slice::from_ref(&node_b),
        "A's poll now names B"
    );
    let (mut a_out, mut a_in) = leg_a.split();
    rt.block_on(a_out.send(recipients_frame(&steered)))
        .expect("A's session takes the re-steer");

    rt.block_on(async {
        tokio::time::timeout(CROSSES, hear_and_see(&mut a_in, &node_b, &b_video))
            .await
            .expect("B's voice, camera and presence must reach A once the fan-out names B");
    });

    // ...and the other way, which is the half a one-sided fix would leave
    // broken: "my mic doesn't work" and "I can't see them" are one bug.
    let a_video = camera_frame(11);
    let a_pump = rt.spawn(publish(a_out, a_video.clone()));
    rt.block_on(async {
        tokio::time::timeout(CROSSES, hear_and_see(&mut b_in, &node_a, &a_video))
            .await
            .expect("A's voice, camera and presence must reach B");
    });

    a_pump.abort();
    b_pump.abort();
}
