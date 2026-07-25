//! `ducktape service run compute` — the standalone compute daemon.
//!
//! This is the compute plane. The node process constructs no provider set, no
//! dispatch pool and no resource ledger any more; it keeps only the podman
//! SERVICE (its still-in-node pty plane needs one) and the consensus lanes.
//! Everything below runs in a separate process with its own failure domain, and
//! reaches its node exactly the way the CLI does — over localhost `/v1` + ws.
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
//! ## what the node still owns
//!
//! The podman service itself: its socket, storage root and egress hook live
//! under the node's data dir and its interactive terminal plane spawns ptys
//! through them. The daemon is a CLIENT of that socket. Container ownership is
//! therefore label-scoped, not socket-scoped — see [`reap`].

use std::collections::BTreeMap;
use std::sync::Arc;

use commonware_cryptography::Signer as _;
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
pub(crate) struct Compute {
    pub(crate) grant: ServiceGrant,
    pub(crate) resolved: config::Resolved,
    pub(crate) http_base: String,
}

/// Serve until the process is stopped. Returns only on a fatal misconfiguration
/// — a node that is merely down is retried forever, because that is an
/// operational state, not an error.
pub(crate) fn serve(compute: Compute) -> Result<(), Box<dyn std::error::Error>> {
    // the pool hands Send futures to `SpawnFn`, so a multi-thread runtime is
    // what it expects; the intake pass itself is !Send (the `Worker` seam is
    // `?Send`) and rides `block_on` on this thread.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(run(compute))
}

async fn run(compute: Compute) -> Result<(), Box<dyn std::error::Error>> {
    let Compute {
        grant,
        resolved,
        http_base,
    } = compute;
    let node_key = resolved.signer.public_key().as_ref().to_vec();
    let node = NodeLink::new(&http_base).with_forge_repo(resolved.storage_dir.join("forge-repo"));

    // the node must be ANSWERING before anything is discovered or reaped: a
    // daemon that boots first would otherwise reap against a socket that does
    // not exist yet and then spin on an unreadable projection.
    await_node(&node).await;

    let backend = resolved
        .sandbox
        .clone()
        .ok_or("no [sandbox] table in node.toml: this host has no configured way to isolate a run")?;
    reap(&backend, &grant).await;

    let (line_tx, line_rx) = tokio::sync::mpsc::channel(link::OUTPUT_LANE);
    let providers = provider_host::discover(
        &node_key,
        provider_host::AgentDirs::under(&resolved.storage_dir),
        Some(output_sink(line_tx)),
        backend,
        &grant.display_id(),
    )?;
    let offered = providers.capabilities();

    let hint = Arc::new(tokio::sync::Notify::new());
    tokio::spawn(link::attach(
        ws_url(&http_base),
        hint.clone(),
        line_rx,
    ));

    let (mut pump, mut delivered) = build_pool(&node, &resolved, node_key, providers).await?;

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
        }
    }
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

/// Adopt this instance's containers and sweep the retired flat label.
///
/// Two sweeps, deliberately different in kind:
///
/// - **this instance's own label.** A daemon that crashed mid-run left
///   containers behind; it returns with the SAME `compute#hex8` (the id is the
///   grant hash and the grant persists in `services.toml`) and so can recognise
///   them as its own. That re-adoption across restart is exactly why the id
///   must survive one.
/// - **`io.ducktape.managed=capability-host`.** The pre-daemon flat label,
///   written when one node process owned every container. Nothing writes it any
///   more. This is DISPOSABLE RUNTIME-STATE CLEANUP, not a compat arm — delete
///   this sweep once no host can still be carrying pre-daemon containers.
///
/// Best-effort throughout: a reap failure is a log line, never a boot failure.
async fn reap(backend: &provider_host::SandboxBackend, grant: &ServiceGrant) {
    let provider_host::SandboxBackend::Podman { socket, .. } = backend else {
        // Tart clones and deletes a VM per run; there is no label to reap.
        return;
    };
    let mine = provider_host::managed_label(&grant.display_id());
    for (label, reason) in [
        (mine.as_str(), "own_orphans"),
        (provider_host::RETIRED_FLAT_MANAGED_LABEL, "retired_label"),
    ] {
        match provider_host::reap_by_label(socket, label).await {
            Ok(0) => {}
            Ok(removed) => tracing::info!(
                target: "ducktape::service",
                removed,
                reason,
                "reaped orphaned sandbox containers"
            ),
            Err(error) => tracing::warn!(
                target: "ducktape::service",
                reason = "reap_failed",
                "could not sweep sandbox containers: {error}"
            ),
        }
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
    resolved: &config::Resolved,
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
            noded::agent_provision::agent_runs_root(&resolved.storage_dir)?,
        )
        .with_forge(
            noded::agent_provision::forge_push_base(resolved.http_listen.as_deref()),
            config::hex_bytes(&node_key),
        )
        .with_node_url(noded::agent_provision::node_http_base(
            resolved.http_listen.as_deref(),
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
        capacity_of(resolved),
        provisioner,
    )
    .with_credential_resolver(resolver);
    let control = pool.attempt_control();
    Ok((
        intake::WorkPump::new(Box::new(pool), control, node_key),
        rx,
    ))
}

/// the announced capacity IS the pool's ledger — one source, so the scheduler
/// never promises what this daemon cannot seat.
fn capacity_of(resolved: &config::Resolved) -> BTreeMap<String, u64> {
    resolved.sandbox_capacity.clone()
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
        // the whole point of the flag day: two services on one podman socket
        // produce two disjoint labels, so neither reaper can see the other's
        // containers — and neither equals the retired flat label the boot
        // sweep removes.
        let compute = provider_host::managed_label("compute#deadbeef");
        let term = provider_host::managed_label(provider_host::NODE_TERM_OWNER);
        assert_eq!(compute, "io.ducktape.managed=compute#deadbeef");
        assert_ne!(compute, term);
        assert_ne!(compute, provider_host::RETIRED_FLAT_MANAGED_LABEL);
        assert_ne!(term, provider_host::RETIRED_FLAT_MANAGED_LABEL);
    }
}
