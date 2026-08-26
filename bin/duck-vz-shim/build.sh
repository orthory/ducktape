#!/usr/bin/env bash
# Build and codesign the macOS VMM shim. macOS only.
#
#   bin/duck-vz-shim/build.sh                # build + ad-hoc sign
#   INSTALL=~/bin bin/duck-vz-shim/build.sh  # …and copy onto PATH
#
# swiftc DIRECTLY, not swift-package: the shim is one dependency-free file,
# and SPM breaks outright on a Command Line Tools install whose llbuild is
# out of step with its own compiler (a dyld symbol abort, seen live) — a
# single swiftc invocation has no build system to be broken by.
#
# The codesign step is NOT optional: Virtualization.framework refuses any
# binary without the com.apple.security.virtualization entitlement, and the
# node's boot probe checks for it (sandbox.rs probe_vz) so the failure lands
# at boot with this script named — not per-run inside the framework.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"

[[ "$(uname -s)" == "Darwin" ]] || { echo "duck-vz-shim builds only on macOS" >&2; exit 1; }

SRC="$HERE/Sources/main.swift"
OUTDIR="$HERE/.build"
BIN="$OUTDIR/duck-vz-shim"
LOG="$OUTDIR/swiftc.log"
TARGET="$(uname -m)-apple-macos13.0"
mkdir -p "$OUTDIR"

try_build() { # $1 = SDK path, or empty for the toolchain default
  xcrun swiftc -O -target "$TARGET" ${1:+-sdk "$1"} \
    -o "$BIN" "$SRC" -framework Virtualization >"$LOG" 2>&1
}

# A CLT install can carry an SDK NEWER than its own compiler (measured:
# Swift 6.1.2 beside the macOS 26.2 SDK cannot even parse that SDK's stdlib
# interface). The toolchain default is tried first — a healthy CLT or full
# Xcode never goes further — then every installed CLT SDK, newest first,
# until one the compiler understands builds cleanly.
if ! try_build ""; then
  built=0
  for sdk in $(ls -d /Library/Developer/CommandLineTools/SDKs/MacOSX[0-9]*.sdk 2>/dev/null | sort -r); do
    if try_build "$sdk"; then
      echo "note: default SDK unusable with this compiler; built against $(basename "$sdk")"
      built=1
      break
    fi
  done
  if [[ "$built" != 1 ]]; then
    echo "swiftc failed against every installed SDK; last attempt:" >&2
    tail -20 "$LOG" >&2
    exit 1
  fi
fi

codesign --force --sign - --entitlements "$HERE/vz.entitlements" "$BIN"
echo "signed: $BIN"

if [[ -n "${INSTALL:-}" ]]; then
  install -m 0755 "$BIN" "$INSTALL/duck-vz-shim"
  echo "installed: $INSTALL/duck-vz-shim"
fi
