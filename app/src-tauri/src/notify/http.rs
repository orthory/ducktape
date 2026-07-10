//! Localhost node query helper: a hand-rolled HTTP/1.1 POST over
//! `std::net::TcpStream` (deliberately no HTTP crate) plus the notifier's
//! reply-root author lookup. The node is always localhost, so blocking I/O
//! with short timeouts is the whole story: any failure is a `None`/`Err`,
//! never a panic and never a stall beyond the 2s socket timeouts.

use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

use serde_json::{Value, json};

/// Connect/read/write timeout for every socket operation against the node.
const TIMEOUT: Duration = Duration::from_secs(2);

/// POST `body` as application/json to `{base_url}{path}`; return the parsed
/// JSON body on a 200. `base_url` is an http origin like
/// "http://127.0.0.1:8844" (a trailing slash is tolerated). Blocking
/// `std::net::TcpStream` with 2s connect/read/write timeouts; honors
/// Content-Length on the response (read-to-end when absent); HTTP/1.1 with
/// `Connection: close`. Chunked responses are rejected rather than mis-parsed.
/// Any failure -> Err(String). Never panics.
pub fn post_json(base_url: &str, path: &str, body: &Value) -> Result<Value, String> {
    let authority = authority_of(base_url)?;
    let addr = resolve(&authority)?;
    let path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };

    let payload = serde_json::to_vec(body).map_err(|err| format!("encode request: {err}"))?;
    let mut stream = TcpStream::connect_timeout(&addr, TIMEOUT)
        .map_err(|err| format!("connect {authority}: {err}"))?;
    stream
        .set_read_timeout(Some(TIMEOUT))
        .map_err(|err| format!("set read timeout: {err}"))?;
    stream
        .set_write_timeout(Some(TIMEOUT))
        .map_err(|err| format!("set write timeout: {err}"))?;

    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {authority}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        payload.len()
    );
    stream
        .write_all(request.as_bytes())
        .and_then(|()| stream.write_all(&payload))
        .map_err(|err| format!("write request: {err}"))?;

    let (status, body) = read_response(&mut stream)?;
    if status != 200 {
        return Err(format!("http status {status}"));
    }
    serde_json::from_slice(&body).map_err(|err| format!("decode response body: {err}"))
}

/// The matcher's `root_author` impl: look up who authored `(channel_id,
/// root_seq)` via the chat module's `messages_range` query on the local node.
/// Returns `Some(lowercase hex of the user key bytes)` ONLY for a
/// `{"user":[..]}` author whose message seq matches `root_seq`; `None` on any
/// failure, non-user author, empty page, or a first message at a different
/// seq (a cleared/missing root must not misattribute the thread). Fast-fails:
/// never blocks a caller beyond the 2s socket timeouts.
pub fn root_author(base_url: &str, channel_id: &str, root_seq: u64) -> Option<String> {
    let query = json!({
        "target": "chat",
        "query": {
            "messages_range": {
                "channel_id": channel_id,
                "from_seq": root_seq,
                "limit": 1
            }
        }
    });
    let reply = post_json(base_url, "/v1/query", &query).ok()?;

    let message = reply.get("messages")?.as_array()?.first()?;
    if message.get("seq")?.as_u64()? != root_seq {
        return None;
    }
    let user = message.get("head")?.get("author")?.get("user")?;
    super::decode::bytes_hex(user)
}

/// "http://127.0.0.1:8844" (optionally with a trailing slash) -> "127.0.0.1:8844".
fn authority_of(base_url: &str) -> Result<String, String> {
    let rest = base_url
        .strip_prefix("http://")
        .ok_or_else(|| format!("base url is not an http:// origin: {base_url}"))?;
    let authority = rest.trim_end_matches('/');
    if authority.is_empty() || authority.contains('/') {
        return Err(format!("base url is not a bare http origin: {base_url}"));
    }
    Ok(authority.to_string())
}

fn resolve(authority: &str) -> Result<SocketAddr, String> {
    // "host:port" resolves directly; a bare host falls back to port 80.
    authority
        .to_socket_addrs()
        .or_else(|_| (authority, 80u16).to_socket_addrs())
        .map_err(|err| format!("resolve {authority}: {err}"))?
        .next()
        .ok_or_else(|| format!("no address for {authority}"))
}

/// Read one HTTP/1.1 response: status line, headers to the blank line, then
/// exactly Content-Length body bytes (read-to-end when the header is absent
/// or the server closes early). Chunked transfer coding is rejected.
fn read_response(stream: &mut TcpStream) -> Result<(u16, Vec<u8>), String> {
    let mut buf = Vec::new();

    let header_end = loop {
        if let Some(pos) = find_double_crlf(&buf) {
            break pos;
        }
        if !read_more(stream, &mut buf)? {
            return Err("connection closed before response headers ended".to_string());
        }
    };

    let head = std::str::from_utf8(&buf[..header_end])
        .map_err(|_| "response headers are not utf-8".to_string())?;
    let mut lines = head.lines();
    let status_line = lines.next().unwrap_or_default();
    let status = parse_status(status_line)?;

    let mut content_length = None;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim();
        if name == "transfer-encoding" && value.to_ascii_lowercase().contains("chunked") {
            return Err("chunked responses are not supported".to_string());
        }
        if name == "content-length" {
            content_length = Some(
                value
                    .parse::<usize>()
                    .map_err(|_| format!("bad content-length: {value}"))?,
            );
        }
    }

    let body_start = header_end + 4;
    match content_length {
        Some(len) => {
            // saturating: an absurd content-length must not overflow-panic —
            // we then just read to EOF/timeout like the header-missing case.
            let want = body_start.saturating_add(len);
            while buf.len() < want && read_more(stream, &mut buf)? {}
            let end = buf.len().min(want);
            Ok((status, buf[body_start..end].to_vec()))
        }
        None => {
            while read_more(stream, &mut buf)? {}
            Ok((status, buf[body_start..].to_vec()))
        }
    }
}

/// One `read` into `buf`: Ok(true) = got bytes, Ok(false) = clean EOF.
/// Timeouts and other socket errors are Err (the caller fast-fails).
fn read_more(stream: &mut TcpStream, buf: &mut Vec<u8>) -> Result<bool, String> {
    let mut chunk = [0u8; 4096];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => return Ok(false),
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                return Ok(true);
            }
            Err(err) if err.kind() == ErrorKind::Interrupted => continue,
            Err(err) => return Err(format!("read response: {err}")),
        }
    }
}

/// "HTTP/1.1 200 OK" -> 200.
fn parse_status(status_line: &str) -> Result<u16, String> {
    let mut parts = status_line.split_whitespace();
    let version = parts.next().unwrap_or_default();
    if !version.starts_with("HTTP/1.") {
        return Err(format!("not an http/1.x response: {status_line}"));
    }
    parts
        .next()
        .and_then(|code| code.parse().ok())
        .ok_or_else(|| format!("bad status line: {status_line}"))
}

fn find_double_crlf(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc::{Receiver, channel};
    use std::time::Instant;

    use serde_json::json;

    use super::*;

    fn request_content_length(head: &str) -> usize {
        head.lines()
            .filter_map(|line| line.split_once(':'))
            .find(|(name, _)| name.trim().eq_ignore_ascii_case("content-length"))
            .and_then(|(_, value)| value.trim().parse().ok())
            .unwrap_or(0)
    }

    /// One-shot HTTP stub: accept a single connection, read the request head
    /// plus its Content-Length body (so the client never deadlocks on write),
    /// send the request head to the test, then write `response` and close.
    fn serve_once(response: String) -> (String, Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind stub listener");
        let port = listener.local_addr().expect("stub addr").port();
        let (tx, rx) = channel();

        std::thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut buf = Vec::new();
            let mut chunk = [0u8; 1024];
            let header_end = loop {
                if let Some(pos) = find_double_crlf(&buf) {
                    break pos;
                }
                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break buf.len(),
                    Ok(n) => buf.extend_from_slice(&chunk[..n]),
                }
            };

            let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
            let need = request_content_length(&head);
            let mut got = buf.len().saturating_sub(header_end + 4);
            while got < need {
                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => got += n,
                }
            }

            let _ = tx.send(head);
            let _ = stream.write_all(response.as_bytes());
        });

        (format!("http://127.0.0.1:{port}"), rx)
    }

    fn http_200(body: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    fn messages_reply(seq: u64, author: Value) -> String {
        json!({
            "messages": [{
                "channel_id": "general",
                "seq": seq,
                "head": {
                    "message_id": "m1",
                    "author": author,
                    "blocks": [],
                    "created_at": 1720000000u64,
                    "rev": 0,
                    "edited_at": null,
                    "base_rev": null,
                    "deleted": false,
                    "thread": null,
                    "reply_count": 1,
                    "last_reply_seq": 9
                },
                "reactions": [],
                "channel_head_seq": 9
            }]
        })
        .to_string()
    }

    #[test]
    fn root_author_returns_user_hex_for_matching_root() {
        let (base_url, requests) =
            serve_once(http_200(&messages_reply(7, json!({"user": [18, 52]}))));

        assert_eq!(
            root_author(&base_url, "general", 7),
            Some("1234".to_string())
        );

        let head = requests.recv().expect("stub saw a request");
        assert!(
            head.starts_with("POST /v1/query HTTP/1.1\r\n"),
            "unexpected request line in: {head}"
        );
    }

    #[test]
    fn root_author_rejects_agent_author() {
        let (base_url, _requests) = serve_once(http_200(&messages_reply(
            7,
            json!({"agent": {"module": "runs", "agent_id": "x"}}),
        )));

        assert_eq!(root_author(&base_url, "general", 7), None);
    }

    #[test]
    fn root_author_rejects_system_author() {
        let (base_url, _requests) = serve_once(http_200(&messages_reply(7, json!("system"))));

        assert_eq!(root_author(&base_url, "general", 7), None);
    }

    #[test]
    fn root_author_rejects_seq_mismatch() {
        // A cleared/missing root: the range query answers with the NEXT
        // message (seq 8). Its author must not be misattributed to root 7.
        let (base_url, _requests) =
            serve_once(http_200(&messages_reply(8, json!({"user": [18, 52]}))));

        assert_eq!(root_author(&base_url, "general", 7), None);
    }

    #[test]
    fn root_author_rejects_empty_page() {
        let (base_url, _requests) = serve_once(http_200(&json!({"messages": []}).to_string()));

        assert_eq!(root_author(&base_url, "general", 7), None);
    }

    #[test]
    fn root_author_rejects_non_200() {
        let body = r#"{"error":"nope"}"#;
        let (base_url, _requests) = serve_once(format!(
            "HTTP/1.1 400 Bad Request\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ));

        assert_eq!(root_author(&base_url, "general", 7), None);
    }

    #[test]
    fn root_author_fails_fast_on_connection_refused() {
        // Bind then immediately drop: the port was just free, so the dial is
        // refused (not filtered), which must fail well under the 2s timeout.
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        drop(listener);

        let started = Instant::now();
        assert_eq!(
            root_author(&format!("http://127.0.0.1:{port}"), "general", 7),
            None
        );
        assert!(
            started.elapsed().as_secs() < 3,
            "refused dial took {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn post_json_tolerates_trailing_slash_base_url() {
        let (base_url, requests) = serve_once(http_200(r#"{"ok":true}"#));

        let reply = post_json(
            &format!("{base_url}/"),
            "/v1/query",
            &json!({"target": "chat"}),
        )
        .expect("trailing-slash base url works");
        assert_eq!(reply, json!({"ok": true}));

        let head = requests.recv().expect("stub saw a request");
        assert!(
            head.starts_with("POST /v1/query HTTP/1.1\r\n"),
            "trailing slash must not double the path: {head}"
        );
    }

    #[test]
    fn post_json_rejects_chunked_responses() {
        let (base_url, _requests) = serve_once(
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n2\r\n{}\r\n0\r\n\r\n"
                .to_string(),
        );

        let err = post_json(&base_url, "/v1/query", &json!({})).expect_err("chunked is rejected");
        assert!(err.contains("chunked"), "error names chunked: {err}");
    }

    #[test]
    fn post_json_rejects_non_http_base_url() {
        assert!(post_json("https://127.0.0.1:1", "/v1/query", &json!({})).is_err());
        assert!(post_json("127.0.0.1:1", "/v1/query", &json!({})).is_err());
    }
}
