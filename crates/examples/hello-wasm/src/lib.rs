//! `hello-wasm` — the reference wasm module. Pure logic over the host surface:
//! a counter whose durable state lives entirely behind `host.state-*`, so the
//! host owns the store and computes root(). Authored against `ducktape:module`.

wit_bindgen::generate!({
    world: "module",
    path: "../../kernel/module-guest/wit",
});

use ducktape::module::host;

struct Component;

const COUNT_KEY: &[u8] = b"count";

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
    fn execute(payload: Vec<u8>) -> Result<(), host::Error> {
        // env is available (proves the import); this module doesn't branch on it.
        let _env = host::get_env();
        match payload.as_slice() {
            b"inc" => {
                let n = read_count().wrapping_add(1);
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
