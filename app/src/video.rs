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
// ponytail: fixed 640x480-ish @ q60, wire ≤60 fps, no rate-ladder response —
// ducktape is a private-network workspace app, so the generous ceiling is
// deliberate (q60 VGA at 60 fps ≈ 2-4 MB/s per sender); wire the RateHint →
// (fps, quality) ladder when a real WAN leg complains.
//
//! THREADING mirrors the audio leg: one OS thread owns the nokhwa camera
//! (not `Send`), polls the camera toggle, opens the device only while it is
//! on, and dies with the session's shutdown sender. Decoded peer frames land
//! in a global store the `call_video_tiles` extern component reads; the strip
//! is a SELF-REDRAWING widget that repaints its own window at the capture
//! cadence — no app message, no view rebuild, no other window woken.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use chat::call_wire::{CapturedFrame, PeerFrame};
use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{Tree, tree};
use iced::advanced::{Shell, Widget, mouse, renderer};
use iced::{Element, Event, Length, Rectangle, Size, window};

/// Toggle/shutdown poll while no camera is open. With a camera open the loop
/// paces itself at the NEGOTIATED MODE'S OWN RATE instead — the local preview
/// runs at camera-native fps, decoupled from what the wire carries.
const IDLE_POLL: std::time::Duration = std::time::Duration::from_millis(40);
/// The wire's send floor: at most one encoded frame per this interval
/// (~60 fps). A camera slower than this just sends every frame; a faster one
/// is thinned to it. Display and preview are NOT gated by this.
const WIRE_INTERVAL: std::time::Duration = std::time::Duration::from_millis(16);
/// The fixed v1 encode quality (see the ponytail note above).
const JPEG_QUALITY: u8 = 60;
/// Tile width in the strip; height follows the frame's aspect.
const TILE_WIDTH: f32 = 128.0;
/// Capture ceiling, in pixels: the documented ~VGA budget whose q60 JPEG
/// stays well under the mesh's MAX_FRAME_BYTES. A camera that only offers
/// bigger modes is box-halved down to it before the encode.
const CAPTURE_PIXEL_BUDGET: u32 = 640 * 480;
/// Decoded-tile ceiling, in pixels: keeps a tile's RGBA under iced_wgpu's
/// 2 MiB synchronous-upload cliff no matter what a peer ships — the sender
/// bounds itself, but a peer is not trusted to. The cliff test is a STRICT
/// `<` (iced_wgpu `image/cache.rs`), so the budget sits one pixel under the
/// exact boundary: at 512·1024 px a frame's RGBA equals 2 MiB, takes the
/// async path, and is not drawn the frame its handle first appears.
const TILE_PIXEL_BUDGET: u32 = 512 * 1024 - 1;

/// One 2×2 box-average pass over an interleaved `CHANNELS`-per-pixel image;
/// odd edges clamp their second sample. Repeated until a budget holds — a
/// pass is one integer average per output byte, cheap enough for the capture
/// thread and the decode's blocking task alike.
fn halve<const CHANNELS: usize>(pixels: &[u8], width: u32, height: u32) -> (Vec<u8>, u32, u32) {
    let (out_w, out_h) = ((width / 2).max(1), (height / 2).max(1));
    let mut out = Vec::with_capacity(out_w as usize * out_h as usize * CHANNELS);
    for y in 0..out_h {
        let (y0, y1) = ((y * 2).min(height - 1), (y * 2 + 1).min(height - 1));
        for x in 0..out_w {
            let (x0, x1) = ((x * 2).min(width - 1), (x * 2 + 1).min(width - 1));
            for channel in 0..CHANNELS {
                let sample = |sx: u32, sy: u32| {
                    u16::from(pixels[(sy * width + sx) as usize * CHANNELS + channel])
                };
                let sum = sample(x0, y0) + sample(x1, y0) + sample(x0, y1) + sample(x1, y1);
                out.push((sum / 4) as u8);
            }
        }
    }
    (out, out_w, out_h)
}

/// Halve `pixels` until `width * height` fits `budget`.
fn shrink_to_budget<const CHANNELS: usize>(
    mut pixels: Vec<u8>,
    mut width: u32,
    mut height: u32,
    budget: u32,
) -> (Vec<u8>, u32, u32) {
    while width * height > budget {
        (pixels, width, height) = halve::<CHANNELS>(&pixels, width, height);
    }
    (pixels, width, height)
}

/// One decoded tile: the renderer handle, built ONCE per decoded frame, plus
/// the capture size the tile's aspect ratio is computed from.
///
/// THE HANDLE'S IDENTITY IS THE WHOLE POINT. `Handle::from_rgba` stamps a
/// fresh `Id` on every call (iced_core `image.rs`), and iced_wgpu treats a
/// never-seen id as a never-seen image: full copy, fresh atlas allocation,
/// and — above its 2 MiB synchronous-upload cliff — nothing drawn in the
/// frame the id first appears (iced_wgpu `image/cache.rs`). Minting the
/// handle here rather than in `tile()` means every view rebuild between two
/// captures hands the renderer the SAME id and hits its cache, so a tile
/// holds the last decoded frame on screen until the next one arrives.
struct TileFrame {
    width: u32,
    height: u32,
    handle: iced::widget::image::Handle,
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

static CAMERA_ON: AtomicBool = AtomicBool::new(false);

pub(crate) fn camera_enabled() -> bool {
    CAMERA_ON.load(Ordering::Relaxed)
}

/// Flip the camera. The capture thread notices the flag on its next tick;
/// the beacon rides the call module's control channel.
pub fn call_set_camera(on: bool) -> bool {
    CAMERA_ON.store(on, Ordering::Relaxed);
    if !on {
        store().lock().expect("video store").preview = None;
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
    // An oversized peer frame is bounded HERE, not trusted to have been
    // bounded at its sender — above the renderer's upload cliff a fresh
    // handle is skipped for a frame, which reads as the tile blinking.
    let (pixels, width, height) =
        shrink_to_budget::<4>(pixels, width as u32, height as u32, TILE_PIXEL_BUDGET);
    Some(TileFrame {
        width,
        height,
        handle: iced::widget::image::Handle::from_rgba(width, height, pixels),
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
        handle: iced::widget::image::Handle::from_rgba(width, height, rgba),
    });
}

/// The camera thread body: poll the toggle, hold the device only while on,
/// capture at the camera's native rate, thin the encode to the wire ceiling,
/// hand frames to the session pump. Ends when `shutdown` drops (the
/// session's own teardown chain).
pub(crate) fn camera_thread(
    frames: tokio::sync::mpsc::UnboundedSender<CapturedFrame>,
    shutdown: std::sync::mpsc::Receiver<()>,
    events: iced::futures::channel::mpsc::UnboundedSender<crate::call::CallEvent>,
) {
    use nokhwa::pixel_format::RgbFormat;
    use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType, Resolution};

    // The open device plus its native frame interval — the pace the loop
    // holds while it runs.
    let mut camera: Option<(nokhwa::Camera, std::time::Duration)> = None;
    let started = std::time::Instant::now();
    // The cadence is a rolling deadline: each pass waits only for whatever
    // remains of the interval after the previous pass's work, so decode +
    // encode time comes out of the interval instead of stretching it. A pass
    // that overruns waits zero (recv_timeout still polls the channel) and the
    // loop runs flat out at its real speed.
    let mut next_capture = std::time::Instant::now();
    // The wire thinning clock — see WIRE_INTERVAL.
    let mut last_sent: Option<std::time::Instant> = None;
    loop {
        let wait = next_capture.saturating_duration_since(std::time::Instant::now());
        // The shutdown sender dropping is the session ending.
        match shutdown.recv_timeout(wait) {
            Ok(()) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
        }
        // The idle pace; a pulled frame below re-arms at the camera's rate.
        next_capture = std::time::Instant::now() + IDLE_POLL;
        if !camera_enabled() {
            if camera.take().is_some() {
                // Device released the moment the toggle goes off.
            }
            continue;
        }
        if camera.is_none() {
            // 640×480 AT ITS HIGHEST FRAME RATE — the size this module has
            // always documented. `AbsoluteHighestFrameRate` alone meant
            // "highest frame rate, then the HIGHEST resolution" (nokhwa-core
            // `types.rs`), so a 720p/1080p webcam negotiated a mode whose q60
            // JPEG overran the mesh's ~126 KiB `MAX_FRAME_BYTES` and whose
            // RGBA blew iced's 2 MiB upload cliff — both read as blinking.
            // A camera with no VGA mode falls back to that same request and
            // the shrink below brings its frames onto the identical budget.
            let vga = RequestedFormat::new::<RgbFormat>(
                RequestedFormatType::HighestResolution(Resolution::new(640, 480)),
            );
            let any =
                RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);
            match nokhwa::Camera::new(CameraIndex::Index(0), vga)
                .or_else(|_| nokhwa::Camera::new(CameraIndex::Index(0), any))
                .and_then(|mut device| device.open_stream().map(|()| device))
            {
                Ok(device) => {
                    // NATIVE CADENCE: the loop runs at the negotiated mode's
                    // own rate, so the preview is as smooth as the camera —
                    // the wire is thinned separately by WIRE_INTERVAL. The
                    // clamp only guards a driver reporting 0 (or nonsense).
                    let rate = device.frame_rate().clamp(1, 240);
                    let native = std::time::Duration::from_secs_f64(1.0 / f64::from(rate));
                    camera = Some((device, native));
                }
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
        let Some((device, native_interval)) = camera.as_mut() else {
            continue;
        };
        next_capture = std::time::Instant::now() + *native_interval;
        let Ok(frame) = device.frame() else {
            continue;
        };
        let Ok(decoded) = frame.decode_image::<RgbFormat>() else {
            continue;
        };
        let (width, height) = (decoded.width(), decoded.height());
        let rgb = decoded.into_raw();
        let (rgb, width, height) =
            shrink_to_budget::<3>(rgb, width, height, CAPTURE_PIXEL_BUDGET);
        // The preview mirrors EVERY captured frame, before and regardless of
        // the wire: the self-view has no bandwidth to respect, and a frame
        // the encoder refuses (over the mesh cap) must not freeze it.
        store_preview(&rgb, width, height);
        let wire_due = last_sent.is_none_or(|at| at.elapsed() >= WIRE_INTERVAL);
        if !wire_due {
            continue;
        }
        let Some(encoded) = encode_frame(&rgb, width as u16, height as u16) else {
            continue;
        };
        last_sent = Some(std::time::Instant::now());
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

/// The tile strip the huddle panel mounts — a SELF-REDRAWING widget, not a
/// state-driven mount. The previous design republished the store's write
/// counter into app state on a 25 Hz subscription, and in iced ONE message
/// rebuilds EVERY window's whole view tree: the console paid full rebuilds
/// 25 times a second for pixels it never shows. This widget instead reads
/// the store in its own draw pass and schedules the next redraw of ITS OWN
/// window (`Shell::request_redraw_at` — redraw requests are per-window all
/// the way down to winit), so a live camera costs the huddle window a paint
/// pass and costs every other window nothing at all.
pub fn call_video_tiles() -> Element<'static, ()> {
    Element::new(VideoStrip)
}

/// Displayed tile plate: fixed 4:3, the frame Cover-cropped onto it, wrapped
/// into rows on the strip's width.
const TILE_HEIGHT: f32 = 96.0;
const TILE_GAP: f32 = 8.0;
/// The paint ceiling while any tile is live (~60 Hz): high enough for a
/// native-rate preview and full-rate peers; frames that didn't change
/// between beats are Arc-cached handles the renderer draws for free.
const REDRAW_INTERVAL: std::time::Duration = std::time::Duration::from_millis(16);

struct VideoStrip;

#[derive(Default)]
struct StripState {
    /// The wrap-grid's height depends ONLY on the tile count, so layout is
    /// invalidated exactly when the count moves (a join, a leave, a camera
    /// toggle) — never per frame.
    tiles_seen: usize,
}

fn tile_count() -> usize {
    let store = store().lock().expect("video store");
    store.peers.len() + usize::from(store.preview.is_some())
}

/// Peers in stable key order, the local preview last — the same order the
/// row-based strip always drew. `Handle` is `Bytes`-backed (Arc) and its
/// `Id` survives the clone, so each entry is a refcount bump that keeps
/// pointing at the renderer's cached upload.
fn tiles_snapshot() -> Vec<(u32, u32, iced::widget::image::Handle)> {
    let store = store().lock().expect("video store");
    let mut ordered: Vec<(&String, &TileFrame)> = store.peers.iter().collect();
    ordered.sort_by(|a, b| a.0.cmp(b.0));
    let mut tiles: Vec<_> = ordered
        .into_iter()
        .map(|(_, frame)| (frame.width, frame.height, frame.handle.clone()))
        .collect();
    if let Some(preview) = &store.preview {
        tiles.push((preview.width, preview.height, preview.handle.clone()));
    }
    tiles
}

fn grid_columns(width: f32) -> usize {
    ((width + TILE_GAP) / (TILE_WIDTH + TILE_GAP)).floor().max(1.0) as usize
}

fn grid_height(count: usize, columns: usize) -> f32 {
    if count == 0 {
        return 0.0;
    }
    let rows = count.div_ceil(columns);
    rows as f32 * TILE_HEIGHT + (rows - 1) as f32 * TILE_GAP
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for VideoStrip
where
    Renderer: iced::advanced::image::Renderer<Handle = iced::widget::image::Handle>,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<StripState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(StripState::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Shrink)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let width = limits.max().width;
        let count = tile_count();
        tree.state.downcast_mut::<StripState>().tiles_seen = count;
        layout::Node::new(Size::new(width, grid_height(count, grid_columns(width))))
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let Event::Window(window::Event::RedrawRequested(now)) = event else {
            return;
        };
        let count = tile_count();
        if tree.state.downcast_ref::<StripState>().tiles_seen != count {
            shell.invalidate_layout();
        }
        if count > 0 {
            shell.request_redraw_at(*now + REDRAW_INTERVAL);
        }
        // Zero tiles = idle, nothing scheduled. Frames cannot appear without
        // a camera beacon riding the call control channel first; that roster
        // message rebuilds the app, the rebuild redraws this window once,
        // and the clock re-arms right here.
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let columns = grid_columns(bounds.width);
        for (index, (width, height, handle)) in tiles_snapshot().into_iter().enumerate() {
            let cell = Rectangle {
                x: bounds.x + (index % columns) as f32 * (TILE_WIDTH + TILE_GAP),
                y: bounds.y + (index / columns) as f32 * (TILE_HEIGHT + TILE_GAP),
                width: TILE_WIDTH,
                height: TILE_HEIGHT,
            };
            let Some(clip) = cell.intersection(viewport) else {
                continue;
            };
            // Cover: scale the frame to fill the plate, center, crop by clip.
            let scale = (TILE_WIDTH / width.max(1) as f32).max(TILE_HEIGHT / height.max(1) as f32);
            let drawn = Size::new(width as f32 * scale, height as f32 * scale);
            let drawing = Rectangle {
                x: cell.x + (cell.width - drawn.width) / 2.0,
                y: cell.y + (cell.height - drawn.height) / 2.0,
                width: drawn.width,
                height: drawn.height,
            };
            renderer.draw_image(
                iced::advanced::image::Image {
                    handle,
                    filter_method: iced::widget::image::FilterMethod::default(),
                    rotation: iced::Radians(0.0),
                    border_radius: 6.0.into(),
                    opacity: 1.0,
                    snap: true,
                },
                drawing,
                clip,
            );
        }
    }
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
        assert!(matches!(
            &tile.handle,
            iced::widget::image::Handle::Rgba { pixels, .. } if pixels.len() == 64 * 48 * 4
        ));
    }

    #[test]
    fn halving_boxes_pixels_and_respects_the_budget() {
        // A 2×2 quad averages to its mean pixel.
        let quad = [0, 0, 0, 40, 40, 40, 80, 80, 80, 120, 120, 120];
        let (half, w, h) = halve::<3>(&quad, 2, 2);
        assert_eq!((w, h), (1, 1));
        assert_eq!(half, vec![60, 60, 60]);
        // An oversized peer frame shrinks by whole halvings until the tile
        // budget holds — 720p lands at 640×360, under the upload cliff.
        let (pixels, w, h) =
            shrink_to_budget::<4>(vec![7; 1280 * 720 * 4], 1280, 720, TILE_PIXEL_BUDGET);
        assert_eq!((w, h), (640, 360));
        assert_eq!(pixels.len(), 640 * 360 * 4);
        assert!(pixels.iter().all(|&byte| byte == 7));
        // A frame already inside the budget passes through untouched.
        let (pixels, w, h) = shrink_to_budget::<3>(vec![9; 64 * 48 * 3], 64, 48, CAPTURE_PIXEL_BUDGET);
        assert_eq!((w, h, pixels.len()), (64, 48, 64 * 48 * 3));
    }

    /// One global store, so this stays ONE test — and it carries the blink's
    /// property: a stored frame owns ONE renderer handle, so every view
    /// rebuild between two captures reads the same id and the renderer keeps
    /// its upload. Only a new frame is a new id.
    #[test]
    fn the_store_folds_frames_and_resets_clean() {
        let preview_id = || {
            store()
                .lock()
                .unwrap()
                .preview
                .as_ref()
                .map(|frame| frame.handle.id())
        };
        reset();
        assert_eq!(tile_count(), 0);
        store_preview(&[10, 20, 30], 1, 1);
        assert_eq!(tile_count(), 1);
        let first = preview_id().expect("preview");
        assert_eq!(first, preview_id().expect("preview"));
        store_preview(&[40, 50, 60], 1, 1);
        assert_ne!(first, preview_id().expect("preview"));
        reset();
        assert!(preview_id().is_none());
        assert!(!camera_enabled());
    }

    /// The strip's whole layout contract: columns floor on width and never
    /// hit zero, height is rows of fixed plates — count in, size out.
    #[test]
    fn the_grid_wraps_on_width_and_sizes_by_count() {
        assert_eq!(grid_columns(300.0), 2);
        assert_eq!(grid_columns(100.0), 1);
        assert_eq!(grid_height(0, 2), 0.0);
        assert_eq!(grid_height(1, 2), TILE_HEIGHT);
        assert_eq!(grid_height(3, 2), 2.0 * TILE_HEIGHT + TILE_GAP);
    }
}
