//! `hello-wasm` — the reference wasm module. Pure logic over the host surface:
//! a counter whose durable state lives entirely behind `host.state-*`, so the
//! host owns the store and computes root(). Authored against `ducktape:module`.

wit_bindgen::generate!({
    world: "module",
    path: "../../module-sdk/wit",
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
    fn initialize(_params: Vec<u8>) -> Result<(), host::Error> {
        Ok(())
    }

    fn finalize_block() -> Result<(), host::Error> {
        Ok(())
    }

    /// the declared shape: plain host-KV keys over a map the host owns, no
    /// network config, read-your-writes queries.
    fn pending_items() -> Result<Vec<host::PendingItem>, host::Error> {
        Ok(Vec::new())
    }

    fn acknowledge(_ack: host::Ack) -> Result<(), host::Error> {
        Err(host::Error::Rejected("module has no outbound queue".into()))
    }

    fn shape() -> host::ModuleShape {
        host::ModuleShape {
            backing: host::Backing::Map,
            config: Vec::new(),
            committed_queries: false,
        }
    }

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
            b"output-cap"
            | b"output-cap-then-small"
            | b"output-cap-then-error"
            | b"assigned-cap"
            | b"declarations-valid" => {
                host::state_set(COUNT_KEY, &1u64.to_le_bytes());
                match payload.as_slice() {
                    b"output-cap" => host::set_output(&vec![0; 256 * 1024 + 1]),
                    b"output-cap-then-small" => {
                        host::set_output(&vec![0; 256 * 1024 + 1]);
                        host::set_output(b"small");
                    }
                    b"output-cap-then-error" => {
                        host::set_output(&vec![0; 256 * 1024 + 1]);
                        return Err(host::Error::Rejected("explicit refusal".into()));
                    }
                    b"assigned-cap" => host::set_assigned(&vec![0; 4 * 1024 + 1]),
                    b"declarations-valid" => {
                        host::set_output(b"result");
                        host::set_assigned(b"stamp");
                    }
                    _ => unreachable!(),
                }
                Ok(())
            }
            b"module-error" => Err(host::Error::Rejected("explicit refusal".into())),
            b"self-query" => host::query_module("hello", b"").map(|_| ()),
            b"missing-query" => host::query_module("missing", b"").map(|_| ()),
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
