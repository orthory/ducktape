//! who this run is, and what it may do — and, for writes, the key that PROVES
//! it.
//!
//! the environment carries which node, which agent, and this run's session key.
//! it carries NOTHING about the grant: owner, `allowed_actions` and
//! `ResourceCaps` are read back from the committed agent registry, so what this
//! module reports is always what consensus actually holds.
//!
//! ## writes are gated in CONSENSUS, not here
//!
//! this binary does not decide whether a write is allowed. it signs
//! `RunsMsg::AgentAction` with the run's session key and submits it as an op
//! frame; the runs module then checks — on every validator — that the origin IS
//! the session key bound to that run, that the run is still in flight, and that
//! the action sits inside the agent's committed `allowed_actions` and caps. a
//! refusal comes back as the module's own words.
//!
//! that is the whole point of the session key. a frame's origin is its VERIFIED
//! public key (`node::decode_frame` binds `(origin, seq, target, payload)`), so
//! an `AgentAction` op is PROOF that this agent's run made it. the frameless
//! `/v1/submit` lane cannot do this: `bin/node` DISCARDS the caller's origin
//! string outright and re-signs with the node key, so an agent write on that
//! lane is indistinguishable from a human's, attributable to the wrong account
//! cross-node, and gated only by whatever the host binary chose to believe.
//!
//! deliberately there is no host-side pre-check duplicating the on-chain gate.
//! two validators drift; one does not.
//!
//! ## the session key is a BEARER CREDENTIAL, and the sandbox is what holds it
//!
//! under codex the agent has a shell and can read this process's environment, so
//! assume it HAS the session key — and assume that means MORE than the action
//! lane. the key signs frames, and a frame can carry any `Msg` to any module.
//! consensus checks the grant on `RunsMsg::AgentAction`; nothing checks it on
//! the key. used directly the key is just an unknown external submitter — which
//! `chat` turns into `AuthorRef::User(key)`, admitted by any `Open` channel with
//! no `chat.post_message` grant in sight — and it stays a valid signer after the
//! run settles, because pruning the session only closes the `AgentAction` lane.
//!
//! it is contained by the codex SANDBOX (no network), not by its own authority:
//! the model's shell cannot reach the node's HTTP lane, and this server — which
//! can — offers only the tools below. the NODE key, by contrast, signs anything
//! at all, which is why the node signs `OpenAgentSession` itself and never lets
//! this process near it.
//!
//! READS are still gated here, against the committed caps (`forge_read`,
//! `duckfs_read`) — they cross no consensus op to be checked by, and `/v1/query`
//! is ambient to any local process anyway.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use agent::{AgentRecord, CapRequest};
use commonware_codec::DecodeExt as _;
use commonware_cryptography::ed25519;
use serde_json::json;

use crate::node::{Node, NodeError, Result};

pub const ENV_NODE: &str = "DUCKTAPE_NODE";
pub const ENV_AGENT: &str = "DUCKTAPE_RUN_AGENT";
pub const ENV_WORKSPACE: &str = "DUCKTAPE_RUN_WORKSPACE";
pub const ENV_SKILLS: &str = "DUCKTAPE_RUN_SKILLS";
/// this run's ed25519 session PRIVATE key, lowercase hex.
pub const ENV_SESSION_KEY: &str = "DUCKTAPE_RUN_SESSION_KEY";
/// the run this session is bound to — the `run_id` every `AgentAction` names.
pub const ENV_RUN_ID: &str = "DUCKTAPE_RUN_ID";

/// the agent-registry module id. the node's genesis registers it under this
/// name (`bin/noded/src/main.rs`), as it does every module the tools speak to.
pub const TARGET_AGENT: &str = "agent";
/// the runs module id — the target of every `AgentAction`, and the only module
/// this binary ever WRITES to. chat, tasks and pages are written by runs, in
/// consensus, on the agent's behalf; that indirection is what earns the write
/// its `AuthorRef::Agent` attribution.
pub const TARGET_RUNS: &str = "runs";

/// this run's write credential: the ed25519 key the node bound to this run in
/// consensus, and the run it is bound to.
pub struct Session {
    signer: ed25519::PrivateKey,
    pub run_id: String,
    /// the frame sequence. a session key is FRESH per run, so this starts at 0
    /// and every op of this run gets a distinct `(origin, seq)` — the replay
    /// identity the ordered lane keys on.
    seq: AtomicU64,
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
    session: Option<Session>,
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
            session: session_from_env(),
            ids: AtomicU64::new(0),
        }
    }

    /// the run this MCP session is bound to. The signer stays private; callers
    /// that only need an evidence id never get access to the session key.
    pub fn run_id(&self) -> Option<&str> {
        self.session.as_ref().map(|session| session.run_id.as_str())
    }

    /// apply one action, mid-run: sign a `RunsMsg::AgentAction` with this run's
    /// session key and submit it as an op frame.
    ///
    /// there is NO permission check here. the runs module makes it, on every
    /// validator, against the agent's committed grant — and its refusal is what
    /// comes back. a second gate in this process could only ever drift from the
    /// one that actually decides.
    pub fn act(&self, action: agent::AgentAction) -> Result<serde_json::Value> {
        let session = self.session.as_ref().ok_or_else(|| {
            NodeError::Rejected(format!(
                "this run has no agent session ({ENV_SESSION_KEY} is unset), so it holds no \
                 credential to prove a write came from it — writing is refused rather than \
                 attributed to the wrong identity"
            ))
        })?;
        let msg = sdk::Msg {
            target: TARGET_RUNS.to_string(),
            payload: runs::encode_msg(&runs::RunsMsg::AgentAction {
                run_id: session.run_id.clone(),
                action,
            }),
        };
        // the frame's origin IS the session public key, bound by the signature
        // over (origin, seq, target, payload). that is the proof.
        let seq = session.seq.fetch_add(1, Ordering::Relaxed);
        let frame = node::encode_frame(&session.signer, seq, &msg);
        self.node.submit_frame(frame)
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

/// this run's session credential, read out of the environment the provisioner
/// set. BOTH halves are required — a key with no run to name, or a run with no
/// key to sign for it, is not a session — and a malformed key is dropped rather
/// than guessed at: the tools then refuse to write and SAY so, which is far
/// better than signing with something that will never verify.
fn session_from_env() -> Option<Session> {
    let hex = std::env::var(ENV_SESSION_KEY).ok().filter(|s| !s.is_empty())?;
    let run_id = std::env::var(ENV_RUN_ID).ok().filter(|s| !s.is_empty())?;
    let raw = decode_hex(&hex)?;
    let signer = ed25519::PrivateKey::decode(raw.as_slice()).ok()?;
    Some(Session {
        signer,
        run_id,
        seq: AtomicU64::new(0),
    })
}

fn decode_hex(s: &str) -> Option<Vec<u8>> {
    s.len()
        .is_multiple_of(2)
        .then(|| {
            (0..s.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
                .collect::<Option<Vec<u8>>>()
        })
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_decodes_only_well_formed_key_material() {
        assert_eq!(decode_hex("00ff10"), Some(vec![0x00, 0xff, 0x10]));
        // an odd-length or non-hex string is NOT a key. returning None here is
        // what makes the tools refuse to write and say why, instead of signing
        // with garbage that could never verify.
        assert_eq!(decode_hex("abc"), None);
        assert_eq!(decode_hex("zz"), None);
    }

    #[test]
    fn a_session_needs_both_a_key_and_a_run_to_name() {
        // guarded against the half-configured node: a key with no run id names
        // no run, and a run id with no key can prove nothing about it.
        unsafe {
            std::env::remove_var(ENV_SESSION_KEY);
            std::env::set_var(ENV_RUN_ID, "s1:0");
        }
        assert!(session_from_env().is_none());
        unsafe {
            std::env::set_var(ENV_SESSION_KEY, "ab".repeat(32));
            std::env::remove_var(ENV_RUN_ID);
        }
        assert!(session_from_env().is_none());
        unsafe {
            std::env::remove_var(ENV_SESSION_KEY);
        }
    }
}
