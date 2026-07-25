//! `ducktape service run agent` — the standalone interactive-terminal daemon.
//!
//! The node process spawns no pty and constructs no provider set. This does
//! both, in its own process with its own failure domain, and reaches its node
//! exactly the way the CLI does — over localhost `/v1` + ws.
//!
//! ## the boundary, and why it is where it is
//!
//! The terminal plane splits AT THE PTY, not at the session. Everything above
//! the pty stayed on the node because it could not leave:
//!
//! - the scrollback and command rings are owned by the node's stream hub, and a
//!   pty client attaches to the NODE's `/v1/ws` to read them;
//! - cross-node sessions ride the mesh term plane, which authenticates peers by
//!   mesh `PeerId` and answers admission from committed state on the node's
//!   actor lane. A daemon holds no keypair and no mesh identity, so that path
//!   stays node-to-node — which is what the design always said it was.
//!
//! What crossed is exactly the podman-touching half: provider discovery, the
//! `PodmanService`, `spawn_interactive`, and each session's pump and reaper.
//! That is the whole milestone — after this, `bin/node` constructs none of them.
//!
//! ## agent and compute are siblings
//!
//! This daemon makes no call of any kind to the compute daemon, and needs none:
//! an interactive session is self-contained. Both link the same
//! provider/sandbox/broker libraries and spawn their own sandboxes; their bus,
//! where they have one at all, is the chain.
//!
//! The one real co-tenancy hazard is podman. Two resolutions, both in force:
//! container ownership is label-scoped to this instance
//! (`io.ducktape.managed=agent#<hex8>`), and this daemon runs its OWN podman
//! service under its own root — so neither reaper can see the other's
//! containers and neither process's exit can kill the other's service child.
//!
//! ## the credential path is unchanged in shape
//!
//! broker-host is still the mandatory per-run isolation boundary: every session
//! gets a per-run loopback endpoint the sandboxed child dials with an opaque
//! bearer, and the real credential never enters the sandbox. Airlock is still
//! only a credential SOURCE, resolved by the NODE from committed gateway state
//! and handed here as a public record (see `agent_service::wire`). This daemon
//! owns no keypair; it never needs one, because it never submits anything.

use std::sync::Arc;

use commonware_cryptography::Signer as _;

use crate::config;
use crate::services::ServiceGrant;

mod link;

/// Everything the daemon needs, resolved before any of it runs.
pub(crate) struct Agent {
    pub(crate) grant: ServiceGrant,
    pub(crate) resolved: config::Resolved,
    pub(crate) http_base: String,
}

/// Serve until the process is stopped. Returns only on a fatal
/// misconfiguration — a node that is merely down is retried forever, because
/// that is an operational state, not an error.
pub(crate) fn serve(agent: Agent) -> Result<(), Box<dyn std::error::Error>> {
    // each session's pump and reaper is its own task, and a pty read must not
    // wait behind a container teardown on another session's thread.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(run(agent))
}

async fn run(agent: Agent) -> Result<(), Box<dyn std::error::Error>> {
    let Agent {
        grant,
        resolved,
        http_base,
    } = agent;
    let node_key = resolved.signer.public_key().as_ref().to_vec();
    let backend = crate::services::podman_backend(&resolved, &grant.kind)?;

    // this daemon's OWN podman service — its socket, storage root and egress
    // hook, under `<storage>/services/agent`. Fail-closed: a start failure ends
    // the process rather than leaving a daemon that signals an interactive
    // plane it cannot sandbox. Held for the process's life; dropping it stops
    // the service child.
    let self_exe = std::env::current_exe()
        .map_err(|error| format!("cannot resolve this daemon's own executable: {error}"))?;
    let _podman = provider_host::PodmanService::start_for(
        &backend,
        &crate::services::podman_data_dir(&resolved, &grant.kind),
        &self_exe,
    )
    .await?;
    reap(&backend, &grant).await;

    let providers = agent_service::discover(
        &node_key,
        provider_host::AgentDirs::under(&resolved.storage_dir),
        backend,
        &grant.display_id(),
    )?;
    let offered = providers.capabilities().len();

    let (events, event_rx) = tokio::sync::mpsc::channel(link::EVENT_LANE);
    let sessions = Arc::new(agent_service::Sessions::new(
        providers,
        provider_host::execution_node_id(&node_key),
        resolved.storage_dir.join("term-sessions"),
        events,
    ));

    tracing::info!(
        target: "ducktape::service",
        instance = %grant.display_id(),
        capabilities = offered,
        cap = agent_service::MAX_TERM_SESSIONS,
        "agent daemon serving"
    );

    // the link never returns: a dropped socket is ordinary (the node restarts,
    // the operator upgrades) and is redialed forever.
    link::attach(ws_url(&http_base), sessions, event_rx).await;
    Ok(())
}

/// Adopt this instance's containers and sweep the retired flat label.
///
/// Two sweeps, deliberately different in kind:
///
/// - **this instance's own label.** A daemon that crashed mid-session left
///   containers behind; it returns with the SAME `agent#hex8` (the id is the
///   grant hash and the grant persists in `services.toml`) and so can recognise
///   them as its own.
/// - **`io.ducktape.managed=node-term`.** The label the node's in-process pty
///   plane wrote before this carve. Nothing writes it any more, and its
///   containers are unreachable: the manager that knew their session ids is
///   gone. This is DISPOSABLE RUNTIME-STATE CLEANUP, not a compat arm — delete
///   this sweep once no host can still be carrying pre-carve containers.
///
/// Both sweeps hit THIS daemon's own socket, so neither can see a compute
/// container even if one somehow wore a matching label.
///
/// Best-effort throughout: a reap failure is a log line, never a boot failure.
async fn reap(backend: &provider_host::SandboxBackend, grant: &ServiceGrant) {
    let provider_host::SandboxBackend::Podman { socket, .. } = backend else {
        // Tart clones and deletes a VM per session; there is no label to reap.
        return;
    };
    let mine = provider_host::managed_label(&grant.display_id());
    let retired = provider_host::managed_label(provider_host::NODE_TERM_OWNER);
    for (label, reason) in [
        (mine.as_str(), "own_orphans"),
        (retired.as_str(), "retired_in_node_pty"),
    ] {
        match provider_host::reap_by_label(socket, label).await {
            Ok(0) => {}
            Ok(removed) => tracing::info!(
                target: "ducktape::service",
                removed,
                reason,
                "reaped orphaned session containers"
            ),
            Err(error) => tracing::warn!(
                target: "ducktape::service",
                reason = "reap_failed",
                "could not sweep session containers: {error}"
            ),
        }
    }
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
        assert_eq!(ws_url("http://127.0.0.1:8844/"), "ws://127.0.0.1:8844/v1/ws");
    }

    #[test]
    fn the_managed_label_separates_agent_from_compute_and_from_the_node() {
        // the co-tenancy guarantee, as an assertion: three disjoint label
        // namespaces, so no reaper can ever sweep another service's containers.
        let agent = provider_host::managed_label("agent#deadbeef");
        let compute = provider_host::managed_label("compute#deadbeef");
        let node_pty = provider_host::managed_label(provider_host::NODE_TERM_OWNER);
        assert_eq!(agent, "io.ducktape.managed=agent#deadbeef");
        assert_ne!(agent, compute);
        assert_ne!(agent, node_pty);
        assert_ne!(agent, provider_host::RETIRED_FLAT_MANAGED_LABEL);
    }
}
