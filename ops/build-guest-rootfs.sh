#!/usr/bin/env bash
# Build the shared guest artifacts every run's microVM boots from: the kernel
# and one read-only ext4 rootfs.
#
#   ops/build-guest-rootfs.sh                 # -> /var/lib/ducktape/guest
#   OUT=~/guest ops/build-guest-rootfs.sh     # anywhere writable
#
# ROOTLESS on purpose, start to finish. `unsquashfs -no-xattrs` extracts the
# base without needing privileges, the agent CLIs are copied in as ordinary
# files, and `mke2fs -d` builds the image without ever mounting it. A node that
# needs root to build its guest is a node that runs as root.
#
# What goes in:
#   /duck-guest-init      the static PID 1 (bin/duck-guest-init, musl)
#   /opt/duck/bin/*       the agent CLIs this node lends, copied from the host
#   /duck /agent          empty mountpoints for the per-run block devices
#
# What does NOT go in: any credential. The broker holds those on the host and
# the guest reaches it over vsock, so the image is safe to share across runs
# and across buyers — which is exactly why it can be read-only and shared.
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${OUT:-/var/lib/ducktape/guest}"
# Beside the output, never under /tmp: the extracted base is ~1 GB and /tmp on
# this class of host is both memory-backed and periodically reaped — a reaped
# cache silently turns every rebuild into a fresh 250 MB download.
WORK="${WORK:-$OUT/.build}"

# The Firecracker CI kernel: a known-good x86_64 vmlinux with virtio-blk,
# -net and -vsock built in. Building our own is an open question in the spec
# (it decides the CVE workflow); tracking this one is what unblocks everything
# else meanwhile.
KERNEL_URL="${KERNEL_URL:-https://s3.amazonaws.com/spec.ccfc.min/firecracker-ci/v1.12/x86_64/vmlinux-6.1.128}"
BASE_URL="${BASE_URL:-https://s3.amazonaws.com/spec.ccfc.min/firecracker-ci/v1.12/x86_64/ubuntu-24.04.squashfs}"

# the host binaries lent to runs. Each must be a real executable; a symlink is
# resolved so the image carries the target, not a dangling link.
#
# Set as a space-separated list: EXECUTORS="claude codex" ops/build-guest-rootfs.sh
read -r -a EXECUTORS <<< "${EXECUTORS:-claude codex}"

say() { printf '  %s\n' "$*"; }

command -v mke2fs >/dev/null || { echo "mke2fs not found; install e2fsprogs" >&2; exit 1; }
command -v unsquashfs >/dev/null || { echo "unsquashfs not found; install squashfs-tools" >&2; exit 1; }

mkdir -p "$OUT" "$WORK"

# ---- 1. the kernel ---------------------------------------------------------
if [[ ! -f "$OUT/vmlinux" ]]; then
  say "fetching the guest kernel"
  curl -fsSL "$KERNEL_URL" -o "$OUT/vmlinux.part"
  mv "$OUT/vmlinux.part" "$OUT/vmlinux"
fi
say "kernel: $(du -h "$OUT/vmlinux" | cut -f1)"

# ---- 2. the base filesystem ------------------------------------------------
BASE="$WORK/base.squashfs"
if [[ ! -f "$BASE" ]]; then
  say "fetching the base rootfs"
  curl -fsSL "$BASE_URL" -o "$BASE.part"
  mv "$BASE.part" "$BASE"
fi

TREE="$WORK/tree"
rm -rf "$TREE"
say "extracting the base"
# -no-xattrs: extracting capabilities/ACLs needs privileges we deliberately
# do not have. Nothing in the guest depends on them — PID 1 runs as root
# inside its own VM.
unsquashfs -no-xattrs -quiet -force -dest "$TREE" "$BASE"

# ---- 3. the init -----------------------------------------------------------
INIT="$HERE/target/x86_64-unknown-linux-musl/release/duck-guest-init"
if [[ ! -x "$INIT" ]]; then
  say "building duck-guest-init (static musl)"
  (cd "$HERE" && cargo build -p duck-guest-init --release --target x86_64-unknown-linux-musl)
fi
install -m 0755 "$INIT" "$TREE/duck-guest-init"
say "init: $(du -h "$TREE/duck-guest-init" | cut -f1) static"

# ---- 4. the agent CLIs -----------------------------------------------------
mkdir -p "$TREE/opt/duck/bin"
for name in "${EXECUTORS[@]}"; do
  host_bin="$(command -v "$name" || true)"
  if [[ -z "$host_bin" ]]; then
    say "skipping $name (not on PATH)"
    continue
  fi
  # resolve: the launcher is usually a symlink into a versioned directory
  real="$(readlink -f "$host_bin")"
  install -m 0755 "$real" "$TREE/opt/duck/bin/$name"
  say "$name: $(du -h "$TREE/opt/duck/bin/$name" | cut -f1) <- $real"

  # A dynamically linked CLI needs its libraries present in the guest. The
  # base is a full Ubuntu userland, so glibc is already there; this reports
  # anything it is NOT, rather than shipping an image that fails at exec with
  # a message no one sees (the run just produces nothing).
  if ldd "$real" 2>/dev/null | grep -q 'not found'; then
    echo "WARNING: $name has unresolved libraries on this host" >&2
  fi
  missing=0
  while read -r lib; do
    [[ -e "$TREE$lib" ]] || { echo "  MISSING in guest: $lib" >&2; missing=1; }
  done < <(ldd "$real" 2>/dev/null | awk '{for(i=1;i<=NF;i++) if($i ~ /^\//) print $i}' | sort -u)
  [[ $missing -eq 0 ]] || echo "WARNING: $name will not exec inside the guest" >&2
done

# a couple of ordinary tools a run's own shell-outs expect
for name in sh cat env; do
  [[ -e "$TREE/opt/duck/bin/$name" ]] || ln -sf "/bin/$name" "$TREE/opt/duck/bin/$name"
done

# ---- 5. mountpoints --------------------------------------------------------
# The per-run devices land here. They must EXIST in the image: mounting onto a
# missing directory fails, and the guest init reports it to a console nobody is
# reading yet.
mkdir -p "$TREE/duck/workspace" "$TREE/agent" "$TREE/proc" "$TREE/sys" "$TREE/dev"

# ---- 6. the image ----------------------------------------------------------
IMG="$OUT/rootfs.ext4"
rm -f "$IMG"
# measured tree + 64 MiB of slack: the image is READ-ONLY in every run, so it
# needs no growth room, only enough not to fail mke2fs on rounding.
bytes=$(du -sb "$TREE" | cut -f1)
blocks=$(( (bytes + 64 * 1024 * 1024) / 4096 ))
say "building the rootfs image ($(numfmt --to=iec "$bytes"))"
mke2fs -q -t ext4 -b 4096 -d "$TREE" "$IMG" "$blocks"

say "rootfs: $(du -h "$IMG" | cut -f1) -> $IMG"
echo
echo "Point the node at these with:"
echo "  [sandbox]"
echo "  runtime = \"firecracker\""
echo "  kernel  = \"$OUT/vmlinux\""
echo "  rootfs  = \"$IMG\""
