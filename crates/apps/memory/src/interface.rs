//! the memory module's public wire surface -- types only.
//!
//! memory is a shared agent workspace shaped like a filesystem (the "NoKV"
//! philosophy — agents want filesystems, not key-value stores). every path is a
//! metadata namespace with atomic write-once publish; consensus makes the
//! publish atomic for free and the namespace byte-identical for every agent on
//! every node. generations are immutable, snapshots are time travel, changes are
//! events (not polls), reads are progressive-disclosure verbs, and evidence is a
//! citable `duck://` URI.
//!
//! writes go via [`MemoryMsg`]; reads via [`MemoryQuery`] -> [`MemoryReply`];
//! watch subscribers receive [`MemoryEvent`] payloads. authorship is never part
//! of a write payload — the module derives it from the dispatch origin — so the
//! wire surface carries `author` only in reply/event records.
//!
//! skills sharing rides the same namespace: a skill is just a document under
//! `/skills/<name>` with `kind=skill` meta; agents discover skills with
//! [`MemoryQuery::Find`] over `/skills/`, and a skill's `(path, generation)` is
//! a stable, hash-pinned reference.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---- write-time caps (consensus constants) ---------------------------------
// every cap is enforced by the module BEFORE staging, with rejection, so an
// oversized value never enters the `root()` preimage (the repo's poison-value
// lesson). shared here so clients can pre-validate.

/// full path byte length bound (including the leading `/`).
pub const MAX_PATH_BYTES: usize = 512;
/// per-segment byte length bound.
pub const MAX_SEGMENT_BYTES: usize = 128;
/// inline published body byte length bound.
pub const MAX_BODY_BYTES: usize = 64 * 1024;
/// entries per [`Meta`] map.
pub const MAX_META_ENTRIES: usize = 16;
/// [`Meta`] key byte length bound.
pub const MAX_META_KEY_BYTES: usize = 64;
/// [`Meta`] value byte length bound.
pub const MAX_META_VALUE_BYTES: usize = 256;
/// generations per path; further publishes are rejected.
pub const MAX_GENERATIONS_PER_PATH: u64 = 1024;
/// distinct live files in the namespace.
pub const MAX_FILES: usize = 65536;
/// named snapshots pinned at once.
pub const MAX_SNAPSHOTS: usize = 256;
/// snapshot name byte length bound.
pub const MAX_SNAPSHOT_NAME_BYTES: usize = 128;
/// registered `(prefix, module)` watches.
pub const MAX_WATCHES: usize = 256;
/// module-id byte length bound (watch targets).
pub const MAX_MODULE_ID_BYTES: usize = 128;
/// query page bound; larger limits are clamped down to this.
pub const MAX_QUERY_LIMIT: u64 = 256;
/// per-hit grep line text bound, in bytes (truncated on a char boundary).
pub const MAX_GREP_LINE_BYTES: usize = 256;

/// reserved meta key: the kind of document — e.g. `"skill"`, `"note"`,
/// `"artifact"`. a convention, documented but not enforced beyond the meta
/// caps; skills live under `/skills/<name>` with `kind=skill`.
pub const META_KIND: &str = "kind";
/// reserved meta value for the [`META_KIND`] key marking a skill document.
pub const KIND_SKILL: &str = "skill";
/// the directory every skill document lives under.
pub const SKILLS_PREFIX: &str = "/skills/";

/// free-form document metadata: a small sorted key/value map. bounded by
/// [`MAX_META_ENTRIES`] / [`MAX_META_KEY_BYTES`] / [`MAX_META_VALUE_BYTES`].
pub type Meta = BTreeMap<String, String>;

/// one immutable generation body, kept directly in memory's consensus state.
/// (file-backed bodies pinned to the old files-module manifests were removed
/// in the duckfs flag-day reset; memory is inline-only until its deletion.)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Body {
    Inline(String),
}

/// write-time body selector — inline only (see [`Body`]).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum PublishBody {
    Inline(String),
}

/// one immutable generation of a file: the write-once body plus its metadata and
/// origin-derived provenance. `generation` starts at 1 and increases by 1 on
/// every publish; a `(path, generation)` pair is a stable, hash-pinned reference.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Generation {
    pub generation: u64,
    pub body: Body,
    pub meta: Meta,
    /// derived from `Env.origin` — a module id verbatim, `"ext:"` + lowercase
    /// hex of the external submitter's bytes (domain-separated so a module id
    /// can never collide with a hex identity), or `"system"`; never
    /// caller-supplied.
    pub author: String,
    pub published_at_height: u64,
}

/// the summary view of a live file: its latest generation's provenance plus the
/// generation counters. `generations` is the count of generations the current
/// live file holds (`latest_generation - first_generation + 1`).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct FileStat {
    pub path: String,
    pub latest_generation: u64,
    pub generations: u64,
    pub latest_meta: Meta,
    pub latest_author: String,
    pub latest_published_at_height: u64,
    pub body_len: u64,
}

/// one entry returned by [`MemoryQuery::Ls`]: an implicit child directory, or a
/// file with its [`FileStat`]. sorted by path within the reply.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LsEntry {
    /// an implicit directory directly under the listed path. `path` is the full
    /// absolute directory path (no trailing slash).
    Dir { path: String },
    /// a file directly under the listed path.
    File(FileStat),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryMsg {
    /// write-once publish: assigns `generation = latest + 1` (1 for a new file)
    /// atomically and appends an immutable generation. body/meta caps are
    /// enforced at execute time with rejection.
    Publish {
        path: String,
        body: PublishBody,
        meta: Meta,
    },
    /// remove a file (all its live generations). snapshots still pin whatever
    /// they captured, so deletion never breaks a snapshot read; a generation's
    /// data is retained as long as some snapshot references it.
    Delete {
        path: String,
    },
    /// pin the CURRENT `path -> latest generation` mapping of the entire
    /// namespace under a name. duplicate names are rejected.
    Snapshot {
        name: String,
    },
    /// release a snapshot's pins; retained generation data with no remaining
    /// reference is dropped.
    DropSnapshot {
        name: String,
    },
    /// subscribe `module_id` to publishes at or below `prefix`, which must be a
    /// CANONICAL absolute path (`"/"` watches the whole namespace). matching is
    /// segment-aware: `/a` matches `/a` and `/a/b` but never `/ab`. on every
    /// successful publish, one follow-up [`MemoryEvent::Published`] is emitted
    /// to each matching watcher module (the chat-hook fan-out pattern).
    RegisterWatch {
        prefix: String,
        module_id: String,
    },
    UnregisterWatch {
        prefix: String,
        module_id: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryQuery {
    /// entries directly under `path` (child dirs + files), sorted by path, up
    /// to `limit` (clamped to [`MAX_QUERY_LIMIT`]).
    Ls { path: String, limit: u64 },
    /// the [`FileStat`] of one file, or `None`.
    Stat { path: String },
    /// one generation of a file. `generation` and `snapshot` are mutually
    /// exclusive (both set is rejected); `snapshot` resolves the pinned
    /// generation; neither reads the latest.
    Read {
        path: String,
        generation: Option<u64>,
        snapshot: Option<String>,
    },
    /// live files under `prefix` whose latest meta matches every `meta_filter`
    /// pair, sorted by path, up to `limit` (clamped to [`MAX_QUERY_LIMIT`]).
    /// this is how agents list skills: `Find { prefix: "/skills/",
    /// meta_filter: { kind: skill } }`.
    Find {
        prefix: String,
        meta_filter: BTreeMap<String, String>,
        limit: u64,
    },
    /// case-sensitive substring scan (no regex — determinism) over the latest
    /// generations of live files under `prefix`, paths in sorted order, up to
    /// `limit` (clamped to [`MAX_QUERY_LIMIT`]) hits.
    Grep {
        prefix: String,
        pattern: String,
        limit: u64,
    },
}

/// a single grep match, carrying a citable evidence URI of the form
/// `duck://memory/<path>@<generation>#L<line>` (line is 1-indexed).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct GrepHit {
    pub uri: String,
    pub path: String,
    pub generation: u64,
    pub line: u64,
    pub text: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryReply {
    Ls(Vec<LsEntry>),
    Stat(Option<FileStat>),
    Read(Option<Generation>),
    Find(Vec<FileStat>),
    Grep(Vec<GrepHit>),
}

/// the watch notification payload: one follow-up [`sdk::Msg`]-shaped dispatch
/// per matching watcher module, emitted in the same block as the publish (P2).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryEvent {
    Published {
        path: String,
        generation: u64,
        meta: Meta,
        author: String,
    },
}

pub fn encode_msg(m: &MemoryMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}

pub fn decode_msg(b: &[u8]) -> Result<MemoryMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

pub fn encode_query(q: &MemoryQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}

pub fn decode_query(b: &[u8]) -> Result<MemoryQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

pub fn encode_reply(r: &MemoryReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}

pub fn decode_reply(b: &[u8]) -> Result<MemoryReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

pub fn encode_event(e: &MemoryEvent) -> Vec<u8> {
    serde_json::to_vec(e).expect("serializable")
}

pub fn decode_event(b: &[u8]) -> Result<MemoryEvent, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
