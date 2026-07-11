#!/usr/bin/env bash
set -euo pipefail

: "${FLEET_ARTIFACT_DIR:?Fleet must provide FLEET_ARTIFACT_DIR}"
: "${FLEET_ARTIFACT_MANIFEST:?Fleet must provide FLEET_ARTIFACT_MANIFEST}"

root="$(cd "$(dirname "$0")/../.." && pwd)"
export CEF_PATH="${CEF_PATH:-$HOME/.local/share/cef}"

if [ ! -d "$root/app/node_modules" ]; then
  (cd "$root/app" && bun install --frozen-lockfile)
fi

target_dir="$(cd "$root" && cargo metadata --no-deps --format-version 1 | bun -e 'console.log((await Bun.stdin.json()).target_directory)')"
triple="$(rustc -vV | sed -n 's/^host: //p')"
(cd "$root" && ops/build-with.sh cargo build -p node-bin --bin ducktape-node)
install -d -m 700 "$root/app/src-tauri/binaries"
install -m 755 "$target_dir/debug/ducktape-node" "$root/app/src-tauri/binaries/ducktape-node-$triple"

(cd "$root/app" && VITE_TAURI_AGENT=1 ../ops/build-with.sh bun run tauri build --debug --no-bundle \
  --config '{"build":{"beforeBuildCommand":"bun run build"}}')

install -d -m 700 "$FLEET_ARTIFACT_DIR/bin"
install -m 755 "$target_dir/debug/ducktape-desktop" "$FLEET_ARTIFACT_DIR/bin/ducktape"
install -m 755 "$target_dir/debug/ducktape-node" "$FLEET_ARTIFACT_DIR/bin/ducktape-node"
for file in libcef.so libEGL.so libGLESv2.so libvk_swiftshader.so libvulkan.so.1 \
  chrome-sandbox chrome_100_percent.pak chrome_200_percent.pak icudtl.dat \
  resources.pak v8_context_snapshot.bin vk_swiftshader_icd.json; do
  install -m 755 "$target_dir/debug/$file" "$FLEET_ARTIFACT_DIR/bin/$file"
done
cp -a "$target_dir/debug/locales" "$FLEET_ARTIFACT_DIR/bin/"

bun -e '
  await Bun.write(process.env.FLEET_ARTIFACT_MANIFEST, JSON.stringify({
    schemaVersion: 1,
    executable: "bin/ducktape",
    args: ["--no-sandbox", "--single-process", "--in-process-gpu"],
    cwd: "bin",
    env: { CEF_PATH: process.env.CEF_PATH, LD_LIBRARY_PATH: "." }
  }) + "\n")
'
