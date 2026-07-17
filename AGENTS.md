<!-- LATTICE_LANE: 98cbfdac-a5d9-4ec5-bb26-75a001f3f7bf -->

# Repository Instructions

## Internal Skills

- Keep repo-specific operational runbooks in `skills/` (`qa`, `sim-lane`, `upgrade`).
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
- **Reap retired QA state BEFORE removing the worktree — this order is not
  optional.** The current Iced workflow has no Fleet instance manager, but old
  external instance homes can still contain a pidfile and detached
  `ducktape-node`. Deleting their worktree first used to delete the teardown
  hook and leave the node running forever. `ops/worktree-clean.sh` retains a
  self-contained, identity-verified reaper for that historical state.
- **`ops/worktree-clean.sh` does the whole sequence safely.** Dry-run by
  default; `--yes` to act. It reaps orphaned retired-QA instances (killing only a pid
  it has verified is that workspace's own `ducktape-node`, by exe and
  `--config` — never `pkill -f`), then removes worktrees whose branch is fully
  merged into `origin/dev`. It REFUSES a worktree that is dirty or carries a
  commit not in `dev`; unmerged work is never its to throw away.
- Never stop desktop/QA processes with `pkill -f` — a pattern match will
  cheerfully kill an editor, a grep, or this script. Find them by process cwd,
  executable, and workspace config or let the native app shut them down.
- Merge to `dev` only when confidence is high: the change is understood and the
  relevant gates are green or any skips are justified. If confidence is medium
  or low, leave the PR open with the risks, failed checks, or follow-up review
  needed instead of merging by default.

## Logging

- Use `tracing`, never `println!`/`eprintln!`. An event reaches BOTH the node's
  stderr (tee'd into `<workspace>/daemon.log`) and the in-memory `LogRing` the
  app's Logs tab streams over the ws `logs` topic. A `println!` reaches NEITHER:
  it is invisible in the app and unfilterable by `RUST_LOG`. Program output is
  not logging — a CLI's stdout (`bin/fs`, `bin/mcp`, `ducktape-node <subcommand>`)
  stays `println!`.
- Two conventions coexist ON PURPOSE, and they are orthogonal — a `target` says
  WHERE an event came from, an `event` field says WHAT it is:
  - `target: "ducktape::<plane>"` — the filtering handle. `RUST_LOG=ducktape::join=debug`
    must light up a plane that spans several crates, which a crate-path target
    cannot express.
  - `event = "<stable_name>"` — the operational-contract events (the node status
    `operations` projection and the `ducktape_*` metrics). These are a MACHINE
    contract: a dashboard keys on the name, so do not rename one without treating
    it as a wire change.
  Use both together on a contract event. Neither replaces the other.
- **If it can fire more than once per block, it is not `info`.** The ring holds
  4096 lines; one `info!` per 100 ms drain tick evicts the whole thing every
  ~7 minutes, destroying the context around the event you were hunting.
  `error` = stopped and will not self-heal. `warn` = we refused or dropped
  something, for a nameable reason. `info` = a lifecycle fact, at most once per
  {boot, block, epoch, session, connection}. `debug` = per-op / per-request.
  `trace` = per-frame.
- A forever-retry loop logs attempt 1, then every Nth, carrying an `attempts`
  field. An unconditional `warn!` in one is a log bomb that evicts the very
  evidence you need — and the counter IS the diagnosis.
- Never log a URI path or query string (`/.duck/ws/{token}` carries a capability
  token in the path, and the ring is visible in the app) or any key
  material. A `reason` is a stable snake_case token, not prose — greppable and
  countable.
- Turn one plane up on a LIVE node rather than restarting it — a restart destroys
  the wedged state you restarted to look at:
  `curl -XPOST localhost:$PORT/v1/log-filter -d 'info,ducktape::join=debug'`
- Doctrine and phased rollout: `docs/superpowers/plans/2026-07-14-logging-doctrine.md`.

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
