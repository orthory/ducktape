#!/usr/bin/env bash
# Stage the debug ducktape-node as Tauri's externalBin sidecar for dev-only
# macOS bundle skeleton builds. The running dev app still uses
# DUCKTAPE_NODE_BIN; this sidecar only satisfies the bundler.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
triple=$(rustc -vV | sed -n 's/^host: //p')
suffix=
case "$triple" in
  *windows*) suffix=.exe ;;
esac

src="$root/target/debug/ducktape-node${suffix}"
if [ ! -x "$src" ]; then
  cargo build -p node-bin --manifest-path "$root/Cargo.toml"
fi

dest="$root/app/src-tauri/binaries/ducktape-node-${triple}${suffix}"
mkdir -p "${dest%/*}"
rm -f "$dest"
install -m 755 "$src" "$dest"
echo "staged debug sidecar $dest"
