//! The huddle's media leg: the `/v1/call/ws` client the node's call hub has
//! been serving since the webview era — mic capture in, mixed playout out,
//! call control as json text frames. Binary framing is `chat::call_wire`, the
//! single definition site; the control json mirrors `noded`'s
//! `CallClientControl`/`CallServerControl` (tag = `type`, snake_case) — the
//! app does not link the daemon crate, so the three-variant shapes are
//! restated here and drift is a wire break the e2e lane would catch.
//!
//! LIFECYCLE IS THE SUBSCRIPTION'S. `call_session` is a `stream` extern the
//! app's one subscribe block runs `when (huddle_joined && connected)`: joining
//! starts the session, leaving (or disconnecting) drops the stream, and every
//! resource follows that drop — the pump task exits when the event channel
//! closes, the websocket closes with the task, and the audio thread drops the
//! cpal streams when its shutdown sender goes with the pump. No imperative
//! stop, nothing to leak.
//!
//! AUDIO THREADING: cpal streams are not `Send`, so they live on one
//! dedicated OS thread that builds input+output and parks on a shutdown
//! channel. Capture callbacks push mono i16 into a frame accumulator and hand
//! full 20 ms frames (`chat::voice::FRAME_SAMPLES`) to the pump over an
//! unbounded channel; playout callbacks drain a shared ring the pump fills
//! from `mixed` frames. Late audio is dead audio: the ring caps at ~200 ms
//! and drops oldest, capture frames drop when the pump is behind.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use chat::call_wire;
use chat::call_wire::CapturedFrame;
use chat::voice::FRAME_SAMPLES;
use iced::futures::stream::BoxStream;
use iced::futures::{SinkExt as _, StreamExt as _};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::tungstenite::Message as WsMessage;

/// One call-session event, flattened for the Ice route: `kind` picks the arm
/// (`connecting` | `live` | `refused` | `closed` | `error` | `peer`),
/// `message` carries refusal/error prose, the rest is a peer beacon.
#[derive(Clone, Debug, Hash, PartialEq, Default)]
pub struct CallEvent {
    pub kind: String,
    pub message: String,
    pub peer: String,
    pub muted: bool,
    pub camera_on: bool,
    pub sharing: bool,
}

impl CallEvent {
    fn of(kind: &str) -> Self {
        Self {
            kind: kind.to_owned(),
            ..Self::default()
        }
    }

    fn failed(kind: &str, message: impl Into<String>) -> Self {
        Self {
            kind: kind.to_owned(),
            message: message.into(),
            ..Self::default()
        }
    }
}

/// Client → hub control, mirroring `noded::CallClientControl`.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientControl {
    Recipients { peers: Vec<String> },
    Beacon {
        muted: bool,
        camera_on: bool,
        sharing: bool,
    },
}

/// Hub → client control, mirroring `noded::CallServerControl`.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ServerControl {
    KeyframeRequest,
    PeerBeacon {
        peer: String,
        muted: bool,
        camera_on: bool,
        sharing: bool,
    },
    RateHint {
        #[allow(dead_code)]
        max_kbps: u32,
    },
}

/// The live session's steerable ends, parked globally so the flat `sync`
/// externs (mute, recipients) can reach the running pump. One session at a
/// time — the subscribe gate guarantees it.
struct Handles {
    muted: Arc<AtomicBool>,
    control: tokio::sync::mpsc::UnboundedSender<ClientControl>,
}

fn handles() -> &'static Mutex<Option<Handles>> {
    static HANDLES: OnceLock<Mutex<Option<Handles>>> = OnceLock::new();
    HANDLES.get_or_init(|| Mutex::new(None))
}

/// Toggle the mic. Applies to the running session (capture frames stop while
/// muted) and beacons the new state to peers; the return value is the state
/// the view should show.
pub fn call_set_muted(muted: bool) -> bool {
    if let Some(handles) = handles().lock().expect("call handles").as_ref() {
        handles.muted.store(muted, Ordering::Relaxed);
    }
    beacon_state();
    muted
}

/// Beacon the CURRENT local state (mute + camera) to peers — the one place
/// the beacon is assembled, called by both toggles and the session open.
pub(crate) fn beacon_state() {
    let guard = handles().lock().expect("call handles");
    let Some(handles) = guard.as_ref() else {
        return;
    };
    let _ = handles.control.send(ClientControl::Beacon {
        muted: handles.muted.load(Ordering::Relaxed),
        camera_on: crate::video::camera_enabled(),
        sharing: false,
    });
}

/// Steer the fan-out set to the huddle roster's peer NODE keys (self
/// excluded). Called wherever the roster refreshes; a no-session call is a
/// no-op `false`.
pub fn call_recipients(nodes: Vec<String>) -> bool {
    let guard = handles().lock().expect("call handles");
    let Some(handles) = guard.as_ref() else {
        return false;
    };
    handles
        .control
        .send(ClientControl::Recipients { peers: nodes })
        .is_ok()
}

/// The session stream: connect, pump, and yield state the handlers fold. The
/// stream owns everything — see the module doc's lifecycle note.
pub fn call_session(rpc: String, channel_id: String) -> BoxStream<'static, CallEvent> {
    let (events_tx, events_rx) = iced::futures::channel::mpsc::unbounded();
    tokio::spawn(run_session(rpc, channel_id, events_tx));
    Box::pin(events_rx)
}

fn ws_url(rpc: &str, channel_id: &str) -> String {
    let base = rpc.trim_end_matches('/');
    let base = base
        .replacen("https://", "wss://", 1)
        .replacen("http://", "ws://", 1);
    format!("{base}/v1/call/ws?channel={channel_id}")
}

async fn run_session(
    rpc: String,
    channel_id: String,
    mut events: iced::futures::channel::mpsc::UnboundedSender<CallEvent>,
) {
    let _ = events.send(CallEvent::of("connecting")).await;
    let url = ws_url(&rpc, &channel_id);
    let connected = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio_tungstenite::connect_async(&url),
    )
    .await;
    let (socket, _) = match connected {
        Ok(Ok(pair)) => pair,
        Ok(Err(error)) => {
            let _ = events
                .send(CallEvent::failed("error", format!("call socket: {error}")))
                .await;
            return;
        }
        Err(_) => {
            let _ = events
                .send(CallEvent::failed("error", "call socket: connection timed out"))
                .await;
            return;
        }
    };
    let (mut ws_out, mut ws_in) = socket.split();

    let muted = Arc::new(AtomicBool::new(false));
    let (control_tx, mut control_rx) = tokio::sync::mpsc::unbounded_channel::<ClientControl>();
    let (mic_tx, mut mic_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<i16>>();
    let playout = Arc::new(Mutex::new(PlayoutRing::default()));

    let audio = AudioThread::start(muted.clone(), mic_tx, playout.clone());
    let audio_note = audio.note.clone();

    // The camera leg (crate::video): its thread mirrors the audio thread's
    // ownership rules and dies with the same teardown chain.
    let (video_tx, mut video_rx) = tokio::sync::mpsc::unbounded_channel::<CapturedFrame>();
    let _video_keepalive = video_tx.clone();
    let (camera_shutdown_tx, camera_shutdown_rx) = std::sync::mpsc::channel::<()>();
    let camera_events = events.clone();
    let camera_thread = std::thread::Builder::new()
        .name("huddle-camera".into())
        .spawn(move || crate::video::camera_thread(video_tx, camera_shutdown_rx, camera_events))
        .ok();

    *handles().lock().expect("call handles") = Some(Handles {
        muted: muted.clone(),
        control: control_tx.clone(),
    });

    // The hub beacons our state at 1 Hz on our behalf; one push seeds it.
    beacon_state();
    let mut live = CallEvent::of("live");
    live.message = audio_note.lock().expect("audio note").clone();
    let _ = events.send(live).await;

    loop {
        tokio::select! {
            inbound = ws_in.next() => match inbound {
                Some(Ok(WsMessage::Binary(bytes))) => {
                    if let Some(frame) = call_wire::decode_audio(&bytes) {
                        playout.lock().expect("playout ring").push_frame(&frame);
                    } else if let Some(frame) = call_wire::decode_peer(&bytes) {
                        // JPEG decode is ~1–3 ms — off the pump, and dropped
                        // frames are free (the next one is a keyframe too).
                        tokio::task::spawn_blocking(move || crate::video::store_peer_frame(frame));
                    }
                }
                Some(Ok(WsMessage::Text(text))) => {
                    match serde_json::from_str::<ServerControl>(&text) {
                        Ok(ServerControl::PeerBeacon { peer, muted, camera_on, sharing }) => {
                            let event = CallEvent {
                                kind: "peer".into(),
                                message: String::new(),
                                peer,
                                muted,
                                camera_on,
                                sharing,
                            };
                            if events.send(event).await.is_err() {
                                break;
                            }
                        }
                        Ok(ServerControl::KeyframeRequest | ServerControl::RateHint { .. }) => {}
                        // Any non-control text frame is the hub's refusal
                        // prose, sent once before it closes the socket.
                        Err(_) => {
                            let _ = events.send(CallEvent::failed("refused", text.to_string())).await;
                            break;
                        }
                    }
                }
                Some(Ok(_)) => {}
                Some(Err(error)) => {
                    let _ = events
                        .send(CallEvent::failed("error", format!("call socket: {error}")))
                        .await;
                    break;
                }
                None => {
                    let _ = events.send(CallEvent::of("closed")).await;
                    break;
                }
            },
            frame = mic_rx.recv() => match frame {
                Some(frame) => {
                    let encoded = call_wire::encode_audio(&frame);
                    if ws_out.send(WsMessage::Binary(encoded)).await.is_err() {
                        let _ = events.send(CallEvent::of("closed")).await;
                        break;
                    }
                }
                None => break,
            },
            frame = video_rx.recv() => match frame {
                Some(frame) => {
                    let encoded = call_wire::encode_captured(&frame);
                    if ws_out.send(WsMessage::Binary(encoded)).await.is_err() {
                        let _ = events.send(CallEvent::of("closed")).await;
                        break;
                    }
                }
                None => break,
            },
            control = control_rx.recv() => match control {
                Some(control) => {
                    let Ok(text) = serde_json::to_string(&control) else { continue };
                    if ws_out.send(WsMessage::Text(text)).await.is_err() {
                        let _ = events.send(CallEvent::of("closed")).await;
                        break;
                    }
                }
                None => break,
            },
        }
        // The subscription dropped the stream — the session is over.
        if events.is_closed() {
            break;
        }
    }

    *handles().lock().expect("call handles") = None;
    crate::video::reset();
    drop(camera_shutdown_tx);
    if let Some(thread) = camera_thread {
        let _ = thread.join();
    }
    drop(audio);
}

// ============================================================================
// audio — one OS thread owns the cpal streams (they are not Send)
// ============================================================================

/// The playout ring: mixed 20 ms frames in, device-rate samples out. Caps at
/// ~200 ms and drops oldest — late audio is dead audio.
#[derive(Default)]
struct PlayoutRing {
    samples: VecDeque<i16>,
}

const PLAYOUT_CAP: usize = FRAME_SAMPLES * 10;

impl PlayoutRing {
    fn push_frame(&mut self, frame: &[i16]) {
        self.samples.extend(frame);
        while self.samples.len() > PLAYOUT_CAP {
            self.samples.pop_front();
        }
    }

    fn drain_into(&mut self, out: &mut [i16]) {
        for slot in out.iter_mut() {
            *slot = self.samples.pop_front().unwrap_or(0);
        }
    }
}

/// Accumulates mono 48 kHz i16 samples into exact voice frames.
#[derive(Default)]
pub struct FrameAccumulator {
    buffer: Vec<i16>,
}

impl FrameAccumulator {
    pub fn push(&mut self, samples: impl IntoIterator<Item = i16>) -> Vec<Vec<i16>> {
        self.buffer.extend(samples);
        let mut frames = Vec::new();
        while self.buffer.len() >= FRAME_SAMPLES {
            frames.push(self.buffer.drain(..FRAME_SAMPLES).collect());
        }
        frames
    }
}

/// Fold an interleaved buffer to mono i16: channels average per sample tick.
pub fn interleaved_to_mono(samples: &[i16], channels: usize) -> Vec<i16> {
    if channels <= 1 {
        return samples.to_vec();
    }
    samples
        .chunks_exact(channels)
        .map(|tick| {
            let sum: i32 = tick.iter().map(|sample| i32::from(*sample)).sum();
            (sum / tick.len() as i32) as i16
        })
        .collect()
}

/// f32 sample to i16, clamped.
pub fn f32_to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * 32767.0) as i16
}

/// A linear resampler from `from_rate` to 48 kHz mono.
// ponytail: linear interpolation, fine for voice; swap for a windowed-sinc
// resampler if capture quality ever matters more than simplicity.
pub struct Resampler {
    step: f64,
    phase: f64,
    last: i16,
}

impl Resampler {
    pub fn new(from_rate: u32) -> Self {
        Self {
            step: f64::from(from_rate) / 48_000.0,
            phase: 0.0,
            last: 0,
        }
    }

    pub fn push(&mut self, input: &[i16]) -> Vec<i16> {
        if (self.step - 1.0).abs() < f64::EPSILON {
            if let Some(last) = input.last() {
                self.last = *last;
            }
            return input.to_vec();
        }
        let mut output = Vec::with_capacity((input.len() as f64 / self.step) as usize + 2);
        for sample in input {
            // Emit every 48 kHz tick that lands before this input sample.
            while self.phase < 1.0 {
                let mixed = f64::from(self.last) * (1.0 - self.phase)
                    + f64::from(*sample) * self.phase;
                output.push(mixed as i16);
                self.phase += self.step;
            }
            self.phase -= 1.0;
            self.last = *sample;
        }
        output
    }
}

/// The audio thread's owner: dropping it signals shutdown and joins.
struct AudioThread {
    shutdown: Option<std::sync::mpsc::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
    /// What the audio layer wants the session surface to say: empty when both
    /// devices opened, otherwise a short "mic unavailable"-class note.
    note: Arc<Mutex<String>>,
}

impl AudioThread {
    fn start(
        muted: Arc<AtomicBool>,
        mic: tokio::sync::mpsc::UnboundedSender<Vec<i16>>,
        playout: Arc<Mutex<PlayoutRing>>,
    ) -> Self {
        let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel::<()>();
        let note = Arc::new(Mutex::new(String::new()));
        let thread_note = note.clone();
        let thread = std::thread::Builder::new()
            .name("huddle-audio".into())
            .spawn(move || audio_thread(muted, mic, playout, shutdown_rx, thread_note))
            .ok();
        Self {
            shutdown: Some(shutdown_tx),
            thread,
            note,
        }
    }
}

impl Drop for AudioThread {
    fn drop(&mut self) {
        self.shutdown.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn audio_thread(
    muted: Arc<AtomicBool>,
    mic: tokio::sync::mpsc::UnboundedSender<Vec<i16>>,
    playout: Arc<Mutex<PlayoutRing>>,
    shutdown: std::sync::mpsc::Receiver<()>,
    note: Arc<Mutex<String>>,
) {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    let host = cpal::default_host();
    let mut notes = Vec::new();

    // The pump's mic arm must stay pending — never closed — in listen-only
    // sessions: this keepalive holds the channel open even when no input
    // device exists, so a missing microphone degrades instead of ending the
    // session.
    let mic_keepalive = mic.clone();

    let input_stream = host.default_input_device().and_then(|device| {
        let mic = mic.clone();
        let config = device.default_input_config().ok()?;
        let channels = config.channels() as usize;
        let rate = config.sample_rate().0;
        let mut accumulator = FrameAccumulator::default();
        let mut resampler = Resampler::new(rate);
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => device.build_input_stream(
                &config.into(),
                move |data: &[f32], _| {
                    if muted.load(Ordering::Relaxed) {
                        return;
                    }
                    let ints: Vec<i16> = data.iter().copied().map(f32_to_i16).collect();
                    let mono = interleaved_to_mono(&ints, channels);
                    for frame in accumulator.push(resampler.push(&mono)) {
                        let _ = mic.send(frame);
                    }
                },
                |_| {},
                None,
            ),
            cpal::SampleFormat::I16 => device.build_input_stream(
                &config.into(),
                move |data: &[i16], _| {
                    if muted.load(Ordering::Relaxed) {
                        return;
                    }
                    let mono = interleaved_to_mono(data, channels);
                    for frame in accumulator.push(resampler.push(&mono)) {
                        let _ = mic.send(frame);
                    }
                },
                |_| {},
                None,
            ),
            _ => return None,
        };
        let stream = stream.ok()?;
        stream.play().ok()?;
        Some(stream)
    });
    if input_stream.is_none() {
        notes.push("no microphone");
    }

    let output_stream = host.default_output_device().and_then(|device| {
        let config = device.default_output_config().ok()?;
        let channels = config.channels() as usize;
        let rate = config.sample_rate().0;
        // The hub mixes at 48 kHz; a device at another rate gets the nearest
        // sample (playout quality follows the ponytail note on Resampler).
        let step = 48_000.0 / f64::from(rate);
        let ring = playout;
        let mut phase = 0.0_f64;
        let mut current = 0i16;
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => device.build_output_stream(
                &config.into(),
                move |data: &mut [f32], _| {
                    let ticks = data.len() / channels.max(1);
                    let mut mono = vec![0i16; ((ticks as f64) * step).ceil() as usize];
                    ring.lock().expect("playout ring").drain_into(&mut mono);
                    let mut source = mono.into_iter();
                    for tick in data.chunks_exact_mut(channels.max(1)) {
                        phase += step;
                        while phase >= 1.0 {
                            current = source.next().unwrap_or(0);
                            phase -= 1.0;
                        }
                        let value = f32::from(current) / 32768.0;
                        for slot in tick {
                            *slot = value;
                        }
                    }
                },
                |_| {},
                None,
            ),
            cpal::SampleFormat::I16 => device.build_output_stream(
                &config.into(),
                move |data: &mut [i16], _| {
                    let ticks = data.len() / channels.max(1);
                    let mut mono = vec![0i16; ((ticks as f64) * step).ceil() as usize];
                    ring.lock().expect("playout ring").drain_into(&mut mono);
                    let mut source = mono.into_iter();
                    for tick in data.chunks_exact_mut(channels.max(1)) {
                        phase += step;
                        while phase >= 1.0 {
                            current = source.next().unwrap_or(0);
                            phase -= 1.0;
                        }
                        for slot in tick {
                            *slot = current;
                        }
                    }
                },
                |_| {},
                None,
            ),
            _ => return None,
        };
        let stream = stream.ok()?;
        stream.play().ok()?;
        Some(stream)
    });
    if output_stream.is_none() {
        notes.push("no speaker");
    }

    *note.lock().expect("audio note") = notes.join(" · ");

    // Park until the session drops the sender; the streams die with the frame.
    let _ = shutdown.recv();
    drop(mic_keepalive);
    drop(input_stream);
    drop(output_stream);
}

// ============================================================================
// state folds — the flat handlers' arms live here
// ============================================================================

/// The status line after `event`: connecting → live (with the audio note
/// folded in) → refused/error prose → closed.
pub fn call_status_after(current: String, event: CallEvent) -> String {
    match event.kind.as_str() {
        "connecting" => "connecting".into(),
        "live" if event.message.is_empty() => "live".into(),
        "live" => format!("live · {}", event.message),
        "refused" | "error" => event.message,
        "closed" => "closed".into(),
        _ => current,
    }
}

/// One peer beacon folded into the presence list, keyed by node key. A
/// session's end (closed/refused/error) clears it — stale badges on the next
/// session's tiles would be someone else's state.
pub fn apply_call_peer(peers: Vec<CallEvent>, event: CallEvent) -> Vec<CallEvent> {
    match event.kind.as_str() {
        "peer" => {
            let mut peers: Vec<CallEvent> = peers
                .into_iter()
                .filter(|peer| peer.peer != event.peer)
                .collect();
            peers.push(event);
            peers
        }
        "closed" | "refused" | "error" => Vec::new(),
        _ => peers,
    }
}

/// Any live camera in the call — the local one or any peer beaconing
/// `camera_on` — gates the tile strip and its repaint tick.
pub fn call_video_live_after(peers: Vec<CallEvent>, camera: bool) -> bool {
    camera || peers.iter().any(|peer| peer.camera_on)
}

/// Whether the roster row at `node` is currently muted, per the beacons.
pub fn call_peer_muted(peers: Vec<CallEvent>, node: String) -> bool {
    peers
        .iter()
        .any(|peer| peer.peer == node && peer.muted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_accumulate_to_exact_voice_frames() {
        let mut accumulator = FrameAccumulator::default();
        assert!(accumulator.push(vec![1i16; FRAME_SAMPLES - 1]).is_empty());
        let frames = accumulator.push(vec![2i16; FRAME_SAMPLES + 1]);
        assert_eq!(frames.len(), 2);
        assert!(frames.iter().all(|frame| frame.len() == FRAME_SAMPLES));
        // The tail stays buffered for the next callback.
        let frames = accumulator.push(vec![3i16; FRAME_SAMPLES]);
        assert_eq!(frames.len(), 1);
    }

    #[test]
    fn interleaved_folds_to_mono_by_average() {
        assert_eq!(interleaved_to_mono(&[10, 20, 30, 50], 2), vec![15, 40]);
        assert_eq!(interleaved_to_mono(&[7, 8, 9], 1), vec![7, 8, 9]);
    }

    #[test]
    fn resampler_identity_and_ratio() {
        let mut same = Resampler::new(48_000);
        assert_eq!(same.push(&[1, 2, 3]), vec![1, 2, 3]);

        let mut up = Resampler::new(24_000);
        let out = up.push(&[100; 240]);
        // 24k → 48k roughly doubles the sample count.
        assert!((470..=490).contains(&out.len()), "got {}", out.len());

        let mut down = Resampler::new(96_000);
        let out = down.push(&[100; 960]);
        assert!((470..=490).contains(&out.len()), "got {}", out.len());
    }

    #[test]
    fn playout_ring_caps_and_zero_fills() {
        let mut ring = PlayoutRing::default();
        ring.push_frame(&[5i16; FRAME_SAMPLES * 12]);
        assert_eq!(ring.samples.len(), PLAYOUT_CAP);
        let mut out = [1i16; 4];
        let mut empty = PlayoutRing::default();
        empty.drain_into(&mut out);
        assert_eq!(out, [0i16; 4]);
    }

    #[test]
    fn status_and_peer_folds() {
        assert_eq!(
            call_status_after("".into(), CallEvent::of("connecting")),
            "connecting"
        );
        assert_eq!(call_status_after("x".into(), CallEvent::of("live")), "live");
        let mut live = CallEvent::of("live");
        live.message = "no microphone".into();
        assert_eq!(
            call_status_after("x".into(), live),
            "live · no microphone"
        );
        assert_eq!(
            call_status_after("live".into(), CallEvent::failed("refused", "nope")),
            "nope"
        );

        let beacon = |peer: &str, muted: bool| CallEvent {
            kind: "peer".into(),
            peer: peer.into(),
            muted,
            ..CallEvent::default()
        };
        let peers = apply_call_peer(Vec::new(), beacon("aa", true));
        let peers = apply_call_peer(peers, beacon("bb", false));
        let peers = apply_call_peer(peers, beacon("aa", false));
        assert_eq!(peers.len(), 2);
        assert!(!call_peer_muted(peers.clone(), "aa".into()));
        assert!(!call_peer_muted(peers, "cc".into()));
    }

    #[test]
    fn control_json_is_the_daemon_wire_verbatim() {
        // These literals ARE the `/v1/call/ws` text-frame contract
        // (`noded::CallClientControl` / `CallServerControl`); this pin is
        // what catches a serde-attribute drift on either side.
        assert_eq!(
            serde_json::to_string(&ClientControl::Recipients {
                peers: vec!["aa".into()]
            })
            .unwrap(),
            r#"{"type":"recipients","peers":["aa"]}"#
        );
        assert_eq!(
            serde_json::to_string(&ClientControl::Beacon {
                muted: true,
                camera_on: false,
                sharing: false
            })
            .unwrap(),
            r#"{"type":"beacon","muted":true,"camera_on":false,"sharing":false}"#
        );
        let beacon: ServerControl = serde_json::from_str(
            r#"{"type":"peer_beacon","peer":"bb","muted":false,"camera_on":true,"sharing":false}"#,
        )
        .unwrap();
        assert!(matches!(
            beacon,
            ServerControl::PeerBeacon { camera_on: true, .. }
        ));
        assert!(matches!(
            serde_json::from_str::<ServerControl>(r#"{"type":"rate_hint","max_kbps":900}"#)
                .unwrap(),
            ServerControl::RateHint { .. }
        ));
        // The refusal path depends on prose NOT parsing as control.
        assert!(serde_json::from_str::<ServerControl>("this node runs no call hub").is_err());
    }

    #[test]
    fn ws_url_swaps_scheme_only() {
        assert_eq!(
            ws_url("http://127.0.0.1:8844/", "eng"),
            "ws://127.0.0.1:8844/v1/call/ws?channel=eng"
        );
        assert_eq!(
            ws_url("https://node.example", "general"),
            "wss://node.example/v1/call/ws?channel=general"
        );
    }
}
