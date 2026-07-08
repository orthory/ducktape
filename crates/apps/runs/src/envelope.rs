//! run envelope composition — the structured dispatch payload.
//!
//! the dispatch plane's rule stands: the DISPATCHER composes the entire model
//! input, in consensus, and it rides the dispatch as committed payload data
//! (P4). what changed from the flat-string era is the SHAPE — the payload is
//! a JSON envelope carrying the agent's real prompt pin (`prompt_hash`), a
//! thread-continuity key, the generic fallback instructions, the strict
//! output contract, and the rendered conversation. the host-side worker
//! detects the `ducktape_run` marker and assembles the final model input,
//! resolving `prompt_hash` from the content-addressed blob store — so the
//! exact prompt bytes stay verifiable against the committed hash. payloads
//! without the marker are legacy flat strings and pass through verbatim,
//! which keeps in-flight ops across an upgrade working.

use agent::{AgentRecord, PROMPT_HASH_LEN};
use chat::{AuthorRef, Block, MessageView};
use serde::Serialize;

use crate::hex;

/// the envelope marker the host worker routes on. bumping it is a payload
/// flag day for the worker, not for consensus state.
pub const RUN_ENVELOPE_VERSION: u32 = 3;

/// the result wrapper v3 providers return. the dispatch plane stores the full
/// bytes; runs unwraps `response_text` deterministically during delivery.
pub(crate) const RUNNER_RESULT_VERSION: u32 = 1;

const WORKSPACE_MOUNT_PATH: &str = "/workspace";

/// generic instructions for an agent without a consensus-resident prompt —
/// the host uses `instructions` only when `prompt_hash` is null.
pub(crate) const DEFAULT_PROMPT: &str =
    "You are a Ducktape agent. Reply helpfully and return only the requested JSON output.";

/// the strict output contract riding every composed payload — exactly the
/// [`agent::AgentResponse`] wire shape.
pub(crate) const STRICT_OUTPUT_INSTRUCTION: &str = r#"Return ONLY a JSON object with this shape:
{"reply_blocks":[{"id":"<uuid>","kind":"paragraph","text":"..."}],"actions":[]}
Allowed reply block kinds are paragraph, heading, and code. heading is rendered as a paragraph in Ducktape chat. code may include an optional "lang". Actions are optional and must use only actions allowed by the agent registry. Do not include markdown fences around the JSON."#;

/// the committed payload shape. FIELD ORDER IS PART OF THE COMMITTED BYTES:
/// serde_json serializes struct fields in declaration order, so this
/// declaration — not any map — is the canonical layout every validator
/// reproduces byte-for-byte.
#[derive(Serialize)]
struct RunEnvelope<'a> {
    ducktape_run: u32,
    agent_id: &'a str,
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
    workspace: WorkspaceEnvelope,
    base_tools: Vec<BaseToolEnvelope>,
    result_contract: ResultContractEnvelope,
}

#[derive(Serialize)]
struct WorkspaceEnvelope {
    source_prefix: String,
    source_snapshot: Option<String>,
    mount_path: &'static str,
}

#[derive(Serialize)]
struct BaseToolEnvelope {
    name: &'static str,
    version: &'static str,
    exposure: &'static str,
}

#[derive(Serialize)]
struct ResultContractEnvelope {
    ducktape_runner_result: u32,
}

/// serialize one envelope — deterministic: fixed field order (see
/// [`RunEnvelope`]) and serde_json's canonical string escaping.
fn envelope(
    agent: &AgentRecord,
    thread_key: Option<String>,
    conversation: String,
    source_snapshot: Option<String>,
) -> String {
    serde_json::to_string(&RunEnvelope {
        ducktape_run: RUN_ENVELOPE_VERSION,
        agent_id: &agent.agent_id,
        prompt_hash: prompt_hash_hex(agent),
        thread_key,
        instructions: DEFAULT_PROMPT,
        contract: STRICT_OUTPUT_INSTRUCTION,
        conversation,
        workspace: WorkspaceEnvelope {
            source_prefix: workspace_source_prefix(agent),
            source_snapshot,
            mount_path: WORKSPACE_MOUNT_PATH,
        },
        base_tools: base_tools(),
        result_contract: ResultContractEnvelope {
            ducktape_runner_result: RUNNER_RESULT_VERSION,
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

fn workspace_source_prefix(agent: &AgentRecord) -> String {
    format!("/shared/agent-workspaces/{}", agent.agent_id)
}

fn base_tools() -> Vec<BaseToolEnvelope> {
    vec![
        BaseToolEnvelope {
            name: "ducktape-files",
            version: "1",
            exposure: "cli",
        },
        BaseToolEnvelope {
            name: "ducktape-index",
            version: "1",
            exposure: "cli",
        },
        BaseToolEnvelope {
            name: "ducktape-chain",
            version: "1",
            exposure: "cli",
        },
    ]
}

/// compose a chat run's payload: the envelope around the rendered transcript
/// window ending at the anchor.
#[cfg(test)]
pub(crate) fn render_payload(
    module_id: &str,
    agent: &AgentRecord,
    channel_id: &str,
    anchor_seq: u64,
    thread_root: Option<u64>,
    transcript: &[MessageView],
) -> String {
    render_payload_with_workspace_snapshot(
        module_id,
        agent,
        channel_id,
        anchor_seq,
        thread_root,
        transcript,
        None,
    )
}

pub(crate) fn render_payload_with_workspace_snapshot(
    module_id: &str,
    agent: &AgentRecord,
    channel_id: &str,
    anchor_seq: u64,
    thread_root: Option<u64>,
    transcript: &[MessageView],
    source_snapshot: Option<String>,
) -> String {
    let thread_key = format!("{channel_id}#{}", thread_root.unwrap_or(anchor_seq));
    envelope(
        agent,
        Some(thread_key),
        render_conversation(module_id, &agent.agent_id, transcript),
        source_snapshot,
    )
}

/// compose a job run's payload: same envelope, no thread key, and the
/// conversation is the job's coordinates plus its FULL submitted spec.
#[cfg(test)]
pub(crate) fn render_job_payload(agent: &AgentRecord, job_id: &str, spec: &str) -> String {
    render_job_payload_with_workspace_snapshot(agent, job_id, spec, None)
}

pub(crate) fn render_job_payload_with_workspace_snapshot(
    agent: &AgentRecord,
    job_id: &str,
    spec: &str,
    source_snapshot: Option<String>,
) -> String {
    envelope(
        agent,
        None,
        format!(
            "Job {job_id} — chat replies are not delivered for job runs; respond with actions only.\n\nJob spec:\n{spec}"
        ),
        source_snapshot,
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

    #[test]
    fn a_chat_envelope_carries_the_prompt_pin_and_anchor_thread_key() {
        let agent = agent_with_hash(vec![7u8; PROMPT_HASH_LEN]);
        let transcript = vec![message(1, AuthorRef::User(vec![1; 32]), "hi bot")];
        let payload = render_payload("runs", &agent, "general", 1, None, &transcript);
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
        assert!(payload.starts_with(r#"{"ducktape_run":3,"agent_id":"bot","prompt_hash":"#));
    }

    #[test]
    fn a_v3_envelope_commits_workspace_tools_and_result_contract() {
        let agent = agent_with_hash(vec![7u8; PROMPT_HASH_LEN]);
        let transcript = vec![message(1, AuthorRef::User(vec![1; 32]), "hi bot")];
        let payload = render_payload("runs", &agent, "general", 1, None, &transcript);
        let v = parse(&payload);

        assert_eq!(v["ducktape_run"], 3);
        assert_eq!(
            v["workspace"]["source_prefix"],
            "/shared/agent-workspaces/bot"
        );
        assert!(
            v["workspace"]["source_snapshot"].is_null(),
            "a run without files-module wiring still commits an explicit null snapshot"
        );
        assert_eq!(v["workspace"]["mount_path"], "/workspace");
        assert_eq!(
            v["base_tools"],
            serde_json::json!([
                {"name":"ducktape-files","version":"1","exposure":"cli"},
                {"name":"ducktape-index","version":"1","exposure":"cli"},
                {"name":"ducktape-chain","version":"1","exposure":"cli"}
            ])
        );
        assert_eq!(v["result_contract"]["ducktape_runner_result"], 1);
        assert!(
            payload.starts_with(r#"{"ducktape_run":3,"agent_id":"bot","prompt_hash":"#),
            "version stays first in the committed byte layout"
        );
    }

    #[test]
    fn an_agent_without_a_prompt_pin_composes_null() {
        let agent = agent_with_hash(Vec::new());
        let payload = render_payload("runs", &agent, "general", 1, None, &[]);
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
        let payload = render_payload("runs", &agent, "general", 3, Some(1), &transcript);
        assert_eq!(
            parse(&payload)["thread_key"],
            "general#1",
            "every run in one thread shares a continuity key"
        );
    }

    #[test]
    fn a_job_envelope_has_no_thread_key_and_preserves_the_job_framing() {
        let agent = agent_with_hash(vec![7u8; PROMPT_HASH_LEN]);
        let payload = render_job_payload(&agent, "job-1", "summarize this work item");
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
        let a = render_payload("runs", &agent, "general", 2, None, &transcript);
        let b = render_payload("runs", &agent, "general", 2, None, &transcript);
        assert_eq!(
            a.as_bytes(),
            b.as_bytes(),
            "composition is byte-deterministic"
        );
        let j1 = render_job_payload(&agent, "job-1", "spec");
        let j2 = render_job_payload(&agent, "job-1", "spec");
        assert_eq!(j1.as_bytes(), j2.as_bytes());
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
        let payload = render_payload("runs", &agent, "general", 3, None, &transcript);
        let conversation = parse(&payload)["conversation"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(conversation.contains("[them] user:"));
        assert!(conversation.contains("[you] agent:runs/bot @2: my own reply"));
        assert!(conversation.contains("[them] agent:runs/other @3: someone else"));
    }
}
