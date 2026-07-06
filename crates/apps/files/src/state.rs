//! the small mutable consensus state — [`Refs`]. everything else in duckfs is
//! an immutable object; the module root commits to THIS struct's canonical
//! encoding only. task 6 grows the fields (head, pins, watches, staging,
//! history window) and the codec; the skeleton pins the encoding frame so an
//! empty-refs root is already deterministic and non-zero.

use sha2::{Digest as _, Sha256};

/// leading codec byte of every refs image. domain-separates the root preimage
/// from raw zero-length input and gets bumped on layout change (flag-day rule:
/// no migrations, fresh genesis).
const REFS_CODEC_VERSION: u8 = 0;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Refs {}

impl Refs {
    /// canonical encoding — the exact `root_bytes` preimage and the exact
    /// persisted/synced image. version byte only until task 6 adds fields.
    pub fn encode(&self) -> Vec<u8> {
        vec![REFS_CODEC_VERSION]
    }

    /// strict decode of [`Refs::encode`] output; anything else rejects, so a
    /// colluding-root install can never smuggle non-canonical bytes in.
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        match bytes {
            [REFS_CODEC_VERSION] => Ok(Self {}),
            _ => Err("files: refs image is not the current codec".into()),
        }
    }

    /// sha256 over the canonical encoding — the module root preimage.
    pub fn root_bytes(&self) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(self.encode());
        h.finalize().into()
    }
}
