//! Validator-role composition: recover application state, register every
//! pre-start mesh lane, resume the epoch engine, then hand ownership to the
//! consensus pump.

pub(crate) mod announce;
mod boot;
mod engine;
mod run;
mod wiring;

use commonware_cryptography::ed25519;
use commonware_p2p::Ingress;
use commonware_p2p::authenticated::discovery::{self, Network};
use commonware_runtime::Quota;
use recovery::{Manifest, Recovery};

use crate::explorer::IndexFold;

/// Run the validator role after the shared boot conductor has selected it.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_validator(
    context: commonware_runtime::tokio::Context,
    network: Network<OverlayCtx, ed25519::PrivateKey>,
    oracle: discovery::Oracle<ed25519::PublicKey>,
    quota: Quota,
    metrics: noded::NodeMetrics,
    sync_source: Option<ed25519::PublicKey>,
    advertised_reach: Ingress,
    status_public_key: String,
    signer: ed25519::PrivateKey,
    label: String,
    namespace: Vec<u8>,
    identity_chain_id: String,
    peers: Vec<ed25519::PublicKey>,
    validators: Vec<ed25519::PublicKey>,
    coordinated: Vec<(ed25519::PublicKey, Ingress, ed25519::PublicKey)>,
    wireguard_listen: Option<std::net::SocketAddr>,
    wireguard_effect: crate::config::WireGuardEffectKind,
    wireguard_key_file: std::path::PathBuf,
    invite_listen: Option<std::net::SocketAddr>,
    coordination: crate::config::Coordination,
    coord_cap: Option<nat_traversal::CoordCap>,
    chain_id: String,
    mesh_state_file: std::path::PathBuf,
    checkpoint_blocks: u64,
    promoted: bool,
    dev_demo: bool,
    announce_capabilities: bool,
    rpc_listener: Option<std::net::TcpListener>,
    http_cmds: futures::channel::mpsc::Receiver<noded::NodeCommand>,
    stream_hub: noded::StreamHub,
    index: std::sync::Arc<indexer::IndexStore>,
    voice_requests: tokio::sync::mpsc::Receiver<noded::CallSessionRequest>,
    blobs: noded::blobs::BlobHandle,
    agent_provisioner: Option<dispatch_oracle::SharedProvisioner>,
    agent_dirs: capability_host::AgentDirs,
    overlay_slot: overlay_net::userspace::StackSlot,
    mut recovery: Recovery<commonware_runtime::tokio::Context>,
    manifest: Option<Manifest>,
    forge_repo: std::path::PathBuf,
    duckfs_dir: std::path::PathBuf,
) {
    // (host, recovered-state, next local submit seq, last checkpoint
    // ONE index fold for the whole boot (journal replay + post-reboot
    // catch-up + post-sync refreshes): its stop flag must persist across
    // phases — a later phase folding past a gap an earlier phase detected
    // would advance watermarks over the hole and hide it from the final
    // heal below.
    let mut boot_fold = IndexFold::new(&index, blobs.clone());
    let (host, resumed, next_seq, prev_ckpt, recovery_manifest_for_resume) = boot::restore(
        &context,
        &index,
        blobs.clone(),
        &mut recovery,
        manifest,
        &forge_repo,
        &duckfs_dir,
        &validators,
        &namespace,
        &identity_chain_id,
        &signer,
        &label,
        &mut boot_fold,
    )
    .await;

    let wiring::PreWiring {
        initial_member_keys,
        initial_resident_keys,
        mesh_oracle,
        bank_base,
        channel_bank,
        sync_tx,
        sync_rx,
        lobby_tx,
        lobby_rx,
        relay_tx,
        relay_rx,
        media_peers,
        reach_cmd,
    } = wiring::wire(
        &context,
        network,
        &oracle,
        quota,
        &host,
        resumed.as_ref(),
        validators.clone(),
        signer.clone(),
        peers.clone(),
        namespace.clone(),
        label.clone(),
        coordinated,
        wireguard_listen,
        wireguard_effect,
        wireguard_key_file,
        chain_id,
        mesh_state_file,
        advertised_reach,
        invite_listen,
        coord_cap,
        voice_requests,
        overlay_slot.clone(),
    )
    .await;
    let (
        sync_tx,
        sync_rx,
        recovery,
        host,
        resumed,
        next_seq,
        prev_ckpt,
        recovery_manifest_for_resume,
        boot_fold,
    ) = boot::post_reboot_catchup(
        &context,
        promoted,
        sync_source,
        sync_tx,
        sync_rx,
        recovery,
        host,
        resumed,
        next_seq,
        prev_ckpt,
        recovery_manifest_for_resume,
        boot_fold,
        signer.clone(),
        label.clone(),
        namespace.clone(),
        identity_chain_id.clone(),
        peers.clone(),
        validators.clone(),
        wireguard_listen,
        wireguard_effect,
        overlay_slot.clone(),
        forge_repo.clone(),
        duckfs_dir.clone(),
        blobs.clone(),
    )
    .await;

    let wiring::RuntimeWiring {
        member_keys,
        participants,
        resume_epoch,
        pending_boot,
        bank_base,
        mesh_oracle,
        channel_bank,
        sync_plane_book,
        blob_peers,
        blob_fetcher,
        sync_state_rx,
        lobby_ingress,
        relay_ingress,
    } = wiring::finish(
        &context,
        &index,
        &host,
        resumed.as_ref(),
        recovery_manifest_for_resume.as_ref(),
        boot_fold,
        &validators,
        signer.clone(),
        label.clone(),
        peers.clone(),
        namespace.clone(),
        wireguard_effect,
        overlay_slot.clone(),
        blobs.clone(),
        initial_member_keys,
        initial_resident_keys,
        mesh_oracle,
        bank_base,
        channel_bank,
        sync_tx,
        sync_rx,
        lobby_rx,
        relay_rx,
    )
    .await;

    let mut epoch_spawner = engine::EpochSpawner::new(
        &context,
        oracle,
        signer.clone(),
        namespace.clone(),
        label.clone(),
        bank_base,
        channel_bank,
    );
    let engine::EngineState {
        node,
        orchestrator,
        last_cert_height,
        latest_floor,
    } = engine::resume(
        &mut epoch_spawner,
        host,
        recovery,
        resumed.as_ref(),
        &member_keys,
        &participants,
        resume_epoch,
        pending_boot,
        &signer,
        &label,
        dev_demo,
    )
    .await;
    run::run_loop(
        &context,
        node,
        orchestrator,
        epoch_spawner,
        last_cert_height,
        latest_floor,
        mesh_oracle,
        sync_plane_book,
        media_peers,
        blob_peers,
        blob_fetcher,
        reach_cmd,
        lobby_tx,
        relay_tx,
        sync_state_rx,
        lobby_ingress,
        relay_ingress,
        next_seq,
        prev_ckpt,
        signer,
        label,
        namespace,
        peers,
        validators,
        dev_demo,
        checkpoint_blocks,
        announce_capabilities,
        rpc_listener,
        http_cmds,
        stream_hub,
        index,
        blobs,
        agent_provisioner,
        agent_dirs,
        metrics,
        status_public_key,
        coordination,
    )
    .await;
}

type OverlayCtx = overlay_net::OverlayContext<commonware_runtime::tokio::Context>;
type MeshSender = commonware_p2p::authenticated::discovery::Sender<
    commonware_cryptography::ed25519::PublicKey,
    OverlayCtx,
>;
type MeshReceiver =
    commonware_p2p::authenticated::discovery::Receiver<commonware_cryptography::ed25519::PublicKey>;
type MeshChannel = (MeshSender, MeshReceiver);
type EpochChannels = (
    MeshChannel,
    MeshChannel,
    MeshChannel,
    MeshChannel,
    MeshChannel,
);
type ChannelBank = Vec<Option<EpochChannels>>;
