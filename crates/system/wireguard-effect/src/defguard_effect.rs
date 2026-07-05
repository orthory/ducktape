use defguard_wireguard_rs::{
    InterfaceConfiguration, Userspace, WGApi, WireguardInterfaceApi, error::WireguardInterfaceError,
};

use crate::WireGuardEffect;

/// Real `WireGuardEffect` backed by `defguard_wireguard_rs`'s userspace
/// (BoringTun) implementation. Not exercised by the automated test suite —
/// CI has no WireGuard userspace runtime, and `create_interface`/`apply`
/// require a privileged host (root or `CAP_NET_ADMIN`) with BoringTun
/// reachable at `/var/run/wireguard/<ifname>.sock`. Verify this path
/// manually, cross-machine, using the `real_userspace_lifecycle_smoke`
/// `#[ignore]`d test below: `cargo test -p wireguard-effect --
/// --ignored real_userspace_lifecycle_smoke` on a Linux box with root, then
/// confirm with `ip addr show <ifname>` and `wg show <ifname>`.
pub struct DefguardWireGuardEffect {
    api: WGApi<Userspace>,
}

impl DefguardWireGuardEffect {
    /// Construct the wrapper for the named interface. This only allocates
    /// the `WGApi` handle — it does not touch the network or require
    /// privilege; `create_interface` does.
    pub fn new(ifname: impl Into<String>) -> Result<Self, WireguardInterfaceError> {
        Ok(Self {
            api: WGApi::<Userspace>::new(ifname)?,
        })
    }
}

impl WireGuardEffect for DefguardWireGuardEffect {
    type Error = WireguardInterfaceError;

    fn create_interface(&mut self) -> Result<(), Self::Error> {
        self.api.create_interface()
    }

    fn apply(&mut self, config: &InterfaceConfiguration) -> Result<(), Self::Error> {
        self.api.configure_interface(config)
    }

    fn remove_interface(&mut self) -> Result<(), Self::Error> {
        self.api.remove_interface()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs_without_touching_network() {
        // `WGApi::new` only stores the interface name; it does not open a
        // socket or require privilege. Safe to run in CI.
        let effect = DefguardWireGuardEffect::new("ducktape-wg-test0");
        assert!(effect.is_ok());
    }

    #[test]
    #[ignore = "requires root + a running WireGuard userspace (BoringTun) runtime; run manually, cross-machine: cargo test -p wireguard-effect -- --ignored real_userspace_lifecycle_smoke"]
    fn real_userspace_lifecycle_smoke() {
        use defguard_wireguard_rs::{key::Key, net::IpAddrMask, peer::Peer};
        use std::net::{IpAddr, Ipv4Addr};

        let mut effect = DefguardWireGuardEffect::new("ducktape-wg-smoke0").unwrap();
        effect.create_interface().unwrap();

        let mut peer = Peer::new(Key::new([9u8; 32]));
        peer.set_allowed_ips(vec![IpAddrMask::new(
            IpAddr::V4(Ipv4Addr::new(100, 64, 0, 2)),
            32,
        )]);
        let config = InterfaceConfiguration {
            name: "ducktape-wg-smoke0".into(),
            prvkey: "cHJpdmF0ZS1rZXktYmFzZTY0LXBsYWNlaG9sZGVy".into(),
            addresses: vec![IpAddrMask::new(
                IpAddr::V4(Ipv4Addr::new(100, 64, 0, 1)),
                32,
            )],
            port: 51820,
            peers: vec![peer],
            mtu: None,
            fwmark: None,
        };
        effect.apply(&config).unwrap();
        effect.remove_interface().unwrap();
    }
}
