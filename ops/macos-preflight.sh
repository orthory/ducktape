#!/usr/bin/env bash
# Everything a Mac needs BEFORE it can run the compute plane (airlock +
# provider runs under the vz sandbox), checked in one pass.
#
#   ops/macos-preflight.sh                       # guest images expected in ~/.ducktape/guest
#   GUEST_DIR=/elsewhere ops/macos-preflight.sh
#
# Reports EVERY missing prerequisite with the exact command that installs it,
# instead of the boot probe's one-refusal-at-a-time loop — the probe's job is
# refusing an unready node loudly, this script's job is getting the install
# done in one sitting. Exit 0 = the node's sandbox probe will pass.
set -uo pipefail

GUEST_DIR="${GUEST_DIR:-$HOME/.ducktape/guest}"

MISSING=0
ok()   { printf '  ok    %s\n' "$1"; }
miss() { printf '  MISS  %s\n        fix: %s\n' "$1" "$2"; MISSING=1; }

[[ "$(uname -s)" == "Darwin" ]] || { echo "this preflight is for macOS; on Linux the probe wants firecracker + kvm" >&2; exit 1; }

echo "duck macOS preflight (guest dir: $GUEST_DIR)"

# ---- the machine ------------------------------------------------------------
if [[ "$(sysctl -n kern.hv_support 2>/dev/null)" == "1" ]]; then
  ok "Hypervisor.framework (kern.hv_support)"
else
  miss "Hypervisor.framework support" "this Mac cannot host VMs (Apple silicon or VT-x required); no install fixes this"
fi

if xcode-select -p >/dev/null 2>&1; then
  ok "Xcode command line tools"
else
  miss "Xcode command line tools (swift + cc)" "xcode-select --install"
fi

# ---- homebrew and the image tools ------------------------------------------
if command -v brew >/dev/null; then
  PREFIX="$(brew --prefix)"
  ok "Homebrew ($PREFIX)"
  # the node resolves keg-only tools from the STANDARD prefixes only
  # (sandbox host_tools.rs); a custom prefix passes here and fails there.
  case "$PREFIX" in
    /opt/homebrew|/usr/local) ;;
    *) miss "standard Homebrew prefix" "the node looks for keg-only tools under /opt/homebrew or /usr/local; a custom prefix ($PREFIX) is not searched" ;;
  esac
  if [[ -x "$PREFIX/opt/e2fsprogs/sbin/mke2fs" && -x "$PREFIX/opt/e2fsprogs/sbin/debugfs" ]]; then
    ok "e2fsprogs (mke2fs + debugfs, keg-only)"
  else
    miss "e2fsprogs (builds and reads each run's workspace image)" "brew install e2fsprogs"
  fi
  if command -v unsquashfs >/dev/null || [[ -x "$PREFIX/bin/unsquashfs" ]]; then
    ok "squashfs (extracts the guest base image)"
  else
    miss "squashfs (guest rootfs build)" "brew install squashfs"
  fi
else
  miss "Homebrew" 'see https://brew.sh — then: brew install e2fsprogs squashfs'
fi

# ---- rust -------------------------------------------------------------------
if command -v rustup >/dev/null; then
  ok "rustup"
  if rustup target list --installed 2>/dev/null | grep -q aarch64-unknown-linux-musl; then
    ok "aarch64-unknown-linux-musl target (guest init cross build)"
  else
    miss "musl target for the guest init" "rustup target add aarch64-unknown-linux-musl"
  fi
else
  miss "rustup" "see https://rustup.rs — then: rustup target add aarch64-unknown-linux-musl"
fi

# ---- the shim ---------------------------------------------------------------
if SHIM="$(command -v duck-vz-shim)"; then
  if codesign --display --entitlements - --xml "$SHIM" 2>&1 | grep -q com.apple.security.virtualization; then
    ok "duck-vz-shim on PATH, virtualization entitlement signed"
  else
    miss "duck-vz-shim entitlement (Virtualization.framework refuses it unsigned)" "INSTALL=<dir-on-PATH> bin/duck-vz-shim/build.sh"
  fi
else
  miss "duck-vz-shim on PATH (the macOS VMM)" "INSTALL=<dir-on-PATH> bin/duck-vz-shim/build.sh"
fi

# ---- the guest artifacts ----------------------------------------------------
if [[ -f "$GUEST_DIR/vmlinux" && -f "$GUEST_DIR/rootfs.ext4" ]]; then
  ok "guest images ($GUEST_DIR)"
else
  miss "guest kernel + rootfs" "OUT=\"$GUEST_DIR\" ops/build-guest-rootfs.sh"
fi

echo
if [[ "$MISSING" == 0 ]]; then
  echo "ready: the sandbox probe will pass. Smoke it with:"
  echo "  cargo run -p sandbox-host --example vm_smoke -- \\"
  echo "      --kernel \"$GUEST_DIR/vmlinux\" --rootfs \"$GUEST_DIR/rootfs.ext4\""
  exit 0
fi
echo "install the missing pieces above, then re-run this script."
exit 1
