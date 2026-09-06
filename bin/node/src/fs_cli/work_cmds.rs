//! the working-copy loop: checkout / status / commit / pin. these operate on a
//! local checkout dir plus its `.duckfs` index; the node address comes from the
//! shared [`crate::cli_args::NodeAddr`] ladder, which for verbs running inside a
//! checkout takes the index's recorded node url as its context rung.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use duckfs_client::checkout::{CheckoutError, CheckoutOptions, checkout_with};
use duckfs_client::commit::{CommitError, CommitOptions, commit_with};
use duckfs_client::http::HttpNode;
use duckfs_client::index::Index;

use crate::fs_cli::args::{CliError, NodeAddr, resolve_node};
use crate::fs_cli::{CheckoutArgs, CommitArgs, PinArgs, StatusArgs};

/// resolve the node for a verb running inside `dir`: the shared addressing
/// ladder, with this checkout's `.duckfs` index as the ambient context rung —
/// below what the operator stated, above the registry inference. A checkout
/// records the node it came FROM, which beats "the one workspace registered on
/// this box".
fn url_for_dir(addr: &NodeAddr, dir: &Path) -> Result<String, CliError> {
    let recorded = || Index::load(dir).ok().map(|index| index.node);
    addr.resolve_with(recorded).map_err(CliError::usage)
}

/// a node whose WRITES carry the acting person's signature.
///
/// Every mutating duckfs route refuses a request that proves nothing, and this
/// verb is a person's: what it stages and commits is charged to the key signing
/// here — the commit's author, the `/home/<owner>/**` authority, and the
/// staging quota (#1312). Opening the key costs one password prompt per verb,
/// which is why it happens ONCE, before the walk, and the closure reuses the
/// opened key for every chunk.
///
/// CEILING: on a validator this authorship stops at the node's ingress, which
/// re-signs an unframed submit with the NODE's key. Carrying it into consensus
/// needs the client to sign the FRAME (`/v1/submit/frame`), which is a wire
/// decision, not this seam's.
fn signing_node(
    addr: &NodeAddr,
    dir: &Path,
    key: Option<PathBuf>,
    trust_node: bool,
) -> Result<HttpNode, CliError> {
    let url = url_for_dir(addr, dir)?;
    let node_key = crate::node_http::pinned_node_key(&url, trust_node)
        .map_err(|error| CliError::failed(error.to_string()))?;
    let ctx = crate::cred_cli::VerbCtx {
        addr: addr.clone(),
        key,
    };
    let key_path = ctx
        .key_path()
        .map_err(|e| CliError::failed(e.to_string()))?;
    let mut stdin = std::io::BufReader::new(std::io::stdin());
    let signer = crate::userkey_cli::load_user_signer(&key_path, &mut stdin)
        .map_err(|e| CliError::failed(e.to_string()))?;
    Ok(
        HttpNode::new(url).with_write_auth(Arc::new(move |method, path, body| {
            noded::signed_req::request_headers(&signer, method, path, &node_key, body)
                .into_iter()
                .map(|(name, value)| (name.to_string(), value))
                .collect()
        })),
    )
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

/// `status [dir] [--path P]...` (default `.`) — one `A|M|D\tpath` line per
/// change, exit 1 when dirty (script-friendly: exit code IS the clean/dirty
/// signal). `--path` reports what the same pathspec would commit.
pub fn status(args: StatusArgs) -> Result<(), CliError> {
    let dir = args.dir.as_deref().unwrap_or(".");
    let dirp = Path::new(dir);
    let st = duckfs_client::status::status(dirp).map_err(|e| CliError::failed(e.to_string()))?;
    let st = match args.paths.is_empty() {
        true => st,
        // the pathspec is written against the checkout, so it needs the prefix
        // the index recorded.
        false => {
            let index = Index::load(dirp).map_err(|e| CliError::failed(e.to_string()))?;
            st.select(&args.paths, &index.prefix)
        }
    };

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

/// `commit [dir] --message <m> [--no-rebase] [--path P]...` — commit the working
/// copy, or the part of it the pathspec selects. prints the new snapshot id; a
/// conflict prints the report to stderr and exits 2.
pub fn commit(args: CommitArgs) -> Result<(), CliError> {
    let dir = args.dir.as_deref().unwrap_or(".");
    let dirp = Path::new(dir);
    let node = signing_node(&args.addr, dirp, args.key, args.trust_node)?;
    let opts = CommitOptions {
        auto_rebase: !args.no_rebase,
        paths: args.paths,
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
    let node = signing_node(&args.addr, Path::new("."), args.key, args.trust_node)?;
    node.pin(&args.snapshot, &args.name).map_err(|e| match e {
        ApiError::NotFound => CliError::failed("snapshot not found"),
        ApiError::Rejected(m) => CliError::failed(m),
        ApiError::Transport(m) => CliError::failed(format!("cannot reach the node: {m}")),
    })?;
    println!("pinned {} as {}", args.snapshot, args.name);
    Ok(())
}
