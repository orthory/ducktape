//! Validator recovery and promoted-node catch-up.
//!
//! The first phase restores or creates the application boundary. The second
//! reconciles a promoted replica with its source before engine resume.

use std::time::Duration;

use commonware_cryptography::{Signer, ed25519};
use commonware_runtime::Clock;

use host::Host;
use recovery::{Manifest, Recovery};

use crate::constants::{POST_REBOOT_CATCHUP_MAX_ATTEMPTS, POST_REBOOT_CATCHUP_MAX_ITERS};
use crate::explorer::{IndexFold, heal_index};
use crate::host_state::{NetworkBindings, genesis_host, preflight_recovery_schema, restore_host};
use crate::host_state::{SyncSubstrates, sync_all_modules};
use crate::sync::catchup::{
    BootP2pSyncClient, PostRebootCatchupError, advance_next_seq_from_frames,
    catch_up_post_reboot_frames, write_post_reboot_catchup_checkpoint,
};
use crate::sync::serve::verify_manifest_floor;
use crate::util::{fatal, hex};

pub(super) type BootState = (
    Host,
    Option<recovery::Recovered>,
    u64,
    (Option<u64>, u64),
    Option<Manifest>,
);

#[allow(clippy::too_many_arguments)]
pub(super) async fn restore(
    context: &commonware_runtime::tokio::Context,
    index: &indexer::IndexStore,
    blobs: noded::blobs::BlobHandle,
    recovery: &mut Recovery<commonware_runtime::tokio::Context>,
    manifest: Option<Manifest>,
    forge_repo: &std::path::Path,
    duckfs_dir: &std::path::Path,
    validators: &[ed25519::PublicKey],
    namespace: &[u8],
    identity_chain_id: &str,
    signer: &ed25519::PrivateKey,
    label: &str,
    boot_fold: &mut IndexFold<'_>,
) -> BootState {
    match manifest {
        None => {
            // a journal without a checkpoint is damage, not a fresh dir —
            // booting genesis over it would silently fork this node.
            if !recovery.journal_is_empty().await {
                fatal!(label, "recovery journal exists but the checkpoint is \
                 missing — wipe the app state and re-sync (KEEP the consensus journal \
                 partitions: they are what prevents this key from double-voting)");
            }
            let host = genesis_host(
                context,
                forge_repo,
                duckfs_dir,
                validators,
                NetworkBindings {
                    invite: namespace,
                    identity_chain_id,
                },
                blobs.clone(),
            )
            .await;
            let pos = recovery.oplog_pos().await;
            let genesis_participants: Vec<Vec<u8>> =
                validators.iter().map(|k| k.as_ref().to_vec()).collect();
            // seq 0 is the dev demo op's; real submits start at 1.
            let genesis_manifest = match Manifest::capture(
                &host,
                None,
                0,
                0,
                genesis_participants,
                Vec::new(),
                None,
                pos,
                1,
            ) {
                Ok(m) => m,
                Err(e) => {
                    fatal!(label, "genesis checkpoint capture: {e}");
                }
            };
            if let Err(e) = recovery.write_manifest(&genesis_manifest).await {
                fatal!(label, "genesis checkpoint write: {e}");
            }
            (host, None, 1, (None, pos), None)
        }
        Some(manifest) => {
            if let Err(e) = preflight_recovery_schema(&manifest) {
                fatal!(label, "cannot recover — {e}");
            }
            let restored = restore_host(
                context,
                forge_repo,
                duckfs_dir,
                &manifest,
                NetworkBindings {
                    invite: namespace,
                    identity_chain_id,
                },
                blobs.clone(),
            )
            .await;
            let mut host = match restored {
                Ok(h) => h,
                Err(e) => {
                    fatal!(label, "checkpoint restore: {e}");
                }
            };
            // heal the derived index against the CHECKPOINT boundary
            // BEFORE replay: a wiped or trailing per-module database
            // re-derives from the verified checkpoint state, so the
            // journal-suffix fold lands contiguously on top instead of
            // folding forward over a pre-checkpoint hole.
            if let Some(ckpt_height) = manifest.height {
                heal_index(index, &host, ckpt_height, label).await;
            }
            let rec = match recovery
                .recover_with_sink(&mut host, &manifest, Some(boot_fold))
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    fatal!(label, "{e}\n\
                     [node {label}] app state cannot be locally recovered. wipe the \
                     app-state partitions and re-sync from a peer — but ALWAYS keep \
                     the consensus journal partitions (\"<pubkey>-e<epoch>\"): they \
                     are the anti-equivocation record for this key.");
                }
            };
            // advance the local submit sequence past everything this
            // identity may already have framed: the checkpointed floor,
            // then any retained frame of ours above it.
            let me_bytes = signer.public_key().as_ref().to_vec();
            let mut next_seq = manifest.next_seq;
            advance_next_seq_from_frames(&mut next_seq, &rec.frames, &me_bytes);
            tracing::info!(
                target: "ducktape::recovery",
                node = %label,
                root_hash = %hex(&rec.root_hash),
                height = %rec.height.map(|h| h.to_string()).unwrap_or_else(|| "genesis".into()),
                epoch = rec.epoch,
                replayed = rec.applied,
                already_on_disk = rec.skipped,
                rolled_forward = rec.rolled_forward,
                "recovered root_hash={}", hex(&rec.root_hash)
            );
            let prev = (manifest.height, manifest.oplog_pos);
            (host, Some(rec), next_seq, prev, Some(manifest))
        }
    }
}

pub(super) type PostCatchupState<'a> = (
    super::MeshSender,
    super::MeshReceiver,
    Recovery<commonware_runtime::tokio::Context>,
    Host,
    Option<recovery::Recovered>,
    u64,
    (Option<u64>, u64),
    Option<Manifest>,
    IndexFold<'a>,
);

#[allow(clippy::too_many_arguments)]
pub(super) async fn post_reboot_catchup<'a>(
    context: &commonware_runtime::tokio::Context,
    promoted: bool,
    sync_source: Option<ed25519::PublicKey>,
    mut sync_tx: super::MeshSender,
    mut sync_rx: super::MeshReceiver,
    mut recovery: Recovery<commonware_runtime::tokio::Context>,
    mut host: Host,
    mut resumed: Option<recovery::Recovered>,
    mut next_seq: u64,
    mut prev_ckpt: (Option<u64>, u64),
    mut recovery_manifest_for_resume: Option<Manifest>,
    mut boot_fold: IndexFold<'a>,
    signer: ed25519::PrivateKey,
    label: String,
    namespace: Vec<u8>,
    identity_chain_id: String,
    validators: Vec<ed25519::PublicKey>,
    forge_repo: std::path::PathBuf,
    duckfs_dir: std::path::PathBuf,
    blobs: noded::blobs::BlobHandle,
) -> PostCatchupState<'a> {
    let promoted_validator_boot = promoted && !validators.contains(&signer.public_key());
    if promoted_validator_boot {
        let Some(server_peer) = sync_source else {
            fatal!(label, "promoted validator has no statesync source for \
                 post-reboot catch-up");
        };
        // like the parked joiner's client: the mesh path, over the channel
        // halves handed back to the serve loop once catch-up completes. carry
        // the real-key standing proof (ADR §5.1): this restore/promoted-boot
        // node's key is in the committed valset, so the serving peer admits it.
        let (sync_requester, sync_proof) = statesync::sign_sync_proof(&signer, &namespace);
        let client =
            BootP2pSyncClient::new(sync_tx, sync_rx, server_peer.clone(), sync_requester, sync_proof);
        // while catch-up replays, realize code-registry swaps through a source
        // that can FETCH a missing committed component from the serve peer
        // (ranged, verified) instead of failing closed on the local store —
        // this binary's embedded components may trail (or lead) the registry.
        recovery.set_code_source(std::sync::Arc::new(crate::blob_fetch::FetchingCodeSource::new(
            blobs.clone(),
            client.clone(),
            crate::constants::MAX_MODULE_CODE_BYTES,
            crate::constants::BLOB_FETCH_ATTEMPTS,
        )));
        let mut attempts = 0usize;
        loop {
            attempts += 1;
            let recovered_height = resumed.as_ref().and_then(|rec| rec.height).unwrap_or(0);
            match catch_up_post_reboot_frames(
                &client,
                &mut recovery,
                &mut host,
                Some(&mut boot_fold),
                recovered_height,
                POST_REBOOT_CATCHUP_MAX_ITERS,
            )
            .await
            {
                Ok(summary) => {
                    tracing::info!(
                        target: "ducktape::statesync",
                        node = %label,
                        from_height = summary.from_height,
                        to_height = summary.to_height,
                        frames = summary.frames,
                        "post-reboot catch-up {} -> {} ({} frames)",
                        summary.from_height,
                        summary.to_height,
                        summary.frames
                    );
                    let Some(target) = summary.target.as_ref() else {
                        if summary.to_height == recovered_height {
                            // the source trails us: a quorum-widening
                            // cutover halts the chain awaiting this very
                            // node's votes, and a promoted replica boots
                            // at its own folded tip — ahead of anything
                            // the halted source can serve. the recovered
                            // state is journal-proven; seat ourselves and
                            // the chain resumes.
                            tracing::warn!(
                                target: "ducktape::statesync",
                                node = %label,
                                recovered_height,
                                reason = "source_trails_recovered_state",
                                "proceeding as the freshest member"
                            );
                            break;
                        }
                        fatal!(label, "post-catch-up target manifest unavailable");
                    };
                    let resumed_epoch = resumed.as_ref().map(|rec| rec.epoch).unwrap_or(0);
                    finalize_catchup_boundary(
                        &mut recovery,
                        &mut host,
                        &mut next_seq,
                        &mut prev_ckpt,
                        &mut resumed,
                        &mut recovery_manifest_for_resume,
                        &mut boot_fold,
                        &signer,
                        &label,
                        &namespace,
                        target,
                        "catch-up",
                        "post-catch-up checkpoint recovery",
                        async |recovery: &mut Recovery<commonware_runtime::tokio::Context>,
                               host: &mut Host,
                               next_seq: &mut u64,
                               base_manifest: Option<&Manifest>| {
                            if target.epoch > resumed_epoch
                                && let Err(e) = node::BlockSink::cutover(
                                    recovery,
                                    target.epoch,
                                    target.view_base,
                                    &target.participants,
                                    &target.residents,
                                )
                                .await
                            {
                                fatal!(label, "catch-up cutover journal write: {e}");
                            }
                            let me_bytes = signer.public_key().as_ref().to_vec();
                            advance_next_seq_from_frames(
                                next_seq,
                                &summary.frame_bytes,
                                &me_bytes,
                            );
                            match write_post_reboot_catchup_checkpoint(
                                recovery,
                                host,
                                base_manifest,
                                target,
                                &summary.blocks,
                                *next_seq,
                            )
                            .await
                            {
                                Ok(ckpt) => ckpt,
                                Err(e) => {
                                    fatal!(label, "{e}");
                                }
                            }
                        },
                    )
                    .await;
                    break;
                }
                Err(PostRebootCatchupError::RangePruned {
                    target,
                    requested_after,
                    retained_from,
                }) => {
                    tracing::warn!(
                        target: "ducktape::statesync",
                        node = %label,
                        requested_after,
                        retained_from,
                        target_height = target.height,
                        reason = "range_pruned",
                        "post-reboot frame range pruned; full-syncing the boundary"
                    );
                    let resumed_epoch = resumed.as_ref().map(|rec| rec.epoch).unwrap_or(0);
                    finalize_catchup_boundary(
                        &mut recovery,
                        &mut host,
                        &mut next_seq,
                        &mut prev_ckpt,
                        &mut resumed,
                        &mut recovery_manifest_for_resume,
                        &mut boot_fold,
                        &signer,
                        &label,
                        &namespace,
                        &target,
                        "full-sync",
                        "full-sync recovery refresh",
                        async |recovery: &mut Recovery<commonware_runtime::tokio::Context>,
                               host: &mut Host,
                               next_seq: &mut u64,
                               _base_manifest: Option<&Manifest>| {
                            let synced = match sync_all_modules(
                                context,
                                &client,
                                &target,
                                NetworkBindings {
                                    invite: &namespace,
                                    identity_chain_id: &identity_chain_id,
                                },
                                SyncSubstrates {
                                    forge_repo: &forge_repo,
                                    duckfs_dir: &duckfs_dir,
                                    blobs: blobs.clone(),
                                },
                                10_000 + attempts,
                            )
                            .await
                            {
                                Ok(host) => host,
                                Err(e) => {
                                    fatal!(label, "full state-sync fallback failed at \
                                         boundary {}: {e}",
                                        target.height);
                                }
                            };
                            *host = synced;
                            if target.epoch > resumed_epoch
                                && let Err(e) = node::BlockSink::cutover(
                                    recovery,
                                    target.epoch,
                                    target.view_base,
                                    &target.participants,
                                    &target.residents,
                                )
                                .await
                            {
                                fatal!(label, "full-sync cutover journal write: {e}");
                            }
                            let pos = recovery.oplog_pos().await;
                            let ckpt = match Manifest::capture(
                                host,
                                Some(target.height),
                                target.epoch,
                                target.view_base,
                                target.participants.clone(),
                                target.residents.clone(),
                                None,
                                pos,
                                *next_seq,
                            ) {
                                Ok(m) => m,
                                Err(e) => {
                                    fatal!(label, "full-sync checkpoint capture: {e}");
                                }
                            };
                            if let Err(e) = recovery.write_manifest(&ckpt).await {
                                fatal!(label, "full-sync checkpoint write: {e}");
                            }
                            ckpt
                        },
                    )
                    .await;
                    break;
                }
                Err(PostRebootCatchupError::Retry(e))
                    if attempts < POST_REBOOT_CATCHUP_MAX_ATTEMPTS =>
                {
                    tracing::warn!(
                        target: "ducktape::statesync",
                        node = %label,
                        attempts,
                        max_attempts = POST_REBOOT_CATCHUP_MAX_ATTEMPTS,
                        error = %e,
                        "post-reboot catch-up unavailable; retrying"
                    );
                    // escalate toward a 5s beat: an overlay-only source
                    // (a fully-NATed inviter) is reachable only once the
                    // reachability plane's tunnels assemble, which can
                    // take a while after a promotion reboot — a restart
                    // would not arrive any sooner, it would just redo the
                    // plane restore from zero.
                    let beat = Duration::from_millis(500)
                        .saturating_mul(attempts as u32)
                        .min(Duration::from_secs(5));
                    context.sleep(beat).await;
                }
                Err(PostRebootCatchupError::Retry(e)) => {
                    fatal!(label, "post-reboot catch-up unavailable after \
                         {attempts} attempts: {e}");
                }
                Err(PostRebootCatchupError::Fatal(e)) => {
                    fatal!(label, "post-reboot catch-up failed: {e}");
                }
            }
        }
        // restore the local-only source BEFORE reclaiming the channel: the
        // fetching source above holds a clone of the client, and into_parts
        // refuses while clones live. the runtime wiring installs the serve-
        // lane fetching source right after boot.
        recovery.set_code_source(std::sync::Arc::new(crate::host_state::BlobCodeSource(
            std::sync::Arc::new(blobs.clone()),
        )));
        match client.into_parts() {
            Ok((tx, rx)) => {
                sync_tx = tx;
                sync_rx = rx;
            }
            Err(e) => {
                fatal!(label, "cannot hand statesync channel to server: {e}");
            }
        }
    }
    (
        sync_tx,
        sync_rx,
        recovery,
        host,
        resumed,
        next_seq,
        prev_ckpt,
        recovery_manifest_for_resume,
        boot_fold,
    )
}

/// the shared finalize sequence both catch-up arms settle a boundary through:
/// verify the target still seats this validator, verify the target's floor
/// certificate, run the arm-specific middle (`make_ckpt`: frames-replay writes
/// the catch-up checkpoint; the pruned arm full-syncs, then captures its own —
/// both cut the cutover journal record before their checkpoint write), write
/// the floor cert, and re-recover from the fresh checkpoint. every failure is
/// the same FATAL exit the arms carried inline; `kind` and `recover_fatal`
/// keep each arm's error wording byte-identical.
#[allow(clippy::too_many_arguments)]
async fn finalize_catchup_boundary<F>(
    recovery: &mut Recovery<commonware_runtime::tokio::Context>,
    host: &mut Host,
    next_seq: &mut u64,
    prev_ckpt: &mut (Option<u64>, u64),
    resumed: &mut Option<recovery::Recovered>,
    recovery_manifest_for_resume: &mut Option<Manifest>,
    boot_fold: &mut IndexFold<'_>,
    signer: &ed25519::PrivateKey,
    label: &str,
    namespace: &[u8],
    target: &statesync::Manifest,
    kind: &str,
    recover_fatal: &str,
    make_ckpt: F,
) where
    F: AsyncFnOnce(
        &mut Recovery<commonware_runtime::tokio::Context>,
        &mut Host,
        &mut u64,
        Option<&Manifest>,
    ) -> Manifest,
{
    if !target
        .participants
        .iter()
        .any(|key| key.as_slice() == signer.public_key().as_ref())
    {
        fatal!(label, "{kind} target height {} no longer \
             includes this validator",
            target.height);
    }
    let floor = match verify_manifest_floor(namespace, target) {
        Ok(floor) => floor,
        Err(e) => {
            fatal!(label, "{kind} target floor verify: {e}");
        }
    };
    let ckpt = make_ckpt(
        recovery,
        host,
        next_seq,
        recovery_manifest_for_resume.as_ref(),
    )
    .await;
    if let Some(cert) = floor {
        let floor = recovery::FloorCert {
            epoch: target.epoch,
            height: target.height,
            cert,
        };
        if let Err(e) = recovery.write_floor_cert(&floor).await {
            fatal!(label, "{kind} floor-cert write: {e}");
        }
    }
    *prev_ckpt = (ckpt.height, ckpt.oplog_pos);
    let refreshed = match recovery.recover_with_sink(host, &ckpt, Some(boot_fold)).await {
        Ok(rec) => rec,
        Err(e) => {
            fatal!(label, "{recover_fatal}: {e}");
        }
    };
    let me_bytes = signer.public_key().as_ref().to_vec();
    advance_next_seq_from_frames(next_seq, &refreshed.frames, &me_bytes);
    *resumed = Some(refreshed);
    *recovery_manifest_for_resume = Some(ckpt);
}
