//! Current-user desired account profile, propagated to joined networks.

use base64::Engine as _;
use serde::{Deserialize, Serialize};

use super::Backend;
use super::private_fs;
use super::workspace_service::write_atomic;

const PROFILE_VERSION: u32 = 1;
const MAX_NAME_BYTES: usize = 64;
const MAX_BIO_BYTES: usize = 280;
pub const MAX_AVATAR_BYTES: usize = 256 * 1024;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalAccountProfile {
    #[serde(default = "profile_version")]
    version: u32,
    pub name: Option<String>,
    pub bio: Option<String>,
    pub avatar: Option<String>,
}

impl std::fmt::Debug for LocalAccountProfile {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LocalAccountProfile")
            .field("version", &self.version)
            .field("name", &self.name)
            .field("bio", &self.bio)
            .field("avatar", &self.avatar.as_ref().map(|_| "[IMAGE DATA]"))
            .finish()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocalAccountProfilePatch {
    /// `None` keeps the field; `Some(None)` clears it.
    pub name: Option<Option<String>>,
    pub bio: Option<Option<String>>,
    pub avatar: Option<Option<String>>,
}

const fn profile_version() -> u32 {
    PROFILE_VERSION
}

impl Default for LocalAccountProfile {
    fn default() -> Self {
        Self {
            version: PROFILE_VERSION,
            name: None,
            bio: None,
            avatar: None,
        }
    }
}

impl Backend {
    pub async fn account_profile(&self) -> Result<LocalAccountProfile, String> {
        let root = self.root.clone();
        self.control.run(move || Ok(load(&root))).await
    }

    pub async fn account_profile_update(
        &self,
        patch: LocalAccountProfilePatch,
    ) -> Result<LocalAccountProfile, String> {
        let root = self.root.clone();
        self.control
            .run(move || {
                let mut profile = load(&root);
                if let Some(name) = patch.name {
                    profile.name = name;
                }
                if let Some(bio) = patch.bio {
                    profile.bio = bio;
                }
                if let Some(avatar) = patch.avatar {
                    profile.avatar = avatar;
                }
                validate(&profile)?;
                private_fs::ensure_private_dir(&root)?;
                let bytes = serde_json::to_vec_pretty(&profile)
                    .map_err(|error| format!("encode account profile: {error}"))?;
                write_atomic(&root.join("account-profile.json"), &bytes)?;
                Ok(profile)
            })
            .await
    }
}

fn load(root: &std::path::Path) -> LocalAccountProfile {
    let Ok(Some(bytes)) = private_fs::read(&root.join("account-profile.json")) else {
        return LocalAccountProfile::default();
    };
    let Ok(profile) = serde_json::from_slice::<LocalAccountProfile>(&bytes) else {
        return LocalAccountProfile::default();
    };
    if validate(&profile).is_err() {
        return LocalAccountProfile::default();
    }
    profile
}

fn validate(profile: &LocalAccountProfile) -> Result<(), String> {
    if profile.version != PROFILE_VERSION {
        return Err("unsupported account profile version".into());
    }
    if let Some(name) = profile.name.as_deref() {
        validate_text(name, MAX_NAME_BYTES, "profile name")?;
    }
    if let Some(bio) = profile.bio.as_deref() {
        validate_text(bio, MAX_BIO_BYTES, "profile bio")?;
    }
    if let Some(avatar) = profile.avatar.as_deref() {
        validate_avatar(avatar)?;
    }
    Ok(())
}

fn validate_text(value: &str, max: usize, field: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > max {
        Err(format!("invalid {field}"))
    } else {
        Ok(())
    }
}

fn validate_avatar(value: &str) -> Result<(), String> {
    let (header, encoded) = value
        .split_once(',')
        .ok_or_else(|| "invalid stored avatar".to_string())?;
    if !matches!(
        header,
        "data:image/png;base64"
            | "data:image/jpeg;base64"
            | "data:image/gif;base64"
            | "data:image/webp;base64"
            | "data:image/avif;base64"
    ) {
        return Err("invalid stored avatar type".into());
    }
    if encoded.len()
        > MAX_AVATAR_BYTES
            .saturating_mul(4)
            .div_ceil(3)
            .saturating_add(4)
    {
        return Err("stored avatar exceeds 256 KiB".into());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| "invalid stored avatar encoding".to_string())?;
    if bytes.is_empty() || bytes.len() > MAX_AVATAR_BYTES {
        return Err("stored avatar exceeds 256 KiB".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_profile_is_bounded_and_redacts_avatar_debug() {
        let avatar = format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode([137, 80, 78, 71])
        );
        let profile = LocalAccountProfile {
            version: PROFILE_VERSION,
            name: Some("Kim".into()),
            bio: Some("building".into()),
            avatar: Some(avatar.clone()),
        };
        assert!(validate(&profile).is_ok());
        assert!(!format!("{profile:?}").contains(&avatar));

        let mut oversized = profile;
        oversized.bio = Some("x".repeat(MAX_BIO_BYTES + 1));
        assert!(validate(&oversized).is_err());
    }

    #[test]
    fn malformed_or_script_bearing_avatar_is_rejected() {
        let profile = LocalAccountProfile {
            version: PROFILE_VERSION,
            name: None,
            bio: None,
            avatar: Some("data:image/svg+xml;base64,PHN2Zz4=".into()),
        };
        assert!(validate(&profile).is_err());
    }

    #[tokio::test]
    async fn patches_keep_clear_and_reload_current_user_profile() {
        let root = std::env::temp_dir().join(format!(
            "ducktape-account-profile-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let backend = Backend::at_root(root.clone()).await.unwrap();
        backend
            .account_profile_update(LocalAccountProfilePatch {
                name: Some(Some("Kim".into())),
                bio: Some(Some("building".into())),
                avatar: None,
            })
            .await
            .unwrap();
        backend
            .account_profile_update(LocalAccountProfilePatch {
                name: None,
                bio: Some(None),
                avatar: None,
            })
            .await
            .unwrap();
        let loaded = backend.account_profile().await.unwrap();
        assert_eq!(loaded.name.as_deref(), Some("Kim"));
        assert_eq!(loaded.bio, None);
        std::fs::remove_dir_all(root).unwrap();
    }
}
