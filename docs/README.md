# Ducktape Docs

This directory is the Vocs project for the Ducktape documentation.

The Vocs config intentionally sets `srcDir: "."`, so pages live at
`docs/pages`. Do not create `docs/docs`.

The page tree is split by reader and language:

- `pages/en/human` and `pages/ko/human` are for human readers.
- `pages/en/agent` and `pages/ko/agent` are for coding agents.

## Commands

```sh
bun install
bun run docs:check
bun run dev
```
