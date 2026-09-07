//! the ordered gate applies in AGREED HEIGHT order, always.
//!
//! a restarted (or mid-epoch joining) validator's engine re-reports its
//! journal tip FIRST and backfills the gap views below it. the inbox releases
//! by view, so the node sees them ascending — and above the resume floor the
//! node REFUSES a height it has already journaled rather than composing an
//! op-log-ordered qmdb root no peer can reproduce.

use directory::Directory;
use directory::{DirMsg, DirQuery, DirReply, decode_reply, encode_msg, encode_query};
use futures::executor::block_on;
use host::Host;
use node::{Error, NullSink, OrderedNode, Orderer, encode_batch, encode_frame};
use sdk::Msg;

/// delivers exactly the `(view, frame)` script it was built with — the seam a
/// real engine's re-report stream drives.
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

fn set(key: &str, value: &str) -> Msg {
    Msg {
        target: "directory".into(),
        payload: encode_msg(&DirMsg::Set {
            key: key.into(),
            value: value.into(),
        }),
    }
}

/// one batch super-frame carrying one signed `set`.
fn batch(seq: u64, key: &str, value: &str) -> Vec<u8> {
    encode_batch(&[encode_frame(&sk(1), seq, &set(key, value))])
}

fn resumed(script: Vec<(u64, Vec<u8>)>, at: u64) -> OrderedNode<ScriptedOrderer, NullSink> {
    let host = Host::genesis(vec![Box::new(Directory::new("directory"))]).expect("genesis");
    let root_hash = host.root_hash();
    OrderedNode::resume(
        host,
        ScriptedOrderer { script },
        NullSink,
        Some(host::FinalizedBlock {
            height: at,
            root_hash,
        }),
        0,
    )
}

async fn get(node: &OrderedNode<ScriptedOrderer, NullSink>, key: &str) -> Option<String> {
    let reply = node
        .host()
        .query(
            "directory",
            &encode_query(&DirQuery::Get { key: key.into() }),
        )
        .await
        .expect("query");
    match decode_reply(&reply).expect("decode") {
        DirReply::Value(v) => v,
    }
}

#[test]
fn a_resume_applies_journaled_finalizations_above_the_floor_ascending() {
    block_on(async {
        // recovered at height 40; views 41..=44 finalized pre-crash but never
        // drained. the inbox releases them ascending, so they apply ascending.
        let script = (41..=44)
            .map(|view| (view, batch(view, &format!("k{view}"), &format!("v{view}"))))
            .collect();
        let mut node = resumed(script, 40);
        assert_eq!(node.drain_delivered().await.expect("drain"), 4);
        let heights: Vec<u64> = node.take_drained().iter().map(|d| d.height).collect();
        assert_eq!(
            heights,
            vec![41, 42, 43, 44],
            "every block above the resume floor applies, in ascending height order"
        );
        for view in 41..=44u64 {
            assert_eq!(
                get(&node, &format!("k{view}")).await.as_deref(),
                Some(format!("v{view}").as_str())
            );
        }
        assert_eq!(node.finalized().expect("boundary").height, 44);
    });
}

#[test]
fn a_re_reported_height_at_or_below_the_resume_floor_is_skipped() {
    block_on(async {
        // views 39 and 40 are recovered history the reopened engine re-reports:
        // a deterministic no-op everywhere, not an error.
        let script = vec![
            (39, batch(39, "old", "already durable")),
            (40, batch(40, "tip", "already durable")),
            (41, batch(41, "new", "fresh")),
        ];
        let mut node = resumed(script, 40);
        assert_eq!(node.drain_delivered().await.expect("drain"), 3);
        let heights: Vec<u64> = node.take_drained().iter().map(|d| d.height).collect();
        assert_eq!(heights, vec![41], "only the height above the floor applies");
        assert_eq!(get(&node, "old").await, None);
        assert_eq!(get(&node, "new").await.as_deref(), Some("fresh"));
    });
}

#[test]
fn a_delivery_below_an_already_applied_height_is_refused_not_skipped() {
    block_on(async {
        // the tip-then-backfill order, delivered raw. 44 journals first; 41 is
        // then BELOW what this process applied — applying it composes a root no
        // peer can reproduce, and skipping it drops a block every peer applied.
        let script = vec![(44, batch(44, "k44", "v44")), (41, batch(41, "k41", "v41"))];
        let mut node = resumed(script, 40);
        let err = node.drain_delivered().await.expect_err("must refuse");
        match err {
            Error::OutOfOrder { height, applied } => {
                assert_eq!((height, applied), (41, 44), "the error names both heights");
            }
            other => panic!("expected OutOfOrder, got {other}"),
        }
        assert_eq!(
            get(&node, "k41").await,
            None,
            "the out-of-order block never applied"
        );
    });
}
