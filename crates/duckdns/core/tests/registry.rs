use duckdns_core::{
    DuckDnsMsg, DuckDnsName, DuckDnsQuery, DuckDnsReply, Registry, ResolvedAccount, ResolvedName,
    ResolvedNode, ServiceAnnouncement, ServiceAuthority, ServiceScope, decode_msg, decode_query,
    decode_reply, encode_msg, encode_query, encode_reply,
};

fn account(handle: &str, service: &str) -> ServiceAnnouncement {
    ServiceAnnouncement {
        scope: ServiceScope::Account {
            handle: handle.into(),
        },
        service: service.into(),
    }
}

fn network(service: &str) -> ServiceAnnouncement {
    ServiceAnnouncement {
        scope: ServiceScope::Network,
        service: service.into(),
    }
}

#[test]
fn wire_types_round_trip_without_transport_metadata() {
    let message = DuckDnsMsg::ReplaceAnnouncements {
        announcements: vec![account("orthory", "huddle")],
    };
    assert_eq!(decode_msg(&encode_msg(&message)).unwrap(), message);

    let query = DuckDnsQuery::Resolve {
        name: DuckDnsName::Account {
            handle: "orthory".into(),
        },
    };
    assert_eq!(decode_query(&encode_query(&query)).unwrap(), query);

    let reply = DuckDnsReply::Resolved(Some(ResolvedName::Account(ResolvedAccount {
        account_id: vec![7; 32],
        nodes: vec![ResolvedNode {
            node: vec![9; 32],
            node_label: "n-090909090909".into(),
        }],
    })));
    assert_eq!(decode_reply(&encode_reply(&reply)).unwrap(), reply);
}

#[test]
fn handle_claims_are_idempotent_owned_and_release_cleans_services() {
    let mut registry = Registry::new("team#A1B2C3D4").unwrap();
    let owner = vec![7; 32];
    let other = vec![8; 32];
    let node = vec![1; 32];

    registry.claim_handle(&owner, "orthory".into()).unwrap();
    registry.claim_handle(&owner, "orthory".into()).unwrap();
    assert!(registry.claim_handle(&other, "orthory".into()).is_err());
    registry
        .replace_announcements(&node, Some(&owner), vec![account("orthory", "huddle")])
        .unwrap();
    registry.commit();

    assert!(registry.release_handle(&other, "orthory").is_err());
    registry.release_handle(&owner, "orthory").unwrap();
    registry.commit();
    assert_eq!(registry.handle_owner("orthory"), None);
    assert!(registry.node_announcements(&node).is_empty());
}

#[test]
fn account_services_require_the_submitting_nodes_account() {
    let mut registry = Registry::new("team#A1B2C3D4").unwrap();
    let owner = vec![7; 32];
    let other = vec![8; 32];
    registry.claim_handle(&owner, "orthory".into()).unwrap();
    registry.commit();

    let declaration = account("orthory", "huddle");
    assert!(
        registry
            .replace_announcements(&[1; 32], None, vec![declaration.clone()])
            .is_err()
    );
    assert!(
        registry
            .replace_announcements(&[1; 32], Some(&other), vec![declaration.clone()])
            .is_err()
    );
    registry
        .replace_announcements(&[1; 32], Some(&owner), vec![declaration])
        .unwrap();
}

#[test]
fn service_resolution_returns_node_identities_not_endpoints() {
    let mut registry = Registry::new("Team Name#A1B2C3D4").unwrap();
    registry
        .replace_announcements(&[2; 32], None, vec![network("search")])
        .unwrap();
    registry
        .replace_announcements(&[1; 32], None, vec![network("search")])
        .unwrap();
    registry.commit();

    let resolved = registry
        .resolve_service(&DuckDnsName::NetworkService {
            service: "search".into(),
            chain: "team-name-a1b2c3d4".into(),
        })
        .unwrap()
        .unwrap();
    assert_eq!(resolved.providers.len(), 2);
    assert_eq!(resolved.authority, ServiceAuthority::Network);
    assert_eq!(resolved.providers[0].node, vec![1; 32]);
    assert_eq!(resolved.providers[1].node, vec![2; 32]);

    let pinned = registry
        .resolve_service(&DuckDnsName::NodeService {
            service: "search".into(),
            node: "n-020202020202".into(),
            chain: "team-name-a1b2c3d4".into(),
        })
        .unwrap()
        .unwrap();
    assert_eq!(pinned.providers.len(), 1);
    assert_eq!(pinned.providers[0].node, vec![2; 32]);

    assert!(
        registry
            .resolve_service(&DuckDnsName::Account {
                handle: "orthory".into(),
            })
            .unwrap()
            .is_none(),
        "bare account names are resolved by the identity-aware adapter"
    );
}

#[test]
fn declarative_replacement_removes_stale_services() {
    let mut registry = Registry::new("team#00000000").unwrap();
    registry
        .replace_announcements(
            &[1; 32],
            None,
            vec![network("search"), network("status")],
        )
        .unwrap();
    registry
        .replace_announcements(&[1; 32], None, vec![network("search")])
        .unwrap();
    registry.commit();

    assert_eq!(registry.node_announcements(&[1; 32]), vec![network("search")]);
    assert!(
        registry
            .resolve_service(&DuckDnsName::NetworkService {
                service: "status".into(),
                chain: "team-00000000".into(),
            })
            .unwrap()
            .is_none()
    );
}

#[test]
fn pending_changes_abort_or_commit_atomically() {
    let mut registry = Registry::new("team#00000000").unwrap();
    let owner = vec![7; 32];
    registry.claim_handle(&owner, "orthory".into()).unwrap();
    assert_eq!(registry.root_bytes(), [0; 32]);
    registry.abort();
    assert_eq!(registry.handle_owner("orthory"), None);

    registry.claim_handle(&owner, "orthory".into()).unwrap();
    registry.commit();
    assert_eq!(registry.handle_owner("orthory"), Some(owner.as_slice()));
    assert_ne!(registry.root_bytes(), [0; 32]);
}

#[test]
fn snapshot_round_trip_is_canonical_and_root_checked() {
    let mut first = Registry::new("team#00000000").unwrap();
    first.claim_handle(&[7; 32], "orthory".into()).unwrap();
    first
        .replace_announcements(&[1; 32], Some(&[7; 32]), vec![account("orthory", "huddle")])
        .unwrap();
    first.commit();

    let snapshot = first.snapshot();
    let root = first.root_bytes();
    let mut restored = Registry::new("team#00000000").unwrap();
    restored.install(&snapshot, root).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(restored.root_bytes(), root);

    let mut corrupt = snapshot.clone();
    corrupt.push(0);
    assert!(restored.install(&corrupt, root).is_err());
    assert!(restored.install(&snapshot, [9; 32]).is_err());
    assert_eq!(restored.root_bytes(), root);
}

#[test]
fn insertion_order_does_not_change_snapshot_or_root() {
    let mut first = Registry::new("team#00000000").unwrap();
    let mut second = Registry::new("team#00000000").unwrap();
    for registry in [&mut first, &mut second] {
        registry.claim_handle(&[7; 32], "orthory".into()).unwrap();
        registry.commit();
    }
    first
        .replace_announcements(
            &[1; 32],
            Some(&[7; 32]),
            vec![network("search"), account("orthory", "huddle")],
        )
        .unwrap();
    second
        .replace_announcements(
            &[1; 32],
            Some(&[7; 32]),
            vec![account("orthory", "huddle"), network("search")],
        )
        .unwrap();
    first.commit();
    second.commit();
    assert_eq!(first.snapshot(), second.snapshot());
    assert_eq!(first.root_bytes(), second.root_bytes());
}
