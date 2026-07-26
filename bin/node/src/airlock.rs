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

use std::sync::Arc;

use gateway::{GatewayQuery, GatewayReply, credential_use_allowed};
use noded::node_link::NodeLink;

use crate::config;
use crate::services::ServiceGrant;

/// The gateway route label this daemon publishes its loopback port under. A
/// borrower resolves `<AIRLOCK_ROUTE>.<owner-handle>.duck` to it.
pub(crate) const AIRLOCK_ROUTE: &str = "airlock";

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

    let node = NodeLink::new(http_base);
    let router = store.router(committed_grant_check(node))?;

    // Register the loopback port only once the router exists: a route pointing
    // at a gateway that never came up is worse than no route at all.
    crate::gateway_routes::register(&workspace, gateway::RouteName::named(AIRLOCK_ROUTE), port)
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

    airlock::server::serve_router(listener, router).await?;
    Ok(())
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
