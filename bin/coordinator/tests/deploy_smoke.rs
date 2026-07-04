//! Proof that the DEPLOYED invocation works: boot the real compiled
//! `coordinator` binary exactly as `ops/coordinator/ducktape-coordinator.service`
//! does (`coordinator --listen <addr>`), then drive a live `NatClient` against
//! it. This is the "[WORKS TODAY]" claim in docs/deploy/coordinator.md, made
//! executable. Hermetic: `--listen 127.0.0.1:0` -> the OS picks the port, which
//! the test reads back from the binary's own stderr line (no fixed port, no
//! sleep-as-sync).

use std::net::SocketAddr;
use std::process::Stdio;
use std::time::Duration;

use nat_traversal::{NatClient, NodeKey};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

#[tokio::test]
async fn deployed_coordinator_binary_answers_a_bind_request() {
    // Boot the ACTUAL binary the recipe installs, with the OS choosing the port.
    let mut child = Command::new(env!("CARGO_BIN_EXE_coordinator"))
        .arg("--listen")
        .arg("127.0.0.1:0")
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn the compiled coordinator binary");

    // The binary prints `coordinator listening on {addr}` to stderr once bound,
    // BEFORE serving. Read the real bound address from that line — this both
    // synchronizes the test and proves the CLI parses `--listen`.
    let stderr = child.stderr.take().expect("piped stderr");
    let mut lines = BufReader::new(stderr).lines();
    let addr: SocketAddr = timeout(Duration::from_secs(10), async {
        while let Some(line) = lines.next_line().await.expect("read stderr") {
            if let Some(rest) = line.strip_prefix("coordinator listening on ") {
                return rest.trim().parse().expect("parse bound addr");
            }
        }
        panic!("coordinator exited before announcing its listen address");
    })
    .await
    .expect("coordinator must announce its listen address promptly");

    // Drive the real STUN reflexive path against the running process.
    let client = NatClient::bind(NodeKey([7u8; 32]), addr)
        .await
        .expect("bind client");
    let reflexive = timeout(Duration::from_secs(5), client.discover_reflexive())
        .await
        .expect("the deployed coordinator must answer a BindRequest")
        .expect("reflexive");

    // Wildcard client bind vs observed loopback source: the port is the
    // load-bearing invariant (same rule as bin/coordinator/tests/smoke.rs).
    assert_eq!(
        reflexive.port(),
        client.local_addr().await.unwrap().port(),
        "the coordinator echoes the client's observed reflexive port"
    );

    // Tidy up (kill_on_drop also covers a panic path).
    let _ = child.start_kill();
}
