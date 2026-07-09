//! Active-workspace registration with the separately installed `duckdnsd`.
//!
//! The desktop never owns DNS, TLS keys, or trust-store mutation. It only
//! leases the selected workspace's canonical namespace and dedicated node
//! ingress to the helper's authenticated loopback control socket.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use duckdnsd::{ControlClient, ControlRequest, SnapshotStatus};
use serde::Serialize;

const LEASE_SECONDS: u64 = 30;
const RETRY_INTERVAL: Duration = Duration::from_secs(2);
const REFRESH_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Clone, Default)]
pub struct Registration {
    inner: Arc<RegistrationInner>,
}

#[derive(Default)]
struct RegistrationInner {
    generation: AtomicU64,
    active: Mutex<Option<String>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub installed: bool,
    pub snapshot: Option<SnapshotStatus>,
    pub error: Option<String>,
}

/// Begin (or replace) the lease refresher after a workspace node is known to
/// be running. An absent helper is an opt-in-not-installed state, so it never
/// makes workspace selection fail; the loop notices a later install/repair.
pub fn activate(
    registration: &Registration,
    workspace_id: String,
    node_http: String,
    ingress: SocketAddr,
) {
    let generation = registration.inner.generation.fetch_add(1, Ordering::SeqCst) + 1;
    let previous = registration
        .inner
        .active
        .lock()
        .expect("DuckDNS registration lock")
        .replace(workspace_id.clone());
    let registration = registration.clone();
    tauri::async_runtime::spawn(async move {
        if let Some(previous) = previous.filter(|previous| previous != &workspace_id) {
            let _ = clear_workspace(&previous).await;
        }

        loop {
            if registration.inner.generation.load(Ordering::SeqCst) != generation {
                return;
            }
            let delay = match refresh(&workspace_id, &node_http, ingress).await {
                Ok(()) => REFRESH_INTERVAL,
                Err(error) => {
                    eprintln!("DuckDNS registration for {workspace_id}: {error}");
                    RETRY_INTERVAL
                }
            };
            tokio::time::sleep(delay).await;
        }
    });
}

/// Stop refreshing and clear the helper synchronously while the native app is
/// still alive. The helper also has a short lease as a crash-safe fallback.
pub fn deactivate(registration: &Registration) -> Result<(), String> {
    registration.inner.generation.fetch_add(1, Ordering::SeqCst);
    let workspace_id = registration
        .inner
        .active
        .lock()
        .expect("DuckDNS registration lock")
        .take();
    let Some(workspace_id) = workspace_id else {
        return Ok(());
    };
    tauri::async_runtime::block_on(clear_workspace(&workspace_id))
}

#[tauri::command]
pub async fn duckdns_status() -> Status {
    match helper_client() {
        Ok(client) => match client.request(ControlRequest::Status).await {
            Ok(snapshot) => Status {
                installed: true,
                snapshot: Some(snapshot),
                error: None,
            },
            Err(error) => Status {
                installed: true,
                snapshot: None,
                error: Some(error),
            },
        },
        Err(error) => Status {
            installed: false,
            snapshot: None,
            error: Some(error),
        },
    }
}

async fn refresh(workspace_id: &str, node_http: &str, ingress: SocketAddr) -> Result<(), String> {
    let names = query_namespace(node_http).await?;
    helper_client()?
        .request(ControlRequest::Register {
            workspace_id: workspace_id.into(),
            ingress,
            names,
            lease_seconds: LEASE_SECONDS,
        })
        .await?;
    Ok(())
}

async fn clear_workspace(workspace_id: &str) -> Result<(), String> {
    helper_client()?
        .request(ControlRequest::Clear {
            workspace_id: workspace_id.into(),
        })
        .await?;
    Ok(())
}

fn helper_client() -> Result<ControlClient, String> {
    let state_dir = duckdnsd::configured_state_dir();
    ControlClient::from_token_file(
        duckdnsd::configured_control_address()?,
        &duckdnsd::control_token_path(&state_dir),
    )
}

async fn query_namespace(node_http: &str) -> Result<Vec<String>, String> {
    let response = reqwest::Client::new()
        .post(format!("{node_http}/v1/query"))
        .json(&serde_json::json!({ "target": "duckdns", "query": "namespace" }))
        .send()
        .await
        .map_err(|error| format!("query active node namespace: {error}"))?;
    let status = response.status();
    let body = response
        .bytes()
        .await
        .map_err(|error| format!("read active node namespace: {error}"))?;
    if !status.is_success() {
        return Err(format!(
            "active node namespace returned {status}: {}",
            String::from_utf8_lossy(&body).trim()
        ));
    }
    #[derive(serde::Deserialize)]
    struct NamespaceReply {
        namespace: Vec<String>,
    }
    serde_json::from_slice::<NamespaceReply>(&body)
        .map(|reply| reply.namespace)
        .map_err(|error| format!("decode active node namespace: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_generation_cancels_and_tracks_active_workspace() {
        let registration = Registration::default();
        let first = registration.inner.generation.fetch_add(1, Ordering::SeqCst) + 1;
        *registration.inner.active.lock().unwrap() = Some("a".into());
        let second = registration.inner.generation.fetch_add(1, Ordering::SeqCst) + 1;
        *registration.inner.active.lock().unwrap() = Some("b".into());
        assert_ne!(first, second);
        assert_eq!(
            registration.inner.active.lock().unwrap().as_deref(),
            Some("b")
        );
    }
}
