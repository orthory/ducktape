# Agent MCP tool plane — design

Date: 2026-07-12
Status: accepted, implementing

## The problem

A Ducktape agent run is blind and one-shot.

The runs module composes an envelope in consensus, the dispatch-oracle pool
hands it to a `codex exec` / `claude -p` child in a per-run duckfs (or forge)
checkout, and the child's only channels are:

- **in** — whatever the envelope pre-injected (instructions, contract,
  rendered conversation, forge item context) plus the files in its workspace.
- **out** — one `AgentResponse` JSON with at most 8 actions, plus whatever it
  wrote in the workspace (committed back by the provisioner).

It cannot look anything up. It cannot read the channel it was not anchored in,
the task it was asked about, the page it must comment on, the sibling issue, or
a file outside its own checkout. Everything must be foreseen by the composer.

Three rails already exist for fixing this, all half-built:

| Rail | State on `dev` |
|---|---|
| `base_tools` in the v3 envelope (`ducktape-files`, `ducktape-index`, `ducktape-chain`, `exposure: "cli"`) | Composed, validated at accept, **bound to nothing.** |
| `ProvisionedWorkspace::path_entries()` | Wired through to the child's `PATH` — and returns an empty `Vec`. |
| `SkillRef` → duckfs ro-mounts → `DUCKTAPE_RUN_SKILLS` | Materializes correctly. The prompt never mentions it. The code comment says: *"nothing reads it yet."* |
| `ResourceCaps.tools` / `CapRequest::Tool` ("tool / mcp ids this agent may invoke") | Defined. Nothing calls it. |

## Why MCP, not a CLI on `PATH`

The obvious lazy move is to bind the already-declared `exposure: "cli"` tools —
put `ducktape-fs` on the run's `PATH` and let the agent shell out.

**It does not work under codex.** The codex spec runs `--sandbox
workspace-write`, which disables network access. A CLI talking to the node over
local HTTP would fail inside that sandbox, silently and per-run.

MCP servers are spawned by the runner CLI itself, *outside* the agent's
sandbox. So MCP is not merely the nicer ergonomic here — under codex it is the
only door that opens at all. It is also self-describing (`tools/list` carries
schemas, `initialize` carries server instructions), which removes the need for
a separate "how to use Ducktape" document shipped alongside.

## Architecture

```
   consensus                     host                        run child
  ┌──────────┐   envelope   ┌──────────────┐   spawn   ┌─────────────────┐
  │   runs   │─────────────▶│ oracle pool  │──────────▶│ codex / claude  │
  │ composer │              │ + provisioner│  env+PATH │  (sandboxed)    │
  └──────────┘              └──────────────┘           └────────┬────────┘
       ▲                            │                           │ stdio MCP
       │ AgentResponse              │ DUCKTAPE_NODE             │ (outside
       │ (≤8 actions, validated     │ DUCKTAPE_RUN_AGENT        │  sandbox)
       │  on-chain)                 ▼                           ▼
       │                     ┌─────────────┐  HTTP    ┌──────────────────┐
       └─────────────────────│    noded    │◀─────────│   ducktape-mcp   │
                             │ /v1/query   │          │  (new binary)    │
                             │ /v1/submit  │          └──────────────────┘
                             └─────────────┘
```

`ducktape-mcp` is a stdio JSON-RPC MCP server. The runner spawns it; it reads
its run identity from the environment the provisioner set, and speaks to the
local node over the HTTP surface that already exists (`/v1/query`,
`/v1/submit`, `/v1/files/*`).

### Run identity comes from the environment, not from arguments

The provisioner injects two variables into `RunContext.env` (which
capability-host already merges into the child, and which the child's MCP
subprocess inherits):

- `DUCKTAPE_NODE` — the node's loopback HTTP base (the same variable
  `ducktape-fs` already reads).
- `DUCKTAPE_RUN_AGENT` — the `agent_id` this run belongs to.

Everything else — owner, `allowed_actions`, `ResourceCaps`, skills — the MCP
server reads back **from consensus** by querying the agent registry with that
id. Nothing about the grant is duplicated into the environment, so nothing can
drift out of sync with the committed record.

`DUCKTAPE_RUN_WORKSPACE` and `DUCKTAPE_RUN_SKILLS` are already set by the
duckfs lane; the forge lane gains the same treatment.

### Tool surface

Read tools are ungated except by `ResourceCaps` where the caps vocabulary
already names the resource:

| Tool | Backing | Gate |
|---|---|---|
| `ducktape_whoami` | agent registry | — |
| `ducktape_chat_channels`, `ducktape_chat_messages` | `ChatQuery` | — |
| `ducktape_tasks`, `ducktape_task` | `TaskQuery` | — |
| `ducktape_page` | `PageQuery` | — |
| `ducktape_forge_repos`, `ducktape_forge_items`, `ducktape_forge_item` | `ForgeQuery` | `forge_read` |
| `ducktape_files_ls`, `ducktape_files_read`, `ducktape_files_grep` | `/v1/files/*` | `duckfs_read` |

Write tools mirror the **existing** `KNOWN_ACTIONS` vocabulary exactly — the
same five names the response contract already validates against, so the tool
plane grants an agent nothing its registered `allowed_actions` did not already
grant it:

| Tool | Action name | Extra gate |
|---|---|---|
| `ducktape_chat_post` | `chat.post` | — |
| `ducktape_task_create` | `tasks.create` | — |
| `ducktape_task_status` | `tasks.update_status` | — |
| `ducktape_page_comment` | `pages.comment` | `pages_write` |
| `ducktape_page_check` | `pages.set_checked` | `pages_write` |

Gating reuses `agent::AgentRecord::permits` and `agent::KNOWN_ACTIONS`
directly — one vocabulary, one enforcement function, two call sites (the
on-chain response path and this one).

### Attribution, and the ceiling on it

Writes go out through `/v1/submit` with `origin` set to the agent's **owner**
bytes, read off the committed `AgentRecord`. Chat's `AuthorRef::User` and every
other origin-derived authorship therefore names the owner, exactly as when the
same user acts through the desktop app.

`/v1/submit`'s `origin` field is documented in `bin/noded/src/lib.rs` as *"a
TRUSTED-CLIENT convention, not authentication: anything that can reach the port
can claim any origin."* That is the honest ceiling of this write plane:

- **It opens no new hole.** Under `claude` (no sandbox) a run child can already
  `curl` `/v1/submit` and claim any origin it likes. The MCP plane makes that
  ambient capability explicit, gated by the agent's own committed grant, and
  routed through one auditable binary.
- **It is weaker than the response path.** Response actions are validated
  *in consensus* against `allowed_actions` + caps. MCP writes are validated
  *host-side*, in `ducktape-mcp`, before the submit. Under codex's network-less
  sandbox that is a real boundary (the MCP server is the only door). Under
  claude it is a guardrail, not a sandbox.
- **Upgrade path**, when `/v1/submit` grows real submitter auth (tracked as the
  blocker on #235): a `RunAction { saga_id, action }` op validated in the runs
  module against the saga's committed lease-holder, reusing `response.rs`'s
  validation verbatim. This is marked in-tree with a `ponytail:` comment rather
  than being built speculatively.

### Wiring the runner CLIs

The capability spec gains one optional section:

```toml
[tools]
# Inserted immediately after args[0] — the mode/subcommand selector — in the
# base args AND every variant's args, on both the invoke and resume paths.
# The trailing stdin marker therefore stays last, which codex requires.
args = ["-c", 'mcp_servers.ducktape.command="ducktape-mcp"']
```

- **codex**: `-c mcp_servers.ducktape.command="ducktape-mcp"`
- **claude**: `--mcp-config '{"mcpServers":{"ducktape":{"command":"ducktape-mcp"}}}'`
  plus `--allowedTools mcp__ducktape` (in `-p` print mode an unapproved MCP
  tool call is a denial, so the server must be pre-allowed).

The command is the bare binary name because the provisioner puts its directory
on the child's `PATH` via `path_entries()` — resolved from
`std::env::current_exe()`, so `ducktape-mcp` ships beside `noded`/`node` and
needs no configured path.

A host without `ducktape-mcp` installed, or a dev/test run with no
`DUCKTAPE_NODE` in the environment, degrades cleanly: the server starts,
`tools/list` still answers, and every call returns a "no node bound" error
rather than killing the run.

### Skills: making the existing mount visible

The skill rail is already built and already invisible. Two changes finish it:

1. **The envelope announces it.** The composer already knows the run's resolved
   skill list (it puts it in the v3 envelope). It gains a deterministic
   instructions section naming each mounted skill and the `DUCKTAPE_RUN_SKILLS`
   root they land under. Composed in consensus from the envelope's own fields —
   no host input, no non-determinism.
2. **The forge lane mounts them too.** The duckfs lane materializes ro-mounts;
   the forge lane currently refuses runs that request them. It gains the same
   sibling-`-ro` checkout.

The "how to use Ducktape" guide itself needs no skill file: MCP's `initialize`
response carries server `instructions`, which both runners surface to the
model. That is where the guide lives, versioned with the binary.

## Layout

`bin/mcp` → binary `ducktape-mcp`, split by responsibility (no mono-file):

- `main.rs` — stdio JSON-RPC loop: `initialize`, `tools/list`, `tools/call`.
- `node.rs` — the node client: `/v1/query`, `/v1/submit`, `/v1/files/*`.
- `identity.rs` — env → run identity → committed `AgentRecord`; the caps gate.
- `tools/read.rs`, `tools/write.rs` — the tool table and handlers.
- `guide.rs` — the `initialize` instructions text.

Changed elsewhere:

- `crates/kernel/capability-host/src/spec.rs` — the `[tools]` section.
- `crates/kernel/capability-host/specs/{codex,claude}.toml` — the MCP args.
- `bin/noded/src/agent_provision{,/duckfs.rs,/forge.rs}` — `path_entries()`,
  `DUCKTAPE_NODE`, `DUCKTAPE_RUN_AGENT`, forge-lane ro-mounts.
- `bin/noded/src/oracle_pool.rs`, `bin/noded/src/main.rs`,
  `bin/node/src/boot/surfaces.rs` — thread the node's HTTP base to the
  provisioner.
- `crates/apps/runs/src/envelope.rs` — the skills instructions section.

## Testing

- `capability-host`: `[tools]` args land after `args[0]` in base, every variant,
  and both resume paths.
- `ducktape-mcp` unit: tool dispatch, schema shape, and the caps gate — a
  denied action never reaches the node client.
- `ducktape-mcp` e2e (mirrors `bin/fs`'s `cli_e2e`): stand a real node with the
  agent/chat/tasks modules behind `noded::router`, register an agent with a
  narrow grant, drive the binary over stdio, and assert that a granted write
  lands on-chain while a denied one is refused without a submit.
- `runs`: the envelope's skills section is deterministic and absent when the
  run mounts no skills.
