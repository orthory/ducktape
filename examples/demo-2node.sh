#!/usr/bin/env bash
# two REAL commonware ducktape nodes as separate OS processes — the payoff: node
# 1 converges on node 0's AddEntry over the broadcast lane, across the process
# boundary. the two commands that matter (everything else is plumbing):
#
#   cargo run -- node --config examples/node0.toml   # bootstrapper, drives the op
#   cargo run -- node --config examples/node1.toml   # dials node 0, converges
#
set -euo pipefail
cd "$(dirname "$0")/.."

# build once up front so the two `cargo run`s don't race the compiler.
cargo build -p ducktape

log0=$(mktemp)
log1=$(mktemp)

echo "launching node 0 (bootstrapper) + node 1 (dialer)..."
cargo run -q -- node --config examples/node0.toml >"$log0" 2>&1 &
pid0=$!
sleep 1
cargo run -q -- node --config examples/node1.toml >"$log1" 2>&1 &
pid1=$!

# wait up to ~30s for node 1 to log convergence.
status=1
for _ in $(seq 1 60); do
  if grep -q "converged" "$log1"; then status=0; break; fi
  sleep 0.5
done

echo "--- node 0 log ---"; cat "$log0"
echo "--- node 1 log ---"; cat "$log1"

kill "$pid0" "$pid1" 2>/dev/null || true
wait "$pid0" "$pid1" 2>/dev/null || true
rm -f "$log0" "$log1"

if [ "$status" -eq 0 ]; then
  echo "PASS: node 1 converged"
else
  echo "FAIL: node 1 did not converge within ~30s"
  exit 1
fi
