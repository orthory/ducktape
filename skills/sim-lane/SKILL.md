---
name: sim-lane
description: Use when any Rust #[test] in this workspace needs a deterministic in-process Ducktape node (no child processes, no fleet) — boot it with simnode::boot from bin/simnode, drive real submit → commit → query round-trips, and step commits deterministically. Covers the embedding harness, SimOpts, and the chat-module wire shapes those tests hit. (The old iced-UI sim lane died with the app/src-iced shell; app/ itself is live and has its own in-crate tests.)
---

# Sim lane — deterministic in-process node

`simnode::boot` (`bin/simnode/src/lib.rs`) boots a deterministic Ducktape node
in-process: the full noded `/v1` HTTP surface plus a synchronous control handle,
no child processes and no timers. Any crate's `#[test]` can drive real
transaction round-trips (submit → commit → query) against it.

> The UI half of this lane died with the `app/src-iced` shell it lived in
> (`SimShell`, the `iced_test::Simulator` harness, the composer/`rich_text`
> traps). **`app/` itself was rewritten in place, not removed** — the live
> `ducktape-app` crate has ordinary `#[cfg(test)]` suites (`cargo test -p
> ducktape-app`) and no simulator lane. Only the embeddable node below survives.

## Where things live

| Thing | Path |
|---|---|
| Embeddable node lib + doc comment | `bin/simnode/src/lib.rs` |
| Cross-crate embedder example | `bin/simnode/tests/embed.rs` |
| Node-semantics scenario suites | `bin/simnode/tests/*.rs` (`cargo test -p simnode`) |

## Embedding the node in any crate's test

```toml
[dev-dependencies]
simnode = { path = "../../bin/simnode" }   # adjust depth
```
```rust
let storage = tempfile::tempdir()?;        // FRESH dir per test — determinism
let sim = simnode::boot(
    storage.path(),
    "127.0.0.1:0".parse()?,                // ephemeral; real port via sim.addr()
    simnode::SimOpts { auto: true, ..Default::default() },
)?;
// HTTP: full noded /v1 surface at sim.addr(). Control: sync handle —
// step()/set_auto()/peer_block()/state()/shutdown() (Drop also tears down).
```

`SimOpts` for embedders: keep `install_log: false` (true stacks a
process-global tracing subscriber + panic hook — binary only). `auto: true`
= submits commit inline; `false` = held mode, commits happen on `step()`
(races as scripts). Governance scenarios: `valset_keys` (raw 32-byte ed25519
pubkeys) + `invite_binding`; `node_key` fabricates `status.public_key`;
`persona` picks daemon (`op_hash` receipts) vs validator (height-only) shape.

`modules_dir` (binary: `--modules <dir>`) is where the genesis reads each
tenant's `<id>.component.wasm`. Leave it `None`: the default is the repo's
`crates/kernel/host/tests/fixtures`, resolved from `CARGO_MANIFEST_DIR`, so a
bare checkout boots with no `make install-node` and no installed module dir
(`<ducktape home>/modules`, i.e. `$DUCKTAPE_HOME` when set, else `~/.ducktape`).
All 15 default tenants ARE wasm components (`--with-valset` adds acl and
governance as components too; only kv/valset/lifecycle stay native), so a sim
boot cranelift-compiles the set — seconds, and paid per boot, per test.
After a host fatal the control surface fails closed (every call errs with
the reason); the *triggering* call may still return Ok — check the next one.

**Tests wait on events, never on time.** The control handle's `step()` /
`state()` are the synchronization seam — no sleeps, no polling. If a test
seems to need a wait, the flow is broken, not slow.

## Chat module wire facts (safe to rely on, verified in-tree)

- Replies are externally tagged: `{"channels":[..]}`, `{"messages":[..]}`.
- `messages_latest` returns messages **ascending by `seq`**
  (`crates/modules/apps/chat/src/lib.rs`) — order assertions are sound.
- Frames carry a caller-supplied signature and the node verifies their origin;
  never expect a real `ducktape` subprocess inside an in-process test.
