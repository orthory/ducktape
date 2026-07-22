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
//!   - DETERMINISM: clients build immutable Git objects before submission;
//!     consensus only compare-and-swaps already-fixed oids and never reads a
//!     validator's node-local object database.
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
//! ## the default repo
//!
//! every [`ForgeMsg`] carries a required `repo` field; an empty slug maps to
//! the well-known `"default"` repo. the unit [`ForgeQuery::Head`] answers the
//! default repo's `main` head.
//!
//! ## the consensus / data-plane boundary
//!
//! Git commit/tree/blob objects are data-plane state. a producer builds them
//! off-chain and the relay distributes their content-addressed pack before the
//! corresponding [`ForgeMsg::PushRefs`] enters consensus. consensus state owns
//! only the canonical branch oids (plus tracker state), and its sole Git write
//! is a pure `prev -> new` CAS. the old file-by-file [`ForgeMsg::Commit`] wire
//! variant is retained only to return a deterministic migration error.
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

use crate::refs::{norm_branch, RepoState, INTEGRATION_BRANCH, MAIN_BRANCH};
use crate::tracker::{author_from_origin, parse_hex_oid, Tracker};

/// the well-known repo an empty `repo` field maps to — the single-repo wire
/// (see the module docstring).
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
/// hashed over the folded preimage in [`compose_state_root`].
const FORGE_ROOT_DOMAIN: &[u8] = b"ducktape.forge.multirepo.v1\x00";

/// the 4-byte magic every forge snapshot container leads with.
pub(crate) const FORGE_SNAPSHOT_MAGIC: &[u8; 4] = b"FGv1";

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

fn reject_legacy_commit() -> Result<(), Error> {
    Err(Error::Module(
        "forge: Commit is retired; build the Git commit off-chain and submit PushRefs".into(),
    ))
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
    /// genesis wiring with a private, default (empty) blob store — useful for
    /// pack-less determinism tests and tracker-only deployments.
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

    /// this repo's read-your-writes `main` head hex (the single-repo Head surface).
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

    /// 2: the root domain + snapshot magic reset to v1 tags with the
    /// no-versioning sweep — same layout, different preimage bytes.
    fn state_schema_revision(&self) -> u32 {
        2
    }

    /// the composed state root — pure, no IO. see the composition invariant.
    fn root(&self) -> StateRoot {
        let entries = self.repos.iter().map(|(n, s)| (n.as_str(), &s.refs));
        compose_state_root(entries, &self.tracker)
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(self.snapshot()?))
    }

    /// apply one write op. Git writes stage pure per-branch CAS updates;
    /// tracker ops mutate the block-scratch tracker and emit chat follow-ups
    /// that commit atomically with the block. execute never opens a Git repo.
    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let now = ctx.env().consensus_time;
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            ForgeMsg::Commit { .. } => reject_legacy_commit(),
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
                // the committed INTEGRATION head (dev, falling back to main) —
                // the same branch every browse surface reads, so a
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
            ForgeQuery::PrDiff { repo, number } => {
                let name = norm_repo(&repo)?;
                let item = self.tracker.get(&name, number).ok_or_else(|| {
                    Error::Module(format!("forge: no item #{number} in repo {name:?}"))
                })?;
                if item.summary.kind != ItemKind::Pr {
                    return Err(Error::Module(format!(
                        "forge: item #{number} is an issue, not a pull request"
                    )));
                }
                let source_branch = item.source_branch.ok_or_else(|| {
                    Error::Module(format!("forge: pull request #{number} has no source branch"))
                })?;
                let target_branch = item.target_branch.ok_or_else(|| {
                    Error::Module(format!("forge: pull request #{number} has no target branch"))
                })?;
                let state = self.repos.get(&name).ok_or_else(|| {
                    Error::Module(format!("forge: no repo {name:?}"))
                })?;
                let source = state.refs.get(&source_branch).copied().ok_or_else(|| {
                    Error::Module(format!(
                        "forge: pull request #{number} source branch {source_branch:?} is not materialized"
                    ))
                })?;
                let target = state.refs.get(&target_branch).copied().ok_or_else(|| {
                    Error::Module(format!(
                        "forge: pull request #{number} target branch {target_branch:?} is not materialized"
                    ))
                })?;
                let repo = git::open(&self.base.join(&name)).map_err(|e| {
                    Error::Module(format!(
                        "forge: repo {name:?} is not materialized (target {target}, source \
                         {source}): {e}"
                    ))
                })?;
                let (patch, truncated, files_changed, additions, deletions) =
                    match git::bounded_diff(
                        &repo,
                        target,
                        source,
                        MAX_PR_DIFF_BYTES,
                        MAX_PR_DIFF_FILES,
                        MAX_PR_DIFF_BLOB_BYTES,
                    ) {
                        Ok(diff) => diff,
                        Err(e @ git::BoundedDiffError::TooLarge { .. }) => {
                            return Err(Error::Module(format!(
                                "forge: pull request #{number} diff is too large to serve \
                                 (target {target}, source {source}): {e}"
                            )));
                        }
                        Err(git::BoundedDiffError::Git(e)) => {
                            return Err(Error::Module(format!(
                                "forge: objects for pull request #{number} are not fully \
                                 materialized (target {target}, source {source}): {e}"
                            )));
                        }
                    };
                Ok(encode_reply(&ForgeReply::PrDiff(PrDiff {
                    source_oid: source.to_string(),
                    target_oid: target.to_string(),
                    files_changed,
                    additions,
                    deletions,
                    patch,
                    truncated,
                })))
            }
        }
    }

    /// publish everything staged: packed head publications + materialization
    /// targets and deletes, then the block-scratch tracker (persisted to disk).
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

    use sdk_testkit::TestCtx;

    // forge's execute reads only env (consensus_time / origin) and CAPTURES
    // emitted follow-ups; the shared TestCtx captures them (read via `msgs()`).
    fn ctx_at(consensus_time: u64) -> TestCtx {
        ctx_with_origin(consensus_time, sdk::Origin::System)
    }
    fn ctx_with_origin(consensus_time: u64, origin: sdk::Origin) -> TestCtx {
        TestCtx::with_env(sdk::Env {
            height: 0,
            consensus_time,
            origin,
            me: "forge".into(),
        })
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

    /// Test-fixture plumbing only: put real objects and a ref directly on disk
    /// so diff/snapshot tests can exercise libgit2 without restoring a
    /// consensus commit-building path.
    fn seed_materialized_commit(
        forge: &mut Forge,
        t: u64,
        repo: &str,
        path: &str,
        content: &str,
        message: &str,
    ) {
        let name = norm_repo(repo).unwrap();
        forge.ensure_repo(&name);
        let git_repo = refs::open_or_init_repo(&forge.base, &name).unwrap();
        let state = forge.repos.get_mut(&name).unwrap();
        let parent = state
            .refs
            .get(MAIN_BRANCH)
            .copied()
            .map(|oid| git_repo.find_commit(oid).unwrap());
        let base_tree = parent.as_ref().map(|commit| commit.tree().unwrap());
        let blob = git_repo.blob(content.as_bytes()).unwrap();
        let tree_oid = git::build_tree(&git_repo, base_tree.as_ref(), path, blob).unwrap();
        let tree = git_repo.find_tree(tree_oid).unwrap();
        let oid = git::commit(&git_repo, &tree, parent.as_ref(), message, t).unwrap();
        git::update_ref(&git_repo, &refs::full_ref(MAIN_BRANCH), oid).unwrap();
        state.refs.insert(MAIN_BRANCH.to_string(), oid);
    }

    fn push_head(forge: &mut Forge, repo: &str, prev: Option<Oid>, new: Oid) {
        let mut ctx = ctx_at(0);
        exec_commit(
            forge,
            &mut ctx,
            &ForgeMsg::PushRefs {
                repo: repo.into(),
                updates: vec![RefUpdate {
                    ref_name: MAIN_BRANCH.into(),
                    prev_oid: prev.map(|oid| oid.as_bytes().to_vec()),
                    new_oid: Some(new.as_bytes().to_vec()),
                }],
                pack_digest: Some(vec![7u8; 32]),
            },
        );
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

    fn materialized_pr(base: &std::path::Path, content: &[u8]) -> (Forge, Oid, Oid) {
        let mut forge = Forge::init("forge", base.to_path_buf()).unwrap();
        seed_materialized_commit(&mut forge, 1, "demo", "base.txt", "base\n", "base");
        let repo = git::open(&base.join("demo")).unwrap();
        let target = git_head_oid(base, "demo");
        let target_commit = repo.find_commit(target).unwrap();
        let target_tree = target_commit.tree().unwrap();
        let blob = repo.blob(content).unwrap();
        let source_tree_oid =
            git::build_tree(&repo, Some(&target_tree), "feature.txt", blob).unwrap();
        let source_tree = repo.find_tree(source_tree_oid).unwrap();
        let source = git::commit(
            &repo,
            &source_tree,
            Some(&target_commit),
            "feature",
            2,
        )
        .unwrap();
        git::update_ref(&repo, "refs/heads/dev", target).unwrap();
        git::update_ref(&repo, "refs/heads/feature", source).unwrap();
        let state = forge.repos.get_mut("demo").unwrap();
        state.refs.insert("dev".into(), target);
        state.refs.insert("feature".into(), source);

        let mut ctx = ctx_with_origin(3, user_origin(1));
        exec_commit(
            &mut forge,
            &mut ctx,
            &ForgeMsg::OpenPr {
                repo: "demo".into(),
                title: "review me".into(),
                body: String::new(),
                source_branch: "feature".into(),
                target_branch: "dev".into(),
            },
        );
        (forge, source, target)
    }

    fn replace_pr_source_with_files(
        forge: &mut Forge,
        base: &std::path::Path,
        target: Oid,
        files: usize,
        content: &[u8],
    ) -> Oid {
        let repo = git::open(&base.join("demo")).unwrap();
        let target_commit = repo.find_commit(target).unwrap();
        let mut tree_oid = target_commit.tree_id();
        let blob = repo.blob(content).unwrap();
        for index in 0..files {
            let tree = repo.find_tree(tree_oid).unwrap();
            tree_oid = git::build_tree(
                &repo,
                Some(&tree),
                &format!("feature-{index:04}.txt"),
                blob,
            )
            .unwrap();
        }
        let tree = repo.find_tree(tree_oid).unwrap();
        let source = git::commit(&repo, &tree, Some(&target_commit), "feature", 2).unwrap();
        git::update_ref(&repo, "refs/heads/feature", source).unwrap();
        forge
            .repos
            .get_mut("demo")
            .unwrap()
            .refs
            .insert("feature".into(), source);
        source
    }

    #[test]
    fn pr_diff_pins_oids_and_returns_a_reviewable_patch() {
        let base = tmp_base("pr-diff");
        let (forge, source, target) = materialized_pr(&base, b"reviewable\n");
        let bytes = futures::executor::block_on(forge.query(&encode_query(
            &ForgeQuery::PrDiff {
                repo: "demo".into(),
                number: 1,
            },
        )))
        .unwrap();
        let ForgeReply::PrDiff(diff) = decode_reply(&bytes).unwrap() else {
            panic!("wrong reply")
        };
        assert_eq!(diff.source_oid, source.to_string());
        assert_eq!(diff.target_oid, target.to_string());
        assert_eq!(diff.files_changed, 1);
        assert_eq!(diff.additions, 1);
        assert_eq!(diff.deletions, 0);
        assert!(diff.patch.contains("+++ b/feature.txt"), "{}", diff.patch);
        assert!(diff.patch.contains("+reviewable"), "{}", diff.patch);
        assert!(!diff.truncated);
        assert!(diff.patch.len() <= MAX_PR_DIFF_BYTES);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn pr_diff_caps_the_patch_and_reports_truncation() {
        let base = tmp_base("pr-diff-cap");
        let content = vec![b'x'; MAX_PR_DIFF_BYTES + 4096];
        let (forge, _, _) = materialized_pr(&base, &content);
        let bytes = futures::executor::block_on(forge.query(&encode_query(
            &ForgeQuery::PrDiff {
                repo: "demo".into(),
                number: 1,
            },
        )))
        .unwrap();
        let ForgeReply::PrDiff(diff) = decode_reply(&bytes).unwrap() else {
            panic!("wrong reply")
        };
        assert!(diff.truncated);
        assert_eq!(diff.patch.len(), MAX_PR_DIFF_BYTES);
        assert_eq!(diff.files_changed, 1, "full stats survive truncation");
        assert_eq!(diff.additions, 1, "full stats survive truncation");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn pr_diff_stops_patch_printing_at_the_response_cap() {
        let base = tmp_base("pr-diff-callback-stop");
        let (_, source, target) = materialized_pr(&base, &[b'x'; 4096]);
        let repo = git::open(&base.join("demo")).unwrap();
        let (patch, truncated, files_changed, additions, deletions) = git::bounded_diff(
            &repo,
            target,
            source,
            64,
            MAX_PR_DIFF_FILES,
            MAX_PR_DIFF_BLOB_BYTES,
        )
        .expect("the deliberate libgit2 callback abort is truncation, not an error");
        assert!(truncated);
        assert_eq!(patch.len(), 64);
        assert_eq!((files_changed, additions, deletions), (1, 1, 0));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn pr_diff_returns_empty_when_trees_are_identical() {
        let base = tmp_base("pr-diff-identical-trees");
        let repo = git::init(&base.join("repo")).unwrap();
        let tree_oid = repo.treebuilder(None).unwrap().write().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        let target = git::commit(&repo, &tree, None, "target", 1).unwrap();
        let parent = repo.find_commit(target).unwrap();
        let source = git::commit(&repo, &tree, Some(&parent), "source", 2).unwrap();

        let result = git::bounded_diff(
            &repo,
            target,
            source,
            MAX_PR_DIFF_BYTES,
            MAX_PR_DIFF_FILES,
            MAX_PR_DIFF_BLOB_BYTES,
        )
        .unwrap();
        assert_eq!(result, (String::new(), false, 0, 0, 0));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn pr_diff_rejects_an_oversized_commit_before_identical_tree_materialization() {
        let base = tmp_base("pr-diff-large-commit");
        let repo = git::init(&base.join("repo")).unwrap();
        let tree_oid = repo.treebuilder(None).unwrap().write().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        let target = git::commit(&repo, &tree, None, "target", 1).unwrap();
        let mut raw = format!(
            "tree {tree_oid}\nparent {target}\nauthor agent <agent@agents.duck> 2 +0000\ncommitter node <node@nodes.duck> 2 +0000\n\n"
        )
        .into_bytes();
        raw.resize(raw.len() + MAX_PR_DIFF_COMMIT_BYTES + 1, b'x');
        let source = repo
            .odb()
            .unwrap()
            .write(git2::ObjectType::Commit, &raw)
            .unwrap();

        let result = git::bounded_diff(
            &repo,
            target,
            source,
            MAX_PR_DIFF_BYTES,
            MAX_PR_DIFF_FILES,
            MAX_PR_DIFF_BLOB_BYTES,
        );
        assert!(
            matches!(
                result,
                Err(git::BoundedDiffError::TooLarge { commit_bytes, .. })
                    if commit_bytes > MAX_PR_DIFF_COMMIT_BYTES
            ),
            "commit headers must bound work before find_commit: {result:?}"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn pr_diff_preflight_handles_nested_add_delete_type_and_mode_changes() {
        let base = tmp_base("pr-diff-tree-walk");
        let repo = git::init(&base.join("repo")).unwrap();
        let old_blob = repo.blob(b"old\n").unwrap();
        let new_blob = repo.blob(b"new\n").unwrap();

        let mut old_nested = repo.treebuilder(None).unwrap();
        old_nested.insert("one.txt", old_blob, 0o100644).unwrap();
        old_nested.insert("two.txt", old_blob, 0o100644).unwrap();
        let old_nested_oid = old_nested.write().unwrap();
        let mut old_root = repo.treebuilder(None).unwrap();
        old_root.insert("node", old_nested_oid, 0o040000).unwrap();
        old_root.insert("mode.txt", old_blob, 0o100644).unwrap();
        old_root.insert("removed.txt", old_blob, 0o100644).unwrap();
        let old_tree = repo.find_tree(old_root.write().unwrap()).unwrap();
        let target = git::commit(&repo, &old_tree, None, "target", 1).unwrap();
        let target_commit = repo.find_commit(target).unwrap();

        let mut new_root = repo.treebuilder(None).unwrap();
        new_root.insert("node", new_blob, 0o100644).unwrap();
        new_root.insert("mode.txt", old_blob, 0o100755).unwrap();
        new_root.insert("added.txt", new_blob, 0o100644).unwrap();
        let new_tree = repo.find_tree(new_root.write().unwrap()).unwrap();
        let source =
            git::commit(&repo, &new_tree, Some(&target_commit), "source", 2).unwrap();

        let (patch, truncated, files_changed, _, _) = git::bounded_diff(
            &repo,
            target,
            source,
            MAX_PR_DIFF_BYTES,
            MAX_PR_DIFF_FILES,
            MAX_PR_DIFF_BLOB_BYTES,
        )
        .unwrap();
        assert!(!truncated);
        assert_eq!(files_changed, 6);
        for path in [
            "added.txt",
            "mode.txt",
            "node",
            "node/one.txt",
            "node/two.txt",
            "removed.txt",
        ] {
            assert!(patch.contains(path), "missing {path} from {patch}");
        }
        assert!(patch.contains("old mode 100644"), "{patch}");
        assert!(patch.contains("new mode 100755"), "{patch}");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn pr_diff_preflight_rejects_a_wide_mostly_unchanged_tree() {
        let base = tmp_base("pr-diff-wide-tree");
        let repo = git::init(&base.join("repo")).unwrap();
        let old_blob = repo.blob(b"old\n").unwrap();
        let new_blob = repo.blob(b"new\n").unwrap();
        let entries = MAX_PR_DIFF_TREE_ENTRIES / 2 + 1;
        let mut old_builder = repo.treebuilder(None).unwrap();
        for index in 0..entries {
            old_builder
                .insert(format!("entry-{index:05}.txt"), old_blob, 0o100644)
                .unwrap();
        }
        let old_tree = repo.find_tree(old_builder.write().unwrap()).unwrap();
        let target = git::commit(&repo, &old_tree, None, "target", 1).unwrap();
        let target_commit = repo.find_commit(target).unwrap();
        let mut new_builder = repo.treebuilder(Some(&old_tree)).unwrap();
        new_builder
            .insert("entry-00000.txt", new_blob, 0o100644)
            .unwrap();
        let new_tree = repo.find_tree(new_builder.write().unwrap()).unwrap();
        let source =
            git::commit(&repo, &new_tree, Some(&target_commit), "source", 2).unwrap();

        let result = git::bounded_diff(
            &repo,
            target,
            source,
            MAX_PR_DIFF_BYTES,
            MAX_PR_DIFF_FILES,
            MAX_PR_DIFF_BLOB_BYTES,
        );
        assert!(
            matches!(
                result,
                Err(git::BoundedDiffError::TooLarge { tree_entries, .. })
                    if tree_entries > MAX_PR_DIFF_TREE_ENTRIES
            ),
            "wide tree traversal must stop at its own work bound: {result:?}"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn pr_diff_preflight_rejects_excessive_tree_depth() {
        let base = tmp_base("pr-diff-deep-tree");
        let repo = git::init(&base.join("repo")).unwrap();
        let empty_oid = repo.treebuilder(None).unwrap().write().unwrap();
        let empty_tree = repo.find_tree(empty_oid).unwrap();
        let target = git::commit(&repo, &empty_tree, None, "target", 1).unwrap();
        let target_commit = repo.find_commit(target).unwrap();
        let blob = repo.blob(b"deep\n").unwrap();
        let mut leaf_builder = repo.treebuilder(None).unwrap();
        leaf_builder.insert("leaf.txt", blob, 0o100644).unwrap();
        let mut tree_oid = leaf_builder.write().unwrap();
        for _ in 0..=MAX_PR_DIFF_TREE_DEPTH {
            let mut builder = repo.treebuilder(None).unwrap();
            builder.insert("nested", tree_oid, 0o040000).unwrap();
            tree_oid = builder.write().unwrap();
        }
        let source_tree = repo.find_tree(tree_oid).unwrap();
        let source = git::commit(
            &repo,
            &source_tree,
            Some(&target_commit),
            "source",
            2,
        )
        .unwrap();

        let result = git::bounded_diff(
            &repo,
            target,
            source,
            MAX_PR_DIFF_BYTES,
            MAX_PR_DIFF_FILES,
            MAX_PR_DIFF_BLOB_BYTES,
        );
        assert!(
            matches!(
                result,
                Err(git::BoundedDiffError::TooLarge { tree_depth, .. })
                    if tree_depth > MAX_PR_DIFF_TREE_DEPTH
            ),
            "deep tree traversal must stop before unbounded recursion: {result:?}"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn pr_diff_preflight_rejects_a_missing_changed_blob() {
        let base = tmp_base("pr-diff-missing-blob");
        let repo = git::init(&base.join("repo")).unwrap();
        let empty_tree_oid = repo.treebuilder(None).unwrap().write().unwrap();
        let empty_tree = repo.find_tree(empty_tree_oid).unwrap();
        let target = git::commit(&repo, &empty_tree, None, "target", 1).unwrap();
        let target_commit = repo.find_commit(target).unwrap();
        let blob = repo.blob(b"will disappear\n").unwrap();
        let mut source_builder = repo.treebuilder(None).unwrap();
        source_builder.insert("missing.txt", blob, 0o100644).unwrap();
        let source_tree = repo
            .find_tree(source_builder.write().unwrap())
            .unwrap();
        let source =
            git::commit(&repo, &source_tree, Some(&target_commit), "source", 2).unwrap();
        let hex = blob.to_string();
        std::fs::remove_file(repo.path().join("objects").join(&hex[..2]).join(&hex[2..]))
            .unwrap();

        let result = git::bounded_diff(
            &repo,
            target,
            source,
            MAX_PR_DIFF_BYTES,
            MAX_PR_DIFF_FILES,
            MAX_PR_DIFF_BLOB_BYTES,
        );
        assert!(
            matches!(result, Err(git::BoundedDiffError::Git(_))),
            "missing changed blob must fail as an unavailable git object"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn pr_diff_rejects_too_many_changed_files_before_patch_generation() {
        let base = tmp_base("pr-diff-many-files");
        let (mut forge, _, target) = materialized_pr(&base, b"initial\n");
        let source = replace_pr_source_with_files(
            &mut forge,
            &base,
            target,
            MAX_PR_DIFF_FILES + 1,
            b"x\n",
        );
        let err = futures::executor::block_on(forge.query(&encode_query(
            &ForgeQuery::PrDiff {
                repo: "demo".into(),
                number: 1,
            },
        )))
        .unwrap_err()
        .to_string();
        assert!(err.contains("diff is too large to serve"), "{err}");
        assert!(
            err.contains(&format!("{} changed files", MAX_PR_DIFF_FILES + 1)),
            "{err}"
        );
        assert!(err.contains(&source.to_string()), "{err}");
        assert!(err.contains(&target.to_string()), "{err}");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn pr_diff_rejects_excessive_materialized_blob_bytes() {
        let base = tmp_base("pr-diff-large-blob");
        let content = vec![b'x'; MAX_PR_DIFF_BLOB_BYTES + 1];
        let (forge, source, target) = materialized_pr(&base, &content);
        let err = futures::executor::block_on(forge.query(&encode_query(
            &ForgeQuery::PrDiff {
                repo: "demo".into(),
                number: 1,
            },
        )))
        .unwrap_err()
        .to_string();
        assert!(err.contains("diff is too large to serve"), "{err}");
        assert!(
            err.contains(&format!("{} materialized blob bytes", content.len())),
            "{err}"
        );
        assert!(err.contains(&source.to_string()), "{err}");
        assert!(err.contains(&target.to_string()), "{err}");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn pr_diff_fails_honestly_when_a_pinned_object_is_unavailable() {
        let base = tmp_base("pr-diff-missing");
        let (mut forge, _, target) = materialized_pr(&base, b"present\n");
        let missing = oid('f');
        forge
            .repos
            .get_mut("demo")
            .unwrap()
            .refs
            .insert("feature".into(), missing);
        let err = futures::executor::block_on(forge.query(&encode_query(
            &ForgeQuery::PrDiff {
                repo: "demo".into(),
                number: 1,
            },
        )))
        .unwrap_err()
        .to_string();
        assert!(err.contains("not fully materialized"), "{err}");
        assert!(err.contains(&missing.to_string()), "{err}");
        assert!(err.contains(&target.to_string()), "{err}");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn pr_diff_rejects_an_issue_before_reading_git_objects() {
        let base = tmp_base("issue-diff");
        let mut forge = Forge::init("forge", base.clone()).unwrap();
        let mut ctx = ctx_with_origin(1, user_origin(1));
        exec_commit(
            &mut forge,
            &mut ctx,
            &ForgeMsg::OpenIssue {
                repo: "demo".into(),
                title: "not a pr".into(),
                body: String::new(),
            },
        );
        let err = futures::executor::block_on(forge.query(&encode_query(
            &ForgeQuery::PrDiff {
                repo: "demo".into(),
                number: 1,
            },
        )))
        .unwrap_err()
        .to_string();
        assert!(err.contains("issue, not a pull request"), "{err}");
        let _ = std::fs::remove_dir_all(&base);
    }

    fn user_origin(b: u8) -> sdk::Origin {
        sdk::Origin::External(vec![b; 8])
    }

    #[test]
    fn legacy_commit_rejects_without_touching_state_or_disk() {
        let base = tmp_base("basic");
        let mut forge = Forge::init("forge", base.clone()).unwrap();
        assert_eq!(forge.root(), StateRoot::ZERO, "empty namespace -> ZERO root");
        let err = futures::executor::block_on(forge.execute(
            &mut ctx_at(100),
            &commit_msg("", "a.txt", "hello", "first"),
        ))
        .unwrap_err()
        .to_string();
        assert!(err.contains("Commit is retired"), "{err}");
        assert_eq!(forge.root(), StateRoot::ZERO);
        assert!(!base.join(DEFAULT_REPO).exists(), "execute performs no Git IO");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn successive_pushes_move_the_root() {
        let base = tmp_base("second");
        let mut forge = Forge::init("forge", base.clone()).unwrap();
        let a = oid('a');
        let b = oid('b');
        push_head(&mut forge, "", None, a);
        let r1 = forge.root();
        push_head(&mut forge, "", Some(a), b);
        let r2 = forge.root();
        assert_ne!(r1, r2, "a second pushed head must advance the root");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn root_composes_into_global_root() {
        let base = tmp_base("compose");
        let mut forge = Forge::init("forge", base.clone()).unwrap();
        let before = host::global_root(&[&forge as &dyn Module]);
        push_head(&mut forge, "", None, oid('a'));
        let after = host::global_root(&[&forge as &dyn Module]);
        assert_ne!(before, after, "forge's root must move the global app-hash");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn pushed_head_root_is_reproducible_across_namespaces() {
        let a = tmp_base("det-a");
        let b = tmp_base("det-b");
        let mut fa = Forge::init("forge", a.clone()).unwrap();
        let mut fb = Forge::init("forge", b.clone()).unwrap();
        push_head(&mut fa, "myrepo", None, oid('a'));
        push_head(&mut fb, "myrepo", None, oid('a'));
        assert_eq!(fa.root(), fb.root(), "same fixed oid -> identical root");
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
        let mut ctx = ctx_at(1);
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
        let mut ctx = ctx_at(2);
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

        let mut ctx = ctx_at(3);
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
        let mut ctx = ctx_at(4);
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

        let mut ctx = ctx_at(5);
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
        let mut ctx = ctx_at(1);
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
        let mut ctx = ctx_with_origin(2, user_origin(1));
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
        assert_eq!(ctx.msgs().len(), 1);
        assert_eq!(ctx.msgs()[0].target, "chat");
        let chat::ChatMsg::CreateChannel { channel_id, name, .. } =
            chat::decode_msg(&ctx.msgs()[0].payload).unwrap()
        else {
            panic!("expected CreateChannel")
        };
        assert_eq!(channel_id, "forge:demo:1");
        assert_eq!(name, "demo#1");

        // a PR from the born feature branch: shares the number space (#2).
        let mut ctx = ctx_with_origin(3, user_origin(2));
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
        assert_eq!(ctx.msgs().len(), 1, "channel follow-up for the PR");

        // a PR from an unborn branch rejects.
        let mut ctx = ctx_with_origin(4, user_origin(2));
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
        let mut ctx = ctx_with_origin(5, user_origin(3));
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
        assert_eq!(ctx.msgs().len(), 1, "approval line emitted");

        // merge: stale target CAS rejects; the real one moves dev AND marks
        // the PR merged in one block.
        let mut ctx = ctx_with_origin(6, user_origin(2));
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

        let mut ctx = ctx_with_origin(7, user_origin(2));
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
        assert_eq!(ctx.msgs().len(), 1, "merged line emitted");

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
        let mut ctx = ctx_with_origin(8, user_origin(2));
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
            let mut ctx = ctx_at(1);
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
            let mut ctx = ctx_with_origin(2, user_origin(9));
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
        seed_materialized_commit(&mut forge, 42, "docs", "a.txt", "x", "c");
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

        // Real fixture objects on main, then a second branch on the same oid —
        // its objects exist, so the snapshot pack closes.
        seed_materialized_commit(&mut forge, 1, "demo", "a.txt", "hello", "c1");
        let head = git_head_oid(&base, "demo");
        let mut ctx = ctx_at(2);
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
        let mut ctx = ctx_with_origin(3, user_origin(4));
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
