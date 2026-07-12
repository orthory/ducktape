#!/usr/bin/env bash
# Run a build command with optional local Rust accelerators.
#
# sccache is useful on every supported host. mold is an ELF linker, so it is
# enabled only for the native Linux host target and only when clang is present.
# Missing helpers are a clean fallback: this script must never turn an ordinary
# Ducktape build into a tool-installation prerequisite.
set -euo pipefail

status_only=0
if [ "${1:-}" = "--status" ]; then
  status_only=1
  shift
fi

if [ "$status_only" -eq 0 ] && [ "$#" -eq 0 ]; then
  echo "usage: ops/build-with.sh [--status] <command> [args...]" >&2
  exit 2
fi

host_os="$(uname -s 2>/dev/null || true)"
rustc_bin="${RUSTC:-rustc}"
host_target="$($rustc_bin -vV 2>/dev/null | sed -n 's/^host: //p' | head -1)"

sccache_state="unavailable"
sccache_path="$(command -v sccache 2>/dev/null || true)"
if [ "${DUCKTAPE_DISABLE_SCCACHE:-0}" = "1" ]; then
  # An explicit empty RUSTC_WRAPPER also overrides a user-global Cargo
  # `[build] rustc-wrapper`; merely declining to set it leaves that wrapper on.
  export RUSTC_WRAPPER=
  sccache_state="disabled by DUCKTAPE_DISABLE_SCCACHE"
elif [ -n "${RUSTC_WRAPPER:-}" ]; then
  sccache_state="preserving RUSTC_WRAPPER=${RUSTC_WRAPPER}"
elif [ -n "$sccache_path" ]; then
  export RUSTC_WRAPPER="$sccache_path"
  export SCCACHE_IGNORE_SERVER_IO_ERROR="${SCCACHE_IGNORE_SERVER_IO_ERROR:-1}"
  sccache_state="enabled ($sccache_path)"

  # Ducktape routinely builds from linked worktrees. Normalizing both the
  # current root and main worktree root lets identical sources share cache keys.
  if [ -z "${SCCACHE_BASEDIRS:-}" ]; then
    repo_root="$(git rev-parse --show-toplevel 2>/dev/null || true)"
    if [ -n "$repo_root" ]; then
      common_dir="$(git -C "$repo_root" rev-parse --git-common-dir 2>/dev/null || true)"
      case "$common_dir" in
        /*) main_root="$(dirname "$common_dir")" ;;
        *)  main_root="$repo_root" ;;
      esac
      if [ "$main_root" = "$repo_root" ]; then
        export SCCACHE_BASEDIRS="$repo_root"
      else
        export SCCACHE_BASEDIRS="$repo_root:$main_root"
      fi
    fi
  fi
fi

mold_state="unavailable"
mold_path="$(command -v mold 2>/dev/null || true)"
clang_path="$(command -v clang 2>/dev/null || true)"
if [ "${DUCKTAPE_DISABLE_MOLD:-0}" = "1" ]; then
  mold_state="disabled by DUCKTAPE_DISABLE_MOLD"
elif [ "$host_os" != "Linux" ] || [ -z "$host_target" ]; then
  mold_state="not applicable ($host_os)"
elif [[ "$host_target" != *-linux-* ]]; then
  mold_state="not applicable ($host_target)"
elif [ -z "$mold_path" ]; then
  mold_state="unavailable"
elif [ -z "$clang_path" ]; then
  mold_state="unavailable (clang is required)"
elif [ -n "${RUSTFLAGS:-}" ] || [ -n "${CARGO_ENCODED_RUSTFLAGS:-}" ]; then
  mold_state="preserving existing Rust flags"
else
  target_key="$(printf '%s' "$host_target" | tr '[:lower:].-' '[:upper:]__')"
  linker_var="CARGO_TARGET_${target_key}_LINKER"
  rustflags_var="CARGO_TARGET_${target_key}_RUSTFLAGS"
  current_linker="$(printenv "$linker_var" 2>/dev/null || true)"

  case "${current_linker##*/}" in
    ""|clang|clang-[0-9]*)
      selected_linker="${current_linker:-$clang_path}"
      current_rustflags="$(printenv "$rustflags_var" 2>/dev/null || true)"
      case "$current_rustflags" in
        *link-arg=-fuse-ld=mold*) ;;
        "") current_rustflags="-C link-arg=-fuse-ld=mold" ;;
        *) current_rustflags="$current_rustflags -C link-arg=-fuse-ld=mold" ;;
      esac
      export "$linker_var=$selected_linker"
      export "$rustflags_var=$current_rustflags"
      mold_state="enabled ($mold_path via $selected_linker for $host_target)"
      ;;
    *)
      mold_state="preserving $linker_var=$current_linker"
      ;;
  esac
fi

if [ "$status_only" -eq 1 ]; then
  printf 'sccache: %s\n' "$sccache_state"
  printf 'mold: %s\n' "$mold_state"
  if [ -n "${SCCACHE_BASEDIRS:-}" ]; then
    printf 'sccache base dirs: %s\n' "$SCCACHE_BASEDIRS"
  fi
  exit 0
fi

if [ "${DUCKTAPE_BUILD_HELPERS_QUIET:-0}" != "1" ]; then
  printf '[build] sccache: %s; mold: %s\n' "$sccache_state" "$mold_state" >&2
fi

exec "$@"
