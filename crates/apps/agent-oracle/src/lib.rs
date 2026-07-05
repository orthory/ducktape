//! host-side LLM oracle worker for agent saga effects.
//!
//! the module state machine emits an opaque saga `WorkerRequest`. this crate is
//! the impure host-side counterpart: it recognizes agent `LlmRequest` specs,
//! runs them on a machine-local [`capability_host::Provider`] (whichever
//! executor CLI the operator brought), and returns a saga `OracleResult` op.
//!
//! this crate knows NOTHING about what it is running: the request names a
//! capability tag, `ProviderSet::resolve` maps the tag to a local provider,
//! and the provider's spec (operator data) says what execution means —
//! binary, flags, model. no executor or model name appears here. a request
//! for an uninstalled capability is a clean per-request error naming exactly
//! what is missing.
//!
//! credentials are emphatically NOT this crate's concern: it never reads,
//! writes, or refreshes any auth file. it renders a prompt, hands it to a BYO
//! CLI, and parses the CLI's answer.
//!
//! ## assignment-aware: run only what is leased to this node
//!
//! a `WorkerRequest` may carry an `assignee` — the node key whose lease this
//! attempt is (rendezvous over the capability's providers). when the
//! assignee is SOMEONE ELSE, this worker deliberately skips the spawn
//! ([`reactor::WorkOutcome::Handled`] with no follow-up): under the strict
//! lease policy a foreign result would be a no-op anyway, and not spawning
//! is what turns N-nodes-each-paying-for-the-same-LLM-call into one call.
//! an UNASSIGNED request (no providers announced) is run by anyone.

use std::sync::atomic::{AtomicBool, Ordering};

use agent_interface::{
    AgentAction, AgentOutput, LlmRequest, MAX_ACTIONS_PER_RUN, MAX_REPLY_BLOCKS_BYTES,
    decode_llm_request, encode_output,
};
use capability_host::ProviderSet;
use chat_interface::{AuthorRef, Block, MessageView};
use reactor::{WorkOutcome, Worker};
use saga_interface::{SagaMsg, WorkerRequest, decode_worker_request, encode_msg};
use sdk::{Effect, Msg};
use serde::Deserialize;
use sha2::{Digest as _, Sha256};

const DEFAULT_PROMPT: &str =
    "You are a Ducktape agent. Reply helpfully and return only the requested JSON output.";
const STRICT_OUTPUT_INSTRUCTION: &str = r#"Return ONLY a JSON object with this shape:
{"reply_blocks":[{"id":"<uuid>","kind":"Paragraph","text":"..."}],"actions":[]}
Allowed reply block kinds are Paragraph, Heading, and Code. Heading is rendered as a paragraph in Ducktape chat. Code may include an optional "lang". Actions are optional and must use only actions allowed by the agent registry. Do not include markdown fences around the JSON."#;

static MISSING_PROMPT_LOGGED: AtomicBool = AtomicBool::new(false);

/// Production LLM worker for agent `LlmRequest` saga effects. holds the
/// host's provider surface (loaded specs + discovered providers) and this
/// node's own key, so foreign-leased work is skipped instead of double-run.
pub struct LlmWorker {
    blobs: files::BlobHandle,
    providers: ProviderSet,
    /// this node's external submit key — compared against a request's
    /// `assignee` to decide whether the lease is ours to execute.
    node_key: Vec<u8>,
}

impl LlmWorker {
    pub fn new(blobs: files::BlobHandle, providers: ProviderSet, node_key: Vec<u8>) -> Self {
        Self {
            blobs,
            providers,
            node_key,
        }
    }

    async fn answer(&self, llm: &LlmRequest) -> Result<Vec<u8>, String> {
        let system = self.prompt_text(llm)?;
        // explicit dispatch: the request names the capability, the spec
        // behind the local provider decides everything else.
        let provider = self.providers.resolve(&llm.capability)?;
        let prompt = render_prompt(&system, llm);
        let text = provider.run(&prompt).await?;
        let output = agent_output_from_text(&text, llm.job_id.is_some());
        Ok(encode_output(&output))
    }

    fn prompt_text(&self, llm: &LlmRequest) -> Result<String, String> {
        let prompt_hash: [u8; 32] = llm.prompt_hash.as_slice().try_into().map_err(|_| {
            format!(
                "prompt hash is {} bytes, expected 32",
                llm.prompt_hash.len()
            )
        })?;
        let Some(bytes) = self.blobs.get_chunk(&prompt_hash) else {
            if !MISSING_PROMPT_LOGGED.swap(true, Ordering::Relaxed) {
                eprintln!(
                    "[agent-oracle] prompt blob {} is missing; using generic fallback instructions",
                    hex(&prompt_hash)
                );
            }
            return Ok(DEFAULT_PROMPT.into());
        };
        let actual: [u8; 32] = Sha256::digest(&bytes).into();
        if actual != prompt_hash {
            return Err("prompt blob digest did not match prompt_hash".into());
        }
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
}

#[async_trait::async_trait(?Send)]
impl Worker for LlmWorker {
    async fn run(&self, effect: &Effect) -> Result<WorkOutcome, reactor::Error> {
        let request = match decode_worker_request(&effect.0) {
            Ok(request) => request,
            Err(_) => return Ok(WorkOutcome::NotMine),
        };
        let llm = match decode_llm_request(&request.spec) {
            Ok(llm) => llm,
            Err(_) => return Ok(WorkOutcome::NotMine),
        };
        match &request.assignee {
            // the lease gate, host side: someone else's assignment is a
            // deliberate skip — claimed (it IS our effect type), not run.
            // the assignee submits the result; running it here would just
            // burn an LLM call on a result strict would no-op.
            Some(assignee) if *assignee != self.node_key => Ok(WorkOutcome::Handled(None)),
            Some(_) => {
                let outcome = self.answer(&llm).await.map_err(clean_error);
                Ok(WorkOutcome::Handled(Some(oracle_result(&request, outcome))))
            }
            // an UNASSIGNED request is an announcement, not a work order:
            // claim it with Accept when this host can actually run the
            // capability, and let the re-emitted request naming the winner
            // do the executing — one LLM call, however many capable nodes.
            None => {
                if self.providers.resolve(&llm.capability).is_err() {
                    return Ok(WorkOutcome::Handled(None));
                }
                Ok(WorkOutcome::Handled(Some(Msg {
                    target: "saga".into(),
                    payload: encode_msg(&SagaMsg::Accept {
                        saga_id: request.saga_id.clone(),
                        attempt: request.attempt,
                    }),
                })))
            }
        }
    }
}

/// flatten the run into a single prompt for a non-interactive CLI: the
/// system instructions, the strict-output contract, then the conversation (or
/// the job coordinates for a jobs-board run). the providers take flat text —
/// structured role turns were an artifact of the old Responses API and are
/// deliberately gone.
fn render_prompt(system: &str, llm: &LlmRequest) -> String {
    let mut out = String::new();
    out.push_str(system);
    out.push_str("\n\n");
    out.push_str(STRICT_OUTPUT_INSTRUCTION);
    out.push_str("\n\n");
    out.push_str(&render_conversation(llm));
    out
}

fn render_conversation(llm: &LlmRequest) -> String {
    if llm.job_id.is_some() {
        return format!(
            "Agent job run {}\nJob id: {}\nContext hash: {}",
            llm.run_id,
            llm.job_id.as_deref().unwrap_or_default(),
            hex(&llm.context_hash)
        );
    }
    if llm.transcript.is_empty() {
        return "No transcript was embedded for this run. Answer the user helpfully.".into();
    }
    let mut out = String::from("Conversation so far:\n");
    for message in &llm.transcript {
        let speaker = if is_this_agent(message, &llm.agent_id) {
            "you"
        } else {
            "them"
        };
        out.push_str(&format!("[{speaker}] {}\n", render_message(message)));
    }
    out.push_str("\nReply as the agent.");
    out
}

fn oracle_result(request: &WorkerRequest, outcome: Result<Vec<u8>, String>) -> Msg {
    Msg {
        target: "saga".into(),
        payload: encode_msg(&SagaMsg::OracleResult {
            saga_id: request.saga_id.clone(),
            attempt: request.attempt,
            outcome,
        }),
    }
}

fn clean_error(error: String) -> String {
    const MAX: usize = 2048;
    if error.len() <= MAX {
        return error;
    }
    let mut keep = MAX;
    while keep > 0 && !error.is_char_boundary(keep) {
        keep -= 1;
    }
    let mut out = error;
    out.truncate(keep);
    out
}

fn is_this_agent(message: &MessageView, agent_id: &str) -> bool {
    matches!(
        &message.head.author,
        AuthorRef::Agent { module, agent_id: author_agent }
            if module == "agent" && author_agent == agent_id
    )
}

fn render_message(message: &MessageView) -> String {
    format!(
        "{} @{}: {}",
        render_author(&message.head.author),
        message.seq,
        render_blocks(&message.head.blocks)
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

fn render_blocks(blocks: &[Block]) -> String {
    blocks
        .iter()
        .map(render_block)
        .collect::<Vec<_>>()
        .join("\n")
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

fn agent_output_from_text(text: &str, job_run: bool) -> AgentOutput {
    let parsed = serde_json::from_str::<AgentOutput>(text)
        .or_else(|_| serde_json::from_str::<ModelOutput>(text).map(ModelOutput::into_agent_output));
    let fallback = || AgentOutput {
        reply_blocks: if job_run {
            Vec::new()
        } else {
            vec![Block::paragraph(non_empty_text(text))]
        },
        actions: Vec::new(),
    };
    normalize_output(parsed.unwrap_or_else(|_| fallback()), text, job_run)
}

#[derive(Debug, Deserialize)]
struct ModelOutput {
    #[serde(default)]
    reply_blocks: Vec<ModelBlock>,
    #[serde(default)]
    actions: Vec<AgentAction>,
}

impl ModelOutput {
    fn into_agent_output(self) -> AgentOutput {
        AgentOutput {
            reply_blocks: self
                .reply_blocks
                .into_iter()
                .filter_map(ModelBlock::into_block)
                .collect(),
            actions: self.actions,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ModelBlock {
    #[allow(dead_code)]
    #[serde(default)]
    id: Option<String>,
    kind: String,
    text: String,
    #[serde(default)]
    lang: Option<String>,
}

impl ModelBlock {
    fn into_block(self) -> Option<Block> {
        let text = self.text.trim().to_string();
        if text.is_empty() {
            return None;
        }
        match self.kind.as_str() {
            "Paragraph" | "Heading" => Some(Block::paragraph(text)),
            "Code" => Some(Block::Code {
                lang: self.lang.filter(|l| !l.is_empty()),
                text,
            }),
            _ => None,
        }
    }
}

fn normalize_output(mut output: AgentOutput, raw_text: &str, job_run: bool) -> AgentOutput {
    output.actions.truncate(MAX_ACTIONS_PER_RUN);
    if job_run {
        output.reply_blocks.clear();
    }
    if !job_run && output.reply_blocks.is_empty() {
        output
            .reply_blocks
            .push(Block::paragraph(non_empty_text(raw_text)));
    }
    if !output.reply_blocks.is_empty() {
        let bytes = serde_json::to_vec(&output.reply_blocks).expect("blocks serialize");
        if bytes.len() > MAX_REPLY_BLOCKS_BYTES {
            output.reply_blocks = vec![Block::paragraph(truncate_utf8(
                &non_empty_text(raw_text),
                MAX_REPLY_BLOCKS_BYTES / 4,
            ))];
        }
    }
    output
}

fn non_empty_text(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        "Done.".into()
    } else {
        trimmed.into()
    }
}

fn truncate_utf8(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let mut keep = max;
    while keep > 0 && !text.is_char_boundary(keep) {
        keep -= 1;
    }
    let mut out = text.to_string();
    out.truncate(keep);
    out
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chat_interface::{MessageHead, ReactionSummary, Span};

    /// a provider surface with one loaded mock spec and NO installed
    /// binaries — enough for every non-live test; no executor is named.
    fn mock_specs_only() -> ProviderSet {
        let spec = capability_host::CapabilitySpec::parse(
            r#"
spec = 1
[capability]
tag = "alpha"
[detect]
bin = "alpha-cli"
[invoke]
args = []
prompt = "stdin"
[output]
format = "text"
"#,
            "test",
        )
        .expect("mock spec parses");
        ProviderSet::assemble(capability_host::SpecSet::from_specs(vec![spec]), Vec::new())
    }

    #[test]
    fn prompt_renders_system_contract_and_transcript() {
        let llm = LlmRequest {
            run_id: "run-1".into(),
            agent_id: "bot".into(),
            capability: "alpha".into(),
            prompt_hash: vec![0; 32],
            channel_id: "general".into(),
            anchor_seq: 2,
            job_id: None,
            context_hash: vec![1, 2, 3],
            transcript: vec![
                message(1, AuthorRef::User(vec![1, 2]), "hello"),
                message(
                    2,
                    AuthorRef::Agent {
                        module: "agent".into(),
                        agent_id: "bot".into(),
                    },
                    "hi",
                ),
            ],
        };
        let prompt = render_prompt("system prompt", &llm);
        assert!(prompt.contains("system prompt"), "system instructions present");
        assert!(
            prompt.contains("Return ONLY a JSON object"),
            "strict-output contract present"
        );
        assert!(prompt.contains("[them]"), "the user turn is tagged them");
        assert!(prompt.contains("[you]"), "the agent's own turn is tagged you");
        assert!(prompt.contains("hello") && prompt.contains("hi"), "turns rendered");
    }

    #[test]
    fn job_runs_render_job_coordinates_not_a_transcript() {
        let llm = LlmRequest {
            run_id: "run-9".into(),
            agent_id: "bot".into(),
            capability: "alpha".into(),
            prompt_hash: vec![0; 32],
            channel_id: "general".into(),
            anchor_seq: 0,
            job_id: Some("job-42".into()),
            context_hash: vec![0xab],
            transcript: Vec::new(),
        };
        let prompt = render_prompt("sys", &llm);
        assert!(prompt.contains("Job id: job-42"), "job id rendered");
        assert!(prompt.contains("Context hash: ab"), "context hash rendered");
    }

    #[test]
    fn model_friendly_json_is_normalized_to_chat_blocks() {
        let text = r#"{"reply_blocks":[{"id":"b1","kind":"Paragraph","text":"hello"},{"kind":"Code","lang":"rust","text":"fn main() {}"}],"actions":[]}"#;
        let out = agent_output_from_text(text, false);
        assert_eq!(
            out.reply_blocks,
            vec![
                Block::paragraph("hello"),
                Block::Code {
                    lang: Some("rust".into()),
                    text: "fn main() {}".into()
                }
            ]
        );

        let wrapped = agent_output_from_text("not json", false);
        assert_eq!(wrapped.reply_blocks, vec![Block::paragraph("not json")]);
    }

    #[test]
    fn a_missing_capability_is_a_clean_error_not_a_panic() {
        // the spec is loaded but no binary is installed: resolve() names the
        // missing capability rather than dispatching into the void — the
        // operator learns WHAT to install.
        let blobs = files::BlobHandle::default();
        let prompt_hash = blobs.put_chunk(b"be helpful".to_vec());
        let worker = LlmWorker::new(blobs, mock_specs_only(), b"me".to_vec());
        let llm = LlmRequest {
            run_id: "r".into(),
            agent_id: "bot".into(),
            capability: "alpha".into(),
            prompt_hash: prompt_hash.to_vec(),
            channel_id: "general".into(),
            anchor_seq: 1,
            job_id: None,
            context_hash: Vec::new(),
            transcript: vec![message(1, AuthorRef::User(b"h".to_vec()), "hi")],
        };
        let err = futures::executor::block_on(worker.answer(&llm)).unwrap_err();
        assert!(err.contains("\"alpha\" is not provided"), "got: {err}");
    }

    #[tokio::test]
    async fn a_foreign_lease_is_skipped_and_an_own_or_open_lease_is_run() {
        use saga_interface::{WorkerRequest, encode_worker_request};

        let blobs = files::BlobHandle::default();
        let prompt_hash = blobs.put_chunk(b"be helpful".to_vec());
        let worker = LlmWorker::new(blobs, mock_specs_only(), b"me".to_vec());
        let llm = LlmRequest {
            run_id: "r".into(),
            agent_id: "bot".into(),
            capability: "alpha".into(),
            prompt_hash: prompt_hash.to_vec(),
            channel_id: "general".into(),
            anchor_seq: 1,
            job_id: None,
            context_hash: Vec::new(),
            transcript: vec![message(1, AuthorRef::User(b"h".to_vec()), "hi")],
        };
        let effect_for = |assignee: Option<&[u8]>| {
            Effect(encode_worker_request(&WorkerRequest {
                saga_id: "s".into(),
                attempt: 0,
                spec: agent_interface::encode_llm_request(&llm),
                deadline: None,
                assignee: assignee.map(|a| a.to_vec()),
            }))
        };

        // someone else's lease: claimed but deliberately not run — the skip
        // that turns N spawns per effect into one.
        match worker.run(&effect_for(Some(b"peer"))).await.unwrap() {
            WorkOutcome::Handled(None) => {}
            other => panic!("a foreign lease must be a claimed skip, got {other:?}"),
        }

        // our own lease executes (here: to the clean not-provided error,
        // proving the spawn path was taken).
        match worker.run(&effect_for(Some(b"me"))).await.unwrap() {
            WorkOutcome::Handled(Some(_)) => {}
            other => panic!("an executable lease must produce an op, got {other:?}"),
        }

        // an UNASSIGNED request is an announcement: with the capability not
        // actually provided here, the worker skips instead of claiming work
        // it could not honor.
        match worker.run(&effect_for(None)).await.unwrap() {
            WorkOutcome::Handled(None) => {}
            other => panic!("an unservable announcement must be a skip, got {other:?}"),
        }
    }

    /// a provider whose only job is making resolve() succeed in tests.
    struct StubProvider;
    #[async_trait::async_trait(?Send)]
    impl capability_host::Provider for StubProvider {
        fn capability(&self) -> &str {
            "alpha"
        }
        async fn run(&self, _prompt: &str) -> Result<String, String> {
            Ok("stub".into())
        }
    }

    #[tokio::test]
    async fn an_unassigned_request_this_host_provides_is_claimed_not_run() {
        use saga_interface::{SagaMsg, WorkerRequest, encode_worker_request};

        let blobs = files::BlobHandle::default();
        let spec = capability_host::CapabilitySpec::parse(
            r#"
spec = 1
[capability]
tag = "alpha"
[detect]
bin = "alpha-cli"
[invoke]
args = []
prompt = "stdin"
[output]
format = "text"
"#,
            "test",
        )
        .expect("mock spec parses");
        let providers = capability_host::ProviderSet::assemble(
            capability_host::SpecSet::from_specs(vec![spec]),
            vec![Box::new(StubProvider)],
        );
        let worker = LlmWorker::new(blobs, providers, b"me".to_vec());
        let llm = LlmRequest {
            run_id: "r".into(),
            agent_id: "bot".into(),
            capability: "alpha".into(),
            prompt_hash: vec![0; 32],
            channel_id: "general".into(),
            anchor_seq: 1,
            job_id: None,
            context_hash: Vec::new(),
            transcript: Vec::new(),
        };
        let effect = Effect(encode_worker_request(&WorkerRequest {
            saga_id: "s".into(),
            attempt: 3,
            spec: agent_interface::encode_llm_request(&llm),
            deadline: None,
            assignee: None,
        }));
        match worker.run(&effect).await.unwrap() {
            WorkOutcome::Handled(Some(msg)) => {
                match saga_interface::decode_msg(&msg.payload).unwrap() {
                    SagaMsg::Accept { saga_id, attempt } => {
                        assert_eq!(saga_id, "s");
                        assert_eq!(attempt, 3);
                    }
                    other => panic!("expected an Accept claim, got {other:?}"),
                }
            }
            other => panic!("a claimable announcement must produce an op, got {other:?}"),
        }
    }

    /// live end-to-end against a REAL locally installed CLI (BYO auth). ignored
    /// by default; name the capability tag your host provides:
    /// `DUCKTAPE_LIVE_CAPABILITY=<tag> cargo test -p agent-oracle -- --ignored live_run`.
    #[tokio::test]
    #[ignore]
    async fn live_run_uses_a_local_cli() {
        let capability = std::env::var("DUCKTAPE_LIVE_CAPABILITY")
            .expect("set DUCKTAPE_LIVE_CAPABILITY to a capability this host provides");
        let blobs = files::BlobHandle::default();
        let prompt_hash = blobs.put_chunk(b"Reply with a tiny JSON AgentOutput.".to_vec());
        let worker = LlmWorker::new(
            blobs,
            capability_host::discover().expect("capability specs load"),
            b"live".to_vec(),
        );
        let llm = LlmRequest {
            run_id: "live".into(),
            agent_id: "bot".into(),
            capability,
            prompt_hash: prompt_hash.to_vec(),
            channel_id: "general".into(),
            anchor_seq: 1,
            job_id: None,
            context_hash: Vec::new(),
            transcript: vec![message(1, AuthorRef::User(b"human".to_vec()), "say hi")],
        };
        let _ = worker.answer(&llm).await.unwrap();
    }

    fn message(seq: u64, author: AuthorRef, text: &str) -> MessageView {
        MessageView {
            channel_id: "general".into(),
            seq,
            head: MessageHead {
                message_id: format!("m{seq}"),
                author,
                blocks: vec![Block::Paragraph(vec![Span::plain(text)])],
                created_at: seq,
                rev: 0,
                edited_at: None,
                base_rev: None,
                deleted: false,
                thread: None,
                reply_count: 0,
                last_reply_seq: None,
            },
            reactions: Vec::<ReactionSummary>::new(),
            channel_head_seq: seq,
        }
    }
}
