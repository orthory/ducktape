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

/// A decoded picture: its drawn dimensions (post-downscale) and the handle.
#[derive(Clone, Debug)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub handle: Handle,
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
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp"
    )
}

/// `1024 × 768` — the caption under a drawn picture.
pub fn picture_caption(width: i64, height: i64) -> String {
    format!("{width} × {height}")
}

/// Decode source bytes into an RGBA handle, downscaling past
/// [`MAX_PICTURE_SIDE`]. Pure and blocking: callers run it under
/// `spawn_blocking`.
pub fn decode_picture(bytes: &[u8]) -> Result<Picture, String> {
    let decoded = image::load_from_memory(bytes).map_err(|error| error.to_string())?;
    let oversized = decoded.width().max(decoded.height()) > MAX_PICTURE_SIDE;
    let fitted = match oversized {
        true => decoded.thumbnail(MAX_PICTURE_SIDE, MAX_PICTURE_SIDE),
        false => decoded,
    };
    let rgba = fitted.into_rgba8();
    let (width, height) = rgba.dimensions();
    Ok(Picture {
        width,
        height,
        handle: Handle::from_rgba(width, height, rgba.into_raw()),
    })
}

/// surface → (path, picture). One slot per surface.
fn store() -> &'static Mutex<HashMap<&'static str, (String, Picture)>> {
    static STORE: OnceLock<Mutex<HashMap<&'static str, (String, Picture)>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Decode off the runtime and park the result under `surface`, replacing
/// whatever that surface held. Returns the drawn `(width, height)`.
pub async fn store_picture(
    surface: &'static str,
    path: String,
    bytes: Vec<u8>,
) -> Result<(u32, u32), String> {
    let decoded = tokio::task::spawn_blocking(move || decode_picture(&bytes))
        .await
        .map_err(|error| format!("picture decode task failed: {error}"))??;
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

/// The viewer itself: the surface's picture, contained to the pane's width
/// at its own aspect. `Shrink` on the height is deliberate — both mounts sit
/// in scroll columns, where a `Fill` height has nothing to fill.
pub fn picture(surface: String, path: String) -> iced::Element<'static, ()> {
    use iced::Length::{Fill, Shrink};
    use iced::widget::{container, image, text};
    let Some(picture) = stored_picture(&surface, &path) else {
        return container(text("")).into();
    };
    container(
        image(picture.handle)
            .width(Fill)
            .height(Shrink)
            .content_fit(iced::ContentFit::Contain),
    )
    .width(Fill)
    .center_x(Fill)
    .into()
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
        for yes in ["a.png", "dir/b.JPG", "c.jpeg", "d.gif", "e.webp", "f.bmp"] {
            assert!(picture_path(yes.into()), "{yes}");
        }
        for no in ["a.svg", "README.md", "logo", "png", "dir.png/file", "x.png.txt"] {
            assert!(!picture_path(no.into()), "{no}");
        }
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

    #[test]
    fn bytes_that_are_not_a_picture_are_an_error_not_a_panic() {
        assert!(decode_picture(b"\0not a picture").is_err());
        assert!(decode_picture(b"").is_err());
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
