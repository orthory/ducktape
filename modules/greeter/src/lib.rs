//! a stateless consumer module that demonstrates typed cross-module composition:
//! it READS the directory module (a live `ctx.query`) and, from what it finds,
//! WRITES a greeting to both directory and kv (typed follow-up `emit_msg`s).
//!
//! it depends only on `directory-interface` and `kv-interface` — never on the
//! directory or kv IMPL crates. that is the whole isolation thesis in one file.

use directory_interface::{decode_reply, encode_msg, encode_query, DirMsg, DirQuery, DirReply};
use kv_interface::{encode as kv_encode, KvMsg};
use sdk::{Ctx, Error, Module, ModuleId, Msg, StateRoot};

pub struct Greeter {
    id: ModuleId,
    directory: ModuleId,
    kv: ModuleId,
}

impl Greeter {
    pub fn new(id: impl Into<ModuleId>) -> Self {
        Self { id: id.into(), directory: "directory".into(), kv: "kv".into() }
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Greeter {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// stateless — contributes a constant root to the app-hash.
    fn root(&self) -> StateRoot {
        StateRoot::ZERO
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        // the payload is the directory key to greet.
        let key = String::from_utf8(msg.payload.clone())
            .map_err(|e| Error::Module(e.to_string()))?;

        // typed cross-module READ (sync, host-routed) of the directory module.
        let reply = ctx.query(&self.directory, &encode_query(&DirQuery::Get { key: key.clone() }))?;
        let name = match decode_reply(&reply).map_err(Error::Module)? {
            DirReply::Value(Some(v)) => v,
            DirReply::Value(None) => return Ok(()), // nothing to greet — no-op
        };

        let gkey = format!("greeting:{key}");
        let greeting = format!("hello {name}");

        // typed cross-module WRITEs as follow-up ops — to directory (sync-readable)
        // and to kv (the qmdb store). one module composing two others, interfaces only.
        ctx.emit_msg(Msg {
            target: self.directory.clone(),
            payload: encode_msg(&DirMsg::Set { key: gkey.clone(), value: greeting.clone() }),
        });
        ctx.emit_msg(Msg {
            target: self.kv.clone(),
            payload: kv_encode(&KvMsg::Set { key: gkey.into_bytes(), value: greeting.into_bytes() }),
        });
        Ok(())
    }
}
