use duckdns_core::{
    DuckDnsMsg, DuckDnsName, DuckDnsQuery, DuckDnsReply, Registry, ResolvedAccount, ResolvedName,
    ResolvedNode, ServiceAnnouncement, ServiceAuthority, ServiceScope, decode_msg, decode_query,
    decode_reply, encode_msg, encode_query, encode_reply,
};

fn account(service: &str) -> ServiceAnnouncement {
    ServiceAnnouncement {
        scope: ServiceScope::Account,
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
        announcements: vec![account("huddle")],
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

    let register = DuckDnsMsg::SetHandle {
        handle: Some("orthory".into()),
    };
    assert_eq!(
        String::from_utf8(encode_msg(&register)).unwrap(),
        r#"{"set_handle":{"handle":"orthory"}}"#
    );
    assert_eq!(decode_msg(&encode_msg(&register)).unwrap(), register);
    assert_eq!(
        String::from_utf8(encode_query(&DuckDnsQuery::Registrations {
            from: 0,
            limit: 256,
        }))
        .unwrap(),
        r#"{"registrations":{"from":0,"limit":256}}"#
    );
}

#[test]
fn handle_registration_is_declarative_unique_per_account_and_service_independent() {
    let mut registry = Registry::new("team#A1B2C3D4").unwrap();
    let owner = vec![7; 32];
    let other = vec![8; 32];
    let node = vec![1; 32];

    registry.set_handle(&owner, Some("orthory".into())).unwrap();
    registry.set_handle(&owner, Some("orthory".into())).unwrap();
    assert!(registry.set_handle(&other, Some("orthory".into())).is_err());
    registry
        .replace_announcements(&node, Some(&owner), vec![account("huddle")])
        .unwrap();
    registry.commit();

    registry.set_handle(&owner, Some("renamed".into())).unwrap();
    registry.commit();
    assert_eq!(registry.handle_owner("orthory"), None);
    assert_eq!(registry.handle_owner("renamed"), Some(owner.as_slice()));

    registry.set_handle(&owner, None).unwrap();
    registry.commit();
    assert_eq!(registry.handle_owner("renamed"), None);
    assert_eq!(
        registry.node_registration(&node).unwrap().announcements,
        vec![account("huddle")]
    );

    registry.set_handle(&owner, Some("final".into())).unwrap();
    registry.commit();
    assert_eq!(registry.registrations(0, 256).unwrap().len(), 1);
    assert_eq!(registry.registrations(0, 256).unwrap()[0].handle, "final");
    assert!(registry.registrations(0, 257).is_err());
}

#[test]
fn account_services_require_and_capture_the_submitting_nodes_account() {
    let mut registry = Registry::new("team#A1B2C3D4").unwrap();
    let owner = vec![7; 32];

    let declaration = account("huddle");
    assert!(
        registry
            .replace_announcements(&[1; 32], None, vec![declaration.clone()])
            .is_err()
    );
    registry
        .replace_announcements(&[1; 32], Some(&owner), vec![declaration])
        .unwrap();
    assert_eq!(
        registry.node_registration(&[1; 32]).unwrap().account_id,
        Some(owner)
    );
}

#[test]
fn account_alias_gates_lookup_not_service_registration() {
    let mut registry = Registry::new("team#A1B2C3D4").unwrap();
    let owner = vec![7; 32];
    registry
        .replace_announcements(&[1; 32], Some(&owner), vec![account("huddle")])
        .unwrap();
    registry.commit();

    let name = DuckDnsName::AccountService {
        service: "huddle".into(),
        handle: "orthory".into(),
    };
    assert!(registry.resolve_service(&name).unwrap().is_none());

    registry.set_handle(&owner, Some("orthory".into())).unwrap();
    registry.commit();
    assert_eq!(
        registry.resolve_service(&name).unwrap().unwrap().providers[0].node,
        vec![1; 32]
    );

    registry.set_handle(&owner, None).unwrap();
    registry.commit();
    assert!(registry.resolve_service(&name).unwrap().is_none());
    assert!(registry.node_registration(&[1; 32]).is_some());
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
        .replace_announcements(&[1; 32], None, vec![network("search"), network("status")])
        .unwrap();
    registry
        .replace_announcements(&[1; 32], None, vec![network("search")])
        .unwrap();
    registry.commit();

    assert_eq!(
        registry.node_registration(&[1; 32]).unwrap().announcements,
        vec![network("search")]
    );
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
    registry.set_handle(&owner, Some("orthory".into())).unwrap();
    assert_eq!(registry.root_bytes(), [0; 32]);
    registry.abort();
    assert_eq!(registry.handle_owner("orthory"), None);

    registry.set_handle(&owner, Some("orthory".into())).unwrap();
    registry.commit();
    assert_eq!(registry.handle_owner("orthory"), Some(owner.as_slice()));
    assert_ne!(registry.root_bytes(), [0; 32]);
}

#[test]
fn snapshot_round_trip_is_canonical_and_root_checked() {
    let mut first = Registry::new("team#00000000").unwrap();
    first.set_handle(&[7; 32], Some("orthory".into())).unwrap();
    first
        .replace_announcements(&[1; 32], Some(&[7; 32]), vec![account("huddle")])
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
    let mut future = snapshot.clone();
    future[0] = 2;
    assert!(restored.install(&future, root).is_err());
    assert!(restored.install(&snapshot, [9; 32]).is_err());
    assert_eq!(restored.root_bytes(), root);
}

#[test]
fn insertion_order_does_not_change_snapshot_or_root() {
    let mut first = Registry::new("team#00000000").unwrap();
    let mut second = Registry::new("team#00000000").unwrap();
    for registry in [&mut first, &mut second] {
        registry
            .set_handle(&[7; 32], Some("orthory".into()))
            .unwrap();
        registry.commit();
    }
    first
        .replace_announcements(
            &[1; 32],
            Some(&[7; 32]),
            vec![network("search"), account("huddle")],
        )
        .unwrap();
    second
        .replace_announcements(
            &[1; 32],
            Some(&[7; 32]),
            vec![account("huddle"), network("search")],
        )
        .unwrap();
    first.commit();
    second.commit();
    assert_eq!(first.snapshot(), second.snapshot());
    assert_eq!(first.root_bytes(), second.root_bytes());
}
