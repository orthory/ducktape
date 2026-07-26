//! `ducktape service run airlock` — the standalone credential-LENDING daemon.
//!
//! The node process no longer embeds a gateway. This serves the operator's own
//! `airlock-creds` store in its own process, with its own failure domain, and
//! reaches its node exactly the way the CLI does — over localhost `/v1`.
//!
//! ## the autonomous stance
//!
//! Nothing assigns work to this daemon. Its desired state is entirely local:
//! the credentials the operator registered with `ducktape user cred add`, and
//! the grants those credentials carry on chain. There is no roles module, so
//! there is no assignment to reconcile against and no stance machinery here to
//! build one from.
//!
//! ## inbound is transport, not protocol
//!
//! This daemon binds a LOOPBACK listener and its port is published as the
//! account's signed `airlock` gateway route. Overlay traffic for
//! `airlock.<handle>.duck` lands on the NODE's `Service::Gateway` stream plane,
//! which authenticates the WireGuard peer, maps it to a caller account,
//! enforces the signed `RouteStatement` policy, and only then dials this
//! listener. That is strictly better than a directly-bound daemon: a keyless
//! service has no overlay identity to bind with, and the node's route policy is
//! a real enforcement layer a direct bind would not have.
//!
//! The invariant that matters survives either way: **no `/v1` request carries
//! that traffic, and nothing is ever pushed to this daemon over its node link.**
//! The link is used in exactly one direction, for exactly one thing — reading
//! committed state to decide a grant.
//!
//! ## TEE trust is bilateral; the node is uninvolved
//!
//! This gateway does not attest. Its trust anchor is the seal PUBLIC key on
//! consensus, which the borrower's broker pins from the credential record. An
//! enclave-attested lender is `bin/airlock-gateway`, a separate minimal binary,
//! and seal_pk pinning + quote verification stay strictly between client and
//! airlock. Neither path routes any of it through the node.
//!
//! ## it spawns nothing
//!
//! No podman, no provider set, no sandbox, no reaper, no containers. A lending
//! node is often a laptop with no container runtime at all, which is exactly
//! why the hello path must not demand one.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use gateway::{GatewayQuery, GatewayReply, credential_use_allowed};
use noded::node_link::NodeLink;

use crate::config;
use crate::services::{AIRLOCK_KIND, ServiceGrant};

/// The gateway route label this daemon publishes its loopback port under. A
/// borrower resolves `<AIRLOCK_ROUTE>.<owner-handle>.duck` to it.
pub(crate) const AIRLOCK_ROUTE: &str = "airlock";

/// How long the grant gate waits on its node.
///
/// A session-open is INTERACTIVE — a borrower's run is blocked on this read —
/// and the read is one committed query against a process on this same host, so
/// a node that cannot answer in seconds is wedged or restarting. [`NodeLink`]'s
/// own ceiling is sized for a submit that rides consensus; inheriting it would
/// make every session-open cost two minutes before failing closed.
const GRANT_QUERY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// Everything the daemon needs, resolved before any of it runs.
pub(crate) struct Airlock {
    pub(crate) grant: ServiceGrant,
    /// A [`config::ServiceConfig`], never a `Resolved`: the type has no field a
    /// secret could live in and `resolve_service` never opens `identity.key`,
    /// so this daemon HOLDING the node key is unrepresentable rather than
    /// merely unused — the same shape compute and agent take. Right here for
    /// the reason the module header gives: the lender signs nothing, submits no
    /// op, and reaches its node exactly the way the CLI does.
    pub(crate) service: config::ServiceConfig,
    pub(crate) http_base: String,
    /// where `node.toml` and `gateway-routes.json` live.
    pub(crate) workspace: std::path::PathBuf,
}

/// Serve until the process is stopped.
pub(crate) fn serve(airlock: Airlock) -> Result<(), Box<dyn std::error::Error>> {
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    runtime.block_on(run(airlock))
}

async fn run(airlock: Airlock) -> Result<(), Box<dyn std::error::Error>> {
    let Airlock { grant, service, http_base, workspace } = airlock;

    // Open the store BEFORE binding: a broken store must fail the process, not
    // leave a listener that 404s every session. Opening also mints the seal
    // keypair on first run, so `user cred add` has a stable public key to
    // publish even though no credential exists yet.
    let store = airlock_service::Store::open(&service.storage_dir)?;
    let credentials = store.len();

    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
    let port = listener.local_addr()?.port();

    let node = NodeLink::new(http_base).with_timeout(GRANT_QUERY_TIMEOUT);
    let router = store.router(committed_grant_check(node))?;

    // Register the loopback port only once the router exists: a route pointing
    // at a gateway that never came up is worse than no route at all.
    let route = gateway::RouteName::named(AIRLOCK_ROUTE);
    crate::gateway_routes::register(&workspace, route.clone(), port)
        .map_err(|error| format!("register airlock gateway route: {error}"))?;

    tracing::info!(
        target: "ducktape::gateway",
        instance = %grant.display_id(),
        credentials,
        route = AIRLOCK_ROUTE,
        "airlock daemon serving"
    );
    if credentials == 0 {
        tracing::warn!(
            target: "ducktape::gateway",
            reason = "airlock_store_empty",
            "no credentials registered yet — add one with: ducktape user cred add"
        );
    }

    // The registered port is a standing instruction to the node: reverse-proxy
    // RouteStatement-authorized overlay ingress to THIS loopback port. So the
    // entry must not outlive the process that owns it — a dead daemon's port is
    // one any local process may subsequently bind. Re-assert it on a beat (a
    // hand `gateway unbind`, or another registrar, is corrected within one) and
    // retire it on the way out.
    let refresh = tokio::spawn(refresh_route(workspace.clone(), route.clone(), port));
    let served = tokio::select! {
        served = airlock::server::serve_router(listener, router) => served.map_err(Into::into),
        () = stop_requested() => Ok(()),
    };
    refresh.abort();
    // JOIN before retiring, not just abort: cancellation only lands at the
    // task's next await, so a re-register already in flight has to finish
    // first — otherwise it restores the entry right after we removed it.
    let _ = refresh.await;
    retire_route(&workspace, &route);
    served
}

/// Resolve when the operator stops this daemon: SIGTERM is what systemd and a
/// killed shell send, SIGINT is Ctrl-C.
///
/// A handler that will not install is NOT fatal — the daemon then dies without
/// retiring its route, which is the behavior before this arm existed, not a new
/// failure. It parks so the server keeps owning the process.
async fn stop_requested() {
    use tokio::signal::unix::{SignalKind, signal};
    let (Ok(mut terminate), Ok(mut interrupt)) =
        (signal(SignalKind::terminate()), signal(SignalKind::interrupt()))
    else {
        tracing::warn!(
            target: "ducktape::gateway",
            reason = "signal_handler_install_failed",
            "the airlock gateway route will not be retired on exit"
        );
        return std::future::pending().await;
    };
    tokio::select! {
        _ = terminate.recv() => {}
        _ = interrupt.recv() => {}
    }
}

/// Re-assert the loopback route on the service heartbeat, so the port the node
/// proxies to can never disagree with the port this process serves on for
/// longer than one beat. Attempt-counted like every other forever-retry loop.
async fn refresh_route(workspace: PathBuf, route: gateway::RouteName, port: u16) -> ! {
    const LOG_EVERY: u64 = 30;
    let mut failures: u64 = 0;
    loop {
        tokio::time::sleep(crate::services::HEARTBEAT).await;
        let Err(error) = crate::gateway_routes::register(&workspace, route.clone(), port) else {
            failures = 0;
            continue;
        };
        failures += 1;
        if failures == 1 || failures.is_multiple_of(LOG_EVERY) {
            tracing::warn!(
                target: "ducktape::gateway",
                attempts = failures,
                reason = "route_refresh_failed",
                "airlock gateway route not re-registered: {error}"
            );
        }
    }
}

/// Drop the loopback route on the way out. Best effort: a workspace that has
/// become unwritable must not turn a clean stop into a failed one, but leaving
/// a live route pointing at a port nothing serves is worth a line.
fn retire_route(workspace: &Path, route: &gateway::RouteName) {
    let Err(error) = crate::gateway_routes::unregister(workspace, route) else {
        return;
    };
    tracing::warn!(
        target: "ducktape::gateway",
        reason = "route_retire_failed",
        "airlock gateway route left registered: {error}"
    );
}

/// Whether this node LENDS nothing although it was asked to: the operator's
/// credential store holds registered credentials and no airlock grant exists,
/// so no daemon will ever serve them. `Some(count)` = say so; `None` = nothing
/// to lend, or the service is granted and the daemon's own absence is what
/// `service status` reports.
///
/// A store that cannot be READ is not evidence of lending: the daemon fails
/// loudly on a broken store, and a node boot must not.
pub(crate) fn lending_without_a_grant(storage: &Path, workspace: &Path) -> Option<usize> {
    let credentials = airlock_service::load_seeds(&airlock_service::cred_store_root(storage))
        .unwrap_or_default()
        .len();
    let lends_nothing = credentials == 0;
    if lends_nothing {
        return None;
    }
    let granted = crate::services::grant_for(workspace, AIRLOCK_KIND)
        .unwrap_or(None)
        .is_some();
    if granted {
        return None;
    }
    Some(credentials)
}

/// The committed-state grant gate the owner's own gateway enforces: given a
/// credential name and the account a session claims, resolve this node's
/// committed gateway record and answer whether that account may draw on it.
///
/// This is the daemon's ONLY use of its node link, and it is a read.
fn committed_grant_check(node: NodeLink) -> airlock::server::GrantCheck {
    Arc::new(move |name: String, account: Vec<u8>| {
        let node = node.clone();
        Box::pin(async move { grant_allows(&node, &name, &account).await })
            as std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>>
    })
}

/// Fail closed: a missing record or any query failure lends nothing. The reason
/// is named at debug (this is per-session), and never carries the account.
async fn grant_allows(node: &NodeLink, name: &str, account: &[u8]) -> bool {
    let record = match committed_credential_record(node, name).await {
        Ok(Some(record)) => record,
        Ok(None) => {
            tracing::debug!(
                target: "ducktape::gateway",
                reason = "credential_record_absent",
                credential = %name,
                "airlock session refused"
            );
            return false;
        }
        Err(error) => {
            tracing::debug!(
                target: "ducktape::gateway",
                reason = "credential_record_unreadable",
                credential = %name,
                "airlock session refused: {error}"
            );
            return false;
        }
    };
    let granted = credential_use_allowed(&record, account);
    if !granted {
        tracing::debug!(
            target: "ducktape::gateway",
            reason = "credential_not_granted",
            credential = %name,
            "airlock session refused"
        );
    }
    granted
}

/// Read one credential record from this node's committed gateway-module state
/// over `/v1/query`, so the gate sees exactly what consensus committed.
async fn committed_credential_record(
    node: &NodeLink,
    name: &str,
) -> Result<Option<gateway::CredentialRecord>, String> {
    let request = gateway::encode_query(&GatewayQuery::Credential { name: name.to_string() });
    let bytes = node.query("gateway", &request).await?;
    match gateway::decode_reply(&bytes)? {
        GatewayReply::Credential(record) => Ok(record),
        other => Err(format!("gateway returned an unexpected reply: {other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// one complete credential dir under `<storage>/airlock-creds/<name>/`,
    /// exactly the shape `ducktape user cred add` writes.
    fn seed_credential(storage: &Path, name: &str) {
        let dir = airlock_service::cred_store_root(storage).join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("kind"), "codex\n").unwrap();
        std::fs::write(
            dir.join("auth.json"),
            r#"{"tokens":{"access_token":"tok"}}"#,
        )
        .unwrap();
    }

    fn grant_airlock(workspace: &Path) {
        std::fs::write(
            workspace.join(crate::services::FILE_NAME),
            "version = 1\n\n[[service]]\nkind = \"airlock\"\ninstance = \"".to_string()
                + &"ab".repeat(32)
                + "\"\nnonce = \""
                + &"cd".repeat(16)
                + "\"\ngranted_unix = 1700000000\ncapabilities = []\nscopes = []\n",
        )
        .unwrap();
    }

    /// The upgrade an operator lands in without asking: credentials registered,
    /// no grant, so nothing lends them and every other diagnostic still looks
    /// healthy. This predicate is the only thing that notices.
    #[test]
    fn a_populated_store_with_no_grant_is_the_one_state_worth_warning_about() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path();

        // nothing registered: there is nothing to lend, so nothing to say.
        assert_eq!(lending_without_a_grant(workspace, workspace), None);

        // a credential and no grant — the silent-loss shape.
        seed_credential(workspace, "owner-codex-1");
        assert_eq!(lending_without_a_grant(workspace, workspace), Some(1));

        // granted: the daemon's absence is `service status`'s job to report,
        // not this line's.
        grant_airlock(workspace);
        assert_eq!(lending_without_a_grant(workspace, workspace), None);
    }
}
