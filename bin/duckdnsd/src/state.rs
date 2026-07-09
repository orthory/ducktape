use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

const MAX_NAMES: usize = 16_384;
const MAX_WORKSPACE_ID_BYTES: usize = 256;
const MAX_LEASE_SECONDS: u64 = 300;

#[derive(Clone, Debug)]
pub struct ActiveWorkspace {
    pub workspace_id: String,
    pub ingress: SocketAddr,
    pub names: BTreeSet<String>,
    expires_at: Instant,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SnapshotStatus {
    Inactive,
    Active {
        workspace_id: String,
        ingress: SocketAddr,
        names: usize,
        lease_millis: u64,
    },
}

#[derive(Clone, Default)]
pub struct SharedState {
    inner: Arc<RwLock<Option<ActiveWorkspace>>>,
}

impl SharedState {
    pub fn replace(
        &self,
        workspace_id: String,
        ingress: SocketAddr,
        names: Vec<String>,
        lease_seconds: u64,
    ) -> Result<(), String> {
        validate_workspace_id(&workspace_id)?;
        if !ingress.ip().is_loopback() || ingress.port() == 0 {
            return Err("duckdnsd: active ingress must be a nonzero loopback socket".into());
        }
        if !(1..=MAX_LEASE_SECONDS).contains(&lease_seconds) {
            return Err(format!(
                "duckdnsd: lease_seconds must be 1..={MAX_LEASE_SECONDS}"
            ));
        }
        if names.len() > MAX_NAMES {
            return Err(format!(
                "duckdnsd: namespace has {} names, maximum is {MAX_NAMES}",
                names.len()
            ));
        }
        let mut canonical = BTreeSet::new();
        for name in names {
            let parsed = duckdns_core::parse_hostname(&name)?;
            if parsed.hostname() != name {
                return Err(format!(
                    "duckdnsd: registered hostname must be canonical lowercase without a trailing dot: {name:?}"
                ));
            }
            canonical.insert(name);
        }
        *self.inner.write().expect("duckdnsd state lock") = Some(ActiveWorkspace {
            workspace_id,
            ingress,
            names: canonical,
            expires_at: Instant::now() + Duration::from_secs(lease_seconds),
        });
        Ok(())
    }

    pub fn clear(&self, workspace_id: &str) -> Result<(), String> {
        validate_workspace_id(workspace_id)?;
        let mut active = self.inner.write().expect("duckdnsd state lock");
        if active
            .as_ref()
            .is_some_and(|current| current.workspace_id != workspace_id)
        {
            return Err("duckdnsd: refusing to clear a different active workspace".into());
        }
        *active = None;
        Ok(())
    }

    pub fn resolves(&self, hostname: &str) -> bool {
        let Ok(parsed) = duckdns_core::parse_hostname(hostname) else {
            return false;
        };
        let canonical = parsed.hostname();
        self.active()
            .is_some_and(|active| active.names.contains(&canonical))
    }

    pub fn ingress(&self) -> Option<SocketAddr> {
        self.active().map(|active| active.ingress)
    }

    pub fn status(&self) -> SnapshotStatus {
        let Some(active) = self.active() else {
            return SnapshotStatus::Inactive;
        };
        SnapshotStatus::Active {
            workspace_id: active.workspace_id,
            ingress: active.ingress,
            names: active.names.len(),
            lease_millis: active
                .expires_at
                .saturating_duration_since(Instant::now())
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
        }
    }

    fn active(&self) -> Option<ActiveWorkspace> {
        let active = self.inner.read().expect("duckdnsd state lock");
        active
            .as_ref()
            .filter(|active| active.expires_at > Instant::now())
            .cloned()
    }
}

fn validate_workspace_id(workspace_id: &str) -> Result<(), String> {
    if workspace_id.is_empty() || workspace_id.len() > MAX_WORKSPACE_ID_BYTES {
        return Err(format!(
            "duckdnsd: workspace id must be 1..={MAX_WORKSPACE_ID_BYTES} bytes"
        ));
    }
    if workspace_id.chars().any(char::is_control) {
        return Err("duckdnsd: workspace id must not contain control characters".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_snapshot_is_canonical_scoped_and_expires() {
        let state = SharedState::default();
        state
            .replace(
                "workspace-a".into(),
                "127.77.0.1:18080".parse().unwrap(),
                vec!["docs.team-a1b2c3d4.net.ducktape.quack".into()],
                1,
            )
            .unwrap();
        assert!(state.resolves("DOCS.TEAM-A1B2C3D4.NET.DUCKTAPE.QUACK."));
        assert!(!state.resolves("unknown.team-a1b2c3d4.net.ducktape.quack"));
        assert!(state.ingress().is_some());

        state.clear("workspace-a").unwrap();
        assert_eq!(state.status(), SnapshotStatus::Inactive);
    }

    #[test]
    fn registration_rejects_unsafe_ingress_and_noncanonical_names() {
        let state = SharedState::default();
        assert!(
            state
                .replace("a".into(), "10.0.0.1:80".parse().unwrap(), Vec::new(), 5,)
                .is_err()
        );
        assert!(
            state
                .replace(
                    "a".into(),
                    "127.0.0.1:80".parse().unwrap(),
                    vec!["Docs.team-a1b2c3d4.net.ducktape.quack".into()],
                    5,
                )
                .is_err()
        );
    }

    #[test]
    fn stale_snapshot_expires_closed() {
        let state = SharedState::default();
        state
            .replace(
                "workspace-a".into(),
                "127.0.0.1:18080".parse().unwrap(),
                vec!["docs.team-a1b2c3d4.net.ducktape.quack".into()],
                1,
            )
            .unwrap();
        std::thread::sleep(Duration::from_millis(1_050));
        assert_eq!(state.status(), SnapshotStatus::Inactive);
        assert!(!state.resolves("docs.team-a1b2c3d4.net.ducktape.quack"));
        assert!(state.ingress().is_none());
    }
}
