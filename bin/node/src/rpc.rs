// ============================================================================
// the local rpc: json-lines over tcp, bridged from blocking threads.
// ============================================================================

/// one rpc request, parsed from a json line.
#[derive(serde::Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub(crate) enum RpcRequest {
    /// submit an op into the ordered lane (accepted != finalized — poll status).
    Submit { target: String, payload_hex: String },
    /// read-only query against a module's committed+staged projection.
    Query { target: String, req_hex: String },
    /// node status: latest applied boundary + every module root.
    Status,
    /// the verified join requests parked joiners announced to THIS member —
    /// the queue the approve button (or `node resident accept`) settles.
    JoinRequests,
    /// the node-owned join state (ADR §6): the ONE authoritative source the
    /// app renders instead of parsing daemon.log markers. derived from gate
    /// progress + committed chain state, never a scattered guess.
    JoinState,
    /// the direct-peer sample: mesh-tracked connections plus per-peer
    /// traffic counters and statesync progression (see [`noded::peers`]).
    Peers,
    /// graceful stop: replies ok, then exits 0 after the current pump turn.
    Shutdown,
}

/// the node-owned join-state projection (ADR §6). `phase` uses the app's
/// onboarding vocabulary so the console renders it verbatim:
/// `parked | admitted | synced | promoted`.
#[derive(serde::Serialize)]
pub(crate) struct JoinStateView {
    pub(crate) phase: String,
    pub(crate) detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) height: Option<u64>,
}

/// one verified, unapproved join announce (node-local, in-memory; the parked
/// joiner re-announces every few seconds, so nothing here is durable state).
pub(crate) struct JoinRequestRecord {
    pub(crate) issuer: Vec<u8>,
    pub(crate) first_seen_ms: u64,
    pub(crate) last_seen_ms: u64,
}

/// the rpc/console projection of one [`JoinRequestRecord`].
#[derive(serde::Serialize)]
pub(crate) struct JoinRequestView {
    /// the key asking to join, hex.
    pub(crate) joiner: String,
    /// the member whose invite token authorized the announce, hex.
    pub(crate) issuer: String,
    pub(crate) first_seen_ms: u64,
    pub(crate) last_seen_ms: u64,
}

#[derive(serde::Serialize)]
pub(crate) struct RpcReply {
    pub(crate) ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) reply_hex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) status: Option<RpcStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) join_requests: Option<Vec<JoinRequestView>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) join_state: Option<JoinStateView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) peers: Option<noded::peers::PeersView>,
}

#[derive(serde::Serialize)]
pub(crate) struct RpcStatus {
    pub(crate) height: Option<u64>,
    pub(crate) root_hash: String,
    pub(crate) modules: std::collections::BTreeMap<String, String>,
}

impl RpcReply {
    pub(crate) fn ok() -> Self {
        Self {
            ok: true,
            error: None,
            reply_hex: None,
            status: None,
            join_requests: None,
            join_state: None,
            peers: None,
        }
    }
    pub(crate) fn err(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            error: Some(msg.into()),
            reply_hex: None,
            status: None,
            join_requests: None,
            join_state: None,
            peers: None,
        }
    }
}

/// a parsed request plus the blocking thread's reply slot.
pub(crate) type RpcJob = (RpcRequest, std::sync::mpsc::Sender<RpcReply>);

/// serve json-lines rpc on `listener`, one OS thread per connection (local,
/// low-volume — an operator console, a script). each line becomes an [`RpcJob`]
/// pushed into the pump's bounded queue; the pump answers between drains, so
/// every reply reflects a block boundary. this runs on PLAIN OS THREADS: it
/// must never touch the async runtime, only the mpsc bridge.
pub(crate) fn spawn_rpc_listener(
    listener: std::net::TcpListener,
    bridge: futures::channel::mpsc::Sender<RpcJob>,
) {
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(conn) = conn else { continue };
            let mut bridge = bridge.clone();
            std::thread::spawn(move || {
                use std::io::{BufRead as _, BufReader, Write as _};
                let reader = BufReader::new(conn.try_clone().expect("clone rpc conn"));
                let mut conn = conn;
                for line in reader.lines() {
                    let Ok(line) = line else { break };
                    if line.trim().is_empty() {
                        continue;
                    }
                    let reply = match serde_json::from_str::<RpcRequest>(&line) {
                        Ok(req) => {
                            let (tx, rx) = std::sync::mpsc::channel();
                            if bridge.try_send((req, tx)).is_err() {
                                RpcReply::err("node busy (rpc queue full)")
                            } else {
                                // the pump answers within a tick; a stuck node
                                // must not park the operator's console forever.
                                rx.recv_timeout(std::time::Duration::from_secs(10))
                                    .unwrap_or_else(|_| RpcReply::err("node unresponsive"))
                            }
                        }
                        Err(e) => RpcReply::err(format!("bad request: {e}")),
                    };
                    let mut out = serde_json::to_string(&reply).expect("reply serializes");
                    out.push('\n');
                    if conn.write_all(out.as_bytes()).is_err() {
                        break;
                    }
                }
            });
        }
    });
}
