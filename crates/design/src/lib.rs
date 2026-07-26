//! Desktop font identity and the product type scale. Shared color, shape, and
//! component tokens come from `ducktape-ui`; this crate only owns assets that
//! are specific to the Ducktape application.

/// Font identity. The app embeds these files via `app.ice` `font "…"`
/// settings (paths under this crate's `assets/fonts/`), and `theme.ice`
/// binds roles to the family names. Swap the face by replacing the asset
/// and the family constant together — the app's guard test pins the two to
/// each other.
pub mod fonts {
    /// the UI face — every sans role (default, medium, display).
    pub const FAMILY_UI: &str = "Geist";
    /// the data face — hashes, seqs, diffs, code, the log ring.
    pub const FAMILY_MONO: &str = "Geist Mono";
    /// the CJK fallback face. never named by a role: cosmic-text falls back
    /// to it per glyph whenever the UI face lacks coverage (hangul, kana,
    /// ideographs), so CJK text renders without any view opting in.
    pub const FAMILY_CJK: &str = "IBM Plex Sans KR";

    /// the embedded files, relative to this crate's root — what `app.ice`
    /// points its `font` settings at.
    pub const ASSETS: [&str; 5] = [
        "assets/fonts/Geist[wght].ttf",
        "assets/fonts/GeistMono[wght].ttf",
        "assets/fonts/IBMPlexSansKR-Regular.ttf",
        "assets/fonts/IBMPlexSansKR-Medium.ttf",
        "assets/fonts/IBMPlexSansKR-SemiBold.ttf",
    ];
}

/// The product type roles from the canonical Ducktape design artifact. The
/// drift guard walks every app-authored `.ice` source and rejects other sizes.
pub mod type_scale {
    pub const BADGE: f64 = 9.0;
    pub const NAV: f64 = 9.5;
    pub const FIELD_LABEL: f64 = 10.0;
    pub const MACHINE_META: f64 = 10.5;
    pub const META: f64 = 11.0;
    pub const MACHINE: f64 = 12.0;
    pub const CAPTION: f64 = 12.5;
    pub const LIST: f64 = 13.0;
    pub const BODY: f64 = 13.5;
    pub const PANE_HEADER: f64 = 14.0;
    pub const SECTION: f64 = 16.0;
    pub const SCREEN_TITLE: f64 = 20.0;
    pub const DISPLAY: f64 = 22.0;

    /// every legal `size=` literal in `.ice` sources, the guard's whitelist.
    /// `1.0` is the established off-screen focus-shim size, not a text step.
    pub const ALL: [f64; 14] = [
        BADGE,
        NAV,
        FIELD_LABEL,
        MACHINE_META,
        META,
        MACHINE,
        CAPTION,
        LIST,
        BODY,
        PANE_HEADER,
        SECTION,
        SCREEN_TITLE,
        DISPLAY,
        1.0,
    ];
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

    #[test]
    fn the_scale_is_strictly_increasing() {
        let steps = [
            type_scale::BADGE,
            type_scale::NAV,
            type_scale::FIELD_LABEL,
            type_scale::MACHINE_META,
            type_scale::META,
            type_scale::MACHINE,
            type_scale::CAPTION,
            type_scale::LIST,
            type_scale::BODY,
            type_scale::PANE_HEADER,
            type_scale::SECTION,
            type_scale::SCREEN_TITLE,
            type_scale::DISPLAY,
        ];
        assert!(steps.windows(2).all(|pair| pair[0] < pair[1]));
    }
}
