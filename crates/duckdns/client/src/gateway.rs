//! The local HTTPS helper's HTTP/1.1 handoff guard. It parses only the request
//! head needed for routing/security, preserves any already-read body bytes,
//! and scrubs caller-supplied forwarding headers before adding trusted ones.

use std::net::IpAddr;

use http::header::{HOST, HeaderName, HeaderValue, ORIGIN};
use http::{HeaderMap, Method};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedHeaders {
    pub hostname: String,
    pub websocket: bool,
}

/// Structured twin of [`prepare_request`] for the node's keep-alive HTTP
/// gateway. Applying the policy per parsed request prevents a safe first
/// request from laundering later unsafe requests over the same TLS connection.
pub fn prepare_headers(
    method: &Method,
    headers: &mut HeaderMap,
    client_ip: IpAddr,
    allow_cross_site: bool,
) -> Result<PreparedHeaders, GatewayError> {
    let hosts: Vec<_> = headers.get_all(HOST).iter().collect();
    if hosts.len() != 1 {
        return Err(GatewayError::Malformed("want exactly one Host"));
    }
    let hostname = canonical_host(
        hosts[0]
            .to_str()
            .map_err(|_| GatewayError::Malformed("Host is not ASCII"))?,
    )?;
    let cross_site = header_values(headers, "sec-fetch-site")
        .any(|value| value.eq_ignore_ascii_case("cross-site"));
    let safe = matches!(
        *method,
        Method::GET | Method::HEAD | Method::OPTIONS | Method::TRACE
    );
    if !allow_cross_site && !safe && cross_site {
        return Err(GatewayError::CrossSite);
    }

    let websocket =
        header_values(headers, "upgrade").any(|value| header_has_token(value, "websocket"));
    if websocket {
        let origins: Vec<_> = headers
            .get_all(ORIGIN)
            .iter()
            .map(|value| value.to_str())
            .collect::<Result<_, _>>()
            .map_err(|_| GatewayError::WebSocketOrigin)?;
        if origins.len() != 1 || !origin_matches(origins[0], &hostname) {
            return Err(GatewayError::WebSocketOrigin);
        }
    }

    let untrusted: Vec<HeaderName> = headers
        .keys()
        .filter(|name| matches_forwarded(name.as_str()))
        .cloned()
        .collect();
    for name in untrusted {
        headers.remove(name);
    }
    insert_header(headers, "x-forwarded-proto", "https")?;
    insert_header(headers, "x-forwarded-host", &hostname)?;
    insert_header(headers, "x-forwarded-for", &client_ip.to_string())?;
    let forwarded_for = match client_ip {
        IpAddr::V4(address) => address.to_string(),
        IpAddr::V6(address) => format!("\"[{address}]\""),
    };
    insert_header(
        headers,
        "forwarded",
        &format!("for={forwarded_for};proto=https;host=\"{hostname}\""),
    )?;
    Ok(PreparedHeaders {
        hostname,
        websocket,
    })
}

fn header_values<'a>(headers: &'a HeaderMap, name: &'static str) -> impl Iterator<Item = &'a str> {
    headers
        .get_all(name)
        .iter()
        .filter_map(|value| value.to_str().ok())
}

fn insert_header(
    headers: &mut HeaderMap,
    name: &'static str,
    value: &str,
) -> Result<(), GatewayError> {
    let value = HeaderValue::from_str(value)
        .map_err(|_| GatewayError::Malformed("forwarding metadata is invalid"))?;
    headers.insert(HeaderName::from_static(name), value);
    Ok(())
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

    #[test]
    fn structured_keep_alive_policy_rechecks_and_rebuilds_forwarding_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HOST,
            HeaderValue::from_static("blog.orthory.ducktape.quack"),
        );
        headers.insert(
            HeaderName::from_static("sec-fetch-site"),
            HeaderValue::from_static("cross-site"),
        );
        headers.insert(
            HeaderName::from_static("x-forwarded-for"),
            HeaderValue::from_static("attacker"),
        );
        assert_eq!(
            prepare_headers(
                &Method::POST,
                &mut headers.clone(),
                "127.0.0.2".parse().unwrap(),
                false,
            ),
            Err(GatewayError::CrossSite)
        );

        headers.insert(
            HeaderName::from_static("sec-fetch-site"),
            HeaderValue::from_static("same-origin"),
        );
        let prepared =
            prepare_headers(&Method::POST, &mut headers, "::1".parse().unwrap(), false).unwrap();
        assert_eq!(prepared.hostname, "blog.orthory.ducktape.quack");
        assert_eq!(headers["x-forwarded-for"], "::1");
        assert_eq!(
            headers["forwarded"],
            "for=\"[::1]\";proto=https;host=\"blog.orthory.ducktape.quack\""
        );
    }
}
