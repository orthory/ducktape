# Authoring wasm modules

How to write, build, and live-update a Ducktape wasm module. The runtime is
`crates/kernel/wasm-host` (wasmtime, pinned `=46.0.1`); the authoring contract is
the `ducktape:module` WIT world (`crates/kernel/module-guest/wit/module.wit`);
the reference modules are `crates/guests/hello-wasm` (v1),
`crates/guests/hello-wasm-v2` (its live-update target), and
`crates/guests/sibling-wasm` (the cross-module-read reference). The first
REAL production tenant is `crates/guests/directory-wasm` — the wasm port of
the `directory` module, bytes-compatible with the native implementation it
replaced (same root, same snapshot encoding: the cutover left the app-hash
untouched).

## The model (design-B: host-owned state, guest as pure logic)

A wasm module is PURE LOGIC over the host surface. It holds **no durable memory
across dispatches** — the host re-instantiates a fresh component instance per
call — so all durable state lives host-side, behind the `host.state-*`
capability, staged during `execute` and published only at the block-commit
boundary. Consequences you design around:

- `root()` is host-computed from the host-owned store, never by the guest. A
  code swap keeps the store, so the module's root — and with it the app-hash —
  is byte-identical across the swap. **That is the live-update primitive.**
- Never cache anything in guest globals/statics expecting it to survive: it
  will not. Read what you need via `state-get`, write via `state-set`.
- Determinism is by construction: fresh instance per dispatch, fuel-metered
  termination, no clock / rng / net / ambient imports (the WIT surface simply
  has none). A trap (including out-of-fuel) is a deterministic rejection — the
  same on every validator — and the host rolls the op back.
- The runtime envelope canonicalizes NaNs and trims every wasm proposal the
  integer/bytes ABI doesn't need (no SIMD/threads/GC/…): float math in a guest
  is defined, identical-everywhere behavior, and the surface can't grow by
  accident.

## The contract

Implement the two exports:

- `execute(payload) -> result<_, error>` — apply one op addressed to this
  module. Reject unknown ops with `error::rejected(..)`; a rejection is a clean
  deterministic no-op, never a fork.
- `query(req) -> result<list<u8>, error>` — a read-only projection over LIVE
  state (the staged overlay on committed — the same read-your-writes surface a
  native module's query serves; out of block the overlay is empty).

And use the imports deliberately: `get-env` for the deterministic block env
(`height`, `consensus-time`, `protocol-version`, `origin`, `me`); `state-*` for
durable state; `emit-msg` for write intents at sibling modules (drained as
follow-up ops, never reentrant); `emit-event` for observability records.

### Sibling reads (`module-root` / `query-module`)

Cross-module READS work from inside a guest: `module-root(target)` is the
sibling's snapshot root as of dispatch start, `query-module(target, req)` a
live host-routed read (self-query is rejected; query cycles are rejected by the
host). Mechanically they are **memoized replay**: the sync guest world cannot
await the host's async ctx, so a read the per-dispatch memo cannot answer
pauses the run, the host resolves it, and the pure guest re-runs with the
answer memoized. Design consequences:

- Reads are answered CONSISTENTLY within one dispatch (the memo), and a
  repeated read costs nothing extra.
- Keep reads bounded: a dispatch may touch at most `MAX_SIBLING_READS` (64)
  DISTINCT reads — one more is a deterministic rejection. Every distinct read
  also costs one replay of your `execute`, so put cheap guards before
  expensive read chains.
- `execute` must stay pure over its inputs (state + env + payload + read
  answers) — that is what makes the replay invisible. Guest globals/statics
  were already worthless (fresh instance per dispatch); the replay is one more
  reason.
- The ctx-less direct `Module::query` path (not host-routed) is SEALED: there,
  `module-root` answers none and `query-module` answers `unsupported`.
  Host-routed queries (`ctx.query` from a native peer, external node queries)
  resolve sibling reads for real.

## State layout is the compatibility contract

A live update swaps CODE while KEEPING the store. Version N+1 of your module
reads the store version N wrote, so the state layout — keys and value
encodings — is your compatibility surface across updates:

- Keep it byte-stable, or make N+1 read both layouts and migrate lazily on
  write. There is no offline migration hook: the swap boundary is a plain block
  boundary.
- `hello-wasm-v2` demonstrates the discipline: same `count` key, same
  little-endian `u64` value, different logic (`inc` steps 100, not 1).

## Build: crate → component

Guest crates are standalone workspaces (never members of the root workspace)
building to `wasm32-unknown-unknown`, componentized with `wasm-tools`:

```
make wasm-modules        # rebuild every guest module + refresh ALL committed copies
make wasm-modules-check  # the drift gate: every committed copy is byte-identical
```

One-off equivalent, per crate:

```
cd crates/guests/hello-wasm
cargo build --target wasm32-unknown-unknown --release
wasm-tools component new target/wasm32-unknown-unknown/release/hello_wasm.wasm -o component.wasm
```

The committed copies of one module's component MUST stay byte-identical (the
node embeds the canonical artifact — its sha256 is the genesis-seeded active
hash — and the kernel test fixtures pin the same bytes). Component bytes are
toolchain-dependent, so `wasm-modules-check` gates mutual consistency, not
reproducibility; refresh the whole set together with `make wasm-modules` and
commit it as one change. The check rides the pre-push `make test` gate.

## Live update: how new code ships

The consensus commitment to WHICH code a module runs is the code registry
(`crates/modules/system/lifecycle`): per module, the active 32-byte sha256 of its component
bytes plus at most one pending height-gated swap. The BYTES travel out-of-band,
content-addressed on the node blob plane. The flow:

1. Build the new component; note `sha256(component.wasm)`.
2. Stage the bytes on the blob plane so every node holds them before the
   boundary (a node lacking the bytes at the boundary FAILS CLOSED — it stops
   rather than forks — so distribute first, then schedule).
3. Drive governance: `GovAction::UpdateModule { name, module_id,
   activation_height, code_hash }` — a member-gated proposal + majority tally;
   on passing it emits `LifecycleMsg::ScheduleSwap` into the registry. Cancel before
   the boundary with `GovAction::CancelModuleUpdate`.
4. At the first applied block at/after `activation_height`, two things happen
   on every node: the drain's injected lifecycle `Advance` flips the committed
   active hash (in the app-hash), and the host's out-of-block realization
   (`Host::realize_module_swaps`) verifies `sha256(bytes) == hash` and swaps
   the running component, keeping the host-owned state.

The realization is keyed purely on committed registry state + height, so the
live drain, restart replay, and state-sync catch-up all land the identical swap
points — and a state-sync joiner that installs post-activation state reconciles
its genesis component to the committed ACTIVE hash before applying its first
block.

## Testing a module

- Runtime-level: `crates/kernel/wasm-host/tests/dispatch.rs` — the
  staged-writes / commit / abort / determinism / hot-swap / snapshot proofs —
  and `crates/kernel/wasm-host/tests/sibling.rs` — the sibling-read proofs
  (replay convergence, memoization, no staging leak across replays, budget).
- Host-level: `crates/kernel/host/tests/module_swap.rs` — the full live-update
  boundary (schedule → realize at H → new logic over kept state, fail-closed,
  joiner reconciliation, cross-node determinism) — plus
  `crates/kernel/host/tests/cross_module.rs` (a native peer composing with a
  wasm module) and `crates/kernel/host/tests/wasm_cutover_parity.rs` (the
  native↔wasm byte-compatibility proof for the directory cutover).
- Authorization-level: `crates/modules/system/governance/tests/
  governance_schedules_module_update.rs` — ballot → registry acceptance.

## Porting a native module (the cutover pattern)

The `directory` port is the template: split the native crate so its wire types
compile standalone (`default-features = false`, the `files`/`duckfs-core`
shape), author the guest against those SAME wire types (drift is then a compile
error), and choose the guest's state layout so the host store's canonical
encoding reproduces the native root — if the native module already hashes
`le-u64 count ‖ sorted (len‖key ‖ len‖value)`, storing the raw key/value bytes
makes root(), snapshot(), and install() BYTE-IDENTICAL across the cutover: the
app-hash does not move and pre-cutover workspaces restore unchanged. Pin that
claim with a parity test before wiring the module into `host_state`.

Point your module's tests at a committed fixture (`include_bytes!`) so the
proof is self-contained, and register the fixture in `make wasm-modules` so it
can never drift from the source crate.
