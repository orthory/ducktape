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
use noded::{TermChunkEvent, TermCommandEvent, TermCommandRing, TermRing};
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};

use crate::voice_plane::MediaPeers;

/// the raw-output feed rides intent 1, the ordered command log intent 2.
const CHUNK_INTENT: u8 = 1;
const COMMAND_INTENT: u8 = 2;
const MAX_EVENT_BYTES: usize = 64 * 1024;
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
        run_bound(plane, service, peers, PeerId(me), terminals, term_commands).await;
    });
}

async fn run_bound<T: DataPlaneTransport>(
    plane: DataPlane<T>,
    service: Arc<StreamService<T>>,
    peers: Arc<MediaPeers>,
    me: PeerId,
    terminals: TermRing,
    term_commands: TermCommandRing,
) {
    let _plane = plane;
    tokio::select! {
        _ = accept_loop(
            Arc::clone(&service),
            Arc::clone(&peers),
            terminals.clone(),
            term_commands.clone(),
        ) => {}
        _ = fanout_loop(service, peers, me, terminals, term_commands) => {}
    }
}

async fn accept_loop<T: DataPlaneTransport>(
    service: Arc<StreamService<T>>,
    peers: Arc<MediaPeers>,
    terminals: TermRing,
    term_commands: TermCommandRing,
) {
    // one live inbound stream per (peer, feed): a peer opens two streams (chunk
    // + command), so the dedupe key carries the intent, not just the peer.
    let active = Arc::new(std::sync::Mutex::new(HashSet::new()));
    while let Some((peer, hello, stream)) = service.accept().await {
        let intent = hello.intent;
        if !hello.meta.is_empty() || !matches!(intent, CHUNK_INTENT | COMMAND_INTENT) {
            continue;
        }
        let key = (peer, intent);
        if !active.lock().expect("term streams lock").insert(key) {
            continue;
        }
        let peers = Arc::clone(&peers);
        let terminals = terminals.clone();
        let term_commands = term_commands.clone();
        let active = Arc::clone(&active);
        tokio::spawn(async move {
            let _ = match intent {
                CHUNK_INTENT => receive_chunks(stream, peer, peers, terminals).await,
                _ => receive_commands(stream, peer, peers, term_commands).await,
            };
            active.lock().expect("term streams lock").remove(&key);
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
async fn write_frame<S: AsyncWrite + Unpin, E: Serialize>(
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
