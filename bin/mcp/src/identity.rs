//! who this run is, and what it may do.
//!
//! the environment carries which node and agent, plus a narrow host endpoint
//! for actions made by this run. The session private key never enters the child.
//! it carries NOTHING about the grant: owner, `allowed_actions` and
//! `ResourceCaps` are read back from the committed agent registry, so what this
//! module reports is always what consensus actually holds.
//!
//! ## writes are gated in CONSENSUS, not here
//!
//! this binary does not decide whether a write is allowed. it asks the scoped
//! host endpoint to sign an allowed runs message; the runs module then checks —
//! on every validator — that the origin IS
//! the session key bound to that run, that the run is still in flight, and that
//! the action sits inside the agent's committed `allowed_actions` and caps. a
//! refusal comes back as the module's own words.
//!
//! a frame's origin is its verified public key. The endpoint accepts only
//! `AgentAction` and `DelegateRun` for its exact run id, so its bearer token is
//! not a general-purpose signer even if the child reads its environment.
//!
//! READS are still gated here, against the committed caps (`forge_read`,
//! `duckfs_read`) — they cross no consensus op to be checked by, and `/v1/query`
//! is ambient to any local process anyway.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use agent::{AgentRecord, CapRequest};
use serde_json::json;

use crate::node::{Node, NodeError, Result};

pub const ENV_NODE: &str = "DUCKTAPE_NODE";
pub const ENV_AGENT: &str = "DUCKTAPE_RUN_AGENT";
pub const ENV_WORKSPACE: &str = "DUCKTAPE_RUN_WORKSPACE";
pub const ENV_SKILLS: &str = "DUCKTAPE_RUN_SKILLS";
pub const ENV_ACTION_URL: &str = "DUCKTAPE_RUN_ACTION_URL";
pub const ENV_ACTION_TOKEN: &str = "DUCKTAPE_RUN_ACTION_TOKEN";
/// the run this session is bound to — the `run_id` every `AgentAction` names.
pub const ENV_RUN_ID: &str = "DUCKTAPE_RUN_ID";
const ENV_PROVIDER_CONTROL_URL: &str = "DUCKTAPE_PROVIDER_CONTROL_URL";
const ENV_PROVIDER_CONTROL_TOKEN: &str = "DUCKTAPE_PROVIDER_CONTROL_TOKEN";
const PROVIDER_CONTROL_HEADER: &str = "x-ducktape-provider-control";

/// the agent-registry module id. the node's genesis registers it under this
/// name (`bin/noded/src/main.rs`), as it does every module the tools speak to.
pub const TARGET_AGENT: &str = "agent";
/// the runs module id — the target of every `AgentAction`, and the only module
/// this binary ever WRITES to. chat, tasks and pages are written by runs, in
/// consensus, on the agent's behalf; that indirection is what earns the write
/// its `AuthorRef::Agent` attribution.
pub const TARGET_RUNS: &str = "runs";

/// The narrow host signer endpoint for this live run. Its random token can ask
/// for Runs agent actions/calls only; no general-purpose private key crosses
/// into this process.
struct ActionControl {
    client: reqwest::blocking::Client,
    url: String,
    token: String,
    pub run_id: String,
}

/// this run, as the tool plane sees it.
pub struct Run {
    pub node: Node,
    /// `None` when `DUCKTAPE_RUN_AGENT` is unset — this server was started
    /// outside a provisioned run. reads still work; `whoami` says so.
    pub agent_id: Option<String>,
    pub workspace: Option<String>,
    pub skills: Option<String>,
    /// `None` when the node opened no session for this run (an older node, or a
    /// run whose `OpenAgentSession` was refused). every WRITE then refuses,
    /// loudly — there is no credential to prove the write came from this agent,
    /// and this binary will not fall back to a lane that would file it under
    /// somebody else's name.
    action: Option<ActionControl>,
    provider_control: Option<ProviderControl>,
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
            action: ActionControl::from_env(),
            provider_control: ProviderControl::from_env(),
            ids: AtomicU64::new(0),
        }
    }

    /// the run this MCP session is bound to. The signer stays private; callers
    /// that only need an evidence id never get access to the session key.
    pub fn run_id(&self) -> Option<&str> {
        self.action.as_ref().map(|action| action.run_id.as_str())
    }

    /// Apply one action mid-run through this run's scoped host signer.
    ///
    /// there is NO permission check here. the runs module makes it, on every
    /// validator, against the agent's committed grant — and its refusal is what
    /// comes back. a second gate in this process could only ever drift from the
    /// one that actually decides.
    pub fn act(&self, action: agent::AgentAction) -> Result<serde_json::Value> {
        self.submit_runs(runs::RunsMsg::AgentAction {
            run_id: self.run_id().unwrap_or_default().to_string(),
            action,
        })
    }

    pub fn delegate(
        &self,
        request_id: String,
        request: agent::DelegationRequest,
    ) -> Result<serde_json::Value> {
        self.submit_runs(runs::RunsMsg::DelegateRun {
            run_id: self.run_id().unwrap_or_default().to_string(),
            request_id,
            request,
        })
    }

    pub fn delegations(&self) -> Result<serde_json::Value> {
        let run_id = self.run_id().ok_or_else(|| {
            NodeError::Rejected(format!(
                "this run has no scoped action endpoint ({ENV_ACTION_URL} is unset)"
            ))
        })?;
        self.node.query(
            TARGET_RUNS,
            json!({"delegations": {"caller_run_id": run_id}}),
        )
    }

    fn submit_runs(&self, message: runs::RunsMsg) -> Result<serde_json::Value> {
        self.action
            .as_ref()
            .ok_or_else(|| {
                NodeError::Rejected(format!(
                    "this run has no scoped action endpoint ({ENV_ACTION_URL} is unset), so writing is refused"
                ))
            })?
            .submit(message)
    }

    /// Ask the host-local controller for more silent provider time. The model
    /// supplies no identity or credential: both arrive as ambient run env and
    /// the broker rotates them for every child invocation.
    pub fn extend_provider_idle(
        &self,
        request_id: String,
        requested_secs: u64,
    ) -> Result<serde_json::Value> {
        let Some(control) = &self.provider_control else {
            return Ok(json!({"status":"denied", "reason":"unavailable"}));
        };
        control.request(request_id, requested_secs)
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

    /// a read-side cap probe. reads cross no consensus op that could check them,
    /// so this is the only gate they get — and it is honest about being one: the
    /// node's `/v1/query` is ambient to any local process. under codex's
    /// network-less sandbox this server IS the only door and the probe is a real
    /// boundary; under claude it is a guardrail on a surface the run could reach
    /// anyway.
    ///
    /// WRITES do not come through here. they are gated in consensus — see
    /// [`Run::act`] and this module's doc.
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

const ACTION_HEADER: &str = "x-ducktape-run-action";

impl ActionControl {
    fn from_env() -> Option<Self> {
        let url = std::env::var(ENV_ACTION_URL)
            .ok()
            .filter(|value| action_url_allowed(value))?;
        let token = std::env::var(ENV_ACTION_TOKEN)
            .ok()
            .filter(|value| provider_control_token_allowed(value))?;
        let run_id = std::env::var(ENV_RUN_ID)
            .ok()
            .filter(|value| !value.is_empty())?;
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(60))
            .build()
            .expect("a loopback action client always builds");
        Some(Self {
            client,
            url,
            token,
            run_id,
        })
    }

    fn submit(&self, message: runs::RunsMsg) -> Result<serde_json::Value> {
        let response = self
            .client
            .post(&self.url)
            .header(ACTION_HEADER, &self.token)
            .json(&json!({"message": message}))
            .send()
            .map_err(|error| NodeError::Transport(error.to_string()))?;
        let status = response.status();
        let value: serde_json::Value = response.json().map_err(|error| {
            NodeError::Transport(format!("action signer returned invalid json: {error}"))
        })?;
        if status.is_success() {
            Ok(value)
        } else {
            Err(NodeError::Rejected(
                value
                    .get("message")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("scoped action signer rejected the request")
                    .to_string(),
            ))
        }
    }
}

fn action_url_allowed(value: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(value) else {
        return false;
    };
    url.scheme() == "http"
        && matches!(url.host_str(), Some("127.0.0.1" | "ducktape-host"))
        && url.port().is_some()
        && url.path() == "/v1/run-action"
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
}

struct ProviderControl {
    client: reqwest::blocking::Client,
    url: String,
    token: String,
}

impl ProviderControl {
    fn from_env() -> Option<Self> {
        let url = std::env::var(ENV_PROVIDER_CONTROL_URL)
            .ok()
            .filter(|value| !value.is_empty())?;
        let token = std::env::var(ENV_PROVIDER_CONTROL_TOKEN)
            .ok()
            .filter(|value| !value.is_empty())?;
        if !provider_control_url_allowed(&url) || !provider_control_token_allowed(&token) {
            return None;
        }
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(5))
            .build()
            .expect("a loopback-only provider control client always builds");
        Some(Self { client, url, token })
    }

    fn request(&self, request_id: String, requested_secs: u64) -> Result<serde_json::Value> {
        let response = match self
            .client
            .post(&self.url)
            .header(PROVIDER_CONTROL_HEADER, &self.token)
            .json(&json!({
                "request_id": request_id,
                "requested_secs": requested_secs,
            }))
            .send()
        {
            Ok(response) => response,
            Err(_) => {
                return Ok(json!({
                    "status":"denied",
                    "reason":"control_unreachable",
                }));
            }
        };
        response.json().map_err(|error| {
            NodeError::Transport(format!(
                "provider idle controller returned an invalid reply: {error}"
            ))
        })
    }
}

fn provider_control_url_allowed(value: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(value) else {
        return false;
    };
    url.scheme() == "http"
        && matches!(url.host_str(), Some("127.0.0.1" | "ducktape-host"))
        && url.port().is_some()
        && url.path() == "/v1/control/provider-idle"
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
}

fn provider_control_token_allowed(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod provider_control_tests {
    use super::*;

    #[test]
    fn control_endpoint_and_token_are_strictly_host_local() {
        assert!(provider_control_url_allowed(
            "http://127.0.0.1:41043/v1/control/provider-idle"
        ));
        assert!(provider_control_url_allowed(
            "http://ducktape-host:41043/v1/control/provider-idle"
        ));
        for rejected in [
            "https://127.0.0.1:41043/v1/control/provider-idle",
            "http://localhost:41043/v1/control/provider-idle",
            "http://example.com:41043/v1/control/provider-idle",
            "http://127.0.0.1:41043/v1/control/provider-idle?token=leak",
            "http://127.0.0.1:41043/other",
        ] {
            assert!(!provider_control_url_allowed(rejected), "accepted {rejected}");
        }
        assert!(provider_control_token_allowed(&"a5".repeat(32)));
        assert!(!provider_control_token_allowed(&"A5".repeat(32)));
        assert!(!provider_control_token_allowed("short"));
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_endpoint_is_strictly_host_local_and_path_scoped() {
        assert!(action_url_allowed("http://127.0.0.1:41043/v1/run-action"));
        assert!(action_url_allowed(
            "http://ducktape-host:41043/v1/run-action"
        ));
        for rejected in [
            "https://127.0.0.1:41043/v1/run-action",
            "http://localhost:41043/v1/run-action",
            "http://127.0.0.1:41043/v1/submit/frame",
            "http://127.0.0.1:41043/v1/run-action?token=leak",
        ] {
            assert!(!action_url_allowed(rejected), "accepted {rejected}");
        }
    }
}
