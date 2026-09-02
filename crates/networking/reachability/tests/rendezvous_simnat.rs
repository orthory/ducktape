//! CI simulated-NAT suite — the private-cutover acceptance gate.
//!
//! Every row drives the PRODUCTION rendezvous stack — [`NatResolver`] over a
//! `NatClient` talking to the authenticated coordinator loop — across
//! `nat_traversal::simnet`'s deterministic NAT topology. The paused clock
//! runs the resolver's real timeouts (coordinator step, punch window,
//! keepalive interval, establishment backoff) virtually, so the suite is
//! both instant and scheduling-deterministic: what passes here is the exact
//! algorithm a node runs, not a stand-in driver.
//!
//! | Acceptance item                 | Test here                                 |
//! |---------------------------------|-------------------------------------------|
//! | reflexive discovery             | `reflexive_discovery`                     |
//! | hole-punch success              | `hole_punch_success_with_idle_passive_side` |
//! | hole-punch failure is terminal  | `hole_punch_failure_is_terminal`          |
//! | endpoint-churn re-advertisement | `endpoint_churn_readvertise_reconnect`    |
//! | multiple coordinators           | `multi_coordinator_failover`              |
//! | keepalive / coordinator death   | `punched_path_survives_coordinator_death` |
//!
//! The coordinator is rendezvous ONLY: there is no relay fallback, so a pair
//! that cannot hole-punch (e.g. symmetric NAT on both sides) terminally
//! fails resolution instead of degrading onto a coordinator-carried path.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use commonware_cryptography::{Signer as _, ed25519};
use nat_traversal::{
    AuthPolicy, NatClient, NatSocket, NodeKey, SimHandle, SimNat, SimNetwork, run_coordinator,
};
use reachability::{
    EndpointResolver as _, NatResolver, RENDEZVOUS_KEEPALIVE, RendezvousStatus, Resolution,
};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// A placeholder advertised endpoint: the resolver must produce a punched
/// path without ever consulting it.
const ADVERTISED: &str = "203.0.113.9:1";

fn ip(octet: u8) -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(198, 51, 100, octet))
}

fn internal(octet: u8) -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, octet, 1)), 51820)
}

fn identity(seed: u64) -> (NodeKey, ed25519::PrivateKey) {
    let signer = ed25519::PrivateKey::from_seed(seed);
    let mut key = [0; 32];
    key.copy_from_slice(signer.public_key().as_ref());
    (NodeKey(key), signer)
}

/// Stand the real authenticated coordinator loop up on a public simulated
/// endpoint. Abort the task before `SimNetwork::remove`-ing the address.
fn spawn_coordinator(net: &SimNetwork) -> (SocketAddr, JoinHandle<()>) {
    let addr: SocketAddr = "192.0.2.1:3478".parse().expect("coordinator addr");
    let sock = net.public(addr);
    let task = tokio::spawn(run_coordinator(
        NatSocket::Simulated(sock),
        AuthPolicy::Public,
    ));
    (addr, task)
}

/// One simulated node: the production resolver over a NATed sim endpoint,
/// with the NAT's out-of-band handle and the non-rendezvous datagram sink.
struct SimNode {
    resolver: NatResolver,
    key: NodeKey,
    nat: SimHandle,
    datagrams: mpsc::Receiver<(SocketAddr, Vec<u8>)>,
}

fn node(
    net: &SimNetwork,
    seed: u64,
    octet: u8,
    coords: Vec<SocketAddr>,
    nat: SimNat,
    keepalive: Duration,
) -> SimNode {
    let (key, signer) = identity(seed);
    let (sock, handle) = net.behind(nat, internal(octet));
    let client = NatClient::with_socket(NatSocket::Simulated(sock), key, coords, signer, None)
        .expect("client over the simulated socket");
    let (datagram_tx, datagram_rx) = mpsc::channel(16);
    let resolver =
        NatResolver::from_client_with_datagram_sink(client, keepalive, Some(datagram_tx));
    SimNode {
        resolver,
        key,
        nat: handle,
        datagrams: datagram_rx,
    }
}

/// Wait (bounded, in virtual time) until rendezvous establishment lands and
/// return the coordinator-observed reflexive.
async fn ready(resolver: &NatResolver) -> SocketAddr {
    let mut status = resolver.status().expect("resolver has coordinators");
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if let RendezvousStatus::Ready { reflexive } = *status.borrow_and_update() {
                return reflexive;
            }
            status.changed().await.expect("establish task alive");
        }
    })
    .await
    .expect("rendezvous must establish over the simulated network")
}

fn punched(resolution: Resolution) -> SocketAddr {
    match resolution {
        Resolution::Punched(endpoint) => endpoint,
        Resolution::Advertised => panic!("expected a punched path, got the advertised endpoint"),
    }
}

#[tokio::test(start_paused = true)]
async fn reflexive_discovery() {
    let net = SimNetwork::new();
    let (coord, _task) = spawn_coordinator(&net);
    let a = node(
        &net,
        1,
        1,
        vec![coord],
        SimNat::new(ip(1)),
        RENDEZVOUS_KEEPALIVE,
    );

    let reflexive = ready(&a.resolver).await;
    assert_eq!(
        reflexive.ip(),
        ip(1),
        "the coordinator observed the NAT's public mapping"
    );
    assert_ne!(
        reflexive,
        internal(1),
        "the reflexive is the mapping, never the private socket"
    );
    assert_eq!(a.resolver.reflexive(), Some(reflexive));
}

#[tokio::test(start_paused = true)]
async fn hole_punch_success_with_idle_passive_side() {
    let net = SimNetwork::new();
    let (coord, _task) = spawn_coordinator(&net);
    let mut a = node(
        &net,
        2,
        1,
        vec![coord],
        SimNat::new(ip(1)),
        RENDEZVOUS_KEEPALIVE,
    );
    let b = node(
        &net,
        3,
        2,
        vec![coord],
        SimNat::new(ip(2)),
        RENDEZVOUS_KEEPALIVE,
    );
    ready(&a.resolver).await;
    let b_reflexive = ready(&b.resolver).await;

    // B never calls resolve: its pump must answer the coordinator's
    // PunchSync fan-out on its own for A's punch to complete.
    let resolution = a
        .resolver
        .resolve(b.key, ADVERTISED.parse().unwrap())
        .await
        .expect("punch completes against an idle peer");
    assert_eq!(
        punched(resolution),
        b_reflexive,
        "A dials B's NAT mapping, resolved through rendezvous"
    );
}

#[tokio::test(start_paused = true)]
async fn hole_punch_failure_is_terminal() {
    // A symmetric-NAT pair defeats simultaneous open, and with no relay
    // fallback that is the END of resolution: an honest error the caller
    // surfaces as `PeerFailed`, never a coordinator-carried data path.
    let net = SimNetwork::new();
    let (coord, _task) = spawn_coordinator(&net);
    let mut a = node(
        &net,
        4,
        1,
        vec![coord],
        SimNat::symmetric(ip(1)),
        RENDEZVOUS_KEEPALIVE,
    );
    let b = node(
        &net,
        5,
        2,
        vec![coord],
        SimNat::symmetric(ip(2)),
        RENDEZVOUS_KEEPALIVE,
    );
    ready(&a.resolver).await;
    ready(&b.resolver).await;

    let err = a
        .resolver
        .resolve(b.key, ADVERTISED.parse().unwrap())
        .await
        .expect_err("a symmetric pair must not connect");
    assert!(
        err.contains("hole-punch failed after"),
        "the failure names the exhausted punch budget: {err}"
    );
}

#[tokio::test(start_paused = true)]
async fn endpoint_churn_readvertise_reconnect() {
    let net = SimNetwork::new();
    let (coord, _task) = spawn_coordinator(&net);
    // A's keepalive is short so its re-advertisement lands well inside B's
    // punch-retry budget once the NAT churns.
    let a = node(
        &net,
        6,
        1,
        vec![coord],
        SimNat::new(ip(1)),
        Duration::from_millis(100),
    );
    let mut b = node(
        &net,
        7,
        2,
        vec![coord],
        SimNat::new(ip(2)),
        RENDEZVOUS_KEEPALIVE,
    );
    let a_reflexive = ready(&a.resolver).await;
    ready(&b.resolver).await;

    let first = b
        .resolver
        .resolve(a.key, ADVERTISED.parse().unwrap())
        .await
        .expect("initial punch");
    let old_endpoint = punched(first);
    assert_eq!(old_endpoint, a_reflexive);

    // A's NAT rebinds: the stale mapping admits nobody. The keepalive
    // re-advertises A's fresh mapping under a higher nonce, superseding the
    // stale registration, and B's re-resolution punches the NEW mapping.
    a.nat.rebind();
    let second = b
        .resolver
        .resolve(a.key, ADVERTISED.parse().unwrap())
        .await
        .expect("reconnect after the rebind rides the re-advertised mapping");
    let new_endpoint = punched(second);
    assert_ne!(
        new_endpoint, old_endpoint,
        "B reconnected against the superseding reflexive, not the stale one"
    );
    assert_eq!(new_endpoint.ip(), ip(1));
}

#[tokio::test(start_paused = true)]
async fn multi_coordinator_failover() {
    let net = SimNetwork::new();
    let (live, _task) = spawn_coordinator(&net);
    // Nobody owns this address: datagrams toward it vanish, so the primary
    // is dark and establishment must fail over to the live secondary.
    let dead: SocketAddr = "192.0.2.9:3478".parse().unwrap();

    let a = node(
        &net,
        8,
        1,
        vec![dead, live],
        SimNat::new(ip(1)),
        RENDEZVOUS_KEEPALIVE,
    );
    let reflexive = ready(&a.resolver).await;
    assert_eq!(
        reflexive.ip(),
        ip(1),
        "establishment landed through the live secondary"
    );
}

#[tokio::test(start_paused = true)]
async fn punched_path_survives_coordinator_death() {
    let net = SimNetwork::new();
    let (coord, coordinator) = spawn_coordinator(&net);
    let mut a = node(
        &net,
        9,
        1,
        vec![coord],
        SimNat::new(ip(1)),
        RENDEZVOUS_KEEPALIVE,
    );
    let mut b = node(
        &net,
        10,
        2,
        vec![coord],
        SimNat::new(ip(2)),
        RENDEZVOUS_KEEPALIVE,
    );
    let a_reflexive = ready(&a.resolver).await;
    ready(&b.resolver).await;

    let resolution = a
        .resolver
        .resolve(b.key, ADVERTISED.parse().unwrap())
        .await
        .expect("punch");
    let b_endpoint = punched(resolution);

    // The coordinator dies. A punched path lives entirely in the two NATs'
    // pinhole state — nothing about it consults the coordinator.
    coordinator.abort();
    net.remove(coord);

    a.resolver
        .send_datagram(b_endpoint, b"direct-after-coordinator-death".to_vec())
        .await
        .expect("the punched path carries datagrams with no coordinator");
    let (src, bytes) = b
        .datagrams
        .recv()
        .await
        .expect("B's datagram sink delivers the direct payload");
    assert_eq!(bytes, b"direct-after-coordinator-death");
    assert_eq!(
        src, a_reflexive,
        "delivered from A's rendezvous-resolved mapping"
    );
}
