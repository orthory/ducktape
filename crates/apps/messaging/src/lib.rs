//! qmdb-backed messaging module with explicit channels.
//!
//! ## per-message storage (why one value per message, not one per channel)
//!
//! each message is a SEPARATE qmdb record keyed `(channel, sequence)`, and a
//! per-channel counter record holds the next sequence. a post writes exactly
//! two small records — the message and the bumped counter — so posting is O(1)
//! regardless of channel size. the earlier layout stored a channel's WHOLE
//! history as one value: every post re-read, appended, and rewrote it (O(n²)
//! writes), and once that value crossed the journal codec's 1 MiB bound
//! `commit_block` panicked — a busy channel was a deterministic whole-network
//! liveness kill. per-message keys remove both the amplification and the bomb;
//! a per-message body cap keeps any single value far under the bound.
//!
//! reads enumerate by counter (qmdb hashes keys, so there is no range scan):
//! `messages` walks `1..=count`, and the paginated query walks only a window.
//! writes stage in memory during a block and flush in one qmdb batch at
//! `commit_block`; the module root is the real qmdb root and the joiner path is
//! commonware storage sync.

use std::collections::BTreeMap;
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
use messaging_interface::{
    Channel, ChatMessage, MessagingMsg, MessagingQuery, MessagingReply, Thread, decode_msg,
    decode_query, encode_reply,
};
use sdk::{Ctx, Error, Module, ModuleId, Msg, StateRoot, StateSyncHandle};
use serde::{Serialize, de::DeserializeOwned};

/// the qmdb key: a fixed-width digest of a logical messaging record key.
type MessagingKey = <Sha256 as Hasher>::Digest;

/// one variable-value qmdb stores all messaging records.
pub type MessagingDb<E> = Db<mmr::Family, E, MessagingKey, Vec<u8>, Sha256, TwoCap, Sequential>;

/// shared by fresh open and state-sync reconstruction so storage layout cannot
/// drift between source and joiner.
type MessagingConfig = VariableConfig<TwoCap, ((), (RangeCfg<usize>, ())), Sequential>;

/// a storage-sync target: qmdb root plus the active operation range.
pub type MessagingTarget = Target<mmr::Family, MessagingKey>;

const CHANNEL_INDEX_KEY: &[u8] = b"channel-index";

/// per-message body ceiling. keeps any single qmdb value far below the journal
/// codec's 1 MiB bound, so a post can never grow a value past what
/// `commit_block` can flush. generous for chat; a file/attachment belongs in a
/// blob-addressed store, not inline.
const MAX_BODY_LEN: usize = 16 * 1024;

fn hash_key(key: &[u8]) -> MessagingKey {
    let mut h = Sha256::new();
    h.update(key);
    h.finalize()
}

fn keyed(prefix: &[u8], id: &str) -> Vec<u8> {
    let mut key = Vec::with_capacity(prefix.len() + 1 + id.len());
    key.extend_from_slice(prefix);
    key.push(0);
    key.extend_from_slice(id.as_bytes());
    key
}

fn channel_key(channel_id: &str) -> Vec<u8> {
    keyed(b"channel", channel_id)
}

/// the per-channel message COUNTER record: the number of top-level messages,
/// which is also the highest assigned sequence (sequences are 1-based dense).
fn channel_count_key(channel_id: &str) -> Vec<u8> {
    keyed(b"channel-count", channel_id)
}

fn keyed_component(key: &mut Vec<u8>, value: &str) {
    key.extend_from_slice(&(value.len() as u64).to_be_bytes());
    key.extend_from_slice(value.as_bytes());
}

/// one message's record key: `(channel, sequence)`. length-prefixed channel so
/// no two `(channel, seq)` pairs can collide across channel-name boundaries.
fn message_at_key(channel_id: &str, sequence: u64) -> Vec<u8> {
    let mut key = Vec::with_capacity(b"message-at".len() + 16 + channel_id.len());
    key.extend_from_slice(b"message-at");
    keyed_component(&mut key, channel_id);
    key.extend_from_slice(&sequence.to_be_bytes());
    key
}

/// the per-thread reply COUNTER record.
fn thread_count_key(channel_id: &str, thread_id: &str) -> Vec<u8> {
    let mut key = Vec::with_capacity(b"thread-count".len() + 24 + channel_id.len() + thread_id.len());
    key.extend_from_slice(b"thread-count");
    keyed_component(&mut key, channel_id);
    keyed_component(&mut key, thread_id);
    key
}

/// one thread reply's record key: `(channel, thread, sequence)`.
fn thread_reply_at_key(channel_id: &str, thread_id: &str, sequence: u64) -> Vec<u8> {
    let mut key =
        Vec::with_capacity(b"thread-at".len() + 24 + channel_id.len() + thread_id.len());
    key.extend_from_slice(b"thread-at");
    keyed_component(&mut key, channel_id);
    keyed_component(&mut key, thread_id);
    key.extend_from_slice(&sequence.to_be_bytes());
    key
}

/// message-id marker: locates any message by its id for O(1) dedup AND for
/// resolving a thread root's `(channel, seq)` without scanning.
fn message_id_key(message_id: &str) -> Vec<u8> {
    keyed(b"message-id", message_id)
}

/// where a message lives, recorded under its id: its channel and its sequence
/// (top-level or within a thread). lets a thread reply find and update its root
/// message in place, and dedup stay O(1).
#[derive(Serialize, serde::Deserialize, Clone)]
struct MessageLocation {
    channel_id: String,
    sequence: u64,
    /// `None` for a top-level message; `Some(thread)` for a reply.
    thread_id: Option<String>,
}

fn messaging_config<E>(context: &E, id: &str) -> MessagingConfig
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

/// storage-backed messaging module.
pub struct Messaging<E>
where
    E: Context + BufferPooler,
{
    id: ModuleId,
    db: MessagingDb<E>,
    /// logical-key -> serialized value writes staged for the current block.
    pending: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl<E> Messaging<E>
where
    E: Context + BufferPooler,
{
    /// open or recover the store on `context` under module identity `id`.
    pub async fn init(context: E, id: impl Into<ModuleId>) -> Self {
        let id = id.into();
        let cfg = messaging_config(&context, &id);
        let db = MessagingDb::<E>::init(context, cfg)
            .await
            .expect("qmdb init failed");
        Self {
            id,
            db,
            pending: BTreeMap::new(),
        }
    }

    fn validate_non_empty(field: &str, value: &str) -> Result<(), Error> {
        if value.is_empty() {
            return Err(Error::Module(format!("{field} must not be empty")));
        }
        Ok(())
    }

    async fn get_raw(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        if let Some(value) = self.pending.get(key) {
            return Ok(Some(value.clone()));
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
            serde_json::to_vec(value).expect("messaging value is serializable"),
        );
    }

    async fn channel_index(&self) -> Result<BTreeMap<String, Channel>, Error> {
        Ok(self.load(CHANNEL_INDEX_KEY).await?.unwrap_or_default())
    }

    async fn channel(&self, channel_id: &str) -> Result<Option<Channel>, Error> {
        self.load(&channel_key(channel_id)).await
    }

    async fn channels(&self) -> Result<Vec<Channel>, Error> {
        Ok(self.channel_index().await?.into_values().collect())
    }

    async fn channel_message_count(&self, channel_id: &str) -> Result<u64, Error> {
        Ok(self.load(&channel_count_key(channel_id)).await?.unwrap_or(0))
    }

    async fn thread_reply_count(&self, channel_id: &str, thread_id: &str) -> Result<u64, Error> {
        Ok(self
            .load(&thread_count_key(channel_id, thread_id))
            .await?
            .unwrap_or(0))
    }

    async fn message_at(
        &self,
        channel_id: &str,
        sequence: u64,
    ) -> Result<Option<ChatMessage>, Error> {
        self.load(&message_at_key(channel_id, sequence)).await
    }

    /// a page of a channel's top-level messages, newest-first, by counter walk.
    /// `before` is an exclusive keyset cursor (return `sequence < before`);
    /// `limit` caps the page. `before = None, limit = None` returns the whole
    /// history ascending — the pre-pagination behavior.
    async fn messages_page(
        &self,
        channel_id: &str,
        before: Option<u64>,
        limit: Option<u32>,
    ) -> Result<Vec<ChatMessage>, Error> {
        let count = self.channel_message_count(channel_id).await?;
        // walk downward from the newest in-window sequence.
        let mut seq = before.map_or(count, |b| b.saturating_sub(1).min(count));
        let cap = limit.map(|l| l as usize);
        let mut out: Vec<ChatMessage> = Vec::new();
        while seq >= 1 {
            if let Some(cap) = cap {
                if out.len() >= cap {
                    break;
                }
            }
            if let Some(m) = self.message_at(channel_id, seq).await? {
                out.push(m);
            }
            seq -= 1;
        }
        // unpaginated callers historically saw ascending order; preserve that
        // for the whole-history read, but a paginated page is newest-first.
        if before.is_none() && limit.is_none() {
            out.reverse();
        }
        Ok(out)
    }

    async fn messages(&self, channel_id: &str) -> Result<Vec<ChatMessage>, Error> {
        self.messages_page(channel_id, None, None).await
    }

    async fn thread_replies(
        &self,
        channel_id: &str,
        thread_id: &str,
    ) -> Result<Vec<ChatMessage>, Error> {
        let count = self.thread_reply_count(channel_id, thread_id).await?;
        let mut replies: Vec<ChatMessage> = Vec::with_capacity(count as usize);
        for seq in 1..=count {
            if let Some(m) = self
                .load(&thread_reply_at_key(channel_id, thread_id, seq))
                .await?
            {
                replies.push(m);
            }
        }
        Ok(replies)
    }

    async fn thread(&self, channel_id: &str, thread_id: &str) -> Result<Option<Thread>, Error> {
        let Some(loc) = self.locate(thread_id).await? else {
            return Ok(None);
        };
        // a thread root must be a top-level message in this channel.
        if loc.channel_id != channel_id || loc.thread_id.is_some() {
            return Ok(None);
        }
        let Some(root) = self.message_at(channel_id, loc.sequence).await? else {
            return Ok(None);
        };
        Ok(Some(Thread {
            root,
            replies: self.thread_replies(channel_id, thread_id).await?,
        }))
    }

    async fn locate(&self, message_id: &str) -> Result<Option<MessageLocation>, Error> {
        self.load(&message_id_key(message_id)).await
    }

    async fn message_exists(&self, message_id: &str) -> Result<bool, Error> {
        Ok(self.get_raw(&message_id_key(message_id)).await?.is_some())
    }

    async fn stage_channel(
        &mut self,
        channel_id: String,
        name: String,
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
        };
        let mut index = self.channel_index().await?;
        index.insert(channel_id.clone(), channel.clone());
        self.store(channel_key(&channel_id), &channel);
        self.store(CHANNEL_INDEX_KEY.to_vec(), &index);
        Ok(())
    }

    fn validate_body(body: &str) -> Result<(), Error> {
        Self::validate_non_empty("body", body)?;
        if body.len() > MAX_BODY_LEN {
            return Err(Error::Module(format!(
                "body exceeds the {MAX_BODY_LEN}-byte ceiling"
            )));
        }
        Ok(())
    }

    async fn stage_message(
        &mut self,
        channel_id: String,
        message_id: String,
        author: String,
        body: String,
        sent_at: u64,
    ) -> Result<(), Error> {
        Self::validate_non_empty("channel_id", &channel_id)?;
        Self::validate_non_empty("message_id", &message_id)?;
        Self::validate_non_empty("author", &author)?;
        Self::validate_body(&body)?;
        if self.channel(&channel_id).await?.is_none() {
            return Err(Error::Module(format!("unknown channel: {channel_id}")));
        }
        if self.message_exists(&message_id).await? {
            return Err(Error::Module(format!(
                "message already exists: {message_id}"
            )));
        }

        // O(1): read the counter, write ONE message record + the bumped counter
        // + the id marker. no whole-history rewrite.
        let sequence = self.channel_message_count(&channel_id).await? + 1;
        let message = ChatMessage {
            id: message_id.clone(),
            channel_id: channel_id.clone(),
            author,
            body,
            sequence,
            sent_at,
            thread_id: None,
            reply_count: 0,
            last_reply_at: None,
        };
        self.store(message_at_key(&channel_id, sequence), &message);
        self.store(channel_count_key(&channel_id), &sequence);
        self.store(
            message_id_key(&message_id),
            &MessageLocation { channel_id, sequence, thread_id: None },
        );
        Ok(())
    }

    async fn stage_thread_reply(
        &mut self,
        channel_id: String,
        thread_id: String,
        message_id: String,
        author: String,
        body: String,
        sent_at: u64,
    ) -> Result<(), Error> {
        Self::validate_non_empty("channel_id", &channel_id)?;
        Self::validate_non_empty("thread_id", &thread_id)?;
        Self::validate_non_empty("message_id", &message_id)?;
        Self::validate_non_empty("author", &author)?;
        Self::validate_body(&body)?;
        if self.channel(&channel_id).await?.is_none() {
            return Err(Error::Module(format!("unknown channel: {channel_id}")));
        }
        if self.message_exists(&message_id).await? {
            return Err(Error::Module(format!(
                "message already exists: {message_id}"
            )));
        }

        // locate the root message by id (O(1)) — it must be a top-level message
        // in this channel, not itself a reply.
        let root_loc = self
            .locate(&thread_id)
            .await?
            .filter(|l| l.channel_id == channel_id && l.thread_id.is_none())
            .ok_or_else(|| Error::Module(format!("unknown thread: {thread_id}")))?;
        let mut root = self
            .message_at(&channel_id, root_loc.sequence)
            .await?
            .ok_or_else(|| Error::Module(format!("thread root vanished: {thread_id}")))?;

        // O(1): one reply record + bumped thread counter + the root's updated
        // metadata (a single small record) + the id marker.
        let sequence = self.thread_reply_count(&channel_id, &thread_id).await? + 1;
        let reply = ChatMessage {
            id: message_id.clone(),
            channel_id: channel_id.clone(),
            author,
            body,
            sequence,
            sent_at,
            thread_id: Some(thread_id.clone()),
            reply_count: 0,
            last_reply_at: None,
        };
        root.reply_count += 1;
        root.last_reply_at = Some(sent_at);

        self.store(thread_reply_at_key(&channel_id, &thread_id, sequence), &reply);
        self.store(thread_count_key(&channel_id, &thread_id), &sequence);
        self.store(message_at_key(&channel_id, root_loc.sequence), &root);
        self.store(
            message_id_key(&message_id),
            &MessageLocation {
                channel_id,
                sequence,
                thread_id: Some(thread_id),
            },
        );
        Ok(())
    }

    // ---- state-sync ---------------------------------------------------------
    // the joiner pulls a proven qmdb operation range rather than replaying
    // exported records. the target root is the trust anchor.

    pub async fn sync_target(&self) -> MessagingTarget {
        let end = self.db.bounds().await.end;
        let start = self.db.sync_boundary();
        Target {
            root: self.db.root(),
            range: NonEmptyRange::new(start..end)
                .expect("a committed store has a non-empty op range"),
        }
    }

    pub fn into_resolver(self) -> Arc<MessagingDb<E>> {
        Arc::new(self.db)
    }

    pub async fn sync_from<R>(
        context: E,
        id: impl Into<ModuleId>,
        target: MessagingTarget,
        resolver: R,
    ) -> Self
    where
        R: DbResolver<MessagingDb<E>>,
    {
        let id = id.into();
        let db_config = messaging_config(&context, &id);
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
        let db = sync::sync(config).await.expect("qmdb sync failed");
        Self {
            id,
            db,
            pending: BTreeMap::new(),
        }
    }
}

#[async_trait::async_trait(?Send)]
impl<E> Module for Messaging<E>
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
            detail: "serve_sync answers qmdb target + op-range requests (statesync wire)".into(),
        })
    }

    /// the network state-sync serve lane: answers the shared qmdb wire requests
    /// (current target, historical proof-carrying op ranges) from committed
    /// state. read-only; the joiner's sync engine merkle-verifies every batch.
    async fn serve_sync(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        statesync::qmdb::serve_bytes(&self.db, req).await
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            MessagingMsg::CreateChannel { channel_id, name } => {
                self.stage_channel(channel_id, name, ctx.env().consensus_time)
                    .await
            }
            MessagingMsg::PostMessage {
                channel_id,
                message_id,
                author,
                body,
            } => {
                self.stage_message(
                    channel_id,
                    message_id,
                    author,
                    body,
                    ctx.env().consensus_time,
                )
                .await
            }
            MessagingMsg::PostThreadReply {
                channel_id,
                thread_id,
                message_id,
                author,
                body,
            } => {
                self.stage_thread_reply(
                    channel_id,
                    thread_id,
                    message_id,
                    author,
                    body,
                    ctx.env().consensus_time,
                )
                .await
            }
        }
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            MessagingQuery::Channels => Ok(encode_reply(&MessagingReply::Channels(
                self.channels().await?,
            ))),
            MessagingQuery::Channel { channel_id } => Ok(encode_reply(&MessagingReply::Channel(
                self.channel(&channel_id).await?,
            ))),
            MessagingQuery::Messages {
                channel_id,
                before,
                limit,
            } => Ok(encode_reply(&MessagingReply::Messages(
                self.messages_page(&channel_id, before, limit).await?,
            ))),
            MessagingQuery::Thread {
                channel_id,
                thread_id,
            } => Ok(encode_reply(&MessagingReply::Thread(
                self.thread(&channel_id, &thread_id).await?,
            ))),
        }
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        if self.pending.is_empty() {
            return Ok(());
        }

        let mut batch = self.db.new_batch();
        for (key, value) in &self.pending {
            batch = batch.write(hash_key(key), Some(value.clone()));
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
