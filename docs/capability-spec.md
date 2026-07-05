# Capability Spec Reference (v1)

A **capability spec** is a TOML file that teaches a Ducktape node how to run
one executor — an installed CLI like `codex`, `claude`, or anything else that
can turn a prompt into text. Everything the node needs is in the file: how to
detect the binary, the exact argv to invoke it, how to parse its output, and
which model refs route to it. **Adding an executor is a config change, never a
code change.**

Specs are the data half of the capability system:

| Layer | Owns | Code |
|---|---|---|
| Consensus (`crates/system/capability`) | *who provides what*, network-wide: node key → announced tag set | never reads specs |
| Host (`crates/kernel/capability-host`) | *actually running it*: spec loading, binary discovery, spawning, parsing | this document |

The consensus registry only ever sees **tags** (`"codex"`). The spec behind a
tag is private to each host — two nodes can announce `codex` with differently
tuned specs and the network cannot tell, by design.

---

## Trust model — read this first

A spec names an arbitrary local binary and the argv to run it with. **Loading
a spec is executing code by proxy.** Specs are operator-trusted configuration,
in the same trust class as a shell profile or a systemd unit:

- They load from exactly two places, both local and operator-controlled:
  1. the specs **embedded in the node binary** at compile time
     (`crates/kernel/capability-host/specs/*.toml`);
  2. the **operator spec directory** (see [Spec sources](#spec-sources)).
- They are **never fetched from the network**, and no consensus code path can
  read one (host-local files are non-deterministic input).
- Prompts and model refs are substituted into argv **verbatim, without shell
  interpretation** — there is no quoting or expansion, so job content cannot
  inject flags or commands. The only templated value is `{model}`.
- The child process runs **fenced**: an empty scratch working directory (never
  the node's data dir), one-shot non-interactive mode, and whatever sandbox
  flags the spec's argv encodes. Fence flags live in the spec — audit them
  when you audit the spec.

**BYO auth is the point.** The node never reads, writes, or refreshes any
credential file. If the executor needs a login (`codex login`,
`claude setup-token`, an API key in *its* config), that is between the
operator and the executor.

---

## Spec sources

Specs load in two passes:

1. **Embedded built-ins** — `codex.toml` and `claude.toml`, compiled into the
   node. These parse through the exact same code path as operator files and
   serve as the reference examples.
2. **Operator directory** — every `*.toml` in:
   - `$DUCKTAPE_CAPABILITY_DIR` if set. Pointing this at a missing directory
     is a **hard error** (you asked for a dir that isn't there);
   - otherwise `~/.ducktape/capabilities`, only if it exists (absent default
     simply means "no operator specs").

**Override rule:** an operator spec whose `tag` matches a built-in **replaces
it wholesale** — there is no field-level merging; the spec file is the unit of
override. This is the supported way to retune a built-in (different sandbox
flags, a different timeout, extra model patterns): copy the embedded spec,
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
tag = "codex"
# human-facing one-liner for docs and status surfaces. optional.
description = "OpenAI Codex CLI, one-shot exec mode"

[detect]
# the binary name probed on PATH (first executable match wins).
bin = "codex"
# optional env var naming an EXPLICIT binary path. when set, it wins over the
# PATH probe — and if it points at something that is not an executable file,
# the capability is dropped with a loud warning. an explicit override never
# silently falls back to PATH: you said "use this", and this doesn't exist.
env = "DUCKTAPE_CODEX_BIN"

[invoke]
# argv template, passed to exec() verbatim — never through a shell. the ONLY
# substitution is "{model}", replaced with the job's resolved model ref.
# encode your fence here: sandbox flags, non-interactive mode, turn limits.
args = [
    "exec",
    "--json",
    "--sandbox", "read-only",
    "--skip-git-repo-check",
    "--model", "{model}",
    "-",
]
# how the prompt reaches the child. v1 supports only "stdin": the prompt is
# written to the child's stdin (concurrently with output collection, so huge
# prompts can't deadlock the pipe) and then EOF. an argv placeholder would
# leak prompts into `ps` output and hit ARG_MAX — deliberately unsupported.
prompt = "stdin"
# per-job wall-clock budget in seconds (1..=3600, default 300). the child is
# KILLED at the deadline. $DUCKTAPE_PROVIDER_TIMEOUT_SECS overrides every
# spec's timeout at once (ops knob for slow hosts).
timeout_secs = 300

[output]
# which NAMED parser extracts the assistant's final text from stdout:
#   "codex-jsonl"  — codex exec --json event stream; the LAST agent_message
#                    wins; tolerates both item shapes the CLI has shipped and
#                    skips non-JSON noise lines.
#   "claude-json"  — claude -p --output-format json; the single
#                    {"type":"result",...} object; an is_error result is
#                    surfaced as the error it is.
#   "text"         — raw stdout, trimmed. THE GENERIC ESCAPE HATCH: any CLI
#                    that prints the answer plainly works with zero code.
#                    empty output on a zero exit is an error ("ran fine,
#                    said nothing" is a broken executor, not an answer).
# this is a CLOSED set on purpose: each name is a tested parser for a real
# output contract. a new name is a code change with tests — that's the point.
format = "codex-jsonl"

[models]
# *-glob patterns over model refs this capability serves. `*` matches any run
# of characters (including none); everything else matches itself. no `?`, no
# character classes — restraint keeps routing specificity obvious.
patterns = ["gpt-*", "*codex*", "*"]
# the model used when an UNPINNED request routes here. optional — a spec
# without one refuses unpinned requests with a clear error.
default = "gpt-5.3-codex-spark"
```

---

## Field reference

### Top level

| Field | Type | Required | Rules |
|---|---|---|---|
| `spec` | integer | yes | must be `1` |

Unknown fields **anywhere** in the file are rejected (a typo like `patern`
fails loud instead of silently changing routing).

### `[capability]`

| Field | Type | Required | Rules |
|---|---|---|---|
| `tag` | string | yes | 1..=64 bytes, `[a-z0-9._-]` only — mirrors the consensus registry exactly |
| `description` | string | no | free text |

### `[detect]`

| Field | Type | Required | Rules |
|---|---|---|---|
| `bin` | string | yes | non-empty; probed on `PATH` |
| `env` | string | no | env var naming an explicit binary path; override wins, broken override = warn + absent |

### `[invoke]`

| Field | Type | Required | Rules |
|---|---|---|---|
| `args` | string array | no (default `[]`) | passed verbatim to exec; `{model}` substituted |
| `prompt` | string | yes | must be `"stdin"` in v1 |
| `timeout_secs` | integer | no (default `300`) | 1..=3600; child killed at deadline |

### `[output]`

| Field | Type | Required | Rules |
|---|---|---|---|
| `format` | string | yes | `"codex-jsonl"` \| `"claude-json"` \| `"text"` |

### `[models]`

| Field | Type | Required | Rules |
|---|---|---|---|
| `patterns` | string array | yes | at least one non-empty `*`-glob |
| `default` | string | no | non-empty when set; used for unpinned requests routed here |

---

## Routing: model ref → capability

One model ref routes to exactly **one** spec, deterministically:

1. Every loaded spec's patterns are tried against the (trimmed) model ref.
2. The matching pattern with the **most literal (non-`*`) characters** wins —
   `claude*` (6 literals) beats the `*` catch-all (0) for `claude-sonnet-5`.
3. Score ties break to the **lexicographically smaller tag**, so routing never
   depends on file order or discovery order.

Two consequences worth internalizing:

- **Unpinned requests** (empty model ref) match only a `*` catch-all pattern.
  The catch-all spec's `[models].default` supplies the model. In the built-in
  set, codex carries the catch-all — an unpinned request runs
  `gpt-5.3-codex-spark` on codex, and an unknown model ref fails loudly on a
  codex-less node instead of silently picking a different CLI.
- **Routing is over ALL loaded specs, installed or not.** `claude-sonnet-5` on
  a node without the claude CLI errors with
  `capability 'claude' … is not provided by this node; this node provides
  ["codex"]` — the capability is named, not shrugged at as an unknown model.

The full resolution pipeline (`ProviderSet::resolve`) is: route the ref to a
spec → resolve the effective model (pinned ref, else the spec's default) →
look up the local provider. Each failure is a distinct error: no spec matches,
no default model to fall back to, or capability not installed here.

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
args = ["run", "{model}"]
prompt = "stdin"
timeout_secs = 600          # local models can be slow to first token

[output]
format = "text"             # plain stdout IS the answer

[models]
patterns = ["llama*", "qwen*", "mistral*"]
default = "llama4"
```

Restart the node. If `ollama` is executable on `PATH`:

- discovery builds an `ollama` provider;
- the node **announces** `ollama` to the capability registry (network-wide,
  member-gated, idempotent);
- an agent with `model_ref = "llama4-scout"` routes here (`llama*`, 5
  literals) and runs locally;
- `capability-host` unit tests exercise exactly this path with a fake CLI
  (`an_operator_spec_discovers_a_custom_executor`).

If the binary is missing, the capability is simply not announced — the spec
sitting in the directory is inert, not an error.

---

## Environment knobs

| Variable | Effect |
|---|---|
| `DUCKTAPE_CAPABILITY_DIR` | operator spec directory (explicit; missing dir = boot error) |
| `DUCKTAPE_CODEX_BIN` / `DUCKTAPE_CLAUDE_BIN` | explicit binary paths for the built-in specs (each spec names its own var in `[detect].env`) |
| `DUCKTAPE_PROVIDER_TIMEOUT_SECS` | overrides **every** spec's `timeout_secs` at once |

---

## Versioning

`spec = 1` is the only version this build reads. Format changes that would
alter the meaning of existing files bump the version; the parser rejects
versions it does not understand rather than guessing. New optional fields
within v1 are **not** added silently — unknown fields are errors, so any field
addition is itself a version bump. Yes, that is strict; strict is the point:
a spec means one thing, on every build that accepts it.
