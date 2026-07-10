//! DuckDNS web streams over the authenticated overlay data plane.
//!
//! The data-plane source `/128` authenticates a peer; [`WebPeers`] admits only
//! the current validator/resident set. The service hello carries one canonical
//! replicated [`duckdns::ServiceIdentity`]. On the provider, that identity must
//! exist in the node-local [`duckdns_client::Publications`] allowlist before a
//! loopback connection is made. No payload-supplied address or port exists.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;

use commonware_cryptography::ed25519;
use data_plane::{
    AddressBook, AdmissionPolicy, DataPlane, FlowId, OpenError, OverlaySockets, PeerId,
    PlaneConfig, Service, SocketFactory, StreamPolicy, StreamService,
};
use duckdns::{
    ServiceIdentity, WEB_STREAM_INTENT, decode_service_identity, encode_service_identity,
};
use duckdns_client::{PublicationTarget, Publications};

const BIND_RETRY: Duration = Duration::from_secs(3);
const WEB_PLANE_CONFIG: PlaneConfig = PlaneConfig {
    bulk_bytes_per_sec: 32_000_000,
    bulk_burst_bytes: 512 * 1024,
};
const WEB_FLOW_DOMAIN: &[u8] = b"ducktape-duckdns-web-v1";

fn ula_of(namespace: &str, raw: &[u8; 32]) -> Ipv6Addr {
    wireguard_upgrade::ula_v6_member_addr(namespace, wireguard_upgrade::ValidatorIdentity(*raw))
}

/// Current standing member set plus its overlay address mapping.
pub struct WebPeers {
    namespace: String,
    reverse: RwLock<HashMap<IpAddr, PeerId>>,
}

impl WebPeers {
    pub fn new(namespace: String) -> Arc<Self> {
        Arc::new(Self {
            namespace,
            reverse: RwLock::new(HashMap::new()),
        })
    }

    pub fn set_peers<'a>(&self, keys: impl Iterator<Item = &'a ed25519::PublicKey>) {
        let reverse = keys
            .map(|key| {
                let raw: [u8; 32] = key.as_ref().try_into().expect("ed25519 keys are 32 bytes");
                (IpAddr::V6(ula_of(&self.namespace, &raw)), PeerId(raw))
            })
            .collect();
        *self.reverse.write().expect("DuckDNS peers lock") = reverse;
    }

    fn contains(&self, peer: PeerId) -> bool {
        self.reverse
            .read()
            .expect("DuckDNS peers lock")
            .values()
            .any(|candidate| *candidate == peer)
    }

    fn own_ip(&self, me: &ed25519::PublicKey) -> IpAddr {
        let raw: [u8; 32] = me.as_ref().try_into().expect("ed25519 keys are 32 bytes");
        IpAddr::V6(ula_of(&self.namespace, &raw))
    }
}

impl AddressBook for WebPeers {
    fn datagram_addr(&self, peer: PeerId) -> Option<SocketAddr> {
        Some(SocketAddr::new(
            IpAddr::V6(ula_of(&self.namespace, &peer.0)),
            Service::Web.overlay_datagram_port(),
        ))
    }

    fn stream_addr(&self, peer: PeerId) -> Option<SocketAddr> {
        Some(SocketAddr::new(
            IpAddr::V6(ula_of(&self.namespace, &peer.0)),
            Service::Web.overlay_stream_port(),
        ))
    }

    fn peer_at(&self, source: IpAddr) -> Option<PeerId> {
        self.reverse
            .read()
            .expect("DuckDNS peers lock")
            .get(&source)
            .copied()
    }
}

impl AdmissionPolicy for WebPeers {
    fn permits(&self, peer: PeerId, service: Service, _flow: FlowId) -> bool {
        service == Service::Web && self.contains(peer)
    }
}

pub type PlaneSlot = Arc<OnceLock<Arc<StreamService<OverlaySockets>>>>;

pub fn web_flow(identity: &ServiceIdentity) -> Result<FlowId, String> {
    let meta = encode_service_identity(identity)?;
    let mut preimage = Vec::with_capacity(WEB_FLOW_DOMAIN.len() + meta.len());
    preimage.extend_from_slice(WEB_FLOW_DOMAIN);
    preimage.extend_from_slice(&meta);
    Ok(FlowId::derive(&preimage))
}

/// Bind the service lazily and run its provider accept loop for process life.
pub fn spawn_bring_up(
    label: String,
    peers: Arc<WebPeers>,
    me: ed25519::PublicKey,
    slot: PlaneSlot,
    factory: Arc<dyn SocketFactory>,
    publications: Arc<Publications>,
    files: noded::ActorNodeApi,
) {
    tokio::spawn(async move {
        let own = peers.own_ip(&me);
        let datagram_bind = SocketAddr::new(own, Service::Web.overlay_datagram_port());
        let stream_bind = SocketAddr::new(own, Service::Web.overlay_stream_port());
        let mut attempts = 0u64;
        let sockets = loop {
            attempts += 1;
            match OverlaySockets::bind_with(
                factory.clone(),
                datagram_bind,
                stream_bind,
                peers.clone(),
            )
            .await
            {
                Ok(sockets) => break sockets,
                Err(error) => {
                    if attempts == 1 || attempts.is_multiple_of(10) {
                        eprintln!(
                            "[node {label}] DuckDNS web plane waiting for overlay sockets at \
                             {stream_bind} (attempt {attempts}): {error}"
                        );
                    }
                    tokio::time::sleep(BIND_RETRY).await;
                }
            }
        };
        let admission: Arc<dyn AdmissionPolicy> = peers.clone();
        let plane = DataPlane::new(sockets, admission, WEB_PLANE_CONFIG);
        let service = match plane.stream_service(
            Service::Web,
            StreamPolicy {
                accept_backlog: 128,
            },
        ) {
            Ok(service) => Arc::new(service),
            Err(error) => {
                eprintln!("[node {label}] DuckDNS plane registration failed: {error}");
                return;
            }
        };
        println!(
            "[node {label}] DuckDNS web plane bound on [{own}]:{}",
            Service::Web.overlay_stream_port()
        );
        let _ = slot.set(Arc::clone(&service));
        let _plane = plane;
        loop {
            let Some((peer, hello, stream)) = service.accept().await else {
                return;
            };
            if hello.intent != WEB_STREAM_INTENT || !peers.contains(peer) {
                continue;
            }
            let Ok(identity) = decode_service_identity(&hello.meta) else {
                continue;
            };
            if web_flow(&identity).ok() != Some(hello.flow) {
                continue;
            }
            let Some(publication) = publications.get(&identity).cloned() else {
                continue;
            };
            let publications = Arc::clone(&publications);
            let files = files.clone();
            tokio::spawn(async move {
                let _ =
                    serve_publication(&identity, &publications, files, publication.target, stream)
                        .await;
            });
        }
    });
}

/// Serve one already-authorized local declaration. Kept shared between an
/// authenticated remote plane accept and the requesting node's self-provider
/// fast path (a solo network intentionally has no peer tunnel to dial).
pub async fn serve_publication<S>(
    identity: &ServiceIdentity,
    publications: &Publications,
    files: noded::ActorNodeApi,
    target: PublicationTarget,
    mut stream: S,
) -> Result<(), String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    match target {
        PublicationTarget::Loopback(_) => {
            duckdns_client::proxy_to_publication(identity, publications, &mut stream)
                .await
                .map(|_| ())
                .map_err(|error| error.to_string())
        }
        PublicationTarget::DuckFs(site) => crate::site::serve(stream, files, site).await,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WebOpenError {
    #[error("DuckDNS web plane is unavailable")]
    Unavailable,
    #[error("provider node key must be 32 bytes")]
    InvalidProvider,
    #[error("invalid DuckDNS service identity: {0}")]
    InvalidIdentity(String),
    #[error("open DuckDNS provider stream: {0}")]
    Open(#[from] OpenError),
}

pub async fn open(
    slot: &PlaneSlot,
    provider: &[u8],
    identity: &ServiceIdentity,
) -> Result<data_plane::plane::PacedStream<data_plane::PlaneStream>, WebOpenError> {
    let provider: [u8; 32] = provider
        .try_into()
        .map_err(|_| WebOpenError::InvalidProvider)?;
    let metadata = encode_service_identity(identity).map_err(WebOpenError::InvalidIdentity)?;
    let flow = web_flow(identity).map_err(WebOpenError::InvalidIdentity)?;
    let service = slot.get().ok_or(WebOpenError::Unavailable)?;
    service
        .open(PeerId(provider), flow, WEB_STREAM_INTENT, metadata)
        .await
        .map_err(WebOpenError::Open)
}

#[cfg(test)]
mod tests {
    use super::*;
    use duckdns::ServiceScope;

    fn key(seed: u64) -> ed25519::PublicKey {
        use commonware_cryptography::Signer as _;
        ed25519::PrivateKey::from_seed(seed).public_key()
    }

    #[test]
    fn service_identity_moves_the_flow_id() {
        let docs = ServiceIdentity {
            scope: ServiceScope::Network,
            service: "docs".into(),
        };
        let status = ServiceIdentity {
            scope: ServiceScope::Network,
            service: "status".into(),
        };
        assert_eq!(web_flow(&docs).unwrap(), web_flow(&docs).unwrap());
        assert_ne!(web_flow(&docs).unwrap(), web_flow(&status).unwrap());
    }

    #[test]
    fn web_admission_tracks_only_current_standing_members() {
        let member = key(1);
        let outsider = key(2);
        let peers = WebPeers::new("team-a1b2c3d4".into());
        peers.set_peers([&member].into_iter());
        let member = PeerId(member.as_ref().try_into().unwrap());
        let outsider = PeerId(outsider.as_ref().try_into().unwrap());
        let flow = FlowId::derive(b"duckdns-admission-test");

        assert!(peers.permits(member, Service::Web, flow));
        assert!(!peers.permits(outsider, Service::Web, flow));
        assert!(!peers.permits(member, Service::StateSync, flow));

        peers.set_peers(std::iter::empty());
        assert!(
            !peers.permits(member, Service::Web, flow),
            "standing revocation immediately closes new Web admissions"
        );
    }
}
