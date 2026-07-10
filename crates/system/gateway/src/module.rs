use std::collections::BTreeSet;

use identity::{
    AccountView, IdentityQuery, IdentityReply, KeyKind, MemberProof,
    decode_reply as identity_decode_reply, encode_query as identity_encode_query, verify_authority,
};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};
use valset::{
    ValsetQuery, ValsetReply, decode_reply as valset_decode_reply,
    encode_query as valset_encode_query,
};

use crate::{
    GATEWAY_ROUTE_NS, GatewayMsg, GatewayQuery, GatewayReply, MemberAuthorization, Registry,
    RouteRecord, RouteStatement, decode_msg, decode_query, encode_reply, route_signing_preimage,
};

pub struct Gateway {
    id: ModuleId,
    identity_id: ModuleId,
    valset_id: Option<ModuleId>,
    chain_id: String,
    registry: Registry,
}

impl Gateway {
    pub fn new(
        id: impl Into<ModuleId>,
        identity_id: impl Into<ModuleId>,
        valset_id: Option<ModuleId>,
        chain_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            identity_id: identity_id.into(),
            valset_id,
            chain_id: chain_id.into(),
            registry: Registry::new(),
        }
    }

    pub fn snapshot(&self) -> Vec<u8> {
        self.registry.snapshot()
    }

    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        self.registry
            .install(bytes, expected.0)
            .map_err(Error::Module)
    }

    async fn members(&self, ctx: &dyn Ctx) -> Result<Option<BTreeSet<Vec<u8>>>, Error> {
        let Some(valset_id) = &self.valset_id else {
            return Ok(None);
        };
        let validators = match valset_decode_reply(
            &ctx.query(valset_id, &valset_encode_query(&ValsetQuery::Validators))
                .await?,
        )
        .map_err(Error::Module)?
        {
            ValsetReply::Validators(nodes) => nodes,
            other => {
                return Err(Error::Module(format!(
                    "gateway: valset answered Validators with {other:?}"
                )));
            }
        };
        let residents = match valset_decode_reply(
            &ctx.query(valset_id, &valset_encode_query(&ValsetQuery::Residents))
                .await?,
        )
        .map_err(Error::Module)?
        {
            ValsetReply::Residents(nodes) => nodes,
            other => {
                return Err(Error::Module(format!(
                    "gateway: valset answered Residents with {other:?}"
                )));
            }
        };
        Ok(Some(validators.into_iter().chain(residents).collect()))
    }

    async fn account_of_node(&self, ctx: &dyn Ctx, node: &[u8]) -> Result<AccountView, Error> {
        match identity_decode_reply(
            &ctx.query(
                &self.identity_id,
                &identity_encode_query(&IdentityQuery::OfNode {
                    node_key: node.to_vec(),
                }),
            )
            .await?,
        )
        .map_err(Error::Module)?
        {
            IdentityReply::Account(Some(account)) => Ok(account),
            IdentityReply::Account(None) => Err(Error::Module(
                "gateway: publisher node is not bound to an Identity account".into(),
            )),
            other => Err(Error::Module(format!(
                "gateway: identity answered OfNode with {other:?}"
            ))),
        }
    }

    fn origin_node(ctx: &dyn Ctx) -> Result<Vec<u8>, Error> {
        match &ctx.env().origin {
            Origin::External(node) if node.len() == crate::NODE_KEY_BYTES => Ok(node.clone()),
            Origin::External(node) => Err(Error::Module(format!(
                "gateway: origin must be a {}-byte node key, got {} bytes",
                crate::NODE_KEY_BYTES,
                node.len()
            ))),
            other => Err(Error::Module(format!(
                "gateway: mutation requires an external node origin, got {other:?}"
            ))),
        }
    }

    async fn set_route(
        &mut self,
        ctx: &dyn Ctx,
        origin: &[u8],
        statement: RouteStatement,
        authorization: MemberAuthorization,
    ) -> Result<(), Error> {
        if statement.chain_id != self.chain_id {
            return Err(Error::Module(
                "gateway: route belongs to another chain".into(),
            ));
        }
        if statement.publisher_node != origin {
            return Err(Error::Module(
                "gateway: publisher does not match the authenticated origin".into(),
            ));
        }
        let account = self.account_of_node(ctx, origin).await?;
        if statement.account_id != account.account_id {
            return Err(Error::Module(
                "gateway: route account does not own the publisher node".into(),
            ));
        }
        let signer_is_current = account
            .member_keys
            .iter()
            .any(|member| member.pubkey == authorization.signer && member.kind == KeyKind::Ed25519);
        if !signer_is_current {
            return Err(Error::Module(
                "gateway: signer is not a current Ed25519 account member".into(),
            ));
        }
        let preimage = route_signing_preimage(&statement).map_err(Error::Module)?;
        let proof = MemberProof::Signature {
            sig: authorization.signature.clone(),
        };
        if !verify_authority(
            KeyKind::Ed25519,
            &authorization.signer,
            None,
            GATEWAY_ROUTE_NS,
            &preimage,
            &proof,
        ) {
            return Err(Error::Module(
                "gateway: route signature does not verify".into(),
            ));
        }
        self.registry
            .set_route(RouteRecord {
                statement,
                authorization,
            })
            .map_err(Error::Module)
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Gateway {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        StateRoot(self.registry.root_bytes())
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(self.snapshot()))
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let origin = Self::origin_node(ctx)?;
        if self
            .members(ctx)
            .await?
            .is_some_and(|members| !members.contains(&origin))
        {
            return Err(Error::Module(
                "gateway: origin is not a validator or admitted resident".into(),
            ));
        }
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            GatewayMsg::SetRoute {
                statement,
                authorization,
            } => self.set_route(ctx, &origin, statement, authorization).await,
        }
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            GatewayQuery::Get { account_id, name } => Ok(encode_reply(&GatewayReply::Route(
                self.registry
                    .route(&account_id, &name)
                    .map_err(Error::Module)?,
            ))),
        }
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        self.registry.commit();
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.registry.abort();
        Ok(())
    }
}
