use super::*;

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct BellData {
    pub unread: i64,
    pub items: Vec<BellItem>,
}

/// Load the bell: this member's notification page (newest first) + unread
/// count from the inbox views. A device without a user key has no inbox.
pub async fn load_bell(rpc: String) -> Result<BellData, AppError> {
    async {
        let Some(member) = local_member().await else {
            return Ok(BellData {
                unread: 0,
                items: Vec::new(),
            });
        };
        let rpc = rpc_client(&rpc)?;
        let listed: serde_json::Value = rpc
            .view(
                "inbox",
                &serde_json::json!({ "list": { "member": member, "from_seq": 0, "limit": 50 } }),
            )
            .await?;
        let unread: serde_json::Value = rpc
            .view(
                "inbox",
                &serde_json::json!({ "unread": { "member": member } }),
            )
            .await?;
        let mut items: Vec<BellItem> = listed["items"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|row| BellItem {
                seq: row["seq"].as_i64().unwrap_or(0),
                kind: row["kind"].as_str().unwrap_or_default().to_string(),
                body: row["body"].as_str().unwrap_or_default().to_string(),
                source: row["source"].as_str().unwrap_or_default().to_string(),
                height: row["height"].as_i64().unwrap_or(0),
                read: row["read"].as_bool().unwrap_or(false),
            })
            .collect();
        items.reverse();
        Ok(BellData {
            unread: unread["unread_count"].as_i64().unwrap_or(0),
            items,
        })
    }
    .await
    .map_err(app_error)
}

/// Mark everything at or below `up_to_seq` read (signed by the local user).
pub async fn mark_bell_read(
    rpc: String,
    password: String,
    up_to_seq: i64,
) -> Result<bool, AppError> {
    async {
        if up_to_seq <= 0 {
            return Ok(());
        }
        let member = local_member()
            .await
            .ok_or_else(|| "no local user key".to_string())?;
        let up_to_seq = u64::try_from(up_to_seq).unwrap_or(0);
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "inbox",
            inbox::encode_msg(&inbox::InboxMsg::MarkRead { member, up_to_seq }),
            password,
        )
        .await
        .map(|_height| ())
    }
    .await
    .map_err(app_error)?;
    Ok(true)
}

/// The delta-fold splices, re-exported shapes the Ice layer applies.
pub fn apply_bell(items: Vec<BellItem>, delta: BellDelta) -> Vec<BellItem> {
    fold_bell_items(items, delta)
}

/// The unread count after one bell delta.
pub fn bell_unread_after(unread: i64, items: Vec<BellItem>, delta: BellDelta) -> i64 {
    inbox::client::apply_bell_unread(unread, &items, &delta)
}

/// The mark-read watermark of the current list.
pub fn bell_head(items: Vec<BellItem>) -> i64 {
    inbox::client::bell_head_seq(&items)
}

/// One notification's severity — `info` | `warn` | `error` — for the row dot,
/// the INFO/WARN/ALERT chip and the badge tint.
///
/// THE WIRE CARRIES NO SEVERITY. `Notification` is seq/member/kind/body/source/
/// created_at (crates/modules/apps/inbox/src/interface.rs; `read` is derived
/// from the member's read watermark, not a per-item field), so this is a
/// PROJECTION of the delivering module's `kind` token, not a field anything
/// signed. A kind this mapping does not name reads `info`: an unclassified
/// notice is a notice, never an alarm.
/// The row's title: the wire `kind` token said as words. The vocabulary is
/// open — any module mints tokens — so this is a rendering, not a registry:
/// `review_requested` reads "Review requested".
pub fn bell_title(kind: &str) -> String {
    let words = kind.replace('_', " ");
    let mut chars = words.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => words,
    }
}

pub fn bell_severity(kind: &str) -> String {
    const WARN: &[&str] = &[
        "review_requested",
        "changes_requested",
        "proposal_opened",
        "vote_needed",
        "run_cancelled",
        "quota",
    ];
    const ERROR: &[&str] = &["failed", "error", "rejected", "conflict", "revoked"];
    let kind = kind.to_lowercase();
    let names_error = ERROR.iter().any(|token| kind.contains(token));
    let names_warning = WARN.iter().any(|token| kind.contains(token));
    // These three strings ARE the tone vocabulary `PulseDot`, `StillDot` and
    // `BellBadge` match on. They used to be `error`/`warn`, which no arm of
    // `BellBadge` carried, so a failed run painted the badge info-blue through
    // the fallthrough. One name per severity, spoken everywhere.
    match (names_error, names_warning) {
        (true, _) => "danger".into(),
        (false, true) => "warning".into(),
        (false, false) => "info".into(),
    }
}

/// The worst severity among the UNREAD rows, for the bell badge's tint —
/// `info` when nothing is unread.
pub fn bell_worst_severity(items: &[BellItem]) -> String {
    let severities: Vec<String> = items
        .iter()
        .filter(|item| !item.read)
        .map(|item| bell_severity(&item.kind))
        .collect();
    let any_error = severities.iter().any(|severity| severity == "danger");
    let any_warning = severities.iter().any(|severity| severity == "warning");
    match (any_error, any_warning) {
        (true, _) => "danger".into(),
        (false, true) => "warning".into(),
        (false, false) => "info".into(),
    }
}
