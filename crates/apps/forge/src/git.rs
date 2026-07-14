//! a thin git2 seam — forge's private substrate via VENDORED libgit2.
//!
//! forge needs only a handful of plumbing ops (init/adopt a repo, read HEAD,
//! hash a blob, build a tree, write a deterministic commit object, move a ref).
//! each verb here is a typed wrapper over `git2` operating on a `&Repository`
//! the caller opens per-call. there is NO `std::process::Command` — libgit2 is
//! vendored INTO the binary, so a node runs with no host `git` installed.
//!
//! repos are git's DEFAULT sha1 object format (sha256 needs experimental
//! libgit2 and can't interop with the git ecosystem). a 20-byte sha1 oid is the
//! sha256 preimage forge rehashes into its 32-byte root (see `lib.rs`).

use std::{
    cmp::Ordering,
    collections::BTreeSet,
    path::Path,
};

use git2::{
    Buf, Commit, DiffFormat, DiffOptions, ErrorCode, ObjectType, Oid, Repository,
    RepositoryInitOptions, Signature, Time, Tree,
};

/// the fixed author/committer identity — pinning it makes the commit oid
/// reproducible across nodes (no host `user.name`/`user.email` leak).
const IDENT_NAME: &str = "ducktape";
const IDENT_EMAIL: &str = "ducktape@localhost";

/// init a fresh sha1 repo at `dir`: hermetic (`external_template(false)`, no
/// host template dir) and pinning the canonical branch name so init does not
/// depend on the host's `init.defaultBranch`. non-bare, so `.git` exists.
pub fn init(dir: &Path) -> Result<Repository, git2::Error> {
    std::fs::create_dir_all(dir)
        .map_err(|e| git2::Error::from_str(&format!("create repo dir: {e}")))?;
    let mut opts = RepositoryInitOptions::new();
    opts.initial_head("main").external_template(false);
    Repository::init_opts(dir, &opts)
}

/// adopt an existing repo (a `.git` left by a prior run).
pub fn open(dir: &Path) -> Result<Repository, git2::Error> {
    Repository::open(dir)
}

/// resolve a ref to its oid, or `None` if it doesn't exist yet (unborn HEAD).
pub fn resolve_ref(repo: &Repository, name: &str) -> Result<Option<Oid>, git2::Error> {
    match repo.refname_to_id(name) {
        Ok(oid) => Ok(Some(oid)),
        Err(e) if e.code() == ErrorCode::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// build a tree from `base` (a parent commit's tree, if any) with a single flat
/// `path -> blob` entry inserted, returning the written tree oid. pure by
/// construction: an in-memory `TreeBuilder`, no on-disk index, no worktree — so
/// no host cruft can leak into the tree bytes.
///
/// NB: `path` must be a single flat segment — libgit2's `TreeBuilder` rejects
/// `/` (it doesn't synthesize intermediate subtrees). every forge caller is
/// flat today. TODO: nested paths (recursive subtree build) for `dir/file`.
pub fn build_tree(
    repo: &Repository,
    base: Option<&Tree>,
    path: &str,
    blob: Oid,
) -> Result<Oid, git2::Error> {
    let mut tb = repo.treebuilder(base)?;
    tb.insert(path, blob, 0o100644)?; // 0o100644 == FileMode::Blob (regular file)
    tb.write()
}

/// write a deterministic commit object over `tree`, chained on `parent` if
/// present, WITHOUT moving any ref: `update_ref = None` is the staging seam —
/// the object lands in the odb but no ref points at it until `commit_block`.
///
/// determinism: a FIXED identity + a `consensus_time`-derived `Time` (offset
/// +0000), set for BOTH author and committer, are the only two timestamps in a
/// commit — pinning both makes the sha1 oid byte-identical across nodes on the
/// same inputs. libgit2's `commit` never gpg-signs (that's a separate call).
pub fn commit(
    repo: &Repository,
    tree: &Tree,
    parent: Option<&Commit>,
    message: &str,
    consensus_time: u64,
) -> Result<Oid, git2::Error> {
    // a `consensus_time` past i64::MAX cannot be represented as a git Time —
    // an `as` cast would silently wrap negative, minting a deterministic-but-
    // corrupt commit date. reject instead: the same guard rejects on every
    // validator (consensus_time is agreed), so this stays consensus-safe.
    let secs = i64::try_from(consensus_time).map_err(|_| {
        git2::Error::from_str("consensus_time exceeds the representable git commit time")
    })?;
    let t = Time::new(secs, 0);
    let sig = Signature::new(IDENT_NAME, IDENT_EMAIL, &t)?;
    let parents: Vec<&Commit> = parent.into_iter().collect();
    repo.commit(None, &sig, &sig, message, tree, &parents)
}

/// move a ref to `target`, create-or-force-update — the update-ref primitive.
/// single-node: the LOCAL ref move at the commit origin. (faithful multi-node
/// applies this same primitive on receipt of a wire RefUpdate, never a commit —
/// see the module docstring in `lib.rs`.)
pub fn update_ref(repo: &Repository, name: &str, target: Oid) -> Result<(), git2::Error> {
    repo.reference(name, target, true, "forge: commit_block")?;
    Ok(())
}

/// the raw width of a sha1 oid — the snapshot's head-oid header size.
pub const OID_RAW_LEN: usize = 20;

/// the ref namespace forge manages — every branch lives under it and the wire
/// carries SHORT names ("main", "feature/x"); this prefix is a local detail.
pub const HEADS_PREFIX: &str = "refs/heads/";

/// every born branch as `(short_name, oid)`, sorted by name (glob iteration is
/// alphabetical in libgit2, but sort explicitly — the caller composes state
/// from this). the multi-ref analogue of `resolve_ref(MAIN_REF)` for restart
/// re-adopt.
pub fn list_branches(repo: &Repository) -> Result<Vec<(String, Oid)>, git2::Error> {
    let mut out = Vec::new();
    for r in repo.references_glob(&format!("{HEADS_PREFIX}*"))? {
        let r = r?;
        let (Some(name), Some(oid)) = (r.name(), r.target()) else {
            continue; // symbolic or non-utf8 ref — not one forge writes
        };
        let Some(short) = name.strip_prefix(HEADS_PREFIX) else {
            continue;
        };
        out.push((short.to_string(), oid));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// pack the FULL object closure reachable from EVERY head into one
/// self-contained packfile — the multi-ref snapshot pack. same determinism
/// posture as [`pack_closure`]: single-threaded, revwalk-inserted, deduped.
pub fn pack_closure_many(repo: &Repository, heads: &[Oid]) -> Result<Vec<u8>, git2::Error> {
    let mut pb = repo.packbuilder()?;
    pb.set_threads(1);
    let mut walk = repo.revwalk()?;
    for head in heads {
        walk.push(*head)?;
    }
    for oid in walk {
        pb.insert_commit(oid?)?;
    }
    let mut buf = Buf::new();
    pb.write_buf(&mut buf)?;
    Ok(buf.to_vec())
}

/// stream a packfile into the odb and commit it. libgit2's indexer re-hashes
/// every object and checks the pack trailer as it indexes, so tampered or
/// malformed bytes fail HERE — before anything could be referenced, with no
/// ref moved. (a failed pack may strand temp/loose junk in the odb: node-
/// local, never part of any root.)
pub fn install_pack(repo: &Repository, pack: &[u8]) -> Result<(), git2::Error> {
    let odb = repo.odb()?;
    let mut pw = odb.packwriter()?;
    std::io::Write::write_all(&mut pw, pack)
        .map_err(|e| git2::Error::from_str(&format!("write pack: {e}")))?;
    pw.commit()?;
    Ok(())
}

/// delete a ref if it exists (idempotent) — installing the empty state onto a
/// repo whose ref was already born must unbind it, or the module's root could
/// never return to ZERO.
pub fn delete_ref(repo: &Repository, name: &str) -> Result<(), git2::Error> {
    match repo.find_reference(name) {
        Ok(mut r) => r.delete(),
        Err(e) if e.code() == ErrorCode::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// whether `head` is a descendant of (or equal to) `ancestor` — a real
/// fast-forward. node-local materialization uses this to refuse moving the
/// on-disk ref onto a head that does NOT build on the prior ref. `head ==
/// ancestor` counts as a descendant so re-materializing an already-current ref
/// is idempotent. purely LOCAL: never a consensus gate (a validator without the
/// pack can't run it, and root must not depend on it).
pub fn is_descendant(repo: &Repository, head: Oid, ancestor: Oid) -> Result<bool, git2::Error> {
    if head == ancestor {
        return Ok(true);
    }
    repo.graph_descendant_of(head, ancestor)
}

/// verify the FULL object closure reachable from `head` is present in the odb.
/// pack indexing hash-verifies each object it CARRIES but says nothing about
/// connectivity — a byzantine pack can ship a genuine head commit and omit the
/// blobs/trees/parents it references. walk every commit from `head` and every
/// tree entry beneath each, requiring each object to exist (a missing parent
/// commit surfaces as a revwalk read error; a missing tree fails `find_tree`;
/// a missing blob fails the odb existence check). submodule gitlinks are
/// skipped: they name commits in ANOTHER repo's odb by design.
pub fn verify_closure(repo: &Repository, head: Oid) -> Result<(), git2::Error> {
    let odb = repo.odb()?;
    let mut walk = repo.revwalk()?;
    walk.push(head)?;
    let mut seen_trees = std::collections::BTreeSet::new();
    for oid in walk {
        let commit = repo.find_commit(oid?)?;
        let mut stack = vec![commit.tree_id()];
        while let Some(tree_id) = stack.pop() {
            if !seen_trees.insert(tree_id) {
                continue;
            }
            let tree = repo.find_tree(tree_id)?;
            for entry in tree.iter() {
                match entry.kind() {
                    Some(git2::ObjectType::Tree) => stack.push(entry.id()),
                    Some(git2::ObjectType::Commit) => {}
                    _ => {
                        if !odb.exists(entry.id()) {
                            return Err(git2::Error::from_str(&format!(
                                "closure incomplete: missing object {}",
                                entry.id()
                            )));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// A pull-request diff that is unsafe to materialize on the synchronous query
/// path.
#[derive(Debug)]
pub enum BoundedDiffError {
    Git(git2::Error),
    TooLarge {
        files_changed: usize,
        blob_bytes: usize,
        max_files: usize,
        max_blob_bytes: usize,
    },
}

impl std::fmt::Display for BoundedDiffError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Git(e) => e.fmt(f),
            Self::TooLarge {
                files_changed,
                blob_bytes,
                max_files,
                max_blob_bytes,
            } => write!(
                f,
                "diff is too large: {files_changed} changed files / {blob_bytes} materialized blob bytes (limits: {max_files} files / {max_blob_bytes} bytes)"
            ),
        }
    }
}

impl std::error::Error for BoundedDiffError {}

impl From<git2::Error> for BoundedDiffError {
    fn from(value: git2::Error) -> Self {
        Self::Git(value)
    }
}

#[derive(Clone, Copy)]
struct TreeEntryMeta {
    oid: Oid,
    kind: ObjectType,
    mode: i32,
}

struct DiffPreflight {
    paths: BTreeSet<String>,
    blob_bytes: usize,
    max_files: usize,
    max_blob_bytes: usize,
}

impl DiffPreflight {
    fn too_large(&self) -> bool {
        self.paths.len() > self.max_files || self.blob_bytes > self.max_blob_bytes
    }

    fn error(&self) -> BoundedDiffError {
        BoundedDiffError::TooLarge {
            files_changed: self.paths.len(),
            blob_bytes: self.blob_bytes,
            max_files: self.max_files,
            max_blob_bytes: self.max_blob_bytes,
        }
    }

    fn add_blob_bytes(
        &mut self,
        repo: &Repository,
        entry: TreeEntryMeta,
    ) -> Result<(), git2::Error> {
        match entry.kind {
            ObjectType::Blob => {
                let (size, kind) = repo.odb()?.read_header(entry.oid)?;
                if kind != ObjectType::Blob {
                    return Err(git2::Error::from_str(
                        "tree entry expected a blob but its object has another type",
                    ));
                }
                self.blob_bytes = self
                    .blob_bytes
                    .saturating_add(size)
                    .min(self.max_blob_bytes.saturating_add(1));
            }
            // Gitlinks commonly name commits absent from the superproject's
            // object database. They carry no materialized blob bytes.
            ObjectType::Commit => {}
            ObjectType::Tree => {
                return Err(git2::Error::from_str(
                    "internal error: tree counted as a changed leaf",
                ));
            }
            _ => return Err(git2::Error::from_str("unsupported git tree entry type")),
        }
        Ok(())
    }

    fn add_leaf(
        &mut self,
        repo: &Repository,
        path: String,
        old: Option<TreeEntryMeta>,
        new: Option<TreeEntryMeta>,
    ) -> Result<(), git2::Error> {
        self.paths.insert(path);
        for entry in old.into_iter().chain(new) {
            self.add_blob_bytes(repo, entry)?;
        }
        Ok(())
    }
}

fn next_tree_entry<'repo>(
    iter: &mut impl Iterator<Item = git2::TreeEntry<'repo>>,
) -> Result<Option<(String, TreeEntryMeta)>, git2::Error> {
    let Some(entry) = iter.next() else {
        return Ok(None);
    };
    let name = entry
        .name()
        .ok_or_else(|| git2::Error::from_str("diff path is not valid UTF-8"))?
        .to_owned();
    let kind = entry
        .kind()
        .ok_or_else(|| git2::Error::from_str("git tree entry has an invalid mode"))?;
    if !matches!(kind, ObjectType::Blob | ObjectType::Tree | ObjectType::Commit) {
        return Err(git2::Error::from_str("unsupported git tree entry type"));
    }
    Ok(Some((
        name,
        TreeEntryMeta {
            oid: entry.id(),
            kind,
            mode: entry.filemode(),
        },
    )))
}

// Git tree entries are ordered as though tree names end in `/`. Matching that
// order lets the preflight merge two arbitrarily wide trees without first
// allocating either directory's full entry set.
fn tree_entry_cmp(
    old_name: &str,
    old_kind: ObjectType,
    new_name: &str,
    new_kind: ObjectType,
) -> Ordering {
    if old_name == new_name {
        return Ordering::Equal;
    }
    let old = old_name.as_bytes();
    let new = new_name.as_bytes();
    let common = old.len().min(new.len());
    match old[..common].cmp(&new[..common]) {
        Ordering::Equal => {
            let old_next = old
                .get(common)
                .copied()
                .unwrap_or(if old_kind == ObjectType::Tree { b'/' } else { 0 });
            let new_next = new
                .get(common)
                .copied()
                .unwrap_or(if new_kind == ObjectType::Tree { b'/' } else { 0 });
            old_next.cmp(&new_next)
        }
        order => order,
    }
}

fn join_path(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_owned()
    } else {
        format!("{prefix}/{name}")
    }
}

fn collect_tree_leaves(
    repo: &Repository,
    tree_oid: Oid,
    prefix: &str,
    old_side: bool,
    preflight: &mut DiffPreflight,
) -> Result<(), BoundedDiffError> {
    let tree = repo.find_tree(tree_oid)?;
    let mut entries = tree.iter();
    while let Some((name, entry)) = next_tree_entry(&mut entries)? {
        if preflight.too_large() {
            return Err(preflight.error());
        }
        let path = join_path(prefix, &name);
        if entry.kind == ObjectType::Tree {
            collect_tree_leaves(repo, entry.oid, &path, old_side, preflight)?;
        } else if old_side {
            preflight.add_leaf(repo, path, Some(entry), None)?;
        } else {
            preflight.add_leaf(repo, path, None, Some(entry))?;
        }
    }
    if preflight.too_large() {
        return Err(preflight.error());
    }
    Ok(())
}

fn compare_trees(
    repo: &Repository,
    old_oid: Oid,
    new_oid: Oid,
    prefix: &str,
    preflight: &mut DiffPreflight,
) -> Result<(), BoundedDiffError> {
    if old_oid == new_oid {
        return Ok(());
    }
    let old_tree = repo.find_tree(old_oid)?;
    let new_tree = repo.find_tree(new_oid)?;
    let mut old_iter = old_tree.iter();
    let mut new_iter = new_tree.iter();
    let mut old_entry = next_tree_entry(&mut old_iter)?;
    let mut new_entry = next_tree_entry(&mut new_iter)?;

    while old_entry.is_some() || new_entry.is_some() {
        if preflight.too_large() {
            return Err(preflight.error());
        }
        let ordering = match (&old_entry, &new_entry) {
            (Some((old_name, old)), Some((new_name, new))) => {
                tree_entry_cmp(old_name, old.kind, new_name, new.kind)
            }
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => break,
        };
        let (name, old_current, new_current) = match ordering {
            Ordering::Less => {
                let (name, entry) = old_entry.take().expect("ordering requires old entry");
                old_entry = next_tree_entry(&mut old_iter)?;
                (name, Some(entry), None)
            }
            Ordering::Greater => {
                let (name, entry) = new_entry.take().expect("ordering requires new entry");
                new_entry = next_tree_entry(&mut new_iter)?;
                (name, None, Some(entry))
            }
            Ordering::Equal => {
                let (name, old) = old_entry.take().expect("ordering requires old entry");
                let (_, new) = new_entry.take().expect("ordering requires new entry");
                old_entry = next_tree_entry(&mut old_iter)?;
                new_entry = next_tree_entry(&mut new_iter)?;
                (name, Some(old), Some(new))
            }
        };
        let path = join_path(prefix, &name);
        if let (Some(old), Some(new)) = (old_current, new_current)
            && old.oid == new.oid
            && old.kind == new.kind
            && old.mode == new.mode
        {
            continue;
        }
        match (old_current, new_current) {
            (Some(old), Some(new))
                if old.kind == ObjectType::Tree && new.kind == ObjectType::Tree =>
            {
                compare_trees(repo, old.oid, new.oid, &path, preflight)?;
            }
            (Some(old), Some(new)) if old.kind == ObjectType::Tree => {
                collect_tree_leaves(repo, old.oid, &path, true, preflight)?;
                preflight.add_leaf(repo, path, None, Some(new))?;
            }
            (Some(old), Some(new)) if new.kind == ObjectType::Tree => {
                preflight.add_leaf(repo, path.clone(), Some(old), None)?;
                collect_tree_leaves(repo, new.oid, &path, false, preflight)?;
            }
            (Some(old), Some(new)) => {
                preflight.add_leaf(repo, path, Some(old), Some(new))?;
            }
            (Some(old), None) if old.kind == ObjectType::Tree => {
                collect_tree_leaves(repo, old.oid, &path, true, preflight)?;
            }
            (None, Some(new)) if new.kind == ObjectType::Tree => {
                collect_tree_leaves(repo, new.oid, &path, false, preflight)?;
            }
            (Some(old), None) => preflight.add_leaf(repo, path, Some(old), None)?,
            (None, Some(new)) => preflight.add_leaf(repo, path, None, Some(new))?,
            (None, None) => unreachable!("name came from one of the trees"),
        }
    }
    if preflight.too_large() {
        return Err(preflight.error());
    }
    Ok(())
}

/// Compare two materialized commits and return a bounded unified-diff prefix
/// plus full statistics for a preflight-bounded diff. No fetch, rename
/// detection, or shell command is attempted.
pub fn bounded_diff(
    repo: &Repository,
    target: Oid,
    source: Oid,
    max_bytes: usize,
    max_files: usize,
    max_blob_bytes: usize,
) -> Result<(String, bool, usize, usize, usize), BoundedDiffError> {
    let target_tree = repo.find_commit(target)?.tree()?;
    let source_tree = repo.find_commit(source)?.tree()?;
    let mut preflight = DiffPreflight {
        paths: BTreeSet::new(),
        blob_bytes: 0,
        max_files,
        max_blob_bytes,
    };
    compare_trees(repo, target_tree.id(), source_tree.id(), "", &mut preflight)?;
    let mut opts = DiffOptions::new();
    opts.context_lines(3)
        .interhunk_lines(0)
        .disable_pathspec_match(true);
    for path in &preflight.paths {
        opts.pathspec(path);
    }
    let diff = repo.diff_tree_to_tree(
        Some(&target_tree),
        Some(&source_tree),
        Some(&mut opts),
    )?;
    let stats = diff.stats()?;
    let counts = (
        stats.files_changed(),
        stats.insertions(),
        stats.deletions(),
    );

    let mut bytes = Vec::with_capacity(max_bytes.min(8 * 1024));
    let mut truncated = false;
    let print_result = diff.print(DiffFormat::Patch, |_delta, _hunk, line| {
        let prefix = match line.origin() {
            'F' | 'H' | 'B' => None,
            origin => Some(origin as u8),
        };
        let needed = line.content().len() + usize::from(prefix.is_some());
        if bytes.len() + needed > max_bytes {
            let remaining = max_bytes.saturating_sub(bytes.len());
            if let Some(prefix) = prefix
                && remaining > 0
            {
                bytes.push(prefix);
            }
            let remaining = max_bytes.saturating_sub(bytes.len());
            bytes.extend_from_slice(&line.content()[..remaining.min(line.content().len())]);
            truncated = true;
            return false;
        }
        if let Some(prefix) = prefix {
            bytes.push(prefix);
        }
        bytes.extend_from_slice(line.content());
        true
    });
    match print_result {
        Ok(()) => {}
        Err(e) if truncated && e.code() == ErrorCode::User => {}
        Err(e) => return Err(e.into()),
    }
    let patch = match std::str::from_utf8(&bytes) {
        Ok(_) => String::from_utf8(bytes).expect("validated UTF-8"),
        Err(e) if truncated && e.error_len().is_none() => {
            bytes.truncate(e.valid_up_to());
            String::from_utf8(bytes).expect("truncated to a UTF-8 boundary")
        }
        Err(_) => {
            return Err(git2::Error::from_str("diff is not valid UTF-8 text").into());
        }
    };
    Ok((patch, truncated, counts.0, counts.1, counts.2))
}
