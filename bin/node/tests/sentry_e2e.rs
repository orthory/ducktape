//! sentry-fronted entry over real sockets: a validator advertises ONLY a
//! transparent TCP forwarder (a "sentry") in front of it — its real listen
//! port is never handed to any peer — and an out-of-mesh joiner still enters,
//! syncs, and promotes entirely through that pipe.
//!
//! this is the tightest proof of the Phase-1 sentry pattern: a network of ONE
//! (`Cluster::new(&[0], &[0])`) has quorum 2-of-2 only after the friend joins,
//! so consensus finalizes an op founder<->friend ONLY if the joiner reached
//! node 0 — and the joiner can reach node 0 ONLY via the sentry (its config
//! lists members by KEY only; its sole address for node 0 is the sentry). A
//! connection counter on the pipe confirms the joiner's entry traffic actually
//! transited the forwarder.
//!
//! the sentry is pure `std::net`/`std::thread` (no new dependency, matching the
//! harness idiom): a `TcpListener` accept loop that, per connection, dials the
//! target and splices both directions with `std::io::copy`. it counts BRIDGED
//! CONNECTIONS, not bytes: the mesh connection is long-lived (a byte tally on
//! close races the assertion), and `std::io::copy` between two `TcpStream`s
//! takes the kernel `splice(2)` fast path — a userspace read/write copy adds
//! enough scheduling latency under load to trip commonware's handshake timeouts.

mod common;

use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use common::{Cluster, poll_until, serial};
use directory_interface::{DirMsg, DirQuery, DirReply, decode_reply, encode_msg, encode_query};

/// convergence budget: mesh formation + leader rotation are real-time on a
/// possibly-loaded CI core; polls exit early, so generosity is free.
const CONVERGE: Duration = Duration::from_secs(180);
/// budget for one submitted op to finalize and become readable elsewhere.
const FINALIZE: Duration = Duration::from_secs(60);

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
    match decode_reply(&reply) {
        Ok(DirReply::Value(v)) => v,
        Err(_) => None,
    }
}

fn hex(bytes: &[u8]) -> String {
    common::hex(bytes)
}

/// a transparent TCP forwarder in front of `target`. returns its public listen
/// address and a counter of CLIENT CONNECTIONS bridged to `target` — a race-free
/// proof that dialers reached node 0 through the forwarder. pure
/// `std::net`/`std::thread`; each direction is spliced with `std::io::copy`
/// (kernel `splice(2)` fast path, so the pipe stays low-latency under load).
///
/// the upstream `connect` is lazy (per client connection): node 0 is already up
/// by the time any client dials the sentry, but a transient connect error must
/// never panic the accept loop — the connection is simply dropped.
fn spawn_sentry(target: SocketAddr) -> (SocketAddr, Arc<AtomicU64>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind sentry listener");
    let sentry_addr = listener.local_addr().expect("sentry local addr");
    let conns = Arc::new(AtomicU64::new(0));
    let accept_conns = Arc::clone(&conns);

    std::thread::spawn(move || {
        for incoming in listener.incoming() {
            let client = match incoming {
                Ok(s) => s,
                Err(_) => continue,
            };
            let upstream = match TcpStream::connect(target) {
                Ok(s) => s,
                Err(_) => continue, // target transiently unreachable — drop this conn
            };
            accept_conns.fetch_add(1, Ordering::SeqCst);
            // two handles per socket so each relay thread owns one read + one
            // write half of the full-duplex pipe.
            let client_rx = client.try_clone().expect("clone client stream");
            let upstream_rx = upstream.try_clone().expect("clone upstream stream");

            // client -> upstream
            std::thread::spawn(move || {
                let mut src = client;
                let mut dst = upstream;
                let _ = std::io::copy(&mut src, &mut dst);
                let _ = dst.shutdown(Shutdown::Write);
            });
            // upstream -> client
            std::thread::spawn(move || {
                let mut src = upstream_rx;
                let mut dst = client_rx;
                let _ = std::io::copy(&mut src, &mut dst);
                let _ = dst.shutdown(Shutdown::Write);
            });
        }
    });

    (sentry_addr, conns)
}

#[test]
fn joiner_enters_through_a_sentry() {
    let _serial = serial();
    // a network of ONE, fronted by a transparent TCP sentry. node 0 is mesh +
    // quorum all by itself; it advertises only the sentry, and its real listen
    // port is never handed to any peer.
    let mut cluster = Cluster::new(&[0], &[0]);

    // node 0's real (private) listen addr — the port was pre-allocated in
    // `new()`, so it is known before the config is written at spawn time.
    let node0_listen: SocketAddr = format!("127.0.0.1:{}", cluster.p2p_ports[0])
        .parse()
        .expect("node 0 listen addr parses");
    let (sentry_addr, bridged) = spawn_sentry(node0_listen);

    // everyone reaches node 0 ONLY through the sentry: node 0 advertises the
    // sentry, and the joiner's bootstrap hint points at the sentry too. set
    // BOTH before `spawn(0)` — the config is generated at spawn time.
    cluster.advertised[0] = Some(sentry_addr.to_string());
    cluster.bootstrap_addr_override = Some(sentry_addr.to_string());

    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", Duration::from_secs(60));
    cluster.wait_marker(0, "converged app_hash=", CONVERGE);

    // the friend starts an out-of-mesh node whose ONLY entry point is the
    // sentry; it must PARK until admitted. an UNINVITED key is (correctly)
    // rejected by node 0's p2p bouncer BEFORE admission — node 0 does not yet
    // track it — so the pre-admission parked state is genuinely
    // "parked: mesh unreachable (...); retrying", matched by the generic
    // "parked:" prefix exactly as the sibling invite_e2e does. (the joiner
    // cannot reach node 0 until `invite-accept` runs, direct OR fronted; the
    // sentry is what carries the sync + votes AFTER admission.)
    let joiner = cluster.spawn_joiner(1);
    cluster.wait_marker(joiner, "joiner mode: parking", Duration::from_secs(60));
    cluster.wait_marker(joiner, "parked:", Duration::from_secs(60));

    // one command on the founder admits the friend.
    let friend_hex = hex(&Cluster::identity(1));
    let cfg = cluster.config_file(0);
    let (ok, out) = cluster.run_verb(&[
        "invite-accept",
        &friend_hex,
        "--config",
        cfg.to_str().expect("utf-8 config path"),
    ]);
    assert!(ok, "invite-accept failed:\n{out}");
    assert!(out.contains("admitted"), "unexpected verb output:\n{out}");

    // the founder cuts over to epoch 1; epoch 1 stalls at its base (quorum
    // 2-of-2) until the friend arrives — a frozen boundary at the genesis floor.
    cluster.wait_marker(0, "cutover complete: epoch 1", CONVERGE);

    // the parked node syncs the boundary and promotes — every byte of that
    // handshake + state-sync crossed the sentry pipe.
    cluster.wait_marker(joiner, "admitted at epoch 1", CONVERGE);
    cluster.wait_marker(joiner, "synced app_hash=", CONVERGE);
    cluster.wait_marker(joiner, "promoted: validator at epoch 1", CONVERGE);
    cluster.wait_marker(joiner, "recovered app_hash=", CONVERGE);

    // PROOF: the joiner's entry traffic transited the sentry. it has NO
    // non-sentry path (its config lists members by key only; its sole address
    // for node 0 is the sentry), so the bootstrap + park + state-sync it just
    // completed necessarily bridged through the forwarder.
    let bridged_conns = bridged.load(Ordering::SeqCst);
    eprintln!("[sentry] client connections bridged to node 0: {bridged_conns}");
    assert!(
        bridged_conns > 0,
        "no connection transited the sentry — the joiner did not enter through it"
    );

    // consensus is live again: a 2-of-2 simplex finalizes nothing without both
    // validators, so an op that commits proves the friend — which ENTERED
    // through the sentry — is now voting. (steady-state votes may ride a direct
    // founder<->friend link once discovery learns the friend's own advertised
    // address; the sentry proof is the ENTRY path, the cross-reads below are
    // independent liveness.) an op submitted via the FOUNDER must become
    // readable via the FRIEND...
    cluster.submit(0, "directory", &dir_set("from-founder", "hello"));
    let value = poll_until("founder's op to read on the friend", FINALIZE, || {
        dir_value(&cluster, joiner, "from-founder")
    });
    assert_eq!(value, "hello");

    // ...and an op submitted via the FRIEND must read on the founder.
    cluster.submit(joiner, "directory", &dir_set("from-friend", "hi back"));
    let value = poll_until("friend's op to read on the founder", FINALIZE, || {
        dir_value(&cluster, 0, "from-friend")
    });
    assert_eq!(value, "hi back");

    // no fork: identical status app-hashes once both sides quiesce (the founder
    // stayed alive throughout — 2-of-2 quorum must not tear down mid-read).
    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(
        cluster.status(0)["app_hash"],
        cluster.status(joiner)["app_hash"],
        "founder and promoted friend disagree on state"
    );
}
