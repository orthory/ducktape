# Recipe-backed UI QA — one recipe, two lanes (iced_test + headless fleet)

**Status:** approved direction (follows the iced_test bridge, #643).

## Goal

Declarative QA recipes that run identically against both QA lanes:

- **In-process lane** (default, CI-shaped): `cargo test` interprets recipes
  through `iced_test::Simulator` + the shell's own `update` loop. No display,
  no node, no side effects.
- **Fleet lane** (integration-shaped): `ops/iced-fleet` boots N isolated
  headless app instances and executes recipes over the agent bridge against
  the live binary.

One scenario authored once (`qa/recipes/*.json`) exercises UI logic fast in
CI and the real app under the fleet — same steps, same semantic addressing
(`role` + `name`, the `sem()` layer both lanes already share).

## Recipe format

JSON, serde-typed in `iced-agent-plugin` (`plugin/src/recipe.rs`) — a thin
composition over the existing bridge vocabulary, no new concepts:

```json
{
  "name": "nav-smoke",
  "preset": "ui-demo",
  "steps": [
    { "click":  { "role": "button", "name": "Chat" } },
    { "type":   "hello" },
    { "press":  { "key": "k", "mods": ["ctrl"] } },
    { "intent": { "section": { "name": "operator" } } },
    { "expect": { "state_path": { "path": "screen", "equals": "chat" } } },
    { "expect": { "node": { "role": "tab", "name": "User", "exists": true } } },
    { "wait":   { "cond": { "node": { "role": "button", "name": "Save", "exists": true } }, "timeout_ms": 5000 } }
  ]
}
```

- `Step` enum (serde, snake_case, externally tagged): `Click{role,name}`,
  `Type(String)`, `Press{key,mods}`, `Intent(Intent)`, `Expect(Cond)`,
  `Wait{cond,timeout_ms}`. `Cond`/`Intent` are the existing protocol types.
- Addressing is semantic (`role`+`name`), never `@ref` — refs are runtime
  handles; runners resolve `find → ref → click` internally.
- `preset` names the boot state the recipe assumes (`ui-demo` today). The
  fleet boots instances with `DUCKTAPE_PRESET=<preset>`; the in-process lane
  boots the same preset fixture. A recipe whose preset an app doesn't define
  fails loudly, not silently.
- `.ice` is deliberately not the format: record/replay was rejected, and
  `.ice` expectations are text-only — ours assert state paths and semantic
  nodes.

## Components

### iced-agent-browser (generic, public repo)

1. `plugin/src/recipe.rs` — `Recipe`/`Step` types + parse, with unit tests.
   Dev-gated like the rest of the crate.
2. `plugin/bin/iced-agent.ts` gains `run <recipe.json> [--app ...]`: executes
   steps over the live bridge (click = `find{role,name}` → `click @ref`;
   `expect` = single `expect` call; `wait` passes through), prints a per-step
   report, exit code = failed steps. Protocol unchanged — the runner is pure
   client-side composition.

### ducktape (app-specific)

3. `app/src-iced/src/shell/qa.rs` (dev-gated, test-only helpers) — the
   in-process interpreter: loop `view(state) → Simulator → apply step →
   drain messages → update(state, message)`; `Click` resolves via
   `by::role`; `Expect(StatePath)` evaluates against the same curated
   projection `agent_wire` serves (extracted into a shared fn so the two
   lanes read identical state). Limitation, documented: `update` Tasks don't
   run (no runtime) — recipes for this lane assert UI-logic, not async
   effects.
4. `qa/recipes/` — first recipes: `nav-smoke` (each module button → screen
   state), `search-palette` (ctrl+k open/escape close), `theme-toggle`,
   `section-switch`. All `preset: ui-demo`.
5. `tests` — a `cargo test` entry that globs `qa/recipes/*.json` and runs
   every recipe whose preset the shell defines through the in-process
   interpreter. Adding a recipe file = adding a CI test, no Rust edits.
6. `ops/iced-fleet` (bun, mirrors the old fleet's lessons):
   - `up <n|ids>` — per instance: own Xvfb display, `dbus-run-session`,
     isolated short-path `HOME`/`XDG_RUNTIME_DIR` under
     `~/.cache/iced-fleet/<id>`, `DUCKTAPE_PRESET` from flag, pidfiles for
     Xvfb/app; waits for the instance's `endpoint.json`.
   - `down [id]` — kills only pids recorded in pidfiles after verifying
     `/proc/<pid>/exe` is our binary (never `pkill -f`), removes state.
   - `status` — id, pid, endpoint, uptime.
   - `run <recipes...>` — fans recipes across up instances (`iced-agent run`
     with the instance's `XDG_RUNTIME_DIR`), aggregates a pass/fail table,
     nonzero exit on any failure.
7. `make ui-qa` — in-process lane (`cargo test -p ducktape-iced qa_recipes`)
   then fleet lane (`iced-fleet up 2 --preset ui-demo && run qa/recipes/*.json
   && down`). `skills/qa` gains the recipe/fleet section.

## Security / hygiene

- Everything dev-gated exactly like the agent stack; the fleet refuses a
  release binary (no endpoint would appear anyway).
- Fleet state under `~/.cache/iced-fleet` (short paths for wayland sockets;
  no Cargo outputs there — nothing big).
- Teardown by verified pidfile only; `ops/worktree-clean.sh` rules untouched.

## Verification

- Public repo: recipe unit tests + runner probe against a mock bridge.
- ducktape: `cargo test -p ducktape-iced` includes the recipe glob test (all
  shipped recipes green in-process); live: `make ui-qa` runs both lanes green
  on this box (fleet of 2, all recipes pass on real binaries); gates + release
  grep audit as usual.
