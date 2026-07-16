//! A bounded in-memory `tracing` ring the `logs` tool reads. Follows the
//! logging doctrine: capped (4096 lines, oldest evicted), and it stores only
//! what log sites emit — instrumented code is responsible for never logging
//! key material or capability paths.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tracing::field::{Field, Visit};
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{Layer, Registry};

const CAP: usize = 4096;

/// One captured log event.
#[derive(Debug, Clone, Serialize)]
pub struct LogLine {
    pub level: String,
    pub target: String,
    pub message: String,
}

/// Reader handle over the ring, shared with the bridge context.
#[derive(Clone)]
pub struct LogsHandle {
    ring: Arc<Mutex<VecDeque<LogLine>>>,
}

impl LogsHandle {
    /// Current contents, oldest first.
    pub fn snapshot(&self) -> Vec<LogLine> {
        self.ring.lock().map(|r| r.iter().cloned().collect()).unwrap_or_default()
    }

    /// Drops every captured line.
    pub fn clear(&self) {
        if let Ok(mut r) = self.ring.lock() {
            r.clear();
        }
    }
}

/// A `tracing` layer pushing events into the ring. Generic over the subscriber
/// so it can be layered in any position (concrete type, not an opaque
/// `impl Layer<Registry>`, so `registry().with(fmt).with(ring)` also composes).
pub struct RingLayer {
    ring: Arc<Mutex<VecDeque<LogLine>>>,
}

impl<S> Layer<S> for RingLayer
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);
        let line = LogLine {
            level: meta.level().to_string(),
            target: meta.target().to_string(),
            message: visitor.message,
        };
        if let Ok(mut ring) = self.ring.lock() {
            if ring.len() >= CAP {
                let _ = ring.pop_front();
            }
            ring.push_back(line);
        }
    }
}

/// Builds the ring layer and a reader handle over the same buffer.
pub fn ring_layer() -> (RingLayer, LogsHandle) {
    let ring = Arc::new(Mutex::new(VecDeque::with_capacity(CAP)));
    (
        RingLayer { ring: Arc::clone(&ring) },
        LogsHandle { ring },
    )
}

/// Asserts `RingLayer` is usable directly on a `Registry`, matching the plan's
/// `impl Layer<Registry>` contract while keeping the concrete generic type.
const _: fn() = || {
    fn assert_layer<L: Layer<Registry>>() {}
    assert_layer::<RingLayer>();
};

#[derive(Default)]
struct MessageVisitor {
    message: String,
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{value:?}");
        } else {
            if !self.message.is_empty() {
                self.message.push(' ');
            }
            self.message.push_str(&format!("{}={value:?}", field.name()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::layer::SubscriberExt;

    #[test]
    fn ring_captures_events() {
        let (layer, handle) = ring_layer();
        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(target: "test", reason = "probe", "hello ring");
        });
        let lines = handle.snapshot();
        assert!(lines.iter().any(|l| l.message.contains("hello ring")));
        assert!(lines.iter().any(|l| l.message.contains("reason=")));
        assert!(lines.iter().any(|l| l.level == "INFO"));
    }

    #[test]
    fn ring_is_capped_and_clearable() {
        let (layer, handle) = ring_layer();
        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            for i in 0..(CAP + 100) {
                tracing::info!(target: "test", "line {}", i);
            }
        });
        assert!(handle.snapshot().len() <= CAP);
        handle.clear();
        assert!(handle.snapshot().is_empty());
    }
}
