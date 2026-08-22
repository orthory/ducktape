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
use std::sync::atomic::{AtomicU8, Ordering};
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
/// A screen share's frame rate — the pace of a pull, not of a device: nothing
/// blocks on a grab, so this interval IS the rate. Shared screens are read,
/// not watched: ~10/s tracks a scroll and a typed line without spending a
/// camera's bandwidth on a mostly-still picture.
const SCREEN_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);
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
/// A shared screen's capture ceiling — the TILE budget, not the camera's,
/// because legibility is the whole point of a screen and the receiver cannot
/// hold more than this anyway (it would halve it again on arrival). A 1080p
/// desktop lands at 960×540: a shared editor is readable, a 4K one is not.
// ponytail: one halving of whatever the desktop is. The way past it is a
// codec that carries a still screen cheaply (delta frames), not a bigger JPEG.
const SCREEN_PIXEL_BUDGET: u32 = TILE_PIXEL_BUDGET;

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

/// What the video leg is sending, if anything.
///
/// ONE DISCRIMINANT, because the camera and the screen are two SOURCES for one
/// STREAM, never two streams: a participant occupies one video flow and one
/// tile, and the beacon's `camera_on`/`sharing` pair says which of the two the
/// far end is looking at. Starting a share therefore stops the camera, and
/// turning the camera on stops the share — there is no state where both are
/// true, so no state where the two could disagree about what the peer sees.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Source {
    Off,
    Camera,
    Screen,
}

impl Source {
    fn code(self) -> u8 {
        match self {
            Source::Off => 0,
            Source::Camera => 1,
            Source::Screen => 2,
        }
    }

    fn of(code: u8) -> Self {
        match code {
            1 => Source::Camera,
            2 => Source::Screen,
            _ => Source::Off,
        }
    }
}

static SOURCE: AtomicU8 = AtomicU8::new(0);

pub(crate) fn source() -> Source {
    Source::of(SOURCE.load(Ordering::Relaxed))
}

/// Both readings of the video source, because a toggle moves BOTH: the view
/// draws a camera button and a share button, and starting either one ends the
/// other.
#[derive(Clone, Debug, Default, Hash, PartialEq)]
pub struct VideoSource {
    pub camera: bool,
    pub sharing: bool,
}

/// Point the video leg at `next`. The capture thread notices on its next pass
/// (it holds no device it is not currently asked for); the beacon rides the
/// call module's control channel.
fn use_source(next: Source) -> VideoSource {
    SOURCE.store(next.code(), Ordering::Relaxed);
    // The outgoing preview belongs to the source that is ending — a camera
    // still on screen under a "sharing" beacon is a lie for one frame.
    store().lock().expect("video store").preview = None;
    crate::call::beacon_state();
    VideoSource {
        camera: next == Source::Camera,
        sharing: next == Source::Screen,
    }
}

/// Turn the camera on or off. On ends any screen share.
pub fn call_use_camera(on: bool) -> VideoSource {
    use_source(if on { Source::Camera } else { Source::Off })
}

/// Start or stop sharing the screen. Starting one turns the camera off.
pub fn call_use_screen(on: bool) -> VideoSource {
    use_source(if on { Source::Screen } else { Source::Off })
}

/// Clear everything at session end — the next session must not open on the
/// last call's faces.
pub(crate) fn reset() {
    let mut store = store().lock().expect("video store");
    store.peers.clear();
    store.preview = None;
    SOURCE.store(Source::Off.code(), Ordering::Relaxed);
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

/// Drop a peer's last frame: they left the huddle, or their beacon says the
/// source behind it is off. Frames only ever arrive, so nothing else would
/// ever take one down.
pub(crate) fn forget_peer(node: &str) {
    store().lock().expect("video store").peers.remove(node);
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
    if out.len() <= chat::video::MAX_FRAME_BYTES {
        return Some(out);
    }
    // OVER THE MESH CAP, so this frame cannot be sent as it is — and a source
    // that overruns once overruns every frame, which is a stream that stops
    // dead with no error anywhere. A busy screen is exactly that source (a
    // detailed desktop at 960×540 out-compresses nothing), so trade its
    // resolution rather than its liveness: half the size, one more try.
    let (small, width, height) = halve::<4>(rgba, u32::from(width), u32::from(height));
    let mut out = Vec::new();
    let encoder = jpeg_encoder::Encoder::new(&mut out, JPEG_QUALITY);
    encoder
        .encode(
            &small,
            width as u16,
            height as u16,
            jpeg_encoder::ColorType::Rgba,
        )
        .ok()?;
    (out.len() <= chat::video::MAX_FRAME_BYTES).then_some(out)
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
            refuse_source(events, format!("camera: {error}"));
            None
        }
    }
}

/// A source that will not open: say why on the session's status line, and put
/// the toggle back where the user can see it is off. The capture thread has
/// nothing left to decide.
fn refuse_source(
    events: &iced::futures::channel::mpsc::UnboundedSender<crate::call::CallEvent>,
    message: String,
) {
    let _ = events.unbounded_send(crate::call::CallEvent {
        kind: "live".into(),
        message,
        ..crate::call::CallEvent::default()
    });
    SOURCE.store(Source::Off.code(), Ordering::Relaxed);
}

/// The screen source: one X11 connection, and a full-desktop grab per frame.
///
/// X11 AND PURE RUST ON PURPOSE. `x11rb` is already in this binary (winit
/// draws through it), so the desktop costs no new dependency, no C toolchain
/// and no build-time system library — which the portal/pipewire route would
/// cost on every machine that builds this app, to buy a Wayland path this app
/// cannot use anyway: iced is built here with the `x11` feature and no
/// `wayland`, so the app itself is an X client.
// ponytail: the WHOLE root window, so a multi-head desktop shares every head
// at once — a per-monitor or per-window picker is the obvious next step and
// wants a picker UI, not a different capture.
struct ScreenSource {
    connection: x11rb::rust_connection::RustConnection,
    root: x11rb::protocol::xproto::Window,
}

impl ScreenSource {
    fn open() -> Result<Self, String> {
        use x11rb::connection::Connection as _;
        use x11rb::protocol::xproto::ImageOrder;

        // A WAYLAND SESSION'S X SERVER IS XWAYLAND, and its root window holds
        // X clients only — a grab there is a black rectangle with this app's
        // own windows in it, never the desktop. Refusing says that; sharing it
        // would be a lie the sharer cannot see (they see their own screen).
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            return Err("screen sharing needs an X11 session, and this one is Wayland".into());
        }
        let (connection, screen) =
            x11rb::connect(None).map_err(|error| format!("no X display ({error})"))?;
        let setup = connection.setup();
        let root = setup
            .roots
            .get(screen)
            .ok_or_else(|| "the X display named no screen".to_string())?
            .root;
        // The one pixel layout `grab` reads: 32 bits per pixel, little-endian,
        // which is every TrueColor desktop this app runs on. Anything else is
        // refused rather than shipped as swapped colour.
        let depth = setup
            .roots
            .get(screen)
            .map(|screen| screen.root_depth)
            .unwrap_or_default();
        let bits = setup
            .pixmap_formats
            .iter()
            .find(|format| format.depth == depth)
            .map(|format| format.bits_per_pixel);
        let packed_bgrx = bits == Some(32) && setup.image_byte_order == ImageOrder::LSB_FIRST;
        if !packed_bgrx {
            return Err(format!(
                "this display's {depth}-bit pixel layout is not one screen sharing can read"
            ));
        }
        Ok(ScreenSource { connection, root })
    }

    /// One grab, RGBA, already inside the wire budget.
    fn grab(&self) -> Result<(Vec<u8>, u32, u32), String> {
        use x11rb::protocol::xproto::{ConnectionExt as _, ImageFormat};

        // The geometry is re-read per frame: a resolution change mid-share
        // would otherwise grab a rectangle the root no longer has.
        let geometry = self
            .connection
            .get_geometry(self.root)
            .map_err(|error| error.to_string())?
            .reply()
            .map_err(|error| error.to_string())?;
        let image = self
            .connection
            .get_image(
                ImageFormat::Z_PIXMAP,
                self.root,
                0,
                0,
                geometry.width,
                geometry.height,
                u32::MAX,
            )
            .map_err(|error| error.to_string())?
            .reply()
            .map_err(|error| error.to_string())?;
        let (mut pixels, width, height) = shrink_to_budget::<4>(
            image.data,
            u32::from(geometry.width),
            u32::from(geometry.height),
            SCREEN_PIXEL_BUDGET,
        );
        // X hands back BGRX; the renderer and the encoder both read RGBA. The
        // swap runs AFTER the shrink, over the small image.
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.swap(0, 2);
            pixel[3] = 0xff;
        }
        Ok((pixels, width, height))
    }
}

/// What the capture thread currently HOLDS, which follows [`Source`] one pass
/// behind it — a device is opened when the toggle asks for it and dropped the
/// moment it is not what is asked for.
// Half a kilobyte of X11 connection in the largest variant, held once, on one
// thread, for as long as a share lasts — boxing it would trade a pointer chase
// per frame for nothing anyone can measure.
#[allow(clippy::large_enum_variant)]
enum Open {
    None,
    Camera(nokhwa::Camera),
    Screen(ScreenSource),
}

impl Open {
    fn is(&self, source: Source) -> bool {
        match self {
            Open::None => source == Source::Off,
            Open::Camera(_) => source == Source::Camera,
            Open::Screen(_) => source == Source::Screen,
        }
    }
}

fn open_source(
    source: Source,
    events: &iced::futures::channel::mpsc::UnboundedSender<crate::call::CallEvent>,
) -> Open {
    match source {
        Source::Off => Open::None,
        Source::Camera => open_camera(events).map_or(Open::None, Open::Camera),
        Source::Screen => match ScreenSource::open() {
            Ok(screen) => Open::Screen(screen),
            Err(reason) => {
                refuse_source(events, format!("share: {reason}"));
                Open::None
            }
        },
    }
}

/// One frame from whatever is open, RGBA and inside its source's budget. An
/// error is the source having stopped answering; the loop drops it and the
/// reopen says why if it cannot come back.
fn grab(open: &mut Open) -> Result<(Vec<u8>, u32, u32), String> {
    use nokhwa::pixel_format::RgbAFormat;

    match open {
        Open::None => Err("nothing is open".into()),
        Open::Camera(device) => {
            // Blocks until the device has a frame: this IS the loop's clock.
            let frame = device.frame().map_err(|error| error.to_string())?;
            let decoded = frame
                .decode_image::<RgbAFormat>()
                .map_err(|error| error.to_string())?;
            let (width, height) = (decoded.width(), decoded.height());
            Ok(shrink_to_budget::<4>(
                decoded.into_raw(),
                width,
                height,
                CAPTURE_PIXEL_BUDGET,
            ))
        }
        Open::Screen(screen) => screen.grab(),
    }
}

/// The capture thread body: follow the source toggle, hold a device only while
/// it is the one asked for, thin the encode to the wire ceiling, hand frames to
/// the session pump. Ends when `shutdown` drops (the session's own teardown
/// chain).
///
/// THE OPEN SOURCE IS THE CLOCK, AND IT IS THE ONLY ONE. `Camera::frame()`
/// blocks until the device has the next frame, so a loop that reads it
/// back-to-back runs at exactly the negotiated rate, self-correcting, forever.
/// The version this replaced ALSO slept a frame interval before that blocking
/// read: a whole period of waiting, and then a wait for the frame after it.
/// The driver's buffers filled while we slept, every pass then took the oldest
/// one, and the self-view arrived a frame late and in bursts — the stutter, in
/// a preview that never touches the network or the codec. A screen grab is the
/// other shape: nothing to wait on, so the wait IS the frame rate.
pub(crate) fn capture_thread(
    frames: tokio::sync::mpsc::UnboundedSender<CapturedFrame>,
    shutdown: std::sync::mpsc::Receiver<()>,
    events: iced::futures::channel::mpsc::UnboundedSender<crate::call::CallEvent>,
) {
    let mut open = Open::None;
    let started = std::time::Instant::now();
    // The wire thinning clock — see WIRE_INTERVAL. The only other clock.
    let mut last_sent: Option<std::time::Instant> = None;
    loop {
        // The shutdown sender dropping is the session ending, and what this
        // waits is the open source's own pace.
        let pace = match &open {
            Open::None => IDLE_POLL,
            Open::Camera(_) => std::time::Duration::ZERO,
            Open::Screen(_) => SCREEN_INTERVAL,
        };
        match shutdown.recv_timeout(pace) {
            Ok(()) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
        }
        let wanted = source();
        if wanted == Source::Off {
            // The device is released the moment the toggle goes off, and the
            // idle pace above becomes the toggle poll.
            open = Open::None;
            continue;
        }
        if !open.is(wanted) {
            // Whatever was open is dropped HERE, by the assignment — a device
            // is never held for a source nobody asked for.
            open = open_source(wanted, &events);
            continue;
        }
        let Ok((rgba, width, height)) = grab(&mut open) else {
            // A source that stopped answering must not spin this loop: drop
            // it, and the reopen above says why if it cannot come back.
            open = Open::None;
            continue;
        };
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

/// The stage the panel mounts above the strip while someone is sharing a
/// screen: ONE frame, as large as the panel is wide, whole.
///
/// A SHARED SCREEN IS NOT A FACE. The strip's plates are a fixed 4:3 crop —
/// the right treatment for a person, and useless for a desktop, which is
/// 16:9 or wider and whose whole content is the point. So the stage takes its
/// height from the frame's own aspect and draws it CONTAINED: every pixel the
/// sharer sees, none of them cropped, none of them stretched.
///
/// `peer` is the sharer's node key, or [`SELF_STAGE`] when this device is the
/// one sharing — seeing your own share is how you know what you published.
pub fn call_video_stage(peer: String) -> Element<'static, ()> {
    let (sized, keyed, alive, painted) = (peer.clone(), peer.clone(), peer.clone(), peer);
    ui_lang_runtime::live_surface(
        REDRAW_INTERVAL,
        move |width| Size::new(width, stage_height(&sized, width)),
        // Layout follows the ASPECT and nothing else: a new frame of the same
        // shape (every frame, ten times a second) must not invalidate layout.
        // Packed, not arithmetic: the width comes off a PEER'S frame, and a
        // multiply wide enough to be readable is a multiply a peer can
        // overflow.
        move || {
            stage_frame(&keyed).map_or(0, |(width, height, _)| {
                u64::from(width) << 32 | u64::from(height)
            })
        },
        move || stage_frame(&alive).is_some(),
        move |renderer, bounds, viewport| paint_stage(&painted, renderer, bounds, viewport),
    )
    .into()
}

/// The stage's stand-in for "the screen this device is sharing" — a sentinel
/// no node key can collide with (they are 64 hex characters).
pub const SELF_STAGE: &str = "you";

/// The staged frame's size and handle: a peer's by node key, or the local
/// preview under [`SELF_STAGE`].
fn stage_frame(peer: &str) -> Option<(u32, u32, iced::widget::image::Handle)> {
    let store = store().lock().expect("video store");
    let frame = match peer {
        SELF_STAGE => store.preview.as_ref(),
        key => store.peers.get(key),
    }?;
    Some((frame.width, frame.height, frame.handle.clone()))
}

/// The height that gives `width` the frame's own aspect. No frame yet is no
/// stage: zero, so the panel reserves nothing for a picture that may never
/// arrive (a sharer whose first frame is still crossing).
fn stage_height(peer: &str, width: f32) -> f32 {
    stage_frame(peer).map_or(0.0, |(frame_width, frame_height, _)| {
        width * frame_height as f32 / frame_width.max(1) as f32
    })
}

fn paint_stage(peer: &str, renderer: &mut iced::Renderer, bounds: Rectangle, viewport: &Rectangle) {
    use iced::advanced::image::Renderer as _;

    let Some((width, height, handle)) = stage_frame(peer) else {
        return;
    };
    let Some(clip) = bounds.intersection(viewport) else {
        return;
    };
    // Contain, not cover: the height above already follows the aspect, so this
    // only matters for the frame or two after a resolution change — and a
    // shared screen with its edges cut off is the one thing this must not do.
    let scale = (bounds.width / width.max(1) as f32).min(bounds.height / height.max(1) as f32);
    let drawn = Size::new(width as f32 * scale, height as f32 * scale);
    let drawing = Rectangle {
        x: bounds.x + (bounds.width - drawn.width) / 2.0,
        y: bounds.y + (bounds.height - drawn.height) / 2.0,
        width: drawn.width,
        height: drawn.height,
    };
    renderer.draw_image(
        iced::advanced::image::Image {
            handle,
            filter_method: iced::widget::image::FilterMethod::default(),
            rotation: iced::Radians(0.0),
            border_radius: 8.0.into(),
            opacity: 1.0,
            snap: true,
        },
        drawing,
        clip,
    );
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

    /// One global store AND one global source, so this stays ONE test, in
    /// sequence — and it carries the blink's property: a stored frame owns ONE
    /// renderer handle, so every view rebuild between two captures reads the
    /// same id and the renderer keeps its upload. Only a new frame is a new id.
    #[test]
    fn the_store_folds_frames_and_the_source_is_one_choice() {
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

        // The staged frame is the local preview under the sentinel, and its
        // height carries the frame's aspect — a 2:1 frame in a 100px column is
        // 50px tall, never a 4:3 plate's crop.
        store_preview(vec![0xff; 8], 2, 1);
        assert!(stage_frame(SELF_STAGE).is_some());
        assert_eq!(stage_height(SELF_STAGE, 100.0), 50.0);
        assert!(stage_frame("a-peer-nobody-sent").is_none());
        assert_eq!(stage_height("a-peer-nobody-sent", 100.0), 0.0);

        // ONE SOURCE: starting either one ends the other, and either one off
        // is off — there is no state where both are live.
        let camera = call_use_camera(true);
        assert_eq!(source(), Source::Camera);
        assert!(camera.camera && !camera.sharing);
        let screen = call_use_screen(true);
        assert_eq!(source(), Source::Screen);
        assert!(screen.sharing && !screen.camera);
        // ...and the outgoing source's last frame goes with it, so the tile
        // strip cannot paint a camera under a "sharing" beacon.
        assert!(preview_id().is_none());
        let off = call_use_screen(false);
        assert_eq!(source(), Source::Off);
        assert!(!off.camera && !off.sharing);

        reset();
        assert!(preview_id().is_none());
        assert_eq!(source(), Source::Off);
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
