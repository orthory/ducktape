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

use crate::mcp::node::{Node, NodeError, Result};

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
}

/// this run, as the tool plane sees it.
pub struct Run {
    pub node: Node,
    /// `None` when `DUCKTAPE_RUN_AGENT` is unset — this server was started
    /// outside a provisioned run. reads still work; `whoami` says so.
    pub agent_id: Option<String>,
    pub workspace: Option<String>,
    pub skills: Option<String>,
    /// the CONSENSUS run id this server is bound to, from `DUCKTAPE_RUN_ID`.
    /// exported for every provisioned run, session or not: it is identity, not
    /// a credential, and the read plane needs it to fetch the run's ceiling.
    run_id: Option<String>,
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
            run_id: std::env::var(ENV_RUN_ID).ok().filter(|s| !s.is_empty()),
            action: ActionControl::from_env(),
            provider_control: ProviderControl::from_env(),
            ids: AtomicU64::new(0),
        }
    }

    /// the run this MCP session is bound to. The signer stays private; callers
    /// that only need an evidence id never get access to the session key.
    pub fn run_id(&self) -> Option<&str> {
        self.run_id.as_deref()
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
                "this server is not bound to a run ({ENV_RUN_ID} is unset)"
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

    /// the agent's COMMITTED record, NARROWED to this run's admission ceiling.
    ///
    /// fetched per call rather than cached at startup: an owner can pause an
    /// agent or narrow its caps mid-run, and a cached grant would keep
    /// honouring a permission that consensus has already taken away.
    ///
    /// the standing record is only half of it. a DELEGATED run carries the
    /// caller's frozen grant as a ceiling, which consensus applies to every
    /// write (`runs`' `agent_for_run`); gating reads on the standing record
    /// alone would let a peer's agent read whatever ITS owner granted it, on
    /// behalf of a caller who granted far less. so the ceiling is fetched with
    /// the record and applied the same way — and FAIL CLOSED: a query this
    /// server cannot complete, or a run the module no longer holds, refuses the
    /// read. falling back to the standing record is exactly the escalation.
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
        let standing: AgentRecord = serde_json::from_value(record.clone()).map_err(|e| {
            NodeError::Transport(format!("the agent registry's record did not decode: {e}"))
        })?;
        let Some(run_id) = self.run_id.as_deref() else {
            return Ok(standing);
        };
        let reply = self
            .node
            .query(TARGET_RUNS, json!({"run_authority": {"run_id": run_id}}))?;
        // RunsReply::RunAuthority(Option<RunAuthorityView>) — externally
        // tagged, so the view sits under "run_authority" and is null for a run
        // the module is not holding.
        let view = reply.get("run_authority").ok_or_else(|| {
            NodeError::Transport(format!(
                "the runs module answered a shape this server does not understand: {reply}"
            ))
        })?;
        if view.is_null() {
            return Err(NodeError::Rejected(format!(
                "run {run_id:?} is not in flight, so its authority cannot be established"
            )));
        }
        let view: runs::RunAuthorityView = serde_json::from_value(view.clone()).map_err(|e| {
            NodeError::Transport(format!("the run's authority did not decode: {e}"))
        })?;
        Ok(match &view.authority {
            Some(ceiling) => ceiling.apply(&standing),
            None => standing,
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
        // the signer is scoped to ONE run and every message it signs names it,
        // so a session with no run id can sign nothing.
        std::env::var(ENV_RUN_ID)
            .ok()
            .filter(|value| !value.is_empty())?;
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(60))
            .build()
            .expect("a loopback action client always builds");
        Some(Self { client, url, token })
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
        && matches!(
            url.host_str(),
            Some("127.0.0.1" | "ducktape-host" | "host.containers.internal")
        )
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
        && matches!(
            url.host_str(),
            Some("127.0.0.1" | "ducktape-host" | "host.containers.internal")
        )
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
        assert!(provider_control_url_allowed(
            "http://host.containers.internal:41043/v1/control/provider-idle"
        ));
        for rejected in [
            "https://127.0.0.1:41043/v1/control/provider-idle",
            "http://localhost:41043/v1/control/provider-idle",
            "http://example.com:41043/v1/control/provider-idle",
            "http://127.0.0.1:41043/v1/control/provider-idle?token=leak",
            "http://127.0.0.1:41043/other",
        ] {
            assert!(
                !provider_control_url_allowed(rejected),
                "accepted {rejected}"
            );
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
        CapRequest::SpawnSubagent => "calling a peer agent (caps.subagent_budget)".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// a node that answers `/v1/query` from a canned table: the agent registry
    /// arm, then the runs `run_authority` arm. one thread, `n` requests, no
    /// framework — the whole point is to watch `record()` make BOTH queries.
    fn fake_node(replies: Vec<serde_json::Value>) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for reply in replies {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                // drain enough of the request to unblock the client, then answer.
                let mut buf = [0u8; 4096];
                let _ = std::io::Read::read(&mut stream, &mut buf);
                let body = reply.to_string();
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
            }
        });
        format!("http://127.0.0.1:{port}")
    }

    fn standing_record() -> AgentRecord {
        let mut record = agent::AgentRecord {
            agent_id: "worker".into(),
            owner: saga::SagaOrigin::External(vec![9; 32]),
            display_name: "Worker".into(),
            capability: "model-1".into(),
            allowed_actions: vec![agent::ACTION_CHAT_POST.into()],
            status: agent::AgentStatus::Active,
            role: agent::AgentRole::General,
            created_at: 0,
            updated_at: 0,
            recipe_hash: Vec::new(),
            caps: agent::ResourceCaps::default(),
            skills: Vec::new(),
        };
        record.caps.duckfs_read = vec!["/shared".into()];
        record
    }

    fn bound_run(node: String, replies: Vec<serde_json::Value>) -> Run {
        Run {
            node: Node::new(Some(fake_node(replies))),
            agent_id: Some("worker".into()),
            workspace: None,
            skills: None,
            run_id: Some(node),
            action: None,
            provider_control: None,
            ids: AtomicU64::new(0),
        }
    }

    #[test]
    fn a_delegated_runs_ceiling_narrows_the_record_the_read_plane_gates_on() {
        let standing = standing_record();
        // the caller granted LESS than the callee's owner did: no duckfs read.
        let ceiling = json!({
            "allowed_actions": [agent::ACTION_CHAT_POST],
            "caps": agent::ResourceCaps::default(),
        });
        let run = bound_run(
            "run-1".into(),
            vec![
                json!({"agent": standing}),
                json!({"run_authority": {
                    "run_id": "run-1", "agent_id": "worker", "authority": ceiling
                }}),
            ],
        );
        let record = run.record().expect("both queries answer");
        assert!(
            record.caps.duckfs_read.is_empty(),
            "the ceiling, not the standing grant: {:?}",
            record.caps.duckfs_read
        );
        assert!(
            !record.permits(&CapRequest::DuckfsRead("/shared/notes")),
            "a read the standing record allows is refused under the ceiling"
        );
        // and the standing record really did allow it.
        assert!(standing.permits(&CapRequest::DuckfsRead("/shared/notes")));
    }

    #[test]
    fn an_ordinary_run_keeps_its_standing_record() {
        let run = bound_run(
            "run-1".into(),
            vec![
                json!({"agent": standing_record()}),
                json!({"run_authority": {
                    "run_id": "run-1", "agent_id": "worker", "authority": null
                }}),
            ],
        );
        let record = run.record().expect("both queries answer");
        assert!(record.permits(&CapRequest::DuckfsRead("/shared/notes")));
    }

    #[test]
    fn a_ceiling_the_server_cannot_establish_refuses_the_read() {
        // the authority query answers a shape this server does not understand
        // (a node mid-restart, a wire drift) — FAIL CLOSED: never fall back to
        // the standing record, which is exactly the escalation.
        let run = bound_run(
            "run-1".into(),
            vec![json!({"agent": standing_record()}), json!({"nope": 1})],
        );
        assert!(run.record().is_err());

        // a run the module is not holding proves no ceiling either.
        let gone = bound_run(
            "run-1".into(),
            vec![
                json!({"agent": standing_record()}),
                json!({"run_authority": null}),
            ],
        );
        assert!(gone.record().is_err());
    }

    #[test]
    fn action_endpoint_is_strictly_host_local_and_path_scoped() {
        assert!(action_url_allowed("http://127.0.0.1:41043/v1/run-action"));
        assert!(action_url_allowed(
            "http://ducktape-host:41043/v1/run-action"
        ));
        assert!(action_url_allowed(
            "http://host.containers.internal:41043/v1/run-action"
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
