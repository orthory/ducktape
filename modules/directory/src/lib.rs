//! a thin, in-memory, key→value directory module.
//!
//! deliberately NOT qmdb: its state is a `BTreeMap`, so `root()` and `query()`
//! are SYNC — which is what lets a peer module do a live cross-module `ctx.query`
//! of it without opening the async-query question. its root is a state-based hash
//! (order-independent + idempotent), the correct shape for a module commitment.

use std::collections::BTreeMap;

use directory_interface::{decode_msg, decode_query, encode_reply, DirMsg, DirQuery, DirReply};
use sdk::{Ctx, Error, Module, ModuleId, Msg, StateRoot};
use sha2::{Digest, Sha256};

pub struct Directory {
    id: ModuleId,
    entries: BTreeMap<String, String>,
}

impl Directory {
    pub fn new(id: impl Into<ModuleId>) -> Self {
        Self { id: id.into(), entries: BTreeMap::new() }
    }

    /// direct sync write (used by `execute` and handy for tests/genesis seeding).
    pub fn set(&mut self, key: String, value: String) {
        self.entries.insert(key, value);
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.entries.get(key)
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Directory {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// state-based commitment: a length-prefixed sha256 over the sorted entries.
    /// order-independent (BTreeMap) and idempotent — f(current state), unlike qmdb.
    fn root(&self) -> StateRoot {
        let mut h = Sha256::new();
        h.update((self.entries.len() as u64).to_le_bytes());
        for (k, v) in &self.entries {
            h.update((k.len() as u64).to_le_bytes());
            h.update(k.as_bytes());
            h.update((v.len() as u64).to_le_bytes());
            h.update(v.as_bytes());
        }
        StateRoot(h.finalize().into())
    }

    async fn execute(&mut self, _ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            DirMsg::Set { key, value } => self.set(key, value),
        }
        Ok(())
    }

    /// read projection — serves other modules' `ctx.query` + external reads.
    /// async per the trait, though the in-memory body has nothing to await.
    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            DirQuery::Get { key } => {
                Ok(encode_reply(&DirReply::Value(self.entries.get(&key).cloned())))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use directory_interface::encode_query;

    #[test]
    fn set_query_and_state_based_root() {
        let mut d = Directory::new("directory");
        let r0 = d.root();
        d.set("a".into(), "1".into());
        let r1 = d.root();
        assert_ne!(r0, r1, "a write must move the root");

        let reply = futures::executor::block_on(d.query(&encode_query(&DirQuery::Get { key: "a".into() }))).unwrap();
        assert_eq!(
            directory_interface::decode_reply(&reply).unwrap(),
            DirReply::Value(Some("1".into()))
        );

        // state-based: same final content -> same root regardless of history.
        let mut e = Directory::new("directory");
        e.set("a".into(), "1".into());
        assert_eq!(r1, e.root(), "root must be f(state), order/history-independent");
    }
}
