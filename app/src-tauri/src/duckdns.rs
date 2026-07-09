//! Active-workspace registration with the separately installed `duckdnsd`.
//!
//! The desktop never owns DNS, TLS keys, or trust-store mutation. It only
//! leases the selected workspace's canonical namespace and dedicated node
//! ingress to the helper's authenticated loopback control socket.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use duckdnsd::{ControlClient, ControlRequest, SnapshotStatus};
use serde::Serialize;
use tauri::Manager as _;

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
    active: Mutex<Option<ActiveRegistration>>,
    operation: tokio::sync::Mutex<()>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ActiveRegistration {
    workspace_id: String,
    token_path: PathBuf,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub installed: bool,
    pub installation: Option<serde_json::Value>,
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
    token_path: PathBuf,
) {
    let generation = registration.inner.generation.fetch_add(1, Ordering::SeqCst) + 1;
    let previous = registration
        .inner
        .active
        .lock()
        .expect("DuckDNS registration lock")
        .replace(ActiveRegistration {
            workspace_id: workspace_id.clone(),
            token_path: token_path.clone(),
        });
    let registration = registration.clone();
    tauri::async_runtime::spawn(async move {
        if let Some(previous) = previous.filter(|previous| previous.workspace_id != workspace_id) {
            let _operation = registration.inner.operation.lock().await;
            if registration.inner.generation.load(Ordering::SeqCst) != generation {
                return;
            }
            let _ = clear_workspace(&previous.workspace_id, &previous.token_path).await;
        }

        loop {
            if registration.inner.generation.load(Ordering::SeqCst) != generation {
                return;
            }
            let operation = registration.inner.operation.lock().await;
            if registration.inner.generation.load(Ordering::SeqCst) != generation {
                return;
            }
            let result = refresh(&workspace_id, &node_http, ingress, &token_path).await;
            drop(operation);
            let delay = match result {
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
    let Some(active) = take_active(registration) else {
        return Ok(());
    };
    tauri::async_runtime::block_on(async {
        let _operation = registration.inner.operation.lock().await;
        clear_workspace(&active.workspace_id, &active.token_path).await
    })
}

pub(crate) async fn deactivate_async(registration: &Registration) -> Result<(), String> {
    let Some(active) = take_active(registration) else {
        return Ok(());
    };
    let _operation = registration.inner.operation.lock().await;
    clear_workspace(&active.workspace_id, &active.token_path).await
}

fn take_active(registration: &Registration) -> Option<ActiveRegistration> {
    registration.inner.generation.fetch_add(1, Ordering::SeqCst);
    registration
        .inner
        .active
        .lock()
        .expect("DuckDNS registration lock")
        .take()
}

#[tauri::command]
pub async fn duckdns_status(app: tauri::AppHandle) -> Status {
    let installation = crate::duckdns_install::helper_installation_status().ok();
    let token_path = match client_token_path(&app) {
        Ok(path) => path,
        Err(error) => {
            return Status {
                installed: installation
                    .as_ref()
                    .and_then(|value| value.get("installed"))
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
                installation,
                snapshot: None,
                error: Some(error),
            };
        }
    };
    match helper_client(&token_path) {
        Ok(client) => match client.request(ControlRequest::Status).await {
            Ok(snapshot) => Status {
                installed: true,
                installation,
                snapshot: Some(snapshot),
                error: None,
            },
            Err(error) => Status {
                installed: true,
                installation,
                snapshot: None,
                error: Some(error),
            },
        },
        Err(error) => Status {
            installed: installation
                .as_ref()
                .and_then(|value| value.get("installed"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            installation,
            snapshot: None,
            error: Some(error),
        },
    }
}

async fn refresh(
    workspace_id: &str,
    node_http: &str,
    ingress: SocketAddr,
    token_path: &Path,
) -> Result<(), String> {
    let names = query_namespace(node_http).await?;
    helper_client(token_path)?
        .request(ControlRequest::Register {
            workspace_id: workspace_id.into(),
            ingress,
            names,
            lease_seconds: LEASE_SECONDS,
        })
        .await?;
    Ok(())
}

async fn clear_workspace(workspace_id: &str, token_path: &Path) -> Result<(), String> {
    helper_client(token_path)?
        .request(ControlRequest::Clear {
            workspace_id: workspace_id.into(),
        })
        .await?;
    Ok(())
}

fn helper_client(token_path: &Path) -> Result<ControlClient, String> {
    ControlClient::from_token_file(duckdnsd::configured_control_address()?, token_path)
}

fn client_state_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    if std::env::var_os("DUCKTAPE_DUCKDNS_STATE").is_some() {
        return Ok(duckdnsd::configured_state_dir());
    }
    app.path()
        .app_data_dir()
        .map(|path| path.join("duckdnsd-client"))
        .map_err(|error| format!("resolve DuckDNS client state: {error}"))
}

pub fn client_token_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(duckdnsd::control_token_path(&client_state_dir(app)?))
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
        *registration.inner.active.lock().unwrap() = Some(ActiveRegistration {
            workspace_id: "a".into(),
            token_path: "/tmp/a".into(),
        });
        let second = registration.inner.generation.fetch_add(1, Ordering::SeqCst) + 1;
        *registration.inner.active.lock().unwrap() = Some(ActiveRegistration {
            workspace_id: "b".into(),
            token_path: "/tmp/b".into(),
        });
        assert_ne!(first, second);
        assert_eq!(
            registration
                .inner
                .active
                .lock()
                .unwrap()
                .as_ref()
                .map(|active| active.workspace_id.as_str()),
            Some("b"),
        );
    }
}
