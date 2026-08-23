//! THE APP'S FRAME-COST PROBES.
//!
//! Ported from ducktape-ui's `crates/ui-lang-runtime/tests/frame_probe.rs`,
//! but driving THIS app's generated views over state installed through the
//! real reducers rather than screen-shaped stand-ins. The app is a bin crate,
//! so `app/tests/` cannot see `Ducktape`; this lives in the crate as a
//! `#[cfg(test)]` module instead.
//!
//! Chat prints six phases and gates allocations per simulated keystroke. Five
//! other screen states gate their warmed build+layout allocation medians.
//! Wall-clock is printed, never asserted — it measures the box, not the code,
//! and an absolute microsecond budget only flakes. Allocation counts do not
//! care how busy the machine is, and the large fixtures make per-row list
//! clones and lost virtualization visible in the medians.

use std::alloc::{GlobalAlloc, Layout, System};
use std::borrow::Cow;
use std::cell::Cell;
use std::sync::Once;

use iced::advanced::renderer;
use iced::advanced::{clipboard, mouse};
use iced::{Event, Point, Size, Theme};
use iced_test::runtime::user_interface::{self, UserInterface};

use super::backend;
use super::{__DucktapeMessage, Ducktape, LiveKind, MessageAction, ShellTab};

/// One synthetic channel's worth of scrollback — `CHAT_VIEW_PAGE_LIMIT`, the
/// page the timeline walk asks for, so the probe measures the widest window a
/// single load can put on screen.
const ROWS: i64 = 256;
/// A workspace big enough that the sidebar's per-row work is visible.
const CHANNELS: i64 = 24;
const WINDOW: Size = Size::new(1440.0, 900.0);
const HUDDLE_WINDOW: Size = Size::new(320.0, 460.0);
const PAGE_ROWS: usize = 128;
/// One full thread page plus its root. The root rail deliberately is not
/// subject to the 256-root hot-window cap.
const THREAD_ROWS: i64 = backend::THREAD_HOT_WINDOW_LIMIT as i64;
const HUDDLE_ROWS: usize = 32;
const LONG_LIST_ROWS: usize = 2_048;
const FILE_ROWS: usize = 256;
const DISCUSSION_ROWS: usize = 256;
/// Settled agent answers on the shell transcript, each carrying one fenced
/// code block — the syntect surface repeated per row.
const ANSWER_ROWS: usize = 20;
/// The same interaction at four timeline sizes. A fixed-size ceiling can stay
/// green while the frame still grows linearly, so responsiveness is the slope,
/// not the single 256-row point.
const CHAT_SLOPE_ROWS: [i64; 4] = [64, 256, 1_024, 4_096];
/// Reducers deliberately pay for the bounded hot window, so their history
/// independence starts at the production cap. Loads larger than this must
/// plateau instead of carrying archive size into an interaction.
const BOUNDED_CHAT_SLOPE_ROWS: [i64; 3] = [256, 1_024, 4_096];
const SLOPE_FRAMES: usize = 5;
const KEYSTROKE_SLOPE_HEADROOM: u64 = 2_000;
const CHANNEL_SWITCH_SLOPE_HEADROOM: u64 = 2_000;
const REMOTE_BURST_SLOPE_HEADROOM: u64 = 8_000;
const REMOTE_BURST_ROWS: [usize; 2] = [32, 256];
/// A loaded or live timeline is a viewport window, not the chat archive. Older
/// history remains queryable by cursor; retaining it all here recreates the
/// O(history) rebuild the probes exist to prevent.
/// Enough passes to fill the lazy parking lot and settle the text caches.
const WARMUP_FRAMES: usize = 4;
const FRAMES: usize = 12;

/// ALLOCATIONS PER KEYSTROKE, AND NOTHING ELSE IS ASSERTED.
///
/// Measured on `dev` after the QA sweep landed: about **16 700** allocations;
/// borrow-aware `for` rows brought the same fixture to **15 982**, and keyed
/// lazy (`by message.seq, message.render_rev` over by-reference keyed rows) to
/// **11 377**. Each ceiling move locks the win. The count is stable inside one
/// process but can
/// move slightly with global font/cache initialization, so the ceiling leaves
/// broad headroom. Deleting the stream's `virtual-row=` alone takes it above
/// 27 000, still well beyond the budget.
const KEYSTROKE_ALLOCATION_CEILING: u64 = 15_000;
/// ALLOCATIONS PER CLICK ON AN ANSWER'S "Show what the agent did" FOLD.
///
/// `steps_open` is one value for the whole transcript, so it must stay OUT
/// of the answer memo's key: the fold is drawn beside the memo, and a click
/// rebuilds the one fold it moved while every answer's markdown is reclaimed.
/// 7,127 measured 2026-08-23; with `steps_open` in each row's key
/// the same click cost 197,874 (63 ms) — one full re-parse of the transcript,
/// growing with its length.
const STEPS_CLICK_ALLOCATION_CEILING: u64 = 10_000;
/// ALLOCATIONS PER `loading` FLIP UNDER A POPULATED STREAM.
///
/// `loading` is the workspace hydration flag: a page load moves it while a
/// full chat timeline is on screen (a reconnect empties the stream first and
/// lands the rows with the release). It must stay OUT of the whole-timeline
/// memo keys — the one reading inside either island was the live row's
/// `disabled=`, and a room switch empties the stream before it raises the
/// flag, so that dim never drew. Measured 2026-08-23 at `ROWS` with a
/// selected row: a flip costs **7 392** allocations (0.9 ms) with the flag out
/// of the key — the screen's own chrome rebuild — against **17 614** (6.4 ms)
/// with it in: every message cloned into the cached element and every row
/// rebuilt, growing with the timeline. With the rail open on `THREAD_ROWS`
/// replies and a reply's card up: **8 535** (1.1 ms) against **18 802**
/// (2.3 ms) with the flag in the rail's key alone.
const LOADING_FLIP_ALLOCATION_CEILING: u64 = 10_000;

struct ScreenProbe {
    label: &'static str,
    size: Size,
    fixture: fn() -> (Ducktape, iced::window::Id),
    allocation_ceiling: u64,
}

const SCREEN_PROBES: &[ScreenProbe] = &[
    // Each ceiling sits between the optimized baseline and the smallest
    // one-change negative control measured with this deterministic fixture:
    // 31,973 vs 233,957 allocations for restoring per-row anchor lookup.
    ScreenProbe {
        label: "pages comments build+layout",
        size: WINDOW,
        fixture: console_in_page_comments,
        allocation_ceiling: 120_000,
    },
    // 13,089 with the keyed (seq, render_rev) lazy vs 18,836 with the plain
    // row-hashing lazy vs 54,599 with no quiet-arm `lazy` at all — the
    // negative control that gates. Removing the rail's `virtual-row=` moves
    // build time (2.4ms -> 2.7ms median here, and every offscreen reply's
    // layout) but few allocations, so the lazy control is the one that gates.
    ScreenProbe {
        label: "chat thread rail build+layout",
        size: WINDOW,
        fixture: console_in_chat_thread,
        allocation_ceiling: 15_500,
    },
    // 4,947 vs 7,059 for restoring per-row peer lookup.
    ScreenProbe {
        label: "huddle build+layout",
        size: HUDDLE_WINDOW,
        fixture: console_in_huddle,
        allocation_ceiling: 6_000,
    },
    // 129,731 measured 2026-08-15 for a wire-cap (1,000 entry) directory
    // listing — the un-virtualized tree column at its honest worst case.
    // 152,723 measured 2026-08-16 post-descent: each row's LOCAL routes now
    // carry their captured values (rpc, repo, rev, path), ~23 allocations a
    // row over the app-routed 129,731 — the measured price of the rows
    // owning their own cycle.
    ScreenProbe {
        label: "forge tree build+layout",
        size: WINDOW,
        fixture: console_in_forge_tree_only,
        allocation_ceiling: 165_000,
    },
    // The syntect reader on a LONG file: the descent took this fixture away
    // until the component test seam could seed the blob through the update
    // loop again. 2,848 measured 2026-08-21 with the Ice-side memo boundary
    // (`lazy file_text by file_text, file_path, dark`) holding the tokenized
    // rows across unchanged frames; 650,972 without it — the negative control
    // this ceiling sits between, and the whole reason the fixture must have a
    // file OPEN. An empty reader makes the number unreachable and the ceiling
    // vacuous.
    ScreenProbe {
        label: "forge code build+layout",
        size: WINDOW,
        fixture: console_in_forge_code,
        allocation_ceiling: 146_000,
    },
    // 318,811 with the discussion rows under the keyed (seq, render_rev)
    // lazy vs 382,671 without one; 387,952 was removing discussion
    // virtualization, and removing diff virtualization is a larger
    // regression still.
    ScreenProbe {
        label: "forge PR build+layout",
        size: WINDOW,
        fixture: console_in_forge_pr,
        allocation_ceiling: 325_000,
    },
    // Re-derived at ducktape-ui af77d53e (#668). Baseline 62,167: dev-side
    // drift unrelated to that pin had already moved it 60,437 -> 61,019, and
    // #668's per-row reconciliation identity adds ~3 allocations per
    // lazy-free component row (384 rows here, +1,148) — the honest price of
    // rows that park per-row instead of sharing one scope. The controls
    // flipped order at this pin: removing the directory-row virtualization
    // is now the smallest at 63,729; restoring the selected-entry scan
    // reaches 73,453 (the per-rebuild list clone grew with the rows).
    ScreenProbe {
        label: "files build+layout",
        size: WINDOW,
        fixture: console_in_files,
        allocation_ceiling: 62_900,
    },
    // Every settled answer is an `agent_markdown` extern — a markdown parse
    // plus a syntect pass over its fenced block. 6,902 measured 2026-08-23
    // with the answer rows behind the keyed (body, provider, status, dark)
    // lazy and their steps folds drawn beside it; 195,968 (and 332 ms a
    // frame at dev opt-levels) with the extern called straight from view,
    // where an UNCHANGED frame re-parsed all twenty transcripts — the F2
    // freeze, and the negative control this ceiling sits between.
    ScreenProbe {
        label: "shell answers build+layout",
        size: WINDOW,
        fixture: console_in_shell_answers,
        allocation_ceiling: 9_000,
    },
    // The Files preview reading a Markdown document of the same twenty
    // fenced blocks through the same extern: 3,189 behind its
    // (preview_text, preview_path, dark) lazy vs 195,246 without.
    ScreenProbe {
        label: "files markdown build+layout",
        size: WINDOW,
        fixture: console_in_files_markdown,
        allocation_ceiling: 5_000,
    },
];

// ---------------------------------------------------------------------------
// The counter. A `GlobalAlloc` shim over `System`, per-thread so the rest of
// the suite running in parallel cannot pollute a sample. `const`-initialized
// TLS never allocates on first touch, which is what makes counting from inside
// the allocator safe; `try_with` covers the destructor window at thread exit.
// ---------------------------------------------------------------------------

thread_local! {
    static ALLOCATIONS: Cell<u64> = const { Cell::new(0) };
}

struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

pub(crate) fn allocations() -> u64 {
    ALLOCATIONS.with(Cell::get)
}

// ---------------------------------------------------------------------------
// Phases
// ---------------------------------------------------------------------------

struct Phase {
    label: &'static str,
    allocations: Vec<u64>,
    elapsed_us: Vec<u128>,
}

impl Phase {
    fn new(label: &'static str) -> Self {
        Self {
            label,
            allocations: Vec::with_capacity(FRAMES),
            elapsed_us: Vec::with_capacity(FRAMES),
        }
    }

    fn sample<T>(&mut self, work: impl FnOnce() -> T) -> T {
        let started = std::time::Instant::now();
        let before = allocations();
        let value = work();
        let spent = allocations() - before;
        let elapsed = started.elapsed().as_micros();
        self.allocations.push(spent);
        self.elapsed_us.push(elapsed);
        value
    }

    fn median_allocations(&self) -> u64 {
        let mut sorted = self.allocations.clone();
        sorted.sort_unstable();
        sorted[sorted.len() / 2]
    }

    fn median_us(&self) -> u128 {
        let mut sorted = self.elapsed_us.clone();
        sorted.sort_unstable();
        sorted[sorted.len() / 2]
    }

    fn report(&self) {
        eprintln!(
            "{:<28} allocs(p50)={:>7}  {:>6}us",
            self.label,
            self.median_allocations(),
            self.median_us()
        );
    }
}

// ---------------------------------------------------------------------------
// The synthetic workspace
// ---------------------------------------------------------------------------

fn probe_message(seq: i64) -> backend::ChatMessage {
    let body = format!(
        "message {seq}: the quick brown fox jumps over the lazy dog while the \
         review bot files another finding about wrapping behaviour in long chat \
         lines that span two rendered rows"
    );
    backend::ChatMessage {
        id: format!("probe-message-{seq}"),
        // Synthetic rows stay in a namespace disjoint from production keys
        // allocated for optimistic rows in this same process.
        view_key: -seq,
        seq,
        author: format!("user-{}", seq % 7),
        meta: format!("#{seq}"),
        blocks: backend::paragraph_blocks(&body),
        body,
        pending: false,
        rev: 1,
        edited: false,
        deleted: false,
        reply_count: seq % 3,
        thread_seq: 0,
        show_author: seq % 4 == 0,
        initial: "U".into(),
        avatar_kind: "human".into(),
        height: seq,
        time: seq,
        reactions: Vec::new(),
        render_rev: 0,
    }
}

fn probe_channel_with_head(index: i64, head_seq: i64) -> backend::ChatChannel {
    backend::ChatChannel {
        id: format!("channel-{index}"),
        name: format!("channel-{index}"),
        archived: false,
        members_only: false,
        huddle_count: 0,
        head_seq,
    }
}

fn probe_channel(index: i64) -> backend::ChatChannel {
    probe_channel_with_head(index, ROWS)
}

fn probe_chat_data_with_rows(channel: &str, generation: i64, rows: i64) -> backend::ChatData {
    backend::ChatData {
        generation,
        channels: (0..CHANNELS)
            .map(|index| probe_channel_with_head(index, rows))
            .collect(),
        messages: (1..=rows).map(probe_message).collect(),
        has_older_history: rows > 1,
        active_channel: channel.into(),
        active_channel_name: channel.into(),
        active_channel_archived: false,
        active_channel_members_only: false,
        huddle_roster: Vec::new(),
        channel_members: Vec::new(),
        selected_message_seq: 0,
        selected_message_rev: 0,
        selected_message_body: String::new(),
        active_thread_seq: 0,
        thread_target_seq: 0,
        thread_messages: Vec::new(),
        thread_has_more: false,
    }
}

/// A console sitting in chat, on a channel with a full page of scrollback,
/// installed through `chat_updated` — the canonical install — so every
/// mirrored field (`rooms`, `dm_rows`, `post_refusal`,
/// `unread_marker_seq`, …) holds what it would hold in front of a user.
fn console_in_chat() -> (Ducktape, iced::window::Id) {
    console_in_chat_with_rows(ROWS)
}

fn console_in_chat_with_rows(rows: i64) -> (Ducktape, iced::window::Id) {
    console_in_chat_with_thread_and_rows(Vec::new(), 0, rows)
}

/// [`console_in_chat`] with the THREAD RAIL open on message 1 and a full
/// page of replies. The rail is the app's one `for`-bound ChatMessage list
/// (the stream and the forge discussion are keyed virtual columns), so this
/// is the fixture that sees a lost `virtual-row=` or quiet-arm `lazy` on the
/// rail — and the one that measures the cheap-key `lazy … by` adoption
/// (ducktape-ui#591 made component-prop chain roots legal; every screen list
/// in this app arrives as a state-fed component prop).
fn console_in_chat_thread() -> (Ducktape, iced::window::Id) {
    let root = probe_message(1);
    let replies = (2..=THREAD_ROWS).map(|seq| {
        let mut reply = probe_message(seq);
        reply.thread_seq = 1;
        reply
    });
    let thread = std::iter::once(root).chain(replies).collect();
    let (app, console) = console_in_chat_with_thread(thread, 1);
    assert_eq!(
        app.thread_messages.len(),
        THREAD_ROWS as usize,
        "the probe drives a full page of replies, not an empty rail"
    );
    assert_eq!(app.active_thread_seq, 1, "the rail is open");
    (app, console)
}

fn console_in_chat_with_thread(
    thread_messages: Vec<backend::ChatMessage>,
    active_thread_seq: i64,
) -> (Ducktape, iced::window::Id) {
    console_in_chat_with_thread_and_rows(thread_messages, active_thread_seq, ROWS)
}

fn console_in_chat_with_thread_and_rows(
    thread_messages: Vec<backend::ChatMessage>,
    active_thread_seq: i64,
    rows: i64,
) -> (Ducktape, iced::window::Id) {
    let (mut app, _) = Ducktape::__boot();
    let console = iced::window::Id::unique();
    app.console_win = Some(console);
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.settings_user_key = "probe-user".into();

    let next = backend::ChatData {
        active_thread_seq,
        thread_messages,
        ..probe_chat_data_with_rows("channel-0", app.chat_generation, rows)
    };
    let _ = app.__update(__DucktapeMessage::ChatUpdated(next));
    let dm_peers = (0..4)
        .map(|index| backend::DmPeer {
            key: format!("peer-{index}"),
            name: format!("Direct peer {index}"),
            initials: "DP".into(),
            is_agent: false,
            channel_id: format!("dm-channel-{index}"),
        })
        .collect();
    let _ = app.__update(__DucktapeMessage::DmPeersLoaded(backend::DmPeersData {
        generation: app.dm_peers_generation,
        peers: dm_peers,
    }));
    let expected_hot_rows = usize::try_from(rows)
        .expect("the synthetic history size is non-negative")
        .min(backend::CHAT_HOT_WINDOW_LIMIT);
    let expected_hot_rows_i64 =
        i64::try_from(expected_hot_rows).expect("the hot window limit fits i64");
    assert_eq!(
        app.messages.len(),
        expected_hot_rows,
        "the probe must retain only the bounded active tail of its synthetic history"
    );
    assert_eq!(
        app.messages.last().map(|message| message.seq),
        (rows > 0).then_some(rows),
        "clamping the hot window must preserve its newest row"
    );
    assert_eq!(
        app.messages.first().map(|message| message.seq),
        (rows > 0).then_some(rows - expected_hot_rows_i64 + 1),
        "the hot window must retain a contiguous newest tail"
    );
    assert_eq!(app.dm_rows.len(), 4, "the probe mounts DIRECT rows too");
    (app, console)
}

fn console_on(tab: ShellTab) -> (Ducktape, iced::window::Id) {
    let (mut app, _) = Ducktape::__boot();
    let console = iced::window::Id::unique();
    app.console_win = Some(console);
    app.connected = true;
    app.connected_rpc = "http://node".into();
    let _ = app.__update(__DucktapeMessage::SelectShellTab(tab));
    assert_eq!(app.shell_tab, tab, "the probe mounts the requested screen");
    (app, console)
}

fn probe_page_block(index: usize) -> backend::PageBlock {
    backend::PageBlock {
        key: index as i64,
        id: format!("block-{index}"),
        parent: "page".into(),
        kind: "Text".into(),
        text: format!(
            "Page paragraph {index} gives the comment rail a stable, non-empty anchor label."
        ),
        pending: false,
        checked: false,
        prefix: String::new(),
        child_count: 0,
    }
}

fn console_in_page_comments() -> (Ducktape, iced::window::Id) {
    let (mut app, console) = console_on(ShellTab::Pages);
    let blocks: Vec<_> = (0..PAGE_ROWS).map(probe_page_block).collect();
    let _ = app.__update(__DucktapeMessage::PagesUpdated(backend::PagesData {
        pages: vec![backend::PageItem {
            id: "page".into(),
            title: "Performance notes".into(),
            parent: String::new(),
            prefix: String::new(),
            child_count: 0,
        }],
        blocks,
        active_page: "page".into(),
        active_page_title: "Performance notes".into(),
        active_page_parent: String::new(),
        comment_thread_total: PAGE_ROWS as i64,
        commented_block_hits: Vec::new(),
    }));
    let _ = app.__update(__DucktapeMessage::ToggleBlockComments);
    let generation = app.block_comments_generation;
    let _ = app.__update(__DucktapeMessage::BlockThreadsLoaded(
        backend::BlockThreadListData {
            generation,
            target: "page".into(),
            from: 0,
            threads: (0..PAGE_ROWS)
                .map(|index| backend::PageCommentThread {
                    id: format!("thread-{index}"),
                    target: format!("block-{index}"),
                    author: format!("reviewer-{}", index % 7),
                    meta: format!("#{index}"),
                    resolved: false,
                    comment_count: 1,
                })
                .collect(),
            total: PAGE_ROWS as i64,
            next_from: 0,
            has_more: false,
        },
    ));
    assert_eq!(app.blocks.len(), PAGE_ROWS);
    assert_eq!(app.block_comment_threads.len(), PAGE_ROWS);
    assert!(app.block_comments_open);
    (app, console)
}

fn probe_huddle_participant(index: usize) -> backend::HuddleParticipant {
    backend::HuddleParticipant {
        key: format!("user-{index}"),
        label: format!("Huddle member {index}"),
        initials: "HM".into(),
        is_agent: false,
        is_you: index == 0,
        joined_at: index as i64,
        node: format!("node-{index}"),
    }
}

fn console_in_huddle() -> (Ducktape, iced::window::Id) {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.settings_user_key = "user-0".into();
    let huddle = iced::window::Id::unique();
    let _ = app.__update(__DucktapeMessage::ChatUpdated(backend::ChatData {
        generation: app.chat_generation,
        channels: vec![probe_channel(0)],
        messages: Vec::new(),
        has_older_history: false,
        active_channel: "channel-0".into(),
        active_channel_name: "channel-0".into(),
        active_channel_archived: false,
        active_channel_members_only: false,
        huddle_roster: (0..HUDDLE_ROWS).map(probe_huddle_participant).collect(),
        channel_members: Vec::new(),
        selected_message_seq: 0,
        selected_message_rev: 0,
        selected_message_body: String::new(),
        active_thread_seq: 0,
        thread_target_seq: 0,
        thread_messages: Vec::new(),
        thread_has_more: false,
    }));
    let _ = app.__update(__DucktapeMessage::HuddleOpened(huddle));
    for index in 0..HUDDLE_ROWS {
        let _ = app.__update(__DucktapeMessage::CallEvent(super::call::CallEvent {
            kind: "peer".into(),
            peer: format!("node-{index}"),
            muted: index % 2 == 0,
            ..super::call::CallEvent::default()
        }));
    }
    assert_eq!(app.huddle_roster.len(), HUDDLE_ROWS);
    assert_eq!(app.call_peers.len(), HUDDLE_ROWS);
    assert_eq!(app.huddle_win, Some(huddle));
    (app, huddle)
}

fn forge_tree_rows() -> Vec<backend::TreeEntry> {
    // The wire caps one listing at forge's MAX_TREE_ENTRIES = 1,000, so this
    // is the largest directory the un-virtualized tree column can ever be
    // handed — the honest worst case for its build cost.
    (0..1_000)
        .map(|index| backend::TreeEntry {
            name: format!("file_{index:04}.rs"),
            path: format!("file_{index:04}.rs"),
            kind: "file".into(),
        })
        .collect()
}

/// The whole browse is `ForgeCodeBrowser` component state now, seeded
/// through the test seam: one headless view pass materializes the keyed
/// instance (its boot queues a real read this harness never runs), the
/// seam names the instance scope, and the listing arrives as the same
/// message the runtime would deliver.
fn console_in_forge_tree(entries: Vec<backend::TreeEntry>) -> (Ducktape, iced::window::Id, String) {
    let (mut app, console) = console_on(ShellTab::Forge);
    let _ = app.__update(__DucktapeMessage::ForgeOpenRepo("probe".into()));
    let _ = app.__update(__DucktapeMessage::ForgeRepoLoaded(backend::ForgeRepoData {
        generation: app.forge_generation,
        repo: "probe".into(),
        branches: vec!["dev".into()],
        items: Vec::new(),
    }));
    let _ = app.__view(console);
    let scope = app
        .__ice_test_scopes_forge_code_browser()
        .pop()
        .expect("the code browser materialized");
    // Deliver the sighting's queued boot NOW — its real read is a dropped
    // task against a dead endpoint — so the first pumped event pass below
    // publishes nothing but what the probe itself causes.
    let boots: Vec<__DucktapeMessage> = app.__ice_boot_queue.borrow_mut().drain(..).collect();
    for message in boots {
        let _ = app.__update(message);
    }
    let expected = entries.len();
    let _ = app.__update(Ducktape::__ice_test_message_forge_code_browser_tree_loaded(
        scope.clone(),
        backend::ForgeTreeData {
            repo: "probe".into(),
            rev: "1111111111111111111111111111111111111111".into(),
            path: String::new(),
            born: true,
            entries,
            truncated: false,
        },
    ));
    let state = app
        .__ice_test_state_forge_code_browser(&scope)
        .expect("the seeded instance answers");
    assert_eq!(state.tree_entries.len(), expected);
    (app, console, scope)
}

/// The reader open on a LONG file — the syntect surface at its measured
/// worst case, restored to the ceiling probe by the seam seed.
fn console_in_forge_code() -> (Ducktape, iced::window::Id) {
    let (mut app, console, scope) = console_in_forge_tree(vec![backend::TreeEntry {
        name: "probe.rs".into(),
        path: "probe.rs".into(),
        kind: "file".into(),
    }]);
    seed_forge_blob(&mut app, &scope, forge_source());
    (app, console)
}

fn forge_source() -> String {
    (0..LONG_LIST_ROWS)
        .map(|index| format!("let line_{index:04} = {index};\n"))
        .collect()
}

fn seed_forge_blob(app: &mut Ducktape, scope: &str, text: String) {
    seed_forge_file(app, scope, "probe.rs", text);
}

/// A solid 64×64 picture of one colour, parked under the forge surface as
/// `path`'s — what `forge_blob` leaves behind for a picture, minus the wire.
fn park_forge_picture(path: &str, rgb: [u8; 3]) {
    let pixels = [rgb[0], rgb[1], rgb[2], 255].repeat(64 * 64);
    backend::park_picture(
        backend::FORGE_SURFACE,
        path.to_owned(),
        backend::Picture {
            width: 64,
            height: 64,
            handle: backend::PictureHandle::Raster(iced::widget::image::Handle::from_rgba(
                64, 64, pixels,
            )),
        },
    );
}

/// A solid 64×64 VECTOR picture of one colour, parked under the forge
/// surface as `path`'s — the SVG branch of the same viewer.
fn park_forge_vector(path: &str, rgb: [u8; 3]) {
    let source = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"64\" height=\"64\">\
         <rect width=\"64\" height=\"64\" fill=\"rgb({},{},{})\"/></svg>",
        rgb[0], rgb[1], rgb[2]
    );
    backend::park_picture(
        backend::FORGE_SURFACE,
        path.to_owned(),
        backend::Picture {
            width: 64,
            height: 64,
            handle: backend::PictureHandle::Vector(iced::widget::svg::Handle::from_memory(
                source.into_bytes(),
            )),
        },
    );
}

/// A solid 64×64 picture of one colour, parked as `doc`'s inline picture at
/// `path` — what `forge_blob` leaves behind for a Markdown blob's image.
fn park_inline_picture(doc: &str, path: &str, rgb: [u8; 3]) {
    let pixels = [rgb[0], rgb[1], rgb[2], 255].repeat(64 * 64);
    backend::park_inline_pictures(
        doc.to_owned(),
        std::collections::HashMap::from([(
            path.to_owned(),
            backend::Picture {
                width: 64,
                height: 64,
                handle: backend::PictureHandle::Raster(iced::widget::image::Handle::from_rgba(
                    64, 64, pixels,
                )),
            },
        )]),
    );
}

/// Open `path` in the browser and land it as a picture blob through the
/// same seam `seed_forge_file` uses for text.
fn seed_forge_picture(app: &mut Ducktape, scope: &str, path: &str) {
    let _ = app.__update(Ducktape::__ice_test_message_forge_code_browser_open_file(
        scope.to_owned(),
        "http://node".into(),
        true,
        "probe".into(),
        "1111111111111111111111111111111111111111".into(),
        String::new(),
        path.into(),
    ));
    let _ = app.__update(Ducktape::__ice_test_message_forge_code_browser_file_loaded(
        scope.to_owned(),
        backend::BlobView {
            repo: "probe".into(),
            rev: "1111111111111111111111111111111111111111".into(),
            path: path.into(),
            text: String::new(),
            truncated: false,
            binary: false,
            lines: 0,
            picture: true,
            width: 64,
            height: 64,
        },
    ));
}

fn seed_forge_file(app: &mut Ducktape, scope: &str, path: &str, text: String) {
    let _ = app.__update(Ducktape::__ice_test_message_forge_code_browser_open_file(
        scope.to_owned(),
        "http://node".into(),
        true,
        "probe".into(),
        "1111111111111111111111111111111111111111".into(),
        String::new(),
        path.into(),
    ));
    let lines = text.lines().count() as i64;
    let _ = app.__update(Ducktape::__ice_test_message_forge_code_browser_file_loaded(
        scope.to_owned(),
        backend::BlobView {
            repo: "probe".into(),
            rev: "1111111111111111111111111111111111111111".into(),
            path: path.into(),
            text,
            truncated: false,
            binary: false,
            lines,
            picture: false,
            width: 0,
            height: 0,
        },
    ));
    let state = app
        .__ice_test_state_forge_code_browser(scope)
        .expect("the seeded instance answers");
    assert_eq!(state.file_path, path);
}

fn console_in_forge_tree_only() -> (Ducktape, iced::window::Id) {
    let (app, console, _) = console_in_forge_tree(forge_tree_rows());
    (app, console)
}

fn forge_diff() -> String {
    let mut diff = String::from(
        "diff --git a/probe.rs b/probe.rs\n--- a/probe.rs\n+++ b/probe.rs\n@@ -1,1 +1,2048 @@\n",
    );
    for index in 0..LONG_LIST_ROWS {
        use std::fmt::Write as _;
        writeln!(&mut diff, "+line {index:04}").expect("writing to a String cannot fail");
    }
    diff
}

fn console_in_forge_pr() -> (Ducktape, iced::window::Id) {
    let (mut app, console) = console_on(ShellTab::Forge);
    let _ = app.__update(__DucktapeMessage::ForgeOpenRepo("probe".into()));
    let _ = app.__update(__DucktapeMessage::ForgeRepoLoaded(backend::ForgeRepoData {
        generation: app.forge_generation,
        repo: "probe".into(),
        branches: vec!["dev".into()],
        items: Vec::new(),
    }));
    let _ = app.__update(__DucktapeMessage::ForgeOpenItem(7));
    let diff = forge_diff();
    assert!(diff.len() < 48 * 1024);
    let _ = app.__update(__DucktapeMessage::ForgeItemLoaded(backend::ForgeItemData {
        generation: app.forge_generation,
        repo: "probe".into(),
        number: 7,
        title: "Bound every rendered list".into(),
        state: "open".into(),
        kind: "pr".into(),
        author_name: "reviewer".into(),
        branches: "perf/probe → dev".into(),
        channel_id: "forge:probe:7".into(),
        source_branch: "perf/probe".into(),
        source_oid: "abc123".into(),
        target_oid: "def456".into(),
        diff,
        files_changed: 1,
        additions: LONG_LIST_ROWS as i64,
        ..backend::ForgeItemData::default()
    }));
    let _ = app.__update(__DucktapeMessage::ForgeDiscussionLoaded(
        backend::ForgeDiscussionData {
            channel_id: "forge:probe:7".into(),
            messages: (1..=DISCUSSION_ROWS as i64).map(probe_message).collect(),
            members: Vec::new(),
        },
    ));
    assert_eq!(
        backend::diff_lines(app.forge_item_diff.clone()).len(),
        LONG_LIST_ROWS + 4
    );
    assert_eq!(app.forge_discussion.len(), DISCUSSION_ROWS);
    (app, console)
}

fn probe_fs_entry(index: usize) -> backend::FsEntry {
    let kind = if index.is_multiple_of(2) {
        "dir"
    } else {
        "file"
    };
    backend::FsEntry {
        key: index as i64,
        path: format!("/shared/entry-{index:04}"),
        name: format!("entry-{index:04}"),
        kind: kind.into(),
        size: index as i64,
        object: format!("object-{index:04}"),
    }
}

fn console_in_files() -> (Ducktape, iced::window::Id) {
    let (mut app, console) = console_on(ShellTab::Files);
    let entries: Vec<_> = (0..FILE_ROWS).map(probe_fs_entry).collect();
    let selected = entries
        .last()
        .expect("the fixture is non-empty")
        .path
        .clone();
    let _ = app.__update(__DucktapeMessage::FsListed(backend::FsListing {
        generation: app.fs_generation,
        path: "/shared".into(),
        entries,
    }));
    let _ = app.__update(__DucktapeMessage::FsOpenFile(selected.clone()));
    let _ = app.__update(__DucktapeMessage::FsPreviewed(backend::FsPreview {
        generation: app.fs_generation,
        path: selected.clone(),
        text: "selected file preview".into(),
        truncated: false,
        binary: false,
        picture: false,
        width: 0,
        height: 0,
    }));
    assert_eq!(app.fs_entries.len(), FILE_ROWS);
    assert_eq!(app.fs_preview_path, selected);
    assert_eq!(app.fs_preview_entry.path, app.fs_preview_path);
    (app, console)
}

/// One settled answer: a heading, a paragraph, and a fenced Rust block the
/// markdown extern hands to syntect.
fn probe_answer_body(index: usize) -> String {
    let code: String = (0..24)
        .map(|line| format!("let line_{line:02} = {index} + {line};\n"))
        .collect();
    format!(
        "## Answer {index}\n\nThe run settled and committed this patch to the \
         network.\n\n```rust\n{code}```\n\nLinks route through `open_link`.\n"
    )
}

/// The two steps a settled answer keeps behind its fold.
fn probe_answer_steps(index: usize) -> Vec<backend::AgentActivity> {
    (0..2)
        .map(|step| backend::AgentActivity {
            id: (index * 2 + step) as i64,
            title: format!("step {step} of answer {index}"),
            detail: "ran the tool and read its output".into(),
            status: "done".into(),
        })
        .collect()
}

/// The shell transcript after `ANSWER_ROWS` prompt/answer turns, installed
/// through the same append seam the settle handler uses.
fn console_in_shell_answers() -> (Ducktape, iced::window::Id) {
    let (mut app, console) = console_on(ShellTab::Shell);
    let mut entries = Vec::new();
    for index in 0..ANSWER_ROWS {
        entries = backend::agent_chat_push_user(entries, format!("prompt {index}"), "codex".into());
        entries = backend::agent_chat_answer(
            entries,
            probe_answer_body(index),
            "codex".into(),
            "done".into(),
            String::new(),
            probe_answer_steps(index),
        );
    }
    app.shell_chat_entries = entries;
    assert_eq!(app.shell_chat_entries.len(), ANSWER_ROWS * 2);
    (app, console)
}

/// The Files preview open on a Markdown document carrying every probe answer.
fn console_in_files_markdown() -> (Ducktape, iced::window::Id) {
    let (mut app, console) = console_on(ShellTab::Files);
    let path = "/shared/README.md";
    let _ = app.__update(__DucktapeMessage::FsListed(backend::FsListing {
        generation: app.fs_generation,
        path: "/shared".into(),
        entries: vec![backend::FsEntry {
            key: 0,
            path: path.into(),
            name: "README.md".into(),
            kind: "file".into(),
            size: 0,
            object: "object-readme".into(),
        }],
    }));
    let _ = app.__update(__DucktapeMessage::FsOpenFile(path.into()));
    let _ = app.__update(__DucktapeMessage::FsPreviewed(backend::FsPreview {
        generation: app.fs_generation,
        path: path.into(),
        text: (0..ANSWER_ROWS).map(probe_answer_body).collect(),
        truncated: false,
        binary: false,
        picture: false,
        width: 0,
        height: 0,
    }));
    assert_eq!(app.fs_preview_path, path);
    (app, console)
}

fn headless_renderer() -> iced::Renderer {
    static LOAD_FONTS: Once = Once::new();
    LOAD_FONTS.call_once(|| {
        let mut fonts = iced::advanced::graphics::text::font_system()
            .write()
            .expect("the shared font system lock");
        fonts.load_font(Cow::Borrowed(include_bytes!(
            "../../crates/design/assets/fonts/Geist[wght].ttf"
        )));
        fonts.load_font(Cow::Borrowed(include_bytes!(
            "../../crates/design/assets/fonts/GeistMono[wght].ttf"
        )));
        fonts.load_font(Cow::Borrowed(include_bytes!(
            "../../crates/design/assets/fonts/NotoColorEmoji.ttf"
        )));
    });
    tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a current-thread runtime to block the headless renderer on")
        .block_on(<iced::Renderer as iced::advanced::renderer::Headless>::new(
            iced::Font::with_name("Geist"),
            iced::Pixels(13.5),
            Some("tiny-skia"),
        ))
        .expect("a headless tiny-skia renderer")
}

/// The stream composer's instance scope, read off a rendered frame — the
/// composers are component instances now (ducktape-ui#697), so a probe that
/// types has to name the one it is typing into.
fn composer_scope(app: &Ducktape) -> String {
    app.__ice_test_scopes_chat_composer()
        .into_iter()
        .find(|scope| scope.contains("/composer("))
        .expect("the stream composer materialized")
}

fn keystroke(scope: &str) -> __DucktapeMessage {
    Ducktape::__ice_test_message_chat_composer_composer_event(
        scope.to_owned(),
        super::editor::ComposerEvent::Apply(super::editor::RichAction::Edit(
            iced::widget::text_editor::Action::Edit(iced::widget::text_editor::Edit::Insert('x')),
        )),
        false,
        crate::ComposerKind::Message,
    )
}

fn probe_posted_update(seq: i64) -> backend::LiveUpdate {
    backend::LiveUpdate {
        kind: LiveKind::Chat,
        status: format!("Live · block {seq}"),
        height: seq,
        module: "chat".into(),
        chat: vec![backend::ChatDelta::Posted {
            channel_id: "channel-0".into(),
            seq,
            message: probe_message(seq),
        }],
        ..backend::LiveUpdate::default()
    }
}

fn assert_bounded_allocation_span(label: &str, samples: &[(i64, u64)], headroom: u64) {
    let smallest = samples
        .iter()
        .map(|(_, allocation_count)| *allocation_count)
        .min()
        .expect("an allocation slope needs at least one sample");
    let largest = samples
        .iter()
        .map(|(_, allocation_count)| *allocation_count)
        .max()
        .expect("an allocation slope needs at least one sample");
    let bounded = largest <= smallest.saturating_add(headroom);
    assert!(
        bounded,
        "{label} allocations must not grow with retained chat history: {samples:?}; \
         the {largest} maximum is more than {headroom} above the {smallest} minimum"
    );
}

// ---------------------------------------------------------------------------
// The probe
// ---------------------------------------------------------------------------

#[test]
fn a_chat_keystroke_stays_under_its_allocation_ceiling() {
    // The generated view wants a deep stack — the same 4 MiB
    // `full_view_fits_a_four_mib_stack` pins — and its own thread keeps the
    // per-thread counter clear of the rest of the suite.
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(probe)
        .expect("the probe thread spawns")
        .join()
        .expect("the probe thread finishes");
}

#[test]
fn chat_keystroke_cost_does_not_grow_with_retained_history() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(probe_chat_keystroke_slope)
        .expect("the keystroke slope probe thread spawns")
        .join()
        .expect("the keystroke slope probe thread finishes");
}

fn probe_chat_keystroke_slope() {
    let samples: Vec<_> = CHAT_SLOPE_ROWS
        .into_iter()
        .map(|rows| (rows, chat_keystroke_allocations(rows)))
        .collect();
    eprintln!("chat keystroke allocation slope: {samples:?}");
    assert_bounded_allocation_span("chat keystroke+rebuild", &samples, KEYSTROKE_SLOPE_HEADROOM);
}

fn chat_keystroke_allocations(rows: i64) -> u64 {
    let (mut app, console) = console_in_chat_with_rows(rows);
    let mut renderer = headless_renderer();
    let mut cache = warm_settled(
        "the chat keystroke slope probe",
        &mut app,
        console,
        WINDOW,
        &mut renderer,
        user_interface::Cache::default(),
    );
    let mut keystrokes = Phase::new("composer keystroke+rebuild slope");
    let composer = composer_scope(&app);
    for _ in 0..SLOPE_FRAMES {
        cache = keystrokes
            .sample(|| {
                let _ = app.__update(keystroke(&composer));
                UserInterface::build(app.__view(console), WINDOW, cache, &mut renderer)
            })
            .into_cache();
    }
    keystrokes.median_allocations()
}

#[test]
fn a_steps_fold_click_rebuilds_one_answer_not_the_transcript() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(probe_steps_click)
        .expect("the steps click probe thread spawns")
        .join()
        .expect("the steps click probe thread finishes");
}

/// Open a different answer's fold on every frame — each click closes the
/// previous fold and opens the next — and measure the rebuild that follows.
fn probe_steps_click() {
    let (mut app, console) = console_in_shell_answers();
    let mut renderer = headless_renderer();
    let mut cache = warm_settled(
        "the steps click probe",
        &mut app,
        console,
        WINDOW,
        &mut renderer,
        user_interface::Cache::default(),
    );
    let answers: Vec<i64> = app
        .shell_chat_entries
        .iter()
        .filter(|entry| entry.role != "user")
        .map(|entry| entry.id)
        .collect();
    let mut clicks = Phase::new("steps click+rebuild");
    for id in answers.into_iter().take(FRAMES) {
        cache = clicks
            .sample(|| {
                let _ = app.__update(__DucktapeMessage::ShellChatStepsToggled(id));
                UserInterface::build(app.__view(console), WINDOW, cache, &mut renderer)
            })
            .into_cache();
    }
    clicks.report();
    let allocations = clicks.median_allocations();
    assert!(
        allocations < STEPS_CLICK_ALLOCATION_CEILING,
        "a steps click rebuilt in {allocations} allocations, over the \
         {STEPS_CLICK_ALLOCATION_CEILING} ceiling. Keep `steps_open` out of the answer \
         memo's key before changing the budget."
    );
}

#[test]
fn a_loading_flip_leaves_the_timeline_memo_alone() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(probe_loading_flip)
        .expect("the loading flip probe thread spawns")
        .join()
        .expect("the loading flip probe thread finishes");
}

/// Move the workspace `loading` flag under a populated, selected stream on
/// every frame — a page load leaving, then its failure releasing it — and
/// measure the rebuild that follows. The selected rows are the ones that
/// ever read the flag, so both fixtures carry one: the stream alone, then
/// the stream with the thread rail open and a reply's action card up.
fn probe_loading_flip() {
    let (app, console) = console_in_chat();
    flip_loading_under("loading flip+rebuild (stream)", app, console);
    let (mut app, console) = console_in_chat_thread();
    let newest = app
        .thread_messages
        .last()
        .cloned()
        .expect("the probe drives a populated rail");
    let _ = app.__update(__DucktapeMessage::OpenThreadMessageActions(
        newest.seq,
        newest.body,
        newest.rev,
    ));
    assert_eq!(
        app.thread_selected_seq, newest.seq,
        "the rail's live row is on screen"
    );
    flip_loading_under("loading flip+rebuild (stream+rail)", app, console);
}

fn flip_loading_under(label: &'static str, mut app: Ducktape, console: iced::window::Id) {
    let newest = app
        .messages
        .last()
        .cloned()
        .expect("the probe drives a populated stream");
    let _ = app.__update(__DucktapeMessage::OpenMessageActions(
        newest.seq,
        newest.body,
        newest.rev,
    ));
    assert_eq!(
        app.selected_message_seq, newest.seq,
        "the live row is on screen"
    );
    assert!(!app.loading, "the landing released the flag");
    let mut renderer = headless_renderer();
    let mut cache = warm_settled(
        "the loading flip probe",
        &mut app,
        console,
        WINDOW,
        &mut renderer,
        user_interface::Cache::default(),
    );
    let mut flips = Phase::new(label);
    for frame in 0..FRAMES {
        let raise = frame % 2 == 0;
        // An empty failure message keeps the error banner out of the frame,
        // so the delta between two frames is the flag and nothing else.
        let flip = if raise {
            __DucktapeMessage::ChoosePage("probe-page".into())
        } else {
            __DucktapeMessage::Failed(backend::AppError {
                message: String::new(),
                committed: false,
            })
        };
        cache = flips
            .sample(|| {
                let _ = app.__update(flip);
                assert_eq!(app.loading, raise, "the fixture must move the flag");
                UserInterface::build(app.__view(console), WINDOW, cache, &mut renderer)
            })
            .into_cache();
    }
    flips.report();
    let allocations = flips.median_allocations();
    assert!(
        allocations < LOADING_FLIP_ALLOCATION_CEILING,
        "{label}: a loading flip rebuilt in {allocations} allocations, over the \
         {LOADING_FLIP_ALLOCATION_CEILING} ceiling. Keep `loading` out of the timeline \
         memo keys before changing the budget."
    );
}

#[test]
fn remote_post_bursts_publish_reduce_and_rebuild_once_per_bounded_batch() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(probe_remote_post_bursts)
        .expect("the remote burst probe thread spawns")
        .join()
        .expect("the remote burst probe thread finishes");
}

#[test]
fn live_chat_batches_take_one_shipping_app_message() {
    const LIFECYCLE: &str = include_str!("ui/handlers/lifecycle.ice");
    let live_updated = LIFECYCLE
        .split_once("on live_updated(next)")
        .expect("the shipping live handler")
        .1
        .split("\non ")
        .next()
        .expect("the live handler body");

    assert_eq!(
        LIFECYCLE
            .matches("run live_events(connected_rpc) when connected -> live_updated _")
            .count(),
        1,
        "the live subscription must route each publication straight to live_updated"
    );
    assert!(
        live_updated.contains("match next.kind"),
        "live_updated must dispatch once on its closed LiveKind"
    );
    for variant in [
        "retry", "tip", "ready", "chat", "bell", "pages", "forge", "plane", "resync",
    ] {
        assert_eq!(
            live_updated.matches(&format!("LiveKind.{variant}")).count(),
            1,
            "every LiveKind variant must have exactly one explicit arm"
        );
    }
    assert_eq!(
        live_updated.matches("fold_live_chat(next.chat").count(),
        1,
        "the shipping handler has one fused chat fold"
    );
    let chat_arm = live_updated.find("LiveKind.chat").expect("the chat arm");
    let chat_fold = live_updated
        .find("fold_live_chat(next.chat")
        .expect("the fused chat fold");
    let bell_arm = live_updated.find("LiveKind.bell").expect("the bell arm");
    assert!(
        chat_arm < chat_fold && chat_fold < bell_arm,
        "fold_live_chat and its by-value arguments must exist only inside the selected chat arm"
    );
    assert!(
        !LIFECYCLE.contains("live_chat_updated") && !LIFECYCLE.contains("chat_live_update("),
        "a chat task/second handler costs a second global app update and rebuild per publication"
    );
    let generated_dir = std::path::Path::new(env!("OUT_DIR")).join("ui-lang-generated");
    let has_two_hop_variant = std::fs::read_dir(generated_dir)
        .expect("the generated ui-lang directory")
        .filter_map(|entry| std::fs::read_to_string(entry.ok()?.path()).ok())
        .any(|source| source.contains("LiveChatUpdated("));
    assert!(
        !has_two_hop_variant,
        "the generated message enum must not carry a two-hop live chat route"
    );
}

fn probe_remote_post_bursts() {
    for burst_rows in REMOTE_BURST_ROWS {
        let samples = [256, 4_096].map(|history_rows| {
            let mut repetitions = [0; 3];
            for sample in &mut repetitions {
                *sample = remote_post_burst_allocations(history_rows, burst_rows);
            }
            repetitions.sort_unstable();
            repetitions[1]
        });
        let allocation_slope = [(256, samples[0]), (4_096, samples[1])];
        eprintln!(
            "remote {burst_rows}-post allocation slope by historical rows: {allocation_slope:?}"
        );
        assert_bounded_allocation_span(
            "remote post batch reducer+rebuild",
            &allocation_slope,
            REMOTE_BURST_SLOPE_HEADROOM,
        );
    }
}

fn remote_post_burst_allocations(history_rows: i64, burst_rows: usize) -> u64 {
    let (mut app, console) = console_in_chat_with_rows(history_rows);
    let mut renderer = headless_renderer();
    let mut cache = warm_settled(
        "the remote post burst probe",
        &mut app,
        console,
        WINDOW,
        &mut renderer,
        user_interface::Cache::default(),
    );
    // Prime the CHANGED-timeline path too. The initial warm-up above only
    // exercises an unchanged dependency hit; without this first unmeasured
    // publication the smaller fixture pays one-time memo/layout allocation in
    // the measured phase while a prior large fixture may reclaim it. The
    // slope compares steady-state remote publications, not process-global
    // cache history.
    let warm_sequence = history_rows + 1;
    let _ = app.__update(__DucktapeMessage::LiveUpdated(probe_posted_update(
        warm_sequence,
    )));
    cache = warm_settled(
        "the remote post burst changed-timeline path",
        &mut app,
        console,
        WINDOW,
        &mut renderer,
        cache,
    );
    let burst_rows_i64 = i64::try_from(burst_rows).expect("the burst size fits i64");
    let expected_sequences: Vec<_> =
        ((warm_sequence + 1)..=(warm_sequence + burst_rows_i64)).collect();
    let publications = backend::batch_live_updates(
        expected_sequences
            .iter()
            .copied()
            .map(probe_posted_update)
            .collect(),
    );
    let expected_publications = burst_rows.div_ceil(backend::LIVE_CHAT_BATCH_LIMIT);
    assert_eq!(
        publications.len(),
        expected_publications,
        "a ready chat burst must publish once per bounded batch"
    );
    assert!(
        publications.iter().all(|update| {
            !update.chat.is_empty() && update.chat.len() <= backend::LIVE_CHAT_BATCH_LIMIT
        }),
        "every chat publication must be non-empty and respect the production cap"
    );
    let published_sequences: Vec<_> = publications
        .iter()
        .flat_map(|update| {
            update.chat.iter().map(|delta| {
                let backend::ChatDelta::Posted { seq, .. } = delta else {
                    panic!("the posted-update fixture emitted another transition")
                };
                *seq
            })
        })
        .collect();
    assert_eq!(
        published_sequences, expected_sequences,
        "batching must preserve every delta in wire order"
    );

    let revision_before = app.messages_revision;
    let mut reducer_updates = 0usize;
    let mut view_builds = 0usize;
    let mut phase = Phase::new("remote post batch reducer+rebuild");
    for publication in publications {
        cache = phase
            .sample(|| {
                let _ = app.__update(__DucktapeMessage::LiveUpdated(publication));
                reducer_updates += 1;
                let ui = UserInterface::build(app.__view(console), WINDOW, cache, &mut renderer);
                view_builds += 1;
                ui
            })
            .into_cache();
    }
    assert_eq!(reducer_updates, expected_publications);
    assert_eq!(view_builds, expected_publications);
    assert_eq!(
        app.messages_revision - revision_before,
        i64::try_from(expected_publications).expect("the publication count fits i64"),
        "one batch advances the timeline revision once, not once per delta"
    );

    let expected_hot_rows = usize::try_from(history_rows)
        .expect("the synthetic history size is non-negative")
        .saturating_add(1)
        .saturating_add(burst_rows)
        .min(backend::CHAT_HOT_WINDOW_LIMIT);
    assert_eq!(app.messages.len(), expected_hot_rows);
    assert_eq!(
        app.messages.last().map(|message| message.seq),
        expected_sequences.last().copied(),
        "the active tail must reach the newest remote post"
    );
    assert!(
        app.messages
            .windows(2)
            .all(|pair| pair[0].seq + 1 == pair[1].seq),
        "the bounded active tail must stay ordered, contiguous, and duplicate-free"
    );
    phase.median_allocations()
}

#[test]
fn large_screens_stay_under_their_allocation_ceilings() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(probe_large_screens)
        .expect("the screen probe thread spawns")
        .join()
        .expect("the screen probe thread finishes");
}

fn probe_large_screens() {
    eprintln!(
        "large screen frame probes: {PAGE_ROWS} page rows, {HUDDLE_ROWS} huddle rows, \
         {LONG_LIST_ROWS} source/diff rows, {DISCUSSION_ROWS} discussion rows, \
         {FILE_ROWS} file rows, {ANSWER_ROWS} answer rows"
    );
    for probe in SCREEN_PROBES {
        let (app, window) = (probe.fixture)();
        let allocations = probe_unchanged_build(probe.label, app, window, probe.size);
        assert!(
            allocations < probe.allocation_ceiling,
            "{} rebuilt in {allocations} allocations, over the {} ceiling. Restore the \
             prepared row projection or keyed virtual-row before changing the budget.",
            probe.label,
            probe.allocation_ceiling,
        );
    }
}

/// Warm a fixture until it stops emitting, applying what the view emits — a
/// first frame may PRIME it (chat's scrolls publish their initial viewport as
/// a real `chat_scrolled` message, exactly as they do in front of a user) —
/// and hand back the settled cache. A fixture still emitting on the last warm
/// frame is a feedback loop the caller would measure (or screenshot) instead
/// of an unchanged frame.
fn warm_settled(
    label: &'static str,
    app: &mut Ducktape,
    window: iced::window::Id,
    size: Size,
    renderer: &mut iced::Renderer,
    mut cache: user_interface::Cache,
) -> user_interface::Cache {
    let mut clipboard = clipboard::Null;
    let mut messages: Vec<__DucktapeMessage> = Vec::new();
    for warm_frame in 0..WARMUP_FRAMES {
        let mut ui = UserInterface::build(app.__view(window), size, cache, renderer);
        ui.update(
            &[Event::Window(iced::window::Event::RedrawRequested(
                iced::time::Instant::now(),
            ))],
            mouse::Cursor::Unavailable,
            renderer,
            &mut clipboard,
            &mut messages,
        );
        let settled = messages.is_empty();
        let last_warm_frame = warm_frame + 1 == WARMUP_FRAMES;
        assert!(
            settled || !last_warm_frame,
            "warming {label} did not settle: {} messages on the last warm frame",
            messages.len()
        );
        cache = ui.into_cache();
        for message in messages.drain(..) {
            let _ = app.__update(message);
        }
    }
    cache
}

fn probe_unchanged_build(
    label: &'static str,
    mut app: Ducktape,
    window: iced::window::Id,
    size: Size,
) -> u64 {
    let mut renderer = headless_renderer();
    let mut cache = warm_settled(
        label,
        &mut app,
        window,
        size,
        &mut renderer,
        user_interface::Cache::default(),
    );

    let mut build = Phase::new(label);
    for _ in 0..FRAMES {
        cache = build
            .sample(|| UserInterface::build(app.__view(window), size, cache, &mut renderer))
            .into_cache();
    }
    build.report();
    build.median_allocations()
}

/// One drawn console frame as raw RGBA — settle first (a state change may make
/// the anchored scroll republish its viewport), then build, update, draw, and
/// screenshot, exactly the runtime's own paint order.
fn drawn_frame(
    app: &mut Ducktape,
    window: iced::window::Id,
    renderer: &mut iced::Renderer,
    cache: user_interface::Cache,
) -> (user_interface::Cache, Vec<u8>) {
    use iced::advanced::renderer::Headless as _;
    use iced::theme::Base as _;
    let theme = Theme::Dark;
    let base = theme.base();
    let cache = warm_settled("the repaint probe", app, window, WINDOW, renderer, cache);
    let mut ui = UserInterface::build(app.__view(window), WINDOW, cache, renderer);
    ui.draw(
        renderer,
        &theme,
        &renderer::Style {
            text_color: base.text_color,
        },
        mouse::Cursor::Unavailable,
    );
    let cache = ui.into_cache();
    let physical = Size {
        width: WINDOW.width as u32,
        height: WINDOW.height as u32,
    };
    let pixels = renderer.screenshot(physical, 1.0, base.background_color);
    (cache, pixels)
}

/// A settled optimistic row keeps the identity already mounted in the keyed
/// virtual timeline. Replacing its client identity with the canonical sequence
/// used to wedge the main stream while the unkeyed thread rail kept working.
#[test]
fn an_optimistic_confirmation_keeps_its_virtual_row_alive() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(probe_optimistic_confirmation)
        .expect("the confirmation probe thread spawns")
        .join()
        .expect("the confirmation probe thread finishes");
}

fn probe_optimistic_confirmation() {
    let (mut app, console) = console_in_chat();
    let _ = app.__update(__DucktapeMessage::ComposerSubmitted(
        crate::ComposerKind::Message,
        "confirmation probe".into(),
        backend::fresh_operation_id("message".into()),
    ));
    assert_eq!(app.messages.len(), backend::CHAT_HOT_WINDOW_LIMIT);
    let pending = app.messages.last().expect("the optimistic row");
    assert!(
        pending.pending,
        "the bounded tail must retain the optimistic row"
    );
    let operation_id = pending.id.clone();
    let pending_view_key = pending.view_key;
    assert_eq!(
        app.messages.first().map(|message| message.seq),
        Some(2),
        "making room for the pending row drops the oldest committed root"
    );

    let mut renderer = headless_renderer();
    let (cache, pending_frame) = drawn_frame(
        &mut app,
        console,
        &mut renderer,
        user_interface::Cache::default(),
    );

    let canonical = backend::ChatMessage {
        id: operation_id.clone(),
        view_key: pending_view_key.saturating_add(10_000),
        seq: ROWS + 1,
        body: "confirmation probe".into(),
        blocks: backend::paragraph_blocks("confirmation probe"),
        ..probe_message(ROWS + 1)
    };
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: LiveKind::Chat,
        chat: vec![backend::ChatDelta::Posted {
            channel_id: "channel-0".into(),
            seq: canonical.seq,
            message: canonical,
        }],
        ..backend::LiveUpdate::default()
    }));

    let confirmed = app
        .messages
        .iter()
        .find(|message| message.id == operation_id)
        .expect("the canonical row replaces its optimistic row");
    assert_eq!(confirmed.view_key, pending_view_key);
    assert!(!confirmed.pending);
    assert_eq!(app.messages.len(), backend::CHAT_HOT_WINDOW_LIMIT);
    assert_eq!(
        app.messages.last().map(|message| message.seq),
        Some(ROWS + 1),
        "confirmation keeps the canonical row at the active tail"
    );

    let (_, confirmed_frame) = drawn_frame(&mut app, console, &mut renderer, cache);
    assert_ne!(
        pending_frame, confirmed_frame,
        "the pending dot must disappear when the row becomes canonical"
    );
}

/// THE STALENESS GUARD. Under the keyed lazy a quiet row repaints ONLY when
/// (seq, render_rev) moves, so a mutation path that misses its `render_rev`
/// bump is not a perf regression but a WRONG FRAME — the reader keeps looking
/// at the pre-mutation row. Drive the two in-place folds a reader sees most —
/// a reaction and an edit — through the app's real live-delta path and assert
/// each repaints the DRAWN frame, with an unchanged-frame control proving the
/// diffs mean repaint rather than render noise.
#[test]
fn a_reaction_and_an_edit_repaint_the_visible_row() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(probe_row_repaint)
        .expect("the repaint probe thread spawns")
        .join()
        .expect("the repaint probe thread finishes");
}

fn probe_row_repaint() {
    let (mut app, console) = console_in_chat();
    let mut renderer = headless_renderer();

    let cache = user_interface::Cache::default();
    let (cache, quiet) = drawn_frame(&mut app, console, &mut renderer, cache);
    let (cache, control) = drawn_frame(&mut app, console, &mut renderer, cache);
    assert!(
        quiet == control,
        "an unchanged frame must draw identical pixels — without this control \
         the repaint assertions below prove nothing"
    );

    // A reaction lands on the BOTTOM row — visible under `anchor-y=end`, and
    // in the quiet arm (nothing selected), so the repaint
    // must come through the keyed lazy's (seq, render_rev) move.
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: LiveKind::Chat,
        chat: vec![backend::ChatDelta::Reaction {
            channel_id: "channel-0".into(),
            seq: ROWS,
            emoji: "👍".into(),
            added: true,
            reactor: "user:repaint-probe".into(),
            by_me: false,
        }],
        ..backend::LiveUpdate::default()
    }));
    let (cache, reacted) = drawn_frame(&mut app, console, &mut renderer, cache);
    assert!(
        control != reacted,
        "a reaction delta must repaint the visible row — `merge_message_reaction` \
         stopped moving `render_rev` if this frame is unchanged"
    );

    // An edit of the same row, one wire revision up.
    let body = "edited by the repaint probe";
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: LiveKind::Chat,
        chat: vec![backend::ChatDelta::Edited {
            channel_id: "channel-0".into(),
            seq: ROWS,
            message: backend::ChatMessage {
                rev: 2,
                body: body.into(),
                blocks: backend::paragraph_blocks(body),
                meta: format!("#{ROWS} · edited"),
                ..backend::ChatMessage::default()
            },
        }],
        ..backend::LiveUpdate::default()
    }));
    let (_, edited) = drawn_frame(&mut app, console, &mut renderer, cache);
    assert!(
        reacted != edited,
        "an edit delta must repaint the visible row — `apply_edit_content` \
         stopped moving `render_rev` if this frame is unchanged"
    );
}

/// A MARKDOWN BLOB'S INLINE PICTURE IS PROVEN BY ITS PIXELS, the same way:
/// the document and its text never change between the two frames, only the
/// picture parked for `logo.png` does.
#[test]
fn the_forge_reader_draws_a_markdown_blobs_inline_picture() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(probe_forge_inline_picture_content)
        .expect("the forge inline picture probe thread spawns")
        .join()
        .expect("the forge inline picture probe thread finishes");
}

fn probe_forge_inline_picture_content() {
    let (mut app, console, scope) = console_in_forge_tree(vec![backend::TreeEntry {
        name: "README.md".into(),
        path: "README.md".into(),
        kind: "file".into(),
    }]);
    let mut renderer = headless_renderer();
    let (cache, empty) = drawn_frame(
        &mut app,
        console,
        &mut renderer,
        user_interface::Cache::default(),
    );

    park_inline_picture("README.md", "./logo.png", [220, 40, 40]);
    seed_forge_file(
        &mut app,
        &scope,
        "README.md",
        "# Probe\n\nA line before.\n\n![logo](./logo.png)\n\nA line after.\n".into(),
    );
    let (cache, red) = drawn_frame(&mut app, console, &mut renderer, cache);
    assert!(
        empty != red,
        "a loaded markdown blob must repaint the reader out of its empty plate"
    );
    let (cache, red_again) = drawn_frame(&mut app, console, &mut renderer, cache);
    assert!(
        red == red_again,
        "an unchanged frame must draw identical pixels — without this control \
         the swap assertion below proves nothing"
    );
    let (cache, red_again) = drawn_frame(&mut app, console, &mut renderer, cache);
    assert!(
        red == red_again,
        "an unchanged frame must draw identical pixels — without this control \
         the swap assertion below proves nothing"
    );

    park_inline_picture("README.md", "./logo.png", [40, 40, 220]);
    let (_cache, blue) = drawn_frame(&mut app, console, &mut renderer, cache);
    assert!(
        red != blue,
        "a replaced inline picture must repaint — identical pixels mean the \
         markdown viewer draws alt text, a stale handle, or no picture at all"
    );
}

/// THE READER MUST DRAW THE PICTURE. The `picture` extern reads a handle out
/// of a process-wide slot, so nothing in the Ice tree changes between two
/// pictures at the same path: only the pixels can prove the extern drew the
/// slot's handle at all — and drew the CURRENT one, not a cached first.
///
/// Raster and vector run in ONE test, in sequence: the forge surface is one
/// process-wide slot, and two tests parking different paths in it from
/// parallel threads would blank each other's frames.
#[test]
fn the_forge_reader_draws_the_loaded_picture() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            probe_forge_picture_content("logo.png", park_forge_picture);
            // THE VECTOR BRANCH DRAWS TOO — resvg under the same headless
            // renderer, the same red→blue swap with nothing in the Ice tree
            // changed.
            probe_forge_picture_content("logo.svg", park_forge_vector);
        })
        .expect("the forge picture probe thread spawns")
        .join()
        .expect("the forge picture probe thread finishes");
}

fn probe_forge_picture_content(file: &str, park: fn(&str, [u8; 3])) {
    let (mut app, console, scope) = console_in_forge_tree(vec![backend::TreeEntry {
        name: file.into(),
        path: file.into(),
        kind: "file".into(),
    }]);
    let mut renderer = headless_renderer();
    let (cache, empty) = drawn_frame(
        &mut app,
        console,
        &mut renderer,
        user_interface::Cache::default(),
    );

    park(file, [220, 40, 40]);
    seed_forge_picture(&mut app, &scope, file);
    let (cache, red) = drawn_frame(&mut app, console, &mut renderer, cache);
    assert!(
        empty != red,
        "a loaded picture must repaint the reader out of its empty plate"
    );
    let (cache, red_again) = drawn_frame(&mut app, console, &mut renderer, cache);
    assert!(
        red == red_again,
        "an unchanged frame must draw identical pixels — without this control \
         the swap assertion below proves nothing"
    );
    let (cache, red_again) = drawn_frame(&mut app, console, &mut renderer, cache);
    assert!(
        red == red_again,
        "an unchanged frame must draw identical pixels — without this control \
         the swap assertion below proves nothing"
    );

    // Same path, same Ice state, a different picture in the slot: only the
    // extern's draw can move these pixels.
    park(file, [40, 40, 220]);
    let (_cache, blue) = drawn_frame(&mut app, console, &mut renderer, cache);
    assert!(
        red != blue,
        "a replaced picture must repaint — identical pixels mean the reader \
         draws a stale handle, or no handle at all"
    );
}

/// THE READER MUST DRAW THE BLOB. Everything around the code pane — the path
/// header, the tabs, the tree — is identical across two blobs at the same
/// path, so a rewritten blob that paints identical pixels means the
/// `forge_code` surface renders nothing at all for the file it says it has
/// open. The allocation probes cannot catch that: a subtree that draws
/// nothing passes any ceiling trivially.
#[test]
fn the_forge_code_pane_draws_the_loaded_blob() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(probe_forge_code_content)
        .expect("the forge content probe thread spawns")
        .join()
        .expect("the forge content probe thread finishes");
}

// The blob content is `ForgeCodeBrowser` component state, reached through the
// generated seam (ducktape-ui#696) rather than the app's update loop. Four
// things must hold: clicking the file row — the reader's real control — moves
// the component's local state and repaints the pane, a landed blob repaints it
// out of its loading plate, a REWRITTEN blob repaints it again, and the pane
// survives an event walk.
fn probe_forge_code_content() {
    let (mut app, console, scope) = console_in_forge_tree(vec![backend::TreeEntry {
        name: "probe.rs".into(),
        path: "probe.rs".into(),
        kind: "file".into(),
    }]);
    let mut renderer = headless_renderer();
    let (cache, first) = drawn_frame(
        &mut app,
        console,
        &mut renderer,
        user_interface::Cache::default(),
    );
    let (mut cache, control) = drawn_frame(&mut app, console, &mut renderer, cache);
    assert!(
        first == control,
        "an unchanged frame must draw identical pixels — without this control \
         the content assertions below prove nothing"
    );

    // The reader's real control still moves it: walk click points down the
    // tree column until one produces a message, replay it, and the pane
    // must repaint out of its empty plate. App-state pins prove the hit was
    // the component's file row and not an app control beside it.
    // Walk click points down the tree column until one visibly moves the
    // reader: an inert hit on the way down (the already-selected Code tab
    // sets a tab that is already set) delivers a message that repaints
    // nothing, and only the file row's local open_file does.
    let mut clipboard = clipboard::Null;
    let mut picked = Vec::new();
    for step in 0..40 {
        let position = Point::new(150.0, 90.0 + step as f32 * 10.0);
        let cursor = mouse::Cursor::Available(position);
        let mut queued: Vec<__DucktapeMessage> = Vec::new();
        let mut ui = UserInterface::build(app.__view(console), WINDOW, cache, &mut renderer);
        let _ = ui.update(
            &[
                Event::Mouse(mouse::Event::CursorMoved { position }),
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            ],
            cursor,
            &mut renderer,
            &mut clipboard,
            &mut queued,
        );
        cache = ui.into_cache();
        let delivered = !queued.is_empty();
        for message in queued {
            let _ = app.__update(message);
        }
        if !delivered {
            continue;
        }
        assert_eq!(app.forge_repo, "probe", "a click must not leave the repo");
        assert_eq!(
            app.forge_tab,
            crate::ForgeTab::Code,
            "a click must not switch seats"
        );
        let (next_cache, frame) = drawn_frame(&mut app, console, &mut renderer, cache);
        cache = next_cache;
        if frame != control {
            picked = frame;
            break;
        }
    }
    assert!(
        !picked.is_empty(),
        "clicking a file row must repaint the reader — identical pixels mean \
         the component's local state moved nothing on screen"
    );

    // The blob lands through the seam — the same message the runtime would
    // deliver — and the syntect surface must draw it.
    seed_forge_blob(&mut app, &scope, forge_source());
    let (cache, loaded) = drawn_frame(&mut app, console, &mut renderer, cache);
    assert!(
        picked != loaded,
        "a loaded blob must repaint the reader out of its loading plate"
    );

    // A REWRITTEN blob must repaint the code pane — identical pixels mean
    // the reader is drawing nothing for the file it says it has open (the
    // memo-boundary regression class that shipped once already).
    seed_forge_blob(&mut app, &scope, "const REWRITTEN: bool = true;\n".into());
    let (cache, rewritten) = drawn_frame(&mut app, console, &mut renderer, cache);
    assert!(
        loaded != rewritten,
        "a rewritten blob must repaint the code pane — identical pixels mean \
         the reader is drawing nothing for the file it says it has open"
    );

    // The live app pumps real events between frames; a cached boundary that
    // loses its element to the event walk's tree diff evades a build+draw
    // probe. A scroll down and back over the pane must land on the pixels it
    // started from.
    let mut queued: Vec<__DucktapeMessage> = Vec::new();
    let position = Point::new(700.0, 300.0);
    let cursor = mouse::Cursor::Available(position);
    let mut ui = UserInterface::build(app.__view(console), WINDOW, cache, &mut renderer);
    let _ = ui.update(
        &[
            Event::Mouse(mouse::Event::CursorMoved { position }),
            Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Lines { x: 0.0, y: -3.0 },
            }),
            Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Lines { x: 0.0, y: 30.0 },
            }),
        ],
        cursor,
        &mut renderer,
        &mut clipboard,
        &mut queued,
    );
    let cache = ui.into_cache();
    let (_, after_events) = drawn_frame(&mut app, console, &mut renderer, cache);
    assert!(
        rewritten == after_events,
        "a scroll down and back over the pane must restore the frame — \
         different pixels mean the surface lost its content to the event diff"
    );
}

/// THE CODE LINES ARE DRAGGABLE. Every plain Ice `text` in the app selects by
/// drag (ducktape-ui wraps it in `selectable_text`); the reader's coloured
/// spans render outside Ice, so they must prove the same contract: a
/// press-drag across the pane paints a selection, Ctrl+C puts exactly the
/// dragged text on the clipboard — line break included, the drag crossed
/// one — and Escape gives the quiet pixels back.
#[test]
fn the_forge_code_lines_select_by_drag() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(probe_forge_code_selection)
        .expect("the forge selection probe thread spawns")
        .join()
        .expect("the forge selection probe thread finishes");
}

#[derive(Default)]
struct RecordingClipboard(Option<String>);

impl iced::advanced::Clipboard for RecordingClipboard {
    fn read(&self, _kind: clipboard::Kind) -> Option<String> {
        self.0.clone()
    }

    fn write(&mut self, _kind: clipboard::Kind, contents: String) {
        self.0 = Some(contents);
    }
}

/// One event walk over the built tree, the way the runtime pumps input
/// between frames; queued messages are delivered so the walk is complete.
fn walk_events(
    app: &mut Ducktape,
    window: iced::window::Id,
    renderer: &mut iced::Renderer,
    cache: user_interface::Cache,
    clipboard: &mut dyn iced::advanced::Clipboard,
    events: &[Event],
    cursor: mouse::Cursor,
) -> user_interface::Cache {
    let mut queued: Vec<__DucktapeMessage> = Vec::new();
    let mut ui = UserInterface::build(app.__view(window), WINDOW, cache, renderer);
    let _ = ui.update(events, cursor, renderer, clipboard, &mut queued);
    let cache = ui.into_cache();
    for message in queued {
        let _ = app.__update(message);
    }
    cache
}

fn key_press(character: char, modifiers: iced::keyboard::Modifiers) -> Event {
    use iced::keyboard::{self, Key, key};
    let code = match character {
        'c' => key::Code::KeyC,
        _ => key::Code::Escape,
    };
    let key = match character {
        '\u{1b}' => Key::Named(key::Named::Escape),
        _ => Key::Character(character.to_string().into()),
    };
    Event::Keyboard(keyboard::Event::KeyPressed {
        key: key.clone(),
        modified_key: key,
        physical_key: key::Physical::Code(code),
        location: keyboard::Location::Standard,
        modifiers,
        text: None,
        repeat: false,
    })
}

fn probe_forge_code_selection() {
    let (mut app, console) = console_in_forge_code();
    let mut renderer = headless_renderer();
    let (cache, quiet) = drawn_frame(
        &mut app,
        console,
        &mut renderer,
        user_interface::Cache::default(),
    );
    let mut clipboard = RecordingClipboard::default();

    // Press on one code line and let go one row down — the pane sits right
    // of the tree column, and 300 → 340 spans two of its 20 px rows. One
    // walk per cursor position: a walk hit-tests every event against the
    // cursor it was given, not the event's own coordinates.
    let from = Point::new(700.0, 300.0);
    let to = Point::new(760.0, 340.0);
    let cache = walk_events(
        &mut app,
        console,
        &mut renderer,
        cache,
        &mut clipboard,
        &[
            Event::Mouse(mouse::Event::CursorMoved { position: from }),
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        ],
        mouse::Cursor::Available(from),
    );
    let cursor = mouse::Cursor::Available(to);
    let cache = walk_events(
        &mut app,
        console,
        &mut renderer,
        cache,
        &mut clipboard,
        &[
            Event::Mouse(mouse::Event::CursorMoved { position: to }),
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        ],
        cursor,
    );
    let (cache, selected) = drawn_frame(&mut app, console, &mut renderer, cache);
    assert!(
        selected != quiet,
        "a drag across the code lines must paint a selection"
    );

    let cache = walk_events(
        &mut app,
        console,
        &mut renderer,
        cache,
        &mut clipboard,
        &[key_press('c', iced::keyboard::Modifiers::CTRL)],
        cursor,
    );
    let copied = clipboard.0.take().expect("Ctrl+C copies the dragged lines");
    assert!(
        copied.contains('\n'),
        "the drag crossed a row, so the copy carries the line break: {copied:?}"
    );
    assert!(
        forge_source().contains(&copied),
        "the copy is the blob's own text: {copied:?}"
    );

    let cache = walk_events(
        &mut app,
        console,
        &mut renderer,
        cache,
        &mut clipboard,
        &[key_press('\u{1b}', iced::keyboard::Modifiers::empty())],
        cursor,
    );
    let (_, released) = drawn_frame(&mut app, console, &mut renderer, cache);
    assert!(released == quiet, "Escape must give the quiet pixels back");
}

/// A MARKDOWN BLOB'S TEXT IS DRAGGABLE TOO, AND ACROSS ITS BLOCKS. A README
/// renders as a document through `agent_markdown` (iced's markdown widget,
/// outside Ice), whose heading, paragraph and code plate are each their own
/// widget — so the drag has to prove more than the one-run code plate does:
/// a drag DOWN THE DOCUMENT paints a selection, Ctrl+C copies every block it
/// ran through and nothing else, Escape gives the quiet pixels back.
#[test]
fn the_forge_markdown_selects_by_drag() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(probe_forge_markdown_selection)
        .expect("the forge markdown selection probe thread spawns")
        .join()
        .expect("the forge markdown selection probe thread finishes");
}

fn probe_forge_markdown_selection() {
    let (mut app, console, scope) = console_in_forge_tree(vec![backend::TreeEntry {
        name: "README.md".into(),
        path: "README.md".into(),
        kind: "file".into(),
    }]);
    let readme = "# Probe\n\nThe quick brown fox jumps over the lazy dog, and keeps \
                  jumping until the paragraph wraps onto a second line of the pane.\n\n\
                  ```rust\nlet answer = 42;\nlet other = answer + 1;\n```\n";
    seed_forge_file(&mut app, &scope, "README.md", readme.into());
    let mut renderer = headless_renderer();
    let (mut cache, quiet) = drawn_frame(
        &mut app,
        console,
        &mut renderer,
        user_interface::Cache::default(),
    );
    let mut clipboard = RecordingClipboard::default();

    // Walk press points down the document column until a drag copies: the
    // header and spacing above the first paragraph are not the document's
    // business to pin here, a copy is. The first press that lands is the
    // topmost block, and every drag runs 250 px down from it — past the
    // paragraph, past the code plate, off the end of the document.
    let mut copied = None;
    for step in 0..30 {
        let y = 190.0 + step as f32 * 8.0;
        let from = Point::new(520.0, y);
        let to = Point::new(640.0, y + 250.0);
        cache = walk_events(
            &mut app,
            console,
            &mut renderer,
            cache,
            &mut clipboard,
            &[
                Event::Mouse(mouse::Event::CursorMoved { position: from }),
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            ],
            mouse::Cursor::Available(from),
        );
        cache = walk_events(
            &mut app,
            console,
            &mut renderer,
            cache,
            &mut clipboard,
            &[
                Event::Mouse(mouse::Event::CursorMoved { position: to }),
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
                key_press('c', iced::keyboard::Modifiers::CTRL),
            ],
            mouse::Cursor::Available(to),
        );
        if let Some(text) = clipboard.0.take() {
            copied = Some(text);
            break;
        }
    }
    let copied = copied.expect("a drag over the document copies its text");
    for line in copied.lines() {
        assert!(
            readme.contains(line),
            "the copy is the README's own words: {line:?} is not in it"
        );
    }
    // The press landed in a block above the code plate and the pointer left
    // the document below it, so everything between is one selection: the copy
    // runs from wherever in the paragraph it started, through the paragraph's
    // end, into the plate and off its last line. Where exactly the press
    // landed is the pane's business; CROSSING is this document's, and one
    // block's worth is the bug pinned here — a drag used to stop dead at the
    // block it started in.
    assert!(
        copied.contains("second line of the pane.") && copied.contains("let other = answer + 1;"),
        "the drag ran to the end of the document, so the copy crosses its \
         blocks: {copied:?}"
    );
    let (cache, selected) = drawn_frame(&mut app, console, &mut renderer, cache);
    assert!(
        selected != quiet,
        "a drag across the document must paint a selection"
    );

    let cache = walk_events(
        &mut app,
        console,
        &mut renderer,
        cache,
        &mut clipboard,
        &[key_press('\u{1b}', iced::keyboard::Modifiers::empty())],
        mouse::Cursor::Available(Point::new(640.0, 300.0)),
    );
    let (_, released) = drawn_frame(&mut app, console, &mut renderer, cache);
    assert!(released == quiet, "Escape must give the quiet pixels back");
}

fn probe() {
    let (mut app, console) = console_in_chat();
    let mut renderer = headless_renderer();
    let mut clipboard = clipboard::Null;
    let mut messages: Vec<__DucktapeMessage> = Vec::new();
    let mut cache = user_interface::Cache::default();

    for _ in 0..WARMUP_FRAMES {
        let ui = UserInterface::build(app.__view(console), WINDOW, cache, &mut renderer);
        cache = ui.into_cache();
    }

    let mut build = Phase::new("unchanged build+layout");
    let mut walk = Phase::new("cursor-move event walk");
    let mut draw = Phase::new("draw");
    let mut keystroke_frame = Phase::new("composer keystroke+rebuild");
    let mut row_edit = Phase::new("one-row edit rebuild");
    let mut screen_switch = Phase::new("screen switch (pages->chat)");
    let composer = composer_scope(&app);

    for frame in 0..FRAMES {
        let mut ui = build
            .sample(|| UserInterface::build(app.__view(console), WINDOW, cache, &mut renderer));

        let position = Point::new(700.0, 120.0 + (frame % 600) as f32);
        let cursor = mouse::Cursor::Available(position);
        walk.sample(|| {
            ui.update(
                &[Event::Mouse(mouse::Event::CursorMoved { position })],
                cursor,
                &mut renderer,
                &mut clipboard,
                &mut messages,
            )
        });
        messages.clear();

        draw.sample(|| {
            ui.draw(
                &mut renderer,
                &Theme::Dark,
                &renderer::Style::default(),
                cursor,
            );
        });
        cache = ui.into_cache();

        // ONE KEYSTROKE, WHOLE FRAME: the composer message through the real
        // handler, then the rebuild every message forces (iced 0.14 has no
        // dirty check). This pair IS what a user pays per typed character —
        // and it is the whole bill only because the app's global key
        // subscription carries `status=ignored`. Unfiltered, it published a
        // SECOND message for the very key the editor had just consumed, in the
        // next loop turn and so in its own batch, and this number was half the
        // truth. `no_keyboard_subscription_charges_a_captured_key_to_a_bare_composer`
        // in `tests.rs` is what keeps it honest; the ceiling cannot see it.
        let ui = keystroke_frame.sample(|| {
            let _ = app.__update(keystroke(&composer));
            UserInterface::build(app.__view(console), WINDOW, cache, &mut renderer)
        });
        cache = ui.into_cache();

        // One row's dependency changes — an edit landing on the stream.
        app.messages[frame % ROWS as usize].rev += 1;
        app.messages_revision += 1;
        let ui = row_edit
            .sample(|| UserInterface::build(app.__view(console), WINDOW, cache, &mut renderer));
        cache = ui.into_cache();

        // Leaving chat unmounts the stream (parking every lazy row) and
        // trims the scrollback; coming back is the cold return.
        let _ = app.__update(__DucktapeMessage::SelectShellTab(ShellTab::Pages));
        let ui = UserInterface::build(app.__view(console), WINDOW, cache, &mut renderer);
        cache = ui.into_cache();
        let _ = app.__update(__DucktapeMessage::SelectShellTab(ShellTab::Chat));
        cache = screen_switch
            .sample(|| UserInterface::build(app.__view(console), WINDOW, cache, &mut renderer))
            .into_cache();
    }

    eprintln!(
        "chat frame probe: {} rows, {} channels, {}x{}",
        app.messages.len(),
        CHANNELS,
        WINDOW.width,
        WINDOW.height
    );
    build.report();
    walk.report();
    draw.report();
    keystroke_frame.report();
    row_edit.report();
    screen_switch.report();

    let per_keystroke = keystroke_frame.median_allocations();
    assert!(
        per_keystroke < KEYSTROKE_ALLOCATION_CEILING,
        "one chat keystroke rebuilt the frame in {per_keystroke} allocations, over the \
         {KEYSTROKE_ALLOCATION_CEILING} ceiling. Something now does per-row work in a \
         frame: a view expression calling an extern that takes a list (every list \
         argument is deep-cloned, the ABI is by value), a per-row scan of a mirrored \
         list, or a lost `virtual-row=`/`lazy` on the stream. Find it before raising \
         this number — the ceiling is the gate, not the target."
    );
}

/// ALLOCATIONS PER ROOM SWITCH, REDUCER ONLY — the click's own cost, before any
/// view work.
///
/// The transition retains only tiny per-room drafts, clears the rich window,
/// and starts one root-window read. A regression that passes the old timeline
/// through a by-value extern will jump far beyond this budget.
const CHANNEL_SWITCH_REDUCER_ALLOCATION_CEILING: u64 = 1_500;
const CHANNEL_SWITCH_FRAME_ALLOCATION_CEILING: u64 = 12_000;

#[test]
fn a_channel_switch_stays_under_its_allocation_ceiling() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(probe_channel_switch)
        .expect("the switch probe thread spawns")
        .join()
        .expect("the switch probe thread finishes");
}

#[test]
fn channel_switch_loading_frame_is_low_and_does_not_grow_with_loaded_history() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(probe_channel_switch_slope)
        .expect("the switch slope probe thread spawns")
        .join()
        .expect("the switch slope probe thread finishes");
}

fn probe_channel_switch_slope() {
    let samples: Vec<_> = BOUNDED_CHAT_SLOPE_ROWS
        .into_iter()
        .map(|rows| (rows, channel_switch_frame_allocations(rows)))
        .collect();
    eprintln!("empty-loading channel switch allocation slope: {samples:?}");
    for (rows, allocations) in &samples {
        assert!(
            *allocations < CHANNEL_SWITCH_FRAME_ALLOCATION_CEILING,
            "the {rows}-row switch frame cost {allocations} allocations, over the \
             {CHANNEL_SWITCH_FRAME_ALLOCATION_CEILING} absolute ceiling"
        );
    }
    assert_bounded_allocation_span(
        "empty-loading channel switch+rebuild",
        &samples,
        CHANNEL_SWITCH_SLOPE_HEADROOM,
    );
}

fn channel_switch_frame_allocations(rows: i64) -> u64 {
    let (mut app, console) = console_in_chat_with_rows(rows);
    let mut renderer = headless_renderer();
    let mut cache = warm_settled(
        "the channel switch slope probe",
        &mut app,
        console,
        WINDOW,
        &mut renderer,
        user_interface::Cache::default(),
    );
    let mut switches = Phase::new("empty-loading switch+rebuild slope");
    for frame in 0..SLOPE_FRAMES {
        let target = match frame % 2 == 0 {
            true => "channel-1",
            false => "channel-2",
        };
        cache = switches
            .sample(|| {
                let _ = app.__update(__DucktapeMessage::ChooseChannel(target.into()));
                UserInterface::build(app.__view(console), WINDOW, cache, &mut renderer)
            })
            .into_cache();
        assert!(app.messages.is_empty(), "the old rich window is gone");
        assert!(app.loading, "the selected root window is still in flight");

        // Settle and render the selected room outside the measured phase so
        // every sample starts from a populated timeline and pays the real
        // full-window -> loading-state diff.
        let landed = probe_chat_data_with_rows(target, app.chat_generation, rows);
        let _ = app.__update(__DucktapeMessage::ChatUpdated(landed));
        cache =
            UserInterface::build(app.__view(console), WINDOW, cache, &mut renderer).into_cache();
    }
    switches.median_allocations()
}

fn probe_channel_switch() {
    let (mut app, _) = console_in_chat_with_rows(ROWS);
    let mut switch = Phase::new("channel switch (reducer)");

    for frame in 0..FRAMES {
        let target = match frame % 2 == 0 {
            true => "channel-1",
            false => "channel-2",
        };
        switch.sample(|| {
            let _ = app.__update(__DucktapeMessage::ChooseChannel(target.into()));
        });
        assert!(app.messages.is_empty(), "the old rich window is gone");
        assert!(app.loading, "the selected root window is still in flight");

        let landed = probe_chat_data_with_rows(target, app.chat_generation, ROWS);
        let _ = app.__update(__DucktapeMessage::ChatUpdated(landed));
    }
    switch.report();

    let per_switch = switch.median_allocations();
    assert!(
        per_switch < CHANNEL_SWITCH_REDUCER_ALLOCATION_CEILING,
        "one room switch cost {per_switch} allocations, over the \
         {CHANNEL_SWITCH_REDUCER_ALLOCATION_CEILING} ceiling. The switch should clear \
         the active rich window, retain only tiny draft stores, and launch one root read."
    );
}

/// A press on the pane beside an open message menu dismisses it — the
/// backdrop's `dismiss`, the one exit a pointer has. The app's codegen wraps
/// an overlay's LAYER in a press swallower (a press on a menu row's padding
/// must not fall through to the backdrop), so a fill-sized layer covered the
/// backdrop end to end: every press on the pane died in the swallower and
/// Esc was the menu's only exit. Driven through the real event path — the
/// float overlay, the swallower, the backdrop — not the reducer.
#[test]
fn a_press_beside_the_message_menu_dismisses_it() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(probe_message_menu_dismiss)
        .expect("the menu dismiss probe thread spawns")
        .join()
        .expect("the menu dismiss probe thread finishes");
}

fn probe_message_menu_dismiss() {
    let (mut app, console) = console_in_chat();
    let _ = app.__update(__DucktapeMessage::OpenMessageActions(
        ROWS,
        "body".into(),
        1,
    ));
    assert_eq!(app.message_action, MessageAction::More, "the menu is open");
    assert_eq!(app.selected_message_seq, ROWS);

    let mut renderer = headless_renderer();
    let cache = warm_settled(
        "the menu dismiss probe",
        &mut app,
        console,
        WINDOW,
        &mut renderer,
        user_interface::Cache::default(),
    );
    // Mid-pane, well left of the 200px menu that hangs off the right edge.
    let position = Point::new(520.0, 450.0);
    let cursor = mouse::Cursor::Available(position);
    let mut clipboard = clipboard::Null;
    let mut queued: Vec<__DucktapeMessage> = Vec::new();
    let mut ui = UserInterface::build(app.__view(console), WINDOW, cache, &mut renderer);
    let _ = ui.update(
        &[
            Event::Mouse(mouse::Event::CursorMoved { position }),
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        ],
        cursor,
        &mut renderer,
        &mut clipboard,
        &mut queued,
    );
    drop(ui);
    for message in queued {
        let _ = app.__update(message);
    }
    assert_eq!(
        app.message_action,
        MessageAction::Toolbar,
        "a press beside the menu must reach the backdrop's dismiss"
    );
    assert_eq!(
        app.selected_message_seq, 0,
        "the selection clears with the menu"
    );
}
