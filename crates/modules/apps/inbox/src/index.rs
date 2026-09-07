//! inbox's read model: per-account notification pages and unread counts —
//! folded from the applied-op feed into inbox's per-module index database.
//!
//! canonical inbox state holds the queues as consensus state (a notification
//! commits in the unit of the attribution delivery that caused it) but serves
//! NO queries: nothing in any execute() path reads an inbox, so the whole
//! read surface lives here, where the engine iterates natively.
//!
//! the feed carries two kinds of applied op, told apart the way the module
//! tells them apart — by the authenticated origin: an op from
//! `Origin::Module(attribution)` is a delivery (its payload an
//! [`attribution::AttributionEvent`], its stamp the module's
//! [`InboxAssigned`]); any other origin's op is an admin [`InboxMsg`].
//!
//! key spaces (inside inbox's per-module index database):
//! - `n/{account}/{seq:016x}` — one [`NotificationRow`] per live item.
//! - `nseq/{account}`    — mirror of the account's last assigned seq, written
//!   from each applied delivery's assigned stamp
//!   ([`InboxAssigned::Delivered`] on the feed row) — the module's exact
//!   in-state assignment, never a counted derivation.
//! - `ncnt/{account}`    — the account's live item count (u64 BE), the
//!   overflow mirror: a delivery past [`MAX_ITEMS_PER_ACCOUNT`] drops the
//!   account's oldest row, exactly like the module.
//! - `nunread/{account}` — the account's unread count (u64 BE), so `unread`
//!   is one point read.
//! - `nread/{account}`   — the account's read watermark (u64 BE), mirrored
//!   from the module's own `MarkRead` semantics: everything at or below it
//!   reads as read. `read` on a [`NotificationRow`] is therefore DERIVED at
//!   query/fold time from `seq <= watermark`, never stored authoritatively.
//!   CLAMPED to the highest seq ever assigned (mirrored at `nseq/`): an
//!   unclamped watermark would mark a FUTURE delivery pre-read on arrival.
//!
//! this file is the DECISION core — pure functions over [`StateRead`],
//! compiled natively and unit-tested against a plain map. the wasm shell
//! (`src/index_guest.rs`, feature `index-guest`) wires it into the engine.

use attribution::{AttributionEvent, ChangeRef, decode_event};
use index_guest::{Fail, OpRow, OriginKind, StateRead, Writes};
use serde::{Deserialize, Serialize};

use crate::{
    AccountNumber, InboxAssigned, InboxMsg, MAX_ITEMS_PER_ACCOUNT, decode_assigned, decode_msg,
};

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
/// [`Fail`] code: an applied delivery carried a missing or undecodable
/// assigned stamp — the same interface-drift class as [`FAIL_OP_DECODE`].
const FAIL_ASSIGNED_DECODE: i32 = 5;

/// one delivered notification, as the list view returns it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationRow {
    pub seq: u64,
    pub account: AccountNumber,
    /// the canonical change, by reference — the attribution plane holds the
    /// record (and its detail) this row points at.
    pub change: ChangeRef,
    pub height: u64,
    pub created_at: u64,
    /// DERIVED at query time from the account's read watermark (`seq <=
    /// watermark`) — never an authoritative stored bit. always `false` as
    /// persisted; a caller sees the real value only through [`serve_view`],
    /// which overwrites it before returning.
    pub read: bool,
}

/// inbox's view requests, externally tagged:
/// `{"list": {"account": 7, "from_seq": 1, "limit": 50}}`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InboxViewQuery {
    /// items for `account`, ascending by seq starting at `from_seq`.
    List {
        account: AccountNumber,
        #[serde(default)]
        from_seq: u64,
        #[serde(default)]
        limit: Option<usize>,
    },
    /// count of unread items for `account`.
    Unread { account: AccountNumber },
}

/// inbox's view replies.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InboxViewReply {
    Items(Vec<NotificationRow>),
    UnreadCount(u64),
}

fn account_prefix(account: AccountNumber) -> String {
    format!("n/{account}/")
}

fn item_key(account: AccountNumber, seq: u64) -> String {
    format!("n/{account}/{seq:016x}")
}

fn seq_key(account: AccountNumber) -> String {
    format!("nseq/{account}")
}

fn count_key(account: AccountNumber) -> String {
    format!("ncnt/{account}")
}

fn unread_key(account: AccountNumber) -> String {
    format!("nunread/{account}")
}

fn watermark_key(account: AccountNumber) -> String {
    format!("nread/{account}")
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
    index_guest::put(out, item_key(row.account, row.seq), bytes);
    Ok(())
}

/// walk an account's rows from the low end up to `up_to_seq` inclusive,
/// calling `visit` on each. bounded by the queue cap, so at most a few scan
/// pages per op.
fn for_items_up_to(
    read: &impl StateRead,
    account: AccountNumber,
    up_to_seq: u64,
    mut visit: impl FnMut(NotificationRow) -> Result<(), Fail>,
) -> Result<(), Fail> {
    let prefix = account_prefix(account);
    let mut after: Option<Vec<u8>> = None;
    loop {
        let page = read.scan_page(prefix.as_bytes(), after.as_deref(), MAX_LIST_LIMIT);
        for (_key, value) in &page.entries {
            let row: NotificationRow = serde_json::from_slice(value)
                .map_err(|e| Fail::new(FAIL_ROW_DECODE, e.to_string()))?;
            if row.seq > up_to_seq {
                return Ok(());
            }
            visit(row)?;
        }
        if !page.has_more {
            return Ok(());
        }
        after = page.next_after.map(String::into_bytes);
    }
}

/// whether an applied op is the attribution source's delivery — the same
/// discriminant the module classifies by.
fn is_delivery(op: &OpRow, attribution: &str) -> bool {
    let from_a_module = op.origin.kind == OriginKind::Module;
    let from_attribution = op.origin.id.as_deref() == Some(attribution);
    from_a_module && from_attribution
}

/// fold one applied delivery: the stamp says what the module did.
fn fold_delivery(op: &OpRow, read: &impl StateRead, out: &mut Writes) -> Result<(), Fail> {
    let AttributionEvent::Changed(change) =
        decode_event(&op.payload).map_err(|e| Fail::new(FAIL_OP_DECODE, e))?;
    let stamp = decode_assigned(&op.assigned).map_err(|e| Fail::new(FAIL_ASSIGNED_DECODE, e))?;
    let seq = match stamp {
        InboxAssigned::Delivered { seq } => seq,
        InboxAssigned::Duplicate | InboxAssigned::Ignored => return Ok(()),
    };
    let account = change.recipient;
    put_u64(out, seq_key(account), seq);
    let mut count = read_u64(read, &count_key(account)) + 1;
    let mut unread = read_u64(read, &unread_key(account)) + 1;
    // overflow mirror: past the cap, the OLDEST row drops. one insert per
    // delivery means at most one drop.
    if count > MAX_ITEMS_PER_ACCOUNT as u64 {
        let prefix = account_prefix(account);
        let page = read.scan_page(prefix.as_bytes(), None, 1);
        if let Some((key, value)) = page.entries.first() {
            let oldest: NotificationRow = serde_json::from_slice(value)
                .map_err(|e| Fail::new(FAIL_ROW_DECODE, e.to_string()))?;
            let watermark = read_u64(read, &watermark_key(account));
            let oldest_was_unread = oldest.seq > watermark;
            if oldest_was_unread {
                unread = unread.saturating_sub(1);
            }
            index_guest::delete(out, String::from_utf8_lossy(key).to_string());
            count -= 1;
        }
    }
    put_u64(out, count_key(account), count);
    put_u64(out, unread_key(account), unread);
    put_row(
        out,
        &NotificationRow {
            seq,
            account,
            change: change.reference(),
            height: op.height,
            created_at: op.time,
            read: false,
        },
    )
}

/// fold one applied admin op.
fn fold_admin(op: &OpRow, read: &impl StateRead, out: &mut Writes) -> Result<(), Fail> {
    match decode_msg(&op.payload).map_err(|e| Fail::new(FAIL_OP_DECODE, e))? {
        InboxMsg::MarkRead { account, up_to_seq } => {
            // O(1): one point read of the watermark plus (at most) the point
            // reads the unread delta needs — never a scan or a per-row write.
            // the live seqs form a contiguous range [last_seq-count+1,
            // last_seq] (both eviction and Clear only ever remove the LOW
            // end), so the count of live items newly covered by raising the
            // watermark is a closed-form intersection, not a walk.
            //
            // CLAMPED to `last_seq` (the highest seq ever assigned, mirrored
            // from `nseq`), same as the module.
            let old_watermark = read_u64(read, &watermark_key(account));
            let last_seq = read_u64(read, &seq_key(account));
            let new_watermark = old_watermark.max(up_to_seq.min(last_seq));
            let unchanged = new_watermark <= old_watermark;
            if unchanged {
                return Ok(());
            }
            let count = read_u64(read, &count_key(account));
            if count > 0 {
                let lowest_live = last_seq + 1 - count;
                let lo = (old_watermark + 1).max(lowest_live);
                let hi = new_watermark;
                if hi >= lo {
                    let newly_read = hi - lo + 1;
                    let unread = read_u64(read, &unread_key(account)).saturating_sub(newly_read);
                    put_u64(out, unread_key(account), unread);
                }
            }
            put_u64(out, watermark_key(account), new_watermark);
        }
        InboxMsg::Clear { account, up_to_seq } => {
            let watermark = read_u64(read, &watermark_key(account));
            let mut dropped = 0u64;
            let mut dropped_unread = 0u64;
            for_items_up_to(read, account, up_to_seq, |row| {
                dropped += 1;
                if row.seq > watermark {
                    dropped_unread += 1;
                }
                index_guest::delete(out, item_key(row.account, row.seq));
                Ok(())
            })?;
            if dropped > 0 {
                let count = read_u64(read, &count_key(account)).saturating_sub(dropped);
                put_u64(out, count_key(account), count);
                let unread = read_u64(read, &unread_key(account)).saturating_sub(dropped_unread);
                put_u64(out, unread_key(account), unread);
            }
        }
    }
    Ok(())
}

/// fold one applied op into derived writes. an applied op passed the
/// module's own validation (a failed op aborts its unit and never reaches
/// the feed), so arms mirror the transition without re-judging it.
/// `attribution` is the attribution module's id — the origin whose ops are
/// deliveries.
pub fn fold_op(op: &OpRow, read: &impl StateRead, attribution: &str) -> Result<Writes, Fail> {
    let mut out = Writes::new();
    match is_delivery(op, attribution) {
        true => fold_delivery(op, read, &mut out)?,
        false => fold_admin(op, read, &mut out)?,
    }
    Ok(out)
}

/// serve one materialized-view request.
pub fn serve_view(read: &impl StateRead, req: &[u8]) -> Result<Vec<u8>, Fail> {
    let query: InboxViewQuery =
        serde_json::from_slice(req).map_err(|e| Fail::new(FAIL_BAD_REQUEST, e.to_string()))?;
    let reply = match query {
        InboxViewQuery::List {
            account,
            from_seq,
            limit,
        } => {
            // the scan cursor is exclusive: to start AT `from_seq`, cursor
            // from the key one sequence below (seq space starts at 1).
            let after = (from_seq > 1).then(|| item_key(account, from_seq - 1).into_bytes());
            let page = read.scan_page(
                account_prefix(account).as_bytes(),
                after.as_deref(),
                limit.unwrap_or(DEFAULT_LIST_LIMIT).clamp(1, MAX_LIST_LIMIT),
            );
            let watermark = read_u64(read, &watermark_key(account));
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
        InboxViewQuery::Unread { account } => {
            InboxViewReply::UnreadCount(read_u64(read, &unread_key(account)))
        }
    };
    serde_json::to_vec(&reply).map_err(|e| Fail::new(FAIL_BAD_REQUEST, e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{encode_assigned, encode_msg};
    use attribution::{Actor, Change, ChangeKind, Reason, Source, encode_event};
    use index_guest::{OriginTag, apply_to_map};
    use std::collections::BTreeMap;

    type Map = BTreeMap<Vec<u8>, Vec<u8>>;

    const ATTRIBUTION: &str = "attribution";
    const ALICE: AccountNumber = 7;
    const BOB: AccountNumber = 9;

    fn change(seq: u64, recipient: AccountNumber, object: &str) -> Change {
        Change {
            seq,
            source: Source {
                module: "chat".into(),
                kind: "message".into(),
                object: object.into(),
            },
            revision: 1,
            recipient,
            reason: Reason::Mention,
            kind: ChangeKind::Added,
            detail: vec![1, 2, 3],
            actor: Actor::Account(BOB),
            cause: sdk::Cause::Direct,
            height: 1,
        }
    }

    /// the test twin of the module's in-state assignment: a delivery takes
    /// the recipient's next sequence.
    fn delivered(map: &Map, change: &Change) -> Vec<u8> {
        encode_assigned(&InboxAssigned::Delivered {
            seq: read_u64(map, &seq_key(change.recipient)) + 1,
        })
    }

    fn op(height: u64, origin: OriginTag, payload: Vec<u8>, assigned: Vec<u8>) -> OpRow {
        OpRow {
            height,
            seq: 0,
            time: 1_000 + height,
            origin,
            payload,
            assigned,
        }
    }

    fn fold_change(map: &mut Map, height: u64, change: &Change) {
        let row = op(
            height,
            OriginTag::module(ATTRIBUTION),
            encode_event(&AttributionEvent::Changed(change.clone())),
            delivered(map, change),
        );
        let writes = fold_op(&row, map, ATTRIBUTION).expect("fold");
        apply_to_map(map, writes);
    }

    fn fold_admin_op(map: &mut Map, height: u64, msg: &InboxMsg) {
        let row = op(
            height,
            OriginTag::external("alice"),
            encode_msg(msg),
            Vec::new(),
        );
        let writes = fold_op(&row, map, ATTRIBUTION).expect("fold");
        apply_to_map(map, writes);
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

    fn unread(map: &Map, account: AccountNumber) -> u64 {
        match view(map, serde_json::json!({"unread": {"account": account}})) {
            InboxViewReply::UnreadCount(n) => n,
            other => panic!("expected unread count, got {other:?}"),
        }
    }

    #[test]
    fn deliveries_page_per_account_and_track_unread() {
        let mut map = Map::new();
        fold_change(&mut map, 1, &change(1, ALICE, "m1"));
        fold_change(&mut map, 2, &change(2, ALICE, "m2"));
        fold_change(&mut map, 3, &change(3, BOB, "m3"));

        let rows = items(&map, serde_json::json!({"list": {"account": ALICE}}));
        assert_eq!(rows.len(), 2);
        assert_eq!((rows[0].seq, rows[1].seq), (1, 2));
        // the row carries the canonical reference, never the detail.
        assert_eq!(rows[0].change, change(1, ALICE, "m1").reference());
        assert_eq!(rows[0].height, 1);
        assert_eq!(rows[0].created_at, 1_001);
        assert_eq!(unread(&map, ALICE), 2);
        assert_eq!(unread(&map, BOB), 1);

        // from_seq starts the page mid-queue.
        let rows = items(
            &map,
            serde_json::json!({"list": {"account": ALICE, "from_seq": 2}}),
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].seq, 2);
    }

    #[test]
    fn duplicate_and_ignored_deliveries_fold_nothing() {
        let mut map = Map::new();
        fold_change(&mut map, 1, &change(1, ALICE, "m1"));
        let before = map.clone();
        for stamp in [InboxAssigned::Duplicate, InboxAssigned::Ignored] {
            let row = op(
                2,
                OriginTag::module(ATTRIBUTION),
                encode_event(&AttributionEvent::Changed(change(1, ALICE, "m1"))),
                encode_assigned(&stamp),
            );
            let writes = fold_op(&row, &map, ATTRIBUTION).expect("fold");
            apply_to_map(&mut map, writes);
            assert_eq!(map, before, "{stamp:?} leaves the read model untouched");
        }
    }

    #[test]
    fn only_the_attribution_origin_folds_as_a_delivery() {
        let map = Map::new();
        // the same event bytes from another module are not a delivery: they
        // are read as an admin op, and do not decode as one.
        let row = op(
            1,
            OriginTag::module("chat"),
            encode_event(&AttributionEvent::Changed(change(1, ALICE, "m1"))),
            Vec::new(),
        );
        assert!(fold_op(&row, &map, ATTRIBUTION).is_err());
        // and a delivery without its stamp is interface drift, not a guess.
        let row = op(
            1,
            OriginTag::module(ATTRIBUTION),
            encode_event(&AttributionEvent::Changed(change(1, ALICE, "m1"))),
            Vec::new(),
        );
        assert!(fold_op(&row, &map, ATTRIBUTION).is_err());
    }

    #[test]
    fn mark_read_and_clear_mirror_module_semantics() {
        let mut map = Map::new();
        for i in 1..=3 {
            fold_change(&mut map, i, &change(i, ALICE, &format!("m{i}")));
        }
        fold_admin_op(
            &mut map,
            4,
            &InboxMsg::MarkRead {
                account: ALICE,
                up_to_seq: 2,
            },
        );
        assert_eq!(unread(&map, ALICE), 1);
        let rows = items(&map, serde_json::json!({"list": {"account": ALICE}}));
        assert_eq!(
            rows.iter().map(|r| r.read).collect::<Vec<_>>(),
            vec![true, true, false]
        );

        // idempotent re-mark changes nothing.
        fold_admin_op(
            &mut map,
            5,
            &InboxMsg::MarkRead {
                account: ALICE,
                up_to_seq: 2,
            },
        );
        assert_eq!(unread(&map, ALICE), 1);

        fold_admin_op(
            &mut map,
            6,
            &InboxMsg::Clear {
                account: ALICE,
                up_to_seq: 2,
            },
        );
        let rows = items(&map, serde_json::json!({"list": {"account": ALICE}}));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].seq, 3);
        assert_eq!(unread(&map, ALICE), 1, "the unread survivor stays counted");

        // a new delivery continues the seq space — Clear never rewinds it.
        fold_change(&mut map, 7, &change(4, ALICE, "m4"));
        let rows = items(&map, serde_json::json!({"list": {"account": ALICE}}));
        assert_eq!(rows.last().map(|r| r.seq), Some(4));

        // clearing everything keeps the watermark: the next delivery arrives
        // unread all the same.
        fold_admin_op(
            &mut map,
            8,
            &InboxMsg::Clear {
                account: ALICE,
                up_to_seq: u64::MAX,
            },
        );
        assert!(items(&map, serde_json::json!({"list": {"account": ALICE}})).is_empty());
        assert_eq!(unread(&map, ALICE), 0);
        fold_change(&mut map, 9, &change(5, ALICE, "m5"));
        let rows = items(&map, serde_json::json!({"list": {"account": ALICE}}));
        assert_eq!(
            rows.iter().map(|r| (r.seq, r.read)).collect::<Vec<_>>(),
            vec![(5, false)]
        );
        assert_eq!(unread(&map, ALICE), 1);
    }

    #[test]
    fn list_limit_defaults_and_clamps() {
        let mut map = Map::new();
        for i in 1..=60 {
            fold_change(&mut map, i, &change(i, ALICE, &format!("m{i}")));
        }

        // no limit: the default page, from the low end.
        let rows = items(&map, serde_json::json!({"list": {"account": ALICE}}));
        assert_eq!(rows.len(), DEFAULT_LIST_LIMIT);
        assert_eq!(
            (rows[0].seq, rows[DEFAULT_LIST_LIMIT - 1].seq),
            (1, DEFAULT_LIST_LIMIT as u64)
        );

        // an explicit limit bounds the page, composed with from_seq.
        let rows = items(
            &map,
            serde_json::json!({"list": {"account": ALICE, "from_seq": 5, "limit": 2}}),
        );
        assert_eq!(rows.iter().map(|r| r.seq).collect::<Vec<_>>(), vec![5, 6]);

        // limit 0 clamps up to one row; an absurd limit clamps down to
        // MAX_LIST_LIMIT — which still covers all sixty rows.
        let rows = items(
            &map,
            serde_json::json!({"list": {"account": ALICE, "limit": 0}}),
        );
        assert_eq!(rows.len(), 1);
        let rows = items(
            &map,
            serde_json::json!({"list": {"account": ALICE, "limit": 100_000}}),
        );
        assert_eq!(rows.len(), 60, "clamped limit still covers every row");
    }

    /// the overflow mirror of the module's [`MAX_ITEMS_PER_ACCOUNT`] cap: a
    /// delivery past the cap drops the account's OLDEST row, with the unread
    /// count following the dropped row's read status.
    #[test]
    fn overflow_drops_oldest_row_like_the_module() {
        let cap = MAX_ITEMS_PER_ACCOUNT as u64;
        let mut map = Map::new();
        // one over the per-account cap, so exactly one drop fires.
        for i in 1..=cap + 1 {
            fold_change(&mut map, i, &change(i, ALICE, "m"));
        }
        assert_eq!(
            unread(&map, ALICE),
            cap,
            "the dropped row was unread: unread sits at the cap, not cap+1"
        );
        let rows = items(
            &map,
            serde_json::json!({"list": {"account": ALICE, "limit": 1}}),
        );
        assert_eq!(rows[0].seq, 2, "seq 1 (the oldest) was dropped");
        let rows = items(
            &map,
            serde_json::json!({"list": {"account": ALICE, "from_seq": cap + 1}}),
        );
        assert_eq!(
            rows.iter().map(|r| r.seq).collect::<Vec<_>>(),
            vec![cap + 1],
            "the newest row survives"
        );

        // dropping a READ oldest must not decrement unread: mark seq 2 read,
        // overflow again — the victim is read, so the new arrival counts in
        // full.
        fold_admin_op(
            &mut map,
            cap + 2,
            &InboxMsg::MarkRead {
                account: ALICE,
                up_to_seq: 2,
            },
        );
        assert_eq!(unread(&map, ALICE), cap - 1);
        fold_change(&mut map, cap + 3, &change(cap + 2, ALICE, "m"));
        assert_eq!(
            unread(&map, ALICE),
            cap,
            "a read drop victim leaves unread to the new arrival"
        );
        let rows = items(
            &map,
            serde_json::json!({"list": {"account": ALICE, "limit": 1}}),
        );
        assert_eq!(
            (rows[0].seq, rows[0].read),
            (3, false),
            "seq 2 (read) was the drop victim"
        );
    }

    #[test]
    fn accounts_do_not_bleed_scans() {
        let mut map = Map::new();
        fold_change(&mut map, 1, &change(1, 1, "for one"));
        fold_change(&mut map, 2, &change(2, 11, "for eleven"));

        let rows = items(&map, serde_json::json!({"list": {"account": 1}}));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].change.source.object, "for one");
        let rows = items(&map, serde_json::json!({"list": {"account": 11}}));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].change.source.object, "for eleven");
    }

    /// mirrors the module's own clamp: `MarkRead { up_to_seq: u64::MAX }`
    /// must not mark a FUTURE delivery pre-read in the derived view either.
    #[test]
    fn mark_read_beyond_the_last_seq_does_not_pre_read_future_deliveries() {
        let mut map = Map::new();
        fold_change(&mut map, 1, &change(1, ALICE, "m1"));
        fold_admin_op(
            &mut map,
            2,
            &InboxMsg::MarkRead {
                account: ALICE,
                up_to_seq: u64::MAX,
            },
        );
        fold_change(&mut map, 3, &change(2, ALICE, "m2"));

        let rows = items(&map, serde_json::json!({"list": {"account": ALICE}}));
        assert_eq!(
            rows.iter().map(|r| (r.seq, r.read)).collect::<Vec<_>>(),
            vec![(1, true), (2, false)],
            "seq 2 was delivered AFTER the mark-read and must not be pre-read"
        );
        assert_eq!(unread(&map, ALICE), 1);
    }
}
