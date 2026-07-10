//! DuckDNS naming over two real node processes. The module maps one optional
//! `.duck` account name to AccountId and deliberately exposes no NodeId, route,
//! endpoint, or service discovery state.

mod common;

use std::time::Duration;

use common::{Cluster, poll_until, serial};
use commonware_cryptography::{Signer as _, ed25519};
use duckdns::{
    DuckDnsMsg, DuckDnsName, DuckDnsQuery, DuckDnsReply, ResolvedAccount,
    decode_reply as duckdns_decode_reply, encode_msg as duckdns_encode_msg,
    encode_query as duckdns_encode_query,
};
use identity::{
    AccountView, IdentityMsg, IdentityQuery, IdentityReply, MemberAuth,
    decode_reply as identity_decode_reply, encode_msg as identity_encode_msg,
    encode_query as identity_encode_query,
};

const READY: Duration = Duration::from_secs(90);
const FINALIZE: Duration = Duration::from_secs(60);

fn bind_auth(member: &ed25519::PrivateKey, chain_id: &str, node: &[u8]) -> MemberAuth {
    MemberAuth {
        key: member.public_key().as_ref().to_vec(),
        kind: identity::KeyKind::Ed25519,
        proof: identity::MemberProof::Signature {
            sig: member
                .sign(
                    identity::IDENTITY_BIND_NS,
                    &identity::bind_preimage(chain_id, node, 0),
                )
                .as_ref()
                .to_vec(),
        },
    }
}

fn account_of_node(cluster: &Cluster, reader: usize, node: &[u8]) -> Option<AccountView> {
    let bytes = cluster.query(
        reader,
        "identity",
        &identity_encode_query(&IdentityQuery::OfNode {
            node_key: node.to_vec(),
        }),
    )?;
    match identity_decode_reply(&bytes).ok()? {
        IdentityReply::Account(account) => account,
        IdentityReply::Accounts(_) => None,
    }
}

/// Outer `Option` is query/decode success; inner `Option` is the naming result.
fn resolve(cluster: &Cluster, reader: usize) -> Option<Option<ResolvedAccount>> {
    let bytes = cluster.query(
        reader,
        "duckdns",
        &duckdns_encode_query(&DuckDnsQuery::Resolve {
            name: DuckDnsName {
                handle: "alice".into(),
            },
        }),
    )?;
    match duckdns_decode_reply(&bytes).ok()? {
        DuckDnsReply::Resolved(account) => Some(account),
        DuckDnsReply::Registrations(_) => None,
    }
}

#[test]
fn account_name_converges_without_node_discovery() {
    let _serial = serial();
    let mut cluster = Cluster::new(&[0, 1], &[0, 1]);
    for idx in 0..2 {
        cluster.spawn(idx);
    }
    for idx in 0..2 {
        cluster.wait_marker(idx, "rpc listening on", READY);
        cluster.wait_marker(idx, "converged app_hash=", READY);
    }

    let member = ed25519::PrivateKey::from_seed(42);
    let account_id = member.public_key().as_ref().to_vec();
    let node = Cluster::identity(0);
    cluster.submit(
        0,
        "identity",
        &identity_encode_msg(&IdentityMsg::BindNode {
            authorizer: bind_auth(&member, &cluster.namespace, &node),
        }),
    );
    poll_until("identity bind", FINALIZE, || {
        account_of_node(&cluster, 0, &node).filter(|account| account.account_id == account_id)
    });

    cluster.submit(
        0,
        "duckdns",
        &duckdns_encode_msg(&DuckDnsMsg::SetHandle {
            handle: Some("alice".into()),
        }),
    );
    for reader in 0..2 {
        let resolved = poll_until(&format!("alice.duck on node {reader}"), FINALIZE, || {
            resolve(&cluster, reader).flatten()
        });
        assert_eq!(resolved.account_id, account_id);
        let wire = serde_json::to_string(&resolved).unwrap();
        for excluded in ["node", "service", "provider", "endpoint", "route", "port"] {
            assert!(
                !wire.contains(excluded),
                "{excluded} leaked into naming result"
            );
        }
    }

    cluster.submit(
        0,
        "duckdns",
        &duckdns_encode_msg(&DuckDnsMsg::SetHandle { handle: None }),
    );
    for reader in 0..2 {
        poll_until(
            &format!("alice.duck removal on node {reader}"),
            FINALIZE,
            || matches!(resolve(&cluster, reader), Some(None)).then_some(()),
        );
    }
}
