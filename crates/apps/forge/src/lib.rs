//! ============================================================================
//! STORAGE SUBSTRATE — DECIDED DIRECTION (2026-07-01)
//!   git2-rs (vendored libgit2) + sha1;  root() = sha256 over per-repo refs
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
//!   branch oids under sha256 makes forge's contribution to the global app-hash
//!   sha256-STRENGTH. the only residual sha1 surface is a *forge-object*
//!   collision (two trees under one commit oid) — expensive and SHA-1DC-guarded —
//!   while the app-hash's collision resistance at the STATE layer stays sha256.
//!   (no committed branch and no tracker item anywhere -> StateRoot::ZERO.)
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
//! forge — a GIT-backed feature module: a NAMED NAMESPACE of multi-branch repos
//! plus a GitHub-shaped issue/PR TRACKER (see [`tracker`]).
//!
//! ## the load-bearing composition invariant
//!
//! forge's authenticated [`StateRoot`] is a CANONICAL SORTED HASH over every
//! born branch of every repo, folded with the tracker's canonical bytes and
//! domain-separated under `FORGE_ROOT_DOMAIN`:
//!
//! ```text
//! root = sha256( FORGE_ROOT_DOMAIN ++ sha256(
//!                 for each repo sorted-by-name with >=1 born branch:
//!                     u32-LE(name.len) ++ name ++ u32-LE(ref_count) ++
//!                     for each (branch, head) sorted-by-branch:
//!                         u32-LE(branch.len) ++ branch ++ head.oid[20]
//!                 ++ if tracker non-empty:
//!                     TRACKER_DOMAIN ++ sha256(tracker.canonical_bytes()) ) )
//! ```
//!
//! with `root() == StateRoot::ZERO` on the empty state (the empty-genesis root).
//! this is a PURE FUNCTION of the committed consensus state — sorted, order-
//! independent, identical on every validator REGARDLESS of pack possession.
//! a branch head advances on every validator the instant its push CASes; the
//! objects catch up node-locally (see [`refs::RepoState::materialize`]) and
//! NEVER enter root/accept-reject. `main` and the shared `dev` integration
//! branch are protected (never deleted, fast-forward-guarded at materialize);
//! feature branches may force-push and be deleted — the GitHub flow.
//!
//! ## back-compat: the default repo (no app change)
//!
//! every [`ForgeMsg`] carries a `#[serde(default)] repo`, so a legacy wire
//! message with no `repo` deserializes with `repo == ""`; the module
//! normalizes an empty repo to the well-known `"default"` repo. the unit
//! [`ForgeQuery::Head`] answers the default repo's `main` head.
//!
//! ## the determinism landmine (per repo)
//!
//! a git *commit* embeds committer identity + a timestamp, so each repo keeps
//! its commit reproducible: a FIXED author/committer identity (`ducktape`) and
//! a date derived from `ctx.env().consensus_time`, so the sha1 oid is byte-
//! identical across independent repos given the same inputs (see [`git`]).
//!
//! KNOWN PRE-EXISTING HAZARD (unchanged by the multi-branch work): a `Commit`
//! op builds on the parent COMMIT OBJECT, which only exists in odbs that have
//! materialized the history — mixing `Commit` and `PushRefs` on one repo can make
//! `Commit` fail on validators that still lack the pushed pack. the app commits
//! to app-managed repos and git users push to git-managed repos, so the mix
//! does not occur in practice; a consensus-visible "pushed" flag is the proper
//! fix if it ever must.
//!
//! ## the host-lent staging seam (per repo + tracker)
//!
//! `execute` stages every change WITHOUT moving refs or the committed tracker
//! (`root()` reads committed state only); `commit_block` publishes staged
//! branches (or records node-local materialization targets) and swaps the
//! staged tracker in (persisting `<base>/.tracker.bin`); `abort_block` drops
//! everything staged.

// the wire surface: this module's shared types, flattened at the crate root.
mod interface;
pub use interface::*;
mod tracker_iface;
pub use tracker_iface::*;

mod codec;
mod git;
/// the multi-head pack builders, shared with bin/noded's git upload-pack
/// (fetch/clone) lane — packing has ONE implementation on both surfaces.
/// `pack_closure_many` is the self-contained closure; `pack_delta` bounds it
/// by the client's common bases (the incremental fetch answer).
pub use git::{pack_closure_many, pack_delta};
pub mod refs;
mod snapshot;
pub mod tracker;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use git2::Oid;
use sdk::{Ctx, Error, Module, ModuleId, Msg, StateRoot, StateSyncHandle};
use sha2::{Digest, Sha256};

use crate::refs::{
    norm_branch, open_or_init_repo, RepoState, INTEGRATION_BRANCH, MAIN_BRANCH,
};
use crate::tracker::{author_from_origin, parse_hex_oid, Tracker};

/// the well-known repo an empty/absent `repo` field maps to — the target of the
/// legacy single-repo wire (see the module docstring).
const DEFAULT_REPO: &str = "default";

/// the max repo-name length in bytes (names are a filesystem path segment and a
/// consensus-visible key, so they are bounded).
const MAX_REPO_NAME_LEN: usize = 64;

/// the node-local file the committed tracker persists to under `base` —
/// canonical bytes, rewritten atomically at every mutating `commit_block`,
/// re-adopted at construction (the tracker analogue of the on-disk git refs).
/// never a valid repo dir name (repos are directories; this is a file).
const TRACKER_FILE: &str = ".tracker.bin";

/// the domain tag folding the tracker's canonical-bytes hash into the root
/// preimage — separates it from the branch material.
const TRACKER_ROOT_DOMAIN: &[u8] = b"ducktape.forge.tracker.v1\x00";

/// normalize + validate a repo slug DETERMINISTICALLY (same input -> same
/// decision on every validator, so it is safe as a consensus gate): empty ->
/// `"default"`; otherwise it must be 1..=`MAX_REPO_NAME_LEN` bytes of
/// `[a-z0-9._-]` and never `.`/`..` (those would escape or collide with the base
/// dir as a path segment). a valid non-empty slug returns unchanged, so the map
/// key equals the on-disk directory name. `pub`: bin/noded's git smart-HTTP
/// layer shares this validator — the security-relevant check has ONE home.
pub fn norm_repo(repo: &str) -> Result<String, Error> {
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

/// the domain tag forge's root preimage is separated under — a fixed constant
/// hashed over the folded preimage in [`compose_state_root`]. (historical note:
/// the `.v2` in the bytes is the retired dual-path era's version tag, kept
/// verbatim so the constant is self-describing on the wire.)
const FORGE_ROOT_DOMAIN: &[u8] = b"ducktape.forge.multirepo.v2\x00";

/// the 4-byte magic every forge snapshot container leads with.
pub(crate) const FORGE_SNAPSHOT_MAGIC: &[u8; 4] = b"FGv2";

/// parse exactly `OID_RAW_LEN` (20) raw sha1 bytes into an `Oid`, with a
/// deterministic module error on any other length.
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

/// parse a 32-byte pack digest from raw wire bytes.
fn parse_digest(bytes: &[u8]) -> Result<[u8; 32], Error> {
    bytes.try_into().map_err(|_| {
        Error::Module(format!(
            "forge: pack_digest must be 32 bytes, got {}",
            bytes.len()
        ))
    })
}

/// parse a 64-char sha256 hex digest (the app-facing MergePr lane).
fn parse_hex_digest(s: &str) -> Result<[u8; 32], Error> {
    if s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::Module(
            "forge: pack_digest must be 64 hex chars".into(),
        ));
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16)
            .map_err(|e| Error::Module(e.to_string()))?;
    }
    Ok(out)
}

/// lowercase-hex a byte slice — for human-readable log lines only.
pub(crate) fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// the composition [`StateRoot`] over the whole forge state: every born branch
/// of every repo (callers pass repos SORTED by name; branch maps are sorted
/// `BTreeMap`s) folded with the tracker's canonical-bytes hash, then
/// domain-separated under [`FORGE_ROOT_DOMAIN`]. the empty state ->
/// [`StateRoot::ZERO`] (the empty-genesis root). see the composition invariant
/// in the module doc.
pub(crate) fn compose_state_root<'a>(
    repos: impl Iterator<Item = (&'a str, &'a BTreeMap<String, Oid>)>,
    tracker: &Tracker,
) -> StateRoot {
    let mut h = Sha256::new();
    let mut any = false;
    for (name, refs) in repos {
        if refs.is_empty() {
            continue;
        }
        any = true;
        // name/branch lengths are cap-bounded (64 / 128 bytes), so the u32
        // casts never truncate.
        h.update((name.len() as u32).to_le_bytes());
        h.update(name.as_bytes());
        h.update((refs.len() as u32).to_le_bytes());
        for (branch, head) in refs {
            h.update((branch.len() as u32).to_le_bytes());
            h.update(branch.as_bytes());
            h.update(head.as_bytes()); // 20 raw sha1 bytes
        }
    }
    if !tracker.is_empty() {
        any = true;
        h.update(TRACKER_ROOT_DOMAIN);
        h.update(Sha256::digest(tracker.canonical_bytes()));
    }
    if !any {
        return StateRoot::ZERO;
    }
    let inner: [u8; 32] = h.finalize().into();
    let mut outer = Sha256::new();
    outer.update(FORGE_ROOT_DOMAIN);
    outer.update(inner);
    StateRoot(outer.finalize().into())
}

pub struct Forge {
    id: ModuleId,
    /// node-local container dir — NOT consensus state (the path may differ per
    /// node); only the committed state under it is. each named repo lives at
    /// `base/<name>` and is opened per-call.
    pub(crate) base: PathBuf,
    /// the node-local body store push packfiles are fetched from by digest —
    /// the SAME plane the files module serves. NEVER read by
    /// `root()`/`execute`/`commit_block`'s accept path.
    pub(crate) blobs: blobstore::BlobHandle,
    /// the repo namespace, keyed by normalized slug and kept SORTED so
    /// `root()` composes order-independently. seeded at construction from the
    /// on-disk repos (restart re-adopt) and grown lazily on first write.
    pub(crate) repos: BTreeMap<String, RepoState>,
    /// the COMMITTED tracker (issues/PRs/reviews) — consensus state, persisted
    /// to [`TRACKER_FILE`] and folded into `root()`.
    pub(crate) tracker: Tracker,
    /// the block-scratch tracker: clone-on-write on the first tracker mutation
    /// of a block, swapped in by `commit_block`, dropped by `abort_block`.
    pub(crate) staged_tracker: Option<Tracker>,
    /// where issue/PR discussion-channel follow-ups go (`emit_msg` target).
    /// `None` (tests / minimal deployments without chat) emits nothing.
    chat_target: Option<String>,
}

impl Forge {
    /// genesis wiring with a private, default (empty) blob store — enough for a
    /// `Commit`-only or test deployment.
    pub fn init(id: impl Into<ModuleId>, base_dir: impl Into<PathBuf>) -> Result<Self, Error> {
        Self::with_blobs(id, base_dir, blobstore::BlobHandle::default())
    }

    /// genesis wiring over an EXISTING node-local blob store. `base_dir` is the
    /// CONTAINER for the repo namespace; construction creates it and RE-ADOPTS
    /// every existing per-repo git dir (seeding each repo's committed branches
    /// from its on-disk refs) and the persisted tracker file — so `root()` is
    /// correct immediately after a restart. a fresh container starts at
    /// [`StateRoot::ZERO`].
    pub fn with_blobs(
        id: impl Into<ModuleId>,
        base_dir: impl Into<PathBuf>,
        blobs: blobstore::BlobHandle,
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
            // be one this module created — ignore it.
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
            let branches = git::list_branches(&repo).map_err(|e| Error::Module(e.to_string()))?;
            repos.insert(name, RepoState::with_refs(branches.into_iter().collect()));
        }

        // re-adopt the persisted tracker. a corrupt file is FAIL-STOP (like a
        // corrupt repo): booting with a silently-empty tracker would compose a
        // wrong root and fork this node at its first app-hash check anyway.
        let tracker_path = base.join(TRACKER_FILE);
        let tracker = if tracker_path.exists() {
            let bytes = std::fs::read(&tracker_path)
                .map_err(|e| Error::Module(format!("forge: read tracker file: {e}")))?;
            Tracker::decode(&bytes)?
        } else {
            Tracker::default()
        };

        Ok(Self {
            id: id.into(),
            base,
            blobs,
            repos,
            tracker,
            staged_tracker: None,
            chat_target: None,
        })
    }

    /// route issue/PR discussion follow-ups at the given chat module. the node
    /// binaries wire `"chat"`; without it forge stays fully functional but
    /// opens no discussion channels.
    pub fn with_chat(mut self, target: impl Into<String>) -> Self {
        self.chat_target = Some(target.into());
        self
    }

    /// ensure a [`RepoState`] entry exists for `name` (already normalized).
    fn ensure_repo(&mut self, name: &str) {
        if !self.repos.contains_key(name) {
            self.repos.insert(name.to_string(), RepoState::default());
        }
    }

    /// node-local catch-up across ALL repos (see [`refs::RepoState::materialize`]).
    pub fn materialize(&mut self) -> Result<(), Error> {
        let base = &self.base;
        let blobs = &self.blobs;
        for (name, state) in self.repos.iter_mut() {
            state.materialize(base, name, blobs)?;
        }
        Ok(())
    }

    /// atomically persist the COMMITTED tracker to [`TRACKER_FILE`].
    pub(crate) fn persist_tracker(&self) -> Result<(), Error> {
        let path = self.base.join(TRACKER_FILE);
        let tmp = self.base.join(".tracker.bin.tmp");
        std::fs::write(&tmp, self.tracker.canonical_bytes())
            .map_err(|e| Error::Module(format!("forge: write tracker file: {e}")))?;
        std::fs::rename(&tmp, &path)
            .map_err(|e| Error::Module(format!("forge: publish tracker file: {e}")))?;
        Ok(())
    }

    /// the tracker as THIS BLOCK sees it (read-your-writes).
    fn tracker_view(&self) -> &Tracker {
        self.staged_tracker.as_ref().unwrap_or(&self.tracker)
    }

    /// clone-on-write access to the block-scratch tracker.
    fn staged_tracker_mut(&mut self) -> &mut Tracker {
        self.staged_tracker
            .get_or_insert_with(|| self.tracker.clone())
    }

    /// emit a system line into an item's discussion channel (no-op without a
    /// chat target). the message id is minted from the item's own monotonic
    /// counter, so it is deterministic and collision-free.
    fn emit_system_line(
        &mut self,
        ctx: &mut dyn Ctx,
        repo: &str,
        number: u64,
        text: &str,
    ) -> Result<(), Error> {
        let Some(chat) = self.chat_target.clone() else {
            return Ok(());
        };
        let message_id = self.staged_tracker_mut().next_sys_message_id(repo, number)?;
        ctx.emit_msg(tracker::system_line_msg(&chat, repo, number, message_id, text));
        Ok(())
    }

    /// stage one `Commit` onto `name`'s `main` (already normalized + ensured):
    /// build the deterministic commit object over the effective parent and
    /// stage it WITHOUT moving the ref. chaining on the staged head gives
    /// multi-commit-in-one-block the correct parent.
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

        let parent_oid = state.effective_head(MAIN_BRANCH);
        let parent_commit = parent_oid
            .map(|oid| repo.find_commit(oid))
            .transpose()
            .map_err(|e| Error::Module(e.to_string()))?;

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

        let commit = git::commit(
            &repo,
            &tree,
            parent_commit.as_ref(),
            &message,
            consensus_time,
        )
        .map_err(|e| Error::Module(e.to_string()))?;

        // a Commit CHAINS in-block, so it replaces any staged main fate rather
        // than conflicting with it.
        state
            .staged
            .insert(MAIN_BRANCH.to_string(), refs::StagedRef::Local(commit));
        Ok(())
    }

    /// stage an atomic multi-branch push: validate the update list, then CAS
    /// every branch. PURE and deterministic — no repo opened, nothing
    /// installed, no ref moves (see [`refs::RepoState::stage_update`]).
    fn stage_push_refs(
        &mut self,
        name: &str,
        updates: Vec<RefUpdate>,
        pack_digest: Option<Vec<u8>>,
    ) -> Result<(), Error> {
        if updates.is_empty() {
            return Err(Error::Module("forge: push carries no ref updates".into()));
        }
        if updates.len() > MAX_REFS_PER_PUSH {
            return Err(Error::Module(format!(
                "forge: too many ref updates ({}, max {MAX_REFS_PER_PUSH})",
                updates.len()
            )));
        }
        let mut seen = BTreeSet::new();
        for u in &updates {
            norm_branch(&u.ref_name)?;
            if !seen.insert(u.ref_name.as_str()) {
                return Err(Error::Module(format!(
                    "forge: duplicate ref update for branch {:?}",
                    u.ref_name
                )));
            }
        }
        let digest = pack_digest.as_deref().map(parse_digest).transpose()?;
        if updates.iter().any(|u| u.new_oid.is_some()) && digest.is_none() {
            return Err(Error::Module(
                "forge: a push that sets heads needs a pack_digest".into(),
            ));
        }

        let state = self.repos.get_mut(name).expect("ensured by caller");
        for u in &updates {
            let prev = u
                .prev_oid
                .as_deref()
                .map(|b| parse_oid(b, "prev_oid"))
                .transpose()?;
            let new = u
                .new_oid
                .as_deref()
                .map(|b| parse_oid(b, "new_oid"))
                .transpose()?;
            state.stage_update(&u.ref_name, prev, new, new.is_some().then(|| digest.unwrap()))?;
        }
        Ok(())
    }

    /// this repo's read-your-writes `main` head hex (the legacy Head surface).
    fn read_head(&self, name: &str) -> Option<String> {
        self.repos
            .get(name)
            .and_then(|s| s.effective_head(MAIN_BRANCH))
            .map(|oid| oid.to_string())
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Forge {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// the composed state root — pure, no IO. see the composition invariant.
    fn root(&self) -> StateRoot {
        let entries = self.repos.iter().map(|(n, s)| (n.as_str(), &s.refs));
        compose_state_root(entries, &self.tracker)
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(self.snapshot()?))
    }

    /// apply one write op. git ops stage per-branch CAS updates or build
    /// deterministic commit objects; tracker ops mutate the block-scratch
    /// tracker and emit chat follow-ups (channel creation, system lines) that
    /// commit atomically with the block. all git2 IO is blocking with no
    /// `.await`.
    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let now = ctx.env().consensus_time;
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            ForgeMsg::Commit {
                repo,
                path,
                content,
                message,
            } => {
                let name = norm_repo(&repo)?;
                self.ensure_repo(&name);
                self.stage_commit(&name, now, path, content, message)
            }
            ForgeMsg::PushRefs {
                repo,
                updates,
                pack_digest,
            } => {
                let name = norm_repo(&repo)?;
                self.ensure_repo(&name);
                self.stage_push_refs(&name, updates, pack_digest)
            }
            ForgeMsg::OpenIssue { repo, title, body } => {
                let name = norm_repo(&repo)?;
                let author = author_from_origin(&ctx.env().origin)?;
                let number = self.staged_tracker_mut().open_item(
                    &name,
                    ItemKind::Issue,
                    title,
                    body,
                    author,
                    now,
                    None,
                )?;
                if let Some(chat) = self.chat_target.clone() {
                    ctx.emit_msg(tracker::create_channel_msg(&chat, &name, number));
                }
                Ok(())
            }
            ForgeMsg::OpenPr {
                repo,
                title,
                body,
                source_branch,
                target_branch,
            } => {
                let name = norm_repo(&repo)?;
                let author = author_from_origin(&ctx.env().origin)?;
                let target = if target_branch.is_empty() {
                    INTEGRATION_BRANCH.to_string()
                } else {
                    target_branch
                };
                norm_branch(&source_branch)?;
                norm_branch(&target)?;
                if source_branch == target {
                    return Err(Error::Module(
                        "forge: a pull request needs distinct source and target branches".into(),
                    ));
                }
                // both branches must be BORN in committed state — a PR from a
                // branch nobody pushed is meaningless, and the checks read
                // agreed state only.
                let state = self
                    .repos
                    .get(&name)
                    .ok_or_else(|| Error::Module(format!("forge: no repo {name:?}")))?;
                for (label, branch) in [("source", &source_branch), ("target", &target)] {
                    if !state.refs.contains_key(branch.as_str()) {
                        return Err(Error::Module(format!(
                            "forge: {label} branch {branch:?} is not born in repo {name:?}"
                        )));
                    }
                }
                let number = self.staged_tracker_mut().open_item(
                    &name,
                    ItemKind::Pr,
                    title,
                    body,
                    author,
                    now,
                    Some((source_branch, target)),
                )?;
                if let Some(chat) = self.chat_target.clone() {
                    ctx.emit_msg(tracker::create_channel_msg(&chat, &name, number));
                }
                Ok(())
            }
            ForgeMsg::EditItem {
                repo,
                number,
                title,
                body,
            } => {
                let name = norm_repo(&repo)?;
                let editor = author_from_origin(&ctx.env().origin)?;
                self.staged_tracker_mut()
                    .edit_item(&name, number, &editor, title, body, now)
            }
            ForgeMsg::SetItemState { repo, number, open } => {
                let name = norm_repo(&repo)?;
                author_from_origin(&ctx.env().origin)?;
                if let Some(verb) = self.staged_tracker_mut().set_state(&name, number, open, now)?
                {
                    self.emit_system_line(ctx, &name, number, &format!("{verb} this"))?;
                }
                Ok(())
            }
            ForgeMsg::MergePr {
                repo,
                number,
                prev_target_oid,
                expected_source_oid,
                merge_oid,
                pack_digest,
            } => {
                let name = norm_repo(&repo)?;
                author_from_origin(&ctx.env().origin)?;
                let prev_target = parse_hex_oid(&prev_target_oid, "prev_target_oid")?;
                let expected_source = parse_hex_oid(&expected_source_oid, "expected_source_oid")?;
                let merge = parse_hex_oid(&merge_oid, "merge_oid")?;
                let digest = parse_hex_digest(&pack_digest)?;

                // the PR must be an open PR; pull its branches.
                let (source, target) = self.tracker_view().pr_branches(&name, number)?;

                // double CAS on COMMITTED refs: the target must not have moved
                // under the merger, and the merge must have been computed
                // against the CURRENT source head (a force-push between compute
                // and submit rejects deterministically).
                let state = self
                    .repos
                    .get_mut(&name)
                    .ok_or_else(|| Error::Module(format!("forge: no repo {name:?}")))?;
                if state.refs.get(&source).copied() != Some(expected_source) {
                    return Err(Error::Module(
                        "forge: pull request source branch moved; recompute the merge".into(),
                    ));
                }
                state.stage_update(&target, Some(prev_target), Some(merge), Some(digest))?;
                self.staged_tracker_mut().merge_pr(&name, number, merge, now)?;
                self.emit_system_line(ctx, &name, number, "merged this pull request")?;
                Ok(())
            }
            ForgeMsg::SubmitReview {
                repo,
                number,
                verdict,
                body,
                commit_oid,
                comments,
            } => {
                let name = norm_repo(&repo)?;
                let author = author_from_origin(&ctx.env().origin)?;
                self.staged_tracker_mut().submit_review(
                    &name, number, author, verdict, body, &commit_oid, comments, now,
                )?;
                let line = match verdict {
                    ReviewVerdict::Approve => Some("approved these changes"),
                    ReviewVerdict::RequestChanges => Some("requested changes"),
                    ReviewVerdict::Comment => None,
                };
                if let Some(text) = line {
                    self.emit_system_line(ctx, &name, number, text)?;
                }
                Ok(())
            }
        }
    }

    /// read projections, served from the cached mirrors — no IO, no `.await`.
    /// `Head`/`HeadOf` are read-your-writes on `main`; the rest serve
    /// COMMITTED state.
    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            ForgeQuery::Head => Ok(encode_reply(&ForgeReply::Head(
                self.read_head(DEFAULT_REPO),
            ))),
            ForgeQuery::HeadOf { repo } => {
                let name = norm_repo(&repo)?;
                Ok(encode_reply(&ForgeReply::Head(self.read_head(&name))))
            }
            ForgeQuery::ListRepos => {
                // the committed INTEGRATION head (dev, falling back to legacy
                // main) — the same branch every browse surface reads, so a
                // dev-only repo lists as browsable, not unborn.
                let repos = self
                    .repos
                    .iter()
                    .map(|(name, s)| RepoHead {
                        name: name.clone(),
                        head: s
                            .refs
                            .get(INTEGRATION_BRANCH)
                            .or_else(|| s.refs.get(MAIN_BRANCH))
                            .map(|oid| oid.to_string()),
                    })
                    .collect();
                Ok(encode_reply(&ForgeReply::Repos(repos)))
            }
            ForgeQuery::ListRefs { repo } => {
                let name = norm_repo(&repo)?;
                let refs = self
                    .repos
                    .get(&name)
                    .map(|s| {
                        s.refs
                            .iter()
                            .map(|(branch, oid)| RefHead {
                                name: branch.clone(),
                                head: oid.to_string(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                Ok(encode_reply(&ForgeReply::Refs(refs)))
            }
            ForgeQuery::ListItems { repo } => {
                let name = norm_repo(&repo)?;
                Ok(encode_reply(&ForgeReply::Items(self.tracker.list(&name))))
            }
            ForgeQuery::GetItem { repo, number } => {
                let name = norm_repo(&repo)?;
                Ok(encode_reply(&ForgeReply::Item(
                    self.tracker.get(&name, number).map(Box::new),
                )))
            }
        }
    }

    /// publish everything staged: per-repo branch fates (Local ref moves,
    /// Packed head publications + materialization targets, Deletes), then the
    /// block-scratch tracker (persisted to disk).
    async fn commit_block(&mut self) -> Result<(), Error> {
        let base = &self.base;
        let blobs = &self.blobs;
        for (name, state) in self.repos.iter_mut() {
            state.publish(base, name, blobs)?;
        }
        if let Some(t) = self.staged_tracker.take() {
            self.tracker = t;
            self.persist_tracker()?;
        }
        Ok(())
    }

    /// discard everything staged — no ref moved, tracker unchanged, `root()`
    /// unchanged.
    async fn abort_block(&mut self) -> Result<(), Error> {
        for state in self.repos.values_mut() {
            state.abort();
        }
        self.staged_tracker = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{decode_reply, encode_msg, encode_query};

    // a minimal Ctx so execute can read consensus_time / origin and CAPTURE
    // emitted follow-ups without a full host.
    struct TestCtx {
        env: sdk::Env,
        emitted: Vec<Msg>,
    }
    impl TestCtx {
        fn at(consensus_time: u64) -> Self {
            Self::with_origin(consensus_time, sdk::Origin::System)
        }
        fn with_origin(consensus_time: u64, origin: sdk::Origin) -> Self {
            Self {
                env: sdk::Env {
                    protocol_version: 0,
                    height: 0,
                    consensus_time,
                    origin,
                    me: "forge".into(),
                },
                emitted: Vec::new(),
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
        fn emit_msg(&mut self, m: Msg) {
            self.emitted.push(m);
        }
        fn emit_event(&mut self, _e: sdk::Event) {}
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

    fn exec(forge: &mut Forge, ctx: &mut TestCtx, m: &ForgeMsg) -> Result<(), Error> {
        let msg = Msg {
            target: "forge".into(),
            payload: encode_msg(m),
        };
        futures::executor::block_on(forge.execute(ctx, &msg))
    }

    fn exec_commit(forge: &mut Forge, ctx: &mut TestCtx, m: &ForgeMsg) {
        exec(forge, ctx, m).unwrap();
        futures::executor::block_on(forge.commit_block()).unwrap();
    }

    fn commit(forge: &mut Forge, t: u64, repo: &str, path: &str, content: &str, message: &str) {
        futures::executor::block_on(forge.execute(
            &mut TestCtx::at(t),
            &commit_msg(repo, path, content, message),
        ))
        .unwrap();
        futures::executor::block_on(forge.commit_block()).unwrap();
    }

    // read a repo's main oid via git2 directly — the independent oracle that
    // root() tracks the real refs, not just the cache.
    fn git_head_oid(base: &std::path::Path, repo: &str) -> Oid {
        git2::Repository::open(base.join(repo))
            .unwrap()
            .refname_to_id("refs/heads/main")
            .unwrap()
    }

    fn oid(hexc: char) -> Oid {
        Oid::from_str(&hexc.to_string().repeat(40)).unwrap()
    }

    fn user_origin(b: u8) -> sdk::Origin {
        sdk::Origin::External(vec![b; 8])
    }

    #[test]
    fn genesis_is_zero_then_commit_makes_root_equal_composed_head() {
        let base = tmp_base("basic");
        let mut forge = Forge::init("forge", base.clone()).unwrap();
        assert_eq!(forge.root(), StateRoot::ZERO, "empty namespace -> ZERO root");

        // a Commit with an EMPTY repo -> the default repo (back-compat wire).
        commit(&mut forge, 100, "", "a.txt", "hello", "first");

        assert_ne!(forge.root(), StateRoot::ZERO, "a commit must move the root");

        // root() == the composition over {"default": {"main": <real HEAD>}}.
        let head = git_head_oid(&base, DEFAULT_REPO);
        let refs: BTreeMap<String, Oid> = [(MAIN_BRANCH.to_string(), head)].into();
        assert_eq!(
            forge.root(),
            compose_state_root(
                [(DEFAULT_REPO, &refs)].into_iter(),
                &Tracker::default()
            ),
            "root() must be the composition of the real default-repo refs"
        );

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
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn root_composes_into_global_root() {
        let base = tmp_base("compose");
        let mut forge = Forge::init("forge", base.clone()).unwrap();
        let before = host::global_root(&[&forge as &dyn Module]);
        commit(&mut forge, 7, "", "a.txt", "x", "c");
        let after = host::global_root(&[&forge as &dyn Module]);
        assert_ne!(before, after, "forge's root must move the global app-hash");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn commit_oid_is_reproducible_across_namespaces() {
        let a = tmp_base("det-a");
        let b = tmp_base("det-b");
        let mut fa = Forge::init("forge", a.clone()).unwrap();
        let mut fb = Forge::init("forge", b.clone()).unwrap();
        commit(&mut fa, 555, "myrepo", "f.txt", "same", "same-msg");
        commit(&mut fb, 555, "myrepo", "f.txt", "same", "same-msg");
        assert_eq!(fa.root(), fb.root(), "pinned identity+date -> identical root");
        let _ = std::fs::remove_dir_all(&a);
        let _ = std::fs::remove_dir_all(&b);
    }

    // multi-branch: an atomic PushRefs births branches, CASes per branch,
    // deletes non-main branches, and refuses main deletion — all reflected in
    // root() without any pack materialized (the determinism invariant).
    #[test]
    fn push_refs_multi_branch_flow() {
        let base = tmp_base("multi-branch");
        let mut forge = Forge::init("forge", base.clone()).unwrap();
        let digest = vec![7u8; 32];

        // birth main + a feature branch in ONE atomic push.
        let mut ctx = TestCtx::at(1);
        exec_commit(
            &mut forge,
            &mut ctx,
            &ForgeMsg::PushRefs {
                repo: "demo".into(),
                updates: vec![
                    RefUpdate {
                        ref_name: "main".into(),
                        prev_oid: None,
                        new_oid: Some(oid('a').as_bytes().to_vec()),
                    },
                    RefUpdate {
                        ref_name: "feature/x".into(),
                        prev_oid: None,
                        new_oid: Some(oid('b').as_bytes().to_vec()),
                    },
                ],
                pack_digest: Some(digest.clone()),
            },
        );
        let r1 = forge.root();
        assert_ne!(r1, StateRoot::ZERO);

        // ListRefs sees both branches.
        let reply =
            futures::executor::block_on(forge.query(&encode_query(&ForgeQuery::ListRefs {
                repo: "demo".into(),
            })))
            .unwrap();
        let ForgeReply::Refs(refs) = decode_reply(&reply).unwrap() else {
            panic!("wrong reply")
        };
        assert_eq!(
            refs.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
            vec!["feature/x", "main"]
        );

        // stale CAS rejects; fresh CAS force-moves the feature branch.
        let mut ctx = TestCtx::at(2);
        assert!(exec(
            &mut forge,
            &mut ctx,
            &ForgeMsg::PushRefs {
                repo: "demo".into(),
                updates: vec![RefUpdate {
                    ref_name: "feature/x".into(),
                    prev_oid: Some(oid('a').as_bytes().to_vec()), // stale
                    new_oid: Some(oid('c').as_bytes().to_vec()),
                }],
                pack_digest: Some(digest.clone()),
            },
        )
        .is_err());
        futures::executor::block_on(forge.abort_block()).unwrap();

        let mut ctx = TestCtx::at(3);
        exec_commit(
            &mut forge,
            &mut ctx,
            &ForgeMsg::PushRefs {
                repo: "demo".into(),
                updates: vec![RefUpdate {
                    ref_name: "feature/x".into(),
                    prev_oid: Some(oid('b').as_bytes().to_vec()),
                    new_oid: Some(oid('c').as_bytes().to_vec()), // force-ish move
                }],
                pack_digest: Some(digest.clone()),
            },
        );
        assert_ne!(forge.root(), r1, "branch move must move the root");

        // deleting main is refused; deleting the feature branch works and is
        // pack-free.
        let mut ctx = TestCtx::at(4);
        assert!(exec(
            &mut forge,
            &mut ctx,
            &ForgeMsg::PushRefs {
                repo: "demo".into(),
                updates: vec![RefUpdate {
                    ref_name: "main".into(),
                    prev_oid: Some(oid('a').as_bytes().to_vec()),
                    new_oid: None,
                }],
                pack_digest: None,
            },
        )
        .is_err());
        futures::executor::block_on(forge.abort_block()).unwrap();

        let mut ctx = TestCtx::at(5);
        exec_commit(
            &mut forge,
            &mut ctx,
            &ForgeMsg::PushRefs {
                repo: "demo".into(),
                updates: vec![RefUpdate {
                    ref_name: "feature/x".into(),
                    prev_oid: Some(oid('c').as_bytes().to_vec()),
                    new_oid: None,
                }],
                pack_digest: None,
            },
        );
        let reply =
            futures::executor::block_on(forge.query(&encode_query(&ForgeQuery::ListRefs {
                repo: "demo".into(),
            })))
            .unwrap();
        let ForgeReply::Refs(refs) = decode_reply(&reply).unwrap() else {
            panic!("wrong reply")
        };
        assert_eq!(refs.len(), 1, "only main survives");

        let _ = std::fs::remove_dir_all(&base);
    }

    // the tracker flow: open issue -> hidden channel follow-up; open PR on a
    // born branch; review; merge via double CAS — refs AND item state move
    // atomically; system lines ride the item's own discussion channel.
    #[test]
    fn tracker_issue_pr_review_merge_flow() {
        let base = tmp_base("tracker");
        let mut forge = Forge::init("forge", base.clone()).unwrap().with_chat("chat");
        let digest = vec![9u8; 32];

        // seed a repo with release main, integration dev, and a feature branch
        // (fabricated oids — packs never gate consensus).
        let mut ctx = TestCtx::at(1);
        exec_commit(
            &mut forge,
            &mut ctx,
            &ForgeMsg::PushRefs {
                repo: "demo".into(),
                updates: vec![
                    RefUpdate {
                        ref_name: "main".into(),
                        prev_oid: None,
                        new_oid: Some(oid('a').as_bytes().to_vec()),
                    },
                    RefUpdate {
                        ref_name: "dev".into(),
                        prev_oid: None,
                        new_oid: Some(oid('a').as_bytes().to_vec()),
                    },
                    RefUpdate {
                        ref_name: "feat".into(),
                        prev_oid: None,
                        new_oid: Some(oid('b').as_bytes().to_vec()),
                    },
                ],
                pack_digest: Some(digest.clone()),
            },
        );

        // an issue: number 1, channel follow-up emitted.
        let mut ctx = TestCtx::with_origin(2, user_origin(1));
        exec(
            &mut forge,
            &mut ctx,
            &ForgeMsg::OpenIssue {
                repo: "demo".into(),
                title: "it breaks".into(),
                body: "details".into(),
            },
        )
        .unwrap();
        futures::executor::block_on(forge.commit_block()).unwrap();
        assert_eq!(ctx.emitted.len(), 1);
        assert_eq!(ctx.emitted[0].target, "chat");
        let chat::ChatMsg::CreateChannel { channel_id, name, .. } =
            chat::decode_msg(&ctx.emitted[0].payload).unwrap()
        else {
            panic!("expected CreateChannel")
        };
        assert_eq!(channel_id, "forge:demo:1");
        assert_eq!(name, "demo#1");

        // a PR from the born feature branch: shares the number space (#2).
        let mut ctx = TestCtx::with_origin(3, user_origin(2));
        exec(
            &mut forge,
            &mut ctx,
            &ForgeMsg::OpenPr {
                repo: "demo".into(),
                title: "fix it".into(),
                body: "the fix".into(),
                source_branch: "feat".into(),
                target_branch: String::new(),
            },
        )
        .unwrap();
        futures::executor::block_on(forge.commit_block()).unwrap();
        assert_eq!(ctx.emitted.len(), 1, "channel follow-up for the PR");

        // a PR from an unborn branch rejects.
        let mut ctx = TestCtx::with_origin(4, user_origin(2));
        assert!(exec(
            &mut forge,
            &mut ctx,
            &ForgeMsg::OpenPr {
                repo: "demo".into(),
                title: "nope".into(),
                body: String::new(),
                source_branch: "ghost".into(),
                target_branch: String::new(),
            },
        )
        .is_err());
        futures::executor::block_on(forge.abort_block()).unwrap();

        // review with a line comment + approval system line.
        let mut ctx = TestCtx::with_origin(5, user_origin(3));
        exec(
            &mut forge,
            &mut ctx,
            &ForgeMsg::SubmitReview {
                repo: "demo".into(),
                number: 2,
                verdict: ReviewVerdict::Approve,
                body: "ship it".into(),
                commit_oid: oid('b').to_string(),
                comments: vec![ReviewComment {
                    path: "src/lib.rs".into(),
                    line: 10,
                    side: DiffSide::New,
                    body: "nice".into(),
                }],
            },
        )
        .unwrap();
        futures::executor::block_on(forge.commit_block()).unwrap();
        assert_eq!(ctx.emitted.len(), 1, "approval line emitted");

        // merge: stale target CAS rejects; the real one moves dev AND marks
        // the PR merged in one block.
        let mut ctx = TestCtx::with_origin(6, user_origin(2));
        assert!(exec(
            &mut forge,
            &mut ctx,
            &ForgeMsg::MergePr {
                repo: "demo".into(),
                number: 2,
                prev_target_oid: oid('f').to_string(), // stale
                expected_source_oid: oid('b').to_string(),
                merge_oid: oid('c').to_string(),
                pack_digest: hex(&digest),
            },
        )
        .is_err());
        futures::executor::block_on(forge.abort_block()).unwrap();

        let mut ctx = TestCtx::with_origin(7, user_origin(2));
        exec(
            &mut forge,
            &mut ctx,
            &ForgeMsg::MergePr {
                repo: "demo".into(),
                number: 2,
                prev_target_oid: oid('a').to_string(),
                expected_source_oid: oid('b').to_string(),
                merge_oid: oid('c').to_string(),
                pack_digest: hex(&digest),
            },
        )
        .unwrap();
        futures::executor::block_on(forge.commit_block()).unwrap();
        assert_eq!(ctx.emitted.len(), 1, "merged line emitted");

        // committed state: release main stays put, dev advances, and the PR is
        // merged with its review recorded.
        assert_eq!(forge.read_head("demo"), Some(oid('a').to_string()));
        let refs = futures::executor::block_on(
            forge.query(&encode_query(&ForgeQuery::ListRefs {
                repo: "demo".into(),
            })),
        )
        .unwrap();
        let ForgeReply::Refs(refs) = decode_reply(&refs).unwrap() else {
            panic!("refs missing")
        };
        assert_eq!(
            refs.iter().find(|head| head.name == "dev").unwrap().head,
            oid('c').to_string()
        );
        let reply = futures::executor::block_on(
            forge.query(&encode_query(&ForgeQuery::GetItem {
                repo: "demo".into(),
                number: 2,
            })),
        )
        .unwrap();
        let ForgeReply::Item(Some(item)) = decode_reply(&reply).unwrap() else {
            panic!("item missing")
        };
        assert_eq!(item.summary.state, ItemState::Merged);
        assert_eq!(item.merge_oid.as_deref(), Some(oid('c').to_string().as_str()));
        assert_eq!(item.reviews.len(), 1);
        assert_eq!(item.channel_id, "forge:demo:2");

        // a merged PR cannot merge/close again.
        let mut ctx = TestCtx::with_origin(8, user_origin(2));
        assert!(exec(
            &mut forge,
            &mut ctx,
            &ForgeMsg::SetItemState {
                repo: "demo".into(),
                number: 2,
                open: false,
            },
        )
        .is_err());
        futures::executor::block_on(forge.abort_block()).unwrap();

        // tracker survives restart via the persisted file.
        drop(forge);
        let reopened = Forge::init("forge", base.clone()).unwrap();
        let reply = futures::executor::block_on(
            reopened.query(&encode_query(&ForgeQuery::ListItems {
                repo: "demo".into(),
            })),
        )
        .unwrap();
        let ForgeReply::Items(items) = decode_reply(&reply).unwrap() else {
            panic!("wrong reply")
        };
        assert_eq!(items.len(), 2, "issue + PR re-adopted from disk");

        let _ = std::fs::remove_dir_all(&base);
    }

    // two independent namespaces replaying the same ops (branches + tracker)
    // compose IDENTICAL roots — the tracker is deterministic consensus state.
    #[test]
    fn tracker_root_is_reproducible_across_namespaces() {
        let run = |tag: &str| {
            let base = tmp_base(tag);
            let mut forge = Forge::init("forge", base.clone()).unwrap().with_chat("chat");
            let mut ctx = TestCtx::at(1);
            exec_commit(
                &mut forge,
                &mut ctx,
                &ForgeMsg::PushRefs {
                    repo: "demo".into(),
                    updates: vec![RefUpdate {
                        ref_name: "main".into(),
                        prev_oid: None,
                        new_oid: Some(oid('a').as_bytes().to_vec()),
                    }],
                    pack_digest: Some(vec![1u8; 32]),
                },
            );
            let mut ctx = TestCtx::with_origin(2, user_origin(9));
            exec(
                &mut forge,
                &mut ctx,
                &ForgeMsg::OpenIssue {
                    repo: "demo".into(),
                    title: "same".into(),
                    body: "same".into(),
                },
            )
            .unwrap();
            futures::executor::block_on(forge.commit_block()).unwrap();
            let root = forge.root();
            let _ = std::fs::remove_dir_all(&base);
            root
        };
        assert_eq!(run("det-t-a"), run("det-t-b"));
    }

    // every snapshot leads with the container magic; install requires it.
    #[test]
    fn snapshot_leads_with_magic_and_install_requires_it() {
        let base = tmp_base("magic");
        let mut forge = Forge::init("forge", base.clone()).unwrap();
        commit(&mut forge, 42, "docs", "a.txt", "x", "c");
        let root = forge.root();
        let snap = forge.snapshot().unwrap();
        assert!(snap.starts_with(FORGE_SNAPSHOT_MAGIC.as_slice()));

        let rt = tmp_base("magic-rt");
        let mut fresh = Forge::init("forge", rt.clone()).unwrap();
        assert!(
            fresh
                .install(&snap[FORGE_SNAPSHOT_MAGIC.len()..], root)
                .is_err(),
            "a container missing the magic must be rejected"
        );
        fresh.install(&snap, root).unwrap();
        assert_eq!(fresh.root(), root, "install reproduces the root");

        let _ = std::fs::remove_dir_all(&base);
        let _ = std::fs::remove_dir_all(&rt);
    }

    // a snapshot carries branches AND tracker; install onto a fresh namespace
    // reproduces the root byte-for-byte.
    #[test]
    fn snapshot_round_trips_branches_and_tracker() {
        let base = tmp_base("snap");
        let mut forge = Forge::init("forge", base.clone()).unwrap().with_chat("chat");

        // real objects on main (Commit builds them), then a second branch on
        // the SAME oid — its objects exist, so the snapshot pack closes.
        commit(&mut forge, 1, "demo", "a.txt", "hello", "c1");
        let head = git_head_oid(&base, "demo");
        let mut ctx = TestCtx::at(2);
        exec_commit(
            &mut forge,
            &mut ctx,
            &ForgeMsg::PushRefs {
                repo: "demo".into(),
                updates: vec![RefUpdate {
                    ref_name: "feat".into(),
                    prev_oid: None,
                    new_oid: Some(head.as_bytes().to_vec()),
                }],
                pack_digest: Some(vec![3u8; 32]),
            },
        );
        let mut ctx = TestCtx::with_origin(3, user_origin(4));
        exec(
            &mut forge,
            &mut ctx,
            &ForgeMsg::OpenIssue {
                repo: "demo".into(),
                title: "carry me".into(),
                body: String::new(),
            },
        )
        .unwrap();
        futures::executor::block_on(forge.commit_block()).unwrap();

        let root = forge.root();
        let snap = forge.snapshot().unwrap();

        let rt = tmp_base("snap-rt");
        let mut fresh = Forge::init("forge", rt.clone()).unwrap();
        fresh.install(&snap, root).unwrap();
        assert_eq!(fresh.root(), root, "install reproduces the root");
        let reply = futures::executor::block_on(
            fresh.query(&encode_query(&ForgeQuery::ListItems {
                repo: "demo".into(),
            })),
        )
        .unwrap();
        let ForgeReply::Items(items) = decode_reply(&reply).unwrap() else {
            panic!("wrong reply")
        };
        assert_eq!(items.len(), 1, "tracker rode the snapshot");

        // a tampered container is rejected by the root gate.
        let mut bad = snap.clone();
        let last = bad.len() - 1;
        bad[last] ^= 1;
        let mut fresh2 = Forge::init("forge", tmp_base("snap-bad")).unwrap();
        assert!(fresh2.install(&bad, root).is_err());

        let _ = std::fs::remove_dir_all(&base);
        let _ = std::fs::remove_dir_all(&rt);
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
