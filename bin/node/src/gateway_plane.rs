//! Secure gateway reverse proxy over the purpose-specific overlay stream.
//!
//! A consumer resolves a signed global route, opens `Service::Gateway` to that
//! exact publisher node, and sends one bounded HTTP-shaped request. The
//! publisher re-resolves the route, revalidates Identity authority, maps the
//! authenticated WireGuard peer to its caller account, enforces the signed
//! audience/method/body/header policy, and only then touches DuckFS or one
//! exact node-local loopback upstream.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
#[cfg(test)]
use commonware_cryptography::Signer as _;
use commonware_cryptography::ed25519;
use data_plane::{
    AddressBook, AdmissionPolicy, BulkPacer, FlowId, OverlaySockets, PeerId, Service, StreamPacing,
    StreamPlaneSpec, StreamPolicy, StreamService, bind_stream_plane,
};
use duckfs_core::{EntryKindWire, FilesQuery, FilesReply};
use futures::channel::{mpsc, oneshot};
use futures::{SinkExt as _, StreamExt as _};
use noded::{GatewayFailure, GatewayJob, GatewayResponse, NodeCommand};
use sha2::{Digest as _, Sha256};
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};

const BIND_RETRY: Duration = Duration::from_secs(3);
const PROXY_IO_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_ERROR_BYTES: usize = 512;
const MAX_PROXY_FRAME_BYTES: usize =
    gateway::MAX_RESPONSE_BODY_BYTES as usize + gateway::MAX_RESPONSE_HEAD_BYTES + 2;

type PlaneSlot = Arc<OnceLock<Arc<StreamService<OverlaySockets>>>>;

pub struct SpawnConfig {
    pub label: String,
    pub book: Arc<OverlayBook>,
    pub me: ed25519::PublicKey,
    pub factory: Arc<dyn data_plane::SocketFactory>,
    pub pacer: BulkPacer,
    pub commands: mpsc::Sender<NodeCommand>,
    pub workspace: PathBuf,
}

fn proxy_flow() -> FlowId {
    FlowId::derive(gateway::PROXY_FLOW_DOMAIN)
}

fn ula_of(namespace: &str, raw: &[u8; 32]) -> std::net::Ipv6Addr {
    wireguard::ula_v6_member_addr(namespace, wireguard::ValidatorIdentity(*raw))
}

/// Forward/reverse identity map and default-deny admission for the gateway.
/// The tracked set follows validator/resident transport membership at cutover.
pub struct OverlayBook {
    namespace: String,
    reverse: RwLock<HashMap<IpAddr, PeerId>>,
}

impl OverlayBook {
    pub fn new(namespace: String) -> Arc<Self> {
        Arc::new(Self {
            namespace,
            reverse: RwLock::new(HashMap::new()),
        })
    }

    pub fn set_peers<'a>(&self, peers: impl Iterator<Item = &'a ed25519::PublicKey>) {
        let reverse = peers
            .map(|key| {
                let raw: [u8; 32] = key.as_ref().try_into().expect("ed25519 keys are 32 bytes");
                (IpAddr::V6(ula_of(&self.namespace, &raw)), PeerId(raw))
            })
            .collect();
        *self.reverse.write().expect("gateway book lock") = reverse;
    }

    fn own_ip(&self, me: &ed25519::PublicKey) -> IpAddr {
        let raw: [u8; 32] = me.as_ref().try_into().expect("ed25519 keys are 32 bytes");
        IpAddr::V6(ula_of(&self.namespace, &raw))
    }

    fn overlay_ip(&self, raw: &[u8; 32]) -> IpAddr {
        IpAddr::V6(ula_of(&self.namespace, raw))
    }
}

impl AddressBook for OverlayBook {
    fn datagram_addr(&self, peer: PeerId) -> Option<SocketAddr> {
        Some(SocketAddr::new(
            self.overlay_ip(&peer.0),
            Service::Gateway.overlay_datagram_port(),
        ))
    }

    fn stream_addr(&self, peer: PeerId) -> Option<SocketAddr> {
        Some(SocketAddr::new(
            self.overlay_ip(&peer.0),
            Service::Gateway.overlay_stream_port(),
        ))
    }

    fn peer_at(&self, src: IpAddr) -> Option<PeerId> {
        self.reverse
            .read()
            .expect("gateway book lock")
            .get(&src)
            .copied()
    }
}

impl AdmissionPolicy for OverlayBook {
    fn permits(&self, peer: PeerId, service: Service, flow: FlowId) -> bool {
        service == Service::Gateway
            && flow == proxy_flow()
            && self
                .reverse
                .read()
                .expect("gateway book lock")
                .values()
                .any(|known| *known == peer)
    }
}

/// Start the local client lane and authenticated overlay server.
pub fn spawn(config: SpawnConfig, mut jobs: tokio::sync::mpsc::Receiver<GatewayJob>) {
    let SpawnConfig {
        label,
        book,
        me,
        factory,
        pacer,
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
                    let result = tokio::time::timeout(PROXY_IO_TIMEOUT, async {
                        if job.publisher_node == own_node {
                            serve_current(
                                &commands, &workspace, &own_node, &own_node, &job.head, &job.body,
                            )
                            .await
                        } else {
                            proxy_remote(
                                &slot,
                                job.publisher_node,
                                job.max_response_bytes,
                                &job.head,
                                &job.body,
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
                    let _ = job.reply.send(result);
                });
            }
        });
    }

    // Server half. Bind retry starts before the userspace WireGuard stack is
    // installed and becomes live automatically once the node owns its ULA.
    tokio::spawn(async move {
        let own = book.own_ip(&me);
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
                eprintln!("[node {label}] gateway plane register failed: {error}");
                return;
            }
        };
        println!("[node {label}] gateway plane: overlay stream bound on {own}");
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
                let outcome = tokio::time::timeout(PROXY_IO_TIMEOUT, async {
                    if hello.intent != gateway::PROXY_INTENT {
                        return Err(GatewayFailure::Invalid(
                            "gateway proxy: unsupported stream intent".into(),
                        ));
                    }
                    let head = gateway::decode_proxy_request_head(&hello.meta)
                        .map_err(GatewayFailure::Invalid)?;
                    let body_len = usize::try_from(head.body_len).map_err(|_| {
                        GatewayFailure::Invalid("gateway proxy: body length overflows usize".into())
                    })?;
                    let mut body = vec![0u8; body_len];
                    stream
                        .read_exact(&mut body)
                        .await
                        .map_err(|error| GatewayFailure::Unavailable(error.to_string()))?;
                    serve_current(&commands, &workspace, &own_node, &requester.0, &head, &body)
                        .await
                })
                .await
                .unwrap_or_else(|_| {
                    Err(GatewayFailure::Unavailable(
                        "gateway proxy request timed out".into(),
                    ))
                });
                let _ = tokio::time::timeout(
                    PROXY_IO_TIMEOUT,
                    write_proxy_response(&mut stream, outcome),
                )
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
    tokio::time::timeout(PROXY_IO_TIMEOUT, async {
        stream
            .write_all(body)
            .await
            .map_err(|error| GatewayFailure::Unavailable(error.to_string()))?;
        stream
            .flush()
            .await
            .map_err(|error| GatewayFailure::Unavailable(error.to_string()))?;
        read_proxy_response(&mut stream, max_response_bytes).await
    })
    .await
    .map_err(|_| GatewayFailure::Unavailable("gateway publisher timed out".into()))?
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
        Ok(gateway::GatewayReply::Routes(_)) => Err(GatewayFailure::Unavailable(
            "gateway returned an unexpected route-list reply".into(),
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
                .any(|candidate| candidate.as_slice() == node) =>
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
        .any(|node| node == &statement.publisher_node);
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
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .timeout(PROXY_IO_TIMEOUT)
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
    if response
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
    let mut response_body = Vec::new();
    let mut chunks = response.bytes_stream();
    while let Some(chunk) = chunks.next().await {
        let chunk = chunk.map_err(|error| GatewayFailure::Unavailable(error.to_string()))?;
        if response_body.len().saturating_add(chunk.len()) as u64 > route.policy.max_response_bytes
        {
            return Err(GatewayFailure::Unavailable(
                "loopback response exceeds the signed route cap".into(),
            ));
        }
        response_body.extend_from_slice(&chunk);
    }
    if head.method == gateway::RouteMethod::Head {
        response_body.clear();
    }
    Ok(GatewayResponse {
        head: response_head,
        body: response_body,
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
    Ok(GatewayResponse {
        head: response_head,
        body: bytes,
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

/// Publisher → caller: a `ResponseHead`, the body split into `MAX_CHUNK_BYTES`
/// `BodyChunk`s, then `End`; a failure is a single `Failure` frame instead.
async fn write_proxy_response<S: AsyncWrite + Unpin>(
    stream: &mut S,
    outcome: Result<GatewayResponse, GatewayFailure>,
) -> std::io::Result<()> {
    match outcome {
        Ok(response) => {
            if let Err(error) = gateway::validate_response_head(&response.head) {
                write_frame(stream, &failure_frame(&GatewayFailure::Unavailable(error))).await?;
                return stream.flush().await;
            }
            write_frame(stream, &gateway::ProxyFrame::ResponseHead(response.head)).await?;
            for chunk in response.body.chunks(gateway::MAX_CHUNK_BYTES) {
                write_frame(stream, &gateway::ProxyFrame::BodyChunk(chunk.to_vec())).await?;
            }
            write_frame(stream, &gateway::ProxyFrame::End).await?;
        }
        Err(failure) => write_frame(stream, &failure_frame(&failure)).await?,
    }
    stream.flush().await
}

/// Caller side (buffered): collect body chunks into one `GatewayResponse`,
/// enforcing the response cap. `max_response_bytes == 0` (unbounded/SSE) is
/// capped at the buffer ceiling on this buffered path — true streaming lands
/// with the duck:// viewer.
async fn read_proxy_response<S: AsyncRead + Unpin>(
    stream: &mut S,
    max_response_bytes: u64,
) -> Result<GatewayResponse, GatewayFailure> {
    let mut buf = Vec::new();
    let head = match read_frame(stream, &mut buf).await? {
        gateway::ProxyFrame::ResponseHead(head) => head,
        gateway::ProxyFrame::Failure(failure) => return Err(failure_from(failure)),
        _ => {
            return Err(GatewayFailure::Unavailable(
                "publisher did not open with a response head".into(),
            ));
        }
    };
    gateway::validate_response_head(&head).map_err(GatewayFailure::Unavailable)?;
    let cap = if max_response_bytes == 0 {
        MAX_PROXY_FRAME_BYTES as u64
    } else {
        max_response_bytes.min(MAX_PROXY_FRAME_BYTES as u64)
    };
    let mut body = Vec::new();
    loop {
        match read_frame(stream, &mut buf).await? {
            gateway::ProxyFrame::BodyChunk(chunk) => {
                if body.len() as u64 + chunk.len() as u64 > cap {
                    return Err(GatewayFailure::Unavailable(
                        "publisher exceeded the response cap".into(),
                    ));
                }
                body.extend_from_slice(&chunk);
            }
            gateway::ProxyFrame::End => break,
            gateway::ProxyFrame::Failure(failure) => return Err(failure_from(failure)),
            _ => {
                return Err(GatewayFailure::Unavailable(
                    "unexpected frame in gateway response body".into(),
                ));
            }
        }
    }
    Ok(GatewayResponse { head, body })
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn response_codec_is_bounded_and_preserves_safe_metadata() {
        let response = GatewayResponse {
            head: gateway::ProxyResponseHead {
                status: 201,
                headers: vec![gateway::ProxyHeader {
                    name: "content-type".into(),
                    value: "application/json".into(),
                }],
            },
            body: br#"{"ok":true}"#.to_vec(),
        };
        let (mut writer, mut reader) = tokio::io::duplex(4096);
        write_proxy_response(&mut writer, Ok(response.clone()))
            .await
            .unwrap();
        assert_eq!(
            read_proxy_response(&mut reader, 1024).await.unwrap(),
            response
        );

        let (mut writer, mut reader) = tokio::io::duplex(2048);
        write_proxy_response(
            &mut writer,
            Err(GatewayFailure::Forbidden("audience denied".into())),
        )
        .await
        .unwrap();
        assert!(matches!(
            read_proxy_response(&mut reader, 0).await,
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
        assert!(matches!(
            read_proxy_response(&mut reader, 0).await,
            Err(GatewayFailure::Unavailable(detail))
                if detail == "gateway publisher is unavailable"
        ));
    }

    #[tokio::test]
    async fn frame_codec_chunks_large_body_and_enforces_cap() {
        // A body larger than one chunk is split into multiple BodyChunk frames
        // and reassembled exactly.
        let big = vec![7u8; gateway::MAX_CHUNK_BYTES * 2 + 100];
        let response = GatewayResponse {
            head: gateway::ProxyResponseHead {
                status: 200,
                headers: vec![],
            },
            body: big.clone(),
        };
        let (mut writer, mut reader) = tokio::io::duplex(1 << 20);
        // The writer can outrun the bounded duplex, so drive it concurrently.
        let pump = tokio::spawn(async move {
            write_proxy_response(&mut writer, Ok(response)).await.unwrap();
        });
        let got = read_proxy_response(&mut reader, (gateway::MAX_CHUNK_BYTES * 3) as u64)
            .await
            .unwrap();
        pump.await.unwrap();
        assert_eq!(got.body, big);

        // A body past the caller's cap is rejected mid-stream.
        let over = GatewayResponse {
            head: gateway::ProxyResponseHead {
                status: 200,
                headers: vec![],
            },
            body: vec![1u8; 5000],
        };
        let (mut writer, mut reader) = tokio::io::duplex(1 << 20);
        let pump = tokio::spawn(async move {
            let _ = write_proxy_response(&mut writer, Ok(over)).await;
        });
        let capped = read_proxy_response(&mut reader, 1000).await;
        let _ = pump.await;
        assert!(matches!(capped, Err(GatewayFailure::Unavailable(_))));
    }

    #[test]
    fn admission_is_service_flow_and_member_scoped() {
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
    ) -> gateway::RouteRecord {
        let statement = gateway::RouteStatement {
            version: gateway::ROUTE_FORMAT_VERSION,
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
                    allow_upgrade: false,
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
            nonce: 0,
            member_keys: vec![identity::MemberKeyView {
                pubkey: member.public_key().as_ref().to_vec(),
                kind: identity::KeyKind::Ed25519,
                label: None,
                added_at: 0,
            }],
            nodes: vec![node.to_vec()],
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
        let route = signed_route(&member, publisher, gateway::RouteAudience::Network);
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
            },
            body,
        )
        .await
        .unwrap();
        assert_eq!(response.head.status, 201);
        assert_eq!(response.body, br#"{"ok":true}"#);
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
        let route = signed_route(&member, publisher, gateway::RouteAudience::Owner);
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
            version: gateway::ROUTE_FORMAT_VERSION,
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
            },
            &[],
        )
        .await
    }

    #[tokio::test]
    async fn duckfs_head_verifies_signed_bytes_before_returning_no_body() {
        let response = serve_content_head(b"safe", b"safe").await.unwrap();
        assert!(response.body.is_empty());
        assert_eq!(response.head.status, 200);

        let empty = serve_content_head(b"", b"").await.unwrap();
        assert!(empty.body.is_empty());

        let error = serve_content_head(b"safe", b"evil").await.unwrap_err();
        assert!(matches!(error, GatewayFailure::Forbidden(_)));
    }
}
