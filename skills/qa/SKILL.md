---
name: qa
description: Verify the real native Ducktape Iced desktop package, its managed node, and its isolated CEF Browser pane. Use for UI/design QA, lifecycle regressions, package checks, and native smoke testing that the static React preview cannot cover.
---

# Native Iced QA

Use the packaged native app, not the React dev server, for desktop claims. Iced
owns the interface and node lifecycle; CEF exists only inside Browser.

## Baseline gates

```bash
cargo test -p ducktape-iced
cd app
bun install --frozen-lockfile
bun run typecheck
bun run test
bun run build
```

Build or run the matching native app from the repository root:

```bash
make dev   # interactive debug app
make app   # release package in target/release/bundle
```

## Native checklist

Use an isolated regular-user profile. Never run the desktop as root or
Administrator.

1. Launch the staged package and complete or restore onboarding.
2. Create/select a workspace; verify its node becomes ready and the UI remains
   responsive while starting.
3. Close the window, reopen/activate the app, and verify the workspace remains
   selected and the node was not duplicated.
4. Quit explicitly; verify the managed node and CEF child processes exit.
5. Open Browser, navigate to a signed `.duck` route, resize/hide/show it, then
   leave Browser. Browsed content must not overlap native chrome, reach a
   direct HTTP(S)/loopback/file URL, or access desktop backend actions.
6. Inspect the workspace's `daemon.log`; do not expose capability-bearing URL
   paths, keys, passwords, or recovery phrases in reports.

On macOS, automate the supported lifecycle and Browser checks:

```bash
make macos-smoke
make macos-cef-smoke
```

The invoking terminal needs Accessibility permission. On Linux or Windows,
record the exact staged executable, isolated profile path, platform/session,
steps, screenshot, and relevant bounded log excerpt.

## Process safety

Never use `pkill -f`. Identify a process by executable, process cwd, and the
workspace's `--config` before signalling it. Use the application's own quit
path first. For merged-worktree cleanup, dry-run `ops/worktree-clean.sh` and
then use `--yes`; its retired-workflow reaper is intentionally preserved for
old external homes.

Report the package path, OS/display backend, CEF result, node lifecycle result,
commands run, and any skipped platform gate.
