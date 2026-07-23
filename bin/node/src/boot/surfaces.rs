use crate::constants::MODULE_IDS;

/// `run_node`'s out-of-runtime surface bring-up (phase P1): the listener
/// binds that must fail as a clean startup error rather than an async
/// surprise, plus the app-surface HTTP server's own OS thread and the
/// channels/stores every later phase pumps.
pub(crate) struct Surfaces {
    pub(crate) rpc_listener: Option<std::net::TcpListener>,
    pub(crate) http_cmds: futures::channel::mpsc::Receiver<noded::NodeCommand>,
    pub(crate) stream_hub: noded::StreamHub,
    pub(crate) index: std::sync::Arc<indexer::IndexStore>,
    pub(crate) voice_requests: tokio::sync::mpsc::Receiver<noded::RealtimeSessionRequest>,
    pub(crate) code_stage_requests: tokio::sync::mpsc::Receiver<noded::CodeStageRequest>,
    pub(crate) blobs: noded::blobs::BlobHandle,
    pub(crate) agent_provisioner: dispatch_oracle::SharedProvisioner,
    pub(crate) gateway_requests: Option<tokio::sync::mpsc::Receiver<noded::GatewayJob>>,
    pub(crate) gateway_commands: futures::channel::mpsc::Sender<noded::NodeCommand>,
    /// the host-side session manager (a clone of the one on the http handle), so
    /// the term plane's control handler can spawn peer-attached sessions. `None`
    /// on a node that hosts no terminal plane (Direct / sync-only / joiner).
    pub(crate) terminals: Option<noded::TerminalSessions>,
    /// the guest-side remote-session lane the term plane's client half drains.
    pub(crate) session_requests: tokio::sync::mpsc::Receiver<noded::SessionJob>,
    /// the host's own browser-gateway base URL — the `via` a resolved credential
    /// routes through. Empty when no browser gateway is bound.
    pub(crate) local_gateway_via: String,
}

pub(crate) struct BindConfig<'a> {
    pub(crate) sync_only: bool,
    /// a not-yet-admitted joiner: it binds and serves http reads-only while
    /// parked, but must NOT host the interactive terminal plane (no standing).
    pub(crate) joiner: bool,
    pub(crate) label: &'a str,
    pub(crate) storage: &'a std::path::Path,
    /// the config dir where `gateway-routes.json` lives (= `storage` in the dev
    /// shape). An embedded airlock gateway registers its loopback port here so
    /// the gateway proxy can find it.
    pub(crate) workspace: &'a std::path::Path,
    pub(crate) rpc_listen: Option<String>,
    pub(crate) http_listen: Option<String>,
    pub(crate) gateway_listen: Option<String>,
    pub(crate) gateway_enabled: bool,
    pub(crate) log_ring: noded::LogRing,
    /// this node's signer identity — the COMMITTER on every forge run commit
    /// (D2: author is the agent, committer is the node).
    pub(crate) forge_committer: String,
    /// this node's consensus public key — the `BindNode` subject the owner-gated
    /// admin namespace resolves ownership against (ADR A5).
    pub(crate) node_key: Vec<u8>,
    /// how the owner-gated admin namespace is exposed (ADR A2/A4).
    pub(crate) admin_exposure: noded::AdminExposure,
    /// how provider runs are spawned (`node.toml sandbox`): Direct, or a
    /// Podman/Tart image. The interactive terminal plane requires Podman/Tart,
    /// so a Direct node hosts no terminal plane.
    pub(crate) sandbox: capability_host::SandboxBackend,
}

pub(crate) fn bind(config: BindConfig<'_>) -> Result<Surfaces, Box<dyn std::error::Error>> {
    let BindConfig {
        sync_only,
        joiner,
        label,
        storage,
        workspace,
        rpc_listen,
        http_listen,
        gateway_listen,
        gateway_enabled,
        log_ring,
        forge_committer,
        node_key,
        admin_exposure,
        sandbox,
    } = config;
    // the rpc listener binds OUTSIDE the runtime (plain std tcp on OS threads)
    // so a bind failure is a clean startup error, not an async surprise. a
    // JOINER binds too: the park loop pumps the same surface — a resident
    // serves local reads from its pre-synced boundary, a still-parked joiner
    // answers with a clear not-admitted error instead of a dead port.
    let rpc_listener = match rpc_listen.as_deref() {
        Some(addr) if !sync_only => Some(std::net::TcpListener::bind(addr)?),
        _ => None,
    };
    // the http/ws app surface: same bind-early rule. the server itself runs on
    // its OWN plain-tokio OS thread (noded's exact split — the host never
    // leaves the commonware runner thread; http handlers only send
    // NodeCommands over the lane), so the pump below is its single consumer.
    let (http_handle, http_cmds, stream_hub) = noded::NodeHandle::channel_with_log_ring(log_ring);
    let gateway_listener = match (gateway_listen.as_deref(), http_listen.as_deref()) {
        (Some(addr), Some(_)) if !sync_only && gateway_enabled => {
            let address: std::net::SocketAddr = addr
                .parse()
                .map_err(|error| format!("invalid gateway_listen {addr:?}: {error}"))?;
            if address.ip() != std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST) {
                return Err("gateway_listen must bind exactly 127.0.0.1".into());
            }
            let listener = std::net::TcpListener::bind(address)?;
            listener.set_nonblocking(true)?;
            let actual = listener.local_addr()?;
            tracing::info!(
                target: "ducktape::gateway",
                node = %label,
                listen = %actual,
                "gateway browser listening"
            );
            Some((listener, actual))
        }
        _ => None,
    };
    // An embedded airlock gateway (credential-provider node): either the
    // disk-backed self-host store (`user cred add`) or the TEE env path
    // (DUCKTAPE_AIRLOCK_SERVE). Run it in-process on loopback and register its
    // port as the `airlock` gateway route, so a compute node can reach it over
    // the overlay (airlock.<handle>.duck). Bound here (out of the runtime) like
    // the browser gateway; served on the app-surface thread below. Only when the
    // gateway plane is up to serve it; route PUBLICATION stays a one-time signed
    // operator step.
    let airlock_bits = match crate::airlock_serve::AirlockServe::resolve(storage) {
        None => None,
        Some(Err(error)) => return Err(format!("airlock serve config: {error}").into()),
        Some(Ok(serve)) if !sync_only && gateway_enabled => {
            let listener = std::net::TcpListener::bind((
                std::net::Ipv4Addr::LOCALHOST,
                serve.port.unwrap_or(0),
            ))?;
            listener.set_nonblocking(true)?;
            let port = listener.local_addr()?.port();
            // Build — and thus ATTEST — BEFORE registering the route or claiming
            // to listen: a node that cannot attest must fail boot loudly here,
            // never register a route to a gateway that will not come up.
            let (router, vendor) = airlock::server::build_seeded(serve.cfg, serve.seeds)
                .map_err(|error| format!("airlock gateway: {error}"))?;
            crate::gateway_routes::register(workspace, gateway::RouteName::named("airlock"), port)
                .map_err(|error| format!("register airlock gateway route: {error}"))?;
            tracing::info!(
                target: "ducktape::gateway",
                node = %label,
                listen = %format_args!("127.0.0.1:{port}"),
                route = "airlock",
                attest = %vendor,
                "airlock gateway listening"
            );
            Some((listener, router))
        }
        Some(Ok(_)) => {
            tracing::warn!(
                target: "ducktape::gateway",
                node = %label,
                reason = "gateway_plane_off",
                "DUCKTAPE_AIRLOCK_SERVE set but airlock is not served"
            );
            None
        }
    };
    let (gateway_lane, gateway_requests) = tokio::sync::mpsc::channel::<noded::GatewayJob>(32);
    // the guest-side remote-session lane: /v1/term/sessions with a `node` hands a
    // SessionJob here, drained by the term plane's client half (mirrors the
    // gateway lane). The host's own browser-gateway base URL is the `via` a
    // resolved credential routes through.
    let (session_lane, session_requests) = tokio::sync::mpsc::channel::<noded::SessionJob>(32);
    let local_gateway_via = gateway_listener
        .as_ref()
        .map(|(_, address)| format!("http://{address}"))
        .unwrap_or_default();
    // the derived per-module index (noded's exact store, <storage>/index),
    // plus the blocks database the explorer reads: the pump folds sealed
    // blocks into it, boot heals it from verified state at sync/recovery
    // boundaries, a resident's follow arm heals it at every state-changing
    // boundary it serves, and the already-routed GET /v1/blocks +
    // /v1/index/* lanes light up through the handle below. an open failure
    // is fatal-with-remedy rather than a silent no-index run: the tier is
    // rebuildable, so the fix is always "delete <storage>/index".
    let index = noded::open_index_store(storage, MODULE_IDS)?;
    stream_hub.prime(index.resume_height()?, String::new());
    // the realtime hub's session lane: /v1/call/ws and /v1/presence/ws ask for
    // sessions here. created up front because the app-surface thread starts
    // before the mesh exists; validator and resident paths drain it, while a
    // sync-only or overlay-less path drops it so the routes refuse promptly.
    let (voice_lane, voice_requests) =
        tokio::sync::mpsc::channel::<noded::RealtimeSessionRequest>(8);
    // the module-code stage lane: POST /v1/admin/module-code/stage fans an
    // artifact out through the node's code plane. same shape as the realtime
    // lane — created up front, drained only where the validator spawns the
    // plane; elsewhere the receiver drops and the route answers 503.
    let (code_stage_lane, code_stage_requests) =
        tokio::sync::mpsc::channel::<noded::CodeStageRequest>(4);
    // point the http handle at this node's forge repo base (the same
    // `storage/forge-repo` the host materializes into) so the git upload-pack
    // (clone/fetch) route can open a repo READ-ONLY and serve its objects.
    let http_handle = http_handle
        // persist node-local blobs (op receipts, agent prompt pins) under
        // <storage>/blobstore so a daemon restart keeps serving them.
        .with_blob_root(storage.join("blobstore"))?
        .with_forge_repo(storage.join("forge-repo"))
        .with_index_store(index.clone())
        .with_call(voice_lane)
        .with_code_stage(code_stage_lane)
        // the duckfs workspace RPC's managed-checkout root (disk state, separate
        // from the module's own `<storage>/duckfs` dir).
        .with_duckfs_workspaces(storage.join("duckfs-workspaces"))
        // the owner-gated control namespace (ADR A2/A5): this node's own key is
        // the `BindNode` subject ownership resolves against; the exposure is the
        // operator's choice (default loopback). shutdown + module-code staging
        // live here, off the unauthenticated public surface.
        .with_admin(noded::AdminConfig {
            exposure: admin_exposure,
            node_key: Some(node_key.clone()),
            ..Default::default()
        });
    let http_handle = if gateway_enabled {
        http_handle.with_gateway(gateway_lane)
    } else {
        drop(gateway_lane);
        http_handle
    };
    let http_handle = match gateway_listener.as_ref() {
        Some((_, address)) => http_handle.with_browser_gateway(*address),
        None => http_handle,
    };
    let blobs = http_handle.blob_handle();
    let gateway_commands = http_handle.command_sender();
    // the REAL portable-agent-run provisioner, built from a clone of the http
    // handle BEFORE the serve/drop match consumes it. portable (v3) runs
    // materialize a per-run duckfs checkout under a root VALIDATED to be
    // outside <storage> (D7) and drive checkout/commit over this SAME
    // NodeHandle actor lane the /v1/fs/workspaces RPC already rides here.
    // LIVE for every agent run: this binary wires the files module
    // unconditionally, so the runs composer emits v3 (the de-versioned
    // activation — no flag day, pre-production re-genesis). a misconfigured
    // root (inside <storage>) is a boot error, never a silent D7 hole.
    let agent_provisioner: dispatch_oracle::SharedProvisioner = std::sync::Arc::new(
        noded::agent_provision::NodedProvisioner::new(
            http_handle.clone(),
            noded::agent_provision::agent_runs_root(storage)
                .unwrap_or_else(|e| panic!("agent runs root failed D7 validation: {e}")),
        )
        // the forge worktree lane (agent-dogfood M1): repos come off the
        // handle's forge base (the same <storage>/forge-repo the host
        // materializes into); pushes dial THIS node's own http surface at
        // loopback (mirroring the serve condition below — no surface, no push
        // lane, and forge runs fail loudly at provision); the committer on
        // every run commit is this node's signer identity (D2 — the author is
        // the agent).
        .with_forge(
            noded::agent_provision::forge_push_base(
                http_listen.as_deref().filter(|_| !sync_only),
            ),
            forge_committer,
        )
        // the agent tool plane: the SAME surface, bare (no /forge), handed to
        // every run as DUCKTAPE_NODE alongside the running binary's dir on
        // PATH — that is how the MCP server the runner CLI spawns (outside the
        // agent's sandbox) finds `ducktape mcp` and the node it acts against.
        // no surface (a sync-only joiner) ⇒ nothing to dial ⇒ the var is unset.
        .with_node_url(noded::agent_provision::node_http_base(
            http_listen.as_deref().filter(|_| !sync_only),
        )),
    );
    // the node-local, off-chain interactive terminal-session plane (lives on the
    // http handle like the stream hub — never consensus). Wired only where the
    // app surface is actually served for a real member: not sync-only, not a
    // parked joiner, and only when an http address was configured. Sourced from
    // the node's OWN config — the resolved `node.toml sandbox` backend and this
    // node's signer identity (`node_key`) — so `discover_interactive` refuses
    // the Direct backend (no terminal plane) and Podman container reaping scopes
    // to the SAME execution id as the node's real agent runs (validator/run.rs
    // discovers its provider set under the same identity). Mirrors bin/noded's
    // wiring; a Podman node's create returns a session (or a clear spawn error),
    // a Direct node's a "requires a configured podman sandbox image" 503 — never
    // the "terminal sessions are not enabled" 503 that meant the plane was
    // missing entirely (this bug).
    let terminals = if !sync_only && !joiner && http_listen.is_some() {
        let interactive = noded::term::discover_interactive(
            &node_key,
            capability_host::AgentDirs::under(storage),
            sandbox,
        );
        tracing::info!(
            target: "ducktape::term",
            enabled = interactive.is_some(),
            "terminal_plane_ready"
        );
        Some(noded::TerminalSessions::new(
            interactive,
            capability_host::execution_node_id(&node_key),
            storage.join("term-sessions"),
            stream_hub.terminals(),
            stream_hub.term_commands(),
        ))
    } else {
        None
    };
    // the term plane's host side (control handler) takes a clone of the same
    // manager the http handle serves; the guest side drains the session lane.
    let http_handle = match terminals.clone() {
        Some(manager) => http_handle.with_terminals(manager).with_session_lane(session_lane),
        None => {
            drop(session_lane);
            http_handle
        }
    };
    // (like the rpc surface above, a joiner binds and the park loop pumps —
    // reads only until promotion re-execs this process into a validator.)
    match http_listen.as_deref() {
        Some(addr) if !sync_only => {
            let listener = std::net::TcpListener::bind(addr)?;
            listener.set_nonblocking(true)?;
            tracing::info!(
                target: "ducktape::http",
                node = %label,
                listen = %listener.local_addr().map(|a| a.to_string()).unwrap_or_default(),
                "app surface listening"
            );
            let thread_label = label.to_string();
            let gateway_listener = gateway_listener.map(|(listener, _)| listener);
            let gateway_handle = http_handle.clone();
            std::thread::Builder::new()
                .name("app-surface".into())
                .spawn(move || {
                    tokio::runtime::Builder::new_multi_thread()
                        .enable_all()
                        .build()
                        .expect("app-surface tokio runtime")
                        .block_on(async move {
                            if let Some(listener) = gateway_listener {
                                let listener = tokio::net::TcpListener::from_std(listener)
                                    .expect("adopt gateway browser listener");
                                tokio::spawn(async move {
                                    if let Err(error) =
                                        noded::serve_browser_gateway(listener, gateway_handle).await
                                    {
                                        tracing::error!(
                                            target: "ducktape::gateway",
                                            error = %error,
                                            "gateway browser server stopped"
                                        );
                                    }
                                });
                            }
                            if let Some((airlock_listener, airlock_router)) = airlock_bits {
                                let airlock_listener =
                                    tokio::net::TcpListener::from_std(airlock_listener)
                                        .expect("adopt airlock gateway listener");
                                tokio::spawn(async move {
                                    if let Err(error) = airlock::server::serve_router(
                                        airlock_listener,
                                        airlock_router,
                                    )
                                    .await
                                    {
                                        tracing::error!(
                                            target: "ducktape::gateway",
                                            error = %error,
                                            "airlock gateway server stopped"
                                        );
                                    }
                                });
                            }
                            let listener = tokio::net::TcpListener::from_std(listener)
                                .expect("adopt app-surface listener");
                            if let Err(e) = noded::serve(listener, http_handle).await {
                                tracing::error!(
                                    target: "ducktape::http",
                                    error = %e,
                                    "app surface server stopped"
                                );
                            }
                        });
                    // a client asked the surface to shut down (POST /v1/admin/shutdown) —
                    // mirror the rpc shutdown: exit the whole process gracefully.
                    tracing::info!(
                        target: "ducktape::node",
                        node = %thread_label,
                        "shutdown requested via app surface; exiting"
                    );
                    std::process::exit(0);
                })?;
        }
        // surface off: dropping the handle terminates the command stream; the
        // pump's select arm sees one None and then never polls it again.
        _ => drop(http_handle),
    }

    Ok(Surfaces {
        rpc_listener,
        http_cmds,
        stream_hub,
        index,
        voice_requests,
        code_stage_requests,
        blobs,
        agent_provisioner,
        gateway_requests: gateway_enabled.then_some(gateway_requests),
        gateway_commands,
        terminals,
        session_requests,
        local_gateway_via,
    })
}
