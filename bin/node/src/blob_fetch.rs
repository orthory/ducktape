//! the blob mesh lane's SERVE half — the host side of
//! `statesync::SyncRequest::Blob`, plus the demux that keeps a peer's frames
//! and our own straight.
//!
//! the REQUESTER half is gone. it existed for exactly one consumer: an agent's
//! registered prompt, pinned in consensus as `prompt_hash` and staged in ONE
//! node's blob store, which a run leasing on any other node had to fetch on
//! miss (the #298 cross-node gap). the persona is a curated SKILL now —
//! content-addressed in duckfs, consensus-replicated, materialized as a
//! read-only mount by the provisioner on whichever node runs — so nothing
//! fetches blobs across the mesh any more and `MeshBlobFetcher` was deleted
//! with the prompt lane.
//!
//! what remains ANSWERS peers: [`serve_blob`] replies to a blob request from
//! this node's store, and [`classify_mesh_frame`] sorts inbound frames on the
//! shared rpc lane. ponytail: with no requester left in the network these are
//! vestigial — nobody asks. they stay because the serve arm is wired into the
//! statesync loop, and unpicking that loop is a separate change from retiring
//! the prompt.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// the serve side answers only blobs up to this size: the mesh channel is
/// sized for statesync's 256 KiB chunks. an oversized blob answers as an
/// honest miss.
pub const MAX_SERVED_BLOB: usize = 1024 * 1024;

/// outstanding fetches keyed by rpc id — the serve loop's demux surface: an
/// incoming mesh frame whose id is in here is a response to US, everything
/// else is a peer's request.
pub type PendingMap =
    Arc<Mutex<HashMap<u64, tokio::sync::oneshot::Sender<statesync::SyncResponse>>>>;

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
