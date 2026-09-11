#!/usr/bin/env bash
# Run from the checkout root. Match guest-builder's canonical source prefixes;
# encoded flags preserve paths containing spaces and replace ambient Rust flags.
set -euo pipefail
ice=$1
shift
repo=$(pwd -P)
cargo_home=$(cd "${CARGO_HOME:-$HOME/.cargo}" && pwd -P)
# A toolchain installed without rustup (the agent guest's /opt/rust) has no
# rustup home; its sysroot is the prefix rust's own paths live under.
rustup_home=$(cd "${RUSTUP_HOME:-$HOME/.rustup}" 2>/dev/null && pwd -P || rustc --print sysroot)
view_target=${CARGO_TARGET_DIR:-$repo/target}
mkdir -p "$view_target"
view_target=$(cd "$view_target" && pwd -P)
printf -v CARGO_ENCODED_RUSTFLAGS '%s\x1f%s\x1f%s\x1f%s' \
  "--remap-path-prefix=$cargo_home=/cargo" \
  "--remap-path-prefix=$rustup_home=/rustup" \
  "--remap-path-prefix=$repo=/ducktape" \
  "--remap-path-prefix=$view_target=/view-builder"
export CARGO_ENCODED_RUSTFLAGS
export CARGO_TARGET_DIR="$view_target"
unset RUSTFLAGS
exec "$ice" bundle --manifest-path crates/views/Cargo.toml "$@" \
  --target wasm32-unknown-unknown --no-wasm-opt --out target/views
