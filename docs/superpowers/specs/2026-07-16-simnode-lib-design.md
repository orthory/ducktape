# simnode as a library — in-process embedding

**Date:** 2026-07-16
**Branch:** `feat/simnode-lib` → PR into `dev`
**Status:** approved design (phase A of the iced in-process sim lane;
phase B lives in `docs/superpowers/specs/2026-07-16-iced-sim-lane-design.md`
on `feat/iced-sim-lane`)

## Problem

`ducktape-simnode` is binary-only: all composition (genesis, host actor,
router, serve) is inline in `main()`/`run_sim()`. A test suite that wants a
deterministic sim node must spawn the prebuilt binary — which breaks
`cargo test` self-containment for the iced app's new sim lane (the user's
directive: foundry-style, no external binaries).

## Decision

Give `bin/simnode` a `[lib]` target exposing one boot function and a
synchronous handle. The binary becomes a thin `main` over the lib. Every
existing consumer (TS vitest harness, the 11 Rust integration suites) keeps
spawning the binary over HTTP unchanged.

```rust
pub struct SimOpts {
    pub auto: bool,
    pub echo_oracle: bool,
    pub valset_keys: Vec<Vec<u8>>,     // same shapes main() parses today
    pub invite_binding: Vec<u8>,       // default b"sim"
    pub node_key: Option<...>,         // same type main() derives
    pub persona: ...,                  // existing persona enum, default local
    pub install_log: bool,             // default false; binary passes true
}
pub fn boot(storage: &Path, listen: SocketAddr, opts: SimOpts)
    -> Result<SimHandle, String>;      // listen may be 127.0.0.1:0

impl SimHandle {
    pub fn addr(&self) -> SocketAddr;                 // resolved (real port)
    pub fn step(&self) -> Result<serde_json::Value, String>;
    pub fn set_auto(&self, enabled: bool) -> Result<(), String>;
    pub fn peer_block(&self, body: serde_json::Value) -> Result<serde_json::Value, String>;
    pub fn state(&self) -> Result<serde_json::Value, String>;
    pub fn wait(self) -> Result<(), String>;          // binary path: block until exit
    pub fn shutdown(self);                            // embedder path; also on Drop
}
```

The handle is fully synchronous (methods ride the existing `SimCommand`
mpsc with `blocking_send` + oneshot replies) — an embedder needs no tokio
runtime. `boot` owns its threads: the existing actor `std::thread` plus a
serve thread running a private runtime for `axum::serve`. Control surface =
the same `SimCommand`s the `/sim/*` routes use; the HTTP routes stay.

## Embedding hazards fixed at the source

These are the three things that today make two-instances-per-process or
in-process embedding unsafe; the lib fixes them rather than documenting
around them:

1. **Global tracing/panic-hook** — `noded::log::init` installs a
   process-global subscriber and *stacks* a panic hook on every call. Gated
   behind `opts.install_log` (binary: true; embedders: false).
2. **`std::process::exit(1)` on fatal** — the two fatal-submit sites in
   `commit`/`commit_batch` would kill the embedding test process. They
   become: mark fatal, error the pending submit, request shutdown. The
   binary preserves its exit-1 semantics via `SimHandle::wait()`.
3. **Implicit storage default** (`temp_dir()/ducktape-simnode-{pid}`) —
   `boot` requires an explicit dir; the default stays in `main`.

Also: `println!` boot lines move to `main` (or become `tracing` behind
`install_log`) — repo logging doctrine; a lib must not print.

## Shutdown contract

`shutdown()` (and Drop) must leave no running threads: request node
shutdown (the `pub(crate) request_shutdown` on `noded::NodeHandle` becomes
`pub` — one-line noded change), join the serve thread, close the control
channel so the actor loop exits, join the actor thread. The embed test
asserts the port refuses connections afterwards.

## Proof (the lib's own tests, `bin/simnode/tests/embed.rs`)

1. `boot` on `:0` → HTTP `/v1/status` answers on `addr()`; auto-mode submit
   commits and queries back — all in-process, no `CARGO_BIN_EXE`.
2. **Two instances in one process**: independent storage, independent
   heights after stepping one of them.
3. `shutdown()` → subsequent TCP connect to `addr()` fails.

Existing gates stay green: all `bin/simnode/tests/*` suites (binary path),
`cargo clippy -p simnode --tests --no-deps`, and — since `handle.rs` is
touched — `cargo clippy -p noded --tests --no-deps` + `cargo test -p noded`.

## Cut on purpose

- No persona setter on the handle (set via `SimOpts` or HTTP; YAGNI).
- No async handle variant; no builder pattern — one options struct.
- No extraction of the noded `LogRing` multi-instance wiring: the first
  `install_log=true` instance owns the global ring, which only the binary
  uses.
