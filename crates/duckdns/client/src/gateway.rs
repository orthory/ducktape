//! The local HTTPS helper's HTTP/1.1 handoff guard. It parses only the request
//! head needed for routing/security, preserves any already-read body bytes,
//! and scrubs caller-supplied forwarding headers before adding trusted ones.

use std::net::IpAddr;

pub const MAX_REQUEST_HEAD: usize = 64 * 1024;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum GatewayError {
    #[error("HTTP request head exceeds {MAX_REQUEST_HEAD} bytes")]
    HeadTooLarge,
    #[error("malformed HTTP/1.1 request head: {0}")]
    Malformed(&'static str),
    #[error("unsafe cross-site browser request is not allowed")]
    CrossSite,
    #[error("WebSocket Origin must match the requested DuckDNS origin")]
    WebSocketOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedRequest {
    /// Canonical lowercase Host without a port or trailing dot.
    pub hostname: String,
    /// Rewritten request bytes, including any body bytes read with the head.
    pub bytes: Vec<u8>,
    pub websocket: bool,
}

/// Validate one complete request-head buffer (`\r\n\r\n` must be present),
/// apply browser request policy, and add trusted forwarding metadata.
pub fn prepare_request(
    input: &[u8],
    client_ip: IpAddr,
    allow_cross_site: bool,
) -> Result<PreparedRequest, GatewayError> {
    let head_end = match find_head_end(input) {
        Some(end) if end > MAX_REQUEST_HEAD => return Err(GatewayError::HeadTooLarge),
        Some(end) => end,
        None if input.len() > MAX_REQUEST_HEAD => return Err(GatewayError::HeadTooLarge),
        None => return Err(GatewayError::Malformed("incomplete head")),
    };
    let head = std::str::from_utf8(&input[..head_end])
        .map_err(|_| GatewayError::Malformed("head is not ASCII/UTF-8"))?;
    let mut lines = head[..head.len() - 4].split("\r\n");
    let request_line = lines
        .next()
        .ok_or(GatewayError::Malformed("missing request line"))?;
    let mut request_parts = request_line.split_ascii_whitespace();
    let method = request_parts
        .next()
        .ok_or(GatewayError::Malformed("missing method"))?;
    let _target = request_parts
        .next()
        .ok_or(GatewayError::Malformed("missing request target"))?;
    if request_parts.next() != Some("HTTP/1.1") || request_parts.next().is_some() {
        return Err(GatewayError::Malformed("want HTTP/1.1 request line"));
    }

    let mut headers = Vec::<(&str, &str)>::new();
    for line in lines {
        if line.starts_with([' ', '\t']) {
            return Err(GatewayError::Malformed("folded header"));
        }
        let (name, value) = line
            .split_once(':')
            .ok_or(GatewayError::Malformed("header without colon"))?;
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
            return Err(GatewayError::Malformed("invalid header name"));
        }
        if value.bytes().any(|b| b == 0 || b == b'\r' || b == b'\n') {
            return Err(GatewayError::Malformed("invalid header value"));
        }
        headers.push((name, value.trim()));
    }

    let hosts: Vec<_> = headers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("host"))
        .map(|(_, value)| *value)
        .collect();
    if hosts.len() != 1 {
        return Err(GatewayError::Malformed("want exactly one Host"));
    }
    let hostname = canonical_host(hosts[0])?;
    let cross_site = headers.iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("sec-fetch-site") && value.eq_ignore_ascii_case("cross-site")
    });
    let safe = matches!(method, "GET" | "HEAD" | "OPTIONS" | "TRACE");
    if !allow_cross_site && !safe && cross_site {
        return Err(GatewayError::CrossSite);
    }

    let websocket = headers.iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("upgrade") && header_has_token(value, "websocket")
    });
    if websocket {
        let origins: Vec<_> = headers
            .iter()
            .filter(|(name, _)| name.eq_ignore_ascii_case("origin"))
            .map(|(_, value)| *value)
            .collect();
        if origins.len() != 1 || !origin_matches(origins[0], &hostname) {
            return Err(GatewayError::WebSocketOrigin);
        }
    }

    let mut bytes = Vec::with_capacity(input.len() + 192);
    bytes.extend_from_slice(request_line.as_bytes());
    bytes.extend_from_slice(b"\r\n");
    for (name, value) in headers {
        if matches_forwarded(name) {
            continue;
        }
        bytes.extend_from_slice(name.as_bytes());
        bytes.extend_from_slice(b": ");
        bytes.extend_from_slice(value.as_bytes());
        bytes.extend_from_slice(b"\r\n");
    }
    bytes.extend_from_slice(b"X-Forwarded-Proto: https\r\n");
    bytes.extend_from_slice(format!("X-Forwarded-Host: {hostname}\r\n").as_bytes());
    bytes.extend_from_slice(format!("X-Forwarded-For: {client_ip}\r\n").as_bytes());
    bytes.extend_from_slice(
        format!("Forwarded: for={client_ip};proto=https;host=\"{hostname}\"\r\n").as_bytes(),
    );
    bytes.extend_from_slice(b"\r\n");
    bytes.extend_from_slice(&input[head_end..]);
    Ok(PreparedRequest {
        hostname,
        bytes,
        websocket,
    })
}

fn find_head_end(input: &[u8]) -> Option<usize> {
    input
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|offset| offset + 4)
}

fn header_has_token(value: &str, wanted: &str) -> bool {
    value
        .split(',')
        .any(|token| token.trim().eq_ignore_ascii_case(wanted))
}

fn canonical_host(authority: &str) -> Result<String, GatewayError> {
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') => (host, Some(port)),
        _ => (authority, None),
    };
    if port.is_some_and(|port| port.parse::<u16>().is_err()) {
        return Err(GatewayError::Malformed("invalid Host port"));
    }
    let host = host.strip_suffix('.').unwrap_or(host).to_ascii_lowercase();
    if host.is_empty() || !host.is_ascii() {
        return Err(GatewayError::Malformed("invalid Host"));
    }
    Ok(host)
}

fn origin_matches(origin: &str, hostname: &str) -> bool {
    let Some(authority) = origin.strip_prefix("https://") else {
        return false;
    };
    if authority.contains('/') || authority.contains('?') || authority.contains('#') {
        return false;
    }
    let Ok(origin_host) = canonical_host(authority) else {
        return false;
    };
    let port_ok = authority
        .rsplit_once(':')
        .filter(|(host, _)| !host.contains(':'))
        .is_none_or(|(_, port)| port == "443");
    port_ok && origin_host == hostname
}

fn matches_forwarded(name: &str) -> bool {
    name.eq_ignore_ascii_case("forwarded")
        || name
            .get(..12)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("x-forwarded-"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(extra: &str) -> Vec<u8> {
        format!("POST /save HTTP/1.1\r\nHost: BLOG.Orthory.DuckTape.Quack:443\r\n{extra}\r\nbody")
            .into_bytes()
    }

    #[test]
    fn preserves_body_and_scrubs_forwarding_spoof() {
        let prepared = prepare_request(
            &request(
                "X-Forwarded-For: attacker\r\nX-Forwarded-Port: 81\r\nSec-Fetch-Site: same-origin\r\n",
            ),
            "127.0.0.2".parse().unwrap(),
            false,
        )
        .unwrap();
        assert_eq!(prepared.hostname, "blog.orthory.ducktape.quack");
        let text = String::from_utf8(prepared.bytes).unwrap();
        assert!(text.ends_with("\r\n\r\nbody"));
        assert!(!text.contains("attacker"));
        assert!(!text.contains("X-Forwarded-Port"));
        assert!(text.contains("X-Forwarded-For: 127.0.0.2"));
        assert!(text.contains("X-Forwarded-Proto: https"));
    }

    #[test]
    fn cross_site_unsafe_rejects_unless_service_opts_in() {
        let bytes = request("Sec-Fetch-Site: cross-site\r\n");
        assert_eq!(
            prepare_request(&bytes, "127.0.0.1".parse().unwrap(), false),
            Err(GatewayError::CrossSite)
        );
        assert!(prepare_request(&bytes, "127.0.0.1".parse().unwrap(), true).is_ok());

        let get =
            b"GET / HTTP/1.1\r\nHost: orthory.ducktape.quack\r\nSec-Fetch-Site: cross-site\r\n\r\n";
        assert!(prepare_request(get, "127.0.0.1".parse().unwrap(), false).is_ok());
    }

    #[test]
    fn websocket_origin_must_match_exact_https_origin() {
        let matching = b"GET /ws HTTP/1.1\r\nHost: blog.orthory.ducktape.quack\r\nUpgrade: websocket\r\nOrigin: https://blog.orthory.ducktape.quack\r\n\r\n";
        assert!(prepare_request(matching, "127.0.0.1".parse().unwrap(), false).is_ok());
        for origin in [
            "https://evil.ducktape.quack",
            "http://blog.orthory.ducktape.quack",
            "https://blog.orthory.ducktape.quack:444",
        ] {
            let request = format!(
                "GET /ws HTTP/1.1\r\nHost: blog.orthory.ducktape.quack\r\nUpgrade: websocket\r\nOrigin: {origin}\r\n\r\n"
            );
            assert_eq!(
                prepare_request(request.as_bytes(), "127.0.0.1".parse().unwrap(), false),
                Err(GatewayError::WebSocketOrigin)
            );
        }

        let duplicate_origin = b"GET /ws HTTP/1.1\r\nHost: blog.orthory.ducktape.quack\r\nUpgrade: h2c, websocket\r\nOrigin: https://blog.orthory.ducktape.quack\r\nOrigin: https://evil.ducktape.quack\r\n\r\n";
        assert_eq!(
            prepare_request(duplicate_origin, "127.0.0.1".parse().unwrap(), false),
            Err(GatewayError::WebSocketOrigin)
        );
    }

    #[test]
    fn malformed_smuggling_shapes_reject() {
        for bytes in [
            b"GET / HTTP/1.1\r\n\r\n".as_slice(),
            b"GET / HTTP/1.1\r\nHost: a.ducktape.quack\r\nHost: b.ducktape.quack\r\n\r\n",
            b"GET / HTTP/1.0\r\nHost: a.ducktape.quack\r\n\r\n",
            b"GET / HTTP/1.1\r\nHost: a.ducktape.quack\r\n folded\r\n\r\n",
        ] {
            assert!(prepare_request(bytes, "127.0.0.1".parse().unwrap(), false).is_err());
        }
    }
}
