//! the duckfs read cap, applied to a REPLY — the one copy of it.
//!
//! `ModelRecord::permits(CapRequest::DuckfsRead(..))` answers about one path.
//! A duckfs PREFIX query (`find`, `grep`) is not one path: duckfs' own prefix
//! rule is a raw string prefix (`/shared/team` also matches
//! `/shared/team-secrets/...`) while the cap is segment-boundary, so passing
//! the gate on a call's `prefix` does not make the rows it answers with
//! covered. Each row's own path — and the resume cursor, which is a path in the
//! same walk — has to be re-checked.
//!
//! Two callers do that, on the same replies: the `ducktape mcp` tool plane
//! (`bin/node/src/mcp/tools/read.rs`) and the sandboxed run's read lane
//! ([`crate::read_lane`]), which fronts the raw `/v1/files/*` routes the tool
//! plane's gate could otherwise be walked around with a `curl`. A second
//! hand-rolled copy in either is how the two drift into disagreeing about what
//! an agent may read, so there is one, here, and a source-parsing test in
//! `read_lane` pins that neither defines its own.
//!
//! It lives in this crate rather than in `runs` because `runs` is a consensus
//! module compiled to wasm: a host-side reply filter has no business in the
//! guest's bytes.

use runs::{CapRequest, ModelRecord};
use serde_json::Value;

/// drop every row of `reply[rows]` whose own `path` the record's `duckfs_read`
/// cap does not cover — a no-op when the reply carries no such array.
pub fn retain_capped_rows(record: &ModelRecord, reply: &mut Value, rows: &str) {
    let Some(list) = reply.get_mut(rows).and_then(Value::as_array_mut) else {
        return;
    };
    list.retain(|row| {
        row.get("path")
            .and_then(Value::as_str)
            .is_some_and(|path| record.permits(&CapRequest::DuckfsRead(path)))
    });
}

/// replace a `next` resume cursor the cap does not cover: such a cursor names a
/// path in a sibling tree the agent may not read at all — a page that ran out
/// of budget mid-scan of an out-of-cap sibling hands that sibling's path back
/// as its cursor.
///
/// The fallback is the last RETAINED row's own path — still a valid resume
/// point, inside the cap — or no cursor at all when no row survived filtering.
/// A no-op when the reply carries no cursor already.
pub fn scrub_uncapped_cursor(record: &ModelRecord, reply: &mut Value, rows: &str) {
    // no cursor (missing key, or an explicit `null` meaning "no more pages") is
    // nothing to scrub.
    let Some(next) = reply.get("next").and_then(Value::as_str) else {
        return;
    };
    if record.permits(&CapRequest::DuckfsRead(next)) {
        return;
    }
    let fallback = reply
        .get(rows)
        .and_then(Value::as_array)
        .and_then(|list| list.last())
        .and_then(|row| row.get("path"))
        .cloned()
        .unwrap_or(Value::Null);
    reply["next"] = fallback;
}
