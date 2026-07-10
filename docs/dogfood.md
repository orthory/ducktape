# Dogfood ceremony: ducktape develops ducktape

The operator's runbook for the agent dogfooding loop: host this repo in its
own forge, register an agent over it, open an issue that references a Pages
spec, mention the agent, and review the resulting PR, block-anchored spec
commentary, and usage — all in the app.

Background: the design spec
([agent dogfooding loop](superpowers/specs/2026-07-10-agent-dogfooding-loop-design.md))
and the runtime contract
([deterministic agent runtime ADR](adr/2026-07-09-deterministic-agent-runtime.mdx)).

## 0. Prereqs

- **A running node.** The dev app (`make dev`) runs a single-node workspace
  where your node is the validator — the simplest setup, everything below
  targets it. A multi-node localnet/fleet works too; where it matters
  ("validator" vs "resident") the steps say so.
- **Host `git` on `PATH`, with worktree support.** The provisioner probes
  once at construction (`git init` + `git worktree list` in a scratch dir,
  `bin/noded/src/agent_provision/forge.rs`); a failed probe makes the forge
  lane permanently unavailable, loudly. Forge repos are git-default **sha1**
  — do NOT set `init.defaultObjectFormat = sha256` host-wide; a sha256 clone
  cannot interop with the node's repos.
- **A capability provider** announced by the executing node: a spec in
  `~/.ducktape/capabilities/` whose tag the agent will "run on" (the register
  form's *Runs on* picker lists announced tags).
- **Raise the provider timeout for cold Rust builds.**
  `DUCKTAPE_PROVIDER_TIMEOUT_SECS=<secs>` on the node process overrides every
  spec's *idle* timeout at once (built-in agentic specs default to 600 s).
  The hard wall-clock cap is always **idle × 6** (`HARD_TIMEOUT_FACTOR`,
  `crates/kernel/capability-host/src/lib.rs`) — a first `cargo build` in a
  fresh worktree can exceed the default ceiling, and the child is killed at
  the cap even while producing output.

## 1. Push the repo: `make dogfood-forge`

```sh
make dogfood-forge
```

`ops/dogfood-forge.sh` resolves the node's HTTP base
(`DUCKTAPE_DEV_FORGE_URL` → the active workspace's `http_listen` from
`~/.ducktape/registry.json` + `node.toml` → `http://127.0.0.1:8844`),
registers a normal git remote `ducktape-dev` at `<base>/forge/ducktape`, and
pushes `HEAD` to `refs/heads/main`. Repo creation *is* the first push — no
separate create step. The whole packfile travels over git smart-HTTP and is
stored node-locally; only a tiny `forge Push` (digest + oids) crosses
consensus.

Knobs: `FORGE_REPO` (default `ducktape`), `FORGE_REMOTE` (default
`ducktape-dev`), `SRC_REF` (default `HEAD` — deliberately not `main`, which
lags `dev` in this repo), `DUCKTAPE_DEV_FORGE_URL`.

Verify: the `ducktape` repo appears in the desktop **Forge** view with `main`
browsable. Update later with `git push ducktape-dev main` (i.e. re-run the
target). Caveat: the remote lives in the shared `.git/config`, visible to
every worktree of this repo — set `FORGE_REMOTE` per worktree if you run
several nodes at once.

## 2. Register the dogfood agent

Two surfaces, because the register form is not yet complete:

- **In-app** (Agents view → register): display name, *Runs on* capability,
  system prompt, agent ID, action grants (chips), and a *Page write access*
  field (`pages_write`). It has **no forge caps fields** — an agent
  registered here cannot read or push the repo.
- **API** (`POST /v1/submit` on the node's HTTP surface): the full
  registration, matching what the e2e does. Use this for the dogfood agent.

**Seed the prompt on the executing node** (the #298 caveat): runs resolve the
registered `prompt_hash` pin from the executing node's blob store — the
strict no-fallback prompt path. Upload the prompt to a **validator** node
(the one whose provider will execute); on the single-node dev workspace
that's just your node.

```sh
# 1. upload the prompt text as a blob; the receipt's digest IS the hash to pin
curl -s <base>/v1/files/blob -X POST -H 'content-type: application/json' \
  -d '"You are the dogfooding duck. Work the referenced spec."'
# → {"digest":"<64-hex sha256>", ...}

# 2. register, granting the full dogfood surface in ONE submit
curl -s <base>/v1/submit -X POST -H 'content-type: application/json' -d '{
  "target": "agent",
  "payload": { "register_agent": {
    "agent_id": "dogfood",
    "display_name": "Dogfood Duck",
    "capability": "<your provider tag>",
    "prompt_hash": [ ...32 bytes, the digest hex decoded... ],
    "allowed_actions": ["chat.post", "tasks.create", "tasks.update_status",
                        "pages.comment", "pages.set_checked"],
    "caps": {
      "forge_read":  ["ducktape"],
      "forge_push":  ["ducktape"],
      "pages_write": ["*"]
    }
  } }
}'
```

Notes on the shape (don't improvise past these):

- `payload` is the agent module's `AgentMsg` enum as JSON (snake_case
  externally tagged); `prompt_hash` is a JSON **byte array**, not a hex
  string. The canonical, always-current example of this exact registration —
  prompt upload, caps naming the repo literally, wire shapes — is
  `bin/node/tests/dogfood_loop_e2e.rs`.
- Grant caps **at registration**. `UpdateAgent` is owner-gated to the
  registering origin, so a form-registered agent (app origin) can't be
  cap-patched later from a `curl` with a different origin.
- Cap semantics: `forge_read`/`forge_push` name repos exactly (push implies
  read); `pages_write` is page-id-scoped with `"*"` meaning every page — no
  prefix matching.

Verify: the agent shows in the Agents view with its grants and caps.

## 3. Write the spec in Pages

Create a page in the **Pages** view. Use **to-do blocks** for the task items
— `pages.set_checked` only applies to todo-kind blocks, so a checklist gives
the agent something to tick as it lands work.

Note the **page id**: the app mints a UUID per page (the root block id). It
isn't shown in the editor chrome; recover it from the page enumeration:

```sh
curl -s <base>/v1/query -X POST -H 'content-type: application/json' \
  -d '{"target":"pages","query":"list_pages"}'
# → ids + titles; pick your spec's id
```

## 4. Open the issue with a `[[page:<id>]]` ref

Forge view → the `ducktape` repo → **Issues** → *New issue*. Put
`[[page:<your-page-id>]]` in the body.

At run compose, every ref found in the trigger message or the injected issue
body resolves against committed pages state and the page's whole subtree is
rendered into the run's context: headings, `- [ ]`/`- [x]` todos with an
inline `[blk:<id>]` per block (the ids the agent targets with its pages
actions), fenced code, 64 KiB budget with a truncation marker. An
unresolvable ref becomes a one-line "not found" marker — never a failed run.
One limit: refs sitting past the 16 KiB item-body truncation point aren't
seen.

## 5. Mention the agent — and iterate in the PR

In the issue's **Discussion** tab, `@`-mention the agent (the composer's
typeahead resolves it; the first mention in an unwatched channel creates the
runs watch automatically). The run then:

1. provisions a **detached** worktree at the pinned base commit (never
   holding the branch),
2. works, commits, and pushes `agent/item-<n>`,
3. opens a PR titled from its reply, and replies in the issue thread.

**The PR is the session.** Re-mention the agent in the PR's own Discussion
(hidden channel `forge:ducktape:<n>`) to iterate — PR runs use the PR's
source branch verbatim, so follow-up commits land on the same branch.

While it works, the agent can `pages.comment` on spec blocks (comments land
agent-authored on the page) and `pages.set_checked` the todo items — the
block-anchored commentary layer. A pages action that fails its gate (cap
deny, bad target, squatted id) degrades **individually** with a breadcrumb in
the run's channel; the run still delivers. If a comment didn't land, read the
breadcrumbs before suspecting the model.

## 6. Review in-app

- **Forge view**: the PR (files/diff, merge box), the `agent/item-<n>`
  branch, and the discussion trail.
- **The spec page**: agent-authored comment threads anchored to the blocks
  they discuss; ticked todos (unattributed — checking stores no author).
- **Agents → Activity**: the delivered-runs history (last-100 ring) with
  per-run duration **in blocks** and a `PR #<n>` chip linking run to
  artifact; above it the **UsageCard** — whose subscription carried how much,
  grouped per account (executor nodes resolved to their bound accounts), with
  per-capability breakdown. Header says *all time · durations in blocks*: the
  ledger counts consensus heights, not seconds, and there is deliberately no
  wall-clock week.

## Known limits (set expectations honestly)

- **Concurrent branch advance is absorbed, not lost**: on a push reject the
  provisioner fetch+rebases+re-pushes (bounded 3, `rebased: true` in the
  receipt). Only a genuine rebase **conflict** degrades the run.
- A losing re-leased attempt's push can leave branch residue in a narrow
  race — harmless, but you may see an unexpected commit on the work branch.
- Forge-lane runs don't yet materialize W6 skill ro-mounts — the agent works
  from the bare worktree plus its prompt.
- **Usage starts at the indexer's deploy boundary**: the ledger doesn't
  rebuild history, so runs before the indexer first ran are absent. "All
  time" means "since this deploy".
- The register form's caps UI is partial (`pages_write` only); forge caps go
  through the API path above.
- The agent's push shares the 60-second commit bracket — a cold, enormous
  diff could exceed it. Raise expectations, not the bracket.
