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
    pub(crate) voice_requests: tokio::sync::mpsc::Receiver<noded::CallSessionRequest>,
    pub(crate) blobs: noded::blobs::BlobHandle,
    pub(crate) agent_provisioner: Option<dispatch_oracle::SharedProvisioner>,
    pub(crate) gateway_requests: Option<tokio::sync::mpsc::Receiver<noded::GatewayJob>>,
    pub(crate) gateway_commands: futures::channel::mpsc::Sender<noded::NodeCommand>,
}

pub(crate) struct BindConfig<'a> {
    pub(crate) sync_only: bool,
    pub(crate) label: &'a str,
    pub(crate) storage: &'a std::path::Path,
    pub(crate) rpc_listen: Option<String>,
    pub(crate) http_listen: Option<String>,
    pub(crate) gateway_listen: Option<String>,
    pub(crate) gateway_enabled: bool,
    pub(crate) log_ring: noded::LogRing,
    /// this node's signer identity — the COMMITTER on every forge run commit
    /// (D2: author is the agent, committer is the node).
    pub(crate) forge_committer: String,
}

pub(crate) fn bind(config: BindConfig<'_>) -> Result<Surfaces, Box<dyn std::error::Error>> {
    let BindConfig {
        sync_only,
        label,
        storage,
        rpc_listen,
        http_listen,
        gateway_listen,
        gateway_enabled,
        log_ring,
        forge_committer,
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
            println!("[node {label}] gateway browser listening on http://{actual}");
            Some((listener, actual))
        }
        _ => None,
    };
    let (gateway_lane, gateway_requests) = tokio::sync::mpsc::channel::<noded::GatewayJob>(32);
    // the derived per-module index (noded's exact store, <storage>/index),
    // plus the blocks database the explorer reads: the pump folds sealed
    // blocks into it, boot heals it from verified state at sync/recovery
    // boundaries, a resident's follow arm heals it at every state-changing
    // boundary it serves, and the already-routed GET /v1/blocks +
    // /v1/index/* lanes light up through the handle below. an open failure
    // is fatal-with-remedy rather than a silent no-index run: the tier is
    // rebuildable, so the fix is always "delete <storage>/index".
    let index = noded::open_index_store(storage, &MODULE_IDS)?;
    stream_hub.prime(index.resume_height()?, String::new());
    // the voice hub's session lane: /v1/call/ws handlers ask for huddle
    // audio sessions here. created up front because the app-surface thread
    // starts before the mesh exists; only the validator path below spawns the
    // hub that drains it — on every other path the receiver just drops and
    // the route answers with a refusal.
    let (voice_lane, voice_requests) = tokio::sync::mpsc::channel::<noded::CallSessionRequest>(8);
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
        // the duckfs workspace RPC's managed-checkout root (disk state, separate
        // from the module's own `<storage>/duckfs` dir).
        .with_duckfs_workspaces(storage.join("duckfs-workspaces"));
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
    let agent_provisioner: Option<dispatch_oracle::SharedProvisioner> = Some(std::sync::Arc::new(
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
        ),
    ));
    // (like the rpc surface above, a joiner binds and the park loop pumps —
    // reads only until promotion re-execs this process into a validator.)
    match http_listen.as_deref() {
        Some(addr) if !sync_only => {
            let listener = std::net::TcpListener::bind(addr)?;
            listener.set_nonblocking(true)?;
            println!(
                "[node {label}] app surface listening on http://{}",
                listener
                    .local_addr()
                    .map(|a| a.to_string())
                    .unwrap_or_default()
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
                                        eprintln!("gateway browser server error: {error}");
                                    }
                                });
                            }
                            let listener = tokio::net::TcpListener::from_std(listener)
                                .expect("adopt app-surface listener");
                            if let Err(e) = noded::serve(listener, http_handle).await {
                                eprintln!("app surface server error: {e}");
                            }
                        });
                    // a client asked the surface to shut down (POST /v1/shutdown) —
                    // mirror the rpc shutdown: exit the whole process gracefully.
                    println!("[node {thread_label}] shutdown requested via app surface — exiting");
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
        blobs,
        agent_provisioner,
        gateway_requests: gateway_enabled.then_some(gateway_requests),
        gateway_commands,
    })
}
