//! Process resource limits required by the production node. The open-file
//! raise lives in `node::resource_limits` — the desktop app needs the same
//! one; only the malloc arena cap is node-shaped.

/// Eight arenas bound the fan-out (glibc's default cap is 8 x cores) while
/// leaving malloc more lock parallelism than the node's concurrently
/// allocating threads ever contend for — small allocations are served
/// lock-free from per-thread tcache regardless of arena count.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
const MALLOC_ARENA_CAP: libc::c_int = 8;

/// Cap glibc's malloc arena count before any thread spawns. Left uncapped,
/// ~150 node threads materialize dozens of 64 MB arena regions whose dirty
/// pages THP then rounds up to 2 MB hugepages — ~100-250 MB of idle RSS that
/// holds no live data. Failure remains non-fatal: the node runs correctly on
/// default arena behavior, just fatter.
pub(crate) fn cap_malloc_arenas() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    // SAFETY: mallopt has no memory-safety preconditions; M_ARENA_MAX with a
    // positive value is a documented parameter.
    if unsafe { libc::mallopt(libc::M_ARENA_MAX, MALLOC_ARENA_CAP) } == 0 {
        eprintln!("[node] warning: could not cap malloc arenas");
    }
}
