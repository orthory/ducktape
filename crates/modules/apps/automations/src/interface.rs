//! the automations module's public wire surface -- types only.
//!
//! writes go via [`AutomationsMsg`]; reads via [`AutomationsQuery`] ->
//! [`AutomationsReply`]. the chat hook seam delivers a `chat::ChatEvent`
//! payload; the automations module decodes it inside its origin-gated hook arm
//! (see [`AutomationsMsg::HookEvent`]).

use serde::{Deserialize, Serialize};

/// what makes a rule fire: a chat message-posted filter. every `None` field is
/// a wildcard; every `Some` field must match the triggering event. (formerly a
/// single-variant `MessagePosted` enum; the snapshot layout still carries a
/// trigger-kind byte so a future non-chat trigger can join that codec without
/// a state break.)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
// deny_unknown_fields is load-bearing: every field is an Option, so without
// it the RETIRED tagged shape ({"message_posted":{...}}) would silently parse
// as an all-None trigger that fires on every message — a quiet-corruption
// hazard, not a flag day. Unknown keys must reject loudly.
#[serde(deny_unknown_fields)]
pub struct Trigger {
    /// an exact channel id, or `None` for any channel.
    pub channel_id: Option<String>,
    /// a substring tested against each mention's display form (see the
    /// module's `display_author`); `None` = no mention constraint.
    pub mention: Option<String>,
    /// a case-sensitive substring tested against the post's concatenated
    /// text blocks; `None` = no text constraint.
    pub text_contains: Option<String>,
}

/// what a firing rule does.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// post `template` (after placeholder substitution) into `channel_id`. the
    /// module derives a deterministic `message_id` = `auto-{rule_id}-{channel}-{seq}`.
    PostMessage {
        channel_id: String,
        template: String,
    },
    /// create a task whose id = `{task_id_prefix}-{channel}-{seq}` (deterministic,
    /// collision-free per triggering message) and title = substituted
    /// `title_template`.
    CreateTask {
        task_id_prefix: String,
        title_template: String,
    },
    /// deliver an inbox notification. `kind` is literal; `member_template` and
    /// `body_template` are substituted at fire time.
    DeliverInbox {
        member_template: String,
        kind: String,
        body_template: String,
    },
}

/// a user-defined automation rule.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub rule_id: String,
    pub enabled: bool,
    pub trigger: Trigger,
    pub action: Action,
    pub created_at: u64,
    /// how many times this rule has emitted an action (successful fires only).
    pub fire_count: u64,
}

/// one entry in the module's bounded global run-history ring.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RunRecord {
    pub rule_id: String,
    /// the triggering channel id.
    pub channel_id: String,
    pub seq: u64,
    pub height: u64,
    /// `true` when an action was emitted; `false` for a skipped fire (malformed
    /// action, or the per-event action budget was exceeded).
    pub action_ok: bool,
    pub detail: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AutomationsMsg {
    CreateRule {
        rule_id: String,
        trigger: Trigger,
        action: Action,
    },
    SetEnabled {
        rule_id: String,
        enabled: bool,
    },
    DeleteRule {
        rule_id: String,
    },
    /// the chat hook payload: the `chat::ChatEvent` bytes chat delivers
    /// as a follow-up. HONORED ONLY when the dispatch origin is the chat module;
    /// a non-chat origin claiming a hook event is rejected as a spoof.
    HookEvent(Vec<u8>),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AutomationsQuery {
    ListRules,
    GetRule {
        rule_id: String,
    },
    /// the most recent `limit` run-history records for `rule_id`, oldest-first.
    RunHistory {
        rule_id: String,
        limit: u64,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AutomationsReply {
    Rules(Vec<Rule>),
    Rule(Option<Rule>),
    History(Vec<RunRecord>),
}

pub fn encode_msg(m: &AutomationsMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}

pub fn decode_msg(b: &[u8]) -> Result<AutomationsMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

pub fn encode_query(q: &AutomationsQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}

pub fn decode_query(b: &[u8]) -> Result<AutomationsQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

pub fn encode_reply(r: &AutomationsReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}

pub fn decode_reply(b: &[u8]) -> Result<AutomationsReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
