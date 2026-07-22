//! the working-copy loop: checkout / status / commit / pin. these operate on a
//! local checkout dir plus its `.duckfs` index; the node address comes from
//! `--node` / `DUCKTAPE_NODE` and, for verbs that run inside a checkout, the
//! index's recorded node url.

use std::path::Path;

use duckfs_client::checkout::{CheckoutError, CheckoutOptions, checkout_with};
use duckfs_client::commit::{CommitError, CommitOptions, commit_with};
use duckfs_client::http::HttpNode;
use duckfs_client::index::Index;

use crate::fs_cli::args::{CliError, NodeAddr, resolve_node, resolve_node_addr};
use crate::fs_cli::{CheckoutArgs, CommitArgs, PinArgs, StatusArgs};

/// resolve the node for a verb running inside `dir`: the addressing chain
/// (`--node` / `-n/--network` / `DUCKTAPE_NODE`) first, else the `.duckfs`
/// index's recorded node url.
fn node_for_dir(addr: &NodeAddr, dir: &Path) -> Result<HttpNode, CliError> {
    if let Some(url) = resolve_node_addr(addr)? {
        return Ok(HttpNode::new(url));
    }
    match Index::load(dir) {
        Ok(index) if !index.node.is_empty() => Ok(HttpNode::new(index.node)),
        _ => Err(CliError::usage(
            "no node address: pass --node <http-url>, -n/--network <id>, set \
             DUCKTAPE_NODE, or run inside a checkout whose index records a node",
        )),
    }
}

fn checkout_err(e: CheckoutError) -> CliError {
    CliError::failed(e.to_string())
}

/// `checkout <prefix> <dir> [--snapshot S]` — materialize the subtree and write
/// the `.duckfs` index recording the node it was checked out from.
pub fn checkout(args: CheckoutArgs) -> Result<(), CliError> {
    // a fresh checkout has no index yet, so the node MUST be explicit.
    let url = resolve_node(&args.addr)?;
    let node = HttpNode::new(url.clone());
    let snapshot = args.snapshot.as_deref();
    let opts = CheckoutOptions {
        node_url: url,
        ..Default::default()
    };
    let index = checkout_with(&node, Path::new(&args.dir), &args.prefix, snapshot, &opts)
        .map_err(checkout_err)?;
    let base = index.base_snapshot.as_deref().unwrap_or("(empty tree)");
    println!("checked out {} at {base} into {}", args.prefix, args.dir);
    Ok(())
}

/// `status [dir]` (default `.`) — one `A|M|D\tpath` line per change, exit 1 when
/// dirty (script-friendly: exit code IS the clean/dirty signal).
pub fn status(args: StatusArgs) -> Result<(), CliError> {
    let dir = args.dir.as_deref().unwrap_or(".");
    let st = duckfs_client::status::status(Path::new(dir))
        .map_err(|e| CliError::failed(e.to_string()))?;

    for e in &st.added {
        println!("A\t{}", e.path);
    }
    for e in &st.modified {
        println!("M\t{}", e.path);
    }
    for path in &st.removed {
        println!("D\t{path}");
    }

    if st.clean {
        Ok(())
    } else {
        // the changes are already printed; exit 1 with no extra error line.
        Err(CliError::silent(1))
    }
}

/// `commit [dir] --message <m> [--no-rebase]` — commit the working copy. prints
/// the new snapshot id; a conflict prints the report to stderr and exits 2.
pub fn commit(args: CommitArgs) -> Result<(), CliError> {
    let dir = args.dir.as_deref().unwrap_or(".");
    let dirp = Path::new(dir);
    let node = node_for_dir(&args.addr, dirp)?;
    let opts = CommitOptions {
        auto_rebase: !args.no_rebase,
    };

    match commit_with(&node, dirp, &args.message, &opts) {
        Ok(summary) => {
            println!("{}", summary.snapshot);
            if summary.rebased {
                eprintln!("ducktape fs: auto-rebased onto the current head before committing");
            }
            Ok(())
        }
        Err(CommitError::Conflict(report)) => {
            eprintln!("ducktape fs: commit conflict");
            eprintln!("  base: {}", report.base.as_deref().unwrap_or("(none)"));
            eprintln!("  head: {}", report.head.as_deref().unwrap_or("(none)"));
            for path in &report.clashing {
                eprintln!("  clashing: {path}");
            }
            if !report.remedy.is_empty() {
                eprintln!("  remedy: {}", report.remedy);
            }
            Err(CliError::silent(2))
        }
        Err(CommitError::Nothing) => Err(CliError::failed(
            "nothing to commit (the working copy is clean)",
        )),
        Err(e) => Err(CliError::failed(e.to_string())),
    }
}

/// `pin <snapshot> <name>` — pin a snapshot by name so gc keeps it reachable.
pub fn pin(args: PinArgs) -> Result<(), CliError> {
    use duckfs_client::api::{ApiError, NodeApi};

    // pin runs against a node directly (default `.` so a checkout's index can
    // supply the node, but `--node`/env win).
    let node = node_for_dir(&args.addr, Path::new("."))?;
    node.pin(&args.snapshot, &args.name).map_err(|e| match e {
        ApiError::NotFound => CliError::failed("snapshot not found"),
        ApiError::Rejected(m) => CliError::failed(m),
        ApiError::Transport(m) => CliError::failed(format!("cannot reach the node: {m}")),
    })?;
    println!("pinned {} as {}", args.snapshot, args.name);
    Ok(())
}
