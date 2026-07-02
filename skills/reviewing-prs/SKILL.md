---
name: reviewing-prs
description: Use when reviewing a Ducktape pull request, deciding whether review feedback is acceptable, addressing PR review findings, re-running review loops, or merging a PR into `main`.
---

# Reviewing PRs

Use this for PRs that may merge into `main`. The goal is not one review pass; the goal is a review loop that keeps going until the remaining feedback is either necessary and addressed or explicitly rejected with evidence.

If you are the clean-context reviewer dispatched by this skill, review directly and do not spawn another reviewer. The subagent loop belongs to the coordinator who owns the PR merge decision.

## Review Loop

1. Confirm the PR target is `main`, the branch is current enough to review, and CI/checks are visible. Do not merge a PR aimed at the wrong base.
2. Start at least one clean-context subagent reviewer. Give it only the PR number, base branch, repository, and review mandate. Do not leak your intended answer or previous conclusions.
3. Inspect the PR diff yourself while the subagent works. Focus on correctness, data loss, contract boundaries, migrations, security/privacy, tests, and user-visible regressions.
4. Classify every reviewer claim:
   - `accept`: real issue or missing proof. Fix it or require the author to fix it.
   - `reject`: false, out of scope, or lower-value than the requested change. Record the evidence.
   - `defer`: valid but not merge-blocking. Capture a follow-up only when it is useful.
5. Address all accepted claims. Use the narrowest matching repo skill before editing.
6. Run relevant verification after fixes. At minimum use `git diff --check origin/main...HEAD`. For repo-skill changes, also run the `publishing-prs` repo-skill validation gate: `quick_validate.py` for each touched skill, direct `agents/openai.yaml` `$skill-name` checks, and a placeholder scan. Add `cargo test --workspace` and `cargo fmt --all -- --check` when the touched surface warrants them or when preparing to merge.
7. Re-run a clean-context review after fixes. The second reviewer should see the updated PR, not your classification notes.
8. Repeat fix -> verify -> re-review until review output contains only necessary unresolved items. A review that keeps finding acceptable defects is not ready.
9. If the same-account GitHub approval restriction applies, leave a review-pass comment with the evidence instead of pretending formal approval happened.
10. Merge only after accepted findings are resolved, necessary checks pass, and the PR still targets `main`. Prefer squash merge for normal feature/fix PRs unless the PR history needs preserving.

## Clean-Context Reviewer Brief

Use a self-contained brief like this:

```text
Review PR #<number> in <owner>/<repo> targeting main. Start from the PR diff and current files only. Do not rely on the parent agent's conclusions. You are the clean-context reviewer: review directly and do not spawn another reviewer. Look for merge-blocking correctness, data-loss, boundary-contract, migration, security/privacy, and test-coverage issues. Return findings with file/line evidence, severity, and the verification you expect before merge. If there are no blocking findings, say so explicitly and list residual risks.
```

## Merge Gate

Before merging, require evidence for each item:

| Gate | Evidence |
| --- | --- |
| Correct base | PR target is `main` |
| Accepted feedback resolved | Diff or author commits address each accepted claim |
| Re-review converged | Latest clean-context review has no new accepted blockers |
| Verification | Relevant command output and CI/check status; repo-skill PRs include the `publishing-prs` repo-skill validation gate plus `git diff --check origin/main...HEAD` |

After merge, record the merge or squash commit, fetch `origin/main`, verify the PR commit is in `main`, delete the remote feature branch when appropriate, and clean up local worktrees only after confirming no unrelated changes live there.

## Common Mistakes

| Mistake | Correction |
| --- | --- |
| One quick subagent review, then merge | Loop until accepted claims are resolved and re-review converges. |
| Passing the subagent your diagnosis | Give a clean-context brief so the review is independent. |
| Treating every claim as true | Classify claims; reject false or out-of-scope feedback with evidence. |
| Letting green CI replace review | CI is one signal. Still inspect the diff and boundary risks. |
| Merging into the wrong branch by habit | This repo's feature PR target is `main` unless the user explicitly says otherwise. |
