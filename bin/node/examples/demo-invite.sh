#!/usr/bin/env bash
# the network-shape onboarding ceremony over real sockets — no seeds, no
# hand-written configs:
#
#   founder: ducktape node init --name demo          -> chain-id + identity
#            ducktape node invite                    -> one-line paste blob
#   friend:  ducktape node join <blob>               -> workspace + identity
#            (sends the printed pubkey back)
#   founder: ducktape node admit <pubkey>            -> pre-genesis membership
#            ducktape node invite                    -> REFRESHED blob
#   friend:  ducktape node join <refreshed blob>     -> now a member
#   both:    ducktape node --config .../node.toml    -> one network
#
# the assertion: both identities boot the SAME genesis app-hash (identical
# descriptor -> identical genesis), an op submitted on the founder's node is
# readable on the friend's (2-validator quorum crossed real TCP), and both
# status app-hashes agree afterward.
set -euo pipefail
cd "$(dirname "$0")/.."
command -v bun >/dev/null || { echo "bun is required" >&2; exit 1; }
command -v nc  >/dev/null || { echo "nc is required" >&2; exit 1; }

BIN=ducktape
echo "building $BIN..."
cargo build -p node-bin --bin "$BIN" >/dev/null 2>&1
BIN_PATH="$(cargo metadata --no-deps --format-version 1 | bun -e 'console.log((await Bun.stdin.json()).target_directory)')/debug/$BIN"

WORK=$(mktemp -d)
A="$WORK/founder"
B="$WORK/friend"
cleanup() { pkill -P $$ 2>/dev/null || true; }
trap cleanup EXIT

echo "founder: init..."
chain_id=$("$BIN_PATH" node init --name demo --dir "$A" \
  --listen 127.0.0.1:53200 --advertised 127.0.0.1:53200 \
  --rpc 127.0.0.1:53300 2>/dev/null)
echo "  chain-id: $chain_id"
invite=$("$BIN_PATH" node invite --config "$A/node.toml" 2>/dev/null)

echo "friend: join (first pass — identity only)..."
friend_key=$("$BIN_PATH" node join "$invite" --dir "$B" \
  --listen 127.0.0.1:53201 --advertised 127.0.0.1:53201 \
  --rpc 127.0.0.1:53301 2>/dev/null)
echo "  friend identity: $friend_key"

echo "founder: admit + refreshed invite..."
"$BIN_PATH" node admit "$friend_key" --config "$A/node.toml" 2>/dev/null
invite2=$("$BIN_PATH" node invite --config "$A/node.toml" 2>/dev/null)

echo "friend: join (refreshed — now a member)..."
"$BIN_PATH" node join "$invite2" --dir "$B" \
  --listen 127.0.0.1:53201 --advertised 127.0.0.1:53201 \
  --rpc 127.0.0.1:53301 >/dev/null 2>&1

loga=$(mktemp)
logb=$(mktemp)
echo "launching both members..."
"$BIN_PATH" node --config "$A/node.toml" >"$loga" 2>&1 &
pa=$!
"$BIN_PATH" node --config "$B/node.toml" >"$logb" 2>&1 &
pb=$!

rpc() { # rpc <port> <json>
  printf '%s\n' "$2" | nc -w 10 127.0.0.1 "$1"
}
hexenc() { printf '%s' "$1" | od -An -tx1 -v | tr -d ' \n'; }

# identical descriptor -> identical genesis app-hash on both.
genesis_ok=""
for _ in $(seq 1 60); do
  ga=$(grep -m1 -oE 'genesis app_hash=[0-9a-f]+' "$loga" | cut -d= -f2 || true)
  gb=$(grep -m1 -oE 'genesis app_hash=[0-9a-f]+' "$logb" | cut -d= -f2 || true)
  if [ -n "$ga" ] && [ -n "$gb" ]; then
    [ "$ga" = "$gb" ] && genesis_ok="yes"
    break
  fi
  if ! kill -0 "$pa" 2>/dev/null || ! kill -0 "$pb" 2>/dev/null; then break; fi
  sleep 0.5
done
if [ -z "$genesis_ok" ]; then
  echo "FAIL: genesis hashes absent or diverged (founder=$ga friend=$gb)"
  tail -n 5 "$loga" "$logb"
  exit 1
fi
echo "genesis agreed: $ga"

# an op submitted on the founder finalizes across the 2-validator quorum and
# is readable on the friend.
set_op=$(hexenc '{"set":{"key":"ceremony","value":"two members, zero seeds"}}')
get_q=$(hexenc '{"get":{"key":"ceremony"}}')
rpc 53300 "{\"cmd\":\"submit\",\"target\":\"directory\",\"payload_hex\":\"$set_op\"}" >/dev/null
converge_ok=""
for _ in $(seq 1 60); do
  reply=$(rpc 53301 "{\"cmd\":\"query\",\"target\":\"directory\",\"req_hex\":\"$get_q\"}" || true)
  decoded=$(printf '%s' "$reply" | bun -e '
try {
  const response = await Bun.stdin.json();
  if (response.ok) process.stdout.write(Buffer.from(response.reply_hex ?? "", "hex").toString());
} catch {}
')
  if echo "$decoded" | grep -q "two members, zero seeds"; then converge_ok="yes"; break; fi
  sleep 0.5
done
if [ -z "$converge_ok" ]; then
  echo "FAIL: the founder's op never became readable on the friend"
  tail -n 5 "$loga" "$logb"
  exit 1
fi

# both status app-hashes agree at the settled boundary.
hash_of() { # hash_of <port>
  rpc "$1" '{"cmd":"status"}' | bun -e 'console.log((await Bun.stdin.json()).status.app_hash)'
}
final_ok=""
for _ in $(seq 1 20); do
  ha=$(hash_of 53300)
  hb=$(hash_of 53301)
  if [ "$ha" = "$hb" ]; then final_ok="yes"; break; fi
  sleep 0.5
done
if [ -z "$final_ok" ]; then
  echo "FAIL: status app-hashes diverged (founder=$ha friend=$hb)"
  exit 1
fi

echo
echo "PASS: $chain_id founded, a friend joined by invite + pre-genesis admit,"
echo "      both converged on app_hash=$ha over real TCP with keygen'd identities"
