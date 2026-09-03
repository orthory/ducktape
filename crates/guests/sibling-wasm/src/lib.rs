//! `sibling-wasm` — the cross-module-read reference guest: every op exercises
//! the `module-root` / `query-module` host surface, so the kernel tests can
//! prove the memoized-replay machinery (convergence, intra-dispatch memo hits,
//! staging rollback across replay rounds, and the sibling-read budget).

wit_bindgen::generate!({
    world: "module",
    path: "../../kernel/module-guest/wit",
});

use ducktape::module::host;

struct Component;

const COUNT_KEY: &[u8] = b"count";
const LAST_KEY: &[u8] = b"last";
const ROOT_KEY: &[u8] = b"root";

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

/// split `target ':' req` — the test wire shape for addressing a sibling.
fn split_target(rest: &[u8]) -> Result<(String, &[u8]), host::Error> {
    let sep = rest
        .iter()
        .position(|&b| b == b':')
        .ok_or_else(|| host::Error::Rejected("missing ':' separator".into()))?;
    let target = String::from_utf8(rest[..sep].to_vec())
        .map_err(|_| host::Error::Rejected("target is not utf-8".into()))?;
    Ok((target, &rest[sep + 1..]))
}

impl Guest for Component {
    fn shape() -> host::ModuleShape {
        host::ModuleShape {
            backing: host::Backing::Map,
            config: Vec::new(),
            committed_queries: false,
        }
    }

    fn execute(payload: Vec<u8>) -> Result<(), host::Error> {
        match payload.split_first() {
            // 'q' target ':' req — read-modify-write the own counter BEFORE the
            // sibling query (the staging-rollback probe: a write leaked from an
            // aborted replay round would double-apply it), query the sibling
            // TWICE with the same request (the intra-dispatch memo probe), and
            // store the answer.
            Some((b'q', rest)) => {
                let (target, req) = split_target(rest)?;
                let n = read_count().wrapping_add(1);
                host::state_set(COUNT_KEY, &n.to_le_bytes());
                let first = host::query_module(&target, req)?;
                let second = host::query_module(&target, req)?;
                if first != second {
                    return Err(host::Error::Rejected("memo answers diverged".into()));
                }
                host::state_set(LAST_KEY, &first);
                Ok(())
            }
            // 'r' target — store the sibling's dispatch-start snapshot root.
            Some((b'r', rest)) => {
                let target = String::from_utf8(rest.to_vec())
                    .map_err(|_| host::Error::Rejected("target is not utf-8".into()))?;
                match host::module_root(&target) {
                    Some(root) => {
                        host::state_set(ROOT_KEY, &root);
                        Ok(())
                    }
                    None => Err(host::Error::NotFound),
                }
            }
            // 'f' + le-u64 count — perform `count` DISTINCT queries: the
            // sibling-read budget probe.
            Some((b'f', rest)) => {
                let n = match rest.len() {
                    8 => {
                        let mut b = [0u8; 8];
                        b.copy_from_slice(rest);
                        u64::from_le_bytes(b)
                    }
                    _ => return Err(host::Error::Rejected("count must be 8 bytes".into())),
                };
                for i in 0..n {
                    let _ = host::query_module("noisy", &i.to_le_bytes())?;
                }
                Ok(())
            }
            _ => Err(host::Error::Rejected("unknown op".into())),
        }
    }

    fn query(req: Vec<u8>) -> Result<Vec<u8>, host::Error> {
        match req.split_first() {
            // 'l' — the stored answer of the last 'q' op.
            Some((b'l', _)) => Ok(host::state_get(LAST_KEY).unwrap_or_default()),
            // 'q' target ':' req — live sibling passthrough (the query_with probe).
            Some((b'q', rest)) => {
                let (target, inner) = split_target(rest)?;
                host::query_module(&target, inner)
            }
            _ => Err(host::Error::Rejected("unknown query".into())),
        }
    }
}

export!(Component);
