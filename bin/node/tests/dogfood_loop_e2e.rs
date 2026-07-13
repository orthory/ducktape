//! the agent dogfooding loop (M1), end to end on REAL `ducktape-node`
//! validators: issue mention → forge-worktree run → PR, then the PR channel
//! as a SESSION, then a deterministic concurrent-advance rebase.
//!
//!   1. a repo is born by its first push; an issue opens (item #1, hidden
//!      channel `forge:<repo>:1`); the agent is registered with forge caps
//!      and the channel is watched.
//!   2. mentioning the agent runs the provider INSIDE a real git worktree of
//!      the repo at the pinned main tip; the host commits + pushes branch
//!      `agent/item-1` through consensus and the PR sink opens a PR whose
//!      title is the bound Forge issue title.
//!   3. re-mentioning in the PR's OWN channel forks the branch TIP: a second
//!      commit lands on the SAME branch (parent = the first), and the
//!      duplicate guard opens NO second PR.
//!   4. the ordering proof: the provider itself advances the work branch
//!      through the loopback remote (clone → empty commit → push) and
//!      CONDITION-POLLS `git ls-remote` until the advertisement (COMMITTED
//!      refs) carries its tip — the happens-before that makes the race
//!      deterministic (a push alone does not order the two ops). the run's
//!      worktree is DETACHED at the pin, so the host commits on the stale
//!      base, its push rejects against the moved tip, and the provisioner
//!      REBASES the run's commit onto the interloper and re-pushes: the run
//!      delivers clean (not degraded) with the receipt's `rebased` flag
//!      set, and the branch tip is the rebased commit whose PARENT is the
//!      interloper — nobody's work clobbered.
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
use common::{Cluster, poll_until, serial};
use runs::{RunOutcome, RunRecord, RunsMsg, RunsQuery, RunsReply, TurnPolicy};

const CONVERGE: Duration = Duration::from_secs(180);
const FINALIZE: Duration = Duration::from_secs(60);
const ROUND_TRIP: Duration = Duration::from_secs(120);

const AGENT_ID: &str = "quacker-dogfood";
const REPO: &str = "dogfood";
const WORK_BRANCH: &str = "agent/item-1";
/// the worker script's single-line reply; it belongs in the published body.
const REPLY_TITLE: &str = "Dogfood loop proof";
const ISSUE_TITLE: &str = "prove the dogfood loop";
const RACE_REPLY: &str = "raced an interloper";

/// one script-backed provider standing in for a coding agent. the script FILE
/// is rewritten between phases (the env var pins the path, not the content):
/// the worker records its provisioned worktree and edits it; the interloper
/// additionally advances the work branch through the loopback remote first.
struct DogfoodProvider {
    tag: String,
    spec_dir: PathBuf,
    env_var: String,
    script: PathBuf,
    /// one `pwd|ref|head` line per provider execution (`ref` is
    /// `--abbrev-ref HEAD`: literally "HEAD" — the worktree is detached).
    trace_log: PathBuf,
    /// the interloper's pushed commit oid (written only after its push).
    interloper_log: PathBuf,
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
        let provider = Self {
            tag: tag.into(),
            spec_dir,
            env_var,
            script: dir.join("provider.sh"),
            trace_log: dir.join("trace.log"),
            interloper_log: dir.join("interloper.log"),
        };
        provider.write_worker_script();
        provider
    }

    fn write_script(&self, body: &str) {
        std::fs::write(&self.script, body).expect("write provider script");
        use std::os::unix::fs::PermissionsExt as _;
        let mut perms = std::fs::metadata(&self.script)
            .expect("script metadata")
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&self.script, perms).expect("chmod provider script");
    }

    /// runs 1+2: record the provisioned worktree (cwd, ref, HEAD), edit it
    /// (the file content differs per run — it carries the pinned HEAD), reply.
    fn write_worker_script(&self) {
        self.write_script(&format!(
            "#!/bin/sh\n\
             set -e\n\
             cat > /dev/null\n\
             echo \"$(pwd)|$(git rev-parse --abbrev-ref HEAD)|$(git rev-parse HEAD)\" >> {trace}\n\
             git rev-parse HEAD > pinned-base.txt\n\
             printf '%s\\n' '{reply}'\n",
            trace = self.trace_log.display(),
            reply = REPLY_TITLE,
        ));
    }

    /// run 3, two deterministic steps (no sleeps — every wait is a
    /// condition):
    /// 1. advance the work branch through the loopback remote (clone →
    ///    empty commit → push), then CONDITION-POLL `ls-remote` (bounded,
    ///    30 × 1s) until the advertisement — COMMITTED refs — carries the
    ///    interloper tip. the run's worktree is DETACHED at the pin, so the
    ///    committed-ref catch-up cannot reparent the host's later commit.
    /// 2. edit the workspace and reply — the host's commit forks the stale
    ///    pin, its push rejects against the moved tip, and the provisioner
    ///    must rebase-and-re-push.
    fn write_interloper_script(&self, forge_base: &str) {
        self.write_script(&format!(
            "#!/bin/sh\n\
             set -e\n\
             cat > /dev/null\n\
             work=$(pwd)\n\
             echo \"$work|$(git rev-parse --abbrev-ref HEAD)|$(git rev-parse HEAD)\" >> {trace}\n\
             export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null GIT_TERMINAL_PROMPT=0\n\
             tmp=$(mktemp -d)\n\
             git clone -q --branch {branch} {base_url}/{repo} \"$tmp/clone\"\n\
             cd \"$tmp/clone\"\n\
             git -c user.name=interloper -c user.email=i@test.local -c commit.gpgsign=false \\\n\
                 commit -q --allow-empty -m interloper\n\
             tip=$(git rev-parse HEAD)\n\
             git push -q origin {branch}\n\
             n=0\n\
             until [ \"$(git ls-remote origin refs/heads/{branch} | cut -f1)\" = \"$tip\" ]; do\n\
                 n=$((n+1))\n\
                 [ \"$n\" -le 30 ]\n\
                 sleep 1\n\
             done\n\
             printf '%s\\n' \"$tip\" > {interloper}\n\
             cd \"$work\"\n\
             rm -rf \"$tmp\"\n\
             echo stale-pin-run > pinned-base.txt\n\
             printf '%s\\n' '{reply}'\n",
            trace = self.trace_log.display(),
            branch = WORK_BRANCH,
            base_url = forge_base,
            repo = REPO,
            interloper = self.interloper_log.display(),
            reply = RACE_REPLY,
        ));
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

    /// every provider execution's `pwd|ref|head` line, in order.
    fn trace(&self) -> Vec<String> {
        std::fs::read_to_string(&self.trace_log)
            .map(|s| s.lines().map(str::to_string).collect())
            .unwrap_or_default()
    }

    fn interloper_oid(&self) -> String {
        std::fs::read_to_string(&self.interloper_log)
            .expect("the interloper pushed and recorded its oid")
            .trim()
            .to_string()
    }
}

/// `pwd|ref|head` → its three fields.
fn trace_parts(line: &str) -> (String, String, String) {
    let mut parts = line.splitn(3, '|').map(str::to_string);
    (
        parts.next().expect("trace pwd"),
        parts.next().expect("trace branch"),
        parts.next().expect("trace head"),
    )
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
        .map(|i| cluster.wait_marker(i, "genesis app_hash=", Duration::from_secs(60)))
        .collect();
    assert_eq!(genesis[0], genesis[1], "genesis fork between nodes 0 and 1");
    assert_eq!(genesis[0], genesis[2], "genesis fork between nodes 0 and 2");
    for i in 0..3 {
        cluster.wait_marker(i, "converged app_hash=", CONVERGE);
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
        &chat::encode_query(&ChatQuery::MessagesLatest {
            channel_id: channel.into(),
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

/// the run's `workspace_receipt.rebased` flag, read off the RETAINED dispatch
/// outcome (the ring does not carry it; this is the narrowest committed
/// surface that does — the dispatch record keeps the runner-result bytes
/// after delivery).
fn run_receipt_rebased(cluster: &Cluster, idx: usize, run_id: &str) -> Option<bool> {
    let reply = cluster.query(
        idx,
        "dispatch",
        &dispatch::encode_query(&dispatch::DispatchQuery::Dispatch {
            receiver: "runs".into(),
            dispatch_id: runs::dispatch_id_for(run_id),
        }),
    )?;
    let dispatch::DispatchReply::Dispatch(Some(view)) = dispatch::decode_reply(&reply).ok()?
    else {
        return None;
    };
    let bytes = view.outcome?.ok()?;
    let result: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    result["workspace_receipt"]["rebased"].as_bool()
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
fn issue_mention_runs_a_worktree_opens_a_pr_and_the_pr_session_survives_a_cas_race() {
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
    cluster.extra_toml.push("announce_capabilities = true".into());
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
    let main_tip = git_stdout(seed.path(), &["rev-parse", "HEAD"]);
    let seed_url = format!("http://127.0.0.1:{}/forge/{REPO}", cluster.http_ports[0]);
    git_ok(seed.path(), &["remote", "add", "origin", &seed_url]);
    git_ok(seed.path(), &["push", "origin", "main"]);
    // committed on the EXECUTING node before any run pins it.
    poll_until("the seed push to finalize on node 1", CONVERGE, || {
        (branch_tip(&cluster, 1, "main")? == main_tip).then_some(())
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

    // ---- run 1: issue mention → worktree at the pinned main tip → branch
    //      `agent/item-1` born → a PR titled by the bound Forge issue.
    post_mention(&cluster, 0, &issue_channel, "m1");
    let run_1 = runs::run_id_for(&issue_channel, seq_of(&cluster, 0, &issue_channel, "m1"), AGENT_ID);
    assert_eq!(
        wait_for_reply(&cluster, 0, &issue_channel, &run_1),
        REPLY_TITLE,
        "run 1 replies in the issue channel"
    );

    let run1_oid = poll_until("branch agent/item-1 to be born", FINALIZE, || {
        branch_tip(&cluster, 0, WORK_BRANCH)
    });
    assert_ne!(run1_oid, main_tip, "the run pushed a NEW commit");

    // the provider ran in a REAL worktree: DETACHED at the pinned main tip
    // (the work branch is push-time only), under the operator-rooted run tree.
    let trace = provider.trace();
    assert_eq!(trace.len(), 1, "one provider execution so far: {trace:?}");
    let (cwd, head_ref, head) = trace_parts(&trace[0]);
    assert!(
        PathBuf::from(&cwd).starts_with(&runs_root),
        "the worktree honors DUCKTAPE_AGENT_RUNS_ROOT: {cwd}"
    );
    assert_eq!(head_ref, "HEAD", "the worktree is DETACHED at the pin");
    assert_eq!(head, main_tip, "run 1 forks the pinned main tip");

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
    assert_eq!(pr.target_branch.as_deref(), Some("main"));
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
    let trace = provider.trace();
    assert_eq!(trace.len(), 2, "two provider executions: {trace:?}");
    let (_, _, head) = trace_parts(&trace[1]);
    assert_eq!(head, run1_oid, "run 2 forks the branch TIP (the session continues)");

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
    assert_eq!(git_stdout(&dest, &["rev-parse", "HEAD~2"]), main_tip);

    assert_eq!(open_pr_count(&cluster, 0), 1, "the duplicate guard opened NO second PR");
    let record = poll_until("run 2 in the delivered-runs ring", FINALIZE, || {
        run_record(&cluster, 0, &run_2)
    });
    assert!(!record.degraded, "run 2 is clean: {record:?}");
    assert_eq!(record.output_ref.as_deref(), Some(format!("{WORK_BRANCH}@{run2_oid}").as_str()));
    assert_eq!(record.pr_number, Some(pr_number), "the ring names the UPDATED PR");

    // ---- run 3: the ordering proof. the interloper script advances the
    //      branch through node 1's loopback remote and awaits its committed
    //      visibility (see its doc) — the detached host commit then forks
    //      the stale pin, its push rejects against the moved tip, and the
    //      provisioner rebases onto the interloper and re-pushes.
    provider.write_interloper_script(&format!("http://127.0.0.1:{}/forge", cluster.http_ports[1]));
    post_mention(&cluster, 0, &pr_channel, "m3");
    let run_3 = runs::run_id_for(&pr_channel, seq_of(&cluster, 0, &pr_channel, "m3"), AGENT_ID);
    assert_eq!(
        wait_for_reply(&cluster, 0, &pr_channel, &run_3),
        RACE_REPLY,
        "the raced run still delivers its reply"
    );

    let interloper_oid = provider.interloper_oid();
    assert_ne!(interloper_oid, run2_oid, "the interloper minted a new commit");
    let record = poll_until("run 3 in the delivered-runs ring", FINALIZE, || {
        run_record(&cluster, 0, &run_3)
    });
    assert_eq!(record.outcome, RunOutcome::Delivered);
    assert!(!record.degraded, "ordering is solved by rebase, never a degrade: {record:?}");
    assert_eq!(record.pr_number, Some(pr_number), "still the ONE open PR");
    let run3_oid = record
        .output_ref
        .as_deref()
        .and_then(|r| r.strip_prefix(&format!("{WORK_BRANCH}@")))
        .unwrap_or_else(|| panic!("run 3 pushed its rebased commit: {record:?}"))
        .to_string();
    assert_ne!(run3_oid, interloper_oid, "the run's own commit landed, rebased");
    assert_eq!(
        run_receipt_rebased(&cluster, 0, &run_3),
        Some(true),
        "the receipt carries the rebased flag"
    );

    // the committed tip is the run's REBASED commit and the interloper is
    // its parent — both bodies of work survive, in interloper-first order.
    assert_eq!(
        branch_tip(&cluster, 0, WORK_BRANCH),
        Some(run3_oid.clone()),
        "the rebased push moved the branch"
    );
    let dest = checkout.path().join("after-run3");
    git_ok(
        checkout.path(),
        &["clone", "--quiet", "--branch", WORK_BRANCH, &clone_url, dest.to_str().unwrap()],
    );
    assert_eq!(git_stdout(&dest, &["rev-parse", "HEAD"]), run3_oid);
    assert_eq!(
        git_stdout(&dest, &["rev-parse", "HEAD^"]),
        interloper_oid,
        "the rebase parented the run's commit on the interloper"
    );
    assert_eq!(git_stdout(&dest, &["rev-parse", "HEAD~2"]), run2_oid);

    let trace = provider.trace();
    assert_eq!(trace.len(), 3, "three provider executions: {trace:?}");
    let (_, _, head) = trace_parts(&trace[2]);
    assert_eq!(head, run2_oid, "run 3 was pinned at run 2's tip (pre-interloper)");
    assert_eq!(open_pr_count(&cluster, 0), 1, "no PR was born from the raced run");
}
