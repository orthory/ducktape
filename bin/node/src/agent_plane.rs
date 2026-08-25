//! Live agent output fan-out over the WireGuard data plane.
//!
//! Final run state, leases, and usage stay on consensus. This plane carries
//! only the bounded live tail already exposed by `RunOutputRegistry`, so a
//! slow or malicious observer can stall only its own per-peer stream task.

use std::collections::{HashMap, HashSet};
use std::io;
use std::sync::Arc;
use std::time::Duration;

use data_plane::{
    BulkPacer, DataPlane, DataPlaneTransport, FlowId, PeerId, Service, SocketFactory, StreamPacing,
    StreamPlaneSpec, StreamPolicy, StreamService, bind_stream_plane,
};
use noded::{RunOutputEvent, RunOutputRegistry};
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};

use crate::overlay_book::{BIND_RETRY, OverlayBook, OverlayPeers, Plane, StreamPlane};

const RUN_OUTPUT_INTENT: u8 = 1;
const MAX_EVENT_BYTES: usize = 64 * 1024;
/// the outbound re-dial cadence: how long a peer's fan-out task waits after
/// a failed open, and how often the fan-out re-reads the tracked set.
const DIAL_RETRY: Duration = Duration::from_secs(3);

fn run_output_flow() -> FlowId {
    FlowId::derive(b"ducktape:agent-run-output:v1")
}

/// the agent-telemetry plane's tag for the shared [`OverlayBook`]:
/// default-deny admission scoped to the service + run-output flow.
struct AgentPlane;

impl Plane for AgentPlane {
    const SERVICE: Service = Service::AgentTelemetry;
}

impl StreamPlane for AgentPlane {
    fn flow() -> FlowId {
        run_output_flow()
    }
}

/// Bind the service in the background. Each member gets one persistent
/// outbound stream; inbound lines enter the same node-local registry the
/// existing websocket topic already tails.
pub(crate) fn spawn(
    label: String,
    factory: Arc<dyn SocketFactory>,
    peers: Arc<OverlayPeers>,
    me: [u8; 32],
    pacer: BulkPacer,
    planes: data_plane::PlaneMonitor,
    registry: RunOutputRegistry,
) {
    tokio::spawn(async move {
        let own = peers.own_ip(&me);
        let spec = StreamPlaneSpec {
            own_ip: own,
            service: Service::AgentTelemetry,
            pacing: StreamPacing::Shared(pacer),
            policy: StreamPolicy { accept_backlog: 64 },
            retry: BIND_RETRY,
        };
        let book = OverlayBook::<AgentPlane>::new(Arc::clone(&peers));
        let (plane, service) = match bind_stream_plane(spec, factory, book).await {
            Ok(bound) => bound,
            Err(error) => {
                tracing::error!(
                    target: "ducktape::dataplane",
                    node = %label,
                    service = "agent_telemetry",
                    error = %error,
                    "agent telemetry plane register failed"
                );
                return;
            }
        };
        tracing::info!(
            target: "ducktape::dataplane",
            node = %label,
            service = "agent_telemetry",
            own = %own,
            "agent telemetry plane: overlay stream bound"
        );
        planes.register("agent", Service::AgentTelemetry, plane.watch());
        run_bound(plane, service, peers, PeerId(me), registry).await;
    });
}

async fn run_bound<T: DataPlaneTransport>(
    plane: DataPlane<T>,
    service: Arc<StreamService<T>>,
    peers: Arc<OverlayPeers>,
    me: PeerId,
    registry: RunOutputRegistry,
) {
    let _plane = plane;
    tokio::select! {
        _ = accept_loop(Arc::clone(&service), Arc::clone(&peers), registry.clone()) => {}
        _ = fanout_loop(service, peers, me, registry) => {}
    }
}

async fn accept_loop<T: DataPlaneTransport>(
    service: Arc<StreamService<T>>,
    peers: Arc<OverlayPeers>,
    registry: RunOutputRegistry,
) {
    let active = Arc::new(std::sync::Mutex::new(HashSet::new()));
    while let Some((peer, hello, stream)) = service.accept().await {
        if hello.intent != RUN_OUTPUT_INTENT || !hello.meta.is_empty() {
            continue;
        }
        if !active.lock().expect("agent streams lock").insert(peer) {
            continue;
        }
        let peers = Arc::clone(&peers);
        let registry = registry.clone();
        let active = Arc::clone(&active);
        tokio::spawn(async move {
            let _ = receive_peer(stream, peer, peers, registry).await;
            active.lock().expect("agent streams lock").remove(&peer);
        });
    }
}

async fn receive_peer<S: AsyncRead + Unpin>(
    mut stream: S,
    peer: PeerId,
    peers: Arc<OverlayPeers>,
    registry: RunOutputRegistry,
) -> io::Result<()> {
    while peers.contains(peer) {
        let Some(mut event) = read_event(&mut stream).await? else {
            return Ok(());
        };
        if !peers.contains(peer) {
            return Ok(());
        }
        // The transport-authenticated source stays visible. Live output is
        // observational and must never masquerade as consensus-authored text.
        event.line = format!(
            "[node {}] {}",
            crate::config::hex_bytes(&peer.0[..4]),
            event.line
        );
        registry.append_remote(event);
    }
    Ok(())
}

async fn fanout_loop<T: DataPlaneTransport>(
    service: Arc<StreamService<T>>,
    peers: Arc<OverlayPeers>,
    me: PeerId,
    registry: RunOutputRegistry,
) {
    let mut tasks: HashMap<PeerId, tokio::task::JoinHandle<()>> = HashMap::new();
    loop {
        tasks.retain(|peer, task| {
            let keep = peers.contains(*peer) && !task.is_finished();
            if !keep {
                task.abort();
            }
            keep
        });
        for peer in peers.peer_ids().into_iter().filter(|peer| *peer != me) {
            tasks.entry(peer).or_insert_with(|| {
                tokio::spawn(send_peer(
                    Arc::clone(&service),
                    Arc::clone(&peers),
                    peer,
                    registry.clone(),
                ))
            });
        }
        tokio::time::sleep(DIAL_RETRY).await;
    }
}

async fn send_peer<T: DataPlaneTransport>(
    service: Arc<StreamService<T>>,
    peers: Arc<OverlayPeers>,
    peer: PeerId,
    registry: RunOutputRegistry,
) {
    let mut appends = registry.subscribe_appends();
    while peers.contains(peer) {
        let mut stream = match service
            .open(peer, run_output_flow(), RUN_OUTPUT_INTENT, Vec::new())
            .await
        {
            Ok(stream) => stream,
            Err(_) => {
                tokio::time::sleep(DIAL_RETRY).await;
                continue;
            }
        };
        loop {
            tokio::select! {
                event = appends.recv() => match event {
                    Ok(event) => {
                        if write_event(&mut stream, &event).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
                },
                _ = tokio::time::sleep(DIAL_RETRY) => {
                    if !peers.contains(peer) {
                        return;
                    }
                }
            }
        }
    }
}

fn valid_event(event: &RunOutputEvent) -> bool {
    event.id.len() == 64 && event.id.bytes().all(|byte| byte.is_ascii_hexdigit())
}

async fn write_event<S: AsyncWrite + Unpin>(
    stream: &mut S,
    event: &RunOutputEvent,
) -> io::Result<()> {
    if !valid_event(event) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid run id",
        ));
    }
    let mut payload = serde_json::to_vec(event).map_err(io::Error::other)?;
    if payload.len() > MAX_EVENT_BYTES {
        // ponytail: giant provider JSON lines are not useful live telemetry;
        // move full-fidelity traces to blob refs if operators ever need them.
        let mut clipped = event.clone();
        clipped.line = format!("[{} byte output line omitted]", event.line.len());
        payload = serde_json::to_vec(&clipped).map_err(io::Error::other)?;
    }
    if payload.len() > MAX_EVENT_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "run output event too large",
        ));
    }
    stream
        .write_all(&(payload.len() as u32).to_be_bytes())
        .await?;
    stream.write_all(&payload).await
}

async fn read_event<S: AsyncRead + Unpin>(stream: &mut S) -> io::Result<Option<RunOutputEvent>> {
    let mut len = [0u8; 4];
    match stream.read_exact(&mut len).await {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error),
    }
    let len = u32::from_be_bytes(len) as usize;
    if len == 0 || len > MAX_EVENT_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid run output frame length",
        ));
    }
    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload).await?;
    let event: RunOutputEvent = serde_json::from_slice(&payload)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    valid_event(&event)
        .then_some(event)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid run id"))
        .map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use commonware_cryptography::{Signer as _, ed25519};
    use data_plane::PlaneConfig;
    use data_plane::sim::{LinkModel, SimNet};

    #[tokio::test(start_paused = true)]
    async fn live_output_crosses_nodes_once_and_keeps_its_source() {
        let key_a = ed25519::PrivateKey::from_seed(1).public_key();
        let key_b = ed25519::PrivateKey::from_seed(2).public_key();
        let raw_a: [u8; 32] = key_a.as_ref().try_into().unwrap();
        let raw_b: [u8; 32] = key_b.as_ref().try_into().unwrap();
        let a = PeerId(raw_a);
        let b = PeerId(raw_b);

        let peers = OverlayPeers::new("agent-plane-test".into());
        peers.set_peers([&key_a, &key_b].into_iter());
        let net = SimNet::new();
        net.set_link(
            a,
            b,
            LinkModel {
                latency: Duration::from_millis(1),
                bytes_per_sec: 10_000_000,
                drop_every: None,
                delay_every: None,
            },
        );
        let config = PlaneConfig {
            bulk_bytes_per_sec: 10_000_000,
            bulk_burst_bytes: 64 * 1024,
        };
        let plane_a = DataPlane::new(
            net.endpoint(a),
            OverlayBook::<AgentPlane>::new(Arc::clone(&peers)),
            config,
        );
        let plane_b = DataPlane::new(
            net.endpoint(b),
            OverlayBook::<AgentPlane>::new(Arc::clone(&peers)),
            config,
        );
        let service_a = Arc::new(
            plane_a
                .stream_service(Service::AgentTelemetry, StreamPolicy { accept_backlog: 4 })
                .unwrap(),
        );
        let service_b = Arc::new(
            plane_b
                .stream_service(Service::AgentTelemetry, StreamPolicy { accept_backlog: 4 })
                .unwrap(),
        );
        let registry_a = RunOutputRegistry::default();
        let registry_b = RunOutputRegistry::default();
        let mut changed = registry_b.subscribe();
        let mut rebroadcast = registry_b.subscribe_appends();
        let receive = tokio::spawn(accept_loop(
            service_b,
            Arc::clone(&peers),
            registry_b.clone(),
        ));
        let send = tokio::spawn(send_peer(
            service_a,
            Arc::clone(&peers),
            b,
            registry_a.clone(),
        ));

        tokio::task::yield_now().await;
        registry_a.append("ab".repeat(32), noded::RunStream::Stdout, "working");
        tokio::time::timeout(Duration::from_secs(2), changed.changed())
            .await
            .expect("remote registry receives the line")
            .expect("remote registry stays live");
        let (rows, _) = registry_b.read_after(&"ab".repeat(32), 0, 10);
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].2,
            format!("[node {}] working", crate::config::hex_bytes(&raw_a[..4]))
        );
        assert!(matches!(
            rebroadcast.try_recv(),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        ));

        send.abort();
        receive.abort();
        drop((plane_a, plane_b));
    }
}
