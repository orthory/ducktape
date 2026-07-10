# Agent dogfooding loop — spec-driven agent development on the ducktape network

**Date:** 2026-07-10
**Status:** approved direction, spec under user review
**Scope:** M1 (loop skeleton) + M2 (Kiro layer + usage ledger), one campaign
**Depends on:** deterministic-agent-runtime ADR (`docs/adr/2026-07-09-deterministic-agent-runtime.mdx`, merged), portable runtime #294 (live), forge tracker #247, pages #250, account/identity split #309

## Product statement

The team writes specs collaboratively in Pages, agents execute them against
forge-hosted repos, every iteration feeds back as a PR session and as
block-anchored spec commentary, and the cost is carried by the team's pooled
AI subscriptions (each member's own logged-in CLI on their own device). First
customer: ducktape itself — the repo is already self-hosted via
`make dogfood-forge`.

The Kiro translation: Kiro fakes spec-driven development with local files
(requirements.md / design.md / tasks.md); ducktape has each piece as a
consensus module. Spec = a Pages subtree; task list = todo blocks +
`SetChecked`; execution = runs chained on a PR branch; traceability =
`BlockRef`-anchored agent comments. The spec page doubles as a live,
team-visible dashboard, which Kiro's local files cannot do.

## Current state this design builds on (verified 2026-07-10)

- **Execution plane works end-to-end**: capability announce (34
  provider_model_effort tags), saga rendezvous + first-Accept claim, BYO-auth
  CLI subprocess (`crates/kernel/capability-host`), off-loop pool
  (`crates/system/dispatch-oracle/src/pool.rs`), resident announce + drain.
- **v3 portable envelope is live on every run** (#294): per-run duckfs
  workspace, host-assembled `RunnerResult`, `output_ref`, W1 root relocation.
- **Forge is production-shaped**: smart-HTTP git remote, multi-branch ref-CAS
  `PushRefs`, issues/PRs/reviews, client-computed merge, hidden
  `forge:<repo>:<n>` chat channel per item (`crates/apps/forge`).
- **The PR sink already exists and waits for a branch**:
  `crates/apps/runs/src/lib.rs` `emit_sink` fires `ForgeMsg::OpenPr` gated on
  the `ForgePush` cap and a `forge_branch_born` probe. Nothing today can birth
  that branch, and the oracle hardcodes `Sink::Chain` (`pool.rs:317`).
- **Gaps this design closes**: workspace is duckfs-only (no repo); no
  forge/pages event can trigger a run (tagging plane is chat-only); issue
  bodies never reach the model (body lives on the record, not in chat);
  `Discussion.tsx` posts raw `postMessage` (no mention parsing, no
  auto-watch); delivered runs prune instantly (no history); pages↔agent
  coupling is zero; no usage accounting.

## M1 — loop skeleton: issue @mention → work → PR → re-mention

### M1.1 Forge workspace source (envelope extension — flag day)

`RunEnvelope v3.workspace` gains a source discriminant:

- `duckfs { source_prefix, source_snapshot }` — today's shape, unchanged
  default for plain chat runs.
- `forge { repo, commit, branch }` — used when the triggering channel is
  `forge:<repo>:<n>`. `commit` = the committed head at compose height
  (`ForgeQuery::ListRefs`), so the source is consensus-pinned (ADR W2).
  `branch` = the stable work branch for the item (M1.4).

Compose rule in `runs`: trigger channel `forge:<repo>:<n>` → forge source;
anything else → duckfs as today. Envelope byte change = coordinated flag day
(M1 of the ADR); compat is waived on dev per 2026-07-10 policy.

### M1.2 D3 enforcement point #2 — ForgeRead at compose

Composing a forge-sourced run requires
`AgentRecord::permits(CapRequest::ForgeRead(repo))`. A missing cap fails the
run at compose with a deterministic, user-visible reason (same lane as other
compose failures). This is the second production cap gate after `ForgePush`
(#298 D3 completion continues per-mechanism as ADR requires).

### M1.3 Worktree provisioning + push-back (NodedProvisioner)

Forge branch of `bin/noded/src/agent_provision.rs`:

1. Materialized repo exists node-local at `<storage>/forge-repo/<repo>`
   (both `bin/node` and `noded` register forge). Provision = `git worktree
   add <run_dir> <pinned_commit>` on the work branch (create if absent). Run
   dir stays under the relocated `agent-runs` root (W1/D7 relocation holds;
   the gitdir pointer into `<storage>/forge-repo` grants no new read power —
   reads are unsandboxed today anyway, tracked under D7's deferred env half).
   Requires host `git` with sha256 object-format support (forge repos are
   sha256); dev boxes satisfy this — probe at provision and fail loudly.
2. After the provider exits: `git add -A && git commit` with **author = the
   agent** (`name = <agent_id>`, synthetic email — D2 two-level attribution
   in commit metadata), committer = node identity. No changes → no commit, no
   push; receipt notes `no_changes`.
3. Push the work branch via the loopback smart-HTTP remote
   (`http://127.0.0.1:<http_listen>/forge/<repo>`) — reuses the
   pack→blob→`PushRefs` lane wholesale, CAS included. A concurrent-update CAS
   reject degrades the receipt (`commit_error`) without losing the reply
   (R4 discipline, same as duckfs commit failure today).
4. `output_ref` = the forge commit oid + branch, carried on
   `WorkspaceReceipt` and distilled into the delivery receipt.
5. Cleanup: `git worktree remove` (W5); the panic/timeout brackets from #326
   wrap this path identically.

### M1.4 Stable work branch + sink routing

- Branch identity is per **item**, not per run: `agent/item-<n>` (issues and
  PRs share one number space per repo). First run on the issue creates it
  from the pinned head;
  every later run in that item's channel forks from the branch tip and pushes
  the same branch. This is what makes M1.6 a session.
- Sink is requested **in the envelope's result contract**, composed by
  `runs` from trigger context: forge item channel → `Pr`; plain chat →
  `Chain`. The agent record does not change; the workflow context is the
  opt-in (ADR O2 spirit: chain stays the global default). The oracle threads
  the requested sink into `RunnerResult` instead of hardcoding `Chain`.
- `emit_sink` gains one guard: if an open PR already exists with this source
  branch, skip `OpenPr` (the branch update itself is the feedback). PR title
  = first line of the `message` facet (clamped), body = full message + a
  receipt breadcrumb (run id, output_ref, executing node).

### M1.5 Trigger — mention in the item discussion

- `Discussion.tsx` swaps its raw `postMessage` path for the shared composer
  machinery: mention typeahead, `splitMentions` mention marks, and
  `ensureMentionWatch` (auto-watch on first agent mention, policy
  `mention`). `forge:*` channels stay hidden from the main chat rail and
  pickers; inside the item view they are first-class.
- Loop safety is already guaranteed: only `Author::User` posts fire
  engagement, so forge's Module-authored state lines and the agent's own
  replies cannot re-trigger runs.

### M1.6 Item context injection

For forge-channel runs, `runs` composes an additional deterministic
instructions section from committed tracker state: item kind/number/state,
title, **body** (today the body never reaches the model), repo coordinates,
work branch, and — when the item is a PR — the PR's source/target branches.
Byte-capped (16 KiB, truncate-with-marker). The 64-message chat window
remains the conversation facet, unchanged.

### M1.7 PR = session (re-mention loop)

A mention in a **PR's** channel composes the same forge source with `commit`
= the PR source branch's committed tip. The run forks the branch tip, pushes
more commits to the same branch; the open PR updates in place; the reply
lands in the same discussion. Review comments → re-mention → fix → push is
the whole iteration loop, each round leaving a receipt. No new mechanism —
this is M1.1–M1.4 applied to a PR item, plus branch-tip (instead of
main-head) pinning.

### M1.8 Observability floor (app)

- Stop pruning delivered runs from app state: retain a bounded history
  (last 100) with outcome, duration, executing node, output_ref, and PR
  number/link when the sink fired. Surface in Agents ▸ Activity.
- Fix the run-finished notification deep-link: target the anchor
  channel/thread (where the reply landed), not the generic Agents screen.
- RunRow renders wall-clock time, not the raw consensus counter.

## M2 — Kiro layer + usage ledger

### M2.1 Spec pages by convention

A spec is a Pages subtree: root page (overview) + subpages (requirements /
design / tasks by convention, not schema). Tasks are todo blocks. No module
change; M2 makes agents able to *read* and *annotate* this shape.

### M2.2 Page injection (spec → run)

- Reference syntax: `[[page:<page-id>]]` in an issue body or chat message.
- At compose, `runs` resolves each ref against committed pages state at
  compose height (deterministic — pages is consensus state, no blob fetch
  involved), renders the subtree (depth-capped, 64 KiB budget,
  truncate-with-marker) to a markdown instructions section: headings, todo
  blocks with checked state and **block ids**, code blocks fenced.
- Block ids are rendered inline (e.g. `[blk:abc123]`) so the model can target
  them with M2.3 actions.

### M2.3 Agent pages actions (boundary effects)

Extend the action vocabulary (`KNOWN_ACTIONS`) with:

- `pages.comment { target: <block-or-page id>, body }` → `PageMsg::AddComment`
  authored `AuthorRef::Agent` (same pattern as agent chat replies: `runs`
  emits the follow-up and pages derives agent authorship from the payload's
  attribution, not the module origin) — block-anchored commentary ("§3
  implemented in commit X; requirement contradicts Y, resolved by Z").
- `pages.set_checked { block, checked }` → `PageMsg::SetChecked` — the agent
  checking off spec tasks as commits land; the spec page becomes the live
  dashboard.

Both apply at the run boundary from the winning attempt (X2), ride the
existing effects lane (`MAX_ACTIONS_PER_RUN = 8` stands; agents batch), and
are validated liberally→strictly like existing actions. Action-vocabulary
extension changes op admission = lockstep upgrade (same class as #232).

### M2.4 D3 pages caps

`ResourceCaps` gains `pages_write` (page-id scoped, `*` allowed);
`CapRequest::PagesWrite(page)` gated in the effect-application path in
`runs`. Reads need no cap in M2 (pages are workspace-visible to members;
revisit with private lanes).

### M2.5 Usage ledger (no consensus change)

- Attribution already exists in committed state: each attempt's `assignee`
  (saga), the capability tag, timing, and outcome; #309 maps node → account.
- Build an indexer view (fluent31 `ModuleIndexer` lane, served via
  `/v1/index/…`) aggregating per-account and per-tag: runs executed, runs
  requested, total duration, tokens where the audit lane recorded them (R6).
- App surface: a Usage card (Account screen or Agents view) — "whose
  subscription carried how much this week". Quotas/fairness deliberately
  deferred to M3.

## Explicitly deferred (M3+)

CI check runs (non-LLM capability specs — first item once agent PR volume
makes human-verified merges the bottleneck), D7 env isolation (child env
hygiene / sandbox mechanism), preferential/pinned routing (warm-cache boxes,
"run on my Max sub"), Merge sink, W6 skill ro-mounts, W4 recipes,
sub-agent delegation (D4/N2), pages read caps, PR-diff↔block deep links.

## Consensus impact / rollout

- **M1 flag day**: envelope workspace discriminant + result-contract sink
  request (composer bytes change).
- **M2 flag day**: action vocabulary + `ResourceCaps` shape.
- Both are lockstep-class on dev where compat is waived; sequence M1 → M2 so
  the Kiro layer lands on a proven loop. M2's rollout order follows ADR M2
  (accept → wire → flip).
- Known open dependency: #298 resident prompt-blob gap — resident-executed
  runs still resolve prompt blobs locally; dogfood ceremony seeds prompts via
  any validator node until that lands.

## Testing

- **M1 e2e** (dispatch_e2e style, fake provider): issue → mention → run →
  branch born → PR opened → re-mention on PR → second commit on same branch,
  receipt chain asserted. CAS-conflict and no-changes degrade paths pinned.
- **Provision unit**: worktree off sha256 repo, agent-authored commit,
  loopback push, cleanup; git-without-sha256 probe failure is loud.
- **Compose tests**: forge-channel source selection, ForgeRead gate, item
  body + page-ref injection (byte caps, determinism at height).
- **M2**: pages action application + cap gate + AuthorRef::Agent authorship;
  ledger indexer aggregation; live QA per repo `qa` skill (headless app,
  real node) for the Discussion mention flow.

## Risks

- **Cold Rust builds vs the 1h hard cap** (idle 600s × 6): dogfood agents run
  with `DUCKTAPE_PROVIDER_TIMEOUT_SECS` raised; warm-cache member boxes are
  the norm; preferential routing is the M3 fix.
- **Host git dependency** (sha256 worktrees): probed at provision, loud fail.
- **Concurrent runs on one item**: branch CAS makes the loser degrade
  honestly; acceptable at dogfood volume.
- **ToS posture**: pooled execution = each member's own CLI on their own
  device; product copy should say "your devices become the team's agent
  runners", not "share your subscription".
