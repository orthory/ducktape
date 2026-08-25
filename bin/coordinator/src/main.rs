use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use coordinator_bin::{process_cpu_ns, process_rss_bytes, select_policy};
use nat_traversal::{
    Coordinator, CoordinatorMetrics, RelayMetrics, run_coordinator_workers_with_metrics_using,
    run_relay_listener,
};
use tokio::net::{TcpListener, UdpSocket};

const USAGE: &str = "\
ducktape coordinator

Usage:
  coordinator [--listen <addr>] [--relay-listen <addr|none>] [--workers <1|4>] [--metrics-interval <secs>] [--genesis-set <network.toml>]

Options:
  --listen <addr>              UDP bind address [default: 0.0.0.0:3478]
  --relay-listen <addr|none>   TCP relay-lane bind; \"none\" disables [default: 0.0.0.0:443]
  --workers <1|4>              Signature-verification workers [default: 1]
  --metrics-interval <secs>    Structured metrics period; 0 disables [default: 10]
  --genesis-set <network.toml> Private mode: pin admission to genesis validators
  -h, --help                   Print this help and exit

Default auth policy:
  public proof-of-possession (no --genesis-set)
";

fn arg_value(flag: &str) -> Option<String> {
    std::env::args().skip_while(|a| a != flag).nth(1)
}

fn validate_args(args: &[String]) -> std::io::Result<()> {
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--listen" | "--relay-listen" | "--workers" | "--metrics-interval"
            | "--genesis-set" => {
                let flag = &args[i];
                let Some(value) = args.get(i + 1).filter(|v| !v.starts_with("--")) else {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("{flag} requires a value"),
                    ));
                };
                if flag == "--listen" {
                    parse_addr(flag, value)?;
                } else if flag == "--relay-listen" {
                    parse_relay_listen(value)?;
                } else if flag == "--workers" {
                    parse_workers(value)?;
                } else if flag == "--metrics-interval" {
                    parse_metrics_interval(value)?;
                }
                i += 2;
            }
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

/// `--relay-listen` takes an addr like `--listen`, or the literal `none` to
/// disable the lane. A malformed value is a HARD error, same contract as
/// `--listen`: a typo must never silently change what gets bound.
fn parse_relay_listen(raw: &str) -> std::io::Result<Option<SocketAddr>> {
    if raw == "none" {
        return Ok(None);
    }
    parse_addr("--relay-listen", raw).map(Some)
}

fn parse_workers(raw: &str) -> std::io::Result<usize> {
    match raw {
        "1" => Ok(1),
        "4" => Ok(4),
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("--workers must be 1 or 4, got {raw:?}"),
        )),
    }
}

fn parse_metrics_interval(raw: &str) -> std::io::Result<u64> {
    raw.parse().map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("--metrics-interval {raw:?} is not seconds: {error}"),
        )
    })
}

async fn log_metrics(metrics: CoordinatorMetrics, relay_metrics: RelayMetrics, seconds: u64) {
    let mut ticker = tokio::time::interval(Duration::from_secs(seconds));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    ticker.tick().await;
    let mut last_wall = Instant::now();
    let mut last_cpu = process_cpu_ns();

    loop {
        ticker.tick().await;
        let elapsed = last_wall.elapsed();
        last_wall = Instant::now();
        let cpu = process_cpu_ns();
        let cpu_pct = match (last_cpu, cpu) {
            (Some(before), Some(after)) => {
                format!(
                    "{:.1}",
                    after.saturating_sub(before) as f64 / elapsed.as_nanos() as f64 * 100.0
                )
            }
            _ => "na".into(),
        };
        last_cpu = cpu;
        let m = metrics.snapshot();
        let r = relay_metrics.snapshot();
        let rss_mib = process_rss_bytes()
            .map(|rss| format!("{:.2}", rss as f64 / 1_048_576.0))
            .unwrap_or_else(|| "na".into());
        eprintln!(
            "coordinator_metrics | traffic received={} authenticated={} rejected={} malformed={} replies={} send_errors={} | queue inflight={} inflight_max={} saturated={} | relay sessions={} rejected={} forwards={} replies={} expired={} | host cpu_pct={cpu_pct} rss_mib={rss_mib}",
            m.received,
            m.authenticated,
            m.rejected,
            m.malformed,
            m.replies,
            m.send_errors,
            m.inflight,
            m.inflight_max,
            m.saturated,
            r.sessions_opened,
            r.sessions_rejected,
            r.forwards,
            r.replies,
            r.expired,
        );
    }
}

// UDP I/O and ordered state stay on one current-thread runtime. `--workers 4`
// adds only the fixed signature-verification pool.
#[tokio::main(flavor = "current_thread")]
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

    // `--relay-listen <addr|none>` selects the TCP relay-lane bind; the same
    // hard-error contract as `--listen` (only the literal "none" disables).
    let relay_listen = match arg_value("--relay-listen") {
        Some(raw) => parse_relay_listen(&raw)?,
        None => Some("0.0.0.0:443".parse().expect("default relay addr parses")),
    };

    // The per-network authorization policy, selected from CLI flags:
    //   --genesis-set <network.toml>  => Private (PoP + pinned valset admission)
    //   (no flag)                     => public with proof-of-possession
    // A malformed --genesis-set path/file is a HARD error, never a silent
    // fall-through to a weaker policy. The Arc is shared verbatim with the
    // relay lane: ONE policy gates both the UDP loops and the TCP relay.
    let policy = Arc::new(select_policy(&args)?);
    let workers = match arg_value("--workers") {
        Some(raw) => parse_workers(&raw)?,
        None => 1,
    };
    let metrics_interval = match arg_value("--metrics-interval") {
        Some(raw) => parse_metrics_interval(&raw)?,
        None => 10,
    };

    let sock = UdpSocket::bind(listen).await?;
    // the address line stays parseable (tooling/tests read its tail).
    eprintln!("coordinator listening on {}", sock.local_addr()?);

    // One Coordinator serves the UDP loops AND lends its advert book to the
    // relay, so a relayed intro resolves targets from the SAME registry the
    // rendezvous keepalives maintain.
    let coord = Coordinator::with_shared_policy(policy.clone());
    let relay_metrics = RelayMetrics::default();
    if let Some(relay_addr) = relay_listen {
        match TcpListener::bind(relay_addr).await {
            Ok(listener) => {
                // parseable like the UDP line above (tooling/tests read its tail).
                eprintln!(
                    "coordinator relay listening on tcp/{}",
                    listener.local_addr()?
                );
                tokio::spawn(run_relay_listener(
                    listener,
                    policy.clone(),
                    coord.adverts(),
                    relay_metrics.clone(),
                ));
            }
            Err(error) => {
                // The relay is a FALLBACK lane: failing to bind it (EACCES on
                // 443 as non-root is the everyday dev case) must not take the
                // UDP rendezvous down with it. Warn loudly and keep serving.
                eprintln!("WARNING: relay lane disabled: binding tcp/{relay_addr} failed: {error}");
            }
        }
    }

    let metrics = CoordinatorMetrics::default();
    if metrics_interval != 0 {
        tokio::spawn(log_metrics(
            metrics.clone(),
            relay_metrics.clone(),
            metrics_interval,
        ));
    }
    run_coordinator_workers_with_metrics_using(
        nat_traversal::NatSocket::Owned(sock),
        coord,
        workers,
        metrics,
    )
    .await;
    Ok(())
}
