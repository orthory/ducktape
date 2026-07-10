# Agent Dogfooding Loop — Implementation Plan (compressed)

> **For agentic workers:** this plan is deliberately compressed for a
> Fable-class executor: it fixes constraints, interfaces, file map, and
> verification gates, and trusts you to derive the code. Read the spec first:
> `docs/superpowers/specs/2026-07-10-agent-dogfooding-loop-design.md`. Read
> the ADR second: `docs/adr/2026-07-09-deterministic-agent-runtime.mdx`.
> Track progress with the checkboxes; do not silently drop a checkbox.

**Goal:** Close the spec-driven dogfooding loop: @mention an agent on a forge
issue → it works in a real worktree of the forge repo → pushes
`agent/item-<n>` → a PR opens → re-mention on the PR iterates the same branch
(PR = session). Then the Kiro layer: Pages specs injected into runs,
block-anchored agent commentary, and a usage ledger.

**Architecture:** No consensus-engine change. All deltas live in the
host-edge apps (`runs`, `dispatch-oracle`, `capability-host` untouched except
sink threading), the provisioner (`bin/noded/src/agent_provision.rs`), and
the app. Two coordinated flag days: M1 (envelope bytes), M2 (action vocab +
caps shape). Compat is waived on dev (2026-07-10 policy) — no dual-version
shims; bump and re-pin goldens.

**Delivery:** Two worktree branches off `origin/dev`, two PRs:
`feat/agent-dogfood-m1` and `feat/agent-dogfood-m2` (M2 rebases on merged
M1). Adversarial clean-context review before each merge, per repo rules.

## Global Constraints

- Lint gate per touched crate: `cargo clippy -p <crate> --tests --no-deps`
  (`--no-deps` is deliberate). `touch` the crate's `.rs` files first — cached
  cargo passes vacuously. Check `cargo check` too (test-only reachability
  hides bin dead_code).
- Never `cargo fmt --all`; format only touched code.
- `cargo check -p files --no-default-features` must stay green.
- Mono-file mandate: new logic goes in NEW files split by responsibility
  (~600-line soft cap). `crates/apps/runs/src/lib.rs` is already a 4.8k
  monster — put new compose/injection/sink logic in new modules
  (`forge_source.rs`, `inject.rs`, `pages_effects.rs`), not in lib.rs bodies.
- Determinism invariants are law: everything composed into the envelope or
  applied as an effect must derive from committed state at compose height
  (I1); effects settle only at the run boundary from the winning attempt
  (X2); malformed model output degrades, never delivery-blocks (R4); the run
  holds no key (D1).
- Byte caps, copied from spec verbatim: item-context injection 16 KiB,
  page render budget 64 KiB (both truncate-with-marker),
  `MAX_ACTIONS_PER_RUN = 8` stands.
- Branch naming: `agent/item-<n>` — per item, NOT per run (session identity).
- Commit authorship: author = agent (`name = <agent_id>`, synthetic email),
  committer = node identity (D2).
- Live QA on this box: use the repo `qa` skill; hazards — `tauri dev`
  truncates `target/debug/ducktape-node` if `bun run sidecar` is skipped (run
  standalone nodes from a copy outside `target/`); a fleet vite on :1430
  hijacks worktree tauri dev; rustc thin-LTO SIGSEGV = retry.
- Known open dependency (do NOT fix here): #298 resident prompt-blob gap —
  resident-executed runs resolve prompt blobs locally. Dogfood ceremony seeds
  prompts via a validator node.

---

## PR 1 — M1: issue-mention → worktree → PR → session (flag day)

### Task 1: envelope + compose (runs)

**Files:** modify `crates/apps/runs/src/envelope.rs`; create
`crates/apps/runs/src/forge_source.rs` + `crates/apps/runs/src/inject.rs`;
modify `crates/apps/runs/src/lib.rs` (wiring only).

**Interfaces (produced, canonical names — downstream tasks use these):**
- `WorkspaceSource::{ Duckfs { source_prefix, source_snapshot }, Forge { repo, commit, branch } }`
  replacing the flat duckfs-only workspace fields in the v3 envelope. Wire
  encoding may change freely (flag day); keep serde shape skip-compatible for
  empty facets as today.
- `result_contract` gains `sink: WireSink` (default `Chain`), composed from
  trigger context: channel `forge:<repo>:<n>` → `Pr`, else `Chain`.
- Compose rules (in `forge_source.rs`): trigger channel `forge:<repo>:<n>` →
  resolve item via forge tracker queries; source `commit` = committed head of
  the repo's default branch (`ForgeQuery::ListRefs`) for issues, or the PR's
  source-branch tip for PR items (this one rule IS the PR=session feature);
  `branch = agent/item-<n>`. Gate on
  `AgentRecord::permits(CapRequest::ForgeRead(repo))` — missing cap fails the
  run at compose with a deterministic reason (existing compose-failure lane).
- Injection (in `inject.rs`): for forge-channel runs, append a deterministic
  instructions section from committed tracker state — item kind/number/state,
  title, **body**, repo coordinates, work branch, PR source/target when
  applicable. 16 KiB cap, truncate-with-marker. Conversation window (64
  msgs) unchanged.

**Verify:** compose unit tests — source selection by channel kind, ForgeRead
gate (permit/deny), issue-head vs PR-tip pinning, injection determinism +
byte cap. Re-pin any envelope golden bytes (flag day). Crate gates green.

### Task 2: worker decode + sink threading (dispatch-oracle)

**Files:** modify `crates/system/dispatch-oracle/src/envelope.rs`,
`provision.rs`, `pool.rs`.

**Interfaces:**
- `WorkspaceSpec` gains the forge variant mirroring `WorkspaceSource::Forge`;
  `PortablePlan` carries the requested sink.
- `pool.rs::execute` threads the plan's requested sink into
  `assemble_runner_result` — delete the hardcoded `Sink::Chain`
  (today at `pool.rs:317`). Chain remains the default when the plan carries
  none.
- Keep the existing brackets intact around the forge path exactly as the
  duckfs path: `workspace_step_timeout()` bound, panic-unwind cleanup guard,
  `(saga_id, attempt)` idempotency.

**Verify:** extend the existing pool tests (pattern:
`a_v3_runs_skills_reach_the_spec_as_ro_mounts`) — forge spec reaches the
provisioner; requested sink reaches the result; timeout/unwind still cover
the forge path.

### Task 3: forge worktree provisioner (noded + node)

**Files:** create `bin/noded/src/agent_provision/forge.rs` (split the
existing file into a module dir if cleaner — mono-file rule); modify
`bin/noded/src/agent_provision.rs`, `bin/node/src/main.rs` +
`bin/noded/src/lib.rs` only if wiring needs the forge base path / http port
handed to the provisioner.

**Behavior (provision → commit → push):**
1. Probe host `git` for sha256 object-format support once at provisioner
   construction (`git init --object-format=sha256` in a temp dir); fail loud
   and permanent if absent.
2. Provision: from the node-local materialized repo
   (`<storage>/forge-repo/<repo>`), `git worktree add <run_dir> <commit>` on
   branch `agent/item-<n>` (create at `<commit>` if absent; reuse tip if the
   branch exists — session continuation). `<run_dir>` stays under the
   relocated agent-runs root (W1 checks unchanged).
3. Commit: after provider exit, `git add -A && git commit` with agent
   authorship (Global Constraints). No changes → receipt `no_changes`, no
   push.
4. Push: `git push http://127.0.0.1:<http_listen>/forge/<repo>
   agent/item-<n>` — reuses the receive-pack→blob→`PushRefs` lane, CAS
   included. CAS/pack failure → receipt `commit_error` + `Status::Degraded`
   (mirror the duckfs commit-error path exactly); the reply still delivers.
5. `WorkspaceReceipt` gains `branch` and the commit oid as the forge
   `output_ref`; `runs` distills both into the delivery receipt.
6. Cleanup: `git worktree remove --force` in the same cleanup/unwind path as
   the duckfs dir removal (W5).

**Verify:** provision unit tests against a temp sha256 repo (worktree off
pinned commit; branch reuse takes the tip; agent-author commit; no-changes;
CAS-conflict degrade; cleanup on panic). Use `std::process::Command` git —
same as production path.

### Task 4: PR sink completion (runs)

**Files:** modify `crates/apps/runs/src/lib.rs` `emit_sink` region (extract
to `sink.rs` if the diff grows past ~100 lines).

- Guard: before `OpenPr`, query tracker state — an OPEN PR whose source is
  `agent/item-<n>` already exists → skip OpenPr (branch update is the
  feedback), breadcrumb notes "updated PR #k".
- PR title = first line of the `message` facet clamped to 100 chars; body =
  full message + receipt breadcrumb (run id, output_ref, executing node).
- Existing gates stay: `ForgePush` cap + `forge_branch_born` probe; all
  degrade paths remain breadcrumbs, never aborts.

**Verify:** unit tests — duplicate-PR guard, title/body derivation, cap-deny
degrade. Then the M1 e2e (Task 6).

### Task 5: app trigger + observability floor

**Files:** modify `app/src/console/views/forge/items/Discussion.tsx`;
reuse `app/src/console/views/chat/mention.ts`, `MentionMenu.tsx`,
`app/src/domain/chat-input.ts` (`splitMentions`), store actions
(`ensureMentionWatch` path in `actions.ts`); modify
`app/src/domain/runs-client.ts`, `app/src/console/views/agent/RunsTimeline.tsx`,
`app/src-tauri/src/notify/matchers.rs`.

- Discussion composer: replace raw `postMessage` with the shared mention
  machinery — typeahead, mention marks, `ensureMentionWatch` (auto-watch on
  first agent mention, policy `mention`). `forge:*` channels stay hidden from
  the main chat rail/pickers (`chat-client.ts:103-106` filter untouched).
  Extract shared composer logic rather than duplicating `Composer.tsx`
  keyboard/IME handling — one mention engine, two shells.
- Run history: `runs-client.ts` stops pruning delivered runs; retain the
  last 100 with outcome, duration, executing node, output_ref (branch +
  commit for forge), PR number when the sink fired. Render in Agents ▸
  Activity; RunRow shows wall-clock time (`isWallClock` guard pattern from
  `chat-helpers.ts:51`), not the raw consensus counter.
- Notification deep-link: run-finished targets the anchor channel/thread
  (where the reply landed) instead of `target("agent")`
  (`matchers.rs:167`).

**Verify:** `cd app && bun run typecheck`; live QA via the `qa` skill —
mention an agent from an issue Discussion in the real headless app, watch
the auto-watch appear, the run land, the reply post in the discussion.

### Task 6: M1 e2e (the loop, proven)

**Files:** create `bin/node/tests/dogfood_loop_e2e.rs` (pattern:
`portable_workspace_e2e.rs` — 3 validators, fake provider via SpawnFn
injection, blob seeding via `POST /v1/files/blob`).

Scenario, one test, asserted end-to-end: create repo + push seed commit →
open issue → mention agent in the item channel (post with mention mark) →
run executes with a forge worktree → branch `agent/item-<n>` born → PR
opened with title from the message facet → re-mention in the PR channel →
second run forks the branch TIP → second commit lands on the same branch →
no second PR → receipts carry branch/commit both times. Plus one degrade
case: concurrent branch advance → CAS reject → `Status::Degraded`, reply
still delivered.

- [ ] Task 1 done & gates green
- [ ] Task 2 done & gates green
- [ ] Task 3 done & gates green
- [ ] Task 4 done & gates green
- [ ] Task 5 done & typecheck + live QA pass
- [ ] Task 6 e2e green
- [ ] PR 1 opened against dev, adversarial review, merge on high confidence

## PR 2 — M2: Kiro layer + usage ledger (flag day)

### Task 7: page injection (runs)

**Files:** extend `crates/apps/runs/src/inject.rs`; touch
`crates/apps/pages/src/interface.rs` only if a subtree query is missing.

- Parse `[[page:<page-id>]]` refs from the triggering message text AND the
  injected item body. Resolve each against committed pages state at compose
  height; render the subtree depth-first to markdown: headings by level,
  todo blocks as `- [ ]`/`- [x]` with inline block ids (`[blk:<id>]`), code
  blocks fenced, comments omitted. 64 KiB budget, truncate-with-marker,
  unresolvable ref → one-line marker (never a failure).

**Verify:** unit tests — render shape, block-id inlining, budget truncation,
missing-page marker, determinism (same height → same bytes).

### Task 8: pages actions + caps (agent + runs + pages)

**Files:** modify `crates/apps/agent/src/interface.rs` (KNOWN_ACTIONS +
`ResourceCaps.pages_write` + `CapRequest::PagesWrite`); create
`crates/apps/runs/src/pages_effects.rs`; modify `crates/apps/pages/src/lib.rs`
(accept agent attribution on AddComment/SetChecked follow-ups).

- New actions: `pages.comment { target, body }` → `PageMsg::AddComment`;
  `pages.set_checked { block, checked }` → `PageMsg::SetChecked`. Applied at
  the run boundary from the winning attempt via the existing effects lane
  (liberal accept → strict validate → apply; bad action = that action
  degrades, run delivers).
- Authorship: `runs` emits follow-ups carrying `(owner, agent_id)`
  attribution; pages constructs `AuthorRef::Agent` from the payload — same
  pattern as agent chat replies. Origin stays Module("runs").
- Cap gate: `permits(CapRequest::PagesWrite(page))` at effect application
  (page-id scoped, `*` allowed). Deny → degrade breadcrumb.
- App: RegisterAgentForm/AgentEditForm expose the two new grants + the
  pages_write cap field (follow the existing allowed_actions chips).

**Verify:** unit tests — action decode/validate, cap permit/deny,
AuthorRef::Agent lands on the committed comment; goldens re-pinned (lockstep,
same class as #232). App typecheck.

### Task 9: usage ledger (indexer + app)

**Files:** create the indexer in the established fluent31 `ModuleIndexer`
lane (find the existing indexer registrations in `bin/noded/src/lib.rs`
around the `/v1/index/{module}/view` route and follow that pattern —
node-local materialized view, NO consensus change); create
`app/src/console/views/agent/UsageCard.tsx` (mount in AgentView or Account
screen, whichever has the lighter diff).

- Aggregate per finalized attempt: assignee node key → account (identity
  module mapping from #309), capability tag, outcome, duration; tokens only
  where the R6 audit lane recorded them. Expose per-account and per-tag
  rollups (runs executed, runs requested, total duration, tokens-if-known).
- App card: "whose subscription carried how much" — per-account rows with
  per-tag breakdown, current week. No quotas, no enforcement (M3).

**Verify:** indexer unit test over a synthetic attempt history; live QA —
run two agents on different nodes in the localnet fleet, see both accounts
accrue.

### Task 10: dogfood ceremony (docs, no code)

**Files:** create `docs/dogfood.md`.

Short runbook: `make dogfood-forge` (repo push) → register the dogfood agent
(prompt seeded via a validator node — #298 caveat; caps: `forge_read` +
`forge_push` on `ducktape`, `pages_write: *`, the two pages actions +
chat.post/tasks.*) → raise `DUCKTAPE_PROVIDER_TIMEOUT_SECS` for cold Rust
builds (idle 600s × 6 hard cap is the ceiling) → open an issue with a
`[[page:<id>]]` spec ref → mention → review the PR in-app.

- [ ] Task 7 done & gates green
- [ ] Task 8 done & gates green (lockstep goldens re-pinned)
- [ ] Task 9 done & live QA pass
- [ ] Task 10 runbook committed
- [ ] PR 2 opened against dev (rebased on merged PR 1), adversarial review,
      merge on high confidence

## Out of scope (do not build here)

CI check runs, D7 env isolation, preferential/pinned routing, Merge sink,
W6 skill ro-mounts, W4 recipes, sub-agent delegation, pages read caps,
PR-diff↔block deep links, quotas/fairness. If a task seems to need one of
these, stop and surface it instead of building it.
