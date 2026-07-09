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
use std::sync::{Arc, Mutex};
use std::time::Duration;

use data_plane::{BoxFuture, DatagramSocket, PlaneStream, SocketFactory, StreamListener};
use tokio::time::{sleep, timeout};

use super::sockets::{VirtualTcpListener, VirtualUdpSocket};
use super::stack::{LISTEN_BACKLOG, StackSlot, VirtualStack};

/// Bound data-plane sockets outlive one reachability epoch. Socket-mode
/// retargeting deliberately removes and recreates the virtual interface, so
/// listeners/datagram sockets must notice the new stack and rebind just like
/// the commonware mesh seam's lazy listener does.
const REBIND_POLL: Duration = Duration::from_secs(1);

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
            let socket = Arc::new(stack.bind_udp(addr.port())?);
            Ok(Box::new(RebindingVirtualDatagramSocket {
                slot: self.slot.clone(),
                local: addr,
                inner: Mutex::new(Some((stack, socket))),
            }) as Box<dyn DatagramSocket>)
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
            Ok(Box::new(RebindingVirtualStreamListener {
                slot: self.slot.clone(),
                local: addr,
                inner: tokio::sync::Mutex::new(Some((stack, listener))),
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

struct RebindingVirtualDatagramSocket {
    slot: StackSlot,
    local: SocketAddr,
    inner: Mutex<Option<(Arc<VirtualStack>, Arc<VirtualUdpSocket>)>>,
}

impl RebindingVirtualDatagramSocket {
    fn current(&self) -> io::Result<(Arc<VirtualStack>, Arc<VirtualUdpSocket>)> {
        let stack = self.slot.get().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "overlay interface is not up")
        })?;
        require_local(&stack, self.local.ip())?;
        let mut inner = self.inner.lock().expect("virtual datagram socket lock");
        let stale = inner
            .as_ref()
            .is_none_or(|(bound_on, _)| !Arc::ptr_eq(bound_on, &stack));
        if stale {
            let socket = Arc::new(stack.bind_udp(self.local.port())?);
            *inner = Some((Arc::clone(&stack), socket));
        }
        let (_, socket) = inner.as_ref().expect("socket installed above");
        Ok((stack, Arc::clone(socket)))
    }

    fn generation_is_current(&self, generation: &Arc<VirtualStack>) -> bool {
        self.slot
            .get()
            .is_some_and(|current| Arc::ptr_eq(&current, generation))
    }
}

impl DatagramSocket for RebindingVirtualDatagramSocket {
    fn send_to<'a>(&'a self, buf: &'a [u8], dest: SocketAddr) -> BoxFuture<'a, io::Result<usize>> {
        Box::pin(async move {
            loop {
                // A sender sees an actually-down interface immediately, as
                // the OS/TUN arm does. Only an operation caught across a live
                // generation swap is transparently retried on the new stack.
                let (generation, socket) = self.current()?;
                match timeout(REBIND_POLL, socket.send_to(buf, dest)).await {
                    Ok(result) => return result,
                    Err(_) if !self.generation_is_current(&generation) => continue,
                    // A full but live UDP queue retains normal backpressure;
                    // the poll timeout exists only to observe generation
                    // changes, not to impose a new transport deadline.
                    Err(_) => continue,
                }
            }
        })
    }

    fn recv_from<'a>(
        &'a self,
        buf: &'a mut [u8],
    ) -> BoxFuture<'a, io::Result<(usize, SocketAddr)>> {
        Box::pin(async move {
            loop {
                let (generation, socket) = match self.current() {
                    Ok(current) => current,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        sleep(REBIND_POLL).await;
                        continue;
                    }
                    Err(error) => return Err(error),
                };
                match timeout(REBIND_POLL, socket.recv_from(buf)).await {
                    Ok(result) => return result,
                    Err(_) if !self.generation_is_current(&generation) => continue,
                    // No datagram yet on the same generation. Re-entering the
                    // receive is lossless: the virtual socket remains bound.
                    Err(_) => {}
                }
            }
        })
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.local)
    }
}

/// [`StreamListener`] over the virtual acceptor: the trait accepts through
/// `&self` (the plane shares its listener), the virtual accept needs `&mut`
/// (it re-arms handshake slots) — an async mutex bridges the two. one
/// accept at a time is exactly the plane's usage (a single acceptor task).
struct RebindingVirtualStreamListener {
    slot: StackSlot,
    local: SocketAddr,
    inner: tokio::sync::Mutex<Option<(Arc<VirtualStack>, VirtualTcpListener)>>,
}

impl StreamListener for RebindingVirtualStreamListener {
    fn accept(&self) -> BoxFuture<'_, io::Result<(PlaneStream, SocketAddr)>> {
        Box::pin(async {
            loop {
                let Some(stack) = self.slot.get() else {
                    *self.inner.lock().await = None;
                    sleep(REBIND_POLL).await;
                    continue;
                };
                require_local(&stack, self.local.ip())?;
                let mut inner = self.inner.lock().await;
                let stale = inner
                    .as_ref()
                    .is_none_or(|(bound_on, _)| !Arc::ptr_eq(bound_on, &stack));
                if stale {
                    match stack.listen_tcp(self.local.port(), LISTEN_BACKLOG) {
                        Ok(listener) => *inner = Some((Arc::clone(&stack), listener)),
                        Err(_) => {
                            *inner = None;
                            drop(inner);
                            sleep(REBIND_POLL).await;
                            continue;
                        }
                    }
                }
                let Some((_, listener)) = inner.as_mut() else {
                    continue;
                };
                match timeout(REBIND_POLL, listener.accept()).await {
                    Ok(Ok((stream, addr))) => {
                        return Ok((Box::new(stream) as PlaneStream, addr));
                    }
                    Ok(Err(_)) => *inner = None,
                    Err(_) => {}
                }
            }
        })
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.local)
    }
}
