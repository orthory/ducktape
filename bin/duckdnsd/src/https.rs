use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::TlsAcceptor;

use crate::{CaStore, IngressRoute, SharedState};

const LEAF_CACHE_TTL: Duration = Duration::from_secs(12 * 60 * 60);
const MAX_CACHED_LEAVES: usize = 4096;
const MAX_ERROR_REQUEST_HEAD: usize = 64 * 1024;

#[derive(Clone)]
pub struct LeafResolver {
    ca: CaStore,
    cache: Arc<Mutex<BTreeMap<String, CachedLeaf>>>,
}

struct CachedLeaf {
    key: Arc<CertifiedKey>,
    expires_at: Instant,
}

impl fmt::Debug for LeafResolver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LeafResolver")
            .field("ca", &self.ca)
            .finish_non_exhaustive()
    }
}

impl LeafResolver {
    pub fn new(ca: CaStore) -> Self {
        Self {
            ca,
            cache: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub fn certificate_for(&self, hostname: &str) -> Result<Arc<CertifiedKey>, String> {
        let parsed = duckdns_core::parse_hostname(hostname)?;
        let hostname = parsed.hostname();
        let now = Instant::now();
        let mut cache = self.cache.lock().expect("DuckDNS leaf cache lock");
        cache.retain(|_, leaf| leaf.expires_at > now);
        if let Some(cached) = cache.get(&hostname) {
            return Ok(Arc::clone(&cached.key));
        }
        if cache.len() >= MAX_CACHED_LEAVES
            && let Some(oldest) = cache.keys().next().cloned()
        {
            cache.remove(&oldest);
        }
        let key = self.ca.mint(&hostname)?;
        cache.insert(
            hostname,
            CachedLeaf {
                key: Arc::clone(&key),
                expires_at: now + LEAF_CACHE_TTL,
            },
        );
        Ok(key)
    }
}

impl ResolvesServerCert for LeafResolver {
    fn resolve(&self, client_hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        self.certificate_for(client_hello.server_name()?).ok()
    }
}

pub fn tls_config(resolver: LeafResolver) -> Arc<rustls::ServerConfig> {
    // Select the provider explicitly: the wider workspace can enable both
    // rustls providers through unrelated crates, in which case the implicit
    // process-global choice deliberately panics.
    let mut config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::aws_lc_rs::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .expect("aws-lc supports rustls default protocol versions")
    .with_no_client_auth()
    .with_cert_resolver(Arc::new(resolver));
    // Deliberately no h2: the node ingress and publication stream speak HTTP/1.1
    // so keep-alive, streaming bodies, and WebSocket upgrades remain byte-exact.
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Arc::new(config)
}

pub async fn run_https(
    listener: TcpListener,
    config: Arc<rustls::ServerConfig>,
    state: SharedState,
) -> io::Result<()> {
    if !listener.local_addr()?.ip().is_loopback() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "DuckDNS HTTPS listener must be loopback",
        ));
    }
    let acceptor = TlsAcceptor::from(config);
    loop {
        let (stream, _) = listener.accept().await?;
        let acceptor = acceptor.clone();
        let state = state.clone();
        tokio::spawn(async move {
            let _ = handle_https(stream, acceptor, state).await;
        });
    }
}

async fn handle_https(
    stream: TcpStream,
    acceptor: TlsAcceptor,
    state: SharedState,
) -> io::Result<()> {
    let mut tls = match tokio::time::timeout(Duration::from_secs(10), acceptor.accept(stream)).await
    {
        Ok(Ok(tls)) => tls,
        Ok(Err(error)) => return Err(io::Error::other(error)),
        Err(_) => {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "TLS handshake timeout",
            ));
        }
    };
    let hostname = tls.get_ref().1.server_name().unwrap_or_default();
    let ingress = match state.route(hostname) {
        IngressRoute::Published(ingress) => ingress,
        IngressRoute::Unpublished => {
            return write_http_error(&mut tls, 404, "Not Found", "unpublished DuckDNS service")
                .await;
        }
        IngressRoute::Inactive => {
            return write_http_error(
                &mut tls,
                503,
                "Service Unavailable",
                "active workspace unavailable",
            )
            .await;
        }
    };
    let mut upstream =
        match tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(ingress)).await {
            Ok(Ok(stream)) => stream,
            _ => {
                return write_http_error(
                    &mut tls,
                    502,
                    "Bad Gateway",
                    "active node ingress unreachable",
                )
                .await;
            }
        };
    tokio::io::copy_bidirectional(&mut tls, &mut upstream).await?;
    Ok(())
}

async fn write_http_error<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin>(
    stream: &mut S,
    status: u16,
    reason: &str,
    message: &str,
) -> io::Result<()> {
    // If the peer already sent a request, closing with unread TLS application
    // bytes can turn the intended response into a TCP reset. Consume one
    // bounded head before the explicit helper-level 502/503 and close_notify.
    let _ = tokio::time::timeout(Duration::from_secs(5), async {
        let mut bytes = Vec::with_capacity(1024);
        while bytes.len() < MAX_ERROR_REQUEST_HEAD {
            if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
            let mut chunk = [0u8; 1024];
            let read = stream.read(&mut chunk).await?;
            if read == 0 {
                break;
            }
            bytes.extend_from_slice(&chunk[..read]);
        }
        Ok::<(), io::Error>(())
    })
    .await;
    write_error(stream, status, reason, message).await?;
    stream.shutdown().await
}

async fn write_error<S: tokio::io::AsyncWrite + Unpin>(
    stream: &mut S,
    status: u16,
    reason: &str,
    message: &str,
) -> io::Result<()> {
    let body = format!("{message}\n");
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaf_cache_is_sni_scoped() {
        let directory = tempfile::tempdir().unwrap();
        let resolver = LeafResolver::new(CaStore::load_or_create(directory.path()).unwrap());
        let first = resolver
            .certificate_for("docs.team-a1b2c3d4.net.ducktape.quack")
            .unwrap();
        let same = resolver
            .certificate_for("DOCS.TEAM-A1B2C3D4.NET.DUCKTAPE.QUACK.")
            .unwrap();
        let other = resolver
            .certificate_for("status.team-a1b2c3d4.net.ducktape.quack")
            .unwrap();
        assert!(Arc::ptr_eq(&first, &same));
        assert!(!Arc::ptr_eq(&first, &other));
        assert!(resolver.certificate_for("public.example").is_err());
        assert!(
            first.cert[0]
                .as_ref()
                .windows(5)
                .any(|window| window == [0x06, 0x03, 0x55, 0x1d, 0x23]),
            "strict TLS clients require a leaf Authority Key Identifier"
        );
    }
}
