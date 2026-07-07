//! the virtual host of the userspace backend: a smoltcp interface bound to
//! the node's overlay ULA, terminating TCP and UDP entirely in-process.
//!
//! where the TUN backend lets the kernel be the host at the node's `/128`,
//! this stack IS that host: the [`device`](super::device) layer feeds it
//! decrypted IP packets over a channel and carries away the packets it
//! emits. the async sockets over it live in [`sockets`](super::sockets) —
//! this module owns the host itself: the bridge device, the interface, and
//! the poll loop that drives them.
//!
//! concurrency model: the whole smoltcp state (interface + socket set +
//! bridge device) lives under ONE `std::sync::Mutex`, held only for
//! non-blocking work. a single poll task drives `Interface::poll` whenever
//! (a) a packet arrives from the WireGuard device, (b) a socket operation
//! signals `poll_wake`, or (c) smoltcp's own `poll_delay` timer expires;
//! socket futures park themselves on smoltcp's waker registration (feature
//! `async`), which `Interface::poll` fires on any state change.

use std::collections::VecDeque;
use std::future::poll_fn;
use std::hash::{BuildHasher, Hasher, RandomState};
use std::io;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::Poll;
use std::time::Duration;

use smoltcp::iface::{Config, Interface, PollResult, SocketHandle, SocketSet};
use smoltcp::phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken};
use smoltcp::socket::{tcp, udp};
use smoltcp::time::Instant as SmolInstant;
use smoltcp::wire::{HardwareAddress, IpCidr, IpEndpoint, IpListenEndpoint};
use tokio::sync::{Notify, mpsc};
use tokio::task::JoinHandle;

use super::sockets::{VirtualTcpListener, VirtualTcpStream, VirtualUdpSocket, addr_to_endpoint};

/// the tunnel MTU: the conventional WireGuard value (1500 minus WG overhead),
/// matching what the TUN path configures — so a socket-mode node never emits
/// an IP packet a tun-mode peer's interface would refuse.
const MTU: usize = 1420;

/// per-socket buffer sizing. UDP metadata slots bound how many datagrams can
/// queue; byte buffers bound total payload. TCP buffers set the offered
/// window — 256 KiB keeps bulk streams moving at overlay latencies without
/// letting one stream hoard unbounded memory.
const UDP_META: usize = 64;
const UDP_BUFFER: usize = 1 << 16;
const TCP_BUFFER: usize = 1 << 18;

/// first ephemeral port for stack-allocated local ports (the IANA dynamic
/// range start, same policy as the kernel's default).
const EPHEMERAL_START: u16 = 49152;

// ── the bridge device ───────────────────────────────────

/// smoltcp's view of the WireGuard tunnel: raw IP in, raw IP out, no link
/// layer (`Medium::Ip` — WireGuard carries bare IP packets).
struct BridgeDevice {
    /// packets decrypted by the WireGuard device, waiting for the stack.
    rx: VecDeque<Vec<u8>>,
    /// packets the stack emits, bound for encryption. `try_send` because
    /// smoltcp's `TxToken::consume` is synchronous: a full channel drops the
    /// packet, which is exactly a full NIC ring — TCP retransmits, datagram
    /// lanes tolerate loss by contract.
    tx: mpsc::Sender<Vec<u8>>,
}

struct BridgeRxToken(Vec<u8>);

impl RxToken for BridgeRxToken {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        f(&self.0)
    }
}

struct BridgeTxToken<'a>(&'a mpsc::Sender<Vec<u8>>);

impl TxToken for BridgeTxToken<'_> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut pkt = vec![0u8; len];
        let result = f(&mut pkt);
        let _ = self.0.try_send(pkt);
        result
    }
}

impl Device for BridgeDevice {
    type RxToken<'a> = BridgeRxToken;
    type TxToken<'a> = BridgeTxToken<'a>;

    fn receive(&mut self, _: SmolInstant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        self.rx
            .pop_front()
            .map(|pkt| (BridgeRxToken(pkt), BridgeTxToken(&self.tx)))
    }

    fn transmit(&mut self, _: SmolInstant) -> Option<Self::TxToken<'_>> {
        Some(BridgeTxToken(&self.tx))
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = Medium::Ip;
        caps.max_transmission_unit = MTU;
        caps
    }
}

// ── the stack ───────────────────────────────────────────

pub(super) struct StackState {
    iface: Interface,
    pub(super) sockets: SocketSet<'static>,
    device: BridgeDevice,
    /// smoltcp's clock base: it wants a monotonic millisecond timestamp, we
    /// give it time-since-stack-creation.
    epoch: std::time::Instant,
    next_ephemeral: u16,
    /// TCP handles whose owners dropped: closed, awaiting teardown to
    /// `Closed` state before removal (a FIN needs its round trip).
    reap: Vec<SocketHandle>,
}

impl StackState {
    fn now(&self) -> SmolInstant {
        SmolInstant::from_micros(self.epoch.elapsed().as_micros() as i64)
    }

    fn allocate_ephemeral(&mut self) -> u16 {
        loop {
            let port = self.next_ephemeral;
            self.next_ephemeral = if port == u16::MAX {
                EPHEMERAL_START
            } else {
                port + 1
            };
            if !self.port_in_use(port) {
                return port;
            }
        }
    }

    /// is any live socket already using this local port? keeps ephemeral
    /// allocation from colliding with long-lived binds after wraparound.
    fn port_in_use(&self, port: u16) -> bool {
        self.sockets.iter().any(|(_, socket)| match socket {
            smoltcp::socket::Socket::Tcp(s) => {
                s.listen_endpoint().port == port
                    || s.local_endpoint().is_some_and(|ep| ep.port == port)
            }
            smoltcp::socket::Socket::Udp(s) => s.endpoint().port == port,
        })
    }

    /// begin a dropped TCP stream's background teardown: FIN now, removal
    /// once the poll loop sees it reach `Closed`.
    pub(super) fn reap_tcp(&mut self, handle: SocketHandle) {
        self.sockets.get_mut::<tcp::Socket>(handle).close();
        self.reap.push(handle);
    }

    /// remove reaped TCP sockets that have fully closed.
    fn collect_reaped(&mut self) {
        let sockets = &mut self.sockets;
        self.reap.retain(|&handle| {
            let socket = sockets.get_mut::<tcp::Socket>(handle);
            if socket.state() == tcp::State::Closed {
                sockets.remove(handle);
                false
            } else {
                true
            }
        });
    }
}

pub(super) struct StackShared {
    state: Mutex<StackState>,
    /// kicks the poll task after any socket operation that queued work.
    pub(super) poll_wake: Notify,
}

impl StackShared {
    pub(super) fn lock(&self) -> MutexGuard<'_, StackState> {
        self.state.lock().expect("stack lock poisoned")
    }
}

/// how many pre-armed listening slots a seam- or factory-minted TCP
/// listener keeps: the bound on connections that can complete a handshake
/// before `accept` collects them. each slot carries its TCP buffers eagerly
/// (2 × 256 KiB), so this is also a per-listener memory commitment (~4 MiB).
pub(crate) const LISTEN_BACKLOG: usize = 8;

/// the publishable handle to the live [`VirtualStack`] — what the overlay
/// seam and the data-plane socket factory route through. the effect layer
/// owns the writes: it publishes on the `apply` that stands a backend up,
/// re-publishes on an interface-replacing rebuild, and clears on
/// `remove_interface`. consumers read PER OPERATION (every dial/bind), so a
/// rebuilt backend serves new connections without any consumer re-wiring —
/// exactly the property epoch cutover needs. an empty slot means the tunnel
/// is not up, which callers surface as the same "interface down" failure the
/// TUN path yields.
#[derive(Clone, Default)]
pub struct StackSlot(Arc<std::sync::RwLock<Option<Arc<VirtualStack>>>>);

impl StackSlot {
    pub fn new() -> Self {
        Self::default()
    }

    /// the live stack, if the backend is up.
    pub fn get(&self) -> Option<Arc<VirtualStack>> {
        self.0.read().expect("stack slot lock poisoned").clone()
    }

    pub(super) fn publish(&self, stack: Arc<VirtualStack>) {
        *self.0.write().expect("stack slot lock poisoned") = Some(stack);
    }

    pub(super) fn clear(&self) {
        *self.0.write().expect("stack slot lock poisoned") = None;
    }
}

/// the virtual host. owns the poll task; dropping the stack stops it.
pub struct VirtualStack {
    shared: Arc<StackShared>,
    local_ip: Mutex<Ipv6Addr>,
    task: JoinHandle<()>,
}

impl VirtualStack {
    /// stand the host up at `local_ip` (the node's overlay `/128`) inside the
    /// chain's `/48` (`prefix_len` — addresses inside it are on-link: the
    /// tunnel is the link, cryptokey routing is the switch fabric), bridged
    /// to the WireGuard device via the channel pair.
    pub fn spawn(
        handle: &tokio::runtime::Handle,
        local_ip: Ipv6Addr,
        prefix_len: u8,
        from_device: mpsc::Receiver<Vec<u8>>,
        to_device: mpsc::Sender<Vec<u8>>,
    ) -> Self {
        let mut device = BridgeDevice {
            rx: VecDeque::new(),
            tx: to_device,
        };
        // TCP initial sequence numbers and similar want per-boot randomness;
        // RandomState is seeded from OS entropy per process, no rng dep.
        let mut config = Config::new(HardwareAddress::Ip);
        config.random_seed = RandomState::new().build_hasher().finish();
        let epoch = std::time::Instant::now();
        let mut iface = Interface::new(config, &mut device, SmolInstant::from_micros(0));
        iface.update_ip_addrs(|addrs| {
            addrs
                .push(IpCidr::new(local_ip.into(), prefix_len))
                .expect("fresh interface has room for one address");
        });
        let shared = Arc::new(StackShared {
            state: Mutex::new(StackState {
                iface,
                sockets: SocketSet::new(Vec::new()),
                device,
                epoch,
                next_ephemeral: EPHEMERAL_START,
                reap: Vec::new(),
            }),
            poll_wake: Notify::new(),
        });
        let task = handle.spawn(poll_loop(shared.clone(), from_device));
        Self {
            shared,
            local_ip: Mutex::new(local_ip),
            task,
        }
    }

    /// the `/128` this host answers at.
    pub fn local_ip(&self) -> Ipv6Addr {
        *self.local_ip.lock().expect("local ip lock poisoned")
    }

    /// re-address the host — the effect's `apply` replacing the interface
    /// ULA. existing sockets keep running only if the address is unchanged;
    /// a genuine re-address implies a new member identity, so live
    /// connections on the old address are dead anyway.
    pub fn set_local_ip(&self, local_ip: Ipv6Addr, prefix_len: u8) {
        let mut current = self.local_ip.lock().expect("local ip lock poisoned");
        if *current == local_ip {
            return;
        }
        *current = local_ip;
        let mut state = self.shared.lock();
        state.iface.update_ip_addrs(|addrs| {
            addrs.clear();
            addrs
                .push(IpCidr::new(local_ip.into(), prefix_len))
                .expect("cleared address list has room");
        });
        self.shared.poll_wake.notify_one();
    }

    /// bind an async UDP socket at the host's ULA. port 0 allocates an
    /// ephemeral one.
    pub fn bind_udp(&self, port: u16) -> io::Result<VirtualUdpSocket> {
        let local_ip = self.local_ip();
        let mut state = self.shared.lock();
        let port = if port == 0 {
            state.allocate_ephemeral()
        } else {
            port
        };
        let rx = udp::PacketBuffer::new(
            vec![udp::PacketMetadata::EMPTY; UDP_META],
            vec![0; UDP_BUFFER],
        );
        let tx = udp::PacketBuffer::new(
            vec![udp::PacketMetadata::EMPTY; UDP_META],
            vec![0; UDP_BUFFER],
        );
        let mut socket = udp::Socket::new(rx, tx);
        socket
            .bind(IpListenEndpoint {
                addr: Some(local_ip.into()),
                port,
            })
            .map_err(|e| io::Error::new(io::ErrorKind::AddrInUse, format!("udp bind: {e}")))?;
        let handle = state.sockets.add(socket);
        Ok(VirtualUdpSocket::new(
            self.shared.clone(),
            handle,
            SocketAddr::new(IpAddr::V6(local_ip), port),
        ))
    }

    /// open a listener at the host's ULA. `backlog` bounds how many inbound
    /// connections can complete their handshake while none has been
    /// `accept`ed yet.
    pub fn listen_tcp(&self, port: u16, backlog: usize) -> io::Result<VirtualTcpListener> {
        let local_ip = self.local_ip();
        let mut state = self.shared.lock();
        let port = if port == 0 {
            state.allocate_ephemeral()
        } else {
            port
        };
        let slots = (0..backlog.max(1))
            .map(|_| listen_slot(&mut state, local_ip, port))
            .collect::<io::Result<Vec<_>>>()?;
        Ok(VirtualTcpListener::new(
            self.shared.clone(),
            slots,
            local_ip,
            port,
        ))
    }

    /// dial `remote` from the host's ULA (an ephemeral source port — the far
    /// side authenticates by source `/128`, not port).
    pub async fn connect_tcp(&self, remote: SocketAddr) -> io::Result<VirtualTcpStream> {
        let local_ip = self.local_ip();
        let handle = {
            let mut state = self.shared.lock();
            let local_port = state.allocate_ephemeral();
            let mut socket = new_tcp_socket();
            let remote: IpEndpoint = addr_to_endpoint(remote)?;
            let local = IpListenEndpoint {
                addr: Some(local_ip.into()),
                port: local_port,
            };
            let StackState { iface, .. } = &mut *state;
            socket
                .connect(iface.context(), remote, local)
                .map_err(|e| {
                    io::Error::new(io::ErrorKind::InvalidInput, format!("tcp connect: {e}"))
                })?;
            state.sockets.add(socket)
        };
        self.shared.poll_wake.notify_one();

        // park until the handshake resolves; smoltcp fires the wakers on
        // every state transition, including the RST/timeout path to Closed.
        let shared = self.shared.clone();
        poll_fn(move |cx| {
            let mut state = shared.lock();
            let socket = state.sockets.get_mut::<tcp::Socket>(handle);
            match socket.state() {
                tcp::State::Established => Poll::Ready(Ok(())),
                tcp::State::Closed | tcp::State::TimeWait => Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::ConnectionRefused,
                    "tcp connect refused or timed out",
                ))),
                _ => {
                    socket.register_send_waker(cx.waker());
                    socket.register_recv_waker(cx.waker());
                    Poll::Pending
                }
            }
        })
        .await
        .inspect_err(|_| {
            let mut state = self.shared.lock();
            state.sockets.remove(handle);
        })?;
        Ok(VirtualTcpStream::new(self.shared.clone(), handle))
    }
}

impl Drop for VirtualStack {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn new_tcp_socket() -> tcp::Socket<'static> {
    tcp::Socket::new(
        tcp::SocketBuffer::new(vec![0; TCP_BUFFER]),
        tcp::SocketBuffer::new(vec![0; TCP_BUFFER]),
    )
}

pub(super) fn listen_slot(
    state: &mut StackState,
    local_ip: Ipv6Addr,
    port: u16,
) -> io::Result<SocketHandle> {
    let mut socket = new_tcp_socket();
    socket
        .listen(IpListenEndpoint {
            addr: Some(local_ip.into()),
            port,
        })
        .map_err(|e| io::Error::new(io::ErrorKind::AddrInUse, format!("tcp listen: {e}")))?;
    Ok(state.sockets.add(socket))
}

/// the single driver of `Interface::poll`: wakes on inbound packets, socket
/// activity, or smoltcp's own timer schedule, and re-polls.
async fn poll_loop(shared: Arc<StackShared>, mut from_device: mpsc::Receiver<Vec<u8>>) {
    loop {
        let delay = {
            let mut state = shared.lock();
            let now = state.now();
            let StackState { iface, sockets, .. } = &mut *state;
            iface
                .poll_delay(now, sockets)
                .map(|d| Duration::from_micros(d.total_micros()))
        };
        tokio::select! {
            received = from_device.recv() => {
                let Some(pkt) = received else { return }; // device gone: shutting down
                let mut state = shared.lock();
                state.device.rx.push_back(pkt);
                // drain whatever else already arrived — one poll serves all.
                while let Ok(more) = from_device.try_recv() {
                    state.device.rx.push_back(more);
                }
            }
            _ = shared.poll_wake.notified() => {}
            _ = sleep_maybe(delay) => {}
        }
        let mut state = shared.lock();
        let now = state.now();
        let StackState {
            iface,
            sockets,
            device,
            ..
        } = &mut *state;
        let _: PollResult = iface.poll(now, device, sockets);
        state.collect_reaped();
    }
}

/// sleep for `delay`, or forever when smoltcp has no timer scheduled (`None`
/// means "nothing to do until new input" — the other select arms are the
/// only valid wake-ups).
async fn sleep_maybe(delay: Option<Duration>) {
    match delay {
        Some(delay) => tokio::time::sleep(delay).await,
        None => std::future::pending().await,
    }
}
