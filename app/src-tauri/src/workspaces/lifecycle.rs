//! Stopping a workspace's node for real, plus the process/port liveness
//! oracles the commands and phase reporting share.

use std::fs;
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::process::{Command, Stdio};
use std::time::Duration;

use super::registry::Ports;

/// is something accepting connections on this localhost port right now? used as
/// a liveness probe for an already-running workspace node.
///
/// BOTH loopback families are probed: the app mints the mesh listener as
/// `[::]:<port>` (dual-stack), and on hosts where an IPv4-mapped connect to a
/// v6 socket doesn't take, a 127.0.0.1-only probe reports a LIVE node as down —
/// the adopt guard then double-spawns, the duplicate dies address-in-use on the
/// mesh port, and the supervisor crash-loops to its cap (observed live, epic
/// QA BUG-1). `::1` sees a `[::]` listener unconditionally; `127.0.0.1` sees an
/// IPv4-bound one. Either answering means the port is held.
pub(crate) fn port_listening(port: u16) -> bool {
    use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
    let timeout = Duration::from_millis(200);
    let v4 = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let v6 = SocketAddr::from((Ipv6Addr::LOCALHOST, port));
    TcpStream::connect_timeout(&v4, timeout).is_ok()
        || TcpStream::connect_timeout(&v6, timeout).is_ok()
}

/// is the node WE spawned for this workspace still alive? reads the pidfile
/// `workspace_select` wrote and signal-0s it. `None` when there is no pidfile
/// (never spawned by us, or an adopted node whose pid we don't own) — the
/// caller must not infer death from an absent pidfile.
pub(super) fn recorded_pid_alive(dir: &Path) -> Option<bool> {
    let raw = fs::read_to_string(pidfile(dir)).ok()?;
    let pid = raw.trim();
    if pid.is_empty() {
        return None;
    }
    Some(pid_alive(pid))
}

/// the recorded pid as a number, or `None` when there is no (parseable)
/// pidfile — an adopted or never-spawned node. mirrors [`recorded_pid_alive`]'s
/// contract but yields the pid itself, for the runtime-facts row.
pub(super) fn read_pid(dir: &Path) -> Option<u32> {
    fs::read_to_string(pidfile(dir)).ok()?.trim().parse().ok()
}

/// elapsed running time of `pid` in seconds, via `ps -o etime`. unix only —
/// `ps` is this module's portable-enough process oracle (see [`cmdline_of`]).
/// `None` when the process is gone or the field can't be parsed.
#[cfg(unix)]
pub(super) fn node_uptime_secs(pid: u32) -> Option<u64> {
    let out = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "etime="])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    super::phase::parse_etime(String::from_utf8_lossy(&out.stdout).trim())
}

#[cfg(not(unix))]
pub(super) fn node_uptime_secs(_pid: u32) -> Option<u64> {
    None
}

/// unix `kill -0 <pid>`: succeeds iff the process exists. shells out to match
/// the rest of this module's teardown path (no libc dep in this crate).
#[cfg(unix)]
fn pid_alive(pid: &str) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn pid_alive(_pid: &str) -> bool {
    true // best-effort on non-unix; the dev box is linux.
}

/// the pidfile `workspace_select` records next to `daemon.log` after a spawn,
/// so teardown can address the detached process directly.
pub(super) fn pidfile(dir: &Path) -> PathBuf {
    dir.join("node.pid")
}

/// the full command line of a live process, or `None` when it is gone (or the
/// platform can't tell). unix only — `ps` is the one portable-enough oracle.
#[cfg(unix)]
fn cmdline_of(pid: u32) -> Option<String> {
    let out = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "command="])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let line = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!line.is_empty()).then_some(line)
}

/// pids of LIVE processes that are verifiably THIS workspace's node: the
/// recorded pidfile pid plus a `pgrep -f` sweep for the workspace dir (a
/// wiped-and-recreated registry loses pidfiles; the sweep still finds those
/// zombies). every candidate is verified against its actual command line
/// before it may be killed — a recycled pid must never take an innocent
/// process down.
#[cfg(unix)]
fn workspace_node_pids(dir: &Path) -> Vec<u32> {
    let marker = dir.to_string_lossy().to_string();
    let mut candidates: Vec<u32> = Vec::new();
    if let Some(pid) = fs::read_to_string(pidfile(dir))
        .ok()
        .and_then(|text| text.trim().parse::<u32>().ok())
    {
        candidates.push(pid);
    }
    if let Ok(out) = Command::new("pgrep").args(["-f", &marker]).output() {
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            if let Ok(pid) = line.trim().parse::<u32>() {
                candidates.push(pid);
            }
        }
    }
    candidates.sort_unstable();
    candidates.dedup();
    let ours = std::process::id();
    candidates
        .into_iter()
        .filter(|pid| *pid != ours)
        .filter(|pid| cmdline_of(*pid).is_some_and(|cmd| cmd.contains(&marker)))
        .collect()
}

/// the first LIVE, verified pid of this workspace's node — the adopt path uses
/// it to make the pidfile honest again after a crash-loop left a corpse's pid
/// behind (epic QA BUG-3). verification is [`workspace_node_pids`]'s exe/
/// cmdline match; never a bare pattern kill.
#[cfg(unix)]
pub(super) fn live_workspace_node_pid(dir: &Path) -> Option<u32> {
    workspace_node_pids(dir).into_iter().find(|pid| process_alive(*pid))
}

/// is `pid` a LIVE process? a zombie counts as dead: the shell never reaps its
/// spawned nodes, so a killed child lingers as `Z` — and `kill -0` keeps
/// succeeding on it, which would burn the whole TERM+KILL grace on an
/// already-dead process. read the state instead of probing signalability.
#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    let Ok(out) = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "stat="])
        .output()
    else {
        return false;
    };
    let stat = String::from_utf8_lossy(&out.stdout).trim().to_string();
    out.status.success() && !stat.is_empty() && !stat.starts_with('Z')
}

/// TERM then (after `grace`) KILL `pid`, waiting for it to exit. best-effort —
/// the caller confirms the outcome by port, not by our signals landing.
#[cfg(unix)]
fn kill_pid(pid: u32, grace: Duration) {
    let alive = process_alive;
    let _ = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .stderr(Stdio::null())
        .status();
    let deadline = std::time::Instant::now() + grace;
    while alive(pid) && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    if alive(pid) {
        let _ = Command::new("kill")
            .args(["-KILL", &pid.to_string()])
            .stderr(Stdio::null())
            .status();
        let deadline = std::time::Instant::now() + grace;
        while alive(pid) && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

/// stop this workspace's node FOR REAL: ask nicely over http first
/// (/v1/admin/shutdown), then kill every verified process of this workspace, then
/// CONFIRM its ports are released. `Err` when something still holds a port —
/// the caller must NOT delete state a live process would just re-create (the
/// zombie-workspace resurrection this replaces).
pub(super) fn stop_workspace_node(
    dir: &Path,
    ports: &Ports,
    grace: Duration,
) -> Result<(), String> {
    // raise the stop-intent BEFORE any kill: teardown escalates to TERM/KILL,
    // which the supervisor would otherwise read as a crash and auto-restart —
    // fighting the very teardown that is trying to release the ports. this flag
    // is the one thing that tells a deliberate stop apart from a crash.
    crate::daemon::mark_stopping(dir);
    // graceful first: the node exits its whole process on this route. a node
    // already down, or a parked joiner serving no http, just fails the connect.
    post_shutdown(ports.http);

    #[cfg(unix)]
    {
        // give the graceful exit a moment before reaching for signals.
        let deadline = std::time::Instant::now() + grace;
        while ports_held(ports) && std::time::Instant::now() < deadline {
            let pids = workspace_node_pids(dir);
            if pids.is_empty() {
                break;
            }
            for pid in pids {
                kill_pid(pid, grace);
            }
        }
        // sweep any survivor once more even if the ports never showed as held
        // (a parked joiner binds only its mesh listener; a fatal-looping node
        // may hold nothing at all between restarts).
        for pid in workspace_node_pids(dir) {
            kill_pid(pid, grace);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = dir; // no pid oracle here — the port check below still gates.
    }

    // the honest gate: something still answering on this workspace's ports
    // means the node is NOT stopped, whatever the signals claimed.
    let deadline = std::time::Instant::now() + grace;
    while ports_held(ports) {
        if std::time::Instant::now() >= deadline {
            return Err(format!(
                "this workspace's node is still running (a listener still holds port {} or {}) \
                 and could not be stopped — aborting so it can't haunt a deleted workspace. \
                 stop the process manually, then try again.",
                ports.listen, ports.http
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = fs::remove_file(pidfile(dir));
    // the node is confirmed down and its ports released; drop the supervisor
    // registration so a later spawn starts from a fresh (cleared) flag.
    crate::daemon::clear_supervised(dir);
    Ok(())
}

/// is anything still listening on the ports this workspace owns? the mesh
/// listener is bound in every phase (parked included); http only once serving.
fn ports_held(ports: &Ports) -> bool {
    port_listening(ports.listen) || port_listening(ports.http)
}

/// best-effort "stop this node": POST /v1/admin/shutdown to its http surface over a
/// raw tcp write. the port addresses the node (mirroring the webview's
/// `shutdownNode` in node-bootstrap.ts), and the node exits the whole process
/// on this route. a node already down, or a parked joiner that serves no http,
/// just fails the connect — [`stop_workspace_node`] escalates from here.
fn post_shutdown(http_port: u16) {
    use std::io::Write as _;
    use std::net::{SocketAddr, TcpStream};
    let addr = SocketAddr::from(([127, 0, 0, 1], http_port));
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(500)) else {
        return;
    };
    let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
    // the owner-gated control surface (ADR A2). loopback-trusted here — this is
    // a 127.0.0.1 dial to the app's own managed node, so no PoP is needed; the
    // node's `Loopback` exposure admits it. a refusal just falls through to the
    // pidfile/signal escalation in `stop_workspace_node`.
    let req = format!(
        "POST /v1/admin/shutdown HTTP/1.1\r\nHost: 127.0.0.1:{http_port}\r\n\
         Content-Length: 0\r\nConnection: close\r\n\r\n"
    );
    let _ = stream.write_all(req.as_bytes());
    let _ = stream.flush();
}

// ── stop_workspace_node: the forget teardown must be REAL ──
//
// the old best-effort http shutdown left parked/wedged nodes running after
// a forget; the detached survivor kept its ports and re-created `storage/`
// under the deleted directory. these pin the repaired contract: verified
// processes of the workspace die, innocents are never signalled, and a
// port still held after teardown refuses instead of lying.
#[cfg(all(test, unix))]
mod tests {
    use std::net::TcpListener;

    use super::super::ports::free_port;
    use super::*;

    /// a scratch workspace dir; its path is what pid verification matches.
    fn scratch_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ducktape-stop-test-{}-{tag}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// a long-lived stand-in node whose command line embeds `dir` (the
    /// trailing `$0` argument keeps `sh` from exec-replacing itself with
    /// `sleep`, which would drop the marker from the command line).
    fn spawn_fake_node(dir: &Path) -> std::process::Child {
        Command::new("sh")
            .arg("-c")
            .arg("sleep 30; : \"$0\"")
            .arg(dir.join("node.toml"))
            .spawn()
            .unwrap()
    }

    /// wait for OUR child to be reaped dead (kill -0 lies for zombies).
    fn died(child: &mut std::process::Child, within: Duration) -> bool {
        let deadline = std::time::Instant::now() + within;
        while std::time::Instant::now() < deadline {
            if child.try_wait().unwrap().is_some() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }

    /// closed-at-probe-time ports — nothing should be listening on them.
    fn closed_ports() -> Ports {
        let listen = free_port(&[]).unwrap();
        let http = free_port(&[listen]).unwrap();
        Ports {
            listen,
            http,
            rpc: 0,
            wireguard: None,
            invite: None,
        }
    }

    #[test]
    fn kills_the_recorded_pid() {
        let dir = scratch_dir("pidfile");
        let mut child = spawn_fake_node(&dir);
        fs::write(pidfile(&dir), child.id().to_string()).unwrap();

        stop_workspace_node(&dir, &closed_ports(), Duration::from_millis(600)).unwrap();

        assert!(
            died(&mut child, Duration::from_secs(2)),
            "the pidfile-recorded node process must be stopped"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn sweeps_a_zombie_with_no_pidfile() {
        // a wiped-and-recreated registry loses pidfiles; the command-line
        // sweep must still find and stop the workspace's process.
        let dir = scratch_dir("sweep");
        let mut child = spawn_fake_node(&dir);

        stop_workspace_node(&dir, &closed_ports(), Duration::from_millis(600)).unwrap();

        assert!(
            died(&mut child, Duration::from_secs(2)),
            "a zombie found by command line must be stopped"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_term_killed_zombie_child_does_not_burn_the_grace() {
        // the shell never reaps its spawned nodes, so a TERM-killed child
        // lingers as a zombie — and `kill -0` keeps SUCCEEDING on zombies.
        // liveness must read the process STATE, or every teardown burns
        // the full TERM+KILL grace on an already-dead process (observed
        // live: an 18s forget).
        let dir = scratch_dir("zombie");
        let mut child = spawn_fake_node(&dir);
        fs::write(pidfile(&dir), child.id().to_string()).unwrap();

        let started = std::time::Instant::now();
        stop_workspace_node(&dir, &closed_ports(), Duration::from_secs(3)).unwrap();
        let elapsed = started.elapsed();

        assert!(
            died(&mut child, Duration::from_secs(2)),
            "the node must be stopped"
        );
        assert!(
            elapsed < Duration::from_secs(2),
            "teardown burned the kill grace on a zombie: {elapsed:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn never_kills_an_unverified_pid() {
        // a recycled pid recorded in a stale pidfile belongs to someone
        // else now — its command line has no trace of this workspace, so
        // it must survive the teardown untouched.
        let dir = scratch_dir("innocent");
        let mut innocent = Command::new("sh")
            .arg("-c")
            .arg("sleep 30")
            .spawn()
            .unwrap();
        fs::write(pidfile(&dir), innocent.id().to_string()).unwrap();

        stop_workspace_node(&dir, &closed_ports(), Duration::from_millis(600)).unwrap();

        assert!(
            innocent.try_wait().unwrap().is_none(),
            "an unverified pid must never be signalled"
        );
        innocent.kill().unwrap();
        innocent.wait().unwrap();
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn refuses_while_a_port_is_still_held() {
        // something unstoppable still listening on the workspace's port
        // means teardown MUST refuse — deleting state under a live process
        // is exactly the zombie-resurrection bug this replaces.
        let dir = scratch_dir("held");
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let held = listener.local_addr().unwrap().port();
        let ports = Ports {
            listen: held,
            http: free_port(&[held]).unwrap(),
            rpc: 0,
            wireguard: None,
            invite: None,
        };

        let err = stop_workspace_node(&dir, &ports, Duration::from_millis(400))
            .expect_err("a held port must refuse the teardown");
        assert!(err.contains("still running"), "{err}");
        drop(listener);
        let _ = fs::remove_dir_all(&dir);
    }

    /// epic QA BUG-1 regression: the liveness probe must see a listener on
    /// EITHER loopback family. The app mints the mesh listener as `[::]:<port>`;
    /// a 127.0.0.1-only probe missing it is what double-spawned a live node
    /// into an address-in-use crash loop.
    #[test]
    fn port_listening_sees_both_loopback_families() {
        // an IPv4-loopback listener (the fleet-seed shape).
        let v4 = TcpListener::bind("127.0.0.1:0").unwrap();
        assert!(port_listening(v4.local_addr().unwrap().port()));
        drop(v4);

        // a v6 wildcard listener (the app-minted `[::]:<port>` mesh shape).
        // skipped, not failed, on a host without IPv6.
        if let Ok(v6) = TcpListener::bind("[::]:0") {
            assert!(
                port_listening(v6.local_addr().unwrap().port()),
                "a [::] listener must read as held"
            );
            drop(v6);
        }

        // a port nobody holds reads as free.
        let free = free_port(&[]).unwrap();
        assert!(!port_listening(free));
    }
}
