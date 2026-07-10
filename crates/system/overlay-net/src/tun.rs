//! the TUN pass-through backend — phase 0's only overlay arm.
//!
//! in TUN mode the kernel owns overlay routing: the reachability plane
//! configures a WireGuard interface, the OS routes the chain's ULA `/48`
//! through it, and an ordinary OS socket bound or dialed on an overlay
//! address just works. so this backend's answer to "carry this overlay
//! connection" IS the inner context's OS socket, verbatim — the seam exists,
//! the behavior is bit-identical.
//!
//! the userspace backend (ADR phase 1) replaces these delegations with an
//! in-process boringtun `Tunn` table + smoltcp host; nothing above the seam
//! changes.

use std::net::SocketAddr;

use commonware_runtime::{Error, Network, SinkOf, StreamOf};

/// carry an overlay bind on the OS network (the kernel routes the ULA to the
/// WireGuard interface).
pub(crate) async fn bind<E: Network>(os: &E, socket: SocketAddr) -> Result<E::Listener, Error> {
    os.bind(socket).await
}

/// carry an overlay dial on the OS network (ditto).
pub(crate) async fn dial<E: Network>(
    os: &E,
    socket: SocketAddr,
) -> Result<(SinkOf<E>, StreamOf<E>), Error> {
    os.dial(socket).await
}
