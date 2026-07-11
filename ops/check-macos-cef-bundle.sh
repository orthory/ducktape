#!/usr/bin/env bash
# Reject the small but superficially valid bundle emitted when the feat/cef
# Tauri CLI does not receive its CEF feature signal. That bundle contains the
# main executable and sidecar, but no framework/helpers, so it panics in
# cef::library_loader as soon as macOS launches it.
set -euo pipefail

app="${1:?usage: check-macos-cef-bundle.sh /path/to/Ducktape.app}"
contents="$app/Contents"
framework="$contents/Frameworks/Chromium Embedded Framework.framework/Chromium Embedded Framework"

fail() {
  echo "[cef-bundle] invalid $app: $*" >&2
  exit 1
}

[ -x "$contents/MacOS/ducktape-desktop" ] || fail "missing main executable"
[ -f "$framework" ] || fail "missing Chromium Embedded Framework"

for suffix in "" " (Alerts)" " (GPU)" " (Plugin)" " (Renderer)"; do
  helper="ducktape-desktop Helper${suffix}"
  helper_exe="$contents/Frameworks/$helper.app/Contents/MacOS/$helper"
  [ -x "$helper_exe" ] || fail "missing $helper.app"
done

echo "[cef-bundle] verified framework + 5 helper apps in $app"
