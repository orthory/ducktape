use defguard_wireguard_rs::{
    InterfaceConfiguration, Userspace, WGApi, WireguardInterfaceApi, error::WireguardInterfaceError,
};

use crate::WireGuardEffect;

/// Real `WireGuardEffect` backed by `defguard_wireguard_rs`'s userspace
/// (BoringTun) implementation. BoringTun runs in-process
/// (`defguard_boringtun::device::DeviceHandle`): `create_interface` opens
/// the TUN device and binds the UAPI socket at
/// `/var/run/wireguard/<ifname>.sock` itself — there is no external runtime
/// to install or start. Not exercised by the automated test suite:
/// `create_interface`/`apply` need a privileged unix host (root or
/// `CAP_NET_ADMIN`, plus `/dev/net/tun`), and `remove_interface` shells out
/// to `resolvconf` for DNS cleanup — missing binary = `IoError(NotFound)`
/// at teardown. Verify manually with the `real_userspace_lifecycle_smoke`
/// `#[ignore]`d test below: `cargo test -p wireguard-effect -- --ignored
/// real_userspace_lifecycle_smoke` on such a host (a privileged Linux
/// container works), then confirm with `ip addr show <ifname>` and
/// `wg show <ifname>`.
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

        // The name must fit IFNAMSIZ - 1 (15 chars) or BoringTun rejects it
        // with `InvalidTunnelName`; production names ("dt-" + 8 hex) always
        // do, so keep the fixture inside the same bound.
        let mut effect = DefguardWireGuardEffect::new("dt-smoke0").unwrap();
        effect.create_interface().unwrap();

        let mut peer = Peer::new(Key::new([9u8; 32]));
        peer.set_allowed_ips(vec![IpAddrMask::new(
            IpAddr::V4(Ipv4Addr::new(100, 64, 0, 2)),
            32,
        )]);
        let config = InterfaceConfiguration {
            name: "dt-smoke0".into(),
            // A real 32-byte key: `Key::try_from(&str)` rejects anything
            // else, so a shorter placeholder would fail `apply` before the
            // UAPI socket is ever touched.
            prvkey: "AQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHyA=".into(),
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
