//! who this run is, and what it is allowed to do.
//!
//! the environment carries exactly two facts — which node, which agent — and
//! NOTHING about the grant. owner, `allowed_actions` and `ResourceCaps` are
//! read back from the committed agent registry by `agent_id`, so the gate this
//! module enforces is always the grant consensus actually holds. duplicating
//! the caps into the environment would have been one fewer round-trip and one
//! more thing that can silently disagree with the chain.
//!
//! ## what the gate is, and is not
//!
//! it reuses `agent::AgentRecord::permits` and `agent::KNOWN_ACTIONS` — the
//! SAME vocabulary and the SAME predicate the runs module applies on-chain to a
//! response's actions. so the tool plane hands an agent nothing its registered
//! grant did not already hand it, and there is one definition of "allowed", not
//! two that can drift.
//!
//! but it runs HOST-side, in this process, before the submit. that makes it:
//!
//! - a REAL boundary under codex, whose `--sandbox workspace-write` disables
//!   network: this server, spawned by the runner outside that sandbox, is the
//!   only route the run has to the node at all.
//! - a GUARDRAIL under claude, which sandboxes nothing: that run could always
//!   have curl'd `/v1/submit` directly and claimed any origin. the node's own
//!   `origin_guard` says as much. this plane opens no hole that was not already
//!   open; it makes the ambient capability explicit, gated by the agent's own
//!   committed grant, and routed through one auditable binary.
//!
// ponytail: host-side enforcement, ceiling named above. the real fix is a
// `RunAction { saga_id, action }` op that the runs module validates in
// consensus against the saga's committed lease-holder, reusing response.rs's
// validator verbatim — but that only becomes worth building when /v1/submit
// grows real submitter auth (the standing blocker on #235). building it before
// then would add a consensus op whose authorization still rested on an ambient
// port.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use agent::{AgentRecord, CapRequest};
use saga::SagaOrigin;
use serde_json::json;

use crate::node::{Node, NodeError, Result};

pub const ENV_NODE: &str = "DUCKTAPE_NODE";
pub const ENV_AGENT: &str = "DUCKTAPE_RUN_AGENT";
pub const ENV_WORKSPACE: &str = "DUCKTAPE_RUN_WORKSPACE";
pub const ENV_SKILLS: &str = "DUCKTAPE_RUN_SKILLS";

/// the agent-registry module id. the node's genesis registers it under this
/// name (`bin/noded/src/main.rs`), as it does every module the tools speak to.
pub const TARGET_AGENT: &str = "agent";

/// this run, as the tool plane sees it: the node it talks to, and the agent
/// whose grant every write is checked against.
pub struct Run {
    pub node: Node,
    /// `None` when `DUCKTAPE_RUN_AGENT` is unset — this server was started
    /// outside a provisioned run. reads still work (they are ungated except
    /// where caps name the resource); every WRITE refuses, because there is no
    /// grant to check it against and no owner to attribute it to.
    pub agent_id: Option<String>,
    pub workspace: Option<String>,
    pub skills: Option<String>,
    /// monotonic within the process — the tail of every minted id.
    ids: AtomicU64,
}

impl Run {
    /// read the run out of the environment. never fails: a missing variable
    /// degrades the affected tools, it does not stop the server. a runner whose
    /// MCP server dies at launch is a worse failure than one whose tools say,
    /// in words the model can read, that they are unbound.
    pub fn from_env() -> Self {
        Self {
            node: Node::new(std::env::var(ENV_NODE).ok().filter(|s| !s.is_empty())),
            agent_id: std::env::var(ENV_AGENT).ok().filter(|s| !s.is_empty()),
            workspace: std::env::var(ENV_WORKSPACE).ok().filter(|s| !s.is_empty()),
            skills: std::env::var(ENV_SKILLS).ok().filter(|s| !s.is_empty()),
            ids: AtomicU64::new(0),
        }
    }

    /// the agent's COMMITTED record. fetched per call rather than cached at
    /// startup: an owner can pause an agent or narrow its caps mid-run, and a
    /// cached grant would keep honouring a permission that consensus has
    /// already taken away.
    pub fn record(&self) -> Result<AgentRecord> {
        let agent_id = self.agent_id.as_deref().ok_or_else(|| {
            NodeError::Rejected(format!(
                "this MCP server was started without {ENV_AGENT}, so it is not acting for any \
                 agent and cannot write"
            ))
        })?;
        let reply = self
            .node
            .query(TARGET_AGENT, json!({"agent": {"agent_id": agent_id}}))?;
        // AgentReply::Agent(Option<AgentRecord>) — snake_case externally
        // tagged, so the record sits under "agent" and is null for an id the
        // registry does not hold.
        let record = reply.get("agent").ok_or_else(|| {
            NodeError::Transport(format!(
                "the agent registry answered a shape this server does not understand: {reply}"
            ))
        })?;
        if record.is_null() {
            return Err(NodeError::Rejected(format!(
                "the agent registry holds no agent {agent_id:?}"
            )));
        }
        serde_json::from_value(record.clone()).map_err(|e| {
            NodeError::Transport(format!("the agent registry's record did not decode: {e}"))
        })
    }

    /// the gate every write passes: the action must be in the agent's granted
    /// `allowed_actions`, and each extra [`CapRequest`] must be permitted by
    /// its `ResourceCaps`. returns the submit origin (the owner) on success —
    /// so a caller CANNOT get an origin to submit with without having passed
    /// the gate. that is the point of the return type: there is no way to
    /// spell "submit but skip the check".
    pub fn authorize(&self, record: &AgentRecord, action: &str, caps: &[CapRequest]) -> Result<String> {
        if !record.allowed_actions.iter().any(|a| a == action) {
            return Err(NodeError::Rejected(format!(
                "agent {:?} was not granted the {action:?} action (it holds: {})",
                record.agent_id,
                granted(record)
            )));
        }
        for cap in caps {
            if !record.permits(cap) {
                return Err(NodeError::Rejected(format!(
                    "agent {:?} holds the {action:?} action but its resource caps do not cover \
                     {}",
                    record.agent_id,
                    describe(cap)
                )));
            }
        }
        owner_origin(&record.owner)
    }

    /// a read-side cap probe: same predicate, no action name, no origin.
    pub fn permits(&self, record: &AgentRecord, cap: &CapRequest) -> Result<()> {
        record.permits(cap).then_some(()).ok_or_else(|| {
            NodeError::Rejected(format!(
                "agent {:?}'s resource caps do not cover {}",
                record.agent_id,
                describe(cap)
            ))
        })
    }

    /// a fresh id for a client-minted key (a chat message, a task, a pages
    /// thread/comment). unique by construction within a process, and across
    /// processes by the nanosecond stamp.
    ///
    /// deliberately NOT the runs module's deterministic derivation: those ids
    /// must be identical on every replaying validator, because the op is minted
    /// IN consensus. this op is minted host-side by one process and submitted
    /// once, so it needs uniqueness, not reproducibility. a collision is not
    /// silent — the module rejects a squatted id and the agent sees why.
    // ponytail: nanos+counter, no hashing, no uuid dep. two servers minting in
    // the same nanosecond on one node would collide; the module rejects that
    // loudly rather than corrupting anything, and a real uuid is the upgrade if
    // it ever actually happens.
    pub fn mint(&self, kind: &str) -> String {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let n = self.ids.fetch_add(1, Ordering::Relaxed);
        format!("mcp/{kind}/{nanos:x}/{n}")
    }
}

fn granted(record: &AgentRecord) -> String {
    if record.allowed_actions.is_empty() {
        return "no actions at all".into();
    }
    record.allowed_actions.join(", ")
}

/// a cap request in the words the agent's own grant uses, so a refusal names
/// the field its owner would have to widen.
fn describe(cap: &CapRequest) -> String {
    match cap {
        CapRequest::ForgeRead(r) => format!("reading forge repo {r:?} (caps.forge_read)"),
        CapRequest::ForgePush(r) => format!("pushing to forge repo {r:?} (caps.forge_push)"),
        CapRequest::DuckfsRead(p) => format!("reading duckfs path {p:?} (caps.duckfs_read)"),
        CapRequest::DuckfsWrite(p) => format!("writing duckfs path {p:?} (caps.duckfs_write)"),
        CapRequest::Tool(t) => format!("invoking tool {t:?} (caps.tools)"),
        CapRequest::Secret(s) => format!("resolving secret {s:?} (caps.secrets)"),
        CapRequest::PagesWrite(p) => format!("writing page {p:?} (caps.pages_write)"),
        CapRequest::SpawnSubagent => "spawning a sub-agent (caps.subagent_budget)".into(),
    }
}

/// the owner half of D2 attribution, as the `origin` string `/v1/submit`
/// stamps into `Origin::External`.
///
/// only an EXTERNAL owner can be spoken for here: a module-owned agent's writes
/// are the module's to emit in consensus, and this process cannot forge a
/// module origin (nor should it be able to). a non-utf8 external key cannot
/// round-trip through the route's `String` origin field, so it refuses LOUDLY
/// rather than falling back to the node's default identity — a silent fallback
/// would file the agent's writes under the operator's name, which is exactly
/// the misattribution this path exists to avoid.
fn owner_origin(owner: &SagaOrigin) -> Result<String> {
    match owner {
        SagaOrigin::External(key) => String::from_utf8(key.clone()).map_err(|_| {
            NodeError::Rejected(
                "this agent's owner key is not valid utf-8, so its writes cannot be attributed \
                 through the node's submit route"
                    .into(),
            )
        }),
        SagaOrigin::Module(m) => Err(NodeError::Rejected(format!(
            "agent is owned by the {m:?} module, whose writes only that module may emit in \
             consensus — a host-side tool cannot act as a module origin"
        ))),
        SagaOrigin::System => Err(NodeError::Rejected(
            "agent is owned by the system origin, which no host-side tool may act as".into(),
        )),
    }
}
