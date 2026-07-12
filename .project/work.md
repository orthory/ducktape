# Binding work rules

- All task work targets the `dev` branch: start worktrees from a checkout on
  `dev` (branches fork from `origin/dev`), keep PRs based on `dev`, and merge
  high-confidence reviewed PRs into `dev`. Never base or merge task PRs on
  `main` — `main` advances only by an explicit, user-requested release of
  `dev`.
- Default feature/fix/doc work happens in an isolated git worktree. Use the
  primary checkout only when the user explicitly asks for in-place work or the
  task is limited to repo-state repair.
- Create task worktrees only at `<primary-checkout>/.worktree/<branch-slug>`.
  Do not use sibling directories, `/tmp`, `.claude/worktrees`, or
  `.codex/worktrees`. Large build outputs must remain on the same disk-backed
  filesystem; a tmpfs target consumes RAM and swap instead of disk.
- After implementation and verification, submit a PR against `dev`.
- Review the PR from a clean context before merge: inspect the diff against
  `dev`, look for scope creep and missing verification, and resolve actionable
  feedback.
- Merge to `dev` only when confidence is high. If confidence is medium or low,
  keep the PR open and report the remaining risks, failed checks, or review
  needed instead of merging by default.
