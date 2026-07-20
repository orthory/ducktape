//! the desktop app's signed-commit lane, end to end minus the webview: the
//! EXACT payload files-client.ts builds (`JSON.stringify({ commit: body })`,
//! serde's externally-tagged `FilesMsg`), wrapped in the op frame
//! `user-sign-frame` prints, POSTed raw to `/v1/submit/frame` — and the files
//! module records the frame's VERIFIED signer as the commit's author, with
//! real `/home/<signer>` authority. the negative half proves the point of the
//! lane: the same commit through the unsigned convenience lane (author
//! `ext:noded`) is REJECTED from that home subtree.

#[path = "fs_support/mod.rs"]
mod support;

use std::io::{Read as _, Write as _};
use std::net::TcpStream;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use commonware_cryptography::{Signer as _, ed25519};
use support::Harness;

/// minimal raw http/1.1 exchange with a body — returns (status, body).
fn http(port: u16, method: &str, path: &str, content_type: &str, body: &[u8]) -> (u16, Vec<u8>) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect harness");
    let head = format!(
        "{method} {path} HTTP/1.1\r\nhost: 127.0.0.1\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).expect("write head");
    stream.write_all(body).expect("write body");
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).expect("read response");
    let text_head_end = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("response has a header/body split");
    let head_text = String::from_utf8_lossy(&raw[..text_head_end]);
    let status: u16 = head_text
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .expect("status line");
    (status, raw[text_head_end + 4..].to_vec())
}

/// the commit payload exactly as the TS transport serializes it.
fn commit_payload(path: &str, message: &str, bytes: &[u8]) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "commit": {
            "base_snapshot": null,
            "message": message,
            "changes": [{
                "put": {
                    "path": path,
                    "exec": false,
                    "meta": {},
                    "content": { "inline": { "b64": STANDARD.encode(bytes) } },
                }
            }],
        }
    }))
    .expect("commit payload serializes")
}

#[test]
fn signed_commit_lands_with_the_signer_as_author_and_home_authority() {
    let h = Harness::start();
    let port = h.node_url()
        .rsplit(':')
        .next()
        .and_then(|p| p.parse::<u16>().ok())
        .expect("harness port");

    let signer = ed25519::PrivateKey::from_seed(7);
    let actor = format!("ext:{}", noded::hex_bytes(signer.public_key().as_ref()));
    let home = format!("/home/{actor}/notes.txt");

    // the signer's own home subtree: authorized because the frame's verified
    // origin IS the owner. seq is a tie-breaker only — any u64.
    let payload = commit_payload(&home, "signed from the app", b"authored bytes");
    let frame = node::encode_frame(
        &signer,
        1,
        &sdk::Msg {
            target: "files".into(),
            payload,
        },
    );
    let (status, body) = http(port, "POST", "/v1/submit/frame", "application/octet-stream", &frame);
    assert_eq!(
        status,
        200,
        "signed commit lands: {}",
        String::from_utf8_lossy(&body)
    );

    // the history records the SIGNER as the author.
    let (status, body) = http(port, "GET", "/v1/files/history?limit=8", "application/json", b"");
    assert_eq!(status, 200);
    let history: serde_json::Value = serde_json::from_slice(&body).expect("history json");
    let author = history["snapshots"][0]["author"]
        .as_str()
        .expect("author string");
    assert_eq!(author, actor, "the commit's author is the frame's verified signer");

    // the negative half: the unsigned convenience lane writes as ext:noded,
    // which has NO authority over this signer's home subtree — rejected.
    let commit_body = serde_json::json!({
        "base_snapshot": history["snapshots"][0]["id"],
        "message": "forged from the daemon lane",
        "changes": [{
            "put": {
                "path": home,
                "exec": false,
                "meta": {},
                "content": { "inline": { "b64": STANDARD.encode(b"forged") } },
            }
        }],
    });
    let (status, body) = http(
        port,
        "POST",
        "/v1/files/commit",
        "application/json",
        &serde_json::to_vec(&commit_body).unwrap(),
    );
    assert_eq!(
        status,
        400,
        "the daemon-origin lane cannot write another owner's home: {}",
        String::from_utf8_lossy(&body)
    );

    // and a tampered frame (payload swapped after signing) never executes.
    let mut tampered = node::encode_frame(
        &signer,
        2,
        &sdk::Msg {
            target: "files".into(),
            payload: commit_payload(&home, "pre-tamper", b"a"),
        },
    );
    let n = tampered.len();
    tampered[n - 70] ^= 0x01; // flip a payload byte behind the signature
    let (status, _) = http(port, "POST", "/v1/submit/frame", "application/octet-stream", &tampered);
    assert_eq!(status, 400, "a tampered frame is refused");
}
