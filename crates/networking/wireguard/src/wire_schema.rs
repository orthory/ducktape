//! Borsh SCHEMA descriptions for the socket-address types borsh serializes
//! but does not describe. A schema is what pins a wire layout across
//! upgrades, so every field a transported struct carries must declare
//! one; these mirror borsh's own encoding exactly — a socket address is a
//! one-byte variant tag, the IP octets, then the port.

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, Ipv6Addr};

use borsh::BorshSchema;
use borsh::schema::{Declaration, Definition, Fields};

/// `std::net::SocketAddr`, for `#[borsh(schema(with_funcs(..)))]`.
pub mod socket_addr {
    use super::*;

    pub fn declaration() -> Declaration {
        "SocketAddr".into()
    }

    pub fn definitions(definitions: &mut BTreeMap<Declaration, Definition>) {
        borsh::schema::add_definition(
            declaration(),
            Definition::Enum {
                tag_width: 1,
                variants: vec![
                    (0, "V4".into(), "SocketAddrV4".into()),
                    (1, "V6".into(), "SocketAddrV6".into()),
                ],
            },
            definitions,
        );
        borsh::schema::add_definition(
            "SocketAddrV4".into(),
            Definition::Struct {
                fields: Fields::NamedFields(vec![
                    ("ip".into(), <Ipv4Addr as BorshSchema>::declaration()),
                    ("port".into(), <u16 as BorshSchema>::declaration()),
                ]),
            },
            definitions,
        );
        borsh::schema::add_definition(
            "SocketAddrV6".into(),
            Definition::Struct {
                fields: Fields::NamedFields(vec![
                    ("ip".into(), <Ipv6Addr as BorshSchema>::declaration()),
                    ("port".into(), <u16 as BorshSchema>::declaration()),
                ]),
            },
            definitions,
        );
        <Ipv4Addr as BorshSchema>::add_definitions_recursively(definitions);
        <Ipv6Addr as BorshSchema>::add_definitions_recursively(definitions);
        <u16 as BorshSchema>::add_definitions_recursively(definitions);
    }
}

/// `Option<std::net::SocketAddr>`, laid out as borsh lays out every
/// `Option`: a one-byte tag, `None` = 0, `Some` = 1.
pub mod option_socket_addr {
    use super::*;

    pub fn declaration() -> Declaration {
        format!("Option<{}>", socket_addr::declaration())
    }

    pub fn definitions(definitions: &mut BTreeMap<Declaration, Definition>) {
        borsh::schema::add_definition(
            declaration(),
            Definition::Enum {
                tag_width: 1,
                variants: vec![
                    (0, "None".into(), <() as BorshSchema>::declaration()),
                    (1, "Some".into(), socket_addr::declaration()),
                ],
            },
            definitions,
        );
        <() as BorshSchema>::add_definitions_recursively(definitions);
        socket_addr::definitions(definitions);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    /// The described layout must be the encoded one: a V4 address is
    /// tag 0, four octets, a little-endian port; V6 is tag 1, sixteen
    /// octets, the port.
    #[test]
    fn declared_layout_matches_the_encoding() {
        let v4: SocketAddr = "8.8.8.8:443".parse().unwrap();
        assert_eq!(borsh::to_vec(&v4).unwrap(), [0, 8, 8, 8, 8, 0xbb, 0x01]);
        let v6: SocketAddr = "[::1]:1".parse().unwrap();
        let mut expected = vec![1u8];
        expected.extend_from_slice(&[0; 15]);
        expected.extend_from_slice(&[1, 1, 0]);
        assert_eq!(borsh::to_vec(&v6).unwrap(), expected);

        let mut definitions = BTreeMap::new();
        option_socket_addr::definitions(&mut definitions);
        assert!(definitions.contains_key("Option<SocketAddr>"));
        assert!(definitions.contains_key("SocketAddr"));
        assert!(definitions.contains_key("SocketAddrV6"));
    }
}
