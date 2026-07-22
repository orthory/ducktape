//! the OS pass-through backend: a node with NO reachability plane has no
//! overlay, so this backend's answer to "carry this overlay connection" IS
//! the inner context's OS socket, verbatim — overlay dials just fail like a
//! downed interface, and everything else routes normally. the seam exists,
//! the behavior is the plain OS network.
//!
//! the userspace backend replaces these delegations with an in-process
//! boringtun `Tunn` table + smoltcp host; nothing above the seam changes.

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
