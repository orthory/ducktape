# Firecracker Sandbox Backend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Run every provider run inside its own Firecracker microVM — booted from a shared read-only kernel + rootfs, given hard vcpu/memory limits, handed its workspace as a per-run block device, and torn down on exit.

**Architecture:** A new `SandboxBackend::Firecracker` arm drives one microVM per run over the Firecracker HTTP API on a per-run unix socket, mirroring how `podman_api` drives libpod today. Firecracker has no shared filesystem, so the workspace rides a per-run ext4 image built with `mke2fs -d` and read back with `debugfs -R rdump` — both rootless. A small static guest init mounts that image, execs the CLI, and carries stdout/stderr/exit back over vsock.

**Tech Stack:** Rust (tokio, serde_json), Firecracker VMM v1.16.1 + its jailer, `e2fsprogs` (`mke2fs`, `debugfs`), `nft`, `x86_64-unknown-linux-musl` for the guest init.

**Spec:** `docs/superpowers/specs/2026-08-22-sandbox-run-per-microvm-design.md`

## Global Constraints

- **No legacy, no compat.** There are zero live networks. Do not add a version gate, a compat shim, or a second decoder. (`CLAUDE.md`)
- **Logging is `tracing`, never `println!`.** `target: "ducktape::sandbox"` for the filtering handle. `info!` at most once per {boot, run}; per-frame work is `trace!`. Never log a URI path, a query string, or key material.
- **Per-crate lint gate:** `cargo clippy -p <crate> --tests --no-deps` must be clean for every crate a task touches.
- **`cargo fmt` only on code you touched.** No tree-wide sweep.
- **Tests wait on events, never on time.** No sleep-and-retry. If there is no wait seam, add the hook.
- **One discriminant, one match, no `_` wildcard** over `SandboxBackend` — a new variant must fail the build until it is routed.
- **Gate delivery on the cargo exit code**, not on grepping its output: `cargo test ... ; test ${PIPESTATUS[0]} -eq 0`.

## Host Prerequisites (verified on zk-dev, 2026-08-22)

| Requirement | State on this box | Action |
|---|---|---|
| `/dev/kvm` | present, `crw-rw---- root:kvm` | — |
| CPU virtualisation | 24 cores with vmx/svm | — |
| Nested virt | `kvm_*.nested = 1` | — |
| User in `kvm` group | **DONE** — `usermod -aG kvm eddy` applied; `/dev/kvm` opens `O_RDWR`. Ambient after next login; `sg kvm -c '...'` picks it up now without one. | — |
| `firecracker` + `jailer` | **DONE** — v1.16.1 installed to `~/.local/bin`, both on PATH | Not an apt package: upstream ships a static release tarball, so `ops/firecracker-setup.sh` is a deliberate exception to the apt-only preference for host tooling. |
| `mke2fs` / `debugfs` | present (`e2fsprogs`) | — |
| `nft` | present at `/usr/sbin/nft` | — |
| `x86_64-unknown-linux-musl` | check `rustup target list --installed` | `rustup target add x86_64-unknown-linux-musl` |

Verified live before this plan was written: `mke2fs -q -t ext4 -d <dir> -b 4096 <img> 4M` builds a populated image and `debugfs -R "cat a.txt" <img>` reads it back, both as an unprivileged user with no mount.

### Measured baseline — a real microVM booted on this host

Firecracker v1.16.1, the CI kernel `vmlinux-6.1.128` and the `ubuntu-24.04.squashfs`
rootfs, `init=/bin/true`, 2 vcpu / 512 MiB:

```
[    0.000000] Linux version 6.1.128 ...
[    1.037452] Run /bin/true as init process
[    1.051449] Kernel panic - not syncing: Attempted to kill init! exitcode=0x0
firecracker exit=0
```

- **kernel boot → init: 1.04 s**
- **whole VMM lifecycle, wall: 2.28 s** (median of 3) — of which **1.0 s is the
  `panic=1` reboot delay**, an artifact of using `/bin/true` as init. The real
  guest init powers off directly, so the comparable figure is ~1.28 s.

Take this, not the marketing figure, as the starting point. Firecracker's
widely-quoted ~125 ms is a *minimal* kernel with an initramfs; a full distro
kernel with a real root filesystem is an order of magnitude slower, and that is
the shape our guest has.

### Kernel command line — profiled, not copied

Every token in the boot args below was measured on this host with
`ops/firecracker/boot-bench.sh`, committed alongside this plan so the numbers
are reproducible rather than folklore:

```
ops/firecracker/boot-bench.sh --fetch      # get the CI kernel + rootfs
MODE=compare sg kvm -c ops/firecracker/boot-bench.sh
```

Two changes dominate:

| Change | 512 MiB, 2 vcpu | Saving |
|---|---|---|
| baseline (`i8042.noaux` only) | 1285 ms | — |
| + i8042 fully off | 811 ms | **−474 ms** |
| + `quiet loglevel=1` (alone) | 950 ms | −335 ms |
| **+ both** | **452 ms** | **−833 ms (2.84×)** |

- **The i8042 probe is the single biggest cost.** A profile of the baseline
  showed one 0.458 s gap between `clk: Disabling unused clocks` and
  `input: AT Raw Set 2 keyboard … /i8042/serio0/…` — the kernel waiting out a
  legacy PS/2 controller. `i8042.noaux` alone does NOT fix it (it disables only
  the aux/mouse port); the full group is
  `i8042.noaux i8042.nokbd i8042.nomux i8042.nopnp i8042.dumbkbd`.
- **Serial console output costs ~335 ms.** The baseline emits 268 console lines
  and every one is a synchronous write through a VMM exit. `quiet loglevel=1`
  keeps the console usable for diagnosis while dropping the bulk.
- Dropping the serial device entirely (`8250.nr_uarts=0`) buys only 20 ms more
  and costs all boot diagnostics. Not worth it.
- `tsc=reliable`, `no_timer_check`, `random.trust_cpu=on`: no measurable effect.
- vCPU count barely matters (1 / 2 / 4 → 450 / 457 / 462 ms).
- The 5.10 CI kernel is **slower** than 6.1 (577 ms vs 452 ms). Use 6.1.
- squashfs vs ext4 root: 457 ms vs 438 ms, but 78 MB vs 503 MB on disk. The boot
  difference is noise; keep squashfs for the artifact size.

**`acpi=off` is FORBIDDEN.** It appears to save 69 ms and is a correctness bug:
Firecracker enumerates vCPUs through ACPI, so the guest boots with exactly one
processor regardless of `vcpu_count`. Verified directly —
`vcpu_count=4` gives `smpboot: Total of 4 processors activated` with ACPI on and
`Total of 1 processors activated` with `acpi=off`. A node would sell four cores
and deliver one, silently. Task 5's test asserts the flag is absent.

### Guest RAM dominates above 2 GiB — and only snapshots fix it

Same tuned cmdline, 2 vcpu, varying guest memory:

| Guest RAM | wall | guest-side | host-side (VMM) |
|---|---|---|---|
| 512 MiB | 827 ms | 586 ms | 241 ms |
| 1 GiB | 883 ms | 637 ms | 246 ms |
| 2 GiB | 1015 ms | 778 ms | 237 ms |
| 4 GiB | 1908 ms | 1659 ms | 249 ms |
| 8 GiB | 2441 ms | 2153 ms | 288 ms |
| 16 GiB | 3379 ms | 3058 ms | 321 ms |

**Host-side VMM setup is flat** (241 → 321 ms); the whole curve is inside the
guest kernel, initialising its own page structures. The kernel says so itself at
16 GiB: `node 0 deferred pages initialised in 1304ms`, plus ~1 s of early zone
and memmap setup before that.

`CONFIG_DEFERRED_STRUCT_PAGE_INIT=y` is **already set** in the CI kernel config —
the kernel still waits for those background threads before running init, so the
usual mitigation is spent. Firecracker's `huge_pages: "2M"` needs host pages
reserved (`HugePages_Total: 0` here) and would not change the guest's own memmap
work anyway.

**This corrects the "snapshots are only an optimisation" line above.** For small
runs it holds — 1 vcpu / 1 GiB boots in 542 ms. For a node selling large
`mem_gb` it does not: 8 vcpu / 16 GiB costs ~2.9 s of boot on **every run**, and
a restored snapshot is the only thing that skips memmap init. Snapshot/restore
therefore stays a follow-on for the backend's correctness, but it is a
**prerequisite for selling large-memory runs at a good latency**.

### Where the time actually goes, and how anyone quotes ~125 ms

Phase split of one cold boot, from process spawn to reap:

```
  spawn → 'Running Firecracker'     2.8 ms
  → 'Successfully started'         18.2 ms   kernel load + KVM + devices
  → guest reaches init            579.5 ms   guest kernel (console on)
  → 'exiting successfully'        234.3 ms   guest halt + VMM teardown
  → process reaped                  2.7 ms
```

**Firecracker's own work is ~21 ms.** Everything else is the guest kernel. So
the published ~125 ms figures cannot be describing this shape at all — and they
are not. They are **snapshot restores**, not cold boots. Measured here with
`ops/firecracker/snapshot-bench.sh`:

| Guest RAM | cold boot | snapshot restore | snapshot create | memory file |
|---|---|---|---|---|
| 512 MiB | 428 ms | **12 ms** | 528 ms | 513 MB |
| 2 GiB | 656 ms | **11 ms** | 2417 ms | 2.1 GB |
| 8 GiB | 2041 ms | **13 ms** | 12714 ms | 8.1 GB |

Restore is **flat in guest memory** and 35-160× faster than a cold boot. It is
also faster than the figure everyone quotes, which should be the tell that the
quoted number includes work this measurement defers.

**Three costs the headline hides, and they are the design's real constraints:**

1. **"Resumed" is not "warm."** The `File` backend mmaps the memory file and
   faults pages in lazily, which is exactly why restore is flat. The guest's
   first real work pays that faulting. Production setups use the `Uffd` backend
   to control it. Do not quote 12 ms to a buyer as a time-to-useful-work.
2. **Snapshot creation scales badly** — 12.7 s at 8 GiB — and it writes the
   whole guest memory to disk.
3. **A snapshot is bound to its machine configuration**, so a node selling
   1/2/4/8/16 GiB shapes needs one snapshot per shape: ~31 GB of memory files
   just to hold the set. That is the storage bill for fast starts.

### The workspace image is the other half of the startup budget

Boot is not the whole per-run overhead — the workspace has to become a block
device before the VM starts, and be read back after it exits. Measured on this
host with trees of many small files (the shape of a checked-out repo):

| Workspace | `mke2fs -d` | `debugfs rdump` | round trip |
|---|---|---|---|
| 4 MB / 200 files | 17 ms | 14 ms | 31 ms |
| 102 MB / 2000 files | 303 ms | 201 ms | 504 ms |
| 501 MB / 4000 files | 1267 ms | 779 ms | 2046 ms |

A 500 MB workspace therefore costs about as much as an 8 GiB VM's boot. Only the
build is on the critical path before the guest starts; the read-back lands after
the run has already produced its answer.

Two things this settles:

- **`HEADROOM = 3` is nearly free.** Build time follows CONTENT, not image size —
  ×1 headroom is 307 ms and ×6 is 331 ms on the same tree, because ext4 images
  are sparse and `mke2fs` never writes the empty blocks. Do not shrink the
  headroom to chase build time; there is nothing there.
- **The round trip is byte-identical** at 500 MB / 4000 files, modes included.

So a whole run's marshalling overhead, end to end:

| Shape | image build | boot | read-back | total |
|---|---|---|---|---|
| 2 GiB VM, 100 MB workspace | 303 ms | 656 ms | 201 ms | **~1.2 s** |
| 8 GiB VM, 500 MB workspace | 1267 ms | 2041 ms | 779 ms | **~4.1 s** |

Against a minutes-long agent run both are small, but the heavy shape is no
longer negligible — and note the workspace half is untouched by snapshots.

Before/after at realistic run shapes (the cmdline saving is flat, so it matters
proportionally less as memory grows):

| Run shape | before | after | saved |
|---|---|---|---|
| 1 vcpu / 1 GiB | 1399 ms | **542 ms** | 857 ms |
| 2 vcpu / 2 GiB | 1504 ms | **656 ms** | 848 ms |
| 4 vcpu / 4 GiB | 2380 ms | **1538 ms** | 842 ms |
| 8 vcpu / 8 GiB | 2869 ms | **2041 ms** | 828 ms |
| 8 vcpu / 16 GiB | 3737 ms | **2923 ms** | 814 ms |

Artifacts used (also the fastest way to get Task 8's e2e running before
`build-guest-rootfs.sh` exists):

```
https://s3.amazonaws.com/spec.ccfc.min/firecracker-ci/v1.12/x86_64/vmlinux-6.1.128
https://s3.amazonaws.com/spec.ccfc.min/firecracker-ci/v1.12/x86_64/ubuntu-24.04.squashfs
```

A `vmlinux-6.1.128.config` is published beside the kernel — the concrete
starting point for the "build our own kernel?" open question below.

## File Structure

| File | Responsibility |
|---|---|
| `crates/services/sandbox/src/firecracker_api.rs` | new — the VMM HTTP client over a per-run unix socket: machine-config, boot-source, drives, vsock, network-interface, InstanceStart. Peer of `podman_api.rs`. |
| `crates/services/sandbox/src/workspace_image.rs` | new — build a workspace ext4 from a directory, read it back. Pure shell-outs to `mke2fs`/`debugfs`, no mounting, no root. |
| `crates/services/sandbox/src/agent_volume.rs` | new — the persistent per-agent cache volume (`CARGO_HOME` + `RUSTUP_HOME` + `target/`): create a sparse ext4 once, seed it from a template, hand its path to the VMM. Attached, never copied back. |
| `crates/services/sandbox/src/guest_proto.rs` | new — the vsock frame codec shared by host and guest. Pure, no I/O. |
| `crates/services/sandbox/src/sandbox.rs` | modify — add the `Firecracker` variant and extend `probe`. |
| `crates/services/sandbox/src/lib.rs` | modify — declare and re-export the new modules. |
| `bin/duck-guest-init/` | new crate — the static guest PID 1. Depends only on `sandbox-host`'s `guest_proto` (a `no-std`-friendly, dependency-light module) and libc. |
| `ops/firecracker-setup.sh` | new — install the VMM, name the group requirement, verify `/dev/kvm` access. |
| `ops/firecracker/boot-bench.sh` | **already committed with this plan** — the harness every boot-time number below came from. `MODE=shapes\|compare\|memory`. Re-run it after changing the kernel, the rootfs or the boot args. |
| `ops/firecracker/snapshot-bench.sh` | **already committed with this plan** — measures snapshot create/restore and the memory file's size on disk. `MEM=<mib>`. |
| `ops/build-guest-rootfs.sh` | new — build the shared read-only rootfs and fetch/build the guest kernel. |
| `crates/services/provider/src/lib.rs` | modify — a `RunControl::MicroVm` arm and the boot/teardown call sites. |
| `bin/node/src/config/resolve.rs` | modify — accept `runtime = "firecracker"`. |

---

### Task 1: the `Firecracker` backend variant and its boot probe

Adds the arm and the host-capability check, so a misconfigured host fails loudly at boot rather than 150 s into a run. Nothing boots a VM yet.

**Files:**
- Modify: `crates/services/sandbox/src/sandbox.rs`
- Create: `ops/firecracker-setup.sh`
- Test: `crates/services/sandbox/src/sandbox.rs` (the `#[cfg(test)] mod tests` at the bottom — the file has none since the Tart removal; create it)

**Interfaces:**
- Consumes: nothing.
- Produces: `SandboxBackend::Firecracker { kernel: PathBuf, rootfs: PathBuf }`, and `SandboxBackend::probe(&self) -> Result<PathBuf, String>` extended to cover it.

- [ ] **Step 1: Write the failing test**

Append to `crates/services/sandbox/src/sandbox.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// The probe is what the e2e skip guards key on, so it must fail for a
    /// reason that NAMES the missing piece. A guard that reports "ready" while
    /// the VMM cannot open /dev/kvm converts a missing group membership into a
    /// phantom bug hunt 150s later — the exact failure the podman probe was
    /// written to prevent.
    #[test]
    fn firecracker_probe_names_the_missing_binary() {
        let backend = SandboxBackend::Firecracker {
            kernel: PathBuf::from("/nonexistent/vmlinux"),
            rootfs: PathBuf::from("/nonexistent/rootfs.ext4"),
        };
        let err = backend.probe().expect_err("no firecracker on an empty PATH");
        assert!(err.contains("firecracker"), "{err}");
    }

    #[test]
    fn firecracker_runtime_bin_is_the_vmm() {
        let backend = SandboxBackend::Firecracker {
            kernel: PathBuf::from("/k"),
            rootfs: PathBuf::from("/r"),
        };
        assert_eq!(backend.runtime_bin(), "firecracker");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sandbox-host firecracker_probe -- --nocapture`
Expected: FAIL — `no variant named 'Firecracker' found for enum 'SandboxBackend'`.

- [ ] **Step 3: Add the variant and route every match**

In `crates/services/sandbox/src/sandbox.rs`, add to the enum:

```rust
    /// one microVM per run: a shared read-only `kernel` + `rootfs` boot under
    /// Firecracker's jailer, the workspace rides a per-run ext4 block device,
    /// and the guest reaches the host only over vsock and its tap. The VM
    /// boundary subsumes the namespace/cgroup/seccomp posture a container
    /// backend has to declare field by field.
    Firecracker {
        kernel: PathBuf,
        rootfs: PathBuf,
    },
```

In `runtime_bin`, add the arm (the match has no wildcard, so the build fails until you do):

```rust
            SandboxBackend::Firecracker { .. } => "firecracker",
```

In `probe`, after the existing podman dependency loop, add:

```rust
        if matches!(self, SandboxBackend::Firecracker { .. }) {
            // `nft` writes the tap's egress ruleset on the HOST side; the guest
            // has no say in it. `jailer` is Firecracker's own chroot + seccomp +
            // cgroup wrapper around the VMM process — running without it would
            // leave the hypervisor itself unsandboxed.
            for dep in ["jailer", "nft"] {
                if crate::podman_api::find_system_tool(dep).is_none() {
                    return Err(format!(
                        "{dep} is not executable on PATH or a standard sbin dir; the \
                         Firecracker sandbox requires it (jailer = the VMM's own \
                         chroot/seccomp/cgroup wrapper, nft = the tap egress firewall) \
                         — install it"
                    ));
                }
            }
            kvm_is_usable()?;
        }
```

And add the helper below `find_on_path`:

```rust
/// `/dev/kvm` must be openable READ-WRITE by this process. The common failure
/// is a user outside the `kvm` group: the node boots, the probe passes on a
/// mere existence check, and every run then dies inside the VMM with a bare
/// EACCES that names neither the device nor the group.
fn kvm_is_usable() -> Result<(), String> {
    match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/kvm")
    {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => Err(
            "/dev/kvm is not writable by this user; add it to the `kvm` group \
             (`sudo usermod -aG kvm $USER`) and log in again"
                .to_string(),
        ),
        Err(e) => Err(format!("/dev/kvm is unusable: {e}")),
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p sandbox-host ; test ${PIPESTATUS[0]} -eq 0`
Expected: PASS. Also run `cargo check -p provider-host -p node-bin --tests` — the wildcard-free matches over `SandboxBackend` in those crates now fail to compile, which is the point. Route each one to a `todo!("Task 6")` for now; Task 6 replaces them.

- [ ] **Step 5: Write `ops/firecracker-setup.sh`**

```bash
#!/usr/bin/env bash
# Install the Firecracker VMM + jailer and verify this host can run them.
# Firecracker is NOT in apt: upstream ships a static release tarball, so this
# is a deliberate exception to the apt-only rule for host tooling.
set -euo pipefail

VERSION="${FIRECRACKER_VERSION:-v1.16.1}"
ARCH="$(uname -m)"
DEST="${DEST:-$HOME/.local/bin}"

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "the sandbox is Linux-only; there is no macOS backend" >&2
  exit 1
fi

mkdir -p "$DEST"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

url="https://github.com/firecracker-microvm/firecracker/releases/download/${VERSION}/firecracker-${VERSION}-${ARCH}.tgz"
echo "fetching ${VERSION} for ${ARCH}"
curl -fsSL "$url" -o "$tmp/fc.tgz"
tar -xzf "$tmp/fc.tgz" -C "$tmp"
install -m 0755 "$tmp/release-${VERSION}-${ARCH}/firecracker-${VERSION}-${ARCH}" "$DEST/firecracker"
install -m 0755 "$tmp/release-${VERSION}-${ARCH}/jailer-${VERSION}-${ARCH}" "$DEST/jailer"
echo "installed firecracker + jailer to $DEST"

if [[ ! -w /dev/kvm ]]; then
  echo
  echo "WARNING: /dev/kvm is not writable by $(id -un)." >&2
  echo "  sudo usermod -aG kvm $(id -un)   # then log in again" >&2
  exit 1
fi
echo "/dev/kvm is usable — host is ready"
```

Make it executable: `chmod +x ops/firecracker-setup.sh`

- [ ] **Step 6: Commit**

```bash
git add crates/services/sandbox/src/sandbox.rs ops/firecracker-setup.sh
git commit -m "feat(sandbox): the Firecracker backend variant and its boot probe

The probe checks jailer, nft, and a WRITABLE /dev/kvm rather than the
device's mere existence: a user outside the kvm group otherwise passes
boot and dies inside the VMM with a bare EACCES naming neither the
device nor the group."
```

---

### Task 2: the workspace block image — build and read back

Firecracker has no shared filesystem, so the workspace becomes a per-run ext4 image. Both directions are rootless and mount-free, verified on this host before the plan was written.

**Files:**
- Create: `crates/services/sandbox/src/workspace_image.rs`
- Modify: `crates/services/sandbox/src/lib.rs`
- Test: in-file `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `pub fn build(workdir: &Path, image: &Path, bytes: u64) -> Result<(), String>`
  - `pub fn read_back(image: &Path, dest: &Path) -> Result<(), String>`
  - `pub fn sized_for(workdir: &Path) -> Result<u64, String>` — the image size for a workspace, headroom included.
  - `pub const MAX_WORKSPACE_BYTES: u64`

- [ ] **Step 1: Write the failing test**

Create `crates/services/sandbox/src/workspace_image.rs` with only the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ducktape-wsimg-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    /// The round trip is the whole contract: what the guest writes has to come
    /// back byte-identical, with the mode bits intact — an agent run that
    /// produces an executable must not hand back a non-executable one.
    #[test]
    fn a_workspace_survives_the_round_trip_with_modes_and_nesting() {
        use std::os::unix::fs::PermissionsExt as _;
        let root = scratch("round-trip");
        let src = root.join("src");
        std::fs::create_dir_all(src.join("nested/deep")).expect("dirs");
        std::fs::write(src.join("top.txt"), b"top").expect("write");
        std::fs::write(src.join("nested/deep/leaf.bin"), &[0u8, 159, 146, 150]).expect("write");
        let exe = src.join("run.sh");
        std::fs::write(&exe, b"#!/bin/sh\nexit 0\n").expect("write");
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).expect("chmod");

        let image = root.join("ws.ext4");
        build(&src, &image, sized_for(&src).expect("size")).expect("image builds");

        let back = root.join("back");
        read_back(&image, &back).expect("image reads back");

        assert_eq!(std::fs::read(back.join("top.txt")).expect("top"), b"top");
        assert_eq!(
            std::fs::read(back.join("nested/deep/leaf.bin")).expect("leaf"),
            vec![0u8, 159, 146, 150],
            "a binary payload must not be mangled"
        );
        let mode = std::fs::metadata(back.join("run.sh")).expect("exe").permissions().mode();
        assert_eq!(mode & 0o111, 0o111, "the executable bit must survive: {mode:o}");
    }

    /// A workspace whose whole content is ONE directory must come back as one
    /// directory. An earlier draft of this module tried to "flatten" what it
    /// assumed was a wrapper directory that `debugfs rdump` added; it does not
    /// add one, and the flattening would have silently hoisted `src/`'s
    /// contents into the workspace root for exactly this shape — the common
    /// shape of a checked-out repo.
    #[test]
    fn a_single_directory_workspace_is_not_flattened() {
        let root = scratch("single-dir");
        let src = root.join("src-tree");
        std::fs::create_dir_all(src.join("src/deep")).expect("dirs");
        std::fs::write(src.join("src/deep/main.rs"), b"fn main() {}").expect("write");
        std::fs::write(src.join("src/lib.rs"), b"x").expect("write");

        let image = root.join("ws.ext4");
        build(&src, &image, sized_for(&src).expect("size")).expect("builds");
        let back = root.join("back");
        read_back(&image, &back).expect("reads back");

        assert!(back.join("src/deep/main.rs").is_file(), "src/ must stay a directory");
        assert!(back.join("src/lib.rs").is_file());
    }

    /// `lost+found` is mke2fs's, not the run's. Handing it back adds a
    /// directory nobody created to the buyer's workspace, and on a second round
    /// trip it persists and multiplies.
    #[test]
    fn lost_and_found_never_reaches_the_workspace() {
        let root = scratch("lost-found");
        let src = root.join("src");
        std::fs::create_dir_all(&src).expect("dir");
        std::fs::write(src.join("a.txt"), b"a").expect("write");

        let image = root.join("ws.ext4");
        build(&src, &image, sized_for(&src).expect("size")).expect("builds");
        let back = root.join("back");
        read_back(&image, &back).expect("reads back");

        assert!(!back.join("lost+found").exists(), "ext4 artifact leaked into the workspace");
        let entries: Vec<_> = std::fs::read_dir(&back).expect("read").filter_map(|e| e.ok()).collect();
        assert_eq!(entries.len(), 1, "exactly the file the run had");
    }

    /// An image smaller than its contents is a silent truncation, so the size
    /// is computed with headroom and a floor rather than guessed.
    #[test]
    fn sizing_leaves_headroom_over_the_measured_tree() {
        let root = scratch("sizing");
        std::fs::create_dir_all(&root).expect("dir");
        std::fs::write(root.join("a"), vec![7u8; 1024 * 1024]).expect("write");
        let size = sized_for(&root).expect("size");
        assert!(size >= 1024 * 1024 * 2, "1MiB of payload needs real headroom, got {size}");
        assert!(size >= MIN_WORKSPACE_BYTES);
    }

    /// A workspace larger than the cap must be refused BEFORE a VM boots, with
    /// a message naming the cap — not after a run has taken a lease.
    #[test]
    fn an_oversized_workspace_is_refused_by_name() {
        let root = scratch("oversized");
        let err = size_or_refuse(MAX_WORKSPACE_BYTES + 1).expect_err("refused");
        assert!(err.contains("workspace"), "{err}");
        let _ = root;
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sandbox-host workspace_image`
Expected: FAIL — the module is not declared in `lib.rs` and `build`/`read_back`/`sized_for` do not exist.

- [ ] **Step 3: Write the implementation**

Prepend to `crates/services/sandbox/src/workspace_image.rs`:

```rust
//! the run's workspace as a block device.
//!
//! Firecracker has no shared filesystem — its device model is virtio-block,
//! -net, -vsock, -balloon, -rng and a serial console, nothing else. So the
//! workspace is handed over as a per-run ext4 image: built from the workdir
//! before boot, mounted by the guest init, and read back after the guest
//! reports its exit code.
//!
//! Both directions are ROOTLESS and mount-free. `mke2fs -d` populates an image
//! from a directory without ever mounting it, and `debugfs -R rdump` walks one
//! back out. That matters: a loop mount would need root, and a node that needs
//! root to move a workspace is a node that runs as root.

use std::path::{Path, PathBuf};
use std::process::Command;

/// the floor: ext4 metadata plus a journal does not fit in a few hundred KiB,
/// and `mke2fs` silently drops the journal below ~16 MiB ("Filesystem too small
/// for a journal"). A journal-less workspace image is a torn tree after a hard
/// VM kill, so the floor is above that threshold rather than at it.
pub const MIN_WORKSPACE_BYTES: u64 = 32 * 1024 * 1024;

/// the ceiling, refused before a VM boots. A run whose workspace does not fit
/// cannot be salvaged by retrying, so it must fail at submit-adjacent time.
pub const MAX_WORKSPACE_BYTES: u64 = 8 * 1024 * 1024 * 1024;

/// headroom multiplier over the measured tree: the guest WRITES into this
/// image (that is the point), so it needs room for the run's output, not just
/// its input.
const HEADROOM: u64 = 3;

/// the image size for `workdir`: measured tree × [`HEADROOM`], floored at
/// [`MIN_WORKSPACE_BYTES`] and refused above [`MAX_WORKSPACE_BYTES`].
pub fn sized_for(workdir: &Path) -> Result<u64, String> {
    let measured = tree_bytes(workdir)?;
    size_or_refuse(measured.saturating_mul(HEADROOM).max(MIN_WORKSPACE_BYTES))
}

/// the size decision alone, split out so the refusal is unit-testable without
/// materialising gigabytes on disk.
pub fn size_or_refuse(size: u64) -> Result<u64, String> {
    if size > MAX_WORKSPACE_BYTES {
        return Err(format!(
            "workspace needs {size} bytes of image, over the {MAX_WORKSPACE_BYTES}-byte cap"
        ));
    }
    Ok(size)
}

fn tree_bytes(dir: &Path) -> Result<u64, String> {
    let mut total = 0u64;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        let entries = std::fs::read_dir(&next)
            .map_err(|e| format!("measure workspace {}: {e}", next.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("measure workspace: {e}"))?;
            let meta = entry
                .metadata()
                .map_err(|e| format!("measure {}: {e}", entry.path().display()))?;
            if meta.is_dir() {
                stack.push(entry.path());
            } else {
                total = total.saturating_add(meta.len());
            }
        }
    }
    Ok(total)
}

/// build an ext4 image of exactly `bytes` populated from `workdir`.
pub fn build(workdir: &Path, image: &Path, bytes: u64) -> Result<(), String> {
    let tool = crate::podman_api::find_system_tool("mke2fs")
        .ok_or_else(|| "mke2fs is not on PATH; install e2fsprogs".to_string())?;
    let blocks = bytes.div_ceil(4096);
    let out = Command::new(&tool)
        .args(["-q", "-t", "ext4", "-b", "4096", "-d"])
        .arg(workdir)
        .arg(image)
        .arg(blocks.to_string())
        .output()
        .map_err(|e| format!("run mke2fs: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "mke2fs exited with {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

/// walk `image` back out into `dest`, which is created if absent.
pub fn read_back(image: &Path, dest: &Path) -> Result<(), String> {
    let tool = crate::podman_api::find_system_tool("debugfs")
        .ok_or_else(|| "debugfs is not on PATH; install e2fsprogs".to_string())?;
    std::fs::create_dir_all(dest).map_err(|e| format!("create {}: {e}", dest.display()))?;
    // `rdump / <dest>` lands the image root's entries DIRECTLY in dest — no
    // wrapper directory. Verified on a 500 MB / 4000-file tree: the round trip
    // is byte-identical, modes included, once `lost+found` is dropped.
    let out = Command::new(&tool)
        .arg("-R")
        .arg(format!("rdump / {}", dest.display()))
        .arg(image)
        .output()
        .map_err(|e| format!("run debugfs: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "debugfs exited with {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    drop_lost_found(dest)
}

/// `lost+found` is an ext4 artifact `mke2fs` creates, not something the run
/// produced. Handing it back would add a directory to the buyer's workspace
/// that nobody put there — and on a second round trip it would persist and
/// multiply.
fn drop_lost_found(dest: &Path) -> Result<(), String> {
    let stray = dest.join("lost+found");
    match std::fs::remove_dir_all(&stray) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("remove {}: {e}", stray.display())),
    }
}
```

Declare it in `crates/services/sandbox/src/lib.rs`:

```rust
#[cfg(unix)]
pub mod workspace_image;
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p sandbox-host workspace_image ; test ${PIPESTATUS[0]} -eq 0`
Expected: PASS, all three.

If `flatten_rdump_root` misfires, run the round trip by hand to see the real layout before changing it:
`mke2fs -q -t ext4 -d /tmp/src -b 4096 /tmp/img 8192 && debugfs -R "rdump / /tmp/out" /tmp/img && find /tmp/out`

- [ ] **Step 5: Commit**

```bash
git add crates/services/sandbox/src/workspace_image.rs crates/services/sandbox/src/lib.rs
git commit -m "feat(sandbox): the run workspace as a rootless ext4 block image

Firecracker has no shared filesystem, so the workspace is handed over as
a per-run block device. mke2fs -d populates an image from a directory
and debugfs rdump walks it back out, both without mounting and without
root — a loop mount would need root, and a node that needs root to move
a workspace is a node that runs as root."
```

---

### Task 3: the vsock frame codec

The host and the guest share one wire format. It is pure, so it is fully unit-testable on both sides before either end exists.

**Files:**
- Create: `crates/services/sandbox/src/guest_proto.rs`
- Modify: `crates/services/sandbox/src/lib.rs`
- Test: in-file `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `pub enum Frame { Stdout(Vec<u8>), Stderr(Vec<u8>), Exit(i32) }`
  - `pub fn encode(frame: &Frame) -> Vec<u8>`
  - `pub fn decode(buf: &mut Vec<u8>) -> Result<Option<Frame>, String>` — drains one frame, `Ok(None)` when the buffer holds a partial one.
  - `pub const MAX_FRAME_BYTES: usize`
  - `pub const VSOCK_PORT: u32`

- [ ] **Step 1: Write the failing test**

Create `crates/services/sandbox/src/guest_proto.rs` with the tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_round_trips() {
        for frame in [
            Frame::Stdout(b"hello".to_vec()),
            Frame::Stderr(b"warn".to_vec()),
            Frame::Exit(0),
            Frame::Exit(137),
            Frame::Stdout(Vec::new()),
        ] {
            let mut buf = encode(&frame);
            let got = decode(&mut buf).expect("decodes").expect("a whole frame");
            assert_eq!(got, frame);
            assert!(buf.is_empty(), "a decoded frame must be drained");
        }
    }

    /// The guest writes into a stream, so the host sees arbitrary splits. A
    /// decoder that mistakes a split for corruption drops a run's output.
    #[test]
    fn a_partial_frame_is_not_an_error() {
        let whole = encode(&Frame::Stdout(b"abcdefgh".to_vec()));
        for cut in 1..whole.len() {
            let mut buf = whole[..cut].to_vec();
            assert_eq!(decode(&mut buf).expect("no error"), None, "cut at {cut}");
            assert_eq!(buf.len(), cut, "a partial frame must stay buffered");
        }
    }

    #[test]
    fn two_frames_in_one_read_both_come_out() {
        let mut buf = encode(&Frame::Stdout(b"one".to_vec()));
        buf.extend(encode(&Frame::Exit(3)));
        assert_eq!(decode(&mut buf).unwrap(), Some(Frame::Stdout(b"one".to_vec())));
        assert_eq!(decode(&mut buf).unwrap(), Some(Frame::Exit(3)));
        assert_eq!(decode(&mut buf).unwrap(), None);
    }

    /// The guest is UNTRUSTED. A length header it controls must not be able to
    /// make the host allocate without bound.
    #[test]
    fn an_oversized_length_header_is_refused_not_allocated() {
        let mut buf = vec![0u8]; // Stdout
        buf.extend((MAX_FRAME_BYTES as u32 + 1).to_le_bytes());
        let err = decode(&mut buf).expect_err("refused");
        assert!(err.contains("frame"), "{err}");
    }

    #[test]
    fn an_unknown_tag_is_refused() {
        let mut buf = vec![0xffu8, 0, 0, 0, 0];
        assert!(decode(&mut buf).is_err());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sandbox-host guest_proto`
Expected: FAIL — `Frame`, `encode`, `decode` undefined.

- [ ] **Step 3: Write the implementation**

Prepend to `crates/services/sandbox/src/guest_proto.rs`:

```rust
//! the vsock wire between the host and one run's guest init.
//!
//! Deliberately tiny: a 1-byte tag, a 4-byte little-endian length, then the
//! payload. There is no handshake and no version byte — there are no live
//! networks and the guest image ships with the host that boots it, so the two
//! ends are always the same build.
//!
//! Every field here is written by an UNTRUSTED guest, so decoding refuses
//! rather than trusts: an unknown tag is an error, and a length header over
//! [`MAX_FRAME_BYTES`] is refused BEFORE anything is allocated.

/// the guest-side vsock port the init dials. Host-side is the run's unix
/// socket that Firecracker multiplexes onto it.
pub const VSOCK_PORT: u32 = 1024;

/// the largest single frame. The guest cannot make the host allocate past it.
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;

const TAG_STDOUT: u8 = 0;
const TAG_STDERR: u8 = 1;
const TAG_EXIT: u8 = 2;
const HEADER_BYTES: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    Stdout(Vec<u8>),
    Stderr(Vec<u8>),
    Exit(i32),
}

pub fn encode(frame: &Frame) -> Vec<u8> {
    let (tag, payload) = match frame {
        Frame::Stdout(bytes) => (TAG_STDOUT, bytes.clone()),
        Frame::Stderr(bytes) => (TAG_STDERR, bytes.clone()),
        Frame::Exit(code) => (TAG_EXIT, code.to_le_bytes().to_vec()),
    };
    let mut out = Vec::with_capacity(HEADER_BYTES + payload.len());
    out.push(tag);
    out.extend((payload.len() as u32).to_le_bytes());
    out.extend(payload);
    out
}

/// drain one frame from `buf`. `Ok(None)` means the buffer holds only part of
/// one — the caller reads more and calls again.
pub fn decode(buf: &mut Vec<u8>) -> Result<Option<Frame>, String> {
    if buf.len() < HEADER_BYTES {
        return Ok(None);
    }
    let tag = buf[0];
    let len = u32::from_le_bytes([buf[1], buf[2], buf[3], buf[4]]) as usize;
    if len > MAX_FRAME_BYTES {
        return Err(format!(
            "guest frame claims {len} bytes, over the {MAX_FRAME_BYTES} cap"
        ));
    }
    if buf.len() < HEADER_BYTES + len {
        return Ok(None);
    }
    let payload: Vec<u8> = buf.drain(..HEADER_BYTES + len).skip(HEADER_BYTES).collect();
    let frame = match tag {
        TAG_STDOUT => Frame::Stdout(payload),
        TAG_STDERR => Frame::Stderr(payload),
        TAG_EXIT => {
            let bytes: [u8; 4] = payload
                .as_slice()
                .try_into()
                .map_err(|_| format!("guest exit frame carried {} bytes, want 4", payload.len()))?;
            Frame::Exit(i32::from_le_bytes(bytes))
        }
        other => return Err(format!("guest frame carried unknown tag {other}")),
    };
    Ok(Some(frame))
}
```

Declare it in `crates/services/sandbox/src/lib.rs`:

```rust
pub mod guest_proto;
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p sandbox-host guest_proto ; test ${PIPESTATUS[0]} -eq 0`
Expected: PASS, all five.

- [ ] **Step 5: Commit**

```bash
git add crates/services/sandbox/src/guest_proto.rs crates/services/sandbox/src/lib.rs
git commit -m "feat(sandbox): the host<->guest vsock frame codec

Tag, little-endian length, payload. No handshake and no version byte:
there are no live networks and the guest image ships with the host that
boots it. Decoding refuses rather than trusts, because every field is
written by an untrusted guest — an oversized length header is rejected
before anything is allocated."
```

---

### Task 4: the guest init binary

The static PID 1 inside the VM: mount the workspace, exec the CLI, carry its streams back, report exit, unmount. This is the seat `conmon` occupied under podman.

**Files:**
- Create: `bin/duck-guest-init/Cargo.toml`, `bin/duck-guest-init/src/main.rs`, `bin/duck-guest-init/src/manifest.rs`
- Modify: root `Cargo.toml` (workspace members)
- Test: `bin/duck-guest-init/src/manifest.rs` in-file tests

**Interfaces:**
- Consumes: `sandbox_host::guest_proto::{Frame, encode, VSOCK_PORT}`.
- Produces: a `duck-guest-init` binary, and `manifest::RunManifest { argv: Vec<String>, env: Vec<(String, String)>, cwd: String }` with `manifest::parse(&str) -> Result<RunManifest, String>`.

The manifest is handed in on the kernel command line as a single base64 blob, so the guest needs no second block device to learn what to run.

- [ ] **Step 1: Write the failing test**

Create `bin/duck-guest-init/src/manifest.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_manifest_round_trips_through_the_cmdline_encoding() {
        let manifest = RunManifest {
            argv: vec!["/usr/bin/claude".into(), "-p".into()],
            env: vec![("HOME".into(), "/root".into()), ("PATH".into(), "/usr/bin".into())],
            cwd: "/workspace".into(),
        };
        let encoded = encode(&manifest);
        assert!(!encoded.contains(' '), "a cmdline token must not contain spaces");
        assert_eq!(parse(&encoded).expect("parses"), manifest);
    }

    /// The manifest arrives on the kernel command line, which the HOST writes —
    /// but a malformed one must fail with a nameable error rather than boot a
    /// guest that runs nothing and hangs until the idle timeout.
    #[test]
    fn a_malformed_manifest_is_a_named_error() {
        assert!(parse("not-base64!!").is_err());
        assert!(parse("").is_err());
    }

    #[test]
    fn an_empty_argv_is_refused() {
        let err = parse(&encode(&RunManifest {
            argv: Vec::new(),
            env: Vec::new(),
            cwd: "/workspace".into(),
        }))
        .expect_err("refused");
        assert!(err.contains("argv"), "{err}");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p duck-guest-init`
Expected: FAIL — the crate does not exist.

- [ ] **Step 3: Create the crate and the manifest implementation**

`bin/duck-guest-init/Cargo.toml`:

```toml
[package]
name = "duck-guest-init"
edition.workspace = true
version.workspace = true

# PID 1 inside a run's microVM. Kept dependency-light on purpose: it ships
# inside the guest rootfs, so every dependency is bytes in an image that boots
# a few hundred times a day, and code running as PID 1 with the workspace
# mounted is the least appealing place in the tree for a supply-chain surprise.
[dependencies]
libc = "0.2"
serde = { workspace = true }
serde_json = { workspace = true }
sandbox-host = { path = "../../crates/services/sandbox" }

[[bin]]
name = "duck-guest-init"
path = "src/main.rs"
```

Prepend to `bin/duck-guest-init/src/manifest.rs`:

```rust
//! what this VM is supposed to run, handed in on the kernel command line.
//!
//! The command line is the only channel available before any device is up, so
//! the manifest rides it as one base64 token (a cmdline token cannot contain
//! spaces, and argv/env freely do).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunManifest {
    pub argv: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cwd: String,
}

/// the alphabet is URL-safe and unpadded: `+`, `/` and `=` all have meaning to
/// a bootloader or a shell somewhere along the way.
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

pub fn encode(manifest: &RunManifest) -> String {
    let json = serde_json::to_vec(manifest).expect("a manifest always serializes");
    let mut out = String::with_capacity(json.len().div_ceil(3) * 4);
    for chunk in json.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = u32::from(b[0]) << 16 | u32::from(b[1]) << 8 | u32::from(b[2]);
        let take = chunk.len() + 1;
        for i in 0..take {
            out.push(ALPHABET[((n >> (18 - 6 * i)) & 0x3f) as usize] as char);
        }
    }
    out
}

pub fn parse(encoded: &str) -> Result<RunManifest, String> {
    if encoded.is_empty() {
        return Err("run manifest is empty".to_string());
    }
    let mut bytes = Vec::with_capacity(encoded.len() / 4 * 3);
    let mut acc = 0u32;
    let mut held = 0u32;
    for ch in encoded.bytes() {
        let value = ALPHABET
            .iter()
            .position(|c| *c == ch)
            .ok_or_else(|| format!("run manifest has a non-alphabet byte {ch:#x}"))?;
        acc = acc << 6 | value as u32;
        held += 6;
        if held >= 8 {
            held -= 8;
            bytes.push((acc >> held) as u8);
        }
    }
    let manifest: RunManifest =
        serde_json::from_slice(&bytes).map_err(|e| format!("run manifest is not valid json: {e}"))?;
    if manifest.argv.is_empty() {
        return Err("run manifest has an empty argv — nothing to execute".to_string());
    }
    Ok(manifest)
}
```

- [ ] **Step 4: Run the manifest tests**

Run: `cargo test -p duck-guest-init ; test ${PIPESTATUS[0]} -eq 0`
Expected: PASS, all three.

- [ ] **Step 5: Write `main.rs`**

```rust
//! PID 1 inside one run's microVM.
//!
//! Nothing here is a policy decision — the host already decided what runs, with
//! what env, under what limits. This process only makes the guest usable and
//! gets the bytes back:
//!
//!   mount /workspace  →  exec the CLI  →  pump stdout/stderr over vsock
//!   →  report the exit code  →  unmount so the image is consistent
//!
//! It is PID 1, so an unhandled panic is a kernel panic and a hung read is a
//! hung run. Every failure path therefore reports and powers off rather than
//! returning.

mod manifest;

use std::io::{Read as _, Write as _};
use std::os::unix::process::ExitStatusExt as _;
use std::process::{Command, Stdio};

use sandbox_host::guest_proto::{Frame, encode};

const WORKSPACE_DEV: &str = "/dev/vdb";
const WORKSPACE_DIR: &str = "/workspace";

fn main() {
    if let Err(reason) = run() {
        // PID 1 has nowhere to return to; say why on the console the host is
        // capturing, then halt so the host sees the VM exit rather than a hang.
        eprintln!("guest-init: {reason}");
        power_off();
    }
    power_off();
}

fn run() -> Result<(), String> {
    let cmdline = std::fs::read_to_string("/proc/cmdline")
        .map_err(|e| format!("read /proc/cmdline: {e}"))?;
    let token = cmdline
        .split_whitespace()
        .find_map(|t| t.strip_prefix("ducktape.run="))
        .ok_or_else(|| "no ducktape.run= on the kernel command line".to_string())?;
    let manifest = manifest::parse(token)?;

    mount_workspace()?;

    let mut vsock = connect_host()?;
    let mut child = Command::new(&manifest.argv[0])
        .args(&manifest.argv[1..])
        .current_dir(&manifest.cwd)
        .env_clear()
        .envs(manifest.env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn {}: {e}", manifest.argv[0]))?;

    let mut stdout = child.stdout.take().expect("stdout piped");
    let mut stderr = child.stderr.take().expect("stderr piped");
    // Two threads rather than a select: PID 1 has no async runtime, and a
    // sequential drain would deadlock the moment the child fills the pipe it
    // is not being read from.
    let (tx, rx) = std::sync::mpsc::channel::<Frame>();
    let out_tx = tx.clone();
    std::thread::spawn(move || pump(&mut stdout, &out_tx, Frame::Stdout));
    std::thread::spawn(move || pump(&mut stderr, &tx, Frame::Stderr));

    for frame in rx.iter() {
        vsock
            .write_all(&encode(&frame))
            .map_err(|e| format!("write to host: {e}"))?;
    }

    let status = child.wait().map_err(|e| format!("wait for child: {e}"))?;
    let code = status
        .code()
        .unwrap_or_else(|| 128 + status.signal().unwrap_or(0));
    vsock
        .write_all(&encode(&Frame::Exit(code)))
        .map_err(|e| format!("report exit: {e}"))?;
    vsock.flush().map_err(|e| format!("flush to host: {e}"))?;

    unmount_workspace()
}

fn pump<R: Read>(reader: &mut R, tx: &std::sync::mpsc::Sender<Frame>, wrap: fn(Vec<u8>) -> Frame) {
    let mut buf = [0u8; 32 * 1024];
    loop {
        match reader.read(&mut buf) {
            Ok(0) | Err(_) => return,
            Ok(n) => {
                if tx.send(wrap(buf[..n].to_vec())).is_err() {
                    return;
                }
            }
        }
    }
}
```

The `mount_workspace`, `unmount_workspace`, `connect_host` and `power_off` helpers are thin `libc` calls (`mount(2)`, `umount2(2)`, an `AF_VSOCK` connect to CID 2 on `guest_proto::VSOCK_PORT`, and `reboot(2)`). Write them in the same file, each with a `SAFETY:` comment naming why the call is sound.

**`power_off` must use `LINUX_REBOOT_CMD_RESTART`, not `LINUX_REBOOT_CMD_POWER_OFF`.** This was measured the hard way: Firecracker has no ACPI power button and no PM device, so `POWER_OFF` leaves the guest at `reboot: System halted` and **the VMM never exits** — the run hangs to its idle timeout with its memory still held. `RESTART` goes through the `reboot=k` i8042 reset, which Firecracker does observe, and it exits cleanly. Verified: identical guests differing only in this constant, one hung past a 120 s timeout, the other completed in 428 ms with `Firecracker exiting successfully`.

Note this still works with the i8042 *probing* disabled by the tuned command line — the reset port is untouched; what we skipped was enumerating it as an input device.

```rust
/// Halt the VM so the VMM exits.
///
/// RESTART, not POWER_OFF. Firecracker exposes no ACPI power button, so
/// POWER_OFF parks the guest at "reboot: System halted" and the VMM lives
/// on — the run hangs to its idle timeout holding all of its memory.
/// RESTART goes through the `reboot=k` i8042 reset, which the VMM watches.
fn power_off() -> ! {
    // SAFETY: reboot(2) with a valid command; it does not return on success,
    // and the `loop` covers the failure path so this function's `!` holds.
    unsafe { libc::reboot(libc::LINUX_REBOOT_CMD_RESTART) };
    loop {
        // SAFETY: pause(2) takes no arguments and only blocks.
        unsafe { libc::pause() };
    }
}
```

`mount_workspace` does more than the workspace. **The rootfs is mounted READ-ONLY and shared across every concurrent run**, so a guest that only mounts `/workspace` cannot write `/tmp`, `/var/tmp` or `$HOME` — and an agent CLI writes all three within seconds of starting. Mount a tmpfs over each before exec:

```rust
/// the shared rootfs is read-only, so everything the CLI expects to be
/// writable gets a per-boot tmpfs. These live in guest RAM and die with the
/// VM, which is the point: nothing a run scribbles outside its workspace
/// survives, and nothing it writes can reach another run's guest.
const TMPFS_DIRS: [&str; 3] = ["/tmp", "/var/tmp", "/root"];
```

Size them modestly (a fraction of the guest's memory) so a runaway write fills a tmpfs and fails the run rather than exhausting the guest and triggering the OOM killer against the CLI itself.

- [ ] **Step 6: Verify it builds static**

Run:
```bash
rustup target add x86_64-unknown-linux-musl
cargo build -p duck-guest-init --target x86_64-unknown-linux-musl --release
file target/x86_64-unknown-linux-musl/release/duck-guest-init
```
Expected: `ELF 64-bit LSB executable ... statically linked`. A dynamically-linked init cannot boot in a rootfs without a loader.

- [ ] **Step 7: Commit**

```bash
git add bin/duck-guest-init Cargo.toml
git commit -m "feat(guest): duck-guest-init, PID 1 inside a run's microVM

Mounts the workspace block device, execs the CLI with the host's env and
cwd, pumps stdout and stderr back over vsock as separate streams, reports
the exit code, and unmounts so the image is consistent for read-back.
Occupies the seat conmon had under podman.

The run manifest rides the kernel command line as one base64 token: the
cmdline is the only channel open before any device is up, and a cmdline
token cannot contain the spaces that argv and env freely do."
```

---

### Task 5: the Firecracker VMM client

The API side: configure and start one VM over its per-run unix socket. Peer of `podman_api.rs`, tested the same way — against a fake socket that asserts the emitted JSON.

**Files:**
- Create: `crates/services/sandbox/src/firecracker_api.rs`
- Modify: `crates/services/sandbox/src/lib.rs`
- Test: in-file `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `workspace_image`, `guest_proto`.
- Produces:
  - `pub struct VmConfig { pub kernel: PathBuf, pub rootfs: PathBuf, pub workspace: PathBuf, pub vcpus: u32, pub mem_mib: u64, pub manifest_token: String, pub tap: Option<String> }`
  - `pub fn boot_requests(cfg: &VmConfig) -> Vec<(&'static str, String, String)>` — `(method, path, json_body)` in the order Firecracker requires. Pure, so the whole configuration is unit-testable with no VMM.
  - `pub struct Vmm` with `pub async fn boot(socket: &Path, cfg: &VmConfig) -> Result<Self, String>`, `pub async fn shutdown(self) -> Result<(), String>`.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> VmConfig {
        VmConfig {
            kernel: PathBuf::from("/img/vmlinux"),
            rootfs: PathBuf::from("/img/rootfs.ext4"),
            workspace: PathBuf::from("/run/ws.ext4"),
            vcpus: 4,
            mem_mib: 8192,
            manifest_token: "AAAA".into(),
            tap: Some("fc-tap0".into()),
        }
    }

    /// Firecracker applies configuration in the order it is PUT, and
    /// InstanceStart must be last — a drive added after start is ignored, with
    /// no error, and the guest simply never sees its workspace.
    #[test]
    fn instance_start_is_the_last_request() {
        let reqs = boot_requests(&cfg());
        let (method, path, _) = reqs.last().expect("at least one request");
        assert_eq!(*method, "PUT");
        assert_eq!(*path, "/actions");
        assert!(
            reqs[..reqs.len() - 1].iter().all(|(_, p, _)| p != "/actions"),
            "only one start action"
        );
    }

    /// The rootfs is SHARED read-only across every concurrent run. A writable
    /// share would let one buyer's run corrupt another's guest.
    #[test]
    fn the_shared_rootfs_is_read_only_and_the_workspace_is_not() {
        let reqs = boot_requests(&cfg());
        let root = reqs.iter().find(|(_, p, _)| p == "/drives/rootfs").expect("rootfs drive");
        assert!(root.2.contains("\"is_read_only\":true"), "{}", root.2);
        let ws = reqs.iter().find(|(_, p, _)| p == "/drives/workspace").expect("workspace drive");
        assert!(ws.2.contains("\"is_read_only\":false"), "{}", ws.2);
    }

    #[test]
    fn limits_ride_the_machine_config() {
        let reqs = boot_requests(&cfg());
        let mc = reqs.iter().find(|(_, p, _)| p == "/machine-config").expect("machine-config");
        assert!(mc.2.contains("\"vcpu_count\":4"), "{}", mc.2);
        assert!(mc.2.contains("\"mem_size_mib\":8192"), "{}", mc.2);
    }

    /// The manifest reaches the guest ONLY through the kernel command line, and
    /// the guest halts without it — so a boot-source body that drops it is a
    /// run that hangs to its idle timeout.
    #[test]
    fn the_boot_source_carries_the_run_manifest_and_a_quiet_console() {
        let reqs = boot_requests(&cfg());
        let boot = reqs.iter().find(|(_, p, _)| p == "/boot-source").expect("boot-source");
        assert!(boot.2.contains("ducktape.run=AAAA"), "{}", boot.2);
        assert!(boot.2.contains("panic=1"), "a guest panic must halt, not hang: {}", boot.2);
    }

    /// The i8042 group and `quiet` are worth ~840 ms together, measured, and the
    /// saving is flat across every run shape. Losing a token here is a silent
    /// regression nobody notices — the run still works, just slower — so the
    /// cmdline is pinned by a test rather than by a comment.
    #[test]
    fn the_boot_args_keep_the_measured_boot_time_wins() {
        let reqs = boot_requests(&cfg());
        let boot = reqs.iter().find(|(_, p, _)| p == "/boot-source").expect("boot-source");
        for token in [
            "i8042.noaux",
            "i8042.nokbd",
            "i8042.nomux",
            "i8042.nopnp",
            "i8042.dumbkbd",
            "quiet",
        ] {
            assert!(boot.2.contains(token), "lost {token}, worth ~840ms total: {}", boot.2);
        }
    }

    /// `acpi=off` reads like a 69 ms win and is a correctness bug: Firecracker
    /// enumerates vCPUs through ACPI, so the guest comes up with ONE processor
    /// whatever `vcpu_count` says. Verified directly — vcpu_count=4 gives
    /// "Total of 4 processors activated" with ACPI and "Total of 1" without.
    /// A node would sell four cores and deliver one, with nothing in any log.
    #[test]
    fn acpi_is_never_disabled() {
        let boot_source = boot_requests(&cfg())
            .into_iter()
            .find(|(_, p, _)| p == "/boot-source")
            .expect("boot-source");
        assert!(
            !boot_source.2.contains("acpi=off"),
            "acpi=off silently drops every vcpu but the boot one: {}",
            boot_source.2
        );
    }

    /// A run with no tap is a run with no network device at all — not a run
    /// with an unconfigured one.
    #[test]
    fn no_tap_means_no_network_interface_request() {
        let mut c = cfg();
        c.tap = None;
        assert!(
            boot_requests(&c).iter().all(|(_, p, _)| !p.starts_with("/network-interfaces")),
            "a tapless config must emit no network device"
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sandbox-host firecracker_api`
Expected: FAIL — module and types undefined.

- [ ] **Step 3: Write `boot_requests`**

```rust
/// the ordered configuration Firecracker needs before it will start. ORDER IS
/// LOAD-BEARING: the VMM applies each PUT as it arrives and ignores a drive
/// added after `InstanceStart` — silently, so the guest simply never sees its
/// workspace and the run fails as an unexplained empty result.
pub fn boot_requests(cfg: &VmConfig) -> Vec<(&'static str, String, String)> {
    // Every token here was MEASURED, not copied from a tutorial — see
    // "Kernel command line" above. The i8042 group and `quiet` are worth ~840 ms
    // together, and that saving is flat across every run shape.
    //
    // `panic=1` so a guest panic HALTS rather than sitting at the kernel prompt
    // burning the run's whole idle timeout.
    //
    // NEVER add `acpi=off`. It looks like a 69 ms win and is a correctness bug:
    // Firecracker enumerates vCPUs through ACPI, so the guest comes up with ONE
    // processor no matter what `vcpu_count` says. A node would sell four cores
    // and deliver one, silently.
    let boot_args = format!(
        "console=ttyS0 reboot=k panic=1 pci=off quiet loglevel=1 \
         i8042.noaux i8042.nokbd i8042.nomux i8042.nopnp i8042.dumbkbd \
         init=/duck-guest-init ducktape.run={}",
        cfg.manifest_token
    );
    let mut reqs = vec![
        (
            "PUT",
            "/machine-config".to_string(),
            format!(
                r#"{{"vcpu_count":{},"mem_size_mib":{},"smt":false}}"#,
                cfg.vcpus, cfg.mem_mib
            ),
        ),
        (
            "PUT",
            "/boot-source".to_string(),
            serde_json::json!({
                "kernel_image_path": cfg.kernel,
                "boot_args": boot_args,
            })
            .to_string(),
        ),
        (
            "PUT",
            "/drives/rootfs".to_string(),
            serde_json::json!({
                "drive_id": "rootfs",
                "path_on_host": cfg.rootfs,
                // SHARED across every concurrent run. Writable would let one
                // buyer's run corrupt another's guest.
                "is_read_only": true,
                "is_root_device": true,
            })
            .to_string(),
        ),
        (
            "PUT",
            "/drives/workspace".to_string(),
            serde_json::json!({
                "drive_id": "workspace",
                "path_on_host": cfg.workspace,
                "is_read_only": false,
                "is_root_device": false,
            })
            .to_string(),
        ),
        (
            "PUT",
            "/vsock".to_string(),
            serde_json::json!({
                "guest_cid": 3,
                "uds_path": cfg.vsock_uds,
            })
            .to_string(),
        ),
    ];
    // No tap means NO network device — not an unconfigured one.
    if let Some(tap) = &cfg.tap {
        reqs.push((
            "PUT",
            "/network-interfaces/eth0".to_string(),
            serde_json::json!({ "iface_id": "eth0", "host_dev_name": tap }).to_string(),
        ));
    }
    reqs.push((
        "PUT",
        "/actions".to_string(),
        r#"{"action_type":"InstanceStart"}"#.to_string(),
    ));
    reqs
}
```

Add `pub vsock_uds: PathBuf` to `VmConfig` (the test's `cfg()` needs it too).

- [ ] **Step 3b: Write the socket client, and run the VMM under its jailer**

Reuse `podman_api`'s HTTP-over-unix-socket helpers (`read_response_head`, `parse_response`, `dechunk`) rather than writing a second client — move them to a shared `http_unix` module and say so in the commit.

`Vmm::boot` must spawn the VMM through **`jailer`**, never `firecracker` directly. The jailer is what puts a chroot, a seccomp filter, a cgroup and a uid/gid drop around the hypervisor process itself; a bare `firecracker` leaves the VMM — the one host process that talks to `/dev/kvm` — unsandboxed:

```rust
// the jailer creates <chroot_base>/firecracker/<id>/root, moves itself in,
// drops to uid/gid, applies its seccomp filter, and only then execs the VMM.
// Every path the VMM is handed must already be INSIDE that root, so the kernel,
// rootfs and workspace images are hard-linked in before boot.
let mut cmd = tokio::process::Command::new(&jailer);
cmd.args(["--id", vm_id])
    .args(["--exec-file", firecracker_bin.to_str().ok_or("non-utf8 firecracker path")?])
    .args(["--uid", &uid.to_string()])
    .args(["--gid", &gid.to_string()])
    .args(["--chroot-base-dir", chroot_base.to_str().ok_or("non-utf8 chroot base")?])
    .arg("--")
    .args(["--api-sock", "/run/firecracker.socket"]);
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p sandbox-host firecracker_api ; test ${PIPESTATUS[0]} -eq 0`
Expected: PASS, all five.

- [ ] **Step 5: Commit**

```bash
git add crates/services/sandbox/src/firecracker_api.rs crates/services/sandbox/src/lib.rs
git commit -m "feat(sandbox): the Firecracker VMM client

boot_requests is pure and returns the ordered (method, path, body) list,
so the whole VM configuration is unit-testable without a VMM. The order
is load-bearing: Firecracker ignores a drive added after InstanceStart,
silently, and the guest then never sees its workspace."
```

---

### Task 6: the run lifecycle — boot, stdio, wait, teardown

Wires the backend into `CliProvider::invoke` beside the podman path, replacing the `todo!("Task 6")` stubs from Task 1.

**Files:**
- Modify: `crates/services/provider/src/lib.rs`
- Test: `crates/services/provider/src/lib.rs` in-file tests

**Interfaces:**
- Consumes: everything from Tasks 1-5.
- Produces:
  - `RunControl::MicroVm(VmHandle)`, where `struct VmHandle { vmm: firecracker_api::Vmm, workspace: PathBuf, workdir: PathBuf, _tap: Option<tap::Tap> }`
  - `pub struct VmStdio { pub stdout: tokio::io::DuplexStream, pub stderr: tokio::io::DuplexStream, pub exit: tokio::sync::oneshot::Receiver<i32>, pub pump: tokio::task::JoinHandle<()> }` — deliberately the same shape as `podman_api::HeadlessIo` minus `stdin`, because a microVM run is headless and the guest init opens stdin as `/dev/null`.
  - `async fn microvm_boot(&self, args: &[String], workdir: &Path, ctx: &RunContext, auth: &RunAuth<'_>) -> Result<(VmHandle, VmStdio), String>`
  - `fn microvm_scratch(run_id: &str) -> PathBuf` — this run's private directory for its workspace image, vsock socket and jailer chroot. Per-run, because two concurrent runs sharing a path is two runs corrupting one image.

- [ ] **Step 1: Write the failing test**

```rust
    /// Teardown must run on EVERY exit path. A VMM left alive holds its memory,
    /// its tap, and its jailer chroot; a hundred of them is the node's RAM.
    #[tokio::test]
    async fn a_boot_failure_still_tears_the_vmm_down() {
        let provider = CliProvider::from_spec(
            broker_spec("c"),
            PathBuf::from("/usr/bin/c"),
            SandboxBackend::Firecracker {
                kernel: PathBuf::from("/nonexistent/vmlinux"),
                rootfs: PathBuf::from("/nonexistent/rootfs.ext4"),
            },
        );
        let ctx = RunContext::default();
        let err = provider
            .microvm_boot(&["--go".into()], Path::new("/nonexistent"), &ctx, &RunAuth::default())
            .await
            .expect_err("a missing kernel cannot boot");
        assert!(err.contains("kernel") || err.contains("vmlinux"), "{err}");
    }

    /// The workspace round trip is the run's OUTPUT. A run that computes the
    /// right answer and loses it on read-back is a failed run.
    #[test]
    fn the_workspace_image_path_is_inside_the_runs_own_scratch() {
        let a = microvm_scratch("run-a");
        let b = microvm_scratch("run-b");
        assert_ne!(a, b, "two runs must not share a workspace image path");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p provider-host microvm`
Expected: FAIL — `microvm_boot` and `microvm_scratch` undefined.

- [ ] **Step 3: Implement the lifecycle**

Add a `RunControl::MicroVm` arm and route it in `wait`/`terminate`. In `invoke`, the backend split becomes a match with three arms (`Podman`, `Firecracker`, `Bare`) and no wildcard.

The boot sequence, in order, each step undoing the previous on failure:
1. `workspace_image::sized_for` then `build` into the run's scratch dir.
2. `firecracker_api::Vmm::boot` with the manifest token from `manifest::encode`.
3. Accept the guest's vsock connection; decode frames into the two `DuplexStream`s the existing output loop already reads, so the refreshable-timeout loop is byte-identical to the podman path.
4. On `Frame::Exit`, `shutdown` the VMM, then `workspace_image::read_back` over the workdir.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p provider-host ; test ${PIPESTATUS[0]} -eq 0`
Then: `cargo clippy -p provider-host --tests --no-deps`
Expected: PASS and clean.

- [ ] **Step 5: Commit**

```bash
git add crates/services/provider/src/lib.rs
git commit -m "feat(sandbox): boot, stream, wait and tear down a run's microVM

The guest's vsock frames are decoded into the same two DuplexStreams the
podman attach path feeds, so the refreshable-timeout output loop is
byte-identical across backends. Teardown runs on every exit path: a VMM
left alive holds its memory, its tap and its jailer chroot."
```

---

### Task 7: tap networking and the egress ruleset

The policy is the one that already ships — public allowed, the operator's private network denied. Only the enforcement point moves: from `nft` inside a container netns via a createRuntime hook to the host side of the run's tap.

**Files:**
- Create: `crates/services/sandbox/src/tap.rs`
- Modify: `crates/services/sandbox/src/lib.rs`
- Test: in-file tests

**Interfaces:**
- Consumes: `podman_api::egress_nftables` (the existing generator — reuse it, do not write a second).
- Produces: `pub fn tap_egress_ruleset(tap: &str, host_ip: &str, resolver_ip: &str, ports: &[u16]) -> String`, `pub struct Tap` with RAII teardown.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// The policy is a DENYLIST of the operator's private network, not an
    /// allowlist of the public internet. Losing a deny line silently exposes
    /// the operator's LAN to a stranger's run.
    #[test]
    fn the_ruleset_denies_the_private_ranges_and_permits_the_rest() {
        let rules = tap_egress_ruleset("fc-tap0", "169.254.1.2", "169.254.1.1", &[8080]);
        for private in ["10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16", "100.64.0.0/10"] {
            assert!(rules.contains(private), "missing deny for {private}:\n{rules}");
        }
        assert!(rules.contains("fc-tap0"), "the ruleset must be scoped to this run's tap");
    }

    /// DNS is scoped to ONE resolver, never a blanket :53 — the tailnet and LAN
    /// resolvers must stay unreachable even though the guest can see them in
    /// its resolv.conf.
    #[test]
    fn dns_is_scoped_to_the_single_resolver() {
        let rules = tap_egress_ruleset("fc-tap0", "169.254.1.2", "169.254.1.1", &[]);
        assert!(rules.contains("169.254.1.1"), "{rules}");
        assert!(!rules.contains("dport 53 accept"), "no blanket DNS: {rules}");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sandbox-host tap`
Expected: FAIL — module undefined.

- [ ] **Step 3: Implement the tap and its ruleset**

`tap_egress_ruleset` composes `egress_nftables`'s existing body with a tap-scoped chain hook. `Tap::create` shells out to `ip tuntap add`/`ip addr`/`ip link set up` and `Drop` removes it — a leaked tap survives its VM and collides with the next run's name.

- [ ] **Step 3b: Teach the broker how a microVM guest reaches it**

`CliProvider::start_broker` currently chooses between `Reachability::Loopback` (the bare harness) and `Reachability::HostGateway("host.containers.internal")` (podman). A microVM guest is in neither: it reaches the host at **its tap's host-side address**, which the run itself allocated, so the gateway is a per-run value rather than a compile-time name.

Write the failing test first:

```rust
    /// A microVM guest cannot resolve `host.containers.internal` — that name is
    /// podman's, injected into a container's /etc/hosts. A broker that hands a
    /// microVM run that base_url gives it an endpoint it can never dial, and
    /// the run fails at its first model call rather than at boot.
    #[tokio::test]
    async fn a_microvm_run_reaches_the_broker_at_its_tap_gateway() {
        let provider = CliProvider::from_spec(
            broker_spec("c"),
            PathBuf::from("/usr/bin/c"),
            SandboxBackend::Firecracker {
                kernel: PathBuf::from("/img/vmlinux"),
                rootfs: PathBuf::from("/img/rootfs.ext4"),
            },
        );
        let reach = provider.broker_reachability(Some("10.200.1.1"));
        assert!(
            matches!(reach, broker::Reachability::HostGateway(host) if host == "10.200.1.1"),
            "{reach:?}"
        );
        assert!(!format!("{reach:?}").contains("containers.internal"));
    }
```

This requires `Reachability::HostGateway` to carry an owned `String` rather than a `&'static str`. That is a one-line widening in `broker/src/lib.rs`; make it, and route the two existing construction sites.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p sandbox-host ; test ${PIPESTATUS[0]} -eq 0`

- [ ] **Step 5: Commit**

```bash
git add crates/services/sandbox/src/tap.rs crates/services/sandbox/src/lib.rs
git commit -m "feat(sandbox): the run's tap and its egress ruleset

Same policy the container path already enforced — public allowed, the
operator's private network denied, DNS scoped to one resolver rather
than a blanket :53. Only the enforcement point moves: the host side of
the run's tap, outside the guest entirely, so the createRuntime hook and
its nsenter are no longer needed."
```

---

### Task 8: config, artifacts, and a hardware-gated end-to-end smoke

Makes the backend selectable and proves it on real hardware.

**Files:**
- Modify: `bin/node/src/config/resolve.rs`, `bin/node/src/config/node_toml.rs`, `bin/node/src/cli.rs`
- Create: `ops/build-guest-rootfs.sh`
- Test: `bin/node/src/config/resolve.rs` in-file tests, and an `#[ignore]`d e2e in `crates/services/provider/src/lib.rs`

- [ ] **Step 1: Write the failing config test**

```rust
        // firecracker ⇒ the microVM backend with the operator's image paths.
        std::fs::write(
            dir.join("node.toml"),
            format!("{base}\n[sandbox]\nruntime = \"firecracker\"\nkernel = \"/img/vmlinux\"\nrootfs = \"/img/rootfs.ext4\"\ncores = 4\nmem_gb = 8\n"),
        )
        .expect("write");
        let fc = resolve(&dir.join("node.toml")).expect("firecracker accepted");
        assert!(matches!(
            fc.service.sandbox,
            Some(SandboxBackend::Firecracker { .. })
        ));

        // a firecracker table without its image paths is a boot error, not a
        // node that boots and fails every run.
        std::fs::write(
            dir.join("node.toml"),
            format!("{base}\n[sandbox]\nruntime = \"firecracker\"\ncores = 4\nmem_gb = 8\n"),
        )
        .expect("write");
        let err = resolve(&dir.join("node.toml")).expect_err("no kernel path refused");
        assert!(err.contains("kernel"), "{err}");
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p node-bin --bin ducktape resolve`
Expected: FAIL — `"firecracker"` hits the unknown-runtime arm.

- [ ] **Step 3: Add the `firecracker` arm, its image keys, and the mandatory limits**

Extend `SandboxToml` with `kernel: Option<PathBuf>` and `rootfs: Option<PathBuf>`, and add the arm to `resolve_sandbox`. A missing path is an error naming the key.

**The absent-limit case must change with it**, per the spec: today a missing `cores`/`mem_gb` omits `resource_limits` entirely and the run is unlimited. A VM has no such state — it is given a size at configuration time, so "unlimited" is unrepresentable. Add the test:

```rust
        // a microVM has no "unlimited": every VM is sized at configuration
        // time, so a firecracker table that probes to nothing is a boot error
        // rather than a node that boots and cannot size a single guest.
        std::fs::write(
            dir.join("node.toml"),
            format!("{base}\n[sandbox]\nruntime = \"firecracker\"\nkernel = \"/img/vmlinux\"\nrootfs = \"/img/rootfs.ext4\"\ncores = 0\nmem_gb = 0\n"),
        )
        .expect("write");
        // `0` means "probe" for podman; for firecracker a probe that yields
        // nothing must not silently become an unsized VM.
        let resolved = resolve(&dir.join("node.toml"));
        if let Ok(r) = &resolved {
            assert!(
                r.service.sandbox_capacity.contains_key("cores")
                    && r.service.sandbox_capacity.contains_key("mem_gb"),
                "a firecracker node must know both dimensions or refuse to boot"
            );
        }
```

- [ ] **Step 4: Write `ops/build-guest-rootfs.sh`**

Builds the shared rootfs: a minimal base, the statically-linked `duck-guest-init` at `/duck-guest-init`, and the agent CLIs. Emits `rootfs.ext4` via the same `mke2fs -d` path Task 2 uses, so there is one image-building mechanism in the tree rather than two.

- [ ] **Step 5: Write the hardware-gated e2e**

```rust
    /// Real Firecracker gate. Skipped unless the host can actually boot a VM,
    /// because a guard that reports "ready" on a host without /dev/kvm turns a
    /// missing group membership into a phantom bug hunt.
    #[tokio::test]
    #[ignore = "requires /dev/kvm, firecracker, and built guest artifacts"]
    async fn firecracker_hardware_smoke() {
        let (Ok(kernel), Ok(rootfs)) = (
            std::env::var("DUCKTAPE_GUEST_KERNEL"),
            std::env::var("DUCKTAPE_GUEST_ROOTFS"),
        ) else {
            eprintln!("skipping: set DUCKTAPE_GUEST_KERNEL and DUCKTAPE_GUEST_ROOTFS");
            return;
        };
        hardware_sandbox_smoke(
            "firecracker-hardware",
            SandboxBackend::Firecracker {
                kernel: kernel.into(),
                rootfs: rootfs.into(),
            },
        )
        .await;
    }
```

- [ ] **Step 6: Run everything**

```bash
cargo test -p sandbox-host -p provider-host -p node-bin ; test ${PIPESTATUS[0]} -eq 0
cargo clippy -p sandbox-host -p provider-host -p node-bin --tests --no-deps
./ops/firecracker-setup.sh && ./ops/build-guest-rootfs.sh
DUCKTAPE_GUEST_KERNEL=... DUCKTAPE_GUEST_ROOTFS=... \
  cargo test -p provider-host firecracker_hardware_smoke -- --ignored --nocapture
```

- [ ] **Step 7: Commit and open the PR**

```bash
git add -A
git commit -m "feat(sandbox): select the Firecracker backend from node.toml

runtime = \"firecracker\" with explicit kernel and rootfs paths. A table
missing either is a boot error rather than a node that boots and fails
every run — the spec's rule that a VM's images and size come from config
and never from a probe."
```

---

## Follow-on plans (not this one)

- **Snapshot/restore.** Already measured (see above): restore is ~12 ms and flat in guest memory, against a 428 ms - 2 s cold boot. The backend is correct without it, so it is not a blocker for shipping — but it is the only fix for the memory-scaling cost, so it becomes a prerequisite the moment a node sells large-`mem_gb` runs at a competitive start latency. The work in that plan is **not** the restore call; it is the three costs around it: a `Uffd` memory backend so the first work is not paying lazy page faults, a build step that produces one snapshot per machine shape, and a store for the resulting memory files (~31 GB for a 1/2/4/8/16 GiB ladder) with invalidation keyed to the rootfs version.
- **Egress logging and the public-egress toggle.** Spec step 3's remaining half.
- ~~**Remove the podman path.**~~ DONE in this branch, not a follow-on: `PodmanService`, the libpod client, the attach framing, the OCI `createRuntime` egress hook and `pasta`/`conmon` are deleted, along with `podman_api.rs` entire. The plan had this last, "until the Firecracker path has run real work" — it has (see the table below), and keeping a second backend would have been the dual-path code this repo's instructions forbid.
- **An interactive pty inside the guest.** The one capability lost with podman. `InteractiveSession` still exists for a local pty, but a microVM run has no guest-side pty, so an interactive session under Firecracker returns an explicit error naming what is missing rather than silently degrading. The live podman TUI suites were deleted with the backend they drove.

## Open questions carried from the spec

- Guest kernel: build our own or track a distro's? Decides the CVE workflow. Task 8's `build-guest-rootfs.sh` needs an answer before it is more than a stub.
- Whether the balloon device ships in v1 or waits for the snapshot work.

---

## Implementation notes (2026-08-22, all eight tasks landed)

What the plan got wrong, and what the running code does instead. Each of these
was found by running the thing, not by reading it, so each carries the
measurement that forced the change.

### 1. The manifest does NOT ride the kernel command line

Task 4 planned one base64 token on the cmdline, "so the guest needs no second
block device to learn what to run". Firecracker caps a cmdline near 2 KiB, and
a run's argv and env come from a capability SPEC — so that cap is one a spec
author crosses by adding an environment variable. Codex's broker overrides
alone made a **2094-byte** cmdline and the VMM refused to boot:

```
Boot source error: The kernel command line is invalid:
Invalid cmdline capacity provided.
```

The manifest now rides a 64 KiB device of its own, read RAW — no filesystem to
mount, which matters because the manifest is what says which filesystems to
mount. It is attached immediately after the root device so its name never moves
when a run gains or loses its agent volume, and a manifest past the device is
refused ON THE HOST, naming the size. The cmdline is now fixed and identical
for every run, guarded by a test. The `base64` dependency went with the
encoding that needed it.

### 2. A PATH entry hands over its commands, not its tree

Nothing in the plan accounted for the fact that a VM COPIES where a container
bind-mounted. A declared PATH directory was copied entire into the run's
read-only asset image. The node's own binary lives in a build directory, so a
real run measured a **39 GB tree against the 0.95 GB of it the run could ever
name** — one file. It filled a tmpfs and died copying an `.rlib`.

An asset is now tagged with what it is. `GuestAsset::Commands` hands over the
executables at its top level and nothing else — not a heuristic, but what a
PATH entry means, since resolution never recurses and never resolves a
non-executable. `GuestAsset::Whole` (the skills tree, the context doc) still
crosses entire.

### 3. The run's images do not belong on `XDG_RUNTIME_DIR`

The plan put the whole run directory there for `SUN_LEN`. That directory is a
tmpfs sized at a fraction of RAM, so a run's block devices were being built in
the node's memory — a 9.1 GB tmpfs, filled. Only the vsock socket stays there
now; the images, boot config and console live on disk.

### 4. Firecracker's vsock maps a half-close onto a RESET

The host's feed task owned the connection's write half and returned after
sending `StdinEof`. Firecracker does not carry a host-side half-close through
as a half-close — it resets the connection. Every frame the guest wrote after
the prompt (its whole output and its exit code) died on `EPIPE`, and the run
reached the operator as "produced nothing, reported no exit code".

The feed task now parks until the run ends. A `UnixStream::pair` honours the
half-close, so an integration-shaped test passes either way and guards nothing;
the guard is a source-parsing test instead.

Relatedly, the guest halted the instant it wrote its exit frame. Firecracker
relays guest writes asynchronously, so a reset that close behind the last write
drops whatever has not been relayed — always the last frame. The host now
closes on `Exit` and the guest waits for that close as its acknowledgement.

### 5. A read-only rootfs still needs four writable directories

`/tmp`, `/run`, `/var/tmp` and the run's `HOME`. An ordinary userland expects
to write all four, and a CLI that cannot fails in whatever way it happens to
fail, far from the cause — measured, `claude` exiting 1 with
`EROFS: mkdir '/tmp/claude-0'`. The init mounts a per-run tmpfs on each.

### 6. A read-only DRIVE must be mounted read-only

Firecracker enforces `is_read_only` at the device, so mounting one read-write
fails outright with `EACCES` — which reaches the operator as "the guest never
dialled back", naming nothing. The guest cannot infer the bit, so each
manifest mount carries it.

### 7. Two shipped-behaviour gaps the container backend had hidden

- The workspace was never read back. `collect_workspace` now runs BEFORE the
  exit-code check: a run that exited non-zero still produced work.
- `ro_paths` had no delivery mechanism at all under a VM. Hence the per-run
  asset image.

### 8. Every host-script test fixture had to become spec ARGV

A microVM mounts nothing from the host, so an executor a node lends has to
already be in the guest rootfs. A staged `provider.sh` arrives as
`execve /opt/duck/bin/provider.sh` and exit 126. The e2e fixtures
(`dispatch_e2e`, `dogfood_loop_e2e`, the provider hardware smoke) now run
`sh -c "<script>"` against the `sh` the rootfs ships. This also surfaced a real
bug: `microvm_boot` substituted `args[0]` with the executor's guest path on the
assumption that `args` carries argv[0]. It does not, so a spec's first real
argument was being eaten — invisible to any run with no arguments, which is
exactly the shape the first end-to-end test had.

### Verified end to end

On a real VMM, with the operator's own subscriptions:

| test | what it proves |
| --- | --- |
| `microvm_echo_round_trips_through_invoke` | prompt in over vsock, output back, exit code, workspace read back |
| `firecracker_hardware_smoke` | a spec's argv and prompt reach a CLI in the guest; the file it wrote comes back |
| `claude_model_turn_in_a_microvm` | a real claude turn through the vsock tunnel to the host broker — answered `PONG` |
| `codex_model_turn_in_a_microvm` | the same for codex — answered `PONG` |

The credential never enters the guest in either model turn: the guest has no
network device at all, so the CLI's only route to the model API is the tunnel
to this run's broker, which attaches the upstream token host-side.
