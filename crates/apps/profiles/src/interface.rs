//! the profiles module's public wire surface -- types only.
//!
//! a profile maps a VERIFIED submit origin (its pubkey bytes) to a chosen
//! display name, so the ui can show names instead of hex. writes go via
//! [`ProfileMsg`]; reads via [`ProfileQuery`] -> [`ProfileReply`].
//!
//! the registry is ORIGIN-GATED: the single write keys on the verified
//! `ctx.env().origin`, never a payload field, so a submitter can only name
//! itself -- spoof-proof by origin routing, exactly like chat authorship. the
//! map key is therefore the origin bytes, lining up 1:1 with chat's
//! `AuthorRef::User(bytes)`.

use serde::{Deserialize, Serialize};

/// max display-name length, in bytes; a longer name is rejected.
pub const MAX_NAME_LEN: usize = 64;

/// query pagination ceiling -- [`ProfileQuery::All`] clamps `limit` to this,
/// like the other product modules.
pub const MAX_QUERY_LIMIT: u64 = 256;

/// one registered profile: the origin key, its display name, and the block
/// timestamp of the last write.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub key: Vec<u8>,
    pub display_name: String,
    pub updated_at: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProfileMsg {
    /// set the SUBMITTER'S OWN display name -- the key is the verified origin,
    /// never carried in the payload, so a submitter can only name itself. a
    /// name that trims to empty CLEARS the record (removes it); a name longer
    /// than [`MAX_NAME_LEN`] bytes is rejected.
    SetName { display_name: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProfileQuery {
    /// every profile, ascending by key, offset+limit paginated (`limit`
    /// clamped to [`MAX_QUERY_LIMIT`]).
    All { from: u64, limit: u64 },
    /// one profile by its origin key.
    Get { key: Vec<u8> },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProfileReply {
    Profiles(Vec<Profile>),
    Profile(Option<Profile>),
}

pub fn encode_msg(m: &ProfileMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}

pub fn decode_msg(b: &[u8]) -> Result<ProfileMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

pub fn encode_query(q: &ProfileQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}

pub fn decode_query(b: &[u8]) -> Result<ProfileQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

pub fn encode_reply(r: &ProfileReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}

pub fn decode_reply(b: &[u8]) -> Result<ProfileReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
