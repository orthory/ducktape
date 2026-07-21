//! Validator-role composition: recover application state, register every
//! pre-start mesh lane, resume the epoch engine, then hand ownership to the
//! consensus pump.

pub(crate) mod announce;
mod boot;
pub(crate) mod code_announce;
mod engine;
mod run;
mod wiring;

use commonware_cryptography::{Signer as _, ed25519};
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
    primary_coordinator: Option<String>,
    wireguard_advertised: Option<Ingress>,
    invite_listen: Option<std::net::SocketAddr>,
    coordination: crate::config::Coordination,
    coord_cap: Option<nat_traversal::CoordCap>,
    chain_id: String,
    mesh_state_file: std::path::PathBuf,
    checkpoint_blocks: u64,
    promoted: bool,
    dev_demo: bool,
    announce_capabilities: bool,
    sandbox: capability_host::SandboxBackend,
    sandbox_capacity: std::collections::BTreeMap<String, u64>,
    rpc_listener: Option<std::net::TcpListener>,
    http_cmds: futures::channel::mpsc::Receiver<noded::NodeCommand>,
    gateway_requests: Option<tokio::sync::mpsc::Receiver<noded::GatewayJob>>,
    gateway_commands: futures::channel::mpsc::Sender<noded::NodeCommand>,
    stream_hub: noded::StreamHub,
    index: std::sync::Arc<indexer::IndexStore>,
    voice_requests: tokio::sync::mpsc::Receiver<noded::RealtimeSessionRequest>,
    code_stage_requests: tokio::sync::mpsc::Receiver<noded::CodeStageRequest>,
    blobs: noded::blobs::BlobHandle,
    agent_provisioner: dispatch_oracle::SharedProvisioner,
    agent_dirs: capability_host::AgentDirs,
    overlay_slot: overlay_net::userspace::StackSlot,
    bulk_pacer: data_plane::BulkPacer,
    planes: data_plane::PlaneMonitor,
    sync_monitor: statesync::monitor::ServeMonitor,
    gateway_workspace: std::path::PathBuf,
    mut recovery: Recovery<commonware_runtime::tokio::Context>,
    manifest: Option<Manifest>,
    forge_repo: std::path::PathBuf,
    duckfs_dir: std::path::PathBuf,
) {
    metrics.set_role_phase(noded::NodeRole::Validator, noded::NodePhase::Recovering);
    tracing::info!(
        event = "node_phase_transition",
        role = "validator",
        phase = "recovering",
        node = %label
    );
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
        relay_tx,
        relay_rx,
        media_peers,
        reach_cmd,
        gate_fwd_rx,
        gate_fwd_keepalive,
        gate_outcomes,
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
        primary_coordinator,
        wireguard_advertised,
        invite_listen,
        coord_cap,
        voice_requests,
        overlay_slot.clone(),
        planes.clone(),
    )
    .await;
    let (
        sync_tx,
        sync_rx,
        mut recovery,
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
        validators.clone(),
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
        gateway_book,
        blob_peers,
        blob_client,
        sync_state_rx,
        sync_lease,
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
        bulk_pacer.clone(),
        planes.clone(),
        sync_monitor,
        gateway_requests,
        gateway_commands,
        gateway_workspace,
        blobs.clone(),
        initial_member_keys,
        initial_resident_keys,
        mesh_oracle,
        bank_base,
        channel_bank,
        sync_tx,
        sync_rx,
        relay_rx,
    )
    .await;

    if let Some(peers) = &media_peers {
        let me: [u8; 32] = signer
            .public_key()
            .as_ref()
            .try_into()
            .expect("ed25519 keys are 32 bytes");
        crate::agent_plane::spawn(
            label.clone(),
            crate::overlay_book::socket_factory(wireguard_effect, &overlay_slot),
            std::sync::Arc::clone(peers),
            me,
            bulk_pacer.clone(),
            planes.clone(),
            stream_hub.run_output(),
        );
        // the terminal-session plane: forwards a session's output ring and
        // ordered command log to peers, so a member on another node streams it.
        crate::term_plane::spawn(
            label.clone(),
            crate::overlay_book::socket_factory(wireguard_effect, &overlay_slot),
            std::sync::Arc::clone(peers),
            me,
            bulk_pacer.clone(),
            planes.clone(),
            stream_hub.terminals(),
            stream_hub.term_commands(),
        );
        // the module-code plane: serves push/pull transfers and drains the
        // admin RPC's stage fan-outs. same overlay book as the agent plane.
        crate::code_plane::spawn(
            label.clone(),
            crate::overlay_book::socket_factory(wireguard_effect, &overlay_slot),
            std::sync::Arc::clone(peers),
            me,
            bulk_pacer,
            planes.clone(),
            blobs.clone(),
            code_stage_requests,
        );
    }

    let mut epoch_spawner = engine::EpochSpawner::new(
        &context,
        oracle,
        signer.clone(),
        namespace.clone(),
        label.clone(),
        bank_base,
        channel_bank,
    );
    // with the serve lane wired, realize code-registry swaps through the
    // FETCHING source for the rest of this validator's life: a committed
    // component the local store lacks is pulled from peers (ranged, verified)
    // before a boundary can fail closed on it.
    recovery.set_code_source(std::sync::Arc::new(crate::blob_fetch::FetchingCodeSource::new(
        blobs.clone(),
        blob_client.clone(),
        crate::constants::MAX_MODULE_CODE_BYTES,
        crate::constants::BLOB_FETCH_ATTEMPTS,
    )));

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
    metrics.set_role_phase(noded::NodeRole::Validator, noded::NodePhase::Validating);
    tracing::info!(
        event = "node_phase_transition",
        role = "validator",
        phase = "validating",
        node = %label,
        epoch = orchestrator.epoch()
    );
    run::run(run::ValidatorLoopState {
        context: &context,
        node,
        orchestrator,
        epoch_spawner,
        last_cert_height,
        latest_floor,
        mesh_oracle,
        gateway_book,
        media_peers,
        blob_peers,
        blob_client,
        reach_cmd,
        relay_tx,
        sync_state_rx,
        gate_fwd_rx,
        gate_fwd_keepalive,
        gate_outcomes,
        relay_ingress,
        next_seq,
        prev_ckpt,
        signer,
        label,
        peers,
        validators,
        dev_demo,
        checkpoint_blocks,
        sync_lease,
        announce_capabilities,
        sandbox,
        sandbox_capacity,
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
    })
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

/// the mesh-carrier REAL arm: one epoch's pre-registered discovery channels
/// (a [`ChannelBank`] slot) + the [`discovery::Oracle`] the resolver keys on.
/// This is the `authenticated::discovery` network's per-spawn transport bundle —
/// the discovery `Network` (`MeshHead`) registers the channels into the bank
/// before start, and this bundles one slot with the oracle at the point
/// [`engine`](self::engine) consumes it, feeding
/// [`consensus::SimplexOrderer::spawn_with_carrier`] the identical values the
/// loose-channel spawn took before the seam.
pub(super) struct DiscoveryMesh {
    vote: Option<MeshChannel>,
    certificate: Option<MeshChannel>,
    resolver: Option<MeshChannel>,
    payload: Option<MeshChannel>,
    fetch: Option<MeshChannel>,
    oracle: discovery::Oracle<ed25519::PublicKey>,
}

impl DiscoveryMesh {
    pub(super) fn new(
        slot: EpochChannels,
        oracle: discovery::Oracle<ed25519::PublicKey>,
    ) -> Self {
        let (vote, certificate, resolver, payload, fetch) = slot;
        Self {
            vote: Some(vote),
            certificate: Some(certificate),
            resolver: Some(resolver),
            payload: Some(payload),
            fetch: Some(fetch),
            oracle,
        }
    }
}

impl consensus::MeshCarrier for DiscoveryMesh {
    type Sender = MeshSender;
    type Receiver = MeshReceiver;
    type Provider = discovery::Oracle<ed25519::PublicKey>;
    type Blocker = discovery::Oracle<ed25519::PublicKey>;

    fn vote(&mut self) -> MeshChannel {
        self.vote.take().expect("vote channel taken once")
    }
    fn certificate(&mut self) -> MeshChannel {
        self.certificate.take().expect("certificate channel taken once")
    }
    fn resolver(&mut self) -> MeshChannel {
        self.resolver.take().expect("resolver channel taken once")
    }
    fn payload(&mut self) -> MeshChannel {
        self.payload.take().expect("payload channel taken once")
    }
    fn fetch(&mut self) -> MeshChannel {
        self.fetch.take().expect("fetch channel taken once")
    }
    fn provider(&self) -> discovery::Oracle<ed25519::PublicKey> {
        self.oracle.clone()
    }
    fn blocker(&self) -> discovery::Oracle<ed25519::PublicKey> {
        self.oracle.clone()
    }
}
