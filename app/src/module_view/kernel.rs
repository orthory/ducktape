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
//! - `git.merge` `{target, repo, ours, theirs, message}` — the merge commit
//!   a git-backed module's wire demands the CLIENT compute, built with
//!   libgit2 against a bare mirror of the node's `/<target>/<repo>` remote
//!   and landed as a pack under the seated key; answered `{merge_oid,
//!   pack_digest}`, or `{conflicts}` and nothing written. No wasm guest can
//!   do this: it is a git implementation plus a second transport.
//! - `picture.put` `{surface, path, pages}` — base64 pages decoded, joined
//!   and parked as the picture the `picture` surface draws under `surface`;
//!   answered `{width, height}`. `picture.inline` `{doc, source, base, net}`
//!   — the pictures a Markdown `source` embeds, resolved against the
//!   document's own `duck://` address and parked under `doc` for the
//!   document surface. Both are the app's decoder and its one outbound
//!   picture gate, which a view has neither of.
//! - `host.badge` `<count>` — the tab badge, handed to the app as the
//!   `badge` event with `{"count": N}` in its detail; `host.roster`
//!   `{scope, members}` — the mention vocabulary of the host composer a
//!   view docked over `scope`.
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
        ("git", "merge") => spawn(guest, id, payload, git_merge),
        ("picture", "put") => spawn_host(guest, id, payload, picture_put),
        ("picture", "inline") => spawn(guest, id, payload, picture_inline),
        ("host", "roster") => match host_roster(payload) {
            Some((scope, members)) => {
                crate::composer_surface::roster(&scope, &members);
                guest.reply(id, Ok(Vec::new()));
            }
            None => guest.refuse(id, "`host.roster` names no scope".into()),
        },
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

type HostCall = fn(
    serde_json::Value,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, String>> + Send>>;

/// [`spawn`] for a door the NODE is not part of — the app's own picture
/// store. It takes no client, so it answers whether or not the app has a
/// connection: refusing one of these while offline would be a lie about
/// where the work happens.
fn spawn_host(guest: &mut Guest, id: u64, payload: &[u8], call: HostCall) {
    let ask: serde_json::Value = match serde_json::from_slice(payload) {
        Ok(ask) => ask,
        Err(error) => {
            guest.refuse(id, format!("request is not JSON: {error}"));
            return;
        }
    };
    let replies = guest.replies.clone();
    replies.in_flight.fetch_add(1, Ordering::SeqCst);
    runtime().spawn(async move {
        let result = call(ask).await;
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

/// Who a view's host composer may complete an `@` to: one scope's members,
/// as the VIEW read them. The kernel keeps no roster of its own — the list
/// is the guest's, parked where the composer surface finds it.
fn host_roster(payload: &[u8]) -> Option<(String, Vec<crate::backend::ChatMember>)> {
    let ask: serde_json::Value = serde_json::from_slice(payload).ok()?;
    let scope = ask["scope"].as_str().filter(|scope| !scope.is_empty())?;
    let members = ask["members"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .map(|row| crate::backend::ChatMember {
                    key: row["key"].as_str().unwrap_or_default().to_owned(),
                    label: row["label"].as_str().unwrap_or_default().to_owned(),
                })
                .collect()
        })
        .unwrap_or_default();
    Some((scope.to_owned(), members))
}

/// The merge commit a git-backed module's wire demands the CLIENT compute:
/// built against a bare mirror of the node's `/<target>/<repo>` smart-HTTP
/// remote, its minimal pack landed in the blob store under the seated key.
/// A conflict writes NOTHING and answers with the paths.
fn git_merge(
    client: ducktape_rpc::Client,
    ask: serde_json::Value,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, String>> + Send>> {
    Box::pin(async move {
        let target = target_of(&ask)?;
        let repo = ask["repo"].as_str().unwrap_or_default().to_owned();
        let ours = ask["ours"].as_str().unwrap_or_default().to_owned();
        let theirs = ask["theirs"].as_str().unwrap_or_default().to_owned();
        let message = ask["message"].as_str().unwrap_or_default().to_owned();
        let pinned = !ours.is_empty() && !theirs.is_empty();
        if !pinned {
            return Err("`git.merge` names no commits to merge".into());
        }
        let endpoint = client.origin().to_owned();
        let build = tokio::task::spawn_blocking(move || {
            crate::backend::build_git_merge(&endpoint, &target, &repo, &ours, &theirs, &message)
        })
        .await
        .map_err(|error| format!("merge build task failed: {error}"))??;
        let (merge_oid, pack) = match build {
            crate::backend::MergeBuild::Conflicts(paths) => {
                let reply = serde_json::json!({ "conflicts": paths });
                return serde_json::to_vec(&reply).map_err(|error| error.to_string());
            }
            crate::backend::MergeBuild::Clean { merge_oid, pack } => (merge_oid, pack),
        };
        // The pack lands under the person's own signature, the same key the
        // op the view submits over it is signed with.
        let signer = crate::backend::seated_data_plane_signer(&client).await?;
        let pack_digest = client
            .with_write_auth(signer)
            .put_blob(pack)
            .await
            .map_err(|error| error.to_string())?
            .to_lowercase();
        let reply = serde_json::json!({ "merge_oid": merge_oid, "pack_digest": pack_digest });
        serde_json::to_vec(&reply).map_err(|error| error.to_string())
    })
}

fn picture_put(
    ask: serde_json::Value,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, String>> + Send>> {
    Box::pin(async move {
        let surface = ask["surface"].as_str().unwrap_or_default().to_owned();
        let path = ask["path"].as_str().unwrap_or_default().to_owned();
        let named = !surface.is_empty() && !path.is_empty();
        if !named {
            return Err("`picture.put` names no surface".into());
        }
        // Each page is padded base64 in its own right, so the runs are
        // decoded separately and the BYTES joined — concatenating the text
        // would re-frame the stream at the first page boundary.
        let mut bytes = Vec::new();
        for page in ask["pages"].as_array().cloned().unwrap_or_default() {
            let page = crate::backend::base64_decode(page.as_str().unwrap_or_default())
                .ok_or_else(|| "`picture.put` page is not valid base64".to_owned())?;
            bytes.extend_from_slice(&page);
        }
        let (width, height) = crate::backend::store_picture(surface, path, bytes).await?;
        let reply = serde_json::json!({ "width": width, "height": height });
        serde_json::to_vec(&reply).map_err(|error| error.to_string())
    })
}

fn picture_inline(
    client: ducktape_rpc::Client,
    ask: serde_json::Value,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, String>> + Send>> {
    Box::pin(async move {
        let doc = ask["doc"].as_str().unwrap_or_default().to_owned();
        let source = ask["source"].as_str().unwrap_or_default().to_owned();
        let base = ask["base"].as_str().unwrap_or_default().to_owned();
        let net = ask["net"].as_str().unwrap_or_default().to_owned();
        if doc.is_empty() {
            return Err("`picture.inline` names no document".into());
        }
        crate::backend::load_inline_pictures(&client, doc, &source, base, net).await;
        Ok(Vec::new())
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
