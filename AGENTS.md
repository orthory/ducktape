# Repository Instructions

## Internal Skills

- Keep reusable project-specific agent workflows in `skills/`.
- `.claude/skills` and `.codex/skills` both point to the shared `skills/` directory.
- Keep assistant-facing repository guidance in this file; `CLAUDE.md` links here so both assistants read the same instructions.
- Standard workflow skills: `plan`, `work`, `done`, and `blast`.

## Branching

- All task work targets `dev`: worktrees fork from `origin/dev`, PRs are based
  on `dev`, and `done` merges into `dev`. `main` advances only by an explicit,
  user-requested release of `dev`. Binding detail lives in `.project/work.md`.
