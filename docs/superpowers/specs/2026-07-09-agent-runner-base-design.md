# Agent runner base v1

**Date:** 2026-07-09
**Status:** approved for implementation

## Summary

Agent runs use Ducktape's existing dispatch, capability, and duckfs systems as a
portable execution layer. This design does not add a new consensus module.
Instead, `runs` commits a `RunEnvelope v3` dispatch payload that includes the
agent prompt hash, transcript, base tool manifest, and duckfs workspace
coordinates. The host runner executes against that workspace and returns a
`RunnerResult v1` receipt whose `response_text` is validated by `runs` as the
existing `AgentResponse`.

Native provider sessions remain valid for legacy v2 payloads, but v3 is
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
| `capability-host` | Host-local executor surface; v3 runs override cwd/env/PATH and disable native sessions. |
| `dispatch-oracle` | Host worker that parses v2/v3 envelopes and submits provider output as saga results. |
| `agent` | Registry for agent id, capability tag, prompt hash, and allowed actions. |
| `runs` | Composes v3 payloads and validates `RunnerResult v1.response_text`. |

| Tool class | v1 role |
| --- | --- |
| Read tools | Chat search, pages search, task views, duckfs read/stat/ls/find/grep/diff/history/refs, node status, and recent blocks. |
| Write path | Only edits inside the checked-out duckfs workspace; the runner commits the final workspace result. |
| Not base v1 | Vaults, governance, upgrade, valset, identity mutation, automations mutation, inbox mutation, huddle/media, overlay networking, and forge push/review writes. |

## `RunEnvelope v3`

`runs` serializes fields in a fixed struct order. Existing v2 fields remain:

- `ducktape_run: 3`
- `agent_id`
- `prompt_hash`
- `thread_key`
- `instructions`
- `contract`
- `conversation`

v3 adds:

- `workspace`: `source_prefix`, `source_snapshot`, and `mount_path`
  (`/workspace` by default). When `runs` is wired to the `files` module, it pins
  the current duckfs head as `source_snapshot`; harnesses without `files` commit
  `null` explicitly.
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

- `workdir_override`: the mounted duckfs workspace path.
- `env`: run-scoped environment for tool bindings and workspace metadata.
- `path_entries`: run-scoped `PATH` prefixes.
- `portable`: disables host-local native session resume/capture.

`dispatch-oracle` sets these fields only for v3. v2 behavior is unchanged.

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

The committed envelope and host context are in place. The remaining runner
piece is the host-side wrapper that creates a managed duckfs workspace before
provider execution, commits it after execution, and emits the populated
`workspace_receipt`.
