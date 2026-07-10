//! Dedicated loopback HTTP ingress fed by the device TLS helper.
//!
//! Hyper parses every HTTP/1.1 request on a keep-alive connection so Host,
//! membership, Fetch Metadata, and WebSocket Origin policy are never only a
//! first-request check. Each request gets one authenticated provider stream;
//! upgrades become an opaque full-duplex copy only after a valid 101 response.

use std::convert::Infallible;
use std::error::Error as StdError;
use std::net::IpAddr;
use std::sync::Arc;

use bytes::Bytes;
use duckdns::{DuckDnsQuery, DuckDnsReply, ResolvedService, decode_reply, encode_query};
use duckdns_client::Publications;
use futures::SinkExt as _;
use http_body_util::combinators::UnsyncBoxBody;
use http_body_util::{BodyExt as _, Full};
use hyper::body::Incoming;
use hyper::client::conn::http1 as client_http1;
use hyper::server::conn::http1 as server_http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::time::{Duration, Instant, timeout};

use crate::plane;

type BodyError = Box<dyn StdError + Send + Sync>;
type Body = UnsyncBoxBody<Bytes, BodyError>;

/// One down overlay route must not pin a logical service ahead of its healthy
/// providers. The outer budget also keeps a large stale pool from multiplying
/// the per-provider deadline into an unbounded browser request.
const PROVIDER_OPEN_TIMEOUT: Duration = Duration::from_secs(5);
const PROVIDER_SELECTION_TIMEOUT: Duration = Duration::from_secs(15);

pub async fn serve(
    listener: std::net::TcpListener,
    commands: futures::channel::mpsc::Sender<noded::NodeCommand>,
    plane: plane::PlaneSlot,
    me: [u8; 32],
    publications: Arc<Publications>,
    files: noded::ActorNodeApi,
) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::from_std(listener)?;
    loop {
        let (stream, peer) = listener.accept().await?;
        let commands = commands.clone();
        let plane = plane.clone();
        let publications = Arc::clone(&publications);
        let files = files.clone();
        tokio::spawn(async move {
            let service = service_fn(move |request| {
                route(
                    request,
                    peer.ip(),
                    commands.clone(),
                    plane.clone(),
                    me,
                    Arc::clone(&publications),
                    files.clone(),
                )
            });
            let _ = server_http1::Builder::new()
                .keep_alive(true)
                .serve_connection(TokioIo::new(stream), service)
                .with_upgrades()
                .await;
        });
    }
}

async fn route(
    request: Request<Incoming>,
    client_ip: IpAddr,
    mut commands: futures::channel::mpsc::Sender<noded::NodeCommand>,
    plane: plane::PlaneSlot,
    me: [u8; 32],
    publications: Arc<Publications>,
    files: noded::ActorNodeApi,
) -> Result<Response<Body>, Infallible> {
    let response = route_inner(
        request,
        client_ip,
        &mut commands,
        &plane,
        me,
        &publications,
        files,
    )
    .await;
    Ok(response)
}

async fn route_inner(
    mut request: Request<Incoming>,
    client_ip: IpAddr,
    commands: &mut futures::channel::mpsc::Sender<noded::NodeCommand>,
    plane: &plane::PlaneSlot,
    me: [u8; 32],
    publications: &Arc<Publications>,
    files: noded::ActorNodeApi,
) -> Response<Body> {
    // First pass extracts Host and applies the unconditional WebSocket-origin
    // rule. Cross-site policy depends on the resolved replicated declaration.
    let method = request.method().clone();
    let routing =
        match duckdns_client::prepare_headers(&method, request.headers_mut(), client_ip, true) {
            Ok(prepared) => prepared,
            Err(duckdns_client::GatewayError::WebSocketOrigin) => {
                return response(
                    StatusCode::FORBIDDEN,
                    "WebSocket Origin does not match DuckDNS Host\n",
                );
            }
            Err(_) => return response(StatusCode::BAD_REQUEST, "malformed HTTP/1.1 request\n"),
        };
    let name = match duckdns::parse_hostname(&routing.hostname) {
        Ok(name) => name,
        Err(_) => return response(StatusCode::NOT_FOUND, "unpublished DuckDNS service\n"),
    };

    let (reply, receiver) = futures::channel::oneshot::channel();
    if commands
        .send(noded::NodeCommand::Query {
            target: "duckdns".into(),
            req: encode_query(&DuckDnsQuery::Resolve { name }),
            reply,
        })
        .await
        .is_err()
    {
        return response(
            StatusCode::SERVICE_UNAVAILABLE,
            "active workspace is unavailable\n",
        );
    }
    let resolved = match receiver.await {
        Ok(Ok(bytes)) => match decode_reply(&bytes) {
            Ok(DuckDnsReply::Resolved(Some(resolved))) => resolved,
            Ok(DuckDnsReply::Resolved(None)) => {
                return response(StatusCode::NOT_FOUND, "unpublished DuckDNS service\n");
            }
            _ => {
                return response(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "active workspace is unavailable\n",
                );
            }
        },
        _ => {
            return response(
                StatusCode::SERVICE_UNAVAILABLE,
                "active workspace is unavailable\n",
            );
        }
    };

    let prepared = match duckdns_client::prepare_headers(
        &method,
        request.headers_mut(),
        client_ip,
        resolved.allow_cross_site,
    ) {
        Ok(prepared) => prepared,
        Err(duckdns_client::GatewayError::CrossSite)
        | Err(duckdns_client::GatewayError::WebSocketOrigin) => {
            return response(
                StatusCode::FORBIDDEN,
                "browser cross-site request refused\n",
            );
        }
        Err(_) => return response(StatusCode::BAD_REQUEST, "malformed HTTP/1.1 request\n"),
    };

    if !has_standing(commands, &me).await {
        return response(StatusCode::FORBIDDEN, "workspace membership refused\n");
    }

    let mut selected = None;
    let selection_deadline = Instant::now() + PROVIDER_SELECTION_TIMEOUT;
    for provider in ordered_providers(&resolved, &prepared.hostname) {
        let remaining = selection_deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        let attempt = remaining.min(PROVIDER_OPEN_TIMEOUT);
        let Ok(Some(stream)) = timeout(
            attempt,
            open_provider(&provider, &resolved, plane, me, publications, files.clone()),
        )
        .await
        else {
            continue;
        };
        // Handshake failure happens before any request byte reaches this
        // provider and is therefore a safe logical-service failover point.
        if let Ok(connection) = client_http1::handshake(TokioIo::new(stream)).await {
            selected = Some(connection);
            break;
        }
    }
    let Some((mut sender, connection)) = selected else {
        return response(
            StatusCode::BAD_GATEWAY,
            "DuckDNS providers are unreachable\n",
        );
    };
    tokio::spawn(async move {
        let _ = connection.with_upgrades().await;
    });

    let downstream_upgrade = prepared.websocket.then(|| hyper::upgrade::on(&mut request));
    let mut upstream_response = match sender.send_request(request).await {
        Ok(response) => response,
        Err(_) => {
            return response(
                StatusCode::BAD_GATEWAY,
                "DuckDNS provider closed before responding\n",
            );
        }
    };

    if let Some(downstream_upgrade) = downstream_upgrade
        && upstream_response.status() == StatusCode::SWITCHING_PROTOCOLS
    {
        let upstream_upgrade = hyper::upgrade::on(&mut upstream_response);
        tokio::spawn(async move {
            let (Ok(downstream), Ok(upstream)) = tokio::join!(downstream_upgrade, upstream_upgrade)
            else {
                return;
            };
            let mut downstream = TokioIo::new(downstream);
            let mut upstream = TokioIo::new(upstream);
            let _ = tokio::io::copy_bidirectional(&mut downstream, &mut upstream).await;
        });
    }
    upstream_response.map(incoming_body)
}

async fn open_provider(
    provider: &duckdns::ServiceProvider,
    resolved: &ResolvedService,
    plane: &plane::PlaneSlot,
    me: [u8; 32],
    publications: &Arc<Publications>,
    files: noded::ActorNodeApi,
) -> Option<Box<dyn WebStream>> {
    if provider.node.as_slice() == me {
        let publication = publications.get(&resolved.identity).cloned()?;
        let (requester, provider) = tokio::io::duplex(64 * 1024);
        let identity = resolved.identity.clone();
        let publications = Arc::clone(publications);
        tokio::spawn(async move {
            let _ = plane::serve_publication(
                &identity,
                &publications,
                files,
                publication.target,
                provider,
            )
            .await;
        });
        Some(Box::new(requester))
    } else {
        plane::open(plane, &provider.node, &resolved.identity)
            .await
            .ok()
            .map(|stream| Box::new(stream) as Box<dyn WebStream>)
    }
}

trait WebStream: AsyncRead + AsyncWrite + Unpin + Send {}

impl<T> WebStream for T where T: AsyncRead + AsyncWrite + Unpin + Send {}

fn ordered_providers(resolved: &ResolvedService, hostname: &str) -> Vec<duckdns::ServiceProvider> {
    let mut providers = resolved.providers.clone();
    providers.sort_by_key(|provider| {
        let mut preimage = Vec::with_capacity(hostname.len() + provider.node.len());
        preimage.extend_from_slice(hostname.as_bytes());
        preimage.extend_from_slice(&provider.node);
        data_plane::FlowId::derive(&preimage).as_u64()
    });
    providers
}

async fn has_standing(
    commands: &mut futures::channel::mpsc::Sender<noded::NodeCommand>,
    me: &[u8; 32],
) -> bool {
    for (query, validator_reply) in [
        (valset::ValsetQuery::Validators, true),
        (valset::ValsetQuery::Residents, false),
    ] {
        let (reply, receiver) = futures::channel::oneshot::channel();
        if commands
            .send(noded::NodeCommand::Query {
                target: "valset".into(),
                req: valset::encode_query(&query),
                reply,
            })
            .await
            .is_err()
        {
            return false;
        }
        let Ok(Ok(bytes)) = receiver.await else {
            return false;
        };
        let Ok(reply) = valset::decode_reply(&bytes) else {
            return false;
        };
        let members = match (validator_reply, reply) {
            (true, valset::ValsetReply::Validators(members)) => members,
            (false, valset::ValsetReply::Residents(members)) => members,
            _ => return false,
        };
        if members.iter().any(|member| member.as_slice() == me) {
            return true;
        }
    }
    false
}

fn incoming_body(body: Incoming) -> Body {
    body.map_err(|error| Box::new(error) as BodyError)
        .boxed_unsync()
}

fn response(status: StatusCode, message: &'static str) -> Response<Body> {
    Response::builder()
        .status(status)
        .header(hyper::header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(hyper::header::CONTENT_LENGTH, message.len())
        .body(
            Full::new(Bytes::from_static(message.as_bytes()))
                .map_err(|never| match never {})
                .boxed_unsync(),
        )
        .expect("static DuckDNS gateway response")
}

#[cfg(test)]
mod tests {
    use super::*;
    use duckdns::{ServiceIdentity, ServiceProvider, ServiceScope};

    #[test]
    fn logical_provider_order_is_stable_but_not_registry_order() {
        let resolved = ResolvedService {
            identity: ServiceIdentity {
                scope: ServiceScope::Network,
                service: "docs".into(),
            },
            providers: (1..=8)
                .map(|byte| ServiceProvider {
                    node: vec![byte; 32],
                    node_label: format!("n-{byte:012x}"),
                })
                .collect(),
            allow_cross_site: false,
        };
        let first = ordered_providers(&resolved, "docs.team-deadbeef.net.ducktape.quack");
        let second = ordered_providers(&resolved, "docs.team-deadbeef.net.ducktape.quack");
        assert_eq!(first, second);
        assert_ne!(
            first, resolved.providers,
            "order is deterministically shuffled"
        );
    }
}
