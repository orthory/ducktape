//! `ducktape-fs` — the duckfs working-copy CLI.
//!
//! a hand-rolled verb dispatcher (no clap — the workspace forbids it) over the
//! `duckfs-client` engine and its `HttpNode` transport. phase 3 ships the read
//! verbs (this task) and the working-copy loop (checkout/status/commit/pin);
//! `mount` is reserved with a phase-4 stub so the verb name is claimed now.
//!
//! exit codes are part of the contract: 0 success, 1 an operational failure (and
//! a dirty `status`), 2 a usage error (and a commit conflict).

mod args;
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

  mount <prefix> <dir>                     (phase 4 — not available yet)
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
        // reserved: the FUSE mount is phase 4 (#221). the verb is claimed now so
        // its name is stable, but it fails clearly rather than pretend to work.
        "mount" => Err(CliError::usage(
            "mount arrives in phase 4 (FUSE); not available yet",
        )),
        "help" | "--help" | "-h" => {
            println!("{USAGE}");
            Ok(())
        }
        other => Err(CliError::usage(format!(
            "unknown verb {other:?}\n\n{USAGE}"
        ))),
    }
}
