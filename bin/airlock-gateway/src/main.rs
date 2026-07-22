//! `airlock-gateway` — the credential side of the airlock. A thin CLI wrapper
//! over [`airlock::server`]. Runs (canonically) inside an Intel TDX / AMD
//! SEV-SNP confidential VM: holds a sealed OAuth refresh token in enclave
//! memory, proxies the Claude messages API, and issues scoped session tokens.
//! The operator cannot read the credential.
//!
//! Subcommands:
//!   serve          the gateway itself
//!   mock-upstream  a fake OAuth + messages server, for the hermetic test/demo

mod mock_upstream;

use anyhow::{Context, Result};

use airlock::server::GatewayConfig;

/// `--flag value` / `--flag=value` lookup over argv (no clap — house rule).
fn arg(name: &str) -> Option<String> {
    let mut it = std::env::args();
    while let Some(a) = it.next() {
        if a == name {
            return it.next();
        }
        if let Some(v) = a.strip_prefix(&format!("{name}=")) {
            return Some(v.to_string());
        }
    }
    None
}

fn arg_or(name: &str, default: &str) -> String {
    arg(name).unwrap_or_else(|| default.to_string())
}

#[tokio::main]
async fn main() -> Result<()> {
    match std::env::args().nth(1).as_deref() {
        Some("serve") => serve().await,
        Some("mock-upstream") => mock_upstream::run(&arg_or("--listen", "127.0.0.1:9101")).await,
        other => {
            eprintln!(
                "usage: airlock-gateway <serve|mock-upstream> [flags]  (got {:?})",
                other.unwrap_or("")
            );
            std::process::exit(2);
        }
    }
}

async fn serve() -> Result<()> {
    let cfg = GatewayConfig {
        attest: arg("--attest").context("--attest is required (tdx|snp|auto)")?,
        anthropic_base: arg_or("--anthropic-base", "http://127.0.0.1:9101"),
        oauth_token_url: arg_or("--oauth-token-url", "http://127.0.0.1:9101/oauth/token"),
        oauth_client_id: arg_or("--oauth-client-id", "9d1c250a-e61b-44d9-88ed-5944d1962f5e"),
        session_ttl_secs: arg("--session-ttl-secs")
            .map(|s| s.parse::<u64>())
            .transpose()
            .context("--session-ttl-secs")?
            .unwrap_or(3600),
        max_requests: arg("--max-requests")
            .map(|s| s.parse::<u32>())
            .transpose()
            .context("--max-requests")?
            .unwrap_or(1000),
    };
    let (app, vendor) = airlock::server::build(cfg)?;
    let listener = tokio::net::TcpListener::bind(arg_or("--listen", "127.0.0.1:9100")).await?;
    eprintln!("[gateway] attest={vendor} listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}
