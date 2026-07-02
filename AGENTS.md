# Repository Instructions

## Internal Skills

- Keep reusable project-specific agent workflows in `skills/`.
- `.claude/skills` and `.codex/skills` both point to the shared `skills/` directory.
- Keep assistant-facing repository guidance in this file; `CLAUDE.md` links here so both assistants read the same instructions.
- Shared workflow skills currently available: `planning-mode`, `working-in-worktrees`, `publishing-prs`, and `reviewing-prs`.
