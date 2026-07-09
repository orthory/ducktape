# Agent runner base v1

**Date:** 2026-07-09
**Status:** approved for implementation — landing in the phased order of ADR
`2026-07-09-deterministic-agent-runtime` (accept → wrapper → flip).

> **Rollout status.** This document describes the TARGET `RunEnvelope v3` /
> `RunnerResult v1` contract. The base slice that has landed is the
> **acceptance** half only: the host worker (`dispatch-oracle`) parses and
> validates v3 and marks such runs portable, and `runs` unwraps a
> `RunnerResult`. The composer stays on **v2** and the host does NOT activate a
> workspace mount yet — composing v3 is a consensus flag-day (ADR M1) and
> pointing a run's cwd at the constant `/workspace` is unwritable on a non-root
> node, so the flip waits for the provisioning wrapper and a coordinated
> upgrade (ADR ROL/M2, W1). Sections below marked _(deferred: flip)_ describe
> the post-wrapper state.

## Summary

Agent runs use Ducktape's existing dispatch, capability, and duckfs systems as a
portable execution layer. This design does not add a new consensus module.
Instead, `runs` composes a `RunEnvelope` dispatch payload that includes the
agent prompt hash, transcript, and — once the flip lands — a base tool manifest
and duckfs workspace coordinates. The host runner executes against that
workspace and returns a `RunnerResult v1` receipt whose `response_text` is
validated by `runs` as the existing `AgentResponse`.

Native provider sessions remain valid for legacy v2 payloads; a v3 run is
portable by construction: the resumable state is the duckfs snapshot and the
conversation transcript, not an executor-local session id.

## Service matrix

| System | v1 role |
| --- | --- |
| `files` / duckfs | Execution evidence layer: source snapshots, output commits, diffs, and receipts. |
| `duckfs-client` | Host-side checkout, status, and commit engine for managed workspaces. |
| Managed workspace RPC | Creates and closes daemon-owned duckfs checkouts for runner sandboxes. |
| `indexer` | Node-local materialized views used by read tools; not audit-critical state. |
| `dispatch` | Consensus task plane that stores full payload/result bytes and delivers outcomes. |
| `saga` | Worker-request, retry, and oracle-result plumbing already used by dispatch. |
| `capability` | Consensus registry of which nodes can serve which runner tags. |
| `capability-host` | Host-local executor surface; disables native sessions for portable runs and resolves the cwd to a per-run writable path (overriding cwd/env/PATH from a materialized mount is _deferred: flip_). |
| `dispatch-oracle` | Host worker that parses v2/v3 envelopes, marks v3 runs portable, and submits provider output as saga results. |
| `agent` | Registry for agent id, capability tag, prompt hash, and allowed actions. |
| `runs` | Composes the payload (v2 today) and validates `RunnerResult v1.response_text`. |

| Tool class | v1 role |
| --- | --- |
| Read tools | Chat search, pages search, task views, duckfs read/stat/ls/find/grep/diff/history/refs, node status, and recent blocks. |
| Write path | Only edits inside the checked-out duckfs workspace; the runner commits the final workspace result. |
| Not base v1 | Vaults, governance, upgrade, valset, identity mutation, automations mutation, inbox mutation, huddle/media, overlay networking, and forge push/review writes. |

## `RunEnvelope v3` _(worker-accepted; composer-emitted only after the flip)_

`runs` serializes fields in a fixed struct order. The v2 fields the composer
emits today are:

- `ducktape_run: 2` (bumped to `3` at the coordinated flip)
- `agent_id`
- `prompt_hash`
- `thread_key`
- `instructions`
- `contract`
- `conversation`

v3 adds (understood by the worker now; emitted at the flip):

- `workspace`: `source_prefix`, `source_snapshot`, and `mount_path`. The mount
  path is a host materialization detail, not a consensus-fixed constant — the
  provisioning wrapper chooses a per-run writable path relocated out of the
  node's sensitive data tree (ADR W1/D7); a constant `/workspace` is exactly the
  unwritable cwd this ordering avoids. `source_snapshot` pins the current duckfs
  head when `runs` is wired to the `files` module (wiring lands with the flip);
  harnesses without `files` commit `null` explicitly.
- `base_tools`: deterministic manifest of enabled host bindings:
  `ducktape-files@1`, `ducktape-index@1`, and `ducktape-chain@1`, all exposed as
  CLI-style tools.
- `result_contract`: `{ "ducktape_runner_result": 1 }`.

## `RunnerResult v1`

The provider returns JSON:

- `ducktape_runner_result: 1`
- `response_text`: the model's final text; `runs` unwraps this and validates it
  as the existing `AgentResponse` contract.
- `workspace_receipt`: `source_prefix`, `source_snapshot`, `output_snapshot`,
  `commit_height`, `rebased`, and `no_changes`.

The dispatch history keeps the full provider bytes for audit. `runs` only
unwraps `response_text` and treats malformed or unsupported runner results as a
failed run result, never as a delivery-block abort. Legacy raw text remains
accepted for v2 and in-flight runs.

## Host execution context

`capability-host::RunContext` carries portable fields in addition to
`agent_id` and `thread_key`:

- `workdir_override`: the mounted duckfs workspace path _(deferred: flip — set
  by the provisioning wrapper, not from a consensus mount path)_.
- `env`: run-scoped environment for tool bindings and workspace metadata
  _(deferred: flip)_.
- `path_entries`: run-scoped `PATH` prefixes _(deferred: flip)_.
- `portable`: disables host-local native session resume/capture.

For a v3 envelope today, `dispatch-oracle` validates the portable block and sets
only `portable`; it does not force `workdir_override`/`env`/`PATH` from the
envelope. The host resolves the cwd through its own scratch/persistent policy,
always to a per-run writable path with a scratch fallback (`ensure_writable_workdir`),
so an unwritable preferred path never fails the run. v2 behavior is unchanged.

## Tool binding contract

The v1 base tools are host-side bindings, not consensus state:

- `ducktape-files`: duckfs read/stat/ls/find/grep/diff/history/refs over the
  pinned workspace or an explicit snapshot.
- `ducktape-index`: chat, pages, and tasks materialized views through the
  existing index routes.
- `ducktape-chain`: node status and recent block facts.

No generic write tool is exposed in v1. Writes happen by modifying the mounted
workspace and committing the workspace as the runner result.

## Open follow-up

The worker-side acceptance (v3 parse/validate + portable flag), the host
`RunContext` fields, the `RunnerResult` unwrap, and the W1 writable-workdir
fallback are in place. The remaining pieces, in the ADR's order:

1. **Wrapper.** The host-side provisioning wrapper that relocates the agent
   workspace root out of the sensitive data tree (D7), creates a managed duckfs
   checkout before provider execution, executes in that per-run writable mount,
   commits it after execution, and emits the populated `workspace_receipt`.
2. **Flip.** A coordinated (flag-day) upgrade that bumps the composer to emit
   `RunEnvelope v3` and activates portable execution (the wrapper sets
   `workdir_override`/`env`/`PATH`). Never flip the composer ahead of the
   wrapper (ADR M1/M2).
