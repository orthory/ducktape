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
//! - `rpc.live` `<module>` — a subscription that gets one item per block
//!   the app's live stream reports for that module's plane
//!   ([`live_hit`]), so the view re-reads what moved.
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

use super::{Guest, ModuleViewEvent, Slot, mounted, wire};

/// The most blocks one `rpc.blocks` may ask for.
const MAX_BLOCKS: usize = 1_000;

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
        ("rpc", "live") => {
            let own_plane = payload == guest.module.as_bytes();
            match own_plane {
                true => guest.live_subscriptions.push(id),
                false => guest.refuse(id, "`rpc.live` names another module's plane".into()),
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

/// A block moved `module`'s plane: every `rpc.live` subscription the
/// module's view holds gets one item. Answers `serial + 1` when a view was
/// told — the app keeps that number in its state, so the redraw that
/// delivers the item follows — and `serial` when none was.
pub fn live_hit(module: &str, serial: i64) -> i64 {
    let Some(module) = super::static_module(module) else {
        return serial;
    };
    let mounted = mounted(module);
    let mut locked = mounted.lock().expect("module view lock");
    let Slot::Ready(guest) = &mut locked.slot else {
        return serial;
    };
    if guest.live_subscriptions.is_empty() {
        return serial;
    }
    for id in &guest.live_subscriptions {
        guest.pending.push(wire::Event::Response {
            id: *id,
            result: Ok(b"{}".to_vec()),
            done: false,
        });
    }
    serial + 1
}
