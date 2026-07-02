#!/usr/bin/env bash
# two REAL commonware ducktape nodes as separate OS processes — the payoff: both
# processes CONVERGE on a byte-identical app-hash over real localhost TCP after
# each applies BOTH nodes' ops, driven by a live simplex BFT engine (NOT gossip).
# node 0 submits DISTINCT op A (directory k0=node-0), node 1 submits DISTINCT op B
# (k1=node-1). with PER-PROCESS content stores a node has NO bytes for the op it
# did not originate, so it can only apply the peer's op if the leader's eager
# relay gossiped the payload on CHANNEL_PAYLOAD and this node's store-only drain
# cached it. each node prints `converged` only once it has applied ALL N ops. the
# two commands that matter:
#
#   ducktape-node --config examples/node0.toml   # bootstrapper
#   ducktape-node --config examples/node1.toml   # dials node 0
#   ducktape-node --config examples/node2.toml --sync-only   # network joiner
#
# the assertion is four-part (each part diagnoses a distinct failure mode):
#   1. both GENESIS app-hashes agree      -> no pre-op fork (genesis determinism)
#   2. both CONVERGED app-hashes present  -> both nodes applied BOTH ops; a node
#      stuck at only its own op never prints (relay broken OR broadcast lost)
#   3. both CONVERGED app-hashes agree    -> real cross-process BFT convergence
#   4. converged != genesis               -> ops actually applied
#   5. the sync-only joiner's SYNCED hash equals the converged hash -> a fresh
#      process rebuilt every module purely over the statesync channel (real
#      network-backed state sync, not an in-process handoff)
set -euo pipefail
cd "$(dirname "$0")/.."

BIN=ducktape-node

echo "building $BIN..."
cargo build -p node-bin --bin "$BIN"
BIN_PATH="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')/debug/$BIN"

# wipe per-process storage roots: qmdb kv + the simplex journal persist to disk,
# so a stale run would start from divergent (non-empty) bases.
rm -rf /tmp/ducktape-node-0 /tmp/ducktape-node-1 /tmp/ducktape-node-2

log0=$(mktemp)
log1=$(mktemp)

echo "launching node 0 (bootstrapper) + node 1 (dialer)..."
"$BIN_PATH" --config examples/node0.toml >"$log0" 2>&1 &
pid0=$!
sleep 1
"$BIN_PATH" --config examples/node1.toml >"$log1" 2>&1 &
pid1=$!

# wait up to ~60s for BOTH nodes to log a converged app-hash.
status=1
for _ in $(seq 1 120); do
  if grep -q "converged app_hash=" "$log0" && grep -q "converged app_hash=" "$log1"; then
    status=0; break
  fi
  # bail early if either process died.
  if ! kill -0 "$pid0" 2>/dev/null || ! kill -0 "$pid1" 2>/dev/null; then break; fi
  sleep 0.5
done

# ---- the sync-only joiner: rebuild everything over the statesync channel ----
log2=$(mktemp)
synced=""
if [ "$status" -eq 0 ]; then
  echo "launching node 2 (sync-only joiner)..."
  if "$BIN_PATH" --config examples/node2.toml --sync-only >"$log2" 2>&1; then
    synced=$(grep -m1 -oE 'synced app_hash=[0-9a-f]+' "$log2" | cut -d= -f2 || true)
  fi
fi

kill "$pid0" "$pid1" 2>/dev/null || true
wait "$pid0" "$pid1" 2>/dev/null || true

echo "--- node 0 log ---"; cat "$log0"
echo "--- node 1 log ---"; cat "$log1"
echo "--- node 2 (joiner) log ---"; cat "$log2"

gen0=$(grep -m1 -oE 'genesis app_hash=[0-9a-f]+' "$log0" | cut -d= -f2 || true)
gen1=$(grep -m1 -oE 'genesis app_hash=[0-9a-f]+' "$log1" | cut -d= -f2 || true)
conv0=$(grep -m1 -oE 'converged app_hash=[0-9a-f]+' "$log0" | cut -d= -f2 || true)
conv1=$(grep -m1 -oE 'converged app_hash=[0-9a-f]+' "$log1" | cut -d= -f2 || true)

rm -f "$log0" "$log1" "$log2"

echo
echo "genesis:   node0=$gen0  node1=$gen1"
echo "converged: node0=$conv0  node1=$conv1"
echo "synced:    node2=$synced"

if [ -z "$gen0" ] || [ "$gen0" != "$gen1" ]; then
  echo "FAIL: genesis app-hashes disagree or missing (pre-op fork / genesis nondeterminism)"; exit 1
fi
if [ "$status" -ne 0 ] || [ -z "$conv0" ] || [ -z "$conv1" ]; then
  echo "FAIL: a node never converged within ~60s — it applied only its own op; the peer payload never delivered (relay broken or broadcast lost / mesh never formed / no quorum)"; exit 1
fi
if [ "$conv0" != "$conv1" ]; then
  echo "FAIL: converged app-hashes DISAGREE (cross-process fork): $conv0 != $conv1"; exit 1
fi
if [ "$conv0" = "$gen0" ]; then
  echo "FAIL: converged hash == genesis (nothing was actually applied)"; exit 1
fi

if [ -z "$synced" ]; then
  echo "FAIL: the sync-only joiner never printed a synced app-hash (statesync service or joiner flow broken)"; exit 1
fi
if [ "$synced" != "$conv0" ]; then
  echo "FAIL: the joiner synced app-hash DISAGREES with the validators: $synced != $conv0"; exit 1
fi

echo "PASS: both validators converged on byte-identical app-hash $conv0 over real TCP (off genesis $gen0), and the sync-only joiner rebuilt it over the statesync channel"
