//! Orchestrator e2e over an in-memory message router: N `run()` instances,
//! each with its own keystore + fake effect, wired Send->Deliver exactly the
//! way bin/node's reachability channel will wire them. Proves the whole
//! phase-A pipeline — record gossip -> signed adverts -> converged mesh
//! version -> pairwise handshakes -> ONE apply per node — with no real
//! sockets and no real WireGuard.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
use defguard_wireguard_rs::net::IpAddrMask;
use nat_traversal::NodeKey;
use reachability::{
    EndpointResolver as _, MeshEpochEvent, ReachabilityCommand, ReachabilityConfig,
    ReachabilityEvent, Resolution, StaticResolver, WireGuardKeypair, binding,
};
use tokio::sync::mpsc;
use tokio::task::LocalSet;
use wireguard_effect::{FakeWireGuardEffect, FakeWireGuardEffectError, WireGuardEffect};
use wireguard_upgrade::{Endpoint, PortPolicy, Transport, ValidatorIdentity};

/// `run()` owns its effect; tests need to inspect it afterwards — a shared
/// handle delegating to the fake underneath.
#[derive(Clone, Default)]
struct SharedFake(Arc<Mutex<FakeWireGuardEffect>>);

impl WireGuardEffect for SharedFake {
    type Error = FakeWireGuardEffectError;

    fn create_interface(&mut self) -> Result<(), Self::Error> {
        self.0.lock().unwrap().create_interface()
    }

    fn apply(
        &mut self,
        config: &defguard_wireguard_rs::InterfaceConfiguration,
    ) -> Result<(), Self::Error> {
        self.0.lock().unwrap().apply(config)
    }

    fn remove_interface(&mut self) -> Result<(), Self::Error> {
        self.0.lock().unwrap().remove_interface()
    }
}

const CHAIN: &str = "net#e2e";

struct TestNode {
    signer: PrivateKey,
    identity: ValidatorIdentity,
    octet: u8,
    cmd: mpsc::Sender<ReachabilityCommand>,
    effect: SharedFake,
}

fn endpoint(policy: &PortPolicy, octet: u8, port: u16, transport: Transport) -> Endpoint {
    Endpoint::new(
        std::net::IpAddr::V4(std::net::Ipv4Addr::new(8, 8, 8, octet)),
        port,
        transport,
        policy,
    )
    .unwrap()
}

/// Spin up `seeds.len()` orchestrators on the local set, wired through an
/// in-memory router; returns the node handles and the collected non-Send
/// event stream as `(node index, event)`.
fn spawn_mesh(
    local: &LocalSet,
    dir: &std::path::Path,
    seeds: &[u64],
    resolvers: Vec<StaticResolver>,
) -> (Vec<TestNode>, mpsc::Receiver<(usize, ReachabilityEvent)>) {
    let policy = PortPolicy::production();
    let signers: Vec<PrivateKey> = seeds.iter().map(|s| PrivateKey::from_seed(*s)).collect();
    let pks: Vec<_> = signers.iter().map(|s| s.public_key()).collect();
    let (collected_tx, collected_rx) = mpsc::channel(256);

    let mut cmds = Vec::new();
    let mut events = Vec::new();
    for _ in seeds {
        let (cmd_tx, cmd_rx) = mpsc::channel(256);
        let (ev_tx, ev_rx) = mpsc::channel(256);
        cmds.push(cmd_tx);
        events.push(Some((cmd_rx, ev_tx, ev_rx)));
    }

    let mut nodes = Vec::new();
    for (i, signer) in signers.iter().cloned().enumerate() {
        let octet = 10 * (i as u8 + 1);
        let (cmd_rx, ev_tx, ev_rx) = events[i].take().unwrap();
        let effect = SharedFake::default();
        let config = ReachabilityConfig {
            chain_id: CHAIN.into(),
            signer: signer.clone(),
            wireguard_key_file: dir.join(format!("wg-{i}.key")),
            wireguard_listen: endpoint(&policy, octet, 51820, Transport::Udp),
            control_endpoint: endpoint(&policy, octet, 443, Transport::Tcp),
            coordinators: vec![],
            port_policy: policy.clone(),
        };
        let resolver = resolvers
            .get(i)
            .map(|r| StaticResolver(r.0.clone()))
            .unwrap_or_default();
        local.spawn_local(reachability::run(
            config,
            effect.clone(),
            resolver,
            cmd_rx,
            ev_tx,
        ));

        // the router: this node's Send events become Deliver commands on the
        // target; everything else is collected for assertions.
        let all_cmds = cmds.clone();
        let all_pks = pks.clone();
        let my_pk = pks[i].clone();
        let collected = collected_tx.clone();
        let mut ev_rx = ev_rx;
        local.spawn_local(async move {
            while let Some(event) = ev_rx.recv().await {
                match event {
                    ReachabilityEvent::Send { to, bytes } => {
                        if let Some(j) = all_pks.iter().position(|pk| *pk == to) {
                            let _ = all_cmds[j]
                                .send(ReachabilityCommand::Deliver {
                                    from: my_pk.clone(),
                                    bytes,
                                })
                                .await;
                        }
                    }
                    other => {
                        let _ = collected.send((i, other)).await;
                    }
                }
            }
        });

        nodes.push(TestNode {
            identity: binding::identity_of(&pks[i]),
            signer,
            octet,
            cmd: cmds[i].clone(),
            effect,
        });
    }
    (nodes, collected_rx)
}

async fn retarget_all(nodes: &[TestNode], members: &[usize], epoch: u64, view: u64) {
    let pks: Vec<_> = members
        .iter()
        .map(|i| nodes[*i].signer.public_key())
        .collect();
    for i in members {
        nodes[*i]
            .cmd
            .send(ReachabilityCommand::Retarget(MeshEpochEvent {
                epoch,
                members: pks.clone(),
                current_view: view,
            }))
            .await
            .unwrap();
    }
}

/// Drain collected events until every node in `want` has emitted
/// `TunnelsApplied` for `epoch`; returns the `MeshReady` versions seen.
async fn await_applied(
    collected: &mut mpsc::Receiver<(usize, ReachabilityEvent)>,
    want: &[usize],
    epoch: u64,
) -> HashMap<usize, Vec<u8>> {
    let mut versions = HashMap::new();
    let mut applied: Vec<usize> = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), async {
        while applied.len() < want.len() {
            let (i, event) = collected.recv().await.expect("event stream open");
            match event {
                ReachabilityEvent::MeshReady {
                    epoch: e, version, ..
                } if e == epoch => {
                    versions.insert(i, version.0.to_vec());
                }
                ReachabilityEvent::TunnelsApplied { epoch: e, .. } if e == epoch => {
                    applied.push(i);
                }
                ReachabilityEvent::EpochFailed { reason, .. } => {
                    panic!("epoch failed on node {i}: {reason}");
                }
                _ => {}
            }
        }
    })
    .await
    .expect("mesh converged in time");
    versions
}

fn ula(identity: ValidatorIdentity) -> IpAddrMask {
    IpAddrMask::new(
        std::net::IpAddr::V6(wireguard_upgrade::ula_v6_member_addr(CHAIN, identity)),
        128,
    )
}

#[tokio::test]
async fn three_member_mesh_converges_and_applies() {
    let local = LocalSet::new();
    let dir = tempfile::tempdir().unwrap();
    local
        .run_until(async {
            // node 0 resolves node 2 through a "punched" path; every other
            // pairing rides advertised endpoints.
            let punched: std::net::SocketAddr = "203.0.113.7:40001".parse().unwrap();
            let identity_of_seed =
                |seed: u64| binding::identity_of(&PrivateKey::from_seed(seed).public_key());
            let mut r0 = StaticResolver::default();
            r0.0.insert(
                NodeKey(identity_of_seed(3).0),
                Resolution::Punched(punched),
            );
            let (nodes, mut collected) = spawn_mesh(
                &local,
                dir.path(),
                &[1, 2, 3],
                vec![r0, StaticResolver::default(), StaticResolver::default()],
            );

            retarget_all(&nodes, &[0, 1, 2], 1, 10).await;
            let versions = await_applied(&mut collected, &[0, 1, 2], 1).await;

            // one content-derived mesh version, byte-identical on every node.
            assert_eq!(versions.len(), 3);
            assert_eq!(versions[&0], versions[&1]);
            assert_eq!(versions[&0], versions[&2]);

            let ifname = binding::interface_name(CHAIN);
            for (i, node) in nodes.iter().enumerate() {
                let fake = node.effect.0.lock().unwrap();
                assert_eq!(fake.create_calls, 1, "node {i}: one interface");
                assert_eq!(fake.applied.len(), 1, "node {i}: one apply");
                let config = &fake.applied[0];
                assert_eq!(config.name, ifname);
                assert_eq!(config.port, 51820);
                // interface address: exactly this node's identity-hash /128.
                assert_eq!(config.addresses, vec![ula(node.identity)]);
                // the persisted keystore is what the interface runs.
                let (keypair, generated) =
                    WireGuardKeypair::load_or_generate(&dir.path().join(format!("wg-{i}.key")))
                        .unwrap();
                assert!(!generated, "node {i}: run() created the key");
                assert_eq!(config.prvkey, keypair.private_key_base64());
                // both peers present, each routing ONLY its own /128 and
                // keyed by ITS persisted public key.
                assert_eq!(config.peers.len(), 2);
                for (j, peer_node) in nodes.iter().enumerate() {
                    if i == j {
                        continue;
                    }
                    let (peer_keys, _) = WireGuardKeypair::load_or_generate(
                        &dir.path().join(format!("wg-{j}.key")),
                    )
                    .unwrap();
                    let entry = config
                        .peers
                        .iter()
                        .find(|p| p.allowed_ips == vec![ula(peer_node.identity)])
                        .unwrap_or_else(|| panic!("node {i}: no peer entry for node {j}"));
                    assert_eq!(entry.public_key.as_array(), peer_keys.public_key().0);
                    let expected = if i == 0 && j == 2 {
                        punched
                    } else {
                        format!("8.8.8.{}:51820", peer_node.octet).parse().unwrap()
                    };
                    assert_eq!(entry.endpoint, Some(expected), "node {i} -> node {j}");
                }
            }

            // shutdown tears the live interface down.
            nodes[0]
                .cmd
                .send(ReachabilityCommand::Shutdown)
                .await
                .unwrap();
            tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    if nodes[0].effect.0.lock().unwrap().remove_calls == 1 {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .expect("shutdown removed the interface");
        })
        .await;
}

#[tokio::test]
async fn epoch_cutover_replaces_the_interface_with_the_reduced_mesh() {
    let local = LocalSet::new();
    let dir = tempfile::tempdir().unwrap();
    local
        .run_until(async {
            let (nodes, mut collected) = spawn_mesh(&local, dir.path(), &[1, 2, 3], vec![]);

            retarget_all(&nodes, &[0, 1, 2], 1, 10).await;
            await_applied(&mut collected, &[0, 1, 2], 1).await;

            // epoch 2: node 2 departs.
            retarget_all(&nodes, &[0, 1], 2, 20).await;
            await_applied(&mut collected, &[0, 1], 2).await;

            for i in [0usize, 1] {
                let fake = nodes[i].effect.0.lock().unwrap();
                assert_eq!(
                    fake.remove_calls, 1,
                    "node {i}: epoch 1's interface was removed"
                );
                assert_eq!(fake.applied.len(), 2);
                let second = &fake.applied[1];
                assert_eq!(second.peers.len(), 1, "node {i}: reduced mesh");
                let other = &nodes[1 - i];
                assert_eq!(second.peers[0].allowed_ips, vec![ula(other.identity)]);
            }
        })
        .await;
}

#[tokio::test]
async fn single_member_mesh_and_stranger_traffic_are_inert() {
    let local = LocalSet::new();
    let dir = tempfile::tempdir().unwrap();
    local
        .run_until(async {
            let (nodes, mut collected) = spawn_mesh(&local, dir.path(), &[1], vec![]);

            retarget_all(&nodes, &[0], 1, 10).await;
            await_applied(&mut collected, &[0], 1).await;
            {
                let fake = nodes[0].effect.0.lock().unwrap();
                assert_eq!(
                    fake.create_calls, 0,
                    "a peerless mesh brings up no interface"
                );
            }

            // a well-formed message from a key outside the member set is
            // refused loudly, never processed.
            let stranger = PrivateKey::from_seed(99);
            nodes[0]
                .cmd
                .send(ReachabilityCommand::Deliver {
                    from: stranger.public_key(),
                    bytes: b"junk".to_vec(),
                })
                .await
                .unwrap();
            let reason = tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    if let Some((_, ReachabilityEvent::PeerFailed { reason, .. })) =
                        collected.recv().await
                    {
                        break reason;
                    }
                }
            })
            .await
            .expect("stranger traffic surfaced");
            assert!(reason.contains("non-member"), "{reason}");
        })
        .await;
}

/// The production resolver against a REAL coordinator + two real UDP
/// clients on loopback: register, lookup, simultaneous-open punch.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn nat_resolver_punches_over_loopback() {
    let coord_sock = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let coord_addr = coord_sock.local_addr().unwrap();
    tokio::spawn(nat_traversal::client::run_coordinator(coord_sock));

    let key_a = NodeKey([0xaa; 32]);
    let key_b = NodeKey([0xbb; 32]);
    let mut a = reachability::NatResolver::bind(key_a, vec![coord_addr])
        .await
        .unwrap();
    let mut b = reachability::NatResolver::bind(key_b, vec![coord_addr])
        .await
        .unwrap();
    assert!(a.reflexive().is_some(), "bind discovered the reflexive");

    let dummy: std::net::SocketAddr = "127.0.0.1:9".parse().unwrap();
    let (ra, rb) = tokio::join!(a.resolve(key_b, dummy), b.resolve(key_a, dummy));
    let (ra, rb) = (ra.unwrap(), rb.unwrap());
    // loopback "NAT": the punched address IS the peer's socket, and both
    // sides punched through.
    match (ra, rb) {
        (Resolution::Punched(to_b), Resolution::Punched(to_a)) => {
            assert_eq!(to_b, b.reflexive().unwrap());
            assert_eq!(to_a, a.reflexive().unwrap());
        }
        other => panic!("expected punched/punched, got {other:?}"),
    }
}

/// An empty coordinator set degrades to pass-through resolution.
#[tokio::test]
async fn nat_resolver_without_coordinators_is_pass_through() {
    let mut r = reachability::NatResolver::bind(NodeKey([1; 32]), vec![])
        .await
        .unwrap();
    assert_eq!(r.reflexive(), None);
    let advertised: std::net::SocketAddr = "8.8.8.10:51820".parse().unwrap();
    assert_eq!(
        r.resolve(NodeKey([2; 32]), advertised).await.unwrap(),
        Resolution::Advertised
    );
}
