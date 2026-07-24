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
//! `member` is an OPAQUE member-identity string. authorship is NOT modeled here:
//! origin-bound member identity is a platform-wide open item, so this crate does
//! not invent an auth scheme. `source` records the DELIVERING origin and is
//! derived by the module from `Env.origin` (never caller-supplied).

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
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub seq: u64,
    /// the opaque member identity this notification belongs to.
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InboxMsg {
    /// enqueue a notification for `member`. accepted from ANY origin: module
    /// follow-ups are the primary writers, but an external submitter may
    /// self-deliver a note. `source` is derived from the origin, not this msg.
    Deliver {
        member: String,
        kind: String,
        body: String,
    },
    /// mark every item with `seq <= up_to_seq` as read. idempotent; an unknown
    /// member or seq is a deterministic no-op (never an error).
    MarkRead { member: String, up_to_seq: u64 },
    /// delete every item with `seq <= up_to_seq`. `next_seq` never rewinds. an
    /// unknown member or seq is a deterministic no-op (never an error).
    Clear { member: String, up_to_seq: u64 },
}

pub fn encode_msg(m: &InboxMsg) -> Vec<u8> {
    sdk::wire::encode(m)
}

pub fn decode_msg(b: &[u8]) -> Result<InboxMsg, String> {
    sdk::wire::decode(b)
}
