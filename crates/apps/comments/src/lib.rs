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

    /// apply one decoded msg with the derived author/time.
    async fn apply(
        &mut self,
        msg: CommentMsg,
        author: AuthorRef,
        now: u64,
    ) -> Result<(), CommentError> {
        match msg {
            CommentMsg::AddComment {
                thread_id,
                comment_id,
                anchor,
                text,
            } => {
                if thread_id.is_empty()
                    || comment_id.is_empty()
                    || thread_id.starts_with('\u{0}')
                    || comment_id.starts_with('\u{0}')
                {
                    return Err(CommentError::ReservedId);
                }
                if text.len() > MAX_COMMENT_TEXT_BYTES {
                    return Err(CommentError::TextTooLarge);
                }
                if self.load_comment(&comment_id).await?.is_some() {
                    return Err(CommentError::DuplicateComment);
                }
                match self.load_thread(&thread_id).await? {
                    Some(mut thread) => {
                        if thread.anchor != anchor {
                            return Err(CommentError::AnchorMismatch);
                        }
                        if thread.comment_ids.len() >= MAX_COMMENTS_PER_THREAD {
                            return Err(CommentError::TooManyComments);
                        }
                        let comment = Comment {
                            id: comment_id.clone(),
                            thread_id: thread_id.clone(),
                            author,
                            text,
                            created_at: now,
                            edited_at: None,
                            deleted: false,
                        };
                        thread.comment_ids.push(comment_id);
                        self.store_comment(&comment)?;
                        self.store_thread(&thread)
                    }
                    None => {
                        let mut ids = self.load_anchor_index(&anchor).await?;
                        if ids.len() >= MAX_THREADS_PER_ANCHOR {
                            return Err(CommentError::TooManyThreads);
                        }
                        let comment = Comment {
                            id: comment_id.clone(),
                            thread_id: thread_id.clone(),
                            author: author.clone(),
                            text,
                            created_at: now,
                            edited_at: None,
                            deleted: false,
                        };
                        let thread = Thread {
                            id: thread_id.clone(),
                            anchor: anchor.clone(),
                            opener: author,
                            created_at: now,
                            resolved: false,
                            resolved_by: None,
                            comment_ids: vec![comment_id],
                        };
                        if !ids.contains(&thread_id) {
                            ids.push(thread_id);
                            ids.sort();
                            self.stage_anchor_index(&anchor, &ids)?;
                        }
                        self.store_comment(&comment)?;
                        self.store_thread(&thread)
                    }
                }
            }
            CommentMsg::EditComment { comment_id, text } => {
                if text.len() > MAX_COMMENT_TEXT_BYTES {
                    return Err(CommentError::TextTooLarge);
                }
                let mut c = self
                    .load_comment(&comment_id)
                    .await?
                    .ok_or(CommentError::CommentNotFound)?;
                if c.deleted {
                    return Err(CommentError::CommentNotFound);
                }
                if c.author != author {
                    return Err(CommentError::NotAuthor);
                }
                c.text = text;
                c.edited_at = Some(now);
                self.store_comment(&c)
            }
            CommentMsg::DeleteComment { comment_id } => {
                let mut c = self
                    .load_comment(&comment_id)
                    .await?
                    .ok_or(CommentError::CommentNotFound)?;
                if c.deleted {
                    return Ok(()); // idempotent
                }
                if c.author != author {
                    return Err(CommentError::NotAuthor);
                }
                c.deleted = true;
                c.text = String::new();
                let thread_id = c.thread_id.clone();
                self.store_comment(&c)?;
                // if no live comments remain, remove the whole thread.
                let thread = self
                    .load_thread(&thread_id)
                    .await?
                    .ok_or(CommentError::Corrupt)?;
                let mut any_live = false;
                for cid in &thread.comment_ids {
                    let cc = self.load_comment(cid).await?.ok_or(CommentError::Corrupt)?;
                    if !cc.deleted {
                        any_live = true;
                        break;
                    }
                }
                if !any_live {
                    for cid in &thread.comment_ids {
                        self.delete_key(&comment_key(cid));
                    }
                    self.delete_key(&thread_key(&thread.id));
                    let mut ids = self.load_anchor_index(&thread.anchor).await?;
                    ids.retain(|t| t != &thread.id);
                    self.stage_anchor_index(&thread.anchor, &ids)?;
                }
                Ok(())
            }
            CommentMsg::ResolveThread { thread_id, resolved } => {
                let mut thread = self
                    .load_thread(&thread_id)
                    .await?
                    .ok_or(CommentError::ThreadNotFound)?;
                thread.resolved = resolved;
                thread.resolved_by = if resolved { Some(author) } else { None };
                self.store_thread(&thread)
            }
        }
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
        let err = |e: CommentError| Error::Module(e.to_string());
        match decode_query(req).map_err(Error::Module)? {
            CommentQuery::ThreadsForAnchors { module, targets } => {
                if targets.len() > MAX_QUERY_TARGETS {
                    return Err(err(CommentError::TooManyTargets));
                }
                let mut out = Vec::with_capacity(targets.len());
                for target in targets {
                    let anchor = Anchor {
                        module: module.clone(),
                        target: target.clone(),
                    };
                    let ids = self.load_anchor_index(&anchor).await.map_err(err)?;
                    let mut threads = Vec::new();
                    for tid in ids {
                        if let Some(view) = self.thread_view(&tid).await.map_err(err)? {
                            threads.push(view);
                        }
                    }
                    out.push(AnchorThreads { target, threads });
                }
                Ok(encode_reply(&CommentReply::Anchored(out)))
            }
            CommentQuery::Thread { thread_id } => {
                let view = self.thread_view(&thread_id).await.map_err(err)?;
                Ok(encode_reply(&CommentReply::Thread(view)))
            }
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
    use sdk::{Env, Origin};

    struct TestCtx {
        env: Env,
    }
    impl TestCtx {
        fn new(origin: Origin) -> Self {
            Self {
                env: Env {
                    protocol_version: 0,
                    height: 0,
                    consensus_time: 7,
                    origin,
                    me: "comments".into(),
                },
            }
        }
    }
    #[async_trait::async_trait(?Send)]
    impl Ctx for TestCtx {
        fn env(&self) -> &Env {
            &self.env
        }
        fn module_root(&self, _t: &str) -> Option<StateRoot> {
            None
        }
        async fn query(&self, _t: &str, _r: &[u8]) -> Result<Vec<u8>, Error> {
            Err(Error::QueryUnsupported)
        }
        fn emit_msg(&mut self, _m: Msg) {}
        fn emit_event(&mut self, _e: sdk::Event) {}
        fn request_effect(&mut self, _e: sdk::Effect) {}
    }
    fn user(name: &str) -> Origin {
        Origin::External(name.as_bytes().to_vec())
    }
    fn wire(m: &CommentMsg) -> Msg {
        Msg {
            target: "comments".into(),
            payload: encode_msg(m),
        }
    }

    async fn apply_commit<E: Context + BufferPooler>(c: &mut Comments<E>, m: &CommentMsg, origin: Origin) {
        c.execute(&mut TestCtx::new(origin), &wire(m)).await.unwrap();
        c.commit_block().await.unwrap();
    }
    async fn apply_err<E: Context + BufferPooler>(
        c: &mut Comments<E>,
        m: &CommentMsg,
        origin: Origin,
        needle: &str,
    ) {
        let e = c
            .execute(&mut TestCtx::new(origin), &wire(m))
            .await
            .expect_err("must reject");
        assert!(
            matches!(e, Error::Module(ref s) if s.contains(needle)),
            "unexpected: {e:?}"
        );
        c.abort_block().await.unwrap();
    }
    async fn anchored<E: Context + BufferPooler>(
        c: &Comments<E>,
        module: &str,
        targets: &[&str],
    ) -> Vec<AnchorThreads> {
        let q = CommentQuery::ThreadsForAnchors {
            module: module.into(),
            targets: targets.iter().map(|s| s.to_string()).collect(),
        };
        match decode_reply(&c.query(&encode_query(&q)).await.unwrap()).unwrap() {
            CommentReply::Anchored(v) => v,
            _ => panic!("expected Anchored"),
        }
    }
    async fn thread_of<E: Context + BufferPooler>(c: &Comments<E>, thread_id: &str) -> Option<ThreadView> {
        match decode_reply(
            &c.query(&encode_query(&CommentQuery::Thread { thread_id: thread_id.into() }))
                .await
                .unwrap(),
        )
        .unwrap()
        {
            CommentReply::Thread(v) => v,
            _ => panic!("expected Thread"),
        }
    }
    fn anchor(target: &str) -> Anchor {
        Anchor { module: "pages".into(), target: target.into() }
    }

    #[test]
    fn a_staged_write_moves_the_root() {
        deterministic::Runner::default().start(|context| async move {
            let mut c = Comments::init(context, "comments").await;
            let r0 = c.root();
            c.pending.insert(b"t:probe".to_vec(), Some(b"{}".to_vec()));
            c.commit_block().await.unwrap();
            assert_ne!(c.root(), r0);
        });
    }

    #[test]
    fn add_opens_then_appends_and_batches_by_anchor() {
        deterministic::Runner::default().start(|context| async move {
            let mut c = Comments::init(context, "comments").await;
            apply_commit(&mut c, &CommentMsg::AddComment {
                thread_id: "t1".into(), comment_id: "m1".into(), anchor: anchor("b1"), text: "first".into(),
            }, user("alice")).await;
            apply_commit(&mut c, &CommentMsg::AddComment {
                thread_id: "t1".into(), comment_id: "m2".into(), anchor: anchor("b1"), text: "second".into(),
            }, user("bob")).await;
            apply_commit(&mut c, &CommentMsg::AddComment {
                thread_id: "t2".into(), comment_id: "m3".into(), anchor: anchor("b1"), text: "other".into(),
            }, user("alice")).await;
            apply_commit(&mut c, &CommentMsg::AddComment {
                thread_id: "t3".into(), comment_id: "m4".into(), anchor: anchor("b2"), text: "elsewhere".into(),
            }, user("alice")).await;

            let groups = anchored(&c, "pages", &["b1", "b2"]).await;
            let b1 = groups.iter().find(|g| g.target == "b1").unwrap();
            assert_eq!(b1.threads.len(), 2);
            let t1 = b1.threads.iter().find(|v| v.thread.id == "t1").unwrap();
            assert_eq!(t1.comments.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(), ["first", "second"]);
            assert_eq!(t1.thread.opener, AuthorRef::User(b"alice".to_vec()));
            assert_eq!(t1.comments[1].author, AuthorRef::User(b"bob".to_vec()));
            let b2 = groups.iter().find(|g| g.target == "b2").unwrap();
            assert_eq!(b2.threads.len(), 1);
        });
    }

    #[test]
    fn append_rejects_anchor_mismatch_and_duplicate_comment() {
        deterministic::Runner::default().start(|context| async move {
            let mut c = Comments::init(context, "comments").await;
            apply_commit(&mut c, &CommentMsg::AddComment {
                thread_id: "t1".into(), comment_id: "m1".into(), anchor: anchor("b1"), text: "x".into(),
            }, user("alice")).await;
            apply_err(&mut c, &CommentMsg::AddComment {
                thread_id: "t1".into(), comment_id: "m2".into(), anchor: anchor("b2"), text: "y".into(),
            }, user("alice"), "anchor mismatch").await;
            apply_err(&mut c, &CommentMsg::AddComment {
                thread_id: "t1".into(), comment_id: "m1".into(), anchor: anchor("b1"), text: "z".into(),
            }, user("alice"), "duplicate comment id").await;
            apply_err(&mut c, &CommentMsg::AddComment {
                thread_id: "t9".into(), comment_id: "m9".into(), anchor: anchor("b1"), text: "z".into(),
            }, Origin::External(vec![]), "empty origin").await;
        });
    }

    #[test]
    fn edit_and_delete_are_author_only() {
        deterministic::Runner::default().start(|context| async move {
            let mut c = Comments::init(context, "comments").await;
            apply_commit(&mut c, &CommentMsg::AddComment {
                thread_id: "t1".into(), comment_id: "m1".into(), anchor: anchor("b1"), text: "orig".into(),
            }, user("alice")).await;
            apply_err(&mut c, &CommentMsg::EditComment { comment_id: "m1".into(), text: "hacked".into() }, user("bob"), "not the comment author").await;
            apply_err(&mut c, &CommentMsg::DeleteComment { comment_id: "m1".into() }, user("bob"), "not the comment author").await;
            apply_commit(&mut c, &CommentMsg::EditComment { comment_id: "m1".into(), text: "edited".into() }, user("alice")).await;
            let v = thread_of(&c, "t1").await.unwrap();
            assert_eq!(v.comments[0].text, "edited");
            assert_eq!(v.comments[0].edited_at, Some(7));
        });
    }

    #[test]
    fn deleting_last_live_comment_removes_the_thread() {
        deterministic::Runner::default().start(|context| async move {
            let mut c = Comments::init(context, "comments").await;
            apply_commit(&mut c, &CommentMsg::AddComment {
                thread_id: "t1".into(), comment_id: "m1".into(), anchor: anchor("b1"), text: "a".into(),
            }, user("alice")).await;
            apply_commit(&mut c, &CommentMsg::AddComment {
                thread_id: "t1".into(), comment_id: "m2".into(), anchor: anchor("b1"), text: "b".into(),
            }, user("alice")).await;
            apply_commit(&mut c, &CommentMsg::DeleteComment { comment_id: "m1".into() }, user("alice")).await;
            let v = thread_of(&c, "t1").await.unwrap();
            assert_eq!(v.comments.iter().map(|x| x.text.as_str()).collect::<Vec<_>>(), ["b"]);
            apply_commit(&mut c, &CommentMsg::DeleteComment { comment_id: "m2".into() }, user("alice")).await;
            assert!(thread_of(&c, "t1").await.is_none());
            let groups = anchored(&c, "pages", &["b1"]).await;
            assert!(groups[0].threads.is_empty());
        });
    }

    #[test]
    fn resolve_toggles_and_records_resolver() {
        deterministic::Runner::default().start(|context| async move {
            let mut c = Comments::init(context, "comments").await;
            apply_commit(&mut c, &CommentMsg::AddComment {
                thread_id: "t1".into(), comment_id: "m1".into(), anchor: anchor("b1"), text: "a".into(),
            }, user("alice")).await;
            apply_commit(&mut c, &CommentMsg::ResolveThread { thread_id: "t1".into(), resolved: true }, user("bob")).await;
            let v = thread_of(&c, "t1").await.unwrap();
            assert!(v.thread.resolved);
            assert_eq!(v.thread.resolved_by, Some(AuthorRef::User(b"bob".to_vec())));
            apply_commit(&mut c, &CommentMsg::ResolveThread { thread_id: "t1".into(), resolved: false }, user("alice")).await;
            let v = thread_of(&c, "t1").await.unwrap();
            assert!(!v.thread.resolved);
            assert_eq!(v.thread.resolved_by, None);
            apply_err(&mut c, &CommentMsg::ResolveThread { thread_id: "ghost".into(), resolved: true }, user("alice"), "thread not found").await;
        });
    }
}
