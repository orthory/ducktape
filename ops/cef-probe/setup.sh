#!/usr/bin/env bash
# CEF build prerequisites. The heavy lifting is GONE from this script: the
# workspace builds against published crates.io tauri plus the standalone
# runtime crate github.com/byeongsu-hong/tauri-runtime-cef, so plain
# `cargo build` works on any machine with no local checkout and no
# Cargo.toml mutation.
#
# What remains, macOS only:
#   1. ninja preflight — cef-dll-sys hardcodes the Ninja CMake generator for
#      the sandbox-wrapper build; fetch the official static binary (no brew:
#      brew install is hostage to the machine's tap-trust/auto-update state).
#   2. a pinned upstream tauri checkout — the .app bundle must be built with
#      the feat/cef tauri CLI (it copies the CEF framework + helper apps; the
#      npm CLI produces a bundle that panics at launch), and a CLI needs
#      source. Pinned to the feat/cef commit the runtime crate was extracted
#      from; this mac bundle flow is a known ceiling until a ducktape-owned
#      bundle script replaces the pinned CLI.
# Linux needs neither and exits immediately.
set -euo pipefail

CLONE="${1:-$HOME/.cache/ducktape-cef-probe/tauri-cef}"
UPSTREAM="https://github.com/tauri-apps/tauri.git"
SHA="3b2823b91"

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

# Shallow-fetch the pinned sha once (feat/cef can rebase, so a branch clone
# would drift; the commit can't). cef-env runs before every make target, so
# no per-run fetch. To move the pin: bump SHA, rm -rf the checkout, re-run.
if [ ! -d "$CLONE" ]; then
  mkdir -p "$CLONE"
  git -C "$CLONE" init -q
  git -C "$CLONE" fetch --depth 1 "$UPSTREAM" "$SHA"
  git -C "$CLONE" checkout -q FETCH_HEAD
fi

echo "cef: ready (CLI checkout at $CLONE)"
