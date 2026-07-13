//! Validator recovery and promoted-node catch-up.
//!
//! The first phase restores or creates the application boundary. The second
//! reconciles a promoted replica with its source before engine resume.

use std::time::Duration;

use commonware_cryptography::{Signer, ed25519};
use commonware_runtime::Clock;

use host::Host;
use recovery::{Manifest, Recovery};

use crate::constants::MAX_PROTOCOL_VERSION;
use crate::constants::{POST_REBOOT_CATCHUP_MAX_ATTEMPTS, POST_REBOOT_CATCHUP_MAX_ITERS};
use crate::explorer::{IndexFold, heal_index};
use crate::host_reads::read_upgrade_version_fields;
use crate::host_state::{NetworkBindings, genesis_host, preflight_recovery_schema, restore_host};
use crate::host_state::{SyncSubstrates, sync_all_modules};
use crate::sync::catchup::{
    BootP2pSyncClient, PostRebootCatchupError, advance_next_seq_from_frames,
    catch_up_post_reboot_frames, write_post_reboot_catchup_checkpoint,
};
use crate::sync::serve::verify_manifest_floor;
use crate::util::hex;

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
                eprintln!(
                    "[node {label}] FATAL: recovery journal exists but the checkpoint is \
                 missing — wipe the app state and re-sync (KEEP the consensus journal \
                 partitions: they are what prevents this key from double-voting)"
                );
                std::process::exit(1);
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
            let (cv, pu) = read_upgrade_version_fields(&host).await;
            let genesis_manifest = match Manifest::capture(
                &host,
                None,
                0,
                0,
                genesis_participants,
                Vec::new(),
                None,
                cv,
                pu,
                pos,
                1,
            ) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("[node {label}] FATAL: genesis checkpoint capture: {e}");
                    std::process::exit(1);
                }
            };
            if let Err(e) = recovery.write_manifest(&genesis_manifest).await {
                eprintln!("[node {label}] FATAL: genesis checkpoint write: {e}");
                std::process::exit(1);
            }
            (host, None, 1, (None, pos), None)
        }
        Some(manifest) => {
            // BOOT PREFLIGHT (design §5 / plan Task 7.3): fail loud EARLY when
            // this binary is too old to apply the blocks at/after the recovered
            // boundary, instead of falling through to an opaque post-replay
            // `AppHashMismatch`. inert on a baseline checkpoint (required_min ==
            // baseline always passes).
            if let Err(e) = manifest.preflight(MAX_PROTOCOL_VERSION) {
                eprintln!(
                    "[node {label}] FATAL: cannot recover — {e} (recovered boundary needs \
                 protocol v{}, this binary supports up to v{MAX_PROTOCOL_VERSION})",
                    manifest.required_min_version
                );
                std::process::exit(1);
            }
            if let Err(e) = preflight_recovery_schema(&manifest) {
                eprintln!("[node {label}] FATAL: cannot recover — {e}");
                std::process::exit(1);
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
                    eprintln!("[node {label}] FATAL: checkpoint restore: {e}");
                    std::process::exit(1);
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
                    eprintln!(
                        "[node {label}] FATAL: {e}\n\
                     [node {label}] app state cannot be locally recovered. wipe the \
                     app-state partitions and re-sync from a peer — but ALWAYS keep \
                     the consensus journal partitions (\"<pubkey>-e<epoch>\"): they \
                     are the anti-equivocation record for this key."
                    );
                    std::process::exit(1);
                }
            };
            // advance the local submit sequence past everything this
            // identity may already have framed: the checkpointed floor,
            // then any retained frame of ours above it.
            let me_bytes = signer.public_key().as_ref().to_vec();
            let mut next_seq = manifest.next_seq;
            advance_next_seq_from_frames(&mut next_seq, &rec.frames, &me_bytes);
            println!(
                "[node {label}] recovered app_hash={} height={} epoch={} (replayed {}, \
             already-on-disk {}{})",
                hex(&rec.app_hash),
                rec.height
                    .map(|h| h.to_string())
                    .unwrap_or_else(|| "genesis".into()),
                rec.epoch,
                rec.applied,
                rec.skipped,
                if rec.rolled_forward {
                    ", rolled 1 forward"
                } else {
                    ""
                },
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
            eprintln!(
                "[node {label}] FATAL: promoted validator has no statesync source for \
                 post-reboot catch-up"
            );
            std::process::exit(1);
        };
        // like the parked joiner's client: the mesh path, over the channel
        // halves handed back to the serve loop once catch-up completes. carry
        // the real-key standing proof (ADR §5.1): this restore/promoted-boot
        // node's key is in the committed valset, so the serving peer admits it.
        let (sync_requester, sync_proof) = statesync::sign_sync_proof(&signer, &namespace);
        let client =
            BootP2pSyncClient::new(sync_tx, sync_rx, server_peer.clone(), sync_requester, sync_proof);
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
                    println!(
                        "[node {label}] post-reboot catch-up {} -> {} ({} frames)",
                        summary.from_height, summary.to_height, summary.frames
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
                            println!(
                                "[node {label}] post-reboot catch-up: the source trails \
                                 the recovered height {recovered_height} — proceeding as \
                                 the freshest member"
                            );
                            break;
                        }
                        eprintln!(
                            "[node {label}] FATAL: post-catch-up target manifest unavailable"
                        );
                        std::process::exit(1);
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
                                eprintln!(
                                    "[node {label}] FATAL: catch-up cutover journal write: {e}"
                                );
                                std::process::exit(1);
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
                                    eprintln!("[node {label}] FATAL: {e}");
                                    std::process::exit(1);
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
                    println!(
                        "[node {label}] post-reboot frame range pruned after \
                         {requested_after} (retained from {retained_from}); full syncing \
                         boundary {}",
                        target.height
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
                                    eprintln!(
                                        "[node {label}] FATAL: full state-sync fallback failed at \
                                         boundary {}: {e}",
                                        target.height
                                    );
                                    std::process::exit(1);
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
                                eprintln!(
                                    "[node {label}] FATAL: full-sync cutover journal write: {e}"
                                );
                                std::process::exit(1);
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
                                target.current_version,
                                target.pending_upgrade.clone(),
                                pos,
                                *next_seq,
                            ) {
                                Ok(m) => m,
                                Err(e) => {
                                    eprintln!(
                                        "[node {label}] FATAL: full-sync checkpoint capture: {e}"
                                    );
                                    std::process::exit(1);
                                }
                            };
                            if let Err(e) = recovery.write_manifest(&ckpt).await {
                                eprintln!("[node {label}] FATAL: full-sync checkpoint write: {e}");
                                std::process::exit(1);
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
                    println!(
                        "[node {label}] post-reboot catch-up unavailable \
                         (attempt {attempts}/{POST_REBOOT_CATCHUP_MAX_ATTEMPTS}): {e}; \
                         retrying"
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
                    eprintln!(
                        "[node {label}] FATAL: post-reboot catch-up unavailable after \
                         {attempts} attempts: {e}"
                    );
                    std::process::exit(1);
                }
                Err(PostRebootCatchupError::Fatal(e)) => {
                    eprintln!("[node {label}] FATAL: post-reboot catch-up failed: {e}");
                    std::process::exit(1);
                }
            }
        }
        match client.into_parts() {
            Ok((tx, rx)) => {
                sync_tx = tx;
                sync_rx = rx;
            }
            Err(e) => {
                eprintln!("[node {label}] FATAL: cannot hand statesync channel to server: {e}");
                std::process::exit(1);
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
        eprintln!(
            "[node {label}] FATAL: {kind} target height {} no longer \
             includes this validator",
            target.height
        );
        std::process::exit(1);
    }
    let floor = match verify_manifest_floor(namespace, target) {
        Ok(floor) => floor,
        Err(e) => {
            eprintln!("[node {label}] FATAL: {kind} target floor verify: {e}");
            std::process::exit(1);
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
            eprintln!("[node {label}] FATAL: {kind} floor-cert write: {e}");
            std::process::exit(1);
        }
    }
    *prev_ckpt = (ckpt.height, ckpt.oplog_pos);
    let refreshed = match recovery.recover_with_sink(host, &ckpt, Some(boot_fold)).await {
        Ok(rec) => rec,
        Err(e) => {
            eprintln!("[node {label}] FATAL: {recover_fatal}: {e}");
            std::process::exit(1);
        }
    };
    let me_bytes = signer.public_key().as_ref().to_vec();
    advance_next_seq_from_frames(next_seq, &refreshed.frames, &me_bytes);
    *resumed = Some(refreshed);
    *recovery_manifest_for_resume = Some(ckpt);
}
