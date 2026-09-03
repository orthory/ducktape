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

/// bring every module whose feed trails `boundary` — or begins at a floor —
/// up to it from the sync source, inline at the seam, before this node serves
/// anything (indexable spec §7). a module that already holds a feed RESUMES
/// above its own watermark and keeps everything under it; one that holds
/// nothing usable is stamped at the boundary and pulled from the source's own
/// floor up; one whose only gap is BELOW its floor is rebuilt the same way,
/// but only when a source says it holds that history.
///
/// # why this is safe exactly here and nowhere else
///
/// EVERY call seam — the join, the resident's park loop, and both arms of the
/// validator's boot catch-up — sits on the ONE task that ever writes this
/// node's index, BEFORE it resumes folding live blocks. so nothing else commits to
/// these databases while the walk runs, an ascending fetch-and-write makes
/// COMMIT ORDER EQUAL KEY ORDER, the changes-mode fold trigger the heal just
/// re-registered folds every row correctly as it lands, and the fold tip
/// advances monotonically to the last backfilled row. no refold is needed, and
/// none is available: this is the only window where the invariant holds.
///
/// READERS are a different question, and the answer is "no worse than before":
/// on an epoch-cutover re-ascension the http/ws surfaces are still up, so a
/// view read can land mid-walk and see a partly backfilled feed. it would
/// otherwise have seen the empty one the heal's wipe just left, and the floor
/// does not drop until the fold has consumed everything.
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
    let stale = match noded::stale_modules(index, boundary) {
        Ok(stale) => stale,
        Err(err) => {
            tracing::error!(
                target: "ducktape::modules",
                event = "node_index_poisoned",
                node = %label,
                height = boundary,
                error = %err,
                "index heal failed; wipe <storage>/index to rebuild"
            );
            return;
        }
    };
    // A MODULE THAT IS NOT STALE CAN STILL BE MISSING EVERYTHING BELOW ITS
    // FLOOR — the marker a boundary stamp leaves behind. That is exactly what
    // a resident restarting over a wiped index directory holds: the journal
    // replay brings every watermark back to the tip, so nothing is stale, and
    // the history below the stamp is reachable only from a source. Listed
    // BEFORE the stale pass, whose own stamps write fresh floors nobody owes
    // a second walk.
    let mut floored: Vec<(String, u64)> = Vec::new();
    for id in index
        .module_ids()
        .filter(|id| !stale.iter().any(|stale| stale == id))
    {
        match index.backfill_height(id) {
            Ok(Some(floor)) => floored.push((id.to_string(), floor)),
            Ok(None) => {}
            Err(err) => tracing::warn!(
                target: "ducktape::statesync",
                node = %label,
                module = %id,
                error = %err,
                reason = "backfill_floor_unreadable",
                "index floor unreadable; this module's gap stays where it is"
            ),
        }
    }
    let mut backfilled: Vec<Backfilled> = Vec::new();
    for module in &stale {
        if let Some(done) = heal_module(index, client, module, boundary, label).await {
            backfilled.push(done);
            continue;
        }
        // A REFUSED MODULE IS ROUTINE; A POISONED STORE IS NOT. A write
        // failure poisons the whole IndexStore, so every later `apply_block`
        // on this node dies too and the derived tier is finished until an
        // operator rebuilds it — the per-module "keeps its boundary floor"
        // warn would badly understate that. stop asking: the remaining
        // modules can only fail the same way, N identical warns deep.
        if index.is_poisoned() {
            tracing::error!(
                target: "ducktape::modules",
                event = "node_index_poisoned",
                node = %label,
                module = %module,
                height = boundary,
                "index backfill poisoned the store; every later fold fails — \
                 wipe <storage>/index to rebuild"
            );
            return;
        }
    }
    for (module, floor) in &floored {
        if let Some(done) = close_floor(index, client, module, *floor, boundary, label).await {
            backfilled.push(done);
            continue;
        }
        if index.is_poisoned() {
            tracing::error!(
                target: "ducktape::modules",
                event = "node_index_poisoned",
                node = %label,
                module = %module,
                height = boundary,
                "index backfill poisoned the store; every later fold fails — \
                 wipe <storage>/index to rebuild"
            );
            return;
        }
    }
    if backfilled.is_empty() {
        return;
    }
    // ONE drain for the whole set: the trigger runner folds on a background
    // thread, and the floor must not drop until the rows it vouches for are
    // derived.
    //
    // this BLOCKS a runtime worker, and the honest scope of that is narrower
    // than "we are pre-serving". PRE-SERVING covers the correctness argument
    // only — no live folds, no view readers, no ws subscribers on THIS node,
    // so nothing observes a half-drained index. it does NOT cover the
    // scheduler: the mesh, sync-serve and reachability tasks share this
    // runtime's two workers and stay live throughout. the whole seam is
    // already synchronous disk work (the heal's wipe, every batch write), so
    // the drain adds no new class of stall — but it is bounded on progress
    // rather than trusted to end (`indexer::drain_fold`), because an
    // unbounded spin here would hang the join and hold half the runtime with
    // it.
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
        // is expected of it — but only a status read that SUCCEEDS says so; a
        // failed one is not evidence of anything, so it still has to show a
        // tip.
        let folds = !matches!(index.fold_status(&module), Ok(None));
        let tip_covers_rows = last_row
            .is_none_or(|last| matches!(index.fold_tip(&module), Ok(Some(tip)) if tip >= last));
        let derived = !folds || tip_covers_rows;
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

/// the op-row seq no real row carries: a watermark vouches for whole HEIGHTS,
/// so a cursor at `(watermark, AFTER_EVERY_SEQ)` names the end of that height
/// — everything at or below it is already in this node's feed.
const AFTER_EVERY_SEQ: u32 = u32::MAX;

/// one stale module: RESUME above the feed it already holds, or stamp it at
/// the boundary and pull the whole history below.
///
/// resuming is the difference between a re-stamping ascension costing one
/// delta and costing the entire op history. the module's watermark is the
/// contract for what it holds, its derived views were folded from exactly
/// those rows, and the delta lands ascending on top — so the stamp's WIPE
/// (feed and views, floored at the boundary) is the fallback, needed only
/// when nothing below can be composed: an empty feed, or a source whose own
/// history starts above the resume point.
async fn heal_module<C: statesync::SyncClient>(
    index: &indexer::IndexStore,
    client: &C,
    module: &str,
    boundary: u64,
    label: &str,
) -> Option<Backfilled> {
    if let Some(resumed) = resume_module(index, client, module, boundary, label).await {
        return Some(resumed);
    }
    stamp_module(index, module, boundary, label)?;
    backfill_module(index, client, module, boundary, None, label)
        .await
        .ok()
}

/// close a module's FLOOR: pull the history below it, when a source holds any.
///
/// THE FEED IS NEVER WIPED AHEAD OF THE PULL. This module HAS a feed; a stamp
/// would destroy it for a walk that can still fail on its next page — a source
/// that drops, a page failing the canonical seal, a source that re-stamps
/// mid-walk — and the next seam, seeing the same floor, would do it again. So
/// the rows land UNDER the feed instead, which only ever GAINS rows here.
///
/// The READ MODEL is a different matter: rows below what the fold already
/// consumed arrive out of key order by construction, so the derived keyspace
/// is cleared and re-driven from the whole feed afterwards
/// ([`indexer::IndexStore::refold`]) — whether the walk finished or died
/// holding half a range. Views are blank for the length of that replay, which
/// is the same window a mapper swap opens at boot, and the feed under them is
/// intact throughout. A walk that wrote NOTHING disturbed nothing, and skips
/// it.
///
/// Asked only when a source can actually close the gap: one empty page
/// carries the source's own floor, and a source floored no lower than this
/// node is not worth a walk.
async fn close_floor<C: statesync::SyncClient>(
    index: &indexer::IndexStore,
    client: &C,
    module: &str,
    floor: u64,
    boundary: u64,
    label: &str,
) -> Option<Backfilled> {
    if !source_reaches_below(client, module, floor, boundary).await {
        return None;
    }
    tracing::info!(
        target: "ducktape::statesync",
        node = %label,
        module,
        floor,
        height = boundary,
        "index backfill closing the floor at {floor}: the source holds history below it"
    );
    // the ceiling is the FLOOR, not the boundary: everything above it is
    // already in this node's feed, and re-fetching it would only re-fold rows
    // the views already carry.
    let closed = backfill_module(index, client, module, floor, None, label).await;
    if matches!(closed, Err(Wrote(None))) {
        // the source promised and then refused before writing a row. the read
        // model is exactly as it was, and replaying the whole feed to prove it
        // would cost every boot this seam runs — it gates `serving`.
        return None;
    }
    if let Err(err) = index.refold(module) {
        tracing::warn!(
            target: "ducktape::statesync",
            node = %label,
            module,
            error = %err,
            reason = "backfill_refold_failed",
            "index backfill could not rebuild the read model; the floor stands"
        );
        return None;
    }
    closed.ok()
}

/// does the source hold op rows BELOW `floor`? every index-op reply carries
/// the source's own floor, so asking from a cursor PAST the boundary answers
/// it with an empty page — the cheapest honest question, and a boot seam
/// cannot afford a dearer one. a refusal answers `false`: an unreachable or
/// unwilling source is never a reason to wipe a module's views.
async fn source_reaches_below<C: statesync::SyncClient>(
    client: &C,
    module: &str,
    floor: u64,
    boundary: u64,
) -> bool {
    let asked = client
        .request(statesync::SyncRequest::IndexOps {
            boundary,
            module: module.to_string(),
            after: Some((boundary, AFTER_EVERY_SEQ)),
        })
        .await;
    let Ok(statesync::SyncResponse::IndexOps { source_floor, .. }) = asked else {
        return false;
    };
    source_floor.is_none_or(|source| source < floor)
}

/// stamp ONE module at the boundary: its feed and views begin there, visibly
/// via the floor. `None` when the store refused — the caller stops asking.
fn stamp_module(
    index: &indexer::IndexStore,
    module: &str,
    boundary: u64,
    label: &str,
) -> Option<()> {
    match index.mark_backfilled(module, boundary) {
        Ok(()) => {
            tracing::info!(
                target: "ducktape::modules",
                node = %label,
                module,
                height = boundary,
                "index for {module} stamped backfilled at height {boundary}"
            );
            Some(())
        }
        Err(err) => {
            tracing::error!(
                target: "ducktape::modules",
                event = "node_index_poisoned",
                node = %label,
                module,
                height = boundary,
                error = %err,
                "index heal failed; wipe <storage>/index to rebuild"
            );
            None
        }
    }
}

/// pull only what this module is MISSING: the rows above its own watermark,
/// written onto the feed it already holds. `None` refuses the resume — the
/// caller falls back to the stamp — and never leaves a claim standing: the
/// partial rows a refused walk wrote are wiped by that stamp.
async fn resume_module<C: statesync::SyncClient>(
    index: &indexer::IndexStore,
    client: &C,
    module: &str,
    boundary: u64,
    label: &str,
) -> Option<Backfilled> {
    let held = match index.applied_height(module) {
        Ok(held) => held,
        Err(err) => {
            tracing::warn!(
                target: "ducktape::statesync",
                node = %label,
                module,
                error = %err,
                reason = "backfill_watermark_unreadable",
                "index watermark unreadable; stamping at the boundary instead of resuming"
            );
            return None;
        }
    };
    if held == 0 {
        return None; // an empty feed has nothing to resume from.
    }
    let done = backfill_module(
        index,
        client,
        module,
        boundary,
        Some((held, AFTER_EVERY_SEQ)),
        label,
    )
    .await
    .ok()?;
    // THE SOURCE MUST REACH THE RESUME POINT. a source floor ABOVE this
    // node's watermark means the source's own history starts inside the range
    // this node is missing, so the delta would leave a HOLE between them —
    // and a floor cannot express a hole. stamp instead, and inherit the
    // source's truncation honestly.
    if done.source_floor.is_some_and(|floor| floor > held) {
        tracing::warn!(
            target: "ducktape::statesync",
            node = %label,
            module,
            held,
            floor = done.source_floor.unwrap_or(0),
            reason = "backfill_resume_uncovered",
            "index backfill cannot resume from this source; stamping at the boundary"
        );
        return None;
    }
    // the feed now reaches the boundary, so the watermark says so. the FLOOR
    // does not move: this node kept every row it already had, and nothing
    // below it was ever claimed.
    if let Err(err) = index.advance_watermark(module, boundary) {
        tracing::warn!(
            target: "ducktape::statesync",
            node = %label,
            module,
            error = %err,
            reason = "backfill_watermark_refused",
            "index backfill could not advance the feed watermark"
        );
        return None;
    }
    let held_floor = match index.backfill_height(module) {
        Ok(floor) => floor,
        Err(err) => {
            tracing::warn!(
                target: "ducktape::statesync",
                node = %label,
                module,
                error = %err,
                reason = "backfill_floor_unreadable",
                "index floor unreadable after a resumed walk; stamping at the boundary instead"
            );
            return None;
        }
    };
    Some(Backfilled {
        source_floor: held_floor,
        ..done
    })
}

/// what a REFUSED walk still left in the feed: the last row it wrote, `None`
/// when it wrote nothing at all. the caller's "is there anything to clean up
/// after" — a walk that never wrote cannot have disturbed anything.
struct Wrote(Option<(u64, u32)>);

/// walk one module's op rows below `boundary` off the source and write them,
/// resuming strictly after `after` when the caller already holds a feed.
/// `Err` when the module keeps its floor, warned once with a stable reason
/// token, carrying how far the walk got.
async fn backfill_module<C: statesync::SyncClient>(
    index: &indexer::IndexStore,
    client: &C,
    module: &str,
    boundary: u64,
    after: Option<(u64, u32)>,
    label: &str,
) -> Result<Backfilled, Wrote> {
    let mut rows = 0usize;
    let mut bytes = 0usize;
    let mut last: Option<(u64, u32)> = None;
    // the fetcher folds a write refusal into the same `SyncError::Module` a
    // wire refusal produces, and the message is already carried by `error`;
    // this only remembers WHICH side failed, for the reason token.
    let mut write_refused = false;
    let walked = statesync::fetch_index_ops(client, module, boundary, after, |page| {
        index.write_backfill_rows(module, page).map_err(|e| {
            write_refused = true;
            e.to_string()
        })?;
        rows += page.len();
        bytes += page.iter().map(|(k, v)| k.len() + v.len()).sum::<usize>();
        // the last row WRITTEN, not the last page's: a final empty page (a
        // source that re-stamped mid-walk) must not erase the position the
        // fold-tip check is about to verify.
        if let Some((key, _)) = page.last() {
            last = indexer::parse_op_key(key.as_bytes());
        }
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
            let reason = if write_refused {
                "backfill_write_failed"
            } else {
                "backfill_fetch_failed"
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
            return Err(Wrote(last));
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
    Ok(Backfilled {
        module: module.to_string(),
        source_floor,
        last_row: last,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::{Arc, Mutex};

    use statesync::{SyncError, SyncRequest, SyncResponse};

    /// a serving source for the index-op lane: answers off a REAL store
    /// through the production serve path (the loop-side read plus the wire
    /// bounding), and records every page it was asked for and served.
    #[derive(Clone)]
    struct SourceNode {
        source: Arc<indexer::IndexStore>,
        asked: Recorded<Option<(u64, u32)>>,
        served: Recorded<(u64, u32)>,
        /// how many asks this source answers before it starts refusing — the
        /// source that drops mid-walk.
        answers: usize,
    }

    /// what the source was asked for / handed out, shared with the test.
    type Recorded<T> = Arc<Mutex<Vec<T>>>;

    impl SourceNode {
        fn new(source: indexer::IndexStore) -> Self {
            Self {
                source: Arc::new(source),
                asked: Arc::new(Mutex::new(Vec::new())),
                served: Arc::new(Mutex::new(Vec::new())),
                answers: usize::MAX,
            }
        }
        /// the same source, dropping after `answers` asks.
        fn answering(mut self, answers: usize) -> Self {
            self.answers = answers;
            self
        }
        fn pages_asked(&self) -> usize {
            self.asked.lock().expect("asked").len()
        }
        fn rows_served(&self) -> Vec<(u64, u32)> {
            self.served.lock().expect("served").clone()
        }
    }

    impl statesync::SyncClient for SourceNode {
        fn request(
            &self,
            req: SyncRequest,
        ) -> impl std::future::Future<Output = Result<SyncResponse, SyncError>> + Send {
            let resp = match req {
                SyncRequest::IndexOps {
                    boundary,
                    module,
                    after,
                } => {
                    let asks = {
                        let mut asked = self.asked.lock().expect("asked");
                        asked.push(after);
                        asked.len()
                    };
                    let read = if asks > self.answers {
                        Err("source dropped mid-walk".to_string())
                    } else {
                        crate::validator::run::sync::read_index_ops(
                            &self.source,
                            &module,
                            after,
                            boundary,
                        )
                    };
                    match read {
                        Ok(page) => {
                            let (resp, _read_ahead) =
                                crate::sync::serve::split_index_ops_response(page);
                            if let SyncResponse::IndexOps { rows, .. } = &resp {
                                self.served.lock().expect("served").extend(
                                    rows.iter().filter_map(|(key, _)| {
                                        indexer::parse_op_key(key.as_bytes())
                                    }),
                                );
                            }
                            resp
                        }
                        Err(e) => SyncResponse::Error(e),
                    }
                }
                other => SyncResponse::Error(format!("unexpected {}", other.kind_name())),
            };
            async move { Ok(resp) }
        }
    }

    fn store(dir: &std::path::Path) -> indexer::IndexStore {
        indexer::IndexStore::open(dir, &[indexer::IndexModule::bare("chat")]).expect("open index")
    }

    /// the reference mapper (`crates/kernel/index-guest/testmap`, refreshed by
    /// `make wasm-modules`) — the same artifact the indexer's own fold tests
    /// run, so a module here can have a REAL fold and a read model to check.
    const TESTMAP: &[u8] = include_bytes!("../../../crates/kernel/index-guest/testmap/index.wasm");

    fn mapped_store(dir: &std::path::Path) -> indexer::IndexStore {
        indexer::IndexStore::open(
            dir,
            &[indexer::IndexModule {
                id: "chat",
                guest: Some(TESTMAP),
            }],
        )
        .expect("open index")
    }

    fn block(height: u64) -> indexer::BlockOps {
        indexer::BlockOps {
            height,
            time: height,
            ops: vec![indexer::AppliedOp {
                module: "chat".into(),
                origin: indexer::OriginTag::external("jess"),
                payload: format!(r#"{{"height":{height}}}"#).into_bytes(),
                assigned: Vec::new(),
            }],
            record: None,
        }
    }

    fn op_rows(index: &indexer::IndexStore) -> Vec<(u64, u32)> {
        index
            .scan("chat", indexer::OP_PREFIX.as_bytes(), None, 1024)
            .expect("scan")
            .entries
            .iter()
            .filter_map(|(key, _)| indexer::parse_op_key(key))
            .collect()
    }

    /// A RE-STAMPING ASCENSION PULLS ONLY WHAT IT IS MISSING. A resident that
    /// already folded blocks 1..=8 and re-ascends at boundary 10 holds every
    /// op row below its own watermark; re-pulling them costs the source (and
    /// the joiner's fold) the whole history for a two-block delta. The wire
    /// must carry the delta and nothing else — and the feed the node already
    /// had must survive the ascension.
    #[tokio::test]
    async fn a_re_stamping_ascension_pulls_only_what_it_is_missing() {
        let source_dir = tempfile::tempdir().expect("source dir");
        let joiner_dir = tempfile::tempdir().expect("joiner dir");
        let source = store(source_dir.path());
        let joiner = store(joiner_dir.path());
        for height in 1..=10 {
            source.apply_block(&block(height)).expect("source folds");
        }
        // the joiner watched the first eight blocks itself: its feed reaches
        // its watermark, which is exactly what a resume may stand on.
        for height in 1..=8 {
            joiner.apply_block(&block(height)).expect("joiner folds");
        }
        let client = SourceNode::new(source);

        heal_and_backfill_index(&joiner, &client, 10, "joiner").await;

        assert_eq!(
            client.rows_served(),
            vec![(9, 0), (10, 0)],
            "only the rows above the joiner's watermark may cross the wire"
        );
        assert_eq!(client.pages_asked(), 1, "one wire page carries the delta");
        assert_eq!(
            op_rows(&joiner),
            (1..=10).map(|h| (h, 0)).collect::<Vec<_>>(),
            "the feed the joiner already held survives the ascension"
        );
        assert_eq!(
            joiner.applied_height("chat").expect("watermark"),
            10,
            "the resumed feed reaches the boundary"
        );
        assert_eq!(
            joiner.backfill_height("chat").expect("floor"),
            None,
            "a resumed module was never floored"
        );
    }

    /// A RESTART OVER A WIPED INDEX DIRECTORY CLOSES ITS FLOOR. The journal
    /// replay brings a wiped module's watermark back to the recovered tip, so
    /// nothing is STALE — and the history below the stamp the wipe left is
    /// reachable only from a source. Before this, that resident stamped and
    /// served a feed that simply began at its restart, forever.
    #[tokio::test]
    async fn a_restart_over_a_wiped_index_pulls_the_history_below_its_floor() {
        let source_dir = tempfile::tempdir().expect("source dir");
        let joiner_dir = tempfile::tempdir().expect("joiner dir");
        let source = store(source_dir.path());
        let joiner = store(joiner_dir.path());
        for height in 1..=10 {
            source.apply_block(&block(height)).expect("source folds");
        }
        // the wiped resident: stamped at its checkpoint, then the journal
        // replay folded the suffix back on top — watermark at the tip, floor
        // at 5, and nothing below it.
        joiner.mark_backfilled("chat", 5).expect("stamp");
        for height in 6..=10 {
            joiner.apply_block(&block(height)).expect("replay folds");
        }
        assert_eq!(joiner.applied_height("chat").expect("watermark"), 10);
        let client = SourceNode::new(source);

        heal_and_backfill_index(&joiner, &client, 10, "resident").await;

        assert_eq!(
            op_rows(&joiner),
            (1..=10).map(|h| (h, 0)).collect::<Vec<_>>(),
            "the history below the floor reaches the feed"
        );
        assert_eq!(
            joiner.backfill_height("chat").expect("floor"),
            None,
            "and the floor it was left holding is gone"
        );
    }

    /// one block carrying many ops — enough op rows at a single height to
    /// span more than one wire page.
    fn wide_block(height: u64, ops: usize) -> indexer::BlockOps {
        indexer::BlockOps {
            height,
            time: height,
            ops: (0..ops)
                .map(|n| indexer::AppliedOp {
                    module: "chat".into(),
                    origin: indexer::OriginTag::external("jess"),
                    payload: format!(r#"{{"height":{height},"n":{n}}}"#).into_bytes(),
                    assigned: Vec::new(),
                })
                .collect(),
            record: None,
        }
    }

    /// CLOSING A FLOOR NEVER TRADES A HEALTHY FEED FOR A WALK THAT MIGHT FAIL.
    /// The probe only proves the source answered once; the walk can still die
    /// on its next page — a dropped source, a page failing the canonical seal,
    /// a source that re-stamps mid-walk. A seam that wiped first would leave
    /// that module — perfectly fine before it ran — floored at the boundary
    /// with its feed and views destroyed, and the next restart would do it
    /// again. So the rows land UNDER the feed and the read model is rebuilt
    /// from the whole of it; a failed walk costs bandwidth, never data.
    #[tokio::test]
    async fn a_close_that_fails_mid_walk_keeps_the_feed_and_floor_it_found() {
        const FLOOR: u64 = 5;
        // wide enough that the history below the floor takes two wire pages,
        // so the source can drop with rows already written.
        const BELOW: usize = statesync::INDEX_OPS_BATCH_LEN + 97;
        let source_dir = tempfile::tempdir().expect("source dir");
        let joiner_dir = tempfile::tempdir().expect("joiner dir");
        let source = store(source_dir.path());
        let joiner = store(joiner_dir.path());
        source
            .apply_block(&wide_block(FLOOR, BELOW))
            .expect("source folds the wide block");
        for height in (FLOOR + 1)..=10 {
            source.apply_block(&block(height)).expect("source folds");
        }
        joiner.mark_backfilled("chat", FLOOR).expect("stamp");
        for height in (FLOOR + 1)..=10 {
            joiner.apply_block(&block(height)).expect("replay folds");
        }
        let held = op_rows(&joiner);

        // the probe is answered, the walk's first page is answered, and then
        // the source is gone.
        let client = SourceNode::new(source).answering(2);
        heal_and_backfill_index(&joiner, &client, 10, "resident").await;

        assert_eq!(client.pages_asked(), 3, "probe, one page, then the refusal");
        let rows = op_rows(&joiner);
        assert_eq!(
            rows.iter().filter(|(height, _)| *height == FLOOR).count(),
            statesync::INDEX_OPS_BATCH_LEN,
            "the one page that did arrive was kept"
        );
        assert!(
            held.iter().all(|row| rows.contains(row)),
            "and not one row the module already had was lost"
        );
        assert_eq!(
            joiner.backfill_height("chat").expect("floor"),
            Some(FLOOR),
            "the floor it found stands: nothing below it can be claimed yet"
        );
    }

    /// THE SEAM REBUILDS THE READ MODEL IT DISTURBS. Every other test here
    /// runs a BARE module — no guest, no fold — which cannot see the half of
    /// this seam that matters most: rows landing below what the fold already
    /// consumed leave the derived keyspace describing a feed that no longer
    /// exists, and the fold tip pointing at the low row it just wrote. A
    /// caller waiting for its own op at height 10 would wait forever. Delete
    /// the refold from `close_floor` and this is what fails.
    #[tokio::test]
    async fn closing_a_floor_leaves_the_read_model_current_with_the_whole_feed() {
        const FLOOR: u64 = 5;
        let source_dir = tempfile::tempdir().expect("source dir");
        let joiner_dir = tempfile::tempdir().expect("joiner dir");
        // the source only has to SERVE rows; the joiner is the one deriving.
        let source = store(source_dir.path());
        let joiner = mapped_store(joiner_dir.path());
        for height in 1..=10 {
            source.apply_block(&block(height)).expect("source folds");
        }
        joiner.mark_backfilled("chat", FLOOR).expect("stamp");
        for height in (FLOOR + 1)..=10 {
            joiner.apply_block(&block(height)).expect("replay folds");
        }
        joiner
            .wait_folds_drained()
            .expect("the replayed suffix folds");
        assert_eq!(joiner.fold_tip("chat").expect("tip"), Some((10, 0)));

        let client = SourceNode::new(source);
        heal_and_backfill_index(&joiner, &client, 10, "resident").await;

        assert_eq!(
            joiner.fold_tip("chat").expect("tip"),
            Some((10, 0)),
            "the tip vouches for the whole feed, not the last row backfilled"
        );
        assert_eq!(
            joiner
                .scan("chat", b"seen/", None, 100)
                .expect("scan")
                .entries
                .len(),
            10,
            "and every row below the floor derived its view row"
        );
        assert_eq!(
            joiner.backfill_height("chat").expect("floor"),
            None,
            "the floor is gone: the feed reaches genesis"
        );
    }

    /// A SOURCE MISSING THE SAME HISTORY IS NOT WORTH A WIPE. Closing a floor
    /// means rebuilding the feed, so the seam asks first — one empty page
    /// carries the source's own floor — and a source floored no lower than
    /// this node ends the matter there.
    #[tokio::test]
    async fn a_floor_the_source_cannot_lower_costs_one_question_and_no_wipe() {
        let source_dir = tempfile::tempdir().expect("source dir");
        let joiner_dir = tempfile::tempdir().expect("joiner dir");
        let source = store(source_dir.path());
        let joiner = store(joiner_dir.path());
        // BOTH nodes begin at 5: the source cannot answer for what neither
        // of them kept.
        source.mark_backfilled("chat", 5).expect("source stamp");
        joiner.mark_backfilled("chat", 5).expect("stamp");
        for height in 6..=10 {
            source.apply_block(&block(height)).expect("source folds");
            joiner.apply_block(&block(height)).expect("replay folds");
        }
        let client = SourceNode::new(source);

        heal_and_backfill_index(&joiner, &client, 10, "resident").await;

        assert_eq!(
            client.pages_asked(),
            1,
            "the question, and nothing after it"
        );
        assert!(client.rows_served().is_empty(), "not one row crosses");
        assert_eq!(
            op_rows(&joiner),
            (6..=10).map(|h| (h, 0)).collect::<Vec<_>>(),
            "the feed it had stands — a wipe would have rebuilt it for nothing"
        );
        assert_eq!(
            joiner.backfill_height("chat").expect("floor"),
            Some(5),
            "and the floor stays honest"
        );
    }

    /// A JOINER WITH NO FEED STILL STAMPS AND PULLS THE WHOLE HISTORY. The
    /// resume above is an optimization on held rows, never a reason to skip
    /// the boundary stamp a fresh joiner needs (#1130).
    #[tokio::test]
    async fn a_fresh_joiner_stamps_and_pulls_the_whole_history() {
        let source_dir = tempfile::tempdir().expect("source dir");
        let joiner_dir = tempfile::tempdir().expect("joiner dir");
        let source = store(source_dir.path());
        let joiner = store(joiner_dir.path());
        for height in 1..=10 {
            source.apply_block(&block(height)).expect("source folds");
        }
        let client = SourceNode::new(source);

        heal_and_backfill_index(&joiner, &client, 10, "joiner").await;

        assert_eq!(
            client.rows_served(),
            (1..=10).map(|h| (h, 0)).collect::<Vec<_>>(),
            "a joiner holding nothing pulls the whole history below the boundary"
        );
        assert_eq!(
            op_rows(&joiner),
            (1..=10).map(|h| (h, 0)).collect::<Vec<_>>(),
            "and every one of them lands in the joiner's feed"
        );
        assert_eq!(joiner.applied_height("chat").expect("watermark"), 10);
        assert_eq!(
            joiner.backfill_height("chat").expect("floor"),
            None,
            "the source covered the range from genesis, so nothing is missing"
        );
    }
}
