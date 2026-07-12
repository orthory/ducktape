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
# Full 40-char sha, deliberately: `git fetch <remote> <commit>` only accepts
# unabbreviated commits, so a short pin makes fresh provisioning impossible.
SHA="3b2823b918d5ea88fca10b472daf349c67c22d51"

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
# no per-run fetch. Guard on a usable HEAD, not directory existence: the
# fetch is not atomic, and a previous attempt that died mid-provision must
# self-heal instead of satisfying the check forever. To move the pin: bump
# SHA (full 40 chars) and re-run — the mismatch gate below forces the
# re-clone.
if ! git -C "$CLONE" rev-parse --quiet --verify HEAD >/dev/null 2>&1; then
  rm -rf "$CLONE"
  mkdir -p "$CLONE"
  git -C "$CLONE" init -q
  git -C "$CLONE" fetch --depth 1 "$UPSTREAM" "$SHA"
  git -C "$CLONE" checkout -q FETCH_HEAD
fi

# A checkout at the wrong commit means the pin moved (or the cache was
# tampered with); silently building a drifted CLI is how bugs hide behind
# rev pins. Refuse with the remedy instead.
HAVE="$(git -C "$CLONE" rev-parse HEAD)"
if [ "$HAVE" != "$SHA" ]; then
  echo "[cef] checkout is at $HAVE but the pin is $SHA — rm -rf $CLONE and re-run" >&2
  exit 1
fi

# Repo-tracked fixes to the pinned checkout, applied idempotently on every
# run (cef-env precedes every make target, and the clone may predate a new
# patch).
#   0001 makes the bundler's CEF helper .apps re-exec the app binary instead
#     of a generic embedded stub: CEF requires every process to register the
#     same custom schemes, and the stub registered none, so
#     `tauri://localhost` origins failed Mojo validation in helper processes
#     and the packaged app rendered a permanently blank window.
#   0002 makes the CLI's CEF packaging probe see `cef` when it is enabled
#     through `default`, so the bundler actually copies the framework and
#     helpers. Without it the .app builds "successfully" with no CEF payload
#     at all and panics in cef::library_loader at launch.
# Both are gated by ops/check-macos-cef-bundle.sh, which `make app` runs.
PATCH_DIR="$(cd "$(dirname "$0")" && pwd)/patches"
for patch in "$PATCH_DIR"/*.patch; do
  [ -e "$patch" ] || continue
  if git -C "$CLONE" apply --check --reverse "$patch" 2>/dev/null; then
    continue # already applied
  fi
  if git -C "$CLONE" apply --check "$patch" 2>/dev/null; then
    echo "[cef] applying $(basename "$patch")"
    git -C "$CLONE" apply "$patch"
  else
    echo "[cef] $(basename "$patch") applies neither forward nor reverse;" \
      "the checkout has drifted — rm -rf $CLONE and re-run" >&2
    exit 1
  fi
done

echo "cef: ready (CLI checkout at $CLONE)"
