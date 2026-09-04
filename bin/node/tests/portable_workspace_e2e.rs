//! live portable-runtime e2e: REAL `ducktape` validators, a REAL
//! script-backed provider, and the REAL `NodedProvisioner` driving duckfs
//! checkout/commit — the full loop this repo's unit suites only mock:
//!
//!   mention -> portable envelope (files module wired) -> lease on the providing
//!   node -> duckfs checkout of the agent's pinned source at a per-run dir
//!   OUTSIDE storage (D7), the agent's skill tree checked out read-only
//!   BESIDE it (W6) -> the provider executes INSIDE the mount and
//!   writes a file -> commit mints the output_ref -> RunnerResult delivery
//!   posts the reply -> the committed files state carries the artifact on
//!   EVERY node -> the NEXT run's checkout materializes it (W2 chaining).
//!
//! the agent carries NO prompt pin — there is no such thing any more. its
//! PERSONA is a curated `Always` skill: a duckfs document, committed here, that
//! the provisioner materializes as a ro mount on the EXECUTING node and the
//! assembler inlines into the run's context document. the provider records the
//! bytes it was actually handed, so this leg proves the persona reached the
//! model THROUGH THE SOUL — the replacement for the retired prompt-blob path,
//! and (since the skill is written on node 0 and the run executes on node 1)
//! the replacement for its cross-node fetch lane too: consensus replicates the
//! document, so nothing has to fetch it.
//!
//! it also proves the LIBRARY paragraph is cap-gated end to end: the agent is
//! granted `duckfs_read` over the shared library prefix, and the assembled
//! document it receives tells it the library is there.

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use agent::{ACTION_CHAT_POST, AgentMsg, SkillRef};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use capability::{CapabilityQuery, CapabilityReply};
use chat::{AuthorRef, Block, ChatMsg, ChatQuery, ChatReply, Mark, PostPolicy, Span};
use common::{Cluster, sandbox_toml, skip_unless_sandboxed};
use duckfs_core::{
    Change, Content, FilesMsg, FilesQuery, FilesReply, decode_reply as files_decode_reply,
    encode_msg as files_encode_msg, encode_query as files_encode_query,
};
use runs::{RunsMsg, RunsQuery, RunsReply, TurnPolicy};

const CONVERGE: Duration = Duration::from_secs(180);
const FINALIZE: Duration = Duration::from_secs(60);
const ROUND_TRIP: Duration = Duration::from_secs(120);

const AGENT_ID: &str = "quacker-portable";
const CHANNEL: &str = "portable";
const ARTIFACT: &str = "agent-artifact.txt";
const ARTIFACT_BODY: &str = "portable evidence";
// the W6 skill: a committed duckfs subtree the agent pins, materialized by
// the provisioner as a READ-ONLY mount beside (never inside) the rw mount. it is
// curated `Always`, which makes it this agent's PERSONA: the assembler inlines
// its whole body into the run's context document, and the provider hands that
// document to a `prompt = "stdin"` provider ahead of the run's input.
const SKILL_NAME: &str = "quackskill";
const SKILL_FILE: &str = "SKILL.md";
const SKILL_BODY: &str = "You are the portable duck. The way of the quack is patience.";
const SKILL_PREFIX: &str = "/shared/skills/quackskill";

/// one script-backed provider that behaves like a coding agent: it records
/// its cwd (the provisioned mount), proves whether the PREVIOUS run's
/// committed artifact was materialized (content-checked), writes the
/// artifact, and answers.
struct PortableProvider {
    tag: String,
    spec_dir: PathBuf,
    env_var: String,
    bin: PathBuf,
}

/// Where the executor records what only it can attest, one file per fact.
///
/// These are written INTO THE WORKSPACE and read back out of COMMITTED duckfs
/// state, which is the only host↔guest evidence channel a microVM run has: the
/// guest shares no filesystem with the host, so a log dir under the node's
/// `$HOME` would simply not exist on the other side. Riding the workspace makes
/// the evidence stronger than the old side channel, too — a line the host can
/// read is a line that went through checkout, the run, commit and consensus.
///
/// They ACCUMULATE across runs for free: each run's checkout materializes the
/// previous run's commit, so `>>` from run 2 lands after run 1's line.
const EVIDENCE_CWD: &str = "evidence-cwd.log";
const EVIDENCE_CHAIN: &str = "evidence-chain.log";
const EVIDENCE_SKILLS: &str = "evidence-skills.log";
const EVIDENCE_PROMPT: &str = "evidence-prompt.log";

/// Records every per-run directory the provisioner creates under `runs_root`,
/// sampled from the HOST while the runs are in flight.
///
/// The child cannot report these. A sandboxed run's workdir is mounted at the
/// SAME guest path for every run (`/duck/workspace`) — that normalization is
/// the isolation working as designed, and it makes two attempts literally
/// indistinguishable from inside the guest. So the properties that are about
/// the HOST layout (distinct per attempt, under the operator root, cleaned up
/// afterwards) have to be observed on the host.
///
/// This also makes W5 checkable at all. The old code asserted
/// `!PathBuf::from(guest_cwd).exists()` on the host, which is vacuously true for
/// a path that only ever existed inside the guest: the cleanup poll passed
/// without ever witnessing a cleanup.
struct RunDirs {
    seen: std::sync::Arc<std::sync::Mutex<std::collections::BTreeSet<PathBuf>>>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl RunDirs {
    /// sample every 20 ms — a run dir lives for seconds, so this cannot miss one
    /// without the run itself being instantaneous.
    fn watch(runs_root: PathBuf) -> Self {
        let seen = std::sync::Arc::new(std::sync::Mutex::new(std::collections::BTreeSet::new()));
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (s, st) = (seen.clone(), stop.clone());
        let handle = std::thread::spawn(move || {
            while !st.load(std::sync::atomic::Ordering::Relaxed) {
                // `<runs_root>/<per-node salt>/<slug>` — the run dirs and their
                // `-ro` skill siblings both sit at this depth.
                if let Ok(salts) = std::fs::read_dir(&runs_root) {
                    for salt in salts.flatten() {
                        if let Ok(entries) = std::fs::read_dir(salt.path()) {
                            for e in entries.flatten() {
                                if e.path().is_dir() {
                                    s.lock().expect("run dir set").insert(e.path());
                                }
                            }
                        }
                    }
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        });
        Self {
            seen,
            stop,
            handle: Some(handle),
        }
    }

    /// the rw run dirs (the `-ro` skill siblings excluded), sorted.
    fn workdirs(&self) -> Vec<PathBuf> {
        self.all().into_iter().filter(|p| !Self::is_ro(p)).collect()
    }

    /// the read-only skill roots the provisioner materialized beside them.
    fn skill_roots(&self) -> Vec<PathBuf> {
        self.all().into_iter().filter(|p| Self::is_ro(p)).collect()
    }

    fn all(&self) -> Vec<PathBuf> {
        self.seen
            .lock()
            .expect("run dir set")
            .iter()
            .cloned()
            .collect()
    }

    fn is_ro(path: &std::path::Path) -> bool {
        path.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with("-ro"))
    }
}

impl Drop for RunDirs {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

/// the stand-in coding agent, as a shell one-liner: KEEP the prompt it was
/// handed (the assembled soul rides it), record the provisioned cwd, prove the
/// prior run's artifact chained in (content-checked), prove the skill ro mount
/// materialized (content-checked, logging the advertised ro root), write this
/// run's artifact, answer.
///
/// It rides the spec's ARGV rather than a staged `provider.sh`, because a run
/// executes inside a microVM that mounts nothing from the host — an executor a
/// node lends has to already be in the guest rootfs. A host script reaches the
/// guest as `execve /opt/duck/bin/provider.sh` and exit 126.
///
/// Every path it writes is CWD-RELATIVE, which is the workspace: that is the
/// one directory whose contents come back to the host (as committed duckfs
/// state, once the run commits). A host absolute path baked in here would name
/// nothing at all inside the guest, and the run would still answer on stdout —
/// a healthy-looking run that merely "saw no skill mount".
fn portable_agent_body() -> String {
    format!(
        "cat >> {prompt}; \
         pwd >> {cwd}; \
         if [ -f {artifact} ] && [ \"$(cat {artifact})\" = '{body}' ]; then \
         echo chained >> {chain}; fi; \
         if [ \"$(cat \"$DUCKTAPE_RUN_SKILLS/{skill_name}/{skill_file}\" 2>/dev/null)\" = '{skill_body}' ]; then \
         printf '%s\\n' \"$DUCKTAPE_RUN_SKILLS\" >> {skills}; fi; \
         printf '%s' '{body}' > {artifact}; \
         printf '%s\\n' 'portable run done'",
        cwd = EVIDENCE_CWD,
        chain = EVIDENCE_CHAIN,
        skills = EVIDENCE_SKILLS,
        prompt = EVIDENCE_PROMPT,
        artifact = ARTIFACT,
        body = ARTIFACT_BODY,
        skill_name = SKILL_NAME,
        skill_file = SKILL_FILE,
        skill_body = SKILL_BODY,
    )
}

/// a shell body as the spec's `args` array: TOML-escape it and hand it to
/// `sh -c`.
fn argv_literal(body: &str) -> String {
    let escaped = body.replace('\\', "\\\\").replace('"', "\\\"");
    format!("[\"-c\", \"{escaped}\"]")
}

impl PortableProvider {
    fn stage(root: &std::path::Path) -> Self {
        let dir = root.join("portable-provider");
        let spec_dir = dir.join("specs");
        std::fs::create_dir_all(&spec_dir).expect("provider spec dir");
        // resolved by basename to /opt/duck/bin/sh inside the guest
        let bin = PathBuf::from("/bin/sh");

        let tag = "quack-portable";
        let env_var = "DUCKTAPE_TEST_QUACK_PORTABLE_BIN".to_string();
        std::fs::write(
            spec_dir.join(format!("{tag}.toml")),
            format!(
                "spec = 1\n\
                 [capability]\n\
                 tag = \"{tag}\"\n\
                 description = \"portable e2e script executor\"\n\
                 [detect]\n\
                 bin = \"{tag}-nonexistent-cli\"\n\
                 env = \"{env_var}\"\n\
                 [invoke]\n\
                 args = {args}\n\
                 prompt = \"stdin\"\n\
                 timeout_secs = 30\n\
                 [output]\n\
                 format = \"text\"\n",
                args = argv_literal(&portable_agent_body()),
            ),
        )
        .expect("write provider spec");
        Self {
            tag: tag.into(),
            spec_dir,
            env_var,
            bin,
        }
    }

    fn env(&self) -> Vec<(String, String)> {
        vec![
            (
                "DUCKTAPE_CAPABILITY_DIR".into(),
                self.spec_dir.display().to_string(),
            ),
            (self.env_var.clone(), self.bin.display().to_string()),
        ]
    }
}

// This suite's stand-in coding agent used to name its own image, a
// fuller base than the harness default. Every node now boots the same shared
// guest rootfs, so what a run can execute is decided when that image is built
// (ops/build-guest-rootfs.sh), not per suite.

/// hermetic env for a node that must provide NOTHING (see dispatch_e2e).
fn hermetic_env(root: &std::path::Path, name: &str) -> Vec<(String, String)> {
    let empty = root.join(name).join("specs");
    std::fs::create_dir_all(&empty).expect("empty spec dir");
    let missing = root.join(name).join("missing-executor");
    vec![
        (
            "DUCKTAPE_CAPABILITY_DIR".into(),
            empty.display().to_string(),
        ),
        ("DUCKTAPE_CLAUDE_BIN".into(), missing.display().to_string()),
        ("DUCKTAPE_CODEX_BIN".into(), missing.display().to_string()),
    ]
}

fn hide_builtins(root: &std::path::Path, name: &str) -> Vec<(String, String)> {
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

fn mention(cluster: &Cluster, idx: usize, message_id: &str) {
    cluster.submit(
        idx,
        "chat",
        &chat::encode_msg(&ChatMsg::PostMessage {
            channel_id: CHANNEL.into(),
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
                Span::plain(" do the portable thing"),
            ])],
            thread: None,
            as_agent: None,
        }),
    );
}

fn wait_for_reply(cluster: &Cluster, idx: usize, run_id: &str) -> String {
    cluster.await_committed(idx, "the agent reply to post", ROUND_TRIP, || {
        let reply = cluster.query(
            idx,
            "chat",
            &chat::encode_query(&ChatQuery::MessagesRange {
                channel_id: CHANNEL.into(),
                from_seq: 1,
                limit: 64,
            }),
        )?;
        let ChatReply::Messages(views) = chat::decode_reply(&reply).ok()? else {
            return None;
        };
        views.into_iter().find_map(|v| {
            (v.head.message_id == format!("agent/{run_id}")).then(|| {
                v.head
                    .blocks
                    .iter()
                    .map(|b| match b {
                        Block::Paragraph(spans) | Block::Quote(spans) => {
                            spans.iter().map(|s| s.text.as_str()).collect::<String>()
                        }
                        Block::Code { text, .. } => text.clone(),
                        Block::Divider => String::new(),
                    })
                    .collect::<String>()
            })
        })
    })
}

/// the committed files-module view of `path` on `idx` — `Some(size)` when the
/// committed head carries it.
fn stat_size(cluster: &Cluster, idx: usize, path: &str) -> Option<u64> {
    let reply = cluster.query(
        idx,
        "files",
        &files_encode_query(&FilesQuery::Stat {
            path: path.into(),
            snapshot: None,
        }),
    )?;
    match files_decode_reply(&reply) {
        Ok(FilesReply::Stat(Some(info))) => Some(info.size),
        _ => None,
    }
}

fn artifact_stat(cluster: &Cluster, idx: usize) -> Option<u64> {
    stat_size(
        cluster,
        idx,
        &format!("/shared/agent-workspaces/{AGENT_ID}/{ARTIFACT}"),
    )
}

/// one evidence file, out of the committed head — the guest's own attestation,
/// read back on the host after it crossed commit and consensus.
///
/// Empty for a file the head does not carry, which is what a run that never
/// executed (or never saw the thing it was asked to check) leaves behind.
fn evidence(cluster: &Cluster, idx: usize, name: &str) -> String {
    let path = format!("/shared/agent-workspaces/{AGENT_ID}/{name}");
    let Some(size) = stat_size(cluster, idx, &path) else {
        return String::new();
    };
    let Some(reply) = cluster.query(
        idx,
        "files",
        &files_encode_query(&FilesQuery::Read {
            path,
            snapshot: None,
            offset: 0,
            len: size,
        }),
    ) else {
        return String::new();
    };
    let Ok(FilesReply::Read { b64, .. }) = files_decode_reply(&reply) else {
        return String::new();
    };
    STANDARD
        .decode(b64)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .unwrap_or_default()
}

/// every line of one evidence file — one per run that reported the fact.
fn evidence_lines(cluster: &Cluster, idx: usize, name: &str) -> Vec<String> {
    evidence(cluster, idx, name)
        .lines()
        .map(str::to_string)
        .collect()
}

#[test]
fn a_portable_run_materializes_commits_and_chains_a_real_duckfs_workspace() {
    if skip_unless_sandboxed(
        "a_portable_run_materializes_commits_and_chains_a_real_duckfs_workspace",
    )
    .is_some()
    {
        return;
    }
    let fixtures = tempfile::TempDir::new().expect("provider fixtures dir");
    let provider = PortableProvider::stage(fixtures.path());
    // the operator-facing root override: one shared base, per-node storage
    // salt keeps the co-located validators' run trees disjoint.
    let runs_root = fixtures.path().join("agent-runs");
    // start watching BEFORE any run: the per-run dirs are created and removed
    // inside a run, so a post-hoc look would find nothing either way.
    std::fs::create_dir_all(&runs_root).expect("runs root");
    let run_dirs = RunDirs::watch(runs_root.clone());
    let runs_root_env = (
        "DUCKTAPE_AGENT_RUNS_ROOT".to_string(),
        runs_root.display().to_string(),
    );

    let mut cluster = Cluster::new(&[0, 1, 2], &[0, 1, 2]);
    // serving is opt-in now (default OFF): this test needs node 1 in the
    // rendezvous pool, so every node opts in. the [sandbox] table comes LAST
    // (nothing may follow a toml table header) — every node boots a microVM
    // compute plane; nodes 0/2 stay hermetic (empty spec dir → nothing
    // discovered or announced).
    cluster.extra_toml.extend(sandbox_toml());
    // the pool needs the compute grant as well as the [sandbox] table, and
    // the grant is what opts these nodes into the rendezvous pool: the node
    // announces the granted tags INTERSECTED with what it discovers, so the
    // hermetic nodes 0/2 still announce nothing.
    cluster.compute_grant = Some(vec![provider.tag.clone()]);
    cluster.env[0] = [
        hermetic_env(fixtures.path(), "node0"),
        vec![runs_root_env.clone()],
    ]
    .concat();
    cluster.env[1] = [
        provider.env(),
        hide_builtins(fixtures.path(), "node1"),
        vec![runs_root_env.clone()],
    ]
    .concat();
    cluster.env[2] = [hermetic_env(fixtures.path(), "node2"), vec![runs_root_env]].concat();
    boot(&mut cluster);

    // node 1 is the tag's ONLY provider, so every lease lands there.
    cluster.await_committed(0, "the provider to announce", FINALIZE, || {
        (providers(&cluster, 0, &provider.tag)? == vec![Cluster::identity(1)]).then_some(())
    });

    // W6: seed the skill subtree in COMMITTED files state before any run
    // composes — the agent pins it below and every portable run must
    // materialize it as a read-only mount beside the rw workspace.
    cluster.submit(
        0,
        "files",
        &files_encode_msg(&FilesMsg::Commit {
            base_snapshot: None,
            message: "seed skill".into(),
            changes: vec![Change::Put {
                path: format!("{SKILL_PREFIX}/{SKILL_FILE}"),
                exec: false,
                meta: BTreeMap::new(),
                content: Content::Inline {
                    b64: STANDARD.encode(SKILL_BODY),
                },
            }],
        }),
    );
    cluster.await_committed(0, "the skill seed to commit", FINALIZE, || {
        (stat_size(&cluster, 0, &format!("{SKILL_PREFIX}/{SKILL_FILE}"))?
            == SKILL_BODY.len() as u64)
            .then_some(())
    });

    cluster.submit(
        0,
        "chat",
        &chat::encode_msg(&ChatMsg::CreateChannel {
            channel_id: CHANNEL.into(),
            name: "Portable".into(),
            post_policy: PostPolicy::Open,
        }),
    );
    cluster.submit(
        0,
        "agent",
        &agent::encode_msg(&AgentMsg::RegisterAgent {
            agent_id: AGENT_ID.into(),
            display_name: AGENT_ID.into(),
            capability: provider.tag.clone(),
            allowed_actions: vec![ACTION_CHAT_POST.into()],
            recipe_hash: None,
            // the library grant the app pre-fills on every new agent: an
            // ordinary duckfs_read cap over the shared skill library. it is what
            // earns the assembled document its library paragraph (and what the
            // MCP tool plane would gate a real grep/read on) — ungranted, the
            // document must never mention a door the tool plane would slam.
            caps: Some(agent::ResourceCaps {
                duckfs_read: vec![agent::SKILL_LIBRARY_PREFIX.into()],
                ..Default::default()
            }),
            // a TRACKING skill (no pin): the composer resolves it to the
            // committed head, the provisioner mounts it read-only (W6). curated
            // `Always`, so it is this agent's PERSONA: the assembler inlines its
            // body into the run's context document — the lane that replaced the
            // prompt blob.
            skills: Some(vec![SkillRef {
                name: SKILL_NAME.into(),
                source_prefix: SKILL_PREFIX.into(),
                source_snapshot: None,
                load: agent::LoadMode::Always,
            }]),
        }),
    );
    cluster.submit(
        0,
        "runs",
        &runs::encode_msg(&RunsMsg::WatchChannel {
            channel_id: CHANNEL.into(),
            policy: TurnPolicy::Mention,
        }),
    );
    cluster.await_committed(0, "the channel watch to commit", FINALIZE, || {
        let reply = cluster.query(0, "runs", &runs::encode_query(&RunsQuery::Watches))?;
        match runs::decode_reply(&reply) {
            Ok(RunsReply::Watches(w)) => w.iter().any(|v| v.channel_id == CHANNEL).then_some(()),
            _ => None,
        }
    });

    // ---- run 1: the agent's workspace prefix has no content yet -> an EMPTY
    // rw checkout (the cold-start case) beside the materialized skill mount;
    // the script writes the artifact, commit mints the output_ref, the reply
    // delivers.
    mention(&cluster, 0, "m1");
    let run_1 = runs::run_id_for(CHANNEL, 1, AGENT_ID);
    assert_eq!(wait_for_reply(&cluster, 0, &run_1), "portable run done");

    // W6 evidence: the script content-checked the skill file under the
    // advertised DUCKTAPE_RUN_SKILLS root before answering. It reaches the host
    // as committed state, so wait for the run's commit rather than the reply.
    let roots = cluster.await_committed(
        0,
        "run 1's skill-mount evidence to commit",
        FINALIZE,
        || {
            let roots = evidence_lines(&cluster, 0, EVIDENCE_SKILLS);
            (!roots.is_empty()).then_some(roots)
        },
    );
    assert_eq!(roots.len(), 1, "run 1 saw its skill ro mount: {roots:?}");

    // THE SOUL, as the model saw it. the persona is no longer a blob resolved
    // from a hash — it is this curated `Always` skill, seeded on node 0,
    // replicated by consensus, materialized on node 1 (the executor), inlined by
    // the assembler, and handed to the provider on stdin. every one of those
    // links is live in this assertion.
    let prompt = evidence(&cluster, 0, EVIDENCE_PROMPT);
    assert!(
        prompt.contains(SKILL_BODY),
        "the persona must reach the model through the assembled context document: {prompt}"
    );
    assert!(
        prompt.contains(&format!("# {SKILL_NAME}")),
        "the persona is headed by its CURATED name (never a name read out of the doc): {prompt}"
    );
    assert!(
        prompt.contains("A Ducktape MCP tool server"),
        "the ambient tool-plane instruction ships with every run: {prompt}"
    );
    // GAP 2, end to end: the agent HAS the library read cap, so the document
    // tells it the library exists and names the tools that open it. an agent
    // without the cap is never told (proved in compute_service::soul).
    assert!(
        prompt.contains("## The shared skill library"),
        "a library-granted agent is told the library is there: {prompt}"
    );
    assert!(
        prompt.contains("ducktape_files_grep") && prompt.contains(agent::SKILL_LIBRARY_PREFIX),
        "…and told, by name, the tool and prefix that open it: {prompt}"
    );

    // the artifact is COMMITTED duckfs state, readable on a node that never
    // executed anything (the whole point of the portable workspace).
    cluster.await_committed(0, "the artifact to reach committed state", FINALIZE, || {
        (artifact_stat(&cluster, 0)? == ARTIFACT_BODY.len() as u64).then_some(())
    });
    assert_eq!(
        artifact_stat(&cluster, 2),
        Some(ARTIFACT_BODY.len() as u64),
        "the artifact is present on every validator"
    );

    // the W6 no-leak property: the skill ro root is a SIBLING of the commit
    // root, so the run's output snapshot must NOT capture the skill content —
    // neither as a subtree named after the mount nor spilled at the root.
    for leak in [
        format!("/shared/agent-workspaces/{AGENT_ID}/{SKILL_NAME}/{SKILL_FILE}"),
        format!("/shared/agent-workspaces/{AGENT_ID}/{SKILL_FILE}"),
    ] {
        assert_eq!(
            stat_size(&cluster, 0, &leak),
            None,
            "skill content leaked into the output snapshot at {leak}"
        );
    }

    // ---- run 2: the composer pins the ADVANCED head, so the checkout must
    // materialize run 1's committed artifact (W2 chaining) — content-checked
    // inside the mount by the script itself.
    mention(&cluster, 0, "m2");
    let run_2 = runs::run_id_for(CHANNEL, 3, AGENT_ID);
    assert_eq!(wait_for_reply(&cluster, 2, &run_2), "portable run done");
    cluster.await_committed(0, "run 2's chain evidence to commit", FINALIZE, || {
        (evidence_lines(&cluster, 0, EVIDENCE_CHAIN).len() == 1).then_some(())
    });

    // ---- the mounts themselves: two runs, two DISTINCT per-run dirs, every
    // one under the operator root (salted per node) — which lives in the
    // fixtures tempdir, DISJOINT from every node's storage tree (D7; the
    // root's boot validation refuses a storage-resident override) — and both
    // cleaned up after their run (W5).
    // GUEST-side evidence: the child ran once per run and reported a cwd. Under
    // a sandbox that cwd is the normalized guest path, identical for both runs —
    // so it proves execution happened, and nothing about the host layout.
    let cwds = evidence_lines(&cluster, 0, EVIDENCE_CWD);
    assert_eq!(
        cwds.len(),
        2,
        "exactly one provider execution per run: {cwds:?}"
    );

    // HOST-side evidence: the layout properties, observed where they are true.
    let workdirs = run_dirs.workdirs();
    assert_eq!(
        workdirs.len(),
        2,
        "one per-run dir per run, under DUCKTAPE_AGENT_RUNS_ROOT: {workdirs:?}"
    );
    assert_ne!(workdirs[0], workdirs[1], "per-attempt dirs never collide");
    for dir in &workdirs {
        assert!(
            dir.starts_with(&runs_root),
            "the mount honors DUCKTAPE_AGENT_RUNS_ROOT: {}",
            dir.display()
        );
        cluster.await_committed(0, "the run dir to be cleaned up (W5)", FINALIZE, || {
            (!dir.exists()).then_some(())
        });
    }

    // ---- the skill ro mounts: one content-checked root per run, each under
    // the operator root, each a SIBLING of its run's rw mount (never inside
    // it — that is the no-leak mechanism), and each cleaned up (W5).
    // GUEST side: each run actually READ the skill through its ro mount and the
    // bytes matched — the one thing only the child can attest, and the whole
    // point of the mount. (This is what silently regressed: with the evidence
    // channel broken it logged nothing, and the run still answered, which is
    // exactly the "silently unsouled agent" this feature must never ship.)
    let roots = evidence_lines(&cluster, 0, EVIDENCE_SKILLS);
    assert_eq!(
        roots.len(),
        2,
        "both runs content-checked the skill: {roots:?}"
    );

    // HOST side: one ro root per run, each a SIBLING of its rw mount (never
    // inside it — that is the no-leak mechanism), each cleaned up.
    let skill_roots = run_dirs.skill_roots();
    assert_eq!(
        skill_roots.len(),
        2,
        "one skill ro root per run: {skill_roots:?}"
    );
    assert_ne!(
        skill_roots[0], skill_roots[1],
        "per-run skill roots never collide"
    );
    for dir in &skill_roots {
        assert!(
            dir.starts_with(&runs_root),
            "the skill root lives under the runs root: {}",
            dir.display()
        );
        assert!(
            !workdirs.iter().any(|w| dir.starts_with(w)),
            "the skill root is a sibling of the rw mount, never inside it: {}",
            dir.display()
        );
        cluster.await_committed(
            0,
            "the skill ro root to be cleaned up (W5)",
            FINALIZE,
            || (!dir.exists()).then_some(()),
        );
    }

    // no correlation debris: every delivered run prunes its pending entry.
    // eventual — the reply was observed on node 2, and node 0 may still be a
    // block behind the delivery that prunes.
    cluster.await_committed(
        0,
        "delivered runs to prune their pending entries",
        FINALIZE,
        || {
            let reply = cluster.query(0, "runs", &runs::encode_query(&RunsQuery::PendingRuns))?;
            match runs::decode_reply(&reply) {
                Ok(RunsReply::PendingRuns(pending)) => pending.is_empty().then_some(()),
                _ => None,
            }
        },
    );
}
