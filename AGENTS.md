<!-- LATTICE_LANE: 98cbfdac-a5d9-4ec5-bb26-75a001f3f7bf -->

# Repository Instructions

## No Legacy, No Compat (until a live network exists)

- There are ZERO live ducktape networks. Nothing deployed needs backward
  compatibility, wire-format tolerance, or an upgrade path from older behavior.
- Keep ONLY the latest-spec implementation of every module and protocol. When a
  spec or format changes, replace the old code — never keep a legacy decoder, a
  versioned enum arm, a compat shim, or a config alias "just in case". Dual-path
  code is a defect, not prudence.
- Version numbering is reset to v1 and stays there: no protocol-version bumps,
  no v2/v3 names, no admission gates keyed on a version number. The
  invitation/join flow in particular is v1 — a "v2" hint anywhere in it is a
  bug.
- This holds until a real network is live. Re-introducing versioning, upgrade
  gating, or migration machinery is an explicit, user-requested decision —
  never a side effect of a task.

## Internal Skills

- Keep repo-specific operational runbooks in `skills/` (`qa`, `sim-lane`,
  `module-dev`). Nothing else lives there: a prompt for an agent running
  inside a network is product, not a runbook.
- `.claude/skills` and `.codex/skills` both point to the shared `skills/` directory.
- Keep assistant-facing repository guidance in this file; `CLAUDE.md` links here so both assistants read the same instructions.
- Workflow helpers are user-global, not repo-tracked; the branching and
  delivery rules below still bind assistant work in this repo.

## Docs Are Not a Record

- There is no decision-record system: no ADRs, no plan/spec archive, no docs
  site. The code, its comments, the skills, and git history are the record. A
  comment states its rule outright; it never cites a document for it.
- Specs and plans the brainstorming/writing-plans workflow writes under
  `docs/superpowers/` are local working files: the directory is gitignored and
  nothing under it ships in a PR. When the PR merges the plan is done and the
  file is garbage, like its worktree.
- `docs/` holds only what an operator executes (`deploy/`, `dogfood.md`,
  `sandbox-macos.md`) and the few records code or a skill cites by path
  (`records/`); `docs/README.md` is the index. A record nothing cites is
  deleted, not archived.

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
- **`ops/worktree-clean.sh` does it safely.** Dry-run by default; `--yes` to
  act. It removes worktrees whose branch is fully merged into `origin/dev`, and
  REFUSES one that is dirty, carries a commit not in `dev`, or has live
  processes under it (`--force` overrides only the last). Unmerged work is
  never its to throw away.
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
  not logging — a CLI's stdout (`ducktape <subcommand>`, `ducktape fs`/`ducktape mcp` included)
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
  `ducktape node log-filter 'info,ducktape::join=debug' -n <chain-id>`
  (the route MUTATES the process — a `trace` filter fills the disk — so it takes
  a user-signed request; the verb signs with the active wallet key, `curl`
  cannot.)

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

## Rust House Rules (code style)

Complement the lint/build gates above. The `rust` skill (let-else guards,
macros) still applies.

- **Explicit control flow.** No boolean-flag steering — don't set `did_x = true`
  up top for a branch below to read; restructure (early-return, extract the two
  paths, or branch once on a discriminant). Early return over nesting: handle the
  terminal case and get out, keep the main path at the left margin. One
  discriminant, one `match`: a multi-way decision branches on one tagged value,
  never a ladder of `if`/`else if` over loosely related booleans. Hot paths (the
  consensus/drain loop, the join gate + settle, signing/redeem) must read
  top-to-bottom; a change that makes one harder to trace is wrong as written,
  even when correct — restructure until the flow is obvious again, never thread
  another flag through one to patch it.
- **State machines = one visible dispatch, pure steps.** Every input is a named
  variant on ONE event enum. ONE `match` that does nothing before or after it:
  each arm is a single delegation to a handler named for its variant — no `_`
  wildcard (a new variant must fail the build until it's routed), no match guards,
  no logic inlined in an arm. Step functions DECIDE and return command/directive
  values; a separate executor performs the effects through a few named writers, in
  order. Decide-fns never write; writers never decide — that keeps transitions
  unit-testable without I/O and effect order owned by one place. When the shape is
  load-bearing, guard it with a source-parsing lint test, not a comment.
- **Named predicates.** Every non-trivial conditional is a named `let`/`const`
  above the branch (the name is the documentation); compose a complex condition
  from smaller named predicates rather than one giant expression. Never chain
  ternaries; a second `?:`-equivalent means lifting to named predicates +
  `if`/`match`.
- **Tests wait on events, never on time.** No bounded spin / sleep-and-retry (a
  disguised timeout that flakes on slow CI). Synchronize on the system's own
  events — a channel message, a drained frame, a status callback. No wait seam
  means a missing hook in the code: add the hook, not a sleep.
- **In-seam mechanical refactors: just do them and label the step** (flag →
  discriminant, `if`/`else if` ladder → `match`, name a predicate, extract a
  nested block) — scoped to the seam you're already in, stated as its own step,
  never silently bundled into an unrelated change. Structural refactors —
  relocating code across modules, changing a boundary/public shape, adding a
  file, or fanning out beyond the seam — are ask-first; when you can't tell
  which bucket a refactor is in, treat it as structural and ask.
