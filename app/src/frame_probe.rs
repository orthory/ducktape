//! THE CHAT SCREEN'S FRAME-COST GATE.
//!
//! Ported from ducktape-ui's `crates/ui-lang-runtime/tests/frame_probe.rs`,
//! but driving THIS app's real `ChatScreen` — the generated view, the real
//! 47 props, over state installed through the real reducers — rather than a
//! chat-shaped stand-in. The app is a bin crate, so `app/tests/` cannot see
//! `Ducktape`; this lives in the crate as a `#[cfg(test)]` module instead.
//!
//! It prints six phases (build, event walk, draw, keystroke rebuild, one-row
//! edit, screen switch) and asserts EXACTLY ONE number: allocations per
//! simulated keystroke. Wall-clock is printed, never asserted — it measures
//! the box, not the code, and an absolute microsecond budget only flakes.
//! Allocation counts do not care how busy the machine is, and every render
//! finding the QA pass raised (a list-taking extern cloned per frame, a
//! per-row read-cursor clone, an un-virtualized column laying out every row)
//! lands in this one number.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

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
/// Enough passes to fill the lazy parking lot and settle the text caches.
const WARMUP_FRAMES: usize = 4;
const FRAMES: usize = 12;

/// ALLOCATIONS PER KEYSTROKE, AND NOTHING ELSE IS ASSERTED.
///
/// Measured on `dev` after the QA sweep landed: **16 657**, and bit-identical
/// across repeated runs — this counts allocations, not time, so there is no
/// run-to-run noise to absorb. The ceiling is that median plus ~32%: deleting
/// the stream's `virtual-row=` alone takes it to 27 622, so the headroom is
/// wide enough never to flake and still far below the first regression it is
/// here to catch.
const KEYSTROKE_ALLOCATION_CEILING: u64 = 22_000;

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

/// A console sitting in chat, on a channel with a full page of scrollback,
/// installed through `chat_updated` — the canonical install — so every
/// mirrored field (`rooms`, `unread_channel_ids`, `post_refusal`,
/// `unread_marker_seq`, …) holds what it would hold in front of a user.
fn console_in_chat() -> (Ducktape, iced::window::Id) {
    let (mut app, _) = Ducktape::__boot();
    let console = iced::window::Id::unique();
    app.console_win = Some(console);
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.settings_user_key = "probe-user".into();

    let next = backend::ChatData {
        generation: app.chat_generation,
        channels: (0..CHANNELS).map(probe_channel).collect(),
        messages: (1..=ROWS).map(probe_message).collect(),
        active_channel: "channel-0".into(),
        active_channel_name: "channel-0".into(),
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
    };
    let _ = app.__update(__DucktapeMessage::ChatUpdated(next));
    assert_eq!(
        app.messages.len(),
        ROWS as usize,
        "the probe drives a full page of scrollback, not an empty room"
    );
    (app, console)
}

fn headless_renderer() -> iced::Renderer {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a current-thread runtime to block the headless renderer on")
        .block_on(<iced::Renderer as iced::advanced::renderer::Headless>::new(
            iced::Font::DEFAULT,
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
    let mut keystroke_rebuild = Phase::new("composer keystroke rebuild");
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
        // dirty check). This pair IS what a user pays per typed character.
        let _ = app.__update(keystroke());
        let ui = keystroke_rebuild
            .sample(|| UserInterface::build(app.__view(console), WINDOW, cache, &mut renderer));
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
    keystroke_rebuild.report();
    row_edit.report();
    screen_switch.report();

    let per_keystroke = keystroke_rebuild.median_allocations();
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
