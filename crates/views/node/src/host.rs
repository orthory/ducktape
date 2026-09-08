//! The node facts the host pushes, the readings folded off them, and the
//! four writes that leave as intents.

use iced::futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use ui_lang_guest::host;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct HostError {
    pub message: String,
}

/// One peer of this node, as `/v1/peers` lists it.
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct PeerRow {
    pub key: String,
    pub role: String,
    pub live: bool,
}

/// One registered module — the same fields the desktop app's registry row
/// carries.
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct ModuleRow {
    pub id: String,
    pub category: String,
    pub root: String,
    pub code_hash: String,
    pub pending_hash: String,
    pub activation_height: i64,
    pub readiness: i64,
    pub ready: bool,
}

/// The screen's facts, as the app holds them: one document, so the head and
/// the checkpoint never describe two different instants.
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct NodeProps {
    pub node_key: String,
    pub node_data_dir: String,
    /// This node's standing — `validator` | `resident` | `guest`, or `""`
    /// while the roster has not answered.
    pub tier: String,
    pub admin: bool,
    pub status: String,
    pub loading: bool,
    pub module_rows: Vec<ModuleRow>,
    pub node_height: i64,
    pub node_checkpoint: i64,
    pub node_last_finalized: i64,
    pub node_reachable_label: String,
    pub node_quorum_label: String,
    pub node_version: String,
    pub node_root_hash: String,
    pub sync_line: String,
    pub node_phase_since: i64,
    pub node_sync_retries: i64,
    pub node_sync_failures: i64,
    pub node_sync_last_error: String,
    pub node_peers: Vec<PeerRow>,
    pub wall_now: i64,
    pub connected: bool,
    pub dark: bool,
}

/// The facts now, and again on every change the host sees.
pub fn props() -> impl Stream<Item = Result<NodeProps, HostError>> + Send + 'static {
    host::subscribe("node.props", &[]).map(|answer| {
        let bytes = answer.map_err(|message| HostError { message })?;
        serde_json::from_slice(&bytes).map_err(|error| HostError {
            message: error.to_string(),
        })
    })
}

/// `node.copy` — the host puts `text` on the clipboard and toasts `label`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Copy {
    pub text: String,
    pub label: String,
}

/// `node.tab` — the tab the reader is on, so the host gates the log stream
/// on it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tab {
    pub tab: String,
}

/// `node.log_filter` — the substring the host filters the log ring by.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogFilter {
    pub filter: String,
}

pub fn copy(text: &str, label: &str) -> bool {
    notify(
        "node.copy",
        &Copy {
            text: text.into(),
            label: label.into(),
        },
    )
}

pub(crate) fn show_tab(tab: crate::NodeTab) -> bool {
    let tab = match tab {
        crate::NodeTab::Overview => "overview",
        crate::NodeTab::Permissions => "permissions",
        crate::NodeTab::Activity => "activity",
        crate::NodeTab::Modules => "modules",
    };
    notify("node.tab", &Tab { tab: tab.into() })
}

pub fn log_filter(filter: &str) -> bool {
    notify(
        "node.log_filter",
        &LogFilter {
            filter: filter.into(),
        },
    )
}

fn notify<T: Serialize>(operation: &str, payload: &T) -> bool {
    let bytes = serde_json::to_vec(payload).expect("an intent encodes");
    host::notify(operation, &bytes);
    true
}

// The desktop app's own readings (app/src/backend), repeated here because the
// view is its own crate: the wire carries the words, not the functions.

pub fn icon(name: &str) -> Vec<u8> {
    design::icons::svg(name).as_bytes().to_vec()
}

pub fn connection_degraded(status: &str) -> bool {
    status == "Offline"
        || status == "Sync delayed"
        || status == "Reconnecting…"
        || status == "Live · resyncing"
}

pub fn reading_pair(left: &str, right: &str) -> String {
    format!("{left} / {right}")
}

pub fn count_label(count: i64) -> String {
    match count > 0 {
        true => count.to_string(),
        false => String::new(),
    }
}

pub fn keep_str(loaded: bool, next: &str, current: &str) -> String {
    if loaded { next } else { current }.to_owned()
}

pub fn initial_of(name: &str) -> String {
    name.trim()
        .chars()
        .next()
        .map(|first| first.to_uppercase().to_string())
        .unwrap_or_default()
}

/// `h 84,912`; a height the node has not reported reads `h —`.
pub fn height_label_short(height: i64) -> String {
    if height < 0 {
        return "h —".into();
    }
    let digits = height.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    format!("h {grouped}")
}

/// `just now` / `5m ago`; a negative stamp is a reading the node never
/// published (`—`), zero a record with no stamp (nothing).
pub fn relative_time(unix_seconds: i64, wall_now: i64) -> String {
    if unix_seconds < 0 {
        return "—".into();
    }
    if unix_seconds == 0 {
        return String::new();
    }
    let elapsed = wall_now.saturating_sub(unix_seconds);
    if elapsed < 60 {
        return "just now".into();
    }
    const MINUTE: i64 = 60;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;
    let (value, unit) = match elapsed {
        span if span < HOUR => (span / MINUTE, "m"),
        span if span < DAY => (span / HOUR, "h"),
        span => (span / DAY, "d"),
    };
    format!("{value}{unit} ago")
}
