use commonware_cryptography::ed25519;
use commonware_p2p::{Receiver as P2pReceiver, Recipients, Sender as P2pSender};
use commonware_runtime::IoBuf;
use host::Host;
use recovery::{Manifest, Recovery};
use sdk::{ModuleId, StateRoot};
use statesync::{fetch_frames, fetch_manifest};

use crate::constants::{BOOT_SYNC_REQUEST_TIMEOUT, CUTOVER_DELAY};
use crate::explorer::IndexFold;
use crate::sync::serve::to_node_disposition;
use crate::util::hex;

pub(crate) async fn apply_verified_suffix_frame(
    host: &mut Host,
    served: &statesync::FinalizedFrame,
    code_source: &dyn host::CodeSource,
) -> Result<Vec<host::DispatchRecord>, String> {
    let expected = to_node_disposition(served.disposition);
    let protocol_version = host.effective_version(served.height).await;
    host.set_active_version(protocol_version);
    // CODE-SWAP REALIZATION, mirroring the live drain and recovery replay: a
    // frame sealed after a code-registry swap executed on the NEW component, so
    // catch-up must swap before re-applying or the served roots cannot
    // reproduce. fail-closed on missing/tampered bytes.
    host.realize_module_swaps(served.height, code_source)
        .await
        .map_err(|e| format!("code-swap realization at height {}: {e}", served.height))?;
    // the served frame is a BATCH: decode its members and apply as ONE block,
    // exactly like the live drain and recovery replay, so the disposition,
    // roots, and app-hash reproduce what the peer served. disposition is
    // DRAIN-based (any member applied or a System injection ran), never
    // app-hash-based.
    let (outcome, dispatches) = match node::decode_batch(&served.frame) {
        Ok(members) => {
            let mut ops = Vec::new();
            for member in &members {
                if let Ok(pair) = node::decode_frame(member) {
                    ops.push(pair);
                }
            }
            let ctx = host::BlockContext {
                protocol_version,
                height: served.height,
                consensus_time: served.height,
                origin: sdk::Origin::System,
            };
            match host.submit_block(ctx, ops).await {
                Ok(batch) => {
                    let mut dispatches = Vec::new();
                    let mut any_applied = false;
                    for member in batch.members {
                        if let host::MemberOutcome::Applied { dispatches: d } = member {
                            any_applied = true;
                            dispatches.extend(d);
                        }
                    }
                    let has_system = !batch.system_dispatches.is_empty();
                    dispatches.extend(batch.system_dispatches);
                    let outcome = if any_applied || has_system {
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
    let app_hash = host.app_hash();
    if app_hash != served.app_hash {
        return Err(format!(
            "served seal mismatch at height {}: app_hash {} != served {}",
            served.height,
            hex(&app_hash),
            hex(&served.app_hash)
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
        app_hash: host.app_hash(),
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
            app_hash: seal.app_hash,
            dispatches: &dispatches,
        });
    }
    Ok(())
}

#[derive(Debug, Default)]
pub(crate) struct PostRebootCatchupApply {
    pub(crate) applied: usize,
    frames: Vec<Vec<u8>>,
    pub(crate) blocks: Vec<(u64, Vec<(ModuleId, StateRoot)>)>,
}

pub(crate) async fn apply_post_reboot_catchup_frames<E>(
    recovery: &mut Recovery<E>,
    host: &mut Host,
    from_height: u64,
    to_height: u64,
    frames: Vec<statesync::FinalizedFrame>,
    mut fold: Option<&mut IndexFold<'_>>,
) -> Result<PostRebootCatchupApply, String>
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
        return Ok(PostRebootCatchupApply::default());
    }
    if frames.last().map(|f| f.height) != Some(to_height) {
        return Err(format!(
            "catch-up frames stopped before target height {to_height}"
        ));
    }

    let mut last = from_height;
    let mut applied = PostRebootCatchupApply::default();
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
        applied.blocks.push((frame.height, frame.roots.clone()));
    }
    Ok(applied)
}

pub(crate) fn catchup_pending_cutover_view(
    base_manifest: Option<&Manifest>,
    target: &statesync::Manifest,
    blocks: &[(u64, Vec<(ModuleId, StateRoot)>)],
) -> Result<Option<u64>, String> {
    let Some(base) = base_manifest else {
        return Ok(None);
    };
    if base.epoch == target.epoch && base.pending_cutover_view.is_some() {
        return Ok(base.pending_cutover_view);
    }
    let Some(mut prev_root) = base.root("valset") else {
        return Ok(None);
    };
    for (height, roots) in blocks {
        let root = roots
            .iter()
            .find(|(id, _)| id == "valset")
            .map(|(_, root)| *root)
            .ok_or_else(|| format!("catch-up seal at height {height} has no valset root"))?;
        if root != prev_root && *height > target.view_base {
            return Ok(Some(*height - target.view_base + CUTOVER_DELAY));
        }
        prev_root = root;
    }
    Ok(None)
}

pub(crate) async fn write_post_reboot_catchup_checkpoint<E>(
    recovery: &mut Recovery<E>,
    host: &Host,
    base_manifest: Option<&Manifest>,
    target: &statesync::Manifest,
    blocks: &[(u64, Vec<(ModuleId, StateRoot)>)],
    next_seq: u64,
) -> Result<Manifest, String>
where
    E: recovery::Context + commonware_runtime::BufferPooler + commonware_runtime::Supervisor,
{
    if host.app_hash() != target.app_hash {
        return Err(format!(
            "catch-up checkpoint host hash {} does not match target {}",
            hex(&host.app_hash()),
            hex(&target.app_hash)
        ));
    }
    let pending_cutover_view = catchup_pending_cutover_view(base_manifest, target, blocks)?;
    let pos = recovery.oplog_pos().await;
    let ckpt = Manifest::capture(
        host,
        Some(target.height),
        target.epoch,
        target.view_base,
        target.participants.clone(),
        target.residents.clone(),
        pending_cutover_view,
        target.current_version,
        target.pending_upgrade.clone(),
        pos,
        next_seq,
    )
    .map_err(|e| format!("catch-up checkpoint capture: {e}"))?;
    recovery
        .write_manifest(&ckpt)
        .await
        .map_err(|e| format!("catch-up checkpoint write: {e}"))?;
    tracing::debug!(
        target: "ducktape::statesync",
        height = target.height,
        "catch-up checkpoint captured"
    );
    Ok(ckpt)
}

#[derive(Debug)]
pub(crate) struct PostRebootCatchup {
    pub(crate) from_height: u64,
    pub(crate) to_height: u64,
    pub(crate) frames: usize,
    pub(crate) target: Option<statesync::Manifest>,
    pub(crate) frame_bytes: Vec<Vec<u8>>,
    pub(crate) blocks: Vec<(u64, Vec<(ModuleId, StateRoot)>)>,
}

#[derive(Debug)]
pub(crate) enum PostRebootCatchupError {
    Retry(String),
    RangePruned {
        target: Box<statesync::Manifest>,
        requested_after: u64,
        retained_from: u64,
    },
    Fatal(String),
}

pub(crate) async fn catch_up_post_reboot_frames<C, E>(
    client: &C,
    recovery: &mut Recovery<E>,
    host: &mut Host,
    fold: Option<&mut IndexFold<'_>>,
    recovered_height: u64,
    max_iterations: usize,
) -> Result<PostRebootCatchup, PostRebootCatchupError>
where
    C: statesync::SyncClient,
    E: recovery::Context + commonware_runtime::BufferPooler + commonware_runtime::Supervisor,
{
    let mut fold = fold;
    let mut current_height = recovered_height;
    let mut total_frames = 0usize;
    let mut target = None;
    let mut frame_bytes = Vec::new();
    let mut blocks = Vec::new();

    for _ in 0..=max_iterations {
        let tip = fetch_manifest(client).await.map_err(|e| {
            PostRebootCatchupError::Retry(format!("catch-up manifest unavailable: {e}"))
        })?;
        if tip.height <= current_height {
            if tip.height == current_height && host.app_hash() != tip.app_hash {
                return Err(PostRebootCatchupError::Fatal(format!(
                    "catch-up source hash {} at height {} does not match recovered host {}",
                    hex(&tip.app_hash),
                    tip.height,
                    hex(&host.app_hash())
                )));
            }
            tracing::debug!(
                target: "ducktape::statesync",
                from = recovered_height,
                to = current_height,
                frames = total_frames,
                "post-reboot catch-up planned"
            );
            return Ok(PostRebootCatchup {
                from_height: recovered_height,
                to_height: current_height,
                frames: total_frames,
                target: target.or_else(|| {
                    (tip.height == current_height && host.app_hash() == tip.app_hash).then_some(tip)
                }),
                frame_bytes,
                blocks,
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
                return Err(PostRebootCatchupError::RangePruned {
                    target: Box::new(tip),
                    requested_after,
                    retained_from,
                });
            }
            Err(e) => {
                return Err(PostRebootCatchupError::Retry(format!(
                    "catch-up frame suffix unavailable: {e}"
                )));
            }
        };
        let applied = apply_post_reboot_catchup_frames(
            recovery,
            host,
            current_height,
            tip.height,
            frames,
            fold.as_deref_mut(),
        )
        .await
        .map_err(PostRebootCatchupError::Fatal)?;
        if host.app_hash() != tip.app_hash {
            return Err(PostRebootCatchupError::Fatal(format!(
                "catch-up frames landed at {}, target manifest {}",
                hex(&host.app_hash()),
                hex(&tip.app_hash)
            )));
        }
        current_height = tip.height;
        total_frames += applied.applied;
        frame_bytes.extend(applied.frames);
        blocks.extend(applied.blocks);
        target = Some(tip);
    }

    tracing::debug!(
        target: "ducktape::statesync",
        from = recovered_height,
        to = current_height,
        frames = total_frames,
        "post-reboot catch-up planned"
    );
    Ok(PostRebootCatchup {
        from_height: recovered_height,
        to_height: current_height,
        frames: total_frames,
        target,
        frame_bytes,
        blocks,
    })
}

pub(crate) struct BootP2pSyncClient<S, R>
where
    S: P2pSender<PublicKey = ed25519::PublicKey>,
    R: P2pReceiver<PublicKey = ed25519::PublicKey>,
{
    sender: S,
    server: ed25519::PublicKey,
    receiver: std::sync::Arc<tokio::sync::Mutex<Option<R>>>,
    next_id: std::sync::Arc<std::sync::atomic::AtomicU64>,
    /// the real-key standing proof (ADR §5.1). the restore/promoted-boot node's
    /// real key IS in the committed valset (validators ∪ residents), so the
    /// serving peer's fail-closed gate admits it.
    requester: [u8; 32],
    proof: [u8; 64],
}

impl<S, R> Clone for BootP2pSyncClient<S, R>
where
    S: P2pSender<PublicKey = ed25519::PublicKey>,
    R: P2pReceiver<PublicKey = ed25519::PublicKey>,
{
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            server: self.server.clone(),
            receiver: std::sync::Arc::clone(&self.receiver),
            next_id: std::sync::Arc::clone(&self.next_id),
            requester: self.requester,
            proof: self.proof,
        }
    }
}

// the boot client is PINNED to its one sync source, so an honest blob miss
// has nowhere to rotate to — retries re-ask the same (manifest-serving) peer.
impl<S, R> crate::blob_fetch::SourceRotate for BootP2pSyncClient<S, R>
where
    S: P2pSender<PublicKey = ed25519::PublicKey>,
    R: P2pReceiver<PublicKey = ed25519::PublicKey>,
{
}

impl<S, R> BootP2pSyncClient<S, R>
where
    S: P2pSender<PublicKey = ed25519::PublicKey>,
    R: P2pReceiver<PublicKey = ed25519::PublicKey>,
{
    pub(crate) fn new(
        sender: S,
        receiver: R,
        server: ed25519::PublicKey,
        requester: [u8; 32],
        proof: [u8; 64],
    ) -> Self {
        Self {
            sender,
            server,
            receiver: std::sync::Arc::new(tokio::sync::Mutex::new(Some(receiver))),
            next_id: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
            requester,
            proof,
        }
    }

    pub(crate) fn into_parts(self) -> Result<(S, R), String> {
        let Self {
            sender, receiver, ..
        } = self;
        let receiver = std::sync::Arc::try_unwrap(receiver)
            .map_err(|_| "boot statesync client still has live clones".to_string())?
            .into_inner()
            .ok_or_else(|| "boot statesync receiver already taken".to_string())?;
        Ok((sender, receiver))
    }
}

impl<S, R> statesync::SyncClient for BootP2pSyncClient<S, R>
where
    S: P2pSender<PublicKey = ed25519::PublicKey> + Send + Sync + 'static,
    R: P2pReceiver<PublicKey = ed25519::PublicKey> + Send + 'static,
{
    fn request(
        &self,
        req: statesync::SyncRequest,
    ) -> impl std::future::Future<Output = Result<statesync::SyncResponse, statesync::SyncError>> + Send
    {
        let mut sender = self.sender.clone();
        let server = self.server.clone();
        let receiver = std::sync::Arc::clone(&self.receiver);
        let requester = self.requester;
        let proof = self.proof;
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        async move {
            let mut guard = receiver.lock().await;
            let receiver = guard.as_mut().ok_or_else(|| {
                statesync::SyncError::Transport("boot statesync receiver closed".into())
            })?;
            let frame =
                statesync::encode_rpc_authed(&requester, &proof, id, &statesync::encode_request(&req));
            let attempted = sender.send(Recipients::One(server.clone()), IoBuf::from(frame), false);
            if attempted.is_empty() {
                return Err(statesync::SyncError::Transport(
                    "server peer unreachable (send attempted no recipients)".into(),
                ));
            }
            loop {
                let delivered =
                    tokio::time::timeout(BOOT_SYNC_REQUEST_TIMEOUT, receiver.recv()).await;
                let (peer, msg) = match delivered {
                    Ok(Ok(item)) => item,
                    Ok(Err(_)) => {
                        return Err(statesync::SyncError::Transport(
                            "boot statesync channel closed".into(),
                        ));
                    }
                    Err(_) => {
                        return Err(statesync::SyncError::Transport(format!(
                            "boot statesync request {id} timed out"
                        )));
                    }
                };
                if peer != server {
                    continue;
                }
                let bytes: Vec<u8> = msg.into();
                // replies carry zero-filled auth fields; only id + body matter.
                let Ok((_requester, _proof, rpc_id, body)) = statesync::decode_rpc_authed(&bytes)
                else {
                    continue;
                };
                if rpc_id != id {
                    continue;
                }
                return Ok(statesync::decode_response(body)?);
            }
        }
    }
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
