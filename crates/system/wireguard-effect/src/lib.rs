//! Effect adapter that takes a validated `wireguard-upgrade` `TunnelInstallPlan`
//! and actually configures a WireGuard interface. `wireguard-upgrade` is a
//! pure validation leaf crate — nothing in the workspace calls
//! `WGApi::configure_interface` today. This crate is that missing consumer:
//! a `WireGuardEffect` trait behind which tests use a deterministic
//! `FakeWireGuardEffect` (CI has no real WireGuard userspace runtime) and
//! real runs use `DefguardWireGuardEffect` (`defguard_wireguard_rs`
//! `WGApi::<Userspace>`).

#[cfg(unix)]
mod defguard_effect;
#[cfg(unix)]
pub use defguard_effect::DefguardWireGuardEffect;

mod wiring;
pub use wiring::{
    PeerTunnelConfig, TUNNEL_MTU, apply_peer_tunnels, apply_tunnel_plan, apply_tunnel_plans,
    plan_peer_configs, update_peer_tunnels,
};

use defguard_wireguard_rs::InterfaceConfiguration;

/// Effect boundary between a validated WireGuard install plan and the real
/// network stack. `create_interface`/`remove_interface` bracket the
/// interface's lifetime; `apply` pushes a full configuration (private key,
/// listen port, overlay addresses, peer set) to it.
pub trait WireGuardEffect {
    type Error: std::fmt::Debug;

    /// Create the underlying WireGuard interface. Call once before `apply`.
    fn create_interface(&mut self) -> Result<(), Self::Error>;

    /// Apply (create-or-replace) the full interface configuration.
    fn apply(&mut self, config: &InterfaceConfiguration) -> Result<(), Self::Error>;

    /// Tear down the interface.
    fn remove_interface(&mut self) -> Result<(), Self::Error>;
}

/// Error produced by `FakeWireGuardEffect`. Unlike the real
/// `DefguardWireGuardEffect` (whose `Error` is `WireguardInterfaceError`),
/// the fake has no I/O to fail on its own — but it still must mirror the one
/// lifecycle invariant the real `WGApi::<Userspace>` path enforces:
/// `configure_interface` talks to `/var/run/wireguard/<ifname>.sock`, a
/// socket that only exists once `create_interface` has run. A fake that
/// accepted `apply` first would let tests pass against call sequences the
/// real adapter rejects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FakeWireGuardEffectError {
    /// `apply` (or `remove_interface`) was called without a preceding,
    /// still-live `create_interface`.
    NotCreated,
    /// `create_interface` was called while the interface is already live.
    /// The real userspace path fails the same way: `DeviceHandle::new`
    /// cannot stand up a second TUN under a name that still exists — a
    /// caller reconfiguring a live interface must `apply`, not re-create.
    AlreadyCreated,
    /// `apply` was rejected because `reject_next_apply` was armed. Stands in
    /// for a real `configure_interface` rejection (e.g. Defguard's
    /// `InterfaceConfiguration.prvkey` failing to decode to a 32-byte key)
    /// so callers can test their handling of that failure without a real
    /// WireGuard userspace runtime.
    Rejected,
}

/// Deterministic in-memory `WireGuardEffect` for tests: records every applied
/// configuration and lifecycle call instead of touching a real network
/// interface. CI has no WireGuard userspace runtime, so this is the only
/// `WireGuardEffect` the automated test suite exercises.
///
/// Enforces the same create-before-configure ordering the real
/// `WGApi::<Userspace>` path requires (see `FakeWireGuardEffectError`), so a
/// caller that gets this fake to accept a call sequence can trust the real
/// adapter accepts it too.
#[derive(Default)]
pub struct FakeWireGuardEffect {
    pub create_calls: usize,
    pub remove_calls: usize,
    pub applied: Vec<InterfaceConfiguration>,
    interface_live: bool,
    /// When set, the next `apply` call fails with `Rejected` instead of
    /// recording its config, then clears itself. Lets tests simulate a real
    /// `configure_interface` rejection (bad config) without a real WireGuard
    /// runtime — in particular to exercise a caller's cleanup-on-failure
    /// path (see `apply_tunnel_plan`).
    pub reject_next_apply: bool,
}

impl WireGuardEffect for FakeWireGuardEffect {
    type Error = FakeWireGuardEffectError;

    fn create_interface(&mut self) -> Result<(), Self::Error> {
        if self.interface_live {
            return Err(FakeWireGuardEffectError::AlreadyCreated);
        }
        self.create_calls += 1;
        self.interface_live = true;
        Ok(())
    }

    fn apply(&mut self, config: &InterfaceConfiguration) -> Result<(), Self::Error> {
        if !self.interface_live {
            return Err(FakeWireGuardEffectError::NotCreated);
        }
        if self.reject_next_apply {
            self.reject_next_apply = false;
            return Err(FakeWireGuardEffectError::Rejected);
        }
        self.applied.push(config.clone());
        Ok(())
    }

    fn remove_interface(&mut self) -> Result<(), Self::Error> {
        if !self.interface_live {
            return Err(FakeWireGuardEffectError::NotCreated);
        }
        self.remove_calls += 1;
        self.interface_live = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use defguard_wireguard_rs::{InterfaceConfiguration, key::Key, net::IpAddrMask, peer::Peer};
    use std::net::{IpAddr, Ipv4Addr};

    fn sample_config() -> InterfaceConfiguration {
        let mut peer = Peer::new(Key::new([7u8; 32]));
        peer.set_allowed_ips(vec![IpAddrMask::new(
            IpAddr::V4(Ipv4Addr::new(100, 64, 0, 2)),
            32,
        )]);
        InterfaceConfiguration {
            name: "ducktape-wg0".into(),
            prvkey: "cHJpdmF0ZS1rZXktYmFzZTY0LXBsYWNlaG9sZGVy".into(),
            addresses: vec![IpAddrMask::new(
                IpAddr::V4(Ipv4Addr::new(100, 64, 0, 1)),
                32,
            )],
            port: 51820,
            peers: vec![peer],
            mtu: None,
            fwmark: None,
        }
    }

    #[test]
    fn fake_records_lifecycle_and_applied_config() {
        let mut fake = FakeWireGuardEffect::default();
        fake.create_interface().unwrap();
        fake.apply(&sample_config()).unwrap();
        fake.remove_interface().unwrap();

        assert_eq!(fake.create_calls, 1);
        assert_eq!(fake.remove_calls, 1);
        assert_eq!(fake.applied.len(), 1);
        assert_eq!(fake.applied[0].name, "ducktape-wg0");
        assert_eq!(fake.applied[0].peers[0].public_key.as_array(), [7u8; 32]);
    }

    #[test]
    fn apply_before_create_interface_is_rejected_like_the_real_adapter() {
        // The real `DefguardWireGuardEffect::apply` calls
        // `WGApi::configure_interface`, which connects to
        // `/var/run/wireguard/<ifname>.sock` — a socket `create_interface`
        // creates. Calling `apply` first has nothing to connect to on the
        // real path, so the fake must reject it too instead of recording a
        // config for an interface that (per the real adapter) never came up.
        let mut fake = FakeWireGuardEffect::default();

        let err = fake.apply(&sample_config()).unwrap_err();

        assert_eq!(err, FakeWireGuardEffectError::NotCreated);
        assert!(fake.applied.is_empty());
    }

    #[test]
    fn create_interface_while_live_is_rejected_like_the_real_adapter() {
        // The real path's `DeviceHandle::new` fails when a TUN of the same
        // name already exists — reconfiguring a live interface goes through
        // `apply` (create-or-replace), never a second `create_interface`.
        let mut fake = FakeWireGuardEffect::default();
        fake.create_interface().unwrap();

        let err = fake.create_interface().unwrap_err();

        assert_eq!(err, FakeWireGuardEffectError::AlreadyCreated);
        assert_eq!(fake.create_calls, 1);
    }

    #[test]
    fn remove_interface_before_create_interface_is_rejected() {
        let mut fake = FakeWireGuardEffect::default();

        let err = fake.remove_interface().unwrap_err();

        assert_eq!(err, FakeWireGuardEffectError::NotCreated);
        assert_eq!(fake.remove_calls, 0);
    }

    #[test]
    fn apply_after_remove_interface_is_rejected() {
        let mut fake = FakeWireGuardEffect::default();
        fake.create_interface().unwrap();
        fake.remove_interface().unwrap();

        let err = fake.apply(&sample_config()).unwrap_err();

        assert_eq!(err, FakeWireGuardEffectError::NotCreated);
        assert!(fake.applied.is_empty());
    }
}
