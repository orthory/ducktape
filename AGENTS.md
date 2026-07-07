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

## Code Organization

- No mono-files. New Rust source files target one responsibility each with a
  ~600-line soft cap; split by seam (`state`/`install`/`events`/`actions`-style
  submodules) before a file grows past it. `interface.rs` stays types-only.
  Inline `#[cfg(test)]` mods live with the code they test and count toward the
  cap; a test-heavy file may exceed it only when splitting would break a shared
  invariant — say so in a comment or the PR.
- Reference crates (`crates/examples/docs-harness`,
  `crates/testing/quack-harness/src/dummy/`) model this layout — copy their
  structure for new modules and packages.
- Never grow the legacy monsters mid-feature (`bin/node/src/main.rs`,
  `crates/apps/runs/src/lib.rs`, `crates/apps/pages/src/lib.rs`): new node CLI
  verbs get their own module (see `bin/node/src/package.rs`), and substantial
  additions to runs/pages should land as new submodules, not appended inline.
  Restructuring the existing bodies is dedicated refactor work, not a rider.
