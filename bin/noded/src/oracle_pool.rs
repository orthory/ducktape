//! off-loop oracle execution for the embedded daemon: wires
//! [`dispatch_oracle::DispatchPool`] into the actor's runtime context and
//! the daemon's own command lane.
//!
//! the pool's gate (lease check, decode, dedup) runs inline where
//! `offer_effects` offers each block's effects; the provider CLI call runs
//! on a spawned child of the actor context. a completed run injects its
//! `OracleResult` as a [`NodeCommand::Submit`] — exactly the http thread's
//! path — so the single serial command loop commits it as its own block
//! (and drains its follow-ups) WITHOUT ever having awaited the provider:
//! Query/Status/Submit commands interleave freely with long runs.

use std::sync::Arc;

use commonware_runtime::{Spawner, Supervisor};
use dispatch_oracle::{BlobResolver, DeliverFn, DispatchPool, SharedProvisioner, SpawnFn};
use futures::SinkExt as _;
use futures::channel::mpsc;
use noded::NodeCommand;

use crate::ORACLE_ORIGIN;

/// the daemon's worker set: the dispatch pool (or, in debug builds under
/// `DUCKTAPE_NODED_ECHO_ORACLE`, the inline echo stand-in).
///
/// `blobs` is the daemon's node-local content-addressed store (the app's
/// putBlob lane) — run-envelope prompt pins resolve through its read path.
/// `agent_dirs` roots persistent agent workspaces + session files under the
/// daemon's storage dir (host-local, never consensus). `storage` keys the
/// portable run-workspace root's per-node salt and its D7 boot validation.
pub(crate) fn oracle_workers<C>(
    context: &C,
    cmds: mpsc::Sender<NodeCommand>,
    node_handle: noded::NodeHandle,
    blobs: noded::blobs::BlobHandle,
    agent_dirs: capability_host::AgentDirs,
    storage: &std::path::Path,
) -> Vec<Box<dyn reactor::Worker>>
where
    C: Spawner + Supervisor + 'static,
{
    #[cfg(debug_assertions)]
    {
        if std::env::var_os("DUCKTAPE_NODED_ECHO_ORACLE").is_some() {
            return vec![Box::new(EchoWorker)];
        }
    }
    let providers = capability_host::discover_with_dirs(agent_dirs)
        // BYO: run whatever executor CLIs the capability specs describe and
        // this host has installed — no credential handling here (see
        // docs/records/specs/capability-spec.md). a broken operator spec is a boot error.
        .unwrap_or_else(|e| panic!("capability specs failed to load: {e}"));

    // one supervised node for the pool; each run spawns as its own child.
    let exec_ctx = context.child("oracle_pool");
    let spawn: SpawnFn = Box::new(move |fut| {
        exec_ctx.child("oracle_run").spawn(move |_ctx| fut);
    });

    let deliver: DeliverFn = Arc::new(move |msg| {
        let mut cmds = cmds.clone();
        Box::pin(async move {
            let (reply, done) = futures::channel::oneshot::channel();
            let cmd = NodeCommand::Submit {
                target: msg.target,
                payload: msg.payload,
                // the daemon's oracle identity, same as the drain-lane
                // follow-ups: an Accept claim recorded this key as the
                // assignee, so the result submits under it too.
                origin: ORACLE_ORIGIN.to_vec(),
                reply,
            };
            if cmds.send(cmd).await.is_err() {
                // the actor is gone (shutdown): the in-flight result dies
                // with the process, exactly like a crash mid-run.
                eprintln!("[noded] command lane closed; dropping an oracle result");
                return;
            }
            match done.await {
                Ok(Ok(_block)) => {}
                // a rejected result is a deterministic verdict (e.g. the
                // saga already settled the attempt) — log, never retry.
                Ok(Err(e)) => eprintln!("[noded] oracle result rejected: {e}"),
                Err(_) => eprintln!("[noded] oracle result reply dropped"),
            }
        })
    });

    // prompt resolution: a synchronous in-memory read behind the pool's
    // async seam. `None` (blob absent on this node) fails the run loudly in
    // the worker — never a silent fallback to the generic instructions.
    let resolver: BlobResolver = Arc::new(move |digest: &[u8; 32]| {
        let blobs = blobs.clone();
        let digest = *digest;
        Box::pin(async move { blobs.get_chunk(&digest) })
    });

    // the REAL workspace provisioner: portable (v3) runs materialize a per-run
    // duckfs checkout under a root VALIDATED to be outside <storage> (D7),
    // drive it over the daemon's OWN actor lane (no self-dial), commit the
    // output_ref, and clean up. LIVE for every agent run: the daemon wires the
    // files module unconditionally, so the runs composer emits v3 (the
    // de-versioned activation — no flag day, pre-production re-genesis). a
    // misconfigured root (inside <storage>) is a boot error, never a silent
    // D7 hole.
    let provisioner: SharedProvisioner = Arc::new(noded::agent_provision::NodedProvisioner::new(
        node_handle,
        noded::agent_provision::agent_runs_root(storage)
            .unwrap_or_else(|e| panic!("agent runs root failed D7 validation: {e}")),
    ));

    vec![Box::new(
        DispatchPool::new(
            Arc::new(providers),
            // the daemon's oracle identity: its worker follow-ups are
            // submitted under ORACLE_ORIGIN, so an Accept claim records that
            // key as the assignee and the re-emitted request must match it.
            ORACLE_ORIGIN.to_vec(),
            spawn,
            deliver,
        )
        .with_resolver(resolver)
        .with_provisioner(provisioner),
    )]
}

/// a debug-only stand-in that answers every dispatch WorkSpec inline with a
/// deterministic echo — keeps daemon e2e block arithmetic exact (no spawn,
/// no lane hop) while exercising the full effect->result->delivery path.
#[cfg(debug_assertions)]
struct EchoWorker;

#[cfg(debug_assertions)]
#[async_trait::async_trait(?Send)]
impl reactor::Worker for EchoWorker {
    async fn run(
        &self,
        effect: &sdk::Effect,
    ) -> Result<reactor::WorkOutcome, reactor::Error> {
        let request = match saga::decode_worker_request(&effect.0) {
            Ok(request) => request,
            Err(_) => return Ok(reactor::WorkOutcome::NotMine),
        };
        // a dispatch-plane WorkSpec echoes its raw-text lane (the dispatch
        // module judged a Text contract; the agent module normalizes).
        let Ok(work) = dispatch::decode_work_spec(&request.spec) else {
            return Ok(reactor::WorkOutcome::NotMine);
        };
        Ok(reactor::WorkOutcome::Handled(Some(sdk::Msg {
            target: "saga".into(),
            payload: saga::encode_msg(&saga::SagaMsg::OracleResult {
                saga_id: request.saga_id,
                attempt: request.attempt,
                outcome: Ok(format!("echo: handling dispatch {}", work.dispatch_id).into_bytes()),
            }),
        })))
    }
}
