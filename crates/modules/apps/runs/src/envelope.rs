//! run envelope composition — the structured dispatch payload.
//!
//! the dispatch plane's rule stands: the DISPATCHER composes the entire model
//! input, in consensus, and it rides the dispatch as committed payload data
//! (P4). the payload is a JSON envelope carrying a thread-continuity key, the
//! generic fallback instructions, the strict output contract, the rendered
//! conversation, and the portable workspace plan ([`PortableInputs`]) — whose
//! skill list now states, per skill, whether it loads `always`. the host-side
//! worker routes on the `ducktape_run` marker and assembles the final model
//! input. every run composes the ONE portable envelope shape; there are no
//! legacy flat-string or older envelope tolerances.
//!
//! the agent's PERSONA is no longer in here. it was a `prompt_hash` the host
//! resolved from the blob store; it is now an `always` skill, whose body the
//! host inlines into the context document the executor natively auto-loads — so
//! the envelope states which skills ran (pins), and the assembler builds the
//! soul from them. the tool-plane sentence and the "your skills are mounted
//! under…" runtime section left with it: both are ambient always-loaded
//! instructions, and they belong in that one document, composed once, rather
//! than as a second prompt-shaped channel here.
//!
//! `instructions` survives as the fallback for an agent with NO always-skill at
//! all. this structurally kills a trap: the old code silently DROPPED
//! `instructions` whenever `prompt_hash` was `Some`, so the fallback was
//! unreachable exactly when it looked configured. now it is reached by the
//! absence of always-skills — a state you can see.

use std::collections::BTreeSet;

use agent::{AgentRecord, LoadMode, MAX_SKILLS_PER_AGENT, SKILL_LIBRARY_PREFIX, SkillRef};
use chat::{AuthorRef, Block, MessageView};
use files::paths::canonical as canonical_duckfs_path;
use serde::Serialize;

use crate::facets::WireSink;
use crate::hex;

/// the envelope marker the host worker routes on: the portable envelope shape
/// (workspace source pin + skill refs + result contract). bumping it is a
/// payload flag day for the worker, not for consensus state.
pub(crate) const RUN_ENVELOPE_VERSION: u32 = 1;

/// the result-wrapper version a portable provider returns. carried in
/// the envelope's `result_contract` so the worker refuses a runner-result
/// version it cannot unwrap; the `runs` delivery path reads it back as `1`.
pub(crate) const RUNNER_RESULT_VERSION: u32 = 1;

/// generic instructions for an agent whose curated skills give it no persona —
/// the floor under an agent with no `always` skill to assemble.
pub(crate) const DEFAULT_PROMPT: &str =
    "You are a Ducktape agent. Reply helpfully and return only the requested JSON output.";

/// the strict output contract riding every composed payload — exactly the
/// [`agent::AgentResponse`] wire shape.
pub(crate) const STRICT_OUTPUT_INSTRUCTION: &str = r#"Return ONLY a JSON object with this shape:
{"reply_blocks":[{"id":"<uuid>","kind":"paragraph","text":"..."}],"actions":[],"delegations":[],"commit_message":"Your Git subject\n\nOptional body"}
Allowed reply block kinds are paragraph, heading, and code. heading is rendered as a paragraph in Ducktape chat. code may include an optional "lang". Actions are optional and must use only actions allowed by the agent registry. Use the live ducktape_delegate and ducktape_delegations tools for peer calls. The terminal delegations field remains only for older runners that cannot call tools mid-run; it is shaped as {"agent_id":"<registered agent>","instruction":"...","skills":["<library skill name>"]}. Every call uses caller ∩ callee authority, and the root subagent_budget admits at most min(N, 8) concurrent calls across the whole recursive tree; completed calls release their slot. For uncommitted workspace changes, use commit_message to author the complete Git message; Ducktape preserves it. Git commits you create keep their own messages. Omit commit_message when no uncommitted changes remain. Do not include markdown fences around the JSON."#;

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
    /// the committed registry name used for Forge authorship and the
    /// canonical Ducktape attribution trailer. This travels beside the id so
    /// the host never has to reconstruct public Git identity from a run key.
    agent_display_name: &'a str,
    /// `<channel_id>#<seq>` — seq is the anchor's thread root when the
    /// anchor is a thread reply (every run in one thread shares a key), else
    /// the anchor itself. null for job runs: there is no channel.
    thread_key: Option<String>,
    instructions: &'a str,
    contract: &'a str,
    conversation: String,
    /// the deterministic forge item-context instructions section (M1),
    /// rendered by `inject` from committed tracker state. the oracle's
    /// `prepare()` assembles the provider input as instructions → context →
    /// contract → conversation, so this reaches the model as its own section; a
    /// context-less envelope assembles byte-identically to the context-less
    /// worker. `None` (an ABSENT key) for every non-forge run.
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<String>,
    workspace: WorkspaceSource,
    skills: Vec<SkillEnvelope>,
    /// whether this agent's committed `duckfs_read` caps cover the global skill
    /// library (`agent::SKILL_LIBRARY_PREFIX`). the HOST cannot work this out —
    /// it has no registry to ask — so the composer states it, and the assembler
    /// emits the library paragraph only when it is true.
    ///
    /// a fact ABOUT the caps, never a widening of them: the answer comes from
    /// `AgentRecord::library_readable`, which is `permits(DuckfsRead(..))` — the
    /// same call the MCP tool plane gates the real read on. so the document can
    /// only advertise a door that will actually open.
    library_readable: bool,
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
        /// the authoritative tracker title at compose height. it owns the
        /// primary capture commit; response prose is only a missing-metadata
        /// fallback.
        item_title: String,
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
        /// the compose-height verdict from the agent's committed
        /// `forge_push` cap for this repo. the host may act on this fact but
        /// cannot widen it.
        forge_push: bool,
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
/// run, mirroring [`agent::SkillRef`]. a tracking skill's snapshot is resolved
/// to the committed head at compose time (see `RunsModule::portable_inputs`).
///
/// `always` is the curated [`LoadMode`], flattened to the bool the host needs:
/// it tells the assembler to inline this skill's full body into the run's
/// context document (the agent's soul) rather than merely index it. consensus
/// decides it; the host only obeys.
#[derive(Serialize, Debug)]
pub(crate) struct SkillEnvelope {
    pub name: String,
    pub source_prefix: String,
    pub source_snapshot: Option<String>,
    pub always: bool,
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

/// resolve the skill refs of a run against the committed duckfs head: a
/// pinned skill passes its snapshot through; a tracking skill (no pin)
/// resolves to the SAME committed head (W2) — deterministic across
/// validators. shared by the duckfs and forge compose lanes (skills are
/// duckfs subtrees either way).
///
/// TWO tiers land here. `agent.skills` is WHO THE AGENT IS — its standing
/// curation. `extra` is WHAT THIS TASK NEEDS — the skills the requester (an
/// operator's `RequestRun`, or a parent delegating this child) curated for this
/// one run; every other intake passes none. The union is ADDITIVE and the
/// agent leads: a name the agent already curates is kept as the agent's own ref
/// (a task supplements a persona, it never rewrites one), and `extra` appends
/// only what the agent lacks. curation ORDER is the order the host inlines the
/// `always` bodies in, so the persona always assembles first.
pub(crate) fn resolve_skills(
    agent: &AgentRecord,
    extra: &[SkillRef],
    head: &Option<String>,
) -> Vec<SkillEnvelope> {
    let have: BTreeSet<&str> = agent.skills.iter().map(|s| s.name.as_str()).collect();
    agent
        .skills
        .iter()
        .chain(extra.iter().filter(|s| !have.contains(s.name.as_str())))
        .map(|s| SkillEnvelope {
            name: s.name.clone(),
            source_prefix: s.source_prefix.clone(),
            source_snapshot: s.source_snapshot.clone().or_else(|| head.clone()),
            always: matches!(s.load, LoadMode::Always),
        })
        .collect()
}

/// expand requester-supplied library skill NAMES into on-demand refs, confined
/// to the shared library by CONSTRUCTION (`/shared/skills/<name>`).
///
/// this is the one place a run gains skills it was not curated with, and the
/// ro-mount that materializes them runs on the node's duckfs authority with no
/// read-cap gate — so a requester must never get to name a raw path. taking
/// NAMES, not refs, means a requester can only ever point at a library entry:
/// the name is canonicalized as the last segment of the library prefix, so a
/// `/` or `..` in it fails to resolve to a single entry and is refused. no
/// pinned snapshot, no `always` — a requester offers a library skill on demand;
/// only an owner's own record inlines a persona.
pub(crate) fn library_skills(names: &[String]) -> Result<Vec<SkillRef>, String> {
    // the SAME ceiling the agent record's own curation carries — the resolved
    // set is bounded there, and an unbounded request would otherwise commit a
    // giant dispatch payload and force one duckfs checkout per name on the
    // executing node for a run the assembler was always going to refuse.
    if names.len() > MAX_SKILLS_PER_AGENT {
        return Err(format!(
            "a run may curate at most {MAX_SKILLS_PER_AGENT} skills, got {}",
            names.len()
        ));
    }
    let mut seen = BTreeSet::new();
    let mut refs = Vec::with_capacity(names.len());
    for name in names {
        let source_prefix = format!("{SKILL_LIBRARY_PREFIX}/{name}");
        let seg =
            canonical_duckfs_path(&source_prefix).map_err(|e| format!("skill {name:?}: {e}"))?;
        // exactly `/shared/skills/<name>` — one segment past the library root.
        // a name carrying a `/` or `..` lands at a different depth (or fails to
        // canonicalize), so this rejects anything but a direct library entry.
        if seg.len() != 3 {
            return Err(format!(
                "a per-run skill must be a single shared-library entry, got {name:?}"
            ));
        }
        if !seen.insert(name.as_str()) {
            return Err(format!("skill named twice: {name}"));
        }
        refs.push(SkillRef {
            name: name.clone(),
            source_prefix,
            source_snapshot: None,
            load: LoadMode::OnDemand,
        });
    }
    Ok(refs)
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
        agent_display_name: &agent.display_name,
        thread_key,
        instructions: DEFAULT_PROMPT,
        contract: STRICT_OUTPUT_INSTRUCTION,
        conversation,
        context: portable.context,
        workspace: portable.workspace,
        skills: portable.skills,
        library_readable: agent.library_readable(),
        result_contract: ResultContractEnvelope {
            ducktape_runner_result: RUNNER_RESULT_VERSION,
            sink: portable.sink,
        },
    })
    .expect("envelope is serializable")
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

    /// an agent defined by its curated skills — there is no prompt to give it.
    fn agent_with_skills(skills: Vec<agent::SkillRef>) -> AgentRecord {
        AgentRecord {
            agent_id: "bot".into(),
            owner: SagaOrigin::External(vec![9; 32]),
            display_name: "BOT".into(),
            capability: "model-1".into(),
            allowed_actions: vec![],
            status: AgentStatus::Active,
            role: agent::AgentRole::General,
            created_at: 0,
            updated_at: 0,
            recipe_hash: Vec::new(),
            caps: agent::ResourceCaps::default(),
            skills,
        }
    }

    fn bot() -> AgentRecord {
        agent_with_skills(Vec::new())
    }

    /// a curated skill ref, as the registry commits it.
    fn skill_ref(name: &str, load: LoadMode) -> agent::SkillRef {
        agent::SkillRef {
            name: name.into(),
            source_prefix: format!("/shared/skills/{name}"),
            source_snapshot: Some("bb".repeat(32)),
            load,
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
            workspace: duckfs_workspace(&bot(), snapshot.map(str::to_string)),
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
            always: false,
        }
    }

    #[test]
    fn a_chat_envelope_carries_the_anchor_thread_key_and_no_prompt_pin() {
        let agent = bot();
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
        assert_eq!(v["agent_display_name"], "BOT");
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

        // the persona plane is GONE from the envelope: no prompt pin to resolve
        // from a blob store, and no composed runtime text. the soul is
        // assembled host-side from the skills below.
        assert!(
            v.get("prompt_hash").is_none(),
            "prompt_hash retired: {payload}"
        );
        assert!(
            v.get("runtime").is_none(),
            "the runtime section moved into the assembled context document: {payload}"
        );

        // field order is part of the committed bytes — assert the layout, not
        // just the values.
        assert!(payload.starts_with(r#"{"ducktape_run":1,"agent_id":"bot","run_id":"#));
        assert!(
            payload.contains(r#","agent_display_name":"BOT","thread_key":"#),
            "the thread key now follows the display name directly: {payload}"
        );
    }

    /// the soul contract: the composer carries each curated skill's load mode
    /// through to the host as `always`, in curation order — that flag is what
    /// tells the assembler which bodies to INLINE into the agent's context
    /// document and which to merely index.
    #[test]
    fn skills_carry_their_load_mode_through_to_the_host() {
        let agent = agent_with_skills(vec![
            skill_ref("persona", LoadMode::Always),
            skill_ref("release", LoadMode::OnDemand),
        ]);
        let skills = resolve_skills(&agent, &[], &None);
        let payload = render_payload(
            "runs",
            &agent,
            &run_id("general", 1),
            "general",
            1,
            None,
            &[],
            PortableInputs {
                workspace: duckfs_workspace(&agent, None),
                skills,
                sink: WireSink::Chain,
                context: None,
            },
        );
        let v = parse(&payload);
        let skills = v["skills"].as_array().unwrap();
        assert_eq!(skills.len(), 2);
        assert_eq!(skills[0]["name"], "persona", "curation order is preserved");
        assert_eq!(
            skills[0]["always"], true,
            "the persona is an always-skill — the host inlines its body"
        );
        assert_eq!(skills[1]["name"], "release");
        assert_eq!(
            skills[1]["always"], false,
            "an on-demand skill is indexed, not inlined"
        );
    }

    /// the library grant the host assembles on is READ OFF THE CAPS, never
    /// assumed: an agent whose `duckfs_read` covers the library prefix composes
    /// `library_readable: true` and is told the library exists; one without the
    /// grant composes `false` and is never pointed at a door the MCP tool plane
    /// would refuse it (the caps ARE that refusal — same `permits` call).
    #[test]
    fn the_envelope_states_the_agents_library_read_grant() {
        let compose = |caps: agent::ResourceCaps| {
            let mut agent = agent_with_skills(Vec::new());
            agent.caps = caps;
            let payload = render_payload(
                "runs",
                &agent,
                &run_id("general", 1),
                "general",
                1,
                None,
                &[],
                PortableInputs {
                    workspace: duckfs_workspace(&agent, None),
                    skills: Vec::new(),
                    sink: WireSink::Chain,
                    context: None,
                },
            );
            parse(&payload)["library_readable"].clone()
        };

        assert_eq!(
            compose(agent::ResourceCaps::default()),
            Value::Bool(false),
            "the empty default grants nothing, so the agent hears nothing about the library"
        );
        assert_eq!(
            compose(agent::ResourceCaps {
                duckfs_read: vec![agent::SKILL_LIBRARY_PREFIX.into()],
                ..Default::default()
            }),
            Value::Bool(true),
            "the grant the app pre-fills is the grant the assembler acts on"
        );
        assert_eq!(
            compose(agent::ResourceCaps {
                duckfs_read: vec!["/shared/agent-workspaces/bot".into()],
                ..Default::default()
            }),
            Value::Bool(false),
            "an unrelated read grant is not a library grant"
        );
    }

    /// the per-run union: the agent's own skills lead, and `extra` appends only
    /// the names the agent does not already carry — a task supplements a
    /// persona, never rewrites it, and order is the `always`-inline order.
    #[test]
    fn extra_skills_append_after_the_agents_and_only_when_new() {
        let agent = agent_with_skills(vec![
            skill_ref("persona", LoadMode::Always),
            skill_ref("rust-gates", LoadMode::Always),
        ]);
        let extra = library_skills(&["rust-gates".into(), "qa".into()]).unwrap();
        let resolved = resolve_skills(&agent, &extra, &None);
        let names: Vec<&str> = resolved.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(
            names,
            ["persona", "rust-gates", "qa"],
            "the already-curated name is kept once (as the agent's own ref), qa is added"
        );
        // the agent's own rust-gates ref wins the collision — its Always load
        // mode is preserved, not downgraded to the extra's OnDemand.
        assert!(
            resolved[1].always,
            "a task supplements; it does not downgrade the agent's own skill"
        );
    }

    /// `library_skills` confines by construction and refuses anything that is
    /// not a single library entry.
    #[test]
    fn library_skills_confines_names_to_the_library() {
        let ok = library_skills(&["rust-gates".into(), "qa".into()]).unwrap();
        assert_eq!(ok[0].source_prefix, "/shared/skills/rust-gates");
        assert!(matches!(ok[0].load, LoadMode::OnDemand));
        assert!(ok[0].source_snapshot.is_none());

        for bad in [
            "../agents/victim/persona", // escapes the library
            "a/b",                      // not a single entry
            "",                         // empty
            "..",                       // parent
        ] {
            assert!(
                library_skills(&[bad.into()]).is_err(),
                "{bad:?} must be refused"
            );
        }
        // a name repeated in one request is refused (it would mount twice).
        assert!(library_skills(&["qa".into(), "qa".into()]).is_err());

        // and the count is capped in consensus — the same ceiling the agent
        // record carries — so a huge request cannot commit a giant payload or
        // force a checkout per name for a doomed run.
        let too_many: Vec<String> = (0..=agent::MAX_SKILLS_PER_AGENT)
            .map(|i| format!("s{i}"))
            .collect();
        assert!(library_skills(&too_many).is_err());
        assert!(library_skills(&too_many[..agent::MAX_SKILLS_PER_AGENT]).is_ok());
    }

    /// a tracking skill (no pin) still resolves to the committed head, and its
    /// load mode is untouched by that resolution.
    #[test]
    fn a_tracking_skill_resolves_to_the_head_and_keeps_its_load_mode() {
        let agent = agent_with_skills(vec![agent::SkillRef {
            name: "persona".into(),
            source_prefix: "/shared/skills/persona".into(),
            source_snapshot: None,
            load: LoadMode::Always,
        }]);
        let head = Some("aa".repeat(32));
        let resolved = resolve_skills(&agent, &[], &head);
        assert_eq!(resolved[0].source_snapshot, head);
        assert!(resolved[0].always);
    }

    #[test]
    fn every_envelope_names_the_run_the_pending_map_is_keyed_by() {
        // the host cannot derive this id (its own run key is
        // `{saga_id}:{attempt}`), so an envelope that omits it leaves the
        // executing node unable to name its own run back to this module — the
        // session bind and every mid-run action then address a run that does
        // not exist. all three lanes must carry it.
        let agent = bot();

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

    /// an agent with no always-skill has no persona to assemble — the generic
    /// `instructions` are the floor under it. this fallback was UNREACHABLE
    /// before (the old composer dropped `instructions` whenever a prompt pin was
    /// set); now it is reached by a state you can see.
    #[test]
    fn a_soulless_agent_falls_back_to_the_generic_instructions() {
        let agent = agent_with_skills(vec![skill_ref("release", LoadMode::OnDemand)]);
        let payload = render_payload(
            "runs",
            &agent,
            &run_id("general", 1),
            "general",
            1,
            None,
            &[],
            portable(None, resolve_skills(&agent, &[], &None)),
        );
        let v = parse(&payload);
        assert_eq!(v["instructions"], DEFAULT_PROMPT);
        assert!(
            v["skills"]
                .as_array()
                .unwrap()
                .iter()
                .all(|s| s["always"] == false),
            "no always-skill ⇒ no assembled persona"
        );
        assert_eq!(
            v["conversation"],
            "No transcript was embedded for this run. Answer the user helpfully.",
            "the empty-transcript wording is preserved"
        );
    }

    #[test]
    fn a_threaded_anchor_keys_by_its_thread_root() {
        let agent = bot();
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
        let agent = bot();
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
        assert_eq!(
            v["conversation"],
            "Job job-1 — chat replies are not delivered for job runs; respond with actions only.\n\nJob spec:\nsummarize this work item"
        );
    }

    #[test]
    fn envelope_bytes_are_deterministic() {
        let agent = bot();
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

        // a skilled run is byte-stable too — the skill list (names, pins, load
        // flags) is composed straight from committed state.
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
            "a skilled compose is a pure function of committed state"
        );
    }

    #[test]
    fn the_agents_own_messages_render_as_you() {
        let agent = bot();
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
        let agent = bot();
        let transcript = vec![message(1, AuthorRef::User(vec![1; 32]), "hi bot")];
        let skills = vec![SkillEnvelope {
            name: "release".into(),
            source_prefix: "/shared/skills/release".into(),
            source_snapshot: Some("bb".repeat(32)),
            always: false,
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
            payload.starts_with(r#"{"ducktape_run":1,"agent_id":"bot","run_id":"#),
            "the marker leads with a stable field order: {payload}"
        );
        assert!(
            payload.contains(r#","agent_display_name":"BOT","thread_key":"#),
            "the thread key follows the display name directly: {payload}"
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
        let agent = bot();
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

    /// the runtime section is GONE from the committed bytes. it used to sit
    /// between `context` and `workspace` and tell the model about the tool plane
    /// and its skill mounts; both are ambient always-loaded instructions, so
    /// they belong in the one context document the host assembles — stated once,
    /// where the CLI already reads it, instead of as a second prompt channel
    /// here. `conversation` now runs straight into `workspace`.
    #[test]
    fn the_envelope_composes_no_runtime_text() {
        let agent = bot();
        let transcript = vec![message(1, AuthorRef::User(vec![1; 32]), "hi bot")];
        let payload = render_payload(
            "runs",
            &agent,
            &run_id("general", 1),
            "general",
            1,
            None,
            &transcript,
            portable(None, vec![skill("release")]),
        );
        assert!(
            !payload.contains("\"runtime\""),
            "no runtime key in the committed layout: {payload}"
        );
        assert!(
            !payload.contains("DUCKTAPE_RUN_SKILLS") && !payload.contains("MCP tool server"),
            "the mount + tool-plane prose moved host-side: {payload}"
        );
        assert!(
            payload.contains(r#"Reply as the agent.","workspace":{"kind":"duckfs""#),
            "conversation runs straight into workspace: {payload}"
        );
        assert_eq!(parse(&payload)["ducktape_run"], 3);
    }

    // ---- the forge workspace source (M1) --------------------------------------

    #[test]
    fn a_forge_run_composes_tagged_source_item_context_and_requested_pr_sink() {
        let agent = bot();
        let transcript = vec![message(1, AuthorRef::User(vec![1; 32]), "hi bot")];
        let commit = "ab".repeat(20);
        let inputs = PortableInputs {
            workspace: WorkspaceSource::Forge {
                repo: "app".into(),
                item_title: "Fix the gate".into(),
                commit: commit.clone(),
                branch: "agent/item-7".into(),
                branch_born: false,
                forge_push: false,
            },
            skills: Vec::new(),
            sink: WireSink::Pr {
                repo: "app".into(),
                source_branch: "agent/item-7".into(),
                target_branch: "dev".into(),
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
        assert_eq!(v["workspace"]["item_title"], "Fix the gate");
        assert_eq!(v["workspace"]["commit"], commit);
        assert_eq!(v["workspace"]["branch"], "agent/item-7");
        assert_eq!(v["workspace"]["branch_born"], false);
        assert!(
            payload.contains(&format!(
                r#""workspace":{{"kind":"forge","repo":"app","item_title":"Fix the gate","commit":"{commit}","branch":"agent/item-7","branch_born":false,"forge_push":false}}"#
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
                r#""result_contract":{"ducktape_runner_result":1,"sink":{"mode":"pr","repo":"app","source_branch":"agent/item-7","target_branch":"dev"}}"#
            ),
            "the requested-sink field order is part of the committed bytes: {payload}"
        );
    }

    #[test]
    fn forge_envelope_bytes_are_deterministic() {
        let agent = bot();
        let transcript = vec![message(1, AuthorRef::User(vec![1; 32]), "go")];
        let inputs = || PortableInputs {
            workspace: WorkspaceSource::Forge {
                repo: "app".into(),
                item_title: "Ship the fix".into(),
                commit: "cd".repeat(20),
                branch: "feature/x".into(),
                branch_born: true,
                forge_push: false,
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
