//! live multi-node dispatch e2e: REAL `ducktape-node` validators over
//! localhost TCP, with REAL script-backed providers wired through the full
//! capability-host path (operator spec dir -> discovery -> announce ->
//! resolve -> spawned CLI), driving the whole agent loop across nodes:
//! mention -> engagement -> dispatch -> saga -> provider execution on the
//! RIGHT node -> oracle result -> next-block delivery -> one chat reply.
//!
//! two scenarios, two clusters:
//!
//! - `mention_routes_to_the_announced_provider_across_nodes`: heterogeneous
//!   providers on two different validators (a `text` executor on node 1, a
//!   `json-result` executor on node 2), each announced into the capability
//!   registry from its own host. a mention of an agent bound to each tag
//!   executes on exactly the node that provides it — however the op entered
//!   consensus — and replies exactly once, one block (or more) after the
//!   result committed (the never-pop-stack rule, observed via the op index).
//!
//! - `unannounced_capable_nodes_race_accept_and_execute_once`: both provider
//!   validators run `announce_capabilities = false` (accept-lane-only), so
//!   the agent's tag has an EMPTY rendezvous pool and the dispatch's
//!   WorkerRequest goes out UNASSIGNED — an announcement. both capable nodes
//!   race `SagaMsg::Accept`; consensus order seats exactly one winner, the
//!   loser's accept finalizes as a deterministic no-op, and the work runs
//!   ONCE — the double-execution safety property, live across processes.

mod common;

use std::path::PathBuf;
use std::time::Duration;

use agent_interface::{ACTION_CHAT_POST, AgentMsg, AgentQuery, AgentReply, TurnPolicy};
use capability_interface::{CapabilityQuery, CapabilityReply};
use chat_interface::{AuthorRef, Block, ChatMsg, ChatQuery, ChatReply, Mark, PostPolicy, Span};
use common::{Cluster, poll_until, serial};
use dispatch_interface::{DispatchQuery, DispatchReply, DispatchStatus};

/// convergence budget: mesh formation + leader rotation are real-time on a
/// possibly-loaded CI core; polls exit early, so generosity is free.
const CONVERGE: Duration = Duration::from_secs(180);
/// budget for one submitted op to finalize and become readable elsewhere.
const FINALIZE: Duration = Duration::from_secs(60);
/// budget for a full mention -> execution -> delivery -> reply round trip
/// (several blocks plus one provider process spawn).
const ROUND_TRIP: Duration = Duration::from_secs(120);

/// one script-backed provider staged on disk for one node: an operator spec
/// dir holding a single capability spec whose `detect.env` points at an
/// executable script. the script appends one line to `exec_log` per
/// invocation (the exactly-once evidence) and answers on stdout in the
/// spec's declared output format — a REAL provider through the full
/// capability-host path, minus the LLM bill.
struct ScriptProvider {
    tag: String,
    spec_dir: PathBuf,
    env_var: String,
    script: PathBuf,
    exec_log: PathBuf,
}

impl ScriptProvider {
    /// stage a provider under `root/<name>`: `format` is the spec's output
    /// format and `stdout` the exact bytes the script prints (compose them to
    /// match). the spec carries a per-provider `detect.env` name, so a node
    /// provides this tag exactly when its process env names this script.
    fn stage(root: &std::path::Path, name: &str, tag: &str, format: &str, stdout: &str) -> Self {
        let dir = root.join(name);
        let spec_dir = dir.join("specs");
        std::fs::create_dir_all(&spec_dir).expect("provider spec dir");
        let exec_log = dir.join("exec.log");
        let script = dir.join("provider.sh");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\n\
                 # a test executor: drain the payload, log the invocation,\n\
                 # answer in the spec's output format.\n\
                 cat > /dev/null\n\
                 echo ran >> {log}\n\
                 printf '%s\\n' '{stdout}'\n",
                log = exec_log.display(),
            ),
        )
        .expect("write provider script");
        let mut perms = std::fs::metadata(&script).expect("script metadata").permissions();
        use std::os::unix::fs::PermissionsExt as _;
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).expect("chmod provider script");

        let env_var = format!(
            "DUCKTAPE_TEST_{}_BIN",
            tag.replace(['-', '.'], "_").to_uppercase()
        );
        std::fs::write(
            spec_dir.join(format!("{tag}.toml")),
            format!(
                "spec = 1\n\
                 [capability]\n\
                 tag = \"{tag}\"\n\
                 description = \"dispatch e2e script executor\"\n\
                 [detect]\n\
                 bin = \"{tag}-nonexistent-cli\"\n\
                 env = \"{env_var}\"\n\
                 [invoke]\n\
                 args = []\n\
                 prompt = \"stdin\"\n\
                 timeout_secs = 30\n\
                 [output]\n\
                 format = \"{format}\"\n"
            ),
        )
        .expect("write provider spec");
        Self {
            tag: tag.into(),
            spec_dir,
            env_var,
            script,
            exec_log,
        }
    }

    /// the env pairs that make node `idx` provide this tag: the operator dir
    /// override plus the spec's detect override. combine multiple providers
    /// on one node by pointing them at the SAME spec dir... this fixture
    /// keeps one dir per provider, so a node carries exactly one.
    fn env(&self) -> Vec<(String, String)> {
        vec![
            (
                "DUCKTAPE_CAPABILITY_DIR".into(),
                self.spec_dir.display().to_string(),
            ),
            (self.env_var.clone(), self.script.display().to_string()),
        ]
    }

    /// how many times the script actually ran on its node.
    fn executions(&self) -> usize {
        std::fs::read_to_string(&self.exec_log)
            .map(|s| s.lines().count())
            .unwrap_or(0)
    }
}

/// env that keeps a node OUT of the provider business regardless of what the
/// host machine has installed: an empty operator dir plus detect overrides
/// pointing the embedded executor specs at nothing (a broken override is a
/// loud warning + absent capability — never a PATH fallback), so a dev box
/// with a real `claude`/`codex` on PATH runs this suite identically to CI.
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

/// the detect overrides that hide the embedded executor specs, for nodes that
/// DO carry a script provider dir.
fn hide_builtins(root: &std::path::Path, name: &str) -> Vec<(String, String)> {
    let missing = root.join(name).join("missing-executor");
    vec![
        ("DUCKTAPE_CLAUDE_BIN".into(), missing.display().to_string()),
        ("DUCKTAPE_CODEX_BIN".into(), missing.display().to_string()),
    ]
}

/// boot the 3-validator cluster and wait for genesis agreement + liveness —
/// the shared preamble of both scenarios.
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

/// the tag's committed provider pool on `idx`, sorted by key.
fn providers(cluster: &Cluster, idx: usize, tag: &str) -> Option<Vec<Vec<u8>>> {
    let reply = cluster.query(
        idx,
        "capability",
        &capability_interface::encode_query(&CapabilityQuery::Providers {
            capability: tag.into(),
        }),
    )?;
    match capability_interface::decode_reply(&reply) {
        Ok(CapabilityReply::Providers(p)) => Some(p),
        _ => None,
    }
}

/// register `agent_id` on `tag`, watch `channel` under Mention, and post the
/// mention that engages it — the whole client-side trigger, submitted through
/// node `idx` (whose key becomes the owner/author). returns the mention's
/// message id.
fn register_and_mention(
    cluster: &Cluster,
    idx: usize,
    channel: &str,
    agent_id: &str,
    tag: &str,
    message_id: &str,
) {
    cluster.submit(
        idx,
        "agent",
        &agent_interface::encode_msg(&AgentMsg::RegisterAgent {
            agent_id: agent_id.into(),
            display_name: agent_id.into(),
            capability: tag.into(),
            prompt_hash: vec![7u8; 32],
            prompt_doc: None,
            allowed_actions: vec![ACTION_CHAT_POST.into()],
        }),
    );
    cluster.submit(
        idx,
        "agent",
        &agent_interface::encode_msg(&AgentMsg::WatchChannel {
            channel_id: channel.into(),
            policy: TurnPolicy::Mention,
        }),
    );
    // the watch must be committed before the mention posts, or the tagging
    // plane has no subscriber to engage.
    poll_until("the channel watch to commit", FINALIZE, || {
        let reply = cluster.query(
            idx,
            "agent",
            &agent_interface::encode_query(&AgentQuery::Watches),
        )?;
        match agent_interface::decode_reply(&reply) {
            Ok(AgentReply::Watches(w)) => {
                w.iter().any(|v| v.channel_id == channel).then_some(())
            }
            _ => None,
        }
    });
    cluster.submit(
        idx,
        "chat",
        &chat_interface::encode_msg(&ChatMsg::PostMessage {
            channel_id: channel.into(),
            message_id: message_id.into(),
            blocks: vec![Block::Paragraph(vec![
                Span::plain("hey "),
                Span {
                    text: format!("@{agent_id}"),
                    marks: vec![Mark::Mention(AuthorRef::Agent {
                        module: "agent".into(),
                        agent_id: agent_id.into(),
                    })],
                },
                Span::plain(" say the word"),
            ])],
            thread: None,
            as_agent: None,
        }),
    );
}

/// poll `channel` on `idx` until the agent's reply to `run_id` exists, and
/// return its plain text.
fn wait_for_reply(cluster: &Cluster, idx: usize, channel: &str, run_id: &str) -> String {
    poll_until("the agent reply to post", ROUND_TRIP, || {
        let reply = cluster.query(
            idx,
            "chat",
            &chat_interface::encode_query(&ChatQuery::MessagesLatest {
                channel_id: channel.into(),
                limit: 64,
            }),
        )?;
        let ChatReply::Messages(views) = chat_interface::decode_reply(&reply).ok()? else {
            return None;
        };
        views.into_iter().find_map(|v| {
            (v.head.message_id == format!("agent/{run_id}")).then(|| {
                assert_eq!(
                    v.head.author,
                    AuthorRef::Agent {
                        module: "agent".into(),
                        agent_id: run_id.rsplit('\u{1f}').next().expect("run id agent").into(),
                    },
                    "the reply must be authored by the agent"
                );
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

/// the run's dispatch record once Delivered — the lifecycle ledger the agent
/// module intentionally does not keep.
fn wait_for_delivered(cluster: &Cluster, idx: usize, run_id: &str) {
    poll_until("the dispatch to reach Delivered", ROUND_TRIP, || {
        let reply = cluster.query(
            idx,
            "dispatch",
            &dispatch_interface::encode_query(&DispatchQuery::Dispatch {
                receiver: "agent".into(),
                dispatch_id: agent::dispatch_id_for(run_id),
            }),
        )?;
        match dispatch_interface::decode_reply(&reply) {
            Ok(DispatchReply::Dispatch(Some(view)))
                if view.status == DispatchStatus::Delivered =>
            {
                Some(())
            }
            _ => None,
        }
    });
}

/// every op row of `module`'s derived op index on `idx`, oldest-first.
fn index_ops(cluster: &Cluster, idx: usize, module: &str) -> Vec<serde_json::Value> {
    let (status, body) = cluster.http(idx, "GET", &format!("/v1/index/{module}/ops?limit=500"), None);
    assert_eq!(status, 200, "index ops for {module} failed: {body}");
    body["ops"].as_array().cloned().unwrap_or_default()
}

/// the height of the first op row whose payload satisfies `pick`.
fn op_height(ops: &[serde_json::Value], pick: impl Fn(&serde_json::Value) -> bool) -> Option<u64> {
    ops.iter()
        .find(|row| pick(&row["payload"]))
        .and_then(|row| row["height"].as_u64())
}

#[test]
fn mention_routes_to_the_announced_provider_across_nodes() {
    let _serial = serial();
    let fixtures = tempfile::TempDir::new().expect("provider fixtures dir");
    // heterogeneous REAL providers: a `text` executor on node 1 and a
    // `json-result` executor on node 2 — two different output-format parse
    // paths, two different hosts, one registry.
    let text_provider = ScriptProvider::stage(
        fixtures.path(),
        "node1",
        "quack-text",
        "text",
        "the word is quack",
    );
    let json_provider = ScriptProvider::stage(
        fixtures.path(),
        "node2",
        "quack-json",
        "json-result",
        r#"{"type":"result","result":"the json word is quack"}"#,
    );

    let mut cluster = Cluster::new(&[0, 1, 2], &[0, 1, 2]);
    cluster.env[0] = hermetic_env(fixtures.path(), "node0");
    cluster.env[1] = [text_provider.env(), hide_builtins(fixtures.path(), "node1")].concat();
    cluster.env[2] = [json_provider.env(), hide_builtins(fixtures.path(), "node2")].concat();
    boot(&mut cluster);

    // both hosts announce their discovered set on boot; the registry maps
    // tag -> exactly the node that provides it.
    poll_until("both providers to announce", FINALIZE, || {
        let text_pool = providers(&cluster, 0, &text_provider.tag)?;
        let json_pool = providers(&cluster, 0, &json_provider.tag)?;
        (text_pool == vec![Cluster::identity(1)] && json_pool == vec![Cluster::identity(2)])
            .then_some(())
    });

    cluster.submit(
        0,
        "chat",
        &chat_interface::encode_msg(&ChatMsg::CreateChannel {
            channel_id: "dispatch".into(),
            name: "Dispatch".into(),
            post_policy: PostPolicy::Open,
        }),
    );

    // beat 1: the text provider's agent. the mention is seq 1 in the fresh
    // channel; the run executes on node 1 (the tag's only announced
    // provider) and replies once.
    register_and_mention(&cluster, 0, "dispatch", "quacker-text", &text_provider.tag, "m1");
    let run_text = agent::run_id_for("dispatch", 1, "quacker-text");
    let reply = wait_for_reply(&cluster, 0, "dispatch", &run_text);
    assert_eq!(reply, "the word is quack", "the text provider's raw answer");
    wait_for_delivered(&cluster, 0, &run_text);

    // beat 2: the json provider's agent, cross-checked from ANOTHER node.
    // the reply above was seq 2, so this mention anchors at seq 3.
    register_and_mention(&cluster, 0, "dispatch", "quacker-json", &json_provider.tag, "m2");
    let run_json = agent::run_id_for("dispatch", 3, "quacker-json");
    let reply = wait_for_reply(&cluster, 2, "dispatch", &run_json);
    assert_eq!(
        reply, "the json word is quack",
        "the json-result provider's extracted answer"
    );
    wait_for_delivered(&cluster, 2, &run_json);

    // exactly one execution per run, each on the node that provides the tag.
    assert_eq!(text_provider.executions(), 1, "text provider ran once");
    assert_eq!(json_provider.executions(), 1, "json provider ran once");

    // the never-pop-stack rule, observed: the agent's reply post (the
    // delivery block's follow-up) landed STRICTLY ABOVE the oracle result
    // that committed the outcome — at least one full block between a result
    // and its consumption.
    let saga_ops = index_ops(&cluster, 0, "saga");
    let chat_ops = index_ops(&cluster, 0, "chat");
    let result_height = op_height(&saga_ops, |p| p.get("OracleResult").is_some())
        .expect("an OracleResult op is indexed");
    let reply_height = op_height(&chat_ops, |p| {
        p.get("PostMessage")
            .and_then(|m| m["message_id"].as_str())
            .is_some_and(|id| id == format!("agent/{run_text}"))
    })
    .expect("the agent reply post is indexed");
    assert!(
        reply_height > result_height,
        "next-block delivery: reply at {reply_height} must sit above the result at {result_height}"
    );

    // no correlation entries left behind: delivery pruned the pending map.
    let reply = cluster
        .query(
            0,
            "agent",
            &agent_interface::encode_query(&AgentQuery::PendingRuns),
        )
        .expect("pending runs query");
    match agent_interface::decode_reply(&reply) {
        Ok(AgentReply::PendingRuns(pending)) => {
            assert!(pending.is_empty(), "delivered runs must prune: {pending:?}")
        }
        other => panic!("unexpected pending-runs reply: {other:?}"),
    }
}

#[test]
fn unannounced_capable_nodes_race_accept_and_execute_once() {
    let _serial = serial();
    let fixtures = tempfile::TempDir::new().expect("provider fixtures dir");
    // the SAME tag on two nodes, both accept-lane-only: the rendezvous pool
    // stays empty, so the dispatch goes out unassigned and both race to
    // claim it.
    let racer_one = ScriptProvider::stage(
        fixtures.path(),
        "node1",
        "quack-race",
        "text",
        "claimed by node one",
    );
    let racer_two = ScriptProvider::stage(
        fixtures.path(),
        "node2",
        "quack-race",
        "text",
        "claimed by node two",
    );

    let mut cluster = Cluster::new(&[0, 1, 2], &[0, 1, 2]);
    cluster.extra_toml.push("announce_capabilities = false".into());
    cluster.env[0] = hermetic_env(fixtures.path(), "node0");
    cluster.env[1] = [racer_one.env(), hide_builtins(fixtures.path(), "node1")].concat();
    cluster.env[2] = [racer_two.env(), hide_builtins(fixtures.path(), "node2")].concat();
    boot(&mut cluster);

    // the knob holds: capable hosts, empty pool.
    let pool = poll_until("the capability registry to answer", FINALIZE, || {
        providers(&cluster, 0, "quack-race")
    });
    assert!(
        pool.is_empty(),
        "suppressed providers must never enter the rendezvous pool: {pool:?}"
    );

    cluster.submit(
        0,
        "chat",
        &chat_interface::encode_msg(&ChatMsg::CreateChannel {
            channel_id: "race".into(),
            name: "Race".into(),
            post_policy: PostPolicy::Open,
        }),
    );
    register_and_mention(&cluster, 0, "race", "racer", "quack-race", "m1");
    let run_id = agent::run_id_for("race", 1, "racer");

    // one winner executes, one reply posts — whichever node won the claim.
    let reply = wait_for_reply(&cluster, 0, "race", &run_id);
    assert!(
        reply == "claimed by node one" || reply == "claimed by node two",
        "the reply must be one racer's answer: {reply:?}"
    );
    wait_for_delivered(&cluster, 0, &run_id);

    // exactly ONE execution across both capable hosts — the whole point of
    // accept-to-claim: N capable nodes, one paid run.
    assert_eq!(
        racer_one.executions() + racer_two.executions(),
        1,
        "the claim race must collapse to a single execution \
         (node1: {}, node2: {})",
        racer_one.executions(),
        racer_two.executions(),
    );
    // and the reply came from the node that actually ran.
    let winner_ran_and_replied = (racer_one.executions() == 1
        && reply == "claimed by node one")
        || (racer_two.executions() == 1 && reply == "claimed by node two");
    assert!(winner_ran_and_replied, "the winner's answer is the reply");

    // BOTH nodes raced: two Accept ops finalized, from two distinct external
    // origins — the loser's landing as a deterministic no-op. the op index
    // is the proof surface (rows carry the verified frame origin).
    poll_until("both accepts to finalize", FINALIZE, || {
        let accepts: Vec<String> = index_ops(&cluster, 0, "saga")
            .iter()
            .filter(|row| row["payload"].get("Accept").is_some())
            .filter_map(|row| row["origin"]["id"].as_str().map(str::to_string))
            .collect();
        (accepts.len() == 2).then(|| {
            let expected: std::collections::BTreeSet<String> = [1u64, 2]
                .iter()
                .map(|s| String::from_utf8_lossy(&Cluster::identity(*s)).into_owned())
                .collect();
            let actual: std::collections::BTreeSet<String> = accepts.into_iter().collect();
            assert_eq!(
                actual, expected,
                "the two accepts must come from the two capable validators"
            );
        })
    });
}
