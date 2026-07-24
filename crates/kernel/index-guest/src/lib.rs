//! the ducktape index-guest contract — the shared vocabulary between the
//! derived tier's host side (`indexer`, which writes the fold feed) and the
//! per-module index mappers (wasm guests installed inside each module's index
//! database, which consume it).
//!
//! a module's index mapper is a fluentabi module with two roles:
//!
//! - **fold** (`on_apply`) — invoked by the engine's changes-mode trigger on
//!   the `op/` range: every op row the host commits arrives exactly once, in
//!   commit order, and the guest folds it into derived read-model keys with
//!   exactly-once effects (its writes and the event's consumption share one
//!   transaction).
//! - **view** (`query`) — the module's materialized-view endpoint, served
//!   read-only at one MVCC snapshot (`POST /v1/index/{module}/view`).
//!
//! the fold is ASYNC by design: derived views trail the op log by the
//! trigger backlog (observable, never lost). the watermark (`meta/height`)
//! vouches for the OP LOG alone — "every block at or below H is in the feed" —
//! not for the derived rows, which are optimistic and converge.
//!
//! ## authoring shape: decide pure, write thin
//!
//! a mapper is decision functions plus a shell. the DECISIONS — fold one op
//! into [`Writes`], serve one view request — are pure functions over a
//! [`StateRead`] and live in the module crate's `src/index.rs`, compiled
//! natively and unit-tested against a plain [`BTreeMap`]. the SHELL
//! (`src/index_guest.rs`, behind the crate's `index-guest` feature, packaged
//! by `guest-builder --index`) backs [`StateRead`] with the engine ABI and
//! applies the decided writes, nothing more:
//!
//! ```ignore
//! // src/index_guest.rs — the whole shell.
//! use index_guest::guest::{self as ig, Change};
//! use index_guest::{Fail, StateRead as _};
//!
//! fn fold(changes: Vec<Change>) -> Result<(), Fail> {
//!     for op in ig::ops(changes)? {
//!         ig::apply(crate::index::fold_op(&op, &ig::EngineRead)?)?;
//!     }
//!     Ok(())
//! }
//!
//! fn view(req: Vec<u8>) -> Result<Vec<u8>, Fail> {
//!     crate::index::serve_view(&ig::EngineRead, &req)
//! }
//!
//! index_guest::fold!(fold);
//! index_guest::view!(view);
//! ```
//!
//! this crate is two layers:
//!
//! - default (types + the pure kit): the borsh [`OpRow`] envelope, key
//!   conventions, [`Fail`], [`StateRead`]/[`Writes`]/[`Page`], and the
//!   [`search`] posting-list helpers. the host side (`indexer`, the node
//!   bins) and every module's native build use exactly this.
//! - `guest` feature: the wasm authoring shell — `fluent-guest` re-exports,
//!   the [`fold!`]/[`view!`] entry macros, the [`guest::ops`] feed decoder,
//!   the engine-backed reader/writer. only guest-builder's synthesized wasm32
//!   workspaces enable it.
//!
//! the op-row envelope is BORSH on purpose: one deterministic byte layout,
//! no map-ordering or float games, decodable in a guest without pulling a
//! json tree. derived row VALUES stay module-defined (json by convention —
//! the scan/view HTTP surface serves them verbatim).

use std::collections::BTreeMap;

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

pub mod search;

// ============================================================================
// key conventions — shared verbatim by the host writer and every guest
// ============================================================================

/// reserved prefix of the host-written per-op rows (the fold feed).
pub const OP_PREFIX: &str = "op/";
/// reserved prefix of the host's store bookkeeping (watermark, backfill
/// floor). never delivered to a fold (the trigger range excludes it).
pub const META_PREFIX: &str = "meta/";
/// hard cap on one [`StateRead::scan_page`] page; larger asks are clamped,
/// mirroring the module query convention rather than erroring.
pub const MAX_SCAN_LIMIT: usize = 1024;

/// the key of one op row: fixed-width hex so lexicographic order IS numeric
/// order. `seq` is the block-wide dispatch index (fits: the host drain budget
/// is 1024 dispatches per block).
/// render an external submitter identity for display in a read model.
///
/// a `User` identity is a claimed display name on the embedded daemon
/// (printable utf-8, e.g. `jess`) but a raw ed25519 public key on the
/// networked node (32 arbitrary bytes). printable utf-8 passes through as
/// the name; anything else — control bytes, invalid utf-8, the common
/// pubkey case — renders as lowercase hex, never lossy `�` boxes. the node
/// layer renders EVERY external origin through this before it enters the
/// feed; guests use the SAME function on payload-carried user bytes
/// (memberships, huddle sweeps) so read-model keys and author strings
/// always agree.
pub fn user_handle(bytes: &[u8]) -> String {
    if let Ok(text) = std::str::from_utf8(bytes)
        && !text.is_empty()
        && !text.chars().any(char::is_control)
    {
        return text.to_string();
    }
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

pub fn op_key(height: u64, seq: u32) -> String {
    format!("{OP_PREFIX}{height:016x}/{seq:04x}")
}

// ============================================================================
// the op-row envelope — what the fold feed carries
// ============================================================================

/// who triggered a dispatch, flattened for the read model.
///
/// serde derives serve the node layer's json projections (`/v1/index/*`
/// responses); the wire between host and guest is borsh.
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct OriginTag {
    pub kind: OriginKind,
    /// external: the submitter identity rendered by the node layer (printable
    /// name, else hex); module: the emitting module id; system: absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum OriginKind {
    External,
    Module,
    System,
}

impl OriginTag {
    pub fn external(id: impl Into<String>) -> Self {
        Self {
            kind: OriginKind::External,
            id: Some(id.into()),
        }
    }

    pub fn module(id: impl Into<String>) -> Self {
        Self {
            kind: OriginKind::Module,
            id: Some(id.into()),
        }
    }

    pub fn system() -> Self {
        Self {
            kind: OriginKind::System,
            id: None,
        }
    }
}

/// the stored shape of one applied op — the borsh value under
/// [`op_key`]`(height, seq)`. `height`/`seq` repeat the key so a row is
/// self-describing when it travels without its key (a fold event, a shipped
/// feed page).
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct OpRow {
    pub height: u64,
    /// the block-wide dispatch index, so cross-module ordering survives the
    /// per-module split.
    pub seq: u32,
    /// the block's agreed timestamp (consensus time, not wall clock).
    pub time: u64,
    pub origin: OriginTag,
    /// the dispatch payload, verbatim.
    pub payload: Vec<u8>,
}

// ============================================================================
// the mapper vocabulary — pure decisions over a readable state
// ============================================================================

/// a mapper failure: a non-zero exit code plus a human-readable message.
/// ducktape's own type so decision cores never touch the engine SDK; the
/// guest shell converts it at the boundary. codes 2..=9 are the mapper's own
/// vocabulary; 1 is the generic default; 10+ are reserved by this crate's
/// helpers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fail {
    pub code: i32,
    pub message: String,
}

impl Fail {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// `?` on string errors: exit code 1.
impl From<String> for Fail {
    fn from(message: String) -> Self {
        Fail { code: 1, message }
    }
}

impl From<&str> for Fail {
    fn from(message: &str) -> Self {
        Fail::new(1, message)
    }
}

/// one decided derived write, in decision order: `Some` puts, `None`
/// deletes. order is load-bearing — when one op deletes and re-puts the same
/// key (a retokenize whose old and new text share a token), the last command
/// must win exactly as it would against the database.
pub type WriteCmd = (String, Option<Vec<u8>>);

/// the decided writes of one folded op.
pub type Writes = Vec<WriteCmd>;

/// stage a put into a [`Writes`] list.
pub fn put(out: &mut Writes, key: impl Into<String>, value: impl Into<Vec<u8>>) {
    out.push((key.into(), Some(value.into())));
}

/// stage a delete into a [`Writes`] list.
pub fn delete(out: &mut Writes, key: impl Into<String>) {
    out.push((key.into(), None));
}

/// one scan page. `next_after` feeds the next call's `after` for cursoring;
/// it is only present when `has_more`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    /// key/value pairs in key order. values are the raw stored bytes.
    pub entries: Vec<(Vec<u8>, Vec<u8>)>,
    pub has_more: bool,
    pub next_after: Option<String>,
}

/// the read surface a decision core sees: point lookups and prefix paging
/// over the module's own index. the guest shell backs it with the engine ABI
/// at the invocation's snapshot (a fold sees its own earlier writes); native
/// unit tests back it with a plain [`BTreeMap`].
pub trait StateRead {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>>;

    /// one page of keys under `prefix`, strictly after cursor `after` when
    /// given, in key order. `limit` is clamped to [`MAX_SCAN_LIMIT`].
    fn scan_page(&self, prefix: &[u8], after: Option<&[u8]>, limit: usize) -> Page;
}

/// lo bound for a prefix scan resuming strictly after `after`: the cursor
/// plus one 0x00 byte (the smallest strictly-greater key), else the prefix.
pub fn scan_lo(prefix: &[u8], after: Option<&[u8]>) -> Vec<u8> {
    match after {
        Some(a) if a >= prefix => {
            let mut lo = a.to_vec();
            lo.push(0);
            lo
        }
        _ => prefix.to_vec(),
    }
}

/// the smallest byte string greater than every key with `prefix`: increment
/// the last non-0xff byte and truncate. `None` = to the end of the space.
pub fn prefix_successor(prefix: &[u8]) -> Option<Vec<u8>> {
    let mut succ = prefix.to_vec();
    while let Some(last) = succ.last_mut() {
        if *last < 0xff {
            *last += 1;
            return Some(succ);
        }
        succ.pop();
    }
    None
}

/// build a [`Page`] from an ordered key/value stream already bounded to the
/// scan range — the one paging discipline both [`StateRead`] backends share.
pub fn collect_page(iter: impl Iterator<Item = (Vec<u8>, Vec<u8>)>, limit: usize) -> Page {
    let limit = limit.clamp(1, MAX_SCAN_LIMIT);
    let mut entries = Vec::new();
    let mut has_more = false;
    for kv in iter {
        if entries.len() == limit {
            has_more = true;
            break;
        }
        entries.push(kv);
    }
    let next_after = (has_more && !entries.is_empty())
        .then(|| String::from_utf8_lossy(&entries[entries.len() - 1].0).into_owned());
    Page {
        entries,
        has_more,
        next_after,
    }
}

/// the native-test backend: a plain ordered map IS a readable index.
impl StateRead for BTreeMap<Vec<u8>, Vec<u8>> {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        BTreeMap::get(self, key).cloned()
    }

    fn scan_page(&self, prefix: &[u8], after: Option<&[u8]>, limit: usize) -> Page {
        let lo = scan_lo(prefix, after);
        let hi = prefix_successor(prefix);
        let in_range = self.range(lo..).take_while(|(k, _)| match &hi {
            Some(hi) => k.as_slice() < hi.as_slice(),
            None => true,
        });
        collect_page(in_range.map(|(k, v)| (k.clone(), v.clone())), limit)
    }
}

/// apply decided writes to the native-test map — the unit-test twin of the
/// guest shell's writer, so decide→apply→decide sequences read their own
/// earlier writes exactly as they do inside a fold transaction.
pub fn apply_to_map(map: &mut BTreeMap<Vec<u8>, Vec<u8>>, writes: Writes) {
    for (key, cmd) in writes {
        match cmd {
            Some(value) => {
                map.insert(key.into_bytes(), value);
            }
            None => {
                map.remove(key.as_bytes());
            }
        }
    }
}

#[cfg(feature = "guest")]
pub mod guest;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn op_keys_order_numerically() {
        let earlier = op_key(1, 2);
        let later_seq = op_key(1, 10);
        let later_height = op_key(0x1_0000, 0);
        assert!(earlier < later_seq);
        assert!(later_seq < later_height);
    }

    #[test]
    fn op_row_borsh_round_trips() {
        let row = OpRow {
            height: 7,
            seq: 3,
            time: 7,
            origin: OriginTag::external("jess"),
            payload: b"{\"post\":1}".to_vec(),
        };
        let bytes = borsh::to_vec(&row).expect("op rows always encode");
        assert_eq!(borsh::from_slice::<OpRow>(&bytes).expect("round trip"), row);
    }

    #[test]
    fn map_backend_pages_with_cursor() {
        let mut map = BTreeMap::new();
        apply_to_map(
            &mut map,
            vec![
                ("a/1".into(), Some(b"1".to_vec())),
                ("a/2".into(), Some(b"2".to_vec())),
                ("a/3".into(), Some(b"3".to_vec())),
                ("b/1".into(), Some(b"x".to_vec())),
            ],
        );

        let first = map.scan_page(b"a/", None, 2);
        assert_eq!(first.entries.len(), 2);
        assert!(first.has_more);
        let cursor = first.next_after.expect("cursor when has_more");

        let rest = map.scan_page(b"a/", Some(cursor.as_bytes()), 10);
        assert_eq!(rest.entries.len(), 1, "resumes strictly after the cursor");
        assert_eq!(rest.entries[0].0, b"a/3".to_vec());
        assert!(!rest.has_more);
        assert!(rest.next_after.is_none());
    }

    #[test]
    fn writes_apply_in_command_order() {
        let mut map = BTreeMap::new();
        let mut writes = Writes::new();
        delete(&mut writes, "kept");
        put(&mut writes, "kept", b"fresh".to_vec());
        put(&mut writes, "dropped", b"old".to_vec());
        delete(&mut writes, "dropped");
        apply_to_map(&mut map, writes);

        assert_eq!(map.get(b"kept".as_slice()), Some(&b"fresh".to_vec()));
        assert!(!map.contains_key(b"dropped".as_slice()));
    }

    #[test]
    fn prefix_successor_edges() {
        assert_eq!(prefix_successor(b"op/"), Some(b"op0".to_vec()));
        assert_eq!(prefix_successor(&[0x01, 0xff]), Some(vec![0x02]));
        assert_eq!(prefix_successor(&[0xff, 0xff]), None);
        assert_eq!(prefix_successor(b""), None);
    }
}
