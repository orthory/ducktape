use duckdns_core::{
    DuckDnsMsg, DuckDnsName, DuckDnsQuery, DuckDnsReply, Registry, ServiceAnnouncement,
    ServiceIdentity, ServiceScope, decode_msg, decode_query, decode_reply, decode_service_identity,
    encode_msg, encode_query, encode_reply, encode_service_identity,
};

fn node(byte: u8) -> Vec<u8> {
    vec![byte; 32]
}

fn user(handle: &str, service: &str, default_homepage: bool) -> ServiceAnnouncement {
    ServiceAnnouncement {
        scope: ServiceScope::User {
            handle: handle.into(),
        },
        service: service.into(),
        default_homepage,
        allow_cross_site: false,
    }
}

fn network(service: &str) -> ServiceAnnouncement {
    ServiceAnnouncement {
        scope: ServiceScope::Network,
        service: service.into(),
        default_homepage: false,
        allow_cross_site: false,
    }
}

#[test]
fn wire_codecs_round_trip() {
    let message = DuckDnsMsg::ReplaceAnnouncements {
        announcements: vec![network("docs")],
    };
    assert_eq!(decode_msg(&encode_msg(&message)).unwrap(), message);
    let query = DuckDnsQuery::Resolve {
        name: DuckDnsName::User {
            handle: "orthory".into(),
        },
    };
    assert_eq!(decode_query(&encode_query(&query)).unwrap(), query);
    let reply = DuckDnsReply::Resolved(None);
    assert_eq!(decode_reply(&encode_reply(&reply)).unwrap(), reply);
    let identity = ServiceIdentity {
        scope: ServiceScope::Network,
        service: "docs".into(),
    };
    assert_eq!(
        decode_service_identity(&encode_service_identity(&identity).unwrap()).unwrap(),
        identity
    );
}

#[test]
fn handles_are_unique_account_owned_and_release_cleans_services() {
    let mut registry = Registry::new("team#a1b2c3d4").unwrap();
    registry
        .claim_handle(b"account-a", "orthory".into())
        .unwrap();
    registry
        .claim_handle(b"account-a", "orthory".into())
        .unwrap();
    assert!(
        registry
            .claim_handle(b"account-b", "orthory".into())
            .unwrap_err()
            .contains("already claimed")
    );
    assert!(
        registry
            .release_handle(b"account-b", "orthory")
            .unwrap_err()
            .contains("another account")
    );

    registry
        .replace_announcements(
            &node(1),
            Some(b"account-a"),
            vec![user("orthory", "home", true)],
        )
        .unwrap();
    registry.release_handle(b"account-a", "orthory").unwrap();
    assert!(registry.handle_owner("orthory").is_none());
    assert!(registry.node_announcements(&node(1)).is_empty());
}

#[test]
fn user_publication_requires_the_claim_owner() {
    let mut registry = Registry::new("team#a1b2c3d4").unwrap();
    registry.claim_handle(b"owner", "orthory".into()).unwrap();
    assert!(
        registry
            .replace_announcements(&node(1), None, vec![user("orthory", "home", true)])
            .unwrap_err()
            .contains("bound account")
    );
    assert!(
        registry
            .replace_announcements(
                &node(1),
                Some(b"intruder"),
                vec![user("orthory", "home", true)]
            )
            .unwrap_err()
            .contains("does not own")
    );
    registry
        .replace_announcements(
            &node(1),
            Some(b"owner"),
            vec![user("orthory", "home", true)],
        )
        .unwrap();
}

#[test]
fn logical_services_pool_and_node_names_pin_one_provider() {
    let mut registry = Registry::new("team#a1b2c3d4").unwrap();
    registry
        .replace_announcements(&node(1), None, vec![network("docs")])
        .unwrap();
    registry
        .replace_announcements(&node(2), None, vec![network("docs")])
        .unwrap();

    let logical = registry
        .resolve(&DuckDnsName::NetworkService {
            service: "docs".into(),
            chain: "team-a1b2c3d4".into(),
        })
        .unwrap()
        .unwrap();
    assert_eq!(logical.providers.len(), 2);
    assert_eq!(logical.providers[0].node, node(1));
    assert_eq!(logical.providers[1].node, node(2));

    let qualified = registry
        .resolve(&DuckDnsName::NodeService {
            service: "docs".into(),
            node: "n-010101010101".into(),
            chain: "team-a1b2c3d4".into(),
        })
        .unwrap()
        .unwrap();
    assert_eq!(qualified.providers.len(), 1);
    assert_eq!(qualified.providers[0].node, node(1));
    assert!(
        registry
            .resolve(&DuckDnsName::NetworkService {
                service: "docs".into(),
                chain: "other-deadbeef".into(),
            })
            .unwrap()
            .is_none()
    );
}

#[test]
fn user_default_and_service_names_share_one_identity() {
    let mut registry = Registry::new("team#a1b2c3d4").unwrap();
    registry.claim_handle(b"owner", "orthory".into()).unwrap();
    registry
        .replace_announcements(
            &node(1),
            Some(b"owner"),
            vec![user("orthory", "home", true)],
        )
        .unwrap();
    let homepage = registry
        .resolve(&DuckDnsName::User {
            handle: "orthory".into(),
        })
        .unwrap()
        .unwrap();
    let explicit = registry
        .resolve(&DuckDnsName::UserService {
            service: "home".into(),
            handle: "orthory".into(),
        })
        .unwrap()
        .unwrap();
    assert_eq!(homepage.identity, explicit.identity);
    assert_eq!(homepage.providers, explicit.providers);
}

#[test]
fn namespace_enumerates_every_canonical_published_name_once() {
    let mut registry = Registry::new("team#a1b2c3d4").unwrap();
    registry.claim_handle(b"owner", "orthory".into()).unwrap();
    registry
        .replace_announcements(
            &node(1),
            Some(b"owner"),
            vec![user("orthory", "home", true), network("docs")],
        )
        .unwrap();
    registry
        .replace_announcements(&node(2), None, vec![network("docs")])
        .unwrap();

    let hostnames: Vec<String> = registry
        .namespace_names()
        .into_iter()
        .map(|name| name.hostname())
        .collect();
    assert_eq!(
        hostnames,
        vec![
            "orthory.ducktape.quack",
            "home.orthory.ducktape.quack",
            "docs.team-a1b2c3d4.net.ducktape.quack",
            "docs.n-010101010101.team-a1b2c3d4.net.ducktape.quack",
            "docs.n-020202020202.team-a1b2c3d4.net.ducktape.quack",
        ]
    );
}

#[test]
fn provider_policy_and_default_homepage_are_service_wide() {
    let mut registry = Registry::new("team#a1b2c3d4").unwrap();
    registry.claim_handle(b"owner", "orthory".into()).unwrap();
    registry
        .replace_announcements(
            &node(1),
            Some(b"owner"),
            vec![user("orthory", "home", true)],
        )
        .unwrap();

    let mut conflicting_policy = user("orthory", "home", true);
    conflicting_policy.allow_cross_site = true;
    assert!(
        registry
            .replace_announcements(&node(2), Some(b"owner"), vec![conflicting_policy])
            .unwrap_err()
            .contains("disagree")
    );
    assert!(
        registry
            .replace_announcements(
                &node(2),
                Some(b"owner"),
                vec![user("orthory", "other", true)]
            )
            .unwrap_err()
            .contains("more than one default")
    );
}

#[test]
fn declarative_replacement_removes_stale_announcements() {
    let mut registry = Registry::new("team#a1b2c3d4").unwrap();
    registry
        .replace_announcements(&node(1), None, vec![network("docs"), network("status")])
        .unwrap();
    registry
        .replace_announcements(&node(1), None, vec![network("docs")])
        .unwrap();
    assert_eq!(registry.node_announcements(&node(1)), vec![network("docs")]);
    assert!(
        registry
            .resolve(&DuckDnsName::NetworkService {
                service: "status".into(),
                chain: "team-a1b2c3d4".into(),
            })
            .unwrap()
            .is_none()
    );
    registry
        .replace_announcements(&node(1), None, vec![])
        .unwrap();
    assert!(registry.node_announcements(&node(1)).is_empty());
}

#[test]
fn commit_abort_and_snapshot_are_deterministic() {
    let mut registry = Registry::new("team#a1b2c3d4").unwrap();
    assert_eq!(registry.root_bytes(), [0; 32]);
    registry.claim_handle(b"owner", "orthory".into()).unwrap();
    assert_eq!(registry.root_bytes(), [0; 32], "pending is not committed");
    registry.abort();
    assert!(registry.handle_owner("orthory").is_none());

    registry.claim_handle(b"owner", "orthory".into()).unwrap();
    registry
        .replace_announcements(
            &node(1),
            Some(b"owner"),
            vec![user("orthory", "home", true)],
        )
        .unwrap();
    registry.commit();
    let root = registry.root_bytes();
    let snapshot = registry.snapshot();

    let mut restored = Registry::new("team#a1b2c3d4").unwrap();
    restored.install(&snapshot, root).unwrap();
    assert_eq!(restored.root_bytes(), root);
    assert_eq!(restored.snapshot(), snapshot);

    let mut trailing = snapshot.clone();
    trailing.push(0);
    assert!(restored.install(&trailing, root).is_err());
    assert_eq!(restored.root_bytes(), root, "failed install is atomic");
    assert!(restored.install(&snapshot, [9; 32]).is_err());
    assert_eq!(restored.root_bytes(), root, "root mismatch is atomic");
}

#[test]
fn root_depends_on_final_state_not_write_history() {
    let mut first = Registry::new("team#a1b2c3d4").unwrap();
    first.claim_handle(b"a", "alpha".into()).unwrap();
    first.claim_handle(b"b", "beta".into()).unwrap();
    first
        .replace_announcements(&node(1), None, vec![network("docs")])
        .unwrap();
    first.commit();

    let mut second = Registry::new("team#a1b2c3d4").unwrap();
    second.claim_handle(b"b", "beta".into()).unwrap();
    second
        .replace_announcements(&node(1), None, vec![network("docs")])
        .unwrap();
    second.claim_handle(b"a", "alpha".into()).unwrap();
    second.commit();

    assert_eq!(first.root_bytes(), second.root_bytes());
    assert_eq!(first.snapshot(), second.snapshot());
}
