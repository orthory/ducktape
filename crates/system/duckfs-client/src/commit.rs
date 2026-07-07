//! the commit engine: plan the working copy, stage only the chunks the cluster
//! lacks (probed, deduped), submit ONE atomic commit against the recorded base,
//! resolve the new snapshot, and rewrite the index.
//!
//! staging is sequential (one chunk = one block — ingest speed is consensus
//! speed) and probed twice: once to skip chunks already present (dedup + resume),
//! and again immediately before submit as TTL insurance. a `"files: chunk not
//! available"` rejection (a chunk expired between probe and submit) re-stages and
//! retries the commit once. conflict handling lands in a later slice.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use files::{MAX_PAGE, MAX_SYNC_IDS, to_hex};

use crate::api::{ApiError, CommitReceipt, NodeApi};
use crate::chunk::{chunk_ids, file_object_id};
use crate::index::{EntryKind, Index, IndexEntry, IndexError};
use crate::plan::{Plan, PlanError, plan};
use crate::scan::{ScanKind, disk_path, scan};
use crate::status::{Status, status};

/// what a successful commit produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitSummary {
    pub snapshot: String,
    pub height: u64,
    /// whether the engine auto-rebased onto a newer head (a later slice sets this).
    pub rebased: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum CommitError {
    #[error(transparent)]
    Plan(#[from] PlanError),
    #[error(transparent)]
    Index(#[from] IndexError),
    #[error("duckfs: nothing to commit (the working copy is clean)")]
    Nothing,
    /// a module rejection (the verbatim `"files: ..."` string).
    #[error("{0}")]
    Rejected(String),
    #[error("duckfs: commit transport: {0}")]
    Transport(String),
    #[error("duckfs: commit io: {0}")]
    Io(String),
}

impl From<ApiError> for CommitError {
    fn from(e: ApiError) -> Self {
        match e {
            ApiError::Rejected(m) => CommitError::Rejected(m),
            ApiError::NotFound => CommitError::Transport("not found".into()),
            ApiError::Transport(m) => CommitError::Transport(m),
        }
    }
}

/// commit the working copy at `dir` with `message`. see the module doc for the
/// stage/probe/submit sequence.
pub fn commit(api: &dyn NodeApi, dir: &Path, message: &str) -> Result<CommitSummary, CommitError> {
    let index = Index::load(dir)?;
    let st = status(dir).map_err(|e| CommitError::Io(e.to_string()))?;
    if st.clean {
        return Err(CommitError::Nothing);
    }
    let planned = plan(&st, dir, &index.prefix)?;

    // stage the chunks the cluster lacks, then re-probe as TTL insurance.
    ensure_staged(api, &planned.blobs)?;
    ensure_staged(api, &planned.blobs)?;

    let base = index.base_snapshot.clone();
    let receipt = submit(api, base.as_deref(), message, &planned)?;

    let snapshot = resolve_snapshot(api, receipt.height)?;
    rebuild_index(&index, &st, dir, &snapshot)?;
    Ok(CommitSummary {
        snapshot,
        height: receipt.height,
        rebased: false,
    })
}

/// probe every chunk digest in ≤256-id batches and stage any the cluster lacks,
/// sequentially (one block per stage). already-present chunks are skipped — this
/// is the dedup + resume path.
fn ensure_staged(api: &dyn NodeApi, blobs: &BTreeMap<String, Vec<u8>>) -> Result<(), CommitError> {
    let digests: Vec<String> = blobs.keys().cloned().collect();
    for batch in digests.chunks(MAX_SYNC_IDS) {
        let present = api.has_chunks(batch)?;
        for (digest, present) in batch.iter().zip(present) {
            if !present {
                api.stage_chunk(&blobs[digest])?;
            }
        }
    }
    Ok(())
}

/// submit the one atomic commit. a `"files: chunk not available"` rejection means
/// a staged chunk expired between the probe and this submit — re-stage the whole
/// set and retry exactly once.
fn submit(
    api: &dyn NodeApi,
    base: Option<&str>,
    message: &str,
    planned: &Plan,
) -> Result<CommitReceipt, CommitError> {
    match api.commit(base, message, planned.changes.clone()) {
        Ok(receipt) => Ok(receipt),
        Err(ApiError::Rejected(m)) if m.contains("files: chunk not available") => {
            for bytes in planned.blobs.values() {
                api.stage_chunk(bytes)?;
            }
            Ok(api.commit(base, message, planned.changes.clone())?)
        }
        Err(e) => Err(e.into()),
    }
}

/// resolve the snapshot id a commit produced: the history entry at the receipt's
/// height, falling back to the current head.
fn resolve_snapshot(api: &dyn NodeApi, height: u64) -> Result<String, CommitError> {
    let history = api.history(MAX_PAGE)?;
    if let Some(snap) = history.iter().find(|s| s.height == height) {
        return Ok(snap.id.clone());
    }
    api.refs()?
        .head
        .ok_or_else(|| CommitError::Transport("commit landed but head is empty".into()))
}

/// rewrite the index after a successful commit: the working copy IS the new base.
/// unchanged files keep their recorded object id (no re-hash of a big untouched
/// file); changed/new files and every symlink are recomputed; mtimes are
/// refreshed from a fresh scan, and the index is saved last so status reads clean.
fn rebuild_index(old: &Index, st: &Status, dir: &Path, snapshot: &str) -> Result<(), CommitError> {
    let scanned = scan(dir, &old.prefix).map_err(|e| CommitError::Io(e.to_string()))?;
    let changed: BTreeSet<&str> = st
        .added
        .iter()
        .chain(st.modified.iter())
        .map(|e| e.path.as_str())
        .collect();

    let mut index = Index::new(&old.prefix, old.node.clone(), Some(snapshot.to_string()));
    for entry in &scanned {
        match entry.kind {
            ScanKind::File => {
                // unchanged file → reuse the recorded object + meta (its bytes did
                // not move); otherwise recompute with empty meta (the plan commits
                // client edits without meta).
                let reuse = (!changed.contains(entry.path.as_str()))
                    .then(|| old.entries.get(&entry.path))
                    .flatten();
                let (object, meta) = match reuse {
                    Some(e) => (e.object.clone(), e.meta.clone()),
                    None => (hash_file(dir, &old.prefix, &entry.path)?, BTreeMap::new()),
                };
                index.entries.insert(
                    entry.path.clone(),
                    IndexEntry {
                        object,
                        size: entry.size,
                        mtime_secs: entry.mtime_secs,
                        mtime_nanos: entry.mtime_nanos,
                        exec: entry.exec,
                        kind: EntryKind::File,
                        meta,
                    },
                );
            }
            ScanKind::Symlink => {
                let target = entry.target.clone().unwrap_or_default();
                let object = to_hex(&file_object_id(
                    target.len() as u64,
                    &chunk_ids(target.as_bytes()),
                    &BTreeMap::new(),
                ));
                index.entries.insert(
                    entry.path.clone(),
                    IndexEntry {
                        object,
                        size: entry.size,
                        mtime_secs: entry.mtime_secs,
                        mtime_nanos: entry.mtime_nanos,
                        exec: false,
                        kind: EntryKind::Symlink,
                        meta: BTreeMap::new(),
                    },
                );
            }
            ScanKind::Dir => {
                if entry.empty_dir {
                    index.entries.insert(
                        entry.path.clone(),
                        IndexEntry {
                            object: String::new(),
                            size: 0,
                            mtime_secs: entry.mtime_secs,
                            mtime_nanos: entry.mtime_nanos,
                            exec: false,
                            kind: EntryKind::Dir,
                            meta: BTreeMap::new(),
                        },
                    );
                }
            }
        }
    }
    index.save(dir)?;
    Ok(())
}

/// recompute a file's object id (empty meta) by reading it from disk.
fn hash_file(dir: &Path, prefix: &str, path: &str) -> Result<String, CommitError> {
    let disk = disk_path(dir, prefix, path);
    let bytes = std::fs::read(&disk).map_err(|e| CommitError::Io(e.to_string()))?;
    Ok(to_hex(&file_object_id(
        bytes.len() as u64,
        &chunk_ids(&bytes),
        &BTreeMap::new(),
    )))
}
