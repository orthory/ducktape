//! the S3-shaped object facade, end to end against the real router + files
//! module: PUT buffers → stages → commits, GET pages the byte-range Read back
//! to raw bytes, DELETE is an idempotent single-change rm. one url = one
//! object; the listing story stays /v1/files/ls.

#[path = "fs_support/mod.rs"]
mod support;

use std::io::{Read as _, Write as _};
use std::net::TcpStream;

use support::Harness;

/// minimal raw http/1.1 exchange with a body — returns (status, body).
fn http(port: u16, method: &str, path: &str, body: &[u8]) -> (u16, Vec<u8>) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect harness");
    let head = format!(
        "{method} {path} HTTP/1.1\r\nhost: 127.0.0.1\r\ncontent-type: application/octet-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).expect("write head");
    stream.write_all(body).expect("write body");
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).expect("read response");
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("header/body split");
    let head_text = String::from_utf8_lossy(&raw[..split]);
    let status: u16 = head_text
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .expect("status line");
    (status, raw[split + 4..].to_vec())
}

fn port_of(h: &Harness) -> u16 {
    h.node_url()
        .rsplit(':')
        .next()
        .and_then(|p| p.parse().ok())
        .expect("harness port")
}

/// a distinctive, non-uniform byte pattern (251 is prime → catches
/// truncation and chunk-order corruption at any power-of-two boundary).
fn pattern(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

#[test]
fn object_facade_round_trips_small_and_multi_chunk_objects() {
    let h = Harness::start();
    let port = port_of(&h);

    // small object: inline commit path.
    let (status, body) = http(port, "PUT", "/v1/files/object/shared/cat.jpg", b"meow bytes");
    assert_eq!(status, 200, "small put: {}", String::from_utf8_lossy(&body));

    let (status, body) = http(port, "GET", "/v1/files/object/shared/cat.jpg", b"");
    assert_eq!(status, 200);
    assert_eq!(body, b"meow bytes", "small object reads back byte-identical");

    // multi-chunk object: staged path (> 1 MiB inline cap ⇒ 3 chunks).
    let big = pattern(2 * 1024 * 1024 + 17);
    let (status, body) = http(port, "PUT", "/v1/files/object/shared/big.bin", &big);
    assert_eq!(status, 200, "big put: {}", String::from_utf8_lossy(&body));

    let (status, body) = http(port, "GET", "/v1/files/object/shared/big.bin", b"");
    assert_eq!(status, 200);
    assert_eq!(body.len(), big.len(), "length survives the chunk round-trip");
    assert_eq!(body, big, "multi-chunk object reads back byte-identical");

    // overwrite is last-writer-wins, like S3.
    let (status, _) = http(port, "PUT", "/v1/files/object/shared/cat.jpg", b"new cat");
    assert_eq!(status, 200);
    let (_, body) = http(port, "GET", "/v1/files/object/shared/cat.jpg", b"");
    assert_eq!(body, b"new cat");

    // the listing story is the existing ls page.
    let (status, body) = http(port, "GET", "/v1/files/ls?path=/shared", b"");
    assert_eq!(status, 200);
    let page: serde_json::Value = serde_json::from_slice(&body).expect("ls json");
    let paths: Vec<&str> = page["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .filter_map(|e| e["path"].as_str())
        .collect();
    assert!(
        paths.contains(&"/shared/cat.jpg") && paths.contains(&"/shared/big.bin"),
        "{paths:?}"
    );

    // DELETE removes, then is an idempotent no-op.
    let (status, _) = http(port, "DELETE", "/v1/files/object/shared/cat.jpg", b"");
    assert_eq!(status, 200);
    let (status, _) = http(port, "GET", "/v1/files/object/shared/cat.jpg", b"");
    assert_eq!(status, 404, "deleted object answers 404");
    let (status, body) = http(port, "DELETE", "/v1/files/object/shared/cat.jpg", b"");
    assert_eq!(status, 200, "repeat delete is a no-op");
    assert_eq!(body, br#"{"deleted":false}"#);

    // GET of a directory is a client error, not a hang or a coerced page.
    let (status, _) = http(port, "GET", "/v1/files/object/shared", b"");
    assert_eq!(status, 400, "a directory is not an object");
}
