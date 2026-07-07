//! id-length enforcement for pages' op validation path.
//!
//! isolated from `lib.rs` (a forbidden-growth mono-file) per the repo's code
//! organization rule: the cap constant lives on [`crate::MAX_ID_BYTES`]
//! (documented there, next to the wire types it bounds); this module is just
//! the one guard function `apply` calls.

use crate::{MAX_ID_BYTES, PageError};

/// reject an op whose `ids` include one over [`MAX_ID_BYTES`], before any
/// storage touch.
///
/// called with every id `apply` NAMES for the op — not only the ones it
/// mints. a reference to an id already committed elsewhere (a `parent`, an
/// `after` sibling anchor, an append's existing `thread_id`, …) is guaranteed
/// to already conform: this is a flag-day cap on an unmerged module, so
/// nothing in the store predates it. checking a reference again costs
/// nothing and keeps the invariant simple to state and to audit: every
/// string an op calls an id is `<= MAX_ID_BYTES`, full stop — rather than a
/// second, narrower list of exactly which fields are "new" this op (a
/// distinction `apply`'s per-variant match would have to get right for every
/// current AND future [`crate::PageMsg`] arm).
pub(crate) fn check_id_len(ids: &[&str]) -> Result<(), PageError> {
    if ids.iter().any(|id| id.len() > MAX_ID_BYTES) {
        return Err(PageError::IdTooLong);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_cap_is_accepted() {
        let at_cap = "x".repeat(MAX_ID_BYTES);
        assert_eq!(check_id_len(&[at_cap.as_str()]), Ok(()));
        assert_eq!(check_id_len(&["short", at_cap.as_str(), "b1"]), Ok(()));
    }

    #[test]
    fn over_cap_is_rejected() {
        let over_cap = "x".repeat(MAX_ID_BYTES + 1);
        assert_eq!(
            check_id_len(&[over_cap.as_str()]),
            Err(PageError::IdTooLong)
        );
        // one oversized id among otherwise-fine ones still rejects the whole
        // op — a partial pass would defeat the point of the cap.
        assert_eq!(
            check_id_len(&["short", over_cap.as_str(), "b1"]),
            Err(PageError::IdTooLong)
        );
    }

    #[test]
    fn empty_list_is_accepted() {
        assert_eq!(check_id_len(&[]), Ok(()));
    }
}
