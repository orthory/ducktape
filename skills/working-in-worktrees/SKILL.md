---
name: working-in-worktrees
description: Use when starting Ducktape feature work, fixes, docs/process changes, or any task where the user mentions a worktree, main-based branch, isolated workspace, branch hygiene, or preserving dirty local work before editing.
---

# Working In Worktrees

## Overview

Start Ducktape work from a clean `origin/main` branch in an isolated worktree. The primary checkout may be dirty or owned by another agent, so treat it as shared state until proven otherwise.

## Workflow

1. Inspect before creating anything:
   - `git status --short --branch`
   - `git rev-parse --git-dir`
   - `git rev-parse --git-common-dir`
   - `git worktree list`
2. If already in a linked worktree, verify it is the intended task branch and continue there. Do not create a nested worktree.
3. If in the primary checkout, do not stash, reset, or switch branches to "clean it up" unless the user explicitly owns every dirty path. Preserve unrelated local work.
4. Fetch the current integration branch with `git fetch origin --prune`.
5. Create the task worktree under the repo-local convention:

```bash
git worktree add .claude/worktrees/<task-slug> -b codex/<task-slug> origin/main
```

6. In the new worktree, verify the active path and branch before editing:

```bash
pwd
git status --short --branch
git rev-parse --abbrev-ref HEAD
```

7. Run a clean baseline before edits when the task touches code. Default to `cargo test --workspace`; add `cargo fmt --all -- --check` when formatting or broad Rust edits are in scope.
8. After the worktree is ready, use the narrowest implementation skill for the touched surface.

## Baseline Rules

- A task branch normally starts from `origin/main`, not local `main`, because local `main` may be stale or checked out in another worktree.
- `.claude/worktrees/` is the established Ducktape worktree location and is ignored.
- Keep all mutations inside the feature worktree. If a file appears in the primary checkout by mistake, move the change into the feature worktree before continuing.

## Common Mistakes

| Mistake | Correction |
| --- | --- |
| Editing the primary checkout because it is already open | Create or enter the task worktree first, then edit there. |
| Branching from stale local `main` | Fetch and branch from `origin/main`. |
| Whole-worktree stash/reset to sync | Preserve dirty work unless ownership is explicit. |
| Skipping baseline tests | Run baseline before edits so failures have provenance. |
| Trusting the current shell path | Re-check `pwd` and branch before each edit batch. |
