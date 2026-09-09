use super::*;

/// One explorer block row.
#[derive(Clone, Debug, Hash, PartialEq, serde::Serialize)]
pub struct ExplorerBlock {
    pub height: i64,
    pub hash: String,
    pub commit: String,
    pub op_count: i64,
}

/// One applied (or rejected) op inside an explorer block.
#[derive(Clone, Debug, Hash, PartialEq, serde::Serialize)]
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
        // WHOLE, as the node prints them (bare lowercase hex). The view adds
        // the `0x` and shows every character; a digest shortened HERE is one
        // no screen can ever recover, and the byte identity the node published
        // is what has to cross.
        blocks.push(ExplorerBlock {
            height,
            hash: row["hash"].as_str().unwrap_or_default().to_string(),
            commit: row["commit_hash"].as_str().unwrap_or_default().to_string(),
            op_count: count_i64(row_ops.len()),
        });
        for op in row_ops {
            ops.push(ExplorerOp {
                height,
                proposer: op["proposer"].as_str().unwrap_or_default().to_string(),
                target: op["target"].as_str().unwrap_or_default().to_string(),
                disposition: op["disposition"].as_str().unwrap_or_default().to_string(),
                // the `GET /v1/files/blob/{op_hash}` key
                op_hash: op["op_hash"].as_str().unwrap_or_default().to_string(),
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

/// First 12 hex chars of a digest — the display form where a screen has no
/// detail to show the whole value in.
pub(crate) fn short_digest(digest: &str) -> String {
    let mut short: String = digest.chars().take(12).collect();
    if digest.chars().count() > 12 {
        short.push('…');
    }
    short
}

/// The op payload, pretty-printed when it parses as JSON. The node already
/// bounds what it sends (`payload_preview` caps the projection at 1024 chars),
/// so the card holds the whole thing it was given — a preview the node cut
/// mid-document fails the parse here and renders verbatim, ellipsis and all.
fn explorer_payload(payload: &serde_json::Value) -> String {
    let Some(text) = payload.as_str() else {
        // already-structured JSON (no projection in between): print it readably.
        let mut parsed = payload.clone();
        hex_byte_arrays(&mut parsed);
        return serde_json::to_string_pretty(&parsed).unwrap_or_else(|_| payload.to_string());
    };
    let Ok(mut parsed) = serde_json::from_str::<serde_json::Value>(text) else {
        return text.to_string();
    };
    hex_byte_arrays(&mut parsed);
    serde_json::to_string_pretty(&parsed).unwrap_or_else(|_| text.to_string())
}

/// How many bytes an array has to carry before this reads it as a digest.
/// Sixteen is the shortest width anything is ever called a digest at; what
/// actually crosses this lane is wider — a git object id is 20, a sha256 and
/// an ed25519 key are 32. Below it, an array of small numbers is far likelier
/// to be a list of small numbers.
const DIGEST_BYTES_MIN: usize = 16;

/// Rewrite every digest-shaped byte array in a payload as `0x…` hex, in place.
///
/// A module message carries its digests as `Vec<u8>` — forge's `prev_oid` and
/// `new_oid`, runs' `recipe_hash`, the registry's `code_hash` — and serde
/// prints those as decimal arrays, so a payload card showed thirty-two lines
/// of three-digit numbers where a hash belongs: not comparable with the block
/// and op hashes beside it, and not pasteable at anything. The same value in
/// the same notation as every other digest on the screen is the whole point.
///
/// THE SHAPE IS THE WHOLE TEST — every element a byte, at least
/// [`DIGEST_BYTES_MIN`] of them — because the field NAMES are the modules',
/// and this reads payloads from all of them. A genuine list of that many
/// small numbers would render as hex too; nothing that reaches this lane
/// produces one, and if something ever does, the module names its digest
/// field rather than this growing a dictionary of the ones it knows.
fn hex_byte_arrays(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(items) => {
            if let Some(bytes) = digest_bytes(items) {
                *value = serde_json::Value::String(format!("0x{}", hex_encode(&bytes)));
                return;
            }
            for item in items {
                hex_byte_arrays(item);
            }
        }
        serde_json::Value::Object(fields) => {
            for (_, field) in fields {
                hex_byte_arrays(field);
            }
        }
        _ => {}
    }
}

/// The bytes this array carries, when every element is one and there are
/// enough of them to be a digest.
fn digest_bytes(items: &[serde_json::Value]) -> Option<Vec<u8>> {
    if items.len() < DIGEST_BYTES_MIN {
        return None;
    }
    items
        .iter()
        .map(|item| u8::try_from(item.as_u64()?).ok())
        .collect()
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
            let emitted_msgs = plural(msgs, "msg", "msgs");
            let emitted_events = plural(events, "event", "events");
            format!("{module} · {emitted_msgs} · {emitted_events}")
        })
        .collect::<Vec<_>>()
        .join(" → ")
}

/// The ops of the selected block (0 selects nothing).
pub fn explorer_ops_at(ops: &[ExplorerOp], height: i64) -> Vec<ExplorerOp> {
    ops.iter()
        .filter(|op| op.height == height)
        .cloned()
        .collect()
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

/// Is the command modifier down? The cheap half of the quit chord: it is read
/// off the modifier stream to ARM the key-press route, so an ordinary keystroke
/// never publishes one. It asks the same `command()` [`quit_chord`] judges by —
/// Command on a Mac, Control elsewhere — because arming on one modifier and
/// judging on another yields a chord that can never fire.
pub fn command_held(modifiers: iced::keyboard::Modifiers) -> bool {
    modifiers.command()
}

/// Is ⇧ down right now? A press carries no modifiers of its own, so the chat's
/// shift-click reads the arming this fills instead of the app growing a second
/// key route to learn the same fact.
pub fn shift_held(modifiers: iced::keyboard::Modifiers) -> bool {
    modifiers.shift()
}

/// Which command chord this press is (⌘Q / ⌘W, Ctrl off a Mac), or none. The
/// ONE answer, so the chords live in a single place rather than re-spelled at
/// each use site — macOS binds both through an app menu this app does not have,
/// so it reads them itself.
///
/// Both key readings, like the palette's toggle: the physical code is what a
/// Dvorak or AZERTY layout still calls Q or W, and the logical character is
/// what a layout that remaps the code actually types.
pub fn command_chord(
    logical: iced::keyboard::Key,
    physical: iced::keyboard::key::Physical,
    modifiers: iced::keyboard::Modifiers,
) -> crate::CommandChord {
    use iced::keyboard::key::{Code, Physical};
    if !modifiers.command() {
        return crate::CommandChord::Ignored;
    }
    let quit = physical == Physical::Code(Code::KeyQ) || types_letter(&logical, "q");
    if quit {
        return crate::CommandChord::Quit;
    }
    let close_window = physical == Physical::Code(Code::KeyW) || types_letter(&logical, "w");
    if close_window {
        return crate::CommandChord::CloseWindow;
    }
    crate::CommandChord::Ignored
}

/// The letter a press actually types, after the layout has had its say — the
/// other half of the physical code in every chord above.
fn types_letter(logical: &iced::keyboard::Key, letter: &str) -> bool {
    matches!(logical, iced::keyboard::Key::Character(typed) if typed.eq_ignore_ascii_case(letter))
}

/// The transient layer currently over the console's content — the TOPMOST one
/// only — or `""` when the content itself is the frontmost thing on screen.
/// The order IS the z-order.
///
/// ONE enumeration, two readers. Escape asks it which surface to close;
/// the keyboard scroll asks it only whether anything at all sits over the pane
/// it would otherwise move. Every keyboard route that ignores this list routes
/// a key at the screen BEHIND the layer the reader is looking at.
///
/// EVERY PER-TAB RUNG IS SCOPED TO THE TAB THAT MOUNTS ITS SURFACE, because no
/// tab switch clears any of this state — `select_shell_tab` leaves every one
/// of these flags set. A flag left set on the tab you came from
/// names a layer that is no longer on screen: Escape then "closes" an
/// invisible menu while the visible screen swallows the press, and the scroll
/// reader refuses to move a pane nothing is actually covering. Which scope a
/// rung gets is read off the slot layout in `components/shell.ice`, not
/// guessed: `slot chat` / `slot forge` sit inside `match tab`, so their menus
/// are per-tab mounts, while `slot palette` and `slot bell` sit OUTSIDE it —
/// the palette, the bell and the create modal ride every tab and stay global
/// on purpose.
//
// One argument per layer, plus the tab that scopes them: the Ice extern
// surface is flat, and the reading must see every layer at once to name the
// topmost. Scoping lives HERE rather than in the call sites' argument lists —
// a conjunction per caller is one guard per rung to forget.
#[allow(clippy::too_many_arguments)]
pub fn topmost_overlay(
    shell_tab: crate::ShellTab,
    palette_open: bool,
    bell_open: bool,
    channel_create_open: bool,
    thread_message_action: crate::MessageAction,
    message_action: crate::MessageAction,
    channel_settings_open: bool,
    page_delete_armed: bool,
    fs_delete_target: &str,
    forge_repo_menu: bool,
) -> String {
    let on_chat = shell_tab == crate::ShellTab::Chat;
    let on_pages = shell_tab == crate::ShellTab::Pages;
    let on_files = shell_tab == crate::ShellTab::Files;
    let on_forge = shell_tab == crate::ShellTab::Forge;
    if palette_open {
        return "palette".into();
    }
    if bell_open {
        return "bell".into();
    }
    if channel_create_open {
        return "channel_create".into();
    }
    // THE DRAWER UNMOUNTS THE THREAD RAIL — `if active_thread_seq > 0 &&
    // !channel_settings_open` in `screens/chat.ice` — and nothing clears the ⋯
    // flag on the way in, so the same rule the tab scoping states one level up
    // applies here: a rung answers only while its surface is mounted. Without
    // the term, opening a thread action and then Channel details was a
    // mouse-reachable state where the first Escape wiped a half-typed
    // `thread_edit_draft` and left the drawer standing. It cannot be expressed
    // by moving one rung in the ladder's total order — the stream's own menu
    // really does float over the drawer and must stay above it.
    if on_chat && !channel_settings_open && thread_message_action != crate::MessageAction::Toolbar {
        return "thread_menu".into();
    }
    if on_chat && message_action != crate::MessageAction::Toolbar {
        return "message_menu".into();
    }
    // BELOW the stream's message menu, which floats over the drawer, and above
    // the repo menu, which lives on another tab. The drawer had no rung at all: it
    // shipped with an `×` and no keyboard exit while every other overlay in the
    // app answered Escape. Measured on the running app — Escape over an open
    // Channel details changed exactly zero pixels.
    if on_chat && channel_settings_open {
        return "channel_settings".into();
    }
    // The pages block-actions menu and insert row used to sit here, and the
    // comments rail is a persistent panel with its own close. THE ARMED DELETE
    // IS NEITHER: it paints a scrim and a confirm over the canvas, and it
    // shipped with the mouse as its only exit. `pages_ready` in
    // `handlers/overlays.ice` names it for the same reason — a layer that eats
    // the mouse must eat the keyboard, or Cmd/Ctrl+Z mutates (and autosaves)
    // the document the reader is being asked to confirm the deletion of.
    if on_pages && page_delete_armed {
        return "page_delete".into();
    }
    // The same confirm one screen over: `fs_delete_target` arms a scrim and a
    // `ConfirmDelete` over duckfs (`screens/storage.ice`), and it had no
    // keyboard exit either — the state the channel drawer was in before #1132
    // gave it a rung. A destructive confirm is the LAST layer that should need
    // the mouse.
    if on_files && !fs_delete_target.is_empty() {
        return "fs_delete".into();
    }
    if on_forge && forge_repo_menu {
        return "repo_menu".into();
    }
    String::new()
}

/// The surface Escape dismisses — the topmost transient layer, minus the one
/// rung Escape does not own. Menus, popovers, the create modal, the bell and
/// the channel drawer close; the thread and comments rails keep their explicit
/// × — closing one from a global key would also have to adjudicate its
/// half-typed drafts. The drawer carries no such debt: its only opener
/// re-seeds the name draft from the live channel name on every open, so its
/// rung leaks nothing the × doesn't.
#[allow(clippy::too_many_arguments)]
pub fn escape_target(
    logical: iced::keyboard::Key,
    shell_tab: crate::ShellTab,
    palette_open: bool,
    bell_open: bool,
    channel_create_open: bool,
    thread_message_action: crate::MessageAction,
    message_action: crate::MessageAction,
    channel_settings_open: bool,
    page_delete_armed: bool,
    fs_delete_target: String,
    forge_repo_menu: bool,
) -> String {
    use iced::keyboard::{Key, key::Named};
    let not_escape = logical != Key::Named(Named::Escape);
    if not_escape {
        return String::new();
    }
    let topmost = topmost_overlay(
        shell_tab,
        palette_open,
        bell_open,
        channel_create_open,
        thread_message_action,
        message_action,
        channel_settings_open,
        page_delete_armed,
        &fs_delete_target,
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

pub fn close_message_action(close: bool, current: crate::MessageAction) -> crate::MessageAction {
    if close {
        crate::MessageAction::Toolbar
    } else {
        current
    }
}

/// True when the live connection is in a state the shell should banner:
/// the stream is down, retrying, or a resync failed and is backing off.
pub fn connection_degraded(status: &str) -> bool {
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

/// Is this press ⌘C? Its own question, not an arm on [`command_chord`]: the
/// route that asks it exists ONLY while a copy range is open, so ⌘C costs
/// nothing on a screen with no selection, and the quit/close chord — which is
/// armed by ⌘ alone, on every screen — keeps a vocabulary of two.
///
/// It reaches that route only when no widget captured the press (the
/// subscription is `status=ignored`), so a caret in a composer or a field with
/// its own selection keeps its own copy, exactly as it should.
pub fn is_copy_chord(
    logical: iced::keyboard::Key,
    physical: iced::keyboard::key::Physical,
    modifiers: iced::keyboard::Modifiers,
) -> bool {
    use iced::keyboard::key::{Code, Physical};
    modifiers.command() && (physical == Physical::Code(Code::KeyC) || types_letter(&logical, "c"))
}
