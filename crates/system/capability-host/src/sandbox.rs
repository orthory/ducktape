//! the sandbox backend seam: how a provider child is spawned. Direct is the
//! historical spawn; Podman wraps the identical argv in a rootless container
//! that enforces the run's numeric limits. paths are mounted at identical
//! container paths so workdir/session/skill logic upstream stays path-blind.
//! HOME is NOT mounted: only the spec's [sandbox] rw_dirs (CLI auth/state)
//! cross the boundary, so the node's data dir and user key stay outside —
//! the D7 enforcement mechanism the provider doc deferred.
//! ponytail: --network=host keeps loopback MCP reachable; a private netns
//! with a gateway route is the upgrade path if network isolation matters.

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
}
