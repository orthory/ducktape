//! The huddle's camera leg: capture → intra-only JPEG → the call socket's
//! video frames, and inbound peer frames → decode → the tile strip.
//!
//! CODEC v1 IS BASELINE JPEG, EVERY FRAME A KEYFRAME. The wire (ws
//! `chat::call_wire` and the mesh fragmentation in `chat::video`) treats the
//! encoded bytes as opaque and both ends of the webview leg are THIS app, so
//! the client picks the codec. Pure-Rust JPEG keeps the build free of C
//! toolchains on every platform; intra-only means a lost frame costs nothing
//! (the next one is a sync point), so inbound `KeyframeRequest`s are
//! meaningless and never sent. The seam to a delta codec (VP8/AV1) is
//! `encode_frame`/`store_peer_frame` — nothing else knows JPEG exists.
//! MAX_FRAME_BYTES on the mesh is ~129 KB; 640×480 at the fixed q60 runs
//! 30–60 KB.
// ponytail: fixed 640x480-ish @ q60, ~12 fps, no rate-ladder response — wire
// the RateHint → (fps, quality) ladder when real WANs complain.
//
//! THREADING mirrors the audio leg: one OS thread owns the nokhwa camera
//! (not `Send`), polls the camera toggle, opens the device only while it is
//! on, and dies with the session's shutdown sender. Decoded peer frames land
//! in a global store the `call_video_tiles` extern component reads; a 15 Hz
//! ice tick republishes the store's generation so the panel repaints while
//! frames move.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Mutex, OnceLock};

use chat::call_wire::{CapturedFrame, PeerFrame};
use iced::Element;

/// Capture cadence while the camera is on (~12 fps).
const CAPTURE_INTERVAL: std::time::Duration = std::time::Duration::from_millis(80);
/// The fixed v1 encode quality (see the ponytail note above).
const JPEG_QUALITY: u8 = 60;
/// Tile width in the strip; height follows the frame's aspect.
const TILE_WIDTH: f32 = 128.0;

/// One decoded tile: RGBA pixels at (width, height).
struct TileFrame {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

struct VideoStore {
    /// peer node-key hex → latest decoded frame.
    peers: HashMap<String, TileFrame>,
    /// the local camera's preview, when on.
    preview: Option<TileFrame>,
}

fn store() -> &'static Mutex<VideoStore> {
    static STORE: OnceLock<Mutex<VideoStore>> = OnceLock::new();
    STORE.get_or_init(|| {
        Mutex::new(VideoStore {
            peers: HashMap::new(),
            preview: None,
        })
    })
}

static GENERATION: AtomicI64 = AtomicI64::new(0);
static CAMERA_ON: AtomicBool = AtomicBool::new(false);

fn bump() {
    GENERATION.fetch_add(1, Ordering::Relaxed);
}

/// The store's write counter — the 15 Hz ice tick copies it into state so the
/// panel rebuilds exactly when a frame moved.
pub fn latest_frame_generation() -> i64 {
    GENERATION.load(Ordering::Relaxed)
}

pub(crate) fn camera_enabled() -> bool {
    CAMERA_ON.load(Ordering::Relaxed)
}

/// Flip the camera. The capture thread notices the flag on its next tick;
/// the beacon rides the call module's control channel.
pub fn call_set_camera(on: bool) -> bool {
    CAMERA_ON.store(on, Ordering::Relaxed);
    if !on {
        store().lock().expect("video store").preview = None;
        bump();
    }
    crate::call::beacon_state();
    on
}

/// Clear everything at session end — the next session must not open on the
/// last call's faces.
pub(crate) fn reset() {
    let mut store = store().lock().expect("video store");
    store.peers.clear();
    store.preview = None;
    CAMERA_ON.store(false, Ordering::Relaxed);
    bump();
}

/// A peer's encoded frame off the call socket: decode and store. Runs on a
/// blocking task — JPEG decode of a 480p frame is ~1–3 ms.
pub(crate) fn store_peer_frame(frame: PeerFrame) {
    let Some(tile) = decode_frame(&frame.data) else {
        return;
    };
    let peer = hex_of(&frame.peer);
    store()
        .lock()
        .expect("video store")
        .peers
        .insert(peer, tile);
    bump();
}

fn hex_of(key: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in key {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn decode_frame(data: &[u8]) -> Option<TileFrame> {
    let mut decoder = zune_jpeg::JpegDecoder::new(data);
    decoder
        .set_options(zune_jpeg::zune_core::options::DecoderOptions::default().jpeg_set_out_colorspace(
            zune_jpeg::zune_core::colorspace::ColorSpace::RGBA,
        ));
    let pixels = decoder.decode().ok()?;
    let (width, height) = decoder.dimensions()?;
    Some(TileFrame {
        width: width as u32,
        height: height as u32,
        rgba: pixels,
    })
}

/// Encode one captured RGB frame to the wire's opaque bytes. Public for the
/// unit round-trip; the capture thread is its only product caller.
pub(crate) fn encode_frame(rgb: &[u8], width: u16, height: u16) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let encoder = jpeg_encoder::Encoder::new(&mut out, JPEG_QUALITY);
    encoder
        .encode(rgb, width, height, jpeg_encoder::ColorType::Rgb)
        .ok()?;
    if out.len() > chat::video::MAX_FRAME_BYTES {
        return None;
    }
    Some(out)
}

/// The local preview mirror of a frame we just sent.
pub(crate) fn store_preview(rgb: &[u8], width: u32, height: u32) {
    let mut rgba = Vec::with_capacity(rgb.len() / 3 * 4);
    for pixel in rgb.chunks_exact(3) {
        rgba.extend_from_slice(pixel);
        rgba.push(0xff);
    }
    store().lock().expect("video store").preview = Some(TileFrame {
        width,
        height,
        rgba,
    });
    bump();
}

/// The camera thread body: poll the toggle, hold the device only while on,
/// capture at ~12 fps, encode, hand frames to the session pump. Ends when
/// `shutdown` drops (the session's own teardown chain).
pub(crate) fn camera_thread(
    frames: tokio::sync::mpsc::UnboundedSender<CapturedFrame>,
    shutdown: std::sync::mpsc::Receiver<()>,
    events: iced::futures::channel::mpsc::UnboundedSender<crate::call::CallEvent>,
) {
    use nokhwa::pixel_format::RgbFormat;
    use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType};

    let mut camera: Option<nokhwa::Camera> = None;
    let started = std::time::Instant::now();
    loop {
        // The shutdown sender dropping is the session ending.
        match shutdown.recv_timeout(CAPTURE_INTERVAL) {
            Ok(()) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
        }
        if !camera_enabled() {
            if camera.take().is_some() {
                // Device released the moment the toggle goes off.
            }
            continue;
        }
        if camera.is_none() {
            let requested = RequestedFormat::new::<RgbFormat>(
                RequestedFormatType::AbsoluteHighestFrameRate,
            );
            match nokhwa::Camera::new(CameraIndex::Index(0), requested)
                .and_then(|mut device| device.open_stream().map(|()| device))
            {
                Ok(device) => camera = Some(device),
                Err(error) => {
                    // Surfaces as "live · camera: …" through the status fold.
                    let event = crate::call::CallEvent {
                        kind: "live".into(),
                        message: format!("camera: {error}"),
                        ..crate::call::CallEvent::default()
                    };
                    let _ = events.unbounded_send(event);
                    CAMERA_ON.store(false, Ordering::Relaxed);
                    continue;
                }
            }
        }
        let Some(device) = camera.as_mut() else {
            continue;
        };
        let Ok(frame) = device.frame() else {
            continue;
        };
        let Ok(decoded) = frame.decode_image::<RgbFormat>() else {
            continue;
        };
        let (width, height) = (decoded.width(), decoded.height());
        let rgb = decoded.into_raw();
        let Some(encoded) = encode_frame(&rgb, width as u16, height as u16) else {
            continue;
        };
        store_preview(&rgb, width, height);
        let captured = CapturedFrame {
            keyframe: true,
            ts_ms: started.elapsed().as_millis() as u32,
            data: encoded,
        };
        if frames.send(captured).is_err() {
            break;
        }
    }
}

/// The tile strip the huddle panel mounts: every peer's latest frame plus the
/// local preview, wrapped to the panel's width. Reads the global store; the
/// `generation` prop only exists so ice rebuilds this mount when the 15 Hz
/// tick sees a new frame.
pub fn call_video_tiles(_generation: i64) -> Element<'static, ()> {
    let store = store().lock().expect("video store");
    let mut tiles: Vec<Element<'static, ()>> = Vec::new();
    let mut ordered: Vec<(&String, &TileFrame)> = store.peers.iter().collect();
    ordered.sort_by(|a, b| a.0.cmp(b.0));
    for (_peer, frame) in ordered {
        tiles.push(tile(frame));
    }
    if let Some(preview) = &store.preview {
        tiles.push(tile(preview));
    }
    drop(store);
    let strip = tiles
        .into_iter()
        .fold(iced::widget::Row::new().spacing(8.0), iced::widget::Row::push);
    iced::widget::container(iced::widget::scrollable(strip).direction(
        iced::widget::scrollable::Direction::Horizontal(
            iced::widget::scrollable::Scrollbar::new().width(2).scroller_width(2),
        ),
    ))
    .into()
}

fn tile(frame: &TileFrame) -> Element<'static, ()> {
    let handle = iced::widget::image::Handle::from_rgba(
        frame.width,
        frame.height,
        frame.rgba.clone(),
    );
    let height = TILE_WIDTH * frame.height.max(1) as f32 / frame.width.max(1) as f32;
    iced::widget::container(
        iced::widget::image(handle)
            .width(TILE_WIDTH)
            .height(height)
            .content_fit(iced::ContentFit::Cover),
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_round_trips_a_synthetic_frame() {
        // A 64×48 gradient: encode must fit the mesh cap and decode back to
        // the same dimensions with RGBA pixels.
        let (width, height) = (64u16, 48u16);
        let rgb: Vec<u8> = (0..u32::from(width) * u32::from(height))
            .flat_map(|i| [(i % 251) as u8, (i % 83) as u8, (i % 199) as u8])
            .collect();
        let encoded = encode_frame(&rgb, width, height).expect("encode");
        assert!(encoded.len() < chat::video::MAX_FRAME_BYTES);
        let tile = decode_frame(&encoded).expect("decode");
        assert_eq!((tile.width, tile.height), (64, 48));
        assert_eq!(tile.rgba.len(), 64 * 48 * 4);
    }

    #[test]
    fn the_store_folds_frames_and_resets_clean() {
        reset();
        let before = latest_frame_generation();
        store_preview(&[10, 20, 30], 1, 1);
        assert!(latest_frame_generation() > before);
        assert!(store().lock().unwrap().preview.is_some());
        reset();
        assert!(store().lock().unwrap().preview.is_none());
        assert!(!camera_enabled());
    }
}
