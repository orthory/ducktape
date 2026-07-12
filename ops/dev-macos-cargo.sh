#!/usr/bin/env bash
# `cargo-tauri dev --runner` replaces the Cargo command itself; it is not
# Cargo's target runner. Keep that outer command Cargo-compatible, then point
# the actual aarch64 macOS target runner at dev-macos-runner.sh so the compiled
# executable is launched from the staged CEF app bundle. ops/dev.sh passes the
# host triple explicitly because Cargo does not apply a target runner to an
# implicit host build. It also enables the dependency-equivalent `dev-cef`
# feature name so the CEF-aware CLI does not take its incomplete macOS dev
# bundling branch before Cargo gets control.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
export CARGO_TARGET_AARCH64_APPLE_DARWIN_RUNNER="$root/ops/dev-macos-runner.sh"
build_with="${BUILD_WITH:-$root/ops/build-with.sh}"

exec "$build_with" "${CARGO:-cargo}" "$@"
