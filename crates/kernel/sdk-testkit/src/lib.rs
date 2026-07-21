//! dev-only shared test doubles for the sdk boundary traits — see Cargo.toml.
//!
//! - [`TestCtx`] — a programmable [`sdk::Ctx`]: an [`sdk::Env`] plus per-target
//!   query handlers registered via [`TestCtx::on_query`], with emitted
//!   msgs/events and `set_output` captured for assertions. Replaces the
//!   hand-rolled `TestCtx`/`CaptureCtx` doubles whose `query` was hardwired to
//!   [`sdk::Error::QueryUnsupported`], so sibling-read paths become testable.
//! - [`MemStore`] — an in-memory [`sdk::MerkleStore`] over a `BTreeMap`; `root`
//!   is sha256 over the sorted `(key, value)` pairs, the same preimage shape as
//!   `WasmModule`'s `StateBacking::Map`.
//!
//! **Rule:** never assert `MemStore` root equality against another backend
//! (e.g. `QmdbStore`) — the shape matches but the bytes are not a cross-backend
//! contract.

use std::cell::RefCell;
use std::collections::BTreeMap;

use sdk::{Ctx, Env, Error, Event, MerkleStore, Msg, Origin, ResolverSyncTarget, StateRoot, ROOT_LEN};
use sha2::{Digest as _, Sha256};

/// one registered sibling-query responder; sees the query payload bytes.
type QueryHandler = Box<dyn FnMut(&[u8]) -> Result<Vec<u8>, Error>>;

/// a programmable [`sdk::Ctx`] test double: env + per-target query handlers +
/// captured `emit_msg`/`emit_event`/`set_output`.
pub struct TestCtx {
    env: Env,
    /// target module id → its response handler. built via [`TestCtx::on_query`].
    /// each handler sits behind its own `RefCell` (not the whole map) so a
    /// handler may reentrantly query a *different* sibling without a borrow
    /// conflict — `query` takes `&self`.
    handlers: BTreeMap<String, RefCell<QueryHandler>>,
    msgs: Vec<Msg>,
    events: Vec<Event>,
    output: Option<Vec<u8>>,
}

impl TestCtx {
    /// a ctx at block `height`, `consensus_time == height` (the convention),
    /// `origin = System`. Tests needing a specific `origin`/`me` use
    /// [`TestCtx::with_env`].
    pub fn at_height(height: u64) -> Self {
        Self::with_env(Env {
            height,
            consensus_time: height,
            origin: Origin::System,
            me: "test".into(),
            protocol_version: 0,
        })
    }

    /// a ctx over an explicit [`sdk::Env`].
    pub fn with_env(env: Env) -> Self {
        Self {
            env,
            handlers: BTreeMap::new(),
            msgs: Vec::new(),
            events: Vec::new(),
            output: None,
        }
    }

    /// register the response `handler` for sibling `target`; the handler gets
    /// the query payload bytes. Chainable. A later registration for the same
    /// `target` replaces the earlier one.
    pub fn on_query(
        mut self,
        target: &str,
        handler: impl FnMut(&[u8]) -> Result<Vec<u8>, Error> + 'static,
    ) -> Self {
        self.handlers
            .insert(target.to_string(), RefCell::new(Box::new(handler)));
        self
    }

    /// the msgs captured from [`Ctx::emit_msg`], in emission order.
    pub fn msgs(&self) -> &[Msg] {
        &self.msgs
    }

    /// the events captured from [`Ctx::emit_event`], in emission order.
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// the last bytes declared via [`Ctx::set_output`], if any.
    pub fn output(&self) -> Option<&[u8]> {
        self.output.as_deref()
    }
}

#[async_trait::async_trait(?Send)]
impl Ctx for TestCtx {
    fn env(&self) -> &Env {
        &self.env
    }

    fn module_root(&self, _target: &str) -> Option<StateRoot> {
        None
    }

    async fn query(&self, target: &str, req: &[u8]) -> Result<Vec<u8>, Error> {
        match self.handlers.get(target) {
            Some(handler) => (handler.borrow_mut())(req),
            None => Err(Error::QueryUnsupported),
        }
    }

    fn emit_msg(&mut self, msg: Msg) {
        self.msgs.push(msg);
    }

    fn emit_event(&mut self, ev: Event) {
        self.events.push(ev);
    }

    fn set_output(&mut self, bytes: Vec<u8>) {
        self.output = Some(bytes);
    }
}

/// an in-memory [`sdk::MerkleStore`] over a `BTreeMap<Vec<u8>, Vec<u8>>`.
#[derive(Default)]
pub struct MemStore {
    map: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl MemStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait::async_trait(?Send)]
impl MerkleStore for MemStore {
    async fn get(&self, key: &[u8; ROOT_LEN]) -> Result<Option<Vec<u8>>, Error> {
        Ok(self.map.get(key.as_slice()).cloned())
    }

    async fn commit_batch(
        &mut self,
        writes: Vec<([u8; ROOT_LEN], Option<Vec<u8>>)>,
    ) -> Result<(), Error> {
        for (key, value) in writes {
            match value {
                Some(value) => {
                    self.map.insert(key.to_vec(), value);
                }
                None => {
                    self.map.remove(key.as_slice());
                }
            }
        }
        Ok(())
    }

    fn root(&self) -> StateRoot {
        // sha256 over the sorted (k, v) pairs — the exact preimage shape
        // `WasmModule::encode_state` uses for `StateBacking::Map` (count, then
        // len-prefixed key + len-prefixed value per pair). BTreeMap iterates
        // in sorted key order, so this is deterministic.
        let mut h = Sha256::new();
        h.update((self.map.len() as u64).to_le_bytes());
        for (k, v) in &self.map {
            h.update((k.len() as u64).to_le_bytes());
            h.update(k);
            h.update((v.len() as u64).to_le_bytes());
            h.update(v);
        }
        StateRoot(h.finalize().into())
    }

    async fn sync_target(&self) -> Result<ResolverSyncTarget, Error> {
        Err(Error::Module(
            "MerkleStore::sync_target is unsupported on MemStore — a test double has no resolver lane".into(),
        ))
    }

    async fn serve_sync(&self, _req: &[u8]) -> Result<Vec<u8>, Error> {
        Err(Error::Module(
            "MerkleStore::serve_sync is unsupported on MemStore — a test double has no sync wire".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;
    use sdk::{Ctx, Module, Msg, StateRoot};

    /// a toy module that reads a sibling and echoes the reply back as an
    /// emitted msg — enough to prove `TestCtx::on_query` serves a real sibling
    /// read, the capability the ~30 hand-rolled `QueryUnsupported` doubles lack.
    struct Toy;

    #[async_trait::async_trait(?Send)]
    impl Module for Toy {
        fn id(&self) -> sdk::ModuleId {
            "toy".into()
        }
        fn root(&self) -> StateRoot {
            StateRoot::ZERO
        }
        async fn execute(&mut self, ctx: &mut dyn Ctx, _msg: &Msg) -> Result<(), sdk::Error> {
            let reply = ctx.query("dispatch", b"ping").await?;
            ctx.emit_msg(Msg {
                target: "dispatch".into(),
                payload: reply,
            });
            Ok(())
        }
    }

    #[test]
    fn on_query_serves_a_sibling_read() {
        let mut ctx = TestCtx::at_height(9).on_query("dispatch", |req| {
            assert_eq!(req, b"ping");
            Ok(b"pong".to_vec())
        });
        let op = Msg {
            target: "toy".into(),
            payload: Vec::new(),
        };
        block_on(Toy.execute(&mut ctx, &op)).expect("execute");
        assert_eq!(ctx.msgs().len(), 1);
        assert_eq!(ctx.msgs()[0].payload, b"pong");
    }

    #[test]
    fn unregistered_target_is_query_unsupported() {
        let ctx = TestCtx::at_height(0);
        let err = block_on(ctx.query("nobody", b"")).unwrap_err();
        assert!(matches!(err, sdk::Error::QueryUnsupported));
    }

    #[test]
    fn memstore_roundtrip_root_changes_and_is_deterministic() {
        let key = [7u8; 32];
        let mut a = MemStore::new();
        let empty_root = a.root();

        block_on(a.commit_batch(vec![(key, Some(b"v".to_vec()))])).expect("commit");
        assert_eq!(block_on(a.get(&key)).expect("get"), Some(b"v".to_vec()));
        assert_ne!(a.root(), empty_root, "a write must move the root");

        // two identically-filled stores agree byte-for-byte (deterministic root).
        let mut b = MemStore::new();
        block_on(b.commit_batch(vec![(key, Some(b"v".to_vec()))])).expect("commit");
        assert_eq!(a.root(), b.root());

        // delete returns to the empty root.
        block_on(a.commit_batch(vec![(key, None)])).expect("delete");
        assert_eq!(block_on(a.get(&key)).expect("get"), None);
        assert_eq!(a.root(), empty_root);
    }
}
