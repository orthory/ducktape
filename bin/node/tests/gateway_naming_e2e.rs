//! `.duck` naming over two real node processes, on the MERGED gateway module's
//! handle plane. It maps one optional `.duck` account name to an account NUMBER
//! and deliberately exposes no NodeId, route, endpoint, or service discovery
//! state. The handle is claimed by a USER-signed frame: the gateway resolves
//! the account from the frame origin through identity's `OfKey`.

mod common;

use std::time::Duration;

use common::{Cluster, create_account, poll_until, serial, submit_frame};
use commonware_cryptography::{Signer as _, ed25519};
use gateway::{
    DuckDnsName, GatewayMsg, GatewayQuery, GatewayReply, ResolvedAccount,
    decode_reply as gw_decode_reply, encode_msg as gw_encode_msg, encode_query as gw_encode_query,
};

const READY: Duration = Duration::from_secs(90);
const FINALIZE: Duration = Duration::from_secs(60);

/// Outer `Option` is query/decode success; inner `Option` is the naming result.
fn resolve(cluster: &Cluster, reader: usize) -> Option<Option<ResolvedAccount>> {
    let bytes = cluster.query(
        reader,
        "gateway",
        &gw_encode_query(&GatewayQuery::Resolve {
            name: DuckDnsName {
                handle: "alice".into(),
            },
        }),
    )?;
    match gw_decode_reply(&bytes).ok()? {
        GatewayReply::Resolved(account) => Some(account),
        _ => None,
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
        cluster.wait_marker(idx, "converged root_hash=", READY);
    }

    let member = ed25519::PrivateKey::from_seed(42);
    let number = create_account(&cluster, 0, &member, "alice");

    submit_frame(
        &cluster,
        0,
        &member,
        "gateway",
        &gw_encode_msg(&GatewayMsg::SetHandle {
            handle: Some("alice".into()),
        }),
    );
    for reader in 0..2 {
        let resolved = poll_until(&format!("alice.duck on node {reader}"), FINALIZE, || {
            resolve(&cluster, reader).flatten()
        });
        assert_eq!(resolved.account_id, number);
        let wire = serde_json::to_string(&resolved).unwrap();
        for excluded in ["node", "service", "provider", "endpoint", "route", "port"] {
            assert!(
                !wire.contains(excluded),
                "{excluded} leaked into naming result"
            );
        }
    }

    submit_frame(
        &cluster,
        0,
        &member,
        "gateway",
        &gw_encode_msg(&GatewayMsg::SetHandle { handle: None }),
    );
    for reader in 0..2 {
        poll_until(
            &format!("alice.duck removal on node {reader}"),
            FINALIZE,
            || matches!(resolve(&cluster, reader), Some(None)).then_some(()),
        );
    }
}
