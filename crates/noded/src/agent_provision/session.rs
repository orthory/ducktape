//! The per-run agent session signer. One fresh ed25519 keypair is bound to the
//! run in consensus, but its private half stays in this host process.
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
//! owner's grant is already committed as `ModelRecord { owner, allowed_actions,
//! caps }`. the session adds proof of ORIGIN, not authority.
//!
//! The child receives only a random token for a host endpoint. That endpoint
//! accepts `AgentAction` and `DelegateRun` for exactly this run, signs them, and
//! dies with the provisioned workspace. A shell can therefore exercise the
//! committed agent grant but can never recover a general-purpose frame signer.
//!
//! a failed open is NOT a failed run (W-degrade): the run proceeds with no
//! session vars set, which is precisely the pre-session behaviour — a read-only
//! tool plane. loudly, in the `[oracle]` voice, so a node that is somehow not the
//! assignee is visible rather than mysterious.

use commonware_codec::DecodeExt as _;
use commonware_cryptography::{Signer as _, ed25519};
use compute_service::WorkspaceSpec;
use futures::channel::oneshot;
use futures::{SinkExt as _, StreamExt as _};
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, Response, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;

use crate::node_link::NodeLink;

/// the module that owns the session registry.
const RUNS_MODULE: &str = "runs";
const ACTION_HEADER: &str = "x-ducktape-run-action";
const MAX_ACTION_REQUEST_BYTES: usize = runs::MAX_ACTIONS_BYTES + runs::MAX_DELEGATIONS_BYTES;

pub(super) const ENV_ACTION_URL: &str = "DUCKTAPE_RUN_ACTION_URL";
pub(super) const ENV_ACTION_TOKEN: &str = "DUCKTAPE_RUN_ACTION_TOKEN";

/// An opened, host-owned signer and its narrow child-facing endpoint.
pub(super) struct RunSession {
    pub(super) action_url: String,
    pub(super) action_token: String,
    shutdown: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for RunSession {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.task.abort();
    }
}

struct ActionState {
    node: NodeLink,
    signer: ed25519::PrivateKey,
    run_id: String,
    token: String,
    seq: tokio::sync::Mutex<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ActionRequest {
    message: runs::RunsMsg,
}

/// mint a session keypair for `spec` and bind its public half to the run.
///
/// `None` — no session, no env vars — when the run has no agent (a workspace
/// nobody acts for), when the envelope named no consensus run id (a pre-field
/// composer: there is no run to bind TO), or when the bind did not commit.
/// never an `Err`: a session is an ADDITIVE capability, and refusing to
/// provision a workspace because the tool plane could not be opened would fail
/// runs that used to work.
pub(super) async fn open(node: &NodeLink, spec: &WorkspaceSpec) -> Option<RunSession> {
    let agent = spec.agent_id.as_ref()?;
    // the CONSENSUS id or nothing. `spec.run_id` is `{saga_id}:{attempt}` — a
    // host-local dir key that names no run in `runs`, so binding on it would
    // open a session against a run that does not exist. an absent id is a
    // pre-field envelope: degrade to the read-only plane, loudly, exactly as a
    // refused bind does.
    let Some(run_id) = spec.consensus_run_id.clone() else {
        tracing::warn!(
            target: "ducktape::agent",
            event = "agent_session_unavailable",
            run_id = spec.run_id.as_str(),
            agent_id = agent.as_str(),
            reason = "missing_consensus_run_id",
            "agent session unavailable"
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
    match submit(node, payload).await {
        Ok(()) => match start_action_server(node.clone(), key, run_id.clone()).await {
            Ok(session) => Some(session),
            Err(detail) => {
                tracing::warn!(
                    target: "ducktape::agent",
                    event = "agent_session_unavailable",
                    run_id = run_id.as_str(),
                    agent_id = agent.as_str(),
                    reason = "signer_endpoint_failed",
                    detail = detail.as_str(),
                    "agent session unavailable"
                );
                None
            }
        },
        Err(detail) => {
            tracing::warn!(
                target: "ducktape::agent",
                event = "agent_session_unavailable",
                run_id = run_id.as_str(),
                agent_id = agent.as_str(),
                reason = "bind_rejected",
                detail = detail.as_str(),
                "agent session unavailable"
            );
            None
        }
    }
}

async fn start_action_server(
    node: NodeLink,
    signer: ed25519::PrivateKey,
    run_id: String,
) -> Result<RunSession, String> {
    // Bind the host interfaces so a child in a private netns can reach the
    // same run-scoped endpoint through its gateway. Direct children still
    // receive a 127.0.0.1 URL; the 256-bit token and closed message/run scope
    // are the boundary, not an ambient network listener.
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::UNSPECIFIED, 0))
        .await
        .map_err(|error| format!("bind scoped action signer: {error}"))?;
    let address = listener
        .local_addr()
        .map_err(|error| format!("read scoped action signer address: {error}"))?;
    let mut secret = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut secret);
    let token = duckfs_core::to_hex(&secret);
    let state = Arc::new(ActionState {
        node,
        signer,
        run_id: run_id.clone(),
        token: token.clone(),
        seq: tokio::sync::Mutex::new(0),
    });
    let app = Router::new()
        .route("/v1/run-action", post(run_action))
        .layer(DefaultBodyLimit::max(MAX_ACTION_REQUEST_BYTES))
        .with_state(state);
    let (shutdown, rx) = oneshot::channel();
    let task = tokio::spawn(async move {
        let _ = axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = rx.await;
            })
            .await;
    });
    Ok(RunSession {
        action_url: format!("http://127.0.0.1:{}/v1/run-action", address.port()),
        action_token: token,
        shutdown: Some(shutdown),
        task,
    })
}

async fn run_action(
    State(state): State<Arc<ActionState>>,
    headers: HeaderMap,
    Json(request): Json<ActionRequest>,
) -> Response<Body> {
    // the listener is not loopback-bound (a child in a private netns must reach
    // it), so the token IS the boundary: compare it in constant time like every
    // other secret in this crate, never with a short-circuiting `==`.
    let authorized = headers
        .get(ACTION_HEADER)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|presented| crate::services::token_matches(presented, &state.token));
    if !authorized {
        return action_response(StatusCode::UNAUTHORIZED, "action token rejected");
    }
    let names_bound_run = match &request.message {
        runs::RunsMsg::AgentAction { run_id, .. } | runs::RunsMsg::DelegateRun { run_id, .. } => {
            run_id == &state.run_id
        }
        _ => false,
    };
    if !names_bound_run {
        return action_response(
            StatusCode::FORBIDDEN,
            "message is outside this run's action scope",
        );
    }
    let result = submit_action(&state, request.message).await;
    match result {
        Ok(()) => action_response(StatusCode::OK, "ok"),
        Err(error) => action_response(StatusCode::BAD_REQUEST, &error),
    }
}

type ActionEvents =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// The subscribe reply is a barrier: the node has registered its block receiver
/// before this returns. Empty topics still receive the committed tip on every block.
async fn action_events(node: &NodeLink) -> Result<ActionEvents, String> {
    let base = node.base();
    let ws_base = match base.strip_prefix("http://") {
        Some(rest) => format!("ws://{rest}"),
        None => match base.strip_prefix("https://") {
            Some(rest) => format!("wss://{rest}"),
            None => return Err("action node URL has no HTTP scheme".into()),
        },
    };
    let (mut events, _) = tokio_tungstenite::connect_async(format!("{ws_base}/v1/ws"))
        .await
        .map_err(|error| format!("connect action receipt events: {error}"))?;
    let subscription =
        serde_json::json!({"op": "subscribe", "topics": [], "resume": {}}).to_string();
    events
        .send(tokio_tungstenite::tungstenite::Message::Text(subscription))
        .await
        .map_err(|error| format!("subscribe action receipt events: {error}"))?;
    while let Some(frame) = events.next().await {
        let frame = frame.map_err(|error| format!("action receipt event stream: {error}"))?;
        let tokio_tungstenite::tungstenite::Message::Text(text) = frame else {
            continue;
        };
        let value: serde_json::Value = serde_json::from_str(&text)
            .map_err(|error| format!("decode action receipt event: {error}"))?;
        match value.get("type").and_then(serde_json::Value::as_str) {
            Some("subscribed") => return Ok(events),
            Some("error") => return Err(format!("action receipt subscription refused: {value}")),
            _ => {}
        }
    }
    Err("action receipt event stream closed before subscription".into())
}

async fn next_action_request(node: &NodeLink, run_id: &str) -> Result<String, String> {
    let bytes = node
        .query(
            RUNS_MODULE,
            &runs::encode_query(&runs::RunsQuery::AgentSessions),
        )
        .await?;
    let runs::RunsReply::AgentSessions(sessions) = runs::decode_reply(&bytes)? else {
        return Err("unexpected run session reply".into());
    };
    let Some(session) = sessions.iter().find(|session| session.run_id == run_id) else {
        return Err("run session has closed".into());
    };
    Ok(runs::action_request_id(run_id, session.actions))
}

async fn action_result(
    node: &NodeLink,
    request_id: &str,
) -> Result<Option<Result<(), String>>, String> {
    let bytes = node
        .query(
            RUNS_MODULE,
            &runs::encode_query(&runs::RunsQuery::ActionRequest {
                request_id: request_id.into(),
            }),
        )
        .await?;
    let runs::RunsReply::ActionRequest(request) = runs::decode_reply(&bytes)? else {
        return Err("unexpected action request reply".into());
    };
    let Some(request) = request else {
        return Ok(None);
    };
    match request.status {
        runs::ActionStatus::AwaitingProgram | runs::ActionStatus::Claimed { .. } => Ok(None),
        runs::ActionStatus::Rejected { reason } => Ok(Some(Err(reason))),
        runs::ActionStatus::Completed { outcome, .. } => match outcome {
            dispatch::CallOutcomeSummary::Applied { .. } => Ok(Some(Ok(()))),
            dispatch::CallOutcomeSummary::Rejected { reason } => Ok(Some(Err(reason))),
            dispatch::CallOutcomeSummary::Refused(reason) => {
                Ok(Some(Err(format!("program action refused: {reason:?}"))))
            }
            dispatch::CallOutcomeSummary::Unrepresentable { .. } => Ok(Some(Err(
                "program action outcome could not be represented".into(),
            ))),
        },
    }
}

async fn await_action_result(
    node: &NodeLink,
    request_id: &str,
    mut events: ActionEvents,
) -> Result<(), String> {
    if let Some(result) = action_result(node, request_id).await? {
        return result;
    }
    let mut observed_height = None;
    while let Some(frame) = events.next().await {
        let frame = frame.map_err(|error| format!("action receipt event stream: {error}"))?;
        let tokio_tungstenite::tungstenite::Message::Text(text) = frame else {
            continue;
        };
        let value: serde_json::Value = serde_json::from_str(&text)
            .map_err(|error| format!("decode action receipt event: {error}"))?;
        match value.get("type").and_then(serde_json::Value::as_str) {
            Some("heartbeat") => {}
            Some("error") => return Err(format!("action receipt stream refused: {value}")),
            _ => continue,
        }
        let Some(height) = value.get("height").and_then(serde_json::Value::as_u64) else {
            continue;
        };
        if observed_height == Some(height) {
            continue;
        }
        observed_height = Some(height);
        if let Some(result) = action_result(node, request_id).await? {
            return result;
        }
    }
    Err("node disconnected before the program action completed".into())
}

async fn submit_action(state: &ActionState, message: runs::RunsMsg) -> Result<(), String> {
    // Serialize both admission and completion so the next session slot cannot
    // overtake an action whose actual target write is still pending.
    let mut next_seq = state.seq.lock().await;
    let pending = match &message {
        runs::RunsMsg::AgentAction { run_id, .. } => {
            let events = action_events(&state.node).await?;
            let request_id = next_action_request(&state.node, run_id).await?;
            Some((request_id, events))
        }
        runs::RunsMsg::DelegateRun {
            run_id, request_id, ..
        } => {
            let events = action_events(&state.node).await?;
            Some((runs::delegation_action_id(run_id, request_id), events))
        }
        _ => return Err("message is outside the run action scope".into()),
    };
    let msg = sdk::Msg {
        target: RUNS_MODULE.into(),
        payload: runs::encode_msg(&message),
    };
    let frame = node::encode_frame(&state.signer, *next_seq, &msg);
    *next_seq = next_seq
        .checked_add(1)
        .ok_or_else(|| "action signer sequence exhausted".to_string())?;
    state.node.submit_frame(frame).await?;
    match pending {
        Some((request_id, events)) => await_action_result(&state.node, &request_id, events).await,
        None => Ok(()),
    }
}

fn action_response(status: StatusCode, message: &str) -> Response<Body> {
    let body = serde_json::json!({"message": message}).to_string();
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Body::from(body))
        .expect("static scoped action response")
}

/// submit the bind on the node's ordinary submit lane.
///
/// `/v1/submit` frames the op with the NODE's key, and that node is the run's
/// committed lease-holder — which is exactly the assignee `runs` checks. That
/// is why this must NOT sign the bind itself: the lane's own identity is the
/// right one, and a session key signing its own bind would prove nothing.
async fn submit(node: &NodeLink, payload: Vec<u8>) -> Result<(), String> {
    // a module rejection rides through verbatim — "not the run's assignee" is
    // the one worth reading in a log.
    node.submit(RUNS_MODULE, &payload).await.map(|_height| ())
}
