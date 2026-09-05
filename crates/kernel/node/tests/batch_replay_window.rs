//! the batch REPLAY WINDOW: a finalized batch that already applied cannot
//! apply again inside the window, whatever the epoch or the process.
//!
//! consensus cannot catch this — a validator votes for any digest whose bytes
//! it holds, so anyone keeping a finalized batch can re-propose it
//! byte-identically and have it finalize at a NEW height. the refusal lives in
//! the apply path, on a protocol constant, so every validator reaches the same
//! verdict at the same block.

use directory::Directory;
use directory::{DirMsg, DirQuery, DirReply, decode_reply, encode_msg, encode_query};
use futures::executor::block_on;
use host::Host;
use node::{Error, NullSink, OrderedNode, Orderer, encode_batch, encode_frame, frame_id};
use sdk::Msg;

struct ScriptedOrderer {
    script: Vec<(u64, Vec<u8>)>,
}

impl Orderer for ScriptedOrderer {
    async fn submit(&mut self, _frame: Vec<u8>) -> Result<(), Error> {
        Ok(())
    }

    fn poll_delivered(&mut self) -> Vec<(u64, Vec<u8>)> {
        std::mem::take(&mut self.script)
    }
}

fn sk(seed: u64) -> commonware_cryptography::ed25519::PrivateKey {
    use commonware_cryptography::Signer as _;
    commonware_cryptography::ed25519::PrivateKey::from_seed(seed)
}

/// a batch super-frame carrying one signed `append` to the same key.
fn batch(seq: u64, value: &str) -> Vec<u8> {
    encode_batch(&[encode_frame(
        &sk(1),
        seq,
        &Msg {
            target: "directory".into(),
            payload: encode_msg(&DirMsg::Set {
                key: "k".into(),
                value: value.into(),
            }),
        },
    )])
}

fn node_over(script: Vec<(u64, Vec<u8>)>) -> OrderedNode<ScriptedOrderer, NullSink> {
    let host = Host::genesis(vec![Box::new(Directory::new("directory"))]).expect("genesis");
    OrderedNode::with_sink(host, ScriptedOrderer { script }, NullSink)
}

async fn get(node: &OrderedNode<ScriptedOrderer, NullSink>) -> Option<String> {
    let reply = node
        .host()
        .query(
            "directory",
            &encode_query(&DirQuery::Get { key: "k".into() }),
        )
        .await
        .expect("query");
    match decode_reply(&reply).expect("decode") {
        DirReply::Value(v) => v,
    }
}

#[test]
fn a_re_finalized_batch_is_refused_at_its_new_height() {
    block_on(async {
        // the SAME batch bytes finalize twice, at two different views — the
        // replay a proposer stages out of any peer's payload cache.
        let replayed = batch(0, "first");
        let script = vec![
            (0, replayed.clone()),
            (1, batch(1, "second")),
            (2, replayed.clone()),
        ];
        let mut node = node_over(script);
        assert_eq!(node.drain_delivered().await.expect("drain"), 3);
        assert_eq!(
            get(&node).await.as_deref(),
            Some("second"),
            "the replay must not overwrite the value the later block wrote"
        );
        let drained = node.take_drained();
        let replay_record = drained
            .iter()
            .find(|d| d.height == 2)
            .expect("the replayed height still seals");
        assert_eq!(replay_record.id, frame_id(&replayed));
        assert_eq!(replay_record.disposition, node::Disposition::Rejected);
        assert_eq!(replay_record.reason.as_deref(), Some("batch replayed"));
        // the view was agreed, so the block still seals: the boundary advances.
        assert_eq!(node.finalized().expect("boundary").height, 2);
    });
}

#[test]
fn the_window_survives_an_epoch_cutover() {
    block_on(async {
        // the cutover mints a fresh orderer with a fresh content store and a
        // fresh exactly-once digest set — the guard used to die with it.
        let replayed = batch(0, "first");
        let mut node = node_over(vec![(0, replayed.clone())]);
        assert_eq!(node.drain_delivered().await.expect("drain"), 1);
        assert_eq!(get(&node).await.as_deref(), Some("first"));

        let fresh = ScriptedOrderer {
            script: vec![(0, replayed.clone())],
        };
        node.cutover(fresh, 1, 10, &[], &[]).await.expect("cutover");
        assert_eq!(node.drain_delivered().await.expect("drain"), 1);
        let drained = node.take_drained();
        assert_eq!(
            drained
                .iter()
                .filter(|d| d.reason.as_deref() == Some("batch replayed"))
                .count(),
            1,
            "the post-cutover epoch refuses the batch the pre-cutover one applied"
        );
    });
}

#[test]
fn a_seeded_window_refuses_what_a_previous_process_journaled() {
    block_on(async {
        // the restart case: recovery hands back the journal suffix's
        // (height, batch id) pairs and the resumed node keeps refusing them.
        let replayed = batch(0, "durable");
        let host = Host::genesis(vec![Box::new(Directory::new("directory"))]).expect("genesis");
        let root_hash = host.root_hash();
        let mut node = OrderedNode::resume(
            host,
            ScriptedOrderer {
                script: vec![(7, replayed.clone())],
            },
            NullSink,
            Some(host::FinalizedBlock {
                height: 5,
                root_hash,
            }),
            0,
        );
        node.seed_replay_window([(4, frame_id(&replayed))]);
        assert_eq!(node.drain_delivered().await.expect("drain"), 1);
        assert_eq!(
            get(&node).await,
            None,
            "a batch the previous process journaled never applies again"
        );
        assert_eq!(
            node.take_drained()[0].reason.as_deref(),
            Some("batch replayed")
        );
    });
}
