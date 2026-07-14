#!/usr/bin/env bash
set -euo pipefail

: "${FLEET_ARTIFACT_DIR:?Fleet must provide FLEET_ARTIFACT_DIR}"
: "${FLEET_ARTIFACT_MANIFEST:?Fleet must provide FLEET_ARTIFACT_MANIFEST}"
export RUST_MIN_STACK="${RUST_MIN_STACK:-67108864}"

root="$(cd "$(dirname "$0")/../.." && pwd)"
export CEF_PATH="${CEF_PATH:-$HOME/.local/share/cef}"
export CEF_CLONE="${CEF_CLONE:-$HOME/.cache/ducktape-cef-probe/tauri-cef}"
export PATH="$(dirname "$CEF_CLONE")/bin:$PATH"

if [ ! -d "$root/app/node_modules" ]; then
  (cd "$root/app" && bun install --frozen-lockfile)
fi

target_dir="$(cd "$root" && cargo metadata --no-deps --format-version 1 | bun -e 'console.log((await Bun.stdin.json()).target_directory)')"
triple="$(rustc -vV | sed -n 's/^host: //p')"
(cd "$root" && cargo build -p node-bin --bin ducktape-node)
install -d -m 700 "$root/app/src-tauri/binaries"
install -m 755 "$target_dir/debug/ducktape-node" "$root/app/src-tauri/binaries/ducktape-node-$triple"

install -d -m 700 "$FLEET_ARTIFACT_DIR/bin"
install -m 755 "$target_dir/debug/ducktape-node" "$FLEET_ARTIFACT_DIR/bin/ducktape-node"

cef_version="$(cd "$root" && cargo metadata --format-version 1 | bun -e '
  const metadata = await Bun.stdin.json()
  const pkg = metadata.packages.find((candidate) => candidate.name === "cef")
  if (!pkg) throw new Error("cargo metadata contains no cef package")
  const version = pkg.version.split("+")[1]
  if (!version) throw new Error(`cef package version has no distribution suffix: ${pkg.version}`)
  console.log(version)
')"

artifact_env='{}'
artifact_executable='bin/ducktape'
artifact_cwd='bin'
case "$(uname -s)" in
  Darwin)
    bash "$root/ops/cef-probe/setup.sh" "$CEF_CLONE"
    (cd "$root/app" && VITE_TAURI_AGENT=1 bun run tauri build \
      --debug --bundles app \
      --config '{"build":{"beforeBuildCommand":"bun run build"}}')
    app_bundle="$target_dir/debug/bundle/macos/Ducktape.app"
    contents="$app_bundle/Contents"
    main_binary="$contents/MacOS/ducktape-desktop"
    case "$(uname -m)" in
      arm64) cef_arch=aarch64 ;;
      x86_64) cef_arch=x86_64 ;;
      *) echo "unsupported macOS CEF architecture: $(uname -m)" >&2; exit 71 ;;
    esac
    framework="$CEF_PATH/Chromium Embedded Framework.framework"
    if [ ! -f "$framework/Chromium Embedded Framework" ]; then
      framework="$CEF_PATH/$cef_version/cef_macos_$cef_arch/Chromium Embedded Framework.framework"
    fi
    [ -f "$framework/Chromium Embedded Framework" ] || {
      echo "CEF framework not found for macOS $cef_arch at $framework" >&2
      exit 71
    }
    install -d -m 700 "$contents/Frameworks"
    rm -rf "$contents/Frameworks/Chromium Embedded Framework.framework"
    cp -R "$framework" "$contents/Frameworks/"
    bundle_id="$(plutil -extract CFBundleIdentifier raw "$contents/Info.plist")"
    bundle_version="$(plutil -extract CFBundleVersion raw "$contents/Info.plist")"
    short_version="$(plutil -extract CFBundleShortVersionString raw "$contents/Info.plist")"
    for suffix in '' ' (Alerts)' ' (GPU)' ' (Plugin)' ' (Renderer)'; do
      helper="ducktape-desktop Helper${suffix}"
      helper_contents="$contents/Frameworks/$helper.app/Contents"
      helper_executable="$helper_contents/MacOS/$helper"
      install -d -m 700 "$helper_contents/MacOS" "$helper_contents/Resources"
      install -m 755 "$main_binary" "$helper_executable"
      plist="$helper_contents/Info.plist"
      plutil -create xml1 "$plist"
      plutil -insert CFBundleDevelopmentRegion -string English "$plist"
      plutil -insert CFBundleDisplayName -string "$helper" "$plist"
      plutil -insert CFBundleExecutable -string "$helper" "$plist"
      plutil -insert CFBundleIdentifier -string "$bundle_id.helper" "$plist"
      plutil -insert CFBundleInfoDictionaryVersion -string 6.0 "$plist"
      plutil -insert CFBundleName -string "$helper" "$plist"
      plutil -insert CFBundlePackageType -string APPL "$plist"
      plutil -insert CFBundleShortVersionString -string "$short_version" "$plist"
      plutil -insert CFBundleVersion -string "$bundle_version" "$plist"
      plutil -insert LSMinimumSystemVersion -string 11.0 "$plist"
      plutil -insert LSUIElement -bool true "$plist"
    done
    bash "$root/ops/check-macos-cef-bundle.sh" "$app_bundle"
    install -d -m 700 "$FLEET_ARTIFACT_DIR/app"
    rm -rf "$FLEET_ARTIFACT_DIR/app/Ducktape.app"
    cp -R "$app_bundle" "$FLEET_ARTIFACT_DIR/app/"
    rm -f "$FLEET_ARTIFACT_DIR/bin/ducktape-node"
    ln -s '../app/Ducktape.app/Contents/MacOS/ducktape-node' \
      "$FLEET_ARTIFACT_DIR/bin/ducktape-node"
    artifact_executable='app/Ducktape.app/Contents/MacOS/ducktape-desktop'
    artifact_cwd='app/Ducktape.app/Contents/MacOS'
    ;;
  *)
    (cd "$root/app" && VITE_TAURI_AGENT=1 bun run tauri build --debug --no-bundle \
      --config '{"build":{"beforeBuildCommand":"bun run build"}}')
    install -m 755 "$target_dir/debug/ducktape-desktop" "$FLEET_ARTIFACT_DIR/bin/ducktape"
    # Hardlink the CEF runtime from the immutable versioned distribution so
    # every artifact shares one on-disk copy (1.3 GB per copy otherwise).
    # Fall back to copying when the dist file is missing, stale, or on
    # another filesystem.
    cef_dist="$CEF_PATH/$cef_version/cef_linux_$(uname -m)"
    for file in libcef.so libEGL.so libGLESv2.so libvk_swiftshader.so libvulkan.so.1 \
      chrome-sandbox chrome_100_percent.pak chrome_200_percent.pak icudtl.dat \
      resources.pak v8_context_snapshot.bin vk_swiftshader_icd.json; do
      built="$target_dir/debug/$file"
      if [ -f "$cef_dist/$file" ] \
        && [ "$(stat -c%s "$cef_dist/$file")" = "$(stat -c%s "$built")" ] \
        && ln -f "$cef_dist/$file" "$FLEET_ARTIFACT_DIR/bin/$file" 2>/dev/null; then
        continue
      fi
      install -m 755 "$built" "$FLEET_ARTIFACT_DIR/bin/$file"
    done
    if ! cp -alf "$cef_dist/locales" "$FLEET_ARTIFACT_DIR/bin/" 2>/dev/null; then
      rm -rf "$FLEET_ARTIFACT_DIR/bin/locales"
      cp -a "$target_dir/debug/locales" "$FLEET_ARTIFACT_DIR/bin/"
    fi
    ;;
esac

export FLEET_ARTIFACT_ENV="$artifact_env"
export FLEET_ARTIFACT_EXECUTABLE="$artifact_executable"
export FLEET_ARTIFACT_CWD="$artifact_cwd"

bun -e '
  await Bun.write(process.env.FLEET_ARTIFACT_MANIFEST, JSON.stringify({
    protocol: "tauri-agent-artifact/v1",
    executable: process.env.FLEET_ARTIFACT_EXECUTABLE,
    args: ["--no-sandbox", "--single-process", "--in-process-gpu"],
    cwd: process.env.FLEET_ARTIFACT_CWD,
    env: JSON.parse(process.env.FLEET_ARTIFACT_ENV)
  }) + "\n")
'
