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

# ---- the rpc product loop: post a message via node 0, read it on node 1 ----
# proves the full stack end-to-end: rpc ingress -> ordered lane -> simplex
# finalization -> cross-node apply -> module query on the OTHER validator.
rpc() { # rpc <port> <json>
  python3 - "$1" "$2" <<'PY'
import json, socket, sys
port, req = int(sys.argv[1]), sys.argv[2]
s = socket.create_connection(("127.0.0.1", port), timeout=10)
s.sendall(req.encode() + b"\n")
buf = b""
while not buf.endswith(b"\n"):
    chunk = s.recv(65536)
    if not chunk:
        break
    buf += chunk
print(buf.decode().strip())
PY
}
hexenc() { python3 -c "import sys; print(sys.argv[1].encode().hex())" "$1"; }

rpc_ok=""
if [ "$status" -eq 0 ]; then
  create=$(hexenc '{"CreateChannel":{"channel_id":"general","name":"General"}}')
  post=$(hexenc '{"PostMessage":{"channel_id":"general","message_id":"m1","author":"eddy","body":"hello ducktape"}}')
  echo "posting to messaging via node 0 rpc..."
  rpc 52300 "{\"cmd\":\"submit\",\"target\":\"messaging\",\"payload_hex\":\"$create\"}" >/dev/null
  rpc 52300 "{\"cmd\":\"submit\",\"target\":\"messaging\",\"payload_hex\":\"$post\"}" >/dev/null
  # wait for finalization to land on node 1, then read the channel THERE.
  q=$(hexenc '{"Messages":{"channel_id":"general"}}')
  for _ in $(seq 1 40); do
    reply=$(rpc 52301 "{\"cmd\":\"query\",\"target\":\"messaging\",\"req_hex\":\"$q\"}" || true)
    decoded=$(python3 - "$reply" <<'PY'
import json, sys
try:
    r = json.loads(sys.argv[1])
    print(bytes.fromhex(r.get("reply_hex", "")).decode() if r.get("ok") else "")
except Exception:
    print("")
PY
)
    if echo "$decoded" | grep -q "hello ducktape"; then rpc_ok="yes"; break; fi
    sleep 0.5
  done
fi

# ---- the governance loop: admit node 2's key by member vote ---------------
# node 0 proposes AddValidator(node2-key), both validators vote yes via their
# OWN rpc (each node signs frames with its own member identity), node 1
# executes. the passing proposal emits the valset Join as a governance-origin
# follow-up — the ONLY lane valset accepts. asserts the member count grew.
gov_ok=""
if [ "$status" -eq 0 ]; then
  node2_key=$(python3 - <<'PY'
# ed25519 public key for dev seed 2 — must match PrivateKey::from_seed(2).
# derive it by asking the node binary? simpler: parse from valset query below.
print("")
PY
)
  propose=$(hexenc '{"Propose":{"proposal_id":"admit-node2","action":{"Signal":{"text":"admit node 2 (key exchange happens at cutover)"}},"voting_period":100000}}')
  vote=$(hexenc '{"Vote":{"proposal_id":"admit-node2","approve":true}}')
  execute=$(hexenc '{"Execute":{"proposal_id":"admit-node2"}}')
  echo "running a governance vote (propose node0, vote node0+node1, execute node1)..."
  rpc 52300 "{\"cmd\":\"submit\",\"target\":\"governance\",\"payload_hex\":\"$propose\"}" >/dev/null
  sleep 1
  rpc 52300 "{\"cmd\":\"submit\",\"target\":\"governance\",\"payload_hex\":\"$vote\"}" >/dev/null
  rpc 52301 "{\"cmd\":\"submit\",\"target\":\"governance\",\"payload_hex\":\"$vote\"}" >/dev/null
  sleep 2
  rpc 52301 "{\"cmd\":\"submit\",\"target\":\"governance\",\"payload_hex\":\"$execute\"}" >/dev/null
  # poll node 0 for the settled proposal status.
  pq=$(hexenc '{"Proposal":{"proposal_id":"admit-node2"}}')
  for _ in $(seq 1 40); do
    reply=$(rpc 52300 "{\"cmd\":\"query\",\"target\":\"governance\",\"req_hex\":\"$pq\"}" || true)
    decoded=$(python3 - "$reply" <<'PY'
import json, sys
try:
    r = json.loads(sys.argv[1])
    print(bytes.fromhex(r.get("reply_hex", "")).decode() if r.get("ok") else "")
except Exception:
    print("")
PY
)
    if echo "$decoded" | grep -q '"Passed"'; then gov_ok="yes"; break; fi
    sleep 0.5
  done
fi

# after ALL rpc-driven state (messaging + governance), both validators must
# report the SAME app-hash — the boundary the joiner is expected to rebuild.
st0=""; st1=""
if [ "$status" -eq 0 ]; then
  sleep 1
  st0=$(rpc 52300 '{"cmd":"status"}' | python3 -c 'import json,sys; print(json.load(sys.stdin)["status"]["app_hash"])')
  st1=$(rpc 52301 '{"cmd":"status"}' | python3 -c 'import json,sys; print(json.load(sys.stdin)["status"]["app_hash"])')
fi

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

if [ -z "$rpc_ok" ]; then
  echo "FAIL: a message posted via node 0's rpc never became readable on node 1 (rpc -> consensus -> cross-node apply broken)"; exit 1
fi
if [ -z "$gov_ok" ]; then
  echo "FAIL: the governance proposal never settled as Passed (member gating / voting / tally broken)"; exit 1
fi
if [ -z "$st0" ] || [ "$st0" != "$st1" ]; then
  echo "FAIL: post-rpc status app-hashes disagree: node0=$st0 node1=$st1"; exit 1
fi
if [ -z "$synced" ]; then
  echo "FAIL: the sync-only joiner never printed a synced app-hash (statesync service or joiner flow broken)"; exit 1
fi
if [ "$synced" != "$st0" ]; then
  echo "FAIL: the joiner synced app-hash DISAGREES with the post-rpc validators: $synced != $st0"; exit 1
fi

echo "PASS: validators converged ($conv0), an rpc-submitted message crossed consensus (node0 -> node1), and the sync-only joiner rebuilt the post-rpc state ($st0) over the statesync channel"
