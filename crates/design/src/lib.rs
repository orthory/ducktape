//! the desktop app's design system — font identity, the type scale, and the
//! depth recipes, as data. the `.ice` sources consume these through extern
//! `style=` calls and through drift-guard tests that hold every hardcoded
//! `size=` / `family=` literal to the values exported here, so a face or
//! scale change is an edit to THIS crate, not a hunt across views.
//!
//! Ice embeds fonts and theme tokens at compile time (no runtime palette
//! until the module-UI runtime lane), so "configurable" today means: one
//! crate to edit, one leash of tests that fail anywhere a view drifts.

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
    pub const FAMILY_CJK: &str = "Noto Sans KR";

    /// the embedded files, relative to this crate's root — what `app.ice`
    /// points its `font` settings at.
    pub const ASSETS: [&str; 3] = [
        "assets/fonts/Geist[wght].ttf",
        "assets/fonts/GeistMono[wght].ttf",
        "assets/fonts/NotoSansKR[wght].ttf",
    ];
}

/// The type scale. Six steps; nothing in the app renders text at any other
/// size (the drift guard walks every `.ice` source and checks). The body
/// step is the app's `default-text-size`.
pub mod type_scale {
    /// timestamps, counters, chip labels, fine print.
    pub const CAPTION: f64 = 12.0;
    /// secondary text, control labels, sidebar rows.
    pub const LABEL: f64 = 13.0;
    /// the reading size — messages, bodies, inputs.
    pub const BODY: f64 = 14.0;
    /// author lines, list-item titles, section headers.
    pub const EMPHASIS: f64 = 15.0;
    /// pane titles (`Forge`, a channel header).
    pub const TITLE: f64 = 17.0;
    /// the workspace/brand header — the one display moment per screen.
    pub const DISPLAY: f64 = 20.0;
    /// DOCUMENT typography, not chrome: a page's own title (pages pane).
    /// document headings reuse [`DISPLAY`] (h1) and [`TITLE`] (h2).
    pub const DOC_TITLE: f64 = 34.0;

    /// every legal `size=` literal in `.ice` sources, the guard's whitelist.
    /// `1.0` is the established off-screen focus-shim size, not a text step.
    pub const ALL: [f64; 8] = [
        CAPTION, LABEL, BODY, EMPHASIS, TITLE, DISPLAY, DOC_TITLE, 1.0,
    ];
}

/// The palette, mirrored from `theme.ice` (theme tokens are compile-time
/// literals there; the app's guard test asserts the two never drift). Depth
/// recipes below derive from these.
pub mod palette {
    /// the app canvas.
    pub const BG: u32 = 0xfcfcfc;
    /// recessed wells: inputs, the composer, inline controls.
    pub const SURFACE: u32 = 0xf5f5f5;
    /// paper — cards and floating layers sit ABOVE the canvas in white.
    pub const POPOVER: u32 = 0xffffff;
    /// the navigation pane.
    pub const SIDEBAR: u32 = 0xf9f9f9;
    /// panels one notch above the canvas.
    pub const ELEVATED: u32 = 0xefefef;
    /// warm ink.
    pub const FG: u32 = 0x2c2b27;
    pub const MUTED: u32 = 0x878787;
    /// the terracotta accent — the single color voice.
    pub const PRIMARY: u32 = 0xa05a3c;
    pub const PRIMARY_HI: u32 = 0x8a4a2e;
    pub const DANGER: u32 = 0xc0483c;
    pub const SUCCESS: u32 = 0x5cb45f;
    pub const BORDER: u32 = 0xe5e5e5;
    /// the warm shadow ink every depth recipe casts with.
    pub const SHADOW_INK: u32 = 0x282622;
}

use iced::widget::container;
use iced::{Background, Border, Color, Shadow, Theme, Vector, border};

fn color(rgb: u32) -> Color {
    Color::from_rgb(
        ((rgb >> 16) & 0xff) as f32 / 255.0,
        ((rgb >> 8) & 0xff) as f32 / 255.0,
        (rgb & 0xff) as f32 / 255.0,
    )
}

fn shadow(alpha: f32, y: f32, blur: f32) -> Shadow {
    Shadow {
        color: Color {
            a: alpha,
            ..color(palette::SHADOW_INK)
        },
        offset: Vector::new(0.0, y),
        blur_radius: blur,
    }
}

/// A card: paper above the canvas — white fill, hairline border, the tight
/// warm shadow. The workhorse surface for sections and list items.
pub fn card_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color(palette::POPOVER))),
        border: Border {
            color: color(palette::BORDER),
            width: 1.0,
            radius: border::radius(11.0),
        },
        shadow: shadow(0.05, 1.0, 2.0),
        ..container::Style::default()
    }
}

/// A raised layer: menus, popovers, the composer — paper with the deep soft
/// shadow that separates a floating surface from the page.
pub fn raised_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color(palette::POPOVER))),
        border: Border {
            color: color(palette::BORDER),
            width: 1.0,
            radius: border::radius(13.0),
        },
        shadow: shadow(0.16, 6.0, 24.0),
        ..container::Style::default()
    }
}

/// A recessed well: the surface a control sits IN — one notch below paper,
/// no shadow (depth comes from the step down, never from translucency).
pub fn well_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color(palette::SURFACE))),
        border: Border {
            color: color(palette::BORDER),
            width: 1.0,
            radius: border::radius(9.0),
        },
        shadow: Shadow::default(),
        ..container::Style::default()
    }
}

/// A quiet inset panel: one notch above the canvas without paper-grade
/// contrast — nested rows inside a card (review comments, log excerpts).
pub fn inset_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color(palette::ELEVATED))),
        border: Border {
            color: Color {
                a: 0.6,
                ..color(palette::BORDER)
            },
            width: 1.0,
            radius: border::radius(9.0),
        },
        shadow: Shadow::default(),
        ..container::Style::default()
    }
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
            type_scale::CAPTION,
            type_scale::LABEL,
            type_scale::BODY,
            type_scale::EMPHASIS,
            type_scale::TITLE,
            type_scale::DISPLAY,
        ];
        assert!(steps.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn depth_recipes_keep_surfaces_fully_opaque() {
        for style in [card_style, raised_style, well_style, inset_style] {
            let Some(Background::Color(fill)) = style(&Theme::Light).background else {
                panic!("every depth recipe names an opaque fill");
            };
            assert_eq!(fill.a, 1.0, "no translucent surfaces — depth is steps + shadow");
        }
    }
}
