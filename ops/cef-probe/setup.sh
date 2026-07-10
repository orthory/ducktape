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
#   CEF_PATH=$HOME/.local/share/cef cargo build -p ducktape-desktop --features cef
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

if [ ! -d "$CLONE" ]; then
  mkdir -p "$(dirname "$CLONE")"
  git clone --depth 1 --branch feat/cef --single-branch \
    https://github.com/tauri-apps/tauri.git "$CLONE"
fi

# 1. default runtime -> Cef (tauri crate sources only; macro def untouched)
grep -rl "default_runtime(crate::Wry, wry)" "$CLONE/crates/tauri/src" \
  | xargs -r sed -i 's/default_runtime(crate::Wry, wry)/default_runtime(crate::Cef, cef)/g'

# 2. version-bump the patched crates above any registry release
for c in tauri tauri-build tauri-runtime tauri-utils tauri-macros tauri-codegen tauri-plugin; do
  sed -i '0,/^version = ".*"/s//version = "2.90.0"/' "$CLONE/crates/$c/Cargo.toml"
done

# 3. point this workspace at the clone (idempotent), drop the stale lock
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
  rm -f "$ROOT/Cargo.lock"
fi

echo "cef probe ready: $CLONE (do NOT commit the Cargo.toml patch or Cargo.lock)"
