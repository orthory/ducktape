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
        Ok(explorer_window(generation, &rows))
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// The `GET /v1/blocks` rows as the screen holds them — newest first, and
/// OP-CARRYING ONLY.
///
/// The endpoint is NOT uniformly filtered, which is the whole reason this gate
/// lives here. Three of the four writers of a block row drop a block that
/// carried nothing: `bin/noded`'s projection stores `record: None` when
/// `ops.is_empty()`, `bin/node`'s boot fold re-runs the identical gate, and the
/// embedded daemon lane is one-op-per-block by construction. The fourth does
/// not. A node that follows from a checkpoint writes ONE `boundary_block_row`
/// (`bin/node/src/explorer.rs`, applied in `replica/park.rs`) at its ascension
/// tip, with `hash: ""` and `ops: []` — the boundary it verified, not a block
/// it folded. That is not an exotic lane: `bin/node/src/main.rs` routes every
/// key that is neither a validator nor seated by the checkpoint into
/// `replica::run`, which is every joined member until promotion, and on a fresh
/// join that row is the ONLY row until the first op-carrying block finalizes.
///
/// Displayed, it is a row that contradicts its own screen: a blank hash column
/// and `0 ops` directly under a subtitle saying these are the blocks that
/// carried operations, and clicking it opens an empty detail pane, because
/// `explorer_ops_at` has nothing to hand it. Its two real fields are not lost
/// by dropping it — the height and the root hash are what the titlebar's status
/// card already prints (`height_label` and `app-hash`). The node keeps writing
/// the row: it is a truthful record of the one thing a follower observed, and
/// it carries the blocks watermark (`IndexStore::apply_block_record`). The
/// reader of a set is the one that has to agree with the name it prints.
pub(crate) fn explorer_window(generation: i64, rows: &[serde_json::Value]) -> ExplorerData {
    let mut blocks = Vec::with_capacity(rows.len());
    let mut ops = Vec::new();
    for row in rows {
        let height = row["height"].as_i64().unwrap_or(0);
        let row_ops = row["ops"].as_array().map(Vec::as_slice).unwrap_or_default();
        // the follower's boundary marker (and any future op-less row): not a
        // block that carried operations, so not in a list that says it is.
        if row_ops.is_empty() {
            continue;
        }
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
    ExplorerData {
        generation,
        blocks,
        ops,
    }
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

/// The dispatch trace summary: one hop per module the op reached, each naming
/// what it emitted. The counts come straight off `host::DispatchRecord` —
/// `emitted_msgs` is "count of follow-up `Msg`s this dispatch emitted (the
/// causal fan-out)", `emitted_events` "count of observability `Event`s" — so
/// the units are spelled the way the fields are named. This rendered
/// `chat(+0m/+0e)` before, a private shorthand nothing on the screen expanded:
/// `m`/`e` are not words, and a reader who has not read `crates/kernel/host`
/// has no way to recover them. The counts join their nouns through `plural`,
/// the app's one count-label seam, so `1 msg` never renders as `1 msgs`.
pub(crate) fn explorer_trace(operations: Option<&Vec<serde_json::Value>>) -> String {
    let Some(operations) = operations else {
        return String::new();
    };
    operations
        .iter()
        .map(|op| {
            let module = op["module"].as_str().unwrap_or("?");
            let msgs = op["emitted_msgs"].as_i64().unwrap_or(0);
            let events = op["emitted_events"].as_i64().unwrap_or(0);
            let emitted_msgs = plural(msgs, "msg".into(), "msgs".into());
            let emitted_events = plural(events, "event".into(), "events".into());
            format!("{module} · {emitted_msgs} · {emitted_events}")
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

/// The transient layer currently over the console's content — the TOPMOST one
/// only — or `""` when the content itself is the frontmost thing on screen.
/// The order IS the z-order.
///
/// ONE enumeration, two readers. Escape asks it which surface to close;
/// the keyboard scroll asks it only whether anything at all sits over the pane
/// it would otherwise move. Every keyboard route that ignores this list routes
/// a key at the screen BEHIND the layer the reader is looking at.
//
// One argument per layer: the Ice extern surface is flat, and the reading must
// see every layer at once to name the topmost.
#[allow(clippy::too_many_arguments)]
pub fn topmost_overlay(
    palette_open: bool,
    bell_open: bool,
    channel_create_open: bool,
    thread_message_action: String,
    message_action: String,
    channel_settings_open: bool,
    forge_repo_menu: bool,
) -> String {
    if palette_open {
        return "palette".into();
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

/// The surface Escape dismisses — the topmost transient layer, minus the one
/// rung Escape does not own. Menus, popovers, the create modal and the bell
/// close; persistent rails (thread, comments, the channel drawer) keep their
/// explicit × — closing one from a global key would also have to adjudicate
/// its half-typed drafts.
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
    if not_escape {
        return String::new();
    }
    let topmost = topmost_overlay(
        palette_open,
        bell_open,
        channel_create_open,
        thread_message_action,
        message_action,
        channel_settings_open,
        forge_repo_menu,
    );
    // `palette_key_action` owns the palette's keys — an open palette swallows
    // Escape, so the ladder yields rather than naming a rung.
    let palette_owns_it = topmost == "palette";
    if palette_owns_it {
        return String::new();
    }
    topmost
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
/// Larger than any content the console can stack. `scroll_by` clamps the result
/// to the pane's own extent, so this IS Home/End — no content measurement, no
/// second operation kind.
const KEY_SCROLL_EXTREME: f64 = 1.0e9;

/// The vertical scroll, in pixels, that a key press asks the console's content
/// pane for — `0.0` for every key the pane does not own.
///
/// iced's `scrollable` answers the wheel, the drag rail and touch; it has no
/// focus and no keyboard handling at all (0.14's widget matches on
/// `Event::Keyboard` only to track modifiers for shift-wheel), so the app is
/// the only layer that can route a keyboard scroll at one. THIS is the whole
/// decision — three conditions, in one place, rather than a guard per key:
///
/// 1. **A focused widget's key is not the pane's.** The subscription feeding
///    this is `status=ignored`, so anything a focused widget consumed never
///    arrives — which covers Home/End (iced's `text_input` captures both,
///    `text_input.rs:1119`/`1139`) and every key the rich composers take. It
///    does NOT cover the arrows: single-line `text_input` falls through
///    Up/Down to `_ => {}` (`text_input.rs:1245`) without capturing them, so a
///    pane that claimed an arrow scrolled the page out from under a live
///    caret. Nothing in this stack can read widget focus (ui-lang has no focus
///    predicate — see the same note on `composer_focus`), so the pane cannot
///    tell an arrow meant for an input from one meant for itself, and an arrow
///    inside an input belongs to the input. The pane claims only Page Up/Down
///    and Home/End: keys no focused widget in this console owns silently.
/// 2. **A transient layer's key is not the pane's.** `topmost_overlay` is the
///    reading — with the palette or the bell up, the pane the reader can see
///    is not the one this would move.
/// 3. **A chord is not the pane's.** Any modifier disqualifies the press;
///    chords belong to their own routers (`palette_key_action`, the composer
///    marks, the page history), and a keyboard-selection chord like
///    Shift+PageDown must not also move the pane.
pub fn content_scroll_step(
    logical: iced::keyboard::Key,
    modifiers: iced::keyboard::Modifiers,
    overlay: String,
) -> f64 {
    use iced::keyboard::{Key, key::Named};
    let layer_over_the_pane = !overlay.is_empty();
    let chord = !modifiers.is_empty();
    if layer_over_the_pane || chord {
        return 0.0;
    }
    match logical {
        Key::Named(Named::PageDown) => KEY_PAGE_STEP,
        Key::Named(Named::PageUp) => -KEY_PAGE_STEP,
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
