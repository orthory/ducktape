use std::time::Duration;

use commonware_cryptography::ed25519;
use commonware_p2p::authenticated::discovery::{self, Network};
use commonware_p2p::{Manager, Receiver as P2pReceiver};
use commonware_runtime::{Clock, Quota, Spawner, Supervisor};
use commonware_utils::ordered::Set;
use statesync::fetch_manifest;
use statesync::p2p::P2pSyncClient;

use crate::constants::*;
use crate::host_state::{NetworkBindings, SyncSubstrates, sync_all_modules};
use crate::util::hex;

/// `run_node`'s terminal `--sync-only` branch (phase P4): registers every
/// channel a mesh member must answer (black-holing everything a joiner with
/// no engine and no votes does not itself consume), starts the mesh, pulls
/// the served manifest, runs boot preflight, and rebuilds every module once
/// via [`sync_all_modules`] before the process is done. Never returns to a
/// validator path — the caller `return`s right after this call.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run(
    context: commonware_runtime::tokio::Context,
    label: &str,
    mut network: Network<
        overlay_net::OverlayContext<commonware_runtime::tokio::Context>,
        ed25519::PrivateKey,
    >,
    mut oracle: discovery::Oracle<ed25519::PublicKey>,
    quota: Quota,
    mesh_participants: Set<ed25519::PublicKey>,
    sync_sources: Vec<ed25519::PublicKey>,
    storage_for_sync: std::path::PathBuf,
    namespace: Vec<u8>,
    identity_chain_id: String,
    blobs: noded::blobs::BlobHandle,
    voice_requests: tokio::sync::mpsc::Receiver<noded::CallSessionRequest>,
) {
    // no consensus coordinates yet: track the genesis mesh at the
    // base index. validators ignore this index if they have rotated
    // past keeping it; connection authorization is the UNION of every
    // tracked set on each side, so the descriptor's members stay
    // reachable.
    oracle.track(PEER_SET, mesh_participants.clone());
    // ---- the SYNC-ONLY joiner: no engine, no votes — just the wire ----
    //
    // validators broadcast consensus traffic (votes, certificates,
    // payload gossip) to EVERY tracked mesh peer, not only to fellow
    // participants — and a message on an UNREGISTERED channel is a
    // protocol violation that makes the peer actor kill the connection
    // (a permanent connect/kill loop that drops every rpc). so a
    // mesh-member-but-not-validator must register every channel and
    // black-hole the consensus lanes it does not consume.
    for epoch in 0..EPOCH_CHANNEL_BANK {
        let (vote, cert, res, payload, fetch) = engine_channels(epoch);
        for ch in [vote, cert, res, payload, fetch] {
            let (_tx, mut rx) = network.register(ch, quota, MAX_BACKLOG);
            let label: &'static str = Box::leak(format!("blackhole_{ch}").into_boxed_str());
            context.child(label).spawn(move |_ctx| async move {
                while rx.recv().await.is_ok() {}
            });
        }
    }
    let (sync_tx, sync_rx) = network.register(CHANNEL_STATE_SYNC, quota, MAX_BACKLOG);
    // the submit-relay lane: a sync-only resident holds no standing,
    // relays no writes, and answers nothing — but an unregistered
    // channel kills the sender, so black-hole.
    {
        let (_tx, mut rx) = network.register(CHANNEL_SUBMIT_RELAY, quota, MAX_BACKLOG);
        context
            .child("blackhole_submit_relay")
            .spawn(move |_ctx| async move { while rx.recv().await.is_ok() {} });
    }
    // the lobby lane: a sync-only resident never announces or answers,
    // but an unregistered channel is a protocol violation — black-hole.
    {
        let (_tx, mut rx) = network.register(CHANNEL_LOBBY, quota, MAX_BACKLOG);
        context.child("blackhole_lobby").spawn(move |_ctx| async move {
            while rx.recv().await.is_ok() {}
        });
    }
    // the reachability lane: a sync-only resident runs no WireGuard
    // plane, but the channel must exist — black-hole.
    {
        let (_tx, mut rx) = network.register(CHANNEL_REACHABILITY, quota, MAX_BACKLOG);
        context
            .child("blackhole_reachability")
            .spawn(move |_ctx| async move { while rx.recv().await.is_ok() {} });
    }
    // the voice lane: a sync-only resident serves no huddle audio,
    // but the channel must exist — black-hole. dropping the session
    // lane makes /v1/call/ws refuse instead of hang (this branch
    // never reaches the validator hub below).
    drop(voice_requests);
    {
        let (_tx, mut rx) = network.register(CHANNEL_VOICE, quota, MAX_BACKLOG);
        context
            .child("blackhole_voice")
            .spawn(move |_ctx| async move { while rx.recv().await.is_ok() {} });
    }
    // the video lane: a sync-only resident serves no huddle video, but
    // the channel must exist — black-hole.
    {
        let (_tx, mut rx) = network.register(CHANNEL_VIDEO, quota, MAX_BACKLOG);
        context
            .child("blackhole_video")
            .spawn(move |_ctx| async move { while rx.recv().await.is_ok() {} });
    }
    network.start();

    if sync_sources.is_empty() {
        eprintln!(
            "[node {label}] no statesync source: no validator other than this node \
             is available to serve (only validators answer the statesync channel)"
        );
        std::process::exit(1);
    }
    // rotate across every validator that can serve — the payloads
    // verify against consensus roots, so source choice is pure
    // availability.
    let client = P2pSyncClient::with_sources(
        context.child("sync_client"),
        sync_tx,
        sync_rx,
        sync_sources.clone(),
    );

    // the mesh takes a moment to connect, and the server only serves
    // once it has a finalized boundary — retry until the manifest lands.
    let manifest = loop {
        match fetch_manifest(&client).await {
            Ok(m) => break m,
            Err(e) => {
                println!("[node {label}] manifest not ready ({e}); retrying");
                context.sleep(Duration::from_millis(500)).await;
            }
        }
    };
    println!(
        "[node {label}] manifest height={} app_hash={}",
        manifest.height,
        hex(&manifest.app_hash)
    );

    // BOOT PREFLIGHT (design §5 / plan Task 7.3): refuse an under-versioned
    // binary against the SERVED boundary before installing/composing, so a
    // too-old joiner fails with a clear "install the newer binary" message
    // rather than an opaque post-compose app-hash mismatch. the served
    // `required_min_version` is an unauthenticated hint (untrusted-server
    // model): a lying value can at worst refuse-to-boot this joiner, never
    // fork. inert on a baseline manifest.
    if let Err(e) = manifest.preflight(MAX_PROTOCOL_VERSION) {
        eprintln!("[node {label}] SYNC REFUSED: {e}");
        std::process::exit(1);
    }

    // rebuild EVERY module in the manifest (a REAL joiner owns its
    // disk, so every store opens under its canonical module id) and
    // print the greppable line the demo script asserts on.
    let forge_repo = storage_for_sync.join("forge-repo");
    let duckfs_dir = storage_for_sync.join("duckfs");
    match sync_all_modules(
        &context,
        &client,
        &manifest,
        NetworkBindings {
            invite: &namespace,
            identity_chain_id: &identity_chain_id,
        },
        SyncSubstrates {
            forge_repo: &forge_repo,
            duckfs_dir: &duckfs_dir,
            blobs: blobs.clone(),
        },
        0,
    )
    .await
    {
        Ok(host) => {
            println!("[node {label}] synced app_hash={}", hex(&host.app_hash()));
        }
        Err(e) => {
            eprintln!("[node {label}] SYNC FAILED: {e}");
            std::process::exit(1);
        }
    }
}
