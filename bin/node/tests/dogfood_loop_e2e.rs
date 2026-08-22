//! the agent dogfooding loop (M1), end to end on REAL `ducktape`
//! validators: issue mention → forge-workspace run → PR, then the PR channel as
//! a SESSION.
//!
//!   1. a repo is born by its first push; an issue opens (item #1, hidden
//!      channel `forge:<repo>:1`); the agent is registered with forge caps
//!      and the channel is watched.
//!   2. mentioning the agent runs the provider INSIDE a real git clone of the
//!      repo, detached at the pinned dev tip; the host commits + pushes branch
//!      `agent/item-1` through consensus and the PR sink opens a PR whose
//!      title is the bound Forge issue title.
//!   3. re-mentioning in the PR's OWN channel forks the branch TIP: a second
//!      commit lands on the SAME branch (parent = the first), and the
//!      duplicate guard opens NO second PR.
//!
//! ## a run is sandboxed, and that changed what this test can see
//!
//! The provider executes INSIDE a container now, so this suite needs a
//! `[sandbox]` table (without one every compute daemon exits at boot — the
//! reason this file spent weeks passing nothing) and its script cannot touch a
//! host path: the fixture directory does not exist in the run's mount
//! namespace. The evidence moved onto the one surface that DOES cross the
//! boundary — the run workspace itself, which is a bind mount the host then
//! commits and pushes. Each run writes [`HEAD_FILE`], and the test reads it back
//! out of committed git history, which is a stronger claim than the old
//! host-side trace log: it is signed into the branch the run produced.
//!
//! `.git/HEAD` is read with `cat`, not `git rev-parse`: a detached HEAD holds
//! the raw oid, so the whole proof (WHICH commit, and that it is DETACHED) is
//! one file read, and the image stays a 4 MB busybox instead of something
//! carrying a git client.
//!
//! ## what this file no longer covers, and why
//!
//! It used to end with an ordering proof: the provider script itself advanced
//! the work branch through the node's loopback forge remote, so the host's push
//! rejected and the provisioner had to rebase. A run's container has no route
//! to its own node — the egress firewall allows this run's broker port and
//! nothing else — so a provider CANNOT race a push any more, and a scenario
//! that cannot happen is not a regression this suite can hold. The property
//! itself is covered where it lives, against the provisioner:
//! `bin/noded/src/agent_provision/forge_tests.rs`'s
//! `a_concurrent_advance_is_rebased_under_the_runs_work_and_pushed` (plus the
//! merge-preserving and author-preservation variants beside it).
//!
//! run alone (cluster e2es flake under parallel load):
//!   cargo test -p node-bin --test dogfood_loop_e2e -- --nocapture

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

use agent::{ACTION_CHAT_POST, AgentMsg, ResourceCaps};
use capability::{CapabilityQuery, CapabilityReply};
use chat::{AuthorRef, Block, ChatMsg, ChatQuery, ChatReply, Mark, Span};
use common::{Cluster, poll_until, sandbox_toml, serial, skip_unless_sandboxed};
use runs::{RunOutcome, RunRecord, RunsMsg, RunsQuery, RunsReply, TurnPolicy};

const CONVERGE: Duration = Duration::from_secs(180);
const FINALIZE: Duration = Duration::from_secs(60);
const ROUND_TRIP: Duration = Duration::from_secs(120);

/// the file each run writes into its workspace, carrying `pwd` and the raw
/// `.git/HEAD` of the clone it was handed. The host stages everything under the
/// workspace, so it rides the run's own commit into the branch — readable from
/// any node, forever, instead of from a host path the container cannot see.
const HEAD_FILE: &str = ".dogfood-run";
/// the neutral guest cwd every sandboxed run gets (`podman_api::GUEST_ROOT`),
/// which is the point: the operator's real layout never reaches the workload.
const GUEST_WORKDIR: &str = "/ducktape/workspace";

const AGENT_ID: &str = "quacker-dogfood";
const REPO: &str = "dogfood";
const WORK_BRANCH: &str = "agent/item-1";
/// the worker script's single-line reply; it belongs in the published body.
const REPLY_TITLE: &str = "Dogfood loop proof";
const ISSUE_TITLE: &str = "prove the dogfood loop";

/// one script-backed provider standing in for a coding agent.
///
/// It runs INSIDE the run's container, so `sh`, `cat` and `printf` are the whole
/// of its dependencies and its only writable surface is the workspace it was
/// handed. It records `pwd|HEAD` into [`HEAD_FILE`] — which the host commits —
/// and answers on stdout.
struct DogfoodProvider {
    tag: String,
    spec_dir: PathBuf,
    env_var: String,
    script: PathBuf,
}

impl DogfoodProvider {
    fn stage(root: &Path) -> Self {
        let dir = root.join("dogfood-provider");
        let spec_dir = dir.join("specs");
        std::fs::create_dir_all(&spec_dir).expect("provider spec dir");
        let tag = "quack-dogfood";
        let env_var = "DUCKTAPE_TEST_QUACK_DOGFOOD_BIN".to_string();
        std::fs::write(
            spec_dir.join(format!("{tag}.toml")),
            format!(
                "spec = 1\n\
                 [capability]\n\
                 tag = \"{tag}\"\n\
                 description = \"dogfood e2e script executor\"\n\
                 [detect]\n\
                 bin = \"{tag}-nonexistent-cli\"\n\
                 env = \"{env_var}\"\n\
                 [invoke]\n\
                 args = []\n\
                 prompt = \"stdin\"\n\
                 timeout_secs = 60\n\
                 [output]\n\
                 format = \"text\"\n"
            ),
        )
        .expect("write provider spec");
        let script = dir.join("provider.sh");
        // Everything the script needs of its image: `sh`, `cat`, `printf`.
        //
        // `.git/HEAD` is read RAW rather than through `git rev-parse`. The run
        // workspace is a self-contained clone (`.git` lives inside the bind
        // mount), detached at the pin — so that file holds the bare oid, and
        // reading it proves BOTH which commit the run forked and that the
        // checkout is detached: a branch checkout would hold `ref: refs/…`
        // instead, and the assertions below would name it.
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\n\
                 set -e\n\
                 cat > /dev/null\n\
                 printf '%s|%s\\n' \"$(pwd)\" \"$(cat .git/HEAD)\" > {HEAD_FILE}\n\
                 printf '%s\\n' '{REPLY_TITLE}'\n"
            ),
        )
        .expect("write provider script");
        use std::os::unix::fs::PermissionsExt as _;
        let mut perms = std::fs::metadata(&script).expect("script metadata").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).expect("chmod provider script");
        Self {
            tag: tag.into(),
            spec_dir,
            env_var,
            script,
        }
    }

    fn env(&self) -> Vec<(String, String)> {
        vec![
            (
                "DUCKTAPE_CAPABILITY_DIR".into(),
                self.spec_dir.display().to_string(),
            ),
            (self.env_var.clone(), self.script.display().to_string()),
        ]
    }
}

/// the `pwd|HEAD` [`HEAD_FILE`] the run at `commit` committed, read out of a
/// clone of the branch — committed evidence, from a node that executed nothing.
fn run_evidence(checkout: &Path, commit: &str) -> (String, String) {
    let line = git_stdout(checkout, &["show", &format!("{commit}:{HEAD_FILE}")]);
    let (cwd, head) = line
        .split_once('|')
        .unwrap_or_else(|| panic!("{HEAD_FILE} at {commit} is `pwd|HEAD`, got {line:?}"));
    (cwd.to_string(), head.to_string())
}

/// hermetic env for a node that must provide NOTHING (see dispatch_e2e).
fn hermetic_env(root: &Path, name: &str) -> Vec<(String, String)> {
    let empty = root.join(name).join("specs");
    std::fs::create_dir_all(&empty).expect("empty spec dir");
    let missing = root.join(name).join("missing-executor");
    vec![
        ("DUCKTAPE_CAPABILITY_DIR".into(), empty.display().to_string()),
        ("DUCKTAPE_CLAUDE_BIN".into(), missing.display().to_string()),
        ("DUCKTAPE_CODEX_BIN".into(), missing.display().to_string()),
    ]
}

fn hide_builtins(root: &Path, name: &str) -> Vec<(String, String)> {
    let missing = root.join(name).join("missing-executor");
    vec![
        ("DUCKTAPE_CLAUDE_BIN".into(), missing.display().to_string()),
        ("DUCKTAPE_CODEX_BIN".into(), missing.display().to_string()),
    ]
}

fn boot(cluster: &mut Cluster) {
    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", Duration::from_secs(60));
    cluster.spawn(1);
    cluster.spawn(2);
    let genesis: Vec<String> = (0..3)
        .map(|i| cluster.wait_marker(i, "genesis root_hash=", Duration::from_secs(60)))
        .collect();
    assert_eq!(genesis[0], genesis[1], "genesis fork between nodes 0 and 1");
    assert_eq!(genesis[0], genesis[2], "genesis fork between nodes 0 and 2");
    for i in 0..3 {
        cluster.wait_marker(i, "converged root_hash=", CONVERGE);
        // the compute plane is a separate process with its own failure domain:
        // gate on ITS lifecycle marker, or a daemon that died at boot leaves a
        // cluster that looks healthy until an unrelated predicate times out
        // three minutes later.
        cluster.wait_compute_marker(i, "compute daemon serving", CONVERGE);
    }
}

fn providers(cluster: &Cluster, idx: usize, tag: &str) -> Option<Vec<Vec<u8>>> {
    let reply = cluster.query(
        idx,
        "capability",
        &capability::encode_query(&CapabilityQuery::Providers {
            capability: tag.into(),
        }),
    )?;
    match capability::decode_reply(&reply) {
        Ok(CapabilityReply::Providers(p)) => Some(p),
        _ => None,
    }
}

/// post a mention of the agent into `channel` under a caller-chosen id.
fn post_mention(cluster: &Cluster, idx: usize, channel: &str, message_id: &str) {
    cluster.submit(
        idx,
        "chat",
        &chat::encode_msg(&ChatMsg::PostMessage {
            channel_id: channel.into(),
            message_id: message_id.into(),
            blocks: vec![Block::Paragraph(vec![
                Span::plain("hey "),
                Span {
                    text: format!("@{AGENT_ID}"),
                    marks: vec![Mark::Mention(AuthorRef::Agent {
                        module: "runs".into(),
                        agent_id: AGENT_ID.into(),
                    })],
                },
                Span::plain(" do the dogfood thing"),
            ])],
            thread: None,
            as_agent: None,
        }),
    );
}

/// find `message_id` in `channel` and return `(seq, concatenated text)`.
fn find_message(
    cluster: &Cluster,
    idx: usize,
    channel: &str,
    message_id: &str,
) -> Option<(u64, String)> {
    let reply = cluster.query(
        idx,
        "chat",
        &chat::encode_query(&ChatQuery::MessagesRange {
            channel_id: channel.into(),
            from_seq: 1,
            limit: 64,
        }),
    )?;
    let ChatReply::Messages(views) = chat::decode_reply(&reply).ok()? else {
        return None;
    };
    views.into_iter().find_map(|v| {
        (v.head.message_id == message_id).then(|| {
            let text = v
                .head
                .blocks
                .iter()
                .map(|b| match b {
                    Block::Paragraph(spans) | Block::Quote(spans) => {
                        spans.iter().map(|s| s.text.as_str()).collect::<String>()
                    }
                    Block::Code { text, .. } => text.clone(),
                    Block::Divider => String::new(),
                })
                .collect::<String>();
            (v.seq, text)
        })
    })
}

/// the anchor seq of a just-posted mention — NEVER hardcoded: forge posts
/// system lines into item channels on state changes, so seqs are discovered.
fn seq_of(cluster: &Cluster, idx: usize, channel: &str, message_id: &str) -> u64 {
    poll_until(&format!("mention {message_id} to finalize"), FINALIZE, || {
        find_message(cluster, idx, channel, message_id).map(|(seq, _)| seq)
    })
}

fn wait_for_reply(cluster: &Cluster, idx: usize, channel: &str, run_id: &str) -> String {
    poll_until("the agent reply to post", ROUND_TRIP, || {
        find_message(cluster, idx, channel, &format!("agent/{run_id}")).map(|(_, text)| text)
    })
}

/// the committed tip of `branch` in the dogfood repo, from ListRefs.
fn branch_tip(cluster: &Cluster, idx: usize, branch: &str) -> Option<String> {
    let reply = cluster.query(
        idx,
        "forge",
        &forge::encode_query(&forge::ForgeQuery::ListRefs { repo: REPO.into() }),
    )?;
    match forge::decode_reply(&reply).ok()? {
        forge::ForgeReply::Refs(refs) => {
            refs.into_iter().find(|r| r.name == branch).map(|r| r.head)
        }
        _ => None,
    }
}

/// the repo's committed tracker items, ascending by number.
fn tracker_items(cluster: &Cluster, idx: usize) -> Vec<forge::ItemSummary> {
    let Some(reply) = cluster.query(
        idx,
        "forge",
        &forge::encode_query(&forge::ForgeQuery::ListItems { repo: REPO.into() }),
    ) else {
        return Vec::new();
    };
    match forge::decode_reply(&reply) {
        Ok(forge::ForgeReply::Items(items)) => items,
        _ => Vec::new(),
    }
}

fn tracker_item(cluster: &Cluster, idx: usize, number: u64) -> Option<forge::ItemDetail> {
    let reply = cluster.query(
        idx,
        "forge",
        &forge::encode_query(&forge::ForgeQuery::GetItem {
            repo: REPO.into(),
            number,
        }),
    )?;
    match forge::decode_reply(&reply).ok()? {
        forge::ForgeReply::Item(item) => item.map(|boxed| *boxed),
        _ => None,
    }
}

/// this run's entry in the delivered-runs ring (Task 5's receipt lane).
fn run_record(cluster: &Cluster, idx: usize, run_id: &str) -> Option<RunRecord> {
    let reply = cluster.query(idx, "runs", &runs::encode_query(&RunsQuery::RecentRuns))?;
    match runs::decode_reply(&reply).ok()? {
        RunsReply::RecentRuns(records) => records.into_iter().find(|r| r.run_id == run_id),
        _ => None,
    }
}

fn open_pr_count(cluster: &Cluster, idx: usize) -> usize {
    tracker_items(cluster, idx)
        .iter()
        .filter(|i| i.kind == forge::ItemKind::Pr && i.state == forge::ItemState::Open)
        .count()
}

// ---- hermetic git helpers (file-local, like resident_submit_e2e) --------------

fn git_command(dir: &Path, args: &[&str]) -> Command {
    let mut command = Command::new("git");
    command
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .args([
            "-c",
            "init.defaultBranch=main",
            "-c",
            "user.name=Ducktape Test",
            "-c",
            "user.email=test@ducktape.local",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args);
    command
}

fn git_output(dir: &Path, args: &[&str]) -> Output {
    git_command(dir, args).output().expect("spawn git")
}

fn git_ok(dir: &Path, args: &[&str]) {
    let output = git_output(dir, args);
    assert!(
        output.status.success(),
        "git {args:?} failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

fn git_stdout(dir: &Path, args: &[&str]) -> String {
    let output = git_output(dir, args);
    assert!(
        output.status.success(),
        "git {args:?} failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    String::from_utf8(output.stdout)
        .expect("git stdout is utf-8")
        .trim()
        .to_string()
}

#[test]
fn issue_mention_runs_a_workspace_opens_a_pr_and_the_pr_channel_is_a_session() {
    if skip_unless_sandboxed(
        "issue_mention_runs_a_workspace_opens_a_pr_and_the_pr_channel_is_a_session",
    )
    .is_some()
    {
        return;
    }
    // the fixture seeds and inspects the repo with the HOST git; the run inside
    // the container needs none.
    if nettest::skip_without(
        "issue_mention_runs_a_workspace_opens_a_pr_and_the_pr_channel_is_a_session",
        nettest::missing_tool("git"),
    )
    .is_some()
    {
        return;
    }
    let _serial = serial();
    let fixtures = tempfile::TempDir::new().expect("provider fixtures dir");
    let provider = DogfoodProvider::stage(fixtures.path());
    let runs_root = fixtures.path().join("agent-runs");
    let runs_root_env = (
        "DUCKTAPE_AGENT_RUNS_ROOT".to_string(),
        runs_root.display().to_string(),
    );

    let mut cluster = Cluster::new(&[0, 1, 2], &[0, 1, 2]);
    // serving is opt-in now (default OFF): this test needs node 1 in the
    // rendezvous pool, so every node opts in.
    // serving is opt-in: the compute grant is what puts a node in the pool.
    cluster.compute_grant = Some(vec![provider.tag.clone()]);
    // HOW a run is isolated (the table) is independent of WHETHER this node runs
    // any (the grant); the compute daemon needs both and refuses to boot without
    // the table. Appended LAST — nothing may follow a toml table header.
    cluster.extra_toml.extend(sandbox_toml());
    cluster.env[0] = [hermetic_env(fixtures.path(), "node0"), vec![runs_root_env.clone()]].concat();
    cluster.env[1] = [
        provider.env(),
        hide_builtins(fixtures.path(), "node1"),
        vec![runs_root_env.clone()],
    ]
    .concat();
    cluster.env[2] = [hermetic_env(fixtures.path(), "node2"), vec![runs_root_env]].concat();
    boot(&mut cluster);

    // node 1 is the tag's ONLY provider, so every lease lands there.
    poll_until("the provider to announce", FINALIZE, || {
        (providers(&cluster, 0, &provider.tag)? == vec![Cluster::identity(1)]).then_some(())
    });

    // ---- seed: the repo is BORN by its first push (no create-repo op).
    let seed = tempfile::tempdir().expect("git seed dir");
    git_ok(seed.path(), &["init"]);
    std::fs::write(seed.path().join("README.md"), "the dogfood repo\n").expect("write readme");
    git_ok(seed.path(), &["add", "README.md"]);
    git_ok(seed.path(), &["commit", "-m", "seed"]);
    let dev_tip = git_stdout(seed.path(), &["rev-parse", "HEAD"]);
    let seed_url = format!("http://127.0.0.1:{}/forge/{REPO}", cluster.http_ports[0]);
    git_ok(seed.path(), &["remote", "add", "origin", &seed_url]);
    git_ok(seed.path(), &["push", "origin", "HEAD:dev"]);
    // committed on the EXECUTING node before any run pins it.
    poll_until("the seed push to finalize on node 1", CONVERGE, || {
        (branch_tip(&cluster, 1, "dev")? == dev_tip).then_some(())
    });

    // ---- the issue: item #1 and its hidden channel `forge:dogfood:1`.
    cluster.submit(
        0,
        "forge",
        &forge::encode_msg(&forge::ForgeMsg::OpenIssue {
            repo: REPO.into(),
            title: ISSUE_TITLE.into(),
            body: "mention the duck, get a PR".into(),
        }),
    );
    let issue = poll_until("the issue to finalize", FINALIZE, || {
        tracker_item(&cluster, 0, 1)
    });
    assert_eq!(issue.summary.kind, forge::ItemKind::Issue);
    let issue_channel = issue.channel_id.clone();
    assert_eq!(issue_channel, format!("forge:{REPO}:1"));

    // ---- the agent (no prompt pin — a persona is a curated `Always` skill now,
    //      and this leg needs none; forge caps naming the repo LITERALLY) and the
    //      watch that arms the trigger (atomic tagging subscribe, P2).
    cluster.submit(
        0,
        "agent",
        &agent::encode_msg(&AgentMsg::RegisterAgent {
            agent_id: AGENT_ID.into(),
            display_name: AGENT_ID.into(),
            capability: provider.tag.clone(),
            allowed_actions: vec![ACTION_CHAT_POST.into()],
            recipe_hash: None,
            caps: Some(ResourceCaps {
                forge_read: vec![REPO.into()],
                forge_push: vec![REPO.into()],
                ..Default::default()
            }),
            skills: None,
        }),
    );
    let watch = |channel: &str| {
        cluster.submit(
            0,
            "runs",
            &runs::encode_msg(&RunsMsg::WatchChannel {
                channel_id: channel.into(),
                policy: TurnPolicy::Mention,
            }),
        );
        let channel = channel.to_string();
        poll_until("the channel watch to commit", FINALIZE, || {
            let reply = cluster.query(0, "runs", &runs::encode_query(&RunsQuery::Watches))?;
            match runs::decode_reply(&reply) {
                Ok(RunsReply::Watches(w)) => {
                    w.iter().any(|v| v.channel_id == channel).then_some(())
                }
                _ => None,
            }
        });
    };
    watch(&issue_channel);

    // ---- run 1: issue mention → worktree at the pinned dev tip → branch
    //      `agent/item-1` born → a PR titled by the bound Forge issue.
    post_mention(&cluster, 0, &issue_channel, "m1");
    let run_1 = runs::run_id_for(&issue_channel, seq_of(&cluster, 0, &issue_channel, "m1"), AGENT_ID);
    assert_eq!(
        wait_for_reply(&cluster, 0, &issue_channel, &run_1),
        REPLY_TITLE,
        "run 1 replies in the issue channel"
    );

    let run1_oid = {
        let deadline = std::time::Instant::now() + FINALIZE;
        loop {
            if let Some(oid) = branch_tip(&cluster, 0, WORK_BRANCH) {
                break oid;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "branch {WORK_BRANCH} never born;\n{}",
                cluster.all_log_tails(80)
            );
            std::thread::sleep(Duration::from_millis(300));
        }
    };
    assert_ne!(run1_oid, dev_tip, "the run pushed a NEW commit");


    // the PR: opened by the sink, titled from verified bound issue metadata.
    let pr_number = poll_until("the PR to open", FINALIZE, || {
        tracker_items(&cluster, 0)
            .iter()
            .find(|i| i.kind == forge::ItemKind::Pr)
            .map(|i| i.number)
    });
    let pr = tracker_item(&cluster, 0, pr_number).expect("the PR item");
    assert_eq!(pr.summary.state, forge::ItemState::Open);
    assert_eq!(
        pr.summary.title, ISSUE_TITLE,
        "PR title = the bound issue title"
    );
    assert_eq!(pr.source_branch.as_deref(), Some(WORK_BRANCH));
    assert_eq!(pr.target_branch.as_deref(), Some("dev"));
    let pr_channel = pr.channel_id.clone();
    assert_eq!(pr_channel, format!("forge:{REPO}:{pr_number}"));

    // the delivered-runs ring carries the receipt: branch@commit + PR number.
    let record = poll_until("run 1 in the delivered-runs ring", FINALIZE, || {
        run_record(&cluster, 0, &run_1)
    });
    assert_eq!(record.outcome, RunOutcome::Delivered);
    assert!(!record.degraded, "run 1 is clean: {record:?}");
    assert_eq!(record.output_ref.as_deref(), Some(format!("{WORK_BRANCH}@{run1_oid}").as_str()));
    assert_eq!(record.pr_number, Some(pr_number));

    // ---- run 2: the PR channel IS the session — re-mention forks the branch
    //      TIP, lands a second commit on the SAME branch, opens NO second PR.
    watch(&pr_channel);
    post_mention(&cluster, 0, &pr_channel, "m2");
    let run_2 = runs::run_id_for(&pr_channel, seq_of(&cluster, 0, &pr_channel, "m2"), AGENT_ID);
    assert_eq!(
        wait_for_reply(&cluster, 0, &pr_channel, &run_2),
        REPLY_TITLE,
        "run 2 replies in the PR channel"
    );

    let run2_oid = poll_until("the branch tip to advance", FINALIZE, || {
        branch_tip(&cluster, 0, WORK_BRANCH).filter(|tip| *tip != run1_oid)
    });
    // parent chain, proven from a node that executed nothing: run2 → run1 →
    // seed — the objects fanned out with the refs.
    let checkout = tempfile::tempdir().expect("git checkout parent");
    let clone_url = format!("http://127.0.0.1:{}/forge/{REPO}", cluster.http_ports[2]);
    let dest = checkout.path().join("after-run2");
    git_ok(
        checkout.path(),
        &["clone", "--quiet", "--branch", WORK_BRANCH, &clone_url, dest.to_str().unwrap()],
    );
    assert_eq!(git_stdout(&dest, &["rev-parse", "HEAD"]), run2_oid);
    assert_eq!(git_stdout(&dest, &["rev-parse", "HEAD^"]), run1_oid, "run 2's parent is run 1's commit");
    assert_eq!(git_stdout(&dest, &["rev-parse", "HEAD~2"]), dev_tip);

    // What each run SAW, read out of the commit it produced: the sandboxed
    // neutral cwd, and a detached `.git/HEAD` naming the commit it forked. Run 1
    // forks the pinned dev tip; run 2 forks the branch TIP, which is what makes
    // the PR channel a session rather than a second independent run.
    for (commit, pinned_at, which) in
        [(&run1_oid, &dev_tip, "run 1"), (&run2_oid, &run1_oid, "run 2")]
    {
        let (cwd, head) = run_evidence(&dest, commit);
        assert_eq!(cwd, GUEST_WORKDIR, "{which} ran at the neutral sandbox cwd");
        assert_eq!(
            &head, pinned_at,
            "{which}'s .git/HEAD is the RAW oid it forked — a `ref: refs/…` here \
             would mean the checkout was not detached"
        );
    }

    assert_eq!(open_pr_count(&cluster, 0), 1, "the duplicate guard opened NO second PR");
    let record = poll_until("run 2 in the delivered-runs ring", FINALIZE, || {
        run_record(&cluster, 0, &run_2)
    });
    assert_eq!(record.outcome, RunOutcome::Delivered);
    assert!(!record.degraded, "run 2 is clean: {record:?}");
    assert_eq!(record.output_ref.as_deref(), Some(format!("{WORK_BRANCH}@{run2_oid}").as_str()));
    assert_eq!(record.pr_number, Some(pr_number), "the ring names the UPDATED PR");
}
