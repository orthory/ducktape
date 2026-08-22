//! the sandbox muscle: how a node-local child is spawned in isolation.
//!
//! [`sandbox`] owns the [`SandboxBackend`] seam and its boot probe.
//! [`podman_api`] owns the backend's execution: the node-private rootless
//! podman driven over its libpod REST socket, the neutral-path container spec,
//! and the egress nft firewall each run's netns gets.
//!
//! This crate is pure muscle — it decides nothing about WHICH executor runs or
//! with what credentials. That is `capability-host`'s job, and it is the only
//! in-tree caller of the run path.

use std::path::Path;

// unix-only: the libpod client speaks over a unix socket, and the egress hook
// enters a netns.
#[cfg(unix)]
pub mod podman_api;
pub mod sandbox;

#[cfg(unix)]
pub use podman_api::{PodmanService, egress_nftables, reap_by_label, run_egress_hook};
pub use sandbox::SandboxBackend;

/// whether `p` is a file this process could exec. the shared predicate behind
/// both PATH walks: the sandbox runtime probe here, and capability-host's
/// executor discovery.
pub fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    p.is_file()
        && std::fs::metadata(p)
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
}
