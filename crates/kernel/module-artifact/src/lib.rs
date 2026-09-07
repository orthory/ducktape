//! One deployable unit: consensus code and its optional derived-index mapper.
//! The registry commits the hash of this whole value and activates it once.
use borsh::BorshSerialize;
use sha2::{Digest as _, Sha256};

#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize)]
pub struct ModuleArtifact {
    pub component: Vec<u8>,
    pub index: Option<Vec<u8>>,
}

impl ModuleArtifact {
    pub fn component(component: Vec<u8>) -> Self {
        Self {
            component,
            index: None,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        borsh::to_vec(self).expect("module artifact serializes")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let view = ModuleArtifactRef::decode(bytes)?;
        Ok(Self {
            component: view.component.to_vec(),
            index: view.index.map(<[u8]>::to_vec),
        })
    }

    pub fn hash(&self) -> [u8; 32] {
        Sha256::digest(self.encode()).into()
    }
}

/// A validated view over the Borsh artifact frame, without copying Wasm bytes.
pub struct ModuleArtifactRef<'a> {
    pub component: &'a [u8],
    pub index: Option<&'a [u8]>,
}

impl<'a> ModuleArtifactRef<'a> {
    pub fn decode(mut bytes: &'a [u8]) -> Result<Self, String> {
        let component = take_bytes(&mut bytes)?;
        let Some((&tag, tail)) = bytes.split_first() else {
            return Err("module artifact has no mapper tag".into());
        };
        bytes = tail;
        let index = match tag {
            0 => None,
            1 => Some(take_bytes(&mut bytes)?),
            _ => return Err("module artifact has an invalid mapper tag".into()),
        };
        if !bytes.is_empty() {
            return Err("module artifact has trailing bytes".into());
        }
        Ok(Self { component, index })
    }
}

fn take_bytes<'a>(bytes: &mut &'a [u8]) -> Result<&'a [u8], String> {
    let Some((length, tail)) = bytes.split_at_checked(4) else {
        return Err("module artifact has a truncated length".into());
    };
    let length = u32::from_le_bytes(length.try_into().expect("four-byte length")) as usize;
    let Some((value, tail)) = tail.split_at_checked(length) else {
        return Err("module artifact has a truncated body".into());
    };
    *bytes = tail;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_commitment_covers_both_components_and_mapper_removal() {
        let bare = ModuleArtifact::component(vec![1, 2, 3]);
        let indexed = ModuleArtifact {
            index: Some(vec![4, 5]),
            ..bare.clone()
        };
        let changed_mapper = ModuleArtifact {
            index: Some(vec![6]),
            ..bare.clone()
        };
        let changed_code = ModuleArtifact {
            component: vec![7],
            ..indexed.clone()
        };
        for other in [&bare, &changed_mapper, &changed_code] {
            assert_ne!(indexed.hash(), other.hash());
        }
        for artifact in [bare, indexed, changed_mapper, changed_code] {
            assert_eq!(
                ModuleArtifact::decode(&artifact.encode()).unwrap(),
                artifact
            );
        }
    }

    #[test]
    fn malformed_or_trailing_frames_are_rejected() {
        let bytes = ModuleArtifact::component(vec![1, 2, 3]).encode();
        for end in 0..bytes.len() {
            assert!(ModuleArtifact::decode(&bytes[..end]).is_err());
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(ModuleArtifact::decode(&trailing).is_err());
        let mut invalid_tag = bytes;
        *invalid_tag.last_mut().unwrap() = 2;
        assert!(ModuleArtifact::decode(&invalid_tag).is_err());
        assert!(ModuleArtifact::decode(b"\0asm\r\0\x01\0").is_err());
    }
}
