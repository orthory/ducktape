//! the process-wide log subscriber: ONE filter, two sinks, retunable live.
//!
//! this lives in the noded LIB because the three binaries that install it —
//! `bin/node`, `bin/noded`, `bin/simnode` — already depend on it (they serve
//! `noded::router`). `bin/coordinator` deliberately does not (its Cargo.toml:
//! "no node-crate dependency"); it emits no events yet, so it gets its own
//! subscriber when it gets its first one, rather than inverting that boundary
//! for a constructor. `bin/fs` and `bin/mcp` are CLIs whose stdout IS their wire
//! (a duckfs command's output, JSON-RPC) — program output is not logging.
//!
//! events reach TWO places, both gated by the same filter:
//!
//! - stderr: the spawner tees it into `<workspace>/daemon.log` — the durable
//!   record, and it survives a crash because stderr is unbuffered.
//! - [`crate::LogRing`]: the ws `logs` topic, i.e. the desktop app's Logs tab —
//!   the live tail.

use std::sync::OnceLock;

use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use tracing_subscriber::{EnvFilter, Layer as _, reload};

/// the floor with RUST_LOG unset. it must not be ERROR: the desktop spawns the
/// node with NO environment beyond PATH (`daemon.rs::prepare_node_command_env`),
/// so anything that needs an env var to be visible is, in practice, invisible.
///
/// if commonware's `info` proves chatty, pin it here — it is just a directive
/// string ("info,commonware_p2p=warn"). don't guess: the ring's dropped-lines
/// marker (`stream.rs::catch_up_logs`) tells you whether it actually evicts.
///
/// defguard_boringtun is pinned off: its rekey timers WARN every ~5 s per
/// unreachable peer — with no peer field, so the lines diagnose nothing while
/// evicting the ring. the replacement is peer-labeled and edge-triggered on
/// `ducktape::overlay` (overlay-net's `device.rs` logs expiry/recovery from
/// `ConnectionExpired` return values, where the peer IS known). RUST_LOG
/// appends after this, so `RUST_LOG=defguard_boringtun=warn` re-arms the raw
/// crate lines.
const DEFAULT_FILTER: &str = "info,defguard_boringtun=off";

/// parse a directive list STRICTLY.
///
/// `EnvFilter::new` SKIPS a malformed directive and carries on, which would make
/// a typo'd filter look like it worked — an operator would turn on `debug`, get a
/// success, and go on reading a log that never got louder. that is the exact class
/// of silent failure this module exists to end, so the one caller a human drives
/// (POST /v1/log-filter) refuses instead.
fn parse(directives: &str) -> Result<EnvFilter, String> {
    EnvFilter::builder()
        .parse(directives)
        .map_err(|err| err.to_string())
}

/// the boot filter: RUST_LOG *adds to* the default rather than replacing it.
///
/// `EnvFilter::from_default_env` REPLACES, and its no-directive default is
/// ERROR — so a bare `RUST_LOG=one::target=debug` silently drops every other
/// event to ERROR. turning one plane UP must never turn the rest OFF.
///
/// a malformed RUST_LOG falls back to the default rather than refusing to boot;
/// the node warns about it below, once there is a subscriber to warn through.
fn boot_filter() -> (EnvFilter, Option<String>) {
    let env = std::env::var("RUST_LOG").unwrap_or_default();
    if env.is_empty() {
        return (
            parse(DEFAULT_FILTER).expect("the default filter parses"),
            None,
        );
    }
    let combined = format!("{DEFAULT_FILTER},{env}");
    match parse(&combined) {
        Ok(filter) => (filter, None),
        Err(err) => (
            parse(DEFAULT_FILTER).expect("the default filter parses"),
            Some(err),
        ),
    }
}

type Reload = Box<dyn Fn(&str) -> Result<(), String> + Send + Sync>;
static RELOAD: OnceLock<Reload> = OnceLock::new();

/// retune a LIVE node (POST /v1/log-filter).
///
/// without this every `debug!` is unreachable in practice: RUST_LOG is read once
/// at boot, and the only way to raise a level would be a restart — which destroys
/// the wedged state you were trying to observe.
pub fn set_filter(directives: &str) -> Result<(), String> {
    match RELOAD.get() {
        Some(reload) => reload(directives),
        None => Err("no subscriber installed".into()),
    }
}

/// install the subscriber. call ONCE, from `main`. `ring: None` for a binary
/// with no stream surface (mcp, fs). `log_file: Some(<workspace>/daemon.log)`
/// makes the node its OWN tee: the desktop spawner that used to pipe stderr
/// into daemon.log is gone, so without this a hand-run `ducktape node run`
/// leaves no durable record at all — the log "looks off by default".
pub fn init(ring: Option<crate::LogRing>, log_file: Option<std::path::PathBuf>) {
    let (boot, bad_env) = boot_filter();
    let (filter_layer, handle) = reload::Layer::new(boot);
    let _ = RELOAD.set(Box::new(move |directives: &str| {
        let filter = parse(directives)?;
        handle.reload(filter).map_err(|err| err.to_string())
    }));

    let stderr_layer = tracing_subscriber::fmt::layer().with_writer(std::io::stderr);
    let ring_layer = ring.map(|ring| {
        tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(ring)
    });
    // an unwritable log file must not kill the node — warn (below, once a
    // subscriber exists) and run on stderr + ring only.
    let (file_layer, file_err) = match log_file {
        None => (None, None),
        Some(path) => match open_log_file(&path) {
            Ok(file) => (
                Some(
                    tracing_subscriber::fmt::layer()
                        .with_ansi(false)
                        .with_writer(std::sync::Mutex::new(file)),
                ),
                None,
            ),
            Err(err) => (None, Some(format!("{}: {err}", path.display()))),
        },
    };
    let _ = tracing_subscriber::registry()
        // ONE filter, gating EVERY sink — the ring, stderr and daemon.log can
        // never disagree about what was worth recording.
        .with(
            stderr_layer
                .and_then(ring_layer)
                .and_then(file_layer)
                .with_filter(filter_layer),
        )
        .try_init();

    // now that there IS a subscriber, say so: a typo'd RUST_LOG that silently did
    // nothing would send someone hunting a bug in the code that "won't log".
    if let Some(err) = bad_env {
        tracing::warn!(
            target: "ducktape::node",
            error = %err,
            "RUST_LOG is malformed — ignored, running at the default filter"
        );
    }
    if let Some(err) = file_err {
        tracing::warn!(
            target: "ducktape::node",
            error = %err,
            "daemon.log unavailable — logging to stderr and the ring only"
        );
    }

    install_panic_hook();
}

/// append-open the daemon log, creating its parent. append (never truncate):
/// the record across restarts is exactly what a crash post-mortem reads.
// ponytail: unbounded growth at info cadence; rotation when a real workspace
// shows a daemon.log worth rotating.
fn open_log_file(path: &std::path::Path) -> std::io::Result<std::fs::File> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::OpenOptions::new().create(true).append(true).open(path)
}

/// a panic in a spawned task kills THAT TASK ONLY: the node stays "up" while one
/// plane goes dark forever. the reachability plane, the voice hub and the overlay
/// stack each own a thread, so this is not hypothetical.
///
/// chain, don't replace — the default hook keeps the backtrace on stderr.
fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!(
            target: "ducktape::node",
            thread = std::thread::current().name().unwrap_or("?"),
            // the "panicked at" text is a marker the desktop shell greps for
            // (app/src-iced/src/backend/workspace_service.rs) — keep it in the message.
            "panicked at: {info}"
        );
        default(info);
    }));
}

/// the drain-side sink for module events that no worker claimed.
///
/// this is not a nicety: 9 of the 11 app modules compile to WASM guests, and the
/// WIT world exposes NO log import — `emit_event` is their entire outbound
/// diagnostic surface, forever. every `runs::note()` breadcrumb arrives here,
/// including `run <id> failed: <reason>` — the single most important line in the
/// agent path, which until now was collected by the host and dropped on the floor.
///
/// one instance per drain: `take_events()` returns everything since the last tick,
/// and a single drain can apply MANY blocks (catch-up, a post-reboot suffix), so
/// an uncapped burst could evict the whole 4096-line ring at exactly the moment an
/// operator is watching a join.
pub struct ModuleNotes {
    height: u64,
    emitted: usize,
    suppressed: usize,
}

/// notes emitted per drain before the rest are counted instead of printed.
const NOTE_BUDGET: usize = 16;

impl ModuleNotes {
    pub fn new(height: u64) -> Self {
        Self {
            height,
            emitted: 0,
            suppressed: 0,
        }
    }

    /// record one unclaimed event. a payload that DECODES as a worker request is
    /// not observability at all — it means a saga is stuck Pending.
    pub fn unclaimed(&mut self, event: &sdk::Event) {
        if saga::decode_worker_request(&event.payload).is_ok() {
            stuck_saga(self.height, &event.source, event.payload.len());
        } else if self.emitted < NOTE_BUDGET {
            self.emitted += 1;
            tracing::info!(
                target: "ducktape::modules",
                height = self.height,
                source = %event.source,
                note = %sanitize(&event.payload),
            );
        } else {
            self.suppressed += 1;
        }
    }

    /// call once at the end of the drain.
    pub fn finish(self) {
        if self.suppressed > 0 {
            tracing::info!(
                target: "ducktape::modules",
                height = self.height,
                suppressed = self.suppressed,
                "module notes suppressed (per-drain budget)"
            );
        }
    }
}

/// a first-and-every-Nth latch for a failure that REPEATS on a retry loop.
///
/// a peer that cannot sync retries forever, so an unconditional `warn!` on the
/// refusal path is a log bomb: it evicts the whole 4096-line ring in seconds and
/// destroys the surrounding context — strictly worse than silence. log the first
/// occurrence, then every Nth, and carry the count. **the counter IS the
/// diagnosis**: "attempts=3000" is what tells you this is wedged, not flaky.
///
/// keyed by a caller-supplied `&'static str`, so distinct refusal reasons latch
/// independently and one noisy reason cannot mask another.
pub struct Latch {
    counts: std::sync::Mutex<std::collections::BTreeMap<&'static str, u64>>,
    every: u64,
}

impl Latch {
    pub const fn new(every: u64) -> Self {
        Self {
            counts: std::sync::Mutex::new(std::collections::BTreeMap::new()),
            every,
        }
    }

    /// returns `Some(occurrences)` when this occurrence should be logged.
    pub fn hit(&self, key: &'static str) -> Option<u64> {
        let mut counts = self.counts.lock().expect("latch lock poisoned");
        let count = counts.entry(key).or_insert(0);
        *count += 1;
        let n = *count;
        (n == 1 || n.is_multiple_of(self.every)).then_some(n)
    }
}

/// a stuck saga does not clear itself: the same WorkerRequest re-fires every
/// block, forever. latch it — an `error!` in a permanent loop stops meaning
/// anything, and it would evict every other line in the ring behind it.
fn stuck_saga(height: u64, source: &str, bytes: usize) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEEN: AtomicU64 = AtomicU64::new(0);
    let occurrences = SEEN.fetch_add(1, Ordering::Relaxed) + 1;
    if occurrences == 1 || occurrences.is_multiple_of(600) {
        tracing::error!(
            target: "ducktape::saga",
            height,
            source,
            bytes,
            occurrences,
            "WorkerRequest with no worker — saga is stuck Pending"
        );
    }
}

/// a module payload is arbitrary bytes from a WASM guest, and `runs::note()`
/// embeds free-form provider/LLM text. cap it and strip control characters before
/// it reaches a terminal — and the webview, which the ring is streamed to.
fn sanitize(payload: &[u8]) -> String {
    String::from_utf8_lossy(payload)
        .chars()
        .filter(|c| !c.is_control())
        .take(256)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_latch_logs_the_first_then_every_nth_and_counts_the_rest() {
        let latch = Latch::new(100);
        // a joiner refused once must be visible IMMEDIATELY — not on the 100th try.
        assert_eq!(latch.hit("not_in_committed_standing"), Some(1));
        for _ in 2..100 {
            assert_eq!(latch.hit("not_in_committed_standing"), None, "no flood");
        }
        // ...and the count is what tells you it is WEDGED, not merely flaky.
        assert_eq!(latch.hit("not_in_committed_standing"), Some(100));

        // distinct reasons latch independently: a noisy one must never mask another.
        assert_eq!(latch.hit("sync_proof_invalid"), Some(1));
    }

    #[test]
    fn a_misspelled_level_is_refused_not_silently_skipped() {
        assert!(parse("info,ducktape::join=debug").is_ok());
        // the typo an operator ACTUALLY makes. `EnvFilter::new` would skip this
        // directive and report success, leaving them to read a log that never got
        // louder and hunt for the bug in the code that "won't log". a partly
        // applied filter is the most confusing outcome of all, so refuse the lot.
        assert!(parse("info,ducktape::join=debgu").is_err());
        // NOT an error, and deliberately so: a bare word is a legal directive —
        // it names a TARGET (enable it at trace). there is nothing to refuse.
        assert!(parse("ducktape::join").is_ok());
    }

    #[test]
    fn sanitize_strips_control_chars_and_caps_length() {
        assert_eq!(
            sanitize(b"run 7f failed: rate\nlimited\x07"),
            "run 7f failed: ratelimited"
        );
        assert_eq!(sanitize(&vec![b'x'; 300]).len(), 256);
        // invalid utf8 is lossy-decoded, never a panic and never a raw byte dump.
        assert_eq!(sanitize(&[b'o', b'k', 0xff]), "ok\u{fffd}");
    }

    #[test]
    fn notes_beyond_the_budget_are_counted_not_printed() {
        let mut notes = ModuleNotes::new(9);
        for _ in 0..NOTE_BUDGET + 5 {
            notes.unclaimed(&sdk::Event {
                source: "runs".into(),
                payload: b"note".to_vec(),
            });
        }
        assert_eq!(notes.emitted, NOTE_BUDGET, "the ring is never flooded");
        assert_eq!(notes.suppressed, 5, "and the overflow is still counted");
    }
}
