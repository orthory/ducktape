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
use dispatch_oracle::{DeliverFn, DispatchPool, SpawnFn};
use futures::SinkExt as _;
use futures::channel::mpsc;
use noded::NodeCommand;

use crate::ORACLE_ORIGIN;

/// the daemon's worker set: the dispatch pool (or, in debug builds under
/// `DUCKTAPE_NODED_ECHO_ORACLE`, the inline echo stand-in).
pub(crate) fn oracle_workers<C>(
    context: &C,
    cmds: mpsc::Sender<NodeCommand>,
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
    let providers = capability_host::discover()
        // BYO: run whatever executor CLIs the capability specs describe and
        // this host has installed — no credential handling here (see
        // docs/capability-spec.md). a broken operator spec is a boot error.
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

    vec![Box::new(DispatchPool::new(
        Arc::new(providers),
        // the daemon's oracle identity: its worker follow-ups are
        // submitted under ORACLE_ORIGIN, so an Accept claim records that
        // key as the assignee and the re-emitted request must match it.
        ORACLE_ORIGIN.to_vec(),
        spawn,
        deliver,
    ))]
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
