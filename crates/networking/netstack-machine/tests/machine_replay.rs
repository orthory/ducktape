//! The sans-I/O drivability proof: whole meshes converge with the test
//! shuttling effects back in as events — no runtime, no sockets, no clock
//! beyond a counter. Every `MeshSend` becomes the target machine's
//! `Deliver`, every `ResolveStart` is answered inline, every `WgApply`
//! round-trips synchronously, exactly as the host executor performs them.
//! The frozen scenario suite grows from this harness: anything it can
//! express is replayable byte-for-byte on any host, wasm included.

use std::collections::VecDeque;
use std::net::{IpAddr, Ipv4Addr};

use commonware_cryptography::{Signer as _, ed25519::PrivateKey, ed25519::PublicKey};
use netstack_machine::{
    Effect, Event, Machine, MachineConfig, MeshEpochEvent, ReachabilityEvent, Resolution,
};
use wireguard::{Endpoint, PortPolicy, Transport, X25519PublicKey};

/// A pure mesh: N machines, a FIFO of undelivered plane messages, and each
/// machine's observed event stream. The simulation clock is a counter —
/// milliseconds only in name — which is the point: nothing the machines do
/// reads a real clock.
struct Sim {
    machines: Vec<Machine>,
    keys: Vec<PublicKey>,
    observed: Vec<Vec<ReachabilityEvent>>,
    queue: VecDeque<(usize, Event)>,
    now_ms: u64,
}

impl Sim {
    /// `octets` gives each machine its distinct key seed and endpoint IP.
    fn new(octets: &[u8]) -> Self {
        let mut machines = Vec::new();
        let mut keys = Vec::new();
        for &octet in octets {
            let signer = PrivateKey::from_seed(u64::from(octet));
            keys.push(signer.public_key());
            let policy = PortPolicy::production();
            let endpoint = |port: u16, transport| {
                // a globally routable address: the production port policy
                // refuses private and reserved ranges.
                Endpoint::new(
                    IpAddr::V4(Ipv4Addr::new(8, 8, 8, octet)),
                    port,
                    transport,
                    &policy,
                )
                .unwrap()
            };
            machines.push(Machine::new(MachineConfig {
                chain_id: "net#pure".into(),
                signer,
                wireguard_public: X25519PublicKey([octet; 32]),
                wireguard_advertised: Some(endpoint(51820, Transport::Udp)),
                control_endpoint: endpoint(443, Transport::Tcp),
                coordinators: vec![],
                port_policy: PortPolicy::production(),
                persist: false,
                gossip_ingress: None,
            }));
        }
        let observed = octets.iter().map(|_| Vec::new()).collect();
        Sim {
            machines,
            keys,
            observed,
            queue: VecDeque::new(),
            now_ms: 0,
        }
    }

    /// Retarget every machine to one epoch over the whole member set, then
    /// run the mesh to quiescence.
    fn converge(&mut self, epoch: u64) {
        for idx in 0..self.machines.len() {
            let event = MeshEpochEvent {
                epoch,
                members: self.keys.clone(),
                standbys: vec![],
                current_view: 1,
            };
            self.drive(
                idx,
                Event::Retarget {
                    event,
                    persisted: None,
                },
            );
        }
        while let Some((idx, event)) = self.queue.pop_front() {
            self.drive(idx, event);
        }
    }

    /// Step one machine and perform its effects the way the host executor
    /// does: sends queue as the target's deliveries, interface pushes and
    /// resolves are answered inline with their cascade's effects processed
    /// before the remainder, observations are recorded.
    fn drive(&mut self, idx: usize, event: Event) {
        let mut stack = vec![self.step(idx, event).into_iter()];
        while let Some(top) = stack.last_mut() {
            let Some(effect) = top.next() else {
                stack.pop();
                continue;
            };
            match effect {
                Effect::MeshSend { to, bytes } => {
                    let target = self
                        .keys
                        .iter()
                        .position(|key| *key == to)
                        .expect("a machine only sends to mesh participants");
                    let from = self.keys[idx].clone();
                    self.queue.push_back((target, Event::Deliver { from, bytes }));
                }
                Effect::Observe(observed) => self.observed[idx].push(observed),
                Effect::WgApply { req, .. } => {
                    let more = self.step(
                        idx,
                        Event::WgApplied {
                            req,
                            outcome: Ok(()),
                        },
                    );
                    stack.push(more.into_iter());
                }
                Effect::ResolveStart { req, .. } => {
                    let more = self.step(
                        idx,
                        Event::Resolved {
                            req,
                            outcome: Ok(Resolution::Advertised),
                        },
                    );
                    stack.push(more.into_iter());
                }
                Effect::WgRemove => {}
                Effect::RendezvousStart { .. }
                | Effect::UdpSend { .. }
                | Effect::UdpSendAwait { .. }
                | Effect::ReplyInstall { .. }
                | Effect::ReplyIntro { .. }
                | Effect::Persist { .. } => {
                    panic!("effect the pure mesh scenario never produces")
                }
            }
        }
    }

    fn step(&mut self, idx: usize, event: Event) -> Vec<Effect> {
        self.now_ms += 1;
        self.machines[idx]
            .step(event, self.now_ms)
            .expect("a pure mesh step never fails")
    }

    fn saw_mesh_ready(&self, idx: usize, epoch: u64) -> bool {
        self.observed[idx]
            .iter()
            .any(|event| matches!(event, ReachabilityEvent::MeshReady { epoch: got, .. } if *got == epoch))
    }

    fn tunnels_applied_peers(&self, idx: usize, epoch: u64) -> Option<usize> {
        self.observed[idx].iter().find_map(|event| match event {
            ReachabilityEvent::TunnelsApplied {
                epoch: got, peers, ..
            } if *got == epoch => Some(*peers),
            _ => None,
        })
    }

    fn saw_peer_failure(&self, idx: usize) -> bool {
        self.observed[idx]
            .iter()
            .any(|event| matches!(event, ReachabilityEvent::PeerFailed { .. }))
    }
}

#[test]
fn a_solo_mesh_applies_without_any_transport() {
    let mut sim = Sim::new(&[10]);
    sim.converge(1);

    assert!(sim.queue.is_empty(), "a solo mesh has nobody to message");
    assert!(sim.saw_mesh_ready(0, 1));
    assert_eq!(sim.tunnels_applied_peers(0, 1), Some(0));
}

#[test]
fn three_machines_converge_by_effect_shuttling_alone() {
    let mut sim = Sim::new(&[10, 20, 30]);
    sim.converge(1);

    for idx in 0..3 {
        assert!(sim.saw_mesh_ready(idx, 1), "machine {idx} verified the mesh");
        assert_eq!(
            sim.tunnels_applied_peers(idx, 1),
            Some(2),
            "machine {idx} applied both peer tunnels"
        );
        assert!(
            !sim.saw_peer_failure(idx),
            "machine {idx} resolved every peer cleanly"
        );
    }
}

#[test]
fn a_cutover_reconfigures_to_the_reduced_mesh() {
    let mut sim = Sim::new(&[10, 20, 30]);
    sim.converge(1);

    // epoch 2 drops the third machine; the survivors retarget and re-verify.
    let survivors: Vec<PublicKey> = sim.keys[..2].to_vec();
    for idx in 0..2 {
        let event = MeshEpochEvent {
            epoch: 2,
            members: survivors.clone(),
            standbys: vec![],
            current_view: 2,
        };
        sim.drive(
            idx,
            Event::Retarget {
                event,
                persisted: None,
            },
        );
    }
    while let Some((idx, event)) = sim.queue.pop_front() {
        // the departed machine still holds epoch 1: deliveries to it are
        // dropped, exactly as a dead node drops traffic.
        if idx == 2 {
            continue;
        }
        sim.drive(idx, event);
    }

    for idx in 0..2 {
        assert!(sim.saw_mesh_ready(idx, 2), "machine {idx} verified epoch 2");
        assert_eq!(
            sim.tunnels_applied_peers(idx, 2),
            Some(1),
            "machine {idx} applied the reduced mesh"
        );
    }
}
