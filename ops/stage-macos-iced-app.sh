#!/usr/bin/env bash
# Stage the native iced application, pinned CEF framework, and re-exec helpers.
set -euo pipefail
cd "$(dirname "$0")/.."

profile="${1:-release}"
case "$profile" in
  debug|release) ;;
  *) echo "usage: stage-macos-iced-app.sh [debug|release]" >&2; exit 2 ;;
esac
build_dir="target/$profile"
source_binary="${DUCKTAPE_MACOS_BINARY:-ducktape-iced}"
app="$build_dir/bundle/macos/Ducktape.app"
contents="$app/Contents"
frameworks="$contents/Frameworks"
cef_root="${CEF_PATH:-$HOME/.local/share/cef}"
icon="app/src-iced/assets/icons/icon.icns"
entitlements="app/src-iced/assets/macos/Entitlements.plist"
sign_identity="${DUCKTAPE_MACOS_SIGN_IDENTITY:-}"
notary_profile="${DUCKTAPE_MACOS_NOTARY_PROFILE:-}"

if [ -n "$notary_profile" ] && [ -z "$sign_identity" ]; then
  echo "[macos-app] DUCKTAPE_MACOS_NOTARY_PROFILE requires DUCKTAPE_MACOS_SIGN_IDENTITY" >&2
  exit 2
fi
if [ -n "$notary_profile" ] && [ "$profile" != release ]; then
  echo "[macos-app] notarization is only supported for release packages" >&2
  exit 2
fi
if [ "$profile" = release ] && [ -n "$sign_identity" ] && [ -z "$notary_profile" ]; then
  echo "[macos-app] a Developer ID release requires DUCKTAPE_MACOS_NOTARY_PROFILE" >&2
  exit 2
fi

cef_version="$(awk '
  $0 == "name = \"cef\"" { in_cef = 1; next }
  in_cef && /^version = / {
    gsub(/^version = \"|\"$/, "")
    split($0, parts, "+")
    print parts[2]
    exit
  }
' Cargo.lock)"
[ -n "$cef_version" ] \
  || { echo "[macos-app] could not resolve the pinned CEF distribution from Cargo.lock" >&2; exit 1; }
case "$(uname -m)" in
  arm64) cef_arch=aarch64 ;;
  x86_64) cef_arch=x86_64 ;;
  *) echo "[macos-app] unsupported macOS architecture: $(uname -m)" >&2; exit 1 ;;
esac
versioned_framework="$cef_root/$cef_version/cef_macos_$cef_arch/Chromium Embedded Framework.framework"
cef_framework="$versioned_framework"

[ -x "$build_dir/$source_binary" ] \
  || { echo "[macos-app] missing $build_dir/$source_binary" >&2; exit 1; }
[ -x "$build_dir/ducktape-node" ] \
  || { echo "[macos-app] missing $build_dir/ducktape-node" >&2; exit 1; }
[ -f "$cef_framework/Chromium Embedded Framework" ] \
  || { echo "[macos-app] missing pinned CEF $cef_version for $cef_arch below $cef_root" >&2; exit 1; }
[ -f "$icon" ] || { echo "[macos-app] missing $icon" >&2; exit 1; }
[ -f "$entitlements" ] || { echo "[macos-app] missing $entitlements" >&2; exit 1; }

rm -rf "$app"
mkdir -p "$contents/MacOS" "$contents/Resources" "$frameworks"
install -m 755 "$build_dir/$source_binary" "$contents/MacOS/ducktape"
install -m 755 "$build_dir/ducktape-node" "$contents/MacOS/ducktape-node"
ditto "$cef_framework" "$frameworks/Chromium Embedded Framework.framework"

install -m 644 "$icon" "$contents/Resources/ducktape.icns"

cat >"$contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleDisplayName</key><string>Ducktape</string>
  <key>CFBundleExecutable</key><string>ducktape</string>
  <key>CFBundleIconFile</key><string>ducktape.icns</string>
  <key>CFBundleIdentifier</key><string>com.ducktape.app</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>Ducktape</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleSignature</key><string>????</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.productivity</string>
  <key>LSMinimumSystemVersion</key><string>14.0</string>
  <key>LSMultipleInstancesProhibited</key><true/>
  <key>LSEnvironment</key><dict><key>MallocNanoZone</key><string>0</string></dict>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSPrincipalClass</key><string>DucktapeApplication</string>
  <key>NSSupportsAutomaticGraphicsSwitching</key><true/>
  <key>NSCameraUsageDescription</key><string>Ducktape uses the camera when you turn it on in a huddle or approve a trusted Duck site request.</string>
  <key>NSMicrophoneUsageDescription</key><string>Ducktape uses the microphone when you join a huddle or approve a trusted Duck site request.</string>
  <key>NSScreenCaptureUsageDescription</key><string>Ducktape records a screen only while you are sharing it in a huddle.</string>
  <key>NSLocalNetworkUsageDescription</key><string>Ducktape uses your local network to link your own devices and connect to workspace nodes.</string>
</dict></plist>
PLIST

for suffix in "" " (Alerts)" " (GPU)" " (Plugin)" " (Renderer)"; do
  helper="ducktape Helper${suffix}"
  case "$suffix" in
    "") helper_id="com.ducktape.app.helper" ;;
    " (Alerts)") helper_id="com.ducktape.app.helper.alerts" ;;
    " (GPU)") helper_id="com.ducktape.app.helper.gpu" ;;
    " (Plugin)") helper_id="com.ducktape.app.helper.plugin" ;;
    " (Renderer)") helper_id="com.ducktape.app.helper.renderer" ;;
  esac
  helper_contents="$frameworks/$helper.app/Contents"
  mkdir -p "$helper_contents/MacOS" "$helper_contents/Resources" "$helper_contents/Frameworks"
  install -m 755 "$build_dir/$source_binary" "$helper_contents/MacOS/$helper"
  cat >"$helper_contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleDisplayName</key><string>$helper</string>
  <key>CFBundleExecutable</key><string>$helper</string>
  <key>CFBundleIdentifier</key><string>$helper_id</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>$helper</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleSignature</key><string>????</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>LSEnvironment</key><dict><key>MallocNanoZone</key><string>0</string></dict>
  <key>LSFileQuarantineEnabled</key><true/>
  <key>LSMinimumSystemVersion</key><string>14.0</string>
  <key>LSUIElement</key><true/>
  <key>NSSupportsAutomaticGraphicsSwitching</key><true/>
  <key>NSCameraUsageDescription</key><string>Ducktape uses the camera only after an in-app permission approval.</string>
  <key>NSMicrophoneUsageDescription</key><string>Ducktape uses the microphone only after an in-app permission approval.</string>
  <key>NSScreenCaptureUsageDescription</key><string>Ducktape captures a screen only while an approved share is active.</string>
</dict></plist>
PLIST
done

if [ -n "$sign_identity" ]; then
  sign_args=(--force --sign "$sign_identity" --options runtime --timestamp)
  signing_label="Developer ID"
else
  sign_args=(--force --sign -)
  signing_label="ad-hoc local-test"
fi

# Sign leaf Mach-O files first, then their containing framework/helper bundles,
# and finally the outer app. This avoids codesign --deep guessing at nested
# boundaries and gives every CEF process the V8/device entitlements it needs.
while IFS= read -r -d '' candidate; do
  if file -b "$candidate" | grep -q 'Mach-O'; then
    lipo "$candidate" -verify_arch "$(uname -m)" \
      || { echo "[macos-app] wrong architecture in $candidate" >&2; exit 1; }
    codesign "${sign_args[@]}" "$candidate"
  fi
done < <(find "$contents" -type f -print0)

codesign "${sign_args[@]}" \
  "$frameworks/Chromium Embedded Framework.framework"
for suffix in "" " (Alerts)" " (GPU)" " (Plugin)" " (Renderer)"; do
  codesign "${sign_args[@]}" --entitlements "$entitlements" \
    "$frameworks/ducktape Helper${suffix}.app"
done
codesign "${sign_args[@]}" --entitlements "$entitlements" "$app"
echo "[macos-app] staged $app ($signing_label signature)"
if [ "$profile" = release ]; then
  archive_base="$build_dir/bundle/macos/Ducktape-macos-$(uname -m)"
  if [ -n "$sign_identity" ]; then
    archive="$archive_base.zip"
  else
    archive="$archive_base-unsigned.zip"
  fi
  rm -f "$archive_base.zip" "$archive_base-unsigned.zip"
  ditto -c -k --sequesterRsrc --keepParent "$app" "$archive"
  if [ -n "$notary_profile" ]; then
    xcrun notarytool submit "$archive" --keychain-profile "$notary_profile" --wait
    xcrun stapler staple "$app"
    xcrun stapler validate "$app"
    spctl --assess --type execute --verbose=2 "$app"
    rm -f "$archive"
    ditto -c -k --sequesterRsrc --keepParent "$app" "$archive"
  fi
  echo "[macos-app] packed $archive"
fi
