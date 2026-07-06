//! the tagging plane's public wire surface — types only.
//!
//! the tagging plane is the network's cross-module engagement router: content
//! in module A names an entity of module B, and B gets an engagement event.
//! a subscriber module registers interest in a source scope
//! ([`TaggingMsg::Subscribe`]); a content module reports each content item as
//! a [`TagEvent`]; the plane fans one [`EngagementEvent`] out to every
//! subscriber of that scope as a follow-up `Msg`.
//!
//! the plane is a ROUTER ONLY. it holds no engagement policy — whether and
//! how a delivered event engages an entity (mention-gating, assignment,
//! round-robin) is the subscriber module's business. and it is 100%
//! module-agnostic: content modules translate their own author and mention
//! shapes into this crate's vocabulary at their edge; the plane never decodes
//! a content module's event payloads.
//!
//! both intakes key trust on the host-assigned dispatch origin, never on
//! payload fields: a [`TagEvent`]'s source IS the emitting module's origin,
//! and a subscription's subscriber IS the subscribing module's origin — so
//! neither can be spoofed by an external submitter.

use serde::{Deserialize, Serialize};

/// the conventional module id the tagging plane registers under.
pub const DEFAULT_TAGGING_TARGET: &str = "tagging";

// ---- consensus constants ----------------------------------------------------

/// hard cap on a container or entity id.
pub const MAX_ID_BYTES: usize = 128;

/// hard cap on the entity tags one content item may carry; a longer list is
/// truncated deterministically (first wins), never rejected — the tag intake
/// is a no-fail arm.
pub const MAX_TAGS_PER_EVENT: usize = 16;

/// hard cap on subscriber modules per (source, container) scope — the bound
/// on the engagement fan-out one content item can cause.
pub const MAX_SUBSCRIBERS_PER_SCOPE: usize = 8;

// ---- the universal entity reference ------------------------------------------

/// one module-scoped entity, addressable from any other module: the module's
/// genesis id plus the module-local entity id (an agent id, a page block id,
/// a document id — the owning module defines the encoding).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EntityRef {
    pub module: String,
    pub entity: String,
}

/// who authored a piece of tagged content — the plane's own vocabulary,
/// mapped from each content module's author shape at that module's edge.
/// the plane's loop rule keys on this: only [`Author::User`] content fires
/// engagement (an engaged entity's reply is itself content, and entity-,
/// module-, and system-authored content must never re-fire).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Author {
    /// an external submitter's (non-empty) public key bytes.
    User(Vec<u8>),
    /// a module-hosted entity (e.g. an agent) — a reply by an engaged entity.
    Entity(EntityRef),
    /// a module writing as itself.
    Module(String),
    /// genesis / system-internal.
    System,
}

/// one content item's tag report, emitted by the content module in the same
/// block as the content itself. the SOURCE module is the dispatch origin;
/// `container` and `content_seq` are source-scoped coordinates (chat: channel
/// id and message sequence) a subscriber hands back to the source module's
/// query surface for its domain work.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct TagEvent {
    pub container: String,
    pub content_seq: u64,
    pub author: Author,
    /// the entities this content names, in content order, deduped by the
    /// content module.
    pub tags: Vec<EntityRef>,
}

// ---- ops -----------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum TaggingMsg {
    /// MODULE-ORIGIN ONLY: register the emitting module as a subscriber of
    /// `(source, container)`. idempotent — re-subscribing an existing
    /// subscription stages nothing.
    Subscribe { source: String, container: String },
    /// MODULE-ORIGIN ONLY: drop the emitting module's subscription.
    /// idempotent — unsubscribing an absent subscription stages nothing.
    Unsubscribe { source: String, container: String },
    /// MODULE-ORIGIN ONLY: report one content item; the source is the
    /// emitting module's origin. a NO-FAIL intake: this rides the same block
    /// as the content itself, so a malformed report is a staged no-op, never
    /// an error.
    Tag(TagEvent),
}

/// the delivery a subscriber module receives as a follow-up `Msg` from the
/// plane, in the same block as the content. `source` is the content module
/// the plane verified by origin; the rest is the [`TagEvent`] verbatim.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct EngagementEvent {
    pub source: String,
    pub container: String,
    pub content_seq: u64,
    pub author: Author,
    pub tags: Vec<EntityRef>,
}

// ---- queries --------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum TaggingQuery {
    /// every subscription scope with its subscriber set.
    Subscriptions,
    /// one scope's subscriber set (empty == no subscription).
    Subscribers { source: String, container: String },
}

/// one subscription scope's observable state.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionView {
    pub source: String,
    pub container: String,
    pub subscribers: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum TaggingReply {
    Subscriptions(Vec<SubscriptionView>),
    Subscribers(Vec<String>),
}

// ---- codecs ---------------------------------------------------------------------

pub fn encode_msg(m: &TaggingMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}
pub fn decode_msg(b: &[u8]) -> Result<TaggingMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_event(e: &EngagementEvent) -> Vec<u8> {
    serde_json::to_vec(e).expect("serializable")
}
pub fn decode_event(b: &[u8]) -> Result<EngagementEvent, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_query(q: &TaggingQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}
pub fn decode_query(b: &[u8]) -> Result<TaggingQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_reply(r: &TaggingReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}
pub fn decode_reply(b: &[u8]) -> Result<TaggingReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
