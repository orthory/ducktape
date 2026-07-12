//! The node's live-call runtime: the bridge between huddle websockets
//! (`noded`'s `/v1/call/ws` typed audio+video+control socket), the chat voice
//! engine + video wire, and the p2p mesh.
//!
//! Runtime shape mirrors the reachability plane's split exactly: the hub runs
//! on its OWN plain-tokio OS thread (the engine's pump and the 20 ms playout
//! tick are tokio-native). On that thread it binds the per-service overlay
//! media planes ([`crate::voice_plane`]) and serves sessions over them —
//! audio + call control on `Service::Voice`'s overlay socket, camera video on
//! `Service::Video`'s. Distinct overlay ports mean the two streams never share
//! a socket or a send queue, so a video keyframe burst can't starve voice (the
//! failure the mesh-encapsulation arm this replaced was prone to — see
//! [`crate::voice_plane`]). Media rides ONLY the overlay: no overlay, no media.
//!
//! Three pieces, all off-consensus:
//! - Two per-use [`DataPlane`]s over the overlay ([`crate::voice_plane`]),
//!   built lazily once the reachability plane has the interface up.
//! - An [`AdmissionPolicy`] over the node's ACTIVE flows, keyed by
//!   `(Service, FlowId)`: this node receives (and emits) call media only for
//!   flows its own operator has a live huddle session on — the mic and control
//!   flows on `Service::Voice`, the camera flow on `Service::Video`. The
//!   overlay already authenticates every peer by its source `/128`;
//!   roster-level gating is the client's job (it steers the fan-out from
//!   consensus state), and unadmitted traffic drops counted at the plane per
//!   its default-deny contract.
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
    Service, SocketFactory,
};
use tokio::sync::{mpsc, watch};

use crate::voice_plane::MediaPeers;

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

/// Stand up the call runtime on its own OS thread. The hub binds the voice and
/// video overlay planes on that thread's runtime (retrying until the overlay
/// `/128` is up) and serves one huddle session at a time over them. `requests`
/// is the app surface's session lane ([`noded::NodeHandle::with_call`]);
/// `factory`/`peers`/`me` are the overlay socket seam, the tracked media peer
/// set (refreshed by the host on valset cutover), and this node's own key.
///
/// Media rides ONLY the overlay — with no overlay there is no media transport
/// (the overlay-only cutover, no mesh fallback), so the host spawns the hub
/// only where the overlay is reachable.
pub fn spawn_hub(
    requests: mpsc::Receiver<noded::CallSessionRequest>,
    factory: Arc<dyn SocketFactory>,
    peers: Arc<MediaPeers>,
    me: [u8; 32],
    planes: data_plane::PlaneMonitor,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("voice-hub".into())
        .spawn(move || {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("voice-hub tokio runtime")
                .block_on(hub_loop(requests, factory, peers, me, planes));
        })
        .expect("spawn voice-hub thread")
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

/// Build the two overlay media planes on the hub runtime (blocking until the
/// overlay is up), then serve sessions over them. One shared active-flow set
/// answers admission for both planes.
async fn hub_loop(
    requests: mpsc::Receiver<noded::CallSessionRequest>,
    factory: Arc<dyn SocketFactory>,
    peers: Arc<MediaPeers>,
    me: [u8; 32],
    planes: data_plane::PlaneMonitor,
) {
    let flows = Arc::new(ActiveFlows::default());
    let (voice_plane, video_plane) = crate::voice_plane::bind_media_planes(
        factory,
        peers,
        me,
        flows.clone() as Arc<dyn AdmissionPolicy>,
    )
    .await;
    // huddle media is the chat module's: both planes report under it.
    planes.register("chat", Service::Voice, voice_plane.watch());
    planes.register("chat", Service::Video, video_plane.watch());
    serve_sessions(requests, voice_plane, video_plane, flows).await;
}

/// The session request loop: run AT MOST ONE session at a time (Slack
/// semantics), each over the shared voice + video planes. Generic over the
/// transport so tests drive it over an in-memory link.
async fn serve_sessions<T: DataPlaneTransport>(
    mut requests: mpsc::Receiver<noded::CallSessionRequest>,
    voice_plane: DataPlane<T>,
    video_plane: DataPlane<T>,
    flows: Arc<ActiveFlows>,
) {
    let mut active: Option<SessionGuard> = None;
    while let Some(request) = requests.recv().await {
        // one huddle at a time: a new join replaces the current session.
        if let Some(previous) = active.take() {
            previous.teardown().await;
        }
        let (session, guard) =
            match open_session(&voice_plane, &video_plane, &flows, &request.channel_id).await {
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
    voice_plane: &DataPlane<T>,
    video_plane: &DataPlane<T>,
    flows: &Arc<ActiveFlows>,
    channel_id: &str,
) -> Result<(noded::CallSession, SessionGuard), String> {
    let mic_flow = channel_flow(channel_id);
    let cam_flow = video_flow(channel_id);
    let control_flow = ctl_flow(channel_id);

    // register all three datagram flows (each behind the retry loop, since a
    // torn-down predecessor releases them asynchronously). mic + control ride
    // the voice plane (Service::Voice, overlay port 45902); camera rides the
    // video plane (Service::Video, port 45903) — separate sockets, so a video
    // burst can't queue ahead of voice.
    let mic_dgram = register_datagram_flow(
        voice_plane,
        Service::Voice,
        mic_flow,
        FLOW_QUEUE,
        channel_id,
        "voice",
    )
    .await?;
    let cam_dgram = register_datagram_flow(
        video_plane,
        Service::Video,
        cam_flow,
        VIDEO_FLOW_QUEUE,
        channel_id,
        "video",
    )
    .await?;
    let ctl_dgram = register_datagram_flow(
        voice_plane,
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
    mut video_in: mpsc::Receiver<chat::call_wire::CapturedFrame>,
    video_out: mpsc::Sender<chat::call_wire::PeerFrame>,
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
    let (mut muted, mut camera_on, mut sharing) = (true, false, false);
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
                        let _ = video_out.try_send(chat::call_wire::PeerFrame {
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
                    chat::video::CallControl::Beacon { muted, camera_on, sharing } => {
                        let _ = control_out.try_send(noded::CallControlOut::PeerBeacon {
                            peer: peer.0, muted, camera_on, sharing,
                        });
                    }
                    chat::video::CallControl::RateHint { max_kbps } => {
                        // hints outside the ladder are hostile-or-broken; clamping
                        // preserves min semantics without letting a peer push the
                        // encoder outside its envelope (a 1 kbps hint would freeze
                        // our video for every recipient via the min; a huge one
                        // would fail the encoder's configure and drop our camera).
                        let clamped = max_kbps.clamp(
                            *chat::video::RATE_LADDER_KBPS
                                .last()
                                .expect("non-empty ladder"),
                            chat::video::RATE_LADDER_KBPS[0],
                        );
                        inbound_hints.insert(peer.0, clamped);
                        push_effective_rate(
                            &recipients, &inbound_hints, &mut effective_kbps, &control_out,
                        );
                    }
                }
            }
            state = control_in.recv() => {
                let Some(state) = state else { break };
                match state {
                    noded::CallControlIn::Beacon { muted: m, camera_on: c, sharing: s } => {
                        (muted, camera_on, sharing) = (m, c, s);
                        // push immediately so toggles feel live; the 1 Hz
                        // tick keeps late joiners current.
                        send_beacon(&ctl, &recipients, muted, camera_on, sharing).await;
                    }
                    noded::CallControlIn::KeyframeRequest { peer } => {
                        if let Some(lane) = peer_lanes.get_mut(&peer) {
                            request_keyframe_if_due(&ctl, PeerId(peer), lane).await;
                        }
                    }
                }
            }
            _ = ctl_tick.tick() => {
                send_beacon(&ctl, &recipients, muted, camera_on, sharing).await;
                // hints from peers no longer in the roster must not pin our rate.
                let live: HashSet<[u8; 32]> = recipients.borrow().iter().copied().collect();
                inbound_hints.retain(|peer, _| live.contains(peer));
                // a peer who left frees their receive lane too: a rejoiner's new
                // session restarts frame_no at 0, which a retained reassembler
                // (high last_emitted) would reject as Stale forever — and Stale
                // doesn't advance dropped_frames, so no keyframe request self-heals
                // it. Evicting the lane means the rejoiner's next frame builds a
                // fresh reassembler and their tile lights up.
                peer_lanes.retain(|peer, _| live.contains(peer));
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
    sharing: bool,
) {
    let frame = chat::video::CallControl::Beacon {
        muted,
        camera_on,
        sharing,
    }
    .encode();
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
    use data_plane::{PlaneConfig, TransportError};

    /// Per-hub in-memory single-service transport (test only). Production media
    /// rides two OVERLAY sockets, one per service; tests wire two hubs over a
    /// pair of in-memory single-service links per direction, which reproduces
    /// the same per-service isolation (voice frames and video frames never
    /// touch the same channel) without standing up a real overlay stack.
    struct MemLink {
        outbound: mpsc::Sender<(PeerId, Vec<u8>)>,
        inbound: tokio::sync::Mutex<mpsc::Receiver<(PeerId, Vec<u8>)>>,
    }

    impl DataPlaneTransport for MemLink {
        type Stream = tokio::io::DuplexStream;

        async fn send_datagram(&self, to: PeerId, frame: Vec<u8>) -> Result<(), TransportError> {
            // fire-and-forget: a full lane drops the frame, exactly as an
            // overlay UDP send would on buffer pressure.
            let _ = self.outbound.try_send((to, frame));
            Ok(())
        }

        async fn recv_datagram(&self) -> Result<(PeerId, Vec<u8>), TransportError> {
            match self.inbound.lock().await.recv().await {
                Some(framed) => Ok(framed),
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

    /// in-memory link depth: generous so a test never drops on backpressure.
    const LANE: usize = 512;

    fn media_plane(
        outbound: mpsc::Sender<(PeerId, Vec<u8>)>,
        inbound: mpsc::Receiver<(PeerId, Vec<u8>)>,
        flows: Arc<ActiveFlows>,
    ) -> DataPlane<MemLink> {
        DataPlane::new(
            MemLink {
                outbound,
                inbound: tokio::sync::Mutex::new(inbound),
            },
            flows as Arc<dyn AdmissionPolicy>,
            PlaneConfig {
                bulk_bytes_per_sec: 1 << 20,
                bulk_burst_bytes: 1 << 20,
            },
        )
    }

    /// One hub over caller-supplied voice/video links; returns its session
    /// request lane. The inbound halves are fed by the caller (a forwarder
    /// from the peer hub, or a direct injector).
    fn hub_over(
        voice_out: mpsc::Sender<(PeerId, Vec<u8>)>,
        voice_in: mpsc::Receiver<(PeerId, Vec<u8>)>,
        video_out: mpsc::Sender<(PeerId, Vec<u8>)>,
        video_in: mpsc::Receiver<(PeerId, Vec<u8>)>,
    ) -> mpsc::Sender<noded::CallSessionRequest> {
        let flows = Arc::new(ActiveFlows::default());
        let voice_plane = media_plane(voice_out, voice_in, flows.clone());
        let video_plane = media_plane(video_out, video_in, flows.clone());
        let (req_tx, req_rx) = mpsc::channel(4);
        tokio::spawn(serve_sessions(req_rx, voice_plane, video_plane, flows));
        req_tx
    }

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

    /// two hubs wired back-to-back through their per-service links: a frame
    /// sent by one operator's session comes out of the other's mixed playout,
    /// and a replacement session (same hub) tears the first down and still
    /// works — proving flow re-registration after teardown.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn sessions_carry_audio_between_two_hubs_and_survive_replacement() {
        let key_a = [0xaa_u8; 32];
        let key_b = [0xbb_u8; 32];
        let (req_a_tx, req_b_tx) = two_hubs(key_a, key_b, |_| true);

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

    /// wire two hubs A↔B over per-service in-memory links, applying `a_to_b`
    /// to every A→B datagram (it may swallow a frame to model loss). Returns
    /// the two request lanes; keep them alive for the test's duration. Each
    /// forwarder relabels the frame with the SENDER's key (the overlay's
    /// source-`/128` authentication in production) and routes to the peer's
    /// matching-service ingress by the plane header's service byte (`frame[1]`).
    fn two_hubs(
        key_a: [u8; 32],
        key_b: [u8; 32],
        mut a_to_b: impl FnMut(&[u8]) -> bool + Send + 'static,
    ) -> (
        mpsc::Sender<noded::CallSessionRequest>,
        mpsc::Sender<noded::CallSessionRequest>,
    ) {
        let (a_id, b_id) = (PeerId(key_a), PeerId(key_b));
        // each hub's egress lanes and each hub's ingress lanes, per service.
        let (a_voice_out, mut a_voice_out_rx) = mpsc::channel::<(PeerId, Vec<u8>)>(LANE);
        let (a_video_out, mut a_video_out_rx) = mpsc::channel::<(PeerId, Vec<u8>)>(LANE);
        let (b_voice_out, mut b_voice_out_rx) = mpsc::channel::<(PeerId, Vec<u8>)>(LANE);
        let (b_video_out, mut b_video_out_rx) = mpsc::channel::<(PeerId, Vec<u8>)>(LANE);
        let (a_voice_in, a_voice_in_rx) = mpsc::channel::<(PeerId, Vec<u8>)>(LANE);
        let (a_video_in, a_video_in_rx) = mpsc::channel::<(PeerId, Vec<u8>)>(LANE);
        let (b_voice_in, b_voice_in_rx) = mpsc::channel::<(PeerId, Vec<u8>)>(LANE);
        let (b_video_in, b_video_in_rx) = mpsc::channel::<(PeerId, Vec<u8>)>(LANE);

        // A→B: the loss filter guards BOTH of A's outbound lanes, so a per-frame
        // decision (e.g. "drop the first frag_index==1") sees every A→B datagram
        // regardless of service. Merge into one filtered forwarder (single-owner
        // filter) that routes to B's voice/video ingress by the service byte.
        tokio::spawn(async move {
            loop {
                let frame = tokio::select! {
                    Some((_to, f)) = a_voice_out_rx.recv() => f,
                    Some((_to, f)) = a_video_out_rx.recv() => f,
                    else => break,
                };
                if a_to_b(&frame) {
                    let dst = if frame.get(1) == Some(&(Service::Video as u8)) {
                        &b_video_in
                    } else {
                        &b_voice_in
                    };
                    let _ = dst.send((a_id, frame)).await;
                }
            }
        });
        // B→A: unfiltered; route both of B's lanes to A's ingress, stamped B.
        tokio::spawn(async move {
            loop {
                let frame = tokio::select! {
                    Some((_to, f)) = b_voice_out_rx.recv() => f,
                    Some((_to, f)) = b_video_out_rx.recv() => f,
                    else => break,
                };
                let dst = if frame.get(1) == Some(&(Service::Video as u8)) {
                    &a_video_in
                } else {
                    &a_voice_in
                };
                let _ = dst.send((b_id, frame)).await;
            }
        });

        let req_a_tx = hub_over(a_voice_out, a_voice_in_rx, a_video_out, a_video_in_rx);
        let req_b_tx = hub_over(b_voice_out, b_voice_in_rx, b_video_out, b_video_in_rx);
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
        // B lists A too: a real huddle roster is symmetric, and B's 1 Hz ctl
        // tick evicts receive lanes for peers NOT in its recipients — without
        // this, a tick landing mid-keyframe would drop A's in-progress frame.
        session_b
            .recipients
            .send(vec![key_a])
            .expect("session b alive");

        // position-dependent fills (not uniform bytes) so a fragment-ordering
        // or reassembly regression in the hub path shows up — a reordered or
        // duplicated fragment would break exact-equality on the full vector.
        let keyframe_data: Vec<u8> = (0..5000).map(|i| (i % 251) as u8).collect();
        let delta_data: Vec<u8> = (0..5000).map(|i| ((i * 7 + 3) % 251) as u8).collect();

        // a 5000-byte keyframe fragments across ≥4 datagrams.
        session_a
            .video_in
            .send(chat::call_wire::CapturedFrame {
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
            .send(chat::call_wire::CapturedFrame {
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
        // symmetric roster (see video_frames_fragment_and_cross_hubs): keeps
        // B's 1 Hz peer-lane eviction from dropping A's in-progress frame 0.
        session_b
            .recipients
            .send(vec![key_a])
            .expect("session b alive");
        let mut control_a = session_a.control_out;

        // frame 0 loses a fragment (incomplete); frame 1 completes.
        session_a
            .video_in
            .send(chat::call_wire::CapturedFrame {
                keyframe: true,
                ts_ms: 1,
                data: vec![0xA0; 5000],
            })
            .await
            .expect("session a alive");
        session_a
            .video_in
            .send(chat::call_wire::CapturedFrame {
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
        // control flow and A surfaced it to its webview. B also beacons A at
        // 1 Hz (symmetric roster), so skip any interleaved peer state.
        let saw_keyframe_req = async {
            loop {
                match control_a.recv().await {
                    Some(noded::CallControlOut::KeyframeRequest) => break,
                    Some(_) => continue,
                    None => panic!("session a ended before a keyframe request"),
                }
            }
        };
        tokio::time::timeout(Duration::from_secs(10), saw_keyframe_req)
            .await
            .expect("keyframe request must reach A");

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
                sharing: true,
            })
            .await
            .expect("session a alive");

        // B's control_out yields A's beacon as peer state (the 1 Hz tick also
        // repeats it, so a generous timeout is safe). `sharing` must survive the
        // cross-node encode/decode + hub relay.
        let state = loop {
            let msg = tokio::time::timeout(Duration::from_secs(10), session_b.control_out.recv())
                .await
                .expect("beacon must reach B")
                .expect("session b alive");
            if let noded::CallControlOut::PeerBeacon {
                peer,
                muted,
                camera_on,
                sharing,
            } = msg
            {
                break (peer, muted, camera_on, sharing);
            }
        };
        assert_eq!(state, (key_a, false, true, true));

        drop((req_a_tx, req_b_tx));
    }

    /// A peer who leaves the roster and rejoins (a fresh session, `frame_no`
    /// reset to 0) must light back up. Without lane eviction, B's retained
    /// reassembler keeps a high `last_emitted` and rejects every post-rejoin
    /// frame as Stale — forever, with no keyframe-request self-heal.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn peer_lane_evicts_on_departure_so_rejoin_is_not_stale() {
        let key_a = [0xaa_u8; 32];
        let key_b = [0xbb_u8; 32];
        let (req_a_tx, req_b_tx) = two_hubs(key_a, key_b, |_| true);

        let session_a = open(req_a_tx.clone()).await;
        let mut session_b = open(req_b_tx.clone()).await;
        session_a.recipients.send(vec![key_b]).expect("session a alive");
        session_b.recipients.send(vec![key_a]).expect("session b alive");

        // A's first stream: one keyframe crosses and B emits it, so B's
        // peer_lane for A now carries last_emitted = 0.
        let first: Vec<u8> = (0..5000).map(|i| (i % 251) as u8).collect();
        session_a
            .video_in
            .send(chat::call_wire::CapturedFrame {
                keyframe: true,
                ts_ms: 1,
                data: first.clone(),
            })
            .await
            .expect("session a alive");
        let got = tokio::time::timeout(Duration::from_secs(10), session_b.video_out.recv())
            .await
            .expect("first frame must cross")
            .expect("session b alive");
        assert_eq!(got.data, first);

        // A leaves B's roster. B's 1 Hz ctl tick derives `live` from its
        // recipients watch, so with A gone for >1 tick B must evict A's stale
        // lane. Wait out two ticks (generous), then restore the roster.
        session_b.recipients.send(vec![]).expect("session b alive");
        tokio::time::sleep(Duration::from_millis(2200)).await;
        session_b.recipients.send(vec![key_a]).expect("session b alive");

        // A rejoins with a FRESH session on the same hub: teardown + reopen
        // resets frame_no to 0.
        let session_a2 = open(req_a_tx.clone()).await;
        session_a2
            .recipients
            .send(vec![key_b])
            .expect("session a2 alive");

        let rejoined: Vec<u8> = (0..5000).map(|i| ((i * 3 + 1) % 251) as u8).collect();
        session_a2
            .video_in
            .send(chat::call_wire::CapturedFrame {
                keyframe: true,
                ts_ms: 2,
                data: rejoined.clone(),
            })
            .await
            .expect("session a2 alive");
        // Without the eviction fix, B rejects frame_no 0 as Stale and never
        // emits — this recv times out. With it, a fresh reassembler completes.
        let got = tokio::time::timeout(Duration::from_secs(10), session_b.video_out.recv())
            .await
            .expect("rejoined frame must cross — B must evict A's lane on departure")
            .expect("session b alive");
        assert_eq!(
            got.data, rejoined,
            "B must emit A's post-rejoin frame, not reject it as stale"
        );

        drop((req_a_tx, req_b_tx));
    }

    /// pull the next RateHint the hub forwards to its local encoder, skipping
    /// any interleaved beacons/keyframe requests.
    async fn next_rate_hint(control_out: &mut mpsc::Receiver<noded::CallControlOut>) -> u32 {
        loop {
            match control_out.recv().await {
                Some(noded::CallControlOut::RateHint { max_kbps }) => return max_kbps,
                Some(_) => continue,
                None => panic!("session ended before a rate hint"),
            }
        }
    }

    /// A hostile peer's out-of-ladder RateHint must be clamped into the ladder
    /// envelope before it reaches our encoder: a floor of 1 kbps (would freeze
    /// our video via min semantics) clamps up to 300, a ceiling of 4e9 (would
    /// blow the encoder configure) clamps down to 1200.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn rate_hint_is_clamped_to_the_ladder() {
        let peer = [0xcc_u8; 32];
        // a lone hub whose voice-plane ingress we drive directly, as a peer
        // would over the overlay. hold the egress receivers so sends don't
        // fail closed.
        let (voice_out, _voice_out_rx) = mpsc::channel::<(PeerId, Vec<u8>)>(LANE);
        let (voice_in, voice_in_rx) = mpsc::channel::<(PeerId, Vec<u8>)>(LANE);
        let (video_out, _video_out_rx) = mpsc::channel::<(PeerId, Vec<u8>)>(LANE);
        let (_video_in, video_in_rx) = mpsc::channel::<(PeerId, Vec<u8>)>(LANE);
        let req_tx = hub_over(voice_out, voice_in_rx, video_out, video_in_rx);

        let session = open(req_tx.clone()).await;
        session.recipients.send(vec![peer]).expect("session alive");
        let mut control_out = session.control_out;

        // hand-craft a control datagram on the session's ctl flow, exactly as a
        // hostile peer could inject over the overlay.
        let flow = ctl_flow("general");
        let inject = |max_kbps: u32| {
            data_plane::wire::encode_datagram(
                Service::Voice,
                flow,
                &chat::video::CallControl::RateHint { max_kbps }.encode(),
            )
            .expect("datagram encodes")
        };

        voice_in
            .send((PeerId(peer), inject(1)))
            .await
            .expect("inbound alive");
        let hint = tokio::time::timeout(Duration::from_secs(5), next_rate_hint(&mut control_out))
            .await
            .expect("a rate hint must reach the encoder");
        assert_eq!(hint, 300, "a 1 kbps hint must clamp up to the ladder bottom");

        voice_in
            .send((PeerId(peer), inject(4_000_000_000)))
            .await
            .expect("inbound alive");
        let hint = tokio::time::timeout(Duration::from_secs(5), next_rate_hint(&mut control_out))
            .await
            .expect("a rate hint must reach the encoder");
        assert_eq!(hint, 1200, "a 4e9 kbps hint must clamp down to the ladder top");

        drop(req_tx);
    }

    /// The dropout this whole cutover fixes, stated as an invariant. On the
    /// retired mesh arm, every channel to a peer funnelled through ONE bounded
    /// per-peer send queue, so a multi-megabit video flood filled it and the
    /// sparse 32 kbps voice stream was dropped behind it — audio out for the
    /// length of the congestion. The overlay arm binds a SEPARATE socket per
    /// service, so voice and video never share a queue. Modelled with a bounded
    /// queue apiece: a video flood that saturates the shared queue starves
    /// audio, but the same flood on an isolated video queue leaves voice's
    /// queue free. A future change that re-merges media onto one queue breaks
    /// this — that is the point.
    #[test]
    fn per_service_isolation_keeps_a_video_flood_from_starving_audio() {
        // one peer's send backlog, sized like the mesh relay's (MAX_BACKLOG).
        const CAP: usize = 128;
        let video_frame = || vec![0xDDu8; 200];
        let voice_frame = || vec![0xAAu8; 80];

        // shared arm (the retired mesh): both services on ONE queue. A video
        // flood saturates it; no voice frame can enqueue behind the backlog.
        let (shared, _shared_rx) = mpsc::channel::<Vec<u8>>(CAP);
        while shared.try_send(video_frame()).is_ok() {} // flood to saturation
        let voice_through_shared = (0..CAP)
            .take_while(|_| shared.try_send(voice_frame()).is_ok())
            .count();
        assert_eq!(
            voice_through_shared, 0,
            "a video flood saturating a SHARED queue starves audio — the mesh bug"
        );

        // overlay arm (this cutover): a queue per service. The same video flood
        // saturates only the video queue; voice's queue is untouched.
        let (voice_q, _voice_rx) = mpsc::channel::<Vec<u8>>(CAP);
        let (video_q, _video_rx) = mpsc::channel::<Vec<u8>>(CAP);
        while video_q.try_send(video_frame()).is_ok() {} // saturate video only
        let voice_through_isolated = (0..CAP)
            .take_while(|_| voice_q.try_send(voice_frame()).is_ok())
            .count();
        assert_eq!(
            voice_through_isolated, CAP,
            "per-service isolation keeps voice flowing under a video flood — the fix"
        );
    }
}

/// Headless end-to-end proof that huddle audio crosses the REAL userspace
/// WireGuard overlay — no TUN, no root, no mics, no GUI (the
/// `crates/system/overlay-net/tests/loopback_pair` harness shape). Two voice
/// hubs run on their OWN runtimes (`spawn_hub`) and bind the per-service
/// overlay sockets over two loopback-peered virtual stacks; audio fed into one
/// comes out the other, Opus-decoded. Unlike the in-memory tests above, this
/// exercises the exact production runtime topology — hub runtime + stack
/// runtime + cross-runtime socket driving + a real WireGuard tunnel — which is
/// the one thing unit-level transports cannot cover.
#[cfg(test)]
mod overlay_e2e {
    use std::net::{IpAddr, Ipv6Addr, SocketAddr};

    use commonware_cryptography::{Signer as _, ed25519};
    use defguard_wireguard_rs::{InterfaceConfiguration, key::Key, net::IpAddrMask, peer::Peer};
    use overlay_net::userspace::{UserspaceWireGuardEffect, VirtualSocketFactory};
    use wireguard::effect::WireGuardEffect;

    use super::*;

    /// a fixture chain namespace — both hubs derive every member's overlay
    /// `/128` from it, exactly as production derives it from the chain id.
    const NS: &str = "e2e-huddle-overlay";

    /// one overlay node: the WireGuard effect (the only handle we drive), its
    /// ed25519 identity (which fixes its overlay `/128`), and the loopback
    /// underlay endpoint of its bound WG socket.
    struct OverlayNode {
        effect: UserspaceWireGuardEffect,
        wg_secret: Key,
        node_key: ed25519::PublicKey,
        raw_key: [u8; 32],
        ula: Ipv6Addr,
        endpoint: SocketAddr,
    }

    /// any 32 bytes are a valid X25519 secret (the curve clamps); each node
    /// needs a distinct one.
    fn wg_secret(seed: u8) -> Key {
        let mut bytes = [seed; 32];
        bytes[0] = seed.wrapping_add(1);
        Key::new(bytes)
    }

    fn config(node: &OverlayNode, port: u16, peers: Vec<Peer>) -> InterfaceConfiguration {
        InterfaceConfiguration {
            name: "dt-huddle".into(),
            prvkey: node.wg_secret.to_string(),
            addresses: vec![IpAddrMask::new(IpAddr::V6(node.ula), 128)],
            port,
            peers,
            mtu: None,
            fwmark: None,
        }
    }

    fn peer_entry(of: &OverlayNode, endpoint: Option<SocketAddr>) -> Peer {
        let mut peer = Peer::new(of.wg_secret.public_key());
        peer.endpoint = endpoint;
        peer.set_allowed_ips(vec![IpAddrMask::new(IpAddr::V6(of.ula), 128)]);
        peer
    }

    /// stand a node up: its overlay `/128` is `ula_v6_member_addr(NS, key)` —
    /// the SAME function the media `AddressBook` resolves peers by, so the two
    /// ends agree with no coordination. first apply is empty-peer/port-0 so the
    /// OS allocates the underlay port before the peered re-apply.
    fn stand_up(node_seed: u64, wg_seed: u8) -> OverlayNode {
        let node_key = ed25519::PrivateKey::from_seed(node_seed).public_key();
        let raw_key: [u8; 32] = node_key.as_ref().try_into().expect("ed25519 is 32 bytes");
        let ula = wireguard::ula_v6_member_addr(
            NS,
            wireguard::ValidatorIdentity(raw_key),
        );
        let mut node = OverlayNode {
            effect: UserspaceWireGuardEffect::new(tokio::runtime::Handle::current()),
            wg_secret: wg_secret(wg_seed),
            node_key,
            raw_key,
            ula,
            endpoint: SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0),
        };
        node.effect.create_interface().expect("create interface");
        node.effect
            .apply(&config(&node, 0, Vec::new()))
            .expect("first apply binds the underlay");
        let bound = node.effect.local_underlay_addr().expect("underlay bound");
        node.endpoint.set_port(bound.port());
        node
    }

    /// peer `a`↔`b`: `a` knows `b`'s endpoint, `b` learns `a`'s from the first
    /// authenticated inbound datagram (the zero-config joiner shape).
    fn peer_up(a: &mut OverlayNode, b: &mut OverlayNode) {
        let (a_port, b_port) = (a.endpoint.port(), b.endpoint.port());
        a.effect
            .apply(&config(a, a_port, vec![peer_entry(b, Some(b.endpoint))]))
            .expect("peered re-apply on a");
        b.effect
            .apply(&config(b, b_port, vec![peer_entry(a, None)]))
            .expect("peered re-apply on b");
    }

    /// the media peer set both hubs track: both members, so each resolves the
    /// other's `/128` (forward) and authenticates its source (reverse).
    fn media_peers(nodes: &[&OverlayNode]) -> Arc<MediaPeers> {
        let peers = MediaPeers::new(NS.to_string());
        peers.set_peers(nodes.iter().map(|n| &n.node_key));
        peers
    }

    /// spawn a hub over a node's overlay stack (its OWN runtime binds the media
    /// sockets; the stack keeps polling on this test's runtime).
    fn spawn_over(
        node: &OverlayNode,
        peers: Arc<MediaPeers>,
    ) -> mpsc::Sender<noded::CallSessionRequest> {
        let (req_tx, req_rx) = mpsc::channel(4);
        let factory: Arc<dyn SocketFactory> =
            Arc::new(VirtualSocketFactory::new(node.effect.stack_slot()));
        spawn_hub(
            req_rx,
            factory,
            peers,
            node.raw_key,
            data_plane::PlaneMonitor::default(),
        );
        req_tx
    }

    async fn open(lane: &mpsc::Sender<noded::CallSessionRequest>) -> noded::CallSession {
        let (reply, opened) = tokio::sync::oneshot::channel();
        lane.send(noded::CallSessionRequest {
            channel_id: "general".into(),
            reply,
        })
        .await
        .expect("hub alive");
        opened.await.expect("hub replies").expect("session opens")
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn audio_crosses_a_real_overlay_between_two_hubs() {
        let mut a = stand_up(1, 0x11);
        let mut b = stand_up(2, 0x22);
        peer_up(&mut a, &mut b);

        let req_a = spawn_over(&a, media_peers(&[&a, &b]));
        let req_b = spawn_over(&b, media_peers(&[&a, &b]));

        let session_a = open(&req_a).await;
        let mut session_b = open(&req_b).await;
        session_a
            .recipients
            .send(vec![b.raw_key])
            .expect("session a alive");
        session_b
            .recipients
            .send(vec![a.raw_key])
            .expect("session b alive");

        // loud constant audio from A must surface as energy in B's mixed
        // playout, having crossed: A's hub runtime → its overlay socket → the
        // WireGuard tunnel → B's overlay socket → B's jitter buffer → Opus
        // decode → mix. A generous deadline covers the handshake + the bind
        // retry loop.
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
        tokio::time::timeout(Duration::from_secs(30), heard)
            .await
            .expect("audio must cross the real overlay between the two hubs");
    }
}
