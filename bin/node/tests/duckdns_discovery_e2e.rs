//! DuckDNS discovery over two real node processes. This deliberately exercises
//! no host DNS, certificate, HTTP proxy, or service transport: consensus maps a
//! logical name to eligible full NodeIds and stops there.

mod common;

use std::time::Duration;

use common::{Cluster, poll_until, serial};
use commonware_cryptography::{Signer as _, ed25519};
use duckdns::{
    DuckDnsName, DuckDnsQuery, DuckDnsReply, ResolvedName, ResolvedService, ServiceAuthority,
};

const READY: Duration = Duration::from_secs(90);

fn resolve(cluster: &Cluster, idx: usize, name: DuckDnsName) -> Option<ResolvedService> {
    let bytes = cluster.query(
        idx,
        "duckdns",
        &duckdns::encode_query(&DuckDnsQuery::Resolve { name }),
    )?;
    match duckdns::decode_reply(&bytes).ok()? {
        DuckDnsReply::Resolved(Some(ResolvedName::Service(service))) => Some(service),
        _ => None,
    }
}

#[test]
fn logical_service_converges_to_provider_node_ids() {
    let _serial = serial();
    let mut cluster = Cluster::new(&[0, 1], &[0, 1]);
    cluster.extra_toml.push(
        "[[duckdns.services]]\n\
         scope = \"network\"\n\
         service = \"huddle\""
            .into(),
    );

    for idx in 0..2 {
        cluster.spawn(idx);
    }
    for idx in 0..2 {
        cluster.wait_marker(idx, "rpc listening on", READY);
        cluster.wait_marker(idx, "converged app_hash=", READY);
    }

    let chain = duckdns::derive_chain_label(&format!("{}#00000000", cluster.namespace))
        .expect("dev namespace has a canonical discovery label");
    let name = DuckDnsName::NetworkService {
        service: "huddle".into(),
        chain,
    };
    let resolved = poll_until("both DuckDNS provider declarations", READY, || {
        let first = resolve(&cluster, 0, name.clone())?;
        let second = resolve(&cluster, 1, name.clone())?;
        (first == second && first.providers.len() == 2).then_some(first)
    });

    let mut expected = cluster
        .peer_ids
        .iter()
        .map(|seed| {
            ed25519::PrivateKey::from_seed(*seed)
                .public_key()
                .as_ref()
                .to_vec()
        })
        .collect::<Vec<_>>();
    expected.sort();
    let actual = resolved
        .providers
        .iter()
        .map(|provider| provider.node.clone())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    assert_eq!(resolved.authority, ServiceAuthority::Network);
    assert!(
        resolved
            .providers
            .iter()
            .all(|provider| provider.node.len() == 32),
        "discovery authenticates with full NodeIds, not short labels"
    );
}
