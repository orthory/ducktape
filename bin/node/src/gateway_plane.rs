//! Secure gateway reverse proxy over the purpose-specific overlay stream.
//!
//! A consumer resolves a signed global route, opens `Service::Gateway` to that
//! exact publisher node, and sends one bounded HTTP-shaped request. The
//! publisher re-resolves the route, revalidates Identity authority, maps the
//! authenticated WireGuard peer to its caller account, enforces the signed
//! audience/method/body/header policy, and only then touches DuckFS or one
//! exact node-local loopback upstream.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
#[cfg(test)]
use commonware_cryptography::Signer as _;
use commonware_cryptography::ed25519;
use data_plane::{
    BulkPacer, FlowId, OverlaySockets, PeerId, Service, StreamPacing, StreamPlaneSpec,
    StreamPolicy, StreamService, bind_stream_plane,
};
use duckfs_core::{EntryKindWire, FilesQuery, FilesReply};
use futures::channel::{mpsc, oneshot};
use futures::{SinkExt as _, StreamExt as _};
use noded::{GatewayFailure, GatewayJob, GatewayResponse, NodeCommand};
use sha2::{Digest as _, Sha256};
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};

const BIND_RETRY: Duration = Duration::from_secs(3);
const PROXY_IO_TIMEOUT: Duration = Duration::from_secs(15);
/// Idle ceiling between BODY reads from a loopback upstream. A live SSE feed
/// emits events/keepalives well inside this; a silent-forever upstream would
/// otherwise pin its accept permit (16 total) and its serve task for good.
const BODY_IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_ERROR_BYTES: usize = 512;

type PlaneSlot = Arc<OnceLock<Arc<StreamService<OverlaySockets>>>>;

pub struct SpawnConfig {
    pub label: String,
    pub book: Arc<OverlayBook>,
    pub me: ed25519::PublicKey,
    pub factory: Arc<dyn data_plane::SocketFactory>,
    pub pacer: BulkPacer,
    /// The open-plane registry the bound plane reports into (metrics).
    pub planes: data_plane::PlaneMonitor,
    pub commands: mpsc::Sender<NodeCommand>,
    pub workspace: PathBuf,
}

fn proxy_flow() -> FlowId {
    FlowId::derive(gateway::PROXY_FLOW_DOMAIN)
}

/// the gateway plane's tag for the shared [`crate::overlay_book::OverlayBook`]:
/// default-deny admission scoped to the gateway service + proxy flow; the
/// tracked set follows validator/resident transport membership at cutover.
pub struct GatewayPlane;

impl crate::overlay_book::Plane for GatewayPlane {
    const SERVICE: Service = Service::Gateway;
    fn flow() -> FlowId {
        proxy_flow()
    }
}

pub type OverlayBook = crate::overlay_book::OverlayBook<GatewayPlane>;

/// Start the local client lane and authenticated overlay server.
pub fn spawn(config: SpawnConfig, mut jobs: tokio::sync::mpsc::Receiver<GatewayJob>) {
    let SpawnConfig {
        label,
        book,
        me,
        factory,
        pacer,
        planes,
        commands,
        workspace,
    } = config;
    let slot: PlaneSlot = Arc::new(OnceLock::new());
    let own_node: [u8; 32] = me.as_ref().try_into().expect("ed25519 keys are 32 bytes");

    // Client half. The Node API has already resolved each job from finalized
    // route state; the plane receives no generic peer or URL dial primitive.
    {
        let slot = Arc::clone(&slot);
        let commands = commands.clone();
        let client_workspace = workspace.clone();
        let permits = Arc::new(tokio::sync::Semaphore::new(16));
        tokio::spawn(async move {
            while let Some(job) = jobs.recv().await {
                let Ok(permit) = Arc::clone(&permits).acquire_owned().await else {
                    return;
                };
                let slot = Arc::clone(&slot);
                let commands = commands.clone();
                let workspace = client_workspace.clone();
                tokio::spawn(async move {
                    let _permit = permit;
                    match job {
                        GatewayJob::Http {
                            publisher_node,
                            max_response_bytes,
                            head,
                            body,
                            reply,
                        } => {
                            // The deadline covers everything up to the response
                            // HEAD; the body streams beyond it (WS precedent).
                            let result = tokio::time::timeout(PROXY_IO_TIMEOUT, async {
                                if publisher_node == own_node {
                                    // Self-serve rides the SAME frame protocol as
                                    // the overlay path over a local duplex, so a
                                    // single-node e2e exercises the real wire.
                                    let (server_end, caller_end) = tokio::io::duplex(64 * 1024);
                                    let serve_commands = commands.clone();
                                    let serve_workspace = workspace.clone();
                                    let head_for_server = head.clone();
                                    let body_for_server = body.clone();
                                    tokio::spawn(async move {
                                        serve_proxy_stream(
                                            &serve_commands,
                                            &serve_workspace,
                                            &own_node,
                                            &own_node,
                                            head_for_server,
                                            Some(body_for_server),
                                            server_end,
                                        )
                                        .await;
                                    });
                                    read_streamed_response(caller_end, max_response_bytes).await
                                } else {
                                    proxy_remote(
                                        &slot,
                                        publisher_node,
                                        max_response_bytes,
                                        &head,
                                        &body,
                                    )
                                    .await
                                }
                            })
                            .await
                            .unwrap_or_else(|_| {
                                Err(GatewayFailure::Unavailable(
                                    "gateway proxy request timed out".into(),
                                ))
                            });
                            let _ = reply.send(result);
                        }
                        // A WebSocket upgrade is long-lived, so it is not wrapped
                        // in the one-shot timeout.
                        GatewayJob::Upgrade {
                            publisher_node,
                            head,
                            to_browser,
                            from_browser,
                        } => {
                            if publisher_node == own_node {
                                // Loopback: pipe our own serve_ws to the caller
                                // pump over a local duplex.
                                let (server_end, caller_end) = tokio::io::duplex(64 * 1024);
                                let serve_commands = commands.clone();
                                let serve_workspace = workspace.clone();
                                tokio::spawn(async move {
                                    serve_ws(
                                        &serve_commands,
                                        &serve_workspace,
                                        &own_node,
                                        &own_node,
                                        &head,
                                        server_end,
                                    )
                                    .await;
                                });
                                caller_ws_pump(caller_end, to_browser, from_browser).await;
                            } else {
                                proxy_remote_ws(
                                    &slot,
                                    publisher_node,
                                    &head,
                                    to_browser,
                                    from_browser,
                                )
                                .await;
                            }
                        }
                    }
                });
            }
        });
    }

    // Server half. Bind retry starts before the userspace WireGuard stack is
    // installed and becomes live automatically once the node owns its ULA.
    tokio::spawn(async move {
        let own = book.own_addr(&me);
        let spec = StreamPlaneSpec {
            own_ip: own,
            service: Service::Gateway,
            pacing: StreamPacing::Shared(pacer),
            policy: StreamPolicy { accept_backlog: 16 },
            retry: BIND_RETRY,
        };
        let (plane, service) = match bind_stream_plane(spec, factory, book).await {
            Ok(bound) => bound,
            Err(error) => {
                tracing::error!(
                    target: "ducktape::gateway",
                    node = %label,
                    error = %error,
                    "gateway plane register failed"
                );
                return;
            }
        };
        tracing::info!(
            target: "ducktape::gateway",
            node = %label,
            own = %own,
            "gateway plane: overlay stream bound"
        );
        planes.register("gateway", Service::Gateway, plane.watch());
        let _ = slot.set(Arc::clone(&service));
        let _plane = plane;
        let permits = Arc::new(tokio::sync::Semaphore::new(16));
        loop {
            let Some((requester, hello, mut stream)) = service.accept().await else {
                return;
            };
            let Ok(permit) = Arc::clone(&permits).acquire_owned().await else {
                return;
            };
            let commands = commands.clone();
            let workspace = workspace.clone();
            tokio::spawn(async move {
                let _permit = permit;
                if hello.intent != gateway::PROXY_INTENT {
                    let _ = write_proxy_response(
                        &mut stream,
                        Err(GatewayFailure::Invalid(
                            "gateway proxy: unsupported stream intent".into(),
                        )),
                    )
                    .await;
                    return;
                }
                let head = match gateway::decode_proxy_request_head(&hello.meta) {
                    Ok(head) => head,
                    Err(error) => {
                        let _ =
                            write_proxy_response(&mut stream, Err(GatewayFailure::Invalid(error)))
                                .await;
                        return;
                    }
                };
                // A WebSocket upgrade is long-lived; it owns the stream and
                // writes its own responses, so it bypasses the one-shot timeout.
                if head.upgrade {
                    serve_ws(&commands, &workspace, &own_node, &requester.0, &head, stream).await;
                    return;
                }
                serve_proxy_stream(&commands, &workspace, &own_node, &requester.0, head, None, stream)
                    .await;
            });
        }
    });
}

async fn proxy_remote(
    slot: &PlaneSlot,
    publisher: [u8; 32],
    max_response_bytes: u64,
    head: &gateway::ProxyRequestHead,
    body: &[u8],
) -> Result<GatewayResponse, GatewayFailure> {
    let service = slot
        .get()
        .ok_or_else(|| GatewayFailure::Unavailable("gateway overlay is still starting".into()))?;
    if body.len() as u64 != head.body_len {
        return Err(GatewayFailure::Invalid(
            "gateway proxy: request body length mismatch".into(),
        ));
    }
    let meta = gateway::encode_proxy_request_head(head).map_err(GatewayFailure::Invalid)?;
    let mut stream = service
        .open(PeerId(publisher), proxy_flow(), gateway::PROXY_INTENT, meta)
        .await
        .map_err(|error| GatewayFailure::Unavailable(error.to_string()))?;
    // The deadline covers the request write + response HEAD; the body pump
    // streams beyond it.
    tokio::time::timeout(PROXY_IO_TIMEOUT, async {
        stream
            .write_all(body)
            .await
            .map_err(|error| GatewayFailure::Unavailable(error.to_string()))?;
        stream
            .flush()
            .await
            .map_err(|error| GatewayFailure::Unavailable(error.to_string()))?;
        read_streamed_response(stream, max_response_bytes).await
    })
    .await
    .map_err(|_| GatewayFailure::Unavailable("gateway publisher timed out".into()))?
}

/// One proxied HTTP exchange over a frame-capable stream (the overlay socket
/// or the self-serve duplex). `body`: `Some` when the caller already holds the
/// request body (self-serve); `None` reads `head.body_len` bytes off the
/// stream (overlay). The deadline covers the body read + serve up to the
/// response HEAD; the body drain streams beyond it.
async fn serve_proxy_stream<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(
    commands: &mpsc::Sender<NodeCommand>,
    workspace: &Path,
    own_node: &[u8; 32],
    caller_node: &[u8; 32],
    head: gateway::ProxyRequestHead,
    body: Option<Vec<u8>>,
    mut stream: S,
) {
    let outcome = tokio::time::timeout(PROXY_IO_TIMEOUT, async {
        let body = match body {
            Some(body) => body,
            None => {
                let body_len = usize::try_from(head.body_len).map_err(|_| {
                    GatewayFailure::Invalid("gateway proxy: body length overflows usize".into())
                })?;
                let mut body = vec![0u8; body_len];
                stream
                    .read_exact(&mut body)
                    .await
                    .map_err(|error| GatewayFailure::Unavailable(error.to_string()))?;
                body
            }
        };
        serve_current(commands, workspace, own_node, caller_node, &head, &body).await
    })
    .await
    .unwrap_or_else(|_| {
        Err(GatewayFailure::Unavailable(
            "gateway proxy request timed out".into(),
        ))
    });
    let _ = write_proxy_response(&mut stream, outcome).await;
}

/// Read the response head, then hand the stream to the body pump. Returns AT
/// the head — the returned `GatewayResponse.body` streams.
async fn read_streamed_response<S: AsyncRead + Unpin + Send + 'static>(
    mut stream: S,
    max_response_bytes: u64,
) -> Result<GatewayResponse, GatewayFailure> {
    let mut buf = Vec::new();
    let head = read_proxy_head(&mut stream, &mut buf).await?;
    Ok(GatewayResponse {
        head,
        body: spawn_body_pump(stream, buf, max_response_bytes),
    })
}

/// Caller side of a remote WebSocket upgrade: open the mesh stream to the
/// publisher, then bridge it to the browser channels. `caller_ws_pump` consumes
/// the publisher's `101` ack. On any open failure the browser is closed.
async fn proxy_remote_ws(
    slot: &PlaneSlot,
    publisher: [u8; 32],
    head: &gateway::ProxyRequestHead,
    to_browser: tokio::sync::mpsc::Sender<noded::GatewayWsMsg>,
    from_browser: tokio::sync::mpsc::Receiver<noded::GatewayWsMsg>,
) {
    let service = match slot.get() {
        Some(service) => service,
        None => {
            let _ = to_browser.send(noded::GatewayWsMsg::Close(1011)).await;
            return;
        }
    };
    let meta = match gateway::encode_proxy_request_head(head) {
        Ok(meta) => meta,
        Err(_) => {
            let _ = to_browser.send(noded::GatewayWsMsg::Close(1011)).await;
            return;
        }
    };
    let stream = match service
        .open(PeerId(publisher), proxy_flow(), gateway::PROXY_INTENT, meta)
        .await
    {
        Ok(stream) => stream,
        Err(_) => {
            let _ = to_browser.send(noded::GatewayWsMsg::Close(1011)).await;
            return;
        }
    };
    caller_ws_pump(stream, to_browser, from_browser).await;
}

async fn serve_current(
    commands: &mpsc::Sender<NodeCommand>,
    workspace: &Path,
    own_node: &[u8; 32],
    caller_node: &[u8; 32],
    head: &gateway::ProxyRequestHead,
    body: &[u8],
) -> Result<GatewayResponse, GatewayFailure> {
    gateway::validate_proxy_request_head(head).map_err(GatewayFailure::Invalid)?;
    if body.len() as u64 != head.body_len {
        return Err(GatewayFailure::Invalid(
            "gateway proxy: request body length mismatch".into(),
        ));
    }
    let record = resolve_route(commands, &head.account_id, &head.name).await?;
    if record.statement.publisher_node.as_slice() != own_node {
        return Err(GatewayFailure::Forbidden(
            "gateway request does not name this publisher".into(),
        ));
    }
    if !gateway::request_matches_record(head, &record) {
        return Err(GatewayFailure::Conflict(
            "gateway request does not match the current signed route".into(),
        ));
    }
    revalidate_route_authority(commands, &record).await?;
    let caller = account_of_node(commands, caller_node).await?;
    let route = record
        .statement
        .route
        .as_ref()
        .expect("resolve_route rejects tombstones");
    if !gateway::audience_allows(
        &route.policy.audience,
        &record.statement.account_id,
        &caller.account_id,
    ) {
        return Err(GatewayFailure::Forbidden(
            "caller account is outside the signed route audience".into(),
        ));
    }
    match &route.target {
        gateway::RouteTarget::DuckFs { .. } => {
            serve_duckfs(commands, own_node, head, &record).await
        }
        gateway::RouteTarget::LoopbackHttp => {
            proxy_loopback(
                workspace,
                caller_node,
                &caller.account_id,
                head,
                body,
                &record,
            )
            .await
        }
    }
}

async fn resolve_route(
    commands: &mpsc::Sender<NodeCommand>,
    account_id: &[u8],
    name: &gateway::RouteName,
) -> Result<gateway::RouteRecord, GatewayFailure> {
    let reply = query(
        commands,
        "gateway",
        gateway::encode_query(&gateway::GatewayQuery::Get {
            account_id: account_id.to_vec(),
            name: name.clone(),
        }),
    )
    .await?;
    match gateway::decode_reply(&reply) {
        Ok(gateway::GatewayReply::Route(route)) => match *route {
            Some(record) if record.statement.route.is_some() => Ok(record),
            _ => Err(GatewayFailure::NotFound(
                "gateway route is not published".into(),
            )),
        },
        // a route `Get` must answer with a `Route`; the list, handle-plane, and
        // credential replies are all wrong shapes here.
        Ok(gateway::GatewayReply::Routes(_))
        | Ok(gateway::GatewayReply::Resolved(_))
        | Ok(gateway::GatewayReply::Registrations(_))
        | Ok(gateway::GatewayReply::Credential(_))
        | Ok(gateway::GatewayReply::Credentials(_)) => Err(GatewayFailure::Unavailable(
            "gateway returned an unexpected reply to a route query".into(),
        )),
        Err(error) => Err(GatewayFailure::Unavailable(error)),
    }
}

async fn account_of_node(
    commands: &mpsc::Sender<NodeCommand>,
    node: &[u8; 32],
) -> Result<identity::AccountView, GatewayFailure> {
    let reply = query(
        commands,
        "identity",
        identity::encode_query(&identity::IdentityQuery::OfNode {
            node_key: node.to_vec(),
        }),
    )
    .await?;
    match identity::decode_reply(&reply) {
        Ok(identity::IdentityReply::Account(Some(account)))
            if account
                .nodes
                .iter()
                .any(|candidate| candidate.node_key.as_slice() == node) =>
        {
            Ok(account)
        }
        Ok(identity::IdentityReply::Account(_)) => Err(GatewayFailure::Forbidden(
            "gateway caller node has no current Identity account".into(),
        )),
        Ok(_) => Err(GatewayFailure::Unavailable(
            "unexpected Identity caller reply".into(),
        )),
        Err(error) => Err(GatewayFailure::Unavailable(error)),
    }
}

async fn revalidate_route_authority(
    commands: &mpsc::Sender<NodeCommand>,
    record: &gateway::RouteRecord,
) -> Result<(), GatewayFailure> {
    let statement = &record.statement;
    let reply = query(
        commands,
        "identity",
        identity::encode_query(&identity::IdentityQuery::Get {
            account_id: statement.account_id.clone(),
        }),
    )
    .await?;
    let account = match identity::decode_reply(&reply) {
        Ok(identity::IdentityReply::Account(Some(account))) => account,
        Ok(identity::IdentityReply::Account(None)) => {
            return Err(GatewayFailure::Forbidden(
                "gateway route account no longer exists".into(),
            ));
        }
        Ok(_) => {
            return Err(GatewayFailure::Unavailable(
                "unexpected Identity route-authority reply".into(),
            ));
        }
        Err(error) => return Err(GatewayFailure::Unavailable(error)),
    };
    let authorization = &record.authorization;
    let signer_is_current = account.member_keys.iter().any(|member| {
        member.kind == identity::KeyKind::Ed25519 && member.pubkey == authorization.signer
    });
    let node_is_current = account
        .nodes
        .iter()
        .any(|node| node.node_key == statement.publisher_node);
    let proof = identity::MemberProof::Signature {
        sig: authorization.signature.clone(),
    };
    let preimage =
        gateway::route_signing_preimage(statement).map_err(GatewayFailure::Unavailable)?;
    if account.account_id != statement.account_id
        || !signer_is_current
        || !node_is_current
        || !identity::verify_authority(
            identity::KeyKind::Ed25519,
            &authorization.signer,
            None,
            gateway::GATEWAY_ROUTE_NS,
            &preimage,
            &proof,
        )
    {
        return Err(GatewayFailure::Forbidden(
            "gateway route authority is no longer current".into(),
        ));
    }
    Ok(())
}

async fn proxy_loopback(
    workspace: &Path,
    caller_node: &[u8; 32],
    caller_account: &[u8],
    head: &gateway::ProxyRequestHead,
    body: &[u8],
    record: &gateway::RouteRecord,
) -> Result<GatewayResponse, GatewayFailure> {
    let route = record
        .statement
        .route
        .as_ref()
        .expect("current route is live");
    let routes = crate::gateway_routes::load(workspace).map_err(GatewayFailure::Unavailable)?;
    let port = routes.port(&head.name).ok_or_else(|| {
        GatewayFailure::NotFound("global gateway route has no local loopback upstream".into())
    })?;
    // Connect + per-read deadlines only: a TOTAL timeout would kill long
    // streamed (SSE) bodies, but a silent-forever upstream must not pin its
    // accept permit — the idle read timeout reclaims it. The head is still
    // deadline-bound by serve_proxy_stream.
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .connect_timeout(PROXY_IO_TIMEOUT)
        .read_timeout(BODY_IDLE_TIMEOUT)
        .build()
        .map_err(|error| GatewayFailure::Unavailable(error.to_string()))?;
    let method = reqwest::Method::from_bytes(head.method.as_http_str().as_bytes())
        .expect("route methods are valid HTTP tokens");
    let url = format!("http://127.0.0.1:{port}{}", head.path_and_query);
    let mut upstream = client
        .request(method, url)
        .header(reqwest::header::ACCEPT_ENCODING, "identity")
        .header(reqwest::header::USER_AGENT, "Ducktape-Gateway/1")
        .header("x-duck-caller-account", hex_bytes(caller_account))
        .header("x-duck-caller-node", hex_bytes(caller_node))
        .header(
            "x-duck-route-account",
            hex_bytes(&record.statement.account_id),
        )
        .header("x-duck-route-label", head.name.local_key())
        .header(
            "x-duck-route-revision",
            record.statement.revision.to_string(),
        );
    for header in &head.headers {
        // Strip hop-by-hop / forwarding / identity headers and never let a
        // caller header shadow a proxy-minted x-duck-* (decode already rejects
        // those, so this is defense in depth).
        if !gateway::header_forwardable(&header.name) {
            continue;
        }
        if header.name == "authorization" && !route.policy.allow_authorization {
            return Err(GatewayFailure::Forbidden(
                "Authorization is disabled by the signed route policy".into(),
            ));
        }
        let name = reqwest::header::HeaderName::from_bytes(header.name.as_bytes())
            .map_err(|error| GatewayFailure::Invalid(error.to_string()))?;
        let value = reqwest::header::HeaderValue::from_str(&header.value)
            .map_err(|error| GatewayFailure::Invalid(error.to_string()))?;
        upstream = upstream.header(name, value);
    }
    if !body.is_empty() {
        upstream = upstream.body(body.to_vec());
    }
    let response = upstream
        .send()
        .await
        .map_err(|error| GatewayFailure::Unavailable(error.to_string()))?;
    let capped = route.policy.max_response_bytes != 0; // 0 = unbounded stream
    if capped
        && response
            .content_length()
            .is_some_and(|length| length > route.policy.max_response_bytes)
    {
        return Err(GatewayFailure::Unavailable(
            "loopback response exceeds the signed route cap".into(),
        ));
    }
    if response
        .headers()
        .get(reqwest::header::CONTENT_ENCODING)
        .is_some_and(|value| value.as_bytes() != b"identity")
    {
        return Err(GatewayFailure::Unavailable(
            "loopback upstream ignored identity content encoding".into(),
        ));
    }
    let mut headers = Vec::new();
    for name in gateway::ALLOWED_RESPONSE_HEADERS {
        if *name == "set-cookie" {
            // The one legitimately repeatable response header. Each cookie is
            // scrubbed to host-only: a publisher must not plant a Domain=.duck
            // (or Domain=<other-handle>.duck) cookie readable across accounts.
            for value in response.headers().get_all(*name) {
                let value = value.to_str().map_err(|_| {
                    GatewayFailure::Unavailable(format!(
                        "loopback returned non-ASCII {name} header"
                    ))
                })?;
                headers.push(gateway::ProxyHeader {
                    name: (*name).to_string(),
                    value: scrub_cookie_domain(value),
                });
            }
            continue;
        }
        let values = response.headers().get_all(*name);
        let mut values = values.iter();
        let Some(value) = values.next() else {
            continue;
        };
        if values.next().is_some() {
            return Err(GatewayFailure::Unavailable(format!(
                "loopback returned duplicate {name} headers"
            )));
        }
        headers.push(gateway::ProxyHeader {
            name: (*name).to_string(),
            value: value
                .to_str()
                .map_err(|_| {
                    GatewayFailure::Unavailable(format!(
                        "loopback returned non-ASCII {name} header"
                    ))
                })?
                .to_string(),
        });
    }
    let response_head = gateway::ProxyResponseHead {
        status: response.status().as_u16(),
        headers,
    };
    gateway::validate_response_head(&response_head).map_err(GatewayFailure::Unavailable)?;
    // Stream the upstream body through a bounded channel with a RUNNING cap
    // (0 = unbounded, the declared-SSE case). The head returns immediately.
    let cap = route.policy.max_response_bytes;
    let is_head_method = head.method == gateway::RouteMethod::Head;
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<bytes::Bytes, GatewayFailure>>(16);
    tokio::spawn(async move {
        if is_head_method {
            return; // headers only; drop the upstream body
        }
        let mut chunks = response.bytes_stream();
        let mut total: u64 = 0;
        while let Some(chunk) = chunks.next().await {
            let item = match chunk {
                Ok(chunk) => {
                    total = total.saturating_add(chunk.len() as u64);
                    let over_cap = cap != 0 && total > cap;
                    if over_cap {
                        let _ = tx
                            .send(Err(GatewayFailure::Unavailable(
                                "loopback response exceeds the signed route cap".into(),
                            )))
                            .await;
                        return;
                    }
                    Ok(chunk)
                }
                Err(error) => {
                    let _ = tx
                        .send(Err(GatewayFailure::Unavailable(error.to_string())))
                        .await;
                    return;
                }
            };
            if tx.send(item).await.is_err() {
                return; // caller went away; drop the upstream stream
            }
        }
    });
    Ok(GatewayResponse {
        head: response_head,
        body: rx,
    })
}

/// DuckFS reads are windowed at 1 MiB; a manifest (≤ 4 MiB) or a file
/// (≤ 64 MiB) is assembled across windows.
const READ_WINDOW: u64 = 1024 * 1024;

fn gateway_path(own_node: &[u8; 32], label: &str, relative: &str) -> String {
    format!(
        "/home/ext:{}/.duck/gateway/{}/{}",
        hex_bytes(own_node),
        label,
        relative
    )
}

async fn serve_duckfs(
    commands: &mpsc::Sender<NodeCommand>,
    own_node: &[u8; 32],
    head: &gateway::ProxyRequestHead,
    record: &gateway::RouteRecord,
) -> Result<GatewayResponse, GatewayFailure> {
    let route = record
        .statement
        .route
        .as_ref()
        .ok_or_else(|| GatewayFailure::NotFound("route is unpublished".into()))?;
    let gateway::RouteTarget::DuckFs { manifest_sha256 } = &route.target else {
        return Err(GatewayFailure::Invalid(
            "route is not content-backed".into(),
        ));
    };
    if !matches!(
        head.method,
        gateway::RouteMethod::Get | gateway::RouteMethod::Head
    ) || head.body_len != 0
    {
        return Err(GatewayFailure::Invalid(
            "content routes serve bodyless GET/HEAD only".into(),
        ));
    }
    let label = head.name.local_key();
    // Pin one DuckFS snapshot across the manifest read and the file read so a
    // publisher-local mutation cannot race them.
    let snapshot = duckfs_head(commands).await?;

    // The manifest is a DuckFS file addressed by the signed hash: read it,
    // verify the exact bytes, then trust its file table.
    let manifest_bytes = read_duckfs_file(
        commands,
        &gateway_path(own_node, label, gateway::MANIFEST_FILE),
        &snapshot,
        gateway::MAX_MANIFEST_BYTES,
    )
    .await?;
    if hex_bytes(&Sha256::digest(&manifest_bytes)) != *manifest_sha256 {
        return Err(GatewayFailure::Forbidden(
            "manifest does not match the signed hash".into(),
        ));
    }
    let manifest: gateway::RouteManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| GatewayFailure::Unavailable(format!("manifest is not valid json: {error}")))?;
    gateway::validate_manifest(&manifest).map_err(GatewayFailure::Forbidden)?;

    let file =
        gateway::manifest_file_for_path(&manifest, &head.path_and_query).map_err(GatewayFailure::NotFound)?;
    // Serve-time cap: the file table is off consensus, so the signed response
    // cap is enforced here rather than at admission.
    if file.size > route.policy.max_response_bytes {
        return Err(GatewayFailure::Forbidden(
            "file exceeds the signed response cap".into(),
        ));
    }
    // HEAD still authenticates the exact bytes before advertising the ETag, so
    // a same-sized local mutation cannot make a stale file look current.
    let mut bytes = read_duckfs_file(
        commands,
        &gateway_path(own_node, label, &file.path),
        &snapshot,
        file.size,
    )
    .await?;
    if bytes.len() as u64 != file.size || hex_bytes(&Sha256::digest(&bytes)) != file.sha256 {
        return Err(GatewayFailure::Forbidden(
            "DuckFS bytes do not match the manifest".into(),
        ));
    }
    if head.method == gateway::RouteMethod::Head {
        bytes.clear();
    }
    let response_head = gateway::ProxyResponseHead {
        status: 200,
        headers: vec![
            gateway::ProxyHeader {
                name: "content-type".into(),
                value: file.mime.clone(),
            },
            gateway::ProxyHeader {
                name: "etag".into(),
                value: format!("\"{}\"", file.sha256),
            },
        ],
    };
    gateway::validate_response_head(&response_head).map_err(GatewayFailure::Unavailable)?;
    // Content responses stay buffered internally; wrap the one blob as a
    // single-chunk stream (the frame writer re-splits at MAX_CHUNK_BYTES).
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<bytes::Bytes, GatewayFailure>>(1);
    if !bytes.is_empty() {
        let _ = tx.try_send(Ok(bytes::Bytes::from(bytes)));
    }
    Ok(GatewayResponse {
        head: response_head,
        body: rx,
    })
}

async fn duckfs_head(
    commands: &mpsc::Sender<NodeCommand>,
) -> Result<duckfs_core::DigestHex, GatewayFailure> {
    match files_query(commands, &FilesQuery::Refs {}).await? {
        FilesReply::Refs(info) => info
            .head
            .ok_or_else(|| GatewayFailure::NotFound("publisher DuckFS is empty".into())),
        _ => Err(GatewayFailure::Unavailable(
            "unexpected DuckFS refs reply".into(),
        )),
    }
}

/// Stat a gateway file, bound its size to `max_len`, then read it across
/// 1 MiB windows. The caller hash-verifies the returned bytes.
async fn read_duckfs_file(
    commands: &mpsc::Sender<NodeCommand>,
    path: &str,
    snapshot: &duckfs_core::DigestHex,
    max_len: u64,
) -> Result<Vec<u8>, GatewayFailure> {
    let size = match files_query(
        commands,
        &FilesQuery::Stat {
            path: path.to_string(),
            snapshot: Some(snapshot.clone()),
        },
    )
    .await?
    {
        FilesReply::Stat(Some(entry)) if entry.kind == EntryKindWire::File => entry.size,
        FilesReply::Stat(Some(_)) => {
            return Err(GatewayFailure::Forbidden(
                "gateway path is not a file".into(),
            ));
        }
        FilesReply::Stat(None) => {
            return Err(GatewayFailure::NotFound("gateway file is missing".into()));
        }
        _ => {
            return Err(GatewayFailure::Unavailable(
                "unexpected DuckFS stat reply".into(),
            ));
        }
    };
    if size > max_len {
        return Err(GatewayFailure::Forbidden(format!(
            "gateway file exceeds {max_len} bytes"
        )));
    }
    let mut bytes = Vec::new();
    let mut offset = 0u64;
    loop {
        // A declared empty file still issues one read to confirm EOF.
        let len = if size == 0 {
            1
        } else {
            (size - offset).min(READ_WINDOW)
        };
        match files_query(
            commands,
            &FilesQuery::Read {
                path: path.to_string(),
                snapshot: Some(snapshot.clone()),
                offset,
                len,
            },
        )
        .await?
        {
            FilesReply::Read { b64, eof } => {
                let chunk = STANDARD.decode(b64.as_bytes()).map_err(|error| {
                    GatewayFailure::Unavailable(format!("DuckFS returned bad base64: {error}"))
                })?;
                offset += chunk.len() as u64;
                bytes.extend_from_slice(&chunk);
                if eof || offset >= size {
                    break;
                }
            }
            _ => {
                return Err(GatewayFailure::Unavailable(
                    "unexpected DuckFS read reply".into(),
                ));
            }
        }
    }
    Ok(bytes)
}

async fn files_query(
    commands: &mpsc::Sender<NodeCommand>,
    request: &FilesQuery,
) -> Result<FilesReply, GatewayFailure> {
    let bytes = query(commands, "files", duckfs_core::encode_query(request)).await?;
    duckfs_core::decode_reply(&bytes).map_err(GatewayFailure::Unavailable)
}

async fn query(
    commands: &mpsc::Sender<NodeCommand>,
    target: &str,
    req: Vec<u8>,
) -> Result<Vec<u8>, GatewayFailure> {
    let (reply, rx) = oneshot::channel();
    let mut commands = commands.clone();
    commands
        .send(NodeCommand::Query {
            target: target.into(),
            req,
            reply,
        })
        .await
        .map_err(|_| GatewayFailure::Unavailable("node actor is gone".into()))?;
    rx.await
        .map_err(|_| GatewayFailure::Unavailable("node actor dropped the query".into()))?
        .map_err(GatewayFailure::Unavailable)
}

/// Authorize a WebSocket upgrade and resolve its loopback `ws://` target. Same
/// checks as an HTTP request plus the signed `allow_upgrade` bit; content
/// routes cannot upgrade.
async fn authorize_ws(
    commands: &mpsc::Sender<NodeCommand>,
    workspace: &Path,
    own_node: &[u8; 32],
    caller_node: &[u8; 32],
    head: &gateway::ProxyRequestHead,
) -> Result<String, GatewayFailure> {
    gateway::validate_proxy_request_head(head).map_err(GatewayFailure::Invalid)?;
    if !head.upgrade {
        return Err(GatewayFailure::Invalid("gateway proxy: not an upgrade".into()));
    }
    let record = resolve_route(commands, &head.account_id, &head.name).await?;
    if record.statement.publisher_node.as_slice() != own_node {
        return Err(GatewayFailure::Forbidden(
            "gateway request does not name this publisher".into(),
        ));
    }
    if !gateway::request_matches_record(head, &record) {
        return Err(GatewayFailure::Conflict(
            "gateway request does not match the current signed route".into(),
        ));
    }
    revalidate_route_authority(commands, &record).await?;
    let caller = account_of_node(commands, caller_node).await?;
    let route = record
        .statement
        .route
        .as_ref()
        .expect("resolve_route rejects tombstones");
    if !gateway::audience_allows(
        &route.policy.audience,
        &record.statement.account_id,
        &caller.account_id,
    ) {
        return Err(GatewayFailure::Forbidden(
            "caller account is outside the signed route audience".into(),
        ));
    }
    if !route.policy.allow_upgrade || !matches!(route.target, gateway::RouteTarget::LoopbackHttp) {
        return Err(GatewayFailure::Forbidden(
            "route does not permit a WebSocket upgrade".into(),
        ));
    }
    let routes = crate::gateway_routes::load(workspace).map_err(GatewayFailure::Unavailable)?;
    let port = routes.port(&head.name).ok_or_else(|| {
        GatewayFailure::NotFound("global gateway route has no local loopback upstream".into())
    })?;
    Ok(format!("ws://127.0.0.1:{port}{}", head.path_and_query))
}

/// Bridge a WebSocket upgrade to the route's loopback upstream. Owns the mesh
/// stream: writes a `Failure` frame on any authorize/dial error, otherwise a
/// `101` `ResponseHead` then pumps `WsFrame`/`WsClose` both ways until close.
async fn serve_ws<S>(
    commands: &mpsc::Sender<NodeCommand>,
    workspace: &Path,
    own_node: &[u8; 32],
    caller_node: &[u8; 32],
    head: &gateway::ProxyRequestHead,
    mut stream: S,
) where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let url = match authorize_ws(commands, workspace, own_node, caller_node, head).await {
        Ok(url) => url,
        Err(failure) => {
            let _ = write_frame(&mut stream, &failure_frame(&failure)).await;
            let _ = stream.flush().await;
            return;
        }
    };
    let upstream = match tokio_tungstenite::connect_async(&url).await {
        Ok((upstream, _response)) => upstream,
        Err(error) => {
            let _ = write_frame(
                &mut stream,
                &failure_frame(&GatewayFailure::Unavailable(error.to_string())),
            )
            .await;
            let _ = stream.flush().await;
            return;
        }
    };
    if write_frame(
        &mut stream,
        &gateway::ProxyFrame::ResponseHead(gateway::ProxyResponseHead {
            status: 101,
            headers: vec![],
        }),
    )
    .await
    .is_err()
        || stream.flush().await.is_err()
    {
        return;
    }
    ws_pump(stream, upstream).await;
}

/// Two independent tasks (mesh→upstream, upstream→mesh); when either direction
/// ends, the other is aborted so both sockets drop and each peer sees EOF.
async fn ws_pump<S, U>(stream: S, upstream: U)
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    U: futures::Stream<Item = Result<tokio_tungstenite::tungstenite::Message, tokio_tungstenite::tungstenite::Error>>
        + futures::Sink<tokio_tungstenite::tungstenite::Message, Error = tokio_tungstenite::tungstenite::Error>
        + Unpin
        + Send
        + 'static,
{
    use tokio_tungstenite::tungstenite::Message;
    let (mut mesh_read, mut mesh_write) = tokio::io::split(stream);
    let (mut ws_tx, mut ws_rx) = upstream.split();
    let mut to_upstream = tokio::spawn(async move {
        let mut buf = Vec::new();
        loop {
            match read_frame(&mut mesh_read, &mut buf).await {
                Ok(gateway::ProxyFrame::WsFrame { binary, payload }) => {
                    let message = if binary {
                        Message::binary(payload)
                    } else {
                        Message::text(String::from_utf8_lossy(&payload).into_owned())
                    };
                    if ws_tx.send(message).await.is_err() {
                        break;
                    }
                }
                _ => {
                    let _ = ws_tx.close().await;
                    break;
                }
            }
        }
    });
    let mut to_mesh = tokio::spawn(async move {
        while let Some(message) = ws_rx.next().await {
            let frame = match message {
                Ok(Message::Text(text)) => gateway::ProxyFrame::WsFrame {
                    binary: false,
                    payload: text.as_bytes().to_vec(),
                },
                Ok(Message::Binary(bytes)) => gateway::ProxyFrame::WsFrame {
                    binary: true,
                    payload: bytes.to_vec(),
                },
                Ok(Message::Close(frame)) => gateway::ProxyFrame::WsClose {
                    code: frame.map(|frame| u16::from(frame.code)).unwrap_or(1000),
                },
                Ok(_) => continue,
                Err(_) => break,
            };
            let closing = matches!(frame, gateway::ProxyFrame::WsClose { .. });
            if write_frame(&mut mesh_write, &frame).await.is_err() {
                break;
            }
            let _ = mesh_write.flush().await;
            if closing {
                break;
            }
        }
    });
    tokio::select! {
        _ = &mut to_upstream => to_mesh.abort(),
        _ = &mut to_mesh => to_upstream.abort(),
    }
}

/// Caller side of a WebSocket upgrade: read the publisher's `101` ack, then
/// bridge the browser's message channels to the mesh stream. Mirrors
/// [`ws_pump`] with the roles reversed — the noded WS door owns the
/// browser/axum translation. Returns once either direction closes. On a
/// non-101 first frame (a `Failure` or garbage) it closes the browser side.
async fn caller_ws_pump<S>(
    mut mesh: S,
    to_browser: tokio::sync::mpsc::Sender<noded::GatewayWsMsg>,
    mut from_browser: tokio::sync::mpsc::Receiver<noded::GatewayWsMsg>,
) where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    use noded::GatewayWsMsg;
    let mut buf = Vec::new();
    match read_frame(&mut mesh, &mut buf).await {
        Ok(gateway::ProxyFrame::ResponseHead(head)) if head.status == 101 => {}
        _ => {
            // Upgrade refused/failed; signal an abnormal close to the browser.
            let _ = to_browser.send(GatewayWsMsg::Close(1011)).await;
            return;
        }
    }
    let (mut mesh_read, mut mesh_write) = tokio::io::split(mesh);
    let mut to_browser_task = tokio::spawn(async move {
        // Seed with any bytes already read past the 101 frame.
        loop {
            match read_frame(&mut mesh_read, &mut buf).await {
                Ok(gateway::ProxyFrame::WsFrame { binary, payload }) => {
                    let message = if binary {
                        GatewayWsMsg::Binary(payload)
                    } else {
                        GatewayWsMsg::Text(String::from_utf8_lossy(&payload).into_owned())
                    };
                    if to_browser.send(message).await.is_err() {
                        break;
                    }
                }
                Ok(gateway::ProxyFrame::WsClose { code }) => {
                    let _ = to_browser.send(GatewayWsMsg::Close(code)).await;
                    break;
                }
                _ => break,
            }
        }
    });
    let mut from_browser_task = tokio::spawn(async move {
        while let Some(message) = from_browser.recv().await {
            let frame = match message {
                GatewayWsMsg::Text(text) => gateway::ProxyFrame::WsFrame {
                    binary: false,
                    payload: text.into_bytes(),
                },
                GatewayWsMsg::Binary(bytes) => gateway::ProxyFrame::WsFrame {
                    binary: true,
                    payload: bytes,
                },
                GatewayWsMsg::Close(code) => gateway::ProxyFrame::WsClose { code },
            };
            let closing = matches!(frame, gateway::ProxyFrame::WsClose { .. });
            if write_frame(&mut mesh_write, &frame).await.is_err() {
                break;
            }
            let _ = mesh_write.flush().await;
            if closing {
                break;
            }
        }
    });
    tokio::select! {
        _ = &mut to_browser_task => from_browser_task.abort(),
        _ = &mut from_browser_task => to_browser_task.abort(),
    }
}

/// Drop any `Domain` attribute from a Set-Cookie value so gateway cookies stay
/// host-only. Chromium's handling of the synthetic `.duck` TLD is not a
/// boundary we rely on: without this, a publisher could try `Domain=.duck` to
/// plant a cookie visible on every account's origins.
///
/// Attributes are `name=value` pairs split on `;` (RFC 6265 §5.2); the name is
/// everything before the first `=`, whitespace-trimmed and case-insensitive —
/// so `Domain=`, `domain =`, and ` DOMAIN = x ` are all caught. The first
/// `;`-segment is the cookie's own `name=value` and is never an attribute, so
/// it is kept verbatim (its value may itself contain `domain=`).
fn scrub_cookie_domain(value: &str) -> String {
    value
        .split(';')
        .enumerate()
        .filter(|(index, attribute)| {
            *index == 0 || {
                let name = attribute.split('=').next().unwrap_or("").trim();
                !name.eq_ignore_ascii_case("domain")
            }
        })
        .map(|(_, attribute)| attribute)
        .collect::<Vec<_>>()
        .join(";")
}

/// Map a typed failure onto a wire frame, redacting `Unavailable` detail
/// (which may carry a workspace path / loopback port / library diagnostic).
fn failure_frame(failure: &GatewayFailure) -> gateway::ProxyFrame {
    use gateway::FailureKind;
    let (kind, mut detail) = match failure {
        GatewayFailure::Invalid(detail) => (FailureKind::Invalid, detail.clone()),
        GatewayFailure::Forbidden(detail) => (FailureKind::Forbidden, detail.clone()),
        GatewayFailure::NotFound(detail) => (FailureKind::NotFound, detail.clone()),
        GatewayFailure::Conflict(detail) => (FailureKind::Conflict, detail.clone()),
        GatewayFailure::Unavailable(_) => (
            FailureKind::Unavailable,
            "gateway publisher is unavailable".to_string(),
        ),
    };
    detail.truncate(MAX_ERROR_BYTES);
    gateway::ProxyFrame::Failure(gateway::ProxyFailure { kind, detail })
}

fn failure_from(failure: gateway::ProxyFailure) -> GatewayFailure {
    use gateway::FailureKind;
    match failure.kind {
        FailureKind::Invalid => GatewayFailure::Invalid(failure.detail),
        FailureKind::Forbidden => GatewayFailure::Forbidden(failure.detail),
        FailureKind::NotFound => GatewayFailure::NotFound(failure.detail),
        FailureKind::Conflict => GatewayFailure::Conflict(failure.detail),
        FailureKind::Unavailable => GatewayFailure::Unavailable(failure.detail),
    }
}

async fn write_frame<S: AsyncWrite + Unpin>(
    stream: &mut S,
    frame: &gateway::ProxyFrame,
) -> std::io::Result<()> {
    let bytes = gateway::encode_frame(frame)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    stream.write_all(&bytes).await
}

/// Read one frame, filling `buf` from the stream until a full frame is
/// available. Leftover bytes stay in `buf` for the next call.
async fn read_frame<S: AsyncRead + Unpin>(
    stream: &mut S,
    buf: &mut Vec<u8>,
) -> Result<gateway::ProxyFrame, GatewayFailure> {
    loop {
        match gateway::decode_frame(buf) {
            Ok((frame, consumed)) => {
                buf.drain(..consumed);
                return Ok(frame);
            }
            Err(gateway::FrameError::Incomplete) => {
                let mut chunk = [0u8; 8192];
                let read = stream
                    .read(&mut chunk)
                    .await
                    .map_err(|error| GatewayFailure::Unavailable(error.to_string()))?;
                if read == 0 {
                    return Err(GatewayFailure::Unavailable(
                        "gateway stream closed mid-frame".into(),
                    ));
                }
                buf.extend_from_slice(&chunk[..read]);
            }
            Err(gateway::FrameError::Malformed(detail)) => {
                return Err(GatewayFailure::Unavailable(format!(
                    "malformed gateway frame: {detail}"
                )));
            }
        }
    }
}

/// Publisher → caller: a `ResponseHead`, then the body DRAINED from its
/// channel as `MAX_CHUNK_BYTES` `BodyChunk` frames (flushed per chunk — SSE
/// latency), then `End`. A pre-head failure is a single `Failure` frame; a
/// mid-stream failure emits `Failure` after the chunks already sent (the
/// caller aborts — truncation). The drain deliberately has NO deadline.
async fn write_proxy_response<S: AsyncWrite + Unpin>(
    stream: &mut S,
    outcome: Result<GatewayResponse, GatewayFailure>,
) -> std::io::Result<()> {
    match outcome {
        Ok(mut response) => {
            if let Err(error) = gateway::validate_response_head(&response.head) {
                write_frame(stream, &failure_frame(&GatewayFailure::Unavailable(error))).await?;
                return stream.flush().await;
            }
            write_frame(stream, &gateway::ProxyFrame::ResponseHead(response.head)).await?;
            stream.flush().await?;
            while let Some(item) = response.body.recv().await {
                match item {
                    Ok(chunk) => {
                        for piece in chunk.chunks(gateway::MAX_CHUNK_BYTES) {
                            write_frame(stream, &gateway::ProxyFrame::BodyChunk(piece.to_vec()))
                                .await?;
                        }
                        stream.flush().await?;
                    }
                    Err(failure) => {
                        write_frame(stream, &failure_frame(&failure)).await?;
                        return stream.flush().await;
                    }
                }
            }
            write_frame(stream, &gateway::ProxyFrame::End).await?;
        }
        Err(failure) => write_frame(stream, &failure_frame(&failure)).await?,
    }
    stream.flush().await
}

/// Caller side, head only: `ResponseHead` (validated) or the failure that
/// replaced it. Leftover stream bytes stay in `buf` for the body pump.
async fn read_proxy_head<S: AsyncRead + Unpin>(
    stream: &mut S,
    buf: &mut Vec<u8>,
) -> Result<gateway::ProxyResponseHead, GatewayFailure> {
    let head = match read_frame(stream, buf).await? {
        gateway::ProxyFrame::ResponseHead(head) => head,
        gateway::ProxyFrame::Failure(failure) => return Err(failure_from(failure)),
        _ => {
            return Err(GatewayFailure::Unavailable(
                "publisher did not open with a response head".into(),
            ));
        }
    };
    gateway::validate_response_head(&head).map_err(GatewayFailure::Unavailable)?;
    Ok(head)
}

/// Frame → chunk pump for a streamed response body. Runs until `End`,
/// `Failure`, overflow of the RUNNING cap (`0` = unbounded — the old buffered
/// clamp is gone; per-frame size stays codec-bounded), or receiver hangup.
fn spawn_body_pump<S: AsyncRead + Unpin + Send + 'static>(
    mut stream: S,
    mut buf: Vec<u8>,
    max_response_bytes: u64,
) -> noded::GatewayBody {
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<bytes::Bytes, GatewayFailure>>(16);
    tokio::spawn(async move {
        let mut total: u64 = 0;
        loop {
            let frame = match read_frame(&mut stream, &mut buf).await {
                Ok(frame) => frame,
                Err(failure) => {
                    let _ = tx.send(Err(failure)).await;
                    return;
                }
            };
            match frame {
                gateway::ProxyFrame::BodyChunk(chunk) => {
                    total = total.saturating_add(chunk.len() as u64);
                    let over_cap = max_response_bytes != 0 && total > max_response_bytes;
                    if over_cap {
                        let _ = tx
                            .send(Err(GatewayFailure::Unavailable(
                                "publisher exceeded the response cap".into(),
                            )))
                            .await;
                        return;
                    }
                    if tx.send(Ok(bytes::Bytes::from(chunk))).await.is_err() {
                        return; // caller went away; drop the stream
                    }
                }
                gateway::ProxyFrame::End => return,
                gateway::ProxyFrame::Failure(failure) => {
                    let _ = tx.send(Err(failure_from(failure))).await;
                    return;
                }
                _ => {
                    let _ = tx
                        .send(Err(GatewayFailure::Unavailable(
                            "unexpected frame in gateway response body".into(),
                        )))
                        .await;
                    return;
                }
            }
        }
    });
    rx
}

use duckfs_core::to_hex as hex_bytes;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn streamed_response_arrives_and_zero_cap_is_unbounded() {
        use tokio::io::AsyncWriteExt as _;
        let (mut writer, mut reader) = tokio::io::duplex(64 * 1024);
        let head = gateway::ProxyResponseHead { status: 200, headers: vec![] };
        let big = vec![0xABu8; 5 * 1024 * 1024]; // > the old 4 MiB buffered clamp
        let mut frames =
            gateway::encode_frame(&gateway::ProxyFrame::ResponseHead(head)).unwrap();
        for chunk in big.chunks(gateway::MAX_CHUNK_BYTES) {
            frames.extend(
                gateway::encode_frame(&gateway::ProxyFrame::BodyChunk(chunk.to_vec())).unwrap(),
            );
        }
        frames.extend(gateway::encode_frame(&gateway::ProxyFrame::End).unwrap());
        tokio::spawn(async move { writer.write_all(&frames).await.unwrap() });

        let mut buf = Vec::new();
        let got_head = read_proxy_head(&mut reader, &mut buf).await.unwrap();
        assert_eq!(got_head.status, 200);
        let mut body = spawn_body_pump(reader, buf, 0);
        let mut total = Vec::new();
        while let Some(chunk) = body.recv().await {
            total.extend_from_slice(&chunk.unwrap());
        }
        assert_eq!(total, big, "a 5 MiB body must stream through with cap 0");
    }

    #[tokio::test]
    async fn running_cap_aborts_mid_stream() {
        use tokio::io::AsyncWriteExt as _;
        let (mut writer, mut reader) = tokio::io::duplex(64 * 1024);
        let head = gateway::ProxyResponseHead { status: 200, headers: vec![] };
        let mut frames =
            gateway::encode_frame(&gateway::ProxyFrame::ResponseHead(head)).unwrap();
        for _ in 0..4 {
            frames.extend(
                gateway::encode_frame(&gateway::ProxyFrame::BodyChunk(vec![0u8; 1024])).unwrap(),
            );
        }
        frames.extend(gateway::encode_frame(&gateway::ProxyFrame::End).unwrap());
        tokio::spawn(async move { writer.write_all(&frames).await.unwrap() });

        let mut buf = Vec::new();
        read_proxy_head(&mut reader, &mut buf).await.unwrap();
        let mut body = spawn_body_pump(reader, buf, 2048);
        let mut seen = 0usize;
        let mut aborted = false;
        while let Some(item) = body.recv().await {
            match item {
                Ok(chunk) => seen += chunk.len(),
                Err(_) => {
                    aborted = true;
                    break;
                }
            }
        }
        assert!(aborted, "exceeding the running cap must surface an error item");
        assert!(seen <= 2048);
    }

    #[test]
    fn set_cookie_domain_is_scrubbed_to_host_only() {
        // A publisher must not be able to plant a cookie readable on another
        // account's duck origins.
        assert_eq!(
            scrub_cookie_domain("s=1; Path=/; Domain=.duck; HttpOnly"),
            "s=1; Path=/; HttpOnly"
        );
        assert_eq!(
            scrub_cookie_domain("s=1; domain=other.duck"),
            "s=1"
        );
        // Whitespace and casing around the attribute name must not sneak a
        // Domain through — a browser trims these and honors the attribute.
        assert_eq!(
            scrub_cookie_domain("s=1; Domain =evil.duck; Path=/"),
            "s=1; Path=/"
        );
        assert_eq!(scrub_cookie_domain("s=1;  DOMAIN = evil.duck "), "s=1");
        // Everything else survives untouched, and the cookie's own value —
        // which may legitimately contain "domain=" — is never treated as an
        // attribute.
        assert_eq!(
            scrub_cookie_domain("s=domain=x; Path=/; Secure; SameSite=Lax"),
            "s=domain=x; Path=/; Secure; SameSite=Lax"
        );
    }

    /// Round-trip one streamed response over the frame codec and collect it.
    async fn round_trip(
        head: gateway::ProxyResponseHead,
        body: Vec<u8>,
        cap: u64,
    ) -> Result<(gateway::ProxyResponseHead, Vec<u8>), GatewayFailure> {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        tx.try_send(Ok(bytes::Bytes::from(body))).unwrap();
        drop(tx);
        let response = GatewayResponse { head, body: rx };
        let (mut writer, reader) = tokio::io::duplex(1 << 20);
        let pump = tokio::spawn(async move {
            let _ = write_proxy_response(&mut writer, Ok(response)).await;
        });
        let result = async {
            let mut streamed = read_streamed_response(reader, cap).await?;
            let body = noded::collect_body(&mut streamed.body).await?;
            Ok((streamed.head, body))
        }
        .await;
        let _ = pump.await;
        result
    }

    #[tokio::test]
    async fn response_codec_is_bounded_and_preserves_safe_metadata() {
        let head = gateway::ProxyResponseHead {
            status: 201,
            headers: vec![gateway::ProxyHeader {
                name: "content-type".into(),
                value: "application/json".into(),
            }],
        };
        let (got_head, got_body) = round_trip(head.clone(), br#"{"ok":true}"#.to_vec(), 1024)
            .await
            .unwrap();
        assert_eq!(got_head, head);
        assert_eq!(got_body, br#"{"ok":true}"#);

        let (mut writer, mut reader) = tokio::io::duplex(2048);
        write_proxy_response(
            &mut writer,
            Err(GatewayFailure::Forbidden("audience denied".into())),
        )
        .await
        .unwrap();
        let mut buf = Vec::new();
        assert!(matches!(
            read_proxy_head(&mut reader, &mut buf).await,
            Err(GatewayFailure::Forbidden(detail)) if detail == "audience denied"
        ));

        let (mut writer, mut reader) = tokio::io::duplex(2048);
        write_proxy_response(
            &mut writer,
            Err(GatewayFailure::Unavailable(
                "/home/alice/workspace on 127.0.0.1:3000".into(),
            )),
        )
        .await
        .unwrap();
        let mut buf = Vec::new();
        assert!(matches!(
            read_proxy_head(&mut reader, &mut buf).await,
            Err(GatewayFailure::Unavailable(detail))
                if detail == "gateway publisher is unavailable"
        ));
    }

    #[tokio::test]
    async fn frame_codec_chunks_large_body_and_enforces_cap() {
        // A body larger than one chunk is split into multiple BodyChunk frames
        // and reassembled exactly.
        let big = vec![7u8; gateway::MAX_CHUNK_BYTES * 2 + 100];
        let head = gateway::ProxyResponseHead { status: 200, headers: vec![] };
        let (_, got_body) = round_trip(head, big.clone(), (gateway::MAX_CHUNK_BYTES * 3) as u64)
            .await
            .unwrap();
        assert_eq!(got_body, big);

        // A body past the caller's RUNNING cap is rejected mid-stream (the
        // head has already arrived; the failure surfaces from the body pump).
        let head = gateway::ProxyResponseHead { status: 200, headers: vec![] };
        let capped = round_trip(head, vec![1u8; 5000], 1000).await;
        assert!(matches!(capped, Err(GatewayFailure::Unavailable(_))));
    }

    #[tokio::test]
    async fn ws_upgrade_bridges_frames_over_the_mesh() {
        use tokio_tungstenite::tungstenite::Message;
        // A WebSocket echo upstream on loopback.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(socket).await.unwrap();
            while let Some(Ok(message)) = ws.next().await {
                match message {
                    Message::Text(_) | Message::Binary(_) => {
                        if ws.send(message).await.is_err() {
                            break;
                        }
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
        });

        let publisher = [2u8; 32];
        let caller = [3u8; 32];
        let member = ed25519::PrivateKey::from_seed(44);
        let route = signed_route(&member, publisher, gateway::RouteAudience::Network, true);
        let owner = account(vec![1; 32], publisher, &member);
        let caller_view = account(vec![9; 32], caller, &ed25519::PrivateKey::from_seed(55));

        let workspace = tempfile::tempdir().unwrap();
        let routes = crate::gateway_routes::LocalRoutes {
            version: 1,
            routes: vec![crate::gateway_routes::LocalRoute {
                name: gateway::RouteName::named("api"),
                port,
            }],
        };
        std::fs::write(
            workspace.path().join(crate::gateway_routes::FILE_NAME),
            serde_json::to_vec_pretty(&routes).unwrap(),
        )
        .unwrap();

        // Fake node actor: route → publisher authority → caller account.
        let (commands, mut requests) = mpsc::channel(4);
        tokio::spawn(async move {
            let replies: Vec<Vec<u8>> = vec![
                gateway::encode_reply(&gateway::GatewayReply::Route(Box::new(Some(route)))),
                identity::encode_reply(&identity::IdentityReply::Account(Some(owner))),
                identity::encode_reply(&identity::IdentityReply::Account(Some(caller_view))),
            ];
            for bytes in replies {
                let NodeCommand::Query { reply, .. } = requests.next().await.unwrap() else {
                    panic!("expected a query")
                };
                let _ = reply.send(Ok(bytes));
            }
        });

        let (mut client, server) = tokio::io::duplex(4096);
        let head = gateway::ProxyRequestHead {
            account_id: vec![1; 32],
            name: gateway::RouteName::named("api"),
            revision: 4,
            method: gateway::RouteMethod::Get,
            path_and_query: "/socket".into(),
            headers: vec![],
            body_len: 0,
            upgrade: true,
        };
        let workspace_path = workspace.path().to_path_buf();
        tokio::spawn(async move {
            serve_ws(&commands, &workspace_path, &publisher, &caller, &head, server).await;
        });

        // The upgrade is acknowledged with a 101, then a frame round-trips
        // through the loopback WebSocket and back.
        let mut buf = Vec::new();
        match read_frame(&mut client, &mut buf).await.unwrap() {
            gateway::ProxyFrame::ResponseHead(head) => assert_eq!(head.status, 101),
            other => panic!("expected a 101 upgrade ack, got {other:?}"),
        }
        write_frame(
            &mut client,
            &gateway::ProxyFrame::WsFrame {
                binary: false,
                payload: b"ping".to_vec(),
            },
        )
        .await
        .unwrap();
        client.flush().await.unwrap();
        match read_frame(&mut client, &mut buf).await.unwrap() {
            gateway::ProxyFrame::WsFrame { binary, payload } => {
                assert!(!binary);
                assert_eq!(payload, b"ping");
            }
            other => panic!("expected an echoed ws frame, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn caller_ws_pump_bridges_browser_channel_to_mesh() {
        // A fake publisher end of the mesh: send the 101 ack, then echo frames.
        let (mesh_caller, mesh_publisher) = tokio::io::duplex(4096);
        tokio::spawn(async move {
            let mut publisher = mesh_publisher;
            write_frame(
                &mut publisher,
                &gateway::ProxyFrame::ResponseHead(gateway::ProxyResponseHead {
                    status: 101,
                    headers: vec![],
                }),
            )
            .await
            .unwrap();
            publisher.flush().await.unwrap();
            let mut buf = Vec::new();
            while let Ok(frame @ gateway::ProxyFrame::WsFrame { .. }) =
                read_frame(&mut publisher, &mut buf).await
            {
                if write_frame(&mut publisher, &frame).await.is_err() {
                    break;
                }
                let _ = publisher.flush().await;
            }
        });

        let (to_browser_tx, mut to_browser_rx) = tokio::sync::mpsc::channel(8);
        let (from_browser_tx, from_browser_rx) = tokio::sync::mpsc::channel(8);
        tokio::spawn(caller_ws_pump(mesh_caller, to_browser_tx, from_browser_rx));

        // A browser message crosses to the mesh, is echoed, and comes back.
        from_browser_tx
            .send(noded::GatewayWsMsg::Text("hi".into()))
            .await
            .unwrap();
        assert_eq!(
            to_browser_rx.recv().await,
            Some(noded::GatewayWsMsg::Text("hi".into()))
        );
    }

    #[tokio::test]
    async fn loopback_ws_upgrade_bridges_browser_to_loopback_upstream() {
        use tokio_tungstenite::tungstenite::Message;
        // WS echo upstream.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(socket).await.unwrap();
            while let Some(Ok(message)) = ws.next().await {
                match message {
                    Message::Text(_) | Message::Binary(_) => {
                        if ws.send(message).await.is_err() {
                            break;
                        }
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
        });

        // An Owner-audience upgrade route: for a loopback upgrade the caller is
        // the publisher node itself, so the owner account admits it.
        let publisher = [2u8; 32];
        let member = ed25519::PrivateKey::from_seed(44);
        let route = signed_route(&member, publisher, gateway::RouteAudience::Owner, true);
        let owner = account(vec![1; 32], publisher, &member);
        let workspace = tempfile::tempdir().unwrap();
        let routes = crate::gateway_routes::LocalRoutes {
            version: 1,
            routes: vec![crate::gateway_routes::LocalRoute {
                name: gateway::RouteName::named("api"),
                port,
            }],
        };
        std::fs::write(
            workspace.path().join(crate::gateway_routes::FILE_NAME),
            serde_json::to_vec_pretty(&routes).unwrap(),
        )
        .unwrap();
        let (commands, mut requests) = mpsc::channel(4);
        tokio::spawn(async move {
            let replies: Vec<Vec<u8>> = vec![
                gateway::encode_reply(&gateway::GatewayReply::Route(Box::new(Some(route)))),
                identity::encode_reply(&identity::IdentityReply::Account(Some(owner.clone()))),
                identity::encode_reply(&identity::IdentityReply::Account(Some(owner))),
            ];
            for bytes in replies {
                let NodeCommand::Query { reply, .. } = requests.next().await.unwrap() else {
                    panic!("expected a query")
                };
                let _ = reply.send(Ok(bytes));
            }
        });

        // The loopback Upgrade path the client half runs: serve_ws piped to
        // caller_ws_pump over a local duplex.
        let (server_end, caller_end) = tokio::io::duplex(64 * 1024);
        let head = gateway::ProxyRequestHead {
            account_id: vec![1; 32],
            name: gateway::RouteName::named("api"),
            revision: 4,
            method: gateway::RouteMethod::Get,
            path_and_query: "/socket".into(),
            headers: vec![],
            body_len: 0,
            upgrade: true,
        };
        let workspace_path = workspace.path().to_path_buf();
        tokio::spawn(async move {
            serve_ws(&commands, &workspace_path, &publisher, &publisher, &head, server_end).await;
        });
        let (to_browser_tx, mut to_browser_rx) = tokio::sync::mpsc::channel(8);
        let (from_browser_tx, from_browser_rx) = tokio::sync::mpsc::channel(8);
        tokio::spawn(caller_ws_pump(caller_end, to_browser_tx, from_browser_rx));

        // A browser message crosses caller_ws_pump -> mesh -> serve_ws -> the
        // loopback WS echo and back.
        from_browser_tx
            .send(noded::GatewayWsMsg::Text("ping".into()))
            .await
            .unwrap();
        assert_eq!(
            to_browser_rx.recv().await,
            Some(noded::GatewayWsMsg::Text("ping".into()))
        );
    }

    #[test]
    fn admission_is_service_flow_and_member_scoped() {
        use data_plane::AdmissionPolicy as _;
        let signer = ed25519::PrivateKey::from_seed(99);
        let book = OverlayBook::new("test".into());
        book.set_peers(std::iter::once(&signer.public_key()));
        let peer = PeerId(signer.public_key().as_ref().try_into().unwrap());
        assert!(book.permits(peer, Service::Gateway, proxy_flow()));
        assert!(!book.permits(peer, Service::StateSync, proxy_flow()));
        assert!(!book.permits(peer, Service::Gateway, FlowId::from_raw(9)));
        assert!(!book.permits(PeerId([8; 32]), Service::Gateway, proxy_flow()));
    }

    fn signed_route(
        signer: &ed25519::PrivateKey,
        publisher: [u8; 32],
        audience: gateway::RouteAudience,
        allow_upgrade: bool,
    ) -> gateway::RouteRecord {
        let statement = gateway::RouteStatement {
            version: 1,
            chain_id: "test".into(),
            account_id: vec![1; 32],
            name: gateway::RouteName::named("api"),
            publisher_node: publisher.to_vec(),
            revision: 4,
            route: Some(gateway::RouteDefinition {
                target: gateway::RouteTarget::LoopbackHttp,
                policy: gateway::RoutePolicy {
                    audience,
                    methods: vec![gateway::RouteMethod::Get, gateway::RouteMethod::Post],
                    max_request_bytes: 1024,
                    max_response_bytes: 4096,
                    allow_authorization: false,
                    allow_upgrade,
                },
            }),
        };
        gateway::RouteRecord {
            authorization: gateway::MemberAuthorization {
                signer: signer.public_key().as_ref().to_vec(),
                signature: signer
                    .sign(
                        gateway::GATEWAY_ROUTE_NS,
                        &gateway::route_signing_preimage(&statement).unwrap(),
                    )
                    .as_ref()
                    .to_vec(),
            },
            statement,
        }
    }

    fn account(id: Vec<u8>, node: [u8; 32], member: &ed25519::PrivateKey) -> identity::AccountView {
        identity::AccountView {
            account_id: id,
            display_name: None,
            avatar: None,
            bio: None,
            nonce: 0,
            member_keys: vec![identity::MemberKeyView {
                pubkey: member.public_key().as_ref().to_vec(),
                kind: identity::KeyKind::Ed25519,
                label: None,
                added_at: 0,
            }],
            nodes: vec![identity::NodeView {
                node_key: node.to_vec(),
                label: None,
            }],
            updated_at: 0,
        }
    }

    #[tokio::test]
    async fn loopback_proxy_forwards_cookie_and_verified_caller() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = vec![0u8; 4096];
            let read = socket.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..read]);
            assert!(request.starts_with("POST /items?source=duck HTTP/1.1\r\n"));
            let lower = request.to_ascii_lowercase();
            // The mesh-verified caller identity is injected; Cookie now flows
            // end to end (v1 stripped it); a caller-set x-duck-* never appears
            // (it is rejected at decode and stripped at forward).
            assert!(lower.contains("x-duck-caller-account: "));
            assert_eq!(lower.matches("x-duck-caller-account:").count(), 1);
            assert!(lower.contains("content-type: application/json"));
            assert!(lower.contains("cookie: session=abc"));
            assert!(request.ends_with("{\"name\":\"quack\"}"));
            socket
                .write_all(
                    b"HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nSet-Cookie: sid=xyz\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}",
                )
                .await
                .unwrap();
        });

        let workspace = tempfile::tempdir().unwrap();
        let routes = crate::gateway_routes::LocalRoutes {
            version: 1,
            routes: vec![crate::gateway_routes::LocalRoute {
                name: gateway::RouteName::named("api"),
                port,
            }],
        };
        std::fs::write(
            workspace.path().join(crate::gateway_routes::FILE_NAME),
            serde_json::to_vec_pretty(&routes).unwrap(),
        )
        .unwrap();

        let publisher = [2u8; 32];
        let caller = [3u8; 32];
        let member = ed25519::PrivateKey::from_seed(44);
        let route = signed_route(&member, publisher, gateway::RouteAudience::Network, false);
        let (commands, mut requests) = mpsc::channel(4);
        tokio::spawn(async move {
            let NodeCommand::Query { target, reply, .. } = requests.next().await.unwrap() else {
                panic!("route query");
            };
            assert_eq!(target, "gateway");
            let _ = reply.send(Ok(gateway::encode_reply(&gateway::GatewayReply::Route(
                Box::new(Some(route.clone())),
            ))));

            let NodeCommand::Query { target, reply, .. } = requests.next().await.unwrap() else {
                panic!("publisher authority query");
            };
            assert_eq!(target, "identity");
            let _ = reply.send(Ok(identity::encode_reply(
                &identity::IdentityReply::Account(Some(account(vec![1; 32], publisher, &member))),
            )));

            let NodeCommand::Query { target, reply, .. } = requests.next().await.unwrap() else {
                panic!("caller account query");
            };
            assert_eq!(target, "identity");
            let caller_member = ed25519::PrivateKey::from_seed(55);
            let _ = reply.send(Ok(identity::encode_reply(
                &identity::IdentityReply::Account(Some(account(
                    vec![9; 32],
                    caller,
                    &caller_member,
                ))),
            )));
        });
        let body = br#"{"name":"quack"}"#;
        let response = serve_current(
            &commands,
            workspace.path(),
            &publisher,
            &caller,
            &gateway::ProxyRequestHead {
                account_id: vec![1; 32],
                name: gateway::RouteName::named("api"),
                revision: 4,
                method: gateway::RouteMethod::Post,
                path_and_query: "/items?source=duck".into(),
                headers: vec![
                    gateway::ProxyHeader {
                        name: "content-type".into(),
                        value: "application/json".into(),
                    },
                    gateway::ProxyHeader {
                        name: "cookie".into(),
                        value: "session=abc".into(),
                    },
                ],
                body_len: body.len() as u64,
                upgrade: false,
            },
            body,
        )
        .await
        .unwrap();
        let mut response = response;
        assert_eq!(response.head.status, 201);
        assert_eq!(
            noded::collect_body(&mut response.body).await.unwrap(),
            br#"{"ok":true}"#
        );
        // Set-Cookie now flows back (v1 stripped it).
        let set_cookie = response
            .head
            .headers
            .iter()
            .find(|header| header.name == "set-cookie")
            .expect("set-cookie flows through");
        assert_eq!(set_cookie.value, "sid=xyz");
    }

    #[tokio::test]
    async fn owner_only_route_denies_remote_account_before_loopback() {
        let workspace = tempfile::tempdir().unwrap();
        let publisher = [2u8; 32];
        let caller = [3u8; 32];
        let member = ed25519::PrivateKey::from_seed(44);
        let route = signed_route(&member, publisher, gateway::RouteAudience::Owner, false);
        let (commands, mut requests) = mpsc::channel(4);
        tokio::spawn(async move {
            let NodeCommand::Query { reply, .. } = requests.next().await.unwrap() else {
                panic!()
            };
            let _ = reply.send(Ok(gateway::encode_reply(&gateway::GatewayReply::Route(
                Box::new(Some(route)),
            ))));
            let NodeCommand::Query { reply, .. } = requests.next().await.unwrap() else {
                panic!()
            };
            let _ = reply.send(Ok(identity::encode_reply(
                &identity::IdentityReply::Account(Some(account(vec![1; 32], publisher, &member))),
            )));
            let NodeCommand::Query { reply, .. } = requests.next().await.unwrap() else {
                panic!()
            };
            let caller_member = ed25519::PrivateKey::from_seed(55);
            let _ = reply.send(Ok(identity::encode_reply(
                &identity::IdentityReply::Account(Some(account(
                    vec![9; 32],
                    caller,
                    &caller_member,
                ))),
            )));
        });
        let error = serve_current(
            &commands,
            workspace.path(),
            &publisher,
            &caller,
            &gateway::ProxyRequestHead {
                account_id: vec![1; 32],
                name: gateway::RouteName::named("api"),
                revision: 4,
                method: gateway::RouteMethod::Get,
                path_and_query: "/".into(),
                headers: vec![],
                body_len: 0,
                upgrade: false,
            },
            &[],
        )
        .await
        .unwrap_err();
        assert!(matches!(error, GatewayFailure::Forbidden(_)));
    }

    fn stat_reply(path: String, size: u64) -> Vec<u8> {
        duckfs_core::encode_reply(&FilesReply::Stat(Some(duckfs_core::EntryInfo {
            path,
            kind: EntryKindWire::File,
            size,
            exec: false,
            object: "bb".repeat(32),
            meta: std::collections::BTreeMap::new(),
        })))
    }

    async fn serve_content_head(
        declared: &[u8],
        actual: &[u8],
    ) -> Result<GatewayResponse, GatewayFailure> {
        let publisher = [2u8; 32];
        let member = ed25519::PrivateKey::from_seed(77);
        // The signed statement binds only the manifest hash; the manifest (a
        // DuckFS file) lists the content file and its own hash.
        let manifest = gateway::RouteManifest {
            default_path: Some("index.html".into()),
            files: vec![gateway::ContentFile {
                path: "index.html".into(),
                mime: "text/html".into(),
                size: declared.len() as u64,
                sha256: hex_bytes(&Sha256::digest(declared)),
            }],
        };
        let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
        let statement = gateway::RouteStatement {
            version: 1,
            chain_id: "test".into(),
            account_id: vec![1; 32],
            name: gateway::RouteName::apex(),
            publisher_node: publisher.to_vec(),
            revision: 1,
            route: Some(gateway::RouteDefinition {
                target: gateway::RouteTarget::DuckFs {
                    manifest_sha256: hex_bytes(&Sha256::digest(&manifest_bytes)),
                },
                policy: gateway::RoutePolicy {
                    audience: gateway::RouteAudience::Owner,
                    methods: vec![gateway::RouteMethod::Get, gateway::RouteMethod::Head],
                    max_request_bytes: 0,
                    max_response_bytes: 1024,
                    allow_authorization: false,
                    allow_upgrade: false,
                },
            }),
        };
        let route = gateway::RouteRecord {
            authorization: gateway::MemberAuthorization {
                signer: member.public_key().as_ref().to_vec(),
                signature: member
                    .sign(
                        gateway::GATEWAY_ROUTE_NS,
                        &gateway::route_signing_preimage(&statement).unwrap(),
                    )
                    .as_ref()
                    .to_vec(),
            },
            statement,
        };
        let owner = account(vec![1; 32], publisher, &member);
        let manifest_path = gateway_path(&publisher, "_apex", gateway::MANIFEST_FILE);
        let file_path = gateway_path(&publisher, "_apex", "index.html");
        let manifest_len = manifest_bytes.len() as u64;
        let manifest_b64 = STANDARD.encode(&manifest_bytes);
        let actual_b64 = STANDARD.encode(actual);
        let actual_len = actual.len() as u64;
        // Ordered replies: route → identity ×2 → refs → manifest stat/read →
        // file stat/read.
        let replies: Vec<Vec<u8>> = vec![
            gateway::encode_reply(&gateway::GatewayReply::Route(Box::new(Some(route)))),
            identity::encode_reply(&identity::IdentityReply::Account(Some(owner.clone()))),
            identity::encode_reply(&identity::IdentityReply::Account(Some(owner))),
            duckfs_core::encode_reply(&FilesReply::Refs(duckfs_core::RefsInfo {
                head: Some("aa".repeat(32)),
                pins: std::collections::BTreeMap::new(),
                window_len: 1,
            })),
            stat_reply(manifest_path, manifest_len),
            duckfs_core::encode_reply(&FilesReply::Read {
                b64: manifest_b64,
                eof: true,
            }),
            stat_reply(file_path, actual_len),
            duckfs_core::encode_reply(&FilesReply::Read {
                b64: actual_b64,
                eof: true,
            }),
        ];
        let (commands, mut requests) = mpsc::channel(8);
        tokio::spawn(async move {
            for bytes in replies {
                let NodeCommand::Query { reply, .. } = requests.next().await.unwrap() else {
                    panic!("expected a query")
                };
                let _ = reply.send(Ok(bytes));
            }
        });
        serve_current(
            &commands,
            Path::new("."),
            &publisher,
            &publisher,
            &gateway::ProxyRequestHead {
                account_id: vec![1; 32],
                name: gateway::RouteName::apex(),
                revision: 1,
                method: gateway::RouteMethod::Head,
                path_and_query: "/".into(),
                headers: vec![],
                body_len: 0,
                upgrade: false,
            },
            &[],
        )
        .await
    }

    #[tokio::test]
    async fn duckfs_head_verifies_signed_bytes_before_returning_no_body() {
        let mut response = serve_content_head(b"safe", b"safe").await.unwrap();
        assert!(noded::collect_body(&mut response.body).await.unwrap().is_empty());
        assert_eq!(response.head.status, 200);

        let mut empty = serve_content_head(b"", b"").await.unwrap();
        assert!(noded::collect_body(&mut empty.body).await.unwrap().is_empty());

        let error = serve_content_head(b"safe", b"evil").await.unwrap_err();
        assert!(matches!(error, GatewayFailure::Forbidden(_)));
    }
}
