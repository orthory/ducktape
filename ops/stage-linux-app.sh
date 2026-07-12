#!/usr/bin/env bash
# Stage the self-contained Linux desktop app and pack the release tarball.
# CEF ships libcef.so only as a shared library (no static build exists), so a
# self-contained Linux app is the binary plus the CEF payload in ONE directory,
# with the $ORIGIN rpath the binary carries (app/src-tauri/build.rs) resolving
# the libraries beside it — no LD_LIBRARY_PATH, so desktop launchers can spawn
# it with a clean environment. `make app` runs this after the release build:
#   target/release/bundle/linux/ducktape/            the relocatable app dir
#   target/release/bundle/linux/ducktape-linux-<arch>.tar.gz   release artifact
# The whitelist below is the CEF runtime set the build linked against (the
# same set qa/fleet/build-cef.sh stages for QA instances); everything else in
# target/release is build debris and must not ship.
set -euo pipefail
cd "$(dirname "$0")/.."

release_dir="target/release"
stage_root="$release_dir/bundle/linux"
stage="$stage_root/ducktape"

[ -x "$release_dir/ducktape-desktop" ] \
  || { echo "[linux-app] $release_dir/ducktape-desktop missing — run the release build first" >&2; exit 1; }
[ -x "$release_dir/ducktape-node" ] \
  || { echo "[linux-app] $release_dir/ducktape-node missing — beforeBuildCommand stages the sidecar" >&2; exit 1; }

rm -rf "$stage"
mkdir -p "$stage"

# The installed binary is named ducktape: the wayland app_id / x11 WM_CLASS is
# the binary name, and the desktop entry's StartupWMClass must match it.
install -m 755 "$release_dir/ducktape-desktop" "$stage/ducktape"
install -m 755 "$release_dir/ducktape-node" "$stage/ducktape-node"

cef_libs=(
  libcef.so libEGL.so libGLESv2.so libvk_swiftshader.so libvulkan.so.1
)
# chrome-sandbox is intentionally NOT setuid: modern kernels give Chromium the
# unprivileged-userns sandbox, and a mode-4755 file cannot ship in a tarball.
cef_bins=(chrome-sandbox)
cef_data=(
  chrome_100_percent.pak chrome_200_percent.pak icudtl.dat resources.pak
  v8_context_snapshot.bin vk_swiftshader_icd.json
)
for file in "${cef_libs[@]}" "${cef_bins[@]}"; do
  install -m 755 "$release_dir/$file" "$stage/$file"
done
for file in "${cef_data[@]}"; do
  install -m 644 "$release_dir/$file" "$stage/$file"
done
mkdir -p "$stage/locales"
install -m 644 "$release_dir"/locales/*.pak "$stage/locales/"

# The distribution's libcef.so carries ~1.1 GB of debug info; strip the STAGED
# copy only (the target-dir original stays symbolized for debugging). strip
# keeps .dynsym on a shared library, so the exported CEF C API is untouched.
strip "$stage/libcef.so"

# Gates: the binary must carry the $ORIGIN rpath and every dependency must
# resolve from the staged directory alone.
readelf -d "$stage/ducktape" | grep -q 'ORIGIN' \
  || { echo "[linux-app] staged binary lacks the \$ORIGIN rpath" >&2; exit 1; }
if ldd "$stage/ducktape" | grep 'not found'; then
  echo "[linux-app] staged binary has unresolved libraries (above)" >&2
  exit 1
fi

arch="$(uname -m)"
tarball="$stage_root/ducktape-linux-$arch.tar.gz"
tar --owner=0 --group=0 -czf "$tarball" -C "$stage_root" ducktape
echo "[linux-app] staged $stage"
echo "[linux-app] packed $tarball ($(du -h "$tarball" | cut -f1))"
