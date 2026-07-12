//! what `ducktape-mcp` itself must get right about the session lane: that a
//! write leaves this process as a correctly SIGNED `RunsMsg::AgentAction` frame,
//! whose origin is the run's session key.
//!
//! this is the one claim no other test in the tree can make for us. `runs`'s own
//! tests prove the consensus ACL (against real dispatch state, with a real
//! lease); the noded router tests prove a frame's verified signer becomes the
//! block's origin. what is left is the seam in between — that the bytes this
//! binary puts on the wire are a frame at all, that it signs them with the key
//! the provisioner gave it, and that it names the right run.
//!
//! so the node here is a STUB: a socket that captures the POSTed body and hands
//! back a receipt. the assertions are made by decoding those captured bytes with
//! `node::decode_frame` — the very function every validator uses. a frame that
//! passes here passes there, because it is the same check.

use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::sync::mpsc;

use commonware_codec::DecodeExt as _;
use commonware_cryptography::{Signer as _, ed25519};
use serde_json::{Value, json};

/// the run + agent every test here signs for.
const RUN_ID: &str = "saga-7:0";
const AGENT_ID: &str = "quackbot";

/// drive one `tools/call` against a stub node and return the raw bytes the
/// server POSTed to `/v1/submit/frame` (or `None` if it never wrote).
fn capture_frame(tool: &str, arguments: Value, session: Option<[u8; 32]>) -> Option<Vec<u8>> {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind stub");
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel::<Option<Vec<u8>>>();

    // the stub node: answer whatever the server asks, and keep the frame body.
    //
    // NON-BLOCKING with a deadline, deliberately. the session-less case is a
    // SUCCESS when the server posts nothing at all — so a stub that parked in
    // accept() waiting for a request that must never come would hang exactly the
    // test whose whole point is that silence.
    let stub = std::thread::spawn(move || {
        listener.set_nonblocking(true).expect("nonblocking");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while std::time::Instant::now() < deadline {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream.set_nonblocking(false).expect("blocking stream");
                    let (path, body) = read_request(&mut stream);
                    let is_frame = path.starts_with("/v1/submit/frame");
                    // a receipt for a submit; an empty agent record for any
                    // registry lookup the server makes first.
                    let reply = if is_frame {
                        json!({"height": 1, "app_hash": "00", "op_hash": "00"}).to_string()
                    } else {
                        json!({"agent": null}).to_string()
                    };
                    let _ = stream.write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
                            reply.len()
                        )
                        .as_bytes(),
                    );
                    if is_frame {
                        let _ = tx.send(Some(body));
                        return;
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
                Err(_) => break,
            }
        }
        let _ = tx.send(None);
    });

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ducktape-mcp"));
    cmd.env("DUCKTAPE_NODE", format!("http://127.0.0.1:{port}"))
        .env("DUCKTAPE_RUN_AGENT", AGENT_ID)
        .env("DUCKTAPE_RUN_ID", RUN_ID)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    match session {
        // the provisioner hands over the 32-byte SEED as hex, which
        // `PrivateKey::decode` reads back — the same encoding the node's own key
        // file uses. the test must speak it exactly, or it would be proving a
        // format production does not produce.
        Some(seed) => {
            cmd.env("DUCKTAPE_RUN_SESSION_KEY", hex(&seed));
        }
        None => {
            cmd.env_remove("DUCKTAPE_RUN_SESSION_KEY");
        }
    }

    let mut child = cmd.spawn().expect("spawn ducktape-mcp");
    let mut stdin = child.stdin.take().unwrap();
    for frame in [
        json!({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {}}),
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {"name": tool, "arguments": arguments},
        }),
    ] {
        writeln!(stdin, "{frame}").expect("write");
    }
    stdin.flush().unwrap();
    drop(stdin);
    let _ = child.wait_with_output();

    let captured = rx.recv_timeout(std::time::Duration::from_secs(20)).ok().flatten();
    let _ = stub.join();
    captured
}

#[test]
fn a_write_leaves_as_an_agent_action_frame_signed_by_the_session_key() {
    let seed = [42u8; 32];
    let frame =
        capture_frame("ducktape_task_create", json!({"title": "prove it"}), Some(seed))
            .expect("the write must reach /v1/submit/frame");

    // decoded by the SAME function every validator uses. it verifies the
    // signature and hands back the origin it binds — so if this passes, the
    // frame is one consensus would accept, not merely one that looks like it.
    let (origin, msg) = node::decode_frame(&frame).expect("a frame consensus would accept");

    // the origin IS the session public key. this is the whole claim: the write
    // carries cryptographic proof of which run made it, which the frameless lane
    // (whose origin string bin/node discards) cannot do at all.
    assert_eq!(
        origin,
        sdk::Origin::External(signer(seed).public_key().as_ref().to_vec()),
        "the frame's origin must be the session key the provisioner handed us"
    );

    // ...and it is addressed to runs, as an AgentAction naming THIS run.
    assert_eq!(msg.target, "runs");
    match runs::decode_msg(&msg.payload).expect("a RunsMsg") {
        runs::RunsMsg::AgentAction { run_id, action } => {
            assert_eq!(run_id, RUN_ID, "the action must name the run it belongs to");
            match action {
                agent::AgentAction::CreateTask { title, .. } => assert_eq!(title, "prove it"),
                other => panic!("expected CreateTask, got {other:?}"),
            }
        }
        other => panic!("expected an AgentAction, got {other:?}"),
    }
}

#[test]
fn a_tampered_frame_no_longer_verifies() {
    // the guard behind the guard: if `decode_frame` would happily accept a
    // mutated payload, then asserting "we produced a valid frame" would prove
    // nothing at all. flip one byte of the signed payload and it must fail.
    let seed = [7u8; 32];
    let mut frame = capture_frame("ducktape_task_create", json!({"title": "tamper"}), Some(seed))
        .expect("a frame");

    let mid = frame.len() / 2;
    frame[mid] ^= 0xff;
    assert!(
        node::decode_frame(&frame).is_err(),
        "a frame whose signed bytes changed must not verify"
    );
}

#[test]
fn every_action_of_a_session_gets_its_own_sequence() {
    // (origin, seq) is the ordered lane's replay identity. a session key is
    // fresh per run, so its first op is seq 0 — and two ops that reused a seq
    // would be the same op twice as far as the lane is concerned.
    let seed = [9u8; 32];
    let frame = capture_frame("ducktape_task_create", json!({"title": "first"}), Some(seed))
        .expect("a frame");
    let (_, seq) = node::frame_origin_seq(&frame).expect("a decodable envelope");
    assert_eq!(seq, 0, "a fresh session key starts its sequence at 0");
}

#[test]
fn a_run_with_no_session_key_refuses_to_write_rather_than_forging_one() {
    // no session key means no credential to prove the write came from this
    // agent. the binary must REFUSE — never quietly fall back to the frameless
    // lane, which would file the write under the executing node's identity and
    // lose the agent entirely. that fallback is exactly the defect this design
    // exists to remove, so it is asserted absent.
    let posted = capture_frame("ducktape_task_create", json!({"title": "nope"}), None);
    assert!(
        posted.is_none(),
        "a session-less run must not put ANY write on the wire"
    );
}

// ---- test plumbing ----------------------------------------------------------

/// read one HTTP request off the socket; return its path and body bytes.
fn read_request(stream: &mut TcpStream) -> (String, Vec<u8>) {
    let mut reader = BufReader::new(stream.try_clone().expect("clone"));
    let mut start = String::new();
    reader.read_line(&mut start).expect("request line");
    let path = start.split_whitespace().nth(1).unwrap_or("/").to_string();

    let mut len = 0usize;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).expect("header");
        if line.trim().is_empty() {
            break;
        }
        if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            len = v.trim().parse().unwrap_or(0);
        }
    }
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body).expect("body");
    (path, body)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// the signer a 32-byte seed yields — exactly how the server rebuilds it from
/// the hex the provisioner set.
fn signer(seed: [u8; 32]) -> ed25519::PrivateKey {
    ed25519::PrivateKey::decode(seed.as_slice()).expect("32 bytes decode")
}

/// keep the `Decode` import honest — the session key the provisioner hands over
/// is hex, and this is the round trip the server does on it.
#[test]
fn the_session_key_hex_round_trips_into_a_signer() {
    let seed = [3u8; 32];
    let encoded = hex(&seed);
    let raw: Vec<u8> = (0..encoded.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&encoded[i..i + 2], 16).unwrap())
        .collect();
    let decoded = ed25519::PrivateKey::decode(raw.as_slice()).expect("32 bytes decode");
    assert_eq!(decoded.public_key(), signer(seed).public_key());
}
