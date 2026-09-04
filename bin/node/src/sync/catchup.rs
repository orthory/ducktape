use host::Host;
use recovery::{Manifest, Recovery};
use statesync::{fetch_frames, fetch_manifest};

use crate::constants::CUTOVER_DELAY;
use crate::explorer::IndexFold;
use crate::sync::serve::to_node_disposition;
use crate::util::hex;

pub(crate) async fn apply_verified_suffix_frame(
    host: &mut Host,
    served: &statesync::FinalizedFrame,
    code_source: &dyn host::CodeSource,
) -> Result<Vec<host::DispatchRecord>, String> {
    let expected = to_node_disposition(served.disposition);
    // CODE-SWAP REALIZATION, mirroring the live drain and recovery replay: a
    // frame sealed after a code-registry swap executed on the NEW component, so
    // catch-up must swap before re-applying or the served roots cannot
    // reproduce. fail-closed on missing/tampered bytes.
    host.realize_module_swaps(served.height, code_source)
        .await
        .map_err(|e| format!("code-swap realization at height {}: {e}", served.height))?;
    // the served frame is a BATCH: decode its members and apply as ONE block,
    // exactly like the live drain and recovery replay, so the disposition,
    // roots, and root-hash reproduce what the peer served. disposition is
    // DRAIN-based (any member applied, or a System injection ran), never
    // root-hash-based.
    let (outcome, dispatches) = match node::decode_batch(&served.frame) {
        Ok(members) => {
            let mut ops = Vec::new();
            for member in &members {
                if let Ok(op) = node::decode_member(member) {
                    ops.push(op);
                }
            }
            let ctx = host::BlockContext {
                height: served.height,
                consensus_time: served.height,
                origin: sdk::Origin::System,
            };
            match host.submit_block_ops(ctx, ops).await {
                Ok(batch) => {
                    let (ran, dispatches) = batch.into_trace();
                    let outcome = if ran {
                        node::Disposition::Applied
                    } else {
                        node::Disposition::Rejected
                    };
                    (outcome, dispatches)
                }
                Err(host::SubmitError::Rejected(_)) => (node::Disposition::Rejected, Vec::new()),
                Err(host::SubmitError::Fatal(f)) => {
                    return Err(format!("fatal host error applying suffix frame: {f}"));
                }
            }
        }
        Err(_) => (node::Disposition::Rejected, Vec::new()),
    };
    if outcome != expected {
        return Err(format!(
            "served seal mismatch at height {}: replay landed as {outcome:?}, \
             served as {expected:?}",
            served.height
        ));
    }
    let roots = host.module_roots();
    if roots != served.roots {
        return Err(format!(
            "served seal mismatch at height {}: roots changed to {:?}, served {:?}",
            served.height, roots, served.roots
        ));
    }
    let root_hash = host.root_hash();
    if root_hash != served.root_hash {
        return Err(format!(
            "served seal mismatch at height {}: root_hash {} != served {}",
            served.height,
            hex(&root_hash),
            hex(&served.root_hash)
        ));
    }
    Ok(dispatches)
}
pub(crate) async fn apply_and_journal_verified_frame<E>(
    recovery: &mut Recovery<E>,
    host: &mut Host,
    frame: &statesync::FinalizedFrame,
    fold: Option<&mut IndexFold<'_>>,
) -> Result<(), String>
where
    E: recovery::Context + commonware_runtime::BufferPooler + commonware_runtime::Supervisor,
{
    node::BlockSink::pre_apply(recovery, frame.height, &frame.frame)
        .await
        .map_err(|e| format!("catch-up WAL write: {e}"))?;
    // realize swaps through the SAME source replay uses (wired on Recovery), so
    // every path reconciles code identically.
    let code_source = recovery.code_source();
    let dispatches = apply_verified_suffix_frame(host, frame, code_source.as_ref()).await?;
    let seal = node::BlockSeal {
        height: frame.height,
        disposition: to_node_disposition(frame.disposition),
        roots: host.module_roots(),
        root_hash: host.root_hash(),
    };
    node::BlockSink::seal(recovery, &seal)
        .await
        .map_err(|e| format!("catch-up seal write: {e}"))?;
    if let Some(fold) = fold {
        use recovery::ReplaySink as _;
        fold.folded_block(&recovery::FoldedBlock {
            height: frame.height,
            frame: &frame.frame,
            disposition: seal.disposition,
            root_hash: seal.root_hash,
            dispatches: &dispatches,
            roots: &seal.roots,
        });
    }
    Ok(())
}

#[derive(Debug, Default)]
pub(crate) struct SuffixCatchupApply {
    pub(crate) applied: usize,
    frames: Vec<Vec<u8>>,
}

pub(crate) async fn apply_suffix_frames<E>(
    recovery: &mut Recovery<E>,
    host: &mut Host,
    from_height: u64,
    to_height: u64,
    frames: Vec<statesync::FinalizedFrame>,
    mut fold: Option<&mut IndexFold<'_>>,
) -> Result<SuffixCatchupApply, String>
where
    E: recovery::Context + commonware_runtime::BufferPooler + commonware_runtime::Supervisor,
{
    if to_height < from_height {
        return Err(format!(
            "invalid catch-up range ({from_height}, {to_height}]"
        ));
    }
    if from_height == to_height {
        if !frames.is_empty() {
            return Err(format!(
                "no-gap catch-up received {} unexpected frames",
                frames.len()
            ));
        }
        return Ok(SuffixCatchupApply::default());
    }
    if frames.last().map(|f| f.height) != Some(to_height) {
        return Err(format!(
            "catch-up frames stopped before target height {to_height}"
        ));
    }

    let mut last = from_height;
    let mut applied = SuffixCatchupApply::default();
    for frame in frames {
        if frame.height <= last || frame.height > to_height {
            return Err(format!(
                "catch-up frame height {} outside ({last}, {to_height}]",
                frame.height
            ));
        }
        apply_and_journal_verified_frame(recovery, host, &frame, fold.as_deref_mut()).await?;
        last = frame.height;
        applied.applied += 1;
        applied.frames.push(frame.frame.clone());
    }
    Ok(applied)
}

#[derive(Debug)]
pub(crate) struct SuffixCatchup {
    pub(crate) to_height: u64,
    pub(crate) frame_bytes: Vec<Vec<u8>>,
}

#[derive(Debug)]
pub(crate) enum SuffixCatchupError {
    Retry(String),
    Fatal(String),
}

pub(crate) async fn catch_up_suffix_frames<C, E>(
    client: &C,
    recovery: &mut Recovery<E>,
    host: &mut Host,
    fold: Option<&mut IndexFold<'_>>,
    recovered_height: u64,
    max_iterations: usize,
) -> Result<SuffixCatchup, SuffixCatchupError>
where
    C: statesync::SyncClient,
    E: recovery::Context + commonware_runtime::BufferPooler + commonware_runtime::Supervisor,
{
    let mut fold = fold;
    let mut current_height = recovered_height;
    let mut total_frames = 0usize;
    let mut frame_bytes = Vec::new();

    for _ in 0..=max_iterations {
        let tip = fetch_manifest(client).await.map_err(|e| {
            SuffixCatchupError::Retry(format!("catch-up manifest unavailable: {e}"))
        })?;
        if tip.height <= current_height {
            if tip.height == current_height && host.root_hash() != tip.root_hash {
                return Err(SuffixCatchupError::Fatal(format!(
                    "catch-up source hash {} at height {} does not match recovered host {}",
                    hex(&tip.root_hash),
                    tip.height,
                    hex(&host.root_hash())
                )));
            }
            tracing::debug!(
                target: "ducktape::statesync",
                from = recovered_height,
                to = current_height,
                frames = total_frames,
                "suffix catch-up planned"
            );
            return Ok(SuffixCatchup {
                to_height: current_height,
                frame_bytes,
            });
        }

        let frames = match fetch_frames(client, current_height, tip.height).await {
            Ok(frames) => frames,
            Err(statesync::SyncError::RangePruned {
                requested_after,
                retained_from,
            }) => {
                // the follower side of the same wedge (#493, macOS "missing blocks").
                // it printed the SAME impossible range on every certificate, which is
                // indistinguishable from healthy catch-up — and so it read as boot
                // noise for days. `permanent` is the word that ends the guessing: this
                // does not heal by waiting, because the source can only prune FURTHER
                // ahead of us.
                tracing::error!(
                    target: "ducktape::statesync",
                    requested_after,
                    retained_from,
                    gap_blocks = retained_from.saturating_sub(requested_after),
                    permanent = true,
                    "catch-up IMPOSSIBLE — the source pruned past our height; waiting will \
                     never fix this, we must full-sync from a fresh checkpoint"
                );
                return Err(SuffixCatchupError::Retry(format!(
                    "source pruned past requested height {requested_after}; retained from \
                     {retained_from}"
                )));
            }
            Err(e) => {
                return Err(SuffixCatchupError::Retry(format!(
                    "catch-up frame suffix unavailable: {e}"
                )));
            }
        };
        let applied = apply_suffix_frames(
            recovery,
            host,
            current_height,
            tip.height,
            frames,
            fold.as_deref_mut(),
        )
        .await
        .map_err(SuffixCatchupError::Fatal)?;
        if host.root_hash() != tip.root_hash {
            return Err(SuffixCatchupError::Fatal(format!(
                "catch-up frames landed at {}, target manifest {}",
                hex(&host.root_hash()),
                hex(&tip.root_hash)
            )));
        }
        current_height = tip.height;
        total_frames += applied.applied;
        frame_bytes.extend(applied.frames);
    }

    tracing::debug!(
        target: "ducktape::statesync",
        from = recovered_height,
        to = current_height,
        frames = total_frames,
        "suffix catch-up planned"
    );
    Ok(SuffixCatchup {
        to_height: current_height,
        frame_bytes,
    })
}

pub(crate) fn advance_next_seq_from_frames(next_seq: &mut u64, frames: &[Vec<u8>], me: &[u8]) {
    for frame in frames {
        if let Some((origin, seq)) = node::frame_origin_seq(frame)
            && origin == me
        {
            *next_seq = (*next_seq).max(seq + 1);
        }
    }
}

pub(crate) fn derive_pending_boot(manifest: &Manifest, rec: &recovery::Recovered) -> Option<u64> {
    let checkpoint_pending = if rec.epoch == manifest.epoch {
        manifest.pending_cutover_view
    } else {
        None
    };
    checkpoint_pending.or_else(|| {
        let mut prev_root = manifest.root("valset").expect("valset is a genesis module");
        let mut armed = None;
        for (height, roots) in &rec.blocks {
            let root = roots
                .iter()
                .find(|(id, _)| id == "valset")
                .map(|(_, r)| *r)
                .expect("every seal carries the full root vector");
            if root != prev_root && *height > rec.view_base && armed.is_none() {
                armed = Some(*height - rec.view_base + CUTOVER_DELAY);
            }
            prev_root = root;
        }
        armed
    })
}
