//! the sandbox backend seam: how a provider child is spawned. every backend
//! is an audited in-tree adapter — a run NEVER executes bare on the host, so
//! the seam has no unsandboxed variant a config could select.
//!
//! This module owns the [`SandboxBackend`] enum + its boot probe, and the Tart
//! (macOS) backend. The Podman backend's execution — building each run's
//! neutral-path `SpecGenerator`, driving create/start/attach/wait over the
//! node-private libpod socket, and the egress firewall — lives in
//! [`crate::podman_api`]; there is no `podman` CLI path any more.
//!
//! Tart (Apple Silicon, Virtualization.framework) is the macOS-guest backend:
//! a run APFS-COW-clones a base image, configures and boots it, executes through
//! SSH, syncs the workspace back, then stops and deletes the clone. The impure
//! lifecycle lives in `CliProvider`; this module builds the mount + guest plan.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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
    /// macOS / Apple Silicon: each run APFS-COW-clones `image`, runs the guest
    /// under a process-wide concurrency cap of 2 ([`TART_MAX_CONCURRENT`]), and
    /// deletes the clone on completion. The `CliProvider` owns its complete
    /// clone/configure/boot/SSH/stop/delete lifecycle.
    Tart {
        image: String,
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
            SandboxBackend::Tart { .. } => "tart",
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


// ---- tart (macOS / Apple Silicon) ------------------------------------------

/// Apple's Virtualization.framework permits at most 2 concurrently-*running*
/// macOS guests per host; a 3rd boot fails. so the Tart backend serializes
/// past 2 with a process-wide semaphore ([`tart_semaphore`]) the spawn path
/// acquires before `tart clone`/`tart run` and holds for the child's lifetime.
/// the 3rd concurrent run WAITS — it is never an error.
/// ponytail: a fixed process-wide cap; a host serving Linux guests (no such
/// limit) could make the cap image-conditional, but v1 is one backend per node.
pub const TART_MAX_CONCURRENT: usize = 2;
pub const TART_MIN_CORES: u64 = 2;

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

/// Where a Tart run's guest tree lives INSIDE the VM, named by the VM.
///
/// Exported because the guest workdir is a real interface, not an internal
/// detail: it is the path the executor's cwd becomes, and therefore the key
/// anything reasoning about "which project is this?" has to use. Derived in one
/// place so a second reader cannot spell it differently.
pub fn tart_run_root(vm: &str) -> PathBuf {
    Path::new("/tmp").join(format!("ducktape-{vm}"))
}

/// This Tart run's guest working directory — the executor's cwd in the VM.
pub fn tart_guest_workdir(vm: &str) -> PathBuf {
    tart_run_root(vm).join("workspace")
}

/// The two commands around a live Tart VM: boot argv and the remote executor
/// script. Resource configuration is deliberately not here: Tart accepts it on
/// `tart set`, which the lifecycle runs between clone and boot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TartPlan {
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
pub fn tart_plan(
    vm: &str,
    bin: &Path,
    args: &[String],
    workdir: &Path,
    envs: &[(String, String)],
    ro_paths: &[PathBuf],
    rw_dirs: &[PathBuf],
    // an interactive (pty) session `exec`s the TUI as the ssh session's
    // foreground process and skips the batch tail (status capture +
    // workspace rsync-back) — a terminal session produces no artifact to sync.
    interactive: bool,
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

    let run_root = tart_run_root(vm);
    let guest_workdir = tart_guest_workdir(vm);
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
    if interactive {
        // the TUI replaces the shell, so the pty carries it directly and its
        // exit is the ssh session's. no status capture, no rsync-back.
        setup.push(format!("exec {command}"));
    } else {
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
    }

    Ok(TartPlan {
        vm: vm.to_string(),
        run_argv,
        guest_script: setup.join("; "),
    })
}

pub fn tart_ssh_argv(ip: &str, guest_script: &str, tty: bool) -> Vec<String> {
    // -T for a headless run (no pty); -tt forces a remote pty for an interactive
    // session, so the guest TUI sees a terminal and SIGWINCH/size relay works.
    let pty_flag = if tty { "-tt" } else { "-T" };
    vec![
        "-p".into(),
        "admin".into(),
        "ssh".into(),
        pty_flag.into(),
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

pub fn tart_set_argv(
    vm: &str,
    limits: &BTreeMap<String, u64>,
) -> Result<Option<Vec<String>>, String> {
    let mut argv = vec!["set".to_string(), vm.to_string()];
    if let Some(cores) = limits.get("cores") {
        if *cores < TART_MIN_CORES {
            return Err(format!(
                "Tart requires at least {TART_MIN_CORES} cores, got {cores}"
            ));
        }
        argv.extend(["--cpu".into(), cores.to_string()]);
    }
    if let Some(mem_gb) = limits.get("mem_gb") {
        argv.extend(["--memory".into(), mem_gb.saturating_mul(1024).to_string()]);
    }
    Ok((argv.len() > 2).then_some(argv))
}

#[cfg(test)]
mod tests {
    use super::*;

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
            false,
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

        let ssh = tart_ssh_argv("192.0.2.10", script, false).join(" ");
        assert!(ssh.starts_with("-p admin ssh -T"), "{ssh}");
        assert!(ssh.contains("admin@192.0.2.10 -- /bin/sh -lc"), "{ssh}");
        // an interactive session forces a remote pty with -tt instead of -T.
        let ssh_tty = tart_ssh_argv("192.0.2.10", script, true).join(" ");
        assert!(ssh_tty.starts_with("-p admin ssh -tt"), "{ssh_tty}");
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
            false,
        )
        .unwrap_err();
        assert!(err.contains("containing ':'"), "{err}");
    }

    #[test]
    fn tart_resources_are_configured_with_set_not_run_flags() {
        let limits = BTreeMap::from([("cores".into(), 4), ("mem_gb".into(), 8), ("gpu".into(), 1)]);
        assert_eq!(
            tart_set_argv("vm", &limits).unwrap().unwrap().join(" "),
            "set vm --cpu 4 --memory 8192"
        );
        assert!(
            tart_set_argv("vm", &BTreeMap::from([("cores".into(), 1)]))
                .unwrap_err()
                .contains("at least 2 cores")
        );
        assert_eq!(tart_set_argv("vm", &BTreeMap::new()).unwrap(), None);
    }
}
