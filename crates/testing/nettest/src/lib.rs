//! Raw-HTTP-over-TCP test client + collision-safe port allocation + a coarse
//! event poll — the three helpers every node/daemon/sim integration harness in
//! the tree used to re-roll locally, now in ONE place.
//!
//! The node's app surface trusts localhost callers, so a hand-rolled std-TCP
//! http/1.1 client is a full citizen by design — the daemon's whole promise is
//! that ANY plain http client works. This crate IS that client, deduplicated,
//! so a suite that needs a raw request never re-writes a subtly-different (and
//! racier) variant. Dev/test only: no production crate depends on it.
//!
//! The body/head split is a BYTE-LEVEL scan for the `\r\n\r\n` terminator, never
//! a lossy utf-8 `split("\r\n\r\n")` — the former is correct for a binary chunk
//! body AND for a text body that itself contains a blank line; the latter
//! corrupts both. Port allocation holds every listener at once so the OS can't
//! hand the same port back twice. These are the safe variants; the racy/naive
//! copies are what this crate exists to kill.

use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

use serde_json::Value;

/// how long a request waits on a loopback peer before giving up. generous: a
/// cold genesis or a slow block legitimately takes seconds, and a real hang is
/// caught by the test's own outer deadline, not this socket timeout.
const IO_TIMEOUT: Duration = Duration::from_secs(30);

/// one raw http/1.1 request over std TCP against a loopback app-surface `port`,
/// returning `(status, body_bytes)`. `content_type` sets the request header;
/// `body` is sent verbatim. panics on a connect/head-write/read failure (a
/// broken loopback is a broken test). the BODY write is best-effort: the server
/// may legally answer 413 and stop reading mid-body, surfacing as a broken pipe.
pub fn http_bytes(
    port: u16,
    method: &str,
    path: &str,
    content_type: &str,
    body: &[u8],
) -> (u16, Vec<u8>) {
    try_http_bytes(port, method, path, content_type, body).expect("app-surface request")
}

/// the fallible core of [`http_bytes`]: an `io::Result` so a readiness loop can
/// treat "connection refused" (node not up yet) as "not ready" instead of a
/// panic.
pub fn try_http_bytes(
    port: u16,
    method: &str,
    path: &str,
    content_type: &str,
    body: &[u8],
) -> std::io::Result<(u16, Vec<u8>)> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))?;
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    let head = format!(
        "{method} {path} HTTP/1.1\r\nhost: 127.0.0.1\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    // best-effort: a 413 refusal closes the read side mid-body.
    let _ = stream.write_all(body);
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw)?;
    // split head/body at the BYTE level — chunk bytes round-trip untouched and a
    // text body's own blank line can't be mistaken for the header terminator.
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "no http header terminator")
    })?;
    let status = String::from_utf8_lossy(&raw[..split])
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    Ok((status, raw[split + 4..].to_vec()))
}

/// a json request/response against a loopback app surface: serialize `body` (if
/// any) as the request, parse the response body as json (`Null` if it isn't).
/// the json twin of [`http_bytes`] — the byte-level split means a json value
/// carrying `\r\n\r\n` inside a string can't corrupt the parse.
pub fn http_json(port: u16, method: &str, path: &str, body: Option<&Value>) -> (u16, Value) {
    try_http_json(port, method, path, body).expect("app-surface json request")
}

/// the fallible core of [`http_json`], for a readiness loop that polls a json
/// route before the node is up.
pub fn try_http_json(
    port: u16,
    method: &str,
    path: &str,
    body: Option<&Value>,
) -> std::io::Result<(u16, Value)> {
    let bytes = body
        .map(|b| serde_json::to_vec(b).expect("request body serializes"))
        .unwrap_or_default();
    let (status, raw) = try_http_bytes(port, method, path, "application/json", &bytes)?;
    let payload = serde_json::from_slice(&raw).unwrap_or(Value::Null);
    Ok((status, payload))
}

/// GET (or any verb) a loopback app surface and return `(status, body_as_text)`
/// — the non-json twin, for text expositions like the Prometheus `/metrics`
/// body or a raw status-body assertion.
pub fn http_text(port: u16, method: &str, path: &str) -> (u16, String) {
    let (status, raw) = http_bytes(port, method, path, "text/plain", &[]);
    (status, String::from_utf8_lossy(&raw).into_owned())
}

/// the http status a loopback app surface answers `method path` with, or `None`
/// if it is not up yet (connect refused) — the readiness/liveness probe every
/// harness polls before driving, and posts its shutdown through.
pub fn http_status(port: u16, method: &str, path: &str) -> Option<u16> {
    try_http_bytes(port, method, path, "application/json", &[])
        .ok()
        .map(|(status, _)| status)
}

/// one free localhost port. fine for a single port; for N distinct ports at
/// once use [`alloc_ports`], which holds every listener so the OS can't hand the
/// same port back twice.
pub fn free_port() -> u16 {
    alloc_ports(1)[0]
}

/// `n` DISTINCT free localhost ports, allocated by holding every listener open
/// AT ONCE — a sequential bind-drop loop can (and on a busy box does) hand the
/// same port back twice, wedging two nodes onto one port.
pub fn alloc_ports(n: usize) -> Vec<u16> {
    let listeners: Vec<TcpListener> = (0..n)
        .map(|_| TcpListener::bind("127.0.0.1:0").expect("bind port-0 probe"))
        .collect();
    listeners
        .iter()
        .map(|l| l.local_addr().expect("probe addr").port())
        .collect()
}

/// poll `probe` every 300ms until it returns `Some`, or panic with `what` past
/// `timeout`. the standard "submitted — now wait for it to finalize and become
/// readable" shape for driving an EXTERNAL process's committed state over http.
/// NOT for in-process synchronization — synchronize those on the system's own
/// events (a channel message, a drained frame), never a coarse wall-clock poll.
pub fn poll_until<T>(what: &str, timeout: Duration, mut probe: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(v) = probe() {
            return v;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(300));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead as _, BufReader};

    /// a one-shot loopback server that drains one request head then replies with
    /// `resp` and closes — enough to exercise the client without a real node.
    fn serve_once(resp: &'static [u8]) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(sock.try_clone().unwrap());
            let mut line = String::new();
            loop {
                line.clear();
                let n = reader.read_line(&mut line).unwrap();
                if n == 0 || line == "\r\n" {
                    break;
                }
            }
            sock.write_all(resp).unwrap();
        });
        port
    }

    #[test]
    fn http_bytes_splits_on_header_terminator_not_a_body_blank_line() {
        // the body itself carries a blank line: a naive `split("\r\n\r\n")` would
        // truncate at it; the byte-scan must return the WHOLE body.
        let port = serve_once(
            b"HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\nconnection: close\r\n\r\nline1\r\n\r\nline2",
        );
        let (status, body) = http_bytes(port, "GET", "/", "text/plain", &[]);
        assert_eq!(status, 200);
        assert_eq!(body, b"line1\r\n\r\nline2");
    }

    #[test]
    fn alloc_ports_are_distinct() {
        let ports = alloc_ports(8);
        let mut deduped = ports.clone();
        deduped.sort_unstable();
        deduped.dedup();
        assert_eq!(deduped.len(), ports.len(), "alloc_ports handed back a duplicate");
    }
}
