//! deterministic in-memory display-name registry, origin-gated.
//!
//! a profile maps a VERIFIED submit origin (`Origin::External(pubkey)`) to a
//! chosen display name. the single write [`ProfileMsg::SetName`] keys on
//! `ctx.env().origin`, never a payload field, so a submitter can only set ITS
//! OWN name -- spoof-proof by origin routing, the same property chat authorship
//! relies on. module and system origins (and the empty external default) are
//! refused: only a real external identity has a profile.
//!
//! like `tasks`/`vaults` this slice is state-based, not qmdb-backed: the api
//! needs ordered list/get over a small canonical state. the module STAGES
//! writes during `execute` into a pending overlay (a `None` entry is a clear),
//! publishes them only at `commit_block`, and computes `root()` from the
//! committed `BTreeMap` alone. `snapshot`/`install` use the exact canonical
//! byte stream that `root()` hashes so a joiner can verify a peer image before
//! adopting it.

// the wire surface: this module's shared types, flattened at the crate root.
mod interface;
pub use interface::*;

use std::collections::BTreeMap;

use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};
use sha2::{Digest, Sha256};

/// one stored profile: the display name plus the last-write block timestamp.
/// the origin key is the map key, so it is not repeated here.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Record {
    display_name: String,
    updated_at: u64,
}

pub struct Profiles {
    id: ModuleId,
    /// committed registry -- what `root()` commits to.
    profiles: BTreeMap<Vec<u8>, Record>,
    /// this block's staged writes: `Some` = upsert, `None` = clear the key.
    pending: BTreeMap<Vec<u8>, Option<Record>>,
}

impl Profiles {
    pub fn new(id: impl Into<ModuleId>) -> Self {
        Self {
            id: id.into(),
            profiles: BTreeMap::new(),
            pending: BTreeMap::new(),
        }
    }

    /// the AUTHENTICATED submitter key. a profile is a user action keyed on the
    /// verified origin: module and system origins are refused so no module can
    /// quietly own a profile, and the empty external default (pre-consensus)
    /// never passes as an identity.
    fn origin_key(ctx: &dyn Ctx) -> Result<Vec<u8>, Error> {
        match &ctx.env().origin {
            Origin::External(bytes) if bytes.is_empty() => Err(Error::Module(
                "external origin must carry a non-empty submitter id".into(),
            )),
            Origin::External(bytes) => Ok(bytes.clone()),
            other => Err(Error::Module(format!(
                "profiles are origin-gated to external submitters, got {other:?}"
            ))),
        }
    }

    /// read one profile through the staged overlay (read-your-writes): a
    /// pending `None` reads as cleared even if a committed record exists.
    fn get(&self, key: &[u8]) -> Option<Record> {
        match self.pending.get(key) {
            Some(change) => change.clone(),
            None => self.profiles.get(key).cloned(),
        }
    }

    /// committed state with this block's staged changes applied -- the read
    /// projection for queries (a `None` overlay entry removes the key).
    fn merged(&self) -> BTreeMap<Vec<u8>, Record> {
        let mut merged = self.profiles.clone();
        for (key, change) in &self.pending {
            match change {
                Some(record) => {
                    merged.insert(key.clone(), record.clone());
                }
                None => {
                    merged.remove(key);
                }
            }
        }
        merged
    }

    fn profile_of(key: &[u8], record: &Record) -> Profile {
        Profile {
            key: key.to_vec(),
            display_name: record.display_name.clone(),
            updated_at: record.updated_at,
        }
    }

    // ---- canonical state bytes ----------------------------------------------

    fn encode_state(profiles: &BTreeMap<Vec<u8>, Record>) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(profiles.len() as u64).to_le_bytes());
        for (key, record) in profiles {
            push_bytes(&mut out, key);
            push_bytes(&mut out, record.display_name.as_bytes());
            out.extend_from_slice(&record.updated_at.to_le_bytes());
        }
        out
    }

    fn root_of(profiles: &BTreeMap<Vec<u8>, Record>) -> StateRoot {
        let mut h = Sha256::new();
        h.update(Self::encode_state(profiles));
        StateRoot(h.finalize().into())
    }

    /// canonical bytes of COMMITTED state -- the exact preimage of `root()`.
    pub fn snapshot(&self) -> Vec<u8> {
        Self::encode_state(&self.profiles)
    }

    /// verify-then-adopt a peer snapshot; any error leaves every layer intact.
    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        let decoded = decode_state(bytes)?;
        if Self::root_of(&decoded) != expected {
            return Err(Error::Module("snapshot root mismatch".into()));
        }
        self.profiles = decoded;
        self.pending.clear();
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Profiles {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        Self::root_of(&self.profiles)
    }

    /// advertise the snapshot lane: [`Profiles::snapshot`] is the exact preimage
    /// of `root()`, and [`Profiles::install`] verifies before adopting.
    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(self.snapshot()))
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let key = Self::origin_key(ctx)?;
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            ProfileMsg::SetName { display_name } => {
                let trimmed = display_name.trim();
                if trimmed.is_empty() {
                    // clearing your name removes the record; a clear of an
                    // absent record commits to the same state (a no-op).
                    self.pending.insert(key, None);
                } else if trimmed.len() > MAX_NAME_LEN {
                    return Err(Error::Module(format!(
                        "display name exceeds the {MAX_NAME_LEN}-byte limit"
                    )));
                } else {
                    self.pending.insert(
                        key,
                        Some(Record {
                            display_name: trimmed.to_string(),
                            updated_at: ctx.env().consensus_time,
                        }),
                    );
                }
                Ok(())
            }
        }
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            ProfileQuery::All { from, limit } => {
                let merged = self.merged();
                let limit = limit.min(MAX_QUERY_LIMIT) as usize;
                let from = usize::try_from(from).unwrap_or(usize::MAX);
                let profiles = merged
                    .iter()
                    .skip(from)
                    .take(limit)
                    .map(|(key, record)| Self::profile_of(key, record))
                    .collect();
                Ok(encode_reply(&ProfileReply::Profiles(profiles)))
            }
            ProfileQuery::Get { key } => Ok(encode_reply(&ProfileReply::Profile(
                self.get(&key).map(|record| Self::profile_of(&key, &record)),
            ))),
        }
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        for (key, change) in std::mem::take(&mut self.pending) {
            match change {
                Some(record) => {
                    self.profiles.insert(key, record);
                }
                None => {
                    self.profiles.remove(&key);
                }
            }
        }
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending.clear();
        Ok(())
    }
}

fn push_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}

// ---- strict snapshot decode (untrusted bytes) -------------------------------

fn take_u64(buf: &mut &[u8]) -> Result<u64, Error> {
    let Some((head, rest)) = buf.split_first_chunk::<8>() else {
        return Err(Error::Module("snapshot truncated".into()));
    };
    *buf = rest;
    Ok(u64::from_le_bytes(*head))
}

fn take_vec(buf: &mut &[u8]) -> Result<Vec<u8>, Error> {
    let len = take_u64(buf)?;
    if len > buf.len() as u64 {
        return Err(Error::Module("snapshot length exceeds buffer".into()));
    }
    let (head, rest) = buf.split_at(len as usize);
    *buf = rest;
    Ok(head.to_vec())
}

fn take_string(buf: &mut &[u8]) -> Result<String, Error> {
    String::from_utf8(take_vec(buf)?).map_err(|_| Error::Module("snapshot: bad utf-8".into()))
}

fn decode_state(bytes: &[u8]) -> Result<BTreeMap<Vec<u8>, Record>, Error> {
    let mut buf = bytes;
    let count = take_u64(&mut buf)?;
    if count > (buf.len() / 8) as u64 {
        return Err(Error::Module("snapshot count exceeds buffer".into()));
    }
    let mut profiles = BTreeMap::new();
    let mut prev: Option<Vec<u8>> = None;
    for _ in 0..count {
        let key = take_vec(&mut buf)?;
        if prev.as_deref().is_some_and(|p| p >= key.as_slice()) {
            return Err(Error::Module(
                "snapshot keys must be strictly increasing".into(),
            ));
        }
        let display_name = take_string(&mut buf)?;
        // execute never commits an empty or over-long name (empty clears the
        // record, over-long is rejected), so an honest committed state always
        // holds a name in [1, MAX_NAME_LEN]. reject anything else -- and the
        // root comparison in `install` is the real integrity check regardless.
        if display_name.is_empty() || display_name.len() > MAX_NAME_LEN {
            return Err(Error::Module("snapshot display name out of bounds".into()));
        }
        let updated_at = take_u64(&mut buf)?;
        prev = Some(key.clone());
        profiles.insert(
            key,
            Record {
                display_name,
                updated_at,
            },
        );
    }
    if !buf.is_empty() {
        return Err(Error::Module("snapshot carries trailing bytes".into()));
    }
    Ok(profiles)
}
