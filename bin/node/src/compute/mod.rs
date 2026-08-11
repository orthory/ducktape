//! `ducktape service run compute` — the standalone compute daemon.
//!
//! This is the compute plane. The node process constructs no provider set, no
//! dispatch pool, no resource ledger and — since the agent carve — no podman
//! service either; it keeps the consensus lanes and nothing else. Everything
//! below runs in a separate process with its own failure domain, and reaches its
//! node exactly the way the CLI does — over localhost `/v1` + ws.
//!
//! ## the seams, and where each one landed
//!
//! | in-process seam | here |
//! |---|---|
//! | `SpawnFn` | this daemon's own tokio runtime |
//! | `DeliverFn` | `POST /v1/submit` — the node re-signs with ITS key, which IS the saga assignee |
//! | `CredentialResolver` | `POST /v1/query` against gateway/identity/saga ([`cred`]) |
//! | `WorkspaceProvisioner` | `noded::agent_provision` over the same `/v1` lane |
//! | work intake (effect lane) | `SagaQuery::AssignedPending` + ws hints ([`intake`]) |
//! | `OutputSink` → stream hub | a `run_output` ws frame ([`link`]) |
//! | `__egress-hook` | already a subcommand of THIS binary — the hook podman fires is `ducktape __egress-hook`, and the daemon ships in the same executable, so nothing moved |
//!
//! ## podman is this daemon's, not the node's
//!
//! This daemon starts its own node-private podman service under
//! `<storage>/services/compute` — socket, storage root and egress hook. The node
//! starts none, and neither does it share one with the agent daemon: a
//! `kill_on_drop` service child shared between two processes would make them a
//! single failure domain. Container ownership is label-scoped ON TOP of that —
//! see [`reap`] — so the two guarantees are independent.

use std::collections::BTreeMap;
use std::sync::Arc;

use compute_service::{DeliverFn, DispatchPool, SpawnFn, SpawnKind, max_concurrent_runs_from_env};
use noded::node_link::NodeLink;

use crate::config;
use crate::services::ServiceGrant;

mod cred;
mod intake;
mod link;

/// how often the daemon re-evaluates committed state without a hint. The ws
/// heartbeat already fires per block, so this is purely the backstop that turns
/// a dropped socket into a delay instead of a stall.
const SWEEP: std::time::Duration = std::time::Duration::from_secs(15);

/// how long to wait between readiness probes while the node is still coming up.
const READY_RETRY: std::time::Duration = std::time::Duration::from_secs(2);
/// log attempt 1, then every Nth — never an unconditional warn in a retry loop.
const LOG_EVERY: u64 = 15;

/// Everything the daemon needs, resolved before any of it runs.
///
/// `service` is [`config::ServiceConfig`], NOT `config::Resolved`: the latter
/// carries the node's ed25519 private key, and this process must not be able to
/// hold one. `node_key` is the node's PUBLIC identity, learned from the node
/// over `/v1/status` — it names provider dirs and forge authorship, and signs
/// nothing.
pub(crate) struct Compute {
    pub(crate) grant: ServiceGrant,
    pub(crate) service: config::ServiceConfig,
    pub(crate) http_base: String,
    pub(crate) node_key: [u8; 32],
}

/// Serve until the process is stopped. Returns only on a fatal misconfiguration
/// — a node that is merely down is retried forever, because that is an
/// operational state, not an error.
///
/// Through [`crate::services::serve_until_stopped`], which owns the runtime and
/// arms SIGTERM/SIGINT before a line of this daemon runs: the `podman system
/// service` started below must never outlive the process that started it.
pub(crate) fn serve(compute: Compute) -> Result<(), Box<dyn std::error::Error>> {
    crate::services::serve_until_stopped(std::future::pending(), |stop| run(compute, stop))
}

async fn run(
    compute: Compute,
    mut stop: crate::services::Stop,
) -> Result<(), Box<dyn std::error::Error>> {
    let Compute {
        grant,
        service,
        http_base,
        node_key,
    } = compute;
    let node_key = node_key.to_vec();
    let node = NodeLink::new(&http_base).with_forge_repo(service.storage_dir.join("forge-repo"));

    // the node must be ANSWERING before anything is discovered or reaped: a
    // daemon that boots first would otherwise reap against a socket that does
    // not exist yet and then spin on an unreadable projection.
    //
    // Raced against the stop, because waiting for a node that never comes is an
    // ordinary state to be SIGTERMed in — and arming the handlers took the
    // default disposition away, so an unraced wait here would be a daemon no
    // signal can end. Nothing has been started yet, so there is nothing to tear
    // down on this path.
    tokio::select! {
        () = await_node(&node) => {}
        () = &mut stop => return Ok(()),
    }

    // this daemon's OWN podman service, under `<storage>/services/compute`. The
    // node used to start one for everybody; it does not any more, because a
    // `kill_on_drop` service child shared between two daemons made them one
    // failure domain. Fail-closed: a start failure ends the process rather than
    // leaving a daemon that announces capacity it cannot sandbox.
    let backend = crate::services::podman_backend(&service, &grant.kind)?;
    let self_exe = std::env::current_exe()
        .map_err(|error| format!("cannot resolve this daemon's own executable: {error}"))?;
    let podman = provider_host::PodmanService::start_for(
        &backend,
        &crate::services::podman_data_dir(&service, &grant.kind),
        &self_exe,
    )
    .await?;
    // whatever still carries this instance's label got here through a death
    // that ran no code — the stop path leaves none — and is destroyed, never
    // resumed. See [`crate::services::Sweep`].
    crate::services::sweep_own_containers(&backend, &grant, crate::services::Sweep::CrashOrphans)
        .await;

    let (line_tx, line_rx) = tokio::sync::mpsc::channel(link::OUTPUT_LANE);
    let providers = provider_host::discover(
        &node_key,
        Some(output_sink(line_tx)),
        // cloned: `discover` consumes the backend, and the teardown below needs
        // the same socket to sweep this instance's containers through.
        backend.clone(),
        &grant.display_id(),
    )?;
    let offered = providers.capabilities();

    let hint = Arc::new(tokio::sync::Notify::new());
    tokio::spawn(link::attach(
        ws_url(&http_base),
        hint.clone(),
        line_rx,
    ));

    let (mut pump, mut delivered) = build_pool(&node, &service, node_key, providers).await?;

    tracing::info!(
        target: "ducktape::service",
        instance = %grant.display_id(),
        capabilities = offered.len(),
        concurrency = max_concurrent_runs_from_env(),
        "compute daemon serving"
    );

    loop {
        tokio::select! {
            // a delivered op: a result belongs to a tracked attempt and is
            // retried until it commits; a lease heartbeat is fire-and-forget.
            Some(msg) = delivered.recv() => match intake::Delivered::classify(msg) {
                intake::Delivered::Result { saga_id, attempt, msg } => {
                    pump.completed(saga_id, attempt, msg);
                    pump.tick(&node).await;
                }
                intake::Delivered::Heartbeat(msg) => {
                    if let Err(error) = node.submit(&msg.target, &msg.payload).await {
                        tracing::debug!(
                            target: "ducktape::saga",
                            error = %error,
                            reason = "lease_renew_failed",
                            "compute lease heartbeat dropped"
                        );
                    }
                }
            },
            // a block committed (or the node's 3s beat): re-evaluate.
            () = hint.notified() => pump.tick(&node).await,
            // the backstop: a missed hint delays work, never loses it.
            () = tokio::time::sleep(SWEEP) => pump.tick(&node).await,
            // SIGTERM/SIGINT. An in-flight run's result is lost with the
            // process exactly as it is on a crash — the saga's lease timeout
            // re-leases it — so there is nothing to drain first.
            () = &mut stop => break,
        }
    }
    stop_sandbox(podman, &backend, &grant).await;
    Ok(())
}

/// Tear the sandbox down, containers FIRST.
///
/// Order is the whole point. Killing the `podman system service` does not stop
/// what it created: each container's conmon is its own parent, ignores SIGTERM,
/// and would keep the workload running under a service that no longer exists.
/// So this instance's containers are REMOVED here rather than left for the next
/// start's reaper — over the socket that is still answering right now, which is
/// the only instrument that reaches them — and only then does the service child
/// go. Leaving them would mean a stopped daemon still burning CPU and holding a
/// graph root until something happened to start that kind again, which on a
/// torn-down workspace is never.
///
/// SIGKILL still leaves both behind, and nothing here can change that: the
/// answer there is the next start of the same kind, where `PodmanService::claim`
/// reaps the podman service under a root nobody holds any more and the boot
/// sweep destroys the containers.
async fn stop_sandbox(
    podman: Option<provider_host::PodmanService>,
    backend: &provider_host::SandboxBackend,
    grant: &ServiceGrant,
) {
    crate::services::sweep_own_containers(backend, grant, crate::services::Sweep::Teardown).await;
    let Some(service) = podman else {
        // a non-Podman backend started no service (Tart deletes its VM per run).
        return;
    };
    service.shutdown().await;
    tracing::info!(
        target: "ducktape::service",
        instance = %grant.display_id(),
        "compute daemon stopped"
    );
}

/// Block until the node answers a committed query.
///
/// A daemon that starts with (or before) its node is the ordinary systemd case,
/// and a node that has not finished syncing answers nothing useful yet — so
/// this retries forever rather than exiting. Attempt-counted logging: the first
/// wait is worth one line, the rest are worth a counter.
async fn await_node(node: &NodeLink) {
    let mut attempts: u64 = 0;
    loop {
        // NextExpiry is the cheapest committed read that proves the saga module
        // is both present and serving — exactly the precondition intake needs.
        let probe = node
            .query("saga", &saga::encode_query(&saga::SagaQuery::NextExpiry))
            .await;
        let Err(error) = probe else {
            if attempts > 0 {
                tracing::info!(target: "ducktape::service", attempts, "node ready");
            }
            return;
        };
        attempts += 1;
        if attempts == 1 || attempts.is_multiple_of(LOG_EVERY) {
            tracing::warn!(
                target: "ducktape::service",
                attempts,
                reason = "node_not_ready",
                "compute daemon waiting for its node: {error}"
            );
        }
        tokio::time::sleep(READY_RETRY).await;
    }
}

/// the live run tail, across the process boundary. A full lane drops the line
/// rather than blocking: output is a display buffer, and back-pressuring a
/// provider's stdout would let a chatty run stall its own execution.
fn output_sink(lines: tokio::sync::mpsc::Sender<link::OutputLine>) -> provider_host::OutputSink {
    Arc::new(move |ctx, line| {
        let Some(run_key) = ctx.run_key.as_deref() else {
            return;
        };
        let _ = lines.try_send(link::OutputLine {
            run_key: run_key.to_string(),
            stderr: line.stream == provider_host::OutputStream::Stderr,
            line: line.line,
        });
    })
}

/// Assemble the pool and the intake pump around it.
async fn build_pool(
    node: &NodeLink,
    service: &config::ServiceConfig,
    node_key: Vec<u8>,
    providers: provider_host::ProviderSet,
) -> Result<
    (
        intake::WorkPump,
        tokio::sync::mpsc::Receiver<sdk::Msg>,
    ),
    Box<dyn std::error::Error>,
> {
    let spawn: SpawnFn = Arc::new(|kind, future| {
        match kind {
            // Queue waiters share the runtime. An admitted run gets a task of
            // its own too — the distinction the node drew (a supervised
            // dedicated lane) exists because its Drop may synchronously reap a
            // process tree, and here the whole process is that lane's owner.
            SpawnKind::Queued | SpawnKind::TeardownOwner => {
                tokio::spawn(future);
            }
        }
    });

    // the delivery lane, shaped like the node's: bounded, drained by the serve
    // loop, and awaited (never dropped) by a completing run.
    let (tx, rx) = tokio::sync::mpsc::channel::<sdk::Msg>(64);
    let deliver: DeliverFn = Arc::new(move |msg| {
        let tx = tx.clone();
        Box::pin(async move {
            // a closed lane means the serve loop is gone (shutdown): the
            // in-flight result is lost with the process, exactly like a crash
            // mid-run — the saga's lease timeout re-leases it.
            if tx.send(msg).await.is_err() {
                tracing::warn!(
                    target: "ducktape::saga",
                    reason = "result_lane_closed",
                    "compute result dropped"
                );
            }
        })
    });

    let provisioner: compute_service::SharedProvisioner = Arc::new(
        noded::agent_provision::NodedProvisioner::new(
            node.clone(),
            noded::agent_provision::agent_runs_root(&service.storage_dir)?,
        )
        .with_forge(
            noded::agent_provision::forge_push_base(service.http_listen.as_deref()),
            config::hex_bytes(&node_key),
        )
        .with_node_url(noded::agent_provision::node_http_base(
            service.http_listen.as_deref(),
        )),
    );

    let resolver: compute_service::SharedCredentialResolver = Arc::new(
        cred::NodeCredentialResolver::new(node.clone(), browser_gateway(node).await),
    );

    let pool = DispatchPool::with_limit(
        Arc::new(providers),
        node_key.clone(),
        spawn,
        deliver,
        max_concurrent_runs_from_env(),
        capacity_of(service),
        provisioner,
    )
    .with_credential_resolver(resolver);
    let control = pool.attempt_control();
    Ok((
        intake::WorkPump::new(Box::new(pool), control, node_key, service.workspace.clone()),
        rx,
    ))
}

/// the announced capacity IS the pool's ledger — one source, so the scheduler
/// never promises what this daemon cannot seat.
fn capacity_of(service: &config::ServiceConfig) -> BTreeMap<String, u64> {
    service.sandbox_capacity.clone()
}

/// the node's browser-gateway base — the `via` a lent credential's traffic
/// routes through. `None` on a node serving no browser gateway, which then
/// cannot host a lent-credential run (and says so at resolve, not here).
async fn browser_gateway(node: &NodeLink) -> Option<String> {
    let body = reqwest::Client::new()
        .get(format!("{}/v1/gateway/browser", node.base()))
        .send()
        .await
        .ok()?
        .json::<serde_json::Value>()
        .await
        .ok()?;
    body["base"].as_str().map(str::to_string)
}

/// `http(s)://host:port` → `ws(s)://host:port/v1/ws`.
fn ws_url(base: &str) -> String {
    let ws_base = match base.strip_prefix("https://") {
        Some(rest) => format!("wss://{rest}"),
        None => match base.strip_prefix("http://") {
            Some(rest) => format!("ws://{rest}"),
            None => base.to_string(),
        },
    };
    format!("{}/v1/ws", ws_base.trim_end_matches('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ws_url_tracks_the_http_scheme() {
        assert_eq!(ws_url("http://127.0.0.1:8844"), "ws://127.0.0.1:8844/v1/ws");
        assert_eq!(ws_url("https://node.example"), "wss://node.example/v1/ws");
        // a trailing slash must not produce a double slash in the path.
        assert_eq!(ws_url("http://127.0.0.1:8844/"), "ws://127.0.0.1:8844/v1/ws");
    }

    #[test]
    fn the_managed_label_is_scoped_to_one_service_instance() {
        // two services produce two disjoint labels, so neither reaper can see
        // the other's containers even if they ever shared a socket.
        let compute = provider_host::managed_label("compute#deadbeef");
        assert_eq!(compute, "io.ducktape.managed=compute#deadbeef");
        assert_ne!(compute, provider_host::managed_label("agent#deadbeef"));
    }
}
