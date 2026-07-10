use duckdns_core::{
    DuckDnsName, RESERVED_ROOT_LABELS, derive_chain_label, node_label, parse_hostname,
    validate_handle, validate_label,
};

#[test]
fn parses_every_hostname_form_and_renders_canonically() {
    let cases = [
        (
            "orthory.duck",
            DuckDnsName::User {
                handle: "orthory".into(),
            },
        ),
        (
            "blog.orthory.duck",
            DuckDnsName::UserService {
                service: "blog".into(),
                handle: "orthory".into(),
            },
        ),
        (
            "docs.team-a1b2c3d4.net.duck",
            DuckDnsName::NetworkService {
                service: "docs".into(),
                chain: "team-a1b2c3d4".into(),
            },
        ),
        (
            "docs.n-ab12cd34ef56.team-a1b2c3d4.net.duck",
            DuckDnsName::NodeService {
                service: "docs".into(),
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
    let parsed = parse_hostname("BLOG.Orthory.Duck.").unwrap();
    assert_eq!(
        parsed,
        DuckDnsName::UserService {
            service: "blog".into(),
            handle: "orthory".into(),
        }
    );
    assert_eq!(parsed.hostname(), "blog.orthory.duck");
}

#[test]
fn derives_readable_chain_and_node_labels() {
    assert_eq!(
        derive_chain_label("Team A#A1B2C3D4").unwrap(),
        "team-a-a1b2c3d4"
    );
    assert_eq!(
        derive_chain_label("team#a1b2c3d4").unwrap(),
        "team-a1b2c3d4"
    );
    let mut node = [0u8; 32];
    node[..6].copy_from_slice(&[0xab, 0x12, 0xcd, 0x34, 0xef, 0x56]);
    assert_eq!(node_label(&node).unwrap(), "n-ab12cd34ef56");

    let long = format!("{}#deadbeef", "A".repeat(100));
    let label = derive_chain_label(&long).unwrap();
    assert_eq!(label.len(), 63);
    assert!(label.ends_with("-deadbeef"));
}

#[test]
fn one_strict_label_rule_and_reserved_handle_are_enforced() {
    for valid in ["a", "abc-123", &"z".repeat(63)] {
        validate_label(valid).unwrap();
    }
    for invalid in ["", "UPPER", "under_score", "-start", "end-", "two.words"] {
        assert!(validate_label(invalid).is_err(), "accepted {invalid:?}");
    }
    assert!(validate_label(&"z".repeat(64)).is_err());
    assert!(RESERVED_ROOT_LABELS.contains(&"net"));
    assert!(validate_handle("net").unwrap_err().contains("reserved"));
}

#[test]
fn malformed_or_ambiguous_hostnames_reject() {
    for hostname in [
        "orthory.example",
        "net.duck",
        "docs..orthory.duck",
        "docs.orthory.duck..",
        "docs.n-xyz.team-a1b2c3d4.net.duck",
        "too.many.labels.for.user.duck",
        " docs.orthory.duck",
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
    assert!(node_label(&[0; 31]).is_err());
}
