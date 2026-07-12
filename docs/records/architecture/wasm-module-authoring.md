# Authoring wasm modules

How to write, build, and live-update a Ducktape wasm module. The runtime is
`crates/kernel/wasm-host` (wasmtime, pinned `=46.0.1`); the authoring contract is
the `ducktape:module` WIT world (`crates/kernel/module-guest/wit/module.wit`);
the reference modules are `crates/examples/hello-wasm` (v1) and
`crates/examples/hello-wasm-v2` (its live-update target).

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

## The contract

Implement the two exports:

- `execute(payload) -> result<_, error>` — apply one op addressed to this
  module. Reject unknown ops with `error::rejected(..)`; a rejection is a clean
  deterministic no-op, never a fork.
- `query(req) -> result<list<u8>, error>` — a read-only projection over
  committed state (the host serves it with no staging overlay).

And use the imports deliberately: `get-env` for the deterministic block env
(`height`, `consensus-time`, `protocol-version`, `origin`, `me`); `state-*` for
durable state; `emit-msg` for write intents at sibling modules (drained as
follow-up ops, never reentrant); `emit-event` for observability records.

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
cd crates/examples/hello-wasm
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
(`crates/system/modreg`): per module, the active 32-byte sha256 of its component
bytes plus at most one pending height-gated swap. The BYTES travel out-of-band,
content-addressed on the node blob plane. The flow:

1. Build the new component; note `sha256(component.wasm)`.
2. Stage the bytes on the blob plane so every node holds them before the
   boundary (a node lacking the bytes at the boundary FAILS CLOSED — it stops
   rather than forks — so distribute first, then schedule).
3. Drive governance: `GovAction::UpdateModule { name, module_id,
   activation_height, code_hash }` — a member-gated proposal + majority tally;
   on passing it emits `ModregMsg::Schedule` into the registry. Cancel before
   the boundary with `GovAction::CancelModuleUpdate`.
4. At the first applied block at/after `activation_height`, two things happen
   on every node: the drain's injected modreg `Advance` flips the committed
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
  staged-writes / commit / abort / determinism / hot-swap / snapshot proofs.
- Host-level: `crates/kernel/host/tests/module_swap.rs` — the full live-update
  boundary (schedule → realize at H → new logic over kept state, fail-closed,
  joiner reconciliation, cross-node determinism).
- Authorization-level: `crates/system/governance/tests/
  governance_schedules_module_update.rs` — ballot → registry acceptance.

Point your module's tests at a committed fixture (`include_bytes!`) so the
proof is self-contained, and register the fixture in `make wasm-modules` so it
can never drift from the source crate.
