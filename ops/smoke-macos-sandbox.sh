#!/usr/bin/env bash
# Reproducible Apple Silicon sandbox gate: real Podman/Tart provider cycles,
# shutdown semantics, and the native CEF Apply + rollback transaction.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

[ "$(uname -s)" = Darwin ] || { echo "[sandbox-smoke] macOS is required" >&2; exit 2; }
[ "$(uname -m)" = arm64 ] || { echo "[sandbox-smoke] Apple Silicon is required" >&2; exit 2; }

scope="${1:-all}"
case "$scope" in all|podman|tart|ui) ;; *) echo "usage: $0 [all|podman|tart|ui]" >&2; exit 2 ;; esac

retry() {
  local attempts="$1"
  shift
  local n=1
  until "$@"; do
    if [ "$n" -ge "$attempts" ]; then
      return 1
    fi
    echo "[sandbox-smoke] attempt $n failed; retrying: $*" >&2
    sleep $((n * 2))
    n=$((n + 1))
  done
}

if [ "$scope" = all ] || [ "$scope" = podman ]; then
  command -v podman >/dev/null || { echo "[sandbox-smoke] podman is not installed" >&2; exit 1; }
  if ! podman info >/dev/null 2>&1; then
    podman machine start || true
    retry 3 podman info >/dev/null
  fi
  podman_image="${DUCKTAPE_MACOS_PODMAN_IMAGE:-docker.io/library/node:22-slim}"
  retry 3 podman pull "$podman_image"
  retry 2 env DUCKTAPE_MACOS_PODMAN_IMAGE="$podman_image" \
    ops/build-with.sh cargo test -p capability-host macos_podman_hardware_smoke -- --ignored --nocapture
fi

if [ "$scope" = all ] || [ "$scope" = tart ]; then
  command -v tart >/dev/null || { echo "[sandbox-smoke] tart is not installed" >&2; exit 1; }
  command -v sshpass >/dev/null || { echo "[sandbox-smoke] sshpass is not installed" >&2; exit 1; }
  tart_image="${DUCKTAPE_MACOS_TART_IMAGE:-ghcr.io/cirruslabs/macos-sonoma-base:latest}"
  retry 2 env DUCKTAPE_MACOS_TART_IMAGE="$tart_image" \
    ops/build-with.sh cargo test -p capability-host macos_tart_hardware_smoke -- --ignored --nocapture
fi

if [ "$scope" = all ] || [ "$scope" = ui ]; then
  fleet="${FLEET:-app/node_modules/@byeongsu-hong/tauri-agent-fleet/dist/cli.js}"
  (cd app && bun install --frozen-lockfile)
  retry 2 ops/build-with.sh cargo test -p noded shutdown_wakes_every_surface_and_remains_sticky
  retry 2 env FLEET="$fleet" bun qa/macos-sandbox-apply-smoke.ts success
  retry 2 env FLEET="$fleet" bun qa/macos-sandbox-apply-smoke.ts rollback
fi

echo "[sandbox-smoke] $scope passed"
