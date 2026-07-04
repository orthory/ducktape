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

/// Deterministic in-memory `WireGuardEffect` for tests: records every applied
/// configuration and lifecycle call instead of touching a real network
/// interface. CI has no WireGuard userspace runtime, so this is the only
/// `WireGuardEffect` the automated test suite exercises.
#[derive(Default)]
pub struct FakeWireGuardEffect {
    pub create_calls: usize,
    pub remove_calls: usize,
    pub applied: Vec<InterfaceConfiguration>,
}

impl WireGuardEffect for FakeWireGuardEffect {
    type Error = std::convert::Infallible;

    fn create_interface(&mut self) -> Result<(), Self::Error> {
        self.create_calls += 1;
        Ok(())
    }

    fn apply(&mut self, config: &InterfaceConfiguration) -> Result<(), Self::Error> {
        self.applied.push(config.clone());
        Ok(())
    }

    fn remove_interface(&mut self) -> Result<(), Self::Error> {
        self.remove_calls += 1;
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
}
