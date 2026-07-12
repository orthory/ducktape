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
//! ## why handing the PRIVATE key to the agent's process tree is safe
//!
//! under codex the agent has a shell and can read its own environment, so treat
//! the key as compromised-by-design. it grants nothing extra: the session key's
//! entire authority is "the actions this agent is ALREADY permitted to perform,
//! for this ONE run, until it settles" — consensus re-checks `allowed_actions`
//! and caps on every action, and the session dies with the run. the key IS the
//! agent's authority, no more; that is least privilege, not a leak.
//!
//! the NODE key is the opposite and must never reach that process: it signs
//! validator votes, valset ops, every op the human makes. hence the split — the
//! run gets a key that can only do what the agent could do anyway.
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
}

/// mint a session keypair for `spec` and bind its public half to the run.
///
/// `None` — no session, no env vars — when the run has no agent (a workspace
/// nobody acts for) or when the bind did not commit. never an `Err`: a session
/// is an ADDITIVE capability, and refusing to provision a workspace because the
/// tool plane could not be opened would fail runs that used to work.
pub(super) async fn open(handle: &NodeHandle, spec: &WorkspaceSpec) -> Option<RunSession> {
    let agent = spec.agent_id.as_ref()?;
    // mint from OS randomness, with the same ed25519 types `node::encode_frame`
    // signs with — no second crypto stack, no hand-rolled key. every 32-byte
    // string is a valid seed (the scheme clamps), so the decode cannot fail;
    // this mirrors the node's own `load_or_generate_identity`.
    let mut seed = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut seed);
    let key = ed25519::PrivateKey::decode(seed.as_slice()).expect("32 random bytes decode");
    let payload = runs::encode_msg(&runs::RunsMsg::OpenAgentSession {
        run_id: spec.run_id.clone(),
        session_key: key.public_key().as_ref().to_vec(),
    });
    match submit(handle, payload).await {
        // the private half never leaves this host: it goes into the run's env
        // and nowhere else — not into the op, not into a log, not to a peer.
        Ok(()) => Some(RunSession {
            private_hex: duckfs_core::to_hex(&seed),
        }),
        Err(detail) => {
            eprintln!(
                "[oracle] agent session for run {} ({agent}) did not open: {detail} — the run \
                 proceeds WITHOUT one (a read-only tool plane); it can still return a response",
                spec.run_id
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
