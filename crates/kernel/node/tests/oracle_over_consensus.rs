//! the async-engine milestone on the ORDERED lane: a saga driven to `Done` by an
//! oracle result that enters as an OP over the agreed total order, so EVERY
//! validator applies the identical agreed result and all converge.
//!
//! N validators, each a `host::Host` carrying one `saga` tracker module, wrapped
//! in an [`OrderedNode`] over a [`RoundOrderer`]. the flow mirrors the design's
//! oracle pattern:
//!
//! 1. a `Trigger` op is agreed -> submitted to every validator's order. draining
//!    it leaves every node's saga at `Pending` and surfaces one `WorkerRequest`
//!    effect per node (the host's effect sink, now readable via `take_effects`).
//! 2. ONE assigned node runs the (mock, non-deterministic-in-spirit) worker on its
//!    effect and produces an `OracleResult` OP. (rendezvous assignment is deferred;
//!    a fixed assignee suffices, and the saga's idempotent OracleResult handler is
//!    the backstop if more than one node ran it.)
//! 3. that op is agreed -> submitted to every validator's order (this models the
//!    consensus broadcast; per-node `RoundOrderer`s share no wire, exactly as the
//!    agreed-order convergence test feeds the same op-set to each node).
//! 4. draining it advances every validator's saga to `Done` on the agreed result;
//!    all converge on the byte-identical app-hash.
//!
//! this proves the OracleResult re-entered as an ORDERED OP: the worker never
//! touched saga state; it only produced a `Msg` that went through `submit`.

use std::time::Duration;

use commonware_runtime::{deterministic, Runner as _};
use host::Host;
use node::{OrderedNode, Orderer, RoundOrderer};
use saga::SagaModule;
use saga_interface::{
    decode_reply, decode_worker_request, encode_msg, encode_query, SagaMsg, SagaQuery, SagaReply,
    SagaStatus, SagaView,
};
use sdk::{Effect, Msg};

fn trigger(id: &str, spec: &[u8]) -> Msg {
    Msg { target: "saga".into(), payload: encode_msg(&SagaMsg::Trigger { saga_id: id.into(), spec: spec.to_vec() }) }
}

/// the MOCK oracle: try-decode a `WorkerRequest`, compute a stand-in result
/// (reversing the spec — a pure transform here, MODELING opaque external work),
/// and return the `OracleResult` op that carries it back through the normal path.
fn mock_worker(eff: &Effect) -> Option<Msg> {
    let wr = decode_worker_request(&eff.0).ok()?;
    let result: Vec<u8> = wr.spec.iter().rev().copied().collect();
    Some(Msg { target: "saga".into(), payload: encode_msg(&SagaMsg::OracleResult { saga_id: wr.saga_id, result }) })
}

async fn drain_fixpoint<O: Orderer>(n: &mut OrderedNode<O>) {
    loop {
        if n.drain_delivered().await.expect("drain") == 0 {
            break;
        }
    }
}

/// submit the identical (agreed) op into every validator's order.
async fn broadcast<O: Orderer>(nodes: &mut [OrderedNode<O>], origin: &[u8], seq: u64, msg: &Msg) {
    for n in nodes.iter_mut() {
        n.submit(origin, seq, msg.clone()).await.expect("submit");
    }
}

async fn saga_view<O: Orderer>(n: &OrderedNode<O>, id: &str) -> Option<SagaView> {
    let reply = n
        .host()
        .query("saga", &encode_query(&SagaQuery::Get { saga_id: id.into() }))
        .await
        .expect("saga query");
    match decode_reply(&reply).expect("decode reply") {
        SagaReply::Saga(v) => v,
    }
}

#[test]
fn oracle_result_over_consensus_converges_all_validators_to_done() {
    deterministic::Runner::timed(Duration::from_secs(60)).start(|_context| async move {
        const N: usize = 3;
        let mut nodes: Vec<OrderedNode<RoundOrderer>> = (0..N)
            .map(|_| {
                let host = Host::genesis(vec![Box::new(SagaModule::new("saga"))]).expect("genesis");
                OrderedNode::new(host, RoundOrderer::new())
            })
            .collect();

        // identical genesis -> identical app-hash on every validator.
        let genesis = nodes[0].app_hash();
        for n in &nodes {
            assert_eq!(n.app_hash(), genesis, "identical genesis -> identical app-hash");
        }

        // (1) the Trigger op is agreed -> submit to every validator's order, drain.
        broadcast(&mut nodes, b"trigger", 0, &trigger("s1", b"hello")).await;
        for n in &mut nodes {
            drain_fixpoint(n).await;
        }

        // every validator holds the saga at Pending (agreed), moved off genesis,
        // and surfaced exactly one WorkerRequest effect.
        let pending = nodes[0].app_hash();
        assert_ne!(pending, genesis, "creating the pending saga moved the app-hash off genesis");
        let effects_per_node: Vec<Vec<Effect>> =
            nodes.iter_mut().map(|n| n.take_effects()).collect();
        for (i, n) in nodes.iter().enumerate() {
            assert_eq!(n.app_hash(), pending, "all validators converge at Pending");
            assert_eq!(effects_per_node[i].len(), 1, "each node surfaced one WorkerRequest effect");
            assert_eq!(saga_view(n, "s1").await.unwrap().status, SagaStatus::Pending, "still Pending: no oracle op yet");
        }

        // (2) exactly ONE assigned node runs the worker on its effect.
        let assignee = 0;
        let oracle_op = mock_worker(&effects_per_node[assignee][0]).expect("worker claims the effect");

        // (3) the OracleResult op is agreed -> submit to every validator's order, drain.
        broadcast(&mut nodes, b"oracle:s1", 0, &oracle_op).await;
        for n in &mut nodes {
            drain_fixpoint(n).await;
        }

        // THE MILESTONE: every validator advanced to Done on the AGREED result and
        // converged on the byte-identical app-hash.
        let done = nodes[0].app_hash();
        assert_ne!(done, pending, "the oracle op moved the app-hash off Pending");
        for n in &nodes {
            assert_eq!(n.app_hash(), done, "all validators converge on the Done app-hash");
            let v = saga_view(n, "s1").await.expect("saga exists");
            assert_eq!(v.status, SagaStatus::Done, "every validator's saga is Done");
            assert_eq!(v.result, Some(b"olleh".to_vec()), "on the identical agreed oracle result");
        }
    });
}
