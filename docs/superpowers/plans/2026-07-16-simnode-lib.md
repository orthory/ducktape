# simnode Library Extraction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `bin/simnode` embeddable: `simnode::boot(storage, listen, SimOpts) -> SimHandle`, binary becomes a thin main, all existing binary consumers unchanged.

**Architecture:** Move the composition currently inline in `main()` (lines ~421–510: IndexStore, LogRing/handle, actor-thread spawn, router merge, serve) and `run_sim` + the whole `Sim` engine into `src/lib.rs`. `boot` binds (possibly `:0`), spawns the actor thread and a serve thread with a private tokio runtime, and returns a synchronous `SimHandle` over the existing `SimCommand` mpsc. Spec: `docs/superpowers/specs/2026-07-16-simnode-lib-design.md`.

**Tech Stack:** Existing deps only (axum 0.8, tokio, commonware runtime, noded path-dep). One one-line visibility change in `bin/noded/src/handle.rs`.

## Global Constraints

- Worktree: `<repo>/.worktree/simnode-lib`, branch `feat/simnode-lib`, PR target `dev`.
- `CARGO_INCREMENTAL=0` on every cargo command. If rustc SIGSEGVs on the dep graph: `sccache --stop-server; export RUSTC_WRAPPER=""; export RUST_MIN_STACK=2147483648` and retry.
- Lint gates: `cargo clippy -p simnode --tests --no-deps` AND `cargo clippy -p noded --tests --no-deps` (handle.rs is touched). Never `cargo fmt --all`.
- The behavior of the BINARY must not change: same flags, same defaults (including the `temp_dir()` storage default and exit-1 on fatal), same stdout lines. The 11 suites in `bin/simnode/tests/` are the regression net — they all spawn the binary.
- A lib must not `println!` and must not install global logging unless asked (`install_log`).
- Package `simnode` keeps its name; the lib target is `simnode`, the `[[bin]]` stays `ducktape-simnode`.

---

### Task 1: Extract `lib.rs` + thin `main.rs` + noded shutdown visibility

**Files:**
- Create: `bin/simnode/src/lib.rs` (the moved engine + new `boot`/`SimOpts`/`SimHandle`)
- Modify: `bin/simnode/src/main.rs` (shrinks to arg parsing + `boot` + `wait`)
- Modify: `bin/simnode/Cargo.toml` (add `[lib]`)
- Modify: `bin/noded/src/handle.rs:267` (`pub(crate) fn request_shutdown` → `pub`, with a doc comment saying embedders use it for graceful teardown)

**Interfaces:**
- Consumes: everything already in `main.rs` (this is a move, not a rewrite).
- Produces (Task 2 relies on these exact signatures):
  - `pub struct SimOpts { pub auto: bool, pub echo_oracle: bool, pub valset_keys: Vec<Vec<u8>>, pub invite_binding: Vec<u8>, pub node_key: Option<K>, pub persona: P, pub install_log: bool }` where `K`/`P` are the exact types `main()` already parses into (read them off the current code — do not invent new types), with `impl Default` (auto=false, echo=false, empty valset, `b"sim".to_vec()`, None, persona-local, install_log=false).
  - `pub fn boot(storage: &std::path::Path, listen: std::net::SocketAddr, opts: SimOpts) -> Result<SimHandle, String>`
  - `pub struct SimHandle` with `pub fn addr(&self) -> SocketAddr`, `pub fn step(&self) -> Result<serde_json::Value, String>`, `pub fn set_auto(&self, enabled: bool) -> Result<(), String>`, `pub fn peer_block(&self, body: serde_json::Value) -> Result<serde_json::Value, String>`, `pub fn state(&self) -> Result<serde_json::Value, String>`, `pub fn wait(self) -> Result<(), String>`, `pub fn shutdown(self)`, and `Drop` = shutdown.

- [ ] **Step 1: Read the current `main.rs` end to end** (~1481 lines). Identify the exact regions named in the architecture note: CLI parse (~325–420), composition (~421–510), `run_sim` (~551–745), `Sim` engine (~517–1195), `sim_router` + handlers (~1318–1441), `strip_receipt_op_hash` (~1449). This task is a MOVE of those regions into `lib.rs` — resist rewriting logic while moving.

- [ ] **Step 2: Add the lib target**

In `bin/simnode/Cargo.toml`, above the existing `[[bin]]`:

```toml
[lib]
name = "simnode"
path = "src/lib.rs"
```

- [ ] **Step 3: Create `lib.rs`** — move everything except arg parsing and the storage default. Structure:

```rust
//! Embeddable deterministic sim node.
//!
//! `boot()` composes the same node the `ducktape-simnode` binary serves —
//! noded's full `/v1` router plus the `/sim/*` determinism lane — on
//! caller-owned storage and a caller-chosen listen address (`:0` for
//! ephemeral), running on private background threads so an embedder needs
//! no tokio runtime. The binary is a thin `main` over this.

// (moved items: BASE_MODULE_IDS, VALSET_MODULE_IDS, HeldOp, Sim, run_sim,
//  sim_router + handlers, strip_receipt_op_hash, SimCommand, persona enum…)

pub struct SimOpts { /* per Interfaces above */ }

pub fn boot(storage: &Path, listen: SocketAddr, opts: SimOpts) -> Result<SimHandle, String> {
    // 1. (moved from main ~429–450) IndexStore::open + indexers; if
    //    opts.install_log { LogRing + noded::log::init(Some(ring)) } else
    //    { build the handle WITHOUT wiring the global ring — use the
    //    NodeHandle constructor that takes no ring, or pass the ring but
    //    skip log::init; read handle.rs for which exists }.
    // 2. (moved from main ~452–461) NodeHandle construction + blob handle
    //    + SimCommand mpsc.
    // 3. (moved from main ~462–481) spawn the "sim-actor" std thread
    //    running run_sim(...). Keep a JoinHandle.
    // 4. NEW: serve thread — std::thread spawning a private
    //    tokio runtime (Builder::new_multi_thread().worker_threads(2)
    //    .enable_all()) that binds tokio TcpListener::bind(listen),
    //    reports listener.local_addr() back over a std::sync::mpsc
    //    (so boot can return the REAL addr), then
    //    axum::serve(listener, app).with_graceful_shutdown(
    //        handle.shutdown_requested()).await.
    //    app = noded::router(handle.clone()).merge(sim_router(...))
    //          .layer(strip_receipt_op_hash …) — exactly main's ~497–507.
    // 5. boot returns SimHandle { addr, control, node: handle,
    //    serve: Some(join), actor: Some(join), fatal: <shared flag> }
    //    after receiving the bound addr (error if the serve thread fails
    //    to bind).
}

pub struct SimHandle { /* addr, control mpsc Sender, NodeHandle, joins, fatal flag */ }

impl SimHandle {
    // step/set_auto/peer_block/state: build the SAME SimCommand values the
    // /sim/* HTTP handlers build (reuse their helper — `control()` at
    // ~1330 — or factor a shared fn), send with control.blocking_send,
    // await the oneshot reply with blocking_recv. Map channel-closed to
    // Err("sim actor stopped").
    // wait(): binary path — join serve thread, then actor; return
    //   Err(reason) if the fatal flag was set (main turns that into exit 1).
    // shutdown(): node.request_shutdown(); join serve; drop control sender
    //   (actor loop must exit when BOTH the control channel closes and the
    //   node command channel closes — verify run_sim's select! exits when
    //   channels close; add that exit arm if missing); join actor.
    // Drop: same as shutdown (guard against double-join with Option::take).
}
```

**Fatal-exit change (the one behavioral edit allowed):** in the moved `commit`/`commit_batch`, replace the two `std::process::exit(1)` calls with: set the shared fatal flag (`Arc<OnceLock<String>>` or `Arc<Mutex<Option<String>>>`), reply an error to the held submitter if one is pending, and `node.request_shutdown()`. `wait()` surfaces it; embedded `step()`/`state()` return `Err` once the actor is gone.

- [ ] **Step 4: Shrink `main.rs`** to: the arg-parse loop (unchanged flags, unchanged storage default `temp_dir()/ducktape-simnode-{pid}`), `SimOpts { install_log: true, .. }`, the existing startup `println!` lines (they are program output of a CLI — they stay in the binary, printing `handle.addr()`), then:

```rust
let handle = simnode::boot(&storage, listen, opts).unwrap_or_else(|error| {
    eprintln!("simnode boot failed: {error}");
    std::process::exit(1);
});
println!("listening on {}", handle.addr());   // keep whatever main prints today, now from handle.addr()
if let Err(reason) = handle.wait() {
    eprintln!("simnode fatal: {reason}");
    std::process::exit(1);
}
```

Match the EXISTING stdout wording — read what main prints today (~483, ~508) and keep those exact strings/order so the TS harness's readiness logic sees nothing new.

- [ ] **Step 5: noded visibility** — `bin/noded/src/handle.rs:267`: change `pub(crate) fn request_shutdown` to `pub fn request_shutdown` with a one-line doc comment (`/// Request graceful shutdown; embedders (simnode lib) call this for teardown.`).

- [ ] **Step 6: Compile both targets**

```bash
CARGO_INCREMENTAL=0 cargo build -p simnode
```
Expected: builds lib + bin cleanly.

- [ ] **Step 7: Run the full existing simnode suite (binary regression net)**

```bash
CARGO_INCREMENTAL=0 cargo test -p simnode
```
Expected: all 11 integration suites + any unit tests pass exactly as on `dev`.

- [ ] **Step 8: Commit**

```bash
git add bin/simnode bin/noded/src/handle.rs
git commit -m "refactor(simnode): extract embeddable lib — boot()/SimHandle, thin binary

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 2: Embedded smoke tests

**Files:**
- Create: `bin/simnode/tests/embed.rs`
- Modify (only if needed for reuse): `bin/simnode/tests/harness/mod.rs` (make `try_request` `pub` so embed.rs can reuse the raw HTTP helper instead of duplicating it)

**Interfaces:**
- Consumes: `simnode::{boot, SimOpts, SimHandle}` exactly as produced by Task 1.
- Produces: nothing downstream — this is the lib's proof.

- [ ] **Step 1: Write `tests/embed.rs`**

```rust
//! In-process embedding proof: no CARGO_BIN_EXE, no child process.

mod harness; // for try_request (raw HTTP against the embedded server)

use std::net::{SocketAddr, TcpStream};

fn loopback0() -> SocketAddr {
    "127.0.0.1:0".parse().expect("addr")
}

fn boot_auto(storage: &std::path::Path) -> simnode::SimHandle {
    simnode::boot(
        storage,
        loopback0(),
        simnode::SimOpts { auto: true, ..Default::default() },
    )
    .expect("boot embedded sim")
}

#[test]
fn embedded_boot_serves_and_commits() {
    let storage = tempfile::tempdir().expect("storage");
    let sim = boot_auto(storage.path());
    let port = sim.addr().port();

    let (status, reply) = harness::try_request(port, "GET", "/v1/status", None).expect("status");
    assert_eq!(status, 200, "status: {reply}");

    // Auto-mode submit commits inline; query reads it back — the whole
    // round-trip in one process.
    let (status, reply) = harness::try_request(
        port,
        "POST",
        "/v1/submit",
        Some(&serde_json::json!({
            "target": "chat",
            "payload": { "create_channel": {
                "channel_id": "embed", "name": "embed", "post_policy": "open" } }
        })),
    )
    .expect("submit");
    assert_eq!(status, 200, "submit: {reply}");

    let (status, reply) =
        harness::try_request(port, "POST", "/v1/query",
            Some(&serde_json::json!({ "target": "chat", "query": "channels" })))
        .expect("query");
    assert_eq!(status, 200);
    assert!(reply.to_string().contains("embed"), "committed channel visible: {reply}");

    sim.shutdown();
}

#[test]
fn two_embedded_instances_are_independent() {
    let (dir_a, dir_b) = (tempfile::tempdir().expect("a"), tempfile::tempdir().expect("b"));
    let a = boot_auto(dir_a.path());
    let b = boot_auto(dir_b.path());
    assert_ne!(a.addr(), b.addr());

    let (status, _) = harness::try_request(
        a.addr().port(), "POST", "/v1/submit",
        Some(&serde_json::json!({ "target": "chat", "payload": { "create_channel": {
            "channel_id": "only-a", "name": "only-a", "post_policy": "open" } } })),
    ).expect("submit a");
    assert_eq!(status, 200);

    let state_a = a.state().expect("state a");
    let state_b = b.state().expect("state b");
    assert_ne!(state_a["height"], state_b["height"], "a committed, b did not: {state_a} vs {state_b}");

    a.shutdown();
    b.shutdown();
}

#[test]
fn shutdown_frees_the_port() {
    let storage = tempfile::tempdir().expect("storage");
    let sim = boot_auto(storage.path());
    let addr = sim.addr();
    sim.shutdown();
    assert!(
        TcpStream::connect(addr).is_err(),
        "port must refuse connections after shutdown"
    );
}

#[test]
fn held_mode_step_via_handle() {
    let storage = tempfile::tempdir().expect("storage");
    let sim = simnode::boot(storage.path(), loopback0(), simnode::SimOpts::default())
        .expect("boot held-mode sim");
    // Held mode: a submit parks until step. Submit from a thread (its HTTP
    // reply hangs until the step), then step via the handle.
    let port = sim.addr().port();
    let submitter = std::thread::spawn(move || {
        harness::try_request(
            port, "POST", "/v1/submit",
            Some(&serde_json::json!({ "target": "chat", "payload": { "create_channel": {
                "channel_id": "held", "name": "held", "post_policy": "open" } } })),
        )
    });
    // Poll sim state until the op is parked, then commit it.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let state = sim.state().expect("state");
        if state["held"].as_u64().unwrap_or(0) > 0 { break; }
        assert!(std::time::Instant::now() < deadline, "op never parked: {state}");
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let stepped = sim.step().expect("step");
    let (status, _) = submitter.join().expect("join").expect("submit reply");
    assert_eq!(status, 200, "held submit resolves after step: {stepped}");
    sim.shutdown();
}
```

Adjust the two state-field names (`height`, `held`) to whatever `/sim/state`'s `snapshot` (~`main.rs:1186` pre-move) actually serializes — read the moved code, don't guess.

- [ ] **Step 2: Run the embed suite**

```bash
CARGO_INCREMENTAL=0 cargo test -p simnode --test embed
```
Expected: 4 passed, no leaked-thread hangs at process exit.

- [ ] **Step 3: Re-run the whole package + noded**

```bash
CARGO_INCREMENTAL=0 cargo test -p simnode
CARGO_INCREMENTAL=0 cargo test -p noded
```
Expected: everything green (noded is touched via handle.rs).

- [ ] **Step 4: Commit**

```bash
git add bin/simnode/tests/
git commit -m "test(simnode): embedded-boot proof — in-process serve, two instances, held step, clean shutdown

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 3: Gates + PR

- [ ] **Step 1: Lint gates (touch first — cached cargo re-emits no warnings)**

```bash
find bin/simnode/src bin/noded/src/handle.rs -name '*.rs' -exec touch {} +
CARGO_INCREMENTAL=0 cargo clippy -p simnode --tests --no-deps
CARGO_INCREMENTAL=0 cargo clippy -p noded --tests --no-deps
```
Expected: clean.

- [ ] **Step 2: Push + PR against `dev`**

```bash
git push -u origin feat/simnode-lib
gh pr create --base dev --title "refactor(simnode): embeddable lib — boot()/SimHandle, thin binary" --body "$(cat <<'EOF'
## Summary
- `bin/simnode` gains a `[lib]`: `simnode::boot(storage, listen, SimOpts) -> SimHandle` — same composition the binary serves (noded `/v1` router + `/sim/*` lane), on caller storage and an ephemeral-capable listen addr, running on private background threads (embedder needs no runtime). Synchronous handle: `addr/step/set_auto/peer_block/state/wait/shutdown`.
- Embedding hazards fixed at the source: global `log::init` + panic-hook behind `SimOpts::install_log` (binary=true, embedders=false); the two fatal-submit `process::exit(1)` sites now flag + graceful-shutdown (binary keeps exit-1 via `wait()`); storage dir explicit in the lib (binary keeps its temp-dir default).
- Binary is now a thin `main` — flags, defaults, stdout, and exit semantics unchanged; all existing binary consumers (TS `simnode-harness.ts`, the 11 Rust suites) untouched.
- New `tests/embed.rs`: in-process boot + commit round-trip, two independent instances in one process, held-mode step via the handle, shutdown frees the port.
- `noded::NodeHandle::request_shutdown` is now `pub` (embedder teardown).

Motivation: phase A of the iced in-process sim lane (`docs/superpowers/specs/2026-07-16-simnode-lib-design.md`); phase B embeds this from the iced app's tests.

## Verification
- `cargo test -p simnode` (all suites incl. new embed) — state counts.
- `cargo test -p noded` — state counts.
- `cargo clippy -p simnode --tests --no-deps` + `-p noded` — clean.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

- [ ] **Step 3: Report** — test counts, gate results, deviations.

---

## Self-Review (authoring time)

- **Spec coverage:** boot/SimOpts/SimHandle (T1), install_log gate (T1), fatal→error (T1), thin main + unchanged binary (T1 steps 4+7), shutdown contract (T1 + T2 `shutdown_frees_the_port`), embed proofs incl. two-instance (T2), gates + noded touch (T3). Spec cuts (persona setter, async handle) not built.
- **Placeholders:** the two `/sim/state` field names in T2 are flagged as read-off-the-code, with the exact source location — deliberate, since the move may rename nothing.
- **Type consistency:** `SimHandle` method set identical in T1 Interfaces, T2 usage, spec.
