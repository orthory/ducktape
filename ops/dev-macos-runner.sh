#!/usr/bin/env bash
# Cargo target runner used by `cargo-tauri dev` on macOS. Tauri still builds
# the debug executable and owns Vite/rebuilds; Cargo passes the finished binary
# here so it can be staged into a valid CEF .app bundle before execution.
set -euo pipefail

binary="${1:?usage: dev-macos-runner.sh /path/to/ducktape-desktop [args...]}"
shift || true

root="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$root"
# shellcheck source=ops/dev.sh
DEV_SH_LIB=1 . "$root/ops/dev.sh"

app="$(stage_macos_debug_bundle "$binary")"
exec "$app/Contents/MacOS/ducktape-desktop" "$@"
