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

use commonware_cryptography::Signer;
use commonware_runtime::{Runner, Supervisor};
use tracing_subscriber::prelude::*;

mod agent_plane;
mod blob_fetch;
mod boot;
mod code_plane;
mod cli;
mod cli_flags;
mod config;
mod constants;
mod drain_actions;
mod explorer;
mod first_contact_join;
mod gateway_plane;
mod gateway_routes;
mod host_reads;
mod host_resources;
mod host_state;
#[cfg(test)]
mod joiner_mesh_tests;
mod lobby;
#[cfg(test)]
mod main_tests;
mod oracle_pool;
mod overlay_book;
mod plane_metrics;
mod reachability_plane;
#[cfg(test)]
mod reachability_plane_tests;
mod relay;
mod relay_runtime;
mod replica;
mod resident_announce;
mod resident_dispatch;
mod resource_limits;
mod rpc;
mod sync;
mod userkey;
mod userkey_cli;
mod util;
mod validator;
mod voice;
mod voice_plane;
use config::Resolved;
use constants::*;
#[cfg(test)]
use explorer::explorer_root_op;
#[cfg(test)]
use explorer::sealed_frame_block_row;
#[cfg(test)]
use replica::promotion::{
    PromotionBoundary, PromotionBoundarySource, choose_promotion_boundary,
    joiner_manifest_fetch_retry,
};
#[cfg(test)]
use sync::catchup::{apply_post_reboot_catchup_frames, apply_verified_suffix_frame};
#[cfg(test)]
use sync::serve::assert_floor_binds_view;
#[cfg(test)]
use util::hex;
#[cfg(test)]
use validator::announce::ReadinessSignaller;

#[cfg(test)]
use directory::{DirQuery, DirReply, decode_reply, encode_query};
use duckfs_disk::SyncScratch;
use recovery::Recovery;
#[cfg(test)]
use sdk::{Msg, StateRoot};

fn main() {
    resource_limits::raise_open_file_limit();
    #[cfg(target_os = "macos")]
    hold_macos_activity();
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

/// macOS: opt this process out of App Nap for its whole life. the desktop
/// shell spawns the node detached but the child stays in the app's darwin
/// coalition, and the app hides to the menu bar with zero visible windows —
/// exactly the state macOS answers with timer coalescing and I/O throttling.
/// a 1s-block consensus follower cannot survive either. the option set
/// deliberately ALLOWS idle system sleep (a node must never turn a laptop
/// into a space heater); `LatencyCritical` additionally asks for full timer
/// precision. the returned token re-enables the nap when it deallocates, so
/// it is forgotten, never dropped.
#[cfg(target_os = "macos")]
fn hold_macos_activity() {
    use objc2_foundation::{NSActivityOptions, NSProcessInfo, NSString};
    let token = NSProcessInfo::processInfo().beginActivityWithOptions_reason(
        NSActivityOptions::UserInitiatedAllowingIdleSystemSleep
            | NSActivityOptions::LatencyCritical,
        &NSString::from_str("ducktape-node follows 1s consensus blocks"),
    );
    std::mem::forget(token);
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
                     user-sign-gateway-route|gateway-route-bind|gateway-route-unbind|\
                     gateway-route-list|\
                     user-webauthn-challenge|user-p256-payload|\
                     init|invite|admit|\
                     invite-accept|promote|resident-remove|\
                     join-requests|member-remove|member-leave|member-status|join|\
                     preflight-state|upgrade-status — or \
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
fn gateway_can_start(
    sync_only: bool,
    gateway_listen: Option<&str>,
    http_listen: Option<&str>,
    wireguard_listen: Option<std::net::SocketAddr>,
    wireguard_effect: config::WireGuardEffectKind,
) -> bool {
    let api_is_loopback = http_listen
        .and_then(|address| address.parse::<std::net::SocketAddr>().ok())
        .is_some_and(|address| address.ip().is_loopback());
    // a configured gateway suppressed ONLY by a non-loopback app surface is a
    // silent degradation — say why, or the operator debugs a dead listener.
    if !sync_only && gateway_listen.is_some() && !api_is_loopback && http_listen.is_some() {
        eprintln!(
            "gateway disabled: http_listen {:?} is not loopback — the browser gateway only \
             starts when the node API binds a loopback address",
            http_listen.unwrap_or_default()
        );
    }
    !sync_only
        && gateway_listen.is_some()
        && api_is_loopback
        && wireguard_listen.is_some()
        && !matches!(wireguard_effect, config::WireGuardEffectKind::Fake)
}

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
        gateway_listen,
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
        sandbox,
        sandbox_capacity,
        promoted,
        joiner,
    } = boot::env::derive(resolved, sync_only);

    let gateway_enabled = gateway_can_start(
        sync_only,
        gateway_listen.as_deref(),
        http_listen.as_deref(),
        wireguard_listen,
        wireguard_effect,
    );

    let boot::surfaces::Surfaces {
        rpc_listener,
        http_cmds,
        stream_hub,
        index,
        voice_requests,
        code_stage_requests,
        blobs,
        agent_provisioner,
        gateway_requests,
        gateway_commands,
    } = boot::surfaces::bind(boot::surfaces::BindConfig {
        sync_only,
        label: &label,
        storage: &storage,
        rpc_listen,
        http_listen,
        gateway_listen,
        gateway_enabled,
        log_ring,
        // the forge worktree lane's committer identity (agent-dogfood M1):
        // every run commit is authored by this node's signer (D2 — the author
        // is the agent).
        forge_committer: config::hex_bytes(signer.public_key().as_ref()),
    })?;

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
    // 15s instead of commonware's 60s default: this read/write deadline is
    // the mesh's only half-open detector — see `constants::MESH_IO_TIMEOUT`.
    let rt_cfg = commonware_runtime::tokio::Config::default()
        .with_storage_directory(storage)
        .with_read_write_timeout(constants::MESH_IO_TIMEOUT);
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
            plane_monitor,
            plane_metrics,
            sync_monitor,
            sync_metrics,
            mesh_participants,
            status_public_key,
            sync_sources,
            sync_source,
            advertised_reach,
            network,
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
        // One process-wide bulk budget: the per-use planes retain separate
        // protocols, queues, sockets, and admission but cannot independently
        // saturate the same WireGuard link.
        let bulk_pacer = overlay_book::shared_bulk_pacer();
        // The `ducktape_dataplane_*` / `ducktape_statesync_serve_*` series
        // unregister when these handles drop — pin them to the whole node
        // future (both role arms await inside this block).
        let _plane_metrics = plane_metrics;
        let _sync_metrics = sync_metrics;

        if sync_only {
            boot::sync_only::run(
                context,
                &label,
                network,
                oracle,
                quota,
                &signer,
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
        // code-registry swaps realize through the blob plane: replay, catch-up,
        // and the live drain (which lifts this off the recovery sink) all fetch
        // committed component bytes from the node's content-addressed store.
        recovery.set_code_source(std::sync::Arc::new(host_state::BlobCodeSource(blobs.clone())));
        let manifest = match recovery.manifest() {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[node {label}] FATAL: recovery checkpoint is damaged: {e}");
                std::process::exit(1);
            }
        };
        // breadcrumb between the surface binds and the mesh/plane wiring: a
        // long journal replay is silent local disk io, and a boot log that
        // ends at "rpc listening" is otherwise indistinguishable from a hang.
        println!(
            "[node {label}] recovery store open (checkpoint: {})",
            if manifest.is_some() { "present" } else { "none" }
        );
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
                sandbox,
                sandbox_capacity,
                rpc_listener,
                http_cmds,
                gateway_requests,
                gateway_commands.clone(),
                &stream_hub,
                index,
                voice_requests,
                blobs,
                &agent_provisioner,
                &agent_dirs,
                overlay_slot,
                bulk_pacer.clone(),
                plane_monitor.clone(),
                storage_for_sync,
                recovery,
                &manifest,
                forge_repo,
                duckfs_dir,
            )
            .await;
        }
        validator::run_validator(
            context,
            network,
            oracle,
            quota,
            metrics,
            sync_source,
            advertised_reach,
            status_public_key,
            signer,
            label,
            namespace,
            identity_chain_id,
            peers,
            validators,
            coordinated,
            wireguard_listen,
            wireguard_effect,
            wireguard_key_file,
            primary_coordinator,
            wireguard_advertised,
            invite_listen,
            coordination,
            coord_cap,
            chain_id,
            mesh_state_file,
            checkpoint_blocks,
            promoted,
            dev_demo,
            announce_capabilities,
            sandbox,
            sandbox_capacity,
            rpc_listener,
            http_cmds,
            gateway_requests,
            gateway_commands,
            stream_hub,
            index,
            voice_requests,
            code_stage_requests,
            blobs,
            agent_provisioner,
            agent_dirs,
            overlay_slot,
            bulk_pacer,
            plane_monitor,
            sync_monitor,
            workspace,
            recovery,
            manifest,
            forge_repo,
            duckfs_dir,
        )
        .await;
    });

    Ok(())
}
