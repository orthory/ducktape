---
name: work
description: Start or resume a Ducktape task in a dedicated worktree from the current branch's latest upstream, work with planning and root-cause discipline, then commit, push, and maintain exactly one PR. Use when the user asks to start feature, fix, docs, or process work; mentions /work, worktrees, branch setup, PR upkeep, or wants a task carried to PR-ready state without merging.
---

# Work

Use this as the standard Ducktape task workflow.

`work` starts or resumes an isolated worktree, keeps the task branch tied to the
branch it forked from, does the work deliberately, then pushes and maintains one
open PR. It never merges; use `done` for that.

## Inputs

- Branch name: take it from the user's request. If absent, set up nothing and ask
  for it.
- Task description: use any text after the branch name. If absent, set up or
  resume the worktree, then ask what the task is.
- New branch: use the provided branch name verbatim.
- Worktree directory: replace `/` with `+` in the branch name and place it under
  the main checkout's `.claude/worktrees/`.

## Step 0: Load Binding Project Rules

Before setup, planning, or code, read `.project/work.md` from the main checkout
root if it exists. Resolve the main root from the parent of the absolute Git
common dir so this also works from linked worktrees:

```bash
COMMON_DIR=$(git rev-parse --path-format=absolute --git-common-dir)
MAIN_ROOT=$(dirname "$COMMON_DIR")
test -f "$MAIN_ROOT/.project/work.md" && cat "$MAIN_ROOT/.project/work.md"
```

Rules in that file are binding for the whole `work` session and override this
skill where they conflict. Re-read it at the start of each resumed increment.

## Setup Or Resume The Worktree

1. Capture the invoking branch immediately:

```bash
CURRENT=$(git branch --show-current)
COMMON_DIR=$(git rev-parse --path-format=absolute --git-common-dir)
MAIN_ROOT=$(dirname "$COMMON_DIR")
BRANCH="<user-provided-branch>"
WT_DIR="$MAIN_ROOT/.claude/worktrees/${BRANCH//\//+}"
```

2. If `git worktree list` already shows `WT_DIR` on `BRANCH`, enter that
   worktree and continue at "Work The Task". Do not fetch, rebranch, or rebase
   work already in progress.

3. If the existing worktree has zero changes and no commits ahead of its
   upstream, treat it as stale or non-effective and leave it instead of building
   on a dead branch.

4. For a new task, fetch only the invoking branch's upstream ref:

```bash
git fetch origin "$CURRENT"
git rev-parse --verify --quiet "refs/remotes/origin/$CURRENT"
```

If the remote ref is absent, stop and ask whether to push the base branch first
or use a different explicit base. Do not silently fall back to local state or
trunk.

5. Check collisions before creating anything:
   - If `BRANCH` exists without a matching worktree, stop and report it.
   - If `WT_DIR` exists but is not this branch's registered worktree, stop and
     report it.

6. Create and enter the worktree:

```bash
git worktree add -b "$BRANCH" "$WT_DIR" "origin/$CURRENT"
git -C "$WT_DIR" branch --set-upstream-to="origin/$CURRENT" "$BRANCH"
cd "$WT_DIR"
git status --short --branch
```

After entering, run all task commands from the worktree. If the environment has
an explicit "enter worktree" control, use it; otherwise set command working
directories to `WT_DIR`.

## Work The Task

1. Build understanding before solutions. Inspect the current implementation,
   summarize the crux, and resolve real ambiguity with the user before guessing.
   Do not open with an options funnel, solution pitch, or pros/cons list while
   the problem is still being understood.
2. Plan before code. Name every file you expect to touch and why. Include build
   or deploy routine changes if the task shifts dependencies, assets, bundling,
   or config. Surface the plan for agreement before editing.
3. Fix root causes, not symptoms. Keep the blast radius contained, but do not
   shrink the problem to make the patch easier. If the real fix exposes
   misplaced code or would fan out across many files, stop and surface the
   architecture fork before coding.
4. Do not hide broken invariants with unsafe fallbacks, swallowed errors,
   non-null assertions, `as any`, or default values unless the fallback is truly
   correct behavior. When it is correct, add a one-line comment explaining why.
5. Keep controllers as orchestrators and work functions as pure as practical:
   inputs in, results out, collaborators passed by dependency injection where it
   improves testability.
6. Before TypeScript edits, use the repository's TypeScript style skill if one
   is available. Prefer pure functions, expression-oriented code, `const` over
   `let` rebinding, result envelopes at boundaries, async composition over
   `try/catch` control flow, and exhaustive `switch` on discriminants.
7. Surface real architectural forks before coding. Recommend the root-cause fix
   with the most contained blast radius, but let the user choose when the fork
   is meaningful.
8. Drive open questions to zero up front. Skip heavy planning only when the task
   is clearly trivial.
9. Name and get agreement on new files before creating them unless the user has
   already explicitly requested those exact files.

## Finish To One PR

1. Run `blast` or produce the equivalent blast-radius analysis for the current
   change set.
2. Run verification proportional to the touched surface. For repo skills, run
   `quick_validate.py` for each touched skill, check each `agents/openai.yaml`
   default prompt mentions `$skill-name`, scan for placeholders, and run
   `git diff --check`.
3. Commit the task increment from the dedicated worktree. Because the worktree is
   task-scoped, `git add -A` is acceptable after confirming `git status` contains
   only in-scope files. Use a conventional commit message and any
   assistant-specific co-author trailer required by the environment. If there is
   nothing to commit, skip straight to push; a previous increment may already
   carry the branch state.
4. Push the branch:

```bash
git push -u origin "$BRANCH"
```

5. Ensure exactly one open PR for the branch:

```bash
gh pr list --head "$BRANCH" --state open --json url --jq '.[0].url'
```

If none exists, open one against the tracked upstream, not trunk:

```bash
BASE=$(git rev-parse --abbrev-ref "$BRANCH@{upstream}")
BASE=${BASE#origin/}
gh pr create --base "$BASE" --head "$BRANCH" --title "<conventional title>" --body "<concise body>"
```

Use a quoted body file when the PR text contains literal `$skill` names.

6. Stop before merging. `work` builds and maintains the PR; `done` merges it.

## Memory Hygiene

Default to writing no memory for routine work. Use memory only for durable facts
that are not derivable from code or git history: lasting user preferences,
external pointers, active decisions, or true resume caveats. Do not create a
memory per PR, and remove stale project memories when their thread closes.
