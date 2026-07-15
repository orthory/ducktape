//! Bounded serialization boundary for blocking node and key operations.

use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write as _};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Once, OnceLock};
use std::thread::sleep;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, oneshot};
use zeroize::Zeroize as _;

use super::private_fs;

const CONTROL_QUEUE_CAPACITY: usize = 32;
const CONTROL_MAX_QUEUE_AGE: Duration = Duration::from_secs(30);

type ControlJob = Box<dyn FnOnce() + Send + 'static>;

#[derive(Debug, Clone)]
pub(super) struct NodeControl {
    queue: mpsc::Sender<ControlJob>,
    stopping: Arc<AtomicBool>,
}

impl NodeControl {
    pub(super) fn new() -> Result<Self, String> {
        install_control_panic_redaction();
        let runtime = tokio::runtime::Handle::try_current()
            .map_err(|_| "node-control must start inside the iced Tokio executor".to_string())?;
        let (queue, mut receiver) = mpsc::channel::<ControlJob>(CONTROL_QUEUE_CAPACITY);
        let stopping = Arc::new(AtomicBool::new(false));
        let worker_stopping = Arc::clone(&stopping);
        runtime.spawn_blocking(move || {
            while !worker_stopping.load(Ordering::Acquire) {
                let Some(job) = receiver.blocking_recv() else {
                    break;
                };
                job();
            }
        });
        Ok(Self { queue, stopping })
    }

    pub(super) async fn run<T, F>(&self, operation: F) -> Result<T, String>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, String> + Send + 'static,
    {
        let (reply, result) = oneshot::channel();
        self.enqueue(control_job(operation, reply))?;
        result
            .await
            .unwrap_or_else(|_| Err("node-control actor stopped".to_string()))
    }

    #[allow(dead_code)]
    pub(super) fn run_blocking<T, F>(&self, operation: F) -> Result<T, String>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, String> + Send + 'static,
    {
        let (reply, result) = oneshot::channel();
        self.enqueue(control_job(operation, reply))?;
        result
            .blocking_recv()
            .unwrap_or_else(|_| Err("node-control actor stopped".to_string()))
    }

    fn enqueue(&self, job: ControlJob) -> Result<(), String> {
        if self.stopping.load(Ordering::Acquire) {
            return Err("node-control is shutting down".to_string());
        }
        self.queue.try_send(job).map_err(|_| {
            "node-control queue is full or unavailable — wait for the current operation to finish"
                .to_string()
        })
    }

    pub(super) fn shutdown(&self) {
        if self.stopping.swap(true, Ordering::AcqRel) {
            return;
        }
        // Wake an idle blocking receiver. If the queue is full, it is already
        // awake and will observe `stopping` after its current bounded job.
        let _ = self.queue.try_send(Box::new(|| {}));
    }
}

fn control_job<T, F>(operation: F, reply: oneshot::Sender<Result<T, String>>) -> ControlJob
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    control_job_with_max_age(operation, reply, CONTROL_MAX_QUEUE_AGE)
}

fn control_job_with_max_age<T, F>(
    operation: F,
    reply: oneshot::Sender<Result<T, String>>,
    max_age: Duration,
) -> ControlJob
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    let enqueued_at = Instant::now();
    Box::new(move || {
        if reply.is_closed() {
            return;
        }
        let outcome = if enqueued_at.elapsed() >= max_age {
            Err("node-control operation expired while waiting in the queue".to_string())
        } else {
            match catch_control_panic(operation) {
                Ok(outcome) => outcome,
                Err(_) => Err("node-control operation panicked".to_string()),
            }
        };
        let _ = reply.send(outcome);
    })
}

thread_local! {
    static REDACT_CONTROL_PANIC: Cell<bool> = const { Cell::new(false) };
}

fn install_control_panic_redaction() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if !REDACT_CONTROL_PANIC.with(Cell::get) {
                previous(info);
            }
        }));
    });
}

fn catch_control_panic<T>(operation: impl FnOnce() -> T) -> std::thread::Result<T> {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            REDACT_CONTROL_PANIC.with(|flag| flag.set(false));
        }
    }

    REDACT_CONTROL_PANIC.with(|flag| flag.set(true));
    let _reset = Reset;
    catch_unwind(AssertUnwindSafe(operation))
}

const VERB_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_VERB_OUTPUT_BYTES: usize = 1024 * 1024;
const MAX_VERB_ARG_BYTES: usize = 1024 * 1024;
const MAX_VERB_STDIN_BYTES: usize = 64 * 1024;
const MAX_VERB_ARGS: usize = 64;
const ALLOWED_VERBS: &[&str] = &[
    "init",
    "keygen",
    "join",
    "invite",
    "join-requests",
    "join-state",
    "member-status",
    "user-key",
    "user-sign-bind",
    "user-sign-unbind",
    "user-sign-possession",
    "user-sign-add-member",
    "user-sign-remove-member",
    "user-sign-gateway-route",
    "user-sign-frame",
    "user-sign-admin",
    "gateway-route-bind",
    "gateway-route-unbind",
    "gateway-route-list",
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
            Ok(count) => {
                let keep = MAX_VERB_OUTPUT_BYTES.saturating_sub(bytes.len()).min(count);
                bytes.extend_from_slice(&chunk[..keep]);
                truncated |= keep != count;
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
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
                | "user-sign-gateway-route"
                | "user-sign-frame"
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

#[allow(dead_code)]
pub(super) fn run_verb(args: &[&str]) -> Result<String, String> {
    run_verb_inner(args, &[])
}

#[allow(dead_code)]
pub(super) fn run_verb_with_stdin(args: &[&str], stdin_lines: &[&str]) -> Result<String, String> {
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
    prepare_private_child(&mut command);
    let mut child = command
        .spawn()
        .map_err(|error| format!("run ducktape-node {verb}: {error}"))?;

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
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("start ducktape-node {verb} stdin writer: {error}"));
            }
        }
    };
    wait_for_verb_with_timeout(verb, child, stdin_writer, stdin_lines, VERB_TIMEOUT)
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
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            if let Some(writer) = stdin_writer {
                let _ = writer.join();
            }
            return Err(format!("start ducktape-node {verb} stdout reader: {error}"));
        }
    };
    let err_reader = match std::thread::Builder::new()
        .name("node-verb-stderr".into())
        .spawn(move || drain_capped(stderr))
    {
        Ok(reader) => reader,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            if let Some(writer) = stdin_writer {
                let _ = writer.join();
            }
            let _ = out_reader.join();
            return Err(format!("start ducktape-node {verb} stderr reader: {error}"));
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
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(format!("wait ducktape-node {verb}: {error}"));
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

#[allow(dead_code)]
pub(super) fn last_line(stdout: &str) -> String {
    stdout
        .lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())
        .unwrap_or("")
        .to_string()
}

fn resolve_node_bin() -> Result<PathBuf, String> {
    resolve_external_bin("DUCKTAPE_NODE_BIN", "ducktape-node")
}

fn resolve_external_bin(variable: &str, binary: &str) -> Result<PathBuf, String> {
    if let Ok(raw) = std::env::var(variable) {
        let value = raw.trim();
        if !value.is_empty() {
            return validate_external_bin(Path::new(value)).map_err(|reason| {
                format!(
                    "{variable}={value} is not a trusted executable ({reason}) — run `make sidecar`, wait for the {binary} rebuild to finish, or unset it"
                )
            });
        }
    }
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let dir = executable
        .parent()
        .ok_or_else(|| "app executable has no parent directory".to_string())?;
    let sibling = dir.join(format!("{binary}{}", std::env::consts::EXE_SUFFIX));
    validate_external_bin(&sibling).map_err(|_| {
        format!(
            "no trusted, executable {binary} at {} — stage the native sidecar with `make sidecar` at the repo root, or set {variable}",
            sibling.display()
        )
    })
}

#[cfg(unix)]
fn dir_replaceable_by_others(uid: u32, gid: u32, mode: u32, euid: u32, egid: u32) -> bool {
    if mode & 0o1000 != 0 {
        return false;
    }
    if mode & 0o002 != 0 {
        return true;
    }
    if mode & 0o020 == 0 {
        return false;
    }
    let trusted_primary_group = uid == euid && gid == egid;
    #[cfg(target_os = "macos")]
    let macos_system_admin_dir = uid == 0 && gid == 80;
    #[cfg(not(target_os = "macos"))]
    let macos_system_admin_dir = false;
    !(trusted_primary_group || macos_system_admin_dir)
}

fn validate_external_bin(path: &Path) -> Result<PathBuf, String> {
    let canonical =
        fs::canonicalize(path).map_err(|error| format!("resolve {}: {error}", path.display()))?;
    let metadata = fs::metadata(&canonical)
        .map_err(|error| format!("inspect {}: {error}", canonical.display()))?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err("file is missing, empty, or not regular".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

        // SAFETY: these calls return process credentials and retain no pointers.
        let effective_uid = unsafe { libc::geteuid() };
        // SAFETY: same value-returning contract as `geteuid`.
        let effective_gid = unsafe { libc::getegid() };
        let owner_is_trusted = |uid| uid == 0 || uid == effective_uid;
        if !owner_is_trusted(metadata.uid()) {
            return Err("file is not owned by the current user or root".into());
        }
        let mode = metadata.permissions().mode();
        if mode & 0o111 == 0 {
            return Err("file is not executable".into());
        }
        let trusted_primary_group =
            metadata.uid() == effective_uid && metadata.gid() == effective_gid;
        if mode & 0o002 != 0 || (mode & 0o020 != 0 && !trusted_primary_group) {
            return Err("file is writable by an untrusted group or other users".into());
        }
        for ancestor in canonical.ancestors().skip(1) {
            let metadata = fs::metadata(ancestor)
                .map_err(|error| format!("inspect {}: {error}", ancestor.display()))?;
            if !owner_is_trusted(metadata.uid()) {
                return Err(format!(
                    "ancestor directory {} is not owned by the current user or root",
                    ancestor.display()
                ));
            }
            if dir_replaceable_by_others(
                metadata.uid(),
                metadata.gid(),
                metadata.permissions().mode(),
                effective_uid,
                effective_gid,
            ) {
                return Err(format!(
                    "ancestor directory {} is writable by an untrusted group or other users",
                    ancestor.display()
                ));
            }
        }
    }
    #[cfg(windows)]
    if !canonical
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
    {
        return Err("file does not have an .exe extension".into());
    }
    Ok(canonical)
}

fn prepare_node_command_env(command: &mut Command) {
    if let Some(path) = node_launch_path(std::env::var_os("PATH"), std::env::var_os("HOME")) {
        command.env("PATH", path);
    }
    if let Some(filter) = std::env::var_os("RUST_LOG") {
        command.env("RUST_LOG", filter);
    }
}

fn prepare_private_child(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;

        // SAFETY: `umask` only updates the soon-to-exec child process and is
        // async-signal-safe. The parent process's global umask is untouched.
        unsafe {
            command.pre_exec(|| {
                libc::umask(0o077);
                Ok(())
            });
        }
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
        for relative in [
            ".local/bin",
            ".cargo/bin",
            ".bun/bin",
            ".volta/bin",
            ".asdf/shims",
            ".nodenv/shims",
        ] {
            add_existing_path_dir(&mut dirs, home.join(relative));
        }
        let versions = home.join(".nvm/versions/node");
        if let Ok(entries) = fs::read_dir(versions) {
            let mut bins: Vec<PathBuf> = entries
                .filter_map(Result::ok)
                .map(|entry| entry.path().join("bin"))
                .filter(|path| path.is_dir())
                .collect();
            bins.sort();
            for bin in bins {
                add_existing_path_dir(&mut dirs, bin);
            }
        }
    }
    (!dirs.is_empty())
        .then(|| std::env::join_paths(dirs).ok())
        .flatten()
}

fn add_existing_path_dir(dirs: &mut Vec<PathBuf>, dir: PathBuf) {
    if dir.is_dir() && !dirs.iter().any(|existing| existing == &dir) {
        dirs.push(dir);
    }
}

const MAX_AUTO_RESTARTS: u32 = 5;
const RESTART_BACKOFF: Duration = Duration::from_secs(2);
const LOG_ROLL_BYTES: u64 = 32 * 1024 * 1024;

#[allow(dead_code)]
pub(super) fn spawn_workspace_node(
    config_path: &Path,
    log_path: &Path,
    ready_port: Option<u16>,
) -> Result<Child, String> {
    let spawn = || -> Result<Child, SpawnFailure> {
        let binary = resolve_node_bin().map_err(|reason| SpawnFailure {
            reason,
            log_tail: String::new(),
        })?;
        roll_if_oversized(log_path);
        let log = private_fs::open_private_append(log_path).map_err(|error| SpawnFailure {
            reason: format!("open {log_path:?}: {error}"),
            log_tail: String::new(),
        })?;
        let log_err = log.try_clone().map_err(|error| SpawnFailure {
            reason: format!("clone {log_path:?}: {error}"),
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
        prepare_private_child(&mut command);
        detach(&mut command);
        spawn_verified(command, log_path, ready_port)
    };
    spawn().map_err(|failure| failure.to_string())
}

#[allow(dead_code)]
pub(super) fn node_binary_display() -> Option<String> {
    node_binary_path().map(|path| path.display().to_string())
}

pub(super) fn node_binary_path() -> Option<PathBuf> {
    resolve_node_bin().ok()
}

#[allow(dead_code)]
pub(super) struct Supervisor {
    pub(super) config: PathBuf,
    pub(super) log: PathBuf,
    pub(super) http_port: u16,
    pub(super) listen_port: u16,
    pub(super) pidfile: PathBuf,
    pub(super) workspace: String,
    pub(super) stopping: Arc<AtomicBool>,
}

fn stop_flags() -> &'static Mutex<HashMap<PathBuf, Arc<AtomicBool>>> {
    static FLAGS: OnceLock<Mutex<HashMap<PathBuf, Arc<AtomicBool>>>> = OnceLock::new();
    FLAGS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_flags() -> std::sync::MutexGuard<'static, HashMap<PathBuf, Arc<AtomicBool>>> {
    stop_flags()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

#[allow(dead_code)]
pub(super) fn register_supervised(dir: &Path) -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));
    lock_flags().insert(dir.to_path_buf(), flag.clone());
    flag
}

#[allow(dead_code)]
pub(super) fn mark_stopping(dir: &Path) {
    if let Some(flag) = lock_flags().get(dir) {
        flag.store(true, Ordering::SeqCst);
    }
}

#[allow(dead_code)]
pub(super) fn clear_supervised(dir: &Path) {
    lock_flags().remove(dir);
}

#[allow(dead_code)]
pub(super) fn record_pid_if_alive(child: &mut Child, pidfile: &Path, workspace: &str) {
    match child.try_wait() {
        Ok(None) => {
            if let Err(error) = private_fs::write_atomic(pidfile, child.id().to_string().as_bytes())
            {
                tracing::warn!(
                    target: "ducktape::shell",
                    %workspace,
                    %error,
                    "could not record the node pid — teardown falls back to process discovery"
                );
            }
        }
        _ => tracing::warn!(
            target: "ducktape::shell",
            %workspace,
            pid = child.id(),
            "spawned node already exited — not recording its pid"
        ),
    }
}

fn should_auto_restart(exit_code: Option<i32>, stopping: bool, restarts: u32) -> bool {
    exit_code != Some(0) && !stopping && restarts < MAX_AUTO_RESTARTS
}

#[allow(dead_code)]
pub(super) fn watch_node_exit(child: Child, supervisor: Supervisor) {
    watch_with_restarts(child, supervisor, 0);
}

fn watch_with_restarts(mut child: Child, supervisor: Supervisor, restarts: u32) {
    let pid = child.id();
    let started = Instant::now();
    let _ = std::thread::Builder::new()
        .name("node-reaper".into())
        .spawn(move || {
            let elapsed_s = || started.elapsed().as_secs();
            let workspace = supervisor.workspace.clone();
            match child.wait() {
                Ok(status) if status.code() == Some(0) => tracing::info!(
                    target: "ducktape::shell",
                    %workspace,
                    pid,
                    elapsed_s = elapsed_s(),
                    "the workspace node stopped cleanly"
                ),
                Ok(status) => {
                    let code = status.code();
                    tracing::warn!(
                        target: "ducktape::shell",
                        event = "workspace_node_exited",
                        reason = "nonzero_exit",
                        %workspace,
                        pid,
                        code = code.unwrap_or(-1),
                        elapsed_s = elapsed_s(),
                        "the workspace node exited; the bounded supervisor will decide whether to restart it"
                    );
                    maybe_restart(supervisor, restarts, code);
                }
                Err(error) => tracing::warn!(
                    target: "ducktape::shell",
                    %workspace,
                    pid,
                    %error,
                    elapsed_s = elapsed_s(),
                    "lost track of the workspace node"
                ),
            }
        });
}

fn maybe_restart(supervisor: Supervisor, restarts: u32, exit_code: Option<i32>) {
    let stopping = supervisor.stopping.load(Ordering::SeqCst);
    if !should_auto_restart(exit_code, stopping, restarts) {
        if stopping {
            tracing::info!(
                target: "ducktape::shell",
                workspace = %supervisor.workspace,
                "supervised node stop was requested — not restarting"
            );
        } else if restarts >= MAX_AUTO_RESTARTS {
            tracing::error!(
                target: "ducktape::shell",
                workspace = %supervisor.workspace,
                restarts,
                "gave up auto-restarting the workspace node — see daemon.log for the cause"
            );
        }
        return;
    }
    sleep(RESTART_BACKOFF);
    if supervisor.stopping.load(Ordering::SeqCst) {
        return;
    }
    if super::workspaces::port_listening(supervisor.listen_port)
        || super::workspaces::port_listening(supervisor.http_port)
    {
        tracing::error!(
            target: "ducktape::shell",
            event = "workspace_node_restart_refused",
            reason = "unverified_listener",
            workspace = %supervisor.workspace,
            "workspace node restart refused because an unverified process holds a configured port"
        );
        return;
    }
    match spawn_workspace_node(
        &supervisor.config,
        &supervisor.log,
        Some(supervisor.http_port),
    ) {
        Ok(mut child) => {
            record_pid_if_alive(&mut child, &supervisor.pidfile, &supervisor.workspace);
            tracing::warn!(
                target: "ducktape::shell",
                workspace = %supervisor.workspace,
                restart = restarts + 1,
                "auto-restarted the crashed workspace node"
            );
            watch_with_restarts(child, supervisor, restarts + 1);
        }
        Err(error) => tracing::error!(
            target: "ducktape::shell",
            workspace = %supervisor.workspace,
            %error,
            "auto-restart of the crashed workspace node failed"
        ),
    }
}

fn roll_if_oversized(log_path: &Path) {
    if fs::metadata(log_path).is_ok_and(|metadata| metadata.len() > LOG_ROLL_BYTES) {
        let _ = fs::rename(log_path, log_path.with_extension("log.1"));
    }
}

#[derive(Debug)]
struct SpawnFailure {
    reason: String,
    log_tail: String,
}

impl std::fmt::Display for SpawnFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.log_tail.trim().is_empty() {
            write!(formatter, "{}", self.reason)
        } else {
            write!(formatter, "{}\n{}", self.reason, self.log_tail.trim_end())
        }
    }
}

fn spawn_verified(
    mut command: Command,
    log_path: &Path,
    ready_port: Option<u16>,
) -> Result<Child, SpawnFailure> {
    let tail = || super::workspaces::read_tail(log_path, 8 * 1024).unwrap_or_default();
    let mut child = {
        let mut attempt = 0;
        loop {
            match command.spawn() {
                Ok(child) => break child,
                Err(error) => {
                    let transient = matches!(error.raw_os_error(), Some(26) | Some(8));
                    if transient && attempt < 3 {
                        attempt += 1;
                        sleep(Duration::from_millis(100));
                        continue;
                    }
                    return Err(SpawnFailure {
                        reason: format!("could not spawn {:?}: {error}", command.get_program()),
                        log_tail: tail(),
                    });
                }
            }
        }
    };
    let deadline = Instant::now() + Duration::from_millis(1_500);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return Err(SpawnFailure {
                    reason: format!("the node exited on start ({status})"),
                    log_tail: tail(),
                });
            }
            Ok(None) => {}
            Err(error) => {
                return Err(SpawnFailure {
                    reason: format!("could not check the node process: {error}"),
                    log_tail: tail(),
                });
            }
        }
        if ready_port.is_some_and(super::workspaces::port_listening) || Instant::now() >= deadline {
            return Ok(child);
        }
        sleep(Duration::from_millis(50));
    }
}

#[cfg(unix)]
fn detach(command: &mut Command) {
    use std::os::unix::process::CommandExt as _;
    command.process_group(0);
}

#[cfg(windows)]
fn detach(command: &mut Command) {
    use std::os::windows::process::CommandExt as _;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;

    #[tokio::test]
    async fn actor_serializes_jobs_and_survives_a_panic() {
        let control = NodeControl::new().unwrap();
        let caller = std::thread::current().id();
        let first = control
            .run(|| Ok(std::thread::current().id()))
            .await
            .unwrap();
        let second = control
            .run(|| Ok(std::thread::current().id()))
            .await
            .unwrap();
        assert_ne!(first, caller);
        assert_eq!(first, second);

        let error = control
            .run::<(), _>(|| panic!("secret panic payload"))
            .await
            .unwrap_err();
        assert_eq!(error, "node-control operation panicked");
        assert_eq!(control.run(|| Ok(7)).await.unwrap(), 7);
    }

    #[tokio::test]
    async fn shutdown_is_idempotent_and_rejects_new_jobs() {
        let control = NodeControl::new().unwrap();
        let clone = control.clone();

        control.shutdown();
        clone.shutdown();

        assert_eq!(
            clone.run(|| Ok(7)).await.unwrap_err(),
            "node-control is shutting down"
        );
    }

    #[test]
    fn stale_and_cancelled_jobs_do_not_run() {
        let ran = Arc::new(AtomicBool::new(false));
        let marker = ran.clone();
        let (reply, result) = oneshot::channel();
        let job = control_job_with_max_age(
            move || {
                marker.store(true, Ordering::SeqCst);
                Ok(())
            },
            reply,
            Duration::ZERO,
        );
        job();
        assert!(result.blocking_recv().unwrap().is_err());
        assert!(!ran.load(Ordering::SeqCst));

        let marker = ran.clone();
        let (reply, result) = oneshot::channel::<Result<(), String>>();
        drop(result);
        control_job_with_max_age(
            move || {
                marker.store(true, Ordering::SeqCst);
                Ok(())
            },
            reply,
            Duration::from_secs(1),
        )();
        assert!(!ran.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn actor_applies_bounded_backpressure() {
        let control = NodeControl::new().unwrap();
        let (started_tx, started_rx) = std::sync::mpsc::sync_channel(1);
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
        control
            .enqueue(Box::new(move || {
                started_tx.send(()).unwrap();
                release_rx.recv().unwrap();
            }))
            .unwrap();
        started_rx.recv().unwrap();
        for _ in 0..CONTROL_QUEUE_CAPACITY {
            control.enqueue(Box::new(|| {})).unwrap();
        }
        assert!(control.enqueue(Box::new(|| {})).is_err());
        release_tx.send(()).unwrap();
    }

    #[test]
    fn invocation_is_allowlisted_and_secret_stdin_is_bounded() {
        assert!(validate_invocation(&[], &[]).is_err());
        assert!(validate_invocation(&["shell", "rm", "-rf"], &[]).is_err());
        assert!(validate_invocation(&["init"], &["secret"]).is_err());
        assert!(validate_invocation(&["user-sign-frame"], &["secret"]).is_ok());
        assert!(validate_invocation(&["user-sign-frame"], &["bad\nline"]).is_err());
        let too_many = vec!["arg"; MAX_VERB_ARGS + 1];
        assert!(validate_invocation(&too_many, &[]).is_err());
    }

    #[test]
    fn restart_policy_only_revives_unexpected_uncapped_exits() {
        assert!(!should_auto_restart(Some(0), false, 0));
        assert!(should_auto_restart(Some(1), false, 0));
        assert!(should_auto_restart(None, false, MAX_AUTO_RESTARTS - 1));
        assert!(!should_auto_restart(None, true, 0));
        assert!(!should_auto_restart(Some(1), false, MAX_AUTO_RESTARTS));
    }

    #[cfg(unix)]
    #[test]
    fn external_binary_must_be_executable_and_not_other_writable() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = scratch("sidecar");
        let binary = dir.join("ducktape-node");
        fs::write(&binary, b"#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(
            validate_external_bin(&binary).unwrap(),
            fs::canonicalize(&binary).unwrap()
        );
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o707)).unwrap();
        assert!(validate_external_bin(&binary).is_err());
        fs::remove_dir_all(dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn verified_spawn_reports_immediate_exit_with_log_tail() {
        let dir = scratch("spawn");
        let log = dir.join("daemon.log");
        fs::write(&log, b"bind failed\n").unwrap();
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "exit 3"]);
        let error = spawn_verified(command, &log, None).unwrap_err();
        assert!(error.reason.contains("exited"));
        assert!(error.log_tail.contains("bind failed"));
        fs::remove_dir_all(dir).ok();
    }

    fn scratch(tag: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "ducktape-iced-node-{tag}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
