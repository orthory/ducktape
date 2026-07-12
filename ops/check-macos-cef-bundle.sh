#!/usr/bin/env bash
# Reject the small but superficially valid bundle emitted when the feat/cef
# Tauri CLI does not receive its CEF feature signal. That bundle contains the
# main executable and sidecar, but no framework/helpers, so it panics in
# cef::library_loader as soon as macOS launches it.
set -euo pipefail

app="${1:?usage: check-macos-cef-bundle.sh /path/to/Ducktape.app}"
contents="$app/Contents"
framework="$contents/Frameworks/Chromium Embedded Framework.framework/Chromium Embedded Framework"
resources="$contents/Frameworks/Chromium Embedded Framework.framework/Resources"

fail() {
  echo "[cef-bundle] invalid $app: $*" >&2
  exit 1
}

[ -x "$contents/MacOS/ducktape-desktop" ] || fail "missing main executable"
[ -f "$framework" ] || fail "missing Chromium Embedded Framework"
[ -f "$resources/icudtl.dat" ] || fail "missing CEF ICU resource icudtl.dat"

# The helper executables must be byte-identical copies of the app binary.
# CEF registers custom schemes per process; a foreign helper (the old
# embedded bundler stub) skips the app's registration, `tauri://localhost`
# origins then fail Mojo validation in helper processes, and the packaged
# app renders a permanently blank window while every process looks healthy.
for suffix in "" " (Alerts)" " (GPU)" " (Plugin)" " (Renderer)"; do
  helper="ducktape-desktop Helper${suffix}"
  helper_exe="$contents/Frameworks/$helper.app/Contents/MacOS/$helper"
  [ -x "$helper_exe" ] || fail "missing $helper.app"
  cmp -s "$helper_exe" "$contents/MacOS/ducktape-desktop" \
    || fail "$helper.app is not the app binary (stale/stub helper breaks scheme registration)"
done

echo "[cef-bundle] verified framework + 5 app-binary helper apps in $app"
