//! the p2p transport binding: a [`SyncClient`] over one commonware p2p channel.
//!
//! requests ride the rpc envelope (`id || frame`) addressed to ONE serving
//! peer at a time; a background dispatch task drains the channel receiver and
//! routes each response to its awaiting request by id. responses claiming to
//! be from a peer outside the candidate SOURCE set are dropped — the mesh is
//! authenticated, and installable payloads are root-verified anyway, but there
//! is no reason to let an unrelated peer complete someone else's request.
//!
//! SOURCE ROTATION: the client holds a candidate list, not a pinned server.
//! every payload is verified against consensus-agreed roots, so which peer
//! serves is purely an availability question — a request that fails at the
//! transport (unreachable send, reaper timeout) advances the cursor to the
//! next candidate and surfaces the error, and the caller's existing retry
//! (manifest loops, the qmdb refetch ladder) lands on the new source. one
//! failure advances the cursor once, no matter how many concurrent requests
//! observed it.
//!
//! every request is TIMED OUT by the dispatch task's reaper: p2p sends to a
//! peer whose link is not (or no longer) up are silently dropped by the mesh,
//! so an unanswered request must fail — the caller retries (a joiner's
//! manifest loop does exactly that while the mesh warms up) instead of parking
//! forever on a reply that will never come. the reaper lives in the dispatch
//! task because runtime contexts are move-only (not `Clone`): the spawned task
//! owns the only clock, and the request future itself stays runtime-free.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use commonware_p2p::{Receiver, Recipients, Sender};
use commonware_runtime::{Clock, IoBuf, Spawner};
use futures::channel::oneshot;

use crate::{
    SyncClient, SyncError, SyncRequest, SyncResponse, decode_response, decode_rpc_authed,
    encode_request, encode_rpc_authed,
};

/// the reaper's sweep interval. a request survives at most two sweeps, so the
/// effective request timeout is between one and two intervals (3–6s).
pub const REAP_INTERVAL: Duration = Duration::from_secs(3);

/// sink for inbound frames whose rpc id matches no pending request: another
/// owner may multiplex its own id space over the same channel (id spaces are
/// disjoint by construction — this client's ids are small and sequential, a
/// co-owner picks a range that can't collide). the hook gets the raw
/// `(id, body)`; the kernel stays payload-agnostic.
pub type UnmatchedFrameHook = Arc<dyn Fn(u64, &[u8]) + Send + Sync>;

/// a pending request: the reply slot plus the reaper tick it was filed under.
struct PendingEntry {
    reply: oneshot::Sender<Vec<u8>>,
    filed_at_tick: u64,
}

struct Shared {
    pending: Mutex<HashMap<u64, PendingEntry>>,
    /// monotonically increasing reaper tick (written only by the reaper).
    tick: AtomicU64,
    next_id: AtomicU64,
}

/// the rotating candidate source set (see the module doc's SOURCE ROTATION).
struct Sources<P> {
    candidates: Vec<P>,
    cursor: AtomicUsize,
}

impl<P: Clone + PartialEq> Sources<P> {
    fn current(&self) -> (usize, P) {
        let at = self.cursor.load(Ordering::Relaxed) % self.candidates.len();
        (at, self.candidates[at].clone())
    }

    /// advance past the source at `observed` — exactly once per failure wave:
    /// a concurrent request that saw the same source fail leaves the cursor
    /// where the first advance put it.
    fn advance_past(&self, observed: usize) {
        let _ = self.cursor.compare_exchange(
            observed,
            observed + 1,
            Ordering::Relaxed,
            Ordering::Relaxed,
        );
    }
}

/// a [`SyncClient`] whose requests cross a real p2p channel to one of a
/// candidate set of serving peers, failing over on transport failure.
pub struct P2pSyncClient<S: Sender> {
    sender: S,
    sources: Arc<Sources<S::PublicKey>>,
    shared: Arc<Shared>,
    /// the caller's real-key standing proof (ADR §5.1), signed ONCE via
    /// [`crate::sign_sync_proof`] and attached to every request. the server
    /// verifies it against committed standing and fail-closes on a mismatch.
    requester: [u8; 32],
    proof: [u8; 64],
}

impl<S: Sender> Clone for P2pSyncClient<S> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            sources: Arc::clone(&self.sources),
            shared: Arc::clone(&self.shared),
            requester: self.requester,
            proof: self.proof,
        }
    }
}

impl<S> P2pSyncClient<S>
where
    S: Sender,
{
    /// bind a client to one `server` — [`P2pSyncClient::with_sources`] with a
    /// single candidate. `requester`/`proof` are the caller's standing proof
    /// ([`crate::sign_sync_proof`]).
    pub fn new<E, R>(
        context: E,
        sender: S,
        receiver: R,
        server: S::PublicKey,
        requester: [u8; 32],
        proof: [u8; 64],
    ) -> Self
    where
        E: Spawner + Clock + Send + 'static,
        R: Receiver<PublicKey = S::PublicKey> + Send + 'static,
    {
        Self::with_sources(context, sender, receiver, vec![server], None, requester, proof)
    }

    /// bind a client to a non-empty ordered candidate set over a registered
    /// channel pair, spawning the dispatch + reaper task on `context`
    /// (consumed — contexts are move-only). requests go to the cursor's
    /// candidate; a transport failure advances the cursor. `unmatched`
    /// receives frames whose id belongs to no pending request here (see
    /// [`UnmatchedFrameHook`]); `None` keeps the old drop-on-miss behavior.
    pub fn with_sources<E, R>(
        context: E,
        sender: S,
        mut receiver: R,
        candidates: Vec<S::PublicKey>,
        unmatched: Option<UnmatchedFrameHook>,
        requester: [u8; 32],
        proof: [u8; 64],
    ) -> Self
    where
        E: Spawner + Clock + Send + 'static,
        R: Receiver<PublicKey = S::PublicKey> + Send + 'static,
    {
        assert!(!candidates.is_empty(), "at least one sync source");
        let sources = Arc::new(Sources {
            candidates,
            cursor: AtomicUsize::new(0),
        });
        let shared = Arc::new(Shared {
            pending: Mutex::new(HashMap::new()),
            tick: AtomicU64::new(0),
            next_id: AtomicU64::new(0),
        });
        let task_shared = Arc::clone(&shared);
        let reap_shared = Arc::clone(&shared);
        let expected = Arc::clone(&sources);
        context.spawn(move |ctx| async move {
            // the REAPER runs as its own task: the dispatch loop below must be
            // a bare `recv().await` loop — select-dropping an actor-backed p2p
            // recv future mid-flight can eat a delivered message, which here
            // would silently turn a served response into a client timeout.
            ctx.spawn(move |reap_ctx| async move {
                loop {
                    reap_ctx.sleep(REAP_INTERVAL).await;
                    // a request filed two ticks ago never got its reply —
                    // dropping its sender fails the caller's await so it can
                    // retry.
                    let now = reap_shared.tick.fetch_add(1, Ordering::Relaxed) + 1;
                    reap_shared
                        .pending
                        .lock()
                        .expect("pending poisoned")
                        .retain(|_, e| now.saturating_sub(e.filed_at_tick) < 2);
                }
            });
            while let Ok((peer, msg)) = receiver.recv().await {
                // only a candidate source may complete requests.
                if !expected.candidates.contains(&peer) {
                    continue;
                }
                let bytes: Vec<u8> = msg.into();
                // replies ride the same authed frame; the auth fields are
                // server zero-fill (the transport-peer check above is the
                // reply's authenticity), so only id + body matter here.
                let Ok((_requester, _proof, id, body)) = decode_rpc_authed(&bytes) else {
                    continue;
                };
                let waiter = task_shared
                    .pending
                    .lock()
                    .expect("pending poisoned")
                    .remove(&id);
                if let Some(entry) = waiter {
                    let _ = entry.reply.send(body.to_vec());
                } else if let Some(hook) = &unmatched {
                    hook(id, body);
                }
            }
            // channel closed: drop every waiter so requests fail instead of
            // hanging forever.
            task_shared
                .pending
                .lock()
                .expect("pending poisoned")
                .clear();
        });
        Self {
            sender,
            sources,
            shared,
            requester,
            proof,
        }
    }
}

impl<S> SyncClient for P2pSyncClient<S>
where
    S: Sender,
{
    fn request(
        &self,
        req: SyncRequest,
    ) -> impl std::future::Future<Output = Result<SyncResponse, SyncError>> + Send {
        let mut sender = self.sender.clone();
        let sources = Arc::clone(&self.sources);
        let shared = Arc::clone(&self.shared);
        let requester = self.requester;
        let proof = self.proof;
        async move {
            let (at, server) = sources.current();
            let id = shared.next_id.fetch_add(1, Ordering::Relaxed);
            let (tx, rx) = oneshot::channel();
            {
                let mut pending = shared.pending.lock().expect("pending poisoned");
                pending.insert(
                    id,
                    PendingEntry {
                        reply: tx,
                        filed_at_tick: shared.tick.load(Ordering::Relaxed),
                    },
                );
            }
            let frame = encode_rpc_authed(&requester, &proof, id, &encode_request(&req));
            let attempted = sender.send(Recipients::One(server), IoBuf::from(frame), false);
            if attempted.is_empty() {
                // the source is offline/unreachable right now — fail fast
                // instead of waiting out the reaper, and rotate.
                shared.pending.lock().expect("pending poisoned").remove(&id);
                sources.advance_past(at);
                return Err(SyncError::Transport(
                    "sync source unreachable (send attempted no recipients)".into(),
                ));
            }
            // resolves when the response routes back — or errs when the reaper
            // drops the slot (dropped send / dead server), rotating so the
            // caller's retry lands on the next candidate.
            let bytes = match rx.await {
                Ok(bytes) => bytes,
                Err(_) => {
                    sources.advance_past(at);
                    return Err(SyncError::Transport(format!(
                        "request {id} timed out (send dropped by the mesh or source dead)"
                    )));
                }
            };
            Ok(decode_response(&bytes)?)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failure_wave_advances_the_cursor_exactly_once() {
        let sources = Sources {
            candidates: vec!["a", "b", "c"],
            cursor: AtomicUsize::new(0),
        };
        let (at, first) = sources.current();
        assert_eq!((at, first), (0, "a"));

        // three concurrent requests all observed source 0 fail — one advance.
        sources.advance_past(at);
        sources.advance_past(at);
        sources.advance_past(at);
        assert_eq!(sources.current().1, "b");

        // the list wraps: a dead tail rotates back to the head.
        sources.advance_past(1);
        assert_eq!(sources.current().1, "c");
        sources.advance_past(2);
        assert_eq!(sources.current().1, "a");
    }
}
