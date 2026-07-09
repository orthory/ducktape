//! live portable-runtime e2e: REAL `ducktape-node` validators, a REAL
//! script-backed provider, and the REAL `NodedProvisioner` driving duckfs
//! checkout/commit — the full ADR loop this repo's unit suites only mock:
//!
//!   mention -> v3 envelope (files module wired) -> lease on the providing
//!   node -> duckfs checkout of the agent's pinned source at a per-run dir
//!   OUTSIDE storage (D7) -> the provider executes INSIDE the mount and
//!   writes a file -> commit mints the output_ref -> RunnerResult delivery
//!   posts the reply -> the committed files state carries the artifact on
//!   EVERY node -> the NEXT run's checkout materializes it (W2 chaining).
//!
//! the agent is registered with a REAL prompt pin whose blob is uploaded to
//! the executing node's blob lane (`POST /v1/files/blob`) — the strict
//! no-fallback prompt path, satisfied rather than sidestepped.

mod common;

use std::path::PathBuf;
use std::time::Duration;

use agent::{ACTION_CHAT_POST, AgentMsg};
use capability::{CapabilityQuery, CapabilityReply};
use chat::{AuthorRef, Block, ChatMsg, ChatQuery, ChatReply, Mark, PostPolicy, Span};
use common::{Cluster, poll_until, serial};
use duckfs_core::{
    FilesQuery, FilesReply, decode_reply as files_decode_reply,
    encode_query as files_encode_query,
};
use runs::{RunsMsg, RunsQuery, RunsReply, TurnPolicy};

const CONVERGE: Duration = Duration::from_secs(180);
const FINALIZE: Duration = Duration::from_secs(60);
const ROUND_TRIP: Duration = Duration::from_secs(120);

const AGENT_ID: &str = "quacker-portable";
const CHANNEL: &str = "portable";
const ARTIFACT: &str = "agent-artifact.txt";
const ARTIFACT_BODY: &str = "portable evidence";

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
}

impl PortableProvider {
    fn stage(root: &std::path::Path) -> Self {
        let dir = root.join("portable-provider");
        let spec_dir = dir.join("specs");
        std::fs::create_dir_all(&spec_dir).expect("provider spec dir");
        let cwd_log = dir.join("cwd.log");
        let chain_log = dir.join("chain.log");
        let script = dir.join("provider.sh");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\n\
                 # a stand-in coding agent: drain the prompt, record the\n\
                 # provisioned cwd, prove the prior run's artifact chained in\n\
                 # (content-checked), write this run's artifact, answer.\n\
                 cat > /dev/null\n\
                 pwd >> {cwd}\n\
                 if [ -f {artifact} ] && [ \"$(cat {artifact})\" = '{body}' ]; then\n\
                 \techo chained >> {chain}\n\
                 fi\n\
                 printf '%s' '{body}' > {artifact}\n\
                 printf '%s\\n' 'portable run done'\n",
                cwd = cwd_log.display(),
                chain = chain_log.display(),
                artifact = ARTIFACT,
                body = ARTIFACT_BODY,
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

/// the committed files-module view of the artifact on `idx` — `Some(size)`
/// when the committed head carries it.
fn artifact_stat(cluster: &Cluster, idx: usize) -> Option<u64> {
    let path = format!("/shared/agent-workspaces/{AGENT_ID}/{ARTIFACT}");
    let reply = cluster.query(
        idx,
        "files",
        &files_encode_query(&FilesQuery::Stat {
            path,
            snapshot: None,
        }),
    )?;
    match files_decode_reply(&reply) {
        Ok(FilesReply::Stat(Some(info))) => Some(info.size),
        _ => None,
    }
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
            skills: None,
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

    // ---- run 1: fresh network, head None -> an EMPTY checkout (the
    // cold-start case), the script writes the artifact, commit mints the
    // output_ref, the reply delivers.
    mention(&cluster, 0, "m1");
    let run_1 = runs::run_id_for(CHANNEL, 1, AGENT_ID);
    assert_eq!(wait_for_reply(&cluster, 0, &run_1), "portable run done");

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
