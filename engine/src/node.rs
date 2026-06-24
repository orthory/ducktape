//! the async p2p Node — the integration capstone.
//!
//! a [`Node`] composes the four moving parts that, until now, lived as isolated
//! seams: the inline [`Engine`] op-dispatcher (which owns the [`Workspace`] +
//! vcs/control handler boxes), a [`Hydrator`] debounce buffer, an agentic
//! supervisor (`agent::orchestrator`), and a byte-oriented [`Transport`].
//!
//! ## the two op flows
//!
//! **inbound** (ops that arrived from a peer): a background task reads the
//! transport's inbound receiver, [`decode_batch`]es, and applies each op through
//! `Engine::apply` — so workspace ops hydrate, vcs/control hit their seams.
//! follow-up ops returned by `apply` are re-fed *locally only*: an op that came
//! off the wire is NEVER re-broadcast, which is what keeps a two-node loop from
//! ping-ponging forever.
//!
//! **outbound** (locally-originated ops): [`Node::apply_local`] applies the op to
//! local state immediately (the "echo", so the local view advances without
//! waiting on a network round-trip) and then propagates the *original* op's
//! bytes out on its [`Lane`]. because `op::Op` is `!Clone`, we encode the op for
//! the wire BEFORE the local apply consumes it.
//!
//! ## hydrator integration (the debounce)
//!
//! the propagation path is wired through the [`Hydrator`] so locally-originated
//! ops are debounced into batches before going out: `apply_local` feeds a
//! re-decoded copy of the op into the hydrator, and an [`OnHydrate`] callback
//! (registered in [`Node::new`]) fires when a batch drains. that callback is a
//! *sync* `Fn(&Batch)`, so it cannot `.await` a `transport.send` directly; we
//! bridge it to async by forwarding the per-lane encoded bytes over a tokio mpsc
//! to a dedicated forwarder task that owns the transport and does the actual
//! sends. the local echo still happens synchronously in `apply_local` (NOT on
//! the drain) so a caller can observe its own op land immediately and so the
//! intermediate "A has diverged" state is deterministic.
//!
//! see [`Node::apply_local_direct`] for the un-debounced path used by the
//! convergence test (deterministic: no interval to wait on).

use std::collections::VecDeque;
use std::sync::Arc;

use hydration::Hydrator;
use tokio::sync::{Mutex, Notify};
use transport::{Inbound, Transport, decode_batch, encode_batch};
use workspace::Workspace;

use crate::{Engine, EngineError};

/// per-node configuration. loadable from toml (see [`Config::from_toml_str`]).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Config {
    /// this node's stable index/id within the peer set. on a [`LoopbackHub`] the
    /// hub assigns the wire index; this is the operator-facing label.
    pub id: usize,

    /// the peer set — addresses/labels of the other nodes. unused on loopback
    /// (the hub fans out to every other registered node) but carried so the
    /// commonware swap has somewhere to read its dialer targets from.
    #[serde(default)]
    pub peers: Vec<String>,

    /// optional keypair seed, present-but-unused. when the transport swaps from
    /// loopback to commonware, the node will derive its signing identity from
    /// this seed; until then it is documentation of intent.
    #[serde(default)]
    pub keypair_seed: Option<String>,

    /// hydrator debounce interval, in milliseconds. small by default so the
    /// drain is responsive; the convergence test uses the direct path instead so
    /// it never waits on this.
    #[serde(default = "default_drain_interval_ms")]
    pub drain_interval_ms: u64,

    /// hydrator high-water mark — ops per batch before a forced rollover.
    #[serde(default = "default_hwm")]
    pub hwm: usize,
}

fn default_drain_interval_ms() -> u64 {
    50
}

fn default_hwm() -> usize {
    256
}

impl Config {
    /// parse a toml string into a [`Config`].
    pub fn from_toml_str(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }

    /// read + parse a toml config file from disk.
    pub fn from_path(path: &std::path::Path) -> std::io::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::from_toml_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    fn hydration_config(&self) -> hydration::Config {
        hydration::Config {
            journal: hydration::JournalConfig { hwm: self.hwm },
            cadence: hydration::CadenceConfig {
                interval: self.drain_interval_ms,
            },
        }
    }
}

/// a default [`Worker`] for `start_agentic_dev` that never shells out: it returns
/// a canned [`TaskRun`] per task so the agentic path can be exercised (and
/// demoed) without a real `claude` binary on PATH. swap in
/// [`agent::orchestrator::ClaudeWorker`] for the live path.
pub struct NoopWorker;

impl agent::orchestrator::Worker for NoopWorker {
    async fn run(
        &self,
        task: &agent::spec::TaskSpec,
        _cwd: &std::path::Path,
    ) -> Result<agent::driver::TaskRun, agent::driver::Error> {
        Ok(agent::driver::TaskRun {
            session_id: None,
            result: format!("noop-developed: {}", task.title),
            total_cost_usd: None,
            is_error: false,
        })
    }
}

/// the async node. generic over the concrete [`Transport`] `T` (default usage is
/// `transport::LoopbackTransport`).
///
/// the engine is shared behind a tokio mutex because both the inbound task and
/// `apply_local` mutate it; the transport is cloneable and held by both the node
/// and the outbound forwarder task.
pub struct Node<T: Transport + Clone + 'static> {
    /// the shared op-dispatcher (owns the workspace + handler seams).
    engine: Arc<Mutex<Engine>>,

    /// this node's transport handle.
    transport: T,

    /// the debounce buffer. locally-originated ops are inserted here; the
    /// registered `OnHydrate` callback forwards drained batches to the outbound
    /// forwarder. unused on the direct path.
    hydrator: Hydrator<op::Op>,

    /// bumped once per applied inbound batch; lets a test await convergence
    /// deterministically (no wall-clock sleep) via [`Node::wait_inbound`].
    inbound_tick: Arc<Notify>,

    /// the node's config (id, peers, ...).
    config: Config,
}

impl<T: Transport + Clone + 'static> Node<T> {
    /// build a node over `engine`, a `transport` handle and that transport's
    /// `inbound` receiver, then start the inbound task + the outbound forwarder
    /// and register the hydrator drain callback.
    pub fn new(
        config: Config,
        engine: Engine,
        transport: T,
        inbound: tokio::sync::mpsc::Receiver<Inbound>,
    ) -> Self {
        let engine = Arc::new(Mutex::new(engine));
        let inbound_tick = Arc::new(Notify::new());
        let hydrator = Hydrator::new_with_config(config.hydration_config());

        // --- outbound forwarder: bridges the SYNC on_hydrate callback to the
        // ASYNC transport.send. the callback (below) can't `.await`, so it sends
        // already-encoded `(Lane, bytes)` over this mpsc; this task owns a
        // transport clone and performs the real sends. ----------------------
        let (out_tx, mut out_rx) = tokio::sync::mpsc::unbounded_channel::<Inbound>();
        {
            let transport = transport.clone();
            tokio::spawn(async move {
                while let Some((lane, bytes)) = out_rx.recv().await {
                    let _ = transport.send(lane, bytes).await;
                }
            });
        }

        // --- the drain callback: group the drained batch by lane, encode each
        // lane's ops, and hand the bytes to the forwarder. skips the empty
        // batches the interval drain fires on an idle journal. -----------------
        let callback_tx = out_tx.clone();
        let _ = hydrator.register_on_hydrate(Box::new(move |batch: &hydration::Batch<op::Op>| {
            if batch.is_empty() {
                return;
            }
            let mut broadcast: Vec<&op::Op> = Vec::new();
            let mut consensus: Vec<&op::Op> = Vec::new();
            for op in batch.ops() {
                match op.lane() {
                    op::Lane::Broadcast => broadcast.push(op),
                    op::Lane::Consensus => consensus.push(op),
                }
            }
            for (lane, ops) in [
                (op::Lane::Broadcast, broadcast),
                (op::Lane::Consensus, consensus),
            ] {
                if ops.is_empty() {
                    continue;
                }
                // we hold `&op::Op` refs (the callback only borrows the batch);
                // serialize the ref-slice directly.
                let bytes = encode_batch_refs(&ops);
                let _ = callback_tx.send((lane, bytes));
            }
        }));

        // --- inbound task: decode + apply, re-feeding follow-ups LOCALLY only
        // (never re-broadcast — that's the loop guard). ------------------------
        {
            let engine = Arc::clone(&engine);
            let tick = Arc::clone(&inbound_tick);
            let mut inbound = inbound;
            tokio::spawn(async move {
                while let Some((_lane, bytes)) = inbound.recv().await {
                    let Ok(ops) = decode_batch(&bytes) else {
                        continue;
                    };
                    {
                        let mut eng = engine.lock().await;
                        apply_with_followups(&mut eng, ops);
                    }
                    // signal one applied batch so a waiter can snapshot.
                    // `notify_one` (not `notify_waiters`) STORES a permit when no
                    // waiter is parked yet, so a wait that arrives AFTER the apply
                    // still wakes — closing the lost-wakeup race. (a single
                    // outstanding permit is enough: each test awaits exactly one
                    // inbound batch.)
                    tick.notify_one();
                }
            });
        }

        Self {
            engine,
            transport,
            hydrator,
            inbound_tick,
            config,
        }
    }

    /// this node's config.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// apply a locally-originated op and propagate it through the HYDRATOR (the
    /// real debounce path): apply the local echo now, then feed a re-decoded copy
    /// of the op into the hydrator so the drain callback broadcasts it on its
    /// interval. follow-ups from the local apply are applied locally only.
    ///
    /// the re-decode is the honest workaround for `op::Op: !Clone` — we already
    /// encoded the op for the wire, so decoding it back to hand the hydrator its
    /// own copy is the cheapest path that keeps the original for the local echo.
    pub async fn apply_local(&self, op: op::Op) -> Result<(), EngineError> {
        // encode FIRST (apply consumes the op).
        let bytes = encode_batch(std::slice::from_ref(&op));

        // local echo: advance our own state immediately.
        {
            let mut eng = self.engine.lock().await;
            let follow_ups = eng.apply(op)?;
            apply_with_followups(&mut eng, follow_ups);
        }

        // hand the hydrator its own decoded copy to debounce + broadcast.
        if let Ok(mut copies) = decode_batch(&bytes) {
            for copy in copies.drain(..) {
                let _ = self.hydrator.insert_op(copy);
            }
        }
        Ok(())
    }

    /// apply a locally-originated op and propagate it DIRECTLY (bypassing the
    /// hydrator debounce): apply the local echo now, then send the op's bytes on
    /// its lane immediately. this is the deterministic path — there's no interval
    /// to wait on — used by the convergence test and the loopback demo.
    pub async fn apply_local_direct(&self, op: op::Op) -> Result<(), EngineError> {
        let lane = op.lane();
        let bytes = encode_batch(std::slice::from_ref(&op));

        {
            let mut eng = self.engine.lock().await;
            let follow_ups = eng.apply(op)?;
            apply_with_followups(&mut eng, follow_ups);
        }

        let _ = self.transport.send(lane, bytes).await;
        Ok(())
    }

    /// run the agentic supervisor over a spec document, locally, with a default
    /// [`NoopWorker`] (never shells out). parse the spec into tasks, develop them
    /// into ops, then feed each resulting op into the LOCAL hydrator so they
    /// debounce + propagate like any other locally-originated op.
    ///
    /// `workspace_root` is where per-task cwds would be minted by a real worker;
    /// the noop worker ignores it.
    pub async fn start_agentic_dev(
        &self,
        spec: &document::Document,
        workspace_root: std::path::PathBuf,
    ) {
        let specs = agent::spec::parse_spec(spec);
        let orch = agent::orchestrator::Orchestrator::new(NoopWorker, workspace_root, 4);
        let ops = orch.develop(specs).await;
        for op in ops {
            let _ = self.hydrator.insert_op(op);
        }
    }

    /// await the next applied inbound batch. deterministic convergence point for
    /// tests — NOT a wall-clock sleep: it parks on the inbound task's notify and
    /// wakes the instant a batch is applied.
    pub async fn wait_inbound(&self) {
        self.inbound_tick.notified().await;
    }

    /// a cheap clone of the current workspace state (a `Workspace` is `Arc`-
    /// backed, so this is an arc bump, not a deep copy). used to snapshot for a
    /// fingerprint compare without holding the engine lock across the compare.
    pub async fn workspace_snapshot(&self) -> Workspace {
        self.engine.lock().await.workspace().clone()
    }
}

/// a single-process two-node loopback demo: build nodes A and B on one
/// [`LoopbackHub`], drive an `AddEntry` op into A, and log B converging. this is
/// the body behind the bin's `node` subcommand — it lives here (not in main.rs)
/// so main.rs stays a thin clap shim and the binary's only engine touch-point is
/// this one call.
///
/// `config` labels node A (the driver); node B is minted with id+1. returns once
/// B has applied the propagated op.
pub async fn run_loopback_demo(config: Config) {
    use transport::LoopbackHub;
    use workspace::Entry;

    // a frontmatter-only seed doc so both nodes start from an identical tree.
    const SEED: &str = "---\ntitle: demo\nauthor: @a\ncreated_at: 1\nupdated_at: 1\n---\n";
    let seed = document::Document::from_reader(SEED.as_bytes()).expect("seed doc parses");
    let ws_a = Workspace::new_from_entry(Entry::Directory(vec![("seed.md".into(), Entry::File(seed))]));
    let ws_b = ws_a.clone();

    let hub = LoopbackHub::new();
    let (transport_a, rx_a) = hub.node();
    let (transport_b, rx_b) = hub.node();

    let id_a = config.id;
    let mut cfg_b = config.clone();
    cfg_b.id = config.id + 1;

    let node_a = Node::new(config, Engine::new(ws_a), transport_a, rx_a);
    let node_b = Node::new(cfg_b, Engine::new(ws_b), transport_b, rx_b);

    println!(
        "[node] loopback demo: A=#{id_a} B=#{} — driving an AddEntry into A",
        node_b.config().id
    );

    let new_doc = document::Document::from_reader(SEED.as_bytes()).expect("new doc parses");
    let op = op::Op::Workspace(workspace::op::Op::AddEntry {
        path: "added-by-a.md".into(),
        entry: Entry::File(new_doc),
    });
    node_a
        .apply_local_direct(op)
        .await
        .expect("A applies + propagates");
    println!("[node] A applied the op locally; awaiting B...");

    node_b.wait_inbound().await;

    // count B's entries to show the propagation took effect.
    let b_ws = node_b.workspace_snapshot().await;
    let entry_count = match b_ws.root().as_ref() {
        Entry::Directory(items) => items.len(),
        Entry::File(_) => 1,
    };
    println!(
        "[node] B received + applied the op; B now has {entry_count} top-level entries (seed + added)"
    );
}

/// encode a batch held as a slice of refs (the on_hydrate callback path holds
/// `&op::Op`, not owned ops).
fn encode_batch_refs(ops: &[&op::Op]) -> Vec<u8> {
    serde_json::to_vec(ops).expect("op batch serializes")
}

/// apply a queue of ops, draining each one's follow-ups back into the queue.
/// bounded in practice because the noop handlers emit no follow-ups today; the
/// queue shape is the loop-safe structure for when they do.
fn apply_with_followups(engine: &mut Engine, ops: impl IntoIterator<Item = op::Op>) {
    let mut queue: VecDeque<op::Op> = ops.into_iter().collect();
    while let Some(op) = queue.pop_front() {
        match engine.apply(op) {
            Ok(follow_ups) => queue.extend(follow_ups),
            Err(_e) => {
                // a bare-document op (no entry context) or a handler failure: a
                // convergence engine must not silently swallow these in prod, but
                // the inbound path can't propagate an error to a caller. drop
                // with the op already accounted for upstream. (TODO: surface via
                // a metrics/log sink once one exists.)
            }
        }
    }
}
