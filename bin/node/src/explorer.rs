use noded::projection::project_root_op;
use sdk::StateRoot;

use crate::constants::{MODULE_IDS, NOP_TARGET};
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
        // an envelope's released continuation is its own op row, right after
        // its parent — the live drain's row order (`noded::projection`).
        if let Some(cont) = op.continuation {
            ops.push(project_root_op(
                blobs,
                &sdk::Origin::Module(op.msg.target),
                &cont.target,
                &cont.payload,
                &[],
                disposition,
            ));
        }
    }
    if ops.is_empty() {
        // a pure nop/idle block — the explorer hides it (same rule as live).
        return None;
    }
    Some(noded::block_row(&noded::BlockRecord {
        height: block.height,
        hash: noded::hex_bytes(&node::frame_id(block.frame)),
        commit_hash: hex(&block.app_hash),
        ops,
    }))
}

/// the resident's explorer row: a followed BOUNDARY, not a sealed frame. the
/// populated fields are verified truth — the boundary height and the
/// app-hash the manifest check passed — and every frame-derived field stays
/// honestly empty, because a resident never sees the frames between
/// boundaries (the same degradation rule that keeps the frameless daemon
/// lane's `hash` empty rather than fabricated).
pub(crate) fn boundary_block_row(height: u64, app_hash: &StateRoot) -> Vec<u8> {
    noded::block_row(&noded::BlockRecord {
        height,
        hash: String::new(),
        commit_hash: hex(app_hash),
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
/// (every boot caller sits after a root/app-hash check; history below the
/// boundary re-enters only by replaying blocks through the feed). failures
/// poison the store and log; the node boots regardless.
pub(crate) fn heal_index(index: &indexer::IndexStore, boundary: u64, label: &str) {
    match noded::stamp_stale_modules(index, boundary) {
        Ok(stamped) => {
            for module in stamped {
                tracing::info!(
                    target: "ducktape::modules",
                    node = %label,
                    module,
                    height = boundary,
                    "index for {module} stamped backfilled at height {boundary}"
                );
            }
        }
        Err(err) => tracing::error!(
            target: "ducktape::modules",
            event = "node_index_poisoned",
            node = %label,
            height = boundary,
            error = %err,
            "index heal failed; wipe <storage>/index to rebuild"
        ),
    }
}

/// cut and frame every derived-index database (modules + the blocks db) for
/// the shipped-index lane (indexable spec §7 lane 2). a database that fails
/// to cut is skipped — whatever a joiner does not receive, its staleness
/// heal re-derives — and a poisoned store cuts nothing, so the shipment
/// comes back empty and the joiner falls back entirely.
pub(crate) fn ship_index_blobs(
    index: &indexer::IndexStore,
    label: &str,
) -> std::collections::BTreeMap<String, Vec<u8>> {
    let mut blobs = std::collections::BTreeMap::new();
    let dbs: Vec<String> = index
        .module_ids()
        .map(str::to_string)
        .chain(std::iter::once(indexer::BLOCKS_DB_ID.to_string()))
        .collect();
    for db in dbs {
        match index.checkpoint_files(&db) {
            Ok(files) => {
                blobs.insert(db, statesync::encode_index_archive(&files));
            }
            Err(err) => tracing::warn!(
                target: "ducktape::statesync",
                node = %label,
                database = %db,
                error = %err,
                reason = "index_checkpoint_failed",
                "shipped index database skipped"
            ),
        }
    }
    blobs
}

/// fetch the sync source's shipped-index checkpoints and stage them for
/// adoption at the promoted reboot — the OPTIONAL, UNVERIFIED warm start
/// over the from-state rebuild (indexable spec §7 lane 2). every outcome
/// short of a staged-and-committed install converges on the same fallback:
/// the boot heal re-derives whatever the watermarks say is missing, so
/// failures here log and fall through, never abort the promotion.
pub(crate) async fn stage_shipped_index<C: statesync::SyncClient>(
    client: &C,
    boundary: statesync::BoundaryId,
    storage: &std::path::Path,
    label: &str,
) {
    let index_base = storage.join("index");
    let known: std::collections::BTreeSet<&str> = MODULE_IDS
        .iter()
        .copied()
        .chain(std::iter::once(indexer::BLOCKS_DB_ID))
        .collect();
    let staged: Result<usize, String> = async {
        // a retry of the promotion loop may have staged a partial set
        // already; start clean so attempts never interleave.
        indexer::discard_staged(&indexer::DiskFs, &index_base).map_err(|e| e.to_string())?;
        let entries = statesync::fetch_index_modules(client, boundary)
            .await
            .map_err(|e| e.to_string())?;
        let mut staged = 0usize;
        for (db, _) in &entries {
            // a db this binary does not know (version skew) would sit
            // unopened on disk forever — skip it, its module heals instead.
            if !known.contains(db.as_str()) {
                tracing::warn!(
                    target: "ducktape::statesync",
                    node = %label,
                    database = %db,
                    reason = "unknown_index_database",
                    "shipped index database skipped"
                );
                continue;
            }
            let blob = statesync::fetch_index_db(client, boundary, db)
                .await
                .map_err(|e| format!("{db}: {e}"))?;
            let files = statesync::decode_index_archive(&blob).map_err(|e| format!("{db}: {e}"))?;
            indexer::stage_shipped_db(&indexer::DiskFs, &index_base, db, &files)
                .map_err(|e| e.to_string())?;
            staged += 1;
        }
        if staged > 0 {
            indexer::commit_staged(&indexer::DiskFs, &index_base).map_err(|e| e.to_string())?;
        }
        Ok(staged)
    }
    .await;
    match staged {
        Ok(0) => tracing::info!(
            target: "ducktape::statesync",
            node = %label,
            "source ships no index; views heal from verified state"
        ),
        Ok(n) => tracing::info!(
            target: "ducktape::statesync",
            node = %label,
            databases = n,
            "shipped index staged; it will be adopted at the promoted reboot"
        ),
        Err(e) => {
            tracing::warn!(
                target: "ducktape::statesync",
                node = %label,
                error = %e,
                reason = "shipped_index_fetch_failed",
                "views will heal from verified state instead"
            );
            if let Err(e) = indexer::discard_staged(&indexer::DiskFs, &index_base) {
                tracing::warn!(
                    target: "ducktape::statesync",
                    node = %label,
                    error = %e,
                    reason = "shipped_index_cleanup_failed",
                    "shipped index staging cleanup failed"
                );
            }
        }
    }
}
