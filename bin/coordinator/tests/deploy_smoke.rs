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

use commonware_cryptography::{Signer as _, ed25519};
use nat_traversal::{NatClient, NodeKey};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

#[tokio::test]
async fn help_prints_usage_without_binding() {
    let output = Command::new(env!("CARGO_BIN_EXE_coordinator"))
        .arg("--help")
        .output()
        .await
        .expect("run coordinator --help");

    assert!(
        output.status.success(),
        "coordinator --help should exit successfully"
    );
    let stdout = String::from_utf8(output.stdout).expect("help is utf8");
    assert!(stdout.contains("Usage:"));
    assert!(stdout.contains("--listen <addr>"));
    assert!(stdout.contains("--genesis-set <network.toml>"));
    assert!(stdout.contains("--allow-anonymous"));
    assert!(
        output.stderr.is_empty(),
        "help should not announce a bound coordinator"
    );
}

#[tokio::test]
async fn cli_rejects_missing_and_unknown_flags() {
    for args in [
        vec!["--listen"],
        vec!["--listen", "--allow-anonymous"],
        vec!["--wat"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_coordinator"))
            .args(args)
            .output()
            .await
            .expect("run coordinator with bad args");

        assert!(!output.status.success(), "bad coordinator args should fail");
    }
}

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

    // Drive the real STUN reflexive path against the running process. The
    // service boots with NO auth flag, so the deployed default is public +
    // proof-of-possession: a real node signs its request with its identity key.
    // Bind an authenticating client whose NodeKey is that key's public half.
    let signer = ed25519::PrivateKey::from_seed(7);
    let key = {
        let mut k = [0u8; 32];
        k.copy_from_slice(signer.public_key().as_ref());
        NodeKey(k)
    };
    let client = NatClient::bind_multi_auth(key, vec![addr], signer, None)
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
