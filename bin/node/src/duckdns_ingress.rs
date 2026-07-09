//! Dedicated loopback HTTP ingress fed by the device TLS helper. It resolves
//! the request Host through the actor's committed DuckDNS module view, opens
//! one authenticated web-plane stream, and then becomes an opaque byte proxy.

use std::net::IpAddr;

use duckdns::{DuckDnsQuery, DuckDnsReply, ResolvedService, decode_reply, encode_query};
use futures::SinkExt as _;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::duckdns_plane;

pub(crate) async fn serve(
    listener: std::net::TcpListener,
    commands: futures::channel::mpsc::Sender<noded::NodeCommand>,
    plane: duckdns_plane::PlaneSlot,
    me: [u8; 32],
) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::from_std(listener)?;
    loop {
        let (stream, peer) = listener.accept().await?;
        let commands = commands.clone();
        let plane = plane.clone();
        tokio::spawn(async move {
            let _ = handle(stream, peer.ip(), commands, plane, me).await;
        });
    }
}

async fn handle(
    mut client: tokio::net::TcpStream,
    client_ip: IpAddr,
    mut commands: futures::channel::mpsc::Sender<noded::NodeCommand>,
    plane: duckdns_plane::PlaneSlot,
    me: [u8; 32],
) -> Result<(), ()> {
    let initial = match read_initial_request(&mut client).await {
        Ok(initial) => initial,
        Err(Status::HeadTooLarge) => {
            return respond(&mut client, 431, "request head too large").await;
        }
        Err(Status::BadRequest) => {
            return respond(&mut client, 400, "malformed HTTP/1.1 request").await;
        }
    };
    // First pass extracts/validates Host while deliberately allowing cross-site
    // policy; the authoritative pass below uses the resolved service policy.
    let routing = match duckdns_client::prepare_request(&initial, client_ip, true) {
        Ok(request) => request,
        Err(duckdns_client::GatewayError::WebSocketOrigin) => {
            return respond(
                &mut client,
                403,
                "WebSocket Origin does not match DuckDNS Host",
            )
            .await;
        }
        Err(_) => return respond(&mut client, 400, "malformed HTTP/1.1 request").await,
    };
    let name = match duckdns::parse_hostname(&routing.hostname) {
        Ok(name) => name,
        Err(_) => return respond(&mut client, 404, "unpublished DuckDNS service").await,
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
        return respond(&mut client, 503, "active workspace is unavailable").await;
    }
    let resolved = match receiver.await {
        Ok(Ok(bytes)) => match decode_reply(&bytes) {
            Ok(DuckDnsReply::Resolved(Some(resolved))) => resolved,
            Ok(DuckDnsReply::Resolved(None)) => {
                return respond(&mut client, 404, "unpublished DuckDNS service").await;
            }
            _ => return respond(&mut client, 503, "active workspace is unavailable").await,
        },
        _ => return respond(&mut client, 503, "active workspace is unavailable").await,
    };

    let prepared =
        match duckdns_client::prepare_request(&initial, client_ip, resolved.allow_cross_site) {
            Ok(request) => request,
            Err(duckdns_client::GatewayError::CrossSite)
            | Err(duckdns_client::GatewayError::WebSocketOrigin) => {
                return respond(&mut client, 403, "browser cross-site request refused").await;
            }
            Err(_) => return respond(&mut client, 400, "malformed HTTP/1.1 request").await,
        };

    // The requesting node must itself have standing. Provider admission also
    // rechecks this remotely, but the local check produces the promised 403.
    if !has_standing(&mut commands, &me).await {
        return respond(&mut client, 403, "workspace membership refused").await;
    }
    if plane.get().is_none() {
        return respond(&mut client, 502, "DuckDNS overlay is unavailable").await;
    }

    let providers = ordered_providers(&resolved, &prepared.hostname);
    let mut upstream = None;
    for provider in providers {
        match duckdns_plane::open(&plane, &provider.node, &resolved.identity).await {
            Ok(stream) => {
                upstream = Some(stream);
                break;
            }
            Err(_) => continue,
        }
    }
    let Some(mut upstream) = upstream else {
        return respond(&mut client, 502, "DuckDNS providers are unreachable").await;
    };
    if upstream.write_all(&prepared.bytes).await.is_err() {
        return respond(
            &mut client,
            502,
            "DuckDNS provider closed before responding",
        )
        .await;
    }
    let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
    Ok(())
}

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

enum Status {
    BadRequest,
    HeadTooLarge,
}

async fn read_initial_request(stream: &mut tokio::net::TcpStream) -> Result<Vec<u8>, Status> {
    let mut bytes = Vec::with_capacity(4096);
    loop {
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            return Ok(bytes);
        }
        if bytes.len() >= duckdns_client::MAX_REQUEST_HEAD {
            return Err(Status::HeadTooLarge);
        }
        let mut chunk = [0u8; 4096];
        let read = stream
            .read(&mut chunk)
            .await
            .map_err(|_| Status::BadRequest)?;
        if read == 0 {
            return Err(Status::BadRequest);
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
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

async fn respond(stream: &mut tokio::net::TcpStream, status: u16, message: &str) -> Result<(), ()> {
    let reason = match status {
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        431 => "Request Header Fields Too Large",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Error",
    };
    let body = format!("{message}\n");
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await.map_err(|_| ())
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
