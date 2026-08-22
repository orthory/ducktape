//! the sandbox muscle: how a node-local child is spawned in isolation.
//!
//! Every run gets its own Firecracker microVM. [`sandbox`] owns the
//! [`SandboxBackend`] seam and its boot probe; [`microvm`] owns the lifecycle
//! (boot, stdio, teardown, workspace read-back); [`firecracker_api`] owns the
//! VM configuration; [`workspace_image`] moves the workspace in and out as a
//! block device, since a microVM has no shared filesystem.
//!
//! [`guest_proto`] and [`guest_manifest`] are the host<->guest contract, shared
//! verbatim with `duck-guest-init`.
//!
//! This crate is pure muscle — it decides nothing about WHICH executor runs or
//! with what credentials. That is `capability-host`'s job, and it is the only
//! in-tree caller of the run path.

use std::path::Path;

// unix-only: the VMM client speaks over unix sockets and the storage path
// shells out to e2fsprogs.
#[cfg(unix)]
pub mod egress;
#[cfg(unix)]
pub mod firecracker_api;
pub mod guest_paths;
#[cfg(unix)]
pub mod host_tools;
#[cfg(unix)]
pub mod microvm;
pub mod sandbox;
// the host<->guest contract. Both files are included verbatim by the guest init
// (`#[path = ...]`) rather than depended on, so the wire format and the run
// manifest exist in exactly one copy while PID 1 stays free of tokio + tracing.
pub mod guest_manifest;
pub mod guest_proto;
// the microVM backend's storage: shells out to e2fsprogs and walks unix modes.
#[cfg(unix)]
pub mod workspace_image;

#[cfg(unix)]
pub use egress::tap_egress_nftables;
pub use guest_paths::GuestLayout;
#[cfg(unix)]
pub use host_tools::{find_on_path, find_system_tool};
#[cfg(unix)]
pub use microvm::{MicroVm, MicroVmIo};
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
