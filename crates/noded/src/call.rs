//! the call lane: the webview end of a huddle (`GET /v1/call/ws`) and the
//! typed session/control types it shares with the node's call hub.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::Response;
use serde::{Deserialize, Serialize};

use crate::{NodeHandle, error_response, hex_bytes};

// ---- the call lane ----------------------------------------------------------
// the webview end of a huddle: GET /v1/call/ws?channel=<id> upgrades to a
// typed socket that carries the huddle's audio, camera video, and call control
// together. the handler asks the node's call hub for a session over the request
// lane below; a daemon without a hub answers 503, and every refusal path says
// WHY as one text frame before closing.
//
// the binary frame layout (tag byte, header fields, BE headers/LE pcm payload
// per D1) and its encode/decode functions live in `media_service::call_wire` — the
// single definition site this handler ports onto below. text frames are json
// control: client→server `CallClientControl` (recipients / beacon /
// keyframe_request), server→client `CallServerControl` (keyframe_request /
// peer_beacon / rate_hint).
//
// the hub side lives with the mesh (validators and standing residents run one): it
// fragments/reassembles the video ends over `Service::Video` and routes control
// (keyframe kicks, presence beacons, rate hints — see `media_service::video`).

/// call-control the WEBVIEW asks the hub to act on (webview → hub).
pub enum CallControlIn {
    /// our local presence/state, pushed immediately AND repeated at 1 Hz as
    /// this session's beacon to every recipient. `sharing` = the video lane is a
    /// screen share rather than the camera.
    Beacon {
        muted: bool,
        camera_on: bool,
        sharing: bool,
    },
    /// our decoder lost `peer`'s stream — ask `peer` for a fresh keyframe.
    KeyframeRequest { peer: [u8; 32] },
}

/// call-control the hub surfaces to the WEBVIEW (hub → webview).
pub enum CallControlOut {
    /// a peer's receiver asked us to send it a fresh keyframe — the webview
    /// tells its encoder to emit one (rate-limited to ≤1 Hz by the hub).
    KeyframeRequest,
    /// a peer's 1 Hz presence beacon — drives the tile's mute/camera badges +
    /// the screen-share treatment.
    PeerBeacon {
        peer: [u8; 32],
        muted: bool,
        camera_on: bool,
        sharing: bool,
    },
    /// the effective outbound bitrate cap (min of every peer's hint) — the
    /// webview retargets its encoder. emitted only when the value changes.
    RateHint { max_kbps: u32 },
}

/// One editor's ephemeral caret/selection inside a page. `block_id=None`
/// means the peer is viewing the page without a body-block caret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageCursor {
    pub block_id: Option<String>,
    pub anchor: u32,
    pub head: u32,
}

/// page-presence control from the webview to the overlay hub.
pub enum PresenceControlIn {
    Cursor(PageCursor),
}

/// page-presence control from a mesh peer to the webview.
pub enum PresenceControlOut {
    PeerCursor { peer: [u8; 32], cursor: PageCursor },
}

/// one live huddle session's channel ends, hub ↔ websocket handler / gateway.
pub struct CallSession {
    /// captured mic frames, exactly [`media_service::voice::FRAME_SAMPLES`] samples each.
    pub pcm_in: tokio::sync::mpsc::Sender<Vec<i16>>,
    /// mixed playout frames at the 20 ms tick, same shape.
    pub mixed_out: tokio::sync::mpsc::Receiver<Vec<i16>>,
    /// where this session's frames fan out: the huddle roster's node keys
    /// (raw ed25519 bytes), steered by the client as consensus state changes.
    pub recipients: tokio::sync::watch::Sender<Vec<[u8; 32]>>,
    /// captured camera frames webview → hub (fragmented onto `Service::Video`).
    pub video_in: tokio::sync::mpsc::Sender<media_service::call_wire::CapturedFrame>,
    /// reassembled peer camera frames hub → webview.
    pub video_out: tokio::sync::mpsc::Receiver<media_service::call_wire::PeerFrame>,
    /// call-control webview → hub (local beacon, keyframe asks).
    pub control_in: tokio::sync::mpsc::Sender<CallControlIn>,
    /// call-control hub → webview (peer beacons, keyframe kicks, rate hints).
    pub control_out: tokio::sync::mpsc::Receiver<CallControlOut>,
}

/// a websocket handler's ask: open (or replace) the call session for a
/// channel. the hub replies with the session's ends or a refusal string.
pub struct CallSessionRequest {
    pub channel_id: String,
    pub reply: tokio::sync::oneshot::Sender<Result<CallSession, String>>,
}

/// A lean Pages presence session. It shares the authenticated overlay control
/// plane with huddles but owns a separate flow and carries no media.
pub struct PresenceSession {
    pub recipients: tokio::sync::watch::Sender<Vec<[u8; 32]>>,
    pub control_in: tokio::sync::mpsc::Sender<PresenceControlIn>,
    pub control_out: tokio::sync::mpsc::Receiver<PresenceControlOut>,
}

pub struct PresenceSessionRequest {
    pub page_id: String,
    pub reply: tokio::sync::oneshot::Sender<Result<PresenceSession, String>>,
}

/// Both live, off-consensus session types share one request lane into the
/// overlay hub. The hub keeps one huddle and one Pages session concurrently.
pub enum RealtimeSessionRequest {
    Call(CallSessionRequest),
    Presence(PresenceSessionRequest),
}

/// the request lane into the realtime overlay hub.
pub type CallLane = tokio::sync::mpsc::Sender<RealtimeSessionRequest>;

/// client → server control messages on the call socket (text frames).
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CallClientControl {
    /// replace the fan-out set with these hex node keys (self excluded —
    /// the client tracks the consensus huddle roster).
    Recipients { peers: Vec<String> },
    /// this client's ephemeral state; the hub beacons it to peers at 1 Hz.
    /// `sharing` defaults false so a pre-share client (no field) still parses.
    Beacon {
        muted: bool,
        camera_on: bool,
        #[serde(default)]
        sharing: bool,
    },
    /// the decoder lost sync with `peer` — ask it for a keyframe.
    KeyframeRequest { peer: String },
}

/// server → client control messages on the call socket (text frames).
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CallServerControl {
    /// a peer lost sync with US: encode the next frame as a keyframe.
    KeyframeRequest,
    /// a peer's 1 Hz beacon (ephemeral presence/state — never consensus).
    PeerBeacon {
        peer: String,
        muted: bool,
        camera_on: bool,
        sharing: bool,
    },
    /// send at no more than this (min across peers' loss reports).
    RateHint {
        max_kbps: u32,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PresenceClientControl {
    Recipients {
        peers: Vec<String>,
    },
    Cursor {
        block_id: Option<String>,
        anchor: u32,
        head: u32,
    },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PresenceServerControl {
    PeerCursor {
        peer: String,
        block_id: Option<String>,
        anchor: u32,
        head: u32,
    },
}

#[derive(Debug, Deserialize)]
pub struct CallParams {
    channel: String,
}

#[derive(Debug, Deserialize)]
pub struct PresenceParams {
    page: String,
}

pub(crate) async fn call_ws(
    State(handle): State<NodeHandle>,
    Query(params): Query<CallParams>,
    upgrade: WebSocketUpgrade,
) -> Response {
    let Some(call) = handle.call.clone() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "calls are not available on this node (no mesh call hub)",
        );
    };
    if params.channel.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "channel must not be empty");
    }
    upgrade.on_upgrade(move |socket| call_session(socket, call, params.channel))
}

/// pump one huddle's audio, camera video, and call control between the webview
/// websocket and the hub session. binary client frames are decoded via
/// `media_service::call_wire` and tag-dispatched: audio → `pcm_in`, captured video →
/// `video_in`; the hub's `mixed_out`/`video_out`/`control_out` ends flow back
/// as `media_service::call_wire`-encoded binary + json text. text client frames steer
/// fan-out and carry beacons/keyframe asks. either side closing ends the
/// session — dropping the ends is the teardown signal the hub watches.
async fn call_session(mut socket: WebSocket, call: CallLane, channel_id: String) {
    let (reply, opened) = tokio::sync::oneshot::channel();
    let request = RealtimeSessionRequest::Call(CallSessionRequest { channel_id, reply });
    // every refusal path says WHY as a text frame before closing — the client
    // surfaces it as a session error instead of a silent no-op.
    // Both lane-closed cases in one sentence, because the handler cannot tell
    // them apart — and "no live call hub" alone left the user with a red
    // "Voice connection failed." and nothing to act on.
    const NO_HUB: &str = "this node runs no call hub, so it cannot host a huddle: it has no mesh \
                          overlay (wireguard_listen unset, or the fake effect — huddle media \
                          rides the overlay), is a sync-only observer, or its hub stopped.";
    let session = match call.send(request).await {
        Ok(()) => match opened.await {
            Ok(Ok(session)) => session,
            Ok(Err(refusal)) => {
                let _ = socket.send(Message::Text(refusal.into())).await;
                return;
            }
            Err(_) => {
                // hub dropped the reply — shutting down.
                let _ = socket.send(Message::Text(NO_HUB.into())).await;
                return;
            }
        },
        Err(_) => {
            // the request lane is closed: a mode that never runs a hub
            // (parked joiner, sync-only observer) or a dead hub thread.
            let _ = socket.send(Message::Text(NO_HUB.into())).await;
            return;
        }
    };
    let CallSession {
        pcm_in,
        mut mixed_out,
        recipients,
        video_in,
        mut video_out,
        control_in,
        mut control_out,
    } = session;
    loop {
        tokio::select! {
            inbound = socket.recv() => match inbound {
                Some(Ok(Message::Binary(bytes))) => {
                    if let Some(frame) = media_service::call_wire::decode_audio(&bytes) {
                        // full lane = the hub is behind; late audio is dead
                        // audio, so drop the frame rather than backpressure.
                        let _ = pcm_in.try_send(frame);
                    } else if let Some(frame) = media_service::call_wire::decode_captured(&bytes) {
                        let _ = video_in.try_send(frame);
                    } // unknown/short frame — drop, stay alive
                }
                Some(Ok(Message::Text(text))) => {
                    match serde_json::from_str::<CallClientControl>(&text) {
                        Ok(CallClientControl::Recipients { peers }) => {
                            let keys: Vec<[u8; 32]> = peers
                                .iter()
                                .filter_map(|hex| duckfs_core::from_hex_32(hex))
                                .collect();
                            let _ = recipients.send(keys);
                        }
                        Ok(CallClientControl::Beacon {
                            muted,
                            camera_on,
                            sharing,
                        }) => {
                            let _ = control_in.try_send(CallControlIn::Beacon {
                                muted,
                                camera_on,
                                sharing,
                            });
                        }
                        Ok(CallClientControl::KeyframeRequest { peer }) => {
                            if let Some(key) = duckfs_core::from_hex_32(&peer) {
                                let _ = control_in
                                    .try_send(CallControlIn::KeyframeRequest { peer: key });
                            }
                        }
                        Err(_) => {} // unknown control — ignore, stay alive
                    }
                }
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
            mixed = mixed_out.recv() => match mixed {
                Some(frame) => {
                    let bytes = media_service::call_wire::encode_audio(&frame);
                    if socket.send(Message::Binary(bytes.into())).await.is_err() {
                        break;
                    }
                }
                None => break, // the hub ended the session (replaced by a newer join).
            },
            video = video_out.recv() => match video {
                Some(frame) => {
                    let bytes = media_service::call_wire::encode_peer(&frame);
                    if socket.send(Message::Binary(bytes.into())).await.is_err() {
                        break;
                    }
                }
                None => break,
            },
            control = control_out.recv() => match control {
                Some(out) => {
                    let message = match out {
                        CallControlOut::KeyframeRequest => CallServerControl::KeyframeRequest,
                        CallControlOut::PeerBeacon { peer, muted, camera_on, sharing } => {
                            CallServerControl::PeerBeacon {
                                peer: hex_bytes(&peer),
                                muted,
                                camera_on,
                                sharing,
                            }
                        }
                        CallControlOut::RateHint { max_kbps } => {
                            CallServerControl::RateHint { max_kbps }
                        }
                    };
                    let text = serde_json::to_string(&message).expect("serializable control");
                    if socket.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                }
                None => break,
            },
        }
    }
}

pub(crate) async fn presence_ws(
    State(handle): State<NodeHandle>,
    Query(params): Query<PresenceParams>,
    upgrade: WebSocketUpgrade,
) -> Response {
    let Some(call) = handle.call.clone() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "presence is not available on this node (no mesh realtime hub)",
        );
    };
    if params.page.is_empty() || params.page.len() > 256 {
        return error_response(StatusCode::BAD_REQUEST, "page must be 1..256 bytes");
    }
    upgrade.on_upgrade(move |socket| presence_session(socket, call, params.page))
}

async fn presence_session(mut socket: WebSocket, call: CallLane, page_id: String) {
    let (reply, opened) = tokio::sync::oneshot::channel();
    let request = RealtimeSessionRequest::Presence(PresenceSessionRequest { page_id, reply });
    const NO_HUB: &str = "this node runs no mesh realtime hub, so Pages presence is unavailable";
    let session = match call.send(request).await {
        Ok(()) => match opened.await {
            Ok(Ok(session)) => session,
            Ok(Err(refusal)) => {
                let _ = socket.send(Message::Text(refusal.into())).await;
                return;
            }
            Err(_) => {
                let _ = socket.send(Message::Text(NO_HUB.into())).await;
                return;
            }
        },
        Err(_) => {
            let _ = socket.send(Message::Text(NO_HUB.into())).await;
            return;
        }
    };
    let PresenceSession {
        recipients,
        control_in,
        mut control_out,
    } = session;
    loop {
        tokio::select! {
            inbound = socket.recv() => match inbound {
                Some(Ok(Message::Text(text))) => {
                    match serde_json::from_str::<PresenceClientControl>(&text) {
                        Ok(PresenceClientControl::Recipients { peers }) => {
                            let keys = peers
                                .iter()
                                .filter_map(|hex| duckfs_core::from_hex_32(hex))
                                .collect();
                            let _ = recipients.send(keys);
                        }
                        Ok(PresenceClientControl::Cursor { block_id, anchor, head })
                            if block_id
                                .as_ref()
                                .is_none_or(|id| !id.is_empty() && id.len() <= 256) =>
                        {
                            let _ = control_in.try_send(PresenceControlIn::Cursor(PageCursor {
                                block_id,
                                anchor,
                                head,
                            }));
                        }
                        Ok(PresenceClientControl::Cursor { .. }) | Err(_) => {}
                    }
                }
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
            control = control_out.recv() => match control {
                Some(PresenceControlOut::PeerCursor { peer, cursor }) => {
                    let message = PresenceServerControl::PeerCursor {
                        peer: hex_bytes(&peer),
                        block_id: cursor.block_id,
                        anchor: cursor.anchor,
                        head: cursor.head,
                    };
                    let text = serde_json::to_string(&message).expect("serializable presence");
                    if socket.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                }
                None => break,
            },
        }
    }
}
