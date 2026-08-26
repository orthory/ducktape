# The sandbox on macOS (vz)

One stack decision, fixed: **Linux → Firecracker over KVM, macOS → the vz
shim over Virtualization.framework** (the same substrate Apple's
`apple/containerization` builds on). Everything above the hypervisor is
shared: the same guest kernel + `duck-guest-init` rootfs, the same ext4
workspace/asset/manifest block devices, the same vsock frame protocol, the
same `<uds>_<port>` socket convention, the same Firecracker-schema config
JSON. The shim (`bin/duck-vz-shim`, ~300 lines of Swift) is the only
macOS-specific component.

## Prerequisites — one pass

```sh
ops/macos-preflight.sh        # GUEST_DIR=… if the images live elsewhere
```

It checks everything the compute plane (airlock + provider runs) needs on a
Mac and prints the exact install command for each missing piece: Apple
silicon/`kern.hv_support`, Xcode command line tools,
`brew install e2fsprogs squashfs` (e2fsprogs is keg-only — the node searches
the standard Homebrew prefixes itself, do not add it to PATH),
`rustup target add aarch64-unknown-linux-musl`, the signed shim, and the
guest images. Exit 0 means the node's boot probe will pass.

## Bring-up on a Mac

```sh
# 1. the shim (builds + ad-hoc codesigns the virtualization entitlement)
INSTALL=~/bin bin/duck-vz-shim/build.sh        # ~/bin must be on PATH

# 2. the guest artifacts (aarch64 kernel + rootfs; cross-builds the init
#    with rust-lld, no musl toolchain needed)
OUT=~/.ducktape/guest ops/build-guest-rootfs.sh

# 3. the smoke: one microVM end to end — boot, stdio, exit code, workspace
#    read-back. This is the first thing to run and the thing to bisect with.
cargo run -p sandbox-host --example vm_smoke -- \
    --kernel ~/.ducktape/guest/vmlinux \
    --rootfs ~/.ducktape/guest/rootfs.ext4

# 4. the node
#   [sandbox]
#   runtime = "vz"
#   kernel  = "/Users/<you>/.ducktape/guest/vmlinux"
#   rootfs  = "/Users/<you>/.ducktape/guest/rootfs.ext4"
```

The boot probe fails loudly (daemon boot, not first run) when any of these is
missing: the shim on PATH, its `com.apple.security.virtualization`
entitlement (re-run `build.sh` — it codesigns), `kern.hv_support`, `mke2fs` /
`debugfs`, or the images.

## What differs from Linux, deliberately

- **No tap, no nftables.** A macOS guest gets no network device at all; runs
  reach the host over the vsock tunnel allowlist only. That is the stricter
  of the two Linux configurations, not a degraded one.
- **The kernel command line** (`firecracker_api::boot_args`): `console=hvc0`
  (virtio console, not 16550), explicit `root=/dev/vda ro` (appending it is a
  Firecracker behavior), and no `pci=off` (VZ attaches virtio over PCI —
  that flag boots a guest that finds no disks).
- **`vsock.listen_ports`** rides the config JSON for vz only:
  Virtualization.framework wants each guest-outbound port declared, while
  Firecracker forwards any port by convention and rejects unknown config
  fields.
- **Executors must be Linux aarch64 ELF binaries.** The rootfs build refuses
  a Mach-O host CLI (it would fail at exec inside the guest as a run that
  produces nothing); fetch the linux/arm64 build of an agent CLI and put it
  on PATH before running `build-guest-rootfs.sh`.

## Known unknowns for the first Mac run

The Rust host side and the config contract are covered by unit tests +
the Linux smoke; these three are what only a Mac can confirm, in the order
they would bite:

1. the CI kernel asset boots under `VZLinuxBootLoader` (the aarch64
   `vmlinux-6.1.128` is an arm64 boot Image, which is the format VZ wants —
   if not, set `KERNEL_URL` to an uncompressed arm64 `Image` build with
   virtio-blk/vsock and `hvc` console built in);
2. a guest `reboot` reaches the shim as `guestDidStop` (how `panic=1` ends a
   run);
3. vsock throughput/half-close behavior under the shim's fd bridging.

`vm_smoke` exercises 1 and 2; a real run through a node exercises 3.
