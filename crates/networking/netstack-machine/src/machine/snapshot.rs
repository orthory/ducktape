//! The machine's state as one value: what a backend swap carries from the
//! machine that steps down to the one that takes over, so the new machine
//! continues the SAME epoch — the same tunnels stay applied, the same
//! handshakes stay in flight, the same nonces keep counting — and the
//! interface never flaps.
//!
//! Everything derived from the identity and the config (`me`, the overlay,
//! the interface name) is re-derived on restore, never carried; the signer
//! is the restoring host's. The one in-flight thing that cannot be carried
//! is an interface push, and the step contract already forbids taking a
//! snapshot while one is out.

use std::collections::BTreeMap;
use std::net::SocketAddr;

use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};
use wireguard::effect::PeerTunnelConfig;
use wireguard::{IdentitySigner, ValidatorIdentity};

use super::pending::{PendingAdopt, PendingOp, PendingRestore};
use super::{InvitePeer, Machine};
use crate::contract::{MachineConfig, ReqId};
use crate::epoch::EpochState;
use crate::wire::{self, SnapshotError, identity_socket_addrs};

/// Every field of the machine that is state rather than derivation, in
/// the order [`Machine`] holds them.
#[derive(Debug, BorshSerialize, BorshDeserialize, BorshSchema)]
pub(crate) struct MachineState {
    /// The identity the state was taken under: a restore by any other
    /// identity is refused, since every record and handshake in here is
    /// signed as it.
    me: ValidatorIdentity,
    now_ms: u64,
    view: u64,
    untargeted_nudges: u64,
    nudges: u64,
    interface_live: bool,
    base_peers: Option<BTreeMap<ValidatorIdentity, PeerTunnelConfig>>,
    invite_peers: BTreeMap<ValidatorIdentity, InvitePeer>,
    #[borsh(schema(with_funcs(
        declaration = "identity_socket_addrs::declaration",
        definitions = "identity_socket_addrs::definitions"
    )))]
    control_endpoints: BTreeMap<ValidatorIdentity, SocketAddr>,
    next_req: u64,
    pending: BTreeMap<ReqId, PendingOp>,
    pending_restore: Option<PendingRestore>,
    pending_adopt: Option<PendingAdopt>,
    epoch: Option<EpochState>,
}

impl Machine {
    /// The machine's whole state as one wire value (`wire`'s `snapshot`
    /// root). Between steps only: an interface push in flight is the
    /// caller's contract breach and is refused.
    pub fn snapshot(&self) -> Result<Vec<u8>, SnapshotError> {
        let push_in_flight = self.driver.wg.is_some();
        if push_in_flight {
            return Err(SnapshotError::PushInFlight);
        }
        debug_assert!(
            self.driver.effects.is_empty(),
            "a step drains its effects before returning"
        );
        let driver = &self.driver;
        let state = MachineState {
            me: driver.me,
            now_ms: driver.now_ms,
            view: driver.view,
            untargeted_nudges: driver.untargeted_nudges,
            nudges: driver.nudges,
            interface_live: driver.interface_live,
            base_peers: driver.base_peers.clone(),
            invite_peers: driver.invite_peers.clone(),
            control_endpoints: driver.control_endpoints.clone(),
            next_req: driver.next_req,
            pending: driver.pending.clone(),
            pending_restore: driver.pending_restore.clone(),
            pending_adopt: driver.pending_adopt.clone(),
            epoch: self.epoch.clone(),
        };
        Ok(wire::encode_snapshot(state))
    }

    /// A machine continuing from `snapshot` under `signer` and `config`:
    /// the identity must be the one the snapshot was taken under, and the
    /// contract must be this build's.
    pub fn restore(
        signer: Box<dyn IdentitySigner>,
        config: MachineConfig,
        snapshot: &[u8],
    ) -> Result<Self, SnapshotError> {
        let state = wire::decode_snapshot(snapshot)?;
        let mut machine = Machine::new(signer, config);
        let same_identity = state.me == machine.driver.me;
        if !same_identity {
            return Err(SnapshotError::Identity);
        }
        let driver = &mut machine.driver;
        driver.now_ms = state.now_ms;
        driver.view = state.view;
        driver.untargeted_nudges = state.untargeted_nudges;
        driver.nudges = state.nudges;
        driver.interface_live = state.interface_live;
        driver.base_peers = state.base_peers;
        driver.invite_peers = state.invite_peers;
        driver.control_endpoints = state.control_endpoints;
        driver.next_req = state.next_req;
        driver.pending = state.pending;
        driver.pending_restore = state.pending_restore;
        driver.pending_adopt = state.pending_adopt;
        machine.epoch = state.epoch;
        Ok(machine)
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use commonware_cryptography::Signer as _;
    use commonware_cryptography::ed25519::PrivateKey;
    use wireguard::{Endpoint, PortPolicy, Transport, X25519PublicKey};

    use super::*;
    use crate::contract::{Effect, Event, MeshEpochEvent};

    fn node(seed: u64) -> (PrivateKey, MachineConfig) {
        let policy = PortPolicy::production();
        let octet = u8::try_from(seed).unwrap();
        let ip = IpAddr::V4(Ipv4Addr::new(8, 8, 8, octet));
        let endpoint = |port, transport| Endpoint::new(ip, port, transport, &policy).unwrap();
        let config = MachineConfig {
            chain_id: "net#snapshot".into(),
            wireguard_public: X25519PublicKey([octet; 32]),
            wireguard_advertised: Some(endpoint(51_820, Transport::Udp)),
            control_endpoint: endpoint(443, Transport::Tcp),
            coordinators: Vec::new(),
            port_policy: policy,
            persist: false,
            gossip_ingress: None,
        };
        (PrivateKey::from_seed(seed), config)
    }

    /// A machine mid-assembly: retargeted to a two-member epoch, its own
    /// record and advert out, a peer's record and advert not yet in.
    fn assembling() -> (PrivateKey, MachineConfig, Machine) {
        let (signer, config) = node(1);
        let (peer, _) = node(2);
        let mut machine = Machine::new(Box::new(signer.clone()), config.clone());
        let retarget = Event::Retarget {
            event: MeshEpochEvent {
                epoch: 1,
                members: vec![signer.public_key(), peer.public_key()],
                standbys: Vec::new(),
                current_view: 10,
            },
            persisted: None,
        };
        let effects = machine.step(retarget, 1_000).unwrap();
        assert!(
            effects
                .iter()
                .any(|effect| matches!(effect, Effect::MeshSend { .. })),
            "the retarget fans the record out"
        );
        (signer, config, machine)
    }

    fn restore_error(
        signer: Box<dyn IdentitySigner>,
        config: MachineConfig,
        snapshot: &[u8],
    ) -> SnapshotError {
        match Machine::restore(signer, config, snapshot) {
            Ok(_) => panic!("the restore was accepted"),
            Err(err) => err,
        }
    }

    #[test]
    fn a_snapshot_restores_to_the_same_state() {
        let (signer, config, machine) = assembling();
        let bytes = machine.snapshot().unwrap();
        let restored = Machine::restore(Box::new(signer), config, &bytes).unwrap();
        assert_eq!(restored.snapshot().unwrap(), bytes);
    }

    /// The restored machine continues the epoch: the same nudge yields the
    /// same re-offer from both.
    #[test]
    fn a_restored_machine_steps_like_the_original() {
        let (signer, config, mut machine) = assembling();
        let bytes = machine.snapshot().unwrap();
        let mut restored = Machine::restore(Box::new(signer), config, &bytes).unwrap();
        let original = machine.step(Event::Nudge, 3_000).unwrap();
        let continued = restored.step(Event::Nudge, 3_000).unwrap();
        assert_eq!(original, continued);
        assert_eq!(machine.snapshot().unwrap(), restored.snapshot().unwrap());
    }

    #[test]
    fn another_identity_cannot_restore_it() {
        let (_, config, machine) = assembling();
        let bytes = machine.snapshot().unwrap();
        let (other, _) = node(3);
        let err = restore_error(Box::new(other), config, &bytes);
        assert!(matches!(err, SnapshotError::Identity), "{err}");
    }

    #[test]
    fn a_foreign_contract_is_refused_by_name() {
        let (signer, config, machine) = assembling();
        let mut bytes = machine.snapshot().unwrap();
        // the contract string is the first field: flip one hex digit.
        let first_hex = 4;
        bytes[first_hex] = if bytes[first_hex] == b'0' { b'1' } else { b'0' };
        let err = restore_error(Box::new(signer), config, &bytes);
        assert!(matches!(err, SnapshotError::Contract { .. }), "{err}");
    }

    #[test]
    fn trailing_bytes_are_refused() {
        let (signer, config, machine) = assembling();
        let mut bytes = machine.snapshot().unwrap();
        bytes.push(0);
        let err = restore_error(Box::new(signer), config, &bytes);
        assert!(matches!(err, SnapshotError::Trailing), "{err}");
    }
}
