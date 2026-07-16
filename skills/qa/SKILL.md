---
name: qa
description: Verify the real native Ducktape Iced desktop, its managed node, and its isolated CEF Browser pane — agent-driven through the iced-agent bridge (tree/find/click/type/state/shot), plus package and lifecycle checks. Use for UI/design QA, lifecycle regressions, package checks, and native smoke testing.
---

# Native Iced QA

Use the packaged native app, not the React dev server, for desktop claims. Iced
owns the interface and node lifecycle; CEF exists only inside Browser. Dev
builds carry the **iced-agent bridge**: a loopback driver with a semantic tree,
synthetic input through iced's real event path, screenshots, a log ring, and a
curated state projection — plus real OS AccessKit fed from the same tree.

## Baseline gates

```bash
cargo test -p ducktape-iced
cargo test -p iced-agent-plugin
cd app
bun install --frozen-lockfile
bun run typecheck
bun run test
bun run build
```

Build or run the matching native app from the repository root:

```bash
make dev   # interactive debug app (agent bridge on)
make app   # release package in target/release/bundle (agent bridge compiled out)
```

## What's wired

| Piece | Where |
|---|---|
| Fork seams (AccessKit adapter, tree push, event injection) | `third_party/iced-agent-browser/iced-winit` (submodule, `[patch.crates-io]`), feature `agent`, marked `// AGENT SEAM` |
| Semantic layer + bridge + tools | `third_party/iced-agent-browser/plugin` (`sem()` tags, Operation collector, loopback JSON-lines server) |
| App wiring (150 ms snapshot loop, intents, a11y-action routing) | `app/src-iced/src/shell/agent_wire.rs` |
| CLI | `ops/iced-agent <cmd>` (bun shim) |
| MCP | `.mcp.json` server `iced-agent`, tools `iced_*` |
| Discovery | `${XDG_RUNTIME_DIR|TMPDIR|/tmp}/iced-agent/com.ducktape.app/endpoint.json` (`cdp` field = Browser-pane CDP URL in dev) |

Everything is dev-only: the seams and wiring compile under
`all(feature = "agent", debug_assertions)`; a release binary registers nothing
and binds nothing.

## Drive the app

```bash
ops/iced-agent tree                          # semantic tree (default --window main)
ops/iced-agent find --role button --name Forge
ops/iced-agent click @3                      # @refs valid until the next tree/find
ops/iced-agent type "hello"                  # per-key into the focused widget
ops/iced-agent press --key k --mod ctrl     # chords; named keys: enter/tab/escape/…
ops/iced-agent state --path section          # curated Shell projection
ops/iced-agent intent '{"navigate":{"url":"chat"}}'
ops/iced-agent shot --out /tmp/app.png       # WM-free PNG via window::screenshot
ops/iced-agent logs                          # in-app tracing ring (4096 lines)
ops/iced-agent wait --role button --name Settings --timeout 5000
ops/iced-agent a11y                          # dump the tree actually pushed to the OS
ops/iced-agent windows                       # main / huddle / tray
```

Or the MCP tools (`iced_tree`, `iced_find`, `iced_click`, …) from Claude Code.
Assertions go over `tree`/`find`/`state` — display-independent; `shot` is for
evidence, not assertions. Browser-pane content is Chromium: read `cdp` from
`endpoint.json` and drive it over CDP directly.

Per-instance isolation: the endpoint lives under `XDG_RUNTIME_DIR`, so parallel
apps isolate by giving each instance its own runtime dir and exporting it
before calling the CLI.

## Headless bring-up (Linux)

```bash
Xvfb :99 -screen 0 1400x900x24 -nolisten tcp &
export DISPLAY=:99 HOME=<isolated> XDG_RUNTIME_DIR=<short-path, chmod 700>
export DUCKTAPE_NODE_BIN="$(pwd)/target/debug/ducktape-node"
dbus-run-session -- bash -c '
  /usr/libexec/at-spi2-registryd &            # bare sessions cannot dbus-activate it
  target/debug/ducktape-iced & sleep 15
  # AccessKit activates only on an IsEnabled *change* signal:
  gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
    --method org.freedesktop.DBus.Properties.Set org.a11y.Status IsEnabled "<true>"
  ...drive with ops/iced-agent...
'
```

Gotchas (each cost a debug round — see memory `iced-agent-plugin-campaign`):
`XDG_RUNTIME_DIR` must be a short path (wayland sockets cap at 108 bytes); the
a11y switch must *change* after boot for OS-AccessKit checks (the bridge works
regardless); rustc ICE on this box → `CARGO_INCREMENTAL=0`.

## Native checklist

Use an isolated regular-user profile. Never run the desktop as root or
Administrator.

1. Launch the staged package and complete or restore onboarding
   (`find --role button` — onboarding's primary/secondary/link buttons are all
   tagged; drive with `click @ref` + `type`).
2. Create/select a workspace; verify its node becomes ready (`state --path
   has_workspace`, `logs`) and the UI stays responsive while starting.
3. Close the window, reopen/activate the app, and verify the workspace remains
   selected and the node was not duplicated.
4. Quit explicitly; verify the managed node and CEF child processes exit.
5. Open Browser, navigate to a signed `.duck` route (over CDP), resize/hide/
   show it, then leave Browser. Browsed content must not overlap native chrome,
   reach a direct HTTP(S)/loopback/file URL, or access desktop backend actions.
6. Inspect the workspace's `daemon.log` and `ops/iced-agent logs`; do not
   expose capability-bearing URL paths, keys, passwords, or recovery phrases in
   reports.

On macOS, the AppleScript smokes still cover packaged-bundle lifecycle:

```bash
make macos-smoke
make macos-cef-smoke
```

## Process safety

Never use `pkill -f`. Identify a process by executable, process cwd, and the
workspace's `--config` before signalling it. Use the application's own quit
path first. For merged-worktree cleanup, dry-run `ops/worktree-clean.sh` and
then use `--yes`; its retired-workflow reaper is intentionally preserved for
old external homes.

Report the package path, OS/display backend, CEF result, node lifecycle result,
commands run, and any skipped platform gate.
