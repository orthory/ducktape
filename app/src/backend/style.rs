use super::*;

pub(crate) fn live_update(kind: &str, status: &str, height: i64) -> LiveUpdate {
    LiveUpdate {
        kind: kind.into(),
        status: status.into(),
        height,
        module: String::new(),
        load_chat: kind == "ready",
        load_pages: kind == "ready",
        debounce: false,
        chat: ChatDelta::default(),
        pages: PagesDelta::default(),
        bell: BellDelta::default(),
        forge: ForgeRefresh::default(),
    }
}

pub(crate) fn live_retry(_message: String) -> LiveUpdate {
    live_update("retry", "Reconnecting…", -1)
}

/// A module's replay is unavailable or unfoldable — the handler reloads that
/// module's slices instead of folding.
pub(crate) fn live_resync(module: &str, height: i64) -> LiveUpdate {
    let mut update = live_update("resync", "Live · resyncing", height);
    update.module = module.to_string();
    update.load_chat = module == "chat";
    update.load_pages = module == "pages";
    update
}

/// The artifact's line icon for `name`, as an SVG document the view hands to
/// iced as an in-memory handle. An unknown name renders an empty document.
pub fn icon(name: impl AsRef<str>) -> String {
    design::icons::svg(name.as_ref()).to_string()
}

/// Tints an icon with one step of the artifact's ink ramp. The asset itself is
/// drawn on `currentColor`, so the tone — not a second asset — is what makes a
/// muted rail icon and an accent action icon different.
pub fn icon_tint(
    _theme: &iced::Theme,
    _status: iced::widget::svg::Status,
    tone: impl AsRef<str>,
) -> iced::widget::svg::Style {
    iced::widget::svg::Style {
        color: Some(rgb(design::ink::tone(tone.as_ref()))),
    }
}

/// An artifact hex literal as an opaque iced color.
fn rgb(hex: u32) -> iced::Color {
    iced::Color::from_rgb8(
        ((hex >> 16) & 0xff) as u8,
        ((hex >> 8) & 0xff) as u8,
        (hex & 0xff) as u8,
    )
}

/// Flat paper card derived from the shared design tokens.
pub fn card_style(_theme: &iced::Theme) -> iced::widget::container::Style {
    let tokens = ducktape_ui::ui::theme::LIGHT;
    iced::widget::container::Style {
        background: Some(iced::Background::Color(tokens.palette.card)),
        border: iced::Border {
            color: tokens.palette.border,
            width: 1.0,
            radius: tokens.radius.card.into(),
        },
        ..Default::default()
    }
}

/// Floating menu/popover surface, derived from the shared design tokens.
pub fn raised_style(_theme: &iced::Theme) -> iced::widget::container::Style {
    let tokens = ducktape_ui::ui::theme::LIGHT;
    iced::widget::container::Style {
        background: Some(iced::Background::Color(tokens.glass.regular)),
        border: iced::Border {
            color: tokens.palette.border,
            width: 1.0,
            radius: tokens.radius.card.into(),
        },
        shadow: tokens.elevation.popover,
        ..Default::default()
    }
}

pub(crate) fn short_hex(bytes: &[u8]) -> String {
    let mut output = String::new();
    for byte in bytes.iter().take(4) {
        let _ = write!(output, "{byte:02x}");
    }
    if bytes.len() > 4 {
        output.push('…');
    }
    output
}

/// A shortened display label for an id string: its first 8 characters, with an
/// ellipsis when more follow.
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

pub(crate) fn parse_block_kind(kind: &str) -> Result<BlockKind, String> {
    match kind {
        "Page" => Ok(BlockKind::Page),
        "Text" => Ok(BlockKind::Paragraph),
        "Heading 1" => Ok(BlockKind::Heading1),
        "Heading 2" => Ok(BlockKind::Heading2),
        "Heading 3" => Ok(BlockKind::Heading3),
        "Bullet" => Ok(BlockKind::Bulleted),
        "Number" => Ok(BlockKind::Numbered),
        "Todo" => Ok(BlockKind::Todo),
        "Toggle" => Ok(BlockKind::Toggle),
        "Quote" => Ok(BlockKind::Quote),
        "Code" => Ok(BlockKind::Code),
        "Callout" => Ok(BlockKind::Callout),
        "Divider" => Ok(BlockKind::Divider),
        _ => Err("choose a valid block type".into()),
    }
}

pub(crate) fn bounded_new_block_text(kind: BlockKind, text: String) -> Result<String, String> {
    if kind == BlockKind::Divider {
        return Ok(String::new());
    }
    let field = if kind == BlockKind::Page {
        "page title"
    } else {
        "block text"
    };
    let limit = if kind == BlockKind::Page {
        512
    } else {
        64 * 1024
    };
    if text.trim().is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    bounded_exact_text(text, field, limit)
}

pub(crate) fn bounded_updated_block_text(kind: BlockKind, text: String) -> Result<String, String> {
    if kind == BlockKind::Divider {
        return Ok(String::new());
    }
    if kind == BlockKind::Page {
        return bounded_exact_text(text, "page title", 512);
    }
    bounded_exact_text(text, "block text", 64 * 1024)
}

pub(crate) async fn debounced_page_text(
    rpc: String,
    mut password: String,
    block_id: String,
    text: String,
) -> Result<bool, String> {
    let key = format!("{rpc}\0{block_id}");
    let ticket = begin_autosave(&key);
    tokio::time::sleep(Duration::from_millis(400)).await;
    if !autosave_is_current(&key, ticket) {
        password.zeroize();
        return Ok(false);
    }
    // ponytail: one writer is enough before a live network; shard by RPC only if latency proves it.
    let _writer = autosave_writer().lock().await;
    if !autosave_is_current(&key, ticket) {
        password.zeroize();
        return Ok(false);
    }
    let result = async {
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "pages",
            pages::encode_msg(&PageMsg::UpdateText {
                block_id,
                text: text.clone(),
                marks: None,
            }),
            password,
        )
        .await
    }
    .await;
    let current = finish_autosave(&key, ticket);
    if !current {
        return Ok(false);
    }
    result?;
    Ok(true)
}

pub(crate) fn autosaves() -> &'static std::sync::Mutex<BTreeMap<String, u64>> {
    static AUTOSAVES: OnceLock<std::sync::Mutex<BTreeMap<String, u64>>> = OnceLock::new();
    AUTOSAVES.get_or_init(Default::default)
}

fn autosave_writer() -> &'static tokio::sync::Mutex<()> {
    static WRITER: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    WRITER.get_or_init(Default::default)
}

pub fn cancel_autosaves(rpc: String, generation: i64) -> i64 {
    let prefix = format!("{}\0", rpc.trim());
    autosaves()
        .lock()
        .expect("autosave lock poisoned")
        .retain(|key, _| !key.starts_with(&prefix));
    generation.saturating_add(1)
}

pub(crate) fn begin_autosave(key: &str) -> u64 {
    static TICKET: AtomicU64 = AtomicU64::new(1);
    let ticket = TICKET.fetch_add(1, Ordering::Relaxed);
    autosaves()
        .lock()
        .expect("autosave lock poisoned")
        .insert(key.to_string(), ticket);
    ticket
}

pub(crate) fn autosave_is_current(key: &str, ticket: u64) -> bool {
    autosaves().lock().expect("autosave lock poisoned").get(key) == Some(&ticket)
}

pub(crate) fn finish_autosave(key: &str, ticket: u64) -> bool {
    let mut autosaves = autosaves().lock().expect("autosave lock poisoned");
    let is_current = autosaves.get(key) == Some(&ticket);
    if !is_current {
        return false;
    }
    autosaves.remove(key);
    true
}

pub(crate) fn block_move(
    blocks: &[pages::Block],
    block_id: &str,
    direction: &str,
) -> Result<(Option<String>, Option<String>), String> {
    let block = blocks
        .iter()
        .find(|block| block.id == block_id)
        .ok_or_else(|| "block was not found".to_string())?;
    let parent_id = block
        .parent
        .as_deref()
        .ok_or_else(|| "top-level pages cannot move inside their own document".to_string())?;
    let parent = blocks
        .iter()
        .find(|block| block.id == parent_id)
        .ok_or_else(|| "block parent was not found".to_string())?;
    let index = parent
        .children
        .iter()
        .position(|child| child == block_id)
        .ok_or_else(|| "block is missing from its parent".to_string())?;
    match direction {
        "up" if index > 0 => Ok((
            Some(parent.id.clone()),
            index
                .checked_sub(2)
                .map(|index| parent.children[index].clone()),
        )),
        "down" if index + 1 < parent.children.len() => Ok((
            Some(parent.id.clone()),
            Some(parent.children[index + 1].clone()),
        )),
        "indent" if index > 0 => {
            let new_parent = blocks
                .iter()
                .find(|block| block.id == parent.children[index - 1])
                .ok_or_else(|| "previous block was not found".to_string())?;
            Ok((
                Some(new_parent.id.clone()),
                new_parent.children.last().cloned(),
            ))
        }
        "outdent" => {
            let promotes_page = block.kind == BlockKind::Page && parent.parent.is_none();
            if promotes_page {
                return Ok((None, None));
            }
            let grandparent = parent
                .parent
                .clone()
                .ok_or_else(|| "block is already at the top level".to_string())?;
            Ok((Some(grandparent), Some(parent.id.clone())))
        }
        "up" => Err("block is already first".into()),
        "down" => Err("block is already last".into()),
        "indent" => Err("block needs a previous sibling to indent under".into()),
        _ => Err("choose a valid block move".into()),
    }
}

pub(crate) fn bounded_detail(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        return "no detail".into();
    }
    value.chars().take(300).collect()
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
        && value.len() <= MAX_FRAME_HEX_BYTES
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
