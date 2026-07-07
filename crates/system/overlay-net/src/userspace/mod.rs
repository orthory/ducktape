//! the userspace overlay backend — ADR phase 1
//! (docs/adr/2026-07-07-userspace-overlay-net.mdx): the TUN-less data plane.
//!
//! three layers, one per module, wired together by
//! [`effect::UserspaceWireGuardEffect`]:
//!
//! - [`device`] — the WireGuard half: one process-owned underlay UDP socket,
//!   one boringtun `Tunn` per peer keyed by allowed-ip `/128`, pumps for
//!   encapsulate/decapsulate, and the timer machinery (handshake retry,
//!   keepalive, rekey) that used to be defguard's device layer's job.
//! - [`stack`] — the host half: a smoltcp interface bound to the node's
//!   overlay ULA — what the kernel's TCP/IP stack did for the TUN backend,
//!   in-process. its async socket surface (UDP, TCP dial/listen) lives in
//!   [`sockets`].
//! - [`effect`] — the orchestration boundary: `WireGuardEffect`
//!   (`create_interface` / `apply` / `remove_interface`), so the
//!   reachability orchestrator, epoch cutover, standby pre-warm, and cold
//!   restart drive this backend exactly as they drive the TUN one.
//!
//! no TUN, no privilege, no external binaries, no host mutation: everything
//! here is ordinary process state. wire compatibility with tun-mode nodes is
//! by construction — same Noise handshake, same keys, same wire format,
//! carried by the same boringtun build defguard embeds.

pub mod device;
pub mod effect;
pub mod sockets;
pub mod stack;

pub use device::{PeerConfig, WgDevice};
pub use effect::{UserspaceEffectError, UserspaceWireGuardEffect};
pub use sockets::{VirtualTcpListener, VirtualTcpStream, VirtualUdpSocket};
pub use stack::VirtualStack;
