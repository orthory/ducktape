use std::net::SocketAddr;
use std::time::{Duration, Instant};

use coordinator_bin::{process_cpu_ns, process_rss_bytes, select_policy};
use nat_traversal::{CoordinatorMetrics, run_coordinator_workers_with_metrics};
use tokio::net::UdpSocket;

const USAGE: &str = "\
ducktape coordinator

Usage:
  coordinator [--listen <addr>] [--workers <1|4>] [--metrics-interval <secs>] [--genesis-set <network.toml> | --allow-anonymous]

Options:
  --listen <addr>              UDP bind address [default: 0.0.0.0:3478]
  --workers <1|4>              Signature-verification workers [default: 1]
  --metrics-interval <secs>    Structured metrics period; 0 disables [default: 10]
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
            "--listen" | "--workers" | "--metrics-interval" | "--genesis-set" => {
                let flag = &args[i];
                let Some(value) = args.get(i + 1).filter(|v| !v.starts_with("--")) else {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("{flag} requires a value"),
                    ));
                };
                if flag == "--listen" {
                    parse_addr(flag, value)?;
                } else if flag == "--workers" {
                    parse_workers(value)?;
                } else if flag == "--metrics-interval" {
                    parse_metrics_interval(value)?;
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

async fn log_metrics(metrics: CoordinatorMetrics, seconds: u64) {
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
        let rss_mib = process_rss_bytes()
            .map(|rss| format!("{:.2}", rss as f64 / 1_048_576.0))
            .unwrap_or_else(|| "na".into());
        eprintln!(
            "coordinator_metrics | traffic received={} authenticated={} rejected={} legacy={} malformed={} replies={} send_errors={} | queue inflight={} inflight_max={} saturated={} | host cpu_pct={cpu_pct} rss_mib={rss_mib}",
            m.received,
            m.authenticated,
            m.rejected,
            m.legacy,
            m.malformed,
            m.replies,
            m.send_errors,
            m.inflight,
            m.inflight_max,
            m.saturated,
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

    // The per-network authorization policy, selected from CLI flags:
    //   --genesis-set <network.toml>  => Private (PoP + pinned valset admission)
    //   --allow-anonymous             => fully-open (legacy, no auth)
    //   (no flag)                     => public with proof-of-possession
    // A malformed --genesis-set path/file is a HARD error, never a silent
    // fall-through to a weaker policy.
    let policy = select_policy(&args)?;
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
    let metrics = CoordinatorMetrics::default();
    if metrics_interval != 0 {
        tokio::spawn(log_metrics(metrics.clone(), metrics_interval));
    }
    run_coordinator_workers_with_metrics(sock, policy, workers, metrics).await;
    Ok(())
}
