//! `object-wasm` — the object-plane reference guest: every op exercises the
//! `object-put` / `object-stat` / `object-get` host surface, so the kernel
//! tests can prove the object plumbing (same-dispatch put→stat/get overlay
//! hits, absent-id `None` resolution, the object-read budget, and staged-put
//! discard on abort). It calls the raw host imports directly (the `GuestOdb`
//! ObjectStore wrapper in `guest-adapter` is proven separately by compile).

wit_bindgen::generate!({
    world: "module",
    path: "../../kernel/module-guest/wit",
});

use ducktape::module::host;

struct Component;

const ID_KEY: &[u8] = b"id";

/// the tagged body a put stores under `id`: `kind ‖ body`.
fn tagged(kind: u8, body: &[u8]) -> Vec<u8> {
    let mut t = Vec::with_capacity(1 + body.len());
    t.push(kind);
    t.extend_from_slice(body);
    t
}

/// a distinct 32-byte id for budget-probe read `i` (never put, so absent).
fn distinct_id(i: u64) -> Vec<u8> {
    let mut id = vec![0u8; 32];
    id[..8].copy_from_slice(&i.to_le_bytes());
    id
}

impl Guest for Component {
    fn execute(payload: Vec<u8>) -> Result<(), host::Error> {
        match payload.split_first() {
            // 'p' kind body.. — put, then stat AND get the returned id IN THE
            // SAME DISPATCH: both must answer from the staged overlay (no pause,
            // no backing). rejects if either disagrees with the put.
            Some((b'p', rest)) => {
                let (&kind, body) = rest
                    .split_first()
                    .ok_or_else(|| host::Error::Rejected("put needs a kind byte".into()))?;
                let id = host::object_put(kind, body);
                let want = tagged(kind, body);

                let stat = host::object_stat(&id);
                if stat != Some((kind, body.len() as u64)) {
                    return Err(host::Error::Rejected("same-dispatch stat missed the put".into()));
                }
                let got = host::object_get(&id);
                if got.as_deref() != Some(want.as_slice()) {
                    return Err(host::Error::Rejected("same-dispatch get missed the put".into()));
                }

                host::state_set(ID_KEY, &id);
                Ok(())
            }
            // 'a' id(32) — assert the id is ABSENT: get/stat/has all empty. a
            // trap loop (instead of a real None) would blow the budget or hang
            // rather than reach this Ok.
            Some((b'a', id)) => {
                if id.len() != 32 {
                    return Err(host::Error::Rejected("absent probe needs a 32-byte id".into()));
                }
                if host::object_stat(id).is_some() {
                    return Err(host::Error::Rejected("expected-absent id had a stat".into()));
                }
                if host::object_get(id).is_some() {
                    return Err(host::Error::Rejected("expected-absent id had a body".into()));
                }
                Ok(())
            }
            // 'b' + le-u64 count — perform `count` DISTINCT object stats: the
            // object-read budget probe. every id is absent, so each read is a
            // fresh memo entry against the budget.
            Some((b'b', rest)) => {
                let n = match rest.len() {
                    8 => {
                        let mut b = [0u8; 8];
                        b.copy_from_slice(rest);
                        u64::from_le_bytes(b)
                    }
                    _ => return Err(host::Error::Rejected("count must be 8 bytes".into())),
                };
                for i in 0..n {
                    let _ = host::object_stat(&distinct_id(i));
                }
                Ok(())
            }
            _ => Err(host::Error::Rejected("unknown op".into())),
        }
    }

    fn query(req: Vec<u8>) -> Result<Vec<u8>, host::Error> {
        match req.split_first() {
            // 'i' — the id of the last 'p' put (the host cross-checks the hash).
            Some((b'i', _)) => Ok(host::state_get(ID_KEY).unwrap_or_default()),
            _ => Err(host::Error::Rejected("unknown query".into())),
        }
    }
}

export!(Component);
