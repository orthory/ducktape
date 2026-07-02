//! Chat module backed by the messaging storage implementation.
//!
//! Chat presents a Slack-like wire surface while delegating persistence,
//! rollback, root calculation, and state-sync to `messaging`.

use std::sync::Arc;

use chat_interface::{
    backing_msg, backing_query, decode_msg, decode_query, encode_reply, reply_from_backing,
};
use commonware_runtime::BufferPooler;
use commonware_storage::{Context, qmdb::sync::DbResolver};
use messaging::{Messaging, MessagingDb, MessagingTarget};
use messaging_interface::{
    decode_reply as decode_messaging_reply, encode_msg as encode_messaging_msg,
    encode_query as encode_messaging_query,
};
use sdk::{Ctx, Error, Module, ModuleId, Msg, StateRoot};

pub struct Chat<E>
where
    E: Context + BufferPooler,
{
    id: ModuleId,
    messaging: Messaging<E>,
}

impl<E> Chat<E>
where
    E: Context + BufferPooler,
{
    /// Open chat with a backing messaging store under the same id.
    pub async fn init(context: E, id: impl Into<ModuleId>) -> Self {
        let id = id.into();
        Self::init_with_messaging_id(context, id.clone(), id).await
    }

    /// Open chat under `id`, backed by a messaging store named `messaging_id`.
    pub async fn init_with_messaging_id(
        context: E,
        id: impl Into<ModuleId>,
        messaging_id: impl Into<ModuleId>,
    ) -> Self {
        let id = id.into();
        let messaging = Messaging::init(context, messaging_id).await;
        Self { id, messaging }
    }

    /// Wrap an existing messaging module as a chat view without copying records.
    pub fn from_messaging(id: impl Into<ModuleId>, messaging: Messaging<E>) -> Self {
        Self {
            id: id.into(),
            messaging,
        }
    }

    pub fn messaging(&self) -> &Messaging<E> {
        &self.messaging
    }

    pub fn messaging_mut(&mut self) -> &mut Messaging<E> {
        &mut self.messaging
    }

    fn registered_backing(&self, ctx: &dyn Ctx) -> Option<ModuleId> {
        let backing_id = self.messaging.id();
        if backing_id != self.id && ctx.module_root(&backing_id).is_some() {
            Some(backing_id)
        } else {
            None
        }
    }

    pub fn into_messaging(self) -> Messaging<E> {
        self.messaging
    }

    pub async fn sync_target(&self) -> MessagingTarget {
        self.messaging.sync_target().await
    }

    pub fn into_resolver(self) -> Arc<MessagingDb<E>> {
        self.messaging.into_resolver()
    }

    pub async fn sync_from<R>(
        context: E,
        id: impl Into<ModuleId>,
        target: MessagingTarget,
        resolver: R,
    ) -> Self
    where
        R: DbResolver<MessagingDb<E>>,
    {
        let id = id.into();
        Self::sync_from_messaging_id(context, id.clone(), id, target, resolver).await
    }

    pub async fn sync_from_messaging_id<R>(
        context: E,
        id: impl Into<ModuleId>,
        messaging_id: impl Into<ModuleId>,
        target: MessagingTarget,
        resolver: R,
    ) -> Self
    where
        R: DbResolver<MessagingDb<E>>,
    {
        let id = id.into();
        let messaging = Messaging::sync_from(context, messaging_id, target, resolver).await;
        Self { id, messaging }
    }
}

#[async_trait::async_trait(?Send)]
impl<E> Module for Chat<E>
where
    E: Context + BufferPooler,
{
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        self.messaging.root()
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let backing = backing_msg(decode_msg(&msg.payload).map_err(Error::Module)?);
        let payload = encode_messaging_msg(&backing);
        if let Some(target) = self.registered_backing(ctx) {
            ctx.emit_msg(Msg { target, payload });
            return Ok(());
        }
        self.messaging
            .execute(
                ctx,
                &Msg {
                    target: self.messaging.id(),
                    payload,
                },
            )
            .await
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        let backing = backing_query(decode_query(req).map_err(Error::Module)?);
        let reply = self
            .messaging
            .query(&encode_messaging_query(&backing))
            .await?;
        let reply = decode_messaging_reply(&reply).map_err(Error::Module)?;
        Ok(encode_reply(&reply_from_backing(reply)))
    }

    async fn query_with(&self, ctx: &dyn Ctx, req: &[u8]) -> Result<Vec<u8>, Error> {
        let backing = backing_query(decode_query(req).map_err(Error::Module)?);
        let reply = if let Some(target) = self.registered_backing(ctx) {
            ctx.query(&target, &encode_messaging_query(&backing))
                .await?
        } else {
            self.messaging
                .query(&encode_messaging_query(&backing))
                .await?
        };
        let reply = decode_messaging_reply(&reply).map_err(Error::Module)?;
        Ok(encode_reply(&reply_from_backing(reply)))
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        self.messaging.commit_block().await
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.messaging.abort_block().await
    }
}
