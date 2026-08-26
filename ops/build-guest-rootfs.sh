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

# The GUEST's architecture: the host's, because there is no cross-hypervisor.
# An x86_64 Linux box boots x86_64 guests under Firecracker; an Apple silicon
# Mac boots aarch64 guests under the vz shim. `uname -m` says arm64 on macOS
# and aarch64 on Linux for the same thing.
ARCH="$(uname -m)"
[[ "$ARCH" == "arm64" ]] && ARCH=aarch64

# macOS: e2fsprogs is keg-only in Homebrew, so mke2fs never reaches PATH.
export PATH="$PATH:/opt/homebrew/opt/e2fsprogs/sbin:/usr/local/opt/e2fsprogs/sbin:/opt/homebrew/sbin"

# The Firecracker CI kernel: a known-good vmlinux with virtio-blk, -net and
# -vsock built in (the aarch64 build is an arm64 boot Image, which is also
# what VZLinuxBootLoader consumes). Building our own is an open question in
# the spec (it decides the CVE workflow); tracking this one is what unblocks
# everything else meanwhile.
KERNEL_URL="${KERNEL_URL:-https://s3.amazonaws.com/spec.ccfc.min/firecracker-ci/v1.12/$ARCH/vmlinux-6.1.128}"
BASE_URL="${BASE_URL:-https://s3.amazonaws.com/spec.ccfc.min/firecracker-ci/v1.12/$ARCH/ubuntu-24.04.squashfs}"

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
MUSL_TARGET="$ARCH-unknown-linux-musl"
INIT="$HERE/target/$MUSL_TARGET/release/duck-guest-init"
# ALWAYS, never "only if it is missing". cargo is already incremental, so this
# costs nothing when the source has not moved — while skipping it on an
# existing binary bakes a stale PID 1 into the image and the next boot silently
# runs last week's init.
say "building duck-guest-init (static musl, $MUSL_TARGET)"
# On a non-Linux host this is a cross build with no system musl toolchain;
# rust-lld + the rustup target's bundled musl libc link a static binary with
# nothing but `rustup target add`.
CROSS_FLAGS=""
[[ "$(uname -s)" == "Linux" ]] || CROSS_FLAGS="-C linker=rust-lld -C link-self-contained=yes"
(cd "$HERE" && RUSTFLAGS="${RUSTFLAGS:-} $CROSS_FLAGS" \
  cargo build -p duck-guest-init --release --target "$MUSL_TARGET")
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

  # The guest is Linux/$ARCH whatever the host is. On macOS every host CLI is
  # Mach-O and would fail inside the guest at exec, silently, as a run that
  # produces nothing — so anything that is not a Linux ELF binary is refused
  # here, where the fix (fetch the linux build of the CLI) can be named.
  magic="$(head -c 4 "$real" | od -An -tx1 | tr -d ' \n')"
  if [[ "$magic" != "7f454c46" ]]; then
    say "skipping $name: not a Linux ELF binary; put a linux/$ARCH build of it on PATH"
    continue
  fi
  install -m 0755 "$real" "$TREE/opt/duck/bin/$name"
  say "$name: $(du -h "$TREE/opt/duck/bin/$name" | cut -f1) <- $real"

  # A dynamically linked CLI needs its libraries present in the guest. The
  # base is a full Ubuntu userland, so glibc is already there; this reports
  # anything it is NOT, rather than shipping an image that fails at exec with
  # a message no one sees (the run just produces nothing). `ldd` only exists
  # on a Linux host; a cross-built image skips the check rather than failing.
  if command -v ldd >/dev/null; then
    if ldd "$real" 2>/dev/null | grep -q 'not found'; then
      echo "WARNING: $name has unresolved libraries on this host" >&2
    fi
    missing=0
    while read -r lib; do
      [[ -e "$TREE$lib" ]] || { echo "  MISSING in guest: $lib" >&2; missing=1; }
    done < <(ldd "$real" 2>/dev/null | awk '{for(i=1;i<=NF;i++) if($i ~ /^\//) print $i}' | sort -u)
    [[ $missing -eq 0 ]] || echo "WARNING: $name will not exec inside the guest" >&2
  fi
done

# a couple of ordinary tools a run's own shell-outs expect
for name in sh cat env; do
  [[ -e "$TREE/opt/duck/bin/$name" ]] || ln -sf "/bin/$name" "$TREE/opt/duck/bin/$name"
done

# ---- 5. mountpoints --------------------------------------------------------
# The per-run devices land here, and so do the tmpfs mounts the init makes for
# a read-only rootfs. They must EXIST in the image: mounting onto a missing
# directory fails, and creating one at boot fails too — the rootfs is read-only.
mkdir -p "$TREE/duck/workspace" "$TREE/agent" "$TREE/proc" "$TREE/sys" "$TREE/dev" \
         "$TREE/tmp" "$TREE/run" "$TREE/var/tmp" "$TREE/root"
chmod 1777 "$TREE/tmp" "$TREE/var/tmp"

# ---- 6. the image ----------------------------------------------------------
IMG="$OUT/rootfs.ext4"
rm -f "$IMG"
# measured tree + 64 MiB of slack: the image is READ-ONLY in every run, so it
# needs no growth room, only enough not to fail mke2fs on rounding.
# `du -sk` and not `-sb`: byte totals are a GNU extension and BSD du (macOS)
# has no -b; kibibytes over-count file bytes toward block usage, which only
# adds slack.
bytes=$(( $(du -sk "$TREE" | cut -f1) * 1024 ))
blocks=$(( (bytes + 64 * 1024 * 1024) / 4096 ))
say "building the rootfs image ($(( bytes / 1024 / 1024 )) MiB)"
mke2fs -q -t ext4 -b 4096 -d "$TREE" "$IMG" "$blocks"

# one runtime per OS: Firecracker over KVM, the vz shim over
# Virtualization.framework.
RUNTIME=firecracker
[[ "$(uname -s)" == "Darwin" ]] && RUNTIME=vz

say "rootfs: $(du -h "$IMG" | cut -f1) -> $IMG"
echo
echo "Point the node at these with:"
echo "  [sandbox]"
echo "  runtime = \"$RUNTIME\""
echo "  kernel  = \"$OUT/vmlinux\""
echo "  rootfs  = \"$IMG\""
