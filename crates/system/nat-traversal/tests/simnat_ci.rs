//! CI simulated-NAT suite — the private-cutover epic merge gate.
//!
//! Maps 1:1 to `docs/superpowers/specs/2026-07-05-private-cutover-coordinator-design.md`
//! §"Acceptance" item 1. Run with `--features simnat`.
//!
//! | Acceptance §1 item                       | Test here                                   |
//! |------------------------------------------|---------------------------------------------|
//! | reflexive discovery                      | `reflexive_discovery`                       |
//! | hole-punch success                       | `hole_punch_success`                        |
//! | hole-punch failure is terminal           | `hole_punch_failure_is_terminal`            |
//! | endpoint-churn re-advertisement          | `endpoint_churn_readvertise_reconnect`      |
//! | (multiple coordinators — Slice 3)        | `multi_coordinator_failover`                |
//! | (keepalive survival — Slice 3)           | `punched_path_survives_coordinator_death`   |
//!
//! The coordinator is rendezvous ONLY: there is no relay fallback, so a pair
//! that cannot hole-punch (e.g. symmetric NAT on both sides) terminally fails
//! resolution instead of degrading onto a coordinator-carried path.
//!
//! Out of THIS gate (node-bin's clippy is pre-existingly red from toolchain
//! drift): v3 invite signature verify/reject and v2 parse-compatibility live in
//! `bin/node/src/config.rs` and are covered by Slice 1's own tests.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use nat_traversal::{
    Coordinator, NatClient, NodeKey, PunchError, SimNat, drive_rebind_reconnect, drive_simulated,
    run_coordinator,
};
use tokio::net::UdpSocket;
use tokio::time::timeout;

fn ip(o: u8) -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(198, 51, 100, o))
}

#[tokio::test]
async fn reflexive_discovery() {
    let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let coord_addr = coord_sock.local_addr().unwrap();
    tokio::spawn(run_coordinator(coord_sock));
    let client = NatClient::bind(NodeKey([1u8; 32]), coord_addr).await.unwrap();
    let reflexive = timeout(Duration::from_secs(2), client.discover_reflexive())
        .await
        .expect("no timeout")
        .expect("reflexive");
    assert_eq!(reflexive.port(), client.local_addr().await.unwrap().port());
}

#[test]
fn hole_punch_success() {
    let a = NodeKey([0xaa; 32]);
    let b = NodeKey([0xbb; 32]);
    let mut a_nat = SimNat::new(ip(1));
    let mut b_nat = SimNat::new(ip(2));
    let mut coord = Coordinator::new();
    let (ap, bp) = drive_simulated(a, b, &mut a_nat, &mut b_nat, &mut coord).expect("punch");
    assert_eq!(ap.peer_reflexive, bp.local_mapped);
    assert_eq!(bp.peer_reflexive, ap.local_mapped);
}

#[test]
fn hole_punch_failure_is_terminal() {
    // A symmetric-NAT pair defeats simultaneous open, and with no relay
    // fallback that is the END of resolution: `NotReachable`, honestly
    // surfaced, never a coordinator-carried data path.
    //
    // Deliberately overlaps punch.rs's symmetric-NAT unit test: this suite
    // mirrors the spec's Acceptance §1 rows one-to-one, so the row keeps its
    // own named test even where a unit test covers the same contract.
    let a = NodeKey([0xaa; 32]);
    let b = NodeKey([0xbb; 32]);
    let mut a_nat = SimNat::symmetric(ip(1));
    let mut b_nat = SimNat::symmetric(ip(2));
    let mut coord = Coordinator::new();
    let err = drive_simulated(a, b, &mut a_nat, &mut b_nat, &mut coord)
        .expect_err("symmetric pair must not connect");
    assert_eq!(err, PunchError::NotReachable);
}

#[test]
fn endpoint_churn_readvertise_reconnect() {
    let a = NodeKey([0xaa; 32]);
    let b = NodeKey([0xbb; 32]);
    let mut a_nat = SimNat::new(ip(1));
    let mut b_nat = SimNat::new(ip(2));
    let mut coord = Coordinator::new();
    let proof = drive_rebind_reconnect(a, b, &mut a_nat, &mut b_nat, &mut coord).expect("rebind");
    assert_ne!(proof.old_a_reflexive, proof.new_a_reflexive);
    assert_eq!(proof.b_plan.peer_reflexive, proof.new_a_reflexive);
}

#[tokio::test]
async fn multi_coordinator_failover() {
    let live = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let live_addr = live.local_addr().unwrap();
    tokio::spawn(run_coordinator(live));
    let dead = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let dead_addr = dead.local_addr().unwrap();

    let mut client = NatClient::bind_multi(NodeKey([3u8; 32]), vec![dead_addr, live_addr])
        .await
        .unwrap();
    let (idx, _reflexive) = timeout(
        Duration::from_secs(2),
        client.discover_reflexive_failover(Duration::from_millis(150)),
    )
    .await
    .expect("bounded")
    .expect("secondary answers");
    assert_eq!(idx, 1);
}

#[tokio::test]
async fn punched_path_survives_coordinator_death() {
    // A punched path lives entirely in the two NATs' filter state: once
    // established, the coordinator's death cannot touch it. (With rendezvous
    // only, no data path ever traverses the coordinator in the first place.)
    //
    // Deliberately overlaps client.rs's coordinator-shutdown unit test: this
    // suite mirrors the spec's Acceptance §1 rows one-to-one, so the row keeps
    // its own named test even where a unit test covers the same contract.
    let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let coord_addr = coord_sock.local_addr().unwrap();
    let coord = tokio::spawn(run_coordinator(coord_sock));
    let a_key = NodeKey([0xaa; 32]);
    let b_key = NodeKey([0xbb; 32]);
    let a = NatClient::bind(a_key, coord_addr).await.unwrap();
    let b = NatClient::bind(b_key, coord_addr).await.unwrap();
    a.register().await.unwrap();
    b.register().await.unwrap();
    let _ = timeout(Duration::from_secs(2), a.lookup(b_key))
        .await
        .expect("no timeout")
        .expect("lookup");
    let b_addr = SocketAddr::new(
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        b.local_addr().await.unwrap().port(),
    );
    let a_addr = SocketAddr::new(
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        a.local_addr().await.unwrap().port(),
    );

    coord.abort();

    // A sends straight to B with no coordinator. Retransmit-until-received so the
    // proof is order-independent and immune to loopback scheduling jitter under
    // the concurrently-running suite (the direct path is what's under test, not
    // the timing of a single datagram).
    let mut got = None;
    for _ in 0..50 {
        a.send_punch_to(b_addr).await.unwrap();
        if let Ok(r) = timeout(Duration::from_millis(100), b.recv_punch_from(a_addr)).await {
            got = Some(r.expect("recv"));
            break;
        }
    }
    assert_eq!(
        got.expect("direct path survives"),
        nat_traversal::Msg::Punch { from: a_key }
    );
}
