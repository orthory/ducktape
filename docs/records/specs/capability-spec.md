# Capability Spec Reference (v1)

A **capability spec** is a TOML file that teaches a Ducktape node how to run
one executor — an installed CLI that can turn a prompt into text. Everything
the node needs is in the file: how to detect the binary, the exact argv to
invoke it, and how to parse its output. **Adding an executor is a config
change, never a code change** — the embedded built-ins are themselves spec
files globbed out of `crates/modules/system/capability-host/specs/` at build time; no
Rust source names an executor.

Specs are the data half of the capability system:

| Layer | Owns | Code |
|---|---|---|
| Consensus (`crates/modules/system/capability`) | *who provides what*, network-wide: node key → announced tag set | never reads specs |
| Host (`crates/modules/system/capability-host`) | *actually running it*: spec loading, binary discovery, spawning, parsing | this document |

The consensus registry only ever sees **tags**. The spec behind a tag is
private to each host — two nodes can announce the same tag with differently
tuned specs and the network cannot tell, by design.

---

## Dispatch: explicit capability, nothing inferred

A job names the capability tag it needs; `ProviderSet::resolve(tag)` maps the
tag to the locally discovered provider. That is the whole dispatch model:

- **Consensus says WHAT** — an agent record carries a `capability` tag, the
  saga trigger carries the same tag, and work is leased over the tag's
  announced providers.
- **The spec says HOW** — binary, flags, sandbox posture, and (if the
  executor takes one) the model, written literally into `[invoke].args`.
  Model choice is host policy; no model name ever crosses consensus.
- **Nothing is inferred.** There is no routing table, no pattern matching,
  no default fallback. A tag either has a loaded spec or resolution fails
  loudly naming the loaded set.

A finer-grained need — "this executor, but pinned to a specific model, with
different flags" — is a **finer tag with its own spec file**, not a routing
rule. Define `tag = "myllm-large"` with the pinning flags in its args, have
agents name that tag, and only hosts carrying that spec (and binary) announce
it. [`[[variants]]`](#variants--one-file-a-family-of-finer-tags) is load-time
sugar for writing a whole family of such finer tags in one file.

---

## Trust model — read this first

A spec names an arbitrary local binary and the argv to run it with. **Loading
a spec is executing code by proxy.** Specs are operator-trusted configuration,
in the same trust class as a shell profile or a systemd unit:

- They load from exactly two places, both local and operator-controlled:
  1. the specs **embedded in the node binary** at compile time
     (`crates/modules/system/capability-host/specs/*.toml`);
  2. the **operator spec directory** (see [Spec sources](#spec-sources)).
- They are **never fetched from the network**, and no consensus code path can
  read one (host-local files are non-deterministic input).
- Argv is **fully literal** and passed to exec **without shell
  interpretation** — no placeholders, no quoting, no expansion. The prompt
  reaches the child only via stdin, so job content cannot inject flags or
  commands.
- The child process runs **fenced**: a working directory the spec's
  `[workspace]` policy picks — an empty scratch dir by default, or a
  per-agent persistent dir under the host's agent-workspaces root (never the
  node's data dir itself) — non-interactive mode, and whatever sandbox flags
  the spec's argv encodes. Fence flags live in the spec — audit them when you
  audit the spec.

**Auth: the operator brings a logged-in CLI; the spec decides where the
credential ends up.** There are exactly two ways, and a spec may use only one:

- **`[isolation]` — the strong path.** The *host* reads the credential and keeps
  it in the node process. A run-scoped, loopback-only broker serves the model
  API, and the child gets only an unrelated opaque run bearer, a localhost base
  URL, and a **fresh, empty config home** (which is what stops the CLI reading
  the operator's real one and forces it through the broker). The credential
  never enters the child's process tree. Codex is here.
- **`[sandbox] rw_dirs` — the weak path.** For a CLI with no broker: its own auth
  dir is mounted into the sandbox and the credential **does** enter the child.
  Claude is here, until an Anthropic-side broker exists.

**Declaring both is a hard load error.** A broker exists so the credential never
reaches the child; `rw_dirs` would mount it in anyway — and the run would still
*work*, so the regression would be silent. The load error is what makes it
impossible: an executor that gains a broker drops its `rw_dirs` in the same edit.

---

## Spec sources

Specs load in two passes:

1. **Embedded built-ins** — every `specs/*.toml` compiled into the node
   (globbed by `build.rs`, sorted by file name). These parse through the
   exact same code path as operator files and serve as the reference
   examples.
2. **Operator directory** — every `*.toml` in:
   - `$DUCKTAPE_CAPABILITY_DIR` if set. Pointing this at a missing directory
     is a **hard error** (you asked for a dir that isn't there);
   - otherwise `~/.ducktape/capabilities`, only if it exists (absent default
     simply means "no operator specs").

**Override rule:** an operator spec whose `tag` matches a built-in **replaces
it wholesale** — there is no field-level merging; the spec file is the unit of
override. This is the supported way to retune a built-in (different sandbox
flags, a different timeout, an explicit model flag): copy the embedded spec,
edit, drop it in the directory.

**Duplicate rule:** two files in the operator directory claiming the same tag
is a **hard error** naming both files. Precedence between operator files would
be file-order guessing; the node refuses to guess.

**Failure posture:** a spec that fails to parse or validate is a **boot
error**, not a skipped file. An operator config mistake should stop the node
loudly, not silently drop an executor and let jobs fail later with a confusing
"capability not provided".

---

## Full annotated example

```toml
# capability spec format version. this build understands exactly 1; any other
# value is rejected loudly so a future-format spec is never silently misread.
spec = 1

[capability]
# the tag announced to the network-wide capability registry. shape rules are
# EXACTLY the consensus module's: 1..=64 bytes of [a-z0-9._-]. a tag that
# validates here can never bounce off an on-chain Announce.
tag = "myllm"
# human-facing one-liner for docs and status surfaces. optional.
description = "example one-shot CLI executor"

[detect]
# the binary name probed on PATH (first executable match wins).
bin = "myllm"
# optional env var naming an EXPLICIT binary path. when set, it wins over the
# PATH probe — and if it points at something that is not an executable file,
# the capability is dropped with a loud warning. an explicit override never
# silently falls back to PATH: you said "use this", and this doesn't exist.
env = "MYLLM_BIN"

[invoke]
# argv, passed to exec() verbatim — never through a shell, no placeholders.
# encode your fence here: sandbox flags, non-interactive mode, turn limits —
# and, if the executor takes one, the model as an ordinary literal flag.
args = ["run", "--json", "--model", "large-v2"]
# how the prompt reaches the child. v1 supports only "stdin": the prompt is
# written to the child's stdin (concurrently with output collection, so huge
# prompts can't deadlock the pipe) and then EOF. an argv placeholder would
# leak prompts into `ps` output and hit ARG_MAX — deliberately unsupported.
prompt = "stdin"
# the child's IDLE budget in seconds (1..=3600, default 300): any output on
# either stream REFRESHES it, so a long agentic run that keeps streaming
# (codex --json events, tool chatter) is never killed mid-work — only a child
# SILENT this long dies. a quiet-by-design CLI (claude -p prints one result at
# the end) gets exactly this many seconds of silence, so budget for its
# longest silent stretch. a continuously-chatty child is still bounded at
# 6x this value (the host hard cap; the saga's consensus deadline bounds the
# run's outcome regardless). $DUCKTAPE_PROVIDER_TIMEOUT_SECS overrides every
# spec's idle budget at once (ops knob for slow hosts).
timeout_secs = 300

[output]
# which NAMED parser extracts the assistant's final text from stdout:
#   "jsonl-events" — a JSONL event stream; the LAST agent_message item wins;
#                    tolerates both item shapes seen in the wild and skips
#                    non-JSON noise lines.
#   "json-result"  — a single {"type":"result",...} object; an is_error
#                    result is surfaced as the error it is.
#   "text"         — raw stdout, trimmed. THE GENERIC ESCAPE HATCH: any CLI
#                    that prints the answer plainly works with zero code.
#                    empty output on a zero exit is an error ("ran fine,
#                    said nothing" is a broken executor, not an answer).
# this is a CLOSED set on purpose: each name is a tested parser for a real
# output contract. a new name is a code change with tests — that's the point.
format = "text"
```

---

## Field reference

### Top level

| Field | Type | Required | Rules |
|---|---|---|---|
| `spec` | integer | yes | must be `1` |
| `[workspace]` | table | no | per-agent persistent working directory — see [Workspace](#workspace--a-persistent-per-agent-working-directory) |
| `[session]` | table | no | thread-continuity capture/resume — see [Session](#session--thread-continuity) |
| `[isolation]` | table | no | host-owned auth broker + fresh executor config home — see [Isolation](#isolation--host-owned-auth) |
| `[sandbox]` | table | no | the executor's own auth dirs, mounted into a sandbox — see [Isolation](#isolation--host-owned-auth) |
| `[tools]` | table | no | argv injected into every argv the file produces — see [Tools](#tools--argv-injected-into-every-argv-the-file-produces) |
| `[[variants]]` | array of tables | no | load-time expansion into finer tags — see [Variants](#variants--one-file-a-family-of-finer-tags) |

Unknown fields **anywhere** in the file are rejected — a typo (or a field
from the retired model-routing era, like `[models]`) fails loud instead of
being silently ignored.

### `[capability]`

| Field | Type | Required | Rules |
|---|---|---|---|
| `tag` | string | yes | 1..=64 bytes, `[a-z0-9._-]` only — the shared consensus rule (`capability_interface::validate_tag`) |
| `description` | string | no | free text |

### `[detect]`

| Field | Type | Required | Rules |
|---|---|---|---|
| `bin` | string | yes | non-empty; probed on `PATH` |
| `env` | string | no | env var naming an explicit binary path; override wins, broken override = warn + absent |

### `[invoke]`

| Field | Type | Required | Rules |
|---|---|---|---|
| `args` | string array | no (default `[]`) | passed verbatim to exec; fully literal, no placeholders |
| `prompt` | string | yes | must be `"stdin"` in v1 |
| `timeout_secs` | integer | no (default `300`) | 1..=3600; IDLE budget — output refreshes it; killed after this much silence, or at 36x regardless |

### `[output]`

| Field | Type | Required | Rules |
|---|---|---|---|
| `format` | string | yes | `"jsonl-events"` \| `"json-result"` \| `"text"` |

### `[workspace]`

| Field | Type | Required | Rules |
|---|---|---|---|
| `mode` | string | yes (when the section is present) | must be `"persistent"`; omit the whole section for the scratch default |

### `[session]`

| Field | Type | Required | Rules |
|---|---|---|---|
| `capture` | string | yes | `"jsonl-events"` \| `"json-result-field:<field>"` |
| `resume_args` | string array | exactly one of the two | FULL replacement resume argv; must carry the `{session_id}` slot |
| `resume_args_append` | string array | exactly one of the two | appended to the spec's own `args`; must carry the `{session_id}` slot |

### `[isolation]`

| Field | Type | Required | Rules |
|---|---|---|---|
| `config_home_env` | string | no | executor config-home env name such as `CODEX_HOME`; must match `[A-Z_][A-Z0-9_]*` |
| `broker` | string | no | currently only `"codex-responses"`; credentials remain in the host process |

### `[sandbox]`

| Field | Type | Required | Rules |
|---|---|---|---|
| `rw_dirs` | string array | no (default `[]`) | the executor's own auth/state dirs, mounted read-write into a sandbox. **Home-relative only** — absolute paths and `..` are rejected at load. **Rejected outright if the spec also declares `[isolation] broker`.** |

### `[tools]`

| Field | Type | Required | Rules |
|---|---|---|---|
| `args` | string array | yes (when the section is present) | spliced into every argv the file produces, immediately after `args[0]` |

---

## Isolation — host-owned auth

An executor CLI needs the operator's model credential. The two sections below are
the two ways to arrange that, and **a spec may declare only one of them** (see
[Trust model](#trust-model--read-this-first)).

### `[isolation]` — the credential stays in the host

```toml
[isolation]
config_home_env = "CODEX_HOME"
broker = "codex-responses"
```

The host reads the credential (`OPENAI_API_KEY`, else `~/.codex/auth.json`) and
keeps it in the node process. The child gets:

- a **loopback base URL** spliced into its argv as a custom model provider, and
- an **opaque random bearer** minted for this run,

neither of which can recover the credential, and both of which die with the run.
`config_home_env` is what makes this real rather than decorative: the child's
`CODEX_HOME` is a **fresh, empty** directory under the run's reserved
`.ducktape-run/` tree, so the CLI *cannot* fall back to reading the operator's
`auth.json`. That reserved directory is deleted before DuckFS or Forge scans the
workspace, so the provider's runtime state can never enter a snapshot or commit.

The broker is deliberately not a generic proxy: it binds an ephemeral loopback
port, requires the per-run bearer, accepts only Responses POSTs, enforces
body/response/total-byte, concurrency and request-count budgets, and is torn down
when the run ends. It substitutes the host credential only on the upstream hop.

**Backends.** Works under `Direct` and `Podman` (whose `--network=host` leaves
the host's loopback reachable from inside the container). **`Tart` is refused
loudly**: a VM guest has its own network stack, so the host's `127.0.0.1` is not
the broker's — every model call would fail as what looks like a broken login.
Giving the guest a host-gateway address is the upgrade path.

### `[sandbox] rw_dirs` — the credential enters the child

```toml
[sandbox]
rw_dirs = ["~/.claude", "~/.claude.json"]
```

For a CLI with no broker. Under a sandbox backend `HOME` is *set* but not
*mounted*, so the node's data dir and user key stay outside (D7); these named
home-relative dirs are the only paths under `HOME` that cross, read-write, at
their identical container paths — enough for the CLI to find its own login.
Inert under `Direct` (the child inherits `HOME` whole).

Entries are validated at load: an absolute path or any `..` segment is rejected,
since either would defeat the boundary the sandbox exists to hold.

---

## Workspace — a persistent per-agent working directory

The v1 fence ran every provider child in an **empty scratch directory**. An
agentic run wants the opposite: a stable directory the executor can read and
write across runs, so work accumulates instead of vanishing per invocation.

```toml
[workspace]
mode = "persistent"
```

With `persistent`, a run that carries an agent identity (composed by the
runs module's envelope — legacy envelope-less runs carry none) executes in
`<data>/agent-workspaces/<agent_id>/`, created on demand. Everything else —
no `[workspace]` section, a legacy run, an embedder that wired no workspaces
root — keeps the scratch fence unchanged. The child still never sees the
node's data directory itself, and the agent id is defensively rejected as a
path component if it carries separators or traversal tokens.

The workspaces root is host policy: the node binaries derive it from their
data dir; `DUCKTAPE_AGENT_WORKSPACES` overrides it. Host-local state, never
consensus — two nodes running the same agent have independent workspaces.

---

## Session — thread continuity

Agentic CLIs keep their own conversation state keyed by a **session id**.
`[session]` teaches the host to capture that id from a successful run's
stdout and to resume it on the next run of the same conversation thread:

```toml
[session]
capture = "jsonl-events"                        # or "json-result-field:session_id"
resume_args = ["exec", "resume", "{session_id}", "--json", "-"]
# or, for flag-style CLIs:
# resume_args_append = ["--resume", "{session_id}"]
```

- **capture** — how the id is read: `"jsonl-events"` scans the event stream
  (`thread.started`.`thread_id`, a top-level `session_id`, or the older
  `session_configured` envelope); `"json-result-field:<field>"` reads the
  named string field of the single result object. Capture is **tolerant**:
  no id in the output means no session is stored — never an error.
- **resume** — how a stored id becomes the next invocation's argv. `resume_args`
  REPLACES the argv wholesale (for CLIs where resuming is a subcommand);
  `resume_args_append` appends to the spec's own `args` (for flag-style
  resuming — every variant keeps its model/effort pins for free). Exactly one
  of the two, and it must use the `{session_id}` slot.

`{session_id}` is the **one substitution in the whole format**, filled
host-side with an id the executor itself minted (validated: short, printable,
space-free) — job content can never reach argv. This is host-local plumbing,
NOT the removed dispatch-time model routing: nothing here touches consensus,
and every tag still resolves to fixed argv shapes known at load time.

Mechanics (all host-local):

- Session ids live in `<data>/agent-sessions/<agent_id>/<sha256(thread_key)>`
  (`DUCKTAPE_AGENT_SESSIONS` overrides the root). The thread key is hashed,
  so any key content is filesystem-safe.
- A **stale session degrades to a cold start**: if the resumed invocation
  fails, the session file is deleted and the run retries ONCE cold before
  reporting failure. Store writes are best-effort (a failure warns and costs
  continuity, never the answer).
- Sessions are **assignee-local by design**: another node executing the same
  thread's next run finds no session file and starts cold — correct, because
  the run envelope carries the full transcript either way.

---

## Tools — argv injected into every argv the file produces

An agentic executor is only as useful as the tools it can reach. `[tools]`
names the flags that wire one in — in the built-ins, the Ducktape MCP server —
without making every argv in the file repeat them:

```toml
[tools]
args = ["-c", 'mcp_servers.ducktape.command="ducktape-mcp"']
```

**The insertion rule: immediately after `args[0]`, never at the end.** An argv
like codex's ends in a bare `-` (the stdin marker) that must stay LAST, so
appending is not an option; `args[0]` is always the mode/subcommand selector
(`exec`, `-p`), so the position right after it is the one slot that is legal
for every executor and stable across variants.

It applies to **every argv the file produces**:

- the `[invoke] args`;
- **every** `[[variants]]` `args` list (variants inherit `[tools]` like they
  inherit everything else — they never repeat it);
- the `[session]` `resume_args` replacement argv (and a variant's own
  replacement), spliced after ITS `args[0]` the same way.

`resume_args_append` is deliberately **not** spliced: that list is a *suffix*
glued onto the spec's own `args`, not an argv of its own — splicing would land
tool flags between a flag and its value. It needs nothing: the args it is
appended to were already injected, so the composed resume argv carries the
tools anyway.

Everything else is the format's usual posture:

- **injection happens once, at load time.** Nothing downstream knows tools
  exist — a spec in hand already has them, and one tag still means one fixed,
  fully literal argv.
- **no `[tools]` section, or an argv with fewer than 1 arg, means no
  insertion** — an older spec is byte-for-byte what it was.
- **override is still wholesale, by tag.** An operator spec that replaces a
  built-in replaces its `[tools]` too: to run an executor without the MCP
  server (or with a different one), copy the embedded spec, edit `[tools]`,
  drop it in the operator dir. There is no field-level merging here either.
- a `[tools]` section with no `args` is a **hard error**, like every other
  section that would do nothing.

The binary a `[tools]` argv names (`ducktape-mcp` in the built-ins) is resolved
from the **run's `PATH`** — the provisioner puts its directory there — so specs
name no absolute path and stay portable across hosts. Claude's built-in also
passes `--allowedTools mcp__ducktape`: in `-p` print mode there is no human to
approve a tool call, so an unapproved MCP call is a denial and a merely
*configured* server would be dead weight.

---

## Variants — one file, a family of finer tags

`[[variants]]` is **load-time sugar** over the finer-tag pattern above: each
entry registers an ADDITIONAL spec under the composed tag
`{parent_tag}_{suffix}`, exactly as if you had written one more spec file for
it. Nothing changes at dispatch time — **one tag still means one fixed,
fully literal argv**.

```toml
spec = 1

[capability]
tag = "myllm"

[detect]
bin = "myllm"

[invoke]
args = ["run"]              # the base tag's argv — untouched by variants
prompt = "stdin"

[output]
format = "text"

# registers the tag "myllm_large-v2_high" with its OWN full argv.
[[variants]]
suffix = "large-v2_high"
args = ["run", "--model", "large-v2", "--effort", "high"]
```

Each entry:

| Field | Type | Required | Rules |
|---|---|---|---|
| `suffix` | string | yes | `<model>_<effort>`, each side `[a-z0-9.-]+` (so exactly one `_`) |
| `args` | string array | yes | the variant's **full** argv — verbatim, complete, never merged with or derived from the parent's args |
| `resume_args` | string array | no | FULL replacement for the inherited `[session]` resume argv (`{session_id}` slot required; parent must declare `[session]`) |

A variant **inherits** `bin`, `env`, `prompt`, `timeout_secs`, `output`,
`[workspace]`, and `[session]` (and `description`) from the parent spec;
`args` is its own, whole, and literal. There is no field merging and no
placeholder substitution — the "argv is literal" invariant holds per tag.
The one nuance is subcommand-style resuming: an inherited `resume_args`
replacement cannot carry a per-variant model flag, so a variant may declare
its own `resume_args` (the embedded codex family does; append-style families
like claude never need to — the appended flags ride each variant's own args).

**The tag grammar.** A composed tag is `{provider}_{model}_{effort}` and
splits into **exactly three segments on `_`** — the contract the desktop
app's provider/model/effort picker decomposes tags by (e.g.
`codex_gpt-5.5_xhigh`, `claude_opus_max`). The loader enforces it fail-loud:

- the parent tag must be underscore-free (a `my_llm` parent cannot declare
  variants — write separate spec files instead);
- the suffix must be `<model>_<effort>` with both sides non-empty
  `[a-z0-9.-]+`;
- the composed tag must pass the shared consensus tag rule (≤ 64 bytes);
- duplicate suffixes in one file — and composed tags colliding with any other
  tag in the operator dir — are hard errors, like every other duplicate tag;
- unknown fields inside a `[[variants]]` entry are rejected, like everywhere
  else in the format.

A tag that does not follow the grammar is still a perfectly good tag — the
app just treats it as opaque (selectable as-is, no cascading picker).

**Override semantics are unchanged**: a variant tag is its own tag, and
operator specs override **by tag, wholesale**. Overriding a built-in base tag
(say `codex`) does NOT touch its built-in variant tags (`codex_*_*` remain);
override those individually if you want them retuned, or shadow them with an
operator file declaring its own `[[variants]]`.

**This is NOT the removed model routing.** The retired `[models]` table and
`{model}` argv placeholder chose flags at *dispatch time*; `[[variants]]`
expands *once at load* into ordinary specs, each with a fixed verbatim argv.
There is still no routing table, no pattern matching, and no substitution
anywhere in the invoke path.

The embedded built-ins use this to ship a curated model/effort matrix:
`codex` (base) plus
`codex_{gpt-5.6-sol,gpt-5.6-terra,gpt-5.6-luna}_{low,medium,high,xhigh,max}`
and `codex_gpt-5.5_{low,medium,high,xhigh}` (the effort set follows what each
model actually supports, so the codex side is not a rectangle), and `claude`
(base) plus `claude_{fable,opus,sonnet,haiku}_{low,medium,high,max}`.

---

## Worked example: wiring a brand-new executor

Suppose you run [ollama](https://ollama.com) locally and want agents to use
it. No Ducktape code changes — one file:

`~/.ducktape/capabilities/ollama.toml`

```toml
spec = 1

[capability]
tag = "ollama"
description = "local ollama daemon via its CLI"

[detect]
bin = "ollama"

[invoke]
# `ollama run <model>` reads the prompt from stdin and prints the answer.
# the model is a literal arg — host policy, invisible to consensus.
args = ["run", "llama4"]
prompt = "stdin"
timeout_secs = 600          # local models can be slow to first token

[output]
format = "text"             # plain stdout IS the answer
```

Restart the node. If `ollama` is executable on `PATH`:

- discovery builds an `ollama` provider;
- the node **announces** `ollama` to the capability registry (network-wide,
  member-gated, idempotent);
- an agent registered with `capability = "ollama"` dispatches here: its runs
  are leased over the nodes announcing `ollama`, and the assigned host
  executes with exactly the argv above;
- `capability-host` unit tests exercise exactly this path with a fake CLI
  (`an_operator_spec_discovers_a_custom_executor`).

If the binary is missing, the capability is simply not announced — the spec
sitting in the directory is inert, not an error. Want the same daemon under
two tunings? Two files, two tags (`ollama`, `ollama-large`), each with its
own literal args — or one file with [`[[variants]]`](#variants--one-file-a-family-of-finer-tags)
if the tunings follow the `provider_model_effort` grammar.

---

## Environment knobs

| Variable | Effect |
|---|---|
| `DUCKTAPE_CAPABILITY_DIR` | operator spec directory (explicit; missing dir = boot error) |
| *(per spec)* `[detect].env` | each spec may name its own explicit-binary override var — see the embedded specs for theirs |
| `DUCKTAPE_PROVIDER_TIMEOUT_SECS` | overrides **every** spec's `timeout_secs` at once |
| `DUCKTAPE_AGENT_WORKSPACES` | overrides the persistent-workspaces root (default `<data>/agent-workspaces`) |
| `DUCKTAPE_AGENT_SESSIONS` | overrides the session-store root (default `<data>/agent-sessions`) |

---

## Versioning

`spec = 1` is the only version this build reads. Format changes that would
alter the meaning of existing files bump the version; the parser rejects
versions it does not understand rather than guessing. New optional fields
within v1 are **not** added silently — unknown fields are errors, so any field
addition is itself a version bump. Yes, that is strict; strict is the point:
a spec means one thing, on every build that accepts it.

(The retired `[models]` routing table and `{model}` argv placeholder were
removed within v1 as a pre-release flag day: files that still carry them fail
loudly at boot with an unknown-field error, never a silent behavior change.
`[[variants]]` was likewise added within v1 pre-release — a build older than
it rejects a file carrying variants loudly as an unknown field, never
misreading it as a single-tag spec. `[workspace]`, `[session]` and `[tools]`
follow the same pre-release precedent: an older build rejects a file carrying
them as unknown fields rather than silently running scratch-and-cold, or
tool-less.)
