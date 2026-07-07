//! the userspace arm of data-plane's socket seam — ADR phase 2: a
//! [`SocketFactory`] whose sockets terminate in the [`VirtualStack`] instead
//! of the kernel, so `OverlaySockets` (statesync's per-use plane, and every
//! future plane) rides the in-process tunnel with no change in the plane or
//! any consumer.
//!
//! the factory resolves the live stack from the [`StackSlot`] PER CALL: an
//! empty slot (tunnel not up yet, or mid-rebuild) surfaces as an `io::Error`,
//! which the node's bring-up retry loop already absorbs — the exact behavior
//! the OS factory shows while the TUN interface's `/128` is still absent.
//!
//! the `/128` bind invariant data-plane documents (bind the node's overlay
//! address, never a wildcard) is enforced here rather than assumed: the
//! virtual host owns exactly one address, and a caller naming any other is a
//! routing bug surfaced loudly.

use std::io;
use std::net::{IpAddr, SocketAddr};

use data_plane::{BoxFuture, DatagramSocket, PlaneStream, SocketFactory, StreamListener};

use super::sockets::{VirtualTcpListener, VirtualUdpSocket};
use super::stack::{LISTEN_BACKLOG, StackSlot, VirtualStack};

/// mints the plane's sockets on the userspace backend's virtual host.
pub struct VirtualSocketFactory {
    slot: StackSlot,
}

impl VirtualSocketFactory {
    pub fn new(slot: StackSlot) -> Self {
        Self { slot }
    }

    fn stack(&self) -> io::Result<std::sync::Arc<VirtualStack>> {
        self.slot
            .get()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "overlay interface is not up"))
    }
}

/// the one-address invariant: the virtual host answers only at the node's
/// own overlay `/128`.
fn require_local(stack: &VirtualStack, ip: IpAddr) -> io::Result<()> {
    if ip == IpAddr::V6(stack.local_ip()) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::AddrNotAvailable,
            format!("{ip} is not this node's overlay /128"),
        ))
    }
}

impl SocketFactory for VirtualSocketFactory {
    fn bind_udp(&self, addr: SocketAddr) -> BoxFuture<'_, io::Result<Box<dyn DatagramSocket>>> {
        Box::pin(async move {
            let stack = self.stack()?;
            require_local(&stack, addr.ip())?;
            Ok(Box::new(stack.bind_udp(addr.port())?) as Box<dyn DatagramSocket>)
        })
    }

    fn bind_listener(
        &self,
        addr: SocketAddr,
    ) -> BoxFuture<'_, io::Result<Box<dyn StreamListener>>> {
        Box::pin(async move {
            let stack = self.stack()?;
            require_local(&stack, addr.ip())?;
            let listener = stack.listen_tcp(addr.port(), LISTEN_BACKLOG)?;
            let local = listener.local_addr();
            Ok(Box::new(VirtualStreamListener {
                inner: tokio::sync::Mutex::new(listener),
                local,
            }) as Box<dyn StreamListener>)
        })
    }

    fn dial_from<'a>(
        &'a self,
        local_ip: IpAddr,
        dest: SocketAddr,
    ) -> BoxFuture<'a, io::Result<PlaneStream>> {
        Box::pin(async move {
            let stack = self.stack()?;
            // the far side authenticates by source /128; the virtual host can
            // only ever present its own, so any other request is a bug.
            require_local(&stack, local_ip)?;
            Ok(Box::new(stack.connect_tcp(dest).await?) as PlaneStream)
        })
    }
}

impl DatagramSocket for VirtualUdpSocket {
    fn send_to<'a>(&'a self, buf: &'a [u8], dest: SocketAddr) -> BoxFuture<'a, io::Result<usize>> {
        Box::pin(VirtualUdpSocket::send_to(self, buf, dest))
    }

    fn recv_from<'a>(
        &'a self,
        buf: &'a mut [u8],
    ) -> BoxFuture<'a, io::Result<(usize, SocketAddr)>> {
        Box::pin(VirtualUdpSocket::recv_from(self, buf))
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        Ok(VirtualUdpSocket::local_addr(self))
    }
}

/// [`StreamListener`] over the virtual acceptor: the trait accepts through
/// `&self` (the plane shares its listener), the virtual accept needs `&mut`
/// (it re-arms handshake slots) — an async mutex bridges the two. one
/// accept at a time is exactly the plane's usage (a single acceptor task).
struct VirtualStreamListener {
    inner: tokio::sync::Mutex<VirtualTcpListener>,
    local: SocketAddr,
}

impl StreamListener for VirtualStreamListener {
    fn accept(&self) -> BoxFuture<'_, io::Result<(PlaneStream, SocketAddr)>> {
        Box::pin(async {
            let (stream, addr) = self.inner.lock().await.accept().await?;
            Ok((Box::new(stream) as PlaneStream, addr))
        })
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.local)
    }
}
