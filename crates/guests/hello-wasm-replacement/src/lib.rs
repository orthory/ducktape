//! `hello-wasm-replacement` — the replacement reference wasm module. It uses
//! the same durable state as `hello-wasm` (the `count` key, little-endian u64), but DIFFERENT
//! logic: `inc` steps the counter by `STEP` (100) instead of 1. Authored against
//! the same `ducktape:module` world, so it is a drop-in code swap over a store
//! that `hello-wasm` populated — the host keeps the store, only the logic changes.

wit_bindgen::generate!({
    world: "module",
    path: "../../module-sdk/wit",
});

use ducktape::module::host;

struct Component;

const COUNT_KEY: &[u8] = b"count";
/// the replacement step — the one observable difference from `hello-wasm` (which steps 1).
const STEP: u64 = 100;

fn read_count() -> u64 {
    match host::state_get(COUNT_KEY) {
        Some(v) if v.len() == 8 => {
            let mut b = [0u8; 8];
            b.copy_from_slice(&v);
            u64::from_le_bytes(b)
        }
        _ => 0,
    }
}

impl Guest for Component {
    /// the same shape as `hello-wasm`: a code swap keeps the store, so the
    /// replacement must declare the backing that store is.
    fn shape() -> host::ModuleShape {
        host::ModuleShape {
            backing: host::Backing::Map,
            config: Vec::new(),
            committed_queries: false,
        }
    }

    fn execute(payload: Vec<u8>) -> Result<(), host::Error> {
        let _env = host::get_env();
        match payload.as_slice() {
            b"inc" => {
                let n = read_count().wrapping_add(STEP);
                host::state_set(COUNT_KEY, &n.to_le_bytes());
                host::emit_event("hello", &n.to_le_bytes());
                Ok(())
            }
            b"reset" => {
                host::state_set(COUNT_KEY, &0u64.to_le_bytes());
                Ok(())
            }
            other => Err(host::Error::Rejected(format!(
                "unknown op ({} bytes)",
                other.len()
            ))),
        }
    }

    fn query(_req: Vec<u8>) -> Result<Vec<u8>, host::Error> {
        Ok(read_count().to_le_bytes().to_vec())
    }
}

export!(Component);
