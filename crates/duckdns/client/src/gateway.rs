//! Per-request policy for the node's parsed HTTP/1.1 ingress. Hyper owns wire
//! framing; this layer canonicalizes Host, enforces browser/WebSocket origin
//! rules, and replaces caller-supplied forwarding metadata with trusted values.

use std::net::IpAddr;

use http::header::{HOST, HeaderName, HeaderValue, ORIGIN};
use http::{HeaderMap, Method};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum GatewayError {
    #[error("malformed HTTP/1.1 request head: {0}")]
    Malformed(&'static str),
    #[error("unsafe cross-site browser request is not allowed")]
    CrossSite,
    #[error("WebSocket Origin must match the requested DuckDNS origin")]
    WebSocketOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedHeaders {
    pub hostname: String,
    pub websocket: bool,
}

/// Applying policy to every parsed request prevents a safe first request from
/// laundering later unsafe requests over the same keep-alive TLS connection.
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

    #[test]
    fn websocket_origin_must_match_exact_https_origin() {
        let mut headers = HeaderMap::new();
        headers.insert(HOST, HeaderValue::from_static("blog.orthory.duck"));
        headers.insert(
            HeaderName::from_static("upgrade"),
            HeaderValue::from_static("websocket"),
        );
        headers.insert(
            ORIGIN,
            HeaderValue::from_static("https://blog.orthory.duck"),
        );
        assert!(
            prepare_headers(
                &Method::GET,
                &mut headers.clone(),
                "127.0.0.1".parse().unwrap(),
                false,
            )
            .is_ok()
        );
        for origin in [
            "https://evil.duck",
            "http://blog.orthory.duck",
            "https://blog.orthory.duck:444",
        ] {
            let mut request_headers = headers.clone();
            request_headers.insert(ORIGIN, HeaderValue::from_str(origin).unwrap());
            assert_eq!(
                prepare_headers(
                    &Method::GET,
                    &mut request_headers,
                    "127.0.0.1".parse().unwrap(),
                    false,
                ),
                Err(GatewayError::WebSocketOrigin)
            );
        }

        headers.append(ORIGIN, HeaderValue::from_static("https://evil.duck"));
        assert_eq!(
            prepare_headers(
                &Method::GET,
                &mut headers,
                "127.0.0.1".parse().unwrap(),
                false,
            ),
            Err(GatewayError::WebSocketOrigin)
        );
    }

    #[test]
    fn missing_or_duplicate_host_rejects() {
        let mut headers = HeaderMap::new();
        assert!(
            prepare_headers(
                &Method::GET,
                &mut headers,
                "127.0.0.1".parse().unwrap(),
                false,
            )
            .is_err()
        );
        headers.append(HOST, HeaderValue::from_static("a.duck"));
        headers.append(HOST, HeaderValue::from_static("b.duck"));
        assert!(
            prepare_headers(
                &Method::GET,
                &mut headers,
                "127.0.0.1".parse().unwrap(),
                false,
            )
            .is_err()
        );
    }

    #[test]
    fn structured_keep_alive_policy_rechecks_and_rebuilds_forwarding_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(HOST, HeaderValue::from_static("blog.orthory.duck"));
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
        assert_eq!(prepared.hostname, "blog.orthory.duck");
        assert_eq!(headers["x-forwarded-for"], "::1");
        assert_eq!(
            headers["forwarded"],
            "for=\"[::1]\";proto=https;host=\"blog.orthory.duck\""
        );
    }
}
