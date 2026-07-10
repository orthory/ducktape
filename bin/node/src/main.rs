//! a runnable multi-process ducktape node over REAL sockets.
//!
//! this is the in-sim N-validator simplex proof (consensus/tests/
//! simplex_agreed_order.rs) turned into an actual network: instead of N
//! `SimplexOrderer`s over ONE `p2p::simulated` network under the DETERMINISTIC
//! clock, each process here stands up its OWN live simplex `Engine` over a real
//! `authenticated::discovery` encrypted TCP mesh on the REAL tokio runtime, and
//! drives an `OrderedNode<SimplexOrderer>` over a `host::Host`.
//!
//! the machinery is REUSED verbatim: `consensus::SimplexOrderer::spawn` is
//! already generic over the runtime context + the three engine channel pairs, so
//! the only substrate that changes vs the sim is (a) `tokio::Runner` instead of
//! `deterministic::Runner` (discovery live-locks under the deterministic clock),
//! (b) `discovery::Network` channels instead of `simulated::Network`, and (c) a
//! per-process `ContentStore`.
//!
//! payload dissemination is REAL: each process submits a DISTINCT op (node N
//! writes directory key `kN`), so a peer that finalizes another node's op-digest
//! has NO local bytes for it. `SimplexOrderer::spawn_with_resolver` wires a
//! `ConsensusRelay` that, at propose time, gossips the proposed frame's bytes to
//! all peers on the payload channel; every peer's STORE-ONLY drain caches them, so
//! when that digest finalizes the reporter resolves it locally and delivers it in
//! BFT order. content-addressing IS the verification (the drain re-hashes on
//! receipt). the relay gossip is one-shot, and quorum is a SUBSET — a validator
//! can finalize a view whose gossip it missed — so a lazy payload FETCH lane
//! backstops it: the resolver pulls missing bytes by digest from the tracked
//! mesh and fills the finalized slot instead of wedging the apply prefix. this
//! is what lets DISTINCT ops converge across processes with per-process stores
//! — quorum votes still cross the real TCP mesh to finalize.
//!
//! ## state-sync service and the sync-only joiner
//!
//! every validator also serves the statesync wire protocol on
//! `CHANNEL_STATE_SYNC`, answered between drains from its latest finalized
//! boundary — so responses are always block-consistent without locks. run with
//! `--sync-only` and the process joins the mesh WITHOUT a consensus engine,
//! pulls a manifest + every module from the bootstrapper over that channel,
//! rebuilds them against their consensus-committed roots, prints its composed
//! `synced app_hash=`, and exits 0 — the network-backed joiner path over real
//! sockets. membership note: `peer_seeds` is the AUTHORIZED MESH (everyone,
//! including sync-only joiners); `validator_seeds` (default: peer_seeds) is the
//! CONSENSUS participant set — the split that lets a non-validator sync.
//!
//! each validator prints its GENESIS app-hash at startup and its CONVERGED
//! app-hash once it has applied ALL validator ops. the demo script asserts every
//! process's genesis line agrees, every converged line agrees, and the sync-only
//! joiner's synced line equals the converged line.

use std::path::PathBuf;
use std::time::Duration;

use commonware_codec::DecodeExt as _;
use commonware_consensus::simplex::scheme::ed25519 as simplex_ed25519;
use commonware_consensus::types::Epoch;
use commonware_cryptography::{Signer, ed25519};
use commonware_p2p::{Ingress, Manager, Receiver as P2pReceiver, Recipients, Sender as P2pSender};
use commonware_runtime::{Clock, IoBuf, Metrics, Runner, Spawner, Supervisor};
use commonware_utils::ordered::Set;
use futures::{FutureExt as _, StreamExt as _};
use tracing_subscriber::prelude::*;

use consensus::{ConsensusScheme, ContentStore, SimplexOrderer};

mod blob_fetch;
mod boot;
mod cli;
mod cli_flags;
mod config;
mod constants;
mod explorer;
mod first_contact_join;
mod host_reads;
mod host_state;
#[cfg(test)]
mod joiner_mesh_tests;
mod lobby;
#[cfg(test)]
mod main_tests;
mod oracle_pool;
mod reachability_plane;
mod relay;
mod relay_runtime;
mod replica;
mod resident_announce;
mod resident_dispatch;
mod resource_limits;
mod rpc;
mod statesync_plane;
mod sync;
mod userkey;
mod userkey_cli;
mod util;
mod validator;
mod voice;
mod voice_plane;
use config::{Resolved, hex_bytes, unhex};
use constants::*;
use explorer::{IndexFold, explorer_root_op, heal_index, ship_index_blobs};
#[cfg(test)]
use explorer::sealed_frame_block_row;
use host_reads::{
    read_members_from_host, read_redemptions_from_host, read_upgrade_state,
    read_upgrade_status_raw, read_upgrade_version_fields, read_valset_members,
    read_valset_residents, resume_member_keys, resume_resident_keys,
};
use host_state::{
    NetworkBindings, SyncSubstrates, genesis_host, restore_host, run_output_sink, sync_all_modules,
};
use reachability_plane::wire_reachability_plane;
#[cfg(test)]
use replica::promotion::{
    choose_promotion_boundary, joiner_manifest_fetch_retry, PromotionBoundary,
    PromotionBoundarySource,
};
use rpc::{spawn_rpc_listener, JoinRequestRecord, JoinRequestView, RpcJob, RpcReply, RpcRequest, RpcStatus};
use sync::catchup::{
    advance_next_seq_from_frames, catch_up_post_reboot_frames, derive_pending_boot,
    write_post_reboot_catchup_checkpoint, BootP2pSyncClient, PostRebootCatchupError,
};
#[cfg(test)]
use sync::catchup::{apply_post_reboot_catchup_frames, apply_verified_suffix_frame};
use sync::serve::{drive_sync_request, verify_manifest_floor, SyncBoundary, SyncStateRequest};
#[cfg(test)]
use sync::serve::assert_floor_binds_view;
use util::{diag_log, epoch_floor, hex, participant_bytes, resident_bytes, unix_ms};
use validator::announce::{
    CapabilityAnnouncer, ReadinessSignaller, dispatch_pending_deliveries, saga_next_expiry,
};

use directory::{DirMsg, DirQuery, DirReply, decode_reply, encode_msg, encode_query};
use duckfs_disk::SyncScratch;
use host::Host;
use node::OrderedNode;
use recovery::{Manifest, Recovery};
use sdk::{Msg, StateRoot};
use statesync::SyncServer;

fn main() {
    resource_limits::raise_open_file_limit();
    // Convert any terminal error into the same stable `FATAL:` marker the node
    // already prints for its other fatal paths (recovery, admission, promotion),
    // plus a non-zero exit. This closes the run-path boot failures (bind
    // conflict, config parse) that used to propagate as a bare `Error: …` the
    // desktop app's classify() didn't recognize — now the app surfaces the
    // reason immediately instead of inferring death. (Onboarding subcommands
    // still surface their own stderr via run_verb; the prefix is harmless there.)
    if let Err(err) = run() {
        eprintln!("FATAL: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(command) = args.first()
        && let Some(result) = cli::dispatch(command, &args[1..])
    {
        return result;
    }

    // the run path: `--config <path> | -n/--network <chain id> [--sync-only]`.
    let mut cfg_path: Option<PathBuf> = None;
    let mut network: Option<String> = None;
    let mut sync_only = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--config" => cfg_path = it.next().map(PathBuf::from),
            "-n" | "--network" => network = it.next().cloned(),
            "--sync-only" => sync_only = true,
            other => {
                return Err(format!(
                    "unexpected arg {other:?} (want a subcommand — \
                     keygen|user-key|user-sign-bind|user-sign-unbind|\
                     user-sign-possession|user-sign-add-member|user-sign-remove-member|\
                     user-webauthn-challenge|user-p256-payload|\
                     init|invite|admit|\
                     invite-accept|promote|resident-remove|\
                     join-requests|member-remove|member-leave|member-status|join|\
                     upgrade-status — or \
                     --config <path> | -n/--network <chain id> [--sync-only])"
                )
                .into());
            }
        }
    }
    // `--network` addresses a workspace by its chain id through the registry;
    // `--config` stays the explicit path. exactly one selects the node.
    let cfg_path = match (network, cfg_path) {
        (Some(needle), None) => config::find_workspace_config(&needle)?,
        (None, Some(path)) => path,
        (Some(_), Some(_)) => {
            return Err("pass either --network <chain id> or --config <path>, not both".into());
        }
        (None, None) => {
            return Err("missing --config <path> (or -n/--network <chain id>)".into());
        }
    };

    let log_ring = noded::LogRing::default();
    init_tracing(log_ring.clone());

    run_node(config::resolve(&cfg_path)?, sync_only, log_ring)
}

fn init_tracing(log_ring: noded::LogRing) {
    // opt-in internals visibility: RUST_LOG=commonware_p2p=debug etc.
    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(tracing_subscriber::EnvFilter::from_default_env());
    // the stream's `logs` topic: info floor by default so hot-path debug/trace
    // events never pay per-event formatting into the ring; RUST_LOG overrides.
    let ring_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_writer(log_ring)
        .with_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        );
    let _ = tracing_subscriber::registry()
        .with(stderr_layer)
        .with(ring_layer)
        .try_init();
}

/// stand up the real-socket node from `cfg` and run it until killed (validator)
/// or until state sync completes (`--sync-only`).
///
/// deliberately NOT `#[tokio::main]`: `tokio::Runner` owns its OWN tokio runtime,
/// and you cannot start a runtime from inside one. so `main` is sync and hands
/// off to `Runner::start`, which drives everything (including the engine's spawned
/// tasks) on the runtime it owns.
fn run_node(
    resolved: Resolved,
    sync_only: bool,
    log_ring: noded::LogRing,
) -> Result<(), Box<dyn std::error::Error>> {
    let boot::env::BootEnv {
        signer,
        label,
        namespace,
        identity_chain_id,
        peers,
        validators,
        bootstrappers,
        coordinated,
        listen,
        advertised,
        storage,
        rpc_listen,
        http_listen,
        wireguard_listen,
        wireguard_effect,
        wireguard_key_file,
        invite_listen,
        invite_token,
        invite_wireguard,
        invite_fronts,
        coordination,
        coord_cap,
        workspace,
        primary_coordinator,
        wireguard_advertised,
        sync_candidates,
        chain_id,
        mesh_state_file,
        checkpoint_blocks,
        dev_demo,
        sync_index,
        announce_capabilities,
        promoted,
        joiner,
    } = boot::env::derive(resolved, sync_only);

    let boot::surfaces::Surfaces {
        rpc_listener,
        http_cmds,
        stream_hub,
        index,
        voice_requests,
        blobs,
        agent_provisioner,
    } = boot::surfaces::bind(
        sync_only,
        &label,
        &storage,
        rpc_listen,
        http_listen,
        log_ring,
    )?;

    // run on commonware's OWN tokio runtime, rooted at our per-process storage dir.
    let storage_for_sync = storage.clone();
    // per-agent host state, rooted OUTSIDE <storage> (D7 isolation floor): the
    // persistent executor workspaces + session files must NOT be descendants of
    // the key/consensus/blob tree, so a `..` from a run's cwd can't reach
    // user.key/node keys/qmdb/blobstore. `DUCKTAPE_AGENT_WORKSPACES` / _SESSIONS
    // override — see capability-host. host-local only, never consensus.
    // non-portable (v2/persistent) agent workspaces stay under <storage>, exactly
    // as today — relocating them would be a live (non-dormant) durability change.
    // D7 relocation applies to the PORTABLE provisioner mount (agent_runs_root),
    // which is out of <storage>; the pre-existing non-portable D7 gap is a
    // separate, migration-aware hardening (tracked as a follow-up).
    let agent_dirs = capability_host::AgentDirs::under(&storage);
    let rt_cfg = commonware_runtime::tokio::Config::default().with_storage_directory(storage);
    let executor = commonware_runtime::tokio::Runner::new(rt_cfg);

    // the seam's stack handle (socket mode): one slot for the process,
    // created HERE so every consumer — the mesh context's backend, the
    // statesync plane's socket factory, and the reachability plane's effect
    // (which owns the writes) — holds the same one. in tun/fake mode it just
    // stays empty.
    let overlay_slot = overlay_net::userspace::StackSlot::new();

    executor.start(|context| async move {
        let boot::mesh::MeshHead {
            context,
            metrics,
            mesh_participants,
            status_public_key,
            sync_sources,
            sync_source,
            advertised_reach,
            mut network,
            mut oracle,
            quota,
        } = boot::mesh::build(
            context,
            signer.clone(),
            namespace.clone(),
            peers.clone(),
            validators.clone(),
            sync_candidates,
            joiner,
            listen,
            advertised,
            bootstrappers,
            wireguard_effect,
            overlay_slot.clone(),
        );

        if sync_only {
            boot::sync_only::run(
                context,
                &label,
                network,
                oracle,
                quota,
                mesh_participants,
                sync_sources,
                storage_for_sync,
                namespace,
                identity_chain_id,
                blobs,
                voice_requests,
            )
            .await;
            return;
        }

        // ---- a VALIDATOR: consensus engine + state-sync service -------------

        // recovery-aware boot FIRST: the app state (and with it the epoch to
        // respawn) must be known before the mesh wiring below decides which
        // epochs' channels to live on. everything here is local disk io.
        let mut recovery = match Recovery::open(context.child("recovery")).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[node {label}] FATAL: cannot open the recovery store: {e}");
                std::process::exit(1);
            }
        };
        let manifest = match recovery.manifest() {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[node {label}] FATAL: recovery checkpoint is damaged: {e}");
                std::process::exit(1);
            }
        };
        let forge_repo = storage_for_sync.join("forge-repo");
        let duckfs_dir = storage_for_sync.join("duckfs");
        // boot sweep (#219): no sync attempt is in flight yet, so any leftover
        // `duckfs_scratch_a*` dir (a crashed attempt, or a promoted scratch
        // whose final removal was interrupted) is safe to remove. best-effort.
        SyncScratch::sweep_stale(&duckfs_dir);

        // ---- the JOINER / REPLICA: park on the mesh, bootstrap a boundary,
        // then FOLD the head (unified-node phase 2) ----
        //
        // decided from the REAL store (the pre-runtime probe only gated
        // listeners): a key outside the genesis set that no checkpoint seats
        // as a participant. a fresh join has no checkpoint at all; a
        // RESTARTED replica has one that names it a resident — it re-enters
        // here and re-ascends (a fresh bootstrap into its existing journal;
        // recovering the folded state by journal replay instead is the
        // remaining phase-2 follow-up). after PROMOTION the checkpoint
        // seats this key, so a rebooted process falls through to the
        // validator path below.
        let checkpoint_seats_me = manifest.as_ref().is_some_and(|m| {
            m.participants
                .iter()
                .any(|k| k.as_slice() == signer.public_key().as_ref())
        });
        if !checkpoint_seats_me && !validators.contains(&signer.public_key()) {
            replica::run(
                context,
                network,
                &mut oracle,
                quota,
                &mesh_participants,
                sync_sources,
                sync_source,
                advertised_reach,
                status_public_key,
                signer,
                label,
                namespace,
                identity_chain_id,
                peers,
                validators,
                wireguard_listen,
                wireguard_effect,
                wireguard_key_file,
                primary_coordinator,
                wireguard_advertised,
                &invite_token,
                &invite_wireguard,
                invite_fronts,
                &coord_cap,
                workspace,
                chain_id,
                mesh_state_file,
                checkpoint_blocks,
                sync_index,
                announce_capabilities,
                rpc_listener,
                http_cmds,
                &stream_hub,
                index,
                voice_requests,
                blobs,
                &agent_provisioner,
                &agent_dirs,
                overlay_slot,
                storage_for_sync,
                recovery,
                &manifest,
                forge_repo,
                duckfs_dir,
            )
            .await;
        }
        // (host, recovered-state, next local submit seq, last checkpoint
        // ONE index fold for the whole boot (journal replay + post-reboot
        // catch-up + post-sync refreshes): its stop flag must persist across
        // phases — a later phase folding past a gap an earlier phase detected
        // would advance watermarks over the hole and hide it from the final
        // heal below.
        let mut boot_fold = IndexFold::new(&index, blobs.clone());
        // (height, oplog position) for the pump's prune bookkeeping, and the
        // manifest that recovery used as its replay baseline).
        type BootState = (
            Host,
            Option<recovery::Recovered>,
            u64,
            (Option<u64>, u64),
            Option<Manifest>,
        );
        let (
            mut host,
            mut resumed,
            mut next_seq,
            mut prev_ckpt,
            mut recovery_manifest_for_resume,
        ): BootState = match manifest.clone() {
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
                    &context,
                    &forge_repo,
                    &duckfs_dir,
                    &validators,
                    NetworkBindings {
                        invite: &namespace,
                        identity_chain_id: &identity_chain_id,
                    },
                    blobs.clone(),
                )
                .await;
                let pos = recovery.oplog_pos().await;
                let genesis_participants: Vec<Vec<u8>> =
                    validators.iter().map(|k| k.as_ref().to_vec()).collect();
                // seq 0 is the dev demo op's; real submits start at 1.
                let (cv, pu) = read_upgrade_version_fields(&host).await;
                let genesis_manifest =
                    match Manifest::capture(
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
                    )
                    {
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
                        manifest.required_min_version()
                    );
                    std::process::exit(1);
                }
                let restored = restore_host(
                    &context,
                    &forge_repo,
                    &duckfs_dir,
                    &manifest,
                    NetworkBindings {
                        invite: &namespace,
                        identity_chain_id: &identity_chain_id,
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
                    heal_index(&index, &host, ckpt_height, &label).await;
                }
                let rec = match recovery
                    .recover_with_sink(&mut host, &manifest, Some(&mut boot_fold))
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
                    rec.height.map(|h| h.to_string()).unwrap_or_else(|| "genesis".into()),
                    rec.epoch,
                    rec.applied,
                    rec.skipped,
                    if rec.rolled_forward { ", rolled 1 forward" } else { "" },
                );
                let prev = (manifest.height, manifest.oplog_pos);
                (host, Some(rec), next_seq, prev, Some(manifest))
            }
        };

        // consensus membership comes from the RECOVERY RECORD: the epoch's
        // ENGINE PARTICIPANT SET (at genesis: exactly the config seed). the
        // recovered valset projection is NOT it — a restart inside a cutover
        // window would read a membership change whose boundary has not been
        // crossed and spawn a different scheme than its peers are running.
        let initial_member_keys = match resume_member_keys(resumed.as_ref(), &validators) {
            Ok(keys) => keys,
            Err(e) => {
                eprintln!("[node {label}] FATAL: {e}");
                std::process::exit(1);
            }
        };
        if !initial_member_keys.contains(&signer.public_key()) {
            println!(
                "[node {label}] this identity is not in the recovered validator set — \
                 halting (restart with --sync-only to observe)"
            );
            std::process::exit(0);
        }
        let initial_resume_epoch = resumed.as_ref().map(|r| r.epoch).unwrap_or(0);

        // the TRANSPORT baseline adds the committed RESIDENT set (granted,
        // quorum-exempt keys the mesh must admit so they can sync). read
        // LIVE from the recovered host, unlike the frozen participant set
        // above: a resident grant arms its own cutover, so within any epoch
        // the resident set is constant — except a reboot inside that cutover
        // window, where this node briefly tracks the wider set alone; the
        // boundary re-tracks identically a few views later.
        let initial_resident_keys: Vec<ed25519::PublicKey> = read_valset_residents(&host)
            .await
            .iter()
            .filter_map(|key| ed25519::PublicKey::decode(key.as_slice()).ok())
            .collect();

        // the validator-owned transport mesh, tracked at index = epoch: the
        // epoch's TRANSPORT members (participants ∪ standby registrants) ∪
        // the descriptor mesh (genesis members + [dev] extras — kept
        // authorized so demoted members and pre-genesis peers can still
        // reach the statesync service). the SAME set on every node at this
        // index: discovery kills peers whose bit-vector length disagrees at
        // a shared index, and boundary-read membership is the only set every
        // node agrees on epoch-for-epoch.
        let mesh_at = {
            let descriptor_mesh = peers.clone();
            move |epoch_members: &std::collections::BTreeSet<ed25519::PublicKey>| {
                let mut union: std::collections::BTreeSet<ed25519::PublicKey> =
                    descriptor_mesh.iter().cloned().collect();
                union.extend(epoch_members.iter().cloned());
                Set::try_from(union.into_iter().collect::<Vec<_>>())
                    .expect("a btree-set union has no duplicates")
            }
        };
        let mut mesh_oracle = oracle.clone();
        mesh_oracle.track(
            initial_resume_epoch,
            mesh_at(
                &initial_member_keys
                    .iter()
                    .chain(initial_resident_keys.iter())
                    .cloned()
                    .collect(),
            ),
        );

        // lanes for epochs BELOW the resume epoch are registered and
        // black-holed (the sync-only arm's exact trick): a lagging peer still
        // gossips there, and an unregistered channel is a protocol violation
        // that would kill its connection — cutting off the very fetch lane it
        // needs to catch up.
        for epoch in 0..initial_resume_epoch {
            let (vote, cert, res, payload, fetch) = engine_channels(epoch);
            for ch in [vote, cert, res, payload, fetch] {
                let (_tx, mut rx) = network.register(ch, quota, MAX_BACKLOG);
                let label: &'static str = Box::leak(format!("blackhole_{ch}").into_boxed_str());
                context.child(label).spawn(move |_ctx| async move {
                    while rx.recv().await.is_ok() {}
                });
            }
        }

        // pre-register the epoch channel bank from the RESUME epoch up
        // (registration is only possible before network.start(); every
        // respawned engine needs fresh channels). bank[i] holds epoch
        // (bank_base + i)'s (vote, certificate, resolver, payload, fetch)
        // pairs until that epoch's engine consumes them. a restart therefore
        // re-arms the full window — EPOCH_CHANNEL_BANK bounds membership
        // changes per process RUN, not per network lifetime.
        let bank_base = initial_resume_epoch;
        let mut channel_bank: Vec<Option<_>> = (0..EPOCH_CHANNEL_BANK)
            .map(|i| {
                let (vote, cert, res, payload, fetch) = engine_channels(bank_base + i);
                Some((
                    network.register(vote, quota, MAX_BACKLOG),
                    network.register(cert, quota, MAX_BACKLOG),
                    network.register(res, quota, MAX_BACKLOG),
                    network.register(payload, quota, MAX_BACKLOG),
                    network.register(fetch, quota, MAX_BACKLOG),
                ))
            })
            .collect();
        let (mut sync_tx, mut sync_rx) = network.register(CHANNEL_STATE_SYNC, quota, MAX_BACKLOG);
        // the lobby lane: parked joiners announce their keys here (connected
        // as the derived lobby identity); this member verifies each announce
        // against the invite token it carries and RECORDS it for approval.
        let (mut lobby_tx, lobby_rx) = network.register(CHANNEL_LOBBY, quota, MAX_BACKLOG);
        // the submit-relay lane: a resident-standing node ships its own
        // signed frame here; this validator takes custody and answers on
        // drain/expiry. bound `mut` because the pump uses `relay_tx` from BOTH
        // the ingress select arm and the drain-resolution/expiry code.
        let (mut relay_tx, relay_rx) = network.register(CHANNEL_SUBMIT_RELAY, quota, MAX_BACKLOG);

        // the voice + video hub: huddle media between members. per the
        // per-use data-plane ADR (docs/adr/2026-07-07-per-use-data-plane.mdx),
        // media rides the OVERLAY — audio+control on Service::Voice's overlay
        // socket (45902), camera on Service::Video's (45903) — never the mesh.
        let media_peers = {
            // media needs the overlay: with no overlay (fake effect, or the
            // reachability plane unconfigured) there is no media transport at
            // all (the overlay-only cutover — no mesh fallback), so drop the
            // session lane and huddle joins refuse fast instead of hanging.
            let overlay_capable = wireguard_listen.is_some()
                && !matches!(wireguard_effect, config::WireGuardEffectKind::Fake);
            if overlay_capable {
                // tracked media set = transport members ∪ residents, refreshed
                // on every valset cutover (below, beside the statesync book).
                let peers = voice_plane::MediaPeers::new(
                    String::from_utf8(namespace.clone()).expect("namespace is utf-8"),
                );
                peers.set_peers(initial_member_keys.iter().chain(initial_resident_keys.iter()));
                let me: [u8; 32] = signer
                    .public_key()
                    .as_ref()
                    .try_into()
                    .expect("ed25519 keys are 32 bytes");
                voice::spawn_hub(
                    voice_requests,
                    statesync_plane::socket_factory(wireguard_effect, &overlay_slot),
                    std::sync::Arc::clone(&peers),
                    me,
                );
                Some(peers)
            } else {
                drop(voice_requests);
                None
            }
        };

        // the reachability lane + the staged WireGuard plane. the channel is
        // registered unconditionally (an unregistered channel is a protocol
        // violation that kills the sender's connection); the plane itself
        // runs only when `wireguard_listen` is configured, on its OWN
        // plain-tokio OS thread (the app-surface split exactly), talking to
        // the mesh through the two pump tasks below.
        let (reach_p2p_tx, mut reach_p2p_rx) =
            network.register(CHANNEL_REACHABILITY, quota, MAX_BACKLOG);
        let reach_cmd: Option<tokio::sync::mpsc::Sender<reachability::ReachabilityCommand>> =
            match wireguard_listen {
                Some(wg_addr) => {
                    // rendezvous coordinators = every coordinated-reach hint's
                    // coordinator ingress, PLUS the ambient override/default
                    // (deduped) — without it an invite-joined member (whose
                    // descriptor carries no `coordinated:` hints, stripped at
                    // mint time) binds zero coordinators and never registers.
                    let mut coordinators: Vec<Ingress> =
                        coordinated.iter().map(|(_, c, _)| c.clone()).collect();
                    match config::coordinator_ingress(primary_coordinator.as_deref()) {
                        Ok(Some(ambient)) => {
                            if !coordinators.contains(&ambient) {
                                coordinators.push(ambient);
                            }
                        }
                        Ok(None) => {}
                        Err(e) => eprintln!(
                            "[node {label}] reachability: ambient coordinator unusable ({e}) — \
                             registering with descriptor-hinted coordinators only"
                        ),
                    }
                    Some(wire_reachability_plane(
                        &context,
                        &label,
                        &chain_id,
                        &signer,
                        &wireguard_key_file,
                        &mesh_state_file,
                        wg_addr,
                        wireguard_effect,
                        overlay_slot.clone(),
                        advertised_reach,
                        wireguard_advertised.clone(),
                        coordinators,
                        // members serve the invite intro: a fresh joiner's
                        // tunnel comes up against this listener before any p2p.
                        invite_listen,
                        coord_cap.clone(),
                        reach_p2p_tx,
                        reach_p2p_rx,
                    ))
                }
                None => {
                    context
                        .child("blackhole_reachability")
                        .spawn(move |_ctx| async move { while reach_p2p_rx.recv().await.is_ok() {} });
                    drop(reach_p2p_tx);
                    None
                }
            };
        // boot: target the resume epoch's member set immediately (with the
        // committed resident set as the pre-warm standbys); cutovers
        // retarget from the orchestrator loop below. the recovered view base
        // keeps advert expiries in the same view regime as live peers.
        if let Some(cmd) = &reach_cmd {
            let _ = cmd
                .send(reachability::ReachabilityCommand::Retarget(
                    reachability::MeshEpochEvent {
                        epoch: initial_resume_epoch,
                        members: initial_member_keys.clone(),
                        standbys: initial_resident_keys.clone(),
                        current_view: resumed.as_ref().map(|r| r.view_base).unwrap_or(0),
                    },
                ))
                .await;
        }

        // start the network actors (dialer/listener/router/tracker). registered
        // receivers buffer regardless, so starting before the engine is fine.
        network.start();

        let promoted_validator_boot = promoted && !validators.contains(&signer.public_key());
        if promoted_validator_boot {
            let Some(server_peer) = sync_source else {
                eprintln!(
                    "[node {label}] FATAL: promoted validator has no statesync source for \
                     post-reboot catch-up"
                );
                std::process::exit(1);
            };
            // like the parked joiner's client: prefer the plane (lazy bind —
            // the promotion reboot restores its tunnels from disk) and fall
            // back to the mesh path on transport failure.
            let mesh_client = BootP2pSyncClient::new(sync_tx, sync_rx, server_peer.clone());
            let client = {
                let plane_slot: statesync_plane::PlaneSlot =
                    std::sync::Arc::new(std::sync::OnceLock::new());
                if statesync_plane::enabled() && wireguard_listen.is_some() {
                    let book = statesync_plane::OverlayBook::new(
                        String::from_utf8(namespace.clone()).expect("namespace is utf-8"),
                    );
                    book.set_peers(peers.iter());
                    statesync_plane::spawn_bring_up(
                        label.clone(),
                        book,
                        signer.public_key(),
                        std::sync::Arc::clone(&plane_slot),
                        statesync_plane::socket_factory(wireguard_effect, &overlay_slot),
                        None,
                    );
                }
                statesync_plane::PlaneFallbackClient::new(
                    plane_slot,
                    &server_peer,
                    mesh_client,
                    label.clone(),
                )
            };
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
                        if !target
                            .participants
                            .iter()
                            .any(|key| key.as_slice() == signer.public_key().as_ref())
                        {
                            eprintln!(
                                "[node {label}] FATAL: catch-up target height {} no longer \
                                 includes this validator",
                                target.height
                            );
                            std::process::exit(1);
                        }
                        let floor = match verify_manifest_floor(&namespace, target) {
                            Ok(floor) => floor,
                            Err(e) => {
                                eprintln!(
                                    "[node {label}] FATAL: catch-up target floor verify: {e}"
                                );
                                std::process::exit(1);
                            }
                        };
                        if target.epoch > resumed.as_ref().map(|rec| rec.epoch).unwrap_or(0)
                            && let Err(e) = node::BlockSink::cutover(
                                &mut recovery,
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
                            &mut next_seq,
                            &summary.frame_bytes,
                            &me_bytes,
                        );
                        let ckpt = match write_post_reboot_catchup_checkpoint(
                            &mut recovery,
                            &host,
                            recovery_manifest_for_resume.as_ref(),
                            target,
                            &summary.blocks,
                            next_seq,
                        )
                        .await
                        {
                            Ok(ckpt) => ckpt,
                            Err(e) => {
                                eprintln!("[node {label}] FATAL: {e}");
                                std::process::exit(1);
                            }
                        };
                        if let Some(cert) = floor {
                            let floor = recovery::FloorCert {
                                epoch: target.epoch,
                                height: target.height,
                                cert,
                            };
                            if let Err(e) = recovery.write_floor_cert(&floor).await {
                                eprintln!("[node {label}] FATAL: catch-up floor-cert write: {e}");
                                std::process::exit(1);
                            }
                        }
                        prev_ckpt = (ckpt.height, ckpt.oplog_pos);
                        let refreshed = match recovery
                            .recover_with_sink(&mut host, &ckpt, Some(&mut boot_fold))
                            .await
                        {
                            Ok(rec) => rec,
                            Err(e) => {
                                eprintln!(
                                    "[node {label}] FATAL: post-catch-up checkpoint recovery: {e}"
                                );
                                std::process::exit(1);
                            }
                        };
                        advance_next_seq_from_frames(&mut next_seq, &refreshed.frames, &me_bytes);
                        resumed = Some(refreshed);
                        recovery_manifest_for_resume = Some(ckpt);
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
                        if !target
                            .participants
                            .iter()
                            .any(|key| key.as_slice() == signer.public_key().as_ref())
                        {
                            eprintln!(
                                "[node {label}] FATAL: full-sync target height {} no longer \
                                 includes this validator",
                                target.height
                            );
                            std::process::exit(1);
                        }
                        let floor = match verify_manifest_floor(&namespace, &target) {
                            Ok(floor) => floor,
                            Err(e) => {
                                eprintln!(
                                    "[node {label}] FATAL: full-sync target floor verify: {e}"
                                );
                                std::process::exit(1);
                            }
                        };
                        let synced = match sync_all_modules(
                            &context,
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
                        host = synced;
                        if target.epoch > resumed.as_ref().map(|rec| rec.epoch).unwrap_or(0)
                            && let Err(e) = node::BlockSink::cutover(
                                &mut recovery,
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
                            &host,
                            Some(target.height),
                            target.epoch,
                            target.view_base,
                            target.participants.clone(),
                            target.residents.clone(),
                            None,
                            target.current_version,
                            target.pending_upgrade.clone(),
                            pos,
                            next_seq,
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
                            eprintln!(
                                "[node {label}] FATAL: full-sync checkpoint write: {e}"
                            );
                            std::process::exit(1);
                        }
                        if let Some(cert) = floor {
                            let floor = recovery::FloorCert {
                                epoch: target.epoch,
                                height: target.height,
                                cert,
                            };
                            if let Err(e) = recovery.write_floor_cert(&floor).await {
                                eprintln!(
                                    "[node {label}] FATAL: full-sync floor-cert write: {e}"
                                );
                                std::process::exit(1);
                            }
                        }
                        prev_ckpt = (ckpt.height, ckpt.oplog_pos);
                        let refreshed = match recovery
                            .recover_with_sink(&mut host, &ckpt, Some(&mut boot_fold))
                            .await
                        {
                            Ok(rec) => rec,
                            Err(e) => {
                                eprintln!(
                                    "[node {label}] FATAL: full-sync recovery refresh: {e}"
                                );
                                std::process::exit(1);
                            }
                        };
                        let me_bytes = signer.public_key().as_ref().to_vec();
                        advance_next_seq_from_frames(&mut next_seq, &refreshed.frames, &me_bytes);
                        resumed = Some(refreshed);
                        recovery_manifest_for_resume = Some(ckpt);
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
            match client.into_inner().into_parts() {
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

        // the FINAL index heal, at the boot tip every path converged on:
        // whatever the replay/catch-up fold could not reproduce (opaque
        // blocks, a state-sync jump, a stopped fold) re-derives here from
        // state that has verified against the boundary app-hash.
        drop(boot_fold);
        if let Some(boot_height) = resumed.as_ref().and_then(|r| r.height) {
            heal_index(&index, &host, boot_height, &label).await;
        }

        let member_keys = match resume_member_keys(resumed.as_ref(), &validators) {
            Ok(keys) => keys,
            Err(e) => {
                eprintln!("[node {label}] FATAL: {e}");
                std::process::exit(1);
            }
        };
        if !member_keys.contains(&signer.public_key()) {
            println!(
                "[node {label}] this identity is not in the recovered validator set — \
                 halting (restart with --sync-only to observe)"
            );
            std::process::exit(0);
        }
        let participants: Set<ed25519::PublicKey> =
            Set::try_from(member_keys.clone()).expect("valset membership has no duplicates");
        let resume_epoch = resumed.as_ref().map(|r| r.epoch).unwrap_or(0);
        mesh_oracle.track(
            resume_epoch,
            mesh_at(&member_keys.iter().cloned().collect()),
        );
        if resume_epoch < bank_base || resume_epoch >= bank_base + EPOCH_CHANNEL_BANK {
            eprintln!(
                "[node {label}] FATAL: recovered epoch {resume_epoch} outside the \
                 pre-registered channel bank [{bank_base}, {})",
                bank_base + EPOCH_CHANNEL_BANK
            );
            std::process::exit(1);
        }
        for epoch in bank_base..resume_epoch {
            let Some(slot) = channel_bank
                .get_mut((epoch - bank_base) as usize)
                .and_then(|slot| slot.take())
            else {
                continue;
            };
            let ((_, vote_rx), (_, cert_rx), (_, res_rx), (_, payload_rx), (_, fetch_rx)) = slot;
            for (suffix, mut rx) in [
                ("vote", vote_rx),
                ("cert", cert_rx),
                ("resolver", res_rx),
                ("payload", payload_rx),
                ("fetch", fetch_rx),
            ] {
                let label: &'static str =
                    Box::leak(format!("blackhole_e{epoch}_{suffix}").into_boxed_str());
                context.child(label).spawn(move |_ctx| async move {
                    while rx.recv().await.is_ok() {}
                });
            }
        }
        let mut pending_boot = recovery_manifest_for_resume
            .as_ref()
            .zip(resumed.as_ref())
            .and_then(|(manifest, rec)| derive_pending_boot(manifest, rec));
        // If no membership cutover already claimed the resume slot, re-arm a
        // pending upgrade at the same deterministic activation boundary an
        // uninterrupted node would use. This runs after post-reboot catch-up, so
        // it reads the freshest recovered host/record.
        if pending_boot.is_none()
            && let Some(rec) = resumed.as_ref()
        {
            pending_boot = read_upgrade_state(&host).await.pending.and_then(|p| {
                let crossed = rec.height.is_some_and(|h| h >= p.activation_height);
                if crossed {
                    None
                } else {
                    p.activation_height.checked_sub(rec.view_base)
                }
            });
        }

        // the statesync INGRESS task: owns the channel receiver and loops a
        // clean `recv().await`, forwarding frames into a local bounded queue.
        // the pump then selects on THAT queue — dropping an mpsc `next()`
        // future between ticks is lossless, whereas dropping the p2p receiver's
        // actor-backed `recv()` future mid-flight could eat a delivered
        // message. bounded + drop-on-full: clients time out and retry, so a
        // flood degrades to retries instead of unbounded memory. the queue
        // carries BOTH statesync carriers — mesh rpc frames and data-plane
        // request streams — so one serve task answers both.
        let (bridge_tx, sync_ingress) =
            futures::channel::mpsc::channel::<statesync_plane::SyncJob>(64);
        // the blob fetch-on-miss lane (the #298 prompt-blob cross-node gap):
        // the oracle pool's resolver asks current peers for a digest its own
        // store lacks, over this same statesync channel. the pending map is
        // the serve loop's demux — frames answering OUR fetches never enter
        // the request path — and the peer set follows every cutover re-track
        // beside the other planes' books.
        let blob_pending: blob_fetch::PendingMap = Default::default();
        let blob_peers: std::sync::Arc<std::sync::RwLock<Vec<ed25519::PublicKey>>> =
            std::sync::Arc::new(std::sync::RwLock::new(
                initial_member_keys
                    .iter()
                    .chain(initial_resident_keys.iter())
                    .cloned()
                    .collect(),
            ));
        let blob_fetcher = blob_fetch::MeshBlobFetcher::new(
            sync_tx.clone(),
            blob_pending.clone(),
            std::sync::Arc::clone(&blob_peers),
            signer.public_key(),
        )
        .into_fetch_fn();
        {
            let mut bridge_tx = bridge_tx.clone();
            context.child("sync_ingress").spawn(move |_ctx| {
                let mut receiver = sync_rx;
                async move {
                    loop {
                        match receiver.recv().await {
                            Ok((peer, msg)) => {
                                let bytes: Vec<u8> = msg.into();
                                // full bridge = flood pressure: drop; clients retry.
                                let _ = bridge_tx
                                    .try_send(statesync_plane::SyncJob::Mesh(peer, bytes));
                            }
                            Err(_) => return, // network shutdown — nothing to serve.
                        }
                    }
                }
            });
        }
        // statesync's per-use data plane (env-gated, default off): the same
        // requests over overlay stream sockets, accepted into the same queue.
        // the address book doubles as admission — members + standbys of the
        // tracked view, updated at every cutover re-track below.
        let sync_plane_book = statesync_plane::enabled().then(|| {
            let book = statesync_plane::OverlayBook::new(
                String::from_utf8(namespace.clone()).expect("namespace is utf-8"),
            );
            book.set_peers(initial_member_keys.iter().chain(initial_resident_keys.iter()));
            statesync_plane::spawn_bring_up(
                label.clone(),
                std::sync::Arc::clone(&book),
                signer.public_key(),
                std::sync::Arc::new(std::sync::OnceLock::new()),
                statesync_plane::socket_factory(wireguard_effect, &overlay_slot),
                Some(bridge_tx.clone()),
            );
            book
        });
        drop(bridge_tx);
        // the statesync SERVE task (the [`SyncStateRequest`] seam): owns the
        // capture cache and both statesync carriers end-to-end — decode,
        // leases, chunk slicing, and the mesh/plane replies — so serving a
        // joiner never occupies the consensus loop. the loop answers only
        // the bounded state touches crossing `sync_state_tx`; when the loop
        // is busy the serve lane backpressures, never the reverse.
        let (sync_state_tx, mut sync_state_rx) =
            futures::channel::mpsc::channel::<SyncStateRequest>(8);
        {
            let state_tx = sync_state_tx;
            let mut sync_tx = sync_tx;
            let mut ingress = sync_ingress;
            let blob_pending = blob_pending.clone();
            let sync_blobs = blobs.clone();
            context
                .child("statesync_serve")
                .spawn(move |_ctx| async move {
                    let mut server = SyncServer::new();
                    while let Some(job) = ingress.next().await {
                        // both carriers land here: mesh frames ride an rpc
                        // envelope (multiplexed channel — the id correlates);
                        // a plane stream IS its own correlation and reply path.
                        let (reply_to, rpc_id, request) = match job {
                            statesync_plane::SyncJob::Mesh(peer, bytes) => {
                                let Ok((rpc_id, body)) = statesync::decode_rpc(&bytes) else {
                                    continue; // malformed rpc envelope: drop, never crash.
                                };
                                // the mesh demux: OUR fetch answers are consumed,
                                // stray responses (a blob answer landing after its
                                // fan-out's sweep) and unparseable frames are
                                // DROPPED — answering either is how two serve
                                // loops bounce Error frames forever. only a real
                                // request proceeds; the reply-on-bad-frame lane is
                                // stream-only below.
                                match blob_fetch::classify_mesh_frame(
                                    &blob_pending,
                                    rpc_id,
                                    body,
                                ) {
                                    blob_fetch::MeshFrame::OurResponse
                                    | blob_fetch::MeshFrame::StrayResponse
                                    | blob_fetch::MeshFrame::Junk => continue,
                                    blob_fetch::MeshFrame::Request(req) => (
                                        statesync_plane::SyncReplyTo::Mesh(peer),
                                        rpc_id,
                                        Ok(req),
                                    ),
                                }
                            }
                            statesync_plane::SyncJob::Plane(stream, req) => (
                                statesync_plane::SyncReplyTo::Plane(stream),
                                0,
                                statesync::decode_request(&req),
                            ),
                        };
                        let resp = match request {
                            // blob fetches are host state — answered from the
                            // node-local store, never routed into SyncServer.
                            Ok(statesync::SyncRequest::Blob { digest }) => {
                                blob_fetch::serve_blob(&sync_blobs, &digest)
                            }
                            Ok(req) => drive_sync_request(&mut server, &state_tx, req).await,
                            // stream-only by construction: a plane stream is a
                            // one-shot request/response, so an Error reply here
                            // can never re-enter a serve loop and oscillate.
                            Err(e) => statesync::SyncResponse::Error(format!(
                                "bad request frame: {e}"
                            )),
                        };
                        let resp = statesync::encode_response(&resp);
                        match reply_to {
                            statesync_plane::SyncReplyTo::Mesh(peer) => {
                                let _ = sync_tx.send(
                                    Recipients::One(peer),
                                    IoBuf::from(statesync::encode_rpc(rpc_id, &resp)),
                                    false,
                                );
                            }
                            statesync_plane::SyncReplyTo::Plane(mut stream) => {
                                // one request per stream: write the response
                                // and drop — the close is the client's
                                // completion.
                                let _ =
                                    statesync::dataplane::write_frame(&mut stream, &resp).await;
                            }
                        }
                    }
                });
        }
        // the lobby lane rides the same bridge pattern: announces are consumed
        // by the pump between drains. drop-on-full is doubly safe here — a
        // parked joiner re-announces every few seconds anyway.
        let (lobby_bridge_tx, mut lobby_ingress) =
            futures::channel::mpsc::channel::<(ed25519::PublicKey, Vec<u8>)>(64);
        context.child("lobby_ingress").spawn(move |_ctx| {
            let mut receiver = lobby_rx;
            let mut bridge_tx = lobby_bridge_tx;
            async move {
                loop {
                    match receiver.recv().await {
                        Ok((peer, msg)) => {
                            let bytes: Vec<u8> = msg.into();
                            let _ = bridge_tx.try_send((peer, bytes));
                        }
                        Err(_) => return,
                    }
                }
            }
        });
        // the submit-relay lane rides the same bounded drop-on-full bridge: a
        // dropped relay degrades to the resident client's honest timeout +
        // re-submit, so flood pressure never blocks the pump.
        let (relay_bridge_tx, mut relay_ingress) =
            futures::channel::mpsc::channel::<(ed25519::PublicKey, Vec<u8>)>(64);
        context.child("relay_ingress").spawn(move |_ctx| {
            let mut receiver = relay_rx;
            let mut bridge_tx = relay_bridge_tx;
            async move {
                loop {
                    match receiver.recv().await {
                        Ok((peer, msg)) => {
                            let bytes: Vec<u8> = msg.into();
                            let _ = bridge_tx.try_send((peer, bytes));
                        }
                        Err(_) => return,
                    }
                }
            }
        });

        // spawn one epoch's engine from the channel bank. scheme built the
        // production way (`signer` finds our key's index in the sorted
        // participant set); per-epoch genesis floor + per-epoch storage
        // partition, so a respawned engine can never collide with a
        // predecessor. the consensus signature scheme is a GENESIS-WIDE
        // constant (ConsensusScheme); adding V2Bls makes the match
        // non-exhaustive — the compiler-enforced rekey point.
        let spawn_epoch = |bank: &mut Vec<Option<_>>,
                               epoch: u64,
                               participants: Set<ed25519::PublicKey>,
                               store: ContentStore,
                               floor_bytes: Option<Vec<u8>>|
         -> SimplexOrderer {
            let slot = bank
                .get_mut(epoch.checked_sub(bank_base).expect("epochs never rebase down") as usize)
                .and_then(|s| s.take())
                .unwrap_or_else(|| {
                    eprintln!(
                        "[node {label}] FATAL: epoch {epoch} exhausts the pre-registered                          channel bank ({EPOCH_CHANNEL_BANK}) — rebuild with a wider bank"
                    );
                    std::process::exit(1);
                });
            let (vote, certificate, resolver, payload, fetch) = slot;
            let scheme = match CONSENSUS_SCHEME {
                ConsensusScheme::V1Ed25519 => simplex_ed25519::Scheme::signer(
                    &namespace,
                    participants,
                    signer.clone(),
                )
                .expect("our key is in the validator participant set"),
                // the engine and tests are V2-capable (see consensus::BlsScheme);
                // wiring V2 into the epoch respawn machinery needs the bls
                // participant BiMap derived per epoch (valset-registered bls
                // keys + proof-of-possession) — fail-stop until that lands.
                ConsensusScheme::V2Bls => {
                    unimplemented!(
                        "V2Bls node wiring lands with valset bls key registration; \
                         the consensus engine itself is V2-capable"
                    )
                }
            };
            // a SAME-EPOCH respawn passes the persisted finalization floor so
            // the reopened journal's replay does not re-report history the
            // recovered state already contains. a damaged floor FAILS — a
            // silent genesis-floor fallback would resurrect the wedge.
            let floor = floor_bytes.map(|bytes| {
                match consensus::decode_finalization(&scheme, &bytes) {
                    Ok(f) => f,
                    Err(e) => {
                        eprintln!("[node {label}] FATAL: {e}");
                        std::process::exit(1);
                    }
                }
            });
            let label: &'static str =
                Box::leak(format!("consensus_e{epoch}").into_boxed_str());
            // spawn WITH the lazy payload-fetch backstop: quorum is a SUBSET
            // (n - floor((n-1)/3)), so a validator can finalize a view it never
            // voted in — and if it also missed the one-shot relay gossip (mesh
            // still forming, transient disconnect), relay-only wiring would
            // silently drop that op's slot and wedge/fork the node. the
            // resolver fetches missing bytes by digest from the tracked mesh
            // (the oracle is provider AND blocker) and fills the ordered slot.
            SimplexOrderer::spawn_with_resolver(
                context.child(label),
                scheme,
                oracle.clone(),
                oracle.clone(),
                signer.public_key(),
                format!("{}-e{epoch}", signer.public_key()),
                Epoch::new(epoch),
                epoch_floor(&namespace, epoch),
                floor,
                // per-process, PER-EPOCH content store: pins/pending of a torn
                // down epoch die with it (in-flight ops are resubmitted). a
                // RESTART's store arrives pre-seeded from the recovery journal.
                store,
                vote,
                certificate,
                resolver,
                payload,
                fetch,
                false,
            )
        };

        // the boot store: seeded with every retained journaled frame so
        // finalizations the reopened engine re-reports (at most the floor
        // cert itself, plus anything finalized-but-undrained at the crash)
        // resolve locally instead of wedging the ordered gate.
        let boot_store = ContentStore::new();
        if let Some(rec) = &resumed {
            for frame in &rec.frames {
                boot_store.pin(frame.clone());
            }
        }
        // the persisted floor is only valid for the epoch it was recorded in
        // (Floor::assert pins the certificate to the engine's epoch).
        let boot_floor = match recovery.floor_cert() {
            Ok(cert) => cert.filter(|c| c.epoch == resume_epoch),
            Err(e) => {
                eprintln!("[node {label}] FATAL: persisted finalization floor is damaged: {e}");
                std::process::exit(1);
            }
        };
        let mut last_cert_height = boot_floor.as_ref().map(|c| c.height);
        // the newest persisted finalization floor, kept in memory so the
        // statesync service can serve it to joiners at a matching boundary.
        let mut latest_floor: Option<recovery::FloorCert> = boot_floor.clone();
        let recovered_height = resumed
            .as_ref()
            .and_then(|rec| rec.height)
            .map(|height| height.to_string())
            .unwrap_or_else(|| "none".to_string());
        let recovered_hash = resumed
            .as_ref()
            .map(|rec| hex(&rec.app_hash))
            .unwrap_or_else(|| "none".to_string());
        let replayed = resumed.as_ref().map(|rec| rec.applied).unwrap_or(0);
        let boot_floor_height = latest_floor
            .as_ref()
            .map(|floor| floor.height.to_string())
            .unwrap_or_else(|| "none".to_string());
        diag_log(format!(
            "DIAG promotion_recovered recovered_height={} recovered_hash={} replayed={} \
             boot_floor_height={}",
            recovered_height, recovered_hash, replayed, boot_floor_height
        ));
        let orderer = spawn_epoch(
            &mut channel_bank,
            resume_epoch,
            participants.clone(),
            boot_store,
            boot_floor.map(|c| c.cert),
        );
        let view_base = resumed.as_ref().map(|r| r.view_base).unwrap_or(0);
        let mut node = match &resumed {
            Some(rec) => OrderedNode::resume(
                host,
                orderer,
                recovery,
                rec.height
                    .map(|height| host::FinalizedBlock { height, app_hash: rec.app_hash }),
                rec.view_base,
            ),
            None => OrderedNode::with_sink(host, orderer, recovery),
        };
        // the observation barrier: every drain batch ends AT a block that
        // moves the valset root, so the orchestration step below observes a
        // membership change at exactly its block's view — the same view on
        // every validator, whatever the local batch shape. without it the
        // armed cutover view (and with it the next epoch's height base)
        // would depend on drain timing: a cross-node fork.
        node.watch_module("valset");

        // the valset ORCHESTRATOR: watches finalized valset module state and
        // schedules deterministic epoch cutovers. it resumes at the recovered
        // epoch coordinates over the epoch's ENGINE PARTICIPANT SET, and
        // re-arms a cutover the pre-crash process had scheduled.
        let resident_keys = match resume_resident_keys(resumed.as_ref()) {
            Ok(keys) => keys,
            Err(e) => {
                eprintln!("[node {label}] FATAL: {e}");
                std::process::exit(1);
            }
        };
        let mut orchestrator = consensus::ValsetOrchestrator::resume(
            CUTOVER_DELAY,
            member_keys.clone(),
            resident_keys.clone(),
            resume_epoch,
            view_base,
            pending_boot,
        );
        if let Some(ceiling) = pending_boot {
            node.set_view_ceiling(ceiling);
            println!(
                "[node {label}] re-armed pending cutover at view {ceiling} (epoch {})",
                resume_epoch + 1
            );
        }

        // the genesis app-hash BEFORE any op — the demo asserts this agrees across
        // processes (a fork here would be a genesis-determinism bug, not consensus).
        // a RESTORED boot prints its recovered line above instead.
        if resumed.is_none() {
            let genesis_hash = node.app_hash();
            println!("[node {label}] genesis app_hash={}", hex(&genesis_hash));
        }

        // introduce a DISTINCT op per process: node N writes directory key "kN" =
        // "node-N". distinct key + distinct origin -> distinct frame -> distinct
        // sha256 digest, so a peer that finalizes THIS op's digest has NO local
        // bytes for it — unless the leader's relay gossiped them on CHANNEL_PAYLOAD
        // and this process's store-only drain cached them. directory is order-
        // INDEPENDENT, so both nodes converge on {k0=node-0, k1=node-1} under any
        // interleaving, isolating the property under test (did the peer's payload
        // cross the wire?) from op ordering. ONE submit — the automaton PEEKS
        // (never pops), so the digest rides out every nullified early view until
        // the mesh forms and this node leads and proposes it.
        // dev shape only — a REAL network's genesis carries no demo scaffolding
        // (and a restored boot must not re-frame it: seq 0 was already spent).
        if dev_demo && resumed.is_none() {
            let n = label.trim_start_matches('#').to_string();
            let op = Msg {
                target: "directory".into(),
                payload: encode_msg(&DirMsg::Set {
                    key: format!("k{n}"),
                    value: format!("node-{n}"),
                }),
            };
            node.submit(&signer, 0, op).await.expect("submit op");
        }

        // the local rpc bridge: blocking listener threads push parsed requests
        // into this bounded queue; the pump answers between drains.
        let (rpc_tx, mut rpc_ingress) = futures::channel::mpsc::channel::<RpcJob>(64);
        if let Some(listener) = rpc_listener {
            println!(
                "[node {label}] rpc listening on {}",
                listener.local_addr().map(|a| a.to_string()).unwrap_or_default()
            );
            spawn_rpc_listener(listener, rpc_tx);
        } else {
            drop(rpc_tx); // rpc off: the branch below just stays pending forever.
        }

        // the ordered lane SIGNS every frame. rpc submits are signed by THIS
        // node's identity (the node is the local caller's custodian until user
        // keys reach the console); `next_seq` was set at boot — 1 on a fresh
        // genesis (after the demo op's 0), or past every recovered frame.

        // pump: drain finalized frames on an interval, apply them in agreed
        // (ascending-view) order, serve statesync rpcs, answer local rpc, and
        // drive the reactor seam between drains (every response reflects a
        // block boundary — never a torn mid-drain view). print `converged` ONCE
        // this node has applied every VALIDATOR's op. this infinite loop IS the
        // "run forever" park (keeps the mesh + sync service alive for joiners);
        // rpc `shutdown` is the graceful exit.
        let expected = validators.len();
        let mut applied = 0usize;
        let mut converged = false;
        // the app-surface lane: held submit replies keyed by the submitted
        // frame's content address, resolved when the frame drains (or expired
        // after SUBMIT_HOLD), plus the last block height published to ws
        // subscribers.
        let mut http_ingress = http_cmds;
        let mut pending_submits: std::collections::HashMap<
            node::FrameId,
            (
                futures::channel::oneshot::Sender<Result<noded::BlockSummary, String>>,
                std::time::Instant,
            ),
        > = std::collections::HashMap::new();
        // relayed submits held for a wire answer, keyed like pending_submits by
        // the frame's content address: resolved by the SAME drain that resolves
        // local holds, expired on the same SUBMIT_HOLD budget. the peer is where
        // the Reply goes.
        let mut pending_relays: std::collections::HashMap<
            node::FrameId,
            (ed25519::PublicKey, std::time::Instant),
        > = std::collections::HashMap::new();
        let mut validator_relay = relay_runtime::ValidatorRelay::new(blobs.clone());
        let mut last_published: Option<u64> = None;
        // verified-but-unapproved join requests, keyed by joiner key. NODE-
        // LOCAL and in-memory by design: this is a doorbell, not state — the
        // parked joiner re-announces every few seconds, so a restart loses
        // nothing durable. read by the `join-requests` rpc; entries whose key
        // has since become a member are dropped at read time.
        let mut join_requests: std::collections::BTreeMap<Vec<u8>, JoinRequestRecord> =
            std::collections::BTreeMap::new();
        // recovery cadence: sealed blocks since the last checkpoint manifest.
        let mut blocks_since_checkpoint: u64 = 0;
        // the last absolute view ticked to the reachability plane — one
        // ViewTick per actual advance, not one per 100ms drain pass.
        let mut last_reach_view: Option<u64> = None;
        // the per-block-time flush cadence: packs the window's enqueued frames
        // (real ops and/or an idle nop) into one batch block. see the flush loop.
        let mut last_flush = std::time::Instant::now();
        // a cutover Retarget the plane's command queue could not take yet
        // (NON-BLOCKING sends: the plane is not consensus, so the loop never
        // waits on it). retried every drain beat until it lands; a newer
        // epoch's Retarget supersedes an undelivered older one.
        let mut pending_retarget: Option<reachability::MeshEpochEvent> = None;
        // dev override (`make dev` sets DUCKTAPE_DISABLE_HEARTBEAT): keep an idle
        // dev chain quiet — no nop blocks — so every committed block is real
        // activity and the journal/logs carry no idle churn. NEVER set this on a
        // multi-node or upgrade-driving network: the heartbeat is what ticks an
        // idle chain across a pending cutover and keeps the console height
        // visibly live.
        let heartbeat_disabled = std::env::var_os("DUCKTAPE_DISABLE_HEARTBEAT").is_some();
        // throttle for the saga crank pump below.
        let mut last_crank = std::time::Instant::now();
        // throttle for the dispatch delivery-nudge pump below.
        let mut last_nudge = std::time::Instant::now();
        // the host-owned worker set (reactor seam): effects of finalized
        // blocks are offered here, and claimed follow-ups re-enter the ordered
        // lane as their own blocks.
        // load capability specs and discover this host's installed executor
        // CLIs (BYO — no credential handling here). the discovered tag set is
        // BOTH what the oracle worker can run and what this node announces to
        // the capability registry, so an announce can never claim more than
        // the host provides (`announce_capabilities = false` narrows the
        // announced set to nothing — never the reverse). routing and
        // default models live in the specs (docs/records/specs/capability-spec.md); a broken
        // operator spec is a boot error, not a silently dropped executor.
        let providers = capability_host::discover_with_dirs_and_output_sink(
            agent_dirs.clone(),
            run_output_sink(stream_hub.run_output()),
        )
        .unwrap_or_else(|e| panic!("capability specs failed to load: {e}"));
        let my_capabilities = providers.capabilities();
        // OFF-LOOP execution: the pool gates effects inline (lease check —
        // WorkerRequests leased to another node's key are skipped, not
        // double-run — under this node's submit key) but runs the provider
        // CLI on spawned background tasks; completed results come back over
        // `oracle_results` (an ingress arm below) and re-enter the ordered
        // lane as ordinary signed submits, so a minutes-long run never
        // stalls the drain/rpc/heartbeat arms of this loop.
        let (oracle_worker, mut oracle_results) = oracle_pool::build(
            &context,
            providers,
            signer.public_key().as_ref().to_vec(),
            blobs.clone(),
            agent_provisioner.clone(),
            // fetch-on-miss over the mesh: a prompt pin staged on another
            // node's blob store resolves here instead of failing the run.
            Some(blob_fetcher),
        );
        let workers: Vec<Box<dyn reactor::Worker>> = vec![oracle_worker];
        // the readiness self-signaller: polls COMMITTED upgrade state between drains
        // and emits ONE truthful validator-origin `SignalReady` per pending upgrade
        // this binary can execute. survives restart/late-join (state-driven, not a
        // one-shot effect). inert before the module is registered.
        let mut signaller =
            ReadinessSignaller::new(MAX_PROTOCOL_VERSION, signer.public_key().as_ref().to_vec());
        // the capability self-announcer: publishes this node's discovered
        // provider set into the capability registry once (state-driven,
        // idempotent). inert when this host installed no executor CLIs.
        let mut announcer =
            CapabilityAnnouncer::new(signer.public_key().as_ref().to_vec(), my_capabilities);
        // one-shot upgrade transition markers keyed off COMMITTED upgrade state,
        // modeled on the `converged` latch: `upgrade armed …` fires when readiness
        // first reaches R==n (every current boundary member signaled) for the
        // pending upgrade — the pre-boundary observable the e2e keys on; `upgrade
        // cleared …` fires when a previously-observed pending clears (the boundary
        // `Advance` reconciliation at H, on ARM or ABORT). the boundary crossing
        // itself prints the `upgrade activated …` / `upgrade aborted …` verdict.
        let mut upgrade_armed_latch: Option<(String, u32)> = None;
        let mut upgrade_pending_seen: Option<String> = None;

        // graceful checkpoint on process signals (SIGTERM/SIGINT): the desktop
        // shell SIGTERMs the daemon on quit, so it must take the SAME safe path
        // as an rpc `Shutdown` — a best-effort final manifest + journal barrier
        // — instead of tearing down mid-block and leaving the disk ahead of the
        // last in-memory checkpoint (the recovery brick). the streams are made
        // INSIDE the tokio async context so the signal driver is live; a
        // failure to install them is non-fatal: log and carry on WITHOUT the
        // graceful-quit arm rather than aborting daemon boot — a hard SIGKILL /
        // power loss already lands on the same WAL-forward recovery, so the
        // worst case of a missing handler is the pre-fix behavior, not a brick.
        let mut sigterm =
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                Ok(s) => Some(s),
                Err(e) => {
                    eprintln!(
                        "[node {label}] WARN: SIGTERM handler install failed ({e}); \
                         graceful-quit checkpoint disabled (a hard kill still recovers)"
                    );
                    None
                }
            };
        let mut sigint =
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt()) {
                Ok(s) => Some(s),
                Err(e) => {
                    eprintln!(
                        "[node {label}] WARN: SIGINT handler install failed ({e}); \
                         graceful-quit checkpoint disabled (a hard kill still recovers)"
                    );
                    None
                }
            };

        // the graceful checkpoint sequence, shared by the rpc `Shutdown` arm and
        // the signal arm so the two can never drift. a macro (not a fn) because
        // it borrows `node` mutably while reading `orchestrator`/`next_seq` and
        // `node`'s type is a large generic — it runs on the SAME single-threaded
        // select loop, so it can never race the periodic checkpoint below.
        // captures the committed upgrade version fields the same way the periodic
        // checkpoint does, so a graceful-quit manifest is byte-identical to one.
        macro_rules! graceful_checkpoint {
            () => {{
                if let Some(f) = node.finalized() {
                    let pos = node.sink_mut().oplog_pos().await;
                    let (cv, pu) = read_upgrade_version_fields(node.host()).await;
                    if let Ok(m) = Manifest::capture(
                        node.host(),
                        Some(f.height),
                        orchestrator.epoch(),
                        orchestrator.epoch_base(),
                        participant_bytes(&orchestrator),
                        resident_bytes(&orchestrator),
                        orchestrator.pending_cutover().map(|c| c.cutover_view()),
                        cv,
                        pu,
                        pos,
                        next_seq,
                    ) {
                        let _ = node.sink_mut().write_manifest(&m).await;
                    }
                }
                let _ = node.sink_mut().sync().await;
            }};
        }
        // the drain deadline (see the drain arm): ABSOLUTE, so the
        // per-iteration select rebuild cannot reset it under ingress load.
        let mut next_drain = context.current() + DRAIN_TICK;
        loop {
            // resolve on whichever signal stream installed; if neither did,
            // this arm simply never fires (pending forever) and the loop runs
            // exactly as before the fix.
            let signalled = async {
                match (sigterm.as_mut(), sigint.as_mut()) {
                    (Some(t), Some(i)) => {
                        let t = t.recv();
                        let i = i.recv();
                        futures::pin_mut!(t, i);
                        futures::future::select(t, i).await;
                    }
                    (Some(t), None) => {
                        t.recv().await;
                    }
                    (None, Some(i)) => {
                        i.recv().await;
                    }
                    (None, None) => futures::future::pending::<()>().await,
                }
            }
            .fuse();
            futures::pin_mut!(signalled);
            futures::select_biased! {
                _ = signalled => {
                    println!(
                        "[node {label}] SIGTERM/SIGINT — graceful checkpoint then exit"
                    );
                    graceful_checkpoint!();
                    std::process::exit(0);
                }
                // DRAIN CADENCE — an ABSOLUTE deadline, hoisted ABOVE the ingress
                // arms. this select is rebuilt every loop iteration, so an
                // arm-local `sleep(100ms)` restarts from zero whenever any other
                // arm completes first — a saturating rpc-submit stream (requests
                // landing well inside 100ms) then resets the timer forever and
                // the drain NEVER runs: heights and status freeze, held submit
                // replies starve, and the epoch cutover (`respawn_if_due` below
                // is drain-driven) stalls for exactly as long as the flood lasts
                // while the armed boundary's discard window swallows every
                // accepted op. an absolute deadline survives the select rebuild,
                // and sitting above the ingress arms makes `select_biased!` take
                // it the moment it is due — load can delay one drain by one
                // request's service time, never starve it.
                _ = context.sleep_until(next_drain).fuse() => {
                    next_drain = context.current() + DRAIN_TICK;
                    // FAIL-STOP: a drain error is a node-local block-boundary
                    // fault — this node's state is indeterminate relative to its
                    // peers, so applying even one more finalized op could
                    // silently fork it. exit loudly; an operator (or supervisor)
                    // restarts the node, which then re-joins via state sync.
                    let drained_count = match node.drain_delivered().await {
                        Ok(n) => n,
                        Err(e) => {
                            eprintln!("[node {label}] FATAL: {e} — halting");
                            std::process::exit(1);
                        }
                    };
                    applied += drained_count;
                    // durabilize the tip seal when the chain goes idle. a seal is a
                    // plain journal append made durable only by the NEXT block's
                    // pre-apply sync; on an idle chain the tip block's seal can sit
                    // un-synced for a whole block-time, and a crash there loses it,
                    // turning the tip into a TRAILING block. that is fine for most
                    // ops, but a trailing SELF-READING op — a files CAS commit whose
                    // re-execution reads the claimant's already-durable post-state —
                    // cannot be selective-replayed and would brick a SOLO node (no
                    // peer to re-sync from). syncing on the idle transition closes
                    // the window; a busy chain amortizes durability against the next
                    // pre-apply and needs no extra sync here.
                    if drained_count > 0
                        && node.pending_batch_len() == 0
                        && node.orderer().pending_len() == 0
                        && let Err(e) = node.sink_mut().sync().await
                    {
                        eprintln!("[node {label}] tip-seal sync failed: {e}");
                    }
                    // resolve held app-surface submits against what this
                    // drain finished with; every disposition is deterministic,
                    // so the reply faithfully reports the op's consensus fate.
                    let drained = node.take_drained();
                    // the once-per-block System-injection traces (upgrade
                    // Advance, mailbox DeliverPending follow-ups) ride beside
                    // the member frames; each height's entry indexes AFTER
                    // that height's member dispatches, matching the replay
                    // paths' row order exactly.
                    let mut system_dispatches: std::collections::BTreeMap<
                        u64,
                        Vec<host::DispatchRecord>,
                    > = node.take_system_dispatches().into_iter().collect();
                    // sealed = journaled: one seal per BLOCK (height), whatever a
                    // batch's member count. count DISTINCT sealed heights so the
                    // checkpoint cadence stays per-block; applied and rejected
                    // members both seal, discarded frames never sealed a height.
                    blocks_since_checkpoint += drained
                        .iter()
                        .filter(|d| d.disposition != node::Disposition::Discarded)
                        .map(|d| d.height)
                        .collect::<std::collections::BTreeSet<u64>>()
                        .len() as u64;
                    // fold every SEALED frame into the derived per-module
                    // index: an applied frame contributes its dispatch trace,
                    // a rejected one folds EMPTY (it still consumed its
                    // height, and every module's watermark must track the
                    // sealed tip or restart staleness checks would rebuild
                    // spuriously). discarded frames never sealed a height.
                    // a frame the explorer shows — a decoded op that isn't
                    // the heartbeat nop (the deliberately-empty block that
                    // only ticks an idle chain) — additionally carries its
                    // explorer row, so GET /v1/blocks survives restarts.
                    // canonical state committed above, so an index failure
                    // degrades read models only — the store poisons itself
                    // and stays loud until rebuilt.
                    // fold each BLOCK once: a batch delivers N DrainedFrames at
                    // ONE height (its members, contiguous in agreed order). the
                    // per-module index and the `ducktape_*` metrics series are
                    // per-BLOCK — folding per frame would over-count blocks as ops
                    // AND lose every member after the first to the index's
                    // idempotent same-height skip. group the run of same-height
                    // frames, concatenate their dispatch traces under one running
                    // seq (so `op_key(height, seq)` stays unique across members),
                    // and fold once. canonical state committed above, so an index
                    // failure degrades read models only — it stays loud.
                    let mut gi = 0;
                    while gi < drained.len() {
                        let height = drained[gi].height;
                        let mut block_dispatches: Vec<host::DispatchRecord> = Vec::new();
                        let mut block_latency = 0u64;
                        let mut any_applied = false;
                        // the block record carries a RootOp for EVERY non-nop
                        // member (agreed order); the block hash is the first
                        // member's frame id and the commit is the members' shared
                        // app-hash. a pure nop/idle block shows no ops.
                        let mut block_ops: Vec<noded::RootOp> = Vec::new();
                        let mut block_hash: Option<node::FrameId> = None;
                        let mut block_app_hash: Option<StateRoot> = None;
                        while gi < drained.len() && drained[gi].height == height {
                            let d = &drained[gi];
                            gi += 1;
                            // a DISCARD never sealed this height (it is carried, not
                            // applied) — it contributes nothing to the fold.
                            if d.disposition == node::Disposition::Discarded {
                                continue;
                            }
                            if let (node::Disposition::Applied, Some(op)) =
                                (&d.disposition, &d.op)
                            {
                                any_applied = true;
                                block_latency = block_latency.saturating_add(op.latency_us);
                                block_dispatches.extend(op.dispatches.iter().cloned());
                            }
                            if let Some(op) = &d.op
                                && op.target != NOP_TARGET
                            {
                                let disposition = match d.disposition {
                                    node::Disposition::Applied => noded::BlockDisposition::Applied,
                                    node::Disposition::Rejected => noded::BlockDisposition::Rejected,
                                    // unreachable: Discarded is filtered at the top
                                    // of the inner loop; kept for match exhaustiveness.
                                    node::Disposition::Discarded => continue,
                                };
                                if block_hash.is_none() {
                                    block_hash = Some(d.id);
                                    block_app_hash = Some(d.app_hash);
                                }
                                block_ops.push(explorer_root_op(
                                    &blobs,
                                    &op.origin,
                                    &op.target,
                                    &op.payload,
                                    &op.dispatches,
                                    disposition,
                                ));
                            }
                        }
                        // the block's System-injection dispatches index AFTER
                        // every member's (the replay paths' merge order) — an
                        // agent reply delivered via the mailbox injection is
                        // an op row here like anywhere else.
                        if let Some(sys) = system_dispatches.remove(&height) {
                            block_dispatches.extend(sys);
                        }
                        // one block per height: an APPLIED block records fully
                        // (count, this node's summed apply latency, per-module
                        // dispatch counters); an all-rejected block (the idle nop
                        // lands here) only follows the height gauge. ops_total
                        // counts the aggregated member ops.
                        if any_applied {
                            metrics.record_block(height, block_latency, &block_dispatches);
                        } else {
                            metrics.record_height(height);
                        }
                        metrics.record_ops(block_ops.len());
                        let record = (!block_ops.is_empty()).then(|| {
                            noded::block_row(&noded::BlockRecord {
                                height,
                                hash: block_hash.map(|h| noded::hex_bytes(&h)).unwrap_or_default(),
                                commit_hash: block_app_hash.map(|h| hex(&h)).unwrap_or_default(),
                                ops: block_ops,
                            })
                        });
                        // this lane's agreed clock IS the height: the drain stamps
                        // BlockContext { consensus_time: height } for every block.
                        let ops = indexer::BlockOps {
                            record,
                            ..noded::index_block_ops(height, height, &block_dispatches)
                        };
                        if let Err(err) = index.apply_block(&ops) {
                            eprintln!(
                                "[node {label}] module index apply failed at height {height}: {err} \
                                 — wipe <storage>/index to rebuild"
                            );
                        }
                    }
                    for d in drained {
                        // a DISCARD is not this hold's outcome: the cutover
                        // carries the frame into the new epoch under the SAME
                        // FrameId, so the hold stays open until the carried
                        // frame finalizes there (or SUBMIT_HOLD expires into
                        // the truthful re-query reply).
                        if d.disposition == node::Disposition::Discarded {
                            continue;
                        }
                        // resolve a relayed hold FIRST: a relayed frame has no
                        // local pending_submits entry, so this must precede the
                        // `else { continue }` below or the wire Reply is lost.
                        if let Some((peer, _)) = pending_relays.remove(&d.id) {
                            let outcome = match d.disposition {
                                node::Disposition::Applied => relay::RelayOutcome::Applied {
                                    height: d.height,
                                    app_hash: hex(&d.app_hash),
                                },
                                node::Disposition::Rejected => relay::RelayOutcome::Rejected {
                                    // carry the module's VERBATIM reason (node-
                                    // local observability off the DrainedFrame)
                                    // so the resident forwards it to its caller
                                    // — the duckfs-client engine keys on the
                                    // "files: conflict:" prefix. generic wording
                                    // only when the drain captured no reason.
                                    detail: d.reason.clone().unwrap_or_else(|| {
                                        "op finalized but rejected (deterministic no-op)".into()
                                    }),
                                },
                                node::Disposition::Discarded => unreachable!("filtered at the loop top"),
                            };
                            let msg = relay::RelayMsg::Reply { frame_id: d.id, outcome };
                            let _ = relay_tx.send(
                                Recipients::One(peer),
                                IoBuf::from(relay::encode_msg(&msg)),
                                false,
                            );
                        }
                        let Some((reply, _)) = pending_submits.remove(&d.id) else { continue };
                        let _ = reply.send(match d.disposition {
                            node::Disposition::Applied => Ok(noded::BlockSummary {
                                height: d.height,
                                // the PER-BLOCK boundary this frame settled at
                                // (not the end-of-drain hash — a drain can
                                // apply several blocks).
                                app_hash: hex(&d.app_hash),
                            }),
                            node::Disposition::Rejected => Err(d.reason.clone().unwrap_or_else(
                                || {
                                    // the module's VERBATIM reason when the drain
                                    // captured one (duckfs-client keys on the
                                    // "files: conflict:" prefix); generic wording
                                    // otherwise.
                                    "op finalized but rejected (deterministic no-op)".into()
                                },
                            )),
                            // unreachable — filtered at the loop top — but
                            // stay total rather than panic.
                            node::Disposition::Discarded => continue,
                        });
                    }
                    validator_relay.expire(std::time::Instant::now(), &mut relay_tx);
                    // expire holds the mesh never finalized in time. the op may
                    // still land later — clients re-query on block events.
                    if !pending_submits.is_empty() {
                        let now = std::time::Instant::now();
                        let expired: Vec<node::FrameId> = pending_submits
                            .iter()
                            .filter(|(_, (_, deadline))| *deadline <= now)
                            .map(|(k, _)| *k)
                            .collect();
                        for k in expired {
                            if let Some((reply, _)) = pending_submits.remove(&k) {
                                let _ = reply.send(Err(
                                    "timed out awaiting finalization — re-query on the next block"
                                        .into(),
                                ));
                            }
                        }
                    }
                    // the same expiry contract for relayed holds: the mesh never
                    // finalized in time, so answer the resident truthfully — the
                    // op may still land, it re-queries on the next block.
                    if !pending_relays.is_empty() {
                        let now = std::time::Instant::now();
                        let expired: Vec<node::FrameId> = pending_relays
                            .iter()
                            .filter(|(_, (_, deadline))| *deadline <= now)
                            .map(|(k, _)| *k)
                            .collect();
                        for k in expired {
                            if let Some((peer, _)) = pending_relays.remove(&k) {
                                let msg = relay::RelayMsg::Reply {
                                    frame_id: k,
                                    outcome: relay::RelayOutcome::Refused {
                                        detail: "timed out awaiting finalization — re-query on the next block".into(),
                                    },
                                };
                                let _ = relay_tx.send(
                                    Recipients::One(peer),
                                    IoBuf::from(relay::encode_msg(&msg)),
                                    false,
                                );
                            }
                        }
                    }
                    // publish each newly-applied boundary to ws subscribers
                    // (send only errs when nobody is subscribed — fine). the
                    // drain loop above already folded each block into the
                    // metrics series; this tip seam carries the ws block
                    // summary only — it fires once per drain.
                    if let Some(f) = node.finalized()
                        && last_published != Some(f.height)
                    {
                        stream_hub.publish_block(f.height, hex(&f.app_hash));
                        last_published = Some(f.height);
                    }

                    // persist the finalization floor once everything at or
                    // below it has drained. read the certificate FIRST, the
                    // gate second: releases happen only on this thread, so a
                    // zero gate proves the cert's view is fully applied — a
                    // floor ahead of app state would suppress replay of
                    // finalized ops a restart still needs.
                    if let Some((view, cert)) = node.orderer().latest_finalization()
                        && view != 0
                        && node.orderer().unreleased_len() == 0
                    {
                        let height = orchestrator.app_height(view);
                        if last_cert_height.is_none_or(|h| height > h) {
                            let fc = recovery::FloorCert {
                                epoch: orchestrator.epoch(),
                                height,
                                cert,
                            };
                            match node.sink_mut().write_floor_cert(&fc).await {
                                Ok(()) => {
                                    last_cert_height = Some(height);
                                    latest_floor = Some(fc);
                                }
                                Err(e) => eprintln!(
                                    "[node {label}] floor cert write failed (will retry): {e}"
                                ),
                            }
                        }
                    }

                    // periodic checkpoint: snapshot the in-memory cohort and
                    // prune the op journal below the PREVIOUS checkpoint once
                    // the persisted floor has passed it (pruned frames must
                    // never be needed to resolve a re-reported finalization).
                    if blocks_since_checkpoint >= checkpoint_blocks
                        && let Some(f) = node.finalized()
                    {
                        let pos = node.sink_mut().oplog_pos().await;
                        let (cv, pu) = read_upgrade_version_fields(node.host()).await;
                        let captured = Manifest::capture(
                            node.host(),
                            Some(f.height),
                            orchestrator.epoch(),
                            orchestrator.epoch_base(),
                            participant_bytes(&orchestrator),
                            resident_bytes(&orchestrator),
                            orchestrator.pending_cutover().map(|c| c.cutover_view()),
                            cv,
                            pu,
                            pos,
                            next_seq,
                        );
                        match captured {
                            Ok(m) => match node.sink_mut().write_manifest(&m).await {
                                Ok(()) => {
                                    blocks_since_checkpoint = 0;
                                    let floor_passed = matches!(
                                        node.sink_mut().floor_cert(),
                                        Ok(Some(fc))
                                            if prev_ckpt.0.is_none_or(|h| fc.height >= h)
                                    );
                                    if floor_passed
                                        && let Err(e) =
                                            node.sink_mut().prune_oplog(prev_ckpt.1).await
                                    {
                                        eprintln!("[node {label}] oplog prune failed: {e}");
                                    }
                                    prev_ckpt = (m.height, pos);
                                }
                                Err(e) => eprintln!(
                                    "[node {label}] checkpoint write failed (will retry): {e}"
                                ),
                            },
                            Err(e) => eprintln!(
                                "[node {label}] checkpoint capture failed (will retry): {e}"
                            ),
                        }
                    }

                    // the VALSET ORCHESTRATION step: observe the finalized
                    // membership projection; a change schedules a deterministic
                    // cutover (arming the discard ceiling), and crossing the
                    // cutover view tears the engine down and respawns it over
                    // the set read AT the boundary. the observation barrier
                    // guarantees this tick's last view IS the changing block's
                    // view when membership moved.
                    if let Some(engine_view) = node.last_engine_view() {
                        // tick the reachability plane's freshness clock.
                        // engine views are EPOCH-LOCAL (they reset at every
                        // cutover), so convert to the absolute app-height
                        // clock (`epoch_base + view`) — the regime the boot
                        // Retarget's `view_base` put the plane's advert and
                        // handshake expiries in.
                        if let Some(cmd) = &reach_cmd {
                            let absolute_view = orchestrator.app_height(engine_view);
                            if last_reach_view.is_none_or(|v| v < absolute_view) {
                                // NON-BLOCKING: the plane is not consensus. a
                                // full command queue (a wedged or slow plane)
                                // sheds this tick — the next drain beat carries
                                // a fresher one — instead of stalling the loop
                                // behind an actor that may never drain.
                                let _ = cmd.try_send(
                                    reachability::ReachabilityCommand::ViewTick(absolute_view),
                                );
                                last_reach_view = Some(absolute_view);
                            }
                            // flush a staged cutover Retarget (see
                            // `pending_retarget`) — MUST eventually land, so
                            // it retries every beat rather than being shed.
                            if let Some(event) = pending_retarget.take()
                                && let Err(tokio::sync::mpsc::error::TrySendError::Full(
                                    reachability::ReachabilityCommand::Retarget(event),
                                )) = cmd.try_send(reachability::ReachabilityCommand::Retarget(
                                    event,
                                ))
                            {
                                pending_retarget = Some(event);
                            }
                        }
                        let members_raw = read_valset_members(node.host()).await;
                        let mut observed: Vec<ed25519::PublicKey> = Vec::new();
                        for key in &members_raw {
                            if let Ok(pk) = ed25519::PublicKey::decode(key.as_slice()) {
                                observed.push(pk);
                            }
                        }
                        // the RESIDENT projection, read at the same frozen
                        // point: a grant/revoke arms the same single cutover
                        // slot (mesh admission is epoch-scoped).
                        let residents_raw = read_valset_residents(node.host()).await;
                        let mut observed_residents: Vec<ed25519::PublicKey> = Vec::new();
                        for key in &residents_raw {
                            if let Ok(pk) = ed25519::PublicKey::decode(key.as_slice()) {
                                observed_residents.push(pk);
                            }
                        }
                        if let consensus::ObservationOutcome::Scheduled(cutover) =
                            orchestrator.observe_members(
                                engine_view,
                                observed.iter().cloned(),
                                observed_residents.iter().cloned(),
                            )
                        {
                            println!(
                                "[node {label}] membership change observed at view {} — cutover to epoch {} at view {}",
                                cutover.observed_view(),
                                cutover.next_epoch(),
                                cutover.cutover_view()
                            );
                            node.set_view_ceiling(cutover.cutover_view());
                        }
                        // a pending upgrade arms the SAME single cutover slot at its
                        // activation height (design §"One boundary carries both
                        // concerns") — never a competing arm: when a membership
                        // cutover already holds the slot `observe_upgrade` returns
                        // Pending and the version flip rides that boundary via the
                        // boundary read in `respawn_if_due`. inert until the module is
                        // registered (`read_upgrade_state` returns baseline/no-pending).
                        let boundary_upgrade = read_upgrade_state(node.host()).await;
                        if let Some(pending) = &boundary_upgrade.pending
                            && let consensus::ObservationOutcome::Scheduled(cutover) =
                                orchestrator.observe_upgrade(engine_view, pending.activation_height)
                        {
                            println!(
                                "[node {label}] upgrade '{}' armed — cutover to epoch {} at view {} (activation height {})",
                                pending.name,
                                cutover.next_epoch(),
                                cutover.cutover_view(),
                                pending.activation_height
                            );
                            node.set_view_ceiling(cutover.cutover_view());
                        }
                        if let Some(plan) = orchestrator.respawn_if_due(
                            engine_view,
                            observed,
                            observed_residents,
                            boundary_upgrade,
                        ) {
                            let members = plan.valset().consensus_members();
                            let member_bytes: Vec<Vec<u8>> =
                                members.iter().map(|k| k.as_ref().to_vec()).collect();
                            let plan_residents: Vec<ed25519::PublicKey> = plan
                                .valset()
                                .transport_members()
                                .difference(members)
                                .cloned()
                                .collect();
                            let plan_resident_bytes: Vec<Vec<u8>> = plan_residents
                                .iter()
                                .map(|k| k.as_ref().to_vec())
                                .collect();
                            // transport FIRST: the new epoch's mesh must admit
                            // its members (a fresh joiner — or a granted
                            // resident — above all) before anything is
                            // expected of them. the mesh tracks the TRANSPORT
                            // union; the engine below gets validators only.
                            // index = epoch, strictly increasing across
                            // cutovers.
                            mesh_oracle.track(plan.epoch(), mesh_at(plan.valset().transport_members()));
                            // the statesync plane serves (and admits) exactly
                            // who the mesh tracks — follow the re-track.
                            if let Some(book) = &sync_plane_book {
                                book.set_peers(plan.valset().transport_members().iter());
                            }
                            // the media planes authenticate inbound by the same
                            // tracked set — follow the re-track too, so a
                            // just-added member's huddle media is admitted.
                            if let Some(peers) = &media_peers {
                                peers.set_peers(plan.valset().transport_members().iter());
                            }
                            // the blob fetch-on-miss lane fans out to the same
                            // tracked set — follow the re-track.
                            *blob_peers.write().expect("blob peers lock") =
                                plan.valset().transport_members().iter().cloned().collect();
                            // the reachability plane retunnels for the new
                            // member set the moment transport admits it —
                            // with the epoch's resident tier as the pre-warm
                            // standbys, so a registered joiner's tunnels
                            // assemble ahead of its activation cutover.
                            // cutover_app_height IS the new epoch's absolute
                            // view at engine view 0 — the raw engine_view
                            // here would be epoch-local, a different clock
                            // than the ViewTicks above and the boot
                            // Retarget's view_base.
                            if reach_cmd.is_some() {
                                // STAGED, not sent inline: the flush below
                                // (every drain beat) try_sends it, so a plane
                                // whose queue is full delays retunneling by
                                // beats — it can never stall the cutover or
                                // the loop.
                                pending_retarget = Some(reachability::MeshEpochEvent {
                                    epoch: plan.epoch(),
                                    members: members.iter().cloned().collect(),
                                    standbys: plan_residents.clone(),
                                    current_view: plan.cutover_app_height(),
                                });
                            }
                            if !members.contains(&signer.public_key()) {
                                println!(
                                    "[node {label}] demoted from the validator set at epoch {} — halting (restart to serve as sync/resident)",
                                    plan.epoch()
                                );
                                std::process::exit(0);
                            }
                            let participants: Set<ed25519::PublicKey> = Set::try_from(
                                members.iter().cloned().collect::<Vec<_>>(),
                            )
                            .expect("orchestrator membership has no duplicates");
                            // a fresh epoch: new store (pins of the torn-down
                            // epoch die with it), genesis floor.
                            let orderer = spawn_epoch(
                                &mut channel_bank,
                                plan.epoch(),
                                participants,
                                ContentStore::new(),
                                None,
                            );
                            match node
                                .cutover(
                                    orderer,
                                    plan.epoch(),
                                    plan.cutover_app_height(),
                                    &member_bytes,
                                    &plan_resident_bytes,
                                )
                                .await
                            {
                                // the accept contract crossing the boundary:
                                // every locally-accepted op the old epoch
                                // never resolved was re-proposed into the
                                // new engine.
                                Ok(carried) if carried > 0 => println!(
                                    "[node {label}] carried {carried} accepted ops across the cutover into epoch {}",
                                    plan.epoch()
                                ),
                                Ok(_) => {}
                                Err(e) => {
                                    eprintln!("[node {label}] FATAL: {e} — halting");
                                    std::process::exit(1);
                                }
                            }
                            // ACTIVATION (design §4): realize the agreed boundary
                            // protocol version into every dual-path module's
                            // active_version (branch selector) at H. driven ONLY by
                            // the agreed `plan.boundary_version()` — deterministic,
                            // non-hashed. the upgrade module's OWN committed
                            // reconciliation (current_version flip + pending clear on
                            // ARM, clear-only on ABORT) is NOT done here: it rides the
                            // single in-block System `Advance` the host drain injects
                            // at the same finalized view (Task 6.3), so both concerns
                            // land at ONE boundary and every node agrees. do NOT branch
                            // a separate abort-only follow-up — the one Advance owns both.
                            node.host_mut().set_active_version(plan.boundary_version());
                            match plan.upgrade_verdict() {
                                consensus::UpgradeVerdict::Armed { name, to_version } => println!(
                                    "[node {label}] upgrade activated name={name} version={to_version} at height {}",
                                    plan.cutover_app_height()
                                ),
                                consensus::UpgradeVerdict::Abort { name } => println!(
                                    "[node {label}] upgrade aborted name={name} (unmet readiness) at height {} — network continues on version {}",
                                    plan.cutover_app_height(),
                                    plan.boundary_version()
                                ),
                                consensus::UpgradeVerdict::None => {}
                            }
                            // checkpoint IMMEDIATELY: the manifest must record
                            // the new epoch's participant set (the journal's
                            // cutover record alone covers only the crash
                            // window until this write lands).
                            let pos = node.sink_mut().oplog_pos().await;
                            // post-boundary committed version fields: after an armed
                            // Advance the module holds `current_version = to_version`
                            // + no pending, so this checkpoint stamps the new baseline.
                            let (cv, pu) = read_upgrade_version_fields(node.host()).await;
                            let captured = Manifest::capture(
                                node.host(),
                                node.finalized().map(|f| f.height),
                                orchestrator.epoch(),
                                orchestrator.epoch_base(),
                                participant_bytes(&orchestrator),
                                resident_bytes(&orchestrator),
                                None,
                                cv,
                                pu,
                                pos,
                                next_seq,
                            );
                            match captured {
                                Ok(m) => match node.sink_mut().write_manifest(&m).await {
                                    Ok(()) => {
                                        blocks_since_checkpoint = 0;
                                        prev_ckpt = (m.height, pos);
                                    }
                                    Err(e) => eprintln!(
                                        "[node {label}] post-cutover checkpoint write failed \
                                         (the journal's cutover record covers a restart): {e}"
                                    ),
                                },
                                Err(e) => eprintln!(
                                    "[node {label}] post-cutover checkpoint capture failed \
                                     (the journal's cutover record covers a restart): {e}"
                                ),
                            }
                            println!(
                                "[node {label}] cutover complete: epoch {} with {} validators (app height base {})",
                                plan.epoch(),
                                members.len(),
                                plan.cutover_app_height()
                            );
                        }
                    }

                    // BLOCK CADENCE + heartbeat, unified. `submit`/`submit_frame`
                    // now ENQUEUE into the node's `pending_batch`; this is the one
                    // place per block-time that FLUSHES the window — packing every
                    // frame that arrived in it (real ops and/or an idle nop) into
                    // ONE batch super-frame and proposing it as a single block.
                    // that is the aggregation: at most one block per BLOCK_TIME,
                    // carrying all the window's txs, never 1-tx-1-block.
                    //
                    // the idle nop still exists: finalized views only advance with
                    // a proposed frame, so an idle network would freeze (its height
                    // never ticks and a pending cutover, which crosses only when
                    // finalized views REACH it, would park forever). so on an EMPTY
                    // window inject one deterministically-rejected nop (unknown
                    // module target: rejects identically everywhere, leaves no
                    // state) and flush that. a window with real ops needs no nop —
                    // the ops ARE the block.
                    //
                    // GATE the idle nop on an empty orderer FIFO too: a nop pushed
                    // while a batch still awaits finalization only piles behind a
                    // finalization stall (a flapping quorum peer would stack idle
                    // blocks). real ops are never gated — they must not wait.
                    if !heartbeat_disabled && last_flush.elapsed() >= consensus::BLOCK_TIME {
                        last_flush = std::time::Instant::now();
                        if node.pending_batch_len() == 0 && node.orderer().pending_len() == 0 {
                            let seq = next_seq;
                            next_seq += 1;
                            if let Err(e) = node
                                .submit(
                                    &signer,
                                    seq,
                                    Msg { target: NOP_TARGET.into(), payload: Vec::new() },
                                )
                                .await
                            {
                                eprintln!("[node {label}] heartbeat nop submit failed: {e}");
                            }
                        }
                        // flush the window: no-op when `pending_batch` is empty
                        // (idle with a batch already in flight — wait for it).
                        if let Err(e) = node.flush_batch().await {
                            eprintln!("[node {label}] batch flush failed: {e}");
                        }
                    }

                    // READINESS SIGNAL (design §3 / plan Task 7.1): a current
                    // boundary member whose binary can execute the pending upgrade
                    // self-submits ONE `SignalReady`. gated to a current member (the
                    // R = n readiness denominator); the signaller's own committed
                    // read + local latch keep it idempotent. inert on a baseline net.
                    if orchestrator
                        .current_members()
                        .contains(&signer.public_key())
                        && let Some((msg, name, to_version)) =
                            signaller.maybe_signal(node.host()).await
                    {
                        let seq = next_seq;
                        next_seq += 1;
                        match node.submit(&signer, seq, msg).await {
                            Ok(_) => println!(
                                "[node {label}] signaled ready name={name} to_version={to_version}"
                            ),
                            Err(e) => {
                                // un-latch so a transient submit failure retries on
                                // the next tick (the module stays idempotent).
                                signaller.signaled = None;
                                eprintln!("[node {label}] readiness signal submit failed: {e}");
                            }
                        }
                    }

                    // CAPABILITY ANNOUNCE: a current member whose discovered
                    // provider set differs from the committed registry
                    // self-submits ONE declarative `Announce`. member-gated (the
                    // module rejects non-members) and idempotent (committed-read
                    // + local latch). inert on a host with no executor CLIs, and
                    // suppressed entirely under `announce_capabilities = false`
                    // (the accept-lane-only provider: this node still executes
                    // what it can, but only by claiming unassigned announcements
                    // — it never enters a tag's rendezvous pool).
                    if announce_capabilities
                        && orchestrator
                            .current_members()
                            .contains(&signer.public_key())
                        && let Some(msg) = announcer.maybe_announce(node.host()).await
                    {
                        let seq = next_seq;
                        next_seq += 1;
                        match node.submit(&signer, seq, msg).await {
                            Ok(_) => println!(
                                "[node {label}] announced capabilities {:?}",
                                announcer.capabilities
                            ),
                            Err(e) => {
                                // un-latch so a transient submit failure retries.
                                announcer.announced = None;
                                eprintln!("[node {label}] capability announce submit failed: {e}");
                            }
                        }
                    }

                    // SAGA CRANK (P7 liveness, host side): nothing else ever
                    // submits `SagaMsg::Crank`, and under strict leases a
                    // saga whose assignee went dark advances ONLY via a crank
                    // (lease re-lease or deadline timeout). state-driven:
                    // when the committed next expiry is at or past the latest
                    // finalized height, push one permissionless crank —
                    // throttled like the heartbeat, since a backlog wider
                    // than CRANK_BUDGET legitimately needs several. duplicate
                    // cranks from other nodes are deterministic no-ops.
                    if last_crank.elapsed() >= consensus::BLOCK_TIME
                        && let Some(finalized_height) = node.finalized().map(|f| f.height)
                        && let Some(expiry) = saga_next_expiry(node.host()).await
                        && expiry <= finalized_height
                    {
                        last_crank = std::time::Instant::now();
                        let seq = next_seq;
                        next_seq += 1;
                        if let Err(e) = node
                            .submit(
                                &signer,
                                seq,
                                Msg {
                                    target: "saga".into(),
                                    payload: saga::encode_msg(
                                        &saga::SagaMsg::Crank {},
                                    ),
                                },
                            )
                            .await
                        {
                            eprintln!("[node {label}] saga crank submit failed: {e}");
                        } else {
                            println!(
                                "[node {label}] saga crank submitted \
                                 (next expiry {expiry} <= height {finalized_height})"
                            );
                        }
                    }

                    // DISPATCH DELIVERY NUDGE (never-pop-stack liveness): a
                    // result committed into the dispatch mailbox delivers via
                    // the drain's DeliverPending injection in the NEXT
                    // successful block — and heartbeat nops are rejected
                    // frames that never apply, so a quiet chain would sit on
                    // its mailbox. state-driven: while the committed mailbox
                    // is non-empty, push one permissionless Nudge — a no-op
                    // whose block carries the injection. duplicate nudges
                    // from other nodes are free.
                    if last_nudge.elapsed() >= consensus::BLOCK_TIME
                        && dispatch_pending_deliveries(node.host()).await > 0
                    {
                        last_nudge = std::time::Instant::now();
                        let seq = next_seq;
                        next_seq += 1;
                        if let Err(e) = node
                            .submit(
                                &signer,
                                seq,
                                Msg {
                                    target: "dispatch".into(),
                                    payload: dispatch::encode_msg(
                                        &dispatch::DispatchMsg::Nudge {},
                                    ),
                                },
                            )
                            .await
                        {
                            eprintln!("[node {label}] dispatch nudge submit failed: {e}");
                        } else {
                            println!("[node {label}] dispatch delivery nudge submitted");
                        }
                    }

                    // UPGRADE TRANSITION MARKERS (one-shot, committed-state driven):
                    // the greppable proof surface the e2e keys on. `armed` is the
                    // module's own R==n verdict (pending set, boundary non-empty,
                    // every current member signaled), so this fires exactly when
                    // readiness first reaches the full set — before H is crossed.
                    if let Some(st) = read_upgrade_status_raw(node.host()).await {
                        match &st.pending {
                            Some(up) => {
                                upgrade_pending_seen = Some(up.name.clone());
                                let key = (up.name.clone(), up.to_version);
                                if st.armed && upgrade_armed_latch.as_ref() != Some(&key) {
                                    println!(
                                        "[node {label}] upgrade armed name={} to_version={} height={}",
                                        up.name, up.to_version, up.activation_height
                                    );
                                    upgrade_armed_latch = Some(key);
                                }
                            }
                            None => {
                                if let Some(name) = upgrade_pending_seen.take() {
                                    // the boundary Advance reconciled the pending
                                    // (ARM flip or ABORT clear) — the slot is free.
                                    println!("[node {label}] upgrade cleared name={name}");
                                    upgrade_armed_latch = None;
                                }
                            }
                        }
                    }

                    // the reactor seam: offer each finalized block's effects to
                    // the host-owned workers; a claiming worker's follow-up op
                    // re-enters through the ordered lane as its own block (the
                    // oracle-as-op). unclaimed effects are logged, not silently
                    // dropped — a saga stuck Pending should be visible.
                    for eff in node.take_effects() {
                        let mut claimed = false;
                        for w in &workers {
                            match w.run(&eff).await {
                                Ok(reactor::WorkOutcome::Handled(Some(follow))) => {
                                    let seq = next_seq;
                                    next_seq += 1;
                                    if let Err(e) =
                                        node.submit(&signer, seq, follow).await
                                    {
                                        eprintln!("[node {label}] worker follow-up submit failed: {e}");
                                    }
                                    claimed = true;
                                    break;
                                }
                                // a deliberate skip (e.g. leased to another
                                // node): claimed, nothing to submit.
                                Ok(reactor::WorkOutcome::Handled(None)) => {
                                    claimed = true;
                                    break;
                                }
                                Ok(reactor::WorkOutcome::NotMine) => {}
                                Err(e) => {
                                    eprintln!("[node {label}] worker error: {e}");
                                    claimed = true; // errored ≠ unclaimed; don't double-log
                                    break;
                                }
                            }
                        }
                        if !claimed {
                            println!(
                                "[node {label}] effect with no worker ({} bytes) — dropped",
                                eff.0.len()
                            );
                        }
                    }
                    if dev_demo && !converged && applied >= expected {
                        let h = node.app_hash();
                        println!("[node {label}] converged app_hash={}", hex(&h));
                        // dump every directory key so the demo can eyeball the ops
                        // (each node ends holding the op it originated AND the peer's).
                        for k in 0..expected {
                            let reply = node
                                .host()
                                .query("directory", &encode_query(&DirQuery::Get { key: format!("k{k}") }))
                                .await
                                .expect("directory query");
                            if let Ok(DirReply::Value(v)) = decode_reply(&reply) {
                                println!("[node {label}]   directory k{k}={v:?}");
                            }
                        }
                        converged = true;
                    }
                }
                job = rpc_ingress.next() => {
                    let Some((req, reply)) = job else { continue };
                    let resp = match req {
                        RpcRequest::Submit { target, payload_hex } => {
                            match unhex(&payload_hex) {
                                Ok(payload) => {
                                    let seq = next_seq;
                                    next_seq += 1;
                                    match node
                                        .submit(&signer, seq, Msg { target, payload })
                                        .await
                                    {
                                        Ok(_) => RpcReply::ok(),
                                        Err(e) => RpcReply::err(format!("submit failed: {e}")),
                                    }
                                }
                                Err(e) => RpcReply::err(format!("bad payload_hex: {e}")),
                            }
                        }
                        RpcRequest::Query { target, req_hex } => match unhex(&req_hex) {
                            Ok(req_bytes) => match node.host().query(&target, &req_bytes).await {
                                Ok(bytes) => RpcReply {
                                    reply_hex: Some(hex_bytes(&bytes)),
                                    ..RpcReply::ok()
                                },
                                Err(e) => RpcReply::err(format!("query failed: {e}")),
                            },
                            Err(e) => RpcReply::err(format!("bad req_hex: {e}")),
                        },
                        RpcRequest::Status => {
                            let mut modules = std::collections::BTreeMap::new();
                            for m in MODULE_IDS {
                                if let Some(root) = node.host().module_root(m) {
                                    modules.insert(m.to_string(), hex(&root));
                                }
                            }
                            RpcReply {
                                status: Some(RpcStatus {
                                    height: node.finalized().map(|f| f.height),
                                    app_hash: hex(&node.app_hash()),
                                    modules,
                                }),
                                ..RpcReply::ok()
                            }
                        }
                        RpcRequest::JoinRequests => {
                            // read-time hygiene: an approved joiner holds
                            // STANDING now (resident or already validator) —
                            // its request is settled, drop it.
                            let members = read_members_from_host(node.host()).await;
                            let residents_now = read_valset_residents(node.host()).await;
                            join_requests.retain(|joiner, _| {
                                !members.contains(joiner) && !residents_now.contains(joiner)
                            });
                            let views = join_requests
                                .iter()
                                .map(|(joiner, r)| JoinRequestView {
                                    joiner: hex_bytes(joiner),
                                    issuer: hex_bytes(&r.issuer),
                                    first_seen_ms: r.first_seen_ms,
                                    last_seen_ms: r.last_seen_ms,
                                })
                                .collect();
                            RpcReply {
                                join_requests: Some(views),
                                ..RpcReply::ok()
                            }
                        }
                        RpcRequest::Shutdown => {
                            // best-effort final checkpoint + journal barrier so
                            // the restart replays a minimal suffix; a failure
                            // here is just the crash path, which also recovers.
                            // SAME sequence as the signal arm (shared macro).
                            graceful_checkpoint!();
                            let _ = reply.send(RpcReply::ok());
                            println!("[node {label}] shutdown requested via rpc — exiting");
                            std::process::exit(0);
                        }
                    };
                    let _ = reply.send(resp);
                }
                result = oracle_results.next() => {
                    // a completed off-loop provider run: its OracleResult op
                    // re-enters the ordered lane as an ordinary signed
                    // submit — the oracle-as-op, unchanged; only WHERE the
                    // provider ran moved.
                    let Some(msg) = result else { continue };
                    let seq = next_seq;
                    next_seq += 1;
                    if let Err(e) = node.submit(&signer, seq, msg).await {
                        eprintln!("[node {label}] oracle result submit failed: {e}");
                    }
                }
                announce = lobby_ingress.next() => {
                    let Some((peer, bytes)) = announce else { continue };
                    // `fatal: true` marks the refusal PERMANENT for this
                    // invite — the joiner stops re-announcing instead of
                    // spinning on a token that can never redeem.
                    let mut send_reply = |recorded: bool, detail: String, cap: Option<Vec<u8>>, fatal: bool| {
                        let msg = lobby::LobbyMsg::JoinReply { recorded, detail, cap, fatal };
                        let _ = lobby_tx.send(
                            Recipients::One(peer.clone()),
                            IoBuf::from(lobby::encode_msg(&msg)),
                            false,
                        );
                    };
                    let msg = match lobby::decode_msg(&bytes) {
                        Ok(m) => m,
                        Err(_) => continue, // junk on the doorbell — drop.
                    };
                    // crypto first (pure, cheap): the token must verify for
                    // THIS network and the announced key must prove itself.
                    let verified = match lobby::verify_join_request(&msg, &namespace) {
                        Ok(v) => v,
                        Err(e) => {
                            send_reply(false, e, None, false);
                            continue;
                        }
                    };
                    // then membership: the issuer must still be a member (a
                    // removed member's outstanding invites die with it), and a
                    // joiner that already holds standing — VALIDATOR or
                    // RESIDENT — has nothing pending.
                    let members = read_members_from_host(node.host()).await;
                    let residents_now = read_valset_residents(node.host()).await;
                    let joiner_bytes = verified.joiner.as_ref().to_vec();
                    if members.contains(&joiner_bytes) {
                        send_reply(false, "already a validator".into(), None, false);
                        continue;
                    }
                    if residents_now.contains(&joiner_bytes) {
                        send_reply(
                            false,
                            "already a resident — a member promotes it into the quorum".into(),
                            None,
                            false,
                        );
                        continue;
                    }
                    if !members.contains(&verified.issuer.as_ref().to_vec()) {
                        send_reply(
                            false,
                            "the inviting member is no longer part of this network".into(),
                            None,
                            false,
                        );
                        continue;
                    }
                    // SPENT-INVITE check: the token's nonce is the
                    // exactly-once key (governance's Redeem handler). a nonce
                    // already redeemed by ANOTHER key can never redeem again —
                    // resubmitting the op is pointless and the joiner would
                    // spin on "redemption not landed yet" forever. fail it
                    // loudly and permanently on both ends instead. (redeemed
                    // by the SAME key = standing already granted; the
                    // validator/resident checks above answered that.)
                    let redemptions = read_redemptions_from_host(node.host()).await;
                    if let Some(spent) = redemptions
                        .iter()
                        .find(|r| r.nonce == verified.nonce.as_slice() && r.joiner != joiner_bytes)
                    {
                        println!(
                            "[node {label}] lobby: {} presented an ALREADY-REDEEMED invite \
                             (spent by {} at height {}) — refusing permanently; an invite \
                             admits exactly one person, mint a fresh one per joiner",
                            hex_bytes(&joiner_bytes[..4]),
                            hex_bytes(&spent.joiner[..4.min(spent.joiner.len())]),
                            spent.height,
                        );
                        send_reply(
                            false,
                            "invite already redeemed — an invite admits exactly one person; \
                             ask the inviter for a fresh invite"
                                .into(),
                            None,
                            true,
                        );
                        continue;
                    }
                    // AUTO-REDEMPTION: minting the invite WAS the approval, so
                    // a verified announce submits the governance Redeem op on
                    // the joiner's behalf — no human step. every validator
                    // re-verifies the token in-consensus and the nonce set
                    // makes it single-use, so racing members (the joiner
                    // round-robins its announce) collapse to one grant and
                    // deterministic rejects. the in-memory map only throttles
                    // re-submits across the joiner's ~3s re-announces.
                    let now = unix_ms();
                    let fresh = !join_requests.contains_key(&joiner_bytes);
                    let record = join_requests
                        .entry(joiner_bytes)
                        .or_insert(JoinRequestRecord {
                            issuer: verified.issuer.as_ref().to_vec(),
                            first_seen_ms: now,
                            last_seen_ms: 0,
                        });
                    // MINT the coordinator capability for the joiner, additive
                    // and side-effect-free (a pure ed25519 sign — no consensus,
                    // no valset change). Gated: only a GENESIS validator on a
                    // PRIVATE network issues one — its key is in the
                    // coordinator's pinned genesis set, so the cap it signs
                    // actually admits. A public network needs no cap; a
                    // non-genesis member cannot mint one the coordinator trusts.
                    // The cap cannot ride the invite (the joiner's key did not
                    // exist at invite-mint time), so the JoinReply is its only
                    // delivery channel — re-delivered on every re-announce in
                    // case a reply was lost. Rotation is DEFERRED — the cap is
                    // long-lived (COORD_CAP_TTL_SECS).
                    let minted_cap = if coordination == config::Coordination::Private
                        && validators.contains(&signer.public_key())
                    {
                        let mut subj = [0u8; 32];
                        subj.copy_from_slice(verified.joiner.as_ref());
                        let cap = nat_traversal::mint_coord_cap(
                            &signer,
                            nat_traversal::NodeKey(subj),
                            nat_traversal::now_secs() + nat_traversal::COORD_CAP_TTL_SECS,
                        );
                        Some(config::pack_coord_cap(&cap))
                    } else {
                        None
                    };
                    const REDEEM_RESUBMIT_MS: u64 = 30_000;
                    if !fresh && now.saturating_sub(record.last_seen_ms) < REDEEM_RESUBMIT_MS {
                        send_reply(
                            true,
                            "redemption in flight — standing lands shortly".into(),
                            minted_cap,
                            false,
                        );
                        continue;
                    }
                    record.last_seen_ms = now;
                    let redeem = governance::GovMsg::Redeem {
                        issuer: verified.issuer.as_ref().to_vec(),
                        nonce: verified.nonce.to_vec(),
                        token_sig: match &msg {
                            lobby::LobbyMsg::JoinRequest { token_sig, .. } => token_sig.clone(),
                            _ => unreachable!("verified above"),
                        },
                        joiner: verified.joiner.as_ref().to_vec(),
                        proof: match &msg {
                            lobby::LobbyMsg::JoinRequest { proof, .. } => proof.clone(),
                            _ => unreachable!("verified above"),
                        },
                    };
                    let seq = next_seq;
                    next_seq += 1;
                    match node
                        .submit(
                            &signer,
                            seq,
                            Msg {
                                target: "governance".into(),
                                payload: governance::encode_msg(&redeem),
                            },
                        )
                        .await
                    {
                        Ok(_) => {
                            println!(
                                "[node {label}] invite redemption submitted: {} (invited by {})",
                                hex_bytes(verified.joiner.as_ref()),
                                hex_bytes(verified.issuer.as_ref())
                            );
                            send_reply(
                                true,
                                "invite verified — redemption submitted, resident standing \
                                 lands at the next block"
                                    .into(),
                                minted_cap,
                                false,
                            );
                        }
                        Err(e) => {
                            send_reply(false, format!("redemption submit failed: {e}"), None, false);
                        }
                    }
                }
                relayed = relay_ingress.next() => {
                    let Some((peer, bytes)) = relayed else { continue };
                    let Ok(msg) = relay::decode_msg(&bytes) else { continue };
                    let needs_standing = matches!(
                        msg,
                        relay::RelayMsg::BlobOffer { .. } | relay::RelayMsg::Submit { .. }
                    );
                    let (members_now, residents_now) = if needs_standing {
                        (
                            read_valset_members(node.host()).await,
                            read_valset_residents(node.host()).await,
                        )
                    } else {
                        (Vec::new(), Vec::new())
                    };
                    let Some(action) = validator_relay.on_message(
                        peer,
                        msg,
                        &members_now,
                        &residents_now,
                        &mut relay_tx,
                    ) else {
                        continue;
                    };
                    match action {
                        relay_runtime::ValidatorAction::SubmitResident {
                            frame_id,
                            frame,
                            peer,
                        } => match node.submit_frame(frame).await {
                            Ok(id) => {
                                debug_assert_eq!(id, frame_id);
                                pending_relays.insert(
                                    id,
                                    (peer, std::time::Instant::now() + SUBMIT_HOLD),
                                );
                            }
                            Err(e) => relay_runtime::send_reply(
                                &mut relay_tx,
                                &peer,
                                frame_id,
                                relay::RelayOutcome::Refused {
                                    detail: format!("submit failed: {e}"),
                                },
                            ),
                        },
                        relay_runtime::ValidatorAction::SubmitLocal {
                            frame_id,
                            frame,
                            reply,
                            deadline,
                        } => match node.submit_frame(frame).await {
                            Ok(id) => {
                                debug_assert_eq!(id, frame_id);
                                pending_submits.insert(id, (reply, deadline));
                            }
                            Err(e) => {
                                let _ = reply.send(Err(format!("submit failed: {e}")));
                            }
                        },
                    }
                }
                cmd = http_ingress.next() => {
                    let Some(cmd) = cmd else { continue };
                    match cmd {
                        // `origin` is the caller's CLAIMED submitter identity —
                        // meaningful on the embedded daemon, but this lane signs
                        // frames, and the signed origin IS this node's pubkey
                        // (authenticated authorship that governance relies on).
                        // a claimed origin cannot ride a signed frame without
                        // making authorship forgeable, so it is ignored here;
                        // display names resolve via the name registry instead.
                        noded::NodeCommand::Submit { target, payload, origin: _, reply } => {
                            let seq = next_seq;
                            next_seq += 1;
                            let frame = node::encode_frame(&signer, seq, &Msg { target, payload });
                            let peers: Vec<ed25519::PublicKey> =
                                if relay::required_blob_digest(&frame).is_some() {
                                    read_valset_members(node.host())
                                        .await
                                        .iter()
                                        .filter_map(|raw| {
                                            ed25519::PublicKey::decode(raw.as_slice()).ok()
                                        })
                                        .filter(|key| key != &signer.public_key())
                                        .collect()
                                } else {
                                    Vec::new()
                                };
                            match validator_relay.prepare_local(
                                frame,
                                reply,
                                peers,
                                &mut relay_tx,
                            ) {
                                Ok(Some(relay_runtime::ValidatorAction::SubmitLocal {
                                    frame_id,
                                    frame,
                                    reply,
                                    deadline,
                                })) => match node.submit_frame(frame).await {
                                    Ok(id) => {
                                        debug_assert_eq!(id, frame_id);
                                        pending_submits.insert(id, (reply, deadline));
                                    }
                                    Err(e) => {
                                        let _ = reply.send(Err(format!("submit failed: {e}")));
                                    }
                                },
                                Ok(Some(relay_runtime::ValidatorAction::SubmitResident { .. })) => {
                                    unreachable!("local preparation returns a local action")
                                }
                                Ok(None) => {}
                                Err((reply, detail)) => {
                                    let _ = reply.send(Err(detail));
                                }
                            }
                        }
                        noded::NodeCommand::Query { target, req, reply } => {
                            let result = node
                                .host()
                                .query(&target, &req)
                                .await
                                .map_err(|e| e.to_string());
                            let _ = reply.send(result);
                        }
                        noded::NodeCommand::Status { reply } => {
                            let modules = MODULE_IDS
                                .iter()
                                .map(|m| noded::ModuleStatus {
                                    id: (*m).into(),
                                    root: node
                                        .host()
                                        .module_root(m)
                                        .map(|r| hex(&r))
                                        .unwrap_or_default(),
                                    category: noded::ModuleCategory::of(m),
                                })
                                .collect();
                            let _ = reply.send(noded::NodeStatus {
                                version: env!("CARGO_PKG_VERSION").into(),
                                app_hash: hex(&node.app_hash()),
                                height: node.finalized().map(|f| f.height).unwrap_or(0),
                                modules,
                                public_key: status_public_key.clone(),
                            });
                        }
                        noded::NodeCommand::Metrics { reply } => {
                            // one registry: commonware's runtime series plus the
                            // `ducktape_*` block series the drain loop records.
                            let _ = reply.send(context.encode());
                        }
                    }
                }
                req = sync_state_rx.next() => {
                    // the statesync serve task's state touches (the
                    // [`SyncStateRequest`] seam): each is one bounded read
                    // against loop-owned state — the heavy serving (decode,
                    // captures, slicing, replies) lives on the serve task.
                    let Some(req) = req else {
                        // the serve task ended (network shutdown) — nothing
                        // left to answer; keep draining consensus regardless.
                        continue;
                    };
                    match req {
                        SyncStateRequest::Boundary { known, reply } => {
                            // the boundary's consensus coordinates ride the manifest.
                            // the floor certificate is served only when it certifies
                            // exactly the current boundary — a cert behind the
                            // boundary would make a joiner skip history it needs.
                            // stamp the served boundary's committed version fields from
                            // live upgrade state (like epoch/view_base). a joiner installs
                            // its dual-path modules at `current_version` and preflights
                            // against `required_min_version` — both derived from these.
                            let (bc_current, bc_pending) =
                                read_upgrade_version_fields(node.host()).await;
                            let coords = statesync::BoundaryCoords {
                                epoch: orchestrator.epoch(),
                                view_base: orchestrator.epoch_base(),
                                participants: participant_bytes(&orchestrator),
                                residents: resident_bytes(&orchestrator),
                                current_version: bc_current,
                                pending_upgrade: bc_pending,
                                floor_cert: latest_floor
                                    .as_ref()
                                    .filter(|fc| fc.epoch == orchestrator.epoch())
                                    .filter(|fc| {
                                        node.finalized().is_some_and(|f| f.height == fc.height)
                                    })
                                    .map(|fc| fc.cert.clone()),
                            };
                            let finalized_for_sync = node.finalized().filter(|f| {
                                f.height <= coords.view_base || coords.floor_cert.is_some()
                            });
                            let answer = match finalized_for_sync {
                                // two refusals, named apart: no boundary at
                                // all (pre-first-block), vs the per-block
                                // window where the tip advanced but its
                                // finalization certificate has not persisted
                                // yet — a retry lands once they align.
                                None => Err(match node.finalized() {
                                    Some(f) => format!(
                                        "boundary {} awaiting its finalization certificate — \
                                         retry",
                                        f.height
                                    ),
                                    None => "no finalized boundary to serve yet".to_string(),
                                }),
                                Some(finalized) => {
                                    let id = statesync::BoundaryId {
                                        height: finalized.height,
                                        app_hash: finalized.app_hash,
                                    };
                                    if known.contains(&id) {
                                        // the serve task holds this boundary's
                                        // payload — coordinates only.
                                        Ok(SyncBoundary { id, coords, data: None })
                                    } else {
                                        statesync::capture_boundary(
                                            node.host(),
                                            finalized,
                                            &coords,
                                        )
                                        .await
                                        .map(|(id, data)| SyncBoundary {
                                            id,
                                            coords,
                                            data: Some(data),
                                        })
                                    }
                                }
                            };
                            let _ = reply.send(answer);
                        }
                        SyncStateRequest::ModuleServe { module_id, body, reply } => {
                            let served = node
                                .host()
                                .serve_sync(&module_id, &body)
                                .await
                                .map_err(|e| format!("module {module_id} serve_sync: {e}"));
                            let _ = reply.send(served);
                        }
                        SyncStateRequest::Frames { after_height, up_to_height, reply } => {
                            let read = node
                                .sink_mut()
                                .read_finalized_frames(after_height, up_to_height)
                                .await;
                            let _ = reply.send(read);
                        }
                        SyncStateRequest::IndexCut { reply } => {
                            let _ = reply.send(ship_index_blobs(&index, &label));
                        }
                        SyncStateRequest::TipCoords { reply } => {
                            // the detection lane: everything here is already
                            // loop-owned state — no capture, and deliberately
                            // no floor-cert alignment gate. that gate protects
                            // a JOINER from syncing a boundary whose history
                            // it would skip; a detection reply carries a
                            // presence bit, never certificate bytes, and every
                            // action taken on it (ascension, promotion)
                            // re-fetches a full manifest through the gated
                            // Boundary path.
                            let answer = match node.finalized() {
                                None => Err("no finalized boundary to serve yet".to_string()),
                                Some(f) => Ok(statesync::TipCoords {
                                    height: f.height,
                                    app_hash: f.app_hash,
                                    epoch: orchestrator.epoch(),
                                    view_base: orchestrator.epoch_base(),
                                    participants: participant_bytes(&orchestrator),
                                    residents: resident_bytes(&orchestrator),
                                    has_floor: latest_floor
                                        .as_ref()
                                        .filter(|fc| fc.epoch == orchestrator.epoch())
                                        .is_some_and(|fc| fc.height == f.height),
                                }),
                            };
                            let _ = reply.send(answer);
                        }
                    }
                }
            }
        }
    });

    Ok(())
}
