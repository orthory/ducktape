#!/usr/bin/env bash
# How fast is a Firecracker snapshot restore, and what does it cost?
#
# Context: cold boot on the dev host is ~430ms tuned. The widely-quoted
# "~125ms microVM boot" is not a cold boot at all — vendors snapshot a booted
# VM and restore it per run. This measures that path rather than arguing about
# the number, and it also measures the two costs the quoted figure hides:
# snapshot CREATION time and the memory file's size on disk.
#
#   ./snapshot-bench.sh              # 512 MiB
#   MEM=8192 ./snapshot-bench.sh     # any guest size
#
# Needs the artifacts from boot-bench.sh --fetch, plus a tiny init that parks
# in userspace (built here if absent — a snapshot needs a VM sitting at a
# steady point, not one racing to exit).
set -uo pipefail

DIR="${DIR:-${TMPDIR:-/tmp}/ducktape-fc-bench}"
MEM="${MEM:-512}"
N="${N:-5}"

# The API socket CANNOT live beside the artifacts: a unix socket path is capped
# near 108 bytes (SUN_LEN) and a scratch dir under a long home blows straight
# through it with `path must be shorter than SUN_LEN`. Same trap the podman
# service hit — see the socket_path comment in bin/node/src/services.rs.
SOCKDIR="${XDG_RUNTIME_DIR:-/tmp}/dtfc"
mkdir -p "$SOCKDIR"
API="$SOCKDIR/fc.sock"
STATE="$DIR/snap.state"
MEMFILE="$DIR/snap.mem"

BOOT_ARGS="console=ttyS0 reboot=k panic=-1 pci=off quiet loglevel=1 \
i8042.noaux i8042.nokbd i8042.nomux i8042.nopnp i8042.dumbkbd"

command -v firecracker >/dev/null || { echo "firecracker not on PATH — see ops/firecracker-setup.sh" >&2; exit 1; }
[[ -w /dev/kvm ]] || { echo "/dev/kvm not writable; wrap this in: sg kvm -c '$0'" >&2; exit 1; }
[[ -f "$DIR/vmlinux" ]] || { echo "missing $DIR/vmlinux — run: ./boot-bench.sh --fetch" >&2; exit 1; }

cleanup() { [[ -n "${FC_PID:-}" ]] && kill "$FC_PID" 2>/dev/null; rm -f "$API"; }
trap cleanup EXIT

# ---- the parked init ------------------------------------------------------
if [[ ! -f "$DIR/initramfs-pause.cpio.gz" ]]; then
  tmp="$DIR/ird-pause"; mkdir -p "$tmp"
  cat > "$tmp/init.c" <<'C'
#include <unistd.h>
int main(void) { if (write(1, "DUCKTAPE_READY\n", 15)) {} for (;;) pause(); }
C
  gcc -static -Os -o "$tmp/init" "$tmp/init.c" || { echo "need gcc to build the parked init" >&2; exit 1; }
  rm -f "$tmp/init.c"
  (cd "$tmp" && find . -print0 | cpio --null -o --format=newc 2>/dev/null | gzip -9 > "$DIR/initramfs-pause.cpio.gz")
fi

api() {
  curl -sS --unix-socket "$API" -X "$1" "http://localhost$2" \
       -H 'Content-Type: application/json' ${3:+-d "$3"} -w '%{http_code}' -o /dev/null
}

rm -f "$API" "$STATE" "$MEMFILE"

# ---- 1. boot to userspace -------------------------------------------------
firecracker --api-sock "$API" > "$DIR/snap-boot.log" 2>&1 &
FC_PID=$!
timeout 5 bash -c "until [ -S '$API' ]; do :; done" || { echo "no api socket"; head -3 "$DIR/snap-boot.log"; exit 1; }

api PUT /machine-config "{\"vcpu_count\":2,\"mem_size_mib\":$MEM,\"smt\":false}" >/dev/null
api PUT /boot-source "{\"kernel_image_path\":\"$DIR/vmlinux\",\"initrd_path\":\"$DIR/initramfs-pause.cpio.gz\",\"boot_args\":\"$BOOT_ARGS\"}" >/dev/null
api PUT /actions '{"action_type":"InstanceStart"}' >/dev/null

timeout 15 grep -q -m1 DUCKTAPE_READY <(tail -f "$DIR/snap-boot.log") \
  || { echo "guest never reached userspace"; tail -5 "$DIR/snap-boot.log"; exit 1; }

# ---- 2. pause and snapshot ------------------------------------------------
t0=$(date +%s%N)
api PATCH /vm '{"state":"Paused"}' >/dev/null
code=$(api PUT /snapshot/create "{\"snapshot_type\":\"Full\",\"snapshot_path\":\"$STATE\",\"mem_file_path\":\"$MEMFILE\"}")
t1=$(date +%s%N)
[[ "$code" == "204" ]] || { echo "snapshot/create -> HTTP $code"; tail -5 "$DIR/snap-boot.log"; exit 1; }

kill "$FC_PID" 2>/dev/null; wait "$FC_PID" 2>/dev/null; FC_PID=""; rm -f "$API"

# ---- 3. restore, N times --------------------------------------------------
restore_once() {
  local s e pid
  s=$(date +%s%N)
  firecracker --api-sock "$API" > "$DIR/snap-restore.log" 2>&1 &
  pid=$!
  until [ -S "$API" ]; do :; done
  curl -sS --unix-socket "$API" -X PUT "http://localhost/snapshot/load" \
    -H 'Content-Type: application/json' \
    -d "{\"snapshot_path\":\"$STATE\",\"mem_backend\":{\"backend_path\":\"$MEMFILE\",\"backend_type\":\"File\"},\"resume_vm\":true}" \
    -o /dev/null
  e=$(date +%s%N)
  kill "$pid" 2>/dev/null; wait "$pid" 2>/dev/null; rm -f "$API"
  echo $(( (e - s) / 1000000 ))
}

times=()
for _ in $(seq "$N"); do times+=("$(restore_once)"); done
med=$(printf '%s\n' "${times[@]}" | sort -n | awk -v n="$N" 'NR==int((n+1)/2)')

printf "guest %s MiB\n" "$MEM"
printf "  snapshot create      %6s ms\n" "$(( (t1-t0)/1000000 ))"
printf "  memory file on disk  %6s\n"    "$(du -h "$MEMFILE" | cut -f1)"
printf "  restore -> resumed   %6s ms   (%s)\n" "$med" "${times[*]}"
echo
echo "  Restore is FLAT in guest memory because the File backend mmaps the"
echo "  memory file and faults pages in lazily. 'Resumed' is not 'warm': the"
echo "  first real work the guest does pays that faulting. Production setups"
echo "  use the UFFD backend to control it. Measure before promising latency"
echo "  to a buyer."
