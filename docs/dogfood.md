# Dogfood ceremony: ducktape develops ducktape

The operator's runbook for the agent dogfooding loop: host this repo in its
own forge, provision a model user over it, open an issue that references a Pages
spec, mention the agent, and review the resulting PR and block-anchored spec
commentary in the app, with run outcomes available through the query API.

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
- **An enabled compute service with a working microVM sandbox.** Install
  guest-architecture executors with `ducktape agent install claude` or
  `ducktape agent install codex`, including the spec's declared companions.
  Discovery checks the sandbox's executor directory. Custom specs live under
  `DUCKTAPE_CAPABILITY_DIR` or `<ducktape home>/capabilities/`
  (`docs/records/specs/capability-spec.md`).
- **Provider authentication on the executing service.** Codex uses
  `OPENAI_API_KEY` or `CODEX_HOME/auth.json`; Claude uses `ANTHROPIC_API_KEY`,
  `CLAUDE_CODE_OAUTH_TOKEN`, or `~/.claude/.credentials.json`. Installing an
  executable does not configure its credentials.
- **Raise the provider timeout for cold Rust builds.**
  `DUCKTAPE_PROVIDER_TIMEOUT_SECS=<secs>` on the executing compute service
  overrides the spec's *idle* timeout. The hard wall-clock cap is
  **idle × `[invoke].hard_timeout_factor`** (default 36)
  and the child is killed at the cap even while producing output, so budget
  for a first `cargo build` in a fresh worktree.

## 1. Push the repo: `make dogfood-forge`

```sh
make dogfood-forge
```

`ops/dogfood-forge.sh` resolves the node's HTTP base
from `DUCKTAPE_DEV_FORGE_URL` or the active workspace's `http_listen` in
`<ducktape home>/registry.json` and `node.toml`, failing if neither resolves
(the home is `$DUCKTAPE_HOME` when set, else `~/.ducktape`),
registers a normal git remote `ducktape-dev` at `<base>/forge/ducktape`,
fetches `origin/dev`, and reconciles it with the forge's `refs/heads/dev`.
It fast-forwards when possible, retains a Forge descendant, or joins equal-tree
divergence with a two-parent bridge; differing-tree divergence fails. It reads
the Forge ref back and verifies the selected tip. Repo creation is the first push — no separate
create step. The whole packfile travels over git smart-HTTP and is stored
node-locally; only a tiny `forge Push` (digest + oids) crosses consensus. Run
this before creating or invoking agent work so a clean but stale local
checkout cannot silently pin the run to obsolete source.

Knobs: `FORGE_REPO` (default `ducktape`), `FORGE_REMOTE` (default
`ducktape-dev`), `SOURCE_REMOTE` (default `origin`), `SOURCE_BRANCH` (default
`dev`), `SRC_REF` (explicit local-ref override), `DUCKTAPE_DEV_FORGE_URL`.

Verify: the `ducktape` repo appears in the desktop **Forge** view with `dev`
browsable. Re-run `make dogfood-forge` before later agent work; a raw
`git push ducktape-dev dev` bypasses the fetch and reconciliation checks. Caveat: the
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

## 2. Provision the dogfood model user

The persona is an **always skill**: a SKILL.md under
/shared/skills/<name>/ whose body the host includes in the run context.
The model user is a keyless identity account running a program; runs stores
its provider, action grants, resource caps and skill references.

The commands below use curl, jq, Python 3 and the current ducktape binary.
They act through the node's operator credential, so that node's identity
account controls the new model user. Use that same controller for later
configuration changes; an app wallet is a separate principal.

```sh
BASE="<base>"
WORKSPACE="<workspace>"
OPERATOR="$(cat "$WORKSPACE/admin.token")"

submit() {
  jq -nc --arg target "$1" --argjson payload "$2" '{target:$target,payload:$payload}' |
    curl -fsS "$BASE/v1/submit" -H 'content-type: application/json' \
      -H "x-ducktape-admin-token: $OPERATOR" --data-binary @-
}
query() {
  jq -nc --arg target "$1" --argjson query "$2" '{target:$target,query:$query}' |
    curl -fsS "$BASE/v1/query" -H 'content-type: application/json' --data-binary @-
}

# Resolve the actual submitting key, founding its controller account if absent.
NODE_KEY="$(ducktape node key --out "$WORKSPACE/identity.key" | tail -n 1)"
KEY_BYTES="$(python3 -c 'import json,sys; print(json.dumps(list(bytes.fromhex(sys.argv[1]))))' "$NODE_KEY")"
CONTROLLER="$(query identity "{\"of_key\":{\"key\":$KEY_BYTES}}" | jq -r '.account.number // empty')"
if [ -z "$CONTROLLER" ]; then
  submit identity '{"create":{"name":"Dogfood operator","scheme":"ed25519"}}'
  CONTROLLER="$(query identity "{\"of_key\":{\"key\":$KEY_BYTES}}" | jq -er '.account.number')"
fi

curl -fsS -X PUT "$BASE/v1/files/object/shared/skills/dogfood/SKILL.md" \
  -H "x-ducktape-admin-token: $OPERATOR" \
  --data-binary 'You are the dogfooding duck. Work the referenced spec.'

# Serialize the current default script; do not maintain a separate recipe copy.
PROGRAM="$(ducktape agent model-program dogfood)"
submit agent "$(jq -nc --argjson program "$PROGRAM" \
  '{provision:{name:"Dogfood Duck",program:$program}}')"
MODEL_ACCOUNT="$(query identity "{\"controlled\":{\"by\":$CONTROLLER,\"from\":0,\"limit\":256}}" |
  jq -er '.accounts | map(select(.name == "Dogfood Duck" and .control.program.executor == "agent")) |
    if length == 1 then .[0].number else error("choose a unique dogfood program account") end')"

submit runs "$(jq -nc --argjson account "$MODEL_ACCOUNT" '{
  configure_model:{operation:{register_model:{
    account:$account, agent_id:"dogfood", display_name:"Dogfood Duck",
    capability:"<your provider tag>",
    allowed_actions:["chat.post","chat.post_message","tasks.create","tasks.update_status",
                     "pages.comment","pages.set_checked"],
    caps:{forge_read:["ducktape"],forge_push:["ducktape"],pages_write:["*"],
          duckfs_read:["/shared/skills"]},
    skills:[{name:"dogfood",source_prefix:"/shared/skills/dogfood",load:"always"}]
  }}}
}')"

query runs '{"model":{"query":{"agent":{"agent_id":"dogfood"}}}}'
```

The final reply is model.agent; its account must equal MODEL_ACCOUNT.
IdentityQuery::Controlled is paged; use its from cursor when the controller
already owns more than 256 accounts. Names are display labels, so the
selection above fails if the controller has several Dogfood Duck accounts.

ConfigureModel wraps runs::ModelMsg from
crates/modules/apps/runs/src/model.rs. The current program account or its
live identity controller can update the record. Forge caps name exact repos;
pages_write names exact page ids, with "*" permitting all page ids at the
runs validation layer. Source modules still enforce their own ownership.
A skill without source_snapshot follows the committed library head; supply
a snapshot id to pin it. The app's Agents view lists the resulting model,
its grants and its controller.

## 3. Write the spec in Pages

Create a page in the **Pages** view. Use **to-do blocks** for the task items
— `pages.set_checked` only applies to todo-kind blocks, so a checklist gives
you a checklist to tick as the model reports completed work. A model can
change todo blocks only on pages authored by its own program account.

Note the **page id**: the app mints a UUID per page (the root block id). It
isn't shown in the editor chrome; recover it from the pages view (the index
tier, `docs/records/specs/indexable-spec.md`):

```sh
curl -s <base>/v1/index/pages/view -X POST -H 'content-type: application/json' \
  -d '{"list_pages":{"limit":50}}'
# → ids + titles, cursor-paged; pick your spec's id.
```

## 4. Open the issue with a page link

Forge view → the `ducktape` repo → **Issues** → *New issue*. Put
`[spec](duck://page/<your-page-id>)` in the body.

At run compose, every ref found in the trigger message or the injected issue
body resolves against committed pages state and the page's whole subtree is
rendered into the run's context: headings, `- [ ]`/`- [x]` todos with an
inline `[blk:<id>]` on todo blocks (the ids the agent targets with its pages
actions), fenced code, a 64 KiB budget with a truncation marker
(`PAGE_CONTEXT_BYTES`, `crates/modules/apps/runs/src/inject.rs`). Page and
attachment resolution share the dispatch's bounded reference-read budget. An
unresolvable ref becomes a one-line "not found" marker — never a failed run.
One limit: refs sitting past the 16 KiB item-body truncation point
(`MAX_CONTEXT_BYTES`) aren't seen.

## 5. Mention the agent — and iterate in the PR

In the issue's **Discussion** tab, `@`-mention the agent (the composer's
typeahead resolves its account). The message commits, attribution delivers
the mention, and its program calls runs to start model work. The run then:

1. provisions an isolated clone with detached HEAD at the pinned base commit,
   with its skills mounted read-only beside it,
2. works, commits, and pushes `agent/item-<issue-number>`,
3. proposes a PR and a reply through program calls. The PR title prefers the
   bound issue's title, using the reply's first line as a fallback. The output
   tip, source branch and open item must still match when the action applies.

Repeating the mention in the original issue's Discussion continues
`agent/item-<issue-number>` and updates its existing PR.

A mention in PR `<n>`'s Discussion (hidden channel `forge:ducktape:<n>`)
forks that PR's source branch into `agent/item-<n>` and opens a child PR
targeting the original source branch. Repeated mentions in that same
Discussion continue `agent/item-<n>` and reuse the child PR. The original
source branch advances when the proposed changes are merged.

While it works, the agent can `pages.comment` on spec blocks (comments land
authored by its program account on the page). The human page author ticks
the spec todos; `pages.set_checked` is available only for pages the program
account itself authored. Preflight skips emit debug breadcrumbs under
`ducktape::modules`; enable that log target to inspect them. Once an action
is admitted, its program call has an independent target outcome in
`ActionRequest`; a refusal cannot undo earlier successful effects. If a comment
didn't land, inspect that receipt, the model's `pages_write` capability and the
target block/page id. Replies and other proposed effects also depend on their
own call outcomes.

## 6. Review in-app

- **Forge view**: the PR (files/diff, merge box), the `agent/item-<n>`
  branch, and the discussion trail.
- **The spec page**: agent-authored comment threads anchored to the blocks
  they discuss; todos checked by their page author.

Inspect the last 100 terminal runs with the query helper from provisioning:

```sh
query runs '"recent_runs"'
```

History records include the executing node, output reference and verified PR
number. Their creation/delivery counters measure blocks, not wall-clock time.

`result_accepted` means the model result passed validation; the program decides
which proposed actions to execute. A reported program or target refusal marks
the run `action_rejected`, and its `ActionRequest` query retains the reason.
`failed` records worker failure, cancellation, or invalid output. PR links name
only an existing verified PR or a successfully committed allocation.

## Known limits

- **No page read-authorization:** `[spec](duck://page/<id>)` injection renders any
  referenced page's subtree into the run context with no read-cap gate
  (pages are workspace-visible to members). A member can surface any page
  they can already see.
- **Page depth is bounded:** one document allows 64 parent edges
  (`MAX_PAGE_DEPTH`). A nested Page block is a leaf in its containing
  document and starts a separate 64-edge document.
- **Concurrent branch advance is absorbed, not lost**: on a push reject the
  provisioner fetches, rebases and re-pushes a bounded number of times
  (`rebased: true` in the receipt). Rebase conflicts, exhausted push retries
  and other commit failures can degrade the run.
- A losing re-leased attempt's push can leave branch residue in a narrow
  race — harmless, but you may see an unexpected commit on the work branch.
- **Usage starts at the indexer's deploy boundary**: the ledger doesn't
  rebuild history, so runs before the indexer first ran are absent. "All
  time" means "since this deploy".
