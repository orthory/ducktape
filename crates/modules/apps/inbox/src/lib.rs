//! qmdb-backed inbox module: per-member notification queues held as
//! consensus state.
//!
//! other modules deliver notifications as FOLLOW-UP ops, so a notification
//! commits atomically in the same block as the event that caused it (platform
//! promise P2). there is no external push service — the queue IS the delivery,
//! which is also the air-gap-native notification story. an external submitter
//! may self-deliver a note; a module follow-up is the primary writer.
//!
//! ## Who owns a queue — DELIVERING and ACKING are different authorities
//!
//! a `member` is a queue name in the shared ACTOR-STRING domain
//! ([`sdk::Origin::actor_string`]): the same domain this module already derives
//! [`Notification::source`] in, and the one tasks' job board and files' owner
//! use. that is the identity decision this module had deferred, and reusing the
//! sdk convention rather than inventing a second spelling is the whole of it —
//! a queue named `origin.actor_string()` is OWNED by that origin.
//!
//! - `Deliver` takes ANY origin and ANY member, unchanged. writing into someone
//!   else's queue IS the feature: a notification is delivered BY one principal
//!   TO another, and gating that would break every module follow-up.
//! - `MarkRead`/`Clear` are refused unless `member` is the submitter's own
//!   actor string ([`check_queue_owner`]). a submitter can therefore only ever
//!   name their own queue, and "permanently delete another member's whole
//!   notification history, unattributed" stops being expressible.
//! - only an AUTHENTICATED EXTERNAL submitter owns a queue. `Origin::Module`
//!   and `Origin::System` are refused outright, as is
//!   the pre-consensus default `Origin::External(vec![])`: nothing in the tree
//!   emits an ack as a follow-up, so admitting a module origin would only have
//!   handed the delivering module a lever over the queue it delivered to —
//!   different principals, deliberately kept different.
//!
//! ## State model
//!
//! pure logic over a host-injected [`sdk::MerkleStore`]: one META record per
//! member (`meta\0{member}` → next_seq + the sorted live-seq list, borsh),
//! one record per live notification (`item\0{len|member}{seq}`), and the
//! `member_count` scalar the distinct-member cap reads — every record is
//! bounded by the field caps below, and NOTHING enumerates members (the
//! whole read surface lives on the index tier), so no roster exists. writes
//! are staged during a block and flushed in one batch at `commit_block`; the
//! module root IS the store's merkle root, and sync belongs to the store
//! (`QmdbStore::sync_from`).
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
//! NO-OP TOLERANCE: `MarkRead`/`Clear` against the submitter's OWN unknown
//! member or seq are deterministic no-ops, never errors — acking a queue that
//! holds nothing yet is a race a client cannot avoid, not an error. that
//! tolerance is scoped to the seq/member LOOKUP and stops at the owner gate: a
//! foreign member is a hard rejection, and no cascade is at risk from it
//! because nothing in the tree emits an ack as a follow-up.

// the wire surface: this module's shared types, flattened at the crate root.
mod interface;
pub use interface::*;

// the derived-tier read model: the PURE decision core (fold + view over
// index_guest::StateRead), compiled everywhere and unit-tested natively.
// the engine shell that runs it inside the module's index database is
// `index_guest` below.
pub mod index;

// the CLIENT view model: the rendered bell item + the member-scoped delta
// fold a feed-following UI splices with. pure, ui.wasm-portable.
pub mod client;

// the wasm index-mapper shell: wires the pure core into the fluent31 engine.
// compiled only by `guest-builder --index`'s synthesized wasm32 workspace
// (feature `index-guest`), never by the native build.
#[cfg(feature = "index-guest")]
mod index_guest;

use borsh::{BorshDeserialize, BorshSerialize};
use sdk::{
    Ctx, Error, MerkleStore, Module, ModuleId, Msg, Origin, ResolverSyncTarget, StagedStore,
    StateRoot, StateSyncHandle,
};

/// the ONE ack-authority decision: `MarkRead`/`Clear` may only touch the
/// submitter's OWN queue.
///
/// exhaustive on purpose. a queue's name IS its owner's
/// [`Origin::actor_string`], so the owner is derived from the dispatch origin
/// and compared — never taken from the payload. `Origin::Module` and
/// `Origin::System` are refused outright rather than allowed to ack the queue
/// named after them: no module or system op emits an ack anywhere in the tree,
/// and admitting one would give a DELIVERING module a lever over the queue it
/// delivered into. `Origin::External(vec![])` is the host's pre-consensus
/// default, not a submitter, so it owns nothing either.
fn check_queue_owner(origin: &Origin, member: &str) -> Result<(), Error> {
    let owner = match origin {
        Origin::External(key) => {
            let is_authenticated_submitter = !key.is_empty();
            if !is_authenticated_submitter {
                return Err(Error::Module(
                    "external origin must carry a non-empty submitter id".into(),
                ));
            }
            origin.actor_string()
        }
        Origin::Module(id) => {
            return Err(Error::Module(format!(
                "a module origin owns no inbox queue: {id}"
            )));
        }
        Origin::System => {
            return Err(Error::Module("a system origin owns no inbox queue".into()));
        }
    };
    let is_own_queue = owner == member;
    if !is_own_queue {
        return Err(Error::Module(format!(
            "only the queue's own member may ack it: {owner} is not {member}"
        )));
    }
    Ok(())
}

/// per-member META record key: prefix + 0 + member identity. safe because
/// every key literal below is fixed and none is another followed by a 0 byte.
fn meta_key(member: &str) -> Vec<u8> {
    let mut key = Vec::with_capacity(4 + 1 + member.len());
    key.extend_from_slice(b"meta");
    key.push(0);
    key.extend_from_slice(member.as_bytes());
    key
}

/// per-notification record key: prefix + 0 + length-framed member + big-endian
/// seq. the length frame keeps the key injective for arbitrary member bytes.
fn item_key(member: &str, seq: u64) -> Vec<u8> {
    let mut key = Vec::with_capacity(4 + 1 + 8 + member.len() + 8);
    key.extend_from_slice(b"item");
    key.push(0);
    key.extend_from_slice(&(member.len() as u64).to_le_bytes());
    key.extend_from_slice(member.as_bytes());
    key.extend_from_slice(&seq.to_be_bytes());
    key
}

/// the distinct-member counter's whole key — the ONE aggregate the member cap
/// reads (a full member roster would be a 16 MiB poison record at the cap;
/// nothing enumerates members, so a scalar count is the honest aggregate).
const MEMBER_COUNT_KEY: &[u8] = b"member_count";

/// one member's queue metadata: the monotonic seq counter plus the sorted
/// live-seq list. `next_seq` is the NEXT seq to assign; it starts at 1 and
/// NEVER rewinds (a `Clear` removes items but leaves `next_seq` alone, so
/// replays and gap-free ordering survive deletion). bounded by construction:
/// at most [`MAX_ITEMS_PER_MEMBER`] seqs.
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
struct MemberMeta {
    next_seq: u64,
    seqs: Vec<u64>,
}

impl MemberMeta {
    fn new() -> Self {
        Self {
            next_seq: 1,
            seqs: Vec::new(),
        }
    }
}

pub struct Inbox {
    id: ModuleId,
    /// the host-injected authenticated store plus this block's staging overlay
    /// (read-your-writes, folded into `root()` at `commit_block`). store key
    /// is `sha256(logical_key)`, owned by [`StagedStore`].
    staged: StagedStore,
}

impl Inbox {
    /// wrap the host-constructed store under module identity `id`.
    pub fn new(id: impl Into<ModuleId>, store: Box<dyn MerkleStore>) -> Self {
        Self {
            id: id.into(),
            staged: StagedStore::new(store),
        }
    }

    // ---- staged-over-committed reads ----------------------------------------

    async fn load<T>(&self, key: &[u8]) -> Result<Option<T>, Error>
    where
        T: BorshDeserialize,
    {
        match self.staged.get(key).await? {
            Some(bytes) => Ok(Some(
                borsh::from_slice(&bytes).map_err(|e| Error::Module(e.to_string()))?,
            )),
            None => Ok(None),
        }
    }

    /// stage a value — every inbox record is bounded by construction (the
    /// field caps below), so no byte gate is needed.
    fn store<T>(&mut self, key: Vec<u8>, value: &T)
    where
        T: BorshSerialize,
    {
        self.staged.stage(
            key,
            borsh::to_vec(value).expect("inbox value is serializable"),
        );
    }

    async fn meta(&self, member: &str) -> Result<Option<MemberMeta>, Error> {
        self.load(&meta_key(member)).await
    }

    /// a live item the meta's seq list points at. a listed seq without its
    /// record is a store bug — loud, never skipped.
    async fn item(&self, member: &str, seq: u64) -> Result<Notification, Error> {
        self.load(&item_key(member, seq))
            .await?
            .ok_or_else(|| Error::Module("missing notification record".into()))
    }

    /// distinct members ever delivered to — the cap denominator.
    async fn member_count(&self) -> Result<u64, Error> {
        Ok(self.load(MEMBER_COUNT_KEY).await?.unwrap_or(0))
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

    async fn stage_deliver(
        &mut self,
        member: String,
        kind: String,
        body: String,
        source: String,
        created_at: u64,
    ) -> Result<u64, Error> {
        Self::validate_deliver(&member, &kind, &body)?;

        // reject a NEW member beyond the cap BEFORE staging, so an over-cap
        // delivery never touches state.
        let current = self.meta(&member).await?;
        if current.is_none() {
            let count = self.member_count().await?;
            if count >= MAX_MEMBERS as u64 {
                return Err(Error::Module(format!(
                    "inbox is at member capacity ({MAX_MEMBERS})"
                )));
            }
            self.store(MEMBER_COUNT_KEY.to_vec(), &(count + 1));
        }
        let mut meta = current.unwrap_or_else(MemberMeta::new);

        // seq-space exhaustion is a deterministic rejection, checked BEFORE any
        // mutation — never a panic or a wrapping re-assignment of an old seq.
        let seq = meta.next_seq;
        meta.next_seq = seq
            .checked_add(1)
            .ok_or_else(|| Error::Module(format!("member seq space exhausted: {member}")))?;

        meta.seqs.push(seq);
        // overflow: drop the OLDEST (lowest seq) item. we insert exactly one
        // per call, so at most one drop is ever needed.
        while meta.seqs.len() > MAX_ITEMS_PER_MEMBER {
            let oldest = meta.seqs.remove(0);
            self.staged.delete(item_key(&member, oldest));
        }
        self.store(
            item_key(&member, seq),
            &Notification {
                seq,
                member: member.clone(),
                kind,
                body,
                source,
                created_at,
                read: false,
            },
        );
        self.store(meta_key(&member), &meta);
        Ok(seq)
    }

    async fn stage_mark_read(
        &mut self,
        origin: &Origin,
        member: String,
        up_to_seq: u64,
    ) -> Result<(), Error> {
        // BEFORE the unknown-member short-circuit: a gate a no-op walks past is
        // not a gate, and a stranger must not learn whether a queue exists from
        // which answer comes back.
        check_queue_owner(origin, &member)?;
        // unknown member: deterministic no-op (never stage, never error).
        let Some(meta) = self.meta(&member).await? else {
            return Ok(());
        };
        for seq in meta.seqs.iter().take_while(|s| **s <= up_to_seq) {
            let mut item = self.item(&member, *seq).await?;
            if !item.read {
                item.read = true;
                self.store(item_key(&member, *seq), &item);
            }
        }
        Ok(())
    }

    async fn stage_clear(
        &mut self,
        origin: &Origin,
        member: String,
        up_to_seq: u64,
    ) -> Result<(), Error> {
        // BEFORE the unknown-member short-circuit (see `stage_mark_read`).
        check_queue_owner(origin, &member)?;
        // unknown member: deterministic no-op.
        let Some(mut meta) = self.meta(&member).await? else {
            return Ok(());
        };
        let keep = meta.seqs.partition_point(|s| *s <= up_to_seq);
        for seq in meta.seqs.drain(..keep) {
            self.staged.delete(item_key(&member, seq));
        }
        // next_seq is intentionally left untouched: it never rewinds — the
        // (possibly item-less) meta record persists so replays stay gap-free.
        self.store(meta_key(&member), &meta);
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Inbox {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// the store's merkle root over all committed records, verbatim — the
    /// staged overlay is invisible here until `commit_block`.
    fn root(&self) -> StateRoot {
        self.staged.root()
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        self.staged.state_sync_handle()
    }

    /// the network state-sync serve lane: answers the shared qmdb wire requests
    /// (historical proof-carrying op ranges) from committed state. read-only;
    /// the joiner's sync engine merkle-verifies every batch.
    async fn serve_sync(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        self.staged.serve_sync(req).await
    }

    async fn resolver_sync_target(&self) -> Result<ResolverSyncTarget, Error> {
        self.staged.sync_target().await
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        // the submitter is bound ONCE, before the payload is decoded, and every
        // arm receives it — an arm that took no origin would be exactly the
        // class of bug the ack gate exists to close. `Deliver` uses it to derive
        // `source`; the ack family uses it to derive the queue owner.
        let consensus_time = ctx.env().consensus_time;
        let origin = ctx.env().origin.clone();
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            InboxMsg::Deliver { member, kind, body } => {
                // the delivering `source` is origin-derived — the only source of
                // truth for who delivered, NEVER caller-supplied. ANY origin may
                // deliver to ANY member: that is the module's purpose, and it is
                // routed here without ever reaching the owner gate.
                let source = origin.actor_string();
                let seq = self
                    .stage_deliver(member, kind, body, source, consensus_time)
                    .await?;
                ctx.set_assigned(encode_assigned(&InboxAssigned::Delivered { seq }));
                Ok(())
            }
            InboxMsg::MarkRead { member, up_to_seq } => {
                self.stage_mark_read(&origin, member, up_to_seq).await
            }
            InboxMsg::Clear { member, up_to_seq } => {
                self.stage_clear(&origin, member, up_to_seq).await
            }
        }
    }

    // NO `query`: nothing in any execute() path reads an inbox, so the whole
    // read surface (paged lists, unread counts) is the index guest's job
    // (`index.rs`) on the derived tier. the default `Error::QueryUnsupported`
    // is the honest answer here.

    /// publish the block's staged writes in ONE store batch. no-op (and no
    /// root movement) if nothing was staged.
    async fn commit_block(&mut self) -> Result<(), Error> {
        self.staged.commit().await
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.staged.abort();
        Ok(())
    }
}

// test-only inspection reads. dev-only: inbox deliberately has NO wire query
// surface (the index tier owns every read), so the state-side tests probe the
// records through this feature-gated seam instead of golden byte images.
#[cfg(feature = "testkit")]
impl Inbox {
    /// one member's staged-over-committed queue: `(next_seq, live items in seq
    /// order)`; `None` for a member never delivered to.
    pub async fn queue_view(
        &self,
        member: &str,
    ) -> Result<Option<(u64, Vec<Notification>)>, Error> {
        let Some(meta) = self.meta(member).await? else {
            return Ok(None);
        };
        let mut items = Vec::with_capacity(meta.seqs.len());
        for seq in &meta.seqs {
            items.push(self.item(member, *seq).await?);
        }
        Ok(Some((meta.next_seq, items)))
    }

    /// stage a member whose seq space is one delivery from exhaustion — the
    /// boundary state is execute-reachable only after 2^64 - 2 deliveries, so
    /// the exhaustion test injects it instead.
    pub async fn testkit_saturate_seq(&mut self, member: &str) -> Result<(), Error> {
        if self.meta(member).await?.is_none() {
            let count = self.member_count().await?;
            self.store(MEMBER_COUNT_KEY.to_vec(), &(count + 1));
        }
        let mut meta = self.meta(member).await?.unwrap_or_else(MemberMeta::new);
        meta.next_seq = u64::MAX;
        self.store(meta_key(member), &meta);
        Ok(())
    }
}

// the wasm-guest port: the dispatch shell that adapts this module to the
// ducktape:module world. compiled only by the guest-builder's synthesized
// wasm32 cdylib workspace (feature `guest`), never by the native build.
#[cfg(feature = "guest")]
mod guest;
