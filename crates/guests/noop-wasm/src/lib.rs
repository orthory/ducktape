//! `noop-wasm` — the smallest compliant wasm module. It implements the three
//! exports of `ducktape:module` and nothing else: it declares the plainest
//! shape (a host-owned map, no network config), every op is accepted as a
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
    /// what the host must know to run this component: which substrate to
    /// wrap it over, which network parameters to seed, how queries read. a
    /// pure constant — the host reads it before any dispatch, with no env.
    fn shape() -> host::ModuleShape {
        host::ModuleShape {
            backing: host::Backing::Map,
            config: Vec::new(),
            committed_queries: false,
        }
    }

    fn execute(_payload: Vec<u8>) -> Result<(), host::Error> {
        Ok(())
    }

    fn query(_req: Vec<u8>) -> Result<Vec<u8>, host::Error> {
        Ok(Vec::new())
    }
}

export!(Component);
