//! off-loop oracle execution for the validator: wires
//! [`dispatch_oracle::DispatchPool`] into the runtime's background-task lane
//! and the select loop's submit path.
//!
//! the pool's gate (lease check, decode, dedup) runs inline where effects
//! are offered; the provider CLI call runs on a spawned child of the node
//! context (the same supervised lane every other background plane uses).
//! completed runs push their `OracleResult` op into a bounded mpsc channel
//! shaped like the rpc ingress lane — the select loop drains it and submits
//! each op through the ordered lane under this node's key, so a result
//! re-enters consensus exactly like any other local submit.

use std::sync::Arc;

use commonware_runtime::{Spawner, Supervisor};
use dispatch_oracle::{DeliverFn, DispatchPool, SpawnFn};
use futures::SinkExt as _;
use sdk::Msg;

/// the result lane's depth — the rpc ingress precedent. results are bounded
/// by the concurrency cap, so this never fills in practice; senders await
/// (off-loop) rather than drop if it ever does.
const ORACLE_RESULT_LANE: usize = 64;

/// build the dispatch worker for this validator: a pool that spawns provider
/// runs as supervised children of `context` and hands completed results back
/// over the returned receiver. the caller owns the receiver as a select-loop
/// ingress arm and submits each `Msg` through the normal signed submit path.
pub(crate) fn build<C>(
    context: &C,
    providers: capability_host::ProviderSet,
    node_key: Vec<u8>,
) -> (
    Box<dyn reactor::Worker>,
    futures::channel::mpsc::Receiver<Msg>,
)
where
    C: Spawner + Supervisor + 'static,
{
    let (tx, rx) = futures::channel::mpsc::channel::<Msg>(ORACLE_RESULT_LANE);

    // one supervised node for the whole pool; each run spawns as its own
    // child task under it (the blackhole/background-lane precedent).
    let exec_ctx = context.child("oracle_pool");
    let spawn: SpawnFn = Box::new(move |fut| {
        exec_ctx.child("oracle_run").spawn(move |_ctx| fut);
    });

    let deliver: DeliverFn = Arc::new(move |msg| {
        let mut tx = tx.clone();
        Box::pin(async move {
            // a closed lane means the select loop is gone (shutdown): the
            // in-flight result is lost with the process, exactly like a
            // crash mid-run — the saga's lease timeout re-leases it.
            if tx.send(msg).await.is_err() {
                eprintln!("[oracle] result lane closed; dropping an oracle result");
            }
        })
    });

    let pool = DispatchPool::new(Arc::new(providers), node_key, spawn, deliver);
    (Box::new(pool), rx)
}
