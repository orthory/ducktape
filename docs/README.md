# docs

What lives here, and only this:

- `deploy/` — operator runbooks still executed by hand: the coordinator, sentry
  fronts, the cross-machine zero-exposure procedure, the reachability
  integration map, unified invite fronts.
- `dogfood.md` — the dogfood ceremony: ducktape develops ducktape.
- `sandbox-macos.md` — the vz sandbox stack on macOS.
- `records/` — the references code or a skill cites by path:
  `specs/capability-spec.md` (provider), `specs/indexable-spec.md` (indexer),
  `protocols/wireguard-tunnel-upgrade.md` (wireguard crate),
  `architecture/agent-collaboration-design.md` (runs, saga),
  `architecture/wasm-module-authoring.md` (the module-dev skill), plus two
  dated research notes under `research/`.

There is no docs site, no decision-record system, and no plan/spec archive.
`docs/superpowers/` is gitignored working scratch for the brainstorming and
planning skills. A document nothing cites is deleted, not archived; the rule
is `AGENTS.md` § "Docs Are Not a Record".
