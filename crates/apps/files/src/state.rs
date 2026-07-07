//! the small mutable consensus state — [`Refs`] — and its canonical codec. this
//! is the ONLY mutable state in duckfs; everything else is an immutable
//! content-addressed object. the module root is `root_bytes` = sha256 over the
//! canonical [`encode_refs`] image of THIS struct, and nothing else.
//!
//! two load-bearing rules pinned here:
//!
//! - **content-only root.** the recovery metadata that wraps the refs image on
//!   disk — the block height and the gc watermark — is per-node bookkeeping and
//!   lives ONLY in the refs-file envelope (`disk::DiskRefs`), never in this
//!   preimage. so the root does not move on an empty block, and two nodes with
//!   identical refs at different heights agree on the root.
//!
//! - **strict, canonical codec.** exactly one byte image encodes any refs value.
//!   [`decode_refs`] enforces it: counts are capped before they are trusted,
//!   pins/staging/watches must arrive in strictly-ascending key order, every
//!   read is bounds-checked, and a decode that does not consume the whole input
//!   is rejected. canonical bytes in, canonical bytes out, or an error — a
//!   colluding install can never smuggle a non-canonical image past the root
//!   check because a non-canonical image simply does not decode.
//!
//! there is no version byte: the frame is a fixed five-field shape (head flag,
//! then four counted sections) that is never empty, so it self-separates from
//! zero-length input, and layout changes ride the flag-day reset rule (fresh
//! genesis, no migrations) rather than an in-band version.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use sha2::{Digest as _, Sha256};

use crate::codec::{Reader, push_string, push_u32};
use crate::objects::ObjectId;
use crate::wire::{HISTORY_WINDOW, MAX_PINS, MAX_STAGING_ENTRIES, MAX_WATCHES};

/// a named pin: the snapshot it protects from gc and the owner allowed to remove
/// it (owner-gated unpin).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PinEntry {
    pub snapshot: ObjectId,
    pub owner: String,
}

/// a staged (putblob'd) chunk awaiting a commit that references it: who staged
/// it, its byte length, and the block height at which it expires if unreferenced
/// (the deterministic, op-stream-driven staging sweep keys on this).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Staged {
    pub owner: String,
    pub len: u64,
    pub expires_at: u64,
}

/// the whole mutable consensus state. `head` is the live snapshot; `window` is
/// the bounded commit history (newest-last insertion order — NOT a sorted set);
/// `pins` name-key protected snapshots; `staging` holds putblob'd-but-uncommitted
/// chunks; `watches` is the set of (prefix, module_id) subscriptions.
#[derive(Clone, Default, PartialEq, Debug)]
pub struct Refs {
    pub head: Option<ObjectId>,
    pub window: VecDeque<ObjectId>,
    pub pins: BTreeMap<String, PinEntry>,
    pub staging: BTreeMap<ObjectId, Staged>,
    pub watches: BTreeSet<(String, String)>,
}

/// the canonical refs image — the exact `root_bytes` preimage and the exact
/// payload the refs-file envelope wraps and the snapshot lane ships. layout
/// (all little-endian; strings are `u64 len ‖ utf-8 bytes`; ids are raw 32 B):
///
/// ```text
/// head    : u8 flag (1 = present) ‖ [32 B] present iff flag == 1
/// window  : u32 count ‖ count × id
/// pins    : u32 count ‖ count × (name ‖ snapshot ‖ owner)   -- name order
/// staging : u32 count ‖ count × (digest ‖ owner ‖ len u64 ‖ expires_at u64)
///                                                            -- digest order
/// watches : u32 count ‖ count × (prefix ‖ module_id)        -- tuple order
/// ```
pub fn encode_refs(r: &Refs) -> Vec<u8> {
    let mut out = Vec::new();

    // head: present-only, matching the object-model Option convention — a flag
    // byte, and the 32 id bytes iff the flag is 1. so None has exactly one
    // encoding (a lone 0) rather than 0 followed by 32 ambiguous bytes.
    match &r.head {
        Some(id) => {
            out.push(1);
            out.extend_from_slice(id);
        }
        None => out.push(0),
    }

    // window: a sequence in history order; the count is u32 and the order is the
    // deque's own order (decode does NOT re-sort it — order is meaningful).
    push_u32(&mut out, r.window.len() as u32);
    for id in &r.window {
        out.extend_from_slice(id);
    }

    // pins: BTreeMap iterates ascending by name, which is exactly the canonical
    // order the decoder re-checks.
    push_u32(&mut out, r.pins.len() as u32);
    for (name, pin) in &r.pins {
        push_string(&mut out, name);
        out.extend_from_slice(&pin.snapshot);
        push_string(&mut out, &pin.owner);
    }

    // staging: BTreeMap keyed by the 32-byte digest, ascending.
    push_u32(&mut out, r.staging.len() as u32);
    for (digest, staged) in &r.staging {
        out.extend_from_slice(digest);
        push_string(&mut out, &staged.owner);
        out.extend_from_slice(&staged.len.to_le_bytes());
        out.extend_from_slice(&staged.expires_at.to_le_bytes());
    }

    // watches: BTreeSet of (prefix, module_id) tuples, ascending lexicographic.
    push_u32(&mut out, r.watches.len() as u32);
    for (prefix, module_id) in &r.watches {
        push_string(&mut out, prefix);
        push_string(&mut out, module_id);
    }

    out
}

/// strict decode of an [`encode_refs`] image; anything non-canonical rejects.
pub fn decode_refs(bytes: &[u8]) -> Result<Refs, String> {
    let mut r = Reader::new("refs image", bytes);

    let head = if r.boolean()? {
        Some(r.bytes32()?)
    } else {
        None
    };

    let window_count = r.u32()? as usize;
    if window_count > HISTORY_WINDOW {
        return Err("files: refs window count over cap".into());
    }
    let mut window = VecDeque::new();
    for _ in 0..window_count {
        window.push_back(r.bytes32()?);
    }

    let pin_count = r.u32()? as usize;
    if pin_count > MAX_PINS {
        return Err("files: refs pin count over cap".into());
    }
    let mut pins: BTreeMap<String, PinEntry> = BTreeMap::new();
    for _ in 0..pin_count {
        let name = r.string()?;
        let snapshot = r.bytes32()?;
        let owner = r.string()?;
        // strictly ascending names keep the image canonical: no duplicate and no
        // reordered pair can produce a second valid preimage of the same refs.
        if pins
            .last_key_value()
            .is_some_and(|(last, _)| last.as_str() >= name.as_str())
        {
            return Err("files: refs pins not strictly ascending".into());
        }
        pins.insert(name, PinEntry { snapshot, owner });
    }

    let staging_count = r.u32()? as usize;
    if staging_count > MAX_STAGING_ENTRIES {
        return Err("files: refs staging count over cap".into());
    }
    let mut staging: BTreeMap<ObjectId, Staged> = BTreeMap::new();
    for _ in 0..staging_count {
        let digest = r.bytes32()?;
        let owner = r.string()?;
        let len = r.u64()?;
        let expires_at = r.u64()?;
        if staging
            .last_key_value()
            .is_some_and(|(last, _)| last >= &digest)
        {
            return Err("files: refs staging not strictly ascending".into());
        }
        staging.insert(
            digest,
            Staged {
                owner,
                len,
                expires_at,
            },
        );
    }

    let watch_count = r.u32()? as usize;
    if watch_count > MAX_WATCHES {
        return Err("files: refs watch count over cap".into());
    }
    let mut watches: BTreeSet<(String, String)> = BTreeSet::new();
    let mut last_watch: Option<(String, String)> = None;
    for _ in 0..watch_count {
        let prefix = r.string()?;
        let module_id = r.string()?;
        let entry = (prefix, module_id);
        if last_watch.as_ref().is_some_and(|last| last >= &entry) {
            return Err("files: refs watches not strictly ascending".into());
        }
        last_watch = Some(entry.clone());
        watches.insert(entry);
    }

    r.finish()?;
    Ok(Refs {
        head,
        window,
        pins,
        staging,
        watches,
    })
}

/// the module root preimage: sha256 over the canonical refs image. height and
/// gc_watermark are deliberately absent — see the module docblock.
pub fn root_bytes(r: &Refs) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(encode_refs(r));
    h.finalize().into()
}

// the cursor codec (push helpers + the strict [`Reader`]) is shared with
// `objects.rs` via `crate::codec` — one grammar, two frames, zero drift.
