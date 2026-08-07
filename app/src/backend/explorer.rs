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
/// ladder's order IS the z-order. Menus, popovers, the create modal, the bell
/// and the channel drawer close; the thread and comments rails keep their
/// explicit × — closing one from a global key would also have to adjudicate
/// its half-typed drafts. The drawer carries no such debt: its only opener
/// re-seeds `channel_name_draft` from the live channel name on every open, so
/// an Escape close leaks nothing its × doesn't. The palette is absent on
/// purpose: `palette_key_action` owns its keys, and an open palette swallows
/// Escape.
///
/// RUNG CORRECTNESS DEPENDS ON `shell_tab`. Every rung below is scoped to the
/// tab whose screen actually mounts its surface, because no tab switch clears
/// any of this state — `select_shell_tab` writes `shell_tab` and nothing else.
/// A flag left set on the tab you came from names a layer that is no longer on
/// screen, and Escape then "closes" an invisible surface while the visible one
/// stays up. Which scope a rung gets is read off `view.ice`'s slot layout, not
/// guessed: `slot chat` / `slot pages` / `slot forge` sit inside
/// `match tab`, so ChatScreen, PagesScreen and ForgeScreen are per-tab mounts,
/// while `slot palette` and `slot bell` sit OUTSIDE it — the bell and the
/// create modal ride every tab and their rungs stay global on purpose.
//
// One argument per closable surface plus the tab that scopes them: the Ice
// extern surface is flat, and the ladder must see every layer at once to name
// the topmost. Scoping lives HERE rather than in the call site's argument list
// — a conjunction per caller is one guard per rung to forget, and three of
// these rungs were already wrong that way.
#[allow(clippy::too_many_arguments)]
pub fn escape_target(
    logical: iced::keyboard::Key,
    shell_tab: String,
    palette_open: bool,
    bell_open: bool,
    channel_create_open: bool,
    thread_message_action: String,
    message_action: String,
    channel_settings_open: bool,
    page_delete_armed: bool,
    forge_repo_menu: bool,
) -> String {
    use iced::keyboard::{Key, key::Named};
    let not_escape = logical != Key::Named(Named::Escape);
    if not_escape || palette_open {
        return String::new();
    }
    let on_chat = shell_tab == "chat";
    let on_pages = shell_tab == "pages";
    let on_forge = shell_tab == "forge";
    // Window-level layers: `view.ice` mounts both outside the tab match, so
    // they stay on screen across a switch and answer Escape from any tab.
    if bell_open {
        return "bell".into();
    }
    if channel_create_open {
        return "channel_create".into();
    }
    if on_chat && thread_message_action != "toolbar" {
        return "thread_menu".into();
    }
    if on_chat && message_action != "toolbar" {
        return "message_menu".into();
    }
    // Under both message menus: the drawer is a rail beside the stream, and a ⋯
    // menu opened while it is out floats over that stream, not under the rail.
    // Two facts put the thread rungs above out of the way while the drawer is
    // out, and only the second makes this rung reachable: the THREAD RAIL's
    // view condition (`active_thread_seq > 0 && !channel_settings_open`) hides
    // that rail while the drawer is out — the drawer's own condition is
    // `channel_settings_open && !empty(active_channel)` — AND
    // `toggle_channel_settings` clears the rail's menu state on the way in.
    // Hiding alone was never enough — visibility is not ladder state, and an
    // unmounted menu whose `thread_message_action` still reads non-"toolbar"
    // answers Escape from a rung above this one.
    if on_chat && channel_settings_open {
        return "channel_settings".into();
    }
    // The pages block-actions menu and insert row used to sit here. The delete
    // confirmation is the one transient layer the document still has: an
    // `overlay` with `backdrop=scrim` over the canvas. The `overlay` widget
    // does not answer Escape by itself — the structurally identical create
    // modal needs its own rung too — so without this one the scrim could only
    // be dismissed by its Cancel button or a backdrop click. The comments rail
    // stays out of the ladder: it is a persistent panel with its own close.
    if on_pages && page_delete_armed {
        return "page_delete".into();
    }
    if on_forge && forge_repo_menu {
        return "repo_menu".into();
    }
    String::new()
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
