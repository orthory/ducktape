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

use airlock::server::GrantAnswer;
use gateway::{GatewayQuery, GatewayReply, credential_use_allowed};
use noded::node_link::NodeLink;

use crate::config;
use crate::gateway_routes::RouteOwner;
use crate::services::{AIRLOCK_KIND, ServiceGrant};

/// The gateway route label this daemon publishes its loopback port under. A
/// borrower resolves `<AIRLOCK_ROUTE>.<owner-handle>.duck` to it.
pub(crate) const AIRLOCK_ROUTE: &str = "airlock";

/// How long the grant gate waits on its node.
///
/// A session-open is INTERACTIVE — a borrower's run is blocked on this read —
/// and [`NodeLink`]'s own ceiling is sized for a submit that rides consensus, so
/// inheriting it would make every session-open cost two minutes.
///
/// Ten seconds does NOT mean a node answering slower is broken. `/v1/query`
/// crosses the node's command lane (unlike `/v1/status`, which deliberately does
/// not), and `http_ingress` is the 7th of 8 arms in the validator's
/// `select_biased!` — behind the 100 ms drain deadline. A catch-up stage can
/// hold the pump past ANY interactive ceiling, so this is not a health verdict
/// and no value would make it one. It is the point at which we stop blocking the
/// borrower and say [`GrantAnswer::Undetermined`] — which the borrower's
/// operator reads as "the lender's node did not answer", and retries, rather
/// than as a missing grant. Short and honestly named beats long and guessed.
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
    let Airlock { grant, service, http_base, workspace } = airlock;
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    runtime.block_on(async move {
        // ARMED before `run` publishes anything (and inside the runtime, which
        // installing a signal handler requires). See [`arm_stop_requested`].
        let stop = arm_stop_requested();
        run(grant.display_id(), service.storage_dir, http_base, workspace, stop).await
    })
}

/// Serve until `stop` resolves. Split from [`serve`] so the route's whole
/// lifetime — register, beat, abort-then-JOIN, retire — is drivable by a test
/// without a signal and without a resolved node config.
async fn run(
    instance: String,
    storage: PathBuf,
    http_base: String,
    workspace: PathBuf,
    stop: impl std::future::Future<Output = ()>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Open the store BEFORE binding: a broken store must fail the process, not
    // leave a listener that 404s every session. Opening also mints the seal
    // keypair on first run, so `user cred add` has a stable public key to
    // publish even though no credential exists yet.
    let store = airlock_service::Store::open(&storage)?;
    let credentials = store.len();

    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
    let port = listener.local_addr()?.port();

    let node = NodeLink::new(http_base).with_timeout(GRANT_QUERY_TIMEOUT);
    let router = store.router(committed_grant_check(node))?;

    // Register the loopback port only once the router exists: a route pointing
    // at a gateway that never came up is worse than no route at all.
    //
    // KNOWN RESIDUAL, not an oversight: this entry survives a death that runs no
    // code — SIGKILL, the OOM killer, `abort()`, power loss, parent death. The
    // node then keeps reverse-proxying authorized overlay ingress to a freed
    // ephemeral port, and nothing re-validates it before dialing. The borrower
    // is not misled (the node's connect-refused is a `GatewayFailure::Unavailable`
    // -> 502, which the broker names `airlock_gateway_unreachable`), so what is
    // missing is only the EVICTION, not the diagnosis. Its own PR: a stale entry
    // dropped after N consecutive connect-refusals in `gateway_plane`, or a lease
    // this beat renews. Deliberately not built here.
    let route = gateway::RouteName::named(AIRLOCK_ROUTE);
    crate::gateway_routes::register(&workspace, route.clone(), port)
        .map_err(|error| format!("register airlock gateway route: {error}"))?;

    tracing::info!(
        target: "ducktape::gateway",
        instance = %instance,
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
    // hand `gateway unbind` is corrected within one) and retire it on the way
    // out. Both are scoped to OUR port: a second daemon that took the route owns
    // it, and neither the beat nor the exit may touch a live entry that is its.
    let refresh = tokio::spawn(refresh_route(workspace.clone(), route.clone(), port));
    let served = tokio::select! {
        served = airlock::server::serve_router(listener, router) => served.map_err(Into::into),
        () = stop => Ok(()),
    };
    refresh.abort();
    // JOIN before retiring, not just abort: cancellation only lands at the
    // task's next await, so a re-register already in flight has to finish
    // first — otherwise it restores the entry right after we removed it.
    let _ = refresh.await;
    retire_route(&workspace, &route, port);
    served
}

/// Install the stop handlers NOW and return a future that waits on them: SIGTERM
/// is what systemd and a killed shell send, SIGINT is Ctrl-C.
///
/// The split matters. `signal()` installs the handler when it is CALLED; the
/// future it returns only waits. Building that future lazily inside the
/// `select!` would leave a window between publishing the route and the first
/// poll in which a SIGTERM takes its DEFAULT disposition — killing the process
/// with a live route pointing at a port anything may then bind.
///
/// A handler that will not install is NOT fatal — the daemon then dies without
/// retiring its route, which is the behavior before this arm existed, not a new
/// failure. The future parks so the server keeps owning the process.
///
/// The other half is deliberately NOT closed, and should stay open: tokio's
/// handlers remain installed after these `Signal`s drop, so a SECOND SIGTERM
/// arriving during [`retire_route`] is swallowed and SIGKILL is the operator's
/// only escape. Retiring is one file write — a hang there means an unwritable
/// workspace, which is the real problem — and a SIGTERM-count escalation is not
/// worth its complexity. Do not "finish" this.
fn arm_stop_requested() -> impl std::future::Future<Output = ()> {
    use tokio::signal::unix::{SignalKind, signal};
    let armed = (signal(SignalKind::terminate()), signal(SignalKind::interrupt()));
    async move {
        let (Ok(mut terminate), Ok(mut interrupt)) = armed else {
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
}

/// A forever-retry loop logs attempt 1, then every 30th — the counter IS the
/// diagnosis, and an unconditional warn on a 10 s beat is a log bomb.
fn beat_is_worth_a_line(beats: u64) -> bool {
    const LOG_EVERY: u64 = 30;
    beats == 1 || beats.is_multiple_of(LOG_EVERY)
}

/// Re-assert the loopback route on the service heartbeat, so the port the node
/// proxies to can never disagree with the port this process serves on for
/// longer than one beat. Scoped to OUR port: a second daemon that took the route
/// keeps it, and this one goes quiet instead of flapping the entry every beat.
async fn refresh_route(workspace: PathBuf, route: gateway::RouteName, port: u16) -> ! {
    let mut beats_not_holding: u64 = 0;
    loop {
        tokio::time::sleep(crate::services::HEARTBEAT).await;
        match crate::gateway_routes::reassert(&workspace, &route, port) {
            Ok(RouteOwner::Vacant | RouteOwner::Ours) => beats_not_holding = 0,
            Ok(RouteOwner::Foreign) => {
                beats_not_holding += 1;
                if beat_is_worth_a_line(beats_not_holding) {
                    tracing::warn!(
                        target: "ducktape::gateway",
                        attempts = beats_not_holding,
                        reason = "route_owned_by_another_daemon",
                        "another airlock daemon owns this workspace's gateway route; \
                         this one serves nothing the node will reach"
                    );
                }
            }
            Err(error) => {
                beats_not_holding += 1;
                if beat_is_worth_a_line(beats_not_holding) {
                    tracing::warn!(
                        target: "ducktape::gateway",
                        attempts = beats_not_holding,
                        reason = "route_refresh_failed",
                        "airlock gateway route not re-registered: {error}"
                    );
                }
            }
        }
    }
}

/// Drop the loopback route on the way out — OURS only. Best effort: a workspace
/// that has become unwritable must not turn a clean stop into a failed one, but
/// leaving a live route pointing at a port nothing serves is worth a line.
fn retire_route(workspace: &Path, route: &gateway::RouteName, port: u16) {
    match crate::gateway_routes::retire(workspace, route, port) {
        Ok(RouteOwner::Vacant | RouteOwner::Ours) => {}
        // once per shutdown, and it explains why the route outlives us.
        Ok(RouteOwner::Foreign) => tracing::info!(
            target: "ducktape::gateway",
            reason = "route_owned_by_another_daemon",
            "airlock gateway route left registered: another daemon owns it now"
        ),
        Err(error) => tracing::warn!(
            target: "ducktape::gateway",
            reason = "route_retire_failed",
            "airlock gateway route left registered: {error}"
        ),
    }
}

/// Whether this node LENDS nothing although it was asked to: the operator's
/// credential store holds registered credentials and no airlock grant exists,
/// so no daemon will ever serve them. `Some(count)` = say so; `None` = nothing
/// to lend, or the service is granted and the daemon's own absence is what
/// `service status` reports.
///
/// A store that cannot be READ is not evidence of lending, and neither is a
/// grant file that cannot be read evidence of an absent grant: the daemon fails
/// loudly on either, and a node boot must not guess. This COUNTS the store — it
/// never opens a credential, because the node process has no business
/// materializing a lending token to produce a number, and no business logging an
/// operator-chosen credential name.
pub(crate) fn lending_without_a_grant(storage: &Path, workspace: &Path) -> Option<usize> {
    let credentials =
        airlock_service::count_credentials(&airlock_service::cred_store_root(storage));
    let lends_nothing = credentials == 0;
    if lends_nothing {
        return None;
    }
    let Ok(grant) = crate::services::grant_for(workspace, AIRLOCK_KIND) else {
        return None;
    };
    if grant.is_some() {
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
        Box::pin(async move { grant_answer(&node, &name, &account).await })
            as std::pin::Pin<Box<dyn std::future::Future<Output = GrantAnswer> + Send>>
    })
}

/// Fail closed, but say WHICH closed door. A committed record that does not
/// admit the account is a refusal the borrower's operator can act on; a node
/// that did not answer is not — reporting it as a refusal sends them to add a
/// grant that already exists, which is the exact bug this taxonomy replaces.
///
/// The reason is named at debug (this is per-session) and never carries the
/// account.
async fn grant_answer(node: &NodeLink, name: &str, account: &[u8]) -> GrantAnswer {
    let record = match committed_credential_record(node, name).await {
        Ok(Some(record)) => record,
        Ok(None) => {
            tracing::debug!(
                target: "ducktape::gateway",
                reason = "credential_record_absent",
                credential = %name,
                "airlock session refused"
            );
            return GrantAnswer::Refused;
        }
        // The node is the AUTHORITY here, and it did not answer: a link timeout
        // ([`GRANT_QUERY_TIMEOUT`]), a refused connection while it restarts, a
        // resident whose `serving` is still None, a reply that would not decode.
        // Nothing is known about the grant, so nothing is claimed about it.
        Err(error) => {
            tracing::debug!(
                target: "ducktape::gateway",
                reason = "grant_authority_unavailable",
                credential = %name,
                "airlock session not decided: {error}"
            );
            return GrantAnswer::Undetermined;
        }
    };
    if !credential_use_allowed(&record, account) {
        tracing::debug!(
            target: "ducktape::gateway",
            reason = "credential_not_granted",
            credential = %name,
            "airlock session refused"
        );
        return GrantAnswer::Refused;
    }
    GrantAnswer::Granted
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
    ///
    /// Storage and workspace are DIFFERENT dirs, as they are in the real shape
    /// (`config::resolve`): passing one dir for both would let the two arguments
    /// be transposed and the test still pass.
    #[test]
    fn a_populated_store_with_no_grant_is_the_one_state_worth_warning_about() {
        let dir = tempfile::tempdir().unwrap();
        let storage = dir.path().join("storage");
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&storage).unwrap();
        std::fs::create_dir_all(&workspace).unwrap();

        // nothing registered: there is nothing to lend, so nothing to say.
        assert_eq!(lending_without_a_grant(&storage, &workspace), None);

        // a credential and no grant — the silent-loss shape.
        seed_credential(&storage, "owner-codex-1");
        assert_eq!(lending_without_a_grant(&storage, &workspace), Some(1));

        // granted: the daemon's absence is `service status`'s job to report,
        // not this line's.
        grant_airlock(&workspace);
        assert_eq!(lending_without_a_grant(&storage, &workspace), None);
    }

    /// Both documented silences. The warn claims an operator's credentials are
    /// going unlent, so it must fire only on EVIDENCE: a store it could not read
    /// proves no lending, and a grant file it could not parse proves no missing
    /// grant. Guessing either way is a false alarm on a healthy node.
    #[test]
    fn a_state_it_cannot_read_is_never_warned_about() {
        let dir = tempfile::tempdir().unwrap();
        let storage = dir.path().join("storage");
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();

        // no store at all (never `cred add`ed): nothing is registered.
        assert_eq!(lending_without_a_grant(&storage, &workspace), None);

        // a store, and a services.toml that does not parse. The grant may well
        // exist; we cannot tell, so we say nothing.
        seed_credential(&storage, "owner-codex-1");
        std::fs::write(workspace.join(crate::services::FILE_NAME), "this is not toml {{{").unwrap();
        assert_eq!(lending_without_a_grant(&storage, &workspace), None);
    }

    /// Arming installs a real signal handler, which PANICS outside a reactor
    /// rather than erroring — so "inside the runtime, before the route is
    /// published" is a constraint a refactor could silently break into a
    /// production-only crash. This is the call site [`serve`] uses.
    #[test]
    fn stop_signals_arm_inside_the_daemon_runtime() {
        let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
        runtime.block_on(async {
            let _armed = arm_stop_requested();
        });
    }

    /// The registered port is a standing instruction to the node's reverse
    /// proxy, so its lifetime must be exactly the daemon's: published before the
    /// listener serves, gone once the daemon stops. This drives the REAL run
    /// loop — the select!, the beat task's abort-then-JOIN, and the retire —
    /// with an injected stop in place of SIGTERM.
    #[test]
    fn the_route_lives_exactly_as_long_as_the_daemon() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("workspace");
        let storage = dir.path().join("storage");
        let route = gateway::RouteName::named(AIRLOCK_ROUTE);

        // The stop future's FIRST poll happens inside the select!, i.e. after
        // the route is registered and the router is being served. That is the
        // daemon's own "serving" event, so this observation waits on the system
        // rather than on a clock.
        let observed = Arc::new(std::sync::Mutex::new(None));
        let seen = observed.clone();
        let peek = (workspace.clone(), route.clone());
        let stop = async move {
            *seen.lock().unwrap() = crate::gateway_routes::load(&peek.0).unwrap().port(&peek.1);
        };

        let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
        runtime
            .block_on(run(
                "airlock#test".into(),
                storage,
                // never dialed: no session is opened in this test.
                "http://127.0.0.1:1".into(),
                workspace.clone(),
                stop,
            ))
            .expect("a stopped daemon exits cleanly");

        let served_on = observed
            .lock()
            .unwrap()
            .expect("the route must be published before the daemon serves");
        assert_ne!(served_on, 0, "a registered route names a real loopback port");
        assert_eq!(
            crate::gateway_routes::load(&workspace).unwrap().port(&route),
            None,
            "a stopped daemon leaves no route pointing at a port any process may now bind"
        );
    }
}
