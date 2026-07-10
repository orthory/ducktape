//! fetch-on-miss for node-local content-addressed blobs, over the statesync
//! mesh channel — the host half of `statesync::SyncRequest::Blob`.
//!
//! consensus pins some bytes by sha256 but never carries them (an agent's
//! registered prompt above all: the registry commits `prompt_hash`, the app
//! stages the text in ONE node's blob store via `POST /v1/files/blob`). a
//! run leasing on any OTHER node used to fail loudly ("prompt blob not in
//! this node's blob store" — the #298 cross-node gap). this module closes it
//! host-side: on a local miss the executing node asks its current peers for
//! the digest, re-hashes the answer (content addressing makes the bytes
//! self-verifying — no trust attaches to which peer answered), and writes
//! the verified copy through its own persistent store so the fetch happens
//! once, not per run. nothing consensus-visible changes.
//!
//! transport: the SAME rpc-envelope lane the mesh statesync serve arm
//! already drains. requests ride `encode_rpc(id, encode_request(Blob))`; the
//! serve loop routes an incoming frame to a pending fetch when its rpc id is
//! ours (see [`route_response`]) and otherwise treats it as a request. our
//! ids live in a random high range (top bit set) so they can never collide
//! with a peer's small sequential request ids.

use std::collections::HashMap;
use std::hash::{BuildHasher as _, Hasher as _};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use commonware_cryptography::ed25519;
use commonware_p2p::{Recipients, Sender as P2pSender};
use commonware_runtime::IoBuf;
use futures::future::BoxFuture;
use sha2::{Digest as _, Sha256};

/// one blob fetch waits this long for the whole peer fan-out — generous for
/// a localhost mesh, small next to the pool's provider timeout budget.
const FETCH_TIMEOUT: Duration = Duration::from_secs(8);

/// the serve side answers only blobs up to this size: the mesh channel is
/// sized for statesync's 256 KiB chunks, and the one production consumer
/// (prompt pins) is kilobytes. an oversized blob answers as an honest miss.
pub const MAX_SERVED_BLOB: usize = 1024 * 1024;

/// outstanding fetches keyed by rpc id — the serve loop's demux surface: an
/// incoming mesh frame whose id is in here is a response to US, everything
/// else is a peer's request.
pub type PendingMap =
    Arc<Mutex<HashMap<u64, tokio::sync::oneshot::Sender<statesync::SyncResponse>>>>;

/// the shape `oracle_pool::build` composes behind its resolver: digest in,
/// verified bytes (or a miss) out.
pub type BlobFetchFn = Arc<dyn Fn([u8; 32]) -> BoxFuture<'static, Option<Vec<u8>>> + Send + Sync>;

/// one classified inbound mesh frame — the serve loop acts on exactly this.
pub enum MeshFrame {
    /// completed (or was addressed to) one of OUR pending fetches: consumed
    /// here, nothing to serve. a malformed body on a matched id lands here
    /// too — the dropped waiter surfaces as that fetch's timeout, never as a
    /// bogus "request" from a peer.
    OurResponse,
    /// a well-formed RESPONSE matching no pending fetch — a blob answer
    /// arriving after its fan-out's sweep. dropped, NEVER answered: replying
    /// to a reply is how two serve loops oscillate Error frames forever (one
    /// slow peer could start that loop at zero cost).
    StrayResponse,
    /// a peer's request, decoded and ready to serve.
    Request(statesync::SyncRequest),
    /// neither request nor response: version skew (a kind this binary does
    /// not know) or a stray — dropped, mirroring the malformed-rpc-envelope
    /// precedent. no Error fast-fail is owed: the authenticated channel does
    /// not corrupt frames, and answering unparseable bytes is the other half
    /// of the oscillation fuel.
    Junk,
}

/// classify one inbound mesh frame. request-decode is tried BEFORE the
/// stray-response check on purpose: a real request whose bytes happen to
/// also parse as a response (the tag spaces overlap) must still be served —
/// the reverse mistake would silently eat legitimate sync requests. the
/// residual (a stray response that happens to parse as a request) costs one
/// spurious served reply, which the peer's own classify then drops — the
/// exchange is self-limiting, never a standing loop.
pub fn classify_mesh_frame(pending: &PendingMap, rpc_id: u64, body: &[u8]) -> MeshFrame {
    if let Some(waiter) = pending.lock().expect("pending blob lock").remove(&rpc_id) {
        if let Ok(resp) = statesync::decode_response(body) {
            let _ = waiter.send(resp);
        }
        return MeshFrame::OurResponse;
    }
    match statesync::decode_request(body) {
        Ok(req) => MeshFrame::Request(req),
        Err(_) => match statesync::decode_response(body) {
            Ok(_) => MeshFrame::StrayResponse,
            Err(_) => MeshFrame::Junk,
        },
    }
}

/// answer a peer's blob request from this node's store — the serve loop's
/// intercept arm (blobs are host state; `SyncServer` never sees these).
pub fn serve_blob(blobs: &blobstore::BlobHandle, digest: &[u8; 32]) -> statesync::SyncResponse {
    let bytes = blobs
        .get_chunk(digest)
        .filter(|b| b.len() <= MAX_SERVED_BLOB);
    statesync::SyncResponse::Blob { bytes }
}

/// the requester: fan a digest out to the current peer set over the mesh and
/// return the first answer that re-hashes to the digest.
pub struct MeshBlobFetcher<S> {
    sender: S,
    pending: PendingMap,
    /// the tracked peer set (members + standbys), swapped at every cutover
    /// re-track — the same set the mesh serves statesync to.
    peers: Arc<RwLock<Vec<ed25519::PublicKey>>>,
    me: ed25519::PublicKey,
    next_id: AtomicU64,
}

impl<S> MeshBlobFetcher<S>
where
    S: P2pSender<PublicKey = ed25519::PublicKey> + Clone + Send + Sync + 'static,
{
    pub fn new(
        sender: S,
        pending: PendingMap,
        peers: Arc<RwLock<Vec<ed25519::PublicKey>>>,
        me: ed25519::PublicKey,
    ) -> Self {
        // a random high id range per process: `RandomState` is std's own
        // per-process entropy, and the forced top bit keeps our ids disjoint
        // from any peer's small sequential request ids by construction.
        let seed = std::collections::hash_map::RandomState::new()
            .build_hasher()
            .finish()
            | (1 << 63);
        Self {
            sender,
            pending,
            peers,
            me,
            next_id: AtomicU64::new(seed),
        }
    }

    /// erase the sender generic for the oracle pool's resolver seam.
    pub fn into_fetch_fn(self) -> BlobFetchFn {
        let this = Arc::new(self);
        Arc::new(move |digest| {
            let this = Arc::clone(&this);
            Box::pin(async move { this.fetch(digest).await })
        })
    }

    /// ask every current peer for `digest` concurrently; first verified
    /// answer wins, a full fan-out of misses (or the deadline) is `None`.
    pub async fn fetch(&self, digest: [u8; 32]) -> Option<Vec<u8>> {
        let peers: Vec<ed25519::PublicKey> = self
            .peers
            .read()
            .expect("blob peers lock")
            .iter()
            .filter(|p| **p != self.me)
            .cloned()
            .collect();
        if peers.is_empty() {
            return None;
        }
        let mut ids = Vec::with_capacity(peers.len());
        let mut waits = Vec::with_capacity(peers.len());
        for peer in &peers {
            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            let (tx, rx) = tokio::sync::oneshot::channel();
            self.pending
                .lock()
                .expect("pending blob lock")
                .insert(id, tx);
            ids.push(id);
            let frame = statesync::encode_rpc(
                id,
                &statesync::encode_request(&statesync::SyncRequest::Blob { digest }),
            );
            let mut sender = self.sender.clone();
            // an empty attempt set (peer offline) is fine: that oneshot just
            // times out with the rest of the fan-out.
            let _attempted = sender.send(Recipients::One(peer.clone()), IoBuf::from(frame), false);
            waits.push(rx);
        }
        let verified = async {
            let mut waits: Vec<_> = waits.into_iter().collect();
            while !waits.is_empty() {
                let (outcome, _idx, rest) = futures::future::select_all(waits).await;
                waits = rest;
                if let Ok(statesync::SyncResponse::Blob { bytes: Some(bytes) }) = outcome {
                    let mut h = Sha256::new();
                    h.update(&bytes);
                    if <[u8; 32]>::from(h.finalize()) == digest {
                        return Some(bytes);
                    }
                    // wrong bytes for the digest: a corrupt or hostile
                    // answer — keep waiting on the remaining peers.
                }
            }
            None
        };
        let outcome = tokio::time::timeout(FETCH_TIMEOUT, verified)
            .await
            .unwrap_or(None);
        // sweep every id this fan-out registered: answered ones are already
        // gone, timed-out ones must not leak into the demux map.
        let mut pending = self.pending.lock().expect("pending blob lock");
        for id in ids {
            pending.remove(&id);
        }
        outcome
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_completes_pending_ids_and_serves_requests() {
        let pending: PendingMap = Default::default();
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        pending.lock().unwrap().insert(9, tx);

        // a real request under an unknown id serves, map untouched.
        let req = statesync::encode_request(&statesync::SyncRequest::Blob { digest: [7u8; 32] });
        assert!(matches!(
            classify_mesh_frame(&pending, 8, &req),
            MeshFrame::Request(statesync::SyncRequest::Blob { digest }) if digest == [7u8; 32]
        ));
        assert_eq!(pending.lock().unwrap().len(), 1);

        // the matching id completes the waiter and clears the entry.
        let body = statesync::encode_response(&statesync::SyncResponse::Blob { bytes: None });
        assert!(matches!(
            classify_mesh_frame(&pending, 9, &body),
            MeshFrame::OurResponse
        ));
        assert!(pending.lock().unwrap().is_empty());
        assert_eq!(
            rx.try_recv().unwrap(),
            statesync::SyncResponse::Blob { bytes: None }
        );
    }

    #[test]
    fn classify_drops_late_responses_instead_of_serving_them() {
        // THE oscillation guard: a blob answer arriving after its fan-out's
        // sweep matches no pending id. it must classify as a stray (dropped),
        // never as a request — a served (or Error-answered) reply would bounce
        // between two serve loops forever.
        let pending: PendingMap = Default::default();
        let late = statesync::encode_response(&statesync::SyncResponse::Blob {
            bytes: Some(b"You are quack.".to_vec()),
        });
        assert!(matches!(
            classify_mesh_frame(&pending, 42, &late),
            MeshFrame::StrayResponse
        ));
        // and the other oscillation half: an Error response (what the old
        // code answered strays with) is itself a stray here, not a request.
        let error = statesync::encode_response(&statesync::SyncResponse::Error(
            "bad request frame: whatever".into(),
        ));
        assert!(matches!(
            classify_mesh_frame(&pending, 42, &error),
            MeshFrame::StrayResponse
        ));
    }

    #[test]
    fn classify_swallows_malformed_frames_for_matched_ids_and_junks_the_rest() {
        let pending: PendingMap = Default::default();
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        pending.lock().unwrap().insert(3, tx);
        // matched id + garbage body: still ours, waiter dropped so the fetch
        // times out instead of hanging or misreading a "request".
        assert!(matches!(
            classify_mesh_frame(&pending, 3, b"not a response"),
            MeshFrame::OurResponse
        ));
        assert!(rx.try_recv().is_err());
        // unmatched garbage is junk: dropped, never answered.
        assert!(matches!(
            classify_mesh_frame(&pending, 4, b"not anything"),
            MeshFrame::Junk
        ));
    }

    #[test]
    fn serve_blob_answers_store_hits_and_honest_misses() {
        let blobs = blobstore::BlobHandle::default();
        let digest = blobs.put_chunk(b"You are quack.".to_vec());
        assert_eq!(
            serve_blob(&blobs, &digest),
            statesync::SyncResponse::Blob {
                bytes: Some(b"You are quack.".to_vec())
            }
        );
        assert_eq!(
            serve_blob(&blobs, &[0u8; 32]),
            statesync::SyncResponse::Blob { bytes: None }
        );
        // oversized blobs answer as a miss — the mesh frame stays bounded.
        let big = blobs.put_chunk(vec![7u8; MAX_SERVED_BLOB + 1]);
        assert_eq!(
            serve_blob(&blobs, &big),
            statesync::SyncResponse::Blob { bytes: None }
        );
    }
}
