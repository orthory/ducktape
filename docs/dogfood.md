# Dogfood ceremony: ducktape develops ducktape

The operator's runbook for the agent dogfooding loop: host this repo in its
own forge, register an agent over it, open an issue that references a Pages
spec, mention the agent, and review the resulting PR, block-anchored spec
commentary, and usage — all in the app.

## 0. Prereqs

- **A running node.** The dev app (`make dev`) runs a single-node workspace
  where your node is the validator — the simplest setup, everything below
  targets it. A multi-node network works too; where it matters ("validator"
  vs "resident") the steps say so. By default every TCP listener binds
  `127.0.0.1`; `DEV_LISTEN=0.0.0.0 DEV_ADVERTISED=<this box's LAN ip>`
  widens the p2p mesh and HTTP API binds AND the dial hint peers actually
  use, so a second machine can join, huddle in, or point its app at this
  node (the WireGuard plane is bound wide regardless).
- **Host `git` on `PATH`, with worktree support.** The provisioner probes
  once at construction (`git init` + `git worktree list` in a scratch dir,
  `crates/noded/src/agent_provision/forge.rs`); a failed probe makes the forge
  lane permanently unavailable, loudly. Forge repos are git-default **sha1**
  — do NOT set `init.defaultObjectFormat = sha256` host-wide; a sha256 clone
  cannot interop with the node's repos.
- **A capability provider** announced by the executing node: a built-in spec
  whose binary is on the node's `PATH`, or an operator spec in
  `<ducktape home>/capabilities/` (`docs/records/specs/capability-spec.md`).
  The register form's *Runs on* picker lists announced tags.
- **Raise the provider timeout for cold Rust builds.**
  `DUCKTAPE_PROVIDER_TIMEOUT_SECS=<secs>` on the node process overrides every
  spec's *idle* timeout at once. The hard wall-clock cap is always
  **idle × `HARD_TIMEOUT_FACTOR`** (36, `crates/services/provider/src/lib.rs`)
  and the child is killed at the cap even while producing output, so budget
  for a first `cargo build` in a fresh worktree.

## 1. Push the repo: `make dogfood-forge`

```sh
make dogfood-forge
```

`ops/dogfood-forge.sh` resolves the node's HTTP base
(`DUCKTAPE_DEV_FORGE_URL` → the active workspace's `http_listen` from
`<ducktape home>/registry.json` + `node.toml` → `http://127.0.0.1:8844`;
the home is `$DUCKTAPE_HOME` when set, else `~/.ducktape`),
registers a normal git remote `ducktape-dev` at `<base>/forge/ducktape`,
fetches `origin/dev`, pushes that exact commit to the forge's `refs/heads/dev`
(release-only `main` is never moved), then reads the Forge ref back and
requires exact OID equality. Repo creation *is* the first push — no separate
create step. The whole packfile travels over git smart-HTTP and is stored
node-locally; only a tiny `forge Push` (digest + oids) crosses consensus. Run
this before creating or invoking agent work so a clean but stale local
checkout cannot silently pin the run to obsolete source.

Knobs: `FORGE_REPO` (default `ducktape`), `FORGE_REMOTE` (default
`ducktape-dev`), `SOURCE_REMOTE` (default `origin`), `SOURCE_BRANCH` (default
`dev`), `SRC_REF` (explicit local-ref override), `DUCKTAPE_DEV_FORGE_URL`.

Verify: the `ducktape` repo appears in the desktop **Forge** view with `dev`
browsable. Re-run `make dogfood-forge` before later agent work; a raw
`git push ducktape-dev dev` bypasses the fetch and equality gate. Caveat: the
remote lives in the shared `.git/config`, visible to every worktree of this
repo — set `FORGE_REMOTE` per worktree if you run several nodes at once.

**A push must prove itself.** `git-receive-pack` takes exactly two proofs:
git's own push certificate (`git push --signed`, whose signer becomes the
repo's owner on chain), or the node's operator credential, which makes the
NODE the owner. `make dogfood-forge` presents the second — it is seeding the
node's own mirror — and a bare `git push` at a node whose `admin.token` you
cannot read is refused. To push by hand:

```sh
export GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=http.extraHeader
export GIT_CONFIG_VALUE_0="x-ducktape-admin-token: $(cat <workspace>/admin.token)"
git push ducktape-dev dev
```

`GIT_CONFIG_*` rather than `git -c`: an argv is world-readable through
`/proc`, and this is a secret.

## 2. Register the dogfood agent

An agent's persona is an **`always` skill**: a `SKILL.md` in the shared skill
library (the files module, under `/shared/skills/<name>/`) whose body the host
inlines into every run's context document. There is no prompt blob and no
prompt hash. The in-app register form (Agents view → register) covers the
common case; the API registration below is the complete one and is the shape
`bin/node/tests/dogfood_loop_e2e.rs` and
`bin/node/tests/portable_workspace_e2e.rs` use.

Both calls MUTATE, so they carry a credential. `/v1` takes either a
per-request user signature or this node's own operator credential — the
secret it mints 0600 into its workspace at every boot — and a curl can only
present the second. `<workspace>` is the directory holding `node.toml` (on
the dev shape, the storage dir).

```sh
OPERATOR="$(cat <workspace>/admin.token)"

# 1. the persona: one skill document in the shared library (PUT is a
#    single-change duckfs commit)
curl -s -X PUT "<base>/v1/files/object/shared/skills/dogfood/SKILL.md" \
  -H "x-ducktape-admin-token: $OPERATOR" \
  --data-binary 'You are the dogfooding duck. Work the referenced spec.'

# 2. register, granting the full dogfood surface in ONE submit
curl -s <base>/v1/submit -X POST -H 'content-type: application/json' \
  -H "x-ducktape-admin-token: $OPERATOR" -d '{
  "target": "agent",
  "payload": { "register_agent": {
    "agent_id": "dogfood",
    "display_name": "Dogfood Duck",
    "capability": "<your provider tag>",
    "allowed_actions": ["chat.post", "tasks.create", "tasks.update_status",
                        "pages.comment", "pages.set_checked"],
    "caps": {
      "forge_read":  ["ducktape"],
      "forge_push":  ["ducktape"],
      "pages_write": ["*"],
      "duckfs_read": ["/shared/skills"]
    },
    "skills": [ { "name": "dogfood",
                  "source_prefix": "/shared/skills/dogfood",
                  "load": "always" } ]
  } }
}'
```

Notes on the shape (don't improvise past these):

- `payload` is the agent module's `AgentMsg` enum as JSON (snake_case
  externally tagged); the field set is
  `crates/modules/apps/agent/src/interface.rs`.
- Grant caps **at registration**. `UpdateAgent` is owner-gated to the
  registering origin, so a form-registered agent (app origin) can't be
  cap-patched later from a `curl` with a different origin.
- Cap semantics: `forge_read`/`forge_push` name repos exactly (push implies
  read); `pages_write` is page-id-scoped with `"*"` meaning every page — no
  prefix matching; `duckfs_read` over `/shared/skills` is the library grant
  every run reads its mounted skills through.
- A skill ref without `source_snapshot` TRACKS the library's committed head;
  pin it to a snapshot id to freeze the persona.

Verify: the agent shows in the Agents view with its grants, caps and skills.

## 3. Write the spec in Pages

Create a page in the **Pages** view. Use **to-do blocks** for the task items
— `pages.set_checked` only applies to todo-kind blocks, so a checklist gives
the agent something to tick as it lands work.

Note the **page id**: the app mints a UUID per page (the root block id). It
isn't shown in the editor chrome; recover it from the pages view (the index
tier, `docs/records/specs/indexable-spec.md`):

```sh
curl -s <base>/v1/index/pages/view -X POST -H 'content-type: application/json' \
  -d '{"list_pages":{"limit":50}}'
# → ids + titles, cursor-paged; pick your spec's id.
```

## 4. Open the issue with a `[[page:<id>]]` ref

Forge view → the `ducktape` repo → **Issues** → *New issue*. Put
`[[page:<your-page-id>]]` in the body.

At run compose, every ref found in the trigger message or the injected issue
body resolves against committed pages state and the page's whole subtree is
rendered into the run's context: headings, `- [ ]`/`- [x]` todos with an
inline `[blk:<id>]` per block (the ids the agent targets with its pages
actions), fenced code, a 64 KiB budget with a truncation marker
(`PAGE_CONTEXT_BYTES`, `crates/modules/apps/runs/src/inject.rs`). Page and
attachment resolution share the dispatch's bounded reference-read budget. An
unresolvable ref becomes a one-line "not found" marker — never a failed run.
One limit: refs sitting past the 16 KiB item-body truncation point
(`MAX_CONTEXT_BYTES`) aren't seen.

## 5. Mention the agent — and iterate in the PR

In the issue's **Discussion** tab, `@`-mention the agent (the composer's
typeahead resolves it; the first mention in an unwatched channel creates the
runs watch automatically). The run then:

1. provisions a **detached** worktree at the pinned base commit (never
   holding the branch), with its skills mounted read-only beside it,
2. works, commits, and pushes `agent/item-<n>`,
3. opens a PR titled from its reply, and replies in the issue thread.

**The PR is the session.** Re-mention the agent in the PR's own Discussion
(hidden channel `forge:ducktape:<n>`) to iterate — PR runs use the PR's
source branch verbatim, so follow-up commits land on the same branch.

While it works, the agent can `pages.comment` on spec blocks (comments land
agent-authored on the page) and `pages.set_checked` the todo items — the
block-anchored commentary layer. A pages action that fails its gate (cap
deny, bad target, squatted id) degrades **individually** — that one action is
dropped and the run still delivers its reply and other effects, with the
drop reason recorded as a `runs` breadcrumb on the run. If a comment didn't
land, check the agent's caps (`pages_write` on the page) and that the target
block/page id is correct.

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

## Known limits

- **No page read-authorization:** `[[page:<id>]]` injection renders any
  referenced page's subtree into the run context with no read-cap gate
  (pages are workspace-visible to members). A member can surface any page
  they can already see.
- **Page depth is bounded:** one document allows 64 parent edges
  (`MAX_PAGE_DEPTH`). A nested Page block is a leaf in its containing
  document and starts a separate 64-edge document.
- **Concurrent branch advance is absorbed, not lost**: on a push reject the
  provisioner fetches, rebases and re-pushes a bounded number of times
  (`rebased: true` in the receipt). Only a genuine rebase **conflict**
  degrades the run.
- A losing re-leased attempt's push can leave branch residue in a narrow
  race — harmless, but you may see an unexpected commit on the work branch.
- **Usage starts at the indexer's deploy boundary**: the ledger doesn't
  rebuild history, so runs before the indexer first ran are absent. "All
  time" means "since this deploy".
