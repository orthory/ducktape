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

    /// start a p2p node. with no `--config` this runs a single-process two-node
    /// loopback demo that drives an op into node A and logs node B converging.
    Node {
        /// path to a toml node config (id, peers, ...). omit for built-in
        /// loopback-demo defaults.
        #[arg(long)]
        config: Option<std::path::PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let cli = Cli::parse();
    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => serve().await,
        Command::Node { config } => node(config).await,
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

/// load (or default) a node config and run the loopback demo. the heavy lifting
/// lives in `engine::run_loopback_demo` so this stays a thin shim.
async fn node(config_path: Option<std::path::PathBuf>) -> Result<(), std::io::Error> {
    let config = match config_path {
        Some(path) => engine::Config::from_path(&path)?,
        // built-in default: id 0, no peers, fast drain.
        None => engine::Config::from_toml_str("id = 0\n")
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?,
    };
    engine::run_loopback_demo(config).await;
    Ok(())
}
