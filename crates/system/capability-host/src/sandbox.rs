//! the sandbox backend seam: how a provider child is spawned. Direct is the
//! historical spawn; Podman wraps the identical argv in a rootless container
//! that enforces the run's numeric limits. paths are mounted at identical
//! container paths so workdir/session/skill logic upstream stays path-blind.
//! HOME is NOT mounted: only the spec's [sandbox] rw_dirs (CLI auth/state)
//! cross the boundary, so the node's data dir and user key stay outside —
//! the D7 enforcement mechanism the provider doc deferred.
//! ponytail: --network=host keeps loopback MCP reachable; a private netns
//! with a gateway route is the upgrade path if network isolation matters.
//!
//! Tart (Apple Silicon, Virtualization.framework) is the macOS-guest backend:
//! a run APFS-COW-clones a base image, runs it, and deletes the clone. its pure
//! argv assembly ([`wrap_tart`]) lives here beside podman's; the impure clone/
//! delete lifecycle + the [`TART_MAX_CONCURRENT`] gate live in the CliProvider
//! spawn path (lib.rs). tart CLI assumptions are documented at [`wrap_tart`].

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// how a provider child is spawned. `Direct` is the plain host spawn (the
/// historical behavior, and every wired call site today); `Podman` wraps each
/// run in a rootless container. a node whose provider set uses `Podman`
/// sandboxes EVERY run it makes — demandless ones included — because a
/// sandboxed node sandboxes everything; the numeric limit flags are added only
/// for the dimensions actually present on the run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum SandboxBackend {
    #[default]
    Direct,
    Podman {
        image: String,
    },
    /// macOS / Apple Silicon: each run APFS-COW-clones `image`, runs the guest
    /// under a process-wide concurrency cap of 2 ([`TART_MAX_CONCURRENT`]), and
    /// deletes the clone on completion. see [`wrap_tart`] for the pure argv and
    /// the CliProvider spawn path for the clone/delete lifecycle.
    Tart {
        image: String,
    },
}

/// translate a provider invocation into a `podman run` argv — PURE, no I/O, so
/// it unit-tests without podman installed. mounts every path at its IDENTICAL
/// container path (nothing upstream translates paths): the workdir rw, the
/// executor bin ro, each `ro_paths` entry ro (PATH-entry dirs), each `rw_dirs`
/// entry rw (the spec's CLI auth/state dirs). only the limit dimensions this
/// backend knows how to enforce (`cores` → `--cpus`, `mem_gb` → `--memory`)
/// become flags; an unknown dimension (e.g. `gpu`) is silently ignored — the
/// scheduler already matched it, the backend enforces only what it can.
// one flat argument per translation input keeps this a PURE, dependency-free
// function (no context struct to build in tests); the alternative bundle would
// exist only to appease the lint.
#[allow(clippy::too_many_arguments)]
pub fn wrap_podman(
    image: &str,
    bin: &Path,
    args: &[String],
    workdir: &Path,
    envs: &[(String, String)],
    ro_paths: &[PathBuf],
    rw_dirs: &[PathBuf],
    limits: &BTreeMap<String, u64>,
) -> (PathBuf, Vec<String>) {
    // -i keeps stdin open: the prompt is fed on the child's stdin.
    let mut argv: Vec<String> = vec![
        "run".into(),
        "--rm".into(),
        "--network=host".into(),
        "-i".into(),
    ];
    if let Some(cores) = limits.get("cores") {
        argv.extend(["--cpus".into(), cores.to_string()]);
    }
    if let Some(mem) = limits.get("mem_gb") {
        argv.extend(["--memory".into(), format!("{mem}g")]);
    }
    argv.extend(["-v".into(), format!("{d}:{d}", d = workdir.display())]);
    argv.extend(["-w".into(), workdir.display().to_string()]);
    argv.extend(["-v".into(), format!("{b}:{b}:ro", b = bin.display())]);
    for p in ro_paths {
        argv.extend(["-v".into(), format!("{p}:{p}:ro", p = p.display())]);
    }
    for d in rw_dirs {
        argv.extend(["-v".into(), format!("{d}:{d}", d = d.display())]);
    }
    for (k, v) in envs {
        argv.extend(["-e".into(), format!("{k}={v}")]);
    }
    argv.push(image.to_string());
    argv.push(bin.display().to_string());
    argv.extend(args.iter().cloned());
    (PathBuf::from("podman"), argv)
}

// ---- tart (macOS / Apple Silicon) ------------------------------------------

/// Apple's Virtualization.framework permits at most 2 concurrently-*running*
/// macOS guests per host; a 3rd boot fails. so the Tart backend serializes
/// past 2 with a process-wide semaphore ([`tart_semaphore`]) the spawn path
/// acquires before `tart clone`/`tart run` and holds for the child's lifetime.
/// the 3rd concurrent run WAITS — it is never an error.
/// ponytail: a fixed process-wide cap; a host serving Linux guests (no such
/// limit) could make the cap image-conditional, but v1 is one backend per node.
pub const TART_MAX_CONCURRENT: usize = 2;

/// the process-wide Tart concurrency gate (see [`TART_MAX_CONCURRENT`]).
pub fn tart_semaphore() -> &'static tokio::sync::Semaphore {
    static SEM: std::sync::OnceLock<tokio::sync::Semaphore> = std::sync::OnceLock::new();
    SEM.get_or_init(|| tokio::sync::Semaphore::new(TART_MAX_CONCURRENT))
}

/// this run's per-run VM name: the workdir's final path component (already
/// unique per run — a portable run's provisioned mount, else the per-pid
/// scratch dir). deterministic, NEVER random, so a crashed run's clone is
/// nameable for cleanup. shared by the run argv and the clone/delete lifecycle.
pub fn tart_vm_name(workdir: &Path) -> String {
    workdir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "ducktape-tart-run".to_string())
}

/// a virtiofs mount TAG derived from a host path: alphanumerics kept, every
/// other byte → `_`, edges trimmed. deterministic (never random) and unique
/// enough for distinct absolute paths in practice.
/// ponytail: two paths differing only in a non-alphanumeric run collide to one
/// tag; a path hash is the upgrade path if that ever bites in the guest.
fn dir_tag(path: &Path) -> String {
    let raw = path.to_string_lossy();
    let mut tag: String = raw
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let trimmed = tag.trim_matches('_');
    if trimmed.len() != tag.len() {
        tag = trimmed.to_string();
    }
    if tag.is_empty() { "root".into() } else { tag }
}

/// translate a provider invocation into a `tart run <vm>` argv — PURE, no I/O,
/// so it unit-tests without tart or a Mac. mirrors [`wrap_podman`]: `vm_name`
/// (the clone the spawn path already made) replaces podman's `image`; the
/// workdir mounts rw, the executor bin ro, each `ro_paths` entry ro, each
/// `rw_dirs` entry rw; `cores`/`mem_gb` become resource flags; the bin+args
/// tail is the in-guest command.
///
/// TART CLI ASSUMPTIONS (verified against cirruslabs/tart docs, 2026-07; the
/// LIVE behavior is real-Mac-QA-deferred — there is no tart on the build box):
///  - mounts use `--dir=<tag>:<hostpath>[:ro]` (verified flag/`:ro` syntax).
///  - `--memory` is MEGABYTES, so `mem_gb * 1024` (verified unit).
///
/// KNOWN DIVERGENCES from real tart, deliberately kept as a SIMPLIFIED
/// SINGLE-SHOT model (this argv would NOT run verbatim on a Mac — that is the
/// deferred wiring, not a silent bug):
///  1. real tart configures cpu/memory via `tart set <vm> --cpu --memory`
///     BETWEEN clone and run, not as `tart run` flags; they ride `tart run`
///     here for one-call parity with podman. ponytail upgrade: split into a
///     `tart set` step in the lifecycle.
///  2. real `tart run <vm>` only BOOTS the guest — it takes no positional
///     in-guest command; the executor runs via `ssh` into the booted VM. the
///     `env K=V… bin args` tail here is exactly what that ssh exec would run.
///     ponytail upgrade: `tart run` (boot) → `tart ip` → `ssh` exec in the
///     lifecycle.
///  3. virtiofs mounts land in-guest at `/Volumes/My Shared Files/<tag>`, NOT
///     at the identical host path podman gives. upstream is path-blind, so the
///     guest needs a boot-time remap (`/Volumes/My Shared Files/<tag>` → the
///     identical host path) for cwd/bin/skill paths to resolve. ponytail
///     upgrade: a guest-agent symlink/bind pass. the host path is the mount
///     SOURCE below so that remap is a rename, not a translation.
// one flat argument per input keeps this PURE and dependency-free — mirrors
// wrap_podman; the `too_many_arguments` bundle would exist only for the lint.
#[allow(clippy::too_many_arguments)]
pub fn wrap_tart(
    vm_name: &str,
    bin: &Path,
    args: &[String],
    workdir: &Path,
    envs: &[(String, String)],
    ro_paths: &[PathBuf],
    rw_dirs: &[PathBuf],
    limits: &BTreeMap<String, u64>,
) -> (PathBuf, Vec<String>) {
    let mut argv: Vec<String> = vec!["run".into(), "--no-graphics".into()];
    if let Some(cores) = limits.get("cores") {
        argv.extend(["--cpu".into(), cores.to_string()]);
    }
    if let Some(mem) = limits.get("mem_gb") {
        // tart --memory is MEGABYTES; mem_gb * 1024 (saturating: no run has an
        // exabyte demand, but overflow would silently wrap).
        argv.extend(["--memory".into(), mem.saturating_mul(1024).to_string()]);
    }
    // mounts: each at its IDENTICAL host path as the SOURCE (tag derived from
    // the path) — workdir rw, bin ro, ro_paths ro, rw_dirs rw.
    argv.push(format!("--dir={}:{}", dir_tag(workdir), workdir.display()));
    argv.push(format!("--dir={}:{}:ro", dir_tag(bin), bin.display()));
    for p in ro_paths {
        argv.push(format!("--dir={}:{}:ro", dir_tag(p), p.display()));
    }
    for d in rw_dirs {
        argv.push(format!("--dir={}:{}", dir_tag(d), d.display()));
    }
    argv.push(vm_name.to_string());
    // the in-guest command (divergence #2): an `env K=V…` prefix carries the
    // run's env exactly as the eventual ssh exec would, then bin + args.
    if !envs.is_empty() {
        argv.push("env".into());
        argv.extend(envs.iter().map(|(k, v)| format!("{k}={v}")));
    }
    argv.push(bin.display().to_string());
    argv.extend(args.iter().cloned());
    (PathBuf::from("tart"), argv)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn podman_wrap_translates_limits_mounts_and_env() {
        let (bin, argv) = wrap_podman(
            "docker.io/library/node:22-slim",
            Path::new("/usr/bin/claude"),
            &["--print".into()],
            Path::new("/tmp/work"),
            &[("FOO".into(), "bar".into())],
            &[PathBuf::from("/opt/skills")],
            &[PathBuf::from("/home/u/.claude")],
            &[("cores".into(), 4u64), ("mem_gb".into(), 8u64)]
                .into_iter()
                .collect(),
        );
        assert_eq!(bin, PathBuf::from("podman"));
        let s = argv.join(" ");
        assert!(s.starts_with("run --rm --network=host"), "got: {s}");
        assert!(
            s.contains("--cpus 4") && s.contains("--memory 8g"),
            "got: {s}"
        );
        assert!(
            s.contains("-v /tmp/work:/tmp/work") && s.contains("-w /tmp/work"),
            "got: {s}"
        );
        assert!(
            s.contains("-v /usr/bin/claude:/usr/bin/claude:ro"),
            "got: {s}"
        );
        assert!(s.contains("-v /opt/skills:/opt/skills:ro"), "got: {s}");
        assert!(
            s.contains("-v /home/u/.claude:/home/u/.claude"),
            "auth state rw, got: {s}"
        );
        assert!(s.contains("-e FOO=bar"), "got: {s}");
        assert!(
            s.ends_with("docker.io/library/node:22-slim /usr/bin/claude --print"),
            "got: {s}"
        );
    }

    #[test]
    fn dimensions_without_a_podman_flag_are_ignored_not_errors() {
        // {"gpu": 1} produces no flag — scheduling already matched it; the
        // backend enforces only what it knows how to enforce.
        let (_bin, argv) = wrap_podman(
            "img",
            Path::new("/bin/x"),
            &[],
            Path::new("/w"),
            &[],
            &[],
            &[],
            &[("gpu".into(), 1u64)].into_iter().collect(),
        );
        let s = argv.join(" ");
        assert!(!s.contains("--cpus") && !s.contains("--memory"), "got: {s}");
        assert!(
            !s.contains("gpu"),
            "an unknown dimension is not a flag: {s}"
        );
    }

    #[test]
    fn always_wraps_even_with_no_limits() {
        // a sandboxed node sandboxes everything: a demandless run still wraps,
        // just without any limit flags.
        let (_bin, argv) = wrap_podman(
            "img",
            Path::new("/bin/x"),
            &[],
            Path::new("/w"),
            &[],
            &[],
            &[],
            &BTreeMap::new(),
        );
        let s = argv.join(" ");
        assert!(s.starts_with("run --rm --network=host"), "got: {s}");
        assert!(s.ends_with("img /bin/x"), "got: {s}");
    }

    // ---- tart backend -------------------------------------------------------

    #[test]
    fn tart_concurrency_cap_is_two() {
        // Apple's Virtualization.framework 2-VM limit — a named const, and the
        // shared gate starts with exactly that many permits.
        assert_eq!(TART_MAX_CONCURRENT, 2);
        assert_eq!(tart_semaphore().available_permits(), TART_MAX_CONCURRENT);
    }

    #[test]
    fn tart_vm_name_is_the_workdir_final_component() {
        // never random: the run's clone is named for its (already unique) workdir.
        assert_eq!(
            tart_vm_name(Path::new("/tmp/ducktape-provider-claude-42")),
            "ducktape-provider-claude-42"
        );
        assert_eq!(tart_vm_name(Path::new("/a/b/c")), "c");
    }

    #[test]
    fn tart_wrap_translates_limits_mounts_and_env() {
        let (bin, argv) = wrap_tart(
            "ducktape-provider-claude-42",
            Path::new("/usr/bin/claude"),
            &["--print".into()],
            Path::new("/tmp/work"),
            &[("FOO".into(), "bar".into())],
            &[PathBuf::from("/opt/skills")],
            &[PathBuf::from("/home/u/.claude")],
            &[("cores".into(), 4u64), ("mem_gb".into(), 8u64)]
                .into_iter()
                .collect(),
        );
        assert_eq!(bin, PathBuf::from("tart"));
        let s = argv.join(" ");
        assert!(s.starts_with("run --no-graphics"), "got: {s}");
        // cpu count as-is; memory is MEGABYTES (mem_gb * 1024).
        assert!(
            s.contains("--cpu 4") && s.contains("--memory 8192"),
            "got: {s}"
        );
        // every path mounts at its IDENTICAL host path as the source: workdir
        // rw (trailing space => no :ro), bin ro, ro_paths ro, rw_dirs rw.
        assert!(s.contains("--dir=tmp_work:/tmp/work "), "workdir rw: {s}");
        assert!(
            s.contains("--dir=usr_bin_claude:/usr/bin/claude:ro"),
            "bin ro: {s}"
        );
        assert!(s.contains("--dir=opt_skills:/opt/skills:ro"), "ro path: {s}");
        assert!(
            s.contains("--dir=home_u__claude:/home/u/.claude "),
            "auth state rw: {s}"
        );
        // the vm name precedes the in-guest command: env prefix, then bin+args.
        assert!(
            s.contains("ducktape-provider-claude-42 env FOO=bar /usr/bin/claude --print"),
            "got: {s}"
        );
        assert!(s.ends_with("/usr/bin/claude --print"), "got: {s}");
    }

    #[test]
    fn tart_dimensions_without_a_flag_are_ignored_not_errors() {
        let (_bin, argv) = wrap_tart(
            "vm",
            Path::new("/bin/x"),
            &[],
            Path::new("/w"),
            &[],
            &[],
            &[],
            &[("gpu".into(), 1u64)].into_iter().collect(),
        );
        let s = argv.join(" ");
        assert!(!s.contains("--cpu") && !s.contains("--memory"), "got: {s}");
        assert!(!s.contains("gpu"), "an unknown dimension is not a flag: {s}");
    }

    #[test]
    fn tart_always_wraps_even_with_no_limits() {
        // a sandboxed node sandboxes everything: a demandless run still wraps,
        // just without any limit flags and with no env prefix.
        let (_bin, argv) = wrap_tart(
            "vm",
            Path::new("/bin/x"),
            &[],
            Path::new("/w"),
            &[],
            &[],
            &[],
            &BTreeMap::new(),
        );
        let s = argv.join(" ");
        assert!(s.starts_with("run --no-graphics"), "got: {s}");
        assert!(s.ends_with("vm /bin/x"), "vm then in-guest bin: {s}");
    }
}
