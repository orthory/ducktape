# Ducktape Docs

This directory is the Vocs project for the Ducktape documentation.

The Vocs config intentionally sets `srcDir: "."`, so pages live at
`docs/pages`. Do not create `docs/docs`.

## Reader Tracks

- `pages/en/human` and `pages/ko/human` are for human readers.
- `pages/en/agent` and `pages/ko/agent` are for coding agents.

Human pages explain product shape, architecture, operations, and status without
assuming the reader is about to edit code. Agent pages are tighter implementation
maps: invariants, verification commands, module boundaries, and open work that a
coding agent can use before touching files.

Each reader track should stay structurally aligned across English and Korean.
When adding a routed page, add all four language/reader variants unless the
content is intentionally reader-specific.

## Where Docs Belong

- Put maintained reader documentation under `docs/pages` so Vocs can route,
  build, and index it.
- Put accepted decision records under `docs/adr` when the decision boundary
  should outlive an implementation branch.
- Keep operator runbooks as standalone Markdown only when an operator still
  executes them directly.
- Do not keep temporary planning archives or speculative design snapshots in
  this repo. Once useful facts ship, fold them into Vocs, ADRs, maintained
  runbooks, tests, or code comments and prune the source planning document.

The Vocs reference page `reference/design-records` in each track lists the
non-page records that are still maintained and explains the current pruning
policy.

## Commands

```sh
bun install
bun run docs:check
bun run dev
```

`bun run docs:check` runs the structure check before the Vocs static build. Use
it before opening a docs PR.
