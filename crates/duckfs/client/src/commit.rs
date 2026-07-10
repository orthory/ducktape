//! the commit engine: plan the working copy, stage only the chunks the cluster
//! lacks (probed, deduped), submit ONE atomic commit against the recorded base,
//! resolve the new snapshot, and rewrite the index.
//!
//! staging is sequential (one chunk = one block — ingest speed is consensus
//! speed) and probed twice: once to skip chunks already present (dedup + resume),
//! and again immediately before submit as TTL insurance. a `"files: chunk not
//! available"` rejection (a chunk expired between probe and submit) re-stages and
//! retries the commit once. a CAS conflict auto-rebases disjoint upstream work or
//! surfaces a structured [`ConflictReport`] — never a silent merge.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use duckfs_core::{Change, MAX_PAGE, MAX_SYNC_IDS, SnapshotInfo, to_hex};

use crate::api::{ApiError, CommitReceipt, ConflictReport, NodeApi};
use crate::chunk::{chunk_ids, file_object_id};
use crate::index::{EntryKind, Index, IndexEntry, IndexError};
use crate::plan::{Plan, PlanError, plan};
use crate::scan::{ScanKind, disk_path, scan};
use crate::status::{Status, status};

/// the conflict strings the engine keys on — verbatim from the module (`fs.rs`),
/// arriving through the http 400 envelope untouched.
const CONFLICT_PREFIX: &str = "files: conflict:";
const BASE_NOT_RESOLVABLE: &str = "files: base snapshot not resolvable";

/// bound the auto-rebase: after this many disjoint rebases the head is clearly
/// churning under us, so stop and report rather than spin.
const MAX_REBASE_ATTEMPTS: usize = 3;

/// what a successful commit produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitSummary {
    pub snapshot: String,
    pub height: u64,
    /// whether the engine auto-rebased onto a newer head before this commit landed.
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
    /// an overlapping (or unrebasable) conflict — no silent merge. carries the
    /// structured report the CLI/RPC surface. boxed to keep the common `Ok` /
    /// small-`Err` `Result` cheap (clippy `result_large_err`).
    #[error("duckfs: commit conflict ({} clashing path(s)); {}", .0.clashing.len(), .0.remedy)]
    Conflict(Box<ConflictReport>),
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

/// commit knobs. `auto_rebase` (the CLI default) rebases disjoint upstream work
/// before reporting a conflict; the CLI's `--no-rebase` turns it off, so the
/// FIRST CAS conflict surfaces as a report instead of silently rebasing.
#[derive(Debug, Clone)]
pub struct CommitOptions {
    pub auto_rebase: bool,
}

impl Default for CommitOptions {
    fn default() -> Self {
        CommitOptions { auto_rebase: true }
    }
}

/// commit the working copy at `dir` with `message`, auto-rebasing disjoint
/// upstream work. see [`commit_with`] to disable the rebase.
pub fn commit(api: &dyn NodeApi, dir: &Path, message: &str) -> Result<CommitSummary, CommitError> {
    commit_with(api, dir, message, &CommitOptions::default())
}

/// [`commit`] with explicit options. see the module doc for the stage/probe/
/// submit sequence.
pub fn commit_with(
    api: &dyn NodeApi,
    dir: &Path,
    message: &str,
    opts: &CommitOptions,
) -> Result<CommitSummary, CommitError> {
    let index = Index::load(dir)?;
    let st = status(dir).map_err(|e| CommitError::Io(e.to_string()))?;
    if st.clean {
        return Err(CommitError::Nothing);
    }
    let planned = plan(&st, dir, &index.prefix)?;

    // stage the chunks the cluster lacks, then re-probe as TTL insurance.
    ensure_staged(api, &planned.blobs)?;
    ensure_staged(api, &planned.blobs)?;

    let (receipt, rebased) = submit_with_rebase(api, &index, message, &planned, opts.auto_rebase)?;

    let snapshot = resolve_snapshot(
        api,
        receipt.height,
        &change_paths(&planned.changes),
        &index.prefix,
    )?;
    rebuild_index(&index, &st, dir, &snapshot)?;
    Ok(CommitSummary {
        snapshot,
        height: receipt.height,
        rebased,
    })
}

/// submit the commit, handling CAS conflicts: on `"files: conflict:"` refetch the
/// head, diff base→head, and auto-rebase (resubmit with `base = head`) ONLY when
/// the upstream change set is disjoint from ours — never a silent merge. an
/// overlapping conflict, an exhausted rebase budget, or a diff that itself rejects
/// (oversized/unresolvable) all fail safe with a structured [`ConflictReport`]. a
/// GC'd base (`"files: base snapshot not resolvable"`) reports a re-checkout
/// remedy without attempting a rebase.
fn submit_with_rebase(
    api: &dyn NodeApi,
    index: &Index,
    message: &str,
    planned: &Plan,
    auto_rebase: bool,
) -> Result<(CommitReceipt, bool), CommitError> {
    let ours = change_paths(&planned.changes);
    let mut base = index.base_snapshot.clone();
    let mut rebased = false;

    for _ in 0..=MAX_REBASE_ATTEMPTS {
        match submit(api, base.as_deref(), message, planned) {
            Ok(receipt) => return Ok((receipt, rebased)),
            Err(CommitError::Rejected(m)) if m.contains(BASE_NOT_RESOLVABLE) => {
                // the base fell out of the 1024-window: no rebase can recover it,
                // the client must re-checkout onto the current head.
                return Err(CommitError::Conflict(Box::new(ConflictReport {
                    base: index.base_snapshot.clone(),
                    head: api.refs().ok().and_then(|r| r.head),
                    ours: sorted(&ours),
                    theirs: Vec::new(),
                    clashing: Vec::new(),
                    remedy: "the base snapshot has been garbage-collected out of the \
                             history window; re-checkout to rebase onto the current head"
                        .into(),
                })));
            }
            Err(CommitError::Rejected(m)) if m.contains(CONFLICT_PREFIX) => {
                let head = api.refs()?.head;
                // without both a base to diff FROM and a head to diff TO, there is
                // nothing to rebase against — a genuine conflict.
                let (Some(base_id), Some(head_id)) = (index.base_snapshot.clone(), head.clone())
                else {
                    return Err(overlap_report(&index.base_snapshot, head, &ours, &ours));
                };
                // a diff that itself rejects (oversized/unresolvable) → fail safe.
                let theirs: BTreeSet<String> = match api.diff(&base_id, &head_id, &index.prefix) {
                    Ok(entries) => entries.into_iter().map(|e| e.path).collect(),
                    Err(_) => return Err(overlap_report(&index.base_snapshot, head, &ours, &ours)),
                };
                let clashing: BTreeSet<String> = ours.intersection(&theirs).cloned().collect();
                if clashing.is_empty() {
                    if auto_rebase {
                        // disjoint upstream work: rebase onto the new head and retry.
                        base = Some(head_id);
                        rebased = true;
                        continue;
                    }
                    // `--no-rebase`: a disjoint conflict the caller declined to
                    // auto-rebase — report it (no clashing paths, but the head
                    // moved) rather than silently rebase.
                    return Err(CommitError::Conflict(Box::new(ConflictReport {
                        base: index.base_snapshot.clone(),
                        head,
                        ours: sorted(&ours),
                        theirs: sorted(&theirs),
                        clashing: Vec::new(),
                        remedy: "upstream advanced with disjoint changes; re-run \
                                 without --no-rebase to auto-rebase, or re-checkout"
                            .into(),
                    })));
                }
                return Err(CommitError::Conflict(Box::new(ConflictReport {
                    base: index.base_snapshot.clone(),
                    head,
                    ours: sorted(&ours),
                    theirs: sorted(&theirs),
                    clashing: sorted(&clashing),
                    remedy: "overlapping edits on the same path(s); re-checkout, \
                             reapply your changes, and commit again"
                        .into(),
                })));
            }
            Err(other) => return Err(other),
        }
    }

    // the rebase budget is exhausted: head kept moving under us.
    Err(CommitError::Conflict(Box::new(ConflictReport {
        base: index.base_snapshot.clone(),
        head: api.refs().ok().and_then(|r| r.head),
        ours: sorted(&ours),
        theirs: Vec::new(),
        clashing: Vec::new(),
        remedy: "the head kept advancing across repeated rebases; re-checkout and \
                 commit again"
            .into(),
    })))
}

/// build an overlap conflict report (used when there is no base/head to diff, or
/// the diff itself failed — fail safe: treat every touched path as clashing).
fn overlap_report(
    base: &Option<String>,
    head: Option<String>,
    ours: &BTreeSet<String>,
    clashing: &BTreeSet<String>,
) -> CommitError {
    CommitError::Conflict(Box::new(ConflictReport {
        base: base.clone(),
        head,
        ours: sorted(ours),
        theirs: Vec::new(),
        clashing: sorted(clashing),
        remedy: "a concurrent change touches your path(s) and could not be \
                 auto-rebased; re-checkout, reapply, and commit again"
            .into(),
    }))
}

/// the set of paths a commit's changes touch — the "ours" side of a conflict.
fn change_paths(changes: &[Change]) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    for change in changes {
        match change {
            Change::Put { path, .. }
            | Change::Mkdir { path }
            | Change::Rm { path }
            | Change::Symlink { path, .. } => {
                set.insert(path.clone());
            }
            Change::Mv { from, to } => {
                set.insert(from.clone());
                set.insert(to.clone());
            }
        }
    }
    set
}

fn sorted(set: &BTreeSet<String>) -> Vec<String> {
    set.iter().cloned().collect()
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

/// resolve the snapshot id THIS commit produced. height alone is not a unique key
/// once the node aggregates multiple member ops into one block (finding #3): N
/// file commits then share one height, and the newest-first history entry at that
/// height is NOT necessarily ours. so: a single entry at the height is
/// unambiguous (the common case — a 1-op-1-block lane, or a single committer in a
/// batch); multiple entries are disambiguated by matching each candidate's
/// INTRODUCED changes (its diff from its own parent) against the paths we
/// committed (`ours`) — concurrent applied commits touch DISJOINT paths, so
/// exactly one candidate's diff intersects ours. if that is inconclusive we fail
/// safe (a clear error, never a silently-wrong base that would corrupt the index).
fn resolve_snapshot(
    api: &dyn NodeApi,
    height: u64,
    ours: &BTreeSet<String>,
    prefix: &str,
) -> Result<String, CommitError> {
    let history = api.history(MAX_PAGE)?;
    let at_height: Vec<&SnapshotInfo> = history.iter().filter(|s| s.height == height).collect();
    match at_height.as_slice() {
        // nothing at that height yet (the window may have advanced past it): the
        // head is the best the client can name.
        [] => api
            .refs()?
            .head
            .ok_or_else(|| CommitError::Transport("commit landed but head is empty".into())),
        // exactly one commit at this height — unambiguous. the common case: a
        // 1-op-1-block lane, or a single committer in an aggregated batch.
        [only] => Ok(only.id.clone()),
        // batch aggregation landed several commits at ONE height, so height alone
        // is ambiguous. `ours` is the paths THIS commit changed; concurrent applied
        // commits touch DISJOINT paths, so exactly one candidate's introduced diff
        // (from its own parent) intersects ours — that one is ours.
        candidates => {
            let mut found: Option<String> = None;
            for cand in candidates {
                let from = cand.parent.clone().unwrap_or_default();
                // a candidate we cannot diff (e.g. a parentless first commit the
                // node won't diff) simply cannot be matched — skip it rather than
                // fail the whole resolution on someone else's snapshot.
                let touched: BTreeSet<String> = match api.diff(&from, &cand.id, prefix) {
                    Ok(entries) => entries.into_iter().map(|e| e.path).collect(),
                    Err(_) => continue,
                };
                if ours.is_disjoint(&touched) {
                    continue;
                }
                if found.is_some() {
                    // two candidates intersect ours — cannot safely disambiguate.
                    found = None;
                    break;
                }
                found = Some(cand.id.clone());
            }
            // fail SAFE: never silently record a wrong base (which would make the
            // next status/commit treat a peer's concurrent files as deletions).
            found.ok_or_else(|| {
                CommitError::Transport(format!(
                    "commit landed at height {height} but its snapshot is ambiguous among \
                     {} same-height commits and could not be matched to the committed paths; \
                     re-checkout to resync the base",
                    candidates.len()
                ))
            })
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::CommitReceipt;
    use duckfs_core::{DiffEntry, DiffKind, EntryInfo, RefsInfo, SnapshotInfo};

    /// a node whose block AGGREGATED two commits into ONE height (finding #3):
    /// snap_a (parent H0, introduced /shared/x) then snap_b (parent snap_a,
    /// introduced /shared/y). history is newest-first, so snap_b is the naive
    /// first-by-height match — wrong for the /shared/x committer.
    struct AggregatedNode;
    const H0: &str = "00";
    const SNAP_A: &str = "aa";
    const SNAP_B: &str = "bb";

    impl NodeApi for AggregatedNode {
        fn history(&self, _limit: u64) -> Result<Vec<SnapshotInfo>, ApiError> {
            let mk = |id: &str, parent: &str, msg: &str| SnapshotInfo {
                id: id.into(),
                parent: Some(parent.into()),
                root_tree: String::new(),
                author: String::new(),
                height: 5,
                consensus_time: 0,
                message: msg.into(),
            };
            Ok(vec![mk(SNAP_B, SNAP_A, "b"), mk(SNAP_A, H0, "a")])
        }
        fn diff(&self, from: &str, to: &str, _prefix: &str) -> Result<Vec<DiffEntry>, ApiError> {
            let path = match (from, to) {
                (H0, SNAP_A) => "/shared/x",
                (SNAP_A, SNAP_B) => "/shared/y",
                _ => return Err(ApiError::Transport("no such diff".into())),
            };
            Ok(vec![DiffEntry {
                path: path.into(),
                kind: DiffKind::Modified,
            }])
        }
        fn refs(&self) -> Result<RefsInfo, ApiError> {
            Ok(RefsInfo {
                head: Some(SNAP_B.into()),
                pins: BTreeMap::new(),
                window_len: 2,
            })
        }
        fn stat(&self, _: &str, _: Option<&str>) -> Result<Option<EntryInfo>, ApiError> {
            unimplemented!()
        }
        fn ls(
            &self,
            _: &str,
            _: Option<&str>,
            _: Option<&str>,
            _: u64,
        ) -> Result<(Vec<EntryInfo>, Option<String>), ApiError> {
            unimplemented!()
        }
        fn find(
            &self,
            _: &str,
            _: Option<&str>,
            _: Option<&str>,
            _: u64,
        ) -> Result<(Vec<EntryInfo>, Option<String>), ApiError> {
            unimplemented!()
        }
        fn read(
            &self,
            _: &str,
            _: Option<&str>,
            _: u64,
            _: u64,
        ) -> Result<(Vec<u8>, bool), ApiError> {
            unimplemented!()
        }
        fn has_chunks(&self, _: &[String]) -> Result<Vec<bool>, ApiError> {
            unimplemented!()
        }
        fn stage_chunk(&self, _: &[u8]) -> Result<String, ApiError> {
            unimplemented!()
        }
        fn commit(
            &self,
            _: Option<&str>,
            _: &str,
            _: Vec<Change>,
        ) -> Result<CommitReceipt, ApiError> {
            unimplemented!()
        }
        fn pin(&self, _: &str, _: &str) -> Result<(), ApiError> {
            unimplemented!()
        }
    }

    fn paths(list: &[&str]) -> BTreeSet<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn resolve_disambiguates_same_height_commits_by_changed_paths() {
        let api = AggregatedNode;
        // the FIRST committer (changed /shared/x => snap_a) must resolve snap_a,
        // NOT the newest-first snap_b that height alone would pick.
        let got = resolve_snapshot(&api, 5, &paths(&["/shared/x"]), "/shared").unwrap();
        assert_eq!(got, SNAP_A, "resolve the commit whose diff matches our paths");
        // the SECOND committer (changed /shared/y => snap_b) resolves snap_b.
        let got = resolve_snapshot(&api, 5, &paths(&["/shared/y"]), "/shared").unwrap();
        assert_eq!(got, SNAP_B);
    }
}
