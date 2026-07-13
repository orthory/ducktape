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

/// bind an ephemeral relay server on `ip` ONLY — never the wildcard. the QR
/// advertises exactly one interface's address, and a wildcard bind would also
/// accept from every interface it never advertised (overlay, VPN, container
/// bridges); the advertised-interface bind keeps reachability exactly equal to
/// what the QR says.
pub fn lan_server(ip: IpAddr) -> Result<(Arc<Server>, u16), String> {
    let server = Arc::new(Server::http((ip, 0)).map_err(|e| format!("bind: {e}"))?);
    let port = server
        .server_addr()
        .to_ip()
        .ok_or("server has no ip address")?
        .port();
    Ok((server, port))
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

    #[test]
    fn lan_server_binds_the_given_ip_on_an_ephemeral_port() {
        use std::net::{Ipv4Addr, TcpStream};

        let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
        let (server, port) = lan_server(ip).expect("bind loopback");
        assert_ne!(port, 0);
        assert_eq!(server.server_addr().to_ip().expect("ip").ip(), ip);
        TcpStream::connect((ip, port)).expect("bound address accepts");
    }

    /// the posture mule: the relay listens on the advertised interface ONLY.
    /// needs a routable non-loopback interface, so it is opt-in:
    ///   cargo test -p ducktape-desktop lan_server_is_unreachable -- --ignored
    #[test]
    #[ignore = "needs a routable non-loopback interface"]
    fn lan_server_is_unreachable_off_the_advertised_interface() {
        use std::net::TcpStream;

        let ip = lan_ipv4().expect("routable interface");
        assert!(!ip.is_loopback(), "host has no non-loopback route");
        let (_server, port) = lan_server(ip).expect("bind advertised interface");
        // reachable exactly where the QR points…
        TcpStream::connect((ip, port)).expect("advertised address accepts");
        // …and refused on any interface the QR never advertised (loopback
        // stands in for overlay/VPN/bridge addresses).
        assert!(TcpStream::connect(("127.0.0.1", port)).is_err());
    }
}
