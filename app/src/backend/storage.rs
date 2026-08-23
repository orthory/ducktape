use super::*;

/// One files-browser row.
#[derive(Clone, Debug, Default, Hash, PartialEq)]
pub struct FsEntry {
    /// Session-stable identity for keyed rendering.
    pub key: i64,
    pub path: String,
    pub name: String,
    pub kind: String,
    pub size: i64,
    /// the entry's content address — already on the ls wire.
    pub object: String,
}

/// Blank selection mirrored into Ice while no listed object is selected.
pub fn no_fs_entry() -> FsEntry {
    FsEntry::default()
}

/// Resolve a selected object when selection or its listing changes, not while
/// the view renders every frame.
pub fn fs_entry_named(entries: Vec<FsEntry>, path: String) -> FsEntry {
    entries
        .into_iter()
        .find(|entry| entry.path == path)
        .unwrap_or_default()
}

/// Directory rows prepared with the listing so the keyed sidebar does not
/// filter and clone the full entry list while building every frame.
pub fn fs_directories(entries: &[FsEntry]) -> Vec<FsEntry> {
    entries
        .iter()
        .filter(|entry| entry.kind == "dir")
        .cloned()
        .collect()
}

/// What is under this crumb, counted. Ice cannot filter a list by field, so the
/// crumb bar's two counts are pure folds over the listing it is already drawn
/// beside — never a second `files_ls`.
pub fn fs_dir_count(entries: &[FsEntry]) -> i64 {
    count_i64(entries.iter().filter(|entry| entry.kind == "dir").count())
}

/// Everything that is not a directory. `files_ls` publishes one `kind` per row
/// and the browser draws exactly two shapes, so the complement IS the file
/// count — no third bucket can hide here.
pub fn fs_file_count(entries: &[FsEntry]) -> i64 {
    count_i64(entries.iter().filter(|entry| entry.kind != "dir").count())
}

/// `12 files · 3 dirs` — the crumb bar's own subtitle, and "" whenever the rows
/// on hand are not this path's. A listing nobody fetched folds to
/// `0 files · 0 dirs`, which reads as "this path is empty" when the truth is
/// "nobody asked"; a listing fetched for the directory you just LEFT folds to
/// that directory's tally, printed under the new one's name. Same rule as the
/// register subtitles in backend/shell.rs: say nothing rather than something
/// false.
pub fn fs_counts_summary(connected: bool, listed: bool, entries: &[FsEntry]) -> String {
    if !connected || !listed || entries.is_empty() {
        return String::new();
    }
    let file_count = fs_file_count(entries);
    let files = plural(file_count, "file", "files");
    let dirs = plural(fs_dir_count(entries), "dir", "dirs");
    format!("{files} · {dirs}")
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
    /// The body is a decoded picture in the Files surface's slot
    /// (`picture.rs`), drawn at `width` × `height`; `text` is empty.
    pub picture: bool,
    pub width: i64,
    pub height: i64,
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

/// The `entries` array of an ls reply as rows.
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
                key: stable_view_key(&format!("duckfs:{entry_path}")),
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

/// Read a file for the preview pane. A picture (by path — `picture_path`)
/// pages its whole body in and decodes it into the Files surface's slot;
/// anything else reads a 64 KiB head.
pub async fn files_preview(
    rpc: String,
    path: String,
    generation: i64,
) -> Result<FsPreview, HydrationError> {
    async {
        let rpc = rpc_client(&rpc)?;
        match super::picture::picture_path(path.clone()) {
            true => files_picture(&rpc, path, generation).await,
            false => files_text(&rpc, path, generation).await,
        }
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// The text preview: the first 64 KiB, branded binary on a control byte.
async fn files_text(
    rpc: &RpcClient,
    path: String,
    generation: i64,
) -> Result<FsPreview, String> {
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
        picture: false,
        width: 0,
        height: 0,
    })
}

/// The picture preview: page the whole file through the 1 MiB `read` lane
/// (the checkout's `read_all` shape), decode off the runtime, park the handle.
/// A file past the byte cap or one that does not decode falls back to the
/// binary plate with the reason as its line — never a global error.
async fn files_picture(
    rpc: &RpcClient,
    path: String,
    generation: i64,
) -> Result<FsPreview, String> {
    use super::picture::{FILES_SURFACE, MAX_PICTURE_BYTES, store_picture};
    let Some(bytes) = files_read_all(rpc, &path).await? else {
        let note = format!(
            "picture larger than the {} MiB preview limit",
            MAX_PICTURE_BYTES >> 20
        );
        return Ok(binary_preview(generation, path, note));
    };
    let size = bytes.len();
    match store_picture(FILES_SURFACE, path.clone(), bytes).await {
        Ok((width, height)) => Ok(FsPreview {
            generation,
            path,
            text: String::new(),
            truncated: false,
            binary: false,
            picture: true,
            width: i64::from(width),
            height: i64::from(height),
        }),
        Err(reason) => Ok(binary_preview(
            generation,
            path,
            format!("{size} binary bytes · did not decode: {reason}"),
        )),
    }
}

/// Page one duckfs file in whole through the `read` lane (1 MiB pages to eof
/// — the checkout's `read_all` shape). `None`: past the picture byte cap,
/// not assembled.
pub(crate) async fn files_read_all(rpc: &RpcClient, path: &str) -> Result<Option<Vec<u8>>, String> {
    use super::picture::MAX_PICTURE_BYTES;
    // The `read` lane's own page cap (duckfs `MAX_READ_BYTES`); the node clamps
    // anything larger, so asking for exactly it is one round-trip per MiB.
    let page_len = (1024 * 1024).to_string();
    let mut bytes = Vec::new();
    loop {
        let offset = bytes.len().to_string();
        let reply = rpc
            .files_get(
                "read",
                &[("path", path), ("offset", offset.as_str()), ("len", page_len.as_str())],
            )
            .await?;
        let page = base64_decode(reply["b64"].as_str().unwrap_or_default()).unwrap_or_default();
        let eof = reply["eof"].as_bool().unwrap_or(true);
        bytes.extend_from_slice(&page);
        let past_cap = bytes.len() > MAX_PICTURE_BYTES;
        if past_cap {
            return Ok(None);
        }
        let done = eof || page.is_empty();
        if done {
            return Ok(Some(bytes));
        }
    }
}

fn binary_preview(generation: i64, path: String, text: String) -> FsPreview {
    FsPreview {
        generation,
        path,
        text,
        truncated: false,
        binary: true,
        picture: false,
        width: 0,
        height: 0,
    }
}

/// The committed snapshot window, newest first.
pub async fn files_history(rpc: String, generation: i64) -> Result<FsHistory, HydrationError> {
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
    // the node refuses to serve back any object larger than files_http's
    // MAX_OBJECT_BYTES, and every staged MiB is a consensus block — a cap
    // HERE turns "drop a video, drive 300 blocks, node RSS grows by 300 MB"
    // into one refusal toast.
    const MAX_DROP_BYTES: u64 = 64 * 1024 * 1024;
    async {
        let source = PathBuf::from(&dropped);
        let name = source
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| "dropped path has no file name".to_string())?
            .to_string();
        let size = std::fs::metadata(&source)
            .map_err(|error| format!("cannot read {dropped}: {error}"))?
            .len();
        if size > MAX_DROP_BYTES {
            return Err(format!(
                "{name} is {} MiB — the node stores files up to {} MiB",
                size / (1024 * 1024),
                MAX_DROP_BYTES / (1024 * 1024)
            ));
        }
        // off the render runtime: a multi-MB read is a blocking call.
        let read = tokio::task::spawn_blocking(move || std::fs::read(&source));
        let bytes = read
            .await
            .map_err(|error| format!("file read task failed: {error}"))?
            .map_err(|error| format!("cannot read {dropped}: {error}"))?;
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

/// A child path under the current directory (`/` is the root, never "").
pub fn fs_child(path: String, name: String) -> String {
    let name = name.trim().trim_matches('/');
    let dir = path.trim_end_matches('/');
    format!("{dir}/{name}")
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

/// The breadcrumb path one level up. The root is `/` — duckfs only accepts
/// absolute paths, and "" earned a 400 from the node on every root open.
pub fn fs_parent(path: String) -> String {
    match path.rfind('/') {
        Some(0) | None => "/".to_string(),
        Some(cut) => path[..cut].to_string(),
    }
}
