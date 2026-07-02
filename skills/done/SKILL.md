---
name: done
description: Ship a completed Ducktape task by committing any in-scope leftovers, pushing the current branch, ensuring one PR, merging it into its target, updating the target locally, releasing the worktree, and leaving only a useful uncommitted handoff. Use when the user says done, ship it, merge the PR, finish the worktree, clean up after a task PR, or invokes /done.
---

# Done

Use this as the canonical Ducktape shipping workflow. `work` creates and keeps
the PR up to date; `done` resolves that PR, updates the target checkout, and
releases the task worktree.

## Resolve The PR

1. Capture the current branch:

```bash
BRANCH=$(git branch --show-current)
```

This must be the task branch carrying the PR.

2. Make the branch shippable, mirroring `work` finish steps when needed:
   - Inspect `git status --short --branch`.
   - Commit in-scope leftovers with a conventional message if needed.
   - Leave genuinely unrelated WIP uncommitted. If scope is ambiguous, ask.
   - Push with `git push -u origin "$BRANCH"`.

3. Find the open PR:

```bash
gh pr list --head "$BRANCH" --state open --json number,url,baseRefName
```

If none exists, open one against the branch's tracked upstream:

```bash
BASE=$(git rev-parse --abbrev-ref "$BRANCH@{upstream}")
BASE=${BASE#origin/}
gh pr create --base "$BASE" --head "$BRANCH" --title "<conventional title>" --body "<concise body>"
```

4. Target branch is the user's argument if provided; otherwise use the PR's
   `baseRefName`.

## Merge

1. Prefer a real merge commit:

```bash
gh pr merge "$BRANCH" --merge --delete-branch
```

2. If merge commits are disallowed, retry with `--rebase`. Use `--squash` only
   when merge and rebase are both disallowed.

3. If `--delete-branch` fails because the target branch is checked out in
   another worktree, rerun without `--delete-branch`, then delete the remote
   branch directly:

```bash
git push origin --delete "$BRANCH"
```

4. If required checks are pending or blocking, rerun the same merge command with
   `--auto`, tell the user it will merge once checks pass, and stop. Keep the
   branch and worktree intact for the deferred merge.

5. Confirm the PR merged:

```bash
gh pr view "$BRANCH" --json state,mergeCommit
```

The state must be `MERGED`.

## Update Target And Release The Worktree

1. Resolve the main checkout from Git's common dir:

```bash
COMMON_DIR=$(git rev-parse --path-format=absolute --git-common-dir)
MAIN_ROOT=$(dirname "$COMMON_DIR")
```

2. Update the target locally. If the target is checked out in the main checkout,
   pull there:

```bash
git -C "$MAIN_ROOT" pull origin "$TARGET"
```

Otherwise check out and pull the target in the current checkout.

3. Record the current worktree path before leaving it:

```bash
WT=$(git rev-parse --show-toplevel)
```

Sanity-check that `WT` is under `.claude/worktrees/` before removing it.

4. Release the session from the worktree. If the environment provides an
   explicit worktree-exit control, use it with a keep/unbind action, then run the
   removal from the main checkout. Otherwise `cd "$MAIN_ROOT"` before removal.

5. Remove the spent worktree and local branch:

```bash
git worktree remove "$WT"
git branch -d "$BRANCH" || git branch -D "$BRANCH"
```

If removal refuses because uncommitted or untracked non-ignored files remain,
stop and report. Do not force past real work. Use `--force` only when the
remaining files are confirmed ignored artifacts.

## Leave A Handoff Only When Useful

First decide whether a handoff is warranted. A handoff carries forward open or
deferred follow-ups, non-obvious resume caveats, external verification gaps, or
gotchas that the next session needs. Do not write one just to recap merged work.

When a handoff is warranted:

- Write one uncommitted `handoff-<slug>.md` at the main checkout root.
- If this session already has a handoff file, update that file instead of
  creating a second.
- Never commit it.
- Include only forward-useful details:
  - shipped PRs and one-line outcomes;
  - verification commands and results;
  - commands not run or checks only verifiable elsewhere;
  - open or deferred follow-ups;
  - resume commands, key files, and gotchas.
- Convert relative dates to absolute dates.

## Report

Report the PR URL, merge result, target's new local state, whether the worktree
was released or kept, and either the handoff filename or that no handoff was
needed.
