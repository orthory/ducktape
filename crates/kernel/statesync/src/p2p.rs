//! the p2p transport binding: a [`SyncClient`] over one commonware p2p channel.
//!
//! requests ride the rpc envelope (`id || frame`) addressed to ONE serving
//! peer; a background dispatch task drains the channel receiver and routes
//! each response to its awaiting request by id. responses claiming to be from
//! any other peer than the chosen server are dropped — the mesh is
//! authenticated, and installable payloads are root-verified anyway, but there
//! is no reason to let an unrelated peer complete someone else's request.
//!
//! every request is TIMED OUT by the dispatch task's reaper: p2p sends to a
//! peer whose link is not (or no longer) up are silently dropped by the mesh,
//! so an unanswered request must fail — the caller retries (a joiner's
//! manifest loop does exactly that while the mesh warms up) instead of parking
//! forever on a reply that will never come. the reaper lives in the dispatch
//! task because runtime contexts are move-only (not `Clone`): the spawned task
//! owns the only clock, and the request future itself stays runtime-free.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
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

/// a [`SyncClient`] whose requests cross a real p2p channel to one server peer.
pub struct P2pSyncClient<S: Sender> {
    sender: S,
    server: S::PublicKey,
    shared: Arc<Shared>,
}

impl<S: Sender> Clone for P2pSyncClient<S> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            server: self.server.clone(),
            shared: Arc::clone(&self.shared),
        }
    }
}

impl<S> P2pSyncClient<S>
where
    S: Sender,
{
    /// bind a client to `server` over a registered channel pair, spawning the
    /// dispatch + reaper task on `context` (consumed — contexts are move-only).
    pub fn new<E, R>(context: E, sender: S, mut receiver: R, server: S::PublicKey) -> Self
    where
        E: Spawner + Clock + Send + 'static,
        R: Receiver<PublicKey = S::PublicKey> + Send + 'static,
    {
        let shared = Arc::new(Shared {
            pending: Mutex::new(HashMap::new()),
            tick: AtomicU64::new(0),
            next_id: AtomicU64::new(0),
        });
        let task_shared = Arc::clone(&shared);
        let reap_shared = Arc::clone(&shared);
        let expected = server.clone();
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
                // only the chosen server may complete requests.
                if peer != expected {
                    continue;
                }
                let bytes: Vec<u8> = msg.into();
                let Ok((id, body)) = decode_rpc(&bytes) else {
                    continue;
                };
                let waiter = task_shared
                    .pending
                    .lock()
                    .expect("pending poisoned")
                    .remove(&id);
                if let Some(entry) = waiter {
                    let _ = entry.reply.send(body.to_vec());
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
            server,
            shared,
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
        let server = self.server.clone();
        let shared = Arc::clone(&self.shared);
        async move {
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
            let frame = encode_rpc(id, &encode_request(&req));
            let attempted = sender.send(Recipients::One(server), IoBuf::from(frame), false);
            if attempted.is_empty() {
                // the server peer is offline/unreachable right now — fail fast
                // instead of waiting out the reaper.
                shared.pending.lock().expect("pending poisoned").remove(&id);
                return Err(SyncError::Transport(
                    "server peer unreachable (send attempted no recipients)".into(),
                ));
            }
            // resolves when the response routes back — or errs when the reaper
            // drops the slot (dropped send / dead server) so callers can retry.
            let bytes = rx.await.map_err(|_| {
                SyncError::Transport(format!(
                    "request {id} timed out (send dropped by the mesh or server dead)"
                ))
            })?;
            Ok(decode_response(&bytes)?)
        }
    }
}
