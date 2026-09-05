//! Live interactive-terminal-session output fan-out over the WireGuard data
//! plane — the verbatim twin of [`crate::agent_plane`].
//!
//! A terminal session's raw output ring (`term:<id>`) and its ordered,
//! attributed command log (`term-cmd:<id>`) are node-local today. This plane
//! forwards BOTH to peer nodes so a member on another node streams the session
//! like a huddle. It tails the two node-local broadcast feeds
//! ([`TermRing::subscribe_appends`] / [`TermCommandRing::subscribe_appends`]),
//! fans each out to every mesh peer over a stream-class plane, and ingests peer
//! events via `append_remote` — which appends to the ring WITHOUT
//! re-broadcasting, breaking the loop. Delivery to the peer's own ws clients is
//! then free: `append_remote` bumps the ring's `watch`, waking that node's
//! `term:<id>` / `term-cmd:<id>` catch-up exactly as a local append would.
//!
//! Like all data-plane traffic this is OFF consensus (opaque bytes, non-BFT):
//! the session itself is node-local, and the forwarded command log stays
//! observational — a peer replays each command at the `seq` the origin node
//! stamped and never re-orders it.
//!
//! The two feeds ride ONE service ([`Service::TermSession`]) on two intents
//! (chunk vs command log), the same one-flow/two-intent shape
//! [`crate::code_plane`] uses for push/pull.

use std::collections::{HashMap, HashSet};
use std::io;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use data_plane::{
    BulkPacer, DataPlane, DataPlaneTransport, FlowId, PeerId, Service, SocketFactory, StreamPacing,
    StreamPlaneSpec, StreamPolicy, StreamService, bind_stream_plane,
};
use futures::SinkExt as _;
use futures::channel::{mpsc as fmpsc, oneshot};
use noded::{
    CreatedSession, NodeCommand, PeerAttach, RemoteSessions, SessionInputWire, SessionJob,
    TermCommandEvent, TermCommandRing, TermError, TermFeedEvent, TermRing, TerminalSessions,
};
use provider_host::ResolvedCredential;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};

use crate::overlay_book::{BIND_RETRY, OverlayBook, OverlayPeers, Plane, StreamPlane};

/// the raw-output feed rides intent 1, the ordered command log intent 2.
const CHUNK_INTENT: u8 = 1;
const COMMAND_INTENT: u8 = 2;
/// guest→host directed create/close, one request → one reply (mirrors gateway
/// PROXY_INTENT). NOT deduped per-peer — each create/close is its own short stream.
const CONTROL_INTENT: u8 = 3;
/// guest→host forwarded keystrokes/resizes, one persistent stream per peer,
/// creator-gated host-side.
const INPUT_INTENT: u8 = 4;
const MAX_EVENT_BYTES: usize = 64 * 1024;
/// the outbound re-dial cadence: how long a peer's fan-out task waits after
/// a failed open, and how often the fan-out re-reads the tracked set.
const DIAL_RETRY: Duration = Duration::from_secs(3);

/// guest→host directed create/close request, one request → one reply. Carries
/// NO creator field — the host knows the creator as the mesh-authenticated
/// requesting peer node, never from client-supplied bytes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum SessionControlRequest {
    Create {
        provider: String,
        cred: String,
        cpu: Option<u64>,
        mem_gb: Option<u64>,
    },
    Close {
        session: String,
    },
}

/// the host's reply to a [`SessionControlRequest`]. Refusal `reason`s are stable
/// snake_case tokens, in two families. **Whose work this host runs**:
/// `work_not_admitted`, `work_caller_unbound`, `work_policy_unreadable`,
/// `work_authority_unavailable` (see [`crate::work_admission`]). **What this
/// host can do with it**: `no_sandbox`, `unknown_credential`,
/// `provider_kind_mismatch`, `limits_exceed_host_ceiling`, `at_capacity`,
/// `unknown_provider`, `spawn_failed`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum SessionControlReply {
    Created { session: String, topic: String },
    Closed,
    Refused { reason: String, detail: String },
}

/// guest→host forwarded input event on the persistent INPUT stream. `data_b64`
/// is base64 of the raw keystroke bytes — never logged.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionInputEvent {
    Input {
        session: String,
        data_b64: String,
    },
    Resize {
        session: String,
        cols: u16,
        rows: u16,
    },
}
fn term_flow() -> FlowId {
    FlowId::derive(b"ducktape:term-session:v1")
}

/// the terminal-session plane's tag for the shared [`OverlayBook`]:
/// default-deny admission scoped to the service + session flow.
struct TermPlane;

impl Plane for TermPlane {
    const SERVICE: Service = Service::TermSession;
}

impl StreamPlane for TermPlane {
    fn flow() -> FlowId {
        term_flow()
    }
}

/// Bind the service in the background. Each peer gets one persistent outbound
/// stream per feed; inbound events enter the same node-local rings the existing
/// `term:<id>` / `term-cmd:<id>` websocket topics already tail.
/// the host-side control state shared by every accepted CONTROL/INPUT stream and
/// the guest-side client half: the session manager (`None` on a sync-only /
/// joiner node → creates refuse `no_sandbox`), the actor command lane the
/// admission queries ride, the host's own browser-gateway base URL (the `via`
/// for a resolved credential), and this node's key (loopback short-circuit).
struct ControlState {
    sessions: Option<TerminalSessions>,
    commands: fmpsc::Sender<NodeCommand>,
    local_gateway_via: String,
    me: [u8; 32],
    /// where this node's `work-admit.toml` lives. Re-read on every create, so
    /// `ducktape node work admit` takes effect without a restart — and takes
    /// effect at the same instant on the compute lane, which reads the same
    /// file the same way.
    workspace: std::path::PathBuf,
}

/// the node actor's query lane, behind the one method the admission needs.
#[async_trait::async_trait]
impl crate::work_admission::CommittedReader for fmpsc::Sender<NodeCommand> {
    async fn read(&self, target: &str, request: Vec<u8>) -> Result<Vec<u8>, String> {
        query(self, target, request).await
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn(
    label: String,
    factory: Arc<dyn SocketFactory>,
    peers: Arc<OverlayPeers>,
    me: [u8; 32],
    pacer: BulkPacer,
    planes: data_plane::PlaneMonitor,
    terminals: TermRing,
    term_commands: TermCommandRing,
    sessions: Option<TerminalSessions>,
    commands: fmpsc::Sender<NodeCommand>,
    local_gateway_via: String,
    workspace: std::path::PathBuf,
    jobs: tokio::sync::mpsc::Receiver<SessionJob>,
    remote_sessions: RemoteSessions,
) {
    tokio::spawn(async move {
        let own = peers.own_ip(&me);
        let spec = StreamPlaneSpec {
            own_ip: own,
            service: Service::TermSession,
            pacing: StreamPacing::Shared(pacer),
            policy: StreamPolicy { accept_backlog: 64 },
            retry: BIND_RETRY,
        };
        let book = OverlayBook::<TermPlane>::new(Arc::clone(&peers));
        let (plane, service) = match bind_stream_plane(spec, factory, book).await {
            Ok(bound) => bound,
            Err(error) => {
                tracing::error!(
                    target: "ducktape::term",
                    node = %label,
                    reason = "plane_register_failed",
                    %error,
                    "term session plane register failed"
                );
                return;
            }
        };
        tracing::info!(target: "ducktape::term", node = %label, %own, "term_session_plane_bound");
        planes.register("term", Service::TermSession, plane.watch());
        let control = Arc::new(ControlState {
            sessions,
            commands,
            local_gateway_via,
            me,
            workspace,
        });
        run_bound(
            plane,
            service,
            peers,
            PeerId(me),
            terminals,
            term_commands,
            control,
            jobs,
            remote_sessions,
        )
        .await;
    });
}

#[allow(clippy::too_many_arguments)]
async fn run_bound<T: DataPlaneTransport>(
    plane: DataPlane<T>,
    service: Arc<StreamService<T>>,
    peers: Arc<OverlayPeers>,
    me: PeerId,
    terminals: TermRing,
    term_commands: TermCommandRing,
    control: Arc<ControlState>,
    jobs: tokio::sync::mpsc::Receiver<SessionJob>,
    remote_sessions: RemoteSessions,
) {
    let _plane = plane;
    tokio::select! {
        _ = accept_loop(
            Arc::clone(&service),
            Arc::clone(&peers),
            terminals.clone(),
            term_commands.clone(),
            Arc::clone(&control),
            remote_sessions,
        ) => {}
        _ = fanout_loop(Arc::clone(&service), Arc::clone(&peers), me, terminals, term_commands) => {}
        _ = client_loop(service, control, jobs) => {}
    }
}

async fn accept_loop<T: DataPlaneTransport>(
    service: Arc<StreamService<T>>,
    peers: Arc<OverlayPeers>,
    terminals: TermRing,
    term_commands: TermCommandRing,
    control: Arc<ControlState>,
    remote_sessions: RemoteSessions,
) {
    // one live inbound stream per (peer, feed): the long-lived feeds (chunk /
    // command / input) hold one stream per (peer, intent), so the dedupe key
    // carries the intent. CONTROL is a short one-shot stream per create/close and
    // is deliberately NOT deduped.
    let active = Arc::new(std::sync::Mutex::new(HashSet::new()));
    while let Some((peer, hello, stream)) = service.accept().await {
        let intent = hello.intent;
        let routed = matches!(
            intent,
            CHUNK_INTENT | COMMAND_INTENT | CONTROL_INTENT | INPUT_INTENT
        );
        if !hello.meta.is_empty() || !routed {
            continue;
        }
        let deduped = intent != CONTROL_INTENT;
        let key = (peer, intent);
        if deduped && !active.lock().expect("term streams lock").insert(key) {
            continue;
        }
        let peers = Arc::clone(&peers);
        let terminals = terminals.clone();
        let term_commands = term_commands.clone();
        let control = Arc::clone(&control);
        let active = Arc::clone(&active);
        let remote_sessions = remote_sessions.clone();
        tokio::spawn(async move {
            let _ = match intent {
                CHUNK_INTENT => {
                    receive_chunks(stream, peer, peers, terminals, remote_sessions).await
                }
                COMMAND_INTENT => {
                    receive_commands(stream, peer, peers, term_commands, remote_sessions).await
                }
                INPUT_INTENT => receive_input(stream, peer, peers, control).await,
                CONTROL_INTENT => serve_control(stream, peer, control).await,
                _ => Ok(()),
            };
            if deduped {
                active.lock().expect("term streams lock").remove(&key);
            }
        });
    }
}

/// per-frame refusals on the inbound feeds: a peer drives these — one frame per
/// output chunk — so an unlatched line is a log bomb. Keyed per (reason, peer)
/// like [`CREATE_REFUSED`], so one peer's flood never silences another's first
/// refusal.
static FEED_REFUSED: PerPeerLatch = PerPeerLatch::new(100);

/// the host gate on an inbound feed grain: this node takes a session's output
/// and command rows ONLY from the peer it recorded as that session's host when
/// its own remote create returned.
///
/// A session id authorizes nothing on its own — every forwarded session's grains
/// fan out to EVERY peer, so the ids are public to the mesh. Without this bind,
/// any member could end a session it does not host, inject bytes into its
/// scrollback, or forge attributed command rows, on any id — including one this
/// node hosts locally, where the ring is keyed in the same namespace.
///
/// An id this node is not mirroring (`None`) is refused too: a session this node
/// never created is not one it has any reason to ring.
fn feed_permitted(remote: &RemoteSessions, session: &str, peer: PeerId) -> bool {
    remote.host_of(session) == Some(peer.0)
}

/// log one refused feed grain, latched. The peer's NODE key is public routing
/// metadata already logged at boot; without it the operator is told "something
/// was dropped" with no way to find out who is sending it.
fn feed_refused(peer: PeerId) {
    if let Some(occurrences) = FEED_REFUSED.hit("feed_not_session_host", peer) {
        tracing::warn!(
            target: "ducktape::term",
            reason = "feed_not_session_host",
            node = %crate::config::hex_bytes(&peer.0[..4]),
            occurrences,
            "term feed grain dropped"
        );
    }
}

async fn receive_chunks<S: AsyncRead + Unpin>(
    mut stream: S,
    peer: PeerId,
    peers: Arc<OverlayPeers>,
    ring: TermRing,
    remote_sessions: RemoteSessions,
) -> io::Result<()> {
    while peers.contains(peer) {
        let Some(event) = read_frame::<_, TermFeedEvent>(&mut stream).await? else {
            return Ok(());
        };
        if !peers.contains(peer) {
            return Ok(());
        }
        // a peer sending a malformed session id is a bug, not consensus — skip
        // the grain and keep the stream (best-effort, observational).
        if !agent_service::wire::valid_session(event.session()) {
            continue;
        }
        if !feed_permitted(&remote_sessions, event.session(), peer) {
            feed_refused(peer);
            continue;
        }
        match event {
            TermFeedEvent::Chunk(chunk) => ring.append_remote(chunk),
            // the host says the pty is over. Flag it LOCAL-ONLY: this node is
            // mirroring someone else's session, so re-publishing would fan the
            // grain back out. Flagging it is what lets this node's `term:<id>`
            // catch-up emit `TermEnded` and release the `agent pty` client that
            // has been blocked on the topic since the child exited.
            TermFeedEvent::Ended { session } => ring.mark_ended_local_only(&session),
        }
    }
    Ok(())
}

async fn receive_commands<S: AsyncRead + Unpin>(
    mut stream: S,
    peer: PeerId,
    peers: Arc<OverlayPeers>,
    ring: TermCommandRing,
    remote_sessions: RemoteSessions,
) -> io::Result<()> {
    while peers.contains(peer) {
        let Some(event) = read_frame::<_, TermCommandEvent>(&mut stream).await? else {
            return Ok(());
        };
        if !peers.contains(peer) {
            return Ok(());
        }
        if !agent_service::wire::valid_session(&event.session) {
            continue;
        }
        // only the session's host may append attributed rows to its command log;
        // a local command takes the `append` path, never this one.
        if !feed_permitted(&remote_sessions, &event.session, peer) {
            feed_refused(peer);
            continue;
        }
        // the ring validates the seq against its own cursor — a peer's number is
        // checked, never assigned.
        ring.append_remote(event);
    }
    Ok(())
}

async fn fanout_loop<T: DataPlaneTransport>(
    service: Arc<StreamService<T>>,
    peers: Arc<OverlayPeers>,
    me: PeerId,
    terminals: TermRing,
    term_commands: TermCommandRing,
) {
    let mut tasks: HashMap<PeerId, tokio::task::JoinHandle<()>> = HashMap::new();
    loop {
        tasks.retain(|peer, task| {
            let keep = peers.contains(*peer) && !task.is_finished();
            if !keep {
                task.abort();
            }
            keep
        });
        for peer in peers.peer_ids().into_iter().filter(|peer| *peer != me) {
            tasks.entry(peer).or_insert_with(|| {
                tokio::spawn(send_peer(
                    Arc::clone(&service),
                    Arc::clone(&peers),
                    peer,
                    terminals.clone(),
                    term_commands.clone(),
                ))
            });
        }
        tokio::time::sleep(DIAL_RETRY).await;
    }
}

/// one task per peer: hold two persistent outbound streams (chunk + command)
/// and pump each feed onto its stream. Subscribes both feeds ONCE, outside the
/// reconnect loop, so a re-open never replays an already-consumed grain.
async fn send_peer<T: DataPlaneTransport>(
    service: Arc<StreamService<T>>,
    peers: Arc<OverlayPeers>,
    peer: PeerId,
    terminals: TermRing,
    term_commands: TermCommandRing,
) {
    let mut chunks = terminals.subscribe_appends();
    let mut commands = term_commands.subscribe_appends();
    while peers.contains(peer) {
        let mut chunk_stream = match service
            .open(peer, term_flow(), CHUNK_INTENT, Vec::new())
            .await
        {
            Ok(stream) => stream,
            Err(_) => {
                tokio::time::sleep(DIAL_RETRY).await;
                continue;
            }
        };
        let mut cmd_stream = match service
            .open(peer, term_flow(), COMMAND_INTENT, Vec::new())
            .await
        {
            Ok(stream) => stream,
            Err(_) => {
                tokio::time::sleep(DIAL_RETRY).await;
                continue;
            }
        };
        loop {
            tokio::select! {
                event = chunks.recv() => match event {
                    Ok(event) => {
                        if agent_service::wire::valid_session(event.session())
                            && write_frame(&mut chunk_stream, &event).await.is_err()
                        {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
                },
                event = commands.recv() => match event {
                    Ok(event) => {
                        if agent_service::wire::valid_session(&event.session)
                            && write_frame(&mut cmd_stream, &event).await.is_err()
                        {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
                },
                _ = tokio::time::sleep(DIAL_RETRY) => {
                    if !peers.contains(peer) {
                        return;
                    }
                }
            }
        }
    }
}

/// length-prefixed JSON, one event per frame. Generic over the event type: the
/// two feeds share the identical codec, differing only in their payload struct.
async fn write_frame<S: AsyncWrite + Unpin + ?Sized, E: Serialize>(
    stream: &mut S,
    event: &E,
) -> io::Result<()> {
    let payload = serde_json::to_vec(event).map_err(io::Error::other)?;
    if payload.len() > MAX_EVENT_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "term event too large",
        ));
    }
    stream
        .write_all(&(payload.len() as u32).to_be_bytes())
        .await?;
    stream.write_all(&payload).await
}

async fn read_frame<S: AsyncRead + Unpin, E: DeserializeOwned>(
    stream: &mut S,
) -> io::Result<Option<E>> {
    let mut len = [0u8; 4];
    match stream.read_exact(&mut len).await {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error),
    }
    let len = u32::from_be_bytes(len) as usize;
    if len == 0 || len > MAX_EVENT_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid term frame length",
        ));
    }
    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload).await?;
    serde_json::from_slice(&payload)
        .map(Some)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

// ---------------------------------------------------------------------------
// host side: control handler (create/close admission) + creator-gated input
// ---------------------------------------------------------------------------

/// serve ONE guest→host control exchange: read the request, dispatch on its
/// variant (one delegation each), write the single reply. No loop — CONTROL is a
/// short stream per create/close.
async fn serve_control<S: AsyncRead + AsyncWrite + Unpin>(
    mut stream: S,
    peer: PeerId,
    control: Arc<ControlState>,
) -> io::Result<()> {
    let Some(request) = read_frame::<_, SessionControlRequest>(&mut stream).await? else {
        return Ok(());
    };
    let reply = match request {
        SessionControlRequest::Create {
            provider,
            cred,
            cpu,
            mem_gb,
        } => serve_create(&control, peer, &provider, &cred, cpu, mem_gb).await,
        SessionControlRequest::Close { session } => serve_close(&control, &session).await,
    };
    write_frame(&mut stream, &reply).await
}

/// the host's create path: resolve the named credential from committed gateway
/// state, decide what THIS HOST admits (sandbox present, credential registered,
/// provider/kind agreement, size ceiling), then spawn a peer-attached session
/// drawing on the guest's self-host gateway. Every refusal carries a stable
/// snake_case `reason`.
///
/// It deliberately does NOT resolve the creator's account *to ship to the
/// lender*. Whether the credential may be drawn on is the LENDER's decision,
/// made against the account its own node stamps when this session's traffic
/// makes the gateway hop — which is this host, not the peer.
///
/// It DOES ask its own [`crate::work_admission`] policy whether it runs this
/// peer's work at all. That is the opposite direction and a different question:
/// not a claim about who a session acts for, but this host's own answer about
/// whose workload it hosts. Without it, "a grant lends to that account's node,
/// for whatever workload it runs" means any mesh peer naming any registered
/// credential gets a container here, on this node's grants.
///
/// `peer` is also still used for the one thing it settles locally: binding the
/// session's input frames to the node that created it.
///
/// every refusal below is PEER-drivable — `serve_control` opens one short
/// CONTROL stream per create/close, and a peer that keeps retrying a refused
/// create can open as many of those as it likes — so they latch by reason
/// instead of flooding, the same discipline `sync::serve`'s statesync refusals
/// use. First occurrence, then every 100th, carrying `occurrences` — but keyed
/// per (reason, peer): the `node` field above exists so the operator knows
/// whom to admit, and a latch keyed on the reason alone silences peer B's
/// first refusal because peer A already hit the same reason.
static CREATE_REFUSED: PerPeerLatch = PerPeerLatch::new(100);

/// Like [`noded::log::Latch`], but keyed on `(reason, peer)` instead of just
/// `reason` — `Latch::hit` only takes a `&'static str`, and a peer id is not
/// one. First occurrence per peer, then every `every`th, per peer.
struct PerPeerLatch {
    counts: std::sync::Mutex<std::collections::BTreeMap<(&'static str, PeerId), u64>>,
    every: u64,
}

impl PerPeerLatch {
    const fn new(every: u64) -> Self {
        Self {
            counts: std::sync::Mutex::new(std::collections::BTreeMap::new()),
            every,
        }
    }

    /// returns `Some(occurrences)` when this peer's occurrence of `reason`
    /// should be logged.
    fn hit(&self, reason: &'static str, peer: PeerId) -> Option<u64> {
        let mut counts = self.counts.lock().expect("latch lock poisoned");
        let count = counts.entry((reason, peer)).or_insert(0);
        *count += 1;
        let n = *count;
        (n == 1 || n.is_multiple_of(self.every)).then_some(n)
    }
}

async fn serve_create(
    control: &ControlState,
    peer: PeerId,
    provider: &str,
    cred: &str,
    cpu: Option<u64>,
    mem_gb: Option<u64>,
) -> SessionControlReply {
    // Whose work does this host run? Asked FIRST, because it depends on nothing
    // about the credential and refusing later would mean reading committed
    // state on a stranger's behalf. `peer.0` is mesh-authenticated (the plane's
    // `permits` gate ran before this stream was accepted) or, on the own-node
    // loopback, this node's own key — derived either way, never asserted.
    match crate::work_admission::admit(
        &control.commands,
        &control.workspace,
        &control.me,
        crate::work_admission::WorkSource::Peer(&peer.0),
    )
    .await
    {
        crate::work_admission::WorkVerdict::Admitted => {}
        crate::work_admission::WorkVerdict::Refused(refusal) => {
            // the peer's NODE key, never its account: a node key is public
            // routing metadata already logged at boot, and without it the
            // operator is told "someone was refused" with no way to find out
            // whom to admit.
            if let Some(occurrences) = CREATE_REFUSED.hit(refusal.reason(), peer) {
                tracing::warn!(
                    target: "ducktape::term",
                    reason = refusal.reason(),
                    node = %crate::config::hex_bytes(&peer.0[..4]),
                    occurrences,
                    "peer session create refused"
                );
            }
            return refused(refusal.reason(), refusal.detail());
        }
        // not a refusal: nothing is known about the policy's subject, so the
        // caller is told to retry rather than sent to fix an admission that may
        // already exist.
        crate::work_admission::WorkVerdict::AuthorityUnavailable => {
            if let Some(occurrences) = CREATE_REFUSED.hit("work_authority_unavailable", peer) {
                tracing::warn!(
                    target: "ducktape::term",
                    reason = "work_authority_unavailable",
                    node = %crate::config::hex_bytes(&peer.0[..4]),
                    occurrences,
                    "peer session create not decided"
                );
            }
            return refused(
                "work_authority_unavailable",
                "this node could not read committed identity to decide whose work it runs",
            );
        }
    }
    let Some(sessions) = control.sessions.clone() else {
        if let Some(occurrences) = CREATE_REFUSED.hit("no_sandbox", peer) {
            tracing::warn!(
                target: "ducktape::term",
                reason = "no_sandbox",
                occurrences,
                "peer session create refused"
            );
        }
        return refused("no_sandbox", "this node hosts no terminal sessions");
    };
    let sandbox_present = sessions.has_sandbox();
    // The creator's ACCOUNT is deliberately not looked up. It used to be, and it
    // used to be shipped to the lender as the grant subject — but the lender
    // authorizes the account ITS node vouched for on the gateway hop, which is
    // THIS host, the one running the sandbox. A creator account resolved here
    // could only ever be a second, uncheckable answer to a question the lender
    // has already answered. `peer.0` still binds the session's input frames to
    // its creator; that is a node-key gate, not an identity claim.
    let record = match credential_record(&control.commands, cred).await {
        Ok(record) => record,
        Err(detail) => return refused("unknown_credential", &detail),
    };
    let admit = match admit_create(provider, record.as_ref(), cpu, mem_gb, sandbox_present) {
        Ok(admit) => admit,
        Err((reason, detail)) => {
            if let Some(occurrences) = CREATE_REFUSED.hit(reason, peer) {
                tracing::warn!(
                    target: "ducktape::term",
                    reason,
                    occurrences,
                    "peer session create refused"
                );
            }
            return refused(reason, &detail);
        }
    };
    let authority = match owner_airlock_authority(&control.commands, admit.owner_account).await {
        Ok(authority) => authority,
        Err(detail) => return refused("unknown_credential", &detail),
    };
    // bin/node owns the record → ResolvedCredential mapping (provider-host must
    // not depend on the gateway crate): the seal_pk is the on-chain anchor, the
    // via is the host's own browser-gateway, the authority is the owner's airlock
    // route.
    let resolved = ResolvedCredential {
        name: admit.name,
        kind: admit.kind,
        authority,
        via: control.local_gateway_via.clone(),
        seal_pk: admit.seal_pk,
    };
    // the record travels to the agent daemon, which pins it as its broker's
    // self-host airlock upstream. Nothing secret crosses, and no identity either:
    // a name, an authority handle, this node's own gateway `via`, and a PUBLIC
    // seal key.
    let attach = PeerAttach {
        creator_node: peer.0,
        credential: agent_service::credential_wire(&resolved),
        limits: admit.limits,
    };
    match sessions.create_for_peer(provider, attach).await {
        Ok(created) => SessionControlReply::Created {
            session: created.session_id,
            topic: created.topic,
        },
        Err(err) => refused_from_term_error(err),
    }
}

/// close on the host: idempotent teardown. The host owns lifecycle; a close from
/// a non-creator names a random id it would have to already know, and the
/// wall-clock + kill-on-drop backstops hold — creator-binding close is a named
/// follow-up.
async fn serve_close(control: &ControlState, session: &str) -> SessionControlReply {
    if let Some(sessions) = &control.sessions {
        sessions.close(session).await;
    }
    SessionControlReply::Closed
}

fn refused(reason: &str, detail: &str) -> SessionControlReply {
    SessionControlReply::Refused {
        reason: reason.to_string(),
        detail: detail.to_string(),
    }
}

/// map the manager's spawn-path error to the matching refusal reason.
fn refused_from_term_error(err: TermError) -> SessionControlReply {
    let (reason, detail) = match err {
        TermError::NoSandbox => (
            "no_sandbox",
            // the same fact the HTTP path reports, worded the same way: this
            // host CAN sandbox, it just has no agent service attached.
            "this node has no agent service attached".to_string(),
        ),
        TermError::AtCapacity => (
            "at_capacity",
            "the host terminal-session cap is reached".to_string(),
        ),
        TermError::Resolve(detail) => ("unknown_provider", detail),
        TermError::Spawn(detail) => ("spawn_failed", detail),
    };
    refused(reason, &detail)
}

/// the host's create decision, given committed state already fetched. Pure so it
/// is unit-testable without a pty. `Ok` carries the resolved credential pieces +
/// container limits; `Err` is a `(reason, detail)`.
#[derive(Debug)]
struct AdmitOk {
    name: String,
    kind: provider_host::CredentialKind,
    seal_pk: [u8; 32],
    owner_account: u64,
    limits: std::collections::BTreeMap<String, u64>,
}

/// What survives is what this HOST knows about itself and the record: can it
/// sandbox, does the name exist, does the requested provider contradict the
/// credential's vendor, what limits apply. The grant check that used to sit here
/// does not: it decided, against a creator account this node resolved, a question
/// the lender decides against the account it vouches for — and the two are
/// different parties the moment the creator is not the host.
fn admit_create(
    provider: &str,
    record: Option<&gateway::CredentialRecord>,
    cpu: Option<u64>,
    mem_gb: Option<u64>,
    sandbox_present: bool,
) -> Result<AdmitOk, (&'static str, String)> {
    if !sandbox_present {
        // `sandbox_present` is `has_sandbox()` = "is an agent service attached",
        // NOT "is a sandbox image configured" — word it as the fact it tested
        // (and as `refused_from_term_error` already does). The old "no
        // configured sandbox image" text sent an operator with a perfectly
        // good `[sandbox]` table hunting the wrong config.
        return Err((
            "no_sandbox",
            "this node has no agent service attached".into(),
        ));
    }
    let Some(record) = record else {
        return Err((
            "unknown_credential",
            "no credential by that name is registered".into(),
        ));
    };
    let over_ceiling = cpu.is_some_and(|cores| cores > MAX_SESSION_CORES)
        || mem_gb.is_some_and(|mem| mem > MAX_SESSION_MEM_GB);
    if over_ceiling {
        return Err((
            "limits_exceed_host_ceiling",
            format!(
                "this host caps a session at {MAX_SESSION_CORES} cores / \
                 {MAX_SESSION_MEM_GB} GB"
            ),
        ));
    }
    let contradicts = provider_contradicts_kind(provider, record.kind);
    if contradicts {
        return Err((
            "provider_kind_mismatch",
            format!("provider {provider} contradicts the credential kind"),
        ));
    }
    Ok(AdmitOk {
        name: record.name.clone(),
        kind: crate::compute::cred::service_kind(record.kind),
        seal_pk: record.seal_pk,
        owner_account: record.owner_account,
        limits: build_limits(cpu, mem_gb),
    })
}

/// true when an EXPLICIT vendor provider tag contradicts the credential's kind.
/// An unknown tag (a test provider) is not a contradiction — the manager's
/// provider resolution decides it (→ `unknown_provider`).
fn provider_contradicts_kind(provider: &str, kind: gateway::CredentialKind) -> bool {
    match provider {
        "claude" => kind != gateway::CredentialKind::Claude,
        "codex" => kind != gateway::CredentialKind::Codex,
        _ => false,
    }
}

/// Per-session ceiling on what a REMOTE creator may ask this host to allocate.
///
/// `cpu`/`mem_gb` arrive from a mesh peer and go straight to the sandbox
/// backend. Before the credential gate moved to the lender, a stranger could not
/// reach [`build_limits`] at all on a node they held no grant on; now any
/// admitted member naming any registered credential can, so the size of the
/// container they get is a number they choose. `TermError::AtCapacity` bounds
/// the session COUNT, not the size of one.
///
/// Refused rather than silently clamped: quietly handing back a tenth of what
/// was asked for is the fail-quiet this repo's refusal doctrine exists to
/// prevent, and the reason token tells the caller what actually happened.
///
/// ponytail: a constant, not the host's real capacity. The compute plane already
/// models that (`compute_service::ResourceLedger`); wiring one into the term
/// plane is the upgrade when a host wants to sell its actual size.
const MAX_SESSION_CORES: u64 = 8;
const MAX_SESSION_MEM_GB: u64 = 32;

/// `--cpu`/`--mem` → the container limit keys the sandbox backend enforces.
fn build_limits(cpu: Option<u64>, mem_gb: Option<u64>) -> std::collections::BTreeMap<String, u64> {
    let mut limits = std::collections::BTreeMap::new();
    if let Some(cores) = cpu {
        limits.insert("cores".to_string(), cores);
    }
    if let Some(mem) = mem_gb {
        limits.insert("mem_gb".to_string(), mem);
    }
    limits
}

/// the creator gate: a forwarded input frame is written only when it arrives from
/// the exact node that created the session. `None` (a local session, or an
/// unknown id) is refused — it is not a peer-attached session.
fn input_permitted(creator: Option<[u8; 32]>, peer: PeerId) -> bool {
    creator == Some(peer.0)
}

/// the host's forwarded-input stream: read events until EOF, gate each on the
/// creator, and write the permitted ones to the pty. Never logs the bytes.
async fn receive_input<S: AsyncRead + Unpin>(
    mut stream: S,
    peer: PeerId,
    peers: Arc<OverlayPeers>,
    control: Arc<ControlState>,
) -> io::Result<()> {
    let Some(sessions) = control.sessions.clone() else {
        return Ok(());
    };
    while peers.contains(peer) {
        let Some(event) = read_frame::<_, SessionInputEvent>(&mut stream).await? else {
            return Ok(());
        };
        if !peers.contains(peer) {
            return Ok(());
        }
        deliver_input(&sessions, peer, event).await;
    }
    Ok(())
}

/// per-frame `ducktape::term` refusals below `deliver_input`/`client_input`:
/// one comes in per keystroke on the forwarded-input stream, so an unlatched
/// line here is the same ~30/s ring eviction [`TERM_WARN`] in `noded::stream`
/// guards against. First occurrence, then every 100th, carrying `occurrences`.
static INPUT_WARN: noded::log::Latch = noded::log::Latch::new(100);

/// gate one input event on the creator, then apply it to the pty (write or
/// resize). Shared by the remote input stream and the loopback client path.
async fn deliver_input(sessions: &TerminalSessions, peer: PeerId, event: SessionInputEvent) {
    let session = input_session(&event).to_string();
    // an unknown (or already-ended) session and a peer that does not own a live
    // one are different diagnoses, and `creator_node` returns `None` for both —
    // so establish existence first, or every stale id would be counted as an
    // authorization failure.
    if sessions.mode(&session).is_none() {
        if let Some(occurrences) = INPUT_WARN.hit("unknown_session") {
            tracing::warn!(
                target: "ducktape::term",
                reason = "unknown_session",
                occurrences,
                "forwarded input dropped"
            );
        }
        return;
    }
    let permitted = input_permitted(sessions.creator_node(&session), peer);
    if !permitted {
        if let Some(occurrences) = INPUT_WARN.hit("input_not_creator") {
            tracing::warn!(
                target: "ducktape::term",
                reason = "input_not_creator",
                occurrences,
                "forwarded input dropped"
            );
        }
        return;
    }
    match event {
        SessionInputEvent::Input { data_b64, .. } => {
            // decoded only to refuse a malformed frame at this boundary; the
            // daemon takes the base64 as-is, so the bytes never round-trip.
            if STANDARD.decode(&data_b64).is_err() {
                if let Some(occurrences) = INPUT_WARN.hit("bad_base64") {
                    tracing::warn!(
                        target: "ducktape::term",
                        reason = "bad_base64",
                        occurrences,
                        "forwarded input dropped"
                    );
                }
                return;
            }
            sessions.input(&session, &data_b64).await;
        }
        SessionInputEvent::Resize { cols, rows, .. } => {
            sessions.resize(&session, cols, rows).await;
        }
    }
}

fn input_session(event: &SessionInputEvent) -> &str {
    match event {
        SessionInputEvent::Input { session, .. } => session,
        SessionInputEvent::Resize { session, .. } => session,
    }
}

// ---------------------------------------------------------------------------
// guest side: the client half draining the SessionJob lane
// ---------------------------------------------------------------------------

/// drain the guest's remote-session lane: a `Create`/`Close` is one CONTROL
/// round-trip, an `Input` rides a persistent per-host INPUT stream. When the
/// host is THIS node it short-circuits over a local duplex / straight to the
/// manager, so a single node still exercises the real frame path.
async fn client_loop<T: DataPlaneTransport>(
    service: Arc<StreamService<T>>,
    control: Arc<ControlState>,
    mut jobs: tokio::sync::mpsc::Receiver<SessionJob>,
) {
    let mut input_streams: HashMap<[u8; 32], Box<dyn AsyncWrite + Unpin + Send>> = HashMap::new();
    while let Some(job) = jobs.recv().await {
        match job {
            SessionJob::Create {
                host,
                provider,
                cred,
                cpu,
                mem_gb,
                reply,
            } => {
                let result =
                    client_create(&service, &control, host, provider, cred, cpu, mem_gb).await;
                let _ = reply.send(result);
            }
            SessionJob::Close { host, session } => {
                input_streams.remove(&host);
                client_close(&service, &control, host, session).await;
            }
            SessionJob::Input { host, event } => {
                client_input(&service, &control, &mut input_streams, host, event).await;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn client_create<T: DataPlaneTransport>(
    service: &Arc<StreamService<T>>,
    control: &Arc<ControlState>,
    host: [u8; 32],
    provider: String,
    cred: String,
    cpu: Option<u64>,
    mem_gb: Option<u64>,
) -> Result<CreatedSession, String> {
    let request = SessionControlRequest::Create {
        provider,
        cred,
        cpu,
        mem_gb,
    };
    match client_control(service, control, host, request).await? {
        SessionControlReply::Created { session, topic } => Ok(CreatedSession {
            session_id: session,
            topic,
        }),
        SessionControlReply::Refused { reason, detail } => Err(format!("{reason}: {detail}")),
        SessionControlReply::Closed => Err("host replied Closed to a create request".into()),
    }
}

async fn client_close<T: DataPlaneTransport>(
    service: &Arc<StreamService<T>>,
    control: &Arc<ControlState>,
    host: [u8; 32],
    session: String,
) {
    // best-effort: the host owns lifecycle backstops if the close never lands.
    let _ = client_control(
        service,
        control,
        host,
        SessionControlRequest::Close { session },
    )
    .await;
}

/// one CONTROL round-trip: loopback over a local duplex when the host is this
/// node (the creator is us), else open a CONTROL stream to the host peer.
async fn client_control<T: DataPlaneTransport>(
    service: &Arc<StreamService<T>>,
    control: &Arc<ControlState>,
    host: [u8; 32],
    request: SessionControlRequest,
) -> Result<SessionControlReply, String> {
    if host == control.me {
        let (server_end, mut caller_end) = tokio::io::duplex(64 * 1024);
        let control = Arc::clone(control);
        tokio::spawn(async move {
            let _ = serve_control(server_end, PeerId(control.me), control).await;
        });
        write_frame(&mut caller_end, &request)
            .await
            .map_err(|error| error.to_string())?;
        read_frame::<_, SessionControlReply>(&mut caller_end)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "loopback control stream closed without a reply".to_string())
    } else {
        let mut stream = service
            .open(PeerId(host), term_flow(), CONTROL_INTENT, Vec::new())
            .await
            .map_err(|error| error.to_string())?;
        write_frame(&mut stream, &request)
            .await
            .map_err(|error| error.to_string())?;
        read_frame::<_, SessionControlReply>(&mut stream)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "host closed the control stream without a reply".to_string())
    }
}

/// forward one input event to the host: loopback straight to the local manager
/// (same creator gate), else write it on the persistent per-host INPUT stream,
/// reopening on error.
async fn client_input<T: DataPlaneTransport>(
    service: &Arc<StreamService<T>>,
    control: &Arc<ControlState>,
    input_streams: &mut HashMap<[u8; 32], Box<dyn AsyncWrite + Unpin + Send>>,
    host: [u8; 32],
    event: SessionInputWire,
) {
    let event = wire_to_event(event);
    if host == control.me {
        if let Some(sessions) = &control.sessions {
            deliver_input(sessions, PeerId(control.me), event).await;
        }
        return;
    }
    // open lazily on the first frame for this host; the entry API can't span the
    // async open, so this is a plain get-or-open, not a `contains_key`+`insert`.
    if input_streams.get(&host).is_none() {
        let opened = service
            .open(PeerId(host), term_flow(), INPUT_INTENT, Vec::new())
            .await;
        match opened {
            Ok(stream) => {
                input_streams.insert(host, Box::new(stream));
            }
            Err(err) => {
                if let Some(occurrences) = INPUT_WARN.hit("input_open_failed") {
                    tracing::warn!(
                        target: "ducktape::term",
                        reason = "input_open_failed",
                        error = %err,
                        occurrences,
                        "forwarded input dropped"
                    );
                }
                return;
            }
        }
    }
    let stream = input_streams
        .get_mut(&host)
        .expect("input stream just inserted");
    if let Err(err) = write_frame(stream.as_mut(), &event).await {
        if let Some(occurrences) = INPUT_WARN.hit("input_write_failed") {
            tracing::warn!(
                target: "ducktape::term",
                reason = "input_write_failed",
                error = %err,
                occurrences,
                "forwarded input dropped; reopening"
            );
        }
        input_streams.remove(&host);
    }
}

fn wire_to_event(event: SessionInputWire) -> SessionInputEvent {
    match event {
        SessionInputWire::Input { session, data_b64 } => {
            SessionInputEvent::Input { session, data_b64 }
        }
        SessionInputWire::Resize {
            session,
            cols,
            rows,
        } => SessionInputEvent::Resize {
            session,
            cols,
            rows,
        },
    }
}

// ---------------------------------------------------------------------------
// committed-state queries over the node actor lane (copied from gateway_plane)
// ---------------------------------------------------------------------------

async fn query(
    commands: &fmpsc::Sender<NodeCommand>,
    target: &str,
    req: Vec<u8>,
) -> Result<Vec<u8>, String> {
    let (reply, rx) = oneshot::channel();
    let mut commands = commands.clone();
    commands
        .send(NodeCommand::Query {
            target: target.into(),
            req,
            reply,
        })
        .await
        .map_err(|_| "node actor is gone".to_string())?;
    rx.await
        .map_err(|_| "node actor dropped the query".to_string())?
}

/// the committed credential record for `name`, or `None` when unregistered.
async fn credential_record(
    commands: &fmpsc::Sender<NodeCommand>,
    name: &str,
) -> Result<Option<gateway::CredentialRecord>, String> {
    let reply = query(
        commands,
        "gateway",
        gateway::encode_query(&gateway::GatewayQuery::Credential {
            name: name.to_string(),
        }),
    )
    .await?;
    match gateway::decode_reply(&reply)? {
        gateway::GatewayReply::Credential(record) => Ok(record),
        _ => Err("unexpected gateway credential reply".into()),
    }
}

/// the `airlock.<handle>.duck` authority for the credential owner's co-hosted
/// gateway, resolved from the owner account's `.duck` handle registration.
async fn owner_airlock_authority(
    commands: &fmpsc::Sender<NodeCommand>,
    owner_account: u64,
) -> Result<String, String> {
    // the registrations query is paginated and the module HARD-CAPS a page at
    // MAX_QUERY_LIMIT (a larger `limit` is rejected outright), so page through in
    // MAX_QUERY_LIMIT chunks until the owner's handle is found or a short page
    // marks the end of the listing.
    let mut from = 0u64;
    loop {
        let reply = query(
            commands,
            "gateway",
            gateway::encode_query(&gateway::GatewayQuery::Registrations {
                from,
                limit: gateway::MAX_QUERY_LIMIT,
            }),
        )
        .await?;
        let page = match gateway::decode_reply(&reply)? {
            gateway::GatewayReply::Registrations(registrations) => registrations,
            _ => return Err("unexpected gateway registrations reply".into()),
        };
        let owned = page
            .iter()
            .find(|registration| registration.account_id == owner_account);
        if let Some(registration) = owned {
            return Ok(format!("airlock.{}.duck", registration.handle));
        }
        let page_len = page.len() as u64;
        let listing_exhausted = page_len < gateway::MAX_QUERY_LIMIT;
        if listing_exhausted {
            return Err("credential owner has no registered duck handle".into());
        }
        from += page_len;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use noded::TermChunkEvent;

    #[test]
    fn per_peer_latch_logs_each_peers_first_refusal() {
        let latch = PerPeerLatch::new(100);
        let a = PeerId([1u8; 32]);
        let b = PeerId([2u8; 32]);
        // peer A's first hit on a reason logs...
        assert_eq!(latch.hit("no_sandbox", a), Some(1));
        // ...its second does not, latched same as the crate-wide Latch...
        assert_eq!(latch.hit("no_sandbox", a), None);
        // ...but peer B's FIRST hit on the SAME reason still logs: the whole
        // point is that one peer's refusal never silences another's.
        assert_eq!(latch.hit("no_sandbox", b), Some(1));
    }

    #[test]
    fn a_feed_grain_binds_to_the_session_host() {
        let remote = RemoteSessions::default();
        let host = PeerId([7u8; 32]);
        let stranger = PeerId([9u8; 32]);
        remote.remember("00000000deadbeef".into(), host.0);
        assert!(feed_permitted(&remote, "00000000deadbeef", host));
        // the same id from any other member is not this session's output.
        assert!(!feed_permitted(&remote, "00000000deadbeef", stranger));
        // an id this node is not mirroring — one it hosts locally, or one a peer
        // invented — is bound to no peer at all.
        assert!(!feed_permitted(&remote, "00000000cafef00d", host));
    }

    #[test]
    fn valid_session_accepts_16_hex_and_rejects_the_rest() {
        assert!(agent_service::wire::valid_session("00000000deadbeef"));
        assert!(!agent_service::wire::valid_session("deadbeef"), "too short");
        assert!(
            !agent_service::wire::valid_session("00000000deadbeef0"),
            "too long"
        );
        assert!(
            !agent_service::wire::valid_session("00000000deadbeeg"),
            "non-hex digit"
        );
        assert!(!agent_service::wire::valid_session(""), "empty");
    }

    #[tokio::test]
    async fn frame_round_trips_both_event_types() {
        let (mut a, mut b) = tokio::io::duplex(64 * 1024);
        let chunk = TermFeedEvent::Chunk(TermChunkEvent {
            session: "00000000deadbeef".into(),
            chunk_b64: "aGVsbG8=".into(),
        });
        write_frame(&mut a, &chunk).await.unwrap();
        let got: TermFeedEvent = read_frame(&mut b).await.unwrap().unwrap();
        assert_eq!(got, chunk);

        // the terminal grain rides the SAME stream as the bytes — a peer that
        // could not decode it would leave every cross-node `agent pty` attached
        // to a dead session.
        let ended = TermFeedEvent::Ended {
            session: "00000000deadbeef".into(),
        };
        write_frame(&mut a, &ended).await.unwrap();
        let got: TermFeedEvent = read_frame(&mut b).await.unwrap().unwrap();
        assert_eq!(got, ended);

        let cmd = TermCommandEvent {
            session: "00000000deadbeef".into(),
            seq: 7,
            origin: "alice".into(),
            text: "ls -la".into(),
        };
        write_frame(&mut a, &cmd).await.unwrap();
        let got: TermCommandEvent = read_frame(&mut b).await.unwrap().unwrap();
        assert_eq!(got, cmd);
    }

    #[tokio::test]
    async fn control_and_input_frames_round_trip() {
        let (mut a, mut b) = tokio::io::duplex(64 * 1024);
        let req = SessionControlRequest::Create {
            provider: "claude".into(),
            cred: "jess-fable-1".into(),
            cpu: Some(1),
            mem_gb: Some(2),
        };
        write_frame(&mut a, &req).await.unwrap();
        let got: SessionControlRequest = read_frame(&mut b).await.unwrap().unwrap();
        assert_eq!(got, req);

        let reply = SessionControlReply::Refused {
            reason: "credential_not_granted".into(),
            detail: "stranger".into(),
        };
        write_frame(&mut a, &reply).await.unwrap();
        let got: SessionControlReply = read_frame(&mut b).await.unwrap().unwrap();
        assert_eq!(got, reply);

        let ev = SessionInputEvent::Resize {
            session: "00000000deadbeef".into(),
            cols: 120,
            rows: 40,
        };
        write_frame(&mut a, &ev).await.unwrap();
        let got: SessionInputEvent = read_frame(&mut b).await.unwrap().unwrap();
        assert_eq!(got, ev);
    }

    fn rec(
        name: &str,
        owner: u64,
        grants: &[u64],
        kind: gateway::CredentialKind,
    ) -> gateway::CredentialRecord {
        gateway::CredentialRecord {
            name: name.into(),
            owner_account: owner,
            publisher_node: vec![9u8; 32],
            kind,
            seal_pk: [1u8; 32],
            grants: grants.iter().copied().collect(),
        }
    }

    /// What the HOST decides, and the boundary of it. Every case here is a fact
    /// about this host or about the record; none is a fact about who is asking.
    ///
    /// The grant is deliberately absent, including for an account the record does
    /// NOT name: a record granting somebody else still admits here, because the
    /// account this host would have checked is not the account the lender checks.
    /// The lender authorizes the account its own node stamps on the gateway hop —
    /// this host's — and it refuses at `/session`, before the sandbox spawns.
    #[test]
    fn admit_gates_on_sandbox_credential_and_kind_but_never_on_who_is_asking() {
        let claude = rec("c1", 1, &[2], gateway::CredentialKind::Claude);

        // no sandbox → refused before any credential decision.
        assert_eq!(
            admit_create("claude", Some(&claude), None, None, false)
                .unwrap_err()
                .0,
            "no_sandbox"
        );
        // unknown credential.
        assert_eq!(
            admit_create("claude", None, None, None, true)
                .unwrap_err()
                .0,
            "unknown_credential"
        );
        // a record this host is on nobody's grant list for still admits: routing
        // is not authorization, and the lender has not been asked yet.
        assert!(admit_create("claude", Some(&claude), None, None, true).is_ok());
        // an explicit provider contradicting the cred's kind is refused.
        assert_eq!(
            admit_create("codex", Some(&claude), None, None, true)
                .unwrap_err()
                .0,
            "provider_kind_mismatch"
        );
        // an unknown provider tag is not a contradiction (the manager resolves it).
        assert!(admit_create("echo", Some(&claude), Some(1), Some(2), true).is_ok());
    }

    /// The size of the container is a number a REMOTE creator picks, and since
    /// the credential gate moved to the lender any admitted member can reach it.
    /// The ceiling is what stops "give me 1024 cores" from being a sentence a
    /// stranger can say to this host.
    #[test]
    fn a_remote_creator_cannot_ask_this_host_for_any_size_it_likes() {
        let claude = rec("c1", 1, &[], gateway::CredentialKind::Claude);

        // at the ceiling is fine; a step past it is refused, per knob.
        assert!(
            admit_create(
                "claude",
                Some(&claude),
                Some(MAX_SESSION_CORES),
                Some(MAX_SESSION_MEM_GB),
                true
            )
            .is_ok()
        );
        assert_eq!(
            admit_create(
                "claude",
                Some(&claude),
                Some(MAX_SESSION_CORES + 1),
                None,
                true
            )
            .unwrap_err()
            .0,
            "limits_exceed_host_ceiling"
        );
        assert_eq!(
            admit_create(
                "claude",
                Some(&claude),
                None,
                Some(MAX_SESSION_MEM_GB + 1),
                true
            )
            .unwrap_err()
            .0,
            "limits_exceed_host_ceiling"
        );
        // and an unset knob is not a request for infinity.
        assert!(admit_create("claude", Some(&claude), None, None, true).is_ok());
    }

    #[test]
    fn admit_maps_limits_and_kind() {
        let codex = rec("x", 1, &[], gateway::CredentialKind::Codex);
        let ok = admit_create("codex", Some(&codex), Some(3), Some(8), true).unwrap();
        assert_eq!(ok.limits.get("cores"), Some(&3));
        assert_eq!(ok.limits.get("mem_gb"), Some(&8));
        assert!(matches!(ok.kind, provider_host::CredentialKind::Codex));
    }

    #[test]
    fn input_frame_is_accepted_only_from_the_creator_node() {
        assert!(input_permitted(Some([7u8; 32]), PeerId([7u8; 32])));
        assert!(!input_permitted(Some([7u8; 32]), PeerId([9u8; 32])));
        // not an attached session (local, or unknown id) → refused.
        assert!(!input_permitted(None, PeerId([7u8; 32])));
    }

    /// **The work-admission call site, behaviourally.**
    ///
    /// A mesh peer is a node, never an account, so under the default policy it
    /// is refused at the door — before the credential record is read, before
    /// any host capability is disclosed, and with ZERO identity reads (the
    /// command channel has no reader). This is the hole #833 left open:
    /// without it, any admitted member naming any registered credential gets a
    /// container here.
    #[tokio::test]
    async fn a_peer_this_node_does_not_admit_is_refused_before_any_credential_read() {
        let workspace =
            std::env::temp_dir().join(format!("ducktape-term-admit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&workspace);
        std::fs::create_dir_all(&workspace).expect("scratch workspace");

        let (commands, _no_reads) = fmpsc::channel(4);
        let control = Arc::new(ControlState {
            // a live manager would still never be reached: the refusal is
            // upstream of every host-capability question.
            sessions: None,
            commands,
            local_gateway_via: String::new(),
            me: [1u8; 32],
            workspace: workspace.clone(),
        });

        let (server, mut caller) = tokio::io::duplex(64 * 1024);
        let serving = Arc::clone(&control);
        tokio::spawn(async move {
            let _ = serve_control(server, PeerId([9u8; 32]), serving).await;
        });
        write_frame(
            &mut caller,
            &SessionControlRequest::Create {
                provider: "claude".into(),
                cred: "c".into(),
                cpu: None,
                mem_gb: None,
            },
        )
        .await
        .unwrap();
        let reply: SessionControlReply = read_frame(&mut caller).await.unwrap().unwrap();
        let SessionControlReply::Refused { reason, detail } = reply else {
            panic!("an unadmitted peer must be refused");
        };
        assert_eq!(reason, "work_not_admitted");
        assert!(
            !detail.contains("account") || detail.contains("<account>"),
            "a refusal never echoes the account that would have been accepted: {detail:?}"
        );

        // and the SAME peer is served once the operator admits anyone — the
        // only policy a peer NODE can be admitted by, re-read per create, so no
        // restart is involved.
        crate::work_admission::admit_anyone_fixture(&workspace).expect("save policy");
        let (server, mut caller) = tokio::io::duplex(64 * 1024);
        tokio::spawn(async move {
            let _ = serve_control(server, PeerId([9u8; 32]), control).await;
        });
        write_frame(
            &mut caller,
            &SessionControlRequest::Create {
                provider: "claude".into(),
                cred: "c".into(),
                cpu: None,
                mem_gb: None,
            },
        )
        .await
        .unwrap();
        let reply: SessionControlReply = read_frame(&mut caller).await.unwrap().unwrap();
        assert_eq!(
            reply,
            SessionControlReply::Refused {
                reason: "no_sandbox".into(),
                detail: "this node hosts no terminal sessions".into(),
            },
            "an admitted peer passes the door and is refused only on this host's own capability"
        );
        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[tokio::test]
    async fn serve_control_refuses_create_without_a_sandbox_and_acks_close() {
        // no session manager (a sync-only / Direct node) → a create is refused
        // `no_sandbox` and a close is still a clean idempotent ack, both over the
        // real CONTROL frame codec. Exercises serve_control without a live pty.
        let (commands, _rx) = fmpsc::channel(1);
        let control = Arc::new(ControlState {
            sessions: None,
            commands,
            local_gateway_via: String::new(),
            // the peer IS this node (the own-node loopback), so the work
            // admission takes its zero-query `ThisNode` path and this test stays
            // about `no_sandbox`. The refusing case is the test below.
            me: [7u8; 32],
            workspace: std::path::PathBuf::from("/nonexistent-work-admission-workspace"),
        });

        let (server, mut caller) = tokio::io::duplex(64 * 1024);
        let control_create = Arc::clone(&control);
        tokio::spawn(async move {
            let _ = serve_control(server, PeerId([7u8; 32]), control_create).await;
        });
        write_frame(
            &mut caller,
            &SessionControlRequest::Create {
                provider: "claude".into(),
                cred: "c".into(),
                cpu: None,
                mem_gb: None,
            },
        )
        .await
        .unwrap();
        let reply: SessionControlReply = read_frame(&mut caller).await.unwrap().unwrap();
        assert_eq!(
            reply,
            SessionControlReply::Refused {
                reason: "no_sandbox".into(),
                detail: "this node hosts no terminal sessions".into(),
            }
        );

        let (server, mut caller) = tokio::io::duplex(64 * 1024);
        tokio::spawn(async move {
            let _ = serve_control(server, PeerId([7u8; 32]), control).await;
        });
        write_frame(
            &mut caller,
            &SessionControlRequest::Close {
                session: "00000000deadbeef".into(),
            },
        )
        .await
        .unwrap();
        let reply: SessionControlReply = read_frame(&mut caller).await.unwrap().unwrap();
        assert_eq!(reply, SessionControlReply::Closed);
    }

    #[test]
    fn intents_do_not_collide() {
        let intents = [CHUNK_INTENT, COMMAND_INTENT, CONTROL_INTENT, INPUT_INTENT];
        for i in intents {
            assert_eq!(
                intents.iter().filter(|x| **x == i).count(),
                1,
                "intent {i} is unique"
            );
        }
    }

    #[tokio::test]
    async fn read_frame_returns_none_on_clean_eof() {
        let (a, mut b) = tokio::io::duplex(16);
        drop(a);
        let got: Option<TermFeedEvent> = read_frame(&mut b).await.unwrap();
        assert!(got.is_none(), "a clean EOF is end-of-stream, not an error");
    }

    #[tokio::test]
    async fn read_frame_rejects_an_oversized_length_prefix() {
        let (mut a, mut b) = tokio::io::duplex(16);
        a.write_all(&(u32::MAX).to_be_bytes()).await.unwrap();
        let err = read_frame::<_, TermFeedEvent>(&mut b).await.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }
}
