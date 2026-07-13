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

use std::collections::BTreeMap;
use std::sync::Arc;

use commonware_runtime::{Spawner, Supervisor};
use dispatch_oracle::{
    AttemptControl, DeliverFn, DispatchPool, SharedProvisioner, SpawnFn,
    max_concurrent_runs_from_env,
};
use futures::SinkExt as _;
use sdk::Msg;

/// the result lane's depth — the rpc ingress precedent. results are bounded
/// by the concurrency cap, so this never fills in practice; senders await
/// (off-loop) rather than drop if it ever does.
const ORACLE_RESULT_LANE: usize = 64;

/// build the dispatch worker for this validator: a pool that spawns provider
/// runs as supervised children of `context`, a cloneable control for cancelling
/// them, and the receiver completed results return over. the caller owns the
/// receiver as a select-loop ingress arm and submits each `Msg` through the
/// normal signed submit path.
///
/// no blob lane is wired any more: an agent's persona used to be an opaque
/// `prompt_hash` blob this pool resolved (locally, then over the mesh). the
/// persona is a curated SKILL now — content-addressed in duckfs, mounted
/// read-only by the provisioner, and assembled into the run's context document
/// there. one content-addressed plane instead of two.
pub(crate) fn build<C>(
    context: &C,
    providers: capability_host::ProviderSet,
    node_key: Vec<u8>,
    provisioner: Option<SharedProvisioner>,
    // the announced sandbox capacity — the pool's `ResourceLedger`. EMPTY for
    // a direct-spawn node (demandless jobs only), the probed host totals for a
    // Podman one. SAME map the capability announce carries, so the ledger and
    // the registry can never disagree.
    capacity: BTreeMap<String, u64>,
) -> (
    Box<dyn host::worker::Worker>,
    AttemptControl,
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

    let mut pool = DispatchPool::with_limit(
        Arc::new(providers),
        node_key,
        spawn,
        deliver,
        max_concurrent_runs_from_env(),
        capacity,
    );
    // portable (v3) runs materialize a per-run duckfs workspace through this,
    // over the SAME NodeHandle actor lane the /v1/fs/workspaces RPC uses. LIVE:
    // the composer emits v3 for every run (files module wired unconditionally),
    // so this seam is on the hot path. `None` keeps the accept-only degrade
    // (raw-text delivery, no workspace) — main.rs currently always passes
    // `Some`, so the branch is defensive plumbing for embedders, not a live
    // production mode.
    if let Some(provisioner) = provisioner {
        pool = pool.with_provisioner(provisioner);
    }
    let control = pool.attempt_control();
    (Box::new(pool), control, rx)
}
