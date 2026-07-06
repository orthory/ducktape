//! ============================================================================
//! STORAGE SUBSTRATE — DECIDED DIRECTION (2026-07-01)
//!   git2-rs (vendored libgit2) + sha1;  root() = sha256 over per-repo HEAD oids
//! ============================================================================
//!
//! forge stores its state in real git repos via libgit2 (the `git2-rs` crate,
//! VENDORED — so the node binary is SELF-CONTAINED, no `git` install needed), in
//! git's DEFAULT sha1 object format. the reasoning, because it is a non-obvious
//! stack of trade-offs:
//!
//! WHY libgit2 (git2-rs, vendored) instead of shelling out to `git`:
//!   DEPLOYABILITY. shelling out makes every validator depend on a compatible
//!   `git` binary on the host (and sha256-mode needs git >= ~2.42). vendoring
//!   libgit2 INTO the node binary makes it SELF-CONTAINED — no `git` install.
//!
//! WHY sha1 (git's default), NOT sha256, even though sha256 is "stronger":
//!   ECOSYSTEM COMPATIBILITY. forge is a *git* feature — users expect to clone it
//!   with a stock `git`, push/pull, import existing repos, and mirror to hosting.
//!   but a sha256 repo can do NONE of that with the outside git world:
//!     - git's sha1<->sha256 interop layer was designed years ago and NEVER
//!       shipped, so a sha256 repo cannot exchange objects with ANY sha1 repo
//!       (no importing an existing repo, no pushing to a sha1 remote);
//!     - hosting (GitHub/GitLab/...) largely REJECTS sha256 repos (no mirroring);
//!     - libgit2's sha256 is still behind an EXPERIMENTAL build flag, API in flux,
//!       not battle-tested in any forge.
//!   sha1 keeps forge a normal, interoperable git repo. the hash weakness is
//!   bounded: modern git/libgit2 use collision-DETECTING sha1 (SHA-1DC), and git
//!   itself ran on sha1 for ~18 years.
//!
//! WHY root() rehashes the oids under sha256, not the oids verbatim:
//!   a [`StateRoot`] is 32 bytes; a sha1 oid is only 20. rehashing the 20-byte
//!   HEAD oids under sha256 makes forge's contribution to the global app-hash
//!   sha256-STRENGTH. the only residual sha1 surface is a *forge-object*
//!   collision (two trees under one commit oid) — expensive and SHA-1DC-guarded —
//!   while the app-hash's collision resistance at the STATE layer stays sha256.
//!   we trade "root IS a git oid verbatim" for real-world git interop, a good
//!   trade for a git PRODUCT. (no committed head anywhere -> StateRoot::ZERO.)
//!
//! WHAT DOES NOT CHANGE:
//!   - DETERMINISM: git2's typed `Signature` sets the FIXED `ducktape` identity +
//!     a date from `ctx.env().consensus_time` (never wall clock), so each repo's
//!     sha1 oid — and thus the composed root — is byte-identical across nodes on
//!     the same inputs.
//!   - the object format is a NETWORK-WIDE GENESIS CONSTANT: every validator MUST
//!     use the identical format. a sha1 node and a sha256 node compute different
//!     roots for the same state and FORK. it is NOT a per-node choice.
//!
//! ============================================================================
//!
//! forge — a GIT-backed feature module, a NAMED NAMESPACE of repos.
//!
//! where the directory module keeps a `BTreeMap` and kv keeps a qmdb, forge's
//! private substrate is a set of real on-disk git repositories — one libgit2
//! repo per named repo, at `base/<name>` — driven through VENDORED libgit2
//! (`git2-rs`), no `git` subprocess. each repo is git's DEFAULT sha1 object
//! format, so a HEAD oid is 20 bytes.
//!
//! ## the load-bearing composition invariant
//!
//! forge's authenticated [`StateRoot`] is a CANONICAL SORTED HASH over the
//! committed HEAD of every repo that HAS one:
//!
//! ```text
//! root = sha256(  for each (name, head) in repos sorted-by-name, head.is_some():
//!                     u32-LE(name.len()) ++ name.bytes ++ head.oid.bytes[20]  )
//! ```
//!
//! with `root() == StateRoot::ZERO` when no repo has a committed head (the
//! empty-genesis root, unchanged). this is a PURE FUNCTION of the committed head
//! oids — sorted, so order-independent, and identical on every validator
//! REGARDLESS of pack possession. that is the phase-1 `Push` determinism
//! invariant, now per-repo and composed: a repo's head advances on every
//! validator the instant a push CASes, whether or not that validator holds the
//! packfile; the objects catch up node-locally (see `materialize`) and NEVER
//! enter root/accept-reject. an unborn repo (a dir that exists but whose ref was
//! never born) does NOT contribute.
//!
//! ## back-compat: the default repo (no app change)
//!
//! [`ForgeMsg::Commit`]/[`ForgeMsg::Push`] carry a `#[serde(default)] repo`, so a
//! legacy wire message with no `repo` deserializes with `repo == ""`; the module
//! normalizes an empty repo to the well-known `"default"` repo. the unit
//! [`ForgeQuery::Head`] answers the default repo's head. an app that sends the
//! old `{Commit:{path,content,message}}` and queries `"Head"` keeps working with
//! ZERO change; [`ForgeQuery::HeadOf`]/[`ForgeQuery::ListRepos`] are additive.
//!
//! ## the determinism landmine (per repo)
//!
//! a git *commit* embeds committer identity + a timestamp, so two nodes
//! committing the same content would normally get DIFFERENT commit oids — and
//! the app-hash would fork. each repo keeps its commit reproducible: a FIXED
//! author/committer identity (`ducktape`, via a typed `git2::Signature`) and a
//! date derived from `ctx.env().consensus_time` (NOT wall clock, offset +0000),
//! set for BOTH author and committer, so the sha1 oid is byte-identical across
//! independent repos given the same inputs; the tree is built in-memory with a
//! `git2::TreeBuilder` seeded from the parent tree, a pure function of (parent,
//! change). no on-disk index, no worktree — nothing for host cruft to leak
//! through. git2 is used precisely because it BYPASSES the host-config traps
//! porcelain would inherit: `commit.gpgsign` never fires, `core.autocrlf` never
//! mangles blob bytes, and the fixed `Signature` overrides `user.*`.
//!
//! ## the host-lent staging seam (per repo)
//!
//! forge follows the host-lent STAGING pattern, now per repo. `execute` stages a
//! change on ONE repo's [`RepoState`] WITHOUT moving that repo's ref, so `root()`
//! (which reads the committed refs) is unchanged. `commit_block` publishes every
//! staged repo (moving each ref, or — for a `Push` — recording a node-local
//! materialization target and deferring the ref move to `materialize`);
//! `abort_block` drops every repo's staged write and the built objects linger
//! unreferenced in the odb (node-local, never in `root()`/the app-hash).

// the wire surface: this module's shared types, flattened at the crate root.
mod interface;
pub use interface::*;

mod git;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use git2::{Oid, Repository};
use sdk::{Ctx, Error, Module, ModuleId, Msg, StateRoot, StateSyncHandle};
use sha2::{Digest, Sha256};

/// the canonical branch every repo commits to and reads HEAD from.
const MAIN_REF: &str = "refs/heads/main";

/// the well-known repo an empty/absent `repo` field maps to — the target of the
/// legacy single-repo wire (see the module docstring).
const DEFAULT_REPO: &str = "default";

/// the max repo-name length in bytes (names are a filesystem path segment and a
/// consensus-visible key, so they are bounded).
const MAX_REPO_NAME_LEN: usize = 64;

/// normalize + validate a repo slug DETERMINISTICALLY (same input -> same
/// decision on every validator, so it is safe as a consensus gate): empty ->
/// `"default"`; otherwise it must be 1..=`MAX_REPO_NAME_LEN` bytes of
/// `[a-z0-9._-]` and never `.`/`..` (those would escape or collide with the base
/// dir as a path segment). a valid non-empty slug returns unchanged, so the map
/// key equals the on-disk directory name.
fn norm_repo(repo: &str) -> Result<String, Error> {
    if repo.is_empty() {
        return Ok(DEFAULT_REPO.to_string());
    }
    if repo.len() > MAX_REPO_NAME_LEN {
        return Err(Error::Module(format!(
            "forge: repo name too long ({} bytes, max {MAX_REPO_NAME_LEN})",
            repo.len()
        )));
    }
    if repo == "." || repo == ".." {
        return Err(Error::Module(
            "forge: repo name may not be '.' or '..'".into(),
        ));
    }
    if !repo
        .bytes()
        .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'-'))
    {
        return Err(Error::Module(format!(
            "forge: repo name {repo:?} must match [a-z0-9._-]"
        )));
    }
    Ok(repo.to_string())
}

// ============================================================================
// upgrade dual-path SEAM (inert) — the version-selected behavior branch.
// ============================================================================
//
// forge is a `root()`-changing module, so a no-downtime protocol upgrade that
// alters its root preimage / wire format ships as a DUAL-PATH binary: the same
// binary can reproduce the OLD behavior below the agreed activation height `H`
// and the NEW behavior at/after `H`, flipping deterministically at the boundary.
// this section wires the SEAM — the single place a version maps to a behavior
// branch — WITHOUT changing any behavior yet. the real divergence (a second
// layout) lands in a later phase; today every version selects the current
// multi-repo layout, so `root()` is byte-identical for every input and every
// accept/reject and snapshot byte is unchanged.
//
// the branch selector comes from two version signals, never hashed into any
// preimage: `Env::protocol_version` (the read-only per-block dispatch input,
// used inside `execute`) and the module's own cached `Forge::active_version`
// (the committed branch selector, used by `root`/`snapshot`/`install`/`query`,
// which have no `Ctx`). both default to the baseline below, so a fresh or
// existing forge behaves EXACTLY as before this seam landed.

/// the forge protocol baseline — the version whose behavior THIS binary
/// reproduces byte-for-byte today. `active_version` defaults here at genesis and
/// sdk's `Env::protocol_version` defaults to the same baseline (`0`), so the
/// inert default branch is the current multi-repo behavior. a later phase raises
/// the ceiling with a fresh higher `to_version` that selects a NEW layout; this
/// baseline never moves the current root.
const FORGE_BASELINE_VERSION: u32 = 0;

/// the first protocol version that selects the forge v2 layout (Phase 9): the
/// height-gated no-downtime demonstrator. below this version every seam picks
/// the baseline multi-repo behavior BYTE-FOR-BYTE (so version 0 AND version 1
/// are inert — every pre-Phase-9 forge test and the Phase-5 inertness test run
/// unchanged); at/after this version the SAME committed heads compose a
/// domain-separated v2 root and ship a v2-tagged snapshot container. this is the
/// single real dual-path divergence a scheduled `to_version >= 2` activates at
/// the agreed height `H`.
const FORGE_MULTIREPO_V2: u32 = 2;

/// the domain tag that separates the v2 root preimage from v1: the SAME sorted
/// `(name, head)` composition, rehashed under this tag, so a v2 node computes a
/// DIFFERENT — but still deterministic and pure — root for identical committed
/// state. this is what makes the activation OBSERVABLE (the forge module root,
/// and hence the global app-hash, changes at `H`) while staying a pure function
/// of the committed heads. NEVER carries a version number into the preimage — the
/// tag is a fixed constant, the version only SELECTS the branch.
const FORGE_V2_ROOT_DOMAIN: &[u8] = b"ducktape.forge.multirepo.v2\x00";

/// the 4-byte magic a v2 snapshot container leads with, so a v2 node's
/// self-contained snapshot bytes are distinguishable from (and never mistaken
/// for) a v1 container. the body after the magic is the identical multi-repo
/// container; only the root GATE differs (v2 gates against [`compose_root_v2`]).
const FORGE_V2_SNAPSHOT_MAGIC: &[u8; 4] = b"FGv2";

/// the version-selected behavior branch for forge's root preimage, snapshot wire
/// format, and repo-field routing. TWO variants now exist (Phase 9): the
/// baseline multi-repo layout (versions `< FORGE_MULTIREPO_V2`) and the v2 layout
/// (versions `>=`). every version-sensitive site (root / snapshot / install /
/// norm-routing) diverges CONSISTENTLY through the single [`forge_layout`] map,
/// so a v2 node round-trips (snapshot v2 -> install v2 -> v2 root) coherently.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ForgeLayout {
    /// the baseline behavior: the sorted multi-repo `compose_root`, the multi-repo
    /// snapshot container, and repo-field-honoring routing (`norm_repo`).
    MultiRepo,
    /// the v2 behavior: the DOMAIN-SEPARATED root ([`compose_root_v2`]) and the
    /// v2-magic snapshot container, with the same multi-repo routing. reached only
    /// after a scheduled `to_version >= FORGE_MULTIREPO_V2` activates at `H`.
    MultiRepoV2,
}

/// map a protocol / active version to the behavior branch it selects. this is
/// the SOLE dual-path decision point. versions below [`FORGE_MULTIREPO_V2`] pick
/// the baseline layout (byte-identical to before Phase 9); at/above they pick the
/// v2 layout. the higher-version arm sits ABOVE the baseline fall-through, and
/// every seam picks it up automatically.
fn forge_layout(version: u32) -> ForgeLayout {
    if version >= FORGE_MULTIREPO_V2 {
        ForgeLayout::MultiRepoV2
    } else {
        ForgeLayout::MultiRepo
    }
}

/// normalize a wire `repo` field UNDER the selected layout. both layouts honor
/// the multi-repo field (empty -> `"default"`, otherwise validated by
/// [`norm_repo`]) — the v2 divergence is in the root preimage / snapshot wire,
/// not in op routing, so a repo targeted before `H` is the same repo after.
fn norm_repo_at(repo: &str, layout: ForgeLayout) -> Result<String, Error> {
    match layout {
        ForgeLayout::MultiRepo | ForgeLayout::MultiRepoV2 => norm_repo(repo),
    }
}

/// parse exactly `OID_RAW_LEN` (20) raw sha1 bytes into an `Oid`, with a
/// deterministic module error on any other length. validates the untrusted
/// `Push` oid fields: `git2::Oid::from_bytes` length-checks too, but a
/// field-named message makes a rejected op self-explaining and the check
/// resolves identically on every validator (same bytes -> same decision).
fn parse_oid(bytes: &[u8], field: &str) -> Result<Oid, Error> {
    if bytes.len() != git::OID_RAW_LEN {
        return Err(Error::Module(format!(
            "forge: {field} must be {} bytes, got {}",
            git::OID_RAW_LEN,
            bytes.len()
        )));
    }
    Oid::from_bytes(bytes).map_err(|e| Error::Module(e.to_string()))
}

/// lowercase-hex a byte slice — for human-readable log lines only (the pack
/// digest in a `materialize` warning).
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// open the per-repo libgit2 repository at `base/<name>`, initializing a fresh
/// sha1 repo there if the dir has no `.git` yet. per-repo dirs are created
/// LAZILY — a repo exists on disk only once something writes to it (a Commit's
/// object build, or a Push's `materialize`). node-local: the dir path is not
/// consensus state, only the committed head oid it yields is.
fn open_or_init_repo(base: &Path, name: &str) -> Result<Repository, Error> {
    let dir = base.join(name);
    let repo = if dir.join(".git").exists() {
        git::open(&dir)
    } else {
        git::init(&dir)
    };
    repo.map_err(|e| Error::Module(e.to_string()))
}

/// the composition [`StateRoot`]: sha256 over `(name, head)` pairs. the caller
/// MUST pass pairs SORTED by name (both callers iterate a `BTreeMap`, which is
/// sorted) — the sort is what makes the root order-independent and a pure
/// function of the committed heads. an empty iterator -> [`StateRoot::ZERO`]
/// (the empty-genesis root). see the composition invariant in the module doc.
fn compose_root<'a>(entries: impl Iterator<Item = (&'a str, Oid)>) -> StateRoot {
    let mut h = Sha256::new();
    let mut any = false;
    for (name, head) in entries {
        any = true;
        // name.len() is byte length; norm_repo bounds names to 64 bytes, so the
        // u32 cast never truncates.
        h.update((name.len() as u32).to_le_bytes());
        h.update(name.as_bytes());
        h.update(head.as_bytes()); // 20 raw sha1 bytes
    }
    if any {
        StateRoot(h.finalize().into())
    } else {
        StateRoot::ZERO
    }
}

/// the v2 composition [`StateRoot`]: the SAME sorted `(name, head)` preimage as
/// [`compose_root`], domain-separated under [`FORGE_V2_ROOT_DOMAIN`] so identical
/// committed heads rehash to a DIFFERENT root under the v2 layout — the
/// observable flip at `H`. still a PURE function of the committed heads (no IO,
/// order-independent via the caller's sort) and preserves the empty-genesis
/// sentinel: no committed head anywhere -> [`StateRoot::ZERO`] under BOTH layouts
/// (so a fresh v2 namespace and a fresh v1 namespace agree on the empty root; the
/// divergence appears exactly when a repo has a committed head).
fn compose_root_v2<'a>(entries: impl Iterator<Item = (&'a str, Oid)>) -> StateRoot {
    let inner = compose_root(entries);
    if inner == StateRoot::ZERO {
        return StateRoot::ZERO;
    }
    let mut h = Sha256::new();
    h.update(FORGE_V2_ROOT_DOMAIN);
    h.update(inner.0);
    StateRoot(h.finalize().into())
}

/// a bounds-checked cursor over untrusted snapshot bytes: every read verifies
/// the remaining length BEFORE slicing, so a forged length field can never
/// allocate or slice past the buffer.
struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}
impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }
    fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }
    fn done(&self) -> bool {
        self.pos == self.buf.len()
    }
    fn u32(&mut self) -> Result<u32, Error> {
        if self.remaining() < 4 {
            return Err(Error::Module("forge snapshot: truncated u32 field".into()));
        }
        let v = u32::from_le_bytes(self.buf[self.pos..self.pos + 4].try_into().unwrap());
        self.pos += 4;
        Ok(v)
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        if self.remaining() < n {
            return Err(Error::Module(format!(
                "forge snapshot: truncated field ({n} bytes needed, {} left)",
                self.remaining()
            )));
        }
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
}

/// the phase-1 per-repo fields, verbatim semantics, now held per named repo in
/// [`Forge::repos`]. every field except `head` is NODE-LOCAL scaffolding — only
/// `head` (the committed HEAD oid) feeds `root()`.
#[derive(Default)]
struct RepoState {
    /// write-through mirror of this repo's COMMITTED `MAIN_REF`: refreshed at
    /// genesis/adopt and by `commit_block`, read by `root()`. the repo/ref is the
    /// source of truth for the committed parent; this cache never feeds a
    /// commit's parent. `None` == unborn repo.
    head: Option<Oid>,
    /// the head STAGED this block: for a `Commit`, `execute` builds the commit
    /// object and points this at it WITHOUT moving the ref; for a `Push`, it is
    /// the pushed `new_oid` (its objects live off-repo, in a node-local pack).
    /// `commit_block` publishes it, `abort_block` drops it. `None` == nothing
    /// staged. NOT in `root()` until committed.
    staged: Option<Oid>,
    /// the pack digest that pairs with a `Push`-`staged` head (32 raw bytes).
    /// `Some` ONLY for a staged Push — a Commit builds its objects straight into
    /// the local odb, so there is nothing to fetch. `commit_block` promotes this
    /// into `pending_pack`; `abort_block` drops it. node-local, never in root().
    staged_pack: Option<[u8; 32]>,
    /// node-local catch-up target: a committed Push head whose objects are not
    /// yet installed on this repo's on-disk `MAIN_REF`, plus the pack digest to
    /// fetch them by. `materialize` clears it once the ref is moved. `root()`
    /// already reflects this head (it is `self.head`) while the on-disk repo
    /// catches up lazily. NOT in `root()`.
    pending_pack: Option<(Oid, [u8; 32])>,
    /// one-shot guard so a not-yet-fetched (or invalid) pack logs ONCE per
    /// pending target instead of on every opportunistic `materialize` retry.
    materialize_warned: bool,
}

impl RepoState {
    /// forget this repo's node-local Push catch-up target — called wherever the
    /// on-disk ref is authoritatively resynced (install / a completed
    /// materialize) so a stale `pending_pack` can't later stomp a newer head.
    /// never touches `head`/`root()`.
    fn clear_pending(&mut self) {
        self.pending_pack = None;
        self.materialize_warned = false;
    }

    /// warn ONCE per pending target (reset when the target changes or clears).
    fn warn(&mut self, msg: String) {
        if !self.materialize_warned {
            eprintln!("[forge] materialize: {msg}");
            self.materialize_warned = true;
        }
    }

    /// node-local catch-up (NON-consensus) for THIS repo: if its on-disk
    /// `MAIN_REF` lags the committed Push head, fetch the head's packfile from
    /// the blob store by digest, install it (libgit2 re-hashes every object),
    /// require the FULL closure, confirm the head fast-forwards the prior ref,
    /// then move the ref. it NEVER reads or writes `head`/`root()` — pack
    /// possession is per-node, so a not-yet-fetched, corrupt, or non-fast-forward
    /// pack is a SAFE no-op that leaves the ref behind (root already reflects the
    /// committed head) and warns once; a later call retries. only a genuine repo
    /// I/O failure surfaces as `Err`. idempotent, and a no-op when nothing is
    /// pending.
    fn materialize(
        &mut self,
        base: &Path,
        name: &str,
        blobs: &files::BlobHandle,
    ) -> Result<(), Error> {
        let Some((head, digest)) = self.pending_pack else {
            return Ok(()); // nothing pending — the common Commit / caught-up case
        };
        let repo = open_or_init_repo(base, name)?;

        // already caught up (e.g. the snapshot install moved the ref)?
        let prior = git::resolve_ref(&repo, MAIN_REF).map_err(|e| Error::Module(e.to_string()))?;
        if prior == Some(head) {
            self.clear_pending();
            return Ok(());
        }

        // fetch the packfile from the node-local body store. absent == not
        // fetched yet: leave the ref behind (root is already correct), warn once.
        let Some(pack) = blobs.get_chunk(&digest) else {
            self.warn(format!(
                "pack {} for repo {name} head {head} not in the blob store yet; \
                 on-disk ref stays behind, root already reflects the committed head",
                hex(&digest)
            ));
            return Ok(());
        };

        // a fetched pack that fails to install / complete the closure / fast-
        // forward is treated exactly like an absent one: a safe no-op that keeps
        // root correct. it is NODE-LOCAL and NEVER gates consensus.
        if let Err(why) = install_and_advance(&repo, head, prior, &pack) {
            self.warn(format!(
                "cannot advance repo {name} on-disk ref to head {head}: {why}; \
                 leaving ref behind (root already correct)"
            ));
            return Ok(());
        }
        self.clear_pending();
        Ok(())
    }
}

/// the pure git side of one materialize attempt: install the pack, require the
/// full closure of `head`, refuse a non-fast-forward onto a born `prior` ref,
/// then move `MAIN_REF` to `head`. any failure is returned so the caller can
/// turn it into a safe no-op.
fn install_and_advance(
    repo: &Repository,
    head: Oid,
    prior: Option<Oid>,
    pack: &[u8],
) -> Result<(), Error> {
    // install re-hashes every object; verify_closure then requires the head
    // commit AND its whole tree/parent closure — a partial pack dies here before
    // the ref moves.
    git::install_pack(repo, pack).map_err(|e| Error::Module(e.to_string()))?;
    git::verify_closure(repo, head).map_err(|e| Error::Module(e.to_string()))?;

    // a born ref may only fast-forward: a normal push builds on the prior head.
    // an unborn prior is the first push and is always allowed. (this is a LOCAL
    // sanity gate — consensus already CAS'd prev_oid; a force push, out of scope
    // here, would legitimately fail this and leave the ref behind with root still
    // correct.)
    if let Some(prior) = prior {
        let ff = git::is_descendant(repo, head, prior).map_err(|e| Error::Module(e.to_string()))?;
        if !ff {
            return Err(Error::Module(format!(
                "head does not fast-forward on-disk ref {prior}"
            )));
        }
    }

    git::update_ref(repo, MAIN_REF, head).map_err(|e| Error::Module(e.to_string()))?;
    Ok(())
}

pub struct Forge {
    id: ModuleId,
    /// node-local container dir — NOT consensus state (the path may differ per
    /// node); only the committed head oids under it are. each named repo lives at
    /// `base/<name>` and is opened per-call, so no `git2` borrow outlives a
    /// method.
    base: PathBuf,
    /// the node-local body store Push packfiles are fetched from by digest —
    /// the SAME plane the files module serves. shared here only so a committed
    /// Push head can be materialized onto the on-disk repo (`materialize`); it is
    /// NEVER read by `root()`/`execute`/`commit_block` and so cannot affect
    /// consensus. a Commit-only deployment can pass a default (unused) handle.
    blobs: files::BlobHandle,
    /// the repo namespace, keyed by normalized slug and kept SORTED (`BTreeMap`)
    /// so `root()` composes order-independently. seeded at construction from the
    /// on-disk dirs (restart re-adopt) and grown lazily on first write.
    repos: BTreeMap<String, RepoState>,
    /// the cached dual-path branch selector (see the SEAM section above). set to
    /// [`FORGE_BASELINE_VERSION`] at genesis, driven deterministically at the
    /// activation height `H` by the host activation hook (a later phase) and
    /// restored per replayed/synced height. it is NEVER part of the
    /// `root()`/`snapshot()` preimage — it only SELECTS the branch — so flipping
    /// it recomposes `root()` from the in-memory heads with zero odb/blob IO.
    active_version: u32,
}

impl Forge {
    /// genesis wiring with a private, default (empty) blob store — enough for a
    /// `Commit`-only or test deployment. a node that wants `Push` materialization
    /// to reuse the files body plane must build forge over that shared handle
    /// with [`Forge::with_blobs`].
    pub fn init(id: impl Into<ModuleId>, base_dir: impl Into<PathBuf>) -> Result<Self, Error> {
        Self::with_blobs(id, base_dir, files::BlobHandle::default())
    }

    /// genesis wiring over an EXISTING node-local blob store — mirrors
    /// [`files::Files::with_blobs`]. the embedding daemon creates one handle,
    /// registers the files module over it (uploads land there), and builds forge
    /// over a clone so a `Push`'s packfile — uploaded before the op is submitted
    /// — is visible to `materialize` without a byte crossing consensus.
    ///
    /// `base_dir` is the CONTAINER for the repo namespace; each repo lives at
    /// `base_dir/<name>`. construction creates the container and RE-ADOPTS every
    /// existing per-repo git dir, seeding each repo's committed head from its
    /// on-disk ref — so `root()` is correct immediately after a restart, where
    /// forge reopens from disk with no snapshot install. a fresh container has no
    /// repos and starts at [`StateRoot::ZERO`], the unborn-genesis root.
    pub fn with_blobs(
        id: impl Into<ModuleId>,
        base_dir: impl Into<PathBuf>,
        blobs: files::BlobHandle,
    ) -> Result<Self, Error> {
        let base = base_dir.into();
        std::fs::create_dir_all(&base)
            .map_err(|e| Error::Module(format!("forge: create base dir: {e}")))?;

        let mut repos = BTreeMap::new();
        for entry in std::fs::read_dir(&base)
            .map_err(|e| Error::Module(format!("forge: scan base dir: {e}")))?
        {
            let entry = entry.map_err(|e| Error::Module(e.to_string()))?;
            if !entry
                .file_type()
                .map_err(|e| Error::Module(e.to_string()))?
                .is_dir()
            {
                continue;
            }
            // a subdir that is not a valid repo slug, or not a git repo, cannot
            // be one this module created — ignore it (it never contributes to
            // root). a valid slug returns unchanged from norm_repo, so `name`
            // equals the directory name.
            let Some(dir_name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Ok(name) = norm_repo(&dir_name) else {
                continue;
            };
            let dir = base.join(&name);
            if !dir.join(".git").exists() {
                continue;
            }
            let repo = git::open(&dir).map_err(|e| Error::Module(e.to_string()))?;
            let head =
                git::resolve_ref(&repo, MAIN_REF).map_err(|e| Error::Module(e.to_string()))?;
            repos.insert(
                name,
                RepoState {
                    head,
                    ..Default::default()
                },
            );
        }

        Ok(Self {
            id: id.into(),
            base,
            blobs,
            repos,
            // genesis boots on the baseline branch — the current behavior. the
            // activation hook (a later phase) is the only thing that moves it.
            active_version: FORGE_BASELINE_VERSION,
        })
    }

    /// the current dual-path branch selector. read-only accessor for the host /
    /// tests; the value is driven by [`Forge::set_active_version`].
    pub fn active_version(&self) -> u32 {
        self.active_version
    }

    /// deterministically set the dual-path branch selector. driven by the host
    /// activation hook at the agreed height `H` (via the [`Module::set_active_version`]
    /// override below, which the host calls across the registry from the
    /// orchestrator's agreed `RespawnPlan::boundary_version`), and by
    /// restart/state-sync restoration. inherent counterpart kept for concrete-typed
    /// callers (tests); it is NEVER folded into the `root()`/`snapshot()` preimage.
    pub fn set_active_version(&mut self, v: u32) {
        self.active_version = v;
    }

    /// ensure a [`RepoState`] entry exists for `name` (already normalized). the
    /// git DIR is created lazily by the first write; construction already
    /// re-adopted every on-disk repo, so a brand-new entry here is genuinely a
    /// repo that does not exist on disk yet and starts unborn.
    fn ensure_repo(&mut self, name: &str) {
        if !self.repos.contains_key(name) {
            self.repos.insert(name.to_string(), RepoState::default());
        }
    }

    // ---- state-sync ---------------------------------------------------------
    // a snapshot is SELF-CONTAINED BYTES — a repo-count then, per repo sorted by
    // name, its name, 20-byte committed head oid, and a packfile of the head's
    // FULL object closure — so it can ride a bulk data channel between nodes that
    // share nothing (no common filesystem, no remote, no `git` binary). the head
    // oids bind the snapshot to the composed root, which is what lets install
    // verify against an expected root before a single byte touches any odb.

    /// serialize the COMMITTED state into self-contained snapshot bytes. the
    /// container is `u32-LE(repo_count)` then, per repo with a committed head
    /// (sorted by name): `u32-LE(name_len) name` `[20-byte head oid]`
    /// `u32-LE(pack_len) pack`. only born repos are carried — they are exactly
    /// the repos that contribute to `root()`, so an installed snapshot reproduces
    /// the composed root. an empty namespace serializes as a single zero count
    /// (`[0,0,0,0]`), the marker for [`StateRoot::ZERO`]. a staged (this-block)
    /// head is deliberately excluded — a snapshot must reproduce `root()`.
    pub fn snapshot(&self) -> Result<Vec<u8>, Error> {
        // SEAM (dual-path snapshot wire): the selected layout picks the
        // container format. `active_version` selects it (a snapshot has no
        // `Ctx`); it is NEVER serialized. inert today — the current multi-repo
        // container, so the bytes are byte-identical to before this seam.
        match forge_layout(self.active_version) {
            ForgeLayout::MultiRepo => self.snapshot_multi_repo(),
            ForgeLayout::MultiRepoV2 => self.snapshot_multi_repo_v2(),
        }
    }

    /// serialize the COMMITTED state under the v2 container: the
    /// [`FORGE_V2_SNAPSHOT_MAGIC`] tag followed by the identical multi-repo body.
    /// the tag makes a v2 snapshot self-identifying on the wire; the root GATE at
    /// install differs (v2 gates against [`compose_root_v2`]), so a v2 snapshot
    /// round-trips only through a v2 install, reproducing the v2 root.
    fn snapshot_multi_repo_v2(&self) -> Result<Vec<u8>, Error> {
        let mut out = FORGE_V2_SNAPSHOT_MAGIC.to_vec();
        out.extend_from_slice(&self.snapshot_multi_repo()?);
        Ok(out)
    }

    /// serialize the COMMITTED state under the multi-repo container (the current,
    /// baseline format — see [`Forge::snapshot`] for the format contract).
    fn snapshot_multi_repo(&self) -> Result<Vec<u8>, Error> {
        let born: Vec<(&str, Oid)> = self
            .repos
            .iter()
            .filter_map(|(name, s)| s.head.map(|h| (name.as_str(), h)))
            .collect();

        let mut out = Vec::new();
        out.extend_from_slice(&(born.len() as u32).to_le_bytes());
        for (name, head) in born {
            // a born head's objects live in its repo's odb (a Commit built them
            // there, or a Push materialized them). pack the full closure.
            let repo = open_or_init_repo(&self.base, name)?;
            let pack = git::pack_closure(&repo, head).map_err(|e| Error::Module(e.to_string()))?;
            out.extend_from_slice(&(name.len() as u32).to_le_bytes());
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(head.as_bytes());
            out.extend_from_slice(&(pack.len() as u32).to_le_bytes());
            out.extend_from_slice(&pack);
        }
        Ok(out)
    }

    /// replace this module's WHOLE namespace with snapshot bytes, gated on
    /// `expected`. the bytes are UNTRUSTED (a byzantine peer produced them), so
    /// the order is verify-then-mutate:
    ///
    /// 1. PARSE the entire container with a bounds-checked reader — no write.
    /// 2. ROOT GATE: the composed root of the parsed heads must equal `expected`
    ///    (the multi-repo analogue of P1's single-oid rehash), before any byte
    ///    reaches an odb. a tampered head oid dies here.
    /// 3. INSTALL each pack (libgit2 re-hashes every object) and require each
    ///    head's FULL closure — still moving NO ref. a tampered/partial pack dies
    ///    here; stranded objects are node-local orphans and `root()` is unchanged.
    /// 4. PUBLISH: install is a full REPLACEMENT — unbind any currently-born repo
    ///    the snapshot drops (durably, so a restart re-adopt can't resurrect it),
    ///    then move every snapshot repo's ref and rebuild the map.
    ///
    /// on any `Err` before step 4 the committed refs — and so `root()` — are
    /// byte-identical to before the call. on `Ok` all staged/pending state is
    /// dropped: install is a full state replacement, not a merge.
    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        // SEAM (dual-path snapshot wire): decode + install under the selected
        // layout (must match the format `snapshot` emits at this version).
        // `active_version` selects it (install has no `Ctx`). inert today — the
        // current multi-repo container, so accept/reject and the gated root are
        // unchanged.
        match forge_layout(self.active_version) {
            ForgeLayout::MultiRepo => self.install_body(bytes, expected, ForgeLayout::MultiRepo),
            ForgeLayout::MultiRepoV2 => {
                // a v2 container leads with the magic tag; strip it, then install
                // the identical body gated against the v2 (domain-separated) root.
                let body = bytes.strip_prefix(FORGE_V2_SNAPSHOT_MAGIC.as_slice()).ok_or_else(|| {
                    Error::Module(
                        "forge snapshot: expected a v2 container (missing FGv2 magic)".into(),
                    )
                })?;
                self.install_body(body, expected, ForgeLayout::MultiRepoV2)
            }
        }
    }

    /// replace the WHOLE namespace from a multi-repo snapshot BODY, gated on
    /// `expected` under `layout` (the layout picks the root composition the gate
    /// verifies against — [`compose_root`] for baseline, [`compose_root_v2`] for
    /// v2; the container bytes are otherwise identical). see [`Forge::install`]
    /// for the verify-then-mutate contract.
    fn install_body(
        &mut self,
        bytes: &[u8],
        expected: StateRoot,
        layout: ForgeLayout,
    ) -> Result<(), Error> {
        // ---- PHASE 1: parse (no writes) -------------------------------------
        let mut r = Reader::new(bytes);
        let count = r.u32()?;
        // parsed is keyed+sorted by name so the composed root matches root().
        let mut parsed: BTreeMap<String, (Oid, &[u8])> = BTreeMap::new();
        for _ in 0..count {
            let name_len = r.u32()? as usize;
            let name = std::str::from_utf8(r.take(name_len)?)
                .map_err(|_| Error::Module("forge snapshot: repo name not utf-8".into()))?;
            // validate the slug deterministically — a byzantine name is rejected
            // identically on every node.
            let name = norm_repo(name)?;
            let oid = Oid::from_bytes(r.take(git::OID_RAW_LEN)?)
                .map_err(|e| Error::Module(e.to_string()))?;
            if oid.is_zero() {
                return Err(Error::Module(format!(
                    "forge snapshot: repo {name} carries a zero head oid \
                     (unborn repos are not serialized)"
                )));
            }
            let pack_len = r.u32()? as usize;
            let pack = r.take(pack_len)?;
            if parsed.insert(name.clone(), (oid, pack)).is_some() {
                return Err(Error::Module(format!(
                    "forge snapshot: duplicate repo {name}"
                )));
            }
        }
        if !r.done() {
            return Err(Error::Module(
                "forge snapshot: trailing bytes after the container".into(),
            ));
        }

        // ---- PHASE 2: root gate BEFORE any byte reaches an odb --------------
        // the layout picks the composition the gate verifies against, so a v2
        // snapshot must rehash to the v2 (domain-separated) root, a v1 to the v1.
        let entries = parsed.iter().map(|(n, (oid, _))| (n.as_str(), *oid));
        let composed = match layout {
            ForgeLayout::MultiRepo => compose_root(entries),
            ForgeLayout::MultiRepoV2 => compose_root_v2(entries),
        };
        if composed != expected {
            return Err(Error::Module(
                "snapshot root mismatch: composed repo heads do not rehash to the expected root"
                    .into(),
            ));
        }

        // ---- PHASE 3: index packs + require closures, moving NO ref ---------
        for (name, (oid, pack)) in &parsed {
            let repo = open_or_init_repo(&self.base, name)?;
            git::install_pack(&repo, pack).map_err(|e| Error::Module(e.to_string()))?;
            git::verify_closure(&repo, *oid).map_err(|e| Error::Module(e.to_string()))?;
        }

        // ---- PHASE 4: publish (full replacement) ----------------------------
        // unbind any currently-born repo the snapshot drops. deleting the ref
        // (not just clearing the cache) keeps a restart re-adopt from resurrecting
        // a superseded head — the multi-repo analogue of the empty-marker unbind.
        let keep: BTreeSet<&str> = parsed.keys().map(String::as_str).collect();
        let drop_born: Vec<String> = self
            .repos
            .iter()
            .filter(|(n, s)| s.head.is_some() && !keep.contains(n.as_str()))
            .map(|(n, _)| n.clone())
            .collect();
        for name in &drop_born {
            let repo = open_or_init_repo(&self.base, name)?;
            git::delete_ref(&repo, MAIN_REF).map_err(|e| Error::Module(e.to_string()))?;
        }

        let mut new_repos = BTreeMap::new();
        for (name, (oid, _)) in parsed {
            let repo = open_or_init_repo(&self.base, &name)?;
            git::update_ref(&repo, MAIN_REF, oid).map_err(|e| Error::Module(e.to_string()))?;
            new_repos.insert(
                name,
                RepoState {
                    head: Some(oid),
                    ..Default::default()
                },
            );
        }
        self.repos = new_repos;
        Ok(())
    }

    /// node-local catch-up across ALL repos: opportunistically materialize each
    /// repo whose on-disk ref lags a committed Push head. a no-op for repos with
    /// nothing pending. NEVER touches `head`/`root()` (see [`RepoState::
    /// materialize`]).
    pub fn materialize(&mut self) -> Result<(), Error> {
        let base = &self.base;
        let blobs = &self.blobs;
        for (name, state) in self.repos.iter_mut() {
            state.materialize(base, name, blobs)?;
        }
        Ok(())
    }

    /// stage one `Commit` onto `name` (already normalized + ensured): build the
    /// deterministic commit object over that repo's parent tree and point its
    /// `staged` at it WITHOUT moving the ref (the host publishes at the block
    /// boundary). the phase-1 commit path, per repo.
    fn stage_commit(
        &mut self,
        name: &str,
        consensus_time: u64,
        path: String,
        content: String,
        message: String,
    ) -> Result<(), Error> {
        let repo = open_or_init_repo(&self.base, name)?;
        let state = self.repos.get_mut(name).expect("ensured by caller");

        // 1. parent := the STAGED head if this block already committed here, else
        //    the REPO's current (committed) head. chaining on the staged head
        //    gives multi-commit-in-one-block the correct parent.
        let parent_oid = match state.staged {
            Some(oid) => Some(oid),
            None => git::resolve_ref(&repo, MAIN_REF).map_err(|e| Error::Module(e.to_string()))?,
        };
        let parent_commit = parent_oid
            .map(|oid| repo.find_commit(oid))
            .transpose()
            .map_err(|e| Error::Module(e.to_string()))?;

        // 2. write the blob and build the tree in-memory (seeded from the
        //    parent's tree = incremental). no on-disk index, no worktree.
        let blob = repo
            .blob(content.as_bytes())
            .map_err(|e| Error::Module(e.to_string()))?;
        let base_tree = parent_commit
            .as_ref()
            .map(|c| c.tree())
            .transpose()
            .map_err(|e| Error::Module(e.to_string()))?;
        let tree_oid = git::build_tree(&repo, base_tree.as_ref(), &path, blob)
            .map_err(|e| Error::Module(e.to_string()))?;
        let tree = repo
            .find_tree(tree_oid)
            .map_err(|e| Error::Module(e.to_string()))?;

        // 3. deterministic commit object: date from consensus_time, fixed identity.
        let commit = git::commit(
            &repo,
            &tree,
            parent_commit.as_ref(),
            &message,
            consensus_time,
        )
        .map_err(|e| Error::Module(e.to_string()))?;

        // 4. STAGE the new head — the objects are already in this odb, so
        //    `staged_pack` stays `None`: commit_block moves the ref directly.
        state.staged = Some(commit);
        state.staged_pack = None;
        Ok(())
    }

    /// stage one `Push` onto `name` (already normalized + ensured): the git-
    /// faithful ref update. PURE and deterministic — the only gate is a compare-
    /// and-swap on THAT repo's COMMITTED head, fully determined by consensus
    /// state, so accept/reject and the resulting composed `root()` are identical
    /// on every validator whether or not it holds the packfile. no repo is
    /// opened, nothing is installed, no ref moves here.
    fn stage_push(
        &mut self,
        name: &str,
        prev_oid: Option<Vec<u8>>,
        new_oid: Vec<u8>,
        pack_digest: Vec<u8>,
    ) -> Result<(), Error> {
        // 1. length-validate the untrusted wire fields (deterministic).
        let new = parse_oid(&new_oid, "new_oid")?;
        let prev = prev_oid
            .as_deref()
            .map(|b| parse_oid(b, "prev_oid"))
            .transpose()?;
        let digest: [u8; 32] = pack_digest.as_slice().try_into().map_err(|_| {
            Error::Module(format!(
                "forge: pack_digest must be 32 bytes, got {}",
                pack_digest.len()
            ))
        })?;

        let state = self.repos.get_mut(name).expect("ensured by caller");

        // 2. CAS on THIS repo's COMMITTED head (never the staged one): the SOLE
        //    consensus gate, reading only agreed state. `None` prev must match an
        //    unborn repo. a mismatch is a non-fast-forward.
        if state.head != prev {
            return Err(Error::Module(
                "non-fast-forward: forge HEAD moved; fetch and retry".into(),
            ));
        }

        // 3. stage the new head + remember its pack digest so commit_block can
        //    record the node-local materialization target.
        state.staged = Some(new);
        state.staged_pack = Some(digest);
        Ok(())
    }

    /// this repo's read-your-writes head hex: a staged (this-block) head shadows
    /// the committed one. `None` when the repo is absent or unborn.
    fn read_head(&self, name: &str) -> Option<String> {
        self.repos
            .get(name)
            .and_then(|s| s.staged.or(s.head))
            .map(|oid| oid.to_string())
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Forge {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// ACTIVATION HOOK (design §4). the host drives this across the registry at
    /// the finalized boundary from the agreed `RespawnPlan::boundary_version`, so
    /// forge selects its dual-path branch deterministically at `H`. `version` is a
    /// non-hashed branch selector — NEVER part of the `root()`/`snapshot()`
    /// preimage.
    fn set_active_version(&mut self, version: u32) {
        self.active_version = version;
    }

    /// the composed namespace root: `sha256` over `(name, committed head)` for
    /// every repo with a head, sorted by name — pure, no IO (that's the whole
    /// reason `head` is a write-through cache). no committed head anywhere ->
    /// `ZERO`. see the composition invariant in the module doc.
    fn root(&self) -> StateRoot {
        // SEAM (dual-path root preimage): the selected layout picks the root
        // composition. `active_version` selects the branch and is NEVER part of
        // the preimage — flipping it recomposes from the same in-memory heads.
        // inert today — every layout composes the current multi-repo
        // `compose_root`, so `root()` is byte-identical for every input.
        let entries = self
            .repos
            .iter()
            .filter_map(|(name, s)| s.head.map(|h| (name.as_str(), h)));
        match forge_layout(self.active_version) {
            ForgeLayout::MultiRepo => compose_root(entries),
            ForgeLayout::MultiRepoV2 => compose_root_v2(entries),
        }
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(self.snapshot()?))
    }

    /// apply one write op to its target repo. a `Commit` builds a deterministic
    /// commit object in that repo's local odb and stages it; a `Push` is PURE —
    /// a compare-and-swap on that repo's committed HEAD, no git IO, no pack
    /// install, no ref move, so its accept/reject and resulting composed `root()`
    /// are identical on every validator regardless of pack possession
    /// (materialization is deferred to `commit_block` -> `materialize`). the
    /// empty/absent `repo` field maps to the `"default"` repo (back-compat). all
    /// git2 IO is blocking with no `.await`, so the "await only deterministic
    /// resources" rule holds vacuously.
    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        // SEAM (dual-path repo routing): interpret the wire `repo` field under
        // the layout the THIS-BLOCK protocol version selects. `protocol_version`
        // is the read-only per-dispatch input (never hashed); inert today — the
        // layout always honors the multi-repo field, so accept/reject is
        // unchanged.
        let layout = forge_layout(ctx.env().protocol_version);
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            ForgeMsg::Commit {
                repo,
                path,
                content,
                message,
            } => {
                let name = norm_repo_at(&repo, layout)?;
                self.ensure_repo(&name);
                self.stage_commit(&name, ctx.env().consensus_time, path, content, message)
            }
            ForgeMsg::Push {
                repo,
                prev_oid,
                new_oid,
                pack_digest,
            } => {
                let name = norm_repo_at(&repo, layout)?;
                self.ensure_repo(&name);
                self.stage_push(&name, prev_oid, new_oid, pack_digest)
            }
        }
    }

    /// read projections over the namespace, served from the cached mirrors — no
    /// IO, no `.await`. `Head`/`HeadOf` are read-your-writes (a staged head
    /// shadows the committed one) and return the raw 40-char sha1 oid hex (the
    /// root's preimage material). `ListRepos` returns every repo's COMMITTED head
    /// hex, sorted by name.
    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            ForgeQuery::Head => Ok(encode_reply(&ForgeReply::Head(
                self.read_head(DEFAULT_REPO),
            ))),
            ForgeQuery::HeadOf { repo } => {
                // SEAM (dual-path repo routing): a query has no `Ctx`, so it
                // routes under the module's committed branch selector. inert
                // today — the layout honors the multi-repo field.
                let name = norm_repo_at(&repo, forge_layout(self.active_version))?;
                Ok(encode_reply(&ForgeReply::Head(self.read_head(&name))))
            }
            ForgeQuery::ListRepos => {
                let repos = self
                    .repos
                    .iter()
                    .map(|(name, s)| RepoHead {
                        name: name.clone(),
                        head: s.head.map(|oid| oid.to_string()),
                    })
                    .collect();
                Ok(encode_reply(&ForgeReply::Repos(repos)))
            }
        }
    }

    /// publish every repo's staged head so `root()` reflects it. for a `Commit`
    /// the objects are already in that repo's odb, so its ref moves here directly.
    /// for a `Push` the objects live in a node-local pack this node may not hold:
    /// publishing must NOT depend on the pack (or validators diverge), so it sets
    /// `head`, records a materialization target, and invokes `materialize`
    /// opportunistically (the submitter, holding the pack, catches up immediately;
    /// a node lacking the pack is a safe no-op with `root()` already correct).
    async fn commit_block(&mut self) -> Result<(), Error> {
        let base = &self.base;
        let blobs = &self.blobs;
        for (name, state) in self.repos.iter_mut() {
            let Some(oid) = state.staged.take() else {
                continue;
            };
            match state.staged_pack.take() {
                None => {
                    // Commit: the commit object is in this odb; move the ref now.
                    let repo = open_or_init_repo(base, name)?;
                    git::update_ref(&repo, MAIN_REF, oid)
                        .map_err(|e| Error::Module(e.to_string()))?;
                    state.head = Some(oid);
                }
                Some(digest) => {
                    // Push: publish the head for `root()` unconditionally (the
                    // determinism invariant), then try to materialize this repo's
                    // on-disk ref from the local pack. a fresh target re-arms the
                    // one-shot warn.
                    state.head = Some(oid);
                    state.pending_pack = Some((oid, digest));
                    state.materialize_warned = false;
                    state.materialize(base, name, blobs)?;
                }
            }
        }
        Ok(())
    }

    /// discard every repo's staged head — no ref moved, so `root()` is unchanged;
    /// any built commit objects linger unreferenced in the odb, and a staged
    /// Push's pack digest is dropped. a committed Push's pending materialization
    /// target is untouched (it belongs to an already-published head).
    async fn abort_block(&mut self) -> Result<(), Error> {
        for state in self.repos.values_mut() {
            state.staged = None;
            state.staged_pack = None;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{decode_reply, encode_msg, encode_query};

    // a minimal Ctx so execute can read consensus_time without a full host.
    struct TestCtx {
        env: sdk::Env,
    }
    impl TestCtx {
        fn at(consensus_time: u64) -> Self {
            Self {
                env: sdk::Env { protocol_version: 0,
                    height: 0,
                    consensus_time,
                    origin: sdk::Origin::System,
                    me: "forge".into(),
                },
            }
        }
    }
    #[async_trait::async_trait(?Send)]
    impl Ctx for TestCtx {
        fn env(&self) -> &sdk::Env {
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

    fn tmp_base(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("ducktape-forge-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    fn commit_msg(repo: &str, path: &str, content: &str, message: &str) -> Msg {
        Msg {
            target: "forge".into(),
            payload: encode_msg(&ForgeMsg::Commit {
                repo: repo.into(),
                path: path.into(),
                content: content.into(),
                message: message.into(),
            }),
        }
    }

    fn commit(forge: &mut Forge, t: u64, repo: &str, path: &str, content: &str, message: &str) {
        futures::executor::block_on(forge.execute(
            &mut TestCtx::at(t),
            &commit_msg(repo, path, content, message),
        ))
        .unwrap();
        futures::executor::block_on(forge.commit_block()).unwrap();
    }

    // read a repo's HEAD oid via git2 directly (opening base/<name>) — the
    // independent oracle that root() tracks the real refs, not just the cache.
    fn git_head_oid(base: &Path, repo: &str) -> Oid {
        git2::Repository::open(base.join(repo))
            .unwrap()
            .refname_to_id(MAIN_REF)
            .unwrap()
    }

    #[test]
    fn genesis_is_zero_then_commit_makes_root_equal_composed_head() {
        let base = tmp_base("basic");
        let mut forge = Forge::init("forge", base.clone()).unwrap();
        assert_eq!(
            forge.root(),
            StateRoot::ZERO,
            "empty namespace -> ZERO root"
        );

        // a Commit with an EMPTY repo -> the default repo (back-compat wire).
        commit(&mut forge, 100, "", "a.txt", "hello", "first");

        assert_ne!(forge.root(), StateRoot::ZERO, "a commit must move the root");

        // root() == the composition over {"default": <real git HEAD oid>}.
        let head = git_head_oid(&base, DEFAULT_REPO);
        assert_eq!(
            forge.root(),
            compose_root([(DEFAULT_REPO, head)].into_iter()),
            "root() must be the composition of the real default-repo HEAD oid"
        );

        // the unit Head query surfaces that same oid hex (the root's preimage).
        let reply =
            futures::executor::block_on(forge.query(&encode_query(&ForgeQuery::Head))).unwrap();
        assert_eq!(
            decode_reply(&reply).unwrap(),
            ForgeReply::Head(Some(head.to_string()))
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn second_commit_moves_the_root() {
        let base = tmp_base("second");
        let mut forge = Forge::init("forge", base.clone()).unwrap();
        commit(&mut forge, 1, "", "a.txt", "one", "c1");
        let r1 = forge.root();
        commit(&mut forge, 2, "", "b.txt", "two", "c2");
        let r2 = forge.root();
        assert_ne!(r1, r2, "a second commit must advance the root");
        assert_eq!(
            r2,
            compose_root([(DEFAULT_REPO, git_head_oid(&base, DEFAULT_REPO))].into_iter())
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn root_composes_into_global_root() {
        let base = tmp_base("compose");
        let mut forge = Forge::init("forge", base.clone()).unwrap();
        let before = state::global_root(&[&forge as &dyn Module]);
        commit(&mut forge, 7, "", "a.txt", "x", "c");
        let after = state::global_root(&[&forge as &dyn Module]);
        assert_ne!(
            before, after,
            "forge's git-backed root must move the global app-hash"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    // determinism: two independent namespaces, same inputs -> identical composed
    // root. the pinned Signature makes each commit's sha1 oid byte-identical; the
    // per-repo odb path never enters the commit bytes.
    #[test]
    fn commit_oid_is_reproducible_across_namespaces() {
        let a = tmp_base("det-a");
        let b = tmp_base("det-b");
        let mut fa = Forge::init("forge", a.clone()).unwrap();
        let mut fb = Forge::init("forge", b.clone()).unwrap();
        commit(&mut fa, 555, "myrepo", "f.txt", "same", "same-msg");
        commit(&mut fb, 555, "myrepo", "f.txt", "same", "same-msg");
        assert_eq!(
            fa.root(),
            fb.root(),
            "pinned identity+date -> reproducible commit oid -> identical root"
        );
        let _ = std::fs::remove_dir_all(&a);
        let _ = std::fs::remove_dir_all(&b);
    }

    // upgrade dual-path SEAM (Phase 5): the branch selector defaults to the
    // baseline and the seam is INERT — flipping `active_version` recomposes the
    // SAME root/snapshot from the same in-memory heads (no real v2 divergence
    // exists yet), and every version maps to the one current layout.
    #[test]
    fn active_version_defaults_to_baseline_and_seam_is_inert() {
        let base = tmp_base("active-version");
        let mut forge = Forge::init("forge", base.clone()).unwrap();

        // genesis boots on the baseline branch.
        assert_eq!(forge.active_version(), FORGE_BASELINE_VERSION);
        // baseline AND baseline+1 stay on the inert layout (the v2 arm is gated
        // at FORGE_MULTIREPO_V2 = 2, exercised by the v2 divergence test); this
        // test flips only to baseline+1 below, so its root/snapshot invariance
        // still holds byte-for-byte.
        assert_eq!(forge_layout(FORGE_BASELINE_VERSION), ForgeLayout::MultiRepo);
        assert_eq!(forge_layout(FORGE_BASELINE_VERSION + 1), ForgeLayout::MultiRepo);

        // stage some committed state so root()/snapshot() are non-trivial.
        commit(&mut forge, 42, "docs", "a.txt", "x", "c");
        commit(&mut forge, 42, "", "b.txt", "y", "c");
        let root_baseline = forge.root();
        let snap_baseline = forge.snapshot().unwrap();

        // flipping the selector must NOT move root() or snapshot() today: the
        // seam exists, the behavior does not. (the real v2 divergence is a later
        // phase.)
        forge.set_active_version(FORGE_BASELINE_VERSION + 1);
        assert_eq!(forge.active_version(), FORGE_BASELINE_VERSION + 1);
        assert_eq!(
            forge.root(),
            root_baseline,
            "inert seam: root() must be branch-invariant"
        );
        assert_eq!(
            forge.snapshot().unwrap(),
            snap_baseline,
            "inert seam: snapshot() must be branch-invariant"
        );

        // and a snapshot round-trips within the baseline layout.
        forge.set_active_version(FORGE_BASELINE_VERSION);
        let rt_base = tmp_base("active-version-rt");
        let mut fresh = Forge::init("forge", rt_base.clone()).unwrap();
        fresh.install(&snap_baseline, root_baseline).unwrap();
        assert_eq!(fresh.root(), root_baseline, "install reproduces the root");

        let _ = std::fs::remove_dir_all(&base);
        let _ = std::fs::remove_dir_all(&rt_base);
    }

    // upgrade dual-path (Phase 9): the forge v2 layout is a REAL divergence.
    // versions below FORGE_MULTIREPO_V2 stay byte-identical to the baseline (so
    // every existing test — which runs at baseline — passes unchanged); at/after
    // it the SAME committed heads compose a DIFFERENT root and a v2-tagged
    // snapshot, and a v2 node round-trips (snapshot v2 -> install v2 -> v2 root).
    #[test]
    fn v2_layout_diverges_and_round_trips_while_v1_stays_inert() {
        let base = tmp_base("v2");
        let mut forge = Forge::init("forge", base.clone()).unwrap();

        // the layout selector: 0 and 1 are baseline (inert), 2+ select v2.
        assert_eq!(forge_layout(0), ForgeLayout::MultiRepo);
        assert_eq!(forge_layout(1), ForgeLayout::MultiRepo);
        assert_eq!(forge_layout(2), ForgeLayout::MultiRepoV2);
        assert_eq!(forge_layout(7), ForgeLayout::MultiRepoV2);

        // stage committed state so root()/snapshot() are non-trivial.
        commit(&mut forge, 42, "docs", "a.txt", "x", "c");
        commit(&mut forge, 42, "", "b.txt", "y", "c");

        // baseline (v0) and v1 must be BYTE-IDENTICAL — the inertness proof.
        forge.set_active_version(0);
        let root_v1 = forge.root();
        let snap_v1 = forge.snapshot().unwrap();
        forge.set_active_version(1);
        assert_eq!(forge.root(), root_v1, "v1 must equal the baseline root");
        assert_eq!(
            forge.snapshot().unwrap(),
            snap_v1,
            "v1 snapshot must equal the baseline snapshot"
        );

        // v2: the SAME committed heads compose a DIFFERENT root, and the snapshot
        // container leads with the v2 magic.
        forge.set_active_version(FORGE_MULTIREPO_V2);
        let root_v2 = forge.root();
        let snap_v2 = forge.snapshot().unwrap();
        assert_ne!(root_v2, root_v1, "v2 root must diverge from v1 (the flip)");
        assert_ne!(root_v2, StateRoot::ZERO, "v2 root over committed heads is non-zero");
        assert!(
            snap_v2.starts_with(FORGE_V2_SNAPSHOT_MAGIC.as_slice()),
            "v2 snapshot leads with the FGv2 magic"
        );
        // and it is exactly the domain-separated composition of the same heads.
        let heads: Vec<(String, Oid)> = forge
            .repos
            .iter()
            .filter_map(|(n, s)| s.head.map(|h| (n.clone(), h)))
            .collect();
        assert_eq!(
            root_v2,
            compose_root_v2(heads.iter().map(|(n, o)| (n.as_str(), *o))),
            "v2 root == compose_root_v2 over the committed heads"
        );

        // a v2 snapshot round-trips ONLY through a v2 install, reproducing the v2 root.
        let rt = tmp_base("v2-rt");
        let mut fresh = Forge::init("forge", rt.clone()).unwrap();
        fresh.set_active_version(FORGE_MULTIREPO_V2);
        fresh.install(&snap_v2, root_v2).unwrap();
        assert_eq!(fresh.root(), root_v2, "v2 install reproduces the v2 root");

        // and a v1 install of a v2 container (or vice versa) is rejected — the
        // magic/gate keep the layouts from being silently confused.
        let rt2 = tmp_base("v2-rt-mismatch");
        let mut mismatch = Forge::init("forge", rt2.clone()).unwrap();
        mismatch.set_active_version(0);
        assert!(
            mismatch.install(&snap_v2, root_v2).is_err(),
            "a baseline install must reject a v2 container"
        );

        let _ = std::fs::remove_dir_all(&base);
        let _ = std::fs::remove_dir_all(&rt);
        let _ = std::fs::remove_dir_all(&rt2);
    }

    #[test]
    fn norm_repo_maps_empty_to_default_and_validates_slugs() {
        assert_eq!(norm_repo("").unwrap(), DEFAULT_REPO);
        assert_eq!(norm_repo("docs").unwrap(), "docs");
        assert_eq!(norm_repo("a.b_c-1").unwrap(), "a.b_c-1");
        for bad in ["Docs", "a/b", "a b", ".", "..", "a\0b"] {
            assert!(norm_repo(bad).is_err(), "{bad:?} must be rejected");
        }
        assert!(norm_repo(&"a".repeat(65)).is_err(), "65 bytes too long");
        assert!(norm_repo(&"a".repeat(64)).is_ok(), "64 bytes ok");
    }
}
