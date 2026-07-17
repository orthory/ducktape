//! Typed `/v1/call/ws` client and the media-driver seam for native huddles.
//!
//! The call socket owns routing and bounds. Hardware capture/playback is a
//! separate bounded port so the selected platform driver never parses wire
//! bytes and the consensus/UI layer never touches device callbacks.

use std::time::Duration;

use futures_util::{SinkExt as _, StreamExt as _};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use crate::transport::NodeClient;

pub const SAMPLE_RATE: u32 = 48_000;
pub const FRAME_SAMPLES: usize = 960;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(12);
const MAX_CONTROL_BYTES: usize = 4 * 1024;
const MAX_VIDEO_BYTES: usize = 4 * 1024 * 1024;
const DRIVER_QUEUE: usize = 32;
const CONTROL_QUEUE: usize = 32;
const EVENT_QUEUE: usize = 64;

const TAG_AUDIO: u8 = 0x01;
const TAG_VIDEO_CAPTURED: u8 = 0x02;
const TAG_VIDEO_PEER: u8 = 0x03;
const FLAG_KEYFRAME: u8 = 0x01;
const CAPTURED_HEADER: usize = 6;
const PEER_HEADER: usize = 38;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriverOutgoing {
    Audio(Box<[i16; FRAME_SAMPLES]>),
    Video {
        keyframe: bool,
        timestamp_ms: u32,
        vp8: Vec<u8>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriverIncoming {
    Audio(Box<[i16; FRAME_SAMPLES]>),
    Video {
        peer: [u8; 32],
        keyframe: bool,
        timestamp_ms: u32,
        vp8: Vec<u8>,
    },
    KeyframeRequested,
    RateHint {
        max_kbps: u32,
    },
}

/// Hardware-side port. Capture sends `outgoing`; playback/render receives
/// `incoming`. Both queues are bounded so a stalled renderer cannot grow RAM.
pub struct MediaDriverPort {
    pub outgoing: mpsc::Sender<DriverOutgoing>,
    pub incoming: mpsc::Receiver<DriverIncoming>,
}

struct SessionMediaPort {
    outgoing: mpsc::Receiver<DriverOutgoing>,
    incoming: mpsc::Sender<DriverIncoming>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Control {
    Recipients(Vec<String>),
    Beacon {
        muted: bool,
        camera_on: bool,
        sharing: bool,
    },
    RequestKeyframe(String),
    #[allow(dead_code)]
    Stop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Connecting,
    Live,
    PeerBeacon {
        peer: String,
        muted: bool,
        camera_on: bool,
        sharing: bool,
    },
    Closed,
    Failed(String),
}

pub struct Handle {
    pub control: mpsc::Sender<Control>,
    pub events: mpsc::Receiver<Event>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Handle {
    pub fn start(client: &NodeClient, channel: &str) -> Result<(Self, MediaDriverPort), String> {
        let url = call_url(client, channel)?;
        let (control_tx, control_rx) = mpsc::channel(CONTROL_QUEUE);
        let (event_tx, event_rx) = mpsc::channel(EVENT_QUEUE);
        let (driver_tx, outgoing) = mpsc::channel(DRIVER_QUEUE);
        let (incoming, driver_rx) = mpsc::channel(DRIVER_QUEUE);
        let task = tokio::spawn(run(
            url,
            control_rx,
            event_tx,
            SessionMediaPort { outgoing, incoming },
        ));
        Ok((
            Self {
                control: control_tx,
                events: event_rx,
                task,
            },
            MediaDriverPort {
                outgoing: driver_tx,
                incoming: driver_rx,
            },
        ))
    }
}

#[derive(Debug, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum ClientControl<'a> {
    Recipients {
        peers: &'a [String],
    },
    Beacon {
        muted: bool,
        camera_on: bool,
        sharing: bool,
    },
    KeyframeRequest {
        peer: &'a str,
    },
}

#[derive(Debug, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum ServerControl {
    KeyframeRequest,
    PeerBeacon {
        peer: String,
        muted: bool,
        camera_on: bool,
        #[serde(default)]
        sharing: bool,
    },
    RateHint {
        max_kbps: u32,
    },
}

async fn run(
    url: Url,
    mut controls: mpsc::Receiver<Control>,
    events: mpsc::Sender<Event>,
    mut media: SessionMediaPort,
) {
    let _ = events.send(Event::Connecting).await;
    let connected = tokio::time::timeout(
        CONNECT_TIMEOUT,
        tokio_tungstenite::connect_async(url.as_str()),
    )
    .await;
    let (socket, _) = match connected {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => {
            let _ = events
                .send(Event::Failed(format!("call connection failed: {error}")))
                .await;
            return;
        }
        Err(_) => {
            let _ = events
                .send(Event::Failed("call connection timed out".into()))
                .await;
            return;
        }
    };
    let (mut sink, mut stream) = socket.split();
    let mut live = false;
    loop {
        tokio::select! {
            control = controls.recv() => match control {
                Some(Control::Recipients(peers)) => {
                    if peers.len() > 64 || peers.iter().any(|peer| !is_key(peer)) {
                        let _ = events.send(Event::Failed("call recipients are invalid".into())).await;
                        break;
                    }
                    if !send_control(&mut sink, &ClientControl::Recipients { peers: &peers }).await {
                        break;
                    }
                }
                Some(Control::Beacon { muted, camera_on, sharing }) => {
                    if !send_control(&mut sink, &ClientControl::Beacon { muted, camera_on, sharing }).await {
                        break;
                    }
                }
                Some(Control::RequestKeyframe(peer)) => {
                    if !is_key(&peer) {
                        continue;
                    }
                    if !send_control(&mut sink, &ClientControl::KeyframeRequest { peer: &peer }).await {
                        break;
                    }
                }
                Some(Control::Stop) | None => {
                    let _ = sink.send(Message::Close(None)).await;
                    break;
                }
            },
            outgoing = media.outgoing.recv() => {
                // The media worker's Sender is dropped when it exits (e.g. mic
                // capture failed to start on a live call). Without this break
                // the closed channel is instantly Ready every poll and select!
                // busy-spins a full core. The sibling arms break on None too.
                let Some(frame) = outgoing else { break };
                let encoded = match encode_driver(frame) {
                    Ok(encoded) => encoded,
                    Err(error) => {
                        tracing::debug!(target: "ducktape::huddle", event = "media_frame_dropped", reason = "invalid_driver_frame", detail = %error);
                        continue;
                    }
                };
                if sink.send(Message::Binary(encoded)).await.is_err() {
                    break;
                }
            },
            incoming = stream.next() => match incoming {
                Some(Ok(Message::Binary(bytes))) => {
                    let Some(frame) = decode_server(&bytes) else {
                        tracing::debug!(target: "ducktape::huddle", event = "media_frame_dropped", reason = "invalid_server_frame");
                        continue;
                    };
                    if !live {
                        live = true;
                        let _ = events.send(Event::Live).await;
                    }
                    if media.incoming.try_send(frame).is_err() {
                        tracing::debug!(target: "ducktape::huddle", event = "media_frame_dropped", reason = "driver_backpressure");
                    }
                }
                Some(Ok(Message::Text(text))) => {
                    if text.len() > MAX_CONTROL_BYTES {
                        let _ = events.send(Event::Failed("call control frame is too large".into())).await;
                        break;
                    }
                    match serde_json::from_str::<ServerControl>(&text) {
                        Ok(control) => {
                            if !live {
                                live = true;
                                let _ = events.send(Event::Live).await;
                            }
                            handle_server_control(control, &events, &media.incoming).await;
                        }
                        Err(_) if !live => {
                            let note = text.chars().filter(|character| !character.is_control()).take(240).collect::<String>();
                            let _ = events.send(Event::Failed(if note.is_empty() { "call was refused".into() } else { note })).await;
                            break;
                        }
                        Err(_) => tracing::debug!(target: "ducktape::huddle", event = "control_frame_dropped", reason = "invalid_json"),
                    }
                }
                Some(Ok(Message::Ping(bytes))) => {
                    if sink.send(Message::Pong(bytes)).await.is_err() { break; }
                }
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                Some(Ok(_)) => {}
            }
        }
    }
    let _ = events.send(Event::Closed).await;
}

async fn send_control<S>(sink: &mut S, control: &ClientControl<'_>) -> bool
where
    S: futures_util::Sink<Message> + Unpin,
{
    let Ok(text) = serde_json::to_string(control) else {
        return false;
    };
    text.len() <= MAX_CONTROL_BYTES && sink.send(Message::Text(text)).await.is_ok()
}

async fn handle_server_control(
    control: ServerControl,
    events: &mpsc::Sender<Event>,
    driver: &mpsc::Sender<DriverIncoming>,
) {
    match control {
        ServerControl::KeyframeRequest => {
            let _ = driver.try_send(DriverIncoming::KeyframeRequested);
        }
        ServerControl::PeerBeacon {
            peer,
            muted,
            camera_on,
            sharing,
        } if is_key(&peer) => {
            let _ = events.try_send(Event::PeerBeacon {
                peer,
                muted,
                camera_on,
                sharing,
            });
        }
        ServerControl::PeerBeacon { .. } => {
            tracing::debug!(target: "ducktape::huddle", event = "control_frame_dropped", reason = "invalid_peer_key");
        }
        ServerControl::RateHint { max_kbps } => {
            let _ = driver.try_send(DriverIncoming::RateHint {
                max_kbps: max_kbps.clamp(300, 1_200),
            });
        }
    }
}

fn call_url(client: &NodeClient, channel: &str) -> Result<Url, String> {
    let channel = channel.trim();
    if channel.is_empty() || channel.len() > 128 || channel.chars().any(char::is_control) {
        return Err("huddle channel is invalid".into());
    }
    let mut url =
        Url::parse(&client.origin()).map_err(|_| "node address is invalid".to_string())?;
    let scheme = match url.scheme() {
        "http" => "ws",
        "https" => "wss",
        _ => return Err("node address cannot carry a call socket".into()),
    };
    url.set_scheme(scheme)
        .map_err(|_| "node address cannot carry a call socket".to_string())?;
    url.set_path("/v1/call/ws");
    url.set_query(None);
    url.query_pairs_mut().append_pair("channel", channel);
    Ok(url)
}

fn encode_driver(frame: DriverOutgoing) -> Result<Vec<u8>, String> {
    match frame {
        DriverOutgoing::Audio(samples) => {
            let mut bytes = Vec::with_capacity(1 + FRAME_SAMPLES * 2);
            bytes.push(TAG_AUDIO);
            for sample in samples.iter() {
                bytes.extend_from_slice(&sample.to_le_bytes());
            }
            Ok(bytes)
        }
        DriverOutgoing::Video {
            keyframe,
            timestamp_ms,
            vp8,
        } => {
            if vp8.is_empty() || vp8.len() > MAX_VIDEO_BYTES {
                return Err("captured video frame is empty or too large".into());
            }
            let mut bytes = Vec::with_capacity(CAPTURED_HEADER + vp8.len());
            bytes.push(TAG_VIDEO_CAPTURED);
            bytes.push(if keyframe { FLAG_KEYFRAME } else { 0 });
            bytes.extend_from_slice(&timestamp_ms.to_be_bytes());
            bytes.extend(vp8);
            Ok(bytes)
        }
    }
}

fn decode_server(bytes: &[u8]) -> Option<DriverIncoming> {
    if bytes.first() == Some(&TAG_AUDIO) && bytes.len() == 1 + FRAME_SAMPLES * 2 {
        let mut samples = Box::new([0i16; FRAME_SAMPLES]);
        for (sample, pair) in samples.iter_mut().zip(bytes[1..].chunks_exact(2)) {
            *sample = i16::from_le_bytes([pair[0], pair[1]]);
        }
        return Some(DriverIncoming::Audio(samples));
    }
    if bytes.first() != Some(&TAG_VIDEO_PEER)
        || bytes.len() <= PEER_HEADER
        || bytes.len() > PEER_HEADER + MAX_VIDEO_BYTES
    {
        return None;
    }
    let mut peer = [0u8; 32];
    peer.copy_from_slice(&bytes[6..38]);
    Some(DriverIncoming::Video {
        peer,
        keyframe: bytes[1] & FLAG_KEYFRAME != 0,
        timestamp_ms: u32::from_be_bytes(bytes[2..6].try_into().ok()?),
        vp8: bytes[PEER_HEADER..].to_vec(),
    })
}

fn is_key(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn audio_wire_is_exact_little_endian_pcm() {
        let mut samples = Box::new([0i16; FRAME_SAMPLES]);
        samples[0] = 1;
        samples[1] = -2;
        let bytes = encode_driver(DriverOutgoing::Audio(samples)).unwrap();
        assert_eq!(bytes.len(), 1 + FRAME_SAMPLES * 2);
        assert_eq!(&bytes[..5], &[TAG_AUDIO, 1, 0, 0xfe, 0xff]);
        assert!(matches!(
            decode_server(&bytes),
            Some(DriverIncoming::Audio(_))
        ));
    }

    #[test]
    fn video_wire_uses_big_endian_headers() {
        let bytes = encode_driver(DriverOutgoing::Video {
            keyframe: true,
            timestamp_ms: 0x0102_0304,
            vp8: vec![0xaa],
        })
        .unwrap();
        assert_eq!(bytes, [TAG_VIDEO_CAPTURED, FLAG_KEYFRAME, 1, 2, 3, 4, 0xaa]);

        let mut peer = vec![TAG_VIDEO_PEER, 0, 0x0a, 0x0b, 0x0c, 0x0d];
        peer.extend([0x11; 32]);
        peer.push(0xf0);
        assert!(matches!(
            decode_server(&peer),
            Some(DriverIncoming::Video {
                timestamp_ms: 0x0a0b_0c0d,
                ..
            })
        ));
    }

    #[test]
    fn control_wire_matches_generated_camel_case_contract() {
        assert_eq!(
            serde_json::to_value(ClientControl::Beacon {
                muted: true,
                camera_on: false,
                sharing: true,
            })
            .unwrap(),
            json!({ "type": "beacon", "muted": true, "cameraOn": false, "sharing": true })
        );
        assert!(
            serde_json::from_value::<ServerControl>(json!({
                "type": "peerBeacon",
                "peer": "ab".repeat(32),
                "muted": false,
                "cameraOn": true,
                "sharing": false
            }))
            .is_ok()
        );
    }

    #[test]
    fn untrusted_frames_are_bounded() {
        assert!(
            encode_driver(DriverOutgoing::Video {
                keyframe: false,
                timestamp_ms: 0,
                vp8: Vec::new(),
            })
            .is_err()
        );
        assert!(decode_server(&[TAG_VIDEO_PEER; PEER_HEADER]).is_none());
        assert!(decode_server(&[TAG_AUDIO; 10]).is_none());
    }
}
