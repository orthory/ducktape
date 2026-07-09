//! OS-side DuckDNS publication and proxy layer.
//!
//! Consensus never sees this crate. It binds replicated [`ServiceIdentity`]
//! values to explicit loopback HTTP targets and byte-proxies an already
//! authenticated overlay stream to exactly that allowlisted target. HTTP
//! parsing, TLS, DNS, and membership remain in their respective outer layers;
//! opaque copying preserves streaming bodies, keep-alive, and WebSocket frames.

mod gateway;
mod proxy;
mod publication;

pub use duckdns_core::*;
pub use gateway::{GatewayError, PreparedHeaders, prepare_headers};
pub use proxy::{ProxyError, proxy_to_publication};
pub use publication::{DuckFsSite, Publication, PublicationTarget, Publications};
