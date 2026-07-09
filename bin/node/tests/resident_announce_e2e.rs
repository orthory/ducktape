//! resident capability announce + dispatch execution, end to end on the
//! network-shape cluster: a fresh identity JOINS the founder's network with a
//! live invite (the product flow — no manual ceremony), lands RESIDENT
//! standing, and — without ever being promoted — (1) announces its
//! script-backed provider into the capability registry through the
//! submit-relay lane, and (2) EXECUTES the dispatch work the saga module
//! rendezvous-assigns to it: a mention of an agent bound to its tag runs on
//! the resident's host and the raw answer comes back as one chat reply,
//! relayed OracleResult and all. the founder is hermetically capability-free,
//! so the only possible executor is the resident — an execution IS the proof
//! that announced residents serve their leases instead of stalling them.
//!
//! run alone (cluster e2es flake under parallel load):
//!   cargo test -p node-bin --test resident_announce_e2e -- --nocapture --test-threads=1

mod common;

use std::path::PathBuf;
use std::time::Duration;

use agent::{ACTION_CHAT_POST, AgentMsg};
use capability::{CapabilityQuery, CapabilityReply};
use chat::{AuthorRef, Block, ChatMsg, ChatQuery, ChatReply, Mark, PostPolicy, Span};
use common::{NetworkShapeCluster, poll_until, serial};
use runs::{RunsMsg, RunsQuery, RunsReply, TurnPolicy};

/// generous like the sibling network-shape legs: standing → follow-arm sync →
/// announce relay → registry commit is several blocks of slack.
const CONVERGE: Duration = Duration::from_secs(180);
/// budget for one submitted op to finalize and become readable elsewhere.
const FINALIZE: Duration = Duration::from_secs(60);
/// budget for a full mention -> resident execution -> delivery -> reply round
/// trip (several blocks, a resident boundary re-sync, one provider spawn).
const ROUND_TRIP: Duration = Duration::from_secs(120);

/// one script-backed provider staged on disk (dispatch_e2e's fixture, trimmed
/// to the text format this leg needs): an operator spec dir holding a single
/// capability spec whose `detect.env` points at an executable script that
/// logs each invocation and answers on stdout.
struct ScriptProvider {
    tag: String,
    spec_dir: PathBuf,
    env_var: String,
    script: PathBuf,
    exec_log: PathBuf,
}

impl ScriptProvider {
    fn stage(root: &std::path::Path, name: &str, tag: &str, stdout: &str) -> Self {
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
                 echo \"ran $(date +%s.%N)\" >> {log}\n\
                 printf '%s\\n' '{stdout}'\n",
                log = exec_log.display(),
            ),
        )
        .expect("write provider script");
        let mut perms = std::fs::metadata(&script)
            .expect("script metadata")
            .permissions();
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
                 description = \"resident announce e2e script executor\"\n\
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
            exec_log,
        }
    }

    /// the env pairs that make a node provide this tag.
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
/// host machine has installed (dispatch_e2e's hermetic knob).
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

/// the detect overrides that hide the embedded executor specs, for the node
/// that DOES carry the script provider dir.
fn hide_builtins(root: &std::path::Path, name: &str) -> Vec<(String, String)> {
    let missing = root.join(name).join("missing-executor");
    vec![
        ("DUCKTAPE_CLAUDE_BIN".into(), missing.display().to_string()),
        ("DUCKTAPE_CODEX_BIN".into(), missing.display().to_string()),
    ]
}

/// the tag's committed provider pool on `idx`, sorted by key.
fn providers(cluster: &NetworkShapeCluster, idx: usize, tag: &str) -> Option<Vec<Vec<u8>>> {
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

#[test]
fn a_joined_resident_announces_and_executes_assigned_dispatch() {
    let _serial = serial();
    let fixtures = tempfile::TempDir::new().expect("provider fixtures dir");
    let provider = ScriptProvider::stage(
        fixtures.path(),
        "friend",
        "quack-resident",
        "the resident word is quack",
    );

    let mut cluster = NetworkShapeCluster::new();
    // the FOUNDER (the only validator) provides nothing; the FRIEND (the
    // joining resident) carries the script provider. any execution of the
    // tag can therefore only have happened on the resident.
    cluster.env[0] = hermetic_env(fixtures.path(), "founder");
    cluster.env[1] = [provider.env(), hide_builtins(fixtures.path(), "friend")].concat();

    let chain_id = cluster.init_founder("resident-announce");
    assert!(!chain_id.is_empty(), "init should print the founded chain id");
    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", Duration::from_secs(60));

    // the PRODUCT join flow, token kept: the parked joiner announces the
    // invite, a member redeems it automatically, resident standing lands.
    let invite = cluster.invite();
    let friend_key_hex = cluster.join_friend(&invite);
    assert_eq!(friend_key_hex.len(), 64, "join prints the friend's pubkey hex");
    let friend_key = common::unhex(&friend_key_hex);
    cluster.spawn(1);
    cluster.wait_marker(1, "joining:", Duration::from_secs(60));
    cluster.wait_marker(1, "resident: standing granted", CONVERGE);
    cluster.wait_marker(1, "resident: pre-synced boundary", CONVERGE);

    // (1) THE ANNOUNCE: without promotion, the resident's discovered tag set
    //     reaches the COMMITTED registry — relayed to the founder, admitted by
    //     the relaxed member gate, applied in consensus.
    cluster.wait_marker(1, "resident: capability announce relayed", CONVERGE);
    poll_until(
        "the resident's announce to land in the founder's registry",
        CONVERGE,
        || {
            let pool = providers(&cluster, 0, &provider.tag)?;
            (pool == vec![friend_key.clone()]).then_some(())
        },
    );
    // and the pump's own settle log confirms the reply lane round-tripped.
    cluster.wait_marker(1, "resident: announced capabilities", CONVERGE);

    // (2) THE DISPATCH: an agent bound to the tag is mentioned on the
    //     founder. rendezvous assignment draws from the announced pool — the
    //     resident, alone — and the resident's state-driven worker pump must
    //     execute the lease and relay the result home.
    cluster.submit(
        0,
        "chat",
        &chat::encode_msg(&ChatMsg::CreateChannel {
            channel_id: "dispatch".into(),
            name: "Dispatch".into(),
            post_policy: PostPolicy::Open,
        }),
    );
    poll_until("the channel to finalize on the founder", FINALIZE, || {
        let raw = cluster.query(
            0,
            "chat",
            &chat::encode_query(&ChatQuery::Channel {
                channel_id: "dispatch".into(),
            }),
        )?;
        matches!(chat::decode_reply(&raw).ok()?, ChatReply::Channel(Some(_))).then_some(())
    });

    // arm the agent the way the app does: the prompt blob uploads FIRST —
    // the run envelope carries its pin and the host REFUSES to run an agent
    // whose registered prompt it cannot resolve (never a silent fallback to
    // the generic instructions) — then RegisterAgent commits the digest.
    // the blob lane takes the raw request body; the harness helper is
    // json-bodied, so the blob is the JSON-encoded string — opaque bytes as
    // far as this test cares (the provider script's answer is fixed).
    //
    // uploaded to BOTH nodes because blob bytes are NODE-LOCAL: the pin
    // replicates through consensus, the bytes do not (yet — the tracked
    // remote-executor replication gap), and the RESIDENT is the executor
    // here. this mirrors what a resident operator's own app does today; a
    // fetch-on-miss lane makes the second upload obsolete.
    let prompt = serde_json::json!("You are quacker, a resident e2e test agent.");
    let mut digests = Vec::new();
    for port in [cluster.http_ports[0], cluster.http_ports[1]] {
        let (blob_code, blob_reply) =
            common::http_request(port, "POST", "/v1/files/blob", Some(&prompt));
        assert_eq!(blob_code, 200, "prompt blob upload failed: {blob_reply}");
        digests.push(common::unhex(
            blob_reply["digest"].as_str().expect("blob reply carries a digest"),
        ));
    }
    assert_eq!(digests[0], digests[1], "content addressing agrees across nodes");
    let prompt_hash = digests.remove(0);
    assert_eq!(prompt_hash.len(), 32, "sha256 digest bytes");
    cluster.submit(
        0,
        "agent",
        &agent::encode_msg(&AgentMsg::RegisterAgent {
            agent_id: "quacker".into(),
            display_name: "quacker".into(),
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
            channel_id: "dispatch".into(),
            policy: TurnPolicy::Mention,
        }),
    );
    // the watch must be committed before the mention posts, or the tagging
    // plane has no subscriber to engage.
    poll_until("the channel watch to commit", FINALIZE, || {
        let reply = cluster.query(0, "runs", &runs::encode_query(&RunsQuery::Watches))?;
        match runs::decode_reply(&reply) {
            Ok(RunsReply::Watches(w)) => {
                w.iter().any(|v| v.channel_id == "dispatch").then_some(())
            }
            _ => None,
        }
    });
    cluster.submit(
        0,
        "chat",
        &chat::encode_msg(&ChatMsg::PostMessage {
            channel_id: "dispatch".into(),
            message_id: "m1".into(),
            blocks: vec![Block::Paragraph(vec![
                Span::plain("hey "),
                Span {
                    text: "@quacker".into(),
                    marks: vec![Mark::Mention(AuthorRef::Agent {
                        module: "runs".into(),
                        agent_id: "quacker".into(),
                    })],
                },
                Span::plain(" say the word"),
            ])],
            thread: None,
            as_agent: None,
        }),
    );

    // the mention was the fresh channel's seq 1.
    let run_id = runs::run_id_for("dispatch", 1, "quacker");
    let reply_text = poll_until("the agent reply to post", ROUND_TRIP, || {
        let reply = cluster.query(
            0,
            "chat",
            &chat::encode_query(&ChatQuery::MessagesLatest {
                channel_id: "dispatch".into(),
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
    });
    assert_eq!(
        reply_text, "the resident word is quack",
        "the reply is the RESIDENT provider's raw answer"
    );
    // the execution evidence: the resident's script ran exactly once — the
    // lease was served on the resident host, not stalled to expiry and not
    // double-run through the re-send path.
    assert_eq!(
        provider.executions(),
        1,
        "the resident executed its assignment exactly once"
    );
    // and the resident's own log shows the relayed result applying.
    cluster.wait_marker(1, "resident: dispatch result for saga", CONVERGE);

    cluster.kill(1);
    cluster.kill(0);
}
