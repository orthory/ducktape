#!/usr/bin/env bash
# build the networked node and stage it as the tauri sidecar for THIS host
# triple. `tauri build` runs this via beforeBuildCommand so the bundle always
# carries a fresh ducktape-node (overwriting the empty placeholder build.rs
# creates for plain `cargo build`).
set -euo pipefail
cd "$(dirname "$0")/.."

triple=$(rustc -vV | sed -n 's/^host: //p')
cargo build --release -p node-bin -p duckdnsd --manifest-path ../Cargo.toml
mkdir -p src-tauri/binaries
# rm first: cp onto the existing build.rs placeholder would keep the
# placeholder's non-executable mode
rm -f "src-tauri/binaries/ducktape-node-${triple}"
cp "../target/release/ducktape-node" "src-tauri/binaries/ducktape-node-${triple}"
rm -f "src-tauri/binaries/duckdnsd-${triple}"
cp "../target/release/duckdnsd" "src-tauri/binaries/duckdnsd-${triple}"
echo "staged src-tauri/binaries/ducktape-node-${triple} and duckdnsd-${triple}"
