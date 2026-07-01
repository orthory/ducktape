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

use std::path::Path;

use git2::{Commit, ErrorCode, Oid, Repository, RepositoryInitOptions, Signature, Time, Tree};

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
    let t = Time::new(consensus_time as i64, 0);
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
