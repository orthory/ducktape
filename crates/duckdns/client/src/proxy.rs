use std::io;

use duckdns_core::ServiceIdentity;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;

use crate::Publications;
use crate::publication::validate_target;

#[derive(Debug, thiserror::Error)]
pub enum ProxyError {
    #[error("DuckDNS service is not locally published")]
    Unpublished,
    #[error("DuckDNS target policy: {0}")]
    TargetPolicy(String),
    #[error("connect to published loopback target: {0}")]
    Connect(#[source] io::Error),
    #[error("proxy published HTTP stream: {0}")]
    Proxy(#[source] io::Error),
}

/// Connect only the exact declared identity to its validated loopback target,
/// then copy both directions until EOF. The caller has already authenticated
/// and membership-gated the overlay peer.
pub async fn proxy_to_publication<S>(
    identity: &ServiceIdentity,
    publications: &Publications,
    stream: &mut S,
) -> Result<(u64, u64), ProxyError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let publication = publications.get(identity).ok_or(ProxyError::Unpublished)?;
    // Defense in depth: constructors validate, but never let a future mutation
    // path turn this function into an arbitrary address dialer.
    validate_target(publication.target).map_err(ProxyError::TargetPolicy)?;
    let mut target = TcpStream::connect(publication.target)
        .await
        .map_err(ProxyError::Connect)?;
    tokio::io::copy_bidirectional(stream, &mut target)
        .await
        .map_err(ProxyError::Proxy)
}
