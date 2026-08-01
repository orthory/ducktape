use super::*;

/// One files-browser row.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct FsEntry {
    pub path: String,
    pub name: String,
    pub kind: String,
    pub size: i64,
    /// the entry's content address — already on the ls/find wire.
    pub object: String,
}

/// What is under this crumb, counted. Ice cannot filter a list by field, so the
/// crumb bar's two counts are pure folds over the listing it is already drawn
/// beside — never a second `files_ls`.
pub fn fs_dir_count(entries: Vec<FsEntry>) -> i64 {
    count_i64(entries.iter().filter(|entry| entry.kind == "dir").count())
}

/// Everything that is not a directory. `files_ls` publishes one `kind` per row
/// and the browser draws exactly two shapes, so the complement IS the file
/// count — no third bucket can hide here.
pub fn fs_file_count(entries: Vec<FsEntry>) -> i64 {
    count_i64(entries.iter().filter(|entry| entry.kind != "dir").count())
}

/// One committed duckfs snapshot.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct FsSnapshot {
    pub id: String,
    pub short_id: String,
    pub author: String,
    pub height: i64,
    pub message: String,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct FsListing {
    pub generation: i64,
    pub path: String,
    pub entries: Vec<FsEntry>,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct FsPreview {
    pub generation: i64,
    pub path: String,
    pub text: String,
    pub truncated: bool,
    pub binary: bool,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct FsHistory {
    pub generation: i64,
    pub snapshots: Vec<FsSnapshot>,
}

/// List one duckfs directory (committed head), name order.
pub async fn files_ls(
    rpc: String,
    path: String,
    generation: i64,
) -> Result<FsListing, HydrationError> {
    offscreen_guard(generation)?;
    async {
        let rpc = rpc_client(&rpc)?;
        let listed = rpc.files_get("ls", &[("path", path.as_str())]).await;
        let reply = match listed {
            Ok(reply) => reply,
            // A CLIENT reads an uncommitted path as an empty directory, not
            // an error: a fresh workspace has no `/shared` until something
            // writes under it, and the module's not-found refusal painted the
            // global banner over every first-run Files open (#804). Any other
            // refusal still surfaces.
            Err(error) => {
                let message: String = error.into();
                let uncommitted_path = message.contains("path not found");
                if !uncommitted_path {
                    return Err(message);
                }
                return Ok(FsListing {
                    generation,
                    entries: Vec::new(),
                    path,
                });
            }
        };
        Ok(FsListing {
            generation,
            entries: fs_entries(&reply),
            path,
        })
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// Every path under one prefix, in full-path order — the duckfs tree sidebar
/// and Explorer's FILE results read the same wire.
pub async fn files_find(
    rpc: String,
    prefix: String,
    generation: i64,
) -> Result<FsListing, HydrationError> {
    async {
        let rpc = rpc_client(&rpc)?;
        let reply = rpc
            .files_get("find", &[("prefix", prefix.as_str())])
            .await?;
        Ok(FsListing {
            generation,
            entries: fs_entries(&reply),
            path: prefix,
        })
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// The `entries` array of an ls/find reply as rows (both serve `EntryInfo`).
fn fs_entries(reply: &serde_json::Value) -> Vec<FsEntry> {
    reply["entries"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|entry| {
            let entry_path = entry["path"].as_str().unwrap_or_default().to_string();
            let name = entry_path
                .rsplit('/')
                .next()
                .unwrap_or(entry_path.as_str())
                .to_string();
            FsEntry {
                name,
                kind: entry["kind"].as_str().unwrap_or_default().to_string(),
                size: entry["size"].as_i64().unwrap_or(0),
                object: entry["object"].as_str().unwrap_or_default().to_string(),
                path: entry_path,
            }
        })
        .collect()
}

/// `412 KB` — a byte count in the unit a person reads.
pub fn size_label(bytes: i64) -> String {
    const KB: i64 = 1_024;
    const MB: i64 = 1_024 * KB;
    const GB: i64 = 1_024 * MB;
    match bytes {
        size if size < KB => format!("{size} B"),
        size if size < MB => format!("{} KB", size / KB),
        size if size < GB => format!("{:.1} MB", size as f64 / MB as f64),
        size => format!("{:.1} GB", size as f64 / GB as f64),
    }
}

/// Read a file's head bytes for the preview pane (64 KiB window).
pub async fn files_preview(
    rpc: String,
    path: String,
    generation: i64,
) -> Result<FsPreview, HydrationError> {
    async {
        let rpc = rpc_client(&rpc)?;
        let reply = rpc
            .files_get("read", &[("path", path.as_str()), ("len", "65536")])
            .await?;
        let b64 = reply["b64"].as_str().unwrap_or_default();
        let eof = reply["eof"].as_bool().unwrap_or(true);
        let bytes = base64_decode(b64).unwrap_or_default();
        let (text, binary) = match String::from_utf8(bytes.clone()) {
            Ok(text)
                if !text
                    .chars()
                    .any(|c| c.is_control() && c != '\n' && c != '\t' && c != '\r') =>
            {
                (text, false)
            }
            _ => (format!("{} binary bytes", bytes.len()), true),
        };
        Ok(FsPreview {
            generation,
            path,
            text,
            truncated: !eof,
            binary,
        })
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// The committed snapshot window, newest first.
pub async fn files_history(rpc: String, generation: i64) -> Result<FsHistory, HydrationError> {
    offscreen_guard(generation)?;
    async {
        let rpc = rpc_client(&rpc)?;
        let reply = rpc.files_get("history", &[("limit", "50")]).await?;
        let snapshots = reply["snapshots"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|snapshot| {
                let id = snapshot["id"].as_str().unwrap_or_default().to_string();
                FsSnapshot {
                    short_id: short_digest(&id),
                    author: short_digest(snapshot["author"].as_str().unwrap_or_default()),
                    height: snapshot["height"].as_i64().unwrap_or(0),
                    message: snapshot["message"].as_str().unwrap_or_default().to_string(),
                    id,
                }
            })
            .collect();
        Ok(FsHistory {
            generation,
            snapshots,
        })
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// The head snapshot id for commit CAS (empty when nothing is committed).
async fn files_head(rpc: &RpcClient) -> Result<Option<String>, String> {
    let refs = rpc.files_get("refs", &[]).await?;
    Ok(refs["head"].as_str().map(str::to_string))
}

/// One files commit through the node's commit lane.
async fn files_commit_one(
    rpc: &RpcClient,
    message: String,
    change: serde_json::Value,
) -> Result<(), String> {
    let head = files_head(rpc).await?;
    rpc.files_post(
        "commit",
        &serde_json::json!({
            "base_snapshot": head,
            "message": message,
            "changes": [change],
        }),
    )
    .await?;
    Ok(())
}

/// Create a directory.
pub async fn files_mkdir(rpc: String, path: String) -> Result<bool, AppError> {
    async {
        let rpc = rpc_client(&rpc)?;
        files_commit_one(
            &rpc,
            format!("mkdir {path}"),
            serde_json::json!({ "mkdir": { "path": path } }),
        )
        .await
    }
    .await
    .map_err(app_error)?;
    Ok(true)
}

/// Remove a file or whole subtree.
pub async fn files_remove(rpc: String, path: String) -> Result<bool, AppError> {
    async {
        let rpc = rpc_client(&rpc)?;
        files_commit_one(
            &rpc,
            format!("rm {path}"),
            serde_json::json!({ "rm": { "path": path } }),
        )
        .await
    }
    .await
    .map_err(app_error)?;
    Ok(true)
}

/// Write a text file (create or replace) as inline content.
pub async fn files_write_text(rpc: String, path: String, text: String) -> Result<bool, AppError> {
    async {
        let rpc = rpc_client(&rpc)?;
        files_commit_one(
            &rpc,
            format!("write {path}"),
            serde_json::json!({
                "put": {
                    "path": path,
                    "exec": false,
                    "meta": {},
                    "content": { "inline": { "b64": base64_encode(text.as_bytes()) } },
                }
            }),
        )
        .await
    }
    .await
    .map_err(app_error)?;
    Ok(true)
}

/// Upload a local file dropped onto the window into the current directory:
/// small files ride inline; larger ones stage 1 MiB chunks then commit a
/// chunk list. The dropped path never leaves this device — only bytes do.
pub async fn files_upload(rpc: String, dir: String, dropped: String) -> Result<bool, AppError> {
    async {
        let source = PathBuf::from(&dropped);
        let name = source
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| "dropped path has no file name".to_string())?
            .to_string();
        let bytes =
            std::fs::read(&source).map_err(|error| format!("cannot read {dropped}: {error}"))?;
        let rpc = rpc_client(&rpc)?;
        let target = fs_child(dir, name.clone());
        let content = match bytes.len() as u64 <= 256 * 1024 {
            true => serde_json::json!({ "inline": { "b64": base64_encode(&bytes) } }),
            false => {
                let mut chunks = Vec::new();
                for chunk in bytes.chunks(1024 * 1024) {
                    chunks.push(rpc.files_stage(chunk.to_vec()).await?);
                }
                serde_json::json!({ "chunks": { "size": bytes.len() as u64, "chunks": chunks } })
            }
        };
        files_commit_one(
            &rpc,
            format!("upload {name}"),
            serde_json::json!({
                "put": { "path": target, "exec": false, "meta": {}, "content": content }
            }),
        )
        .await
    }
    .await
    .map_err(app_error)?;
    Ok(true)
}

/// The Added/Removed/Modified leaves between a snapshot and the head.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct FsDiffEntry {
    pub path: String,
    pub kind: String,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct FsDiff {
    pub generation: i64,
    pub from: String,
    pub entries: Vec<FsDiffEntry>,
}

/// Diff one committed snapshot against the current head.
pub async fn files_diff(
    rpc: String,
    from: String,
    generation: i64,
) -> Result<FsDiff, HydrationError> {
    async {
        let rpc = rpc_client(&rpc)?;
        let head = files_head(&rpc)
            .await?
            .ok_or_else(|| "nothing committed yet".to_string())?;
        let reply = rpc
            .files_get("diff", &[("from", from.as_str()), ("to", head.as_str())])
            .await?;
        let entries = reply["entries"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|entry| FsDiffEntry {
                path: entry["path"].as_str().unwrap_or_default().to_string(),
                kind: entry["kind"].as_str().unwrap_or_default().to_string(),
            })
            .collect();
        Ok(FsDiff {
            generation,
            from,
            entries,
        })
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// Who last changed one duckfs PATH, and at which block.
///
/// This is the path's last COMMIT — never blob authorship. duckfs stores
/// content-addressed objects with no per-blob author, so the honest label is
/// "last changed at this path", which is exactly what walking the snapshot
/// window answers.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ChangeStamp {
    pub generation: i64,
    pub path: String,
    /// The committing member, short form; empty when no commit in the window
    /// touched this path.
    pub author: String,
    /// The committing block height; 0 with an empty author.
    pub height: i64,
}

/// How far back a last-changed walk looks. A path untouched in this many
/// commits reads as unknown rather than wrong.
//
// ponytail: one diff round-trip per snapshot until the first hit — recent
// paths answer in one or two. Bound it lower, or ask the module for a
// per-path log, if a cold path ever makes this walk visible.
const CHANGE_STAMP_WINDOW: usize = 50;

/// Walk the committed snapshots newest-first and stop at the first one whose
/// diff against its parent touches `path`.
pub async fn last_changed_at_path(
    rpc: String,
    path: String,
    generation: i64,
) -> Result<ChangeStamp, HydrationError> {
    async {
        let client = rpc_client(&rpc)?;
        let limit = CHANGE_STAMP_WINDOW.to_string();
        let history = client
            .files_get("history", &[("limit", limit.as_str())])
            .await?;
        // `history` is newest-first, which is the order this walk wants.
        let snapshots = history["snapshots"].as_array().cloned().unwrap_or_default();
        for snapshot in &snapshots {
            let id = snapshot["id"].as_str().unwrap_or_default();
            let stamp = ChangeStamp {
                generation,
                path: path.clone(),
                author: short_digest(snapshot["author"].as_str().unwrap_or_default()),
                height: snapshot["height"].as_i64().unwrap_or(0),
            };
            // The root snapshot has no parent to diff against: everything the
            // window still holds was introduced there.
            let Some(parent) = snapshot["parent"].as_str() else {
                return Ok(stamp);
            };
            let diff = client
                .files_get(
                    "diff",
                    &[("from", parent), ("to", id), ("prefix", path.as_str())],
                )
                .await?;
            let touched = diff["entries"]
                .as_array()
                .is_some_and(|entries| !entries.is_empty());
            if touched {
                return Ok(stamp);
            }
        }
        Ok(ChangeStamp {
            generation,
            path,
            author: String::new(),
            height: 0,
        })
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

pub(crate) fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let mut acc = 0u32;
        for (i, byte) in chunk.iter().enumerate() {
            acc |= u32::from(*byte) << (16 - 8 * i);
        }
        for i in 0..4 {
            let live = i * 6 < chunk.len() * 8 + 6 && i <= chunk.len();
            match live {
                true => out.push(TABLE[((acc >> (18 - 6 * i)) & 0x3f) as usize] as char),
                false => out.push('='),
            }
        }
    }
    out
}

/// A child path under the current directory.
pub fn fs_child(path: String, name: String) -> String {
    let name = name.trim().trim_matches('/');
    if path.is_empty() {
        return format!("/{name}");
    }
    format!("{path}/{name}")
}

/// Minimal base64 (standard alphabet, padded) — the files read lane's wire.
pub(crate) fn base64_decode(input: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let value = |c: u8| TABLE.iter().position(|t| *t == c).map(|i| i as u32);
    let clean: Vec<u8> = input.bytes().filter(|b| !b" \n\r\t".contains(b)).collect();
    let mut out = Vec::with_capacity(clean.len() / 4 * 3);
    for chunk in clean.chunks(4) {
        let mut acc = 0u32;
        let mut bits = 0u32;
        for byte in chunk {
            if *byte == b'=' {
                break;
            }
            acc = (acc << 6) | value(*byte)?;
            bits += 6;
        }
        while bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xff) as u8);
        }
    }
    Some(out)
}

/// The breadcrumb path one level up ("" at the root).
pub fn fs_parent(path: String) -> String {
    match path.rfind('/') {
        Some(0) | None => String::new(),
        Some(cut) => path[..cut].to_string(),
    }
}
