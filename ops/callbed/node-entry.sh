#!/usr/bin/env bash
# Node container entrypoint: wait for bootstrap to have written this node's
# config into the shared volume, then run the validator. `depends_on:
# service_completed_successfully` already gates on bootstrap exiting, so the
# wait is just belt-and-braces against volume-mount races.
set -euo pipefail
CFG="${1:?usage: node-entry.sh <config-dir>}"
for _ in $(seq 1 120); do [ -f "$CFG/node.toml" ] && break; sleep 0.5; done
[ -f "$CFG/node.toml" ] || { echo "[node] $CFG/node.toml never appeared"; exit 1; }
echo "[node] launching ducktape node run --config $CFG/node.toml"
exec /usr/local/bin/ducktape node run --config "$CFG/node.toml"
