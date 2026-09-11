//! Evaluating the active-page timer must not scan, clone, or reshape the
//! document on every unrelated update. Canonical reconciliation and the dirty
//! check happen in the timer handler, including when the app mirror is clean.
//! Allocations are asserted; wall-clock is diagnostic only.

use std::time::Instant;

use super::*;

const LINES: usize = 2_000;
const SAMPLES: usize = 5;
/// Keep the existing generous ceiling to catch accidental document shaping.
const GATE_ALLOCATION_CEILING: u64 = 2 * LINES as u64;

#[test]
fn page_autosave_gate_does_not_scan_or_reshape_the_open_document() {
    let mut app = reading_alpha();
    let lines: Vec<String> = std::iter::once("Alpha".to_owned())
        .chain((0..LINES).map(|index| {
            format!(
                "paragraph {index}: a page long enough that re-shaping it per update is a freeze"
            )
        }))
        .collect();
    let text = lines.join("\n");
    app.page_text = text.to_string();
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
