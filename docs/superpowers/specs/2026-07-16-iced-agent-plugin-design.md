# iced-agent-plugin — agent driver for the native iced shell

**Status:** approved design (user-selected substrate: own the event loop — forked
`iced_winit` — with real OS AccessKit).

## Goal

Full `tauri-agent` parity for the iced desktop shell (`app/src-iced`,
`ducktape-iced`): semantic tree, role/name find with `@ref` handles, input
drive (click/type/press/hover/drag/scroll), screenshots, log tap, state
queries, wait/expect, a CLI and a stdio MCP server, headless operation under
Xvfb, and per-instance fleet isolation — **plus** real OS accessibility
(AT-SPI / UIA / macOS AX) fed from the same tree.

The Tauri stack got its semantic substrate for free from the DOM. iced renders
`view()` straight to wgpu/tiny-skia pixels and mainline iced 0.14 ships **zero**
accessibility (no accesskit anywhere in its graph), so the semantic layer must
be built. This is the deliberate, heaviest variant: we own the seams in the
windowing shell itself.

## Non-goals

- Replacing `iced_winit` with a bespoke shell (rewriting iced's runtime half).
- Forking `iced_widget` to make built-in widgets emit a11y nodes; tree content
  comes from app-side instrumentation.
- Release-build automation. Everything here is dev-only.
- DOM tooling for the CEF Browser pane — Chromium already speaks CDP; we only
  expose it.

## Architecture

Five units. One semantic tree, two consumers (bridge + OS a11y).

```
view code ──sem(role,name)──► semantic registry ─┐
                                                 ├─► AccessKit tree per window
advanced::Operation walk ──bounds by id──────────┘        │            │
                                                     fork adapter   bridge
                                                     (AT-SPI/UIA/AX) (tree/find/@ref)
input drive:  @ref → bounds → synthetic winit events → fork inject → real iced path
              intents → curated enum → Message injection → update()
              AccessKit ActionRequest → same handler
```

### 1. `third_party/iced-winit` — vendored fork (the loop)

Vendored copy of crates.io `iced_winit 0.14.0`, applied graph-wide via
workspace `[patch.crates-io]` (version number stays `0.14.0`; the `iced`
facade resolves to it transparently). Every modification is marked
`// AGENT SEAM` and kept upstream-shaped — iced needs a11y eventually and this
diff is the PR candidate. Base version documented in the fork's README;
in-tree vendor now, promotable to a proper GitHub fork + submodule later
without design change.

Exactly three seams, all compiled under
`#[cfg(all(feature = "agent", debug_assertions))]` — the app enables the
`agent` feature unconditionally, and `debug_assertions` guarantees a release
binary compiles none of it (Cargo features are not profile-dependent; the
`cfg` pair is, matching the tauri-plugin-agent precedent):

1. **AccessKit adapter per window.** On window creation, attach an
   `accesskit_winit::Adapter` (version pinned at spike time against
   winit `0.30.13`). The event loop forwards winit window events to the
   adapter; `ActionRequest`s are drained onto a channel the app owns.
2. **Tree push.** `agent::set_tree(window_id, TreeUpdate)` — the app pushes
   tree updates into the adapter whenever its snapshot changes.
3. **Synthetic event injection.** `agent::inject(window_id, WindowEvent)` —
   synthetic `CursorMoved` / `MouseInput` / `KeyboardInput` / `MouseWheel`
   enter the exact path real input takes (iced hit-testing, hover, focus,
   drag). Display-server-independent, headless-clean, zero per-platform input
   code. This seam is the reason the fork exists: `window::run(id, |w| ...)`
   could have attached AccessKit sidecar-style, but nothing outside the loop
   can feed events through iced's real input path.

Seam plumbing between app and fork: a small `agent` module in the fork
exposing handle registration (channels keyed by `window::Id`), dev-gated.

### 2. Semantic layer (the tree source)

- **Tagging:** one thin wrapper widget — `sem(role, name, element)` with
  builder extras (`.description()`, `.value()`, `.disabled()`, actions). View
  code wraps meaningful widgets as it builds them. This is the long tail of
  work (~20k LOC of views) and is swept incrementally (P4); untagged interactive
  regions still appear as bounds-only nodes so gaps are visible, not silent.
- **Geometry:** a custom `advanced::widget::Operation` walks the live widget
  tree per snapshot, collecting each `sem` node's post-layout bounds and the
  focus/scroll state iced already tracks.
- **Output format is AccessKit `Node` itself.** No intermediate schema. The
  same per-window tree is (a) pushed through the fork adapter — real screen
  readers see it — and (b) served by the bridge for `tree`/`find`. One source,
  two consumers; the `iced_a11y` tool dumps what was actually pushed, so QA
  continuously verifies the accessibility surface.
- **Multi-window:** main / huddle / tray each get an adapter and a tree; every
  tool takes a `window` parameter defaulting to main.

### 3. Drive

- **Primary:** `find` → `@ref` → bounds center → synthetic winit events via
  the fork. Same fidelity as a user; works under bare Xvfb with no WM and no
  XTEST/uinput/CGEvent.
- **Semantic intents:** a curated serde enum (Navigate(Route), Section,
  ToggleTheme, …) → injected as `Message` into `update()`. Deliberately *not*
  the full `Message` enum — the bridge is a trust boundary; only reviewed,
  side-effect-understood intents are exposed.
- **AccessKit actions:** adapter `ActionRequest`s (Click/Focus/SetValue) route
  into the same dispatch as bridge commands — screen-reader operation and agent
  operation exercise identical code.
- `@ref` validity follows the tauri-agent convention: refs are valid until the
  next `tree`/`find` snapshot.

### 4. Bridge, discovery, tool surface

- **In-app module** (inside the `iced-agent-plugin` crate, app depends on it
  dev-only): loopback TCP, JSON-lines protocol, one command per line,
  request/response with an id; a `stream` mode for log/tree following.
- **Discovery:** publishes
  `${XDG_RUNTIME_DIR|TMPDIR|TMP}/iced-agent/com.ducktape.app/endpoint.json`
  (`{host, port, pid, windows[]}`), same shape and lifecycle as the old
  tauri-agent registry so fleet-style per-instance isolation
  (`XDG_RUNTIME_DIR` per instance) carries over unchanged.
- **CLI + MCP:** bun/TS (`bin/iced-agent.ts`, `bin/iced-agent-mcp.ts`) speaking
  the JSON protocol to the bridge; stdio MCP server exposes `iced_*` tools.

| tauri-agent | iced-agent | implementation |
|---|---|---|
| `tree` / `find` / `@ref` | `iced_tree` / `iced_find` | semantic tree |
| `click/type/press/hover/drag/scroll` | same | synthetic winit events |
| `eval` | `iced_state` + `iced_intent` | Shell state projection (path query) / curated Message injection |
| `shot` (DOM-SVG) | `iced_shot` (PNG) | built-in `window::screenshot`, WM-free |
| `logs` | `iced_logs` | tracing ring layer |
| `wait` / `expect` / `windows` | same | tree/state polling |
| — | `iced_a11y` | dump the AccessKit tree actually pushed to the OS adapter |

- **State:** `iced_state` serves serde projections of curated `Shell` subtrees
  by path (screen, route, workspace, notifications…). Curated for the same
  trust-boundary reason as intents; never serializes secrets/key material.
- **Logs:** a ring-buffer `tracing` layer inside the plugin (the app already
  installs `tracing_subscriber`); `iced_logs` reads/clears it. Ring follows the
  logging doctrine (bounded, no capability paths).

### 5. CEF Browser pane

Dev builds pass `--remote-debugging-port=0` to CEF and publish the resolved
CDP URL in `endpoint.json`. Anything inside the Browser pane is driven over
CDP directly; the bridge does not reimplement DOM tooling.

## Security / dev-only stance

- Fork seams and all app-side agent wiring:
  `#[cfg(all(feature = "agent", debug_assertions))]`; release binaries compile
  none of it regardless of enabled features.
- Bridge binds loopback only, endpoint file in the user-private runtime dir
  (`chmod 700` convention from fleet).
- Curated intents + curated state projections; no arbitrary code execution
  surface (there is no `eval` analog by design).
- Never expose capability-bearing URLs or key material through `iced_state`,
  `iced_logs`, or the tree.

## Risks

- **Fork maintenance** — accepted explicitly. Mitigation: 3 marked seams,
  minimal diff, iced pinned `=0.14.0`, upstream-shaped for a future PR.
- **accesskit_winit ↔ winit 0.30.13 version fit** — resolved first thing in
  the P0 spike; if the winit adapter mismatches, fall back to per-platform
  accesskit adapters inside the same fork seam (same design, more glue).
- **Instrumentation coverage** — incremental by design; bounds-only fallback
  keeps gaps visible.
- **Custom widgets** (terminal grid, browser chrome, huddle) need hand-written
  `sem` mapping — scheduled in P4, not blocking the core.

## Phasing

- **P0 spike (the only technical-risk gate):** vendor fork + adapter + stub
  tree + injection. Prove under Xvfb: AT-SPI exposes the stub nodes, and a
  synthetic click flips real app state.
- **P1:** `sem()` + Operation walk + registry; bridge with `tree`/`find`;
  instrument shell chrome + two screens.
- **P2:** full drive (synthetic events, intents, AccessKit action routing).
- **P3:** `shot/logs/state/wait/expect`, CLI + MCP, endpoint registry,
  `.mcp.json`.
- **P4:** instrumentation sweep across all screens, CEF CDP exposure,
  `skills/qa` rewrite from manual checklist to agent-driven flow.

## Verification

- Per-crate gates: `cargo clippy -p iced-agent-plugin --tests --no-deps`,
  `cargo clippy -p ducktape-iced --tests --no-deps`, `cargo test -p ducktape-iced`.
- Live e2e under Xvfb: endpoint appears → `iced_tree` returns the shell →
  `iced_find --role button` → `iced_click @ref` changes `iced_state` →
  `iced_shot` writes a non-empty PNG → `iced_a11y` shows the same nodes AT-SPI
  sees (probe via `busctl --user` / atspi client).
- Fork seam audit: release-profile build compiles zero agent code
  (`cargo tree -e features` / grep gate).
