use super::*;

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct BellData {
    pub unread: i64,
    pub items: Vec<BellItem>,
}

/// Load the current account's attribution notifications and unread count.
pub async fn load_bell(rpc: String) -> Result<BellData, AppError> {
    async {
        let rpc = rpc_client(&rpc)?;
        let Some(account) = local_account(&rpc).await? else {
            return Ok(BellData {
                unread: 0,
                items: Vec::new(),
            });
        };
        let listed: inbox::index::InboxViewReply = rpc
            .view(
                "inbox",
                &inbox::index::InboxViewQuery::List {
                    account,
                    from_seq: 0,
                    limit: Some(50),
                },
            )
            .await?;
        let inbox::index::InboxViewReply::Items(rows) = listed else {
            return Err("the inbox returned the wrong list reply".to_string());
        };
        let unread: inbox::index::InboxViewReply = rpc
            .view("inbox", &inbox::index::InboxViewQuery::Unread { account })
            .await?;
        let inbox::index::InboxViewReply::UnreadCount(unread) = unread else {
            return Err("the inbox returned the wrong unread reply".to_string());
        };
        let items = rows
            .iter()
            .rev()
            .map(inbox::client::bell_item_from_row)
            .collect();
        Ok(BellData {
            unread: i64::try_from(unread).unwrap_or(i64::MAX),
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
        let rpc = rpc_client(&rpc)?;
        let account = local_account(&rpc)
            .await?
            .ok_or_else(|| "this key is on no account".to_string())?;
        let up_to_seq = u64::try_from(up_to_seq).unwrap_or(0);
        signed_write(
            &rpc,
            "inbox",
            inbox::encode_msg(&inbox::InboxMsg::MarkRead { account, up_to_seq }),
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

/// The relation reason as words, including source-defined reason names.
pub fn bell_title(kind: &str) -> String {
    let words = kind.replace('_', " ");
    let mut chars = words.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => words,
    }
}

/// Source-defined reason names may carry a severity; unclassified reasons
/// stay informational. This does not infer severity from opaque source detail.
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

/// The canonical change and its authenticated actor, shown below the reason.
pub fn bell_detail(item: &BellItem) -> String {
    let change = item.kind.replace('_', " ");
    format!("{change} · {}", item.actor)
}

/// The worst severity among the UNREAD rows, for the bell badge's tint —
/// `info` when nothing is unread.
pub fn bell_worst_severity(items: &[BellItem]) -> String {
    let severities: Vec<String> = items
        .iter()
        .filter(|item| !item.read)
        .map(|item| bell_severity(&item.reason))
        .collect();
    let any_error = severities.iter().any(|severity| severity == "danger");
    let any_warning = severities.iter().any(|severity| severity == "warning");
    match (any_error, any_warning) {
        (true, _) => "danger".into(),
        (false, true) => "warning".into(),
        (false, false) => "info".into(),
    }
}
