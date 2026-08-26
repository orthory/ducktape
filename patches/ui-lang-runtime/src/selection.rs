//! Which selection the window is showing.
//!
//! A window shows one selection at a time, and it is not one widget's business
//! which: plain text is selectable here, and an application may make something
//! else selectable too — a rendered Markdown answer, a table cell. Each keeps
//! its own anchor and cursor, and they agree on nothing except this: the last
//! one to take the selection has it, and everything else is quiet.
//!
//! Holding it is a token rather than a flag, so letting go never has to be
//! delivered. A widget takes a fresh token when a drag starts and asks whether
//! it still holds it when it draws; a widget whose token has been superseded
//! draws nothing without having been told anything.
//!
//! It is held per thread rather than per process because that is the shape of
//! what it describes: an application draws its windows on one thread, so one
//! thread is one set of windows showing one selection. Two applications in one
//! process — which is what a test binary is — do not share a pointer, and must
//! not share this either.

use std::cell::Cell;

thread_local! {
    static NEXT: Cell<u64> = const { Cell::new(1) };
    static ACTIVE: Cell<u64> = const { Cell::new(0) };
}

/// Take the window's selection, and hand back the token that holds it.
pub fn claim() -> u64 {
    let token = NEXT.get();
    NEXT.set(token + 1);
    ACTIVE.set(token);
    token
}

/// Whether this token is still the one holding the selection.
pub fn holds(token: u64) -> bool {
    token != 0 && ACTIVE.get() == token
}

/// Let the selection go, so nothing is showing one.
pub fn clear() {
    ACTIVE.set(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole point: a second claim quiets the first without the first
    /// being told. Two highlights on screen at once is what this prevents.
    #[test]
    fn a_second_claim_takes_the_selection_from_the_first() {
        let first = claim();
        assert!(holds(first));

        let second = claim();
        assert!(!holds(first), "the first has to go quiet on its own");
        assert!(holds(second));

        clear();
        assert!(!holds(second), "and clearing quiets whoever held it");
    }

    /// A widget that has never claimed holds nothing, even before anything
    /// else has — a zero token must never read as the active one.
    #[test]
    fn a_widget_that_never_claimed_holds_nothing() {
        clear();
        assert!(!holds(0));
    }

    /// One thread's selection is not another's. A test binary runs a whole
    /// application per thread, and one of them taking the selection must not
    /// put out the selection in another.
    #[test]
    fn one_thread_taking_it_leaves_another_thread_holding_its_own() {
        let mine = claim();

        std::thread::spawn(|| {
            let theirs = claim();
            assert!(holds(theirs));
        })
        .join()
        .expect("the other thread finished");

        assert!(holds(mine), "another thread cannot take this one away");
    }
}
