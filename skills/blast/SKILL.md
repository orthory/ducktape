---
name: blast
description: Produce a terse blast-radius analysis of a Ducktape change set with a category, two-line summary, and per-file line-ranged bullets. Use when the user asks for blast range, blast radius, change impact, /blast, or a concise analysis of current uncommitted, staged, branch, or last-commit changes.
---

# Blast

Summarize the blast radius of a change set in a fixed, scannable format: what
category of change it is, two lines on what it does, then one line per touched
file with exact line ranges. This is analysis only; do not run builds.

## Scope

Resolve scope in this order:

1. Explicit user target: a ref such as `HEAD` or `main`, `staged`, a path, or
   `vs <branch>`.
2. Otherwise, uncommitted working-tree changes, including staged, unstaged, and
   untracked files.
3. If the working tree is clean, the last commit (`HEAD~1..HEAD`).
4. If on a feature branch, prefer the branch diff from its merge-base with the
   default branch.

If some changes are obviously unrelated or pre-existing, exclude them once under
an `Excluded:` line.

## Gather Precise Line Ranges

1. Run `git diff -U0 <scope> -- <file>` for each changed file.
2. Parse each `@@ -old +new @@` hunk header and use the new-side range. A hunk
   with `+A,B` spans `A` through `A+B-1`; omitted `B` means a single line.
3. Consolidate adjacent or overlapping ranges per file.
4. For untracked new files, cite the whole file (`1-N`) or the meaningful span.
5. Read each changed region before describing it. Do not infer impact from the
   filename alone.

## Output Format

```text
**<Category> - <short noun phrase>**

<Two lines max: what the change does, plus the key blast-containment fact if there is one.>

- `path:Lx-Ly`: <one line describing impact at that site>
- `path:Lx`: <one line describing impact at that site>
```

- Category is one of `Feature`, `Fix`, `Refactor`, `Perf`, `Chore`, `Test`, or
  `Docs`.
- When the diff spans more than about five files, group bullets under short
  sub-area labels such as `Data model`, `Persistence`, `View`, `UI`, or `Tests`.
- Collapse many similar test-fixture edits into one bullet spanning the files.
- Paths are repo-relative and line numbers are post-change line numbers.

## Rules

- Keep one line per bullet.
- Describe impact, not mechanics.
- Do not run tests or builds.
- If verification status is already known from the session, state it in one
  line.
- End with `Unverified:` when behavior cannot be confirmed by the available
  typecheck/tests, such as runtime gestures, network paths, or UI layout.
- Note deliberately untouched boundaries when they materially contain blast
  radius, such as public API, schema, version, or server contract.
- Keep the result tight.
