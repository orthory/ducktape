//! a network that survives reboots — the real-socket proof for restart
//! recovery. one OS-process solo validator (a network of one: simplex with
//! participants = {self} is live) is crashed with SIGKILL and later shut down
//! gracefully; each respawn over the SAME storage dir must recover the
//! composed app-hash WITHOUT re-running genesis, resume the finalized
//! boundary, and keep finalizing new ops over its reopened consensus journal
//! (the anti-equivocation record makes a fresh-journal respawn unsafe, so
//! liveness after respawn is the property that proves the journal reopened).

mod common;

use std::time::Duration;

use common::{Cluster, poll_until};
use directory_interface::{DirMsg, DirQuery, DirReply, decode_reply, encode_msg, encode_query};

fn dir_set(key: &str, value: &str) -> Vec<u8> {
    encode_msg(&DirMsg::Set {
        key: key.into(),
        value: value.into(),
    })
}

fn dir_value(cluster: &Cluster, idx: usize, key: &str) -> Option<String> {
    let reply = cluster.query(
        idx,
        "directory",
        &encode_query(&DirQuery::Get { key: key.into() }),
    )?;
    match decode_reply(&reply).ok()? {
        DirReply::Value(v) => v,
    }
}

/// submit a directory write via rpc and wait until it is readable back — the
/// end-to-end "the engine is live and finalizing" probe.
fn write_and_confirm(cluster: &Cluster, idx: usize, key: &str, value: &str) {
    cluster.submit(idx, "directory", &dir_set(key, value));
    poll_until(
        &format!("directory {key}={value} to finalize"),
        Duration::from_secs(30),
        || (dir_value(cluster, idx, key).as_deref() == Some(value)).then_some(()),
    );
}

#[test]
fn solo_validator_survives_crash_and_graceful_restart() {
    let _guard = common::serial();
    // a network of ONE: the sharpest recovery case — no peer holds the state,
    // so local recovery is the only path back.
    let mut cluster = Cluster::new(&[0], &[0]);
    cluster.spawn(0);
    cluster.wait_marker(0, "genesis app_hash=", Duration::from_secs(30));

    // real state across two of the substrates the checkpoint has to cover:
    // directory is in-memory canonical-bytes (dies without recovery).
    write_and_confirm(&cluster, 0, "who", "ducktape");
    write_and_confirm(&cluster, 0, "where", "a-worktree");
    let before = cluster.status(0);
    let app_hash_before = before["app_hash"]
        .as_str()
        .expect("status app_hash")
        .to_string();
    let height_before = before["height"].as_u64().expect("status height");

    // ---- crash: SIGKILL, no goodbye ------------------------------------
    cluster.kill(0);

    // respawn over the SAME storage dir. the node must RECOVER, not re-run
    // genesis: the greppable marker flips from `genesis app_hash=` to
    // `recovered app_hash=`.
    cluster.spawn(0);
    let recovered = cluster.wait_marker(0, "recovered app_hash=", Duration::from_secs(30));
    let recovered_hash = recovered.split_whitespace().next().expect("recovered hash");
    assert_eq!(
        recovered_hash, app_hash_before,
        "recovered app-hash must be byte-identical to the pre-crash boundary"
    );
    assert!(
        cluster.marker(0, "genesis app_hash=").is_none(),
        "a restart must not re-run genesis"
    );

    // the recovered boundary answers status identically... (height is
    // monotone, not frozen: idle views finalize empty proposals that drain
    // as deterministic rejections, so a view may seal between the status
    // read and the kill — state-identical, height +n.)
    let after = poll_until("status after recovery", Duration::from_secs(30), || {
        let s = cluster.status(0);
        (s["app_hash"] == before["app_hash"]).then_some(s)
    });
    assert!(after["height"].as_u64().expect("height") >= height_before);
    assert_eq!(dir_value(&cluster, 0, "who").as_deref(), Some("ducktape"));
    assert_eq!(
        dir_value(&cluster, 0, "where").as_deref(),
        Some("a-worktree")
    );

    // ...and the engine is LIVE again over its reopened journal: new ops
    // finalize (a same-epoch respawn that lost its vote state would refuse
    // or double-vote; one that never resumed would park this forever).
    write_and_confirm(&cluster, 0, "after-crash", "still-here");

    // ---- graceful: rpc shutdown (final checkpoint + journal barrier) ----
    let reply = cluster.rpc(0, serde_json::json!({ "cmd": "shutdown" }));
    assert_eq!(reply["ok"], true, "shutdown rpc failed: {reply}");
    cluster.wait_exit(0, Duration::from_secs(15));

    cluster.spawn(0);
    cluster.wait_marker(0, "recovered app_hash=", Duration::from_secs(30));
    assert!(
        cluster.marker(0, "genesis app_hash=").is_none(),
        "the second restart must not re-run genesis either"
    );
    // everything from BOTH earlier lives is present...
    poll_until(
        "post-shutdown state to answer",
        Duration::from_secs(30),
        || (dir_value(&cluster, 0, "after-crash").as_deref() == Some("still-here")).then_some(()),
    );
    assert_eq!(dir_value(&cluster, 0, "who").as_deref(), Some("ducktape"));
    // ...and the network keeps running as scheduled.
    write_and_confirm(&cluster, 0, "after-shutdown", "and-again");
}

/// the SIGTERM/SIGINT graceful-checkpoint arm — the fix's ACTUAL production
/// trigger. the desktop shell SIGTERMs the daemon on app quit; before the fix
/// that tore the node down mid-block and could brick a solo genesis node (the
/// disk substrate left ahead of the last in-memory checkpoint). now a SIGTERM
/// is one more branch of the run loop that runs the SAME graceful sequence as an
/// rpc `Shutdown` (a final manifest + journal barrier) and exits 0. this drives
/// it over a REAL OS signal on a REAL process: SIGTERM a live solo node, assert
/// it logs the signal arm and exits on its own, restart, and assert recovery to
/// the byte-identical tip with no brick. (restart_e2e is pre-existing-flaky
/// under load, so the timeouts are deliberately generous.)
#[test]
fn solo_validator_survives_sigterm_restart() {
    let _guard = common::serial();
    let mut cluster = Cluster::new(&[0], &[0]);
    cluster.spawn(0);
    cluster.wait_marker(0, "genesis app_hash=", Duration::from_secs(30));

    // land real state the checkpoint must carry across the signal.
    write_and_confirm(&cluster, 0, "quit", "gracefully");
    write_and_confirm(&cluster, 0, "via", "sigterm");
    let before = cluster.status(0);
    let app_hash_before = before["app_hash"]
        .as_str()
        .expect("status app_hash")
        .to_string();
    let height_before = before["height"].as_u64().expect("status height");

    // ---- SIGTERM: the desktop-quit signal, not a SIGKILL --------------------
    cluster.term(0);
    // the signal arm announces itself, runs the graceful checkpoint, exits 0.
    cluster.wait_marker(
        0,
        "SIGTERM/SIGINT — graceful checkpoint then exit",
        Duration::from_secs(30),
    );
    cluster.wait_exit(0, Duration::from_secs(30));

    // ---- restart: must RECOVER (not re-run genesis) to the exact tip --------
    cluster.spawn(0);
    let recovered = cluster.wait_marker(0, "recovered app_hash=", Duration::from_secs(30));
    let recovered_hash = recovered.split_whitespace().next().expect("recovered hash");
    assert_eq!(
        recovered_hash, app_hash_before,
        "recovered app-hash after a graceful SIGTERM must be byte-identical to the pre-signal boundary"
    );
    assert!(
        cluster.marker(0, "genesis app_hash=").is_none(),
        "a SIGTERM restart must not re-run genesis (no brick)"
    );

    let after = poll_until("status after sigterm recovery", Duration::from_secs(30), || {
        let s = cluster.status(0);
        (s["app_hash"] == before["app_hash"]).then_some(s)
    });
    assert!(after["height"].as_u64().expect("height") >= height_before);
    // both pre-signal writes survived the graceful checkpoint.
    assert_eq!(dir_value(&cluster, 0, "quit").as_deref(), Some("gracefully"));
    assert_eq!(dir_value(&cluster, 0, "via").as_deref(), Some("sigterm"));
    // and the engine is LIVE again over its reopened journal.
    write_and_confirm(&cluster, 0, "after-sigterm", "still-here");
}
