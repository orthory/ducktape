//! forge — a GIT-backed feature module.
//!
//! where the directory module keeps a `BTreeMap` and kv keeps a qmdb, forge's
//! private substrate is a real on-disk git repository. its authenticated
//! [`StateRoot`] is the repo's HEAD commit oid: in sha256-mode git that oid is
//! exactly 32 bytes, so `root()` IS the HEAD oid verbatim, and it composes into
//! the global app-hash next to a qmdb merkle root with zero special-casing.
//!
//! ## the determinism landmine (single-node slice)
//!
//! a git *commit* embeds committer identity + a timestamp, so two nodes
//! committing the same content would normally get DIFFERENT commit oids — and
//! the app-hash would fork. this slice keeps the commit reproducible anyway:
//!
//! - `root()` is the repo's current HEAD commit oid as a 32-byte [`StateRoot`];
//! - the commit object uses a FIXED author/committer identity (`ducktape`) and a
//!   date derived from `ctx.env().consensus_time` (NOT wall clock), so the oid is
//!   byte-identical across independent repos given the same inputs (verified);
//! - the tree is built in an isolated index (`GIT_INDEX_FILE`), so it's a pure
//!   function of (parent, change) with no worktree cruft.
//!
//! deliberately git PLUMBING, not porcelain `git add` + `git commit`: porcelain
//! would inherit host config (`commit.gpgsign` -> a nondeterministic signature,
//! `core.autocrlf` -> mangled blob bytes) and the checked-out worktree, either of
//! which reintroduces the fork. the path is `hash-object` -> isolated-index
//! `write-tree` -> `commit-tree` -> `update-ref`. a CONSEQUENCE: the worktree is
//! never materialized — HEAD's tree is authoritative but `git status` in the repo
//! shows the committed files as "deleted" (the working dir is empty). that's
//! intentional for this substrate (nothing reads the worktree), not a bug.
//!
//! ### deferred to the p2p port (faithful multi-node)
//!
//! true cross-node convergence is "results on the wire, not commands": the wire
//! fact is a `RefUpdate { name, target_oid, prev }` applied by `git update-ref`
//! on receivers, which NEVER run `git commit`. only the origin commits locally
//! (the `execute` below). the pinned identity/date keeps the origin's oid
//! reproducible as a backstop, but RefUpdate stays canonical because bit-
//! identical commit encoding across git builds isn't guaranteed. that split
//! (origin commits + emits RefUpdate; receivers fetch closure + update-ref) is
//! out of scope for this single-node demo.
//!
//! ### the sha1 fallback
//!
//! if the host git lacks sha256 support, `init` falls back to sha1 (20-byte
//! oids) and `root()` becomes `sha256(oid_bytes)` — a stable 32-byte commitment
//! one indirection removed. NB: object-format is a genesis-uniform consensus
//! parameter, not per-node graceful degradation — a sha256 node's root (oid `X`)
//! and a sha1 node's root (`sha256(Y)`) never agree, so a mixed validator set
//! would fork. sha256 is the happy path; the fallback exists so the demo/tests
//! still run on a host without sha256 git.

mod git;

use std::path::PathBuf;

use forge_interface::{decode_msg, decode_query, encode_reply, ForgeMsg, ForgeQuery, ForgeReply};
use sdk::{Ctx, Error, Module, ModuleId, Msg, StateRoot};
use sha2::{Digest, Sha256};

use git::ObjectFormat;

/// the canonical branch this module commits to and reads HEAD from.
const MAIN_REF: &str = "refs/heads/main";

pub struct Forge {
    id: ModuleId,
    /// node-local repo dir — NOT consensus state (the path may differ per node);
    /// only the HEAD oid it produces is.
    repo: PathBuf,
    /// the object format the repo was initialized in (drives oid -> root mapping).
    fmt: ObjectFormat,
    /// write-through mirror of `MAIN_REF`: written last in `execute` (and at
    /// genesis), read ONLY by `root()`/`query`. the repo/ref is the source of
    /// truth for the parent — this cache never feeds a commit's parent, so an
    /// error mid-commit can't strand a stale head. `None` == unborn repo.
    head: Option<String>,
}

impl Forge {
    /// genesis wiring: init (or adopt) a git repo at `repo_dir`, then seed the
    /// cached head from its current `MAIN_REF` (`None` on a fresh empty repo, so
    /// `root()` starts at [`StateRoot::ZERO`]). deterministic given the dir state.
    pub fn init(id: impl Into<ModuleId>, repo_dir: impl Into<PathBuf>) -> Result<Self, Error> {
        let repo = repo_dir.into();
        let dot_git = repo.join(".git");
        let fmt = if dot_git.exists() {
            git::object_format(&repo).map_err(|e| Error::Module(e.to_string()))?
        } else {
            git::init(&repo).map_err(|e| Error::Module(e.to_string()))?
        };
        let head = git::resolve_ref(&repo, MAIN_REF).map_err(|e| Error::Module(e.to_string()))?;
        Ok(Self { id: id.into(), repo, fmt, head })
    }

    /// map a HEAD oid hex into the fixed-width state root per the repo's format.
    /// sha256: the 32 oid bytes ARE the root. sha1: `sha256(oid_bytes)`.
    fn oid_to_root(&self, hex: &str) -> Result<StateRoot, Error> {
        match self.fmt {
            ObjectFormat::Sha256 => {
                Ok(StateRoot(git::oid_bytes32(hex).map_err(|e| Error::Module(e.to_string()))?))
            }
            ObjectFormat::Sha1 => {
                // 40-char sha1 hex -> 20 bytes -> sha256 -> 32-byte commitment.
                if hex.len() != 40 {
                    return Err(Error::Module(format!("expected 40 hex chars, got {}", hex.len())));
                }
                let mut raw = [0u8; 20];
                for i in 0..20 {
                    raw[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
                        .map_err(|_| Error::Module(format!("bad sha1 oid: {hex}")))?;
                }
                let mut h = Sha256::new();
                h.update(raw);
                Ok(StateRoot(h.finalize().into()))
            }
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Forge {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// the repo's HEAD commit oid as a [`StateRoot`] — pure, no IO (that's the
    /// whole reason `head` is a write-through cache). `None` -> `ZERO`.
    fn root(&self) -> StateRoot {
        match &self.head {
            None => StateRoot::ZERO,
            Some(hex) => self.oid_to_root(hex).unwrap_or(StateRoot::ZERO),
        }
    }

    /// commit one file change to the repo. deterministic: fixed identity + a
    /// `consensus_time`-derived date + an isolated-index tree build, so the
    /// resulting commit oid is reproducible. all git IO is blocking with no
    /// `.await`, so the "await only deterministic resources" rule holds vacuously
    /// — the git shell-out is forge's private state substrate, not an effect.
    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let ForgeMsg::Commit { path, content, message } =
            decode_msg(&msg.payload).map_err(Error::Module)?;

        // 1. parent := the REPO's current head (source of truth, not the cache).
        let parent =
            git::resolve_ref(&self.repo, MAIN_REF).map_err(|e| Error::Module(e.to_string()))?;
        let parent_tree = match &parent {
            Some(p) => Some(
                git::commit_tree_oid(&self.repo, p).map_err(|e| Error::Module(e.to_string()))?,
            ),
            None => None,
        };

        // 2. stage the blob into an isolated index and write the tree.
        let blob = git::hash_blob(&self.repo, content.as_bytes())
            .map_err(|e| Error::Module(e.to_string()))?;
        let index = git::index_path(&self.repo);
        let tree = git::write_tree_with(
            &self.repo,
            &index,
            parent_tree.as_deref(),
            &path,
            &blob,
        )
        .map_err(|e| Error::Module(e.to_string()))?;

        // 3. deterministic commit object: date from consensus_time, fixed identity.
        let ts = ctx.env().consensus_time;
        let commit = git::commit_tree(&self.repo, &tree, parent.as_deref(), &message, ts)
            .map_err(|e| Error::Module(e.to_string()))?;

        // 4. move the local ref (single-node). multi-node applies this same
        //    primitive on receipt of a wire RefUpdate — never a fresh commit.
        git::update_ref(&self.repo, MAIN_REF, &commit)
            .map_err(|e| Error::Module(e.to_string()))?;

        // 5. LAST + infallible: refresh the write-through head mirror. an error
        //    before this can't leave a stale cached head (step 1 re-reads the repo).
        self.head = Some(commit);
        Ok(())
    }

    /// read projection: the current HEAD as hex (or `None` on an unborn repo).
    /// served straight from the cached mirror — no IO, no `.await`. in sha256
    /// mode this hex equals `root()`'s bytes; the sha1 fallback breaks that
    /// identity (root is `sha256(oid)`) but the raw oid hex is still returned here.
    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            ForgeQuery::Head => Ok(encode_reply(&ForgeReply::Head(self.head.clone()))),
        }
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

    // read HEAD hex via the git cli directly — the independent oracle that
    // root() really is the repo's HEAD, not just "some 32 bytes that moved".
    fn git_head_hex(repo: &PathBuf) -> String {
        git::resolve_ref(repo, MAIN_REF).unwrap().unwrap()
    }

    #[test]
    fn genesis_is_zero_then_commit_makes_root_equal_head() {
        let dir = tmp_repo("basic");
        let mut forge = Forge::init("forge", dir.clone()).unwrap();
        assert_eq!(forge.root(), StateRoot::ZERO, "unborn repo -> ZERO root");

        futures::executor::block_on(
            forge.execute(&mut TestCtx::at(100), &commit_msg("a.txt", "hello", "first")),
        )
        .unwrap();

        assert_ne!(forge.root(), StateRoot::ZERO, "a commit must move the root off ZERO");

        // root IS the git HEAD oid (the load-bearing identity in sha256 mode).
        let head_hex = git_head_hex(&dir);
        assert_eq!(
            forge.root(),
            forge.oid_to_root(&head_hex).unwrap(),
            "root() must equal the real git HEAD oid"
        );

        // and query(Head) surfaces that same hex.
        let reply =
            futures::executor::block_on(forge.query(&encode_query(&ForgeQuery::Head))).unwrap();
        assert_eq!(decode_reply(&reply).unwrap(), ForgeReply::Head(Some(head_hex)));

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
        let r1 = forge.root();
        futures::executor::block_on(
            forge.execute(&mut TestCtx::at(2), &commit_msg("b.txt", "two", "c2")),
        )
        .unwrap();
        let r2 = forge.root();
        assert_ne!(r1, r2, "a second commit must advance the root");
        assert_eq!(r2, forge.oid_to_root(&git_head_hex(&dir)).unwrap());
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
        let after = state::global_root(&[&forge as &dyn Module]);
        assert_ne!(before, after, "forge's git-backed root must move the global app-hash");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // determinism: two independent repos, same inputs -> identical HEAD oid.
    #[test]
    fn commit_oid_is_reproducible_across_repos() {
        let da = tmp_repo("det-a");
        let db = tmp_repo("det-b");
        let mut fa = Forge::init("forge", da.clone()).unwrap();
        let mut fb = Forge::init("forge", db.clone()).unwrap();
        futures::executor::block_on(
            fa.execute(&mut TestCtx::at(555), &commit_msg("f.txt", "same", "same-msg")),
        )
        .unwrap();
        futures::executor::block_on(
            fb.execute(&mut TestCtx::at(555), &commit_msg("f.txt", "same", "same-msg")),
        )
        .unwrap();
        assert_eq!(fa.root(), fb.root(), "pinned identity+date -> reproducible commit oid");
        let _ = std::fs::remove_dir_all(&da);
        let _ = std::fs::remove_dir_all(&db);
    }
}
