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
use iced::{Element, Rectangle, Size};

/// Toggle/shutdown poll while no camera is open. WITH ONE OPEN THE LOOP KEEPS
/// NO CLOCK AT ALL: `Camera::frame()` blocks until the device has the next
/// frame, so the camera itself is the pace — see [`camera_thread`].
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
    decoder.set_options(
        zune_jpeg::zune_core::options::DecoderOptions::default()
            .jpeg_set_out_colorspace(zune_jpeg::zune_core::colorspace::ColorSpace::RGBA),
    );
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

/// Encode one captured RGBA frame to the wire's opaque bytes (the encoder
/// ignores the alpha channel). Public for the unit round-trip; the capture
/// thread is its only product caller.
///
/// RGBA, NOT RGB, BECAUSE THE PREVIEW IS RGBA. The camera is decoded once,
/// into the layout the renderer wants, and the wire copy borrows that — the
/// arrangement this replaced decoded to RGB and then rebuilt a whole second
/// RGBA image per frame, on the capture thread, in the gap between two frames.
pub(crate) fn encode_frame(rgba: &[u8], width: u16, height: u16) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let encoder = jpeg_encoder::Encoder::new(&mut out, JPEG_QUALITY);
    encoder
        .encode(rgba, width, height, jpeg_encoder::ColorType::Rgba)
        .ok()?;
    if out.len() > chat::video::MAX_FRAME_BYTES {
        return None;
    }
    Some(out)
}

/// The local preview: the camera's own pixels, taking ownership of the frame
/// the capture pass decoded.
pub(crate) fn store_preview(rgba: Vec<u8>, width: u32, height: u32) {
    store().lock().expect("video store").preview = Some(TileFrame {
        width,
        height,
        handle: iced::widget::image::Handle::from_rgba(width, height, rgba),
    });
}

/// Open the camera, or say why. 640×480 AT ITS HIGHEST FRAME RATE — the size
/// this module has always documented. `AbsoluteHighestFrameRate` alone meant
/// "highest frame rate, then the HIGHEST resolution" (nokhwa-core `types.rs`),
/// so a 720p/1080p webcam negotiated a mode whose q60 JPEG overran the mesh's
/// ~126 KiB `MAX_FRAME_BYTES` and whose RGBA blew iced's 2 MiB upload cliff —
/// both read as blinking. A camera with no VGA mode falls back to that same
/// request and the capture shrink brings its frames onto the identical budget.
///
/// A refusal turns the toggle back off and surfaces as "live · camera: …"
/// through the status fold; the caller has nothing to decide.
fn open_camera(
    events: &iced::futures::channel::mpsc::UnboundedSender<crate::call::CallEvent>,
) -> Option<nokhwa::Camera> {
    use nokhwa::pixel_format::RgbAFormat;
    use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType, Resolution};

    let vga = RequestedFormat::new::<RgbAFormat>(RequestedFormatType::HighestResolution(
        Resolution::new(640, 480),
    ));
    let any = RequestedFormat::new::<RgbAFormat>(RequestedFormatType::AbsoluteHighestFrameRate);
    match nokhwa::Camera::new(CameraIndex::Index(0), vga)
        .or_else(|_| nokhwa::Camera::new(CameraIndex::Index(0), any))
        .and_then(|mut device| device.open_stream().map(|()| device))
    {
        Ok(device) => Some(device),
        Err(error) => {
            let event = crate::call::CallEvent {
                kind: "live".into(),
                message: format!("camera: {error}"),
                ..crate::call::CallEvent::default()
            };
            let _ = events.unbounded_send(event);
            CAMERA_ON.store(false, Ordering::Relaxed);
            None
        }
    }
}

/// The camera thread body: poll the toggle, hold the device only while on,
/// capture at the camera's own rate, thin the encode to the wire ceiling,
/// hand frames to the session pump. Ends when `shutdown` drops (the
/// session's own teardown chain).
///
/// THE CAMERA IS THE CLOCK, AND IT IS THE ONLY ONE. `Camera::frame()` blocks
/// until the device has the next frame, so a loop that reads it back-to-back
/// runs at exactly the negotiated rate, self-correcting, forever. The version
/// this replaced ALSO slept a frame interval before that blocking read: a
/// whole period of waiting, and then a wait for the frame after it. The
/// driver's buffers filled while we slept, every pass then took the oldest
/// one, and the self-view arrived a frame late and in bursts — the stutter,
/// in a preview that never touches the network or the codec.
pub(crate) fn camera_thread(
    frames: tokio::sync::mpsc::UnboundedSender<CapturedFrame>,
    shutdown: std::sync::mpsc::Receiver<()>,
    events: iced::futures::channel::mpsc::UnboundedSender<crate::call::CallEvent>,
) {
    use nokhwa::pixel_format::RgbAFormat;

    let mut camera: Option<nokhwa::Camera> = None;
    let started = std::time::Instant::now();
    // The wire thinning clock — see WIRE_INTERVAL. The only clock left.
    let mut last_sent: Option<std::time::Instant> = None;
    loop {
        // The shutdown sender dropping is the session ending. With a camera
        // open this only polls — the blocking read below is the pace; with
        // none it is the idle clock.
        let idle_wait = match camera.is_some() {
            true => std::time::Duration::ZERO,
            false => IDLE_POLL,
        };
        match shutdown.recv_timeout(idle_wait) {
            Ok(()) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
        }
        if !camera_enabled() {
            // Device released the moment the toggle goes off.
            camera = None;
            continue;
        }
        if camera.is_none() {
            camera = open_camera(&events);
            continue;
        }
        let Some(device) = camera.as_mut() else {
            continue;
        };
        let Ok(frame) = device.frame() else {
            // A device that stopped answering must not spin this loop: drop
            // it, and the reopen above says why on its next attempt.
            camera = None;
            continue;
        };
        let Ok(decoded) = frame.decode_image::<RgbAFormat>() else {
            continue;
        };
        let (width, height) = (decoded.width(), decoded.height());
        let rgba = decoded.into_raw();
        let (rgba, width, height) =
            shrink_to_budget::<4>(rgba, width, height, CAPTURE_PIXEL_BUDGET);
        // The wire's copy only BORROWS the frame, so it is taken first and the
        // preview then takes ownership: the self-view mirrors every captured
        // frame regardless of the wire — it has no bandwidth to respect, and a
        // frame the encoder refuses (over the mesh cap) must not freeze it.
        let wire_due = last_sent.is_none_or(|at| at.elapsed() >= WIRE_INTERVAL);
        let encoded = wire_due
            .then(|| encode_frame(&rgba, width as u16, height as u16))
            .flatten();
        store_preview(rgba, width, height);
        let Some(encoded) = encoded else {
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

/// The tile strip the huddle panel mounts — a runtime `LiveSurface`, not a
/// state-driven mount. The surface reads the store in its own draw pass and
/// repaints only ITS OWN window at the paint ceiling, so a live camera costs
/// the huddle window a paint pass and costs every other window nothing at
/// all (the state-driven predecessor rebuilt EVERY window's view tree per
/// beat). The layout key is the tile count: the wrap-grid's height depends
/// only on it, so layout invalidates on a join/leave/camera toggle — never
/// per frame. Zero tiles parks the clock; frames cannot appear without a
/// camera beacon riding the call control channel first, and that roster
/// message redraws the window once, which re-arms it.
pub fn call_video_tiles() -> Element<'static, ()> {
    ui_lang_runtime::live_surface(
        REDRAW_INTERVAL,
        |width| Size::new(width, grid_height(tile_count(), grid_columns(width))),
        || tile_count() as u64,
        || tile_count() > 0,
        paint_tiles,
    )
    .into()
}

/// Displayed tile plate: fixed 4:3, the frame Cover-cropped onto it, wrapped
/// into rows on the strip's width.
const TILE_HEIGHT: f32 = 96.0;
const TILE_GAP: f32 = 8.0;
/// How soon after painting a live tile the surface asks to be painted again.
///
/// SHORTER THAN ANY DISPLAY'S FRAME, ON PURPOSE — this is not a target rate,
/// it is "there is always a repaint owed". The window presents on vsync, so
/// what a beat longer than a refresh period buys is a beat that drifts against
/// it: ask again 16 ms after a frame the compositor showed 16.7 ms apart and
/// every few frames the request lands a hair too late, waits a whole extra
/// refresh, and shows the same picture twice — a periodic hitch on a preview
/// whose pixels arrived on time. At 4 ms the redraw is always already owed and
/// each vsync paints the newest camera frame in hand, which is as close to
/// "straight from the camera" as a composited window gets.
///
/// It costs the huddle's own window a paint pass per refresh and no other
/// window anything (that is what `live_surface` is for), and it parks
/// completely when no tile is live.
const REDRAW_INTERVAL: std::time::Duration = std::time::Duration::from_millis(4);

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
    ((width + TILE_GAP) / (TILE_WIDTH + TILE_GAP))
        .floor()
        .max(1.0) as usize
}

fn grid_height(count: usize, columns: usize) -> f32 {
    if count == 0 {
        return 0.0;
    }
    let rows = count.div_ceil(columns);
    rows as f32 * TILE_HEIGHT + (rows - 1) as f32 * TILE_GAP
}

fn paint_tiles(renderer: &mut iced::Renderer, bounds: Rectangle, viewport: &Rectangle) {
    use iced::advanced::image::Renderer as _;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_round_trips_a_synthetic_frame() {
        // A 64×48 gradient: encode must fit the mesh cap and decode back to
        // the same dimensions with RGBA pixels. Capture is RGBA end to end now
        // — the camera decodes once, into the layout the renderer wants — and
        // the encoder drops the alpha it is handed.
        let (width, height) = (64u16, 48u16);
        let rgba: Vec<u8> = (0..u32::from(width) * u32::from(height))
            .flat_map(|i| [(i % 251) as u8, (i % 83) as u8, (i % 199) as u8, 0xff])
            .collect();
        let encoded = encode_frame(&rgba, width, height).expect("encode");
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
        let (pixels, w, h) =
            shrink_to_budget::<3>(vec![9; 64 * 48 * 3], 64, 48, CAPTURE_PIXEL_BUDGET);
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
        store_preview(vec![10, 20, 30, 0xff], 1, 1);
        assert_eq!(tile_count(), 1);
        let first = preview_id().expect("preview");
        assert_eq!(first, preview_id().expect("preview"));
        store_preview(vec![40, 50, 60, 0xff], 1, 1);
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
