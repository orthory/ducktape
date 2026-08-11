//! inbox's CLIENT view model — the rendered bell item and the op-delta fold
//! a feed-following UI splices its notification state with. module-owned
//! beside `index.rs` (same feed, same vocabulary), pure data-in/data-out,
//! ui.wasm-portable like `chat::client`.

use crate::{InboxAssigned, InboxMsg, decode_msg};

/// One rendered bell notification.
#[derive(Clone, Debug, Hash, PartialEq, Default)]
pub struct BellItem {
    pub seq: i64,
    pub kind: String,
    pub body: String,
    /// the delivering origin, rendered like the index does.
    pub source: String,
    pub height: i64,
    pub read: bool,
}

/// One folded inbox op, scoped to ONE member (the local user).
#[derive(Clone, Debug, Hash, PartialEq, Default)]
pub struct BellDelta {
    /// `delivered` | `read` | `cleared` — empty when the op targets another
    /// member (invisible to this bell).
    pub kind: String,
    pub item: BellItem,
    /// `read`/`cleared`: everything at or below this seq is affected.
    pub up_to_seq: i64,
}

/// Translate one applied inbox op into this member's bell delta. `Ok(None)`
/// = another member's traffic. `Err` = undecodable — the caller reloads.
pub fn delta_from_op(
    payload: &[u8],
    assigned: Option<&serde_json::Value>,
    origin_kind: &str,
    origin_id: Option<&str>,
    member: &str,
) -> Result<Option<BellDelta>, String> {
    let msg = decode_msg(payload)?;
    let delta = match msg {
        InboxMsg::Deliver {
            member: target,
            kind,
            body,
        } => {
            if target != member {
                return Ok(None);
            }
            let value = assigned.ok_or("applied Deliver carried no stamp")?;
            let InboxAssigned::Delivered { seq } =
                serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
            BellDelta {
                kind: "delivered".into(),
                item: BellItem {
                    seq: i64::try_from(seq).unwrap_or(i64::MAX),
                    kind,
                    body,
                    source: render_source(origin_kind, origin_id),
                    height: 0,
                    read: false,
                },
                up_to_seq: 0,
            }
        }
        InboxMsg::MarkRead {
            member: target,
            up_to_seq,
        } => {
            if target != member {
                return Ok(None);
            }
            BellDelta {
                kind: "read".into(),
                item: BellItem::default(),
                up_to_seq: i64::try_from(up_to_seq).unwrap_or(i64::MAX),
            }
        }
        InboxMsg::Clear {
            member: target,
            up_to_seq,
        } => {
            if target != member {
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

fn render_source(kind: &str, id: Option<&str>) -> String {
    let id = id.unwrap_or_default();
    match kind {
        "module" => format!("module:{id}"),
        "external" => format!("user:{id}"),
        _ => "system".to_string(),
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode_msg;

    fn deliver(member: &str, body: &str) -> Vec<u8> {
        encode_msg(&InboxMsg::Deliver {
            member: member.into(),
            kind: "mention".into(),
            body: body.into(),
        })
    }

    #[test]
    fn bell_folds_only_this_members_traffic() {
        let stamp = serde_json::to_value(serde_json::json!({"delivered": {"seq": 3}})).unwrap();
        let mine = delta_from_op(
            &deliver("ext:ab", "ping"),
            Some(&stamp),
            "module",
            Some("automations"),
            "ext:ab",
        )
        .unwrap()
        .expect("my delivery folds");
        assert_eq!(mine.kind, "delivered");
        assert_eq!(mine.item.seq, 3);
        assert_eq!(mine.item.source, "module:automations");

        let theirs = delta_from_op(
            &deliver("ext:cd", "ping"),
            Some(&stamp),
            "module",
            Some("automations"),
            "ext:ab",
        )
        .unwrap();
        assert!(theirs.is_none());

        let items = apply_bell_items(Vec::new(), mine.clone());
        assert_eq!(items.len(), 1);
        assert_eq!(apply_bell_unread(0, &[], &mine), 1);

        // marking read at the head zeroes the unread count
        let read = BellDelta {
            kind: "read".into(),
            item: BellItem::default(),
            up_to_seq: 3,
        };
        let after = apply_bell_items(items.clone(), read.clone());
        assert!(after[0].read);
        assert_eq!(apply_bell_unread(1, &items, &read), 0);
    }

    #[test]
    fn missing_stamp_is_an_error_not_a_guess() {
        assert!(delta_from_op(&deliver("ext:ab", "x"), None, "system", None, "ext:ab").is_err());
    }
}
