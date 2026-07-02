//! deterministic in-memory inbox module: per-member notification queues held as
//! consensus state.
//!
//! other modules deliver notifications as FOLLOW-UP ops, so a notification
//! commits atomically in the same block as the event that caused it (platform
//! promise P2). there is no external push service — the queue IS the delivery,
//! which is also the air-gap-native notification story. an external submitter
//! may self-deliver a note; a module follow-up is the primary writer.
//!
//! like the tasks module this slice is state-based rather than qmdb-backed: the
//! API needs ordered per-member list/query semantics over a small canonical
//! state. writes are STAGED during `execute` (a per-member overlay), published
//! only at `commit_block`, and discarded at `abort_block`; `root()` is computed
//! from committed state alone, so a staged or aborted block leaves the root
//! byte-identical. `snapshot`/`install` use the exact canonical byte stream that
//! `root()` hashes, so a joiner verifies a peer image against the committed root
//! before adopting it.
//!
//! CAP POLICY (enforced at execute, with rejection, so oversized bytes never
//! enter the root preimage):
//! - `kind` <= 64 B, `body` <= 16 KiB, `member` non-empty and <= 256 B —
//!   an over-cap `Deliver` is REJECTED (fails the block).
//! - per member, at most [`MAX_ITEMS_PER_MEMBER`] items: when a delivery would
//!   overflow, the OLDEST item (lowest seq) is DROPPED deterministically. this
//!   is a notification queue, NOT a ledger — bounded memory beats total
//!   retention, and the drop is a pure function of committed state.
//! - at most [`MAX_MEMBERS`] distinct members: a `Deliver` that would introduce
//!   a NEW member beyond the cap is REJECTED.
//!
//! NO-OP TOLERANCE: `MarkRead`/`Clear` against an unknown member or seq are
//! deterministic no-ops, never errors — a notification ack must never abort the
//! block cascade that a delivering module started.

use std::collections::BTreeMap;

use inbox_interface::{
    InboxMsg, InboxQuery, InboxReply, MAX_BODY_BYTES, MAX_ITEMS_PER_MEMBER, MAX_KIND_BYTES,
    MAX_MEMBER_BYTES, MAX_MEMBERS, MAX_QUERY_LIMIT, Notification, decode_msg, decode_query,
    encode_reply,
};
use sdk::{Ctx, Env, Error, Module, ModuleId, Msg, Origin, StateRoot};
use sha2::{Digest, Sha256};

/// one member's queue: a monotonic seq counter plus its live items. `next_seq`
/// is the NEXT seq to assign; it starts at 1 and NEVER rewinds (a `Clear`
/// removes items but leaves `next_seq` alone, so replays and gap-free ordering
/// survive deletion).
#[derive(Clone, Debug, PartialEq, Eq)]
struct MemberQueue {
    next_seq: u64,
    items: BTreeMap<u64, Notification>,
}

impl MemberQueue {
    fn new() -> Self {
        Self {
            next_seq: 1,
            items: BTreeMap::new(),
        }
    }
}

pub struct Inbox {
    id: ModuleId,
    /// committed per-member queues, keyed by member identity.
    members: BTreeMap<String, MemberQueue>,
    /// staged overlay: a member present here shadows its committed queue for the
    /// duration of the block. published at `commit_block`, dropped at
    /// `abort_block`.
    pending: BTreeMap<String, MemberQueue>,
    /// number of staged members that are NOT yet committed — kept incrementally
    /// so the distinct-member cap check stays O(1) instead of re-unioning the
    /// two maps on every `Deliver`.
    new_pending: usize,
}

impl Inbox {
    pub fn new(id: impl Into<ModuleId>) -> Self {
        Self {
            id: id.into(),
            members: BTreeMap::new(),
            pending: BTreeMap::new(),
            new_pending: 0,
        }
    }

    /// the effective (staged-over-committed) queue for a member, for reads.
    fn effective(&self, member: &str) -> Option<&MemberQueue> {
        self.pending
            .get(member)
            .or_else(|| self.members.get(member))
    }

    /// distinct members currently known: committed members plus staged members
    /// not yet committed. O(1) via [`Inbox::new_pending`].
    fn distinct_members(&self) -> usize {
        self.members.len() + self.new_pending
    }

    /// stage a member queue for mutation, cloning the committed queue on first
    /// touch (or creating a fresh one for a brand-new member) and maintaining
    /// [`Inbox::new_pending`].
    fn stage_queue(&mut self, member: &str) -> &mut MemberQueue {
        if !self.pending.contains_key(member) {
            let (base, is_new) = match self.members.get(member) {
                Some(q) => (q.clone(), false),
                None => (MemberQueue::new(), true),
            };
            if is_new {
                self.new_pending += 1;
            }
            self.pending.insert(member.to_owned(), base);
        }
        self.pending
            .get_mut(member)
            .expect("staged queue just inserted")
    }

    fn validate_deliver(member: &str, kind: &str, body: &str) -> Result<(), Error> {
        if member.is_empty() {
            return Err(Error::Module("member must not be empty".into()));
        }
        if member.len() > MAX_MEMBER_BYTES {
            return Err(Error::Module(format!(
                "member exceeds {MAX_MEMBER_BYTES} bytes"
            )));
        }
        if kind.len() > MAX_KIND_BYTES {
            return Err(Error::Module(format!(
                "kind exceeds {MAX_KIND_BYTES} bytes"
            )));
        }
        if body.len() > MAX_BODY_BYTES {
            return Err(Error::Module(format!(
                "body exceeds {MAX_BODY_BYTES} bytes"
            )));
        }
        Ok(())
    }

    fn stage_deliver(
        &mut self,
        member: String,
        kind: String,
        body: String,
        source: String,
        created_at: u64,
    ) -> Result<(), Error> {
        Self::validate_deliver(&member, &kind, &body)?;

        // reject a NEW member beyond the cap BEFORE staging, so an over-cap
        // delivery never touches state.
        let is_new = !self.members.contains_key(&member) && !self.pending.contains_key(&member);
        if is_new && self.distinct_members() >= MAX_MEMBERS {
            return Err(Error::Module(format!(
                "inbox is at member capacity ({MAX_MEMBERS})"
            )));
        }

        // seq-space exhaustion is a deterministic rejection, checked BEFORE any
        // mutation — never a panic or a wrapping re-assignment of an old seq.
        let seq = self
            .effective(&member)
            .map(|queue| queue.next_seq)
            .unwrap_or(1);
        let bumped = seq
            .checked_add(1)
            .ok_or_else(|| Error::Module(format!("member seq space exhausted: {member}")))?;

        let queue = self.stage_queue(&member);
        queue.next_seq = bumped;
        queue.items.insert(
            seq,
            Notification {
                seq,
                member,
                kind,
                body,
                source,
                created_at,
                read: false,
            },
        );
        // overflow: drop the OLDEST (lowest seq) item. we insert exactly one per
        // call, so at most one drop is ever needed.
        while queue.items.len() > MAX_ITEMS_PER_MEMBER {
            let oldest = *queue
                .items
                .keys()
                .next()
                .expect("non-empty over-capacity queue");
            queue.items.remove(&oldest);
        }
        Ok(())
    }

    fn stage_mark_read(&mut self, member: String, up_to_seq: u64) {
        // unknown member: deterministic no-op (never stage, never error).
        if self.effective(&member).is_none() {
            return;
        }
        let queue = self.stage_queue(&member);
        for (_, item) in queue.items.range_mut(..=up_to_seq) {
            item.read = true;
        }
    }

    fn stage_clear(&mut self, member: String, up_to_seq: u64) {
        // unknown member: deterministic no-op.
        if self.effective(&member).is_none() {
            return;
        }
        let queue = self.stage_queue(&member);
        let doomed: Vec<u64> = queue
            .items
            .range(..=up_to_seq)
            .map(|(seq, _)| *seq)
            .collect();
        for seq in doomed {
            queue.items.remove(&seq);
        }
        // next_seq is intentionally left untouched: it never rewinds.
    }

    fn list(&self, member: &str, from_seq: u64, limit: u64) -> Vec<Notification> {
        let limit = limit.min(MAX_QUERY_LIMIT) as usize;
        match self.effective(member) {
            Some(queue) => queue
                .items
                .range(from_seq..)
                .take(limit)
                .map(|(_, item)| item.clone())
                .collect(),
            None => Vec::new(),
        }
    }

    fn unread(&self, member: &str) -> u64 {
        self.effective(member)
            .map(|queue| queue.items.values().filter(|item| !item.read).count() as u64)
            .unwrap_or(0)
    }

    fn root_of(members: &BTreeMap<String, MemberQueue>) -> StateRoot {
        let mut h = Sha256::new();
        h.update(Self::encode(members));
        StateRoot(h.finalize().into())
    }

    /// canonical byte encoding — the exact `root()` preimage AND the snapshot
    /// wire. deterministic: members ascend by identity, items ascend by seq,
    /// strings are length-prefixed, integers are little-endian. the member
    /// identity is encoded once at the queue level; each item's redundant
    /// `member` field is reconstructed from the key on decode.
    fn encode(members: &BTreeMap<String, MemberQueue>) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(members.len() as u64).to_le_bytes());
        for (member, queue) in members {
            push_string(&mut out, member);
            out.extend_from_slice(&queue.next_seq.to_le_bytes());
            out.extend_from_slice(&(queue.items.len() as u64).to_le_bytes());
            for item in queue.items.values() {
                out.extend_from_slice(&item.seq.to_le_bytes());
                push_string(&mut out, &item.kind);
                push_string(&mut out, &item.body);
                push_string(&mut out, &item.source);
                out.extend_from_slice(&item.created_at.to_le_bytes());
                out.push(item.read as u8);
            }
        }
        out
    }

    pub fn snapshot(&self) -> Vec<u8> {
        Self::encode(&self.members)
    }

    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        let members = decode_snapshot(bytes)?;
        if Self::root_of(&members) != expected {
            return Err(Error::Module("snapshot root mismatch".into()));
        }
        self.members = members;
        self.pending.clear();
        self.new_pending = 0;
        Ok(())
    }
}

/// derive the delivering `source` from the dispatch origin — the only source of
/// truth for who delivered. NEVER caller-supplied. a module is recorded as its
/// id verbatim; an external submitter as `"ext:"` + the lowercase hex of its id
/// bytes (empty bytes -> `"ext:"`); genesis / system-internal as `"system"`.
/// the `ext:` prefix is actor DOMAIN SEPARATION: a future module whose id
/// happens to be pure hex can never collide with an external key's hex.
fn source_from_origin(origin: &Origin) -> String {
    match origin {
        Origin::Module(id) => id.clone(),
        Origin::External(bytes) => format!("ext:{}", hex_lower(bytes)),
        Origin::System => "system".to_owned(),
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from_digit((b >> 4) as u32, 16).expect("nibble"));
        out.push(char::from_digit((b & 0x0f) as u32, 16).expect("nibble"));
    }
    out
}

fn push_string(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u64).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}

/// decode (and validate) a snapshot. install must accept every
/// execute-reachable state — the root comparison is the integrity check — but
/// states execute can NEVER produce are rejected as defense-in-depth: the caps
/// below are all enforced at execute time, so an image violating them is
/// corrupt or hostile, never an honest validator's state.
fn decode_snapshot(bytes: &[u8]) -> Result<BTreeMap<String, MemberQueue>, Error> {
    let mut off = 0usize;
    let member_count = read_u64(bytes, &mut off)?;
    if member_count > MAX_MEMBERS as u64 {
        return Err(Error::Module("snapshot exceeds member capacity".into()));
    }

    let mut members: BTreeMap<String, MemberQueue> = BTreeMap::new();
    for _ in 0..member_count {
        let member = read_string(bytes, &mut off)?;
        if member.is_empty() {
            return Err(Error::Module("snapshot member id is empty".into()));
        }
        if member.len() > MAX_MEMBER_BYTES {
            return Err(Error::Module("snapshot member id exceeds cap".into()));
        }
        if members
            .last_key_value()
            .is_some_and(|(last, _)| last.as_str() >= member.as_str())
        {
            return Err(Error::Module(
                "snapshot member ids not strictly ascending".into(),
            ));
        }

        let next_seq = read_u64(bytes, &mut off)?;
        if next_seq == 0 {
            // next_seq starts at 1 and only ever increments.
            return Err(Error::Module("snapshot next_seq is zero".into()));
        }
        let item_count = read_u64(bytes, &mut off)?;
        if item_count > MAX_ITEMS_PER_MEMBER as u64 {
            return Err(Error::Module(
                "snapshot member queue exceeds item capacity".into(),
            ));
        }
        let mut items: BTreeMap<u64, Notification> = BTreeMap::new();
        for _ in 0..item_count {
            let seq = read_u64(bytes, &mut off)?;
            if items.last_key_value().is_some_and(|(last, _)| *last >= seq) {
                return Err(Error::Module(
                    "snapshot item seqs not strictly ascending".into(),
                ));
            }
            if seq >= next_seq {
                // every assigned seq was strictly below next_seq at assignment;
                // a seq at/above it is not execute-reachable.
                return Err(Error::Module("snapshot item seq exceeds next_seq".into()));
            }
            let kind = read_string(bytes, &mut off)?;
            if kind.len() > MAX_KIND_BYTES {
                return Err(Error::Module("snapshot kind exceeds cap".into()));
            }
            let body = read_string(bytes, &mut off)?;
            if body.len() > MAX_BODY_BYTES {
                return Err(Error::Module("snapshot body exceeds cap".into()));
            }
            let source = read_string(bytes, &mut off)?;
            let created_at = read_u64(bytes, &mut off)?;
            let read = read_bool(bytes, &mut off)?;
            items.insert(
                seq,
                Notification {
                    seq,
                    member: member.clone(),
                    kind,
                    body,
                    source,
                    created_at,
                    read,
                },
            );
        }
        members.insert(member, MemberQueue { next_seq, items });
    }
    if off != bytes.len() {
        return Err(Error::Module("snapshot has trailing bytes".into()));
    }
    Ok(members)
}

fn read_bool(bytes: &[u8], off: &mut usize) -> Result<bool, Error> {
    match read_u8(bytes, off)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(Error::Module("snapshot read flag is not 0/1".into())),
    }
}

fn read_u8(bytes: &[u8], off: &mut usize) -> Result<u8, Error> {
    let end = off
        .checked_add(1)
        .filter(|&end| end <= bytes.len())
        .ok_or_else(|| Error::Module("snapshot truncated".into()))?;
    let value = bytes[*off];
    *off = end;
    Ok(value)
}

fn read_u64(bytes: &[u8], off: &mut usize) -> Result<u64, Error> {
    let end = off
        .checked_add(8)
        .filter(|&end| end <= bytes.len())
        .ok_or_else(|| Error::Module("snapshot truncated".into()))?;
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&bytes[*off..end]);
    *off = end;
    Ok(u64::from_le_bytes(buf))
}

fn read_string(bytes: &[u8], off: &mut usize) -> Result<String, Error> {
    let len = read_u64(bytes, off)?;
    let len = usize::try_from(len).map_err(|_| Error::Module("snapshot truncated".into()))?;
    if len > bytes.len() - *off {
        return Err(Error::Module("snapshot truncated".into()));
    }
    let value = std::str::from_utf8(&bytes[*off..*off + len])
        .map_err(|_| Error::Module("snapshot string is not utf-8".into()))?;
    *off += len;
    Ok(value.to_owned())
}

#[async_trait::async_trait(?Send)]
impl Module for Inbox {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        Self::root_of(&self.members)
    }

    /// advertise the snapshot lane: [`Inbox::snapshot`] is the exact preimage of
    /// `root()`, and [`Inbox::install`] verifies before adopting.
    fn state_sync_handle(&self) -> Result<sdk::StateSyncHandle, Error> {
        Ok(sdk::StateSyncHandle::SnapshotBytes(self.snapshot()))
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let Env { consensus_time, .. } = *ctx.env();
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            InboxMsg::Deliver { member, kind, body } => {
                let source = source_from_origin(&ctx.env().origin);
                self.stage_deliver(member, kind, body, source, consensus_time)
            }
            InboxMsg::MarkRead { member, up_to_seq } => {
                self.stage_mark_read(member, up_to_seq);
                Ok(())
            }
            InboxMsg::Clear { member, up_to_seq } => {
                self.stage_clear(member, up_to_seq);
                Ok(())
            }
        }
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            InboxQuery::List {
                member,
                from_seq,
                limit,
            } => Ok(encode_reply(&InboxReply::Items(
                self.list(&member, from_seq, limit),
            ))),
            InboxQuery::Unread { member } => {
                Ok(encode_reply(&InboxReply::UnreadCount(self.unread(&member))))
            }
        }
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        for (member, queue) in std::mem::take(&mut self.pending) {
            self.members.insert(member, queue);
        }
        self.new_pending = 0;
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending.clear();
        self.new_pending = 0;
        Ok(())
    }
}
