# Ducktape Docs

This directory is the Nimbus/Astro project for the Ducktape documentation.
Routed content lives under `src/content/docs`; the custom landing page lives at
`src/pages/index.mdx`. Non-page records remain outside the content collection,
so Nimbus never publishes them accidentally.

## Reader Tracks

- `src/content/docs/en/human` and `src/content/docs/ko/human` are for human readers.
- `src/content/docs/en/agent` and `src/content/docs/ko/agent` are for coding agents.

Human pages explain product shape, architecture, operations, and status without
assuming the reader is about to edit code. Agent pages are tighter implementation
maps: invariants, verification commands, module boundaries, and open work that a
coding agent can use before touching files.

Each reader track should stay structurally aligned across English and Korean.
When adding a routed page, add all four language/reader variants unless the
content is intentionally reader-specific.

## Where Docs Belong

- Put maintained reader documentation under `docs/src/content/docs` so Nimbus
  can route, build, and index it.
- Put accepted decision records under `docs/adr` when the decision boundary
  should outlive an implementation branch.
- Put maintained non-page records that are not ADRs or operator runbooks under
  `docs/records`; keep them listed from the `reference/design-records` pages.
- Keep operator runbooks as standalone Markdown only when an operator still
  executes them directly.
- Keep `docs/superpowers` small and reviewed: active design records, approved
  specs, and execution plans may stay there until durable facts are folded into
  Nimbus, ADRs, maintained runbooks, tests, or code comments. Do not prune a
  document just because it lives under `docs/superpowers`; prune it only after a
  content review identifies its replacement owner or shows it is obsolete.

The Nimbus reference page `reference/design-records` in each track lists the
non-page records that are still maintained and explains the current pruning
policy.

## Commands

```sh
bun install
bun run docs:check
bun run typecheck
bun run lint:docs
bun run dev
```

`bun run docs:check` runs the structure check before the Nimbus static build.
`DOCS_SITE_URL` sets the canonical, Open Graph, and sitemap origin; local builds
default to `http://localhost:4321`. Set it to the externally reachable origin for
every non-local preview or deployment (for example,
`DOCS_SITE_URL=https://docs.example.com bun run build`). Use the three checks
before opening a docs PR.
