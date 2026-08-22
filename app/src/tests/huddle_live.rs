//! THE LIVE HUDDLE LANE — ignored by default, because it needs a live node,
//! a second person, and a camera this box may not have.
//!
//! Every other test of the huddle stops at a boundary: the unit suites run
//! pure folds, and the node's `huddle_media_e2e` drives two real nodes with a
//! client written for the test. This one is the app's OWN leg —
//! `backend::join_huddle`, `call::call_session`, its roster poll, its ws pump,
//! its capture thread, its decode store — run as ONE SIDE of a two-sided
//! huddle. The other side is another process (another machine, in the real
//! thing) doing the same.
//!
//! ONE PROCESS IS ONE PERSON, which is the whole reason this is not a
//! two-session test: identity is process-global (`DUCKTAPE_HOME`/
//! `DUCKTAPE_USER_KEY` name ONE user key), and it is the roster row matching
//! that key that a session filters itself out by. Two sessions in one process
//! are one person twice, which is not the arrangement that broke.
//!
//! `ops/huddle-lane.sh` stands the network up and prints the two commands.
//! Each side runs:
//!
//! ```text
//! DUCKTAPE_HOME=<side dir> DUCKTAPE_NODE=http://127.0.0.1:<http port> \
//! DUCKTAPE_HUDDLE_PASSWORD=<key password> DUCKTAPE_HUDDLE_CHANNEL=eng \
//! cargo test -p ducktape-app -- --ignored --nocapture huddle_live
//! ```

use std::time::Duration;

use iced::futures::StreamExt as _;

/// The huddle is a meeting: this side waits for the other one to show up,
/// join, and start publishing. Generous on purpose — a person is slower than
/// a test.
const MEETING: Duration = Duration::from_secs(120);
/// One roster read per second, the same cadence the session's own poll runs.
const ROSTER_READ: Duration = Duration::from_secs(1);
/// How long this side keeps publishing after it is satisfied, so the other
/// side — a second behind at worst — still has somebody to see.
const COURTESY: Duration = Duration::from_secs(15);

fn required(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| {
        panic!("{name} must be set — see this module's doc, or ops/huddle-lane.sh")
    })
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "needs a live node, a second side, and a camera; see the module doc"]
async fn this_side_hears_and_sees_the_other_through_the_apps_own_leg() {
    let node = required("DUCKTAPE_NODE");
    let password = required("DUCKTAPE_HUDDLE_PASSWORD");
    let channel = std::env::var("DUCKTAPE_HUDDLE_CHANNEL").unwrap_or_else(|_| "eng".into());

    // Join the way the button joins: this device's user key signs, and the
    // node key it stamps is the one its own `/v1/status` publishes.
    crate::backend::join_huddle(node.clone(), password, channel.clone())
        .await
        .expect("this side joins the huddle");

    // Wait for the other side, by the only fact that says they are here: the
    // fan-out this device would steer to names exactly them. (A chain read is
    // the event; there is no push for "somebody joined a room you are in".)
    let deadline = std::time::Instant::now() + MEETING;
    let peer = loop {
        let fanout = crate::backend::huddle_fanout_nodes(&node, &channel)
            .await
            .expect("the node serves the huddle roster");
        if let [peer] = &fanout[..] {
            break peer.clone();
        }
        assert!(
            std::time::Instant::now() < deadline,
            "no second side joined {channel} within {MEETING:?}; the roster reads {fanout:?}"
        );
        tokio::time::sleep(ROSTER_READ).await;
    };

    // The camera is this side's picture. A box with no camera turns the
    // toggle back off and says why on the status line — which is what the
    // failure below prints.
    assert!(
        crate::video::call_use_camera(true).camera,
        "the camera toggle must take"
    );

    let joined_at = std::time::Instant::now();
    let mut events = crate::call::call_session(node, channel);
    let mut seen_peer = false;
    let mut note = String::new();
    let watch = async {
        loop {
            // Every event is also a chance to ask the store whether their
            // picture landed: frames arrive on a blocking decode task, not on
            // this stream, and the 1 Hz beacons keep this loop turning.
            let Some(event) = events.next().await else {
                panic!("the session ended before the huddle worked ({note})");
            };
            if !event.message.is_empty() {
                note = format!("{}: {}", event.kind, event.message);
            }
            seen_peer |= event.peer == peer;
            if !seen_peer {
                continue;
            }
            let Some((width, height, _)) = crate::video::stage_frame(&peer) else {
                continue;
            };
            // Program output, not logging: the lane is run by hand and the
            // numbers ARE the result — a picture that is 640×480 came off the
            // far camera, not out of an empty store.
            println!(
                "peer {peer} beaconed and their picture is here: {width}x{height}, \
                 {:?} after this side joined",
                joined_at.elapsed()
            );
            return;
        }
    };
    tokio::time::timeout(MEETING, watch)
        .await
        .unwrap_or_else(|_| {
            panic!(
                "the other side must arrive and be seen — beacon: {seen_peer}, \
                 picture: {} (last note: {note})",
                crate::video::stage_frame(&peer).is_some(),
            )
        });

    // STAY IN THE ROOM. Leaving the instant this side is satisfied takes this
    // camera with it, and the other side — which may be a second behind — then
    // waits for a picture nobody is sending any more. A real participant does
    // not hang up the moment they can see you.
    let _ = tokio::time::timeout(COURTESY, async { while events.next().await.is_some() {} }).await;
}
