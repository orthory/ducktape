//! the per-run AGENT SESSION KEY: one fresh ed25519 keypair per portable run
//! that has an agent, bound to that run in consensus before the run starts, and
//! handed to the run's tool plane as `DUCKTAPE_RUN_SESSION_KEY`.
//!
//! why a key at all: an agent's mid-run writes have to be attributable, and the
//! frameless `/v1/submit` lane cannot carry attribution — its `origin` is a
//! caller-supplied string that `bin/node` discards outright and re-signs with
//! the NODE key. an op signed by a session key is different in kind: the frame's
//! origin IS its verified signer ([`node::decode_frame`] binds
//! `(origin, seq, target, payload)`), so consensus can check "this op came from
//! that agent's run" instead of taking a host's word for it.
//!
//! the BIND is self-authorizing: `RunsMsg::OpenAgentSession` is submitted through
//! the node's ORDINARY submit lane, whose op is framed with the node's own key —
//! and that node is the run's committed lease-holder, because it is the node
//! executing the run. `runs` checks exactly that. no owner is at a keyboard to
//! sign anything (an issue-mention run has nobody), and none is needed: the
//! owner's grant is already committed as `AgentRecord { owner, allowed_actions,
//! caps }`. the session adds proof of ORIGIN, not authority.
//!
//! ## the key is a BEARER CREDENTIAL; the SANDBOX is the boundary
//!
//! `DUCKTAPE_RUN_SESSION_KEY` is a raw ed25519 PRIVATE key in the child's
//! environment, and under codex the agent has a shell that can read it. so be
//! exact about what it confers: whoever holds it can sign ANY `Msg` to ANY
//! module, not just `RunsMsg::AgentAction`. consensus gates the ACTION lane —
//! `allowed_actions`, caps, the bound session — it does not gate the KEY. off
//! that lane the key is simply an unknown external submitter, and
//! `chat::author_from_origin` maps any non-empty `Origin::External` to
//! `AuthorRef::User(key)`: an `Open` channel admits it with no
//! `chat.post_message` grant at all, and channels, tasks and the rest are
//! equally open to it. the grant lane bounds what an agent may do AS the agent;
//! it never bounded this key.
//!
//! nor is the key bounded in TIME the way the session is. pruning the session
//! with its run only closes the `AgentAction` lane; the keypair stays a valid
//! signer forever. a leaked one is a leaked user key, not a spent ticket.
//!
//! what actually contains it is the codex SANDBOX the run executes under
//! (`--sandbox workspace-write`, no network): a shell the model runs cannot
//! reach the node's HTTP submit lane to use the key at all. the MCP server that
//! CAN reach the node is a separate process outside that sandbox, exposing only
//! the vetted tool surface. so the containment is the sandbox — say it plainly,
//! because a comment claiming the key is harmless is the one that gets a future
//! reader to hand it somewhere the sandbox is not.
//!
//! what the split with the NODE key still buys is real: the node key signs
//! validator votes, valset ops, every op the human makes, and it must never
//! enter that process tree. a session key's blast radius is one unprivileged
//! external identity; the node key's is the node.
//!
//! a failed open is NOT a failed run (W-degrade): the run proceeds with no
//! session vars set, which is precisely the pre-session behaviour — a read-only
//! tool plane. loudly, in the `[oracle]` voice, so a node that is somehow not the
//! assignee is visible rather than mysterious.

use commonware_codec::DecodeExt as _;
use commonware_cryptography::{Signer as _, ed25519};
use dispatch_oracle::WorkspaceSpec;
use futures::channel::oneshot;

use crate::{NodeCommand, NodeHandle, ORACLE_ORIGIN};

/// the module that owns the session registry.
const RUNS_MODULE: &str = "runs";

/// an OPENED session: the private half the run's tool server signs its ops with.
/// only ever constructed after the bind COMMITTED — a `RunSession` in hand means
/// consensus has the matching public key against this run.
pub(super) struct RunSession {
    /// the ed25519 private key as lowercase hex — the SAME encoding the node's
    /// own key file uses, so `ed25519::PrivateKey::decode(unhex(..))` reads it
    /// back and there is one key format in the system, not two.
    pub(super) private_hex: String,
    /// the run this session is bound to, in the ONLY id space `runs` resolves:
    /// [`WorkspaceSpec::consensus_run_id`], never the spec's host-local
    /// `run_id`. it is what the bind named, so it is what every action the run
    /// signs must name too — the key and this id are one credential.
    pub(super) run_id: String,
}

/// mint a session keypair for `spec` and bind its public half to the run.
///
/// `None` — no session, no env vars — when the run has no agent (a workspace
/// nobody acts for), when the envelope named no consensus run id (a pre-field
/// composer: there is no run to bind TO), or when the bind did not commit.
/// never an `Err`: a session is an ADDITIVE capability, and refusing to
/// provision a workspace because the tool plane could not be opened would fail
/// runs that used to work.
pub(super) async fn open(handle: &NodeHandle, spec: &WorkspaceSpec) -> Option<RunSession> {
    let agent = spec.agent_id.as_ref()?;
    // the CONSENSUS id or nothing. `spec.run_id` is `{saga_id}:{attempt}` — a
    // host-local dir key that names no run in `runs`, so binding on it would
    // open a session against a run that does not exist. an absent id is a
    // pre-field envelope: degrade to the read-only plane, loudly, exactly as a
    // refused bind does.
    let Some(run_id) = spec.consensus_run_id.clone() else {
        eprintln!(
            "[oracle] the run envelope for {} ({agent}) names no consensus run id — no agent \
             session is opened (a read-only tool plane); the run still returns a response",
            spec.run_id
        );
        return None;
    };
    // mint from OS randomness, with the same ed25519 types `node::encode_frame`
    // signs with — no second crypto stack, no hand-rolled key. every 32-byte
    // string is a valid seed (the scheme clamps), so the decode cannot fail;
    // this mirrors the node's own `load_or_generate_identity`.
    let mut seed = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut seed);
    let key = ed25519::PrivateKey::decode(seed.as_slice()).expect("32 random bytes decode");
    let payload = runs::encode_msg(&runs::RunsMsg::OpenAgentSession {
        run_id: run_id.clone(),
        session_key: key.public_key().as_ref().to_vec(),
    });
    match submit(handle, payload).await {
        // the private half never leaves this host: it goes into the run's env
        // and nowhere else — not into the op, not into a log, not to a peer.
        Ok(()) => Some(RunSession {
            private_hex: duckfs_core::to_hex(&seed),
            run_id,
        }),
        Err(detail) => {
            eprintln!(
                "[oracle] agent session for run {run_id} ({agent}) did not open: {detail} — the \
                 run proceeds WITHOUT one (a read-only tool plane); it can still return a response"
            );
            None
        }
    }
}

/// submit the bind on the node's ordinary actor lane.
///
/// the `origin` here is only ever read by the EMBEDDED daemon, whose executing
/// identity is that string ([`ORACLE_ORIGIN`] — the same one its dispatch pool
/// claims runs under, so it IS the assignee `runs` will check against). the real
/// node discards it and frames the op with its node key, which is the assignee
/// there. one call, correct on both: the lane's own identity is the right one,
/// which is exactly why this must NOT sign the bind itself.
async fn submit(handle: &NodeHandle, payload: Vec<u8>) -> Result<(), String> {
    let (reply, rx) = oneshot::channel();
    handle
        .send(NodeCommand::Submit {
            target: RUNS_MODULE.into(),
            payload,
            origin: ORACLE_ORIGIN.to_vec(),
            reply,
        })
        .await
        .map_err(|_| "the node actor is gone".to_string())?;
    match rx.await {
        Ok(Ok(_block)) => Ok(()),
        // a module rejection is the interesting case and rides through verbatim
        // — "not the run's assignee" is the one worth reading in a log.
        Ok(Err(detail)) => Err(detail),
        Err(_) => Err("the node actor dropped the reply".into()),
    }
}
