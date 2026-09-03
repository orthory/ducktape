//! the WireGuard data plane of the userspace backend: a table of boringtun
//! `Tunn`s (one per peer, keyed by allowed-ip `/128`) pumped over ONE
//! process-owned underlay UDP socket.
//!
//! this is the sans-io half of `UserspaceOverlayNet`: it never sees
//! a TCP stream or a virtual socket — it moves raw IP packets between the
//! underlay (encrypted WireGuard datagrams on the UDP socket) and the
//! [`stack`](super::stack) (plaintext IP packets over a channel pair), and it
//! owns everything the kernel used to: cryptokey routing, endpoint roaming,
//! and the timer machinery (handshake retry, keepalive, rekey) that
//! boringtun's device layer would otherwise drive.
//!
//! locking discipline: `Tunn` is sans-io and `&mut`, so each peer's tunnel
//! sits behind a `std::sync::Mutex` that is NEVER held across an await —
//! every pump copies the packets a `Tunn` call produces out of the lock, then
//! sends. the peer table itself is an `RwLock` written only by
//! [`WgDevice::replace_peers`] (the effect's `apply`), which swaps the whole
//! table in one write — the peer-set replace must be atomic.

use std::collections::HashMap;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use defguard_boringtun::noise::errors::WireGuardError;
use defguard_boringtun::noise::handshake::parse_handshake_anon;
use defguard_boringtun::noise::{Packet, Tunn, TunnResult};
use defguard_boringtun::x25519::{PublicKey, StaticSecret};
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;
use tokio::task::JoinHandle;

/// max size of one WireGuard datagram / decapsulated IP packet we handle;
/// comfortably above the 1420 tunnel MTU plus WG overhead, and equal to what
/// boringtun's own device layer uses.
const MAX_PACKET: usize = u16::MAX as usize;

/// how often the timer pump ticks every peer's `Tunn::update_timers` —
/// boringtun's device layer uses 250ms, and the timer contract (rekey,
/// keepalive, handshake retransmit) is calibrated to roughly that cadence.
const TIMER_TICK: Duration = Duration::from_millis(250);

/// capacity of each demux lane (WG datagrams to the device, everything else
/// to the bypass); overflow drops the datagram, exactly as a full kernel
/// UDP receive buffer would.
const LANE_CHANNEL: usize = 1024;

/// one raw datagram off the underlay, tagged with its source.
pub type Datagram = (Vec<u8>, SocketAddr);

/// the WG lane's re-installable sender: each spawned device attaches its own.
type WgLane = Arc<RwLock<Option<mpsc::Sender<Datagram>>>>;

/// count one dropped packet and report it at a bounded cadence — the first
/// drop and every 1024th after, carrying the running count. a drop is a
/// per-frame event, so it never gets an unconditional log line, and it is
/// never silent either: the counter IS the diagnosis.
pub(super) fn note_drop(counter: &AtomicU64, reason: &'static str) {
    let dropped = counter.fetch_add(1, Ordering::Relaxed) + 1;
    if dropped == 1 || dropped.is_multiple_of(1024) {
        tracing::warn!(
            target: "ducktape::dataplane",
            reason,
            dropped,
            "overlay packet dropped"
        );
    }
}

// ── the underlay socket ─────────────────────────────────

/// one process-owned underlay UDP socket per node: the bound WG
/// listen socket plus its single receive pump, demuxing inbound datagrams
/// between the WireGuard device and a BYPASS lane.
///
/// the bypass lane is why this exists as its own object: the
/// NAT punch must originate from the same 5-tuple the tunnel runs on, so the
/// nat-traversal client sends through [`send_to`](Self::send_to) and
/// receives whatever the pump classifies as not-WireGuard. classification is
/// deterministic, not heuristic: a WireGuard datagram starts with a message
/// type 1–4 followed by three reserved zero bytes and a type-specific length
/// (`Tunn::parse_incoming_packet`), while every inbound nat-traversal reply
/// (tags 2/5/6/7) fails that parse by construction — tag 2 is followed by an
/// address-family byte (4 or 6, never 0), the rest are tags above 4.
///
/// the socket outlives interface rebuilds when shared (the effect's
/// socket-mode wiring): rendezvous keepalives keep flowing — and the NAT
/// pinhole stays open — while the tunnel itself is torn down and re-applied.
pub struct UnderlaySocket {
    udp: Arc<UdpSocket>,
    /// the address the underlay actually bound (port-0 resolved) — captured
    /// once so [`send_to`](Self::send_to) can gate its family handling on the
    /// socket's OWN family without a per-send syscall. a real IPv4 (`0.0.0.0`)
    /// bind in production; the family is fixed for the socket's life.
    local: SocketAddr,
    /// the WG lane's sender — installed by each [`WgDevice`] at spawn, so a
    /// rebuilt device re-attaches to the same socket; `None` (or a closed
    /// sender) while no device is live, when WG datagrams are dropped
    /// exactly as they would be on a downed interface.
    wg_lane: WgLane,
    /// the bypass lane's receiver, handed out once via
    /// [`take_bypass`](Self::take_bypass).
    bypass: Mutex<Option<mpsc::Receiver<Datagram>>>,
    pump: JoinHandle<()>,
}

impl UnderlaySocket {
    /// bind the underlay socket (a real IPv4 `0.0.0.0:port` socket) and spawn
    /// its receive pump on `handle`.
    ///
    /// a REAL AF_INET socket, not dual-stack `[::]` with v4-mapped sends: on
    /// macOS 464XLAT (CLAT46) networks, a dual-stack socket's v4-mapped
    /// datagrams take a different NAT translation path than a true IPv4
    /// socket — the coordinator sees a different public mapping, and the peer
    /// hole punch that mapping vouches for never lands (the punch `send`
    /// succeeds, the peer's punch never arrives). every underlay endpoint a
    /// node talks to — coordinators, advertised endpoints, punched reflexives
    /// — is a V4 literal, and the overlay's own addressing is ULA-v6 INSIDE
    /// the tunnel, independent of this underlay family. so bind V4 and let the
    /// punch ride the same real IPv4 5-tuple the standalone resolver always
    /// used.
    ///
    /// binding absorbs a predecessor's asynchronous teardown: a replace
    /// cycle (remove→create→apply, or a rebuild inside `apply`) drops the
    /// old backend, but its pump tasks release the socket only when the
    /// runtime collects them — a same-port rebind can race that by a few
    /// milliseconds. bounded retry, loud on genuine conflicts; the Defguard
    /// effect's `remove_interface` polls the same way (10ms steps, 2s cap)
    /// for its TUN teardown.
    pub fn bind(handle: &tokio::runtime::Handle, port: u16) -> io::Result<Arc<Self>> {
        let std_socket = bind_retrying(port)?;
        std_socket.set_nonblocking(true)?;
        let _runtime = handle.enter();
        let udp = Arc::new(UdpSocket::from_std(std_socket)?);
        let local = udp.local_addr()?;
        let wg_lane: WgLane = Arc::new(RwLock::new(None));
        let (bypass_tx, bypass_rx) = mpsc::channel(LANE_CHANNEL);
        let pump = handle.spawn(demux_pump(udp.clone(), wg_lane.clone(), bypass_tx));
        Ok(Arc::new(Self {
            udp,
            local,
            wg_lane,
            bypass: Mutex::new(Some(bypass_rx)),
            pump,
        }))
    }

    /// the underlay address the socket actually bound (resolves port 0).
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.local)
    }

    /// send one datagram from the shared socket — the tunnel's exact
    /// 5-tuple, which is the property the NAT punch shares it for.
    pub async fn send_to(&self, buf: &[u8], dst: SocketAddr) -> io::Result<usize> {
        // family handling keys off the socket's OWN family (as
        // `NatSocket::send_to` does): a real IPv4 underlay sends a V4
        // destination directly; only a V6-bound socket must map a V4
        // destination to v4-MAPPED v6 (EINVAL otherwise). production binds
        // V4, so this is the identity — the point of the V4 bind is that the
        // datagram leaves as real IPv4, taking the same NAT translation the
        // punch established.
        let dst = match (self.local, dst) {
            (SocketAddr::V6(_), SocketAddr::V4(v4)) => {
                SocketAddr::new(IpAddr::V6(v4.ip().to_ipv6_mapped()), v4.port())
            }
            _ => dst,
        };
        self.udp.send_to(buf, dst).await
    }

    /// the bypass lane: every inbound datagram that is not WireGuard.
    /// consumable once — the nat-traversal client is its single reader.
    pub fn take_bypass(&self) -> Option<mpsc::Receiver<Datagram>> {
        self.bypass.lock().expect("bypass lock poisoned").take()
    }

    /// a SEND handle on the raw socket, for the bypass lane's consumer (the
    /// NAT client sends its protocol from the tunnel's 5-tuple). sends only:
    /// the receive side belongs to the demux pump, and a caller that
    /// `recv_from`s this handle races it for datagrams.
    pub fn sender(&self) -> Arc<UdpSocket> {
        self.udp.clone()
    }

    /// attach a device's WG lane (replacing any predecessor's — a rebuilt
    /// device re-attaches to the same socket).
    fn set_wg_lane(&self, sender: mpsc::Sender<Datagram>) {
        *self.wg_lane.write().expect("wg lane lock poisoned") = Some(sender);
    }
}

impl Drop for UnderlaySocket {
    fn drop(&mut self) {
        self.pump.abort();
    }
}

/// see [`UnderlaySocket::bind`].
fn bind_retrying(port: u16) -> io::Result<std::net::UdpSocket> {
    let mut last = None;
    for _ in 0..200 {
        match std::net::UdpSocket::bind((Ipv4Addr::UNSPECIFIED, port)) {
            Ok(socket) => return Ok(socket),
            Err(err) if err.kind() == io::ErrorKind::AddrInUse => {
                last = Some(err);
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(err) => return Err(err),
        }
    }
    Err(last.expect("retried only on AddrInUse"))
}

/// the single owner of the socket's receive side: WireGuard datagrams to the
/// live device's lane, everything else to the bypass. `try_send` on both —
/// overflow is a full kernel receive buffer (WG retransmits what matters,
/// the NAT protocol's own retries absorb a lost reply), and a blocking send
/// would let one congested lane starve the other.
async fn demux_pump(udp: Arc<UdpSocket>, wg_lane: WgLane, bypass: mpsc::Sender<Datagram>) {
    let mut buf = Box::new([0u8; MAX_PACKET]);
    let wg_lane_full = AtomicU64::new(0);
    let bypass_full = AtomicU64::new(0);
    loop {
        let (len, src) = match udp.recv_from(&mut buf[..]).await {
            Ok(received) => received,
            Err(_) => continue,
        };
        // the real IPv4 underlay already reports V4 sources as plain V4, so
        // this is a defensive identity today; it stays because a V6-bound
        // socket would surface a v4 sender as `::ffff:a.b.c.d`, which must
        // canonicalize to V4 to compare equal to the addresses configs and
        // coordinators carry.
        let src = match src {
            SocketAddr::V6(v6) => match v6.ip().to_ipv4_mapped() {
                Some(v4) => SocketAddr::new(IpAddr::V4(v4), v6.port()),
                None => src,
            },
            SocketAddr::V4(_) => src,
        };
        // a closed lane is its consumer gone (no live device, the NAT client
        // released) — a downed interface, not a drop to account for.
        if Tunn::parse_incoming_packet(&buf[..len]).is_ok() {
            let lane = wg_lane.read().expect("wg lane lock poisoned").clone();
            if let Some(lane) = lane
                && let Err(TrySendError::Full(_)) = lane.try_send((buf[..len].to_vec(), src))
            {
                note_drop(&wg_lane_full, "wg_lane_full");
            }
        } else if let Err(TrySendError::Full(_)) = bypass.try_send((buf[..len].to_vec(), src)) {
            note_drop(&bypass_full, "bypass_lane_full");
        }
    }
}

/// one peer relationship, as the effect layer hands it to the device: the
/// validated form of one `PeerTunnelConfig`.
#[derive(Clone, PartialEq, Eq)]
pub struct PeerConfig {
    /// the peer's static X25519 public key.
    pub public_key: [u8; 32],
    /// where to send the peer's encrypted datagrams. `None` for a passive
    /// relationship: the endpoint is learned from the peer's first
    /// authenticated inbound datagram (WireGuard roaming), exactly as the
    /// kernel/TUN path behaves for an endpoint-less peer.
    pub endpoint: Option<SocketAddr>,
    /// persistent keepalive seconds, driven by the timer pump.
    pub persistent_keepalive: Option<u16>,
    /// the peer's overlay `/128`s — cryptokey routing: outbound packets to
    /// these addresses encrypt to this peer, and inbound packets from this
    /// peer must carry one of them as source or they are dropped.
    pub allowed_ips: Vec<Ipv6Addr>,
}

/// one `Tunn` invocation, named so the drain loop below can re-borrow the
/// scratch buffer per iteration (a `TunnResult` borrows the buffer it was
/// given; an op enum keeps each borrow scoped to its own loop pass).
enum TunnOp<'a> {
    Decapsulate { src: IpAddr, datagram: &'a [u8] },
    Encapsulate { packet: &'a [u8] },
    UpdateTimers,
    HandshakeInitiation { force: bool },
}

/// everything one `Tunn` call (plus its drain) produced, copied out of the
/// tunnel lock: datagrams for the underlay, plaintext packets for the stack,
/// and whether the input authenticated against the peer's keys (drives
/// endpoint roaming).
#[derive(Default)]
struct TunnOutput {
    to_network: Vec<Vec<u8>>,
    to_tunnel: Vec<(Vec<u8>, IpAddr)>,
    authenticated: bool,
}

/// a live peer: its `Tunn`, its (roaming) endpoint, and the identity facts
/// the pumps route by.
struct PeerState {
    config: PeerConfig,
    /// the device-assigned 24-bit index; boringtun stamps it into the high
    /// bits of every session id (`index << 8 | session`), so inbound
    /// non-handshake-init packets route back here by `receiver_idx >> 8`.
    index: u32,
    tunn: Mutex<Tunn>,
    /// current underlay endpoint; rewritten on every authenticated inbound
    /// datagram (roaming), read by every outbound send.
    endpoint: RwLock<Option<SocketAddr>>,
    /// edge trigger for session-expiry logging. boringtun logs its expiry with
    /// no peer field and then returns `ConnectionExpired` on EVERY timer tick
    /// (its targets are pinned off in noded's log filter), so this seam — the
    /// one place that knows the peer — logs only the down-transition, and the
    /// recovery only on the next authenticated inbound.
    session_down: AtomicBool,
}

impl PeerState {
    /// run one `Tunn` op under the lock and copy everything it produces out.
    /// honors boringtun's drain contract: after `decapsulate` yields
    /// `WriteToNetwork`, it must be re-called with an empty datagram until it
    /// stops yielding (that flushes packets queued behind a completed
    /// handshake).
    fn tunn_call(&self, op: TunnOp<'_>, buf: &mut [u8; MAX_PACKET]) -> TunnOutput {
        let drain = matches!(op, TunnOp::Decapsulate { .. });
        let mut op = Some(op);
        let mut out = TunnOutput::default();
        let mut tunn = self.tunn.lock().expect("tunn lock poisoned");
        loop {
            let result = match op.take() {
                Some(TunnOp::Decapsulate { src, datagram }) => {
                    tunn.decapsulate(Some(src), datagram, &mut buf[..])
                }
                Some(TunnOp::Encapsulate { packet }) => tunn.encapsulate(packet, &mut buf[..]),
                Some(TunnOp::UpdateTimers) => tunn.update_timers(&mut buf[..]),
                Some(TunnOp::HandshakeInitiation { force }) => {
                    tunn.format_handshake_initiation(&mut buf[..], force)
                }
                // the drain: flush whatever the completed handshake unblocked.
                None => tunn.decapsulate(None, &[], &mut buf[..]),
            };
            match result {
                TunnResult::WriteToNetwork(pkt) => {
                    out.authenticated = true;
                    out.to_network.push(pkt.to_vec());
                    if !drain {
                        break;
                    }
                }
                TunnResult::WriteToTunnelV6(pkt, src) => {
                    out.authenticated = true;
                    out.to_tunnel.push((pkt.to_vec(), IpAddr::V6(src)));
                    break;
                }
                // the overlay is ULA-v6 only; a v4 payload can only be a
                // misconfigured peer — drop it (but it did authenticate).
                TunnResult::WriteToTunnelV4(..) => {
                    out.authenticated = true;
                    break;
                }
                // `Done` on the first pass is an authenticated no-op (a
                // keepalive, a cookie absorbed); on a drain pass it just ends
                // the flush, leaving `authenticated` as the first pass set it.
                TunnResult::Done => {
                    out.authenticated = true;
                    break;
                }
                TunnResult::Err(WireGuardError::ConnectionExpired) => {
                    self.note_session_expired();
                    break;
                }
                TunnResult::Err(_) => break,
            }
        }
        let peer_proved_alive = drain && out.authenticated;
        if peer_proved_alive {
            self.note_session_recovered();
        }
        out
    }

    /// the peer-labeled replacement for boringtun's contextless
    /// `CONNECTION_EXPIRED` error: 90 s of handshakes went unanswered and the
    /// session was torn down. fires once per down-transition, not per tick.
    fn note_session_expired(&self) {
        let already_down = self.session_down.swap(true, Ordering::Relaxed);
        if already_down {
            return;
        }
        let endpoint = *self.endpoint.read().expect("endpoint lock poisoned");
        tracing::warn!(
            target: "ducktape::dataplane",
            peer = %self.overlay_ip(),
            endpoint = ?endpoint,
            reason = "handshake_unanswered",
            "wg session EXPIRED — peer stopped answering handshakes; retunnels on its next authenticated packet"
        );
    }

    /// once per outage: the peer authenticated an inbound datagram again.
    fn note_session_recovered(&self) {
        let was_down = self.session_down.swap(false, Ordering::Relaxed);
        if !was_down {
            return;
        }
        let endpoint = *self.endpoint.read().expect("endpoint lock poisoned");
        tracing::info!(
            target: "ducktape::dataplane",
            peer = %self.overlay_ip(),
            endpoint = ?endpoint,
            "wg session RE-ESTABLISHED"
        );
    }

    /// the peer's overlay `/128` — its stable, human-recognizable log label.
    /// (`replace_peers` builds every peer from cryptokey routing, so an empty
    /// allowed-ips list is a config defect, not a normal state.)
    fn overlay_ip(&self) -> Ipv6Addr {
        self.config
            .allowed_ips
            .first()
            .copied()
            .unwrap_or(Ipv6Addr::UNSPECIFIED)
    }
}

/// the peer table: one source of truth, three lookup keys.
#[derive(Default)]
struct PeerTable {
    by_key: HashMap<[u8; 32], Arc<PeerState>>,
    by_ip: HashMap<Ipv6Addr, Arc<PeerState>>,
    by_index: HashMap<u32, Arc<PeerState>>,
}

/// the WireGuard device: underlay socket + peer table + pumps.
pub struct WgDevice {
    inner: Arc<DeviceInner>,
    tasks: Vec<JoinHandle<()>>,
    /// device-assigned `Tunn` indices; monotonic so a replaced peer's stale
    /// sessions can never alias a successor's.
    next_index: Mutex<u32>,
}

struct DeviceInner {
    underlay: Arc<UnderlaySocket>,
    secret: StaticSecret,
    public: PublicKey,
    peers: RwLock<PeerTable>,
}

impl WgDevice {
    /// stand the device up on the underlay socket and spawn its pumps on
    /// `handle`: inbound (the underlay's WG lane → decapsulate → `to_stack`),
    /// outbound (`from_stack` → encapsulate → UDP), and the timer driver.
    /// peers start empty; [`replace_peers`](Self::replace_peers) installs
    /// them.
    pub fn spawn(
        handle: &tokio::runtime::Handle,
        underlay: Arc<UnderlaySocket>,
        secret: StaticSecret,
        to_stack: mpsc::Sender<Vec<u8>>,
        from_stack: mpsc::Receiver<Vec<u8>>,
    ) -> Self {
        let public = PublicKey::from(&secret);
        let (wg_tx, wg_rx) = mpsc::channel(LANE_CHANNEL);
        underlay.set_wg_lane(wg_tx);
        let inner = Arc::new(DeviceInner {
            underlay,
            secret,
            public,
            peers: RwLock::new(PeerTable::default()),
        });
        let tasks = vec![
            handle.spawn(inbound_pump(inner.clone(), wg_rx, to_stack)),
            handle.spawn(outbound_pump(inner.clone(), from_stack)),
            handle.spawn(timer_pump(inner.clone())),
        ];
        Self {
            inner,
            tasks,
            next_index: Mutex::new(0),
        }
    }

    /// the underlay address the socket actually bound (resolves port 0).
    pub fn local_underlay_addr(&self) -> io::Result<SocketAddr> {
        self.inner.underlay.local_addr()
    }

    /// replace the peer set atomically — the device half of
    /// `WireGuardEffect::apply`. a peer whose `PeerConfig` is UNCHANGED keeps
    /// its live `PeerState` (sessions, roamed endpoint, timers survive — the
    /// property standby pre-warm's mid-epoch re-apply depends on); a changed
    /// or new config gets a fresh `Tunn`; a peer absent from `new` is dropped
    /// wholesale, sessions and all.
    pub fn replace_peers(&self, new: &[PeerConfig]) {
        let mut table = self.inner.peers.write().expect("peer table lock poisoned");
        let mut next = PeerTable::default();
        for config in new {
            let state = match table.by_key.get(&config.public_key) {
                Some(existing) if existing.config == *config => existing.clone(),
                // a changed relationship (endpoint, keepalive, allowed ips,
                // psk) re-tunnels: the config is authoritative over roamed
                // state, and everything else feeds `Tunn::new`, which has no
                // partial update.
                _ => {
                    let index = self.allocate_index();
                    Arc::new(PeerState {
                        config: config.clone(),
                        index,
                        tunn: Mutex::new(Tunn::new(
                            self.inner.secret.clone(),
                            PublicKey::from(config.public_key),
                            None,
                            config.persistent_keepalive,
                            index,
                            None,
                        )),
                        endpoint: RwLock::new(config.endpoint),
                        session_down: AtomicBool::new(false),
                    })
                }
            };
            next.by_index.insert(state.index, state.clone());
            for ip in &config.allowed_ips {
                next.by_ip.insert(*ip, state.clone());
            }
            next.by_key.insert(config.public_key, state);
        }
        *table = next;
    }

    fn allocate_index(&self) -> u32 {
        let mut next_index = self.next_index.lock().expect("index lock poisoned");
        let index = *next_index;
        // 24-bit space: boringtun shifts the index into the high bits of a
        // u32 session id, leaving 8 bits for the session ring.
        assert!(index < (1 << 24), "peer index space exhausted");
        *next_index += 1;
        index
    }

    /// initiate (or with `force`, re-initiate) a handshake toward the peer
    /// owning `peer_ip` — the manual rekey lever. WireGuard rekeys on its own
    /// timers; this exists for callers that must not wait for them (key
    /// rotation, and the loopback rekey proof).
    pub async fn initiate_handshake(&self, peer_ip: Ipv6Addr, force: bool) -> io::Result<()> {
        let peer = self
            .peer_by_ip(peer_ip)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no peer for address"))?;
        let mut buf = Box::new([0u8; MAX_PACKET]);
        let out = peer.tunn_call(TunnOp::HandshakeInitiation { force }, &mut buf);
        send_to_endpoint(&self.inner, &peer, out.to_network).await;
        Ok(())
    }

    /// time since the peer's last completed handshake — `None` before the
    /// first. the observable the rekey and session-preservation proofs read.
    pub fn time_since_last_handshake(&self, peer_ip: Ipv6Addr) -> Option<Duration> {
        let peer = self.peer_by_ip(peer_ip)?;
        let tunn = peer.tunn.lock().expect("tunn lock poisoned");
        tunn.time_since_last_handshake()
    }

    /// a cheap, cloneable handle for HANDSHAKE PROBES that must outlive the move
    /// of this device into the effect (and thence into the executor).
    ///
    /// the device's state already lives behind an `Arc`, so this is a refcount
    /// bump — it exists because applying a tunnel CONFIG and completing a
    /// WireGuard HANDSHAKE are different events, and until now only the former
    /// was observable. that gap is the difference between "the overlay never came
    /// up" and "the overlay is up but the peer is dark", which are two different
    /// bugs that presented as one string.
    pub fn handshake_probe(&self) -> HandshakeProbe {
        HandshakeProbe {
            inner: self.inner.clone(),
        }
    }

    fn peer_by_ip(&self, ip: Ipv6Addr) -> Option<Arc<PeerState>> {
        let table = self.inner.peers.read().expect("peer table lock poisoned");
        table.by_ip.get(&ip).cloned()
    }
}

/// a read-only view of the device's peer table, for liveness sampling.
#[derive(Clone)]
pub struct HandshakeProbe {
    inner: Arc<DeviceInner>,
}

/// the publishable probe handle, mirroring [`super::stack::StackSlot`]: the
/// sampler is wired before the effect (and its device) exists, so it holds this
/// slot and reads through it once `apply` stands a backend up.
#[derive(Clone, Default)]
pub struct ProbeSlot(Arc<RwLock<Option<HandshakeProbe>>>);

impl ProbeSlot {
    pub fn new() -> Self {
        Self::default()
    }

    /// the live probe, if a backend is up. `None` before the first `apply` —
    /// which is itself the signal that the overlay never came up at all.
    pub fn get(&self) -> Option<HandshakeProbe> {
        self.0.read().expect("probe slot lock poisoned").clone()
    }

    pub(super) fn publish(&self, probe: HandshakeProbe) {
        *self.0.write().expect("probe slot lock poisoned") = Some(probe);
    }

    pub(super) fn clear(&self) {
        *self.0.write().expect("probe slot lock poisoned") = None;
    }
}

impl HandshakeProbe {
    /// every installed peer, with the time since its last COMPLETED handshake.
    ///
    /// `None` means the crypto handshake has never completed for that peer: its
    /// config was accepted and nothing ever crossed. That peer is DARK, and no
    /// event in the system said so before this.
    pub fn peers(&self) -> Vec<(Ipv6Addr, Option<Duration>)> {
        let table = self.inner.peers.read().expect("peer table lock poisoned");
        table
            .by_ip
            .iter()
            .map(|(ip, peer)| {
                let since = peer
                    .tunn
                    .lock()
                    .expect("tunn lock poisoned")
                    .time_since_last_handshake();
                (*ip, since)
            })
            .collect()
    }
}

impl Drop for WgDevice {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

/// send every staged datagram to the peer's current endpoint; endpoint-less
/// peers simply drop them (a handshake toward an unknown endpoint cannot go
/// anywhere — it fires once the peer roams in).
async fn send_to_endpoint(inner: &DeviceInner, peer: &PeerState, packets: Vec<Vec<u8>>) {
    if packets.is_empty() {
        return;
    }
    let Some(endpoint) = *peer.endpoint.read().expect("endpoint lock poisoned") else {
        return;
    };
    for pkt in packets {
        // a transient underlay send failure is the medium losing a datagram —
        // WireGuard's timers retransmit what matters.
        let _ = inner.underlay.send_to(&pkt, endpoint).await;
    }
}

/// the underlay's WG lane → `Tunn::decapsulate` → stack. also owns endpoint
/// roaming and the inbound half of cryptokey routing.
async fn inbound_pump(
    inner: Arc<DeviceInner>,
    mut wg_lane: mpsc::Receiver<Datagram>,
    to_stack: mpsc::Sender<Vec<u8>>,
) {
    let mut buf = Box::new([0u8; MAX_PACKET]);
    let unroutable = AtomicU64::new(0);
    let unadmitted = AtomicU64::new(0);
    while let Some((datagram, src)) = wg_lane.recv().await {
        // route the datagram to its peer: a handshake initiation identifies
        // the peer by its (encrypted) static key; everything else carries a
        // receiver session id whose high bits are the device-assigned index.
        let peer = {
            let parsed = match Tunn::parse_incoming_packet(&datagram) {
                Ok(parsed) => parsed,
                Err(_) => continue,
            };
            let table = inner.peers.read().expect("peer table lock poisoned");
            match parsed {
                Packet::HandshakeInit(ref init) => {
                    match parse_handshake_anon(&inner.secret, &inner.public, init) {
                        Ok(half) => table.by_key.get(&half.peer_static_public).cloned(),
                        Err(_) => None,
                    }
                }
                Packet::HandshakeResponse(resp) => {
                    table.by_index.get(&(resp.receiver_idx >> 8)).cloned()
                }
                Packet::PacketCookieReply(reply) => {
                    table.by_index.get(&(reply.receiver_idx >> 8)).cloned()
                }
                Packet::PacketData(data) => table.by_index.get(&(data.receiver_idx >> 8)).cloned(),
            }
        };
        let Some(peer) = peer else {
            note_drop(&unroutable, "wg_inbound_unroutable");
            continue;
        };

        let out = peer.tunn_call(
            TunnOp::Decapsulate {
                src: src.ip(),
                datagram: &datagram,
            },
            &mut buf,
        );

        // roaming: the datagram authenticated against this peer's tunnel, so
        // its underlay source is the peer's endpoint now.
        if out.authenticated {
            *peer.endpoint.write().expect("endpoint lock poisoned") = Some(src);
        }
        // handshake replies / cookie messages go straight back to the source.
        for pkt in out.to_network {
            let _ = inner.underlay.send_to(&pkt, src).await;
        }
        // the inbound half of cryptokey routing: a decrypted packet is only
        // admitted if its inner source address belongs to the peer that
        // carried it — the exact check the kernel's allowed-ips table does.
        for (pkt, inner_src) in out.to_tunnel {
            let admitted = match inner_src {
                IpAddr::V6(v6) => peer.config.allowed_ips.contains(&v6),
                IpAddr::V4(_) => false,
            };
            if admitted {
                // backpressure from the stack is real backpressure: block the
                // pump rather than grow an unbounded queue.
                if to_stack.send(pkt).await.is_err() {
                    return; // stack gone — the backend is shutting down.
                }
            } else {
                note_drop(&unadmitted, "wg_inner_source_unadmitted");
            }
        }
    }
}

/// stack → `Tunn::encapsulate` → UDP. the outbound half of cryptokey routing:
/// the packet's destination `/128` selects the peer (and thereby the key).
async fn outbound_pump(inner: Arc<DeviceInner>, mut from_stack: mpsc::Receiver<Vec<u8>>) {
    let mut buf = Box::new([0u8; MAX_PACKET]);
    while let Some(pkt) = from_stack.recv().await {
        let Some(IpAddr::V6(dst)) = Tunn::dst_address(&pkt) else {
            continue;
        };
        let peer = {
            let table = inner.peers.read().expect("peer table lock poisoned");
            table.by_ip.get(&dst).cloned()
        };
        // no peer owns the destination: the stack tried to reach an address
        // outside the cryptokey table — drop, exactly as the kernel would
        // (no route / no allowed-ip).
        let Some(peer) = peer else { continue };
        // with no live session, encapsulate queues the packet inside the
        // tunn and yields a handshake initiation instead — either way,
        // whatever it wants sent goes to the peer's endpoint.
        let out = peer.tunn_call(TunnOp::Encapsulate { packet: &pkt }, &mut buf);
        send_to_endpoint(&inner, &peer, out.to_network).await;
    }
}

/// drive every peer's timer machinery: handshake retransmission, persistent
/// keepalive, rekey-after-time, session expiry. this loop is what makes the
/// `Tunn`s live objects rather than passive codecs — the new moving part the
/// userspace backend adds.
async fn timer_pump(inner: Arc<DeviceInner>) {
    let mut tick = tokio::time::interval(TIMER_TICK);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut buf = Box::new([0u8; MAX_PACKET]);
    loop {
        tick.tick().await;
        let peers: Vec<Arc<PeerState>> = {
            let table = inner.peers.read().expect("peer table lock poisoned");
            table.by_index.values().cloned().collect()
        };
        for peer in peers {
            let out = peer.tunn_call(TunnOp::UpdateTimers, &mut buf);
            send_to_endpoint(&inner, &peer, out.to_network).await;
        }
    }
}
