//! The kernel contract: what EVERY module view may ask of the app, with no
//! per-module code on this side of the wire. A view that speaks only this
//! contract is replaced by a module deployment alone — the app binary
//! never changes for it.
//!
//! - `rpc.query` `{target, query}` — one module query on the connected
//!   node, answered with the reply's JSON; `rpc.view` the same against
//!   the module's index-tier view.
//! - `rpc.blocks` `{limit}` — the recent block feed; `rpc.status` and
//!   `rpc.peers` the node's own status and peers JSON.
//! - `rpc.live` `<plane>` — a subscription that gets one item per block
//!   the app's live stream reports for that plane: a module's name
//!   ([`live_hit`], any module — an audited view is trusted to read what
//!   it names), or `block` for every block ([`block_hit`]), so the view
//!   re-reads what moved.
//! - `rpc.stream` `{topic, params}` — a subscription on ONE of the node's
//!   own event topics, opened over `/v1/ws` with `params` as the upgrade's
//!   query and the SEATED key as the proof on it. Every frame the node
//!   sends arrives as one item, byte for byte; the close is the item that
//!   ends it. The kernel reads neither the topic nor the frames — the node
//!   decides what this key may hear.
//! - `rpc.query_as_reader` `{target, query}` — the query AS THE SEATED
//!   KEY'S HOLDER (`/v1/query/reader`), for a module that serves protected
//!   content to its reader; refused while the key is locked.
//! - `files.get` `{lane, params}` — one `/v1/files/<lane>` read with its
//!   query-string params; `blob.get` `{digest, limit}` — a blob by hex
//!   digest, verified against it.
//! - `picture.load` `{surface, path}` — a duckfs file paged in, decoded and
//!   parked in a host picture surface's slot; answered with its drawn size.
//! - `files.stage` `<raw bytes>` / `blob.put` `<raw bytes>` — a duckfs
//!   chunk or a blob landed on the node, proven with the seated key;
//!   answered with the digest.
//! - `op.submit` `{target, payload}` — one module op, signed with the
//!   SEATED key and submitted; answered with the block height. The view
//!   never carries a password, an endpoint or a key.
//! - `host.badge` `<count>` — the tab badge, handed to the app as the
//!   `badge` event with `{"count": N}` in its detail.
//!
//! A query and a submit go to the node off the window thread, on the
//! kernel's own runtime, and their answers wait in [`Replies`] for the
//! view's next redraw; the widget polls while any is in flight.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use super::{Guest, ModuleViewEvent, Slot, wire};

/// The most blocks one `rpc.blocks` may ask for.
const MAX_BLOCKS: usize = 1_000;
/// The most a `blob.get` may pull: a frame's worth, as the loader's own cap.
const MAX_BLOB_BYTES: usize = 16 << 20;
/// The most one `rpc.stream` frame may carry into a view. A node frame is a
/// line of a run's output or the like; a frame past this is the node
/// misbehaving, and the subscription ends rather than growing the guest.
const MAX_STREAM_FRAME_BYTES: usize = 1 << 20;

/// The kernel's answers to a view's requests, written off-thread and
/// drained into the guest's pending events at its next redraw.
#[derive(Default)]
pub(super) struct Replies {
    events: Mutex<Vec<wire::Event>>,
    in_flight: AtomicUsize,
    /// Told on every answer delivered: a test waits here for the node
    /// calls in flight, never on a clock.
    landed: std::sync::Condvar,
}

impl Replies {
    pub(super) fn drain_into(&self, pending: &mut Vec<wire::Event>) {
        let mut events = self.events.lock().expect("kernel replies");
        pending.append(&mut events);
    }

    /// Whether a query or a submit is still on its way: the widget keeps
    /// polling until none is.
    pub(super) fn any_in_flight(&self) -> bool {
        self.in_flight.load(Ordering::SeqCst) > 0
    }

    /// Blocks until nothing is in flight.
    #[cfg(test)]
    pub(super) fn wait_idle(&self) {
        let mut events = self.events.lock().expect("kernel replies");
        while self.any_in_flight() {
            events = self.landed.wait(events).expect("kernel replies");
        }
    }

    /// One item for a request the kernel is running; `done` ends it for the
    /// guest. The in-flight count is [`Replies::settled`]'s to give back —
    /// a subscription's last item and its count are not the same moment.
    fn item(&self, id: u64, result: Result<Vec<u8>, String>, done: bool) {
        let mut events = self.events.lock().expect("kernel replies");
        events.push(wire::Event::Response { id, result, done });
        self.landed.notify_all();
    }

    /// One request off the in-flight count, under the lock a waiter holds.
    fn settled(&self) {
        let _events = self.events.lock().expect("kernel replies");
        self.in_flight.fetch_sub(1, Ordering::SeqCst);
        self.landed.notify_all();
    }

    fn deliver(&self, id: u64, result: Result<Vec<u8>, String>) {
        self.item(id, result, true);
        self.settled();
    }
}

/// The in-flight count one subscription took, given back when its task
/// ends — INCLUDING THE ABORT a cancel or a replaced view fires, which is
/// the only way a socket waiting on the node stops waiting. Without this
/// the count would outlive the socket and the widget would poll forever.
struct InFlight(std::sync::Arc<Replies>);

impl Drop for InFlight {
    fn drop(&mut self) {
        self.0.settled();
    }
}

/// The kernel's own runtime, on its own thread: the window thread never
/// blocks on the node, and the app's executor is not this module's to use.
fn runtime() -> tokio::runtime::Handle {
    static HANDLE: OnceLock<tokio::runtime::Handle> = OnceLock::new();
    HANDLE
        .get_or_init(|| {
            let (send, recv) = std::sync::mpsc::channel();
            std::thread::Builder::new()
                .name("views-kernel".into())
                .spawn(move || {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("the views kernel runtime");
                    send.send(runtime.handle().clone())
                        .expect("the kernel handle is taken");
                    runtime.block_on(std::future::pending::<()>());
                })
                .expect("the views kernel thread");
            recv.recv().expect("the kernel runtime came up")
        })
        .clone()
}

/// Routes one kernel request; `false` when the kind is not the kernel's.
pub(super) fn answer(
    guest: &mut Guest,
    capability: &str,
    operation: &str,
    id: u64,
    payload: &[u8],
) -> bool {
    match (capability, operation) {
        ("rpc", "query") => spawn(guest, id, payload, query),
        ("rpc", "view") => spawn(guest, id, payload, view),
        ("rpc", "blocks") => spawn(guest, id, payload, blocks),
        ("rpc", "status") => spawn(guest, id, b"{}", status),
        ("rpc", "peers") => spawn(guest, id, b"{}", peers),
        ("rpc", "query_as_reader") => spawn(guest, id, payload, query_as_reader),
        ("rpc", "stream") => stream_open(guest, id, payload),
        ("files", "get") => spawn(guest, id, payload, files_get),
        ("files", "stage") => spawn_raw(guest, id, payload, files_stage),
        ("picture", "load") => spawn(guest, id, payload, picture_load),
        ("blob", "get") => spawn(guest, id, payload, blob_get),
        ("blob", "put") => spawn_raw(guest, id, payload, blob_put),
        ("rpc", "live") => {
            let plane = std::str::from_utf8(payload).unwrap_or_default().trim();
            let named = plane == BLOCK_PLANE || workspace_config::validate_module_id(plane).is_ok();
            match named {
                true => guest.live_subscriptions.push((id, plane.to_owned())),
                false => guest.refuse(id, "`rpc.live` names no plane".into()),
            }
        }
        ("op", "submit") => spawn(guest, id, payload, submit),
        ("host", "badge") => {
            let count = std::str::from_utf8(payload)
                .ok()
                .and_then(|text| text.trim().parse::<i64>().ok());
            match count {
                Some(count) => {
                    guest.intents.push(ModuleViewEvent {
                        kind: "badge".into(),
                        detail: format!("{{\"count\":{count}}}"),
                    });
                    guest.reply(id, Ok(Vec::new()));
                }
                None => guest.refuse(id, "`host.badge` carries no count".into()),
            }
        }
        _ => return false,
    }
    true
}

type Call = fn(
    ducktape_rpc::Client,
    serde_json::Value,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, String>> + Send>>;

/// Runs one node call for a request: decoded here, answered on the kernel
/// runtime, delivered at the guest's next redraw.
fn spawn(guest: &mut Guest, id: u64, payload: &[u8], call: Call) {
    let ask: serde_json::Value = match serde_json::from_slice(payload) {
        Ok(ask) => ask,
        Err(error) => {
            guest.refuse(id, format!("request is not JSON: {error}"));
            return;
        }
    };
    let client = super::connection()
        .lock()
        .expect("views rpc")
        .client
        .clone();
    let Some(client) = client else {
        guest.refuse(id, "not connected to a node".into());
        return;
    };
    let replies = guest.replies.clone();
    replies.in_flight.fetch_add(1, Ordering::SeqCst);
    runtime().spawn(async move {
        let result = call(client, ask).await;
        replies.deliver(id, result);
    });
}

type RawCall = fn(
    ducktape_rpc::Client,
    Vec<u8>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, String>> + Send>>;

/// [`spawn`] for a request whose payload is the bytes themselves.
fn spawn_raw(guest: &mut Guest, id: u64, payload: &[u8], call: RawCall) {
    let client = super::connection()
        .lock()
        .expect("views rpc")
        .client
        .clone();
    let Some(client) = client else {
        guest.refuse(id, "not connected to a node".into());
        return;
    };
    let bytes = payload.to_vec();
    let replies = guest.replies.clone();
    replies.in_flight.fetch_add(1, Ordering::SeqCst);
    runtime().spawn(async move {
        let result = call(client, bytes).await;
        replies.deliver(id, result);
    });
}

/// A node stream the kernel is running for one subscription. Dropped with
/// the guest that asked, or with the cancel that retires it — and dropping
/// it ends the socket, so a view that is replaced leaves nothing reading.
pub(super) struct NodeStream(tokio::task::JoinHandle<()>);

impl Drop for NodeStream {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// `{topic, params}` read once: the topic to ask the node for, and the
/// `/v1/ws` query the signature will cover. A param that is not a plain
/// token is REFUSED rather than escaped — the signed string and the
/// requested string must be the same one, and a view has nothing to name
/// here that is not a token.
fn stream_ask(ask: &serde_json::Value) -> Result<(String, String), String> {
    let plain = |text: &str| {
        !text.is_empty()
            && text
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_.:~".contains(&byte))
    };
    let topic = ask["topic"].as_str().unwrap_or_default();
    if !plain(topic) {
        return Err("`rpc.stream` names no topic".into());
    }
    let mut query = String::new();
    for (key, value) in ask["params"].as_object().into_iter().flatten() {
        let value = value.as_str().unwrap_or_default();
        if !plain(key) || !plain(value) {
            return Err("`rpc.stream` takes plain token params".into());
        }
        let separator = match query.is_empty() {
            true => '?',
            false => '&',
        };
        query.push(separator);
        query.push_str(key);
        query.push('=');
        query.push_str(value);
    }
    Ok((topic.to_owned(), query))
}

/// Opens one node topic for a view: the socket under the seated key, then
/// every frame it sends, until it ends.
fn stream_open(guest: &mut Guest, id: u64, payload: &[u8]) {
    let ask: serde_json::Value = match serde_json::from_slice(payload) {
        Ok(ask) => ask,
        Err(error) => {
            guest.refuse(id, format!("request is not JSON: {error}"));
            return;
        }
    };
    let (topic, query) = match stream_ask(&ask) {
        Ok(named) => named,
        Err(refusal) => {
            guest.refuse(id, refusal);
            return;
        }
    };
    let client = super::connection()
        .lock()
        .expect("views rpc")
        .client
        .clone();
    let Some(client) = client else {
        guest.refuse(id, "not connected to a node".into());
        return;
    };
    let replies = guest.replies.clone();
    replies.in_flight.fetch_add(1, Ordering::SeqCst);
    let counted = InFlight(replies.clone());
    let handle = runtime().spawn(async move {
        let _counted = counted;
        match open_topic(client.origin(), &topic, &query).await {
            Ok(socket) => forward(&replies, id, socket).await,
            Err(error) => replies.item(id, Err(error), true),
        }
    });
    guest.streams.push((id, NodeStream(handle)));
}

/// The node's own event socket for ONE topic, proven with the SEATED key:
/// the signature rides the upgrade, over the same path the request carries,
/// and the node admits or refuses this key for that topic before the socket
/// exists. Nothing here is per-topic — the app's run-output watcher presents
/// exactly this proof, and this is that door with the run taken out of it.
async fn open_topic(
    rpc: &str,
    topic: &str,
    query: &str,
) -> Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    String,
> {
    use iced::futures::SinkExt as _;
    use tokio_tungstenite::tungstenite::Message;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest as _;
    use tokio_tungstenite::tungstenite::http::{HeaderName, HeaderValue};
    let node_key = crate::backend::node_public_key(rpc).await?;
    let signed =
        crate::backend::seated_request_headers("GET", &format!("/v1/ws{query}"), &node_key, b"")
            .await
            .ok_or_else(|| "`rpc.stream` needs the session key unlocked".to_owned())?;
    let mut request = format!("{}{query}", crate::backend::agent_ws_url(rpc))
        .into_client_request()
        .map_err(|error| format!("could not address the node: {error}"))?;
    for (name, value) in signed {
        let value = HeaderValue::from_str(&value)
            .map_err(|error| format!("the signature is not a header value: {error}"))?;
        request
            .headers_mut()
            .insert(HeaderName::from_static(name), value);
    }
    let (mut socket, _) = tokio_tungstenite::connect_async(request)
        .await
        .map_err(|error| format!("could not open the node stream: {error}"))?;
    let subscribe = serde_json::json!({"op": "subscribe", "topics": [topic]});
    socket
        .send(Message::Text(subscribe.to_string()))
        .await
        .map_err(|error| format!("could not subscribe to the node stream: {error}"))?;
    Ok(socket)
}

/// Every frame an open node socket sends, as one item each, verbatim: the
/// kernel never reads a frame, so a topic it has never heard of needs no
/// code here. The node ending the socket is the `done` that ends the
/// subscription.
async fn forward<S>(replies: &Replies, id: u64, mut socket: S)
where
    S: iced::futures::Stream<
            Item = Result<
                tokio_tungstenite::tungstenite::Message,
                tokio_tungstenite::tungstenite::Error,
            >,
        > + Unpin,
{
    use iced::futures::StreamExt as _;
    use tokio_tungstenite::tungstenite::Message;
    while let Some(message) = socket.next().await {
        let frame = match message {
            Ok(Message::Text(text)) => text.into_bytes(),
            Ok(Message::Binary(bytes)) => bytes,
            Ok(Message::Close(_)) => break,
            Ok(_) => continue,
            Err(error) => {
                replies.item(id, Err(format!("the node stream failed: {error}")), true);
                return;
            }
        };
        if frame.len() > MAX_STREAM_FRAME_BYTES {
            replies.item(
                id,
                Err(format!(
                    "a node stream frame carries more than {MAX_STREAM_FRAME_BYTES} bytes"
                )),
                true,
            );
            return;
        }
        replies.item(id, Ok(frame), false);
    }
    replies.item(id, Ok(Vec::new()), true);
}

fn query_as_reader(
    client: ducktape_rpc::Client,
    ask: serde_json::Value,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, String>> + Send>> {
    Box::pin(async move {
        let target = target_of(&ask)?;
        let signer = crate::backend::seated_data_plane_signer(&client).await?;
        let reply: serde_json::Value = client
            .with_write_auth(signer)
            .query_as_reader(&target, &ask["query"])
            .await
            .map_err(|error| error.to_string())?;
        serde_json::to_vec(&reply).map_err(|error| error.to_string())
    })
}

fn files_get(
    client: ducktape_rpc::Client,
    ask: serde_json::Value,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, String>> + Send>> {
    Box::pin(async move {
        let lane = ask["lane"].as_str().unwrap_or_default();
        let lane_named = !lane.is_empty()
            && lane
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-');
        if !lane_named {
            return Err("`files.get` names no lane".into());
        }
        let params: Vec<(String, String)> = ask["params"]
            .as_object()
            .map(|params| {
                params
                    .iter()
                    .map(|(key, value)| {
                        let value = match value {
                            serde_json::Value::String(text) => text.clone(),
                            other => other.to_string(),
                        };
                        (key.clone(), value)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let params: Vec<(&str, &str)> = params
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();
        let reply = client
            .files_get(lane, &params)
            .await
            .map_err(|error| error.to_string())?;
        serde_json::to_vec(&reply).map_err(|error| error.to_string())
    })
}

/// `picture.load` `{surface, path}` — the whole duckfs file paged in, decoded
/// off the runtime and parked in a host picture surface's slot, answered with
/// the size it will be drawn at. The decode and the widget are the host's (a
/// tree wire carries no pixels), so a view that wants one names the slot it
/// left and the file to put in it. The two slots are the ones
/// [`crate::backend::picture`] draws; anything else is refused rather than
/// growing the store.
fn picture_load(
    client: ducktape_rpc::Client,
    ask: serde_json::Value,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, String>> + Send>> {
    use crate::backend::{FILES_SURFACE, FORGE_SURFACE, MAX_PICTURE_BYTES, store_picture};
    Box::pin(async move {
        let surface = match ask["surface"].as_str() {
            Some(FILES_SURFACE) => FILES_SURFACE,
            Some(FORGE_SURFACE) => FORGE_SURFACE,
            _ => return Err("`picture.load` names no surface".into()),
        };
        let path = ask["path"].as_str().unwrap_or_default().to_owned();
        let Some(bytes) = crate::backend::files_read_all(&client, &path).await? else {
            return Err(format!(
                "picture larger than the {} MiB preview limit",
                MAX_PICTURE_BYTES >> 20
            ));
        };
        let size = bytes.len();
        let (width, height) = store_picture(surface, path, bytes)
            .await
            .map_err(|reason| format!("{size} binary bytes · did not decode: {reason}"))?;
        serde_json::to_vec(&serde_json::json!({ "width": width, "height": height }))
            .map_err(|error| error.to_string())
    })
}

fn files_stage(
    client: ducktape_rpc::Client,
    bytes: Vec<u8>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, String>> + Send>> {
    Box::pin(async move {
        let signer = crate::backend::seated_data_plane_signer(&client).await?;
        let digest = client
            .with_write_auth(signer)
            .files_stage(bytes)
            .await
            .map_err(|error| error.to_string())?;
        Ok(digest.into_bytes())
    })
}

fn blob_put(
    client: ducktape_rpc::Client,
    bytes: Vec<u8>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, String>> + Send>> {
    Box::pin(async move {
        let signer = crate::backend::seated_data_plane_signer(&client).await?;
        let digest = client
            .with_write_auth(signer)
            .put_blob(bytes)
            .await
            .map_err(|error| error.to_string())?;
        Ok(digest.into_bytes())
    })
}

fn blob_get(
    client: ducktape_rpc::Client,
    ask: serde_json::Value,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, String>> + Send>> {
    Box::pin(async move {
        let digest = crate::backend::hex_decode(ask["digest"].as_str().unwrap_or_default())?;
        let digest: [u8; 32] = digest
            .try_into()
            .map_err(|_| "`blob.get` digest is not 32 bytes".to_owned())?;
        let limit = ask["limit"].as_u64().unwrap_or(0) as usize;
        client
            .get_blob(&digest, limit.clamp(1, MAX_BLOB_BYTES))
            .await
            .map_err(|error| error.to_string())
    })
}

fn target_of(ask: &serde_json::Value) -> Result<String, String> {
    let target = ask["target"].as_str().unwrap_or_default();
    workspace_config::validate_module_id(target)
        .map_err(|error| format!("request names no module target: {error}"))?;
    Ok(target.to_owned())
}

fn query(
    client: ducktape_rpc::Client,
    ask: serde_json::Value,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, String>> + Send>> {
    Box::pin(async move {
        let target = target_of(&ask)?;
        let reply: serde_json::Value = client
            .query(&target, &ask["query"])
            .await
            .map_err(|error| error.to_string())?;
        serde_json::to_vec(&reply).map_err(|error| error.to_string())
    })
}

fn view(
    client: ducktape_rpc::Client,
    ask: serde_json::Value,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, String>> + Send>> {
    Box::pin(async move {
        let target = target_of(&ask)?;
        let reply: serde_json::Value = client
            .view(&target, &ask["query"])
            .await
            .map_err(|error| error.to_string())?;
        serde_json::to_vec(&reply).map_err(|error| error.to_string())
    })
}

fn status(
    client: ducktape_rpc::Client,
    _ask: serde_json::Value,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, String>> + Send>> {
    Box::pin(async move {
        let reply = client
            .status_json()
            .await
            .map_err(|error| error.to_string())?;
        serde_json::to_vec(&reply).map_err(|error| error.to_string())
    })
}

fn peers(
    client: ducktape_rpc::Client,
    _ask: serde_json::Value,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, String>> + Send>> {
    Box::pin(async move {
        let reply = client.peers().await.map_err(|error| error.to_string())?;
        serde_json::to_vec(&reply).map_err(|error| error.to_string())
    })
}

fn blocks(
    client: ducktape_rpc::Client,
    ask: serde_json::Value,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, String>> + Send>> {
    Box::pin(async move {
        let limit = ask["limit"].as_u64().unwrap_or(0) as usize;
        let blocks = client
            .blocks(limit.clamp(1, MAX_BLOCKS))
            .await
            .map_err(|error| error.to_string())?;
        serde_json::to_vec(&blocks).map_err(|error| error.to_string())
    })
}

fn submit(
    client: ducktape_rpc::Client,
    ask: serde_json::Value,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, String>> + Send>> {
    Box::pin(async move {
        let target = target_of(&ask)?;
        let payload = serde_json::to_vec(&ask["payload"]).map_err(|error| error.to_string())?;
        let height = crate::backend::seated_write(&client, &target, payload).await?;
        Ok(height.to_string().into_bytes())
    })
}

/// The plane every block moves: a view that reads the feed itself
/// subscribes to it.
const BLOCK_PLANE: &str = "block";

/// A block moved `plane` (a module's, or [`BLOCK_PLANE`]): every `rpc.live`
/// subscription on it, in whichever view holds it, gets one item. Answers
/// `serial + 1` when a view was told — the app keeps that number in its
/// state, so the redraw that delivers the item follows — and `serial` when
/// none was.
pub fn live_hit(plane: &str, serial: i64) -> i64 {
    // lock order, everywhere: registry, then a view
    let registry = super::registry().lock().expect("module views");
    let mut told = false;
    for mounted in registry.values() {
        let mut locked = mounted.lock().expect("module view lock");
        let Slot::Ready(guest) = &mut locked.slot else {
            continue;
        };
        let ids: Vec<u64> = guest
            .live_subscriptions
            .iter()
            .filter(|(_, subscribed)| subscribed == plane)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            guest.pending.push(wire::Event::Response {
                id,
                result: Ok(b"{}".to_vec()),
                done: false,
            });
            told = true;
        }
    }
    match told {
        true => serial + 1,
        false => serial,
    }
}

/// The node's height as the app last heard it: a height that moved is a
/// hit on [`BLOCK_PLANE`]; the same height again, or none, is not.
pub fn block_hit(height: i64, serial: i64) -> i64 {
    static LAST: Mutex<i64> = Mutex::new(-1);
    let mut last = LAST.lock().expect("last height");
    let moved = height >= 0 && height != *last;
    if !moved {
        return serial;
    }
    *last = height;
    drop(last);
    live_hit(BLOCK_PLANE, serial)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_tungstenite::tungstenite::Message;

    /// The two strings a `rpc.stream` ask becomes, and what it refuses: the
    /// query is built ONCE here because the signature covers exactly the
    /// string the upgrade carries, and anything that would have to be
    /// escaped to survive that round trip is refused instead of escaped.
    #[test]
    fn a_stream_ask_becomes_one_topic_and_one_query_or_a_refusal() {
        let ask = serde_json::json!({
            "topic": "run-output:d4c3",
            "params": {"run": "d4c3"},
        });
        assert_eq!(
            stream_ask(&ask).expect("a plain ask"),
            ("run-output:d4c3".to_owned(), "?run=d4c3".to_owned())
        );
        assert_eq!(
            stream_ask(&serde_json::json!({"topic": "logs"})).expect("no params"),
            ("logs".to_owned(), String::new())
        );
        assert!(stream_ask(&serde_json::json!({"params": {"run": "a"}})).is_err());
        assert!(stream_ask(&serde_json::json!({"topic": ""})).is_err());
        assert!(stream_ask(&serde_json::json!({"topic": "a b"})).is_err());
        let smuggled = serde_json::json!({"topic": "logs", "params": {"run": "a&admin=1"}});
        assert!(
            stream_ask(&smuggled).is_err(),
            "a param that would need escaping is refused, never escaped"
        );
    }

    /// Every frame the node sends reaches the view verbatim, the kernel
    /// reading none of it, and the node's close is the item that ends the
    /// subscription — nothing after it is forwarded, and the in-flight count
    /// the open took comes back, which is what the test waits on.
    #[test]
    fn a_node_stream_forwards_every_frame_and_the_close_ends_it() {
        let replies = std::sync::Arc::new(Replies::default());
        replies.in_flight.fetch_add(1, Ordering::SeqCst);
        let frames = iced::futures::stream::iter(vec![
            Ok(Message::Text(r#"{"topic":"run-output:d4c3"}"#.to_owned())),
            Ok(Message::Ping(Vec::new())),
            Ok(Message::Binary(vec![7, 8])),
            Ok(Message::Close(None)),
            Ok(Message::Text("after the close".to_owned())),
        ]);
        let running = replies.clone();
        let counted = InFlight(replies.clone());
        runtime().spawn(async move {
            let _counted = counted;
            forward(&running, 7, frames).await;
        });

        replies.wait_idle();
        let mut landed = Vec::new();
        replies.drain_into(&mut landed);
        let items: Vec<(u64, Result<Vec<u8>, String>, bool)> = landed
            .into_iter()
            .map(|event| match event {
                wire::Event::Response { id, result, done } => (id, result, done),
                other => panic!("the stream delivered {other:?}"),
            })
            .collect();
        assert_eq!(
            items,
            vec![
                (7, Ok(br#"{"topic":"run-output:d4c3"}"#.to_vec()), false),
                (7, Ok(vec![7, 8]), false),
                (7, Ok(Vec::new()), true),
            ]
        );
        assert!(!replies.any_in_flight());
    }

    /// A subscription the view abandons is aborted mid-wait — a socket
    /// waiting on the node stops no other way — and the in-flight count it
    /// took comes back with it. It must, or the widget polls for a stream
    /// nobody is reading for the rest of the process.
    #[test]
    fn a_cancelled_stream_gives_back_the_count_it_took() {
        let replies = std::sync::Arc::new(Replies::default());
        replies.in_flight.fetch_add(1, Ordering::SeqCst);
        let running = replies.clone();
        let counted = InFlight(replies.clone());
        let waiting = runtime().spawn(async move {
            let _counted = counted;
            forward(
                &running,
                7,
                iced::futures::stream::pending::<
                    Result<Message, tokio_tungstenite::tungstenite::Error>,
                >(),
            )
            .await;
        });
        assert!(replies.any_in_flight());

        drop(NodeStream(waiting));
        replies.wait_idle();
        let mut landed = Vec::new();
        replies.drain_into(&mut landed);
        assert!(landed.is_empty(), "an abort delivers nothing: {landed:?}");
    }
}
