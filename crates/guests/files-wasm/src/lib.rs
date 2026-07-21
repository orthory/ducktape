//! `files-wasm` — the wasm port of the `files` module, built the ADAPTER way:
//! the NATIVE `files` crate's guest surface ([`files::FilesGuest`]) is compiled
//! to wasm32 and adapted to the `ducktape:module` world through `guest-adapter`,
//! so the op semantics are single-sourced (a change to the native `Fs` verbs IS
//! the wasm change).
//!
//! files is the ROOT-CONTINUOUS odb tenant, so this port is unusually thin: the
//! host owns the whole committed surface (`root`/`query`/`snapshot`/`install`/
//! `serve_sync` + the object plane) through the kernel's [`StateBacking::Odb`]
//! backing (`files::FilesOdbBacking`), and the guest owns ONLY `execute`.
//! [`FilesGuest`] loads the refs image + block-object index from the host state
//! lane, runs the SAME `Fs` verb the native module's `execute` calls, and
//! re-stages the new refs image — the host publishes it at the real block
//! boundary in duckfs's durability order. `query` is UNREACHABLE here: the kernel
//! serves it host-side from committed refs + the disk odb without instantiating
//! the guest (body reads cannot happen in a sealed round), so the forward exists
//! only to satisfy the `Guest` export and fails loud if ever reached.
//!
//! [`StateBacking::Odb`]: wasm_host::WasmModule::with_odb

use files::FilesGuest;
use guest_adapter::{host, Guest};

struct Component;

impl Guest for Component {
    fn execute(payload: Vec<u8>) -> Result<(), host::Error> {
        FilesGuest::execute(payload)
    }

    fn query(req: Vec<u8>) -> Result<Vec<u8>, host::Error> {
        FilesGuest::query(req)
    }
}

guest_adapter::export_module!(Component);
