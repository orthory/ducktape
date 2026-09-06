//! qmdb-backed chat module: block-based channels, threads, edits, tombstones,
//! reactions, membership, and hook notifications.
//!
//! pure logic over a host-injected [`sdk::MerkleStore`]: the HOST constructs
//! the concrete store (qmdb today — `statesync::qmdb::QmdbStore`) and hands it
//! to [`Chat::new`], so this crate never names a storage crate. the store is
//! used for what it is — hash-addressable authenticated state, one logical
//! record per entity, every read a point lookup the DISPATCH path needs:
//! per-channel records, one record per message head, immutable per-edit
//! revision records, per-emoji reaction sets (plus the bounded per-message
//! emoji index the caps and tombstone cleanup require), message-id pointers
//! for global dedup, and membership records for the post policy. no stored
//! enumeration lists, no stand-in range indexes: everything a human scrolls,
//! lists, or searches is served by the index guest (`index.rs`) on the
//! derived tier. the one computed-key iteration that remains — the
//! `MessagesRange` context window over the gap-free `head_seq` space — exists
//! because CONSENSUS consumers (runs, automations) read it in `execute()`,
//! and consensus can never depend on the unverifiable derived tier.
//!
//! authorship is derived from `ctx.env().origin` on every write; payloads
//! carry no author field. an empty external origin (the pre-consensus default)
//! is rejected. like `document` and `kv`, writes are staged in memory during a
//! block and flushed to the store in one batch at `commit_block`; the module
//! root IS the store's merkle root. sync belongs to the store, not this
//! module: a joiner rebuilds the concrete store from a peer
//! (`QmdbStore::sync_from`) and wraps a fresh `Chat` around it — this module
//! only forwards the trait's serve surface.

// the wire surface: this module's shared types, flattened at the crate root.
mod interface;
pub use interface::*;

// the wasm-guest port: the dispatch shell that adapts this module to the
// ducktape:module world. compiled only by the guest-builder's synthesized
// wasm32 cdylib workspace (feature `guest`), never by the native build.
#[cfg(feature = "guest")]
mod guest;
// everything below is OFF-consensus: none of it touches qmdb or the
// root-hash, and the index engine's deps (fluent31 IO) cannot cross into the
// wasm guest — so the consensus state machine above compiles for wasm32
// without them. (The call media planes live in the `media-service` crate.)
//
// the derived-tier materialized view: the PURE decision core (fold + view
// over index_guest::StateRead), compiled everywhere and unit-tested
// natively. the engine shell that runs it inside the module's index
// database is `index_guest` below.
pub mod index;

// the CLIENT view model: rendered row types, composer parsing, optimistic
// merges, and the op-delta fold a feed-following UI splices state with.
// module-owned beside the index fold (same feed, same vocabulary); pure
// data-in/data-out, so the module-bundled-UI lane can compile it into the
// shipped ui.wasm unchanged.
pub mod client;

// the wasm index-mapper shell: wires the pure core into the fluent31 engine.
// compiled only by `guest-builder --index`'s synthesized wasm32 workspace
// (feature `index-guest`), never by the native build.
#[cfg(feature = "index-guest")]
mod index_guest;

use std::collections::BTreeSet;

use sdk::{
    Ctx, Error, MerkleStore, Module, ModuleId, Msg, Origin, ResolverSyncTarget, StagedStore,
    StateRoot, StateSyncHandle,
};
use serde::{Serialize, de::DeserializeOwned};
use tagging::{TagEvent, TaggingMsg};

/// single-component key: prefix + 0 + id. safe because every prefix is a fixed
/// literal and no prefix is another prefix followed by a 0 byte.
fn keyed(prefix: &[u8], id: &str) -> Vec<u8> {
    let mut key = Vec::with_capacity(prefix.len() + 1 + id.len());
    key.extend_from_slice(prefix);
    key.push(0);
    key.extend_from_slice(id.as_bytes());
    key
}

/// length-prefixed component for multi-part keys — never a 0-byte separator,
/// so components containing separators cannot collide.
fn component(key: &mut Vec<u8>, value: &[u8]) {
    key.extend_from_slice(&(value.len() as u64).to_be_bytes());
    key.extend_from_slice(value);
}

fn channel_key(channel_id: &str) -> Vec<u8> {
    keyed(b"channel", channel_id)
}

fn msg_key(channel_id: &str, seq: u64) -> Vec<u8> {
    let mut key = Vec::with_capacity(3 + 8 + channel_id.len() + 8);
    key.extend_from_slice(b"msg");
    component(&mut key, channel_id.as_bytes());
    key.extend_from_slice(&seq.to_be_bytes());
    key
}

fn rev_key(channel_id: &str, seq: u64, rev: u32) -> Vec<u8> {
    let mut key = Vec::with_capacity(3 + 8 + channel_id.len() + 12);
    key.extend_from_slice(b"rev");
    component(&mut key, channel_id.as_bytes());
    key.extend_from_slice(&seq.to_be_bytes());
    key.extend_from_slice(&rev.to_be_bytes());
    key
}

fn react_key(channel_id: &str, seq: u64, emoji: &str) -> Vec<u8> {
    let mut key = Vec::with_capacity(5 + 16 + channel_id.len() + 8 + emoji.len());
    key.extend_from_slice(b"react");
    component(&mut key, channel_id.as_bytes());
    key.extend_from_slice(&seq.to_be_bytes());
    component(&mut key, emoji.as_bytes());
    key
}

/// per-message emoji index: hashed keys cannot enumerate `react/...`, so the
/// distinct-emoji list (bounded by [`MAX_REACTION_EMOJIS`]) is its own record.
fn reactidx_key(channel_id: &str, seq: u64) -> Vec<u8> {
    let mut key = Vec::with_capacity(8 + 8 + channel_id.len() + 8);
    key.extend_from_slice(b"reactidx");
    component(&mut key, channel_id.as_bytes());
    key.extend_from_slice(&seq.to_be_bytes());
    key
}

fn msgid_key(message_id: &str) -> Vec<u8> {
    keyed(b"msgid", message_id)
}

fn member_key(channel_id: &str, user: &[u8]) -> Vec<u8> {
    let mut key = Vec::with_capacity(6 + 16 + channel_id.len() + user.len());
    key.extend_from_slice(b"member");
    component(&mut key, channel_id.as_bytes());
    component(&mut key, user);
    key
}

/// derive the message author from the dispatch origin — the only authorship
/// path. the pre-consensus default `Origin::External(vec![])` must not pass as
/// an authenticated user.
fn author_from_origin(origin: &Origin) -> Result<AuthorRef, Error> {
    match origin {
        Origin::External(bytes) if bytes.is_empty() => Err(Error::Module(
            "external origin must carry a non-empty submitter id".into(),
        )),
        Origin::External(bytes) => Ok(AuthorRef::User(bytes.clone())),
        Origin::Module(id) => Ok(AuthorRef::Module(id.clone())),
        Origin::System => Ok(AuthorRef::System),
    }
}

/// structured mentions from message blocks, first occurrence order, deduped.
fn collect_mentions(blocks: &[Block]) -> Vec<AuthorRef> {
    let mut mentions: Vec<AuthorRef> = Vec::new();
    for block in blocks {
        let spans = match block {
            Block::Paragraph(spans) | Block::Quote(spans) => spans,
            Block::Code { .. } | Block::Divider => continue,
        };
        for span in spans {
            for mark in &span.marks {
                if let Mark::Mention(author) = mark
                    && !mentions.contains(author)
                {
                    mentions.push(author.clone());
                }
            }
        }
    }
    mentions
}

/// chat's author shape in the tagging plane's vocabulary — the edge
/// translation the plane's module-agnosticism depends on.
fn tag_author(author: &AuthorRef) -> tagging::Author {
    match author {
        AuthorRef::User(key) => tagging::Author::User(key.clone()),
        AuthorRef::Agent { module, agent_id } => tagging::Author::Entity(tagging::EntityRef {
            module: module.clone(),
            entity: agent_id.clone(),
        }),
        AuthorRef::Module(module) => tagging::Author::Module(module.clone()),
        AuthorRef::System => tagging::Author::System,
    }
}

/// the entity tag a mention names, if it names a module-hosted entity at all
/// (user mentions address people, not module entities — no tag).
fn tag_ref(mention: &AuthorRef) -> Option<tagging::EntityRef> {
    match mention {
        AuthorRef::Agent { module, agent_id } => Some(tagging::EntityRef {
            module: module.clone(),
            entity: agent_id.clone(),
        }),
        AuthorRef::User(_) | AuthorRef::Module(_) | AuthorRef::System => None,
    }
}

fn clamp_limit(limit: u64) -> u64 {
    limit.min(MAX_QUERY_LIMIT)
}

/// what a successful post staged — the inputs of the hook notifications.
struct Posted {
    seq: u64,
    thread_root: Option<u64>,
    hooks: Vec<String>,
}

/// storage-backed chat module.
pub struct Chat {
    id: ModuleId,
    /// the host-injected authenticated store plus this block's staging overlay
    /// (logical-key -> staged write; `None` = delete; read-your-writes, folded
    /// into `root()` at `commit_block`). store key is `sha256(logical_key)`,
    /// owned by [`StagedStore`].
    staged: StagedStore,
    /// the tagging plane every post is reported to (one `TagEvent` follow-up
    /// per post, same block). `None` = no plane on this host (tests, minimal
    /// registries). the plane owns the loop rule and the subscription check;
    /// chat only translates its shapes at this edge.
    tagging: Option<ModuleId>,
}

impl Chat {
    /// wrap the host-constructed store under module identity `id`. sync — the
    /// store arrives already opened (or already synced to a verified root).
    pub fn new(id: impl Into<ModuleId>, store: Box<dyn MerkleStore>) -> Self {
        Self {
            id: id.into(),
            staged: StagedStore::new(store),
            tagging: None,
        }
    }

    /// report every post to `tagging` as a [`tagging::TagEvent`].
    pub fn with_tagging(mut self, tagging: impl Into<ModuleId>) -> Self {
        self.tagging = Some(tagging.into());
        self
    }

    fn validate_non_empty(field: &str, value: &str) -> Result<(), Error> {
        if value.is_empty() {
            return Err(Error::Module(format!("{field} must not be empty")));
        }
        Ok(())
    }

    async fn get_raw(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        self.staged.get(key).await
    }

    async fn load<T>(&self, key: &[u8]) -> Result<Option<T>, Error>
    where
        T: DeserializeOwned,
    {
        match self.get_raw(key).await? {
            Some(bytes) => Ok(Some(
                serde_json::from_slice(&bytes).map_err(|e| Error::Module(e.to_string()))?,
            )),
            None => Ok(None),
        }
    }

    fn store<T>(&mut self, key: Vec<u8>, value: &T)
    where
        T: Serialize,
    {
        self.staged.stage(
            key,
            serde_json::to_vec(value).expect("chat value is serializable"),
        );
    }

    /// stage a value only if its serialized size fits `cap` — the write-time
    /// guard against poison values (the qmdb codec cap is decode-only).
    fn store_bounded<T>(
        &mut self,
        key: Vec<u8>,
        value: &T,
        cap: usize,
        what: &str,
    ) -> Result<(), Error>
    where
        T: Serialize,
    {
        let bytes = serde_json::to_vec(value).expect("chat value is serializable");
        if bytes.len() > cap {
            return Err(Error::Module(format!(
                "{what} record too large: {} > {cap} bytes",
                bytes.len()
            )));
        }
        self.staged.stage(key, bytes);
        Ok(())
    }

    fn delete(&mut self, key: Vec<u8>) {
        self.staged.delete(key);
    }

    async fn channel(&self, channel_id: &str) -> Result<Option<Channel>, Error> {
        self.load(&channel_key(channel_id)).await
    }

    async fn require_channel(&self, channel_id: &str) -> Result<Channel, Error> {
        self.channel(channel_id)
            .await?
            .ok_or_else(|| Error::Module(format!("unknown channel: {channel_id}")))
    }

    async fn head(&self, channel_id: &str, seq: u64) -> Result<Option<MessageHead>, Error> {
        self.load(&msg_key(channel_id, seq)).await
    }

    async fn require_head(&self, channel_id: &str, seq: u64) -> Result<MessageHead, Error> {
        self.head(channel_id, seq)
            .await?
            .ok_or_else(|| Error::Module(format!("unknown message: {channel_id}/{seq}")))
    }

    async fn is_member(&self, channel_id: &str, user: &[u8]) -> Result<bool, Error> {
        Ok(self.get_raw(&member_key(channel_id, user)).await?.is_some())
    }

    /// gate a post/reaction on the channel policy. module, agent, and system
    /// authors always pass — modules are genesis-fixed trusted code (the agent
    /// module must be able to answer in members-only channels, and an agent
    /// author is a module origin refined by `as_agent`); external users need
    /// membership under `MembersOnly`.
    async fn check_post_policy(&self, channel: &Channel, author: &AuthorRef) -> Result<(), Error> {
        // an archived channel rejects posts, reactions, and huddle join/sweep:
        // every posting-class op routes through here, so one guard turns them
        // all away. edits and deletes deliberately do not call this — redacting
        // your own message stays possible in a closed channel — and neither do
        // membership, rename, or unarchive.
        if channel.archived {
            return Err(Error::Module(format!("channel {} is archived", channel.id)));
        }
        match (&channel.post_policy, author) {
            (PostPolicy::MembersOnly, AuthorRef::User(user)) => {
                if !self.is_member(&channel.id, user).await? {
                    return Err(Error::Module(format!(
                        "channel {} is members-only and the author is not a member",
                        channel.id
                    )));
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// one external user's standing in one channel, answered from chat's own
    /// gates — [`ChatQuery::Access`]. a module that acts on a user's behalf
    /// (automations firing that user's rule) asks HERE instead of re-deriving
    /// the admission rule, because a second copy of it is a second rule.
    /// an unknown channel answers `false` to both: the caller fails closed.
    async fn channel_access(&self, channel_id: &str, user: &[u8]) -> Result<ChannelAccess, Error> {
        let Some(channel) = self.channel(channel_id).await? else {
            return Ok(ChannelAccess {
                may_read: false,
                may_post: false,
            });
        };
        // reading is not policied per-message: an OPEN channel is readable by
        // any authenticated user, a members-only one only by its members.
        let is_open = matches!(channel.post_policy, PostPolicy::Open);
        let may_read = is_open || self.is_member(channel_id, user).await?;
        // the post answer is the post GATE, run verbatim — archival and policy
        // included — so the two can never drift apart.
        let may_post = self
            .check_post_policy(&channel, &AuthorRef::User(user.to_vec()))
            .await
            .is_ok();
        Ok(ChannelAccess { may_read, may_post })
    }

    /// enforce the reserved channel-id namespace: ids containing ':' belong
    /// to modules, and a module may only mint ids under its own `"{module}:"`
    /// prefix (e.g. forge's per-issue discussion channels `forge:<repo>:<n>`),
    /// so no origin can squat another's namespace. system origin is
    /// unrestricted. unconditional consensus rule — not version-gated.
    fn validate_channel_namespace(author: &AuthorRef, channel_id: &str) -> Result<(), Error> {
        // '/' is the read model's key-path separator: a channel id carrying
        // one would bleed across the index tier's prefix scans ("a" vs
        // "a/b"). unconditional consensus rule, mirroring the ':' gate.
        if channel_id.contains('/') {
            return Err(Error::Module(
                "chat: channel ids may not contain '/'".into(),
            ));
        }
        match author {
            AuthorRef::User(_) => {
                if channel_id.contains(':') {
                    return Err(Error::Module(
                        "chat: channel ids containing ':' are reserved for modules".into(),
                    ));
                }
                Ok(())
            }
            // an agent author is a module origin refined by `as_agent`
            // (PostMessage only), so it cannot reach CreateChannel — but the
            // hosting module's prefix rule is the right one if it ever does.
            AuthorRef::Module(module) | AuthorRef::Agent { module, .. } => {
                if !channel_id.starts_with(&format!("{module}:")) {
                    return Err(Error::Module(format!(
                        "chat: module '{module}' may only create channel ids prefixed '{module}:'"
                    )));
                }
                Ok(())
            }
            AuthorRef::System => Ok(()),
        }
    }

    /// authorize a channel-admin op — rename, archive, membership, and hook
    /// (un)registration, i.e. every write that changes who may write. an owned
    /// channel admits only its owner among `User` origins; an UNOWNED
    /// (module/system-minted) channel admits NO user, because the principal
    /// that minted it is a module and a user is not it — `forge:<repo>:<n>` is
    /// the live case, and admitting any user there would hand a stranger the
    /// roster and the hook list of another module's channel. module/agent/system
    /// origins are genesis-fixed trusted code and always pass.
    ///
    /// exhaustive on purpose, both levels: a security decision must never route
    /// a new `AuthorRef` variant — or the `None` owner — through a wildcard.
    fn check_channel_admin(channel: &Channel, author: &AuthorRef) -> Result<(), Error> {
        match author {
            AuthorRef::User(user) => match &channel.owner {
                None => Err(Error::Module(format!(
                    "channel {} is unowned; no user may administer it",
                    channel.id
                ))),
                Some(owner) => {
                    let is_owner = owner == user;
                    if !is_owner {
                        return Err(Error::Module(format!(
                            "only the owner may administer channel {}",
                            channel.id
                        )));
                    }
                    Ok(())
                }
            },
            AuthorRef::Module(_) | AuthorRef::Agent { .. } | AuthorRef::System => Ok(()),
        }
    }

    async fn stage_channel(
        &mut self,
        author: &AuthorRef,
        channel_id: String,
        name: String,
        post_policy: PostPolicy,
        created_at: u64,
    ) -> Result<(), Error> {
        Self::validate_non_empty("channel_id", &channel_id)?;
        Self::validate_non_empty("name", &name)?;
        Self::validate_channel_namespace(author, &channel_id)?;
        if self.channel(&channel_id).await?.is_some() {
            return Err(Error::Module(format!(
                "channel already exists: {channel_id}"
            )));
        }

        // a user-created channel is owned by its creator (only the owner may
        // later rename/archive it); module/system-minted channels are unowned.
        let owner = match author {
            AuthorRef::User(user) => Some(user.clone()),
            AuthorRef::Module(_) | AuthorRef::Agent { .. } | AuthorRef::System => None,
        };
        let channel = Channel {
            id: channel_id.clone(),
            name,
            created_at,
            head_seq: 0,
            post_policy,
            hooks: Vec::new(),
            pinned: Vec::new(),
            huddle: Vec::new(),
            owner,
            archived: false,
        };
        self.store_bounded(
            channel_key(&channel_id),
            &channel,
            MAX_CHANNEL_RECORD_BYTES,
            "channel",
        )
    }

    /// rename a channel. reuses `CreateChannel`'s name validation (non-empty +
    /// the `:` namespace gate — the id is unchanged, but the gate still keeps a
    /// user off a module-namespaced channel) and the record byte cap.
    async fn stage_rename(
        &mut self,
        author: &AuthorRef,
        channel_id: &str,
        name: String,
    ) -> Result<(), Error> {
        Self::validate_non_empty("channel_id", channel_id)?;
        Self::validate_non_empty("name", &name)?;
        Self::validate_channel_namespace(author, channel_id)?;
        let mut channel = self.require_channel(channel_id).await?;
        Self::check_channel_admin(&channel, author)?;
        if channel.name == name {
            // idempotent: a same-name rename stages nothing, so the op log —
            // and the root — is byte-identical to no write at all.
            return Ok(());
        }
        channel.name = name;
        self.store_bounded(
            channel_key(channel_id),
            &channel,
            MAX_CHANNEL_RECORD_BYTES,
            "channel",
        )
    }

    /// archive or unarchive a channel. authorization mirrors `stage_rename`
    /// (the same `:` namespace gate keeps a user off a module-namespaced
    /// channel — otherwise any user could archive a `forge:<repo>:<n>`
    /// discussion, and an archived channel rejects the owning module's posts
    /// too); the flag itself is what `check_post_policy` reads to gate writes.
    async fn stage_set_archived(
        &mut self,
        author: &AuthorRef,
        channel_id: &str,
        archived: bool,
    ) -> Result<(), Error> {
        Self::validate_non_empty("channel_id", channel_id)?;
        Self::validate_channel_namespace(author, channel_id)?;
        let mut channel = self.require_channel(channel_id).await?;
        Self::check_channel_admin(&channel, author)?;
        if channel.archived == archived {
            return Ok(());
        }
        channel.archived = archived;
        self.store_bounded(
            channel_key(channel_id),
            &channel,
            MAX_CHANNEL_RECORD_BYTES,
            "channel",
        )
    }

    async fn stage_message(
        &mut self,
        author: AuthorRef,
        channel_id: &str,
        message_id: String,
        blocks: Vec<Block>,
        thread: Option<u64>,
        now: u64,
    ) -> Result<Posted, Error> {
        Self::validate_non_empty("channel_id", channel_id)?;
        Self::validate_non_empty("message_id", &message_id)?;
        if blocks.is_empty() {
            return Err(Error::Module("blocks must not be empty".into()));
        }
        let mut channel = self.require_channel(channel_id).await?;
        self.check_post_policy(&channel, &author).await?;
        if self.get_raw(&msgid_key(&message_id)).await?.is_some() {
            return Err(Error::Module(format!(
                "message already exists: {message_id}"
            )));
        }

        // the per-channel sequence comes from the head_seq counter — never
        // from the last element of a list (P3: gap-free, assigned in-state).
        let seq = channel.head_seq + 1;
        channel.head_seq = seq;

        if let Some(root_seq) = thread {
            // a reply is a normal message record with its own channel seq;
            // the root tracks the summary (`reply_count` is the reply-cap
            // authority — replies are enumerable from it as the root's
            // sequence-ordered descendants, so no stored reply list exists).
            // a tombstoned root still anchors its thread, so replying to it
            // stays legal.
            let mut root = self.require_head(channel_id, root_seq).await?;
            if root.thread.is_some() {
                return Err(Error::Module(format!(
                    "thread replies cannot start subthreads: {channel_id}/{root_seq}"
                )));
            }
            if root.reply_count >= MAX_THREAD_REPLIES as u64 {
                return Err(Error::Module(format!(
                    "thread reply cap reached: {channel_id}/{root_seq}"
                )));
            }
            root.reply_count += 1;
            root.last_reply_seq = Some(seq);
            self.store_bounded(
                msg_key(channel_id, root_seq),
                &root,
                MAX_MESSAGE_HEAD_BYTES,
                "message",
            )?;
        }

        let head = MessageHead {
            message_id: message_id.clone(),
            author,
            blocks,
            created_at: now,
            rev: 0,
            edited_at: None,
            base_rev: None,
            deleted: false,
            thread,
            reply_count: 0,
            last_reply_seq: None,
        };
        self.store_bounded(
            msg_key(channel_id, seq),
            &head,
            MAX_MESSAGE_HEAD_BYTES,
            "message",
        )?;
        self.store(msgid_key(&message_id), &(channel_id.to_string(), seq));
        let hooks = channel.hooks.clone();
        self.store_bounded(
            channel_key(channel_id),
            &channel,
            MAX_CHANNEL_RECORD_BYTES,
            "channel",
        )?;
        Ok(Posted {
            seq,
            thread_root: thread,
            hooks,
        })
    }

    async fn stage_edit(
        &mut self,
        author: AuthorRef,
        channel_id: &str,
        seq: u64,
        blocks: Vec<Block>,
        base_rev: Option<u32>,
        now: u64,
    ) -> Result<u32, Error> {
        Self::validate_non_empty("channel_id", channel_id)?;
        if blocks.is_empty() {
            return Err(Error::Module("blocks must not be empty".into()));
        }
        let head = self.require_head(channel_id, seq).await?;
        if head.deleted {
            return Err(Error::Module(format!(
                "cannot edit a deleted message: {channel_id}/{seq}"
            )));
        }
        if head.author != author {
            return Err(Error::Module("only the author may edit a message".into()));
        }
        if head.rev >= MAX_REVISIONS - 1 {
            return Err(Error::Module(format!(
                "revision cap reached: {channel_id}/{seq}"
            )));
        }

        // head is last-write-wins under the total order; the prior head moves
        // into the immutable revision history. a stale base_rev is recorded on
        // the new head (base_rev != prior rev), never rejected — the author
        // gate makes conflicts same-author multi-device races.
        self.store(rev_key(channel_id, seq, head.rev), &head);
        let rev = head.rev + 1;
        let new_head = MessageHead {
            blocks,
            rev,
            edited_at: Some(now),
            base_rev,
            ..head
        };
        self.store_bounded(
            msg_key(channel_id, seq),
            &new_head,
            MAX_MESSAGE_HEAD_BYTES,
            "message",
        )?;
        Ok(rev)
    }

    async fn stage_delete(
        &mut self,
        author: AuthorRef,
        channel_id: &str,
        seq: u64,
    ) -> Result<(), Error> {
        Self::validate_non_empty("channel_id", channel_id)?;
        let head = self.require_head(channel_id, seq).await?;
        if head.deleted {
            return Err(Error::Module(format!(
                "message already deleted: {channel_id}/{seq}"
            )));
        }
        if head.author != author {
            return Err(Error::Module("only the author may delete a message".into()));
        }

        // clear reactions; the emoji index says which records exist.
        let emojis: BTreeSet<String> = self
            .load(&reactidx_key(channel_id, seq))
            .await?
            .unwrap_or_default();
        for emoji in &emojis {
            self.delete(react_key(channel_id, seq, emoji));
        }
        self.delete(reactidx_key(channel_id, seq));

        // tombstone: blocks cleared, skeleton (seq, thread linkage, reply
        // summary, authorship, revision history) preserved so thread integrity
        // and the sequence promise survive.
        let tombstone = MessageHead {
            blocks: Vec::new(),
            deleted: true,
            ..head
        };
        self.store_bounded(
            msg_key(channel_id, seq),
            &tombstone,
            MAX_MESSAGE_HEAD_BYTES,
            "message",
        )
    }

    /// shared reaction-op prelude: emoji + policy + target-message checks.
    async fn reaction_target(
        &self,
        author: &AuthorRef,
        channel_id: &str,
        seq: u64,
        emoji: &str,
    ) -> Result<(), Error> {
        Self::validate_non_empty("channel_id", channel_id)?;
        Self::validate_non_empty("emoji", emoji)?;
        if emoji.len() > MAX_EMOJI_BYTES {
            return Err(Error::Module(format!(
                "emoji too long: {} > {MAX_EMOJI_BYTES} bytes",
                emoji.len()
            )));
        }
        let channel = self.require_channel(channel_id).await?;
        self.check_post_policy(&channel, author).await?;
        let head = self.require_head(channel_id, seq).await?;
        if head.deleted {
            return Err(Error::Module(format!(
                "cannot react to a deleted message: {channel_id}/{seq}"
            )));
        }
        Ok(())
    }

    async fn stage_add_reaction(
        &mut self,
        author: AuthorRef,
        channel_id: &str,
        seq: u64,
        emoji: &str,
    ) -> Result<(), Error> {
        self.reaction_target(&author, channel_id, seq, emoji)
            .await?;
        let mut reactors: BTreeSet<AuthorRef> = self
            .load(&react_key(channel_id, seq, emoji))
            .await?
            .unwrap_or_default();
        if reactors.contains(&author) {
            // idempotent: a duplicate add stages NOTHING, so the qmdb op log —
            // and therefore the root — is byte-identical to a single add.
            return Ok(());
        }
        let mut emojis: BTreeSet<String> = self
            .load(&reactidx_key(channel_id, seq))
            .await?
            .unwrap_or_default();
        if reactors.is_empty() && !emojis.contains(emoji) && emojis.len() >= MAX_REACTION_EMOJIS {
            return Err(Error::Module(format!(
                "distinct emoji cap reached: {channel_id}/{seq}"
            )));
        }
        reactors.insert(author);
        if emojis.insert(emoji.to_string()) {
            self.store(reactidx_key(channel_id, seq), &emojis);
        }
        self.store_bounded(
            react_key(channel_id, seq, emoji),
            &reactors,
            MAX_MESSAGE_HEAD_BYTES,
            "reaction",
        )
    }

    async fn stage_remove_reaction(
        &mut self,
        author: AuthorRef,
        channel_id: &str,
        seq: u64,
        emoji: &str,
    ) -> Result<(), Error> {
        self.reaction_target(&author, channel_id, seq, emoji)
            .await?;
        let mut reactors: BTreeSet<AuthorRef> = self
            .load(&react_key(channel_id, seq, emoji))
            .await?
            .unwrap_or_default();
        if !reactors.remove(&author) {
            // exact remove: absent (emoji, author) is a deterministic no-op.
            return Ok(());
        }
        if reactors.is_empty() {
            self.delete(react_key(channel_id, seq, emoji));
            let mut emojis: BTreeSet<String> = self
                .load(&reactidx_key(channel_id, seq))
                .await?
                .unwrap_or_default();
            emojis.remove(emoji);
            if emojis.is_empty() {
                self.delete(reactidx_key(channel_id, seq));
            } else {
                self.store(reactidx_key(channel_id, seq), &emojis);
            }
            return Ok(());
        }
        self.store_bounded(
            react_key(channel_id, seq, emoji),
            &reactors,
            MAX_MESSAGE_HEAD_BYTES,
            "reaction",
        )
    }

    /// register a hook module on a channel. channel-admin authority: a hook is
    /// a standing subscription to everything posted there, so attaching one is
    /// the owner's call, not any member's.
    async fn stage_register_hook(
        &mut self,
        author: &AuthorRef,
        channel_id: &str,
        module_id: String,
    ) -> Result<(), Error> {
        Self::validate_non_empty("channel_id", channel_id)?;
        Self::validate_non_empty("module_id", &module_id)?;
        let mut channel = self.require_channel(channel_id).await?;
        Self::check_channel_admin(&channel, author)?;
        if channel.hooks.contains(&module_id) {
            // idempotent: registering twice stages nothing.
            return Ok(());
        }
        if channel.hooks.len() >= MAX_HOOKS_PER_CHANNEL {
            return Err(Error::Module(format!("hook cap reached: {channel_id}")));
        }
        channel.hooks.push(module_id);
        self.store_bounded(
            channel_key(channel_id),
            &channel,
            MAX_CHANNEL_RECORD_BYTES,
            "channel",
        )
    }

    /// unregister a hook module. same authority as registration — an ungated
    /// unregister is a one-message off switch for every automation on the
    /// channel, which is the sharper half of the pair.
    async fn stage_unregister_hook(
        &mut self,
        author: &AuthorRef,
        channel_id: &str,
        module_id: &str,
    ) -> Result<(), Error> {
        Self::validate_non_empty("channel_id", channel_id)?;
        let mut channel = self.require_channel(channel_id).await?;
        Self::check_channel_admin(&channel, author)?;
        let before = channel.hooks.len();
        channel.hooks.retain(|hook| hook != module_id);
        if channel.hooks.len() == before {
            return Ok(());
        }
        self.store_bounded(
            channel_key(channel_id),
            &channel,
            MAX_CHANNEL_RECORD_BYTES,
            "channel",
        )
    }

    /// add/remove a user from the channel roster. channel-admin authority: the
    /// roster IS `PostPolicy::MembersOnly`'s admission list, so a self-service
    /// roster is no admission rule at all — anyone could add themselves and
    /// post right through the policy.
    async fn stage_membership(
        &mut self,
        author: &AuthorRef,
        channel_id: &str,
        user: Vec<u8>,
        member: bool,
    ) -> Result<(), Error> {
        Self::validate_non_empty("channel_id", channel_id)?;
        if user.is_empty() {
            return Err(Error::Module("user must not be empty".into()));
        }
        let channel = self.require_channel(channel_id).await?;
        Self::check_channel_admin(&channel, author)?;
        // idempotent: an unchanged membership stages nothing, so the qmdb op
        // log — and the root — is byte-identical to no write at all. the point
        // record is the policy read; the roster VIEW lives on the index tier.
        let already_member = self
            .get_raw(&member_key(channel_id, &user))
            .await?
            .is_some();
        if already_member == member {
            return Ok(());
        }
        if member {
            self.store(member_key(channel_id, &user), &true);
        } else {
            self.delete(member_key(channel_id, &user));
        }
        Ok(())
    }

    /// join (or start) the channel's huddle. only external users may — the
    /// roster is a room of people, so module/system origins are rejected —
    /// and members-only channels gate exactly like posting. re-joining with
    /// the same node key stages nothing (idempotent, byte-identical op log).
    async fn stage_join_huddle(
        &mut self,
        author: AuthorRef,
        channel_id: &str,
        node: Vec<u8>,
        now: u64,
    ) -> Result<(), Error> {
        Self::validate_non_empty("channel_id", channel_id)?;
        let AuthorRef::User(user) = &author else {
            return Err(Error::Module(
                "only external users may join a huddle".into(),
            ));
        };
        if node.len() != HUDDLE_NODE_KEY_BYTES {
            return Err(Error::Module(format!(
                "huddle node key must be {HUDDLE_NODE_KEY_BYTES} bytes, got {}",
                node.len()
            )));
        }
        let mut channel = self.require_channel(channel_id).await?;
        self.check_post_policy(&channel, &author).await?;
        if let Some(existing) = channel.huddle.iter_mut().find(|m| &m.user == user) {
            if existing.node == node {
                return Ok(());
            }
            existing.node = node;
        } else {
            if channel.huddle.len() >= MAX_HUDDLE_MEMBERS {
                return Err(Error::Module(format!("huddle is full: {channel_id}")));
            }
            channel.huddle.push(HuddleMember {
                user: user.clone(),
                node,
                joined_at: now,
            });
        }
        self.store_bounded(
            channel_key(channel_id),
            &channel,
            MAX_CHANNEL_RECORD_BYTES,
            "channel",
        )
    }

    /// leave the channel's huddle. absent participation is a deterministic
    /// no-op; the last leaver empties the roster (= the huddle ends).
    async fn stage_leave_huddle(
        &mut self,
        author: AuthorRef,
        channel_id: &str,
    ) -> Result<(), Error> {
        Self::validate_non_empty("channel_id", channel_id)?;
        let AuthorRef::User(user) = &author else {
            return Err(Error::Module(
                "only external users may leave a huddle".into(),
            ));
        };
        let mut channel = self.require_channel(channel_id).await?;
        let before = channel.huddle.len();
        channel.huddle.retain(|m| &m.user != user);
        if channel.huddle.len() == before {
            return Ok(());
        }
        self.store_bounded(
            channel_key(channel_id),
            &channel,
            MAX_CHANNEL_RECORD_BYTES,
            "channel",
        )
    }

    /// whether `actor` may evict `target` from `channel`'s huddle: the
    /// channel's admin always may (`check_channel_admin` — the owner, or any
    /// module/agent/system origin). a self-sweep is legitimate too, but it is
    /// routed to `stage_leave_huddle` before this predicate ever runs, not
    /// decided here. there is no third arm: `HuddleMember` carries only
    /// `joined_at`, set once at join and never refreshed on liveness, so the
    /// module holds no call-presence signal a staleness rule could read —
    /// only the admin authority remains.
    fn may_sweep(channel: &Channel, actor: &AuthorRef) -> bool {
        Self::check_channel_admin(channel, actor).is_ok()
    }

    /// evict `user` from the channel's huddle (staleness cleanup — see
    /// `ChatMsg::SweepHuddle`). a poster naming themself is a leave in
    /// disguise; naming anyone else is an admin-only eviction (`may_sweep`).
    /// absent target = no-op either way.
    async fn stage_sweep_huddle(
        &mut self,
        author: AuthorRef,
        channel_id: &str,
        user: &[u8],
    ) -> Result<(), Error> {
        Self::validate_non_empty("channel_id", channel_id)?;
        let AuthorRef::User(actor) = &author else {
            return Err(Error::Module(
                "only external users may sweep a huddle".into(),
            ));
        };
        if actor.as_slice() == user {
            return self.stage_leave_huddle(author, channel_id).await;
        }
        let mut channel = self.require_channel(channel_id).await?;
        if !Self::may_sweep(&channel, &author) {
            return Err(Error::Module(format!(
                "only the channel admin may sweep another user's huddle entry in {channel_id}"
            )));
        }
        let before = channel.huddle.len();
        channel.huddle.retain(|m| m.user != user);
        if channel.huddle.len() == before {
            return Ok(());
        }
        self.store_bounded(
            channel_key(channel_id),
            &channel,
            MAX_CHANNEL_RECORD_BYTES,
            "channel",
        )
    }

    // ---- dispatch reads --------------------------------------------------
    // the three reads other modules' execute() paths consume. everything a
    // human lists, scrolls, or searches is the index guest's job (index.rs).

    /// point-lookup one page of message views for computed sequences. the
    /// sequence space is gap-free (P3), so a missing head is a store bug.
    async fn views(
        &self,
        channel_id: &str,
        seqs: impl Iterator<Item = u64>,
    ) -> Result<Vec<MessageView>, Error> {
        let mut views = Vec::new();
        for seq in seqs {
            let head = self.require_head(channel_id, seq).await.map_err(|_| {
                Error::Module(format!("missing message record: {channel_id}/{seq}"))
            })?;
            views.push(MessageView {
                channel_id: channel_id.to_string(),
                seq,
                head,
            });
        }
        Ok(views)
    }

    async fn messages_range(
        &self,
        channel_id: &str,
        from_seq: u64,
        limit: u64,
    ) -> Result<Vec<MessageView>, Error> {
        let channel = self.require_channel(channel_id).await?;
        let limit = clamp_limit(limit);
        let from = from_seq.max(1);
        if limit == 0 || from > channel.head_seq {
            return Ok(Vec::new());
        }
        let to = channel.head_seq.min(from.saturating_add(limit - 1));
        self.views(channel_id, from..=to).await
    }

    async fn message_by_id(&self, message_id: &str) -> Result<Option<MessageView>, Error> {
        let Some((channel_id, seq)) = self.load::<(String, u64)>(&msgid_key(message_id)).await?
        else {
            return Ok(None);
        };
        let head = self.require_head(&channel_id, seq).await?;
        Ok(Some(MessageView {
            channel_id,
            seq,
            head,
        }))
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Chat {
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
        let now = ctx.env().consensus_time;
        // every write op requires an authenticated author, even ops that do
        // not store one — the empty demo-default external origin never passes.
        let author = author_from_origin(&ctx.env().origin)?;
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            ChatMsg::CreateChannel {
                channel_id,
                name,
                post_policy,
            } => {
                self.stage_channel(&author, channel_id, name, post_policy, now)
                    .await
            }
            ChatMsg::RenameChannel { channel_id, name } => {
                self.stage_rename(&author, &channel_id, name).await
            }
            ChatMsg::SetChannelArchived {
                channel_id,
                archived,
            } => {
                self.stage_set_archived(&author, &channel_id, archived)
                    .await
            }
            ChatMsg::PostMessage {
                channel_id,
                message_id,
                blocks,
                thread,
                as_agent,
            } => {
                // `as_agent` refines a MODULE origin into an individual agent
                // author. modules are genesis-trusted code, so the module id
                // half of the author is still origin-derived and spoof-proof;
                // an external or system submitter claiming an agent identity
                // is rejected outright.
                let author = match as_agent {
                    None => author,
                    Some(agent_id) => {
                        Self::validate_non_empty("as_agent", &agent_id)?;
                        match author {
                            AuthorRef::Module(module) => AuthorRef::Agent { module, agent_id },
                            _ => {
                                return Err(Error::Module(
                                    "as_agent requires a module origin".into(),
                                ));
                            }
                        }
                    }
                };
                let mentions = collect_mentions(&blocks);
                let posted = self
                    .stage_message(author.clone(), &channel_id, message_id, blocks, thread, now)
                    .await?;
                ctx.set_assigned(encode_assigned(&ChatAssigned::Posted { seq: posted.seq }));
                // one follow-up per registered hook, drained in this block —
                // the message and every notification commit (or abort) as one
                // atomic unit (P2). chat stays agent-agnostic: any subscriber
                // module decodes the ChatEvent payload.
                for hook in posted.hooks {
                    ctx.emit_msg(Msg {
                        target: hook,
                        payload: encode_event(&ChatEvent::MessagePosted {
                            channel_id: channel_id.clone(),
                            seq: posted.seq,
                            thread_root: posted.thread_root,
                            author: author.clone(),
                            mentions: mentions.clone(),
                        }),
                    });
                }
                // report the post to the tagging plane in this same block,
                // translating chat shapes at this edge. unconditional on
                // author kind: the loop rule (only user posts engage) is the
                // PLANE's rule, stated once there — chat does not pre-judge.
                if let Some(tagging) = &self.tagging {
                    ctx.emit_msg(Msg {
                        target: tagging.clone(),
                        payload: tagging::encode_msg(&TaggingMsg::Tag(TagEvent {
                            container: channel_id,
                            content_seq: posted.seq,
                            author: tag_author(&author),
                            tags: mentions.iter().filter_map(tag_ref).collect(),
                        })),
                    });
                }
                Ok(())
            }
            ChatMsg::EditMessage {
                channel_id,
                seq,
                blocks,
                base_rev,
            } => {
                let rev = self
                    .stage_edit(author, &channel_id, seq, blocks, base_rev, now)
                    .await?;
                ctx.set_assigned(encode_assigned(&ChatAssigned::Edited { rev }));
                Ok(())
            }
            ChatMsg::DeleteMessage { channel_id, seq } => {
                self.stage_delete(author, &channel_id, seq).await
            }
            ChatMsg::AddReaction {
                channel_id,
                seq,
                emoji,
            } => {
                self.stage_add_reaction(author, &channel_id, seq, &emoji)
                    .await
            }
            ChatMsg::RemoveReaction {
                channel_id,
                seq,
                emoji,
            } => {
                self.stage_remove_reaction(author, &channel_id, seq, &emoji)
                    .await
            }
            ChatMsg::RegisterHook {
                channel_id,
                module_id,
            } => {
                // the target must be a registered module other than chat
                // itself, or every later post would poison the block. WHO may
                // attach it is `check_channel_admin`, inside the stage fn.
                if module_id == self.id {
                    return Err(Error::Module("chat cannot hook itself".into()));
                }
                if ctx.module_root(&module_id).is_none() {
                    return Err(Error::Module(format!("unknown hook module: {module_id}")));
                }
                self.stage_register_hook(&author, &channel_id, module_id)
                    .await
            }
            ChatMsg::UnregisterHook {
                channel_id,
                module_id,
            } => {
                self.stage_unregister_hook(&author, &channel_id, &module_id)
                    .await
            }
            ChatMsg::SetMembership {
                channel_id,
                user,
                member,
            } => {
                self.stage_membership(&author, &channel_id, user, member)
                    .await
            }
            ChatMsg::JoinHuddle { channel_id, node } => {
                self.stage_join_huddle(author, &channel_id, node, now).await
            }
            ChatMsg::LeaveHuddle { channel_id } => {
                self.stage_leave_huddle(author, &channel_id).await
            }
            ChatMsg::SweepHuddle { channel_id, user } => {
                self.stage_sweep_huddle(author, &channel_id, &user).await
            }
        }
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            ChatQuery::Channel { channel_id } => Ok(encode_reply(&ChatReply::Channel(
                self.channel(&channel_id).await?,
            ))),
            ChatQuery::MessagesRange {
                channel_id,
                from_seq,
                limit,
            } => Ok(encode_reply(&ChatReply::Messages(
                self.messages_range(&channel_id, from_seq, limit).await?,
            ))),
            ChatQuery::Message { message_id } => Ok(encode_reply(&ChatReply::Message(
                self.message_by_id(&message_id).await?,
            ))),
            ChatQuery::Access { channel_id, user } => Ok(encode_reply(&ChatReply::Access(
                self.channel_access(&channel_id, &user).await?,
            ))),
        }
    }

    /// publish the block's staged writes in ONE store batch. no-op (and no
    /// root movement) if nothing was staged. BTreeMap iteration keeps the
    /// write order deterministic across validators, and a staged `None` ships
    /// as a delete of the hashed key.
    async fn commit_block(&mut self) -> Result<(), Error> {
        self.staged.commit().await
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.staged.abort();
        Ok(())
    }
}
