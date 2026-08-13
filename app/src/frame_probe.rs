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
use super::{__DucktapeMessage, Ducktape};

/// One synthetic channel's worth of scrollback — `CHAT_VIEW_PAGE_LIMIT`, the
/// page the timeline walk asks for, so the probe measures the widest window a
/// single load can put on screen.
const ROWS: i64 = 256;
/// A workspace big enough that the sidebar's per-row work is visible.
const CHANNELS: i64 = 24;
const WINDOW: Size = Size::new(1440.0, 900.0);
const HUDDLE_WINDOW: Size = Size::new(320.0, 460.0);
const PAGE_ROWS: usize = 128;
/// Two thread pages of replies in the open rail (root + 127 replies).
const THREAD_ROWS: i64 = 128;
const HUDDLE_ROWS: usize = 32;
const LONG_LIST_ROWS: usize = 2_048;
const FILE_ROWS: usize = 256;
const DISCUSSION_ROWS: usize = 256;
/// Enough passes to fill the lazy parking lot and settle the text caches.
const WARMUP_FRAMES: usize = 4;
const FRAMES: usize = 12;

/// ALLOCATIONS PER KEYSTROKE, AND NOTHING ELSE IS ASSERTED.
///
/// Measured on `dev` after the QA sweep landed: about **16 700** allocations;
/// the ducktape-ui a099fa6b pin's borrow-aware `for` rows brought the same
/// fixture to **15 982**, and the 690b84d9 keyed lazy (`by message.seq,
/// message.render_rev` over by-reference keyed rows) to **11 377** — each
/// ceiling move locks the win. The count is stable inside one process but can
/// move slightly with global font/cache initialization, so the ceiling leaves
/// broad headroom. Deleting the stream's `virtual-row=` alone takes it above
/// 27 000, still well beyond the budget.
const KEYSTROKE_ALLOCATION_CEILING: u64 = 15_000;

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
    // 142,166 vs 150,202 for removing source-row virtualization.
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
    // 60,437 vs 61,979 for restoring the selected-entry scan; removing the
    // directory-row virtualization reaches 61,993.
    ScreenProbe {
        label: "files build+layout",
        size: WINDOW,
        fixture: console_in_files,
        allocation_ceiling: 61_200,
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

fn allocations() -> u64 {
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

fn probe_channel(index: i64) -> backend::ChatChannel {
    backend::ChatChannel {
        id: format!("channel-{index}"),
        name: format!("channel-{index}"),
        archived: false,
        members_only: false,
        huddle_count: 0,
        head_seq: ROWS,
    }
}

/// One window loader's answer: a full page of scrollback for one room, against
/// the whole channel list. Extracted so the switch probe can land the same
/// window in several rooms; the rail fixture overrides the two thread fields.
fn probe_chat_data(channel: &str, generation: i64) -> backend::ChatData {
    backend::ChatData {
        generation,
        channels: (0..CHANNELS).map(probe_channel).collect(),
        messages: (1..=ROWS).map(probe_message).collect(),
        active_channel: channel.into(),
        active_channel_name: channel.into(),
        active_channel_archived: false,
        active_channel_members_only: false,
        active_channel_huddle_count: 0,
        huddle_roster: Vec::new(),
        channel_members: Vec::new(),
        selected_message_seq: 0,
        selected_message_rev: 0,
        selected_message_body: String::new(),
        active_thread_seq: 0,
        thread_target_seq: 0,
        thread_messages: Vec::new(),
        thread_next_reply_offset: 0,
        thread_has_more: false,
    }
}

/// A console sitting in chat, on a channel with a full page of scrollback,
/// installed through `chat_updated` — the canonical install — so every
/// mirrored field (`rooms`, `dm_rows`, `post_refusal`,
/// `unread_marker_seq`, …) holds what it would hold in front of a user.
fn console_in_chat() -> (Ducktape, iced::window::Id) {
    console_in_chat_with_thread(Vec::new(), 0)
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
    let (mut app, _) = Ducktape::__boot();
    let console = iced::window::Id::unique();
    app.console_win = Some(console);
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.settings_user_key = "probe-user".into();

    let next = backend::ChatData {
        active_thread_seq,
        thread_messages,
        ..probe_chat_data("channel-0", app.chat_generation)
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
    assert_eq!(
        app.messages.len(),
        ROWS as usize,
        "the probe drives a full page of scrollback, not an empty room"
    );
    assert_eq!(app.dm_rows.len(), 4, "the probe mounts DIRECT rows too");
    (app, console)
}

fn console_on(tab: &str) -> (Ducktape, iced::window::Id) {
    let (mut app, _) = Ducktape::__boot();
    let console = iced::window::Id::unique();
    app.console_win = Some(console);
    app.connected = true;
    app.connected_rpc = "http://node".into();
    let _ = app.__update(__DucktapeMessage::SelectShellTab(tab.into()));
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
    let (mut app, console) = console_on("pages");
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
        active_channel: "channel-0".into(),
        active_channel_name: "channel-0".into(),
        active_channel_archived: false,
        active_channel_members_only: false,
        active_channel_huddle_count: HUDDLE_ROWS as i64,
        huddle_roster: (0..HUDDLE_ROWS).map(probe_huddle_participant).collect(),
        channel_members: Vec::new(),
        selected_message_seq: 0,
        selected_message_rev: 0,
        selected_message_body: String::new(),
        active_thread_seq: 0,
        thread_target_seq: 0,
        thread_messages: Vec::new(),
        thread_next_reply_offset: 0,
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

fn forge_source() -> String {
    (0..LONG_LIST_ROWS)
        .map(|index| format!("let line_{index:04} = {index};\n"))
        .collect()
}

fn console_in_forge_code() -> (Ducktape, iced::window::Id) {
    let (mut app, console) = console_on("forge");
    let _ = app.__update(__DucktapeMessage::ForgeOpenRepo("probe".into()));
    let _ = app.__update(__DucktapeMessage::ForgeRepoLoaded(backend::ForgeRepoData {
        generation: app.forge_generation,
        repo: "probe".into(),
        branches: vec!["dev".into()],
        items: Vec::new(),
    }));
    let _ = app.__update(__DucktapeMessage::ForgeTreeLoaded(backend::ForgeTreeData {
        generation: app.forge_code_generation,
        repo: "probe".into(),
        rev: "1111111111111111111111111111111111111111".into(),
        path: String::new(),
        born: true,
        entries: vec![backend::TreeEntry {
            name: "probe.rs".into(),
            path: "probe.rs".into(),
            kind: "file".into(),
        }],
        truncated: false,
    }));
    let _ = app.__update(__DucktapeMessage::ForgeOpenFile("probe.rs".into()));
    let source = forge_source();
    assert!(source.len() < 64 * 1024);
    let _ = app.__update(__DucktapeMessage::ForgeBlobLoaded(backend::BlobView {
        generation: app.forge_code_generation,
        repo: "probe".into(),
        rev: "1111111111111111111111111111111111111111".into(),
        path: "probe.rs".into(),
        text: source,
        truncated: false,
        binary: false,
        lines: LONG_LIST_ROWS as i64,
    }));
    assert_eq!(app.forge_file_text.lines().count(), LONG_LIST_ROWS);
    assert_eq!(app.forge_file_path, "probe.rs");
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
    let (mut app, console) = console_on("forge");
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
            generation: app.forge_discussion_generation,
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
    let (mut app, console) = console_on("files");
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
    }));
    assert_eq!(app.fs_entries.len(), FILE_ROWS);
    assert_eq!(app.fs_preview_path, selected);
    assert_eq!(app.fs_preview_entry.path, app.fs_preview_path);
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

fn keystroke() -> __DucktapeMessage {
    __DucktapeMessage::ChatComposerEvent(super::editor::ComposerEvent::Apply(
        super::editor::RichAction::Edit(iced::widget::text_editor::Action::Edit(
            iced::widget::text_editor::Edit::Insert('x'),
        )),
    ))
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
         {FILE_ROWS} file rows"
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
    // in the quiet arm (nothing selected, nothing flashing), so the repaint
    // must come through the keyed lazy's (seq, render_rev) move.
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: "chat".into(),
        chat: backend::ChatDelta {
            kind: "reaction".into(),
            channel_id: "channel-0".into(),
            seq: ROWS,
            emoji: "👍".into(),
            added: true,
            reactor: "user:repaint-probe".into(),
            ..backend::ChatDelta::default()
        },
        ..backend::LiveUpdate::default()
    }));
    let (cache, reacted) = drawn_frame(&mut app, console, &mut renderer, cache);
    assert!(
        control != reacted,
        "a reaction delta must repaint the visible row — `apply_reaction` \
         stopped moving `render_rev` if this frame is unchanged"
    );

    // An edit of the same row, one wire revision up.
    let body = "edited by the repaint probe";
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: "chat".into(),
        chat: backend::ChatDelta {
            kind: "edited".into(),
            channel_id: "channel-0".into(),
            seq: ROWS,
            message: backend::ChatMessage {
                rev: 2,
                body: body.into(),
                blocks: backend::paragraph_blocks(body),
                meta: format!("#{ROWS} · edited"),
                ..backend::ChatMessage::default()
            },
            ..backend::ChatDelta::default()
        },
        ..backend::LiveUpdate::default()
    }));
    let (_, edited) = drawn_frame(&mut app, console, &mut renderer, cache);
    assert!(
        reacted != edited,
        "an edit delta must repaint the visible row — `apply_edit_content` \
         stopped moving `render_rev` if this frame is unchanged"
    );
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
            let _ = app.__update(keystroke());
            UserInterface::build(app.__view(console), WINDOW, cache, &mut renderer)
        });
        cache = ui.into_cache();

        // One row's dependency changes — an edit landing on the stream.
        app.messages[frame % ROWS as usize].rev += 1;
        let ui = row_edit
            .sample(|| UserInterface::build(app.__view(console), WINDOW, cache, &mut renderer));
        cache = ui.into_cache();

        // Leaving chat unmounts the stream (parking every lazy row) and
        // trims the scrollback; coming back is the cold return.
        let _ = app.__update(__DucktapeMessage::SelectShellTab("pages".into()));
        let ui = UserInterface::build(app.__view(console), WINDOW, cache, &mut renderer);
        cache = ui.into_cache();
        let _ = app.__update(__DucktapeMessage::SelectShellTab("chat".into()));
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

/// A console alternating between rooms until the window cache is FULL —
/// `CHANNEL_WINDOW_CACHE` parked windows of `ROWS` rows each, which is what a
/// reader who moves between three rooms carries.
fn console_with_a_full_window_cache() -> Ducktape {
    let (mut app, _) = console_in_chat();
    for channel in ["channel-1", "channel-2", "channel-3"] {
        let _ = app.__update(__DucktapeMessage::ChooseChannel(channel.into()));
        let landed = probe_chat_data(channel, app.chat_generation);
        let _ = app.__update(__DucktapeMessage::ChatUpdated(landed));
    }
    assert_eq!(
        app.message_cache.len(),
        3,
        "the switch probe measures a FULL cache, not a warm one"
    );
    app
}

/// ALLOCATIONS PER ROOM SWITCH, REDUCER ONLY — the click's own cost, before any
/// view work.
///
/// The extern ABI passes every list BY VALUE, so each `message_cache` argument
/// is a deep copy of ALL three parked windows — thousands of `ChatMessage`
/// clones — and the switch used to spend three of them before a pixel moved:
/// the park, then one each for the rows and the member roll. Measured with this
/// fixture: **23 345** allocations with the restore folded into one
/// `cached_window` call, **30 265** with it split back into two calls asking
/// the same window for two of its fields. The ceiling sits between them.
const CHANNEL_SWITCH_ALLOCATION_CEILING: u64 = 27_000;

#[test]
fn a_channel_switch_stays_under_its_allocation_ceiling() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(probe_channel_switch)
        .expect("the switch probe thread spawns")
        .join()
        .expect("the switch probe thread finishes");
}

fn probe_channel_switch() {
    let mut app = console_with_a_full_window_cache();
    let mut switch = Phase::new("channel switch (reducer)");

    // A→B→A is the motion the cache exists for, and both rooms are parked, so
    // every sample is the cache-HIT path with a full cache behind it.
    for frame in 0..FRAMES {
        let target = match frame % 2 == 0 {
            true => "channel-1",
            false => "channel-2",
        };
        switch.sample(|| {
            let _ = app.__update(__DucktapeMessage::ChooseChannel(target.into()));
        });
        assert_eq!(
            app.messages.len(),
            ROWS as usize,
            "each sample must be a cache HIT — an empty room measures nothing"
        );
    }
    switch.report();

    let per_switch = switch.median_allocations();
    assert!(
        per_switch < CHANNEL_SWITCH_ALLOCATION_CEILING,
        "one room switch cost {per_switch} allocations, over the \
         {CHANNEL_SWITCH_ALLOCATION_CEILING} ceiling. Something walks the window cache \
         an extra time: every `message_cache` argument is a deep copy of every parked \
         window, so two calls asking the same window for two of its fields cost twice \
         what one does. Find it before raising this number."
    );
}
