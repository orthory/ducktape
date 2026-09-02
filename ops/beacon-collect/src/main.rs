//! Headless consumer for iced's built-in frame telemetry, for QA rigs where
//! upstream `comet` (a GUI) is useless. Build the app with
//! `cargo run -p ducktape-app --features iced/debug` and its beacon client
//! streams per-stage spans (Update/View/Layout/Interact/Draw/Present) here.
//!
//! Listens where comet would (127.0.0.1:9167, ICED_BEACON_SERVER_ADDRESS to
//! override), prints an immediate line for any span over the stall threshold
//! (STALL_MS env, default 100), and dumps a per-stage summary table with a
//! fresh 10 s window each time — so scenario segments read clean. Update
//! spans are also bucketed by message name.
//!
//! This is the tool that attributed the 2026-08-16 chat lag (550 ms Layout
//! stall per channel switch → the emoji fallback scan).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use futures::StreamExt;
use iced_beacon::span::Span;
use iced_beacon::Event;

#[derive(Default)]
struct Stat {
    samples: Vec<u128>, // microseconds
}

impl Stat {
    fn push(&mut self, d: Duration) {
        self.samples.push(d.as_micros());
    }

    fn report(&self) -> (usize, u128, u128, u128, u128) {
        let mut sorted = self.samples.clone();
        sorted.sort_unstable();
        let n = sorted.len();
        let total: u128 = sorted.iter().sum();
        let p = |q: f64| sorted[((n as f64 - 1.0) * q) as usize];
        (n, p(0.5), p(0.95), sorted[n - 1], total)
    }
}

fn span_key(span: &Span) -> String {
    match span {
        Span::Boot => "Boot".into(),
        Span::Update { .. } => "Update".into(),
        Span::View { .. } => "View".into(),
        Span::Layout { .. } => "Layout".into(),
        Span::Interact { .. } => "Interact".into(),
        Span::Draw { .. } => "Draw".into(),
        Span::Present { .. } => "Present".into(),
        Span::Custom { name } => format!("Custom:{name}"),
    }
}

/// "ChatScrolled(Viewport { .. })" -> "ChatScrolled"
fn message_bucket(message: &str) -> String {
    message
        .split(['(', ' ', '{'])
        .next()
        .unwrap_or(message)
        .to_string()
}

fn dump(
    started: Instant,
    stages: &HashMap<String, Stat>,
    updates: &HashMap<String, Stat>,
) {
    let elapsed = started.elapsed().as_secs_f64();
    println!("\n=== summary @ {elapsed:.0}s ===");
    println!(
        "{:<16} {:>7} {:>9} {:>9} {:>9} {:>9} {:>7}",
        "stage", "count", "p50(us)", "p95(us)", "max(us)", "total(ms)", "/sec"
    );
    let mut keys: Vec<_> = stages.keys().collect();
    keys.sort();
    for key in keys {
        let (n, p50, p95, max, total) = stages[key].report();
        println!(
            "{key:<16} {n:>7} {p50:>9} {p95:>9} {max:>9} {:>9} {:>7.1}",
            total / 1000,
            n as f64 / elapsed
        );
    }
    let mut by_cost: Vec<_> = updates
        .iter()
        .map(|(name, stat)| {
            let (n, p50, _, max, total) = stat.report();
            (total, name, n, p50, max)
        })
        .collect();
    by_cost.sort_unstable_by(|a, b| b.0.cmp(&a.0));
    println!("--- update messages by total cost ---");
    for (total, name, n, p50, max) in by_cost.iter().take(15) {
        println!(
            "{name:<44} {n:>6}x  p50={p50:>7}us  max={max:>8}us  total={:>7}ms",
            total / 1000
        );
    }
}

#[tokio::main]
async fn main() {
    let stall = Duration::from_millis(
        std::env::var("STALL_MS")
            .ok()
            .and_then(|raw| raw.parse().ok())
            .unwrap_or(100),
    );
    let started = Instant::now();
    let mut stages: HashMap<String, Stat> = HashMap::new();
    let mut updates: HashMap<String, Stat> = HashMap::new();
    let mut last_dump = Instant::now();

    println!(
        "listening on {} (stall threshold {:?})",
        iced_beacon::client::server_address_from_env(),
        stall
    );

    let mut events = Box::pin(iced_beacon::run());
    while let Some(event) = events.next().await {
        match event {
            Event::Connected { name, version, .. } => {
                println!("[{:>8.2}s] connected: {name} {version}", started.elapsed().as_secs_f64());
            }
            Event::Disconnected { .. } => {
                println!("[{:>8.2}s] disconnected", started.elapsed().as_secs_f64());
                dump(started, &stages, &updates);
            }
            Event::SpanFinished { duration, span, .. } => {
                let key = span_key(&span);
                stages.entry(key.clone()).or_default().push(duration);
                if let Span::Update { message, .. } = &span {
                    updates
                        .entry(message_bucket(message))
                        .or_default()
                        .push(duration);
                }
                if duration >= stall {
                    let detail = match &span {
                        Span::Update { message, .. } => message.clone(),
                        other => span_key(other),
                    };
                    println!(
                        "[{:>8.2}s] STALL {key} {}ms  {detail}",
                        started.elapsed().as_secs_f64(),
                        duration.as_millis()
                    );
                }
            }
            _ => {}
        }
        // Each dump is a fresh 10 s window so scenario segments read clean.
        if last_dump.elapsed() > Duration::from_secs(10) {
            dump(started, &stages, &updates);
            stages.clear();
            updates.clear();
            last_dump = Instant::now();
        }
    }
}
