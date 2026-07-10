//! the FUSE `mount` verb (cargo feature `fuse`).
//!
//! `mount <prefix> <dir> [--snapshot S] [--rw] [--auto-commit N] [--node URL]`
//! fronts a duckfs subtree as a real kernel filesystem and blocks until
//! SIGINT/SIGTERM unmounts it cleanly.
//!
//! - **read-only (default)** serves directly over the phase-3 [`NodeApi`] at a
//!   snapshot resolved ONCE (explicit `--snapshot`, else the head at mount time)
//!   and PINNED for the mount's lifetime — a stable, reproducible view. content at
//!   a snapshot is immutable, so it caches hard (a lazy inode table off `ls`/`stat`
//!   and a bounded read-through block cache). reads never see the moving head; a
//!   remount is how you observe newer commits. see [`ro`].
//! - **`--rw`** fronts a REAL phase-3 working copy: a checkout into a hidden
//!   `<dir>.duckfs-backing` dir served passthrough, committed through the phase-3
//!   engine — explicit by default (unmount prints how to commit), `--auto-commit N`
//!   opt-in. see [`rw`].
//!
//! this crate mounts UNPRIVILEGED via the setuid `fusermount3` helper (fuser 0.17,
//! no libfuse). the whole module is gated behind the `fuse` feature so a default
//! build needs neither `fuser` nor `libc`.
//!
//! documented non-goals (also in the CLI help + module docs): no cross-node lock
//! coherence, no mmap coherence, POSIX uid/gid/mode are synthetic, and
//! case-colliding sibling names cannot materialize on a case-insensitive host.

mod attr;
mod cache;
mod inode;
mod ro;
mod rw;

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::args::{CliError, parse_flags, resolve_node};

/// the parsed `mount` invocation.
struct MountArgs {
    prefix: String,
    dir: PathBuf,
    snapshot: Option<String>,
    rw: bool,
    auto_commit: Option<Duration>,
    node_url: String,
}

/// parse the mount args, resolve the node, and run the mount (blocks until a
/// SIGINT/SIGTERM unmount). signals are blocked FIRST — before any thread is
/// spawned (the reqwest client's runtime, the fuse session, the auto-commit
/// worker) — so every thread inherits the block and the one `sigwait` on the main
/// thread reaps the signal and drives a clean unmount instead of an abrupt kill
/// that would strand the mountpoint.
pub fn run(args: &[String]) -> Result<(), CliError> {
    block_term_signals();
    let mount = parse_mount_args(args)?;
    std::fs::create_dir_all(&mount.dir)
        .map_err(|e| CliError::failed(format!("mountpoint {}: {e}", mount.dir.display())))?;
    if mount.rw {
        rw::mount_rw(&mount)
    } else {
        ro::mount_ro(&mount)
    }
}

fn parse_mount_args(args: &[String]) -> Result<MountArgs, CliError> {
    let (pos, flags) = parse_flags(args)?;
    let [prefix, dir] = pos.as_slice() else {
        return Err(CliError::usage(
            "mount needs <prefix> <dir> (e.g. mount /shared ~/mnt --node http://127.0.0.1:8844)",
        ));
    };
    let node_url = resolve_node(&flags)?;
    let snapshot = flags.get("snapshot").filter(|s| !s.is_empty()).cloned();
    let rw = flags.contains_key("rw");
    let auto_commit = match flags.get("auto-commit") {
        Some(v) if !v.is_empty() => {
            let secs: u64 = v
                .parse()
                .map_err(|_| CliError::usage("--auto-commit needs a whole number of seconds"))?;
            if secs == 0 {
                return Err(CliError::usage("--auto-commit seconds must be > 0"));
            }
            Some(Duration::from_secs(secs))
        }
        _ => None,
    };
    if auto_commit.is_some() && !rw {
        return Err(CliError::usage(
            "--auto-commit only applies to a --rw mount",
        ));
    }
    Ok(MountArgs {
        prefix: prefix.trim_end_matches('/').to_string(),
        dir: PathBuf::from(dir),
        snapshot,
        rw,
        auto_commit,
        node_url,
    })
}

/// how long the kernel may cache an attr/entry reply. RO content at a pinned
/// snapshot is immutable, so this is generous; the RW mount reuses it because the
/// kernel revalidates on write and we serve real metadata.
const TTL: Duration = Duration::from_secs(60);

/// build the fuse session config. `#[non_exhaustive]` `Config` is constructed via
/// `default()` then field assignment (a struct literal is forbidden downstream).
/// a single event-loop thread keeps the `&self` filesystem callbacks serialized,
/// so the interior `Mutex`es never contend.
fn session_config(read_only: bool) -> fuser::Config {
    let mut cfg = fuser::Config::default();
    let mut opts = vec![
        fuser::MountOption::FSName("duckfs".to_string()),
        fuser::MountOption::Subtype("duckfs".to_string()),
    ];
    opts.push(if read_only {
        fuser::MountOption::RO
    } else {
        fuser::MountOption::RW
    });
    cfg.mount_options = opts;
    cfg.n_threads = Some(1);
    cfg
}

/// mount `fs` at `dir`, print `banner`, and block until SIGINT/SIGTERM, then drop
/// the session (fuser's `BackgroundSession::drop` unmounts). the caller does any
/// post-unmount work (the RW mount's final commit) after this returns.
fn serve_until_signal<FS: fuser::Filesystem + Send + 'static>(
    fs: FS,
    dir: &Path,
    read_only: bool,
    banner: &str,
) -> Result<(), CliError> {
    let cfg = session_config(read_only);
    let session = fuser::spawn_mount2(fs, dir, &cfg)
        .map_err(|e| CliError::failed(format!("mount {}: {e}", dir.display())))?;
    eprintln!("{banner}");
    wait_term_signal();
    eprintln!("ducktape-fs: unmounting {}", dir.display());
    drop(session);
    Ok(())
}

/// block SIGINT + SIGTERM on the calling thread. threads spawned afterward inherit
/// the block, so no thread is killed by these signals — they stay pending until
/// [`wait_term_signal`] reaps one.
fn block_term_signals() {
    unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        libc::sigaddset(&mut set, libc::SIGINT);
        libc::sigaddset(&mut set, libc::SIGTERM);
        libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut());
    }
}

/// block until a SIGINT or SIGTERM arrives (they are masked, so `sigwait` reaps
/// the pending one deterministically on this thread).
fn wait_term_signal() {
    unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        libc::sigaddset(&mut set, libc::SIGINT);
        libc::sigaddset(&mut set, libc::SIGTERM);
        let mut sig: libc::c_int = 0;
        libc::sigwait(&set, &mut sig);
    }
}

/// the mounting user's uid/gid — every synthetic entry is owned by them.
fn caller_ids() -> (u32, u32) {
    unsafe { (libc::getuid(), libc::getgid()) }
}
