//! The desktop shell's node-control boundary.
//!
//! Tauri commands NEVER execute or wait on `ducktape-node` themselves. They
//! submit a closed-over, typed Rust operation to [`NodeControl`], a single
//! bounded actor running on its own OS thread. A wedged CLI verb can therefore
//! occupy only that actor (and is killed at its deadline); it cannot consume a
//! Tauri async-runtime worker per invocation until the desktop stops pumping.
//!
//! The actor is also the serialization boundary for registry/key mutations:
//! two concurrent webview invokes cannot race a load-modify-save sequence or
//! run two custody operations against `user.key` at once. The frontend never
//! supplies an executable path or an arbitrary shell command. Backend code
//! reaches the fixed `ducktape-node` sidecar through [`run_verb`] or starts the
//! one long-lived workspace process through [`spawn_workspace_node`].
//!
//! Binary resolution, in order: the `DUCKTAPE_NODE_BIN` env override, then
//! `ducktape-node` next to this executable — which covers BOTH builds: in dev
//! the workspace target dir holds the binaries side by side (run
//! `bun run sidecar` once), and in the bundle Tauri's externalBin places the
//! sidecar next to the app executable.
//!
//! Detaching: on unix the child gets its own process group, so a terminal
//! Ctrl-C to `tauri dev` (or the app quitting) never signals it; on windows
//! DETACHED_PROCESS + CREATE_NEW_PROCESS_GROUP is the equivalent. stdio goes
//! to `daemon.log`. The Tauri shell plugin's sidecar API is NOT
//! used on purpose — it kills children when the app exits.

use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write as _};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use zeroize::Zeroize as _;

/// Maximum queued node-control jobs. Backpressure is deliberate: a compromised
/// or buggy webview cannot allocate an unbounded pile of closures/passwords
/// behind one slow node verb.
const CONTROL_QUEUE_CAPACITY: usize = 32;
/// A request is stale after this long *waiting* behind another operation. In
/// particular, a burst of destructive commands must not execute minutes after
/// the UI that issued them has moved on.
const CONTROL_MAX_QUEUE_AGE: Duration = Duration::from_secs(30);

type ControlJob = Box<dyn FnOnce() + Send + 'static>;

/// A bounded, single-thread actor for every blocking node-control operation.
///
/// Clone is cheap (a bounded channel sender). Jobs catch their own panic and
/// report a content-free error so one malformed request cannot kill the actor
/// or accidentally reflect a secret-bearing panic payload into the webview.
#[derive(Clone)]
pub(crate) struct NodeControl {
    queue: tauri::async_runtime::Sender<ControlJob>,
}

impl NodeControl {
    pub(crate) fn new() -> Result<Self, String> {
        let (queue, mut receiver) =
            tauri::async_runtime::channel::<ControlJob>(CONTROL_QUEUE_CAPACITY);
        std::thread::Builder::new()
            .name("desktop-node-control".into())
            .spawn(move || {
                while let Some(job) = receiver.blocking_recv() {
                    job();
                }
            })
            .map_err(|err| format!("start node-control actor: {err}"))?;
        Ok(Self { queue })
    }

    /// Run one blocking operation on the actor and await it without occupying a
    /// Tauri runtime worker. The operation itself is backend code, never a
    /// frontend-provided executable or closure.
    pub(crate) async fn run<T, F>(&self, operation: F) -> Result<T, String>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, String> + Send + 'static,
    {
        let (reply, mut result) = tauri::async_runtime::channel(1);
        let job = control_job(operation, reply);
        self.enqueue(job)?;
        result
            .recv()
            .await
            .unwrap_or_else(|| Err("node-control actor stopped".to_string()))
    }

    /// The same actor boundary for a non-Tauri OS thread (the ephemeral phone
    /// enrollment server). Never call this from inside an actor operation.
    pub(crate) fn run_blocking<T, F>(&self, operation: F) -> Result<T, String>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, String> + Send + 'static,
    {
        let (reply, mut result) = tauri::async_runtime::channel(1);
        let job = control_job(operation, reply);
        self.enqueue(job)?;
        result
            .blocking_recv()
            .unwrap_or_else(|| Err("node-control actor stopped".to_string()))
    }

    fn enqueue(&self, job: ControlJob) -> Result<(), String> {
        self.queue.try_send(job).map_err(|_| {
            "node-control queue is full or unavailable — wait for the current operation to finish"
                .to_string()
        })
    }
}

fn control_job<T, F>(
    operation: F,
    reply: tauri::async_runtime::Sender<Result<T, String>>,
) -> ControlJob
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    control_job_with_max_age(operation, reply, CONTROL_MAX_QUEUE_AGE)
}

fn control_job_with_max_age<T, F>(
    operation: F,
    reply: tauri::async_runtime::Sender<Result<T, String>>,
    max_age: Duration,
) -> ControlJob
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    let enqueued_at = Instant::now();
    Box::new(move || {
        // Closing a webview/cancelling its command future drops the receiver.
        // Do not perform a now-unobserved mutation just because it was queued.
        if reply.is_closed() {
            return;
        }
        let outcome = if enqueued_at.elapsed() >= max_age {
            Err("node-control operation expired while waiting in the queue".to_string())
        } else {
            match catch_unwind(AssertUnwindSafe(operation)) {
                Ok(outcome) => outcome,
                Err(_) => Err("node-control operation panicked".to_string()),
            }
        };
        let _ = reply.blocking_send(outcome);
    })
}

/// High-authority node/key commands belong to the console window. The tray and
/// huddle webviews intentionally share only their narrow command sets; even if
/// one is compromised it cannot create/delete a workspace or ask the user key
/// to sign an account mutation.
pub(crate) fn require_main_window(window: &tauri::WebviewWindow) -> Result<(), String> {
    if window.label() == "main" {
        Ok(())
    } else {
        Err("this command is restricted to the main console window".into())
    }
}

/// A short-lived node CLI operation may run this long. The actor keeps the
/// desktop responsive; the deadline keeps one wedged operation from owning the
/// actor forever.
const VERB_TIMEOUT: Duration = Duration::from_secs(30);
/// A trusted node verb should never need unbounded stdout/stderr. Drain the
/// pipes fully (so the child cannot deadlock on a full pipe), but retain at most
/// this much from each and reject a truncated success as a broken contract.
const MAX_VERB_OUTPUT_BYTES: usize = 1024 * 1024;
/// Bounds attacker-controlled strings before they become argv/stdin buffers.
const MAX_VERB_ARG_BYTES: usize = 1024 * 1024;
const MAX_VERB_STDIN_BYTES: usize = 64 * 1024;
const MAX_VERB_ARGS: usize = 64;
const ALLOWED_VERBS: &[&str] = &[
    "init",
    "keygen",
    "join",
    "invite",
    "join-requests",
    "invite-accept",
    "member-remove",
    "promote",
    "resident-remove",
    "member-leave",
    "member-status",
    "user-key",
    "user-sign-bind",
    "user-sign-unbind",
    "user-sign-possession",
    "user-sign-add-member",
    "user-sign-remove-member",
    "user-p256-payload",
];

struct SecretBytes(Vec<u8>);

impl Drop for SecretBytes {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

struct Capture {
    bytes: Vec<u8>,
    truncated: bool,
    read_failed: bool,
}

fn drain_capped(mut pipe: impl Read) -> Capture {
    let mut bytes = Vec::new();
    let mut truncated = false;
    let mut read_failed = false;
    let mut chunk = [0u8; 8 * 1024];
    loop {
        match pipe.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                let keep = MAX_VERB_OUTPUT_BYTES.saturating_sub(bytes.len()).min(n);
                bytes.extend_from_slice(&chunk[..keep]);
                truncated |= keep != n;
            }
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => {
                read_failed = true;
                break;
            }
        }
    }
    Capture {
        bytes,
        truncated,
        read_failed,
    }
}

fn validate_invocation(args: &[&str], stdin_lines: &[&str]) -> Result<(), String> {
    let verb = args
        .first()
        .copied()
        .ok_or_else(|| "node operation has no verb".to_string())?;
    if !ALLOWED_VERBS.contains(&verb) {
        return Err(format!("node operation verb {verb:?} is not allowed"));
    }
    if !stdin_lines.is_empty()
        && !matches!(
            verb,
            "user-key"
                | "user-sign-bind"
                | "user-sign-unbind"
                | "user-sign-possession"
                | "user-sign-add-member"
                | "user-sign-remove-member"
        )
    {
        return Err(format!(
            "node operation verb {verb:?} does not accept secret stdin"
        ));
    }
    if args.len() > MAX_VERB_ARGS {
        return Err(format!(
            "node operation has too many arguments ({} > {MAX_VERB_ARGS})",
            args.len()
        ));
    }
    let arg_bytes = args.iter().try_fold(0usize, |total, arg| {
        if arg.as_bytes().contains(&0) {
            return Err("node operation argument contains NUL".to_string());
        }
        total
            .checked_add(arg.len())
            .ok_or_else(|| "node operation arguments are too large".to_string())
    })?;
    if arg_bytes > MAX_VERB_ARG_BYTES {
        return Err(format!(
            "node operation arguments are too large ({arg_bytes} bytes; limit {MAX_VERB_ARG_BYTES})"
        ));
    }
    let stdin_bytes = stdin_lines.iter().try_fold(0usize, |total, line| {
        if line
            .as_bytes()
            .iter()
            .any(|byte| matches!(byte, 0 | b'\n' | b'\r'))
        {
            return Err("node operation stdin contains a forbidden delimiter".to_string());
        }
        total
            .checked_add(line.len().saturating_add(1))
            .ok_or_else(|| "node operation stdin is too large".to_string())
    })?;
    if stdin_bytes > MAX_VERB_STDIN_BYTES {
        return Err(format!(
            "node operation stdin is too large ({stdin_bytes} bytes; limit {MAX_VERB_STDIN_BYTES})"
        ));
    }
    Ok(())
}

/// Run one allowlisted backend-built `ducktape-node` verb. There is no shell:
/// arguments are passed as distinct argv entries, the executable path is
/// resolved internally, stdin is closed, and output is bounded.
pub(crate) fn run_verb(args: &[&str]) -> Result<String, String> {
    run_verb_inner(args, &[])
}

/// As [`run_verb`], with newline-delimited secret stdin fields. The assembled
/// buffer is zeroed on every exit path and written on a helper thread so even a
/// child that never reads stdin remains governed by [`VERB_TIMEOUT`].
pub(crate) fn run_verb_with_stdin(args: &[&str], stdin_lines: &[&str]) -> Result<String, String> {
    run_verb_inner(args, stdin_lines)
}

fn run_verb_inner(args: &[&str], stdin_lines: &[&str]) -> Result<String, String> {
    validate_invocation(args, stdin_lines)?;
    let verb = args.first().copied().unwrap_or("");
    let binary = resolve_node_bin()?;
    let mut command = Command::new(binary);
    command
        .args(args)
        .stdin(if stdin_lines.is_empty() {
            Stdio::null()
        } else {
            Stdio::piped()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|err| format!("run ducktape-node {verb}: {err}"))?;

    let stdin_writer = if stdin_lines.is_empty() {
        None
    } else {
        let mut payload = SecretBytes(Vec::new());
        for line in stdin_lines {
            payload.0.extend_from_slice(line.as_bytes());
            payload.0.push(b'\n');
        }
        let mut pipe = child.stdin.take().expect("stdin piped");
        match std::thread::Builder::new()
            .name("node-verb-stdin".into())
            .spawn(move || {
                let _ = pipe.write_all(&payload.0);
                let _ = pipe.flush();
            }) {
            Ok(writer) => Some(writer),
            Err(err) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("start ducktape-node {verb} stdin writer: {err}"));
            }
        }
    };

    wait_for_verb(verb, child, stdin_writer, stdin_lines)
}

fn wait_for_verb(
    verb: &str,
    child: Child,
    stdin_writer: Option<std::thread::JoinHandle<()>>,
    secret_lines: &[&str],
) -> Result<String, String> {
    wait_for_verb_with_timeout(verb, child, stdin_writer, secret_lines, VERB_TIMEOUT)
}

fn wait_for_verb_with_timeout(
    verb: &str,
    mut child: Child,
    stdin_writer: Option<std::thread::JoinHandle<()>>,
    secret_lines: &[&str],
    timeout: Duration,
) -> Result<String, String> {
    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");
    let out_reader = match std::thread::Builder::new()
        .name("node-verb-stdout".into())
        .spawn(move || drain_capped(stdout))
    {
        Ok(reader) => reader,
        Err(err) => {
            let _ = child.kill();
            let _ = child.wait();
            if let Some(writer) = stdin_writer {
                let _ = writer.join();
            }
            return Err(format!("start ducktape-node {verb} stdout reader: {err}"));
        }
    };
    let err_reader = match std::thread::Builder::new()
        .name("node-verb-stderr".into())
        .spawn(move || drain_capped(stderr))
    {
        Ok(reader) => reader,
        Err(err) => {
            let _ = child.kill();
            let _ = child.wait();
            if let Some(writer) = stdin_writer {
                let _ = writer.join();
            }
            let _ = out_reader.join();
            return Err(format!("start ducktape-node {verb} stderr reader: {err}"));
        }
    };

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if Instant::now() < deadline => sleep(Duration::from_millis(25)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(format!(
                    "ducktape-node {verb} did not respond within {}s — the operation was stopped",
                    timeout.as_secs()
                ));
            }
            Err(err) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(format!("wait ducktape-node {verb}: {err}"));
            }
        }
    };

    if let Some(writer) = stdin_writer {
        let _ = writer.join();
    }
    let stdout = out_reader.join().unwrap_or(Capture {
        bytes: Vec::new(),
        truncated: false,
        read_failed: true,
    });
    let stderr = err_reader.join().unwrap_or(Capture {
        bytes: Vec::new(),
        truncated: false,
        read_failed: true,
    });
    let status = status?;
    if stdout.read_failed || stderr.read_failed {
        return Err(format!("could not read ducktape-node {verb} output"));
    }
    if stdout.truncated || stderr.truncated {
        return Err(format!(
            "ducktape-node {verb} output exceeded the {MAX_VERB_OUTPUT_BYTES}-byte safety limit"
        ));
    }
    if !status.success() {
        let detail = String::from_utf8_lossy(&stderr.bytes);
        let detail = detail.trim();
        let contains_secret = secret_lines.iter().any(|secret| {
            !secret.is_empty()
                && (detail.contains(secret)
                    || secret
                        .split_whitespace()
                        .filter(|part| part.chars().count() >= 3)
                        .any(|part| detail.contains(part)))
        });
        return Err(if detail.is_empty() {
            format!("ducktape-node {verb} exited {status}")
        } else if contains_secret {
            format!("ducktape-node {verb} rejected the operation (detail redacted)")
        } else {
            detail.to_string()
        });
    }
    Ok(String::from_utf8_lossy(&stdout.bytes).trim().to_string())
}

/// The last non-empty stdout line — the machine-readable datum emitted by the
/// node's CLI verbs.
pub(crate) fn last_line(stdout: &str) -> String {
    stdout
        .lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())
        .unwrap_or("")
        .to_string()
}

/// Start the one long-lived node for a workspace. This is the only non-verb
/// execution path and it too lives behind [`NodeControl`].
pub(crate) fn spawn_workspace_node(
    config_path: &Path,
    log_path: &Path,
    ready_port: Option<u16>,
) -> Result<Child, String> {
    let spawn = || -> Result<Child, SpawnFailure> {
        let binary = resolve_node_bin().map_err(|reason| SpawnFailure {
            reason,
            log_tail: String::new(),
        })?;
        let log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .map_err(|err| SpawnFailure {
                reason: format!("open {log_path:?}: {err}"),
                log_tail: String::new(),
            })?;
        let log_err = log.try_clone().map_err(|err| SpawnFailure {
            reason: format!("clone {log_path:?}: {err}"),
            log_tail: String::new(),
        })?;
        let mut command = Command::new(binary);
        command
            .arg("--config")
            .arg(config_path)
            .stdin(Stdio::null())
            .stdout(log)
            .stderr(log_err);
        prepare_node_command_env(&mut command);
        detach(&mut command);
        spawn_verified(command, log_path, ready_port)
    };
    spawn().map_err(|failure| failure.to_string())
}

/// Best-effort display value for the diagnostics UI. Keeping binary resolution
/// private to this module means no command module can accidentally grow a
/// second execution path around the node-control boundary.
pub(crate) fn node_binary_display() -> Option<String> {
    resolve_node_bin()
        .ok()
        .map(|path| path.display().to_string())
}

/// resolve the networked `ducktape-node` binary path for the workspace flow:
/// the `DUCKTAPE_NODE_BIN` override, else the sibling next to this executable.
/// There is deliberately no PATH lookup or legacy-daemon fallback: the
/// frontend cannot select a program, and workspaces always run the bundled
/// network-shape node (real identity, real descriptor).
fn resolve_node_bin() -> Result<PathBuf, String> {
    if let Some(explicit) = env_node_bin()? {
        return Ok(explicit);
    }
    let exe = std::env::current_exe().map_err(|err| err.to_string())?;
    let dir = exe
        .parent()
        .ok_or_else(|| "app executable has no parent dir".to_string())?;
    let sibling = dir.join(format!("ducktape-node{}", std::env::consts::EXE_SUFFIX));
    if let Ok(validated) = validate_node_bin(&sibling) {
        return Ok(validated);
    }
    Err(format!(
        "no trusted, executable ducktape-node at {} — stage it with `bun run sidecar` \
         (or `make sidecar` at the repo root), or set DUCKTAPE_NODE_BIN",
        sibling.display()
    ))
}

/// the `DUCKTAPE_NODE_BIN` override, trimmed and validated: `Ok(None)` when
/// unset or empty (fall through to the sibling), `Ok(Some(path))` when it names
/// a usable binary, and an actionable `Err` when it is set but points at a
/// missing / empty / non-executable file. that last case is the dev trap the
/// old code walked straight into: `tauri dev`'s build.rs can leave a 0-byte
/// placeholder at the pinned path, and spawning it fails with a baffling
/// `Exec format error` / `Permission denied` far from the cause. reject it here
/// with a message that names the fix, and never silently substitute the sibling
/// for an explicitly-pinned (if broken) path.
fn env_node_bin() -> Result<Option<PathBuf>, String> {
    let raw = match std::env::var("DUCKTAPE_NODE_BIN") {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let path = PathBuf::from(trimmed);
    validate_node_bin(&path).map(Some).map_err(|reason| {
        format!(
            "DUCKTAPE_NODE_BIN={trimmed} is not a trusted executable ({reason}) — \
             run `cargo build -p node-bin`, wait for the rebuild to finish, or unset it"
        )
    })
}

/// Resolve symlinks once and validate the target before execution. In addition
/// to the historical present/non-empty/executable checks, reject a target that
/// another OS user can replace in place or swap through a writable ancestor
/// directory. Sticky shared directories (for example `/tmp`) remain valid
/// because their ownership rule prevents another user replacing our entry.
/// Same-user replacement is outside this boundary (that user can already
/// modify `~/.ducktape` and the running app).
fn validate_node_bin(path: &Path) -> Result<PathBuf, String> {
    let canonical =
        fs::canonicalize(path).map_err(|err| format!("resolve {}: {err}", path.display()))?;
    let meta = fs::metadata(&canonical)
        .map_err(|err| format!("inspect {}: {err}", canonical.display()))?;
    if !meta.is_file() || meta.len() == 0 {
        return Err("file is missing, empty, or not regular".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

        // SAFETY: `geteuid` has no preconditions and does not retain pointers.
        let effective_uid = unsafe { libc::geteuid() };
        // SAFETY: `getegid` has the same no-precondition value-return contract.
        let effective_gid = unsafe { libc::getegid() };
        let owner_is_trusted = |uid| uid == 0 || uid == effective_uid;
        if !owner_is_trusted(meta.uid()) {
            return Err("file is not owned by the current user or root".into());
        }
        let mode = meta.permissions().mode();
        if mode & 0o111 == 0 {
            return Err("file is not executable".into());
        }
        let trusted_primary_group = meta.uid() == effective_uid && meta.gid() == effective_gid;
        if mode & 0o002 != 0 || (mode & 0o020 != 0 && !trusted_primary_group) {
            return Err("file is writable by an untrusted group or other users".into());
        }
        for ancestor in canonical.ancestors().skip(1) {
            let ancestor_meta = fs::metadata(ancestor)
                .map_err(|err| format!("inspect {}: {err}", ancestor.display()))?;
            if !owner_is_trusted(ancestor_meta.uid()) {
                return Err(format!(
                    "ancestor directory {} is not owned by the current user or root",
                    ancestor.display()
                ));
            }
            let ancestor_mode = ancestor_meta.permissions().mode();
            let trusted_primary_group =
                ancestor_meta.uid() == effective_uid && ancestor_meta.gid() == effective_gid;
            let replaceable = ancestor_mode & 0o002 != 0
                || (ancestor_mode & 0o020 != 0 && !trusted_primary_group);
            if replaceable && ancestor_mode & 0o1000 == 0 {
                return Err(format!(
                    "ancestor directory {} is writable by an untrusted group or other users",
                    ancestor.display()
                ));
            }
        }
    }
    #[cfg(windows)]
    {
        let is_exe = canonical
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"));
        if !is_exe {
            return Err("file does not have an .exe extension".into());
        }
    }
    Ok(canonical)
}

/// macOS GUI apps inherit launchd's sparse PATH (`/usr/bin:/bin:/usr/sbin:/sbin`),
/// not the user's shell PATH. Without repair, a desktop-spawned node cannot
/// discover agent executors installed under Homebrew, ~/.local/bin, or NVM.
fn prepare_node_command_env(cmd: &mut Command) {
    if let Some(path) = node_launch_path(std::env::var_os("PATH"), std::env::var_os("HOME")) {
        cmd.env("PATH", path);
    }
}

fn node_launch_path(current: Option<OsString>, home: Option<OsString>) -> Option<OsString> {
    let mut dirs: Vec<PathBuf> = current
        .as_deref()
        .map(std::env::split_paths)
        .into_iter()
        .flatten()
        .collect();

    for dir in [
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/opt/homebrew/sbin"),
        PathBuf::from("/usr/local/bin"),
    ] {
        add_existing_path_dir(&mut dirs, dir);
    }

    if let Some(home) = home {
        let home = PathBuf::from(home);
        for rel in [
            ".local/bin",
            ".cargo/bin",
            ".bun/bin",
            ".volta/bin",
            ".asdf/shims",
            ".nodenv/shims",
        ] {
            add_existing_path_dir(&mut dirs, home.join(rel));
        }
        add_nvm_node_bins(&mut dirs, &home);
    }

    if dirs.is_empty() {
        None
    } else {
        std::env::join_paths(dirs).ok()
    }
}

fn add_nvm_node_bins(dirs: &mut Vec<PathBuf>, home: &Path) {
    let versions = home.join(".nvm/versions/node");
    let Ok(entries) = fs::read_dir(versions) else {
        return;
    };
    let mut bins: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("bin"))
        .filter(|path| path.is_dir())
        .collect();
    bins.sort();
    for bin in bins {
        add_existing_path_dir(dirs, bin);
    }
}

fn add_existing_path_dir(dirs: &mut Vec<PathBuf>, dir: PathBuf) {
    if dir.is_dir() && !dirs.iter().any(|existing| existing == &dir) {
        dirs.push(dir);
    }
}

/// A verified-spawn failure: the node forked (or failed to) but did not survive
/// the grace window, carrying a human reason and the tail of `daemon.log` so the
/// real cause — a bind conflict, a config parse error, a boot panic — reaches
/// the caller instead of a silent `Ok`.
#[derive(Debug)]
struct SpawnFailure {
    reason: String,
    log_tail: String,
}

impl std::fmt::Display for SpawnFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.log_tail.trim().is_empty() {
            write!(f, "{}", self.reason)
        } else {
            write!(f, "{}\n{}", self.reason, self.log_tail.trim_end())
        }
    }
}

/// Spawn `cmd` detached, then VERIFY the child did not die within a short grace
/// window (~1.5s) — the missing check that lets a node crash on boot yet report
/// success. `ready_port`, when `Some`, is polled as a fast success signal (the
/// member/founder http surface); a joiner passes `None` — it serves no http
/// while parked, so "still alive after the grace" is the only available signal.
/// On an immediate exit the last lines of `log_path` are read back into the
/// error. The initial spawn is retried a few times on ETXTBSY/ENOEXEC — a
/// hot-reload rewriting the binary as we exec it.
fn spawn_verified(
    mut cmd: Command,
    log_path: &Path,
    ready_port: Option<u16>,
) -> Result<Child, SpawnFailure> {
    let tail = || crate::workspaces::read_tail(log_path, 8 * 1024).unwrap_or_default();

    // exec, tolerating the hot-reload binary-rewrite race for a few tries.
    let mut child = {
        let mut attempt = 0;
        loop {
            match cmd.spawn() {
                Ok(child) => break child,
                Err(err) => {
                    // 26 = ETXTBSY (binary open for writing), 8 = ENOEXEC
                    // (half-written) — both transient during a hot rebuild.
                    let transient = matches!(err.raw_os_error(), Some(26) | Some(8));
                    if transient && attempt < 3 {
                        attempt += 1;
                        sleep(Duration::from_millis(100));
                        continue;
                    }
                    return Err(SpawnFailure {
                        reason: format!("could not spawn {:?}: {err}", cmd.get_program()),
                        log_tail: tail(),
                    });
                }
            }
        }
    };

    // grace window: catch a node that dies milliseconds after fork.
    let deadline = Instant::now() + Duration::from_millis(1500);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return Err(SpawnFailure {
                    reason: format!("the node exited on start ({status})"),
                    log_tail: tail(),
                });
            }
            Ok(None) => {} // still alive
            Err(err) => {
                return Err(SpawnFailure {
                    reason: format!("could not check the node process: {err}"),
                    log_tail: tail(),
                });
            }
        }
        if ready_port.is_some_and(crate::workspaces::port_listening) {
            return Ok(child); // confirmed serving its port
        }
        if Instant::now() >= deadline {
            return Ok(child); // alive but not yet serving — let the caller poll
        }
        sleep(Duration::from_millis(50));
    }
}

#[cfg(unix)]
fn detach(cmd: &mut Command) {
    use std::os::unix::process::CommandExt as _;
    // own process group: terminal signals aimed at the app never reach it
    cmd.process_group(0);
}

#[cfg(windows)]
fn detach(cmd: &mut Command) {
    use std::os::windows::process::CommandExt as _;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("dt-daemon-{tag}-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn node_control_uses_one_dedicated_thread_and_survives_a_job_panic() {
        let control = NodeControl::new().expect("actor");
        let caller = std::thread::current().id();
        let first = control
            .run_blocking(|| Ok(std::thread::current().id()))
            .expect("first job");
        let second =
            tauri::async_runtime::block_on(control.run(|| Ok(std::thread::current().id())))
                .expect("async job");
        assert_ne!(first, caller, "jobs must not run on the caller thread");
        assert_eq!(first, second, "one actor thread must serialize all jobs");

        let err = control
            .run_blocking::<(), _>(|| panic!("sentinel panic detail"))
            .expect_err("panic must become an error");
        assert_eq!(err, "node-control operation panicked");
        assert!(!err.contains("sentinel"), "panic payload crossed boundary");
        assert_eq!(control.run_blocking(|| Ok(7)).expect("actor recovered"), 7);
    }

    #[test]
    fn control_jobs_skip_expired_and_cancelled_operations() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};

        let expired_ran = Arc::new(AtomicBool::new(false));
        let ran = expired_ran.clone();
        let (reply, mut result) = tauri::async_runtime::channel(1);
        let job = control_job_with_max_age(
            move || {
                ran.store(true, Ordering::SeqCst);
                Ok(())
            },
            reply,
            Duration::ZERO,
        );
        job();
        let err = result
            .blocking_recv()
            .expect("expiry result")
            .expect_err("job must expire");
        assert!(err.contains("expired while waiting"), "{err}");
        assert!(!expired_ran.load(Ordering::SeqCst));

        let cancelled_ran = Arc::new(AtomicBool::new(false));
        let ran = cancelled_ran.clone();
        let (reply, result) = tauri::async_runtime::channel::<Result<(), String>>(1);
        drop(result);
        let job = control_job_with_max_age(
            move || {
                ran.store(true, Ordering::SeqCst);
                Ok(())
            },
            reply,
            Duration::from_secs(1),
        );
        job();
        assert!(!cancelled_ran.load(Ordering::SeqCst));
    }

    #[test]
    fn node_control_applies_bounded_backpressure() {
        let control = NodeControl::new().expect("actor");
        let (started_tx, started_rx) = std::sync::mpsc::sync_channel(1);
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
        control
            .enqueue(Box::new(move || {
                started_tx.send(()).expect("report start");
                release_rx.recv().expect("release actor");
            }))
            .expect("blocking job");
        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("actor started job");

        for _ in 0..CONTROL_QUEUE_CAPACITY {
            control.enqueue(Box::new(|| {})).expect("queue slot");
        }
        let err = control
            .enqueue(Box::new(|| {}))
            .expect_err("one job beyond capacity must be rejected");
        assert!(err.contains("queue is full"), "unexpected error: {err}");
        release_tx.send(()).expect("release actor");
    }

    #[test]
    fn invocation_limits_reject_oversize_and_delimiter_injection() {
        assert!(validate_invocation(&[], &[]).is_err());
        assert!(validate_invocation(&["not-a-node-verb"], &[]).is_err());
        assert!(validate_invocation(&["invite"], &["unexpected secret"]).is_err());
        let mut too_many = vec!["arg"; MAX_VERB_ARGS + 1];
        too_many[0] = "invite";
        assert!(validate_invocation(&too_many, &[]).is_err());

        let oversized_arg = "x".repeat(MAX_VERB_ARG_BYTES + 1);
        assert!(validate_invocation(&["invite", &oversized_arg], &[]).is_err());
        assert!(validate_invocation(&["invite", "bad\0arg"], &[]).is_err());

        let oversized_stdin = "x".repeat(MAX_VERB_STDIN_BYTES);
        assert!(validate_invocation(&["user-key"], &[&oversized_stdin]).is_err());
        for injected in ["secret\0suffix", "secret\nsecond-field", "secret\rfield"] {
            assert!(validate_invocation(&["user-key"], &[injected]).is_err());
        }
        assert!(validate_invocation(&["user-key"], &["bounded secret"]).is_ok());
    }

    #[test]
    fn output_capture_is_bounded_but_drains_the_source() {
        let exact = drain_capped(std::io::Cursor::new(vec![b'x'; MAX_VERB_OUTPUT_BYTES]));
        assert_eq!(exact.bytes.len(), MAX_VERB_OUTPUT_BYTES);
        assert!(!exact.truncated && !exact.read_failed);

        let extra = drain_capped(std::io::Cursor::new(vec![b'x'; MAX_VERB_OUTPUT_BYTES + 17]));
        assert_eq!(extra.bytes.len(), MAX_VERB_OUTPUT_BYTES);
        assert!(extra.truncated && !extra.read_failed);
    }

    #[cfg(unix)]
    #[test]
    fn spawn_verified_catches_insta_death_with_log_tail() {
        let log = scratch("insta").join("daemon.log");
        fs::write(&log, b"boom: bind 127.0.0.1:8844: address already in use\n").unwrap();
        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c").arg("exit 3");
        let err = spawn_verified(cmd, &log, None).expect_err("insta-exit must be an error");
        assert!(err.reason.contains("exited"), "reason: {}", err.reason);
        assert!(err.log_tail.contains("boom"), "tail: {}", err.log_tail);
    }

    #[cfg(unix)]
    #[test]
    fn spawn_verified_ok_for_a_live_child() {
        let log = scratch("live").join("daemon.log");
        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c").arg("sleep 5");
        let mut child = spawn_verified(cmd, &log, None).expect("a still-alive child is Ok");
        child.kill().ok();
        child.wait().ok();
    }

    #[cfg(unix)]
    #[test]
    fn verb_deadline_kills_and_reaps_a_wedged_child() {
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "exec sleep 5"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let child = command.spawn().expect("wedged child");
        let started = Instant::now();
        let err =
            wait_for_verb_with_timeout("test-timeout", child, None, &[], Duration::from_millis(75))
                .expect_err("deadline must fail");
        assert!(err.contains("operation was stopped"), "{err}");
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "deadline failed to stop promptly"
        );
    }

    #[cfg(unix)]
    #[test]
    fn verb_failure_never_reflects_secret_stdin_content() {
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "printf 'bad hunter2-secret' >&2; exit 1"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let child = command.spawn().expect("failing child");
        let err = wait_for_verb_with_timeout(
            "user-key",
            child,
            None,
            &["hunter2-secret"],
            Duration::from_secs(1),
        )
        .expect_err("child must fail");
        assert!(err.contains("detail redacted"), "{err}");
        assert!(!err.contains("hunter2"), "secret reflected in error: {err}");
    }

    #[cfg(unix)]
    #[test]
    fn node_launch_path_adds_developer_bins_for_gui_spawn() {
        let home = scratch("node-path-home");
        let nvm_bin = home.join(".nvm/versions/node/v24.13.1/bin");
        let local_bin = home.join(".local/bin");
        fs::create_dir_all(&nvm_bin).unwrap();
        fs::create_dir_all(&local_bin).unwrap();

        let path = node_launch_path(
            Some(std::ffi::OsString::from("/usr/bin:/bin:/usr/sbin:/sbin")),
            Some(home.as_os_str().to_os_string()),
        )
        .expect("path");
        let dirs: Vec<PathBuf> = std::env::split_paths(&path).collect();

        assert!(dirs.contains(&PathBuf::from("/usr/bin")));
        assert!(dirs.contains(&PathBuf::from("/bin")));
        assert!(dirs.contains(&local_bin));
        assert!(dirs.contains(&nvm_bin));

        fs::remove_dir_all(&home).ok();
    }

    #[cfg(unix)]
    #[test]
    fn validation_accepts_private_group_and_rejects_world_writes() {
        use std::os::unix::fs::PermissionsExt as _;

        let path = scratch("permissions").join("ducktape-node");
        fs::write(&path, b"#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o775)).unwrap();
        assert_eq!(
            validate_node_bin(&path).expect("owner-controlled executable"),
            fs::canonicalize(&path).unwrap()
        );

        fs::set_permissions(&path, fs::Permissions::from_mode(0o757)).unwrap();
        let err = validate_node_bin(&path).expect_err("world-writable executable");
        assert!(err.contains("untrusted group or other"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn validation_rejects_a_replaceable_parent_directory() {
        use std::os::unix::fs::PermissionsExt as _;

        let parent = scratch("replaceable-parent");
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o777)).unwrap();
        let path = parent.join("ducktape-node");
        fs::write(&path, b"#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        let err = validate_node_bin(&path).expect_err("replaceable parent");
        assert!(err.contains("ancestor directory"), "{err}");
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn validation_rejects_empty_and_zero_byte() {
        assert!(
            validate_node_bin(&PathBuf::from("")).is_err(),
            "empty path is not usable"
        );
        let zero = scratch("zero").join("node");
        fs::write(&zero, b"").unwrap();
        assert!(
            validate_node_bin(&zero).is_err(),
            "a 0-byte file is not usable"
        );
    }
}
