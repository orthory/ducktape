//! one run, one microVM: boot it, carry its stdio, read its workspace back.
//!
//! This is the seat a container runtime's create/start/attach/wait held, and it is
//! smaller because a VM needs no daemon: the VMM is a child of this process and
//! dies with the run. There is nothing to reap, no socket to keep alive between
//! runs, and no image store to garbage-collect.
//!
//! The whole lifecycle in order:
//!
//! 1. build the run's workspace into an ext4 image ([`crate::workspace_image`])
//! 2. listen on the vsock socket the guest will dial — BEFORE the VMM starts
//! 3. spawn `firecracker --no-api --config-file`
//! 4. accept the guest, then pump frames both ways
//! 5. on the guest's exit frame, wait for the VMM and read the image back
//!
//! Step 2 is ordered deliberately. Firecracker connects the guest's outbound
//! vsock to `<uds_path>_<port>` on the host; if nothing is listening there when
//! the guest dials, the guest's connect fails and the run produces no output at
//! all — a silence that looks exactly like a hung CLI.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpStream, UnixListener, UnixStream};

use crate::firecracker_api::{self, VmConfig};
use crate::guest_manifest::RunManifest;
use crate::guest_proto::{self, Frame};

/// how long to wait for the guest to dial back after the VMM starts.
///
/// This is a BOOT budget, not a run budget: the measured cold boot on the
/// development host is 452 ms tuned, and the guest dials immediately after
/// mounting. A guest that has not connected in this long is not slow, it is
/// broken — a missing init, an unmountable workspace, a kernel panic — and the
/// useful thing to do is fail with the VMM's console output rather than wait.
const GUEST_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// a live run's stdio, shaped like the pipes an ordinary child would give, so
/// the caller's output loop does not care which backend produced them.
pub struct MicroVmIo {
    pub stdin: tokio::io::DuplexStream,
    pub stdout: tokio::io::DuplexStream,
    pub stderr: tokio::io::DuplexStream,
    /// the guest's own exit frame. Distinct from the VMM process's status,
    /// which only says whether the hypervisor exited cleanly.
    pub exit: tokio::sync::oneshot::Receiver<i32>,
    pub pump: tokio::task::JoinHandle<()>,
    /// every host→guest frame goes out through here, which is why the resize
    /// lane exists at all: ONE task owns the socket's write half (it must, see
    /// [`pump_frames`]), so a second writer would have to be a second owner.
    input: tokio::sync::mpsc::Sender<Frame>,
}

impl MicroVmIo {
    /// take this io apart into the halves a TERMINAL session drives
    /// independently.
    ///
    /// Separate owners on purpose: the terminal plane pumps output in one task
    /// while feeding keystrokes from another, so a single lock over both would
    /// let a quiet terminal block the next keystroke indefinitely.
    pub fn into_terminal(self) -> TerminalIo {
        TerminalIo {
            output: self.stdout,
            input: self.stdin,
            exit: self.exit,
            resize: ResizeLane(self.input),
            idle_stderr: self.stderr,
            pump: self.pump,
        }
    }
}

/// one interactive session's io: what to read, what to write, how to resize,
/// and when the guest's child exited.
///
/// Every field is public and separately owned, because a terminal session wants
/// them in different places at once — output under one lock, input under
/// another, and resize under NO lock (the caller notices a window change on a
/// synchronous path and cannot await one).
pub struct TerminalIo {
    /// terminal output. A pty merges the child's stderr into it, so this is the
    /// whole of what the session renders.
    pub output: tokio::io::DuplexStream,
    /// keystrokes, as ordinary stdin bytes. The guest writes them into the pty
    /// master, which is what makes them terminal input.
    pub input: tokio::io::DuplexStream,
    pub exit: tokio::sync::oneshot::Receiver<i32>,
    pub resize: ResizeLane,
    /// KEEP THIS ALIVE, never read it. The pump writes any stderr frame here,
    /// and a dropped receiver turns that write into an error that ends the
    /// whole session. A pty run sends no stderr frames, so this is a trap being
    /// disarmed rather than a lane in use.
    pub idle_stderr: tokio::io::DuplexStream,
    /// the pump task, so it is owned for the session's lifetime rather than
    /// detached.
    pub pump: tokio::task::JoinHandle<()>,
}

/// the window-size lane, split out so it can be used without holding the input
/// lock: a resize arrives on a synchronous path that has no lock to await.
#[derive(Clone)]
pub struct ResizeLane(tokio::sync::mpsc::Sender<Frame>);

impl ResizeLane {
    /// tell the guest the operator's terminal changed size.
    ///
    /// Best-effort and non-blocking: a resize is a redraw hint, and a session
    /// whose input queue is momentarily full must not stall the caller that
    /// noticed the window move.
    pub fn resize(&self, cols: u16, rows: u16) {
        let _ = self.0.try_send(Frame::Resize { cols, rows });
    }
}

/// a booted microVM. Dropping it kills the VMM — a run whose caller went away
/// must not keep a guest holding memory.
pub struct MicroVm {
    vmm: tokio::process::Child,
    run_dir: PathBuf,
    workspace_image: PathBuf,
    console: PathBuf,
    /// this run's vsock socket. Held for its PARENT directory, which is the
    /// run's alone and is removed with it.
    vsock_uds: PathBuf,
    /// the tunnel acceptors, aborted when the VM goes away.
    tunnels: Vec<tokio::task::JoinHandle<()>>,
}

impl Drop for MicroVm {
    /// Aborts the tunnels and DELETES the run's scratch — both the run
    /// directory and the socket directory.
    ///
    /// The run directory holds this run's workspace image, its read-only asset
    /// image and its manifest: gigabytes for a run whose PATH entries are a
    /// build tree, and nothing there outlives the VM. Leaving it was measured
    /// at 21 GB of `/tmp` across one afternoon of tests, with no owner to
    /// notice — a node serving runs continuously would fill its disk.
    ///
    /// In Drop rather than in [`Self::collect`] so that a run that FAILED
    /// cleans up too. Every diagnostic that outlives the VM has already been
    /// read into an error string by then (see [`Self::boot_failure`]), so
    /// there is nothing here left to read.
    fn drop(&mut self) {
        for tunnel in self.tunnels.drain(..) {
            tunnel.abort();
        }
        let _ = std::fs::remove_dir_all(&self.run_dir);
        if let Some(socket_dir) = self.vsock_uds.parent() {
            let _ = std::fs::remove_dir_all(socket_dir);
        }
    }
}

impl MicroVm {
    /// boot a VM for `manifest`, with its workspace built from `workdir`.
    ///
    /// `run_dir` must be SHORT. The vsock path lives under it and a unix socket
    /// path is capped near 108 bytes (`SUN_LEN`); a scratch directory under a
    /// long home blows straight through it with `path must be shorter than
    /// SUN_LEN`. Put it under `XDG_RUNTIME_DIR`.
    pub async fn boot(
        run_dir: &Path,
        workdir: &Path,
        assets: &[crate::workspace_image::GuestAsset],
        cfg: &VmConfig,
        manifest: &RunManifest,
    ) -> Result<(Self, MicroVmIo), String> {
        std::fs::create_dir_all(run_dir)
            .map_err(|e| format!("create run dir {}: {e}", run_dir.display()))?;

        // 1. the run's per-run block devices: what it is supposed to run, its
        //    read-only inputs, and the workspace that will be read back.
        let blob = crate::guest_manifest::encode(manifest)?;
        std::fs::write(&cfg.manifest, &blob)
            .map_err(|e| format!("write {}: {e}", cfg.manifest.display()))?;
        crate::workspace_image::build_assets(assets, &cfg.assets, &run_dir.join("assets"))?;
        let size = crate::workspace_image::sized_for(workdir)?;
        crate::workspace_image::build(workdir, &cfg.workspace, size)?;

        // 2. listen BEFORE the VMM starts (see the module docs)
        //
        // The BASE path is Firecracker's own: it binds `uds_path` for
        // host-initiated connections into the guest. A leftover file there
        // from an earlier run on this run directory makes the VMM exit
        // immediately with `Address in use`, before any guest code runs — so
        // it is cleared here rather than left to a teardown that a crash
        // skipped.
        let _ = std::fs::remove_file(&cfg.vsock_uds);
        let guest_socket = vsock_port_path(&cfg.vsock_uds, guest_proto::VSOCK_PORT);
        let _ = std::fs::remove_file(&guest_socket);
        let listener = UnixListener::bind(&guest_socket)
            .map_err(|e| format!("listen on {}: {e}", guest_socket.display()))?;

        // the tunnels' host ends, bound on the same schedule and for the same
        // reason: the guest may dial them as soon as it is up.
        //
        // One listener per service, in the manifest's order. The guest picks a
        // vsock port, never a destination, so this loop IS the allowlist.
        let mut tunnels = Vec::with_capacity(manifest.tunnel_ports.len());
        for (index, port) in manifest.tunnel_ports.iter().enumerate() {
            let vsock_port = guest_proto::TUNNEL_PORT_BASE + index as u32;
            let path = vsock_port_path(&cfg.vsock_uds, vsock_port);
            let _ = std::fs::remove_file(&path);
            let tunnel_listener = UnixListener::bind(&path)
                .map_err(|e| format!("listen on {}: {e}", path.display()))?;
            tunnels.push(tokio::spawn(serve_tunnel(tunnel_listener, *port)));
        }

        // 3. the VMM
        let config = firecracker_api::boot_config(cfg);
        let config_path = firecracker_api::write_boot_config(run_dir, &config)?;
        let firecracker = crate::host_tools::find_on_path("firecracker")
            .ok_or_else(|| "firecracker is not executable on PATH".to_string())?;
        let console = run_dir.join("console.log");
        let console_file = std::fs::File::create(&console)
            .map_err(|e| format!("create {}: {e}", console.display()))?;
        let vmm = tokio::process::Command::new(&firecracker)
            .arg("--no-api")
            .arg("--config-file")
            .arg(&config_path)
            .stdin(Stdio::null())
            // the guest's serial console. The ONLY diagnostic for a guest that
            // never reaches userspace, so it is captured rather than inherited.
            .stdout(
                console_file
                    .try_clone()
                    .map_err(|e| format!("dup console: {e}"))?,
            )
            .stderr(console_file)
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("spawn firecracker: {e}"))?;

        let mut vm = MicroVm {
            vmm,
            run_dir: run_dir.to_path_buf(),
            workspace_image: cfg.workspace.clone(),
            console: console.clone(),
            vsock_uds: cfg.vsock_uds.clone(),
            tunnels,
        };

        // 4. the guest dials back
        let stream = match tokio::time::timeout(GUEST_CONNECT_TIMEOUT, listener.accept()).await {
            Ok(Ok((stream, _))) => stream,
            Ok(Err(e)) => {
                return Err(vm
                    .boot_failure(&format!("accept the guest vsock: {e}"))
                    .await);
            }
            Err(_) => {
                return Err(vm
                    .boot_failure("the guest never dialled back before the boot timeout")
                    .await);
            }
        };

        let io = spawn_pump(stream);
        Ok((vm, io))
    }

    /// the run's console output, for a failure whose cause is only visible
    /// there. Truncated: a kernel panic's trace is long and the useful part is
    /// the end.
    fn console_tail(&self) -> String {
        let raw = std::fs::read_to_string(&self.console).unwrap_or_default();
        let tail: Vec<&str> = raw.lines().rev().take(20).collect();
        tail.into_iter().rev().collect::<Vec<_>>().join("\n")
    }

    /// a boot failure, with the console attached. Kills the VMM first: whatever
    /// went wrong, a guest nobody is listening to must not keep running.
    async fn boot_failure(&mut self, what: &str) -> String {
        let _ = self.vmm.kill().await;
        format!("{what}\nguest console:\n{}", self.console_tail())
    }

    /// wait for the VMM to exit, then walk the workspace image back into
    /// `workdir`.
    ///
    /// Ordered, not concurrent: the guest syncs and unmounts before it halts,
    /// so reading the image before the VMM is gone risks reading a filesystem
    /// with an open journal.
    pub async fn collect(mut self, workdir: &Path) -> Result<(), String> {
        self.vmm
            .wait()
            .await
            .map_err(|e| format!("wait for the VMM: {e}"))?;
        crate::workspace_image::read_back(&self.workspace_image, workdir)
    }

    /// stop the VM now. Used when the run is abandoned rather than finished.
    pub async fn terminate(&mut self) {
        let _ = self.vmm.kill().await;
    }

    pub fn run_dir(&self) -> &Path {
        &self.run_dir
    }
}

/// the host end of one tunnel: every guest connection on this vsock port is
/// spliced to `service_port` on this host's loopback.
///
/// The destination is CLOSED OVER, not read off the wire. The services bind
/// LOOPBACK, not a routable interface, so nothing outside this process can
/// reach them — and the guest reaches exactly this one address because it is
/// the only address this function will ever dial. That property is what
/// replaces the container backend's nft input chain, and it is stronger: there
/// is no rule to get wrong, because there is no destination the guest can name.
async fn serve_tunnel(listener: UnixListener, service_port: u16) {
    loop {
        let Ok((guest, _)) = listener.accept().await else {
            return;
        };
        tokio::spawn(async move {
            let Ok(service) = TcpStream::connect(("127.0.0.1", service_port)).await else {
                return;
            };
            let (mut guest_read, mut guest_write) = guest.into_split();
            let (mut broker_read, mut broker_write) = service.into_split();
            // both directions concurrently: splicing them in sequence would
            // deadlock as soon as either side filled its buffer.
            let up = async {
                let _ = tokio::io::copy(&mut guest_read, &mut broker_write).await;
                let _ = broker_write.shutdown().await;
            };
            let down = async {
                let _ = tokio::io::copy(&mut broker_read, &mut guest_write).await;
                let _ = guest_write.shutdown().await;
            };
            tokio::join!(up, down);
        });
    }
}

/// Firecracker connects a guest's outbound vsock to `<uds_path>_<port>`.
fn vsock_port_path(uds: &Path, port: u32) -> PathBuf {
    let mut name = uds.as_os_str().to_os_string();
    name.push(format!("_{port}"));
    PathBuf::from(name)
}

/// wire the guest connection to duplex streams the caller can treat as pipes.
/// how many host→guest frames may queue before the writer backpressures.
///
/// Bounded on purpose: an unbounded queue would let a prompt larger than the
/// socket buffer pile up in host memory instead of slowing the reader that
/// produced it. Small, because the only producers are one prompt reader and the
/// occasional resize.
const INPUT_QUEUE: usize = 16;

fn spawn_pump(stream: UnixStream) -> MicroVmIo {
    // 64 KiB matches the invoke loop's read granularity.
    let (stdin_host, stdin_task) = tokio::io::duplex(64 * 1024);
    let (out_task, stdout_host) = tokio::io::duplex(64 * 1024);
    let (err_task, stderr_host) = tokio::io::duplex(64 * 1024);
    let (exit_tx, exit_rx) = tokio::sync::oneshot::channel();
    let (input_tx, input_rx) = tokio::sync::mpsc::channel(INPUT_QUEUE);

    tokio::spawn(frame_stdin(stdin_task, input_tx.clone()));
    let pump = tokio::spawn(pump_frames(stream, input_rx, out_task, err_task, exit_tx));

    MicroVmIo {
        stdin: stdin_host,
        stdout: stdout_host,
        stderr: stderr_host,
        exit: exit_rx,
        pump,
        input: input_tx,
    }
}

/// turn the caller's stdin writes into frames.
///
/// Its own task so a prompt larger than the socket buffer cannot block the
/// guest's output from being drained: the caller writing the prompt and the
/// caller reading the answer are frequently the same loop.
async fn frame_stdin(mut stdin: tokio::io::DuplexStream, input: tokio::sync::mpsc::Sender<Frame>) {
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = match stdin.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        if input.send(Frame::Stdin(buf[..n].to_vec())).await.is_err() {
            return;
        }
    }
    // The prompt is complete. A headless guest closes the child's stdin on
    // this so a CLI blocking on EOF proceeds; an interactive one ignores it,
    // because a terminal has no EOF to close.
    let _ = input.send(Frame::StdinEof).await;
}

async fn pump_frames(
    stream: UnixStream,
    mut input: tokio::sync::mpsc::Receiver<Frame>,
    mut stdout: tokio::io::DuplexStream,
    mut stderr: tokio::io::DuplexStream,
    exit: tokio::sync::oneshot::Sender<i32>,
) {
    let (mut read_half, mut write_half) = stream.into_split();

    // host -> guest. ONE owner of the write half, which is why every inbound
    // lane (the prompt, keystrokes, resizes) is funnelled through one channel
    // rather than writing here directly.
    let feed = tokio::spawn(async move {
        while let Some(frame) = input.recv().await {
            if write_half
                .write_all(&guest_proto::encode(&frame))
                .await
                .is_err()
            {
                break;
            }
        }

        // Everything sendable is sent, but this task must NOT end: it owns the
        // write half, and Firecracker's vsock backend maps a host-side
        // half-close onto a full connection RESET rather than a half-close.
        // Dropping it here therefore kills the guest's output stream mid-run —
        // measured, the guest echoed the prompt and got EPIPE, and the host saw
        // a run that produced nothing and reported no exit code.
        //
        // The abort at the end of this function is what ends it, and that
        // close is also the guest's signal that its exit frame landed.
        std::future::pending::<()>().await
    });

    // guest -> host
    let mut pending: Vec<u8> = Vec::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut exit = Some(exit);
    'outbound: loop {
        let n = match read_half.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        pending.extend_from_slice(&buf[..n]);
        loop {
            let frame = match guest_proto::decode(&mut pending) {
                Ok(Some(frame)) => frame,
                Ok(None) => break,
                // A guest that speaks nonsense is untrusted input, not a bug.
                // Stop reading; the caller sees EOF and a missing exit code.
                Err(_) => return,
            };
            match frame {
                Frame::Stdout(bytes) => {
                    if stdout.write_all(&bytes).await.is_err() {
                        return;
                    }
                }
                Frame::Stderr(bytes) => {
                    if stderr.write_all(&bytes).await.is_err() {
                        return;
                    }
                }
                // Last frame of the run, and closing here is the guest's
                // signal that it may reset. Firecracker relays the guest's
                // vsock writes asynchronously, so a guest that halted the
                // instant after writing would lose whatever the VMM had not
                // relayed yet — and the frame that goes missing is this one.
                Frame::Exit(code) => {
                    if let Some(tx) = exit.take() {
                        let _ = tx.send(code);
                    }
                    break 'outbound;
                }
                // the guest never sends these; a stray one is not worth failing
                // a finished run over.
                Frame::Stdin(_) | Frame::StdinEof | Frame::Resize { .. } => {}
            }
        }
    }
    // Awaited, not just aborted: the write half lives in that task, and the
    // guest is blocked on this socket closing. Dropping the handle without
    // awaiting leaves the close to whenever the runtime gets round to it.
    feed.abort();
    let _ = feed.await;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Firecracker's guest-outbound convention. Getting this wrong means
    /// nothing is listening where the guest dials, and the run produces no
    /// output at all — a silence indistinguishable from a hung CLI.
    #[test]
    fn the_guest_dials_the_port_suffixed_socket() {
        assert_eq!(
            vsock_port_path(Path::new("/run/user/1000/dt/v.sock"), 1024),
            PathBuf::from("/run/user/1000/dt/v.sock_1024")
        );
    }

    /// The prompt is finished long before the run is, and the task that sent it
    /// OWNS the write half. Letting it return there drops that half, and
    /// Firecracker's vsock backend maps a host-side half-close onto a full
    /// connection RESET rather than a half-close — measured, the guest echoed
    /// its prompt straight into EPIPE and the run reached the operator as
    /// "produced nothing, reported no exit code".
    ///
    /// Parsed from the source rather than exercised, because the behaviour
    /// being guarded belongs to Firecracker's backend: a `UnixStream::pair`
    /// honours the half-close, so a socketpair test passes either way and
    /// guards nothing. What IS checkable here is the shape that avoids it.
    #[test]
    fn the_feed_task_parks_instead_of_returning() {
        let src = include_str!("microvm.rs");
        let feed = src
            .split_once("let feed = tokio::spawn(")
            .expect("the feed task")
            .1
            .split_once("\n    });")
            .expect("the feed task is one spawn block")
            .0;
        assert!(
            feed.contains("std::future::pending::<()>().await"),
            "the feed task owns the write half and must park, never return:\n{feed}"
        );
    }

    /// A unix socket path is capped near 108 bytes. The run directory is
    /// chosen by the caller, so the constraint is documented on `boot` — this
    /// pins the arithmetic that makes it checkable.
    #[test]
    fn a_run_directory_under_xdg_runtime_stays_inside_sun_len() {
        let path = vsock_port_path(
            Path::new("/run/user/1000/ducktape/run-0123456789/v.sock"),
            1024,
        );
        assert!(
            path.as_os_str().len() < 108,
            "{} is {} bytes",
            path.display(),
            path.as_os_str().len()
        );
    }
}
