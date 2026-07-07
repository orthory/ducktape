# Repository Instructions

## Internal Skills

- Keep repo-specific operational runbooks in `skills/` (`qa`, `tauri-debug`, `upgrade`).
- `.claude/skills` and `.codex/skills` both point to the shared `skills/` directory.
- Keep assistant-facing repository guidance in this file; `CLAUDE.md` links here so both assistants read the same instructions.
- Workflow skills (`work`, `done`, `blast`, `plan`, ...) are user-global, not repo-tracked; the branching rules below still bind them here.

## Branching

- All task work targets `dev`: worktrees fork from `origin/dev`, PRs are based
  on `dev`, and `done` merges into `dev`. `main` advances only by an explicit,
  user-requested release of `dev`. Binding detail lives in `.project/work.md`.

## Rust Gates

- Per-crate lint gate: `cargo clippy -p <crate> --tests --no-deps` — the
  `--no-deps` is deliberate. Without it, a crate whose dev-deps pull
  host/dispatch/saga inherits ~a dozen pre-existing version-drift lints from
  those crates; a task is accountable only for lints in the crates it touched.
- Don't run `cargo fmt --all`: large bin files carry pre-existing fmt debt,
  and a tree-wide reformat forces painful rebases on in-flight branches. Only
  format code you touched; the mechanical whole-tree sweep is a dedicated PR.
- The files crate's wasm-readiness gate: `cargo check -p files
  --no-default-features` must stay green (no `std::fs`/sdk leaks into the
  pure core).
