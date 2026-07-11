use duckdns::{
    DuckDnsMsg, DuckDnsName, DuckDnsQuery, DuckDnsReply, Registry, ResolvedAccount, decode_msg,
    decode_query, decode_reply, encode_msg, encode_query, encode_reply,
};

fn account_name(handle: &str) -> DuckDnsName {
    DuckDnsName {
        handle: handle.into(),
    }
}

#[test]
fn wire_round_trips_account_naming_only() {
    let message = DuckDnsMsg::SetHandle {
        handle: Some("orthory".into()),
    };
    assert_eq!(decode_msg(&encode_msg(&message)).unwrap(), message);
    assert_eq!(
        String::from_utf8(encode_msg(&message)).unwrap(),
        r#"{"set_handle":{"handle":"orthory"}}"#
    );

    let query = DuckDnsQuery::Resolve {
        name: account_name("orthory"),
    };
    assert_eq!(decode_query(&encode_query(&query)).unwrap(), query);
    let reply = DuckDnsReply::Resolved(Some(ResolvedAccount {
        account_id: vec![7; 32],
    }));
    assert_eq!(decode_reply(&encode_reply(&reply)).unwrap(), reply);

    let wire = String::from_utf8(encode_reply(&reply)).unwrap();
    for excluded in ["node", "service", "provider", "endpoint", "route", "port"] {
        assert!(
            !wire.contains(excluded),
            "{excluded} leaked into naming wire"
        );
    }
}

#[test]
fn registration_is_declarative_unique_and_resolves_only_account_id() {
    let mut registry = Registry::new();
    let owner = vec![7; 32];
    let other = vec![8; 32];

    registry.set_handle(&owner, Some("orthory".into())).unwrap();
    registry.set_handle(&owner, Some("orthory".into())).unwrap();
    assert!(registry.set_handle(&other, Some("orthory".into())).is_err());
    registry.commit();
    assert_eq!(
        registry.resolve(&account_name("orthory")).unwrap(),
        Some(ResolvedAccount {
            account_id: owner.clone(),
        })
    );

    registry.set_handle(&owner, Some("renamed".into())).unwrap();
    registry.commit();
    assert!(
        registry
            .resolve(&account_name("orthory"))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        registry.resolve(&account_name("renamed")).unwrap(),
        Some(ResolvedAccount {
            account_id: owner.clone(),
        })
    );

    registry.set_handle(&owner, None).unwrap();
    registry.commit();
    assert!(
        registry
            .resolve(&account_name("renamed"))
            .unwrap()
            .is_none()
    );
}

#[test]
fn registration_listing_is_sorted_paginated_and_bounded() {
    let mut registry = Registry::new();
    registry.set_handle(&[2], Some("beta".into())).unwrap();
    registry.set_handle(&[1], Some("alpha".into())).unwrap();
    registry.commit();

    let rows = registry.registrations(0, 256).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].handle, "alpha");
    assert_eq!(rows[1].handle, "beta");
    assert_eq!(registry.registrations(1, 1).unwrap(), vec![rows[1].clone()]);
    assert!(registry.registrations(0, 257).is_err());
}

#[test]
fn pending_changes_abort_or_commit_atomically() {
    let mut registry = Registry::new();
    let owner = vec![7; 32];
    registry.set_handle(&owner, Some("orthory".into())).unwrap();
    assert_eq!(registry.root_bytes(), [0; 32]);
    registry.abort();
    assert!(
        registry
            .resolve(&account_name("orthory"))
            .unwrap()
            .is_none()
    );

    registry.set_handle(&owner, Some("orthory".into())).unwrap();
    registry.commit();
    assert_eq!(
        registry.resolve(&account_name("orthory")).unwrap(),
        Some(ResolvedAccount { account_id: owner })
    );
    assert_ne!(registry.root_bytes(), [0; 32]);
}

#[test]
fn naming_snapshot_is_canonical_and_root_checked() {
    let mut first = Registry::new();
    first.set_handle(&[2], Some("beta".into())).unwrap();
    first.set_handle(&[1], Some("alpha".into())).unwrap();
    first.commit();

    let mut second = Registry::new();
    second.set_handle(&[1], Some("alpha".into())).unwrap();
    second.set_handle(&[2], Some("beta".into())).unwrap();
    second.commit();
    assert_eq!(first.snapshot(), second.snapshot());
    assert_eq!(first.root_bytes(), second.root_bytes());

    let snapshot = first.snapshot();
    let root = first.root_bytes();
    let mut restored = Registry::new();
    restored.install(&snapshot, root).unwrap();
    assert_eq!(restored.snapshot(), snapshot);

    let mut trailing = snapshot.clone();
    trailing.push(0);
    assert!(restored.install(&trailing, root).is_err());
    assert!(restored.install(&snapshot, [9; 32]).is_err());
}
