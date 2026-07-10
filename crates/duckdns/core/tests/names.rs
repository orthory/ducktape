use duckdns_core::{
    DuckDnsName, RESERVED_ROOT_LABELS, derive_chain_label, node_label, parse_hostname,
    validate_handle, validate_label,
};

#[test]
fn parses_every_name_form_and_renders_canonically() {
    let cases = [
        (
            "orthory.duck",
            DuckDnsName::Account {
                handle: "orthory".into(),
            },
        ),
        (
            "huddle.orthory.duck",
            DuckDnsName::AccountService {
                service: "huddle".into(),
                handle: "orthory".into(),
            },
        ),
        (
            "search.team-a1b2c3d4.net.duck",
            DuckDnsName::NetworkService {
                service: "search".into(),
                chain: "team-a1b2c3d4".into(),
            },
        ),
        (
            "search.n-ab12cd34ef56.team-a1b2c3d4.net.duck",
            DuckDnsName::NodeService {
                service: "search".into(),
                node: "n-ab12cd34ef56".into(),
                chain: "team-a1b2c3d4".into(),
            },
        ),
    ];
    for (hostname, expected) in cases {
        let parsed = parse_hostname(hostname).unwrap();
        assert_eq!(parsed, expected);
        assert_eq!(parsed.hostname(), hostname);
    }
}

#[test]
fn lookup_normalizes_dns_case_and_one_trailing_dot() {
    let parsed = parse_hostname("HUDDLE.Orthory.Duck.").unwrap();
    assert_eq!(
        parsed,
        DuckDnsName::AccountService {
            service: "huddle".into(),
            handle: "orthory".into(),
        }
    );
    assert_eq!(parsed.hostname(), "huddle.orthory.duck");
}

#[test]
fn chain_and_node_labels_are_deterministic() {
    assert_eq!(
        derive_chain_label("Research Team#A1B2C3D4").unwrap(),
        "research-team-a1b2c3d4"
    );
    let long = format!("{}#12345678", "a".repeat(100));
    let label = derive_chain_label(&long).unwrap();
    assert_eq!(label.len(), 63);
    assert!(label.ends_with("-12345678"));

    let node: Vec<u8> = (0..32).collect();
    assert_eq!(node_label(&node).unwrap(), "n-000102030405");
    assert!(node_label(&node[..31]).is_err());
}

#[test]
fn strict_labels_and_reserved_roots_are_enforced() {
    for valid in ["a", "abc-123", &"z".repeat(63)] {
        validate_label(valid).unwrap();
    }
    for invalid in ["", "-a", "a-", "A", "a_b", "a.b", "döcs"] {
        assert!(validate_label(invalid).is_err(), "accepted {invalid:?}");
    }
    assert!(validate_label(&"z".repeat(64)).is_err());
    assert!(RESERVED_ROOT_LABELS.contains(&"net"));
    assert!(validate_handle("net").unwrap_err().contains("reserved"));
}

#[test]
fn malformed_or_ambiguous_names_reject() {
    for hostname in [
        "orthory.example",
        "net.duck",
        "huddle..orthory.duck",
        "huddle.orthory.duck..",
        "search.n-xyz.team-a1b2c3d4.net.duck",
        "too.many.labels.for.account.duck",
        " huddle.orthory.duck",
        "döcs.orthory.duck",
    ] {
        assert!(parse_hostname(hostname).is_err(), "accepted {hostname:?}");
    }
    for chain_id in ["team", "team#123", "team#gggggggg", "---#12345678"] {
        assert!(
            derive_chain_label(chain_id).is_err(),
            "accepted {chain_id:?}"
        );
    }
}
