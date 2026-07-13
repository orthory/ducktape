//! the sandbox backend seam: how a provider child is spawned. Direct is the
//! historical spawn; Podman wraps the identical argv in a rootless container
//! that enforces the run's numeric limits. paths are mounted at identical
//! container paths so workdir/session/skill logic upstream stays path-blind.
//! HOME is NOT mounted: only the spec's [sandbox] rw_dirs (CLI auth/state)
//! cross the boundary, so the node's data dir and user key stay outside —
//! this is the D7 filesystem-isolation boundary for sandboxed providers.
//! ponytail: --network=host keeps loopback MCP reachable; a private netns
//! with a gateway route is the upgrade path if network isolation matters.
//!
//! Tart (Apple Silicon, Virtualization.framework) is the macOS-guest backend:
//! a run APFS-COW-clones a base image, configures and boots it, executes through
//! SSH, syncs the workspace back, then stops and deletes the clone. The impure
//! lifecycle lives in `CliProvider`; this module builds the mount + guest plan.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// how a provider child is spawned. `Direct` is the plain host spawn (the
/// historical behavior); `Podman` wraps each run in a rootless container. a
/// node whose provider set uses `Podman`
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
    /// deletes the clone on completion. The `CliProvider` owns its complete
    /// clone/configure/boot/SSH/stop/delete lifecycle.
    Tart {
        image: String,
    },
}

/// translate a provider invocation into a `podman run` argv — PURE, no I/O, so
/// it unit-tests without podman installed. mounts every path at its IDENTICAL
/// container path (nothing upstream translates paths): the workdir rw, the
/// executor bin ro, each `ro_paths` entry ro (the run's PATH-entry dirs and its
/// W6 skills tree — see `CliProvider::sandbox_ro_paths`), each `rw_dirs`
/// entry rw (the spec's CLI auth/state dirs). only the limit dimensions this
/// backend knows how to enforce (`cores` → `--cpus`, `mem_gb` → `--memory` and
/// `--memory-swap` with the same value) become flags; an unknown dimension
/// (e.g. `gpu`) is silently ignored — the scheduler already matched it, the
/// backend enforces only what it can.
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
        let mem = format!("{mem}g");
        argv.extend(["--memory".into(), mem.clone()]);
        argv.extend(["--memory-swap".into(), mem]);
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

const TART_SHARED_ROOT: &str = "/Volumes/My Shared Files";

#[derive(Debug, Clone, PartialEq, Eq)]
struct TartMount {
    host: PathBuf,
    guest: PathBuf,
    read_only: bool,
}

/// The two commands around a live Tart VM: boot argv and the remote executor
/// script. Resource configuration is deliberately not here: Tart accepts it on
/// `tart set`, which the lifecycle runs between clone and boot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TartPlan {
    pub vm: String,
    pub run_argv: Vec<String>,
    pub guest_script: String,
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn mount_source(path: &Path) -> PathBuf {
    if path.is_file() {
        path.parent()
            .unwrap_or_else(|| Path::new("/"))
            .to_path_buf()
    } else {
        path.to_path_buf()
    }
}

fn add_mount(mounts: &mut Vec<TartMount>, host: PathBuf, read_only: bool) {
    if let Some(existing) = mounts.iter_mut().find(|mount| mount.host == host) {
        // The workdir/auth state must stay writable even if the same path also
        // arrives through the read-only list.
        existing.read_only &= read_only;
        return;
    }
    let tag = format!("dt{}", mounts.len());
    mounts.push(TartMount {
        host,
        guest: Path::new(TART_SHARED_ROOT).join(tag),
        read_only,
    });
}

fn translate_path(path: &Path, mounts: &[TartMount]) -> PathBuf {
    mounts
        .iter()
        .filter_map(|mount| {
            path.strip_prefix(&mount.host)
                .ok()
                .map(|suffix| (&mount.host, mount.guest.join(suffix)))
        })
        .max_by_key(|(host, _)| host.components().count())
        .map(|(_, guest)| guest)
        .unwrap_or_else(|| path.to_path_buf())
}

fn translate_command_path(
    path: &Path,
    mounts: &[TartMount],
    workdir: &Path,
    guest_workdir: &Path,
) -> PathBuf {
    if let Ok(suffix) = path.strip_prefix(workdir) {
        guest_workdir.join(suffix)
    } else {
        translate_path(path, mounts)
    }
}

fn translate_value(
    value: &str,
    mounts: &[TartMount],
    workdir: &Path,
    guest_workdir: &Path,
) -> String {
    let path = Path::new(value);
    if path.is_absolute() {
        return translate_command_path(path, mounts, workdir, guest_workdir)
            .display()
            .to_string();
    }
    // Broker configuration embeds the project path inside a larger TOML
    // argument, so whole-value path parsing is insufficient. Replace longest
    // host prefixes first to keep nested mounts deterministic.
    let host_workdir = workdir.to_string_lossy();
    let local_workdir = guest_workdir.to_string_lossy();
    let mut translated = value.replace(host_workdir.as_ref(), local_workdir.as_ref());
    let mut ordered = mounts.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|mount| std::cmp::Reverse(mount.host.components().count()));
    for mount in ordered {
        let host = mount.host.to_string_lossy();
        let guest = mount.guest.to_string_lossy();
        translated = translated.replace(host.as_ref(), guest.as_ref());
    }
    translated
}

/// Build the real Tart boot + SSH execution shape. Tart exposes host folders
/// below `/Volumes/My Shared Files`; the guest command therefore translates
/// every mounted path. The workspace is copied to the guest disk before the
/// executor starts and rsynced back only after it exits, so `../AGENTS.md`
/// context keeps its parent relationship without exposing the workspace's host
/// siblings or committing the temporary context file.
#[allow(clippy::too_many_arguments)]
pub(crate) fn tart_plan(
    vm: &str,
    bin: &Path,
    args: &[String],
    workdir: &Path,
    envs: &[(String, String)],
    ro_paths: &[PathBuf],
    rw_dirs: &[PathBuf],
) -> Result<TartPlan, String> {
    let bin_dir = bin
        .parent()
        .ok_or_else(|| format!("Tart executor {} has no parent directory", bin.display()))?;
    let mut mounts = Vec::new();
    add_mount(&mut mounts, workdir.to_path_buf(), false);
    add_mount(&mut mounts, bin_dir.to_path_buf(), true);
    for path in ro_paths {
        add_mount(&mut mounts, mount_source(path), true);
    }
    for path in rw_dirs {
        add_mount(&mut mounts, path.to_path_buf(), false);
    }
    for mount in &mounts {
        if mount.host.to_string_lossy().contains(':') {
            return Err(format!(
                "Tart cannot mount a host path containing ':': {}",
                mount.host.display()
            ));
        }
    }

    let mut run_argv = vec!["run".into(), "--no-graphics".into()];
    for mount in &mounts {
        let tag = mount.guest.file_name().unwrap().to_string_lossy();
        run_argv.push(format!(
            "--dir={tag}:{}{}",
            mount.host.display(),
            if mount.read_only { ":ro" } else { "" }
        ));
    }
    run_argv.push(vm.to_string());

    let run_root = Path::new("/tmp").join(format!("ducktape-{vm}"));
    let guest_workdir = run_root.join("workspace");
    let mounted_workdir = translate_path(workdir, &mounts);
    let guest_home = run_root.join("home");
    let host_home = envs
        .iter()
        .find(|(key, _)| key == "HOME")
        .map(|(_, value)| PathBuf::from(value));

    let mut setup = vec![
        "set -eu".to_string(),
        format!("rm -rf {}", shell_quote(&run_root.display().to_string())),
        format!(
            "mkdir -p {} {}",
            shell_quote(&guest_workdir.display().to_string()),
            shell_quote(&guest_home.display().to_string())
        ),
        format!(
            "cp -R {}/. {}",
            shell_quote(&mounted_workdir.display().to_string()),
            shell_quote(&guest_workdir.display().to_string())
        ),
    ];
    let needs_host_gateway = envs
        .iter()
        .any(|(_, value)| value.contains("ducktape-host"))
        || args.iter().any(|arg| arg.contains("ducktape-host"));
    if needs_host_gateway {
        setup.push(
            "gateway=$(/sbin/route -n get default | /usr/bin/awk '/gateway:/{print $2; exit}')"
                .into(),
        );
        setup.push("test -n \"$gateway\"".into());
        setup.push(
            "printf '%s\\tducktape-host\\n' \"$gateway\" | sudo -n tee -a /etc/hosts >/dev/null"
                .into(),
        );
    }

    // A workspace-parent context document must remain one level above cwd.
    // Other read-only paths are directories (PATH bindings / skills).
    for path in ro_paths.iter().filter(|path| path.is_file()) {
        let source = translate_path(path, &mounts);
        let target = run_root.join(path.file_name().unwrap());
        setup.push(format!(
            "cp {} {}",
            shell_quote(&source.display().to_string()),
            shell_quote(&target.display().to_string())
        ));
    }

    if let Some(home) = host_home.as_deref() {
        for dir in rw_dirs {
            let relative = dir.strip_prefix(home).map_err(|_| {
                format!(
                    "Tart auth directory {} is outside HOME {}",
                    dir.display(),
                    home.display()
                )
            })?;
            let target = guest_home.join(relative);
            if let Some(parent) = target.parent() {
                setup.push(format!(
                    "mkdir -p {}",
                    shell_quote(&parent.display().to_string())
                ));
            }
            setup.push(format!(
                "ln -s {} {}",
                shell_quote(&translate_path(dir, &mounts).display().to_string()),
                shell_quote(&target.display().to_string())
            ));
        }
    }

    let mut command = vec!["env".to_string()];
    for (key, value) in envs {
        let value = if key == "HOME" {
            guest_home.display().to_string()
        } else if key == "PATH" {
            std::env::split_paths(value)
                .map(|path| translate_command_path(&path, &mounts, workdir, &guest_workdir))
                .collect::<Vec<_>>()
                .iter()
                .map(|path| path.to_string_lossy())
                .collect::<Vec<_>>()
                .join(":")
        } else {
            translate_value(value, &mounts, workdir, &guest_workdir)
        };
        command.push(format!("{key}={value}"));
    }
    command.push(translate_path(bin, &mounts).display().to_string());
    command.extend(
        args.iter()
            .map(|arg| translate_value(arg, &mounts, workdir, &guest_workdir)),
    );
    let command = command
        .iter()
        .map(|arg| shell_quote(arg))
        .collect::<Vec<_>>()
        .join(" ");
    setup.push(format!(
        "cd {}",
        shell_quote(&guest_workdir.display().to_string())
    ));
    setup.push("set +e".into());
    setup.push(command);
    setup.push("status=$?".into());
    // Tart virtiofs rejects mtime updates on the shared root (-O).
    setup.push(format!(
        "rsync -aO --delete {}/ {}/ >/dev/null 2>&1 || sync_status=$?",
        shell_quote(&guest_workdir.display().to_string()),
        shell_quote(&mounted_workdir.display().to_string())
    ));
    setup.push("if [ \"$status\" -ne 0 ]; then exit \"$status\"; fi".into());
    setup.push("exit ${sync_status:-0}".into());

    Ok(TartPlan {
        vm: vm.to_string(),
        run_argv,
        guest_script: setup.join("; "),
    })
}

pub(crate) fn tart_ssh_argv(ip: &str, guest_script: &str) -> Vec<String> {
    vec![
        "-p".into(),
        "admin".into(),
        "ssh".into(),
        "-T".into(),
        "-o".into(),
        "StrictHostKeyChecking=no".into(),
        "-o".into(),
        "UserKnownHostsFile=/dev/null".into(),
        "-o".into(),
        "ConnectTimeout=5".into(),
        format!("admin@{ip}"),
        "--".into(),
        "/bin/sh".into(),
        "-lc".into(),
        shell_quote(guest_script),
    ]
}

pub(crate) fn tart_set_argv(vm: &str, limits: &BTreeMap<String, u64>) -> Option<Vec<String>> {
    let mut argv = vec!["set".to_string(), vm.to_string()];
    if let Some(cores) = limits.get("cores") {
        argv.extend(["--cpu".into(), cores.to_string()]);
    }
    if let Some(mem_gb) = limits.get("mem_gb") {
        argv.extend(["--memory".into(), mem_gb.saturating_mul(1024).to_string()]);
    }
    (argv.len() > 2).then_some(argv)
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
            s.contains("--cpus 4") && s.contains("--memory 8g") && s.contains("--memory-swap 8g"),
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
        assert!(
            !s.contains("--cpus") && !s.contains("--memory") && !s.contains("--memory-swap"),
            "got: {s}"
        );
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
        assert!(
            !s.contains("--memory") && !s.contains("--memory-swap"),
            "got: {s}"
        );
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
    fn tart_plan_uses_real_boot_mount_and_ssh_shapes() {
        let root = std::env::temp_dir().join(format!("dt-tart-plan-{}", std::process::id()));
        let workdir = root.join("work");
        let bin_dir = root.join("bin");
        let skills = root.join("skills");
        let auth = root.join("home/.claude");
        for dir in [&workdir, &bin_dir, &skills, &auth] {
            std::fs::create_dir_all(dir).unwrap();
        }
        let bin = bin_dir.join("claude");
        std::fs::write(&bin, b"bin").unwrap();

        let plan = tart_plan(
            "dt-42-7",
            &bin,
            &["--print".into(), format!("project={}", workdir.display())],
            &workdir,
            &[
                ("HOME".into(), root.join("home").display().to_string()),
                ("DUCKTAPE_RUN_SKILLS".into(), skills.display().to_string()),
                ("BROKER".into(), "http://ducktape-host:4321/v1".into()),
            ],
            std::slice::from_ref(&skills),
            std::slice::from_ref(&auth),
        )
        .unwrap();
        let run = plan.run_argv.join(" ");
        assert!(run.starts_with("run --no-graphics"), "{run}");
        assert!(run.ends_with("dt-42-7"), "{run}");
        assert!(
            run.contains(&format!("--dir=dt0:{}", workdir.display())),
            "{run}"
        );
        assert!(
            run.contains(&format!("--dir=dt1:{}:ro", bin_dir.display())),
            "{run}"
        );
        assert!(!run.contains("--cpu") && !run.contains("--memory"), "{run}");

        let script = &plan.guest_script;
        assert!(script.contains("cp -R"), "{script}");
        assert!(script.contains("rsync -aO --delete"), "{script}");
        assert!(
            script.contains("/Volumes/My Shared Files/dt1/claude"),
            "{script}"
        );
        assert!(
            script.contains("DUCKTAPE_RUN_SKILLS=/Volumes/My Shared Files/dt2"),
            "{script}"
        );
        assert!(script.contains("ln -s"), "{script}");
        assert!(script.contains("route -n get default"), "{script}");
        assert!(script.contains("ducktape-host"), "{script}");
        assert!(script.contains("--print"), "{script}");
        assert!(
            script.contains("project=/tmp/ducktape-dt-42-7/workspace"),
            "{script}"
        );

        let ssh = tart_ssh_argv("192.0.2.10", script).join(" ");
        assert!(ssh.starts_with("-p admin ssh -T"), "{ssh}");
        assert!(ssh.contains("admin@192.0.2.10 -- /bin/sh -lc"), "{ssh}");
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn tart_plan_rejects_unrepresentable_mount_paths() {
        let err = tart_plan(
            "vm",
            Path::new("/bin/x"),
            &[],
            Path::new("/tmp/a:b"),
            &[("HOME".into(), "/home/u".into())],
            &[],
            &[],
        )
        .unwrap_err();
        assert!(err.contains("containing ':'"), "{err}");
    }

    #[test]
    fn tart_resources_are_configured_with_set_not_run_flags() {
        let limits = BTreeMap::from([("cores".into(), 4), ("mem_gb".into(), 8), ("gpu".into(), 1)]);
        assert_eq!(
            tart_set_argv("vm", &limits).unwrap().join(" "),
            "set vm --cpu 4 --memory 8192"
        );
        assert_eq!(tart_set_argv("vm", &BTreeMap::new()), None);
    }
}
