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
//! today's only backend is the TUN pass-through ([`tun`]): the kernel routes
//! overlay ULAs through the WireGuard interface, so the backend's answer IS
//! the OS socket and behavior is bit-identical to routing nothing at all.
//! the point of carving the seam anyway is that the p2p dialer never
//! `connect()`s an overlay ULA on a raw OS socket AS AN ASSUMPTION again —
//! the userspace (TUN-less) backend lands behind exactly this boundary, as a
//! second arm of the [`OverlayListener`]/[`OverlaySink`]/[`OverlayStream`]
//! wrappers, without touching any consumer.
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

// ── The wrapper context ─────────────────────────────────

/// a runtime context that routes network calls by address and delegates
/// everything else to the wrapped context. see the module docs for why this
/// exists and how it survives the supervision tree.
#[derive(Clone)]
pub struct OverlayContext<E> {
    inner: E,
    router: OverlayRouter,
}

impl<E> OverlayContext<E> {
    pub fn new(inner: E, router: OverlayRouter) -> Self {
        Self { inner, router }
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
        }
    }

    fn with_attribute(self, key: &'static str, value: impl std::fmt::Display) -> Self {
        Self {
            inner: self.inner.with_attribute(key, value),
            router: self.router,
        }
    }
}

impl<E: Spawner> Spawner for OverlayContext<E> {
    fn shared(self, blocking: bool) -> Self {
        Self {
            inner: self.inner.shared(blocking),
            router: self.router,
        }
    }

    fn dedicated(self) -> Self {
        Self {
            inner: self.inner.dedicated(),
            router: self.router,
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
        self.inner
            .spawn(move |inner| f(Self { inner, router }))
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
        let listener = if self.router.is_overlay(&socket) {
            tun::bind(&self.inner, socket).await?
        } else {
            self.inner.bind(socket).await?
        };
        Ok(OverlayListener::Os(listener))
    }

    async fn dial(&self, socket: SocketAddr) -> Result<(SinkOf<Self>, StreamOf<Self>), Error> {
        let (sink, stream) = if self.router.is_overlay(&socket) {
            tun::dial(&self.inner, socket).await?
        } else {
            self.inner.dial(socket).await?
        };
        Ok((OverlaySink::Os(sink), OverlayStream::Os(stream)))
    }
}

// ── Connection wrappers ─────────────────────────────────
//
// single-variant today: every connection is carried by an OS socket, whether
// it was routed (overlay, via the TUN backend) or passed through. the enums
// exist so the userspace backend adds a `Virtual` arm WITHOUT changing
// `OverlayContext`'s associated types — consumers only ever see these
// wrappers, so phase 1 never touches them.

pub enum OverlayListener<L> {
    Os(L),
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
        }
    }

    fn local_addr(&self) -> Result<SocketAddr, std::io::Error> {
        match self {
            Self::Os(listener) => listener.local_addr(),
        }
    }
}

pub enum OverlaySink<S> {
    Os(S),
}

impl<S: Sink> Sink for OverlaySink<S> {
    async fn send(&mut self, bufs: impl Into<IoBufs> + Send) -> Result<(), Error> {
        match self {
            Self::Os(sink) => sink.send(bufs).await,
        }
    }
}

pub enum OverlayStream<S> {
    Os(S),
}

impl<S: Stream> Stream for OverlayStream<S> {
    async fn recv(&mut self, len: usize) -> Result<IoBufs, Error> {
        match self {
            Self::Os(stream) => stream.recv(len).await,
        }
    }

    fn peek(&self, max_len: usize) -> &[u8] {
        match self {
            Self::Os(stream) => stream.peek(max_len),
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
        Ipv6Addr::from([0xfd, 0xa2, 0x8a, 0xd3, 0xea, 0xee, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
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
