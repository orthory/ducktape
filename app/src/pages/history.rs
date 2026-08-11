//! Bounded undo for the page document.
//!
//! The Ice `editor` state is a bare `Content` with no history of its own, so
//! the stacks live here, beside the edit layer that feeds them. Snapshots,
//! not deltas: a page is kilobytes, the budget caps the worst case, and a
//! snapshot restore can never desynchronize the way a mis-rebased delta can.
//!
//! One document, one history: the page surface mounts a single editor, and
//! [`reset`] clears both stacks whenever a page install replaces the buffer
//! (see `crate::backend::installed_page_editor`).
// ponytail: a global pair of stacks — per-window histories only matter if the
// console ever mounts two page editors at once.

use iced::widget::text_editor::{Content, Cursor};
use std::cell::RefCell;
use std::time::{Duration, Instant};

/// Keystrokes inside this window coalesce into one undo step — the reference
/// editor's own grouping cadence.
const COALESCE: Duration = Duration::from_millis(750);
const MAX_STEPS: usize = 200;
const MAX_BYTES: usize = 16 * 1024 * 1024;

struct Snapshot {
    text: String,
    cursor: Cursor,
}

#[derive(Default)]
struct History {
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    bytes: usize,
    group_open_until: Option<Instant>,
}

// Thread-local, not global: every caller lives on the iced update thread, and
// in tests each thread then owns its own stacks — no cross-test bleed.
thread_local! {
    static HISTORY: RefCell<History> = const { RefCell::new(History {
        undo: Vec::new(),
        redo: Vec::new(),
        bytes: 0,
        group_open_until: None,
    }) };
}

fn with_history<T>(f: impl FnOnce(&mut History) -> T) -> T {
    HISTORY.with(|history| f(&mut history.borrow_mut()))
}

/// A page install replaced the buffer — the stacks belong to the old page.
pub(crate) fn reset() {
    with_history(|history| *history = History::default());
}

/// Record the buffer as it stands BEFORE an edit. Inside the coalescing
/// window the existing snapshot already anchors the group, so `snapshot` is
/// only evaluated when a new group opens — the text allocation happens once
/// per group, not once per keystroke.
pub(crate) fn record(snapshot: impl FnOnce() -> (String, Cursor)) {
    with_history(|history| {
        let now = Instant::now();
        let group_open = history
            .group_open_until
            .is_some_and(|open_until| now < open_until);
        history.group_open_until = Some(now + COALESCE);
        if group_open {
            return;
        }
        let (text, cursor) = snapshot();
        history.bytes += text.len();
        history.undo.push(Snapshot { text, cursor });
        history.bytes -= history
            .redo
            .drain(..)
            .map(|snapshot| snapshot.text.len())
            .sum::<usize>();
        while history.undo.len() > MAX_STEPS || history.bytes > MAX_BYTES {
            let Some(oldest) = (!history.undo.is_empty()).then(|| history.undo.remove(0)) else {
                break;
            };
            history.bytes -= oldest.text.len();
        }
    })
}

/// Restore the newest undo snapshot, parking the current buffer on the redo
/// stack. `None` when there is nothing to undo.
pub(crate) fn undo(current: impl FnOnce() -> (String, Cursor)) -> Option<Content> {
    with_history(|history| {
        let snapshot = history.undo.pop()?;
        history.bytes -= snapshot.text.len();
        let (text, cursor) = current();
        history.bytes += text.len();
        history.redo.push(Snapshot { text, cursor });
        // The group is closed: the next keystroke opens a fresh undo step.
        history.group_open_until = None;
        Some(restore(snapshot))
    })
}

/// Inverse of [`undo`].
pub(crate) fn redo(current: impl FnOnce() -> (String, Cursor)) -> Option<Content> {
    with_history(|history| {
        let snapshot = history.redo.pop()?;
        history.bytes -= snapshot.text.len();
        let (text, cursor) = current();
        history.bytes += text.len();
        history.undo.push(Snapshot { text, cursor });
        history.group_open_until = None;
        Some(restore(snapshot))
    })
}

fn restore(snapshot: Snapshot) -> Content {
    let mut content = Content::with_text(&snapshot.text);
    content.move_to(snapshot.cursor);
    content
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::widget::text_editor::Position;

    fn snap(text: &str) -> (String, Cursor) {
        snap_at(text, 0, 0)
    }

    fn snap_at(text: &str, line: usize, column: usize) -> (String, Cursor) {
        (
            text.into(),
            Cursor {
                position: Position { line, column },
                selection: None,
            },
        )
    }

    #[test]
    fn undo_and_redo_restore_the_recorded_cursor_not_the_origin() {
        reset();
        record(|| snap_at("one\ntwo\nthree", 2, 3));
        let restored = undo(|| snap_at("one\ntwo\nthrXee", 2, 4)).expect("undo");
        assert_eq!(restored.cursor().position, Position { line: 2, column: 3 });
        let redone = redo(|| snap_at("one\ntwo\nthree", 2, 3)).expect("redo");
        assert_eq!(redone.cursor().position, Position { line: 2, column: 4 });
        reset();
    }

    #[test]
    fn undo_restores_the_recorded_text_and_redo_brings_the_edit_back() {
        reset();
        record(|| snap("one"));
        let restored = undo(|| snap("one edited")).expect("an undo step");
        assert_eq!(restored.text(), "one");
        let redone = redo(|| snap("one")).expect("a redo step");
        assert_eq!(redone.text(), "one edited");
        reset();
    }

    #[test]
    fn keystrokes_inside_the_window_coalesce_into_one_step() {
        reset();
        record(|| snap("a"));
        record(|| panic!("inside the window the snapshot must not be taken"));
        assert_eq!(undo(|| snap("abc")).expect("one step").text(), "a");
        assert!(undo(|| snap("a")).is_none(), "one group, one step");
        reset();
    }

    #[test]
    fn a_fresh_edit_clears_the_redo_lane() {
        reset();
        record(|| snap("a"));
        let _ = undo(|| snap("ab"));
        std::thread::sleep(COALESCE);
        record(|| snap("a"));
        assert!(redo(|| snap("aX")).is_none(), "redo dies on a fresh edit");
        reset();
    }
}
