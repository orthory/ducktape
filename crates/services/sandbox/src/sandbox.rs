//! the sandbox backend seam: how a provider child is spawned. every backend
//! is an audited in-tree adapter — a run NEVER executes bare on the host, so
//! the seam has no unsandboxed variant a config could select.
//!
//! This module owns the [`SandboxBackend`] enum + its boot probe. The Podman
//! backend's execution — building each run's neutral-path `SpecGenerator`,
//! driving create/start/attach/wait over the node-private libpod socket, and
//! the egress firewall — lives in [`crate::podman_api`]; there is no `podman`
//! CLI path any more.

use std::path::PathBuf;

/// how a provider child is spawned — always inside an isolation adapter; a
/// bare host spawn is unrepresentable here by design ("nothing ever runs
/// directly on the node"). `Podman` wraps each run in a rootless container. a
/// node sandboxes EVERY run it makes — demandless ones included — because a
/// sandboxed node sandboxes everything; the numeric limit flags are added only
/// for the dimensions actually present on the run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxBackend {
    Podman {
        image: String,
        /// the node-private rootless podman socket this backend drives (libpod
        /// REST over a unix socket — never the `podman` CLI). Owned by the
        /// node's [`crate::PodmanService`], isolated from any other podman on
        /// the host.
        socket: std::path::PathBuf,
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
            SandboxBackend::Podman { .. } => "podman",
            #[cfg(any(test, feature = "testkit"))]
            SandboxBackend::Bare => "sh",
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

    /// verify this host can actually run the chosen adapter: the runtime
    /// binary must be executable somewhere on `PATH`. a config naming an
    /// unusable runtime is a loud boot error — there is no bare fallback.
    /// Podman additionally requires `pasta` (the netns backend — podman 6's only
    /// one; the run uses `nsmode = "pasta"` for deterministic host + DNS
    /// addresses), `nft` + `nsenter` (the egress firewall the createRuntime
    /// hook installs in each run's netns), and `conmon` (the per-container
    /// monitor podman spawns; without it podman answers
    /// `could not find a working conmon binary` and serves nothing).
    ///
    /// `conmon` is checked HERE rather than left to podman because this probe
    /// is what the e2e skip guards key on. While it was missing from the list,
    /// a host with the other three passed the guard, ran the suite, and failed
    /// 156 s later as `timed out waiting for the agent reply to post` — a
    /// message that names neither podman nor conmon, and reads like a product
    /// defect. A guard that reports "ready" while the runtime cannot start a
    /// container is worse than no guard: it converts a missing package into a
    /// phantom bug hunt.
    ///
    /// All are hard dependencies, so a missing one fails at boot, never as a
    /// silently unsandboxed / unfirewalled run.
    pub fn probe(&self) -> Result<PathBuf, String> {
        let bin = self.runtime_bin();
        let found = find_on_path(bin).ok_or_else(|| {
            format!("sandbox runtime {bin:?} is not executable on PATH; install it or pick a runtime this host provides")
        })?;
        if matches!(self, SandboxBackend::Podman { .. }) {
            for dep in ["pasta", "nft", "nsenter", "conmon"] {
                if crate::podman_api::find_system_tool(dep).is_none() {
                    return Err(format!(
                        "{dep} is not executable on PATH or a standard sbin dir; the Podman \
                         sandbox requires it (pasta = netns, nft + nsenter = egress firewall, \
                         conmon = the container monitor podman spawns per run) — install it"
                    ));
                }
            }
        }
        Ok(found)
    }
}

/// first executable named `bin` on `PATH`, if any.
fn find_on_path(bin: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(bin))
        .find(|candidate| crate::is_executable(candidate))
}


