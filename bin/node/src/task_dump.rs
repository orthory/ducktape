//! SIGUSR1 task dump: when a validator wedges, nothing in the tree can say
//! which async task it is parked on. tokio's unstable taskdump API answers
//! that with no new dependency (`--cfg tokio_unstable`, wired in
//! `.cargo/config.toml`, Linux x86_64/aarch64 only). The signal handler
//! itself is installed in `validator/run.rs`, next to SIGTERM/SIGINT; this
//! module holds only the dump-and-write step so that call site stays a
//! one-line delegation.
//!
//! The dump is never logged into the ring — a wedged node can be dumping
//! hundreds of tasks, and the ring is a 4096-line window other diagnostics
//! need. It goes straight to `<workspace>/tasks.txt`, overwritten each time.

#[cfg(all(
    tokio_unstable,
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
pub(crate) async fn dump_tasks(workspace: &std::path::Path, label: &str) {
    use std::io::Write as _;

    let path = workspace.join("tasks.txt");
    let dump = match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::runtime::Handle::current().dump(),
    )
    .await
    {
        Ok(dump) => dump,
        Err(_) => {
            tracing::warn!(
                target: "ducktape::node",
                node = %label,
                reason = "task_dump_timeout",
                "SIGUSR1 task dump timed out"
            );
            return;
        }
    };

    let tasks: Vec<_> = dump.tasks().iter().collect();
    let write_result = (|| -> std::io::Result<()> {
        let mut file = std::fs::File::create(&path)?;
        for task in &tasks {
            writeln!(file, "{}", task.trace())?;
            writeln!(file)?;
        }
        Ok(())
    })();

    if let Err(e) = write_result {
        tracing::warn!(
            target: "ducktape::node",
            node = %label,
            error = %e,
            reason = "task_dump_write_failed",
            "SIGUSR1 task dump could not be written"
        );
        return;
    }

    tracing::warn!(
        target: "ducktape::node",
        node = %label,
        event = "task_dump_written",
        tasks = tasks.len(),
        path = %path.display(),
        "SIGUSR1 task dump written"
    );
}

/// non-Linux / stable-tokio builds: no taskdump support. Called once at
/// boot so the operator knows why a `kill -USR1` did nothing.
#[cfg(not(all(
    tokio_unstable,
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
)))]
pub(crate) fn log_unsupported(label: &str) {
    tracing::debug!(
        target: "ducktape::node",
        node = %label,
        reason = "task_dump_unsupported",
        "SIGUSR1 task dump not installed on this target"
    );
}
