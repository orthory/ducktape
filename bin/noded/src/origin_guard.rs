//! Who is allowed to be a BROWSER principal against the control plane.
//!
//! ## the hole this closes
//!
//! `/v1/submit`, `/v1/query`, `/v1/files/*`, `/v1/fs/*` and `/forge/*` are the
//! node's control plane: they forge consensus ops as this node, read all
//! replicated state, write the filesystem, and push git. They carried
//! `CorsLayer::permissive()` and no origin check, which was safe for exactly one
//! reason — the trusted console was the only web page that ever ran in this
//! process.
//!
//! That premise dies the moment a webview renders content we did not write:
//! gateway content today, arbitrary `https://` pages once the browser opens to
//! the internet. A page cannot be stopped from ATTEMPTING
//! `fetch("http://127.0.0.1:<port>/v1/submit")` — `on_navigation` gates
//! navigation, not `fetch`, and CORS never prevents a request from ARRIVING,
//! only from being read. Permissive CORS additionally handed the response back,
//! so a hostile page could read every byte of state too.
//!
//! ## why an origin allowlist, and not a bearer token
//!
//! A browser sets `Origin` itself and JS cannot forge it. So an allowlist
//! distinguishes exactly the principal we care about — web content — while
//! non-browser clients (the CLI, agents, `git push` to forge) send no `Origin`
//! at all and are untouched. A token would defend additionally against a hostile
//! LOCAL PROCESS, but a local process can already read `user.key` off the disk,
//! so that is not a boundary this file can meaningfully hold.
//!
//! ## the allowlist
//!
//! - `tauri://localhost` — the console under CEF.
//! - `http://localhost:<port>` / `http://127.0.0.1:<port>` — the dev server and
//!   the fleet's per-worktree vite tiles. Any loopback ORIGIN implies a local
//!   server we already trust; hostile content never gets one.
//!
//! Everything else is refused, and two exclusions are the point of the whole
//! file: `null` (a sandboxed/`data:` document) and `<token>.localhost` (gateway
//! content — a DIFFERENT host from `localhost`, so it never matches).

use axum::{
    extract::Request,
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use tower_http::cors::{AllowOrigin, CorsLayer};

/// Extra origins, comma-separated. For an embedder that serves the console from
/// somewhere unusual; not needed for the desktop app or the fleet.
const EXTRA_ORIGINS_ENV: &str = "DUCKTAPE_ALLOWED_ORIGINS";

fn extra_origins() -> Vec<String> {
    std::env::var(EXTRA_ORIGINS_ENV)
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Is `origin` a browser principal we trust with the control plane?
///
/// Compared as the exact RFC 6454 serialization the browser sends. `null` is
/// refused explicitly: it is what a sandboxed iframe or a `data:` document
/// sends, and treating it as "absent" would hand those the control plane.
pub fn origin_allowed(origin: &str) -> bool {
    if origin == "tauri://localhost" {
        return true;
    }
    if let Some(rest) = origin
        .strip_prefix("http://localhost")
        .or_else(|| origin.strip_prefix("http://127.0.0.1"))
    {
        // "" (default port) or ":<port>" — and nothing else, so that
        // "http://localhost.evil.com" cannot pass by prefix.
        return rest.is_empty()
            || (rest.starts_with(':') && rest[1..].chars().all(|c| c.is_ascii_digit()));
    }
    extra_origins().iter().any(|allowed| allowed == origin)
}

/// Refuse any request that arrives with an `Origin` this node does not trust.
///
/// A request with NO `Origin` is allowed through: that is the CLI, an agent,
/// `git`, or a same-origin navigation — none of which is the threat, and all of
/// which would break under a blanket rule.
pub async fn guard(request: Request, next: Next) -> Response {
    if let Some(origin) = request.headers().get(axum::http::header::ORIGIN) {
        let allowed = origin.to_str().map(origin_allowed).unwrap_or(false);
        if !allowed {
            return (
                StatusCode::FORBIDDEN,
                "this origin may not reach the node control plane",
            )
                .into_response();
        }
    }
    next.run(request).await
}

/// CORS for the control plane: the same allowlist, so that even a request we let
/// through (an `Origin`-less GET) can never have its RESPONSE read by a page.
pub fn cors() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(
            |origin: &HeaderValue, _: &axum::http::request::Parts| {
                origin.to_str().map(origin_allowed).unwrap_or(false)
            },
        ))
        .allow_methods(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_console_and_loopback_dev_servers_are_allowed() {
        assert!(origin_allowed("tauri://localhost"));
        assert!(origin_allowed("http://localhost:1430"));
        assert!(origin_allowed("http://127.0.0.1:1430"));
        assert!(origin_allowed("http://localhost"));
        // the fleet's per-worktree vite tiles land on arbitrary loopback ports
        assert!(origin_allowed("http://localhost:1437"));
    }

    /// The two exclusions this file exists for.
    #[test]
    fn gateway_content_and_null_are_refused() {
        // gateway sessions run at `<32hex>.localhost` — a DIFFERENT host than
        // `localhost`, and the reason the check is not a substring match.
        assert!(!origin_allowed(
            "http://0123456789abcdef0123456789abcdef.localhost:49152"
        ));
        // a sandboxed iframe / data: document
        assert!(!origin_allowed("null"));
    }

    #[test]
    fn the_public_web_is_refused() {
        assert!(!origin_allowed("https://evil.com"));
        assert!(!origin_allowed("http://evil.com"));
        // the prefix trap: a hostile host that merely STARTS with our allowlist
        assert!(!origin_allowed("http://localhost.evil.com"));
        assert!(!origin_allowed("http://127.0.0.1.evil.com"));
        // and a duck:// page, once the browser renders those
        assert!(!origin_allowed("duck://site.alice.duck"));
    }

    #[test]
    fn a_port_must_be_numeric() {
        assert!(!origin_allowed("http://localhost:abc"));
        assert!(!origin_allowed("http://localhost:1430x"));
    }
}
