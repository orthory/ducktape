//! the pieces every ducktape process needs around [`compose`](crate::compose):
//! where component bytes come from on disk, where stores come from, and how a
//! composed set becomes a `Host`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use commonware_runtime::Supervisor as _;
use host::Host;
use statesync::qmdb::QmdbStore;
use wasm_host::WasmModule;

use crate::compose::BoxFut;

/// `<dir>/<id>.component.wasm` — the founding-set naming convention (the
/// kernel fixtures and the staged set alike), owned by the workspace crate
/// beside the genesis it composes.
pub use workspace_config::component_path;

/// sha256 every `<id>.component.wasm` in `dir` for `ids`; a missing file names
/// its path, because the operator's next move is to look at that directory.
///
/// The walk is BY ID, not in the caller's selection order, so a bundle missing
/// several components always names the same one first — the operator fixes a
/// stable list instead of chasing a topology-order lottery one file at a time.
pub fn hash_bundle(dir: &Path, ids: &[&str]) -> Result<BTreeMap<String, [u8; 32]>, String> {
    use sha2::Digest as _;
    let mut ids = ids.to_vec();
    ids.sort_unstable();
    let mut out = BTreeMap::new();
    for id in &ids {
        let path = component_path(dir, id);
        let bytes = std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        out.insert((*id).to_string(), sha2::Sha256::digest(&bytes).into());
    }
    Ok(out)
}

/// a [`host::CodeSource`] over a directory of `<id>.component.wasm`, keyed by
/// each file's sha256. a hash the directory has no file for is a `None` — the
/// boundary fails closed, and the composer re-hashes whatever bytes come back
/// regardless (a dir keyed by filename is a lookup, not a guarantee).
pub struct DirCodeSource {
    /// the directory `<id>.component.wasm` files are read from.
    dir: PathBuf,
    /// content hash → the module id whose file carries those bytes.
    by_hash: BTreeMap<[u8; 32], String>,
}

impl DirCodeSource {
    /// hash every `<id>.component.wasm` for `ids` (the selection's wasm ids);
    /// returns the source and the id → hash map `Bindings::code_hashes` wants.
    pub fn open(dir: &Path, ids: &[&str]) -> Result<(Self, BTreeMap<String, [u8; 32]>), String> {
        let by_id = hash_bundle(dir, ids)?;
        let by_hash = by_id.iter().map(|(id, h)| (*h, id.clone())).collect();
        Ok((
            Self {
                dir: dir.to_path_buf(),
                by_hash,
            },
            by_id,
        ))
    }
}

#[async_trait::async_trait(?Send)]
impl host::CodeSource for DirCodeSource {
    async fn fetch(&self, code_hash: &[u8]) -> Option<Vec<u8>> {
        let digest: [u8; 32] = code_hash.try_into().ok()?;
        let id = self.by_hash.get(&digest)?;
        std::fs::read(component_path(&self.dir, id)).ok()
    }
}

/// the canonical store source: every store-backed module `init`s its qmdb
/// store under its own id in this runtime's storage root — fresh at genesis,
/// reopened at its committed position on restore.
pub fn qmdb_stores<'a>(
    context: &'a commonware_runtime::tokio::Context,
) -> impl FnMut(&'static str) -> BoxFut<'a, Result<Box<dyn sdk::MerkleStore>, String>> + 'a {
    move |id: &'static str| -> BoxFut<'a, Result<Box<dyn sdk::MerkleStore>, String>> {
        let child = context.child(id);
        Box::pin(async move {
            Ok(Box::new(QmdbStore::init(child, id).await) as Box<dyn sdk::MerkleStore>)
        })
    }
}

/// the wasm-runtime [`host::ModuleFactory`]: a post-genesis ADMISSION
/// (governance `RegisterModule` → lifecycle `ScheduleRegister`) instantiates its
/// module from the verified component bytes at the activation boundary — the
/// constructor twin of the code source.
pub struct WasmModuleFactory;

impl host::ModuleFactory for WasmModuleFactory {
    fn instantiate(&self, id: &str, bytes: &[u8]) -> Result<Box<dyn sdk::Module>, sdk::Error> {
        Ok(Box::new(WasmModule::from_bytes(id, bytes)?))
    }
}

/// compose the module set into a [`Host`]. registration order is NOT
/// consensus-relevant (the host keys modules in a `BTreeMap`) — only the
/// module set and each module's constructed state compose the root-hash.
/// every production host admits post-genesis modules through the wasm
/// runtime — genesis, restore, and statesync compositions alike.
pub fn host_from(modules: Vec<Box<dyn sdk::Module>>) -> Result<Host, sdk::Error> {
    let mut host = Host::genesis(modules)?;
    host.set_module_factory(Box::new(WasmModuleFactory));
    Ok(host)
}
