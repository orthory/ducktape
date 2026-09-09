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

/// One open document tab, titled from the page list, the active one flagged.
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct DocTab {
    pub id: String,
    pub title: String,
    pub active: bool,
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

/// One comment thread on the open page.
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct PageCommentThread {
    pub id: String,
    pub target: String,
    pub author: String,
    pub meta: String,
    pub resolved: bool,
    pub comment_count: i64,
}

/// A thread with the label of the block it anchors on.
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct PageCommentThreadRow {
    pub thread: PageCommentThread,
    pub anchor: String,
}

/// One comment of the open thread.
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct PageComment {
    pub id: String,
    pub ordinal: i64,
    pub author: String,
    pub meta: String,
    pub text: String,
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
    pub doc_tabs: Vec<DocTab>,
    pub subpages: Vec<Subpage>,
    pub orphaned_comment_drafts: Vec<String>,
    pub block_comments_open: bool,
    pub thread_total: i64,
    pub comment_rows: Vec<PageCommentThreadRow>,
    pub threads_loading: bool,
    pub threads_has_more: bool,
    pub active_thread: String,
    pub thread_resolved: bool,
    pub active_thread_anchor: String,
    pub comments: Vec<PageComment>,
    pub comments_loading: bool,
    pub comments_has_more: bool,
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

/// `pages.close_tab` — close one document tab.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloseTab {
    pub id: String,
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

/// `pages.open_thread` — open one comment thread on its own anchor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenThread {
    pub id: String,
    pub target: String,
}

/// `pages.resolve` — resolve (or reopen) the open thread.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolve {
    pub resolved: bool,
}

/// `pages.post` — post `text` as a comment on the caret's block or the
/// open thread.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Post {
    pub text: String,
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

pub fn close_tab(id: &str) -> bool {
    notify("pages.close_tab", &CloseTab { id: id.into() })
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

pub fn open_thread(id: &str, target: &str) -> bool {
    notify(
        "pages.open_thread",
        &OpenThread {
            id: id.into(),
            target: target.into(),
        },
    )
}

pub fn resolve(resolved: bool) -> bool {
    notify("pages.resolve", &Resolve { resolved })
}

pub fn more_threads() -> bool {
    host::notify("pages.more_threads", &[]);
    true
}

pub fn close_thread() -> bool {
    host::notify("pages.close_thread", &[]);
    true
}

pub fn more_comments() -> bool {
    host::notify("pages.more_comments", &[]);
    true
}

pub fn post(text: &str) -> bool {
    notify("pages.post", &Post { text: text.into() })
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
pub fn edited(source: Vec<u8>, reference: Vec<u8>, navigation: Vec<u8>) -> bool {
    host::notify(
        "pages.edited",
        &serde_json::to_vec(&crate::document_source::Accepted {
            source,
            reference,
            navigation,
        })
        .expect("accepted document metadata"),
    );
    true
}
