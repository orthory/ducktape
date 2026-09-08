//! The facts the host pushes, the readings folded off them, and the writes
//! that leave as intents — one per act the forge screen offers.

use iced::futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use ui_lang_guest::host;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct HostError {
    pub message: String,
}

/// One repository card: its name and the head the last push landed.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ForgeRepo {
    pub name: String,
    pub head: String,
}

/// One tracker row. `kind` is `issue` | `pr`; `state` is `open` | `closed`
/// | `merged`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ForgeItem {
    pub number: i64,
    pub kind: String,
    pub state: String,
    pub title: String,
    pub author: String,
    pub author_name: String,
}

/// One inline run of a rich paragraph, pre-sorted into the style arm the
/// span template renders it with: exactly one field is non-empty.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChatSpan {
    pub mention: String,
    pub link_text: String,
    pub link: String,
    pub bold_italic: String,
    pub bold: String,
    pub italic: String,
    pub plain: String,
}

/// One rendered block of a body: `paragraph` | `code` | `quote` | `divider`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChatBlock {
    pub kind: String,
    pub text: String,
    pub lang: String,
    pub rich: bool,
    pub spans: Vec<ChatSpan>,
}

/// One discussion note, in the fields this screen draws: the app's row
/// carries more, and the wire drops what is not named here.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub seq: i64,
    pub author: String,
    pub meta: String,
    pub blocks: Vec<ChatBlock>,
    pub initial: String,
    pub avatar_kind: String,
    pub render_rev: i64,
}

/// One line comment a review carried, anchored `path:line (side)`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ForgeReviewComment {
    pub anchor: String,
    pub body: String,
    pub blocks: Vec<ChatBlock>,
}

/// One submitted review, rendered.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ForgeReview {
    pub author: String,
    pub author_name: String,
    pub verdict: String,
    pub body: String,
    pub blocks: Vec<ChatBlock>,
    pub commit: String,
    pub outdated: bool,
    pub created_at: i64,
    pub comments: Vec<ForgeReviewComment>,
}

/// One line comment staged for the next review, as the app holds it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ForgeDraftComment {
    pub anchor: String,
    pub path: String,
    pub line: String,
    pub side: String,
    pub body: String,
}

/// One entry of the open directory. `kind` is `dir` | `file`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TreeEntry {
    pub name: String,
    pub path: String,
    pub kind: String,
}

/// One painted row of the unified patch. `kind` is `file` | `hunk` | `add`
/// | `del` | `ctx`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DiffLine {
    pub key: i64,
    pub kind: String,
    pub old_no: String,
    pub new_no: String,
    pub sign: String,
    pub text: String,
    pub path: String,
    pub side: String,
}

/// The screen's facts, as the app holds them. The phases, the tab and the
/// verdict are the app's enum words. `drafts_cleared` moves once per
/// committed op and `drafts_scope` names which drafts it consumed (`item`:
/// every draft, `review`: the review body and the line comment, `comment`:
/// the line comment alone). `landed_tick` moves once per deep link that
/// landed on `landed_seq` in the discussion.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ForgeProps {
    pub dark: bool,
    pub connected: bool,
    pub org: String,
    pub about: String,
    pub tier: String,
    pub network_chain_id: String,
    pub connected_rpc: String,
    pub repos: Vec<ForgeRepo>,
    pub list_phase: String,
    pub open_repo: String,
    pub repo_menu: bool,
    pub repo_phase: String,
    pub branches: Vec<String>,
    pub tab: String,
    pub items: Vec<ForgeItem>,
    pub forge_item_number: i64,
    pub item_phase: String,
    pub forge_item_kind: String,
    pub forge_item_title: String,
    pub forge_item_state: String,
    pub forge_item_author: String,
    pub forge_item_branches: String,
    pub forge_item_body: String,
    pub forge_item_blocks: Vec<ChatBlock>,
    pub forge_item_files_changed: i64,
    pub forge_item_additions: i64,
    pub forge_item_deletions: i64,
    pub diff_rows: Vec<DiffLine>,
    pub forge_item_diff_truncated: bool,
    pub forge_item_merge_oid: String,
    pub forge_item_source_oid: String,
    pub forge_item_approvals: i64,
    pub forge_item_change_requests: i64,
    pub forge_item_reviews: Vec<ForgeReview>,
    pub merge_conflicts: Vec<String>,
    pub merge_busy: bool,
    pub review_verdict: String,
    pub review_busy: bool,
    pub staged_comments: Vec<ForgeDraftComment>,
    pub comment_cap_reached: bool,
    pub discussion: Vec<ChatMessage>,
    pub linked_note: Vec<ChatMessage>,
    pub landed_seq: i64,
    pub landed_tick: i64,
    pub tree_path: String,
    pub tree_rev: String,
    pub tree_entries: Vec<TreeEntry>,
    pub tree_born: bool,
    pub tree_truncated: bool,
    pub tree_phase: String,
    pub file_path: String,
    pub file_text: String,
    pub file_binary: bool,
    pub file_truncated: bool,
    pub file_picture: bool,
    pub file_width: i64,
    pub file_height: i64,
    pub file_note: String,
    pub file_header: String,
    pub file_phase: String,
    pub drafts_cleared: i64,
    pub drafts_scope: String,
}

/// The facts now, and again on every change the host sees.
pub fn props() -> impl Stream<Item = Result<ForgeProps, HostError>> + Send + 'static {
    host::subscribe("forge.props", &[]).map(|answer| {
        let bytes = answer.map_err(|message| HostError { message })?;
        serde_json::from_slice(&bytes).map_err(|error| HostError {
            message: error.to_string(),
        })
    })
}

/// `forge.open_repo` — open one repository.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Name {
    pub name: String,
}

/// `forge.tab` — the repo seat to show (`code`, `pulls`, `issues`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tab {
    pub tab: String,
}

/// `forge.open_item` — open one tracker item.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Number {
    pub number: i64,
}

/// `forge.review_pick` — the verdict the next review goes out under.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verdict {
    pub verdict: String,
}

/// `forge.review_submit` — the review body; the staged comments ride with it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Body {
    pub body: String,
}

/// `forge.comment_stage` — one line comment for the next review.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentStage {
    pub path: String,
    pub line: String,
    pub side: String,
    pub body: String,
}

/// `forge.comment_drop` — the staged comment at one anchor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Anchor {
    pub anchor: String,
}

/// `forge.tree` and `forge.blob` — a directory to list or a file to read,
/// from the repo root.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Path {
    pub path: String,
}

/// `forge.open_link` — a link a body or the reader's document carried.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    pub url: String,
}

/// `forge.copy` — the host puts `text` on the clipboard and toasts `label`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Copy {
    pub text: String,
    pub label: String,
}

pub fn open_repo(name: &str) -> bool {
    notify("forge.open_repo", &Name { name: name.into() })
}

pub fn close_repo() -> bool {
    notify("forge.close_repo", &())
}

pub fn toggle_repo_menu() -> bool {
    notify("forge.toggle_repo_menu", &())
}

pub fn pick_tab(tab: &str) -> bool {
    notify("forge.tab", &Tab { tab: tab.into() })
}

pub fn open_item(number: i64) -> bool {
    notify("forge.open_item", &Number { number })
}

pub fn close_item() -> bool {
    notify("forge.close_item", &())
}

pub fn merge() -> bool {
    notify("forge.merge", &())
}

pub fn review_pick(verdict: &str) -> bool {
    notify(
        "forge.review_pick",
        &Verdict {
            verdict: verdict.into(),
        },
    )
}

pub fn review_submit(body: &str) -> bool {
    notify("forge.review_submit", &Body { body: body.into() })
}

pub fn comment_stage(path: &str, line: &str, side: &str, body: &str) -> bool {
    notify(
        "forge.comment_stage",
        &CommentStage {
            path: path.into(),
            line: line.into(),
            side: side.into(),
            body: body.into(),
        },
    )
}

pub fn comment_drop(anchor: &str) -> bool {
    notify(
        "forge.comment_drop",
        &Anchor {
            anchor: anchor.into(),
        },
    )
}

pub fn open_dir(path: &str) -> bool {
    notify("forge.tree", &Path { path: path.into() })
}

pub fn open_file(path: &str) -> bool {
    notify("forge.blob", &Path { path: path.into() })
}

pub fn open_link(url: &str) -> bool {
    notify("forge.open_link", &Link { url: url.into() })
}

pub fn copy(text: &str, label: &str) -> bool {
    notify(
        "forge.copy",
        &Copy {
            text: text.into(),
            label: label.into(),
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

pub fn plural(count: i64, one: &str, many: &str) -> String {
    let noun = if count == 1 { one } else { many };
    format!("{count} {noun}")
}

/// The tracker's Pull requests / Issues split; the Code seat lists nothing.
pub fn filter_forge_items(items: &[ForgeItem], tab: &str) -> Vec<ForgeItem> {
    let kind = match tab {
        "pulls" => "pr",
        "issues" => "issue",
        _ => return Vec::new(),
    };
    items
        .iter()
        .filter(|item| item.kind == kind)
        .cloned()
        .collect()
}

/// The tab count chips — open work only: a PR counts until it merges, an
/// issue until it closes.
pub fn forge_open_count(items: &[ForgeItem], kind: &str) -> i64 {
    let open = items
        .iter()
        .filter(|item| item.kind == kind)
        .filter(|item| match kind {
            "pr" => item.state != "merged",
            _ => item.state == "open",
        })
        .count();
    i64::try_from(open).unwrap_or(i64::MAX)
}

/// The merged-state banner: the short merge oid plus the branch line.
pub fn forge_merge_note(merge_oid: &str, branches: &str) -> String {
    let short: String = merge_oid.chars().take(8).collect();
    match branches.is_empty() {
        true => format!("Merged as {short}"),
        false => format!("Merged as {short} · {branches}"),
    }
}

/// A review verdict key as its timeline verb.
pub fn verdict_label(verdict: &str) -> String {
    match verdict {
        "approve" => "approved".into(),
        "request_changes" => "requested changes".into(),
        _ => "commented".into(),
    }
}

/// A verdict picker label, dotted when it is the current pick.
pub fn verdict_pick_label(current: &str, key: &str, label: &str) -> String {
    match current == key {
        true => format!("● {label}"),
        false => label.to_owned(),
    }
}

pub fn markdown_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".md") || lower.ends_with(".markdown")
}

/// `1024 × 768` — the caption under a drawn picture.
pub fn picture_caption(width: i64, height: i64) -> String {
    format!("{width} × {height}")
}

/// The binary plate's line: the loader's reason when it gave one (a picture
/// past the cap or one that did not decode), else the generic one.
pub fn binary_note(text: &str) -> String {
    match text.is_empty() {
        true => "This is not text — the reader shows no preview for it.".to_owned(),
        false => text.to_owned(),
    }
}

/// The `?net=` a produced `duck://` link carries — the minted digest after
/// the chain id's last `#` — or "" when the producer has no chain id. The
/// same spelling as the app's `duck_net_query` (chat::client).
fn net_query(chain_id: &str) -> String {
    let digest = chain_id.rsplit_once('#').map(|(_, hex)| hex).unwrap_or("");
    match digest.is_empty() {
        true => String::new(),
        false => format!("?net={digest}"),
    }
}

/// `duck://forge/<repo>/<number>?net=…` — one issue or PR.
pub fn duck_forge_item_link(repo: &str, number: i64, chain_id: &str) -> String {
    format!("duck://forge/{repo}/{number}{}", net_query(chain_id))
}

/// `duck://forge/<repo>?net=…` — the repo itself.
pub fn duck_forge_repo_link(repo: &str, chain_id: &str) -> String {
    format!("duck://forge/{repo}{}", net_query(chain_id))
}

/// The command that makes a repo: forge IS a git remote, and a repo comes
/// into existence when a push lands on it.
pub fn forge_push_command(rpc: &str) -> String {
    let endpoint = rpc.trim_end_matches('/');
    format!("git remote add ducktape {endpoint}/forge/my-repo && git push ducktape main")
}

/// The label a picked-but-unstaged line wears above the composer, empty when
/// no line is picked — the composer keys its whole visibility on this. The
/// same `path:line (side)` the app stamps on a staged row.
pub fn forge_comment_target(path: &str, line: &str, side: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    format!("{path}:{line} ({side})")
}

/// Whether a consumption the app reported under `scope` took `draft`:
/// opening an item takes every draft, a landed review takes its body and
/// the line comment, a moved branch takes the line comment alone.
pub fn drafts_cleared_by(scope: &str, draft: &str) -> bool {
    match scope {
        "item" => true,
        "review" => draft == "review" || draft == "comment",
        "comment" => draft == "comment",
        _ => false,
    }
}

/// The draft, or nothing once the app consumed it.
pub fn keep_draft(consumed: bool, draft: &str) -> String {
    if consumed {
        String::new()
    } else {
        draft.to_owned()
    }
}

/// The seq a fresh landing names, or 0 when the props carried no new one.
pub fn landed_seq_of(fresh: bool, seq: i64) -> i64 {
    if fresh { seq } else { 0 }
}

/// The appearance as the word the handler matches on.
pub(crate) fn appearance_of(dark: bool) -> crate::Appearance {
    if dark {
        crate::Appearance::Dark
    } else {
        crate::Appearance::Light
    }
}
