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
    SyncClient, SyncError, SyncRequest, SyncResponse, decode_response, decode_rpc, encode_request,
    encode_rpc,
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

/// the dispatch task's lane-reclaim seam: fire `stop` and the task hands the
/// p2p receiver back over `handback`, then exits. built by
/// [`LaneReclaim::arm`]; the caller keeps the returned trigger halves and
/// passes the seam into [`P2pSyncClient::with_sources`]. reclaim is only
/// sound once the caller has quiesced its own requests — any frame the
/// revoked `recv()` had in flight is dropped, which with an empty pending map
/// and no unmatched hook is exactly what the dispatch loop would have done.
pub struct LaneReclaim<R> {
    stop: oneshot::Receiver<()>,
    handback: oneshot::Sender<R>,
}

impl<R> LaneReclaim<R> {
    /// build the seam: the client-side halves (trigger, receiver-return) and
    /// the task-side seam itself.
    pub fn arm() -> (oneshot::Sender<()>, oneshot::Receiver<R>, Self) {
        let (stop_tx, stop_rx) = oneshot::channel();
        let (handback_tx, handback_rx) = oneshot::channel();
        (
            stop_tx,
            handback_rx,
            Self {
                stop: stop_rx,
                handback: handback_tx,
            },
        )
    }
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
        Self::with_sources(
            context,
            sender,
            receiver,
            vec![server],
            None,
            requester,
            proof,
            None,
        )
    }

    /// bind a client to a non-empty ordered candidate set over a registered
    /// channel pair, spawning the dispatch + reaper task on `context`
    /// (consumed — contexts are move-only). requests go to the cursor's
    /// candidate; a transport failure advances the cursor. `unmatched`
    /// receives frames whose id belongs to no pending request here (see
    /// [`UnmatchedFrameHook`]); `None` keeps the old drop-on-miss behavior.
    /// `reclaim` (see [`LaneReclaim`]) lets the caller revoke the dispatch
    /// task and take the receiver back; `None` = the lane is the task's for
    /// the process's life.
    #[allow(clippy::too_many_arguments)]
    pub fn with_sources<E, R>(
        context: E,
        sender: S,
        mut receiver: R,
        candidates: Vec<S::PublicKey>,
        unmatched: Option<UnmatchedFrameHook>,
        requester: [u8; 32],
        proof: [u8; 64],
        reclaim: Option<LaneReclaim<R>>,
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
        // ONE loop shape whether or not a reclaim seam was armed: an unarmed
        // client parks a never-firing stop (its trigger rides in the task, so
        // the oneshot can't cancel) and the handback goes nowhere.
        let (stop_rx, handback_tx, _hold_open) = match reclaim {
            Some(LaneReclaim { stop, handback }) => (stop, handback, None),
            None => {
                let (hold, stop) = oneshot::channel::<()>();
                let (handback, _dropped) = oneshot::channel::<R>();
                (stop, handback, Some(hold))
            }
        };
        context.spawn(move |ctx| async move {
            let _hold_open = _hold_open;
            let mut stop_rx = stop_rx;
            // the REAPER runs as its own task: the dispatch loop below stays
            // a bare `recv().await` loop in steady state — select-dropping an
            // actor-backed p2p recv future mid-flight can eat a delivered
            // message, which here would silently turn a served response into
            // a client timeout. the stop arm only ever fires at reclaim,
            // whose contract (see [`LaneReclaim`]) requires quiesced
            // requests, so the one drop it can cause loses nothing a live
            // request was owed.
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
            loop {
                // the recv borrow must end before the receiver can be handed
                // back, so the stop arm resolves to `None` and the handback
                // happens outside the select expression.
                let frame = futures::select_biased! {
                    _ = stop_rx => None,
                    frame = futures::FutureExt::fuse(receiver.recv()) => Some(frame),
                };
                let Some(frame) = frame else {
                    let _ = handback_tx.send(receiver);
                    return;
                };
                let Ok((peer, msg)) = frame else { break };
                // only a candidate source may complete requests.
                if !expected.candidates.contains(&peer) {
                    continue;
                }
                let bytes: Vec<u8> = msg.into();
                // replies ride the same authed frame; the auth fields are
                // server zero-fill (the transport-peer check above is the
                // reply's authenticity), so only id + body matter here.
                let Ok((_requester, _proof, id, body)) = decode_rpc(&bytes) else {
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

    /// a clone of the underlying channel sender — the promotion seam pairs it
    /// with the receiver a [`LaneReclaim`] hands back.
    pub fn lane_sender(&self) -> S {
        self.sender.clone()
    }
}

impl<S> P2pSyncClient<S>
where
    S: Sender,
{
    /// The candidate selected for the next request. Operational surfaces use
    /// this after a successful request so source rotation stays visible.
    pub fn current_source(&self) -> S::PublicKey {
        self.sources.current().1
    }

    /// advance the serving cursor one candidate. the transport rotates on
    /// FAILURE by itself ([`Sources::advance_past`], wave-deduped); this is
    /// for callers whose retry policy also rotates on an HONEST answer — a
    /// blob fetch's "don't have it" is a valid response the failure path
    /// never sees.
    pub fn advance_source(&self) {
        self.sources.cursor.fetch_add(1, Ordering::Relaxed);
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
            let frame = encode_rpc(&requester, &proof, id, &encode_request(&req));
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
