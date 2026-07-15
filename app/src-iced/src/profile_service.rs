//! DuckDNS and global account-profile operations for the native Home surface.

use base64::Engine as _;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use crate::account_service::{AccountFacts, query_account};
use crate::backend::{
    Backend, IdentityStatus, LocalAccountProfile, LocalAccountProfilePatch, MAX_AVATAR_BYTES,
    Workspace,
};
use crate::screens::user::{AvatarDraft, AvatarEdit};
use crate::transport::NodeClient;
use crate::user_content_service;

const MAX_BIO_BYTES: usize = 280;

pub async fn choose_avatar() -> Result<Option<AvatarDraft>, String> {
    let Some(handle) = rfd::AsyncFileDialog::new()
        .add_filter(
            "Avatar image",
            &["png", "jpg", "jpeg", "gif", "webp", "avif"],
        )
        .pick_file()
        .await
    else {
        return Ok(None);
    };
    let source = handle.path();
    let metadata = tokio::fs::metadata(source)
        .await
        .map_err(|error| format!("could not inspect the image: {error}"))?;
    if !metadata.is_file() || metadata.len() > MAX_AVATAR_BYTES as u64 {
        return Err("image exceeds 256 KiB".into());
    }
    let bytes = tokio::fs::read(source)
        .await
        .map_err(|error| format!("could not read the image: {error}"))?;
    let extension = source
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mime = match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        _ => return Err("pick a PNG, JPEG, GIF, WebP, or AVIF image".into()),
    };
    validate_avatar_bytes(mime, &bytes)?;
    Ok(Some(AvatarDraft {
        mime: mime.into(),
        bytes,
    }))
}

pub async fn save_display_name(
    backend: Option<Backend>,
    workspace: Option<Workspace>,
    client: Option<NodeClient>,
    display_name: String,
) -> Result<(), String> {
    let backend = backend.ok_or_else(|| "desktop backend is unavailable".to_string())?;
    let workspace = require_workspace(workspace.as_ref())?;
    let client = require_client(client.as_ref())?;
    bound_account(client, workspace).await?;
    let display_name = display_name.trim().to_string();
    if display_name.len() > 64 {
        return Err("display name must be 64 bytes or fewer".into());
    }
    persist_best_effort(
        &backend,
        LocalAccountProfilePatch {
            name: Some((!display_name.is_empty()).then_some(display_name.clone())),
            ..Default::default()
        },
    )
    .await;
    client
        .submit(
            "identity",
            json!({ "set_account_name": { "display_name": display_name } }),
            Some(&workspace.pubkey),
        )
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}

pub async fn set_duck_name(
    workspace: Option<Workspace>,
    client: Option<NodeClient>,
    handle: Option<String>,
) -> Result<(), String> {
    let workspace = require_workspace(workspace.as_ref())?;
    let client = require_client(client.as_ref())?;
    bound_account(client, workspace).await?;
    let handle = handle
        .map(|handle| normalize_duck_name(&handle))
        .transpose()?;
    client
        .submit(
            "duckdns",
            json!({ "set_handle": { "handle": handle } }),
            Some(&workspace.pubkey),
        )
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}

pub async fn save_profile(
    backend: Option<Backend>,
    workspace: Option<Workspace>,
    client: Option<NodeClient>,
    bio: String,
    avatar: AvatarEdit,
) -> Result<(), String> {
    let backend = backend.ok_or_else(|| "desktop backend is unavailable".to_string())?;
    let bio = bio.trim().to_string();
    if bio.len() > MAX_BIO_BYTES {
        return Err("bio exceeds the 280-byte limit".into());
    }
    let avatar_data = match &avatar {
        AvatarEdit::Keep => None,
        AvatarEdit::Remove => Some(None),
        AvatarEdit::Replace(avatar) => {
            validate_avatar_bytes(&avatar.mime, &avatar.bytes)?;
            Some(Some(format!(
                "data:{};base64,{}",
                avatar.mime,
                base64::engine::general_purpose::STANDARD.encode(&avatar.bytes)
            )))
        }
    };
    persist_best_effort(
        &backend,
        LocalAccountProfilePatch {
            bio: Some((!bio.is_empty()).then_some(bio.clone())),
            avatar: avatar_data,
            ..Default::default()
        },
    )
    .await;

    let workspace = require_workspace(workspace.as_ref())?;
    let client = require_client(client.as_ref())?;
    let account = owned_account(&backend, client, workspace).await?;
    let avatar_path = match avatar {
        AvatarEdit::Keep => account.avatar,
        AvatarEdit::Remove => None,
        AvatarEdit::Replace(avatar) => {
            let derived = derive_avatar(&avatar.mime, avatar.bytes)?;
            if account.avatar.as_deref() != Some(&derived.path) {
                upload_avatar(&backend, client, &derived).await?;
            }
            Some(derived.path)
        }
    };
    set_profile(
        client,
        workspace,
        avatar_path,
        (!bio.is_empty()).then_some(bio),
    )
    .await
}

pub async fn reconcile_best_effort(backend: &Backend, workspace: &Workspace, client: &NodeClient) {
    if let Err(error) = reconcile(backend, workspace, client).await {
        tracing::debug!(
            target: "ducktape::account",
            event = "profile_reconcile_skipped",
            reason = "profile_reconcile_failed",
            detail = %error,
            "global account profile will retry on the next connection"
        );
    }
}

async fn reconcile(
    backend: &Backend,
    workspace: &Workspace,
    client: &NodeClient,
) -> Result<(), String> {
    let account = match owned_account(backend, client, workspace).await {
        Ok(account) => account,
        Err(error) if error == "this node isn't linked to an account yet" => return Ok(()),
        Err(error) => return Err(error),
    };
    let mut profile: LocalAccountProfile = backend.account_profile().await?;
    let seed_name = profile
        .name
        .is_none()
        .then(|| account.display_name.clone())
        .flatten();
    let seed_bio = profile.bio.is_none().then(|| account.bio.clone()).flatten();
    if seed_name.is_some() || seed_bio.is_some() {
        if let Some(name) = seed_name.as_ref() {
            profile.name = Some(name.clone());
        }
        if let Some(bio) = seed_bio.as_ref() {
            profile.bio = Some(bio.clone());
        }
        persist_best_effort(
            backend,
            LocalAccountProfilePatch {
                name: seed_name.map(Some),
                bio: seed_bio.map(Some),
                ..Default::default()
            },
        )
        .await;
    }
    if let Some(name) = profile.name.as_deref()
        && account.display_name.as_deref() != Some(name)
    {
        client
            .submit(
                "identity",
                json!({ "set_account_name": { "display_name": name } }),
                Some(&workspace.pubkey),
            )
            .await
            .map_err(|error| error.to_string())?;
    }

    let mut avatar_path = account.avatar.clone();
    let mut bio = account.bio.clone();
    let mut changed = false;
    if let Some(data_url) = profile.avatar.as_deref() {
        let avatar = derive_avatar_data_url(data_url)?;
        if account.avatar.as_deref() != Some(&avatar.path) {
            upload_avatar(backend, client, &avatar).await?;
            avatar_path = Some(avatar.path);
            changed = true;
        }
    }
    if let Some(desired) = profile.bio
        && account.bio.as_deref() != Some(&desired)
    {
        bio = Some(desired);
        changed = true;
    }
    if changed {
        set_profile(client, workspace, avatar_path, bio).await?;
    }
    Ok(())
}

pub async fn duck_name(client: &NodeClient, account_id: &str) -> Result<Option<String>, String> {
    let account_id = hex_bytes(account_id, "account id")?;
    let reply = client
        .query(
            "duckdns",
            json!({ "registrations": { "from": 0, "limit": 256 } }),
        )
        .await
        .map_err(|error| error.to_string())?;
    let rows = reply
        .get("registrations")
        .and_then(Value::as_array)
        .ok_or_else(|| "node returned invalid Duck name registrations".to_string())?;
    if rows.len() > 256 {
        return Err("node returned too many Duck name registrations".into());
    }
    for row in rows {
        let row = row
            .as_object()
            .ok_or_else(|| "node returned an invalid Duck name registration".to_string())?;
        let handle = row
            .get("handle")
            .and_then(Value::as_str)
            .ok_or_else(|| "node returned an invalid Duck name".to_string())?;
        normalize_duck_name(handle)?;
        let registered = wire_bytes(row.get("account_id"), 32, "Duck name account id")?;
        if registered == account_id {
            return Ok(Some(handle.to_string()));
        }
    }
    Ok(None)
}

pub async fn load_avatar_bytes(client: &NodeClient, path: &str) -> Result<Vec<u8>, String> {
    validate_avatar_path(path)?;
    let entry = client
        .files_stat(path, None)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "profile avatar is missing from DuckFS".to_string())?;
    if entry.kind != "file" || entry.size == 0 || entry.size > MAX_AVATAR_BYTES as u64 {
        return Err("profile avatar is not a bounded DuckFS image".into());
    }
    let (bytes, eof) = client
        .files_preview(path, None)
        .await
        .map_err(|error| error.to_string())?;
    if !eof || bytes.len() as u64 != entry.size {
        return Err("profile avatar changed while it was read".into());
    }
    let extension = path
        .rsplit_once('.')
        .map(|(_, extension)| extension)
        .unwrap_or_default();
    let mime = match extension {
        "png" => "image/png",
        "jpg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        _ => return Err("profile avatar has an invalid raster type".into()),
    };
    validate_avatar_bytes(mime, &bytes)?;
    Ok(bytes)
}

async fn bound_account(client: &NodeClient, workspace: &Workspace) -> Result<AccountFacts, String> {
    query_account(
        client,
        json!({ "of_node": { "node_key": hex_bytes(&workspace.pubkey, "active node key")? } }),
    )
    .await?
    .ok_or_else(|| "this node isn't linked to an account yet".to_string())
}

async fn owned_account(
    backend: &Backend,
    client: &NodeClient,
    workspace: &Workspace,
) -> Result<AccountFacts, String> {
    let account = bound_account(client, workspace).await?;
    let identity = backend.identity_state().await?;
    if !matches!(
        identity.state,
        IdentityStatus::Plaintext | IdentityStatus::Unlocked
    ) {
        return Err("unlock this account before editing its global profile".into());
    }
    let member = identity
        .pubkey
        .ok_or_else(|| "readable identity has no public key".to_string())?;
    if !account
        .member_keys
        .iter()
        .any(|(key, _)| key.eq_ignore_ascii_case(&member))
    {
        return Err("this node is bound to someone else's account — profile not written".into());
    }
    Ok(account)
}

async fn set_profile(
    client: &NodeClient,
    workspace: &Workspace,
    avatar: Option<String>,
    bio: Option<String>,
) -> Result<(), String> {
    client
        .submit(
            "identity",
            json!({ "set_profile": { "avatar": avatar, "bio": bio } }),
            Some(&workspace.pubkey),
        )
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}

struct DerivedAvatar {
    path: String,
    bytes: Vec<u8>,
    mime: String,
}

fn derive_avatar_data_url(data_url: &str) -> Result<DerivedAvatar, String> {
    let (header, encoded) = data_url
        .split_once(',')
        .ok_or_else(|| "avatar is not a base64 data URL".to_string())?;
    let mime = header
        .strip_prefix("data:")
        .and_then(|header| header.strip_suffix(";base64"))
        .ok_or_else(|| "avatar is not a base64 data URL".to_string())?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| "avatar is not valid base64".to_string())?;
    derive_avatar(mime, bytes)
}

fn derive_avatar(mime: &str, bytes: Vec<u8>) -> Result<DerivedAvatar, String> {
    validate_avatar_bytes(mime, &bytes)?;
    let extension = avatar_extension(mime)?;
    let digest = Sha256::digest(&bytes);
    let short = digest[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(DerivedAvatar {
        path: format!("/shared/attachments/avatars/{short}.{extension}"),
        bytes,
        mime: mime.into(),
    })
}

async fn upload_avatar(
    backend: &Backend,
    client: &NodeClient,
    avatar: &DerivedAvatar,
) -> Result<(), String> {
    // Stage the bounded raster first so the account-signed commit stays below
    // the signed-frame cap; the signature still authorizes the final path.
    let chunk = client
        .put_blob(avatar.bytes.clone())
        .await
        .map_err(|error| error.to_string())?;
    let head = client
        .files_refs()
        .await
        .map_err(|error| error.to_string())?
        .head;
    user_content_service::submit_signed(
        Some(backend),
        Some(client),
        crate::backend::ContentTarget::Files,
        json!({
            "commit": {
                "base_snapshot": head,
                "message": "avatar",
                "changes": [{
                    "put": {
                        "path": avatar.path,
                        "exec": false,
                        "meta": { "mime": avatar.mime },
                        "content": {
                            "chunks": {
                                "size": avatar.bytes.len(),
                                "chunks": [chunk]
                            }
                        }
                    }
                }]
            }
        }),
    )
    .await
}

async fn persist_best_effort(backend: &Backend, patch: LocalAccountProfilePatch) {
    if let Err(error) = backend.account_profile_update(patch).await {
        tracing::debug!(
            target: "ducktape::account",
            event = "account_profile_store_failed",
            reason = "local_state_write_failed",
            detail = %error,
            "global account profile could not be persisted locally"
        );
    }
}

fn normalize_duck_name(value: &str) -> Result<String, String> {
    let handle = value.trim().to_ascii_lowercase();
    if handle.is_empty() {
        Err("Enter a name.".into())
    } else if handle.len() > 63 {
        Err("Use 63 characters or fewer.".into())
    } else if handle.starts_with('-') || handle.ends_with('-') {
        Err("A name cannot start or end with a hyphen.".into())
    } else if !handle
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        Err("Use lowercase letters, numbers, and hyphens.".into())
    } else if matches!(handle.as_str(), "net" | "agents") {
        Err(format!("{handle}.duck is reserved."))
    } else {
        Ok(handle)
    }
}

fn avatar_extension(mime: &str) -> Result<&'static str, String> {
    match mime {
        "image/png" => Ok("png"),
        "image/jpeg" => Ok("jpg"),
        "image/gif" => Ok("gif"),
        "image/webp" => Ok("webp"),
        "image/avif" => Ok("avif"),
        _ => Err(format!("unsupported avatar type: {mime}")),
    }
}

fn validate_avatar_bytes(mime: &str, bytes: &[u8]) -> Result<(), String> {
    avatar_extension(mime)?;
    if bytes.is_empty() || bytes.len() > MAX_AVATAR_BYTES {
        return Err("image exceeds 256 KiB".into());
    }
    let valid = match mime {
        "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        "image/gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        "image/webp" => bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP",
        "image/avif" => is_avif(bytes),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err("the selected file does not match its image type".into())
    }
}

fn is_avif(bytes: &[u8]) -> bool {
    if bytes.len() < 16 || &bytes[4..8] != b"ftyp" {
        return false;
    }
    let declared = u32::from_be_bytes(bytes[..4].try_into().expect("four bytes")) as usize;
    let box_len = if declared == 0 { bytes.len() } else { declared };
    if !(16..=bytes.len()).contains(&box_len) {
        return false;
    }
    matches!(&bytes[8..12], b"avif" | b"avis")
        || bytes[16..box_len]
            .chunks_exact(4)
            .any(|brand| matches!(brand, b"avif" | b"avis"))
}

fn validate_avatar_path(path: &str) -> Result<(), String> {
    let name = path
        .strip_prefix("/shared/attachments/avatars/")
        .ok_or_else(|| "profile avatar is outside the avatar directory".to_string())?;
    let (digest, extension) = name
        .split_once('.')
        .ok_or_else(|| "profile avatar has an invalid path".to_string())?;
    if digest.len() != 16
        || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
        || !matches!(extension, "png" | "jpg" | "gif" | "webp" | "avif")
    {
        return Err("profile avatar has an invalid path".into());
    }
    Ok(())
}

fn hex_bytes(value: &str, field: &str) -> Result<Vec<u8>, String> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("invalid {field}"));
    }
    (0..value.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&value[index..index + 2], 16).map_err(|_| format!("invalid {field}"))
        })
        .collect()
}

fn wire_bytes(value: Option<&Value>, len: usize, field: &str) -> Result<Vec<u8>, String> {
    let bytes = value
        .and_then(Value::as_array)
        .ok_or_else(|| format!("node returned an invalid {field}"))?;
    if bytes.len() != len {
        return Err(format!("node returned an invalid {field}"));
    }
    bytes
        .iter()
        .map(|byte| {
            byte.as_u64()
                .filter(|byte| *byte <= u8::MAX as u64)
                .map(|byte| byte as u8)
                .ok_or_else(|| format!("node returned an invalid {field}"))
        })
        .collect()
}

fn require_workspace(workspace: Option<&Workspace>) -> Result<&Workspace, String> {
    workspace.ok_or_else(|| "enter a network to update your profile".into())
}

fn require_client(client: Option<&NodeClient>) -> Result<&NodeClient, String> {
    client.ok_or_else(|| "enter a network to update your profile".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duck_names_match_consensus_label_rules() {
        assert_eq!(normalize_duck_name("  Kim-7 ").unwrap(), "kim-7");
        for invalid in ["", "-kim", "kim-", "Kim!", "net", "agents"] {
            assert!(normalize_duck_name(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn avatar_paths_are_sha256_addressed_and_script_formats_are_refused() {
        let png = b"\x89PNG\r\n\x1a\nbody".to_vec();
        let avatar = derive_avatar("image/png", png).unwrap();
        assert_eq!(
            avatar.path,
            "/shared/attachments/avatars/776f30dcc133299a.png"
        );
        assert!(derive_avatar("image/svg+xml", b"<svg/>".to_vec()).is_err());
    }

    #[test]
    fn avatar_data_url_is_strict_and_bounded() {
        let url = format!(
            "data:image/gif;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(b"GIF89a...")
        );
        assert_eq!(derive_avatar_data_url(&url).unwrap().mime, "image/gif");
        assert!(derive_avatar_data_url("data:image/png,raw").is_err());
    }
}
