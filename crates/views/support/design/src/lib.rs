//! Bundled desktop fonts shared by native text rendering and WASM font names.

/// Font identity. The native shell loads these files into GPUI's text system.
/// Guest wire text names the same families. Replace an asset and its family
/// constant together when changing the product face.
pub mod fonts {
    /// the UI face — every sans role (default, medium, display).
    pub const FAMILY_UI: &str = "Geist";
    /// the data face — hashes, seqs, diffs, code, the log ring.
    pub const FAMILY_MONO: &str = "Geist Mono";
    /// Bundled files relative to this crate. The emoji face supplies fallback
    /// glyphs; it is not a separate product type role.
    pub const ASSETS: [&str; 3] = [
        "assets/fonts/Geist[wght].ttf",
        "assets/fonts/GeistMono[wght].ttf",
        "assets/fonts/NotoColorEmoji.ttf",
    ];
}

/// The native shell's default text size.
pub mod type_scale {
    pub const BODY: f64 = 13.5;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_embedded_font_file_exists_and_is_truetype() {
        for asset in fonts::ASSETS {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(asset);
            let bytes = std::fs::read(&path)
                .unwrap_or_else(|error| panic!("font asset {asset} unreadable: {error}"));
            let magic = &bytes[..4];
            assert!(
                magic == b"\x00\x01\x00\x00" || magic == b"OTTO" || magic == b"true",
                "{asset} is not a TrueType/OpenType file"
            );
        }
    }
}
