//! per-repo MULTI-BRANCH state — the flag-day generalization of the phase-1
//! single-`main` [`RepoState`].
//!
//! consensus state per repo is now a sorted map of born branches
//! (`short_name -> head oid`); `main` is the protected default (never deleted,
//! fast-forward-guarded at materialize time), every other branch is a plain
//! CAS-guarded ref that may force-push or be deleted — the GitHub flow
//! (`git push origin feature/x`, open a PR from it).
//!
//! the phase-1 determinism invariant carries over PER BRANCH: consensus only
//! ever gates on a compare-and-swap against a branch's COMMITTED head; packs,
//! closures, and descent checks stay node-local catch-up, never accept/reject.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use git2::{Oid, Repository};
use sdk::Error;

use crate::git;
use crate::tracker_iface::MAX_BRANCH_BYTES;

/// the protected default branch's SHORT name.
pub const MAIN_BRANCH: &str = "main";

/// a branch short name to its full refname.
pub fn full_ref(short: &str) -> String {
    format!("{}{short}", git::HEADS_PREFIX)
}

/// validate a branch SHORT name deterministically (a consensus gate): 1..=128
/// bytes of `[a-zA-Z0-9._/-]`, `/`-separated into non-empty segments, no
/// segment starting with `.` or `-`, no segment equal to `.`/`..`, no `.lock`
/// suffix. strict enough to be a safe refname and an unambiguous map key.
pub fn norm_branch(name: &str) -> Result<(), Error> {
    if name.is_empty() || name.len() > MAX_BRANCH_BYTES {
        return Err(Error::Module(format!(
            "forge: branch name must be 1..={MAX_BRANCH_BYTES} bytes"
        )));
    }
    if !name
        .bytes()
        .all(|b| matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'.' | b'_' | b'-' | b'/'))
    {
        return Err(Error::Module(format!(
            "forge: branch name {name:?} must match [a-zA-Z0-9._/-]"
        )));
    }
    for seg in name.split('/') {
        if seg.is_empty() {
            return Err(Error::Module(format!(
                "forge: branch name {name:?} has an empty path segment"
            )));
        }
        if seg.starts_with('.') || seg.starts_with('-') {
            return Err(Error::Module(format!(
                "forge: branch segment {seg:?} may not start with '.' or '-'"
            )));
        }
        if seg.ends_with(".lock") {
            return Err(Error::Module(format!(
                "forge: branch segment {seg:?} may not end with '.lock'"
            )));
        }
    }
    Ok(())
}

/// one branch's staged (this-block) fate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StagedRef {
    /// objects already live in this repo's odb (the `Commit` path) — the ref
    /// moves directly at `commit_block`.
    Local(Oid),
    /// objects ride a node-local pack (the push/merge path): the committed
    /// head publishes unconditionally, the on-disk ref catches up via
    /// `materialize`.
    Packed(Oid, [u8; 32]),
    /// the branch is deleted (object-free — the ref just unbinds).
    Delete,
}

/// one repo's state: the committed branch map (feeds `root()`) plus node-local
/// staging / catch-up scaffolding.
#[derive(Default)]
pub struct RepoState {
    /// write-through mirror of the COMMITTED born branches — `short_name ->
    /// head`. sorted (`BTreeMap`) so the root preimage composes
    /// order-independently. the ONLY field that feeds `root()`.
    pub refs: BTreeMap<String, Oid>,
    /// branches staged this block. published by `commit_block`, dropped by
    /// `abort_block`. NOT in `root()` until committed.
    pub staged: BTreeMap<String, StagedRef>,
    /// node-local catch-up targets: committed Push/Merge heads whose objects
    /// are not yet installed on the on-disk ref, per branch.
    pub pending: BTreeMap<String, (Oid, [u8; 32])>,
    /// one-shot warn guard per branch (reset when its target changes/clears).
    warned: BTreeSet<String>,
}

impl RepoState {
    /// a fresh state over an adopted/installed committed branch map.
    pub fn with_refs(refs: BTreeMap<String, Oid>) -> Self {
        Self {
            refs,
            ..Default::default()
        }
    }

    /// read-your-writes head of one branch: a staged fate shadows the
    /// committed one.
    pub fn effective_head(&self, branch: &str) -> Option<Oid> {
        match self.staged.get(branch) {
            Some(StagedRef::Local(oid) | StagedRef::Packed(oid, _)) => Some(*oid),
            Some(StagedRef::Delete) => None,
            None => self.refs.get(branch).copied(),
        }
    }

    /// stage one CAS-guarded branch update — the SOLE consensus gate of the
    /// push path. `prev` must equal the branch's COMMITTED head (`None` ==
    /// unborn); `new: None` deletes. one staged fate per branch per block: a
    /// second update to the same branch in one block is rejected
    /// deterministically (one submit == one block, so this can only be an
    /// in-block conflict, e.g. a merge racing a push in one atomic op chain).
    pub fn stage_update(
        &mut self,
        branch: &str,
        prev: Option<Oid>,
        new: Option<Oid>,
        digest: Option<[u8; 32]>,
    ) -> Result<(), Error> {
        if self.staged.contains_key(branch) {
            return Err(Error::Module(format!(
                "forge: branch {branch:?} already has a staged update this block"
            )));
        }
        if self.refs.get(branch).copied() != prev {
            return Err(Error::Module(
                "non-fast-forward: forge HEAD moved; fetch and retry".into(),
            ));
        }
        let fate = match new {
            None => {
                if branch == MAIN_BRANCH {
                    return Err(Error::Module(
                        "forge: the main branch cannot be deleted".into(),
                    ));
                }
                if prev.is_none() {
                    return Err(Error::Module(format!(
                        "forge: cannot delete unborn branch {branch:?}"
                    )));
                }
                StagedRef::Delete
            }
            Some(oid) => match digest {
                Some(d) => StagedRef::Packed(oid, d),
                None => StagedRef::Local(oid),
            },
        };
        self.staged.insert(branch.to_string(), fate);
        Ok(())
    }

    /// publish every staged branch (the `commit_block` half): move/unbind
    /// on-disk refs for Local/Delete fates, publish Packed heads to the
    /// committed map + record their materialization targets, then attempt the
    /// node-local catch-up opportunistically.
    pub fn publish(
        &mut self,
        base: &Path,
        name: &str,
        blobs: &blobstore::BlobHandle,
    ) -> Result<(), Error> {
        let staged = std::mem::take(&mut self.staged);
        for (branch, fate) in staged {
            match fate {
                StagedRef::Local(oid) => {
                    let repo = open_or_init_repo(base, name)?;
                    git::update_ref(&repo, &full_ref(&branch), oid)
                        .map_err(|e| Error::Module(e.to_string()))?;
                    self.refs.insert(branch, oid);
                }
                StagedRef::Packed(oid, digest) => {
                    self.refs.insert(branch.clone(), oid);
                    self.pending.insert(branch.clone(), (oid, digest));
                    self.warned.remove(&branch);
                }
                StagedRef::Delete => {
                    let repo = open_or_init_repo(base, name)?;
                    git::delete_ref(&repo, &full_ref(&branch))
                        .map_err(|e| Error::Module(e.to_string()))?;
                    self.refs.remove(&branch);
                    self.pending.remove(&branch);
                    self.warned.remove(&branch);
                }
            }
        }
        self.materialize(base, name, blobs)
    }

    /// drop every staged fate — no ref moved, `root()` unchanged.
    pub fn abort(&mut self) {
        self.staged.clear();
    }

    /// node-local catch-up for THIS repo: per pending branch, fetch the pack
    /// by digest, install + verify the FULL closure, then move the on-disk
    /// ref. `main` additionally requires a fast-forward (merges satisfy it —
    /// the merge commit's first parent is the old main); other branches may
    /// force-push, so their ref moves unconditionally once the closure
    /// verifies. absent/corrupt packs are SAFE no-ops (root already reflects
    /// the committed head) that warn once. NEVER touches `refs`/`root()`.
    pub fn materialize(
        &mut self,
        base: &Path,
        name: &str,
        blobs: &blobstore::BlobHandle,
    ) -> Result<(), Error> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let repo = open_or_init_repo(base, name)?;
        let mut done = Vec::new();
        for (branch, (head, digest)) in &self.pending {
            let refname = full_ref(branch);
            let prior =
                git::resolve_ref(&repo, &refname).map_err(|e| Error::Module(e.to_string()))?;
            if prior == Some(*head) {
                done.push(branch.clone());
                continue;
            }
            let Some(pack) = blobs.get_chunk(digest) else {
                if self.warned.insert(branch.clone()) {
                    eprintln!(
                        "[forge] materialize: pack {} for repo {name} branch {branch} head {head} \
                         not in the blob store yet; on-disk ref stays behind, root already \
                         reflects the committed head",
                        crate::hex(digest)
                    );
                }
                continue;
            };
            if let Err(why) = install_and_advance(
                &repo,
                &refname,
                *head,
                prior,
                &pack,
                branch == MAIN_BRANCH,
            ) {
                if self.warned.insert(branch.clone()) {
                    eprintln!(
                        "[forge] materialize: cannot advance repo {name} branch {branch} to head \
                         {head}: {why}; leaving ref behind (root already correct)"
                    );
                }
                continue;
            }
            done.push(branch.clone());
        }
        for branch in done {
            self.pending.remove(&branch);
            self.warned.remove(&branch);
        }
        Ok(())
    }
}

/// the pure git side of one branch's materialize attempt: install the pack,
/// require the full closure of `head`, optionally require fast-forward (main
/// only), then move the ref. any failure is returned so the caller can turn
/// it into a safe no-op.
fn install_and_advance(
    repo: &Repository,
    refname: &str,
    head: Oid,
    prior: Option<Oid>,
    pack: &[u8],
    require_ff: bool,
) -> Result<(), Error> {
    git::install_pack(repo, pack).map_err(|e| Error::Module(e.to_string()))?;
    git::verify_closure(repo, head).map_err(|e| Error::Module(e.to_string()))?;
    if require_ff && let Some(prior) = prior {
        let ff =
            git::is_descendant(repo, head, prior).map_err(|e| Error::Module(e.to_string()))?;
        if !ff {
            return Err(Error::Module(format!(
                "head does not fast-forward on-disk ref {prior}"
            )));
        }
    }
    git::update_ref(repo, refname, head).map_err(|e| Error::Module(e.to_string()))?;
    Ok(())
}

/// open the per-repo libgit2 repository at `base/<name>`, initializing a fresh
/// sha1 repo there if the dir has no `.git` yet. node-local: the dir path is
/// not consensus state, only the committed branch oids it yields are.
pub fn open_or_init_repo(base: &Path, name: &str) -> Result<Repository, Error> {
    let dir = base.join(name);
    let repo = if dir.join(".git").exists() {
        git::open(&dir)
    } else {
        git::init(&dir)
    };
    repo.map_err(|e| Error::Module(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_names_validate_deterministically() {
        for ok in ["main", "feature/x", "a.b_c-1", "Feature/UPPER", "v1.2.3"] {
            assert!(norm_branch(ok).is_ok(), "{ok:?} must be accepted");
        }
        for bad in [
            "", "/x", "x/", "a//b", "-x", ".x", "a/.hidden", "a/-b", "x.lock", "a/b.lock",
            "a b", "a:b", "a~b", "한글",
        ] {
            assert!(norm_branch(bad).is_err(), "{bad:?} must be rejected");
        }
        assert!(norm_branch(&"a".repeat(129)).is_err());
        assert!(norm_branch(&"a".repeat(128)).is_ok());
    }

    #[test]
    fn stage_update_cas_and_protection_rules() {
        let mut st = RepoState::default();
        let a = Oid::from_str("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        let b = Oid::from_str("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").unwrap();

        // unborn branch: prev must be None.
        assert!(st.stage_update("feat", Some(a), Some(b), None).is_err());
        st.stage_update("feat", None, Some(a), None).unwrap();
        // double-stage in one block is rejected.
        assert!(st.stage_update("feat", None, Some(b), None).is_err());

        // committed CAS.
        st.refs.insert("main".into(), a);
        assert!(st.stage_update("main", Some(b), Some(a), None).is_err());
        st.stage_update("main", Some(a), Some(b), None).unwrap();

        // main cannot be deleted; unborn branches cannot be deleted.
        let mut st2 = RepoState::default();
        st2.refs.insert("main".into(), a);
        st2.refs.insert("feat".into(), a);
        assert!(st2.stage_update("main", Some(a), None, None).is_err());
        assert!(st2.stage_update("ghost", None, None, None).is_err());
        st2.stage_update("feat", Some(a), None, None).unwrap();
        assert_eq!(st2.effective_head("feat"), None, "staged delete shadows");
        assert_eq!(st2.effective_head("main"), Some(a));
    }
}
