//! Genesis-validator recovery: restore or create the application boundary.

use commonware_cryptography::{Signer, ed25519};

use host::Host;
use recovery::{Manifest, Recovery};

use crate::explorer::{IndexFold, heal_index};
use crate::host_state::{NetworkBindings, genesis_host, restore_host};
use crate::sync::catchup::advance_next_seq_from_frames;
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
                fatal!(
                    label,
                    "recovery journal exists but the checkpoint is \
                 missing — wipe the app state and re-sync (KEEP the consensus journal \
                 partitions: they are what prevents this key from double-voting)"
                );
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
            let restored =
                restore_host(context, forge_repo, duckfs_dir, &manifest, blobs.clone()).await;
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
                heal_index(index, ckpt_height, label);
            }
            let rec = match recovery
                .recover_with_sink(&mut host, &manifest, Some(boot_fold))
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    fatal!(
                        label,
                        "{e}\n\
                     [node {label}] app state cannot be locally recovered. wipe the \
                     app-state partitions and re-sync from a peer — but ALWAYS keep \
                     the consensus journal partitions (\"<pubkey>-e<epoch>\"): they \
                     are the anti-equivocation record for this key."
                    );
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
