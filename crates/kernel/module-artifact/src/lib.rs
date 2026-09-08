//! One deployable unit: consensus code, optional index mapper, and optional view.
//! The registry commits the hash of this whole value and activates it once.
use std::collections::BTreeMap;

use borsh::BorshSerialize;
use sha2::{Digest as _, Sha256};

/// The complete encoded artifact is bounded like the node's staging lane.
pub const MAX_ARTIFACT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_VIEW_ASSETS: usize = 4096;
pub const MAX_ASSET_PATH_BYTES: usize = 1024;

#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize)]
pub struct ModuleArtifact {
    pub component: Vec<u8>,
    pub index: Option<Vec<u8>>,
    /// `None` removes the view, including all its assets, at activation.
    pub view: Option<ViewArtifact>,
}

#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize)]
pub struct ViewArtifact {
    pub component: Vec<u8>,
    /// Logical relative paths, never archive entries or filesystem extraction targets.
    pub assets: BTreeMap<String, Vec<u8>>,
}

impl ModuleArtifact {
    pub fn component(component: Vec<u8>) -> Self {
        Self {
            component,
            index: None,
            view: None,
        }
    }

    /// Serializes the owned value. Trust boundaries must validate with `decode`.
    pub fn encode(&self) -> Vec<u8> {
        borsh::to_vec(self).expect("module artifact serializes")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let artifact = ModuleArtifactRef::decode(bytes)?;
        Ok(Self {
            component: artifact.component.to_vec(),
            index: artifact.index.map(<[u8]>::to_vec),
            view: artifact.view.map(|view| ViewArtifact {
                component: view.component.to_vec(),
                assets: view
                    .assets
                    .into_iter()
                    .map(|(path, bytes)| (path.to_owned(), bytes.to_vec()))
                    .collect(),
            }),
        })
    }

    pub fn hash(&self) -> [u8; 32] {
        Sha256::digest(self.encode()).into()
    }
}

/// A validated view over the Borsh artifact frame, without copying payload bytes.
#[derive(Debug)]
pub struct ModuleArtifactRef<'a> {
    pub component: &'a [u8],
    pub index: Option<&'a [u8]>,
    pub view: Option<ViewArtifactRef<'a>>,
}

/// Only map nodes allocate; keys and payloads borrow the bounded artifact frame.
#[derive(Debug)]
pub struct ViewArtifactRef<'a> {
    pub component: &'a [u8],
    pub assets: BTreeMap<&'a str, &'a [u8]>,
}

impl<'a> ModuleArtifactRef<'a> {
    pub fn decode(mut bytes: &'a [u8]) -> Result<Self, String> {
        if bytes.len() > MAX_ARTIFACT_BYTES {
            return Err("module artifact exceeds the 16 MiB frame limit".into());
        }
        let component = take_bytes(&mut bytes)?;
        let index = match take_tag(&mut bytes)? {
            0 => None,
            1 => Some(take_bytes(&mut bytes)?),
            _ => return Err("module artifact has an invalid mapper tag".into()),
        };
        let view = match take_tag(&mut bytes)? {
            0 => None,
            1 => Some(take_view(&mut bytes)?),
            _ => return Err("module artifact has an invalid view tag".into()),
        };
        if !bytes.is_empty() {
            return Err("module artifact has trailing bytes".into());
        }
        Ok(Self {
            component,
            index,
            view,
        })
    }
}

fn take_view<'a>(bytes: &mut &'a [u8]) -> Result<ViewArtifactRef<'a>, String> {
    let component = take_bytes(bytes)?;
    let count = take_length(bytes)?;
    if count > MAX_VIEW_ASSETS {
        return Err("module artifact has more than 4096 view assets".into());
    }
    let mut assets = BTreeMap::new();
    let mut previous = None;
    for _ in 0..count {
        let path = std::str::from_utf8(take_bytes(bytes)?)
            .map_err(|_| "module artifact asset path is not UTF-8")?;
        validate_asset_path(path)?;
        let out_of_order = previous.is_some_and(|previous| previous >= path);
        if out_of_order {
            return Err("module artifact asset paths are duplicated or out of order".into());
        }
        let payload = take_bytes(bytes)?;
        assets.insert(path, payload);
        previous = Some(path);
    }
    Ok(ViewArtifactRef { component, assets })
}

/// Canonical logical paths use nonempty UTF-8 slash-separated segments.
/// Empty, dot and parent segments, backslashes, colons and controls are refused.
/// No filesystem normalization or extraction is performed.
pub fn validate_asset_path(path: &str) -> Result<(), String> {
    let bad_length = path.is_empty() || path.len() > MAX_ASSET_PATH_BYTES;
    if bad_length {
        return Err("module artifact asset path exceeds its length bounds".into());
    }
    let bad_character = path
        .chars()
        .any(|c| c.is_control() || matches!(c, '\\' | ':'));
    let bad_segment = path.split('/').any(|part| matches!(part, "" | "." | ".."));
    if bad_character || bad_segment {
        return Err("module artifact asset path is not a bounded canonical relative path".into());
    }
    Ok(())
}

fn take_tag(bytes: &mut &[u8]) -> Result<u8, String> {
    let Some((&tag, tail)) = bytes.split_first() else {
        return Err("module artifact has a missing option tag".into());
    };
    *bytes = tail;
    Ok(tag)
}

fn take_length(bytes: &mut &[u8]) -> Result<usize, String> {
    let Some((length, tail)) = bytes.split_at_checked(4) else {
        return Err("module artifact has a truncated length".into());
    };
    *bytes = tail;
    Ok(u32::from_le_bytes(length.try_into().expect("four-byte length")) as usize)
}

fn take_bytes<'a>(bytes: &mut &'a [u8]) -> Result<&'a [u8], String> {
    let length = take_length(bytes)?;
    let Some((value, tail)) = bytes.split_at_checked(length) else {
        return Err("module artifact has a truncated body".into());
    };
    *bytes = tail;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn viewed() -> ModuleArtifact {
        ModuleArtifact {
            component: vec![1],
            index: Some(vec![2]),
            view: Some(ViewArtifact {
                component: vec![3],
                assets: BTreeMap::from([("icons/mark.svg".into(), vec![4, 5])]),
            }),
        }
    }

    // A Vec of pairs has the map's Borsh framing but lets hostile tests supply
    // duplicate or out-of-order keys that BTreeMap itself cannot produce.
    fn raw_view(assets: Vec<(String, Vec<u8>)>) -> Vec<u8> {
        borsh::to_vec(&(vec![1u8], None::<Vec<u8>>, Some((vec![2u8], assets)))).unwrap()
    }

    #[test]
    fn view_and_assets_are_one_commitment_and_borrow_the_frame() {
        let original = viewed();
        let encoded = original.encode();
        assert_eq!(ModuleArtifact::decode(&encoded).unwrap(), original);
        let borrowed = ModuleArtifactRef::decode(&encoded).unwrap();
        assert_eq!(borrowed.component, [1]);
        assert_eq!(borrowed.index.unwrap(), [2]);
        let view = borrowed.view.unwrap();
        assert_eq!(view.component, [3]);
        assert_eq!(view.assets["icons/mark.svg"], [4, 5]);
        let frame = encoded.as_ptr() as usize..encoded.as_ptr() as usize + encoded.len();
        assert!(frame.contains(&(view.component.as_ptr() as usize)));
        let (key, value) = view.assets.first_key_value().unwrap();
        assert!(frame.contains(&(key.as_ptr() as usize)));
        assert!(frame.contains(&(value.as_ptr() as usize)));
        let mut variants = Vec::new();
        let mut changed = original.clone();
        changed.view.as_mut().unwrap().component.push(6);
        variants.push(changed);
        let mut changed = original.clone();
        changed
            .view
            .as_mut()
            .unwrap()
            .assets
            .get_mut("icons/mark.svg")
            .unwrap()
            .push(6);
        variants.push(changed);
        let mut changed = original.clone();
        changed.view.as_mut().unwrap().assets =
            BTreeMap::from([("icons/other.svg".into(), vec![4, 5])]);
        variants.push(changed);
        let mut changed = original.clone();
        changed.view.as_mut().unwrap().assets.clear();
        variants.push(changed);
        let mut removed = original.clone();
        removed.view = None;
        variants.push(removed);
        for variant in variants {
            assert_ne!(
                variant.hash(),
                original.hash(),
                "view change escaped commitment"
            );
            assert_eq!(ModuleArtifact::decode(&variant.encode()).unwrap(), variant);
        }
        let mut empty_view = original.clone();
        empty_view.view = Some(ViewArtifact {
            component: Vec::new(),
            assets: BTreeMap::new(),
        });
        assert!(
            ModuleArtifact::decode(&empty_view.encode())
                .unwrap()
                .view
                .is_some(),
            "only None is removal"
        );
    }

    #[test]
    fn only_canonical_bounded_asset_paths_are_admitted() {
        for path in [
            "", "/a", "a/", "a//b", ".", "..", "a/./b", "a/../b", "a\\b", "C:a", "a\0b", "a\nb",
        ] {
            assert!(
                ModuleArtifactRef::decode(&raw_view(vec![(path.into(), vec![])])).is_err(),
                "accepted path {path:?}"
            );
        }
        let longest = "a".repeat(MAX_ASSET_PATH_BYTES);
        for path in ["icons/마크.svg", longest.as_str()] {
            assert!(ModuleArtifactRef::decode(&raw_view(vec![(path.into(), vec![])])).is_ok());
        }
        assert!(
            ModuleArtifactRef::decode(&raw_view(vec![(
                "a".repeat(MAX_ASSET_PATH_BYTES + 1),
                vec![]
            )]))
            .is_err()
        );
        for keys in [["a", "a"], ["b", "a"]] {
            let bytes = raw_view(keys.into_iter().map(|key| (key.into(), vec![])).collect());
            assert!(
                ModuleArtifactRef::decode(&bytes).is_err(),
                "accepted noncanonical key order {keys:?}"
            );
        }
    }

    #[test]
    fn asset_count_is_bounded_before_reading_entries() {
        let bytes = borsh::to_vec(&(vec![1u8], None::<Vec<u8>>, 1u8, vec![2u8], u32::MAX)).unwrap();
        let error = ModuleArtifactRef::decode(&bytes).unwrap_err();
        assert!(
            error.contains("4096 view assets"),
            "must reject count before entries: {error}"
        );
        let assets: Vec<_> = (0..MAX_VIEW_ASSETS)
            .map(|i| (format!("{i:04}"), vec![]))
            .collect();
        assert_eq!(
            ModuleArtifactRef::decode(&raw_view(assets))
                .unwrap()
                .view
                .unwrap()
                .assets
                .len(),
            MAX_VIEW_ASSETS
        );
    }

    #[test]
    fn total_frame_limit_includes_all_encoded_bytes() {
        // Four-byte component length plus the two absent-option tags.
        let exact = ModuleArtifact::component(vec![0; MAX_ARTIFACT_BYTES - 6]).encode();
        assert_eq!(exact.len(), MAX_ARTIFACT_BYTES);
        assert!(ModuleArtifactRef::decode(&exact).is_ok());
        let oversized = ModuleArtifact::component(vec![0; MAX_ARTIFACT_BYTES - 5]).encode();
        assert!(
            ModuleArtifactRef::decode(&oversized).is_err(),
            "oversized frame accepted"
        );
        let mut aggregate = viewed();
        aggregate
            .view
            .as_mut()
            .unwrap()
            .assets
            .insert("large".into(), vec![0; MAX_ARTIFACT_BYTES]);
        assert!(
            ModuleArtifactRef::decode(&aggregate.encode()).is_err(),
            "aggregate view bytes escaped frame limit"
        );
    }

    #[test]
    fn hostile_view_lengths_tags_and_previous_format_are_rejected() {
        let bytes = viewed().encode();
        for end in 0..bytes.len() {
            assert!(
                ModuleArtifactRef::decode(&bytes[..end]).is_err(),
                "accepted truncated frame at {end}"
            );
        }
        let old = borsh::to_vec(&(vec![1u8], None::<Vec<u8>>)).unwrap();
        assert!(
            ModuleArtifactRef::decode(&old).is_err(),
            "previous artifact format accepted"
        );
        let mut invalid = raw_view(vec![("a".into(), vec![])]);
        invalid[6] = 2; // view option tag after component and mapper tag
        assert!(ModuleArtifactRef::decode(&invalid).is_err());
        let mut invalid = raw_view(vec![("a".into(), vec![])]);
        invalid[20] = 255; // asset path's first byte
        assert!(
            ModuleArtifactRef::decode(&invalid).is_err(),
            "invalid UTF-8 path accepted"
        );
        assert!(
            ModuleArtifactRef::decode(&[255; 4]).is_err(),
            "hostile length accepted"
        );
        assert!(
            ModuleArtifactRef::decode(&viewed().encode()).is_ok(),
            "refusals poisoned next decode"
        );
    }

    #[test]
    fn a_canonical_view_frame_is_accepted() {
        let assets = std::collections::BTreeMap::from([("icons/mark.svg", vec![4u8, 5])]);
        let bytes =
            borsh::to_vec(&(vec![1u8], None::<Vec<u8>>, Some((vec![2u8], assets)))).unwrap();
        assert!(
            ModuleArtifactRef::decode(&bytes).is_ok(),
            "canonical view artifact must decode"
        );
    }

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
