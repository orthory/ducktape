use std::io;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use commonware_cryptography::{Signer as _, ed25519};
use coordinator_bin::process_cpu_ns;
use nat_traversal::{AuthRequest, Msg, NodeKey, now_secs, sign_authenticator};
use tokio::net::{UdpSocket, lookup_host};
use tokio::task::JoinSet;

const USAGE: &str = "\
ducktape coordinator load probe

Usage:
  coordinator-load --target <host:port> [options]

Options:
  --duration <secs>          Total load duration [default: 10]
  --clients <count>          Valid request/response clients [default: 16]
  --invalid-clients <count>  Invalid-signature flood clients [default: 0]
  --timeout-ms <1..10000>    Per-request timeout [default: 1000]
  --rate <requests/sec>      Total valid request rate; 0 is unlimited [default: 0]
  --report-interval <secs>   Interval rows; 0 prints only summary [default: 60]
  --recovery <secs>          Valid-only phase after a flood [default: 5]
  --output <table|log>       Human table or key=value rows [default: table]
  -h, --help                 Print this help and exit
";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Output {
    Table,
    Log,
}

#[derive(Debug, PartialEq, Eq)]
struct Config {
    target: String,
    duration: Duration,
    clients: usize,
    invalid_clients: usize,
    timeout: Duration,
    rate: u64,
    report_interval: Duration,
    recovery: Duration,
    output: Output,
}

fn number<T>(flag: &str, raw: &str) -> Result<T, String>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    raw.parse().map_err(|error| format!("{flag}: {error}"))
}

fn parse_args(args: &[String]) -> Result<Config, String> {
    let mut target = None;
    let mut duration = 10;
    let mut clients = 16;
    let mut invalid_clients = 0;
    let mut timeout_ms = 1_000;
    let mut rate = 0;
    let mut report_interval = 60;
    let mut recovery = 5;
    let mut output = Output::Table;
    let mut index = 0;

    while index < args.len() {
        let flag = args[index].as_str();
        let value = args
            .get(index + 1)
            .filter(|value| !value.starts_with("--"))
            .ok_or_else(|| format!("{flag} requires a value"))?;
        match flag {
            "--target" => target = Some(value.clone()),
            "--duration" => duration = number(flag, value)?,
            "--clients" => clients = number(flag, value)?,
            "--invalid-clients" => invalid_clients = number(flag, value)?,
            "--timeout-ms" => timeout_ms = number(flag, value)?,
            "--rate" => rate = number(flag, value)?,
            "--report-interval" => report_interval = number(flag, value)?,
            "--recovery" => recovery = number(flag, value)?,
            "--output" => {
                output = match value.as_str() {
                    "table" => Output::Table,
                    "log" => Output::Log,
                    _ => return Err("--output must be table or log".into()),
                }
            }
            _ => return Err(format!("unknown flag {flag:?}")),
        }
        index += 2;
    }

    if duration == 0 {
        return Err("--duration must be greater than zero".into());
    }
    if !(1..=10_000).contains(&timeout_ms) {
        return Err("--timeout-ms must be between 1 and 10000".into());
    }
    if clients == 0 && invalid_clients == 0 {
        return Err("at least one valid or invalid client is required".into());
    }

    Ok(Config {
        target: target.ok_or_else(|| "--target is required".to_string())?,
        duration: Duration::from_secs(duration),
        clients,
        invalid_clients,
        timeout: Duration::from_millis(timeout_ms),
        rate,
        report_interval: Duration::from_secs(report_interval),
        recovery: Duration::from_secs(recovery),
        output,
    })
}

#[derive(Default)]
struct Histogram {
    buckets: Vec<u64>,
    samples: u64,
    total_us: u128,
}

impl Histogram {
    const BUCKET_US: u128 = 100;

    fn new(timeout: Duration) -> Self {
        Self {
            buckets: vec![0; (timeout.as_micros() / Self::BUCKET_US + 2) as usize],
            ..Self::default()
        }
    }

    fn record(&mut self, elapsed: Duration) {
        let micros = elapsed.as_micros();
        let index = (micros / Self::BUCKET_US) as usize;
        let last = self.buckets.len() - 1;
        self.buckets[index.min(last)] += 1;
        self.samples += 1;
        self.total_us += micros;
    }

    fn merge(&mut self, other: &Self) {
        for (left, right) in self.buckets.iter_mut().zip(&other.buckets) {
            *left += right;
        }
        self.samples += other.samples;
        self.total_us += other.total_us;
    }

    fn percentile_us(&self, percentile: u64) -> Option<u128> {
        if self.samples == 0 {
            return None;
        }
        let rank = self.samples.saturating_mul(percentile).div_ceil(100);
        let mut seen = 0;
        self.buckets
            .iter()
            .position(|count| {
                seen += count;
                seen >= rank
            })
            .map(|index| (index as u128 + 1) * Self::BUCKET_US)
    }

    fn average_us(&self) -> Option<u128> {
        (self.samples != 0).then(|| self.total_us / self.samples as u128)
    }
}

struct LoadStats {
    valid_sent: u64,
    valid_received: u64,
    invalid_sent: u64,
    latency: Histogram,
}

impl LoadStats {
    fn new(timeout: Duration) -> Self {
        Self {
            valid_sent: 0,
            valid_received: 0,
            invalid_sent: 0,
            latency: Histogram::new(timeout),
        }
    }

    fn merge(&mut self, other: &Self) {
        self.valid_sent += other.valid_sent;
        self.valid_received += other.valid_received;
        self.invalid_sent += other.invalid_sent;
        self.latency.merge(&other.latency);
    }
}

enum TaskStats {
    Valid(LoadStats),
    Invalid(u64),
}

struct PhaseStats {
    load: LoadStats,
    elapsed: Duration,
    client_cpu_pct: Option<f64>,
}

fn node_key(signer: &ed25519::PrivateKey) -> NodeKey {
    let mut key = [0; 32];
    key.copy_from_slice(signer.public_key().as_ref());
    NodeKey(key)
}

fn lookup_key(seed: u64, sequence: u64) -> NodeKey {
    let mut key = [0; 32];
    key[..8].copy_from_slice(&seed.to_be_bytes());
    key[8..16].copy_from_slice(&sequence.to_be_bytes());
    NodeKey(key)
}

fn signed_lookup(
    signer: &ed25519::PrivateKey,
    caller: NodeKey,
    seed: u64,
    sequence: u64,
    timestamp: u64,
) -> (NodeKey, Vec<u8>) {
    let key = lookup_key(seed, sequence);
    let inner = Msg::Lookup { key };
    let encoded_inner = inner.encode_inline();
    let request = AuthRequest {
        caller,
        auth: sign_authenticator(signer, &encoded_inner, timestamp, None),
        inner,
    }
    .encode();
    (key, request)
}

async fn valid_client(
    target: SocketAddr,
    deadline: Instant,
    timeout: Duration,
    pace: Option<Duration>,
    seed: u64,
) -> io::Result<LoadStats> {
    let bind = if target.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    };
    let socket = UdpSocket::bind(bind).await?;
    socket.connect(target).await?;
    let signer = ed25519::PrivateKey::from_seed(seed);
    let caller = node_key(&signer);
    let mut stats = LoadStats::new(timeout);
    let mut signed_at = now_secs();
    let mut sequence = 0;
    let (mut key, mut request) = signed_lookup(&signer, caller, seed, sequence, signed_at);
    let mut response = [0; 128];

    while Instant::now() < deadline {
        let cycle_started = Instant::now();
        let now = now_secs();
        if now.abs_diff(signed_at) >= 20 {
            (_, request) = signed_lookup(&signer, caller, seed, sequence, now);
            signed_at = now;
        }
        let started = Instant::now();
        socket.send(&request).await?;
        stats.valid_sent += 1;

        let matching_response = async {
            loop {
                let size = socket.recv(&mut response).await?;
                if matches!(
                    Msg::decode(&response[..size]),
                    Ok(Msg::LookupResponse { key: response_key, .. }) if response_key == key
                ) {
                    return Ok::<(), io::Error>(());
                }
            }
        };
        match tokio::time::timeout(timeout, matching_response).await {
            Ok(result) => {
                result?;
                stats.valid_received += 1;
                stats.latency.record(started.elapsed());
            }
            Err(_) => {
                sequence = sequence.wrapping_add(1);
                signed_at = now_secs();
                (key, request) = signed_lookup(&signer, caller, seed, sequence, signed_at);
            }
        }
        if let Some(delay) = pace.and_then(|pace| pace.checked_sub(cycle_started.elapsed())) {
            tokio::time::sleep(delay).await;
        }
    }
    Ok(stats)
}

async fn invalid_client(target: SocketAddr, deadline: Instant, seed: u64) -> io::Result<u64> {
    let bind = if target.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    };
    let socket = UdpSocket::bind(bind).await?;
    socket.connect(target).await?;
    let claimed = ed25519::PrivateKey::from_seed(seed);
    let forger = ed25519::PrivateKey::from_seed(seed.wrapping_add(1));
    let caller = node_key(&claimed);
    let inner = Msg::BindRequest { from: caller };
    let encoded_inner = inner.encode_inline();
    let mut signed_at = 0;
    let mut request = None;
    let mut sent = 0;

    while Instant::now() < deadline {
        let now = now_secs();
        if now.abs_diff(signed_at) >= 10 {
            request = Some(
                AuthRequest {
                    caller,
                    auth: sign_authenticator(&forger, &encoded_inner, now, None),
                    inner: inner.clone(),
                }
                .encode_inline(),
            );
            signed_at = now;
        }
        socket
            .send(request.as_ref().expect("request initialized"))
            .await?;
        sent += 1;
        if sent % 256 == 0 {
            tokio::task::yield_now().await;
        }
    }
    Ok(sent)
}

async fn run_phase(
    target: SocketAddr,
    duration: Duration,
    clients: usize,
    invalid_clients: usize,
    timeout: Duration,
    rate: u64,
) -> io::Result<PhaseStats> {
    let started = Instant::now();
    let cpu_started = process_cpu_ns();
    let deadline = started + duration;
    let pace =
        (rate != 0 && clients != 0).then(|| Duration::from_secs_f64(clients as f64 / rate as f64));
    let mut tasks = JoinSet::new();
    for index in 0..clients {
        tasks.spawn(async move {
            valid_client(target, deadline, timeout, pace, index as u64 + 1)
                .await
                .map(TaskStats::Valid)
        });
    }
    for index in 0..invalid_clients {
        tasks.spawn(async move {
            invalid_client(target, deadline, index as u64 + 1_000_001)
                .await
                .map(TaskStats::Invalid)
        });
    }

    let mut load = LoadStats::new(timeout);
    while let Some(result) = tasks.join_next().await {
        match result.map_err(io::Error::other)?? {
            TaskStats::Valid(stats) => load.merge(&stats),
            TaskStats::Invalid(sent) => load.invalid_sent += sent,
        }
    }
    let elapsed = started.elapsed();
    let client_cpu_pct = cpu_started.zip(process_cpu_ns()).map(|(before, after)| {
        after.saturating_sub(before) as f64 / elapsed.as_nanos() as f64 * 100.0
    });
    Ok(PhaseStats {
        load,
        elapsed,
        client_cpu_pct,
    })
}

fn value(value: Option<f64>) -> String {
    value.map_or_else(|| "na".into(), |value| format!("{value:.3}"))
}

fn print_table_header() {
    println!(
        "{:<9} {:<13} {:>8} {:>10} {:>9} {:>9} {:>8} {:>10} {:>8}",
        "PHASE",
        "WINDOW",
        "TIME(s)",
        "VALID/s",
        "P99(ms)",
        "DROPS",
        "DROP%",
        "INVALID/s",
        "CLI CPU%",
    );
    println!("{}", "-".repeat(99));
}

fn print_stats(
    output: Output,
    phase: &str,
    scope: &str,
    stats: &LoadStats,
    elapsed: Duration,
    cpu: Option<f64>,
) {
    let drops = stats.valid_sent.saturating_sub(stats.valid_received);
    let drop_pct = (stats.valid_sent != 0).then(|| drops as f64 / stats.valid_sent as f64 * 100.0);
    let p99_ms = stats
        .latency
        .percentile_us(99)
        .map(|micros| micros as f64 / 1_000.0);
    let average_ms = stats
        .latency
        .average_us()
        .map(|micros| micros as f64 / 1_000.0);
    let valid_rps = stats.valid_received as f64 / elapsed.as_secs_f64();
    let invalid_pps = stats.invalid_sent as f64 / elapsed.as_secs_f64();
    match output {
        Output::Table => println!(
            "{phase:<9} {scope:<13} {:>8.1} {:>10.0} {:>9} {drops:>9} {:>8} {:>10.0} {:>8}",
            elapsed.as_secs_f64(),
            valid_rps,
            value(p99_ms),
            value(drop_pct),
            invalid_pps,
            value(cpu),
        ),
        Output::Log => println!(
            "coordinator_load phase={phase} scope={scope} seconds={:.3} valid_sent={} valid_received={} drops={drops} drop_pct={} p99_ms={} average_ms={} invalid_sent={} valid_rps={valid_rps:.0} invalid_pps={invalid_pps:.0} client_cpu_pct={}",
            elapsed.as_secs_f64(),
            stats.valid_sent,
            stats.valid_received,
            value(drop_pct),
            value(p99_ms),
            value(average_ms),
            stats.invalid_sent,
            value(cpu),
        ),
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print!("{USAGE}");
        return Ok(());
    }
    let config = parse_args(&args).map_err(|error| {
        io::Error::new(io::ErrorKind::InvalidInput, format!("{error}\n\n{USAGE}"))
    })?;
    let target = lookup_host(&config.target).await?.next().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::AddrNotAvailable,
            format!("{} resolved to no address", config.target),
        )
    })?;
    if config.output == Output::Table {
        print_table_header();
    }
    let started = Instant::now();
    let cpu_started = process_cpu_ns();
    let deadline = started + config.duration;
    let mut total = LoadStats::new(config.timeout);
    let mut interval = 0;

    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let duration = if config.report_interval.is_zero() {
            remaining
        } else {
            remaining.min(config.report_interval)
        };
        let report = run_phase(
            target,
            duration,
            config.clients,
            config.invalid_clients,
            config.timeout,
            config.rate,
        )
        .await?;
        total.merge(&report.load);
        interval += 1;
        if !config.report_interval.is_zero() {
            print_stats(
                config.output,
                "load",
                &format!("interval-{interval}"),
                &report.load,
                report.elapsed,
                report.client_cpu_pct,
            );
        }
    }
    let elapsed = started.elapsed();
    let cpu = cpu_started.zip(process_cpu_ns()).map(|(before, after)| {
        after.saturating_sub(before) as f64 / elapsed.as_nanos() as f64 * 100.0
    });
    print_stats(config.output, "load", "summary", &total, elapsed, cpu);

    if config.invalid_clients != 0 && !config.recovery.is_zero() {
        let recovery = run_phase(
            target,
            config.recovery,
            config.clients.max(1),
            0,
            config.timeout,
            0,
        )
        .await?;
        print_stats(
            config.output,
            "recovery",
            "summary",
            &recovery.load,
            recovery.elapsed,
            recovery.client_cpu_pct,
        );
        if recovery.load.valid_received == 0 {
            return Err("coordinator did not recover after the invalid-signature flood".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn histogram_reports_the_p99_bucket_without_storing_every_sample() {
        let mut histogram = Histogram::new(Duration::from_secs(1));
        for _ in 0..99 {
            histogram.record(Duration::from_micros(50));
        }
        histogram.record(Duration::from_micros(950));
        assert_eq!(histogram.percentile_us(99), Some(100));
        assert_eq!(histogram.percentile_us(100), Some(1_000));
    }

    #[test]
    fn parser_keeps_the_soak_and_flood_knobs_in_one_probe() {
        let args = [
            "--target",
            "example.com:3478",
            "--duration",
            "86400",
            "--clients",
            "1",
            "--invalid-clients",
            "8",
            "--report-interval",
            "60",
        ]
        .map(str::to_owned);
        let config = parse_args(&args).unwrap();
        assert_eq!(config.duration, Duration::from_secs(86_400));
        assert_eq!(config.invalid_clients, 8);
        assert_eq!(config.report_interval, Duration::from_secs(60));
    }

    #[tokio::test]
    async fn probe_floods_invalid_signatures_then_recovers() {
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let target = socket.local_addr().unwrap();
        let metrics = nat_traversal::CoordinatorMetrics::default();
        let server = tokio::spawn(nat_traversal::run_coordinator_workers_with_metrics(
            socket,
            nat_traversal::AuthPolicy::Open { require_pop: true },
            4,
            metrics.clone(),
        ));

        let flood = run_phase(
            target,
            Duration::from_millis(200),
            1,
            1,
            Duration::from_millis(50),
            0,
        )
        .await
        .unwrap();
        assert!(flood.load.invalid_sent != 0);
        let recovery = run_phase(
            target,
            Duration::from_secs(1),
            1,
            0,
            Duration::from_millis(100),
            0,
        )
        .await
        .unwrap();
        assert!(recovery.load.valid_received != 0);
        assert!(metrics.snapshot().rejected != 0);

        server.abort();
        let _ = server.await;
    }
}
