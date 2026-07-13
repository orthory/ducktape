//! Module-code distribution over the WireGuard data plane.
//!
//! Consensus commits WHICH code a module runs as a 32-byte hash (modreg);
//! this plane moves the content-addressed BYTES — wasm components today,
//! quack capsules tomorrow. Two stream intents, both self-verifying (the
//! receiver publishes a blob only when the assembled whole re-hashes to the
//! digest, so no trust ever attaches to which peer the bytes came from):
//!
//! - PUSH: the staging custodian fans a new artifact out to every member
//!   BEFORE the governance proposal referencing its hash is submitted. The
//!   receiver acks with its resume offset (transfers survive drops), streams
//!   the tail into a disk-staged slot, and answers one result frame.
//! - PULL: a node missing a committed artifact asks a peer to stream it —
//!   the data-plane twin of the mesh's ranged blob lane.
//!
//! Admission is default-deny per the plane's contract: members only, one
//! live transfer per digest, per-kind size caps, and a process-wide staging
//! byte budget — a rogue member can waste bounded disk, never poison a blob.

use std::collections::HashSet;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use data_plane::{
    AddressBook, AdmissionPolicy, BulkPacer, DataPlaneTransport, FlowId, PeerId, Service,
    SocketFactory, StreamPacing, StreamPlaneSpec, StreamPolicy, StreamService, bind_stream_plane,
};
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};

use crate::constants::MAX_MODULE_CODE_BYTES;
use crate::voice_plane::MediaPeers;

const INTENT_PUSH: u8 = 1;
const INTENT_PULL: u8 = 2;

/// the artifact kinds this plane admits, with their size caps. wire-stable
/// (the admin RPC names the same value as `noded::CODE_KIND_MODULE`).
pub(crate) const KIND_MODULE_CODE: u8 = noded::CODE_KIND_MODULE;

/// stream copy window: bounded buffers on both ends, whatever the blob size.
const WINDOW: usize = 256 * 1024;

/// total bytes of in-flight push staging this node accepts at once. bounds a
/// rogue member's disk waste; completed/failed transfers return their budget.
const STAGING_BUDGET: u64 = 2 * 1024 * 1024 * 1024;

/// one fan-out send: generous, because a capsule may be large and the bulk
/// pacer deliberately throttles below the link — a stalled stream fails on
/// its own; this only reaps a peer that accepts and then goes silent.
const PUSH_TIMEOUT: Duration = Duration::from_secs(600);

const RETRY: Duration = Duration::from_secs(3);

fn code_flow() -> FlowId {
    FlowId::derive(b"ducktape:module-code:v1")
}

fn kind_cap(kind: u8) -> Option<u64> {
    (kind == KIND_MODULE_CODE).then_some(MAX_MODULE_CODE_BYTES)
}

// ---- ack / result wire frames (BE, fixed width) -------------------------------

const ACK_SEND_FROM: u8 = 0;
const ACK_ALREADY_HAVE: u8 = 1;
const ACK_REFUSED: u8 = 2;

const RESULT_OK: u8 = 0;
const RESULT_CORRUPT: u8 = 1;
const RESULT_STAGE_FAILED: u8 = 2;

const PULL_SERVING: u8 = 0;
const PULL_MISS: u8 = 1;

struct CodeBook {
    peers: Arc<MediaPeers>,
}

impl AddressBook for CodeBook {
    fn datagram_addr(&self, peer: PeerId) -> Option<SocketAddr> {
        Some(SocketAddr::new(
            self.peers.overlay_ip(&peer.0),
            Service::ModuleCode.overlay_datagram_port(),
        ))
    }

    fn stream_addr(&self, peer: PeerId) -> Option<SocketAddr> {
        Some(SocketAddr::new(
            self.peers.overlay_ip(&peer.0),
            Service::ModuleCode.overlay_stream_port(),
        ))
    }

    fn peer_at(&self, src: std::net::IpAddr) -> Option<PeerId> {
        self.peers.peer_at(src)
    }
}

impl AdmissionPolicy for CodeBook {
    fn permits(&self, peer: PeerId, service: Service, flow: FlowId) -> bool {
        service == Service::ModuleCode && flow == code_flow() && self.peers.contains(peer)
    }
}

/// bind the service in the background, draining the admin RPC's stage
/// requests (the daemon surface owns the sender — see
/// `noded::NodeHandle::with_code_stage`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn(
    label: String,
    factory: Arc<dyn SocketFactory>,
    peers: Arc<MediaPeers>,
    me: [u8; 32],
    pacer: BulkPacer,
    planes: data_plane::PlaneMonitor,
    blobs: blobstore::BlobHandle,
    stage_rx: tokio::sync::mpsc::Receiver<noded::CodeStageRequest>,
) {
    tokio::spawn(async move {
        let own = peers.own_ip(&me);
        let spec = StreamPlaneSpec {
            own_ip: own,
            service: Service::ModuleCode,
            pacing: StreamPacing::Shared(pacer),
            policy: StreamPolicy { accept_backlog: 16 },
            retry: RETRY,
        };
        let book = Arc::new(CodeBook {
            peers: Arc::clone(&peers),
        });
        let (plane, service) = match bind_stream_plane(spec, factory, book).await {
            Ok(bound) => bound,
            Err(error) => {
                eprintln!("[node {label}] module-code plane register failed: {error}");
                return;
            }
        };
        println!("[node {label}] module-code plane: overlay stream bound on {own}");
        planes.register("module-code", Service::ModuleCode, plane.watch());
        let _plane = plane;
        tokio::select! {
            _ = accept_loop(Arc::clone(&service), blobs.clone()) => {}
            _ = stage_loop(service, peers, PeerId(me), blobs, stage_rx) => {}
        }
    });
}

// ---- inbound: pushes staged into the store, pulls served from it --------------

async fn accept_loop<T: DataPlaneTransport>(
    service: Arc<StreamService<T>>,
    blobs: blobstore::BlobHandle,
) {
    // one live transfer per digest (the staging slot is single-writer), plus
    // the process-wide staging byte budget.
    let inflight: Arc<std::sync::Mutex<HashSet<[u8; 32]>>> = Default::default();
    let budget = Arc::new(AtomicU64::new(0));
    while let Some((_peer, hello, stream)) = service.accept().await {
        match hello.intent {
            INTENT_PUSH => {
                let Some((kind, digest, len)) = decode_push_meta(&hello.meta) else {
                    continue;
                };
                let blobs = blobs.clone();
                let inflight = Arc::clone(&inflight);
                let budget = Arc::clone(&budget);
                tokio::spawn(async move {
                    let _ = receive_push(stream, kind, digest, len, blobs, inflight, budget).await;
                });
            }
            INTENT_PULL => {
                let Some((digest, offset)) = decode_pull_meta(&hello.meta) else {
                    continue;
                };
                let blobs = blobs.clone();
                tokio::spawn(async move {
                    let _ = serve_pull(stream, digest, offset, blobs).await;
                });
            }
            _ => {}
        }
    }
}

/// the receive half of one push: admission-check, ack with the resume
/// offset, stream the tail into a disk-staged slot, verify-then-publish,
/// answer one result frame. a transport drop mid-stream KEEPS the staging —
/// the custodian's retry resumes at the high-water.
async fn receive_push<S: AsyncRead + AsyncWrite + Unpin>(
    mut stream: S,
    kind: u8,
    digest: [u8; 32],
    len: u64,
    blobs: blobstore::BlobHandle,
    inflight: Arc<std::sync::Mutex<HashSet<[u8; 32]>>>,
    budget: Arc<AtomicU64>,
) -> io::Result<()> {
    let refuse = |mut stream: S| async move {
        stream.write_all(&[ACK_REFUSED]).await?;
        stream.write_all(&0u64.to_be_bytes()).await
    };
    let Some(cap) = kind_cap(kind) else {
        return refuse(stream).await;
    };
    if len > cap {
        return refuse(stream).await;
    }
    if blobs.has_chunk(&digest) {
        stream.write_all(&[ACK_ALREADY_HAVE]).await?;
        return stream.write_all(&0u64.to_be_bytes()).await;
    }
    if !inflight.lock().expect("inflight lock").insert(digest) {
        return refuse(stream).await;
    }
    // hold the slot for the whole transfer; released on every exit below.
    let release = |budget_taken: u64| {
        inflight.lock().expect("inflight lock").remove(&digest);
        budget.fetch_sub(budget_taken, Ordering::Relaxed);
    };
    if budget.fetch_add(len, Ordering::Relaxed) + len > STAGING_BUDGET {
        release(len);
        return refuse(stream).await;
    }
    let mut slot = match blobs.stage(digest, len) {
        Ok(slot) => slot,
        Err(_) => {
            release(len);
            return refuse(stream).await;
        }
    };
    stream.write_all(&[ACK_SEND_FROM]).await?;
    stream.write_all(&slot.offset().to_be_bytes()).await?;

    let mut buf = vec![0u8; WINDOW];
    while slot.offset() < len {
        let want = buf.len().min((len - slot.offset()) as usize);
        let n = match stream.read(&mut buf[..want]).await {
            Ok(0) | Err(_) => {
                // dropped mid-transfer: staging stays for a resume.
                release(len);
                return Ok(());
            }
            Ok(n) => n,
        };
        if slot.append(&buf[..n]).is_err() {
            release(len);
            return stream.write_all(&[RESULT_STAGE_FAILED]).await;
        }
    }
    let result = match slot.finish() {
        Ok(_) => RESULT_OK,
        Err(blobstore::StageError::HashMismatch) => RESULT_CORRUPT,
        Err(_) => RESULT_STAGE_FAILED,
    };
    release(len);
    stream.write_all(&[result]).await
}

/// serve one pull: a status+len header, then the raw bytes from `offset`.
async fn serve_pull<S: AsyncRead + AsyncWrite + Unpin>(
    mut stream: S,
    digest: [u8; 32],
    offset: u64,
    blobs: blobstore::BlobHandle,
) -> io::Result<()> {
    let Some(len) = blobs.chunk_len(&digest) else {
        stream.write_all(&[PULL_MISS]).await?;
        return stream.write_all(&0u64.to_be_bytes()).await;
    };
    if offset > len {
        stream.write_all(&[PULL_MISS]).await?;
        return stream.write_all(&0u64.to_be_bytes()).await;
    }
    stream.write_all(&[PULL_SERVING]).await?;
    stream.write_all(&len.to_be_bytes()).await?;
    let mut at = offset;
    while at < len {
        let Some(window) = blobs.read_range(&digest, at, WINDOW) else {
            // the blob vanished under us (impossible without manual disk
            // surgery); the abrupt close reads as a transport error upstream.
            return Ok(());
        };
        stream.write_all(&window).await?;
        at += window.len() as u64;
    }
    Ok(())
}

// ---- outbound: the custodian's fan-out + the pull client ----------------------

/// drain stage requests: each fans the digest out to every member
/// concurrently and reports per-peer receipts.
async fn stage_loop<T: DataPlaneTransport>(
    service: Arc<StreamService<T>>,
    peers: Arc<MediaPeers>,
    me: PeerId,
    blobs: blobstore::BlobHandle,
    mut requests: tokio::sync::mpsc::Receiver<noded::CodeStageRequest>,
) {
    while let Some(req) = requests.recv().await {
        let targets: Vec<PeerId> = peers.peer_ids().into_iter().filter(|p| *p != me).collect();
        let sends = targets.into_iter().map(|peer| {
            let service = Arc::clone(&service);
            let blobs = blobs.clone();
            async move {
                let status =
                    match tokio::time::timeout(PUSH_TIMEOUT, push_peer(service, peer, req.kind, req.digest, blobs))
                        .await
                    {
                        Ok(Ok(status)) => return receipt(peer, status, true),
                        Ok(Err(reason)) => reason,
                        Err(_) => "push timed out".into(),
                    };
                receipt(peer, status, false)
            }
        });
        let receipts = futures::future::join_all(sends).await;
        let _ = req.reply.send(receipts);
    }
}

fn receipt(peer: PeerId, status: String, ok: bool) -> noded::CodePeerReceipt {
    noded::CodePeerReceipt {
        peer: crate::config::hex_bytes(&peer.0),
        status,
        ok,
    }
}

/// push one locally-resident artifact to one peer; `Ok` carries the receipt
/// wording ("stored" / "already-have").
async fn push_peer<T: DataPlaneTransport>(
    service: Arc<StreamService<T>>,
    peer: PeerId,
    kind: u8,
    digest: [u8; 32],
    blobs: blobstore::BlobHandle,
) -> Result<String, String> {
    let len = blobs
        .chunk_len(&digest)
        .ok_or_else(|| "artifact not resident locally".to_string())?;
    let mut stream = service
        .open(peer, code_flow(), INTENT_PUSH, encode_push_meta(kind, &digest, len))
        .await
        .map_err(|e| format!("open failed: {e}"))?;
    let mut ack = [0u8; 9];
    stream
        .read_exact(&mut ack)
        .await
        .map_err(|e| format!("no ack: {e}"))?;
    let resume = u64::from_be_bytes(ack[1..9].try_into().expect("8 bytes"));
    match ack[0] {
        ACK_ALREADY_HAVE => return Ok("already-have".into()),
        ACK_REFUSED => return Err("peer refused the transfer".into()),
        ACK_SEND_FROM if resume <= len => {}
        _ => return Err("malformed ack".into()),
    }
    let mut at = resume;
    while at < len {
        let window = blobs
            .read_range(&digest, at, WINDOW)
            .ok_or_else(|| "artifact vanished from the local store".to_string())?;
        stream
            .write_all(&window)
            .await
            .map_err(|e| format!("send failed at {at}: {e}"))?;
        at += window.len() as u64;
    }
    let mut result = [0u8; 1];
    stream
        .read_exact(&mut result)
        .await
        .map_err(|e| format!("no result: {e}"))?;
    match result[0] {
        RESULT_OK => Ok("stored".into()),
        RESULT_CORRUPT => Err("peer rejected the bytes as corrupt".into()),
        _ => Err("peer failed to stage the bytes".into()),
    }
}

// ---- meta codecs ---------------------------------------------------------------

fn encode_push_meta(kind: u8, digest: &[u8; 32], len: u64) -> Vec<u8> {
    let mut meta = Vec::with_capacity(41);
    meta.push(kind);
    meta.extend_from_slice(digest);
    meta.extend_from_slice(&len.to_be_bytes());
    meta
}

fn decode_push_meta(meta: &[u8]) -> Option<(u8, [u8; 32], u64)> {
    if meta.len() != 41 {
        return None;
    }
    Some((
        meta[0],
        meta[1..33].try_into().ok()?,
        u64::from_be_bytes(meta[33..41].try_into().ok()?),
    ))
}

fn decode_pull_meta(meta: &[u8]) -> Option<([u8; 32], u64)> {
    if meta.len() != 40 {
        return None;
    }
    Some((
        meta[..32].try_into().ok()?,
        u64::from_be_bytes(meta[32..40].try_into().ok()?),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use commonware_cryptography::{Signer as _, ed25519};
    use data_plane::{DataPlane, PlaneConfig};
    use data_plane::sim::{LinkModel, SimNet};

    fn two_peer_net() -> (PeerId, PeerId, Arc<MediaPeers>, SimNet) {
        let key_a = ed25519::PrivateKey::from_seed(1).public_key();
        let key_b = ed25519::PrivateKey::from_seed(2).public_key();
        let a = PeerId(key_a.as_ref().try_into().unwrap());
        let b = PeerId(key_b.as_ref().try_into().unwrap());
        let peers = MediaPeers::new("code-plane-test".into());
        peers.set_peers([&key_a, &key_b].into_iter());
        let net = SimNet::new();
        net.set_link(
            a,
            b,
            LinkModel {
                latency: Duration::from_millis(1),
                bytes_per_sec: 50_000_000,
                drop_every: None,
                delay_every: None,
            },
        );
        (a, b, peers, net)
    }

    fn plane_pair(
        net: &SimNet,
        a: PeerId,
        b: PeerId,
        peers: &Arc<MediaPeers>,
    ) -> (Arc<StreamService<impl DataPlaneTransport>>, Arc<StreamService<impl DataPlaneTransport>>)
    {
        let config = PlaneConfig {
            bulk_bytes_per_sec: 50_000_000,
            bulk_burst_bytes: 256 * 1024,
        };
        let plane = |id: PeerId| {
            DataPlane::new(
                net.endpoint(id),
                Arc::new(CodeBook {
                    peers: Arc::clone(peers),
                }),
                config,
            )
        };
        let plane_a = plane(a);
        let plane_b = plane(b);
        let sa = Arc::new(
            plane_a
                .stream_service(Service::ModuleCode, StreamPolicy { accept_backlog: 4 })
                .unwrap(),
        );
        let sb = Arc::new(
            plane_b
                .stream_service(Service::ModuleCode, StreamPolicy { accept_backlog: 4 })
                .unwrap(),
        );
        // the planes must outlive the services; leak them for test lifetime.
        std::mem::forget(plane_a);
        std::mem::forget(plane_b);
        (sa, sb)
    }

    fn payload() -> Vec<u8> {
        (0..(2 * WINDOW + 777)).map(|i| (i % 250) as u8).collect()
    }

    #[tokio::test]
    async fn push_streams_verifies_and_reports_stored() {
        let (a, b, peers, net) = two_peer_net();
        let (sa, sb) = plane_pair(&net, a, b, &peers);
        let src = blobstore::BlobHandle::default();
        let dst = blobstore::BlobHandle::default();
        let digest = src.put_chunk(payload());

        let _accept = tokio::spawn(accept_loop(sb, dst.clone()));
        tokio::task::yield_now().await;
        let status = push_peer(sa, b, KIND_MODULE_CODE, digest, src)
            .await
            .expect("push lands");
        assert_eq!(status, "stored");
        assert_eq!(dst.get_chunk(&digest), Some(payload()));
    }

    #[tokio::test]
    async fn push_to_a_holder_is_already_have_and_unknown_kind_refused() {
        let (a, b, peers, net) = two_peer_net();
        let (sa, sb) = plane_pair(&net, a, b, &peers);
        let src = blobstore::BlobHandle::default();
        let dst = blobstore::BlobHandle::default();
        let digest = src.put_chunk(b"tiny".to_vec());
        dst.put_chunk(b"tiny".to_vec());

        let _accept = tokio::spawn(accept_loop(sb.clone(), dst.clone()));
        tokio::task::yield_now().await;
        let status = push_peer(Arc::clone(&sa), b, KIND_MODULE_CODE, digest, src.clone())
            .await
            .expect("holder answers");
        assert_eq!(status, "already-have");

        let err = push_peer(sa, b, 0xEE, digest, src)
            .await
            .expect_err("unknown kind refused");
        assert!(err.contains("refused"), "got: {err}");
    }

    #[tokio::test]
    async fn pull_serves_from_offset_and_misses_honestly() {
        let (a, b, peers, net) = two_peer_net();
        let (sa, sb) = plane_pair(&net, a, b, &peers);
        let holder = blobstore::BlobHandle::default();
        let digest = holder.put_chunk(payload());

        let _accept = tokio::spawn(accept_loop(sb, holder.clone()));
        tokio::task::yield_now().await;

        // pull the tail from a mid-blob offset.
        let offset = WINDOW as u64 + 5;
        let mut stream = sa
            .open(b, code_flow(), INTENT_PULL, {
                let mut m = digest.to_vec();
                m.extend_from_slice(&offset.to_be_bytes());
                m
            })
            .await
            .expect("pull opens");
        let mut header = [0u8; 9];
        stream.read_exact(&mut header).await.expect("header");
        assert_eq!(header[0], PULL_SERVING);
        let len = u64::from_be_bytes(header[1..9].try_into().unwrap());
        assert_eq!(len, payload().len() as u64);
        let mut tail = vec![0u8; (len - offset) as usize];
        stream.read_exact(&mut tail).await.expect("tail bytes");
        assert_eq!(tail, payload()[offset as usize..]);

        // a digest nobody holds answers a miss header.
        let mut stream = sa
            .open(b, code_flow(), INTENT_PULL, {
                let mut m = vec![0u8; 32];
                m.extend_from_slice(&0u64.to_be_bytes());
                m
            })
            .await
            .expect("pull opens");
        let mut header = [0u8; 9];
        stream.read_exact(&mut header).await.expect("header");
        assert_eq!(header[0], PULL_MISS);
    }
}
