//! bearer client invite, end to end over a real HTTP surface: mint with
//! `invite --client`, redeem with `user-redeem-invite` as a fresh user key,
//! observe client standing in consensus, and pin single-use first-wins
//! against a second key on the same blob.

mod common;

use std::process::Command;
use std::time::Duration;

use common::{NetworkShapeCluster, poll_until, serial};

/// budget for one submitted op to finalize on a solo founder.
const FINALIZE: Duration = Duration::from_secs(60);

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// does the founder's committed clients set contain `key_hex`?
fn clients_contains(cluster: &NetworkShapeCluster, key_hex: &str) -> bool {
    use clients::{ClientsQuery, ClientsReply};
    let req = clients::encode_query(&ClientsQuery::Clients);
    let Some(raw) = cluster.query(0, "clients", &req) else {
        return false;
    };
    let Ok(ClientsReply::Clients(list)) = clients::decode_reply(&raw) else {
        return false;
    };
    list.iter().any(|c| hex(c) == key_hex)
}

/// run `user-redeem-invite <blob>` as the key at `key_path` (fresh path =
/// auto-minted plain identity) against the founder's http surface.
fn redeem(blob: &str, http: &str, key_path: &std::path::Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ducktape-node"))
        .args(["user-redeem-invite", blob, "--node", http, "--key"])
        .arg(key_path)
        .output()
        .expect("run user-redeem-invite")
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
    let out = Command::new(env!("CARGO_BIN_EXE_ducktape-node"))
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

    // key A redeems: a fresh --key path mints a plain identity on the spot.
    let scratch = tempfile::tempdir().expect("client key scratch");
    let key_a = scratch.path().join("client-a.key");
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
    let pub_a = {
        let out = Command::new(env!("CARGO_BIN_EXE_ducktape-node"))
            .args(["keygen", "--out"])
            .arg(&key_a)
            .output()
            .expect("keygen reuse");
        assert!(out.status.success(), "keygen reuse failed");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    };
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
