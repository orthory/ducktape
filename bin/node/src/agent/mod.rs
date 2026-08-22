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
//! What crossed is exactly the sandbox-touching half: provider discovery,
//! `spawn_interactive`, and each session's pump and reaper. That is the whole
//! milestone — after this, `bin/node` constructs none of them.
//!
//! ## agent and compute are siblings
//!
//! This daemon makes no call of any kind to the compute daemon, and needs none:
//! an interactive session is self-contained. Both link the same
//! provider/sandbox/broker libraries and spawn their own sandboxes; their bus,
//! where they have one at all, is the chain.
//!
//! Co-tenancy is not a hazard here, and the microVM backend is why: a run's VMM
//! is a child of the daemon that started it and dies with it, so there is no
//! shared daemon, no shared storage root and no shared reaper to confuse. Run
//! ownership is still recorded per instance (`agent#<hex8>`) so a restarted
//! daemon can tell its own leftovers from a sibling's.
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

use crate::config;
use crate::services::ServiceGrant;

mod link;

/// Everything the daemon needs, resolved before any of it runs.
///
/// `service` is [`config::ServiceConfig`], NOT `config::Resolved`: the latter
/// carries the node's ed25519 private key, and the module doc above claims this
/// daemon owns no keypair — a claim the TYPE now makes true. `node_key` is the
/// node's PUBLIC identity, learned from the node over `/v1/status`; it names
/// this host's execution id and signs nothing.
pub(crate) struct Agent {
    pub(crate) grant: ServiceGrant,
    pub(crate) service: config::ServiceConfig,
    pub(crate) http_base: String,
    pub(crate) node_key: [u8; 32],
    /// where `node.toml` and the node's 0600 service-link token live.
    pub(crate) workspace: std::path::PathBuf,
}

/// Serve until the process is stopped. Returns only on a fatal
/// misconfiguration — a node that is merely down is retried forever, because
/// that is an operational state, not an error.
///
/// Through [`crate::services::serve_until_stopped`], which owns the runtime and
/// arms SIGTERM/SIGINT before a line of this daemon runs: a session's VMM is a
/// child of this process and must never outlive it holding guest memory.
pub(crate) fn serve(agent: Agent) -> Result<(), Box<dyn std::error::Error>> {
    crate::services::serve_until_stopped(std::future::pending(), |stop| run(agent, stop))
}

async fn run(
    agent: Agent,
    stop: crate::services::Stop,
) -> Result<(), Box<dyn std::error::Error>> {
    let Agent {
        grant,
        service,
        http_base,
        node_key,
        workspace,
    } = agent;
    let node_key = node_key.to_vec();
    let backend = crate::services::sandbox_backend(&service)?;
    // Fail-closed at BOOT rather than 150 s into a run: probe verifies
    // /dev/kvm opens read-write for this process and that the guest images
    // exist. A daemon that signals an interactive plane it cannot sandbox is
    // worse than one that refuses to start.
    backend.probe()?;
    // There is no daemon to start and nothing to sweep. Each run's VMM is a
    // child of this process spawned kill_on_drop, so a death that ran no code
    // still takes its guests with it.

    let providers = agent_service::discover(&node_key, backend, &grant.display_id())?;
    let offered = providers.capabilities().len();

    let (events, event_rx) = tokio::sync::mpsc::channel(link::EVENT_LANE);
    let sessions = Arc::new(agent_service::Sessions::new(
        providers,
        provider_host::execution_node_id(&node_key),
        service.storage_dir.join("term-sessions"),
        events,
    ));

    tracing::info!(
        target: "ducktape::service",
        instance = %grant.display_id(),
        capabilities = offered,
        cap = agent_service::MAX_TERM_SESSIONS,
        "agent daemon serving"
    );

    // the link never returns on its own: a dropped socket is ordinary (the node
    // restarts, the operator upgrades) and is redialed forever. A stop is what
    // ends this daemon — an attached session dies with the process either way,
    // and its container is taken down by the teardown below rather than left
    // running under a service that is about to go.
    tokio::select! {
        () = link::attach(ws_url(&http_base), workspace, sessions, event_rx) => {}
        () = stop => {}
    }
    // Nothing to tear down. Every live run's VMM is a child of this process
    // spawned kill_on_drop, so returning from here is the teardown — and that
    // covers SIGKILL too, which the container backend's sweep could not.
    tracing::info!(
        target: "ducktape::service",
        instance = %grant.display_id(),
        "agent daemon stopped"
    );
    Ok(())
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
    fn the_managed_label_separates_agent_from_compute() {
        // the co-tenancy guarantee as an assertion. It is the SECOND line of
        // defence — per-service graph roots already mean neither daemon can even
        // enumerate the other's containers — but a shared socket would be an
        // easy future mistake, and this is what would catch it.
        let agent = provider_host::managed_label("agent#deadbeef");
        assert_eq!(agent, "io.ducktape.managed=agent#deadbeef");
        assert_ne!(agent, provider_host::managed_label("compute#deadbeef"));
    }
}
