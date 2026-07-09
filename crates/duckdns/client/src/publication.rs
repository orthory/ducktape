use std::collections::BTreeMap;
use std::net::SocketAddr;

use duckdns_core::{
    MAX_ANNOUNCEMENTS_PER_NODE, ServiceAnnouncement, ServiceIdentity, ServiceScope,
};

/// One explicit node-local target. `announcement` may replicate; `target`
/// never leaves this process.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Publication {
    pub announcement: ServiceAnnouncement,
    pub target: SocketAddr,
}

/// Validated allowlist keyed by stable service identity.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Publications {
    entries: BTreeMap<ServiceIdentity, Publication>,
}

impl Publications {
    pub fn new(publications: Vec<Publication>) -> Result<Self, String> {
        if publications.len() > MAX_ANNOUNCEMENTS_PER_NODE {
            return Err(format!(
                "duckdns: {} local publications exceed the {MAX_ANNOUNCEMENTS_PER_NODE} cap",
                publications.len()
            ));
        }
        let mut entries = BTreeMap::new();
        let mut homepages = BTreeMap::<String, String>::new();
        for publication in publications {
            publication.announcement.validate()?;
            validate_target(publication.target)?;
            let identity = ServiceIdentity {
                scope: publication.announcement.scope.clone(),
                service: publication.announcement.service.clone(),
            };
            if entries
                .insert(identity.clone(), publication.clone())
                .is_some()
            {
                return Err(format!(
                    "duckdns: local service {identity:?} is declared more than once"
                ));
            }
            if publication.announcement.default_homepage
                && let ServiceScope::User { handle } = &publication.announcement.scope
                && homepages
                    .insert(handle.clone(), publication.announcement.service.clone())
                    .is_some_and(|old| old != publication.announcement.service)
            {
                return Err(format!(
                    "duckdns: handle {handle:?} declares more than one local default homepage"
                ));
            }
        }
        Ok(Self { entries })
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn get(&self, identity: &ServiceIdentity) -> Option<&Publication> {
        self.entries.get(identity)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Publication> {
        self.entries.values()
    }

    pub fn announcements(&self) -> Vec<ServiceAnnouncement> {
        self.entries
            .values()
            .map(|publication| publication.announcement.clone())
            .collect()
    }
}

pub(crate) fn validate_target(target: SocketAddr) -> Result<(), String> {
    if !target.ip().is_loopback() {
        return Err(format!(
            "duckdns: target {target} is not loopback; only 127.0.0.0/8 and ::1 are allowed"
        ));
    }
    if target.port() == 0 {
        return Err(format!("duckdns: target {target} uses invalid port 0"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn publication(target: &str) -> Publication {
        Publication {
            announcement: ServiceAnnouncement {
                scope: ServiceScope::Network,
                service: "docs".into(),
                default_homepage: false,
                allow_cross_site: false,
            },
            target: target.parse().unwrap(),
        }
    }

    #[test]
    fn accepts_all_ipv4_loopback_and_ipv6_loopback_only() {
        Publications::new(vec![publication("127.42.0.1:8080")]).unwrap();
        Publications::new(vec![publication("[::1]:8080")]).unwrap();
        assert!(Publications::new(vec![publication("10.0.0.1:8080")]).is_err());
        assert!(Publications::new(vec![publication("[::2]:8080")]).is_err());
        assert!(Publications::new(vec![publication("127.0.0.1:0")]).is_err());
    }

    #[test]
    fn service_identity_is_a_unique_dial_allowlist_key() {
        let p = publication("127.0.0.1:8080");
        assert!(Publications::new(vec![p.clone(), p]).is_err());
    }
}
