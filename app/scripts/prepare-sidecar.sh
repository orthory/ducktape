#!/usr/bin/env bash
# build the node daemon and stage it as the tauri sidecar for THIS host triple.
# `tauri build` runs this via beforeBuildCommand so the bundle always carries a
# fresh ducktape-noded (overwriting the empty placeholder build.rs creates for
# plain `cargo build`).
set -euo pipefail
cd "$(dirname "$0")/.."

triple=$(rustc -vV | sed -n 's/^host: //p')
cargo build --release -p noded --manifest-path ../Cargo.toml
mkdir -p src-tauri/binaries
# rm first: cp onto the existing build.rs placeholder would keep the
# placeholder's non-executable mode
rm -f "src-tauri/binaries/ducktape-noded-${triple}"
cp "../target/release/ducktape-noded" "src-tauri/binaries/ducktape-noded-${triple}"
echo "staged src-tauri/binaries/ducktape-noded-${triple}"
