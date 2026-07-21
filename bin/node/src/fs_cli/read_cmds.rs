//! the read verbs: ls / cat / stat / history / diff. thin veneers over the
//! `NodeApi` transport with stable line-oriented output (tab-separated, so a
//! script can `cut`/`grep` it). every verb resolves the node address the same
//! way (`--node` or `DUCKTAPE_NODE`) and streams paged reads to completion.

use std::collections::BTreeMap;
use std::io::Write as _;

use duckfs_client::api::{ApiError, NodeApi};
use duckfs_client::http::HttpNode;
use duckfs_core::{DiffEntry, DiffKind, EntryInfo, EntryKindWire, MAX_PAGE, MAX_READ_BYTES, SnapshotInfo};

use crate::fs_cli::args::{CliError, flag_u64, parse_flags, resolve_node};

// --- `--json` row shapes: each mirrors exactly the columns the prose form
// prints, so the JSON string values are byte-for-byte the text column values
// (`kind_tag`/`diff_tag` strings, not the wire enum spellings). they borrow the
// wire structs so no field is copied.

#[derive(serde::Serialize)]
struct LsRow<'a> {
    kind: &'a str,
    size: u64,
    path: &'a str,
}

#[derive(serde::Serialize)]
struct StatRow<'a> {
    path: &'a str,
    kind: &'a str,
    size: u64,
    exec: bool,
    object: &'a str,
}

#[derive(serde::Serialize)]
struct HistoryRow<'a> {
    height: u64,
    id: &'a str,
    message: &'a str,
}

#[derive(serde::Serialize)]
struct DiffRow<'a> {
    kind: &'a str,
    path: &'a str,
}

fn ls_row(e: &EntryInfo) -> LsRow<'_> {
    LsRow {
        kind: kind_tag(&e.kind),
        size: e.size,
        path: &e.path,
    }
}

fn stat_row(e: &EntryInfo) -> StatRow<'_> {
    StatRow {
        path: &e.path,
        kind: kind_tag(&e.kind),
        size: e.size,
        exec: e.exec,
        object: &e.object,
    }
}

fn history_row(s: &SnapshotInfo) -> HistoryRow<'_> {
    HistoryRow {
        height: s.height,
        id: &s.id,
        message: &s.message,
    }
}

fn diff_row(e: &DiffEntry) -> DiffRow<'_> {
    DiffRow {
        kind: diff_tag(&e.kind),
        path: &e.path,
    }
}

/// serialize a value to a single stdout line — the machine value the read
/// verbs' `--json` mode emits (the last-line contract).
fn print_json<T: serde::Serialize>(value: &T) {
    println!("{}", serde_json::to_string(value).expect("serializable"));
}

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

/// `ls <path> [--snapshot S] [--limit N] [--json]` — one `kind\tsize\tpath` line
/// per entry, paged to completion (`--limit` is the per-request page size).
/// `--json` collects every page into one `[{kind,size,path}, ...]` array on one
/// stdout line.
pub fn ls(args: &[String]) -> Result<(), CliError> {
    let (pos, flags) = parse_flags(args)?;
    let path = pos
        .first()
        .ok_or_else(|| CliError::usage("ls needs a <path>"))?;
    let node = node(&flags)?;
    let snapshot = flags.get("snapshot").map(String::as_str);
    let limit = flag_u64(&flags, "limit")?.unwrap_or(MAX_PAGE);
    let want_json = flags.contains_key("json");

    let mut after: Option<String> = None;
    let mut collected: Vec<EntryInfo> = Vec::new();
    loop {
        let (entries, next) = node
            .ls(path, snapshot, after.as_deref(), limit)
            .map_err(api_err)?;
        if want_json {
            collected.extend(entries);
        } else {
            for e in &entries {
                println!("{}\t{}\t{}", kind_tag(&e.kind), e.size, e.path);
            }
        }
        match next {
            Some(cursor) => after = Some(cursor),
            None => break,
        }
    }
    if want_json {
        let rows: Vec<LsRow> = collected.iter().map(ls_row).collect();
        print_json(&rows);
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

/// `stat <path> [--snapshot S] [--json]` — the entry's facts, one `key\tvalue`
/// per line (or `{path,kind,size,exec,object}` under `--json`).
pub fn stat(args: &[String]) -> Result<(), CliError> {
    let (pos, flags) = parse_flags(args)?;
    let path = pos
        .first()
        .ok_or_else(|| CliError::usage("stat needs a <path>"))?;
    let node = node(&flags)?;
    let snapshot = flags.get("snapshot").map(String::as_str);
    let want_json = flags.contains_key("json");

    let Some(e) = node.stat(path, snapshot).map_err(api_err)? else {
        return Err(CliError::failed(format!("no entry at {path}")));
    };
    if want_json {
        print_json(&stat_row(&e));
        return Ok(());
    }
    println!("path\t{}", e.path);
    println!("kind\t{}", kind_tag(&e.kind));
    println!("size\t{}", e.size);
    println!("exec\t{}", e.exec);
    println!("object\t{}", e.object);
    Ok(())
}

/// `history [--limit N] [--json]` — the commit window newest-first, one
/// `height\tsnapshot\tmessage` line each (or `[{height,id,message}, ...]` under
/// `--json`).
pub fn history(args: &[String]) -> Result<(), CliError> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(CliError::usage("history takes no positional args"));
    }
    let node = node(&flags)?;
    let limit = flag_u64(&flags, "limit")?.unwrap_or(MAX_PAGE);
    let want_json = flags.contains_key("json");

    let snapshots = node.history(limit).map_err(api_err)?;
    if want_json {
        let rows: Vec<HistoryRow> = snapshots.iter().map(history_row).collect();
        print_json(&rows);
        return Ok(());
    }
    for s in &snapshots {
        println!("{}\t{}\t{}", s.height, s.id, s.message);
    }
    Ok(())
}

/// `diff <from> <to> [--prefix P] [--json]` — the Added/Removed/Modified leaves
/// between two committed snapshots, one `A|D|M\tpath` line each (or
/// `[{kind,path}, ...]` under `--json`, with the same `A`/`D`/`M` tag).
pub fn diff(args: &[String]) -> Result<(), CliError> {
    let (pos, flags) = parse_flags(args)?;
    let [from, to] = pos.as_slice() else {
        return Err(CliError::usage("diff needs <from> <to>"));
    };
    let node = node(&flags)?;
    let prefix = flags.get("prefix").map(String::as_str).unwrap_or("");
    let want_json = flags.contains_key("json");

    let entries = node.diff(from, to, prefix).map_err(api_err)?;
    if want_json {
        let rows: Vec<DiffRow> = entries.iter().map(diff_row).collect();
        print_json(&rows);
        return Ok(());
    }
    for e in &entries {
        println!("{}\t{}", diff_tag(&e.kind), e.path);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn entry() -> EntryInfo {
        EntryInfo {
            path: "/dir/a.rs".into(),
            kind: EntryKindWire::File,
            size: 42,
            exec: true,
            object: "abc123".into(),
            meta: BTreeMap::new(),
        }
    }

    /// the ls row JSON carries the SAME three facts the text columns do — and
    /// `kind` is the `kind_tag` spelling, not the wire enum's.
    #[test]
    fn ls_row_mirrors_text_columns() {
        let e = entry();
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&ls_row(&e)).unwrap()).unwrap();
        assert_eq!(v["kind"], kind_tag(&e.kind)); // "file"
        assert_eq!(v["size"], 42);
        assert_eq!(v["path"], "/dir/a.rs");
        assert_eq!(v.as_object().unwrap().len(), 3);
    }

    /// the stat row JSON carries all five `key\tvalue` facts.
    #[test]
    fn stat_row_mirrors_text_fields() {
        let e = entry();
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&stat_row(&e)).unwrap()).unwrap();
        assert_eq!(v["path"], "/dir/a.rs");
        assert_eq!(v["kind"], "file");
        assert_eq!(v["size"], 42);
        assert_eq!(v["exec"], true);
        assert_eq!(v["object"], "abc123");
    }

    #[test]
    fn history_row_mirrors_text_columns() {
        let s = SnapshotInfo {
            id: "snap9".into(),
            parent: None,
            root_tree: "root".into(),
            author: "who".into(),
            height: 7,
            consensus_time: 100,
            message: "did a thing".into(),
        };
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&history_row(&s)).unwrap()).unwrap();
        assert_eq!(v["height"], 7);
        assert_eq!(v["id"], "snap9");
        assert_eq!(v["message"], "did a thing");
    }

    /// the diff row `kind` uses the SAME `A`/`D`/`M` tag as the text column.
    #[test]
    fn diff_row_uses_text_tag() {
        for (kind, tag) in [
            (DiffKind::Added, "A"),
            (DiffKind::Removed, "D"),
            (DiffKind::Modified, "M"),
        ] {
            let e = DiffEntry {
                path: "/x".into(),
                kind: kind.clone(),
            };
            let v: serde_json::Value =
                serde_json::from_str(&serde_json::to_string(&diff_row(&e)).unwrap()).unwrap();
            assert_eq!(v["kind"], tag);
            assert_eq!(v["kind"], diff_tag(&kind));
            assert_eq!(v["path"], "/x");
        }
    }
}
