use std::sync::Arc;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ducktape")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// run the http document server (the default when no subcommand is given).
    Serve,

    /// start a p2p node. behaviour is driven by the config:
    /// - no `--config`, or a config WITHOUT a `listen` addr → single-process
    ///   two-node loopback demo (drives an op into node A, logs node B converging).
    /// - a config WITH a `listen` addr → a REAL commonware p2p node that meshes
    ///   with the `peer_seeds` over real sockets and converges across OS processes.
    Node {
        /// path to a toml node config. omit for built-in loopback-demo defaults.
        #[arg(long)]
        config: Option<std::path::PathBuf>,
    },
}

/// deliberately NOT `#[tokio::main]`. the real-node path stands up commonware's
/// OWN tokio runtime via `commonware_runtime::tokio::Runner`, and you cannot
/// start a runtime from inside an existing one ("cannot start a runtime from
/// within a runtime" panics). so `main` is sync and each path builds exactly the
/// runtime it needs: the serve/loopback paths `block_on` an explicit tokio
/// runtime; the commonware path hands off to `Runner::start`, which owns its
/// runtime end-to-end.
fn main() -> Result<(), std::io::Error> {
    let cli = Cli::parse();
    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => tokio::runtime::Runtime::new()?.block_on(serve()),
        Command::Node { config } => {
            let cfg = load_node_config(config)?;
            if cfg.is_commonware() {
                // real p2p node — owns its own runtime (must be outside tokio).
                run_commonware_node(cfg)
            } else {
                // loopback demo — fine on a plain tokio runtime.
                tokio::runtime::Runtime::new()?
                    .block_on(async move { engine::run_loopback_demo(cfg).await });
                Ok(())
            }
        }
    }
}

/// the http document server path (unchanged from before the cli split).
async fn serve() -> Result<(), std::io::Error> {
    let listen_addr = "0.0.0.0:21922".to_string();
    let base_path = ".".to_string();
    let document_service = api::services::document::DocumentService::new(base_path);
    api::http::server::create_server(listen_addr, Arc::new(document_service)).await;
    Ok(())
}

/// load (or default) a node config. with no path, the built-in default is `id 0`
/// with no `listen` → the loopback demo.
fn load_node_config(
    config_path: Option<std::path::PathBuf>,
) -> Result<engine::Config, std::io::Error> {
    match config_path {
        Some(path) => engine::Config::from_path(&path),
        None => engine::Config::from_toml_str("id = 0\n")
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
    }
}

/// count a workspace's top-level entries (seed + whatever propagated in).
fn entry_count(ws: &workspace::Workspace) -> usize {
    match ws.root().as_ref() {
        workspace::Entry::Directory(items) => items.len(),
        workspace::Entry::File(_) => 1,
    }
}

/// parse a socket addr, surfacing a bad addr as an io error rather than a panic.
fn parse_addr(s: &str) -> Result<std::net::SocketAddr, std::io::Error> {
    s.parse().map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("bad socket addr {s:?}: {e}"),
        )
    })
}

/// stand up a REAL commonware p2p node from `cfg` and run it until killed.
///
/// the de-nesting lives here: this is called from a SYNC `main`, so
/// `Runner::start` (which builds + owns commonware's tokio runtime) is free to
/// spin up its own runtime. everything node-side runs INSIDE the `start`
/// closure, on that runtime — including `engine::Node`'s internal `tokio::spawn`
/// tasks, which find the ambient runtime because `start` drives via `block_on`.
fn run_commonware_node(cfg: engine::Config) -> Result<(), std::io::Error> {
    use commonware_cryptography::{ed25519, Signer};
    use commonware_runtime::{Clock, Runner, Spawner, Supervisor as _};
    use std::time::Duration;
    use transport::Transport as _; // brings `.send` into scope for the resend loop.

    // --- map engine::Config (plain data) → net::Config (commonware types) ----
    // engine stays transport-agnostic, so the bin owns this translation: seeds →
    // ed25519 keys, addr strings → SocketAddrs, the namespace → bytes.
    let id = cfg.id;
    let listen = parse_addr(
        cfg.listen
            .as_deref()
            .expect("is_commonware() guarantees listen is set"),
    )?;
    let advertised = match cfg.advertised.as_deref() {
        Some(a) => parse_addr(a)?,
        None => listen, // advertised defaults to listen.
    };
    let namespace = cfg
        .namespace
        .clone()
        .unwrap_or_else(|| "ducktape-local".to_string())
        .into_bytes();
    // each seed → an ed25519 identity; together the authorized participant set.
    let peers: Vec<ed25519::PublicKey> = cfg
        .peer_seeds
        .iter()
        .map(|s| ed25519::PrivateKey::from_seed(*s).public_key())
        .collect();
    // node 0 bootstraps nobody; everyone else dials node 0 (= peer_seeds[0]).
    let bootstrappers = if id == 0 {
        Vec::new()
    } else {
        let boot_seed = *cfg.peer_seeds.first().expect(
            "a bootstrapping node needs a non-empty peer_seeds (peer_seeds[0] = node 0)",
        );
        let boot_key = ed25519::PrivateKey::from_seed(boot_seed).public_key();
        let boot_addr = parse_addr(
            cfg.bootstrapper_addr
                .as_deref()
                .expect("a non-zero node needs bootstrapper_addr set"),
        )?;
        vec![(boot_key, boot_addr)]
    };
    let net_cfg = net::Config {
        seed: id as u64,
        namespace,
        listen,
        advertised,
        peers,
        bootstrappers,
    };

    println!(
        "[node #{id}] starting commonware node on {listen} ({} peers)",
        cfg.peer_seeds.len()
    );

    // --- run on commonware's OWN runtime (it owns its tokio runtime) ---------
    let executor = commonware_runtime::tokio::Runner::default();
    executor.start(|context| async move {
        // build the real transport + live simplex consensus engine. KEEP
        // `engine_handle` alive for the whole closure — dropping it aborts the
        // consensus engine, which would make this a non-faithful node.
        let (transport, inbox, engine_handle) =
            net::CommonwareTransport::new(context.child("node"), net_cfg);
        let _engine_handle = engine_handle;

        // clone the transport for the resend loop BEFORE it moves into the Node.
        // only node 0 resends, so only node 0 needs the clone.
        let resend_transport = if id == 0 { Some(transport.clone()) } else { None };

        // a frontmatter-only seed doc so every node starts from an identical tree
        // (the SEED pattern lifted from engine::run_loopback_demo).
        const SEED: &str = "---\ntitle: demo\nauthor: @a\ncreated_at: 1\nupdated_at: 1\n---\n";
        let seed = document::Document::from_reader(SEED.as_bytes()).expect("seed doc parses");
        let ws = workspace::Workspace::new_from_entry(workspace::Entry::Directory(vec![(
            "seed.md".into(),
            workspace::Entry::File(seed),
        )]));

        let node = engine::Node::new(cfg, engine::Engine::new(ws), transport, inbox);
        println!(
            "[node #{id}] up: {} entries, awaiting mesh...",
            entry_count(&node.workspace_snapshot().await)
        );

        // node 0 drives one AddEntry into the mesh, then resends it on a 250ms
        // loop: right after start the mesh isn't formed, so a single gossip
        // reaches nobody and is silently dropped — keep sending until peers have
        // converged (exactly how commonware's own connectivity tests drive sends).
        let _resend_handle = if id == 0 {
            let new_doc =
                document::Document::from_reader(SEED.as_bytes()).expect("new doc parses");
            let op = op::Op::Workspace(workspace::op::Op::AddEntry {
                path: "added-by-node0.md".into(),
                entry: workspace::Entry::File(new_doc),
            });
            // encode the wire bytes BEFORE apply consumes the op (op::Op is !Clone).
            let bytes = transport::encode_batch(std::slice::from_ref(&op));
            node.apply_local_direct(op)
                .await
                .expect("node 0 applies + propagates the AddEntry");
            println!(
                "[node #{id}] applied AddEntry locally: {} entries",
                entry_count(&node.workspace_snapshot().await)
            );
            let resend_transport = resend_transport.expect("node 0 cloned its transport");
            // hold the spawn handle so the resend task isn't aborted on drop.
            Some(context.child("resend").spawn(move |ctx| async move {
                loop {
                    let _ = resend_transport
                        .send(op::Lane::Broadcast, bytes.clone())
                        .await;
                    ctx.sleep(Duration::from_millis(250)).await;
                }
            }))
        } else {
            None
        };

        // ALL nodes: park on inbound batches forever, logging the FIRST time the
        // tree grows past its seed. one-shot (gated on `converged`) so a resend
        // that re-applies the AddEntry doesn't spam the log. this infinite loop
        // IS the "run forever" park — the process stays up until killed.
        let mut converged = false;
        loop {
            node.wait_inbound().await;
            let n = entry_count(&node.workspace_snapshot().await);
            if !converged && n > 1 {
                println!("[node #{id}] converged: {n} entries");
                converged = true;
            }
        }
    });

    Ok(())
}
