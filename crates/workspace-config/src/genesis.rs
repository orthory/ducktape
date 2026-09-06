//! The genesis file — `<workspace>/genesis` — and the founding set it is
//! composed from.
//!
//! A network's wasm is the network's: the genesis carries every consensus
//! component and every index guest a node on that network runs, and the
//! workspace's first boot installs them from here (components into the blob
//! store, index guests into the index databases) and unpacks them as bare
//! files into `<workspace>/modules` (founding-set layout — the readable copy
//! of what the network runs). Nothing outside the workspace decides what its
//! network runs, so two networks carry two sets.
//!
//! The descriptor pins it twice: `network.toml`'s `genesis` is the sha256 of
//! this whole file (in the genesis fingerprint — the network IS its genesis)
//! and its `modules` table names each component's own hash (what the code
//! registry seeds at block zero). A joiner verifies both before a byte of it
//! is installed.
//!
//! The FOUNDING SET is the one place bare wasm files are read: a directory of
//! `<id>.component.wasm`, `<id>.index.wasm` and `netstack.component.wasm` the
//! build stages beside its binaries (`crates/noded/build.rs`), read by `node
//! init` to compose a genesis and by the daemons that run no network (noded,
//! simnode, the dev shape) to compose directly. It must hold a component for
//! every wasm id of the selection — one with no file is a refusal naming the
//! path — and an index guest for exactly the modules that ship one: the
//! build stages a module's `<id>.index.wasm` iff its crate declares the
//! guest, so the file's presence IS the declaration.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use borsh::{BorshDeserialize, BorshSerialize};

use crate::hex_bytes;

/// the genesis file's name inside a workspace.
pub const GENESIS_FILE: &str = "genesis";

/// the netstack guest's file name inside a founding set: the reachability
/// machine as a `ducktape:netstack` component. Not a genesis artifact — a
/// joiner runs it to reach the mesh before it holds any genesis — so it is
/// read from the founding set at plane wiring, never from the genesis.
pub const NETSTACK_COMPONENT_FILE: &str = "netstack.component.wasm";

/// `<dir>/genesis`.
pub fn genesis_path(dir: &Path) -> PathBuf {
    dir.join(GENESIS_FILE)
}

/// `<dir>/modules` — where a workspace's genesis is unpacked into bare files
/// ([`Genesis::materialize`]), in founding-set layout: the network's wasm,
/// readable on disk beside `network.toml`.
pub fn modules_path(dir: &Path) -> PathBuf {
    dir.join("modules")
}

/// `<dir>/<id>.component.wasm` — a consensus component in a founding set.
pub fn component_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.component.wasm"))
}

/// `<dir>/<id>.index.wasm` — a module's index guest in a founding set.
pub fn index_guest_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.index.wasm"))
}

/// `<dir>/netstack.component.wasm` — the netstack guest in a founding set.
pub fn netstack_component_path(dir: &Path) -> PathBuf {
    dir.join(NETSTACK_COMPONENT_FILE)
}

/// one wasm artifact a genesis carries, by the module id it belongs to.
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Artifact {
    pub id: String,
    pub bytes: Vec<u8>,
}

/// the genesis: every wasm a node on this network runs. Both lists are
/// id-sorted, so the same set always encodes to the same bytes and the whole
/// file has one hash.
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Genesis {
    /// the consensus components, one per wasm tenant of the genesis set.
    pub components: Vec<Artifact>,
    /// the index guests, one per module of the set that ships a mapper.
    pub index_guests: Vec<Artifact>,
}

impl Genesis {
    /// compose a genesis from the founding set at `source`: every id in
    /// `wasm_ids` must have its component there, and the index guests are
    /// the `<id>.index.wasm` files the set holds for those ids. The walk is
    /// by sorted id, so an incomplete set always names the same missing file
    /// first.
    pub fn compose(source: &Path, wasm_ids: &[&str]) -> Result<Self, String> {
        let components = read_artifacts(wasm_ids, |id| component_path(source, id))?;
        let index_guests = read_present_artifacts(wasm_ids, |id| index_guest_path(source, id))?;
        Ok(Self {
            components,
            index_guests,
        })
    }

    /// unpack every artifact into `dir` in founding-set layout
    /// (`<id>.component.wasm`, `<id>.index.wasm`): the genesis's readable
    /// twin on disk. Rewritten in full on every call (each file atomically),
    /// so the directory always says what the genesis says — a founding set
    /// composed from it yields this genesis again.
    pub fn materialize(&self, dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        for a in &self.components {
            write_atomic(&component_path(dir, &a.id), &a.bytes)?;
        }
        for a in &self.index_guests {
            write_atomic(&index_guest_path(dir, &a.id), &a.bytes)?;
        }
        Ok(())
    }

    /// the file's bytes.
    pub fn encode(&self) -> Vec<u8> {
        borsh::to_vec(self).expect("a genesis serializes")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let genesis: Self =
            borsh::from_slice(bytes).map_err(|e| format!("genesis file does not decode: {e}"))?;
        let components_sorted = genesis.components.windows(2).all(|w| w[0].id < w[1].id);
        let index_guests_sorted = genesis.index_guests.windows(2).all(|w| w[0].id < w[1].id);
        let canonical = components_sorted && index_guests_sorted;
        if !canonical {
            return Err("genesis file is not canonical (ids unsorted or duplicated)".into());
        }
        Ok(genesis)
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        Self::decode(&bytes).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// id → sha256 of each component: the descriptor's `modules` table.
    pub fn component_hashes(&self) -> BTreeMap<String, [u8; 32]> {
        self.components
            .iter()
            .map(|a| (a.id.clone(), sha256(&a.bytes)))
            .collect()
    }

    pub fn component(&self, id: &str) -> Option<&[u8]> {
        self.components
            .iter()
            .find(|a| a.id == id)
            .map(|a| a.bytes.as_slice())
    }

    pub fn index_guest(&self, id: &str) -> Option<&[u8]> {
        self.index_guests
            .iter()
            .find(|a| a.id == id)
            .map(|a| a.bytes.as_slice())
    }

    /// the components are exactly the descriptor's module set (`want`: id →
    /// `code_hash`), byte for byte: the same ids, each hashing to its entry.
    /// A mismatch names the module, because the operator's next move is to
    /// look at it.
    pub fn verify_components(&self, want: &BTreeMap<String, [u8; 32]>) -> Result<(), String> {
        let have = self.component_hashes();
        for (id, hash) in want {
            match have.get(id) {
                None => {
                    return Err(format!(
                        "module {id}: the genesis carries no component for it"
                    ));
                }
                Some(got) if got != hash => {
                    return Err(format!(
                        "module {id}: the genesis component hashes to {} but network.toml says {}",
                        hex_bytes(got),
                        hex_bytes(hash)
                    ));
                }
                Some(_) => {}
            }
        }
        if let Some(extra) = have.keys().find(|id| !want.contains_key(*id)) {
            return Err(format!(
                "module {extra}: the genesis carries a component network.toml does not name"
            ));
        }
        Ok(())
    }
}

/// sha256 of a genesis file's bytes — the descriptor's `genesis` pin.
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    use sha2::Digest as _;
    sha2::Sha256::digest(bytes).into()
}

/// decode `bytes` as the genesis the descriptor pins: the whole file must
/// hash to `hash` (the descriptor's `genesis`) and its components must be
/// `modules` (the descriptor's table, id → `code_hash`). Every reader of a
/// genesis that did not compose it goes through here — a joiner installing
/// a handed-out or fetched file, a member reopening its own.
pub fn verify_genesis(
    bytes: &[u8],
    hash: &[u8; 32],
    modules: &BTreeMap<String, [u8; 32]>,
) -> Result<Genesis, String> {
    let got = sha256(bytes);
    let is_the_networks_genesis = got == *hash;
    if !is_the_networks_genesis {
        return Err(format!(
            "genesis hashes to {} but network.toml says {} — this is not the network's genesis",
            hex_bytes(&got),
            hex_bytes(hash)
        ));
    }
    let genesis = Genesis::decode(bytes)?;
    genesis.verify_components(modules)?;
    Ok(genesis)
}

/// install a genesis into `dir` from `bytes` (a file the founder handed out,
/// or the blob a member served), verified by [`verify_genesis`] first.
/// Nothing is written until it holds; the write is atomic (tmp + rename), so
/// a workspace never holds a half-written genesis.
pub fn install_genesis(
    dir: &Path,
    hash: &[u8; 32],
    modules: &BTreeMap<String, [u8; 32]>,
    bytes: &[u8],
) -> Result<Genesis, String> {
    let genesis = verify_genesis(bytes, hash, modules)?;
    write_atomic(&genesis_path(dir), bytes)?;
    Ok(genesis)
}

fn sorted_ids(ids: &[&str]) -> Vec<String> {
    let mut ids: Vec<String> = ids.iter().map(|id| (*id).to_string()).collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// one artifact per id; a missing file is a refusal naming its path.
fn read_artifacts(
    ids: &[&str],
    path_of: impl Fn(&str) -> PathBuf,
) -> Result<Vec<Artifact>, String> {
    sorted_ids(ids)
        .into_iter()
        .map(|id| {
            let path = path_of(&id);
            let bytes =
                std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
            Ok(Artifact { id, bytes })
        })
        .collect()
}

/// the artifacts the set HOLDS for `ids`: an absent file is not an artifact
/// (a module without an index guest ships none); any other read failure
/// still names its path.
fn read_present_artifacts(
    ids: &[&str],
    path_of: impl Fn(&str) -> PathBuf,
) -> Result<Vec<Artifact>, String> {
    let mut artifacts = Vec::new();
    for id in sorted_ids(ids) {
        let path = path_of(&id);
        match std::fs::read(&path) {
            Ok(bytes) => artifacts.push(Artifact { id, bytes }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("read {}: {e}", path.display())),
        }
    }
    Ok(artifacts)
}

/// write `bytes` to `path` via tmp-file + rename: a reader never observes a
/// truncated or partial file, only the old contents or the new ones. Shared
/// with [`crate::NetworkDescriptor::save`] — the descriptor is as much a
/// workspace identity file as the genesis is.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    std::fs::write(&tmp, bytes).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .map_err(|e| format!("rename {} -> {}: {e}", tmp.display(), path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ModuleCode, NetworkDescriptor};

    fn founding_set(dir: &Path, components: &[(&str, &[u8])], guests: &[(&str, &[u8])]) {
        for (id, bytes) in components {
            std::fs::write(component_path(dir, id), bytes).unwrap();
        }
        for (id, bytes) in guests {
            std::fs::write(index_guest_path(dir, id), bytes).unwrap();
        }
    }

    fn descriptor_for(genesis: &Genesis) -> NetworkDescriptor {
        let bytes = genesis.encode();
        NetworkDescriptor {
            chain_id: "net#a1b2c3d4".into(),
            validators: vec![],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
            block_time_ms: crate::DEFAULT_BLOCK_TIME_MS,
            genesis: hex_bytes(&sha256(&bytes)),
            modules: genesis
                .component_hashes()
                .iter()
                .map(|(id, h)| ModuleCode {
                    id: id.clone(),
                    code_hash: hex_bytes(h),
                })
                .collect(),
        }
    }

    #[test]
    fn compose_reads_the_declared_set_sorted_and_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        founding_set(
            dir.path(),
            &[("pages", b"P"), ("chat", b"C")],
            &[("chat", b"c-map")],
        );
        let genesis = Genesis::compose(dir.path(), &["pages", "chat"]).unwrap();
        let ids: Vec<&str> = genesis.components.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, ["chat", "pages"], "id-sorted regardless of selection order");
        assert_eq!(genesis.component("pages"), Some(&b"P"[..]));
        assert_eq!(genesis.index_guest("chat"), Some(&b"c-map"[..]));
        assert_eq!(genesis.index_guest("pages"), None);
        let bytes = genesis.encode();
        assert_eq!(Genesis::decode(&bytes).unwrap(), genesis);
        assert_eq!(genesis.encode(), bytes, "canonical: same set, same bytes");
    }

    #[test]
    fn materialize_writes_a_founding_set_that_composes_back() {
        let src = tempfile::tempdir().unwrap();
        founding_set(
            src.path(),
            &[("pages", b"P"), ("chat", b"C")],
            &[("chat", b"c-map")],
        );
        let genesis = Genesis::compose(src.path(), &["pages", "chat"]).unwrap();

        let ws = tempfile::tempdir().unwrap();
        let modules = modules_path(ws.path());
        // a stale file from an older genesis is overwritten, never kept
        std::fs::create_dir_all(&modules).unwrap();
        std::fs::write(component_path(&modules, "pages"), b"old").unwrap();
        genesis.materialize(&modules).unwrap();
        assert_eq!(
            std::fs::read(component_path(&modules, "pages")).unwrap(),
            b"P"
        );
        assert_eq!(
            std::fs::read(index_guest_path(&modules, "chat")).unwrap(),
            b"c-map"
        );
        let again = Genesis::compose(&modules, &["pages", "chat"]).unwrap();
        assert_eq!(again, genesis, "the directory is the genesis, unpacked");
    }

    /// a component is owed for every id; an index guest only for the ids
    /// whose file the set holds — its absence is a module that ships none.
    #[test]
    fn compose_refuses_a_missing_component_by_path_and_takes_the_index_guests_present() {
        let dir = tempfile::tempdir().unwrap();
        founding_set(dir.path(), &[("pages", b"P")], &[]);
        let err = Genesis::compose(dir.path(), &["pages", "chat"]).unwrap_err();
        assert!(err.contains("chat.component.wasm"), "{err}");
        let genesis = Genesis::compose(dir.path(), &["pages"]).unwrap();
        assert!(genesis.index_guests.is_empty(), "no file, no guest");
    }

    #[test]
    fn decode_refuses_a_non_canonical_file() {
        let unsorted = Genesis {
            components: vec![
                Artifact {
                    id: "pages".into(),
                    bytes: vec![1],
                },
                Artifact {
                    id: "chat".into(),
                    bytes: vec![2],
                },
            ],
            index_guests: vec![],
        };
        let err = Genesis::decode(&unsorted.encode()).unwrap_err();
        assert!(err.contains("canonical"), "{err}");
    }

    #[test]
    fn install_verifies_the_whole_file_then_every_component() {
        let dir = tempfile::tempdir().unwrap();
        let genesis = Genesis {
            components: vec![Artifact {
                id: "pages".into(),
                bytes: vec![1, 2, 3],
            }],
            index_guests: vec![Artifact {
                id: "pages".into(),
                bytes: vec![9],
            }],
        };
        let descriptor = descriptor_for(&genesis);
        let bytes = genesis.encode();
        let hash = descriptor.genesis_hash().unwrap();
        let modules = descriptor.module_hashes().unwrap();

        let installed = install_genesis(dir.path(), &hash, &modules, &bytes).unwrap();
        assert_eq!(installed, genesis);
        assert_eq!(std::fs::read(genesis_path(dir.path())).unwrap(), bytes);

        // a different file: refused by the whole-file hash, nothing written.
        let other = tempfile::tempdir().unwrap();
        let mut tampered = bytes.clone();
        tampered.push(0);
        let err = install_genesis(other.path(), &hash, &modules, &tampered).unwrap_err();
        assert!(err.contains("not the network's genesis"), "{err}");
        assert!(!genesis_path(other.path()).exists());

        // the right file against a descriptor whose module table disagrees:
        // refused by module name.
        let mut wrong_modules = modules.clone();
        wrong_modules.insert("pages".into(), [0u8; 32]);
        let err = genesis.verify_components(&wrong_modules).unwrap_err();
        assert!(err.starts_with("module pages:"), "{err}");
        let mut extra = modules.clone();
        extra.insert("chat".into(), [0u8; 32]);
        let err = genesis.verify_components(&extra).unwrap_err();
        assert!(err.contains("no component"), "{err}");
    }
}
