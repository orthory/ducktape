//! qmdb-backed chat module: block-based channels, threads, edits, tombstones,
//! reactions, membership, and hook notifications.
//!
//! the module stores one logical record per entity in one commonware qmdb:
//! a channel index (enumeration only — hashed qmdb keys cannot be listed; its
//! 1 MiB codec bound is accepted debt), per-channel records, one record per
//! message head, immutable per-edit revision records, per-emoji reaction sets,
//! message-id pointers for global dedup, membership records, and small
//! per-thread / per-message index records that stand in for range scans.
//! qmdb keys are hashed, so pagination is computed-key point lookups driven by
//! the channel's `head_seq` counter — never derived from a stored list.
//!
//! authorship is derived from `ctx.env().origin` on every write; payloads
//! carry no author field. an empty external origin (the pre-consensus default)
//! is rejected. like `document` and `kv`, writes are staged in memory during a
//! block and flushed to qmdb in one batch at `commit_block`; the module root is
//! the real qmdb root and the joiner path is commonware storage sync.

// the wire surface: this module's shared types, flattened at the crate root.
mod interface;
pub use interface::*;
// the derived-tier materialized view; registered only by serving binaries.
pub mod index;
// the real-time voice media engine (Opus over the data plane's datagram
// class). Off-consensus: it touches no qmdb and no app-hash — the chat
// module's consensus state (channels, membership) is what will drive its
// admission and channel→flow derivation. Kept as a self-contained submodule.
pub mod voice;
// the video call media wire (frame fragmentation/reassembly + call control)
// over the data plane's Service::Video / Service::Voice flows. Off-consensus
// like `voice`, for the same reason: consensus never carries media.
pub mod video;

use std::collections::{BTreeMap, BTreeSet};
use std::num::{NonZeroU16, NonZeroU64, NonZeroUsize};
use std::sync::Arc;

use commonware_codec::RangeCfg;
use commonware_cryptography::{Hasher, Sha256};
use commonware_parallel::Sequential;
use commonware_runtime::{BufferPooler, buffer::paged::CacheRef};
use commonware_storage::{
    Context, journal, mmr,
    qmdb::{
        any::{VariableConfig, unordered::variable::Db},
        sync::{self, DbResolver, Target, engine::Config as SyncConfig},
    },
    translator::TwoCap,
};
use commonware_utils::range::NonEmptyRange;
use sdk::{
    Ctx, Error, Module, ModuleId, Msg, Origin, ResolverSyncTarget, StateRoot, StateSyncHandle,
};
use serde::{Serialize, de::DeserializeOwned};
use tagging::{TagEvent, TaggingMsg};

/// the qmdb key: a fixed-width digest of a logical chat record key.
type ChatKey = <Sha256 as Hasher>::Digest;

/// one variable-value qmdb stores all chat records.
pub type ChatDb<E> = Db<mmr::Family, E, ChatKey, Vec<u8>, Sha256, TwoCap, Sequential>;

/// shared by fresh open and state-sync reconstruction so storage layout cannot
/// drift between source and joiner.
type ChatConfig = VariableConfig<TwoCap, ((), (RangeCfg<usize>, ())), Sequential>;

/// a storage-sync target: qmdb root plus the active operation range.
pub type ChatTarget = Target<mmr::Family, ChatKey>;

const CHANNEL_INDEX_KEY: &[u8] = b"channel-index";

fn hash_key(key: &[u8]) -> ChatKey {
    let mut h = Sha256::new();
    h.update(key);
    h.finalize()
}

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

fn memberidx_key(channel_id: &str) -> Vec<u8> {
    keyed(b"memberidx", channel_id)
}

/// per-thread reply index: reply sequences in post order, bounded by
/// [`MAX_THREAD_REPLIES`], so thread pages need no range scan.
fn threadidx_key(channel_id: &str, root_seq: u64) -> Vec<u8> {
    let mut key = Vec::with_capacity(9 + 8 + channel_id.len() + 8);
    key.extend_from_slice(b"threadidx");
    component(&mut key, channel_id.as_bytes());
    key.extend_from_slice(&root_seq.to_be_bytes());
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
                if let Mark::Mention(author) = mark {
                    if !mentions.contains(author) {
                        mentions.push(author.clone());
                    }
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

fn chat_config<E>(context: &E, id: &str) -> ChatConfig
where
    E: Context + BufferPooler,
{
    let page_cache = CacheRef::from_pooler(
        context,
        NonZeroU16::new(128).unwrap(),
        NonZeroUsize::new(64).unwrap(),
    );
    let codec_config = ((), (RangeCfg::from(0..=1 << 20), ()));

    VariableConfig {
        merkle_config: mmr::full::Config {
            journal_partition: format!("{id}-merkle-journal"),
            metadata_partition: format!("{id}-merkle-meta"),
            items_per_blob: NonZeroU64::new(64).unwrap(),
            write_buffer: NonZeroUsize::new(1024).unwrap(),
            strategy: Sequential,
            page_cache: page_cache.clone(),
        },
        journal_config: journal::contiguous::variable::Config {
            partition: format!("{id}-log"),
            items_per_section: NonZeroU64::new(64).unwrap(),
            write_buffer: NonZeroUsize::new(1024).unwrap(),
            compression: None,
            codec_config,
            page_cache,
        },
        translator: TwoCap,
    }
}

/// what a successful post staged — the inputs of the hook notifications.
struct Posted {
    seq: u64,
    thread_root: Option<u64>,
    hooks: Vec<String>,
}

/// storage-backed chat module.
pub struct Chat<E>
where
    E: Context + BufferPooler,
{
    id: ModuleId,
    db: ChatDb<E>,
    /// logical-key -> staged write for the current block; `None` = delete.
    pending: BTreeMap<Vec<u8>, Option<Vec<u8>>>,
    /// the tagging plane every post is reported to (one `TagEvent` follow-up
    /// per post, same block). `None` = no plane on this host (tests, minimal
    /// registries). the plane owns the loop rule and the subscription check;
    /// chat only translates its shapes at this edge.
    tagging: Option<ModuleId>,
}

impl<E> Chat<E>
where
    E: Context + BufferPooler,
{
    /// open or recover the store on `context` under module identity `id`.
    pub async fn init(context: E, id: impl Into<ModuleId>) -> Self {
        let id = id.into();
        let cfg = chat_config(&context, &id);
        let db = ChatDb::<E>::init(context, cfg)
            .await
            .expect("qmdb init failed");
        Self {
            id,
            db,
            pending: BTreeMap::new(),
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
        if let Some(value) = self.pending.get(key) {
            return Ok(value.clone());
        }
        self.db
            .get(&hash_key(key))
            .await
            .map_err(|e| Error::Module(format!("qmdb get failed: {e}")))
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
        self.pending.insert(
            key,
            Some(serde_json::to_vec(value).expect("chat value is serializable")),
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
        self.pending.insert(key, Some(bytes));
        Ok(())
    }

    fn delete(&mut self, key: Vec<u8>) {
        self.pending.insert(key, None);
    }

    async fn channel_index(&self) -> Result<BTreeSet<String>, Error> {
        Ok(self.load(CHANNEL_INDEX_KEY).await?.unwrap_or_default())
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

    async fn stage_channel(
        &mut self,
        channel_id: String,
        name: String,
        post_policy: PostPolicy,
        created_at: u64,
    ) -> Result<(), Error> {
        Self::validate_non_empty("channel_id", &channel_id)?;
        Self::validate_non_empty("name", &name)?;
        if self.channel(&channel_id).await?.is_some() {
            return Err(Error::Module(format!(
                "channel already exists: {channel_id}"
            )));
        }

        let channel = Channel {
            id: channel_id.clone(),
            name,
            created_at,
            head_seq: 0,
            post_policy,
            hooks: Vec::new(),
            pinned: Vec::new(),
            huddle: Vec::new(),
        };
        let mut index = self.channel_index().await?;
        index.insert(channel_id.clone());
        self.store_bounded(
            channel_key(&channel_id),
            &channel,
            MAX_CHANNEL_RECORD_BYTES,
            "channel",
        )?;
        self.store(CHANNEL_INDEX_KEY.to_vec(), &index);
        Ok(())
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
            // the root tracks the summary. a tombstoned root still anchors its
            // thread, so replying to it stays legal.
            let mut root = self.require_head(channel_id, root_seq).await?;
            if root.thread.is_some() {
                return Err(Error::Module(format!(
                    "thread replies cannot start subthreads: {channel_id}/{root_seq}"
                )));
            }
            let mut replies: Vec<u64> = self
                .load(&threadidx_key(channel_id, root_seq))
                .await?
                .unwrap_or_default();
            if replies.len() >= MAX_THREAD_REPLIES {
                return Err(Error::Module(format!(
                    "thread reply cap reached: {channel_id}/{root_seq}"
                )));
            }
            replies.push(seq);
            root.reply_count += 1;
            root.last_reply_seq = Some(seq);
            self.store_bounded(
                msg_key(channel_id, root_seq),
                &root,
                MAX_MESSAGE_HEAD_BYTES,
                "message",
            )?;
            self.store(threadidx_key(channel_id, root_seq), &replies);
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
    ) -> Result<(), Error> {
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
        let new_head = MessageHead {
            blocks,
            rev: head.rev + 1,
            edited_at: Some(now),
            base_rev,
            ..head
        };
        self.store_bounded(
            msg_key(channel_id, seq),
            &new_head,
            MAX_MESSAGE_HEAD_BYTES,
            "message",
        )
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

    /// hook-origin hygiene: when the emitter is a MODULE, it may only
    /// (un)register ITSELF — `module_id` must equal the origin, so no module
    /// can wire or unwire another module behind the operator's back. external
    /// and system origins pass through (operator wiring).
    fn require_module_self(origin: &Origin, module_id: &str) -> Result<(), Error> {
        match origin {
            Origin::Module(emitter) if emitter != module_id => Err(Error::Module(format!(
                "a module origin may only (un)register itself as a hook \
                 (emitter {emitter:?}, target {module_id:?})"
            ))),
            _ => Ok(()),
        }
    }

    async fn stage_register_hook(
        &mut self,
        channel_id: &str,
        module_id: String,
    ) -> Result<(), Error> {
        Self::validate_non_empty("channel_id", channel_id)?;
        Self::validate_non_empty("module_id", &module_id)?;
        let mut channel = self.require_channel(channel_id).await?;
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

    async fn stage_unregister_hook(
        &mut self,
        channel_id: &str,
        module_id: &str,
    ) -> Result<(), Error> {
        Self::validate_non_empty("channel_id", channel_id)?;
        let mut channel = self.require_channel(channel_id).await?;
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

    async fn stage_membership(
        &mut self,
        channel_id: &str,
        user: Vec<u8>,
        member: bool,
    ) -> Result<(), Error> {
        Self::validate_non_empty("channel_id", channel_id)?;
        if user.is_empty() {
            return Err(Error::Module("user must not be empty".into()));
        }
        self.require_channel(channel_id).await?;
        let mut index: BTreeSet<Vec<u8>> = self
            .load(&memberidx_key(channel_id))
            .await?
            .unwrap_or_default();
        let changed = if member {
            index.insert(user.clone())
        } else {
            index.remove(&user)
        };
        if !changed {
            return Ok(());
        }
        if member {
            self.store(member_key(channel_id, &user), &true);
        } else {
            self.delete(member_key(channel_id, &user));
        }
        self.store_bounded(
            memberidx_key(channel_id),
            &index,
            MAX_CHANNEL_RECORD_BYTES,
            "membership index",
        )
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

    /// evict `user` from the channel's huddle (staleness cleanup — see
    /// `ChatMsg::SweepHuddle`). gated like posting; absent target = no-op.
    async fn stage_sweep_huddle(
        &mut self,
        author: AuthorRef,
        channel_id: &str,
        user: &[u8],
    ) -> Result<(), Error> {
        Self::validate_non_empty("channel_id", channel_id)?;
        let AuthorRef::User(_) = &author else {
            return Err(Error::Module(
                "only external users may sweep a huddle".into(),
            ));
        };
        let mut channel = self.require_channel(channel_id).await?;
        self.check_post_policy(&channel, &author).await?;
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

    // ---- query assembly ------------------------------------------------------

    async fn reactions(&self, channel_id: &str, seq: u64) -> Result<Vec<ReactionSummary>, Error> {
        let emojis: BTreeSet<String> = self
            .load(&reactidx_key(channel_id, seq))
            .await?
            .unwrap_or_default();
        let mut summaries = Vec::with_capacity(emojis.len());
        for emoji in emojis {
            let reactors: BTreeSet<AuthorRef> = self
                .load(&react_key(channel_id, seq, &emoji))
                .await?
                .unwrap_or_default();
            if !reactors.is_empty() {
                summaries.push(ReactionSummary { emoji, reactors });
            }
        }
        Ok(summaries)
    }

    async fn view(&self, channel: &Channel, seq: u64) -> Result<Option<MessageView>, Error> {
        let Some(head) = self.head(&channel.id, seq).await? else {
            return Ok(None);
        };
        Ok(Some(MessageView {
            channel_id: channel.id.clone(),
            seq,
            head,
            reactions: self.reactions(&channel.id, seq).await?,
            channel_head_seq: channel.head_seq,
        }))
    }

    /// point-lookup one page of message views for computed sequences. the
    /// sequence space is gap-free (P3), so a missing head is a store bug.
    async fn views(
        &self,
        channel: &Channel,
        seqs: impl Iterator<Item = u64>,
    ) -> Result<Vec<MessageView>, Error> {
        let mut views = Vec::new();
        for seq in seqs {
            let view = self.view(channel, seq).await?.ok_or_else(|| {
                Error::Module(format!("missing message record: {}/{seq}", channel.id))
            })?;
            views.push(view);
        }
        Ok(views)
    }

    async fn channels(&self) -> Result<Vec<Channel>, Error> {
        let index = self.channel_index().await?;
        let mut channels = Vec::with_capacity(index.len());
        for id in index {
            let channel = self
                .channel(&id)
                .await?
                .ok_or_else(|| Error::Module(format!("missing channel record: {id}")))?;
            channels.push(channel);
        }
        Ok(channels)
    }

    async fn messages_latest(
        &self,
        channel_id: &str,
        limit: u64,
    ) -> Result<Vec<MessageView>, Error> {
        let channel = self.require_channel(channel_id).await?;
        let limit = clamp_limit(limit);
        if limit == 0 || channel.head_seq == 0 {
            return Ok(Vec::new());
        }
        let from = channel.head_seq.saturating_sub(limit - 1).max(1);
        self.views(&channel, from..=channel.head_seq).await
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
        self.views(&channel, from..=to).await
    }

    async fn message_by_id(&self, message_id: &str) -> Result<Option<MessageView>, Error> {
        let Some((channel_id, seq)) = self.load::<(String, u64)>(&msgid_key(message_id)).await?
        else {
            return Ok(None);
        };
        let channel = self.require_channel(&channel_id).await?;
        self.view(&channel, seq).await
    }

    async fn revisions(&self, channel_id: &str, seq: u64) -> Result<Vec<MessageHead>, Error> {
        let Some(head) = self.head(channel_id, seq).await? else {
            return Ok(Vec::new());
        };
        let mut revisions = Vec::with_capacity(head.rev as usize);
        for rev in 0..head.rev {
            let prior: MessageHead = self
                .load(&rev_key(channel_id, seq, rev))
                .await?
                .ok_or_else(|| {
                    Error::Module(format!("missing revision record: {channel_id}/{seq}/{rev}"))
                })?;
            revisions.push(prior);
        }
        Ok(revisions)
    }

    async fn thread(
        &self,
        channel_id: &str,
        root_seq: u64,
        from: u64,
        limit: u64,
    ) -> Result<Option<Thread>, Error> {
        let channel = self.require_channel(channel_id).await?;
        let Some(root) = self.view(&channel, root_seq).await? else {
            return Ok(None);
        };
        if root.head.thread.is_some() {
            // a reply is not a thread root.
            return Ok(None);
        }
        let reply_seqs: Vec<u64> = self
            .load(&threadidx_key(channel_id, root_seq))
            .await?
            .unwrap_or_default();
        let limit = clamp_limit(limit) as usize;
        let from = usize::try_from(from).unwrap_or(usize::MAX);
        let page = reply_seqs
            .iter()
            .skip(from)
            .take(limit)
            .copied()
            .collect::<Vec<_>>();
        let replies = self.views(&channel, page.into_iter()).await?;
        Ok(Some(Thread { root, replies }))
    }

    async fn members(&self, channel_id: &str) -> Result<Vec<Vec<u8>>, Error> {
        let index: BTreeSet<Vec<u8>> = self
            .load(&memberidx_key(channel_id))
            .await?
            .unwrap_or_default();
        Ok(index.into_iter().collect())
    }

    // ---- state-sync ---------------------------------------------------------
    // the joiner pulls a proven qmdb operation range rather than replaying
    // exported records. the target root is the trust anchor.

    pub async fn sync_target(&self) -> ChatTarget {
        let end = self.db.bounds().await.end;
        let start = self.db.sync_boundary();
        Target {
            root: self.db.root(),
            range: NonEmptyRange::new(start..end)
                .expect("a committed store has a non-empty op range"),
        }
    }

    pub fn into_resolver(self) -> Arc<ChatDb<E>> {
        Arc::new(self.db)
    }

    pub async fn sync_from<R>(
        context: E,
        id: impl Into<ModuleId>,
        target: ChatTarget,
        resolver: R,
    ) -> Result<Self, String>
    where
        R: DbResolver<ChatDb<E>>,
    {
        let id = id.into();
        let db_config = chat_config(&context, &id);
        let config = SyncConfig {
            context,
            resolver,
            target,
            max_outstanding_requests: 1,
            fetch_batch_size: NonZeroU64::new(64).unwrap(),
            apply_batch_size: 1024,
            db_config,
            update_rx: None,
            finish_rx: None,
            reached_target_tx: None,
            max_retained_roots: 8,
        };
        // a sync failure (transport blip, dropped source) is the caller's
        // retry loop to own — never a process kill.
        let db = sync::sync(config)
            .await
            .map_err(|e| format!("qmdb sync: {e:?}"))?;
        Ok(Self {
            id,
            db,
            pending: BTreeMap::new(),
            tagging: None,
        })
    }
}

#[async_trait::async_trait(?Send)]
impl<E> Module for Chat<E>
where
    E: Context + BufferPooler,
{
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        StateRoot(self.db.root().0)
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::ResolverBacked {
            backend: "qmdb".into(),
            detail: "serve_sync answers qmdb op-range requests (statesync wire)".into(),
        })
    }

    /// the network state-sync serve lane: answers the shared qmdb wire requests
    /// (historical proof-carrying op ranges) from committed state. read-only;
    /// the joiner's sync engine merkle-verifies every batch.
    async fn serve_sync(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        statesync::qmdb::serve_bytes(&self.db, req).await
    }

    async fn resolver_sync_target(&self) -> Result<ResolverSyncTarget, Error> {
        statesync::qmdb::resolver_sync_target(&self.db).await
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
            } => self.stage_channel(channel_id, name, post_policy, now).await,
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
                self.stage_edit(author, &channel_id, seq, blocks, base_rev, now)
                    .await
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
                // hook hygiene: a MODULE origin may only register ITSELF
                // (spoof-proof self-subscription); external (operator) origins
                // may wire any registered module — automations depends on it.
                Self::require_module_self(&ctx.env().origin, &module_id)?;
                // the target must be a registered module other than chat
                // itself, or every later post would poison the block.
                if module_id == self.id {
                    return Err(Error::Module("chat cannot hook itself".into()));
                }
                if ctx.module_root(&module_id).is_none() {
                    return Err(Error::Module(format!("unknown hook module: {module_id}")));
                }
                self.stage_register_hook(&channel_id, module_id).await
            }
            ChatMsg::UnregisterHook {
                channel_id,
                module_id,
            } => {
                // same origin rule as RegisterHook: a module may not unwire
                // ANOTHER module's subscription.
                Self::require_module_self(&ctx.env().origin, &module_id)?;
                self.stage_unregister_hook(&channel_id, &module_id).await
            }
            ChatMsg::SetMembership {
                channel_id,
                user,
                member,
            } => self.stage_membership(&channel_id, user, member).await,
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
            ChatQuery::Channels => Ok(encode_reply(&ChatReply::Channels(self.channels().await?))),
            ChatQuery::Channel { channel_id } => Ok(encode_reply(&ChatReply::Channel(
                self.channel(&channel_id).await?,
            ))),
            ChatQuery::MessagesLatest { channel_id, limit } => Ok(encode_reply(
                &ChatReply::Messages(self.messages_latest(&channel_id, limit).await?),
            )),
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
            ChatQuery::Revisions { channel_id, seq } => Ok(encode_reply(&ChatReply::Revisions(
                self.revisions(&channel_id, seq).await?,
            ))),
            ChatQuery::Thread {
                channel_id,
                root_seq,
                from,
                limit,
            } => Ok(encode_reply(&ChatReply::Thread(
                self.thread(&channel_id, root_seq, from, limit).await?,
            ))),
            ChatQuery::Reactions { channel_id, seq } => Ok(encode_reply(&ChatReply::Reactions(
                self.reactions(&channel_id, seq).await?,
            ))),
            ChatQuery::Members { channel_id } => Ok(encode_reply(&ChatReply::Members(
                self.members(&channel_id).await?,
            ))),
        }
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        if self.pending.is_empty() {
            return Ok(());
        }

        let mut batch = self.db.new_batch();
        for (key, value) in &self.pending {
            batch = batch.write(hash_key(key), value.clone());
        }
        let batch = batch
            .merkleize(&self.db, None::<Vec<u8>>)
            .await
            .expect("merkleize failed");
        self.db
            .apply_batch(batch)
            .await
            .expect("apply_batch failed");
        self.db.commit().await.expect("commit failed");
        self.pending.clear();
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending.clear();
        Ok(())
    }
}
