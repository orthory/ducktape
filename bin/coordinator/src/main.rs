use std::net::SocketAddr;

use coordinator_bin::select_policy;
use nat_traversal::run_coordinator;
use tokio::net::UdpSocket;

const USAGE: &str = "\
ducktape coordinator

Usage:
  coordinator [--listen <addr>] [--genesis-set <network.toml> | --allow-anonymous]

Options:
  --listen <addr>              UDP bind address [default: 0.0.0.0:3478]
  --genesis-set <network.toml> Private mode: pin admission to genesis validators
  --allow-anonymous            Legacy development mode: disable proof-of-possession
  -h, --help                   Print this help and exit

Default auth policy:
  public proof-of-possession (no --genesis-set and no --allow-anonymous)
";

fn arg_value(flag: &str) -> Option<String> {
    std::env::args().skip_while(|a| a != flag).nth(1)
}

fn validate_args(args: &[String]) -> std::io::Result<()> {
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--listen" | "--genesis-set" => {
                let flag = &args[i];
                let Some(value) = args.get(i + 1).filter(|v| !v.starts_with("--")) else {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("{flag} requires a value"),
                    ));
                };
                if flag == "--listen" {
                    parse_addr(flag, value)?;
                }
                i += 2;
            }
            "--allow-anonymous" => i += 1,
            "-h" | "--help" => i += 1,
            other => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("unknown coordinator flag {other:?}"),
                ));
            }
        }
    }
    Ok(())
}

fn wants_help() -> bool {
    std::env::args().any(|a| a == "--help" || a == "-h")
}

fn parse_addr(flag: &str, raw: &str) -> std::io::Result<SocketAddr> {
    raw.parse().map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{flag} {raw:?} is not a valid host:port: {e}"),
        )
    })
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    if wants_help() {
        print!("{USAGE}");
        return Ok(());
    }

    let args: Vec<String> = std::env::args().skip(1).collect();
    validate_args(&args)?;

    // `--listen <addr>` selects the bind; a malformed value is a HARD error, not
    // a silent fall-through to 0.0.0.0 — a typo'd flag or address must never
    // quietly expose the untrusted control port on every interface.
    let listen: SocketAddr = match arg_value("--listen") {
        Some(s) => parse_addr("--listen", &s)?,
        None => "0.0.0.0:3478".parse().expect("default addr parses"),
    };

    // The per-network authorization policy, selected from CLI flags:
    //   --genesis-set <network.toml>  => Private (PoP + pinned valset admission)
    //   --allow-anonymous             => fully-open (legacy, no auth)
    //   (no flag)                     => public with proof-of-possession
    // A malformed --genesis-set path/file is a HARD error, never a silent
    // fall-through to a weaker policy.
    let policy = select_policy(&args)?;

    let sock = UdpSocket::bind(listen).await?;
    // the address line stays parseable (tooling/tests read its tail).
    eprintln!("coordinator listening on {}", sock.local_addr()?);
    run_coordinator(sock, policy).await;
    Ok(())
}
