#!/usr/bin/env bash
# Reject an incomplete native iced bundle. A bundle with only the executable
# and node sidecar looks valid to Finder but cannot initialize CEF.
set -euo pipefail

app="${1:?usage: check-macos-cef-bundle.sh /path/to/Ducktape.app}"
contents="$app/Contents"
framework="$contents/Frameworks/Chromium Embedded Framework.framework/Chromium Embedded Framework"
resources="$contents/Frameworks/Chromium Embedded Framework.framework/Resources"
sandbox="$contents/Frameworks/Chromium Embedded Framework.framework/Libraries/libcef_sandbox.dylib"

fail() {
  echo "[cef-bundle] invalid $app: $*" >&2
  exit 1
}

version_not_newer_than() {
  awk -v actual="$1" -v limit="$2" 'BEGIN {
    if (actual !~ /^[0-9]+([.][0-9]+){0,2}$/ || limit !~ /^[0-9]+([.][0-9]+){0,2}$/) exit 2
    actual_count = split(actual, actual_parts, ".")
    limit_count = split(limit, limit_parts, ".")
    for (index = 1; index <= 3; index++) {
      actual_part = index <= actual_count ? actual_parts[index] + 0 : 0
      limit_part = index <= limit_count ? limit_parts[index] + 0 : 0
      if (actual_part < limit_part) exit 0
      if (actual_part > limit_part) exit 1
    }
    exit 0
  }'
}

check_macos_minimum() {
  local executable="$1" label="$2" versions
  versions="$(otool -l "$executable" | awk '
    $1 == "cmd" { command = $2; next }
    command == "LC_BUILD_VERSION" && $1 == "minos" { print $2; command = ""; next }
    command == "LC_VERSION_MIN_MACOSX" && $1 == "version" { print $2; command = "" }
  ')" || fail "could not inspect the macOS deployment target for $label"
  [ -n "$versions" ] || fail "$label has no readable macOS deployment target"
  while IFS= read -r version; do
    version_not_newer_than "$version" 14.0 \
      || fail "$label requires macOS $version (bundle minimum is 14.0)"
  done <<<"$versions"
}

[ -x "$contents/MacOS/ducktape" ] || fail "missing main executable"
[ -x "$contents/MacOS/ducktape-node" ] || fail "missing node sidecar"
[ -f "$contents/Resources/ducktape.icns" ] || fail "missing application icon"
[ -f "$framework" ] || fail "missing Chromium Embedded Framework"
[ -f "$sandbox" ] || fail "missing CEF macOS sandbox library"
[ -f "$resources/icudtl.dat" ] || fail "missing CEF ICU resource icudtl.dat"

macho_count=0
while IFS= read -r -d '' candidate; do
  if file -b "$candidate" | grep -q 'Mach-O'; then
    label="${candidate#"$contents/"}"
    lipo -verify_arch "$(uname -m)" "$candidate" >/dev/null \
      || fail "$label does not contain the host architecture $(uname -m)"
    check_macos_minimum "$candidate" "$label"
    macho_count=$((macho_count + 1))
  fi
done < <(find "$contents" -type f -print0)
[ "$macho_count" -gt 0 ] || fail "bundle contains no Mach-O payloads"

plutil -lint "$contents/Info.plist" >/dev/null || fail "invalid Info.plist"
for key in CFBundleExecutable CFBundleIdentifier LSMinimumSystemVersion NSCameraUsageDescription \
  NSMicrophoneUsageDescription NSScreenCaptureUsageDescription \
  NSLocalNetworkUsageDescription; do
  plutil -extract "$key" raw "$contents/Info.plist" >/dev/null \
    || fail "Info.plist is missing $key"
done
bundle_minimum="$(plutil -extract LSMinimumSystemVersion raw "$contents/Info.plist")"
[ "$bundle_minimum" = 14.0 ] \
  || fail "Info.plist minimum is $bundle_minimum (expected 14.0)"
principal_class="$(plutil -extract NSPrincipalClass raw "$contents/Info.plist" 2>/dev/null)" \
  || fail "Info.plist is missing NSPrincipalClass"
[ "$principal_class" = DucktapeApplication ] \
  || fail "NSPrincipalClass is $principal_class (expected DucktapeApplication)"

# The helper executables must carry the same executable code as the app binary.
# CEF registers custom schemes per process; a foreign helper (the old
# unrelated helper stub) skips the app's `duck` scheme registration; Mojo then
# rejects cross-process navigation while every process still looks healthy.
# Mach-O signatures are intentionally different because each helper has a
# unique bundle identifier, so compare temporary signature-stripped copies.
comparison_dir="$(mktemp -d "${TMPDIR:-/tmp}/ducktape-cef-compare.XXXXXX")" \
  || fail "could not create helper comparison directory"
trap 'rm -rf "$comparison_dir"' EXIT
cp "$contents/MacOS/ducktape" "$comparison_dir/main"
codesign --remove-signature "$comparison_dir/main" >/dev/null 2>&1 \
  || fail "could not inspect the unsigned main executable payload"
for suffix in "" " (Alerts)" " (GPU)" " (Plugin)" " (Renderer)"; do
  helper="ducktape Helper${suffix}"
  case "$suffix" in
    "") expected_helper_id="com.ducktape.app.helper" ;;
    " (Alerts)") expected_helper_id="com.ducktape.app.helper.alerts" ;;
    " (GPU)") expected_helper_id="com.ducktape.app.helper.gpu" ;;
    " (Plugin)") expected_helper_id="com.ducktape.app.helper.plugin" ;;
    " (Renderer)") expected_helper_id="com.ducktape.app.helper.renderer" ;;
  esac
  helper_contents="$contents/Frameworks/$helper.app/Contents"
  helper_exe="$helper_contents/MacOS/$helper"
  [ -x "$helper_exe" ] || fail "missing $helper.app"
  plutil -lint "$helper_contents/Info.plist" >/dev/null \
    || fail "$helper.app has an invalid Info.plist"
  helper_id="$(plutil -extract CFBundleIdentifier raw "$helper_contents/Info.plist" 2>/dev/null)" \
    || fail "$helper.app is missing CFBundleIdentifier"
  [ "$helper_id" = "$expected_helper_id" ] \
    || fail "$helper.app has bundle id $helper_id (expected $expected_helper_id)"
  helper_minimum="$(plutil -extract LSMinimumSystemVersion raw "$helper_contents/Info.plist" 2>/dev/null)" \
    || fail "$helper.app is missing LSMinimumSystemVersion"
  [ "$helper_minimum" = 14.0 ] \
    || fail "$helper.app minimum is $helper_minimum (expected 14.0)"
  for key in NSCameraUsageDescription NSMicrophoneUsageDescription NSScreenCaptureUsageDescription; do
    plutil -extract "$key" raw "$helper_contents/Info.plist" >/dev/null \
      || fail "$helper.app is missing $key"
  done
  helper_entitlements="$(codesign -d --entitlements :- "$helper_contents" 2>/dev/null || true)"
  grep -q 'com.apple.security.device.camera' <<<"$helper_entitlements" \
    || fail "$helper.app is missing its camera entitlement"
  grep -q 'com.apple.security.device.audio-input' <<<"$helper_entitlements" \
    || fail "$helper.app is missing its microphone entitlement"
  grep -q 'com.apple.security.cs.allow-jit' <<<"$helper_entitlements" \
    || fail "$helper.app is missing its CEF JIT entitlement"
  helper_copy="$comparison_dir/helper${suffix//[^A-Za-z]/_}"
  cp "$helper_exe" "$helper_copy"
  codesign --remove-signature "$helper_copy" >/dev/null 2>&1 \
    || fail "could not inspect the unsigned payload for $helper.app"
  cmp -s "$helper_copy" "$comparison_dir/main" \
    || fail "$helper.app does not contain the app executable code (stale/stub helper breaks scheme registration)"
done

codesign --verify --deep --strict "$app" 2>/dev/null || fail "invalid nested code signature"
entitlements="$(codesign -d --entitlements :- "$app" 2>/dev/null || true)"
grep -q 'com.apple.security.device.camera' <<<"$entitlements" \
  || fail "missing camera entitlement"
grep -q 'com.apple.security.device.audio-input' <<<"$entitlements" \
  || fail "missing microphone entitlement"
grep -q 'com.apple.security.cs.allow-jit' <<<"$entitlements" \
  || fail "missing CEF JIT entitlement"

signature="$(codesign -dvvv "$app" 2>&1)" || fail "could not inspect code signature"
if grep -q '^Signature=adhoc$' <<<"$signature"; then
  signing_label="ad-hoc local-test"
else
  grep -q '^Authority=Developer ID Application:' <<<"$signature" \
    || fail "distribution signature is not a Developer ID Application identity"
  grep -q 'flags=.*runtime' <<<"$signature" \
    || fail "distribution signature does not enable the hardened runtime"
  grep -q '^Timestamp=' <<<"$signature" \
    || fail "distribution signature has no secure timestamp"
  signing_label="Developer ID"
fi

echo "[cef-bundle] verified $signing_label iced app + node + $macho_count Mach-O payloads + 5 helpers in $app"
