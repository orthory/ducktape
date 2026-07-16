use iced::{Color, Font, Theme, font};

pub const SANS: Font = Font::with_name("Geist");
// Geist weight-500 falls back to serif in the cosmic-text render path, so
// medium-intent text aliases to the semibold face (600 loads fine) — aliasing
// to Regular collapsed the app's entire weight hierarchy into one flat weight.
pub const SANS_MEDIUM: Font = SANS_SEMIBOLD;
pub const SANS_SEMIBOLD: Font = Font {
    weight: font::Weight::Semibold,
    ..SANS
};
pub const MONO: Font = Font::with_name("Geist Mono");

/// The type scale. Every `.size()` in the app uses one of these — ad-hoc px
/// sizes are a defect. Matches the original React app's ~13px body rhythm.
/// (dead_code allows drop out with the parallel module migration that
/// consumes them — remove in the merge sweep.)
#[allow(dead_code)]
pub const CAPTION: f32 = 10.5;
#[allow(dead_code)]
pub const LABEL: f32 = 12.0;
#[allow(dead_code)]
pub const BODY: f32 = 13.0;
#[allow(dead_code)]
pub const BODY_LG: f32 = 14.0;
#[allow(dead_code)]
pub const TITLE: f32 = 15.5;
#[allow(dead_code)]
pub const HEADING: f32 = 18.0;

pub const FONT_BYTES: [&[u8]; 6] = [
    include_bytes!("../assets/fonts/geist-sans-400.woff2"),
    include_bytes!("../assets/fonts/geist-sans-500.woff2"),
    include_bytes!("../assets/fonts/geist-sans-600.woff2"),
    include_bytes!("../assets/fonts/geist-mono-400.woff2"),
    include_bytes!("../assets/fonts/geist-mono-500.woff2"),
    include_bytes!("../assets/fonts/geist-mono-600.woff2"),
];

pub const RADIUS_SM: f32 = 7.0;
pub const RADIUS_MD: f32 = 9.0;
pub const RADIUS_LG: f32 = 11.0;
#[allow(dead_code)]
pub const RADIUS_WINDOW: f32 = 13.0;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgb8(r, g, b)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Light,
    Dark,
}

impl Mode {
    pub const fn toggled(self) -> Self {
        match self {
            Self::Light => Self::Dark,
            Self::Dark => Self::Light,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub ink: Color,
    pub ink_soft: Color,
    pub ink_softer: Color,
    pub muted: Color,
    pub muted_2: Color,
    pub muted_3: Color,
    pub icon_idle: Color,
    pub paper: Color,
    pub canvas: Color,
    pub sidebar: Color,
    pub titlebar: Color,
    pub hover: Color,
    pub sunken: Color,
    pub panel: Color,
    pub window_border: Color,
    pub border: Color,
    pub border_soft: Color,
    pub border_strong: Color,
    pub chip: Color,
    pub filled: Color,
    pub on_filled: Color,
    pub green: Color,
    pub amber: Color,
    pub blue: Color,
    pub red: Color,
    pub purple: Color,
    pub danger: Color,
    pub danger_soft: Color,
    pub danger_border: Color,
    /// Base color for card/popover shadows; use sites pick the alpha. Mode-aware
    /// so dark mode doesn't inherit light mode's warm-brown shadow.
    /// (allow drops out with the module migration — remove in the merge sweep.)
    #[allow(dead_code)]
    pub shadow: Color,
}

pub const LIGHT: Palette = Palette {
    ink: rgb(0x2c, 0x2b, 0x27),
    ink_soft: rgb(0x3f, 0x3e, 0x39),
    ink_softer: rgb(0x4a, 0x48, 0x43),
    muted: rgb(0x60, 0x60, 0x60),
    muted_2: rgb(0x64, 0x64, 0x64),
    muted_3: rgb(0x5c, 0x5c, 0x5c),
    icon_idle: rgb(0xc5, 0xc5, 0xc5),
    paper: rgb(0xff, 0xff, 0xff),
    canvas: rgb(0xfc, 0xfc, 0xfc),
    sidebar: rgb(0xf9, 0xf9, 0xf9),
    titlebar: rgb(0xf1, 0xf1, 0xf1),
    hover: rgb(0xed, 0xed, 0xed),
    sunken: rgb(0xf5, 0xf5, 0xf5),
    panel: rgb(0xef, 0xef, 0xef),
    window_border: rgb(0xd1, 0xd1, 0xd1),
    border: rgb(0xe5, 0xe5, 0xe5),
    border_soft: rgb(0xec, 0xec, 0xec),
    border_strong: rgb(0xd6, 0xd6, 0xd6),
    chip: rgb(0xe3, 0xe3, 0xe3),
    filled: rgb(0x26, 0x25, 0x1f),
    on_filled: rgb(0xef, 0xef, 0xef),
    green: rgb(0x5c, 0xb4, 0x5f),
    amber: rgb(0xc0, 0x8a, 0x3e),
    blue: rgb(0x5f, 0x7a, 0x9e),
    red: rgb(0xa3, 0x52, 0x48),
    purple: rgb(0x7a, 0x6f, 0x9e),
    danger: rgb(0xc0, 0x48, 0x3c),
    danger_soft: rgb(0xfa, 0xf1, 0xef),
    danger_border: rgb(0xec, 0xcb, 0xc5),
    shadow: rgb(0x28, 0x26, 0x22),
};

pub const DARK: Palette = Palette {
    ink: rgb(0xec, 0xea, 0xe4),
    ink_soft: rgb(0xd4, 0xd2, 0xcb),
    ink_softer: rgb(0xbf, 0xbd, 0xb5),
    muted: rgb(0x9f, 0x9c, 0x95),
    muted_2: rgb(0x9c, 0x99, 0x92),
    muted_3: rgb(0xa7, 0xa4, 0x9d),
    icon_idle: rgb(0x5b, 0x59, 0x52),
    paper: rgb(0x1b, 0x1a, 0x17),
    canvas: rgb(0x17, 0x16, 0x11),
    sidebar: rgb(0x20, 0x1f, 0x1b),
    titlebar: rgb(0x24, 0x23, 0x1f),
    hover: rgb(0x2b, 0x2a, 0x25),
    sunken: rgb(0x15, 0x14, 0x10),
    panel: rgb(0x26, 0x25, 0x1f),
    window_border: rgb(0x34, 0x33, 0x2d),
    border: rgb(0x2e, 0x2d, 0x27),
    border_soft: rgb(0x29, 0x28, 0x23),
    border_strong: rgb(0x3b, 0x3a, 0x33),
    chip: rgb(0x32, 0x30, 0x29),
    filled: rgb(0xec, 0xeb, 0xe5),
    on_filled: rgb(0x1b, 0x1a, 0x17),
    green: rgb(0x6c, 0xc0, 0x6f),
    amber: rgb(0xd3, 0xa2, 0x5c),
    blue: rgb(0x7f, 0x9b, 0xc4),
    red: rgb(0xcf, 0x6d, 0x61),
    purple: rgb(0x9a, 0x8f, 0xc4),
    danger: rgb(0xd4, 0x65, 0x5a),
    danger_soft: rgb(0x2c, 0x1c, 0x19),
    danger_border: rgb(0x4c, 0x30, 0x2b),
    shadow: Color::BLACK,
};

pub const ACCENTS: [Color; 3] = [
    rgb(0xa0, 0x5a, 0x3c),
    rgb(0x3d, 0x63, 0xb8),
    rgb(0x3f, 0x7d, 0x54),
];

pub const fn palette(mode: Mode) -> &'static Palette {
    match mode {
        Mode::Light => &LIGHT,
        Mode::Dark => &DARK,
    }
}

pub fn iced_theme(mode: Mode, accent: Color) -> Theme {
    let p = palette(mode);
    Theme::custom(
        match mode {
            Mode::Light => "Ducktape Light",
            Mode::Dark => "Ducktape Dark",
        },
        iced::theme::Palette {
            background: p.paper,
            text: p.ink,
            primary: accent,
            success: p.green,
            warning: p.amber,
            danger: p.danger,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_toggle_is_reversible() {
        assert_eq!(Mode::Light.toggled(), Mode::Dark);
        assert_eq!(Mode::Light.toggled().toggled(), Mode::Light);
    }

    #[test]
    fn palettes_keep_design_contract_anchors() {
        assert_eq!(LIGHT.titlebar, rgb(0xf1, 0xf1, 0xf1));
        assert_eq!(DARK.titlebar, rgb(0x24, 0x23, 0x1f));
        assert_eq!(ACCENTS[0], rgb(0xa0, 0x5a, 0x3c));
    }

    #[test]
    fn small_muted_text_meets_contrast_on_app_surfaces() {
        fn luminance(color: Color) -> f32 {
            let linear = |channel: f32| {
                if channel <= 0.04045 {
                    channel / 12.92
                } else {
                    ((channel + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
        }
        fn contrast(a: Color, b: Color) -> f32 {
            let (light, dark) = if luminance(a) > luminance(b) {
                (luminance(a), luminance(b))
            } else {
                (luminance(b), luminance(a))
            };
            (light + 0.05) / (dark + 0.05)
        }

        for palette in [LIGHT, DARK] {
            for foreground in [palette.muted, palette.muted_2, palette.muted_3] {
                for background in [
                    palette.paper,
                    palette.canvas,
                    palette.sidebar,
                    palette.titlebar,
                    palette.hover,
                    palette.sunken,
                    palette.panel,
                    palette.chip,
                    palette.border_soft,
                ] {
                    assert!(contrast(foreground, background) >= 4.5);
                }
            }
        }
        let terminal_overlay = Color {
            r: DARK.canvas.r * 0.94,
            g: DARK.canvas.g * 0.94,
            b: DARK.canvas.b * 0.94,
            a: 1.0,
        };
        for foreground in [DARK.muted_2, DARK.amber, DARK.danger] {
            assert!(contrast(foreground, terminal_overlay) >= 4.5);
        }
    }
}
