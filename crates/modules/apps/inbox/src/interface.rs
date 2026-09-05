//! the inbox module's public wire surface — types only.
//!
//! writes go via [`InboxMsg`]; the read surface (paged lists, unread counts)
//! lives on the derived tier (`src/index.rs`), not here — the module serves no
//! queries. the
//! inbox holds per-member notification queues as consensus state: other modules
//! deliver notifications as follow-up ops, so a notification commits atomically
//! with the event that caused it (platform promise P2), and no external push
//! service is involved (the air-gap-native notification story).
//!
//! `member` names a queue in the shared ACTOR-STRING domain
//! ([`sdk::Origin::actor_string`]) — the same domain [`Notification::source`]
//! already records the delivering origin in, and the one tasks' job board and
//! files' owner use. it is the module's whole identity model: a queue whose
//! name is `origin.actor_string()` is OWNED by that origin, which is what makes
//! an ack authorizable at all.
//!
//! DELIVERING and ACKING are different authorities. a module/system origin (a
//! follow-up from chat, tasks, automations, …) may `Deliver` to any member;
//! an external origin may `Deliver` only to its OWN queue (an unattributed
//! signed op cannot mint a fabricated member or flood a stranger's queue).
//! only the queue's OWN member may `MarkRead` or `Clear` it, and only an
//! authenticated external submitter owns a queue.

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

// ---- write-time caps (consensus constants) ---------------------------------
// enforced by the module BEFORE staging, so oversized bytes never enter the
// `root()` preimage. shared here so clients can pre-validate.

/// notification `kind` byte bound.
pub const MAX_KIND_BYTES: usize = 64;
/// notification `body` byte bound.
pub const MAX_BODY_BYTES: usize = 16 * 1024;
/// member-identity byte bound (must also be non-empty).
pub const MAX_MEMBER_BYTES: usize = 256;
/// per-member queue bound. when a delivery would exceed this, the OLDEST item
/// is dropped (this is a notification queue, not a ledger).
pub const MAX_ITEMS_PER_MEMBER: usize = 4096;
/// distinct members bound; a delivery that would introduce a new member beyond
/// this is rejected.
pub const MAX_MEMBERS: usize = 65536;

/// one delivered notification. `seq` is assigned per member, monotonic and
/// gap-free within what was ever assigned (a `Clear` removes items but never
/// rewinds the member's `next_seq`).
#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Notification {
    pub seq: u64,
    /// the queue this notification belongs to, in the actor-string domain (see
    /// the module header) — the principal that may ack it.
    pub member: String,
    pub kind: String,
    pub body: String,
    /// the delivering origin, derived by the module from `Env.origin`: a module
    /// id verbatim, `"ext:"` + the lowercase hex of external submitter bytes,
    /// or `"system"`. NEVER caller-supplied. the `ext:` prefix domain-separates
    /// external keys from module ids that happen to be pure hex.
    pub source: String,
    pub created_at: u64,
    pub read: bool,
}

/// the ack family (`MarkRead`, `Clear`) is MEMBER-BOUND: `member` must be the
/// submitter's own [`sdk::Origin::actor_string`], so a submitter can only ever
/// name their own queue. `Deliver` is deliberately outside that gate — writing
/// INTO a queue is the module's whole purpose and every origin may do it.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum InboxMsg {
    /// enqueue a notification for `member`. a module/system origin may
    /// deliver to any member (module follow-ups are the primary writers); an
    /// external origin may deliver only to its OWN queue (self-delivery).
    /// `source` is derived from the origin, not this msg.
    Deliver {
        member: String,
        kind: String,
        body: String,
    },
    /// mark every item with `seq <= up_to_seq` in the submitter's OWN queue as
    /// read. idempotent; an unknown member or seq is a deterministic no-op
    /// (never an error), but a member that is not the submitter is REFUSED.
    MarkRead { member: String, up_to_seq: u64 },
    /// delete every item with `seq <= up_to_seq` from the submitter's OWN
    /// queue. `next_seq` never rewinds. an unknown member or seq is a
    /// deterministic no-op; a member that is not the submitter is REFUSED.
    Clear { member: String, up_to_seq: u64 },
}

/// the assigned stamp inbox declares per applied op
/// ([`sdk::Ctx::set_assigned`]): the per-member sequence a `Deliver`
/// assigned in-state. rides the dispatch trace onto the derived-tier
/// op-feed row, so the fold consumes the exact assignment instead of
/// re-deriving it by counting.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum InboxAssigned {
    /// `Deliver`: the notification's assigned per-member sequence.
    Delivered { seq: u64 },
}

pub fn encode_msg(m: &InboxMsg) -> Vec<u8> {
    sdk::wire::encode(m)
}

pub fn decode_msg(b: &[u8]) -> Result<InboxMsg, String> {
    sdk::wire::decode(b)
}

pub fn encode_assigned(a: &InboxAssigned) -> Vec<u8> {
    sdk::wire::encode(a)
}

pub fn decode_assigned(b: &[u8]) -> Result<InboxAssigned, String> {
    sdk::wire::decode(b)
}
