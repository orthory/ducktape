//! pages' CLIENT view model — applied-op classification for feed followers.
//!
//! pages folds as SCOPED reloads in the shell for now: the block tree's flat
//! projection (pre-order prefixes, child counts) is rebuilt when the
//! keyboard-first editor rebuild lands, and the true per-op fold ships with
//! it. until then a follower only needs to know WHICH slices an op touched.
//! module-owned beside `index.rs` like chat's `client`, and pure, so the
//! module-bundled-UI lane can compile it into the shipped ui.wasm unchanged.

use crate::{PageMsg, decode_msg};

/// One classified pages op.
#[derive(Clone, Debug, Hash, PartialEq, Default)]
pub struct PagesDelta {
    /// `touched` — a pages op landed; the shell reloads the page slices
    /// (debounced). empty = not a pages-visible op.
    pub kind: String,
    /// the op was a comment op — the open comment panel refreshes too.
    pub comments: bool,
}

/// Classify one applied pages op. `Err` = the payload did not decode — the
/// caller's signal to fall back to a scoped resync.
pub fn delta_from_op(payload: &[u8]) -> Result<PagesDelta, String> {
    let msg = decode_msg(payload)?;
    let comments = matches!(
        msg,
        PageMsg::AddComment { .. }
            | PageMsg::MoveCommentThread { .. }
            | PageMsg::EditComment { .. }
            | PageMsg::DeleteComment { .. }
            | PageMsg::ResolveThread { .. }
    );
    Ok(PagesDelta {
        kind: "touched".into(),
        comments,
    })
}
