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
//! - `Deliver`'s authority is split by WHO is delivering, because the acl
//!   table is empty/open at genesis (`crates/modules/system/acl`) and nothing
//!   in noded sets a policy for inbox: a `Module`/`System` origin (a
//!   follow-up from chat, tasks, automations, …) may deliver to ANY member,
//!   unchanged — writing into someone else's queue IS the feature there, and
//!   gating it would break every module follow-up. an `External` origin may
//!   deliver only to its OWN queue ([`deliver_is_permitted`]): otherwise a raw
//!   signed op with no follow-up behind it could mint an unbounded number of
//!   fabricated members (exhausting [`MAX_MEMBERS`] forever, since nothing
//!   ever decrements it — see `MemberMeta`) or flood a real member's queue
//!   past [`MAX_ITEMS_PER_MEMBER`] to evict their genuine notifications.
//! - `MarkRead`/`Clear` are refused unless `member` is the submitter's own
//!   actor string ([`check_queue_owner`]). a submitter can therefore only ever
//!   name their own queue, and "permanently delete another member's whole
//!   notification history, unattributed" stops being expressible.
//! - only an AUTHENTICATED EXTERNAL submitter owns a queue. `Origin::Module`
//!   and `Origin::System` own none (refused outright for acking, and
//!   unrestricted rather than self-scoped for delivering), as does
//!   the pre-consensus default `Origin::External(vec![])`: nothing in the tree
//!   emits an ack as a follow-up, so admitting a module origin would only have
//!   handed the delivering module a lever over the queue it delivered to —
//!   different principals, deliberately kept different.
//!
//! ## State model
//!
//! pure logic over a host-injected [`sdk::MerkleStore`]: one META record per
//! member (`meta\0{member}` → next_seq + the sorted live-seq list + the read
//! watermark, borsh),
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
//!   retention, and the drop is a pure function of committed state. with
//!   strangers unable to `Deliver` to a queue they don't own, an eviction is
//!   now self-inflicted (a member spamming its own queue) or module-driven
//!   (a producer's own bug); [`MemberMeta::evicted`] counts them per member so
//!   that loss stays visible instead of silent.
//! - at most [`MAX_MEMBERS`] distinct members: a `Deliver` that would introduce
//!   a NEW member beyond the cap is REJECTED, and the counter FALLS when a
//!   `Clear` empties a member's queue entirely: `stage_clear` then deletes the
//!   META record along with the last item, handing the slot back. `next_seq`
//!   is deliberately lost with it — a later delivery to that same member
//!   re-mints its seq space from 1, which is the whole point: a queue that is
//!   cleared and never redelivered to no longer counts against the cap.
//!
//! NO-OP TOLERANCE: `MarkRead`/`Clear` against the submitter's OWN unknown
//! member or seq are deterministic no-ops, never errors — acking a queue that
//! holds nothing yet is a race a client cannot avoid, not an error. that
//! tolerance is scoped to the seq/member LOOKUP and stops at the owner gate: a
//! foreign member is a hard rejection, and no cascade is at risk from it
//! because nothing in the tree emits an ack as a follow-up.
//!
//! READ TRACKING is a per-member WATERMARK ([`MemberMeta::read_watermark`]),
//! never a per-item flag: `stage_mark_read` costs exactly one meta read and
//! (at most) one meta write, regardless of queue length. A queue at
//! [`MAX_ITEMS_PER_MEMBER`] would otherwise need one distinct store read per
//! live item to flip each `read` bit — with the wasm host's per-dispatch
//! store-read budget also capped at 4096, a full queue's `MarkRead` could
//! never complete. Nothing marks a single item read in isolation (`MarkRead`
//! is always a range), so there is no per-item `read` field to keep in sync:
//! every read path (the index guest's list/unread view) derives `read` as
//! `seq <= read_watermark`. the watermark is CLAMPED to the last seq ever
//! assigned (`next_seq - 1`): the old per-item flag could only ever touch
//! items that already existed, so an unclamped `MarkRead { up_to_seq:
//! u64::MAX }` would otherwise mark every FUTURE delivery pre-read on
//! arrival — a regression, not a rewrite of the same behavior.

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

/// the queue an origin owns, for a principal that CAN own one — an
/// authenticated external submitter, identified the same way
/// [`Notification::source`] renders it. `Origin::Module` and `Origin::System`
/// own no queue (no module or system op emits an ack anywhere in the tree,
/// and admitting one would give a DELIVERING module a lever over the queue it
/// delivered into); `Origin::External(vec![])` is the host's pre-consensus
/// default, not a submitter, so it owns nothing either. shared by
/// [`check_queue_owner`] (the ack gate) and [`deliver_is_permitted`] (the
/// external half of the deliver gate) so the two authorities derive "whose
/// queue is this" identically.
fn owned_queue(origin: &Origin) -> Result<String, Error> {
    match origin {
        Origin::External(key) if !key.is_empty() => Ok(origin.actor_string()),
        Origin::External(_) => Err(Error::Module(
            "external origin must carry a non-empty submitter id".into(),
        )),
        Origin::Module(id) => Err(Error::Module(format!(
            "a module origin owns no inbox queue: {id}"
        ))),
        Origin::System => Err(Error::Module("a system origin owns no inbox queue".into())),
    }
}

/// the ONE ack-authority decision: `MarkRead`/`Clear` may only touch the
/// submitter's OWN queue. exhaustive on purpose — the owner is derived from
/// the dispatch origin via [`owned_queue`] and compared, never taken from the
/// payload.
fn check_queue_owner(origin: &Origin, member: &str) -> Result<(), Error> {
    let owner = owned_queue(origin)?;
    let is_own_queue = owner == member;
    if !is_own_queue {
        return Err(Error::Module(format!(
            "only the queue's own member may ack it: {owner} is not {member}"
        )));
    }
    Ok(())
}

/// the ONE deliver-authority decision (issues closed: an unbounded member-mint
/// flood that permanently exhausts [`MAX_MEMBERS`], and an unattributed
/// queue-eviction flood past [`MAX_ITEMS_PER_MEMBER`]). a `Module`/`System`
/// origin — a follow-up from chat, tasks, automations, … — may deliver to ANY
/// member: that is the module's whole purpose, and every module follow-up
/// depends on it. an `External` origin, reachable directly by any validly
/// signed op (the acl table is empty/open at genesis), may deliver only to
/// the queue [`owned_queue`] derives for it — the same restriction acking
/// already has, extended to writing.
fn deliver_is_permitted(origin: &Origin, member: &str) -> bool {
    match origin {
        Origin::Module(_) | Origin::System => true,
        Origin::External(_) => matches!(owned_queue(origin), Ok(owner) if owner == member),
    }
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

/// one member's queue metadata: the monotonic seq counter, the sorted
/// live-seq list, and the eviction counter. `next_seq` is the NEXT seq to
/// assign; it starts at 1 and never rewinds WHILE the record survives (a
/// `Clear` that leaves at least one live item removes the cleared items but
/// leaves `next_seq` alone, so replays and gap-free ordering survive
/// deletion). the record itself does NOT survive a `Clear` that empties the
/// queue entirely — `stage_clear` deletes it and gives back the member's slot
/// in [`MEMBER_COUNT_KEY`] — so a later delivery to that member re-mints a
/// fresh `MemberMeta` starting at seq 1. `seqs` is bounded by construction: at
/// most [`MAX_ITEMS_PER_MEMBER`] entries. `evicted` counts every item this
/// member has ever lost to the overflow drop below — the queue's own
/// visible tally of otherwise-silent loss. `read_watermark` is the seq up to
/// which every item is read: `MarkRead` only ever raises it, so a read item
/// is exactly one whose `seq <= read_watermark` — no per-item flag exists to
/// desync from it.
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
struct MemberMeta {
    next_seq: u64,
    seqs: Vec<u64>,
    evicted: u64,
    read_watermark: u64,
}

impl MemberMeta {
    fn new() -> Self {
        Self {
            next_seq: 1,
            seqs: Vec::new(),
            evicted: 0,
            read_watermark: 0,
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
    /// record is a store bug — loud, never skipped. only `queue_view`
    /// (testkit) still reads individual items: every real read path derives
    /// `read` from the watermark instead of loading records one at a time.
    #[cfg(feature = "testkit")]
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
        // per call, so at most one drop is ever needed. counted in `evicted`
        // so the loss stays visible instead of silent.
        while meta.seqs.len() > MAX_ITEMS_PER_MEMBER {
            let oldest = meta.seqs.remove(0);
            self.staged.delete(item_key(&member, oldest));
            meta.evicted += 1;
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
            },
        );
        self.store(meta_key(&member), &meta);
        Ok(seq)
    }

    /// O(1): one meta read, at most one meta write — never a per-item read or
    /// write. `read_watermark` only ever rises, so an `up_to_seq` at or below
    /// it is a byte-identical no-op (idempotent re-acks never move the root).
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
        let Some(mut meta) = self.meta(&member).await? else {
            return Ok(());
        };
        // clamp to the last seq ever ASSIGNED (`next_seq - 1`), never the raw
        // `up_to_seq`: the old per-item flag could only ever touch items that
        // already existed, and an unclamped watermark would let `up_to_seq =
        // u64::MAX` mark every FUTURE delivery pre-read on arrival — a real
        // regression, not just a difference in mechanism.
        let last_seq = meta.next_seq.saturating_sub(1);
        let watermark = up_to_seq.min(last_seq);
        if watermark <= meta.read_watermark {
            return Ok(());
        }
        meta.read_watermark = watermark;
        self.store(meta_key(&member), &meta);
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
        if meta.seqs.is_empty() {
            // the queue is now empty: delete the META record entirely and
            // give back its slot in the distinct-member cap — a queue that is
            // cleared and never redelivered to no longer counts against
            // MAX_MEMBERS. `next_seq` is deliberately lost with it: a later
            // delivery to this member re-mints a fresh MemberMeta from seq 1,
            // which is fine — nothing in the store still refers to the old
            // seq space once every item in it is gone.
            self.staged.delete(meta_key(&member));
            let count = self.member_count().await?;
            self.store(MEMBER_COUNT_KEY.to_vec(), &count.saturating_sub(1));
            return Ok(());
        }
        // next_seq is intentionally left untouched: it never rewinds while
        // the member has at least one live item — the meta record persists
        // so replays stay gap-free.
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
                // truth for who delivered, NEVER caller-supplied. a module/system
                // follow-up may deliver to ANY member (the module's whole
                // purpose); an external origin may deliver only to its OWN
                // queue (`deliver_is_permitted`) — the acl gap this closes.
                if !deliver_is_permitted(&origin, &member) {
                    let source = origin.actor_string();
                    return Err(Error::Module(format!(
                        "an external origin may only deliver to its own queue: {source} is not {member}"
                    )));
                }
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

    /// the number of items this member has ever lost to the overflow drop —
    /// `0` for a member never delivered to or whose queue was later fully
    /// cleared (the counter rides the META record, so it goes with it).
    pub async fn evicted_count(&self, member: &str) -> Result<u64, Error> {
        Ok(self.meta(member).await?.map(|m| m.evicted).unwrap_or(0))
    }

    /// the distinct-member counter the cap reads — exposed so a test can
    /// assert it actually falls, not just that a fresh member is admitted.
    pub async fn member_count_view(&self) -> Result<u64, Error> {
        self.member_count().await
    }

    /// a member's read watermark — everything at or below it reads as read.
    /// `0` (never marked) for a member never delivered to.
    pub async fn read_watermark_view(&self, member: &str) -> Result<u64, Error> {
        Ok(self
            .meta(member)
            .await?
            .map(|m| m.read_watermark)
            .unwrap_or(0))
    }

    /// whether `seq` in `member`'s queue currently reads as read — derived
    /// from the watermark, exactly like every real read path.
    pub async fn is_read(&self, member: &str, seq: u64) -> Result<bool, Error> {
        Ok(seq <= self.read_watermark_view(member).await?)
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
