//! bearer client invite, end to end over a real HTTP surface: mint with
//! `invite --role client`, redeem with `user-redeem-invite` as a fresh user key,
//! observe client standing in consensus, and pin single-use first-wins
//! against a second key on the same blob.

mod common;

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

use common::{NetworkShapeCluster, poll_until, serial};

/// budget for one submitted op to finalize on a solo founder.
const FINALIZE: Duration = Duration::from_secs(60);

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// does the founder's committed client set (identity's submit-door ACL facet)
/// contain `key_hex`?
fn clients_contains(cluster: &NetworkShapeCluster, key_hex: &str) -> bool {
    use identity::{IdentityQuery, IdentityReply};
    let req = identity::encode_query(&IdentityQuery::Clients);
    let Some(raw) = cluster.query(0, "identity", &req) else {
        return false;
    };
    let Ok(IdentityReply::Clients(list)) = identity::decode_reply(&raw) else {
        return false;
    };
    list.iter().any(|c| hex(c) == key_hex)
}

/// The password every key here is sealed with.
///
/// A user key is ENCRYPTED — `user key init` seals it and `user redeem-invite`
/// opens it, each reading one password line from stdin. Key A is minted once
/// and redeemed twice, so every run against it presents the same line.
const KEY_PASSWORD: &str = "bearer-e2e-password";

/// Run one `ducktape` verb with the shared password on stdin.
///
/// Every key verb reads its secret from stdin and nowhere else, so a test that
/// forgets the pipe gets `FATAL: missing password on stdin` rather than a
/// prompt — there is no terminal here to prompt at.
fn with_password(mut command: Command) -> std::process::Output {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn ducktape");
    // the handle drops at the end of this statement, closing the pipe — the
    // CLI reads its one line and then sees EOF.
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(format!("{KEY_PASSWORD}\n").as_bytes())
        .expect("write the password line");
    child.wait_with_output().expect("ducktape output")
}

/// Mint a fresh sealed user key at `path` and hand back its pubkey hex.
///
/// `user key init` prints the 24-word mnemonic line THEN the pubkey line, and
/// the pubkey is the LAST stdout line by contract.
fn mint(path: &std::path::Path) -> String {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ducktape"));
    command.arg("user").args(["key", "init", "--out"]).arg(path);
    let out = with_password(command);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "user key init failed:\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    stdout
        .trim()
        .lines()
        .last()
        .expect("pubkey is the last line")
        .to_string()
}

/// run `user redeem-invite <blob>` as the sealed key at `key_path` against the
/// founder's http surface.
fn redeem(blob: &str, http: &str, key_path: &std::path::Path) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ducktape"));
    command
        .arg("user")
        .args(["redeem-invite", blob, "--node", http, "--key"])
        .arg(key_path);
    with_password(command)
}

#[test]
fn a_bearer_client_invite_redeems_over_http_exactly_once() {
    let _guard = serial();
    let mut cluster = NetworkShapeCluster::new();
    cluster.init_founder("bearer-e2e");
    cluster.spawn(0);

    // the founder serves the /v1 app surface on its --http port once up.
    let http = format!("http://127.0.0.1:{}", cluster.http_ports[0]);
    poll_until("founder http up", FINALIZE, || {
        reqwest::blocking::get(format!("{http}/v1/status"))
            .ok()
            .filter(|r| r.status().is_success())
            .map(|_| ())
    });

    // mint a bearer client blob — no --target, no invitee key exchange.
    let out = Command::new(env!("CARGO_BIN_EXE_ducktape")).arg("node")
        .args(["invite", "--role", "client", "--config"])
        .arg(cluster.config_file(0))
        .output()
        .expect("run invite --role client");
    assert!(
        out.status.success(),
        "invite --role client failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let blob = stdout.trim().lines().last().expect("blob line").to_string();

    // key A redeems. Minting is its OWN step — `redeem-invite` opens an
    // existing key and never creates one, so the pubkey the standing is checked
    // against comes straight out of the mint rather than being re-derived.
    let scratch = tempfile::tempdir().expect("client key scratch");
    let key_a = scratch.path().join("client-a.key");
    let pub_a = mint(&key_a);
    let out = redeem(&blob, &http, &key_a);
    let verdict = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "redeem A failed:\nstdout: {verdict}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        verdict.contains("admitted: client standing committed at height"),
        "verdict names the committed height: {verdict}"
    );

    // the standing is committed consensus state, queryable from the node.
    poll_until("client standing committed", FINALIZE, || {
        clients_contains(&cluster, &pub_a).then_some(())
    });

    // a re-redeem by the SAME key is idempotent success (script-friendly)…
    let out = redeem(&blob, &http, &key_a);
    assert!(
        out.status.success(),
        "same-key re-redeem should be idempotent: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // …but a SECOND key on the same blob is refused: one invite, one person.
    let key_b = scratch.path().join("client-b.key");
    mint(&key_b);
    let out = redeem(&blob, &http, &key_b);
    assert!(
        !out.status.success(),
        "a spent bearer invite must refuse a second key"
    );
    let err = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(err.contains("already redeemed"), "single-use: {err}");
}
