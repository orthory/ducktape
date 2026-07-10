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
    pub target: PublicationTarget,
}

/// A publication backend. DuckFS is the primary static-site path; loopback is
/// retained for explicitly published dynamic HTTP/WebSocket applications.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublicationTarget {
    DuckFs(DuckFsSite),
    Loopback(SocketAddr),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DuckFsSite {
    /// Canonical absolute subtree containing the site's files.
    pub prefix: String,
    /// Optional committed snapshot id. `None` follows the current head.
    pub snapshot: Option<String>,
    /// One file name used for directory requests.
    pub index: String,
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
            publication.target.validate()?;
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

impl PublicationTarget {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::DuckFs(site) => site.validate(),
            Self::Loopback(target) => validate_loopback(*target),
        }
    }
}

impl DuckFsSite {
    pub fn validate(&self) -> Result<(), String> {
        let segments = duckfs_core::paths::canonical(&self.prefix)?;
        if segments.is_empty() {
            return Err("duckdns: a DuckFS site cannot publish the filesystem root".into());
        }
        let index_segments = duckfs_core::paths::canonical(&format!("/{}", self.index))?;
        if index_segments.len() != 1 {
            return Err("duckdns: DuckFS site index must be one file name".into());
        }
        if let Some(snapshot) = &self.snapshot
            && (snapshot.len() != 64
                || !snapshot
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
        {
            return Err("duckdns: DuckFS site snapshot must be 64 lowercase hex digits".into());
        }
        Ok(())
    }
}

pub(crate) fn validate_loopback(target: SocketAddr) -> Result<(), String> {
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
            target: PublicationTarget::Loopback(target.parse().unwrap()),
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

    #[test]
    fn validates_duckfs_site_boundaries() {
        let mut p = publication("127.0.0.1:8080");
        p.target = PublicationTarget::DuckFs(DuckFsSite {
            prefix: "/shared/sites/docs".into(),
            snapshot: Some("ab".repeat(32)),
            index: "index.html".into(),
        });
        Publications::new(vec![p.clone()]).unwrap();

        let PublicationTarget::DuckFs(site) = &mut p.target else {
            unreachable!()
        };
        site.prefix = "/".into();
        assert!(Publications::new(vec![p]).is_err());
    }
}
