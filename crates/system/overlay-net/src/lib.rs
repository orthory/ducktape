//! the overlay-net seam — overlay reachability as an abstraction the node
//! routes through, not an assumption that overlay ULAs are OS-routable.
//!
//! phase 0 of the userspace-overlay ADR
//! (docs/adr/2026-07-07-userspace-overlay-net.mdx): the p2p stack is generic
//! over its runtime context (`commonware_runtime`), so the seam is a WRAPPER
//! CONTEXT — [`OverlayContext`] delegates every runtime trait to the inner
//! context verbatim except [`commonware_runtime::Network`], whose
//! `dial`/`bind` route BY ADDRESS: sockets on the chain's overlay ULA `/48`
//! (see [`OverlayRouter`]) go to the active overlay backend, everything else
//! passes straight through to the OS.
//!
//! two backends live behind the boundary ([`OverlayBackend`]):
//!
//! - the TUN pass-through (`tun`): the kernel routes overlay ULAs through
//!   the WireGuard interface, so the backend's answer IS the OS socket and
//!   behavior is bit-identical to routing nothing at all.
//! - the userspace backend (ADR phases 1–2, [`userspace`]): overlay
//!   connections terminate in the in-process smoltcp host, carried as the
//!   `Virtual` arm of the [`OverlayListener`]/[`OverlaySink`]/
//!   [`OverlayStream`] wrappers — no TUN, no privilege, no consumer changes.
//!
//! the wrapper survives the supervision tree BY CONSTRUCTION: `spawn`,
//! `child`, `shared`, `dedicated`, and `with_attribute` all re-wrap the fresh
//! inner context they produce, so a dialer task spawned five levels deep
//! still routes by address. losing the wrapper across a spawn would silently
//! collapse the seam exactly where the mesh actually dials.

use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::time::{Duration, SystemTime};

use commonware_runtime::{
    BufferPool, BufferPooler, Clock, Error, Handle, IoBufs, Listener, Metrics, Name, Network,
    Resolver, Sink, SinkOf, Spawner, Stream, StreamOf, Supervisor, signal, telemetry,
};

mod tun;
pub mod userspace;

use userspace::StackSlot;

// ── Routing ─────────────────────────────────────────────

/// classifies a socket address onto its carrying plane: the chain's overlay
/// (a ULA inside the chain-derived `/48`) or the plain OS network. the one
/// routing decision the whole seam turns on.
#[derive(Clone, Copy, Debug)]
pub struct OverlayRouter {
    /// the chain's ULA `/48`: `fd` + 5 chain-hash bytes.
    prefix: [u8; 6],
}

impl OverlayRouter {
    /// build the router from the chain's ULA `/48` prefix ADDRESS — the value
    /// of `wireguard_upgrade::ula_v6_prefix(chain_id)`. taking the address
    /// rather than the chain id keeps this crate free of the
    /// wireguard-upgrade dependency (mirroring data-plane's dependency-free
    /// posture: the derivation stays the node layer's business).
    pub fn for_prefix48(prefix: Ipv6Addr) -> Self {
        let octets = prefix.octets();
        Self {
            prefix: [
                octets[0], octets[1], octets[2], octets[3], octets[4], octets[5],
            ],
        }
    }

    /// does `addr` live on this chain's overlay? member ULAs are the `/48`
    /// prefix + 80 identity-hash bits (`ula_v6_member_addr`), so a prefix
    /// match is exact membership of the overlay address plane. v4 is never
    /// overlay — the mesh's overlay addressing is ULA v6 only.
    pub fn is_overlay(&self, addr: &SocketAddr) -> bool {
        match addr.ip() {
            IpAddr::V6(v6) => v6.octets()[..6] == self.prefix,
            IpAddr::V4(_) => false,
        }
    }
}

// ── Backend selection ───────────────────────────────────

/// where overlay-routed connections terminate — the per-node backend choice
/// the ADR's `wireguard_effect = socket | tun` config resolves to.
#[derive(Clone)]
pub enum OverlayBackend {
    /// TUN pass-through: the kernel routes overlay ULAs through the
    /// WireGuard interface, so overlay connections ride ordinary OS sockets.
    Tun,
    /// userspace: overlay connections terminate in the in-process virtual
    /// stack. the slot is published by `UserspaceWireGuardEffect` when its
    /// backend stands up and read here per dial/bind, so an interface
    /// replace (epoch cutover) needs no context rewiring; while it is empty
    /// the tunnel is down and overlay dials/binds fail, exactly as they
    /// would on a downed TUN interface.
    Userspace(StackSlot),
}

// ── The wrapper context ─────────────────────────────────

/// a runtime context that routes network calls by address and delegates
/// everything else to the wrapped context. see the module docs for why this
/// exists and how it survives the supervision tree.
#[derive(Clone)]
pub struct OverlayContext<E> {
    inner: E,
    router: OverlayRouter,
    backend: OverlayBackend,
}

impl<E> OverlayContext<E> {
    /// the TUN-backed context — every shipped caller's arm.
    pub fn new(inner: E, router: OverlayRouter) -> Self {
        Self::with_backend(inner, router, OverlayBackend::Tun)
    }

    pub fn with_backend(inner: E, router: OverlayRouter, backend: OverlayBackend) -> Self {
        Self {
            inner,
            router,
            backend,
        }
    }
}

impl<E: Supervisor> Supervisor for OverlayContext<E> {
    fn name(&self) -> Name {
        self.inner.name()
    }

    fn child(&self, label: &'static str) -> Self {
        Self {
            inner: self.inner.child(label),
            router: self.router,
            backend: self.backend.clone(),
        }
    }

    fn with_attribute(self, key: &'static str, value: impl std::fmt::Display) -> Self {
        Self {
            inner: self.inner.with_attribute(key, value),
            router: self.router,
            backend: self.backend,
        }
    }
}

impl<E: Spawner> Spawner for OverlayContext<E> {
    fn shared(self, blocking: bool) -> Self {
        Self {
            inner: self.inner.shared(blocking),
            router: self.router,
            backend: self.backend,
        }
    }

    fn dedicated(self) -> Self {
        Self {
            inner: self.inner.dedicated(),
            router: self.router,
            backend: self.backend,
        }
    }

    fn spawn<F, Fut, T>(self, f: F) -> Handle<T>
    where
        Self: Sized,
        F: FnOnce(Self) -> Fut + Send + 'static,
        Fut: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        // RE-WRAP the fresh context the runtime hands the task: this is the
        // line that keeps the seam alive inside spawned dialer tasks.
        let router = self.router;
        let backend = self.backend;
        self.inner.spawn(move |inner| {
            f(Self {
                inner,
                router,
                backend,
            })
        })
    }

    fn stop(
        self,
        value: i32,
        timeout: Option<Duration>,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        self.inner.stop(value, timeout)
    }

    fn stopped(&self) -> signal::Signal {
        self.inner.stopped()
    }
}

impl<E: Metrics> Metrics for OverlayContext<E> {
    fn register<N: Into<String>, H: Into<String>, M: telemetry::metrics::Metric>(
        &self,
        name: N,
        help: H,
        metric: M,
    ) -> telemetry::metrics::Registered<M> {
        self.inner.register(name, help, metric)
    }

    fn encode(&self) -> String {
        self.inner.encode()
    }
}

impl<E: Clock> Clock for OverlayContext<E> {
    fn current(&self) -> SystemTime {
        self.inner.current()
    }

    fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send + 'static {
        self.inner.sleep(duration)
    }

    fn sleep_until(&self, deadline: SystemTime) -> impl Future<Output = ()> + Send + 'static {
        self.inner.sleep_until(deadline)
    }
}

impl<E: Clock> governor::clock::Clock for OverlayContext<E> {
    type Instant = SystemTime;

    fn now(&self) -> Self::Instant {
        self.inner.current()
    }
}

impl<E: Clock> governor::clock::ReasonablyRealtime for OverlayContext<E> {}

impl<E: rand_core::RngCore> rand_core::RngCore for OverlayContext<E> {
    fn next_u32(&mut self) -> u32 {
        self.inner.next_u32()
    }

    fn next_u64(&mut self) -> u64 {
        self.inner.next_u64()
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.inner.fill_bytes(dest)
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        self.inner.try_fill_bytes(dest)
    }
}

impl<E: rand_core::CryptoRng> rand_core::CryptoRng for OverlayContext<E> {}

impl<E: BufferPooler> BufferPooler for OverlayContext<E> {
    fn network_buffer_pool(&self) -> &BufferPool {
        self.inner.network_buffer_pool()
    }

    fn storage_buffer_pool(&self) -> &BufferPool {
        self.inner.storage_buffer_pool()
    }
}

impl<E: Resolver> Resolver for OverlayContext<E> {
    async fn resolve(&self, host: &str) -> Result<Vec<IpAddr>, Error> {
        self.inner.resolve(host).await
    }
}

impl<E: Network> Network for OverlayContext<E> {
    type Listener = OverlayListener<E::Listener>;

    async fn bind(&self, socket: SocketAddr) -> Result<Self::Listener, Error> {
        // socket mode's mesh listener (ADR phase 3): an unspecified-address
        // bind means "accept from anywhere" — but a tunnel-carried inbound
        // connection terminates in the virtual stack, which the OS listener
        // can never see. so the wildcard bind carries BOTH: the OS socket
        // for the underlay, plus a lazy virtual leg at the node's own ULA
        // on the same port (lazy: the stack exists only while a tunnel is
        // applied, and is replaced on interface rebuilds).
        if let OverlayBackend::Userspace(slot) = &self.backend
            && socket.ip().is_unspecified()
        {
            let os = self.inner.bind(socket).await?;
            return Ok(OverlayListener::Dual(
                os,
                userspace::seam::LazyVirtualListener::new(slot.clone(), socket.port()),
            ));
        }
        if !self.router.is_overlay(&socket) {
            return Ok(OverlayListener::Os(self.inner.bind(socket).await?));
        }
        match &self.backend {
            OverlayBackend::Tun => Ok(OverlayListener::Os(tun::bind(&self.inner, socket).await?)),
            OverlayBackend::Userspace(slot) => Ok(OverlayListener::Virtual(
                userspace::seam::bind(slot, socket).await?,
            )),
        }
    }

    async fn dial(&self, socket: SocketAddr) -> Result<(SinkOf<Self>, StreamOf<Self>), Error> {
        if !self.router.is_overlay(&socket) {
            let (sink, stream) = self.inner.dial(socket).await?;
            return Ok((OverlaySink::Os(sink), OverlayStream::Os(stream)));
        }
        match &self.backend {
            OverlayBackend::Tun => {
                let (sink, stream) = tun::dial(&self.inner, socket).await?;
                Ok((OverlaySink::Os(sink), OverlayStream::Os(stream)))
            }
            OverlayBackend::Userspace(slot) => {
                let (sink, stream) = userspace::seam::dial(slot, socket).await?;
                Ok((OverlaySink::Virtual(sink), OverlayStream::Virtual(stream)))
            }
        }
    }
}

// ── Connection wrappers ─────────────────────────────────
//
// two arms: `Os` carries an OS socket (passed through, or overlay in TUN
// mode), `Virtual` carries a connection terminating in the userspace
// backend's smoltcp host. the enums are what let the backend vary WITHOUT
// changing `OverlayContext`'s associated types — consumers only ever see
// these wrappers.

pub enum OverlayListener<L> {
    Os(L),
    Virtual(userspace::seam::VirtualListener),
    /// socket mode's wildcard bind: the OS listener for the underlay AND the
    /// lazy virtual leg at the node's own ULA (see [`Network::bind`] above).
    Dual(L, userspace::seam::LazyVirtualListener),
}

impl<L: Listener> Listener for OverlayListener<L> {
    type Sink = OverlaySink<L::Sink>;
    type Stream = OverlayStream<L::Stream>;

    async fn accept(&mut self) -> Result<(SocketAddr, Self::Sink, Self::Stream), Error> {
        match self {
            Self::Os(listener) => {
                let (addr, sink, stream) = listener.accept().await?;
                Ok((addr, OverlaySink::Os(sink), OverlayStream::Os(stream)))
            }
            Self::Virtual(listener) => {
                let (addr, sink, stream) = listener.accept().await?;
                Ok((
                    addr,
                    OverlaySink::Virtual(sink),
                    OverlayStream::Virtual(stream),
                ))
            }
            // both arms are cancel-safe (tokio accept; poll-based slot scan),
            // so whichever loses the race abandons no connection state.
            Self::Dual(os, virt) => tokio::select! {
                accepted = os.accept() => {
                    let (addr, sink, stream) = accepted?;
                    Ok((addr, OverlaySink::Os(sink), OverlayStream::Os(stream)))
                }
                (addr, sink, stream) = virt.accept() => Ok((
                    addr,
                    OverlaySink::Virtual(sink),
                    OverlayStream::Virtual(stream),
                )),
            },
        }
    }

    fn local_addr(&self) -> Result<SocketAddr, std::io::Error> {
        match self {
            Self::Os(listener) => listener.local_addr(),
            Self::Virtual(listener) => listener.local_addr(),
            Self::Dual(os, _) => os.local_addr(),
        }
    }
}

pub enum OverlaySink<S> {
    Os(S),
    Virtual(userspace::seam::VirtualSink),
}

impl<S: Sink> Sink for OverlaySink<S> {
    async fn send(&mut self, bufs: impl Into<IoBufs> + Send) -> Result<(), Error> {
        match self {
            Self::Os(sink) => sink.send(bufs).await,
            Self::Virtual(sink) => sink.send(bufs).await,
        }
    }
}

pub enum OverlayStream<S> {
    Os(S),
    Virtual(userspace::seam::VirtualStream),
}

impl<S: Stream> Stream for OverlayStream<S> {
    async fn recv(&mut self, len: usize) -> Result<IoBufs, Error> {
        match self {
            Self::Os(stream) => stream.recv(len).await,
            Self::Virtual(stream) => stream.recv(len).await,
        }
    }

    fn peek(&self, max_len: usize) -> &[u8] {
        match self {
            Self::Os(stream) => stream.peek(max_len),
            Self::Virtual(stream) => stream.peek(max_len),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use commonware_runtime::{Runner as _, deterministic};

    /// a fixture /48: fd + 5 arbitrary hash bytes, the shape
    /// `ula_v6_prefix` mints.
    fn fixture_prefix() -> Ipv6Addr {
        Ipv6Addr::from([
            0xfd, 0xa2, 0x8a, 0xd3, 0xea, 0xee, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ])
    }

    #[test]
    fn router_matches_only_the_chain_ula_48() {
        let router = OverlayRouter::for_prefix48(fixture_prefix());
        // a member /128 inside the /48.
        let member: SocketAddr = "[fda2:8ad3:eaee:1234:5678:9abc:def0:1]:52200"
            .parse()
            .unwrap();
        assert!(router.is_overlay(&member));
        // another chain's ULA — same fd00::/8, different hash bytes.
        let other: SocketAddr = "[fdff:1111:2222:3333::1]:52200".parse().unwrap();
        assert!(!router.is_overlay(&other));
        // the underlay: public v6, loopback, and any v4 at all.
        let public_v6: SocketAddr = "[2001:db8::1]:443".parse().unwrap();
        assert!(!router.is_overlay(&public_v6));
        let v4: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        assert!(!router.is_overlay(&v4));
    }

    /// both routing arms carry a live connection end-to-end through the
    /// wrapper, and a listener bound inside a SPAWNED task (a re-wrapped
    /// child context) still routes — the supervision-tree survival the seam
    /// depends on.
    #[test]
    fn wrapped_context_carries_connections_on_both_arms() {
        let executor = deterministic::Runner::default();
        executor.start(|context| async move {
            let router = OverlayRouter::for_prefix48(fixture_prefix());
            let context = OverlayContext::new(context, router);

            // one address per arm: an overlay member /128 and a plain OS addr.
            let overlay_addr: SocketAddr = "[fda2:8ad3:eaee::42]:52200".parse().unwrap();
            // below 32768: the deterministic runtime refuses localhost binds
            // inside its dialer-ephemeral range (32768..61000).
            let os_addr: SocketAddr = "127.0.0.1:9000".parse().unwrap();

            for addr in [overlay_addr, os_addr] {
                let listener_ctx = context.child("listener");
                let accepted = listener_ctx.spawn(move |ctx| async move {
                    let mut listener = ctx.bind(addr).await.expect("bind through the seam");
                    let (_, _, mut stream) = listener.accept().await.expect("accept");
                    stream.recv(5).await.expect("recv")
                });
                // the listener binds inside the spawned task, so the dial
                // legitimately races it — retry on the deterministic clock
                // until the bind lands (bounded so a real failure stays loud).
                let mut dialed = None;
                for _ in 0..100 {
                    match context.dial(addr).await {
                        Ok(conn) => {
                            dialed = Some(conn);
                            break;
                        }
                        Err(_) => context.sleep(Duration::from_millis(10)).await,
                    }
                }
                let (mut sink, _) = dialed.expect("dial through the seam");
                sink.send(&b"seam!"[..]).await.expect("send");
                let got = accepted.await.expect("listener task");
                assert_eq!(got.coalesce().as_ref(), b"seam!");
            }
        });
    }
}
