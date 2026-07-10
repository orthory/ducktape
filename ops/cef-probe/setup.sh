#!/usr/bin/env bash
# CEF runtime probe — reproduce the environment that runs ducktape-desktop on
# tauri-runtime-cef (Chromium) instead of wry/WebKitGTK.
#
# Upstream is the UNRELEASED `feat/cef` branch of tauri (runtime crate
# tauri-runtime-cef, CEF 148). Two local modifications make our shell build
# without touching ~120 command signatures:
#   1. flip tauri's `default_runtime` annotations from (crate::Wry, wry) to
#      (crate::Cef, cef) so bare AppHandle/WebviewWindow default to Cef;
#   2. bump the patched crates to 2.90.0 so [patch.crates-io] outranks the
#      registry's newer release (patch loses the version race otherwise).
#
# After running this, from the repo root:
#   CEF_PATH=$HOME/.local/share/cef cargo build -p ducktape-desktop   # cef IS the runtime now, no flag
#   cp target/debug/{ducktape-node from a real build} target/debug/ducktape-node
#   chmod 755 target/debug/ducktape-node          # cp can drop the exec bit;
#                                                 # daemon.rs trust check needs it
#   # debug builds bake tauri.conf.json's devUrl: either run vite on :1430 or
#   # temporarily drop build.devUrl to serve the embedded ../dist
#   (cd target/debug && LD_LIBRARY_PATH=. ./ducktape-desktop)   # DISPLAY or Xvfb
#
# Verified 2026-07-11 on the headless Debian 13 box (Xvfb, no GPU): full boot —
# console UI, IPC round-trip, node sidecar spawn, identity unlock gate.
set -euo pipefail

CLONE="${1:-$HOME/.cache/ducktape-cef-probe/tauri-cef}"
ROOT="$(git rev-parse --show-toplevel)"

# macOS: cef-dll-sys builds CEF's sandbox wrapper through CMake with the Ninja
# generator HARDCODED (its build.rs calls `.generator("Ninja")`) — without
# cmake+ninja the build dies deep inside tauri-bundler's helper step with
# "CMake was unable to find a build program corresponding to Ninja". Preflight
# here where the error is actionable; auto-install via Homebrew when present.
# Linux never enters that path (no sandbox wrapper build).
if [ "$(uname -s)" = "Darwin" ]; then
  missing=""
  command -v cmake >/dev/null || missing="cmake"
  command -v ninja >/dev/null || missing="$missing ninja"
  if [ -n "$missing" ]; then
    if command -v brew >/dev/null; then
      echo "[cef-probe] installing required build tools:$missing"
      # shellcheck disable=SC2086
      brew install $missing
    else
      echo "[cef-probe] missing build tools:$missing" >&2
      echo "[cef-probe] install them first: brew install cmake ninja" >&2
      exit 1
    fi
  fi
fi

if [ ! -d "$CLONE" ]; then
  mkdir -p "$(dirname "$CLONE")"
  git clone --depth 1 --branch feat/cef --single-branch \
    https://github.com/tauri-apps/tauri.git "$CLONE"
fi

# 1. default runtime -> Cef (tauri crate sources only; macro def untouched).
# perl, not sed: GNU `sed -i` / `0,/re/` don't exist on BSD/macOS sed.
for f in $(grep -rl "default_runtime(crate::Wry, wry)" "$CLONE/crates/tauri/src"); do
  perl -pi -e 's/default_runtime\(crate::Wry, wry\)/default_runtime(crate::Cef, cef)/g' "$f"
done

# 2. version-bump the patched crates above any registry release
for c in tauri tauri-build tauri-runtime tauri-utils tauri-macros tauri-codegen tauri-plugin; do
  perl -pi -e 's/^version = ".*"$/version = "2.90.0"/ if !$done && /^version = /; $done = 1 if /^version = "2\.90\.0"$/' \
    "$CLONE/crates/$c/Cargo.toml"
done

# 3. point this workspace at the clone (idempotent), then adopt the patch
# SURGICALLY. Never `rm Cargo.lock` here: a full re-resolve moves unrelated
# git deps (it broke fluent31) — update only the seven patched crates.
if ! grep -q "CEF runtime probe" "$ROOT/Cargo.toml"; then
  cat >> "$ROOT/Cargo.toml" <<EOF

# --- CEF runtime probe (local-only; appended by ops/cef-probe/setup.sh) ---
[patch.crates-io]
tauri = { path = "$CLONE/crates/tauri" }
tauri-build = { path = "$CLONE/crates/tauri-build" }
tauri-runtime = { path = "$CLONE/crates/tauri-runtime" }
tauri-utils = { path = "$CLONE/crates/tauri-utils" }
tauri-macros = { path = "$CLONE/crates/tauri-macros" }
tauri-codegen = { path = "$CLONE/crates/tauri-codegen" }
tauri-plugin = { path = "$CLONE/crates/tauri-plugin" }
EOF
  (cd "$ROOT" && cargo update -p tauri -p tauri-build -p tauri-runtime \
    -p tauri-utils -p tauri-macros -p tauri-codegen -p tauri-plugin)
fi

echo "cef probe ready: $CLONE (do NOT commit the Cargo.toml patch or Cargo.lock)"
