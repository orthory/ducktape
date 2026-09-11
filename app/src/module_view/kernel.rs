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
//! - `rpc.query_as_reader` `{target, query}` — the query AS THE SEATED
//!   KEY'S HOLDER (`/v1/query/reader`), for a module that serves protected
//!   content to its reader; refused while the key is locked.
//! - `files.get` `{lane, params}` — one `/v1/files/<lane>` read with its
//!   query-string params; `blob.get` `{digest, limit}` — a blob by hex
//!   digest, verified against it.
//! - `files.stage` `<raw bytes>` / `blob.put` `<raw bytes>` — a duckfs
//!   chunk or a blob landed on the node, proven with the seated key;
//!   answered with the digest.
//! - `op.submit` `{target, payload}` — one module op, signed with the
//!   SEATED key and submitted; answered with the block height. The view
//!   never carries a password, an endpoint or a key.
//! - `host.badge` `<count>` — the tab badge, handed to the app as the
//!   `badge` event with `{"count": N}` in its detail.
//! - `host.id` `<prefix>` — one id, unique on this device, for a module
//!   whose records are addressed by ids its WRITER mints. A view has no
//!   clock and no entropy of its own, so the app mints it.
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
/// The longest `host.id` prefix: a word naming the kind of record, not a
/// payload of its own.
const MAX_ID_PREFIX: usize = 32;

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

    fn deliver(&self, id: u64, result: Result<Vec<u8>, String>) {
        let mut events = self.events.lock().expect("kernel replies");
        events.push(wire::Event::Response {
            id,
            result,
            done: true,
        });
        self.in_flight.fetch_sub(1, Ordering::SeqCst);
        self.landed.notify_all();
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
        ("files", "get") => spawn(guest, id, payload, files_get),
        ("files", "stage") => spawn_raw(guest, id, payload, files_stage),
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
        ("host", "id") => {
            let prefix = std::str::from_utf8(payload).unwrap_or_default().trim();
            let named = !prefix.is_empty()
                && prefix.len() <= MAX_ID_PREFIX
                && prefix.bytes().all(|byte| byte.is_ascii_alphanumeric());
            match named {
                true => guest.reply(
                    id,
                    Ok(crate::backend::fresh_id(prefix).into_bytes()),
                ),
                false => guest.refuse(id, "`host.id` names no prefix".into()),
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

/// One index-tier view read, AFTER the module's fold has caught up with
/// everything this client knows it wrote.
///
/// A derived read model folds BEHIND the block loop, so a view read fired on
/// the heels of this view's own `op.submit` answers a tier that predates it:
/// the moved block back where it was, the deleted line still alive, the line
/// just typed missing. A module whose records the view then plans against
/// (the pages document save) turns that into a DUPLICATE write, so the wait
/// belongs on the kernel's read rather than in each view that has to
/// remember it. `crate::backend::await_seen_fold` waits for nothing when
/// nothing is outstanding, which is every read a view makes that did not
/// just write.
fn view(
    client: ducktape_rpc::Client,
    ask: serde_json::Value,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, String>> + Send>> {
    Box::pin(async move {
        let target = target_of(&ask)?;
        crate::backend::await_seen_fold(&client, &target, &ask["query"]).await;
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
