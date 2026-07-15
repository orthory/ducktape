//! Bounded off-consensus Pages cursor presence over `/v1/presence/ws`.

use std::collections::BTreeSet;
use std::time::Duration;

use futures_util::{SinkExt as _, StreamExt as _};
use reqwest::Url;
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use crate::screens::user::PagePresence;
use crate::transport::NodeClient;

const MAX_CONTROL_BYTES: usize = 4 * 1024;
const QUEUE: usize = 32;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(12);
const RECONNECT_DELAY: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Control {
    Cursor {
        block: Option<String>,
        anchor: usize,
        head: usize,
    },
    Stop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Peer(PagePresence),
    Failed(String),
    Closed,
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
    pub fn start(client: &NodeClient, page: &str) -> Result<Self, String> {
        let url = presence_url(client, page)?;
        let client = client.clone();
        let (control, controls) = mpsc::channel(QUEUE);
        let (events, event_rx) = mpsc::channel(QUEUE);
        let task = tokio::spawn(run(client, url, controls, events));
        Ok(Self {
            control,
            events: event_rx,
            task,
        })
    }
}

async fn run(
    client: NodeClient,
    url: Url,
    mut controls: mpsc::Receiver<Control>,
    events: mpsc::Sender<Event>,
) {
    let (recipients, self_key) = recipients(&client).await.unwrap_or_default();
    let mut cursor = (None, 0, 0);
    let mut failed_attempts = 0_u64;
    'connect: loop {
        let connected = tokio::time::timeout(
            CONNECT_TIMEOUT,
            tokio_tungstenite::connect_async(url.as_str()),
        )
        .await;
        let (socket, _) = match connected {
            Ok(Ok(socket)) => socket,
            _ => {
                failed_attempts = failed_attempts.saturating_add(1);
                if failed_attempts == 1 || failed_attempts.is_multiple_of(20) {
                    let _ = events.try_send(Event::Failed(format!(
                        "page presence connection failed after {failed_attempts} attempts"
                    )));
                }
                if reconnect(&mut controls, &mut cursor).await {
                    continue;
                }
                break;
            }
        };
        let (mut sink, mut stream) = socket.split();
        if !send(
            &mut sink,
            json!({ "type": "recipients", "peers": recipients }),
        )
        .await
            || !send_cursor(&mut sink, &cursor).await
        {
            failed_attempts = failed_attempts.saturating_add(1);
            if failed_attempts == 1 || failed_attempts.is_multiple_of(20) {
                let _ = events.try_send(Event::Failed(format!(
                    "page presence handshake failed after {failed_attempts} attempts"
                )));
            }
            if reconnect(&mut controls, &mut cursor).await {
                continue;
            }
            break;
        }
        failed_attempts = 0;
        loop {
            tokio::select! {
                control = controls.recv() => match control {
                    Some(Control::Cursor { block, anchor, head }) if valid_cursor(block.as_deref(), anchor, head) => {
                        cursor = (block, anchor, head);
                        if !send_cursor(&mut sink, &cursor).await { break; }
                    }
                    Some(Control::Cursor { .. }) => {}
                    Some(Control::Stop) | None => {
                        let _ = sink.send(Message::Close(None)).await;
                        break 'connect;
                    }
                },
                incoming = stream.next() => match incoming {
                    Some(Ok(Message::Text(text))) if text.len() <= MAX_CONTROL_BYTES => {
                        if let Some(peer) = parse_peer_cursor(&text)
                            && self_key.as_ref().is_none_or(|self_key| self_key != &peer.peer)
                        {
                            let _ = events.try_send(Event::Peer(peer));
                        }
                    }
                    Some(Ok(Message::Ping(bytes))) => {
                        if sink.send(Message::Pong(bytes)).await.is_err() { break; }
                    }
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                    Some(Ok(_)) => {}
                }
            }
        }
        if !reconnect(&mut controls, &mut cursor).await {
            break;
        }
    }
    let _ = events.send(Event::Closed).await;
}

async fn reconnect(
    controls: &mut mpsc::Receiver<Control>,
    cursor: &mut (Option<String>, usize, usize),
) -> bool {
    loop {
        tokio::select! {
            _ = tokio::time::sleep(RECONNECT_DELAY) => return true,
            control = controls.recv() => match control {
                Some(Control::Cursor { block, anchor, head }) if valid_cursor(block.as_deref(), anchor, head) => {
                    *cursor = (block, anchor, head);
                }
                Some(Control::Cursor { .. }) => {}
                Some(Control::Stop) | None => return false,
            }
        }
    }
}

async fn send_cursor<S>(sink: &mut S, cursor: &(Option<String>, usize, usize)) -> bool
where
    S: futures_util::Sink<Message> + Unpin,
{
    send(
        sink,
        json!({ "type": "cursor", "blockId": cursor.0, "anchor": cursor.1, "head": cursor.2 }),
    )
    .await
}

async fn send<S>(sink: &mut S, value: Value) -> bool
where
    S: futures_util::Sink<Message> + Unpin,
{
    let text = value.to_string();
    text.len() <= MAX_CONTROL_BYTES && sink.send(Message::Text(text)).await.is_ok()
}

fn parse_peer_cursor(text: &str) -> Option<PagePresence> {
    let value: Value = serde_json::from_str(text).ok()?;
    if value.get("type")?.as_str()? != "peerCursor" {
        return None;
    }
    let peer = value.get("peer")?.as_str()?.to_ascii_lowercase();
    let block = match value.get("blockId")? {
        Value::Null => None,
        Value::String(block) if !block.is_empty() && block.len() <= 256 => Some(block.clone()),
        _ => return None,
    };
    let anchor = usize::try_from(value.get("anchor")?.as_u64()?).ok()?;
    let head = usize::try_from(value.get("head")?.as_u64()?).ok()?;
    (is_key(&peer) && valid_cursor(block.as_deref(), anchor, head)).then_some(PagePresence {
        peer,
        block,
        anchor,
        head,
    })
}

async fn recipients(client: &NodeClient) -> Result<(Vec<String>, Option<String>), String> {
    let status = client.status().await.map_err(|error| error.to_string())?;
    let self_key = status.public_key.map(|key| key.to_ascii_lowercase());
    let mut peers = BTreeSet::new();
    for variant in ["validators", "residents"] {
        let reply = client
            .query("valset", Value::String(variant.into()))
            .await
            .map_err(|error| error.to_string())?;
        let rows = reply
            .get(variant)
            .and_then(Value::as_array)
            .ok_or_else(|| "node returned an invalid presence recipient list".to_string())?;
        if rows.len() > 512 {
            return Err("node returned too many presence recipients".into());
        }
        for row in rows {
            let bytes = row
                .as_array()
                .ok_or_else(|| "invalid presence key".to_string())?;
            if bytes.len() != 32 {
                return Err("invalid presence key".into());
            }
            let mut key = String::with_capacity(64);
            for byte in bytes {
                use std::fmt::Write as _;
                write!(
                    &mut key,
                    "{:02x}",
                    byte.as_u64()
                        .filter(|byte| *byte <= 255)
                        .ok_or_else(|| "invalid presence key".to_string())?
                )
                .unwrap();
            }
            if self_key.as_ref() != Some(&key) {
                peers.insert(key);
            }
        }
    }
    Ok((peers.into_iter().collect(), self_key))
}

fn presence_url(client: &NodeClient, page: &str) -> Result<Url, String> {
    if page.is_empty() || page.len() > 256 || page.chars().any(char::is_control) {
        return Err("page presence id is invalid".into());
    }
    let mut url =
        Url::parse(&client.origin()).map_err(|_| "node address is invalid".to_string())?;
    let scheme = match url.scheme() {
        "http" => "ws",
        "https" => "wss",
        _ => return Err("node address cannot carry page presence".into()),
    };
    url.set_scheme(scheme)
        .map_err(|_| "node address cannot carry page presence".to_string())?;
    url.set_path("/v1/presence/ws");
    url.set_query(None);
    url.query_pairs_mut().append_pair("page", page);
    Ok(url)
}

fn valid_cursor(block: Option<&str>, anchor: usize, head: usize) -> bool {
    block.is_none_or(|block| !block.is_empty() && block.len() <= 256)
        && anchor <= 1_000_000
        && head <= 1_000_000
}

fn is_key(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peer_cursor_wire_is_strict_and_bounded() {
        let peer = "ab".repeat(32);
        assert_eq!(
            parse_peer_cursor(&json!({ "type": "peerCursor", "peer": peer, "blockId": "b1", "anchor": 2, "head": 5 }).to_string()),
            Some(PagePresence { peer: "ab".repeat(32), block: Some("b1".into()), anchor: 2, head: 5 })
        );
        assert!(
            parse_peer_cursor(
                r#"{"type":"peerCursor","peer":"bad","blockId":null,"anchor":0,"head":0}"#
            )
            .is_none()
        );
    }
}
