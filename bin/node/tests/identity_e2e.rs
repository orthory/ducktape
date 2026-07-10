//! live 2-validator e2e for the `identity` module: one USER key binds two
//! DIFFERENT node identities across a real cluster, and the registry
//! converges identically on both.
//!
//! `bin/node` has no lib target (only a `[[bin]]`), so the cert-minting
//! helpers (`config::mint_bind_cert`/`mint_unbind_cert`) are not reachable
//! from an integration test — they are replicated inline from the identity
//! crate's public `bind_preimage`/`unbind_preimage` + a commonware ed25519
//! sign, exactly as `config.rs` itself does it.

mod common;

use std::time::Duration;

use commonware_cryptography::{Signer as _, ed25519};
use common::{Cluster, poll_until, serial};
use identity::{
    AccountView, IdentityMsg, IdentityQuery, IdentityReply, MemberAuth, decode_reply, encode_msg,
    encode_query,
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
        .map(|i| cluster.wait_marker(i, "genesis app_hash=", Duration::from_secs(60)))
        .collect();
    assert_eq!(genesis[0], genesis[1], "genesis fork between nodes 0 and 1");
    for i in 0..2 {
        cluster.wait_marker(i, "converged app_hash=", CONVERGE);
    }
}

/// build the `MemberAuth` a bind op carries, exactly like
/// `config::ed25519_member_auth` over the bind preimage: the user (ed25519)
/// key's signature in the bind NS domain, wrapped as an ed25519 member.
fn bind_auth(user: &ed25519::PrivateKey, chain_id: &str, node_pub: &[u8], nonce: u64) -> MemberAuth {
    MemberAuth {
        key: user.public_key().as_ref().to_vec(),
        kind: identity::KeyKind::Ed25519,
        proof: identity::MemberProof::Signature {
            sig: user
                .sign(
                    identity::IDENTITY_BIND_NS,
                    &identity::bind_preimage(chain_id, node_pub, nonce),
                )
                .as_ref()
                .to_vec(),
        },
    }
}

/// the `MemberAuth` an unbind op carries (same shape, unbind NS domain).
fn unbind_auth(
    user: &ed25519::PrivateKey,
    chain_id: &str,
    node_pub: &[u8],
    nonce: u64,
) -> MemberAuth {
    MemberAuth {
        key: user.public_key().as_ref().to_vec(),
        kind: identity::KeyKind::Ed25519,
        proof: identity::MemberProof::Signature {
            sig: user
                .sign(
                    identity::IDENTITY_UNBIND_NS,
                    &identity::unbind_preimage(chain_id, node_pub, nonce),
                )
                .as_ref()
                .to_vec(),
        },
    }
}

/// `UserOf(node_key)` on node `idx`. `None` covers both a rejected query and
/// a query that resolved successfully to "not bound" — the caller only ever
/// needs to distinguish "resolved as user X" from "not (yet) that", and
/// `poll_until` already treats both as "not yet" while a bind is landing.
fn account_of_node(cluster: &Cluster, idx: usize, node_key: &[u8]) -> Option<AccountView> {
    let reply = cluster.query(
        idx,
        "identity",
        &encode_query(&IdentityQuery::OfNode {
            node_key: node_key.to_vec(),
        }),
    )?;
    match decode_reply(&reply).ok()? {
        IdentityReply::Account(a) => a,
        IdentityReply::Accounts(_) => None,
    }
}

/// both validators' committed `identity` module root (hex, from `status`).
fn identity_roots(cluster: &Cluster) -> [serde_json::Value; 2] {
    [cluster.status(0)["modules"]["identity"].clone(), cluster.status(1)["modules"]["identity"].clone()]
}

#[test]
fn identity_two_nodes_one_user() {
    let _serial = serial();
    let mut cluster = Cluster::new(&[0, 1], &[0, 1]);
    boot(&mut cluster);

    // the harness's dev-shape chain id IS `Cluster::namespace` (config.rs:
    // `chain_id: namespace.clone()` for the dev shape) — the exact string
    // `Identity::new` was constructed with on both nodes.
    let chain_id = cluster.namespace.clone();
    let user = ed25519::PrivateKey::from_seed(42);
    let user_pub = user.public_key().as_ref().to_vec();
    let a_pub = Cluster::identity(0);
    let b_pub = Cluster::identity(1);

    // node A binds ITSELF (the verified submit origin) to the user. fresh
    // user record -> nonce 0.
    cluster.submit(
        0,
        "identity",
        &encode_msg(&IdentityMsg::BindNode {
            authorizer: bind_auth(&user, &chain_id, &a_pub, 0),
        }),
    );
    poll_until("node A's bind to finalize", FINALIZE, || {
        account_of_node(&cluster, 0, &a_pub).filter(|v| v.account_id == user_pub)
    });

    // node B binds itself to the SAME user. A's bind already bumped the
    // user's nonce to 1 — the cert must sign over that current nonce.
    cluster.submit(
        1,
        "identity",
        &encode_msg(&IdentityMsg::BindNode {
            authorizer: bind_auth(&user, &chain_id, &b_pub, 1),
        }),
    );
    poll_until("node B's bind to finalize", FINALIZE, || {
        account_of_node(&cluster, 1, &b_pub).filter(|v| v.account_id == user_pub)
    });

    // both binds must be visible from EITHER node's rpc: UserOf(A) and
    // UserOf(B) resolve to the same user, with both nodes in its set.
    let sees_both = |view: &AccountView| {
        view.account_id == user_pub
            && view.nodes.len() == 2
            && view.nodes.contains(&a_pub)
            && view.nodes.contains(&b_pub)
    };
    for reader in [0usize, 1] {
        let a_view = poll_until(
            &format!("OfNode(A) to show both bindings on node {reader}"),
            FINALIZE,
            || account_of_node(&cluster, reader, &a_pub).filter(&sees_both),
        );
        let b_view = poll_until(
            &format!("OfNode(B) to show both bindings on node {reader}"),
            FINALIZE,
            || account_of_node(&cluster, reader, &b_pub).filter(&sees_both),
        );
        assert_eq!(a_view.account_id, user_pub, "node {reader}: OfNode(A) wrong account");
        assert_eq!(b_view.account_id, user_pub, "node {reader}: OfNode(B) wrong account");
        assert_eq!(a_view.nodes.len(), 2, "node {reader}: expected both nodes bound (via A)");
        assert_eq!(b_view.nodes.len(), 2, "node {reader}: expected both nodes bound (via B)");
        let mut nodes = a_view.nodes.clone();
        nodes.sort();
        let mut expected = vec![a_pub.clone(), b_pub.clone()];
        expected.sort();
        assert_eq!(nodes, expected, "node {reader}: wrong bound node set");
    }

    // both validators agree on the identity module's committed root.
    poll_until("identity module root to converge across nodes", FINALIZE, || {
        let roots = identity_roots(&cluster);
        (!roots[0].is_null() && roots[0] == roots[1]).then_some(())
    });

    // node B unbinds node A — the recovery path: a surviving device evicts a
    // lost one, with NO member/origin gate beyond "external". two prior
    // binds each bumped the nonce once, so the cert signs over nonce 2.
    cluster.submit(
        1,
        "identity",
        &encode_msg(&IdentityMsg::UnbindNode {
            node_key: a_pub.clone(),
            authorizer: unbind_auth(&user, &chain_id, &a_pub, 2),
        }),
    );
    for reader in [0usize, 1] {
        poll_until(&format!("UnbindNode(A) to finalize on node {reader}"), FINALIZE, || {
            account_of_node(&cluster, reader, &a_pub).is_none().then_some(())
        });
    }

    // node B remains bound, alone, on both nodes' views.
    for reader in [0usize, 1] {
        let b_view = poll_until(&format!("OfNode(B) after unbind on node {reader}"), FINALIZE, || {
            account_of_node(&cluster, reader, &b_pub)
        });
        assert_eq!(b_view.account_id, user_pub, "node {reader}: B should remain bound");
        assert_eq!(b_view.nodes, vec![b_pub.clone()], "node {reader}: only B should remain bound");
    }

    // final convergence: both validators still agree on the identity root
    // after the unbind commits.
    poll_until("identity module root to converge post-unbind", FINALIZE, || {
        let roots = identity_roots(&cluster);
        (!roots[0].is_null() && roots[0] == roots[1]).then_some(())
    });
}
