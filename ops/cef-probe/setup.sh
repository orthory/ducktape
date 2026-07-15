#!/usr/bin/env bash
# CEF build prerequisites for the native iced shell. `cef-dll-sys` downloads
# the pinned distribution itself; this script only supplies Ninja on macOS
# when the host does not already have it.
#
# What remains, macOS only:
#   1. ninja preflight — cef-dll-sys hardcodes the Ninja CMake generator for
#      the sandbox-wrapper build; fetch the official static binary (no brew:
#      brew install is hostage to the machine's tap-trust/auto-update state).
# The native `stage-macos-iced-app.sh` owns framework/helper packaging, so no
# external desktop CLI or patched bundler is part of the build. Linux
# needs no provisioning and exits immediately.
set -euo pipefail

TOOLS="${1:-$HOME/.cache/ducktape-cef-tools}"

[ "$(uname -s)" = "Darwin" ] || { echo "cef: nothing to provision on $(uname -s)"; exit 0; }

if ! command -v cmake >/dev/null; then
  echo "[cef] cmake is required (cef sandbox wrapper build): brew install cmake" >&2
  exit 1
fi
TOOLS_BIN="$TOOLS/bin"
if ! command -v ninja >/dev/null && [ ! -x "$TOOLS_BIN/ninja" ]; then
  NINJA_VERSION="v1.12.1"
  echo "[cef] fetching ninja $NINJA_VERSION (official static binary) -> $TOOLS_BIN/ninja"
  mkdir -p "$TOOLS_BIN"
  curl -fsSL -o "$TOOLS_BIN/ninja-mac.zip" \
    "https://github.com/ninja-build/ninja/releases/download/$NINJA_VERSION/ninja-mac.zip"
  unzip -o -q "$TOOLS_BIN/ninja-mac.zip" -d "$TOOLS_BIN"
  rm -f "$TOOLS_BIN/ninja-mac.zip"
  chmod 755 "$TOOLS_BIN/ninja"
fi
echo "cef: ready"
