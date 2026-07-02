//! qmdb-backed messaging module with explicit channels.
//!
//! the module stores a small set of logical records in one commonware qmdb:
//! a channel index, per-channel records, per-channel message histories, and
//! message-id markers for global duplicate detection. like `document` and `kv`,
//! writes are staged in memory during a block and flushed to qmdb in one batch at
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

fn messages_key(channel_id: &str) -> Vec<u8> {
    keyed(b"messages", channel_id)
}

fn keyed_component(key: &mut Vec<u8>, value: &str) {
    key.extend_from_slice(&(value.len() as u64).to_be_bytes());
    key.extend_from_slice(value.as_bytes());
}

fn thread_key(channel_id: &str, thread_id: &str) -> Vec<u8> {
    let mut key = Vec::with_capacity(b"thread".len() + 16 + channel_id.len() + thread_id.len());
    key.extend_from_slice(b"thread");
    keyed_component(&mut key, channel_id);
    keyed_component(&mut key, thread_id);
    key
}

fn message_id_key(message_id: &str) -> Vec<u8> {
    keyed(b"message-id", message_id)
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

    async fn messages(&self, channel_id: &str) -> Result<Vec<ChatMessage>, Error> {
        let mut messages: Vec<ChatMessage> = self
            .load(&messages_key(channel_id))
            .await?
            .unwrap_or_default();
        messages.sort_by(|a, b| a.sequence.cmp(&b.sequence).then_with(|| a.id.cmp(&b.id)));
        Ok(messages)
    }

    async fn thread_replies(
        &self,
        channel_id: &str,
        thread_id: &str,
    ) -> Result<Vec<ChatMessage>, Error> {
        let mut replies: Vec<ChatMessage> = self
            .load(&thread_key(channel_id, thread_id))
            .await?
            .unwrap_or_default();
        replies.sort_by(|a, b| a.sequence.cmp(&b.sequence).then_with(|| a.id.cmp(&b.id)));
        Ok(replies)
    }

    async fn thread(&self, channel_id: &str, thread_id: &str) -> Result<Option<Thread>, Error> {
        let messages = self.messages(channel_id).await?;
        let Some(root) = messages.into_iter().find(|m| m.id == thread_id) else {
            return Ok(None);
        };
        Ok(Some(Thread {
            root,
            replies: self.thread_replies(channel_id, thread_id).await?,
        }))
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
        if self.channel(&channel_id).await?.is_none() {
            return Err(Error::Module(format!("unknown channel: {channel_id}")));
        }
        if self.message_exists(&message_id).await? {
            return Err(Error::Module(format!(
                "message already exists: {message_id}"
            )));
        }

        let mut messages = self.messages(&channel_id).await?;
        let sequence = messages.last().map_or(1, |m| m.sequence + 1);
        messages.push(ChatMessage {
            id: message_id.clone(),
            channel_id: channel_id.clone(),
            author,
            body,
            sequence,
            sent_at,
            thread_id: None,
            reply_count: 0,
            last_reply_at: None,
        });
        self.store(messages_key(&channel_id), &messages);
        self.store(message_id_key(&message_id), &channel_id);
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
        if self.channel(&channel_id).await?.is_none() {
            return Err(Error::Module(format!("unknown channel: {channel_id}")));
        }
        if self.message_exists(&message_id).await? {
            return Err(Error::Module(format!(
                "message already exists: {message_id}"
            )));
        }

        let mut messages = self.messages(&channel_id).await?;
        let Some(root) = messages.iter_mut().find(|m| m.id == thread_id) else {
            return Err(Error::Module(format!("unknown thread: {thread_id}")));
        };
        if root.thread_id.is_some() {
            return Err(Error::Module(format!(
                "thread replies cannot start subthreads: {thread_id}"
            )));
        }

        let mut replies = self.thread_replies(&channel_id, &thread_id).await?;
        let sequence = replies.last().map_or(1, |m| m.sequence + 1);
        replies.push(ChatMessage {
            id: message_id.clone(),
            channel_id: channel_id.clone(),
            author,
            body,
            sequence,
            sent_at,
            thread_id: Some(thread_id.clone()),
            reply_count: 0,
            last_reply_at: None,
        });

        root.reply_count += 1;
        root.last_reply_at = Some(sent_at);
        self.store(messages_key(&channel_id), &messages);
        self.store(thread_key(&channel_id, &thread_id), &replies);
        self.store(message_id_key(&message_id), &channel_id);
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
            detail: "call Messaging::sync_target() at this root and serve it with a DbResolver"
                .into(),
        })
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
            MessagingQuery::Messages { channel_id } => Ok(encode_reply(&MessagingReply::Messages(
                self.messages(&channel_id).await?,
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
