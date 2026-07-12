//! live portable-runtime e2e: REAL `ducktape-node` validators, a REAL
//! script-backed provider, and the REAL `NodedProvisioner` driving duckfs
//! checkout/commit — the full ADR loop this repo's unit suites only mock:
//!
//!   mention -> v3 envelope (files module wired) -> lease on the providing
//!   node -> duckfs checkout of the agent's pinned source at a per-run dir
//!   OUTSIDE storage (D7), the agent's skill tree checked out read-only
//!   BESIDE it (W6) -> the provider executes INSIDE the mount and
//!   writes a file -> commit mints the output_ref -> RunnerResult delivery
//!   posts the reply -> the committed files state carries the artifact on
//!   EVERY node -> the NEXT run's checkout materializes it (W2 chaining).
//!
//! the agent is registered with a REAL prompt pin whose blob is uploaded to
//! the executing node's blob lane (`POST /v1/files/blob`) — the strict
//! no-fallback prompt path, satisfied rather than sidestepped.

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use agent::{ACTION_CHAT_POST, AgentMsg, SkillRef};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use capability::{CapabilityQuery, CapabilityReply};
use chat::{AuthorRef, Block, ChatMsg, ChatQuery, ChatReply, Mark, PostPolicy, Span};
use common::{Cluster, poll_until, serial};
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
// the provisioner as a READ-ONLY mount beside (never inside) the rw mount.
const SKILL_NAME: &str = "quackskill";
const SKILL_FILE: &str = "SKILL.md";
const SKILL_BODY: &str = "the way of the quack";
const SKILL_PREFIX: &str = "/shared/skills/quackskill";

/// one script-backed provider that behaves like a coding agent: it records
/// its cwd (the provisioned mount), proves whether the PREVIOUS run's
/// committed artifact was materialized (content-checked), writes the
/// artifact, and answers.
struct PortableProvider {
    tag: String,
    spec_dir: PathBuf,
    env_var: String,
    script: PathBuf,
    cwd_log: PathBuf,
    chain_log: PathBuf,
    skills_log: PathBuf,
}

impl PortableProvider {
    fn stage(root: &std::path::Path) -> Self {
        let dir = root.join("portable-provider");
        let spec_dir = dir.join("specs");
        std::fs::create_dir_all(&spec_dir).expect("provider spec dir");
        let cwd_log = dir.join("cwd.log");
        let chain_log = dir.join("chain.log");
        let skills_log = dir.join("skills.log");
        let script = dir.join("provider.sh");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\n\
                 # a stand-in coding agent: drain the prompt, record the\n\
                 # provisioned cwd, prove the prior run's artifact chained in\n\
                 # (content-checked), prove the skill ro mount materialized\n\
                 # (content-checked, logging the advertised ro root), write\n\
                 # this run's artifact, answer.\n\
                 cat > /dev/null\n\
                 pwd >> {cwd}\n\
                 if [ -f {artifact} ] && [ \"$(cat {artifact})\" = '{body}' ]; then\n\
                 \techo chained >> {chain}\n\
                 fi\n\
                 if [ \"$(cat \"$DUCKTAPE_RUN_SKILLS/{skill_name}/{skill_file}\" 2>/dev/null)\" = '{skill_body}' ]; then\n\
                 \tprintf '%s\\n' \"$DUCKTAPE_RUN_SKILLS\" >> {skills}\n\
                 fi\n\
                 printf '%s' '{body}' > {artifact}\n\
                 printf '%s\\n' 'portable run done'\n",
                cwd = cwd_log.display(),
                chain = chain_log.display(),
                skills = skills_log.display(),
                artifact = ARTIFACT,
                body = ARTIFACT_BODY,
                skill_name = SKILL_NAME,
                skill_file = SKILL_FILE,
                skill_body = SKILL_BODY,
            ),
        )
        .expect("write provider script");
        use std::os::unix::fs::PermissionsExt as _;
        let mut perms = std::fs::metadata(&script).expect("script metadata").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).expect("chmod provider script");

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
                 args = []\n\
                 prompt = \"stdin\"\n\
                 timeout_secs = 30\n\
                 [output]\n\
                 format = \"text\"\n"
            ),
        )
        .expect("write provider spec");
        Self {
            tag: tag.into(),
            spec_dir,
            env_var,
            script,
            cwd_log,
            chain_log,
            skills_log,
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

    /// every provisioned cwd the script observed, one per run.
    fn cwds(&self) -> Vec<String> {
        std::fs::read_to_string(&self.cwd_log)
            .map(|s| s.lines().map(str::to_string).collect())
            .unwrap_or_default()
    }

    /// how many runs found the prior run's committed artifact in their mount.
    fn chained(&self) -> usize {
        std::fs::read_to_string(&self.chain_log)
            .map(|s| s.lines().count())
            .unwrap_or(0)
    }

    /// every `DUCKTAPE_RUN_SKILLS` root whose skill mount content-checked
    /// inside the run, one per run (W6 evidence).
    fn skill_roots(&self) -> Vec<String> {
        std::fs::read_to_string(&self.skills_log)
            .map(|s| s.lines().map(str::to_string).collect())
            .unwrap_or_default()
    }
}

/// hermetic env for a node that must provide NOTHING (see dispatch_e2e).
fn hermetic_env(root: &std::path::Path, name: &str) -> Vec<(String, String)> {
    let empty = root.join(name).join("specs");
    std::fs::create_dir_all(&empty).expect("empty spec dir");
    let missing = root.join(name).join("missing-executor");
    vec![
        ("DUCKTAPE_CAPABILITY_DIR".into(), empty.display().to_string()),
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

/// upload the agent's prompt to `idx`'s node-local blob lane and return the
/// 32-byte digest the registry pins — the REAL prompt path (strict, no
/// fallback), satisfied on the node that will execute the run.
fn upload_prompt(cluster: &Cluster, idx: usize) -> Vec<u8> {
    let (status, body) = cluster.http(
        idx,
        "POST",
        "/v1/files/blob",
        Some(&serde_json::json!("You are the portable QA duck.")),
    );
    assert_eq!(status, 200, "blob upload failed: {body}");
    let hex = body["digest"].as_str().expect("digest in blob receipt");
    let digest = duckfs_core::from_hex_32(hex).expect("a 32-byte digest");
    digest.to_vec()
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
    poll_until("the agent reply to post", ROUND_TRIP, || {
        let reply = cluster.query(
            idx,
            "chat",
            &chat::encode_query(&ChatQuery::MessagesLatest {
                channel_id: CHANNEL.into(),
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

#[test]
fn a_portable_run_materializes_commits_and_chains_a_real_duckfs_workspace() {
    let _serial = serial();
    let fixtures = tempfile::TempDir::new().expect("provider fixtures dir");
    let provider = PortableProvider::stage(fixtures.path());
    // the operator-facing root override: one shared base, per-node storage
    // salt keeps the co-located validators' run trees disjoint.
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

    // the strict prompt path: pin a real blob on the executing node.
    let prompt_hash = upload_prompt(&cluster, 1);

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
    poll_until("the skill seed to commit", FINALIZE, || {
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
            prompt_hash,
            allowed_actions: vec![ACTION_CHAT_POST.into()],
            recipe_hash: None,
            caps: None,
            // a TRACKING skill (no pin): the composer resolves it to the
            // committed head, the provisioner mounts it read-only (W6).
            skills: Some(vec![SkillRef {
                name: SKILL_NAME.into(),
                source_prefix: SKILL_PREFIX.into(),
                source_snapshot: None,
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
    poll_until("the channel watch to commit", FINALIZE, || {
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
    // advertised DUCKTAPE_RUN_SKILLS root before answering.
    let roots = provider.skill_roots();
    assert_eq!(roots.len(), 1, "run 1 saw its skill ro mount: {roots:?}");

    // the artifact is COMMITTED duckfs state, readable on a node that never
    // executed anything (the whole point of the portable workspace).
    poll_until("the artifact to reach committed state", FINALIZE, || {
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
    poll_until("run 2's chain evidence", FINALIZE, || {
        (provider.chained() == 1).then_some(())
    });

    // ---- the mounts themselves: two runs, two DISTINCT per-run dirs, every
    // one under the operator root (salted per node) — which lives in the
    // fixtures tempdir, DISJOINT from every node's storage tree (D7; the
    // root's boot validation refuses a storage-resident override) — and both
    // cleaned up after their run (W5).
    let cwds = provider.cwds();
    assert_eq!(cwds.len(), 2, "exactly one provider execution per run: {cwds:?}");
    assert_ne!(cwds[0], cwds[1], "per-attempt dirs never collide");
    for cwd in &cwds {
        let dir = PathBuf::from(cwd);
        assert!(
            dir.starts_with(&runs_root),
            "the mount honors DUCKTAPE_AGENT_RUNS_ROOT: {cwd}"
        );
        poll_until("the run dir to be cleaned up (W5)", FINALIZE, || {
            (!dir.exists()).then_some(())
        });
    }

    // ---- the skill ro mounts: one content-checked root per run, each under
    // the operator root, each a SIBLING of its run's rw mount (never inside
    // it — that is the no-leak mechanism), and each cleaned up (W5).
    let roots = provider.skill_roots();
    assert_eq!(roots.len(), 2, "both runs saw the skill ro mount: {roots:?}");
    assert_ne!(roots[0], roots[1], "per-run skill roots never collide");
    for (root, cwd) in roots.iter().zip(&cwds) {
        let dir = PathBuf::from(root);
        assert!(
            dir.starts_with(&runs_root),
            "the skill root lives under the runs root: {root}"
        );
        assert!(
            !dir.starts_with(cwd),
            "the skill root is a sibling of the rw mount, never inside it: {root} vs {cwd}"
        );
        poll_until("the skill ro root to be cleaned up (W5)", FINALIZE, || {
            (!dir.exists()).then_some(())
        });
    }

    // no correlation debris: every delivered run prunes its pending entry.
    // eventual — the reply was observed on node 2, and node 0 may still be a
    // block behind the delivery that prunes.
    poll_until("delivered runs to prune their pending entries", FINALIZE, || {
        let reply = cluster.query(0, "runs", &runs::encode_query(&RunsQuery::PendingRuns))?;
        match runs::decode_reply(&reply) {
            Ok(RunsReply::PendingRuns(pending)) => pending.is_empty().then_some(()),
            _ => None,
        }
    });
}
