# Desktop operations

Ducktape's supported desktop is the native Rust application in
`app/src-iced`. Iced owns the UI, workspace registry, node lifecycle, system
integration, and media. The pinned CEF runtime is present only for the isolated
Browser pane. The React application under `app/` is an independent static web
twin and is not part of the desktop package.

## Development

From the repository root:

```bash
make dev       # matching debug node + native Iced app
make web       # independent React bundle -> app/dist
```

`make dev` stages a real `.app` on macOS, runs the flat binary with an explicit
node sidecar on Linux, and stages the CEF bootstrap package on Windows. CEF is
Cargo-pinned; `make cef-env` installs only the build prerequisite needed by the
current host.

## Packages

```bash
make app       # self-contained package under target/release/bundle
make install   # current-user install; no root required
```

Every desktop package includes the matching `ducktape-node` sidecar and pinned
CEF payload. Platform staging lives in `stage-macos-iced-app.sh`,
`stage-linux-app.sh`, and `stage-windows-app.ps1`. Do not copy only the main
executable: its sibling sidecar and CEF resources are part of the package
contract.

Windows staging must run in an MSVC/Windows SDK environment where `mt.exe` is
on `PATH`; the script embeds the `asInvoker` manifest in the bootstrap PE and
reads it back before packaging.

macOS local builds are ad-hoc signed and require macOS 14 or newer. For a
direct-distribution release, provide a Developer ID Application identity and,
optionally, a notarytool keychain profile:

```bash
DUCKTAPE_MACOS_SIGN_IDENTITY='Developer ID Application: Example (TEAMID)' \
DUCKTAPE_MACOS_NOTARY_PROFILE=ducktape-notary make app
```

The staging script signs nested Mach-O files inside-out with the hardened
runtime, submits the release ZIP when a notary profile is set, staples the app,
and repacks the stapled artifact. Credentials remain in the login keychain.

## Native verification

Run the repository gates first:

```bash
cargo test -p ducktape-iced
cd app && bun install --frozen-lockfile
bun run typecheck
bun run test
bun run build
```

On macOS, the supported real-window gates are:

```bash
make macos-smoke       # launch, close-to-menu-bar, activation reopen
make macos-cef-smoke   # staged CEF child, navigation, bounds, teardown
```

The terminal running `macos-smoke` needs Accessibility permission. On Linux
and Windows, launch the staged package from `target/release/bundle`, complete
onboarding in an isolated user profile, enter a workspace, and open Browser.
Verify that `ducktape-node` belongs to that workspace and that the CEF child
exits with the app. Use the `qa` and `tauri-debug` compatibility-named skills
for the exact checklist.

## Worktree cleanup

Current native QA has no Fleet configuration or external instance manager.
`ops/worktree-clean.sh` intentionally retains a self-contained, identity-
verified reaper for homes left by the retired Fleet workflow. Always dry-run it
before removing merged worktrees, then pass `--yes`; it refuses dirty or
unmerged work and never uses `pkill -f`.
