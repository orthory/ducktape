//! inbox's CLIENT view model — the rendered bell item and the op-delta fold
//! a feed-following UI splices its notification state with. module-owned
//! beside `index.rs` (same feed, same vocabulary), pure data-in/data-out,
//! ui.wasm-portable like `chat::client`.

use attribution::{Actor, AttributionEvent, ChangeKind, ChangeRef, Reason, Source, decode_event};

use crate::index::NotificationRow;
use crate::{AccountNumber, InboxAssigned, InboxMsg, decode_msg};

/// One rendered bell notification: the canonical change, rendered.
#[derive(Clone, Debug, Hash, PartialEq, Default)]
pub struct BellItem {
    /// the inbox's own per-account sequence.
    pub seq: i64,
    /// the attribution plane's canonical change sequence — the reference a
    /// client follows for the full record.
    pub change_seq: i64,
    /// the source object, rendered `module/kind/object`.
    pub source: String,
    /// the relation's reason, rendered (`mention`, `authorship`, … or a
    /// source-defined name).
    pub reason: String,
    /// the change kind, rendered (`added`, `withdrawn`, `transferred_in`,
    /// `transferred_out`).
    pub kind: String,
    /// the actor the source vouched for, rendered (`account:7`, `key:…`,
    /// `module:chat`, `system`).
    pub actor: String,
    pub height: i64,
    pub read: bool,
}

/// One folded inbox op, scoped to ONE account (the local user).
#[derive(Clone, Debug, Hash, PartialEq, Default)]
pub struct BellDelta {
    /// `delivered` | `read` | `cleared` — empty when the op targets another
    /// account (invisible to this bell).
    pub kind: String,
    pub item: BellItem,
    /// `read`/`cleared`: everything at or below this seq is affected.
    pub up_to_seq: i64,
}

pub fn render_source(source: &Source) -> String {
    format!("{}/{}/{}", source.module, source.kind, source.object)
}

pub fn render_reason(reason: &Reason) -> String {
    match reason {
        Reason::Mention => "mention".into(),
        Reason::Authorship => "authorship".into(),
        Reason::Ownership => "ownership".into(),
        Reason::Assignment => "assignment".into(),
        Reason::Credit => "credit".into(),
        Reason::Result => "result".into(),
        Reason::Report => "report".into(),
        Reason::Defined(name) => name.clone(),
    }
}

pub fn render_kind(kind: &ChangeKind) -> String {
    match kind {
        ChangeKind::Added => "added".into(),
        ChangeKind::Withdrawn => "withdrawn".into(),
        ChangeKind::TransferredIn { from } => format!("transferred_in:{from}"),
        ChangeKind::TransferredOut { to } => format!("transferred_out:{to}"),
    }
}

pub fn render_actor(actor: &Actor) -> String {
    match actor {
        Actor::Account(account) => format!("account:{account}"),
        Actor::Key(key) => {
            let mut out = String::with_capacity(4 + key.len() * 2);
            out.push_str("key:");
            for byte in key {
                out.push_str(&format!("{byte:02x}"));
            }
            out
        }
        Actor::Module(module) => format!("module:{module}"),
        Actor::System => "system".into(),
    }
}

fn bell_item(seq: u64, change: &ChangeRef, height: u64, read: bool) -> BellItem {
    BellItem {
        seq: i64::try_from(seq).unwrap_or(i64::MAX),
        change_seq: i64::try_from(change.seq).unwrap_or(i64::MAX),
        source: render_source(&change.source),
        reason: render_reason(&change.reason),
        kind: render_kind(&change.kind),
        actor: render_actor(&change.actor),
        height: i64::try_from(height).unwrap_or(i64::MAX),
        read,
    }
}

/// One index row, rendered the way the delta fold renders a delivery — so a
/// loaded page and the deltas that follow it agree byte for byte.
pub fn bell_item_from_row(row: &NotificationRow) -> BellItem {
    bell_item(row.seq, &row.change, row.height, row.read)
}

/// Translate one applied inbox op into this account's bell delta. `Ok(None)`
/// = another account's traffic, or a delivery the module stamped as changing
/// nothing. `Err` = undecodable — the caller reloads. `attribution` is the
/// attribution module's id: the origin whose ops are deliveries.
pub fn delta_from_op(
    payload: &[u8],
    assigned: Option<&serde_json::Value>,
    origin_kind: &str,
    origin_id: Option<&str>,
    account: AccountNumber,
    attribution: &str,
) -> Result<Option<BellDelta>, String> {
    let from_attribution = origin_kind == "module" && origin_id == Some(attribution);
    if from_attribution {
        return delivery_delta(payload, assigned, account);
    }
    let delta = match decode_msg(payload)? {
        InboxMsg::MarkRead {
            account: target,
            up_to_seq,
        } => {
            if target != account {
                return Ok(None);
            }
            BellDelta {
                kind: "read".into(),
                item: BellItem::default(),
                up_to_seq: i64::try_from(up_to_seq).unwrap_or(i64::MAX),
            }
        }
        InboxMsg::Clear {
            account: target,
            up_to_seq,
        } => {
            if target != account {
                return Ok(None);
            }
            BellDelta {
                kind: "cleared".into(),
                item: BellItem::default(),
                up_to_seq: i64::try_from(up_to_seq).unwrap_or(i64::MAX),
            }
        }
    };
    Ok(Some(delta))
}

fn delivery_delta(
    payload: &[u8],
    assigned: Option<&serde_json::Value>,
    account: AccountNumber,
) -> Result<Option<BellDelta>, String> {
    let AttributionEvent::Changed(change) = decode_event(payload)?;
    if change.recipient != account {
        return Ok(None);
    }
    let value = assigned.ok_or("applied delivery carried no stamp")?;
    let stamp: InboxAssigned = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
    let seq = match stamp {
        InboxAssigned::Delivered { seq } => seq,
        InboxAssigned::Duplicate | InboxAssigned::Ignored => return Ok(None),
    };
    Ok(Some(BellDelta {
        kind: "delivered".into(),
        item: bell_item(seq, &change.reference(), change.height, false),
        up_to_seq: 0,
    }))
}

/// Fold one bell delta into the newest-first item list (idempotent by seq).
pub fn apply_bell_items(mut items: Vec<BellItem>, delta: BellDelta) -> Vec<BellItem> {
    match delta.kind.as_str() {
        "delivered" => {
            if items.iter().any(|item| item.seq == delta.item.seq) {
                return items;
            }
            items.insert(0, delta.item);
            items.truncate(50);
        }
        "read" => {
            for item in &mut items {
                if item.seq <= delta.up_to_seq {
                    item.read = true;
                }
            }
        }
        "cleared" => {
            items.retain(|item| item.seq > delta.up_to_seq);
        }
        _ => {}
    }
    items
}

/// Fold one bell delta into the unread count.
pub fn apply_bell_unread(unread: i64, items_before: &[BellItem], delta: &BellDelta) -> i64 {
    match delta.kind.as_str() {
        "delivered" => unread.saturating_add(1),
        "read" | "cleared" => {
            let still_unread = items_before
                .iter()
                .filter(|item| !item.read && item.seq > delta.up_to_seq)
                .count();
            i64::try_from(still_unread).unwrap_or(i64::MAX)
        }
        _ => unread,
    }
}

/// The newest seq in the list (the mark-read watermark), 0 when empty.
pub fn bell_head_seq(items: &[BellItem]) -> i64 {
    items.iter().map(|item| item.seq).max().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode_msg;
    use attribution::{Change, encode_event};

    const ALICE: AccountNumber = 7;
    const BOB: AccountNumber = 9;

    fn changed(recipient: AccountNumber) -> Vec<u8> {
        encode_event(&AttributionEvent::Changed(Change {
            seq: 12,
            source: Source {
                module: "chat".into(),
                kind: "message".into(),
                object: "m1".into(),
            },
            revision: 1,
            recipient,
            reason: Reason::Mention,
            kind: ChangeKind::Added,
            detail: Vec::new(),
            actor: Actor::Account(BOB),
            cause: sdk::Cause::Direct,
            height: 4,
        }))
    }

    #[test]
    fn bell_folds_only_this_accounts_deliveries() {
        let stamp = serde_json::json!({"delivered": {"seq": 3}});
        let mine = delta_from_op(
            &changed(ALICE),
            Some(&stamp),
            "module",
            Some("attribution"),
            ALICE,
            "attribution",
        )
        .unwrap()
        .expect("my delivery folds");
        assert_eq!(mine.kind, "delivered");
        assert_eq!(
            mine.item,
            BellItem {
                seq: 3,
                change_seq: 12,
                source: "chat/message/m1".into(),
                reason: "mention".into(),
                kind: "added".into(),
                actor: "account:9".into(),
                height: 4,
                read: false,
            }
        );

        let theirs = delta_from_op(
            &changed(BOB),
            Some(&stamp),
            "module",
            Some("attribution"),
            ALICE,
            "attribution",
        )
        .unwrap();
        assert!(theirs.is_none());

        // a delivery that changed nothing is invisible to the bell.
        for stamp in [serde_json::json!("duplicate"), serde_json::json!("ignored")] {
            let none = delta_from_op(
                &changed(ALICE),
                Some(&stamp),
                "module",
                Some("attribution"),
                ALICE,
                "attribution",
            )
            .unwrap();
            assert!(none.is_none());
        }

        let items = apply_bell_items(Vec::new(), mine.clone());
        assert_eq!(items.len(), 1);
        assert_eq!(apply_bell_unread(0, &[], &mine), 1);

        // marking read at the head zeroes the unread count
        let read = delta_from_op(
            &encode_msg(&InboxMsg::MarkRead {
                account: ALICE,
                up_to_seq: 3,
            }),
            None,
            "external",
            Some("alice"),
            ALICE,
            "attribution",
        )
        .unwrap()
        .expect("my ack folds");
        assert_eq!(read.kind, "read");
        let after = apply_bell_items(items.clone(), read.clone());
        assert!(after[0].read);
        assert_eq!(apply_bell_unread(1, &items, &read), 0);
    }

    #[test]
    fn a_delivery_from_another_origin_is_not_a_delivery() {
        // the same bytes from chat are read as an admin op and fail to
        // decode — the bell never renders a forged delivery.
        let stamp = serde_json::json!({"delivered": {"seq": 3}});
        assert!(
            delta_from_op(
                &changed(ALICE),
                Some(&stamp),
                "module",
                Some("chat"),
                ALICE,
                "attribution"
            )
            .is_err()
        );
    }

    #[test]
    fn missing_stamp_is_an_error_not_a_guess() {
        assert!(
            delta_from_op(
                &changed(ALICE),
                None,
                "module",
                Some("attribution"),
                ALICE,
                "attribution"
            )
            .is_err()
        );
    }

    #[test]
    fn a_row_renders_like_a_delta() {
        let stamp = serde_json::json!({"delivered": {"seq": 3}});
        let delta = delta_from_op(
            &changed(ALICE),
            Some(&stamp),
            "module",
            Some("attribution"),
            ALICE,
            "attribution",
        )
        .unwrap()
        .unwrap();
        let AttributionEvent::Changed(change) = decode_event(&changed(ALICE)).unwrap();
        let row = NotificationRow {
            seq: 3,
            account: ALICE,
            change: change.reference(),
            height: 4,
            created_at: 100,
            read: false,
        };
        assert_eq!(bell_item_from_row(&row), delta.item);
        assert_eq!(render_actor(&Actor::Key(vec![0xab, 0x01])), "key:ab01");
        assert_eq!(
            render_kind(&ChangeKind::TransferredIn { from: 3 }),
            "transferred_in:3"
        );
        assert_eq!(
            render_reason(&Reason::Defined("maintainer".into())),
            "maintainer"
        );
    }
}
