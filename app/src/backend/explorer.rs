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
    block_actions_open: bool,
    block_insert_open: bool,
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
    if block_actions_open {
        return "block_actions".into();
    }
    if block_insert_open {
        return "block_insert".into();
    }
    if forge_repo_menu {
        return "repo_menu".into();
    }
    String::new()
}

/// The slash menu: a draft starting with `/` filters the insertable block
/// kinds by case-insensitive prefix (`/h` -> the headings). Empty when the
/// draft is not a slash command.
pub fn slash_kind_matches(draft: String, kinds: Vec<String>) -> Vec<String> {
    let Some(needle) = draft.strip_prefix('/') else {
        return Vec::new();
    };
    let needle = needle.trim().to_ascii_lowercase();
    kinds
        .into_iter()
        .filter(|kind| kind.to_ascii_lowercase().starts_with(&needle))
        .collect()
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
