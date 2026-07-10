//! Canonical DuckDNS labels and hostname parsing. Consensus values are strict:
//! validators reject non-canonical input rather than silently rewriting it.
//! Hostname lookup is DNS-like and case-normalizes before producing those same
//! canonical values.

use std::fmt::Write as _;

use crate::{
    DUCKDNS_ZONE, DuckDnsName, MAX_LABEL_LEN, NODE_KEY_LEN, NODE_LABEL_HEX_LEN,
    RESERVED_ROOT_LABELS, ServiceAnnouncement, ServiceScope,
};

/// The one validator for user, service, and derived chain labels: lowercase
/// ASCII `[a-z0-9-]`, no leading/trailing hyphen, maximum 63 bytes.
pub fn validate_label(label: &str) -> Result<(), String> {
    if label.is_empty() {
        return Err("duckdns: DNS label must be non-empty".into());
    }
    if label.len() > MAX_LABEL_LEN {
        return Err(format!(
            "duckdns: DNS label exceeds {MAX_LABEL_LEN} bytes: {} bytes",
            label.len()
        ));
    }
    if label.starts_with('-') || label.ends_with('-') {
        return Err(format!(
            "duckdns: DNS label must not start or end with a hyphen: {label:?}"
        ));
    }
    if !label
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(format!(
            "duckdns: DNS label has invalid characters (want lowercase [a-z0-9-]): {label:?}"
        ));
    }
    Ok(())
}

pub fn validate_handle(handle: &str) -> Result<(), String> {
    validate_label(handle)?;
    if RESERVED_ROOT_LABELS.contains(&handle) {
        return Err(format!(
            "duckdns: account handle {handle:?} is reserved for a root namespace"
        ));
    }
    Ok(())
}

/// Derive the DNS chain label from `<readable-name>#<8-hex>`. Punctuation runs
/// in the readable part become a hyphen; the stem is truncated only as needed
/// to preserve the full existing salt under the DNS label limit.
pub fn derive_chain_label(chain_id: &str) -> Result<String, String> {
    let (name, salt) = chain_id.rsplit_once('#').ok_or_else(|| {
        "duckdns: chain id must end in the existing # plus eight-hex salt".to_string()
    })?;
    if salt.len() != 8 || !salt.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!(
            "duckdns: chain id salt must be exactly eight hexadecimal digits: {salt:?}"
        ));
    }

    let mut stem = String::with_capacity(name.len());
    for byte in name.bytes() {
        if byte.is_ascii_alphanumeric() {
            stem.push(byte.to_ascii_lowercase() as char);
        } else if !stem.is_empty() && !stem.ends_with('-') {
            stem.push('-');
        }
    }
    while stem.ends_with('-') {
        stem.pop();
    }
    if stem.is_empty() {
        return Err(
            "duckdns: chain readable name does not contain an ASCII letter or digit".into(),
        );
    }

    let max_stem = MAX_LABEL_LEN - 1 - salt.len();
    stem.truncate(max_stem);
    while stem.ends_with('-') {
        stem.pop();
    }
    let label = format!("{stem}-{}", salt.to_ascii_lowercase());
    validate_label(&label)?;
    Ok(label)
}

pub fn node_label(node: &[u8]) -> Result<String, String> {
    if node.len() != NODE_KEY_LEN {
        return Err(format!(
            "duckdns: node key must be {NODE_KEY_LEN} bytes, got {}",
            node.len()
        ));
    }
    let mut label = String::with_capacity(2 + NODE_LABEL_HEX_LEN);
    label.push_str("n-");
    for byte in &node[..NODE_LABEL_HEX_LEN / 2] {
        write!(&mut label, "{byte:02x}").expect("writing to a String cannot fail");
    }
    validate_label(&label)?;
    Ok(label)
}

fn validate_node_label(label: &str) -> Result<(), String> {
    validate_label(label)?;
    let Some(hex) = label.strip_prefix("n-") else {
        return Err(format!("duckdns: node label must start with n-: {label:?}"));
    };
    if hex.len() != NODE_LABEL_HEX_LEN
        || !hex
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(format!(
            "duckdns: node label must carry {NODE_LABEL_HEX_LEN} lowercase hex digits: {label:?}"
        ));
    }
    Ok(())
}

impl DuckDnsName {
    /// Validate a value decoded directly from the wire.
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::User { handle } => validate_handle(handle),
            Self::UserService { service, handle } => {
                validate_label(service)?;
                validate_handle(handle)
            }
            Self::NetworkService { service, chain } => {
                validate_label(service)?;
                validate_label(chain)
            }
            Self::NodeService {
                service,
                node,
                chain,
            } => {
                validate_label(service)?;
                validate_node_label(node)?;
                validate_label(chain)
            }
        }
    }

    pub fn hostname(&self) -> String {
        match self {
            Self::User { handle } => format!("{handle}.{DUCKDNS_ZONE}"),
            Self::UserService { service, handle } => {
                format!("{service}.{handle}.{DUCKDNS_ZONE}")
            }
            Self::NetworkService { service, chain } => {
                format!("{service}.{chain}.net.{DUCKDNS_ZONE}")
            }
            Self::NodeService {
                service,
                node,
                chain,
            } => format!("{service}.{node}.{chain}.net.{DUCKDNS_ZONE}"),
        }
    }
}

/// Parse one hostname, accepting DNS case-insensitivity and one terminal dot.
pub fn parse_hostname(hostname: &str) -> Result<DuckDnsName, String> {
    if hostname.trim() != hostname {
        return Err("duckdns: hostname must not have surrounding whitespace".into());
    }
    let without_dot = hostname.strip_suffix('.').unwrap_or(hostname);
    if without_dot.is_empty() || without_dot.ends_with('.') {
        return Err("duckdns: hostname has an empty label".into());
    }
    if !without_dot.is_ascii() {
        return Err("duckdns: hostname must be ASCII".into());
    }
    let canonical = without_dot.to_ascii_lowercase();
    let suffix = format!(".{DUCKDNS_ZONE}");
    let prefix = canonical
        .strip_suffix(&suffix)
        .ok_or_else(|| format!("duckdns: hostname is outside {DUCKDNS_ZONE}: {hostname:?}"))?;
    let labels: Vec<&str> = prefix.split('.').collect();
    if labels.iter().any(|label| label.is_empty()) {
        return Err("duckdns: hostname has an empty label".into());
    }

    let name = match labels.as_slice() {
        [handle] => DuckDnsName::User {
            handle: (*handle).to_owned(),
        },
        [service, handle] => DuckDnsName::UserService {
            service: (*service).to_owned(),
            handle: (*handle).to_owned(),
        },
        [service, chain, "net"] => DuckDnsName::NetworkService {
            service: (*service).to_owned(),
            chain: (*chain).to_owned(),
        },
        [service, node, chain, "net"] => DuckDnsName::NodeService {
            service: (*service).to_owned(),
            node: (*node).to_owned(),
            chain: (*chain).to_owned(),
        },
        _ => {
            return Err(format!(
                "duckdns: hostname has no unambiguous namespace form: {hostname:?}"
            ));
        }
    };
    name.validate()?;
    Ok(name)
}

impl ServiceAnnouncement {
    pub fn validate(&self) -> Result<(), String> {
        validate_label(&self.service)?;
        match &self.scope {
            ServiceScope::User { handle } => validate_handle(handle),
            ServiceScope::Network if self.default_homepage => {
                Err("duckdns: network service cannot be a default user homepage".into())
            }
            ServiceScope::Network => Ok(()),
        }
    }
}

impl crate::ServiceIdentity {
    pub fn validate(&self) -> Result<(), String> {
        validate_label(&self.service)?;
        match &self.scope {
            ServiceScope::User { handle } => validate_handle(handle),
            ServiceScope::Network => Ok(()),
        }
    }
}
