#!/usr/bin/env bash
# CEF build prerequisites. The heavy lifting is GONE from this script: the
# workspace's [patch.crates-io] now points at the committed fork branch
# github.com/byeongsu-hong/tauri#ducktape-cef (upstream feat/cef + the
# default-runtime flip + 2.90.0 bump), so plain `cargo build` works on any
# machine with no local checkout and no Cargo.toml mutation.
#
# What remains, macOS only:
#   1. ninja preflight — cef-dll-sys hardcodes the Ninja CMake generator for
#      the sandbox-wrapper build; fetch the official static binary (no brew:
#      brew install is hostage to the machine's tap-trust/auto-update state).
#   2. a checkout of the fork branch — the .app bundle must be built with the
#      feat/cef tauri CLI (it copies the CEF framework + helper apps; the npm
#      CLI produces a bundle that panics at launch), and a CLI needs source.
# Linux needs neither and exits immediately.
set -euo pipefail

CLONE="${1:-$HOME/.cache/ducktape-cef-probe/tauri-cef}"
FORK="https://github.com/byeongsu-hong/tauri.git"
BRANCH="ducktape-cef"

[ "$(uname -s)" = "Darwin" ] || { echo "cef: nothing to provision on $(uname -s)"; exit 0; }

if ! command -v cmake >/dev/null; then
  echo "[cef] cmake is required (cef sandbox wrapper build): brew install cmake" >&2
  exit 1
fi
PROBE_BIN="$(dirname "$CLONE")/bin"
if ! command -v ninja >/dev/null && [ ! -x "$PROBE_BIN/ninja" ]; then
  NINJA_VERSION="v1.12.1"
  echo "[cef] fetching ninja $NINJA_VERSION (official static binary) -> $PROBE_BIN/ninja"
  mkdir -p "$PROBE_BIN"
  curl -fsSL -o "$PROBE_BIN/ninja-mac.zip" \
    "https://github.com/ninja-build/ninja/releases/download/$NINJA_VERSION/ninja-mac.zip"
  unzip -o -q "$PROBE_BIN/ninja-mac.zip" -d "$PROBE_BIN"
  rm -f "$PROBE_BIN/ninja-mac.zip"
  chmod 755 "$PROBE_BIN/ninja"
fi

# Clone once; cef-env runs before every make target, so no per-run fetch.
# To pick up new fork commits: rm -rf the checkout and re-run.
if [ ! -d "$CLONE" ]; then
  mkdir -p "$(dirname "$CLONE")"
  git clone --depth 1 --branch "$BRANCH" --single-branch "$FORK" "$CLONE"
fi

echo "cef: ready (CLI checkout at $CLONE)"
