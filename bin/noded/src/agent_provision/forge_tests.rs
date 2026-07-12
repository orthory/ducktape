//! forge-lane tests against temp SHA-1 repos constructed the way forge
//! materializes them: libgit2 init (non-bare, initial_head main, hermetic),
//! commits written straight into the odb with the ref set and NOTHING ever
//! checked out. the provisioner ops run REAL host `git` (same as production);
//! pushes rendezvous at a `file://` bare repo — the URL is config, production
//! passes the loopback http URL (that lane is Task 6's e2e). stock git's
//! fetch-first refusal on the bare remote is the CAS-reject stand-in.

use super::super::NodedProvisioner;
use super::super::plane_tests::{
    SKILL_BODY, SKILL_FILE, skill_mount, skill_tree, spawn_files_actor,
};
use super::*;
use dispatch_oracle::WorkspaceProvisioner as _;

const REPO: &str = "app";
const BRANCH: &str = "agent/item-7";
const AGENT: &str = "quackbot";
const AGENT_DISPLAY_NAME: &str = "Quack Agent";
const NODE_IDENT: &str = "node-f00f";

/// init a repo EXACTLY as forge::git::init does (that fn is crate-private):
/// non-bare, `initial_head("main")`, `external_template(false)` — sha1 by
/// default, `.git` exists, working tree never checked out.
fn init_repo(dir: &Path) -> git2::Repository {
    std::fs::create_dir_all(dir).unwrap();
    let mut opts = git2::RepositoryInitOptions::new();
    opts.initial_head("main").external_template(false);
    git2::Repository::init_opts(dir, &opts).unwrap()
}

/// write a commit straight into the odb (no index, no checkout — forge's
/// materialization shape) and return its hex oid. moves NO ref.
fn odb_commit(
    repo: &git2::Repository,
    parent: Option<&str>,
    path: &str,
    content: &str,
) -> String {
    let blob = repo.blob(content.as_bytes()).unwrap();
    let parent_commit =
        parent.map(|hex| repo.find_commit(git2::Oid::from_str(hex).unwrap()).unwrap());
    let base_tree = parent_commit.as_ref().map(|c| c.tree().unwrap());
    let mut tb = repo.treebuilder(base_tree.as_ref()).unwrap();
    tb.insert(path, blob, 0o100644).unwrap();
    let tree = repo.find_tree(tb.write().unwrap()).unwrap();
    let t = git2::Time::new(1_000, 0);
    let sig = git2::Signature::new("ducktape", "ducktape@localhost", &t).unwrap();
    let parents: Vec<&git2::Commit> = parent_commit.iter().collect();
    repo.commit(None, &sig, &sig, "seed", &tree, &parents)
        .unwrap()
        .to_string()
}

fn set_ref(repo: &git2::Repository, branch: &str, oid: &str) {
    repo.reference(
        &format!("refs/heads/{branch}"),
        git2::Oid::from_str(oid).unwrap(),
        true,
        "test",
    )
    .unwrap();
}

fn ref_oid(repo_dir: &Path, branch: &str) -> Option<String> {
    let repo = git2::Repository::open(repo_dir).unwrap();
    repo.refname_to_id(&format!("refs/heads/{branch}"))
        .ok()
        .map(|o| o.to_string())
}

fn git_stdout(dir: &Path, args: &[&str]) -> String {
    run_git(dir, args, &[]).unwrap_or_else(|e| panic!("git {args:?}: {e}"))
}

/// `unwrap_err` for provision results (`dyn ProvisionedWorkspace` is not Debug).
#[allow(clippy::type_complexity)]
fn provision_err(result: Result<Box<dyn ProvisionedWorkspace>, String>) -> String {
    match result {
        Ok(_) => panic!("provision unexpectedly succeeded"),
        Err(e) => e,
    }
}

/// registered (non-prunable) worktrees of `repo_dir`, primary included.
fn worktree_count(repo_dir: &Path) -> usize {
    git_stdout(repo_dir, &["worktree", "list", "--porcelain"])
        .lines()
        .filter(|l| l.starts_with("worktree "))
        .count()
}

/// one test bed: a materialized-shape substrate repo at `<base>/app` with
/// `main` born at `head`, a bare-repo rendezvous dir for pushes, and a runs
/// root — all under one temp dir.
struct Bed {
    _tmp: tempfile::TempDir,
    repo_base: PathBuf,
    repo_dir: PathBuf,
    bares: PathBuf,
    runs_root: PathBuf,
    head: String,
}

fn bed() -> Bed {
    let tmp = tempfile::tempdir().unwrap();
    let repo_base = tmp.path().join("forge-git");
    let repo_dir = repo_base.join(REPO);
    let repo = init_repo(&repo_dir);
    let head = odb_commit(&repo, None, "readme.md", "hello\n");
    set_ref(&repo, "main", &head);
    let bares = tmp.path().join("bares");
    std::fs::create_dir_all(&bares).unwrap();
    let runs_root = tmp.path().join("runs");
    std::fs::create_dir_all(&runs_root).unwrap();
    Bed {
        _tmp: tmp,
        repo_base,
        repo_dir,
        bares,
        runs_root,
        head,
    }
}

impl Bed {
    fn push_base(&self) -> String {
        format!("file://{}", self.bares.display())
    }

    fn provisioner(&self) -> NodedProvisioner {
        let (handle, _rx, _hub) = NodeHandle::channel();
        NodedProvisioner::new(handle.with_forge_repo(&self.repo_base), &self.runs_root)
            .with_forge(Some(self.push_base()), NODE_IDENT)
    }

    /// a provisioner whose actor lane is SERVED (the plain one above drops its
    /// receiver — fine for runs with no mounts, but a W6 checkout needs a node
    /// on the other end). `reject_reads` fails the mount checkout mid-way. the
    /// actor handle must outlive the provision, so it comes back with it.
    fn skill_provisioner(
        &self,
        reject_reads: bool,
    ) -> (NodedProvisioner, tokio::task::JoinHandle<()>) {
        let (handle, rx, _hub) = NodeHandle::channel();
        let actor = spawn_files_actor(rx, skill_tree(), reject_reads);
        let prov = NodedProvisioner::new(handle.with_forge_repo(&self.repo_base), &self.runs_root)
            .with_forge(Some(self.push_base()), NODE_IDENT);
        (prov, actor)
    }

    /// [`Self::spec`] with the W6 skill mount the plane tests share.
    fn skill_spec(&self, run_id: &str, commit: &str) -> WorkspaceSpec {
        WorkspaceSpec {
            ro_mounts: vec![skill_mount()],
            ..self.spec(run_id, commit, false)
        }
    }

    /// snapshot the substrate into the bare rendezvous repo (ALL refs) —
    /// the stand-in for the forge remote's committed state at compose time.
    fn snapshot_bare(&self) -> PathBuf {
        let dst = self.bares.join(REPO);
        run_git(
            &self.bares,
            &[
                "clone",
                "--bare",
                "-q",
                self.repo_dir.to_str().unwrap(),
                dst.to_str().unwrap(),
            ],
            &[],
        )
        .unwrap();
        dst
    }

    fn spec(&self, run_id: &str, commit: &str, branch_born: bool) -> WorkspaceSpec {
        WorkspaceSpec {
            run_id: run_id.into(),
            // the forge lane's session bind rides the same id as every other:
            // the one `runs` resolves (a forge item run is a chat run on the
            // item's channel), never the host-local `run_id` above.
            consensus_run_id: Some(runs::run_id_for(&format!("forge:{REPO}:7"), 1, AGENT)),
            agent_id: Some(AGENT.into()),
            agent_display_name: Some(AGENT_DISPLAY_NAME.into()),
            source: WorkspaceSource::Forge {
                repo: REPO.into(),
                commit: commit.into(),
                branch: BRANCH.into(),
                branch_born,
            },
            ro_mounts: Vec::new(),
        }
    }
}

// ---- construction: probe + config legs -----------------------------------

#[test]
fn the_probe_passes_on_a_runtime_compatible_host_git() {
    // the suite itself requires host git; the REAL probe must agree.
    probe_host_git().expect("this box has a runtime-compatible git");
}

#[test]
fn the_probe_fails_loud_when_git_is_absent() {
    let err = probe_host_git_with("ducktape-no-such-git-binary").unwrap_err();
    assert!(
        err.contains("probe failed") && err.contains("failed to spawn"),
        "the failure names the probe and the cause: {err}"
    );
}

#[cfg(unix)]
#[test]
fn the_probe_rejects_git_without_the_runtime_rebase_options() {
    use std::os::unix::fs::PermissionsExt as _;

    let tmp = tempfile::tempdir().unwrap();
    let shim = tmp.path().join("git-with-old-rebase");
    std::fs::write(
        &shim,
        "#!/bin/sh\n\
         for arg in \"$@\"; do\n\
           case \"$arg\" in\n\
             --reapply-cherry-picks|--empty=keep)\n\
               echo \"error: unknown option $arg\" >&2\n\
               exit 129\n\
               ;;\n\
           esac\n\
         done\n\
         exec git \"$@\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();

    let err = probe_host_git_with(shim.to_str().unwrap()).unwrap_err();
    assert!(err.contains("runtime-compatible git"), "{err}");
    assert!(err.contains("runtime rebase options"), "{err}");
    assert!(err.contains("unknown option"), "{err}");
}

#[tokio::test]
async fn a_failed_probe_is_permanent_and_loud_and_leaves_no_debris() {
    let bed = bed();
    let (handle, _rx, _hub) = NodeHandle::channel();
    let prov = NodedProvisioner::new(handle.with_forge_repo(&bed.repo_base), &bed.runs_root)
        .with_forge_probed(Some(bed.push_base()), NODE_IDENT, || {
            Err("git probe exploded".into())
        });
    // PERMANENT: every forge attempt fails with the construction-time reason.
    for run in ["s1:0", "s1:1"] {
        let err = provision_err(prov.provision(&bed.spec(run, &bed.head, false)).await);
        assert!(
            err.contains("unavailable") && err.contains("git probe exploded"),
            "{err}"
        );
    }
    assert_eq!(
        std::fs::read_dir(&bed.runs_root).unwrap().count(),
        0,
        "an unavailable lane mints nothing"
    );
}

#[tokio::test]
async fn no_http_surface_means_a_clear_forge_provision_error() {
    let bed = bed();
    let (handle, _rx, _hub) = NodeHandle::channel();
    let prov = NodedProvisioner::new(handle.with_forge_repo(&bed.repo_base), &bed.runs_root)
        .with_forge(None, NODE_IDENT);
    let err = provision_err(prov.provision(&bed.spec("s1:0", &bed.head, false)).await);
    assert!(err.contains("no http surface"), "{err}");
}

#[tokio::test]
async fn a_handle_without_a_forge_repo_base_is_a_clear_error() {
    let bed = bed();
    let (handle, _rx, _hub) = NodeHandle::channel(); // no with_forge_repo
    let prov = NodedProvisioner::new(handle, &bed.runs_root)
        .with_forge(Some(bed.push_base()), NODE_IDENT);
    let err = provision_err(prov.provision(&bed.spec("s1:0", &bed.head, false)).await);
    assert!(err.contains("no forge repo base"), "{err}");
}

#[test]
fn the_push_base_rewrites_wildcard_binds_to_loopback() {
    assert_eq!(
        forge_push_base(Some("0.0.0.0:8844")).as_deref(),
        Some("http://127.0.0.1:8844/forge")
    );
    // a v6 wildcard rewrites to the v6 loopback: a bindv6only [::] listener
    // refuses 127.0.0.1, which broke every push AND the mid-loop fetch dial.
    assert_eq!(
        forge_push_base(Some("[::]:9001")).as_deref(),
        Some("http://[::1]:9001/forge")
    );
    assert_eq!(
        forge_push_base(Some("127.0.0.1:8844")).as_deref(),
        Some("http://127.0.0.1:8844/forge")
    );
    assert_eq!(
        forge_push_base(Some("[::1]:8844")).as_deref(),
        Some("http://[::1]:8844/forge")
    );
    // a hostname bind is trusted verbatim (the operator chose it).
    assert_eq!(
        forge_push_base(Some("localhost:8844")).as_deref(),
        Some("http://localhost:8844/forge")
    );
    assert_eq!(forge_push_base(None), None);
}

// ---- provision ------------------------------------------------------------

#[tokio::test]
async fn provisions_a_worktree_at_the_pinned_commit_from_an_odb_only_repo() {
    // the substrate repo has NO checkout (forge materialization shape) — the
    // worktree must still materialize the pinned tree.
    let bed = bed();
    let ws = bed
        .provisioner()
        .provision(&bed.spec("s1:0", &bed.head, false))
        .await
        .expect("provision");
    let dir = ws.workdir();
    assert!(dir.starts_with(&bed.runs_root), "run dir under the W1 root");
    assert_eq!(
        std::fs::read_to_string(dir.join("readme.md")).unwrap(),
        "hello\n"
    );
    assert_eq!(git_stdout(&dir, &["rev-parse", "HEAD"]), bed.head);
    // DETACHED: no branch is checked out (no shared-repo ref to hold or move).
    assert_eq!(git_stdout(&dir, &["branch", "--show-current"]), "");
    assert_eq!(
        ws.env().get("DUCKTAPE_RUN_WORKSPACE"),
        Some(&dir.display().to_string())
    );
    ws.cleanup().await;
}

#[tokio::test]
async fn provision_never_touches_shared_repo_refs() {
    let bed = bed();
    // a node-local ref for the work branch already exists (consensus
    // materialized it, or a dead run left it) pointing somewhere else — the
    // detached provision must neither read nor move it: the pin is the base.
    let repo = git2::Repository::open(&bed.repo_dir).unwrap();
    let stale = odb_commit(&repo, Some(&bed.head), "junk.txt", "stale");
    set_ref(&repo, BRANCH, &stale);

    let ws = bed
        .provisioner()
        .provision(&bed.spec("s1:0", &bed.head, false))
        .await
        .expect("provision");
    assert_eq!(git_stdout(&ws.workdir(), &["rev-parse", "HEAD"]), bed.head);
    assert_eq!(
        ref_oid(&bed.repo_dir, BRANCH).as_deref(),
        Some(stale.as_str()),
        "the shared-repo ref is NOT ours to move — it stays put"
    );
    ws.cleanup().await;
}

#[tokio::test]
async fn a_born_branch_is_forced_to_the_committed_tip_and_pushes_fast_forward() {
    let bed = bed();
    let repo = git2::Repository::open(&bed.repo_dir).unwrap();
    // session so far: the work branch is BORN at `tip` in committed refs...
    let tip = odb_commit(&repo, Some(&bed.head), "work.txt", "session so far\n");
    set_ref(&repo, BRANCH, &tip);
    let bare = bed.snapshot_bare(); // the remote knows main + BRANCH@tip
    // ...but the node-local ref has drifted (leftover from a torn run).
    let drift = odb_commit(&repo, Some(&bed.head), "drift.txt", "node-local drift\n");
    set_ref(&repo, BRANCH, &drift);

    let spec = bed.spec("s2:0", &tip, true);
    let ws = bed.provisioner().provision(&spec).await.expect("provision");
    let dir = ws.workdir();
    assert_eq!(
        git_stdout(&dir, &["rev-parse", "HEAD"]),
        tip,
        "the detached checkout sits at the PIN — node-local drift is invisible"
    );

    // continuation run: new work fast-forwards the born branch (CAS holds:
    // remote is exactly at the pinned base).
    std::fs::write(dir.join("more.txt"), "more work\n").unwrap();
    let receipt = ws.commit("agent run s2:0").await.expect("commit+push");
    let new_oid = receipt.output_commit.clone().expect("pushed oid");
    assert_ne!(new_oid, tip);
    assert_eq!(receipt.branch.as_deref(), Some(BRANCH));
    assert_eq!(
        ref_oid(&bare, BRANCH).as_deref(),
        Some(new_oid.as_str()),
        "the remote fast-forwarded to the run's commit"
    );
    // the new commit builds ON the pinned tip — never a rebase, never a force.
    assert_eq!(
        git_stdout(&dir, &["rev-parse", "HEAD^"]),
        tip,
        "the fork base is the pinned commit"
    );
    ws.cleanup().await;
}

#[tokio::test]
async fn concurrent_attempts_of_one_item_both_provision_detached() {
    // no branch refs are held, so there is nothing to refuse — both attempts
    // run and the push loop orders whoever finishes second.
    let bed = bed();
    let prov = bed.provisioner();
    let a = prov
        .provision(&bed.spec("s1:0", &bed.head, false))
        .await
        .expect("first attempt");
    let b = prov
        .provision(&bed.spec("s1:1", &bed.head, false))
        .await
        .expect("second attempt provisions too — detached HEADs don't contend");
    assert_ne!(a.workdir(), b.workdir());
    a.cleanup().await;
    b.cleanup().await;
}

#[tokio::test]
async fn a_shared_repo_ref_force_move_mid_run_cannot_reparent_the_commit() {
    // THE hazard the detached checkout closes: consensus catch-up force-moves
    // refs/heads/<branch> in the shared repo WHILE the run executes. a
    // worktree holding that branch would commit onto the moved tip with its
    // old tree — silently reverting the interloper, then fast-forwarding past
    // the CAS. detached, the base stays the pin; the interloper is folded in
    // by the FETCH-driven rebase and its content survives.
    let bed = bed();
    let bare = bed.snapshot_bare();
    let ws = bed
        .provisioner()
        .provision(&bed.spec("s1:0", &bed.head, true))
        .await
        .expect("provision");
    let dir = ws.workdir();

    // mid-run: the interloper lands remotely AND catch-up moves the local ref.
    let bare_repo = git2::Repository::open(&bare).unwrap();
    let c2 = odb_commit(&bare_repo, Some(&bed.head), "rival.txt", "interloper content\n");
    set_ref(&bare_repo, BRANCH, &c2);
    let local = git2::Repository::open(&bed.repo_dir).unwrap();
    let c2_local = odb_commit(&local, Some(&bed.head), "rival.txt", "interloper content\n");
    assert_eq!(c2_local, c2, "same commit both sides (fixed identities)");
    set_ref(&local, BRANCH, &c2);

    std::fs::write(dir.join("mine.txt"), "the run's work\n").unwrap();
    let receipt = ws.commit("agent run s1:0").await.expect("rebase-retry push");
    assert!(receipt.rebased);
    let oid = receipt.output_commit.expect("post-rebase oid");
    assert_eq!(ref_oid(&bare, BRANCH).as_deref(), Some(oid.as_str()));
    // the interloper stays an ANCESTOR and its content SURVIVES at the tip —
    // never silently reverted.
    assert_eq!(git_stdout(&dir, &["rev-parse", "HEAD^"]), c2);
    assert_eq!(
        git_stdout(&dir, &["show", "HEAD:rival.txt"]),
        "interloper content"
    );
    assert_eq!(git_stdout(&dir, &["show", "HEAD:mine.txt"]), "the run's work");
    ws.cleanup().await;
}

#[tokio::test]
async fn a_missing_pinned_commit_fails_provision_before_any_worktree() {
    // committed refs can lead the on-disk pack (materialization lag) — a
    // wrong-base worktree must never be minted.
    let bed = bed();
    let absent = "ab".repeat(20);
    let err = provision_err(
        bed.provisioner()
            .provision(&bed.spec("s1:0", &absent, false))
            .await,
    );
    assert!(
        err.contains(&absent) && err.contains("not present"),
        "the failure names the missing commit: {err}"
    );
    assert_eq!(std::fs::read_dir(&bed.runs_root).unwrap().count(), 0);
    assert_eq!(worktree_count(&bed.repo_dir), 1, "primary only");
}

#[tokio::test]
async fn a_repo_missing_on_this_node_fails_provision_loudly() {
    let bed = bed();
    let mut spec = bed.spec("s1:0", &bed.head, false);
    spec.source = WorkspaceSource::Forge {
        repo: "ghost".into(),
        commit: bed.head.clone(),
        branch: BRANCH.into(),
        branch_born: false,
    };
    let err = provision_err(bed.provisioner().provision(&spec).await);
    assert!(
        err.contains("ghost") && err.contains("not materialized"),
        "{err}"
    );
    assert_eq!(std::fs::read_dir(&bed.runs_root).unwrap().count(), 0);
}

// ---- W6 skill mounts ------------------------------------------------------

#[tokio::test]
async fn skill_mounts_land_beside_the_worktree_where_git_can_never_see_them() {
    // the forge lane used to REFUSE these (running without them); it now
    // materializes them exactly like the duckfs lane — at the `-ro` SIBLING,
    // never inside the worktree, where `git add -A` would commit and PUSH the
    // skill trees onto the agent's work branch.
    let bed = bed();
    let (prov, _actor) = bed.skill_provisioner(false);
    let ws = prov
        .provision(&bed.skill_spec("s1:0", &bed.head))
        .await
        .expect("provision");
    let dir = ws.workdir();
    let ro = PathBuf::from(ws.env().get("DUCKTAPE_RUN_SKILLS").expect("skills root"));

    assert_eq!(ro, PathBuf::from(format!("{}-ro", dir.display())));
    assert_eq!(
        std::fs::read_to_string(ro.join("qa").join(SKILL_FILE)).unwrap(),
        SKILL_BODY
    );
    // the worktree is UNTOUCHED by the mounts: nothing to stage, nothing to push.
    run_git(&dir, &["add", "-A"], &[]).unwrap();
    assert_eq!(
        git_stdout(&dir, &["status", "--porcelain"]),
        "",
        "the skill trees are invisible to git"
    );

    ws.cleanup().await;
    assert!(!dir.exists());
    assert!(!ro.exists(), "the skill root is the run's debris too");
    assert_eq!(worktree_count(&bed.repo_dir), 1, "primary only");
}

#[tokio::test]
async fn a_failed_skill_mount_checkout_leaves_no_debris() {
    // W5: the run never gets a workspace handle on a failed provision, so the
    // provision unwinds EVERYTHING it made — the partial ro tree AND the
    // worktree it had already added (metadata in the shared repo included).
    let bed = bed();
    let (prov, _actor) = bed.skill_provisioner(true);
    let err = provision_err(prov.provision(&bed.skill_spec("s1:0", &bed.head)).await);
    assert!(
        err.contains("chunk not available"),
        "the module's rejection rides out verbatim: {err}"
    );
    assert_eq!(
        std::fs::read_dir(&bed.runs_root).unwrap().count(),
        0,
        "neither the worktree nor the ro root survives"
    );
    assert_eq!(
        worktree_count(&bed.repo_dir),
        1,
        "primary only — metadata pruned"
    );
}

// ---- commit + push --------------------------------------------------------

#[test]
fn commit_message_normalization_strips_supplied_identity_trailers_and_owns_attribution() {
    let message = normalize_commit_message(
        Some(
            "Fix the actual bug\r\n\r\nKeep the useful body:\r\n\tindented code survives\r\n\r\n\
             Co-Authored-By: Human <human@example.com>\r\n\
             Co-Authored-By: Old Bot <old@agents.ducktape.local>\r\n\
             Co-Authored-By: Forged via Ducktape <forged@example.com>\r\n\
             Signed-off-by: Forged DCO <dco@example.com>\r\n\
             Reviewed-by: Fake Reviewer <fake@example.com>\r\n\
             Acked-by: Fake Acker <ack@example.com>\r\n\
             Tested-by: Fake Tester <test@example.com>",
        ),
        "Quack\nAgent <unsafe>",
        "quack/bot@example",
    );
    assert!(
        message.starts_with("Fix the actual bug\n\nKeep the useful body:\n\tindented code survives"),
        "{message:?}"
    );
    assert!(!message.contains("Human") && !message.contains("Old Bot"));
    assert!(!message.contains("Forged") && !message.contains("Fake"));
    for trailer in ["Signed-off-by:", "Reviewed-by:", "Acked-by:", "Tested-by:"] {
        assert!(!message.contains(trailer), "{trailer} leaked: {message:?}");
    }
    assert_eq!(message.matches("Co-Authored-By:").count(), 1);
    assert_eq!(message.matches("via Ducktape").count(), 1);
    let local = attribution_email_local_part("quack/bot@example");
    assert!(message.ends_with(&format!(
        "Co-Authored-By: QuackAgent unsafe via Ducktape <{local}@agents.duck>"
    )));
    assert!(!message.contains('\r'));
}

#[test]
fn attribution_addresses_hash_the_complete_id_after_a_readable_slug() {
    let ids = ["qa/luna", "qa luna", "qa@luna"];
    let locals = ids.map(attribution_email_local_part);
    for local in &locals {
        assert!(local.starts_with("qa-luna-"), "{local}");
        assert!(local.len() <= MAX_AGENT_ID_BYTES, "{local}");
    }
    assert_ne!(locals[0], locals[1]);
    assert_ne!(locals[0], locals[2]);
    assert_ne!(locals[1], locals[2]);
    assert_eq!(locals[0], attribution_email_local_part(ids[0]));
}

#[test]
fn invalid_empty_and_oversized_proposals_fall_back_without_splitting_identity_utf8() {
    let oversized = "x".repeat(MAX_COMMIT_MESSAGE_BYTES + 1);
    for proposal in [
        None,
        Some(" \r\n "),
        Some("dispatch\u{1f}runs\u{1f}private:0"),
        Some(oversized.as_str()),
    ] {
        let long_name = "오리".repeat(100);
        let safe_name = sanitize_display_name(&long_name);
        assert!(safe_name.len() <= MAX_DISPLAY_NAME_BYTES);
        assert!(safe_name.is_char_boundary(safe_name.len()));
        let message = normalize_commit_message(proposal, &long_name, "bot");
        assert!(
            message.starts_with("Apply agent changes\n\n"),
            "{message:?}"
        );
        assert!(!message.contains("dispatch") && message.len() <= MAX_COMMIT_MESSAGE_BYTES);
        assert!(message.is_char_boundary(message.len()));
    }
}

#[tokio::test]
async fn a_push_lands_and_the_receipt_is_the_forge_output_ref() {
    let bed = bed();
    let bare = bed.snapshot_bare();
    let ws = bed
        .provisioner()
        .provision(&bed.spec("s1:0", &bed.head, false))
        .await
        .expect("provision");
    std::fs::write(ws.workdir().join("answer.md"), "the work\n").unwrap();

    let receipt = ws.commit("Apply agent changes").await.expect("commit+push");
    assert!(!receipt.no_changes && receipt.commit_error.is_none());
    assert!(!receipt.rebased, "an uncontended push never claims a rebase");
    assert_eq!(receipt.source_prefix, format!("forge:{REPO}"));
    assert_eq!(receipt.source_snapshot.as_deref(), Some(bed.head.as_str()));
    assert_eq!(receipt.branch.as_deref(), Some(BRANCH));
    assert_eq!(receipt.output_snapshot, None);
    assert_eq!(receipt.commit_height, None);
    let oid = receipt.output_commit.expect("the new commit oid");
    assert_ne!(oid, bed.head);
    // the push CROSSED: the remote's branch is exactly the receipt's oid,
    // and the internal run key never reaches Git text.
    assert_eq!(ref_oid(&bare, BRANCH).as_deref(), Some(oid.as_str()));
    assert_eq!(
        git_stdout(&ws.workdir(), &["log", "-1", "--format=%s"]),
        "Apply agent changes"
    );
    let body = git_stdout(&ws.workdir(), &["log", "-1", "--format=%B"]);
    assert!(!body.contains("s1:0"));
    assert!(body.ends_with(&format!(
        "Co-Authored-By: Quack Agent via Ducktape <{}@agents.duck>",
        attribution_email_local_part(AGENT)
    )));
    ws.cleanup().await;
}

#[tokio::test]
async fn the_commit_carries_agent_author_and_node_committer() {
    let bed = bed();
    bed.snapshot_bare();
    let ws = bed
        .provisioner()
        .provision(&bed.spec("s1:0", &bed.head, false))
        .await
        .expect("provision");
    std::fs::write(ws.workdir().join("answer.md"), "the work\n").unwrap();
    ws.commit("Apply agent changes").await.expect("commit+push");
    // D2: author = the agent (synthetic email), committer = the node identity.
    assert_eq!(
        git_stdout(&ws.workdir(), &["log", "-1", "--format=%an|%ae|%cn|%ce"]),
        format!(
            "{AGENT_DISPLAY_NAME}|{}@agents.duck|{NODE_IDENT}|node@ducktape.local",
            attribution_email_local_part(AGENT)
        )
    );
    ws.cleanup().await;
}

#[tokio::test]
async fn a_clean_tree_yields_no_changes_and_no_push() {
    let bed = bed();
    let bare = bed.snapshot_bare();
    let ws = bed
        .provisioner()
        .provision(&bed.spec("s1:0", &bed.head, false))
        .await
        .expect("provision");
    let runtime = ws.workdir().join(capability_host::RUN_RUNTIME_DIR);
    std::fs::create_dir_all(runtime.join("provider-config")).unwrap();
    std::fs::write(runtime.join("provider-config/auth.json"), "must-not-push").unwrap();

    let receipt = ws.commit("agent run s1:0").await.expect("commit");
    assert!(receipt.no_changes, "a clean tree is a true no_changes");
    assert!(!runtime.exists(), "provider runtime debris is removed before commit");
    assert_eq!(receipt.source_prefix, format!("forge:{REPO}"));
    assert_eq!(receipt.source_snapshot.as_deref(), Some(bed.head.as_str()));
    assert_eq!(receipt.branch, None, "no push landed");
    assert_eq!(receipt.output_commit, None);
    assert_eq!(
        ref_oid(&bare, BRANCH),
        None,
        "NO push happened — the remote never saw the branch"
    );
    ws.cleanup().await;
}

#[tokio::test]
async fn an_agents_own_commits_push_even_with_a_clean_tree() {
    // an agent may `git commit` inside its worktree itself: the tree is clean
    // but the branch moved off the pinned base — that is WORK, not no_changes.
    let bed = bed();
    let bare = bed.snapshot_bare();
    let ws = bed
        .provisioner()
        .provision(&bed.spec("s1:0", &bed.head, false))
        .await
        .expect("provision");
    let dir = ws.workdir();
    std::fs::write(dir.join("own.txt"), "self-committed\n").unwrap();
    run_git(&dir, &["add", "-A"], &[]).unwrap();
    run_git(
        &dir,
        &[
            "commit",
            "-m",
            "Fix repository output",
            "-m",
            "Keep the agent-selected body.\n\nCo-Authored-By: Human <human@example.com>",
        ],
        &[
            ("GIT_AUTHOR_NAME", "self"),
            ("GIT_AUTHOR_EMAIL", "self@x"),
            ("GIT_COMMITTER_NAME", "self"),
            ("GIT_COMMITTER_EMAIL", "self@x"),
        ],
    )
    .unwrap();
    let own = git_stdout(&dir, &["rev-parse", "HEAD"]);
    let own_tree = git_stdout(&dir, &["rev-parse", "HEAD^{tree}"]);

    let receipt = ws.commit("Apply agent changes").await.expect("push");
    assert!(!receipt.no_changes);
    let normalized = receipt.output_commit.expect("normalized run commit");
    assert_ne!(normalized, own, "the intermediate commit must not leak");
    assert_eq!(ref_oid(&bare, BRANCH).as_deref(), Some(normalized.as_str()));
    assert_eq!(git_stdout(&dir, &["rev-parse", "HEAD^"]), bed.head);
    assert_eq!(git_stdout(&dir, &["rev-parse", "HEAD^{tree}"]), own_tree);
    let body = git_stdout(&dir, &["log", "-1", "--format=%B"]);
    assert!(
        body.starts_with("Fix repository output\n\nKeep the agent-selected body."),
        "{body}"
    );
    assert!(!body.contains("Human <human@example.com>"), "{body}");
    assert_eq!(body.matches("Co-Authored-By:").count(), 1, "{body}");
    assert_eq!(body.matches("via Ducktape").count(), 1, "{body}");
    ws.cleanup().await;
}

#[tokio::test]
async fn an_invalid_agent_message_falls_back_without_losing_the_agents_tree() {
    let bed = bed();
    let bare = bed.snapshot_bare();
    let ws = bed
        .provisioner()
        .provision(&bed.spec("s1:0", &bed.head, false))
        .await
        .expect("provision");
    let dir = ws.workdir();
    std::fs::write(dir.join("safe.txt"), "must survive\n").unwrap();
    run_git(&dir, &["add", "-A"], &[]).unwrap();
    run_git(
        &dir,
        &["commit", "-m", "dispatch\u{1f}runs\u{1f}private-hash:0"],
        &[
            ("GIT_AUTHOR_NAME", "attacker"),
            ("GIT_AUTHOR_EMAIL", "attacker@example.com"),
            ("GIT_COMMITTER_NAME", "attacker"),
            ("GIT_COMMITTER_EMAIL", "attacker@example.com"),
        ],
    )
    .unwrap();
    let final_tree = git_stdout(&dir, &["rev-parse", "HEAD^{tree}"]);

    let receipt = ws.commit("ignored internal value").await.expect("push");
    let oid = receipt.output_commit.expect("normalized output");
    assert_eq!(ref_oid(&bare, BRANCH).as_deref(), Some(oid.as_str()));
    assert_eq!(git_stdout(&dir, &["rev-parse", "HEAD^{tree}"]), final_tree);
    assert_eq!(git_stdout(&dir, &["show", "HEAD:safe.txt"]), "must survive");
    let body = git_stdout(&dir, &["log", "-1", "--format=%B"]);
    assert!(body.starts_with("Apply agent changes\n\n"), "{body}");
    assert!(!body.contains("dispatch") && !body.contains('\u{1f}'), "{body:?}");
    ws.cleanup().await;
}

#[tokio::test]
async fn a_concurrent_advance_is_rebased_under_the_runs_work_and_pushed() {
    // "we replicate all of git actions": a concurrent advance is an ORDERING
    // problem — fetch, rebase, push; both commits land as linear history.
    let bed = bed();
    let bare = bed.snapshot_bare();
    // someone advanced the branch on the remote AFTER this run's base was
    // pinned: mint C2 (a child of head) straight in the bare repo.
    let bare_repo = git2::Repository::open(&bare).unwrap();
    let c2 = odb_commit(&bare_repo, Some(&bed.head), "rival.txt", "concurrent work\n");
    set_ref(&bare_repo, BRANCH, &c2);

    let spec = bed.spec("s1:0", &bed.head, true);
    let ws = bed.provisioner().provision(&spec).await.expect("provision");
    let dir = ws.workdir();
    std::fs::write(dir.join("mine.txt"), "forked work\n").unwrap();

    let receipt = ws.commit("agent run s1:0").await.expect("rebase-retry push");
    assert!(receipt.rebased, "the receipt says the base moved under the work");
    assert!(!receipt.no_changes && receipt.commit_error.is_none());
    assert_eq!(receipt.branch.as_deref(), Some(BRANCH));
    let oid = receipt.output_commit.expect("post-rebase oid");
    // linear history: the interloper is now the run commit's PARENT …
    assert_eq!(ref_oid(&bare, BRANCH).as_deref(), Some(oid.as_str()));
    assert_eq!(git_stdout(&dir, &["rev-parse", "HEAD^"]), c2);
    // … and the rebase preserved the agent author + node committer (D2).
    assert_eq!(
        git_stdout(&dir, &["log", "-1", "--format=%an|%ae|%cn|%ce"]),
        format!(
            "{AGENT_DISPLAY_NAME}|{}@agents.duck|{NODE_IDENT}|node@ducktape.local",
            attribution_email_local_part(AGENT)
        )
    );
    ws.cleanup().await;
}

#[tokio::test]
async fn an_identical_concurrent_patch_keeps_a_normalized_attribution_commit() {
    let bed = bed();
    let bare = bed.snapshot_bare();
    let bare_repo = git2::Repository::open(&bare).unwrap();
    let c2 = odb_commit(&bare_repo, Some(&bed.head), "same.txt", "identical work\n");
    set_ref(&bare_repo, BRANCH, &c2);

    let ws = bed
        .provisioner()
        .provision(&bed.spec("s1:0", &bed.head, true))
        .await
        .expect("provision");
    let dir = ws.workdir();
    std::fs::write(dir.join("same.txt"), "identical work\n").unwrap();

    let receipt = ws
        .commit("agent run s1:0")
        .await
        .expect("rebase-retry push");
    assert!(receipt.rebased);
    let oid = receipt.output_commit.expect("normalized output commit");
    assert_ne!(
        oid, c2,
        "the unrelated upstream commit is not the receipt output"
    );
    assert_eq!(ref_oid(&bare, BRANCH).as_deref(), Some(oid.as_str()));
    assert_eq!(git_stdout(&dir, &["rev-parse", "HEAD^"]), c2);
    assert_eq!(
        git_stdout(&dir, &["log", "-1", "--format=%an|%ae|%s"]),
        format!(
            "{AGENT_DISPLAY_NAME}|{}@agents.duck|Apply agent changes",
            attribution_email_local_part(AGENT)
        )
    );
    let body = git_stdout(&dir, &["log", "-1", "--format=%B"]);
    assert_eq!(body.matches("Co-Authored-By:").count(), 1, "{body}");
    assert!(body.contains(" via Ducktape <"), "{body}");
    ws.cleanup().await;
}

#[tokio::test]
async fn a_rebase_conflict_aborts_cleanly_and_degrades() {
    let bed = bed();
    let bare = bed.snapshot_bare();
    // the interloper and the run edit the SAME lines of readme.md.
    let bare_repo = git2::Repository::open(&bare).unwrap();
    let c2 = odb_commit(&bare_repo, Some(&bed.head), "readme.md", "rival edit\n");
    set_ref(&bare_repo, BRANCH, &c2);

    let spec = bed.spec("s1:0", &bed.head, true);
    let ws = bed.provisioner().provision(&spec).await.expect("provision");
    let dir = ws.workdir();
    std::fs::write(dir.join("readme.md"), "my conflicting edit\n").unwrap();

    let err = ws.commit("agent run s1:0").await.unwrap_err();
    assert!(
        err.contains("rebase conflict") && err.contains(BRANCH),
        "ONLY a genuine conflict degrades, naming the branch: {err}"
    );
    // aborted cleanly: no mid-rebase state left behind …
    let rebase_dir = git_stdout(&dir, &["rev-parse", "--git-path", "rebase-merge"]);
    let rebase_path = Path::new(&rebase_dir);
    let rebase_abs = if rebase_path.is_absolute() {
        rebase_path.to_path_buf()
    } else {
        dir.join(rebase_path)
    };
    assert!(!rebase_abs.exists(), "no rebase-merge debris at {rebase_dir}");
    // … the interloper's tip stays branch head …
    assert_eq!(ref_oid(&bare, BRANCH).as_deref(), Some(c2.as_str()));
    // … and the pool maps exactly this Err to commit_failed + Degraded.
    let receipt = WorkspaceReceipt::commit_failed(&spec, err);
    assert_eq!(receipt.branch.as_deref(), Some(BRANCH));
    assert!(receipt.commit_error.is_some() && !receipt.no_changes);
    assert_eq!(receipt.output_commit, None);
    ws.cleanup().await;
}

#[tokio::test]
async fn a_remote_that_always_rejects_exhausts_the_bounded_retries() {
    let bed = bed();
    // the branch must be born on the remote so mid-loop fetch/rebase succeed
    // and the BOUND — not a fetch error — is what ends the loop.
    let repo = git2::Repository::open(&bed.repo_dir).unwrap();
    set_ref(&repo, BRANCH, &bed.head);
    let bare = bed.snapshot_bare();
    // a pre-receive hook that logs every push attempt and declines it.
    let hook = bare.join("hooks").join("pre-receive");
    std::fs::create_dir_all(hook.parent().unwrap()).unwrap();
    std::fs::write(&hook, "#!/bin/sh\necho x >> hook.log\nexit 1\n").unwrap();
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let ws = bed
        .provisioner()
        .provision(&bed.spec("s1:0", &bed.head, true))
        .await
        .expect("provision");
    std::fs::write(ws.workdir().join("mine.txt"), "work\n").unwrap();

    let err = ws.commit("agent run s1:0").await.unwrap_err();
    assert!(
        err.contains("after 3 attempts") && err.contains("pre-receive"),
        "the bound ends the loop carrying the real reject: {err}"
    );
    let pushes = std::fs::read_to_string(bare.join("hook.log")).unwrap();
    assert_eq!(pushes.lines().count(), 3, "exactly PUSH_ATTEMPTS pushes, no spin");
    ws.cleanup().await;
}

// ---- cleanup (W5) ---------------------------------------------------------

#[tokio::test]
async fn cleanup_removes_the_worktree_and_its_metadata_even_uncommitted() {
    // the unwind-path shape: the pool calls cleanup() unconditionally —
    // panicking providers included — so a dirty, never-committed tree must
    // go quietly, metadata and all.
    let bed = bed();
    let ws = bed
        .provisioner()
        .provision(&bed.spec("s1:0", &bed.head, false))
        .await
        .expect("provision");
    let dir = ws.workdir();
    std::fs::write(dir.join("scratch.txt"), "uncommitted\n").unwrap();

    ws.cleanup().await;
    assert!(!dir.exists(), "the run dir is gone");
    assert_eq!(
        worktree_count(&bed.repo_dir),
        1,
        "primary only — the worktree metadata was pruned"
    );
    // idempotent: a second cleanup (or one after external deletion) is quiet.
    ws.cleanup().await;
    assert!(!dir.exists());
}

#[tokio::test]
async fn cleanup_after_a_successful_push_leaves_only_the_branch_ref() {
    let bed = bed();
    bed.snapshot_bare();
    let ws = bed
        .provisioner()
        .provision(&bed.spec("s1:0", &bed.head, false))
        .await
        .expect("provision");
    std::fs::write(ws.workdir().join("answer.md"), "done\n").unwrap();
    let receipt = ws.commit("agent run s1:0").await.expect("push");
    let dir = ws.workdir();
    ws.cleanup().await;
    assert!(!dir.exists());
    assert_eq!(worktree_count(&bed.repo_dir), 1, "primary only");
    let _ = receipt;
    // detached lane: the work branch lives only on the remote — the shared
    // repo never grew a local ref for it.
    assert_eq!(ref_oid(&bed.repo_dir, BRANCH), None);
}
