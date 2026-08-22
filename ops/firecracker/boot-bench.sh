#!/usr/bin/env bash
# Measure Firecracker cold-boot time for the sandbox backend.
#
# Every boot-time number in
#   docs/superpowers/plans/2026-08-22-firecracker-sandbox-backend.md
# came out of this script. Re-run it after changing the kernel, the rootfs or
# the boot args — the two wins it found (the i8042 group, `quiet`) are silent
# to lose: the run still works, it just gets slower.
#
#   ./boot-bench.sh                    # tuned cmdline across run shapes
#   MODE=compare ./boot-bench.sh       # before/after the cmdline tuning
#   MODE=memory ./boot-bench.sh        # the guest-RAM curve, host/guest split
#
# Artifacts default to $DIR; fetch them with --fetch on the first run.
#
# The user must be able to open /dev/kvm. If this session predates the
# `usermod -aG kvm`, wrap the call: `sg kvm -c ./boot-bench.sh`.
set -uo pipefail

DIR="${DIR:-${TMPDIR:-/tmp}/ducktape-fc-bench}"
N="${N:-5}"
MODE="${MODE:-shapes}"
CI="https://s3.amazonaws.com/spec.ccfc.min/firecracker-ci/v1.12/x86_64"

# The tuned command line. `acpi=off` is DELIBERATELY absent: it reads like a
# 69ms win and silently drops every vcpu but the boot one, because Firecracker
# enumerates vCPUs through ACPI.
TUNED="i8042.noaux i8042.nokbd i8042.nomux i8042.nopnp i8042.dumbkbd quiet loglevel=1"
# What an untuned cmdline looks like — `i8042.noaux` covers only the mouse port.
UNTUNED="i8042.noaux"
COMMON="console=ttyS0 reboot=k panic=-1 pci=off init=/bin/true"

fetch() {
  mkdir -p "$DIR"
  [[ -f "$DIR/vmlinux" ]] || curl -fsSL "$CI/vmlinux-6.1.128" -o "$DIR/vmlinux"
  [[ -f "$DIR/rootfs.squashfs" ]] || curl -fsSL "$CI/ubuntu-24.04.squashfs" -o "$DIR/rootfs.squashfs"
  echo "artifacts in $DIR"
}

[[ "${1:-}" == "--fetch" ]] && { fetch; exit 0; }

for f in vmlinux rootfs.squashfs; do
  [[ -f "$DIR/$f" ]] || { echo "missing $DIR/$f — run: $0 --fetch" >&2; exit 1; }
done
command -v firecracker >/dev/null || { echo "firecracker not on PATH — see ops/firecracker-setup.sh" >&2; exit 1; }
[[ -w /dev/kvm ]] || { echo "/dev/kvm is not writable; add yourself to the kvm group, or wrap this in: sg kvm -c '$0'" >&2; exit 1; }

# one boot; echoes elapsed milliseconds and leaves the console log at $DIR/last.log
boot() {
  local args="$1" mem="$2" vcpu="$3" cfg="$DIR/bench.json"
  cat > "$cfg" <<JSON
{ "boot-source": { "kernel_image_path": "$DIR/vmlinux", "boot_args": "$COMMON $args" },
  "drives": [{ "drive_id": "rootfs", "path_on_host": "$DIR/rootfs.squashfs",
               "is_root_device": true, "is_read_only": true }],
  "machine-config": { "vcpu_count": $vcpu, "mem_size_mib": $mem, "smt": false } }
JSON
  local s e
  s=$(date +%s%N)
  firecracker --no-api --config-file "$cfg" > "$DIR/last.log" 2>&1
  e=$(date +%s%N)
  echo $(( (e - s) / 1000000 ))
}

median() {
  local args="$1" mem="$2" vcpu="$3" times=()
  for _ in $(seq "$N"); do times+=("$(boot "$args" "$mem" "$vcpu")"); done
  printf '%s\n' "${times[@]}" | sort -n | awk -v n="$N" 'NR==int((n+1)/2)'
}

echo "firecracker $(firecracker --version 2>&1 | head -1 | awk '{print $2}')  n=$N  mode=$MODE"
echo

case "$MODE" in
shapes)
  printf "%-18s %10s\n" "run shape" "cold boot"
  for shape in 1:1024 2:2048 4:4096 8:8192 8:16384; do
    printf "%2s vcpu / %5s M %8s ms\n" "${shape%%:*}" "${shape#*:}" \
      "$(median "$TUNED" "${shape#*:}" "${shape%%:*}")"
  done
  ;;
compare)
  printf "%-18s %10s %10s %9s\n" "run shape" "untuned" "tuned" "saved"
  for shape in 1:1024 2:2048 4:4096 8:8192 8:16384; do
    vcpu=${shape%%:*}; mem=${shape#*:}
    b=$(median "$UNTUNED" "$mem" "$vcpu"); a=$(median "$TUNED" "$mem" "$vcpu")
    printf "%2s vcpu / %5s M %8s ms %8s ms %7s ms\n" "$vcpu" "$mem" "$b" "$a" "$(( b - a ))"
  done
  ;;
memory)
  # Splits wall clock into guest-side (kernel timestamp when init runs) and
  # host-side (the remainder: VMM process startup and teardown). The console
  # has to stay on to read the kernel timestamp, so these run untuned-quiet.
  printf "%8s %9s %11s %11s  %s\n" "guest" "wall" "guest-side" "host-side" "vcpus seen"
  for mem in 512 1024 2048 4096 8192 16384; do
    m=$(median "${TUNED/ quiet loglevel=1/}" "$mem" 2)
    g=$(grep -oE "^\[ *[0-9.]+\] Run " "$DIR/last.log" | grep -oE "[0-9]+\.[0-9]+" | head -1)
    gms=$(awk -v x="${g:-0}" 'BEGIN{printf "%d", x*1000}')
    cpus=$(grep -oE "Total of [0-9]+ processors" "$DIR/last.log" | grep -oE "[0-9]+")
    printf "%6s M %7s ms %8s ms %8s ms  %s\n" "$mem" "$m" "$gms" "$(( m - gms ))" "${cpus:-?}"
  done
  echo
  echo "Host-side should stay flat. If the guest-side column climbs with RAM,"
  echo "that is the kernel initialising its own page structures — grep the log"
  echo "for 'deferred pages initialised'. Only snapshot/restore skips it."
  ;;
*)
  echo "unknown MODE=$MODE (want: shapes | compare | memory)" >&2; exit 1 ;;
esac
