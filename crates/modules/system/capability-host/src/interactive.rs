//! interactive, pty-backed CLI sessions — the same executor a headless run
//! spawns, but attached to a pseudo-terminal so a human can drive its native
//! TUI (codex, claude) keystroke-by-keystroke instead of feeding it one prompt
//! on stdin.
//!
//! This shares the headless path's whole isolation seam — the broker holds the
//! credential and the child gets only an opaque bearer + loopback base URL, the
//! fresh config home stops any dotfile fallback, and the Podman backend fences
//! the filesystem — and differs in exactly two places: the argv is the spec's
//! `[interactive]` TUI argv (not `[invoke]`'s headless one), and the child's
//! stdio is a pty the host holds the master of, not pipes.
//!
//! **Podman only.** The `Direct` backend has no mount namespace and no fresh
//! HOME, so an interactive session on it would expose the operator's whole home
//! to whoever is typing — the exact thing the sandbox exists to prevent. So
//! [`CliProvider::spawn_interactive_session`] refuses anything but Podman; the
//! pty primitive underneath ([`InteractiveSession::spawn_on_pty`]) is backend
//! agnostic only so its behavior can be unit-tested against a plain local child.
//!
//! There is deliberately NO idle-timeout kill here (a terminal is idle by
//! nature): a session ends on explicit [`InteractiveSession::close`], on the
//! child exiting (the master read returns EIO → EOF), or when dropped. Spend is
//! bounded by the broker's own request/byte caps, not by output silence.

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::io::unix::AsyncFd;
use tokio::sync::Mutex;

use crate::broker::RunBroker;
use crate::sandbox::{self, SandboxBackend};
use crate::{
    BrokerKind, CliProvider, LiveChild, PodmanRun, RunAuth, RunContext, TartGuard,
    broker_provider_overrides, canonical_mount_path, configure_process_group,
};

/// a live interactive session: the child process on one end of a pty, the host
/// holding the master. Every method takes `&self` (the read and write halves are
/// separate dups of the master), so the owner can wrap it in an `Arc` and pump
/// bytes in one task while feeding input from another without splitting it.
pub struct InteractiveSession {
    /// the master's read half (a dup of the pty master, non-blocking).
    reader: AsyncFd<OwnedFd>,
    /// the master's write half (a second dup of the same master).
    writer: AsyncFd<OwnedFd>,
    /// the child + its Podman lifecycle, behind a lock so `close` can tear it
    /// down through `&self`. `kill_on_drop` is the final safety net.
    live: Mutex<LiveChild>,
    /// the Tart VM guard, when the backend is Tart. Declared AFTER `live` so the
    /// ssh child dies before this guard's Drop (synchronously) stops/deletes the
    /// VM. `None` under Podman (where `live` carries the container lifecycle).
    _tart: Option<TartGuard>,
    /// held for the session's lifetime: dropping the broker tears its loopback
    /// endpoint down, and the config home must outlive the child that reads it.
    _broker: Option<RunBroker>,
    _config_home: Option<PathBuf>,
}

impl InteractiveSession {
    /// spawn `command` with its stdio wired to a fresh pty and keep the master.
    /// `podman`/`broker`/`config_home` are the session's owned lifecycle handles.
    /// Backend-agnostic on purpose — [`CliProvider::spawn_interactive_session`]
    /// hands it a `podman run -it …` command; a test hands it a plain `cat`.
    fn spawn_on_pty(
        mut command: tokio::process::Command,
        podman: Option<PodmanRun>,
        tart: Option<TartGuard>,
        broker: Option<RunBroker>,
        config_home: Option<PathBuf>,
    ) -> Result<Self, String> {
        let (master, slave) = open_pty()?;
        set_nonblocking(&master)?;
        let writer_fd = master
            .try_clone()
            .map_err(|e| format!("dup pty master: {e}"))?;
        let stdin = slave.try_clone().map_err(|e| format!("dup pty slave: {e}"))?;
        let stdout = slave.try_clone().map_err(|e| format!("dup pty slave: {e}"))?;
        command
            .stdin(Stdio::from(stdin))
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(slave))
            .kill_on_drop(true);
        configure_process_group(&mut command);
        let child = command
            .spawn()
            .map_err(|e| format!("spawn interactive session: {e}"))?;
        let live = LiveChild::new(child, podman);
        let reader = AsyncFd::new(master).map_err(|e| format!("register pty master: {e}"))?;
        let writer = AsyncFd::new(writer_fd).map_err(|e| format!("register pty master: {e}"))?;
        Ok(Self {
            reader,
            writer,
            live: Mutex::new(live),
            _tart: tart,
            _broker: broker,
            _config_home: config_home,
        })
    }

    /// read the next chunk of terminal output. `Ok(0)` means end of session: on
    /// Linux the master read returns EIO once the last slave (the child) is
    /// gone, which we map to EOF.
    pub async fn read(&self, buf: &mut [u8]) -> std::io::Result<usize> {
        loop {
            let mut guard = self.reader.readable().await?;
            let res = guard.try_io(|inner| {
                let fd = inner.get_ref().as_raw_fd();
                // SAFETY: fd is our live master pty; buf is valid for `buf.len()`.
                let n = unsafe { libc::read(fd, buf.as_mut_ptr().cast(), buf.len()) };
                if n < 0 {
                    let e = std::io::Error::last_os_error();
                    // the master EIOs when every slave has closed → treat as EOF.
                    if e.raw_os_error() == Some(libc::EIO) {
                        return Ok(0);
                    }
                    Err(e)
                } else {
                    Ok(n as usize)
                }
            });
            match res {
                Ok(result) => return result,
                Err(_would_block) => continue,
            }
        }
    }

    /// write input (keystrokes) to the terminal, in full.
    pub async fn write_all(&self, mut data: &[u8]) -> std::io::Result<()> {
        while !data.is_empty() {
            let mut guard = self.writer.writable().await?;
            let res = guard.try_io(|inner| {
                let fd = inner.get_ref().as_raw_fd();
                // SAFETY: fd is our live master pty; data is valid for `data.len()`.
                let n = unsafe { libc::write(fd, data.as_ptr().cast(), data.len()) };
                if n < 0 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(n as usize)
                }
            });
            match res {
                Ok(Ok(n)) => data = &data[n..],
                Ok(Err(e)) => return Err(e),
                Err(_would_block) => continue,
            }
        }
        Ok(())
    }

    /// resize the terminal. Setting the master's window size makes the kernel
    /// SIGWINCH the slave's foreground group; under Podman that is the container
    /// process, which relays the new size to the CLI's own tty.
    pub fn resize(&self, cols: u16, rows: u16) -> std::io::Result<()> {
        let ws = libc::winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        let fd = self.reader.get_ref().as_raw_fd();
        // SAFETY: fd is our live master pty; &ws is a valid winsize for the ioctl.
        let rc = unsafe { libc::ioctl(fd, libc::TIOCSWINSZ, &ws) };
        if rc != 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// end the session: terminate the child's process group and clean up the
    /// Podman container. Idempotent-ish — safe to call once on session teardown.
    pub async fn close(&self) {
        self.live.lock().await.terminate().await;
    }

    #[cfg(test)]
    fn window_size(&self) -> (u16, u16) {
        // SAFETY: zeroed winsize is valid; ioctl fills it from the master pty.
        let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
        let fd = self.reader.get_ref().as_raw_fd();
        let rc = unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, &mut ws) };
        assert_eq!(rc, 0, "TIOCGWINSZ failed: {}", std::io::Error::last_os_error());
        (ws.ws_col, ws.ws_row)
    }
}

impl CliProvider {
    /// spawn this capability's interactive TUI on a pty. Requires a SANDBOX
    /// backend (Podman or Tart) — `Direct` has no mount namespace / fresh HOME
    /// and is refused — and a spec with an `[interactive]` argv. The isolation is
    /// the headless path's (a broker holding the credential, a fresh config home,
    /// the container/VM fence); only the argv and the stdio (pty, not pipes)
    /// differ. Podman keeps the container lifecycle on the pty child itself; Tart
    /// spawns `sshpass ssh -tt` into a guest VM and the returned session holds
    /// the [`TartGuard`] that stops/deletes it on drop.
    pub(crate) async fn spawn_interactive_session(
        &self,
        ctx: &RunContext,
        restricted: bool,
    ) -> Result<InteractiveSession, String> {
        if matches!(self.backend, SandboxBackend::Direct) {
            return Err(format!(
                "{}: interactive sessions require a sandbox backend (Podman or Tart); \
                 this node runs Direct",
                self.spec.tag
            ));
        }
        let Some(interactive) = self.spec.interactive.clone() else {
            return Err(format!(
                "{}: this capability declares no [interactive] argv",
                self.spec.tag
            ));
        };
        // a restricted (shared/command-lane) session runs the read-only,
        // non-prompting argv; a capability that declares none does not support
        // shared mode, and we refuse rather than run it unrestricted.
        let base = if restricted {
            match interactive.restricted_args.as_deref() {
                Some(a) => a,
                None => {
                    return Err(format!(
                        "{}: this capability declares no restricted [interactive] argv \
                         (shared/command-lane session unsupported)",
                        self.spec.tag
                    ));
                }
            }
        } else {
            &interactive.args
        };
        let workdir = self.ensure_writable_workdir(ctx)?;
        let workdir = canonical_mount_path(&workdir, "sandbox workdir")?;
        let config_home = self.prepare_config_home(&workdir, ctx)?;
        // no per-run airlock resolution is wired into interactive sessions yet;
        // the env/host-credential path is unchanged.
        let broker = self.start_broker(None).await?;
        let auth = RunAuth {
            config_home: config_home.as_deref(),
            broker: broker.as_ref().map(|b| &b.endpoint),
        };
        let args = interactive_argv(base, &auth, &workdir, self.spec.isolation.broker);

        match &self.backend {
            SandboxBackend::Podman { image } => {
                let (mut command, run) =
                    self.podman_command(image, &args, &workdir, ctx, &auth, true)?;
                run.prepare_cidfile()?;
                command.current_dir(&workdir);
                InteractiveSession::spawn_on_pty(command, Some(run), None, broker, config_home)
            }
            SandboxBackend::Tart { .. } => {
                // build the interactive guest plan (its script `exec`s the TUI,
                // no rsync-back), clone/boot the VM, then attach a pty to
                // `sshpass ssh -tt` into it. the guard rides in the session so
                // the VM is stopped/deleted when the session ends.
                let plan = self.tart_plan(&args, &workdir, ctx, &auth, true)?;
                let guard = self
                    .tart_setup(Some(&plan), ctx)
                    .await?
                    .ok_or_else(|| format!("{}: Tart setup returned no VM guard", self.spec.tag))?;
                let mut command = tokio::process::Command::new("sshpass");
                command.args(sandbox::tart_ssh_argv(&guard.ip, &plan.guest_script, true));
                command.current_dir(&workdir);
                InteractiveSession::spawn_on_pty(command, None, Some(guard), broker, config_home)
            }
            SandboxBackend::Direct => unreachable!("Direct is refused above"),
        }
    }
}

/// the interactive TUI argv. For CODEX, the broker's `-c` overrides are
/// PREPENDED (a TUI argv has no `exec` subcommand to splice them after, and
/// codex reads `-c` as global config). For the Anthropic broker (claude) the
/// aiming is entirely by ENV (see [`CliProvider::apply_auth_env`]), so the base
/// argv passes through verbatim — as it does when there is no broker at all.
fn interactive_argv(
    base: &[String],
    auth: &RunAuth<'_>,
    workdir: &Path,
    kind: Option<BrokerKind>,
) -> Vec<String> {
    let (Some(broker), Some(BrokerKind::CodexResponses)) = (auth.broker, kind) else {
        return base.to_vec();
    };
    let mut argv = broker_provider_overrides(broker, workdir);
    argv.extend(base.iter().cloned());
    argv
}

/// allocate a pseudo-terminal via the POSIX path (`posix_openpt` + `grantpt` +
/// `unlockpt` + a platform slave-name lookup + `open`), so this needs no
/// `libutil` link the way `openpty` would. Returns `(master, slave)`.
fn open_pty() -> Result<(OwnedFd, OwnedFd), String> {
    // SAFETY: posix_openpt allocates a master pty; O_NOCTTY keeps it from
    // becoming this process's controlling terminal.
    let master = unsafe { libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY) };
    if master < 0 {
        return Err(format!("posix_openpt: {}", std::io::Error::last_os_error()));
    }
    // SAFETY: `master` is a fresh fd we now own.
    let master = unsafe { OwnedFd::from_raw_fd(master) };
    let mfd = master.as_raw_fd();
    // SAFETY: mfd is a live master pty for the two setup ioctls.
    if unsafe { libc::grantpt(mfd) } != 0 {
        return Err(format!("grantpt: {}", std::io::Error::last_os_error()));
    }
    if unsafe { libc::unlockpt(mfd) } != 0 {
        return Err(format!("unlockpt: {}", std::io::Error::last_os_error()));
    }
    let mut name = [0 as libc::c_char; 256];
    pty_slave_name(mfd, &mut name)?;
    // SAFETY: `name` is NUL-terminated by the platform slave-name lookup.
    let slave = unsafe { libc::open(name.as_ptr(), libc::O_RDWR | libc::O_NOCTTY) };
    if slave < 0 {
        return Err(format!("open pty slave: {}", std::io::Error::last_os_error()));
    }
    // SAFETY: `slave` is a fresh fd we now own.
    let slave = unsafe { OwnedFd::from_raw_fd(slave) };
    // Give the pty a sane INITIAL size (80x24). A pty is created at 0x0; a TUI
    // handed 0x0 (podman -t relays it into the container tty) renders blank until
    // the first resize — and the app's first resize can be dropped while its ws
    // is still connecting. The real client geometry replaces this via `resize`.
    let ws = libc::winsize {
        ws_row: 24,
        ws_col: 80,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: mfd is a live master pty; &ws is a valid winsize.
    unsafe { libc::ioctl(mfd, libc::TIOCSWINSZ, &ws) };
    Ok((master, slave))
}

#[cfg(target_os = "macos")]
fn pty_slave_name(mfd: libc::c_int, name: &mut [libc::c_char]) -> Result<(), String> {
    // macOS has no ptsname_r. TIOCPTYGNAME is its thread-safe kernel interface
    // for copying the slave path into a caller-owned MAXPATHLEN-sized buffer.
    // SAFETY: mfd is a live master pty and `name` provides 256 writable bytes,
    // the buffer size encoded by TIOCPTYGNAME.
    if unsafe { libc::ioctl(mfd, libc::TIOCPTYGNAME.into(), name.as_mut_ptr()) } != 0 {
        return Err(format!(
            "ioctl TIOCPTYGNAME: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn pty_slave_name(mfd: libc::c_int, name: &mut [libc::c_char]) -> Result<(), String> {
    // SAFETY: `name` is a writable buffer; ptsname_r writes the NUL-terminated
    // slave path into it.
    if unsafe { libc::ptsname_r(mfd, name.as_mut_ptr(), name.len()) } != 0 {
        return Err(format!("ptsname_r: {}", std::io::Error::last_os_error()));
    }
    Ok(())
}

/// mark `fd` non-blocking (`O_NONBLOCK` on the open file description, which its
/// dups share) so [`AsyncFd`] can drive it.
fn set_nonblocking(fd: &OwnedFd) -> Result<(), String> {
    let raw = fd.as_raw_fd();
    // SAFETY: raw is a live fd we own.
    let flags = unsafe { libc::fcntl(raw, libc::F_GETFL) };
    if flags < 0 {
        return Err(format!("fcntl F_GETFL: {}", std::io::Error::last_os_error()));
    }
    // SAFETY: raw is a live fd we own; setting O_NONBLOCK on its status flags.
    if unsafe { libc::fcntl(raw, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(format!("fcntl F_SETFL: {}", std::io::Error::last_os_error()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// the pty primitive pumps bytes both ways: write to the master, the child
    /// (`cat`) echoes it back, we read it. Proves openpty + AsyncFd read/write
    /// without needing podman or a logged-in CLI. (`cat` on a pty also sees the
    /// line-discipline echo, so the payload is guaranteed to come back.)
    #[tokio::test]
    async fn pty_round_trips_bytes_through_a_child() {
        let session =
            InteractiveSession::spawn_on_pty(
                tokio::process::Command::new("cat"),
                None,
                None,
                None,
                None,
            )
            .expect("spawn cat on a pty");
        session.write_all(b"ping\n").await.expect("write to pty");

        let mut seen = Vec::new();
        let mut buf = [0u8; 256];
        // read until the payload appears or the child is gone.
        for _ in 0..20 {
            let n = tokio::time::timeout(std::time::Duration::from_secs(5), session.read(&mut buf))
                .await
                .expect("read did not hang")
                .expect("read from pty");
            if n == 0 {
                break;
            }
            seen.extend_from_slice(&buf[..n]);
            if seen.windows(4).any(|w| w == b"ping") {
                break;
            }
        }
        session.close().await;
        assert!(
            seen.windows(4).any(|w| w == b"ping"),
            "expected the pty to echo 'ping', got {:?}",
            String::from_utf8_lossy(&seen)
        );
    }

    /// resize sets the master's window size; the kernel reflects it back through
    /// TIOCGWINSZ. Pure ioctl round-trip — no podman.
    #[tokio::test]
    async fn resize_sets_the_window_size() {
        let session = InteractiveSession::spawn_on_pty(
            tokio::process::Command::new("cat"),
            None,
            None,
            None,
            None,
        )
        .expect("spawn cat on a pty");
        session.resize(120, 40).expect("resize");
        assert_eq!(session.window_size(), (120, 40));
        session.close().await;
    }

    /// the broker overrides are PREPENDED for a CODEX interactive (TUI) argv
    /// (no `exec` selector to splice after); every other case passes the base
    /// argv through verbatim.
    #[test]
    fn interactive_argv_prepends_only_for_codex() {
        // no broker → base argv verbatim.
        let bare = interactive_argv(
            &["--foo".to_string()],
            &RunAuth::default(),
            Path::new("/w"),
            None,
        );
        assert_eq!(bare, vec!["--foo".to_string()]);

        let endpoint = crate::broker::BrokerEndpoint {
            base_url: "http://127.0.0.1:9/v1".into(),
            run_bearer: "b".into(),
            control_url: String::new(),
            control_token: String::new(),
        };
        let auth = RunAuth {
            config_home: None,
            broker: Some(&endpoint),
        };
        // codex → overrides prepended.
        let codex = interactive_argv(
            &["--foo".to_string()],
            &auth,
            Path::new("/w"),
            Some(BrokerKind::CodexResponses),
        );
        assert_eq!(codex.first().map(String::as_str), Some("-c"));
        assert_eq!(codex.last().map(String::as_str), Some("--foo"));

        // claude (Anthropic) → base argv verbatim, aiming is by ENV not argv.
        let claude = interactive_argv(
            &["--foo".to_string()],
            &auth,
            Path::new("/w"),
            Some(BrokerKind::AnthropicMessages),
        );
        assert_eq!(claude, vec!["--foo".to_string()]);
    }

    // ---- live podman integration (skips when podman is unavailable) ---------
    // These exercise the SAME pty primitive against a REAL `podman run -it`
    // container — the bridge PR1 could only argv-assert off a podman host.

    fn podman_available() -> bool {
        std::process::Command::new("podman")
            .arg("version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    async fn read_until(session: &InteractiveSession, needle: &[u8], rounds: usize) -> Vec<u8> {
        let mut seen = Vec::new();
        let mut buf = [0u8; 4096];
        for _ in 0..rounds {
            match tokio::time::timeout(
                std::time::Duration::from_secs(20),
                session.read(&mut buf),
            )
            .await
            {
                Ok(Ok(0)) | Ok(Err(_)) | Err(_) => break,
                Ok(Ok(n)) => seen.extend_from_slice(&buf[..n]),
            }
            if seen.windows(needle.len()).any(|w| w == needle) {
                break;
            }
        }
        seen
    }

    /// REAL podman: openpty + `podman run -it … cat` bridges bytes both ways.
    #[tokio::test]
    async fn pty_bridges_a_real_podman_container() {
        if !podman_available() {
            eprintln!("skipping pty_bridges_a_real_podman_container: no working podman");
            return;
        }
        let mut cmd = tokio::process::Command::new("podman");
        cmd.args([
            "run", "--rm", "-i", "-t", "--network=host",
            "docker.io/library/debian:13-slim", "cat",
        ]);
        let session = InteractiveSession::spawn_on_pty(cmd, None, None, None, None)
            .expect("spawn podman on a pty");
        session.write_all(b"ping\n").await.expect("write to pty");
        let seen = read_until(&session, b"ping", 60).await;
        session.close().await;
        assert!(
            seen.windows(4).any(|w| w == b"ping"),
            "container cat should echo 'ping', got {:?}",
            String::from_utf8_lossy(&seen)
        );
    }

    /// FULL codex path: `discover` the real embedded codex spec on a Podman
    /// backend, `spawn_interactive`, and confirm codex's TUI actually renders in
    /// the container through our argv + mount + broker + pty. `#[ignore]` — it
    /// needs podman + a host codex binary + `~/.codex/auth.json` (the broker's
    /// upstream) and runs a real container; drive with:
    ///   PATH=<podman helpers> cargo test -p capability-host -- --ignored --nocapture codex_tui_renders
    /// It does NOT submit a prompt, so it makes no model call / spends nothing.
    #[tokio::test]
    #[ignore = "live: needs podman + host codex + ~/.codex/auth.json"]
    async fn codex_tui_renders_in_a_real_container() {
        if !podman_available() {
            eprintln!("skipping: no working podman");
            return;
        }
        let image = std::env::var("DUCKTAPE_SANDBOX_IMAGE")
            .unwrap_or_else(|_| "localhost/ducktape-agent:dev".into());
        let dirs = crate::AgentDirs::under(std::path::Path::new("/tmp/ducktape-codex-verify"));
        let set = crate::discover(
            b"verify-node-000000000000000000000",
            dirs,
            None,
            crate::SandboxBackend::Podman { image },
            // match the terminal plane's forced private netns (L9).
            true,
        )
        .expect("discover codex on Podman");
        let provider = set.resolve("codex").expect("codex provider present");
        let ctx = RunContext {
            agent_id: Some("verify".into()),
            executing_node: Some(crate::execution_node_id(
                b"verify-node-000000000000000000000",
            )),
            env: std::iter::once(("TERM".to_string(), "xterm-256color".to_string())).collect(),
            ..Default::default()
        };
        let session = match provider.spawn_interactive(&ctx, false).await {
            Ok(s) => s,
            Err(e) => panic!("spawn_interactive(codex) failed: {e}"),
        };
        // read codex's initial TUI render (no prompt submitted → no model call).
        let seen = read_until(&session, b"\x1b[", 40).await; // any ANSI = a TUI drew
        session.close().await;
        let text = String::from_utf8_lossy(&seen);
        eprintln!("--- codex TUI output ({} bytes) ---\n{text}\n--- end ---", seen.len());
        assert!(
            !seen.is_empty(),
            "codex produced no output in the container — TUI did not launch"
        );
    }

    /// build the Podman provider set the live model-turn tests share.
    #[cfg(test)]
    fn live_podman_set() -> Option<crate::ProviderSet> {
        if !podman_available() {
            eprintln!("skipping live model turn: no working podman");
            return None;
        }
        let image = std::env::var("DUCKTAPE_SANDBOX_IMAGE")
            .unwrap_or_else(|_| "localhost/ducktape-agent:dev".into());
        Some(
            crate::discover(
                b"verify-node-000000000000000000000",
                crate::AgentDirs::under(std::path::Path::new("/tmp/ducktape-live-verify")),
                None,
                crate::SandboxBackend::Podman { image },
                // match the terminal plane's forced private netns (L9).
                true,
            )
            .expect("discover on Podman"),
        )
    }

    #[cfg(test)]
    fn live_ctx(agent: &str) -> RunContext {
        let mut pairs = vec![("TERM".to_string(), "xterm-256color".to_string())];
        // Let the operator pin the model for the live turn (e.g. a less-throttled
        // tier) without editing the test — forwarded into the sandbox as env.
        if let Ok(model) = std::env::var("ANTHROPIC_MODEL") {
            pairs.push(("ANTHROPIC_MODEL".to_string(), model));
        }
        RunContext {
            agent_id: Some(agent.into()),
            executing_node: Some(crate::execution_node_id(b"verify-node-000000000000000000000")),
            env: pairs.into_iter().collect(),
            ..Default::default()
        }
    }

    /// FULL codex MODEL TURN through the broker: `provider.run` a trivial prompt
    /// in the container; the broker (holding ~/.codex/auth.json) reaches the
    /// model and codex returns an answer. Proves credential-isolated model access
    /// end-to-end. `#[ignore]` — spends a tiny bit of the operator's codex quota.
    #[tokio::test]
    #[ignore = "live model turn: spends codex quota; needs podman + ~/.codex/auth.json"]
    async fn codex_model_turn_through_the_broker() {
        let Some(set) = live_podman_set() else { return };
        let provider = set.resolve("codex").expect("codex provider");
        let answer = provider
            .run(
                "Reply with exactly one word: PONG. Nothing else.",
                &live_ctx("verify-codex"),
            )
            .await
            .expect("codex model turn failed");
        eprintln!("--- codex answer ---\n{answer}\n--- end ---");
        assert!(!answer.trim().is_empty(), "codex returned an empty answer");
    }

    /// FULL claude MODEL TURN through the Anthropic broker (PR2): `provider.run`
    /// a trivial prompt; the broker (holding ~/.claude/.credentials.json) proxies
    /// /v1/messages to api.anthropic.com and claude returns an answer. Exercises
    /// the SSE broker + the OAuth path against the REAL upstream. `#[ignore]` —
    /// spends a tiny bit of the operator's claude quota.
    #[tokio::test]
    #[ignore = "live model turn: spends claude quota; needs podman + ~/.claude/.credentials.json"]
    async fn claude_model_turn_through_the_broker() {
        let Some(set) = live_podman_set() else { return };
        let provider = set.resolve("claude").expect("claude provider");
        let answer = provider
            .run(
                "Reply with exactly one word: PONG. Nothing else.",
                &live_ctx("verify-claude"),
            )
            .await
            .expect("claude model turn failed");
        eprintln!("--- claude answer ---\n{answer}\n--- end ---");
        assert!(!answer.trim().is_empty(), "claude returned an empty answer");
    }

    /// read whatever the session emits for ~`secs` seconds into `sink`.
    #[cfg(test)]
    async fn drain_for(sink: &mut Vec<u8>, session: &InteractiveSession, secs: u64) {
        let mut buf = [0u8; 8192];
        for _ in 0..(secs * 10) {
            match tokio::time::timeout(
                std::time::Duration::from_millis(100),
                session.read(&mut buf),
            )
            .await
            {
                Ok(Ok(0)) | Ok(Err(_)) => return,
                Ok(Ok(n)) => sink.extend_from_slice(&buf[..n]),
                Err(_) => {} // 100ms tick with nothing — keep waiting
            }
        }
    }

    /// strip ANSI/control noise so a dumped TUI screen is human-readable.
    #[cfg(test)]
    fn deansi(bytes: &[u8]) -> String {
        let s = String::from_utf8_lossy(bytes);
        let mut out = String::new();
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\x1b' {
                // skip CSI/OSC etc until a letter/BEL terminates it
                while let Some(&n) = chars.peek() {
                    chars.next();
                    if n.is_ascii_alphabetic() || n == '\x07' {
                        break;
                    }
                }
            } else if c == '\r' || (!c.is_control() || c == '\n') {
                out.push(c);
            }
        }
        out
    }

    /// drive an interactive session: let the TUI fully init, type `prompt`,
    /// submit (Enter, twice to be robust to a not-yet-ready composer), read the
    /// reply. Returns the full raw transcript.
    #[cfg(test)]
    async fn drive_tui(session: &InteractiveSession, prompt: &str) -> Vec<u8> {
        use std::time::Duration;
        let mut all = Vec::new();
        drain_for(&mut all, session, 7).await; // initial render / first-run dialog
        // claude's first run in a fresh workspace shows a "trust this folder?"
        // dialog whose default is Yes — confirm it. codex has no such dialog, so
        // this Enter lands in an empty composer and is a harmless no-op.
        session.write_all(b"\r").await.ok();
        drain_for(&mut all, session, 3).await; // composer becomes ready
        session.write_all(prompt.as_bytes()).await.ok();
        tokio::time::sleep(Duration::from_millis(1500)).await;
        session.write_all(b"\r").await.ok(); // submit
        tokio::time::sleep(Duration::from_millis(2000)).await;
        session.write_all(b"\r").await.ok(); // again, in case the first was pre-ready
        drain_for(&mut all, session, 40).await; // model reply
        all
    }

    /// does `needle` appear in `hay` once every NON-alphanumeric byte (ANSI,
    /// cursor moves, spaces, newlines) is stripped? A TUI renders each glyph in
    /// its own cell with cursor moves between, so the reply is never contiguous
    /// RAW — but stripping the noise leaves its letters adjacent. Contiguous (not
    /// subsequence) match, so scattered chrome letters can't spuriously satisfy it.
    #[cfg(test)]
    fn letters_contains(hay: &[u8], needle: &str) -> bool {
        // strip ANSI FIRST (its parameter digits/letters would otherwise splice
        // into the text stream), then keep only alphanumerics.
        let letters: String = deansi(hay)
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .collect();
        letters.contains(needle)
    }

    // The prompt asks the model to TRANSFORM a word to uppercase, so the reply
    // (ZEPHYR) is distinguishable from the prompt's own echo (zephyr).
    #[cfg(test)]
    const TURN_PROMPT: &str = "Reply with ONLY the uppercase form of the word zephyr and nothing else.";
    #[cfg(test)]
    const TURN_REPLY: &str = "ZEPHYR";

    /// LIVE interactive MODEL TURN for codex: launch the TUI, type a prompt, and
    /// read the model's rendered reply. `#[ignore]` — spends codex quota.
    #[tokio::test]
    #[ignore = "live interactive turn: spends codex quota"]
    async fn codex_interactive_model_turn() {
        let Some(set) = live_podman_set() else { return };
        let provider = set.resolve("codex").expect("codex provider");
        let session = provider
            .spawn_interactive(&live_ctx("verify-codex-tui"), false)
            .await
            .expect("spawn codex TUI");
        let raw = drive_tui(&session, TURN_PROMPT).await;
        session.close().await;
        eprintln!("=== codex TUI transcript (deansi) ===\n{}\n=== end ===", deansi(&raw));
        assert!(
            letters_contains(&raw, TURN_REPLY),
            "codex TUI never rendered the model reply ({TURN_REPLY})"
        );
    }

    /// LIVE interactive MODEL TURN for claude: same, against the Anthropic broker.
    #[tokio::test]
    #[ignore = "live interactive turn: spends claude quota"]
    async fn claude_interactive_model_turn() {
        let Some(set) = live_podman_set() else { return };
        let provider = set.resolve("claude").expect("claude provider");
        let session = provider
            .spawn_interactive(&live_ctx("verify-claude-tui"), false)
            .await
            .expect("spawn claude TUI");
        let raw = drive_tui(&session, TURN_PROMPT).await;
        session.close().await;
        eprintln!("=== claude TUI transcript (deansi) ===\n{}\n=== end ===", deansi(&raw));
        assert!(
            letters_contains(&raw, TURN_REPLY),
            "claude TUI never rendered the model reply ({TURN_REPLY})"
        );
    }

    /// LIVE: a SHARED (restricted) codex session spawns under the read-only,
    /// never-ask argv and STILL renders a plain model reply — proving the
    /// restricted argv is accepted by the real binary and that read-only does
    /// not gag ordinary conversation (only writes/exec). `#[ignore]` — quota.
    #[tokio::test]
    #[ignore = "live interactive turn: spends codex quota"]
    async fn codex_shared_restricted_model_turn() {
        let Some(set) = live_podman_set() else { return };
        let provider = set.resolve("codex").expect("codex provider");
        let session = provider
            .spawn_interactive(&live_ctx("verify-codex-shared"), true)
            .await
            .expect("spawn restricted codex TUI");
        let raw = drive_tui(&session, TURN_PROMPT).await;
        session.close().await;
        eprintln!("=== codex RESTRICTED TUI transcript (deansi) ===\n{}\n=== end ===", deansi(&raw));
        assert!(
            letters_contains(&raw, TURN_REPLY),
            "restricted codex TUI never rendered the model reply ({TURN_REPLY})"
        );
    }

    /// REAL podman: `-t` gives the CONTAINER process a genuine tty — what a TUI
    /// needs. `test -t 0` is true only over a real pty.
    #[tokio::test]
    async fn podman_dash_t_gives_the_container_a_real_tty() {
        if !podman_available() {
            eprintln!("skipping podman tty test: no working podman");
            return;
        }
        let mut cmd = tokio::process::Command::new("podman");
        cmd.args([
            "run", "--rm", "-i", "-t", "--network=host",
            "docker.io/library/debian:13-slim",
            "sh", "-c", "test -t 0 && printf ISATTY; sleep 1",
        ]);
        let session = InteractiveSession::spawn_on_pty(cmd, None, None, None, None)
            .expect("spawn podman on a pty");
        let seen = read_until(&session, b"ISATTY", 60).await;
        session.close().await;
        assert!(
            seen.windows(6).any(|w| w == b"ISATTY"),
            "container stdin should be a tty under -t, got {:?}",
            String::from_utf8_lossy(&seen)
        );
    }
}
