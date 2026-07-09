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
///
/// HELD AT 2 on purpose: the portable `v3` shape (workspace mounts + base-tool
/// manifest) is a consensus flag-day (M1) and MUST NOT be composed until the
/// provisioning wrapper exists and a coordinated upgrade flips it (ADR
/// `2026-07-09-deterministic-agent-runtime`, ROL/M2). The worker already
/// *accepts* v3 (see `dispatch-oracle`) so that flip lands on ready nodes.
pub const RUN_ENVELOPE_VERSION: u32 = 2;

/// the result-wrapper version a portable (`v3`) provider returns. carried here
/// so the `runs` delivery path can unwrap `response_text` from a runner result
/// while the composer stays on v2 — a forward-compatible accept, no flip.
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
}

/// serialize one envelope — deterministic: fixed field order (see
/// [`RunEnvelope`]) and serde_json's canonical string escaping.
fn envelope(agent: &AgentRecord, thread_key: Option<String>, conversation: String) -> String {
    serde_json::to_string(&RunEnvelope {
        ducktape_run: RUN_ENVELOPE_VERSION,
        agent_id: &agent.agent_id,
        prompt_hash: prompt_hash_hex(agent),
        thread_key,
        instructions: DEFAULT_PROMPT,
        contract: STRICT_OUTPUT_INSTRUCTION,
        conversation,
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
pub(crate) fn render_payload(
    module_id: &str,
    agent: &AgentRecord,
    channel_id: &str,
    anchor_seq: u64,
    thread_root: Option<u64>,
    transcript: &[MessageView],
) -> String {
    let thread_key = format!("{channel_id}#{}", thread_root.unwrap_or(anchor_seq));
    envelope(
        agent,
        Some(thread_key),
        render_conversation(module_id, &agent.agent_id, transcript),
    )
}

/// compose a job run's payload: same envelope, no thread key, and the
/// conversation is the job's coordinates plus its FULL submitted spec.
pub(crate) fn render_job_payload(agent: &AgentRecord, job_id: &str, spec: &str) -> String {
    envelope(
        agent,
        None,
        format!(
            "Job {job_id} — chat replies are not delivered for job runs; respond with actions only.\n\nJob spec:\n{spec}"
        ),
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
        assert!(payload.starts_with(r#"{"ducktape_run":2,"agent_id":"bot","prompt_hash":"#));
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
