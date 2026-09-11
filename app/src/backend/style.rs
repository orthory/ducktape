use super::*;

pub(crate) fn live_update(kind: crate::LiveKind, status: &str, height: i64) -> LiveUpdate {
    LiveUpdate {
        kind,
        status: status.into(),
        height,
        module: String::new(),
        load_chat: kind == crate::LiveKind::Ready,
        debounce: false,
        chat: Vec::new(),
        bell: BellDelta::default(),
        permit: LivePermit::default(),
    }
}

pub(crate) fn live_retry(_message: String) -> LiveUpdate {
    live_update(crate::LiveKind::Retry, "Reconnecting…", -1)
}

/// One plane's module committed something. The handler refetches that plane and
/// nothing else — `module` is the whole payload.
pub(crate) fn live_plane(module: &str, height: i64) -> LiveUpdate {
    let mut update = live_update(
        crate::LiveKind::Plane,
        &format!("Live · block {height}"),
        height,
    );
    update.module = module.to_string();
    update
}

/// A module's replay is unavailable or unfoldable — the handler reloads that
/// module's slices instead of folding.
pub(crate) fn live_resync(module: &str, height: i64) -> LiveUpdate {
    let mut update = live_update(crate::LiveKind::Resync, "Live · resyncing", height);
    update.module = module.to_string();
    update.load_chat = module == "chat";
    update
}

/// The artifact's line icon for `name`, as the SVG bytes the view hands to
/// iced as an in-memory handle. An unknown name renders an empty document.
///
/// BYTES, NOT `str`: the `svg … memory` node feeds its source straight into
/// `svg::Handle::from_memory`, and a `str` source makes codegen emit
/// `(…).as_bytes().to_vec()` — so a `&'static str` became a String and then a
/// second Vec, per icon, per frame, on a surface that mounts dozens of them
/// outside the cached message rows. `bytes` lowers to the Vec the handle wants
/// and the copy happens once.
pub fn icon(name: &str) -> Vec<u8> {
    design::icons::svg(name).as_bytes().to_vec()
}

/// The titlebar's extra left padding. On macOS the window is drawn with a
/// hidden title and a transparent, full-size content view (`app.ice`), so the
/// three traffic lights overlay the content's top-left ~70px — the chain chip
/// must start past them. Zero on every other platform.
pub fn titlebar_inset() -> f64 {
    if cfg!(target_os = "macos") { 68.0 } else { 0.0 }
}

/// Whether the live palette is the dark reading. The generated theme's base
/// text color IS `app_text`, so light text means a dark surface — no theme
/// name string to allocate and compare per style call.
pub(crate) fn theme_is_dark(theme: &iced::Theme) -> bool {
    theme.palette().text.r > 0.5
}

/// The token set matching the live palette reading.
pub(crate) fn app_tokens(theme: &iced::Theme) -> ui_lang_components::ui::theme::Theme {
    if theme_is_dark(theme) {
        ui_lang_components::ui::theme::DARK
    } else {
        ui_lang_components::ui::theme::LIGHT
    }
}

/// Floating menu/popover surface, derived from the shared design tokens.
///
/// `popover`, not `glass.regular`: glass is a TRANSLUCENT role that only reads
/// as a material when the renderer blurs what is behind it, and **iced has no
/// backdrop blur** — the app window is opaque for exactly that reason. Painted
/// without one, a 62%-alpha plate over a message just lets the message through
/// it, so the sentence under a menu item and the item's own label overlapped
/// and both became hard to read. `popover` is the design system's own opaque
/// floating surface; the border and `elevation.popover` carry the lift.
pub fn raised_style(theme: &iced::Theme) -> iced::widget::container::Style {
    let tokens = app_tokens(theme);
    iced::widget::container::Style {
        background: Some(iced::Background::Color(tokens.palette.popover)),
        border: iced::Border {
            color: tokens.palette.border,
            width: 1.0,
            radius: tokens.radius.card.into(),
        },
        shadow: tokens.elevation.popover,
        ..Default::default()
    }
}

pub(crate) const fn block_kind_name(kind: BlockKind) -> &'static str {
    match kind {
        BlockKind::Page => "Page",
        BlockKind::Paragraph => "Text",
        BlockKind::Heading1 => "Heading 1",
        BlockKind::Heading2 => "Heading 2",
        BlockKind::Heading3 => "Heading 3",
        BlockKind::Bulleted => "Bullet",
        BlockKind::Numbered => "Number",
        BlockKind::Todo => "Todo",
        BlockKind::Toggle => "Toggle",
        BlockKind::Quote => "Quote",
        BlockKind::Code => "Code",
        BlockKind::Callout => "Callout",
        BlockKind::Divider => "Divider",
    }
}

pub(crate) fn next_sequence() -> u64 {
    static SEQUENCE: OnceLock<AtomicU64> = OnceLock::new();
    SEQUENCE
        .get_or_init(|| AtomicU64::new(epoch_nanos() as u64))
        .fetch_add(1, Ordering::Relaxed)
}

pub(crate) fn fresh_id(prefix: &str) -> String {
    format!("{prefix}-{}-{}", epoch_nanos(), next_sequence())
}

fn epoch_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

pub(crate) fn hex_encode(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

pub(crate) fn hex_decode(value: &str) -> Result<Vec<u8>, String> {
    let valid = !value.is_empty()
        && value.len() <= ::node::MAX_FRAME_HEX_BYTES
        && value.len().is_multiple_of(2)
        && value.bytes().all(|byte| byte.is_ascii_hexdigit());
    if !valid {
        return Err("ducktape signer returned an invalid frame".into());
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).expect("validated ASCII hex");
            u8::from_str_radix(pair, 16).map_err(|_| "ducktape signer returned invalid hex".into())
        })
        .collect()
}

