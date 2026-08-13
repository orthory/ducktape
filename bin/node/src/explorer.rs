use noded::projection::project_root_op;
use sdk::StateRoot;

use crate::constants::NOP_TARGET;
use crate::util::hex;

// ---------------------------------------------------------------------------
// the derived-index boot fold. consensus never depends on it: fold errors
// poison the store and log, heal errors log — recovery and the drain proceed
// identically with or without the index. the live drain's row construction
// (`project_root_op`) now lives in `noded::projection`; the fold below re-runs
// the SAME seam over journal-replayed frames so both writers stay byte-identical.
// ---------------------------------------------------------------------------

/// rebuild the explorer row for a replayed sealed frame — the boot fold's
/// equivalent of the drain's row construction, fed from the journal instead
/// of the live decode. `None` mirrors the drain's gates exactly: an
/// undecodable frame never had a row (its drain `op` was `None`), the
/// heartbeat nop is the deliberately-empty block the explorer hides, and a
/// discarded frame is never journaled (the arm keeps this total anyway).
pub(crate) fn sealed_frame_block_row(
    blobs: &dyn blobstore::Blobs,
    block: &recovery::FoldedBlock<'_>,
) -> Option<Vec<u8>> {
    // the sealed frame is a BATCH: decode its members (exactly
    // like the live drain) and show each as a block op. per-member
    // dispositions/traces are not carried in the fold (recovery folds the
    // block-level disposition + aggregate trace), so a replayed op shows the
    // block disposition and an empty trace — the LIVE drain carries the full
    // per-op detail.
    let members = node::decode_batch(block.frame).ok()?;
    let disposition = match block.disposition {
        node::Disposition::Applied => noded::BlockDisposition::Applied,
        node::Disposition::Rejected => noded::BlockDisposition::Rejected,
        node::Disposition::Discarded => return None,
    };
    let mut ops = Vec::new();
    for member in &members {
        let Ok(op) = node::decode_member(member) else {
            continue;
        };
        if op.msg.target == NOP_TARGET {
            continue;
        }
        ops.push(project_root_op(
            blobs,
            &op.origin,
            &op.msg.target,
            &op.msg.payload,
            &[],
            disposition,
        ));
    }
    if ops.is_empty() {
        // a pure nop/idle block — the explorer hides it (same rule as live).
        return None;
    }
    Some(noded::block_row(&noded::BlockRecord {
        height: block.height,
        hash: noded::hex_bytes(&node::frame_id(block.frame)),
        commit_hash: hex(&block.root_hash),
        ops,
    }))
}

/// the resident's explorer row: a followed BOUNDARY, not a sealed frame. the
/// populated fields are verified truth — the boundary height and the
/// root-hash the manifest check passed — and every frame-derived field stays
/// honestly empty, because a resident never sees the frames between
/// boundaries (the same degradation rule that keeps the frameless daemon
/// lane's `hash` empty rather than fabricated).
pub(crate) fn boundary_block_row(height: u64, root_hash: &StateRoot) -> Vec<u8> {
    noded::block_row(&noded::BlockRecord {
        height,
        hash: String::new(),
        commit_hash: hex(root_hash),
        // a resident follows boundaries, not frames: no member ops to show.
        ops: Vec::new(),
    })
}

/// folds sealed blocks into the derived per-module index during boot (journal
/// replay + post-reboot frame catch-up), with the GAP DISCIPLINE: once one
/// sealed height's content is unreproducible (opaque) above some module's
/// watermark, folding stops for good. advancing watermarks past the hole
/// would hide it from the post-boot heal, which re-derives from verified
/// state exactly when a watermark trails the boot tip. a re-executed block
/// carries its sealed frame, so the fold also rebuilds the explorer row the
/// live drain wrote — the blocks database is the one derived tier a
/// from-state rebuild can NOT repair (rows are node-layer observations, not
/// canonical state), so the crash-window suffix must be re-derived here or
/// `GET /v1/blocks` loses those heights for good.
pub(crate) struct IndexFold<'a> {
    index: &'a indexer::IndexStore,
    blobs: std::sync::Arc<dyn blobstore::Blobs>,
    stopped: bool,
}

impl<'a> IndexFold<'a> {
    pub(crate) fn new(
        index: &'a indexer::IndexStore,
        blobs: std::sync::Arc<dyn blobstore::Blobs>,
    ) -> Self {
        Self {
            index,
            blobs,
            stopped: false,
        }
    }

    /// the LOWEST module watermark: an opaque height at or below it is
    /// already reflected everywhere; above it, at least one module would be
    /// folded past a hole.
    fn min_watermark(&self) -> Option<u64> {
        let mut min: Option<u64> = None;
        for id in self.index.module_ids() {
            match self.index.applied_height(id) {
                Ok(h) => min = Some(min.map_or(h, |m| m.min(h))),
                Err(_) => return None,
            }
        }
        min
    }
}

impl recovery::ReplaySink for IndexFold<'_> {
    fn folded_block(&mut self, block: &recovery::FoldedBlock<'_>) {
        if self.stopped {
            return;
        }
        let height = block.height;
        let ops = indexer::BlockOps {
            record: sealed_frame_block_row(&*self.blobs, block),
            // the validator's consensus time IS the height (see BlockContext).
            ..noded::index_block_ops(height, height, block.dispatches)
        };
        if let Err(err) = self.index.apply_block(&ops) {
            tracing::error!(
                target: "ducktape::modules",
                event = "node_index_poisoned",
                height,
                error = %err,
                "module index fold stopped"
            );
            self.stopped = true;
        }
    }

    fn opaque_block(&mut self, height: u64) {
        if self.stopped {
            return;
        }
        match self.min_watermark() {
            Some(watermark) if height <= watermark => {}
            _ => self.stopped = true,
        }
    }
}

/// stamp every index module whose watermark trails `boundary` as backfilled
/// (every boot caller sits after a root/root-hash check; history below the
/// boundary re-enters only by replaying blocks through the feed or by the
/// op-row backfill below). failures poison the store and log; the node boots
/// regardless. returns the stamped module ids.
pub(crate) fn heal_index(index: &indexer::IndexStore, boundary: u64, label: &str) -> Vec<String> {
    match noded::stamp_stale_modules(index, boundary) {
        Ok(stamped) => {
            for module in &stamped {
                tracing::info!(
                    target: "ducktape::modules",
                    node = %label,
                    module,
                    height = boundary,
                    "index for {module} stamped backfilled at height {boundary}"
                );
            }
            stamped
        }
        Err(err) => {
            tracing::error!(
                target: "ducktape::modules",
                event = "node_index_poisoned",
                node = %label,
                height = boundary,
                error = %err,
                "index heal failed; wipe <storage>/index to rebuild"
            );
            Vec::new()
        }
    }
}

/// stamp the derived index at `boundary`, then BACKFILL every stamped module's
/// op rows from the sync source — inline, at the join seam, before this node
/// serves anything (indexable spec §7).
///
/// # why this is safe exactly here and nowhere else
///
/// pre-serving there are no live block folds, no ws subscribers, and no view
/// readers on this node. so an ascending fetch-and-write makes COMMIT ORDER
/// EQUAL KEY ORDER, the changes-mode fold trigger the heal just re-registered
/// folds every row correctly as it lands, and the fold tip advances
/// monotonically to the last backfilled row. no refold is needed, and none is
/// available: this is the only window where the invariant holds.
///
/// per-module failure — network, a source that cannot cover the range, a page
/// that fails structural validation — leaves that module's stamped floor
/// standing, which is today's honest behavior. the join NEVER aborts on it.
pub(crate) async fn heal_and_backfill_index<C: statesync::SyncClient>(
    index: &indexer::IndexStore,
    client: &C,
    boundary: u64,
    label: &str,
) {
    let stamped = heal_index(index, boundary, label);
    let mut backfilled: Vec<Backfilled> = Vec::new();
    for module in &stamped {
        if let Some(done) = backfill_module(index, client, module, boundary, label).await {
            backfilled.push(done);
        }
    }
    if backfilled.is_empty() {
        return;
    }
    // ONE drain for the whole set: the trigger runner folds on a background
    // thread, and the floor must not drop until the rows it vouches for are
    // derived. this BLOCKS the calling thread by design — acceptable only
    // because we are pre-serving: nothing else on this node is folding,
    // reading, or waiting on a view while it runs.
    if let Err(err) = index.wait_folds_drained() {
        tracing::warn!(
            target: "ducktape::statesync",
            node = %label,
            error = %err,
            reason = "backfill_fold_stuck",
            "index backfill folded incompletely; boundary floors stand"
        );
        return;
    }
    for Backfilled {
        module,
        source_floor,
        last_row,
    } in backfilled
    {
        // the fold has to have CONSUMED what we wrote before the floor may
        // claim it. a module with no folding guest never has a tip, and none
        // is expected of it.
        let folds = index.fold_status(&module).ok().flatten().is_some();
        let derived = !folds
            || last_row
                .is_none_or(|last| matches!(index.fold_tip(&module), Ok(Some(tip)) if tip >= last));
        if !derived {
            tracing::warn!(
                target: "ducktape::statesync",
                node = %label,
                module = %module,
                reason = "backfill_tip_behind",
                "index backfill rows are not folded; boundary floor stands"
            );
            continue;
        }
        if let Err(err) = index.set_backfill_floor(&module, source_floor) {
            tracing::warn!(
                target: "ducktape::statesync",
                node = %label,
                module = %module,
                error = %err,
                reason = "backfill_floor_refused",
                "index backfill floor not lowered"
            );
        }
    }
}

/// one module whose op rows all landed: what the source said its floor was,
/// and the last row position written (`None` for a module with no history
/// below the boundary — nothing for the fold to consume).
struct Backfilled {
    module: String,
    source_floor: Option<u64>,
    last_row: Option<(u64, u32)>,
}

/// walk one module's op rows below `boundary` off the source and write them.
/// `None` when the module keeps its stamped floor, warned once with a stable
/// reason token.
async fn backfill_module<C: statesync::SyncClient>(
    index: &indexer::IndexStore,
    client: &C,
    module: &str,
    boundary: u64,
    label: &str,
) -> Option<Backfilled> {
    let mut rows = 0usize;
    let mut bytes = 0usize;
    let mut last: Option<(u64, u32)> = None;
    let mut write_failed: Option<String> = None;
    let walked = statesync::fetch_index_ops(client, module, boundary, |page| {
        index.write_backfill_rows(module, page).map_err(|e| {
            write_failed = Some(e.to_string());
            e.to_string()
        })?;
        rows += page.len();
        bytes += page.iter().map(|(k, v)| k.len() + v.len()).sum::<usize>();
        last = page
            .last()
            .and_then(|(key, _)| indexer::parse_op_key(key.as_bytes()));
        tracing::debug!(
            target: "ducktape::statesync",
            node = %label,
            module = %module,
            rows,
            bytes,
            "index backfill page written"
        );
        Ok(())
    })
    .await;
    let source_floor = match walked {
        Ok(floor) => floor,
        Err(err) => {
            // a write failure and a wire failure differ in what an operator
            // does next, so they get their own reason tokens.
            let reason = match write_failed.take() {
                Some(_) => "backfill_write_failed",
                None => "backfill_fetch_failed",
            };
            tracing::warn!(
                target: "ducktape::statesync",
                node = %label,
                module = %module,
                height = boundary,
                error = %err,
                reason,
                "index backfill refused; the module keeps its boundary floor"
            );
            return None;
        }
    };
    tracing::info!(
        target: "ducktape::statesync",
        event = "index_backfill_complete",
        node = %label,
        module = %module,
        height = boundary,
        rows,
        bytes,
        floor = source_floor.unwrap_or(0),
        "index backfill wrote {rows} op rows for {module} below boundary {boundary}"
    );
    Some(Backfilled {
        module: module.to_string(),
        source_floor,
        last_row: last,
    })
}
