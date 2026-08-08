use super::*;

/// One explorer block row.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ExplorerBlock {
    pub height: i64,
    pub hash: String,
    pub commit: String,
    pub op_count: i64,
}

/// One applied (or rejected) op inside an explorer block.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ExplorerOp {
    pub height: i64,
    pub proposer: String,
    pub target: String,
    pub disposition: String,
    pub op_hash: String,
    pub payload: String,
    pub trace: String,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ExplorerData {
    pub generation: i64,
    pub blocks: Vec<ExplorerBlock>,
    pub ops: Vec<ExplorerOp>,
}

/// Load the recent block window for the explorer pane, newest first.
pub async fn load_explorer(rpc: String, generation: i64) -> Result<ExplorerData, HydrationError> {
    offscreen_guard(generation)?;
    async {
        let rpc = rpc_client(&rpc)?;
        let rows = rpc.blocks(100).await?;
        let mut blocks = Vec::with_capacity(rows.len());
        let mut ops = Vec::new();
        for row in &rows {
            let height = row["height"].as_i64().unwrap_or(0);
            let row_ops = row["ops"].as_array().cloned().unwrap_or_default();
            blocks.push(ExplorerBlock {
                height,
                hash: short_digest(row["hash"].as_str().unwrap_or_default()),
                commit: short_digest(row["commit_hash"].as_str().unwrap_or_default()),
                op_count: count_i64(row_ops.len()),
            });
            for op in row_ops {
                ops.push(ExplorerOp {
                    height,
                    proposer: short_digest(op["proposer"].as_str().unwrap_or_default()),
                    target: op["target"].as_str().unwrap_or_default().to_string(),
                    disposition: op["disposition"].as_str().unwrap_or_default().to_string(),
                    op_hash: short_digest(op["op_hash"].as_str().unwrap_or_default()),
                    payload: explorer_payload(&op["payload"]),
                    trace: explorer_trace(op["operations"].as_array()),
                });
            }
        }
        blocks.reverse();
        ops.reverse();
        Ok(ExplorerData {
            generation,
            blocks,
            ops,
        })
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// First 12 hex chars of a digest — the explorer's display form.
pub(crate) fn short_digest(digest: &str) -> String {
    let mut short: String = digest.chars().take(12).collect();
    if digest.chars().count() > 12 {
        short.push('…');
    }
    short
}

/// The op payload preview: verbatim short strings, else a truncated render.
fn explorer_payload(payload: &serde_json::Value) -> String {
    let rendered = match payload.as_str() {
        Some(text) => text.to_string(),
        None => payload.to_string(),
    };
    let mut preview: String = rendered.chars().take(160).collect();
    if rendered.chars().count() > 160 {
        preview.push('…');
    }
    preview
}

/// The dispatch trace summary: `module(+msgs/+events)` per hop.
fn explorer_trace(operations: Option<&Vec<serde_json::Value>>) -> String {
    let Some(operations) = operations else {
        return String::new();
    };
    operations
        .iter()
        .map(|op| {
            let module = op["module"].as_str().unwrap_or("?");
            let msgs = op["emitted_msgs"].as_i64().unwrap_or(0);
            let events = op["emitted_events"].as_i64().unwrap_or(0);
            format!("{module}(+{msgs}m/+{events}e)")
        })
        .collect::<Vec<_>>()
        .join(" → ")
}

/// The ops of the selected block (0 selects nothing).
pub fn explorer_ops_at(ops: Vec<ExplorerOp>, height: i64) -> Vec<ExplorerOp> {
    ops.into_iter().filter(|op| op.height == height).collect()
}

/// The global-key router for the command palette: platform-Command+K
/// toggles, Escape closes an open palette; anything else is `none`.
pub fn palette_key_action(
    logical: iced::keyboard::Key,
    physical: iced::keyboard::key::Physical,
    modifiers: iced::keyboard::Modifiers,
    open: bool,
) -> String {
    use iced::keyboard::{
        Key,
        key::{Code, Named, Physical},
    };
    let is_toggle = modifiers.command() && physical == Physical::Code(Code::KeyK);
    if is_toggle {
        return match open {
            true => "close".into(),
            false => "open".into(),
        };
    }
    if open && logical == Key::Named(Named::Escape) {
        return "close".into();
    }
    "none".into()
}

/// The surface Escape dismisses — the TOPMOST transient layer only, and the
/// ladder's order IS the z-order. Menus, popovers, the create modal and the
/// bell close; persistent rails (thread, comments, the channel drawer) keep
/// their explicit × — closing one from a global key would also have to
/// adjudicate its half-typed drafts. The palette is absent on purpose:
/// `palette_key_action` owns its keys, and an open palette swallows Escape.
//
// One argument per closable surface: the Ice extern surface is flat, and the
// ladder must see every layer at once to name the topmost.
#[allow(clippy::too_many_arguments)]
pub fn escape_target(
    logical: iced::keyboard::Key,
    palette_open: bool,
    bell_open: bool,
    channel_create_open: bool,
    thread_message_action: String,
    message_action: String,
    channel_settings_open: bool,
    forge_repo_menu: bool,
) -> String {
    use iced::keyboard::{Key, key::Named};
    let not_escape = logical != Key::Named(Named::Escape);
    if not_escape || palette_open {
        return String::new();
    }
    if bell_open {
        return "bell".into();
    }
    if channel_create_open {
        return "channel_create".into();
    }
    if thread_message_action != "toolbar" {
        return "thread_menu".into();
    }
    if message_action != "toolbar" {
        return "message_menu".into();
    }
    // BELOW both message menus, which float over the drawer, and above the
    // repo menu, which lives on another tab. The drawer had no rung at all: it
    // shipped with an `×` and no keyboard exit while every other overlay in the
    // app answered Escape. Measured on the running app — Escape over an open
    // Channel details changed exactly zero pixels.
    if channel_settings_open {
        return "channel_settings".into();
    }
    // The pages block-actions menu and insert row used to sit here. The page
    // document has neither: there is no transient layer over the canvas to
    // dismiss, and the comments rail is a persistent panel with its own close.
    if forge_repo_menu {
        return "repo_menu".into();
    }
    String::new()
}

/// One keyboard "page", in pixels. iced's scrollable reports its viewport only
/// through `on_scroll`, so a pane that has never been scrolled cannot tell us
/// its own height and there is nothing to page BY — a constant is the only
/// reading available. It is deliberately shorter than the shortest content band
/// the console can render (the window's own minimum is 820x540, less a 40px
/// titlebar and a 50px screen header), so Page Down always overlaps and can
/// never skip past unread content. Undershooting costs a keypress; overshooting
/// loses text.
const KEY_PAGE_STEP: f64 = 400.0;
/// One arrow-key step: about three lines of body text.
const KEY_LINE_STEP: f64 = 60.0;
/// Larger than any content the console can stack. `scroll_by` clamps the result
/// to the pane's own extent, so this IS Home/End — no content measurement, no
/// second operation kind.
const KEY_SCROLL_EXTREME: f64 = 1.0e9;

/// The vertical scroll, in pixels, that a key press asks the console's content
/// pane for — `0.0` for every key that is not a scroll key.
///
/// iced's `scrollable` answers the wheel, the drag rail and touch; it has no
/// focus and no keyboard handling at all (0.14's widget matches on
/// `Event::Keyboard` only to track modifiers for shift-wheel), so Page
/// Down/Up, Home/End and the arrows moved nothing anywhere in the console. The
/// app is the only layer that can route them, and the subscription that feeds
/// this one is `status=ignored`: a key a focused widget already consumed —
/// Home inside a text field, an arrow inside an open combo box — never reaches
/// here, so the pane scroll is strictly the fallback for a key nothing wanted.
///
/// Any modifier disqualifies the press. Chords belong to their own routers
/// (`palette_key_action`, the composer marks, the page history), and a
/// keyboard-selection chord like Shift+PageDown must not also move the pane.
pub fn content_scroll_step(
    logical: iced::keyboard::Key,
    modifiers: iced::keyboard::Modifiers,
) -> f64 {
    use iced::keyboard::{Key, key::Named};
    if !modifiers.is_empty() {
        return 0.0;
    }
    match logical {
        Key::Named(Named::PageDown) => KEY_PAGE_STEP,
        Key::Named(Named::PageUp) => -KEY_PAGE_STEP,
        Key::Named(Named::ArrowDown) => KEY_LINE_STEP,
        Key::Named(Named::ArrowUp) => -KEY_LINE_STEP,
        Key::Named(Named::End) => KEY_SCROLL_EXTREME,
        Key::Named(Named::Home) => -KEY_SCROLL_EXTREME,
        _ => 0.0,
    }
}

/// True when the live connection is in a state the shell should banner:
/// the stream is down, retrying, or a resync failed and is backing off.
pub fn connection_degraded(status: String) -> bool {
    status == "Offline"
        || status == "Sync delayed"
        || status == "Reconnecting…"
        || status == "Live · resyncing"
}

pub fn canonical_endpoint(input: String) -> String {
    let configured = input.trim();
    rpc_client(configured)
        .map(|rpc| rpc.origin().to_string())
        .unwrap_or_else(|_| configured.to_string())
}
