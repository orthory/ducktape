//! `directory-wasm` — the wasm port of the `directory` module, the first real
//! production tenant of the module runtime.
//!
//! bytes-compatible with the native implementation by construction: the wire
//! surface is the `directory` interface crate itself, and the host-owned store
//! carries the raw utf-8 key/value bytes — so the host-computed root() and
//! snapshot encoding are BYTE-IDENTICAL to the native module's. the cutover
//! preserves the app-hash, and pre-cutover snapshots install unchanged.

wit_bindgen::generate!({
    world: "module",
    path: "../../kernel/module-guest/wit",
});

use directory::{DirMsg, DirQuery, DirReply, decode_msg, decode_query, encode_reply};
use ducktape::module::host;

struct Component;

impl Guest for Component {
    fn execute(payload: Vec<u8>) -> Result<(), host::Error> {
        match decode_msg(&payload).map_err(host::Error::Rejected)? {
            // a staged write: the host publishes it at the block boundary,
            // exactly like the native module's `stage`/`commit_block` pair.
            DirMsg::Set { key, value } => {
                host::state_set(key.as_bytes(), value.as_bytes());
                Ok(())
            }
        }
    }

    fn query(req: Vec<u8>) -> Result<Vec<u8>, host::Error> {
        match decode_query(&req).map_err(host::Error::Rejected)? {
            DirQuery::Get { key } => {
                let value = match host::state_get(key.as_bytes()) {
                    Some(bytes) => Some(String::from_utf8(bytes).map_err(|_| {
                        // unreachable through the write path (DirMsg carries
                        // Strings); fail loud rather than lie about state.
                        host::Error::Rejected("stored value is not utf-8".into())
                    })?),
                    None => None,
                };
                Ok(encode_reply(&DirReply::Value(value)))
            }
        }
    }
}

export!(Component);
