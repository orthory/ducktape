//! What the view asks of the host kernel, and the readings the screen folds
//! off the register it reads for itself.
//!
//! The kernel pushes only session facts (`pages.props`: connected, dark, the
//! chain the titlebar names, and the page a `duck://` link asked for). Every
//! page, block, comment thread and search hit below is read by this view —
//! `rpc.view` on the pages index tier, re-read on every `rpc.live` hit for
//! the pages plane — and folded here. A create, a delete, a comment, a
//! resolve and every op of a document save leave as `op.submit` carrying the
//! pages module's own message, signed by the kernel with the seated key: the
//! view never sees the key, the endpoint or the password.
//!
//! The document itself never crosses the wire any more. The guest owns the
//! editor, so the page's text is folded here out of its blocks and installed
//! into that editor directly; the save tick plans the reverse translation
//! ([`crate::document_sync`]) and submits its ops strictly in order.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use iced::futures::{Stream, StreamExt, stream};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use ui_lang_guest::host;

use crate::document_sync::{
    self, BlockOp, PageBlock, document_body, document_plan, document_title, page_document_text,
    stored_lines,
};

/// How many cursor pages of the page list or of one document one read walks
/// before it gives up: a bound on a cursor the node could otherwise repeat.
const MAX_CURSOR_PAGES: usize = 64;
/// The most hits one page search asks for.
const SEARCH_HITS: usize = 50;
/// The pages module's own caps, restated where the write is built so a
/// refusal is spoken here rather than by the node
/// (`crates/modules/apps/pages`: `MAX_PAGE_TITLE_LEN`, `MAX_BLOCK_LEN`,
/// `MAX_COMMENT_TEXT_BYTES`).
const MAX_PAGE_TITLE_BYTES: usize = 512;
const MAX_BLOCK_BYTES: usize = 768 * 1024;
const MAX_COMMENT_BYTES: usize = 64 * 1024;

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

// ---------- the session ----------

/// The session facts the kernel pushes, one item per change. `route_page` is
/// the page a `duck://` link asked the app to open, and `route_serial` moves
/// once per ask — the same link twice still opens the page twice.
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub connected: bool,
    pub dark: bool,
    pub chain: String,
    pub route_page: String,
    pub route_serial: i64,
}

/// One item of the session subscription: the facts, or why not.
#[derive(Clone, Debug, Default, Hash, PartialEq)]
pub struct SessionItem {
    pub next: Session,
    pub error: String,
}

/// The session now, and again on every change the kernel sees.
pub fn session() -> iced::Subscription<SessionItem> {
    iced::Subscription::run(|| {
        host::subscribe("pages.props", &[]).map(|answer| {
            let read = answer.and_then(|bytes| {
                serde_json::from_slice(&bytes).map_err(|error| error.to_string())
            });
            match read {
                Ok(next) => SessionItem {
                    next,
                    error: String::new(),
                },
                Err(error) => SessionItem {
                    next: Session::default(),
                    error,
                },
            }
        })
    })
}

/// The serial the register subscription is keyed by: it moves when the
/// session comes up, so a reconnect reads the workspace afresh.
pub fn connection_serial_after(was_connected: bool, connected: bool, serial: i64) -> i64 {
    let came_up = connected && !was_connected;
    match came_up {
        true => serial + 1,
        false => serial,
    }
}

/// Whether a route the session pushed is a fresh ask.
pub fn route_arrived(serial: i64, seen: i64) -> bool {
    serial != seen
}

// ---------- the kernel reads ----------

fn encode(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).expect("a kernel request encodes")
}

/// One index-tier view read. The kernel waits for the module's fold to carry
/// everything this client has written before it answers, so a read that
/// follows this view's own op never sees the page as it was before it.
async fn view(ask: Value) -> Result<Value, String> {
    kernel_read("rpc.view", ask).await
}

async fn kernel_read(kind: &'static str, ask: Value) -> Result<Value, String> {
    let request = json!({ "target": "pages", "query": ask });
    let reply = host::request(kind, &encode(&request)).await?;
    serde_json::from_slice(&reply).map_err(|error| error.to_string())
}

/// One signed op onto the pages module, answered with the block that took it.
async fn submit(message: Value) -> Result<i64, String> {
    let op = json!({ "target": "pages", "payload": message });
    let reply = host::request("op.submit", &encode(&op)).await?;
    String::from_utf8_lossy(&reply)
        .trim()
        .parse()
        .map_err(|_| "the node answered no block height".to_string())
}

/// A client-minted id, unique on this device: the pages module takes the id
/// of every page, block, thread and comment from its writer.
async fn mint(prefix: &str) -> Result<String, String> {
    let reply = host::request("host.id", prefix.as_bytes()).await?;
    String::from_utf8(reply).map_err(|error| error.to_string())
}

fn text_of(value: &Value) -> String {
    value.as_str().unwrap_or_default().to_owned()
}

fn rows(value: &Value) -> Vec<Value> {
    value.as_array().cloned().unwrap_or_default()
}

fn count_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// Refuse a text the module would refuse, in this screen's own words.
fn bounded(text: &str, field: &str, limit: usize) -> Result<String, String> {
    match text.len() > limit {
        true => Err(format!("{field} is longer than {limit} bytes")),
        false => Ok(text.to_owned()),
    }
}

// ---------- the register ----------

/// One item of the register subscription: the workspace as this view reads
/// it, or why not.
#[derive(Clone, Debug, Default, Hash, PartialEq)]
pub struct RegisterItem {
    pub pages: Vec<PageItem>,
    pub active_page: String,
    pub active_page_title: String,
    pub active_page_parent: String,
    pub blocks: Vec<PageBlock>,
    pub subpages: Vec<Subpage>,
    /// The page's canonical document text — its title as line 0, its blocks
    /// under it. What a clean buffer is installed with, and what a save's
    /// plan is diffed against.
    pub document: String,
    pub comment_rows: Vec<PageCommentThreadRow>,
    pub thread_total: i64,
    pub commented_hits: Vec<String>,
    pub error: String,
}

/// The workspace now and after every pages block: read once per connection
/// and per page picked, then again on each `rpc.live` hit for the pages
/// plane.
pub fn register(page: String, serial: i64) -> iced::Subscription<RegisterItem> {
    iced::Subscription::run_with((page, serial), |key| {
        let page = key.0.clone();
        let live = host::subscribe("rpc.live", b"pages");
        let first = load_register(page.clone());
        stream::once(first).chain(live.then(move |_| load_register(page.clone())))
    })
}

async fn load_register(page: String) -> RegisterItem {
    match read_register(&page).await {
        Ok(item) => item,
        Err(error) => RegisterItem {
            error,
            ..RegisterItem::default()
        },
    }
}

async fn read_register(requested: &str) -> Result<RegisterItem, String> {
    let pages = read_page_index().await?;
    // A REQUESTED PAGE THE INDEX DOES NOT HOLD IS NOT THE PAGE: it was
    // deleted, or it never existed. Falling back to the first page is what
    // keeps a delete landing somewhere readable instead of on a document the
    // network no longer has.
    let active_page = pages
        .iter()
        .find(|item| item.id == requested)
        .or_else(|| pages.first())
        .map(|item| item.id.clone())
        .unwrap_or_default();
    let active_page_parent = pages
        .iter()
        .find(|item| item.id == active_page)
        .map(|item| item.parent.clone())
        .unwrap_or_default();
    if active_page.is_empty() {
        return Ok(RegisterItem {
            pages,
            active_page_parent,
            ..RegisterItem::default()
        });
    }
    let wire = read_page_blocks(&active_page).await?;
    let active_page_title = wire
        .first()
        .map(|block| block.text.clone())
        .unwrap_or_default();
    let blocks = page_blocks(&wire, &active_page);
    let subpages = document_sync::subpages(&blocks)
        .into_iter()
        .map(|block| Subpage {
            id: block.id.clone(),
            title: block.text.clone(),
        })
        .collect();
    let document = page_document_text(&active_page_title, &blocks);
    // One grouped read, so the surface knows its comment story — the header
    // count and the commented-line washes — the moment the page opens, not
    // only once the rail is.
    let threads = read_threads(&active_page, &blocks).await?;
    let commented_hits = commented_targets(&active_page, &threads);
    let names = match threads.is_empty() {
        true => Names::default(),
        false => read_names().await,
    };
    let comment_rows = threads
        .iter()
        .map(|thread| PageCommentThreadRow {
            anchor: document_sync::comment_anchor_label(
                &blocks,
                &text_of(&thread["target"]),
                &active_page,
            ),
            thread: comment_thread(thread, &names),
        })
        .collect();
    Ok(RegisterItem {
        thread_total: count_i64(threads.len()),
        pages,
        active_page,
        active_page_title,
        active_page_parent,
        blocks,
        subpages,
        document,
        comment_rows,
        commented_hits,
        error: String::new(),
    })
}

/// Every page of the workspace index, cursor-paged, folded into sidebar rows.
async fn read_page_index() -> Result<Vec<PageItem>, String> {
    let mut wire: Vec<Value> = Vec::new();
    let mut after: Option<String> = None;
    for _ in 0..MAX_CURSOR_PAGES {
        let reply = view(json!({ "list_pages": { "after": after, "limit": null } })).await?;
        let listed = &reply["pages"];
        if listed.is_null() {
            return Err("the node returned an invalid page list".into());
        }
        wire.extend(rows(&listed["pages"]));
        let next = listed["next_after"].as_str().map(str::to_owned);
        let done = !listed["has_more"].as_bool().unwrap_or(false) || next.is_none() || next == after;
        if done {
            return Ok(page_items(&wire));
        }
        after = next;
    }
    Ok(page_items(&wire))
}

/// Every block of one page in PREORDER, cursor-paged. The page's own record
/// is element 0.
async fn read_page_blocks(page_id: &str) -> Result<Vec<WireBlock>, String> {
    let mut blocks: Vec<WireBlock> = Vec::new();
    let mut after: Option<String> = None;
    for _ in 0..MAX_CURSOR_PAGES {
        let ask = json!({ "get_page": { "page_id": page_id, "after": after, "limit": 0 } });
        let reply = view(ask).await?;
        let page = &reply["page"];
        if page.is_null() {
            return Err("the page was not found".into());
        }
        blocks.extend(rows(&page["blocks"]).iter().map(wire_block));
        let next = page["next_after"].as_str().map(str::to_owned);
        let done = next.is_none() || next == after;
        if done {
            return Ok(blocks);
        }
        after = next;
    }
    Ok(blocks)
}

/// Every thread anchored to the page or any of its blocks, one grouped read.
async fn read_threads(page_id: &str, blocks: &[PageBlock]) -> Result<Vec<Value>, String> {
    let mut targets = vec![Value::from(page_id)];
    targets.extend(blocks.iter().map(|block| Value::from(block.id.as_str())));
    let reply = view(json!({ "threads_for_targets": { "targets": targets } })).await?;
    let groups = reply["threads"].as_array().cloned();
    let Some(groups) = groups else {
        return Err("the node returned an invalid comment thread list".into());
    };
    Ok(groups
        .iter()
        .flat_map(|group| rows(&group["threads"]))
        .collect())
}

/// One block of the page as the module holds it — the shape a save's own
/// reads need, `children` and all.
#[derive(Clone, Debug, Default)]
struct WireBlock {
    id: String,
    parent: Option<String>,
    kind: String,
    text: String,
    checked: bool,
    children: Vec<String>,
}

fn wire_block(block: &Value) -> WireBlock {
    WireBlock {
        id: text_of(&block["id"]),
        parent: block["parent"].as_str().map(str::to_owned),
        kind: text_of(&block["kind"]),
        text: text_of(&block["text"]),
        checked: block["checked"].as_bool().unwrap_or(false),
        children: rows(&block["children"]).iter().map(text_of).collect(),
    }
}

/// The page's blocks as the screen reads them: the page head dropped (it is
/// line 0 of the document, not a block), each row carrying its depth prefix.
fn page_blocks(wire: &[WireBlock], page_id: &str) -> Vec<PageBlock> {
    let parents: BTreeMap<&str, Option<&str>> = wire
        .iter()
        .map(|block| (block.id.as_str(), block.parent.as_deref()))
        .collect();
    wire.iter()
        .skip(1)
        .enumerate()
        .map(|(index, block)| PageBlock {
            key: count_i64(index),
            prefix: "  ".repeat(block_depth(block, page_id, &parents)),
            id: block.id.clone(),
            parent: block.parent.clone().unwrap_or_default(),
            kind: block_kind_name(&block.kind).into(),
            text: block.text.clone(),
            checked: block.checked,
            child_count: count_i64(block.children.len()),
        })
        .collect()
}

fn block_depth(block: &WireBlock, page_id: &str, parents: &BTreeMap<&str, Option<&str>>) -> usize {
    let mut depth = 0;
    let mut parent = block.parent.as_deref();
    while let Some(parent_id) = parent {
        if parent_id == page_id || depth >= parents.len() {
            break;
        }
        depth += 1;
        parent = parents.get(parent_id).copied().flatten();
    }
    depth
}

/// The page list as a TREE, depth-first: a child follows its parent and wears
/// one indent step more. A row whose parent the index does not hold is a root.
fn page_items(wire: &[Value]) -> Vec<PageItem> {
    let known: BTreeSet<String> = wire.iter().map(|page| text_of(&page["id"])).collect();
    let mut children: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, page) in wire.iter().enumerate() {
        let parent = page["parent"]
            .as_str()
            .filter(|parent| known.contains(*parent))
            .unwrap_or_default()
            .to_owned();
        children.entry(parent).or_default().push(index);
    }
    let mut stack: Vec<(usize, usize)> = children
        .get("")
        .into_iter()
        .flatten()
        .rev()
        .map(|index| (*index, 0))
        .collect();
    let mut visited: BTreeSet<usize> = BTreeSet::new();
    let mut items = Vec::with_capacity(wire.len());
    while items.len() < wire.len() {
        let Some((index, depth)) = stack.pop() else {
            // A cycle among parents leaves rows nothing reaches: hang the
            // first of them off the root rather than dropping it.
            let Some(orphan) = (0..wire.len()).find(|index| !visited.contains(index)) else {
                break;
            };
            stack.push((orphan, 0));
            continue;
        };
        if !visited.insert(index) {
            continue;
        }
        let page = &wire[index];
        let id = text_of(&page["id"]);
        let title = text_of(&page["title"]);
        let below = children.get(&id);
        items.push(PageItem {
            title: match title.is_empty() {
                true => "Untitled".into(),
                false => title,
            },
            parent: text_of(&page["parent"]),
            prefix: "  ".repeat(depth),
            child_count: below.map_or(0, |below| count_i64(below.len())),
            id,
        });
        if let Some(below) = below {
            stack.extend(below.iter().rev().map(|index| (*index, depth + 1)));
        }
    }
    items
}

/// ONE ENTRY PER UNRESOLVED THREAD, not per block — the repetition IS the
/// count the margin chip spells. The page's own id is not a line, so it never
/// marks one.
fn commented_targets(page_id: &str, threads: &[Value]) -> Vec<String> {
    let mut targets: Vec<String> = threads
        .iter()
        .filter(|thread| !thread["resolved"].as_bool().unwrap_or(false))
        .map(|thread| text_of(&thread["target"]))
        .filter(|target| target != page_id)
        .collect();
    targets.sort();
    targets
}

fn comment_thread(thread: &Value, names: &Names) -> PageCommentThread {
    let live = rows(&thread["comments"])
        .iter()
        .filter(|comment| !comment["deleted"].as_bool().unwrap_or(false))
        .count();
    let comment_count = count_i64(live);
    let count_label = match comment_count {
        1 => "1 comment".to_string(),
        count => format!("{count} comments"),
    };
    let resolved = thread["resolved"].as_bool().unwrap_or(false);
    PageCommentThread {
        id: text_of(&thread["id"]),
        target: text_of(&thread["target"]),
        author: names.display(&text_of(&thread["opener"])),
        meta: match resolved {
            true => format!("{count_label} · resolved"),
            false => count_label,
        },
        resolved,
        comment_count,
    }
}

// ---------- who wrote it ----------

/// The account bound to each user key and to each account number, as the
/// identity module holds them. Every surface that names a key names the
/// account holding it; a key nobody claims is named by its handle.
#[derive(Clone, Debug, Default)]
struct Names {
    by_key: BTreeMap<String, String>,
    by_account: BTreeMap<i64, String>,
}

impl Names {
    /// A rendered author handle (`user:{hex}`, `acct:{number}`,
    /// `module:{id}`, `system`) as a person's name.
    fn display(&self, handle: &str) -> String {
        match handle.split_once(':') {
            Some(("user", key)) => self
                .by_key
                .get(key)
                .cloned()
                .unwrap_or_else(|| format!("user {}", short_label(key))),
            Some(("acct", number)) => number
                .parse()
                .ok()
                .and_then(|number: i64| self.by_account.get(&number).cloned())
                .unwrap_or_else(|| format!("account {number}")),
            Some(("module", id)) => id.to_owned(),
            _ => "system".into(),
        }
    }
}

/// The identity module's page cap, restated at the read that walks it.
const IDENTITY_PAGE: i64 = 256;

/// The name directory, read off the module that holds it. A directory that
/// cannot be read names nobody: every comment falls back to its handle,
/// which is what it reads as before a network has ever been seen.
async fn read_names() -> Names {
    let mut names = Names::default();
    let mut from = 0;
    for _ in 0..MAX_CURSOR_PAGES {
        let ask = json!({ "target": "identity", "query": { "all": { "from": from, "limit": IDENTITY_PAGE } } });
        let Ok(reply) = host::request("rpc.query", &encode(&ask)).await else {
            return names;
        };
        let Ok(reply) = serde_json::from_slice::<Value>(&reply) else {
            return names;
        };
        let accounts = rows(&reply["accounts"]);
        for account in &accounts {
            let number = account["number"].as_i64().unwrap_or_default();
            let name = text_of(&account["name"]);
            for key in rows(&account["keys"]) {
                names.by_key.insert(hex_encode(&json_bytes(&key["pubkey"])), name.clone());
            }
            names.by_account.insert(number, name);
        }
        let last = accounts
            .last()
            .and_then(|account| account["number"].as_i64());
        let Some(last) = last.filter(|_| count_i64(accounts.len()) >= IDENTITY_PAGE) else {
            return names;
        };
        from = last + 1;
    }
    names
}

/// A serde `Vec<u8>` as it arrives over JSON: an array of numbers.
fn json_bytes(value: &Value) -> Vec<u8> {
    rows(value)
        .iter()
        .filter_map(|byte| byte.as_u64().map(|byte| byte as u8))
        .collect()
}

fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn short_label(id: &str) -> String {
    let mut label: String = id.chars().take(8).collect();
    if id.chars().count() > 8 {
        label.push('…');
    }
    label
}

// ---------- the open thread ----------

/// One item of the open thread's subscription: its comments, or why not.
#[derive(Clone, Debug, Default, Hash, PartialEq)]
pub struct ThreadItem {
    pub thread_id: String,
    pub comments: Vec<PageComment>,
    pub resolved: bool,
    pub error: String,
}

/// The open thread's comments, re-read on every pages block.
pub fn thread(thread_id: String, target: String, serial: i64) -> iced::Subscription<ThreadItem> {
    iced::Subscription::run_with((thread_id, target, serial), |key| {
        let open = (key.0.clone(), key.1.clone());
        let live = host::subscribe("rpc.live", b"pages");
        let first = load_thread(open.clone());
        stream::once(first).chain(live.then(move |_| load_thread(open.clone())))
    })
}

async fn load_thread((thread_id, target): (String, String)) -> ThreadItem {
    match read_thread(&thread_id, &target).await {
        Ok(item) => item,
        Err(error) => ThreadItem {
            thread_id,
            error,
            ..ThreadItem::default()
        },
    }
}

async fn read_thread(thread_id: &str, target: &str) -> Result<ThreadItem, String> {
    let reply = view(json!({ "get_thread": { "thread_id": thread_id } })).await?;
    let thread = &reply["thread"];
    if thread.is_null() {
        return Err("the comment thread was not found".into());
    }
    // THE THREAD'S OWN ANCHOR, not the page: a thread an earlier build
    // anchored on a block is listed under the page but belongs to the block,
    // and reading it against the wrong target would answer somebody else's
    // discussion.
    let anchored_here = text_of(&thread["target"]) == target;
    if !anchored_here {
        return Err("the node returned comments for another block".into());
    }
    let names = read_names().await;
    let comments = rows(&thread["comments"])
        .iter()
        .filter(|comment| !comment["deleted"].as_bool().unwrap_or(false))
        .enumerate()
        .map(|(index, comment)| page_comment(index + 1, comment, &names))
        .collect();
    Ok(ThreadItem {
        thread_id: text_of(&thread["id"]),
        resolved: thread["resolved"].as_bool().unwrap_or(false),
        comments,
        error: String::new(),
    })
}

fn page_comment(ordinal: usize, comment: &Value, names: &Names) -> PageComment {
    let ordinal = count_i64(ordinal);
    let edited = !comment["edited_at"].is_null();
    PageComment {
        id: text_of(&comment["id"]),
        author: names.display(&text_of(&comment["author"])),
        meta: match edited {
            true => format!("#{ordinal} · edited"),
            false => format!("#{ordinal}"),
        },
        text: text_of(&comment["text"]),
        ordinal,
    }
}

// ---------- the search ----------

/// One item of the search subscription: the hits, or why not.
#[derive(Clone, Debug, Default, Hash, PartialEq)]
pub struct SearchItem {
    pub query: String,
    pub hits: Vec<PageSearchHit>,
    pub error: String,
}

/// The page search: one answer per query the reader sends.
pub fn search(query: String, serial: i64) -> iced::Subscription<SearchItem> {
    iced::Subscription::run_with((query, serial), |key| stream::once(run_search(key.0.clone())))
}

async fn run_search(query: String) -> SearchItem {
    match read_search(&query).await {
        Ok(hits) => SearchItem {
            query,
            hits,
            error: String::new(),
        },
        Err(error) => SearchItem {
            query,
            hits: Vec::new(),
            error,
        },
    }
}

async fn read_search(query: &str) -> Result<Vec<PageSearchHit>, String> {
    let ask = json!({ "search": { "text": query, "page_id": null, "limit": SEARCH_HITS } });
    let reply = view(ask).await?;
    let found = rows(&reply["hits"]);
    if found.is_empty() {
        return Ok(Vec::new());
    }
    // A LABEL IS DECORATION AND MUST NEVER DESTROY THE PAYLOAD: a page list
    // that fails leaves every hit on the same "Untitled" fallback an unknown
    // page id already takes, rather than discarding a search the node
    // answered.
    let titles: BTreeMap<String, String> = read_page_index()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|page| (page.id, page.title))
        .collect();
    Ok(found
        .iter()
        .map(|hit| {
            let page_id = text_of(&hit["page_id"]);
            PageSearchHit {
                page_title: titles
                    .get(&page_id)
                    .filter(|title| !title.is_empty())
                    .cloned()
                    .unwrap_or_else(|| "Untitled".into()),
                block_id: text_of(&hit["block_id"]),
                kind: block_kind_name(&text_of(&hit["kind"])).into(),
                text: text_of(&hit["text"]),
                page_id,
            }
        })
        .collect())
}

// ---------- the block vocabulary ----------

/// The block kind as the screen and the document spell it, off the module's
/// own snake_case wire.
fn block_kind_name(kind: &str) -> &'static str {
    match kind {
        "page" => "Page",
        "heading1" => "Heading 1",
        "heading2" => "Heading 2",
        "heading3" => "Heading 3",
        "bulleted" => "Bullet",
        "numbered" => "Number",
        "todo" => "Todo",
        "toggle" => "Toggle",
        "quote" => "Quote",
        "code" => "Code",
        "callout" => "Callout",
        "divider" => "Divider",
        _ => "Text",
    }
}

/// The same vocabulary, back on the wire.
fn block_kind_wire(kind: &str) -> Result<&'static str, String> {
    match kind {
        "Page" => Ok("page"),
        "Text" => Ok("paragraph"),
        "Heading 1" => Ok("heading1"),
        "Heading 2" => Ok("heading2"),
        "Heading 3" => Ok("heading3"),
        "Bullet" => Ok("bulleted"),
        "Number" => Ok("numbered"),
        "Todo" => Ok("todo"),
        "Toggle" => Ok("toggle"),
        "Quote" => Ok("quote"),
        "Code" => Ok("code"),
        "Callout" => Ok("callout"),
        "Divider" => Ok("divider"),
        _ => Err("choose a valid block type".into()),
    }
}

// ---------- the writes ----------

/// One finished act: what it landed on, and the refusal if any.
#[derive(Clone, Debug, Default, Hash, PartialEq)]
pub struct ActItem {
    /// The page a create landed on; empty for every other act.
    pub page: String,
    /// The thread a comment landed on; empty for every other act.
    pub thread: String,
    pub error: String,
}

/// One finished document save.
#[derive(Clone, Debug, Default, Hash, PartialEq)]
pub struct SaveItem {
    /// Whether anything was actually written: a no-op save settles the
    /// baseline on the text it submitted, a write on the node's own.
    pub written: bool,
    /// A plan the module cannot carry out without losing records. Not an
    /// error — nothing was attempted, and the canonical document rolls the
    /// illegal edit back.
    pub refusal: String,
    /// The page's canonical text after the save.
    pub document: String,
    pub error: String,
}

type Act = Pin<Box<dyn Future<Output = ActItem>>>;
type Save = Pin<Box<dyn Future<Output = SaveItem>>>;

#[derive(Default)]
struct Pending {
    acts: Vec<Act>,
    saves: Vec<Save>,
    act_waker: Option<Waker>,
    save_waker: Option<Waker>,
}

thread_local! {
    // One per thread = one per driver, like the guest's request registry:
    // a wasm module has one thread, and every native test drives its own
    // app on its own thread — a process-wide list would hand one app's
    // answer to another's stream.
    static PENDING: RefCell<Pending> = RefCell::default();
}

fn push_act(act: Act) -> bool {
    PENDING.with_borrow_mut(|pending| {
        pending.acts.push(act);
        if let Some(waker) = pending.act_waker.take() {
            waker.wake();
        }
    });
    true
}

fn push_save(save: Save) -> bool {
    PENDING.with_borrow_mut(|pending| {
        pending.saves.push(save);
        if let Some(waker) = pending.save_waker.take() {
            waker.wake();
        }
    });
    true
}

/// Every act's outcome, as the kernel answers it.
pub fn acts() -> iced::Subscription<ActItem> {
    iced::Subscription::run(|| ActStream)
}

/// Every save's outcome, kept apart from the acts because only a save
/// settles a baseline.
pub fn saves() -> iced::Subscription<SaveItem> {
    iced::Subscription::run(|| SaveStream)
}

/// Polls a queue of in-flight writes and yields the first one that finishes,
/// dropping it. The queue is a `Vec` and not a `select_all` because an act is
/// pushed by a handler on the window thread, long after this stream started.
fn poll_queue<T>(
    queue: &mut Vec<Pin<Box<dyn Future<Output = T>>>>,
    waker: &mut Option<Waker>,
    cx: &mut Context<'_>,
) -> Poll<Option<T>> {
    let mut finished = None;
    for (index, pending) in queue.iter_mut().enumerate() {
        if let Poll::Ready(item) = pending.as_mut().poll(cx) {
            finished = Some((index, item));
            break;
        }
    }
    let Some((index, item)) = finished else {
        *waker = Some(cx.waker().clone());
        return Poll::Pending;
    };
    drop(queue.remove(index));
    Poll::Ready(Some(item))
}

struct ActStream;

impl Stream for ActStream {
    type Item = ActItem;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<ActItem>> {
        PENDING.with_borrow_mut(|pending| {
            let Pending {
                acts, act_waker, ..
            } = pending;
            poll_queue(acts, act_waker, cx)
        })
    }
}

struct SaveStream;

impl Stream for SaveStream {
    type Item = SaveItem;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<SaveItem>> {
        PENDING.with_borrow_mut(|pending| {
            let Pending {
                saves, save_waker, ..
            } = pending;
            poll_queue(saves, save_waker, cx)
        })
    }
}

fn acted(result: Result<ActItem, String>) -> ActItem {
    match result {
        Ok(item) => item,
        Err(error) => ActItem {
            error,
            ..ActItem::default()
        },
    }
}

/// `CreatePage` — a new top-level page, landed on as soon as it exists.
pub fn create(title: &str) -> bool {
    let title = title.trim().to_owned();
    push_act(Box::pin(async move { acted(create_page(title).await) }))
}

async fn create_page(title: String) -> Result<ActItem, String> {
    if title.is_empty() {
        return Err("a page needs a title".into());
    }
    let title = bounded(&title, "page title", MAX_PAGE_TITLE_BYTES)?;
    let page_id = mint("page").await?;
    submit(json!({ "create_page": { "page_id": page_id, "title": title, "blocks": [] } })).await?;
    Ok(ActItem {
        page: page_id,
        thread: String::new(),
        error: String::new(),
    })
}

/// `RemoveBlock` on a page: the module takes its whole subtree with it.
pub fn delete(page_id: &str) -> bool {
    let page_id = page_id.to_owned();
    push_act(Box::pin(async move {
        acted(delete_page(page_id).await)
    }))
}

async fn delete_page(page_id: String) -> Result<ActItem, String> {
    if page_id.is_empty() {
        return Err("choose a page first".into());
    }
    submit(json!({ "remove_block": { "block_id": page_id } })).await?;
    Ok(ActItem::default())
}

/// `AddComment` — a new thread on `target`, or a reply on the open one.
pub fn post(text: &str, target: &str, thread_id: &str) -> bool {
    let (text, target, thread_id) = (text.trim().to_owned(), target.to_owned(), thread_id.to_owned());
    push_act(Box::pin(async move {
        acted(post_comment(text, target, thread_id).await)
    }))
}

async fn post_comment(
    text: String,
    target: String,
    thread_id: String,
) -> Result<ActItem, String> {
    if text.is_empty() || target.is_empty() {
        return Err("write a comment first".into());
    }
    let text = bounded(&text, "comment", MAX_COMMENT_BYTES)?;
    let thread_id = match thread_id.is_empty() {
        true => mint("thread").await?,
        false => thread_id,
    };
    let comment_id = mint("comment").await?;
    submit(json!({ "add_comment": {
        "thread_id": thread_id,
        "comment_id": comment_id,
        "target": target,
        "text": text,
    } }))
    .await?;
    Ok(ActItem {
        page: String::new(),
        thread: thread_id,
        error: String::new(),
    })
}

/// `ResolveThread` — flip the open thread's resolved flag.
pub fn resolve(thread_id: &str, resolved: bool) -> bool {
    let thread_id = thread_id.to_owned();
    push_act(Box::pin(async move {
        acted(resolve_thread(thread_id, resolved).await)
    }))
}

async fn resolve_thread(thread_id: String, resolved: bool) -> Result<ActItem, String> {
    if thread_id.is_empty() {
        return Err("open a comment thread first".into());
    }
    submit(json!({ "resolve_thread": { "thread_id": thread_id, "resolved": resolved } })).await?;
    Ok(ActItem::default())
}

/// Reconcile the edited buffer against the page as the node currently holds
/// it: the plan's ops, awaited strictly in order.
pub fn save(page_id: &str, text: &str, saved: &str) -> bool {
    let (page_id, text, saved) = (page_id.to_owned(), text.to_owned(), saved.to_owned());
    push_save(Box::pin(async move {
        match save_document(page_id, text, saved).await {
            Ok(item) => item,
            Err(error) => SaveItem {
                error,
                ..SaveItem::default()
            },
        }
    }))
}

async fn save_document(page_id: String, text: String, saved: String) -> Result<SaveItem, String> {
    if page_id.is_empty() {
        return Err("choose a page first".into());
    }
    // THE PLAN IS DIFFED AGAINST THIS, so it must not predate this reader's
    // own last tick: the kernel's view read waits for the module's fold to
    // carry every write this client has made. A tree missing the line the
    // previous tick inserted is not merely stale — `document_plan` pairs the
    // disturbed middle POSITIONALLY, so the line comes back as a second
    // insert and the document ends up holding it twice.
    let current = read_page_blocks(&page_id).await?;
    // A SAVE MUST PLAN AGAINST THE PAGE ITS CALLER NAMED: the plan pairs this
    // buffer against those blocks positionally, so a plan built on another
    // page's blocks is a remove for every line of a document the reader never
    // opened.
    let names_this_page = current.first().is_some_and(|head| head.id == page_id);
    if !names_this_page {
        return Err("the page was not found".into());
    }
    let node_title = current
        .first()
        .map(|head| head.text.clone())
        .unwrap_or_default();
    let blocks = page_blocks(&current, &page_id);
    let plan = document_plan(&stored_lines(&blocks), &document_body(&text));
    if !plan.refusal.is_empty() {
        return Ok(SaveItem {
            written: false,
            refusal: plan.refusal,
            document: page_document_text(&node_title, &blocks),
            error: String::new(),
        });
    }
    // The title is line 0 of the same buffer but a page property on the wire,
    // so it gets its own write — before the body, so a rename lands even if a
    // block op is refused after it.
    let title = document_title(&text);
    let title_moved = title_write_owed(&title, &saved, &node_title);
    if title_moved {
        let title = bounded(&title, "page title", MAX_PAGE_TITLE_BYTES)?;
        submit(json!({ "update_text": { "block_id": page_id, "text": title } })).await?;
    }
    let mut anchor = String::new();
    for op in &plan.ops {
        apply_op(&page_id, &mut anchor, op).await?;
    }
    let written = title_moved || !plan.ops.is_empty();
    if !written {
        return Ok(SaveItem {
            written,
            refusal: String::new(),
            document: page_document_text(&node_title, &blocks),
            error: String::new(),
        });
    }
    let landed = read_page_blocks(&page_id).await?;
    let landed_title = landed
        .first()
        .map(|head| head.text.clone())
        .unwrap_or_default();
    Ok(SaveItem {
        written,
        refusal: String::new(),
        document: page_document_text(&landed_title, &page_blocks(&landed, &page_id)),
        error: String::new(),
    })
}

/// Whether this save owes the node a title write.
///
/// A TITLE IS ONLY "MOVED" WHEN THIS READER MOVED IT. Disagreeing with the
/// node is not enough: it disagrees just as loudly when SOMEONE ELSE renamed
/// the page and this buffer has not caught up. Line 0 IS the title, so a
/// reader who never touched it still carries the old one — and the write
/// would then revert the other person's rename on chain, silently, on the
/// next keystroke.
pub fn title_write_owed(title: &str, saved: &str, node_title: &str) -> bool {
    let node_disagrees = title != node_title;
    let reader_retitled_it = title != document_title(saved);
    node_disagrees && reader_retitled_it
}

/// One op, awaited. `anchor` carries the id an insert chain hangs off: the
/// block just inserted becomes the anchor for the next one.
///
/// Three of these arms read the live tree before deciding what to write, and
/// the op before them has already moved it — which is why every read here is
/// the kernel's fold-waiting view read.
async fn apply_op(page_id: &str, anchor: &mut String, op: &BlockOp) -> Result<(), String> {
    match op {
        BlockOp::SetText { id, text } => {
            // The module bounds text per KIND; the plan never changes both in
            // the same op, so the stored kind is the one to bound against.
            let blocks = read_page_blocks(page_id).await?;
            let stored = blocks
                .iter()
                .find(|block| &block.id == id)
                .ok_or_else(|| "the block was not found".to_string())?;
            let text = bounded_block_text(block_kind_name(&stored.kind), text)?;
            submit(json!({ "update_text": { "block_id": id, "text": text } })).await?;
            Ok(())
        }
        BlockOp::SetKind { id, kind } => {
            let kind = block_kind_wire(kind)?;
            submit(json!({ "set_kind": { "block_id": id, "kind": kind } })).await?;
            Ok(())
        }
        BlockOp::SetChecked { id, checked } => {
            submit(json!({ "set_checked": { "block_id": id, "checked": checked } })).await?;
            Ok(())
        }
        BlockOp::Insert { after, kind, text } => {
            let after = match after.is_empty() {
                true => anchor.clone(),
                false => after.clone(),
            };
            *anchor = insert_block(page_id, &after, kind, text).await?;
            Ok(())
        }
        BlockOp::Nest { id, direction } => {
            // The direction is resolved against the LIVE tree; the plan never
            // names a parent.
            let blocks = read_page_blocks(page_id).await?;
            let (parent, after) = block_move(&blocks, id, direction)?;
            submit(json!({ "move_block": { "block_id": id, "parent": parent, "after": after } }))
                .await?;
            Ok(())
        }
        BlockOp::Remove { id } => {
            submit(json!({ "remove_block": { "block_id": id } })).await?;
            Ok(())
        }
    }
}

/// What the pages MODULE accepts for one block's text. The view never invents
/// its own cap: a tighter one refuses text the node would have taken, with no
/// way to shorten a block some other signer already landed.
fn bounded_block_text(kind: &str, text: &str) -> Result<String, String> {
    match kind {
        "Divider" => Ok(String::new()),
        "Page" => bounded(text, "page title", MAX_PAGE_TITLE_BYTES),
        _ => bounded(text, "block text", MAX_BLOCK_BYTES),
    }
}

/// Insert one block after `after`, adopting that block's parent — the depth
/// of a new line is the depth of the line above it, never inferred from the
/// text. An insert that anchors on nothing lands under the page's own record.
async fn insert_block(
    page_id: &str,
    after: &str,
    kind: &str,
    text: &str,
) -> Result<String, String> {
    let wire_kind = block_kind_wire(kind)?;
    let text = bounded_block_text(kind, text)?;
    if kind == "Page" && text.trim().is_empty() {
        return Err("page title must not be empty".into());
    }
    let blocks = read_page_blocks(page_id).await?;
    // The page's own record is element 0 and is never an anchor: failing to
    // find it is exactly what makes an unanchored insert land under the page
    // rather than in the page that contains it.
    let anchor = blocks
        .iter()
        .skip(1)
        .find(|block| block.id == after)
        .cloned();
    let parent = anchor
        .as_ref()
        .and_then(|block| block.parent.clone())
        .unwrap_or_else(|| page_id.to_owned());
    let id = mint("block").await?;
    submit(json!({ "insert_block": {
        "parent": parent,
        "after": anchor.map(|block| block.id),
        "block": { "id": id, "kind": wire_kind, "text": text },
    } }))
    .await?;
    Ok(id)
}

/// Where one nest step lands: the parent to move under and the sibling to
/// follow, resolved against the live tree.
fn block_move(
    blocks: &[WireBlock],
    block_id: &str,
    direction: &str,
) -> Result<(Option<String>, Option<String>), String> {
    let block = blocks
        .iter()
        .find(|block| block.id == block_id)
        .ok_or_else(|| "the block was not found".to_string())?;
    let parent_id = block
        .parent
        .as_deref()
        .ok_or_else(|| "top-level pages cannot move inside their own document".to_string())?;
    let parent = blocks
        .iter()
        .find(|block| block.id == parent_id)
        .ok_or_else(|| "the block's parent was not found".to_string())?;
    let index = parent
        .children
        .iter()
        .position(|child| child == block_id)
        .ok_or_else(|| "the block is missing from its parent".to_string())?;
    match direction {
        "indent" if index > 0 => {
            let sibling = &parent.children[index - 1];
            let new_parent = blocks
                .iter()
                .find(|block| &block.id == sibling)
                .ok_or_else(|| "the previous block was not found".to_string())?;
            Ok((
                Some(new_parent.id.clone()),
                new_parent.children.last().cloned(),
            ))
        }
        "outdent" => {
            let promotes_page = block.kind == "page" && parent.parent.is_none();
            if promotes_page {
                return Ok((None, None));
            }
            let grandparent = parent
                .parent
                .clone()
                .ok_or_else(|| "the block is already at the top level".to_string())?;
            Ok((Some(grandparent), Some(parent.id.clone())))
        }
        "indent" => Err("the block needs a previous sibling to indent under".into()),
        _ => Err("choose a valid block move".into()),
    }
}

// ---------- the OS doors ----------

/// `pages.copy` — the host puts `text` on the clipboard and toasts `label`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Copy {
    pub text: String,
    pub label: String,
}

pub fn copy(text: &str, label: &str) -> bool {
    host::notify(
        "pages.copy",
        &encode(&json!({ "text": text, "label": label })),
    );
    true
}

/// `pages.open_link` — a link pressed in the document. It goes through the
/// app's ONE open plane, not straight to the OS: a page cites `duck://`
/// addresses as readily as a chat message does, and only that plane knows
/// the module table and the network scope.
pub fn open_link(link: &str) -> bool {
    if link.is_empty() {
        return false;
    }
    host::notify("pages.open_link", &encode(&json!({ "link": link })));
    true
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

pub fn keep_i64(keep: bool, next: i64, current: i64) -> i64 {
    match keep {
        true => next,
        false => current,
    }
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

/// `duck://page/<id>?net=<chain>` — the open page's own address.
pub fn page_address(page_id: &str, chain: &str) -> String {
    if page_id.is_empty() {
        return String::new();
    }
    match chain.is_empty() {
        true => format!("duck://page/{page_id}"),
        false => format!("duck://page/{page_id}?net={chain}"),
    }
}

/// The title the sidebar shows for a page, or the one already on screen when
/// the list does not hold it yet.
pub fn page_display_title(pages: &[PageItem], id: &str, current: &str) -> String {
    pages
        .iter()
        .find(|page| page.id == id)
        .map(|page| page.title.clone())
        .unwrap_or_else(|| current.to_owned())
}

/// The composer's caption: where a NEW comment will anchor.
pub fn compose_hint_of(blocks: &[PageBlock], target: &str, page_id: &str) -> String {
    document_sync::comment_compose_hint(blocks, target, page_id)
}

/// The margin badges: one per commented block, carrying its thread count.
pub fn comment_marks(
    blocks: &[PageBlock],
    hits: &[String],
) -> Vec<crate::document_sync::CommentMark> {
    document_sync::comment_marks(blocks, hits)
}

/// The document lines wearing a commented block's wash.
pub fn commented_lines(blocks: &[PageBlock], hits: &[String]) -> Vec<i64> {
    document_sync::commented_lines(blocks, hits)
}

/// Where a thread anchors, in the reader's own words.
pub fn anchor_label(blocks: &[PageBlock], target: &str, page_id: &str) -> String {
    document_sync::comment_anchor_label(blocks, target, page_id)
}

/// The block a caret line sits in — the target a NEW comment anchors on. The
/// title and unsaved fresh lines select the page itself.
pub fn block_at_line(blocks: &[PageBlock], line: i64) -> String {
    document_sync::block_at_line(blocks, usize::try_from(line).unwrap_or(0))
}

/// The text the editor is holding.
pub fn document_text(document: &ui_lang_guest::Editor) -> String {
    document.text()
}

/// A document installed from the node: the text, caret at the origin.
pub fn document_editor(text: &str) -> ui_lang_guest::Editor {
    ui_lang_guest::Editor::new(text)
}

/// The buffer a context change installs: the incoming page's canonical text
/// when the page MOVED or the buffer is clean; the reader's own buffer when
/// they are mid-typing in the page that merely reloaded.
pub fn install_decision(
    text: &str,
    current_page: &str,
    next_page: &str,
    saved: &str,
    canonical: &str,
) -> bool {
    if current_page != next_page {
        return true;
    }
    // A clean, IDENTICAL buffer is left alone: a rebuilt editor throws the
    // caret to the origin, and there is nothing to install.
    let clean = text == saved;
    clean && canonical != saved
}

/// An open ``` swallows every line under it when parsed: the plan would
/// REMOVE every block below it, and removing a block purges its comment
/// threads. The save waits for the close — quietly.
pub fn has_unclosed_fence(text: &str) -> bool {
    document_sync::has_unclosed_fence(text)
}

/// The dirty baseline after a save settles.
///
/// A WRITE moves the baseline to the node's canonical text: anything typed
/// during the round trip — or a depth change that takes one nest step per
/// tick — stays dirty and the next tick carries it. A NO-OP save adopts the
/// submitted text instead: the buffer said the same thing in different
/// spelling (`* item` for `- item`), and a canonical baseline there would
/// leave the tick firing forever over a difference no op can close.
pub fn saved_baseline(written: bool, canonical: &str, submitted: &str) -> String {
    match written {
        true => canonical,
        false => submitted,
    }
    .to_owned()
}

/// The baseline, corrected to the title that was actually SUBMITTED.
///
/// A save adopts the node's canonical text, and that text carries a title
/// somebody else may have changed while this reader was typing — a title this
/// buffer has never displayed, because the dirty guard refuses to rebuild a
/// buffer mid-sentence. Line 0 IS the title, so taking the canonical line 0
/// makes the baseline claim a sync that did not happen, and the next tick
/// reads that manufactured difference as THIS reader retitling the page,
/// writing the old name back over the rename.
pub fn baseline_at_submitted_title(canonical: &str, submitted: &str) -> String {
    let submitted_line = submitted
        .split_once('\n')
        .map_or(submitted, |(head, _)| head);
    let title_agrees = document_title(canonical) == document_title(submitted_line);
    if title_agrees {
        return canonical.to_owned();
    }
    match canonical.split_once('\n') {
        Some((_, body)) => format!("{submitted_line}\n{body}"),
        None => submitted_line.to_owned(),
    }
}

/// The draft after a seed the screen pushed itself: the seed when it moved,
/// the reader's own text otherwise.
pub fn seeded(moved: bool, seed: &str, draft: &str) -> String {
    if moved { seed } else { draft }.to_owned()
}

/// A draft abandoned on a page the reader is leaving, kept so it can be
/// offered back. An empty draft is nothing to keep, and a draft already held
/// is not kept twice.
pub fn remember_draft(drafts: &[String], draft: &str) -> Vec<String> {
    let draft = draft.trim();
    let mut kept = drafts.to_vec();
    if draft.is_empty() || kept.iter().any(|held| held == draft) {
        return kept;
    }
    kept.push(draft.to_owned());
    kept
}

/// A recovered draft, taken up or thrown away.
pub fn forget_draft(drafts: &[String], draft: &str) -> Vec<String> {
    drafts
        .iter()
        .filter(|held| *held != draft)
        .cloned()
        .collect()
}

/// The link a document interaction named, decoded off the editor binding's
/// own navigation bytes.
pub fn navigation_link(interaction: Vec<u8>) -> String {
    decode_navigation(&interaction).link
}

/// The line a margin badge was pressed on, or `-1` when the interaction was
/// not a badge press.
pub fn navigation_comment_line(interaction: Vec<u8>) -> i64 {
    decode_navigation(&interaction)
        .comment_line
        .map_or(-1, i64::from)
}

fn decode_navigation(interaction: &[u8]) -> crate::document_sync::Navigation {
    if interaction.is_empty() {
        return crate::document_sync::Navigation::default();
    }
    ui_lang_guest::wire::decode(interaction).unwrap_or_default()
}
