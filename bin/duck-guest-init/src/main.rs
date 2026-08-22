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

// Both halves of the host<->guest contract live in sandbox-host and are
// included here verbatim rather than depended on. The host has to ENCODE the
// manifest this parses and DECODE the frames this writes, so a second
// definition on either side is a wire-format fork waiting to happen — while
// PID 1 still must not carry tokio and tracing.
//
// Each end uses only part of each module (the guest parses manifests and
// encodes frames; the host does the reverse), so the unused part is dead code
// here by construction, not by oversight.
#[allow(dead_code)]
#[path = "../../../crates/services/sandbox/src/guest_manifest.rs"]
mod manifest;

#[allow(dead_code)]
#[path = "../../../crates/services/sandbox/src/guest_proto.rs"]
mod guest_proto;

use guest_proto::{Frame, encode};
use manifest::RunManifest;
use std::ffi::CString;
use std::io::Write as _;
use std::os::fd::{AsRawFd as _, RawFd};

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

    for (index, port) in manifest.tunnel_ports.iter().enumerate() {
        // best-effort: a run whose tunnel fails to bind will fail its API calls
        // with a clear connection error from the CLI, which is a better
        // diagnosis than refusing to boot with none of the run's own output.
        if let Err(e) = start_tunnel(*port, index) {
            let _ = writeln!(std::io::stderr(), "duck-guest-init: tunnel {port}: {e}");
        }
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
// the broker tunnel
// ---------------------------------------------------------------------------

/// serve `127.0.0.1:port` inside the guest and forward every connection to the
/// host over vsock.
///
/// The run's CLI dials its credential broker as ordinary HTTP. With no network
/// device there is no route out of the VM, so this is the route — and it is a
/// better one: the far end is a socket the host process owns, not an interface,
/// so the guest cannot reach anything else on the host by changing an address.
/// The credential itself never enters the VM; the broker holds it and the guest
/// carries only an opaque per-run bearer.
fn start_tunnel(port: u16, index: usize) -> Result<(), String> {
    bring_up_loopback()?;
    let listener = std::net::TcpListener::bind(("127.0.0.1", port))
        .map_err(|e| format!("bind 127.0.0.1:{port}: {e}"))?;
    // the host bound its listeners in this order; the guest never names a
    // destination, it only picks the matching vsock port.
    let vsock_port = guest_proto::TUNNEL_PORT_BASE + index as u32;
    std::thread::Builder::new()
        .name(format!("tunnel-{port}"))
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                // one vsock connection per TCP connection: HTTP keep-alive and
                // concurrent requests both work, and a stuck request cannot
                // block another.
                std::thread::spawn(move || tunnel_one(stream, vsock_port));
            }
        })
        .map_err(|e| format!("spawn tunnel thread: {e}"))?;
    Ok(())
}

fn tunnel_one(tcp: std::net::TcpStream, vsock_port: u32) {
    let Ok(vsock) = connect_vsock(vsock_port) else {
        return;
    };
    let Ok(tcp_write) = tcp.try_clone() else {
        unsafe { libc::close(vsock) };
        return;
    };
    let tcp_fd = tcp.as_raw_fd();
    // two directions, two threads. Splicing both in one thread would deadlock
    // the moment either side filled its buffer.
    let up = std::thread::spawn(move || {
        splice(tcp_fd, vsock);
        // half-close so the far end sees EOF rather than waiting forever
        unsafe { libc::shutdown(vsock, libc::SHUT_WR) };
    });
    splice(vsock, tcp_write.as_raw_fd());
    let _ = tcp_write.shutdown(std::net::Shutdown::Write);
    let _ = up.join();
    unsafe { libc::close(vsock) };
}

/// copy `from` to `to` until either end closes.
fn splice(from: RawFd, to: RawFd) {
    let mut buf = vec![0u8; 32 * 1024];
    loop {
        let n = unsafe { libc::read(from, buf.as_mut_ptr().cast(), buf.len()) };
        if n <= 0 {
            return;
        }
        write_all(to, &buf[..n as usize]);
    }
}

/// bring `lo` up. Without it, binding 127.0.0.1 succeeds and connecting to it
/// fails with `ENETUNREACH` — a failure that reads like the broker is down.
fn bring_up_loopback() -> Result<(), String> {
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
    if fd < 0 {
        return Err(format!("socket: {}", std::io::Error::last_os_error()));
    }
    let mut req: libc::ifreq = unsafe { std::mem::zeroed() };
    for (i, b) in b"lo".iter().enumerate() {
        req.ifr_name[i] = *b as libc::c_char;
    }
    req.ifr_ifru.ifru_flags = (libc::IFF_UP | libc::IFF_RUNNING) as libc::c_short;
    // `as _`, not a named type: ioctl's request argument is c_ulong on glibc
    // and c_int on musl, and this file is compiled against both — musl for the
    // shipped guest binary, glibc for `cargo test` on the host.
    let rc = unsafe { libc::ioctl(fd, libc::SIOCSIFFLAGS as _, &req) };
    unsafe { libc::close(fd) };
    if rc != 0 {
        return Err(format!("bring up lo: {}", std::io::Error::last_os_error()));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// the host connection
// ---------------------------------------------------------------------------

/// dial the host over vsock. Guest-initiated is the simpler direction: the host
/// listens on `<uds>_<port>` and Firecracker bridges this connection to it, so
/// no in-guest listener and no connect-back race.
fn connect_vsock(port: u32) -> Result<RawFd, String> {
    let fd = unsafe { libc::socket(libc::AF_VSOCK, libc::SOCK_STREAM, 0) };
    if fd < 0 {
        return Err(format!("vsock socket: {}", std::io::Error::last_os_error()));
    }
    let mut addr: libc::sockaddr_vm = unsafe { std::mem::zeroed() };
    addr.svm_family = libc::AF_VSOCK as libc::sa_family_t;
    addr.svm_cid = libc::VMADDR_CID_HOST;
    addr.svm_port = port;
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
        return Err(format!("vsock connect to host port {port}: {err}"));
    }
    Ok(fd)
}

fn connect_to_host() -> Result<RawFd, String> {
    connect_vsock(guest_proto::VSOCK_PORT)
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
    let mut in_pipe = [0 as RawFd; 2];
    let mut out_pipe = [0 as RawFd; 2];
    let mut err_pipe = [0 as RawFd; 2];
    for pipe in [&mut in_pipe, &mut out_pipe, &mut err_pipe] {
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
            libc::close(in_pipe[1]);
            libc::close(out_pipe[0]);
            libc::close(err_pipe[0]);
            libc::dup2(in_pipe[0], libc::STDIN_FILENO);
            libc::dup2(out_pipe[1], libc::STDOUT_FILENO);
            libc::dup2(err_pipe[1], libc::STDERR_FILENO);
            libc::close(in_pipe[0]);
            libc::close(out_pipe[1]);
            libc::close(err_pipe[1]);
        }
        exec_child(manifest);
    }

    unsafe {
        libc::close(in_pipe[0]);
        libc::close(out_pipe[1]);
        libc::close(err_pipe[1]);
    }
    pump(host, in_pipe[1], out_pipe[0], err_pipe[0]);
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

/// carry the run's three streams between the child and the host until both
/// output pipes reach EOF.
///
/// All three directions are driven by ONE `poll`, which is what makes stdin
/// concurrent with stdout: the run's prompt arrives on stdin and can be larger
/// than a pipe buffer, so a guest that wrote stdin first and only then read
/// output would deadlock against a CLI that streams before it drains.
///
/// `poll` rather than threads — PID 1 with four descriptors does not need a
/// runtime.
fn pump(host: RawFd, in_fd: RawFd, out_fd: RawFd, err_fd: RawFd) {
    let mut stdin_fd = in_fd;
    let mut host_buf: Vec<u8> = Vec::new();
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
        // the host half: inbound stdin frames. Retired on EOF like the others,
        // but it does NOT keep the loop alive — the run ends when the child's
        // output ends, not when the host stops talking.
        libc::pollfd {
            fd: host,
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
                match slot {
                    0 => send(host, &Frame::Stdout(payload)),
                    1 => send(host, &Frame::Stderr(payload)),
                    // the host half carries stdin frames the other way.
                    _ => {
                        host_buf.extend_from_slice(&payload);
                        feed_stdin(&mut host_buf, &mut stdin_fd);
                    }
                }
                continue;
            }
            // 0 = EOF, <0 = a broken pipe. Either way this direction is done;
            // retire it so `poll` stops reporting it ready forever.
            let is_output_pipe = slot < 2;
            if is_output_pipe {
                unsafe { libc::close(pollfd.fd) };
                open -= 1;
            }
            // The host descriptor is NOT closed here: it is the same socket
            // `send` writes to, and the run's exit code still has to go out
            // after the host has stopped sending stdin.
            pollfd.fd = -1;
        }
    }
}

/// drain whole frames out of `buf` and apply them to the child's stdin.
///
/// A partial frame stays in the buffer for the next read — a vsock read returns
/// whatever happened to arrive, and the prompt routinely spans several.
fn feed_stdin(buf: &mut Vec<u8>, stdin_fd: &mut RawFd) {
    loop {
        let frame = match guest_proto::decode(buf) {
            Ok(Some(frame)) => frame,
            // a whole frame has not arrived yet
            Ok(None) => return,
            // The host writes these, so a malformed one is our own bug rather
            // than a hostile guest. Nothing useful to do mid-run: stop feeding
            // stdin and let the child see EOF.
            Err(e) => {
                let _ = writeln!(std::io::stderr(), "duck-guest-init: stdin frame: {e}");
                close_stdin(stdin_fd);
                return;
            }
        };
        match frame {
            Frame::Stdin(bytes) => write_all(*stdin_fd, &bytes),
            Frame::StdinEof => close_stdin(stdin_fd),
            // The host never sends these; ignoring them keeps a host-side bug
            // from taking the run down.
            Frame::Stdout(_) | Frame::Stderr(_) | Frame::Exit(_) => {}
        }
    }
}

fn close_stdin(stdin_fd: &mut RawFd) {
    if *stdin_fd < 0 {
        return;
    }
    unsafe { libc::close(*stdin_fd) };
    *stdin_fd = -1;
}

fn write_all(fd: RawFd, bytes: &[u8]) {
    if fd < 0 {
        return;
    }
    let mut written = 0usize;
    while written < bytes.len() {
        let n =
            unsafe { libc::write(fd, bytes[written..].as_ptr().cast(), bytes.len() - written) };
        if n <= 0 {
            return;
        }
        written += n as usize;
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
