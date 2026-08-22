//! the sandbox backend seam: how a provider child is spawned. every backend
//! is an audited in-tree adapter — a run NEVER executes bare on the host, so
//! the seam has no unsandboxed variant a config could select.
//!
//! This module owns the [`SandboxBackend`] enum + its boot probe. The Podman
//! backend's execution — building each run's neutral-path `SpecGenerator`,
//! driving create/start/attach/wait over the node-private libpod socket, and
//! the egress firewall — lives in [`crate::podman_api`]; there is no `podman`
//! CLI path any more.

use std::path::{Path, PathBuf};

/// how a provider child is spawned — always inside an isolation adapter; a
/// bare host spawn is unrepresentable here by design ("nothing ever runs
/// directly on the node"). `Podman` wraps each run in a rootless container. a
/// node sandboxes EVERY run it makes — demandless ones included — because a
/// sandboxed node sandboxes everything; the numeric limit flags are added only
/// for the dimensions actually present on the run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxBackend {
    /// one Firecracker microVM per run: hard vcpu/memory limits enforced by the
    /// hypervisor, no container runtime inside the guest. `kernel` and `rootfs`
    /// are the immutable images every run boots from — shared read-only, never
    /// written by a run.
    Firecracker {
        kernel: std::path::PathBuf,
        rootfs: std::path::PathBuf,
    },
    /// test-harness spawn: the bin exec'd directly, compiled ONLY into test
    /// builds so the run loop stays testable without a container runtime on the
    /// test host. a shipped binary cannot express a bare spawn — the variant
    /// does not exist outside `cfg(test)` / the `testkit` feature, and nothing
    /// but a dev-dependency turns that feature on.
    #[cfg(any(test, feature = "testkit"))]
    Bare,
}

impl SandboxBackend {
    /// the host runtime binary this adapter drives.
    pub fn runtime_bin(&self) -> &'static str {
        match self {
            SandboxBackend::Firecracker { .. } => "firecracker",
            #[cfg(any(test, feature = "testkit"))]
            SandboxBackend::Bare => "sh",
        }
    }

    /// the host tools this adapter shells out to, each paired with what it is
    /// for. Checked at boot alongside [`Self::runtime_bin`] so a host missing
    /// one fails loudly here instead of mid-run.
    ///
    /// The pairing is load-bearing for the error message. A tool once went
    /// missing from this list under the previous backend; a host without it
    /// passed this probe, ran the e2e suite, and failed 156 s later as `timed
    /// out waiting for the agent reply to post` — a message naming neither the
    /// runtime nor the tool, which reads like a product defect. A guard that
    /// reports "ready" while the runtime cannot start a run is worse than no
    /// guard.
    fn required_tools(&self) -> &'static [(&'static str, &'static str)] {
        match self {
            SandboxBackend::Firecracker { .. } => &[
                ("mke2fs", "builds each run's workspace block image"),
                ("debugfs", "reads that image back after the guest exits"),
                ("nft", "the egress firewall on the run's tap device"),
            ],
            #[cfg(any(test, feature = "testkit"))]
            SandboxBackend::Bare => &[],
        }
    }

    /// whether this run spawns through the test-only bare harness (host paths,
    /// no mount canonicalization). always false in shipped code.
    pub fn is_bare_test(&self) -> bool {
        #[cfg(any(test, feature = "testkit"))]
        {
            matches!(self, SandboxBackend::Bare)
        }
        #[cfg(not(any(test, feature = "testkit")))]
        {
            false
        }
    }

    /// verify this host can actually run the chosen adapter: the runtime binary
    /// must be executable somewhere on `PATH`, every tool in
    /// [`Self::required_tools`] must be present, and the adapter's own host
    /// capabilities must hold. a config naming an unusable runtime is a loud
    /// boot error — there is no bare fallback.
    ///
    /// All of it is a hard dependency, so a missing one fails at boot, never as
    /// a silently unsandboxed / unfirewalled run.
    pub fn probe(&self) -> Result<PathBuf, String> {
        let bin = self.runtime_bin();
        let found = crate::host_tools::find_on_path(bin).ok_or_else(|| {
            format!("sandbox runtime {bin:?} is not executable on PATH; install it or pick a runtime this host provides")
        })?;
        for (tool, why) in self.required_tools() {
            if crate::host_tools::find_system_tool(tool).is_none() {
                return Err(format!(
                    "{tool} is not executable on PATH or a standard sbin dir; the {bin} sandbox \
                     requires it ({why}) — install it"
                ));
            }
        }
        self.probe_host_capabilities()?;
        Ok(found)
    }

    /// the adapter-specific host state a tool check cannot express.
    fn probe_host_capabilities(&self) -> Result<(), String> {
        match self {
            SandboxBackend::Firecracker { kernel, rootfs } => {
                probe_kvm()?;
                for (label, image) in [("kernel", kernel), ("rootfs", rootfs)] {
                    if !image.is_file() {
                        return Err(format!(
                            "the Firecracker {label} image is missing or not a file; build it with \
                             ops/build-guest-rootfs.sh, or point [sandbox] at one that exists"
                        ));
                    }
                }
                Ok(())
            }
            #[cfg(any(test, feature = "testkit"))]
            SandboxBackend::Bare => Ok(()),
        }
    }
}

/// `/dev/kvm` must open read-write for THIS process — the check that actually
/// predicts a boot.
///
/// Presence is not enough and neither is group membership on paper: `usermod -aG
/// kvm` does not touch a session that is already running, so a host can list the
/// group and still get `EACCES` until the next login. Opening the device is the
/// only question whose answer matches what Firecracker will see.
fn probe_kvm() -> Result<(), String> {
    let dev = Path::new("/dev/kvm");
    if !dev.exists() {
        return Err(
            "/dev/kvm is absent; the Firecracker sandbox needs hardware virtualisation — check \
             that the CPU exposes vmx/svm and that the kvm module is loaded"
                .into(),
        );
    }
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(dev)
        .map(drop)
        .map_err(|e| {
            format!(
                "/dev/kvm exists but this process cannot open it read-write ({e}); add the node's \
                 user to the kvm group (a running session does not pick that up — log in again, \
                 or start the node under `sg kvm`)"
            )
        })
}
