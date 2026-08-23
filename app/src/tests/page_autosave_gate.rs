//! THE PAGE AUTOSAVE GATE'S COST AT REST.
//!
//! iced re-evaluates `subscription()` after EVERY update batch, so the
//! `when` gate on `page_autosave_tick` runs on every keystroke, wheel tick,
//! `wall_tick` and live push anywhere in the app while a page is open. The
//! gate compares the buffer's text to the saved baseline; reading that text
//! must be a borrow of the buffer, never a clone of it — iced's
//! `Content::clone` is `with_text(&self.text())`, a full cosmic-text re-shape
//! of the whole document under the global font-system write lock.
//!
//! Allocations are asserted; wall-clock is printed only (it measures the box).

use std::time::Instant;

use super::*;

const LINES: usize = 2_000;
const SAMPLES: usize = 5;
/// Measured 2026-08-23 at 2,000 lines, debug build: **2,043** allocations and
/// ~190 us with the gate reading `editor_text(page_editor)` — one allocation a
/// line is iced's own `Content::text()` (each `Line` is `Cow::Owned` across the
/// `RefCell` borrow), the floor for reading the buffer at all. The pre-fix
/// `page_text(page_editor)` extern took the editor BY VALUE, so the gate
/// cloned the `Content` first: **104,084** allocations and ~400 ms per
/// evaluation — on every update batch while a page was open. Two allocations
/// a line leaves room for the join's growth and the batch; the clone sits
/// 26x over this ceiling and 51x over the measured floor.
const GATE_ALLOCATION_CEILING: u64 = 2 * LINES as u64;

#[test]
fn page_autosave_gate_borrows_the_open_page_instead_of_reshaping_it() {
    let mut app = reading_alpha();
    let lines: Vec<String> = std::iter::once("Alpha".to_owned())
        .chain((0..LINES).map(|index| {
            format!(
                "paragraph {index}: a page long enough that re-shaping it per update is a freeze"
            )
        }))
        .collect();
    let text = lines.join("\n");
    app.page_editor = compose(&text);
    // Clean: the at-rest state every unrelated update pays the gate in.
    app.page_saved_text = text;
    assert!(app.connected);
    assert!(!app.active_page.is_empty());

    // One warm pass settles first-touch caches before sampling.
    drop(app.__subscription());
    let mut allocations = Vec::with_capacity(SAMPLES);
    let mut elapsed_us = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let before = crate::frame_probe::allocations();
        let started = Instant::now();
        let subscription = app.__subscription();
        elapsed_us.push(started.elapsed().as_micros());
        allocations.push(crate::frame_probe::allocations() - before);
        drop(subscription);
    }
    allocations.sort_unstable();
    elapsed_us.sort_unstable();
    let median_allocations = allocations[SAMPLES / 2];
    let median_us = elapsed_us[SAMPLES / 2];
    eprintln!(
        "page autosave gate ({LINES} lines)   allocs(p50)={median_allocations:>7}  {median_us:>6}us"
    );
    assert!(
        median_allocations <= GATE_ALLOCATION_CEILING,
        "evaluating the subscription gate cost {median_allocations} allocations \
         (ceiling {GATE_ALLOCATION_CEILING}): the gate is cloning the page editor"
    );
}
