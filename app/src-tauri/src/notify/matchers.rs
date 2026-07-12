use std::collections::BTreeMap;

use super::{
    decode::{self, OpRow, OriginKind},
    huddle,
};

pub use super::huddle::MatchState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Mention,
    Reply,
    Huddle,
    Run,
    Forge,
    Governance,
}

/// A matched desktop notification: title + body for the OS toast, plus the
/// channel for mute/focus filtering. There is deliberately no deep-link
/// target — see the note in [`super::present`].
#[derive(Debug, Clone, PartialEq)]
pub struct Notification {
    pub category: Category,
    pub title: String,
    pub body: String,
    /// Channel used for focus suppression and mute filtering. Runs, forge, and
    /// governance notifications have no channel and use `None`.
    pub channel_id: Option<String>,
}

pub struct MatcherCtx<'a> {
    /// My user key in hex, used to recognize direct mentions.
    pub self_user_key_hex: Option<&'a str>,
    /// Every node key bound to my user, in hex, used to recognize my own events.
    pub self_node_keys_hex: &'a [String],
    /// Known display names keyed by lowercase author key hex.
    pub author_names: &'a BTreeMap<String, String>,
    /// Looks up the author of a thread root. Tests inject a closure here; the
    /// stream module wires the real lookup.
    pub root_author: &'a dyn Fn(&str, u64) -> Option<String>,
}

pub fn match_topic(
    topic: &str,
    op: &OpRow,
    ctx: &MatcherCtx<'_>,
    state: &mut MatchState,
) -> Option<Notification> {
    match topic {
        "module:chat" => match_chat(op, ctx, state),
        "module:runs" => match_run(op),
        "module:forge" => match_forge(op),
        "module:governance" => match_governance(op),
        _ => None,
    }
}

fn match_chat(op: &OpRow, ctx: &MatcherCtx<'_>, state: &mut MatchState) -> Option<Notification> {
    let payload = op.payload.as_ref()?;

    if let Some(message) = decode::variant(payload, "post_message") {
        return match_message(message, op, ctx);
    }
    if let Some(join) = decode::variant(payload, "join_huddle") {
        return huddle::match_huddle_join(join, op, ctx, &mut state.huddles);
    }
    if let Some(leave) = decode::variant(payload, "leave_huddle") {
        huddle::track_huddle_leave(leave, op, &mut state.huddles);
    } else if let Some(sweep) = decode::variant(payload, "sweep_huddle") {
        huddle::track_huddle_sweep(sweep, &mut state.huddles);
    }

    None
}

fn match_message(
    message: &serde_json::Value,
    op: &OpRow,
    ctx: &MatcherCtx<'_>,
) -> Option<Notification> {
    if op.origin.kind != OriginKind::External {
        return None;
    }
    let author = op.origin.id.as_deref()?;
    if is_me(ctx, author) {
        return None;
    }

    let channel = message.get("channel_id")?.as_str()?;
    let blocks = message.get("blocks")?;
    blocks.as_array()?;
    let thread = match message.get("thread")? {
        serde_json::Value::Null => None,
        value => Some(value.as_u64()?),
    };
    let name = display_name(ctx, author);

    let mentioned = ctx.self_user_key_hex.is_some_and(|my_user| {
        decode::mention_user_hexes(blocks)
            .iter()
            .any(|user| user.eq_ignore_ascii_case(my_user))
    });
    if mentioned {
        return Some(chat_notification(
            Category::Mention,
            format!("{name} mentioned you in #{channel}"),
            decode::blocks_preview(blocks, 140),
            channel,
        ));
    }

    let root = thread?;
    let root_author = (ctx.root_author)(channel, root)?;
    if !is_me(ctx, &root_author) {
        return None;
    }

    Some(chat_notification(
        Category::Reply,
        format!("{name} replied to your thread in #{channel}"),
        decode::blocks_preview(blocks, 140),
        channel,
    ))
}

fn match_run(op: &OpRow) -> Option<Notification> {
    if op.origin.kind != OriginKind::Module || op.origin.id.as_deref() != Some("dispatch") {
        return None;
    }
    let payload = op.payload.as_ref()?.as_object()?;
    let dispatch_id = payload.get("dispatch_id")?.as_str()?;
    let outcome = payload.get("outcome")?.as_object()?;

    let (title, body) = if outcome.contains_key("Ok") {
        (
            "Agent run finished",
            format!("dispatch {}…", truncate(dispatch_id, 12)),
        )
    } else {
        (
            "Agent run failed",
            truncate(outcome.get("Err")?.as_str()?, 140),
        )
    };

    Some(Notification {
        category: Category::Run,
        title: title.to_string(),
        body,
        channel_id: None,
    })
}

fn match_forge(op: &OpRow) -> Option<Notification> {
    let merged = decode::variant(op.payload.as_ref()?, "merge_pr")?;
    let repo = match merged.get("repo")?.as_str()? {
        "" => "default",
        repo => repo,
    };
    let number = merged.get("number")?.as_u64()?;

    Some(Notification {
        category: Category::Forge,
        title: format!("PR #{number} merged in {repo}"),
        body: String::new(),
        channel_id: None,
    })
}

fn match_governance(op: &OpRow) -> Option<Notification> {
    let payload = op.payload.as_ref()?;
    if let Some(proposal) = decode::variant(payload, "propose") {
        let proposal_id = proposal.get("proposal_id")?.as_str()?;
        let action = proposal.get("action")?;
        let admission = ["add_validator", "add_resident"]
            .iter()
            .any(|variant| decode::variant(action, variant).is_some());
        if !admission {
            return None;
        }
        return Some(Notification {
            category: Category::Governance,
            title: "New admission proposal".to_string(),
            body: format!("proposal {proposal_id}"),
            channel_id: None,
        });
    }

    let redeem = decode::variant(payload, "redeem")?;
    let joiner = decode::bytes_hex(redeem.get("joiner")?)?;
    Some(Notification {
        category: Category::Governance,
        title: "New member admitted".to_string(),
        body: format!("{} joined via invite", short_hex(&joiner)),
        channel_id: None,
    })
}

pub(super) fn is_me(ctx: &MatcherCtx<'_>, hex: &str) -> bool {
    ctx.self_node_keys_hex
        .iter()
        .any(|key| key.eq_ignore_ascii_case(hex))
}

pub(super) fn display_name(ctx: &MatcherCtx<'_>, hex: &str) -> String {
    ctx.author_names
        .get(&hex.to_ascii_lowercase())
        .cloned()
        .unwrap_or_else(|| short_hex(hex))
}

fn short_hex(hex: &str) -> String {
    format!("{}…", truncate(hex, 8))
}

fn truncate(value: &str, max: usize) -> String {
    value.chars().take(max).collect()
}

pub(super) fn chat_notification(
    category: Category,
    title: String,
    body: String,
    channel: &str,
) -> Notification {
    Notification {
        category,
        title,
        body,
        channel_id: Some(channel.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::LazyLock};

    use serde_json::{json, Value};

    use super::*;
    use crate::notify::decode::Origin;

    static NODE_KEYS: LazyLock<Vec<String>> =
        LazyLock::new(|| vec!["aaaa".to_string(), "bbbb".to_string()]);
    static AUTHOR_NAMES: LazyLock<BTreeMap<String, String>> = LazyLock::new(|| {
        BTreeMap::from([
            ("cccc".to_string(), "Casey".to_string()),
            ("dddd".to_string(), "Devon".to_string()),
        ])
    });

    fn op(origin_kind: OriginKind, origin_id: &str, payload: Value) -> OpRow {
        OpRow {
            origin: Origin {
                kind: origin_kind,
                id: Some(origin_id.to_string()),
            },
            payload: Some(payload),
        }
    }

    fn ctx(root_author: &dyn Fn(&str, u64) -> Option<String>) -> MatcherCtx<'_> {
        MatcherCtx {
            self_user_key_hex: Some("1234"),
            self_node_keys_hex: &NODE_KEYS,
            author_names: &AUTHOR_NAMES,
            root_author,
        }
    }

    fn no_root(_: &str, _: u64) -> Option<String> {
        None
    }

    fn once(topic: &str, row: &OpRow, ctx: &MatcherCtx<'_>) -> Option<Notification> {
        match_topic(topic, row, ctx, &mut MatchState::default())
    }

    fn chat(row: &OpRow, ctx: &MatcherCtx<'_>, state: &mut MatchState) -> Option<Notification> {
        match_topic("module:chat", row, ctx, state)
    }

    fn message_payload(mention: Option<[u8; 2]>, thread: Option<u64>) -> Value {
        let marks = mention.map_or_else(Vec::new, |user| {
            vec![json!({ "mention": { "user": user } })]
        });
        json!({
            "post_message": {
                "channel_id": "general",
                "message_id": "m1",
                "blocks": [{
                    "paragraph": [{ "text": "hello", "marks": marks }]
                }],
                "thread": thread,
                "as_agent": null
            }
        })
    }

    fn with_root(row: &OpRow, author: Option<&str>) -> Option<Notification> {
        let roots: HashMap<(String, u64), String> = author
            .map(|author| (("general".to_string(), 7), author.to_string()))
            .into_iter()
            .collect();
        let root_author =
            |channel: &str, root: u64| roots.get(&(channel.to_string(), root)).cloned();
        once("module:chat", row, &ctx(&root_author))
    }

    fn join(origin: &str, node: [u8; 2], channel: &str) -> OpRow {
        op(
            OriginKind::External,
            origin,
            json!({ "join_huddle": { "channel_id": channel, "node": node } }),
        )
    }

    fn leave(origin: &str, channel: &str) -> OpRow {
        op(
            OriginKind::External,
            origin,
            json!({ "leave_huddle": { "channel_id": channel } }),
        )
    }

    #[test]
    fn matches_mentions_from_other_people_only() {
        let ctx = ctx(&no_root);
        let mention = op(
            OriginKind::External,
            "cccc",
            message_payload(Some([18, 52]), None),
        );
        let notification = once("module:chat", &mention, &ctx).unwrap();
        assert_eq!(notification.category, Category::Mention);
        assert_eq!(notification.title, "Casey mentioned you in #general");
        assert_eq!(notification.body, "hello");

        let own = op(
            OriginKind::External,
            "AAAA",
            mention.payload.clone().unwrap(),
        );
        assert!(once("module:chat", &own, &ctx).is_none());
        let other = op(
            OriginKind::External,
            "cccc",
            message_payload(Some([86, 120]), None),
        );
        assert!(once("module:chat", &other, &ctx).is_none());
    }

    #[test]
    fn matches_replies_to_my_roots_and_prioritizes_mentions() {
        let reply = op(OriginKind::External, "cccc", message_payload(None, Some(7)));
        let notification = with_root(&reply, Some("aaaa")).unwrap();
        assert_eq!(notification.category, Category::Reply);
        assert_eq!(
            notification.title,
            "Casey replied to your thread in #general"
        );
        assert!(with_root(&reply, Some("cccc")).is_none());
        assert!(with_root(&reply, None).is_none());

        let both = op(
            OriginKind::External,
            "cccc",
            message_payload(Some([18, 52]), Some(7)),
        );
        assert_eq!(
            with_root(&both, Some("aaaa")).unwrap().category,
            Category::Mention
        );
    }

    #[test]
    fn tracks_huddle_rosters_and_renotifies_after_they_empty() {
        let ctx = ctx(&no_root);
        let mut state = MatchState::default();

        let first = chat(&join("cccc", [204, 204], "team"), &ctx, &mut state).unwrap();
        assert_eq!(first.title, "Huddle started in #team");
        assert!(chat(&join("dddd", [221, 221], "team"), &ctx, &mut state).is_none());
        for leaver in ["cccc", "dddd"] {
            assert!(chat(&leave(leaver, "team"), &ctx, &mut state).is_none());
        }
        assert!(chat(&join("eeee", [238, 238], "team"), &ctx, &mut state).is_some());

        let sweep = op(
            OriginKind::System,
            "system",
            json!({ "sweep_huddle": { "channel_id": "team", "user": [238, 238] } }),
        );
        assert!(chat(&sweep, &ctx, &mut state).is_none());
        assert!(chat(&join("ffff", [255, 255], "team"), &ctx, &mut state).is_some());

        for (origin, node, channel) in [
            ("cccc", [170, 170], "node-self"),
            ("AAAA", [204, 204], "origin-self"),
        ] {
            let mut state = MatchState::default();
            assert!(chat(&join(origin, node, channel), &ctx, &mut state).is_none());
            assert!(chat(&join("dddd", [221, 221], channel), &ctx, &mut state).is_none());
        }
    }

    #[test]
    fn malformed_huddle_node_still_marks_the_roster_non_empty() {
        let ctx = ctx(&no_root);
        let mut state = MatchState::default();
        let malformed = op(
            OriginKind::External,
            "cccc",
            json!({ "join_huddle": { "channel_id": "team", "node": "not-bytes" } }),
        );

        assert!(chat(&malformed, &ctx, &mut state).is_none());
        assert!(chat(&join("dddd", [221, 221], "team"), &ctx, &mut state).is_none());
    }

    #[test]
    fn matches_dispatch_run_results_only() {
        let ctx = ctx(&no_root);
        let ok = op(
            OriginKind::Module,
            "dispatch",
            json!({ "dispatch_id": "d1", "recipe_id": "r", "outcome": { "Ok": [1] } }),
        );
        let notification = once("module:runs", &ok, &ctx).unwrap();
        assert_eq!(notification.category, Category::Run);
        assert_eq!(notification.title, "Agent run finished");
        assert_eq!(notification.body, "dispatch d1…");

        let failed = op(
            OriginKind::Module,
            "dispatch",
            json!({ "dispatch_id": "d2", "recipe_id": "r", "outcome": { "Err": "boom" } }),
        );
        let notification = once("module:runs", &failed, &ctx).unwrap();
        assert_eq!(notification.title, "Agent run failed");
        assert_eq!(notification.body, "boom");

        let wrong_origin = op(OriginKind::Module, "runs", ok.payload.clone().unwrap());
        assert!(once("module:runs", &wrong_origin, &ctx).is_none());
        let watch = op(
            OriginKind::Module,
            "dispatch",
            json!({ "watch_channel": { "channel_id": "general" } }),
        );
        assert!(once("module:runs", &watch, &ctx).is_none());
    }

    #[test]
    fn matches_merged_pull_requests_and_normalizes_default_repo() {
        let ctx = ctx(&no_root);
        let merged = op(
            OriginKind::External,
            "aaaa",
            json!({ "merge_pr": { "repo": "", "number": 7 } }),
        );
        let notification = once("module:forge", &merged, &ctx).unwrap();
        assert_eq!(notification.category, Category::Forge);
        assert_eq!(notification.title, "PR #7 merged in default");

        let opened = op(
            OriginKind::External,
            "cccc",
            json!({ "open_pr": { "repo": "default", "number": 8 } }),
        );
        assert!(once("module:forge", &opened, &ctx).is_none());
    }

    #[test]
    fn matches_admission_governance_events_only() {
        let ctx = ctx(&no_root);
        let admission = op(
            OriginKind::External,
            "cccc",
            json!({
                "propose": {
                    "proposal_id": "p1",
                    "action": { "add_resident": {} },
                    "voting_period": 10
                }
            }),
        );
        let notification = once("module:governance", &admission, &ctx).unwrap();
        assert_eq!(notification.category, Category::Governance);
        assert_eq!(notification.title, "New admission proposal");

        let signal = op(
            OriginKind::External,
            "cccc",
            json!({
                "propose": {
                    "proposal_id": "p2",
                    "action": { "signal": { "text": "x" } },
                    "voting_period": 10
                }
            }),
        );
        assert!(once("module:governance", &signal, &ctx).is_none());

        let redeem = op(
            OriginKind::External,
            "cccc",
            json!({ "redeem": { "joiner": [1, 2], "issuer": [3, 4] } }),
        );
        let notification = once("module:governance", &redeem, &ctx).unwrap();
        assert_eq!(notification.title, "New member admitted");
    }

    #[test]
    fn ignores_unknown_topics() {
        let row = op(OriginKind::External, "cccc", json!({}));
        assert!(once("module:pages", &row, &ctx(&no_root)).is_none());
    }
}
