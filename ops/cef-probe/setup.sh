#!/usr/bin/env bash
# CEF build prerequisites for the native iced shell. `cef-dll-sys` downloads
# the pinned distribution itself; this script supplies Ninja on macOS when the
# host does not already have it and checks Windows native-build prerequisites.
#
# What remains, macOS only:
#   1. ninja preflight — cef-dll-sys hardcodes the Ninja CMake generator for
#      the sandbox-wrapper build; fetch the official static binary (no brew:
#      brew install is hostage to the machine's tap-trust/auto-update state).
# The native staging scripts own CEF packaging, so no external desktop CLI or
# patched bundler is part of the build. Linux needs no provisioning and exits
# immediately.
set -euo pipefail

TOOLS="${1:-$HOME/.cache/ducktape-cef-tools}"

host_os="$(uname -s)"
case "$host_os" in
  Darwin) ;;
  MINGW*|MSYS*|CYGWIN*)
    for tool in cmake ninja mt.exe; do
      command -v "$tool" >/dev/null \
        || { echo "[cef] $tool is required in an MSVC/Windows SDK environment" >&2; exit 1; }
    done
    echo "cef: ready"
    exit 0
    ;;
  *)
    echo "cef: nothing to provision on $host_os"
    exit 0
    ;;
esac

if ! command -v cmake >/dev/null; then
  echo "[cef] cmake is required (cef sandbox wrapper build): brew install cmake" >&2
  exit 1
fi
TOOLS_BIN="$TOOLS/bin"
NINJA_VERSION="v1.12.1"
NINJA_ARCHIVE_SHA256="89a287444b5b3e98f88a945afa50ce937b8ffd1dcc59c555ad9b1baf855298c9"
NINJA_BINARY_SHA256="a46eca0aae6e8b7792dde0580b766f948fba5fb58fca8127078f7374573af6d5"
if [ -e "$TOOLS_BIN/ninja" ]; then
  cached_sha256="$(shasum -a 256 "$TOOLS_BIN/ninja" | awk '{print $1}')"
  [ "$cached_sha256" = "$NINJA_BINARY_SHA256" ] \
    || { echo "[cef] cached Ninja is not the pinned official $NINJA_VERSION binary: $TOOLS_BIN/ninja" >&2; exit 1; }
  chmod 755 "$TOOLS_BIN/ninja"
elif ! command -v ninja >/dev/null; then
  mkdir -p "$TOOLS_BIN"
  archive="$TOOLS_BIN/ninja-mac.zip"
  trap 'rm -f "$archive"' EXIT
  echo "[cef] fetching ninja $NINJA_VERSION (official static binary) -> $TOOLS_BIN/ninja"
  curl -fsSL -o "$archive" \
    "https://github.com/ninja-build/ninja/releases/download/$NINJA_VERSION/ninja-mac.zip"
  archive_sha256="$(shasum -a 256 "$archive" | awk '{print $1}')"
  [ "$archive_sha256" = "$NINJA_ARCHIVE_SHA256" ] \
    || { echo "[cef] Ninja archive SHA-256 mismatch" >&2; exit 1; }
  unzip -o -q "$archive" -d "$TOOLS_BIN"
  binary_sha256="$(shasum -a 256 "$TOOLS_BIN/ninja" | awk '{print $1}')"
  [ "$binary_sha256" = "$NINJA_BINARY_SHA256" ] \
    || { rm -f "$TOOLS_BIN/ninja"; echo "[cef] extracted Ninja SHA-256 mismatch" >&2; exit 1; }
  chmod 755 "$TOOLS_BIN/ninja"
  rm -f "$archive"
  trap - EXIT
fi
echo "cef: ready"
