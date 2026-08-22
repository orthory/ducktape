//! PID 1 inside a run's microVM.
//!
//! The seat `conmon` occupied under podman, and deliberately smaller: mount the
//! pseudo-filesystems and whatever block devices the host attached, read the
//! run manifest off the kernel command line, exec the agent CLI, carry its
//! stdout and stderr back over vsock as separate streams, report the exit code,
//! then halt the VM.
//!
//! Two things about being PID 1 shape the whole file:
//!
//! - **Returning from `main` panics the kernel.** Every path ends at
//!   [`halt`], including every failure path. A guest that dies without halting
//!   hangs its run until the host's idle timeout, holding all of its memory.
//! - **`RESTART`, never `POWER_OFF`.** Firecracker exposes no ACPI power
//!   button, so `LINUX_REBOOT_CMD_POWER_OFF` parks the guest at
//!   `reboot: System halted` and the VMM never exits. Measured: one guest hung
//!   past 120 s, an otherwise identical one exited in 428 ms. `RESTART` goes
//!   through the `reboot=k` i8042 reset, which the VMM does observe.

mod manifest;

// the vsock wire, shared with the host by including its source directly rather
// than depending on sandbox-host — PID 1 must not carry tokio and tracing. See
// that file's module docs.
// Each end uses half the codec — the guest only encodes, the host only decodes
// — so the unused half is dead code here by construction, not by oversight.
#[allow(dead_code)]
#[path = "../../../crates/services/sandbox/src/guest_proto.rs"]
mod guest_proto;

use guest_proto::{Frame, encode};
use manifest::RunManifest;
use std::ffi::CString;
use std::io::Write as _;
use std::os::fd::RawFd;

/// exit code reported when the init itself failed, as opposed to the run. Well
/// clear of the 0-125 a program picks and of the 128+N a signal produces.
const INIT_FAILED: i32 = 126;

fn main() {
    // The console is the only diagnostic channel before vsock is up, and this
    // is a guest program's own output rather than node logging — the tracing
    // rule in CLAUDE.md governs the node, which has a subscriber and a log
    // ring. This has neither.
    let code = match run() {
        Ok(code) => code,
        Err(e) => {
            let _ = writeln!(std::io::stderr(), "duck-guest-init: {e}");
            INIT_FAILED
        }
    };
    let _ = writeln!(std::io::stderr(), "duck-guest-init: exit {code}");
    halt();
}

fn run() -> Result<i32, String> {
    mount_pseudo_filesystems()?;

    let cmdline = std::fs::read_to_string("/proc/cmdline")
        .map_err(|e| format!("read /proc/cmdline: {e}"))?;
    let manifest = manifest::from_cmdline(&cmdline)?;

    for (device, mountpoint) in &manifest.mounts {
        mount_ext4(device, mountpoint)?;
    }

    let host = connect_to_host()?;
    let code = exec_and_pump(&manifest, host);

    // Flush the workspace image before the VM dies. `sync` is not optional: the
    // host reads the image back with debugfs immediately after the VMM exits,
    // and an unflushed page cache means the run's output is simply missing.
    unsafe { libc::sync() };
    for (_, mountpoint) in manifest.mounts.iter().rev() {
        unmount(mountpoint);
    }
    code
}

// ---------------------------------------------------------------------------
// mounts
// ---------------------------------------------------------------------------

fn mount_pseudo_filesystems() -> Result<(), String> {
    // devtmpfs first and non-negotiable: without it there are no /dev/vd*
    // nodes, so every block device in the manifest is unmountable.
    for (source, target, fstype) in [
        ("devtmpfs", "/dev", "devtmpfs"),
        ("proc", "/proc", "proc"),
        ("sysfs", "/sys", "sysfs"),
    ] {
        std::fs::create_dir_all(target).map_err(|e| format!("create {target}: {e}"))?;
        mount(source, target, fstype, 0, None)?;
    }
    Ok(())
}

fn mount_ext4(device: &str, mountpoint: &str) -> Result<(), String> {
    std::fs::create_dir_all(mountpoint).map_err(|e| format!("create {mountpoint}: {e}"))?;
    mount(device, mountpoint, "ext4", 0, None)
}

fn mount(
    source: &str,
    target: &str,
    fstype: &str,
    flags: libc::c_ulong,
    data: Option<&str>,
) -> Result<(), String> {
    let c_source = cstring(source)?;
    let c_target = cstring(target)?;
    let c_fstype = cstring(fstype)?;
    let c_data = data.map(cstring).transpose()?;
    let data_ptr = c_data
        .as_ref()
        .map_or(std::ptr::null(), |d| d.as_ptr().cast());
    let rc = unsafe {
        libc::mount(
            c_source.as_ptr(),
            c_target.as_ptr(),
            c_fstype.as_ptr(),
            flags,
            data_ptr,
        )
    };
    if rc != 0 {
        return Err(format!(
            "mount {source} -> {target} ({fstype}): {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

/// unmount best-effort: `sync` above already flushed, so a busy mountpoint at
/// teardown costs nothing and must not turn a finished run into a failed one.
fn unmount(mountpoint: &str) {
    let Ok(c_target) = CString::new(mountpoint) else {
        return;
    };
    unsafe { libc::umount(c_target.as_ptr()) };
}

// ---------------------------------------------------------------------------
// the host connection
// ---------------------------------------------------------------------------

/// dial the host over vsock. Guest-initiated is the simpler direction: the host
/// listens on `<uds>_1024` and Firecracker bridges this connection to it, so no
/// in-guest listener and no connect-back race.
fn connect_to_host() -> Result<RawFd, String> {
    let fd = unsafe { libc::socket(libc::AF_VSOCK, libc::SOCK_STREAM, 0) };
    if fd < 0 {
        return Err(format!(
            "vsock socket: {}",
            std::io::Error::last_os_error()
        ));
    }
    let mut addr: libc::sockaddr_vm = unsafe { std::mem::zeroed() };
    addr.svm_family = libc::AF_VSOCK as libc::sa_family_t;
    addr.svm_cid = libc::VMADDR_CID_HOST;
    addr.svm_port = guest_proto::VSOCK_PORT;
    let rc = unsafe {
        libc::connect(
            fd,
            std::ptr::addr_of!(addr).cast(),
            std::mem::size_of::<libc::sockaddr_vm>() as libc::socklen_t,
        )
    };
    if rc != 0 {
        let err = std::io::Error::last_os_error();
        unsafe { libc::close(fd) };
        return Err(format!("vsock connect to host: {err}"));
    }
    Ok(fd)
}

fn send(fd: RawFd, frame: &Frame) {
    let bytes = encode(frame);
    let mut written = 0usize;
    while written < bytes.len() {
        let n = unsafe {
            libc::write(
                fd,
                bytes[written..].as_ptr().cast(),
                bytes.len() - written,
            )
        };
        if n <= 0 {
            // The host hung up. Nothing to report it to, and the run's exit
            // still has to reach `halt`.
            return;
        }
        written += n as usize;
    }
}

// ---------------------------------------------------------------------------
// the run
// ---------------------------------------------------------------------------

fn exec_and_pump(manifest: &RunManifest, host: RawFd) -> Result<i32, String> {
    let mut out_pipe = [0 as RawFd; 2];
    let mut err_pipe = [0 as RawFd; 2];
    for pipe in [&mut out_pipe, &mut err_pipe] {
        if unsafe { libc::pipe(pipe.as_mut_ptr()) } != 0 {
            return Err(format!("pipe: {}", std::io::Error::last_os_error()));
        }
    }

    let child = unsafe { libc::fork() };
    if child < 0 {
        return Err(format!("fork: {}", std::io::Error::last_os_error()));
    }
    if child == 0 {
        // ---- child: never returns ----
        unsafe {
            libc::close(out_pipe[0]);
            libc::close(err_pipe[0]);
            libc::dup2(out_pipe[1], libc::STDOUT_FILENO);
            libc::dup2(err_pipe[1], libc::STDERR_FILENO);
            libc::close(out_pipe[1]);
            libc::close(err_pipe[1]);
        }
        exec_child(manifest);
    }

    unsafe {
        libc::close(out_pipe[1]);
        libc::close(err_pipe[1]);
    }
    pump(host, out_pipe[0], err_pipe[0]);
    let code = wait_for(child);
    send(host, &Frame::Exit(code));
    Ok(code)
}

/// the child half of the fork. Diverges: on a successful `execve` this process
/// becomes the CLI, and on a failed one it must `_exit` rather than return into
/// a second copy of the init.
fn exec_child(manifest: &RunManifest) -> ! {
    let fail = |msg: &str| -> ! {
        let _ = writeln!(std::io::stderr(), "duck-guest-init: {msg}");
        unsafe { libc::_exit(INIT_FAILED) }
    };

    // stdin is /dev/null: a microVM run is headless, and a CLI that blocks on a
    // read from a terminal that will never exist hangs the run.
    let Ok(devnull) = CString::new("/dev/null") else {
        fail("/dev/null path")
    };
    let null_fd = unsafe { libc::open(devnull.as_ptr(), libc::O_RDONLY) };
    if null_fd >= 0 {
        unsafe {
            libc::dup2(null_fd, libc::STDIN_FILENO);
            libc::close(null_fd);
        }
    }

    if std::env::set_current_dir(&manifest.cwd).is_err() {
        fail(&format!("chdir {}", manifest.cwd));
    }

    let Ok(argv): Result<Vec<CString>, _> = manifest
        .argv
        .iter()
        .map(|a| CString::new(a.as_bytes()))
        .collect()
    else {
        fail("argv holds an interior NUL")
    };
    let Ok(envp): Result<Vec<CString>, _> = manifest
        .env
        .iter()
        .map(|(k, v)| CString::new(format!("{k}={v}")))
        .collect()
    else {
        fail("env holds an interior NUL")
    };

    let mut argv_ptrs: Vec<*const libc::c_char> = argv.iter().map(|a| a.as_ptr()).collect();
    argv_ptrs.push(std::ptr::null());
    let mut envp_ptrs: Vec<*const libc::c_char> = envp.iter().map(|e| e.as_ptr()).collect();
    envp_ptrs.push(std::ptr::null());

    unsafe { libc::execve(argv[0].as_ptr(), argv_ptrs.as_ptr(), envp_ptrs.as_ptr()) };
    fail(&format!("execve {}", manifest.argv[0]))
}

/// forward both pipes to the host until each reaches EOF, tagging every read
/// with the stream it came from. `poll` rather than two threads: PID 1 with two
/// file descriptors does not need a runtime.
fn pump(host: RawFd, out_fd: RawFd, err_fd: RawFd) {
    let mut fds = [
        libc::pollfd {
            fd: out_fd,
            events: libc::POLLIN,
            revents: 0,
        },
        libc::pollfd {
            fd: err_fd,
            events: libc::POLLIN,
            revents: 0,
        },
    ];
    let mut buf = vec![0u8; 64 * 1024];
    let mut open = 2;
    while open > 0 {
        // -1: block. The child's own output is the only clock here — a timeout
        // would be a guess about how long a run may be quiet, and runs are
        // quiet for minutes at a time.
        let rc = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, -1) };
        if rc < 0 {
            let interrupted = std::io::Error::last_os_error().kind()
                == std::io::ErrorKind::Interrupted;
            if interrupted {
                continue;
            }
            return;
        }
        for (slot, pollfd) in fds.iter_mut().enumerate() {
            let retired = pollfd.fd < 0;
            let ready = pollfd.revents & (libc::POLLIN | libc::POLLHUP) != 0;
            if retired || !ready {
                continue;
            }
            let n = unsafe { libc::read(pollfd.fd, buf.as_mut_ptr().cast(), buf.len()) };
            if n > 0 {
                let payload = buf[..n as usize].to_vec();
                let frame = match slot {
                    0 => Frame::Stdout(payload),
                    _ => Frame::Stderr(payload),
                };
                send(host, &frame);
                continue;
            }
            // 0 = EOF, <0 = a broken pipe. Either way this stream is done;
            // retire it so `poll` stops reporting it ready forever.
            unsafe { libc::close(pollfd.fd) };
            pollfd.fd = -1;
            open -= 1;
        }
    }
}

/// the child's exit status as a single number, matching shell convention:
/// 128 + N for a signal death, so a killed run is distinguishable from one that
/// returned the same small number.
fn wait_for(child: libc::pid_t) -> i32 {
    let mut status: libc::c_int = 0;
    loop {
        let rc = unsafe { libc::waitpid(child, &mut status, 0) };
        if rc < 0 {
            let interrupted =
                std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted;
            if interrupted {
                continue;
            }
            return INIT_FAILED;
        }
        if libc::WIFEXITED(status) {
            return libc::WEXITSTATUS(status);
        }
        if libc::WIFSIGNALED(status) {
            return 128 + libc::WTERMSIG(status);
        }
    }
}

// ---------------------------------------------------------------------------
// halt
// ---------------------------------------------------------------------------

/// stop the VM. Diverges — PID 1 has nowhere to return to.
fn halt() -> ! {
    // RESTART, not POWER_OFF: see the module docs. `reboot=k` on the kernel
    // command line turns this into the i8042 reset the VMM watches for.
    unsafe { libc::reboot(libc::RB_AUTOBOOT) };
    // Only reachable if reboot(2) itself failed, which means this is not PID 1
    // or the kernel refused. Spin rather than return into a kernel panic.
    loop {
        unsafe { libc::pause() };
    }
}

fn cstring(s: &str) -> Result<CString, String> {
    CString::new(s).map_err(|_| format!("{s:?} holds an interior NUL"))
}
