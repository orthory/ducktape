//! ============================================================================
//! STORAGE SUBSTRATE — DECIDED DIRECTION (2026-07-01)
//!   git2-rs (vendored libgit2) + sha1;  root() = sha256(<sha1 HEAD oid>)
//! ============================================================================
//!
//! forge stores its state in a real git repo via libgit2 (the `git2-rs` crate,
//! VENDORED — so the node binary is SELF-CONTAINED, no `git` install needed), in
//! git's DEFAULT sha1 object format, with `root() = sha256(<20-byte sha1 head
//! oid>)`. the reasoning, because it is a non-obvious stack of trade-offs:
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
//! WHY root() = sha256(oid) (rehash), not the oid verbatim:
//!   a [`StateRoot`] is 32 bytes; a sha1 oid is only 20. so the 20-byte HEAD oid
//!   is the sha256 PREIMAGE -> 32 bytes. the payoff: forge's contribution to the
//!   global app-hash is sha256-STRENGTH. the only residual sha1 surface is a
//!   *forge-object* collision (two trees under one commit oid) — expensive and
//!   SHA-1DC-guarded — while the app-hash's collision resistance at the STATE
//!   layer stays sha256. we trade "root IS a git oid verbatim" for real-world git
//!   interop, a good trade for a git PRODUCT. (empty repo -> StateRoot::ZERO.)
//!
//! WHAT DOES NOT CHANGE:
//!   - DETERMINISM: git2's typed `Signature` sets the FIXED `ducktape` identity +
//!     a date from `ctx.env().consensus_time` (never wall clock), so the sha1 oid
//!     — and thus sha256(oid) — is byte-identical across nodes on the same inputs.
//!   - the object format is a NETWORK-WIDE GENESIS CONSTANT: every validator MUST
//!     use the identical format. a sha1 node and a sha256 node compute different
//!     roots for the same state and FORK. it is NOT a per-node choice.
//!
//! ============================================================================
//!
//! forge — a GIT-backed feature module.
//!
//! where the directory module keeps a `BTreeMap` and kv keeps a qmdb, forge's
//! private substrate is a real on-disk git repository, driven through VENDORED
//! libgit2 (`git2-rs`) — no `git` subprocess, so the node binary is self-
//! contained. the repo is git's DEFAULT sha1 object format, so a HEAD oid is 20
//! bytes; forge's authenticated [`StateRoot`] is `sha256(<HEAD oid bytes>)`, a
//! 32-byte commitment that composes into the global app-hash next to a qmdb
//! merkle root with zero special-casing. (unborn repo -> [`StateRoot::ZERO`].)
//!
//! ## the determinism landmine (single-node slice)
//!
//! a git *commit* embeds committer identity + a timestamp, so two nodes
//! committing the same content would normally get DIFFERENT commit oids — and
//! the app-hash would fork. this slice keeps the commit reproducible anyway:
//!
//! - `root()` is `sha256` of the repo's current HEAD sha1 oid, a 32-byte
//!   [`StateRoot`];
//! - the commit object uses a FIXED author/committer identity (`ducktape`, via a
//!   typed `git2::Signature`) and a date derived from `ctx.env().consensus_time`
//!   (NOT wall clock, offset +0000), set for BOTH author and committer — so the
//!   sha1 oid is byte-identical across independent repos given the same inputs;
//! - the tree is built in-memory with a `git2::TreeBuilder` seeded from the
//!   parent tree, so it's a pure function of (parent, change) — no on-disk index,
//!   no worktree, nothing for host cruft to leak through.
//!
//! git2 is used precisely because it BYPASSES the host-config traps porcelain
//! would inherit: `commit.gpgsign` never fires (libgit2's `commit` doesn't sign;
//! signing is a separate call), `core.autocrlf` never mangles blob bytes
//! (`repo.blob` writes the buffer verbatim — filters run only on worktree
//! checkout, which forge never does), and the fixed `Signature` overrides
//! `user.name`/`user.email`. no worktree is ever materialized (nothing reads
//! it); the HEAD tree is authoritative.
//!
//! ### the host-lent staging seam
//!
//! forge follows the host-lent STAGING pattern. `execute` builds the commit
//! object with `repo.commit(None, ...)` — the `None` update_ref means the object
//! lands in the odb but NO ref moves, so `root()` (which reads the committed
//! ref) is unchanged. `commit_block` publishes it by moving the ref
//! (`repo.reference(force)`) and refreshing the committed mirror; `abort_block`
//! drops the staged oid and the built commit objects linger unreferenced in the
//! odb (node-local, never in `root()`/the app-hash). loose objects flush to disk
//! immediately, so a per-call `Repository` opened in `commit_block` reads back
//! the object a different `Repository` staged in `execute`.
//!
//! ### deferred to the p2p port (faithful multi-node)
//!
//! true cross-node convergence is "results on the wire, not commands": the wire
//! fact is a `RefUpdate { name, target_oid, prev }` applied via `repo.reference`
//! on receivers, which NEVER build a commit. only the origin commits locally
//! (the `execute` below). the pinned identity/date keeps the origin's oid
//! reproducible as a backstop, but RefUpdate stays canonical because bit-
//! identical commit encoding across git builds isn't guaranteed. that split
//! (origin commits + emits RefUpdate; receivers fetch closure + update-ref) is
//! out of scope for this single-node demo.

mod git;

use std::path::PathBuf;

use forge_interface::{ForgeMsg, ForgeQuery, ForgeReply, decode_msg, decode_query, encode_reply};
use git2::Oid;
use sdk::{Ctx, Error, Module, ModuleId, Msg, StateRoot, StateSyncHandle};
use sha2::{Digest, Sha256};

/// the canonical branch this module commits to and reads HEAD from.
const MAIN_REF: &str = "refs/heads/main";

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

pub struct Forge {
    id: ModuleId,
    /// node-local repo dir — NOT consensus state (the path may differ per node);
    /// only the HEAD oid it produces is. repos are opened per-call off this path
    /// rather than held open, so no `git2` borrow outlives a method.
    repo: PathBuf,
    /// write-through mirror of the COMMITTED `MAIN_REF`: refreshed at genesis and
    /// by `commit_block`, read by `root()`. the repo/ref is the source of truth
    /// for the committed parent; this cache never feeds a commit's parent. `None`
    /// == unborn repo.
    head: Option<Oid>,
    /// the head STAGED this block: for a `Commit`, `execute` builds the commit
    /// object and points this at it WITHOUT moving the ref; for a `Push`, it is
    /// the pushed `new_oid` (its objects live off-repo, in a node-local pack).
    /// later ops read-your-writes via `query`; `commit_block` publishes it,
    /// `abort_block` drops it. `None` == nothing staged. NOT in `root()` until
    /// committed.
    staged: Option<Oid>,
    /// the node-local body store Push packfiles are fetched from by digest —
    /// the SAME plane the files module serves. shared here only so a committed
    /// Push head can be materialized onto the on-disk repo (`materialize`); it
    /// is NEVER read by `root()`/`execute`/`commit_block` and so cannot affect
    /// consensus. a Commit-only deployment can pass a default (unused) handle.
    blobs: files::BlobHandle,
    /// the pack digest that pairs with a `Push`-`staged` head (32 raw bytes).
    /// `Some` ONLY for a staged Push — a Commit builds its objects straight into
    /// the local odb, so there is nothing to fetch. `commit_block` promotes this
    /// into `pending_pack`; `abort_block` drops it. node-local, never in root().
    staged_pack: Option<[u8; 32]>,
    /// node-local catch-up target: a committed Push head whose objects are not
    /// yet installed on the on-disk `MAIN_REF`, plus the pack digest to fetch
    /// them by. `materialize` clears it once the ref is moved. this is the whole
    /// point of the decoupling — `root()` already reflects this head (it is
    /// `self.head`) while the on-disk repo catches up lazily. NOT in `root()`.
    pending_pack: Option<(Oid, [u8; 32])>,
    /// one-shot guard so a not-yet-fetched (or invalid) pack logs ONCE per
    /// pending target instead of on every opportunistic `materialize` retry.
    materialize_warned: bool,
}

impl Forge {
    /// genesis wiring: init (or adopt) a git repo at `repo_dir`, then seed the
    /// cached head from its current `MAIN_REF` (`None` on a fresh empty repo, so
    /// `root()` starts at [`StateRoot::ZERO`]). deterministic given the dir state.
    /// this convenience overload gives forge a private, default (empty) blob
    /// store — enough for a `Commit`-only or test deployment. a node that wants
    /// `Push` materialization to reuse the files body plane must build forge over
    /// that shared handle with [`Forge::with_blobs`].
    pub fn init(id: impl Into<ModuleId>, repo_dir: impl Into<PathBuf>) -> Result<Self, Error> {
        Self::with_blobs(id, repo_dir, files::BlobHandle::default())
    }

    /// genesis wiring over an EXISTING node-local blob store — mirrors
    /// [`files::Files::with_blobs`]. the embedding daemon creates one handle,
    /// registers the files module over it (uploads land there), and builds forge
    /// over a clone so a `Push`'s packfile — uploaded before the op is submitted
    /// — is visible to [`Forge::materialize`] without a byte ever crossing
    /// consensus. the handle feeds ONLY node-local materialization; `root()` is
    /// still `sha256(<committed head oid>)` and never touches the store.
    pub fn with_blobs(
        id: impl Into<ModuleId>,
        repo_dir: impl Into<PathBuf>,
        blobs: files::BlobHandle,
    ) -> Result<Self, Error> {
        let repo_dir = repo_dir.into();
        let repo = if repo_dir.join(".git").exists() {
            git::open(&repo_dir).map_err(|e| Error::Module(e.to_string()))?
        } else {
            git::init(&repo_dir).map_err(|e| Error::Module(e.to_string()))?
        };
        let head = git::resolve_ref(&repo, MAIN_REF).map_err(|e| Error::Module(e.to_string()))?;
        Ok(Self {
            id: id.into(),
            repo: repo_dir,
            head,
            staged: None,
            blobs,
            staged_pack: None,
            pending_pack: None,
            materialize_warned: false,
        })
    }

    /// map a HEAD sha1 oid into the fixed-width state root: the 20 raw oid bytes
    /// are the sha256 PREIMAGE, so `root() = sha256(oid_bytes)`. infallible.
    fn oid_to_root(oid: Oid) -> StateRoot {
        let mut h = Sha256::new();
        h.update(oid.as_bytes()); // 20 raw sha1 bytes
        StateRoot(h.finalize().into())
    }

    // ---- state-sync ---------------------------------------------------------
    // a snapshot is SELF-CONTAINED BYTES — the 20-byte committed head oid, then
    // a packfile carrying the head's FULL object closure — so it can ride a
    // bulk data channel between nodes that share nothing (no common filesystem,
    // no remote, no `git` binary). the oid prefix binds the pack to a state
    // root (root() = sha256(oid)), which is what lets install verify against
    // an expected root before a single byte touches the odb.

    /// serialize the COMMITTED state into self-contained snapshot bytes: the
    /// 20-byte head sha1 oid, then a packfile of every object reachable from
    /// it. a staged (this-block) head is deliberately excluded — a snapshot
    /// must reproduce `root()`, and `root()` covers only the committed ref.
    /// an unborn repo serializes as 20 zero bytes and NO pack: the marker for
    /// the [`StateRoot::ZERO`] state (a zero oid names no git object, so the
    /// encoding is unambiguous).
    pub fn snapshot(&self) -> Result<Vec<u8>, Error> {
        let Some(head) = self.head else {
            return Ok(vec![0u8; git::OID_RAW_LEN]);
        };
        let repo = git::open(&self.repo).map_err(|e| Error::Module(e.to_string()))?;
        let pack = git::pack_closure(&repo, head).map_err(|e| Error::Module(e.to_string()))?;
        let mut bytes = Vec::with_capacity(git::OID_RAW_LEN + pack.len());
        bytes.extend_from_slice(head.as_bytes());
        bytes.extend_from_slice(&pack);
        Ok(bytes)
    }

    /// replace this module's state with snapshot bytes, gated on `expected`.
    /// the bytes are UNTRUSTED (a byzantine peer produced them), so the order
    /// is verify-then-mutate: length-check, parse the oid, require it to
    /// rehash to `expected` through the same mapping `root()` uses — all
    /// BEFORE any write; then index the pack (libgit2 re-hashes every object
    /// and the pack trailer), require the head commit and its root tree to
    /// actually parse out of the odb, and only then move the ref. on any Err
    /// the committed ref — and so `root()` — is byte-identical to before the
    /// call (a failed pack can at most strand unreferenced objects in the
    /// odb: node-local, never authenticated). no worktree is ever
    /// materialized, so there is nothing to reset. on Ok any staged head is
    /// dropped — install is a full state replacement, not a merge.
    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        // untrusted input: bound the read before parsing anything.
        if bytes.len() < git::OID_RAW_LEN {
            return Err(Error::Module(format!(
                "snapshot truncated: {} bytes, oid header needs {}",
                bytes.len(),
                git::OID_RAW_LEN
            )));
        }
        let (oid_bytes, pack) = bytes.split_at(git::OID_RAW_LEN);
        let oid = Oid::from_bytes(oid_bytes).map_err(|e| Error::Module(e.to_string()))?;

        // the empty marker: a zero oid names no object, so it encodes the
        // unborn state and must carry nothing after the header. NB it binds
        // to StateRoot::ZERO — the same None -> ZERO mapping root() uses —
        // NOT to sha256(<zero oid>).
        if oid.is_zero() {
            if !pack.is_empty() {
                return Err(Error::Module(
                    "empty-state snapshot carries trailing bytes".into(),
                ));
            }
            if expected != StateRoot::ZERO {
                return Err(Error::Module(
                    "snapshot root mismatch: empty state, non-ZERO expected".into(),
                ));
            }
            let repo = git::open(&self.repo).map_err(|e| Error::Module(e.to_string()))?;
            git::delete_ref(&repo, MAIN_REF).map_err(|e| Error::Module(e.to_string()))?;
            self.head = None;
            self.staged = None;
            self.clear_pending_materialization();
            return Ok(());
        }

        // the root binding, verified BEFORE any byte reaches the odb.
        if Self::oid_to_root(oid) != expected {
            return Err(Error::Module(
                "snapshot root mismatch: head oid does not rehash to expected root".into(),
            ));
        }

        // index the pack: every object is hash-verified as it lands, so a
        // tampered pack dies here with the ref unmoved. the pack is the
        // terminal field — its own trailer checksum delimits it.
        let repo = git::open(&self.repo).map_err(|e| Error::Module(e.to_string()))?;
        git::install_pack(&repo, pack).map_err(|e| Error::Module(e.to_string()))?;

        // the pack verified per-object, but nothing yet says it CONTAINS the
        // head — or the head's FULL closure: libgit2 indexes a partial pack
        // fine (per-object hashes, no connectivity), so a byzantine snapshot
        // could carry the genuine head commit and omit the blobs/trees/parents
        // it references. walk the closure and require every reachable object
        // before publishing.
        git::verify_closure(&repo, oid).map_err(|e| Error::Module(e.to_string()))?;

        // fully verified — publish with the same ref move commit_block uses,
        // then refresh the committed mirror so root() reflects it.
        git::update_ref(&repo, MAIN_REF, oid).map_err(|e| Error::Module(e.to_string()))?;
        self.head = Some(oid);
        self.staged = None;
        // install ships the head's full closure and moves the ref in one shot,
        // so the on-disk repo is already current: any pending Push catch-up is
        // moot and must be dropped (its digest belonged to a superseded head).
        self.clear_pending_materialization();
        Ok(())
    }

    /// forget any node-local Push catch-up target. called wherever the on-disk
    /// ref is authoritatively resynced (install / a completed materialize) so a
    /// stale `pending_pack` can't later stomp a newer head. never touches
    /// `head`/`root()`.
    fn clear_pending_materialization(&mut self) {
        self.pending_pack = None;
        self.materialize_warned = false;
    }

    /// node-local catch-up (NON-consensus): if the on-disk `MAIN_REF` lags the
    /// committed Push head, fetch the head's packfile from the blob store by its
    /// recorded digest, install it (libgit2 re-hashes every object), require the
    /// FULL closure to be present, confirm the head fast-forwards the prior ref,
    /// then move the ref. it NEVER reads or writes `head`/`root()` — pack
    /// possession is per-node, so a not-yet-fetched, corrupt, or non-fast-forward
    /// pack is a SAFE no-op that leaves the ref behind (root already reflects the
    /// committed head) and warns once; a later call retries. only a genuine repo
    /// I/O failure surfaces as `Err`. idempotent, and a no-op when nothing is
    /// pending.
    ///
    /// the SUBMITTER holds the pack locally (uploaded before submit) so its
    /// `commit_block` materializes immediately; other nodes catch up when they
    /// fetch the pack (or, for a fresh joiner, via the state-sync snapshot, which
    /// already carries the closure through `install`). driving p2p pack fetch is
    /// out of scope here — this makes the local-pack path work and the
    /// missing-pack path safe.
    pub fn materialize(&mut self) -> Result<(), Error> {
        let Some((head, digest)) = self.pending_pack else {
            return Ok(()); // nothing pending — the common Commit / caught-up case
        };
        let repo = git::open(&self.repo).map_err(|e| Error::Module(e.to_string()))?;

        // already caught up (e.g. the snapshot install moved the ref)? drop the
        // pending target and stop.
        let prior = git::resolve_ref(&repo, MAIN_REF).map_err(|e| Error::Module(e.to_string()))?;
        if prior == Some(head) {
            self.clear_pending_materialization();
            return Ok(());
        }

        // fetch the packfile from the node-local body store. absent == not
        // fetched yet: leave the ref behind (root is already correct), warn once.
        let Some(pack) = self.blobs.get_chunk(&digest) else {
            self.warn_materialize(format!(
                "pack {} for head {head} not in the blob store yet; on-disk ref \
                 stays behind, root already reflects the committed head",
                hex(&digest)
            ));
            return Ok(());
        };

        // a fetched pack that fails to install / complete the closure / fast-
        // forward is treated exactly like an absent one: a safe no-op that keeps
        // root correct. it is NODE-LOCAL and NEVER gates consensus.
        if let Err(why) = Self::install_and_advance(&repo, head, prior, &pack) {
            self.warn_materialize(format!(
                "cannot advance on-disk ref to head {head}: {why}; leaving ref \
                 behind (root already correct)"
            ));
            return Ok(());
        }
        self.clear_pending_materialization();
        Ok(())
    }

    /// the pure git side of one materialize attempt: install the pack, require
    /// the full closure of `head`, refuse a non-fast-forward onto a born
    /// `prior` ref, then move `MAIN_REF` to `head`. any failure is returned so
    /// the caller can turn it into a safe no-op.
    fn install_and_advance(
        repo: &git2::Repository,
        head: Oid,
        prior: Option<Oid>,
        pack: &[u8],
    ) -> Result<(), Error> {
        // install re-hashes every object; verify_closure then requires the head
        // commit AND its whole tree/parent closure — a partial pack dies here
        // before the ref moves.
        git::install_pack(repo, pack).map_err(|e| Error::Module(e.to_string()))?;
        git::verify_closure(repo, head).map_err(|e| Error::Module(e.to_string()))?;

        // a born ref may only fast-forward: a normal push builds on the prior
        // head. an unborn prior is the first push and is always allowed. (this
        // is a LOCAL sanity gate — consensus already CAS'd prev_oid; a force
        // push, out of scope here, would legitimately fail this and leave the
        // ref behind with root still correct.)
        if let Some(prior) = prior {
            let ff =
                git::is_descendant(repo, head, prior).map_err(|e| Error::Module(e.to_string()))?;
            if !ff {
                return Err(Error::Module(format!(
                    "head does not fast-forward on-disk ref {prior}"
                )));
            }
        }

        git::update_ref(repo, MAIN_REF, head).map_err(|e| Error::Module(e.to_string()))?;
        Ok(())
    }

    /// warn ONCE per pending target (reset when the target changes or clears).
    fn warn_materialize(&mut self, msg: String) {
        if !self.materialize_warned {
            eprintln!("[forge] materialize: {msg}");
            self.materialize_warned = true;
        }
    }

    /// stage one `Commit`: build the deterministic commit object over the parent
    /// tree and point `staged` at it WITHOUT moving the ref (the host publishes
    /// at the block boundary). unchanged from the file-by-file commit path.
    fn stage_commit(
        &mut self,
        ctx: &mut dyn Ctx,
        path: String,
        content: String,
        message: String,
    ) -> Result<(), Error> {
        let repo = git::open(&self.repo).map_err(|e| Error::Module(e.to_string()))?;

        // 1. parent := the STAGED head if this block already committed here,
        //    else the REPO's current (committed) head. chaining on the staged
        //    head gives multi-commit-in-one-block the correct parent.
        let parent_oid = match self.staged {
            Some(oid) => Some(oid),
            None => git::resolve_ref(&repo, MAIN_REF).map_err(|e| Error::Module(e.to_string()))?,
        };
        let parent_commit = parent_oid
            .map(|oid| repo.find_commit(oid))
            .transpose()
            .map_err(|e| Error::Module(e.to_string()))?;

        // 2. write the blob to the odb and build the tree in-memory (seeded from
        //    the parent's tree = incremental). no on-disk index, no worktree.
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
        let ts = ctx.env().consensus_time;
        let commit = git::commit(&repo, &tree, parent_commit.as_ref(), &message, ts)
            .map_err(|e| Error::Module(e.to_string()))?;

        // 4. STAGE the new head — do NOT move the ref. the host publishes it at
        //    the block boundary (`commit_block` -> `update_ref`); on abort the ref
        //    never moves and these commit objects stay orphaned in the odb (node-
        //    local, not authenticated state — no trace in `root()`/app-hash). the
        //    objects are already in this odb, so `staged_pack` stays `None`:
        //    commit_block moves the ref directly, nothing to fetch.
        self.staged = Some(commit);
        self.staged_pack = None;
        Ok(())
    }

    /// stage one `Push`: the git-faithful ref update. PURE and deterministic —
    /// the only gate is a compare-and-swap on the COMMITTED head, fully
    /// determined by consensus state, so accept/reject and the resulting `root()`
    /// are identical on every validator whether or not it holds the packfile.
    /// no repo is opened, nothing is installed, no ref moves here.
    fn stage_push(
        &mut self,
        prev_oid: Option<Vec<u8>>,
        new_oid: Vec<u8>,
        pack_digest: Vec<u8>,
    ) -> Result<(), Error> {
        // 1. length-validate the untrusted wire fields (deterministic on every
        //    validator: same op bytes -> same decision). a 20-byte sha1 oid, an
        //    optional 20-byte prev, a 32-byte pack sha256.
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

        // 2. CAS on the COMMITTED head (never the staged one): this is the SOLE
        //    consensus gate and it reads only agreed state. `None` prev must
        //    match an unborn repo. a mismatch is a non-fast-forward — the client
        //    raced another push and must fetch + retry.
        if self.head != prev {
            return Err(Error::Module(
                "non-fast-forward: forge HEAD moved; fetch and retry".into(),
            ));
        }

        // 3. stage the new head + remember its pack digest so commit_block can
        //    record the node-local materialization target. `root()` still
        //    reflects only the committed head until commit_block publishes this.
        self.staged = Some(new);
        self.staged_pack = Some(digest);
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Forge {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// the repo's HEAD commit oid as a [`StateRoot`] (`sha256` of its 20 sha1
    /// bytes) — pure, no IO (that's the whole reason `head` is a write-through
    /// cache). `None` -> `ZERO`.
    fn root(&self) -> StateRoot {
        self.head.map_or(StateRoot::ZERO, Self::oid_to_root)
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(self.snapshot()?))
    }

    /// apply one write op. a `Commit` builds a deterministic commit object in the
    /// local odb (fixed `Signature` + `consensus_time` date + in-memory tree, so
    /// the sha1 oid is reproducible) and stages it. a `Push` is PURE: it does a
    /// compare-and-swap on the committed HEAD and stages the new oid — no git IO,
    /// no pack install, no ref move, so it stays fast and, crucially, its accept/
    /// reject and resulting `root()` are identical on every validator regardless
    /// of pack possession (materialization is deferred; see `commit_block` ->
    /// `materialize`). all git2 IO is blocking with no `.await`, so the "await
    /// only deterministic resources" rule holds vacuously.
    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            ForgeMsg::Commit {
                path,
                content,
                message,
            } => self.stage_commit(ctx, path, content, message),
            ForgeMsg::Push {
                prev_oid,
                new_oid,
                pack_digest,
            } => self.stage_push(prev_oid, new_oid, pack_digest),
        }
    }

    /// read projection: the current HEAD as hex (or `None` on an unborn repo).
    /// served straight from the cached mirror — no IO, no `.await`. this is the
    /// raw 40-char sha1 oid hex; `root()` is `sha256` of its 20 bytes, so the hex
    /// is the state root's PREIMAGE, not its rendering.
    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            // read-your-writes: a staged (this-block) head shadows the committed
            // one; `root()` still reflects only the committed head.
            ForgeQuery::Head => Ok(encode_reply(&ForgeReply::Head(
                self.staged.or(self.head).map(|oid| oid.to_string()),
            ))),
        }
    }

    /// publish the staged head so `root()` now reflects it. for a `Commit` the
    /// objects are already in the local odb, so the ref moves here directly (as
    /// before). for a `Push` the objects live in a node-local pack this node may
    /// not hold yet: publishing must NOT depend on the pack, or validators would
    /// diverge — so it records a node-local materialization target and defers the
    /// on-disk ref move to `materialize`, which it then invokes opportunistically
    /// (the submitter, holding the pack, catches up immediately; a node lacking
    /// the pack is a safe no-op with `root()` already correct). no-op if nothing
    /// was staged this block.
    async fn commit_block(&mut self) -> Result<(), Error> {
        let Some(oid) = self.staged.take() else {
            return Ok(());
        };
        match self.staged_pack.take() {
            None => {
                // Commit: the commit object is in this odb; move the ref now.
                let repo = git::open(&self.repo).map_err(|e| Error::Module(e.to_string()))?;
                git::update_ref(&repo, MAIN_REF, oid).map_err(|e| Error::Module(e.to_string()))?;
                self.head = Some(oid);
            }
            Some(digest) => {
                // Push: publish the head for `root()` unconditionally (the
                // determinism invariant), then try to materialize the on-disk ref
                // from the local pack. a fresh target -> re-arm the one-shot warn.
                self.head = Some(oid);
                self.pending_pack = Some((oid, digest));
                self.materialize_warned = false;
                self.materialize()?;
            }
        }
        Ok(())
    }

    /// discard the staged head — the ref was never moved, so `root()` is
    /// unchanged; any built commit objects linger unreferenced in the odb, and a
    /// staged Push's pack digest is dropped with it (a committed Push's pending
    /// materialization target is untouched — it belongs to an already-published
    /// head, not this aborted block).
    async fn abort_block(&mut self) -> Result<(), Error> {
        self.staged = None;
        self.staged_pack = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_interface::{decode_reply, encode_msg, encode_query};

    // a minimal Ctx so execute can read consensus_time without a full host.
    struct TestCtx {
        env: sdk::Env,
    }
    impl TestCtx {
        fn at(consensus_time: u64) -> Self {
            Self {
                env: sdk::Env {
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

    fn tmp_repo(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("ducktape-forge-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    fn commit_msg(path: &str, content: &str, message: &str) -> Msg {
        Msg {
            target: "forge".into(),
            payload: encode_msg(&ForgeMsg::Commit {
                path: path.into(),
                content: content.into(),
                message: message.into(),
            }),
        }
    }

    // read HEAD oid via git2 directly (opening the on-disk repo) — the
    // independent oracle that root() really tracks the repo's HEAD ref, not just
    // "some 32 bytes that moved". independent of forge's `head` cache. no `git`
    // binary involved (that's the whole point of vendoring libgit2).
    fn git_head_oid(repo: &PathBuf) -> Oid {
        git2::Repository::open(repo)
            .unwrap()
            .refname_to_id(MAIN_REF)
            .unwrap()
    }

    #[test]
    fn genesis_is_zero_then_commit_makes_root_equal_head() {
        let dir = tmp_repo("basic");
        let mut forge = Forge::init("forge", dir.clone()).unwrap();
        assert_eq!(forge.root(), StateRoot::ZERO, "unborn repo -> ZERO root");

        futures::executor::block_on(forge.execute(
            &mut TestCtx::at(100),
            &commit_msg("a.txt", "hello", "first"),
        ))
        .unwrap();
        futures::executor::block_on(forge.commit_block()).unwrap();

        assert_ne!(
            forge.root(),
            StateRoot::ZERO,
            "a commit must move the root off ZERO"
        );

        // root() == sha256(HEAD oid) — tracks the real ref (not the cache).
        let head_oid = git_head_oid(&dir);
        assert_eq!(
            forge.root(),
            Forge::oid_to_root(head_oid),
            "root() must equal sha256 of the real git HEAD oid"
        );

        // and query(Head) surfaces that same oid hex (the root's preimage).
        let reply =
            futures::executor::block_on(forge.query(&encode_query(&ForgeQuery::Head))).unwrap();
        assert_eq!(
            decode_reply(&reply).unwrap(),
            ForgeReply::Head(Some(head_oid.to_string()))
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn second_commit_moves_the_root() {
        let dir = tmp_repo("second");
        let mut forge = Forge::init("forge", dir.clone()).unwrap();
        futures::executor::block_on(
            forge.execute(&mut TestCtx::at(1), &commit_msg("a.txt", "one", "c1")),
        )
        .unwrap();
        futures::executor::block_on(forge.commit_block()).unwrap();
        let r1 = forge.root();
        futures::executor::block_on(
            forge.execute(&mut TestCtx::at(2), &commit_msg("b.txt", "two", "c2")),
        )
        .unwrap();
        futures::executor::block_on(forge.commit_block()).unwrap();
        let r2 = forge.root();
        assert_ne!(r1, r2, "a second commit must advance the root");
        assert_eq!(r2, Forge::oid_to_root(git_head_oid(&dir)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn root_composes_into_global_root() {
        let dir = tmp_repo("compose");
        let mut forge = Forge::init("forge", dir.clone()).unwrap();
        let before = state::global_root(&[&forge as &dyn Module]);
        futures::executor::block_on(
            forge.execute(&mut TestCtx::at(7), &commit_msg("a.txt", "x", "c")),
        )
        .unwrap();
        futures::executor::block_on(forge.commit_block()).unwrap();
        let after = state::global_root(&[&forge as &dyn Module]);
        assert_ne!(
            before, after,
            "forge's git-backed root must move the global app-hash"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    // determinism: two independent repos, same inputs -> identical HEAD oid ->
    // identical sha256(oid) root. the pinned Signature (fixed ducktape identity +
    // Time::new(555, 0)) is what makes the commit bytes — and thus the sha1 oid —
    // byte-identical; the per-repo odb path never enters the commit bytes.
    #[test]
    fn commit_oid_is_reproducible_across_repos() {
        let da = tmp_repo("det-a");
        let db = tmp_repo("det-b");
        let mut fa = Forge::init("forge", da.clone()).unwrap();
        let mut fb = Forge::init("forge", db.clone()).unwrap();
        futures::executor::block_on(fa.execute(
            &mut TestCtx::at(555),
            &commit_msg("f.txt", "same", "same-msg"),
        ))
        .unwrap();
        futures::executor::block_on(fa.commit_block()).unwrap();
        futures::executor::block_on(fb.execute(
            &mut TestCtx::at(555),
            &commit_msg("f.txt", "same", "same-msg"),
        ))
        .unwrap();
        futures::executor::block_on(fb.commit_block()).unwrap();
        assert_eq!(
            fa.root(),
            fb.root(),
            "pinned identity+date -> reproducible commit oid"
        );
        let _ = std::fs::remove_dir_all(&da);
        let _ = std::fs::remove_dir_all(&db);
    }
}
