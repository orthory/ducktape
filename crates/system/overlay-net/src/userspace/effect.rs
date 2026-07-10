//! the userspace backend behind the orchestration boundary: a
//! [`WireGuardEffect`] whose "interface" is the in-process
//! [`WgDevice`] + [`VirtualStack`]
//! pair instead of a TUN.
//!
//! the ADR's rule is that the orchestration boundary does not move: the
//! reachability orchestrator, epoch cutover, standby pre-warm, and cold
//! restart keep driving tunnels through `create_interface` / `apply` /
//! `remove_interface` with the same `InterfaceConfiguration`. this adapter
//! maps that contract onto the userspace core:
//!
//! - `create_interface` marks the interface live (the create-before-apply
//!   gate every other effect enforces). the actual allocation happens on the
//!   FIRST `apply` — the listen port and overlay address only arrive with
//!   the configuration, so there is nothing real to allocate earlier.
//! - `apply` binds the underlay UDP socket and stands up device + stack on
//!   first call; on re-apply it replaces the peer set and the host ULA
//!   atomically. an unchanged peer keeps its live sessions (the mid-epoch
//!   re-apply contract `update_peer_tunnels` documents); a changed listen
//!   port or private key is an interface replacement — the backend is
//!   rebuilt, sessions and all, exactly as a TUN would be torn down and
//!   re-created.
//! - `remove_interface` drops the backend: pumps abort, the socket closes,
//!   nothing to clean up on the host — the whole point of this backend.

use std::io;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::sync::Arc;

use defguard_wireguard_rs::{InterfaceConfiguration, key::Key};
use tokio::sync::mpsc;
use wireguard_effect::WireGuardEffect;

use super::device::{PeerConfig, UnderlaySocket, WgDevice};
use super::stack::{StackSlot, VirtualStack};

/// the chain overlay's on-link scope: member `/128`s live in the chain's ULA
/// `/48` (`ula_v6_prefix`), and inside the tunnel that whole `/48` IS the
/// link — cryptokey routing (the device's allowed-ip table) is the switch
/// fabric, so the stack needs no route table beyond it. the same 48 bits
/// `OverlayRouter::for_prefix48` routes by.
const OVERLAY_ONLINK_PREFIX: u8 = 48;

/// capacity of the device↔stack packet channels; at the 1420-byte tunnel MTU
/// this bounds each direction's in-flight buffer around 1.4 MB.
const PACKET_CHANNEL: usize = 1024;

/// why the userspace effect refused a lifecycle call or a configuration.
#[derive(Debug)]
pub enum UserspaceEffectError {
    /// `apply`/`remove_interface` without a live `create_interface` — the
    /// same ordering invariant `FakeWireGuardEffect` and the Defguard path
    /// enforce.
    NotCreated,
    /// `create_interface` while the interface is already live.
    AlreadyCreated,
    /// `prvkey` is not a valid base64 32-byte X25519 key.
    InvalidPrivateKey,
    /// no v6 overlay address on the interface — the userspace backend is
    /// ULA-v6 only (the shipped overlay mode); v4 overlays need `tun`.
    NoOverlayAddress,
    /// a peer allowed-ip the userspace backend cannot cryptokey-route: only
    /// v6 `/128`s are supported. carries the offending entry, loudly —
    /// silently ignoring it would leave a peer half-reachable.
    UnsupportedAllowedIp(String),
    /// binding the underlay UDP socket failed.
    Bind(io::Error),
    /// the configuration names a listen port other than the one the shared
    /// underlay socket is bound to. the shared socket is bound once at plane
    /// start (the NAT punch rides it), so a port change cannot be honored —
    /// and the two values come from the same `wireguard_listen` config, so
    /// divergence is a wiring bug to surface, not roll with.
    PortMismatch { configured: u16, bound: u16 },
}

/// what `apply` needs, decoded and validated out of defguard's
/// stringly/masked `InterfaceConfiguration`.
struct ParsedConfig {
    private_key: [u8; 32],
    port: u16,
    ula: Ipv6Addr,
    peers: Vec<PeerConfig>,
}

fn parse_config(config: &InterfaceConfiguration) -> Result<ParsedConfig, UserspaceEffectError> {
    let private_key = Key::try_from(config.prvkey.as_str())
        .map_err(|_| UserspaceEffectError::InvalidPrivateKey)?
        .as_array();
    let port = config.port;
    let ula = config
        .addresses
        .iter()
        .find_map(|mask| match mask.address {
            IpAddr::V6(v6) => Some(v6),
            IpAddr::V4(_) => None,
        })
        .ok_or(UserspaceEffectError::NoOverlayAddress)?;
    let peers = config
        .peers
        .iter()
        .map(|peer| {
            let allowed_ips = peer
                .allowed_ips
                .iter()
                .map(|mask| match (mask.address, mask.cidr) {
                    (IpAddr::V6(v6), 128) => Ok(v6),
                    _ => Err(UserspaceEffectError::UnsupportedAllowedIp(format!(
                        "{}/{}",
                        mask.address, mask.cidr
                    ))),
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(PeerConfig {
                public_key: peer.public_key.as_array(),
                preshared_key: peer.preshared_key.as_ref().map(|key| key.as_array()),
                endpoint: peer.endpoint,
                persistent_keepalive: peer.persistent_keepalive_interval,
                allowed_ips,
            })
        })
        .collect::<Result<Vec<_>, UserspaceEffectError>>()?;
    Ok(ParsedConfig {
        private_key,
        port,
        ula,
        peers,
    })
}

/// the live backend: everything the first `apply` stood up.
struct Backend {
    device: WgDevice,
    stack: Arc<VirtualStack>,
    /// the RESOLVED underlay port (a configured port 0 recorded as what the
    /// OS allocated) — so a re-apply naming the resolved port is recognized
    /// as the same interface, not a port change.
    port: u16,
    private_key: [u8; 32],
}

/// `WireGuardEffect` over the in-process userspace overlay. construct once
/// per interface (like `DefguardWireGuardEffect::new`), with the tokio
/// runtime the pumps should live on injected.
pub struct UserspaceWireGuardEffect {
    handle: tokio::runtime::Handle,
    /// `Some` between `create_interface` and `remove_interface`; the inner
    /// backend is `Some` from the first successful `apply`.
    live: Option<Option<Backend>>,
    /// the seam's handle to the live stack (ADR phase 2): published on the
    /// `apply` that stands a backend up, cleared whenever the backend drops.
    slot: StackSlot,
    /// a node-owned underlay socket shared with the NAT punch (ADR phase 3),
    /// reused across interface rebuilds instead of binding per backend;
    /// `None` = each backend binds its own (the standalone posture the
    /// loopback proofs and the interop probe use).
    shared_underlay: Option<Arc<UnderlaySocket>>,
}

impl UserspaceWireGuardEffect {
    pub fn new(handle: tokio::runtime::Handle) -> Self {
        Self {
            handle,
            live: None,
            slot: StackSlot::new(),
            shared_underlay: None,
        }
    }

    /// the node wiring (ADR phase 3): the seam's slot is created by the node
    /// before the reachability plane's thread exists (the mesh context and
    /// the data-plane factory consume it), and the underlay socket is bound
    /// at plane start so the NAT client shares the tunnel's 5-tuple — this
    /// effect attaches every backend it builds to both.
    pub fn with_shared_underlay(
        handle: tokio::runtime::Handle,
        slot: StackSlot,
        underlay: Arc<UnderlaySocket>,
    ) -> Self {
        Self {
            handle,
            live: None,
            slot,
            shared_underlay: Some(underlay),
        }
    }

    /// the publishable stack handle the overlay seam ([`crate::OverlayBackend`])
    /// and the data-plane factory ([`super::VirtualSocketFactory`]) consume.
    /// tracks this effect's backend across rebuilds for the effect's lifetime.
    pub fn stack_slot(&self) -> StackSlot {
        self.slot.clone()
    }

    /// the virtual host, for the seam's socket surface (and the loopback
    /// proofs). `None` before the first `apply`.
    pub fn stack(&self) -> Option<Arc<VirtualStack>> {
        self.live
            .as_ref()?
            .as_ref()
            .map(|backend| backend.stack.clone())
    }

    /// the WireGuard device, for handshake probes and (later) sharing the
    /// underlay socket with the NAT punch. `None` before the first `apply`.
    pub fn device(&self) -> Option<&WgDevice> {
        self.live.as_ref()?.as_ref().map(|backend| &backend.device)
    }

    /// the underlay address the backend actually bound (resolves port 0).
    pub fn local_underlay_addr(&self) -> Option<SocketAddr> {
        self.device()?.local_underlay_addr().ok()
    }
}

fn build_backend(
    handle: &tokio::runtime::Handle,
    parsed: &ParsedConfig,
    shared_underlay: Option<&Arc<UnderlaySocket>>,
) -> Result<Backend, UserspaceEffectError> {
    // one process-owned underlay socket per node: the WG listen endpoint,
    // dual-stack so a peer endpoint of either family reaches it — the
    // node-owned shared socket (which the NAT punch also rides) when
    // injected, a fresh bind per backend in the standalone posture.
    let underlay = match shared_underlay {
        Some(underlay) => underlay.clone(),
        None => UnderlaySocket::bind(handle, parsed.port).map_err(UserspaceEffectError::Bind)?,
    };
    let port = underlay
        .local_addr()
        .map_err(UserspaceEffectError::Bind)?
        .port();

    let (to_stack, from_device) = mpsc::channel(PACKET_CHANNEL);
    let (to_device, from_stack) = mpsc::channel(PACKET_CHANNEL);
    let stack = Arc::new(VirtualStack::spawn(
        handle,
        parsed.ula,
        OVERLAY_ONLINK_PREFIX,
        from_device,
        to_device,
    ));
    let device = WgDevice::spawn(
        handle,
        underlay,
        parsed.private_key.into(),
        to_stack,
        from_stack,
    );
    Ok(Backend {
        device,
        stack,
        port,
        private_key: parsed.private_key,
    })
}

impl WireGuardEffect for UserspaceWireGuardEffect {
    type Error = UserspaceEffectError;

    fn create_interface(&mut self) -> Result<(), Self::Error> {
        if self.live.is_some() {
            return Err(UserspaceEffectError::AlreadyCreated);
        }
        self.live = Some(None);
        Ok(())
    }

    fn apply(&mut self, config: &InterfaceConfiguration) -> Result<(), Self::Error> {
        let Some(live) = self.live.as_mut() else {
            return Err(UserspaceEffectError::NotCreated);
        };
        let parsed = parse_config(config)?;

        // a shared underlay's port is fixed for the process life (the NAT
        // punch rides it): refuse a diverging config up front, before any
        // teardown.
        if let Some(underlay) = &self.shared_underlay {
            let bound = underlay
                .local_addr()
                .map_err(UserspaceEffectError::Bind)?
                .port();
            if parsed.port != 0 && parsed.port != bound {
                return Err(UserspaceEffectError::PortMismatch {
                    configured: parsed.port,
                    bound,
                });
            }
        }

        // a changed listen port or identity key is an interface replacement,
        // not a reconfiguration: rebuild the backend. (drop first, so a
        // same-port rebind does not race the old socket; clear the published
        // stack with it — until the rebuild lands, the tunnel is down.)
        // port 0 on a re-apply means "any" and never forces a rebuild.
        if live.as_ref().is_some_and(|backend| {
            (parsed.port != 0 && backend.port != parsed.port)
                || backend.private_key != parsed.private_key
        }) {
            self.slot.clear();
            *live = None;
        }

        match live {
            Some(backend) => {
                backend.device.replace_peers(&parsed.peers);
                backend
                    .stack
                    .set_local_ip(parsed.ula, OVERLAY_ONLINK_PREFIX);
            }
            None => {
                let backend = build_backend(&self.handle, &parsed, self.shared_underlay.as_ref())?;
                backend.device.replace_peers(&parsed.peers);
                self.slot.publish(backend.stack.clone());
                *live = Some(backend);
            }
        }
        Ok(())
    }

    fn remove_interface(&mut self) -> Result<(), Self::Error> {
        if self.live.take().is_none() {
            return Err(UserspaceEffectError::NotCreated);
        }
        self.slot.clear();
        // dropping the backend aborts the pumps and closes the socket —
        // there is no host state (no device node, no routes, no DNS) to
        // clean up, which is the point of this backend.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use defguard_wireguard_rs::net::IpAddrMask;
    use defguard_wireguard_rs::peer::Peer;

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("test runtime")
    }

    fn sample_config() -> InterfaceConfiguration {
        let mut peer = Peer::new(Key::new([7u8; 32]));
        peer.set_allowed_ips(vec![IpAddrMask::new(
            "fda2:8ad3:eaee::7".parse().unwrap(),
            128,
        )]);
        InterfaceConfiguration {
            name: "dt-userspace0".into(),
            prvkey: "AQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHyA=".into(),
            addresses: vec![IpAddrMask::new("fda2:8ad3:eaee::1".parse().unwrap(), 128)],
            // port 0: the OS allocates — tests must not claim fixed ports.
            port: 0,
            peers: vec![peer],
            mtu: None,
            fwmark: None,
        }
    }

    #[test]
    fn lifecycle_ordering_matches_the_other_effects() {
        let runtime = runtime();
        let mut effect = UserspaceWireGuardEffect::new(runtime.handle().clone());

        // apply / remove before create: rejected, like Fake and Defguard.
        assert!(matches!(
            effect.apply(&sample_config()),
            Err(UserspaceEffectError::NotCreated)
        ));
        assert!(matches!(
            effect.remove_interface(),
            Err(UserspaceEffectError::NotCreated)
        ));

        effect.create_interface().unwrap();
        assert!(matches!(
            effect.create_interface(),
            Err(UserspaceEffectError::AlreadyCreated)
        ));

        effect.apply(&sample_config()).unwrap();
        assert!(effect.stack().is_some(), "first apply allocates the stack");
        let bound = effect.local_underlay_addr().expect("underlay bound");
        assert_ne!(bound.port(), 0, "port 0 resolved to a real allocation");

        // the remove→create→apply replace cycle (epoch cutover shape) — on
        // the SAME resolved port, so this also proves the rebind absorbs the
        // predecessor's asynchronous socket release.
        effect.remove_interface().unwrap();
        assert!(effect.stack().is_none(), "remove drops the backend");
        effect.create_interface().unwrap();
        let mut same_port = sample_config();
        same_port.port = bound.port();
        effect.apply(&same_port).unwrap();
        assert_eq!(
            effect.local_underlay_addr().expect("rebound").port(),
            bound.port(),
            "the replacement claimed the same underlay port"
        );
        effect.remove_interface().unwrap();
    }

    #[test]
    fn config_validation_is_loud() {
        let runtime = runtime();
        let mut effect = UserspaceWireGuardEffect::new(runtime.handle().clone());
        effect.create_interface().unwrap();

        let mut bad_key = sample_config();
        bad_key.prvkey = "not-a-key".into();
        assert!(matches!(
            effect.apply(&bad_key),
            Err(UserspaceEffectError::InvalidPrivateKey)
        ));

        let mut no_ula = sample_config();
        no_ula.addresses = vec![IpAddrMask::new("100.64.0.1".parse().unwrap(), 32)];
        assert!(matches!(
            effect.apply(&no_ula),
            Err(UserspaceEffectError::NoOverlayAddress)
        ));

        let mut coarse_route = sample_config();
        coarse_route.peers[0].set_allowed_ips(vec![IpAddrMask::new(
            "fda2:8ad3:eaee::".parse().unwrap(),
            48,
        )]);
        assert!(matches!(
            effect.apply(&coarse_route),
            Err(UserspaceEffectError::UnsupportedAllowedIp(_))
        ));
    }
}
