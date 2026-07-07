//! `ducktape-fs` — the duckfs working-copy CLI.
//!
//! a hand-rolled verb dispatcher (no clap — the workspace forbids it) over the
//! `duckfs-client` engine and its `HttpNode` transport. it ships the read verbs
//! and the working-copy loop (checkout/status/commit/pin); `mount` fronts a duckfs
//! subtree as a real kernel filesystem behind the `fuse` cargo feature (a default
//! build keeps `mount` a clear "rebuild with --features fuse" error, so the
//! workspace never depends on libfuse).
//!
//! exit codes are part of the contract: 0 success, 1 an operational failure (and
//! a dirty `status`), 2 a usage error (and a commit conflict).

mod args;
#[cfg(feature = "fuse")]
mod fuse;
mod mount_cmd;
mod read_cmds;
mod work_cmds;

use std::process::ExitCode;

use args::CliError;

const USAGE: &str = "\
ducktape-fs — the duckfs working-copy CLI

usage: ducktape-fs <verb> [args...]

read verbs (need --node <http-url> or the DUCKTAPE_NODE env):
  ls <path> [--snapshot S] [--limit N]     list a directory
  cat <path> [--snapshot S]                stream a file to stdout
  stat <path> [--snapshot S]               print an entry's facts
  history [--limit N]                      the commit window, newest-first
  diff <from> <to> [--prefix P]            changed leaves between two snapshots

working-copy verbs (the node comes from --node/DUCKTAPE_NODE or the .duckfs index):
  checkout <prefix> <dir> [--snapshot S]   materialize a subtree + .duckfs index
  status [dir]                             show A/M/D (exit 1 when dirty)
  commit [dir] --message <m> [--no-rebase] commit the working copy (exit 2 on conflict)
  pin <snapshot> <name>                    pin a snapshot so gc keeps it

fuse mount (needs a build with --features fuse; unprivileged, via fusermount3):
  mount <prefix> <dir> [--snapshot S] [--rw] [--auto-commit N] [--node URL]
      front a duckfs subtree as a live kernel filesystem, then block until
      SIGINT/SIGTERM unmounts it. read-only by default at a snapshot PINNED for
      the mount's lifetime (explicit --snapshot, else the head at mount time — a
      remount is how you see newer commits). --rw fronts a real working copy in a
      <dir>.duckfs-backing checkout: writes land locally and reach the cluster
      only on commit — explicit by default (unmount prints how to commit),
      --auto-commit N commits every N seconds while dirty. non-goals: no
      cross-node lock/mmap coherence, uid/gid/mode are synthetic, case-colliding
      siblings can't materialize on a case-insensitive host. a crash leaves a
      stale mount recoverable with `fusermount3 -u <dir>`.
";

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let Some((verb, rest)) = argv.split_first() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    match dispatch(verb.as_str(), rest) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            if !e.message.is_empty() {
                eprintln!("ducktape-fs: {}", e.message);
            }
            ExitCode::from(e.code)
        }
    }
}

fn dispatch(verb: &str, rest: &[String]) -> Result<(), CliError> {
    match verb {
        "ls" => read_cmds::ls(rest),
        "cat" => read_cmds::cat(rest),
        "stat" => read_cmds::stat(rest),
        "history" => read_cmds::history(rest),
        "diff" => read_cmds::diff(rest),
        "checkout" => work_cmds::checkout(rest),
        "status" => work_cmds::status(rest),
        "commit" => work_cmds::commit(rest),
        "pin" => work_cmds::pin(rest),
        // the FUSE mount. real behind `--features fuse`; a clear rebuild error
        // otherwise (see `mount_cmd`), so a default build never needs libfuse.
        "mount" => mount_cmd::mount(rest),
        "help" | "--help" | "-h" => {
            println!("{USAGE}");
            Ok(())
        }
        other => Err(CliError::usage(format!(
            "unknown verb {other:?}\n\n{USAGE}"
        ))),
    }
}
