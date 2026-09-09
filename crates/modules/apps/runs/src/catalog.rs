//! The module-owned action catalog: every operation an agent may invoke, with
//! its target/input/result schemas, the authority it needs and the lanes that
//! admit it. The host carries an [`ActionEnvelope`] and a receipt; this module
//! decodes the envelope into a typed [`Operation`], validates and prepares it.
//! Adding an operation is a change here and nowhere in the host binary.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{
    ACTION_CHAT_POST_MESSAGE, ACTION_DUCKFS_WRITE_TEXT, ACTION_JOBS_COMMENT,
    ACTION_COLLABORATION_ACKNOWLEDGE, ACTION_COLLABORATION_SEND, ACTION_MODULES_UPDATE,
    ACTION_PAGES_COMMENT, ACTION_PAGES_SET_CHECKED, ACTION_TASKS_CREATE,
    ACTION_TASKS_UPDATE_STATUS, MAX_DUCKFS_WRITE_TEXT_BYTES, MAX_REQUEST_ID_BYTES,
    ModuleUpdateSpec, ReplyBlock,
};

// ---- the envelope ----------------------------------------------------------------

/// One invocation of a catalog operation, as the host carries it: the operation
/// name, the destination it selects (when its schema takes one) and its input.
/// Both `target` and `input` are opaque to the host; this module owns their
/// schemas. The invocation's identity (`request_id`) travels beside it.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionEnvelope {
    pub operation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Value>,
    #[serde(default)]
    pub input: Value,
}

impl ActionEnvelope {
    pub fn new(operation: impl Into<String>, target: Option<Value>, input: Value) -> Self {
        Self {
            operation: operation.into(),
            target,
            input,
        }
    }

    /// The digest an idempotency key is checked against: a retry with these
    /// exact bytes is the same invocation; anything else is a refused reuse.
    pub fn digest(&self) -> [u8; 32] {
        let mut canonical = serde_json::to_value(self).expect("envelopes serialize");
        canonical.sort_all_objects();
        Sha256::digest(sdk::wire::encode(&canonical)).into()
    }
}

/// A caller-chosen idempotency key: non-empty, bounded, free of the run-key
/// separator (it is hashed into the receipt id, but the bound keeps the
/// envelope itself bounded).
pub fn validate_request_id(request_id: &str) -> Result<(), String> {
    let shaped = !request_id.is_empty()
        && request_id.len() <= MAX_REQUEST_ID_BYTES
        && !request_id.contains(crate::RESERVED_ID_SEPARATOR);
    if !shaped {
        return Err(format!(
            "request_id must be 1..={MAX_REQUEST_ID_BYTES} bytes and contain no reserved separator"
        ));
    }
    Ok(())
}

// ---- content -----------------------------------------------------------------------

/// One typed part of an action's content. Text renders as a paragraph; code
/// keeps its language. A part kind this module does not know is refused by
/// name, never silently dropped.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ContentPart {
    Text {
        text: String,
    },
    Code {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        lang: Option<String>,
    },
}

impl ContentPart {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }
}

/// Content parts as the reply blocks every conversational destination renders.
pub fn content_blocks(content: &[ContentPart]) -> Vec<ReplyBlock> {
    content
        .iter()
        .map(|part| match part {
            ContentPart::Text { text } => ReplyBlock {
                kind: crate::response::REPLY_KIND_PARAGRAPH.into(),
                text: text.clone(),
                lang: None,
            },
            ContentPart::Code { text, lang } => ReplyBlock {
                kind: crate::response::REPLY_KIND_CODE.into(),
                text: text.clone(),
                lang: lang.clone().filter(|lang| !lang.is_empty()),
            },
        })
        .collect()
}

fn content_schema() -> Value {
    json!({
        "type": "array",
        "minItems": 1,
        "description": "Typed content parts. text renders as a paragraph; code keeps an optional lang.",
        "items": {
            "oneOf": [
                {"type": "object", "properties": {"type": {"const": "text"}, "text": {"type": "string"}}, "required": ["type", "text"], "additionalProperties": false},
                {"type": "object", "properties": {"type": {"const": "code"}, "text": {"type": "string"}, "lang": {"type": "string"}}, "required": ["type", "text"], "additionalProperties": false}
            ]
        }
    })
}

// ---- the catalog view --------------------------------------------------------------

/// The authority an operation needs, in the grant vocabulary an owner writes.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Grant {
    /// A fixed action name from [`crate::KNOWN_ACTIONS`].
    Action(String),
    /// Resolved from the run's committed source: chat.post for a chat source,
    /// pages.comment for a Pages source, jobs.comment for a job source.
    Source,
    /// A resource cap on the model record rather than an action name.
    Cap(String),
}

/// Which lanes admit an operation.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LaneKind {
    /// Mid-run, through the session signer and the program's call.
    Live,
    /// In the final response, after the run's output commits.
    Final,
}

/// One catalog entry as [`crate::RunsQuery::Catalog`] answers it.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OperationView {
    pub name: String,
    pub description: String,
    pub grant: Grant,
    /// JSON Schema of the target object; `None` when the operation takes none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Value>,
    /// JSON Schema of the input object.
    pub input: Value,
    /// JSON Schema of the receipt's result.
    pub result: Value,
    pub lanes: Vec<LaneKind>,
    /// Lowercase hex sha256 of this entry without the digest itself; a proposal
    /// is pinned to it so a module swap cannot reinterpret queued payloads.
    pub schema_digest: String,
}

struct Spec {
    name: &'static str,
    description: &'static str,
    grant: Grant,
    target: Option<Value>,
    input: Value,
    result: Value,
    lanes: &'static [LaneKind],
}

impl Spec {
    fn view(self) -> OperationView {
        let mut view = OperationView {
            name: self.name.into(),
            description: self.description.into(),
            grant: self.grant,
            target: self.target,
            input: self.input,
            result: self.result,
            lanes: self.lanes.to_vec(),
            schema_digest: String::new(),
        };
        let mut canonical = serde_json::to_value(&view).expect("catalog views serialize");
        canonical.sort_all_objects();
        view.schema_digest = crate::hex(&Sha256::digest(sdk::wire::encode(&canonical)));
        view
    }
}

fn object(properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

const LIVE_AND_FINAL: &[LaneKind] = &[LaneKind::Live, LaneKind::Final];
const LIVE_ONLY: &[LaneKind] = &[LaneKind::Live];
const FINAL_ONLY: &[LaneKind] = &[LaneKind::Final];

pub const OP_REPLY: &str = "reply";
pub const OP_AGENT_CALL: &str = "agent.call";

fn specs() -> Vec<Spec> {
    vec![
        Spec {
            name: OP_REPLY,
            description: "Reply where this run was called: its chat thread, Pages block or comment thread, or job discussion. Runs resolves the destination from the committed source; a source-less run cannot reply. Chat sources need chat.post, Pages sources pages.comment plus the page in pages_write, job sources jobs.comment.",
            grant: Grant::Source,
            target: None,
            input: object(json!({"content": content_schema()}), &["content"]),
            result: object(
                json!({"destination": {"type": "object"}, "id": {"type": "string"}}),
                &["destination", "id"],
            ),
            lanes: LIVE_AND_FINAL,
        },
        Spec {
            name: ACTION_CHAT_POST_MESSAGE,
            description: "Post to a chat channel. Omit thread to start a new post; name a root message seq to reply in its thread. Requires chat.post_message.",
            grant: Grant::Action(ACTION_CHAT_POST_MESSAGE.into()),
            target: Some(object(
                json!({"channel_id": {"type": "string"}, "thread": {"type": "integer", "description": "Seq of the root message to reply under."}}),
                &["channel_id"],
            )),
            input: object(json!({"content": content_schema()}), &["content"]),
            result: object(
                json!({"channel_id": {"type": "string"}, "thread": {"type": ["integer", "null"]}, "message_id": {"type": "string"}}),
                &["channel_id", "message_id"],
            ),
            lanes: LIVE_AND_FINAL,
        },
        Spec {
            name: ACTION_PAGES_COMMENT,
            description: "Comment on a page or block (target opens a new thread) or continue an existing comment thread (thread_id). Requires pages.comment and the owning page in pages_write.",
            grant: Grant::Action(ACTION_PAGES_COMMENT.into()),
            target: Some(json!({
                "type": "object",
                "oneOf": [
                    {"properties": {"target": {"type": "string", "description": "A page id or block id."}}, "required": ["target"], "additionalProperties": false},
                    {"properties": {"thread_id": {"type": "string", "description": "An existing comment thread."}}, "required": ["thread_id"], "additionalProperties": false}
                ]
            })),
            input: object(json!({"content": content_schema()}), &["content"]),
            result: object(
                json!({"target": {"type": "string"}, "thread_id": {"type": "string"}, "comment_id": {"type": "string"}}),
                &["target", "thread_id", "comment_id"],
            ),
            lanes: LIVE_AND_FINAL,
        },
        Spec {
            name: ACTION_PAGES_SET_CHECKED,
            description: "Tick or untick a todo block. Requires pages.set_checked and the owning page in pages_write.",
            grant: Grant::Action(ACTION_PAGES_SET_CHECKED.into()),
            target: Some(object(json!({"block_id": {"type": "string"}}), &["block_id"])),
            input: object(json!({"checked": {"type": "boolean"}}), &["checked"]),
            result: object(
                json!({"block_id": {"type": "string"}, "checked": {"type": "boolean"}}),
                &["block_id", "checked"],
            ),
            lanes: LIVE_AND_FINAL,
        },
        Spec {
            name: ACTION_JOBS_COMMENT,
            description: "Comment on a job's discussion. Requires jobs.comment.",
            grant: Grant::Action(ACTION_JOBS_COMMENT.into()),
            target: Some(object(json!({"job_id": {"type": "string"}}), &["job_id"])),
            input: object(json!({"content": content_schema()}), &["content"]),
            result: object(
                json!({"job_id": {"type": "string"}, "comment_id": {"type": "string"}}),
                &["job_id", "comment_id"],
            ),
            lanes: LIVE_AND_FINAL,
        },
        Spec {
            name: ACTION_TASKS_CREATE,
            description: "Create a task. Omit task_id and Runs derives one from the run. Requires tasks.create.",
            grant: Grant::Action(ACTION_TASKS_CREATE.into()),
            target: None,
            input: object(
                json!({"title": {"type": "string"}, "task_id": {"type": "string", "description": "Optional caller-chosen id; must be free."}}),
                &["title"],
            ),
            result: object(json!({"task_id": {"type": "string"}}), &["task_id"]),
            lanes: LIVE_AND_FINAL,
        },
        Spec {
            name: ACTION_TASKS_UPDATE_STATUS,
            description: "Move a task to open, in_progress or done. Requires tasks.update_status.",
            grant: Grant::Action(ACTION_TASKS_UPDATE_STATUS.into()),
            target: Some(object(json!({"task_id": {"type": "string"}}), &["task_id"])),
            input: object(
                json!({"status": {"type": "string", "enum": ["open", "in_progress", "done"]}}),
                &["status"],
            ),
            result: object(
                json!({"task_id": {"type": "string"}, "status": {"type": "string"}}),
                &["task_id", "status"],
            ),
            lanes: LIVE_AND_FINAL,
        },
        Spec {
            name: ACTION_DUCKFS_WRITE_TEXT,
            description: "Write one small UTF-8 text file in the shared filesystem (duckfs). base_snapshot is the snapshot the write was read against (files' own per-path compare-and-set); omit it to require that the path is new. Requires duckfs.write_text and a duckfs_write prefix containing the path.",
            grant: Grant::Action(ACTION_DUCKFS_WRITE_TEXT.into()),
            target: Some(object(json!({"path": {"type": "string", "description": "Absolute duckfs path."}}), &["path"])),
            input: object(
                json!({"text": {"type": "string", "maxLength": MAX_DUCKFS_WRITE_TEXT_BYTES}, "base_snapshot": {"type": "string"}}),
                &["text"],
            ),
            result: object(
                json!({"path": {"type": "string"}, "base_snapshot": {"type": ["string", "null"]}}),
                &["path"],
            ),
            lanes: LIVE_AND_FINAL,
        },
        Spec {
            name: ACTION_MODULES_UPDATE,
            description: "Request deployment of a module artifact committed in this run's forge output. Final response only: Runs binds the artifact to the host-pushed commit. Requires modules.update.",
            grant: Grant::Action(ACTION_MODULES_UPDATE.into()),
            target: None,
            input: object(
                json!({
                    "module_id": {"type": "string"},
                    "artifact": {"type": "string", "description": "Path relative to the forge checkout."},
                    "code_hash": {"type": "string", "description": "Lowercase SHA-256 of the artifact file."},
                    "after": {"type": "integer", "description": "Activation lead in blocks."}
                }),
                &["module_id", "artifact", "code_hash", "after"],
            ),
            result: object(
                json!({"module_id": {"type": "string"}, "code_hash": {"type": "string"}}),
                &["module_id", "code_hash"],
            ),
            lanes: FINAL_ONLY,
        },
        Spec {
            name: ACTION_COLLABORATION_SEND,
            description: "Send one message in a collaboration conversation, as a participant this run's account is BOUND to. Live lane only: the message reaches collaboration as this account's program origin, and that module refuses it unless the participant's owner bound this account to the conversation under `credential`. Sequence is yours to choose and must be monotonic per credential; resending identical bytes under the same sequence is the same message, not a second one. Requires collaboration.send.",
            grant: Grant::Action(ACTION_COLLABORATION_SEND.into()),
            target: Some(object(
                json!({
                    "conversation_id": {"type": "string"},
                    "participant_id": {"type": "string", "description": "The participant this account is bound to, and the sender."}
                }),
                &["conversation_id", "participant_id"],
            )),
            input: object(
                json!({
                    "credential": {"type": "integer", "description": "The binding's credential; also the generation half of the message id."},
                    "sequence": {"type": "integer", "description": "Monotonic within this credential."},
                    "recipient_participant_id": {"type": "string"},
                    "kind": {"type": "string", "enum": ["notice", "question", "task_request", "task_update", "result"]},
                    "body": {"type": "string"},
                    "expires_at": {"type": "integer", "description": "ABSOLUTE consensus time; the network's unit, not seconds."},
                    "reply_to": {"type": ["integer", "null"], "description": "Conversation sequence this answers."},
                    "task": {"type": ["object", "null"], "properties": {"id": {"type": "string"}, "expected_attempt": {"type": "integer", "minimum": 0}}, "required": ["id", "expected_attempt"], "additionalProperties": false, "description": "Current task attempt; required for task_update."}
                }),
                &["credential", "sequence", "recipient_participant_id", "kind", "body", "expires_at"],
            ),
            result: object(
                json!({
                    "conversation_id": {"type": "string"},
                    "credential": {"type": "integer"},
                    "sequence": {"type": "integer"}
                }),
                &["conversation_id", "credential", "sequence"],
            ),
            lanes: LIVE_ONLY,
        },
        Spec {
            name: ACTION_COLLABORATION_ACKNOWLEDGE,
            description: "Record what happened to a message this bound participant received: queued, adapter_accepted, held, refused, delivery_unknown. Live lane only, same binding rule as collaboration.send. Which participant is reporting is NOT stated here — collaboration reads it off the binding this account holds. `reason` is a stable snake_case token, never prose. Requires collaboration.acknowledge.",
            grant: Grant::Action(ACTION_COLLABORATION_ACKNOWLEDGE.into()),
            target: Some(object(
                json!({"conversation_id": {"type": "string"}}),
                &["conversation_id"],
            )),
            input: object(
                json!({
                    "credential": {"type": "integer"},
                    "seq": {"type": "integer", "description": "The conversation sequence the message occupies."},
                    "state": {"type": "string", "enum": ["queued", "adapter_accepted", "held", "refused", "delivery_unknown"]},
                    "reason": {"type": ["string", "null"], "description": "A stable snake_case token."}
                }),
                &["credential", "seq", "state"],
            ),
            result: object(
                json!({
                    "conversation_id": {"type": "string"},
                    "seq": {"type": "integer"},
                    "state": {"type": "string"}
                }),
                &["conversation_id", "seq", "state"],
            ),
            lanes: LIVE_ONLY,
        },
        Spec {
            name: OP_AGENT_CALL,
            description: "Call another registered agent while this run is live. The callee runs with caller ∩ callee authority; the root run's subagent_budget bounds concurrent calls. Collect results with the agent.calls query.",
            grant: Grant::Cap("subagent_budget".into()),
            target: Some(object(json!({"agent_id": {"type": "string"}}), &["agent_id"])),
            input: object(
                json!({
                    "instruction": {"type": "string"},
                    "skills": {"type": "array", "items": {"type": "string"}, "description": "Shared-library skill names offered to the callee."}
                }),
                &["instruction"],
            ),
            result: object(
                json!({"delegation_id": {"type": "string"}, "callee_agent_id": {"type": "string"}}),
                &["delegation_id", "callee_agent_id"],
            ),
            lanes: LIVE_ONLY,
        },
    ]
}

/// The catalog, optionally narrowed to operations whose name starts with
/// `filter`.
pub fn catalog(filter: Option<&str>) -> Vec<OperationView> {
    specs()
        .into_iter()
        .filter(|spec| filter.is_none_or(|prefix| spec.name.starts_with(prefix)))
        .map(Spec::view)
        .collect()
}

/// One catalog entry by name.
pub fn operation_view(name: &str) -> Option<OperationView> {
    specs()
        .into_iter()
        .find(|spec| spec.name == name)
        .map(Spec::view)
}

// ---- the decoded operation ---------------------------------------------------------

/// Where a page comment lands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PageAnchor {
    /// A page or block id: the comment opens a new thread there.
    Target(String),
    /// An existing comment thread.
    Thread(String),
}

/// A catalog operation with its target and input decoded against the schema
/// this module owns. Everything downstream (grants, probes, the prepared
/// target message) works on this, never on the envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Operation {
    Reply {
        content: Vec<ContentPart>,
    },
    ChatPost {
        channel_id: String,
        thread: Option<u64>,
        content: Vec<ContentPart>,
    },
    PagesComment {
        anchor: PageAnchor,
        content: Vec<ContentPart>,
    },
    PagesSetChecked {
        block_id: String,
        checked: bool,
    },
    JobsComment {
        job_id: String,
        content: Vec<ContentPart>,
    },
    TasksCreate {
        task_id: Option<String>,
        title: String,
    },
    TasksUpdateStatus {
        task_id: String,
        status: String,
    },
    DuckfsWriteText {
        path: String,
        text: String,
        base_snapshot: Option<String>,
    },
    CollaborationSend {
        conversation_id: String,
        participant_id: String,
        credential: u64,
        sequence: u64,
        recipient_participant_id: String,
        kind: String,
        body: String,
        expires_at: u64,
        reply_to: Option<u64>,
        task: Option<collaboration::TaskRef>,
    },
    CollaborationAcknowledge {
        conversation_id: String,
        credential: u64,
        seq: u64,
        state: String,
        reason: Option<String>,
    },
    ModulesUpdate(ModuleUpdateSpec),
    AgentCall {
        agent_id: String,
        instruction: String,
        skills: Vec<String>,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContentInput {
    content: Vec<ContentPart>,
}

/// Which conversation, and which of the caller's participants it acts as.
/// Neither is an authority: `collaboration` verifies both against the binding
/// the program origin actually holds.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CollaborationTarget {
    conversation_id: String,
    participant_id: String,
}

/// An acknowledgement names only the conversation: the reporting participant
/// is the one this account's binding names, which only `collaboration` can
/// resolve.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CollaborationAckTarget {
    conversation_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CollaborationSendInput {
    credential: u64,
    sequence: u64,
    recipient_participant_id: String,
    kind: String,
    body: String,
    expires_at: u64,
    #[serde(default)]
    reply_to: Option<u64>,
    #[serde(default)]
    task: Option<collaboration::TaskRef>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CollaborationAckInput {
    credential: u64,
    seq: u64,
    state: String,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChatTarget {
    channel_id: String,
    #[serde(default)]
    thread: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
enum PagesCommentTarget {
    #[serde(rename = "target")]
    Target(String),
    #[serde(rename = "thread_id")]
    Thread(String),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BlockTarget {
    block_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckedInput {
    checked: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JobTarget {
    job_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TaskCreateInput {
    title: String,
    #[serde(default)]
    task_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TaskTarget {
    task_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusInput {
    status: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PathTarget {
    path: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TextInput {
    text: String,
    #[serde(default)]
    base_snapshot: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentTarget {
    agent_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CallInput {
    instruction: String,
    #[serde(default)]
    skills: Vec<String>,
}

fn decode_target<T: serde::de::DeserializeOwned>(
    envelope: &ActionEnvelope,
) -> Result<T, String> {
    let Some(target) = &envelope.target else {
        return Err(format!("{} requires a target", envelope.operation));
    };
    serde_json::from_value(target.clone())
        .map_err(|error| format!("{} target: {error}", envelope.operation))
}

fn no_target(envelope: &ActionEnvelope) -> Result<(), String> {
    if envelope.target.is_some() {
        return Err(format!("{} takes no target", envelope.operation));
    }
    Ok(())
}

fn decode_input<T: serde::de::DeserializeOwned>(envelope: &ActionEnvelope) -> Result<T, String> {
    serde_json::from_value(envelope.input.clone())
        .map_err(|error| format!("{} input: {error}", envelope.operation))
}

impl Operation {
    /// Decode one envelope against the catalog. An unknown operation, a
    /// missing or extra target, or an input outside its schema is refused by
    /// name so the caller can correct what it can see.
    pub(crate) fn decode(envelope: &ActionEnvelope) -> Result<Self, String> {
        match envelope.operation.as_str() {
            OP_REPLY => {
                no_target(envelope)?;
                let input: ContentInput = decode_input(envelope)?;
                Ok(Self::Reply {
                    content: input.content,
                })
            }
            ACTION_CHAT_POST_MESSAGE => {
                let target: ChatTarget = decode_target(envelope)?;
                let input: ContentInput = decode_input(envelope)?;
                Ok(Self::ChatPost {
                    channel_id: target.channel_id,
                    thread: target.thread,
                    content: input.content,
                })
            }
            ACTION_PAGES_COMMENT => {
                let target: PagesCommentTarget = decode_target(envelope)?;
                let input: ContentInput = decode_input(envelope)?;
                Ok(Self::PagesComment {
                    anchor: match target {
                        PagesCommentTarget::Target(id) => PageAnchor::Target(id),
                        PagesCommentTarget::Thread(id) => PageAnchor::Thread(id),
                    },
                    content: input.content,
                })
            }
            ACTION_PAGES_SET_CHECKED => {
                let target: BlockTarget = decode_target(envelope)?;
                let input: CheckedInput = decode_input(envelope)?;
                Ok(Self::PagesSetChecked {
                    block_id: target.block_id,
                    checked: input.checked,
                })
            }
            ACTION_JOBS_COMMENT => {
                let target: JobTarget = decode_target(envelope)?;
                let input: ContentInput = decode_input(envelope)?;
                Ok(Self::JobsComment {
                    job_id: target.job_id,
                    content: input.content,
                })
            }
            ACTION_TASKS_CREATE => {
                no_target(envelope)?;
                let input: TaskCreateInput = decode_input(envelope)?;
                Ok(Self::TasksCreate {
                    task_id: input.task_id,
                    title: input.title,
                })
            }
            ACTION_TASKS_UPDATE_STATUS => {
                let target: TaskTarget = decode_target(envelope)?;
                let input: StatusInput = decode_input(envelope)?;
                Ok(Self::TasksUpdateStatus {
                    task_id: target.task_id,
                    status: input.status,
                })
            }
            ACTION_DUCKFS_WRITE_TEXT => {
                let target: PathTarget = decode_target(envelope)?;
                let input: TextInput = decode_input(envelope)?;
                Ok(Self::DuckfsWriteText {
                    path: target.path,
                    text: input.text,
                    base_snapshot: input.base_snapshot,
                })
            }
            ACTION_COLLABORATION_SEND => {
                let target: CollaborationTarget = decode_target(envelope)?;
                let input: CollaborationSendInput = decode_input(envelope)?;
                Ok(Self::CollaborationSend {
                    conversation_id: target.conversation_id,
                    participant_id: target.participant_id,
                    credential: input.credential,
                    sequence: input.sequence,
                    recipient_participant_id: input.recipient_participant_id,
                    kind: input.kind,
                    body: input.body,
                    expires_at: input.expires_at,
                    reply_to: input.reply_to,
                    task: input.task,
                })
            }
            ACTION_COLLABORATION_ACKNOWLEDGE => {
                let target: CollaborationAckTarget = decode_target(envelope)?;
                let input: CollaborationAckInput = decode_input(envelope)?;
                Ok(Self::CollaborationAcknowledge {
                    conversation_id: target.conversation_id,
                    credential: input.credential,
                    seq: input.seq,
                    state: input.state,
                    reason: input.reason,
                })
            }
            ACTION_MODULES_UPDATE => {
                no_target(envelope)?;
                let spec: ModuleUpdateSpec = decode_input(envelope)?;
                Ok(Self::ModulesUpdate(spec))
            }
            OP_AGENT_CALL => {
                let target: AgentTarget = decode_target(envelope)?;
                let input: CallInput = decode_input(envelope)?;
                Ok(Self::AgentCall {
                    agent_id: target.agent_id,
                    instruction: input.instruction,
                    skills: input.skills,
                })
            }
            other => Err(format!(
                "{other:?} is not a catalog operation; discover the catalog to see the names"
            )),
        }
    }

    /// The catalog name this operation was decoded from.
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Reply { .. } => OP_REPLY,
            Self::ChatPost { .. } => ACTION_CHAT_POST_MESSAGE,
            Self::PagesComment { .. } => ACTION_PAGES_COMMENT,
            Self::PagesSetChecked { .. } => ACTION_PAGES_SET_CHECKED,
            Self::JobsComment { .. } => ACTION_JOBS_COMMENT,
            Self::TasksCreate { .. } => ACTION_TASKS_CREATE,
            Self::TasksUpdateStatus { .. } => ACTION_TASKS_UPDATE_STATUS,
            Self::DuckfsWriteText { .. } => ACTION_DUCKFS_WRITE_TEXT,
            Self::CollaborationSend { .. } => ACTION_COLLABORATION_SEND,
            Self::CollaborationAcknowledge { .. } => ACTION_COLLABORATION_ACKNOWLEDGE,
            Self::ModulesUpdate(_) => ACTION_MODULES_UPDATE,
            Self::AgentCall { .. } => OP_AGENT_CALL,
        }
    }

    /// The fixed grant this operation needs, or `None` when it is resolved
    /// from the source (`reply`) or gated by a cap (`agent.call`).
    pub(crate) fn fixed_grant(&self) -> Option<&'static str> {
        match self {
            Self::Reply { .. } | Self::AgentCall { .. } => None,
            other => Some(other.name()),
        }
    }

    /// The schema identity a proposal of this operation is pinned to.
    pub(crate) fn schema_digest(&self) -> String {
        operation_view(self.name())
            .expect("every decoded operation is in the catalog")
            .schema_digest
    }

    /// Whether this operation belongs to the degrade lane on the settle path:
    /// pages annotations and duckfs writes fail alone with a breadcrumb rather
    /// than costing the response its reply.
    pub(crate) fn is_pages(&self) -> bool {
        matches!(self, Self::PagesComment { .. } | Self::PagesSetChecked { .. })
    }

    pub(crate) fn is_duckfs(&self) -> bool {
        matches!(self, Self::DuckfsWriteText { .. })
    }

    /// Whether this operation is a conversational or task write the response
    /// lane prepares (`emit_response`): a reply, a chat post, a job comment, a
    /// task create or status update.
    pub(crate) fn is_conversational(&self) -> bool {
        matches!(
            self,
            Self::Reply { .. }
                | Self::ChatPost { .. }
                | Self::JobsComment { .. }
                | Self::TasksCreate { .. }
                | Self::TasksUpdateStatus { .. }
        )
    }

    /// Whether the operation may run in `lane`.
    pub(crate) fn admits(&self, lane: LaneKind) -> bool {
        operation_view(self.name())
            .expect("every decoded operation is in the catalog")
            .lanes
            .contains(&lane)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope(operation: &str, target: Option<Value>, input: Value) -> ActionEnvelope {
        ActionEnvelope::new(operation, target, input)
    }

    #[test]
    fn every_catalog_entry_decodes_its_own_example_shape() {
        let cases = [
            envelope(
                OP_REPLY,
                None,
                json!({"content": [{"type": "text", "text": "hi"}]}),
            ),
            envelope(
                ACTION_CHAT_POST_MESSAGE,
                Some(json!({"channel_id": "general", "thread": 3})),
                json!({"content": [{"type": "code", "text": "x", "lang": "rs"}]}),
            ),
            envelope(
                ACTION_PAGES_COMMENT,
                Some(json!({"target": "b1"})),
                json!({"content": [{"type": "text", "text": "hi"}]}),
            ),
            envelope(
                ACTION_PAGES_COMMENT,
                Some(json!({"thread_id": "t1"})),
                json!({"content": [{"type": "text", "text": "hi"}]}),
            ),
            envelope(
                ACTION_PAGES_SET_CHECKED,
                Some(json!({"block_id": "b1"})),
                json!({"checked": true}),
            ),
            envelope(
                ACTION_JOBS_COMMENT,
                Some(json!({"job_id": "j1"})),
                json!({"content": [{"type": "text", "text": "hi"}]}),
            ),
            envelope(ACTION_TASKS_CREATE, None, json!({"title": "t"})),
            envelope(
                ACTION_TASKS_UPDATE_STATUS,
                Some(json!({"task_id": "t1"})),
                json!({"status": "done"}),
            ),
            envelope(
                ACTION_DUCKFS_WRITE_TEXT,
                Some(json!({"path": "/shared/x"})),
                json!({"text": "hello", "base_snapshot": "s1"}),
            ),
            envelope(
                ACTION_MODULES_UPDATE,
                None,
                json!({"module_id": "hello", "artifact": "hello.module", "code_hash": "ab".repeat(32), "after": 50}),
            ),
            envelope(
                OP_AGENT_CALL,
                Some(json!({"agent_id": "reviewer"})),
                json!({"instruction": "review", "skills": ["review"]}),
            ),
            envelope(
                ACTION_COLLABORATION_SEND,
                Some(json!({"conversation_id": "c1", "participant_id": "alice"})),
                json!({
                    "credential": 2,
                    "sequence": 1,
                    "recipient_participant_id": "bob",
                    "kind": "notice",
                    "body": "hi",
                    "expires_at": 900
                }),
            ),
            // the acknowledgement names no participant: collaboration reads the
            // reporter off the binding the origin holds.
            envelope(
                ACTION_COLLABORATION_ACKNOWLEDGE,
                Some(json!({"conversation_id": "c1"})),
                json!({"credential": 2, "seq": 4, "state": "queued"}),
            ),
        ];
        let names: Vec<&str> = cases
            .iter()
            .map(|case| {
                Operation::decode(case)
                    .unwrap_or_else(|error| panic!("{}: {error}", case.operation))
                    .name()
            })
            .collect();
        let catalog: Vec<String> = catalog(None).into_iter().map(|view| view.name).collect();
        for name in &catalog {
            assert!(names.contains(&name.as_str()), "no decode case for {name}");
        }
    }

    #[test]
    fn a_decode_refusal_names_the_operation_and_the_field() {
        let unknown = Operation::decode(&envelope("chat.shout", None, json!({}))).unwrap_err();
        assert!(unknown.contains("chat.shout"), "{unknown}");
        let missing_target =
            Operation::decode(&envelope(ACTION_CHAT_POST_MESSAGE, None, json!({}))).unwrap_err();
        assert!(missing_target.contains("requires a target"), "{missing_target}");
        let extra_target = Operation::decode(&envelope(
            OP_REPLY,
            Some(json!({"channel_id": "x"})),
            json!({"content": []}),
        ))
        .unwrap_err();
        assert!(extra_target.contains("takes no target"), "{extra_target}");
        let bad_part = Operation::decode(&envelope(
            OP_REPLY,
            None,
            json!({"content": [{"type": "image", "ref": "x"}]}),
        ))
        .unwrap_err();
        assert!(bad_part.contains("reply input"), "{bad_part}");
        let stray = Operation::decode(&envelope(
            ACTION_TASKS_CREATE,
            None,
            json!({"title": "t", "owner": "me"}),
        ))
        .unwrap_err();
        assert!(stray.contains("owner"), "{stray}");
    }

    #[test]
    fn schema_digests_are_stable_per_entry_and_distinct_across_entries() {
        let views = catalog(None);
        let mut digests: Vec<&str> = views.iter().map(|v| v.schema_digest.as_str()).collect();
        digests.sort_unstable();
        digests.dedup();
        assert_eq!(digests.len(), views.len());
        for view in &views {
            assert_eq!(
                operation_view(&view.name).unwrap().schema_digest,
                view.schema_digest
            );
            assert_eq!(view.schema_digest.len(), 64);
        }
    }

    #[test]
    fn the_filter_is_a_name_prefix() {
        let pages: Vec<String> = catalog(Some("pages."))
            .into_iter()
            .map(|v| v.name)
            .collect();
        assert_eq!(pages, [ACTION_PAGES_COMMENT, ACTION_PAGES_SET_CHECKED]);
        assert!(catalog(Some("nothing.")).is_empty());
    }

    #[test]
    fn every_fixed_grant_is_a_known_action_and_every_known_action_has_an_operation() {
        for view in catalog(None) {
            if let Grant::Action(name) = &view.grant {
                assert!(
                    crate::KNOWN_ACTIONS.contains(&name.as_str()),
                    "{} is gated on an unknown grant {name}",
                    view.name
                );
            }
        }
        // chat.post is the source grant `reply` resolves to; every other
        // known action is a catalog operation of its own name.
        for action in crate::KNOWN_ACTIONS {
            let source_only = action == crate::ACTION_CHAT_POST;
            assert!(
                source_only || operation_view(action).is_some(),
                "no catalog operation for the {action} grant"
            );
        }
    }

    #[test]
    fn envelope_digests_ignore_key_order_and_track_every_field() {
        let a = envelope(
            ACTION_CHAT_POST_MESSAGE,
            Some(json!({"channel_id": "c", "thread": 1})),
            json!({"content": [{"type": "text", "text": "x"}]}),
        );
        let b = envelope(
            ACTION_CHAT_POST_MESSAGE,
            Some(json!({"thread": 1, "channel_id": "c"})),
            json!({"content": [{"text": "x", "type": "text"}]}),
        );
        assert_eq!(a.digest(), b.digest());
        let c = envelope(
            ACTION_CHAT_POST_MESSAGE,
            Some(json!({"channel_id": "c", "thread": 2})),
            json!({"content": [{"type": "text", "text": "x"}]}),
        );
        assert_ne!(a.digest(), c.digest());
    }

    #[test]
    fn request_ids_are_bounded_and_separator_free() {
        assert!(validate_request_id("build-status").is_ok());
        assert!(validate_request_id("").is_err());
        assert!(validate_request_id(&"x".repeat(MAX_REQUEST_ID_BYTES + 1)).is_err());
        assert!(validate_request_id("a\u{1f}b").is_err());
    }
}
