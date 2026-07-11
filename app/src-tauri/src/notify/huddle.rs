//! Observed-roster tracking for huddle notification matching.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    decode::{self, OpRow, OriginKind},
    matchers::{chat_notification, display_name, is_me, Category, MatcherCtx, Notification},
};

#[derive(Debug, Default)]
pub struct HuddleTracker(BTreeMap<String, BTreeSet<String>>);

#[derive(Debug, Default)]
pub struct MatchState {
    pub huddles: HuddleTracker,
}

pub(super) fn match_huddle_join(
    join: &serde_json::Value,
    op: &OpRow,
    ctx: &MatcherCtx<'_>,
    tracker: &mut HuddleTracker,
) -> Option<Notification> {
    if op.origin.kind != OriginKind::External {
        return None;
    }
    let joiner = op.origin.id.as_deref()?;
    let channel = join.get("channel_id")?.as_str()?;
    let roster = tracker.0.entry(channel.to_string()).or_default();
    let was_empty = roster.is_empty();
    roster.insert(joiner.to_ascii_lowercase());
    let node = decode::bytes_hex(join.get("node")?)?;

    if !was_empty || is_me(ctx, joiner) || is_me(ctx, &node) {
        return None;
    }

    Some(chat_notification(
        Category::Huddle,
        format!("Huddle started in #{channel}"),
        format!("{} started a huddle", display_name(ctx, joiner)),
        channel,
    ))
}

pub(super) fn track_huddle_leave(
    leave: &serde_json::Value,
    op: &OpRow,
    tracker: &mut HuddleTracker,
) {
    let Some(channel) = leave.get("channel_id").and_then(serde_json::Value::as_str) else {
        return;
    };
    let Some(leaver) = op.origin.id.as_deref() else {
        return;
    };
    if let Some(roster) = tracker.0.get_mut(channel) {
        roster.remove(&leaver.to_ascii_lowercase());
    }
}

pub(super) fn track_huddle_sweep(sweep: &serde_json::Value, tracker: &mut HuddleTracker) {
    let Some(channel) = sweep.get("channel_id").and_then(serde_json::Value::as_str) else {
        return;
    };
    let Some(user) = sweep.get("user").and_then(decode::bytes_hex) else {
        return;
    };
    if let Some(roster) = tracker.0.get_mut(channel) {
        roster.remove(&user);
    }
}
