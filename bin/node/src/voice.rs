//! The node's live-voice runtime: the bridge between huddle websockets
//! (`noded`'s `/v1/voice/ws`), the chat voice engine, and the p2p mesh.
//!
//! Runtime shape mirrors the reachability plane's split exactly: the hub runs
//! on its OWN plain-tokio OS thread (the engine's pump and the 20 ms playout
//! tick are tokio-native), and mesh I/O crosses to the commonware runner over
//! two channel pumps `main.rs` owns on the dedicated `CHANNEL_VOICE` lane.
//!
//! Three pieces, all off-consensus:
//! - [`ChannelTransport`] — a datagram-only [`DataPlaneTransport`] arm over
//!   the pump channels. The designed transport is UDP on the reachability
//!   plane's WireGuard overlay; riding the authenticated TCP mesh instead
//!   trades head-of-line latency (absorbed by the engine's jitter buffer) for
//!   zero new infrastructure, and swaps out later behind the same trait
//!   without touching the engine.
//! - An [`AdmissionPolicy`] over the node's ACTIVE sessions: this node
//!   receives (and emits) voice only for flows its own operator has a live
//!   huddle websocket on. The mesh already authenticates every peer as a
//!   workspace member; roster-level gating is the client's job (it steers the
//!   fan-out from consensus state), and unadmitted traffic drops counted at
//!   the plane per its default-deny contract.
//! - The hub loop — drains [`noded::VoiceSessionRequest`]s from the app
//!   surface and runs AT MOST ONE session at a time (Slack semantics: you are
//!   in one huddle). A session owns a [`VoiceEngine`] on the channel-derived
//!   flow and pumps: websocket pcm in → encode + fan-out; a 20 ms tick →
//!   mixed playout → websocket out. Dropping either websocket end tears the
//!   session down; a new request replaces the current session.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chat::voice::{FRAME_MILLIS, FRAME_SAMPLES, VoiceConfig, VoiceEngine};
use data_plane::{
    AdmissionPolicy, DataPlane, DataPlaneTransport, DatagramPolicy, FlowId, PeerId, PlaneConfig,
    Service, TransportError,
};
use tokio::sync::{mpsc, watch};

/// One voice datagram crossing the mesh pumps: (raw ed25519 peer key, frame).
/// Outbound the key names the recipient; inbound it is the authenticated
/// sender the mesh reports.
pub type VoiceDatagram = ([u8; 32], Vec<u8>);

/// Mesh-pump lane depth: ~5 s of one speaker's frames. Voice is
/// fire-and-forget, so overflow drops rather than backpressures.
const WIRE_LANE: usize = 256;
/// Inbound datagram queue per flow: ~2.5 s of one speaker's frames. Overflow
/// drops the oldest inside the flow (the plane's drop-oldest contract).
const FLOW_QUEUE: usize = 128;
/// Webview↔hub pcm lanes: a small cushion (8 × 20 ms); late audio is dead
/// audio, so both sides drop rather than backpressure when it fills.
const PCM_LANE: usize = 8;

/// derive the voice flow for a chat channel — the exact domain string both
/// ends agree on (every participant derives it from the same channel id).
fn channel_flow(channel_id: &str) -> FlowId {
    FlowId::derive(format!("voice-channel:{channel_id}").as_bytes())
}

/// Stand up the voice runtime on its own OS thread and return the mesh ends:
/// `main.rs` drains the outbound receiver into the `CHANNEL_VOICE` sender and
/// feeds mesh receipts into the inbound sender. `requests` is the app
/// surface's session lane ([`noded::NodeHandle::with_voice`]).
pub fn spawn_hub(
    requests: mpsc::Receiver<noded::VoiceSessionRequest>,
) -> (mpsc::Receiver<VoiceDatagram>, mpsc::Sender<VoiceDatagram>) {
    let (outbound_tx, outbound_rx) = mpsc::channel(WIRE_LANE);
    let (inbound_tx, inbound_rx) = mpsc::channel(WIRE_LANE);
    std::thread::Builder::new()
        .name("voice-hub".into())
        .spawn(move || {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("voice-hub tokio runtime")
                .block_on(hub_loop(requests, outbound_tx, inbound_rx));
        })
        .expect("spawn voice-hub thread");
    (outbound_rx, inbound_tx)
}

/// The datagram-only transport arm over the mesh pump channels. Voice never
/// opens streams, so the stream half reports [`TransportError::Closed`] — the
/// plane's acceptor loop exits immediately and `connect` refuses.
struct ChannelTransport {
    outbound: mpsc::Sender<VoiceDatagram>,
    inbound: tokio::sync::Mutex<mpsc::Receiver<VoiceDatagram>>,
}

impl DataPlaneTransport for ChannelTransport {
    type Stream = tokio::io::DuplexStream;

    fn max_datagram(&self) -> usize {
        data_plane::MAX_DATAGRAM
    }

    async fn send_datagram(&self, to: PeerId, frame: Vec<u8>) -> Result<(), TransportError> {
        // fire-and-forget per the trait: a full pump lane drops the frame —
        // voice retries nothing, the jitter buffer renders the gap.
        let _ = self.outbound.try_send((to.0, frame));
        Ok(())
    }

    async fn recv_datagram(&self) -> Result<(PeerId, Vec<u8>), TransportError> {
        match self.inbound.lock().await.recv().await {
            Some((peer, bytes)) => Ok((PeerId(peer), bytes)),
            None => Err(TransportError::Closed),
        }
    }

    async fn connect(&self, _to: PeerId) -> Result<Self::Stream, TransportError> {
        Err(TransportError::Closed)
    }

    async fn accept(&self) -> Result<(PeerId, Self::Stream), TransportError> {
        Err(TransportError::Closed)
    }
}

/// The flows this node's operator is live on. Shared between the plane's
/// admission checks (per datagram) and the hub (session open/close).
#[derive(Default)]
struct ActiveFlows(Mutex<HashSet<FlowId>>);

impl ActiveFlows {
    fn insert(&self, flow: FlowId) {
        self.0.lock().expect("flows lock").insert(flow);
    }

    fn remove(&self, flow: &FlowId) {
        self.0.lock().expect("flows lock").remove(flow);
    }
}

impl AdmissionPolicy for ActiveFlows {
    fn permits(&self, _peer: PeerId, service: Service, flow: FlowId) -> bool {
        service == Service::Voice && self.0.lock().expect("flows lock").contains(&flow)
    }
}

/// One live session's teardown handle: aborting the task drops the engine,
/// which aborts its pump and releases the flow registration.
struct SessionGuard {
    task: tokio::task::JoinHandle<()>,
    flow: FlowId,
    flows: Arc<ActiveFlows>,
}

impl SessionGuard {
    /// end the session and WAIT for its state to drop, so the next session
    /// for the same channel can re-register the flow without racing.
    async fn teardown(self) {
        self.task.abort();
        let _ = self.task.await;
        self.flows.remove(&self.flow);
    }
}

async fn hub_loop(
    mut requests: mpsc::Receiver<noded::VoiceSessionRequest>,
    outbound: mpsc::Sender<VoiceDatagram>,
    inbound: mpsc::Receiver<VoiceDatagram>,
) {
    let flows = Arc::new(ActiveFlows::default());
    let plane = DataPlane::new(
        ChannelTransport {
            outbound,
            inbound: tokio::sync::Mutex::new(inbound),
        },
        flows.clone() as Arc<dyn AdmissionPolicy>,
        // stream-class pacing config; voice runs no streams, so these only
        // need to exist.
        PlaneConfig {
            bulk_bytes_per_sec: 1 << 20,
            bulk_burst_bytes: 1 << 20,
        },
    );
    let mut active: Option<SessionGuard> = None;
    while let Some(request) = requests.recv().await {
        // one huddle at a time: a new join replaces the current session.
        if let Some(previous) = active.take() {
            previous.teardown().await;
        }
        let (session, guard) = match open_session(&plane, &flows, &request.channel_id) {
            Ok(opened) => opened,
            Err(refusal) => {
                let _ = request.reply.send(Err(refusal));
                continue;
            }
        };
        if request.reply.send(Ok(session)).is_err() {
            // the websocket died before the session opened.
            guard.teardown().await;
            continue;
        }
        active = Some(guard);
    }
    // the app surface dropped its lane (shutdown): end the live session so
    // the runtime can wind down.
    if let Some(previous) = active.take() {
        previous.teardown().await;
    }
}

fn open_session<T: DataPlaneTransport>(
    plane: &DataPlane<T>,
    flows: &Arc<ActiveFlows>,
    channel_id: &str,
) -> Result<(noded::VoiceSession, SessionGuard), String> {
    let flow = channel_flow(channel_id);
    let datagram_flow = plane
        .datagram_flow(
            Service::Voice,
            flow,
            DatagramPolicy {
                max_queued: FLOW_QUEUE,
            },
        )
        .map_err(|e| format!("voice flow unavailable for {channel_id}: {e}"))?;
    let engine = VoiceEngine::new(datagram_flow, VoiceConfig::default())
        .map_err(|e| format!("voice codec init failed: {e}"))?;
    flows.insert(flow);
    let (pcm_tx, pcm_rx) = mpsc::channel(PCM_LANE);
    let (mixed_tx, mixed_rx) = mpsc::channel(PCM_LANE);
    let (recipients_tx, recipients_rx) = watch::channel(Vec::new());
    let task = tokio::spawn(run_session(
        engine,
        pcm_rx,
        mixed_tx,
        recipients_rx,
        flows.clone(),
        flow,
    ));
    Ok((
        noded::VoiceSession {
            pcm_in: pcm_tx,
            mixed_out: mixed_rx,
            recipients: recipients_tx,
        },
        SessionGuard {
            task,
            flow,
            flows: flows.clone(),
        },
    ))
}

/// The session pump: captured frames out, mixed playout back, until the
/// websocket drops either lane end.
async fn run_session<T: DataPlaneTransport>(
    mut engine: VoiceEngine<T>,
    mut pcm_in: mpsc::Receiver<Vec<i16>>,
    mixed_out: mpsc::Sender<Vec<i16>>,
    recipients: watch::Receiver<Vec<[u8; 32]>>,
    flows: Arc<ActiveFlows>,
    flow: FlowId,
) {
    let mut tick = tokio::time::interval(Duration::from_millis(FRAME_MILLIS));
    // audio has no catch-up: a missed tick's frame is gone, do not burst.
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            captured = pcm_in.recv() => {
                let Some(captured) = captured else { break };
                if captured.len() != FRAME_SAMPLES {
                    continue; // not a whole frame — drop, stay alive
                }
                let peers: Vec<PeerId> = recipients
                    .borrow()
                    .iter()
                    .map(|raw| PeerId(*raw))
                    .collect();
                if peers.is_empty() {
                    continue; // alone in the huddle — nothing to send
                }
                let mut frame = [0i16; FRAME_SAMPLES];
                frame.copy_from_slice(&captured);
                // a send failure (peer unreachable, admission flapped) must
                // not end the session — the next frame just tries again.
                let _ = engine.send_frame(&frame, &peers).await;
            }
            _ = tick.tick() => {
                if mixed_out.is_closed() {
                    break;
                }
                let mixed = engine.playout();
                // full lane = the websocket is behind; drop this frame rather
                // than queue stale audio.
                let _ = mixed_out.try_send(mixed.to_vec());
            }
        }
    }
    flows.remove(&flow);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// two hubs wired back-to-back through their mesh lanes: a frame sent by
    /// one operator's session comes out of the other's mixed playout, and a
    /// replacement session (same hub) tears the first down and still works —
    /// proving flow re-registration after teardown.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn sessions_carry_audio_between_two_hubs_and_survive_replacement() {
        let key_a = [0xaa_u8; 32];
        let key_b = [0xbb_u8; 32];

        let (req_a_tx, req_a) = mpsc::channel(4);
        let (req_b_tx, req_b) = mpsc::channel(4);
        let (mut out_a, in_a) = spawn_hub(req_a);
        let (mut out_b, in_b) = spawn_hub(req_b);
        // the "mesh": a's outbound datagrams appear on b's inbound stamped
        // with a's key, and vice versa.
        tokio::spawn(async move {
            while let Some((_to, frame)) = out_a.recv().await {
                let _ = in_b.send((key_a, frame)).await;
            }
        });
        tokio::spawn(async move {
            while let Some((_to, frame)) = out_b.recv().await {
                let _ = in_a.send((key_b, frame)).await;
            }
        });

        let open = |lane: mpsc::Sender<noded::VoiceSessionRequest>| async move {
            let (reply, opened) = tokio::sync::oneshot::channel();
            lane.send(noded::VoiceSessionRequest {
                channel_id: "general".into(),
                reply,
            })
            .await
            .expect("hub alive");
            opened.await.expect("hub replies").expect("session opens")
        };
        // both request lanes must outlive the sessions: a closed lane means
        // app-surface shutdown and the hub tears its active session down.
        let session_a = open(req_a_tx.clone()).await;
        let mut session_b = open(req_b_tx.clone()).await;
        session_a
            .recipients
            .send(vec![key_b])
            .expect("session a alive");

        // a loud constant frame from a — b must eventually play out energy.
        let loud = vec![8000i16; FRAME_SAMPLES];
        let heard = async {
            loop {
                let _ = session_a.pcm_in.send(loud.clone()).await;
                let Some(mixed) = session_b.mixed_out.recv().await else {
                    panic!("session b ended early");
                };
                if mixed.iter().any(|s| s.abs() > 1000) {
                    break;
                }
            }
        };
        tokio::time::timeout(Duration::from_secs(10), heard)
            .await
            .expect("audio must cross the hubs");

        // replace a's session with a new one on the SAME channel: teardown
        // must release the flow so the re-open succeeds.
        let session_a2 = open(req_a_tx.clone()).await;
        assert!(
            !session_a2.pcm_in.is_closed(),
            "replacement session must be live"
        );
        drop((req_a_tx, req_b_tx));
    }
}
