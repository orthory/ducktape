//! a module-backed `NodeApi`: a real native `files::Files` on a tempdir, driven
//! exactly like `crates/apps/files/tests/commit.rs` — `execute` + `commit_block`
//! per op, `block_on` at the top level, one block per write. it stands in for a
//! live node so the engine's logic (checkout, planning, staging, conflict) is
//! tested against real module semantics without a daemon. `stage_chunk`/`commit`
//! call counters back the dedup/resume assertions.
#![allow(dead_code)]

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::future::Future;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use duckfs_client::api::{ApiError, CommitReceipt, NodeApi};
use duckfs_core::{
    Change, DiffEntry, DigestHex, EntryInfo, FilesMsg, FilesQuery, FilesReply, RefsInfo,
    SnapshotInfo, decode_reply, encode_msg, encode_putblob, encode_query,
};
use files::Files;
use sdk::{Ctx, Effect, Env, Error, Event, Module as _, Msg, Origin, StateRoot};

// ---- a deterministic Ctx (copied from the files harness) --------------------

pub struct TestCtx {
    env: Env,
    emitted: VecDeque<Msg>,
}

impl TestCtx {
    fn new(origin: Origin, height: u64) -> Self {
        Self {
            env: Env {
                protocol_version: 0,
                height,
                consensus_time: height,
                origin,
                me: "files".into(),
            },
            emitted: VecDeque::new(),
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Ctx for TestCtx {
    fn env(&self) -> &Env {
        &self.env
    }
    fn module_root(&self, _target: &str) -> Option<StateRoot> {
        None
    }
    async fn query(&self, _target: &str, _req: &[u8]) -> Result<Vec<u8>, Error> {
        Err(Error::QueryUnsupported)
    }
    fn emit_msg(&mut self, msg: Msg) {
        self.emitted.push_back(msg);
    }
    fn emit_event(&mut self, _event: Event) {}
    fn request_effect(&mut self, _effect: Effect) {}
}

// ---- the mock node ----------------------------------------------------------

pub struct ModuleNode {
    files: RefCell<Files>,
    _dir: tempfile::TempDir,
    height: Cell<u64>,
    pub stage_calls: Cell<usize>,
    pub commit_calls: Cell<usize>,
}

impl Default for ModuleNode {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleNode {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let files = Files::open("files", dir.path().to_path_buf()).expect("open files");
        ModuleNode {
            files: RefCell::new(files),
            _dir: dir,
            height: Cell::new(0),
            stage_calls: Cell::new(0),
            commit_calls: Cell::new(0),
        }
    }

    fn block_on<F: Future>(f: F) -> F::Output {
        futures::executor::block_on(f)
    }

    fn next_height(&self) -> u64 {
        let h = self.height.get() + 1;
        self.height.set(h);
        h
    }

    /// execute one op at a fresh height then adopt the block — one block per op,
    /// like production. returns the height the op committed in.
    fn exec(&self, payload: Vec<u8>) -> Result<u64, ApiError> {
        let h = self.next_height();
        let mut f = self.files.borrow_mut();
        let mut ctx = TestCtx::new(Origin::System, h);
        Self::block_on(f.execute(
            &mut ctx,
            &Msg {
                target: "files".into(),
                payload,
            },
        ))
        .map_err(map_err)?;
        Self::block_on(f.commit_block()).map_err(map_err)?;
        Ok(h)
    }

    fn run_query(&self, q: &FilesQuery) -> Result<FilesReply, ApiError> {
        let f = self.files.borrow();
        let bytes = Self::block_on(f.query(&encode_query(q))).map_err(map_err)?;
        decode_reply(&bytes).map_err(ApiError::Transport)
    }

    // ---- seeding helpers (do NOT touch the NodeApi call counters) ----

    /// seed a commit directly (test fixture path).
    pub fn seed_commit(
        &self,
        base: Option<&str>,
        message: &str,
        changes: Vec<Change>,
    ) -> Result<u64, ApiError> {
        self.exec(encode_msg(&FilesMsg::Commit {
            base_snapshot: base.map(Into::into),
            message: message.into(),
            changes,
        }))
    }

    /// seed a staged chunk directly (test fixture path).
    pub fn seed_stage(&self, bytes: &[u8]) -> Result<DigestHex, ApiError> {
        self.exec(encode_putblob(bytes))?;
        Ok(duckfs_core::to_hex(&duckfs_core::objects::object_id(
            duckfs_core::Kind::Chunk,
            bytes,
        )))
    }

    pub fn head(&self) -> Option<String> {
        self.refs().expect("refs").head
    }

    /// shrink the module's bounded history window so a GC'd-base conflict can be
    /// driven with a few commits (the `#[doc(hidden)]` module test seam).
    pub fn set_history_window(&self, n: usize) {
        self.files.borrow_mut().set_history_window_for_tests(n);
    }
}

fn map_err(e: Error) -> ApiError {
    match e {
        // a module rejection passes through verbatim — the conflict taxonomy
        // depends on the exact string.
        Error::Module(m) => ApiError::Rejected(m),
        other => ApiError::Transport(format!("{other:?}")),
    }
}

fn unexpected(reply: FilesReply) -> ApiError {
    ApiError::Transport(format!("unexpected files reply: {reply:?}"))
}

impl NodeApi for ModuleNode {
    fn refs(&self) -> Result<RefsInfo, ApiError> {
        match self.run_query(&FilesQuery::Refs {})? {
            FilesReply::Refs(info) => Ok(info),
            other => Err(unexpected(other)),
        }
    }

    fn stat(&self, path: &str, snapshot: Option<&str>) -> Result<Option<EntryInfo>, ApiError> {
        match self.run_query(&FilesQuery::Stat {
            path: path.into(),
            snapshot: snapshot.map(Into::into),
        })? {
            FilesReply::Stat(e) => Ok(e),
            other => Err(unexpected(other)),
        }
    }

    fn ls(
        &self,
        path: &str,
        snapshot: Option<&str>,
        after: Option<&str>,
        limit: u64,
    ) -> Result<(Vec<EntryInfo>, Option<String>), ApiError> {
        match self.run_query(&FilesQuery::Ls {
            path: path.into(),
            snapshot: snapshot.map(Into::into),
            after: after.map(Into::into),
            limit,
        })? {
            FilesReply::Ls { entries, next } => Ok((entries, next)),
            other => Err(unexpected(other)),
        }
    }

    fn find(
        &self,
        prefix: &str,
        snapshot: Option<&str>,
        after: Option<&str>,
        limit: u64,
    ) -> Result<(Vec<EntryInfo>, Option<String>), ApiError> {
        match self.run_query(&FilesQuery::Find {
            prefix: prefix.into(),
            snapshot: snapshot.map(Into::into),
            after: after.map(Into::into),
            limit,
        })? {
            FilesReply::Find { entries, next } => Ok((entries, next)),
            other => Err(unexpected(other)),
        }
    }

    fn read(
        &self,
        path: &str,
        snapshot: Option<&str>,
        offset: u64,
        len: u64,
    ) -> Result<(Vec<u8>, bool), ApiError> {
        match self.run_query(&FilesQuery::Read {
            path: path.into(),
            snapshot: snapshot.map(Into::into),
            offset,
            len,
        })? {
            FilesReply::Read { b64, eof } => {
                let bytes = STANDARD
                    .decode(b64.as_bytes())
                    .map_err(|e| ApiError::Transport(e.to_string()))?;
                Ok((bytes, eof))
            }
            other => Err(unexpected(other)),
        }
    }

    fn history(&self, limit: u64) -> Result<Vec<SnapshotInfo>, ApiError> {
        match self.run_query(&FilesQuery::History { limit })? {
            FilesReply::History(snaps) => Ok(snaps),
            other => Err(unexpected(other)),
        }
    }

    fn diff(&self, from: &str, to: &str, prefix: &str) -> Result<Vec<DiffEntry>, ApiError> {
        match self.run_query(&FilesQuery::Diff {
            from: from.into(),
            to: to.into(),
            prefix: prefix.into(),
        })? {
            FilesReply::Diff(entries) => Ok(entries),
            other => Err(unexpected(other)),
        }
    }

    fn has_chunks(&self, ids: &[String]) -> Result<Vec<bool>, ApiError> {
        match self.run_query(&FilesQuery::HasChunks { ids: ids.to_vec() })? {
            FilesReply::HasChunks { present } => Ok(present),
            other => Err(unexpected(other)),
        }
    }

    fn stage_chunk(&self, bytes: &[u8]) -> Result<DigestHex, ApiError> {
        self.stage_calls.set(self.stage_calls.get() + 1);
        self.exec(encode_putblob(bytes))?;
        Ok(duckfs_core::to_hex(&duckfs_core::objects::object_id(
            duckfs_core::Kind::Chunk,
            bytes,
        )))
    }

    fn commit(
        &self,
        base: Option<&str>,
        message: &str,
        changes: Vec<Change>,
    ) -> Result<CommitReceipt, ApiError> {
        self.commit_calls.set(self.commit_calls.get() + 1);
        let height = self.exec(encode_msg(&FilesMsg::Commit {
            base_snapshot: base.map(Into::into),
            message: message.into(),
            changes,
        }))?;
        Ok(CommitReceipt { height })
    }

    fn pin(&self, snapshot: &str, name: &str) -> Result<(), ApiError> {
        self.exec(encode_msg(&FilesMsg::Pin {
            snapshot: snapshot.into(),
            name: name.into(),
        }))?;
        Ok(())
    }
}
