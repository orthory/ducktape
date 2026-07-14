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

use git2::{Buf, Commit, ErrorCode, Oid, Repository, RepositoryInitOptions, Signature, Time, Tree};

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

/// pack only the objects reachable from `heads` but from NONE of `bases` —
/// the fetch lane's INCREMENTAL pack. hidden commits mark their trees
/// uninteresting, so unchanged trees/blobs never re-cross the wire. every
/// `bases` oid must be a commit present in this repo (the caller filters the
/// client's haves down to what the repo knows). same determinism posture as
/// [`pack_closure_many`]: single-threaded, revwalk-driven, deduped.
pub fn pack_delta(repo: &Repository, heads: &[Oid], bases: &[Oid]) -> Result<Vec<u8>, git2::Error> {
    let mut pb = repo.packbuilder()?;
    pb.set_threads(1);
    let mut walk = repo.revwalk()?;
    for head in heads {
        walk.push(*head)?;
    }
    for base in bases {
        walk.hide(*base)?;
    }
    pb.insert_walk(&mut walk)?;
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
