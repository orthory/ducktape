//! Env is wired from the agreed finalized view — NOT hardcoded.
//!
//! a probe module records the [`sdk::Env`] it sees on every dispatch. driven
//! through [`OrderedNode`] over the deterministic [`RoundOrderer`], it proves:
//!
//! - `height` / `consensus_time` come from the agreed view (a later-finalized op
//!   sees a STRICTLY higher height than an earlier one), and they are equal (the
//!   logical agreed clock);
//! - every validator AGREES on the height/time a given op saw (determinism);
//! - `origin` is the op's REAL submitter (`Origin::External(frame.origin)`), not
//!   the old hardcoded empty external origin.

use std::sync::{Arc, Mutex};

use node::{OrderedNode, RoundOrderer};
use sdk::{Ctx, Env, Error, Module, ModuleId, Msg, Origin, StateRoot};

/// what the probe saw for one dispatch, keyed by the op payload.
#[derive(Clone, Debug, PartialEq)]
struct Seen {
    height: u64,
    consensus_time: u64,
    origin: Vec<u8>,
}

/// a stateless module that records the `Env` of every op routed to it into a
/// shared log (keyed by payload). its root never moves — it exists only to
/// observe the environment the host stamps.
#[derive(Clone)]
struct Probe {
    log: Arc<Mutex<Vec<(Vec<u8>, Seen)>>>,
}

#[async_trait::async_trait(?Send)]
impl Module for Probe {
    fn id(&self) -> ModuleId {
        "probe".into()
    }
    fn root(&self) -> StateRoot {
        StateRoot::ZERO
    }
    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let env: &Env = ctx.env();
        let origin = match &env.origin {
            Origin::External(bytes) => bytes.clone(),
            other => panic!("root op origin must be External, got {other:?}"),
        };
        self.log.lock().unwrap().push((
            msg.payload.clone(),
            Seen {
                height: env.height,
                consensus_time: env.consensus_time,
                origin,
            },
        ));
        Ok(())
    }
}

fn probe_host(log: Arc<Mutex<Vec<(Vec<u8>, Seen)>>>) -> host::Host {
    host::Host::genesis(vec![Box::new(Probe { log })]).expect("genesis")
}

fn op(payload: &[u8]) -> Msg {
    Msg {
        target: "probe".into(),
        payload: payload.to_vec(),
    }
}

async fn drain_to_fixpoint(node: &mut OrderedNode<RoundOrderer>) {
    while node.drain_delivered().await.expect("drain") != 0 {}
}

#[test]
fn env_reflects_agreed_view_height_time_and_real_origin() {
    futures::executor::block_on(async {
        const N: usize = 3;

        // N validators, each with its own probe log.
        let mut logs: Vec<Arc<Mutex<Vec<(Vec<u8>, Seen)>>>> = Vec::new();
        let mut nodes: Vec<OrderedNode<RoundOrderer>> = Vec::new();
        for _ in 0..N {
            let log = Arc::new(Mutex::new(Vec::new()));
            nodes.push(OrderedNode::new(
                probe_host(log.clone()),
                RoundOrderer::new(),
            ));
            logs.push(log);
        }

        // PHASE 1: every validator proposes the SAME op (submitter "alice"),
        // then drains. one frame per round -> agreed view 0.
        for node in nodes.iter_mut() {
            node.submit(&sk(10), 0, op(b"first")).await.expect("submit");
            drain_to_fixpoint(node).await;
        }

        // PHASE 2: a LATER op (submitter "bob") -> agreed view 1 (strictly higher).
        for node in nodes.iter_mut() {
            node.submit(&sk(11), 0, op(b"second"))
                .await
                .expect("submit");
            drain_to_fixpoint(node).await;
        }

        // each validator saw exactly the two ops in order.
        let v0 = logs[0].lock().unwrap().clone();
        assert_eq!(v0.len(), 2, "probe saw both ops");

        let (p0, s0) = &v0[0];
        let (p1, s1) = &v0[1];
        assert_eq!(p0, b"first");
        assert_eq!(p1, b"second");

        // height/consensus_time come from the agreed view: NOT hardcoded 0, and a
        // later-finalized op sees a STRICTLY higher height.
        assert_eq!(s0.height, 0, "first op finalized at view 0");
        assert_eq!(s1.height, 1, "later op finalized at a higher view");
        assert!(s1.height > s0.height, "later view -> higher height");

        // consensus_time IS the view (logical agreed clock) — equal to height.
        assert_eq!(s0.consensus_time, s0.height);
        assert_eq!(s1.consensus_time, s1.height);

        // origin is the REAL submitter — the VERIFIED ed25519 public key the
        // frame was signed with, not caller-chosen bytes.
        assert_eq!(
            s0.origin,
            pk(10),
            "root origin is the verified submitter key"
        );
        assert_eq!(s1.origin, pk(11));

        // DETERMINISM: every validator agrees on what each op saw.
        for log in &logs {
            assert_eq!(
                *log.lock().unwrap(),
                v0,
                "all validators agree on Env per op"
            );
        }
    });
}

/// a deterministic dev signer for test frames (any u64 seed).
fn sk(seed: u64) -> commonware_cryptography::ed25519::PrivateKey {
    use commonware_cryptography::Signer as _;
    commonware_cryptography::ed25519::PrivateKey::from_seed(seed)
}

/// the public-key bytes frames signed by `sk(seed)` carry as their origin.
fn pk(seed: u64) -> Vec<u8> {
    use commonware_cryptography::Signer as _;
    sk(seed).public_key().as_ref().to_vec()
}
