//! the async socket surface over the [`stack`](super::stack)'s virtual
//! host: tokio-vocabulary UDP and TCP endpoints whose I/O terminates in
//! smoltcp sockets instead of the kernel.
//!
//! every future here follows one pattern: lock the shared stack state, try
//! the smoltcp operation, and either complete or park on the socket's waker
//! registration (`Interface::poll` fires those wakers on any state change);
//! operations that queue work kick `poll_wake` so the poll task moves the
//! bytes. `VirtualTcpStream` implements `AsyncRead`/`AsyncWrite`, so it
//! slots wherever an OS `TcpStream` does — the shape the data plane's
//! socket factory and the overlay seam consume.

use std::future::poll_fn;
use std::io;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use smoltcp::iface::SocketHandle;
use smoltcp::socket::{tcp, udp};
use smoltcp::wire::{IpEndpoint, IpListenEndpoint};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use super::stack::{StackShared, listen_slot};

/// an async datagram socket at the host's ULA.
pub struct VirtualUdpSocket {
    shared: Arc<StackShared>,
    handle: SocketHandle,
    local: SocketAddr,
}

impl VirtualUdpSocket {
    pub(super) fn new(shared: Arc<StackShared>, handle: SocketHandle, local: SocketAddr) -> Self {
        Self {
            shared,
            handle,
            local,
        }
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local
    }

    pub async fn send_to(&self, data: &[u8], dest: SocketAddr) -> io::Result<usize> {
        poll_fn(|cx| {
            let mut state = self.shared.lock();
            let socket = state.sockets.get_mut::<udp::Socket>(self.handle);
            let endpoint = match addr_to_endpoint(dest) {
                Ok(endpoint) => endpoint,
                Err(err) => return Poll::Ready(Err(err)),
            };
            match socket.send_slice(data, endpoint) {
                Ok(()) => {
                    self.shared.poll_wake.notify_one();
                    Poll::Ready(Ok(data.len()))
                }
                Err(udp::SendError::BufferFull) => {
                    socket.register_send_waker(cx.waker());
                    Poll::Pending
                }
                Err(udp::SendError::Unaddressable) => Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "unaddressable datagram destination",
                ))),
            }
        })
        .await
    }

    pub async fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        poll_fn(|cx| {
            let mut state = self.shared.lock();
            let socket = state.sockets.get_mut::<udp::Socket>(self.handle);
            match socket.recv_slice(buf) {
                Ok((len, meta)) => {
                    // freed buffer space: let the stack ack/window forward.
                    self.shared.poll_wake.notify_one();
                    let addr = endpoint_to_addr(meta.endpoint);
                    Poll::Ready(Ok((len, addr)))
                }
                Err(udp::RecvError::Exhausted) => {
                    socket.register_recv_waker(cx.waker());
                    Poll::Pending
                }
                Err(udp::RecvError::Truncated) => Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "datagram larger than the provided buffer",
                ))),
            }
        })
        .await
    }
}

impl Drop for VirtualUdpSocket {
    fn drop(&mut self) {
        let mut state = self.shared.lock();
        state.sockets.remove(self.handle);
    }
}

/// an accepting TCP endpoint at the host's ULA: a fixed pool of smoltcp
/// listening sockets ("slots"); `accept` hands out whichever slot completed
/// a handshake and immediately re-arms a fresh slot in its place.
pub struct VirtualTcpListener {
    shared: Arc<StackShared>,
    slots: Vec<SocketHandle>,
    local_ip: Ipv6Addr,
    port: u16,
}

impl VirtualTcpListener {
    pub(super) fn new(
        shared: Arc<StackShared>,
        slots: Vec<SocketHandle>,
        local_ip: Ipv6Addr,
        port: u16,
    ) -> Self {
        Self {
            shared,
            slots,
            local_ip,
            port,
        }
    }

    pub fn local_addr(&self) -> SocketAddr {
        SocketAddr::new(IpAddr::V6(self.local_ip), self.port)
    }

    pub async fn accept(&mut self) -> io::Result<(VirtualTcpStream, SocketAddr)> {
        let listen_endpoint = IpListenEndpoint {
            addr: Some(self.local_ip.into()),
            port: self.port,
        };
        let (slot_index, remote) = poll_fn(|cx| {
            let mut state = self.shared.lock();
            for (slot_index, &handle) in self.slots.iter().enumerate() {
                let socket = state.sockets.get_mut::<tcp::Socket>(handle);
                // a slot that fully closed (handshake aborted, or the far
                // side connected and vanished before accept) is dead, not
                // listening — re-arm it in place.
                if socket.state() == tcp::State::Closed {
                    let _ = socket.listen(listen_endpoint);
                    continue;
                }
                // a slot is ready the moment the peer is known — Established
                // normally, or already half-closed if the dialer wrote and
                // FINed before we accepted (its data is still readable).
                if let Some(remote) = socket.remote_endpoint()
                    && socket.state() != tcp::State::SynReceived
                {
                    return Poll::Ready((slot_index, endpoint_to_addr(remote)));
                }
            }
            for &handle in &self.slots {
                let socket = state.sockets.get_mut::<tcp::Socket>(handle);
                socket.register_recv_waker(cx.waker());
                socket.register_send_waker(cx.waker());
            }
            Poll::Pending
        })
        .await;

        // re-arm the slot: the accepted socket keeps its handle, a fresh
        // listening socket takes its place in the pool.
        let handle = {
            let mut state = self.shared.lock();
            let fresh = listen_slot(&mut state, self.local_ip, self.port)?;
            std::mem::replace(&mut self.slots[slot_index], fresh)
        };
        Ok((
            VirtualTcpStream {
                shared: self.shared.clone(),
                handle,
            },
            remote,
        ))
    }
}

impl Drop for VirtualTcpListener {
    fn drop(&mut self) {
        let mut state = self.shared.lock();
        for &handle in &self.slots {
            state.sockets.remove(handle);
        }
    }
}

/// an established TCP connection through the tunnel; tokio `AsyncRead` +
/// `AsyncWrite`, so it slots wherever an OS `TcpStream` does.
pub struct VirtualTcpStream {
    shared: Arc<StackShared>,
    handle: SocketHandle,
}

impl VirtualTcpStream {
    pub(super) fn new(shared: Arc<StackShared>, handle: SocketHandle) -> Self {
        Self { shared, handle }
    }
}

impl AsyncRead for VirtualTcpStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let mut state = self.shared.lock();
        let socket = state.sockets.get_mut::<tcp::Socket>(self.handle);
        if socket.can_recv() {
            let read = socket
                .recv_slice(buf.initialize_unfilled())
                .map_err(|e| io::Error::new(io::ErrorKind::ConnectionReset, format!("recv: {e}")));
            return Poll::Ready(read.map(|n| {
                buf.advance(n);
                // consumed window: let the stack ack it forward.
                self.shared.poll_wake.notify_one();
            }));
        }
        if !socket.may_recv() {
            // remote closed its half (or the connection is gone): EOF.
            return Poll::Ready(Ok(()));
        }
        socket.register_recv_waker(cx.waker());
        Poll::Pending
    }
}

impl AsyncWrite for VirtualTcpStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<io::Result<usize>> {
        let mut state = self.shared.lock();
        let socket = state.sockets.get_mut::<tcp::Socket>(self.handle);
        if !socket.may_send() {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "tcp send half closed",
            )));
        }
        if socket.can_send() {
            let sent = socket
                .send_slice(data)
                .map_err(|e| io::Error::new(io::ErrorKind::ConnectionReset, format!("send: {e}")));
            if sent.is_ok() {
                self.shared.poll_wake.notify_one();
            }
            return Poll::Ready(sent);
        }
        socket.register_send_waker(cx.waker());
        Poll::Pending
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let mut state = self.shared.lock();
        let socket = state.sockets.get_mut::<tcp::Socket>(self.handle);
        // flushed = everything we queued has left the socket (sent AND
        // acked); anything less can still be retransmitted, so the queue is
        // the honest signal.
        if socket.send_queue() == 0 || !socket.may_send() {
            Poll::Ready(Ok(()))
        } else {
            socket.register_send_waker(cx.waker());
            self.shared.poll_wake.notify_one();
            Poll::Pending
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let mut state = self.shared.lock();
        let socket = state.sockets.get_mut::<tcp::Socket>(self.handle);
        socket.close();
        self.shared.poll_wake.notify_one();
        if socket.send_queue() == 0 {
            Poll::Ready(Ok(()))
        } else {
            socket.register_send_waker(cx.waker());
            Poll::Pending
        }
    }
}

impl Drop for VirtualTcpStream {
    fn drop(&mut self) {
        // graceful close in the background: FIN now, reap once fully closed
        // (the poll loop removes it when it reaches `Closed`).
        let mut state = self.shared.lock();
        state.reap_tcp(self.handle);
        self.shared.poll_wake.notify_one();
    }
}

fn endpoint_to_addr(endpoint: IpEndpoint) -> SocketAddr {
    endpoint.into()
}

/// the overlay carries ULA v6 only (the stack is built proto-ipv6-only); a
/// v4 destination is a caller error, surfaced rather than mapped.
pub(super) fn addr_to_endpoint(addr: SocketAddr) -> io::Result<IpEndpoint> {
    match addr.ip() {
        IpAddr::V6(v6) => Ok((v6, addr.port()).into()),
        IpAddr::V4(_) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the overlay is ULA-v6 only",
        )),
    }
}
