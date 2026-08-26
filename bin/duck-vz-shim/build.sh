#!/usr/bin/env bash
# Build and codesign the macOS VMM shim. macOS only.
#
#   bin/duck-vz-shim/build.sh                # build + ad-hoc sign
#   INSTALL=~/bin bin/duck-vz-shim/build.sh  # …and copy onto PATH
#
# The codesign step is NOT optional: Virtualization.framework refuses any
# binary without the com.apple.security.virtualization entitlement, and the
# node's boot probe checks for it (sandbox.rs probe_vz) so the failure lands
# at boot with this script named — not per-run inside the framework.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"

[[ "$(uname -s)" == "Darwin" ]] || { echo "duck-vz-shim builds only on macOS" >&2; exit 1; }

swift build --package-path "$HERE" -c release
BIN="$HERE/.build/release/duck-vz-shim"

codesign --force --sign - --entitlements "$HERE/vz.entitlements" "$BIN"
echo "signed: $BIN"

if [[ -n "${INSTALL:-}" ]]; then
  install -m 0755 "$BIN" "$INSTALL/duck-vz-shim"
  echo "installed: $INSTALL/duck-vz-shim"
fi
