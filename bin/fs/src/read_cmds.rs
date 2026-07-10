//! the read verbs: ls / cat / stat / history / diff. thin veneers over the
//! `NodeApi` transport with stable line-oriented output (tab-separated, so a
//! script can `cut`/`grep` it). every verb resolves the node address the same
//! way (`--node` or `DUCKTAPE_NODE`) and streams paged reads to completion.

use std::collections::BTreeMap;
use std::io::Write as _;

use duckfs_client::api::{ApiError, NodeApi};
use duckfs_client::http::HttpNode;
use duckfs_core::{DiffKind, EntryKindWire, MAX_PAGE, MAX_READ_BYTES};

use crate::args::{CliError, flag_u64, parse_flags, resolve_node};

/// build the transport for a read verb from the resolved node address.
fn node(flags: &BTreeMap<String, String>) -> Result<HttpNode, CliError> {
    Ok(HttpNode::new(resolve_node(flags)?))
}

/// map a transport failure to a CLI failure (exit 1).
fn api_err(e: ApiError) -> CliError {
    match e {
        ApiError::NotFound => CliError::failed("not found"),
        ApiError::Rejected(m) => CliError::failed(m),
        ApiError::Transport(m) => CliError::failed(format!("cannot reach the node: {m}")),
    }
}

fn kind_tag(kind: &EntryKindWire) -> &'static str {
    match kind {
        EntryKindWire::File => "file",
        EntryKindWire::Dir => "dir",
        EntryKindWire::Symlink => "symlink",
    }
}

fn diff_tag(kind: &DiffKind) -> &'static str {
    match kind {
        DiffKind::Added => "A",
        DiffKind::Removed => "D",
        DiffKind::Modified => "M",
    }
}

/// `ls <path> [--snapshot S] [--limit N]` — one `kind\tsize\tpath` line per
/// entry, paged to completion (`--limit` is the per-request page size).
pub fn ls(args: &[String]) -> Result<(), CliError> {
    let (pos, flags) = parse_flags(args)?;
    let path = pos
        .first()
        .ok_or_else(|| CliError::usage("ls needs a <path>"))?;
    let node = node(&flags)?;
    let snapshot = flags.get("snapshot").map(String::as_str);
    let limit = flag_u64(&flags, "limit")?.unwrap_or(MAX_PAGE);

    let mut after: Option<String> = None;
    loop {
        let (entries, next) = node
            .ls(path, snapshot, after.as_deref(), limit)
            .map_err(api_err)?;
        for e in &entries {
            println!("{}\t{}\t{}", kind_tag(&e.kind), e.size, e.path);
        }
        match next {
            Some(cursor) => after = Some(cursor),
            None => break,
        }
    }
    Ok(())
}

/// `cat <path> [--snapshot S]` — stream the file's bytes to stdout in paged
/// reads (raw bytes, so a binary file round-trips exactly).
pub fn cat(args: &[String]) -> Result<(), CliError> {
    let (pos, flags) = parse_flags(args)?;
    let path = pos
        .first()
        .ok_or_else(|| CliError::usage("cat needs a <path>"))?;
    let node = node(&flags)?;
    let snapshot = flags.get("snapshot").map(String::as_str);

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let mut offset = 0u64;
    loop {
        let (bytes, eof) = node
            .read(path, snapshot, offset, MAX_READ_BYTES)
            .map_err(api_err)?;
        let empty = bytes.is_empty();
        offset += bytes.len() as u64;
        out.write_all(&bytes)
            .map_err(|e| CliError::failed(e.to_string()))?;
        if eof || empty {
            break;
        }
    }
    out.flush().map_err(|e| CliError::failed(e.to_string()))?;
    Ok(())
}

/// `stat <path> [--snapshot S]` — the entry's facts, one `key\tvalue` per line.
pub fn stat(args: &[String]) -> Result<(), CliError> {
    let (pos, flags) = parse_flags(args)?;
    let path = pos
        .first()
        .ok_or_else(|| CliError::usage("stat needs a <path>"))?;
    let node = node(&flags)?;
    let snapshot = flags.get("snapshot").map(String::as_str);

    match node.stat(path, snapshot).map_err(api_err)? {
        Some(e) => {
            println!("path\t{}", e.path);
            println!("kind\t{}", kind_tag(&e.kind));
            println!("size\t{}", e.size);
            println!("exec\t{}", e.exec);
            println!("object\t{}", e.object);
            Ok(())
        }
        None => Err(CliError::failed(format!("no entry at {path}"))),
    }
}

/// `history [--limit N]` — the commit window newest-first, one
/// `height\tsnapshot\tmessage` line each.
pub fn history(args: &[String]) -> Result<(), CliError> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(CliError::usage("history takes no positional args"));
    }
    let node = node(&flags)?;
    let limit = flag_u64(&flags, "limit")?.unwrap_or(MAX_PAGE);

    for s in node.history(limit).map_err(api_err)? {
        println!("{}\t{}\t{}", s.height, s.id, s.message);
    }
    Ok(())
}

/// `diff <from> <to> [--prefix P]` — the Added/Removed/Modified leaves between
/// two committed snapshots, one `A|D|M\tpath` line each.
pub fn diff(args: &[String]) -> Result<(), CliError> {
    let (pos, flags) = parse_flags(args)?;
    let [from, to] = pos.as_slice() else {
        return Err(CliError::usage("diff needs <from> <to>"));
    };
    let node = node(&flags)?;
    let prefix = flags.get("prefix").map(String::as_str).unwrap_or("");

    for e in node.diff(from, to, prefix).map_err(api_err)? {
        println!("{}\t{}", diff_tag(&e.kind), e.path);
    }
    Ok(())
}
