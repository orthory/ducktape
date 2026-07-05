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
