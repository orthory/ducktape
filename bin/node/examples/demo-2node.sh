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
#   6. a governance-passed AddValidator triggers a LIVE EPOCH CUTOVER on both
#      validators (engine teardown + respawn over the 3-member set), and the
#      epoch-1 network still finalizes rpc-submitted ops
set -euo pipefail
cd "$(dirname "$0")/.."

BIN=ducktape-node

echo "building $BIN..."
cargo build -p node-bin --bin "$BIN"
BIN_PATH="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')/debug/$BIN"

# wipe per-process storage roots: qmdb kv + the simplex journal persist to disk,
# so a stale run would start from divergent (non-empty) bases.
rm -rf /tmp/ducktape-node-0 /tmp/ducktape-node-1 /tmp/ducktape-node-2 /tmp/ducktape-node-3

log0=$(mktemp)
log1=$(mktemp)
logv2=$(mktemp)

echo "launching node 0 (bootstrapper) + node 1 + node 2 (validators)..."
"$BIN_PATH" --config examples/node0.toml >"$log0" 2>&1 &
pid0=$!
sleep 1
"$BIN_PATH" --config examples/node1.toml >"$log1" 2>&1 &
pid1=$!
"$BIN_PATH" --config examples/node2.toml >"$logv2" 2>&1 &
pidv2=$!

# wait up to ~60s for ALL THREE validators to log a converged app-hash.
status=1
for _ in $(seq 1 120); do
  if grep -q "converged app_hash=" "$log0" && grep -q "converged app_hash=" "$log1" && grep -q "converged app_hash=" "$logv2"; then
    status=0; break
  fi
  # bail early if any process died.
  if ! kill -0 "$pid0" 2>/dev/null || ! kill -0 "$pid1" 2>/dev/null || ! kill -0 "$pidv2" 2>/dev/null; then break; fi
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
  create=$(hexenc '{"CreateChannel":{"channel_id":"general","name":"General","post_policy":"Open"}}')
  post=$(hexenc '{"PostMessage":{"channel_id":"general","message_id":"m1","blocks":[{"Paragraph":[{"text":"hello ducktape","marks":[]}]}],"thread":null,"as_agent":null}}')
  echo "posting to chat via node 0 rpc..."
  rpc 52300 "{\"cmd\":\"submit\",\"target\":\"chat\",\"payload_hex\":\"$create\"}" >/dev/null
  rpc 52300 "{\"cmd\":\"submit\",\"target\":\"chat\",\"payload_hex\":\"$post\"}" >/dev/null
  # wait for finalization to land on node 1, then read the channel THERE.
  q=$(hexenc '{"MessagesLatest":{"channel_id":"general","limit":10}}')
  for _ in $(seq 1 40); do
    reply=$(rpc 52301 "{\"cmd\":\"query\",\"target\":\"chat\",\"req_hex\":\"$q\"}" || true)
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
cutover_ok=""
post_cutover_ok=""
if [ "$status" -eq 0 ]; then
  # node 3's identity, printed by node 0 at startup (deterministic dev seed 3).
  node3_key=$(grep -m1 -oE 'peer\[3\] identity=[0-9a-f]+' "$log0" | cut -d= -f2)
  if [ -z "$node3_key" ]; then echo "FAIL: node 3 identity not found in node 0 log"; exit 1; fi
  propose=$(python3 - "$node3_key" <<'PY'
import json, sys
key = list(bytes.fromhex(sys.argv[1]))
req = {"Propose": {"proposal_id": "admit-node3",
                   "action": {"AddValidator": {"key": key}},
                   "voting_period": 100000}}
print(json.dumps(req, separators=(",", ":")).encode().hex())
PY
)
  vote=$(hexenc '{"Vote":{"proposal_id":"admit-node3","approve":true}}')
  execute=$(hexenc '{"Execute":{"proposal_id":"admit-node3"}}')
  echo "running a governance vote (propose node0, vote node0+node1, execute node1)..."
  rpc 52300 "{\"cmd\":\"submit\",\"target\":\"governance\",\"payload_hex\":\"$propose\"}" >/dev/null
  sleep 1
  rpc 52300 "{\"cmd\":\"submit\",\"target\":\"governance\",\"payload_hex\":\"$vote\"}" >/dev/null
  rpc 52301 "{\"cmd\":\"submit\",\"target\":\"governance\",\"payload_hex\":\"$vote\"}" >/dev/null
  sleep 2
  rpc 52301 "{\"cmd\":\"submit\",\"target\":\"governance\",\"payload_hex\":\"$execute\"}" >/dev/null
  # poll node 0 for the settled proposal status.
  pq=$(hexenc '{"Proposal":{"proposal_id":"admit-node3"}}')
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

  # ---- LIVE EPOCH CUTOVER: the passed AddValidator changed the valset ------
  # push filler ops so finalized views advance past the scheduled cutover
  # (observe + CUTOVER_DELAY), then wait for both validators to respawn.
  if [ -n "$gov_ok" ]; then
    echo "advancing views past the scheduled cutover..."
    for i in $(seq 1 6); do
      filler=$(hexenc "{\"Set\":{\"key\":\"cutover-filler-$i\",\"value\":\"x\"}}")
      rpc 52300 "{\"cmd\":\"submit\",\"target\":\"directory\",\"payload_hex\":\"$filler\"}" >/dev/null
      sleep 0.5
    done
    for _ in $(seq 1 60); do
      if grep -q "cutover complete: epoch 1" "$log0" && grep -q "cutover complete: epoch 1" "$log1" && grep -q "cutover complete: epoch 1" "$logv2"; then
        cutover_ok="yes"; break
      fi
      sleep 0.5
    done
    # the epoch-1 network must still finalize ops: post a message via node 0,
    # read it via node 1 — through the RESPAWNED engines.
    if [ -n "$cutover_ok" ]; then
      post2=$(hexenc '{"PostMessage":{"channel_id":"general","message_id":"m2","blocks":[{"Paragraph":[{"text":"epoch one lives","marks":[]}]}],"thread":null,"as_agent":null}}')
      rpc 52300 "{\"cmd\":\"submit\",\"target\":\"chat\",\"payload_hex\":\"$post2\"}" >/dev/null
      q2=$(hexenc '{"MessagesLatest":{"channel_id":"general","limit":10}}')
      for _ in $(seq 1 40); do
        reply=$(rpc 52302 "{\"cmd\":\"query\",\"target\":\"chat\",\"req_hex\":\"$q2\"}" || true)
        decoded=$(python3 - "$reply" <<'PY'
import json, sys
try:
    r = json.loads(sys.argv[1])
    print(bytes.fromhex(r.get("reply_hex", "")).decode() if r.get("ok") else "")
except Exception:
    print("")
PY
)
        if echo "$decoded" | grep -q "epoch one lives"; then post_cutover_ok="yes"; break; fi
        sleep 0.5
      done
    fi
  fi
fi

# after ALL rpc-driven state (chat + governance), both validators must
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
  echo "launching node 3 (sync-only joiner)..."
  if "$BIN_PATH" --config examples/node3.toml --sync-only >"$log2" 2>&1; then
    synced=$(grep -m1 -oE 'synced app_hash=[0-9a-f]+' "$log2" | cut -d= -f2 || true)
  fi
fi

kill "$pid0" "$pid1" "$pidv2" 2>/dev/null || true
wait "$pid0" "$pid1" "$pidv2" 2>/dev/null || true

echo "--- node 0 log (tail) ---"; tail -20 "$log0"
echo "--- node 1 log (tail) ---"; tail -12 "$log1"
echo "--- node 2 log (tail) ---"; tail -12 "$logv2"
echo "--- node 3 (joiner) log ---"; cat "$log2"

gen0=$(grep -m1 -oE 'genesis app_hash=[0-9a-f]+' "$log0" | cut -d= -f2 || true)
gen1=$(grep -m1 -oE 'genesis app_hash=[0-9a-f]+' "$log1" | cut -d= -f2 || true)
conv0=$(grep -m1 -oE 'converged app_hash=[0-9a-f]+' "$log0" | cut -d= -f2 || true)
conv1=$(grep -m1 -oE 'converged app_hash=[0-9a-f]+' "$log1" | cut -d= -f2 || true)

rm -f "$log0" "$log1" "$logv2" "$log2"

echo
echo "genesis:   node0=$gen0  node1=$gen1"
echo "converged: node0=$conv0  node1=$conv1"
echo "synced:    node3=$synced"

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
if [ -z "$cutover_ok" ]; then
  echo "FAIL: the validators never completed the epoch-1 cutover after the AddValidator passed (orchestrator/respawn broken)"; exit 1
fi
if [ -z "$post_cutover_ok" ]; then
  echo "FAIL: an op submitted after the cutover never crossed consensus (the epoch-1 engines are not live)"; exit 1
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

echo "PASS: validators converged ($conv0), rpc + governance + LIVE EPOCH CUTOVER all crossed consensus (epoch-1 finalizes ops), and the sync-only joiner rebuilt the final state ($st0) over the statesync channel"
