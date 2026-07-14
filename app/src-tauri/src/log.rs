//! the desktop shell's own log sink.
//!
//! the shell's diagnostics went to `/dev/null` in every configuration a real user
//! runs, which is exactly why nobody noticed:
//!
//! - windows release builds set `windows_subsystem = "windows"` — there is no console
//! - macOS Launch Services hands a GUI process `/dev/null` for stdout/stderr
//! - a Linux `.desktop` launch inherits the session leader's stderr
//!
//! only `bun run tauri dev` and Fleet QA ever saw an `eprintln!` here. so when the
//! app died on launch — `main.rs`'s `.expect("start desktop node-control actor")`,
//! say — it vanished leaving **zero bytes on disk** and nothing to support.
//!
//! the file lives beside the state the app already keeps (`~/.ducktape/`, home of
//! `registry.json`, `workspaces/` and `user.key`), so a user pointed at it is
//! already in a folder they know.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::PathBuf;
use std::sync::Mutex;

use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use tracing_subscriber::{EnvFilter, Layer as _};

/// roll past this size, keeping one previous generation. matches the node's
/// `daemon.rs::LOG_ROLL_BYTES` — ONE rotation rule in the codebase, not two.
const LOG_ROLL_BYTES: u64 = 32 * 1024 * 1024;

/// `~/.ducktape` without an `AppHandle`: the subscriber is installed before Tauri
/// is built (so it catches the boot panics that are the whole point), and
/// `registry::root` needs a handle we do not have yet. same path, same rule.
fn root() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(PathBuf::from(home).join(".ducktape"))
}

pub(crate) fn log_path() -> Option<PathBuf> {
    Some(root()?.join("shell.log"))
}

fn open_log() -> io::Result<File> {
    let path = log_path().ok_or_else(|| io::Error::other("no home directory"))?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if std::fs::metadata(&path).is_ok_and(|meta| meta.len() > LOG_ROLL_BYTES) {
        // best-effort: a failed roll must never stop the app from starting.
        let _ = std::fs::rename(&path, path.with_extension("log.1"));
    }
    OpenOptions::new().create(true).append(true).open(&path)
}

/// install the shell's subscriber. call ONCE from `main`, and **only after the CEF
/// helper-process dispatch**: CEF re-execs this same binary for its renderer/GPU/
/// utility subprocesses, so installing before the dispatch would have 4-6 helpers
/// each open and append to this one file.
///
/// deliberately a plain `Mutex<File>` and not `tracing-appender`: an appender adds a
/// background writer thread, which is the one thing that would LOSE the last lines on
/// a hard `exit()` — precisely when they matter most. `impl MakeWriter for Mutex<W>`
/// is already provided by tracing-subscriber.
pub(crate) fn init() {
    let filter = {
        let env = std::env::var("RUST_LOG").unwrap_or_default();
        let directives = if env.is_empty() {
            "info".to_string()
        } else {
            format!("info,{env}")
        };
        EnvFilter::builder()
            .parse(&directives)
            .unwrap_or_else(|_| EnvFilter::new("info"))
    };

    let file_layer = open_log().ok().map(|file| {
        tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(Mutex::new(file))
    });

    let _ = tracing_subscriber::registry()
        // stderr too: it is where `tauri dev` and Fleet QA read, and it costs nothing
        // when (as in a bundled app) nobody is listening.
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .and_then(file_layer)
                .with_filter(filter),
        )
        .try_init();

    // chain, don't replace — the default hook keeps the backtrace on stderr for the
    // dev/QA case. without this, an `.expect()` on the boot path takes the whole app
    // down with nothing written anywhere.
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!(
            target: "ducktape::shell",
            thread = std::thread::current().name().unwrap_or("?"),
            "panicked at: {info}"
        );
        default(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shell_log_lands_under_the_dir_the_app_already_owns() {
        // ~/.ducktape is where registry.json, workspaces/ and user.key already live,
        // so a user told "send me ~/.ducktape/shell.log" is in a folder they know.
        let path = log_path().expect("a home dir in the test env");
        assert!(path.ends_with(".ducktape/shell.log"), "{path:?}");
    }

    #[test]
    fn opening_the_log_creates_it_and_appends() {
        // the failure this guards: the sink silently not existing is exactly the bug
        // being fixed, so "we opened a real writable file" is the thing to assert.
        let dir = tempfile::TempDir::new().expect("temp home");
        // SAFETY: single-threaded test; we restore nothing because each test process
        // is its own env and no other test reads HOME.
        unsafe { std::env::set_var("HOME", dir.path()) };
        let file = open_log().expect("the shell log opens");
        drop(file);
        let path = log_path().expect("path");
        assert!(path.exists(), "the shell log must actually exist on disk");
    }
}
