//! The node's live-call runtime: the bridge between huddle websockets
//! (`noded`'s `/v1/voice/ws`, and the WebRTC gateway to come), the chat voice
//! engine + video wire, and the p2p mesh.
//!
//! Runtime shape mirrors the reachability plane's split exactly: the hub runs
//! on its OWN plain-tokio OS thread (the engine's pump and the 20 ms playout
//! tick are tokio-native), and mesh I/O crosses to the commonware runner over
//! channel pumps `main.rs` owns on two dedicated lanes: audio and call control
//! ride `CHANNEL_VOICE`, camera video rides `CHANNEL_VIDEO` so a keyframe burst
//! can't queue ahead of voice. Each datagram still carries the plane's
//! per-(service, flow) header, which demultiplexes the flows within a lane.
//!
//! Three pieces, all off-consensus:
//! - [`ChannelTransport`] — a datagram-only [`DataPlaneTransport`] arm over
//!   the pump channels. The designed transport is UDP on the reachability
//!   plane's WireGuard overlay; riding the authenticated TCP mesh instead
//!   trades head-of-line latency (absorbed by the engine's jitter buffer) for
//!   zero new infrastructure, and swaps out later behind the same trait
//!   without touching the engine.
//! - An [`AdmissionPolicy`] over the node's ACTIVE flows, now keyed by
//!   `(Service, FlowId)`: this node receives (and emits) call media only for
//!   flows its own operator has a live huddle session on — the mic and control
//!   flows on `Service::Voice`, the camera flow on `Service::Video`. The mesh
//!   already authenticates every peer as a workspace member; roster-level
//!   gating is the client's job (it steers the fan-out from consensus state),
//!   and unadmitted traffic drops counted at the plane per its default-deny
//!   contract.
//! - The hub loop — drains [`noded::CallSessionRequest`]s from the app
//!   surface and runs AT MOST ONE session at a time (Slack semantics: you are
//!   in one huddle). A session owns a [`VoiceEngine`] on the channel-derived
//!   audio flow plus datagram flows for camera video and call control, and
//!   pumps: websocket pcm in → encode + fan-out; a 20 ms tick → mixed playout
//!   → websocket out; captured camera frames → fragment + fan-out; inbound
//!   fragments → reassemble → webview; and the call-control machinery
//!   (keyframe requests, 1 Hz presence beacons, the sender/receiver bitrate
//!   ladder). Dropping the webview ends tears the session down; a new request
//!   replaces the current session.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chat::voice::{FRAME_MILLIS, FRAME_SAMPLES, VoiceConfig, VoiceEngine};
use data_plane::{
    AdmissionPolicy, DataPlane, DataPlaneTransport, DatagramFlow, DatagramPolicy, FlowId, PeerId,
    PlaneConfig, Service, TransportError,
};
use tokio::sync::{mpsc, watch};

/// One call datagram crossing the mesh pumps: (raw ed25519 peer key, frame).
/// Outbound the key names the recipient; inbound it is the authenticated
/// sender the mesh reports. The frame carries the plane's `(service, flow)`
/// header; [`ChannelTransport`] routes each outbound datagram to the voice or
/// video mesh lane by that header's service byte.
pub type VoiceDatagram = ([u8; 32], Vec<u8>);

/// Voice mesh-pump lane depth: ~5 s of one speaker's frames. Call media is
/// fire-and-forget, so overflow drops rather than backpressures.
const WIRE_LANE: usize = 256;
/// Video mesh-pump lane depth: ~4 keyframes of fragments. Its own outbound
/// lane so a keyframe burst can't queue ahead of voice.
const VIDEO_WIRE_LANE: usize = 512;
/// Inbound audio queue per flow: ~2.5 s of one speaker's frames. Overflow
/// drops the oldest inside the flow (the plane's drop-oldest contract).
const FLOW_QUEUE: usize = 128;
/// Inbound camera queue per flow: ~2 keyframe-burst frames of fragments.
const VIDEO_FLOW_QUEUE: usize = 256;
/// Inbound call-control queue per flow: tiny, one message per event.
const CTL_FLOW_QUEUE: usize = 32;
/// Webview↔hub pcm lanes: a small cushion (8 × 20 ms); late audio is dead
/// audio, so both sides drop rather than backpressure when it fills.
const PCM_LANE: usize = 8;
/// Webview↔hub video lanes: frames (not fragments) — ~1 s at 30 fps.
const VIDEO_LANE: usize = 32;
/// Webview↔hub call-control lanes.
const CTL_LANE: usize = 32;

/// derive the audio flow for a chat channel — the exact domain string both
/// ends agree on (every participant derives it from the same channel id).
fn channel_flow(channel_id: &str) -> FlowId {
    FlowId::derive(format!("voice-channel:{channel_id}").as_bytes())
}

/// the camera flow for a chat channel (Service::Video).
fn video_flow(channel_id: &str) -> FlowId {
    FlowId::derive(format!("video-channel:{channel_id}").as_bytes())
}

/// the call-control flow (Service::Voice — control must work in an
/// audio-only build, ADR §2).
fn ctl_flow(channel_id: &str) -> FlowId {
    FlowId::derive(format!("callctl-channel:{channel_id}").as_bytes())
}

/// Stand up the call runtime on its own OS thread and return the mesh ends:
/// `main.rs` drains the voice outbound receiver into the `CHANNEL_VOICE`
/// sender and the video outbound receiver into the `CHANNEL_VIDEO` sender, and
/// feeds mesh receipts from both lanes into the one inbound sender.
/// [`ChannelTransport`] routes each datagram to a lane by its plane header's
/// service byte (`frame[1]`); audio and call control ride voice, camera video
/// rides video. `requests` is the app surface's session lane
/// ([`noded::NodeHandle::with_call`]).
pub fn spawn_hub(
    requests: mpsc::Receiver<noded::CallSessionRequest>,
) -> (
    mpsc::Receiver<VoiceDatagram>,
    mpsc::Receiver<VoiceDatagram>,
    mpsc::Sender<VoiceDatagram>,
) {
    let (outbound_voice_tx, outbound_voice_rx) = mpsc::channel(WIRE_LANE);
    let (outbound_video_tx, outbound_video_rx) = mpsc::channel(VIDEO_WIRE_LANE);
    let (inbound_tx, inbound_rx) = mpsc::channel(WIRE_LANE);
    std::thread::Builder::new()
        .name("voice-hub".into())
        .spawn(move || {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("voice-hub tokio runtime")
                .block_on(hub_loop(
                    requests,
                    outbound_voice_tx,
                    outbound_video_tx,
                    inbound_rx,
                ));
        })
        .expect("spawn voice-hub thread");
    (outbound_voice_rx, outbound_video_rx, inbound_tx)
}

/// The datagram-only transport arm over the mesh pump channels. Call media
/// never opens streams, so the stream half reports [`TransportError::Closed`]
/// — the plane's acceptor loop exits immediately and `connect` refuses.
struct ChannelTransport {
    outbound_voice: mpsc::Sender<VoiceDatagram>,
    outbound_video: mpsc::Sender<VoiceDatagram>,
    inbound: tokio::sync::Mutex<mpsc::Receiver<VoiceDatagram>>,
}

impl DataPlaneTransport for ChannelTransport {
    type Stream = tokio::io::DuplexStream;

    fn max_datagram(&self) -> usize {
        data_plane::MAX_DATAGRAM
    }

    async fn send_datagram(&self, to: PeerId, frame: Vec<u8>) -> Result<(), TransportError> {
        // fire-and-forget per the trait: a full pump lane drops the frame —
        // call media retries nothing, the jitter buffer / next keyframe
        // renders the gap.
        // route by the plane header's service byte: video fragments ride
        // their own mesh lane so a keyframe burst can't queue ahead of voice.
        let lane = if frame.get(1) == Some(&(Service::Video as u8)) {
            &self.outbound_video
        } else {
            &self.outbound_voice
        };
        let _ = lane.try_send((to.0, frame));
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

/// The `(service, flow)` pairs this node's operator is live on. Shared between
/// the plane's admission checks (per datagram) and the hub (session
/// open/close). A session admits three: mic + control on `Service::Voice`,
/// camera on `Service::Video`.
#[derive(Default)]
struct ActiveFlows(Mutex<HashSet<(Service, FlowId)>>);

impl ActiveFlows {
    fn insert(&self, key: (Service, FlowId)) {
        self.0.lock().expect("flows lock").insert(key);
    }

    fn remove(&self, key: &(Service, FlowId)) {
        self.0.lock().expect("flows lock").remove(key);
    }
}

impl AdmissionPolicy for ActiveFlows {
    fn permits(&self, _peer: PeerId, service: Service, flow: FlowId) -> bool {
        self.0.lock().expect("flows lock").contains(&(service, flow))
    }
}

/// One live session's teardown handle: aborting the task drops the engine and
/// the video/control flow handles, releasing their plane registrations.
struct SessionGuard {
    task: tokio::task::JoinHandle<()>,
    /// the three `(service, flow)` admissions this session opened.
    registered: [(Service, FlowId); 3],
    flows: Arc<ActiveFlows>,
}

impl SessionGuard {
    /// end the session and WAIT for its state to drop, so the next session
    /// for the same channel can re-register the flows without racing.
    async fn teardown(self) {
        self.task.abort();
        let _ = self.task.await;
        for key in &self.registered {
            self.flows.remove(key);
        }
    }
}

async fn hub_loop(
    mut requests: mpsc::Receiver<noded::CallSessionRequest>,
    outbound_voice: mpsc::Sender<VoiceDatagram>,
    outbound_video: mpsc::Sender<VoiceDatagram>,
    inbound: mpsc::Receiver<VoiceDatagram>,
) {
    let flows = Arc::new(ActiveFlows::default());
    let plane = DataPlane::new(
        ChannelTransport {
            outbound_voice,
            outbound_video,
            inbound: tokio::sync::Mutex::new(inbound),
        },
        flows.clone() as Arc<dyn AdmissionPolicy>,
        // stream-class pacing config; call media runs no streams, so these
        // only need to exist.
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
        let (session, guard) = match open_session(&plane, &flows, &request.channel_id).await {
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

/// register one datagram flow, retrying the ~1 s window a torn-down
/// predecessor needs to release it. a same-channel rejoin can transiently
/// collide because the previous session's engine/flow handles release their
/// plane registration asynchronously (task abort), so retry instead of
/// refusing the join. a loaded runtime can take a while to actually drop the
/// aborted pump — 40 × 25 ms ≈ 1 s.
async fn register_datagram_flow<T: DataPlaneTransport>(
    plane: &DataPlane<T>,
    service: Service,
    flow: FlowId,
    max_queued: usize,
    channel_id: &str,
    label: &str,
) -> Result<DatagramFlow<T>, String> {
    let mut attempts = 0;
    loop {
        match plane.datagram_flow(service, flow, DatagramPolicy { max_queued }) {
            Ok(handle) => return Ok(handle),
            Err(e) if attempts >= 40 => {
                return Err(format!("{label} flow unavailable for {channel_id}: {e}"));
            }
            Err(_) => {
                attempts += 1;
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        }
    }
}

async fn open_session<T: DataPlaneTransport>(
    plane: &DataPlane<T>,
    flows: &Arc<ActiveFlows>,
    channel_id: &str,
) -> Result<(noded::CallSession, SessionGuard), String> {
    let mic_flow = channel_flow(channel_id);
    let cam_flow = video_flow(channel_id);
    let control_flow = ctl_flow(channel_id);

    // register all three datagram flows (each behind the retry loop, since a
    // torn-down predecessor releases them asynchronously).
    let mic_dgram =
        register_datagram_flow(plane, Service::Voice, mic_flow, FLOW_QUEUE, channel_id, "voice")
            .await?;
    let cam_dgram = register_datagram_flow(
        plane,
        Service::Video,
        cam_flow,
        VIDEO_FLOW_QUEUE,
        channel_id,
        "video",
    )
    .await?;
    let ctl_dgram = register_datagram_flow(
        plane,
        Service::Voice,
        control_flow,
        CTL_FLOW_QUEUE,
        channel_id,
        "control",
    )
    .await?;

    let engine = VoiceEngine::new(mic_dgram, VoiceConfig::default())
        .map_err(|e| format!("voice codec init failed: {e}"))?;

    let registered = [
        (Service::Voice, mic_flow),
        (Service::Video, cam_flow),
        (Service::Voice, control_flow),
    ];
    for key in &registered {
        flows.insert(*key);
    }

    let (pcm_tx, pcm_rx) = mpsc::channel(PCM_LANE);
    let (mixed_tx, mixed_rx) = mpsc::channel(PCM_LANE);
    let (recipients_tx, recipients_rx) = watch::channel(Vec::new());
    let (video_in_tx, video_in_rx) = mpsc::channel(VIDEO_LANE);
    let (video_out_tx, video_out_rx) = mpsc::channel(VIDEO_LANE);
    let (control_in_tx, control_in_rx) = mpsc::channel(CTL_LANE);
    let (control_out_tx, control_out_rx) = mpsc::channel(CTL_LANE);

    let task = tokio::spawn(run_session(
        engine,
        cam_dgram,
        ctl_dgram,
        pcm_rx,
        mixed_tx,
        video_in_rx,
        video_out_tx,
        control_in_rx,
        control_out_tx,
        recipients_rx,
        flows.clone(),
        registered,
    ));
    Ok((
        noded::CallSession {
            pcm_in: pcm_tx,
            mixed_out: mixed_rx,
            recipients: recipients_tx,
            video_in: video_in_tx,
            video_out: video_out_rx,
            control_in: control_in_tx,
            control_out: control_out_rx,
        },
        SessionGuard {
            task,
            registered,
            flows: flows.clone(),
        },
    ))
}

/// per-sending-peer receive state on the video/control flows.
struct PeerLane {
    reassembler: chat::video::Reassembler,
    /// last time we asked THIS peer for a keyframe (≥1 s apart).
    last_keyframe_req: Option<Instant>,
    /// `dropped_frames()` when we last inspected this peer — a keyframe ask
    /// fires only when the count ADVANCES, so mid-frame fragments don't spam.
    last_seen_dropped: u64,
    /// the hint we currently give this peer, and this window's loss counts.
    hint_kbps: u32,
    clean_windows: u8,
    window_complete: u64,
    window_dropped_base: u64, // reassembler.dropped_frames() at window start
}

impl PeerLane {
    fn new() -> Self {
        PeerLane {
            reassembler: chat::video::Reassembler::default(),
            last_keyframe_req: None,
            last_seen_dropped: 0,
            hint_kbps: chat::video::RATE_LADDER_KBPS[0],
            clean_windows: 0,
            window_complete: 0,
            window_dropped_base: 0,
        }
    }
}

/// The session pump: audio + camera video + call control, until the webview
/// drops its lane ends.
#[allow(clippy::too_many_arguments)]
async fn run_session<T: DataPlaneTransport>(
    mut engine: VoiceEngine<T>,
    video: DatagramFlow<T>,
    ctl: DatagramFlow<T>,
    mut pcm_in: mpsc::Receiver<Vec<i16>>,
    mixed_out: mpsc::Sender<Vec<i16>>,
    mut video_in: mpsc::Receiver<noded::CapturedVideo>,
    video_out: mpsc::Sender<noded::PeerVideo>,
    mut control_in: mpsc::Receiver<noded::CallControlIn>,
    control_out: mpsc::Sender<noded::CallControlOut>,
    recipients: watch::Receiver<Vec<[u8; 32]>>,
    flows: Arc<ActiveFlows>,
    registered: [(Service, FlowId); 3],
) {
    let mut tick = tokio::time::interval(Duration::from_millis(FRAME_MILLIS));
    // audio has no catch-up: a missed tick's frame is gone, do not burst.
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    let mut frame_no: u32 = 0;
    let mut peer_lanes: HashMap<[u8; 32], PeerLane> = HashMap::new();
    // what the webview last told us — repeated at 1 Hz as our beacon.
    let (mut muted, mut camera_on) = (true, false);
    // rate hints RECEIVED from each peer about OUR sending; effective = min.
    let mut inbound_hints: HashMap<[u8; 32], u32> = HashMap::new();
    let mut effective_kbps: u32 = chat::video::RATE_LADDER_KBPS[0];
    // ≥1 s between keyframes we ask our own encoder for.
    let mut last_encoder_kick: Option<Instant> = None;
    let mut ctl_tick = tokio::time::interval(Duration::from_secs(1));
    ctl_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut window: u8 = 0; // 5 ctl ticks = one rate window

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
            captured = video_in.recv() => {
                let Some(frame) = captured else { break };
                let recipients_now: Vec<PeerId> =
                    recipients.borrow().iter().map(|raw| PeerId(*raw)).collect();
                if recipients_now.is_empty() { continue; }
                let Ok(fragments) = chat::video::fragment_frame(
                    frame_no, frame.keyframe, frame.ts_ms, &frame.data,
                ) else { continue }; // oversize/empty: drop, stay alive
                frame_no = frame_no.wrapping_add(1);
                for fragment in &fragments {
                    for peer in &recipients_now {
                        // fire-and-forget, same posture as voice.
                        let _ = video.send_to(*peer, fragment).await;
                    }
                }
            }
            inbound = video.recv() => {
                let (peer, bytes) = inbound;
                let Ok((header, payload)) = chat::video::decode_fragment(&bytes) else { continue };
                let lane = peer_lanes.entry(peer.0).or_insert_with(PeerLane::new);
                match lane.reassembler.insert(header, payload) {
                    chat::video::Assembly::Complete(done) => {
                        lane.window_complete += 1;
                        // full lane = the webview is behind; a dropped frame
                        // is recovered by the next keyframe request from the
                        // browser decoder, so shed rather than backpressure.
                        let _ = video_out.try_send(noded::PeerVideo {
                            peer: peer.0,
                            keyframe: done.keyframe,
                            ts_ms: done.ts_ms,
                            data: done.data,
                        });
                    }
                    chat::video::Assembly::Progress | chat::video::Assembly::Stale => {
                        // a frame died incomplete since we last looked → ask
                        // its sender for a sync point (rate-limited). gate on
                        // the dropped counter ADVANCING so mid-frame fragments
                        // of a healthy frame don't spam the limiter.
                        let dropped_now = lane.reassembler.dropped_frames();
                        if dropped_now > lane.last_seen_dropped {
                            lane.last_seen_dropped = dropped_now;
                            request_keyframe_if_due(&ctl, peer, lane).await;
                        }
                    }
                }
            }
            inbound = ctl.recv() => {
                let (peer, bytes) = inbound;
                let Ok(message) = chat::video::CallControl::decode(&bytes) else { continue };
                match message {
                    chat::video::CallControl::KeyframeRequest => {
                        // honor at most one encoder kick per second.
                        let due = last_encoder_kick
                            .is_none_or(|at| at.elapsed() >= Duration::from_secs(1));
                        if due {
                            last_encoder_kick = Some(Instant::now());
                            let _ = control_out.try_send(noded::CallControlOut::KeyframeRequest);
                        }
                    }
                    chat::video::CallControl::Beacon { muted, camera_on } => {
                        let _ = control_out.try_send(noded::CallControlOut::PeerBeacon {
                            peer: peer.0, muted, camera_on,
                        });
                    }
                    chat::video::CallControl::RateHint { max_kbps } => {
                        inbound_hints.insert(peer.0, max_kbps);
                        push_effective_rate(
                            &recipients, &inbound_hints, &mut effective_kbps, &control_out,
                        );
                    }
                }
            }
            state = control_in.recv() => {
                let Some(state) = state else { break };
                match state {
                    noded::CallControlIn::Beacon { muted: m, camera_on: c } => {
                        (muted, camera_on) = (m, c);
                        // push immediately so toggles feel live; the 1 Hz
                        // tick keeps late joiners current.
                        send_beacon(&ctl, &recipients, muted, camera_on).await;
                    }
                    noded::CallControlIn::KeyframeRequest { peer } => {
                        if let Some(lane) = peer_lanes.get_mut(&peer) {
                            request_keyframe_if_due(&ctl, PeerId(peer), lane).await;
                        }
                    }
                }
            }
            _ = ctl_tick.tick() => {
                send_beacon(&ctl, &recipients, muted, camera_on).await;
                // hints from peers no longer in the roster must not pin our rate.
                let live: HashSet<[u8; 32]> = recipients.borrow().iter().copied().collect();
                inbound_hints.retain(|peer, _| live.contains(peer));
                push_effective_rate(&recipients, &inbound_hints, &mut effective_kbps, &control_out);
                window += 1;
                if window >= 5 {
                    window = 0;
                    evaluate_rate_windows(&ctl, &mut peer_lanes).await;
                }
            }
        }
    }
    for key in &registered {
        flows.remove(key);
    }
}

/// send a KeyframeRequest to `peer` unless one went out under a second ago.
async fn request_keyframe_if_due<T: DataPlaneTransport>(
    ctl: &DatagramFlow<T>,
    peer: PeerId,
    lane: &mut PeerLane,
) {
    if lane
        .last_keyframe_req
        .is_none_or(|at| at.elapsed() >= Duration::from_secs(1))
    {
        lane.last_keyframe_req = Some(Instant::now());
        let _ = ctl
            .send_to(peer, &chat::video::CallControl::KeyframeRequest.encode())
            .await;
    }
}

/// our 1 Hz presence beacon to every current recipient.
async fn send_beacon<T: DataPlaneTransport>(
    ctl: &DatagramFlow<T>,
    recipients: &watch::Receiver<Vec<[u8; 32]>>,
    muted: bool,
    camera_on: bool,
) {
    let frame = chat::video::CallControl::Beacon { muted, camera_on }.encode();
    let peers: Vec<PeerId> = recipients.borrow().iter().map(|raw| PeerId(*raw)).collect();
    for peer in peers {
        let _ = ctl.send_to(peer, &frame).await;
    }
}

/// sender side of REMB: min inbound hint (or the ladder top with no hints),
/// forwarded to the webview encoder only when it changes.
fn push_effective_rate(
    recipients: &watch::Receiver<Vec<[u8; 32]>>,
    inbound_hints: &HashMap<[u8; 32], u32>,
    effective_kbps: &mut u32,
    control_out: &mpsc::Sender<noded::CallControlOut>,
) {
    let live = recipients.borrow();
    let next = live
        .iter()
        .filter_map(|peer| inbound_hints.get(peer))
        .copied()
        .min()
        .unwrap_or(chat::video::RATE_LADDER_KBPS[0]);
    if next != *effective_kbps {
        *effective_kbps = next;
        let _ = control_out.try_send(noded::CallControlOut::RateHint { max_kbps: next });
    }
}

/// receiver side of REMB, every 5 s per sending peer: >10% lost frames steps
/// the hint down; 3 consecutive clean windows step it back up. Hints are
/// sent only when they change.
async fn evaluate_rate_windows<T: DataPlaneTransport>(
    ctl: &DatagramFlow<T>,
    peers: &mut HashMap<[u8; 32], PeerLane>,
) {
    for (raw, lane) in peers.iter_mut() {
        let dropped = lane.reassembler.dropped_frames() - lane.window_dropped_base;
        let complete = lane.window_complete;
        lane.window_dropped_base = lane.reassembler.dropped_frames();
        lane.window_complete = 0;
        if complete + dropped == 0 {
            continue; // peer isn't sending video — nothing to rate
        }
        let lossy = dropped * 10 > (complete + dropped); // >10%
        let next = if lossy {
            lane.clean_windows = 0;
            chat::video::step_down(lane.hint_kbps)
        } else {
            lane.clean_windows = lane.clean_windows.saturating_add(1);
            if lane.clean_windows >= 3 {
                lane.clean_windows = 0;
                chat::video::step_up(lane.hint_kbps)
            } else {
                lane.hint_kbps
            }
        };
        if next != lane.hint_kbps {
            lane.hint_kbps = next;
            let _ = ctl
                .send_to(
                    PeerId(*raw),
                    &chat::video::CallControl::RateHint { max_kbps: next }.encode(),
                )
                .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// open (or replace) a session on channel "general" over a hub lane.
    async fn open(lane: mpsc::Sender<noded::CallSessionRequest>) -> noded::CallSession {
        let (reply, opened) = tokio::sync::oneshot::channel();
        lane.send(noded::CallSessionRequest {
            channel_id: "general".into(),
            reply,
        })
        .await
        .expect("hub alive");
        opened.await.expect("hub replies").expect("session opens")
    }

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
        let (voice_out_a, video_out_a, in_a) = spawn_hub(req_a);
        let (voice_out_b, video_out_b, in_b) = spawn_hub(req_b);
        // the "mesh": a's outbound datagrams — BOTH the voice and video lanes —
        // appear on b's inbound stamped with a's key, and vice versa.
        for mut out in [voice_out_a, video_out_a] {
            let in_b = in_b.clone();
            tokio::spawn(async move {
                while let Some((_to, frame)) = out.recv().await {
                    let _ = in_b.send((key_a, frame)).await;
                }
            });
        }
        for mut out in [voice_out_b, video_out_b] {
            let in_a = in_a.clone();
            tokio::spawn(async move {
                while let Some((_to, frame)) = out.recv().await {
                    let _ = in_a.send((key_b, frame)).await;
                }
            });
        }

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
        // must release the flows so the re-open succeeds.
        let session_a2 = open(req_a_tx.clone()).await;
        assert!(
            !session_a2.pcm_in.is_closed(),
            "replacement session must be live"
        );
        drop((req_a_tx, req_b_tx));
    }

    /// wire two hubs A→B / B→A applying `a_to_b` to every A→B datagram (it may
    /// swallow a frame to model loss). returns the two request lanes; keep them
    /// alive for the test's duration.
    fn two_hubs(
        key_a: [u8; 32],
        key_b: [u8; 32],
        mut a_to_b: impl FnMut(&[u8]) -> bool + Send + 'static,
    ) -> (
        mpsc::Sender<noded::CallSessionRequest>,
        mpsc::Sender<noded::CallSessionRequest>,
    ) {
        let (req_a_tx, req_a) = mpsc::channel(4);
        let (req_b_tx, req_b) = mpsc::channel(4);
        let (mut voice_out_a, mut video_out_a, in_a) = spawn_hub(req_a);
        let (voice_out_b, video_out_b, in_b) = spawn_hub(req_b);
        // A→B: the loss filter guards BOTH of A's outbound lanes, so a
        // per-frame decision (e.g. "drop the first frag_index==1") sees every
        // A→B datagram regardless of which lane carried it. Merge the two
        // lanes into one filtered forwarder to keep the filter single-owner.
        tokio::spawn(async move {
            loop {
                let frame = tokio::select! {
                    Some((_to, f)) = voice_out_a.recv() => f,
                    Some((_to, f)) = video_out_a.recv() => f,
                    else => break,
                };
                if a_to_b(&frame) {
                    let _ = in_b.send((key_a, frame)).await;
                }
            }
        });
        // B→A: unfiltered; forward both of B's lanes.
        for mut out in [voice_out_b, video_out_b] {
            let in_a = in_a.clone();
            tokio::spawn(async move {
                while let Some((_to, frame)) = out.recv().await {
                    let _ = in_a.send((key_b, frame)).await;
                }
            });
        }
        (req_a_tx, req_b_tx)
    }

    /// captured camera frames fan out from A, fragment across the plane, and
    /// reassemble intact on B — a multi-fragment keyframe then a delta.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn video_frames_fragment_and_cross_hubs() {
        let key_a = [0xaa_u8; 32];
        let key_b = [0xbb_u8; 32];
        let (req_a_tx, req_b_tx) = two_hubs(key_a, key_b, |_| true);

        let session_a = open(req_a_tx.clone()).await;
        let mut session_b = open(req_b_tx.clone()).await;
        session_a
            .recipients
            .send(vec![key_b])
            .expect("session a alive");

        // position-dependent fills (not uniform bytes) so a fragment-ordering
        // or reassembly regression in the hub path shows up — a reordered or
        // duplicated fragment would break exact-equality on the full vector.
        let keyframe_data: Vec<u8> = (0..5000).map(|i| (i % 251) as u8).collect();
        let delta_data: Vec<u8> = (0..5000).map(|i| ((i * 7 + 3) % 251) as u8).collect();

        // a 5000-byte keyframe fragments across ≥4 datagrams.
        session_a
            .video_in
            .send(noded::CapturedVideo {
                keyframe: true,
                ts_ms: 7,
                data: keyframe_data.clone(),
            })
            .await
            .expect("session a alive");
        let got = tokio::time::timeout(Duration::from_secs(10), session_b.video_out.recv())
            .await
            .expect("video must cross the hubs")
            .expect("session b alive");
        assert_eq!(got.peer, key_a);
        assert!(got.keyframe);
        assert_eq!(got.ts_ms, 7);
        assert_eq!(got.data, keyframe_data);

        // a second (delta) frame with a different fill crosses intact too.
        session_a
            .video_in
            .send(noded::CapturedVideo {
                keyframe: false,
                ts_ms: 40,
                data: delta_data.clone(),
            })
            .await
            .expect("session a alive");
        let got = tokio::time::timeout(Duration::from_secs(10), session_b.video_out.recv())
            .await
            .expect("second video frame must cross")
            .expect("session b alive");
        assert!(!got.keyframe);
        assert_eq!(got.ts_ms, 40);
        assert_eq!(got.data, delta_data);

        drop((req_a_tx, req_b_tx));
    }

    /// a lost fragment leaves frame 0 incomplete; when frame 1 supersedes it,
    /// B notices the drop and asks A's encoder (over the control flow) for a
    /// keyframe. B emits only frame 1's bytes.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn lost_fragment_triggers_keyframe_request() {
        let key_a = [0xaa_u8; 32];
        let key_b = [0xbb_u8; 32];
        // drop the FIRST A→B video datagram whose decoded header has
        // frag_index == 1; pass everything else (audio, control, other frags).
        let mut dropped_one = false;
        let (req_a_tx, req_b_tx) = two_hubs(key_a, key_b, move |frame| {
            if !dropped_one
                && frame.len() > 12
                && frame[1] == Service::Video as u8
                && let Ok((header, _)) = chat::video::decode_fragment(&frame[12..])
                && header.frag_index == 1
            {
                dropped_one = true;
                return false; // swallow this fragment
            }
            true
        });

        let session_a = open(req_a_tx.clone()).await;
        let mut session_b = open(req_b_tx.clone()).await;
        session_a
            .recipients
            .send(vec![key_b])
            .expect("session a alive");
        let mut control_a = session_a.control_out;

        // frame 0 loses a fragment (incomplete); frame 1 completes.
        session_a
            .video_in
            .send(noded::CapturedVideo {
                keyframe: true,
                ts_ms: 1,
                data: vec![0xA0; 5000],
            })
            .await
            .expect("session a alive");
        session_a
            .video_in
            .send(noded::CapturedVideo {
                keyframe: false,
                ts_ms: 2,
                data: vec![0xB1; 5000],
            })
            .await
            .expect("session a alive");

        // B emits ONLY frame 1's bytes (frame 0 died incomplete).
        let got = tokio::time::timeout(Duration::from_secs(10), session_b.video_out.recv())
            .await
            .expect("frame 1 must cross")
            .expect("session b alive");
        assert_eq!(got.ts_ms, 2);
        assert_eq!(got.data, vec![0xB1; 5000]);

        // B's hub asked A's encoder to sync — the keyframe request crossed the
        // control flow and A surfaced it to its webview.
        let kick = tokio::time::timeout(Duration::from_secs(10), control_a.recv())
            .await
            .expect("keyframe request must reach A")
            .expect("session a alive");
        assert!(matches!(kick, noded::CallControlOut::KeyframeRequest));

        drop((req_a_tx, req_b_tx));
    }

    /// a beacon the webview pushes on A crosses the control flow and lands on
    /// B as peer state tagged with A's key.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn beacons_cross_as_peer_state() {
        let key_a = [0xaa_u8; 32];
        let key_b = [0xbb_u8; 32];
        let (req_a_tx, req_b_tx) = two_hubs(key_a, key_b, |_| true);

        let session_a = open(req_a_tx.clone()).await;
        let mut session_b = open(req_b_tx.clone()).await;
        // A beacons to its recipients — B must be one of them.
        session_a
            .recipients
            .send(vec![key_b])
            .expect("session a alive");

        session_a
            .control_in
            .send(noded::CallControlIn::Beacon {
                muted: false,
                camera_on: true,
            })
            .await
            .expect("session a alive");

        // B's control_out yields A's beacon as peer state (the 1 Hz tick also
        // repeats it, so a generous timeout is safe).
        let state = loop {
            let msg = tokio::time::timeout(Duration::from_secs(10), session_b.control_out.recv())
                .await
                .expect("beacon must reach B")
                .expect("session b alive");
            if let noded::CallControlOut::PeerBeacon {
                peer,
                muted,
                camera_on,
            } = msg
            {
                break (peer, muted, camera_on);
            }
        };
        assert_eq!(state, (key_a, false, true));

        drop((req_a_tx, req_b_tx));
    }
}
