//! The facts the host pushes, the readings folded off them, and the writes
//! that leave as intents — one per act the pages screen offers.

use iced::futures::StreamExt;
use serde::{Deserialize, Serialize};
use ui_lang_guest::host;

/// One page of the workspace, as the sidebar lists it: `prefix` is two
/// spaces per depth, the only hierarchy signal the row has.
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct PageItem {
    pub id: String,
    pub title: String,
    pub parent: String,
    pub prefix: String,
    pub child_count: i64,
}

/// A subpage block of the open page: navigation, listed under the body.
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct Subpage {
    pub id: String,
    pub title: String,
}

/// One page-search hit: the page it was found in, the block and its text.
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct PageSearchHit {
    pub page_id: String,
    pub page_title: String,
    pub block_id: String,
    pub kind: String,
    pub text: String,
}

/// One comment of a thread.
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct PageComment {
    pub id: String,
    pub ordinal: i64,
    pub author: String,
    pub meta: String,
    pub text: String,
}

/// One comment thread on the open page, WITH its whole conversation: the node
/// answers threads and comments in one query, so the card draws every thread
/// expanded and never asks per thread.
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct PageCommentThread {
    pub id: String,
    pub target: String,
    pub author: String,
    pub meta: String,
    pub resolved: bool,
    pub comment_count: i64,
    pub comments: Vec<PageComment>,
}

/// A thread with the label of the block it anchors on.
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct PageCommentThreadRow {
    pub thread: PageCommentThread,
    pub anchor: String,
}

/// The open threads sharing one anchor, under the quote that names it. In page
/// scope the card lists a group per commented block; in block scope there is
/// only ever the one.
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct PageCommentGroup {
    pub target: String,
    pub anchor: String,
    pub threads: Vec<PageCommentThread>,
}

/// The screen's facts, as the app holds them. Document bytes arrive through a
/// separate bounded source subscription; props carry only its identity. `seed_rev`
/// moves when the app wants the view's drafts to BECOME `page_seed` and
/// `comment_seed` — a recovered draft taken up, a failed post handed back.
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct PagesProps {
    pub comment_marks: Vec<crate::document_source::CommentMark>,
    pub document_source: Vec<u8>,
    pub document_error: String,
    pub commented_lines: Vec<i64>,
    pub dark: bool,
    pub connected: bool,
    pub loading: bool,
    pub busy: bool,
    pub page_link: String,
    pub pages: Vec<PageItem>,
    pub page_create_open: bool,
    pub active_page: String,
    pub active_page_title: String,
    pub active_page_parent: String,
    pub page_searching: bool,
    pub page_search_hits: Vec<PageSearchHit>,
    pub page_search_query: String,
    pub page_delete_armed: bool,
    pub autosave: String,
    pub page_refusal: String,
    pub subpages: Vec<Subpage>,
    pub orphaned_comment_drafts: Vec<String>,
    pub block_comments_open: bool,
    /// The block the card is scoped to, or empty for the whole page.
    pub scope_target: String,
    /// A badge-opened card is its block's, whole: no way back out to the page.
    pub scope_pinned: bool,
    pub scope_label: String,
    pub thread_total: i64,
    pub comment_rows: Vec<PageCommentThreadRow>,
    pub threads_loading: bool,
    pub compose_hint: String,
    pub seed_rev: i64,
    pub page_seed: String,
    pub comment_seed: String,
}

/// One item of the facts subscription: the facts, or why not.
#[derive(Clone, Debug, Default, Hash, PartialEq)]
pub struct PropsItem {
    pub next: PagesProps,
    pub error: String,
}

/// The facts now, and again on every change the host sees — a
/// subscription, so a view restored from a snapshot asks again on its own.
pub fn props() -> iced::Subscription<PropsItem> {
    iced::Subscription::run(|| {
        host::subscribe("pages.props", &[]).map(|answer| {
            let read = answer.and_then(|bytes| {
                serde_json::from_slice(&bytes).map_err(|error| error.to_string())
            });
            match read {
                Ok(next) => PropsItem {
                    next,
                    error: String::new(),
                },
                Err(error) => PropsItem {
                    next: PagesProps::default(),
                    error,
                },
            }
        })
    })
}

/// `pages.create` — a new page titled `title`; the comment draft the rail
/// held when the selection moves off this page.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Create {
    pub title: String,
    pub comment_draft: String,
}

/// `pages.choose` — open a page; `comment_draft` is the unsent comment the
/// rail held, for the app to keep as a recovered draft.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Choose {
    pub id: String,
    pub comment_draft: String,
}

/// `pages.search` — run a page search for `query`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Search {
    pub query: String,
}

/// `pages.open_hit` — open the page a search hit was found in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenHit {
    pub page_id: String,
    pub block_id: String,
    pub comment_draft: String,
}

/// `pages.use_draft` / `pages.discard_draft` — a recovered comment draft.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Draft {
    pub draft: String,
    pub comment_draft: String,
}

/// `pages.toggle_comments` / `pages.close_comments` / `pages.delete` — the
/// comment draft the rail held, for the app to keep as a recovered draft.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rail {
    pub comment_draft: String,
}

/// `pages.narrow` — scope the card to one block's threads.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Narrow {
    pub target: String,
}

/// `pages.resolve` — resolve (or reopen) one named thread.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolve {
    pub id: String,
    pub resolved: bool,
}

/// `pages.post` — a REPLY when `thread_id` names a thread, which anchors it on
/// that thread's own target; an empty `thread_id` opens a new thread on the
/// scope the card is showing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Post {
    pub text: String,
    pub thread_id: String,
}

/// `pages.copy` — put `text` on the clipboard and toast `label`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Copy {
    pub text: String,
    pub label: String,
}

fn notify<T: Serialize>(kind: &str, payload: &T) -> bool {
    host::notify(kind, &serde_json::to_vec(payload).expect("intent encode"));
    true
}

pub fn toggle_create() -> bool {
    host::notify("pages.toggle_create", &[]);
    true
}

pub fn create(title: &str, comment_draft: &str) -> bool {
    notify(
        "pages.create",
        &Create {
            title: title.into(),
            comment_draft: comment_draft.into(),
        },
    )
}

pub fn choose(id: &str, comment_draft: &str) -> bool {
    notify(
        "pages.choose",
        &Choose {
            id: id.into(),
            comment_draft: comment_draft.into(),
        },
    )
}

pub fn search(query: &str) -> bool {
    notify(
        "pages.search",
        &Search {
            query: query.into(),
        },
    )
}

pub fn clear_search() -> bool {
    host::notify("pages.clear_search", &[]);
    true
}

pub fn arm_delete() -> bool {
    host::notify("pages.arm_delete", &[]);
    true
}

pub fn disarm_delete() -> bool {
    host::notify("pages.disarm_delete", &[]);
    true
}

pub fn delete(comment_draft: &str) -> bool {
    notify(
        "pages.delete",
        &Rail {
            comment_draft: comment_draft.into(),
        },
    )
}

pub fn open_hit(page_id: &str, block_id: &str, comment_draft: &str) -> bool {
    notify(
        "pages.open_hit",
        &OpenHit {
            page_id: page_id.into(),
            block_id: block_id.into(),
            comment_draft: comment_draft.into(),
        },
    )
}

pub fn use_draft(draft: &str, comment_draft: &str) -> bool {
    notify(
        "pages.use_draft",
        &Draft {
            draft: draft.into(),
            comment_draft: comment_draft.into(),
        },
    )
}

pub fn discard_draft(draft: &str) -> bool {
    notify(
        "pages.discard_draft",
        &Draft {
            draft: draft.into(),
            comment_draft: String::new(),
        },
    )
}

pub fn toggle_comments(comment_draft: &str) -> bool {
    notify(
        "pages.toggle_comments",
        &Rail {
            comment_draft: comment_draft.into(),
        },
    )
}

pub fn close_comments(comment_draft: &str) -> bool {
    notify(
        "pages.close_comments",
        &Rail {
            comment_draft: comment_draft.into(),
        },
    )
}

pub fn narrow(target: &str) -> bool {
    notify(
        "pages.narrow",
        &Narrow {
            target: target.into(),
        },
    )
}

pub fn widen() -> bool {
    host::notify("pages.widen", &[]);
    true
}

pub fn resolve(id: &str, resolved: bool) -> bool {
    notify(
        "pages.resolve",
        &Resolve {
            id: id.into(),
            resolved,
        },
    )
}

pub fn post(text: &str, thread_id: &str) -> bool {
    notify(
        "pages.post",
        &Post {
            text: text.into(),
            thread_id: thread_id.into(),
        },
    )
}

pub fn copy(text: &str, label: &str) -> bool {
    notify(
        "pages.copy",
        &Copy {
            text: text.into(),
            label: label.into(),
        },
    )
}

// ---------- readings ----------

pub fn icon(name: &str) -> Vec<u8> {
    design::icons::svg(name).as_bytes().to_vec()
}

/// A count as the header chip prints it: the number, or nothing for zero.
pub fn count_label(count: i64) -> String {
    match count > 0 {
        true => count.to_string(),
        false => String::new(),
    }
}

pub fn keep_str(keep: bool, next: &str, current: &str) -> String {
    if keep { next } else { current }.to_owned()
}

/// The page list's narrowest and widest, in logical pixels: under the first a
/// page title is a column of syllables, over the second the list is reading
/// the document's own room.
const SIDEBAR_MINIMUM: f64 = 180.0;
const SIDEBAR_MAXIMUM: f64 = 420.0;

/// Where a drag on the list's edge leaves it. The document keeps at least half
/// the window whatever the reader drags, so a narrow console cannot be dragged
/// down to a sliver of page.
pub fn sidebar_width_after_delta(width: f64, delta: f64, viewport: f64) -> f64 {
    let maximum = (viewport * 0.5).clamp(SIDEBAR_MINIMUM, SIDEBAR_MAXIMUM);
    (width + delta).clamp(SIDEBAR_MINIMUM, maximum)
}

/// A search answer is standing when the query it was sent for is still what
/// the box holds and the round trip is over.
pub fn search_answer_stands(query: &str, draft: &str, searching: bool) -> bool {
    !searching && !query.is_empty() && draft.trim() == query
}

/// A principal's plate letters: two initials, or the first letter.
pub fn initials_of(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().take(2).collect();
    if words.len() == 2 {
        let letters: String = words
            .iter()
            .filter_map(|word| word.chars().find(char::is_ascii_alphanumeric))
            .collect();
        if letters.chars().count() == 2 {
            return letters.to_uppercase();
        }
    }
    let letters: String = name
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(2)
        .collect();
    match letters.is_empty() {
        true => "?".into(),
        false => letters.to_uppercase(),
    }
}

/// The draft after a seed the app pushed: the seed when the seed moved,
/// the reader's own text otherwise.
pub fn seeded(moved: bool, seed: &str, draft: &str) -> String {
    if moved { seed } else { draft }.to_owned()
}

/// Only the accepted canonical reference crosses back. The app resolves its
/// bytes from the matching host editor and keeps the ordinary save/CAS path.
pub fn edited(
    source: Vec<u8>,
    reference: Vec<u8>,
    navigation: Vec<u8>,
    comment_draft: &str,
) -> bool {
    host::notify(
        "pages.edited",
        &serde_json::to_vec(&crate::document_source::Accepted {
            comment_draft: comment_draft.into(),
            source,
            reference,
            navigation,
        })
        .expect("accepted document metadata"),
    );
    true
}

/// This notification binds the app source to the installed editor revision.
pub fn installed(document: &ui_lang_guest::Editor, source: Vec<u8>) -> bool {
    let state = document.state_view();
    host::notify(
        "pages.installed",
        &serde_json::to_vec(&crate::document_source::Installed {
            source,
            reset: state.reset,
            revision: state.revision,
            text_revision: state.text_revision,
            byte_len: state.text.len() as u32,
            cursor: state.cursor,
        })
        .expect("installed document metadata"),
    );
    true
}

/// Screen placement is local presentation state; the host still validates the line.
pub fn comment_navigation(navigation: Vec<u8>) -> bool {
    ui_lang_guest::wire::decode::<crate::document_source::Navigation>(&navigation)
        .is_ok_and(|navigation| navigation.comment_line.is_some())
}

/// A card the reader opened AT A LINE stays near that line, so it is bounded;
/// the header chip's card is the page's whole conversation and takes the room.
pub fn comment_card_height(anchor_y: f64, viewport_height: f64) -> f64 {
    let available = (viewport_height - 83.0).max(0.0);
    match anchor_y >= 0.0 {
        true => available.min(400.0),
        false => available,
    }
}

/// The pane geometry the card is placed against, in logical pixels. `max-w`
/// bounds a box INCLUDING its padding, so `DOCUMENT_SURFACE` is the `max-w` the
/// document arm carries and `DOCUMENT_PADDING` (pl 22 + pr 40) comes off it
/// before the editor gets its column.
const DOCUMENT_SURFACE: f64 = 766.0;
const DOCUMENT_MINIMUM: f64 = 560.0;
const DOCUMENT_PADDING: f64 = 62.0;
const DOCUMENT_LEFT_PADDING: f64 = 22.0;
const DOCUMENT_TOP_PADDING: f64 = 26.0;
const COMMENTS_CARD: f64 = 340.0;
const COMMENTS_GAP: f64 = 16.0;

/// The card floats in the margin while the pane holds the document at full
/// width plus the card and its two gutters; it squeezes the document left
/// while the document can give up that width and stay readable; below that
/// there is no margin to float in and the card drops into the text.
const BESIDE_FLOOR: f64 = DOCUMENT_SURFACE + COMMENTS_CARD + 2.0 * COMMENTS_GAP;
const SQUEEZE_FLOOR: f64 = DOCUMENT_MINIMUM + COMMENTS_CARD + 2.0 * COMMENTS_GAP;

/// How the card sits against a document pane this wide. View-local derived
/// state: nothing the host holds changes with it, so a window resize re-decides
/// it on the next sensor tick with the rail, its scope and its drafts intact.
pub(crate) fn comments_mode(pane: f64) -> crate::CommentsMode {
    if pane >= BESIDE_FLOOR {
        return crate::CommentsMode::Beside;
    }
    if pane >= SQUEEZE_FLOOR {
        return crate::CommentsMode::Squeeze;
    }
    crate::CommentsMode::Inline
}

/// The document arm's `max-w`. Only Squeeze narrows it, by exactly the card and
/// the gutter on either side of it, so the text reflows left of the card rather
/// than under it.
pub fn document_width(pane: f64, open: bool) -> f64 {
    if !open {
        return DOCUMENT_SURFACE;
    }
    match comments_mode(pane) {
        crate::CommentsMode::Beside | crate::CommentsMode::Inline => DOCUMENT_SURFACE,
        crate::CommentsMode::Squeeze => {
            (pane - COMMENTS_CARD - 2.0 * COMMENTS_GAP).clamp(DOCUMENT_MINIMUM, DOCUMENT_SURFACE)
        }
    }
}

/// The card's own width: the fixed rail, or the document's text column when it
/// drops inline under the block it belongs to.
pub fn comments_card_width(pane: f64) -> f64 {
    match comments_mode(pane) {
        crate::CommentsMode::Beside | crate::CommentsMode::Squeeze => COMMENTS_CARD,
        crate::CommentsMode::Inline => (pane.min(DOCUMENT_SURFACE) - DOCUMENT_PADDING).max(1.0),
    }
}

/// Where the card lands sideways. A float's geometry is arithmetic on its own
/// locals — a view module cannot call into it — so the choice of edge travels
/// as the WEIGHT on the right-edge term: 1 pins the card's right edge a gutter
/// inside the pane, 0 drops that term and leaves the inset below to place it.
pub fn comments_right_anchor(pane: f64) -> f64 {
    match comments_mode(pane) {
        crate::CommentsMode::Beside | crate::CommentsMode::Squeeze => 1.0,
        crate::CommentsMode::Inline => 0.0,
    }
}

/// What is added to that term: the gutter the card keeps off the pane's right
/// edge, or — inline — the document's left padding, which puts the card on the
/// text column it belongs to.
pub fn comments_left_inset(pane: f64) -> f64 {
    match comments_mode(pane) {
        crate::CommentsMode::Beside | crate::CommentsMode::Squeeze => -COMMENTS_GAP,
        crate::CommentsMode::Inline => DOCUMENT_LEFT_PADDING,
    }
}

/// Line 0 is the page title: 22px of glyph at 1.15 between the 4px block pads
/// `editor_markdown` gives it. A body line is 14px at 1.65 between the same.
const TITLE_LINE: f64 = 22.0 * 1.15 + 8.0;
const BODY_LINE: f64 = 14.0 * 1.65 + 8.0;
/// The card layer's own top edge, and so the origin every offset below is
/// measured from: the 50px document header and its 1px separator.
const LAYER_TOP: f64 = 51.0;

/// How far the card may be pushed down and still keep its 400px body above the
/// pane's bottom inset.
fn offset_ceiling(viewport_height: f64) -> f64 {
    (viewport_height - LAYER_TOP - 400.0 - COMMENTS_GAP).max(COMMENTS_GAP)
}

pub fn comment_card_offset(pane: f64, anchor_y: f64, viewport_height: f64) -> f64 {
    let ceiling = offset_ceiling(viewport_height);
    match comments_mode(pane) {
        crate::CommentsMode::Beside | crate::CommentsMode::Squeeze => {
            (anchor_y - LAYER_TOP).clamp(COMMENTS_GAP, ceiling)
        }
        crate::CommentsMode::Inline => inline_card_offset(anchor_y).clamp(COMMENTS_GAP, ceiling),
    }
}

/// Inline, the card sits in the gap the reserve opened under its line: half a
/// gutter below that line's bottom edge. With no anchored line the scope is the
/// whole page and the gap is under the title.
fn inline_card_offset(anchor_y: f64) -> f64 {
    let page_scope = anchor_y < 0.0;
    if page_scope {
        return DOCUMENT_TOP_PADDING + TITLE_LINE + COMMENTS_GAP / 2.0;
    }
    // The margin badge is centred on its line, so the pointer that opened the
    // card is half a body line above that line's bottom edge.
    anchor_y - LAYER_TOP + BODY_LINE / 2.0 + COMMENTS_GAP / 2.0
}

/// The gap the editor opens under the anchored line for an inline card: the
/// card's measured height plus the gutter below it. Zero in every other mode —
/// a card floating in the margin displaces nothing.
pub fn comments_reserve(
    pane: f64,
    open: bool,
    anchor_line: i64,
    card_height: f64,
) -> crate::editor_view::EditorReserve {
    let inline = matches!(comments_mode(pane), crate::CommentsMode::Inline);
    let height = (card_height + COMMENTS_GAP).round().max(0.0) as i64;
    match open && inline && card_height > 0.0 {
        true => crate::editor_view::EditorReserve {
            line: anchor_line.max(0),
            height,
        },
        false => crate::editor_view::EditorReserve::default(),
    }
}

/// The card's measured height, in whole pixels and taken up only when it
/// actually moved: the gap is laid out from this number, so a sub-pixel
/// remeasure of the card inside it must not re-open the gap it just measured.
pub fn measured_card_height(current: f64, measured: f64) -> f64 {
    let moved = (measured - current).abs() > 1.0;
    match moved {
        true => measured.round().max(0.0),
        false => current,
    }
}

/// The line the open scope anchors to: the block whose margin badge was
/// pressed, or line 0 — the title — for the page-scoped rail.
pub fn comment_line_after_navigation(navigation: Vec<u8>, line: i64) -> i64 {
    ui_lang_guest::wire::decode::<crate::document_source::Navigation>(&navigation)
        .ok()
        .and_then(|navigation| navigation.comment_line)
        .map_or(line, i64::from)
}

pub fn comment_line_after_props(current_page: &str, next_page: &str, open: bool, line: i64) -> i64 {
    match current_page != next_page || !open {
        true => 0,
        false => line,
    }
}

/// Replies a thread card keeps visible before it folds the rest away. Three
/// is the Docs threshold: enough to read the shape of a conversation, few
/// enough that one long thread cannot push every other one off the card.
const VISIBLE_REPLIES: usize = 3;

/// The words the thread was opened with — the body the card draws under its
/// author, above the replies. A thread whose every comment was deleted keeps
/// its row and says so rather than drawing a blank card.
pub fn opener_text(thread: &PageCommentThread) -> String {
    match thread.comments.first() {
        Some(opener) => opener.text.clone(),
        None => "This comment was deleted.".into(),
    }
}

/// The replies under it, held to [`VISIBLE_REPLIES`] until the reader asks.
pub fn thread_replies(thread: &PageCommentThread, expanded: bool) -> Vec<PageComment> {
    let replies = thread.comments.iter().skip(1);
    match expanded {
        true => replies.cloned().collect(),
        false => replies.take(VISIBLE_REPLIES).cloned().collect(),
    }
}

/// What the fold's own button says, or `""` when there is nothing to fold.
pub fn reply_toggle_label(thread: &PageCommentThread, expanded: bool) -> String {
    let hidden = thread.comments.len().saturating_sub(1 + VISIBLE_REPLIES);
    if hidden == 0 {
        return String::new();
    }
    match expanded {
        true => "Fewer replies".into(),
        false => format!("{hidden} more replies"),
    }
}

/// The open threads in the card's scope, grouped under the block they anchor
/// to in the order the document met them — the page's own threads first, since
/// the grouped query asks for the page before any of its blocks.
pub fn comment_groups(rows: Vec<PageCommentThreadRow>, page_id: &str) -> Vec<PageCommentGroup> {
    let mut groups: Vec<PageCommentGroup> = Vec::new();
    for row in rows.into_iter().filter(|row| !row.thread.resolved) {
        let anchors_to_page = row.thread.target == page_id || row.thread.target.is_empty();
        let target = row.thread.target.clone();
        match groups.iter_mut().find(|group| group.target == target) {
            Some(group) => group.threads.push(row.thread),
            None => groups.push(PageCommentGroup {
                target,
                anchor: match anchors_to_page {
                    true => "This page".into(),
                    false => row.anchor,
                },
                threads: vec![row.thread],
            }),
        }
    }
    groups
}

/// The settled threads, kept out of the list and behind their own toggle.
pub fn resolved_rows(rows: Vec<PageCommentThreadRow>) -> Vec<PageCommentThreadRow> {
    rows.into_iter().filter(|row| row.thread.resolved).collect()
}

pub fn resolved_label(rows: &[PageCommentThreadRow]) -> String {
    format!(
        "Resolved · {}",
        rows.iter().filter(|row| row.thread.resolved).count()
    )
}

/// What an empty scope says, naming the scope rather than the whole document.
pub fn empty_scope_label(scope: &str) -> String {
    match scope.is_empty() {
        true => "No comments on this page yet".into(),
        false => "No comments on this block yet".into(),
    }
}

/// The reply box follows the thread the reader picked, and a second press on
/// the same thread puts it away.
pub fn reply_thread_after_press(current: &str, pressed: &str) -> String {
    match current == pressed {
        true => String::new(),
        false => pressed.to_owned(),
    }
}

/// View-local fold state, as a set of thread ids.
pub fn expanded(ids: &[String], id: &str) -> bool {
    ids.iter().any(|held| held == id)
}

pub fn toggled(ids: Vec<String>, id: &str) -> Vec<String> {
    if ids.iter().any(|held| held == id) {
        return ids.into_iter().filter(|held| held != id).collect();
    }
    let mut ids = ids;
    ids.push(id.to_owned());
    ids
}

pub fn comment_anchor_after_props(
    current_page: &str,
    next_page: &str,
    open: bool,
    anchor: f64,
) -> f64 {
    if current_page != next_page || !open {
        -1.0
    } else {
        anchor
    }
}
pub fn comment_anchor_after_navigation(opens: bool, pointer: f64, anchor: f64) -> f64 {
    if opens { pointer } else { anchor }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CommentsMode;

    #[test]
    fn the_three_placements_are_decided_at_their_own_pane_widths() {
        // 766 of document + 340 of card + two 16px gutters.
        assert_eq!(BESIDE_FLOOR, 1138.0);
        // 560 of document at its narrowest, and the same card and gutters.
        assert_eq!(SQUEEZE_FLOOR, 932.0);
        for (pane, mode) in [
            (f64::MAX, CommentsMode::Beside),
            (1139.0, CommentsMode::Beside),
            (1138.0, CommentsMode::Beside),
            (1137.0, CommentsMode::Squeeze),
            (933.0, CommentsMode::Squeeze),
            (932.0, CommentsMode::Squeeze),
            (931.0, CommentsMode::Inline),
            (0.0, CommentsMode::Inline),
        ] {
            assert_eq!(comments_mode(pane), mode, "pane {pane}");
        }
    }

    #[test]
    fn only_a_squeeze_narrows_the_document_and_it_narrows_by_the_card_and_its_gutters() {
        // Closed, the document keeps its own width at every pane width.
        for pane in [400.0, 932.0, 1138.0, 2000.0] {
            assert_eq!(document_width(pane, false), 766.0, "pane {pane}");
        }
        assert_eq!(document_width(1138.0, true), 766.0);
        assert_eq!(document_width(2000.0, true), 766.0);
        // Squeeze gives up exactly the card and the two gutters …
        assert_eq!(document_width(1137.0, true), 1137.0 - 340.0 - 32.0);
        assert_eq!(document_width(1000.0, true), 628.0);
        // … and meets the document minimum exactly at the inline floor.
        assert_eq!(document_width(932.0, true), 560.0);
        // Inline hands the document its full width back and takes the text
        // column — the surface's own, or the pane's when the pane is narrower.
        assert_eq!(document_width(931.0, true), 766.0);
        assert_eq!(comments_card_width(931.0), 766.0 - 62.0);
        assert_eq!(comments_card_width(600.0), 600.0 - 62.0);
        assert_eq!(comments_card_width(2000.0), 340.0);
        assert_eq!(comments_card_width(932.0), 340.0);
    }

    #[test]
    fn the_card_floats_right_beside_the_document_and_onto_its_text_column_inline() {
        // The float in `pages.ice` computes exactly this.
        let placed = |pane: f64, viewport_width: f64, original_x: f64, card: f64| {
            (0.0 + viewport_width - original_x - card) * comments_right_anchor(pane)
                + comments_left_inset(pane)
                + original_x
        };
        // Beside and Squeeze: the card's right edge lands a gutter inside the
        // pane's right edge, whatever its natural position was.
        for pane in [2000.0, 1000.0] {
            assert_eq!(placed(pane, 900.0, 0.0, 340.0) + 340.0, 900.0 - 16.0);
        }
        // Inline: the card lands on the text column, 22 into the surface.
        assert_eq!(placed(800.0, 900.0, 0.0, 340.0), 22.0);
    }

    #[test]
    fn an_inline_card_drops_into_the_gap_under_its_line_and_a_floating_one_onto_the_pointer() {
        // Beside and Squeeze put the card's top on the pointer that opened it:
        // 51 of header and separator, measured from the layer's own corner.
        assert_eq!(comment_card_offset(2000.0, 300.0, 900.0), 249.0);
        assert_eq!(comment_card_offset(1000.0, 300.0, 900.0), 249.0);
        // A gutter below the separator at the top, and its body above the
        // bottom inset at the other end.
        assert_eq!(comment_card_offset(2000.0, 0.0, 900.0), 16.0);
        assert_eq!(comment_card_offset(2000.0, 5000.0, 900.0), 433.0);
        // Inline, half a body line below the pointer plus half a gutter.
        assert_eq!(
            comment_card_offset(800.0, 300.0, 900.0),
            300.0 - 51.0 + BODY_LINE / 2.0 + 8.0
        );
        // Page scope: under the title, measured from the surface's top padding.
        assert_eq!(
            comment_card_offset(800.0, -1.0, 900.0),
            26.0 + TITLE_LINE + 8.0
        );
    }

    #[test]
    fn only_an_inline_card_reserves_a_gap_and_only_once_it_has_been_measured() {
        let reserve = comments_reserve(800.0, true, 7, 210.0);
        assert_eq!(reserve.line, 7);
        assert_eq!(reserve.height, 226);
        // Closed, unmeasured, or floating in the margin: nothing is displaced.
        for (pane, open, height) in [
            (800.0, false, 210.0),
            (800.0, true, 0.0),
            (1000.0, true, 210.0),
            (2000.0, true, 210.0),
        ] {
            assert_eq!(
                comments_reserve(pane, open, 7, height),
                crate::editor_view::EditorReserve::default(),
                "pane {pane} open {open} height {height}"
            );
        }
    }

    #[test]
    fn a_remeasured_card_reopens_the_gap_only_when_it_actually_moved() {
        assert_eq!(measured_card_height(0.0, 210.4), 210.0);
        assert_eq!(measured_card_height(210.0, 210.6), 210.0);
        assert_eq!(measured_card_height(210.0, 211.0), 210.0);
        assert_eq!(measured_card_height(210.0, 211.5), 212.0);
        assert_eq!(measured_card_height(210.0, 190.0), 190.0);
    }

    #[test]
    fn the_reserved_line_is_the_pressed_block_and_a_page_scoped_rail_takes_the_title() {
        let navigation = |line: Option<u32>| {
            ui_lang_guest::wire::encode(&crate::document_source::Navigation {
                link: String::new(),
                comment_line: line,
            })
        };
        assert_eq!(comment_line_after_navigation(navigation(Some(7)), 0), 7);
        assert_eq!(comment_line_after_navigation(navigation(None), 7), 7);
        assert_eq!(comment_line_after_navigation(Vec::new(), 7), 7);
        assert_eq!(comment_line_after_props("alpha", "alpha", true, 7), 7);
        assert_eq!(comment_line_after_props("alpha", "beta", true, 7), 0);
        assert_eq!(comment_line_after_props("alpha", "alpha", false, 7), 0);
    }
}
