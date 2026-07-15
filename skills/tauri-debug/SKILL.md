---
name: tauri-debug
description: Debug the real native Ducktape Iced window when a static React preview is insufficient. Use for native layout, workspace/node lifecycle, system integration, media, packaging, or the isolated CEF Browser pane.
---

# Native desktop debug

The skill name is retained as a compatibility alias. The application is native
Iced; there is no DOM driver, JavaScript evaluator, webview endpoint, or desktop
MCP server.

## Launch the real app

```bash
make dev
```

This builds the matching `ducktape-node`. macOS launches a staged `.app`, Linux
runs the debug Iced binary with an explicit sidecar, and Windows uses the staged
CEF bootstrap package. For packaging-only failures use `make app` and launch
the artifact under `target/release/bundle`.

## Observe before changing code

- Reproduce with an isolated regular-user HOME/profile when safe.
- Capture the OS, display backend, package/executable path, active workspace,
  visible state, and exact interaction sequence.
- Read the workspace's bounded `daemon.log` and native stderr. Use `RUST_LOG`
  to raise one logging plane on a live node instead of restarting away the
  failure.
- Locate processes by executable/cwd and workspace config. Never use
  `pkill -f`, and never log key material or capability-bearing URL paths.

## Platform probes

On macOS:

```bash
make macos-smoke
make macos-cef-smoke
```

The first validates native window activation and close-to-menu-bar behavior;
the second stages the dedicated CEF probe and validates child creation,
navigation, bounds, and teardown. Accessibility permission is required.

On Linux/Windows, launch the staged package and inspect native accessibility
or screenshot tools supplied by the host. Do not infer native behavior from
`bun run dev`; that command serves only the static React twin.

For Browser defects, first determine whether the failure is native chrome,
CEF lifecycle, proxy/policy, or page content. CEF resources must come from the
same staged package as the executable; do not substitute a system runtime.

## Teardown

Quit through the app, verify its managed node and CEF children exit, then remove
the isolated profile. For merged worktrees use `ops/worktree-clean.sh` in
dry-run mode first and `--yes` only after reviewing its refusals.
