//! the block-projection seam: RootOp assembly, explorer-row bytes, and the
//! member-then-System ordering shared by every block-apply lane.
//!
//! today's callers are the validator drain and the replica park loop; later
//! campaign tasks route the noded submit lane and simnode through the same
//! seam. This is the ONE place a block's decoded contents become an explorer
//! row, so a row reads byte-identically regardless of which lane wrote it —
//! pinned by the golden test below. The seam owns assembly + row bytes only;
//! each lane keeps its own index feed, stream publish, metrics, and receipt
//! shaping (those diverge by role — validator receipts, replica seal checks,
//! per-lane stream cadence — and are not part of the projection).

use sdk::{Origin, StateRoot};

use crate::blobs::BlobHandle;
use crate::{
    BlockDisposition, BlockRecord, DispatchInfo, RootOp, block_row, hex_bytes, hex_root,
    index_block_ops, payload_preview,
};

/// the idle-chain heartbeat filler's target — a module that deliberately does
/// not exist, so the nop rejects identically on every validator and leaves no
/// state. the projection hides a block whose only op is this nop. the heartbeat
/// SUBMITS with this exact target (`bin/node`'s constants re-export it), so the
/// submit and this filter can never drift.
pub const NOP_TARGET: &str = "consensus.nop";

/// build one explorer row op ([`RootOp`]) from a block member's decoded parts —
/// THE RootOp assembly seam, shared by the live drain, the boot fold, and (as
/// later tasks adopt it) the noded submit lane and simnode, so every writer
/// produces byte-identical ops. staging the payload IS computing `op_hash`
/// (put_chunk keys the blob by sha256), and on the fold path the re-staging is
/// load-bearing: the blob store is in-memory, so the live drain's staging dies
/// with the process and this is what makes `GET /v1/files/blob/{op_hash}`
/// answer again after a restart.
///
/// a dispatch's origin renders through [`DispatchInfo`] — `Origin::External`
/// flattens to plain `"external"` (the block-level `proposer` already carries
/// the key). that is the ONE shape; the noded submit lane's old `external:<name>`
/// variant is retired as it adopts this seam.
///
/// takes `&dyn Blobs` (not the concrete handle) so the boot-fold path, which
/// holds only a trait object, feeds the same seam.
pub fn project_root_op(
    blobs: &dyn blobstore::Blobs,
    origin: &Origin,
    target: &str,
    payload: &[u8],
    dispatches: &[host::DispatchRecord],
    disposition: BlockDisposition,
) -> RootOp {
    RootOp {
        proposer: match origin {
            Origin::External(key) => hex_bytes(key),
            // frames only carry verified External authorship; label the
            // impossible rest.
            Origin::Module(id) => format!("module:{id}"),
            Origin::System => "system".into(),
        },
        disposition,
        target: target.to_string(),
        operations: dispatches.iter().map(DispatchInfo::from).collect(),
        payload: payload_preview(payload),
        op_hash: hex_bytes(&blobs.put_chunk(payload.to_vec())),
    }
}

/// one finalized block's projection: the per-block facts both role loops fold
/// into metrics, receipts, and seal checks, plus the explorer row bytes to feed
/// the derived index (`record` — [`None`] for an idle/nop or discarded-only
/// block the explorer hides).
pub struct BlockProjection {
    pub height: u64,
    pub dispatches: Vec<host::DispatchRecord>,
    pub record: Option<Vec<u8>>,
    pub sealed_hash: Option<StateRoot>,
    pub applied: bool,
    pub latency_us: u64,
    pub applied_ops: usize,
    pub rejected_ops: usize,
}

/// Group a drain's per-frame outcomes into the per-block projections consumed
/// by both role loops. Member dispatches precede System dispatches, matching
/// live indexing and replay; discarded frames retain an empty projection.
pub fn project_block(
    drained: &[node::DrainedFrame],
    system_dispatches: Vec<(u64, Vec<host::DispatchRecord>)>,
    blobs: &BlobHandle,
) -> Vec<BlockProjection> {
    let mut system_dispatches: std::collections::BTreeMap<_, _> =
        system_dispatches.into_iter().collect();
    let mut projections = Vec::new();
    let mut i = 0;
    while i < drained.len() {
        let height = drained[i].height;
        let mut dispatches = Vec::new();
        let mut latency_us = 0u64;
        let mut applied = false;
        let mut ops = Vec::new();
        let mut applied_ops = 0usize;
        let mut rejected_ops = 0usize;
        let mut block_hash = None;
        let mut block_root_hash = None;
        let mut sealed_hash = None;
        while i < drained.len() && drained[i].height == height {
            let frame = &drained[i];
            i += 1;
            if frame.disposition == node::Disposition::Discarded {
                continue;
            }
            sealed_hash = Some(frame.root_hash);
            if let (node::Disposition::Applied, Some(op)) = (&frame.disposition, &frame.op) {
                applied = true;
                latency_us = latency_us.saturating_add(op.latency_us);
                dispatches.extend(op.dispatches.iter().cloned());
            }
            // the envelope's released continuation: its dispatches join the
            // block's op stream right after its parent's (the host's event
            // order), INDEPENDENT of the parent's disposition — a rejected
            // parent still releases, and an applied continuation is real work.
            if let Some(cont) = frame.op.as_ref().and_then(|op| op.continuation.as_ref())
                && cont.disposition == node::Disposition::Applied
            {
                applied = true;
                dispatches.extend(cont.dispatches.iter().cloned());
            }
            if let Some(op) = &frame.op
                && op.target != NOP_TARGET
            {
                let disposition = match frame.disposition {
                    node::Disposition::Applied => {
                        applied_ops += 1;
                        BlockDisposition::Applied
                    }
                    node::Disposition::Rejected => {
                        rejected_ops += 1;
                        BlockDisposition::Rejected
                    }
                    node::Disposition::Discarded => continue,
                };
                if block_hash.is_none() {
                    block_hash = Some(frame.id);
                    block_root_hash = Some(frame.root_hash);
                }
                ops.push(project_root_op(
                    blobs,
                    &op.origin,
                    &op.target,
                    &op.payload,
                    &op.dispatches,
                    disposition,
                ));
                // the continuation is its own row, right after its parent:
                // `Origin::Module(parent_target)` is the sending lane, and its
                // own disposition — not the parent's — is the row's.
                if let Some(cont) = &op.continuation {
                    let cont_disposition = match cont.disposition {
                        node::Disposition::Applied => {
                            applied_ops += 1;
                            BlockDisposition::Applied
                        }
                        _ => {
                            rejected_ops += 1;
                            BlockDisposition::Rejected
                        }
                    };
                    ops.push(project_root_op(
                        blobs,
                        &Origin::Module(op.target.clone()),
                        &cont.target,
                        &cont.payload,
                        &cont.dispatches,
                        cont_disposition,
                    ));
                }
            }
        }
        if let Some(system) = system_dispatches.remove(&height) {
            dispatches.extend(system);
        }
        let record = (!ops.is_empty()).then(|| {
            block_row(&BlockRecord {
                height,
                hash: block_hash.map(|hash| hex_bytes(&hash)).unwrap_or_default(),
                commit_hash: block_root_hash.map(|hash| hex_root(&hash)).unwrap_or_default(),
                ops,
            })
        });
        projections.push(BlockProjection {
            height,
            dispatches,
            record,
            sealed_hash,
            applied,
            latency_us,
            applied_ops,
            rejected_ops,
        });
    }
    projections
}

/// Fold one finalized block into the derived per-module index — the shared
/// epilogue of every block-apply lane (validator drain, embedded daemon submit,
/// sim). Merges the lane's explorer `record` onto [`index_block_ops`] and
/// applies it, logging the ONE STALE-index error they share on failure.
///
/// The derived index is a READ MODEL: a failure here degrades the app's views
/// (every module view the app reads is served from it, and it does not
/// self-heal), NEVER consensus — canonical state is already sealed, so the block
/// stands regardless. Hence loud-but-non-fatal. `event` is the operational
/// contract (#603); `target` is the filtering plane — orthogonal, carry both.
pub fn apply_block_to_index(
    index: &indexer::IndexStore,
    height: u64,
    consensus_time: u64,
    record: Option<Vec<u8>>,
    dispatches: &[host::DispatchRecord],
) {
    let ops = indexer::BlockOps {
        record,
        ..index_block_ops(height, consensus_time, dispatches)
    };
    if let Err(err) = index.apply_block(&ops) {
        tracing::error!(
            target: "ducktape::consensus",
            event = "node_index_poisoned",
            height,
            error = %err,
            "module index apply failed — the app's views are now STALE; wipe \
             <storage>/index to rebuild"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dispatch(module: &str, payload: &[u8]) -> host::DispatchRecord {
        host::DispatchRecord {
            module: module.into(),
            origin: Origin::Module("source".into()),
            payload: payload.to_vec(),
            emitted_msgs: 0,
            emitted_events: 0,
        }
    }

    fn drained(
        id: u8,
        height: u64,
        disposition: node::Disposition,
        root: u8,
        op: Option<node::DrainedOp>,
    ) -> node::DrainedFrame {
        node::DrainedFrame {
            id: [id; 32],
            height,
            disposition,
            root_hash: StateRoot([root; sdk::ROOT_LEN]),
            op,
            reason: None,
        }
    }

    fn cont(
        target: &str,
        payload: &[u8],
        disposition: node::Disposition,
        dispatches: Vec<host::DispatchRecord>,
    ) -> node::DrainedContinuation {
        node::DrainedContinuation {
            target: target.into(),
            payload: payload.to_vec(),
            disposition,
            dispatches,
            reason: None,
        }
    }

    /// GOLDEN: the exact `block_row` bytes this seam produces, captured from the
    /// pre-extraction validator path (`bin/node`'s `block_actions`). Any change
    /// to the projection that shifts these bytes is a wire change to
    /// `GET /v1/blocks` and must be treated as one. Covers: applied member,
    /// rejected member, System dispatch fold, multi-member block, envelope
    /// continuation row ordering, and the two empty-batch-no-row shapes
    /// (nop-only + System dispatch, and discarded-only).
    #[test]
    fn project_block_row_bytes_are_golden() {
        let blobs = BlobHandle::default();
        let member = dispatch("chat", b"member");
        let system = dispatch("lifecycle", b"system");
        let frames = vec![
            // multi-member block: applied + rejected members, plus a System
            // dispatch folded into the block's dispatch stream (never a row).
            drained(
                1,
                7,
                node::Disposition::Applied,
                9,
                Some(node::DrainedOp {
                    origin: Origin::External(vec![1, 2, 3]),
                    target: "chat".into(),
                    payload: b"hello world".to_vec(),
                    dispatches: vec![member.clone()],
                    latency_us: 11,
                    continuation: None,
                }),
            ),
            drained(
                2,
                7,
                node::Disposition::Rejected,
                9,
                Some(node::DrainedOp {
                    origin: Origin::External(vec![4, 5, 6]),
                    target: "chat".into(),
                    payload: b"nope".to_vec(),
                    dispatches: Vec::new(),
                    latency_us: 99,
                    continuation: None,
                }),
            ),
            // applied member carrying an applied continuation (an envelope): the
            // continuation is its own row, right after its parent.
            drained(
                4,
                8,
                node::Disposition::Applied,
                10,
                Some(node::DrainedOp {
                    origin: Origin::External(vec![7]),
                    target: "tasks".into(),
                    payload: b"parent".to_vec(),
                    dispatches: vec![member.clone()],
                    latency_us: 3,
                    continuation: Some(cont(
                        "pages",
                        b"child",
                        node::Disposition::Applied,
                        vec![dispatch("pages", b"cont")],
                    )),
                }),
            ),
            // empty-batch-no-row: a nop-only height that still folds a System
            // dispatch — no explorer row, but the dispatch advances the index.
            drained(
                5,
                9,
                node::Disposition::Rejected,
                11,
                Some(node::DrainedOp {
                    origin: Origin::External(vec![8]),
                    target: NOP_TARGET.into(),
                    payload: Vec::new(),
                    dispatches: Vec::new(),
                    latency_us: 0,
                    continuation: None,
                }),
            ),
            // discarded-only height: no row either.
            drained(6, 10, node::Disposition::Discarded, 12, None),
        ];

        let projections = project_block(
            &frames,
            vec![(7, vec![system.clone()]), (9, vec![system.clone()])],
            &blobs,
        );

        let rows: Vec<(u64, Option<String>)> = projections
            .iter()
            .map(|p| {
                (
                    p.height,
                    p.record
                        .as_deref()
                        .map(|b| std::str::from_utf8(b).unwrap().to_string()),
                )
            })
            .collect();

        assert_eq!(
            rows,
            vec![
                (7, Some(r#"{"height":7,"hash":"0101010101010101010101010101010101010101010101010101010101010101","commit_hash":"0909090909090909090909090909090909090909090909090909090909090909","ops":[{"proposer":"010203","disposition":"applied","target":"chat","operations":[{"module":"chat","origin":"module:source","emitted_msgs":0,"emitted_events":0}],"payload":"hello world","op_hash":"b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"},{"proposer":"040506","disposition":"rejected","target":"chat","operations":[],"payload":"nope","op_hash":"ca3704aa0b06f5954c79ee837faa152d84d6b2d42838f0637a15eda8337dbdce"}]}"#.to_string())),
                (8, Some(r#"{"height":8,"hash":"0404040404040404040404040404040404040404040404040404040404040404","commit_hash":"0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a","ops":[{"proposer":"07","disposition":"applied","target":"tasks","operations":[{"module":"chat","origin":"module:source","emitted_msgs":0,"emitted_events":0}],"payload":"parent","op_hash":"e47125968b3b71049fbc4802d1e40a71ea1359decfabacf70b34588037d4ff0c"},{"proposer":"module:tasks","disposition":"applied","target":"pages","operations":[{"module":"pages","origin":"module:source","emitted_msgs":0,"emitted_events":0}],"payload":"child","op_hash":"ddc9e669194254cef019a29d3619a2c16592e5d52e1a81e98b01bd52319149a3"}]}"#.to_string())),
                (9, None),
                (10, None),
            ]
        );

        // the non-row facts the role loops fold: the System dispatch rides the
        // block's dispatch stream after the members; the nop-only height (9)
        // still carries its System dispatch; discarded (10) carries nothing.
        assert_eq!(projections[0].dispatches, vec![member.clone(), system.clone()]);
        assert_eq!((projections[0].applied_ops, projections[0].rejected_ops), (1, 1));
        assert!(projections[0].applied);
        assert_eq!(projections[0].latency_us, 11);
        assert_eq!(projections[1].dispatches, vec![member.clone(), dispatch("pages", b"cont")]);
        assert_eq!(projections[2].dispatches, vec![system]);
        assert!(!projections[2].applied);
        assert!(projections[3].dispatches.is_empty());
        assert_eq!(projections[3].sealed_hash, None);
    }
}
