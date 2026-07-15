//! Files wire and platform adapter.

use super::*;

const FILE_CHUNK_BYTES: usize = 1024 * 1024;
const MAX_INLINE_COMMIT_BYTES: u64 = 256 * 1024;
const MAX_FILE_CHANGES: usize = 4_096;
const MAX_FILE_PATH_BYTES: usize = 4_096;

#[derive(Debug)]
pub(super) enum UploadEntry {
    Directory {
        relative: String,
    },
    File {
        relative: String,
        source: PathBuf,
        size: u64,
        executable: bool,
    },
}
pub(super) async fn load_files(
    client: Option<NodeClient>,
    path: String,
    snapshot: Option<String>,
) -> Result<Option<FileListing>, String> {
    let client = client.ok_or_else(|| "enter a network to load Files".to_string())?;
    let (entries, refs, history) = tokio::try_join!(
        client.files_ls(&path, snapshot.as_deref()),
        client.files_refs(),
        client.files_history(64),
    )
    .map_err(|error| error.to_string())?;
    let entries = entries
        .into_iter()
        .filter_map(|entry| {
            let kind = match entry.kind.as_str() {
                "dir" => FileKind::Directory,
                "file" => FileKind::File,
                "symlink" => FileKind::Symlink,
                _ => return None,
            };
            let name = entry
                .path
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .to_string();
            Some(ScreenFileEntry {
                path: entry.path,
                name,
                kind,
                size: entry.size,
                executable: entry.exec,
            })
        })
        .collect();
    let history = history
        .into_iter()
        .map(|entry| ScreenFileSnapshot {
            id: entry.id,
            message: entry.message,
            height: entry.height,
            time: clock_time(entry.consensus_time),
        })
        .collect();
    Ok(Some(FileListing {
        path,
        entries,
        preview: None,
        read_only: snapshot.is_some(),
        refreshing: false,
        head: refs.head,
        snapshot,
        history,
        diff: Vec::new(),
    }))
}

pub(super) async fn load_file(
    client: Option<NodeClient>,
    path: String,
    snapshot: Option<String>,
) -> Result<FilePreview, String> {
    let client = client.ok_or_else(|| "enter a network to load Files".to_string())?;
    let (bytes, complete) = client
        .files_preview(&path, snapshot.as_deref())
        .await
        .map_err(|error| error.to_string())?;
    let detail = if complete {
        format!("{} bytes", bytes.len())
    } else {
        format!("{} byte preview · file continues", bytes.len())
    };
    Ok(FilePreview {
        path,
        content: classify_file_preview(bytes, complete),
        detail,
    })
}

pub(super) fn classify_file_preview(bytes: Vec<u8>, complete: bool) -> FilePreviewContent {
    if bytes
        .windows(5)
        .take(1_024)
        .any(|header| header == b"%PDF-")
    {
        return FilePreviewContent::Pdf;
    }
    if let Ok(format) = image::guess_format(&bytes)
        && matches!(
            format,
            image::ImageFormat::Png
                | image::ImageFormat::Jpeg
                | image::ImageFormat::Gif
                | image::ImageFormat::WebP
        )
    {
        if !complete {
            return FilePreviewContent::Unsupported(
                "Image exceeds the 1 MiB encoded preview limit. Download it to open safely.".into(),
            );
        }
        let mut reader = image::ImageReader::new(std::io::Cursor::new(&bytes));
        reader.set_format(format);
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(4_096);
        limits.max_image_height = Some(4_096);
        limits.max_alloc = Some(64 * 1024 * 1024);
        reader.limits(limits);
        return match reader.into_dimensions() {
            Ok((width, height)) => FilePreviewContent::Image {
                bytes,
                width,
                height,
            },
            Err(_) => FilePreviewContent::Unsupported(
                "Image is invalid or exceeds the 4096 × 4096 preview limit.".into(),
            ),
        };
    }
    match String::from_utf8(bytes) {
        Ok(text)
            if !text.chars().any(|character| {
                character.is_control() && !matches!(character, '\n' | '\r' | '\t')
            }) =>
        {
            FilePreviewContent::Text(text)
        }
        _ => FilePreviewContent::Unsupported(
            "This binary file type is not rendered by the desktop preview.".into(),
        ),
    }
}

pub(super) async fn choose_download(
    client: Option<&NodeClient>,
    path: &str,
    size: u64,
    snapshot: Option<&str>,
) -> Result<(), String> {
    let name = path
        .rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or("download");
    let Some(destination) = rfd::AsyncFileDialog::new()
        .set_file_name(name)
        .save_file()
        .await
    else {
        return Ok(());
    };
    user_content_service::download_file(client, path, snapshot, size, destination.path()).await
}

pub(super) async fn load_file_diff(
    client: Option<&NodeClient>,
    from: &str,
    to: &str,
    prefix: &str,
) -> Result<Vec<ScreenFileDiff>, String> {
    let diff = user_content_service::file_diff(client, from, to, prefix)
        .await?
        .iter()
        .map(parse_file_diff)
        .collect::<Result<Vec<_>, String>>()?;
    Ok(diff)
}

pub(super) fn parse_file_diff(value: &Value) -> Result<ScreenFileDiff, String> {
    let path = value
        .get("path")
        .and_then(Value::as_str)
        .filter(|path| path.starts_with('/') && path.len() <= 4_096)
        .ok_or_else(|| "node returned an invalid files diff path".to_string())?;
    let kind = value
        .get("kind")
        .and_then(Value::as_str)
        .filter(|kind| matches!(*kind, "added" | "removed" | "modified"))
        .ok_or_else(|| "node returned an invalid files diff kind".to_string())?;
    Ok(ScreenFileDiff {
        path: path.to_string(),
        kind: kind.to_string(),
    })
}

pub(super) async fn create_folder(
    backend: Option<Backend>,
    client: Option<NodeClient>,
    parent: String,
    name: String,
) -> Result<(), String> {
    let client = client.ok_or_else(|| "enter a network to use Files".to_string())?;
    if name.is_empty()
        || name.len() > 255
        || name == "."
        || name == ".."
        || name
            .chars()
            .any(|character| matches!(character, '/' | '\\' | '\0'))
    {
        return Err("folder name is invalid".into());
    }
    let path = if parent == "/" {
        format!("/{name}")
    } else {
        format!("{}/{name}", parent.trim_end_matches('/'))
    };
    let head = client
        .files_refs()
        .await
        .map_err(|error| error.to_string())?
        .head;
    let body = json!({
        "base_snapshot": head,
        "message": format!("create {name}"),
        "changes": [{ "mkdir": { "path": path } }]
    });
    commit_files(backend, &client, body).await
}

pub(super) async fn choose_files(
    backend: Option<Backend>,
    client: Option<NodeClient>,
    target: String,
) -> Result<(), String> {
    let Some(handles) = rfd::AsyncFileDialog::new().pick_files().await else {
        return Ok(());
    };
    let paths = handles
        .into_iter()
        .map(|handle| handle.path().to_owned())
        .collect::<Vec<_>>();
    let entries = tokio::task::spawn_blocking(move || selected_files(paths))
        .await
        .map_err(|_| "native file selection task failed".to_string())??;
    upload_entries(backend, client, target, entries, "upload files").await
}

pub(super) async fn choose_folder(
    backend: Option<Backend>,
    client: Option<NodeClient>,
    target: String,
) -> Result<(), String> {
    let Some(handle) = rfd::AsyncFileDialog::new().pick_folder().await else {
        return Ok(());
    };
    let path = handle.path().to_owned();
    let entries = tokio::task::spawn_blocking(move || selected_folder(&path))
        .await
        .map_err(|_| "native folder selection task failed".to_string())??;
    upload_entries(backend, client, target, entries, "upload folder").await
}

pub(super) async fn upload_dropped(
    backend: Option<Backend>,
    client: Option<NodeClient>,
    target: String,
    source: PathBuf,
) -> Result<(), String> {
    let entries = tokio::task::spawn_blocking(move || dropped_entries(source))
        .await
        .map_err(|_| "native dropped-file inspection task failed".to_string())??;
    upload_entries(backend, client, target, entries, "drop files").await
}

pub(super) fn dropped_entries(source: PathBuf) -> Result<Vec<UploadEntry>, String> {
    let metadata = std::fs::symlink_metadata(&source)
        .map_err(|error| format!("could not inspect dropped path: {error}"))?;
    if metadata.file_type().is_symlink() {
        return Err("symbolic links cannot be imported".into());
    }
    if metadata.is_dir() {
        selected_folder(&source)
    } else if metadata.is_file() {
        selected_files(vec![source])
    } else {
        Err("the dropped path is not a regular file or folder".into())
    }
}

fn selected_files(paths: Vec<PathBuf>) -> Result<Vec<UploadEntry>, String> {
    if paths.len() > MAX_FILE_CHANGES {
        return Err("file selection exceeds the 4096-item limit".into());
    }
    paths
        .into_iter()
        .map(|source| {
            let relative = source
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| "a selected file name is not valid UTF-8".to_string())?;
            validate_relative_path(relative)?;
            file_entry(relative.to_owned(), source)
        })
        .collect()
}

fn selected_folder(root: &Path) -> Result<Vec<UploadEntry>, String> {
    let root_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "the selected folder name is not valid UTF-8".to_string())?
        .to_owned();
    validate_relative_path(&root_name)?;
    let metadata = std::fs::symlink_metadata(root)
        .map_err(|error| format!("could not inspect selected folder: {error}"))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("the selected item is not a regular folder".into());
    }

    let mut entries = vec![UploadEntry::Directory {
        relative: root_name.clone(),
    }];
    walk_folder(root, &root_name, &mut entries)?;
    Ok(entries)
}

fn walk_folder(root: &Path, relative: &str, entries: &mut Vec<UploadEntry>) -> Result<(), String> {
    let mut children = std::fs::read_dir(root)
        .map_err(|error| format!("could not read selected folder: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("could not read selected folder: {error}"))?;
    children.sort_by_key(std::fs::DirEntry::file_name);
    for child in children {
        if entries.len() >= MAX_FILE_CHANGES {
            return Err("folder selection exceeds the 4096-item limit".into());
        }
        let name = child
            .file_name()
            .into_string()
            .map_err(|_| "a selected path is not valid UTF-8".to_string())?;
        let child_relative = format!("{relative}/{name}");
        validate_relative_path(&child_relative)?;
        let metadata = child
            .path()
            .symlink_metadata()
            .map_err(|error| format!("could not inspect selected path: {error}"))?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "symbolic links cannot be imported: {child_relative}"
            ));
        }
        if metadata.is_dir() {
            entries.push(UploadEntry::Directory {
                relative: child_relative.clone(),
            });
            walk_folder(&child.path(), &child_relative, entries)?;
        } else if metadata.is_file() {
            entries.push(file_entry(child_relative, child.path())?);
        } else {
            return Err(format!("unsupported filesystem entry: {child_relative}"));
        }
    }
    Ok(())
}

pub(super) fn file_entry(relative: String, source: PathBuf) -> Result<UploadEntry, String> {
    let metadata = std::fs::symlink_metadata(&source)
        .map_err(|error| format!("could not inspect selected file: {error}"))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(format!("selected path is not a regular file: {relative}"));
    }
    #[cfg(unix)]
    let executable = {
        use std::os::unix::fs::PermissionsExt as _;
        metadata.permissions().mode() & 0o111 != 0
    };
    #[cfg(not(unix))]
    let executable = false;
    Ok(UploadEntry::File {
        relative,
        source,
        size: metadata.len(),
        executable,
    })
}

pub(super) fn validate_relative_path(path: &str) -> Result<(), String> {
    if path.is_empty() || path.starts_with('/') || path.len() > MAX_FILE_PATH_BYTES {
        return Err("selected path is invalid or too long".into());
    }
    for segment in path.split('/') {
        if segment.is_empty()
            || segment == "."
            || segment == ".."
            || segment.len() > MAX_FILE_NAME_BYTES
            || segment.contains(['\\', '\0'])
        {
            return Err(format!("selected path contains an invalid segment: {path}"));
        }
    }
    Ok(())
}

pub(super) fn upload_path(target: &str, relative: &str) -> Result<String, String> {
    if !target.starts_with('/') || target.contains('\0') {
        return Err("file upload target must be an absolute DuckFS path".into());
    }
    if target
        .split('/')
        .skip(1)
        .filter(|segment| !segment.is_empty())
        .any(|segment| {
            segment == "."
                || segment == ".."
                || segment.len() > MAX_FILE_NAME_BYTES
                || segment.contains('\\')
        })
    {
        return Err("file upload target contains an invalid path segment".into());
    }
    let target = target.trim_end_matches('/');
    let path = if target.is_empty() {
        format!("/{relative}")
    } else {
        format!("{target}/{relative}")
    };
    if path.len() > MAX_FILE_PATH_BYTES {
        return Err(format!("file upload target is too long: {path}"));
    }
    Ok(path)
}

pub(super) async fn upload_entries(
    backend: Option<Backend>,
    client: Option<NodeClient>,
    target: String,
    entries: Vec<UploadEntry>,
    label: &str,
) -> Result<(), String> {
    let client = client.ok_or_else(|| "enter a network to use Files".to_string())?;
    if entries.is_empty() || entries.len() > MAX_FILE_CHANGES {
        return Err("file import requires between 1 and 4096 items".into());
    }
    let mut seen = HashSet::with_capacity(entries.len());
    let mut changes = Vec::with_capacity(entries.len());
    let mut inline_bytes = 0_u64;
    for entry in entries {
        match entry {
            UploadEntry::Directory { relative } => {
                let path = upload_path(&target, &relative)?;
                if !seen.insert(path.clone()) {
                    return Err(format!("file selection contains a duplicate path: {path}"));
                }
                changes.push(json!({ "mkdir": { "path": path } }));
            }
            UploadEntry::File {
                relative,
                source,
                size,
                executable,
            } => {
                let path = upload_path(&target, &relative)?;
                if !seen.insert(path.clone()) {
                    return Err(format!("file selection contains a duplicate path: {path}"));
                }
                let content = if size > 0
                    && inline_bytes.saturating_add(size) <= MAX_INLINE_COMMIT_BYTES
                {
                    let bytes = tokio::fs::read(&source)
                        .await
                        .map_err(|error| format!("could not read {relative}: {error}"))?;
                    if bytes.len() as u64 != size {
                        return Err(format!("selected file changed while importing: {relative}"));
                    }
                    inline_bytes += size;
                    json!({
                        "inline": {
                            "b64": base64::engine::general_purpose::STANDARD.encode(bytes)
                        }
                    })
                } else {
                    let chunks = stage_file(&client, &source, size, &relative).await?;
                    json!({ "chunks": { "size": size, "chunks": chunks } })
                };
                changes.push(json!({
                    "put": {
                        "path": path,
                        "exec": executable,
                        "meta": {},
                        "content": content
                    }
                }));
            }
        }
    }
    let head = client
        .files_refs()
        .await
        .map_err(|error| error.to_string())?
        .head;
    let body = json!({
        "base_snapshot": head,
        "message": format!("{label} to {}", if target.is_empty() { "/" } else { &target }),
        "changes": changes
    });
    commit_files(backend, &client, body).await
}

async fn stage_file(
    client: &NodeClient,
    source: &Path,
    expected_size: u64,
    relative: &str,
) -> Result<Vec<String>, String> {
    if expected_size == 0 {
        return Ok(Vec::new());
    }
    let mut file = tokio::fs::File::open(source)
        .await
        .map_err(|error| format!("could not read {relative}: {error}"))?;
    let mut remaining = expected_size;
    let mut chunks = Vec::new();
    while remaining > 0 {
        let wanted = remaining.min(FILE_CHUNK_BYTES as u64) as usize;
        let mut chunk = vec![0_u8; wanted];
        file.read_exact(&mut chunk).await.map_err(|error| {
            format!("selected file changed while importing {relative}: {error}")
        })?;
        chunks.push(
            client
                .put_blob(chunk)
                .await
                .map_err(|error| error.to_string())?,
        );
        remaining -= wanted as u64;
    }
    let mut trailing = [0_u8; 1];
    if file
        .read(&mut trailing)
        .await
        .map_err(|error| format!("could not finish reading {relative}: {error}"))?
        != 0
    {
        return Err(format!("selected file changed while importing: {relative}"));
    }
    Ok(chunks)
}

async fn commit_files(
    backend: Option<Backend>,
    client: &NodeClient,
    body: Value,
) -> Result<(), String> {
    user_content_service::submit_signed(
        backend.as_ref(),
        Some(client),
        crate::backend::ContentTarget::Files,
        json!({ "commit": body }),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_diff_wire_is_flat_and_closed() {
        assert_eq!(
            parse_file_diff(&json!({ "path": "/shared/new", "kind": "added" })).unwrap(),
            ScreenFileDiff {
                path: "/shared/new".into(),
                kind: "added".into(),
            }
        );
        assert!(parse_file_diff(&json!({ "path": "relative", "kind": "added" })).is_err());
        assert!(parse_file_diff(&json!({ "path": "/shared/new", "kind": "renamed" })).is_err());
    }

    #[test]
    fn file_preview_classifier_keeps_binary_content_inert_and_bounded() {
        assert_eq!(
            classify_file_preview(b"hello\nworld".to_vec(), true),
            FilePreviewContent::Text("hello\nworld".into())
        );
        assert_eq!(
            classify_file_preview(b"%PDF-1.7\n".to_vec(), true),
            FilePreviewContent::Pdf
        );
        assert!(matches!(
            classify_file_preview(vec![0, 159, 146, 150], true),
            FilePreviewContent::Unsupported(_)
        ));
        let png = base64::engine::general_purpose::STANDARD
            .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=")
            .unwrap();
        assert!(matches!(
            classify_file_preview(png.clone(), true),
            FilePreviewContent::Image {
                width: 1,
                height: 1,
                ..
            }
        ));
        assert!(matches!(
            classify_file_preview(png, false),
            FilePreviewContent::Unsupported(_)
        ));
    }

    #[test]
    fn upload_paths_are_normalized_and_cannot_escape_duckfs() {
        assert_eq!(
            upload_path("/shared", "Project/readme.md").unwrap(),
            "/shared/Project/readme.md"
        );
        assert_eq!(upload_path("/", "readme.md").unwrap(), "/readme.md");
        assert!(upload_path("shared", "readme.md").is_err());
        assert!(upload_path("/shared/../private", "readme.md").is_err());
        assert!(validate_relative_path("../private/key").is_err());
        assert!(validate_relative_path("Project\\key").is_err());
        assert!(validate_relative_path("Project/readme.md").is_ok());
    }

    #[test]
    fn dropped_files_and_folders_use_the_picker_validation_path() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("design.txt");
        std::fs::write(&file, b"design").unwrap();
        let file_entries = dropped_entries(file).unwrap();
        assert!(matches!(
            file_entries.as_slice(),
            [UploadEntry::File { relative, size: 6, .. }] if relative == "design.txt"
        ));

        let folder = root.path().join("Project");
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("README.md"), b"readme").unwrap();
        let folder_entries = dropped_entries(folder).unwrap();
        assert!(matches!(
            folder_entries.first(),
            Some(UploadEntry::Directory { relative }) if relative == "Project"
        ));
        assert!(matches!(
            folder_entries.get(1),
            Some(UploadEntry::File { relative, .. }) if relative == "Project/README.md"
        ));
    }

    #[cfg(unix)]
    #[test]
    fn dropped_symlinks_are_rejected() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("real.txt");
        let link = root.path().join("link.txt");
        std::fs::write(&file, b"real").unwrap();
        symlink(file, &link).unwrap();
        assert!(
            dropped_entries(link)
                .unwrap_err()
                .contains("symbolic links")
        );
    }
}
