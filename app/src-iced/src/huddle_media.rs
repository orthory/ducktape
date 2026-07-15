//! Native huddle capture, playback, and VP8 processing.
//!
//! CEF never participates here: the call socket hands bounded typed frames to
//! one native worker, while device callbacks only touch bounded queues.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::thread::JoinHandle;

use oxideav_vp8::decoder::Vp8DecodedFrame;
use oxideav_vp8::encoder::{I420Frame, KeyframeParams};
use oxideav_vp8::state::Vp8DecoderState;
use oxideav_vp8::stream::Vp8InterStreamEncoder;

use crate::huddle_session::MediaDriverPort;

pub const VIDEO_WIDTH: u32 = 640;
pub const VIDEO_HEIGHT: u32 = 360;
pub const VIDEO_FPS: u32 = 10;
const KEYFRAME_INTERVAL: u64 = (VIDEO_FPS * 10) as u64;
const EVENT_QUEUE: usize = 16;
const COMMAND_QUEUE: usize = 16;
const MAX_DECODE_PIXELS: u64 = 1280 * 720;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    SetMuted(bool),
    SetCamera(bool),
    SetScreenShare(bool),
    RefreshDevices,
    SetMicrophone(Option<usize>),
    SetCameraDevice(Option<usize>),
    SetSpeaker(Option<usize>),
    SetScreenSource(Option<usize>),
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    MicrophoneDenied,
    MicrophoneUnavailable,
    CameraDenied,
    CameraUnavailable,
    ScreenDenied,
    ScreenUnavailable,
    DeviceSelection,
    Codec,
    Unsupported,
}

impl FailureKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::MicrophoneDenied => "Microphone permission was denied",
            Self::MicrophoneUnavailable => "Microphone or speaker is unavailable",
            Self::CameraDenied => "Camera permission was denied",
            Self::CameraUnavailable => "Camera is unavailable",
            Self::ScreenDenied => "Screen recording permission was denied",
            Self::ScreenUnavailable => "Screen capture is unavailable",
            Self::DeviceSelection => "The selected media device is unavailable",
            Self::Codec => "Video processing failed",
            Self::Unsupported => "Native media is unavailable on this platform",
        }
    }
}

#[derive(Debug, Clone)]
pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
}

#[derive(Debug, Clone, Default)]
pub struct DeviceOptions {
    pub microphones: Vec<String>,
    pub cameras: Vec<String>,
    pub speakers: Vec<String>,
    pub screen_sources: Vec<String>,
    pub microphone: Option<usize>,
    pub camera: Option<usize>,
    pub speaker: Option<usize>,
    pub screen_source: Option<usize>,
}

#[derive(Debug)]
pub enum Event {
    Ready,
    VideoState { camera_on: bool, sharing: bool },
    LocalFrame(VideoFrame),
    PeerFrame { peer: String, frame: VideoFrame },
    RequestKeyframe(String),
    Devices(DeviceOptions),
    Failed { kind: FailureKind, detail: String },
    Stopped,
}

pub struct Handle {
    pub commands: SyncSender<Command>,
    pub events: Receiver<Event>,
    running: Arc<AtomicBool>,
    level: Arc<AtomicU8>,
    worker: Option<JoinHandle<()>>,
}

trait MediaBackend {
    fn run(
        port: MediaDriverPort,
        commands: Receiver<Command>,
        events: SyncSender<Event>,
        running: Arc<AtomicBool>,
        level: Arc<AtomicU8>,
    );
}

impl Handle {
    pub fn start(port: MediaDriverPort) -> Self {
        let (commands, command_rx) = sync_channel(COMMAND_QUEUE);
        let (event_tx, events) = sync_channel(EVENT_QUEUE);
        let running = Arc::new(AtomicBool::new(true));
        let level = Arc::new(AtomicU8::new(0));
        let worker_running = Arc::clone(&running);
        let worker_level = Arc::clone(&level);
        let worker_events = event_tx.clone();
        let worker = std::thread::Builder::new()
            .name("ducktape-huddle-media".into())
            .spawn(move || {
                <platform::Backend as MediaBackend>::run(
                    port,
                    command_rx,
                    worker_events,
                    worker_running,
                    worker_level,
                );
            });
        let worker = match worker {
            Ok(worker) => Some(worker),
            Err(error) => {
                running.store(false, Ordering::Release);
                emit(
                    &event_tx,
                    Event::Failed {
                        kind: FailureKind::MicrophoneUnavailable,
                        detail: format!("could not start native media worker: {error}"),
                    },
                );
                None
            }
        };
        Self {
            commands,
            events,
            running,
            level,
            worker,
        }
    }

    pub fn send(&self, command: Command) {
        if command == Command::Stop {
            // Stop is lifecycle, not best-effort UI input. A full command queue
            // must not strand a microphone/camera worker during retry or quit.
            self.running.store(false, Ordering::Release);
        }
        let _ = self.commands.try_send(command);
    }

    pub fn is_stopped(&self) -> bool {
        self.worker.as_ref().is_none_or(JoinHandle::is_finished)
    }

    pub fn level(&self) -> u8 {
        self.level.load(Ordering::Acquire)
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.send(Command::Stop);
        if let Some(worker) = self.worker.take() {
            // Device permission prompts may outlive the window. Reap off the UI
            // thread so closing a huddle never waits on an OS prompt.
            let _ = std::thread::Builder::new()
                .name("ducktape-huddle-reaper".into())
                .spawn(move || {
                    let _ = worker.join();
                });
        }
    }
}

struct OwnedI420 {
    width: u32,
    height: u32,
    y: Vec<u8>,
    u: Vec<u8>,
    v: Vec<u8>,
}

impl OwnedI420 {
    fn frame(&self) -> I420Frame<'_> {
        I420Frame::packed(self.width, self.height, &self.y, &self.u, &self.v)
    }
}

struct VideoCodec {
    encoder: Vp8InterStreamEncoder,
    decoders: HashMap<[u8; 32], Vp8DecoderState>,
    qindex: u8,
}

impl VideoCodec {
    fn new(max_kbps: u32) -> Self {
        let qindex = qindex(max_kbps);
        Self {
            encoder: new_encoder(qindex),
            decoders: HashMap::new(),
            qindex,
        }
    }

    fn set_rate(&mut self, max_kbps: u32) {
        let qindex = qindex(max_kbps);
        if qindex != self.qindex {
            self.qindex = qindex;
            self.encoder = new_encoder(qindex);
        }
    }

    fn encode(
        &mut self,
        frame: &OwnedI420,
        force_keyframe: bool,
    ) -> Result<(bool, Vec<u8>), String> {
        let encoded = self
            .encoder
            .encode_frame_with_force(&frame.frame(), force_keyframe)
            .map_err(|error| error.to_string())?;
        Ok((encoded.is_keyframe(), encoded.bytes))
    }

    fn decode(&mut self, peer: [u8; 32], keyframe: bool, vp8: &[u8]) -> Result<VideoFrame, String> {
        if !self.decoders.contains_key(&peer) && self.decoders.len() >= 8 {
            if !keyframe {
                return Err("a keyframe is required for the new video participant".into());
            }
            if let Some(stale) = self.decoders.keys().next().copied() {
                self.decoders.remove(&stale);
            }
        }
        let decoded = self
            .decoders
            .entry(peer)
            .or_insert_with(|| Vp8DecoderState::new().with_max_pixels_per_frame(MAX_DECODE_PIXELS))
            .decode_frame(vp8)
            .map_err(|error| error.to_string())?;
        Ok(decoded_to_rgba(&decoded, VIDEO_WIDTH, VIDEO_HEIGHT))
    }
}

fn new_encoder(qindex: u8) -> Vp8InterStreamEncoder {
    Vp8InterStreamEncoder::new(
        KeyframeParams {
            y_ac_qi: qindex,
            loop_filter_level: 8,
            ..KeyframeParams::default()
        },
        KEYFRAME_INTERVAL,
    )
    .expect("the keyframe interval is non-zero")
}

const fn qindex(max_kbps: u32) -> u8 {
    match max_kbps {
        0..=399 => 60,
        400..=599 => 52,
        600..=899 => 44,
        _ => 36,
    }
}

fn rgb_to_i420(
    source: &[u8],
    source_width: u32,
    source_height: u32,
    channels: usize,
    contain: bool,
) -> Result<OwnedI420, String> {
    let required = (source_width as usize)
        .checked_mul(source_height as usize)
        .and_then(|pixels| pixels.checked_mul(channels));
    if !matches!(channels, 3 | 4)
        || source_width == 0
        || source_height == 0
        || required.is_none_or(|required| source.len() < required)
    {
        return Err("captured frame has invalid dimensions".into());
    }
    let width = VIDEO_WIDTH;
    let height = VIDEO_HEIGHT;
    let mut y = vec![0; (width * height) as usize];
    let mut u = vec![0; (width * height / 4) as usize];
    let mut v = vec![0; (width * height / 4) as usize];
    for dy in 0..height {
        for dx in 0..width {
            let (r, g, b) = source_rgb(
                source,
                source_width,
                source_height,
                channels,
                dx,
                dy,
                contain,
            );
            y[(dy * width + dx) as usize] = rgb_y(r, g, b);
        }
    }
    for dy in (0..height).step_by(2) {
        for dx in (0..width).step_by(2) {
            let mut r = 0u32;
            let mut g = 0u32;
            let mut b = 0u32;
            for oy in 0..2 {
                for ox in 0..2 {
                    let rgb = source_rgb(
                        source,
                        source_width,
                        source_height,
                        channels,
                        dx + ox,
                        dy + oy,
                        contain,
                    );
                    r += u32::from(rgb.0);
                    g += u32::from(rgb.1);
                    b += u32::from(rgb.2);
                }
            }
            let index = ((dy / 2) * (width / 2) + dx / 2) as usize;
            u[index] = rgb_u((r / 4) as u8, (g / 4) as u8, (b / 4) as u8);
            v[index] = rgb_v((r / 4) as u8, (g / 4) as u8, (b / 4) as u8);
        }
    }
    Ok(OwnedI420 {
        width,
        height,
        y,
        u,
        v,
    })
}

fn source_rgb(
    source: &[u8],
    source_width: u32,
    source_height: u32,
    channels: usize,
    dx: u32,
    dy: u32,
    contain: bool,
) -> (u8, u8, u8) {
    let source_width_64 = u64::from(source_width);
    let source_height_64 = u64::from(source_height);
    let target_width = u64::from(VIDEO_WIDTH);
    let target_height = u64::from(VIDEO_HEIGHT);
    let dx = u64::from(dx);
    let dy = u64::from(dy);
    let source_is_wider = source_width_64 * target_height > target_width * source_height_64;
    let mapped = if contain && source_is_wider {
        let view_height = (source_height_64 * target_width / source_width_64).max(1);
        let top = (target_height - view_height) / 2;
        (dy >= top && dy < top + view_height).then(|| {
            (
                dx * source_width_64 / target_width,
                (dy - top) * source_height_64 / view_height,
            )
        })
    } else if contain {
        let view_width = (source_width_64 * target_height / source_height_64).max(1);
        let left = (target_width - view_width) / 2;
        (dx >= left && dx < left + view_width).then(|| {
            (
                (dx - left) * source_width_64 / view_width,
                dy * source_height_64 / target_height,
            )
        })
    } else if source_is_wider {
        let view_width = source_height_64 * target_width / target_height;
        Some((
            (source_width_64 - view_width) / 2 + dx * view_width / target_width,
            dy * source_height_64 / target_height,
        ))
    } else {
        let view_height = source_width_64 * target_height / target_width;
        Some((
            dx * source_width_64 / target_width,
            (source_height_64 - view_height) / 2 + dy * view_height / target_height,
        ))
    };
    let Some((sx, sy)) = mapped else {
        return (0, 0, 0);
    };
    let sx = sx.min(source_width_64 - 1) as usize;
    let sy = sy.min(source_height_64 - 1) as usize;
    let index = (sy * source_width as usize + sx) * channels;
    (source[index], source[index + 1], source[index + 2])
}

fn decoded_to_rgba(frame: &Vp8DecodedFrame, width: u32, height: u32) -> VideoFrame {
    let mut rgba = vec![0; (width * height * 4) as usize];
    let chroma_width = frame.width.div_ceil(2);
    for dy in 0..height {
        let sy = dy * frame.height / height;
        for dx in 0..width {
            let sx = dx * frame.width / width;
            let y = i32::from(frame.y[(sy * frame.width + sx) as usize]) - 16;
            let u = i32::from(frame.u[((sy / 2) * chroma_width + sx / 2) as usize]) - 128;
            let v = i32::from(frame.v[((sy / 2) * chroma_width + sx / 2) as usize]) - 128;
            let index = ((dy * width + dx) * 4) as usize;
            rgba[index] = clamp((298 * y + 409 * v + 128) >> 8);
            rgba[index + 1] = clamp((298 * y - 100 * u - 208 * v + 128) >> 8);
            rgba[index + 2] = clamp((298 * y + 516 * u + 128) >> 8);
            rgba[index + 3] = 255;
        }
    }
    VideoFrame {
        width,
        height,
        rgba: rgba.into(),
    }
}

fn preview(frame: &OwnedI420) -> VideoFrame {
    decoded_to_rgba(
        &Vp8DecodedFrame {
            width: frame.width,
            height: frame.height,
            y: frame.y.clone(),
            u: frame.u.clone(),
            v: frame.v.clone(),
        },
        VIDEO_WIDTH,
        VIDEO_HEIGHT,
    )
}

const fn rgb_y(r: u8, g: u8, b: u8) -> u8 {
    clamp(((66 * r as i32 + 129 * g as i32 + 25 * b as i32 + 128) >> 8) + 16)
}

const fn rgb_u(r: u8, g: u8, b: u8) -> u8 {
    clamp(((-38 * r as i32 - 74 * g as i32 + 112 * b as i32 + 128) >> 8) + 128)
}

const fn rgb_v(r: u8, g: u8, b: u8) -> u8 {
    clamp(((112 * r as i32 - 94 * g as i32 - 18 * b as i32 + 128) >> 8) + 128)
}

const fn clamp(value: i32) -> u8 {
    if value < 0 {
        0
    } else if value > 255 {
        255
    } else {
        value as u8
    }
}

fn emit(events: &SyncSender<Event>, event: Event) {
    match events.try_send(event) {
        Ok(()) | Err(TrySendError::Full(_)) => {}
        Err(TrySendError::Disconnected(_)) => {}
    }
}

#[cfg(target_os = "macos")]
mod platform;

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[path = "huddle_media/platform_nonmac.rs"]
mod platform;

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
mod platform {
    use super::*;

    pub struct Backend;

    impl MediaBackend for Backend {
        fn run(
            _port: MediaDriverPort,
            commands: Receiver<Command>,
            events: SyncSender<Event>,
            running: Arc<AtomicBool>,
            _level: Arc<AtomicU8>,
        ) {
            emit(
                &events,
                Event::Failed {
                    kind: FailureKind::Unsupported,
                    detail: "native huddle media currently targets macOS, Linux, and Windows"
                        .into(),
                },
            );
            while running.load(Ordering::Acquire) {
                match commands.recv_timeout(std::time::Duration::from_millis(50)) {
                    Ok(Command::Stop) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        break;
                    }
                    Ok(_) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                }
            }
            emit(&events, Event::Stopped);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_conversion_and_vp8_round_trip_are_bounded() {
        let mut rgb = vec![0; 64 * 48 * 3];
        for (index, pixel) in rgb.chunks_exact_mut(3).enumerate() {
            pixel[0] = (index % 251) as u8;
            pixel[1] = ((index / 3) % 241) as u8;
            pixel[2] = 80;
        }
        let frame = rgb_to_i420(&rgb, 64, 48, 3, false).unwrap();
        assert_eq!(frame.y.len(), (VIDEO_WIDTH * VIDEO_HEIGHT) as usize);
        assert_eq!(frame.u.len(), (VIDEO_WIDTH * VIDEO_HEIGHT / 4) as usize);

        let mut codec = VideoCodec::new(800);
        let (key, encoded) = codec.encode(&frame, false).unwrap();
        assert!(key);
        assert!(!encoded.is_empty());
        let decoded = codec.decode([7; 32], true, &encoded).unwrap();
        assert_eq!((decoded.width, decoded.height), (VIDEO_WIDTH, VIDEO_HEIGHT));
        assert_eq!(
            decoded.rgba.len(),
            (VIDEO_WIDTH * VIDEO_HEIGHT * 4) as usize
        );
    }

    #[test]
    fn malformed_capture_is_rejected_before_allocation() {
        assert!(rgb_to_i420(&[0; 8], 640, 360, 4, false).is_err());
        assert!(rgb_to_i420(&[0; 12], 1, 1, 2, false).is_err());
    }

    #[test]
    fn screen_frames_are_letterboxed_instead_of_cropped() {
        let square = [255u8; 4 * 4 * 3];
        let frame = rgb_to_i420(&square, 4, 4, 3, true).unwrap();
        assert_eq!(frame.y[0], 16, "left pillar must remain black");
        assert!(frame.y[(VIDEO_WIDTH / 2) as usize] > 200);
    }

    #[test]
    fn server_rate_hint_maps_to_a_small_fixed_quality_ladder() {
        assert!(qindex(300) > qindex(800));
        assert_eq!(qindex(1_200), qindex(10_000));
    }

    #[test]
    fn stop_cancels_a_worker_even_when_its_command_queue_is_full() {
        let (commands, command_rx) = sync_channel(COMMAND_QUEUE);
        let (_event_tx, events) = sync_channel(EVENT_QUEUE);
        let running = Arc::new(AtomicBool::new(true));
        let worker_running = Arc::clone(&running);
        let worker = std::thread::spawn(move || {
            while worker_running.load(Ordering::Acquire) {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            drop(command_rx);
        });
        for _ in 0..COMMAND_QUEUE {
            commands.try_send(Command::RefreshDevices).unwrap();
        }
        let handle = Handle {
            commands,
            events,
            running,
            level: Arc::new(AtomicU8::new(0)),
            worker: Some(worker),
        };

        handle.send(Command::Stop);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
        while !handle.is_stopped() && std::time::Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(handle.is_stopped());
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    #[test]
    fn unsupported_platform_fails_honestly_without_touching_devices() {
        let (outgoing, _outgoing_rx) = tokio::sync::mpsc::channel(1);
        let (_incoming_tx, incoming) = tokio::sync::mpsc::channel(1);
        let handle = Handle::start(MediaDriverPort { outgoing, incoming });
        let event = handle
            .events
            .recv_timeout(std::time::Duration::from_secs(1))
            .unwrap();
        assert!(matches!(
            event,
            Event::Failed {
                kind: FailureKind::Unsupported,
                ..
            }
        ));
    }
}
