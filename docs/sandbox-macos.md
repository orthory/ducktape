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

`make dev` runs this for you with `--prompt`: on a Mac it lists what is
missing and asks once — "install these now? [Y/n]" — then runs the accepted
steps (brew packages, the musl target, the shim build+sign, the guest
images). Declining just leaves a node that refuses provider runs.

It checks everything the compute plane (airlock + provider runs) needs on a
Mac and prints the exact install command for each missing piece: Apple
silicon/`kern.hv_support`, Xcode command line tools,
`brew install e2fsprogs squashfs zstd` (e2fsprogs is keg-only — the node searches
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

# 2b. the agent CLIs this host lends to runs (a checklist of what is missing,
#     with each download's url and expected sha256)
ducktape agent install

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
- **Executors must be Linux aarch64 ELF binaries**, and they do not come from
  the host `PATH` — a Mac's own `claude`/`codex` is Mach-O and the guest cannot
  exec it at all. `ducktape agent install <name>` fetches the pinned linux/arm64
  build into `~/.ducktape/executors`, and the node derives a read-only image
  from that directory and mounts it at `/opt/duck/bin` for each run. The rootfs
  carries no CLI, so installing one needs no image rebuild.

## What the first Mac run taught (all fixed in-tree)

The bring-up on an M4 Mac mini (macOS 26.5) turned every "known unknown"
into a measured fact:

1. **The Firecracker CI kernel cannot work under VZ** — it is virtio-MMIO
   only, VZ attaches virtio over PCI, and the boot is a silent black hole
   (no console, no disks). `build-guest-rootfs.sh` therefore extracts the
   Kata Containers VM kernel on macOS (virtio-pci + -mmio, no-initrd boot,
   57 ms to init) and refuses any macOS kernel without `virtio_pci` in it.
2. **A guest reboot really REBOOTS under VZ** (no `guestDidStop`) — the
   init's Firecracker-style RESTART exit boot-looped the VM forever. The
   host now tells the guest how to die: the vz cmdline carries
   `DUCK_HALT=poweroff` and the init powers off via PSCI, which is what
   stops the VM. `panic=` stays default on vz so a panicking guest parks
   for the host's timeout instead of boot-looping.
3. **The shim's fd bridging had two real bugs**, both visible only live:
   VZ hands over `O_NONBLOCK` fds (EAGAIN read as EOF), and Swift's
   `&buffer[i]` inout-to-pointer corrupted every write past byte one (the
   guest decoded a 95 MB frame length out of a StdinEof frame).
4. **A CLT install can be internally skewed** — SPM aborted outright and
   the newest SDK was unparseable by its own compiler. `build.sh` now uses
   bare `swiftc` and walks the installed SDKs newest-first until one
   builds.

End-to-end proof: `vm_smoke: OK` on the Mac — boot, manifest, mounts, stdio
frames over vsock, exit code, power-off, workspace read-back.
