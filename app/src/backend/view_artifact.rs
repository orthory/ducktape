//! Fetch the view belonging to an already-selected module deployment.
//!
//! The caller retains the expected hash and request generation, and rechecks
//! both before installing the result. This loader never selects a deployment
//! or substitutes a desktop resource after a failed fetch.

use std::fmt;

use ducktape_rpc::Client;
use module_artifact::{MAX_ARTIFACT_BYTES, ModuleArtifactRef, ViewArtifact};
use sha2::{Digest as _, Sha256};

#[derive(Debug)]
pub enum Error {
    Transport(ducktape_rpc::Error),
    HashMismatch,
    InvalidArtifact(String),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(error) => write!(formatter, "fetch view artifact: {error}"),
            Self::HashMismatch => formatter.write_str("view artifact deployment hash mismatch"),
            Self::InvalidArtifact(error) => write!(formatter, "invalid view artifact: {error}"),
        }
    }
}

impl std::error::Error for Error {}

/// Only a verified artifact with no view returns `Ok(None)`.
pub async fn load(client: &Client, expected_hash: [u8; 32]) -> Result<Option<ViewArtifact>, Error> {
    let bytes = client
        .get_blob(&expected_hash, MAX_ARTIFACT_BYTES)
        .await
        .map_err(Error::Transport)?;
    verified_view(&bytes, expected_hash)
}

fn verified_view(bytes: &[u8], expected_hash: [u8; 32]) -> Result<Option<ViewArtifact>, Error> {
    let actual_hash: [u8; 32] = Sha256::digest(bytes).into();
    if actual_hash != expected_hash {
        return Err(Error::HashMismatch);
    }
    let artifact = ModuleArtifactRef::decode(bytes).map_err(Error::InvalidArtifact)?;
    Ok(artifact.view.map(|view| ViewArtifact {
        component: view.component.to_vec(),
        assets: view
            .assets
            .into_iter()
            .map(|(path, bytes)| (path.to_owned(), bytes.to_vec()))
            .collect(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use module_artifact::ModuleArtifact;

    #[test]
    fn a_verified_view_preserves_assets_and_removal_is_explicit() {
        let view = ViewArtifact {
            component: vec![4, 5, 6],
            assets: [("icons/action.svg".to_owned(), b"<svg/>".to_vec())].into(),
        };
        let artifact = ModuleArtifact {
            component: vec![1, 2, 3],
            index: None,
            view: Some(view.clone()),
        };
        assert_eq!(
            verified_view(&artifact.encode(), artifact.hash()).unwrap(),
            Some(view)
        );
        let removed = ModuleArtifact {
            view: None,
            ..artifact
        };
        assert_eq!(
            verified_view(&removed.encode(), removed.hash()).unwrap(),
            None
        );
    }

    #[test]
    fn tampering_and_malformed_artifacts_are_errors_not_removal() {
        let artifact = ModuleArtifact::component(vec![1, 2, 3]);
        let mut bytes = artifact.encode();
        bytes[4] ^= 1;
        assert!(matches!(
            verified_view(&bytes, artifact.hash()),
            Err(Error::HashMismatch)
        ));
        let malformed = b"not an artifact";
        assert!(matches!(
            verified_view(malformed, Sha256::digest(malformed).into()),
            Err(Error::InvalidArtifact(_))
        ));
    }
}
