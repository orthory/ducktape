//! `noop-wasm` — the smallest compliant wasm module. It implements the two
//! exports of `ducktape:module` and nothing else: every op is accepted as a
//! deterministic no-op (no state, no events, no emitted messages) and every
//! query answers empty. Its host-computed root is the empty store's root and
//! never moves. It is the admission fixture for "a module a network can take
//! in that touches nothing", and the template a new module starts from.

wit_bindgen::generate!({
    world: "module",
    path: "../../kernel/module-guest/wit",
});

use ducktape::module::host;

struct Component;

impl Guest for Component {
    fn execute(_payload: Vec<u8>) -> Result<(), host::Error> {
        Ok(())
    }

    fn query(_req: Vec<u8>) -> Result<Vec<u8>, host::Error> {
        Ok(Vec::new())
    }
}

export!(Component);
