//! The picture viewer: one decoded image per surface, drawn by one extern.
//!
//! A picture is decoded ONCE, off the runtime, into an RGBA
//! [`iced::widget::image::Handle`] and parked under its surface's slot; the
//! `picture` extern hands the SAME handle to every view rebuild, so iced_wgpu
//! keeps hitting its upload cache instead of re-uploading per frame (the
//! lesson `video.rs` paid for — a `Handle::from_rgba` per view is a fresh id,
//! and a fresh id above 2 MiB draws nothing on its first frame). Two surfaces
//! exist — the Files preview and the forge reader — and each keeps exactly one
//! picture, so the store's memory is bounded by the side cap, not by history.
//!
//! The loaders (`files_preview`, `forge_blob`) decide by path whether a file
//! is a picture and page its bytes in; this module only decodes and draws.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use iced::widget::image::Handle;

/// Source-byte ceiling: a file past it is shown as "too large", never
/// decoded. ponytail: 16 MiB holds every screenshot and most photos; raise
/// with a streaming decoder if RAW/TIFF previews ever matter.
pub const MAX_PICTURE_BYTES: usize = 16 * 1024 * 1024;
/// Long-side ceiling after decode. A 2048² RGBA is 16 MiB on the GPU; the
/// pane never draws more pixels than that anyway.
pub const MAX_PICTURE_SIDE: u32 = 2048;
/// The Files preview's slot.
pub const FILES_SURFACE: &str = "files";
/// The forge reader's slot.
pub const FORGE_SURFACE: &str = "forge";
/// How many of a Markdown document's in-repo pictures the loader fetches, in
/// document order. ponytail: the rest keep their alt text; page them lazily
/// if a README ever carries more.
pub const MAX_INLINE_PICTURES: usize = 8;

/// What the renderer is handed: decoded RGBA for a raster, the source bytes
/// for a vector (resvg rasterizes at draw size, so a vector is never
/// downscaled — it has no pixels to lose).
#[derive(Clone, Debug)]
pub enum PictureHandle {
    Raster(Handle),
    Vector(iced::widget::svg::Handle),
}

/// A decoded picture: its drawn dimensions (post-downscale for a raster, the
/// declared size for a vector) and the handle.
#[derive(Clone, Debug)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub handle: PictureHandle,
}

impl Picture {
    /// The picture as a widget: contained to the pane's width at its own
    /// aspect, `Shrink` tall — every mount sits in a scroll column, where a
    /// `Fill` height has nothing to fill.
    pub fn element<Message: 'static>(&self) -> iced::Element<'static, Message> {
        use iced::ContentFit::Contain;
        use iced::Length::{Fill, Shrink};
        use iced::widget::{image, svg};
        match &self.handle {
            PictureHandle::Raster(handle) => image(handle.clone())
                .width(Fill)
                .height(Shrink)
                .content_fit(Contain)
                .into(),
            PictureHandle::Vector(handle) => svg(handle.clone())
                .width(Fill)
                .height(Shrink)
                .content_fit(Contain)
                .into(),
        }
    }
}

/// Does the path name a picture the viewer decodes? The extension is the
/// path's call — the wires only say binary-or-text. SVG is a different
/// widget and is left out on purpose.
pub fn picture_path(path: String) -> bool {
    let name = path.rsplit('/').next().unwrap_or_default();
    let Some((_, extension)) = name.rsplit_once('.') else {
        return false;
    };
    matches!(
        extension.to_ascii_lowercase().as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg"
    )
}

/// `1024 × 768` — the caption under a drawn picture.
pub fn picture_caption(width: i64, height: i64) -> String {
    format!("{width} × {height}")
}

/// Decode source bytes into a picture: an SVG document (by its first tag —
/// the bytes' call, not the path's) is validated and measured by usvg and
/// kept as a vector; anything else decodes to RGBA, downscaled past
/// [`MAX_PICTURE_SIDE`]. Pure and blocking: callers run it under
/// `spawn_blocking`.
pub fn decode_picture(bytes: &[u8]) -> Result<Picture, String> {
    match looks_like_svg(bytes) {
        true => decode_vector(bytes),
        false => decode_raster(bytes),
    }
}

/// An XML or `<svg` opening tag within the first bytes, after a BOM or
/// whitespace. A non-SVG XML document then fails usvg, as it should.
fn looks_like_svg(bytes: &[u8]) -> bool {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(256)]);
    let head = head.trim_start_matches('\u{feff}').trim_start();
    head.starts_with("<svg") || head.starts_with("<?xml")
}

fn decode_vector(bytes: &[u8]) -> Result<Picture, String> {
    let tree = usvg::Tree::from_data(bytes, &usvg::Options::default())
        .map_err(|error| error.to_string())?;
    let size = tree.size();
    Ok(Picture {
        width: size.width().round() as u32,
        height: size.height().round() as u32,
        handle: PictureHandle::Vector(iced::widget::svg::Handle::from_memory(bytes.to_vec())),
    })
}

/// A raster decodes to RGBA the way the camera meant it: the EXIF
/// orientation (a phone photo is stored sideways and tagged) is applied after
/// the downscale, so the drawn size is the upright one.
fn decode_raster(bytes: &[u8]) -> Result<Picture, String> {
    use image::ImageDecoder;
    let mut decoder = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| error.to_string())?
        .into_decoder()
        .map_err(|error| error.to_string())?;
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    // `ImageReader::decode` reserves the decoded size against the crate's
    // default allocation limit before decoding; the decoder path does not,
    // and a 65-byte PNG declaring 40000² pixels would abort the app on a
    // 6.4 GB `vec!`. Reserve the same way.
    let mut limits = image::Limits::default();
    limits
        .reserve(decoder.total_bytes())
        .map_err(|error| error.to_string())?;
    decoder
        .set_limits(limits)
        .map_err(|error| error.to_string())?;
    let decoded = image::DynamicImage::from_decoder(decoder).map_err(|error| error.to_string())?;
    let oversized = decoded.width().max(decoded.height()) > MAX_PICTURE_SIDE;
    let mut fitted = match oversized {
        true => decoded.thumbnail(MAX_PICTURE_SIDE, MAX_PICTURE_SIDE),
        false => decoded,
    };
    fitted.apply_orientation(orientation);
    let rgba = fitted.into_rgba8();
    let (width, height) = rgba.dimensions();
    Ok(Picture {
        width,
        height,
        handle: PictureHandle::Raster(Handle::from_rgba(width, height, rgba.into_raw())),
    })
}

/// surface → (path, picture). One slot per surface.
fn store() -> &'static Mutex<HashMap<&'static str, (String, Picture)>> {
    static STORE: OnceLock<Mutex<HashMap<&'static str, (String, Picture)>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// [`decode_picture`] under `spawn_blocking` — the one way a loader decodes.
pub async fn decode_off_thread(bytes: Vec<u8>) -> Result<Picture, String> {
    tokio::task::spawn_blocking(move || decode_picture(&bytes))
        .await
        .map_err(|error| format!("picture decode task failed: {error}"))?
}

/// Decode off the runtime and park the result under `surface`, replacing
/// whatever that surface held. Returns the drawn `(width, height)`.
pub async fn store_picture(
    surface: &'static str,
    path: String,
    bytes: Vec<u8>,
) -> Result<(u32, u32), String> {
    let decoded = decode_off_thread(bytes).await?;
    let dimensions = (decoded.width, decoded.height);
    park_picture(surface, path, decoded);
    Ok(dimensions)
}

/// Park one decoded picture under `surface` as `path`'s, replacing whatever
/// the surface held. The one writer to the store.
pub(crate) fn park_picture(surface: &'static str, path: String, picture: Picture) {
    store()
        .lock()
        .expect("picture store")
        .insert(surface, (path, picture));
}

/// The picture parked under `surface`, only if it is still `path`'s — a slot
/// holding the previous file never draws under the next file's name.
pub fn stored_picture(surface: &str, path: &str) -> Option<Picture> {
    store()
        .lock()
        .expect("picture store")
        .get(surface)
        .filter(|(stored, _)| stored == path)
        .map(|(_, picture)| picture.clone())
}

/// Resolve a Markdown image URL against the document's place in the repo:
/// `img/a.png` beside `docs/README.md` is `docs/img/a.png`, a leading `/` is
/// the repo root, `.`/`..` fold, a query or fragment is dropped. Anything
/// with a scheme (`https:`, `data:`, `mailto:`), an empty target, or a walk
/// past the root is `None` — the viewer keeps the alt text for those.
pub fn resolve_repo_path(doc: &str, url: &str) -> Option<String> {
    let target = url.split(['?', '#']).next().unwrap_or_default();
    let external = target.is_empty() || target.contains(':');
    if external {
        return None;
    }
    let rooted = target.strip_prefix('/');
    let doc_dir = doc.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("");
    let mut segments: Vec<&str> = match rooted {
        Some(_) => Vec::new(),
        None => doc_dir.split('/').filter(|s| !s.is_empty()).collect(),
    };
    for segment in rooted.unwrap_or(target).split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop()?;
            }
            name => segments.push(name),
        }
    }
    let nothing_left = segments.is_empty();
    if nothing_left {
        return None;
    }
    Some(segments.join("/"))
}

/// doc path → its in-repo pictures by resolved path. One document at a time:
/// the reader shows one Markdown blob, and the loader replaces the whole set
/// when the next one lands.
fn inline_store() -> &'static Mutex<(String, HashMap<String, Picture>)> {
    static STORE: OnceLock<Mutex<(String, HashMap<String, Picture>)>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new((String::new(), HashMap::new())))
}

/// Park a document's in-repo pictures, replacing the previous document's.
pub(crate) fn park_inline_pictures(doc: String, pictures: HashMap<String, Picture>) {
    *inline_store().lock().expect("inline picture store") = (doc, pictures);
}

/// `path`'s picture, only if the parked set is `doc`'s.
pub fn inline_picture(doc: &str, path: &str) -> Option<Picture> {
    let store = inline_store().lock().expect("inline picture store");
    let same_doc = store.0 == doc;
    match same_doc {
        true => store.1.get(path).cloned(),
        false => None,
    }
}

/// The tallest box a raster's viewer takes in the flow. The viewer keeps
/// every wheel event over it (zoom, even at the scale cap), so a box taller
/// than the pane would trap the page's scroll under the picture; a bounded
/// box leaves only the caption below it to scroll to, and a picture taller
/// than the box is contained in it and zoomed into instead.
/// ponytail: a fixed cap, not the pane's height — the extern cannot see the
/// pane; a wrapper that gates the wheel on Ctrl would lift it.
const MAX_VIEWER_HEIGHT: f32 = 560.0;

/// The viewer itself: the surface's picture, centred in the pane. A raster
/// zooms under the wheel and pans under a drag (iced's `image::viewer`); a
/// vector is drawn contained — the viewer is raster-only. The viewer's
/// zoom/pan state lives in the widget tree, so the element is keyed by the
/// path: the next file opens at its own size, not at the last one's zoom.
pub fn picture(surface: String, path: String) -> iced::Element<'static, ()> {
    use iced::ContentFit::Contain;
    use iced::Length::{Fill, Shrink};
    use iced::widget::{container, image, keyed_column, text};
    let Some(picture) = stored_picture(&surface, &path) else {
        return container(text("")).into();
    };
    let element = match &picture.handle {
        PictureHandle::Raster(handle) => container(
            image::viewer(handle.clone())
                .width(Fill)
                .height(Shrink)
                .content_fit(Contain),
        )
        .max_height(MAX_VIEWER_HEIGHT)
        .into(),
        PictureHandle::Vector(_) => picture.element(),
    };
    container(keyed_column([(path_key(&path), element)]))
        .width(Fill)
        .center_x(Fill)
        .into()
}

/// The path as a `keyed_column` key — a 64-bit hash, since iced keys are
/// `Copy`. A collision between two paths open in one session would only
/// carry a zoom across; it is not worth a longer key.
fn path_key(path: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::hash::DefaultHasher::new();
    path.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            width,
            height,
            image::Rgba([10, 20, 30, 255]),
        ))
        .write_to(&mut out, image::ImageFormat::Png)
        .expect("encode");
        out.into_inner()
    }

    #[test]
    fn the_extension_is_the_paths_call() {
        for yes in ["a.png", "dir/b.JPG", "c.jpeg", "d.gif", "e.webp", "f.bmp", "g.svg"] {
            assert!(picture_path(yes.into()), "{yes}");
        }
        for no in ["README.md", "logo", "png", "dir.png/file", "x.png.txt", "a.xml"] {
            assert!(!picture_path(no.into()), "{no}");
        }
    }

    /// A JPEG tagged EXIF orientation 6 (stored rotated 90° CCW, to be shown
    /// rotated 90° CW) decodes upright: a 3×2 file is a 2×3 picture.
    fn sideways_jpeg() -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(3, 2, image::Rgb([10, 20, 30])))
            .write_to(&mut out, image::ImageFormat::Jpeg)
            .expect("encode");
        let jpeg = out.into_inner();
        // APP1 "Exif\0\0" + little-endian TIFF header + one IFD0 entry:
        // tag 0x0112 Orientation, SHORT ×1, value 6; no next IFD.
        let tiff: [u8; 26] = [
            b'I', b'I', 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00, // TIFF header, IFD0 at 8
            0x01, 0x00, // one entry
            0x12, 0x01, 0x03, 0x00, 0x01, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, // next IFD: none
        ];
        let payload_len = 2 + 6 + tiff.len();
        let mut spliced = jpeg[..2].to_vec();
        spliced.extend([0xFF, 0xE1, (payload_len >> 8) as u8, payload_len as u8]);
        spliced.extend(b"Exif\0\0");
        spliced.extend(tiff);
        spliced.extend(&jpeg[2..]);
        spliced
    }

    /// A PNG whose header declares `side`² RGBA pixels and nothing else —
    /// the smallest file that asks the decoder for a huge allocation.
    fn png_declaring(side: u32) -> Vec<u8> {
        fn crc32(data: &[u8]) -> u32 {
            let mut crc = 0xFFFF_FFFFu32;
            for byte in data {
                crc ^= u32::from(*byte);
                for _ in 0..8 {
                    crc = match crc & 1 {
                        1 => 0xEDB8_8320 ^ (crc >> 1),
                        _ => crc >> 1,
                    };
                }
            }
            !crc
        }
        fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
            let mut out = (data.len() as u32).to_be_bytes().to_vec();
            out.extend(kind);
            out.extend(data);
            out.extend(crc32(&[&kind[..], data].concat()).to_be_bytes());
            out
        }
        let mut ihdr = side.to_be_bytes().to_vec();
        ihdr.extend(side.to_be_bytes());
        ihdr.extend([8, 6, 0, 0, 0]);
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        png.extend(chunk(b"IHDR", &ihdr));
        png.extend(chunk(b"IEND", &[]));
        png
    }

    #[test]
    fn a_picture_declaring_more_pixels_than_the_limit_is_an_error_not_an_abort() {
        assert!(decode_picture(&png_declaring(40_000)).is_err());
    }

    #[test]
    fn a_sideways_jpeg_decodes_upright_by_its_exif_orientation() {
        let picture = decode_picture(&sideways_jpeg()).expect("decodes");
        assert_eq!((picture.width, picture.height), (2, 3));
    }

    #[test]
    fn a_small_picture_decodes_at_its_own_size() {
        let picture = decode_picture(&png(3, 2)).expect("decodes");
        assert_eq!((picture.width, picture.height), (3, 2));
    }

    #[test]
    fn an_oversized_picture_is_downscaled_to_the_side_cap_at_its_aspect() {
        let picture = decode_picture(&png(MAX_PICTURE_SIDE * 2, 10)).expect("decodes");
        assert_eq!((picture.width, picture.height), (MAX_PICTURE_SIDE, 5));
    }

    fn svg(width: u32, height: u32, fill: &str) -> Vec<u8> {
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\">\
             <rect width=\"{width}\" height=\"{height}\" fill=\"{fill}\"/></svg>"
        )
        .into_bytes()
    }

    #[test]
    fn a_vector_picture_keeps_its_bytes_and_reports_its_declared_size() {
        let picture = decode_picture(&svg(10, 4, "red")).expect("decodes");
        assert_eq!((picture.width, picture.height), (10, 4));
        assert!(matches!(picture.handle, PictureHandle::Vector(_)));
        let prologue = [b"\xef\xbb\xbf<?xml version=\"1.0\"?>".as_slice(), &svg(3, 3, "blue")].concat();
        assert!(decode_picture(&prologue).is_ok(), "a BOM and an XML prologue are still an SVG");
        let padded = [b"  \n".as_slice(), &svg(3, 3, "blue")].concat();
        assert!(decode_picture(&padded).is_ok(), "leading whitespace is still an SVG");
        let raster = decode_picture(&png(2, 2)).expect("decodes");
        assert!(matches!(raster.handle, PictureHandle::Raster(_)));
    }

    #[test]
    fn a_vector_that_does_not_parse_is_an_error_not_a_blank() {
        assert!(decode_picture(b"<svg xmlns=\"http://www.w3.org/2000/svg\"><rect").is_err());
        assert!(decode_picture(b"<?xml version=\"1.0\"?><not-svg/>").is_err());
    }

    #[test]
    fn bytes_that_are_not_a_picture_are_an_error_not_a_panic() {
        assert!(decode_picture(b"\0not a picture").is_err());
        assert!(decode_picture(b"").is_err());
    }

    #[test]
    fn a_markdown_image_url_resolves_against_the_documents_directory() {
        let cases = [
            ("docs/README.md", "img/a.png", Some("docs/img/a.png")),
            ("README.md", "./a.png", Some("a.png")),
            ("docs/guide/x.md", "../assets/b.jpg", Some("docs/assets/b.jpg")),
            ("docs/x.md", "/logo.png", Some("logo.png")),
            ("x.md", "a.png?raw=1#frag", Some("a.png")),
            ("x.md", "https://host/a.png", None),
            ("x.md", "data:image/png;base64,AAAA", None),
            ("x.md", "../../a.png", None),
            ("x.md", "", None),
            ("x.md", "./", None),
        ];
        for (doc, url, want) in cases {
            assert_eq!(resolve_repo_path(doc, url).as_deref(), want, "{doc} + {url}");
        }
    }

    #[test]
    fn a_documents_inline_pictures_answer_only_under_that_document() {
        let picture = decode_picture(&png(2, 2)).expect("decodes");
        park_inline_pictures("README.md".into(), HashMap::from([("a.png".to_string(), picture)]));
        assert!(inline_picture("README.md", "a.png").is_some());
        assert!(inline_picture("README.md", "b.png").is_none());
        assert!(inline_picture("docs/README.md", "a.png").is_none(), "another document's set never answers");
        park_inline_pictures("docs/README.md".into(), HashMap::new());
        assert!(inline_picture("README.md", "a.png").is_none(), "the next document replaces the set");
    }

    #[test]
    fn a_surface_holds_one_picture_under_its_own_path_only() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let dims = runtime
            .block_on(store_picture("test", "a.png".into(), png(4, 4)))
            .expect("stored");
        assert_eq!(dims, (4, 4));
        assert!(stored_picture("test", "a.png").is_some());
        assert!(stored_picture("test", "b.png").is_none(), "a stale slot never draws under a new path");
        runtime
            .block_on(store_picture("test", "b.png".into(), png(2, 2)))
            .expect("stored");
        assert!(stored_picture("test", "a.png").is_none(), "one slot per surface");
        assert_eq!(stored_picture("test", "b.png").map(|p| p.width), Some(2));
    }
}
