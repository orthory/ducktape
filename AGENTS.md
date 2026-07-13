<!-- LATTICE_LANE: 98cbfdac-a5d9-4ec5-bb26-75a001f3f7bf -->

# Repository Instructions

## Internal Skills

- Keep repo-specific operational runbooks in `skills/` (`qa`, `tauri-debug`, `upgrade`).
- `.claude/skills` and `.codex/skills` both point to the shared `skills/` directory.
- Keep assistant-facing repository guidance in this file; `CLAUDE.md` links here so both assistants read the same instructions.
- Workflow helpers are user-global, not repo-tracked; the branching and
  delivery rules below still bind assistant work in this repo.

## Branching and Delivery

- All task work targets `dev`: worktrees fork from `origin/dev`, PRs are based
  on `dev`, and high-confidence reviewed PRs merge into `dev`. `main` advances
  only by an explicit, user-requested release of `dev`. Binding detail lives in
  `.project/work.md`.
- Default feature/fix/doc work happens in an isolated git worktree rather than
  directly in the primary checkout. Use the current checkout only when the user
  explicitly asks for in-place work or the task is limited to repo-state repair.
- Put every task worktree under `<primary-checkout>/.worktree/<branch-slug>`.
  Never create one as a sibling checkout, under `/tmp`, or in assistant-specific
  `.claude/worktrees` or `.codex/worktrees` directories. Keep Cargo targets and
  other large build outputs in the disk-backed worktree too: `/tmp` may be a
  memory-backed filesystem, so building there consumes RAM and swap.
- After the worktree change is implemented and verified, submit it as a PR
  against `dev`. Do not treat local completion as done when the requested flow
  is delivery.

## Worktree Cleanup (a merged worktree is garbage — remove it)

- **A worktree's life ends when its PR merges.** Once merged, remove the
  worktree and delete its branch. Leaving it costs ~20 GB of Cargo target each
  and nothing else; twelve of them once ate 250 GB and had to be swept by hand.
- **Stop the QA fleet BEFORE removing the worktree — this order is not optional.**
  `.tauri-agent/fleet.json` points `cleanupInstance` at
  `qa/fleet/cleanup-instance.ts`, a path *inside the worktree*, while the
  instance's workspace, pidfile and detached `ducktape-node` live *outside* it
  under `FLEET_HOME`. Delete the worktree first and you delete the only thing
  that could ever stop its node: it then runs forever, unreachable by
  `fleet down`. That is not hypothetical — a node was found still up 40 hours
  after its worktree was gone, beside 9.2 GB of instance homes whose worktrees
  no longer existed. So: `"$FLEET" down <instance-id>`, *then* remove the tree.
- **`ops/worktree-clean.sh` does the whole sequence safely.** Dry-run by
  default; `--yes` to act. It reaps orphaned fleet instances (killing only a pid
  it has verified is that workspace's own `ducktape-node`, by exe and
  `--config` — never `pkill -f`), then removes worktrees whose branch is fully
  merged into `origin/dev`. It REFUSES a worktree that is dirty or carries a
  commit not in `dev`; unmerged work is never its to throw away.
- Never stop desktop/QA processes with `pkill -f` — a pattern match will
  cheerfully kill an editor, a grep, or this script. Find them by process cwd or
  let Fleet's own teardown do it.
- Review the PR from a clean context before merging: re-read the diff against
  `dev`, check for scope creep and missing verification, and address actionable
  feedback before deciding mergeability.
- Merge to `dev` only when confidence is high: the change is understood, the
  relevant gates are green or any skips are justified, and the clean-context
  review has no blocking concerns. If confidence is medium or low, leave the PR
  open with the risks, failed checks, or follow-up review needed instead of
  merging by default.

## Rust Gates

- Per-crate lint gate:
  `cargo clippy -p <crate> --tests --no-deps` — the
  `--no-deps` is deliberate. Without it, a crate whose dev-deps pull
  host/dispatch/saga inherits ~a dozen pre-existing version-drift lints from
  those crates; a task is accountable only for lints in the crates it touched.
- Don't run `cargo fmt --all`: large bin files carry pre-existing fmt debt,
  and a tree-wide reformat forces painful rebases on in-flight branches. Only
  format code you touched; the mechanical whole-tree sweep is a dedicated PR.
- The files crate's wasm-readiness gate:
  `cargo check -p files --no-default-features` must stay green
  (no `std::fs`/sdk leaks into the pure core).
