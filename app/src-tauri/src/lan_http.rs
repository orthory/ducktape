//! Shared plumbing for the desktop's EPHEMERAL, session-token-gated LAN HTTP
//! relays (`enroll.rs`, `link_relay.rs`): the LAN-facing address probe, token
//! minting + constant-time comparison, strict body reading, and the tiny_http
//! response builders. Every relay built on this stays a RELAY ONLY — the
//! authority for anything that lands on-chain is the desktop UI approving and
//! signing, never the server itself.

use std::io::{Cursor, Read as _};
use std::net::{IpAddr, UdpSocket};
use std::sync::Arc;

use serde::Deserialize;
use tiny_http::{Header, Request, Response, Server};

/// the LAN-facing IPv4: connect a UDP socket at a public address (no packets
/// are sent) and read back which local interface the OS would route through.
pub fn lan_ipv4() -> Result<IpAddr, String> {
    let sock = UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("udp bind: {e}"))?;
    sock.connect("8.8.8.8:80")
        .map_err(|e| format!("udp connect: {e}"))?;
    Ok(sock
        .local_addr()
        .map_err(|e| format!("local addr: {e}"))?
        .ip())
}

/// a 128-bit hex session token from OS randomness.
pub fn random_token() -> Result<String, String> {
    let mut buf = [0u8; 16];
    getrandom::getrandom(&mut buf).map_err(|err| format!("os randomness: {err}"))?;
    Ok(buf.iter().map(|b| format!("{b:02x}")).collect())
}

/// hex bytes only — reject anything else before it reaches a node verb.
pub fn is_hex(s: &str) -> bool {
    !s.is_empty() && s.len().is_multiple_of(2) && s.bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn token_matches(expected: &str, supplied: &str) -> bool {
    let expected = expected.as_bytes();
    let supplied = supplied.as_bytes();
    let mut different = expected.len() ^ supplied.len();
    for (index, byte) in expected.iter().enumerate() {
        different |= usize::from(*byte ^ supplied.get(index).copied().unwrap_or(0));
    }
    different == 0
}

fn header(name: &[u8], value: &[u8]) -> Header {
    Header::from_bytes(name, value).expect("static response header")
}

pub fn json(body: String) -> Response<Cursor<Vec<u8>>> {
    Response::from_string(body)
        .with_header(header(b"Content-Type", b"application/json"))
        .with_header(header(b"Cache-Control", b"no-store"))
        .with_header(header(b"X-Content-Type-Options", b"nosniff"))
}
pub fn html(body: &str) -> Response<Cursor<Vec<u8>>> {
    Response::from_string(body)
        .with_header(header(b"Content-Type", b"text/html; charset=utf-8"))
        .with_header(header(b"Cache-Control", b"no-store"))
        .with_header(header(b"X-Content-Type-Options", b"nosniff"))
        .with_header(header(
            b"Content-Security-Policy",
            b"default-src 'none'; script-src 'self'; style-src 'unsafe-inline'; connect-src 'self'; base-uri 'none'; form-action 'none'",
        ))
}
pub fn js(body: &str) -> Response<Cursor<Vec<u8>>> {
    Response::from_string(body)
        .with_header(header(b"Content-Type", b"text/javascript; charset=utf-8"))
        .with_header(header(b"Cache-Control", b"no-store"))
        .with_header(header(b"X-Content-Type-Options", b"nosniff"))
}
pub fn status(code: u16) -> Response<Cursor<Vec<u8>>> {
    Response::from_string("").with_status_code(code)
}

pub const MAX_REQUEST_BODY_BYTES: u64 = 8 * 1024;

pub fn read_json<T: for<'de> Deserialize<'de>>(req: &mut Request) -> Option<T> {
    let mut body = String::new();
    req.as_reader()
        .take(MAX_REQUEST_BODY_BYTES + 1)
        .read_to_string(&mut body)
        .ok()?;
    if body.len() as u64 > MAX_REQUEST_BODY_BYTES {
        return None;
    }
    serde_json::from_str(&body).ok()
}

/// drive `server` with `handle` until it is `unblock`ed (cancel/restart).
pub fn serve(server: Arc<Server>, handle: fn(&mut Request) -> Response<Cursor<Vec<u8>>>) {
    for mut req in server.incoming_requests() {
        let resp = handle(&mut req);
        let _ = req.respond(resp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_hex_accepts_only_even_length_hex() {
        assert!(is_hex("00ff"));
        assert!(is_hex("deadbeef"));
        assert!(!is_hex(""));
        assert!(!is_hex("abc")); // odd length
        assert!(!is_hex("zz")); // non-hex
        assert!(!is_hex("00 ff")); // space
    }

    #[test]
    fn token_comparison_is_shape_strict() {
        assert!(token_matches("0123456789abcdef", "0123456789abcdef"));
        assert!(!token_matches("0123456789abcdef", "0123456789abcdee"));
        assert!(!token_matches("0123456789abcdef", "0123456789abcdef00"));
        assert!(!token_matches("0123456789abcdef", "01234567"));
    }
}
