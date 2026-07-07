//! the dummy harness's wire surface — action tags, write-time caps, the note
//! payload schema, and the notes/status queries (serde_json on the wire,
//! like every module interface).

use serde::{Deserialize, Serialize};

/// create a note: `{ "note_id": ..., "text": ... }`.
pub const ACTION_NOTE_ADD: &str = "dummy.note.add";
/// replace an EXISTING note's text: `{ "note_id": ..., "text": ... }`.
pub const ACTION_NOTE_SET_TEXT: &str = "dummy.note.set_text";

pub(crate) const MAX_NOTE_ID_BYTES: usize = 64;
pub(crate) const MAX_NOTE_TEXT_BYTES: usize = 4096;
pub(crate) const MAX_NOTES: usize = 1024;

// ---- wire surface ----------------------------------------------------------------

/// one note, as served by [`DummyQuery::Notes`].
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub note_id: String,
    pub text: String,
}

/// the harness's committed lifecycle view.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DummyStatus {
    pub package: String,
    /// `"active"`, `"suspended"`, or `"unplugged"`.
    pub phase: String,
    pub agents: Vec<String>,
    /// how many jobs this harness has minted over its life.
    pub minted: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DummyQuery {
    Notes,
    Status,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DummyReply {
    Notes(Vec<Note>),
    Status(Option<DummyStatus>),
}

pub fn encode_query(q: &DummyQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}
pub fn decode_query(b: &[u8]) -> Result<DummyQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_reply(r: &DummyReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}
pub fn decode_reply(b: &[u8]) -> Result<DummyReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

/// both note actions share one payload schema; unknown fields reject at probe.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct NotePayload {
    pub(crate) note_id: String,
    pub(crate) text: String,
}
