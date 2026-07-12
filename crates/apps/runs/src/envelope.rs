//! run envelope composition — the structured dispatch payload.
//!
//! the dispatch plane's rule stands: the DISPATCHER composes the entire model
//! input, in consensus, and it rides the dispatch as committed payload data
//! (P4). the payload is a JSON envelope carrying the agent's real prompt pin
//! (`prompt_hash`), a thread-continuity key, the generic fallback
//! instructions, the strict output contract, the rendered conversation, the
//! runtime section (the tool plane and the run's skill mounts), and the
//! portable workspace plan ([`PortableInputs`]). the host-side worker
//! routes on the `ducktape_run` marker and assembles the final model input,
//! resolving `prompt_hash` from the content-addressed blob store — so the
//! exact prompt bytes stay verifiable against the committed hash. every run
//! composes the portable v3 envelope; the flat-string and v2 tolerances are
//! gone (flag day — in-flight legacy runs are unsupported).

use agent::{AgentRecord, PROMPT_HASH_LEN};
use chat::{AuthorRef, Block, MessageView};
use serde::Serialize;

use crate::facets::WireSink;
use crate::hex;

/// the envelope marker the host worker routes on: the portable envelope shape
/// (workspace source pin + skill refs + result contract). bumping it is a
/// payload flag day for the worker, not for consensus state.
pub(crate) const RUN_ENVELOPE_VERSION: u32 = 3;

/// the result-wrapper version a portable (`v3`) provider returns. carried in
/// the envelope's `result_contract` so the worker refuses a runner-result
/// version it cannot unwrap; the `runs` delivery path reads it back as `1`.
pub(crate) const RUNNER_RESULT_VERSION: u32 = 1;

/// generic instructions for an agent without a consensus-resident prompt —
/// the host uses `instructions` only when `prompt_hash` is null.
pub(crate) const DEFAULT_PROMPT: &str =
    "You are a Ducktape agent. Reply helpfully and return only the requested JSON output.";

/// the strict output contract riding every composed payload — exactly the
/// [`agent::AgentResponse`] wire shape.
pub(crate) const STRICT_OUTPUT_INSTRUCTION: &str = r#"Return ONLY a JSON object with this shape:
{"reply_blocks":[{"id":"<uuid>","kind":"paragraph","text":"..."}],"actions":[]}
Allowed reply block kinds are paragraph, heading, and code. heading is rendered as a paragraph in Ducktape chat. code may include an optional "lang". Actions are optional and must use only actions allowed by the agent registry. Do not include markdown fences around the JSON."#;

/// the tool-plane sentence every v3 run is told. deliberately SHORT: the
/// `ducktape` MCP server carries the full tool guide in its own `initialize`
/// instructions (versioned with the binary), so restating the surface here
/// would only give the two something to drift apart about. this section points
/// at the plane; the plane describes itself.
pub(crate) const TOOL_PLANE_INSTRUCTION: &str = "A Ducktape MCP tool server named \"ducktape\" is available in this session. It is how you read and write Ducktape state — chat, tasks, pages, forge items, and duckfs files. Call its tools instead of guessing; its own instructions describe every tool it offers.";

/// the deterministic runtime section: what this run's session carries. composed
/// in consensus from the envelope's OWN fields — the skill names it lists are
/// exactly the ones the host materializes from `skills` — so it stays a pure
/// function of committed state, with no host input, wall clock, or ordering of
/// its own (skills keep envelope order).
///
/// the skills sentence exists to close a real gap: the host checks the skill
/// trees out read-only and exports `DUCKTAPE_RUN_SKILLS`, and until now nothing
/// ever TOLD the model the directory was there. a run that mounts no skills
/// gains no skills sentence — an empty mount list is not worth a line of the
/// model's attention — but every run gets the tool-plane sentence.
fn runtime_section(skills: &[SkillEnvelope]) -> String {
    if skills.is_empty() {
        return TOOL_PLANE_INSTRUCTION.to_string();
    }
    let names = skills
        .iter()
        .map(|s| s.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{TOOL_PLANE_INSTRUCTION}\nYour skills are mounted read-only under the directory named by the DUCKTAPE_RUN_SKILLS environment variable, one directory per skill: {names}."
    )
}

/// the committed payload shape. FIELD ORDER IS PART OF THE COMMITTED BYTES:
/// serde_json serializes struct fields in declaration order, so this
/// declaration — not any map — is the canonical layout every validator
/// reproduces byte-for-byte.
#[derive(Serialize)]
struct RunEnvelope<'a> {
    ducktape_run: u32,
    agent_id: &'a str,
    /// the run's CONSENSUS id — the key this module resolves a run by
    /// (`dispatch_id_for(run_id)` is the pending map's key, and the agent
    /// session lane binds on it). it rides the envelope because it is NOT
    /// derivable host-side: the provisioner's own `WorkspaceSpec.run_id` is
    /// `{saga_id}:{attempt}`, a host-local key for the on-disk workspace dir,
    /// and hashing THAT resolves no run at all — every session bind and every
    /// mid-run action named a run that never existed, and the whole write plane
    /// degraded silently. the composer knows the id; the host must be TOLD it.
    run_id: &'a str,
    /// lowercase hex of the agent's [`PROMPT_HASH_LEN`]-byte prompt pin, or
    /// null when the record carries none — the host resolves the prompt
    /// content by this digest and falls back to `instructions` on null.
    prompt_hash: Option<String>,
    /// `<channel_id>#<seq>` — seq is the anchor's thread root when the
    /// anchor is a thread reply (every run in one thread shares a key), else
    /// the anchor itself. null for job runs: there is no channel.
    thread_key: Option<String>,
    instructions: &'a str,
    contract: &'a str,
    conversation: String,
    /// the deterministic forge item-context instructions section (M1),
    /// rendered by `inject` from committed tracker state. the oracle's
    /// `prepare()` assembles the provider input as instructions → runtime →
    /// context → contract → conversation, so this reaches the model as its own
    /// section; a context-less envelope assembles byte-identically to the
    /// context-less worker. `None` (an ABSENT key) for every non-forge run.
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<String>,
    /// the deterministic runtime section: the ducktape tool plane, plus the
    /// skill mounts this envelope's own `skills` list asks the host for. every
    /// v3 run carries it — the tool plane is unconditional — so unlike
    /// `context` it is never an absent key. it rides AFTER `context` in the
    /// bytes but BEFORE it in the assembled input: field order is the committed
    /// layout, reading order is the worker's, and the worker decodes by name.
    runtime: String,
    workspace: WorkspaceSource,
    skills: Vec<SkillEnvelope>,
    result_contract: ResultContractEnvelope,
}

/// the portable workspace source — WHERE the run's rw workspace is checked out
/// from, tagged by kind. carries NO `mount_path` (D7): the envelope states
/// committed source coordinates only, and the host wrapper picks the per-run
/// writable cwd — never a consensus-supplied host path.
#[derive(Serialize, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum WorkspaceSource {
    /// a duckfs subtree checkout — exactly the flat era's two fields.
    Duckfs {
        source_prefix: String,
        /// the W2 consensus pin of the duckfs head. ALWAYS emitted (null when
        /// the files head is unresolved) so the envelope states its pin
        /// decision.
        source_snapshot: Option<String>,
    },
    /// a forge repo checkout (M1) — the git-native workspace lane.
    Forge {
        repo: String,
        /// the pinned base: a 40-hex sha1 tip resolved from COMMITTED forge
        /// refs at compose height (I1) — the work-branch tip when born, else
        /// the main tip an issue run forks from.
        commit: String,
        /// the work branch — per ITEM, not per run (`agent/item-<n>`, or the
        /// PR's own source branch: session identity).
        branch: String,
        /// advisory compose-time metadata: whether `branch` existed in
        /// committed forge refs at compose height. the provisioner derives
        /// its push CAS base from the FETCHED remote advertisement (a fetch
        /// miss ⇒ zero-oid create), not this flag — kept as a pinned wire
        /// surface and an audit/M2 signal.
        branch_born: bool,
    },
}

/// the runner-result contract the worker must honor for a portable run.
#[derive(Serialize)]
struct ResultContractEnvelope {
    ducktape_runner_result: u32,
    /// the REQUESTED output sink, composed from the trigger context (a forge
    /// item channel requests `Pr`). `Chain` is an ABSENT key — mirroring the
    /// oracle's own is_chain skip.
    #[serde(skip_serializing_if = "WireSink::is_chain")]
    sink: WireSink,
}

/// a C4 skill ref: a duckfs read-only source subtree the host mounts for the
/// run, mirroring the phase-4 [`agent::SkillRef`] (`name` + `source_prefix` +
/// optional `source_snapshot`). a tracking skill's snapshot is resolved to the
/// committed head at compose time (see `RunsModule::portable_inputs`).
#[derive(Serialize, Debug)]
pub(crate) struct SkillEnvelope {
    pub name: String,
    pub source_prefix: String,
    pub source_snapshot: Option<String>,
}

/// the portable half of every envelope, resolved by the composer's callsite
/// from COMMITTED state.
#[derive(Debug)]
pub(crate) struct PortableInputs {
    /// the tagged workspace source (duckfs or forge).
    pub workspace: WorkspaceSource,
    pub skills: Vec<SkillEnvelope>,
    /// the requested output sink; [`WireSink::Chain`] (the default) composes
    /// as an absent key.
    pub sink: WireSink,
    /// the deterministic item-context section (forge runs only).
    pub context: Option<String>,
}

/// the duckfs subtree a portable run's rw workspace is checked out from.
fn workspace_source_prefix(agent: &AgentRecord) -> String {
    format!("/shared/agent-workspaces/{}", agent.agent_id)
}

/// the duckfs workspace source for `agent`, pinned at `source_snapshot` — the
/// non-forge composer lane's workspace.
pub(crate) fn duckfs_workspace(
    agent: &AgentRecord,
    source_snapshot: Option<String>,
) -> WorkspaceSource {
    WorkspaceSource::Duckfs {
        source_prefix: workspace_source_prefix(agent),
        source_snapshot,
    }
}

/// resolve an agent's C4 skill refs against the committed duckfs head: a
/// pinned skill passes its snapshot through; a tracking skill (no pin)
/// resolves to the SAME committed head (W2) — deterministic across
/// validators. shared by the duckfs and forge compose lanes (skills are
/// duckfs subtrees either way).
pub(crate) fn resolve_skills(agent: &AgentRecord, head: &Option<String>) -> Vec<SkillEnvelope> {
    agent
        .skills
        .iter()
        .map(|s| SkillEnvelope {
            name: s.name.clone(),
            source_prefix: s.source_prefix.clone(),
            source_snapshot: s.source_snapshot.clone().or_else(|| head.clone()),
        })
        .collect()
}

/// serialize one envelope — deterministic: fixed field order (see
/// [`RunEnvelope`]) and serde_json's canonical string escaping.
///
/// `run_id` is passed in, never re-derived here: the callsite already minted it
/// to key the dispatch, and one id minted in two places is exactly the drift
/// this field exists to close.
fn envelope(
    agent: &AgentRecord,
    run_id: &str,
    thread_key: Option<String>,
    conversation: String,
    portable: PortableInputs,
) -> String {
    serde_json::to_string(&RunEnvelope {
        ducktape_run: RUN_ENVELOPE_VERSION,
        agent_id: &agent.agent_id,
        run_id,
        prompt_hash: prompt_hash_hex(agent),
        thread_key,
        instructions: DEFAULT_PROMPT,
        contract: STRICT_OUTPUT_INSTRUCTION,
        conversation,
        context: portable.context,
        runtime: runtime_section(&portable.skills),
        workspace: portable.workspace,
        skills: portable.skills,
        result_contract: ResultContractEnvelope {
            ducktape_runner_result: RUNNER_RESULT_VERSION,
            sink: portable.sink,
        },
    })
    .expect("envelope is serializable")
}

/// the agent's prompt pin as the lowercase hex the host resolves blobs by.
/// the registry validates pins to exactly [`PROMPT_HASH_LEN`] bytes, so a
/// record holding anything else has no resolvable prompt and composes as
/// null — the generic instructions apply.
fn prompt_hash_hex(agent: &AgentRecord) -> Option<String> {
    (agent.prompt_hash.len() == PROMPT_HASH_LEN).then(|| hex(&agent.prompt_hash))
}

/// compose a chat run's payload: the envelope around the rendered transcript
/// window ending at the anchor.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_payload(
    module_id: &str,
    agent: &AgentRecord,
    run_id: &str,
    channel_id: &str,
    anchor_seq: u64,
    thread_root: Option<u64>,
    transcript: &[MessageView],
    portable: PortableInputs,
) -> String {
    let thread_key = format!("{channel_id}#{}", thread_root.unwrap_or(anchor_seq));
    envelope(
        agent,
        run_id,
        Some(thread_key),
        render_conversation(module_id, &agent.agent_id, transcript),
        portable,
    )
}

/// compose a job run's payload: same envelope, no thread key, and the
/// conversation is the job's coordinates plus its FULL submitted spec.
pub(crate) fn render_job_payload(
    agent: &AgentRecord,
    run_id: &str,
    job_id: &str,
    spec: &str,
    portable: PortableInputs,
) -> String {
    envelope(
        agent,
        run_id,
        None,
        format!(
            "Job {job_id} — chat replies are not delivered for job runs; respond with actions only.\n\nJob spec:\n{spec}"
        ),
        portable,
    )
}

/// Compose a Pages-comment run. The whole referenced page is supplied as
/// context, while the conversation keeps the triggering comment and stable
/// thread/ordinal coordinates explicit.
pub(crate) fn render_page_comment_payload(
    agent: &AgentRecord,
    run_id: &str,
    thread_id: &str,
    ordinal: u64,
    author: &str,
    text: &str,
    portable: PortableInputs,
) -> String {
    envelope(
        agent,
        run_id,
        Some(format!("pages:{thread_id}")),
        format!(
            "Pages comment thread {thread_id}, comment {ordinal}. Reply to this comment thread.\n\n{author}: {text}"
        ),
        portable,
    )
}

/// the transcript block, rendered exactly as the flat-payload era did — the
/// host feeds it to the model verbatim, so the wording is part of the
/// committed prompt input.
fn render_conversation(module_id: &str, agent_id: &str, transcript: &[MessageView]) -> String {
    if transcript.is_empty() {
        return "No transcript was embedded for this run. Answer the user helpfully.".into();
    }
    let mut out = String::from("Conversation so far:\n");
    for message in transcript {
        let speaker = match &message.head.author {
            AuthorRef::Agent {
                module,
                agent_id: author,
            } if module == module_id && author == agent_id => "you",
            _ => "them",
        };
        out.push_str(&format!("[{speaker}] {}\n", render_message(message)));
    }
    out.push_str("\nReply as the agent.");
    out
}

fn render_message(message: &MessageView) -> String {
    format!(
        "{} @{}: {}",
        render_author(&message.head.author),
        message.seq,
        message
            .head
            .blocks
            .iter()
            .map(render_block)
            .collect::<Vec<_>>()
            .join("\n")
    )
}

fn render_author(author: &AuthorRef) -> String {
    match author {
        AuthorRef::User(bytes) => format!("user:{}", hex(bytes)),
        AuthorRef::Agent { module, agent_id } => format!("agent:{module}/{agent_id}"),
        AuthorRef::Module(module) => format!("module:{module}"),
        AuthorRef::System => "system".into(),
    }
}

fn render_block(block: &Block) -> String {
    match block {
        Block::Paragraph(spans) => spans.iter().map(|s| s.text.as_str()).collect(),
        Block::Code { lang, text } => match lang {
            Some(lang) if !lang.is_empty() => format!("```{lang}\n{text}\n```"),
            _ => format!("```\n{text}\n```"),
        },
        Block::Quote(spans) => {
            let text: String = spans.iter().map(|s| s.text.as_str()).collect();
            text.lines()
                .map(|line| format!("> {line}"))
                .collect::<Vec<_>>()
                .join("\n")
        }
        Block::Divider => "---".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent::AgentStatus;
    use chat::MessageHead;
    use saga::SagaOrigin;
    use serde_json::Value;

    fn agent_with_hash(prompt_hash: Vec<u8>) -> AgentRecord {
        AgentRecord {
            agent_id: "bot".into(),
            owner: SagaOrigin::External(vec![9; 32]),
            display_name: "BOT".into(),
            capability: "model-1".into(),
            prompt_hash,
            allowed_actions: vec![],
            status: AgentStatus::Active,
            created_at: 0,
            updated_at: 0,
            recipe_hash: Vec::new(),
            caps: agent::ResourceCaps::default(),
            skills: Vec::new(),
        }
    }

    fn message(seq: u64, author: AuthorRef, text: &str) -> MessageView {
        MessageView {
            channel_id: "general".into(),
            seq,
            head: MessageHead {
                message_id: format!("m{seq}"),
                author,
                blocks: vec![Block::paragraph(text)],
                created_at: 0,
                rev: 0,
                edited_at: None,
                base_rev: None,
                deleted: false,
                thread: None,
                reply_count: 0,
                last_reply_seq: None,
            },
            reactions: Vec::new(),
            channel_head_seq: seq,
        }
    }

    fn parse(payload: &str) -> Value {
        serde_json::from_str(payload).expect("the payload is a JSON envelope")
    }

    /// the run ids the composer's REAL callsites mint — never a hand-written
    /// string: the envelope's whole job is to carry the id the pending map is
    /// keyed by, so a test that invents one would prove nothing.
    fn run_id(channel: &str, anchor_seq: u64) -> String {
        crate::run_id_for(channel, anchor_seq, "bot")
    }

    fn job_id(job: &str) -> String {
        crate::job_run_id_for(job, "bot", 3)
    }

    fn portable(snapshot: Option<&str>, skills: Vec<SkillEnvelope>) -> PortableInputs {
        PortableInputs {
            workspace: duckfs_workspace(
                &agent_with_hash(vec![7u8; PROMPT_HASH_LEN]),
                snapshot.map(str::to_string),
            ),
            skills,
            sink: WireSink::Chain,
            context: None,
        }
    }

    /// the plain duckfs inputs most tests compose under.
    fn plain() -> PortableInputs {
        portable(None, Vec::new())
    }

    fn skill(name: &str) -> SkillEnvelope {
        SkillEnvelope {
            name: name.into(),
            source_prefix: format!("/shared/skills/{name}"),
            source_snapshot: Some("bb".repeat(32)),
        }
    }

    fn runtime_of(payload: &str) -> String {
        parse(payload)["runtime"]
            .as_str()
            .expect("every v3 envelope carries a runtime section")
            .to_string()
    }

    #[test]
    fn a_chat_envelope_carries_the_prompt_pin_and_anchor_thread_key() {
        let agent = agent_with_hash(vec![7u8; PROMPT_HASH_LEN]);
        let transcript = vec![message(1, AuthorRef::User(vec![1; 32]), "hi bot")];
        let payload = render_payload(
            "runs",
            &agent,
            &run_id("general", 1),
            "general",
            1,
            None,
            &transcript,
            plain(),
        );
        let v = parse(&payload);

        assert_eq!(v["ducktape_run"], RUN_ENVELOPE_VERSION);
        assert_eq!(v["agent_id"], "bot");
        assert_eq!(v["prompt_hash"], "07".repeat(PROMPT_HASH_LEN));
        assert_eq!(
            v["thread_key"], "general#1",
            "a non-thread anchor keys itself"
        );
        assert_eq!(v["instructions"], DEFAULT_PROMPT);
        assert_eq!(v["contract"], STRICT_OUTPUT_INSTRUCTION);
        let conversation = v["conversation"].as_str().unwrap();
        assert!(conversation.starts_with("Conversation so far:\n"));
        assert!(conversation.contains("hi bot"));
        assert!(conversation.ends_with("\nReply as the agent."));

        // field order is part of the committed bytes — assert the layout, not
        // just the values.
        assert!(payload.starts_with(r#"{"ducktape_run":3,"agent_id":"bot","run_id":"#));
    }

    #[test]
    fn every_envelope_names_the_run_the_pending_map_is_keyed_by() {
        // the host cannot derive this id (its own run key is
        // `{saga_id}:{attempt}`), so an envelope that omits it leaves the
        // executing node unable to name its own run back to this module — the
        // session bind and every mid-run action then address a run that does
        // not exist. all three lanes must carry it.
        let agent = agent_with_hash(vec![7u8; PROMPT_HASH_LEN]);

        let chat = run_id("general", 1);
        let v = parse(&render_payload(
            "runs",
            &agent,
            &chat,
            "general",
            1,
            None,
            &[],
            plain(),
        ));
        assert_eq!(v["run_id"], chat);
        assert_eq!(
            chat,
            crate::run_id_for("general", 1, "bot"),
            "the composed id IS the turn-claim key the dispatch is registered under"
        );

        let job = job_id("job-1");
        let v = parse(&render_job_payload(&agent, &job, "job-1", "spec", plain()));
        assert_eq!(
            v["run_id"], job,
            "job runs have no channel, but do have a run"
        );

        let page = crate::page_run_id_for("thread-1", 1, "bot");
        let v = parse(&render_page_comment_payload(
            &agent,
            &page,
            "thread-1",
            1,
            "user:aa",
            "hi",
            plain(),
        ));
        assert_eq!(v["run_id"], page);
    }

    #[test]
    fn an_agent_without_a_prompt_pin_composes_null() {
        let agent = agent_with_hash(Vec::new());
        let payload = render_payload(
            "runs",
            &agent,
            &run_id("general", 1),
            "general",
            1,
            None,
            &[],
            plain(),
        );
        let v = parse(&payload);
        assert!(v["prompt_hash"].is_null());
        assert_eq!(
            v["conversation"],
            "No transcript was embedded for this run. Answer the user helpfully.",
            "the empty-transcript wording is preserved"
        );
    }

    #[test]
    fn a_threaded_anchor_keys_by_its_thread_root() {
        let agent = agent_with_hash(vec![7u8; PROMPT_HASH_LEN]);
        let transcript = vec![message(3, AuthorRef::User(vec![1; 32]), "in thread")];
        let payload = render_payload(
            "runs",
            &agent,
            &run_id("general", 3),
            "general",
            3,
            Some(1),
            &transcript,
            plain(),
        );
        assert_eq!(
            parse(&payload)["thread_key"],
            "general#1",
            "every run in one thread shares a continuity key"
        );
    }

    #[test]
    fn a_job_envelope_has_no_thread_key_and_preserves_the_job_framing() {
        let agent = agent_with_hash(vec![7u8; PROMPT_HASH_LEN]);
        let payload = render_job_payload(
            &agent,
            &job_id("job-1"),
            "job-1",
            "summarize this work item",
            plain(),
        );
        let v = parse(&payload);
        assert!(v["thread_key"].is_null(), "job runs have no channel");
        assert_eq!(v["agent_id"], "bot");
        assert_eq!(v["prompt_hash"], "07".repeat(PROMPT_HASH_LEN));
        assert_eq!(
            v["conversation"],
            "Job job-1 — chat replies are not delivered for job runs; respond with actions only.\n\nJob spec:\nsummarize this work item"
        );
    }

    #[test]
    fn envelope_bytes_are_deterministic() {
        let agent = agent_with_hash(vec![7u8; PROMPT_HASH_LEN]);
        let transcript = vec![
            message(1, AuthorRef::User(vec![1; 32]), "hello \"quoted\"\nline"),
            message(
                2,
                AuthorRef::Agent {
                    module: "runs".into(),
                    agent_id: "bot".into(),
                },
                "earlier reply",
            ),
        ];
        let a = render_payload(
            "runs",
            &agent,
            &run_id("general", 2),
            "general",
            2,
            None,
            &transcript,
            plain(),
        );
        let b = render_payload(
            "runs",
            &agent,
            &run_id("general", 2),
            "general",
            2,
            None,
            &transcript,
            plain(),
        );
        assert_eq!(
            a.as_bytes(),
            b.as_bytes(),
            "composition is byte-deterministic"
        );
        let j1 = render_job_payload(&agent, &job_id("job-1"), "job-1", "spec", plain());
        let j2 = render_job_payload(&agent, &job_id("job-1"), "job-1", "spec", plain());
        assert_eq!(j1.as_bytes(), j2.as_bytes());

        // the runtime section is composed, not passed in — so it must be
        // byte-stable across composes of the same skill list too.
        let skilled = || portable(None, vec![skill("release"), skill("triage")]);
        let s1 = render_payload(
            "runs",
            &agent,
            &run_id("general", 2),
            "general",
            2,
            None,
            &transcript,
            skilled(),
        );
        let s2 = render_payload(
            "runs",
            &agent,
            &run_id("general", 2),
            "general",
            2,
            None,
            &transcript,
            skilled(),
        );
        assert_eq!(
            s1.as_bytes(),
            s2.as_bytes(),
            "the runtime section is a pure function of the envelope's own fields"
        );
    }

    #[test]
    fn the_agents_own_messages_render_as_you() {
        let agent = agent_with_hash(vec![7u8; PROMPT_HASH_LEN]);
        let transcript = vec![
            message(1, AuthorRef::User(vec![1; 32]), "question"),
            message(
                2,
                AuthorRef::Agent {
                    module: "runs".into(),
                    agent_id: "bot".into(),
                },
                "my own reply",
            ),
            message(
                3,
                AuthorRef::Agent {
                    module: "runs".into(),
                    agent_id: "other".into(),
                },
                "someone else",
            ),
        ];
        let payload = render_payload(
            "runs",
            &agent,
            &run_id("general", 3),
            "general",
            3,
            None,
            &transcript,
            plain(),
        );
        let conversation = parse(&payload)["conversation"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(conversation.contains("[them] user:"));
        assert!(conversation.contains("[you] agent:runs/bot @2: my own reply"));
        assert!(conversation.contains("[them] agent:runs/other @3: someone else"));
    }

    // ---- the portable plan ---------------------------------------------------

    #[test]
    fn the_envelope_carries_source_coords_and_skills_but_no_mount_path() {
        let agent = agent_with_hash(vec![7u8; PROMPT_HASH_LEN]);
        let transcript = vec![message(1, AuthorRef::User(vec![1; 32]), "hi bot")];
        let skills = vec![SkillEnvelope {
            name: "release".into(),
            source_prefix: "/shared/skills/release".into(),
            source_snapshot: Some("bb".repeat(32)),
        }];
        let payload = render_payload(
            "runs",
            &agent,
            &run_id("general", 1),
            "general",
            1,
            None,
            &transcript,
            portable(Some(&"aa".repeat(32)), skills),
        );

        // field order is stable — the marker leads.
        assert!(
            payload.starts_with(r#"{"ducktape_run":3,"agent_id":"bot","run_id":"#),
            "the marker leads with a stable field order: {payload}"
        );
        let v = parse(&payload);
        assert_eq!(v["ducktape_run"], 3);
        assert_eq!(
            v["workspace"]["kind"], "duckfs",
            "the workspace source is tagged"
        );
        assert_eq!(
            v["workspace"]["source_prefix"],
            "/shared/agent-workspaces/bot"
        );
        assert_eq!(v["workspace"]["source_snapshot"], "aa".repeat(32));
        // D7: the envelope carries SOURCE coords only — never a host mount path.
        assert!(
            v["workspace"].get("mount_path").is_none(),
            "the workspace must NOT carry a mount_path (D7): {}",
            v["workspace"]
        );
        assert!(
            v.get("context").is_none(),
            "a duckfs run composes no context key"
        );
        assert_eq!(v["result_contract"]["ducktape_runner_result"], 1);
        assert!(
            v["result_contract"].get("sink").is_none(),
            "a chain sink is an ABSENT key, mirroring the oracle's is_chain skip"
        );
        let skills = v["skills"].as_array().unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0]["name"], "release");
        assert_eq!(skills[0]["source_prefix"], "/shared/skills/release");
        assert_eq!(skills[0]["source_snapshot"], "bb".repeat(32));
    }

    #[test]
    fn no_head_states_a_null_pin_not_an_absent_key() {
        // an unresolved head is an EXPLICIT null pin decision, not a missing key.
        let agent = agent_with_hash(vec![7u8; PROMPT_HASH_LEN]);
        let payload = render_payload(
            "runs",
            &agent,
            &run_id("general", 1),
            "general",
            1,
            None,
            &[],
            plain(),
        );
        let v = parse(&payload);
        assert_eq!(v["ducktape_run"], 3);
        assert!(
            v["workspace"]["source_snapshot"].is_null(),
            "an unresolved head composes source_snapshot: null"
        );
        assert!(
            v["workspace"]
                .as_object()
                .unwrap()
                .contains_key("source_snapshot"),
            "the pin decision is stated as null, not omitted"
        );
        assert_eq!(v["skills"].as_array().unwrap().len(), 0, "no skills is []");
    }

    // ---- the runtime section (tool plane + skill mounts) -----------------------

    #[test]
    fn the_runtime_section_names_every_mounted_skill_in_envelope_order() {
        // the host mounts the skills ro under DUCKTAPE_RUN_SKILLS, one dir per
        // name — this section is the ONLY thing that tells the model they exist.
        let agent = agent_with_hash(vec![7u8; PROMPT_HASH_LEN]);
        let skills = vec![skill("release"), skill("triage"), skill("qa")];
        let payload = render_payload(
            "runs",
            &agent,
            &run_id("general", 1),
            "general",
            1,
            None,
            &[],
            portable(None, skills),
        );
        let runtime = runtime_of(&payload);

        assert!(
            runtime.starts_with(TOOL_PLANE_INSTRUCTION),
            "the tool plane leads: {runtime}"
        );
        assert!(
            runtime.contains("DUCKTAPE_RUN_SKILLS"),
            "the section names the env var the host actually exports: {runtime}"
        );
        // names ride VERBATIM, in envelope order — the mount subpaths are these
        // names, so a reordered or renamed list would point the model at
        // directories that are not there.
        assert!(
            runtime.ends_with("one directory per skill: release, triage, qa."),
            "every mounted skill, in envelope order: {runtime}"
        );
    }

    #[test]
    fn a_skill_less_run_composes_the_tool_plane_but_no_skills_sentence() {
        let agent = agent_with_hash(vec![7u8; PROMPT_HASH_LEN]);
        let payload = render_payload(
            "runs",
            &agent,
            &run_id("general", 1),
            "general",
            1,
            None,
            &[],
            plain(),
        );
        let runtime = runtime_of(&payload);
        assert_eq!(
            runtime, TOOL_PLANE_INSTRUCTION,
            "no mounts ⇒ no mounts sentence; the tool plane is unconditional"
        );
        assert!(!runtime.contains("DUCKTAPE_RUN_SKILLS"));
    }

    #[test]
    fn the_runtime_key_rides_between_context_and_workspace() {
        // FIELD ORDER IS THE COMMITTED BYTES: runtime sits after `context` and
        // before `workspace`. (its READING order in the assembled model input
        // is the worker's business — dispatch-oracle decodes by name and puts
        // runtime BEFORE context.)
        let agent = agent_with_hash(vec![7u8; PROMPT_HASH_LEN]);
        let payload = render_payload(
            "runs",
            &agent,
            &run_id("general", 1),
            "general",
            1,
            None,
            &[],
            plain(),
        );
        let quoted = serde_json::to_string(TOOL_PLANE_INSTRUCTION).unwrap();
        assert!(
            payload.contains(&format!(
                r#""runtime":{quoted},"workspace":{{"kind":"duckfs""#
            )),
            "runtime precedes workspace in the committed layout: {payload}"
        );
        // v3 is the ONLY composed version (the v2/flat tolerances died at the
        // flag day), so there is no legacy composer whose bytes this could
        // move: every composed envelope is v3 and every v3 envelope carries a
        // runtime key.
        assert_eq!(parse(&payload)["ducktape_run"], 3);
    }

    // ---- the forge workspace source (M1) --------------------------------------

    #[test]
    fn a_forge_run_composes_tagged_source_item_context_and_requested_pr_sink() {
        let agent = agent_with_hash(vec![7u8; PROMPT_HASH_LEN]);
        let transcript = vec![message(1, AuthorRef::User(vec![1; 32]), "hi bot")];
        let commit = "ab".repeat(20);
        let inputs = PortableInputs {
            workspace: WorkspaceSource::Forge {
                repo: "app".into(),
                commit: commit.clone(),
                branch: "agent/item-7".into(),
                branch_born: false,
            },
            skills: Vec::new(),
            sink: WireSink::Pr {
                repo: "app".into(),
                source_branch: "agent/item-7".into(),
                target_branch: "main".into(),
                title: String::new(),
                body: String::new(),
            },
            context: Some("Forge item context:\nrepo: app".into()),
        };
        let payload = render_payload(
            "runs",
            &agent,
            &run_id("forge:app:7", 1),
            "forge:app:7",
            1,
            None,
            &transcript,
            inputs,
        );
        let v = parse(&payload);
        assert_eq!(v["ducktape_run"], 3);
        assert_eq!(
            v["thread_key"], "forge:app:7#1",
            "thread continuity keys are unchanged — replies land in the item discussion"
        );
        // context rides IMMEDIATELY after conversation — field order is the bytes.
        assert!(
            payload.contains(r#"Reply as the agent.","context":"Forge item context:"#),
            "context follows conversation byte-adjacent: {payload}"
        );
        // the tagged forge source, with its committed field order.
        assert_eq!(v["workspace"]["kind"], "forge");
        assert_eq!(v["workspace"]["repo"], "app");
        assert_eq!(v["workspace"]["commit"], commit);
        assert_eq!(v["workspace"]["branch"], "agent/item-7");
        assert_eq!(v["workspace"]["branch_born"], false);
        assert!(
            payload.contains(&format!(
                r#""workspace":{{"kind":"forge","repo":"app","commit":"{commit}","branch":"agent/item-7","branch_born":false}}"#
            )),
            "the forge workspace field order is part of the committed bytes: {payload}"
        );
        // the requested sink carries mode/repo/source/target and NO title/body
        // keys — delivery derives them from the message facet (Task 4).
        assert_eq!(v["result_contract"]["ducktape_runner_result"], 1);
        assert_eq!(v["result_contract"]["sink"]["mode"], "pr");
        assert!(v["result_contract"]["sink"].get("title").is_none());
        assert!(v["result_contract"]["sink"].get("body").is_none());
        assert!(
            payload.contains(
                r#""result_contract":{"ducktape_runner_result":1,"sink":{"mode":"pr","repo":"app","source_branch":"agent/item-7","target_branch":"main"}}"#
            ),
            "the requested-sink field order is part of the committed bytes: {payload}"
        );
    }

    #[test]
    fn forge_envelope_bytes_are_deterministic() {
        let agent = agent_with_hash(vec![7u8; PROMPT_HASH_LEN]);
        let transcript = vec![message(1, AuthorRef::User(vec![1; 32]), "go")];
        let inputs = || PortableInputs {
            workspace: WorkspaceSource::Forge {
                repo: "app".into(),
                commit: "cd".repeat(20),
                branch: "feature/x".into(),
                branch_born: true,
            },
            skills: Vec::new(),
            sink: WireSink::Pr {
                repo: "app".into(),
                source_branch: "feature/x".into(),
                target_branch: "dev".into(),
                title: String::new(),
                body: String::new(),
            },
            context: Some("ctx".into()),
        };
        let a = render_payload(
            "runs",
            &agent,
            &run_id("forge:app:9", 1),
            "forge:app:9",
            1,
            None,
            &transcript,
            inputs(),
        );
        let b = render_payload(
            "runs",
            &agent,
            &run_id("forge:app:9", 1),
            "forge:app:9",
            1,
            None,
            &transcript,
            inputs(),
        );
        assert_eq!(
            a.as_bytes(),
            b.as_bytes(),
            "forge composition is byte-deterministic"
        );
    }
}
