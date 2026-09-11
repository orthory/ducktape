//! THE APP'S FRAME-COST PROBES.
//!
//! Ported from ducktape-ui's `crates/ui-lang-runtime/tests/frame_probe.rs`,
//! but driving THIS app's generated views over state installed through the
//! real reducers rather than screen-shaped stand-ins. The app is a bin crate,
//! so `app/tests/` cannot see `Ducktape`; this lives in the crate as a
//! `#[cfg(test)]` module instead.
//!
//! Each screen state gates its warmed build+layout allocation median.
//! Wall-clock is printed, never asserted — it measures the box, not the code,
//! and an absolute microsecond budget only flakes. Allocation counts do not
//! care how busy the machine is, and the large fixtures make per-row list
//! clones and lost virtualization visible in the medians.
//!
//! THE CHAT PROBES LEFT WITH THE CHAT SCREEN. Its timeline, rail, composer
//! keystroke and room switch are the `chat` view's frames now, measured
//! against the view's own tree rather than this app's.

use std::alloc::{GlobalAlloc, Layout, System};
use std::borrow::Cow;
use std::cell::Cell;
use std::sync::Once;

use iced::advanced::{clipboard, mouse};
use iced::{Event, Size};
use iced_test::runtime::user_interface::{self, UserInterface};

use super::backend;
use super::{__DucktapeMessage, Ducktape, ShellTab};

/// The head seq a probe channel wears — a workspace with real scrollback
/// behind each room, so the sidebar's per-row work is visible.
const ROWS: i64 = 256;
const WINDOW: Size = Size::new(1440.0, 900.0);
const HUDDLE_WINDOW: Size = Size::new(320.0, 460.0);
const PAGE_ROWS: usize = 128;
const HUDDLE_ROWS: usize = 32;
/// Enough passes to fill the lazy parking lot and settle the text caches.
const WARMUP_FRAMES: usize = 4;
pub(crate) const FRAMES: usize = 12;

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
    // 24,063 measured 2026-08-23 at ducktape-ui af41cc28 with the screen's
    // externs borrowing their list and string arguments
    // (`subpage_blocks`, `comment_scope_label`, `comment_compose_hint`, and
    // the `page_document` mount's `blocks`/`hits`): 26,542 with the same
    // externs cloning them per frame.
    ScreenProbe {
        label: "pages comments build+layout",
        size: WINDOW,
        fixture: console_in_page_comments,
        allocation_ceiling: 30_000,
    },
    // 4,947 vs 7,059 for restoring per-row peer lookup.
    ScreenProbe {
        label: "huddle build+layout",
        size: HUDDLE_WINDOW,
        fixture: console_in_huddle,
        allocation_ceiling: 6_000,
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

pub(crate) struct Phase {
    label: &'static str,
    allocations: Vec<u64>,
    elapsed_us: Vec<u128>,
}

impl Phase {
    pub(crate) fn new(label: &'static str) -> Self {
        Self {
            label,
            allocations: Vec::with_capacity(FRAMES),
            elapsed_us: Vec::with_capacity(FRAMES),
        }
    }

    pub(crate) fn sample<T>(&mut self, work: impl FnOnce() -> T) -> T {
        let started = std::time::Instant::now();
        let before = allocations();
        let value = work();
        let spent = allocations() - before;
        let elapsed = started.elapsed().as_micros();
        self.allocations.push(spent);
        self.elapsed_us.push(elapsed);
        value
    }

    pub(crate) fn median_allocations(&self) -> u64 {
        let mut sorted = self.allocations.clone();
        sorted.sort_unstable();
        sorted[sorted.len() / 2]
    }

    fn median_us(&self) -> u128 {
        let mut sorted = self.elapsed_us.clone();
        sorted.sort_unstable();
        sorted[sorted.len() / 2]
    }

    pub(crate) fn report(&self) {
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
            threads: (0..PAGE_ROWS)
                .map(|index| backend::PageCommentThread {
                    id: format!("thread-{index}"),
                    target: format!("block-{index}"),
                    author: format!("reviewer-{}", index % 7),
                    meta: "1 comment".into(),
                    resolved: false,
                    comment_count: 1,
                    comments: vec![backend::PageComment {
                        id: format!("comment-{index}"),
                        ordinal: 1,
                        author: format!("reviewer-{}", index % 7),
                        meta: "#1".into(),
                        text: format!("A note on block {index}."),
                    }],
                })
                .collect(),
            total: PAGE_ROWS as i64,
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
        active_channel: "channel-0".into(),
        active_channel_name: "channel-0".into(),
        active_channel_archived: false,
        active_channel_members_only: false,
        huddle_roster: (0..HUDDLE_ROWS).map(probe_huddle_participant).collect(),
        channel_members: Vec::new(),
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

pub(crate) fn headless_renderer() -> iced::Renderer {
    static LOAD_FONTS: Once = Once::new();
    LOAD_FONTS.call_once(|| {
        let mut fonts = iced::advanced::graphics::text::font_system()
            .write()
            .expect("the shared font system lock");
        fonts.load_font(Cow::Borrowed(include_bytes!(
            "../../crates/views/support/design/assets/fonts/Geist[wght].ttf"
        )));
        fonts.load_font(Cow::Borrowed(include_bytes!(
            "../../crates/views/support/design/assets/fonts/GeistMono[wght].ttf"
        )));
        fonts.load_font(Cow::Borrowed(include_bytes!(
            "../../crates/views/support/design/assets/fonts/NotoColorEmoji.ttf"
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

// ---------------------------------------------------------------------------
// The probe
// ---------------------------------------------------------------------------

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
    for variant in ["retry", "tip", "ready", "chat", "bell", "pages", "plane", "resync"] {
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

#[test]
fn large_screens_stay_under_their_allocation_ceilings() {
    let _turn = crate::module_view::tests::blocking_connection_turn();
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(probe_large_screens)
        .expect("the screen probe thread spawns")
        .join()
        .expect("the screen probe thread finishes");
}

fn probe_large_screens() {
    eprintln!("large screen frame probes: {PAGE_ROWS} page rows, {HUDDLE_ROWS} huddle rows");
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

