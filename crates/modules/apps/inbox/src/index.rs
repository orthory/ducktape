//! inbox's read model: per-member notification pages and unread counts —
//! folded from the applied-op feed into inbox's per-module index database.
//!
//! canonical inbox state holds the queues as consensus state (a delivery
//! commits atomically with the event that caused it — P2) but serves NO
//! queries: nothing in any execute() path reads an inbox, so the whole read
//! surface lives here, where the engine iterates natively.
//!
//! key spaces (inside inbox's per-module index database):
//! - `n/{hex(member)}/{seq:016x}` — one [`NotificationRow`] per live item;
//!   the member component is hex-encoded because member identities are
//!   opaque strings that may contain `/`, which would otherwise bleed one
//!   member's scan into another's.
//! - `nseq/{hex(member)}`    — mirror of the member's last assigned seq,
//!   written from each applied `Deliver`'s assigned stamp
//!   ([`crate::InboxAssigned::Delivered`] on the feed row) — the module's
//!   exact in-state assignment, never a counted derivation, so the mirror
//!   cannot desync across a boundary stamp.
//! - `ncnt/{hex(member)}`    — the member's live item count (u64 BE), the
//!   overflow mirror: a Deliver past [`MAX_ITEMS_PER_MEMBER`] drops the
//!   member's oldest row, exactly like the module.
//! - `nunread/{hex(member)}` — the member's unread count (u64 BE), so
//!   `unread` is one point read.
//! - `nread/{hex(member)}`   — the member's read watermark (u64 BE), mirrored
//!   from the module's own `MarkRead` semantics: everything at or below it
//!   reads as read. `read` on a [`NotificationRow`] is therefore DERIVED at
//!   query/fold time from `seq <= watermark`, never stored authoritatively —
//!   `MarkRead` costs one point read and one point write here too, never a
//!   scan-and-rewrite of every row in range. CLAMPED to the highest seq ever
//!   assigned (mirrored at `nseq/{hex(member)}`): an unclamped watermark
//!   would mark a FUTURE delivery pre-read on arrival.
//!
//! this file is the DECISION core — pure functions over [`StateRead`],
//! compiled natively and unit-tested against a plain map. the wasm shell
//! (`src/index_guest.rs`, feature `index-guest`) wires it into the engine.

use index_guest::{Fail, OpRow, OriginKind, OriginTag, StateRead, Writes};
use serde::{Deserialize, Serialize};

use crate::{InboxAssigned, InboxMsg, MAX_ITEMS_PER_MEMBER, decode_assigned, decode_msg};

/// default and max page size for notification listing.
const DEFAULT_LIST_LIMIT: usize = 50;
const MAX_LIST_LIMIT: usize = 256;

/// [`Fail`] code: an applied op's payload did not decode — interface drift,
/// which only a refold can honestly repair.
const FAIL_OP_DECODE: i32 = 2;
/// [`Fail`] code: a stored row did not decode — a damaged read model.
const FAIL_ROW_DECODE: i32 = 3;
/// [`Fail`] code: a view request this mapper does not speak.
const FAIL_BAD_REQUEST: i32 = 4;
/// [`Fail`] code: an applied `Deliver` carried a missing or undecodable
/// assigned stamp — the same interface-drift class as [`FAIL_OP_DECODE`].
const FAIL_ASSIGNED_DECODE: i32 = 5;

/// one delivered notification, as the list view returns it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationRow {
    pub seq: u64,
    /// the opaque member identity this notification belongs to.
    pub member: String,
    pub kind: String,
    pub body: String,
    /// the delivering origin, rendered: `user:{id}`, `module:{id}`, or
    /// `system`.
    pub source: String,
    pub height: u64,
    pub created_at: u64,
    /// DERIVED at query time from the member's read watermark (`seq <=
    /// watermark`) — never an authoritative stored bit. always `false` as
    /// persisted; a caller sees the real value only through [`serve_view`],
    /// which overwrites it before returning.
    pub read: bool,
}

/// inbox's view requests, externally tagged:
/// `{"list": {"member": "...", "from_seq": 1, "limit": 50}}`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InboxViewQuery {
    /// items for `member`, ascending by seq starting at `from_seq`.
    List {
        member: String,
        #[serde(default)]
        from_seq: u64,
        #[serde(default)]
        limit: Option<usize>,
    },
    /// count of unread items for `member`.
    Unread { member: String },
}

/// inbox's view replies.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InboxViewReply {
    Items(Vec<NotificationRow>),
    UnreadCount(u64),
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn member_prefix(member: &str) -> String {
    format!("n/{}/", hex_lower(member.as_bytes()))
}

fn item_key(member: &str, seq: u64) -> String {
    format!("n/{}/{seq:016x}", hex_lower(member.as_bytes()))
}

fn seq_key(member: &str) -> String {
    format!("nseq/{}", hex_lower(member.as_bytes()))
}

fn count_key(member: &str) -> String {
    format!("ncnt/{}", hex_lower(member.as_bytes()))
}

fn unread_key(member: &str) -> String {
    format!("nunread/{}", hex_lower(member.as_bytes()))
}

fn watermark_key(member: &str) -> String {
    format!("nread/{}", hex_lower(member.as_bytes()))
}

/// rendered delivering origin.
fn render_source(origin: &OriginTag) -> String {
    let id = origin.id.as_deref().unwrap_or_default();
    match origin.kind {
        OriginKind::Module => format!("module:{id}"),
        OriginKind::External => format!("user:{id}"),
        OriginKind::System => "system".to_string(),
    }
}

fn read_u64(read: &impl StateRead, key: &str) -> u64 {
    read.get(key.as_bytes())
        .and_then(|v| <[u8; 8]>::try_from(v.as_slice()).ok())
        .map(u64::from_be_bytes)
        .unwrap_or(0)
}

fn put_u64(out: &mut Writes, key: String, value: u64) {
    index_guest::put(out, key, value.to_be_bytes().to_vec());
}

fn put_row(out: &mut Writes, row: &NotificationRow) -> Result<(), Fail> {
    let bytes = serde_json::to_vec(row).map_err(|e| Fail::new(FAIL_ROW_DECODE, e.to_string()))?;
    index_guest::put(out, item_key(&row.member, row.seq), bytes);
    Ok(())
}

/// walk a member's rows from the low end up to `up_to_seq` inclusive,
/// calling `visit` on each. bounded by the queue cap, so at most a few scan
/// pages per op.
fn for_items_up_to(
    read: &impl StateRead,
    member: &str,
    up_to_seq: u64,
    mut visit: impl FnMut(NotificationRow) -> Result<(), Fail>,
) -> Result<(), Fail> {
    let prefix = member_prefix(member);
    let mut after: Option<Vec<u8>> = None;
    loop {
        let page = read.scan_page(prefix.as_bytes(), after.as_deref(), MAX_LIST_LIMIT);
        for (key, value) in &page.entries {
            let row: NotificationRow = serde_json::from_slice(value)
                .map_err(|e| Fail::new(FAIL_ROW_DECODE, e.to_string()))?;
            if row.seq > up_to_seq {
                return Ok(());
            }
            let _ = key; // rows carry their own address; the key is not re-parsed.
            visit(row)?;
        }
        if !page.has_more {
            return Ok(());
        }
        after = page.next_after.map(String::into_bytes);
    }
}

/// fold one applied op into derived writes. an applied op passed the
/// module's own validation (a failed op aborts its block and never reaches
/// the feed), so arms mirror the transition without re-judging it.
pub fn fold_op(op: &OpRow, read: &impl StateRead) -> Result<Writes, Fail> {
    let msg = decode_msg(&op.payload).map_err(|e| Fail::new(FAIL_OP_DECODE, e))?;
    let mut out = Writes::new();
    match msg {
        InboxMsg::Deliver { member, kind, body } => {
            let InboxAssigned::Delivered { seq } =
                decode_assigned(&op.assigned).map_err(|e| Fail::new(FAIL_ASSIGNED_DECODE, e))?;
            put_u64(&mut out, seq_key(&member), seq);
            let mut count = read_u64(read, &count_key(&member)) + 1;
            let mut unread = read_u64(read, &unread_key(&member)) + 1;
            // overflow mirror: past the cap, the OLDEST row drops. one
            // insert per op means at most one drop.
            if count > MAX_ITEMS_PER_MEMBER as u64 {
                let prefix = member_prefix(&member);
                let page = read.scan_page(prefix.as_bytes(), None, 1);
                if let Some((key, value)) = page.entries.first() {
                    let oldest: NotificationRow = serde_json::from_slice(value)
                        .map_err(|e| Fail::new(FAIL_ROW_DECODE, e.to_string()))?;
                    let watermark = read_u64(read, &watermark_key(&member));
                    let oldest_was_unread = oldest.seq > watermark;
                    if oldest_was_unread {
                        unread = unread.saturating_sub(1);
                    }
                    index_guest::delete(&mut out, String::from_utf8_lossy(key).to_string());
                    count -= 1;
                }
            }
            put_u64(&mut out, count_key(&member), count);
            put_u64(&mut out, unread_key(&member), unread);
            put_row(
                &mut out,
                &NotificationRow {
                    seq,
                    member,
                    kind,
                    body,
                    source: render_source(&op.origin),
                    height: op.height,
                    created_at: op.time,
                    read: false,
                },
            )?;
        }
        InboxMsg::MarkRead { member, up_to_seq } => {
            // O(1): one point read of the watermark plus (at most) the point
            // reads the unread delta needs — never a scan or a per-row write.
            // the live seqs form a contiguous range [last_seq-count+1,
            // last_seq] (both eviction and Clear only ever remove the LOW
            // end), so the count of live items newly covered by raising the
            // watermark is a closed-form intersection, not a walk.
            //
            // CLAMPED to `last_seq` (the highest seq ever assigned, mirrored
            // from `nseq`), same as the module: the old per-item flag could
            // only ever touch items that already existed, so storing a raw
            // `up_to_seq` (e.g. `u64::MAX`) would mark every FUTURE delivery
            // pre-read on arrival.
            let old_watermark = read_u64(read, &watermark_key(&member));
            let last_seq = read_u64(read, &seq_key(&member));
            let new_watermark = old_watermark.max(up_to_seq.min(last_seq));
            if new_watermark > old_watermark {
                let count = read_u64(read, &count_key(&member));
                if count > 0 {
                    let lowest_live = last_seq + 1 - count;
                    let lo = (old_watermark + 1).max(lowest_live);
                    // `new_watermark <= last_seq` by construction, so this is
                    // just `new_watermark` — spelled as a min to keep the
                    // range-intersection shape visible.
                    let hi = new_watermark.min(last_seq);
                    if hi >= lo {
                        let newly_read = hi - lo + 1;
                        let unread =
                            read_u64(read, &unread_key(&member)).saturating_sub(newly_read);
                        put_u64(&mut out, unread_key(&member), unread);
                    }
                }
                put_u64(&mut out, watermark_key(&member), new_watermark);
            }
        }
        InboxMsg::Clear { member, up_to_seq } => {
            let watermark = read_u64(read, &watermark_key(&member));
            let mut dropped = 0u64;
            let mut dropped_unread = 0u64;
            for_items_up_to(read, &member, up_to_seq, |row| {
                dropped += 1;
                if row.seq > watermark {
                    dropped_unread += 1;
                }
                index_guest::delete(&mut out, item_key(&row.member, row.seq));
                Ok(())
            })?;
            if dropped > 0 {
                let count = read_u64(read, &count_key(&member)).saturating_sub(dropped);
                put_u64(&mut out, count_key(&member), count);
                let unread = read_u64(read, &unread_key(&member)).saturating_sub(dropped_unread);
                put_u64(&mut out, unread_key(&member), unread);
                if count == 0 {
                    // the queue is now empty: the module deletes the
                    // member's META record (a later delivery re-mints fresh
                    // seqs from 1), so a stale watermark here would wrongly
                    // mark those NEW items read on arrival.
                    index_guest::delete(&mut out, watermark_key(&member));
                }
            }
        }
    }
    Ok(out)
}

/// serve one materialized-view request.
pub fn serve_view(read: &impl StateRead, req: &[u8]) -> Result<Vec<u8>, Fail> {
    let query: InboxViewQuery =
        serde_json::from_slice(req).map_err(|e| Fail::new(FAIL_BAD_REQUEST, e.to_string()))?;
    let reply = match query {
        InboxViewQuery::List {
            member,
            from_seq,
            limit,
        } => {
            // the scan cursor is exclusive: to start AT `from_seq`, cursor
            // from the key one sequence below (seq space starts at 1).
            let after = (from_seq > 1).then(|| item_key(&member, from_seq - 1).into_bytes());
            let page = read.scan_page(
                member_prefix(&member).as_bytes(),
                after.as_deref(),
                limit.unwrap_or(DEFAULT_LIST_LIMIT).clamp(1, MAX_LIST_LIMIT),
            );
            let watermark = read_u64(read, &watermark_key(&member));
            let mut items = Vec::with_capacity(page.entries.len());
            for (_key, value) in &page.entries {
                let mut row: NotificationRow = serde_json::from_slice(value)
                    .map_err(|e| Fail::new(FAIL_ROW_DECODE, e.to_string()))?;
                // DERIVED, never the stored (always-false) bit — see
                // `NotificationRow::read`.
                row.read = row.seq <= watermark;
                items.push(row);
            }
            InboxViewReply::Items(items)
        }
        InboxViewQuery::Unread { member } => {
            InboxViewReply::UnreadCount(read_u64(read, &unread_key(&member)))
        }
    };
    serde_json::to_vec(&reply).map_err(|e| Fail::new(FAIL_BAD_REQUEST, e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{encode_assigned, encode_msg};
    use index_guest::apply_to_map;
    use std::collections::BTreeMap;

    type Map = BTreeMap<Vec<u8>, Vec<u8>>;

    /// the test twin of the module's in-state assignment (`stage_deliver`):
    /// a Deliver takes the member's next sequence; nothing else assigns.
    fn assigned_for(map: &Map, msg: &InboxMsg) -> Vec<u8> {
        match msg {
            InboxMsg::Deliver { member, .. } => encode_assigned(&InboxAssigned::Delivered {
                seq: read_u64(map, &seq_key(member)) + 1,
            }),
            InboxMsg::MarkRead { .. } | InboxMsg::Clear { .. } => Vec::new(),
        }
    }

    fn op(map: &Map, height: u64, origin: OriginTag, msg: &InboxMsg) -> OpRow {
        OpRow {
            height,
            seq: 0,
            time: 1_000 + height,
            origin,
            payload: encode_msg(msg),
            assigned: assigned_for(map, msg),
        }
    }

    fn fold(map: &mut Map, height: u64, msg: &InboxMsg) {
        let writes = fold_op(&op(map, height, OriginTag::module("chat"), msg), map).expect("fold");
        apply_to_map(map, writes);
    }

    fn deliver(member: &str, kind: &str, body: &str) -> InboxMsg {
        InboxMsg::Deliver {
            member: member.into(),
            kind: kind.into(),
            body: body.into(),
        }
    }

    fn view(map: &Map, req: serde_json::Value) -> InboxViewReply {
        let bytes = serve_view(map, &serde_json::to_vec(&req).unwrap()).expect("view");
        serde_json::from_slice(&bytes).expect("reply decodes")
    }

    fn items(map: &Map, req: serde_json::Value) -> Vec<NotificationRow> {
        match view(map, req) {
            InboxViewReply::Items(items) => items,
            other => panic!("expected items, got {other:?}"),
        }
    }

    fn unread(map: &Map, member: &str) -> u64 {
        match view(map, serde_json::json!({"unread": {"member": member}})) {
            InboxViewReply::UnreadCount(n) => n,
            other => panic!("expected unread count, got {other:?}"),
        }
    }

    #[test]
    fn deliveries_page_per_member_and_track_unread() {
        let mut map = Map::new();
        fold(
            &mut map,
            1,
            &deliver("alice", "mention", "you were mentioned"),
        );
        fold(&mut map, 2, &deliver("alice", "reply", "someone replied"));
        fold(&mut map, 3, &deliver("bob", "mention", "bob's own"));

        let rows = items(&map, serde_json::json!({"list": {"member": "alice"}}));
        assert_eq!(rows.len(), 2);
        assert_eq!((rows[0].seq, rows[1].seq), (1, 2));
        assert_eq!(rows[0].source, "module:chat");
        assert_eq!(unread(&map, "alice"), 2);
        assert_eq!(unread(&map, "bob"), 1);

        // from_seq starts the page mid-queue.
        let rows = items(
            &map,
            serde_json::json!({"list": {"member": "alice", "from_seq": 2}}),
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].seq, 2);
    }

    #[test]
    fn mark_read_and_clear_mirror_module_semantics() {
        let mut map = Map::new();
        for i in 1..=3 {
            fold(&mut map, i, &deliver("alice", "k", &format!("n{i}")));
        }
        fold(
            &mut map,
            4,
            &InboxMsg::MarkRead {
                member: "alice".into(),
                up_to_seq: 2,
            },
        );
        assert_eq!(unread(&map, "alice"), 1);
        let rows = items(&map, serde_json::json!({"list": {"member": "alice"}}));
        assert_eq!(
            rows.iter().map(|r| r.read).collect::<Vec<_>>(),
            vec![true, true, false]
        );

        // idempotent re-mark changes nothing.
        fold(
            &mut map,
            5,
            &InboxMsg::MarkRead {
                member: "alice".into(),
                up_to_seq: 2,
            },
        );
        assert_eq!(unread(&map, "alice"), 1);

        fold(
            &mut map,
            6,
            &InboxMsg::Clear {
                member: "alice".into(),
                up_to_seq: 2,
            },
        );
        let rows = items(&map, serde_json::json!({"list": {"member": "alice"}}));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].seq, 3);
        assert_eq!(
            unread(&map, "alice"),
            1,
            "the unread survivor stays counted"
        );

        // a new delivery continues the seq space — Clear never rewinds it.
        fold(&mut map, 7, &deliver("alice", "k", "n4"));
        let rows = items(&map, serde_json::json!({"list": {"member": "alice"}}));
        assert_eq!(rows.last().map(|r| r.seq), Some(4));
    }

    #[test]
    fn list_limit_defaults_and_clamps() {
        let mut map = Map::new();
        for i in 1..=60 {
            fold(&mut map, i, &deliver("alice", "k", &format!("n{i}")));
        }

        // no limit: the default page, from the low end.
        let rows = items(&map, serde_json::json!({"list": {"member": "alice"}}));
        assert_eq!(rows.len(), DEFAULT_LIST_LIMIT);
        assert_eq!(
            (rows[0].seq, rows[DEFAULT_LIST_LIMIT - 1].seq),
            (1, DEFAULT_LIST_LIMIT as u64)
        );

        // an explicit limit bounds the page, composed with from_seq.
        let rows = items(
            &map,
            serde_json::json!({"list": {"member": "alice", "from_seq": 5, "limit": 2}}),
        );
        assert_eq!(rows.iter().map(|r| r.seq).collect::<Vec<_>>(), vec![5, 6]);

        // limit 0 clamps up to one row; an absurd limit clamps down to
        // MAX_LIST_LIMIT — which still covers all sixty rows.
        let rows = items(
            &map,
            serde_json::json!({"list": {"member": "alice", "limit": 0}}),
        );
        assert_eq!(rows.len(), 1);
        let rows = items(
            &map,
            serde_json::json!({"list": {"member": "alice", "limit": 100_000}}),
        );
        assert_eq!(rows.len(), 60, "clamped limit still covers every row");
    }

    /// the overflow mirror of the module's [`MAX_ITEMS_PER_MEMBER`] cap: a
    /// Deliver past the cap drops the member's OLDEST row, with the unread
    /// count following the dropped row's read flag.
    #[test]
    fn overflow_drops_oldest_row_like_the_module() {
        let cap = MAX_ITEMS_PER_MEMBER as u64;
        let mut map = Map::new();
        // one over the per-member cap, so exactly one drop fires.
        for i in 1..=cap + 1 {
            fold(&mut map, i, &deliver("alice", "k", "b"));
        }
        assert_eq!(
            unread(&map, "alice"),
            cap,
            "the dropped row was unread: unread sits at the cap, not cap+1"
        );
        let rows = items(
            &map,
            serde_json::json!({"list": {"member": "alice", "limit": 1}}),
        );
        assert_eq!(rows[0].seq, 2, "seq 1 (the oldest) was dropped");
        let rows = items(
            &map,
            serde_json::json!({"list": {"member": "alice", "from_seq": cap + 1}}),
        );
        assert_eq!(
            rows.iter().map(|r| r.seq).collect::<Vec<_>>(),
            vec![cap + 1],
            "the newest row survives"
        );

        // dropping a READ oldest must not decrement unread: mark seq 2 read,
        // overflow again — the victim is read, so the new arrival counts in
        // full.
        fold(
            &mut map,
            cap + 2,
            &InboxMsg::MarkRead {
                member: "alice".into(),
                up_to_seq: 2,
            },
        );
        assert_eq!(unread(&map, "alice"), cap - 1);
        fold(&mut map, cap + 3, &deliver("alice", "k", "overflow again"));
        assert_eq!(
            unread(&map, "alice"),
            cap,
            "a read drop victim leaves unread to the new arrival"
        );
        let rows = items(
            &map,
            serde_json::json!({"list": {"member": "alice", "limit": 1}}),
        );
        assert_eq!(
            (rows[0].seq, rows[0].read),
            (3, false),
            "seq 2 (read) was the drop victim"
        );
    }

    #[test]
    fn members_with_slashes_do_not_bleed_scans() {
        let mut map = Map::new();
        fold(&mut map, 1, &deliver("a", "k", "for a"));
        fold(&mut map, 2, &deliver("a/b", "k", "for a slash b"));

        let rows = items(&map, serde_json::json!({"list": {"member": "a"}}));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].body, "for a");
        let rows = items(&map, serde_json::json!({"list": {"member": "a/b"}}));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].body, "for a slash b");
    }

    /// mirrors the module's own clamp: `MarkRead { up_to_seq: u64::MAX }`
    /// must not mark a FUTURE delivery pre-read in the derived view either.
    #[test]
    fn mark_read_beyond_the_last_seq_does_not_pre_read_future_deliveries() {
        let mut map = Map::new();
        fold(&mut map, 1, &deliver("alice", "k", "b"));
        fold(
            &mut map,
            2,
            &InboxMsg::MarkRead {
                member: "alice".into(),
                up_to_seq: u64::MAX,
            },
        );
        fold(&mut map, 3, &deliver("alice", "k", "c"));

        let rows = items(&map, serde_json::json!({"list": {"member": "alice"}}));
        assert_eq!(
            rows.iter().map(|r| (r.seq, r.read)).collect::<Vec<_>>(),
            vec![(1, true), (2, false)],
            "seq 2 was delivered AFTER the mark-read and must not be pre-read"
        );
        assert_eq!(unread(&map, "alice"), 1);
    }
}
