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
    /// the embedded files, relative to this crate's root — what `app.ice`
    /// points its `font` settings at.
    pub const ASSETS: [&str; 2] = [
        "assets/fonts/Geist[wght].ttf",
        "assets/fonts/GeistMono[wght].ttf",
    ];
}

/// The product type roles from the canonical Ducktape design artifact. The
/// drift guard walks every app-authored `.ice` source and rejects other sizes.
pub mod type_scale {
    pub const MICRO: f64 = 7.5;
    pub const BADGE: f64 = 9.0;
    pub const NAV: f64 = 9.5;
    pub const FIELD_LABEL: f64 = 10.0;
    pub const MACHINE_META: f64 = 10.5;
    pub const META: f64 = 11.0;
    pub const CONTROL: f64 = 11.5;
    pub const MACHINE: f64 = 12.0;
    pub const CAPTION: f64 = 12.5;
    pub const LIST: f64 = 13.0;
    pub const BODY: f64 = 13.5;
    pub const PANE_HEADER: f64 = 14.0;
    pub const HEADING: f64 = 14.5;
    pub const SECTION: f64 = 16.0;
    pub const SCREEN_TITLE: f64 = 20.0;
    pub const DISPLAY: f64 = 22.0;

    /// every legal `size=` literal in `.ice` sources, the guard's whitelist.
    /// `1.0` is the established off-screen focus-shim size, not a text step.
    pub const ALL: [f64; 17] = [
        MICRO,
        BADGE,
        NAV,
        FIELD_LABEL,
        MACHINE_META,
        META,
        CONTROL,
        MACHINE,
        CAPTION,
        LIST,
        BODY,
        PANE_HEADER,
        HEADING,
        SECTION,
        SCREEN_TITLE,
        DISPLAY,
        1.0,
    ];
}

/// The line icon set, lifted glyph-for-glyph out of the canonical design
/// artifact. Every file is a 24×24 stroke drawing on `currentColor`, so one
/// asset serves every tone: the view tints it through the `icon_tint` SVG
/// style rather than shipping a per-color copy.
pub mod icons {
    /// The SVG source for `name`, or an empty document when the name is not in
    /// the set. An unknown name renders nothing instead of panicking a view.
    pub fn svg(name: &str) -> &'static str {
        match name {
            "agent-tile" => include_str!("../assets/icons/agent-tile.svg"),
            "arrow-right" => include_str!("../assets/icons/arrow-right.svg"),
            "bell" => include_str!("../assets/icons/bell.svg"),
            "branch" => include_str!("../assets/icons/branch.svg"),
            "brightness" => include_str!("../assets/icons/brightness.svg"),
            "check" => include_str!("../assets/icons/check.svg"),
            "chevron-down" => include_str!("../assets/icons/chevron-down.svg"),
            "chevron-right" => include_str!("../assets/icons/chevron-right.svg"),
            "code-brackets" => include_str!("../assets/icons/code-brackets.svg"),
            "code-slash" => include_str!("../assets/icons/code-slash.svg"),
            "collapse" => include_str!("../assets/icons/collapse.svg"),
            "copy" => include_str!("../assets/icons/copy.svg"),
            "copy-lg" => include_str!("../assets/icons/copy-lg.svg"),
            "doc" => include_str!("../assets/icons/doc.svg"),
            "emoji" => include_str!("../assets/icons/emoji.svg"),
            "external" => include_str!("../assets/icons/external.svg"),
            "file" => include_str!("../assets/icons/file.svg"),
            "folder" => include_str!("../assets/icons/folder.svg"),
            "gear" => include_str!("../assets/icons/gear.svg"),
            "gear-rays" => include_str!("../assets/icons/gear-rays.svg"),
            "headphones" => include_str!("../assets/icons/headphones.svg"),
            "inline-ref" => include_str!("../assets/icons/inline-ref.svg"),
            "issue-closed" => include_str!("../assets/icons/issue-closed.svg"),
            "issue-open" => include_str!("../assets/icons/issue-open.svg"),
            "link" => include_str!("../assets/icons/link.svg"),
            "list" => include_str!("../assets/icons/list.svg"),
            "lock" => include_str!("../assets/icons/lock.svg"),
            "mic" => include_str!("../assets/icons/mic.svg"),
            "mic-off" => include_str!("../assets/icons/mic-off.svg"),
            "modules" => include_str!("../assets/icons/modules.svg"),
            "nav-agents" => include_str!("../assets/icons/nav-agents.svg"),
            "nav-chat" => include_str!("../assets/icons/nav-chat.svg"),
            "nav-explorer" => include_str!("../assets/icons/nav-explorer.svg"),
            "nav-files" => include_str!("../assets/icons/nav-files.svg"),
            "nav-forge" => include_str!("../assets/icons/nav-forge.svg"),
            "nav-members" => include_str!("../assets/icons/nav-members.svg"),
            "nav-pages" => include_str!("../assets/icons/nav-pages.svg"),
            "node" => include_str!("../assets/icons/node.svg"),
            "pin" => include_str!("../assets/icons/pin.svg"),
            "plus" => include_str!("../assets/icons/plus.svg"),
            "plus-lg" => include_str!("../assets/icons/plus-lg.svg"),
            "popout" => include_str!("../assets/icons/popout.svg"),
            "pull-request" => include_str!("../assets/icons/pull-request.svg"),
            "quote" => include_str!("../assets/icons/quote.svg"),
            "screen-share" => include_str!("../assets/icons/screen-share.svg"),
            "search" => include_str!("../assets/icons/search.svg"),
            "search-lg" => include_str!("../assets/icons/search-lg.svg"),
            "shield" => include_str!("../assets/icons/shield.svg"),
            "shield-check" => include_str!("../assets/icons/shield-check.svg"),
            _ => EMPTY,
        }
    }

    /// What an unknown name renders: a valid, invisible document.
    pub const EMPTY: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"></svg>"#;
}

/// The artifact's ink ramp — the text/stroke colors that fade back as content
/// recedes. `ducktape-ui` owns the semantic palette; these are the extra steps
/// the artifact names but a semantic role has no word for.
pub mod ink {
    /// The darkest ink: brand tiles, primary fills, the strongest headings.
    pub const INK: u32 = 0x26251f;
    /// `ink` under the pointer.
    pub const INK_HOVER: u32 = 0x322f28;
    /// Body copy.
    pub const BODY: u32 = 0x2c2b27;
    /// Emphasised body — list titles, active labels.
    pub const STRONG: u32 = 0x3f3e39;
    /// Machine values: hashes, heights, endpoints.
    pub const MONO: u32 = 0x5e5c55;
    /// Secondary copy.
    pub const MUTED: u32 = 0x6b6962;
    /// Supporting explanation under a title.
    pub const CAPTION: u32 = 0x9a988f;
    /// Timestamps, counts, key fragments.
    pub const META: u32 = 0xa7a59b;
    /// Placeholder text.
    pub const HINT: u32 = 0xb3b1a8;
    /// All-caps field labels and section numbers.
    pub const LABEL: u32 = 0xbdbbb1;
    /// The avatar plate behind initials.
    pub const AVATAR: u32 = 0xd2d0c7;
    /// A rail icon nobody is pointing at.
    pub const IDLE: u32 = 0xcbc9bf;
    /// The label under the rail icon you are on.
    pub const STRONG_INK: u32 = 0x3a3934;
    /// The single accent.
    pub const ACCENT: u32 = 0xa05a3c;
    /// Status inks.
    pub const SUCCESS: u32 = 0x5f9e74;
    /// The lighter success tick used on progress marks.
    pub const SUCCESS_TICK: u32 = 0x7ba78c;
    /// Pending/waiting ink.
    pub const WARNING: u32 = 0xa07b32;
    /// Refusal/destructive ink.
    pub const DANGER: u32 = 0xb8544c;
    /// Paper, for ink drawn on a dark plate.
    pub const PAPER: u32 = 0xf3f1ea;

    /// The ramp keyed by the artifact's own name for each step. An unknown
    /// tone falls back to `MUTED` so a view never renders an invisible icon.
    pub fn tone(name: &str) -> u32 {
        match name {
            "ink" => INK,
            "ink-hover" => INK_HOVER,
            "body" => BODY,
            "strong" => STRONG,
            "mono" => MONO,
            "muted" => MUTED,
            "caption" => CAPTION,
            "meta" => META,
            "hint" => HINT,
            "label" => LABEL,
            "avatar" => AVATAR,
            "idle" => IDLE,
            "strong-ink" => STRONG_INK,
            "accent" => ACCENT,
            "success" => SUCCESS,
            "success-tick" => SUCCESS_TICK,
            "warning" => WARNING,
            "danger" => DANGER,
            "paper" => PAPER,
            _ => MUTED,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_asset_is_routed_and_every_route_resolves() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icons");
        let mut count = 0;
        for entry in std::fs::read_dir(&dir).expect("icon directory unreadable") {
            let path = entry.expect("icon entry unreadable").path();
            let name = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .expect("icon file name is not utf-8");
            let routed = icons::svg(name);
            assert_ne!(
                routed,
                icons::EMPTY,
                "icon asset {name}.svg has no arm in icons::svg"
            );
            assert!(
                routed.starts_with("<svg") && routed.trim_end().ends_with("</svg>"),
                "icon {name} is not a standalone svg document"
            );
            count += 1;
        }
        assert!(count >= 49, "expected the full artifact icon set, saw {count}");
    }

    #[test]
    fn an_unknown_icon_renders_nothing_instead_of_panicking() {
        assert_eq!(icons::svg("no-such-icon"), icons::EMPTY);
    }

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
