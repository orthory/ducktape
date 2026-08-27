#!/usr/bin/env bash
# Build the shared guest artifacts every run's microVM boots from: the kernel
# and one read-only ext4 rootfs.
#
#   ops/build-guest-rootfs.sh                 # -> ~/.ducktape/guest
#   OUT=~/guest ops/build-guest-rootfs.sh     # anywhere writable
#
# The default is where `node init` writes a fresh [sandbox] table
# (workspace-config's `default_guest_dir`) — build here and the node already
# points at it. Under the operator's home because this build is rootless.
#
# ROOTLESS on purpose, start to finish. `unsquashfs -no-xattrs` extracts the
# base without needing privileges, the agent CLIs are copied in as ordinary
# files, and `mke2fs -d` builds the image without ever mounting it. A node that
# needs root to build its guest is a node that runs as root.
#
# THIS SCRIPT FETCHES NO EXECUTABLE. The kernel and the base rootfs come from
# their upstreams; the agent CLIs come from the executors directory, which only
# `ducktape agent install` (an operator's explicit act) writes to.
#
# What goes in:
#   /duck-guest-init      the static PID 1 (bin/duck-guest-init, musl)
#   /opt/duck/bin/*       the agent CLIs this node lends, from ~/.ducktape/executors
#   /duck /agent          empty mountpoints for the per-run block devices
#
# What does NOT go in: any credential. The broker holds those on the host and
# the guest reaches it over vsock, so the image is safe to share across runs
# and across buyers — which is exactly why it can be read-only and shared.
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${OUT:-${DUCKTAPE_HOME:-$HOME/.ducktape}/guest}"
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

# macOS: e2fsprogs is keg-only in Homebrew, so mke2fs never reaches PATH —
# and in a non-login shell (ssh command, make, cron) brew's own bin dirs
# don't either.
export PATH="$PATH:/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:/opt/homebrew/opt/e2fsprogs/sbin:/usr/local/opt/e2fsprogs/sbin"

# The guest kernel differs per HYPERVISOR, not per taste: Firecracker attaches
# virtio over MMIO and its CI kernel carries only those drivers, while
# Virtualization.framework attaches virtio over PCI — measured, the CI kernel
# under VZ boots to a silent black hole (no console, no disks, no vsock). So
# Linux fetches the Firecracker CI kernel and macOS extracts the Kata
# Containers VM kernel (virtio-pci and -mmio both built in, boots a rootfs
# with no initrd). `KERNEL_URL` overrides either. Building our own kernel is
# an open question in the spec (it decides the CVE workflow); tracking these
# is what unblocks everything else meanwhile.
KERNEL_URL="${KERNEL_URL:-}"
[[ "$(uname -s)" == "Linux" && -z "$KERNEL_URL" ]] \
  && KERNEL_URL="https://s3.amazonaws.com/spec.ccfc.min/firecracker-ci/v1.12/$ARCH/vmlinux-6.1.128"
KATA_VERSION="${KATA_VERSION:-4.1.0}"
BASE_URL="${BASE_URL:-https://s3.amazonaws.com/spec.ccfc.min/firecracker-ci/v1.12/$ARCH/ubuntu-24.04.squashfs}"

# the agent CLIs lent to runs, taken from the executors directory — NOT from
# the host's PATH.
#
# A run executes inside a Linux microVM on every host, so this binary must be a
# Linux build for the GUEST's architecture. The host's own CLI is that only on
# Linux, by coincidence; on macOS it is Mach-O and the guest cannot exec it at
# all. Reading the host PATH therefore meant "works on Linux, silently produces
# an unusable image on a Mac" — and pointed the operator at an instruction they
# could not carry out ("put a linux/$ARCH build of it on PATH", on a machine
# where such a binary is not a host tool and does not belong there).
#
# `ducktape agent install` owns that directory and the one approved way to fill
# it. This script never fetches anything: a missing executor prints the command
# and is skipped.
#
# Set as a space-separated list: EXECUTORS="codex" ops/build-guest-rootfs.sh
read -r -a EXECUTORS <<< "${EXECUTORS:-claude codex}"
EXEC_DIR="${DUCKTAPE_EXECUTOR_DIR:-${DUCKTAPE_HOME:-$HOME/.ducktape}/executors}"

say() { printf '  %s\n' "$*"; }

# macOS ships `shasum`; most Linux images ship `sha256sum` and not always both.
sha256_of() {
  if command -v sha256sum >/dev/null; then sha256sum "$1" | awk '{print $1}'
  else shasum -a 256 "$1" | awk '{print $1}'; fi
}

command -v mke2fs >/dev/null || { echo "mke2fs not found; install e2fsprogs" >&2; exit 1; }
command -v unsquashfs >/dev/null || { echo "unsquashfs not found; install squashfs-tools" >&2; exit 1; }

mkdir -p "$OUT" "$WORK"

# ---- 1. the kernel ---------------------------------------------------------
# extract the plain VM kernel out of Kata's static release bundle. The
# tarball is ~600 MB for a ~40 MB kernel, so it is cached in WORK; the plain
# kernel is the one whose name is bare `vmlinux-<version>` (the gpu/debug/
# dragonball variants carry suffixes).
fetch_kata_kernel() {
  command -v zstd >/dev/null || { echo "zstd not found; install zstd (brew install zstd)" >&2; exit 1; }
  local kata_arch=amd64
  [[ "$ARCH" == "aarch64" ]] && kata_arch=arm64
  local tarball="$WORK/kata-static-$KATA_VERSION-$kata_arch.tar.zst"
  if [[ ! -f "$tarball" ]]; then
    say "fetching the Kata VM kernel bundle ($kata_arch)"
    curl -fsSL "https://github.com/kata-containers/kata-containers/releases/download/$KATA_VERSION/kata-static-$KATA_VERSION-$kata_arch.tar.zst" \
      -o "$tarball.part"
    mv "$tarball.part" "$tarball"
  fi
  local member
  member=$(zstd -dc "$tarball" | tar -tf - | grep -E 'share/kata-containers/vmlinux-[0-9]+\.[0-9.]+-[0-9]+$' | head -1)
  [[ -n "$member" ]] || { echo "no plain vmlinux in the Kata bundle" >&2; exit 1; }
  say "extracting $(basename "$member")"
  zstd -dc "$tarball" | tar -xf - -C "$WORK" "$member"
  install -m 0644 "$WORK/$member" "$OUT/vmlinux"
}

if [[ ! -f "$OUT/vmlinux" ]]; then
  if [[ -n "$KERNEL_URL" ]]; then
    say "fetching the guest kernel"
    curl -fsSL "$KERNEL_URL" -o "$OUT/vmlinux.part"
    mv "$OUT/vmlinux.part" "$OUT/vmlinux"
  else
    fetch_kata_kernel
  fi
fi
# On macOS the kernel MUST speak virtio-pci — without it every VZ device is
# invisible and the boot is a silent hang, which is the single most
# expensive failure to diagnose from outside. Refuse it here, where the fix
# (delete the kernel, rerun, or point KERNEL_URL at a PCI-capable one) is
# printable.
# `grep -c`, not `-q`: -q closes the pipe on the first match, strings dies
# of SIGPIPE, and under `pipefail` a KERNEL THAT PASSES gets refused (141).
if [[ "$(uname -s)" == "Darwin" ]] && [[ "$(strings "$OUT/vmlinux" | grep -c virtio_pci)" == "0" ]]; then
  echo "$OUT/vmlinux has no virtio-pci support and cannot see any VZ device;" >&2
  echo "remove it and re-run (Kata kernel), or set KERNEL_URL to a PCI-capable kernel" >&2
  exit 1
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
  real="$EXEC_DIR/$name"
  if [[ ! -x "$real" ]]; then
    say "skipping $name (not installed)"
    say "  install it: ducktape agent install $name"
    continue
  fi

  # Still checked, because the operator may have put this file here by hand:
  # a non-ELF binary fails inside the guest at exec, silently, as a run that
  # produces nothing. Refuse it here, where the fix can be named.
  magic="$(head -c 4 "$real" | od -An -tx1 | tr -d ' \n')"
  if [[ "$magic" != "7f454c46" ]]; then
    say "skipping $name: $real is not a Linux ELF binary"
    say "  replace it: ducktape agent install $name"
    continue
  fi
  install -m 0755 "$real" "$TREE/opt/duck/bin/$name"
  # The sha256 of what actually went in, so the image's contents are
  # attributable without unpacking it.
  say "$name: $(du -h "$TREE/opt/duck/bin/$name" | cut -f1) sha256:$(sha256_of "$real" | cut -c1-16)…"

  # codex needs its Code Mode sibling beside it in the guest; the executors
  # directory carries them together (one release artifact).
  if [[ "$name" == "codex" && -x "$EXEC_DIR/codex-code-mode-host" ]]; then
    install -m 0755 "$EXEC_DIR/codex-code-mode-host" "$TREE/opt/duck/bin/codex-code-mode-host"
    say "codex-code-mode-host: $(du -h "$TREE/opt/duck/bin/codex-code-mode-host" | cut -f1)"
  fi

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
