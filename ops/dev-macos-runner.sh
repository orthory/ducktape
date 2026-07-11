#!/usr/bin/env bash
# Runner used by `cargo-tauri dev` on macOS. Tauri still builds the debug
# executable and owns Vite/rebuilds; this script stages that executable into a
# valid CEF .app bundle before execing it.
set -euo pipefail

binary="${1:?usage: dev-macos-runner.sh /path/to/ducktape-desktop [args...]}"
shift || true

root="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$root"
BUILD_WITH="${BUILD_WITH:-$ROOT/ops/build-with.sh}"
# shellcheck source=ops/dev.sh
DEV_SH_LIB=1 . "$root/ops/dev.sh"

app="$(stage_macos_debug_bundle "$binary")"
exec "$app/Contents/MacOS/ducktape-desktop" "$@"
