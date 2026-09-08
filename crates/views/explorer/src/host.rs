//! The ledger and the search answer the host pushes, the readings folded off
//! them, and the four writes that leave as intents.

use iced::futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use ui_lang_guest::host;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct HostError {
    pub message: String,
}

/// One block of the ledger, as the desktop app's inspector row carries it.
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct ExplorerBlock {
    pub height: i64,
    pub hash: String,
    pub commit: String,
    pub op_count: i64,
}

/// One operation a block carried.
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct ExplorerOp {
    pub height: i64,
    pub proposer: String,
    pub target: String,
    pub disposition: String,
    pub op_hash: String,
    pub payload: String,
    pub trace: String,
}

/// One workspace search hit.
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct ExplorerHit {
    pub kind: String,
    pub code: String,
    pub title: String,
    pub snippet: String,
    pub meta: String,
    pub target: String,
}

/// How many hits one kind answered with.
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct KindCount {
    pub kind: String,
    pub label: String,
    pub count: i64,
}

/// The ledger as the app holds it, and the answer to the last search the
/// app ran for this view (`sent_query` is the query it stands for, `""`
/// while none does).
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct ExplorerProps {
    pub connected: bool,
    pub loading: bool,
    pub dark: bool,
    pub blocks: Vec<ExplorerBlock>,
    pub ops: Vec<ExplorerOp>,
    pub head: i64,
    pub sync_line: String,
    pub hits: Vec<ExplorerHit>,
    pub kinds: Vec<KindCount>,
    pub partial: String,
    pub searching: bool,
    pub sent_query: String,
}

/// The ledger now, and again on every change the host sees.
pub fn props() -> impl Stream<Item = Result<ExplorerProps, HostError>> + Send + 'static {
    host::subscribe("explorer.props", &[]).map(|answer| {
        let bytes = answer.map_err(|message| HostError { message })?;
        serde_json::from_slice(&bytes).map_err(|error| HostError {
            message: error.to_string(),
        })
    })
}

/// `explorer.copy` — the host puts `text` on the clipboard and toasts `label`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Copy {
    pub text: String,
    pub label: String,
}

/// `explorer.search` — the trimmed query the host runs a workspace search
/// for; the answer comes back as props.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Search {
    pub query: String,
}

/// `explorer.refresh` — reload the ledger.
pub fn refresh_ledger() -> bool {
    notify("explorer.refresh", &())
}

pub fn copy(text: &str, label: &str) -> bool {
    notify(
        "explorer.copy",
        &Copy {
            text: text.into(),
            label: label.into(),
        },
    )
}

pub fn search(query: &str) -> bool {
    notify(
        "explorer.search",
        &Search {
            query: query.into(),
        },
    )
}

/// `explorer.clear` — drop the standing answer and any search in flight.
pub fn clear_search() -> bool {
    notify("explorer.clear", &())
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

pub fn explorer_ops_at(ops: &[ExplorerOp], height: i64) -> Vec<ExplorerOp> {
    ops.iter()
        .filter(|op| op.height == height)
        .cloned()
        .collect()
}

/// `h 84,912`; a height the node has not reported reads `h —`.
/// The list's landmark for a digest: its first twelve hex chars and an
/// ellipsis. The whole value stays in the props for the detail and the copy.
pub fn short(digest: &str) -> String {
    let mut short: String = digest.chars().take(12).collect();
    if digest.chars().count() > 12 {
        short.push('\u{2026}');
    }
    short
}

pub fn height_label(height: i64) -> String {
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

pub fn plural(count: i64, one: &str, many: &str) -> String {
    let noun = if count == 1 { one } else { many };
    format!("{count} {noun}")
}

/// Does the zero-hit plate speak for what is in the box now? Only while the
/// answered query and the trimmed draft still match, and nothing is in flight.
pub fn search_answer_stands(query: &str, draft: &str, searching: bool) -> bool {
    !searching && !query.is_empty() && draft.trim() == query
}
