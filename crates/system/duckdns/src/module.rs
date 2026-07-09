use std::collections::BTreeSet;

use duckdns_core::{
    DuckDnsMsg, DuckDnsName, DuckDnsQuery, DuckDnsReply, NODE_KEY_LEN, Registry, ResolvedService,
    ServiceScope, decode_msg, decode_query, encode_reply, validate_handle,
};
use identity::{
    IdentityQuery, IdentityReply, decode_reply as identity_decode_reply,
    encode_query as identity_encode_query,
};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};
use valset::{
    ValsetQuery, ValsetReply, decode_reply as valset_decode_reply,
    encode_query as valset_encode_query,
};

pub struct DuckDns {
    id: ModuleId,
    identity_id: ModuleId,
    /// `None` is for the single-node developer daemon. Network-shape nodes
    /// always provide the valset module id.
    valset_id: Option<ModuleId>,
    registry: Registry,
}

impl DuckDns {
    pub fn new(
        id: impl Into<ModuleId>,
        identity_id: impl Into<ModuleId>,
        valset_id: Option<ModuleId>,
        chain_id: &str,
    ) -> Result<Self, String> {
        Ok(Self {
            id: id.into(),
            identity_id: identity_id.into(),
            valset_id,
            registry: Registry::new(chain_id)?,
        })
    }

    pub fn chain_label(&self) -> &str {
        self.registry.chain_label()
    }

    pub fn snapshot(&self) -> Vec<u8> {
        self.registry.snapshot()
    }

    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        self.registry
            .install(bytes, expected.0)
            .map_err(Error::Module)
    }

    fn origin_node(ctx: &dyn Ctx) -> Result<Vec<u8>, Error> {
        match &ctx.env().origin {
            Origin::External(node) if node.len() == NODE_KEY_LEN => Ok(node.clone()),
            Origin::External(node) => Err(Error::Module(format!(
                "duckdns: origin must be a {NODE_KEY_LEN}-byte node key, got {} bytes",
                node.len()
            ))),
            other => Err(Error::Module(format!(
                "duckdns: operation requires an external node origin, got {other:?}"
            ))),
        }
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
            ValsetReply::Validators(validators) => validators,
            other => {
                return Err(Error::Module(format!(
                    "duckdns: valset answered Validators with {other:?}"
                )));
            }
        };
        let residents = match valset_decode_reply(
            &ctx.query(valset_id, &valset_encode_query(&ValsetQuery::Residents))
                .await?,
        )
        .map_err(Error::Module)?
        {
            ValsetReply::Residents(residents) => residents,
            other => {
                return Err(Error::Module(format!(
                    "duckdns: valset answered Residents with {other:?}"
                )));
            }
        };
        Ok(Some(validators.into_iter().chain(residents).collect()))
    }

    async fn require_standing(&self, ctx: &dyn Ctx, node: &[u8]) -> Result<(), Error> {
        if self
            .members(ctx)
            .await?
            .is_some_and(|members| !members.contains(node))
        {
            return Err(Error::Module(
                "duckdns: origin is not a validator or admitted resident".into(),
            ));
        }
        Ok(())
    }

    async fn account_of_node(&self, ctx: &dyn Ctx, node: &[u8]) -> Result<Option<Vec<u8>>, Error> {
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
            IdentityReply::Account(account) => Ok(account.map(|account| account.account_id)),
            other => Err(Error::Module(format!(
                "duckdns: identity answered OfNode with {other:?}"
            ))),
        }
    }

    async fn require_account(&self, ctx: &dyn Ctx, node: &[u8]) -> Result<Vec<u8>, Error> {
        self.account_of_node(ctx, node).await?.ok_or_else(|| {
            Error::Module("duckdns: origin is not bound to an identity account".into())
        })
    }

    async fn filter_resolution(
        &self,
        ctx: &dyn Ctx,
        name: &DuckDnsName,
        mut resolved: ResolvedService,
    ) -> Result<Option<ResolvedService>, Error> {
        let standing = self.members(ctx).await?;
        let owner = match &resolved.identity.scope {
            ServiceScope::User { handle } => self.registry.handle_owner(handle).map(<[u8]>::to_vec),
            ServiceScope::Network => None,
        };

        let mut providers = Vec::with_capacity(resolved.providers.len());
        for provider in resolved.providers {
            if standing
                .as_ref()
                .is_some_and(|members| !members.contains(&provider.node))
            {
                continue;
            }
            if let Some(owner) = &owner
                && self.account_of_node(ctx, &provider.node).await?.as_ref() != Some(owner)
            {
                continue;
            }
            providers.push(provider);
        }

        // Prefix labels are intentionally short and may theoretically collide.
        // A node-qualified name must resolve to exactly one eligible provider;
        // it never degrades into logical-service failover.
        if matches!(name, DuckDnsName::NodeService { .. }) && providers.len() != 1 {
            return Ok(None);
        }
        if providers.is_empty() {
            return Ok(None);
        }
        resolved.providers = providers;
        Ok(Some(resolved))
    }

    fn query_state(&self, query: DuckDnsQuery) -> Result<Vec<u8>, Error> {
        match query {
            DuckDnsQuery::Resolve { .. } => Err(Error::Module(
                "duckdns: resolution requires a host query context".into(),
            )),
            DuckDnsQuery::HandleOwner { handle } => {
                validate_handle(&handle).map_err(Error::Module)?;
                Ok(encode_reply(&DuckDnsReply::HandleOwner(
                    self.registry.handle_owner(&handle).map(<[u8]>::to_vec),
                )))
            }
            DuckDnsQuery::NodeAnnouncements { node } => Ok(encode_reply(
                &DuckDnsReply::NodeAnnouncements(self.registry.node_announcements(&node)),
            )),
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Module for DuckDns {
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
        let node = Self::origin_node(ctx)?;
        self.require_standing(ctx, &node).await?;
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            DuckDnsMsg::ClaimHandle { handle } => {
                let account = self.require_account(ctx, &node).await?;
                self.registry
                    .claim_handle(&account, handle)
                    .map_err(Error::Module)
            }
            DuckDnsMsg::ReleaseHandle { handle } => {
                let account = self.require_account(ctx, &node).await?;
                self.registry
                    .release_handle(&account, &handle)
                    .map_err(Error::Module)
            }
            DuckDnsMsg::ReplaceAnnouncements { announcements } => {
                let needs_account = announcements
                    .iter()
                    .any(|announcement| matches!(announcement.scope, ServiceScope::User { .. }));
                let account = if needs_account {
                    Some(self.require_account(ctx, &node).await?)
                } else {
                    None
                };
                self.registry
                    .replace_announcements(&node, account.as_deref(), announcements)
                    .map_err(Error::Module)
            }
        }
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        self.query_state(decode_query(req).map_err(Error::Module)?)
    }

    async fn query_with(&self, ctx: &dyn Ctx, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            DuckDnsQuery::Resolve { name } => {
                let resolved = match self.registry.resolve(&name).map_err(Error::Module)? {
                    Some(resolved) => self.filter_resolution(ctx, &name, resolved).await?,
                    None => None,
                };
                Ok(encode_reply(&DuckDnsReply::Resolved(resolved)))
            }
            query => self.query_state(query),
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
