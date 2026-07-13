#!/usr/bin/env bash
# Fleet-only fault injector for the sandbox Apply rollback smoke. User-key and
# other verbs pass through; an armed long-lived node start fails exactly once.
set -euo pipefail

: "${FLEET_ARTIFACT_DIR:?Fleet must provide FLEET_ARTIFACT_DIR}"
: "${FLEET_HOME:?Fleet must provide FLEET_HOME}"

marker="$FLEET_HOME/.ducktape/qa-fail-next-node-start"
if [ "${1:-}" = "--config" ] && [ -f "$marker" ]; then
  rm -f "$marker"
  echo "intentional Fleet QA node-start failure" >&2
  exit 70
fi

exec "$FLEET_ARTIFACT_DIR/bin/ducktape-node" "$@"
