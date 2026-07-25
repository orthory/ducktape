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
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use data_plane::{
    AddressBook, AdmissionPolicy, BulkPacer, DataPlane, DataPlaneTransport, FlowId, PeerId,
    Service, SocketFactory, StreamPacing, StreamPlaneSpec, StreamPolicy, StreamService,
    bind_stream_plane,
};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use provider_host::ResolvedCredential;
use futures::SinkExt as _;
use futures::channel::{mpsc as fmpsc, oneshot};
use noded::{
    CreatedSession, NodeCommand, PeerAttach, SessionInputWire, SessionJob, TermChunkEvent,
    TermCommandEvent, TermCommandRing, TermError, TerminalSessions, TermRing,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};

use crate::voice_plane::MediaPeers;

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

/// guest→host directed create/close request, one request → one reply. Carries
/// NO creator field — the host derives the creator from the mesh-authenticated
/// requesting peer node (identity `OfNode`), never from client-supplied bytes.
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
/// snake_case tokens: `no_sandbox`, `unknown_credential`, `credential_not_granted`,
/// `provider_kind_mismatch`, `at_capacity`, `unknown_provider`.
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
    Input { session: String, data_b64: String },
    Resize { session: String, cols: u16, rows: u16 },
}
const RETRY: Duration = Duration::from_secs(3);

fn term_flow() -> FlowId {
    FlowId::derive(b"ducktape:term-session:v1")
}

/// a session id is 16 lowercase hex (see `term::spawn`'s `format!("{:016x}",
/// …)`). The agent plane's twin guard checks a 64-hex run id; ours accepts the
/// shorter session id and rejects anything else, so a malformed grain never
/// reaches a ring.
fn valid_session(session: &str) -> bool {
    session.len() == 16 && session.bytes().all(|byte| byte.is_ascii_hexdigit())
}

struct TermBook {
    peers: Arc<MediaPeers>,
}

impl AddressBook for TermBook {
    fn datagram_addr(&self, peer: PeerId) -> Option<SocketAddr> {
        Some(SocketAddr::new(
            self.peers.overlay_ip(&peer.0),
            Service::TermSession.overlay_datagram_port(),
        ))
    }

    fn stream_addr(&self, peer: PeerId) -> Option<SocketAddr> {
        Some(SocketAddr::new(
            self.peers.overlay_ip(&peer.0),
            Service::TermSession.overlay_stream_port(),
        ))
    }

    fn peer_at(&self, src: std::net::IpAddr) -> Option<PeerId> {
        self.peers.peer_at(src)
    }
}

impl AdmissionPolicy for TermBook {
    fn permits(&self, peer: PeerId, service: Service, flow: FlowId) -> bool {
        service == Service::TermSession && flow == term_flow() && self.peers.contains(peer)
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
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn(
    label: String,
    factory: Arc<dyn SocketFactory>,
    peers: Arc<MediaPeers>,
    me: [u8; 32],
    pacer: BulkPacer,
    planes: data_plane::PlaneMonitor,
    terminals: TermRing,
    term_commands: TermCommandRing,
    sessions: Option<TerminalSessions>,
    commands: fmpsc::Sender<NodeCommand>,
    local_gateway_via: String,
    jobs: tokio::sync::mpsc::Receiver<SessionJob>,
) {
    tokio::spawn(async move {
        let own = peers.own_ip(&me);
        let spec = StreamPlaneSpec {
            own_ip: own,
            service: Service::TermSession,
            pacing: StreamPacing::Shared(pacer),
            policy: StreamPolicy { accept_backlog: 64 },
            retry: RETRY,
        };
        let book = Arc::new(TermBook {
            peers: Arc::clone(&peers),
        });
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
        )
        .await;
    });
}

#[allow(clippy::too_many_arguments)]
async fn run_bound<T: DataPlaneTransport>(
    plane: DataPlane<T>,
    service: Arc<StreamService<T>>,
    peers: Arc<MediaPeers>,
    me: PeerId,
    terminals: TermRing,
    term_commands: TermCommandRing,
    control: Arc<ControlState>,
    jobs: tokio::sync::mpsc::Receiver<SessionJob>,
) {
    let _plane = plane;
    tokio::select! {
        _ = accept_loop(
            Arc::clone(&service),
            Arc::clone(&peers),
            terminals.clone(),
            term_commands.clone(),
            Arc::clone(&control),
        ) => {}
        _ = fanout_loop(Arc::clone(&service), Arc::clone(&peers), me, terminals, term_commands) => {}
        _ = client_loop(service, control, jobs) => {}
    }
}

async fn accept_loop<T: DataPlaneTransport>(
    service: Arc<StreamService<T>>,
    peers: Arc<MediaPeers>,
    terminals: TermRing,
    term_commands: TermCommandRing,
    control: Arc<ControlState>,
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
        tokio::spawn(async move {
            let _ = match intent {
                CHUNK_INTENT => receive_chunks(stream, peer, peers, terminals).await,
                COMMAND_INTENT => receive_commands(stream, peer, peers, term_commands).await,
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

async fn receive_chunks<S: AsyncRead + Unpin>(
    mut stream: S,
    peer: PeerId,
    peers: Arc<MediaPeers>,
    ring: TermRing,
) -> io::Result<()> {
    while peers.contains(peer) {
        let Some(event) = read_frame::<_, TermChunkEvent>(&mut stream).await? else {
            return Ok(());
        };
        if !peers.contains(peer) {
            return Ok(());
        }
        // a peer sending a malformed session id is a bug, not consensus — skip
        // the grain and keep the stream (best-effort, observational).
        if valid_session(&event.session) {
            ring.append_remote(event);
        }
    }
    Ok(())
}

async fn receive_commands<S: AsyncRead + Unpin>(
    mut stream: S,
    peer: PeerId,
    peers: Arc<MediaPeers>,
    ring: TermCommandRing,
) -> io::Result<()> {
    while peers.contains(peer) {
        let Some(event) = read_frame::<_, TermCommandEvent>(&mut stream).await? else {
            return Ok(());
        };
        if !peers.contains(peer) {
            return Ok(());
        }
        if valid_session(&event.session) {
            // append_remote replays the origin's seq verbatim — the peer shows
            // the same total order and never re-stamps it.
            ring.append_remote(event);
        }
    }
    Ok(())
}

async fn fanout_loop<T: DataPlaneTransport>(
    service: Arc<StreamService<T>>,
    peers: Arc<MediaPeers>,
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
        tokio::time::sleep(RETRY).await;
    }
}

/// one task per peer: hold two persistent outbound streams (chunk + command)
/// and pump each feed onto its stream. Subscribes both feeds ONCE, outside the
/// reconnect loop, so a re-open never replays an already-consumed grain.
async fn send_peer<T: DataPlaneTransport>(
    service: Arc<StreamService<T>>,
    peers: Arc<MediaPeers>,
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
                tokio::time::sleep(RETRY).await;
                continue;
            }
        };
        let mut cmd_stream = match service
            .open(peer, term_flow(), COMMAND_INTENT, Vec::new())
            .await
        {
            Ok(stream) => stream,
            Err(_) => {
                tokio::time::sleep(RETRY).await;
                continue;
            }
        };
        loop {
            tokio::select! {
                event = chunks.recv() => match event {
                    Ok(event) => {
                        if valid_session(&event.session)
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
                        if valid_session(&event.session)
                            && write_frame(&mut cmd_stream, &event).await.is_err()
                        {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
                },
                _ = tokio::time::sleep(RETRY) => {
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

/// the host's create path: derive the creator account from the mesh-authenticated
/// peer, resolve the named credential from committed gateway state, decide
/// admission, then spawn a peer-attached session drawing on the guest's
/// self-host gateway. Every refusal carries a stable snake_case `reason`.
async fn serve_create(
    control: &ControlState,
    peer: PeerId,
    provider: &str,
    cred: &str,
    cpu: Option<u64>,
    mem_gb: Option<u64>,
) -> SessionControlReply {
    let Some(sessions) = control.sessions.clone() else {
        tracing::warn!(target: "ducktape::term", reason = "no_sandbox", "peer session create refused");
        return refused("no_sandbox", "this node hosts no terminal sessions");
    };
    let sandbox_present = sessions.has_sandbox();
    // the creator is the mesh-authenticated requesting node's account — never a
    // client-supplied field. When the creator runs on their own node (the lending
    // case) this is cryptographic.
    let creator_account = match account_of_node(&control.commands, &peer.0).await {
        Ok(account) => account,
        Err(detail) => {
            tracing::warn!(target: "ducktape::term", reason = "credential_not_granted", "peer session create refused");
            return refused("credential_not_granted", &detail);
        }
    };
    let record = match credential_record(&control.commands, cred).await {
        Ok(record) => record,
        Err(detail) => return refused("unknown_credential", &detail),
    };
    let admit = match admit_create(
        provider,
        &creator_account,
        record.as_ref(),
        cpu,
        mem_gb,
        sandbox_present,
    ) {
        Ok(admit) => admit,
        Err((reason, detail)) => {
            tracing::warn!(target: "ducktape::term", reason, "peer session create refused");
            return refused(reason, &detail);
        }
    };
    let authority = match owner_airlock_authority(&control.commands, &admit.owner_account).await {
        Ok(authority) => authority,
        Err(detail) => return refused("unknown_credential", &detail),
    };
    // bin/node owns the record → ResolvedCredential mapping (capability-host must
    // not depend on the gateway crate): the seal_pk is the on-chain anchor, the
    // via is the host's own browser-gateway, the authority is the owner's airlock
    // route.
    let resolved = ResolvedCredential {
        name: admit.name,
        kind: admit.kind,
        authority,
        via: control.local_gateway_via.clone(),
        seal_pk: admit.seal_pk,
        // the creator (a mesh peer, or the owner itself) is who draws on the
        // credential; the owner's gateway checks THIS account against the grant.
        account: creator_account,
    };
    // the record travels to the agent daemon, which pins it as its broker's
    // self-host airlock upstream. Nothing secret crosses: a name, an authority
    // handle, this node's own gateway `via`, a PUBLIC seal key and the account
    // the owner's gateway checks the grant against.
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
            "this node has no configured sandbox image".to_string(),
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
    owner_account: Vec<u8>,
    limits: std::collections::BTreeMap<String, u64>,
}

fn admit_create(
    provider: &str,
    creator_account: &[u8],
    record: Option<&gateway::CredentialRecord>,
    cpu: Option<u64>,
    mem_gb: Option<u64>,
    sandbox_present: bool,
) -> Result<AdmitOk, (&'static str, String)> {
    if !sandbox_present {
        return Err((
            "no_sandbox",
            "this node has no configured sandbox image".into(),
        ));
    }
    let Some(record) = record else {
        return Err((
            "unknown_credential",
            "no credential by that name is registered".into(),
        ));
    };
    let is_allowed = gateway::credential_use_allowed(record, creator_account);
    if !is_allowed {
        return Err((
            "credential_not_granted",
            "creator is neither the owner nor a grantee".into(),
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
        kind: map_kind(record.kind),
        seal_pk: record.seal_pk,
        owner_account: record.owner_account.clone(),
        limits: build_limits(cpu, mem_gb),
    })
}

/// gateway kind → capability-host kind (the two are deliberately separate types;
/// capability-host must not depend on the gateway crate).
fn map_kind(kind: gateway::CredentialKind) -> provider_host::CredentialKind {
    match kind {
        gateway::CredentialKind::Claude => provider_host::CredentialKind::Claude,
        gateway::CredentialKind::Codex => provider_host::CredentialKind::Codex,
    }
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
    peers: Arc<MediaPeers>,
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

/// gate one input event on the creator, then apply it to the pty (write or
/// resize). Shared by the remote input stream and the loopback client path.
async fn deliver_input(sessions: &TerminalSessions, peer: PeerId, event: SessionInputEvent) {
    let session = input_session(&event).to_string();
    let permitted = input_permitted(sessions.creator_node(&session), peer);
    if !permitted {
        tracing::warn!(target: "ducktape::term", reason = "input_not_creator", "forwarded input dropped");
        return;
    }
    match event {
        SessionInputEvent::Input { data_b64, .. } => {
            // decoded only to refuse a malformed frame at this boundary; the
            // daemon takes the base64 as-is, so the bytes never round-trip.
            if STANDARD.decode(&data_b64).is_err() {
                tracing::warn!(target: "ducktape::term", reason = "bad_base64", "forwarded input dropped");
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
                tracing::warn!(target: "ducktape::term", reason = "input_open_failed", error = %err, "forwarded input dropped");
                return;
            }
        }
    }
    let stream = input_streams
        .get_mut(&host)
        .expect("input stream just inserted");
    if let Err(err) = write_frame(stream.as_mut(), &event).await {
        tracing::warn!(target: "ducktape::term", reason = "input_write_failed", error = %err, "forwarded input dropped; reopening");
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

/// the account bound to the mesh-authenticated requesting node (the creator).
async fn account_of_node(
    commands: &fmpsc::Sender<NodeCommand>,
    node: &[u8; 32],
) -> Result<Vec<u8>, String> {
    let reply = query(
        commands,
        "identity",
        identity::encode_query(&identity::IdentityQuery::OfNode {
            node_key: node.to_vec(),
        }),
    )
    .await?;
    match identity::decode_reply(&reply)? {
        identity::IdentityReply::Account(Some(account))
            if account
                .nodes
                .iter()
                .any(|candidate| candidate.node_key.as_slice() == node) =>
        {
            Ok(account.account_id)
        }
        identity::IdentityReply::Account(_) => {
            Err("creator node has no current identity account".into())
        }
        _ => Err("unexpected identity reply".into()),
    }
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
    owner_account: &[u8],
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
            .find(|registration| registration.account_id.as_slice() == owner_account);
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

    #[test]
    fn valid_session_accepts_16_hex_and_rejects_the_rest() {
        assert!(valid_session("00000000deadbeef"));
        assert!(!valid_session("deadbeef"), "too short");
        assert!(!valid_session("00000000deadbeef0"), "too long");
        assert!(!valid_session("00000000deadbeeg"), "non-hex digit");
        assert!(!valid_session(""), "empty");
    }

    #[tokio::test]
    async fn frame_round_trips_both_event_types() {
        let (mut a, mut b) = tokio::io::duplex(64 * 1024);
        let chunk = TermChunkEvent {
            session: "00000000deadbeef".into(),
            chunk_b64: "aGVsbG8=".into(),
        };
        write_frame(&mut a, &chunk).await.unwrap();
        let got: TermChunkEvent = read_frame(&mut b).await.unwrap().unwrap();
        assert_eq!(got, chunk);

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
        owner: &[u8],
        grants: &[&[u8]],
        kind: gateway::CredentialKind,
    ) -> gateway::CredentialRecord {
        gateway::CredentialRecord {
            name: name.into(),
            owner_account: owner.to_vec(),
            publisher_node: vec![9u8; 32],
            kind,
            seal_pk: [1u8; 32],
            grants: grants.iter().map(|g| g.to_vec()).collect(),
        }
    }

    #[test]
    fn admit_gates_on_sandbox_credential_grant_and_kind() {
        let owner = b"owner-acct".to_vec();
        let grantee = b"grantee-acct".to_vec();
        let stranger = b"stranger".to_vec();
        let claude = rec("c1", &owner, &[&grantee], gateway::CredentialKind::Claude);

        // no sandbox → refused before any credential decision.
        assert_eq!(
            admit_create("claude", &owner, Some(&claude), None, None, false)
                .unwrap_err()
                .0,
            "no_sandbox"
        );
        // unknown credential.
        assert_eq!(
            admit_create("claude", &owner, None, None, None, true)
                .unwrap_err()
                .0,
            "unknown_credential"
        );
        // owner is allowed; grantee is allowed; stranger is refused.
        assert!(admit_create("claude", &owner, Some(&claude), None, None, true).is_ok());
        assert!(admit_create("claude", &grantee, Some(&claude), None, None, true).is_ok());
        assert_eq!(
            admit_create("claude", &stranger, Some(&claude), None, None, true)
                .unwrap_err()
                .0,
            "credential_not_granted"
        );
        // an explicit provider contradicting the cred's kind is refused.
        assert_eq!(
            admit_create("codex", &owner, Some(&claude), None, None, true)
                .unwrap_err()
                .0,
            "provider_kind_mismatch"
        );
        // an unknown provider tag is not a contradiction (the manager resolves it).
        assert!(admit_create("echo", &owner, Some(&claude), Some(1), Some(2), true).is_ok());
    }

    #[test]
    fn admit_maps_limits_and_kind() {
        let owner = b"owner".to_vec();
        let codex = rec("x", &owner, &[], gateway::CredentialKind::Codex);
        let ok = admit_create("codex", &owner, Some(&codex), Some(3), Some(8), true).unwrap();
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
            me: [0u8; 32],
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
        let got: Option<TermChunkEvent> = read_frame(&mut b).await.unwrap();
        assert!(got.is_none(), "a clean EOF is end-of-stream, not an error");
    }

    #[tokio::test]
    async fn read_frame_rejects_an_oversized_length_prefix() {
        let (mut a, mut b) = tokio::io::duplex(16);
        a.write_all(&(u32::MAX).to_be_bytes()).await.unwrap();
        let err = read_frame::<_, TermChunkEvent>(&mut b).await.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }
}
