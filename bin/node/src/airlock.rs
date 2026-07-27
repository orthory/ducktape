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

use airlock::server::{GrantAnswer, GrantQuestion};
use airlock::wire::WorkRef;
use gateway::{CredentialRecord, GatewayQuery, GatewayReply, credential_use_allowed};
use noded::node_link::NodeLink;
use saga::{SagaOrigin, SagaQuery, SagaReply, SagaView};

use crate::config;
use crate::gateway_routes::RouteOwner;
use crate::services::{AIRLOCK_KIND, ServiceGrant};
use crate::work_admission::{CommittedReader, account_of_node};

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
    serve_until(
        grant.display_id(),
        service.storage_dir,
        http_base,
        workspace,
        std::future::pending(),
    )
}

/// Serve until either a stop signal or `also_stop` resolves. [`serve`] is this
/// with the config unpacked and `also_stop` never resolving, so a test driving
/// this holds the real arming-before-publication ordering instead of a replica
/// of it — and a replica is what an earlier guard was: hoisting the arming out
/// of `block_on`, the production-only panic it exists to prevent, left that test
/// green. The arming itself now belongs to
/// [`crate::services::serve_until_stopped`], which every daemon enters through.
fn serve_until(
    instance: String,
    storage: PathBuf,
    http_base: String,
    workspace: PathBuf,
    also_stop: impl std::future::Future<Output = ()> + 'static,
) -> Result<(), Box<dyn std::error::Error>> {
    crate::services::serve_until_stopped(also_stop, |stop| {
        run(instance, storage, http_base, workspace, stop)
    })
}

/// Serve until `stop` resolves. Split from [`serve_until`] so the route's
/// lifetime — published before the listener is served, retired once the daemon
/// stops — is drivable without a resolved node config.
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

    // A daemon start is the natural cadence to reap temps a killed writer left
    // in this workspace; no hot path pays for it.
    crate::gateway_routes::sweep_stale_temporaries(&workspace);

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
    //
    // UNTESTED, deliberately: observing it needs the beat to be mid-write when
    // the stop lands, and HEARTBEAT is tens of seconds. The only way to see it
    // is to inject a tiny interval, which is a test waiting on time — worse than
    // no test. Deleting this line does not fail anything; keep it anyway.
    let _ = refresh.await;
    retire_route(&workspace, &route, port);
    served
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
/// One counter PER CAUSE, each reset by the other: `attempts` has to mean
/// "consecutive beats of THIS failure", or a run that alternates between a
/// foreign owner and an unwritable workspace logs a total describing neither.
async fn refresh_route(workspace: PathBuf, route: gateway::RouteName, port: u16) -> ! {
    let mut foreign_beats: u64 = 0;
    let mut failed_beats: u64 = 0;
    loop {
        tokio::time::sleep(crate::services::HEARTBEAT).await;
        match crate::gateway_routes::reassert(&workspace, &route, port) {
            Ok(RouteOwner::Vacant | RouteOwner::Ours) => {
                foreign_beats = 0;
                failed_beats = 0;
            }
            Ok(RouteOwner::Foreign) => {
                failed_beats = 0;
                foreign_beats += 1;
                if beat_is_worth_a_line(foreign_beats) {
                    tracing::warn!(
                        target: "ducktape::gateway",
                        attempts = foreign_beats,
                        reason = "route_owned_by_another_daemon",
                        "another airlock daemon owns this workspace's gateway route; \
                         this one serves nothing the node will reach"
                    );
                }
            }
            Err(error) => {
                foreign_beats = 0;
                failed_beats += 1;
                if beat_is_worth_a_line(failed_beats) {
                    tracing::warn!(
                        target: "ducktape::gateway",
                        attempts = failed_beats,
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

/// A work pointer longer than this names no committed saga on any chain this
/// node has ever seen, and turning it into a `/v1/query` would make an unadmitted
/// caller's byte count this node's problem. Sagas in the tree are short
/// (`sched\x1f<name>`, `dispatch\x1fruns\x1f<id>`); this is slack, not a policy.
const MAX_WORK_POINTER_BYTES: usize = 512;

/// The committed-state grant gate the owner's own gateway enforces: given the
/// credential named, the account the node's proxy VOUCHED for, and the work the
/// session says it is for, resolve this node's own committed state and answer
/// whether that session may open.
///
/// This is the daemon's ONLY use of its node link, and every one of them is a
/// read.
fn committed_grant_check(node: NodeLink) -> airlock::server::GrantCheck {
    Arc::new(move |question: GrantQuestion| {
        let node = node.clone();
        Box::pin(async move { grant_answer(&node, &question).await })
            as std::pin::Pin<Box<dyn std::future::Future<Output = GrantAnswer> + Send>>
    })
}

/// Fail closed, but say WHICH closed door. A committed record that does not
/// admit the session is a refusal the borrower's operator can act on; a node
/// that did not answer is not — reporting it as a refusal sends them to add a
/// grant that already exists, which is the exact bug this taxonomy replaces.
///
/// TWO ways in, and only two:
///
/// 1. **the caller's own grant** — the account this node vouched for on the hop
///    is the credential's owner or a grantee. Unchanged since PR #833, and
///    checked first because it needs no read beyond the record itself.
/// 2. **delegation** — [`delegated_answer`], for a session that presented a
///    pointer to committed work.
///
/// The reason is named at debug (this is per-session) and never carries an
/// account, a saga id, or a token.
async fn grant_answer(reader: &dyn CommittedReader, question: &GrantQuestion) -> GrantAnswer {
    let record = match committed_credential_record(reader, &question.credential).await {
        Ok(Some(record)) => record,
        Ok(None) => {
            tracing::debug!(
                target: "ducktape::gateway",
                reason = "credential_record_absent",
                credential = %question.credential,
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
                credential = %question.credential,
                "airlock session not decided: {error}"
            );
            return GrantAnswer::Undetermined;
        }
    };
    let caller_is_granted = credential_use_allowed(&record, &question.caller);
    if caller_is_granted {
        return GrantAnswer::Granted;
    }
    match &question.work {
        // Nothing to delegate against. An interactive session takes this arm by
        // construction: there is no committed record of who asked for a pty.
        WorkRef::Direct => {
            tracing::debug!(
                target: "ducktape::gateway",
                reason = "credential_not_granted",
                credential = %question.credential,
                "airlock session refused"
            );
            GrantAnswer::Refused
        }
        WorkRef::Saga { saga_id } => {
            delegated_answer(reader, &record, &question.caller, saga_id).await
        }
    }
}

/// One resolved half of the delegated check. THREE states for the same reason
/// [`GrantAnswer`] has three: a read that did not answer is not a "no".
enum Half {
    Yes,
    No,
    Unreadable,
}

/// **Delegation: a run submitted by A and executed on B draws on A's grant.**
///
/// BOTH halves are required, resolved from this lender's OWN committed state,
/// and a future reader must not "simplify" either away:
///
/// - **the saga's origin is an account this credential is granted to**, AND
/// - **the caller is that saga's assignee.**
///
/// Drop the second and B may point at ANY saga A ever submitted and draw on A's
/// grant for work A never assigned to it. Drop the first and this is just the
/// old rule wearing a pointer. Neither is sufficient; the conjunction is the
/// whole security argument.
///
/// Nothing here trusts the caller beyond "which record to look up": the origin
/// is a signature-proven node key (`/v1/submit` re-signs with the node's own
/// signer), the assignee is what the saga module committed, and both are mapped
/// to accounts through the identity module. See [`airlock::wire::WorkRef`].
async fn delegated_answer(
    reader: &dyn CommittedReader,
    record: &CredentialRecord,
    caller: &[u8],
    saga_id: &str,
) -> GrantAnswer {
    let pointer_is_plausible = saga_id.len() <= MAX_WORK_POINTER_BYTES;
    if !pointer_is_plausible {
        tracing::debug!(
            target: "ducktape::gateway",
            reason = "work_pointer_oversized",
            "airlock session refused"
        );
        return GrantAnswer::Refused;
    }
    let saga = match committed_saga(reader, saga_id).await {
        Ok(Some(saga)) => saga,
        // NOT a refusal. This lender simply cannot SEE that saga yet — a follower
        // behind head, or an id naming nothing at all. Reporting "no" here would
        // tell the borrower's operator to go add a grant they may already hold,
        // which is the exact bug the three-state taxonomy exists to prevent; a
        // 503 tells them to retry, and a run whose saga never commits fails on
        // its own lane rather than on this one.
        Ok(None) => {
            tracing::debug!(
                target: "ducktape::gateway",
                reason = "delegated_work_unseen",
                "airlock session not decided"
            );
            return GrantAnswer::Undetermined;
        }
        Err(error) => {
            tracing::debug!(
                target: "ducktape::gateway",
                reason = "grant_authority_unavailable",
                "airlock session not decided: {error}"
            );
            return GrantAnswer::Undetermined;
        }
    };
    // HALF ONE, and it runs first because it is what binds the pointer to THIS
    // caller. Short-circuiting also means a caller pointing at somebody else's
    // saga never costs this node the second read.
    match caller_holds_the_lease(reader, &saga, caller).await {
        Half::Unreadable => return GrantAnswer::Undetermined,
        Half::No => {
            tracing::debug!(
                target: "ducktape::gateway",
                reason = "delegated_caller_not_assignee",
                "airlock session refused"
            );
            return GrantAnswer::Refused;
        }
        Half::Yes => {}
    }
    // HALF TWO.
    match submitter_is_granted(reader, record, &saga.origin).await {
        Half::Yes => GrantAnswer::Granted,
        Half::No => {
            tracing::debug!(
                target: "ducktape::gateway",
                reason = "delegated_submitter_not_granted",
                "airlock session refused"
            );
            GrantAnswer::Refused
        }
        Half::Unreadable => GrantAnswer::Undetermined,
    }
}

/// Half one: does the vouched-for caller hold this saga's lease? Only the node
/// the saga module actually assigned the attempt to may present its pointer.
///
/// An attempt not yet leased has no assignee, and that is a real committed state
/// rather than an unreadable one: nobody holds the lease, so nobody may draw on
/// it.
async fn caller_holds_the_lease(
    reader: &dyn CommittedReader,
    saga: &SagaView,
    caller: &[u8],
) -> Half {
    let Some(assignee) = saga.assignee.as_deref() else {
        return Half::No;
    };
    match account_of_node(reader, assignee).await {
        Ok(Some(account)) => match account == caller {
            true => Half::Yes,
            false => Half::No,
        },
        // The assignee node is bound to no account, so it is not this caller.
        Ok(None) => Half::No,
        Err(_) => Half::Unreadable,
    }
}

/// Half two: is the account that SUBMITTED this saga one the credential admits?
///
/// `SagaOrigin::External` is the only attributable arm, and it is attributable
/// precisely because `/v1/submit` re-signs with the node's own key. A saga a
/// MODULE triggered (the dispatch family: chat mention, pages/forge comment, the
/// jobs board, `RunsMsg::RequestRun`) names no account at this layer, so there is
/// no grant to check and no subject to draw as — refused. It closes nothing
/// legitimate: those payloads are composed in consensus by `runs::envelope`,
/// which has no credential field at all, so a module-origin saga cannot name a
/// credential in the first place. `System` is genesis, and the same.
///
/// Note this is the OPPOSITE call from `work_admission`, which ADMITS a
/// module-origin saga — deliberately, and the two are answering different
/// questions. "Will I spend my CPU on this?" has a defensible answer for
/// unattributable work (yes, it is bounded and it cannot name a credential).
/// "Whose subscription does this spend?" does not: there is no whose.
async fn submitter_is_granted(
    reader: &dyn CommittedReader,
    record: &CredentialRecord,
    origin: &SagaOrigin,
) -> Half {
    let submitter = match origin {
        SagaOrigin::External(node_key) => node_key,
        SagaOrigin::Module(_) => return Half::No,
        SagaOrigin::System => return Half::No,
    };
    match account_of_node(reader, submitter).await {
        Ok(Some(account)) => match credential_use_allowed(record, &account) {
            true => Half::Yes,
            false => Half::No,
        },
        Ok(None) => Half::No,
        Err(_) => Half::Unreadable,
    }
}

/// Read one credential record from this node's committed gateway-module state
/// over `/v1/query`, so the gate sees exactly what consensus committed.
async fn committed_credential_record(
    reader: &dyn CommittedReader,
    name: &str,
) -> Result<Option<CredentialRecord>, String> {
    let request = gateway::encode_query(&GatewayQuery::Credential { name: name.to_string() });
    let bytes = reader.read("gateway", request).await?;
    match gateway::decode_reply(&bytes)? {
        GatewayReply::Credential(record) => Ok(record),
        other => Err(format!("gateway returned an unexpected reply: {other:?}")),
    }
}

/// Read one saga from this node's committed saga-module state. `Ok(None)` is
/// "this node has not committed that saga", which is a genuinely different thing
/// from a read that failed — see [`delegated_answer`], which keeps them apart.
async fn committed_saga(
    reader: &dyn CommittedReader,
    saga_id: &str,
) -> Result<Option<SagaView>, String> {
    let request = saga::encode_query(&SagaQuery::Get {
        saga_id: saga_id.to_string(),
    });
    let bytes = reader.read("saga", request).await?;
    match saga::decode_reply(&bytes)? {
        SagaReply::Saga(view) => Ok(view),
        other => Err(format!("saga returned an unexpected reply: {other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- the delegation gate ------------------------------------------------

    const CRED: &str = "owner-claude-1";
    const OWNER_NODE: &[u8] = b"owner-node";
    const OWNER_ACCOUNT: &[u8] = b"owner-account";
    const EXEC_NODE: &[u8] = b"executor-node";
    const EXEC_ACCOUNT: &[u8] = b"executor-account";
    const STRANGER_NODE: &[u8] = b"stranger-node";
    const STRANGER_ACCOUNT: &[u8] = b"stranger-account";
    const SAGA: &str = "sched\u{1f}delegated";

    /// A committed-state stand-in for the lender's own node. Answers the three
    /// reads the gate makes and nothing else; anything unexpected panics rather
    /// than defaulting, so a new read cannot slip in unnoticed.
    struct Committed {
        /// `None` = the lender has not committed this saga (a follower behind
        /// head, or an id naming nothing).
        saga: Option<SagaView>,
        /// which reads fail outright, by module target.
        unreadable: &'static [&'static str],
        grants: Vec<Vec<u8>>,
    }

    impl Committed {
        fn new(origin: SagaOrigin, assignee: Option<&[u8]>) -> Self {
            Self {
                saga: Some(view(origin, assignee)),
                unreadable: &[],
                grants: Vec::new(),
            }
        }
    }

    fn view(origin: SagaOrigin, assignee: Option<&[u8]>) -> SagaView {
        SagaView {
            origin,
            reply_to: None,
            reply_payload: Vec::new(),
            spec: Vec::new(),
            capability: None,
            status: saga::SagaStatus::Pending,
            attempt: 0,
            max_attempts: 1,
            assignee: assignee.map(<[u8]>::to_vec),
            pinned_assignee: assignee.map(<[u8]>::to_vec),
            lease_views: None,
            lease_expires_at: None,
            deadline: None,
            result: None,
            error: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[async_trait::async_trait]
    impl CommittedReader for Committed {
        async fn read(&self, target: &str, request: Vec<u8>) -> Result<Vec<u8>, String> {
            if self.unreadable.contains(&target) {
                return Err(format!("{target} did not answer"));
            }
            match target {
                "gateway" => Ok(gateway::encode_reply(&GatewayReply::Credential(Some(
                    CredentialRecord {
                        name: CRED.into(),
                        owner_account: OWNER_ACCOUNT.to_vec(),
                        publisher_node: OWNER_NODE.to_vec(),
                        kind: gateway::CredentialKind::Claude,
                        seal_pk: [3u8; 32],
                        grants: self.grants.iter().cloned().collect(),
                    },
                )))),
                "saga" => Ok(saga::encode_reply(&SagaReply::Saga(self.saga.clone()))),
                "identity" => {
                    let identity::IdentityQuery::OfNode { node_key } =
                        identity::decode_query(&request).expect("an identity query")
                    else {
                        panic!("the gate asks identity only OfNode");
                    };
                    let account = match node_key.as_slice() {
                        OWNER_NODE => Some(OWNER_ACCOUNT.to_vec()),
                        EXEC_NODE => Some(EXEC_ACCOUNT.to_vec()),
                        STRANGER_NODE => Some(STRANGER_ACCOUNT.to_vec()),
                        _unbound => None,
                    };
                    Ok(identity::encode_reply(&identity::IdentityReply::Account(
                        account.map(|account_id| identity::AccountView {
                            account_id,
                            display_name: None,
                            avatar: None,
                            bio: None,
                            nonce: 0,
                            member_keys: Vec::new(),
                            nodes: Vec::new(),
                            updated_at: 0,
                        }),
                    )))
                }
                other => panic!("the gate read {other:?}, which it has no business reading"),
            }
        }
    }

    fn question(caller: &[u8], work: WorkRef) -> GrantQuestion {
        GrantQuestion {
            credential: CRED.into(),
            caller: caller.to_vec(),
            work,
        }
    }

    fn pointer() -> WorkRef {
        WorkRef::Saga {
            saga_id: SAGA.into(),
        }
    }

    /// THE DELEGATED ADMISSION: the executor holds the lease AND the submitter is
    /// granted — here implicitly, because the submitter owns the credential. The
    /// executor is on no grant list at all.
    #[tokio::test]
    async fn a_pointer_admits_the_lease_holder_on_the_submitters_grant() {
        let state = Committed::new(SagaOrigin::External(OWNER_NODE.to_vec()), Some(EXEC_NODE));
        assert_eq!(
            grant_answer(&state, &question(EXEC_ACCOUNT, pointer())).await,
            GrantAnswer::Granted
        );
    }

    /// HALF ONE. The origin is the owner, so a gate that checked only the origin
    /// admits this — and that is precisely the hole: every saga the owner ever
    /// submitted would become a key to the owner's subscription, for work the
    /// owner never assigned to this node.
    #[tokio::test]
    async fn a_caller_that_does_not_hold_the_lease_is_refused() {
        let state = Committed::new(
            SagaOrigin::External(OWNER_NODE.to_vec()),
            Some(STRANGER_NODE),
        );
        assert_eq!(
            grant_answer(&state, &question(EXEC_ACCOUNT, pointer())).await,
            GrantAnswer::Refused
        );
    }

    /// …and an attempt nobody holds yet is the same answer, from committed state
    /// rather than from a failed read.
    #[tokio::test]
    async fn an_unleased_attempt_is_held_by_nobody() {
        let state = Committed::new(SagaOrigin::External(OWNER_NODE.to_vec()), None);
        assert_eq!(
            grant_answer(&state, &question(EXEC_ACCOUNT, pointer())).await,
            GrantAnswer::Refused
        );
    }

    /// HALF TWO. The caller genuinely holds this lease — dropping the origin
    /// check would make the pointer a universal key for any assignee.
    #[tokio::test]
    async fn a_submitter_the_credential_does_not_admit_is_refused() {
        let state = Committed::new(
            SagaOrigin::External(STRANGER_NODE.to_vec()),
            Some(EXEC_NODE),
        );
        assert_eq!(
            grant_answer(&state, &question(EXEC_ACCOUNT, pointer())).await,
            GrantAnswer::Refused
        );
    }

    /// …and an explicit grant to that submitter is the only thing that changes
    /// it. Same saga, same caller, same lease.
    #[tokio::test]
    async fn a_granted_submitter_delegates_without_owning_the_credential() {
        let mut state = Committed::new(
            SagaOrigin::External(STRANGER_NODE.to_vec()),
            Some(EXEC_NODE),
        );
        state.grants = vec![STRANGER_ACCOUNT.to_vec()];
        assert_eq!(
            grant_answer(&state, &question(EXEC_ACCOUNT, pointer())).await,
            GrantAnswer::Granted
        );
    }

    /// A saga this lender has NOT committed decides nothing. It is what a
    /// follower behind head sees, and a 403 there would send the borrower's
    /// operator to add a grant they may already hold.
    #[tokio::test]
    async fn a_saga_the_lender_cannot_see_is_undetermined_not_refused() {
        let mut state = Committed::new(SagaOrigin::External(OWNER_NODE.to_vec()), Some(EXEC_NODE));
        state.saga = None;
        assert_eq!(
            grant_answer(&state, &question(EXEC_ACCOUNT, pointer())).await,
            GrantAnswer::Undetermined
        );
    }

    /// The same three-state rule for each read the delegated path makes.
    #[tokio::test]
    async fn a_read_that_did_not_answer_is_undetermined() {
        for unreadable in [&["saga"][..], &["identity"][..]] {
            let mut state =
                Committed::new(SagaOrigin::External(OWNER_NODE.to_vec()), Some(EXEC_NODE));
            state.unreadable = unreadable;
            assert_eq!(
                grant_answer(&state, &question(EXEC_ACCOUNT, pointer())).await,
                GrantAnswer::Undetermined,
                "an unanswered {unreadable:?} read must not read as a refusal"
            );
        }
    }

    /// A saga a MODULE triggered names no account, so there is no subject to
    /// draw as. Deliberately the OPPOSITE call from `work_admission`, which
    /// admits the same origin: "will I spend my CPU" has a defensible answer for
    /// unattributable work; "whose subscription does this spend" does not.
    #[tokio::test]
    async fn a_module_triggered_saga_names_no_account_to_draw_as() {
        for origin in [SagaOrigin::Module("dispatch".into()), SagaOrigin::System] {
            let state = Committed::new(origin, Some(EXEC_NODE));
            assert_eq!(
                grant_answer(&state, &question(EXEC_ACCOUNT, pointer())).await,
                GrantAnswer::Refused
            );
        }
    }

    /// A session presenting no pointer is decided exactly as it was before
    /// delegation existed — every interactive pty takes this arm.
    #[tokio::test]
    async fn a_direct_session_never_delegates() {
        let state = Committed::new(SagaOrigin::External(OWNER_NODE.to_vec()), Some(EXEC_NODE));
        assert_eq!(
            grant_answer(&state, &question(EXEC_ACCOUNT, WorkRef::Direct)).await,
            GrantAnswer::Refused,
            "a delegable saga existing does not delegate a session that never named it"
        );
    }

    /// The caller's OWN grant is answered off the credential record alone. Not an
    /// optimization: reading a saga here would make every ordinary session — and
    /// every pty — depend on the saga module answering.
    #[tokio::test]
    async fn a_granted_caller_costs_no_saga_read() {
        struct RecordOnly;
        #[async_trait::async_trait]
        impl CommittedReader for RecordOnly {
            async fn read(&self, target: &str, _request: Vec<u8>) -> Result<Vec<u8>, String> {
                assert_eq!(target, "gateway", "a granted caller reads only its record");
                Ok(gateway::encode_reply(&GatewayReply::Credential(Some(
                    CredentialRecord {
                        name: CRED.into(),
                        owner_account: OWNER_ACCOUNT.to_vec(),
                        publisher_node: OWNER_NODE.to_vec(),
                        kind: gateway::CredentialKind::Claude,
                        seal_pk: [3u8; 32],
                        grants: [EXEC_ACCOUNT.to_vec()].into_iter().collect(),
                    },
                ))))
            }
        }
        assert_eq!(
            grant_answer(&RecordOnly, &question(EXEC_ACCOUNT, pointer())).await,
            GrantAnswer::Granted
        );
    }

    /// A pointer nobody could have committed is refused before it becomes a
    /// query: an unadmitted caller's byte count is not this node's problem.
    #[tokio::test]
    async fn an_oversized_pointer_is_refused_before_it_is_looked_up() {
        struct RecordThenPanic;
        #[async_trait::async_trait]
        impl CommittedReader for RecordThenPanic {
            async fn read(&self, target: &str, _request: Vec<u8>) -> Result<Vec<u8>, String> {
                assert_eq!(target, "gateway", "an oversized pointer is never looked up");
                Ok(gateway::encode_reply(&GatewayReply::Credential(Some(
                    CredentialRecord {
                        name: CRED.into(),
                        owner_account: OWNER_ACCOUNT.to_vec(),
                        publisher_node: OWNER_NODE.to_vec(),
                        kind: gateway::CredentialKind::Claude,
                        seal_pk: [3u8; 32],
                        grants: Default::default(),
                    },
                ))))
            }
        }
        let oversized = WorkRef::Saga {
            saga_id: "x".repeat(MAX_WORK_POINTER_BYTES + 1),
        };
        assert_eq!(
            grant_answer(&RecordThenPanic, &question(EXEC_ACCOUNT, oversized)).await,
            GrantAnswer::Refused
        );
    }

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

    /// The registered port is a standing instruction to the node's reverse
    /// proxy, so its lifetime must be exactly the daemon's: published before the
    /// listener serves, gone once the daemon stops.
    ///
    /// It drives [`serve_until`], which is [`serve`] minus only the config
    /// unpacking — so it holds the arming ordering too. Arming installs a real
    /// signal handler, which PANICS outside a reactor rather than erroring; a
    /// refactor hoisting it out of `block_on` is a production-only crash with no
    /// compile-time complaint, and a hand-rolled replica of the call site (the
    /// guard this replaces) stayed green through exactly that mutation.
    ///
    /// What it does NOT cover: the beat task's abort-then-join. See the comment
    /// on that line for why it has no honest test seam.
    #[test]
    fn the_route_lives_exactly_as_long_as_the_daemon() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("workspace");
        let storage = dir.path().join("storage");
        let route = gateway::RouteName::named(AIRLOCK_ROUTE);

        // The stop future's FIRST poll happens inside `run`'s select!, i.e.
        // after the route is registered and the router is being served. That is
        // the daemon's own "serving" event, so this observation waits on the
        // system rather than on a clock.
        let observed = Arc::new(std::sync::Mutex::new(None));
        let seen = observed.clone();
        let peek = (workspace.clone(), route.clone());
        let stop = async move {
            *seen.lock().unwrap() = crate::gateway_routes::load(&peek.0).unwrap().port(&peek.1);
        };

        serve_until(
            "airlock#test".into(),
            storage,
            // never dialed: no session is opened in this test.
            "http://127.0.0.1:1".into(),
            workspace.clone(),
            stop,
        )
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
