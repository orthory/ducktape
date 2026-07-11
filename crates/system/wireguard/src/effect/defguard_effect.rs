use defguard_wireguard_rs::{
    InterfaceConfiguration, Userspace, WGApi, WireguardInterfaceApi, error::WireguardInterfaceError,
};

use crate::effect::WireGuardEffect;

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
/// `#[ignore]`d test below: `cargo test -p wireguard -- --ignored
/// real_userspace_lifecycle_smoke` on such a host (a privileged Linux
/// container works), then confirm with `ip addr show <ifname>` and
/// `wg show <ifname>`.
pub struct DefguardWireGuardEffect {
    ifname: String,
    api: WGApi<Userspace>,
}

impl DefguardWireGuardEffect {
    /// Construct the wrapper for the named interface. This only allocates
    /// the `WGApi` handle — it does not touch the network or require
    /// privilege; `create_interface` does.
    pub fn new(ifname: impl Into<String>) -> Result<Self, WireguardInterfaceError> {
        let ifname = ifname.into();
        Ok(Self {
            api: WGApi::<Userspace>::new(ifname.clone())?,
            ifname,
        })
    }
}

impl WireGuardEffect for DefguardWireGuardEffect {
    type Error = WireguardInterfaceError;

    fn create_interface(&mut self) -> Result<(), Self::Error> {
        self.api.create_interface()
    }

    fn apply(&mut self, config: &InterfaceConfiguration) -> Result<(), Self::Error> {
        self.api.configure_interface(config)?;
        // The userspace `configure_interface` assigns addresses and writes
        // the UAPI config but neither flips IFF_UP nor installs allowed-ip
        // routes — the kernel path gets its UP flag inside defguard's
        // (crate-private) `netlink::create_interface`, and peer routing is a
        // separate trait call. Without both, the "applied" tunnel cannot
        // carry a single packet (the two-container smoke surfaced the
        // interface sitting in `state DOWN` with no route to the peer's
        // overlay address). `ip` is the same external tool defguard's own
        // peer routing shells out to on this path.
        #[cfg(target_os = "linux")]
        {
            let status = std::process::Command::new("ip")
                .args(["link", "set", "up", "dev", &config.name])
                .status()?;
            if !status.success() {
                return Err(WireguardInterfaceError::Interface(format!(
                    "`ip link set up dev {}` exited with {status}",
                    config.name
                )));
            }
            self.api.configure_peer_routing(&config.peers)?;
        }
        Ok(())
    }

    fn remove_interface(&mut self) -> Result<(), Self::Error> {
        self.api.remove_interface()?;
        // A remove->create cycle on the same name (epoch cutover replacing
        // the interface; live assembly replacing a restored mesh) trips a
        // lifecycle bug in defguard's userspace `WGApi`: `remove_interface`
        // takes `&self` and CANNOT clear the stale `DeviceHandle` it holds,
        // so the next `create_interface` drops that handle only AFTER the
        // new device has bound the same UAPI socket path — and
        // `DeviceHandle::drop`'s `clean()` then unlinks the NEW device's
        // socket, making the following `configure_interface` fail with
        // `IoError(NotFound)`. Rebuild the `WGApi` here instead, so the
        // stale handle drops NOW, while the socket path is already gone.
        self.api = WGApi::<Userspace>::new(self.ifname.clone())?;
        // The old device's worker threads exit asynchronously (they hold
        // the TUN until they observe the exit trigger); creating a same-name
        // TUN against that window fails. Wait for the device to actually
        // vanish — bounded, and a timeout stays loud: the caller's next
        // `create_interface` is the operation that would break.
        #[cfg(target_os = "linux")]
        {
            let sys = format!("/sys/class/net/{}", self.ifname);
            for _ in 0..200 {
                if !std::path::Path::new(&sys).exists() {
                    return Ok(());
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(WireguardInterfaceError::Interface(format!(
                "interface {} still exists 2s after removal",
                self.ifname
            )))
        }
        #[cfg(not(target_os = "linux"))]
        Ok(())
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
    #[ignore = "requires root + a running WireGuard userspace (BoringTun) runtime; run manually, cross-machine: cargo test -p wireguard -- --ignored real_userspace_lifecycle_smoke"]
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

        // The replace cycle every epoch cutover (and every restored-mesh
        // handoff) runs: remove the live interface and stand it back up
        // under the SAME name. Without the WGApi rebuild in
        // `remove_interface`, the stale DeviceHandle's drop unlinks the new
        // device's UAPI socket and this second `apply` fails NotFound.
        effect.remove_interface().unwrap();
        effect.create_interface().unwrap();
        effect.apply(&config).unwrap();

        // `apply` must leave a USABLE tunnel: link up (the flags string
        // gains `UP`; down is bare `<POINTOPOINT,MULTICAST,NOARP>`) and a
        // route to the peer's allowed ip via this interface.
        #[cfg(target_os = "linux")]
        {
            let link = std::process::Command::new("ip")
                .args(["-o", "link", "show", "dt-smoke0"])
                .output()
                .unwrap();
            let link = String::from_utf8_lossy(&link.stdout).to_string();
            assert!(link.contains("UP"), "link is not up: {link}");
            let route = std::process::Command::new("ip")
                .args(["-4", "route", "get", "100.64.0.2"])
                .output()
                .unwrap();
            let route = String::from_utf8_lossy(&route.stdout).to_string();
            assert!(
                route.contains("dt-smoke0"),
                "peer allowed-ip does not route via the tunnel: {route}"
            );
        }

        // The standby pre-warm cycle: re-apply the FULL config on the LIVE
        // interface (no remove) with the peer set grown by one — the exact
        // call `update_peer_tunnels` makes when a standby's record arrives
        // mid-epoch. Must succeed (same addresses re-assigned, existing peer
        // routes tolerated) and leave both peers configured and routed.
        let mut second = Peer::new(Key::new([8u8; 32]));
        second.set_allowed_ips(vec![IpAddrMask::new(
            IpAddr::V4(Ipv4Addr::new(100, 64, 0, 3)),
            32,
        )]);
        let mut grown = config.clone();
        grown.peers.push(second);
        effect.apply(&grown).unwrap();
        #[cfg(target_os = "linux")]
        {
            let dump = std::process::Command::new("wg")
                .args(["show", "dt-smoke0", "peers"])
                .output()
                .unwrap();
            let peers = String::from_utf8_lossy(&dump.stdout).to_string();
            assert_eq!(
                peers.lines().count(),
                2,
                "the live reconfigure carries both peers: {peers}"
            );
            let route = std::process::Command::new("ip")
                .args(["-4", "route", "get", "100.64.0.3"])
                .output()
                .unwrap();
            let route = String::from_utf8_lossy(&route.stdout).to_string();
            assert!(
                route.contains("dt-smoke0"),
                "the added peer's allowed-ip does not route via the tunnel: {route}"
            );
        }

        effect.remove_interface().unwrap();
    }
}
