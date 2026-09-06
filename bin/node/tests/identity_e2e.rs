//! live 2-validator e2e for the `identity` module: one USER founds an account
//! through node A, a second device key JOINS it through node B on the founder's
//! consent, and the association converges identically on both validators; then
//! the junior key cannot evict the founder, while the founder can revoke the
//! junior key, and the registry converges again.
//!
//! every op is a USER-signed frame over `/v1/submit/frame`: the module
//! attributes each op to its frame origin, and no node key is involved — a
//! node is never bound to an account.

mod common;

use std::time::Duration;

use common::{
    Cluster, account_of_key, add_key, create_account, key_gen, submit_frame, try_submit_frame,
};
use commonware_cryptography::{Signer as _, ed25519};
use identity::{
    AccountView, IdentityMsg, IdentityQuery, IdentityReply, decode_reply, encode_query,
};

/// convergence budget: mesh formation + leader rotation are real-time on a
/// possibly-loaded CI core; polls exit early, so generosity is free.
const CONVERGE: Duration = Duration::from_secs(180);
/// budget for one submitted op to finalize and become readable elsewhere.
const FINALIZE: Duration = Duration::from_secs(60);

/// boot the standard 2-validator cluster and wait for genesis agreement +
/// liveness — the shared preamble every socket e2e in this suite uses.
fn boot(cluster: &mut Cluster) {
    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", Duration::from_secs(60));
    cluster.spawn(1);
    let genesis: Vec<String> = (0..2)
        .map(|i| cluster.wait_marker(i, "genesis root_hash=", Duration::from_secs(60)))
        .collect();
    assert_eq!(genesis[0], genesis[1], "genesis fork between nodes 0 and 1");
    for i in 0..2 {
        cluster.wait_marker(i, "converged root_hash=", CONVERGE);
    }
}

/// `Get { number }` on node `idx`.
fn account(cluster: &Cluster, idx: usize, number: u64) -> Option<AccountView> {
    let reply = cluster.query(
        idx,
        "identity",
        &encode_query(&IdentityQuery::Get { number }),
    )?;
    match decode_reply(&reply).ok()? {
        IdentityReply::Account(a) => a,
        IdentityReply::Accounts(_) | IdentityReply::Gen(_) => None,
    }
}

/// the association's public keys, ascending (the order the module exposes).
fn pubkeys(view: &AccountView) -> Vec<Vec<u8>> {
    view.keys.iter().map(|k| k.pubkey.clone()).collect()
}

/// both validators' committed `identity` module root (hex, from `status`).
fn identity_roots(cluster: &Cluster) -> [serde_json::Value; 2] {
    [
        cluster.status(0)["modules"]["identity"].clone(),
        cluster.status(1)["modules"]["identity"].clone(),
    ]
}

fn assert_roots_converge(cluster: &Cluster, what: &str) {
    cluster.await_committed(0, what, FINALIZE, || {
        let roots = identity_roots(cluster);
        (!roots[0].is_null() && roots[0] == roots[1]).then_some(())
    });
}

#[test]
fn identity_two_nodes_one_account() {
    let mut cluster = Cluster::new(&[0, 1], &[0, 1]);
    boot(&mut cluster);

    let founder = ed25519::PrivateKey::from_seed(42);
    let joiner = ed25519::PrivateKey::from_seed(43);
    let founder_pub = founder.public_key().as_ref().to_vec();
    let joiner_pub = joiner.public_key().as_ref().to_vec();
    let mut both = vec![founder_pub.clone(), joiner_pub.clone()];
    both.sort();

    // the founder's Create lands through node A: the first account is 1, and
    // `Get { 1 }` reads it on EITHER node with the founder as its sole key.
    let number = create_account(&cluster, 0, &founder, "alice");
    assert_eq!(number, 1, "the first account a chain founds is 1");
    for reader in [0usize, 1] {
        let view = cluster.await_committed(
            reader,
            &format!("Get(1) on node {reader}"),
            FINALIZE,
            || account(&cluster, reader, 1),
        );
        assert_eq!(view.name, "alice", "node {reader}: name");
        assert_eq!(
            pubkeys(&view),
            vec![founder_pub.clone()],
            "node {reader}: the founder is the sole key"
        );
    }

    // the joiner is admitted through node B on the founder's consent at the
    // joiner's current generation (0). the frame is signed by the JOINER: the
    // origin is the key being admitted, the consent names it.
    let view = add_key(&cluster, 1, &founder, &joiner);
    assert_eq!(view.number, 1, "the joiner landed in the founder's account");

    // both keys resolve to account 1 from EITHER node, with both keys listed.
    let sees_both = |view: &AccountView| view.number == 1 && pubkeys(view) == both;
    for reader in [0usize, 1] {
        for (who, key) in [("founder", &founder_pub), ("joiner", &joiner_pub)] {
            cluster.await_committed(
                reader,
                &format!("OfKey({who}) to show both keys on node {reader}"),
                FINALIZE,
                || account_of_key(&cluster, reader, key).filter(&sees_both),
            );
        }
    }
    assert_roots_converge(&cluster, "identity module root to converge after the join");

    // A newly admitted key cannot use its membership to evict an older one.
    let before = identity_roots(&cluster);
    let (status, rejection) = try_submit_frame(
        &cluster,
        1,
        &joiner,
        "identity",
        &identity::encode_msg(&IdentityMsg::RemoveKey {
            key: founder_pub.clone(),
        }),
    );
    assert_eq!(status, 400, "junior key removal must reject: {rejection}");
    assert!(
        rejection["error"]
            .as_str()
            .is_some_and(|reason| reason.contains("cannot remove a key admitted before your own")),
        "{rejection}"
    );
    assert_eq!(identity_roots(&cluster), before);
    for reader in [0usize, 1] {
        assert!(sees_both(
            &account(&cluster, reader, 1).expect("the account remains")
        ));
    }

    // The founder may revoke the junior device, leaving its own account intact.
    submit_frame(
        &cluster,
        0,
        &founder,
        "identity",
        &identity::encode_msg(&IdentityMsg::RemoveKey {
            key: joiner_pub.clone(),
        }),
    );
    for reader in [0usize, 1] {
        cluster.await_committed(
            reader,
            &format!("OfKey(joiner) to clear on node {reader}"),
            FINALIZE,
            || {
                account_of_key(&cluster, reader, &joiner_pub)
                    .is_none()
                    .then_some(())
            },
        );
        let view = cluster.await_committed(
            reader,
            &format!("OfKey(founder) on node {reader}"),
            FINALIZE,
            || account_of_key(&cluster, reader, &founder_pub),
        );
        assert_eq!(view.number, 1, "node {reader}: the founder keeps account 1");
        assert_eq!(
            pubkeys(&view),
            vec![founder_pub.clone()],
            "node {reader}: only the founder remains"
        );
    }

    // Revocation preserves the joiner's consumed consent generation: a new
    // admission must sign at 1. The founder's Create leaves its generation 0.
    for reader in [0usize, 1] {
        assert_eq!(
            key_gen(&cluster, reader, &joiner_pub),
            Some(1),
            "node {reader}: the joiner's generation"
        );
        assert_eq!(
            key_gen(&cluster, reader, &founder_pub),
            Some(0),
            "node {reader}: the founder's generation"
        );
    }
    assert_roots_converge(
        &cluster,
        "identity module root to converge after the removal",
    );
}
