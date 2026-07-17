//! Account-signed writes and bounded reads shared by Chat, Pages, and Files.
//!
//! Presentation state stays in `screens::user`; this module keeps the content
//! trust boundary in one place so no native screen can accidentally fall back
//! to the daemon's unsigned convenience endpoint.

use std::path::Path;

use serde_json::{Value, json};

use crate::backend::{Backend, ContentTarget};
use crate::transport::NodeClient;

const MAX_SIGNED_PAYLOAD_BYTES: usize = 23 * 1024;
const MAX_DOWNLOAD_BYTES: u64 = 512 * 1024 * 1024;

pub(crate) async fn submit_signed(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    target: ContentTarget,
    payload: Value,
) -> Result<(), String> {
    let backend = backend.ok_or_else(|| "desktop identity backend is unavailable".to_string())?;
    let node = node.ok_or_else(|| format!("enter a network to use {}", target.as_str()))?;
    let bytes = serde_json::to_vec(&payload)
        .map_err(|error| format!("could not encode {} write: {error}", target.as_str()))?;
    if bytes.is_empty() || bytes.len() > MAX_SIGNED_PAYLOAD_BYTES {
        return Err(format!(
            "{} write exceeds the signed-frame payload limit",
            target.as_str()
        ));
    }
    let frame = backend
        .sign_content_frame(target, hex_encode(&bytes))
        .await?;
    node.submit_frame(hex_decode(&frame)?)
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}

pub(crate) async fn delete_file(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    path: &str,
) -> Result<(), String> {
    let path = validate_file_path(path)?;
    let node = node.ok_or_else(|| "enter a network to use Files".to_string())?;
    let head = node
        .files_refs()
        .await
        .map_err(|error| error.to_string())?
        .head;
    submit_signed(
        backend,
        Some(node),
        ContentTarget::Files,
        // The files module takes externally-tagged ops; a bare commit body is
        // rejected with `unknown variant base_snapshot` (caught by
        // shell::sim::files::delete_folder).
        json!({
            "commit": {
                "base_snapshot": head,
                "message": format!("rm {path}"),
                "changes": [{ "rm": { "path": path } }]
            }
        }),
    )
    .await
}

pub(crate) async fn download_file(
    node: Option<&NodeClient>,
    path: &str,
    snapshot: Option<&str>,
    expected_size: u64,
    destination: &Path,
) -> Result<(), String> {
    let path = validate_file_path(path)?;
    if expected_size > MAX_DOWNLOAD_BYTES {
        return Err("file exceeds the desktop download limit".into());
    }
    let node = node.ok_or_else(|| "enter a network to use Files".to_string())?;
    let snapshot = match snapshot {
        Some(snapshot) => snapshot.to_owned(),
        None => node
            .files_refs()
            .await
            .map_err(|error| error.to_string())?
            .head
            .ok_or_else(|| "DuckFS has no live snapshot".to_string())?,
    };
    let bytes = node
        .files_read_exact(path, &snapshot, expected_size)
        .await
        .map_err(|error| error.to_string())?;
    let destination = destination.to_owned();
    tokio::task::spawn_blocking(move || {
        use std::io::Write as _;

        let parent = destination
            .parent()
            .ok_or_else(|| "download destination has no parent directory".to_string())?;
        let mut temporary = tempfile::Builder::new()
            .prefix(".ducktape-download-")
            .tempfile_in(parent)
            .map_err(|error| format!("could not create download: {error}"))?;
        temporary
            .write_all(&bytes)
            .map_err(|error| format!("could not write download: {error}"))?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|error| format!("could not finish download: {error}"))?;
        temporary
            .persist(destination)
            .map_err(|error| format!("could not save download: {}", error.error))?;
        Ok(())
    })
    .await
    .map_err(|_| "download writer task failed".to_string())?
}

pub(crate) async fn file_diff(
    node: Option<&NodeClient>,
    from: &str,
    to: &str,
    prefix: &str,
) -> Result<Vec<Value>, String> {
    let prefix = validate_file_path(prefix)?;
    let reply = node
        .ok_or_else(|| "enter a network to use Files".to_string())?
        .query(
            "files",
            json!({ "diff": { "from": from, "to": to, "prefix": prefix } }),
        )
        .await
        .map_err(|error| error.to_string())?;
    let rows = reply
        .get("diff")
        .and_then(Value::as_array)
        .ok_or_else(|| "node returned an invalid files diff reply".to_string())?;
    if rows.len() > 4_096 {
        return Err("files diff exceeds the desktop safety limit".into());
    }
    Ok(rows.clone())
}

pub(crate) async fn chat_write(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    payload: Value,
) -> Result<(), String> {
    submit_signed(backend, node, ContentTarget::Chat, payload).await
}

pub(crate) async fn pages_write(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    payload: Value,
) -> Result<(), String> {
    submit_signed(backend, node, ContentTarget::Pages, payload).await
}

pub(crate) fn user_key_bytes(value: &str) -> Result<Vec<u8>, String> {
    let bytes = hex_decode(value.trim())?;
    if bytes.len() != 32 {
        return Err("member key must be 32 bytes".into());
    }
    Ok(bytes)
}

pub(crate) fn validate_emoji(value: &str) -> Result<&str, String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 64 || value.chars().any(char::is_control) {
        return Err("reaction must be between 1 and 64 bytes".into());
    }
    Ok(value)
}

fn validate_file_path(value: &str) -> Result<&str, String> {
    if !value.starts_with('/')
        || value.len() > 4_096
        || value.bytes().any(|byte| matches!(byte, 0 | b'\r' | b'\n'))
        || value.split('/').any(|part| matches!(part, "." | ".."))
    {
        return Err("unsafe file path".into());
    }
    Ok(value)
}

fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn hex_decode(value: &str) -> Result<Vec<u8>, String> {
    if !value.len().is_multiple_of(2) {
        return Err("signed frame is not even-length hexadecimal".into());
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair)
                .map_err(|_| "signed frame is not hexadecimal".to_string())?;
            u8::from_str_radix(pair, 16).map_err(|_| "signed frame is not hexadecimal".to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_frames_decode_strictly() {
        assert_eq!(hex_decode("00ff").unwrap(), [0, 255]);
        assert!(hex_decode("0ff").is_err());
        assert!(hex_decode("zz").is_err());
    }

    #[test]
    fn signed_payload_limit_matches_control_cli() {
        assert!(
            serde_json::to_vec(&json!({ "text": "x".repeat(MAX_SIGNED_PAYLOAD_BYTES) }))
                .unwrap()
                .len()
                > MAX_SIGNED_PAYLOAD_BYTES
        );
    }

    #[test]
    fn chat_keys_and_reactions_are_strictly_bounded() {
        assert_eq!(user_key_bytes(&"ab".repeat(32)).unwrap(), [0xab; 32]);
        assert!(user_key_bytes("ab").is_err());
        assert_eq!(validate_emoji(" 👍 ").unwrap(), "👍");
        assert!(validate_emoji("").is_err());
        assert!(validate_emoji(&"x".repeat(65)).is_err());
    }

    #[test]
    fn signed_file_paths_are_absolute_and_cannot_escape() {
        assert_eq!(
            validate_file_path("/shared/design/logo.svg").unwrap(),
            "/shared/design/logo.svg"
        );
        assert!(validate_file_path("shared/design/logo.svg").is_err());
        assert!(validate_file_path("/shared/../secret").is_err());
        assert!(validate_file_path("/shared\nsecret").is_err());
    }
}
