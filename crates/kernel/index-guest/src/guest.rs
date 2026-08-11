//! the wasm shell surface — everything a module's `src/index_guest.rs` needs
//! to wire its pure decision core (`src/index.rs`) into the engine: the feed
//! decoder, the engine-backed [`StateRead`], the writer, and the entry
//! macros. see the crate docs for the whole authoring shape.
//!
//! failure discipline: a fold that cannot mirror an APPLIED op has no honest
//! fallback — return [`Fail`] and let the engine hold the queue (the event is
//! retained, the error surfaces on the trigger). never skip an op silently.

/// the engine SDK types a shell touches directly. everything else routes
/// through [`ops`], [`EngineRead`], and [`apply`].
pub use fluent_guest::{Change, Scan, errno, log};

#[doc(hidden)]
pub use fluent_guest::__entry;

use crate::{Fail, OpRow, Page, StateRead, Writes, collect_page, prefix_successor, scan_lo};

/// [`Fail`] code: a delete arrived on the fold feed. op rows are immutable
/// and only ever wiped with the trigger torn down first (`mark_backfilled`),
/// so a delivered delete means the host contract broke — hold the queue.
pub const FAIL_FEED_DELETE: i32 = 10;
/// [`Fail`] code: an op row's borsh envelope did not decode.
pub const FAIL_ROW_DECODE: i32 = 11;
/// [`Fail`] code: a value-elided op row was not readable from state.
pub const FAIL_ROW_MISSING: i32 = 12;
/// [`Fail`] code: the engine refused a derived write (errno in the message).
pub const FAIL_WRITE_REFUSED: i32 = 13;

/// ducktape's [`Fail`] crossing into the engine SDK at the entry boundary —
/// the one place the two vocabularies meet.
impl From<Fail> for fluent_guest::Fail {
    fn from(fail: Fail) -> Self {
        fluent_guest::Fail::new(fail.code, fail.message)
    }
}

/// decode a fold invocation's changes into the op rows they carry, in commit
/// order. a value above the engine's inline cap arrives elided — the row is
/// re-read from state, which is exact: op rows are immutable, so "current
/// state" IS the change.
pub fn ops(changes: Vec<Change>) -> Result<Vec<OpRow>, Fail> {
    changes.into_iter().map(op_from_change).collect()
}

fn op_from_change(change: Change) -> Result<OpRow, Fail> {
    let Change::Put { key, value, .. } = change else {
        return Err(Fail::new(
            FAIL_FEED_DELETE,
            "delete on the fold feed — op rows are wiped only with the trigger down",
        ));
    };
    let bytes = match value {
        Some(bytes) => bytes,
        None => fluent_guest::get(&key)
            .ok_or_else(|| Fail::new(FAIL_ROW_MISSING, "value-elided op row absent from state"))?,
    };
    borsh::from_slice(&bytes)
        .map_err(|e| Fail::new(FAIL_ROW_DECODE, format!("op row envelope: {e}")))
}

/// the engine-backed [`StateRead`]: reads at this invocation's snapshot — a
/// fold additionally sees its own transaction's earlier writes (so apply each
/// op's writes before deciding the next), a view is a pure snapshot.
pub struct EngineRead;

impl StateRead for EngineRead {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        fluent_guest::get(key)
    }

    fn scan_page(&self, prefix: &[u8], after: Option<&[u8]>, limit: usize) -> Page {
        let lo = scan_lo(prefix, after);
        let hi = prefix_successor(prefix);
        let Ok(scan) = fluent_guest::scan(Some(&lo), hi.as_deref()) else {
            // scan_open only errors on handle exhaustion — a page of nothing
            // is wrong, but silently truncating would hide it; empty + no
            // cursor makes the miss visible upstream.
            return Page {
                entries: Vec::new(),
                has_more: false,
                next_after: None,
            };
        };
        collect_page(scan, limit)
    }
}

/// apply decided writes through the engine, in command order, inside the
/// current transaction.
pub fn apply(writes: Writes) -> Result<(), Fail> {
    for (key, cmd) in writes {
        let refused = match cmd {
            Some(value) => fluent_guest::put(key.as_bytes(), &value).err(),
            None => fluent_guest::delete(key.as_bytes()).err(),
        };
        if let Some(errno) = refused {
            return Err(Fail::new(
                FAIL_WRITE_REFUSED,
                format!("write of {key:?} refused: errno {errno}"),
            ));
        }
    }
    Ok(())
}

/// export `$f: fn(Vec<Change>) -> Result<(), index_guest::Fail>` as the
/// mapper's fold entry (fluentabi `on_apply` — the changes-mode trigger
/// hook).
///
/// expands to `#[unsafe(no_mangle)] pub extern "C" fn on_apply() -> i32`
/// delegating decode/encode/exit-code glue to the engine SDK's entry shim,
/// with the [`Fail`] conversion at the boundary.
#[macro_export]
macro_rules! fold {
    ($f:path) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn on_apply() -> i32 {
            $crate::guest::__entry(|changes| $f(changes).map_err(::core::convert::Into::into))
        }
    };
}

/// export `$f: fn(Vec<u8>) -> Result<Vec<u8>, index_guest::Fail>` as the
/// mapper's view entry (fluentabi `query` — read-only at one MVCC snapshot).
/// same shim and conversion as [`fold!`].
#[macro_export]
macro_rules! view {
    ($f:path) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn query() -> i32 {
            $crate::guest::__entry(|req| $f(req).map_err(::core::convert::Into::into))
        }
    };
}
