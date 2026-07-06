//! qmdb-backed comments module — threads anchored to an addressable record.
//!
//! one record per qmdb key (`sha256(logical_key)`): a thread under `t:<id>`, a
//! comment under `c:<id>`, and a reserved per-anchor index under
//! `\0a:<json(anchor)>` holding the sorted thread ids on that anchor. writes
//! stage in an in-memory overlay and flush in one batch at `commit_block`
//! (`abort_block` drops it), exactly like the pages/chat modules; state-sync
//! delegates to commonware's qmdb sync engine.

mod interface;
pub use interface::*;

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

use sdk::{
    Ctx, Env, Error, Module, ModuleId, Msg, Origin, ResolverSyncTarget, StateRoot, StateSyncHandle,
};

/// write-time cap on ONE serialized record (thread, comment, or index value).
/// leaves the same framing margin the pages module keeps under the 1 MiB codec
/// bound.
pub const MAX_RECORD_LEN: usize = 768 * 1024;

/// reserved logical-key prefix for the per-anchor thread index. its leading NUL
/// makes it uncollidable with a `t:`/`c:` record key.
const ANCHOR_INDEX_PREFIX: &str = "\u{0}a:";

type CommentKey = <Sha256 as Hasher>::Digest;
type CommentsDb<E> = Db<mmr::Family, E, CommentKey, Vec<u8>, Sha256, TwoCap, Sequential>;
type CommentsConfig = VariableConfig<TwoCap, ((), (RangeCfg<usize>, ())), Sequential>;
pub type CommentsTarget = Target<mmr::Family, CommentKey>;

fn hash_key(k: &[u8]) -> CommentKey {
    let mut h = Sha256::new();
    h.update(k);
    h.finalize()
}

fn thread_key(id: &str) -> String {
    format!("t:{id}")
}
fn comment_key(id: &str) -> String {
    format!("c:{id}")
}
fn anchor_index_key(anchor: &Anchor) -> String {
    format!(
        "{ANCHOR_INDEX_PREFIX}{}",
        serde_json::to_string(anchor).expect("anchor serializable")
    )
}

fn comments_config<E>(context: &E, id: &str) -> CommentsConfig
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum CommentError {
    EmptyOrigin,
    ThreadNotFound,
    CommentNotFound,
    DuplicateComment,
    AnchorMismatch,
    NotAuthor,
    TextTooLarge,
    TooManyComments,
    TooManyThreads,
    TooManyTargets,
    ReservedId,
    Corrupt,
    Unsupported,
}

impl core::fmt::Display for CommentError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let s = match self {
            CommentError::EmptyOrigin => "empty origin",
            CommentError::ThreadNotFound => "thread not found",
            CommentError::CommentNotFound => "comment not found",
            CommentError::DuplicateComment => "duplicate comment id",
            CommentError::AnchorMismatch => "anchor mismatch",
            CommentError::NotAuthor => "not the comment author",
            CommentError::TextTooLarge => "comment text too large",
            CommentError::TooManyComments => "too many comments in thread",
            CommentError::TooManyThreads => "too many threads on anchor",
            CommentError::TooManyTargets => "too many query targets",
            CommentError::ReservedId => "reserved id",
            CommentError::Corrupt => "stored comment state is corrupt",
            CommentError::Unsupported => "unsupported",
        };
        f.write_str(s)
    }
}

/// derive the author from the dispatch origin (mirrors chat). the pre-consensus
/// default `Origin::External(vec![])` must never pass as a real user.
fn author_from_origin(origin: &Origin) -> Result<AuthorRef, CommentError> {
    match origin {
        Origin::External(bytes) if bytes.is_empty() => Err(CommentError::EmptyOrigin),
        Origin::External(bytes) => Ok(AuthorRef::User(bytes.clone())),
        Origin::Module(id) => Ok(AuthorRef::Module(id.to_string())),
        Origin::System => Ok(AuthorRef::System),
    }
}

pub struct Comments<E>
where
    E: Context + BufferPooler,
{
    id: ModuleId,
    db: CommentsDb<E>,
    pending: BTreeMap<Vec<u8>, Option<Vec<u8>>>,
}

impl<E> Comments<E>
where
    E: Context + BufferPooler,
{
    pub async fn init(context: E, id: impl Into<ModuleId>) -> Self {
        let id = id.into();
        let cfg = comments_config(&context, &id);
        let db = CommentsDb::<E>::init(context, cfg)
            .await
            .expect("qmdb init failed");
        Self {
            id,
            db,
            pending: BTreeMap::new(),
        }
    }

    async fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        if let Some(staged) = self.pending.get(key) {
            return staged.clone();
        }
        self.db.get(&hash_key(key)).await.expect("get failed")
    }

    fn stage(&mut self, key: &str, bytes: Vec<u8>) -> Result<(), CommentError> {
        if bytes.len() > MAX_RECORD_LEN {
            return Err(CommentError::TextTooLarge);
        }
        self.pending.insert(key.as_bytes().to_vec(), Some(bytes));
        Ok(())
    }

    fn delete_key(&mut self, key: &str) {
        self.pending.insert(key.as_bytes().to_vec(), None);
    }

    async fn load_thread(&self, id: &str) -> Result<Option<Thread>, CommentError> {
        match self.get(thread_key(id).as_bytes()).await {
            Some(b) => Ok(Some(
                serde_json::from_slice(&b).map_err(|_| CommentError::Corrupt)?,
            )),
            None => Ok(None),
        }
    }

    async fn load_comment(&self, id: &str) -> Result<Option<Comment>, CommentError> {
        match self.get(comment_key(id).as_bytes()).await {
            Some(b) => Ok(Some(
                serde_json::from_slice(&b).map_err(|_| CommentError::Corrupt)?,
            )),
            None => Ok(None),
        }
    }

    fn store_thread(&mut self, t: &Thread) -> Result<(), CommentError> {
        self.stage(
            &thread_key(&t.id),
            serde_json::to_vec(t).expect("thread serializable"),
        )
    }

    fn store_comment(&mut self, c: &Comment) -> Result<(), CommentError> {
        self.stage(
            &comment_key(&c.id),
            serde_json::to_vec(c).expect("comment serializable"),
        )
    }

    async fn load_anchor_index(&self, anchor: &Anchor) -> Result<Vec<String>, CommentError> {
        match self.get(anchor_index_key(anchor).as_bytes()).await {
            Some(b) => serde_json::from_slice(&b).map_err(|_| CommentError::Corrupt),
            None => Ok(Vec::new()),
        }
    }

    fn stage_anchor_index(&mut self, anchor: &Anchor, ids: &[String]) -> Result<(), CommentError> {
        if ids.is_empty() {
            self.delete_key(&anchor_index_key(anchor));
            Ok(())
        } else {
            self.stage(
                &anchor_index_key(anchor),
                serde_json::to_vec(ids).expect("ids serializable"),
            )
        }
    }

    /// a thread plus its LIVE (non-tombstoned) comments in order. `None` when
    /// the thread is absent. a listed comment missing from the store is
    /// corruption, surfaced loudly.
    async fn thread_view(&self, thread_id: &str) -> Result<Option<ThreadView>, CommentError> {
        let thread = match self.load_thread(thread_id).await? {
            Some(t) => t,
            None => return Ok(None),
        };
        let mut comments = Vec::new();
        for cid in &thread.comment_ids {
            let c = self.load_comment(cid).await?.ok_or(CommentError::Corrupt)?;
            if !c.deleted {
                comments.push(c);
            }
        }
        Ok(Some(ThreadView { thread, comments }))
    }

    /// apply one decoded msg with the derived author/time. every arm is a stub
    /// until Tasks 7–10.
    async fn apply(
        &mut self,
        _msg: CommentMsg,
        _author: AuthorRef,
        _now: u64,
    ) -> Result<(), CommentError> {
        Err(CommentError::Unsupported)
    }

    // ---- state-sync (verbatim from pages) ----
    pub async fn sync_target(&self) -> CommentsTarget {
        let end = self.db.bounds().await.end;
        let start = self.db.sync_boundary();
        Target {
            root: self.db.root(),
            range: NonEmptyRange::new(start..end)
                .expect("committed store has a non-empty op range"),
        }
    }
    pub fn into_resolver(self) -> Arc<CommentsDb<E>> {
        Arc::new(self.db)
    }
    pub async fn sync_from<R>(
        context: E,
        id: impl Into<ModuleId>,
        target: CommentsTarget,
        resolver: R,
    ) -> Result<Self, String>
    where
        R: DbResolver<CommentsDb<E>>,
    {
        let id = id.into();
        let db_config = comments_config(&context, &id);
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
        let db = sync::sync(config)
            .await
            .map_err(|e| format!("qmdb sync: {e:?}"))?;
        Ok(Self {
            id,
            db,
            pending: BTreeMap::new(),
        })
    }
}

#[async_trait::async_trait(?Send)]
impl<E> Module for Comments<E>
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
    async fn serve_sync(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        statesync::qmdb::serve_bytes(&self.db, req).await
    }
    async fn resolver_sync_target(&self) -> Result<ResolverSyncTarget, Error> {
        statesync::qmdb::resolver_sync_target(&self.db).await
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let env: &Env = ctx.env();
        let author = author_from_origin(&env.origin).map_err(|e| Error::Module(e.to_string()))?;
        let now = env.consensus_time;
        let m = decode_msg(&msg.payload).map_err(Error::Module)?;
        self.apply(m, author, now)
            .await
            .map_err(|e| Error::Module(e.to_string()))
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            CommentQuery::ThreadsForAnchors { .. } => {
                Ok(encode_reply(&CommentReply::Anchored(Vec::new())))
            }
            CommentQuery::Thread { .. } => Ok(encode_reply(&CommentReply::Thread(None))),
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
        self.db.apply_batch(batch).await.expect("apply_batch failed");
        self.db.commit().await.expect("commit failed");
        self.pending.clear();
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use commonware_runtime::{Runner as _, deterministic};

    #[test]
    fn a_staged_write_moves_the_root() {
        deterministic::Runner::default().start(|context| async move {
            let mut c = Comments::init(context, "comments").await;
            let r0 = c.root();
            // stage a raw thread record directly (apply arms are stubs in Task 6).
            c.pending.insert(b"t:probe".to_vec(), Some(b"{}".to_vec()));
            c.commit_block().await.unwrap();
            assert_ne!(c.root(), r0, "a committed write must move the root");
        });
    }
}
