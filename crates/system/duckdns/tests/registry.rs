use duckdns::{
    DuckDnsMsg, DuckDnsName, DuckDnsQuery, DuckDnsReply, Registry, ResolvedAccount, decode_msg,
    decode_query, decode_reply, encode_msg, encode_query, encode_reply, parse_hostname,
};
use sha2::{Digest, Sha256};

fn account_name(handle: &str) -> DuckDnsName {
    DuckDnsName {
        handle: handle.into(),
    }
}

/// Forge the canonical bytes an OLD binary committed, before `agents` was
/// reserved. `agentx` is the same byte length, so swapping the label in place
/// leaves every length prefix in the encoding valid — the result is exactly what
/// `encode_state` would have produced for a squatted `agents` handle.
fn legacy_snapshot_holding(owner: &[u8], reserved: &str) -> Vec<u8> {
    let stand_in = format!("agent{}", "x".repeat(reserved.len() - 5));
    assert_eq!(stand_in.len(), reserved.len());
    let mut old = Registry::new();
    old.set_handle(owner, Some(stand_in.clone())).unwrap();
    old.commit();
    let bytes = old.snapshot();
    let at = bytes
        .windows(stand_in.len())
        .position(|w| w == stand_in.as_bytes())
        .expect("the stand-in label is in the canonical bytes");
    let mut forged = bytes;
    forged[at..at + reserved.len()].copy_from_slice(reserved.as_bytes());
    forged
}

/// THE FLAG-DAY GATE. `agents` joined `RESERVED_ROOT_LABELS` in this change —
/// so out there is a snapshot an old binary committed with `agents` registered:
/// precisely the squat the reservation exists to close. Reserving a label must
/// change only what ADMITS, never what DECODES. If decode enforced the policy, a
/// node on the new binary could not install duckdns state at all (no state sync)
/// and could not restore its own recovery checkpoint (it reinstalls canonical
/// snapshot bytes for non-persisting modules at boot): a permanent brick with no
/// migration path. The legacy squat decodes. It is merely INERT.
#[test]
fn a_snapshot_holding_a_newly_reserved_handle_still_installs_and_is_inert() {
    let squatter = vec![7; 32];
    let legacy = legacy_snapshot_holding(&squatter, "agents");
    let root: [u8; 32] = Sha256::digest(&legacy).into();

    let mut node = Registry::new();
    node.install(&legacy, root)
        .expect("a legacy `agents` handle must still DECODE — see validate_handle_shape");
    assert_eq!(node.snapshot(), legacy, "and re-encode canonically");
    assert_eq!(node.root_bytes(), root);
    assert_eq!(
        node.registrations(0, 8).unwrap()[0].handle,
        "agents",
        "the squat is still in state, plainly visible"
    );

    // ...but it is inert: it never resolves, and it is not a hostname.
    assert!(node.resolve(&account_name("agents")).is_err());
    assert!(parse_hostname("agents.duck").is_err());

    // nobody may claim it anew — not another account, not even the squatter.
    assert!(
        node.set_handle(&[9; 32], Some("agents".into()))
            .unwrap_err()
            .contains("reserved")
    );
    assert!(
        node.set_handle(&squatter, Some("agents".into()))
            .unwrap_err()
            .contains("reserved")
    );

    // and the squatter can still move off it — the state is not frozen.
    node.set_handle(&squatter, Some("orthory".into())).unwrap();
    node.commit();
    assert!(
        node.registrations(0, 8)
            .unwrap()
            .iter()
            .all(|r| r.handle != "agents")
    );
}

/// ADMISSION: the whole point of the reservation.
#[test]
fn a_reserved_root_label_is_refused_at_admission() {
    let mut registry = Registry::new();
    for label in duckdns::RESERVED_ROOT_LABELS {
        let err = registry
            .set_handle(&[1; 32], Some((*label).into()))
            .unwrap_err();
        assert!(err.contains("reserved"), "{label}: {err}");
    }
    assert_eq!(registry.root_bytes(), [0; 32]);
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
