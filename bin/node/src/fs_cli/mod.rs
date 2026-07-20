//! `ducktape fs` — the duckfs working-copy CLI.
//!
//! a hand-rolled verb dispatcher (no clap — the workspace forbids it) over the
//! `duckfs-client` engine and its `HttpNode` transport. it ships the read verbs
//! and the working-copy loop (checkout/status/commit/pin). the FUSE `mount`
//! verb was removed in the 2026-07-13 storage-plane revision (see the duckfs
//! spec's Mount surface section; the implementation lives in git history at
//! e7b4e1d1 if genuine mount demand ever returns).
//!
//! exit codes are part of the contract: 0 success, 1 an operational failure (and
//! a dirty `status`), 2 a usage error (and a commit conflict).

mod args;
mod read_cmds;
mod work_cmds;

use self::args::CliError;

const USAGE: &str = "\
ducktape fs — the duckfs working-copy CLI

usage: ducktape fs <verb> [args...]

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
";

pub(crate) fn run(argv: &[String]) -> u8 {
    let Some((verb, rest)) = argv.split_first() else {
        eprintln!("{USAGE}");
        return 2;
    };
    match dispatch(verb.as_str(), rest) {
        Ok(()) => 0,
        Err(e) => {
            if !e.message.is_empty() {
                eprintln!("ducktape fs: {}", e.message);
            }
            e.code
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
        "help" | "--help" | "-h" => {
            println!("{USAGE}");
            Ok(())
        }
        other => Err(CliError::usage(format!(
            "unknown verb {other:?}\n\n{USAGE}"
        ))),
    }
}
