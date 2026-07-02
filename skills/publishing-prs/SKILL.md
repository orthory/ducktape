---
name: publishing-prs
description: Use when turning local Ducktape work into a pull request targeting main, pushing a worktree branch, preparing PR evidence, packaging docs/skill changes, or handing a branch to the review/merge loop.
---

# Publishing PRs

## Overview

Package Ducktape work into a clean PR to `main` without dragging in unrelated local changes. This skill ends when the PR is open and evidence is recorded; use `reviewing-prs` for review loops and merging.

## Workflow

1. Confirm you are in the feature worktree and not the primary dirty checkout:

```bash
pwd
git status --short --branch
git diff --name-only
git diff --cached --name-only
```

2. Fetch before publishing: `git fetch origin --prune`.
3. Confirm the base is `main`. For issue-linked work, use explicit issue references only when the user wants them in the PR body.
4. Review the diff yourself before staging. Stage exact files; never use broad staging from a dirty checkout.
5. Run verification proportional to the touched surface:

| Surface | Minimum evidence |
| --- | --- |
| Docs only | Placeholder scan plus `git diff --check origin/main...HEAD` |
| Repo skills | `quick_validate.py` for each touched skill, direct `agents/openai.yaml` `$skill-name` check, placeholder scan, and `git diff --check origin/main...HEAD` |
| Rust code | `cargo test --workspace`, `cargo fmt --all -- --check`, and `git diff --check` |

Use the repo validator from the skill-creator bundle for skill folders:

```bash
uv run --with pyyaml python /Users/eddy/.codex/skills/.system/skill-creator/scripts/quick_validate.py skills/<skill-name>
```

6. Commit only after verification evidence is known. If commit signing blocks a local commit and the repo has no stricter instruction, use `--no-gpg-sign` and record that fact in the PR notes.
7. Push the branch and create a draft PR targeting `main`. Use a quoted body file for text containing `$skill` names.
8. Include in the PR body: summary, validation commands, changed skill names, and residual risk.
9. If the user requested review or merge, switch to `reviewing-prs` after the PR exists.
10. Clean up local worktrees only after merge is confirmed and the worktree has no unrelated changes.

## PR Body Checklist

- Target branch is `main`.
- Summary names the user-visible or workflow-visible change.
- Validation lists exact commands and outcomes, not generic "tests passed".
- Draft status is explicit when review is expected.
- Issue references use `Refs #...` unless closure is intentional.

## Common Mistakes

| Mistake | Correction |
| --- | --- |
| Creating a PR from the wrong base | Inspect the PR target and branch ancestry before publishing. |
| Letting shell expansion mangle `$skill` in PR text | Write the body to a quoted temp file, then pass `--body-file`. |
| Relying on `quick_validate.py` alone for skill metadata | Also check `agents/openai.yaml` contains `$skill-name` in `default_prompt`. |
| Staging unrelated primary-checkout changes | Publish from the isolated feature worktree and stage exact paths. |
| Treating PR creation as merge readiness | Use `reviewing-prs` for review convergence and merge decisions. |
