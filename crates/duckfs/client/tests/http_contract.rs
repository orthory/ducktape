//! contract tests for `HttpNode` against a hand-rolled `std::net::TcpListener`
//! stub — the `daemon_e2e.rs` raw-http house style inverted (we are the server).
//!
//! this pins the exact request lines/bodies the engine sends and the reply
//! shapes it parses, WITHOUT a daemon: a stage POSTs raw bytes and reads
//! `{digest}`, a commit POSTs the snake_case body and reads the CAMELCASE
//! `BlockSummary`, and a module rejection arriving as a 400 `{"error": ...}`
//! surfaces as `ApiError::Rejected` with the string VERBATIM (the conflict
//! taxonomy depends on it). the real daemon round-trip lives in
//! `bin/noded/tests/daemon_e2e.rs`.

use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use duckfs_client::api::{ApiError, NodeApi};
use duckfs_client::http::HttpNode;
use duckfs_core::{Change, Content};

/// one captured request: the method, the path (with query), and the raw body.
#[derive(Debug, Clone)]
struct Recorded {
    method: String,
    path: String,
    body: Vec<u8>,
}

/// a canned http server that records every request and answers each with a
/// responder-chosen `(status, json)` keyed on the path. it always closes the
/// connection (one request per socket) so reqwest opens a fresh one each call —
/// the simplest thing a hand-rolled server can promise.
struct Stub {
    addr: String,
    requests: Arc<Mutex<Vec<Recorded>>>,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Stub {
    fn new<F>(responder: F) -> Self
    where
        F: Fn(&str, &str, &[u8]) -> (u16, serde_json::Value) + Send + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind stub");
        let addr = listener.local_addr().expect("stub addr").to_string();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let shutdown = Arc::new(AtomicBool::new(false));
        let reqs = requests.clone();
        let stop = shutdown.clone();
        let handle = std::thread::spawn(move || {
            for stream in listener.incoming() {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                let mut stream = match stream {
                    Ok(s) => s,
                    Err(_) => break,
                };
                let recorded = read_request(&mut stream);
                reqs.lock().unwrap().push(recorded.clone());
                let (status, body) = responder(&recorded.method, &recorded.path, &recorded.body);
                write_response(&mut stream, status, &body);
            }
        });
        Stub {
            addr,
            requests,
            shutdown,
            handle: Some(handle),
        }
    }

    fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    fn requests(&self) -> Vec<Recorded> {
        self.requests.lock().unwrap().clone()
    }
}

impl Drop for Stub {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        // nudge the blocking accept so the loop notices the shutdown flag.
        let _ = TcpStream::connect(&self.addr);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

/// read one http/1.1 request: the request line, headers to the blank line, then
/// exactly `content-length` body bytes.
fn read_request(stream: &mut TcpStream) -> Recorded {
    let mut reader = BufReader::new(stream.try_clone().expect("clone stub stream"));
    let mut request_line = String::new();
    reader.read_line(&mut request_line).expect("request line");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or_default().to_string();

    let mut content_length = 0usize;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).expect("header line");
        if header == "\r\n" || header.is_empty() {
            break;
        }
        if let Some(v) = header.to_ascii_lowercase().strip_prefix("content-length:") {
            content_length = v.trim().parse().unwrap_or(0);
        }
    }
    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body).expect("request body");
    }
    Recorded { method, path, body }
}

/// write a canned http/1.1 response with an explicit content-length and a close.
fn write_response(stream: &mut TcpStream, status: u16, body: &serde_json::Value) {
    let payload = serde_json::to_vec(body).expect("response body serializes");
    let head = format!(
        "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        payload.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&payload);
    let _ = stream.flush();
}

#[test]
fn stage_posts_raw_bytes_and_parses_the_digest() {
    let stub =
        Stub::new(|_method, _path, _body| (200, serde_json::json!({ "digest": "de".repeat(32) })));
    let node = HttpNode::new(stub.url());

    let digest = node.stage_chunk(b"abc").expect("stage ok");
    assert_eq!(digest, "de".repeat(32));

    let reqs = stub.requests();
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].method, "POST");
    assert_eq!(reqs[0].path, "/v1/files/stage");
    // the raw chunk rides as the body VERBATIM (no json envelope, no base64).
    assert_eq!(reqs[0].body, b"abc");
}

#[test]
fn commit_posts_snake_case_and_parses_camelcase_block() {
    let stub = Stub::new(|_method, _path, _body| {
        // the daemon answers a commit with a camelCase BlockSummary.
        (
            200,
            serde_json::json!({ "height": 5, "root_hash": "ab".repeat(32) }),
        )
    });
    let node = HttpNode::new(stub.url());

    let receipt = node
        .commit(
            Some("basesnap"),
            "hello",
            vec![Change::Put {
                path: "/shared/x".into(),
                exec: false,
                meta: Default::default(),
                content: Content::Inline {
                    b64: STANDARD.encode(b"hi"),
                },
            }],
        )
        .expect("commit ok");
    assert_eq!(receipt.height, 5);

    let reqs = stub.requests();
    assert_eq!(reqs[0].method, "POST");
    assert_eq!(reqs[0].path, "/v1/files/commit");
    // the request body is the snake_case CommitBody the module wire speaks.
    let sent: serde_json::Value = serde_json::from_slice(&reqs[0].body).expect("commit body json");
    assert_eq!(sent["base_snapshot"], "basesnap");
    assert_eq!(sent["message"], "hello");
    assert_eq!(sent["changes"][0]["put"]["path"], "/shared/x");
}

#[test]
fn unpin_deletes_the_percent_encoded_pin_path() {
    let stub = Stub::new(|_method, _path, _body| (200, serde_json::json!({ "height": 9 })));
    let node = HttpNode::new(stub.url());

    // a name with a byte the path can't carry raw ('/') round-trips through
    // percent-encoding rather than becoming an extra path segment.
    node.unpin("a/b").expect("unpin ok");

    let reqs = stub.requests();
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].method, "DELETE");
    assert_eq!(reqs[0].path, "/v1/files/pin/a%2Fb");
    assert!(reqs[0].body.is_empty(), "DELETE carries no body");
}

#[test]
fn a_400_error_envelope_surfaces_as_rejected_verbatim() {
    let stub = Stub::new(|_method, _path, _body| {
        (
            400,
            serde_json::json!({ "error": "files: conflict: /x changed since base" }),
        )
    });
    let node = HttpNode::new(stub.url());

    let err = node
        .commit(None, "m", Vec::new())
        .expect_err("a 400 must reject");
    // the conflict string passes through UNTOUCHED — the taxonomy keys on it.
    assert_eq!(
        err,
        ApiError::Rejected("files: conflict: /x changed since base".into())
    );
}

#[test]
fn stat_maps_404_to_ok_none() {
    let stub = Stub::new(|_method, _path, _body| (404, serde_json::json!({ "error": "no entry" })));
    let node = HttpNode::new(stub.url());

    // a 404 on stat is "nothing there", NOT a transport failure.
    let got = node.stat("/shared/missing", None).expect("stat ok");
    assert!(got.is_none());
}

#[test]
fn refs_read_and_has_chunks_parse_their_replies() {
    let stub = Stub::new(|_method, path, _body| {
        if path.starts_with("/v1/files/refs") {
            (
                200,
                serde_json::json!({ "head": "cd".repeat(32), "pins": {}, "window_len": 3 }),
            )
        } else if path.starts_with("/v1/files/read") {
            (
                200,
                serde_json::json!({ "b64": STANDARD.encode(b"hello"), "eof": true }),
            )
        } else if path.starts_with("/v1/files/has-chunks") {
            (200, serde_json::json!({ "present": [true, false] }))
        } else {
            (500, serde_json::json!({ "error": "unexpected" }))
        }
    });
    let node = HttpNode::new(stub.url());

    let refs = node.refs().expect("refs ok");
    assert_eq!(refs.head.as_deref(), Some(&*"cd".repeat(32)));
    assert_eq!(refs.window_len, 3);

    let (bytes, eof) = node.read("/shared/x", None, 0, 1024).expect("read ok");
    assert_eq!(bytes, b"hello");
    assert!(eof);

    let present = node
        .has_chunks(&["aa".repeat(32), "bb".repeat(32)])
        .expect("has_chunks ok");
    assert_eq!(present, vec![true, false]);

    // the has-chunks ids ride as a comma-joined query param (percent-decoded
    // server-side back to the request order).
    let hc = stub
        .requests()
        .into_iter()
        .find(|r| r.path.starts_with("/v1/files/has-chunks"))
        .expect("a has-chunks request");
    assert!(
        hc.path.contains("ids="),
        "ids ride in the query: {}",
        hc.path
    );
}
