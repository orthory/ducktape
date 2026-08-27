//! The wasm guest over the frozen suite: every scenario, against the SAME
//! fixtures the native machine is held to — the traces must match byte for
//! byte — plus the one behavior the native machine cannot show: a fault.

use std::net::{IpAddr, Ipv4Addr};

use commonware_cryptography::Signer as _;
use commonware_cryptography::ed25519::PrivateKey;
use netstack_machine::{Event, MachineConfig, NetstackMachine, StepError};
use netstack_wasm::NetstackGuest;
use wireguard::{Endpoint, IdentitySigner, PortPolicy, Transport, X25519PublicKey};

/// The canonical artifact, built by guest-builder from the machine crate and
/// committed beside it (`make wasm-modules`).
const COMPONENT: &[u8] = include_bytes!("../../netstack-machine/component.wasm");

fn guest(signer: Box<dyn IdentitySigner>, config: MachineConfig) -> Box<dyn NetstackMachine> {
    Box::new(NetstackGuest::new(COMPONENT, signer, config).expect("the netstack component loads"))
}

netstack_scenarios::suite!(guest);

fn public_node(octet: u8) -> (PrivateKey, MachineConfig) {
    let policy = PortPolicy::production();
    let ip = IpAddr::V4(Ipv4Addr::new(8, 8, 8, octet));
    let endpoint = |port, transport| Endpoint::new(ip, port, transport, &policy).unwrap();
    let signer = PrivateKey::from_seed(u64::from(octet));
    let config = MachineConfig {
        chain_id: "net#guest".into(),
        wireguard_public: X25519PublicKey([octet; 32]),
        wireguard_advertised: Some(endpoint(51_820, Transport::Udp)),
        control_endpoint: endpoint(443, Transport::Tcp),
        coordinators: Vec::new(),
        port_policy: policy.clone(),
        persist: false,
        gossip_ingress: None,
    };
    (signer, config)
}

/// A guest that exhausts its step budget is a FAULT — the executor's
/// fail-over signal — never a protocol error, and never a silent no-op.
#[test]
fn an_exhausted_step_budget_is_a_fault() {
    let (signer, config) = public_node(10);
    let mut guest = NetstackGuest::with_fuel(COMPONENT, Box::new(signer), config, 1)
        .expect("the configure call runs under the default budget");
    let err = guest.step(Event::Nudge, 1_000).unwrap_err();
    assert!(matches!(err, StepError::Fault(_)), "{err}");
}

/// After a fault the instance is not reused: a fresh guest over the same
/// component steps normally.
#[test]
fn a_fresh_guest_steps_after_another_faulted() {
    let (signer, config) = public_node(10);
    let mut faulted =
        NetstackGuest::with_fuel(COMPONENT, Box::new(signer.clone()), config.clone(), 1).unwrap();
    assert!(faulted.step(Event::Nudge, 1_000).is_err());
    let mut fresh = NetstackGuest::new(COMPONENT, Box::new(signer), config).unwrap();
    assert!(fresh.step(Event::Nudge, 1_000).unwrap().is_empty());
}
